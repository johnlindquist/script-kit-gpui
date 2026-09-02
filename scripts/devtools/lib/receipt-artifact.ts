import { createHash } from "node:crypto";
import { closeSync, constants, fstatSync, lstatSync, openSync, readSync, realpathSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { OUTPUT_OWNER_FILE, canonicalJson, readOwnedJson, validateArtifact, type ArtifactLifecycleReceipt, type ArtifactSpec, type OutputClaim } from "../../agentic/artifact-lifecycle.ts";
import type { Json as JsonObject } from "../driver.ts";

export const OWNED_RECEIPT_FORMAT = "script-kit-owned-receipt";
export const OWNED_RECEIPT_VERSION = 1;
export const MAX_RECEIPT_DETAIL_BYTES = 64 * 1024 * 1024;
export const MAX_COMPACT_RECEIPT_BYTES = 1024 * 1024;
export const OBSERVATION_SPEC: ArtifactSpec = { id: "observation", sourceName: "observation.json", required: true, mediaType: "application/json", kind: "json" };
const summaryFields = ["schemaVersion", "primitiveId", "tool", "command", "receiptId", "runId", "generatedBy", "taskId", "classification", "disposition", "pass", "evidenceClass", "provesRuntimeBehavior"] as const;
const object = (value: unknown): JsonObject => value !== null && typeof value === "object" && !Array.isArray(value) ? value as JsonObject : {};
const fail = (reason: string): never => { throw new Error(`invalid_receipt_reference:${reason}`); };

/** Bound the allocation before reading, reject aliases, and hash the exact bytes parsed. */
function readBounded(path: string, maximum: number): Buffer {
  if (realpathSync(path) !== resolve(path)) fail("noncanonical_path");
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const before = fstatSync(fd);
    if (!before.isFile() || before.nlink !== 1 || before.size < 1 || before.size > maximum) fail("file_size_or_identity");
    const bytes = Buffer.alloc(before.size);
    let offset = 0;
    while (offset < bytes.length) {
      const count = readSync(fd, bytes, offset, bytes.length - offset, offset);
      if (!count) fail("file_changed");
      offset += count;
    }
    const after = fstatSync(fd), current = lstatSync(path);
    if (after.dev !== before.dev || after.ino !== before.ino || after.size !== before.size || after.mtimeMs !== before.mtimeMs || after.ctimeMs !== before.ctimeMs || current.dev !== before.dev || current.ino !== before.ino || current.isSymbolicLink()) fail("file_changed");
    return bytes;
  } finally { closeSync(fd); }
}

export function readReceiptDocument(path: string): JsonObject {
  try {
    const value = JSON.parse(readBounded(realpathSync(path), MAX_RECEIPT_DETAIL_BYTES).toString("utf8"));
    if (value === null || typeof value !== "object" || Array.isArray(value)) fail("document_object_required");
    if (isReferenceReceipt(value) && realpathSync(path) !== resolve(path)) fail("noncanonical_receipt_path");
    return value;
  } catch { return fail("unreadable_document"); }
}

export function isReferenceReceipt(receipt: JsonObject): boolean {
  return "receiptFormat" in receipt || "detailReference" in receipt;
}

export function ownedObservationDocument(claim: OutputClaim, receipt: JsonObject): JsonObject {
  if (isReferenceReceipt(receipt) || "artifactLifecycle" in receipt) fail("nested_reference");
  const document = { schemaVersion: 1, kind: "owned-receipt-observation", ownerSha256: createHash("sha256").update(canonicalJson(claim.owner)).digest("hex"), receipt };
  if (Buffer.byteLength(JSON.stringify(document, null, 2)) + 1 > MAX_RECEIPT_DETAIL_BYTES) fail("detail_size_limit");
  return document;
}

export function compactOwnedReceipt(claim: OutputClaim, receipt: JsonObject, artifactLifecycle: ArtifactLifecycleReceipt): JsonObject {
  const summary: JsonObject = {};
  for (const key of summaryFields) if (receipt[key] !== undefined) summary[key] = receipt[key];
  const wire = { receiptFormat: OWNED_RECEIPT_FORMAT, receiptFormatVersion: OWNED_RECEIPT_VERSION, ...summary,
    detailReference: { artifactId: OBSERVATION_SPEC.id, ownerSha256: createHash("sha256").update(canonicalJson(claim.owner)).digest("hex") }, artifactLifecycle };
  if (Buffer.byteLength(JSON.stringify(wire, null, 2)) + 1 > MAX_COMPACT_RECEIPT_BYTES) fail("compact_size_limit");
  return wire;
}

