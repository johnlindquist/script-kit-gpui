import { describe, expect, test } from "bun:test";
import {
  aggregateDisposition,
  compositeEvaluator,
  validateChildReceipt,
  validateUniqueScenarioSet,
  type EvidenceIdentity,
} from "./glass-evidence-contract.ts";

const identity: EvidenceIdentity = {
  runId: "run-fresh",
  gitCommit: "abc123",
  binary: "/tmp/app",
  binarySha256: "sha256",
};

const receipt = {
  schemaVersion: 2,
  ...identity,
  scenario: "main-window",
  startedAt: "2026-07-23T00:00:00Z",
  finishedAt: "2026-07-23T00:00:01Z",
  disposition: "EVALUABLE_PASS",
  pid: 42,
  visualMatrix: { states: [{}, {}, {}, {}] },
  widthMatrix: { rows: [{}, {}, {}, {}, {}, {}] },
  initialCompleteNativeInventory: { pass: true },
  finalCompleteNativeInventory: { pass: true },
  pass: true,
};

describe("glass evidence contract", () => {
  test("accepts one exact immutable child receipt", () => {
    expect(validateChildReceipt(receipt, identity, "main-window", 0)).toEqual([]);
  });

  test("rejects stale identity and a nonzero child hidden by pass=true", () => {
    const errors = validateChildReceipt(
      { ...receipt, runId: "stale", binarySha256: "old" },
      identity,
      "main-window",
      1,
    );
    expect(errors).toContain("runId mismatch");
    expect(errors).toContain("binary SHA-256 mismatch");
    expect(errors).toContain("nonzero child exit cannot be a pass");
  });

  test("requires exactly one receipt for every named scenario", () => {
    expect(validateUniqueScenarioSet(["a", "a", "c"], ["a", "b"])).toEqual([
      "a: expected exactly one, observed 2",
      "b: expected exactly one, observed 0",
      "unexpected scenario: c",
    ]);
  });

  test("propagates invalid classifications instead of converting them to failures", () => {
    expect(aggregateDisposition([
      { pass: true, disposition: "EVALUABLE_PASS" },
      { pass: false, disposition: "INVALID_OBSERVER" },
    ])).toBe("INVALID_OBSERVER");
    expect(aggregateDisposition([
      { pass: false, disposition: "INVALID_INTERFERENCE" },
      { pass: false, disposition: "INVALID_OBSERVER" },
    ])).toBe("INVALID_INTERFERENCE");
  });

  test("only an exact all-pass set can pass", () => {
    expect(aggregateDisposition([
      { pass: true, disposition: "EVALUABLE_PASS" },
      { pass: true, disposition: "EVALUABLE_PASS" },
    ])).toBe("EVALUABLE_PASS");
    expect(aggregateDisposition([
      { pass: true, disposition: "EVALUABLE_PASS" },
      { pass: false, disposition: "EVALUABLE_FAIL" },
    ])).toBe("EVALUABLE_FAIL");
    expect(aggregateDisposition([], ["missing child"])).toBe("INVALID_SETUP");
  });

  test("composite observer crashes cannot aggregate as evaluable passes", () => {
    const evaluator = compositeEvaluator(false, true);
    expect(evaluator).toEqual({
      pass: false,
      disposition: "INVALID_OBSERVER",
    });
    expect(aggregateDisposition([
      { pass: true, disposition: "EVALUABLE_PASS" },
      evaluator,
    ])).toBe("INVALID_OBSERVER");
  });

  test("fails closed when a main child omits one required width row", () => {
    const errors = validateChildReceipt(
      { ...receipt, widthMatrix: { rows: [{}, {}, {}, {}, {}] } },
      identity,
      "main-window",
      0,
    );
    expect(errors).toContain("main-window width matrix must contain exactly six rows");
  });
});

// ---------------------------------------------------------------------------
// WP9 (glass-smoke-harness-max-info): imported lifecycle evidence must be
// identity- and integrity-matched on EVERY axis before reuse.
// ---------------------------------------------------------------------------

