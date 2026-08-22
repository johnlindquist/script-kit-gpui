import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import type { CoverageBindingRecord } from "../surfaces.ts";
import { validateReceipt } from "./receipt-schema.ts";
import type { JsonObject } from "./privacy.ts";

export type RuntimeProofReceipt = {
  path: string;
  receipt: JsonObject;
};

export type RuntimeCoverageOptions = {
  sourceCommit?: string | null;
  binarySha256?: string | null;
  ownerValidationErrors?: readonly string[];
};

type RuntimeCoverageMapping = {
  bindingId: string;
  contractKind: string;
  appViewVariant: string;
  hostKind: string;
  fixtureFamily: string;
  staticEvidenceGrade: string;
  status:
    | "DIRECT_RUNTIME_PASS"
    | "BLOCKED_MISSING_RUNTIME_PROOF"
    | "BLOCKED_UNSUPPORTED_PRIMITIVE";
  requiredPrimitiveIds: string[];
  provenPrimitiveIds: string[];
  missingPrimitiveIds: string[];
  transactionId: string | null;
  receiptPaths: string[];
};

function record(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonObject)
    : {};
}

function isRuntimeClass(value: unknown): boolean {
  return (
    value === "RUNTIME_HIDDEN" ||
    value === "RUNTIME_VISIBLE" ||
    value === "PACKAGED_APP"
  );
}

function transactionMatchesBinding(
  binding: CoverageBindingRecord,
  transaction: JsonObject,
): boolean {
  const expected = binding.expectedTargetIdentity;
  const actualHost = transaction.hostKind;
  return (
    transaction.surfaceKind === binding.contractKind &&
    transaction.appViewVariant === binding.appViewVariant &&
    transaction.windowKind === expected.windowKind &&
    (actualHost == null || actualHost === expected.hostKind) &&
    (!expected.parentRequired ||
      (typeof transaction.parentAutomationId === "string" &&
        transaction.parentAutomationId.length > 0))
  );
}

function bestTransaction(
  candidates: Map<string, Map<string, RuntimeProofReceipt>>,
  required: readonly string[],
): [string | null, Map<string, RuntimeProofReceipt>] {
  let selectedId: string | null = null;
  let selected = new Map<string, RuntimeProofReceipt>();
  for (const [transactionId, primitives] of candidates) {
    const score = required.filter((id) => primitives.has(id)).length;
    const selectedScore = required.filter((id) => selected.has(id)).length;
    if (score > selectedScore) {
      selectedId = transactionId;
      selected = primitives;
    }
  }
  return [selectedId, selected];
}

