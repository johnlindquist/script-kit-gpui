#!/usr/bin/env bun

import { createHash, randomUUID } from "node:crypto";
import {
  constants,
  copyFileSync,
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import {
  basename,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";
import { isDeepStrictEqual } from "node:util";

export const OUTPUT_OWNER_FILE = ".artifact-lifecycle-owner.json";
const OWNER = "script-kit-gpui-probe" as const;
const approvedStagingAnchors = new WeakSet<object>();
const exactBufferEquals = Function.prototype.call.bind(Buffer.prototype.equals) as (
  buffer: Buffer,
  otherBuffer: Uint8Array,
) => boolean;

export interface ProbeOutputOwner {
  schemaVersion: 1;
  owner: typeof OWNER;
  probeId: string;
  runId: string;
  canonicalRoot: string;
  token: string;
  createdAt: string;
}

export interface OutputPlan {
  kind: "directory" | "receipt";
  probeId: string;
  repoRoot: string;
  target: string;
  canonicalTarget: string;
  allowedAnchor: string;
  lexicalAnchor: string;
}

export interface OutputClaim {
  plan: OutputPlan;
  root: string;
  receiptPath: string;
  artifactsRoot: string;
  owner: ProbeOutputOwner;
  markerPath: string;
}

export interface ApprovedStagingAnchor {
  canonicalParent: string;
  parentDevice: number;
  parentInode: number;
  claimRunId: string;
  claimToken: string;
}

export interface CommitHooks {
  beforeCommit?: () => void;
}

interface RegularFileIdentity {
  device: number;
  inode: number;
}

export interface ArtifactSpec {
  id: string;
  sourceName: string;
  destinationName?: string;
  required: boolean;
  mediaType: string;
  kind: "text" | "json" | "ndjson";
  requireNonEmpty?: boolean;
  acceptedTextMarkers?: string[];
  correlations?: ProtocolCorrelation[];
}

export interface RetainedArtifact {
  id: string;
  sourceName: string;
  stagedPath: string;
  required: boolean;
  device: number;
  inode: number;
}

export interface ProtocolCorrelation {
  requestId: string;
  expectedType: string;
  requireNestedResponse?: boolean;
}

export interface ArtifactReceipt {
  id: string;
  required: boolean;
  identity: {
    sourceName: string;
    destinationName: string;
    kind: "text" | "json" | "ndjson";
    mediaType: string;
  };
  path: string;
  relativePath: string;
  mediaType: string;
  bytes: number;
  sha256: string;
  finalizedAfterWriters: true;
  readable: boolean;
  validation: {
    kind: "text" | "json" | "ndjson";
    parsed: boolean;
    semanticallyNonEmpty: boolean;
    recordCount?: number;
    correlation?: {
      expected: number;
      matchedExactlyOnce: number;
      missing: string[];
      duplicates: string[];
      unexpectedType: string[];
    };
    failures: string[];
  };
}

export interface ArtifactLifecycleReceipt {
  schemaVersion: 1;
  phase: "committed";
  output: {
    root: string;
    receiptPath: string;
    ownershipVerifiedBeforeMutation: true;
    runId: string;
  };
  finalization: {
    kind: "driver-close" | "strict-session-stop";
    writersFinalized: boolean;
    completedAt: string;
  };
  requiredArtifactIds: string[];
  optionalArtifactIds: string[];
  artifacts: ArtifactReceipt[];
  missingRequired: string[];
  invalidRequired: string[];
  allRequiredValid: boolean;
  recordedPaths: string[];
  allRecordedPathsReadable: boolean;
}

function canonicalizeThroughNearestExisting(path: string): string {
  const missing: string[] = [];
  let cursor = resolve(path);
  while (!existsSync(cursor)) {
    const parent = dirname(cursor);
    if (parent === cursor) throw new Error(`no existing ancestor for ${path}`);
    missing.unshift(basename(cursor));
    cursor = parent;
  }
  return resolve(realpathSync.native(cursor), ...missing);
}

export function isStrictDescendant(root: string, candidate: string): boolean {
  const rel = relative(root, candidate);
  return (
    rel.length > 0
    && rel !== ".."
    && !rel.startsWith(`..${sep}`)
    && !isAbsolute(rel)
  );
}

function existingPathComponents(anchor: string, candidate: string): string[] {
  const rel = relative(anchor, candidate);
  const components = rel.split(sep).filter(Boolean);
  const paths: string[] = [];
  let cursor = anchor;
  for (const component of components) {
    cursor = join(cursor, component);
    if (!existsSync(cursor)) break;
    paths.push(cursor);
  }
  return paths;
}

function assertNoSymlinkComponents(anchor: string, candidate: string): void {
  for (const path of existingPathComponents(anchor, candidate)) {
    if (lstatSync(path).isSymbolicLink()) {
      throw new Error(`output path contains symlink component: ${path}`);
    }
  }
}

function assertLeafName(name: string, label: string): void {
  if (
    !name
    || name === "."
    || name === ".."
    || basename(name) !== name
    || name.includes("/")
    || name.includes("\\")
  ) {
    throw new Error(`${label} must be a single artifact leaf name: ${JSON.stringify(name)}`);
  }
}

function allowedAnchors(repoRoot: string): Array<{ lexical: string; canonical: string }> {
  return [
    {
      lexical: resolve(join(repoRoot, ".test-output")),
      canonical: canonicalizeThroughNearestExisting(join(repoRoot, ".test-output")),
    },
    {
      lexical: resolve(tmpdir()),
      canonical: canonicalizeThroughNearestExisting(tmpdir()),
    },
  ];
}

/** Pure validation: this function performs no filesystem mutation. */
export function validateOutputTarget(options: {
  repoRoot: string;
  candidate: string;
  kind: "directory" | "receipt";
  probeId: string;
}): OutputPlan {
  const repoRoot = realpathSync.native(resolve(options.repoRoot));
  const target = resolve(options.candidate);
  const canonicalTarget = canonicalizeThroughNearestExisting(target);
  const anchor = allowedAnchors(repoRoot).find(({ canonical }) =>
    isStrictDescendant(canonical, canonicalTarget)
  );
  if (!anchor) {
    throw new Error(
      `unsafe ${options.kind} target outside canonical .test-output/tmp roots: ${target}`,
    );
  }
  assertNoSymlinkComponents(anchor.lexical, target);

  if (existsSync(target)) {
    const stat = lstatSync(target);
    if (options.kind === "directory") {
      if (!stat.isDirectory()) throw new Error(`output target is not a directory: ${target}`);
      if (readdirSync(target).length > 0) {
        throw new Error(`output directory must be absent or empty: ${target}`);
      }
    } else {
      throw new Error(`receipt target already exists and will not be overwritten: ${target}`);
    }
  } else {
    const parent = canonicalizeThroughNearestExisting(dirname(target));
    if (!isStrictDescendant(anchor.canonical, parent) && parent !== anchor.canonical) {
      throw new Error(`output parent escapes allowed anchor: ${parent}`);
    }
  }

  return {
    kind: options.kind,
    probeId: options.probeId,
    repoRoot,
    target,
    canonicalTarget,
    allowedAnchor: anchor.canonical,
    lexicalAnchor: anchor.lexical,
  };
}

export function claimOutput(plan: OutputPlan, runId = randomUUID()): OutputClaim {
  if (!runId || basename(runId) !== runId || runId === "." || runId === "..") {
    throw new Error(`invalid output run id: ${JSON.stringify(runId)}`);
  }
  const token = randomUUID();
  const root =
    plan.kind === "directory"
      ? plan.target
      : join(
          dirname(plan.target),
          `${basename(plan.target, ".json")}-artifacts`,
          runId,
        );
  const currentCanonicalTarget = canonicalizeThroughNearestExisting(plan.target);
  if (currentCanonicalTarget !== plan.canonicalTarget) {
    throw new Error(`validated output target changed before claim: ${plan.target}`);
  }
  assertNoSymlinkComponents(plan.lexicalAnchor, plan.target);
  if (!isStrictDescendant(plan.allowedAnchor, currentCanonicalTarget)) {
    throw new Error(`validated output target escaped allowed anchor before claim: ${plan.target}`);
  }
  const currentCanonicalRoot = canonicalizeThroughNearestExisting(root);
  assertNoSymlinkComponents(plan.lexicalAnchor, root);
  if (!isStrictDescendant(plan.allowedAnchor, currentCanonicalRoot)) {
    throw new Error(`derived claim root escaped allowed anchor before claim: ${root}`);
  }
  if (existsSync(root)) {
    if (!lstatSync(root).isDirectory() || readdirSync(root).length > 0) {
      throw new Error(`claim root is not a fresh empty directory: ${root}`);
    }
  } else {
    mkdirSync(root, { recursive: true });
  }
  const canonicalRoot = canonicalizeThroughNearestExisting(root);
  if (!isStrictDescendant(plan.allowedAnchor, canonicalRoot)) {
    throw new Error(`claim root escaped allowed anchor: ${canonicalRoot}`);
  }
  assertNoSymlinkComponents(plan.lexicalAnchor, root);
  const owner: ProbeOutputOwner = {
    schemaVersion: 1,
    owner: OWNER,
    probeId: plan.probeId,
    runId,
    canonicalRoot,
    token,
    createdAt: new Date().toISOString(),
  };
  const markerPath = join(root, OUTPUT_OWNER_FILE);
  writeFileSync(markerPath, `${JSON.stringify(owner, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  return {
    plan,
    root,
    receiptPath: plan.kind === "receipt" ? plan.target : join(root, "receipt.json"),
    artifactsRoot: plan.kind === "receipt" ? root : join(root, "artifacts", runId),
    owner,
    markerPath,
  };
}

export function assertOutputOwnership(claim: OutputClaim): void {
  const markerStat = lstatSync(claim.markerPath);
  if (!markerStat.isFile() || markerStat.isSymbolicLink()) {
    throw new Error(`invalid ownership marker: ${claim.markerPath}`);
  }
  const disk = JSON.parse(readFileSync(claim.markerPath, "utf8")) as ProbeOutputOwner;
  const canonicalRoot = realpathSync.native(claim.root);
  if (
    disk.schemaVersion !== 1
    || disk.owner !== OWNER
    || disk.probeId !== claim.owner.probeId
    || disk.runId !== claim.owner.runId
    || disk.token !== claim.owner.token
    || disk.canonicalRoot !== canonicalRoot
    || claim.owner.canonicalRoot !== canonicalRoot
  ) {
    throw new Error(`output ownership marker/token mismatch: ${claim.markerPath}`);
  }
}

export function createOwnedStagingDirectory(
  claim: OutputClaim,
  options: {
    name?: string;
    anchor?: ApprovedStagingAnchor;
  } = {},
): string {
  assertOutputOwnership(claim);
  const name = options.name ?? `artifact-staging-${claim.owner.runId}`;
  if (!name || basename(name) !== name || name === "." || name === "..") {
    throw new Error(`invalid staging directory name: ${JSON.stringify(name)}`);
  }
  const canonicalParent = options.anchor?.canonicalParent ?? realpathSync.native(claim.root);
  if (options.anchor) {
    if (
      !approvedStagingAnchors.has(options.anchor)
      || options.anchor.claimRunId !== claim.owner.runId
      || options.anchor.claimToken !== claim.owner.token
    ) {
      throw new Error("staging parent is not approved for this output claim");
    }
    const currentParent = realpathSync.native(options.anchor.canonicalParent);
    if (currentParent !== options.anchor.canonicalParent) {
      throw new Error("approved staging parent changed before mkdir");
    }
    const parentStat = lstatSync(options.anchor.canonicalParent);
    if (
      !parentStat.isDirectory()
      || parentStat.isSymbolicLink()
      || Number(parentStat.dev) !== options.anchor.parentDevice
      || Number(parentStat.ino) !== options.anchor.parentInode
    ) {
      throw new Error("approved staging parent identity changed before mkdir");
    }
    const isBaseAnchor = allowedAnchors(claim.plan.repoRoot).some(
      ({ canonical }) => canonical === options.anchor!.canonicalParent,
    );
    if (
      options.anchor.canonicalParent !== claim.owner.canonicalRoot
      && !isBaseAnchor
    ) {
      assertOwnedAuxiliaryDirectory(claim, options.anchor.canonicalParent);
    }
  } else if (canonicalParent !== claim.owner.canonicalRoot) {
    throw new Error("staging parent is not bound to the output claim");
  }
  const staging = join(canonicalParent, name);
  if (existsSync(staging)) throw new Error(`staging directory already exists: ${staging}`);
  assertOutputOwnership(claim);
  mkdirSync(staging);
  const marker = {
    ...claim.owner,
    canonicalRoot: realpathSync.native(staging),
    markerKind: "auxiliary-staging",
    canonicalParent,
  };
  writeFileSync(join(staging, OUTPUT_OWNER_FILE), `${JSON.stringify(marker, null, 2)}\n`, {
    flag: "wx",
  });
  return staging;
}

export function approveStagingAnchor(
  claim: OutputClaim,
  parent: string,
): ApprovedStagingAnchor {
  assertOutputOwnership(claim);
  const canonicalParent = realpathSync.native(parent);
  const parentStat = lstatSync(parent);
  if (!parentStat.isDirectory() || parentStat.isSymbolicLink()) {
    throw new Error(`approved staging parent must be a real directory: ${parent}`);
  }
  const baseAnchor = allowedAnchors(claim.plan.repoRoot).find(({ canonical }) =>
    canonicalParent === canonical || isStrictDescendant(canonical, canonicalParent)
  );
  if (!baseAnchor) {
    throw new Error(`staging parent is outside the approved output anchor: ${parent}`);
  }
  if (
    canonicalParent !== baseAnchor.canonical
    && canonicalParent !== claim.owner.canonicalRoot
  ) {
    assertOwnedAuxiliaryDirectory(claim, parent);
  }
  const approval = Object.freeze({
    canonicalParent,
    parentDevice: Number(parentStat.dev),
    parentInode: Number(parentStat.ino),
    claimRunId: claim.owner.runId,
    claimToken: claim.owner.token,
  });
  approvedStagingAnchors.add(approval);
  return approval;
}

function assertOwnedAuxiliaryDirectory(claim: OutputClaim, path: string): void {
  const markerPath = join(path, OUTPUT_OWNER_FILE);
  const stat = lstatSync(markerPath);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`invalid auxiliary ownership marker: ${markerPath}`);
  }
  const disk = JSON.parse(readFileSync(markerPath, "utf8")) as ProbeOutputOwner & {
    markerKind?: string;
    canonicalParent?: string;
  };
  const canonicalPath = realpathSync.native(path);
  if (
    disk.schemaVersion !== 1
    || disk.owner !== OWNER
    || disk.probeId !== claim.owner.probeId
    || disk.runId !== claim.owner.runId
    || disk.token !== claim.owner.token
    || disk.canonicalRoot !== canonicalPath
    || disk.markerKind !== "auxiliary-staging"
    || disk.canonicalParent !== realpathSync.native(dirname(path))
  ) {
    throw new Error(`auxiliary ownership marker/token mismatch: ${markerPath}`);
  }
  if (lstatSync(path).isSymbolicLink()) {
    throw new Error(`refusing to traverse auxiliary symlink: ${path}`);
  }
}

export function removeOwnedAuxiliaryDirectory(
  claim: OutputClaim,
  path: string,
): void {
  assertOutputOwnership(claim);
  assertOwnedAuxiliaryDirectory(claim, path);
  rmSync(path, { recursive: true });
}

export function retainLiveSessionArtifacts(
  claim: OutputClaim,
  sessionDir: string,
  stagingDir: string,
  specs: ArtifactSpec[],
): RetainedArtifact[] {
  assertOutputOwnership(claim);
  assertOwnedAuxiliaryDirectory(claim, stagingDir);
  const sessionStat = statSync(sessionDir);
  const stagingStat = statSync(stagingDir);
  if (!sessionStat.isDirectory() || !stagingStat.isDirectory()) {
    throw new Error("session and staging paths must be directories");
  }
  if (sessionStat.dev !== stagingStat.dev) {
    throw new Error("session and staging directories are not on the same filesystem");
  }
  const retained: RetainedArtifact[] = [];
  for (const spec of specs) {
    assertLeafName(spec.sourceName, `artifact ${spec.id} sourceName`);
    assertLeafName(
      spec.destinationName ?? spec.sourceName,
      `artifact ${spec.id} destinationName`,
    );
    const source = join(sessionDir, spec.sourceName);
    if (!existsSync(source)) {
      if (spec.required) throw new Error(`required live artifact missing: ${source}`);
      continue;
    }
    const sourceStat = lstatSync(source);
    if (!sourceStat.isFile() || sourceStat.isSymbolicLink()) {
      throw new Error(`live artifact must be a regular file: ${source}`);
    }
    const stagedPath = join(stagingDir, spec.destinationName ?? spec.sourceName);
    linkSync(source, stagedPath);
    const linkedStat = lstatSync(stagedPath);
    if (sourceStat.dev !== linkedStat.dev || sourceStat.ino !== linkedStat.ino) {
      throw new Error(`retained artifact is not the same inode: ${source}`);
    }
    retained.push({
      id: spec.id,
      sourceName: spec.sourceName,
      stagedPath,
      required: spec.required,
      device: Number(sourceStat.dev),
      inode: Number(sourceStat.ino),
    });
  }
  return retained;
}

export async function waitForProcessesDead(
  pids: Record<string, number | null | undefined>,
  options: { timeoutMs?: number; pollMs?: number } = {},
): Promise<Record<string, boolean>> {
  const timeoutMs = options.timeoutMs ?? 5_000;
  const pollMs = options.pollMs ?? 25;
  const deadline = Date.now() + timeoutMs;
  const entries = Object.entries(pids).filter(
    (entry): entry is [string, number] => Number.isInteger(entry[1]) && entry[1]! > 0,
  );
  while (Date.now() <= deadline) {
    const result = Object.fromEntries(entries.map(([name, pid]) => [name, !processAlive(pid)]));
    if (Object.values(result).every(Boolean)) return result;
    await Bun.sleep(pollMs);
  }
  const result = Object.fromEntries(entries.map(([name, pid]) => [name, !processAlive(pid)]));
  const live = Object.entries(result).filter(([, dead]) => !dead).map(([name]) => name);
  if (live.length > 0) throw new Error(`writers still alive after ${timeoutMs}ms: ${live.join(", ")}`);
  return result;
}

function processAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code !== "ESRCH";
  }
}

function bindRegularFileIdentity(path: string, label: string): RegularFileIdentity {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`${label} must be an exclusive regular file: ${path}`);
  }
  return {
    device: Number(stat.dev),
    inode: Number(stat.ino),
  };
}

function hasRegularFileIdentity(path: string, identity: RegularFileIdentity): boolean {
  try {
    const stat = lstatSync(path);
    return (
      stat.isFile()
      && !stat.isSymbolicLink()
      && Number(stat.dev) === identity.device
      && Number(stat.ino) === identity.inode
    );
  } catch {
    return false;
  }
}

function assertRegularFileIdentity(
  path: string,
  identity: RegularFileIdentity,
  label: string,
): void {
  if (!hasRegularFileIdentity(path, identity)) {
    throw new Error(`${label} identity changed before commit: ${path}`);
  }
}

export function materializeAtomic(
  claim: OutputClaim,
  options: {
    sourceRoot: string;
    sourceName: string;
    destinationName: string;
  },
  hooks: CommitHooks = {},
): void {
  assertOutputOwnership(claim);
  assertLeafName(options.sourceName, "artifact sourceName");
  assertLeafName(options.destinationName, "artifact destinationName");
  const source = join(options.sourceRoot, options.sourceName);
  const sourceStat = lstatSync(source);
  if (!sourceStat.isFile() || sourceStat.isSymbolicLink()) {
    throw new Error(`artifact source must be a regular file: ${source}`);
  }
  assertSafeClaimDestination(claim, join(claim.artifactsRoot, options.destinationName));
  mkdirSync(claim.artifactsRoot, { recursive: true });
  const destination = join(claim.artifactsRoot, options.destinationName);
  const temporary = join(
    claim.artifactsRoot,
    `.${options.destinationName}.tmp-${claim.owner.runId}-${randomUUID()}`,
  );
  let temporaryIdentity: RegularFileIdentity | undefined;
  try {
    copyFileSync(source, temporary, constants.COPYFILE_EXCL);
    temporaryIdentity = bindRegularFileIdentity(temporary, "materialized temporary");
    hooks.beforeCommit?.();
    assertSafeClaimDestination(claim, destination);
    assertRegularFileIdentity(temporary, temporaryIdentity, "materialized temporary");
    linkSync(temporary, destination);
  } finally {
    if (temporaryIdentity && hasRegularFileIdentity(temporary, temporaryIdentity)) {
      unlinkSync(temporary);
    }
  }
}

export function sha256File(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function ndjsonValueKind(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

function responseType(record: unknown): string | null {
  if (!isObjectRecord(record)) return null;
  if (typeof record.responseType === "string") return record.responseType;
  if (typeof record.type === "string") return record.type;
  return null;
}

export function validateArtifact(
  path: string,
  spec: ArtifactSpec,
  durableRoot: string,
): ArtifactReceipt {
  assertLeafName(spec.sourceName, `artifact ${spec.id} sourceName`);
  const destinationName = spec.destinationName ?? spec.sourceName;
  assertLeafName(destinationName, `artifact ${spec.id} destinationName`);
  const resolvedRoot = resolve(durableRoot);
  const expectedPath = resolve(join(resolvedRoot, destinationName));
  const failures: string[] = [];
  let bytes = 0;
  let sha256 = "";
  let readable = false;
  let parsed = false;
  let semanticallyNonEmpty = false;
  let recordCount: number | undefined;
  let correlation: ArtifactReceipt["validation"]["correlation"];
  let content = Buffer.alloc(0);

  try {
    const rootStat = lstatSync(resolvedRoot);
    if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) {
      throw new Error("owned durable root is not a canonical directory");
    }
    const resolvedPath = resolve(path);
    const pathMatchesExpected = resolvedPath === expectedPath;
    if (!pathMatchesExpected) {
      failures.push("artifact path does not match expected durable destination");
    }
    if (!isStrictDescendant(resolvedRoot, resolvedPath)) {
      throw new Error("artifact is outside owned durable root");
    }
    for (const component of existingPathComponents(resolvedRoot, resolvedPath)) {
      if (lstatSync(component).isSymbolicLink()) {
        throw new Error("artifact path contains symlink component");
      }
    }
    const canonicalRoot = realpathSync.native(resolvedRoot);
    const canonicalPath = realpathSync.native(resolvedPath);
    if (!isStrictDescendant(canonicalRoot, canonicalPath)) {
      throw new Error("artifact is outside owned durable root");
    }
    if (pathMatchesExpected) {
      const stat = lstatSync(resolvedPath);
      if (!stat.isFile() || stat.isSymbolicLink()) throw new Error("not a regular file");
      content = readFileSync(resolvedPath);
      bytes = content.byteLength;
      sha256 = createHash("sha256").update(content).digest("hex");
      readable = true;
    }
  } catch (error) {
    failures.push(error instanceof Error ? error.message : String(error));
  }

  const text = content.toString("utf8");
  if (readable) {
    if (spec.kind === "text") {
      parsed = true;
      semanticallyNonEmpty = text.trim().length > 0;
      if (
        spec.acceptedTextMarkers?.length
        && !spec.acceptedTextMarkers.some((marker) => text.includes(marker))
      ) {
        failures.push("accepted readiness marker missing");
      }
    } else if (spec.kind === "json") {
      try {
        const value = JSON.parse(text);
        parsed = true;
        semanticallyNonEmpty =
          value !== null
          && (typeof value !== "object" || Object.keys(value as object).length > 0);
      } catch (error) {
        failures.push(`invalid JSON: ${error instanceof Error ? error.message : error}`);
      }
    } else {
      const records: Record<string, unknown>[] = [];
      let parsedRecordCount = 0;
      if (bytes > 0 && !text.endsWith("\n")) failures.push("truncated NDJSON final line");
      const lines = text.split("\n").slice(0, -1);
      for (let index = 0; index < lines.length; index += 1) {
        if (lines[index].trim().length === 0) {
          failures.push(`blank NDJSON record at line ${index + 1}`);
          continue;
        }
        try {
          const record: unknown = JSON.parse(lines[index]);
          parsedRecordCount += 1;
          if (isObjectRecord(record)) {
            records.push(record);
          } else {
            failures.push(
              `non-object NDJSON record at line ${index + 1}: ${ndjsonValueKind(record)}`,
            );
          }
        } catch (error) {
          failures.push(
            `invalid NDJSON at line ${index + 1}: ${error instanceof Error ? error.message : error}`,
          );
        }
      }
      parsed = failures.every((failure) => !failure.includes("NDJSON"));
      recordCount = parsedRecordCount;
      semanticallyNonEmpty = parsedRecordCount > 0;
      if (spec.correlations) {
        const missing: string[] = [];
        const duplicates: string[] = [];
        const unexpectedType: string[] = [];
        let matchedExactlyOnce = 0;
        for (const expected of spec.correlations) {
          const matches = records.filter((record) => record.requestId === expected.requestId);
          if (matches.length === 0) missing.push(expected.requestId);
          else if (matches.length > 1) duplicates.push(expected.requestId);
          else if (
            responseType(matches[0]) !== expected.expectedType
            || (
              expected.requireNestedResponse === true
              && (
                typeof matches[0].response !== "object"
                || matches[0].response === null
                || (matches[0].response as Record<string, unknown>).requestId !== expected.requestId
                || responseType(matches[0].response as Record<string, unknown>)
                  !== expected.expectedType
              )
            )
          ) {
            unexpectedType.push(
              `${expected.requestId}:${responseType(matches[0]) ?? "null"}!=${expected.expectedType}`
              + (expected.requireNestedResponse === true ? ":nested-response-mismatch" : ""),
            );
          } else {
            matchedExactlyOnce += 1;
          }
        }
        correlation = {
          expected: spec.correlations.length,
          matchedExactlyOnce,
          missing,
          duplicates,
          unexpectedType,
        };
        if (missing.length) failures.push(`missing correlations: ${missing.join(", ")}`);
        if (duplicates.length) failures.push(`duplicate correlations: ${duplicates.join(", ")}`);
        if (unexpectedType.length) failures.push(`wrong response types: ${unexpectedType.join(", ")}`);
      }
    }
  }

  if ((spec.requireNonEmpty ?? spec.required) && !semanticallyNonEmpty) {
    failures.push("artifact is semantically empty");
  }
  if (readable && !/^[a-f0-9]{64}$/.test(sha256)) failures.push("invalid SHA-256");

  return {
    id: spec.id,
    required: spec.required,
    identity: {
      sourceName: spec.sourceName,
      destinationName,
      kind: spec.kind,
      mediaType: spec.mediaType,
    },
    path: expectedPath,
    relativePath: destinationName,
    mediaType: spec.mediaType,
    bytes,
    sha256,
    finalizedAfterWriters: true,
    readable,
    validation: {
      kind: spec.kind,
      parsed,
      semanticallyNonEmpty,
      ...(recordCount === undefined ? {} : { recordCount }),
      ...(correlation ? { correlation } : {}),
      failures,
    },
  };
}

function freshArtifactReceipt(
  claim: OutputClaim,
  spec: ArtifactSpec,
): ArtifactReceipt {
  const destinationName = spec.destinationName ?? spec.sourceName;
  return validateArtifact(
    join(claim.artifactsRoot, destinationName),
    spec,
    claim.artifactsRoot,
  );
}

function assertCompleteArtifactReceiptMatch(
  actual: ArtifactReceipt,
  expected: ArtifactReceipt,
  label: string,
): void {
  if (!isDeepStrictEqual(actual, expected)) {
    throw new Error(`${label} does not match fresh validation for ${expected.id}`);
  }
}

function snapshotArtifactReceipt(receipt: ArtifactReceipt): ArtifactReceipt {
  return structuredClone(receipt);
}

function assembleArtifactLifecycle(
  claim: OutputClaim,
  finalization: ArtifactLifecycleReceipt["finalization"],
  specs: ArtifactSpec[],
  artifacts: ArtifactReceipt[],
): ArtifactLifecycleReceipt {
  const requiredArtifactIds = specs.filter((spec) => spec.required).map((spec) => spec.id);
  const optionalArtifactIds = specs.filter((spec) => !spec.required).map((spec) => spec.id);
  const byId = new Map(artifacts.map((artifact) => [artifact.id, artifact]));
  const canonicalArtifacts = specs.map((spec) => byId.get(spec.id)!);
  const missingRequired = requiredArtifactIds.filter((id) => {
    const artifact = byId.get(id);
    return !artifact || !artifact.readable;
  });
  const invalidRequired = requiredArtifactIds.filter((id) => {
    const artifact = byId.get(id);
    return !artifact || !artifact.readable || artifact.validation.failures.length > 0;
  });
  const recordedArtifacts = canonicalArtifacts.filter((artifact) => artifact.readable);
  const recordedPaths = recordedArtifacts.map((artifact) => artifact.path);
  const allRecordedPathsReadable = recordedArtifacts.every((artifact) => {
    try {
      return lstatSync(artifact.path).isFile() && sha256File(artifact.path) === artifact.sha256;
    } catch {
      return false;
    }
  });
  return {
    schemaVersion: 1,
    phase: "committed",
    output: {
      root: claim.root,
      receiptPath: claim.receiptPath,
      ownershipVerifiedBeforeMutation: true,
      runId: claim.owner.runId,
    },
    finalization: {
      kind: finalization.kind,
      writersFinalized: finalization.writersFinalized,
      completedAt: finalization.completedAt,
    },
    requiredArtifactIds,
    optionalArtifactIds,
    artifacts: canonicalArtifacts,
    missingRequired,
    invalidRequired,
    allRequiredValid:
      finalization.writersFinalized
      && missingRequired.length === 0
      && invalidRequired.length === 0,
    recordedPaths,
    allRecordedPathsReadable,
  };
}

export function buildArtifactLifecycle(options: {
  claim: OutputClaim;
  finalizationKind: "driver-close" | "strict-session-stop";
  writersFinalized: boolean;
  specs: ArtifactSpec[];
  artifacts: ArtifactReceipt[];
}): ArtifactLifecycleReceipt {
  assertOutputOwnership(options.claim);
  const specsById = new Map<string, ArtifactSpec>();
  for (const spec of options.specs) {
    if (specsById.has(spec.id)) {
      throw new Error(`duplicate artifact spec id: ${spec.id}`);
    }
    specsById.set(spec.id, spec);
  }
  const byId = new Map<string, ArtifactReceipt>();
  for (const artifact of options.artifacts) {
    if (byId.has(artifact.id)) {
      throw new Error(`duplicate artifact receipt id: ${artifact.id}`);
    }
    const spec = specsById.get(artifact.id);
    if (!spec) {
      throw new Error(`artifact receipt has no matching spec: ${artifact.id}`);
    }
    if (spec && spec.required !== artifact.required) {
      throw new Error(`artifact requiredness mismatch for ${artifact.id}`);
    }
    const expectedIdentity: ArtifactReceipt["identity"] = {
      sourceName: spec.sourceName,
      destinationName: spec.destinationName ?? spec.sourceName,
      kind: spec.kind,
      mediaType: spec.mediaType,
    };
    for (const field of ["sourceName", "destinationName", "kind", "mediaType"] as const) {
      if (
        artifact.identity?.[field] !== expectedIdentity[field]
        || (field === "kind" && artifact.validation.kind !== expectedIdentity.kind)
        || (field === "mediaType" && artifact.mediaType !== expectedIdentity.mediaType)
      ) {
        throw new Error(`artifact identity mismatch for ${artifact.id}: ${field}`);
      }
    }
    const fresh = freshArtifactReceipt(options.claim, spec);
    assertCompleteArtifactReceiptMatch(artifact, fresh, "artifact receipt");
    byId.set(artifact.id, snapshotArtifactReceipt(fresh));
  }
  const omittedReceiptIds = [...specsById.keys()].filter((id) => !byId.has(id));
  if (omittedReceiptIds.length > 0) {
    throw new Error(`artifact receipt set is incomplete: ${omittedReceiptIds.join(", ")}`);
  }
  return assembleArtifactLifecycle(
    options.claim,
    {
      kind: options.finalizationKind,
      writersFinalized: options.writersFinalized,
      completedAt: new Date().toISOString(),
    },
    options.specs,
    [...byId.values()],
  );
}

function writeJsonNoReplace(
  claim: OutputClaim,
  destination: string,
  value: Record<string, unknown>,
  hooks: CommitHooks,
  verifyAfterHook?: () => void,
): void {
  assertSafeClaimDestination(claim, destination);
  mkdirSync(dirname(destination), { recursive: true });
  const serialized = Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
  const temporary = join(
    dirname(destination),
    `.${basename(destination)}.tmp-${claim.owner.runId}-${randomUUID()}`,
  );
  let temporaryIdentity: RegularFileIdentity | undefined;
  try {
    writeFileSync(temporary, serialized, { flag: "wx" });
    temporaryIdentity = bindRegularFileIdentity(temporary, "JSON temporary");
    hooks.beforeCommit?.();
    verifyAfterHook?.();
    assertSafeClaimDestination(claim, destination);
    assertRegularFileIdentity(temporary, temporaryIdentity, "JSON temporary");
    if (!exactBufferEquals(readFileSync(temporary), serialized)) {
      throw new Error(`JSON temporary bytes changed before commit: ${temporary}`);
    }
    linkSync(temporary, destination);
  } finally {
    if (temporaryIdentity && hasRegularFileIdentity(temporary, temporaryIdentity)) {
      unlinkSync(temporary);
    }
  }
}

function assertSafeClaimDestination(claim: OutputClaim, destination: string): void {
  assertOutputOwnership(claim);
  const resolvedDestination = resolve(destination);
  assertNoSymlinkComponents(claim.plan.lexicalAnchor, resolvedDestination);
  const canonicalDestination = canonicalizeThroughNearestExisting(resolvedDestination);
  if (resolvedDestination === resolve(claim.receiptPath)) {
    if (resolvedDestination !== resolve(claim.plan.target) && claim.plan.kind === "receipt") {
      throw new Error(`final receipt destination does not match validated target: ${destination}`);
    }
    if (!isStrictDescendant(claim.plan.allowedAnchor, canonicalDestination)) {
      throw new Error(`final receipt destination escaped allowed anchor: ${destination}`);
    }
    return;
  }
  const canonicalRoot = realpathSync.native(claim.root);
  if (!isStrictDescendant(canonicalRoot, canonicalDestination)) {
    throw new Error(`artifact destination escaped owned claim root: ${destination}`);
  }
}

export function writeJsonArtifactAtomic(
  claim: OutputClaim,
  destinationName: string,
  value: Record<string, unknown>,
  hooks: CommitHooks = {},
): void {
  assertLeafName(destinationName, "JSON artifact destinationName");
  writeJsonNoReplace(claim, join(claim.artifactsRoot, destinationName), value, hooks);
}

export function commitFinalReceipt(
  claim: OutputClaim,
  receipt: Record<string, unknown>,
  specs: ArtifactSpec[],
  artifacts: ArtifactReceipt[],
  hooks: CommitHooks = {},
): void {
  const receiptSnapshot = structuredClone(receipt);
  const specsSnapshot = structuredClone(specs);
  const artifactsSnapshot = structuredClone(artifacts);
  if (!Array.isArray(specsSnapshot)) {
    throw new Error("final receipt requires an explicit complete artifact spec set");
  }
  if (!Array.isArray(artifactsSnapshot)) {
    throw new Error("final receipt requires an explicit complete artifact receipt set");
  }
  const lifecycleSnapshot = receiptSnapshot.artifactLifecycle as ArtifactLifecycleReceipt | undefined;
  if (
    !lifecycleSnapshot
    || !Array.isArray(lifecycleSnapshot.artifacts)
    || !lifecycleSnapshot.finalization
  ) {
    throw new Error("final receipt requires committed artifact lifecycle evidence");
  }
  const specsById = new Map<string, ArtifactSpec>();
  for (const spec of specsSnapshot) {
    if (specsById.has(spec.id)) throw new Error(`duplicate artifact spec id: ${spec.id}`);
    specsById.set(spec.id, spec);
  }
  const suppliedById = new Map<string, ArtifactReceipt>();
  for (const artifact of artifactsSnapshot) {
    if (suppliedById.has(artifact.id)) {
      throw new Error(`duplicate final receipt artifact id: ${artifact.id}`);
    }
    suppliedById.set(artifact.id, artifact);
    const spec = specsById.get(artifact.id);
    if (!spec) throw new Error(`final receipt artifact has no matching spec: ${artifact.id}`);
    if (artifact.required !== spec.required) {
      throw new Error(`final receipt artifact requiredness mismatch for ${artifact.id}`);
    }
    const declaredIdentity: ArtifactReceipt["identity"] = {
      sourceName: spec.sourceName,
      destinationName: spec.destinationName ?? spec.sourceName,
      kind: spec.kind,
      mediaType: spec.mediaType,
    };
    for (const field of ["sourceName", "destinationName", "kind", "mediaType"] as const) {
      if (
        artifact.identity?.[field] !== declaredIdentity[field]
        || (field === "kind" && artifact.validation.kind !== artifact.identity?.kind)
        || (field === "mediaType" && artifact.mediaType !== artifact.identity?.mediaType)
      ) {
        throw new Error(`final receipt artifact identity mismatch for ${artifact.id}: ${field}`);
      }
    }
  }
  if (suppliedById.size !== specsById.size) {
    throw new Error("final receipt artifact set is incomplete or does not match artifact specs");
  }

  const verifyCurrentArtifactsAndLifecycle = (): void => {
    const freshArtifacts = specsSnapshot.map((spec) => {
      const suppliedArtifact = suppliedById.get(spec.id)!;
      const fresh = freshArtifactReceipt(claim, spec);
      assertCompleteArtifactReceiptMatch(
        suppliedArtifact,
        fresh,
        "supplied artifact receipt",
      );
      return snapshotArtifactReceipt(fresh);
    });
    const expectedLifecycle = assembleArtifactLifecycle(
      claim,
      lifecycleSnapshot.finalization,
      specsSnapshot,
      freshArtifacts,
    );
    if (!isDeepStrictEqual(lifecycleSnapshot, expectedLifecycle)) {
      throw new Error("final receipt lifecycle does not match reconstructed lifecycle evidence");
    }
  };

  verifyCurrentArtifactsAndLifecycle();
  writeJsonNoReplace(
    claim,
    claim.receiptPath,
    receiptSnapshot,
    hooks,
    verifyCurrentArtifactsAndLifecycle,
  );
}

export function removeOwnedTree(claim: OutputClaim, path = claim.root): void {
  assertOutputOwnership(claim);
  const canonicalPath = realpathSync.native(path);
  const canonicalRoot = realpathSync.native(claim.root);
  if (canonicalPath !== canonicalRoot && !isStrictDescendant(canonicalRoot, canonicalPath)) {
    throw new Error(`refusing to remove path outside owned root: ${canonicalPath}`);
  }
  if (lstatSync(path).isSymbolicLink()) throw new Error(`refusing to traverse symlink: ${path}`);
  rmSync(path, { recursive: true });
}
