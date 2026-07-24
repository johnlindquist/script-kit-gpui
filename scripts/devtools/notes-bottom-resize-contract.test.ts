import { describe, expect, test } from "bun:test";
import { validateNotesBottomResizeReceipt } from "./notes-bottom-resize-contract.ts";

function validReceipt(): any {
  const region = {
    group: "notes-footer",
    index: 0,
    elementId: "glass-capsule-notes-footer-0",
    bounds: { x: 40, y: 250, width: 80, height: 28 },
  };
  return {
    schemaVersion: 1,
    disposition: "EVALUABLE_PASS",
    edgeTrial: {
      pass: true,
      distinctHeights: 40,
      result: { untaggedInputCount: 0 },
    },
    shrinkTrial: {
      pass: true,
      distinctHeights: 30,
      result: { untaggedInputCount: 0 },
    },
    resizedFooterHitRegions: { regions: [region] },
    buttonTrials: [{
      region,
      pass: true,
      route: { route: "protectedFooterButton" },
      noFrameChange: true,
      noAction: true,
      result: { untaggedInputCount: 0 },
    }],
    persistence: { pass: true },
    topology: { visibleNotesOwners: [{ windowId: 42 }] },
    screenshots: Array.from({ length: 5 }, (_, index) => ({
      path: `/tmp/${index}.png`,
      sha256: "a".repeat(64),
    })),
    cleanedUp: true,
  };
}

describe("Notes bottom-resize receipt contract", () => {
  test("accepts complete real-edge and protected-button evidence", () => {
    expect(validateNotesBottomResizeReceipt(validReceipt())).toEqual({
      pass: true,
      failures: [],
    });
  });

  test("rejects a false green when a button changes the frame", () => {
    const receipt = validReceipt();
    receipt.buttonTrials[0].noFrameChange = false;
    const result = validateNotesBottomResizeReceipt(receipt);
    expect(result.pass).toBe(false);
    expect(result.failures).toContain("button trial 0 changed the Notes frame");
  });

  test("rejects missing inventory and untagged input", () => {
    const receipt = validReceipt();
    receipt.buttonTrials = [];
    receipt.edgeTrial.result.untaggedInputCount = 1;
    const result = validateNotesBottomResizeReceipt(receipt);
    expect(result.pass).toBe(false);
    expect(result.failures).toContain(
      "every resized footer region must have one button-origin trial",
    );
    expect(result.failures).toContain("edge trials observed untagged input");
  });
});
