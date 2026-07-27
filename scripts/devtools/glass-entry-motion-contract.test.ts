import { describe, expect, test } from "bun:test";
import {
  ACTIONS_GLASS_ENTRY_EXPECTATION,
  MAIN_GLASS_ENTRY_EXPECTATION,
  analyzeEntryMotionEnvelope,
  analyzeLoggedEntryGeometry,
} from "./glass-entry-motion-contract.ts";

const settled = [[381, 166], [750, 480]] as [[number, number], [number, number]];

/** A healthy main-window visible-tail capture: 101.2% → 98.7% → 100%. */
const mainTailFrames = [
  { sequence: 0, windowAlpha: 0.85, windowBounds: [[377, 166], [759, 480]] },
  { sequence: 1, windowAlpha: 0.93, windowBounds: [[378, 166], [755, 480]] },
  { sequence: 2, windowAlpha: 0.99, windowBounds: [[380, 166], [750.5, 480]] },
  { sequence: 3, windowAlpha: 0.99, windowBounds: [[382, 166], [740.5, 480]] },
  { sequence: 4, windowAlpha: 0.99, windowBounds: [[382, 166], [741.5, 480]] },
  { sequence: 5, windowAlpha: 0.995, windowBounds: [[381, 166], [745, 480]] },
  { sequence: 6, windowAlpha: 1.0, windowBounds: [[381, 166], [748, 480]] },
  { sequence: 7, windowAlpha: 1.0, windowBounds: [[381, 166], [750, 480]] },
] as const;

const NEW_MAIN_LOG =
  "event=glass_morph window=Main window variant=window_frame phase=enter duration=0.21s inset=0.006 start_alpha=0.85 start_alpha_bits=3feb333333333333 settle_duration_ns=209999993 configured_at_host_time_ns=1 expected_settle_deadline_ns=209999994 frames=759x480->740x480->750x480 start_scale_x=1.012000 start_scale_y=1.000000 squish_scale_x=0.987000 squish_scale_y=1.000000 phase1_ns=69999998 hold_ns=0 phase2_ns=139999995 alpha_phase1_target=0.990000 alpha_ramp_ns=35000000 alpha_finish_ns=52000000 geometry_curve=easeOut rebound_curve=easeInEaseOut alpha_curve=easeOut";

describe("visible-tail glass entry motion contract", () => {
  test("accepts the predicted main visible-tail frames", () => {
    const result = analyzeEntryMotionEnvelope(
      [...mainTailFrames],
      settled,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(result.errors).toEqual([]);
    expect(result.underResolved).toBe(false);
    expect(result.pass).toBe(true);
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
      "phase2_ns=139999995",
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
