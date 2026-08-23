import { describe, expect, test } from "bun:test";
import { evidenceIntersectionRatio, isValidEvidenceRect } from "./lib/geometry-evidence.ts";

const bounds = { x: 10, y: 20, width: 100, height: 40 };

describe("shared proof rectangle evidence", () => {
  test("finite positive observed rectangles are valid without coercion", () => {
    expect(isValidEvidenceRect(bounds)).toBe(true);
    expect(isValidEvidenceRect({ ...bounds, x: "10" })).toBe(false);
    expect(isValidEvidenceRect({ ...bounds, x: NaN })).toBe(false);
    expect(isValidEvidenceRect({ ...bounds, width: Infinity })).toBe(false);
  });

  test("zero-area visibility is explicit and negative geometry is never valid", () => {
    expect(isValidEvidenceRect({ ...bounds, width: 0 })).toBe(false);
    expect(isValidEvidenceRect({ ...bounds, width: 0 }, true)).toBe(true);
    expect(isValidEvidenceRect({ ...bounds, height: -1 }, true)).toBe(false);
    expect(isValidEvidenceRect(null, true)).toBe(false);
  });

  test("intersection ratios distinguish full, clipped, absent, and malformed paint", () => {
    expect(evidenceIntersectionRatio(bounds, bounds)).toBe(1);
    expect(evidenceIntersectionRatio(bounds, { ...bounds, width: 50 })).toBe(0.5);
    expect(evidenceIntersectionRatio(bounds, { ...bounds, x: 1000 })).toBe(0);
    expect(evidenceIntersectionRatio({ ...bounds, width: NaN }, bounds)).toBe(0);
  });
});
