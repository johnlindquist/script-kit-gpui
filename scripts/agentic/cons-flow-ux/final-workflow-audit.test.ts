import { describe, expect, test } from "bun:test";
import { CONS_FLOW_UX_IDS, PROGRAM_IDS } from "../../devtools/consistency.ts";
import {
  parseWorkflowAuditArgs,
  summarizeWorkflowScope,
  WorkflowAuditError,
} from "./final-workflow-audit.ts";

const HEAD = "a".repeat(40);
const IDS = [...CONS_FLOW_UX_IDS].sort();

function scopeReceipt(passedCount = 0): Record<string, unknown> {
  const passing = new Set(IDS.slice(0, passedCount));
  const complete = passing.size === IDS.length;
  return {
    schemaVersion: 2,
    primitiveId: "devtools.consistency.verify-scope",
    tool: "script-kit-devtools.consistency",
    command: "consistency.verify-scope",
    classification: complete ? "ok" : "blocked-missing-primitive",
    scope: "cons-flow-ux",
    catalogTaskCount: PROGRAM_IDS.size,
    scopeTaskCount: IDS.length,
    scopePassedTaskCount: passing.size,
    missingScopeTaskIds: IDS.filter((taskId) => !passing.has(taskId)),
    taskDispositions: Object.fromEntries(IDS.map((taskId) => [
      taskId,
      passing.has(taskId) ? "EVALUABLE_PASS" : "BLOCKED_MISSING_PRIMITIVE",
    ])),
    headCommit: HEAD,
    registry: { registryVersion: 1 },
    errors: [],
    disposition: complete ? "EVALUABLE_PASS" : "BLOCKED_MISSING_PRIMITIVE",
    pass: complete,
    producerValidation: { valid: true, errors: [] },
  };
}

describe("canonical workflow completion audit", () => {
  test("a real blocked scope reports zero of 28 runtime proofs without invented test counts", () => {
    const result = summarizeWorkflowScope(scopeReceipt(), HEAD);

    expect(result.verdict).toBe("BLOCKED");
    expect(result.sourceCommit).toBe(HEAD);
    expect(result.taskCoverage).toEqual({
      expected: 28,
      passed: 0,
      taskIds: IDS,
      passedTaskIds: [],
      outstandingTaskIds: IDS,
      missingTaskIds: IDS,
    });
    expect(result.runtimeProof).toEqual({
      requiredTaskCount: 28,
      provenTaskCount: 0,
      outstandingTaskIds: IDS,
    });
    expect(result).not.toHaveProperty("focusedMatrix");
    expect(result).not.toHaveProperty("governance");
    expect(result).not.toHaveProperty("productCommit");
    expect(result).not.toHaveProperty("oracleConsultCount");
  });

  test("only all 28 canonical passing dispositions produce PASS", () => {
    const result = summarizeWorkflowScope(scopeReceipt(28), HEAD);

    expect(result.verdict).toBe("PASS");
    expect((result.taskCoverage as Record<string, unknown>).passed).toBe(28);
    expect((result.runtimeProof as Record<string, unknown>).provenTaskCount).toBe(28);
  });

  test("27 passing tasks keep the exact final task visibly outstanding", () => {
    const result = summarizeWorkflowScope(scopeReceipt(27), HEAD);

    expect(result.verdict).toBe("BLOCKED");
    expect((result.taskCoverage as Record<string, unknown>).passed).toBe(27);
    expect((result.taskCoverage as Record<string, unknown>).outstandingTaskIds).toEqual(["WF-024"]);
  });

  test("stale source commits cannot be promoted by a green historical lane", () => {
    expect(() => summarizeWorkflowScope(scopeReceipt(28), "b".repeat(40)))
      .toThrow("source commit is stale");
  });

  test("a noncanonical workflow scope cannot stand in for SAFE/WF proof", () => {
    expect(() => summarizeWorkflowScope({ ...scopeReceipt(28), scope: "cons-proof-gov" }, HEAD))
      .toThrow("canonical cons-flow-ux scope");
  });

  test.each(["focusedMatrix", "governance", "oracleConsultCount", "productCommit"])(
    "hardcoded historical %s cannot enter a canonical workflow receipt",
    (field) => {
      expect(() => summarizeWorkflowScope({ ...scopeReceipt(), [field]: { passed: 625 } }, HEAD))
        .toThrow(`fabricated legacy ${field} evidence`);
    },
  );

  test("a fabricated 28/28 count cannot hide blocked task dispositions", () => {
    expect(() => summarizeWorkflowScope({ ...scopeReceipt(), scopePassedTaskCount: 28 }, HEAD))
      .toThrow("passing count does not match actual task dispositions");
  });

  test("a passing verdict with any missing runtime task is invalid", () => {
    expect(() => summarizeWorkflowScope({
      ...scopeReceipt(27),
      disposition: "EVALUABLE_PASS",
      classification: "ok",
      pass: true,
    }, HEAD)).toThrow("canonical receipt validation");
  });

  test("missing, duplicated, and foreign workflow identities fail closed", () => {
    const receipt = scopeReceipt(28);
    const dispositions = { ...(receipt.taskDispositions as Record<string, unknown>) };
    delete dispositions["SAFE-001"];
    dispositions["UX-001"] = "EVALUABLE_PASS";

    expect(() => summarizeWorkflowScope({ ...receipt, taskDispositions: dispositions }, HEAD))
      .toThrow("exact 28 canonical SAFE/WF task identities");
  });

  test("failed producer validation cannot masquerade as direct runtime proof", () => {
    expect(() => summarizeWorkflowScope({
      ...scopeReceipt(28),
      producerValidation: { valid: false, errors: ["stale producer"] },
    }, HEAD)).toThrow("producer validation did not pass");
  });

  test("a passing task cannot also be reported as missing", () => {
    const receipt = scopeReceipt(1);
    receipt.missingScopeTaskIds = [IDS[0], ...IDS.slice(1)];

    expect(() => summarizeWorkflowScope(receipt, HEAD))
      .toThrow("passing or unknown task as missing");
  });

  test("the full current Git identity is mandatory", () => {
    expect(() => summarizeWorkflowScope(scopeReceipt(), "493769e"))
      .toThrow("exact current 40-character source commit");
  });

  test("read-only audit mode is explicit and never requires an output mutation", () => {
    const options = parseWorkflowAuditArgs(["--no-write", "--receipts", "/tmp/reviewed"]);

    expect(options.writeOutput).toBe(false);
    expect(options.receiptsRoot).toBe("/tmp/reviewed");
  });

  test("source-current workflow receipts never rewrite the tracked historical marker", () => {
    expect(parseWorkflowAuditArgs([]).outputPath).toBe(
      ".artifacts/consistency/cons-flow-ux/final-audit/current/lane-receipt.json",
    );
  });

  test("unknown arguments and missing paths fail before auditing", () => {
    expect(() => parseWorkflowAuditArgs(["--launch-app"])).toThrow(WorkflowAuditError);
    expect(() => parseWorkflowAuditArgs(["--out"])).toThrow("--out requires a path");
  });
});
