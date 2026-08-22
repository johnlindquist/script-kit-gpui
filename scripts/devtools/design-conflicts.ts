#!/usr/bin/env bun
/**
 * Inspect the checked-in generated design contract without regenerating it.
 *
 * This proves the existing conflict lifecycle records only. Reading a checked-in
 * artifact is NOT proof that rerunning the Rust exporter produces identical
 * bytes; the independent generated-byte-compare obligation stays blocked.
 */
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { AUTHORIZED_CONFLICT_COUNT } from "./consistency.ts";
import { taskProofPolicy } from "./lib/task-proof-policy.ts";

export const GENERATED_DESIGN_CONTRACT_PATH =
  "design/mockups/generated/tokens.json";

const lifecycleKinds = new Set([
  "intentionalFact",
  "modelDrift",
  "consumerDrift",
  "evidencePending",
  "compatibility",
]);

type JsonObject = Record<string, unknown>;

function asObject(value: unknown): JsonObject | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : null;
}

function nonempty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

export function inspectDesignConflictLifecycle(
  bundle: unknown,
  source: { path?: string; sha256?: string } = {},
) {
  const document = asObject(bundle);
  const conflicts = Array.isArray(document?.conflicts)
    ? document.conflicts
    : [];
  const ids = new Set<string>();
  const duplicateIds = new Set<string>();
  const unownedHighConflicts = new Set<string>();
  const incompleteLifecycleRecords = new Set<string>();
  const unknownLifecycleKinds = new Set<string>();
  const unknownTaskIds = new Set<string>();
  const kindCounts: Record<string, number> = {};
  let classifiedConflictCount = 0;

  for (const [index, candidate] of conflicts.entries()) {
    const conflict = asObject(candidate);
    const id = nonempty(conflict?.id)
      ? conflict.id
      : `(missing-conflict-id:${index})`;
    const lifecycle = asObject(conflict?.lifecycle);
    if (ids.has(id)) duplicateIds.add(id);
    ids.add(id);

    const owner = lifecycle?.owner;
    if (conflict?.severity === "high" && !nonempty(owner)) {
      unownedHighConflicts.add(id);
    }

    const kind = lifecycle?.kind;
    if (!nonempty(kind) || !lifecycleKinds.has(kind)) {
      unknownLifecycleKinds.add(id);
    } else {
      kindCounts[kind] = (kindCounts[kind] ?? 0) + 1;
    }

    const task = lifecycle?.task;
    if (!nonempty(task) || taskProofPolicy(task) === null) {
      unknownTaskIds.add(id);
    }

    const complete =
      conflict !== null &&
      nonempty(conflict.id) &&
      nonempty(conflict.explanation) &&
      lifecycle !== null &&
      nonempty(kind) &&
      lifecycleKinds.has(kind) &&
      nonempty(task) &&
      taskProofPolicy(task) !== null &&
      owner === `design-contract:${task}` &&
      lifecycle.intendedContract === conflict.explanation &&
      nonempty(lifecycle.modelMeasurementId) &&
      lifecycle.modelMeasurementId.startsWith(`${id}:`) &&
      nonempty(lifecycle.renderMeasurementId) &&
      lifecycle.renderMeasurementId.startsWith(`${id}:`) &&
      nonempty(lifecycle.removalCondition) &&
      lifecycle.removalCondition.includes(task) &&
      lifecycle.lastReceipt === `.artifacts/consistency/${task}/task.json` &&
      (kind === "evidencePending"
        ? nonempty(lifecycle.blocker)
        : lifecycle.blocker === null) &&
      (conflict.severity !== "high" || kind === "consumerDrift") &&
      (conflict.severity !== "warning" || kind === "modelDrift");

    if (complete) classifiedConflictCount += 1;
    else incompleteLifecycleRecords.add(id);
  }

  const observedConflictCount = conflicts.length;
  return {
    evidenceClass: "STATIC_INVENTORY",
    provesRuntimeBehavior: false,
    provesExporterByteEquality: false,
    generatedArtifactPath: source.path ?? GENERATED_DESIGN_CONTRACT_PATH,
    generatedArtifactSha256: source.sha256 ?? null,
    bundleHash: document?.bundleHash ?? null,
    observedConflictCount,
    classifiedConflictCount,
    authorizedConflictCount: AUTHORIZED_CONFLICT_COUNT,
    duplicateIds: [...duplicateIds],
    unownedHighConflicts: [...unownedHighConflicts],
    incompleteLifecycleRecords: [...incompleteLifecycleRecords],
    unknownLifecycleKinds: [...unknownLifecycleKinds],
    unknownTaskIds: [...unknownTaskIds],
    kindCounts,
    pass:
      observedConflictCount === AUTHORIZED_CONFLICT_COUNT &&
      classifiedConflictCount === AUTHORIZED_CONFLICT_COUNT &&
      duplicateIds.size === 0 &&
      unownedHighConflicts.size === 0 &&
      incompleteLifecycleRecords.size === 0 &&
      unknownLifecycleKinds.size === 0 &&
      unknownTaskIds.size === 0 &&
      /^sha256:[a-f0-9]{64}$/.test(String(document?.bundleHash ?? "")),
  };
}

export function inspectCheckedInDesignConflicts() {
  const bytes = readFileSync(GENERATED_DESIGN_CONTRACT_PATH);
  return inspectDesignConflictLifecycle(JSON.parse(bytes.toString("utf8")), {
    path: GENERATED_DESIGN_CONTRACT_PATH,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  });
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    console.log(
      "Usage: bun scripts/devtools/design-conflicts.ts [--out .artifacts/consistency/GOV-005/conflicts.json]",
    );
    process.exit(0);
  }
  const outputIndex = args.indexOf("--out");
  const output = outputIndex >= 0 ? args[outputIndex + 1] : null;
  if (args.length !== (output === null ? 0 : 2)) {
    console.error("only an optional --out <ignored GOV-005 receipt> is supported");
    process.exit(64);
  }
  const receipt = inspectCheckedInDesignConflicts();
  if (!receipt.pass) {
    console.error(JSON.stringify(receipt, null, 2));
    process.exit(2);
  }
  if (output !== null) {
    const root = resolve(".artifacts/consistency/GOV-005");
    const candidate = resolve(output);
    const relation = relative(root, candidate);
    if (relation.startsWith("..") || relation.length === 0 || !candidate.endsWith(".json")) {
      console.error("design conflict output must stay under .artifacts/consistency/GOV-005");
      process.exit(64);
    }
    mkdirSync(dirname(candidate), { recursive: true });
    writeFileSync(candidate, `${JSON.stringify(receipt, null, 2)}\n`);
  }
  console.log(JSON.stringify(receipt, null, 2));
}
