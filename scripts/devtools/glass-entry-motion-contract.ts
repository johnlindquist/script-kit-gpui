/**
 * Glass entry-motion contract — the visible-tail calibration.
 *
 * Shape retuned 2026-07-26 (glass-entry-spotlight-retune): the first safe
 * frame (NSWindow alpha 0.85) is phase-aligned to Spotlight's measured
 * materialized state (101.2% width), ease-out compression, no hold,
 * ease-in-out rebound at a 1:2 ratio, alpha 0.85 → 0.99 → 1.0.
 * Every duration was then HALVED on 2026-07-27 at explicit user request
 * ("2x faster"): 35ms compression, 70ms rebound, 18ms alpha ramp, 26ms
 * alpha finish. Reference shape: https://eager-hollow-dyyf.here.now/
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

// 2026-08-13 soft-materialize retune (user-supplied 57fps Spotlight
// reference): the FIRST VISIBLE frame is 103.05% of settled width and eases
// to the preserved 101.2% visible-tail start over 18ms inside the 44ms
// material prefix; main's onset defocus is 12pt resolving to 0 across that
// prefix. Actions/popups stay tail-aligned with the historical 8pt/full-entry
// ramp.
export const MAIN_GLASS_ENTRY_EXPECTATION = {
  durationMs: 105,
  materialOnsetMs: 44,
  compressionMs: 35,
  holdMs: 0,
  reboundMs: 70,
  onsetGeometryMs: 18,
  onsetGeometryToleranceMs: 18,
  firstVisibleWidthScale: 1.0305,
  startWidthScale: 1.012,
  extremeWidthScale: 0.987,
  finalWidthScale: 1,
  startHeightScale: 1,
  extremeHeightScale: 1,
  startAlpha: 0.85,
  phase1Alpha: 0.99,
  alphaRampMs: 18,
  alphaFinishMs: 26,
  contentHoldMs: 0,
  contentFadeMs: 44,
  entryBlurRadius: 12,
  entryBlurToRadius: 0,
  entryBlurDurationMs: 44,
  footerBlurRadius: 12,
  footerBlurToRadius: 0,
  footerBlurScope: "per_capsule",
  footerBlurDurationMs: 44,
  footerMinimumCapsuleCount: 1,
  footerEnrolled: false,
  direction: "shrink-in",
} as const;

export const ACTIONS_GLASS_ENTRY_EXPECTATION = {
  ...MAIN_GLASS_ENTRY_EXPECTATION,
  onsetGeometryMs: 0,
  firstVisibleWidthScale: 0.988,
  startWidthScale: 0.988,
  extremeWidthScale: 1.013,
  entryBlurRadius: 8,
  entryBlurDurationMs: 149,
  footerBlurRadius: 0,
  footerMinimumCapsuleCount: 0,
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
    || Math.abs(first.widthScale - expectation.firstVisibleWidthScale) > widthTolerance
  ) {
    errors.push(
      `first visible width scale must be ${expectation.firstVisibleWidthScale.toFixed(4)} ±${widthTolerance.toFixed(4)}`,
    );
  }
  if (first != null && (first.alpha < 0.84 || first.alpha > 0.88)) {
    errors.push(
      `first visible alpha ${first.alpha.toFixed(3)} outside the 0.84–0.88 start window`,
    );
  }

  // Soft-materialize convergence: a timestamped frame inside the material
  // prefix must reach the preserved visible-tail start width near the 18ms
  // onset-geometry duration (2026-08-13 retune).
  let onsetTailVisible: (typeof measured)[number] | null = null;
  if (first != null && expectation.onsetGeometryMs > 0) {
    const firstTime = typeof first.displayTimeNs === "number"
      ? first.displayTimeNs
      : null;
    const candidates = measured.slice(1).flatMap((frame) => {
      const displayTimeNs = typeof frame.displayTimeNs === "number"
        ? frame.displayTimeNs
        : null;
      if (firstTime === null || displayTimeNs === null) return [];
      const elapsedMs = (displayTimeNs - firstTime) / 1_000_000;
      return elapsedMs > 0 && elapsedMs <= expectation.materialOnsetMs
        ? [{ frame, elapsedMs }]
        : [];
    });
    // Damage-driven ~60Hz capture samples the ease sparsely: accept a
    // converged frame OR a strictly-narrowing intermediate frame (the native
    // onset receipt proves the animation parameters independently); fail only
    // when frames exist and none narrows.
    const converged = candidates.find((candidate) =>
      Math.abs(candidate.frame.widthScale - expectation.startWidthScale) <= widthTolerance
    ) ?? null;
    const narrowing = candidates.find((candidate) =>
      first != null
      && candidate.frame.widthScale < first.widthScale - 1e-6
      && candidate.frame.widthScale > expectation.startWidthScale - widthTolerance
    ) ?? null;
    const proof = converged ?? narrowing;
    if (candidates.length === 0) {
      errors.push("no timestamped onset frame proves convergence to the visible-tail start");
    } else if (proof == null) {
      errors.push(
        `onset must converge to tail width scale ${expectation.startWidthScale.toFixed(3)} ±${widthTolerance.toFixed(4)}`,
      );
    } else {
      onsetTailVisible = proof.frame;
    }
  }
  if (expectation.direction === "shrink-in") {
    const minimum = measured.length
      ? Math.min(...measured.map((frame) => frame.widthScale))
      : null;
    // Upper edge 0.995: damage-frame sampling of the 35ms compression can
    // land its deepest frame at ~0.9947; a missing compression reads ~1.0.
    if (minimum !== null && (minimum < 0.981 || minimum > 0.995)) {
      errors.push(
        `shrink-in minimum width scale ${minimum.toFixed(4)} outside 0.981–0.995`,
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
    onsetTailVisible,
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
  const match = line.match(new RegExp(`(?:^| )${key}=([0-9.]+)(?= |$)`));
  return match ? Number(match[1]) : null;
}

function parseBooleanField(line: string, key: string): boolean | null {
  const match = line.match(new RegExp(`(?:^| )${key}=(true|false)(?= |$)`));
  return match ? match[1] === "true" : null;
}

function parseTextField(line: string, key: string): string | null {
  const match = line.match(new RegExp(`(?:^| )${key}=([A-Za-z0-9_-]+)(?= |$)`));
  return match ? match[1] : null;
}

export function analyzeOnsetReceipt(
  logLines: string[],
  expectation: GlassEntryExpectation = MAIN_GLASS_ENTRY_EXPECTATION,
) {
  const line = [...logLines].reverse().find((candidate) =>
    candidate.includes("event=native_glass_entry_onset")
  );
  if (!line) {
    return {
      present: false,
      line: null,
      supported: null,
      entryBlurRadius: null,
      entryBlurToRadius: null,
      footerBlurRadius: null,
      footerBlurToRadius: null,
      footerBlurScope: null,
      footerBlurDurationNs: null,
      footerCapsuleCount: null,
      footerBlurredCapsuleCount: null,
      footerEnrolled: null,
      entryBlurDurationNs: null,
      onsetStartWidthScale: null,
      tailStartWidthScale: null,
      onsetGeometryDurationNs: null,
      contentRootCount: null,
      onsetDurationNs: null,
      contentHoldNs: null,
      contentFadeNs: null,
      windowAlpha: null,
      errors: ["native onset receipt is missing"],
      pass: false,
    };
  }

  const receipt = {
    present: true,
    line,
    supported: parseBooleanField(line, "supported"),
    entryBlurRadius: parseExactField(line, "entry_blur_radius"),
    entryBlurToRadius: parseExactField(line, "entry_blur_to_radius"),
    footerBlurRadius: parseExactField(line, "footer_blur_radius"),
    footerBlurToRadius: parseExactField(line, "footer_blur_to_radius"),
    footerBlurScope: parseTextField(line, "footer_blur_scope"),
    footerBlurDurationNs: parseExactField(line, "footer_blur_duration_ns"),
    footerCapsuleCount: parseExactField(line, "footer_capsule_count"),
    footerBlurredCapsuleCount: parseExactField(line, "footer_blurred_capsule_count"),
    footerEnrolled: parseBooleanField(line, "footer_enrolled"),
    entryBlurDurationNs: parseExactField(line, "entry_blur_duration_ns"),
    onsetStartWidthScale: parseExactField(line, "onset_start_width_scale"),
    tailStartWidthScale: parseExactField(line, "tail_start_width_scale"),
    onsetGeometryDurationNs: parseExactField(line, "onset_geometry_duration_ns"),
    contentRootCount: parseExactField(line, "content_root_count"),
    onsetDurationNs: parseExactField(line, "duration_ns"),
    contentHoldNs: parseExactField(line, "content_hold_ns"),
    contentFadeNs: parseExactField(line, "content_fade_ns"),
    windowAlpha: parseExactField(line, "window_alpha"),
  };
  const errors: string[] = [];
  const check = (
    label: string,
    actual: number | null,
    expected: number,
    tolerance: number,
  ) => {
    if (actual === null || Math.abs(actual - expected) > tolerance) {
      errors.push(`${label}=${actual} must be ${expected} ±${tolerance}`);
    }
  };

  if (receipt.supported !== true) errors.push(`supported=${receipt.supported} must be true`);
  check("entry_blur_radius", receipt.entryBlurRadius, expectation.entryBlurRadius, 0.05);
  check("entry_blur_to_radius", receipt.entryBlurToRadius, expectation.entryBlurToRadius, 0.01);
  check(
    "entry_blur_duration_ns",
    receipt.entryBlurDurationNs,
    expectation.entryBlurDurationMs * 1_000_000,
    1_000_000,
  );
  check(
    "onset_start_width_scale",
    receipt.onsetStartWidthScale,
    expectation.firstVisibleWidthScale,
    0.0005,
  );
  check(
    "tail_start_width_scale",
    receipt.tailStartWidthScale,
    expectation.startWidthScale,
    0.0005,
  );
  check(
    "onset_geometry_duration_ns",
    receipt.onsetGeometryDurationNs,
    expectation.onsetGeometryMs * 1_000_000,
    1_000_000,
  );
  check(
    "duration_ns",
    receipt.onsetDurationNs,
    expectation.materialOnsetMs * 1_000_000,
    1_000_000,
  );
  check("content_hold_ns", receipt.contentHoldNs, expectation.contentHoldMs * 1_000_000, 1_000_000);
  check("content_fade_ns", receipt.contentFadeNs, expectation.contentFadeMs * 1_000_000, 1_000_000);
  check("window_alpha", receipt.windowAlpha, expectation.startAlpha, 0.001);
  check("footer_blur_radius", receipt.footerBlurRadius, expectation.footerBlurRadius, 0.05);
  if (
    receipt.footerBlurRadius !== null
    && receipt.entryBlurRadius !== null
    && Math.abs(receipt.footerBlurRadius - receipt.entryBlurRadius) > 0.001
    && expectation.footerMinimumCapsuleCount > 0
  ) {
    errors.push(
      `footer_blur_radius=${receipt.footerBlurRadius} must equal entry_blur_radius=${receipt.entryBlurRadius}`,
    );
  }
  check("footer_blur_to_radius", receipt.footerBlurToRadius, expectation.footerBlurToRadius, 0.01);
  if (receipt.footerBlurScope !== expectation.footerBlurScope) {
    errors.push(
      `footer_blur_scope=${receipt.footerBlurScope} must be ${expectation.footerBlurScope}`,
    );
  }
  check(
    "footer_blur_duration_ns",
    receipt.footerBlurDurationNs,
    expectation.footerBlurDurationMs * 1_000_000,
    1_000_000,
  );
  if (
    receipt.footerCapsuleCount === null
    || receipt.footerCapsuleCount < expectation.footerMinimumCapsuleCount
  ) {
    errors.push(
      `footer_capsule_count=${receipt.footerCapsuleCount} must be >= ${expectation.footerMinimumCapsuleCount}`,
    );
  }
  if (
    receipt.footerBlurredCapsuleCount === null
    || receipt.footerBlurredCapsuleCount !== receipt.footerCapsuleCount
  ) {
    errors.push(
      `footer_blurred_capsule_count=${receipt.footerBlurredCapsuleCount} must equal footer_capsule_count=${receipt.footerCapsuleCount}`,
    );
  }
  if (receipt.footerEnrolled !== expectation.footerEnrolled) {
    errors.push(
      `footer_enrolled=${receipt.footerEnrolled} must be ${expectation.footerEnrolled}`,
    );
  }

  return { ...receipt, errors, pass: errors.length === 0 };
}

/**
 * The WP0 surface/travel fields added to the entry morph line.
 *
 * These answer questions the fractional scale fields cannot: how far the window
 * actually moves in POINTS (the same percentage is a very different gesture on
 * a 750pt window than a 340pt popup), and whether AppKit still considered the
 * popup a child window when its morph was armed.
 */
export function analyzeEntrySurfaceFields(
  logLines: string[],
) {
  const line = [...logLines].reverse().find((candidate) =>
    candidate.includes("variant=window_frame")
    && candidate.includes("phase=enter")
  );
  if (!line) return { present: false, line: null };
  const text = (key: string) => {
    const match = line.match(new RegExp(`${key}=([A-Za-z_]+)`));
    return match ? match[1] : null;
  };
  return {
    present: true,
    line,
    surfaceProfile: text("surface_profile"),
    direction: text("direction"),
    travelPolicy: text("travel_policy"),
    finalWidthPt: parseExactField(line, "final_width_pt"),
    startTravelPerSidePt: parseExactField(line, "start_travel_per_side_pt"),
    extremeTravelPerSidePt: parseExactField(line, "extreme_travel_per_side_pt"),
    visibleTailDurationNs: parseExactField(line, "visible_tail_duration_ns"),
    totalEntryDurationNs: parseExactField(line, "total_entry_duration_ns"),
    parentAttachedAtArm: text("parent_attached_at_arm") === "true",
    nativeParentWindowNumber: parseExactField(line, "native_parent_window_number"),
  };
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
