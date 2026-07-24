import { describe, expect, test } from "bun:test";
import {
  analyzeEntryMotionEnvelope,
  analyzeLoggedEntryGeometry,
} from "./glass-entry-motion-contract.ts";

const settled = [[381, 166], [750, 480]] as [[number, number], [number, number]];

describe("locked glass entry motion envelope", () => {
  test("accepts the measured main-window 106% shrink-in", () => {
    const result = analyzeEntryMotionEnvelope([
      { sequence: 0, windowAlpha: 0.007, windowBounds: [[358, 160], [795, 492]] },
      { sequence: 1, windowAlpha: 0.22, windowBounds: [[365, 162], [781, 489]] },
      { sequence: 2, windowAlpha: 0.58, windowBounds: [[377, 165], [757, 482]] },
    ], settled, 1.06);
    expect(result.pass).toBe(true);
  });

  test("accepts the measured Actions 94% grow-in", () => {
    const result = analyzeEntryMotionEnvelope([
      { sequence: 0, windowAlpha: 0.01, windowBounds: [[400, 180], [705, 469]] },
      { sequence: 1, windowAlpha: 0.28, windowBounds: [[392, 174], [728, 475]] },
      { sequence: 2, windowAlpha: 0.72, windowBounds: [[384, 168], [748, 480]] },
    ], settled, 0.94);
    expect(result.pass).toBe(true);
  });

  test("rejects the regressed 180% main and 20% Actions starts", () => {
    expect(analyzeEntryMotionEnvelope([
      { sequence: 0, windowAlpha: 0.12, windowBounds: [[81, 89], [1350, 634]] },
      { sequence: 1, windowAlpha: 0.3, windowBounds: [[100, 95], [1200, 600]] },
      { sequence: 2, windowAlpha: 0.6, windowBounds: [[200, 120], [1000, 550]] },
    ], settled, 1.06).pass).toBe(false);
    expect(analyzeEntryMotionEnvelope([
      { sequence: 0, windowAlpha: 0.12, windowBounds: [[650, 350], [150, 96]] },
      { sequence: 1, windowAlpha: 0.3, windowBounds: [[600, 320], [300, 192]] },
      { sequence: 2, windowAlpha: 0.6, windowBounds: [[500, 250], [500, 320]] },
    ], settled, 0.94).pass).toBe(false);
  });

  test("locks the real Actions runtime geometry receipt", () => {
    expect(analyzeLoggedEntryGeometry([
      "event=glass_morph window=Actions popup variant=window_frame phase=enter duration=0.28s inset=0.030 start_alpha=0.00 frames=319x351->350x364->340x360",
    ], 0.94).pass).toBe(true);
    expect(analyzeLoggedEntryGeometry([
      "event=glass_morph window=Actions popup variant=window_frame phase=enter duration=0.39s inset=0.400 start_alpha=1.00 frames=68x204->360x307->340x300",
    ], 0.94).pass).toBe(false);
  });
});