/** One level only: never recursively expand references or trust a summary as proof. */
function resolveOwnedReceiptDetails(receipt: JsonObject, receiptPath?: string): JsonObject {
  if (!isReferenceReceipt(receipt)) {
    if (object(receipt.artifactLifecycle).artifacts?.some?.((value: JsonObject) => value.id === "observation")) fail("obsolete_inline_owned_receipt");
    return receipt;
  }
  if (receipt.receiptFormat !== OWNED_RECEIPT_FORMAT || receipt.receiptFormatVersion !== OWNED_RECEIPT_VERSION) fail("wire_version");
  if (Object.keys(receipt).some(key => !["receiptFormat", "receiptFormatVersion", "detailReference", "artifactLifecycle", ...summaryFields].includes(key)) || Buffer.byteLength(JSON.stringify(receipt)) > MAX_COMPACT_RECEIPT_BYTES) fail("noncompact_wire");
  const lifecycle = object(receipt.artifactLifecycle), output = object(lifecycle.output), reference = object(receipt.detailReference);
  if (lifecycle.schemaVersion !== 1 || lifecycle.phase !== "committed" || output.ownershipVerifiedBeforeMutation !== true || typeof output.root !== "string" || typeof output.receiptPath !== "string" || typeof output.runId !== "string" || reference.artifactId !== "observation" || typeof reference.ownerSha256 !== "string" || !/^[a-f0-9]{64}$/.test(reference.ownerSha256) || Object.keys(reference).length !== 2) fail("lifecycle_identity");
  if (basename(output.runId) !== output.runId || [".", ".."].includes(output.runId) || summaryFields.some(key => receipt[key] !== undefined && receipt[key] !== null && !["string", "number", "boolean"].includes(typeof receipt[key]))) fail("summary_or_run_identity");
  const root = resolve(output.root);
  if (root !== output.root || realpathSync(root) !== root || !lstatSync(root).isDirectory()) fail("output_root");
  if (receiptPath !== undefined && (resolve(receiptPath) !== output.receiptPath || realpathSync(receiptPath) !== resolve(receiptPath))) fail("receipt_path_mismatch");
  const owner = readOwnedJson(join(root, OUTPUT_OWNER_FILE));
  if (owner.schemaVersion !== 1 || owner.owner !== "script-kit-gpui-probe" || owner.canonicalRoot !== root || owner.runId !== output.runId || createHash("sha256").update(canonicalJson(owner)).digest("hex") !== reference.ownerSha256) fail("output_owner_mismatch");
  const directoryReceipt = output.receiptPath === join(root, "receipt.json");
  if (!directoryReceipt && (basename(root) !== output.runId || dirname(root) !== join(dirname(output.receiptPath), `${basename(output.receiptPath, ".json")}-artifacts`))) fail("receipt_destination");
  const artifactsRoot = directoryReceipt ? join(root, "artifacts", output.runId) : root;
  const artifacts = lifecycle.artifacts;
  if (!Array.isArray(artifacts) || artifacts.length < 1) fail("artifact_count");
  const ids = new Set<string>(), paths = new Set<string>();
  let document: JsonObject | undefined;
  for (const artifact of artifacts) {
    const identity = object(artifact.identity);
    if (typeof artifact.id !== "string" || ids.has(artifact.id) || typeof artifact.relativePath !== "string" || basename(artifact.relativePath) !== artifact.relativePath || [".", ".."].includes(artifact.relativePath) || paths.has(artifact.relativePath) || identity.destinationName !== artifact.relativePath || typeof identity.sourceName !== "string" || basename(identity.sourceName) !== identity.sourceName || !["json", "text", "ndjson"].includes(identity.kind) || typeof identity.mediaType !== "string" || typeof artifact.required !== "boolean") fail("artifact_identity");
    ids.add(artifact.id); paths.add(artifact.relativePath);
    if (!Number.isSafeInteger(artifact.bytes) || artifact.bytes < 1 || artifact.bytes > MAX_RECEIPT_DETAIL_BYTES || !/^[a-f0-9]{64}$/.test(artifact.sha256)) fail("artifact_size_or_hash");
    const path = join(artifactsRoot, artifact.relativePath);
    if (artifact.path !== path) fail("artifact_path");
    const bytes = readBounded(path, artifact.bytes);
    if (bytes.length !== artifact.bytes || createHash("sha256").update(bytes).digest("hex") !== artifact.sha256) fail("artifact_size_or_hash");
    const spec: ArtifactSpec = { id: artifact.id, sourceName: identity.sourceName, destinationName: identity.destinationName, kind: identity.kind, mediaType: identity.mediaType, required: artifact.required };
    const fresh = validateArtifact(path, spec, artifactsRoot);
    if (!isDeepStrictEqual(fresh, artifact) || !fresh.readable || fresh.validation.failures.length) fail("artifact_validation");
    if (artifact.id === OBSERVATION_SPEC.id) {
      if (!artifact.required || identity.sourceName !== OBSERVATION_SPEC.sourceName || identity.destinationName !== OBSERVATION_SPEC.sourceName || identity.kind !== "json" || identity.mediaType !== "application/json") fail("observation_identity");
      document = object(JSON.parse(bytes.toString("utf8")));
    }
  }
  if (!document || document.schemaVersion !== 1 || document.kind !== "owned-receipt-observation" || document.ownerSha256 !== reference.ownerSha256) fail("observation_owner");
  if (!isDeepStrictEqual(lifecycle.requiredArtifactIds, artifacts.filter(value => value.required).map(value => value.id)) || !isDeepStrictEqual(lifecycle.optionalArtifactIds, artifacts.filter(value => !value.required).map(value => value.id)) || !isDeepStrictEqual(lifecycle.recordedPaths, artifacts.map(value => value.path)) || !isDeepStrictEqual(lifecycle.missingRequired, []) || !isDeepStrictEqual(lifecycle.invalidRequired, []) || lifecycle.allRecordedPathsReadable !== true || lifecycle.allRequiredValid !== (object(lifecycle.finalization).writersFinalized === true)) fail("lifecycle_artifact_set");
  const detail = object(document.receipt);
  if (!Object.keys(detail).length || isReferenceReceipt(detail) || "artifactLifecycle" in detail) fail("nested_reference");
  for (const key of summaryFields) if (!isDeepStrictEqual(receipt[key], detail[key])) fail(`summary_mismatch:${key}`);
  return detail;
}

/** Failures disclose stable reason codes, never paths or contents from filesystem errors. */
export function resolveReceiptDetails(receipt: JsonObject, receiptPath?: string): JsonObject {
  try { return resolveOwnedReceiptDetails(receipt, receiptPath); }
  catch (error) {
    if (error instanceof Error && error.message.startsWith("invalid_receipt_reference:")) throw error;
    return fail("unreadable_artifact");
  }
}