export function buildRuntimeCoverageScorecard(
  bindings: readonly CoverageBindingRecord[],
  receipts: readonly RuntimeProofReceipt[],
  options: RuntimeCoverageOptions = {},
) {
  const rejectedReceipts: Array<{ path: string; reason: string }> = [];
  const usableReceipts: RuntimeProofReceipt[] = [];
  let privacyViolationCount = 0;
  let scenarioFailureCount = 0;
  let totalRuntimeDurationMs = 0;

  for (const candidate of receipts) {
    const receipt = candidate.receipt;
    const transaction = record(receipt.transaction);
    const privacy = record(receipt.privacy);
    const privacyScan = record(privacy.recursiveCanaryScan);
    const cleanup = record(receipt.cleanup);
    const survivors = Array.isArray(cleanup.survivors) ? cleanup.survivors : [];

    if (!isRuntimeClass(receipt.evidenceClass)) {
      rejectedReceipts.push({
        path: candidate.path,
        reason: "receipt is static, compile-only, or lacks an explicit runtime evidence class",
      });
      continue;
    }
    if (receipt.disposition !== "EVALUABLE_PASS" || receipt.pass !== true) {
      scenarioFailureCount += 1;
      rejectedReceipts.push({
        path: candidate.path,
        reason: "runtime scenario does not have a passing terminal disposition",
      });
      continue;
    }
    if (
      privacy.rawContentReturned === true ||
      privacyScan.performed !== true ||
      privacyScan.pass !== true ||
      Number(privacy.canaryMatches ?? 0) > 0
    ) {
      privacyViolationCount += 1;
      rejectedReceipts.push({
        path: candidate.path,
        reason: "runtime receipt has an incomplete or failing recursive privacy scan",
      });
      continue;
    }
    if (cleanup.closed !== true || survivors.length > 0) {
      rejectedReceipts.push({
        path: candidate.path,
        reason: "runtime receipt has unclosed cleanup or surviving owned processes",
      });
      continue;
    }

    const primitiveId =
      typeof receipt.primitiveId === "string" ? receipt.primitiveId : "";
    const validation = validateReceipt(primitiveId, receipt);
    if (!validation.valid || record(receipt.producerValidation).valid !== true) {
      rejectedReceipts.push({
        path: candidate.path,
        reason: "runtime receipt does not pass its authoritative producer schema",
      });
      continue;
    }
    if (
      options.sourceCommit &&
      record(receipt.repository).gitCommit !== options.sourceCommit
    ) {
      rejectedReceipts.push({
        path: candidate.path,
        reason: "runtime receipt belongs to a different source commit",
      });
      continue;
    }
    if (
      options.binarySha256 &&
      transaction.binarySha256 !== options.binarySha256
    ) {
      rejectedReceipts.push({
        path: candidate.path,
        reason: "runtime receipt belongs to a different application binary",
      });
      continue;
    }
    if (
      typeof transaction.transactionId !== "string" ||
      transaction.transactionId.length === 0
    ) {
      rejectedReceipts.push({
        path: candidate.path,
        reason: "runtime receipt has no comparable transaction identity",
      });
      continue;
    }

    usableReceipts.push(candidate);
    if (typeof receipt.durationMs === "number" && receipt.durationMs >= 0) {
      totalRuntimeDurationMs += receipt.durationMs;
    }
  }

  const mappings: RuntimeCoverageMapping[] = bindings.map((binding) => {
    const grouped = new Map<string, Map<string, RuntimeProofReceipt>>();
    for (const candidate of usableReceipts) {
      const receipt = candidate.receipt;
      const primitiveId = String(receipt.primitiveId);
      if (!binding.requiredPrimitiveIds.includes(primitiveId)) continue;
      const transaction = record(receipt.transaction);
      if (!transactionMatchesBinding(binding, transaction)) continue;
      const transactionId = String(transaction.transactionId);
      const primitives = grouped.get(transactionId) ?? new Map();
      primitives.set(primitiveId, candidate);
      grouped.set(transactionId, primitives);
    }

    const [transactionId, selected] = bestTransaction(
      grouped,
      binding.requiredPrimitiveIds,
    );
    const provenPrimitiveIds = binding.requiredPrimitiveIds.filter((id) =>
      selected.has(id),
    );
    const missingPrimitiveIds = binding.requiredPrimitiveIds.filter((id) =>
      !selected.has(id),
    );

    return {
      bindingId: binding.bindingId,
      contractKind: binding.contractKind,
      appViewVariant: binding.appViewVariant,
      hostKind: binding.hostKind,
      fixtureFamily: binding.fixtureFamily,
      staticEvidenceGrade: binding.evidenceGrade,
      status:
        binding.missingPrimitiveIds.length > 0
          ? "BLOCKED_UNSUPPORTED_PRIMITIVE"
          : missingPrimitiveIds.length === 0
            ? "DIRECT_RUNTIME_PASS"
            : "BLOCKED_MISSING_RUNTIME_PROOF",
      requiredPrimitiveIds: [...binding.requiredPrimitiveIds],
      provenPrimitiveIds,
      missingPrimitiveIds,
      transactionId,
      receiptPaths: provenPrimitiveIds.map((id) => selected.get(id)!.path),
    } satisfies RuntimeCoverageMapping;
  });

  const directRuntimeMappingCount = mappings.filter(
    (mapping) => mapping.status === "DIRECT_RUNTIME_PASS",
  ).length;
  const promptMappings = mappings.filter(
    (mapping) => mapping.fixtureFamily === "script-prompt",
  );
  const ownerValidationErrors = [...(options.ownerValidationErrors ?? [])];

  return {
    schemaVersion: 1,
    evidenceClass:
      directRuntimeMappingCount > 0
        ? ("DIRECT_RUNTIME_PROOF" as const)
        : ("STATIC_INVENTORY" as const),
    disposition:
      bindings.length > 0 &&
      directRuntimeMappingCount === bindings.length &&
      ownerValidationErrors.length === 0 &&
      privacyViolationCount === 0 &&
      scenarioFailureCount === 0
        ? ("EVALUABLE_PASS" as const)
        : ("BLOCKED_MISSING_PRIMITIVE" as const),
    sourceCommit: options.sourceCommit ?? null,
    binarySha256: options.binarySha256 ?? null,
    totalMappingCount: bindings.length,
    staticDirectBindingCount: bindings.filter(
      (binding) => binding.evidenceGrade === "Direct",
    ).length,
    directRuntimeMappingCount,
    supportedPromptFamilyCount: promptMappings.length,
    directlyProvenPromptFamilyCount: promptMappings.filter(
      (mapping) => mapping.status === "DIRECT_RUNTIME_PASS",
    ).length,
    unsupportedMappingCount: mappings.filter(
      (mapping) => mapping.status === "BLOCKED_UNSUPPORTED_PRIMITIVE",
    ).length,
    staleOwnerPathCount: ownerValidationErrors.length,
    ownerValidationErrors,
    scenarioFailureCount,
    privacyViolationCount,
    totalRuntimeDurationMs,
    candidateReceiptCount: receipts.length,
    acceptedReceiptCount: usableReceipts.length,
    rejectedReceipts,
    missingPrimitiveIds: [
      ...new Set(mappings.flatMap((mapping) => mapping.missingPrimitiveIds)),
    ].sort(),
    mappings,
  };
}

export function discoverRuntimeCoverageReceipts(root: string): RuntimeProofReceipt[] {
  if (!existsSync(root)) return [];
  const receipts: RuntimeProofReceipt[] = [];
  const ignoredDirectories = new Set([
    "attempts",
    "invalid",
    "history",
    "superseded",
    "baseline",
    "negative",
  ]);

  function visit(directory: string): void {
    for (const entry of readdirSync(directory)) {
      const path = join(directory, entry);
      const stats = statSync(path);
      if (stats.isDirectory()) {
        if (!ignoredDirectories.has(entry)) visit(path);
        continue;
      }
      if (!entry.endsWith(".json") || entry === "task.json") continue;
      try {
        const value = JSON.parse(readFileSync(path, "utf8"));
        const receipt = record(value);
        if (
          typeof receipt.primitiveId === "string" &&
          typeof receipt.disposition === "string"
        ) {
          receipts.push({ path, receipt });
        }
      } catch {
        receipts.push({
          path,
          receipt: { evidenceClass: "UNREADABLE_RUNTIME_RECEIPT" },
        });
      }
    }
  }

  visit(root);
  return receipts;
}