import { validateArtifactReference } from "./glass-evidence-contract.ts";

const LIFECYCLE_SCENARIOS = [
  "main-exit",
  "main-entry",
  "notes-entry",
  "notes-close-before-settle-reopen",
  "dictation-exit-reopen",
];

function importableReceipt() {
  return {
    binarySha256: "bin-sha",
    themeFixture: { sha256: "theme-sha" },
    backgroundFixture: {
      mode: "saturated-stripes",
      configurationSha256: "config-sha",
      displayID: 1,
    },
    helperSha256: "helper-sha",
    interference: { pass: true, disposition: "EVALUABLE_PASS" },
    scenarios: LIFECYCLE_SCENARIOS.map((name) => ({
      name,
      filmstrip: {
        receipt: {
          captureHealthPass: true,
          refreshRateHz: 120,
          frames: [
            { path: `/frames/${name}.png`, sha256: `frame-${name}` },
          ],
        },
      },
    })),
  };
}

const expectation = {
  binarySha256: "bin-sha",
  themeFixtureSha256: "theme-sha",
  backgroundFixtureMode: "saturated-stripes",
  backgroundFixtureConfigurationSha256: "config-sha",
  displayId: 1,
  refreshRateHz: 120,
  helperSha256: "helper-sha",
  requiredScenarioNames: LIFECYCLE_SCENARIOS,
};

const matchingHash = (path: string) =>
  `frame-${path.split("/").at(-1)!.replace(".png", "")}`;

describe("validateArtifactReference", () => {
  test("a fully matching receipt with intact frames is accepted", () => {
    expect(
      validateArtifactReference(importableReceipt(), expectation, {
        hashFile: matchingHash,
      }),
    ).toEqual([]);
  });

  test("a receipt for another binary is rejected", () => {
    const receipt = { ...importableReceipt(), binarySha256: "other" };
    const errors = validateArtifactReference(receipt, expectation);
    expect(errors.join(" ")).toContain("binarySha256");
  });

  test("one modified frame after import is rejected", () => {
    const errors = validateArtifactReference(
      importableReceipt(),
      expectation,
      {
        hashFile: (path) =>
          path.includes("main-entry") ? "tampered" : matchingHash(path),
      },
    );
    expect(errors.join(" ")).toContain("frame hash mismatch");
  });

  test("a frame missing on disk is rejected", () => {
    const errors = validateArtifactReference(
      importableReceipt(),
      expectation,
      { hashFile: () => null },
    );
    expect(errors.join(" ")).toContain("frame missing on disk");
  });

  test("INVALID_INTERFERENCE imported evidence remains invalid", () => {
    const receipt = importableReceipt();
    receipt.interference = {
      pass: false,
      disposition: "INVALID_INTERFERENCE",
    };
    const errors = validateArtifactReference(receipt, expectation, {
      hashFile: matchingHash,
    });
    expect(errors.join(" ")).toContain("not interference-clean");
  });

  test("a receipt lacking a required identity field is a mismatch, never a pass", () => {
    const receipt = importableReceipt();
    delete (receipt as any).backgroundFixture;
    const errors = validateArtifactReference(receipt, expectation, {
      hashFile: matchingHash,
    });
    expect(errors.join(" ")).toContain(
      "backgroundFixtureMode: receipt does not carry the field",
    );
  });

  test("a wrong or incomplete scenario set is rejected", () => {
    const receipt = importableReceipt();
    receipt.scenarios = receipt.scenarios.slice(0, 3);
    const errors = validateArtifactReference(receipt, expectation, {
      hashFile: matchingHash,
    });
    expect(errors.join(" ")).toContain("expected exactly one, observed 0");
  });

  test("capture-health failure in any scenario is rejected", () => {
    const receipt = importableReceipt();
    (receipt.scenarios[2].filmstrip.receipt as any).captureHealthPass = false;
    const errors = validateArtifactReference(receipt, expectation, {
      hashFile: matchingHash,
    });
    expect(errors.join(" ")).toContain("captureHealthPass is not true");
  });
});
