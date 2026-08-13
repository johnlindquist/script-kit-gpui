import { describe, expect, test } from "bun:test";
import {
  ACTIONS_GLASS_ENTRY_EXPECTATION,
  MAIN_GLASS_ENTRY_EXPECTATION,
  analyzeEntryMotionEnvelope,
  analyzeLoggedEntryGeometry,
  analyzeOnsetReceipt,
} from "./glass-entry-motion-contract.ts";

const settled = [[381, 166], [750, 480]] as [[number, number], [number, number]];

/** A healthy main-window visible-tail capture: 101.2% → 98.7% → 100%. */
const mainTailFrames = [
  {
    sequence: 0,
    displayTimeNs: 1_000_000_000,
    windowAlpha: 0.85,
    windowBounds: [[370, 166], [772.875, 480]],
  },
  {
    sequence: 1,
    displayTimeNs: 1_018_000_000,
    windowAlpha: 0.85,
    windowBounds: [[377, 166], [759, 480]],
  },
  {
    sequence: 2,
    displayTimeNs: 1_044_000_000,
    windowAlpha: 0.85,
    windowBounds: [[377, 166], [759, 480]],
  },
  {
    sequence: 3,
    displayTimeNs: 1_060_000_000,
    windowAlpha: 0.97,
    windowBounds: [[380, 166], [750.5, 480]],
  },
  {
    sequence: 4,
    displayTimeNs: 1_079_000_000,
    windowAlpha: 0.99,
    windowBounds: [[382, 166], [740.5, 480]],
  },
  {
    sequence: 5,
    displayTimeNs: 1_096_000_000,
    windowAlpha: 0.99,
    windowBounds: [[382, 166], [741.5, 480]],
  },
  {
    sequence: 6,
    displayTimeNs: 1_114_000_000,
    windowAlpha: 0.995,
    windowBounds: [[381, 166], [745, 480]],
  },
  {
    sequence: 7,
    displayTimeNs: 1_132_000_000,
    windowAlpha: 1.0,
    windowBounds: [[381, 166], [748, 480]],
  },
  {
    sequence: 8,
    displayTimeNs: 1_149_000_000,
    windowAlpha: 1.0,
    windowBounds: [[381, 166], [750, 480]],
  },
] as const;

const NEW_MAIN_LOG =
  "event=glass_morph window=Main window variant=window_frame phase=enter duration=0.10s inset=0.006 start_alpha=0.85 start_alpha_bits=3feb333333333333 settle_duration_ns=104999996 configured_at_host_time_ns=1 expected_settle_deadline_ns=104999997 frames=759x480->740x480->750x480 start_scale_x=1.012000 start_scale_y=1.000000 squish_scale_x=0.987000 squish_scale_y=1.000000 phase1_ns=34999999 hold_ns=0 phase2_ns=69999998 alpha_phase1_target=0.990000 alpha_ramp_ns=18000000 alpha_finish_ns=26000000 geometry_curve=easeOut rebound_curve=easeInEaseOut alpha_curve=easeOut";
const NEW_MAIN_ONSET_LOG =
  "event=native_glass_entry_onset primitive=material_parameters supported=true entry_blur_radius=12.00 entry_blur_to_radius=0.00 footer_blur_radius=12.00 footer_blur_to_radius=0.00 footer_blur_scope=per_capsule footer_blur_duration_ns=44000000 footer_capsule_count=4 footer_blurred_capsule_count=4 footer_material_ramp_count=4 footer_foreground_fade_count=4 footer_enrolled=false entry_blur_duration_ns=44000000 onset_start_width_scale=1.030500 tail_start_width_scale=1.012000 onset_geometry_duration_ns=18000000 from_style=clear to_style=regular duration_ns=44000000 content_root_count=4 content_hold_ns=0 content_fade_ns=44000000 content_start_alpha=0.21 window_alpha=0.85";

