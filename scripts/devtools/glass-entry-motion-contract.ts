/**
 * Glass entry-motion contract — the visible-tail calibration.
 *
 * Retuned 2026-07-26 (Oracle session glass-entry-spotlight-retune, user
 * authorized): Script Kit's first safe frame (NSWindow alpha 0.85) is
 * phase-aligned to Spotlight's measured t≈88ms state (101.2% width), not
 * Spotlight's nearly invisible ~110% first photon. 70ms ease-out
 * compression, no hold, 140ms ease-in-out rebound; alpha 0.85 → 0.99 over
 * 35ms (ease-out) and 0.99 → 1.0 over 52ms from rebound start. Reference:
 * https://eager-hollow-dyyf.here.now/
 *
 * The analyzers verify the EXACT floating-point runtime scale fields logged
 * by the morph (integer `frames=` values are diagnostics only — far too
 * coarse for a 340-point Actions popup).
 */

export type NativeWindowBounds =
  | [[number, number], [number, number]]
  | null
  | undefined;

type EntryFrame = {
  sequence?: number;
  displayTimeNs?: number | null;
  windowAlpha?: number | null;
  windowBounds?: NativeWindowBounds;
};

export const MAIN_GLASS_ENTRY_EXPECTATION = {
  durationMs: 210,
  compressionMs: 70,
  holdMs: 0,
  reboundMs: 140,
  startWidthScale: 1.012,
  extremeWidthScale: 0.987,
  finalWidthScale: 1,
  startHeightScale: 1,
  extremeHeightScale: 1,
  startAlpha: 0.85,
  phase1Alpha: 0.99,
  alphaRampMs: 35,
  alphaFinishMs: 52,
  direction: "shrink-in",
} as const;

export const ACTIONS_GLASS_ENTRY_EXPECTATION = {
  ...MAIN_GLASS_ENTRY_EXPECTATION,
  startWidthScale: 0.988,
  extremeWidthScale: 1.013,
  direction: "grow-in",
} as const;

export type GlassEntryExpectation =
  | typeof MAIN_GLASS_ENTRY_EXPECTATION
  | typeof ACTIONS_GLASS_ENTRY_EXPECTATION;

function size(bounds: NativeWindowBounds) {
  if (
    !Array.isArray(bounds)
    || !Array.isArray(bounds[1])
    || !Number.isFinite(Number(bounds[1][0]))
    || !Number.isFinite(Number(bounds[1][1]))
  ) {
    return null;
  }
  return {
    width: Number(bounds[1][0]),
    height: Number(bounds[1][1]),
  };
}

/**
 * Filmstrip capture analysis against a named expectation.
 *
 * Returns `underResolved: true` (instead of a hard product verdict) when
 * the damage-driven capture produced too few geometry samples to judge the
 * curve — callers must then prove the endpoints with the exact runtime
 * scale fields (`analyzeLoggedEntryGeometry`) rather than widening
 * tolerances (decision rule 5B, glass-entry-spotlight-retune).
 */
export function analyzeEntryMotionEnvelope(
  frames: EntryFrame[],
  settledBounds: NativeWindowBounds,
  expectation: GlassEntryExpectation,
) {
  const settled = size(settledBounds);
  const errors: string[] = [];
  if (!settled || settled.width <= 0 || settled.height <= 0) {
    return {
      pass: false,
      underResolved: false,
      errors: ["settled native bounds are missing"],
      frames: [],
    };
  }

  const measured = frames.flatMap((frame) => {
    const measuredSize = size(frame.windowBounds);
    const alpha = Number(frame.windowAlpha);
    if (!measuredSize || !Number.isFinite(alpha) || alpha < 0.001) return [];
    return [{
      sequence: frame.sequence ?? null,
      displayTimeNs: frame.displayTimeNs ?? null,
      alpha,
      widthScale: measuredSize.width / settled.width,
      heightScale: measuredSize.height / settled.height,
    }];
  });
  const first = measured[0];
  const distinctWidths = new Set(
    measured.map((frame) => frame.widthScale.toFixed(4)),
  ).size;
  const underResolved = measured.length < 6 || distinctWidths < 4;

  const widthTolerance = Math.max(0.006, 2 / settled.width);
  if (
    first == null
    || Math.abs(first.widthScale - expectation.startWidthScale) > widthTolerance
  ) {
    errors.push(
      `first visible width scale must be ${expectation.startWidthScale.toFixed(3)} ±${widthTolerance.toFixed(4)}`,
    );
  }
  if (first != null && (first.alpha < 0.84 || first.alpha > 0.88)) {
    errors.push(
      `first visible alpha ${first.alpha.toFixed(3)} outside the 0.84–0.88 start window`,
    );
  }
  if (expectation.direction === "shrink-in") {
    const minimum = measured.length
      ? Math.min(...measured.map((frame) => frame.widthScale))
      : null;
    if (minimum !== null && (minimum < 0.981 || minimum > 0.993)) {
      errors.push(
        `shrink-in minimum width scale ${minimum.toFixed(4)} outside 0.981–0.993`,
      );
    }
    // Spotlight invariant: never fully opaque while wider than natural.
    if (
      measured.some((frame) => frame.alpha >= 0.999 && frame.widthScale > 1.002)
    ) {
      errors.push(
        "a frame was fully opaque while wider than natural size (Spotlight never is)",
      );
    }
  } else {
    const maximum = measured.length
      ? Math.max(...measured.map((frame) => frame.widthScale))
      : null;
    if (maximum !== null && (maximum < 1.007 || maximum > 1.019)) {
      errors.push(
        `grow-in maximum width scale ${maximum.toFixed(4)} outside 1.007–1.019`,
      );
    }
  }
  if (
    measured.some((frame) =>
      frame.heightScale < 0.997 || frame.heightScale > 1.003
    )
  ) {
    errors.push("height escaped the 0.997–1.003 envelope (height participation must be 0)");
  }

  return {
    expectation,
    settled,
    firstVisible: first ?? null,
    minimumWidthScale: measured.length
      ? Math.min(...measured.map((frame) => frame.widthScale))
      : null,
    maximumWidthScale: measured.length
      ? Math.max(...measured.map((frame) => frame.widthScale))
      : null,
    minimumHeightScale: measured.length
      ? Math.min(...measured.map((frame) => frame.heightScale))
      : null,
    maximumHeightScale: measured.length
      ? Math.max(...measured.map((frame) => frame.heightScale))
      : null,
    distinctWidths,
    measuredFrameCount: measured.length,
    underResolved,
    frames: measured,
    errors,
    pass: errors.length === 0 && !underResolved,
  };
}

