#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  closeSync,
  existsSync,
  lstatSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  readSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { inflateRawSync } from "node:zlib";
import {
  GENERATED_BYTE_COMPARE_OUTPUT_PATHS,
  GENERATED_BYTE_COMPARE_SOURCE_PATHS,
  validateGeneratedByteCompareReceipt,
} from "./devtools/generated-byte-compare.ts";
import {
  buildCanonicalMappings,
  buildCoverageBindingSet,
  validateCoverageBindingSet,
  type CoverageBindingRecord,
  type SurfaceContractRegistry,
} from "./devtools/surfaces.ts";
import {
  buildRuntimeCoverageScorecard,
  type RuntimeProofReceipt,
} from "./devtools/lib/runtime-coverage.ts";

export const RELEASE_EVIDENCE_SCHEMA_VERSION = 1;
export const RELEASE_MANIFEST_SCHEMA_VERSION = 3;

export type EvidenceClass =
  | "UNIT_BEHAVIOR"
  | "SDK_BEHAVIOR"
  | "STATIC_INVENTORY"
  | "RUNTIME_HIDDEN"
  | "RUNTIME_VISIBLE"
  | "PACKAGED_IDENTITY"
  | "PACKAGED_APP";

const REQUIRED_GATE_CLASSES = {
  "rust-tests": "UNIT_BEHAVIOR",
  "integration-tests": "UNIT_BEHAVIOR",
  "domain-tests": "UNIT_BEHAVIOR",
  "first-run-fixtures": "UNIT_BEHAVIOR",
  "permissions-fixtures": "UNIT_BEHAVIOR",
  "mock-ai-fixtures": "UNIT_BEHAVIOR",
  "privacy-fixtures": "UNIT_BEHAVIOR",
  "proof-contracts": "UNIT_BEHAVIOR",
  "generated-design-contracts": "UNIT_BEHAVIOR",
  "sdk-tests": "SDK_BEHAVIOR",
  "consistency-catalog": "STATIC_INVENTORY",
  "packaged-signing": "PACKAGED_IDENTITY",
  "packaged-root-frame": "RUNTIME_HIDDEN",
  "packaged-first-install": "PACKAGED_APP",
  "packaged-permissions": "PACKAGED_APP",
  "packaged-migration": "PACKAGED_APP",
  "packaged-mock-ai": "PACKAGED_APP",
  "packaged-direct-matrix": "RUNTIME_HIDDEN",
  "packaged-ratified-performance": "RUNTIME_VISIBLE",
} as const satisfies Record<string, EvidenceClass>;

export type GateId = keyof typeof REQUIRED_GATE_CLASSES;

export const REQUIRED_PACKAGED_JOURNEYS = [
  "packaged-first-install",
  "packaged-permissions",
  "packaged-migration",
  "packaged-mock-ai",
] as const;

type PackagedJourneyId = typeof REQUIRED_PACKAGED_JOURNEYS[number];

export const REQUIRED_PACKAGED_ASSURANCES = [
  { id: "direct_matrix", gateId: "packaged-direct-matrix", evidenceClass: "RUNTIME_HIDDEN" },
  {
    id: "ratified_perf",
    gateId: "packaged-ratified-performance",
    evidenceClass: "RUNTIME_VISIBLE",
  },
] as const;

type PackagedAssuranceId = typeof REQUIRED_PACKAGED_ASSURANCES[number]["id"];
type PackagedAssuranceGateId = typeof REQUIRED_PACKAGED_ASSURANCES[number]["gateId"];
const REQUIRED_DIRECT_SURFACE_MAPPING_COUNT = 54;

export const RELEASE_INTEGRATION_SUITES = [
  "ai_capability_preflight_contract",
  "legacy_design_variant_migration",
  "protocol_batch",
  "protocol_wait_for",
  "script_content_model",
  "window_resize_logic",
] as const;

const REQUIRED_PROOF_SUITES = [
  "scripts/devtools/operator-safety.test.ts",
  "scripts/devtools/privacy.test.ts",
  "scripts/devtools/family-fixtures.test.ts",
  "scripts/devtools/alpha-byte-contract.test.ts",
  "scripts/devtools/generated-byte-compare.test.ts",
  "scripts/devtools/state-ownership.test.ts",
  "scripts/migrate/__tests__/classify.test.ts",
  "scripts/agentic/cargo-build-policy.test.ts",
  "scripts/agentic/macos-input.test.ts",
  "scripts/agentic/quick-ai-latency-bench.test.ts",
  "tests/sdk/runner-safety.test.ts",
] as const;

const REQUIRED_OPERATOR_SAFETY_OWNERS = [
  "scripts/agentic/session.sh",
  "scripts/agentic/index.ts",
  "scripts/agentic/flow-composer-multiline-probe.ts",
  "scripts/agentic/cons-flow-ux/dictation-history-probe.ts",
  "scripts/agentic/cons-flow-ux/conversation-hosts-probe.ts",
  "scripts/agentic/cons-flow-ux/notes-actions-probe.ts",
  "scripts/agentic/root-search-visual-stability.ts",
  "scripts/agentic/glass-smoke-study.ts",
  "scripts/agentic/automation-window.ts",
  "scripts/agentic/verify-shot.ts",
  "scripts/agentic/window.ts",
  "scripts/agentic/macos-input.ts",
  "scripts/agentic/macos-input.test.ts",
  "scripts/agentic/filterable-surface-matrix.ts",
  "scripts/agentic/surface-navigator.ts",
  "scripts/agentic/surface-navigator-inventory-audit.ts",
  "scripts/agentic/target-thread.ts",
  "scripts/agentic/scenario.ts",
  "scripts/agentic/devtools-session-lib.sh",
  "scripts/agentic/start-isolated.sh",
  "scripts/agentic/devtools-session.sh",
  "scripts/agentic/wait-session-ready.sh",
  "scripts/devtools/driver.ts",
  "scripts/devtools/actions.ts",
  "scripts/devtools/agent_chat.ts",
  "scripts/devtools/dictation.ts",
  "scripts/devtools/events.ts",
  "scripts/devtools/main.ts",
  "scripts/devtools/notes-live-resize.ts",
  "scripts/devtools/notes-bottom-resize.ts",
  "scripts/devtools/notes-glass-entry-fallback.ts",
  "scripts/devtools/actions-entry-filmstrip.ts",
  "scripts/devtools/glass-lifecycle-filmstrip.ts",
  "scripts/devtools/rapid-toggle-stress.ts",
  "scripts/devtools/glass-observers.ts",
  "scripts/devtools/glass-interference.ts",
  "scripts/devtools/glass-motion-contrast.ts",
  "scripts/devtools/glass-native-helper-cache.ts",
  "scripts/devtools/spotlight-sync-filmstrip.ts",
  "scripts/devtools/main-window-native-drag.ts",
  "scripts/devtools/act.ts",
  "scripts/devtools/devtools.ts",
  "scripts/devtools/perf.ts",
  "scripts/devtools/capture-dom-fidelity.ts",
  "scripts/devtools/window-engine-foundation.ts",
  "scripts/devtools/inspect.ts",
  "scripts/devtools/notes.ts",
  "scripts/devtools/lib/client.ts",
  "scripts/devtools/lib/operator-safety.ts",
  "scripts/devtools/lib/target-identity.ts",
  "scripts/devtools/test-status.ts",
] as const;

const REQUIRED_SDK_SAFETY_OWNERS = [
  "scripts/test-runner.ts",
  "tests/sdk/fixtures/runner-negative-case.ts",
  "tests/sdk/runner-safety.test.ts",
] as const;

const REQUIRED_BUILD_SAFETY_OWNERS = [
  "scripts/agent-check.sh",
  "scripts/agentic/agent-cargo.sh",
  "scripts/agentic/cargo-cache-locks.sh",
  "scripts/agentic/cargo-build-policy.test.ts",
  "scripts/agentic/reuse-rust-test-binary.sh",
  "scripts/agentic/build-isolated-binary.sh",
] as const;

const REQUIRED_FOCUSED_FIXTURES = {
  "first-run-fixtures": "setup::tests::test_fresh_install_seeds_canonical_menu_syntax_handlers",
  "permissions-fixtures": "permissions_wizard::tests::test_snapshot_missing_required",
  "mock-ai-fixtures":
    "ai_reliability::model_tests::blocked_capability_produces_actionable_recovery_without_starting",
  "privacy-fixtures": "ai::reliability::tests::redactor_allowlists_json_masks_secrets_paths_and_bounds_output",
} as const;

export interface GateReceipt {
  schemaVersion: typeof RELEASE_EVIDENCE_SCHEMA_VERSION;
  gateId: GateId;
  evidenceClass: EvidenceClass;
  status: "pass";
  sourceSha: string;
  sourceState: "clean" | "dirty";
  publishable: boolean;
  fingerprintScope?: "DECLARED_OWNERS_NON_EXHAUSTIVE";
  worktreeFingerprintSha256?: string;
  worktreeOwners?: Array<{ path: string; sha256: string }>;
  noninteractive: true;
  observedAt: string;
  result?: {
    passed?: number;
    failed?: number;
    skipped?: number;
    suites?: number;
    suiteNames?: string[];
    files?: number;
    assertions?: number;
    binarySha256?: string;
    exporterSha256?: string;
    exporterSourceFingerprintSha256?: string;
    rawEvidenceSha256?: string;
    designTokensJsonSha256?: string;
    designTokensCssSha256?: string;
    outputCount?: number;
    startsApplication?: false;
    sidecarSha256?: string;
    notarizedArchiveSha256?: string;
    teamIdentifier?: string;
    notarizationSubmissionId?: string;
    hardenedRuntime?: true;
    stapled?: true;
    gatekeeperAccepted?: true;
    metricKind?: string;
    fixtureSha256?: string;
    journeyId?: string;
    assuranceId?: PackagedAssuranceId;
    transactionId?: string;
    surfaceContractSha256?: string;
    bindingFingerprintSha256?: string;
    bindingIds?: string[];
    primitiveReceiptCount?: number;
    expectedMappings?: number;
    directProvenMappings?: number;
    observationLayer?: string;
    measuresPaint?: boolean;
    ownerVisibleAuthorization?: true;
    ownerRatified?: true;
    ratifiedBudgetId?: string;
    ratificationReference?: string;
    sampleCount?: number;
    p50Ms?: number;
    p95Ms?: number;
    maxMs?: number;
  };
}

export interface ManifestOptions {
  zipPath: string;
  appPath: string;
  contractsPath: string;
  designTokensPath: string;
  designCssPath: string;
  designProofPath: string;
  repositoryRoot?: string;
  outputPath: string;
  sourceSha: string;
  version: string;
  tag: string;
  evidencePaths: string[];
}

interface ReleaseManifest {
  schema_version: typeof RELEASE_MANIFEST_SCHEMA_VERSION;
  version: string;
  tag: string;
  source_sha: string;
  bundle: {
    identifier: string;
    info_plist: { path: string; sha256: string };
    content_tree: {
      algorithm: "SHA256_CANONICAL_APP_TREE_V1";
      sha256: string;
      entry_count: number;
    };
    executable: { path: string; sha256: string };
    sidecar: { path: string; sha256: string };
  };
  sdk: { version: string; path: string; sha256: string };
  surface_contracts: { schema_version: number; path: string; sha256: string };
  design_contracts: {
    exporter_sha256: string;
    exporter_source_fingerprint_sha256: string;
    raw_evidence: { path: string; sha256: string };
    tokens_json: { path: string; sha256: string };
    tokens_css: { path: string; sha256: string };
  };
  distribution_security: {
    team_identifier: string;
    notarization_submission_id: string;
    notarized_archive_sha256: string;
    hardened_runtime: true;
    stapled: true;
    gatekeeper_accepted: true;
  };
  verification: {
    status: "pass";
    visibility: {
      journeys: "hidden_only";
      paintedOutput: "owner_authorized_visible";
    };
    gates: Array<GateReceipt & { sha256: string }>;
  };
  artifacts: Array<{
    name: string;
    platform: "macos";
    sha256: string;
    size_bytes: number;
  }>;
}

interface ReleaseScorecard {
  schemaVersion: 1;
  version: string;
  tag: string;
  sourceSha: string;
  bundleIdentifier: string;
  binarySha256: string;
  sidecarSha256: string;
  sdkVersion: string;
  surfaceContractSchemaVersion: number;
  designContracts: {
    exporterSha256: string;
    exporterSourceFingerprintSha256: string;
    rawEvidenceSha256: string;
    tokensJsonSha256: string;
    tokensCssSha256: string;
  };
  archiveSha256: string;
  distributionSecurity: {
    teamIdentifier: string;
    notarizationSubmissionId: string;
    notarizedArchiveSha256: string;
    hardenedRuntime: true;
    stapled: true;
    gatekeeperAccepted: true;
  };
  gates: Array<{
    gateId: GateId;
    evidenceClass: EvidenceClass;
    status: "pass";
    passed?: number;
    failed?: number;
    skipped?: number;
    suites?: number;
    suiteNames?: string[];
    files?: number;
    assertions?: number;
  }>;
  journeys: Array<{
    id: string;
    evidenceClass: "RUNTIME_HIDDEN" | "PACKAGED_APP";
    status: "pass";
    metricKind: string;
    measuresPaint: false;
    startsApplication: true;
    isolatedCiLaunchAuthorized: true;
    revealsWindow: false;
    drivesNativeInput: false;
    capturesScreen: false;
  }>;
  directSurfaceCoverage: {
    status: "pass";
    evidenceClass: "RUNTIME_HIDDEN";
    expectedMappings: number;
    directProvenMappings: number;
    transactionId: string;
    surfaceContractSha256: string;
  };
  paintedLatency:
    | {
      status: "not_measured";
      reason: string;
    }
    | {
      status: "pass";
      evidenceClass: "RUNTIME_VISIBLE";
      metricKind: "PAINTED_OUTPUT";
      p50Ms: number;
      p95Ms: number;
      maxMs: number;
      sampleCount: number;
      budgetRatified: true;
      ownerVisibleAuthorization: true;
      ratifiedBudgetId: string;
      ratificationReference: string;
    };
}

export type SigningCommandRunner = (
  executable: string,
  args: string[],
) => { status: number | null; stdout: string; stderr: string };

interface SigningAttestation {
  sourceSha: string;
  binarySha256: string;
  sidecarSha256: string;
  notarizedArchiveSha256: string;
  teamIdentifier: string;
  notarizationSubmissionId: string;
  hardenedRuntime: true;
  stapled: true;
  gatekeeperAccepted: true;
}