describe("visible-tail glass entry motion contract", () => {
  test("accepts the predicted main visible-tail frames", () => {
    const result = analyzeEntryMotionEnvelope(
      [...mainTailFrames],
      settled,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.errors).toEqual([]);
    expect(result.underResolved).toBe(false);
    expect(result.onsetTailVisible?.widthScale).toBeCloseTo(1.012, 6);
    expect(result.pass).toBe(true);
  });

  test("rejects the prior 101.2% first-visible main start", () => {
    const frames = mainTailFrames.map((frame, index) =>
      index === 0
        ? { ...frame, windowBounds: [[377, 166], [759, 480]] as any }
        : frame
    );
    const result = analyzeEntryMotionEnvelope(
      frames as any,
      settled,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.errors.some((error) => error.includes("first visible width scale"))).toBe(true);
    expect(result.pass).toBe(false);
  });

  test("accepts the reciprocal Actions grow-in frames", () => {
    const result = analyzeEntryMotionEnvelope(
      [
        { sequence: 0, windowAlpha: 0.85, windowBounds: [[385, 166], [741, 480]] },
        { sequence: 1, windowAlpha: 0.94, windowBounds: [[384, 166], [744, 480]] },
        { sequence: 2, windowAlpha: 0.99, windowBounds: [[382, 166], [749, 480]] },
        { sequence: 3, windowAlpha: 0.99, windowBounds: [[380, 166], [757, 480]] },
        { sequence: 4, windowAlpha: 0.99, windowBounds: [[380, 166], [759.5, 480]] },
        { sequence: 5, windowAlpha: 1.0, windowBounds: [[381, 166], [755, 480]] },
        { sequence: 6, windowAlpha: 1.0, windowBounds: [[381, 166], [750, 480]] },
      ],
      settled,
      ACTIONS_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.errors).toEqual([]);
    expect(result.pass).toBe(true);
  });

  test("rejects the old 106% @ 0.85 main start", () => {
    const result = analyzeEntryMotionEnvelope(
      [
        { sequence: 0, windowAlpha: 0.85, windowBounds: [[358, 160], [795, 480]] },
        ...mainTailFrames.slice(1),
      ],
      settled,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.pass).toBe(false);
  });

  test("rejects the old 94% Actions start", () => {
    const result = analyzeEntryMotionEnvelope(
      [
        { sequence: 0, windowAlpha: 0.85, windowBounds: [[400, 166], [705, 480]] },
        { sequence: 1, windowAlpha: 0.9, windowBounds: [[392, 166], [728, 480]] },
        { sequence: 2, windowAlpha: 0.99, windowBounds: [[384, 166], [748, 480]] },
        { sequence: 3, windowAlpha: 0.99, windowBounds: [[380, 166], [759, 480]] },
        { sequence: 4, windowAlpha: 1.0, windowBounds: [[381, 166], [754, 480]] },
        { sequence: 5, windowAlpha: 1.0, windowBounds: [[381, 166], [750, 480]] },
      ],
      settled,
      ACTIONS_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.pass).toBe(false);
  });

  test("rejects a fully opaque frame that is wider than natural", () => {
    const frames = mainTailFrames.map((frame, index) =>
      index === 0 ? { ...frame, windowAlpha: 1.0 } : frame,
    );
    const result = analyzeEntryMotionEnvelope(
      frames,
      settled,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(
      result.errors.some((error) => error.includes("fully opaque while wider")),
    ).toBe(true);
    expect(result.pass).toBe(false);
  });

  test("rejects height movement beyond the zero-participation envelope", () => {
    const frames = mainTailFrames.map((frame, index) =>
      index === 2
        ? { ...frame, windowBounds: [[380, 160], [750.5, 492]] as any }
        : frame,
    );
    expect(
      analyzeEntryMotionEnvelope(frames, settled, MAIN_GLASS_ENTRY_EXPECTATION)
        .pass,
    ).toBe(false);
  });

  test("flags under-resolved captures instead of passing them", () => {
    const result = analyzeEntryMotionEnvelope(
      mainTailFrames.slice(0, 3),
      settled,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.underResolved).toBe(true);
    expect(result.pass).toBe(false);
  });
});

describe("native soft-materialize onset receipt", () => {
  test("accepts measured main onset with clipped per-capsule footer parity", () => {
    const result = analyzeOnsetReceipt([NEW_MAIN_ONSET_LOG]);
    expect(result.errors).toEqual([]);
    expect(result.entryBlurRadius).toBe(12);
    expect(result.onsetStartWidthScale).toBe(1.0305);
    expect(result.tailStartWidthScale).toBe(1.012);
    expect(result.footerBlurRadius).toBe(12);
    expect(result.footerBlurToRadius).toBe(0);
    expect(result.footerBlurScope).toBe("per_capsule");
    expect(result.footerBlurDurationNs).toBe(44_000_000);
    expect(result.footerBlurredCapsuleCount).toBe(result.footerCapsuleCount);
    expect(result.footerMaterialRampCount).toBe(result.footerCapsuleCount);
    expect(result.footerForegroundFadeCount).toBe(result.footerCapsuleCount);
    expect(result.contentStartAlpha).toBe(0.21);
    expect(result.footerEnrolled).toBe(false);
    expect(result.pass).toBe(true);
  });

  test("rejects a capsule missing the material ramp, foreground fade, or content floor", () => {
    const stale = NEW_MAIN_ONSET_LOG
      .replace("footer_material_ramp_count=4", "footer_material_ramp_count=3")
      .replace("footer_foreground_fade_count=4", "footer_foreground_fade_count=0")
      .replace("content_start_alpha=0.21", "content_start_alpha=0.00");
    const result = analyzeOnsetReceipt([stale]);
    expect(result.pass).toBe(false);
    expect(
      result.errors.some((error) => error.startsWith("footer_material_ramp_count=")),
    ).toBe(true);
    expect(
      result.errors.some((error) => error.startsWith("footer_foreground_fade_count=")),
    ).toBe(true);
    expect(result.errors.some((error) => error.startsWith("content_start_alpha="))).toBe(true);
  });

  test("rejects the stale eight-point full-entry blur and old first frame", () => {
    const stale = NEW_MAIN_ONSET_LOG
      .replace("entry_blur_radius=12.00", "entry_blur_radius=8.00")
      .replace("entry_blur_duration_ns=44000000", "entry_blur_duration_ns=149000000")
      .replace("onset_start_width_scale=1.030500", "onset_start_width_scale=1.012000");
    const result = analyzeOnsetReceipt([stale]);
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.startsWith("entry_blur_radius="))).toBe(true);
    expect(result.errors.some((error) => error.startsWith("entry_blur_duration_ns="))).toBe(true);
    expect(result.errors.some((error) => error.startsWith("onset_start_width_scale="))).toBe(true);
  });

  test("rejects stale zero, container scope, partial coverage, or content fade", () => {
    const stale = NEW_MAIN_ONSET_LOG
      .replace("footer_blur_radius=12.00", "footer_blur_radius=0.00")
      .replace("footer_blur_scope=per_capsule", "footer_blur_scope=container")
      .replace("footer_blur_duration_ns=44000000", "footer_blur_duration_ns=149000000")
      .replace("footer_blurred_capsule_count=4", "footer_blurred_capsule_count=3")
      .replace("footer_enrolled=false", "footer_enrolled=true");
    const result = analyzeOnsetReceipt([stale]);
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.startsWith("footer_blur_radius="))).toBe(true);
    expect(result.errors).toContain("footer_blur_scope=container must be per_capsule");
    expect(result.errors.some((error) => error.startsWith("footer_blur_duration_ns="))).toBe(true);
    expect(
      result.errors.some((error) => error.startsWith("footer_blurred_capsule_count=")),
    ).toBe(true);
    expect(result.errors).toContain("footer_enrolled=true must be false");
  });
});

