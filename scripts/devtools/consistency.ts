#!/usr/bin/env bun
/**
 * GOV-006 — consistency completion auditor.
 *
 * Audits the 75-task consistency program (28-task cons-proof-gov primary
 * scope) against durable receipts under `.artifacts/consistency/**`, the
 * tracked task catalog in `scripts/devtools/consistency-catalog.md`, and the
 * tracked progress ledger `.notes/CONSISTENCY-PROGRESS.md`. An ignored local
 * investigation document may still be selected explicitly with `--fixes`, but
 * clean checkout and CI behavior must never depend on that optional file.
 *
 * Truth rules (from .notes/oracle/cons-finish-six-lane/lanes/06-gov-integrate-audit/plan.md §2.5–2.9):
 * - Freshness is decided ONLY by identity (registry version/fingerprint,
 *   producer fingerprints, binary SHA + build-time source commit, fixture
 *   SHAs, protected-path SHAs, generated-output SHAs, progress-section
 *   SHAs). Timestamps are diagnostics, never freshness authority.
 * - Only EVALUABLE_PASS exits 0. EVALUABLE_FAIL exits 2, any BLOCKED_*
 *   exits 3, any INVALID_* or ANALYSIS_PENDING exits 4, CLI usage errors
 *   exit 64 before evaluation.
 * - An invalid, blocked, or interference receipt is NEVER counted as pass.
 * - verify-all must remain nonzero, with exact missing IDs, until all 75
 *   tasks carry fresh passing evidence — a passing 28-task scope is not a
 *   program pass.
 *
 * Receipt discovery convention: current evidence for task `X` lives in the
 * JSON files under `<receiptsRoot>/X/`. Directories named `attempts`, `invalid`,
 * `history`, `superseded`, or `baseline` hold preserved historical/invalid
 * attempts and reference captures; they are counted as archived, never as
 * current evidence. Aggregate outputs (`task.json`) are never re-read as
 * producer evidence.
 */
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { ArtifactVerificationError, verifyImmutableArtifact, type ArtifactReference } from "../agentic/build-artifact.ts";
import {
  RECEIPT_REGISTRY_VERSION,
  RECEIPT_SCHEMA_VERSION,
  producerIdentityForTool,
  receiptDispositions,
  receiptRegistryIdentity,
  RUNTIME_TASK_PROOF_SPECS,
  validateReceipt,
  type ReceiptDisposition,
} from "./lib/receipt-schema.ts";
import { sanitizeReceipt, type JsonObject } from "./lib/privacy.ts";
import { readReceiptDocument, resolveReceiptDetails } from "./lib/receipt-artifact.ts";
import { classifyReceiptEvidence } from "./lib/evidence-class.ts";
import {
  TASK_PROOF_POLICIES,
  taskProofPolicy,
  type TaskProofPolicy,
} from "./lib/task-proof-policy.ts";
import {
  WORKFLOW_TASK_PRIMITIVE_ID,
  WORKFLOW_TASK_PROOF_SPECS,
  workflowTaskProofErrors,
  workflowTaskProofSourceOwners,
  type WorkflowTaskProofId,
} from "./lib/workflow-task-contract.ts";
import { validateCompleteFacadeMigrationScope } from "./facade-migrations.ts";
import {
  GENERATED_BYTE_COMPARE_OUTPUT_PATHS,
  validateGeneratedByteCompareReceipt,
} from "./generated-byte-compare.ts";

// ── Canonical ID sets ───────────────────────────────────────────────────────

export function ids(prefix: string, start: number, end: number): string[] {
  return Array.from(
    { length: end - start + 1 },
    (_, index) => `${prefix}-${String(start + index).padStart(3, "0")}`,
  );
}

export const PROGRAM_IDS: ReadonlySet<string> = new Set([
  "RPT-001",
  ...ids("SAFE", 1, 4),
  ...ids("PF", 1, 12),
  ...ids("UX", 1, 18),
  ...ids("WF", 1, 24),
  ...ids("GEO", 1, 9),
  ...ids("GOV", 1, 7),
]);

export const CONS_PROOF_GOV_IDS: ReadonlySet<string> = new Set([
  "RPT-001",
  ...ids("PF", 1, 12),
  ...ids("GEO", 1, 9),
  ...ids("GOV", 2, 7),
]);

export const CONS_FLOW_UX_IDS: ReadonlySet<string> = new Set([
  ...ids("SAFE", 1, 4),
  ...ids("WF", 1, 24),
]);

if (PROGRAM_IDS.size !== 75) {
  throw new Error(`canonical program ID set must contain 75 IDs, found ${PROGRAM_IDS.size}`);
}
if (CONS_PROOF_GOV_IDS.size !== 28) {
  throw new Error(`canonical cons-proof-gov ID set must contain 28 IDs, found ${CONS_PROOF_GOV_IDS.size}`);
}
if (
  CONS_FLOW_UX_IDS.size !== 28 ||
  [...CONS_FLOW_UX_IDS].some((taskId) =>
    !PROGRAM_IDS.has(taskId) || CONS_PROOF_GOV_IDS.has(taskId)
  )
) {
  throw new Error("canonical cons-flow-ux scope must contain 28 distinct SAFE/WF program tasks");
}
if (
  TASK_PROOF_POLICIES.size !== PROGRAM_IDS.size ||
  [...PROGRAM_IDS].some((taskId) => !TASK_PROOF_POLICIES.has(taskId)) ||
  [...TASK_PROOF_POLICIES.keys()].some((taskId) => !PROGRAM_IDS.has(taskId))
) {
  throw new Error("every canonical consistency task must have exactly one proof policy");
}

export const KNOWN_SCOPES: Record<string, ReadonlySet<string>> = {
  "cons-proof-gov": CONS_PROOF_GOV_IDS,
  "cons-flow-ux": CONS_FLOW_UX_IDS,
  program: PROGRAM_IDS,
};

export const DEFAULT_CONSISTENCY_CATALOG_PATH =
  "scripts/devtools/consistency-catalog.md";

export const FAMILY_IDS = [
  "main-menu",
  "filterable-launcher-list",
  "script-prompt",
  "utility-workspace",
  "attachment-portal",
  "assistant-workspace",
  "feedback-surface",
  "attached-popup-dialog",
  "native-secondary-window",
] as const;

/** Current generated GOV-005 lifecycle inventory; never force obsolete conflicts back into the exporter. */
export const AUTHORIZED_CONFLICT_COUNT = 29;

const ARCHIVED_DIRECTORY_NAMES = new Set([
  "attempts",
  "invalid",
  "history",
  "superseded",
  "baseline",
]);

// ── CLI contract ────────────────────────────────────────────────────────────

export type ConsistencyCommand =
  | { kind: "catalog"; fixesPath: string }
  | {
      kind: "verify-task";
      taskId: string;
      fixesPath: string;
      receiptsRoot: string;
      outPath: string;
    }
  | { kind: "verify-family"; familyId: string; receiptsRoot: string; outPath?: string }
  | { kind: "verify-scope"; scope: string; fixesPath: string; receiptsRoot: string; outPath: string }
  | { kind: "verify-all"; fixesPath: string; receiptsRoot: string; outPath: string };

export class UsageError extends Error {}

function flagValue(argv: string[], flag: string): string | undefined {
  const index = argv.indexOf(flag);
  if (index === -1) return undefined;
  const value = argv[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new UsageError(`flag ${flag} requires a value`);
  }
  return value;
}

export function parseArgs(argv: string[]): ConsistencyCommand {
  const [command, ...rest] = argv;
  const fixes = () =>
    flagValue(rest, "--fixes") ?? DEFAULT_CONSISTENCY_CATALOG_PATH;
  const receipts = () => flagValue(rest, "--receipts") ?? ".artifacts/consistency";
  switch (command) {
    case "catalog":
      return { kind: "catalog", fixesPath: fixes() };
    case "verify-task": {
      const optionValues = new Set(
        ["--receipts", "--out", "--fixes"]
          .map((flag) => flagValue(rest, flag))
          .filter((value): value is string => typeof value === "string"),
      );
      const taskId = rest.find(
        (value) => !value.startsWith("--") && !optionValues.has(value),
      );
      if (!taskId) throw new UsageError("verify-task requires a task ID argument");
      const outPath = flagValue(rest, "--out") ?? join(receipts(), taskId, "task.json");
      return {
        kind: "verify-task",
        taskId,
        fixesPath: fixes(),
        receiptsRoot: receipts(),
        outPath,
      };
    }
    case "verify-family": {
      const familyId = flagValue(rest, "--family") ?? rest.find((value) => !value.startsWith("--"));
      if (!familyId) throw new UsageError("verify-family requires --family <id>");
      return { kind: "verify-family", familyId, receiptsRoot: receipts(), outPath: flagValue(rest, "--out") };
    }
    case "verify-scope": {
      const scope = flagValue(rest, "--scope") ?? rest.find((value) => !value.startsWith("--"));
      if (!scope) throw new UsageError("verify-scope requires --scope <scope>");
      const outPath = flagValue(rest, "--out");
      if (!outPath) throw new UsageError("verify-scope requires --out <path>");
      return { kind: "verify-scope", scope, fixesPath: fixes(), receiptsRoot: receipts(), outPath };
    }
    case "verify-all": {
      const outPath = flagValue(rest, "--out");
      if (!outPath) throw new UsageError("verify-all requires --out <path>");
      return { kind: "verify-all", fixesPath: fixes(), receiptsRoot: receipts(), outPath };
    }
    default:
      throw new UsageError(
        `unknown command: ${command ?? "(none)"}; expected catalog | verify-task | verify-family | verify-scope | verify-all`,
      );
  }
}

// ── Markdown parsing (fence-aware line scanner) ─────────────────────────────

const TASK_HEADING = /^###\s+([A-Z][A-Z0-9]*-\d{3})\s+—\s+(.+?)\s*$/;
const NEARLY_TASK_HEADING = /^###\s+([A-Z][A-Z0-9]*-\d{1,3})\b/;
const SECTION_BOUNDARY = /^#{2,3}\s+/;

export interface TaskDefinition {
  id: string;
  title: string;
  line: number;
  sectionStart: number;
  sectionEnd: number;
  sectionSha256: string;
}

export interface CatalogError {
  code: string;
  detail: string;
}

export interface ParsedSections {
  tasks: TaskDefinition[];
  byId: Map<string, TaskDefinition>;
  duplicateIds: string[];
  errors: CatalogError[];
}

function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

