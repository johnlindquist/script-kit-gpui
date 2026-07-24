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