describe("exact runtime geometry receipt", () => {
  test("accepts the new exact visible-tail schema", () => {
    const result = analyzeLoggedEntryGeometry(
      [NEW_MAIN_LOG],
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.errors).toEqual([]);
    expect(result.pass).toBe(true);
  });

  test("integer frame truncation does not defeat the exact fields", () => {
    // 340-point Actions popup: integer frames cannot express 1.3%.
    const line = NEW_MAIN_LOG
      .replace("window=Main window", "window=Actions popup")
      .replace("start_scale_x=1.012000", "start_scale_x=0.988000")
      .replace("squish_scale_x=0.987000", "squish_scale_x=1.013000")
      .replace("frames=759x480->740x480->750x480", "frames=336x364->344x364->340x364");
    const result = analyzeLoggedEntryGeometry(
      [line],
      ACTIONS_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.errors).toEqual([]);
    expect(result.pass).toBe(true);
  });

  test("an old-schema log (no exact fields) fails closed", () => {
    const result = analyzeLoggedEntryGeometry(
      [
        "event=glass_morph window=Main window variant=window_frame phase=enter duration=0.28s inset=0.030 start_alpha=0.85 frames=795x492->738x477->750x480",
      ],
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.pass).toBe(false);
    expect(result.errors[0]).toContain("exact runtime scale fields missing");
  });

  test("a 50ms hold or 90ms rebound fails", () => {
    const held = NEW_MAIN_LOG.replace("hold_ns=0", "hold_ns=50000000");
    expect(
      analyzeLoggedEntryGeometry([held], MAIN_GLASS_ENTRY_EXPECTATION).pass,
    ).toBe(false);
    const shortRebound = NEW_MAIN_LOG.replace(
      "phase2_ns=69999998",
      "phase2_ns=90000000",
    );
    expect(
      analyzeLoggedEntryGeometry([shortRebound], MAIN_GLASS_ENTRY_EXPECTATION)
        .pass,
    ).toBe(false);
  });

  test("the old 106% exact start fails against the new expectation", () => {
    const wide = NEW_MAIN_LOG.replace(
      "start_scale_x=1.012000",
      "start_scale_x=1.060000",
    );
    expect(
      analyzeLoggedEntryGeometry([wide], MAIN_GLASS_ENTRY_EXPECTATION).pass,
    ).toBe(false);
  });
});