function scanSections(markdown: string, options: { expectedIds?: ReadonlySet<string> }): ParsedSections {
  const lines = markdown.replace(/^﻿/, "").replace(/\r\n/g, "\n").split("\n");
  let fence: "```" | "~~~" | null = null;
  const tasks: TaskDefinition[] = [];
  const errors: CatalogError[] = [];
  const fencedHeadingIds = new Map<string, number>();

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (/^```/.test(line)) {
      fence = fence === "```" ? null : fence === null ? "```" : fence;
      continue;
    }
    if (/^~~~/.test(line)) {
      fence = fence === "~~~" ? null : fence === null ? "~~~" : fence;
      continue;
    }
    if (fence) {
      const fenced = TASK_HEADING.exec(line);
      if (fenced) fencedHeadingIds.set(fenced[1], index + 1);
      continue;
    }
    const match = TASK_HEADING.exec(line);
    if (!match) {
      const nearly = NEARLY_TASK_HEADING.exec(line);
      if (nearly && !TASK_HEADING.test(line)) {
        errors.push({
          code: "malformed-task-heading",
          detail: `line ${index + 1}: heading resembles a task heading but does not match the exact grammar: ${line.trim()}`,
        });
      }
      continue;
    }
    tasks.push({
      id: match[1],
      title: match[2],
      line: index + 1,
      sectionStart: index,
      sectionEnd: lines.length,
      sectionSha256: "",
    });
  }

  for (let index = 0; index < tasks.length; index += 1) {
    const task = tasks[index];
    let end = lines.length;
    for (let cursor = task.sectionStart + 1; cursor < lines.length; cursor += 1) {
      if (SECTION_BOUNDARY.test(lines[cursor])) {
        end = cursor;
        break;
      }
    }
    task.sectionEnd = end;
    task.sectionSha256 = sha256(lines.slice(task.sectionStart, end).join("\n"));
  }

  const byId = new Map<string, TaskDefinition>();
  const duplicateIds: string[] = [];
  for (const task of tasks) {
    const existing = byId.get(task.id);
    if (existing) {
      duplicateIds.push(task.id);
      errors.push({
        code: "duplicate-task-id",
        detail: `${task.id} defined at lines ${existing.line} and ${task.line}`,
      });
    } else {
      byId.set(task.id, task);
    }
  }

  if (options.expectedIds) {
    for (const [id, line] of fencedHeadingIds) {
      if (!byId.has(id) && options.expectedIds.has(id)) {
        errors.push({
          code: "task-heading-inside-code-fence",
          detail: `${id} only appears inside a code fence (line ${line})`,
        });
      }
    }
    for (const task of tasks) {
      if (!options.expectedIds.has(task.id)) {
        errors.push({ code: "unknown-task-id", detail: `${task.id} at line ${task.line}` });
      }
    }
    for (const id of options.expectedIds) {
      if (!byId.has(id)) {
        errors.push({ code: "missing-task-id", detail: id });
      }
    }
  }

  return { tasks, byId, duplicateIds, errors };
}

export interface TaskCatalog extends ParsedSections {
  path: string;
  catalogSha256: string;
}

export function parseTaskCatalog(
  markdown: string,
  path = DEFAULT_CONSISTENCY_CATALOG_PATH,
): TaskCatalog {
  const parsed = scanSections(markdown, { expectedIds: PROGRAM_IDS });
  return { ...parsed, path, catalogSha256: sha256(markdown) };
}

export interface ProgressCatalog extends ParsedSections {
  path: string;
  progressSha256: string;
}

export function parseProgressSections(markdown: string, path = ".notes/CONSISTENCY-PROGRESS.md"): ProgressCatalog {
  // Progress sections are only required for tasks that claim completion, so
  // no expected-set errors here; duplicates are still structural errors.
  const parsed = scanSections(markdown, {});
  const errors = parsed.errors.map((error) =>
    error.code === "duplicate-task-id"
      ? { code: "duplicate-progress-section", detail: error.detail }
      : error,
  );
  return { ...parsed, errors, path, progressSha256: sha256(markdown) };
}

// ── Current identity ────────────────────────────────────────────────────────

export interface CurrentIdentity {
  headCommit: string | null;
  registry: { schemaVersion: number; registryVersion: number; registryFingerprint: string };
  protectedPaths: Array<{ path: string; sha256: string }>;
  fileSha256: (path: string) => string | null;
  producerFingerprint: (tool: string) => string;
}

const fileHashCache = new Map<string, { identity: string; sha256: string }>();

export function fileSha256(path: string): string | null {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const before = statSync(path, { bigint: true });
      const beforeIdentity =
        `${before.dev}:${before.ino}:${before.size}:${before.mtimeNs}:${before.ctimeNs}`;
      const cached = fileHashCache.get(path);
      if (cached?.identity === beforeIdentity) return cached.sha256;
      const bytes = readFileSync(path);
      const after = statSync(path, { bigint: true });
      const afterIdentity =
        `${after.dev}:${after.ino}:${after.size}:${after.mtimeNs}:${after.ctimeNs}`;
      if (beforeIdentity !== afterIdentity) continue;
      const value = sha256(bytes);
      fileHashCache.set(path, { identity: afterIdentity, sha256: value });
      return value;
    } catch {
      fileHashCache.delete(path);
      return null;
    }
  }
  fileHashCache.delete(path);
  return null;
}

function gitHead(): string | null {
  const result = Bun.spawnSync(["git", "rev-parse", "HEAD"], { stdout: "pipe", stderr: "pipe" });
  return result.exitCode === 0 ? new TextDecoder().decode(result.stdout).trim() : null;
}

export function currentIdentity(overrides: Partial<CurrentIdentity> = {}): CurrentIdentity {
  return {
    headCommit: overrides.headCommit !== undefined ? overrides.headCommit : gitHead(),
    registry: overrides.registry ?? receiptRegistryIdentity(),
    protectedPaths: overrides.protectedPaths ?? [],
    fileSha256: overrides.fileSha256 ?? fileSha256,
    producerFingerprint:
      overrides.producerFingerprint ?? ((tool: string) => producerIdentityForTool(tool).fingerprint),
  };
}

// ── Receipt discovery ───────────────────────────────────────────────────────

export interface DiscoveredReceipt {
  path: string;
  receipt: JsonObject;
  disposition: ReceiptDisposition;
  archived: boolean;
}

const FACADE_LEDGER_ASSERTIONS = [
  "allFacadesValueFree",
  "allProductionCallersMigrated",
  "allTestCallersMigrated",
  "zeroCallerFacadesRemoved",
  "persistedNamesLiveAtCanonicalOwnersOnly",
] as const;

function isAuthenticFacadeLedger(
  taskDir: string,
  path: string,
  receipt: JsonObject | null,
): boolean {
  if (
    basename(taskDir) !== "GOV-002" ||
    resolve(path) !== resolve(join(taskDir, "facade-ledger.json")) ||
    receipt === null ||
    receipt.schemaVersion !== 1 ||
    receipt.generatedBy !== "scripts/devtools/facade-ledger.ts" ||
    receipt.taskId !== "GOV-002" ||
    receipt.evidenceClass !== "STATIC_INVENTORY" ||
    receipt.provesRuntimeBehavior !== false ||
    receipt.provesExporterByteEquality !== false ||
    !["EVALUABLE_PASS", "EVALUABLE_FAIL"].includes(String(receipt.disposition)) ||
    ["primitiveId", "tool", "command", "catalogBinding", "transaction"]
      .some((field) => Object.prototype.hasOwnProperty.call(receipt, field))
  ) {
    return false;
  }
  const assertions = asObject(receipt.assertions);
  if (
    !FACADE_LEDGER_ASSERTIONS.every((field) =>
      typeof assertions[field] === "boolean"
    )
  ) {
    return false;
  }
  const scope = asObject(receipt.facadeMigrations);
  if (validateCompleteFacadeMigrationScope(scope).length > 0) {
    return false;
  }
  return (
    Array.isArray(receipt.facades) &&
    JSON.stringify(receipt.facades) === JSON.stringify(scope.facades)
  );
}

function walkJsonFiles(root: string, archived: boolean, out: Array<{ path: string; archived: boolean }>) {
  let entries: string[];
  try {
    entries = readdirSync(root);
  } catch {
    return;
  }
  for (const entry of entries) {
    const full = join(root, entry);
    let stats;
    try {
      stats = statSync(full);
    } catch {
      continue;
    }
    if (stats.isDirectory()) {
      walkJsonFiles(full, archived || ARCHIVED_DIRECTORY_NAMES.has(entry), out);
    } else if (entry.endsWith(".json")) {
      out.push({ path: full, archived });
    }
  }
}

export function discoverReceipts(taskDir: string): {
  receipts: DiscoveredReceipt[];
  archivedCount: number;
  evidenceArtifactPaths: string[];
  unreadablePaths: string[];
} {
  const files: Array<{ path: string; archived: boolean }> = [];
  walkJsonFiles(taskDir, false, files);
  const receipts: DiscoveredReceipt[] = [];
  const evidenceArtifactPaths: string[] = [];
  const unreadablePaths: string[] = [];
  let archivedCount = 0;
  for (const file of files) {
    if (basename(file.path) === "task.json") continue; // our own aggregate output
    let parsed: unknown;
    try {
      parsed = resolveReceiptDetails(readReceiptDocument(file.path), file.path);
    } catch {
      unreadablePaths.push(file.path);
      continue;
    }
    const receipt = parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as JsonObject)
      : null;
    if (
      basename(taskDir) === "GOV-002" &&
      resolve(file.path) === resolve(join(taskDir, "facade-ledger.json"))
    ) {
      if (isAuthenticFacadeLedger(taskDir, file.path, receipt)) {
        evidenceArtifactPaths.push(file.path);
      } else {
        unreadablePaths.push(file.path);
      }
      continue;
    }
    if (
      basename(taskDir) === "GOV-005" &&
      resolve(file.path) ===
        resolve(join(taskDir, "generated-byte-compare.json"))
    ) {
      if (validateGeneratedByteCompareReceipt(receipt).pass) {
        evidenceArtifactPaths.push(file.path);
      } else {
        unreadablePaths.push(file.path);
      }
      continue;
    }
    const disposition = receipt && typeof receipt.disposition === "string" &&
        (receiptDispositions as readonly string[]).includes(receipt.disposition)
      ? (receipt.disposition as ReceiptDisposition)
      : null;
    if (!receipt || !disposition) {
      evidenceArtifactPaths.push(file.path);
      continue;
    }
    if (file.archived) {
      archivedCount += 1;
      continue;
    }
    receipts.push({ path: file.path, receipt, disposition, archived: false });
  }
  return { receipts, archivedCount, evidenceArtifactPaths, unreadablePaths };
}

// ── Staleness by identity ───────────────────────────────────────────────────

export interface StaleReason {
  code: string;
  detail?: string;
}

function asObject(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as JsonObject) : {};
}

function artifactBinaryStaleReason(binary: JsonObject): StaleReason | null {
  if (binary.artifactReference === undefined) {
    return { code: "stale-binary-provenance-missing" };
  }
  try {
    const artifact = verifyImmutableArtifact(process.cwd(), binary.artifactReference as ArtifactReference, {
      kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content",
    });
    for (const [key, value] of Object.entries(artifact.binary)) {
      if (binary[key] !== value) {
        return { code: "stale-binary-provenance-identity", detail: `observed_${key}_mismatch` };
      }
    }
    return null;
  } catch (error) {
    // References and verifier exceptions may contain private input; publish only typed codes.
    return {
      code: "stale-binary-provenance",
      detail: error instanceof ArtifactVerificationError ? error.code : "manifest_invalid",
    };
  }
}

export function receiptStaleReasons(entry: DiscoveredReceipt, current: CurrentIdentity): StaleReason[] {
  const reasons: StaleReason[] = [];
  let receipt: JsonObject;
  try { receipt = resolveReceiptDetails(entry.receipt); }
  catch (error) { return [{ code: "invalid-receipt-reference", detail: error instanceof Error ? error.message : String(error) }]; }
  const producerValidation = asObject(receipt.producerValidation);
  if (
    producerValidation.registryVersion !== undefined &&
    producerValidation.registryVersion !== current.registry.registryVersion
  ) {
    reasons.push({ code: "stale-registry-version", detail: entry.path });
  }
  if (
    typeof producerValidation.registryFingerprint === "string" &&
    producerValidation.registryFingerprint !== current.registry.registryFingerprint
  ) {
    reasons.push({ code: "stale-registry-fingerprint", detail: entry.path });
  }
  const repository = asObject(receipt.repository);
  if (typeof receipt.tool === "string" && typeof repository.producerSourceFingerprint === "string") {
    if (current.producerFingerprint(receipt.tool) !== repository.producerSourceFingerprint) {
      reasons.push({ code: "stale-producer", detail: `${receipt.tool} (${entry.path})` });
    }
  }
  if (receipt.runtimeTaskProof && typeof receipt.runtimeTaskProof === "object") {
    const sourceFingerprints = asObject(receipt.sourceFingerprints);
    for (const [path, expected] of Object.entries(sourceFingerprints)) {
      if (
        !(path.startsWith("scripts/devtools/") ||
          path.startsWith("scripts/agentic/cons-proof-gov/") ||
          path === "scripts/agentic/compiler-input-paths.txt") ||
        path.split("/").includes("..") ||
        typeof expected !== "string" || !/^[a-f0-9]{64}$/.test(expected)
      ) {
        reasons.push({ code: "stale-runtime-proof-source-owner", detail: `${path} (${entry.path})` });
        continue;
      }
      const actual = current.fileSha256(path);
      if (actual === null || actual !== expected) {
        reasons.push({
          code: actual === null ? "stale-runtime-proof-source-missing" : "stale-runtime-proof-source",
          detail: `${path} (${entry.path})`,
        });
      }
    }
  }
  if (receipt.workflowTaskProof && typeof receipt.workflowTaskProof === "object") {
    const taskId = typeof receipt.taskId === "string" ? receipt.taskId : "";
    const expectedOwners = taskId in WORKFLOW_TASK_PROOF_SPECS
      ? workflowTaskProofSourceOwners(taskId as WorkflowTaskProofId)
      : [];
    for (const [path, expected] of Object.entries(asObject(receipt.sourceFingerprints))) {
      if (
        !expectedOwners.includes(path) ||
        path.split("/").includes("..") ||
        typeof expected !== "string" || !/^[a-f0-9]{64}$/.test(expected)
      ) {
        reasons.push({ code: "stale-workflow-proof-source-owner", detail: `${path} (${entry.path})` });
        continue;
      }
      const actual = current.fileSha256(path);
      if (actual === null || actual !== expected) {
        reasons.push({
          code: actual === null ? "stale-workflow-proof-source-missing" : "stale-workflow-proof-source",
          detail: `${path} (${entry.path})`,
        });
      }
    }
  }
  if (receipt.primitiveId === "devtools.consistency.safe-task-proof") {
    const reviewedWorkflowSuite =
      "scripts/agentic/cons-flow-ux/final-workflow-audit.test.ts";
    const reviewedWorkflowOwner =
      "scripts/agentic/cons-flow-ux/final-workflow-audit.ts";
    for (const [path, expected] of Object.entries(asObject(receipt.sourceFingerprints))) {
      if (
        !(
          path.startsWith("src/") ||
          path.startsWith("scripts/devtools/") ||
          path.startsWith("crates/sk-protocol/src/") ||
          path.startsWith("design/mockups/generated/") ||
          path === reviewedWorkflowSuite ||
          (receipt.taskId === "GOV-006" &&
            (path === reviewedWorkflowOwner || path === "scripts/agentic/compiler-input-paths.txt"))
        ) ||
        path.split("/").includes("..") ||
        typeof expected !== "string" ||
        !/^[a-f0-9]{64}$/.test(expected)
      ) {
        reasons.push({ code: "stale-proof-source-owner", detail: `${path} (${entry.path})` });
        continue;
      }
      const actual = current.fileSha256(path);
      if (actual === null) {
        reasons.push({ code: "stale-proof-source-missing", detail: `${path} (${entry.path})` });
      } else if (actual !== expected) {
        reasons.push({ code: "stale-proof-source", detail: `${path} (${entry.path})` });
      }
    }
  }
  const binary = asObject(receipt.binary);
  const artifactBacked = binary.artifactReference !== undefined || !!(receipt.runtimeTaskProof || receipt.workflowTaskProof);
  if (!artifactBacked && typeof binary.path === "string" && typeof binary.sha256 === "string") {
    const currentSha = current.fileSha256(binary.path);
    if (currentSha === null) {
      reasons.push({ code: "stale-binary-missing", detail: binary.path });
    } else if (currentSha !== binary.sha256) {
      reasons.push({ code: "stale-binary", detail: binary.path });
    }
  }
  if (!artifactBacked && typeof binary.sourceCommit === "string" && current.headCommit && binary.sourceCommit !== current.headCommit) {
    reasons.push({ code: "stale-binary-source-commit", detail: `${binary.sourceCommit} != HEAD (${entry.path})` });
  }
  if (artifactBacked) {
    const stale = artifactBinaryStaleReason(binary);
    if (stale) reasons.push(stale);
  }
  const fixture = asObject(receipt.fixture);
  if (typeof fixture.path === "string" && typeof fixture.sha256 === "string") {
    const currentSha = current.fileSha256(fixture.path);
    if (currentSha === null) {
      reasons.push({ code: "stale-fixture-missing", detail: fixture.path });
    } else if (currentSha !== fixture.sha256) {
      reasons.push({ code: "stale-fixture", detail: fixture.path });
    }
  }
  return reasons;
}

/**
 * Staleness for a previously written task aggregate re-read later
 * (verify-scope / verify-all re-derive aggregates and then re-check them
 * against the identity captured at aggregate time).
 */
export function stalenessReasons(task: JsonObject, current: CurrentIdentity): StaleReason[] {
  const reasons: StaleReason[] = [];
  const identities = asObject(task.identities);
  if (identities.receiptRegistryVersion !== current.registry.registryVersion) {
    reasons.push({ code: "stale-registry-version" });
  }
  if (identities.receiptRegistryFingerprint !== current.registry.registryFingerprint) {
    reasons.push({ code: "stale-registry-fingerprint" });
  }
  for (const [tool, expected] of Object.entries(asObject(identities.producerSourceFingerprints))) {
    if (typeof expected === "string" && current.producerFingerprint(tool) !== expected) {
      reasons.push({ code: "stale-producer", detail: tool });
    }
  }
  const implementationFiles = asObject(identities.implementationFiles);
  for (const [path, expected] of Object.entries(implementationFiles)) {
    if (typeof expected === "string" && current.fileSha256(path) !== expected) {
      reasons.push({ code: "stale-implementation", detail: path });
    }
  }
  if (typeof task.implementationFingerprint === "string") {
    const recomputed = sha256(
      Object.entries(implementationFiles)
        .map(([path, hash]) => `${path}:${hash}`)
        .sort()
        .join("\n"),
    );
    if (recomputed !== task.implementationFingerprint) {
      reasons.push({ code: "stale-implementation", detail: "implementationFingerprint disagrees with implementationFiles" });
    }
  }
  for (const [path, expected] of Object.entries(asObject(identities.fixtureHashes))) {
    if (typeof expected === "string" && current.fileSha256(path) !== expected) {
      reasons.push({ code: "stale-fixture", detail: path });
    }
  }
  for (const [path, expected] of Object.entries(asObject(identities.protectedHashes))) {
    if (typeof expected === "string" && current.fileSha256(path) !== expected) {
      reasons.push({ code: "protected-hash-drift", detail: path });
    }
  }
  for (const binary of Array.isArray(identities.binaries) ? identities.binaries : []) {
    const record = asObject(binary);
    if (record.artifactReference !== undefined ||
      typeof task.taskId === "string" && (task.taskId in RUNTIME_TASK_PROOF_SPECS || task.taskId in WORKFLOW_TASK_PROOF_SPECS)) {
      const stale = artifactBinaryStaleReason(record);
      if (stale) reasons.push(stale);
      continue;
    }
    if (typeof record.path === "string" && typeof record.sha256 === "string") {
      if (current.fileSha256(record.path) !== record.sha256) {
        reasons.push({ code: "stale-binary", detail: String(record.path) });
      }
    }
    if (typeof record.sourceCommit === "string" && current.headCommit && record.sourceCommit !== current.headCommit) {
      reasons.push({ code: "stale-binary-source-commit", detail: String(record.path ?? "") });
    }
  }
  for (const [path, expected] of Object.entries(asObject(identities.generatedOutputHashes))) {
    if (typeof expected === "string" && current.fileSha256(path) !== expected) {
      reasons.push({ code: "stale-generated-output", detail: path });
    }
  }
  return reasons;
}

// ── verify-task ─────────────────────────────────────────────────────────────

export type LayerStatus =
  | {
      applicability: "required";
      pass: boolean;
      receiptPaths: string[];
      fingerprint: string;
      summary: Record<string, unknown>;
    }
  | {
      applicability: "not-applicable";
      reason: string;
      receiptPaths: [];
      pass: true;
    };

const EVIDENCE_LAYERS = [
  ["intended", "intendedContract"],
  ["model", "modelEvidence"],
  ["rendered", "renderedEvidence"],
  ["accessibility", "axEvidence"],
  ["interaction", "interactionOutcome"],
] as const;

export interface VerifyTaskInput {
  taskId: string;
  scope: string;
  receiptsRoot: string;
  catalog: TaskCatalog;
  progress: ProgressCatalog;
  current: CurrentIdentity;
}

const DISPOSITION_RANK: Array<(d: ReceiptDisposition) => boolean> = [
  (d) => d.startsWith("INVALID_") || d === "ANALYSIS_PENDING",
  (d) => d.startsWith("BLOCKED_"),
  (d) => d === "EVALUABLE_FAIL",
];

function rollupDisposition(candidates: ReceiptDisposition[], fallback: ReceiptDisposition): ReceiptDisposition {
  for (const matches of DISPOSITION_RANK) {
    const found = candidates.find(matches);
    if (found) return found;
  }
  return candidates.length > 0 ? candidates[0] : fallback;
}

function exitCodeForDisposition(disposition: ReceiptDisposition): number {
  if (disposition === "EVALUABLE_PASS") return 0;
  if (disposition === "EVALUABLE_FAIL") return 2;
  if (disposition.startsWith("BLOCKED_")) return 3;
  return 4;
}

const CLASSIFICATION_FOR_DISPOSITION: Record<string, string> = {
  EVALUABLE_PASS: "ok",
  EVALUABLE_FAIL: "reproduced",
  BLOCKED_MISSING_PRIMITIVE: "blocked-missing-primitive",
  BLOCKED_TARGET_AMBIGUITY: "blocked-target-ambiguity",
  BLOCKED_STALE_GENERATION: "blocked-stale-generation",
  BLOCKED_PERMISSION: "blocked-permission",
  BLOCKED_REAL_DATA_RISK: "blocked-real-data-risk",
  BLOCKED_TIMEOUT: "blocked-timeout",
  BLOCKED_SCOPE_DRIFT: "blocked-scope-drift",
  BLOCKED_UNSUPPORTED_PROJECTION: "blocked-unsupported-projection",
  INVALID_SCHEMA: "invalid-schema",
  INVALID_IDENTITY: "invalid-identity",
  INVALID_GENERATION: "invalid-generation",
  INVALID_PRIVACY: "invalid-privacy",
  INVALID_BINARY: "invalid-binary",
  INVALID_FIXTURE: "invalid-fixture",
  INVALID_OBSERVER: "invalid-observer",
  INVALID_INTERFERENCE: "invalid-interference",
  INVALID_CLEANUP: "invalid-cleanup",
  ANALYSIS_PENDING: "analysis-pending",
};

interface AggregateShell extends JsonObject {}

function finalizeAggregate(
  primitiveId: string,
  command: string,
  disposition: ReceiptDisposition,
  body: JsonObject,
): { receipt: JsonObject; exitCode: number } {
  const receipt: AggregateShell = {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    primitiveId,
    tool: "script-kit-devtools.consistency",
    command,
    classification: CLASSIFICATION_FOR_DISPOSITION[disposition] ?? "invalid-schema",
    ...body,
    disposition,
    pass: disposition === "EVALUABLE_PASS",
  };
  const validation = validateReceipt(primitiveId, receipt);
  receipt.producerValidation = {
    registryVersion: RECEIPT_REGISTRY_VERSION,
    registryFingerprint: receiptRegistryIdentity().registryFingerprint,
    schemaId: `${primitiveId}@1`,
    valid: validation.valid,
    errors: validation.errors,
  };
  if (!validation.valid && disposition === "EVALUABLE_PASS") {
    // A pass aggregate that fails its own registry validation is not a pass.
    receipt.disposition = validation.disposition;
    receipt.pass = false;
    receipt.classification = CLASSIFICATION_FOR_DISPOSITION[validation.disposition] ?? "invalid-schema";
    return { receipt, exitCode: exitCodeForDisposition(validation.disposition) };
  }
  return { receipt, exitCode: exitCodeForDisposition(receipt.disposition as ReceiptDisposition) };
}

export function verifyTask(input: VerifyTaskInput): { receipt: JsonObject; exitCode: number } {
  const { taskId, scope, receiptsRoot, catalog, progress, current } = input;
  const errors: Array<{ code: string; detail: unknown }> = [];
  const staleReasons: StaleReason[] = [];
  const proofPolicy: TaskProofPolicy | null = taskProofPolicy(taskId);

  if (!PROGRAM_IDS.has(taskId)) {
    errors.push({ code: "unknown-task-id", detail: taskId });
  }
  const catalogEntry = catalog.byId.get(taskId);
  if (!catalogEntry) {
    errors.push({ code: "task-missing-from-catalog", detail: taskId });
  }
  for (const error of catalog.errors) {
    if (error.code === "duplicate-task-id" && error.detail.startsWith(taskId)) {
      errors.push({ code: "duplicate-task-id", detail: error.detail });
    }
  }

  const progressEntries = progress.tasks.filter((task) => task.id === taskId);
  if (progressEntries.length === 0) {
    errors.push({ code: "missing-progress-section", detail: taskId });
  } else if (progressEntries.length > 1) {
    errors.push({
      code: "duplicate-progress-section",
      detail: `${taskId} at lines ${progressEntries.map((entry) => entry.line).join(", ")}`,
    });
  }
  const progressEntry = progressEntries[0] ?? null;

  const taskDir = join(receiptsRoot, taskId);
  const discovery = discoverReceipts(taskDir);
  for (const path of discovery.unreadablePaths) {
    errors.push({ code: "unreadable-receipt", detail: path });
  }

  const positiveReceiptPaths: string[] = [];
  const negativeControlPaths: string[] = [];
  const receiptDispositionList: ReceiptDisposition[] = [];
  let negativeTotal = 0;
  let negativeFailed = 0;
  let privacyPerformed = false;
  let privacyPass = true;
  let rawContentReturned = false;
  let canaryMatches = 0;
  let interferenceMonitored = false;
  let interferencePass = true;
  let interferenceDisposition: string | null = null;
  let cleanupClosed = true;
  const survivors: unknown[] = [];
  const producerSourceFingerprints: Record<string, string> = {};
  const implementationFiles: Record<string, string> = {};
  const fixtureHashes: Record<string, string> = {};
  const binaries: Array<Record<string, unknown>> = [];
  const layerPaths: Record<string, string[]> = {
    intended: [],
    model: [],
    rendered: [],
    accessibility: [],
    interaction: [],
  };

  for (const entry of discovery.receipts) {
    receiptDispositionList.push(entry.disposition);
    const receipt = entry.receipt;

    if (entry.disposition !== "EVALUABLE_PASS" && receipt.pass === true) {
      const code = entry.disposition === "INVALID_INTERFERENCE"
        ? "interference-pass-through"
        : entry.disposition.startsWith("BLOCKED_")
          ? "blocked-receipt-marked-pass"
          : "invalid-receipt-marked-pass";
      errors.push({ code, detail: `${entry.disposition} marked pass at ${entry.path}` });
    }
    if (entry.disposition === "EVALUABLE_PASS" && receipt.pass === false) {
      errors.push({ code: "pass-disposition-marked-fail", detail: entry.path });
    }

    if (entry.disposition === "EVALUABLE_PASS" && proofPolicy) {
      const observation = classifyReceiptEvidence(receipt);
      const binding = asObject(receipt.catalogBinding);
      const declaredTaskIds = [
        ...(typeof receipt.taskId === "string" ? [receipt.taskId] : []),
        ...(
          Array.isArray(receipt.taskIds)
            ? receipt.taskIds.filter((value): value is string => typeof value === "string")
            : []
        ),
      ];
      if (
        !declaredTaskIds.includes(taskId) ||
        binding.taskId !== taskId ||
        binding.sectionSha256 !== catalogEntry?.sectionSha256 ||
        binding.title !== catalogEntry?.title
      ) {
        errors.push({
          code: "task-canonical-catalog-binding-mismatch",
          detail:
            `${entry.path}: ${String(binding.taskId ?? "missing-task-id")} / ` +
            `${String(binding.sectionSha256 ?? "missing-section-hash")}`,
        });
      }
      if (!proofPolicy.acceptedEvidenceClasses.includes(observation.evidenceClass)) {
        errors.push({
          code: "task-evidence-class-not-accepted",
          detail:
            `${taskId} requires ${proofPolicy.requirement}; ` +
            `${entry.path} provides ${observation.evidenceClass}`,
        });
      }
      if (observation.errors.length > 0) {
        errors.push({
          code: "task-evidence-observation-invalid",
          detail: `${entry.path}: ${observation.errors.join("; ")}`,
        });
      }
      if (
        proofPolicy.provesRuntimeBehavior &&
        proofPolicy.acceptedEvidenceClasses.includes(observation.evidenceClass)
      ) {
        const primitiveId =
          typeof receipt.primitiveId === "string" ? receipt.primitiveId : null;
        const workflowSpec = WORKFLOW_TASK_PROOF_SPECS[
          taskId as WorkflowTaskProofId
        ];
        if (workflowSpec && primitiveId !== WORKFLOW_TASK_PRIMITIVE_ID) {
          errors.push({
            code: "task-workflow-proof-contract-missing",
            detail:
              `${taskId} requires its exact observed journey from ${workflowSpec.producerOwner}; ` +
              `${entry.path} provides ${primitiveId ?? "no registered primitive"}`,
          });
        } else if (workflowSpec) {
          const workflowErrors = workflowTaskProofErrors(receipt);
          if (workflowErrors.length > 0) {
            errors.push({
              code: "task-workflow-proof-contract-invalid",
              detail: `${taskId}: ${workflowErrors.join("; ")}`,
            });
          }
        }
        const foundationSpec = RUNTIME_TASK_PROOF_SPECS[
          taskId as keyof typeof RUNTIME_TASK_PROOF_SPECS
        ];
        if (foundationSpec && primitiveId !== foundationSpec.primitiveId) {
          errors.push({
            code: "task-runtime-primitive-mismatch",
            detail:
              `${taskId} requires ${foundationSpec.primitiveId}; ` +
              `${entry.path} provides ${primitiveId ?? "no registered primitive"}`,
          });
        }
        if (foundationSpec && primitiveId === foundationSpec.primitiveId) {
          const observedMode = primitiveId === "devtools.elements.snapshot"
            ? asObject(receipt.semanticProjection).proofMode
            : primitiveId === "devtools.scroll.inspect"
              ? (asObject(receipt.renderedSafeViewport).required === true
                ? "rendered-safe-viewport"
                : null)
              : receipt.proofMode;
          if (observedMode !== foundationSpec.proofMode) {
            errors.push({
              code: "task-runtime-proof-mode-mismatch",
              detail:
                `${taskId} requires ${foundationSpec.proofMode}; ` +
                `${entry.path} provides ${String(observedMode ?? "no actual proof mode")}`,
            });
          }
          const runtimeProof = asObject(receipt.runtimeTaskProof);
          const sourceFingerprints = asObject(receipt.sourceFingerprints);
          const expectedOwners = [
            "scripts/devtools/lib/runtime-task-proof.ts",
            "scripts/agentic/compiler-input-paths.txt",
            "scripts/devtools/lib/receipt-schema.ts",
            foundationSpec.productionOwner,
            foundationSpec.runtimeProducer,
          ];
          const declaredOwners = Array.isArray(runtimeProof.sourceOwners)
            ? runtimeProof.sourceOwners.filter((path): path is string => typeof path === "string")
            : [];
          if (
            runtimeProof.productionOwner !== foundationSpec.productionOwner ||
            runtimeProof.runtimeProducer !== foundationSpec.runtimeProducer ||
            runtimeProof.proofMode !== foundationSpec.proofMode ||
            declaredOwners.length !== expectedOwners.length ||
            new Set(declaredOwners).size !== expectedOwners.length ||
            expectedOwners.some((path) => !declaredOwners.includes(path)) ||
            Object.keys(sourceFingerprints).length !== expectedOwners.length ||
            expectedOwners.some((path) => typeof sourceFingerprints[path] !== "string")
          ) {
            errors.push({
              code: "task-runtime-proof-source-ownership-mismatch",
              detail: `${taskId} requires the exact reviewed primitive, adapter, schema, and runtime-producer owners`,
            });
          }
          const executedControls = Array.isArray(receipt.negativeControls)
            ? receipt.negativeControls.map(asObject)
            : [];
          const controlIds = executedControls.map((control) => String(control.id ?? ""));
          const requiredControls = foundationSpec.negativeControlIds
            .filter((id) => !controlIds.includes(id));
          if (
            requiredControls.length > 0 ||
            new Set(controlIds).size !== controlIds.length
          ) {
            errors.push({
              code: "task-runtime-required-negative-control-missing",
              detail: `${taskId}: ${requiredControls.join(", ") || "duplicate negative controls"}`,
            });
          }
        }
        const validation = primitiveId ? validateReceipt(primitiveId, receipt) : null;
        if (
          !validation?.valid ||
          validation.disposition !== "EVALUABLE_PASS" ||
          asObject(receipt.producerValidation).valid !== true
        ) {
          errors.push({
            code: "task-runtime-proof-not-registry-validated",
            detail:
              `${entry.path}: ${primitiveId ?? "missing primitiveId"}; ` +
              `${validation?.errors.join("; ") ?? "no registered producer"}`,
          });
        }
      }
    }

    const missing = Array.isArray(receipt.missingPrimitives) ? receipt.missingPrimitives : [];
    if (entry.disposition === "EVALUABLE_PASS" && missing.length > 0) {
      errors.push({ code: "pass-receipt-missing-primitives", detail: `${entry.path}: ${missing.join(", ")}` });
    }

    const negatives = Array.isArray(receipt.negativeControls) ? receipt.negativeControls : [];
    for (const control of negatives) {
      negativeTotal += 1;
      const record = asObject(control);
      const controlPassed = record.pass === true || record.rejected === true || record.ok === true;
      if (!controlPassed) {
        negativeFailed += 1;
        errors.push({ code: "failed-negative-control", detail: `${entry.path}: ${String(record.id ?? "unnamed")}` });
      }
    }
    if (negatives.length > 0) negativeControlPaths.push(entry.path);

    const privacy = asObject(receipt.privacy);
    const scan = asObject(privacy.recursiveCanaryScan);
    if (scan.performed === true) privacyPerformed = true;
    if (scan.pass === false) privacyPass = false;
    if (privacy.rawContentReturned === true) {
      rawContentReturned = true;
      privacyPass = false;
    }
    canaryMatches += Number(privacy.canaryMatches ?? 0);

    const interference = asObject(receipt.interference);
    if (interference.monitored === true) interferenceMonitored = true;
    if (typeof interference.disposition === "string" && interference.disposition.length > 0) {
      interferenceDisposition = interference.disposition;
      if (interference.disposition.toUpperCase().includes("INTERFERENCE")) {
        interferencePass = false;
      }
    }

    const cleanup = asObject(receipt.cleanup);
    if (cleanup.closed === false) cleanupClosed = false;
    for (const survivor of Array.isArray(cleanup.survivors) ? cleanup.survivors : []) {
      survivors.push(survivor);
      errors.push({ code: "cleanup-survivor", detail: `${entry.path}: ${String(survivor)}` });
    }

    const repository = asObject(receipt.repository);
    if (typeof receipt.tool === "string" && typeof repository.producerSourceFingerprint === "string") {
      producerSourceFingerprints[receipt.tool] = repository.producerSourceFingerprint;
    }
    const binary = asObject(receipt.binary);
    if (typeof binary.sha256 === "string") {
      binaries.push(binary.artifactReference !== undefined || receipt.runtimeTaskProof || receipt.workflowTaskProof
        ? asObject(sanitizeReceipt(binary).sanitized)
        : { ...binary });
    }
    const fixture = asObject(receipt.fixture);
    if (typeof fixture.path === "string" && typeof fixture.sha256 === "string") {
      fixtureHashes[fixture.path] = fixture.sha256;
    }

    const evidence = asObject(receipt.evidence);
    for (const [layer] of EVIDENCE_LAYERS) {
      if (evidence[layer] !== null && evidence[layer] !== undefined) {
        layerPaths[layer].push(entry.path);
      }
    }

    staleReasons.push(...receiptStaleReasons(entry, current));

    if (entry.disposition === "EVALUABLE_PASS") {
      positiveReceiptPaths.push(entry.path);
      const hash = current.fileSha256(entry.path);
      if (hash) implementationFiles[entry.path] = hash;
    }
  }

  if (positiveReceiptPaths.length === 0) {
    errors.push({ code: "missing-task-receipt", detail: `${taskId}: no current EVALUABLE_PASS receipt under ${taskDir}` });
  }
  if (positiveReceiptPaths.length > 0 && negativeTotal === 0) {
    errors.push({ code: "missing-negative-controls", detail: taskId });
  }
  if (positiveReceiptPaths.length > 0 && !privacyPerformed) {
    errors.push({ code: "missing-privacy-scan", detail: taskId });
  }

  if (progressEntry) {
    implementationFiles[progress.path] = current.fileSha256(progress.path) ?? "unreadable";
  }
  const implementationFingerprint = sha256(
    Object.entries(implementationFiles)
      .map(([path, hash]) => `${path}:${hash}`)
      .sort()
      .join("\n"),
  );

  const layers: Record<string, LayerStatus> = {};
  for (const [layer, field] of EVIDENCE_LAYERS) {
    const paths = layerPaths[layer];
    layers[field] = paths.length > 0
      ? {
          applicability: "required",
          pass: true,
          receiptPaths: paths,
          fingerprint: sha256(paths.map((path) => `${path}:${current.fileSha256(path) ?? ""}`).sort().join("\n")),
          summary: { receiptCount: paths.length },
        }
      : {
          applicability: "not-applicable",
          reason: `no current receipt for ${taskId} declares ${layer} evidence`,
          receiptPaths: [],
          pass: true,
        };
  }

  // Disposition rollup: invalid > blocked > fail; stale evidence is blocked
  // (stale generation), never silently pass.
  const hasInvalid = errors.some((error) =>
    [
      "interference-pass-through",
      "invalid-receipt-marked-pass",
      "blocked-receipt-marked-pass",
      "pass-disposition-marked-fail",
      "pass-receipt-missing-primitives",
      "cleanup-survivor",
      "unknown-task-id",
      "task-missing-from-catalog",
      "duplicate-task-id",
      "missing-progress-section",
      "duplicate-progress-section",
      "unreadable-receipt",
      "task-evidence-class-not-accepted",
      "task-evidence-observation-invalid",
      "task-runtime-proof-not-registry-validated",
      "task-workflow-proof-contract-missing",
      "task-workflow-proof-contract-invalid",
      "task-runtime-primitive-mismatch",
      "task-runtime-proof-mode-mismatch",
      "task-runtime-proof-source-ownership-mismatch",
      "task-runtime-required-negative-control-missing",
      "task-canonical-catalog-binding-mismatch",
    ].includes(error.code),
  ) || receiptDispositionList.some((d) => d.startsWith("INVALID_") || d === "ANALYSIS_PENDING");

  let disposition: ReceiptDisposition;
  if (hasInvalid) {
    disposition = errors.some((error) => error.code === "cleanup-survivor")
      ? "INVALID_CLEANUP"
      : "INVALID_SCHEMA";
  } else if (!privacyPass || canaryMatches > 0) {
    disposition = "INVALID_PRIVACY";
  } else if (receiptDispositionList.some((d) => d.startsWith("BLOCKED_"))) {
    disposition = rollupDisposition(receiptDispositionList.filter((d) => d.startsWith("BLOCKED_")), "BLOCKED_MISSING_PRIMITIVE");
  } else if (positiveReceiptPaths.length === 0) {
    disposition = "BLOCKED_MISSING_PRIMITIVE";
  } else if (staleReasons.length > 0) {
    disposition = "BLOCKED_STALE_GENERATION";
  } else if (
    receiptDispositionList.includes("EVALUABLE_FAIL") ||
    negativeFailed > 0 ||
    errors.length > 0
  ) {
    disposition = "EVALUABLE_FAIL";
  } else {
    disposition = "EVALUABLE_PASS";
  }

  const body: JsonObject = {
    taskId,
    scope,
    proofPolicy,
    implementationCommit: current.headCommit,
    implementationFingerprint,
    owners: [],
    changedPaths: Object.keys(implementationFiles),
    ...layers,
    positiveReceiptPaths,
    negativeControlPaths,
    negativeControlSummary: { totalCount: negativeTotal, failedCount: negativeFailed },
    archivedReceiptCount: discovery.archivedCount,
    evidenceArtifactPaths: discovery.evidenceArtifactPaths,
    privacyStatus: {
      performed: privacyPerformed,
      pass: privacyPass && canaryMatches === 0,
      rawContentReturned,
      canaryMatches,
    },
    interferenceStatus: {
      monitored: interferenceMonitored,
      pass: interferencePass,
      disposition: interferenceDisposition,
    },
    cleanupStatus: {
      closed: cleanupClosed,
      ownedPids: [],
      ownedSessions: [],
      ownedBrowserPids: [],
      survivors,
    },
    generatedProjectionStatus: {
      applicability: "not-applicable",
      reason: "generated-output freshness is audited at program scope (verify-all)",
      pass: true,
      outputHashes: {},
    },
    progressNote: progressEntry
      ? {
          path: progress.path,
          sectionAnchor: `${taskId} — ${progressEntry.title}`,
          sectionSha256: progressEntry.sectionSha256,
        }
      : null,
    identities: {
      receiptSchemaVersion: RECEIPT_SCHEMA_VERSION,
      receiptRegistryVersion: current.registry.registryVersion,
      receiptRegistryFingerprint: current.registry.registryFingerprint,
      producerSourceFingerprints,
      implementationFiles,
      fixtureHashes,
      protectedHashes: {},
      binaries,
      generatedOutputHashes: {},
      comparisonBases: [],
    },
    staleReasons,
    errors,
  };

  return finalizeAggregate("devtools.consistency.verify-task", "consistency.verify-task", disposition, body);
}

// ── verify-family ───────────────────────────────────────────────────────────

export interface VerifyFamilyInput {
  familyId: string;
  receiptsRoot: string;
  current: CurrentIdentity;
}

function resolveFamilyMemberReceiptPath(
  receiptsRoot: string,
  declaredPath: string,
): string | null {
  const rootPath = resolve(receiptsRoot);
  const cwdPath = resolve(declaredPath);
  const candidate = isAbsolute(declaredPath)
    ? cwdPath
    : cwdPath === rootPath || cwdPath.startsWith(`${rootPath}${sep}`)
      ? cwdPath
      : resolve(rootPath, declaredPath);
  const relativePath = relative(rootPath, candidate);
  if (
    relativePath === ".." ||
    relativePath.startsWith(`..${sep}`) ||
    isAbsolute(relativePath)
  ) {
    return null;
  }
  return candidate;
}

export function verifyFamily(input: VerifyFamilyInput): { receipt: JsonObject; exitCode: number } {
  const { familyId, receiptsRoot, current } = input;
  const errors: Array<{ code: string; detail: unknown }> = [];
  const fixturePath = join(receiptsRoot, "families", familyId, "fixture.json");
  let binding: JsonObject | null = null;
  let memberReceiptCount = 0;
  let runtimeProofCount = 0;
  const runtimeProofPaths: string[] = [];
  const unprovenMemberReceiptPaths: string[] = [];
  let disposition: ReceiptDisposition;

  if (!(FAMILY_IDS as readonly string[]).includes(familyId)) {
    errors.push({ code: "unknown-family-id", detail: familyId });
    disposition = "INVALID_SCHEMA";
  } else if (!existsSync(fixturePath)) {
    errors.push({ code: "missing-family-binding", detail: fixturePath });
    disposition = "BLOCKED_MISSING_PRIMITIVE";
  } else {
    try {
      binding = asObject(JSON.parse(readFileSync(fixturePath, "utf8")));
    } catch {
      errors.push({ code: "unreadable-family-binding", detail: fixturePath });
      disposition = "INVALID_SCHEMA";
    }
    if (binding) {
      const members = Array.isArray(binding.memberReceiptPaths) ? binding.memberReceiptPaths : [];
      memberReceiptCount = members.length;
      const appView = typeof binding.appView === "string" ? binding.appView : "";
      const host = typeof binding.host === "string" ? binding.host : "";
      const declaredFamily = typeof binding.familyId === "string" ? binding.familyId : "";
      const expectedAppView = typeof binding.expectedAppView === "string" ? binding.expectedAppView : null;
      const expectedHost = typeof binding.expectedHost === "string" ? binding.expectedHost : null;
      if (
        declaredFamily !== familyId ||
        appView.length === 0 ||
        host.length === 0 ||
        (expectedAppView !== null && expectedAppView !== appView) ||
        (expectedHost !== null && expectedHost !== host)
      ) {
        errors.push({
          code: "wrong-family-appview-host",
          detail: `declared family=${declaredFamily || "(empty)"} appView=${appView || "(empty)"} host=${host || "(empty)"}`,
        });
        disposition = "EVALUABLE_FAIL";
      } else if (memberReceiptCount === 0) {
        errors.push({ code: "missing-family-member-receipts", detail: fixturePath });
        disposition = "BLOCKED_MISSING_PRIMITIVE";
      } else {
        const memberDispositions: ReceiptDisposition[] = [];
        for (const declaredPath of members) {
          if (typeof declaredPath !== "string" || declaredPath.trim().length === 0) {
            errors.push({ code: "invalid-family-member-path", detail: declaredPath });
            memberDispositions.push("INVALID_SCHEMA");
            continue;
          }
          const memberPath = resolveFamilyMemberReceiptPath(
            receiptsRoot,
            declaredPath,
          );
          if (memberPath === null) {
            errors.push({
              code: "family-member-path-escapes-receipts-root",
              detail: declaredPath,
            });
            unprovenMemberReceiptPaths.push(declaredPath);
            memberDispositions.push("INVALID_SCHEMA");
            continue;
          }
          let member: JsonObject;
          try {
            member = resolveReceiptDetails(readReceiptDocument(memberPath), memberPath);
          } catch {
            errors.push({
              code: "missing-or-unreadable-family-member-receipt",
              detail: memberPath,
            });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push("BLOCKED_MISSING_PRIMITIVE");
            continue;
          }

          if (
            member.evidenceClass !== "RUNTIME_HIDDEN" &&
            member.evidenceClass !== "RUNTIME_VISIBLE" &&
            member.evidenceClass !== "PACKAGED_APP"
          ) {
            errors.push({
              code: "family-member-not-direct-runtime-evidence",
              detail: `${memberPath}: ${String(member.evidenceClass ?? "undeclared")}`,
            });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push("INVALID_SCHEMA");
            continue;
          }
          if (member.disposition !== "EVALUABLE_PASS" || member.pass !== true) {
            errors.push({
              code: "family-member-not-passing",
              detail: `${memberPath}: ${String(member.disposition ?? "missing")}`,
            });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push("EVALUABLE_FAIL");
            continue;
          }

          const primitiveId =
            typeof member.primitiveId === "string" ? member.primitiveId : "";
          const validation = validateReceipt(primitiveId, member);
          const producerValidation = asObject(member.producerValidation);
          if (!validation.valid || producerValidation.valid !== true) {
            errors.push({
              code: "invalid-family-member-producer-receipt",
              detail: `${memberPath}: ${validation.errors.join(", ") || "producer validation is not passing"}`,
            });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push(validation.valid ? "INVALID_SCHEMA" : validation.disposition);
            continue;
          }

          const transaction = asObject(member.transaction);
          const hostMatches =
            transaction.hostKind === host ||
            transaction.windowKind === host ||
            (host === "MainWindow" && transaction.windowKind === "Main");
          if (transaction.appViewVariant !== appView || !hostMatches) {
            errors.push({
              code: "family-member-target-identity-mismatch",
              detail:
                `${memberPath}: expected ${appView}@${host}, got ` +
                `${String(transaction.appViewVariant ?? "missing")}@` +
                `${String(transaction.hostKind ?? transaction.windowKind ?? "missing")}`,
            });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push("INVALID_IDENTITY");
            continue;
          }

          const privacy = asObject(member.privacy);
          const privacyScan = asObject(privacy.recursiveCanaryScan);
          if (
            privacy.rawContentReturned === true ||
            privacyScan.performed !== true ||
            privacyScan.pass !== true ||
            Number(privacy.canaryMatches ?? 0) > 0
          ) {
            errors.push({ code: "family-member-privacy-violation", detail: memberPath });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push("INVALID_PRIVACY");
            continue;
          }
          const cleanup = asObject(member.cleanup);
          const survivors = Array.isArray(cleanup.survivors)
            ? cleanup.survivors
            : [];
          if (cleanup.closed !== true || survivors.length > 0) {
            errors.push({ code: "family-member-cleanup-not-closed", detail: memberPath });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push("INVALID_CLEANUP");
            continue;
          }

          const stale = receiptStaleReasons(
            {
              path: memberPath,
              receipt: member,
              disposition: "EVALUABLE_PASS",
              archived: false,
            },
            current,
          );
          const repository = asObject(member.repository);
          if (
            current.headCommit &&
            repository.gitCommit !== current.headCommit
          ) {
            stale.push({
              code: "stale-repository-source-commit",
              detail: `${String(repository.gitCommit ?? "missing")} != ${current.headCommit}`,
            });
          }
          if (stale.length > 0) {
            errors.push({
              code: "stale-family-member-receipt",
              detail: { path: memberPath, reasons: stale },
            });
            unprovenMemberReceiptPaths.push(memberPath);
            memberDispositions.push("BLOCKED_STALE_GENERATION");
            continue;
          }

          runtimeProofCount += 1;
          runtimeProofPaths.push(memberPath);
        }

        disposition =
          memberDispositions.length === 0 && runtimeProofCount === memberReceiptCount
            ? "EVALUABLE_PASS"
            : rollupDisposition(memberDispositions, "BLOCKED_MISSING_PRIMITIVE");
      }
    } else {
      disposition = "INVALID_SCHEMA";
    }
  }

  const body: JsonObject = {
    evidenceClass: "DIRECT_RUNTIME_PROOF",
    familyId,
    binding: binding ?? {},
    bindingPath: fixturePath,
    memberReceiptCount,
    runtimeProofCount,
    runtimeProofPaths,
    unprovenMemberReceiptPaths,
    errors,
  };
  return finalizeAggregate("devtools.consistency.verify-family", "consistency.verify-family", disposition!, body);
}

// ── verify-scope / verify-all ───────────────────────────────────────────────

export interface VerifyScopeInput {
  scope: string;
  fixesPath: string;
  progressPath: string;
  receiptsRoot: string;
  current: CurrentIdentity;
}

function readMarkdown(path: string): string | null {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return null;
  }
}

function catalogInvalid(catalog: TaskCatalog): boolean {
  return catalog.errors.length > 0;
}

interface TaskRunResult {
  taskId: string;
  disposition: ReceiptDisposition;
  receipt: JsonObject;
}

function proofRequirementCensus(results: readonly TaskRunResult[]): JsonObject {
  const requirementNames = [
    "static-inventory",
    "unit-behavior",
    "fixture-contract",
    "direct-runtime",
  ] as const;
  const requirements: Record<string, JsonObject> = {};
  for (const requirement of requirementNames) {
    const expectedIds = [...TASK_PROOF_POLICIES.values()]
      .filter((policy) => policy.requirement === requirement)
      .map((policy) => policy.taskId)
      .sort();
    const matching = results.filter((result) =>
      taskProofPolicy(result.taskId)?.requirement === requirement
    );
    const passedIds = matching
      .filter((result) => result.disposition === "EVALUABLE_PASS")
      .map((result) => result.taskId)
      .sort();
    const passing = new Set(passedIds);
    requirements[requirement] = {
      requiredTaskCount: expectedIds.length,
      passingTaskCount: passedIds.length,
      passingTaskIds: passedIds,
      unprovenTaskIds: expectedIds.filter((taskId) => !passing.has(taskId)),
    };
  }
  const directRuntime = requirements["direct-runtime"]!;
  return {
    requirements,
    runtimeInteractionRequiredTaskCount: directRuntime.requiredTaskCount,
    runtimeInteractionProvenTaskCount: directRuntime.passingTaskCount,
    runtimeInteractionBlockedTaskIds: directRuntime.unprovenTaskIds,
    note:
      "Static inventories, unit behavior, and deterministic fixture contracts never count as direct runtime interaction evidence.",
  };
}

function runTasks(
  taskIds: ReadonlySet<string>,
  scope: string,
  fixesPath: string,
  progressPath: string,
  receiptsRoot: string,
  current: CurrentIdentity,
): { results: TaskRunResult[]; catalog: TaskCatalog | null; progress: ProgressCatalog | null; setupErrors: Array<{ code: string; detail: unknown }> } {
  const setupErrors: Array<{ code: string; detail: unknown }> = [];
  const fixesMarkdown = readMarkdown(fixesPath);
  const progressMarkdown = readMarkdown(progressPath);
  if (fixesMarkdown === null) setupErrors.push({ code: "missing-catalog-file", detail: fixesPath });
  if (progressMarkdown === null) setupErrors.push({ code: "missing-progress-file", detail: progressPath });
  if (fixesMarkdown === null || progressMarkdown === null) {
    return { results: [], catalog: null, progress: null, setupErrors };
  }
  const catalog = parseTaskCatalog(fixesMarkdown, fixesPath);
  const progress = parseProgressSections(progressMarkdown, progressPath);
  const results: TaskRunResult[] = [];
  for (const taskId of [...taskIds].sort()) {
    const { receipt } = verifyTask({ taskId, scope, receiptsRoot, catalog, progress, current });
    results.push({ taskId, disposition: receipt.disposition as ReceiptDisposition, receipt });
  }
  return { results, catalog, progress, setupErrors };
}

export function verifyScope(input: VerifyScopeInput): { receipt: JsonObject; exitCode: number } {
  const scopeIds = KNOWN_SCOPES[input.scope];
  if (!scopeIds) {
    const body: JsonObject = {
      scope: input.scope,
      catalogTaskCount: 0,
      scopeTaskCount: 0,
      scopePassedTaskCount: 0,
      missingScopeTaskIds: [],
      taskDispositions: {},
      errors: [{ code: "unknown-scope", detail: input.scope }],
    };
    return finalizeAggregate("devtools.consistency.verify-scope", "consistency.verify-scope", "INVALID_SCHEMA", body);
  }
  const { results, catalog, setupErrors } = runTasks(
    scopeIds,
    input.scope,
    input.fixesPath,
    input.progressPath,
    input.receiptsRoot,
    input.current,
  );
  const errors: Array<{ code: string; detail: unknown }> = [...setupErrors];
  if (catalog) errors.push(...catalog.errors.map((error) => ({ code: error.code, detail: error.detail })));

  const taskDispositions: Record<string, string> = {};
  const missingScopeTaskIds: string[] = [];
  let scopePassedTaskCount = 0;
  for (const result of results) {
    taskDispositions[result.taskId] = result.disposition;
    if (result.disposition === "EVALUABLE_PASS") scopePassedTaskCount += 1;
    const receiptErrors = Array.isArray(result.receipt.errors) ? result.receipt.errors : [];
    if (receiptErrors.some((error) => asObject(error).code === "missing-task-receipt")) {
      missingScopeTaskIds.push(result.taskId);
    }
  }

  const dispositions = results.map((result) => result.disposition);
  let disposition: ReceiptDisposition;
  if (catalog === null || catalogInvalid(catalog)) {
    disposition = "INVALID_SCHEMA";
  } else if (dispositions.some((d) => d.startsWith("INVALID_") || d === "ANALYSIS_PENDING")) {
    disposition = "INVALID_SCHEMA";
  } else if (dispositions.some((d) => d.startsWith("BLOCKED_"))) {
    disposition = rollupDisposition(dispositions.filter((d) => d.startsWith("BLOCKED_")), "BLOCKED_MISSING_PRIMITIVE");
  } else if (dispositions.some((d) => d === "EVALUABLE_FAIL")) {
    disposition = "EVALUABLE_FAIL";
  } else {
    disposition = "EVALUABLE_PASS";
  }

  const body: JsonObject = {
    scope: input.scope,
    catalogTaskCount: catalog?.tasks.length ?? 0,
    catalogSha256: catalog?.catalogSha256 ?? null,
    scopeTaskCount: scopeIds.size,
    scopePassedTaskCount,
    missingScopeTaskIds,
    taskDispositions,
    headCommit: input.current.headCommit,
    registry: input.current.registry,
    errors,
  };
  return finalizeAggregate("devtools.consistency.verify-scope", "consistency.verify-scope", disposition, body);
}

export interface VerifyAllInput {
  fixesPath: string;
  progressPath: string;
  receiptsRoot: string;
  current: CurrentIdentity;
}

function readJson(path: string): JsonObject | null {
  try {
    return resolveReceiptDetails(readReceiptDocument(path), path);
  } catch {
    return null;
  }
}

export function verifyAll(input: VerifyAllInput): { receipt: JsonObject; exitCode: number } {
  const { results, catalog, setupErrors } = runTasks(
    PROGRAM_IDS,
    "program",
    input.fixesPath,
    input.progressPath,
    input.receiptsRoot,
    input.current,
  );
  const errors: Array<{ code: string; detail: unknown }> = [...setupErrors];
  if (catalog) errors.push(...catalog.errors.map((error) => ({ code: error.code, detail: error.detail })));

  const missingTaskIds: string[] = [];
  const blockedTaskIds: string[] = [];
  const invalidTaskIds: string[] = [];
  const failedTaskIds: string[] = [];
  let passedTaskCount = 0;
  let privacyPass = true;
  let cleanupClosed = true;
  const taskDispositions: Record<string, string> = {};

  for (const result of results) {
    taskDispositions[result.taskId] = result.disposition;
    const receiptErrors = Array.isArray(result.receipt.errors) ? result.receipt.errors : [];
    const missingReceipt = receiptErrors.some((error) => asObject(error).code === "missing-task-receipt");
    if (result.disposition === "EVALUABLE_PASS") {
      passedTaskCount += 1;
    } else if (missingReceipt) {
      missingTaskIds.push(result.taskId);
    } else if (result.disposition.startsWith("BLOCKED_")) {
      blockedTaskIds.push(result.taskId);
    } else if (result.disposition === "EVALUABLE_FAIL") {
      failedTaskIds.push(result.taskId);
    } else {
      invalidTaskIds.push(result.taskId);
    }
    if (asObject(result.receipt.privacyStatus).pass === false) privacyPass = false;
    if (asObject(result.receipt.cleanupStatus).closed === false) cleanupClosed = false;
  }

  // Protected hashes: authoritative list from the integration-run manifest.
  const runManifest = readJson(join(input.receiptsRoot, "run.json"));
  let protectedHashesPass = false;
  if (!runManifest) {
    errors.push({ code: "missing-run-manifest", detail: join(input.receiptsRoot, "run.json") });
  } else {
    const protectedPaths = Array.isArray(runManifest.protectedPaths) ? runManifest.protectedPaths : [];
    if (protectedPaths.length === 0) {
      errors.push({ code: "missing-protected-paths", detail: "run.json declares no protected paths" });
    } else {
      protectedHashesPass = true;
      for (const entry of protectedPaths) {
        const record = asObject(entry);
        const path = typeof record.path === "string" ? record.path : "";
        const expected = typeof record.sha256 === "string" ? record.sha256 : "";
        if (!path || !expected || input.current.fileSha256(path) !== expected) {
          protectedHashesPass = false;
          errors.push({ code: "protected-hash-drift", detail: path || "(malformed protected path entry)" });
        }
      }
    }
  }

  // Generated outputs: GOV-005 byte-compare receipt must exist, prove byte
  // equality through the exporter, and match the checked-in bytes right now.
  const byteCompare = readJson(join(input.receiptsRoot, "GOV-005", "generated-byte-compare.json"));
  let generatedOutputsPass = false;
  if (!byteCompare) {
    errors.push({ code: "missing-generated-byte-compare", detail: "GOV-005/generated-byte-compare.json" });
  } else if (byteCompare.byteEqual !== true || byteCompare.handEditedGeneratedOutput === true) {
    errors.push({ code: "hand-edited-generated-output", detail: "generated outputs are not exporter-derived byte-stable" });
  } else {
    const verification = validateGeneratedByteCompareReceipt(byteCompare, {
      currentSourceSha: input.current.headCommit,
      currentFileSha256: input.current.fileSha256,
    });
    if (!verification.pass) {
      errors.push({
        code: "invalid-generated-byte-compare",
        detail: verification.errors,
      });
    } else {
      generatedOutputsPass = true;
      // Keep the existing stable stale-output error code as an independent
      // defense; the typed validator already requires both exact output paths.
      for (const path of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
        const expected = asObject(byteCompare.outputHashes)[path];
        if (input.current.fileSha256(path) !== expected) {
          generatedOutputsPass = false;
          errors.push({ code: "stale-generated-output", detail: path });
        }
      }
    }
  }

  // Conflict lifecycle: GOV-005 conflicts receipt with the authorized count.
  const conflicts = readJson(join(input.receiptsRoot, "GOV-005", "conflicts.json"));
  let conflictLifecyclePass = false;
  if (!conflicts) {
    errors.push({ code: "missing-conflict-lifecycle", detail: "GOV-005/conflicts.json" });
  } else {
    const observed = Number(conflicts.observedConflictCount ?? -1);
    const classified = Number(conflicts.classifiedConflictCount ?? -1);
    const duplicateIds = Array.isArray(conflicts.duplicateIds) ? conflicts.duplicateIds : ["missing"];
    const unowned = Array.isArray(conflicts.unownedHighConflicts) ? conflicts.unownedHighConflicts : ["missing"];
    const incomplete = Array.isArray(conflicts.incompleteLifecycleRecords) ? conflicts.incompleteLifecycleRecords : ["missing"];
    if (observed !== AUTHORIZED_CONFLICT_COUNT || classified !== AUTHORIZED_CONFLICT_COUNT) {
      errors.push({
        code: "conflict-count-drift",
        detail: `observed=${observed} classified=${classified} authorized=${AUTHORIZED_CONFLICT_COUNT}`,
      });
    } else if (duplicateIds.length > 0 || unowned.length > 0 || incomplete.length > 0) {
      errors.push({ code: "incomplete-conflict-lifecycle", detail: { duplicateIds, unowned, incomplete } });
    } else {
      conflictLifecyclePass = true;
    }
  }

  // Façade lifecycle: GOV-002 executable ledger must prove both retired
  // modules, canonical owners, real migrated callers, and exact source bytes.
  const facadeLedger = readJson(join(input.receiptsRoot, "GOV-002", "facade-ledger.json"));
  let facadeLifecyclePass = false;
  if (!facadeLedger) {
    errors.push({ code: "missing-facade-ledger", detail: "GOV-002/facade-ledger.json" });
  } else {
    const assertions = asObject(facadeLedger.assertions);
    const failed = FACADE_LEDGER_ASSERTIONS.filter((field) => assertions[field] !== true);
    const scope = asObject(facadeLedger.facadeMigrations);
    const scopeFailures = validateCompleteFacadeMigrationScope(scope);
    if (
      !Array.isArray(facadeLedger.facades) ||
      JSON.stringify(facadeLedger.facades) !== JSON.stringify(scope.facades)
    ) {
      scopeFailures.push("top-level-facade-records-disagree-with-scope");
    }
    if (
      facadeLedger.evidenceClass !== "STATIC_INVENTORY" ||
      facadeLedger.provesRuntimeBehavior !== false ||
      facadeLedger.provesExporterByteEquality !== false
    ) {
      scopeFailures.push("facade-ledger-overclaims-proof-class");
    }
    // Never let an untrusted malformed ledger turn identity verification into
    // reads of arbitrary absolute paths or files outside the Rust source tree.
    if (scopeFailures.length === 0) {
      for (const candidate of Array.isArray(scope.sourceDigests) ? scope.sourceDigests : []) {
        const record = asObject(candidate);
        const path = typeof record.path === "string" ? record.path : "";
        if (path.length === 0) continue;
        const actual = input.current.fileSha256(path);
        const expected = record.state === "ABSENT" ? null : record.sha256;
        if (actual !== expected) {
          scopeFailures.push("facade-source-identity-drift:" + path);
        }
      }
    }
    if (
      failed.length > 0 ||
      scopeFailures.length > 0 ||
      facadeLedger.disposition !== "EVALUABLE_PASS"
    ) {
      errors.push({
        code: "incomplete-facade-lifecycle",
        detail: { failedAssertions: failed, scopeFailures },
      });
    } else {
      facadeLifecyclePass = true;
    }
  }

  const dispositions = results.map((result) => result.disposition);
  let disposition: ReceiptDisposition;
  if (catalog === null || catalogInvalid(catalog)) {
    disposition = "INVALID_SCHEMA";
  } else if (dispositions.some((d) => d.startsWith("INVALID_") || d === "ANALYSIS_PENDING")) {
    disposition = "INVALID_SCHEMA";
  } else if (
    missingTaskIds.length > 0 ||
    blockedTaskIds.length > 0 ||
    !protectedHashesPass ||
    !generatedOutputsPass ||
    !conflictLifecyclePass ||
    !facadeLifecyclePass
  ) {
    disposition = "BLOCKED_MISSING_PRIMITIVE";
  } else if (failedTaskIds.length > 0 || !privacyPass || !cleanupClosed) {
    disposition = "EVALUABLE_FAIL";
  } else {
    disposition = "EVALUABLE_PASS";
  }

  const body: JsonObject = {
    programTaskCount: PROGRAM_IDS.size,
    catalogTaskCount: catalog?.tasks.length ?? 0,
    catalogSha256: catalog?.catalogSha256 ?? null,
    passedTaskCount,
    missingTaskIds,
    blockedTaskIds,
    invalidTaskIds,
    failedTaskIds,
    taskDispositions,
    proofCoverage: proofRequirementCensus(results),
    privacyPass,
    cleanup: { closed: cleanupClosed },
    protectedHashesPass,
    generatedOutputsPass,
    conflictLifecyclePass,
    facadeLifecyclePass,
    headCommit: input.current.headCommit,
    registry: input.current.registry,
    errors,
  };
  return finalizeAggregate("devtools.consistency.verify-all", "consistency.verify-all", disposition, body);
}

// ── main ────────────────────────────────────────────────────────────────────

function writeAggregate(outPath: string, receipt: JsonObject) {
  mkdirSync(resolve(outPath, ".."), { recursive: true });
  const temporary = `${outPath}.tmp-${process.pid}`;
  writeFileSync(temporary, `${JSON.stringify(receipt, null, 2)}\n`);
  renameSync(temporary, outPath);
}

const DEFAULT_PROGRESS_PATH = ".notes/CONSISTENCY-PROGRESS.md";

export async function main(argv: string[]): Promise<number> {
  let command: ConsistencyCommand;
  try {
    command = parseArgs(argv);
  } catch (error) {
    if (error instanceof UsageError) {
      console.error(`usage error: ${error.message}`);
      return 64;
    }
    throw error;
  }

  const current = currentIdentity();

  switch (command.kind) {
    case "catalog": {
      const markdown = readMarkdown(command.fixesPath);
      if (markdown === null) {
        console.error(`usage error: catalog file not found: ${command.fixesPath}`);
        return 64;
      }
      const catalog = parseTaskCatalog(markdown, command.fixesPath);
      const missingTaskIds = catalog.errors.filter((e) => e.code === "missing-task-id").map((e) => e.detail);
      const unknownTaskIds = catalog.errors.filter((e) => e.code === "unknown-task-id").map((e) => e.detail);
      const disposition: ReceiptDisposition = catalog.errors.length === 0 ? "EVALUABLE_PASS" : "INVALID_SCHEMA";
      const { receipt, exitCode } = finalizeAggregate(
        "devtools.consistency.catalog",
        "consistency.catalog",
        disposition,
        {
          evidenceClass: "STATIC_INVENTORY",
          provesRuntimeBehavior: false,
          catalogPath: command.fixesPath,
          catalogSha256: catalog.catalogSha256,
          catalogTaskCount: catalog.tasks.length,
          expectedProgramTaskCount: PROGRAM_IDS.size,
          expectedScopeTaskCount: CONS_PROOF_GOV_IDS.size,
          missingTaskIds,
          unknownTaskIds,
          duplicateTaskIds: catalog.duplicateIds,
          errors: catalog.errors,
          tasks: catalog.tasks.map((task) => ({
            id: task.id,
            title: task.title,
            line: task.line,
            sectionSha256: task.sectionSha256,
          })),
        },
      );
      console.log(JSON.stringify(receipt, null, 2));
      return exitCode;
    }
    case "verify-task": {
      const fixesMarkdown = readMarkdown(command.fixesPath);
      const progressMarkdown = readMarkdown(DEFAULT_PROGRESS_PATH);
      if (fixesMarkdown === null || progressMarkdown === null) {
        console.error(
          `usage error: catalog or progress file missing: ${command.fixesPath}, ${DEFAULT_PROGRESS_PATH}`,
        );
        return 64;
      }
      const scope = CONS_PROOF_GOV_IDS.has(command.taskId)
        ? "cons-proof-gov"
        : CONS_FLOW_UX_IDS.has(command.taskId)
          ? "cons-flow-ux"
          : "program";
      const { receipt, exitCode } = verifyTask({
        taskId: command.taskId,
        scope,
        receiptsRoot: command.receiptsRoot,
        catalog: parseTaskCatalog(fixesMarkdown, command.fixesPath),
        progress: parseProgressSections(progressMarkdown, DEFAULT_PROGRESS_PATH),
        current,
      });
      writeAggregate(command.outPath, receipt);
      console.log(JSON.stringify(receipt, null, 2));
      return exitCode;
    }
    case "verify-family": {
      const { receipt, exitCode } = verifyFamily({
        familyId: command.familyId,
        receiptsRoot: command.receiptsRoot,
        current,
      });
      if (command.outPath) writeAggregate(command.outPath, receipt);
      console.log(JSON.stringify(receipt, null, 2));
      return exitCode;
    }
    case "verify-scope": {
      const { receipt, exitCode } = verifyScope({
        scope: command.scope,
        fixesPath: command.fixesPath,
        progressPath: DEFAULT_PROGRESS_PATH,
        receiptsRoot: command.receiptsRoot,
        current,
      });
      writeAggregate(command.outPath, receipt);
      console.log(JSON.stringify(receipt, null, 2));
      return exitCode;
    }
    case "verify-all": {
      const { receipt, exitCode } = verifyAll({
        fixesPath: command.fixesPath,
        progressPath: DEFAULT_PROGRESS_PATH,
        receiptsRoot: command.receiptsRoot,
        current,
      });
      writeAggregate(command.outPath, receipt);
      console.log(JSON.stringify(receipt, null, 2));
      return exitCode;
    }
  }
}

if (import.meta.main) {
  process.exit(await main(process.argv.slice(2)));
}
