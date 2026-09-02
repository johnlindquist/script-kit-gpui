#!/usr/bin/env bun

import { createHash, randomUUID } from "node:crypto";
import {
  constants,
  closeSync,
  fstatSync,
  fsyncSync,
  openSync,
  renameSync,
  chmodSync,
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
import type { ArtifactReference, Sha256, SourceIdentity } from "./build-artifact.ts";
import { spawnSync } from "node:child_process";

export interface TaskIdentity {
  readonly kind: "build-job" | "runtime-run" | "evidence-run";
  readonly id: string;
  readonly generation: string;
  readonly revision: number;
}
export interface OwnedProcessIdentity {
  readonly pid: number;
  readonly processStartTime: string;
  readonly processInstanceId: string;
  readonly processGroupId: number;
  readonly supervisorPid: number;
  readonly supervisorStartTime: string;
  readonly sessionGeneration: string;
}
export interface OwnedCleanup {
  readonly resourcesAcquired: boolean;
  readonly processExited: boolean;
  readonly processGroupExited: boolean;
  readonly streamsDrained: boolean;
  readonly logWriterClosed: boolean;
  readonly ownedWindowsClosed: boolean | null;
  readonly referencesFinalized: boolean;
  readonly closed: boolean;
  readonly survivors: readonly { kind: string; identity: string; observation: "present" | "unknown" }[];
  readonly failureCodes: readonly string[];
}
export interface ManagedKeepSet {
  schemaVersion: 1;
  revision: string;
  references: ArtifactReference[];
}
export interface ManagedPublicationIntent {
  id: string;
  generation: string;
  pendingPath: string;
  destinationPath: string;
  directoryDevice: number;
  directoryInode: number;
  phase: "pending" | "published" | "failed";
}
export interface TaskRecord {
  identity: TaskIdentity;
  state: "queued" | "running" | "finalizing" | "closed" | "protected";
  artifactReferences: ArtifactReference[];
  publicationIntents?: ManagedPublicationIntent[];
  ownedProcesses: OwnedProcessIdentity[];
  source?: SourceIdentity;
  effectiveConfiguration?: Readonly<Record<string, unknown>>;
  result: Readonly<Record<string, unknown>>;
  cleanup: OwnedCleanup;
}
export const RETENTION_CANDIDATE_KINDS = ["build-job", "runtime-run", "evidence-run", "artifact"] as const;

export interface RetentionCandidate {
  kind: typeof RETENTION_CANDIDATE_KINDS[number];
  id: string;
  generation: string;
  revision: number;
  recordSha256: Sha256;
  directoryDevice: number;
  directoryInode: number;
}
export interface ManagedTask {
  readonly identity: TaskIdentity;
  readonly recordPath: string;
}

export const OUTPUT_OWNER_FILE = ".artifact-lifecycle-owner.json";
const OWNER = "script-kit-gpui-probe" as const;
const approvedStagingAnchors = new WeakSet<object>();
const claimedOutputIdentities = new WeakMap<OutputClaim, { root: string; markerPath: string; device: number; inode: number; markerDevice: number; markerInode: number; token: string }>();
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

export function claimOutput(plan: OutputPlan, runId: string = randomUUID()): OutputClaim {
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
  const claim: OutputClaim = {
    plan,
    root,
    receiptPath: plan.kind === "receipt" ? plan.target : join(root, "receipt.json"),
    artifactsRoot: plan.kind === "receipt" ? root : join(root, "artifacts", runId),
    owner,
    markerPath,
  };
  const directoryStat = lstatSync(root), markerStat = lstatSync(markerPath);
  claimedOutputIdentities.set(claim, { root, markerPath, device: directoryStat.dev, inode: directoryStat.ino, markerDevice: markerStat.dev, markerInode: markerStat.ino, token });
  return claim;
}

export function assertOutputOwnership(claim: OutputClaim): void {
  const original = claimedOutputIdentities.get(claim);
  if (!original || original.root !== claim.root || original.markerPath !== claim.markerPath || original.token !== claim.owner.token) throw new Error("output claim identity changed");
  const directoryStat = lstatSync(claim.root);
  const markerStat = lstatSync(claim.markerPath);
  if (!directoryStat.isDirectory() || directoryStat.isSymbolicLink() || directoryStat.dev !== original.device || directoryStat.ino !== original.inode
    || markerStat.dev !== original.markerDevice || markerStat.ino !== original.markerInode || !markerStat.isFile() || markerStat.isSymbolicLink() || markerStat.nlink !== 1) {
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
  assertAuxiliaryOwnership(claim.owner, path, realpathSync.native(path), realpathSync.native(dirname(path)));
}

function assertAuxiliaryOwnership(owner: ProbeOutputOwner, path: string, canonicalRoot: string, canonicalParent: string): void {
  const markerPath = join(path, OUTPUT_OWNER_FILE);
  const stat = lstatSync(markerPath);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.nlink !== 1) {
    throw new Error(`invalid auxiliary ownership marker: ${markerPath}`);
  }
  const disk = readOwnedJson(markerPath);
  if (
    disk.schemaVersion !== 1
    || disk.owner !== OWNER
    || disk.probeId !== owner.probeId
    || disk.runId !== owner.runId
    || disk.token !== owner.token
    || disk.createdAt !== owner.createdAt
    || disk.canonicalRoot !== canonicalRoot
    || disk.markerKind !== "auxiliary-staging"
    || disk.canonicalParent !== canonicalParent
  ) {
    throw new Error(`auxiliary ownership marker/token mismatch: ${markerPath}`);
  }
  const directory = lstatSync(path);
  if (!directory.isDirectory() || directory.isSymbolicLink()) {
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
          if (matches.length === 0) { missing.push(expected.requestId); continue; }
          if (matches.length > 1) { duplicates.push(expected.requestId); continue; }
          const envelope = matches[0]!;
          const nested = isObjectRecord(envelope.response) ? envelope.response : null;
          const response = expected.requireNestedResponse ? nested : envelope;
          if (responseType(envelope) !== expected.expectedType || !response
            || response.requestId !== expected.requestId || responseType(response) !== expected.expectedType
            || envelope.protocolVersion !== 2 || response.protocolVersion !== 2
            || (expected.expectedType === "simulateGpuiEventResult"
              && (response.dispatchCompleted !== true || response.dispatchScheduled === true))) {
            unexpectedType.push(`${expected.requestId}:terminal-protocol-correlation-mismatch`);
          } else matchedExactlyOnce += 1;
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

const managedTasks = new WeakMap<ManagedTask, { claim: OutputClaim; record: TaskRecord; device: number; inode: number }>();
const taskIndexName = ".test-output/managed-task-index.json";

export function emptyOwnedCleanup(): OwnedCleanup {
  return { resourcesAcquired: false, processExited: true, processGroupExited: true,
    streamsDrained: true, logWriterClosed: true, ownedWindowsClosed: null,
    referencesFinalized: true, closed: true, survivors: [], failureCodes: [] };
}

export function canonicalJson(value: unknown): string {
  const normalize = (entry: unknown): unknown => Array.isArray(entry) ? entry.map(normalize)
    : entry && typeof entry === "object" ? Object.fromEntries(Object.entries(entry).sort(([a], [b]) => a.localeCompare(b)).map(([key, val]) => [key, normalize(val)])) : entry;
  return JSON.stringify(normalize(value));
}

const managedJsonMaxBytes = 8 * 1024 * 1024;

function boundedManagedJson(value: unknown, error = "managed_record_too_large"): string {
  const contents = `${canonicalJson(value)}\n`;
  if (Buffer.byteLength(contents, "utf8") > managedJsonMaxBytes) throw new Error(error);
  return contents;
}

export function readOwnedJson(path: string): Record<string, any> {
  assertNoSymlinkComponents(sep, resolve(path));
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile() || stat.size > managedJsonMaxBytes) throw new Error("invalid_managed_record_file");
    const value = JSON.parse(readFileSync(fd, "utf8"));
    if (!isObjectRecord(value)) throw new Error("invalid_managed_record_object");
    return value;
  } finally { closeSync(fd); }
}

export function atomicManagedJson(path: string, value: unknown): void {
  const contents = boundedManagedJson(value);
  assertNoSymlinkComponents(sep, resolve(path));
  const temporary = `${path}.tmp-${randomUUID()}`;
  const fd = openSync(temporary, constants.O_CREAT | constants.O_EXCL | constants.O_WRONLY | constants.O_NOFOLLOW, 0o600);
  try { writeFileSync(fd, contents); fsyncSync(fd); } finally { closeSync(fd); }
  renameSync(temporary, path);
}

export function cacheLease(operation: string, path: string, args: readonly string[] = []): Record<string, any> {
  const outcome = spawnSync("bash", [resolve(import.meta.dir, "cargo-cache-locks.sh"), operation, path, ...args], {
    encoding: "utf8", timeout: operation === "acquire" ? Number(args[2] ?? 5000) + 2000 : 10_000,
    maxBuffer: 2 * 1024 * 1024,
  });
  if (outcome.status !== 0) throw new Error(outcome.stderr.trim() || `cache_lease_${operation}_failed`);
  return JSON.parse(outcome.stdout);
}

const metadataLeases = new Map<string, Record<string, any>>();

export function withManagedMetadata<T>(repositoryRoot: string, operation: () => T): T {
  const lock = join(realpathSync(repositoryRoot), "target-agent/.locks/metadata.lock");
  const held = metadataLeases.get(lock);
  if (held) {
    const diagnosis = cacheLease("diagnose", lock), lease = diagnosis.lease;
    if (diagnosis.state !== "protected" || diagnosis.reasonCode || canonicalJson(lease) !== canonicalJson(held)
      || lease?.pid !== process.pid || diagnosis.observations?.length !== 1
      || diagnosis.observations[0]?.pid !== process.pid || diagnosis.observations[0]?.expected !== held.processStartTime
      || diagnosis.observations[0]?.observed !== held.processStartTime) throw new Error("metadata_lease_changed");
    return operation();
  }
  const generation = randomUUID();
  const lease = cacheLease("acquire", lock, [String(process.pid), generation, "5000"]);
  metadataLeases.set(lock, lease);
  try { return operation(); }
  finally { metadataLeases.delete(lock); cacheLease("release", lock, [String(process.pid), generation]); }
}

function taskIndex(repositoryRoot: string): Record<string, string> {
  const path = join(repositoryRoot, taskIndexName);
  return existsSync(path) ? readOwnedJson(path) as Record<string, string> : {};
}

export function beginManagedTask(claim: OutputClaim, kind: TaskIdentity["kind"], artifactReferences: readonly ArtifactReference[]): ManagedTask {
  return withManagedMetadata(claim.plan.repoRoot, () => {
    assertOutputOwnership(claim);
    const index = taskIndex(claim.plan.repoRoot);
    if (index[claim.owner.runId]) throw new Error("managed_task_identity_exists");
    const refs = artifactReferences.map(reference => validateManagedReference(claim.plan.repoRoot, reference));
    const record: TaskRecord = {
      identity: { kind, id: claim.owner.runId, generation: randomUUID(), revision: 1 },
      state: "queued", artifactReferences: refs, ownedProcesses: [], result: {},
      cleanup: { ...emptyOwnedCleanup(), referencesFinalized: false, closed: false },
    };
    const recordPath = join(claim.root, "task.json");
    if (existsSync(recordPath)) throw new Error("managed_task_record_exists");
    atomicManagedJson(recordPath, record);
    index[record.identity.id] = recordPath;
    mkdirSync(join(claim.plan.repoRoot, ".test-output"), { recursive: true });
    atomicManagedJson(join(claim.plan.repoRoot, taskIndexName), index);
    const task: ManagedTask = Object.freeze({ get identity() { return Object.freeze({ ...managedTasks.get(task)!.record.identity }); }, recordPath });
    const stat = lstatSync(claim.root);
    managedTasks.set(task, { claim, record, device: stat.dev, inode: stat.ino });
    return task;
  });
}

export function readManagedTask(recordPath: string, expected: Pick<TaskIdentity, "id" | "generation">): TaskRecord {
  const value = readOwnedJson(recordPath);
  const owner = readOwnedJson(join(dirname(recordPath), OUTPUT_OWNER_FILE));
  if (owner.owner !== OWNER || owner.schemaVersion !== 1 || typeof owner.token !== "string" || !owner.token
    || owner.runId !== expected.id || owner.canonicalRoot !== realpathSync(dirname(recordPath))
    || value.identity?.id !== expected.id || value.identity?.generation !== expected.generation
    || !["build-job", "runtime-run", "evidence-run"].includes(value.identity?.kind)
    || !Number.isSafeInteger(value.identity?.revision) || value.identity.revision < 1
    || !["queued", "running", "finalizing", "closed", "protected"].includes(value.state)
    || !Array.isArray(value.artifactReferences) || !Array.isArray(value.ownedProcesses)
    || !isObjectRecord(value.result) || !isObjectRecord(value.cleanup)
    || !Array.isArray(value.cleanup.survivors) || !Array.isArray(value.cleanup.failureCodes)) throw new Error("managed_task_identity_invalid");
  if (value.artifactReferences.some(reference => !isManagedReference(reference))) throw new Error("managed_task_references_invalid");
  if (value.publicationIntents !== undefined && (!Array.isArray(value.publicationIntents)
    || value.publicationIntents.some(intent => !validPublicationIntent(intent))
    || new Set(value.publicationIntents.map(intent => intent.id)).size !== value.publicationIntents.length)) throw new Error("managed_publication_intents_invalid");
  return value as unknown as TaskRecord;
}

export function managedTaskRecordPath(repositoryRoot: string, identity: Pick<TaskIdentity, "id" | "generation">): string {
  const path = taskIndex(repositoryRoot)[identity.id];
  if (typeof path !== "string") throw new Error("managed_task_missing");
  readManagedTask(path, identity);
  return path;
}

function mutateManagedTask(task: ManagedTask, change: (record: TaskRecord) => TaskRecord): TaskRecord {
  const owned = managedTasks.get(task);
  if (!owned) throw new Error("managed_task_not_owned");
  return withManagedMetadata(owned.claim.plan.repoRoot, () => {
    assertOutputOwnership(owned.claim);
    const stat = lstatSync(owned.claim.root);
    if (stat.dev !== owned.device || stat.ino !== owned.inode) throw new Error("managed_task_directory_changed");
    const previous = readManagedTask(task.recordPath, owned.record.identity);
    if (canonicalJson(previous) !== canonicalJson(owned.record) || ["closed", "protected"].includes(previous.state)) throw new Error("managed_task_revision_changed_or_terminal");
    const next = change(structuredClone(previous));
    next.identity = { ...previous.identity, revision: previous.identity.revision + 1 };
    atomicManagedJson(task.recordPath, next);
    owned.record = next;
    return structuredClone(next);
  });
}

export function updateManagedTask(task: ManagedTask, patch: Partial<Pick<TaskRecord, "state" | "source" | "effectiveConfiguration" | "result" | "ownedProcesses">>): TaskRecord {
  if (patch.state === "closed" || patch.state === "protected") throw new Error("managed_task_requires_finalization");
  return mutateManagedTask(task, record => Object.assign(record, structuredClone(patch)));
}

function completeManagedCleanup(record: TaskRecord): boolean {
  const cleanup = record.cleanup;
  return record.state === "closed" && cleanup.closed === true && cleanup.referencesFinalized === true && cleanup.processExited === true
    && cleanup.processGroupExited === true && cleanup.streamsDrained === true && cleanup.logWriterClosed === true
    && (cleanup.ownedWindowsClosed === null || cleanup.ownedWindowsClosed === true) && cleanup.survivors.length === 0;
}

function isManagedReference(value: unknown): value is ArtifactReference {
  return isObjectRecord(value) && Object.keys(value).sort().join(",") === "manifestPath,manifestSha256"
    && typeof value.manifestPath === "string" && /^target-agent\/artifacts\/[a-zA-Z0-9][a-zA-Z0-9._-]*\/manifest\.json$/.test(value.manifestPath)
    && typeof value.manifestSha256 === "string" && /^[a-f0-9]{64}$/.test(value.manifestSha256);
}

function validateManagedReference(repositoryRoot: string, reference: ArtifactReference): ArtifactReference {
  if (!isManagedReference(reference)) throw new Error("invalid_artifact_reference");
  const path = join(repositoryRoot, reference.manifestPath);
  const manifest = readOwnedJson(path);
  if (sha256File(path) !== reference.manifestSha256) throw new Error("artifact_reference_changed");
  const directory = lstatSync(dirname(path)), file = lstatSync(path);
  if (!directory.isDirectory() || directory.isSymbolicLink() || file.nlink !== 1
    || (typeof process.getuid === "function" && (directory.uid !== process.getuid() || file.uid !== process.getuid()))) throw new Error("artifact_publication_invalid");
  const publication = manifest.publication;
  if (manifest.schemaVersion !== 3 || manifest.artifactId !== basename(dirname(path))
    || publication?.owner !== "scripts/agentic/agent-cargo.sh" || publication.pool !== "agent-debug"
    || publication.immutable !== true || publication.exportedWhileLeaseHeld !== true || !publication.leaseGeneration
    || publication.buildTask?.kind !== "build-job") throw new Error("artifact_publication_invalid");
  const task = readManagedTask(managedTaskRecordPath(repositoryRoot, publication.buildTask), publication.buildTask);
  if (!completeManagedCleanup(task) || task.result.status !== "succeeded" || task.identity.revision <= publication.buildTask.revision
    || !Array.isArray(task.result.artifacts) || !task.result.artifacts.some(ref => canonicalJson(ref) === canonicalJson(reference))) throw new Error("artifact_not_finalized");
  return { manifestPath: reference.manifestPath, manifestSha256: reference.manifestSha256 };
}

const keepSetName = ".test-output/managed-artifact-keep.json";

export function managedKeepSet(repositoryRoot: string): ManagedKeepSet {
  const path = join(repositoryRoot, keepSetName);
  if (!existsSync(path)) return { schemaVersion: 1, revision: retentionRevision([]), references: [] };
  const value = readOwnedJson(path);
  if (value.schemaVersion !== 1 || !Array.isArray(value.references) || value.revision !== retentionRevision(value.references)) throw new Error("managed_keep_set_invalid");
  for (const reference of value.references) {
    if (!isManagedReference(reference)) throw new Error("managed_keep_set_invalid");
  }
  return value as unknown as ManagedKeepSet;
}

export function updateManagedKeepSet(repositoryRoot: string, expectedRevision: string, references: readonly ArtifactReference[]): ManagedKeepSet {
  return withManagedMetadata(repositoryRoot, () => {
    const previous = managedKeepSet(repositoryRoot);
    if (previous.revision !== expectedRevision) throw new Error("managed_keep_set_changed");
    if (!Array.isArray(references)) throw new Error("invalid_artifact_references");
    const exact = references.map(reference => validateManagedReference(repositoryRoot, reference))
      .sort((a, b) => a.manifestPath.localeCompare(b.manifestPath) || a.manifestSha256.localeCompare(b.manifestSha256));
    if (new Set(exact.map(reference => reference.manifestPath)).size !== exact.length) throw new Error("duplicate_keep_reference");
    const next: ManagedKeepSet = { schemaVersion: 1, revision: retentionRevision(exact), references: exact };
    mkdirSync(dirname(join(repositoryRoot, keepSetName)), { recursive: true });
    durableRetentionJson(join(repositoryRoot, keepSetName), next);
    return next;
  });
}

export function registerManagedArtifactReference(task: ManagedTask, reference: ArtifactReference): TaskRecord {
  const owned = managedTasks.get(task);
  if (!owned) throw new Error("managed_task_not_owned");
  return mutateManagedTask(task, record => {
    const exact = validateManagedReference(owned.claim.plan.repoRoot, reference);
    if (!record.artifactReferences.some(ref => canonicalJson(ref) === canonicalJson(exact))) record.artifactReferences.push(exact);
    return record;
  });
}

function validPublicationIntent(value: unknown): value is ManagedPublicationIntent {
  if (!isObjectRecord(value)) return false;
  return typeof value.id === "string" && /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(value.id)
    && typeof value.generation === "string" && /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(value.generation)
    && value.pendingPath === `target-agent/artifacts/.pending-${value.id}`
    && value.destinationPath === `target-agent/artifacts/${value.id}`
    && typeof value.directoryDevice === "number" && Number.isSafeInteger(value.directoryDevice) && value.directoryDevice >= 0
    && typeof value.directoryInode === "number" && Number.isSafeInteger(value.directoryInode) && value.directoryInode > 0
    && typeof value.phase === "string" && ["pending", "published", "failed"].includes(value.phase);
}

export function registerManagedPublicationIntent(task: ManagedTask, intent: Omit<ManagedPublicationIntent, "phase">): TaskRecord {
  const owned = managedTasks.get(task);
  if (!owned) throw new Error("managed_task_not_owned");
  return mutateManagedTask(task, record => {
    const entry: ManagedPublicationIntent = { ...intent, phase: "pending" };
    if (record.identity.kind !== "build-job" || !validPublicationIntent(entry)
      || record.publicationIntents?.some(previous => previous.id === entry.id)) throw new Error("managed_publication_intent_invalid");
    assertRetentionDirectory(join(owned.claim.plan.repoRoot, entry.pendingPath), entry);
    if (existsSync(join(owned.claim.plan.repoRoot, entry.destinationPath))) throw new Error("managed_publication_destination_exists");
    if (readdirSync(join(owned.claim.plan.repoRoot, entry.pendingPath)).length !== 0) throw new Error("managed_publication_staging_not_empty");
    if (listManagedTasks(owned.claim.plan.repoRoot).some(existing => existing.record?.publicationIntents?.some(previous =>
      previous.pendingPath === entry.pendingPath || previous.destinationPath === entry.destinationPath
      || (previous.directoryDevice === entry.directoryDevice && previous.directoryInode === entry.directoryInode)))) throw new Error("managed_publication_already_owned");
    record.publicationIntents = [...(record.publicationIntents ?? []), entry];
    return record;
  });
}

export function updateManagedPublicationIntent(task: ManagedTask, id: string, phase: "published" | "failed"): TaskRecord {
  const owned = managedTasks.get(task);
  if (!owned) throw new Error("managed_task_not_owned");
  return mutateManagedTask(task, record => {
    const entry = record.publicationIntents?.find(intent => intent.id === id);
    if (!entry || !["published", "failed"].includes(phase) || entry.phase === "failed") throw new Error("managed_publication_intent_invalid");
    const pending = join(owned.claim.plan.repoRoot, entry.pendingPath), destination = join(owned.claim.plan.repoRoot, entry.destinationPath);
    const paths = [pending, destination].filter(path => existsSync(path));
    if (paths.length !== 1 || (phase === "published" && paths[0] !== destination)) throw new Error("managed_publication_directory_invalid");
    assertRetentionDirectory(paths[0]!, entry);
    entry.phase = phase;
    return record;
  });
}

export function finalizeManagedTask(task: ManagedTask, cleanup: OwnedCleanup): TaskRecord {
  return mutateManagedTask(task, record => {
    const complete = cleanup.closed && cleanup.processExited && cleanup.processGroupExited
      && cleanup.streamsDrained && cleanup.logWriterClosed && cleanup.referencesFinalized
      && cleanup.ownedWindowsClosed !== false && cleanup.survivors.length === 0;
    record.cleanup = { ...structuredClone(cleanup), closed: complete };
    record.state = complete ? "closed" : "protected";
    return record;
  });
}

function listManagedTaskSubtree(repositoryRoot: string, subtree?: string): Array<{ recordPath: string; record?: TaskRecord; reason?: string }> {
  return Object.entries(taskIndex(repositoryRoot)).filter(([, path]) => !subtree || isStrictDescendant(subtree, path)).map(([id, recordPath]) => {
    try {
      const value = readOwnedJson(recordPath);
      return { recordPath, record: readManagedTask(recordPath, { id, generation: value.identity?.generation }) };
    } catch (error) { return { recordPath, reason: String(error) }; }
  });
}

export function listManagedTasks(repositoryRoot: string): Array<{ recordPath: string; record?: TaskRecord; reason?: string }> {
  return listManagedTaskSubtree(repositoryRoot);
}

interface ManagedRetentionRecord extends RetentionCandidate {
  path: string;
  owner?: ProbeOutputOwner;
  ownerSha256?: string;
  ownerIdentity?: RegularFileIdentity;
  buildTaskId?: string;
  recordSnapshot: Record<string, any>;
  publicationIntent?: ManagedPublicationIntent;
  producerRecordPath?: string;
  producerSnapshot?: TaskRecord;
  producerRecordSha256?: Sha256;
  manifestContents?: string;
}

interface ManagedRetentionAuxiliary {
  path: string;
  ownerTaskPath: string;
  directoryDevice: number;
  directoryInode: number;
  markerIdentity: RegularFileIdentity;
  markerSha256: string;
}

interface ManagedRetentionSubtree extends ManagedRetentionRecord {
  coveredRecords: ManagedRetentionRecord[];
  auxiliaries: ManagedRetentionAuxiliary[];
}

interface ManagedRetentionPlan {
  candidates: ManagedRetentionSubtree[];
  protectedRecords: unknown[];
  references: ArtifactReference[];
  policy: string;
  cachesDeletable: false;
  legacyDriverDirectories: string;
  reclaimablePhysicalBytes: null;
  revision: string;
}

interface RetentionNode {
  path: string;
  record?: ManagedRetentionRecord;
  protected: boolean;
  parent?: RetentionNode;
  selected?: ManagedRetentionSubtree;
}

interface ManagedPruneJournal {
  schemaVersion: 2;
  generation: string;
  plan: ManagedRetentionPlan;
  selection: RetentionCandidate[];
  steps: Array<{ quarantine: string; phase: "pending" | "quarantined" | "indexed" | "removed" | "withdrawn" }>;
}

export interface ManagedPruneHooks {
  afterQuarantine?: (quarantine: string) => void;
  afterIndexCommit?: (quarantine: string) => void;
  beforeRemove?: (quarantine: string) => void;
}

const pruneJournalName = ".test-output/managed-retention.json";

function retentionRevision(value: unknown): string {
  return createHash("sha256").update(canonicalJson(value)).digest("hex");
}

function scratchOnlyRetention(candidate: ManagedRetentionSubtree): boolean {
  return candidate.coveredRecords.every(record => (record.kind === "runtime-run" || record.kind === "evidence-run")
    && (record.recordSnapshot.publicationIntents === undefined
      || (Array.isArray(record.recordSnapshot.publicationIntents) && record.recordSnapshot.publicationIntents.length === 0)));
}

function buildManagedRetentionPlan(repositoryRoot: string, taskSubtree?: string): ManagedRetentionPlan {
  const tasks = listManagedTaskSubtree(repositoryRoot, taskSubtree);
  const keep = managedKeepSet(repositoryRoot);
  const references = [...keep.references, ...tasks.flatMap(entry => entry.record && completeManagedCleanup(entry.record) ? [] : entry.record?.artifactReferences ?? [])];
  let unknownManaged = tasks.some(entry => !entry.record);
  for (const reference of keep.references) {
    try { validateManagedReference(repositoryRoot, reference); }
    catch { unknownManaged = true; }
  }
  const nodes = new Map<string, RetentionNode>();
  const auxiliaries = new Map<string, ManagedRetentionAuxiliary>();
  const protectedRecords: unknown[] = [];
  const addNode = (path: string, record?: ManagedRetentionRecord): void => {
    path = resolve(path);
    const previous = nodes.get(path);
    nodes.set(path, { path, record: record ?? previous?.record, protected: !record || previous?.protected === true });
  };
  // The index and recovery journal must survive even if a managed /tmp root contains the repository.
  addNode(dirname(join(repositoryRoot, taskIndexName)));
  const artifactsRoot = join(repositoryRoot, "target-agent/artifacts");
  const artifactIds = !taskSubtree && existsSync(artifactsRoot) ? readdirSync(artifactsRoot).sort() : [];
  for (const id of artifactIds) {
    try {
      const manifest = readOwnedJson(join(artifactsRoot, id, "manifest.json"));
      if (manifest.schemaVersion === 3 && manifest.derivation?.input?.manifestPath && manifest.derivation?.input?.manifestSha256) references.push(manifest.derivation.input);
    } catch { /* Unreadable managed artifacts remain protected below. */ }
  }
  const retainedBuildTasks = new Set<string>();
  const failedIntents = tasks.flatMap(entry => (entry.record?.publicationIntents ?? [])
    .filter(intent => intent.phase === "failed").map(intent => ({ ...entry, intent })));
  for (const id of artifactIds) {
    const path = join(artifactsRoot, id);
    const claimedProducers = tasks.filter(entry => entry.record?.identity.kind === "build-job"
      && (entry.record.publicationIntents?.some(intent => [intent.pendingPath, intent.destinationPath].includes(relative(repositoryRoot, path)))
        || (Array.isArray(entry.record.result.artifacts) && entry.record.result.artifacts.some(ref => isManagedReference(ref)
          && ref.manifestPath === `${relative(repositoryRoot, path)}/manifest.json`))));
    let buildId: string | undefined;
    try {
      const stat = lstatSync(path);
      if (!stat.isDirectory() || stat.isSymbolicLink()) throw new Error("unmanaged_artifact");
      const failed = failedIntents.filter(entry => [entry.intent.pendingPath, entry.intent.destinationPath].includes(relative(repositoryRoot, path)));
      if (failed.length) {
        if (failed.length !== 1) throw new Error("ambiguous_publication_intent");
        const { intent, record: producer, recordPath } = failed[0]!;
        buildId = producer!.identity.id;
        assertRetentionDirectory(path, intent);
        const alternate = join(repositoryRoot, relative(repositoryRoot, path) === intent.pendingPath ? intent.destinationPath : intent.pendingPath);
        if (!completeManagedCleanup(producer!) || existsSync(alternate) || unknownManaged
          || references.some(ref => [intent.pendingPath, intent.destinationPath].some(directory => ref.manifestPath === `${directory}/manifest.json`))) throw new Error("failed_publication_protected");
        addNode(path, { kind: "artifact", id: intent.id, generation: intent.generation, revision: producer!.identity.revision,
          recordSha256: sha256File(recordPath), directoryDevice: stat.dev, directoryInode: stat.ino, path,
          buildTaskId: buildId, recordSnapshot: producer!, publicationIntent: intent, producerRecordPath: recordPath });
        continue;
      }
      const manifest = readOwnedJson(join(path, "manifest.json"));
      buildId = manifest.publication?.buildTask?.id;
      const digest = sha256File(join(path, "manifest.json"));
      validateManagedReference(repositoryRoot, { manifestPath: relative(repositoryRoot, join(path, "manifest.json")), manifestSha256: digest });
      const producerRecordPath = managedTaskRecordPath(repositoryRoot, manifest.publication.buildTask);
      const task = readManagedTask(producerRecordPath, manifest.publication.buildTask);
      const manifestContents = readFileSync(join(path, "manifest.json"), "utf8");
      if (createHash("sha256").update(manifestContents).digest("hex") !== digest) throw new Error("artifact_reference_changed");
      if (unknownManaged || references.some(ref => ref.manifestPath === relative(repositoryRoot, join(path, "manifest.json")))) {
        retainedBuildTasks.add(task.identity.id);
        protectedRecords.push({ kind: "artifact", id, reason: "active_or_unknown_managed_reference" });
        addNode(path);
      } else addNode(path, { kind: "artifact", id, generation: digest, revision: 1, recordSha256: digest,
        directoryDevice: stat.dev, directoryInode: stat.ino, path, buildTaskId: task.identity.id, recordSnapshot: manifest,
        producerSnapshot: task, producerRecordPath, producerRecordSha256: sha256File(producerRecordPath), manifestContents });
    } catch (error) {
      for (const producer of claimedProducers) retainedBuildTasks.add(producer.record!.identity.id);
      if (buildId) retainedBuildTasks.add(buildId);
      protectedRecords.push({ kind: "artifact", id, reason: String(error) });
      addNode(path);
    }
  }
  for (const entry of tasks) {
    const path = dirname(entry.recordPath);
    const record = entry.record;
    if (!record || !completeManagedCleanup(record) || retainedBuildTasks.has(record.identity.id)) {
      protectedRecords.push({ recordPath: entry.recordPath, reason: entry.reason ?? "active_or_required_task" });
      addNode(path);
      continue;
    }
    try {
      const stat = lstatSync(path);
      const ownerPath = join(path, OUTPUT_OWNER_FILE);
      const owner = readOwnedJson(ownerPath);
      if (!stat.isDirectory() || stat.isSymbolicLink() || owner.schemaVersion !== 1 || owner.owner !== OWNER
        || typeof owner.token !== "string" || !owner.token || owner.runId !== record.identity.id || owner.canonicalRoot !== path
        || typeof owner.probeId !== "string" || !owner.probeId || typeof owner.createdAt !== "string" || !owner.createdAt
        || owner.markerKind !== undefined || owner.canonicalParent !== undefined || lstatSync(ownerPath).nlink !== 1
        || !allowedAnchors(repositoryRoot).some(anchor => isStrictDescendant(anchor.canonical, path))) throw new Error("unmanaged_task_directory");
      addNode(path, { ...record.identity, recordSha256: sha256File(entry.recordPath), directoryDevice: stat.dev,
        directoryInode: stat.ino, path, owner: owner as unknown as ProbeOutputOwner, ownerSha256: sha256File(ownerPath),
        ownerIdentity: bindRegularFileIdentity(ownerPath, "retention owner"), recordSnapshot: record });
    } catch (error) {
      protectedRecords.push({ recordPath: entry.recordPath, reason: String(error) });
      addNode(path);
    }
  }
  // Output ownership precedes task registration; an interrupted registration is still protected.
  const registeredPaths = new Set(tasks.map(entry => resolve(dirname(entry.recordPath))));
  // A blocked ancestor may stop traversal before a registered child is reached.
  // Only skip records that were actually visited, not every descendant of a scan root.
  const visitedRecords = new Set<string>();
  for (const node of [...nodes.values()].filter(node => node.record).sort((a, b) => a.path.length - b.path.length)) {
    if (visitedRecords.has(node.path)) continue;
    let scanned = 0;
    const device = lstatSync(node.path).dev;
    const scan = (path: string, owningTask?: ManagedRetentionRecord): void => {
      if (nodes.get(path)?.record) visitedRecords.add(path);
      try {
        if (++scanned > 250_000 || lstatSync(path).dev !== device) throw new Error("retention_scan_incomplete");
        if (registeredPaths.has(path)) owningTask = nodes.get(path)?.record;
        const entries = readdirSync(path, { withFileTypes: true });
        if (entries.some(entry => entry.name === OUTPUT_OWNER_FILE) && !registeredPaths.has(path)) {
          const parent = dirname(path);
          if (!owningTask?.owner || (parent !== owningTask.path && auxiliaries.get(parent)?.ownerTaskPath !== owningTask.path)) {
            throw new Error("unregistered_managed_output");
          }
          assertAuxiliaryOwnership(owningTask.owner, path, path, parent);
          const stat = lstatSync(path), markerPath = join(path, OUTPUT_OWNER_FILE);
          auxiliaries.set(path, { path, ownerTaskPath: owningTask.path, directoryDevice: stat.dev, directoryInode: stat.ino,
            markerIdentity: bindRegularFileIdentity(markerPath, "retention auxiliary owner"), markerSha256: sha256File(markerPath) });
        }
        for (const entry of entries) {
          if (entry.isSymbolicLink()) throw new Error("retention_symlink");
          if (entry.isDirectory()) scan(join(path, entry.name), owningTask);
        }
      } catch (error) {
        protectedRecords.push({ recordPath: path, reason: `unknown_managed_descendants: ${String(error)}` });
        addNode(path);
      }
    };
    scan(node.path);
  }
  // Ancestors precede descendants even when sibling names share a prefix (a-parent-extra).
  const ordered = [...nodes.values()].sort((a, b) => a.path.length - b.path.length || a.path.localeCompare(b.path));
  for (const node of ordered) {
    let parent = dirname(node.path);
    while (parent !== dirname(parent) && !nodes.has(parent)) parent = dirname(parent);
    node.parent = nodes.get(parent);
  }
  for (let index = ordered.length - 1; index >= 0; index--) {
    const node = ordered[index]!;
    if (node.protected && node.parent) node.parent.protected = true;
  }
  // Preserve inventory order while joining each record only to its actual ancestors.
  // Pairwise path filtering makes large scratch inventories quadratic per revalidation.
  const coveredByPath = new Map<string, ManagedRetentionRecord[]>();
  const auxiliariesByPath = new Map<string, ManagedRetentionAuxiliary[]>();
  for (const node of ordered) {
    if (!node.record || node.protected) continue;
    for (let ancestor: RetentionNode | undefined = node; ancestor; ancestor = ancestor.parent) {
      if (!ancestor.record || ancestor.protected) continue;
      const records = coveredByPath.get(ancestor.path) ?? [];
      records.push(node.record);
      coveredByPath.set(ancestor.path, records);
    }
  }
  for (const auxiliary of auxiliaries.values()) {
    let parent = dirname(auxiliary.path);
    while (parent !== dirname(parent) && !nodes.has(parent)) parent = dirname(parent);
    for (let ancestor = nodes.get(parent); ancestor; ancestor = ancestor.parent) {
      if (!ancestor.record || ancestor.protected) continue;
      const entries = auxiliariesByPath.get(ancestor.path) ?? [];
      entries.push(auxiliary);
      auxiliariesByPath.set(ancestor.path, entries);
    }
  }
  const candidates: ManagedRetentionSubtree[] = [];
  for (const node of ordered) {
    if (!node.record) continue;
    if (node.protected) {
      protectedRecords.push({ recordPath: node.path, reason: "protected_managed_descendant" });
      continue;
    }
    candidates.push({ ...node.record, coveredRecords: coveredByPath.get(node.path)!,
      auxiliaries: auxiliariesByPath.get(node.path) ?? [] });
  }
  // An artifact must disappear before its publishing task, including when either is nested.
  const taskRoots = new Map<string, ManagedRetentionSubtree[]>();
  for (const candidate of candidates) {
    for (const record of candidate.coveredRecords) if (record.kind !== "artifact") {
      const roots = taskRoots.get(record.id) ?? [];
      roots.push(candidate);
      taskRoots.set(record.id, roots);
    }
  }
  const dependents = new Map(candidates.map(candidate => [candidate, new Set<ManagedRetentionSubtree>()]));
  const dependencies = new Map(candidates.map(candidate => [candidate, 0]));
  for (const candidate of candidates) {
    for (const record of candidate.coveredRecords) {
      for (const taskRoot of record.buildTaskId ? taskRoots.get(record.buildTaskId) ?? [] : []) {
        if (taskRoot !== candidate && !isStrictDescendant(candidate.path, taskRoot.path) && !dependents.get(candidate)!.has(taskRoot)) {
          dependents.get(candidate)!.add(taskRoot);
          dependencies.set(taskRoot, dependencies.get(taskRoot)! + 1);
        }
      }
    }
  }
  const orderedCandidates = candidates.filter(candidate => dependencies.get(candidate) === 0);
  for (let index = 0; index < orderedCandidates.length; index++) {
    for (const dependent of dependents.get(orderedCandidates[index]!)!) {
      const remaining = dependencies.get(dependent)! - 1;
      dependencies.set(dependent, remaining);
      if (remaining === 0) orderedCandidates.push(dependent);
    }
  }
  for (const candidate of candidates) {
    if (dependencies.get(candidate)! > 0) protectedRecords.push({ recordPath: candidate.path, reason: "managed_dependency_cycle" });
  }
  const body = { candidates: orderedCandidates, protectedRecords, references, keepRevision: keep.revision, policy: "exact-selected-managed-subtrees-v3", cachesDeletable: false as const,
    legacyDriverDirectories: "No implicit references inferred from absent binary records", reclaimablePhysicalBytes: null };
  return { ...body, revision: retentionRevision(body) };
}

function retentionIdentity(candidate: RetentionCandidate): RetentionCandidate {
  const { kind, id, generation, revision, recordSha256, directoryDevice, directoryInode } = candidate;
  return { kind, id, generation, revision, recordSha256, directoryDevice, directoryInode };
}

function isRetentionCandidate(value: unknown): value is RetentionCandidate {
  return isObjectRecord(value) && RETENTION_CANDIDATE_KINDS.some(kind => kind === value.kind)
    && typeof value.id === "string" && value.id.length > 0
    && typeof value.generation === "string" && value.generation.length > 0
    && typeof value.revision === "number" && Number.isSafeInteger(value.revision) && value.revision >= 1
    && typeof value.recordSha256 === "string" && /^[a-f0-9]{64}$/.test(value.recordSha256)
    && typeof value.directoryDevice === "number" && Number.isSafeInteger(value.directoryDevice) && value.directoryDevice >= 0
    && typeof value.directoryInode === "number" && Number.isSafeInteger(value.directoryInode) && value.directoryInode > 0;
}

function normalizeRetentionSelection(selection: readonly RetentionCandidate[]): RetentionCandidate[] {
  if (!Array.isArray(selection)) throw new Error("retention_selection_required");
  const result = selection.map(candidate => {
    if (!isRetentionCandidate(candidate)) throw new Error("retention_selection_invalid");
    return retentionIdentity(candidate);
  }).sort((a, b) => canonicalJson(a).localeCompare(canonicalJson(b)));
  if (new Set(result.map(candidate => `${candidate.kind}:${candidate.id}`)).size !== result.length) throw new Error("retention_selection_duplicate");
  return result;
}

function syncRetentionDirectory(path: string): void {
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  try { fsyncSync(fd); } finally { closeSync(fd); }
}

function durableRetentionJson(path: string, value: unknown): void {
  atomicManagedJson(path, value);
  syncRetentionDirectory(dirname(path));
}

function retirementReceipt(journal: ManagedPruneJournal, expectedRevision: string, recovering: boolean) {
  const outcome = (phase: "removed" | "withdrawn") => journal.plan.candidates.flatMap((candidate, index) => journal.steps[index]!.phase === phase
    ? candidate.coveredRecords.map(({ kind, id, generation }) => ({ kind, id, generation })) : []);
  return { schemaVersion: 1, generation: journal.generation, expectedRevision, selection: journal.selection,
    candidates: journal.plan.candidates, removed: outcome("removed"), withdrawn: outcome("withdrawn"), replanRequired: recovering,
    physicalBytesReclaimed: null };
}

function preflightRetirement(journal: ManagedPruneJournal, expectedRevision: string, recovering: boolean): void {
  // Reserve the longest reachable journal phase before any rename or index update.
  boundedManagedJson({ ...journal, steps: journal.steps.map(step => ({ ...step,
    phase: step.phase === "pending" ? "quarantined" : step.phase })) }, "retention_journal_too_large");
  // Moving outcomes between the two arrays can only reduce their combined comma count.
  // Fresh retirement must also leave room for a later interrupted recovery receipt.
  const terminal = { ...journal, steps: journal.steps.map(step => ({ ...step, phase: "removed" as const })) };
  boundedManagedJson(retirementReceipt(terminal, expectedRevision, recovering), "retention_history_too_large");
}

function assertRetentionAuxiliary(candidate: ManagedRetentionSubtree, auxiliary: ManagedRetentionAuxiliary, root: string): void {
  const owner = candidate.coveredRecords.find(record => record.path === auxiliary.ownerTaskPath);
  const parent = dirname(auxiliary.path);
  if (!owner?.owner || owner.kind === "artifact" || !isStrictDescendant(owner.path, auxiliary.path)
    || (parent !== owner.path && !candidate.auxiliaries.some(entry => entry.path === parent && entry.ownerTaskPath === owner.path))) {
    throw new Error("retention_auxiliary_owner_changed");
  }
  const path = join(root, relative(candidate.path, auxiliary.path)), markerPath = join(path, OUTPUT_OWNER_FILE);
  assertRetentionDirectory(path, auxiliary);
  assertRegularFileIdentity(markerPath, auxiliary.markerIdentity, "retention auxiliary owner");
  assertAuxiliaryOwnership(owner.owner, path, auxiliary.path, parent);
  if (sha256File(markerPath) !== auxiliary.markerSha256) throw new Error("retention_auxiliary_owner_changed");
}

function assertRetentionIdle(candidate: ManagedRetentionSubtree, path: string): void {
  for (const record of candidate.coveredRecords) {
    const task = (record.kind === "artifact" ? record.producerSnapshot ?? (record.publicationIntent ? record.recordSnapshot : undefined) : record.recordSnapshot) as TaskRecord | undefined;
    if (!task || !completeManagedCleanup(task)) throw new Error("retention_producer_not_closed");
    for (const child of task.ownedProcesses) {
      if (![child.pid, child.supervisorPid, child.processGroupId].every(value => Number.isSafeInteger(value) && value > 0)
        || !child.processStartTime || !child.supervisorStartTime || !child.processInstanceId || !child.sessionGeneration) throw new Error("retention_process_observation_unknown");
      if (processAlive(child.pid) || processAlive(child.supervisorPid) || processAlive(-child.processGroupId)) throw new Error("retention_owned_process_present_or_unknown");
    }
  }
  let count = 0;
  const inspectTree = (entry: string): void => {
    if (++count > 250_000) throw new Error("retention_scan_incomplete");
    const stat = lstatSync(entry);
    if (stat.dev !== candidate.directoryDevice) throw new Error("retention_cross_device");
    if (typeof process.getuid === "function" && stat.uid !== process.getuid()) throw new Error("retention_owner_changed");
    if (stat.isDirectory() && !stat.isSymbolicLink()) {
      const ownerPath = join(entry, OUTPUT_OWNER_FILE);
      if (existsSync(ownerPath)) {
        const original = join(candidate.path, relative(path, entry));
        const auxiliary = candidate.auxiliaries.find(auxiliary => auxiliary.path === original);
        if (auxiliary) assertRetentionAuxiliary(candidate, auxiliary, path);
        else {
          const expected = candidate.coveredRecords.find(record => record.owner && record.path === original);
          if (!expected?.ownerIdentity || !hasRegularFileIdentity(ownerPath, expected.ownerIdentity)
            || sha256File(ownerPath) !== expected.ownerSha256) throw new Error("retention_unselected_descendant");
        }
      }
      for (const child of readdirSync(entry)) inspectTree(join(entry, child));
    } else if (!stat.isFile()) throw new Error("retention_special_file");
  };
  inspectTree(path);
  const handles = spawnSync(process.platform === "darwin" ? "/usr/sbin/lsof" : "lsof", ["-nP", "-F", "p", "+D", path], {
    encoding: "utf8", timeout: 10_000, maxBuffer: 2 * 1024 * 1024,
  });
  if (handles.error || handles.status !== 1 || handles.stdout.trim() || handles.stderr.trim()) throw new Error("retention_open_handles_present_or_unknown");
}

function assertStartedUnreferenced(repositoryRoot: string, journal: ManagedPruneJournal, candidate: ManagedRetentionSubtree, quarantine: string): void {
  const index = taskIndex(repositoryRoot);
  const coveredTasks = new Set(candidate.coveredRecords.filter(record => record.kind !== "artifact").map(record => record.id));
  for (const [id, recordPath] of Object.entries(index)) {
    if (isStrictDescendant(quarantine, recordPath) && !coveredTasks.has(id)) throw new Error("retention_unselected_descendant");
  }
  const archivedTasks = new Map<string, ManagedRetentionRecord>();
  for (const [position, other] of journal.plan.candidates.entries()) {
    const step = journal.steps[position]!;
    if (step.phase === "withdrawn" || (step.phase === "pending" && !existsSync(step.quarantine))) continue;
    for (const record of other.coveredRecords) if (record.kind !== "artifact") {
      if ((step.phase === "indexed" || step.phase === "removed") && index[record.id] !== undefined) throw new Error("retention_task_index_changed");
      archivedTasks.set(join(record.path, "task.json"), record);
    }
  }
  const references = [...managedKeepSet(repositoryRoot).references];
  // References pin publications, not scratch directories. A non-publishing scratch
  // subtree still gets the full index, owner, descendant, liveness and manifest
  // publisher checks, without reopening every unrelated task twice per deletion.
  const tasks = scratchOnlyRetention(candidate) ? [] : listManagedTasks(repositoryRoot);
  let unknown = false;
  for (const entry of tasks) {
    const archived = archivedTasks.get(entry.recordPath);
    if (archived && !existsSync(entry.recordPath) && index[archived.id] === entry.recordPath) continue;
    if (!entry.record) { unknown = true; continue; }
    if (!completeManagedCleanup(entry.record)) references.push(...entry.record.artifactReferences);
  }
  const taskIds = new Set(candidate.coveredRecords.filter(record => record.kind !== "artifact").map(record => record.id));
  const root = join(repositoryRoot, "target-agent/artifacts");
  for (const id of existsSync(root) ? readdirSync(root) : []) {
    const path = join(root, id);
    if (path === quarantine || isStrictDescendant(quarantine, path)) continue;
    let manifest: Record<string, any>;
    try { manifest = readOwnedJson(join(path, "manifest.json")); }
    catch {
      if (tasks.some(entry => entry.record && taskIds.has(entry.record.identity.id)
        && entry.record.publicationIntents?.some(intent => [intent.pendingPath, intent.destinationPath].includes(relative(repositoryRoot, path))))) throw new Error("retention_publication_still_present");
      continue;
    }
    if (taskIds.has(manifest.publication?.buildTask?.id)) throw new Error("retention_publication_still_present");
    if (manifest.derivation?.input) references.push(manifest.derivation.input);
  }
  if (unknown && candidate.coveredRecords.some(record => record.kind === "artifact")) throw new Error("retention_unknown_reference");
  for (const record of candidate.coveredRecords) if (record.kind === "artifact") {
    const paths = record.publicationIntent ? [record.publicationIntent.pendingPath, record.publicationIntent.destinationPath] : [relative(repositoryRoot, record.path)];
    if (references.some(ref => paths.some(path => ref.manifestPath === `${path}/manifest.json`))) throw new Error("retention_new_reference");
  }
}

function readPruneJournal(repositoryRoot: string): ManagedPruneJournal | undefined {
  const path = join(repositoryRoot, pruneJournalName);
  if (!existsSync(path)) return undefined;
  const raw = readOwnedJson(path);
  if (raw.schemaVersion === 1) throw new Error("retention_legacy_journal_requires_compatible_reader_or_reviewed_recovery");
  const value = raw as unknown as ManagedPruneJournal;
  if (value.schemaVersion !== 2 || !/^[a-f0-9-]{36}$/.test(value.generation) || !isObjectRecord(value.plan)
    || !Array.isArray(value.plan.candidates) || !Array.isArray(value.steps) || !Array.isArray(value.selection)
    || value.steps.length !== value.plan.candidates.length) throw new Error("retention_journal_invalid");
  const { revision, ...body } = value.plan;
  if (retentionRevision(body) !== revision) throw new Error("retention_journal_plan_changed");
  const selection = normalizeRetentionSelection(value.selection);
  const archived: RetentionCandidate[] = [];
  for (const [index, step] of value.steps.entries()) {
    const candidate = value.plan.candidates[index]!;
    if (typeof candidate.path !== "string" || !isAbsolute(candidate.path) || resolve(candidate.path) !== candidate.path
      || step.quarantine !== `${candidate.path}.quarantine-${value.generation}`
      || !["pending", "quarantined", "indexed", "removed", "withdrawn"].includes(step.phase)
      || !Array.isArray(candidate.coveredRecords) || !candidate.coveredRecords.length
      || !Array.isArray(candidate.auxiliaries)
      || candidate.auxiliaries.some(auxiliary => !isObjectRecord(auxiliary) || typeof auxiliary.path !== "string"
        || !isStrictDescendant(candidate.path, auxiliary.path) || resolve(auxiliary.path) !== auxiliary.path
        || !candidate.coveredRecords.some(record => record.kind !== "artifact" && record.path === auxiliary.ownerTaskPath)
        || !isObjectRecord(auxiliary.markerIdentity) || !Number.isSafeInteger(auxiliary.markerIdentity.device)
        || !Number.isSafeInteger(auxiliary.markerIdentity.inode) || auxiliary.markerIdentity.inode <= 0
        || !Number.isSafeInteger(auxiliary.directoryDevice) || !Number.isSafeInteger(auxiliary.directoryInode)
        || auxiliary.directoryInode <= 0 || !/^[a-f0-9]{64}$/.test(auxiliary.markerSha256))
      || candidate.coveredRecords.some(record => !isObjectRecord(record.recordSnapshot)
        || (record.kind !== "artifact" && (!isObjectRecord(record.ownerIdentity) || !Number.isSafeInteger(record.ownerIdentity.device)
          || !Number.isSafeInteger(record.ownerIdentity.inode) || record.ownerIdentity.inode <= 0 || !record.owner
          || record.owner.runId !== record.id || record.owner.canonicalRoot !== record.path || !/^[a-f0-9]{64}$/.test(record.ownerSha256 ?? "")))
        || (record.path !== candidate.path && !isStrictDescendant(candidate.path, record.path)))) throw new Error("retention_journal_invalid");
    for (const record of candidate.coveredRecords) {
      const artifactRoot = join(repositoryRoot, "target-agent/artifacts");
      if (record.kind === "artifact" ? dirname(record.path) !== artifactRoot
        : !allowedAnchors(repositoryRoot).some(anchor => isStrictDescendant(anchor.canonical, record.path))) throw new Error("retention_journal_scope_invalid");
      archived.push(record);
    }
  }
  if (canonicalJson(normalizeRetentionSelection(archived)) !== canonicalJson(selection)) throw new Error("retention_journal_selection_changed");
  return value;
}

export function managedRetentionPlan(repositoryRoot: string): Record<string, any> {
  const journal = readPruneJournal(repositoryRoot);
  if (!journal) return buildManagedRetentionPlan(repositoryRoot);
  const revision = retentionRevision({ journal, current: buildManagedRetentionPlan(repositoryRoot) });
  return { ...journal.plan, candidates: journal.plan.candidates.flatMap(candidate => candidate.coveredRecords),
    selection: journal.selection,
    recovery: { path: join(repositoryRoot, pruneJournalName), expectedRevision: revision, generation: journal.generation, steps: journal.steps }, revision };
}

export function isRetiredManagedArtifact(repositoryRoot: string, reference: ArtifactReference): boolean {
  if (!isManagedReference(reference)) throw new Error("invalid_artifact_reference");
  const root = join(repositoryRoot, ".test-output/managed-retention-receipts");
  if (!existsSync(root)) return false;
  assertNoSymlinkComponents(sep, resolve(root));
  let retired = false;
  for (const name of readdirSync(root).sort()) {
    if (!/^[a-f0-9-]{36}\.json$/.test(name)) throw new Error("retention_history_invalid");
    const path = join(root, name), receipt = readOwnedJson(path), stat = lstatSync(path);
    if (stat.nlink !== 1 || (typeof process.getuid === "function" && stat.uid !== process.getuid())
      || receipt.schemaVersion !== 1 || name !== `${receipt.generation}.json` || typeof receipt.replanRequired !== "boolean"
      || !Array.isArray(receipt.candidates) || !Array.isArray(receipt.removed) || !Array.isArray(receipt.withdrawn)) throw new Error("retention_history_invalid");
    const records: ManagedRetentionRecord[] = [];
    for (const candidate of receipt.candidates) {
      if (!isObjectRecord(candidate) || !Array.isArray(candidate.coveredRecords) || !candidate.coveredRecords.length) throw new Error("retention_history_invalid");
      records.push(...candidate.coveredRecords);
    }
    if (canonicalJson(normalizeRetentionSelection(records)) !== canonicalJson(normalizeRetentionSelection(receipt.selection))) throw new Error("retention_history_selection_changed");
    const outcomes = [...receipt.removed, ...receipt.withdrawn];
    if (outcomes.length !== records.length) throw new Error("retention_history_not_terminal");
    const seen = new Set<string>();
    for (const outcome of outcomes) {
      if (!isObjectRecord(outcome) || Object.keys(outcome).sort().join(",") !== "generation,id,kind") throw new Error("retention_history_invalid");
      const key = canonicalJson(outcome);
      if (seen.has(key) || !records.some(record => record.kind === outcome.kind && record.id === outcome.id && record.generation === outcome.generation)) throw new Error("retention_history_not_terminal");
      seen.add(key);
    }
    for (const record of records) if (record.kind === "artifact" && !record.publicationIntent) {
      if (typeof record.manifestContents !== "string" || createHash("sha256").update(record.manifestContents).digest("hex") !== record.recordSha256
        || record.generation !== record.recordSha256 || record.path !== join(repositoryRoot, "target-agent/artifacts", record.id)) throw new Error("retention_history_manifest_changed");
      const manifest = JSON.parse(record.manifestContents);
      if (manifest.schemaVersion !== 3 || manifest.artifactId !== record.id || canonicalJson(manifest) !== canonicalJson(record.recordSnapshot)) throw new Error("retention_history_manifest_changed");
      if (reference.manifestPath === `target-agent/artifacts/${record.id}/manifest.json` && reference.manifestSha256 === record.recordSha256
        && receipt.removed.some(outcome => outcome.kind === "artifact" && outcome.id === record.id && outcome.generation === record.generation)) retired = true;
    }
  }
  return retired;
}


function assertRetentionDirectory(path: string, expected: Pick<RetentionCandidate, "directoryDevice" | "directoryInode">): void {
  assertNoSymlinkComponents(sep, resolve(path));
  const stat = lstatSync(path);
  if (!stat.isDirectory() || stat.isSymbolicLink() || stat.dev !== expected.directoryDevice || stat.ino !== expected.directoryInode) throw new Error("retention_directory_changed");
  if (typeof process.getuid === "function" && stat.uid !== process.getuid()) throw new Error("retention_owner_changed");
}

function assertQuarantinedRecords(candidate: ManagedRetentionSubtree, quarantine: string, partial = false): void {
  for (const record of candidate.coveredRecords) {
    const path = join(quarantine, relative(candidate.path, record.path));
    if (partial && !existsSync(path)) continue;
    assertRetentionDirectory(path, record);
    const recordPath = record.publicationIntent ? record.producerRecordPath! : join(path, record.kind === "artifact" ? "manifest.json" : "task.json");
    if (record.publicationIntent || !partial || existsSync(recordPath)) {
      readOwnedJson(recordPath);
      if (sha256File(recordPath) !== record.recordSha256) throw new Error("retention_record_changed");
    }
    if (record.producerRecordSha256) {
      readOwnedJson(record.producerRecordPath!);
      if (sha256File(record.producerRecordPath!) !== record.producerRecordSha256) throw new Error("retention_producer_changed");
    }
    if (record.kind !== "artifact") {
      const ownerPath = join(path, OUTPUT_OWNER_FILE);
      if (partial && !existsSync(ownerPath)) continue;
      const owner = readOwnedJson(ownerPath);
      if (!record.ownerIdentity || !hasRegularFileIdentity(ownerPath, record.ownerIdentity) || lstatSync(ownerPath).nlink !== 1
        || owner.canonicalRoot !== record.path || canonicalJson(owner) !== canonicalJson(record.owner)
        || sha256File(ownerPath) !== record.ownerSha256) throw new Error("retention_owner_changed");
    }
  }
  for (const auxiliary of candidate.auxiliaries) {
    const path = join(quarantine, relative(candidate.path, auxiliary.path));
    if (partial && !existsSync(path)) continue;
    assertRetentionDirectory(path, auxiliary);
    if (partial && !existsSync(join(path, OUTPUT_OWNER_FILE))) continue;
    assertRetentionAuxiliary(candidate, auxiliary, quarantine);
  }
}

function removeQuarantinedTree(path: string, device: number): void {
  const writable = (directory: string): void => {
    const current = lstatSync(directory);
    if (current.dev !== device) throw new Error("retention_cross_device");
    if (current.isDirectory() && !current.isSymbolicLink()) {
      chmodSync(directory, 0o700);
      for (const child of readdirSync(directory)) writable(join(directory, child));
    }
  };
  writable(path);
  rmSync(path, { recursive: true });
}

export function pruneManagedRecords(repositoryRoot: string, expectedRevision: string, selection: readonly RetentionCandidate[], hooks: ManagedPruneHooks = {}): Record<string, unknown> {
  const selected = normalizeRetentionSelection(selection);
  if (metadataLeases.has(join(realpathSync(repositoryRoot), "target-agent/.locks/metadata.lock"))) throw new Error("retention_lease_order_invalid");
  // Publishing takes pool before metadata; pruning must use the same order.
  const poolLock = join(repositoryRoot, "target-agent/.locks/pool-agent-debug.lock"), poolGeneration = randomUUID();
  cacheLease("acquire", poolLock, [String(process.pid), poolGeneration, "5000"]);
  try {
    return withManagedMetadata(repositoryRoot, () => {
      const journalPath = join(repositoryRoot, pruneJournalName);
      let journal = readPruneJournal(repositoryRoot);
      const recovering = !!journal;
      if (journal) {
        if (expectedRevision !== managedRetentionPlan(repositoryRoot).revision) throw new Error("retention_plan_changed");
        if (canonicalJson(selected) !== canonicalJson(journal.selection)) throw new Error("retention_selection_changed");
        preflightRetirement(journal, expectedRevision, true);
      } else {
        const plan = buildManagedRetentionPlan(repositoryRoot);
        if (plan.revision !== expectedRevision) throw new Error("retention_plan_changed");
        const candidates = selected.map(identity => {
          const candidate = plan.candidates.find(entry => canonicalJson(retentionIdentity(entry)) === canonicalJson(identity));
          if (!candidate) throw new Error("retention_candidate_not_authorized");
          return candidate;
        });
        const selectedKeys = new Set(selected.map(identity => canonicalJson(identity)));
        for (const candidate of candidates) {
          if (candidate.coveredRecords.some(record => !selectedKeys.has(canonicalJson(retentionIdentity(record))))) throw new Error("retention_unselected_descendant");
          const taskIds = new Set(candidate.coveredRecords.filter(record => record.kind !== "artifact").map(record => record.id));
          if (plan.candidates.some(other => other.buildTaskId && taskIds.has(other.buildTaskId)
            && !selectedKeys.has(canonicalJson(retentionIdentity(other))))) throw new Error("retention_unselected_publication");
        }
        if (!selected.length) return { expectedRevision, removed: [], withdrawn: [], replanRequired: false, physicalBytesReclaimed: null };
        const roots = plan.candidates.filter(candidate => selectedKeys.has(canonicalJson(retentionIdentity(candidate)))
          && !candidates.some(parent => isStrictDescendant(parent.path, candidate.path)));
        const { revision: _revision, ...planBody } = { ...plan, candidates: roots };
        const generation = randomUUID();
        journal = { schemaVersion: 2, generation, plan: { ...planBody, revision: retentionRevision(planBody) }, selection: selected,
          steps: roots.map(candidate => ({ quarantine: `${candidate.path}.quarantine-${generation}`, phase: "pending" })) };
        preflightRetirement(journal, expectedRevision, false);
        mkdirSync(dirname(journalPath), { recursive: true });
        durableRetentionJson(journalPath, journal);
      }
      const blocked: string[] = [];
      for (const [position, candidate] of journal.plan.candidates.entries()) {
        const step = journal.steps[position]!;
        if (step.phase === "removed" || step.phase === "withdrawn") continue;
        if (recovering && step.phase === "pending" && !existsSync(step.quarantine)) {
          // Never extend old authorization to untouched originals, even if still eligible.
          step.phase = "withdrawn";
          durableRetentionJson(journalPath, journal);
          continue;
        }
        try {
          if (step.phase === "pending" && !existsSync(step.quarantine)) {
            const current = buildManagedRetentionPlan(repositoryRoot, scratchOnlyRetention(candidate) ? candidate.path : undefined)
              .candidates.find(entry => entry.path === candidate.path);
            if (!current || canonicalJson(current) !== canonicalJson(candidate)) throw new Error("retention_plan_changed");
            assertRetentionDirectory(candidate.path, candidate);
            assertQuarantinedRecords(candidate, candidate.path);
            assertRetentionIdle(candidate, candidate.path);
            renameSync(candidate.path, step.quarantine);
            hooks.afterQuarantine?.(step.quarantine);
          }
          assertStartedUnreferenced(repositoryRoot, journal, candidate, step.quarantine);
          if (step.phase === "pending" || step.phase === "quarantined") {
            if (existsSync(candidate.path)) throw new Error("retention_original_path_replaced");
            assertQuarantinedRecords(candidate, step.quarantine);
            assertRetentionIdle(candidate, step.quarantine);
            const index = taskIndex(repositoryRoot);
            const covered = new Map(candidate.coveredRecords.filter(record => record.kind !== "artifact").map(record => [record.id, join(record.path, "task.json")]));
            let present = 0;
            for (const [id, path] of covered) {
              if (index[id] === path) present++;
              else if (index[id] !== undefined) throw new Error("retention_task_index_changed");
            }
            if ((present !== 0 && present !== covered.size) || (step.phase === "pending" && present !== covered.size)) throw new Error("retention_task_index_changed");
            for (const [id, path] of Object.entries(index)) {
              if ((isStrictDescendant(candidate.path, path) || isStrictDescendant(step.quarantine, path)) && !covered.has(id)) throw new Error("retention_task_index_changed");
            }
            step.phase = "quarantined";
            durableRetentionJson(journalPath, journal);
            for (const id of covered.keys()) delete index[id];
            durableRetentionJson(join(repositoryRoot, taskIndexName), index);
            hooks.afterIndexCommit?.(step.quarantine);
            step.phase = "indexed";
            durableRetentionJson(journalPath, journal);
          }
          if (existsSync(step.quarantine)) {
            hooks.beforeRemove?.(step.quarantine);
            assertStartedUnreferenced(repositoryRoot, journal, candidate, step.quarantine);
            assertRetentionDirectory(step.quarantine, candidate);
            assertQuarantinedRecords(candidate, step.quarantine, true);
            assertRetentionIdle(candidate, step.quarantine);
            removeQuarantinedTree(step.quarantine, candidate.directoryDevice);
          }
          step.phase = "removed";
          durableRetentionJson(journalPath, journal);
        } catch (error) {
          if (!recovering) throw error;
          blocked.push(String(error));
        }
      }
      if (blocked.length) throw new Error(`retention_recovery_protected: ${blocked.join("; ")}`);
      const receipt = retirementReceipt(journal, expectedRevision, recovering);
      const receiptPath = join(repositoryRoot, ".test-output/managed-retention-receipts", `${journal.generation}.json`);
      boundedManagedJson(receipt, "retention_history_too_large");
      mkdirSync(dirname(receiptPath), { recursive: true });
      durableRetentionJson(receiptPath, receipt);
      unlinkSync(journalPath);
      syncRetentionDirectory(dirname(journalPath));
      return { ...receipt, receiptPath };
    });
  } finally { cacheLease("release", poolLock, [String(process.pid), poolGeneration]); }
}

/** Gated native exec binds its already-created task before the owner sees the PID. */
export function bindSupervisorTask(repositoryRoot: string, recordPath: string, expected: Pick<TaskIdentity, "id" | "generation">, identity: OwnedProcessIdentity): void {
  withManagedMetadata(repositoryRoot, () => {
    const record = readManagedTask(recordPath, expected);
    if (record.identity.kind !== "runtime-run" || record.state !== "queued" || record.ownedProcesses.length ||
        identity.supervisorPid !== process.ppid) throw new Error("native_task_binding_invalid");
    const observed = spawnSync("ps", ["-p", String(process.ppid), "-o", "lstart="], { env: { ...process.env, LC_ALL: "C" }, encoding: "utf8", timeout: 1000 });
    if (observed.status !== 0 || observed.stdout.trim() !== identity.supervisorStartTime) throw new Error("native_task_supervisor_changed");
    atomicManagedJson(recordPath, { ...record, identity: { ...record.identity, revision: record.identity.revision + 1 },
      state: "running", ownedProcesses: [identity] });
  });
}

/** Adopt only the exact one-revision supervisor binding of this in-memory owned task. */
export function adoptSupervisorTask(task: ManagedTask, identity: OwnedProcessIdentity): void {
  const owned = managedTasks.get(task);
  if (!owned) throw new Error("managed_task_not_owned");
  withManagedMetadata(owned.claim.plan.repoRoot, () => {
    assertOutputOwnership(owned.claim);
    const stat = lstatSync(owned.claim.root);
    if (stat.dev !== owned.device || stat.ino !== owned.inode) throw new Error("managed_task_directory_changed");
    const current = readManagedTask(task.recordPath, task.identity);
    const expected = { ...owned.record, identity: { ...owned.record.identity, revision: owned.record.identity.revision + 1 },
      state: "running", ownedProcesses: [identity] };
    if (canonicalJson(current) !== canonicalJson(expected)) throw new Error("native_task_binding_changed");
    owned.record = current;
  });
}

/** The existing Python session supervisor may finalize its own persisted task.
 * This is not a PID signalling API: only its direct helper child can use it. */
export function finalizeSupervisorTask(repositoryRoot: string, recordPath: string, expected: Pick<TaskIdentity, "id" | "generation">, cleanup: OwnedCleanup, exitCode: number, nativeLifecycle?: unknown, expectedProcess?: OwnedProcessIdentity): TaskRecord {
  return withManagedMetadata(repositoryRoot, () => {
    const record = readManagedTask(recordPath, expected);
    const supervisor = record.ownedProcesses[0];
    if (!supervisor || supervisor.supervisorPid !== process.ppid || ["closed", "protected"].includes(record.state)) throw new Error("session_task_not_owned");
    if (expectedProcess && canonicalJson(record.ownedProcesses) !== canonicalJson([expectedProcess])) throw new Error("native_task_process_changed");
    const observed = spawnSync("ps", ["-p", String(process.ppid), "-o", "lstart="], { env: { ...process.env, LC_ALL: "C" }, encoding: "utf8", timeout: 1000 });
    if (observed.status !== 0 || observed.stdout.trim() !== supervisor.supervisorStartTime) throw new Error("session_supervisor_lifetime_changed");
    const complete = cleanup.closed && cleanup.processExited && cleanup.processGroupExited && cleanup.streamsDrained
      && cleanup.logWriterClosed && cleanup.referencesFinalized && cleanup.ownedWindowsClosed !== false && cleanup.survivors.length === 0;
    const next: TaskRecord = { ...record, identity: { ...record.identity, revision: record.identity.revision + 1 },
      state: complete ? "closed" : "protected", cleanup: { ...cleanup, closed: complete }, result: { ...record.result, status: exitCode === 0 ? "succeeded" : "failed", exitCode,
        ...(nativeLifecycle === undefined ? {} : { nativeLifecycle }) } };
    atomicManagedJson(recordPath, next);
    return next;
  });
}

export function removeFinalizedManagedTask(task: ManagedTask): void {
  const owned = managedTasks.get(task);
  if (!owned) throw new Error("managed_task_not_owned");
  withManagedMetadata(owned.claim.plan.repoRoot, () => {
    const record = readManagedTask(task.recordPath, task.identity);
    if (record.state !== "closed" || !record.cleanup.closed) throw new Error("task_cleanup_unproved");
    const index = taskIndex(owned.claim.plan.repoRoot);
    if (index[record.identity.id] !== task.recordPath) throw new Error("task_index_changed");
    removeOwnedTree(owned.claim);
    delete index[record.identity.id];
    atomicManagedJson(join(owned.claim.plan.repoRoot, taskIndexName), index);
    managedTasks.delete(task);
  });
}