function parseExactField(line: string, key: string): number | null {
  const match = line.match(new RegExp(`${key}=([0-9.]+)`));
  return match ? Number(match[1]) : null;
}

/**
 * Verify the morph's EXACT logged calibration against a named expectation.
 * A binary that logs only the old integer schema fails closed (old-schema
 * receipts cannot prove the visible-tail calibration).
 */
export function analyzeLoggedEntryGeometry(
  logLines: string[],
  expectation: GlassEntryExpectation,
) {
  const line = [...logLines].reverse().find((candidate) =>
    candidate.includes("variant=window_frame")
    && candidate.includes("phase=enter")
  );
  if (!line) {
    return {
      pass: false,
      errors: ["runtime entry geometry log is missing"],
      line: null,
    };
  }
  const errors: string[] = [];
  const startScaleX = parseExactField(line, "start_scale_x");
  const startScaleY = parseExactField(line, "start_scale_y");
  const squishScaleX = parseExactField(line, "squish_scale_x");
  const squishScaleY = parseExactField(line, "squish_scale_y");
  const phase1Ns = parseExactField(line, "phase1_ns");
  const holdNs = parseExactField(line, "hold_ns");
  const phase2Ns = parseExactField(line, "phase2_ns");
  const alphaTarget = parseExactField(line, "alpha_phase1_target");
  const alphaRampNs = parseExactField(line, "alpha_ramp_ns");
  const alphaFinishNs = parseExactField(line, "alpha_finish_ns");
  const startAlpha = parseExactField(line, "start_alpha");

  if (
    startScaleX === null
    || squishScaleX === null
    || phase1Ns === null
    || phase2Ns === null
  ) {
    return {
      pass: false,
      errors: [
        "exact runtime scale fields missing — old binary schema cannot prove the visible-tail calibration",
      ],
      line,
    };
  }

  const scaleEpsilon = 0.0005;
  // f32 theme durations quantize the nanosecond phases (0.21f32/3 →
  // 69,999,998ns); tolerate 1ms of quantization, never more.
  const nsEpsilon = 1_000_000;
  const check = (
    label: string,
    actual: number | null,
    expected: number,
    epsilon: number,
  ) => {
    if (actual === null || Math.abs(actual - expected) > epsilon) {
      errors.push(`${label}=${actual} must be ${expected} ±${epsilon}`);
    }
  };
  check("start_scale_x", startScaleX, expectation.startWidthScale, scaleEpsilon);
  check("start_scale_y", startScaleY, expectation.startHeightScale, scaleEpsilon);
  check("squish_scale_x", squishScaleX, expectation.extremeWidthScale, scaleEpsilon);
  check("squish_scale_y", squishScaleY, expectation.extremeHeightScale, scaleEpsilon);
  check("phase1_ns", phase1Ns, expectation.compressionMs * 1_000_000, nsEpsilon);
  check("hold_ns", holdNs, expectation.holdMs * 1_000_000, nsEpsilon);
  check("phase2_ns", phase2Ns, expectation.reboundMs * 1_000_000, nsEpsilon);
  check("alpha_phase1_target", alphaTarget, expectation.phase1Alpha, 0.000001);
  check("alpha_ramp_ns", alphaRampNs, expectation.alphaRampMs * 1_000_000, nsEpsilon);
  check(
    "alpha_finish_ns",
    alphaFinishNs,
    expectation.alphaFinishMs * 1_000_000,
    nsEpsilon,
  );
  check("start_alpha", startAlpha, expectation.startAlpha, 0.001);
  for (const curve of [
    "geometry_curve=easeOut",
    "rebound_curve=easeInEaseOut",
    "alpha_curve=easeOut",
  ]) {
    if (!line.includes(curve)) errors.push(`missing ${curve}`);
  }

  // Integer frames= remain useful diagnostics but are never the proof.
  const frameMatch = line.match(/frames=(\d+)x(\d+)->(\d+)x(\d+)->(\d+)x(\d+)/);
  return {
    line,
    expectation,
    startScaleX,
    startScaleY,
    squishScaleX,
    squishScaleY,
    phase1Ns,
    holdNs,
    phase2Ns,
    alphaTarget,
    alphaRampNs,
    alphaFinishNs,
    startAlpha,
    integerFrames: frameMatch ? frameMatch.slice(1).map(Number) : null,
    errors,
    pass: errors.length === 0,
  };
}
