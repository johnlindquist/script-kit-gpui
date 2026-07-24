import { describe, expect, test } from "bun:test";
import {
  classifyNativeInventory,
  deriveUniqueOwnerDelta,
} from "./glass-topology-contract.ts";
import { classifyInterference } from "./glass-interference.ts";

const main = {
  windowId: 10,
  ownerPid: 42,
  title: "",
  layer: 101,
  alpha: 1,
  onscreen: true,
  bounds: { x: 20, y: 20, width: 750, height: 480 },
};
const auxiliary = {
  windowId: 11,
  ownerPid: 42,
  title: "",
  layer: 0,
  alpha: 1,
  onscreen: false,
  bounds: { x: 0, y: 0, width: 500, height: 500 },
};

describe("complete same-PID topology", () => {
  test("accepts the main owner and known hidden GPUI auxiliary", () => {
    expect(classifyNativeInventory([main, auxiliary], 42, 10).pass).toBe(true);
  });

  test("rejects a hidden alpha-zero duplicate Notes owner", () => {
    const notes = {
      ...main,
      windowId: 12,
      title: "Notes",
      alpha: 1,
      bounds: { x: 0, y: 0, width: 350, height: 280 },
    };
    const duplicate = { ...notes, windowId: 13, alpha: 0, onscreen: false };
    expect(
      classifyNativeInventory([main, auxiliary, notes, duplicate], 42, 10).errors,
    ).toContain("Notes has 2 complete native owners");
  });

  test("rejects a footer child and an unknown same-PID owner", () => {
    const footer = {
      ...main,
      windowId: 14,
      title: "",
      bounds: { x: 20, y: 508, width: 750, height: 80 },
    };
    const unknown = { ...main, windowId: 15, title: "Mystery" };
    const errors = classifyNativeInventory(
      [main, auxiliary, footer, unknown],
      42,
      10,
    ).errors;
    expect(errors).toContain("detached footer child window present");
    expect(errors).toContain("unknown or stale same-PID native window present");
  });

  test("pins a new owner only from one complete before/after delta", () => {
    const notes = {
      ...main,
      windowId: 12,
      title: "Notes",
      bounds: { x: 0, y: 0, width: 350, height: 280 },
    };
    expect(
      deriveUniqueOwnerDelta(
        [main, auxiliary],
        [main, auxiliary, notes],
        "Notes",
        42,
        10,
      ),
    ).toMatchObject({ pass: true, candidateIds: [12] });
  });
});

describe("hammer interference classification", () => {
  test("accidental typing is INVALID_INTERFERENCE, not a product failure", () => {
    expect(classifyInterference({
      status: "ok",
      untaggedInputCount: 1,
      frontmostAppChanged: false,
      pointerDeviationPx: 0,
      targetMovedExternally: false,
    }).disposition).toBe("INVALID_INTERFERENCE");
  });

  test("a quiet stationary observer remains evaluable", () => {
    expect(classifyInterference({
      status: "ok",
      untaggedInputCount: 0,
      frontmostAppChanged: false,
      pointerDeviationPx: 0,
      targetMovedExternally: false,
    }).disposition).toBe("EVALUABLE_PASS");
  });
});