function defaultSigningCommandRunner(executable: string, args: string[]): {
  status: number | null;
  stdout: string;
  stderr: string;
} {
  const result = spawnSync(executable, args, { encoding: "utf8" });
  requireCondition(!result.error,
    `unable to execute distribution-security verification: ${executable}: ${result.error?.message}`);
  return { status: result.status, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

function requireSuccessfulSecurityCommand(
  runCommand: SigningCommandRunner,
  executable: string,
  args: string[],
): string {
  const result = runCommand(executable, args);
  requireCondition(result.status === 0,
    `distribution-security verification failed: ${executable} ${args.join(" ")}: ${result.stderr}`);
  return `${result.stdout}\n${result.stderr}`;
}

export function buildSigningAttestation(options: {
  appPath: string;
  notarizationPath: string;
  notarizedArchivePath: string;
  sourceSha: string;
  teamIdentifier: string;
  runCommand?: SigningCommandRunner;
}): SigningAttestation {
  requireSourceSha(options.sourceSha);
  requireCondition(/^[A-Z0-9]{10}$/.test(options.teamIdentifier),
    "distribution-security attestation requires an exact ten-character Apple team identifier");
  const executablePath = join(options.appPath, "Contents/MacOS/script-kit-gpui");
  const sidecarPath = join(options.appPath, "Contents/MacOS/pi");
  verifyExecutable(executablePath);
  verifyExecutable(sidecarPath);
  const notarization = readJson(options.notarizationPath);
  requireCondition(notarization.status === "Accepted",
    "Apple notarization must explicitly report Accepted for the submitted archive");
  requireCondition(typeof notarization.id === "string" &&
    /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(notarization.id),
    "Apple notarization is missing its exact submission identifier");

  const runCommand = options.runCommand ?? defaultSigningCommandRunner;
  for (const target of [options.appPath, executablePath, sidecarPath]) {
    const details = requireSuccessfulSecurityCommand(runCommand, "codesign", [
      "-d", "--verbose=4", target,
    ]);
    const actualTeam = details.match(/^TeamIdentifier=([^\r\n]+)$/m)?.[1]?.trim();
    requireCondition(actualTeam === options.teamIdentifier,
      `signed artifact belongs to another Apple team: ${target}`);
    requireCondition(/^flags=.*\bruntime\b/m.test(details),
      `signed artifact lacks hardened runtime: ${target}`);
    requireSuccessfulSecurityCommand(runCommand, "codesign", [
      "--verify", "--strict", "--verbose=4", target,
    ]);
  }
  requireSuccessfulSecurityCommand(runCommand, "xcrun", ["stapler", "validate", options.appPath]);
  requireSuccessfulSecurityCommand(runCommand, "spctl", [
    "--assess", "--verbose=4", "--type", "execute", options.appPath,
  ]);

  return {
    sourceSha: options.sourceSha,
    binarySha256: sha256File(executablePath),
    sidecarSha256: sha256File(sidecarPath),
    notarizedArchiveSha256: sha256File(options.notarizedArchivePath),
    teamIdentifier: options.teamIdentifier,
    notarizationSubmissionId: notarization.id,
    hardenedRuntime: true,
    stapled: true,
    gatekeeperAccepted: true,
  };
}

function requireCondition(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}

function requireSourceSha(value: string): void {
  requireCondition(/^[a-f0-9]{40,64}$/.test(value), `invalid source SHA: ${value}`);
}

function sha256File(path: string): string {
  const hash = createHash("sha256");
  const descriptor = openSync(path, "r");
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  try {
    while (true) {
      const count = readSync(descriptor, buffer, 0, buffer.length, null);
      if (count === 0) break;
      hash.update(buffer.subarray(0, count));
    }
  } finally {
    closeSync(descriptor);
  }
  return hash.digest("hex");
}

const RELEASE_ARCHIVE_ROOT = "Script Kit.app";
const RELEASE_ARCHIVE_MAX_DIRECTORY_BYTES = 64 * 1024 * 1024;
const RELEASE_ARCHIVE_MAX_MEMBER_BYTES = 512 * 1024 * 1024;

interface ReleaseArchiveEntry {
  path: string;
  flags: number;
  compression: number;
  crc32: number;
  compressedBytes: number;
  uncompressedBytes: number;
  localOffset: number;
  unixMode: number;
  isDirectory: boolean;
}

interface ReleaseArchiveMembers {
  executable: Buffer;
  sidecar: Buffer;
  sdk: Buffer;
  infoPlist: Buffer;
  contentTree: ReleaseAppContentTree;
}

interface ReleaseAppContentEntry {
  path: string;
  kind: "file" | "symlink";
  mode: number;
  byteLength: number;
  sha256: string;
}

interface ReleaseAppContentTree {
  algorithm: "SHA256_CANONICAL_APP_TREE_V1";
  sha256: string;
  entryCount: number;
  entries: ReleaseAppContentEntry[];
}

function appContentTree(entries: ReleaseAppContentEntry[]): ReleaseAppContentTree {
  const sorted = [...entries].sort((left, right) =>
    Buffer.compare(Buffer.from(left.path, "utf8"), Buffer.from(right.path, "utf8")));
  const digest = createHash("sha256");
  digest.update("script-kit-app-content-tree-v1\0");
  for (const entry of sorted) {
    for (const field of [
      entry.path,
      entry.kind,
      entry.mode.toString(10),
      entry.byteLength.toString(10),
      entry.sha256,
    ]) {
      const bytes = Buffer.from(field, "utf8");
      const length = Buffer.allocUnsafe(4);
      length.writeUInt32BE(bytes.length, 0);
      digest.update(length);
      digest.update(bytes);
    }
  }
  return {
    algorithm: "SHA256_CANONICAL_APP_TREE_V1",
    sha256: digest.digest("hex"),
    entryCount: sorted.length,
    entries: sorted,
  };
}

function applicationContentTree(appPath: string): ReleaseAppContentTree {
  const entries: ReleaseAppContentEntry[] = [];
  const walk = (directory: string, prefix: string): void => {
    for (const member of readdirSync(directory, { withFileTypes: true })) {
      const relativePath = prefix ? `${prefix}/${member.name}` : member.name;
      const path = join(directory, member.name);
      const metadata = lstatSync(path);
      if (metadata.isDirectory()) {
        walk(path, relativePath);
      } else if (metadata.isFile()) {
        entries.push({
          path: relativePath,
          kind: "file",
          mode: metadata.mode & 0o7777,
          byteLength: metadata.size,
          sha256: sha256File(path),
        });
      } else if (metadata.isSymbolicLink()) {
        const target = Buffer.from(readlinkSync(path), "utf8");
        entries.push({
          path: relativePath,
          kind: "symlink",
          mode: metadata.mode & 0o7777,
          byteLength: target.length,
          sha256: createHash("sha256").update(target).digest("hex"),
        });
      } else {
        throw new Error(`signed application contains an unsupported filesystem node: ${relativePath}`);
      }
    }
  };
  walk(appPath, "");
  requireCondition(entries.some((entry) =>
    entry.path === "Contents/_CodeSignature/CodeResources" && entry.kind === "file"),
  "signed application is missing its CodeResources signature envelope");
  return appContentTree(entries);
}

function readArchiveRange(
  descriptor: number,
  archiveBytes: number,
  offset: number,
  length: number,
  description: string,
): Buffer {
  requireCondition(Number.isSafeInteger(offset) && Number.isSafeInteger(length) &&
    offset >= 0 && length >= 0 && offset <= archiveBytes && length <= archiveBytes - offset,
  `release ZIP has an out-of-bounds ${description}`);
  const bytes = Buffer.allocUnsafe(length);
  let consumed = 0;
  while (consumed < length) {
    const count = readSync(descriptor, bytes, consumed, length - consumed, offset + consumed);
    requireCondition(count > 0, `release ZIP has a truncated ${description}`);
    consumed += count;
  }
  return bytes;
}

function releaseArchiveMembers(zipPath: string): ReleaseArchiveMembers {
  const metadata = statSync(zipPath);
  requireCondition(metadata.isFile() && metadata.size >= 22,
    "release archive is not a valid ZIP: missing end-of-central-directory record");
  const descriptor = openSync(zipPath, "r");
  try {
    const tailBytes = Math.min(metadata.size, 22 + 0xffff);
    const tailOffset = metadata.size - tailBytes;
    const tail = readArchiveRange(descriptor, metadata.size, tailOffset, tailBytes, "ZIP trailer");
    let trailerOffset = -1;
    for (let index = tail.length - 22; index >= 0; index--) {
      if (tail.readUInt32LE(index) === 0x06054b50 &&
        index + 22 + tail.readUInt16LE(index + 20) === tail.length) {
        trailerOffset = index;
        break;
      }
    }
    requireCondition(trailerOffset >= 0,
      "release archive is not a valid ZIP: missing end-of-central-directory record");
    requireCondition(tail.readUInt16LE(trailerOffset + 4) === 0 &&
      tail.readUInt16LE(trailerOffset + 6) === 0,
    "release ZIP cannot span multiple disks");
    const entriesOnDisk = tail.readUInt16LE(trailerOffset + 8);
    const entryCount = tail.readUInt16LE(trailerOffset + 10);
    const directoryBytes = tail.readUInt32LE(trailerOffset + 12);
    const directoryOffset = tail.readUInt32LE(trailerOffset + 16);
    requireCondition(entriesOnDisk === entryCount && entryCount !== 0xffff &&
      directoryBytes !== 0xffffffff && directoryOffset !== 0xffffffff,
    "release ZIP64 or split archives are unsupported");
    requireCondition(directoryBytes <= RELEASE_ARCHIVE_MAX_DIRECTORY_BYTES,
      "release ZIP central directory exceeds its bounded inspection budget");
    requireCondition(directoryOffset + directoryBytes === tailOffset + trailerOffset,
      "release ZIP central directory does not end at its verified trailer");
    const directory = readArchiveRange(
      descriptor,
      metadata.size,
      directoryOffset,
      directoryBytes,
      "central directory",
    );

    const entries = new Map<string, ReleaseArchiveEntry>();
    const normalizedEntries = new Map<string, ReleaseArchiveEntry>();
    let cursor = 0;
    for (let index = 0; index < entryCount; index++) {
      requireCondition(cursor + 46 <= directory.length &&
        directory.readUInt32LE(cursor) === 0x02014b50,
      "release ZIP contains an invalid central-directory entry");
      const flags = directory.readUInt16LE(cursor + 8);
      const compression = directory.readUInt16LE(cursor + 10);
      const nameBytes = directory.readUInt16LE(cursor + 28);
      const extraBytes = directory.readUInt16LE(cursor + 30);
      const commentBytes = directory.readUInt16LE(cursor + 32);
      const recordBytes = 46 + nameBytes + extraBytes + commentBytes;
      requireCondition(nameBytes > 0 && cursor + recordBytes <= directory.length,
        "release ZIP contains a truncated central-directory entry");
      const rawName = directory.subarray(cursor + 46, cursor + 46 + nameBytes);
      const rawPath = rawName.toString("utf8");
      requireCondition(Buffer.from(rawPath, "utf8").equals(rawName),
        "release ZIP contains a non-UTF-8 member path");
      const isDirectory = rawPath.endsWith("/");
      const path = isDirectory ? rawPath.slice(0, -1) : rawPath;
      const parts = path.split("/");
      requireCondition(parts.every((part) => part.length > 0 && part !== "." && part !== "..") &&
        !path.includes("\\") && !path.includes("\0"),
      `release ZIP contains an unsafe member path: ${rawPath}`);
      requireCondition(parts[0] === RELEASE_ARCHIVE_ROOT || parts[0] === "__MACOSX",
        `release ZIP contains an unexpected top-level application root: ${rawPath}`);
      const normalizedPath = path.normalize("NFC").toLowerCase();
      requireCondition(!normalizedEntries.has(normalizedPath),
        `release ZIP contains duplicate or aliased member paths: ${rawPath}`);
      requireCondition((flags & 0x41) === 0,
        `release ZIP contains an encrypted member: ${rawPath}`);
      requireCondition(compression === 0 || compression === 8,
        `release ZIP uses unsupported compression for ${rawPath}`);
      const compressedBytes = directory.readUInt32LE(cursor + 20);
      const uncompressedBytes = directory.readUInt32LE(cursor + 24);
      const localOffset = directory.readUInt32LE(cursor + 42);
      requireCondition(compressedBytes !== 0xffffffff && uncompressedBytes !== 0xffffffff &&
        localOffset !== 0xffffffff,
      `release ZIP64 member is unsupported: ${rawPath}`);

      const entry = {
        path,
        flags,
        compression,
        crc32: directory.readUInt32LE(cursor + 16),
        compressedBytes,
        uncompressedBytes,
        localOffset,
        unixMode: (directory.readUInt32LE(cursor + 38) >>> 16) & 0xffff,
        isDirectory,
      };
      entries.set(path, entry);
      normalizedEntries.set(normalizedPath, entry);
      cursor += recordBytes;
    }
    requireCondition(cursor === directory.length,
      "release ZIP central-directory entry count does not cover its exact bytes");

    const readMember = (relativePath: string, requiredRegular = true): Buffer => {
      const path = `${RELEASE_ARCHIVE_ROOT}/${relativePath}`;
      const entry = entries.get(path);
      requireCondition(entry && !entry.isDirectory,
        `release ZIP is missing its required application member: ${relativePath}`);
      const type = entry.unixMode & 0o170000;
      if (requiredRegular) {
        requireCondition(type === 0 || type === 0o100000,
          `release ZIP required member is not a regular file: ${relativePath}`);
      } else {
        requireCondition(type === 0 || type === 0o100000 || type === 0o120000,
          `release ZIP contains an unsupported signed application node: ${relativePath}`);
      }

      const components = path.split("/");
      for (let index = 1; index < components.length; index++) {
        const ancestor = components.slice(0, index).join("/");
        const parent = normalizedEntries.get(ancestor.normalize("NFC").toLowerCase());
        if (!parent) continue;
        const parentType = parent.unixMode & 0o170000;
        requireCondition(parent.isDirectory && (parentType === 0 || parentType === 0o040000),
          `release ZIP required member traverses a non-directory or symlink: ${ancestor}`);
      }
      requireCondition((!requiredRegular || entry.uncompressedBytes > 0) &&
        entry.uncompressedBytes <= RELEASE_ARCHIVE_MAX_MEMBER_BYTES &&
        entry.compressedBytes <= RELEASE_ARCHIVE_MAX_MEMBER_BYTES,
      `release ZIP required member exceeds its bounded inspection budget: ${relativePath}`);

      const local = readArchiveRange(
        descriptor,
        metadata.size,
        entry.localOffset,
        30,
        `local header for ${relativePath}`,
      );
      requireCondition(local.readUInt32LE(0) === 0x04034b50 &&
        local.readUInt16LE(6) === entry.flags &&
        local.readUInt16LE(8) === entry.compression,
      `release ZIP local header disagrees with its central directory: ${relativePath}`);
      const localNameBytes = local.readUInt16LE(26);
      const localExtraBytes = local.readUInt16LE(28);
      const localName = readArchiveRange(
        descriptor,
        metadata.size,
        entry.localOffset + 30,
        localNameBytes,
        `local path for ${relativePath}`,
      );
      requireCondition(localName.equals(Buffer.from(entry.path, "utf8")),
        `release ZIP local member path disagrees with its central directory: ${relativePath}`);
      if ((entry.flags & 0x08) === 0) {
        requireCondition(local.readUInt32LE(14) === entry.crc32 &&
          local.readUInt32LE(18) === entry.compressedBytes &&
          local.readUInt32LE(22) === entry.uncompressedBytes,
        `release ZIP local member sizes disagree with its central directory: ${relativePath}`);
      }
      const dataOffset = entry.localOffset + 30 + localNameBytes + localExtraBytes;
      requireCondition(dataOffset + entry.compressedBytes <= directoryOffset,
        `release ZIP member data overlaps its central directory: ${relativePath}`);
      const compressed = readArchiveRange(
        descriptor,
        metadata.size,
        dataOffset,
        entry.compressedBytes,
        `member data for ${relativePath}`,
      );
      const contents = entry.compression === 0
        ? compressed
        : inflateRawSync(compressed, { maxOutputLength: RELEASE_ARCHIVE_MAX_MEMBER_BYTES });
      requireCondition(contents.length === entry.uncompressedBytes,
        `release ZIP member size disagrees with its central directory: ${relativePath}`);
      return contents;
    };

    const requiredMembers = new Map<string, Buffer>([
      ["Contents/MacOS/script-kit-gpui", readMember("Contents/MacOS/script-kit-gpui")],
      ["Contents/MacOS/pi", readMember("Contents/MacOS/pi")],
      ["Contents/Resources/scripts/kit-sdk.ts", readMember("Contents/Resources/scripts/kit-sdk.ts")],
      ["Contents/Info.plist", readMember("Contents/Info.plist")],
    ]);
    const appEntries: ReleaseAppContentEntry[] = [];
    for (const entry of entries.values()) {
      if (entry.isDirectory || !entry.path.startsWith(`${RELEASE_ARCHIVE_ROOT}/`)) continue;
      const relativePath = entry.path.slice(RELEASE_ARCHIVE_ROOT.length + 1);
      const type = entry.unixMode & 0o170000;
      const kind = type === 0o120000 ? "symlink" : "file";
      const contents = requiredMembers.get(relativePath) ?? readMember(relativePath, false);
      requireCondition(kind !== "symlink" || contents.length > 0,
        `release ZIP contains an empty symbolic-link target: ${relativePath}`);
      appEntries.push({
        path: relativePath,
        kind,
        mode: entry.unixMode & 0o7777,
        byteLength: contents.length,
        sha256: createHash("sha256").update(contents).digest("hex"),
      });
    }
    requireCondition(appEntries.some((entry) =>
      entry.path === "Contents/_CodeSignature/CodeResources" && entry.kind === "file"),
    "release ZIP is missing its signed CodeResources envelope");

    return {
      executable: requiredMembers.get("Contents/MacOS/script-kit-gpui")!,
      sidecar: requiredMembers.get("Contents/MacOS/pi")!,
      sdk: requiredMembers.get("Contents/Resources/scripts/kit-sdk.ts")!,
      infoPlist: requiredMembers.get("Contents/Info.plist")!,
      contentTree: appContentTree(appEntries),
    };
  } finally {
    closeSync(descriptor);
  }
}

function readJson(path: string): Record<string, any> {
  const value: unknown = JSON.parse(readFileSync(path, "utf8"));
  requireCondition(value !== null && typeof value === "object" && !Array.isArray(value),
    `${path} must contain a JSON object`);
  return value as Record<string, any>;
}

function writeJson(path: string, value: unknown): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function verifyExecutable(path: string): void {
  const metadata = statSync(path);
  requireCondition(metadata.isFile(), `required executable is not a file: ${path}`);
  requireCondition((metadata.mode & 0o111) !== 0, `required executable is not executable: ${path}`);
}

function bundleIdentifierFromContents(plist: string, description: string): string {
  const match = plist.match(/<key>\s*CFBundleIdentifier\s*<\/key>\s*<string>\s*([^<]+?)\s*<\/string>/);
  requireCondition(match?.[1], `Info.plist does not contain CFBundleIdentifier: ${description}`);
  return match[1].trim();
}

function bundleIdentifier(plistPath: string): string {
  return bundleIdentifierFromContents(readFileSync(plistPath, "utf8"), plistPath);
}

function sdkVersion(sdkPath: string): string {
  const sdk = readFileSync(sdkPath, "utf8");
  const match = sdk.match(/export\s+const\s+SDK_VERSION\s*=\s*['"]([^'"]+)['"]/);
  requireCondition(match?.[1], `bundled SDK does not export SDK_VERSION: ${sdkPath}`);
  return match[1];
}

function requireGateId(value: string): asserts value is GateId {
  requireCondition(Object.hasOwn(REQUIRED_GATE_CLASSES, value), `unknown release gate: ${value}`);
}

function sdkSuiteSummary(path: string): { passed: number; failed: number; skipped: number } {
  const result = readJson(path);
  requireCondition(Number.isInteger(result.total_passed) && result.total_passed > 0,
    "SDK gate must contain at least one passing behavior test");
  requireCondition(result.total_failed === 0, "SDK gate contains failing tests");
  requireCondition(Number.isInteger(result.total_skipped) && result.total_skipped === 0,
    "SDK release gate cannot skip behavior coverage or depend on unavailable external services");
  return { passed: result.total_passed, failed: result.total_failed, skipped: result.total_skipped };
}

function rustSuiteSummary(path: string): { passed: number; failed: number; skipped: number } {
  const output = readFileSync(path, "utf8").replace(/\x1b\[[0-9;]*m/g, "");
  const summaries = [...output.matchAll(
    /test result:\s+ok\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored/g,
  )];
  requireCondition(summaries.length > 0,
    "Rust gate is missing executed test-result summaries; compile-only output cannot satisfy it");

  const counts = summaries.reduce((total, match) => ({
    passed: total.passed + Number(match[1]),
    failed: total.failed + Number(match[2]),
    skipped: total.skipped + Number(match[3]),
  }), { passed: 0, failed: 0, skipped: 0 });

  requireCondition(counts.passed > 0, "Rust gate did not execute any passing tests");
  requireCondition(counts.failed === 0 && !/test result:\s+FAILED\b/.test(output),
    "Rust gate contains failing tests");
  return counts;
}

function focusedRustSuiteSummary(
  path: string,
  expectedTest: string,
): { passed: number; failed: number; skipped: number } {
  const output = readFileSync(path, "utf8").replace(/\x1b\[[0-9;]*m/g, "");
  const summary = rustSuiteSummary(path);
  const escapedTest = expectedTest.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  requireCondition(
    new RegExp(`^\\s*test\\s+${escapedTest}\\s+\\.\\.\\.\\s+ok\\s*$`, "m").test(output),
    `focused fixture did not execute the required passing test: ${expectedTest}`,
  );
  requireCondition(summary.passed === 1,
    `focused fixture must execute exactly its required test: ${expectedTest}`);
  return summary;
}

function integrationSuiteSummary(path: string): {
  passed: number;
  failed: number;
  skipped: number;
  suites: number;
  suiteNames: string[];
} {
  const output = readFileSync(path, "utf8").replace(/\x1b\[[0-9;]*m/g, "");
  requireCondition(!/test result:\s+FAILED\b/.test(output),
    "integration gate contains failing tests");

  const required = new Set<string>(RELEASE_INTEGRATION_SUITES);
  const observed = new Map<string, { passed: number; failed: number; skipped: number }>();
  let activeSuite: string | undefined;

  for (const line of output.split(/\r?\n/)) {
    if (/^\s*Running\s+/.test(line)) {
      activeSuite = line.match(/^\s*Running\s+tests\/([\w.-]+)\.rs\b/)?.[1];
      continue;
    }
    const result = line.match(
      /test result:\s+ok\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored/,
    );
    if (!result || !activeSuite || !required.has(activeSuite)) continue;
    requireCondition(!observed.has(activeSuite),
      `integration suite produced duplicate result summaries: ${activeSuite}`);
    const passed = Number(result[1]);
    const failed = Number(result[2]);
    const skipped = Number(result[3]);
    requireCondition(passed > 0, `integration suite executed no passing tests: ${activeSuite}`);
    requireCondition(failed === 0, `integration suite contains failing tests: ${activeSuite}`);
    observed.set(activeSuite, { passed, failed, skipped });
    activeSuite = undefined;
  }

  for (const suite of RELEASE_INTEGRATION_SUITES) {
    requireCondition(observed.has(suite),
      `integration gate is missing a directly executed required suite: ${suite}`);
  }

  const counts = [...observed.values()].reduce((total, result) => ({
    passed: total.passed + result.passed,
    failed: total.failed + result.failed,
    skipped: total.skipped + result.skipped,
  }), { passed: 0, failed: 0, skipped: 0 });
  return {
    ...counts,
    suites: observed.size,
    suiteNames: [...observed.keys()].sort(),
  };
}

function proofSuiteSummary(path: string): {
  passed: number;
  failed: number;
  skipped: number;
  files: number;
  assertions: number;
} {
  const output = readFileSync(path, "utf8").replace(/\x1b\[[0-9;]*m/g, "");
  const passed = output.match(/^\s*(\d+)\s+pass\s*$/m);
  const failed = output.match(/^\s*(\d+)\s+fail\s*$/m);
  const assertions = output.match(/^\s*(\d+)\s+expect\(\)\s+calls\s*$/m);
  const totals = output.match(/^Ran\s+(\d+)\s+tests?\s+across\s+(\d+)\s+files?\./m);
  requireCondition(passed && failed && assertions && totals,
    "proof gate requires complete executed Bun test, assertion, and file summaries");
  const passedCount = Number(passed[1]);
  const failedCount = Number(failed[1]);
  const assertionCount = Number(assertions[1]);
  const executedCount = Number(totals[1]);
  const fileCount = Number(totals[2]);
  requireCondition(passedCount > 0 && assertionCount > 0 && fileCount > 0,
    "proof gate must execute nonzero tests, assertions, and files");
  requireCondition(failedCount === 0 && passedCount === executedCount,
    "proof gate contains failing or unaccounted behavior tests");
  for (const suite of REQUIRED_PROOF_SUITES) {
    requireCondition(output.includes(`${suite}:`),
      `proof gate is missing its required directly executed fixture suite: ${suite}`);
  }
  return {
    passed: passedCount,
    failed: failedCount,
    skipped: 0,
    files: fileCount,
    assertions: assertionCount,
  };
}

function packagedJourneySummary(
  path: string,
  expectedJourney: PackagedJourneyId,
  sourceSha: string,
): NonNullable<GateReceipt["result"]> {
  const receipt = readJson(path);
  requireCondition(receipt.status === "pass" && receipt.evidenceClass === "PACKAGED_APP",
    `${expectedJourney} requires direct packaged-app behavior, never a unit fixture or hidden-root alias`);
  requireCondition(receipt.journey?.id === expectedJourney && receipt.journey?.status === "pass",
    `${expectedJourney} requires its own exact passing packaged user journey`);
  requireCondition(receipt.provenance?.gitSha === sourceSha,
    `${expectedJourney} belongs to another source revision`);
  for (const field of ["binarySha256", "sidecarSha256", "fixtureSha256"]) {
    requireCondition(/^[a-f0-9]{64}$/.test(String(receipt.provenance?.[field])),
      `${expectedJourney} requires an exact application, sidecar, and isolated fixture identity`);
  }

  const safety = receipt.safety;
  requireCondition(safety?.startsApplication === true &&
    safety.isolatedCiLaunchAuthorized === true && safety.sandboxHome === true &&
    safety.windowRevealAllowed === false && safety.windowFocusAllowed === false &&
    safety.nativeInputAllowed === false && safety.screenCaptureAllowed === false &&
    safety.microphoneAllowed === false && safety.cameraAllowed === false &&
    safety.liveAiAllowed === false,
    `${expectedJourney} lacks a hidden, isolated, no-input/no-capture/no-device/no-provider contract`);
  requireCondition(receipt.cleanup?.hidden === true && receipt.cleanup?.closed === true,
    `${expectedJourney} leaked or failed to terminate its isolated application`);

  const observations = receipt.journey.observations;
  switch (expectedJourney) {
    case "packaged-first-install":
      requireCondition(observations?.freshSandboxHome === true &&
        observations.bundledSdkDiscovered === true &&
        Number.isInteger(observations.starterScriptsIndexed) &&
        observations.starterScriptsIndexed > 0 && observations.readyToType === true,
        "packaged first-install must prove a fresh home, bundled SDK, indexed starters, and readiness");
      break;
    case "packaged-permissions":
      requireCondition(observations?.syntheticPermissionSnapshot === true &&
        observations.permissionRequestsStarted === 0 &&
        Array.isArray(observations.missingRequired) &&
        observations.missingRequired.includes("Accessibility") &&
        observations.recoverableGuidanceVisibleToAutomation === true,
        "packaged permissions must prove synthetic denial and actionable guidance without requests");
      break;
    case "packaged-migration":
      requireCondition(/^[a-f0-9]{64}$/.test(String(observations?.originalUserDataSha256)) &&
        observations.originalUserDataSha256 === observations.preservedUserDataSha256 &&
        observations.legacyFixtureLoaded === true && observations.migrationCompleted === true,
        "packaged migration must prove a real isolated legacy fixture and exact preserved user bytes");
      break;
    case "packaged-mock-ai":
      requireCondition(observations?.mockProvider === true &&
        observations.liveProviderStarts === 0 && observations.failureObserved === true &&
        Number.isInteger(observations.recoveryActionCount) && observations.recoveryActionCount > 0,
        "packaged mock AI must prove a real recovery transition without a live provider");
      break;
  }

  return {
    binarySha256: receipt.provenance.binarySha256,
    sidecarSha256: receipt.provenance.sidecarSha256,
    fixtureSha256: receipt.provenance.fixtureSha256,
    journeyId: expectedJourney,
    metricKind: "packaged_app_journey",
  };
}

const authoritativeBindingCache = new Map<string, {
  bindings: CoverageBindingRecord[];
  fingerprint: string;
}>();
const directRuntimeScorecardCache = new Map<string,
  ReturnType<typeof buildRuntimeCoverageScorecard>>();

function authoritativeSurfaceBindings(
  repositoryRoot: string,
  expectedContractSha256: string,
): { bindings: CoverageBindingRecord[]; fingerprint: string } {
  const contractPath = join(repositoryRoot, "docs/ai/contracts/surface-contracts.json");
  requireCondition(existsSync(contractPath) &&
    sha256File(contractPath) === expectedContractSha256,
  "direct_matrix canonical contract must be the exact independently loaded candidate source artifact");

  const cached = authoritativeBindingCache.get(expectedContractSha256);
  if (cached) return cached;

  const registry = readJson(contractPath) as unknown as SurfaceContractRegistry;
  requireCondition(registry.schemaVersion === 1 && Array.isArray(registry.entries),
    "direct_matrix requires the complete canonical generated surface-contract registry");
  const build = buildCoverageBindingSet(buildCanonicalMappings(registry));
  const validation = validateCoverageBindingSet(build.set);
  requireCondition(build.errors.length === 0 && validation.pass &&
    build.set.bindings.length === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT,
  "direct_matrix canonical binding/profile/primitive inventory is invalid or incomplete");

  const value = { bindings: build.set.bindings, fingerprint: build.set.fingerprint };
  if (authoritativeBindingCache.size >= 8) authoritativeBindingCache.clear();
  authoritativeBindingCache.set(expectedContractSha256, value);
  return value;
}

function directRuntimeCoverage(
  observation: Record<string, any>,
  bindings: CoverageBindingRecord[],
  fingerprint: string,
  sourceSha: string,
  binarySha256: string,
): {
  scorecard: ReturnType<typeof buildRuntimeCoverageScorecard>;
  rawReceipts: RuntimeProofReceipt[];
} {
  requireCondition(Array.isArray(observation.primitiveReceipts) &&
    observation.primitiveReceipts.length > 0,
  "direct_matrix requires actual authoritative target-scoped primitive runtime receipts");

  const paths = new Set<string>();
  const identities = new Set<string>();
  const rawReceipts = observation.primitiveReceipts.map((entry: any) => {
    requireCondition(entry && typeof entry === "object" &&
      typeof entry.path === "string" && entry.path.length > 0 &&
      !isAbsolute(entry.path) && !entry.path.split("/").includes("..") &&
      !paths.has(entry.path) &&
      entry.receipt && typeof entry.receipt === "object" &&
      entry.receipt.evidenceClass === "RUNTIME_HIDDEN" &&
      typeof entry.receipt.receiptId === "string" &&
      entry.receipt.receiptId.length > 0 &&
      !identities.has(entry.receipt.receiptId) &&
      entry.receipt.transaction?.transactionId === observation.transactionId,
    "direct_matrix contains missing, duplicate, visible, or foreign-transaction primitive evidence");
    paths.add(entry.path);
    identities.add(entry.receipt.receiptId);
    return entry as RuntimeProofReceipt;
  });

  const rawSha256 = createHash("sha256")
    .update(JSON.stringify(rawReceipts), "utf8")
    .digest("hex");
  const cacheKey = `${fingerprint}:${sourceSha}:${binarySha256}:${rawSha256}`;
  let scorecard = directRuntimeScorecardCache.get(cacheKey);
  if (!scorecard) {
    scorecard = buildRuntimeCoverageScorecard(bindings, rawReceipts, {
      sourceCommit: sourceSha,
      binarySha256,
    });
    if (directRuntimeScorecardCache.size >= 8) directRuntimeScorecardCache.clear();
    directRuntimeScorecardCache.set(cacheKey, scorecard);
  }
  requireCondition(scorecard.disposition === "EVALUABLE_PASS" &&
    scorecard.evidenceClass === "DIRECT_RUNTIME_PROOF" &&
    scorecard.totalMappingCount === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT &&
    scorecard.directRuntimeMappingCount === scorecard.totalMappingCount &&
    scorecard.acceptedReceiptCount === rawReceipts.length &&
    scorecard.rejectedReceipts.length === 0 &&
    scorecard.unsupportedMappingCount === 0 &&
    scorecard.scenarioFailureCount === 0 && scorecard.privacyViolationCount === 0 &&
    scorecard.staleOwnerPathCount === 0,
  "direct_matrix canonical runtime scorecard rejects missing, stale, unsafe, unsupported, or forged primitives");
  return { scorecard, rawReceipts };
}

function packagedAssuranceSummary(
  path: string,
  gateId: PackagedAssuranceGateId,
  sourceSha: string,
  repositoryRoot: string,
): NonNullable<GateReceipt["result"]> {
  const receipt = readJson(path);
  const requirement = REQUIRED_PACKAGED_ASSURANCES.find((item) => item.gateId === gateId)!;
  requireCondition(receipt.status === "pass" && receipt.evidenceClass === requirement.evidenceClass,
    `${gateId} requires direct ${requirement.evidenceClass} evidence, never static or synthetic inventory`);
  requireCondition(receipt.assurance?.id === requirement.id && receipt.assurance.status === "pass",
    `${gateId} requires its own exact passing candidate assurance`);
  requireCondition(receipt.provenance?.gitSha === sourceSha &&
    /^[a-f0-9]{64}$/.test(String(receipt.provenance.binarySha256)) &&
    /^[a-f0-9]{64}$/.test(String(receipt.provenance.sidecarSha256)),
    `${gateId} must bind to the exact signed source, application, and Pi sidecar`);

  const safety = receipt.safety;
  requireCondition(safety?.startsApplication === true &&
    safety.isolatedCiLaunchAuthorized === true && safety.sandboxHome === true &&
    safety.windowFocusAllowed === false && safety.nativeInputAllowed === false &&
    safety.screenCaptureAllowed === false && safety.microphoneAllowed === false &&
    safety.cameraAllowed === false && safety.liveAiAllowed === false,
    `${gateId} requires isolated no-focus/no-input/no-capture/no-device/no-provider evidence`);
  requireCondition(receipt.cleanup?.hidden === true && receipt.cleanup?.closed === true,
    `${gateId} did not cleanly hide and terminate its exact isolated candidate`);

  const observation = receipt.assurance.observation;
  if (gateId === "packaged-direct-matrix") {
    requireCondition(safety.windowRevealAllowed === false &&
      observation?.observationLayer === "RUNTIME_HIDDEN" &&
      observation.expectedMappings === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT &&
      observation.directProvenMappings === observation.expectedMappings &&
      typeof observation.transactionId === "string" && observation.transactionId.length > 0 &&
      /^[a-f0-9]{64}$/.test(String(receipt.provenance.surfaceContractSha256)) &&
      Array.isArray(observation.targets) &&
      observation.targets.length === observation.expectedMappings &&
      observation.everyTargetDirect === true && observation.sameTransaction === true &&
      observation.privacyRedacted === true && observation.cleanupVerified === true,
    "direct_matrix requires complete target-scoped same-transaction hidden proof with privacy and cleanup");
    const canonical = authoritativeSurfaceBindings(
      repositoryRoot,
      receipt.provenance.surfaceContractSha256,
    );
    const runtime = directRuntimeCoverage(
      observation,
      canonical.bindings,
      canonical.fingerprint,
      sourceSha,
      receipt.provenance.binarySha256,
    );
    const bindings = new Map(canonical.bindings.map((binding) => [binding.bindingId, binding]));
    const mappings = new Map(runtime.scorecard.mappings.map((mapping) =>
      [mapping.bindingId, mapping]));
    const primitiveReceipts = new Map(runtime.rawReceipts.map((candidate) =>
      [candidate.path, candidate.receipt]));
    const bindingIds = new Set<string>();
    const targetStates = new Set<string>();
    for (const target of observation.targets) {
      const binding = bindings.get(target?.bindingId);
      const mapping = mappings.get(target?.bindingId);
      const expected = binding?.expectedTargetIdentity;
      const targetStateId = `${target?.targetId}:${target?.windowGeneration}:` +
        `${target?.surfaceGeneration}:${target?.surfaceKind}:${target?.appViewVariant}`;
      requireCondition(typeof target.bindingId === "string" && target.bindingId.length > 0 &&
        typeof target.targetId === "string" && target.targetId.length > 0 &&
        !bindingIds.has(target.bindingId) && !targetStates.has(targetStateId) &&
        binding !== undefined && mapping !== undefined &&
        target.surfaceKind === binding.contractKind &&
        target.appViewVariant === binding.appViewVariant &&
        target.windowKind === expected!.windowKind && target.hostKind === expected!.hostKind &&
        (!expected!.parentRequired ||
          (typeof target.parentAutomationId === "string" &&
            target.parentAutomationId.length > 0 &&
            target.parentAutomationId !== target.targetId)) &&
        target.transactionId === observation.transactionId &&
        target.evidenceClass === "RUNTIME_HIDDEN" && target.sourceSha === sourceSha &&
        target.binarySha256 === receipt.provenance.binarySha256 &&
        Number.isInteger(target.lifetimeGeneration) && target.lifetimeGeneration > 0 &&
        typeof target.windowInstanceId === "string" && target.windowInstanceId.length > 0 &&
        Number.isInteger(target.windowGeneration) && target.windowGeneration > 0 &&
        Number.isInteger(target.targetGeneration) && target.targetGeneration > 0 &&
        Number.isInteger(target.surfaceGeneration) && target.surfaceGeneration > 0 &&
        target.privacyRedacted === true && target.cleanupVerified === true &&
        mapping.status === "DIRECT_RUNTIME_PASS" &&
        mapping.transactionId === observation.transactionId &&
        mapping.requiredPrimitiveIds.length > 0 &&
        mapping.requiredPrimitiveIds.length === mapping.provenPrimitiveIds.length &&
        mapping.receiptPaths.length === mapping.requiredPrimitiveIds.length,
      "direct_matrix contains a foreign canonical surface/AppView/host/parent, reused state, stale generation, or incomplete direct evidence");

      for (const primitivePath of mapping.receiptPaths) {
        const primitive = primitiveReceipts.get(primitivePath);
        const transaction = primitive?.transaction as Record<string, unknown> | undefined;
        requireCondition(primitive && transaction &&
          primitive.evidenceClass === "RUNTIME_HIDDEN" &&
          primitive.primitiveId && binding.requiredPrimitiveIds.includes(String(primitive.primitiveId)) &&
          transaction.transactionId === target.transactionId &&
          transaction.automationId === target.targetId &&
          transaction.windowInstanceId === target.windowInstanceId &&
          transaction.windowGeneration === target.windowGeneration &&
          transaction.targetGeneration === target.targetGeneration &&
          transaction.surfaceGeneration === target.surfaceGeneration &&
          transaction.windowKind === target.windowKind &&
          transaction.hostKind === target.hostKind &&
          transaction.surfaceKind === target.surfaceKind &&
          transaction.appViewVariant === target.appViewVariant &&
          (!expected!.parentRequired ||
            transaction.parentAutomationId === target.parentAutomationId),
        "direct_matrix primitive does not prove its exact canonical target/window/parent/generation identity");
      }
      bindingIds.add(target.bindingId);
      targetStates.add(targetStateId);
    }
    return {
      assuranceId: "direct_matrix",
      binarySha256: receipt.provenance.binarySha256,
      sidecarSha256: receipt.provenance.sidecarSha256,
      metricKind: "direct_surface_matrix",
      observationLayer: "RUNTIME_HIDDEN",
      measuresPaint: false,
      transactionId: observation.transactionId,
      surfaceContractSha256: receipt.provenance.surfaceContractSha256,
      bindingFingerprintSha256: canonical.fingerprint,
      rawEvidenceSha256: sha256File(path),
      primitiveReceiptCount: runtime.rawReceipts.length,
      bindingIds: [...bindingIds].sort(),
      expectedMappings: observation.expectedMappings,
      directProvenMappings: observation.directProvenMappings,
    };
  }

  requireCondition(safety.windowRevealAllowed === true &&
    safety.ownerVisibleAuthorization === true,
    "ratified_perf requires separate explicit owner authorization for actually visible paint");
  requireCondition(observation?.observationLayer === "PAINTED_OUTPUT" &&
    observation.metricKind === "painted_latency" && observation.measuresPaint === true &&
    observation.ownerRatified === true &&
    typeof observation.ratifiedBudgetId === "string" && observation.ratifiedBudgetId.length > 0 &&
    typeof observation.ratificationReference === "string" &&
    observation.ratificationReference.length > 0 &&
    Number.isInteger(observation.sampleCount) && observation.sampleCount > 0 &&
    Array.isArray(observation.samplesMs) &&
    observation.samplesMs.length === observation.sampleCount &&
    observation.samplesMs.every((sample: unknown) => typeof sample === "number" &&
      Number.isFinite(sample) && sample > 0) &&
    observation.paintEvidence?.source === "compositor_presented_frame" &&
    Number.isInteger(observation.paintEvidence.presentedFrameCount) &&
    observation.paintEvidence.presentedFrameCount >= observation.sampleCount,
    "ratified_perf requires actual painted-output observations, owner-ratified budget, reference, and samples");
  for (const name of ["p50Ms", "p95Ms", "maxMs"]) {
    requireCondition(typeof observation[name] === "number" &&
      Number.isFinite(observation[name]) && observation[name] > 0 &&
      typeof observation.budgets?.[name] === "number" &&
      Number.isFinite(observation.budgets[name]) && observation.budgets[name] > 0 &&
      observation[name] <= observation.budgets[name],
      `ratified_perf ${name} must be observed and satisfy its explicitly ratified budget`);
  }
  requireCondition(observation.p50Ms <= observation.p95Ms && observation.p95Ms <= observation.maxMs,
    "ratified_perf percentile observations must remain ordered");
  const sortedSamples = [...observation.samplesMs].sort((left, right) => left - right);
  requireCondition(observation.p50Ms === sortedSamples[Math.ceil(sortedSamples.length * 0.5) - 1] &&
    observation.p95Ms === sortedSamples[Math.ceil(sortedSamples.length * 0.95) - 1] &&
    observation.maxMs === sortedSamples[sortedSamples.length - 1],
    "ratified_perf percentiles must be derived from the actual independently observed paint samples");
  return {
    assuranceId: "ratified_perf",
    binarySha256: receipt.provenance.binarySha256,
    sidecarSha256: receipt.provenance.sidecarSha256,
    metricKind: "painted_latency",
    observationLayer: "PAINTED_OUTPUT",
    measuresPaint: true,
    ownerVisibleAuthorization: true,
    ownerRatified: true,
    ratifiedBudgetId: observation.ratifiedBudgetId,
    ratificationReference: observation.ratificationReference,
    sampleCount: observation.sampleCount,
    p50Ms: observation.p50Ms,
    p95Ms: observation.p95Ms,
    maxMs: observation.maxMs,
  };
}

function generatedDesignContractSummary(
  path: string,
  sourceSha: string,
  repositoryRoot: string,
): NonNullable<GateReceipt["result"]> {
  const receipt = readJson(path);
  const validation = validateGeneratedByteCompareReceipt(receipt, {
    currentSourceSha: sourceSha,
    currentFileSha256(sourcePath) {
      try {
        return sha256File(resolve(repositoryRoot, sourcePath));
      } catch {
        return null;
      }
    },
  });
  requireCondition(validation.pass,
    "generated-design-contracts requires exact-source non-GUI exporter byte equality: " +
      validation.errors.join("; "));

  const canonicalSourceFingerprints = Object.fromEntries(
    [...GENERATED_BYTE_COMPARE_SOURCE_PATHS]
      .sort()
      .map((owner) => [owner, receipt.sourceFingerprints[owner]]),
  );
  return {
    metricKind: "generated_design_byte_equality",
    exporterSha256: receipt.binary.sha256,
    exporterSourceFingerprintSha256: createHash("sha256")
      .update(JSON.stringify(canonicalSourceFingerprints), "utf8")
      .digest("hex"),
    rawEvidenceSha256: sha256File(path),
    designTokensJsonSha256: receipt.outputHashes[GENERATED_BYTE_COMPARE_OUTPUT_PATHS[0]],
    designTokensCssSha256: receipt.outputHashes[GENERATED_BYTE_COMPARE_OUTPUT_PATHS[1]],
    outputCount: GENERATED_BYTE_COMPARE_OUTPUT_PATHS.length,
    startsApplication: false,
  };
}

function verifyTransportedGeneratedDesignProof(options: {
  proofPath: string;
  sourceSha: string;
  repositoryRoot: string;
  designTokensPath: string;
  designCssPath: string;
  gateResult: NonNullable<GateReceipt["result"]>;
}): void {
  requireCondition(existsSync(options.proofPath),
    "authoritative generated-design raw exporter evidence is missing");
  requireCondition(sha256File(options.proofPath) === options.gateResult.rawEvidenceSha256,
    "authoritative generated-design raw exporter evidence was modified after execution");
  const receipt = readJson(options.proofPath);
  const validation = validateGeneratedByteCompareReceipt(receipt, {
    currentSourceSha: options.sourceSha,
  });
  requireCondition(validation.pass,
    "authoritative generated-design raw exporter evidence is invalid: " +
      validation.errors.join("; "));

  for (const sourcePath of GENERATED_BYTE_COMPARE_SOURCE_PATHS) {
    requireCondition(sha256File(resolve(options.repositoryRoot, sourcePath)) ===
      receipt.sourceFingerprints[sourcePath],
    `authoritative generated-design source changed after exporter proof: ${sourcePath}`);
  }
  const currentOutputHashes = [
    sha256File(options.designTokensPath),
    sha256File(options.designCssPath),
  ];
  for (const [index, outputPath] of GENERATED_BYTE_COMPARE_OUTPUT_PATHS.entries()) {
    requireCondition(currentOutputHashes[index] === receipt.outputHashes[outputPath],
      `authoritative generated-design checked-in output changed after proof: ${outputPath}`);
  }
  const canonicalSourceFingerprints = Object.fromEntries(
    [...GENERATED_BYTE_COMPARE_SOURCE_PATHS]
      .sort()
      .map((sourcePath) => [sourcePath, receipt.sourceFingerprints[sourcePath]]),
  );
  const fingerprintSha = createHash("sha256")
    .update(JSON.stringify(canonicalSourceFingerprints), "utf8")
    .digest("hex");
  requireCondition(receipt.binary.sha256 === options.gateResult.exporterSha256 &&
    fingerprintSha === options.gateResult.exporterSourceFingerprintSha256 &&
    currentOutputHashes[0] === options.gateResult.designTokensJsonSha256 &&
    currentOutputHashes[1] === options.gateResult.designTokensCssSha256,
    "authoritative generated-design raw proof does not match its immutable release gate");
}

function requireValidGateResult(receipt: GateReceipt): void {
  const result = receipt.result;

  switch (receipt.gateId) {
    case "rust-tests":
    case "domain-tests":
      requireCondition(Number.isInteger(result?.passed) && result!.passed! > 0 &&
        result.failed === 0 && Number.isInteger(result.skipped) && result.skipped! >= 0,
        `${receipt.gateId} requires a nonzero, passing executed Rust behavior summary`);
      return;
    case "integration-tests": {
      const expectedSuites = [...RELEASE_INTEGRATION_SUITES].sort();
      requireCondition(Number.isInteger(result?.passed) && result!.passed! > 0 &&
        result.failed === 0 && Number.isInteger(result.skipped) && result.skipped! >= 0 &&
        result.suites === expectedSuites.length &&
        Array.isArray(result.suiteNames) &&
        result.suiteNames.length === expectedSuites.length &&
        result.suiteNames.every((suite, index) => suite === expectedSuites[index]),
        "integration-tests requires every exact nonintrusive suite and nonzero passing behavior");
      return;
    }
    case "first-run-fixtures":
    case "permissions-fixtures":
    case "mock-ai-fixtures":
    case "privacy-fixtures":
      requireCondition(result?.passed === 1 && result.failed === 0 && result.skipped === 0,
        `${receipt.gateId} requires exactly one passing, unskipped focused behavior fixture`);
      return;
    case "proof-contracts":
      requireCondition(Number.isInteger(result?.passed) && result!.passed! > 0 &&
        result.failed === 0 && result.skipped === 0 &&
        Number.isInteger(result.files) && result.files! >= REQUIRED_PROOF_SUITES.length &&
        Number.isInteger(result.assertions) && result.assertions! > 0,
        "proof-contracts requires nonzero passing tests, assertions, and required fixture files");
      return;
    case "generated-design-contracts":
      requireCondition(result?.metricKind === "generated_design_byte_equality" &&
        /^[a-f0-9]{64}$/.test(String(result.exporterSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.exporterSourceFingerprintSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.rawEvidenceSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.designTokensJsonSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.designTokensCssSha256)) &&
        result.outputCount === GENERATED_BYTE_COMPARE_OUTPUT_PATHS.length &&
        result.startsApplication === false,
      "generated-design-contracts requires both exactly matched token outputs and a non-GUI exporter identity");
      return;
    case "sdk-tests":
      requireCondition(Number.isInteger(result?.passed) && result!.passed! > 0 &&
        result.failed === 0 && result.skipped === 0,
        "sdk-tests requires nonzero passing behavior with zero failures or skipped tests");
      return;
    case "consistency-catalog":
      requireCondition(result === undefined,
        "consistency-catalog is a static inventory and cannot contain invented behavior results");
      return;
    case "packaged-signing":
      requireCondition(/^[a-f0-9]{64}$/.test(String(result?.binarySha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.sidecarSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.notarizedArchiveSha256)) &&
        /^[A-Z0-9]{10}$/.test(String(result.teamIdentifier)) &&
        /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i
          .test(String(result.notarizationSubmissionId)) &&
        result.hardenedRuntime === true && result.stapled === true &&
        result.gatekeeperAccepted === true,
        "packaged-signing requires complete exact accepted Apple distribution security evidence");
      return;
    case "packaged-root-frame":
      requireCondition(/^[a-f0-9]{64}$/.test(String(result?.binarySha256)) &&
        result.metricKind === "semantic_frame_identity",
        "packaged-root-frame requires exact hidden semantic binary behavior evidence");
      return;
    case "packaged-direct-matrix":
      requireCondition(result?.assuranceId === "direct_matrix" &&
        result.metricKind === "direct_surface_matrix" &&
        result.observationLayer === "RUNTIME_HIDDEN" && result.measuresPaint === false &&
        /^[a-f0-9]{64}$/.test(String(result.binarySha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.sidecarSha256)) &&
        typeof result.transactionId === "string" && result.transactionId.length > 0 &&
        /^[a-f0-9]{64}$/.test(String(result.surfaceContractSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.bindingFingerprintSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.rawEvidenceSha256)) &&
        Number.isInteger(result.primitiveReceiptCount) && result.primitiveReceiptCount! > 0 &&
        result.expectedMappings === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT &&
        result.directProvenMappings === result.expectedMappings &&
        Array.isArray(result.bindingIds) &&
        result.bindingIds.length === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT &&
        new Set(result.bindingIds).size === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT,
        "direct_matrix requires complete direct hidden target coverage and exact candidate identities");
      return;
    case "packaged-ratified-performance":
      requireCondition(result?.assuranceId === "ratified_perf" &&
        result.metricKind === "painted_latency" &&
        result.observationLayer === "PAINTED_OUTPUT" && result.measuresPaint === true &&
        result.ownerVisibleAuthorization === true && result.ownerRatified === true &&
        /^[a-f0-9]{64}$/.test(String(result.binarySha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.sidecarSha256)) &&
        typeof result.ratifiedBudgetId === "string" && result.ratifiedBudgetId.length > 0 &&
        typeof result.ratificationReference === "string" &&
        result.ratificationReference.length > 0 &&
        Number.isInteger(result.sampleCount) && result.sampleCount! > 0 &&
        typeof result.p50Ms === "number" && result.p50Ms > 0 &&
        typeof result.p95Ms === "number" && result.p95Ms >= result.p50Ms &&
        typeof result.maxMs === "number" && result.maxMs >= result.p95Ms,
        "ratified_perf requires owner-authorized visible painted samples and an owner-ratified budget");
      return;
    case "packaged-first-install":
    case "packaged-permissions":
    case "packaged-migration":
    case "packaged-mock-ai":
      requireCondition(result?.journeyId === receipt.gateId &&
        result.metricKind === "packaged_app_journey" &&
        /^[a-f0-9]{64}$/.test(String(result.binarySha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.sidecarSha256)) &&
        /^[a-f0-9]{64}$/.test(String(result.fixtureSha256)),
        `${receipt.gateId} requires its direct exact packaged journey and immutable identities`);
      return;
  }
}

function canonicalGateReceiptSha256(receipt: GateReceipt): string {
  return createHash("sha256")
    .update(`${JSON.stringify(receipt, null, 2)}\n`, "utf8")
    .digest("hex");
}

interface GateSourceProvenance {
  sourceState: "clean" | "dirty";
  publishable: boolean;
  fingerprintScope?: "DECLARED_OWNERS_NON_EXHAUSTIVE";
  worktreeFingerprintSha256?: string;
  worktreeOwners?: Array<{ path: string; sha256: string }>;
}

function gitSourceIdentity(repositoryRoot: string, args: string[]) {
  return spawnSync("git", ["-C", repositoryRoot, ...args], {
    encoding: "utf8",
    maxBuffer: 128 * 1024,
  });
}

function requiredCleanReleaseSourceOwners(repositoryRoot: string): string[] {
  const verifier = readFileSync(resolve(repositoryRoot, "scripts/verify.sh"), "utf8");
  const array = verifier.match(/local -a required_source_owners=\(\r?\n([\s\S]*?)\r?\n\s*\)/);
  requireCondition(array?.[1],
    "publishable release evidence is missing the canonical bounded source-owner inventory");
  const owners = array[1].split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  requireCondition(owners.length > 0 && new Set(owners).size === owners.length &&
    owners.every((owner) => /^[A-Za-z0-9_./-]+$/.test(owner) &&
      !owner.startsWith("/") && !owner.split("/").includes("..")) &&
    owners.includes("scripts/verify.sh") &&
    owners.includes("scripts/release-evidence.ts") &&
    REQUIRED_PROOF_SUITES.every((suite) => owners.includes(suite)) &&
    REQUIRED_OPERATOR_SAFETY_OWNERS.every((owner) => owners.includes(owner)) &&
    REQUIRED_SDK_SAFETY_OWNERS.every((owner) => owners.includes(owner)) &&
    REQUIRED_BUILD_SAFETY_OWNERS.every((owner) => owners.includes(owner)) &&
    RELEASE_INTEGRATION_SUITES.every((suite) => owners.includes(`tests/${suite}.rs`)),
  "publishable release evidence has an invalid or incomplete canonical source-owner inventory");
  return owners;
}

function inspectGateSourceProvenance(options: {
  sourceSha: string;
  gateId: string;
  diagnosticDirty: boolean;
  ownerPaths: string[];
  repositoryRoot?: string;
}): GateSourceProvenance {
  const repositoryRoot = resolve(options.repositoryRoot ?? process.cwd());
  const head = gitSourceIdentity(repositoryRoot, ["rev-parse", "HEAD"]);
  requireCondition(head.status === 0 && head.stdout.trim() === options.sourceSha,
    "release gate source SHA does not match the current checkout HEAD");
  const diff = gitSourceIdentity(repositoryRoot, ["diff", "--quiet", "HEAD", "--"]);
  requireCondition(diff.status === 0 || diff.status === 1,
    "release gate could not verify the tracked worktree source state");

  if (diff.status === 0) {
    requireCondition(!options.diagnosticDirty && options.ownerPaths.length === 0,
      "dirty-source diagnostic evidence requires a genuinely dirty tracked worktree");
    const requiredOwners = requiredCleanReleaseSourceOwners(repositoryRoot);
    const tracked = gitSourceIdentity(repositoryRoot, [
      "ls-files",
      "--error-unmatch",
      ...requiredOwners,
    ]);
    requireCondition(tracked.status === 0 &&
      requiredOwners.every((path) =>
        existsSync(resolve(repositoryRoot, path))),
    "publishable release evidence requires every mandatory source owner to be tracked");
    return { sourceState: "clean", publishable: true };
  }

  requireCondition(options.diagnosticDirty,
    "dirty tracked source cannot produce publishable release evidence; explicitly request --diagnostic-dirty");
  requireCondition(options.ownerPaths.length > 0,
    "dirty diagnostic evidence requires at least one explicitly reviewed --owner path");

  const canonicalRoot = realpathSync(repositoryRoot);
  const owners = [...new Set(options.ownerPaths)].sort().map((path) => {
    requireCondition(!isAbsolute(path) && !path.includes("\\") &&
      !path.split("/").some((part) => part === "" || part === "." || part === ".." ||
        part.startsWith(".")) &&
      /^(?:src\/|crates\/|scripts\/|tests\/|kit-init\/|docs\/ai\/contracts\/|design\/mockups\/generated\/|Cargo\.toml$|Cargo\.lock$)/
        .test(path) &&
      /\.(?:rs|ts|tsx|js|json|css|md|toml|lock|sh)$/.test(path) &&
      !/(?:secret|credential|private[_-]?key|\.pem$|\.key$|\.env)/i.test(path),
    `dirty diagnostic owner is not an approved non-sensitive source path: ${path}`);
    const absolutePath = resolve(repositoryRoot, path);
    const canonicalPath = realpathSync(absolutePath);
    const containment = relative(canonicalRoot, canonicalPath);
    requireCondition(containment.length > 0 && containment !== ".." &&
      !containment.startsWith("../") && !isAbsolute(containment) &&
      statSync(canonicalPath).isFile(),
    `dirty diagnostic owner must be a repository-contained regular source file: ${path}`);
    return { path, sha256: sha256File(canonicalPath) };
  });
  const payload = {
    sourceSha: options.sourceSha,
    gateId: options.gateId,
    fingerprintScope: "DECLARED_OWNERS_NON_EXHAUSTIVE" as const,
    owners,
  };
  return {
    sourceState: "dirty",
    publishable: false,
    fingerprintScope: payload.fingerprintScope,
    worktreeFingerprintSha256: createHash("sha256")
      .update(JSON.stringify(payload), "utf8")
      .digest("hex"),
    worktreeOwners: owners,
  };
}

function requireValidGateSourceProvenance(receipt: GateReceipt): void {
  if (receipt.sourceState === "clean") {
    requireCondition(receipt.publishable === true &&
      receipt.fingerprintScope === undefined &&
      receipt.worktreeFingerprintSha256 === undefined &&
      receipt.worktreeOwners === undefined,
    "clean release evidence must be publishable and cannot contain dirty-worktree provenance");
    return;
  }
  requireCondition(receipt.sourceState === "dirty" && receipt.publishable === false &&
    receipt.fingerprintScope === "DECLARED_OWNERS_NON_EXHAUSTIVE" &&
    /^[a-f0-9]{64}$/.test(String(receipt.worktreeFingerprintSha256)) &&
    Array.isArray(receipt.worktreeOwners) && receipt.worktreeOwners.length > 0 &&
    receipt.worktreeOwners.every((owner) => typeof owner.path === "string" &&
      owner.path.length > 0 && /^[a-f0-9]{64}$/.test(owner.sha256)),
  "dirty diagnostic evidence must remain nonpublishable and bind explicit non-exhaustive owner fingerprints");
  const expectedFingerprint = createHash("sha256")
    .update(JSON.stringify({
      sourceSha: receipt.sourceSha,
      gateId: receipt.gateId,
      fingerprintScope: receipt.fingerprintScope,
      owners: receipt.worktreeOwners,
    }), "utf8")
    .digest("hex");
  requireCondition(receipt.worktreeFingerprintSha256 === expectedFingerprint,
    "dirty diagnostic worktree fingerprint does not match its declared exact owners");
}

export function buildGateReceipt(options: {
  gateId: string;
  evidenceClass: string;
  sourceSha: string;
  resultPath?: string;
  repositoryRoot?: string;
  sourceProvenance?: GateSourceProvenance;
}): GateReceipt {
  requireGateId(options.gateId);
  requireSourceSha(options.sourceSha);
  const expectedClass = REQUIRED_GATE_CLASSES[options.gateId];
  requireCondition(options.evidenceClass === expectedClass,
    `${options.gateId} requires ${expectedClass}, received ${options.evidenceClass}`);

  const receipt: GateReceipt = {
    schemaVersion: RELEASE_EVIDENCE_SCHEMA_VERSION,
    gateId: options.gateId,
    evidenceClass: expectedClass,
    status: "pass",
    sourceSha: options.sourceSha,
    ...(options.sourceProvenance ?? { sourceState: "clean", publishable: true }),
    noninteractive: true,
    observedAt: new Date().toISOString(),
  };

  if (options.gateId === "rust-tests" || options.gateId === "domain-tests") {
    requireCondition(options.resultPath,
      `${options.gateId} requires real executed Rust test-result summaries`);
    receipt.result = rustSuiteSummary(options.resultPath);
  } else if (options.gateId === "integration-tests") {
    requireCondition(options.resultPath,
      "integration-tests requires directly executed named integration-suite summaries");
    receipt.result = integrationSuiteSummary(options.resultPath);
  } else if (options.gateId === "first-run-fixtures" ||
    options.gateId === "permissions-fixtures" || options.gateId === "mock-ai-fixtures" ||
    options.gateId === "privacy-fixtures") {
    requireCondition(options.resultPath,
      `${options.gateId} requires its exact executed behavior-fixture summary`);
    receipt.result = focusedRustSuiteSummary(
      options.resultPath,
      REQUIRED_FOCUSED_FIXTURES[options.gateId],
    );
  } else if (options.gateId === "proof-contracts") {
    requireCondition(options.resultPath,
      "proof-contracts requires complete executed Bun behavior-test summaries");
    receipt.result = proofSuiteSummary(options.resultPath);
  } else if (options.gateId === "generated-design-contracts") {
    requireCondition(options.resultPath,
      "generated-design-contracts requires an actually executed exporter byte-comparison receipt");
    receipt.result = generatedDesignContractSummary(
      options.resultPath,
      options.sourceSha,
      resolve(options.repositoryRoot ?? process.cwd()),
    );
  } else if (options.gateId === "sdk-tests") {
    requireCondition(options.resultPath, "SDK gate requires its complete machine-readable suite result");
    receipt.result = sdkSuiteSummary(options.resultPath);
  } else if (options.gateId === "packaged-signing") {
    requireCondition(options.resultPath,
      "packaged signing requires a fresh Apple distribution-security attestation");
    const result = readJson(options.resultPath);
    requireCondition(result.sourceSha === options.sourceSha,
      "packaged signing attestation belongs to another source revision");
    requireCondition(/^[a-f0-9]{64}$/.test(String(result.binarySha256)) &&
      /^[a-f0-9]{64}$/.test(String(result.sidecarSha256)) &&
      /^[a-f0-9]{64}$/.test(String(result.notarizedArchiveSha256)),
      "packaged signing attestation requires exact executable, sidecar, and notarized archive identities");
    requireCondition(/^[A-Z0-9]{10}$/.test(String(result.teamIdentifier)),
      "packaged signing attestation requires the exact Apple team identity");
    requireCondition(typeof result.notarizationSubmissionId === "string" &&
      /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i
        .test(result.notarizationSubmissionId),
      "packaged signing attestation requires an accepted notarization submission identity");
    requireCondition(result.hardenedRuntime === true && result.stapled === true &&
      result.gatekeeperAccepted === true,
      "packaged signing attestation requires hardened runtime, a valid staple, and Gatekeeper acceptance");
    receipt.result = {
      binarySha256: result.binarySha256,
      sidecarSha256: result.sidecarSha256,
      notarizedArchiveSha256: result.notarizedArchiveSha256,
      teamIdentifier: result.teamIdentifier,
      notarizationSubmissionId: result.notarizationSubmissionId,
      hardenedRuntime: true,
      stapled: true,
      gatekeeperAccepted: true,
    };
  } else if (options.gateId === "packaged-direct-matrix" ||
    options.gateId === "packaged-ratified-performance") {
    requireCondition(options.resultPath,
      `${options.gateId} requires its own exact-source signed-candidate assurance receipt`);
    receipt.result = packagedAssuranceSummary(
      options.resultPath,
      options.gateId,
      options.sourceSha,
      resolve(options.repositoryRoot ?? process.cwd()),
    );
  } else if (REQUIRED_PACKAGED_JOURNEYS.includes(options.gateId as PackagedJourneyId)) {
    requireCondition(options.resultPath,
      `${options.gateId} requires its own direct same-binary packaged-app journey receipt`);
    receipt.result = packagedJourneySummary(
      options.resultPath,
      options.gateId as PackagedJourneyId,
      options.sourceSha,
    );
  } else if (options.gateId === "packaged-root-frame") {
    requireCondition(options.resultPath, "packaged root gate requires a direct runtime receipt");
    const result = readJson(options.resultPath);
    requireCondition(result.status === "pass", "packaged root gate did not pass");
    requireCondition(result.behavior?.status === "pass", "packaged root behavior did not pass");
    requireCondition(result.metricKind === "semantic_frame_identity",
      "packaged root gate must be semantic-frame evidence, never painted-output evidence");
    requireCondition(result.evidenceClass === "RUNTIME_HIDDEN" && result.measuresPaint === false,
      "packaged root gate must honestly report hidden semantic evidence");
    requireCondition(result.safety?.startsApplication === true &&
      result.safety?.isolatedCiLaunchAuthorized === true &&
      result.safety?.sandboxHome === true &&
      result.safety.windowRevealAllowed === false &&
      result.safety.windowFocusAllowed === false &&
      result.safety.nativeInputAllowed === false &&
      result.safety.screenCaptureAllowed === false &&
      Number.isInteger(result.safety.hiddenStateAssertionCount) &&
      result.safety.hiddenStateAssertionCount > 0,
      "packaged root gate lacks verified nonintrusive hidden-window safety");
    requireCondition(result.cleanup?.hidden === true && result.cleanup?.closed === true,
      "packaged root gate did not cleanly hide and terminate its owned process");
    requireCondition(result.artifactLifecycle?.allRequiredValid === true &&
      result.artifactLifecycle?.allRecordedPathsReadable === true,
      "packaged root gate is missing its finalized direct-proof artifacts");
    requireCondition(result.provenance?.gitSha === options.sourceSha,
      "packaged root receipt belongs to another source revision");
    requireCondition(typeof result.provenance?.binarySha256 === "string" &&
      /^[a-f0-9]{64}$/.test(result.provenance.binarySha256),
      "packaged root receipt is missing its exact binary SHA-256");
    receipt.result = {
      binarySha256: result.provenance.binarySha256,
      metricKind: result.metricKind,
    };
  } else {
    requireCondition(options.resultPath === undefined,
      `${options.gateId} does not accept unrelated runtime results`);
  }

  requireValidGateSourceProvenance(receipt);
  requireValidGateResult(receipt);
  return receipt;
}

function readGateReceipts(paths: string[], sourceSha: string): Array<GateReceipt & { sha256: string }> {
  const receipts = paths.map((path) => {
    const receipt = readJson(path) as unknown as GateReceipt;
    requireGateId(receipt.gateId);
    requireCondition(receipt.schemaVersion === RELEASE_EVIDENCE_SCHEMA_VERSION,
      `unsupported gate evidence schema: ${path}`);
    requireCondition(receipt.evidenceClass === REQUIRED_GATE_CLASSES[receipt.gateId],
      `wrong evidence class for ${receipt.gateId}: ${receipt.evidenceClass}`);
    requireCondition(receipt.status === "pass", `${receipt.gateId} did not pass`);
    requireCondition(receipt.sourceSha === sourceSha,
      `${receipt.gateId} belongs to another source revision`);
    requireValidGateSourceProvenance(receipt);
    requireCondition(receipt.sourceState === "clean" && receipt.publishable === true,
      `${receipt.gateId} is nonpublishable dirty-worktree diagnostic evidence`);
    requireCondition(receipt.noninteractive === true,
      `${receipt.gateId} is missing its nonintrusive verification contract`);
    requireValidGateResult(receipt);
    const sha256 = sha256File(path);
    requireCondition(sha256 === canonicalGateReceiptSha256(receipt),
      `${receipt.gateId} is not an immutable canonically encoded release receipt`);
    return { ...receipt, sha256 };
  });

  const ids = new Set<string>();
  for (const receipt of receipts) {
    requireCondition(!ids.has(receipt.gateId), `duplicate release gate: ${receipt.gateId}`);
    ids.add(receipt.gateId);
  }
  for (const gateId of Object.keys(REQUIRED_GATE_CLASSES)) {
    requireCondition(ids.has(gateId), `missing required release gate: ${gateId}`);
  }

  return receipts.sort((left, right) => left.gateId.localeCompare(right.gateId));
}

export function buildReleaseManifest(options: ManifestOptions): ReleaseManifest {
  requireSourceSha(options.sourceSha);
  requireCondition(options.tag === `v${options.version}`,
    `release tag ${options.tag} does not match version ${options.version}`);

  const executablePath = join(options.appPath, "Contents/MacOS/script-kit-gpui");
  const sidecarPath = join(options.appPath, "Contents/MacOS/pi");
  const sdkPath = join(options.appPath, "Contents/Resources/scripts/kit-sdk.ts");
  const infoPlistPath = join(options.appPath, "Contents/Info.plist");
  verifyExecutable(executablePath);
  verifyExecutable(sidecarPath);

  const executableSha = sha256File(executablePath);
  const gates = readGateReceipts(options.evidencePaths, options.sourceSha);
  const generatedDesignGate = gates.find((gate) => gate.gateId === "generated-design-contracts");
  const designTokensSha = sha256File(options.designTokensPath);
  const designCssSha = sha256File(options.designCssPath);
  requireCondition(generatedDesignGate?.result?.designTokensJsonSha256 === designTokensSha &&
    generatedDesignGate.result.designTokensCssSha256 === designCssSha,
    "generated design-token JSON/CSS differ from the exact exporter-verified candidate outputs");
  verifyTransportedGeneratedDesignProof({
    proofPath: options.designProofPath,
    sourceSha: options.sourceSha,
    repositoryRoot: resolve(options.repositoryRoot ?? process.cwd()),
    designTokensPath: options.designTokensPath,
    designCssPath: options.designCssPath,
    gateResult: generatedDesignGate.result,
  });
  const runtimeGate = gates.find((gate) => gate.gateId === "packaged-root-frame");
  requireCondition(runtimeGate?.result?.binarySha256 === executableSha,
    "packaged runtime evidence was produced by a different application binary");
  const signingGate = gates.find((gate) => gate.gateId === "packaged-signing");
  requireCondition(signingGate?.result?.binarySha256 === executableSha &&
    signingGate.result.sidecarSha256 === sha256File(sidecarPath),
    "packaged signing attestation belongs to a different application executable or Pi sidecar");
  requireCondition(signingGate.result.hardenedRuntime === true &&
    signingGate.result.stapled === true && signingGate.result.gatekeeperAccepted === true,
    "packaged signing attestation does not prove every distribution-security control");
  for (const journeyId of REQUIRED_PACKAGED_JOURNEYS) {
    const journey = gates.find((gate) => gate.gateId === journeyId);
    requireCondition(journey?.result?.journeyId === journeyId &&
      journey.result.binarySha256 === executableSha &&
      journey.result.sidecarSha256 === signingGate.result.sidecarSha256 &&
      /^[a-f0-9]{64}$/.test(String(journey.result.fixtureSha256)),
      `${journeyId} does not belong to the exact signed application, Pi sidecar, and isolated fixture`);
  }
  for (const assurance of REQUIRED_PACKAGED_ASSURANCES) {
    const gate = gates.find((candidate) => candidate.gateId === assurance.gateId);
    requireCondition(gate?.result?.assuranceId === assurance.id &&
      gate.result.binarySha256 === executableSha &&
      gate.result.sidecarSha256 === signingGate.result.sidecarSha256,
      `${assurance.id} does not belong to the exact signed application and Pi sidecar`);
    if (assurance.id === "direct_matrix") {
      requireCondition(gate.result.surfaceContractSha256 === sha256File(options.contractsPath),
        "direct_matrix does not match the exact published surface-contract schema identity");
    }
  }

  const contracts = readJson(options.contractsPath);
  requireCondition(Number.isInteger(contracts.schemaVersion) && contracts.schemaVersion > 0,
    "surface contracts do not contain a valid schema version");
  requireCondition(Array.isArray(contracts.entries) && contracts.entries.length > 0,
    "surface contracts contain no registered surfaces");
  const canonicalBindingIds = contracts.entries.flatMap((entry: Record<string, unknown>) => {
    requireCondition(typeof entry.surfaceKind === "string" &&
      Array.isArray(entry.appViewVariants),
      "surface contracts must expose exact kind-to-AppView mappings");
    const host = entry.surfaceKind === "ActionsDialog" ? "ActionsDialog" : "MainWindow";
    return entry.appViewVariants.map((variant: unknown) => {
      requireCondition(typeof variant === "string" && variant.length > 0,
        "surface contracts contain an invalid canonical AppView variant");
      return `${entry.surfaceKind}::${variant}@${host}`;
    });
  }).sort();
  const directGate = gates.find((gate) => gate.gateId === "packaged-direct-matrix");
  requireCondition(canonicalBindingIds.length === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT &&
    new Set(canonicalBindingIds).size === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT &&
    directGate?.result?.bindingIds?.length === canonicalBindingIds.length &&
    directGate.result.bindingIds.every((id, index) => id === canonicalBindingIds[index]),
    "direct_matrix target IDs do not exactly match all canonical generated-contract bindings");

  const zipMetadata = statSync(options.zipPath);
  requireCondition(zipMetadata.isFile() && zipMetadata.size > 0,
    `release archive is missing or empty: ${options.zipPath}`);
  const archived = releaseArchiveMembers(options.zipPath);
  const archivedSha256 = (bytes: Buffer) => createHash("sha256").update(bytes).digest("hex");
  requireCondition(archivedSha256(archived.executable) === executableSha,
    "release ZIP executable does not match the exact verified application binary");
  requireCondition(archivedSha256(archived.sidecar) === sha256File(sidecarPath),
    "release ZIP Pi sidecar does not match the exact verified application");
  requireCondition(archivedSha256(archived.sdk) === sha256File(sdkPath),
    "release ZIP SDK does not match the exact verified application");
  requireCondition(archivedSha256(archived.infoPlist) === sha256File(infoPlistPath),
    "release ZIP Info.plist does not match the exact verified application");
  const signedApplicationTree = applicationContentTree(options.appPath);
  requireCondition(archived.contentTree.entryCount === signedApplicationTree.entryCount &&
    archived.contentTree.sha256 === signedApplicationTree.sha256,
  "release ZIP complete signed application tree does not match the verified app");

  return {
    schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
    version: options.version,
    tag: options.tag,
    source_sha: options.sourceSha,
    bundle: {
      identifier: bundleIdentifier(infoPlistPath),
      info_plist: { path: "Contents/Info.plist", sha256: sha256File(infoPlistPath) },
      content_tree: {
        algorithm: signedApplicationTree.algorithm,
        sha256: signedApplicationTree.sha256,
        entry_count: signedApplicationTree.entryCount,
      },
      executable: { path: "Contents/MacOS/script-kit-gpui", sha256: executableSha },
      sidecar: { path: "Contents/MacOS/pi", sha256: sha256File(sidecarPath) },
    },
    sdk: {
      version: sdkVersion(sdkPath),
      path: "Contents/Resources/scripts/kit-sdk.ts",
      sha256: sha256File(sdkPath),
    },
    surface_contracts: {
      schema_version: contracts.schemaVersion,
      path: "docs/ai/contracts/surface-contracts.json",
      sha256: sha256File(options.contractsPath),
    },
    design_contracts: {
      exporter_sha256: generatedDesignGate.result.exporterSha256!,
      exporter_source_fingerprint_sha256:
        generatedDesignGate.result.exporterSourceFingerprintSha256!,
      raw_evidence: {
        path: "generated-design-contracts-proof.json",
        sha256: generatedDesignGate.result.rawEvidenceSha256!,
      },
      tokens_json: { path: GENERATED_BYTE_COMPARE_OUTPUT_PATHS[0], sha256: designTokensSha },
      tokens_css: { path: GENERATED_BYTE_COMPARE_OUTPUT_PATHS[1], sha256: designCssSha },
    },
    distribution_security: {
      team_identifier: signingGate.result.teamIdentifier!,
      notarization_submission_id: signingGate.result.notarizationSubmissionId!,
      notarized_archive_sha256: signingGate.result.notarizedArchiveSha256!,
      hardened_runtime: true,
      stapled: true,
      gatekeeper_accepted: true,
    },
    verification: {
      status: "pass",
      visibility: {
        journeys: "hidden_only",
        paintedOutput: "owner_authorized_visible",
      },
      gates,
    },
    artifacts: [{
      name: options.zipPath.split("/").pop()!,
      platform: "macos",
      sha256: sha256File(options.zipPath),
      size_bytes: zipMetadata.size,
    }],
  };
}

export function verifyReleaseManifest(options: {
  manifestPath: string;
  zipPath: string;
  sourceSha: string;
  tag: string;
  appPath?: string;
  designTokensPath: string;
  designCssPath: string;
  designProofPath: string;
  repositoryRoot?: string;
}): ReleaseManifest {
  const manifest = readJson(options.manifestPath) as unknown as ReleaseManifest;
  requireCondition(manifest.schema_version === RELEASE_MANIFEST_SCHEMA_VERSION,
    "release manifest has an unsupported schema");
  requireCondition(manifest.source_sha === options.sourceSha,
    "release manifest belongs to another source revision");
  requireCondition(manifest.tag === options.tag && manifest.tag === `v${manifest.version}`,
    "release manifest tag/version does not match the publication ref");
  requireCondition(manifest.verification?.status === "pass" &&
    manifest.verification.visibility?.journeys === "hidden_only" &&
    manifest.verification.visibility.paintedOutput === "owner_authorized_visible",
    "release manifest must separate hidden journeys from explicitly owner-authorized painted output");
  requireCondition(Array.isArray(manifest.verification.gates),
    "release manifest is missing gate receipts");

  const gateIds = new Set<string>();
  for (const gate of manifest.verification.gates) {
    requireGateId(gate.gateId);
    requireCondition(!gateIds.has(gate.gateId), `duplicate manifest gate: ${gate.gateId}`);
    gateIds.add(gate.gateId);
    requireCondition(gate.status === "pass" && gate.sourceSha === options.sourceSha &&
      gate.sourceState === "clean" && gate.publishable === true &&
      gate.schemaVersion === RELEASE_EVIDENCE_SCHEMA_VERSION &&
      gate.evidenceClass === REQUIRED_GATE_CLASSES[gate.gateId] &&
      gate.noninteractive === true && /^[a-f0-9]{64}$/.test(gate.sha256),
      `manifest contains invalid evidence for ${gate.gateId}`);
    requireValidGateSourceProvenance(gate);
    requireValidGateResult(gate);
    const { sha256, ...originalReceipt } = gate;
    requireCondition(canonicalGateReceiptSha256(originalReceipt) === sha256,
      `manifest evidence was modified after execution: ${gate.gateId}`);
  }
  for (const gateId of Object.keys(REQUIRED_GATE_CLASSES)) {
    requireCondition(gateIds.has(gateId), `manifest is missing required release gate: ${gateId}`);
  }
  const generatedDesignGate = manifest.verification.gates.find((gate) =>
    gate.gateId === "generated-design-contracts");
  requireCondition(generatedDesignGate?.result?.exporterSha256 ===
      manifest.design_contracts?.exporter_sha256 &&
    generatedDesignGate.result.exporterSourceFingerprintSha256 ===
      manifest.design_contracts.exporter_source_fingerprint_sha256 &&
    generatedDesignGate.result.designTokensJsonSha256 ===
      manifest.design_contracts.tokens_json?.sha256 &&
    generatedDesignGate.result.designTokensCssSha256 ===
      manifest.design_contracts.tokens_css?.sha256 &&
    generatedDesignGate.result.rawEvidenceSha256 ===
      manifest.design_contracts.raw_evidence?.sha256 &&
    manifest.design_contracts.raw_evidence.path === "generated-design-contracts-proof.json" &&
    manifest.design_contracts.tokens_json.path === GENERATED_BYTE_COMPARE_OUTPUT_PATHS[0] &&
    manifest.design_contracts.tokens_css.path === GENERATED_BYTE_COMPARE_OUTPUT_PATHS[1],
    "release manifest is missing exact exporter-verified design-token JSON/CSS/raw-proof identities");
  verifyTransportedGeneratedDesignProof({
    proofPath: options.designProofPath,
    sourceSha: options.sourceSha,
    repositoryRoot: resolve(options.repositoryRoot ?? process.cwd()),
    designTokensPath: options.designTokensPath,
    designCssPath: options.designCssPath,
    gateResult: generatedDesignGate.result,
  });

  requireCondition(manifest.artifacts?.length === 1,
    "release manifest must identify exactly one macOS archive");
  const archive = manifest.artifacts[0];
  requireCondition(archive.platform === "macos", "release artifact has the wrong platform");
  requireCondition(archive.name === options.zipPath.split("/").pop(),
    "release artifact filename does not match the publication archive");
  requireCondition(archive.sha256 === sha256File(options.zipPath),
    "release archive SHA-256 does not match the manifest");
  requireCondition(archive.size_bytes === statSync(options.zipPath).size,
    "release archive size does not match the manifest");
  requireCondition(manifest.bundle?.info_plist?.path === "Contents/Info.plist" &&
    /^[a-f0-9]{64}$/.test(String(manifest.bundle.info_plist.sha256)),
  "release manifest is missing the exact archived Info.plist identity");
  requireCondition(manifest.bundle?.content_tree?.algorithm === "SHA256_CANONICAL_APP_TREE_V1" &&
    /^[a-f0-9]{64}$/.test(String(manifest.bundle.content_tree.sha256)) &&
    Number.isSafeInteger(manifest.bundle.content_tree.entry_count) &&
    manifest.bundle.content_tree.entry_count > 0,
  "release manifest is missing the exact signed application content-tree identity");
  const archived = releaseArchiveMembers(options.zipPath);
  const archivedSha256 = (bytes: Buffer) => createHash("sha256").update(bytes).digest("hex");
  requireCondition(archivedSha256(archived.executable) === manifest.bundle.executable.sha256,
    "release ZIP executable does not match the verified manifest binary");
  requireCondition(archivedSha256(archived.sidecar) === manifest.bundle.sidecar.sha256,
    "release ZIP Pi sidecar does not match the verified manifest");
  requireCondition(archivedSha256(archived.sdk) === manifest.sdk.sha256,
    "release ZIP SDK does not match the verified manifest");
  requireCondition(archivedSha256(archived.infoPlist) === manifest.bundle.info_plist.sha256,
    "release ZIP Info.plist does not match the verified manifest");
  requireCondition(archived.contentTree.entryCount === manifest.bundle.content_tree.entry_count &&
    archived.contentTree.sha256 === manifest.bundle.content_tree.sha256,
  "release ZIP complete signed application tree does not match the verified manifest");
  requireCondition(bundleIdentifierFromContents(
    archived.infoPlist.toString("utf8"),
    `${options.zipPath}!/Contents/Info.plist`,
  ) === manifest.bundle.identifier,
  "release ZIP Info.plist does not match the verified bundle identifier");

  const runtimeGate = manifest.verification.gates.find((gate) => gate.gateId === "packaged-root-frame");
  requireCondition(runtimeGate?.result?.binarySha256 === manifest.bundle?.executable?.sha256,
    "packaged runtime evidence does not match the manifest binary");
  const signingGate = manifest.verification.gates.find((gate) => gate.gateId === "packaged-signing");
  const security = manifest.distribution_security;
  requireCondition(signingGate?.result?.binarySha256 === manifest.bundle.executable.sha256 &&
    signingGate.result.sidecarSha256 === manifest.bundle.sidecar.sha256,
    "packaged signing evidence does not match the published application and Pi sidecar");
  for (const journeyId of REQUIRED_PACKAGED_JOURNEYS) {
    const journey = manifest.verification.gates.find((gate) => gate.gateId === journeyId);
    requireCondition(journey?.evidenceClass === "PACKAGED_APP" &&
      journey.result?.journeyId === journeyId &&
      journey.result.binarySha256 === manifest.bundle.executable.sha256 &&
      journey.result.sidecarSha256 === manifest.bundle.sidecar.sha256 &&
      /^[a-f0-9]{64}$/.test(String(journey.result.fixtureSha256)),
      `manifest is missing exact direct packaged-app evidence for ${journeyId}`);
  }
  for (const assurance of REQUIRED_PACKAGED_ASSURANCES) {
    const gate = manifest.verification.gates.find((candidate) =>
      candidate.gateId === assurance.gateId);
    requireCondition(gate?.evidenceClass === assurance.evidenceClass &&
      gate.result?.assuranceId === assurance.id &&
      gate.result.binarySha256 === manifest.bundle.executable.sha256 &&
      gate.result.sidecarSha256 === manifest.bundle.sidecar.sha256,
      `manifest is missing exact candidate ${assurance.id} evidence`);
    if (assurance.id === "direct_matrix") {
      requireCondition(gate.result.surfaceContractSha256 === manifest.surface_contracts.sha256,
        "manifest direct_matrix surface bindings belong to another generated contract");
    }
  }
  requireCondition(/^[A-Z0-9]{10}$/.test(String(security?.team_identifier)) &&
    /^[a-f0-9]{64}$/.test(String(security?.notarized_archive_sha256)) &&
    security?.team_identifier === signingGate.result.teamIdentifier &&
    security.notarization_submission_id === signingGate.result.notarizationSubmissionId &&
    security.notarized_archive_sha256 === signingGate.result.notarizedArchiveSha256 &&
    security.hardened_runtime === true && security.stapled === true &&
    security.gatekeeper_accepted === true && signingGate.result.hardenedRuntime === true &&
    signingGate.result.stapled === true && signingGate.result.gatekeeperAccepted === true,
    "release manifest is missing a matching accepted, stapled, hardened Gatekeeper attestation");
  requireCondition(typeof manifest.bundle.identifier === "string" && manifest.bundle.identifier.length > 0,
    "release manifest is missing its bundle identifier");
  requireCondition(/^[a-f0-9]{64}$/.test(manifest.bundle.sidecar.sha256),
    "release manifest is missing its Pi sidecar identity");
  requireCondition(typeof manifest.sdk?.version === "string" &&
    /^[a-f0-9]{64}$/.test(manifest.sdk.sha256),
    "release manifest is missing its SDK identity");
  requireCondition(Number.isInteger(manifest.surface_contracts?.schema_version) &&
    /^[a-f0-9]{64}$/.test(manifest.surface_contracts.sha256),
    "release manifest is missing its surface-contract identity");

  if (options.appPath) {
    requireCondition(manifest.bundle.info_plist.sha256 ===
      sha256File(join(options.appPath, manifest.bundle.info_plist.path)),
    "packaged Info.plist no longer matches the release manifest");
    requireCondition(manifest.bundle.executable.sha256 ===
      sha256File(join(options.appPath, manifest.bundle.executable.path)),
      "packaged executable no longer matches the release manifest");
    requireCondition(manifest.bundle.sidecar.sha256 ===
      sha256File(join(options.appPath, manifest.bundle.sidecar.path)),
      "packaged Pi sidecar no longer matches the release manifest");
    requireCondition(manifest.sdk.sha256 === sha256File(join(options.appPath, manifest.sdk.path)),
      "packaged SDK no longer matches the release manifest");
  }

  return manifest;
}

export function buildReleaseScorecard(manifest: ReleaseManifest): ReleaseScorecard {
  const runtime = manifest.verification.gates.find((gate) => gate.gateId === "packaged-root-frame");
  requireCondition(runtime?.result?.metricKind === "semantic_frame_identity",
    "release scorecard requires an exact packaged semantic-frame journey");
  const performance = manifest.verification.gates.find((gate) =>
    gate.gateId === "packaged-ratified-performance");
  requireCondition(performance?.evidenceClass === "RUNTIME_VISIBLE" &&
    performance.result?.assuranceId === "ratified_perf" &&
    performance.result.observationLayer === "PAINTED_OUTPUT" &&
    performance.result.measuresPaint === true &&
    performance.result.ownerVisibleAuthorization === true &&
    performance.result.ownerRatified === true &&
    performance.result.binarySha256 === manifest.bundle.executable.sha256 &&
    performance.result.sidecarSha256 === manifest.bundle.sidecar.sha256,
    "release scorecard requires explicitly authorized actual painted output from the signed candidate");
  const directMatrix = manifest.verification.gates.find((gate) =>
    gate.gateId === "packaged-direct-matrix");
  requireCondition(directMatrix?.result?.assuranceId === "direct_matrix" &&
    directMatrix.result.expectedMappings === REQUIRED_DIRECT_SURFACE_MAPPING_COUNT &&
    directMatrix.result.directProvenMappings === directMatrix.result.expectedMappings &&
    directMatrix.result.surfaceContractSha256 === manifest.surface_contracts.sha256,
    "release scorecard requires complete exact-contract hidden surface coverage");

  return {
    schemaVersion: 1,
    version: manifest.version,
    tag: manifest.tag,
    sourceSha: manifest.source_sha,
    bundleIdentifier: manifest.bundle.identifier,
    binarySha256: manifest.bundle.executable.sha256,
    sidecarSha256: manifest.bundle.sidecar.sha256,
    sdkVersion: manifest.sdk.version,
    surfaceContractSchemaVersion: manifest.surface_contracts.schema_version,
    designContracts: {
      exporterSha256: manifest.design_contracts.exporter_sha256,
      exporterSourceFingerprintSha256:
        manifest.design_contracts.exporter_source_fingerprint_sha256,
      rawEvidenceSha256: manifest.design_contracts.raw_evidence.sha256,
      tokensJsonSha256: manifest.design_contracts.tokens_json.sha256,
      tokensCssSha256: manifest.design_contracts.tokens_css.sha256,
    },
    archiveSha256: manifest.artifacts[0].sha256,
    distributionSecurity: {
      teamIdentifier: manifest.distribution_security.team_identifier,
      notarizationSubmissionId: manifest.distribution_security.notarization_submission_id,
      notarizedArchiveSha256: manifest.distribution_security.notarized_archive_sha256,
      hardenedRuntime: true,
      stapled: true,
      gatekeeperAccepted: true,
    },
    gates: manifest.verification.gates.map((gate) => ({
      gateId: gate.gateId,
      evidenceClass: gate.evidenceClass,
      status: gate.status,
      ...(gate.result?.passed === undefined ? {} : {
        passed: gate.result.passed,
        failed: gate.result.failed,
        skipped: gate.result.skipped,
      }),
      ...(gate.result?.suites === undefined ? {} : {
        suites: gate.result.suites,
        suiteNames: gate.result.suiteNames,
      }),
      ...(gate.result?.files === undefined ? {} : {
        files: gate.result.files,
        assertions: gate.result.assertions,
      }),
    })),
    journeys: [{
      id: "packaged-root-frame",
      evidenceClass: "RUNTIME_HIDDEN",
      status: "pass",
      metricKind: runtime.result.metricKind,
      measuresPaint: false,
      startsApplication: true,
      isolatedCiLaunchAuthorized: true,
      revealsWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
    }, ...REQUIRED_PACKAGED_JOURNEYS.map((id) => ({
      id,
      evidenceClass: "PACKAGED_APP" as const,
      status: "pass" as const,
      metricKind: "packaged_app_journey",
      measuresPaint: false as const,
      startsApplication: true as const,
      isolatedCiLaunchAuthorized: true as const,
      revealsWindow: false as const,
      drivesNativeInput: false as const,
      capturesScreen: false as const,
    }))],
    directSurfaceCoverage: {
      status: "pass",
      evidenceClass: "RUNTIME_HIDDEN",
      expectedMappings: directMatrix.result.expectedMappings!,
      directProvenMappings: directMatrix.result.directProvenMappings!,
      transactionId: directMatrix.result.transactionId!,
      surfaceContractSha256: directMatrix.result.surfaceContractSha256!,
    },
    paintedLatency: {
      status: "pass",
      evidenceClass: "RUNTIME_VISIBLE",
      metricKind: "PAINTED_OUTPUT",
      p50Ms: performance.result.p50Ms!,
      p95Ms: performance.result.p95Ms!,
      maxMs: performance.result.maxMs!,
      sampleCount: performance.result.sampleCount!,
      budgetRatified: true,
      ownerVisibleAuthorization: true,
      ratifiedBudgetId: performance.result.ratifiedBudgetId!,
      ratificationReference: performance.result.ratificationReference!,
    },
  };
}

export function assessPackagedJourneyReadiness(options: {
  appPath: string;
  evidenceDirectory: string;
  sourceSha: string;
}): {
  schemaVersion: 1;
  status: "pass" | "blocked";
  sourceSha: string;
  binarySha256: string;
  sidecarSha256: string;
  journeys: Array<{
    id: PackagedJourneyId;
    evidenceClass: "PACKAGED_APP";
    status: "pass" | "unmeasured" | "invalid";
    reason?: string;
  }>;
  assurances: Array<{
    id: PackagedAssuranceId;
    gateId: PackagedAssuranceGateId;
    evidenceClass: "RUNTIME_HIDDEN" | "RUNTIME_VISIBLE";
    status: "pass" | "unmeasured" | "invalid";
    reason?: string;
  }>;
  missingJourneys: PackagedJourneyId[];
  invalidJourneys: PackagedJourneyId[];
  missingAssurances: PackagedAssuranceId[];
  invalidAssurances: PackagedAssuranceId[];
} {
  requireSourceSha(options.sourceSha);
  const binarySha256 = sha256File(join(options.appPath, "Contents/MacOS/script-kit-gpui"));
  const sidecarSha256 = sha256File(join(options.appPath, "Contents/MacOS/pi"));
  const journeys = REQUIRED_PACKAGED_JOURNEYS.map((id) => {
    const path = join(options.evidenceDirectory, `${id}.json`);
    if (!existsSync(path)) {
      return {
        id,
        evidenceClass: "PACKAGED_APP" as const,
        status: "unmeasured" as const,
        reason: "No direct same-binary packaged journey receipt was produced.",
      };
    }
    try {
      const receipt = readJson(path);
      requireCondition(receipt.schemaVersion === RELEASE_EVIDENCE_SCHEMA_VERSION &&
        receipt.gateId === id && receipt.evidenceClass === "PACKAGED_APP" &&
        receipt.status === "pass" && receipt.noninteractive === true &&
        receipt.sourceState === "clean" && receipt.publishable === true &&
        receipt.sourceSha === options.sourceSha && receipt.result?.journeyId === id &&
        receipt.result.binarySha256 === binarySha256 &&
        receipt.result.sidecarSha256 === sidecarSha256 &&
        /^[a-f0-9]{64}$/.test(String(receipt.result.fixtureSha256)),
        "Direct packaged proof is stale, unsafe, or bound to another source, app, sidecar, or fixture.");
      requireValidGateResult(receipt as GateReceipt);
      return { id, evidenceClass: "PACKAGED_APP" as const, status: "pass" as const };
    } catch (error) {
      return {
        id,
        evidenceClass: "PACKAGED_APP" as const,
        status: "invalid" as const,
        reason: error instanceof Error ? error.message : String(error),
      };
    }
  });
  const missingJourneys = journeys
    .filter((journey) => journey.status === "unmeasured")
    .map((journey) => journey.id);
  const invalidJourneys = journeys
    .filter((journey) => journey.status === "invalid")
    .map((journey) => journey.id);
  const assurances = REQUIRED_PACKAGED_ASSURANCES.map((requirement) => {
    const path = join(options.evidenceDirectory, `${requirement.gateId}.json`);
    if (!existsSync(path)) {
      return {
        ...requirement,
        status: "unmeasured" as const,
        reason: requirement.id === "direct_matrix"
          ? "No complete same-candidate target-scoped direct surface matrix was produced."
          : "No owner-authorized visible painted-output samples or owner-ratified budget were produced.",
      };
    }
    try {
      const receipt = readJson(path);
      requireCondition(receipt.schemaVersion === RELEASE_EVIDENCE_SCHEMA_VERSION &&
        receipt.gateId === requirement.gateId &&
        receipt.evidenceClass === requirement.evidenceClass && receipt.status === "pass" &&
        receipt.noninteractive === true && receipt.sourceState === "clean" &&
        receipt.publishable === true && receipt.sourceSha === options.sourceSha &&
        receipt.result?.assuranceId === requirement.id &&
        receipt.result.binarySha256 === binarySha256 &&
        receipt.result.sidecarSha256 === sidecarSha256,
        "Candidate assurance is stale, unsafe, or bound to another source, app, or Pi sidecar.");
      requireValidGateResult(receipt as GateReceipt);
      return { ...requirement, status: "pass" as const };
    } catch (error) {
      return {
        ...requirement,
        status: "invalid" as const,
        reason: error instanceof Error ? error.message : String(error),
      };
    }
  });
  const missingAssurances = assurances
    .filter((assurance) => assurance.status === "unmeasured")
    .map((assurance) => assurance.id);
  const invalidAssurances = assurances
    .filter((assurance) => assurance.status === "invalid")
    .map((assurance) => assurance.id);
  return {
    schemaVersion: 1,
    status: missingJourneys.length === 0 && invalidJourneys.length === 0 &&
      missingAssurances.length === 0 && invalidAssurances.length === 0 ? "pass" : "blocked",
    sourceSha: options.sourceSha,
    binarySha256,
    sidecarSha256,
    journeys,
    assurances,
    missingJourneys,
    invalidJourneys,
    missingAssurances,
    invalidAssurances,
  };
}

function flag(argv: string[], name: string, required = true): string | undefined {
  const index = argv.indexOf(name);
  if (index < 0) {
    requireCondition(!required, `missing required flag ${name}`);
    return undefined;
  }
  const value = argv[index + 1];
  requireCondition(value !== undefined && !value.startsWith("--"),
    `flag ${name} requires a value`);
  return value;
}

function flags(argv: string[], name: string): string[] {
  const result: string[] = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] !== name) continue;
    const value = argv[index + 1];
    requireCondition(value !== undefined && !value.startsWith("--"),
      `flag ${name} requires a value`);
    result.push(value);
  }
  return result;
}

function run(argv: string[]): void {
  const [command, ...arguments_] = argv;

  switch (command) {
    case "packaged-readiness": {
      const readiness = assessPackagedJourneyReadiness({
        appPath: resolve(flag(arguments_, "--app")!),
        evidenceDirectory: resolve(flag(arguments_, "--evidence-dir")!),
        sourceSha: flag(arguments_, "--source-sha")!,
      });
      const outputPath = resolve(flag(arguments_, "--output")!);
      writeJson(outputPath, readiness);
      console.log(JSON.stringify(readiness, null, 2));
      requireCondition(readiness.status === "pass",
        `packaged release BLOCKED: ` +
        `${readiness.missingJourneys.length + readiness.missingAssurances.length} UNMEASURED and ` +
        `${readiness.invalidJourneys.length + readiness.invalidAssurances.length} invalid ` +
        `same-binary journeys/assurances`);
      return;
    }
    case "attest-signing": {
      const attestation = buildSigningAttestation({
        appPath: resolve(flag(arguments_, "--app")!),
        notarizationPath: resolve(flag(arguments_, "--notarization")!),
        notarizedArchivePath: resolve(flag(arguments_, "--notarized-archive")!),
        sourceSha: flag(arguments_, "--source-sha")!,
        teamIdentifier: flag(arguments_, "--team-id")!,
      });
      const outputPath = resolve(flag(arguments_, "--output")!);
      writeJson(outputPath, attestation);
      console.log(`release_evidence signed team=${attestation.teamIdentifier} notarization=${attestation.notarizationSubmissionId} output=${outputPath}`);
      return;
    }
    case "sdk-summary": {
      const result = sdkSuiteSummary(flag(arguments_, "--result")!);
      console.log(`release_evidence sdk passed=${result.passed} failed=${result.failed} skipped=${result.skipped}`);
      return;
    }
    case "gate": {
      const gateId = flag(arguments_, "--gate")!;
      const sourceSha = flag(arguments_, "--source-sha")!;
      const sourceProvenance = inspectGateSourceProvenance({
        gateId,
        sourceSha,
        diagnosticDirty: arguments_.includes("--diagnostic-dirty"),
        ownerPaths: flags(arguments_, "--owner"),
      });
      const receipt = buildGateReceipt({
        gateId,
        evidenceClass: flag(arguments_, "--class")!,
        sourceSha,
        resultPath: flag(arguments_, "--result", false),
        sourceProvenance,
      });
      const outputPath = resolve(flag(arguments_, "--output")!);
      writeJson(outputPath, receipt);
      console.log(`release_evidence gate=${receipt.gateId} class=${receipt.evidenceClass} output=${outputPath}`);
      return;
    }
    case "manifest": {
      const options: ManifestOptions = {
        zipPath: resolve(flag(arguments_, "--zip")!),
        appPath: resolve(flag(arguments_, "--app")!),
        contractsPath: resolve(flag(arguments_, "--contracts")!),
        designTokensPath: resolve(flag(arguments_, "--design-tokens")!),
        designCssPath: resolve(flag(arguments_, "--design-css")!),
        designProofPath: resolve(flag(arguments_, "--design-proof")!),
        outputPath: resolve(flag(arguments_, "--output")!),
        sourceSha: flag(arguments_, "--source-sha")!,
        version: flag(arguments_, "--version")!,
        tag: flag(arguments_, "--tag")!,
        evidencePaths: flags(arguments_, "--evidence").map((path) => resolve(path)),
      };
      const manifest = buildReleaseManifest(options);
      writeJson(options.outputPath, manifest);
      console.log(JSON.stringify(manifest, null, 2));
      return;
    }
    case "verify": {
      const manifest = verifyReleaseManifest({
        manifestPath: resolve(flag(arguments_, "--manifest")!),
        zipPath: resolve(flag(arguments_, "--zip")!),
        sourceSha: flag(arguments_, "--source-sha")!,
        tag: flag(arguments_, "--tag")!,
        appPath: flag(arguments_, "--app", false),
        designTokensPath: resolve(flag(arguments_, "--design-tokens")!),
        designCssPath: resolve(flag(arguments_, "--design-css")!),
        designProofPath: resolve(flag(arguments_, "--design-proof")!),
      });
      console.log(`release_evidence verified version=${manifest.version} source=${manifest.source_sha} gates=${manifest.verification.gates.length}`);
      return;
    }
    case "scorecard": {
      const manifest = verifyReleaseManifest({
        manifestPath: resolve(flag(arguments_, "--manifest")!),
        zipPath: resolve(flag(arguments_, "--zip")!),
        sourceSha: flag(arguments_, "--source-sha")!,
        tag: flag(arguments_, "--tag")!,
        appPath: flag(arguments_, "--app", false),
        designTokensPath: resolve(flag(arguments_, "--design-tokens")!),
        designCssPath: resolve(flag(arguments_, "--design-css")!),
        designProofPath: resolve(flag(arguments_, "--design-proof")!),
      });
      const outputPath = resolve(flag(arguments_, "--output")!);
      const scorecard = buildReleaseScorecard(manifest);
      writeJson(outputPath, scorecard);
      console.log(JSON.stringify(scorecard, null, 2));
      return;
    }
    default:
      throw new Error(`unknown release-evidence command: ${command ?? "(none)"}`);
  }
}

if (import.meta.main) {
  try {
    run(process.argv.slice(2));
  } catch (error) {
    console.error(`release_evidence FAIL: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
