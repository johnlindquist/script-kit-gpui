import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { AUTHORIZED_CONFLICT_COUNT } from "./consistency.ts";
import {
  GENERATED_DESIGN_CONTRACT_PATH,
  inspectCheckedInDesignConflicts,
  inspectDesignConflictLifecycle,
} from "./design-conflicts.ts";

function bundle() {
  return JSON.parse(readFileSync(GENERATED_DESIGN_CONTRACT_PATH, "utf8"));
}

describe("generated design conflict lifecycle contracts", () => {
  test("the checked-in generated artifact classifies every current production conflict", () => {
    const receipt = inspectCheckedInDesignConflicts();
    expect(receipt.pass).toBe(true);
    expect(bundle().conflicts).toHaveLength(AUTHORIZED_CONFLICT_COUNT);
    expect(receipt.observedConflictCount).toBe(AUTHORIZED_CONFLICT_COUNT);
    expect(receipt.classifiedConflictCount).toBe(AUTHORIZED_CONFLICT_COUNT);
    expect(receipt.duplicateIds).toEqual([]);
    expect(receipt.unownedHighConflicts).toEqual([]);
    expect(receipt.incompleteLifecycleRecords).toEqual([]);
    expect(receipt.unknownLifecycleKinds).toEqual([]);
    expect(receipt.unknownTaskIds).toEqual([]);
    expect(receipt.generatedArtifactSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(receipt.kindCounts.evidencePending).toBeGreaterThan(0);
    expect(receipt.kindCounts.consumerDrift).toBeGreaterThan(0);
    expect(receipt.provesRuntimeBehavior).toBe(false);
    expect(receipt.provesExporterByteEquality).toBe(false);
  });

  test("missing and duplicated conflicts can never hide behind an authorized count", () => {
    const missing = bundle();
    missing.conflicts.pop();
    expect(inspectDesignConflictLifecycle(missing).pass).toBe(false);

    const duplicate = bundle();
    duplicate.conflicts[1] = structuredClone(duplicate.conflicts[0]);
    const result = inspectDesignConflictLifecycle(duplicate);
    expect(result.observedConflictCount).toBe(AUTHORIZED_CONFLICT_COUNT);
    expect(result.duplicateIds).toEqual([duplicate.conflicts[0].id]);
    expect(result.pass).toBe(false);
  });

  test("unknown lifecycle kinds, unauthorized tasks, and forged owners fail closed", () => {
    for (const mutate of [
      (conflict: any) => { conflict.lifecycle.kind = "silentlyResolved"; },
      (conflict: any) => { conflict.lifecycle.task = "GOV-999"; },
      (conflict: any) => { conflict.lifecycle.owner = "unknown-owner"; },
      (conflict: any) => { conflict.lifecycle.intendedContract = "different contract"; },
    ]) {
      const document = bundle();
      mutate(document.conflicts[0]);
      const result = inspectDesignConflictLifecycle(document);
      expect(result.pass).toBe(false);
      expect(result.incompleteLifecycleRecords).toContain(document.conflicts[0].id);
    }
  });

  test("measurement, removal, receipt, and pending-evidence lifecycle fields are mandatory", () => {
    for (const mutate of [
      (conflict: any) => { conflict.lifecycle.modelMeasurementId = null; },
      (conflict: any) => { conflict.lifecycle.renderMeasurementId = "foreign:value"; },
      (conflict: any) => { conflict.lifecycle.removalCondition = "delete without proof"; },
      (conflict: any) => { conflict.lifecycle.lastReceipt = ".artifacts/other/task.json"; },
    ]) {
      const document = bundle();
      mutate(document.conflicts[0]);
      expect(inspectDesignConflictLifecycle(document).pass).toBe(false);
    }

    const missingBlocker = bundle();
    const pending = missingBlocker.conflicts.find(
      (conflict: any) => conflict.lifecycle.kind === "evidencePending",
    );
    pending.lifecycle.blocker = null;
    const result = inspectDesignConflictLifecycle(missingBlocker);
    expect(result.incompleteLifecycleRecords).toContain(pending.id);
    expect(result.pass).toBe(false);
  });

  test("a high-severity conflict without an owner is independently identified", () => {
    const document = bundle();
    const high = document.conflicts.find((conflict: any) => conflict.severity === "high");
    high.lifecycle.owner = "";
    const receipt = inspectDesignConflictLifecycle(document);
    expect(receipt.unownedHighConflicts).toEqual([high.id]);
    expect(receipt.pass).toBe(false);
  });
});
