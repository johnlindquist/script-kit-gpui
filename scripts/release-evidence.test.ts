import { afterEach, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  renameSync,
  rmSync,
  statSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { deflateRawSync } from "node:zlib";

import {
  assessPackagedJourneyReadiness,
  buildGateReceipt,
  buildReleaseManifest,
  buildReleaseScorecard,
  buildSigningAttestation,
  RELEASE_INTEGRATION_SUITES,
  REQUIRED_PACKAGED_ASSURANCES,
  REQUIRED_PACKAGED_JOURNEYS,
  verifyReleaseManifest,
  type GateId,
  type ManifestOptions,
} from "./release-evidence.ts";
import {
  GENERATED_BYTE_COMPARE_OUTPUT_PATHS,
  GENERATED_BYTE_COMPARE_SOURCE_PATHS,
} from "./devtools/generated-byte-compare.ts";
import {
  buildCanonicalMappings,
  buildCoverageBindingSet,
  type CoverageBindingRecord,
  type SurfaceContractRegistry,
} from "./devtools/surfaces.ts";
import { prepareValidatedReceipt } from "./devtools/lib/receipt-schema.ts";

const SOURCE_SHA = "a".repeat(40);
const APPLE_TEAM_ID = "A1B2C3D4E5";
const NOTARIZATION_ID = "12345678-1234-4123-a123-123456789abc";
const TEMPORARY_DIRECTORIES: string[] = [];

interface ReleaseFixture {
  root: string;
  appPath: string;
  executablePath: string;
  sidecarPath: string;
  sdkPath: string;
  zipPath: string;
  contractsPath: string;
  designTokensPath: string;
  designCssPath: string;
  designProofPath: string;
  manifestPath: string;
  gatePaths: Map<GateId, string>;
}

function hash(contents: string | Uint8Array): string {
  return createHash("sha256").update(contents).digest("hex");
}

interface SyntheticZipMember {
  path: string;
  contents: string | Uint8Array;
  compression?: 0 | 8;
  unixMode?: number;
  localPath?: string;
  usesDataDescriptor?: boolean;
}

function zipCrc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc >>> 1) ^ ((crc & 1) === 0 ? 0 : 0xedb88320);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function writeSyntheticZip(path: string, members: SyntheticZipMember[]): void {
  const localRecords: Buffer[] = [];
  const centralRecords: Buffer[] = [];
  let localOffset = 0;
  for (const member of members) {
    const name = Buffer.from(member.path, "utf8");
    const localName = Buffer.from(member.localPath ?? member.path, "utf8");
    const contents = Buffer.from(member.contents);
    const compression = member.compression ?? 0;
    const compressed = compression === 8 ? deflateRawSync(contents) : contents;
    const crc = zipCrc32(contents);
    const flags = member.usesDataDescriptor ? 0x08 : 0;

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(flags, 6);
    local.writeUInt16LE(compression, 8);
    if (!member.usesDataDescriptor) {
      local.writeUInt32LE(crc, 14);
      local.writeUInt32LE(compressed.length, 18);
      local.writeUInt32LE(contents.length, 22);
    }
    local.writeUInt16LE(localName.length, 26);
    localRecords.push(local, localName, compressed);
    let descriptorBytes = 0;
    if (member.usesDataDescriptor) {
      const descriptor = Buffer.alloc(16);
      descriptor.writeUInt32LE(0x08074b50, 0);
      descriptor.writeUInt32LE(crc, 4);
      descriptor.writeUInt32LE(compressed.length, 8);
      descriptor.writeUInt32LE(contents.length, 12);
      localRecords.push(descriptor);
      descriptorBytes = descriptor.length;
    }

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE((3 << 8) | 20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(flags, 8);
    central.writeUInt16LE(compression, 10);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(compressed.length, 20);
    central.writeUInt32LE(contents.length, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE(((member.unixMode ?? 0o100644) << 16) >>> 0, 38);
    central.writeUInt32LE(localOffset, 42);
    centralRecords.push(central, name);
    localOffset += local.length + localName.length + compressed.length + descriptorBytes;
  }

  const directory = Buffer.concat(centralRecords);
  const trailer = Buffer.alloc(22);
  trailer.writeUInt32LE(0x06054b50, 0);
  trailer.writeUInt16LE(members.length, 8);
  trailer.writeUInt16LE(members.length, 10);
  trailer.writeUInt32LE(directory.length, 12);
  trailer.writeUInt32LE(localOffset, 16);
  writeFileSync(path, Buffer.concat([...localRecords, directory, trailer]));
}

function syntheticAppMembers(appPath: string): SyntheticZipMember[] {
  const required = [
    "Contents/MacOS/script-kit-gpui",
    "Contents/MacOS/pi",
    "Contents/Resources/scripts/kit-sdk.ts",
    "Contents/Info.plist",
  ];
  const paths: string[] = [];
  const walk = (directory: string, prefix: string): void => {
    for (const member of readdirSync(directory, { withFileTypes: true })) {
      const relativePath = prefix ? `${prefix}/${member.name}` : member.name;
      const path = join(directory, member.name);
      if (lstatSync(path).isDirectory()) {
        walk(path, relativePath);
      } else {
        paths.push(relativePath);
      }
    }
  };
  walk(appPath, "");
  const ordered = [...required, ...paths.filter((path) => !required.includes(path)).sort()];
  return ordered.map((relativePath) => {
    const path = join(appPath, relativePath);
    const metadata = lstatSync(path);
    return {
      path: `Script Kit.app/${relativePath}`,
      contents: metadata.isSymbolicLink()
        ? Buffer.from(readlinkSync(path), "utf8")
        : readFileSync(path),
      unixMode: metadata.mode,
    };
  });
}

let cachedSyntheticDirectMatrix: {
  registry: SurfaceContractRegistry;
  observation: Record<string, unknown>;
} | undefined;

function syntheticDirectMatrix(): {
  registry: SurfaceContractRegistry;
  observation: Record<string, unknown>;
} {
  if (cachedSyntheticDirectMatrix) return cachedSyntheticDirectMatrix;

  // The receipt objects below never leave this isolated unit-test fixture.
  // Production always derives the proof policy from the actual committed
  // contract and refuses every surface until genuine runtime receipts exist.
  const source = JSON.parse(readFileSync(
    resolve(import.meta.dir, "../docs/ai/contracts/surface-contracts.json"),
    "utf8",
  )) as SurfaceContractRegistry;
  const registry: SurfaceContractRegistry = {
    ...source,
    entries: source.entries.map((entry) => ({
      ...entry,
      proofPolicy: "StateReceiptProof",
      focusPolicy: "NoEditableFocus",
      keyboardPolicy: "NoEditableKeyboard",
      actionsPolicy: "NoSurfaceActions",
      visualPolicy: "CompactLauncherVisual",
      vocabulary: { ...entry.vocabulary, inputOwnership: "NoEditableInput" },
    })),
  };
  const canonical = buildCoverageBindingSet(buildCanonicalMappings(registry));
  if (canonical.errors.length > 0 || canonical.set.bindings.length !== 54) {
    throw new Error(`invalid synthetic canonical bindings: ${canonical.errors.join(", ")}`);
  }

  const transactionId = "synthetic-same-candidate-transaction";
  const primitiveReceipts: Array<{ path: string; receipt: Record<string, unknown> }> = [];
  const targets = canonical.set.bindings.map((binding: CoverageBindingRecord, index) => {
    const targetId = binding.expectedTargetIdentity.parentRequired
      ? "synthetic-actions-popup"
      : "main";
    const target = {
      bindingId: binding.bindingId,
      targetId,
      transactionId,
      evidenceClass: "RUNTIME_HIDDEN",
      sourceSha: SOURCE_SHA,
      binarySha256: hash("application-binary"),
      lifetimeGeneration: 1,
      windowInstanceId: `${targetId}@1`,
      windowGeneration: 1,
      targetGeneration: index + 1,
      surfaceGeneration: index + 1,
      surfaceKind: binding.contractKind,
      appViewVariant: binding.appViewVariant,
      windowKind: binding.expectedTargetIdentity.windowKind,
      hostKind: binding.expectedTargetIdentity.hostKind,
      ...(binding.expectedTargetIdentity.parentRequired
        ? { parentAutomationId: "main" }
        : {}),
      privacyRedacted: true,
      cleanupVerified: true,
    };
    const transaction = {
      transactionId,
      runId: "release-unit-fixture-only",
      pid: 4242,
      processStartTime: "synthetic-unit-fixture-process",
      binarySha256: target.binarySha256,
      automationId: targetId,
      windowInstanceId: target.windowInstanceId,
      windowGeneration: target.windowGeneration,
      targetGeneration: target.targetGeneration,
      surfaceGeneration: target.surfaceGeneration,
      dataGeneration: index + 1,
      windowKind: target.windowKind,
      hostKind: target.hostKind,
      surfaceKind: target.surfaceKind,
      semanticSurface: "synthetic-unit-fixture-only",
      appViewVariant: target.appViewVariant,
      ...(binding.expectedTargetIdentity.parentRequired
        ? { parentAutomationId: "main" }
        : {}),
      bounds: { x: 0, y: 0, width: 800, height: 600 },
    };
    const selector = targetId === "main"
      ? { type: "main" }
      : { type: "id", id: targetId };
    const targetIdentity = {
      automationId: targetId,
      windowInstanceId: target.windowInstanceId,
      windowGeneration: target.windowGeneration,
      targetGeneration: target.targetGeneration,
      surfaceGeneration: target.surfaceGeneration,
      appViewVariant: target.appViewVariant,
      surfaceKind: target.surfaceKind,
      visible: false,
      bounds: transaction.bounds,
    };

    for (const primitiveId of binding.requiredPrimitiveIds) {
      const detail = primitiveId === "devtools.targets.inspect"
        ? {
          tool: "script-kit-devtools.targets",
          command: "targets.inspect",
          requestedTarget: { selector },
          resolvedTarget: targetIdentity,
        }
        : primitiveId === "devtools.surface.inspect"
          ? {
            tool: "script-kit-devtools.surface",
            command: "surface.inspect",
            requestedTarget: { selector },
            target: targetIdentity,
            contract: { surfaceKind: target.surfaceKind },
            runtime: { capabilities: [], missingPrimitives: [] },
          }
          : undefined;
      if (!detail) throw new Error(`unexpected synthetic fixture primitive: ${primitiveId}`);
      const prepared = prepareValidatedReceipt(primitiveId, {
        schemaVersion: 2,
        classification: "ok",
        evidenceClass: "RUNTIME_HIDDEN",
        repository: { gitCommit: SOURCE_SHA },
        binary: { sha256: target.binarySha256 },
        receiptId: `${binding.bindingId}:${primitiveId}`,
        transaction,
        durationMs: 1,
        missingPrimitives: [],
        errors: [],
        cleanup: { closed: true, survivors: [] },
        ...detail,
      });
      if (prepared.exitCode !== 0) {
        throw new Error(`invalid synthetic direct primitive: ${prepared.validation.errors.join(", ")}`);
      }
      primitiveReceipts.push({
        path: `fixture/${encodeURIComponent(binding.bindingId)}/${primitiveId}.json`,
        receipt: prepared.receipt,
      });
    }
    return target;
  });

  cachedSyntheticDirectMatrix = {
    registry,
    observation: {
      observationLayer: "RUNTIME_HIDDEN",
      expectedMappings: 54,
      directProvenMappings: 54,
      transactionId,
      everyTargetDirect: true,
      sameTransaction: true,
      privacyRedacted: true,
      cleanupVerified: true,
      targets,
      primitiveReceipts,
    },
  };
  return cachedSyntheticDirectMatrix;
}

function json(path: string, value: unknown): void {
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function syntheticSigningRunner(
  executable: string,
  args: string[],
): { status: number; stdout: string; stderr: string } {
  if (executable === "codesign" && args[0] === "-d") {
    return {
      status: 0,
      stdout: "",
      stderr: `TeamIdentifier=${APPLE_TEAM_ID}\nflags=0x10000(runtime)\n`,
    };
  }
  return { status: 0, stdout: "accepted", stderr: "" };
}

function makeFixture(): ReleaseFixture {
  const root = mkdtempSync(join(tmpdir(), "script-kit-release-evidence-"));
  TEMPORARY_DIRECTORIES.push(root);

  const appPath = join(root, "Script Kit.app");
  const executablePath = join(appPath, "Contents/MacOS/script-kit-gpui");
  const sidecarPath = join(appPath, "Contents/MacOS/pi");
  const sdkPath = join(appPath, "Contents/Resources/scripts/kit-sdk.ts");
  const zipPath = join(root, "Script-Kit-macos.zip");
  const contractsPath = join(root, "docs/ai/contracts/surface-contracts.json");
  const designTokensPath = join(root, "design/mockups/generated/tokens.json");
  const designCssPath = join(root, "design/mockups/generated/tokens.css");
  const manifestPath = join(root, "release-manifest.json");

  mkdirSync(join(appPath, "Contents/MacOS"), { recursive: true });
  mkdirSync(join(appPath, "Contents/Resources/scripts"), { recursive: true });
  mkdirSync(join(appPath, "Contents/Resources/scripts/migrate"), { recursive: true });
  mkdirSync(join(appPath, "Contents/Resources/assets"), { recursive: true });
  mkdirSync(join(appPath, "Contents/_CodeSignature"), { recursive: true });
  writeFileSync(executablePath, "application-binary", { mode: 0o755 });
  writeFileSync(sidecarPath, "pi-sidecar-binary", { mode: 0o755 });
  writeFileSync(sdkPath, "export const SDK_VERSION = '0.2.0';\n");
  writeFileSync(join(appPath, "Contents/Info.plist"),
    "<plist><dict><key>CFBundleIdentifier</key><string>com.scriptkit.app</string></dict></plist>");
  writeFileSync(join(appPath, "Contents/Resources/scripts/migrate/cli.ts"),
    "export const privateMigration = 'signed-original';\n");
  writeFileSync(join(appPath, "Contents/Resources/assets/icon.svg"), "<svg>signed-icon</svg>");
  writeFileSync(join(appPath, "Contents/Resources/assets/empty-resource"), "");
  writeFileSync(join(appPath, "Contents/_CodeSignature/CodeResources"),
    "synthetic-signed-resource-envelope");
  writeSyntheticZip(zipPath, syntheticAppMembers(appPath));
  mkdirSync(join(root, "design/mockups/generated"), { recursive: true });
  writeFileSync(designTokensPath, '{"synthetic":"canonical-design-token"}\n');
  writeFileSync(designCssPath, ':root { --synthetic: #00b3b3; }\n');
  for (const path of GENERATED_BYTE_COMPARE_SOURCE_PATHS) {
    const sourcePath = join(root, path);
    mkdirSync(join(sourcePath, ".."), { recursive: true });
    writeFileSync(sourcePath, `source:${path}`);
  }
  const designExporterPath = join(root,
    "target-agent/pools/agent-debug/debug/export_design_tokens");
  mkdirSync(join(designExporterPath, ".."), { recursive: true });
  writeFileSync(designExporterPath, "synthetic-design-exporter", { mode: 0o755 });
  json(contractsPath, syntheticDirectMatrix().registry);

  const sdkResultPath = join(root, "sdk-result.json");
  json(sdkResultPath, { total_passed: 215, total_failed: 0, total_skipped: 0 });

  const rustResultPath = join(root, "rust-result.log");
  writeFileSync(rustResultPath,
    "running 32 tests\ntest result: ok. 32 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out\n");

  const integrationResultPath = join(root, "integration-result.log");
  writeFileSync(integrationResultPath, RELEASE_INTEGRATION_SUITES.map((suite) => [
    `     Running tests/${suite}.rs (target/debug/deps/${suite}-1234)`,
    "running 3 tests",
    "test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out",
  ].join("\n")).join("\n"));

  const firstRunResultPath = join(root, "first-run-result.log");
  writeFileSync(firstRunResultPath, [
    "running 1 test",
    "test setup::tests::test_fresh_install_seeds_canonical_menu_syntax_handlers ... ok",
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out",
  ].join("\n"));

  const permissionsResultPath = join(root, "permissions-result.log");
  writeFileSync(permissionsResultPath, [
    "running 1 test",
    "test permissions_wizard::tests::test_snapshot_missing_required ... ok",
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out",
  ].join("\n"));

  const mockAiResultPath = join(root, "mock-ai-result.log");
  writeFileSync(mockAiResultPath, [
    "running 1 test",
    "test ai_reliability::model_tests::blocked_capability_produces_actionable_recovery_without_starting ... ok",
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out",
  ].join("\n"));

  const privacyResultPath = join(root, "privacy-result.log");
  writeFileSync(privacyResultPath, [
    "running 1 test",
    "test ai::reliability::tests::redactor_allowlists_json_masks_secrets_paths_and_bounds_output ... ok",
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out",
  ].join("\n"));

  const proofResultPath = join(root, "proof-result.log");
  writeFileSync(proofResultPath, [
    "scripts/devtools/operator-safety.test.ts:",
    "scripts/devtools/privacy.test.ts:",
    "scripts/devtools/family-fixtures.test.ts:",
    "scripts/devtools/alpha-byte-contract.test.ts:",
    "scripts/devtools/generated-byte-compare.test.ts:",
    "scripts/devtools/state-ownership.test.ts:",
    "scripts/migrate/__tests__/classify.test.ts:",
    "scripts/agentic/cargo-build-policy.test.ts:",
    "scripts/agentic/quick-ai-latency-bench.test.ts:",
    "tests/sdk/runner-safety.test.ts:",
    " 355 pass",
    " 0 fail",
    " 1112 expect() calls",
    "Ran 355 tests across 24 files. [8.20s]",
  ].join("\n"));

  const designResultPath = join(root, "generated-design-contracts-result.json");
  const designOutputs = GENERATED_BYTE_COMPARE_OUTPUT_PATHS.map((path, index) => {
    const contents = readFileSync(index === 0 ? designTokensPath : designCssPath);
    const sha256 = createHash("sha256").update(contents).digest("hex");
    return {
      path,
      checkedInSha256: sha256,
      generatedSha256: sha256,
      byteEqual: true,
      byteLength: contents.byteLength,
    };
  });
  const designOutputHashes = Object.fromEntries(
    designOutputs.map((output) => [output.path, output.checkedInSha256]),
  );
  json(designResultPath, {
    schemaVersion: 1,
    generatedBy: "scripts/devtools/generated-byte-compare.ts",
    taskId: "GOV-005",
    evidenceClass: "UNIT_BEHAVIOR",
    provesRuntimeBehavior: false,
    sourceSha: SOURCE_SHA,
    sourceCoverage: {
      mode: "DECLARED_EXPORTER_SOURCE_OWNERS",
      sourceGraphExhaustive: false,
    },
    sourceFingerprints: Object.fromEntries(
      GENERATED_BYTE_COMPARE_SOURCE_PATHS.map((path) => [path, hash(`source:${path}`)]),
    ),
    binary: {
      path: "target-agent/pools/agent-debug/debug/export_design_tokens",
      sha256: hash("synthetic-design-exporter"),
      sizeBytes: "synthetic-design-exporter".length,
    },
    outputHashes: designOutputHashes,
    generatedOutputHashes: designOutputHashes,
    outputs: designOutputs,
    byteEqual: true,
    handEditedGeneratedOutput: false,
    safety: {
      noninteractive: true,
      startsApplication: false,
      revealsWindow: false,
      focusesWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
      accessesNetwork: false,
      usesLiveAi: false,
      startsExporter: true,
      isolatedTempOutput: true,
    },
    execution: { exitCode: 0, stdoutSha256: hash(""), stderrSha256: hash("") },
    cleanup: { closed: true, survivors: [] },
    disposition: "EVALUABLE_PASS",
    pass: true,
  });

  const runtimeResultPath = join(root, "runtime-result.json");
  json(runtimeResultPath, {
    status: "pass",
    behavior: { status: "pass" },
    evidenceClass: "RUNTIME_HIDDEN",
    metricKind: "semantic_frame_identity",
    measuresPaint: false,
    provenance: { gitSha: SOURCE_SHA, binarySha256: hash("application-binary") },
    safety: {
      startsApplication: true,
      isolatedCiLaunchAuthorized: true,
      sandboxHome: true,
      windowRevealAllowed: false,
      windowFocusAllowed: false,
      nativeInputAllowed: false,
      screenCaptureAllowed: false,
      hiddenStateAssertionCount: 4,
    },
    cleanup: { hidden: true, closed: true },
    artifactLifecycle: { allRequiredValid: true, allRecordedPathsReadable: true },
  });

  const notarizationPath = join(root, "notarization-result.json");
  json(notarizationPath, { id: NOTARIZATION_ID, status: "Accepted" });
  const signingResultPath = join(root, "signing-result.json");
  json(signingResultPath, buildSigningAttestation({
    appPath,
    notarizationPath,
    notarizedArchivePath: zipPath,
    sourceSha: SOURCE_SHA,
    teamIdentifier: APPLE_TEAM_ID,
    runCommand: syntheticSigningRunner,
  }));

  const packagedJourneyResults = new Map<string, string>();
  for (const journeyId of REQUIRED_PACKAGED_JOURNEYS) {
    const resultPath = join(root, `${journeyId}-result.json`);
    const observations = journeyId === "packaged-first-install" ? {
      freshSandboxHome: true,
      bundledSdkDiscovered: true,
      starterScriptsIndexed: 4,
      readyToType: true,
    } : journeyId === "packaged-permissions" ? {
      syntheticPermissionSnapshot: true,
      permissionRequestsStarted: 0,
      missingRequired: ["Accessibility"],
      recoverableGuidanceVisibleToAutomation: true,
    } : journeyId === "packaged-migration" ? {
      originalUserDataSha256: hash("preserved-user-data"),
      preservedUserDataSha256: hash("preserved-user-data"),
      legacyFixtureLoaded: true,
      migrationCompleted: true,
    } : {
      mockProvider: true,
      liveProviderStarts: 0,
      failureObserved: true,
      recoveryActionCount: 2,
    };
    json(resultPath, {
      status: "pass",
      evidenceClass: "PACKAGED_APP",
      journey: { id: journeyId, status: "pass", observations },
      provenance: {
        gitSha: SOURCE_SHA,
        binarySha256: hash("application-binary"),
        sidecarSha256: hash("pi-sidecar-binary"),
        fixtureSha256: hash(`fixture:${journeyId}`),
      },
      safety: {
        startsApplication: true,
        isolatedCiLaunchAuthorized: true,
        sandboxHome: true,
        windowRevealAllowed: false,
        windowFocusAllowed: false,
        nativeInputAllowed: false,
        screenCaptureAllowed: false,
        microphoneAllowed: false,
        cameraAllowed: false,
        liveAiAllowed: false,
      },
      cleanup: { hidden: true, closed: true },
    });
    packagedJourneyResults.set(journeyId, resultPath);
  }

  const packagedAssuranceResults = new Map<string, string>();
  for (const requirement of REQUIRED_PACKAGED_ASSURANCES) {
    const resultPath = join(root, `${requirement.gateId}-result.json`);
    const painted = requirement.id === "ratified_perf";
    const observation = painted ? {
      observationLayer: "PAINTED_OUTPUT",
      metricKind: "painted_latency",
      measuresPaint: true,
      ownerRatified: true,
      ratifiedBudgetId: "owner-approved-release-latency-v1",
      ratificationReference: "owner-review:2026-08-22/release-latency-v1",
      sampleCount: 30,
      samplesMs: Array.from({ length: 30 }, (_, index) =>
        index < 15 ? 12 : index < 29 ? 28 : 70),
      paintEvidence: { source: "compositor_presented_frame", presentedFrameCount: 30 },
      p50Ms: 12,
      p95Ms: 28,
      maxMs: 70,
      budgets: { p50Ms: 25, p95Ms: 50, maxMs: 150 },
    } : syntheticDirectMatrix().observation;
    json(resultPath, {
      status: "pass",
      evidenceClass: requirement.evidenceClass,
      assurance: { id: requirement.id, status: "pass", observation },
      provenance: {
        gitSha: SOURCE_SHA,
        binarySha256: hash("application-binary"),
        sidecarSha256: hash("pi-sidecar-binary"),
        ...(painted ? {} : {
          surfaceContractSha256: hash(readFileSync(contractsPath, "utf8")),
        }),
      },
      safety: {
        startsApplication: true,
        isolatedCiLaunchAuthorized: true,
        sandboxHome: true,
        windowRevealAllowed: painted,
        ...(painted ? { ownerVisibleAuthorization: true } : {}),
        windowFocusAllowed: false,
        nativeInputAllowed: false,
        screenCaptureAllowed: false,
        microphoneAllowed: false,
        cameraAllowed: false,
        liveAiAllowed: false,
      },
      cleanup: { hidden: true, closed: true },
    });
    packagedAssuranceResults.set(requirement.gateId, resultPath);
  }

  const gatePaths = new Map<GateId, string>();
  const classes = {
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
  } as const;

  for (const [gateId, evidenceClass] of Object.entries(classes)) {
    const resultPath = gateId === "rust-tests" || gateId === "domain-tests" ? rustResultPath :
      gateId === "integration-tests" ? integrationResultPath :
      gateId === "first-run-fixtures" ? firstRunResultPath :
      gateId === "permissions-fixtures" ? permissionsResultPath :
      gateId === "mock-ai-fixtures" ? mockAiResultPath :
      gateId === "privacy-fixtures" ? privacyResultPath :
      gateId === "proof-contracts" ? proofResultPath :
      gateId === "generated-design-contracts" ? designResultPath :
      gateId === "sdk-tests" ? sdkResultPath :
      gateId === "packaged-signing" ? signingResultPath :
      gateId === "packaged-root-frame" ? runtimeResultPath :
      packagedAssuranceResults.get(gateId) ?? packagedJourneyResults.get(gateId);
    const receipt = buildGateReceipt({
      gateId,
      evidenceClass,
      sourceSha: SOURCE_SHA,
      resultPath,
      repositoryRoot: root,
    });
    const path = join(root, "evidence", `${gateId}.json`);
    json(path, receipt);
    gatePaths.set(gateId as GateId, path);
  }

  return {
    root,
    appPath,
    executablePath,
    sidecarPath,
    sdkPath,
    zipPath,
    contractsPath,
    designTokensPath,
    designCssPath,
    designProofPath: designResultPath,
    manifestPath,
    gatePaths,
  };
}

function options(fixture: ReleaseFixture): ManifestOptions {
  return {
    zipPath: fixture.zipPath,
    appPath: fixture.appPath,
    contractsPath: fixture.contractsPath,
    designTokensPath: fixture.designTokensPath,
    designCssPath: fixture.designCssPath,
    designProofPath: fixture.designProofPath,
    repositoryRoot: fixture.root,
    outputPath: fixture.manifestPath,
    sourceSha: SOURCE_SHA,
    version: "0.1.17",
    tag: "v0.1.17",
    evidencePaths: [...fixture.gatePaths.values()],
  };
}

function verificationOptions(
  fixture: ReleaseFixture,
): Parameters<typeof verifyReleaseManifest>[0] {
  return {
    manifestPath: fixture.manifestPath,
    zipPath: fixture.zipPath,
    sourceSha: SOURCE_SHA,
    tag: "v0.1.17",
    appPath: fixture.appPath,
    designTokensPath: fixture.designTokensPath,
    designCssPath: fixture.designCssPath,
    designProofPath: fixture.designProofPath,
    repositoryRoot: fixture.root,
  };
}

afterEach(() => {
  for (const path of TEMPORARY_DIRECTORIES.splice(0)) {
    rmSync(path, { recursive: true, force: true });
  }
});

describe("fail-closed release evidence", () => {
  test("records exact package, SDK, sidecar, source, contract, and gate identities", () => {
    const fixture = makeFixture();
    const manifest = buildReleaseManifest(options(fixture));

    expect(manifest.schema_version).toBe(3);
    expect(manifest.source_sha).toBe(SOURCE_SHA);
    expect(manifest.bundle.identifier).toBe("com.scriptkit.app");
    expect(manifest.bundle.info_plist).toEqual({
      path: "Contents/Info.plist",
      sha256: hash(readFileSync(join(fixture.appPath, "Contents/Info.plist"))),
    });
    expect(manifest.bundle.content_tree.algorithm).toBe("SHA256_CANONICAL_APP_TREE_V1");
    expect(manifest.bundle.content_tree.entry_count)
      .toBe(syntheticAppMembers(fixture.appPath).length);
    expect(manifest.bundle.content_tree.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(manifest.bundle.executable.sha256).toBe(hash("application-binary"));
    expect(manifest.bundle.sidecar.sha256).toBe(hash("pi-sidecar-binary"));
    expect(manifest.sdk.version).toBe("0.2.0");
    expect(manifest.surface_contracts.schema_version).toBe(1);
    expect(manifest.verification.visibility).toEqual({
      journeys: "hidden_only",
      paintedOutput: "owner_authorized_visible",
    });
    expect(manifest.verification.gates).toHaveLength(19);
    expect(manifest.distribution_security).toEqual({
      team_identifier: APPLE_TEAM_ID,
      notarization_submission_id: NOTARIZATION_ID,
      notarized_archive_sha256: hash(readFileSync(fixture.zipPath)),
      hardened_runtime: true,
      stapled: true,
      gatekeeper_accepted: true,
    });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "sdk-tests")?.result)
      .toEqual({ passed: 215, failed: 0, skipped: 0 });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "rust-tests")?.result)
      .toEqual({ passed: 32, failed: 0, skipped: 2 });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "integration-tests")?.result)
      .toEqual({
        passed: 18,
        failed: 0,
        skipped: 6,
        suites: 6,
        suiteNames: [...RELEASE_INTEGRATION_SUITES].sort(),
      });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "first-run-fixtures")?.result)
      .toEqual({ passed: 1, failed: 0, skipped: 0 });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "permissions-fixtures")?.result)
      .toEqual({ passed: 1, failed: 0, skipped: 0 });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "mock-ai-fixtures")?.result)
      .toEqual({ passed: 1, failed: 0, skipped: 0 });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "privacy-fixtures")?.result)
      .toEqual({ passed: 1, failed: 0, skipped: 0 });
    expect(manifest.verification.gates.find((gate) => gate.gateId === "proof-contracts")?.result)
      .toEqual({ passed: 355, failed: 0, skipped: 0, files: 24, assertions: 1112 });
    const directMatrix = manifest.verification.gates.find((gate) =>
      gate.gateId === "packaged-direct-matrix")?.result;
    expect(directMatrix?.rawEvidenceSha256).toBe(hash(readFileSync(
      join(fixture.root, "packaged-direct-matrix-result.json"),
      "utf8",
    )));
    expect(directMatrix?.bindingFingerprintSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(directMatrix?.primitiveReceiptCount).toBe(108);
    expect(manifest.verification.gates.find((gate) => gate.gateId === "consistency-catalog")?.evidenceClass)
      .toBe("STATIC_INVENTORY");
  });

  test("SDK proof rejects empty, failing, and skipped behavior suites", () => {
    const fixture = makeFixture();
    const resultPath = join(fixture.root, "empty-sdk.json");
    json(resultPath, { total_passed: 0, total_failed: 0, total_skipped: 3 });

    expect(() => buildGateReceipt({
      gateId: "sdk-tests", evidenceClass: "SDK_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("at least one passing");

    json(resultPath, { total_passed: 3, total_failed: 1, total_skipped: 0 });
    expect(() => buildGateReceipt({
      gateId: "sdk-tests", evidenceClass: "SDK_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("contains failing tests");

    json(resultPath, { total_passed: 215, total_failed: 0, total_skipped: 1 });
    expect(() => buildGateReceipt({
      gateId: "sdk-tests", evidenceClass: "SDK_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("cannot skip behavior coverage");
  });

  test("Rust proof rejects compile-only output and zero executed tests", () => {
    const fixture = makeFixture();
    const resultPath = join(fixture.root, "compile-only.log");
    writeFileSync(resultPath, "Finished test [unoptimized + debuginfo] target(s) in 2.0s\n");

    expect(() => buildGateReceipt({
      gateId: "rust-tests", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("compile-only output cannot satisfy");

    writeFileSync(resultPath,
      "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n");
    expect(() => buildGateReceipt({
      gateId: "rust-tests", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("did not execute any passing tests");
  });

  test("integration proof requires every named behavior suite with nonzero passing tests", () => {
    const fixture = makeFixture();
    const resultPath = join(fixture.root, "incomplete-integration.log");
    writeFileSync(resultPath,
      "running 40 tests\ntest result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n");
    expect(() => buildGateReceipt({
      gateId: "integration-tests", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("missing a directly executed required suite");

    writeFileSync(resultPath, RELEASE_INTEGRATION_SUITES.map((suite) => [
      `Running tests/${suite}.rs (target/debug/deps/${suite})`,
      `test result: ok. ${suite === "protocol_batch" ? 0 : 1} passed; 0 failed; 0 ignored`,
    ].join("\n")).join("\n"));
    expect(() => buildGateReceipt({
      gateId: "integration-tests", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("integration suite executed no passing tests: protocol_batch");
  });

  test("first-run and privacy fixtures reject unrelated passing Rust suites", () => {
    const fixture = makeFixture();
    const resultPath = join(fixture.root, "rust-result.log");
    for (const gateId of [
      "first-run-fixtures", "permissions-fixtures", "mock-ai-fixtures", "privacy-fixtures",
    ] as const) {
      expect(() => buildGateReceipt({
        gateId, evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
      })).toThrow("focused fixture did not execute the required passing test");
    }
  });

  test("proof gate rejects zero-count summaries and missing migration or safety fixtures", () => {
    const fixture = makeFixture();
    const resultPath = join(fixture.root, "invalid-proof.log");
    writeFileSync(resultPath, " 0 pass\n 0 fail\n 0 expect() calls\nRan 0 tests across 0 files.\n");
    expect(() => buildGateReceipt({
      gateId: "proof-contracts", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("nonzero tests, assertions, and files");

    const valid = readFileSync(join(fixture.root, "proof-result.log"), "utf8");
    writeFileSync(resultPath, valid.replace("scripts/migrate/__tests__/classify.test.ts:", ""));
    expect(() => buildGateReceipt({
      gateId: "proof-contracts", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("missing its required directly executed fixture suite: scripts/migrate/__tests__/classify.test.ts");

    writeFileSync(resultPath, valid.replace("scripts/devtools/alpha-byte-contract.test.ts:", ""));
    expect(() => buildGateReceipt({
      gateId: "proof-contracts", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("missing its required directly executed fixture suite: scripts/devtools/alpha-byte-contract.test.ts");

    writeFileSync(resultPath, valid.replace("scripts/agentic/cargo-build-policy.test.ts:", ""));
    expect(() => buildGateReceipt({
      gateId: "proof-contracts", evidenceClass: "UNIT_BEHAVIOR", sourceSha: SOURCE_SHA, resultPath,
    })).toThrow("missing its required directly executed fixture suite: scripts/agentic/cargo-build-policy.test.ts");
  });

  test("signing attestation requires an accepted notarization and the exact signing team", () => {
    const fixture = makeFixture();
    const notarizationPath = join(fixture.root, "notarization-result.json");
    const signingOptions = {
      appPath: fixture.appPath,
      notarizationPath,
      notarizedArchivePath: fixture.zipPath,
      sourceSha: SOURCE_SHA,
      teamIdentifier: APPLE_TEAM_ID,
      runCommand: syntheticSigningRunner,
    };

    json(notarizationPath, { id: NOTARIZATION_ID, status: "In Progress" });
    expect(() => buildSigningAttestation(signingOptions)).toThrow("explicitly report Accepted");

    json(notarizationPath, { id: "missing-accepted-id", status: "Accepted" });
    expect(() => buildSigningAttestation(signingOptions)).toThrow("exact submission identifier");

    json(notarizationPath, { id: NOTARIZATION_ID, status: "Accepted" });
    expect(() => buildSigningAttestation({
      ...signingOptions,
      runCommand: (executable, args) => executable === "codesign" && args[0] === "-d"
        ? { status: 0, stdout: "", stderr: "TeamIdentifier=Z9Y8X7W6V5\nflags=0x10000(runtime)\n" }
        : syntheticSigningRunner(executable, args),
    })).toThrow("belongs to another Apple team");
  });

  test.each([
    ["codesign", "--verify"],
    ["xcrun", "stapler"],
    ["spctl", "--assess"],
  ])("signing attestation fails closed when %s %s rejects the actual package", (tool, argument) => {
    const fixture = makeFixture();
    expect(() => buildSigningAttestation({
      appPath: fixture.appPath,
      notarizationPath: join(fixture.root, "notarization-result.json"),
      notarizedArchivePath: fixture.zipPath,
      sourceSha: SOURCE_SHA,
      teamIdentifier: APPLE_TEAM_ID,
      runCommand: (executable, args) => executable === tool && args[0] === argument
        ? { status: 1, stdout: "", stderr: "synthetic controlled rejection" }
        : syntheticSigningRunner(executable, args),
    })).toThrow("distribution-security verification failed");
  });

  test("signing attestation refuses missing hardened runtime and stripped security controls", () => {
    const fixture = makeFixture();
    expect(() => buildSigningAttestation({
      appPath: fixture.appPath,
      notarizationPath: join(fixture.root, "notarization-result.json"),
      notarizedArchivePath: fixture.zipPath,
      sourceSha: SOURCE_SHA,
      teamIdentifier: APPLE_TEAM_ID,
      runCommand: (executable, args) => executable === "codesign" && args[0] === "-d"
        ? { status: 0, stdout: "", stderr: `TeamIdentifier=${APPLE_TEAM_ID}\nflags=0x0(none)\n` }
        : syntheticSigningRunner(executable, args),
    })).toThrow("lacks hardened runtime");

    const path = join(fixture.root, "signing-result.json");
    const attestation = JSON.parse(readFileSync(path, "utf8"));
    for (const field of ["hardenedRuntime", "stapled", "gatekeeperAccepted"]) {
      json(path, { ...attestation, [field]: false });
      expect(() => buildGateReceipt({
        gateId: "packaged-signing", evidenceClass: "PACKAGED_IDENTITY",
        sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow("requires hardened runtime, a valid staple, and Gatekeeper acceptance");
    }
  });

  test("static inventory can never masquerade as runtime proof", () => {
    expect(() => buildGateReceipt({
      gateId: "packaged-root-frame", evidenceClass: "STATIC_INVENTORY", sourceSha: SOURCE_SHA,
    })).toThrow("requires RUNTIME_HIDDEN");
  });

  test("generated design gate rejects stale, static, forged, or missing exporter proof", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "generated-design-contracts-result.json");
    const original = JSON.parse(readFileSync(path, "utf8"));

    for (const invalid of [
      { sourceSha: "b".repeat(40) },
      { evidenceClass: "STATIC_INVENTORY" },
      { provesRuntimeBehavior: true },
      { generatedBy: "scripts/devtools/invented-exporter.ts" },
      { byteEqual: false },
      { handEditedGeneratedOutput: true },
    ]) {
      json(path, { ...original, ...invalid });
      expect(() => buildGateReceipt({
        gateId: "generated-design-contracts",
        evidenceClass: "UNIT_BEHAVIOR",
        sourceSha: SOURCE_SHA,
        resultPath: path,
        repositoryRoot: fixture.root,
      })).toThrow("exact-source non-GUI exporter byte equality");
    }

    json(path, { ...original, binary: { ...original.binary, path: "script-kit-gpui" } });
    expect(() => buildGateReceipt({
      gateId: "generated-design-contracts", evidenceClass: "UNIT_BEHAVIOR",
      sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
    })).toThrow("exporter binary identity is missing, malformed, or outside the repository");

    const { [GENERATED_BYTE_COMPARE_SOURCE_PATHS[0]]: _missing, ...missingOwner } =
      original.sourceFingerprints;
    json(path, { ...original, sourceFingerprints: missingOwner });
    expect(() => buildGateReceipt({
      gateId: "generated-design-contracts", evidenceClass: "UNIT_BEHAVIOR",
      sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
    })).toThrow("fingerprint every exact declared source owner");
  });

  test("generated design gate requires both actual JSON/CSS bytes and strictly non-GUI cleanup", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "generated-design-contracts-result.json");
    const original = JSON.parse(readFileSync(path, "utf8"));

    json(path, { ...original, outputs: original.outputs.slice(0, 1) });
    expect(() => buildGateReceipt({
      gateId: "generated-design-contracts", evidenceClass: "UNIT_BEHAVIOR",
      sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
    })).toThrow("exactly two distinct output observations");

    const outputPath = GENERATED_BYTE_COMPARE_OUTPUT_PATHS[1];
    json(path, {
      ...original,
      generatedOutputHashes: { ...original.generatedOutputHashes, [outputPath]: "f".repeat(64) },
    });
    expect(() => buildGateReceipt({
      gateId: "generated-design-contracts", evidenceClass: "UNIT_BEHAVIOR",
      sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
    })).toThrow(`checked-in and generated bytes disagree or are missing: ${outputPath}`);

    for (const invalid of [
      { safety: { ...original.safety, startsApplication: true } },
      { safety: { ...original.safety, capturesScreen: true } },
      { safety: { ...original.safety, accessesNetwork: true } },
      { execution: { ...original.execution, exitCode: 1 } },
      { cleanup: { closed: true, survivors: ["stray-exporter"] } },
    ]) {
      json(path, { ...original, ...invalid });
      expect(() => buildGateReceipt({
        gateId: "generated-design-contracts", evidenceClass: "UNIT_BEHAVIOR",
        sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
      })).toThrow("exact-source non-GUI exporter byte equality");
    }
  });

  test("manifest rejects checked-in generated bytes that changed after exporter proof", () => {
    const fixture = makeFixture();
    writeFileSync(fixture.designCssPath, ":root { --synthetic: forged; }\n");

    expect(() => buildReleaseManifest(options(fixture)))
      .toThrow("differ from the exact exporter-verified candidate outputs");
  });

  test("authoritative generated-design raw proof must remain present and byte-identical", () => {
    const tampered = makeFixture();
    writeFileSync(tampered.designProofPath,
      `${readFileSync(tampered.designProofPath, "utf8")}\n`);
    expect(() => buildReleaseManifest(options(tampered)))
      .toThrow("raw exporter evidence was modified after execution");

    const missing = makeFixture();
    rmSync(missing.designProofPath);
    expect(() => buildReleaseManifest(options(missing)))
      .toThrow("raw exporter evidence is missing");
  });

  test("downstream raw-proof verification rechecks all six current exporter-source owners", () => {
    const fixture = makeFixture();
    const sourcePath = GENERATED_BYTE_COMPARE_SOURCE_PATHS[2];
    writeFileSync(join(fixture.root, sourcePath), "stale or tampered owner source");

    expect(() => buildReleaseManifest(options(fixture)))
      .toThrow(`source changed after exporter proof: ${sourcePath}`);
  });

  test("downstream publication revalidates raw proof without requiring the original exporter binary", () => {
    const fixture = makeFixture();
    json(fixture.manifestPath, buildReleaseManifest(options(fixture)));
    rmSync(join(fixture.root, "target-agent/pools/agent-debug/debug/export_design_tokens"));

    const manifest = verifyReleaseManifest(verificationOptions(fixture));
    expect(manifest.design_contracts.raw_evidence.sha256)
      .toBe(hash(readFileSync(fixture.designProofPath, "utf8")));
  });

  test("packaged journeys reject unit fixtures, hidden-root aliases, wrong targets, and unsafe access", () => {
    const fixture = makeFixture();
    for (const journeyId of REQUIRED_PACKAGED_JOURNEYS) {
      const path = join(fixture.root, `${journeyId}-result.json`);
      const original = JSON.parse(readFileSync(path, "utf8"));

      json(path, { ...original, evidenceClass: "UNIT_BEHAVIOR" });
      expect(() => buildGateReceipt({
        gateId: journeyId, evidenceClass: "PACKAGED_APP", sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow("never a unit fixture or hidden-root alias");

      json(path, { ...original, journey: { ...original.journey, id: "packaged-root-frame" } });
      expect(() => buildGateReceipt({
        gateId: journeyId, evidenceClass: "PACKAGED_APP", sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow("its own exact passing packaged user journey");

      json(path, { ...original, safety: { ...original.safety, microphoneAllowed: true } });
      expect(() => buildGateReceipt({
        gateId: journeyId, evidenceClass: "PACKAGED_APP", sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow("no-input/no-capture/no-device/no-provider contract");
    }
  });

  test("packaged journey observations fail closed on invented installs, permissions, migrations, or AI", () => {
    const fixture = makeFixture();
    const invalidCases = [
      ["packaged-first-install", { starterScriptsIndexed: 0 }, "fresh home, bundled SDK"],
      ["packaged-permissions", { permissionRequestsStarted: 1 }, "synthetic denial"],
      ["packaged-migration", { preservedUserDataSha256: "f".repeat(64) }, "exact preserved user bytes"],
      ["packaged-mock-ai", { liveProviderStarts: 1 }, "without a live provider"],
    ] as const;
    for (const [journeyId, invalid, expectedError] of invalidCases) {
      const path = join(fixture.root, `${journeyId}-result.json`);
      const original = JSON.parse(readFileSync(path, "utf8"));
      json(path, {
        ...original,
        journey: {
          ...original.journey,
          observations: { ...original.journey.observations, ...invalid },
        },
      });
      expect(() => buildGateReceipt({
        gateId: journeyId, evidenceClass: "PACKAGED_APP", sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow(expectedError);
    }
  });

  test("direct surface matrices reject aliases, incomplete targets, foreign transactions, or privacy loss", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "packaged-direct-matrix-result.json");
    const original = JSON.parse(readFileSync(path, "utf8"));

    json(path, { ...original, evidenceClass: "STATIC_INVENTORY" });
    expect(() => buildGateReceipt({
      gateId: "packaged-direct-matrix", evidenceClass: "RUNTIME_HIDDEN",
      sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
    })).toThrow("never static or synthetic inventory");

    for (const invalid of [
      { expectedMappings: 1, directProvenMappings: 1 },
      { directProvenMappings: 53 },
      { transactionId: "" },
      { everyTargetDirect: false },
      { sameTransaction: false },
      { privacyRedacted: false },
      { cleanupVerified: false },
    ]) {
      json(path, {
        ...original,
        assurance: {
          ...original.assurance,
          observation: { ...original.assurance.observation, ...invalid },
        },
      });
      expect(() => buildGateReceipt({
        gateId: "packaged-direct-matrix", evidenceClass: "RUNTIME_HIDDEN",
        sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
      })).toThrow("complete target-scoped same-transaction hidden proof");
    }

    for (const targetInvalid of [
      { transactionId: "stale-transaction" },
      { evidenceClass: "STATIC_INVENTORY" },
      { sourceSha: "f".repeat(40) },
      { lifetimeGeneration: 0 },
      { privacyRedacted: false },
    ]) {
      const targets = original.assurance.observation.targets.map(
        (target: Record<string, unknown>, index: number) =>
          index === 0 ? { ...target, ...targetInvalid } : target,
      );
      json(path, {
        ...original,
        assurance: {
          ...original.assurance,
          observation: { ...original.assurance.observation, targets },
        },
      });
      expect(() => buildGateReceipt({
        gateId: "packaged-direct-matrix", evidenceClass: "RUNTIME_HIDDEN",
        sourceSha: SOURCE_SHA, resultPath: path, repositoryRoot: fixture.root,
      })).toThrow("foreign canonical surface/AppView/host/parent, reused state, stale generation");
    }
  });

  test("direct runtime coverage binds all 54 states to canonical kind, view, host, parent, and generations", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "packaged-direct-matrix-result.json");
    const original = JSON.parse(readFileSync(path, "utf8"));
    const popupIndex = original.assurance.observation.targets.findIndex(
      (target: Record<string, unknown>) => target.parentAutomationId !== undefined,
    );
    expect(popupIndex).toBeGreaterThanOrEqual(0);

    for (const [index, mutation] of [
      [0, { surfaceKind: "ForeignSurface" }],
      [0, { appViewVariant: "WrongAppView" }],
      [0, { hostKind: "attachedPopup" }],
      [0, { windowKind: "Notes" }],
      [0, { windowInstanceId: "foreign-window@1" }],
      [0, { windowGeneration: 0 }],
      [0, { targetGeneration: 0 }],
      [0, { surfaceGeneration: 0 }],
      [0, { targetId: "foreign-automation-target" }],
      [popupIndex, { parentAutomationId: "" }],
      [popupIndex, { parentAutomationId: "foreign-parent" }],
    ] as Array<[number, Record<string, unknown>]>) {
      const targets = original.assurance.observation.targets.map(
        (target: Record<string, unknown>, targetIndex: number) =>
          targetIndex === index ? { ...target, ...mutation } : target,
      );
      json(path, {
        ...original,
        assurance: {
          ...original.assurance,
          observation: { ...original.assurance.observation, targets },
        },
      });
      expect(() => buildGateReceipt({
        gateId: "packaged-direct-matrix",
        evidenceClass: "RUNTIME_HIDDEN",
        sourceSha: SOURCE_SHA,
        repositoryRoot: fixture.root,
        resultPath: path,
      })).toThrow(/canonical surface\/AppView\/host\/parent|exact canonical target\/window\/parent/);
    }
  });

  test("direct runtime coverage refuses invented, missing, stale, unsafe, or foreign primitive receipts", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "packaged-direct-matrix-result.json");
    const original = JSON.parse(readFileSync(path, "utf8"));

    const invalidReceipts: Array<(receipts: Array<Record<string, any>>) => void> = [
      (receipts) => { receipts.splice(0, 1); },
      (receipts) => { receipts[0]!.receipt.evidenceClass = "STATIC_INVENTORY"; },
      (receipts) => {
        receipts[0]!.receipt.transaction.transactionId = "foreign-transaction";
      },
      (receipts) => { receipts[0]!.receipt.repository.gitCommit = "f".repeat(40); },
      (receipts) => {
        receipts[0]!.receipt.transaction.binarySha256 = "f".repeat(64);
        receipts[0]!.receipt.binary.sha256 = "f".repeat(64);
      },
      (receipts) => { receipts[0]!.receipt.privacy.rawContentReturned = true; },
      (receipts) => { receipts[0]!.receipt.cleanup.closed = false; },
      (receipts) => { receipts[0]!.receipt.producerValidation.valid = false; },
      (receipts) => { receipts[0]!.receipt.receiptId = receipts[1]!.receipt.receiptId; },
      (receipts) => { receipts[0]!.path = receipts[1]!.path; },
      (receipts) => { receipts[0]!.receipt.transaction.surfaceGeneration += 1; },
    ];

    for (const invalidate of invalidReceipts) {
      const primitiveReceipts = structuredClone(original.assurance.observation.primitiveReceipts);
      invalidate(primitiveReceipts);
      json(path, {
        ...original,
        assurance: {
          ...original.assurance,
          observation: { ...original.assurance.observation, primitiveReceipts },
        },
      });
      expect(() => buildGateReceipt({
        gateId: "packaged-direct-matrix",
        evidenceClass: "RUNTIME_HIDDEN",
        sourceSha: SOURCE_SHA,
        repositoryRoot: fixture.root,
        resultPath: path,
      })).toThrow(/primitive evidence|canonical runtime scorecard|canonical target\/window\/parent/);
    }

    json(path, {
      ...original,
      assurance: {
        ...original.assurance,
        observation: { ...original.assurance.observation, primitiveReceipts: [] },
      },
    });
    expect(() => buildGateReceipt({
      gateId: "packaged-direct-matrix",
      evidenceClass: "RUNTIME_HIDDEN",
      sourceSha: SOURCE_SHA,
      repositoryRoot: fixture.root,
      resultPath: path,
    })).toThrow("actual authoritative target-scoped primitive runtime receipts");
  });

  test("direct runtime proof rejects foreign authoritative generated-contract bytes", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "packaged-direct-matrix-result.json");
    writeFileSync(fixture.contractsPath, '{"schemaVersion":1,"entries":[]}\n');

    expect(() => buildGateReceipt({
      gateId: "packaged-direct-matrix",
      evidenceClass: "RUNTIME_HIDDEN",
      sourceSha: SOURCE_SHA,
      repositoryRoot: fixture.root,
      resultPath: path,
    })).toThrow("exact independently loaded candidate source artifact");
  });

  test("ratified performance rejects hidden aliases, fake paint, missing owner approval, and bad samples", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "packaged-ratified-performance-result.json");
    const original = JSON.parse(readFileSync(path, "utf8"));

    for (const invalid of [
      { observationLayer: "STATE_ECHO" },
      { measuresPaint: false },
      { ownerRatified: false },
      { ratifiedBudgetId: "" },
      { ratificationReference: "" },
      { sampleCount: 0 },
      { samplesMs: [] },
      { paintEvidence: { source: "state_echo", presentedFrameCount: 30 } },
    ]) {
      json(path, {
        ...original,
        assurance: {
          ...original.assurance,
          observation: { ...original.assurance.observation, ...invalid },
        },
      });
      expect(() => buildGateReceipt({
        gateId: "packaged-ratified-performance", evidenceClass: "RUNTIME_VISIBLE",
        sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow("actual painted-output observations, owner-ratified budget");
    }

    json(path, {
      ...original,
      assurance: {
        ...original.assurance,
        observation: { ...original.assurance.observation, p95Ms: 51 },
      },
    });
    expect(() => buildGateReceipt({
      gateId: "packaged-ratified-performance", evidenceClass: "RUNTIME_VISIBLE",
      sourceSha: SOURCE_SHA, resultPath: path,
    })).toThrow("p95Ms must be observed and satisfy its explicitly ratified budget");

    json(path, {
      ...original,
      safety: { ...original.safety, ownerVisibleAuthorization: false },
    });
    expect(() => buildGateReceipt({
      gateId: "packaged-ratified-performance", evidenceClass: "RUNTIME_VISIBLE",
      sourceSha: SOURCE_SHA, resultPath: path,
    })).toThrow("separate explicit owner authorization");
  });

  test("release readiness reports every missing packaged journey as UNMEASURED rather than green", () => {
    const fixture = makeFixture();
    const emptyDirectory = join(fixture.root, "empty-evidence");
    mkdirSync(emptyDirectory, { recursive: true });

    const blocked = assessPackagedJourneyReadiness({
      appPath: fixture.appPath,
      evidenceDirectory: emptyDirectory,
      sourceSha: SOURCE_SHA,
    });
    expect(blocked.status).toBe("blocked");
    expect(blocked.missingJourneys).toEqual([...REQUIRED_PACKAGED_JOURNEYS]);
    expect(blocked.journeys.every((journey) => journey.status === "unmeasured")).toBe(true);
    expect(blocked.missingAssurances).toEqual(["direct_matrix", "ratified_perf"]);
    expect(blocked.assurances.every((assurance) => assurance.status === "unmeasured")).toBe(true);

    const complete = assessPackagedJourneyReadiness({
      appPath: fixture.appPath,
      evidenceDirectory: join(fixture.root, "evidence"),
      sourceSha: SOURCE_SHA,
    });
    expect(complete.status).toBe("pass");
    expect(complete.missingJourneys).toEqual([]);
    expect(complete.missingAssurances).toEqual([]);
  });

  test("wrong-binary packaged readiness stays explicitly blocked", () => {
    const fixture = makeFixture();
    const path = fixture.gatePaths.get("packaged-mock-ai")!;
    const receipt = JSON.parse(readFileSync(path, "utf8"));
    receipt.result.binarySha256 = "f".repeat(64);
    json(path, receipt);

    const readiness = assessPackagedJourneyReadiness({
      appPath: fixture.appPath,
      evidenceDirectory: join(fixture.root, "evidence"),
      sourceSha: SOURCE_SHA,
    });
    expect(readiness.status).toBe("blocked");
    expect(readiness.invalidJourneys).toEqual(["packaged-mock-ai"]);
  });

  test("wrong-candidate direct or owner-ratified assurances remain explicitly blocked", () => {
    for (const requirement of REQUIRED_PACKAGED_ASSURANCES) {
      const fixture = makeFixture();
      const path = fixture.gatePaths.get(requirement.gateId)!;
      const receipt = JSON.parse(readFileSync(path, "utf8"));
      receipt.result.sidecarSha256 = "f".repeat(64);
      json(path, receipt);

      const readiness = assessPackagedJourneyReadiness({
        appPath: fixture.appPath,
        evidenceDirectory: join(fixture.root, "evidence"),
        sourceSha: SOURCE_SHA,
      });
      expect(readiness.status).toBe("blocked");
      expect(readiness.invalidAssurances).toEqual([requirement.id]);
    }
  });

  test("publication CLI writes its honest blocked scorecard before refusing missing journeys", () => {
    const fixture = makeFixture();
    const emptyDirectory = join(fixture.root, "empty-packaged-evidence");
    const scorecardPath = join(fixture.root, "blocked-release-scorecard.json");
    mkdirSync(emptyDirectory, { recursive: true });

    const result = Bun.spawnSync({
      cmd: [
        "bun", "scripts/release-evidence.ts", "packaged-readiness",
        "--app", fixture.appPath,
        "--evidence-dir", emptyDirectory,
        "--source-sha", SOURCE_SHA,
        "--output", scorecardPath,
      ],
      cwd: resolve(import.meta.dir, ".."),
      env: {
        ...process.env,
        SCRIPT_KIT_NONINTERACTIVE: "1",
        SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
        SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
        SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
        SCRIPT_KIT_ALLOW_LIVE_AI: "0",
      },
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode).not.toBe(0);
    expect(
      result.stderr.toString(),
      `blocked publication exited ${result.exitCode} without an inspectable failure`,
    ).toContain("6 UNMEASURED");
    const scorecard = JSON.parse(readFileSync(scorecardPath, "utf8"));
    expect(scorecard.status).toBe("blocked");
    expect(scorecard.missingJourneys).toEqual([...REQUIRED_PACKAGED_JOURNEYS]);
    expect(scorecard.missingAssurances).toEqual(["direct_matrix", "ratified_perf"]);
    expect(scorecard.journeys.every((journey: { status: string }) =>
      journey.status === "unmeasured")).toBe(true);
  });

  test("runtime proof rejects visible, native-input, or leaked-process receipts", () => {
    const fixture = makeFixture();
    const path = join(fixture.root, "runtime-result.json");
    const base = JSON.parse(readFileSync(path, "utf8"));

    for (const forbiddenField of ["windowRevealAllowed", "windowFocusAllowed", "nativeInputAllowed", "screenCaptureAllowed"]) {
      json(path, { ...base, safety: { ...base.safety, [forbiddenField]: true } });
      expect(() => buildGateReceipt({
        gateId: "packaged-root-frame", evidenceClass: "RUNTIME_HIDDEN", sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow("nonintrusive hidden-window safety");
    }

    for (const requiredField of ["startsApplication", "isolatedCiLaunchAuthorized"]) {
      json(path, { ...base, safety: { ...base.safety, [requiredField]: false } });
      expect(() => buildGateReceipt({
        gateId: "packaged-root-frame", evidenceClass: "RUNTIME_HIDDEN", sourceSha: SOURCE_SHA, resultPath: path,
      })).toThrow("nonintrusive hidden-window safety");
    }

    json(path, { ...base, cleanup: { hidden: true, closed: false } });
    expect(() => buildGateReceipt({
      gateId: "packaged-root-frame", evidenceClass: "RUNTIME_HIDDEN", sourceSha: SOURCE_SHA, resultPath: path,
    })).toThrow("cleanly hide and terminate");
  });

  test("rejects a missing, nonexecutable, or changed Pi sidecar", () => {
    const fixture = makeFixture();
    chmodSync(fixture.sidecarPath, 0o644);
    expect(() => buildReleaseManifest(options(fixture))).toThrow("not executable");

    chmodSync(fixture.sidecarPath, 0o755);
    const manifest = buildReleaseManifest(options(fixture));
    json(fixture.manifestPath, manifest);
    writeFileSync(fixture.sidecarPath, "different-sidecar");

    expect(() => verifyReleaseManifest(verificationOptions(fixture)))
      .toThrow("Pi sidecar no longer matches");
  });

  test("rejects every missing required gate", () => {
    const fixture = makeFixture();

    for (const gateId of fixture.gatePaths.keys()) {
      const remaining = [...fixture.gatePaths.entries()]
        .filter(([candidate]) => candidate !== gateId)
        .map(([, path]) => path);

      expect(() => buildReleaseManifest({ ...options(fixture), evidencePaths: remaining }))
        .toThrow(`missing required release gate: ${gateId}`);
    }
  });

  test("rejects duplicate, stale-source, or failed gate receipts", () => {
    const fixture = makeFixture();
    const first = fixture.gatePaths.get("rust-tests")!;

    expect(() => buildReleaseManifest({
      ...options(fixture), evidencePaths: [...fixture.gatePaths.values(), first],
    })).toThrow("duplicate release gate");

    const receipt = JSON.parse(readFileSync(first, "utf8"));
    receipt.sourceSha = "b".repeat(40);
    json(first, receipt);
    expect(() => buildReleaseManifest(options(fixture))).toThrow("another source revision");

    receipt.sourceSha = SOURCE_SHA;
    receipt.status = "fail";
    json(first, receipt);
    expect(() => buildReleaseManifest(options(fixture))).toThrow("did not pass");
  });

  test("publication rejects absent, forged, or explicitly dirty gate-source provenance", () => {
    for (const [invalid, expectedError] of [
      [{ sourceState: undefined }, "dirty diagnostic evidence must remain nonpublishable"],
      [{ publishable: false }, "clean release evidence must be publishable"],
      [{ sourceState: "dirty", publishable: false },
        "dirty diagnostic evidence must remain nonpublishable"],
    ] as const) {
      const fixture = makeFixture();
      const path = fixture.gatePaths.get("rust-tests")!;
      const receipt = JSON.parse(readFileSync(path, "utf8"));
      json(path, { ...receipt, ...invalid });
      expect(() => buildReleaseManifest(options(fixture))).toThrow(expectedError);
    }

    const fixture = makeFixture();
    const path = fixture.gatePaths.get("rust-tests")!;
    const receipt = JSON.parse(readFileSync(path, "utf8"));
    const worktreeOwners = [{ path: "src/lib.rs", sha256: hash("dirty-source-owner") }];
    const dirty = {
      ...receipt,
      sourceState: "dirty",
      publishable: false,
      fingerprintScope: "DECLARED_OWNERS_NON_EXHAUSTIVE",
      worktreeOwners,
      worktreeFingerprintSha256: hash(JSON.stringify({
        sourceSha: SOURCE_SHA,
        gateId: "rust-tests",
        fingerprintScope: "DECLARED_OWNERS_NON_EXHAUSTIVE",
        owners: worktreeOwners,
      })),
    };
    json(path, dirty);
    expect(() => buildReleaseManifest(options(fixture)))
      .toThrow("nonpublishable dirty-worktree diagnostic evidence");
  });

  test("rejects a complete same-candidate matrix when any target is not canonical", () => {
    const fixture = makeFixture();
    const resultPath = join(fixture.root, "packaged-direct-matrix-result.json");
    const original = JSON.parse(readFileSync(resultPath, "utf8"));
    const targets = original.assurance.observation.targets.map(
      (target: Record<string, unknown>, index: number) => index === 0
        ? { ...target, bindingId: "ForeignSurface::ForgedVariant@MainWindow" }
        : target,
    );
    json(resultPath, {
      ...original,
      assurance: {
        ...original.assurance,
        observation: { ...original.assurance.observation, targets },
      },
    });
    expect(() => buildGateReceipt({
      gateId: "packaged-direct-matrix",
      evidenceClass: "RUNTIME_HIDDEN",
      sourceSha: SOURCE_SHA,
      resultPath,
      repositoryRoot: fixture.root,
    })).toThrow("foreign canonical surface/AppView/host/parent, reused state, stale generation");
  });

  test("rejects fabricated zero-count, incomplete-suite, and skipped behavior receipts", () => {
    const invalidCases = [
      ["rust-tests", { passed: 0 }, "nonzero, passing executed Rust"],
      ["domain-tests", { failed: 1 }, "nonzero, passing executed Rust"],
      ["integration-tests", { suites: RELEASE_INTEGRATION_SUITES.length - 1 },
        "every exact nonintrusive suite"],
      ["permissions-fixtures", { passed: 2 }, "exactly one passing"],
      ["proof-contracts", { assertions: 0 }, "nonzero passing tests, assertions"],
      ["sdk-tests", { skipped: 1 }, "zero failures or skipped tests"],
    ] as const;

    for (const [gateId, invalid, expectedError] of invalidCases) {
      const fixture = makeFixture();
      const path = fixture.gatePaths.get(gateId)!;
      const receipt = JSON.parse(readFileSync(path, "utf8"));
      receipt.result = { ...receipt.result, ...invalid };
      json(path, receipt);

      expect(() => buildReleaseManifest(options(fixture))).toThrow(expectedError);
    }
  });

  test("static inventories reject invented behavior and receipts must remain canonical", () => {
    const staticFixture = makeFixture();
    const staticPath = staticFixture.gatePaths.get("consistency-catalog")!;
    const staticReceipt = JSON.parse(readFileSync(staticPath, "utf8"));
    staticReceipt.result = { passed: 1, failed: 0, skipped: 0 };
    json(staticPath, staticReceipt);
    expect(() => buildReleaseManifest(options(staticFixture)))
      .toThrow("static inventory and cannot contain invented behavior");

    const encodingFixture = makeFixture();
    const path = encodingFixture.gatePaths.get("rust-tests")!;
    writeFileSync(path, JSON.stringify(JSON.parse(readFileSync(path, "utf8"))));
    expect(() => buildReleaseManifest(options(encodingFixture)))
      .toThrow("not an immutable canonically encoded release receipt");
  });

  test("publication verification binds each embedded result to its exact executed receipt hash", () => {
    const fixture = makeFixture();
    const manifest = buildReleaseManifest(options(fixture));
    const sdkGate = manifest.verification.gates.find((gate) => gate.gateId === "sdk-tests")!;
    sdkGate.result!.passed! += 1;
    json(fixture.manifestPath, manifest);

    expect(() => verifyReleaseManifest(verificationOptions(fixture)))
      .toThrow("manifest evidence was modified after execution: sdk-tests");
  });

  test("rejects runtime proof collected from a sibling or stale binary", () => {
    const fixture = makeFixture();
    const runtimePath = fixture.gatePaths.get("packaged-root-frame")!;
    const receipt = JSON.parse(readFileSync(runtimePath, "utf8"));
    receipt.result.binarySha256 = "f".repeat(64);
    json(runtimePath, receipt);

    expect(() => buildReleaseManifest(options(fixture))).toThrow("different application binary");
  });

  test("rejects signing evidence collected from a sibling application or sidecar", () => {
    const fixture = makeFixture();
    const signingPath = fixture.gatePaths.get("packaged-signing")!;
    const receipt = JSON.parse(readFileSync(signingPath, "utf8"));

    receipt.result.sidecarSha256 = "f".repeat(64);
    json(signingPath, receipt);
    expect(() => buildReleaseManifest(options(fixture))).toThrow("different application executable or Pi sidecar");
  });

  test("publication verifier rejects changed archives and stripped proof", () => {
    const fixture = makeFixture();
    const manifest = buildReleaseManifest(options(fixture));
    json(fixture.manifestPath, manifest);

    expect(verifyReleaseManifest(verificationOptions(fixture)).verification.gates)
      .toHaveLength(19);

    writeFileSync(fixture.zipPath, "changed-release-archive");
    expect(() => verifyReleaseManifest(verificationOptions(fixture)))
      .toThrow("SHA-256 does not match");
  });

  test("manifest refuses plain text masquerading as the signed release ZIP", () => {
    const fixture = makeFixture();
    writeFileSync(fixture.zipPath, "signed-notarized-zipped-app");

    expect(() => buildReleaseManifest(options(fixture)))
      .toThrow("not a valid ZIP");
  });

  test("downstream verification inspects deflated descriptor-backed ZIP members without an app", () => {
    const fixture = makeFixture();
    writeSyntheticZip(fixture.zipPath, syntheticAppMembers(fixture.appPath).map((member) => ({
      ...member,
      compression: 8,
      usesDataDescriptor: true,
    })));
    const manifest = buildReleaseManifest(options(fixture));
    json(fixture.manifestPath, manifest);
    const { appPath: _appPath, ...downstream } = verificationOptions(fixture);

    expect(verifyReleaseManifest(downstream).bundle.info_plist.sha256)
      .toBe(hash(readFileSync(join(fixture.appPath, "Contents/Info.plist"))));
  });

  test("the production macOS ditto archive format passes capture-free publication verification", () => {
    const fixture = makeFixture();
    const frameworks = join(fixture.appPath, "Contents/Frameworks");
    mkdirSync(join(frameworks, "Versions/A"), { recursive: true });
    writeFileSync(join(frameworks, "Versions/A/framework.dylib"), "signed framework");
    symlinkSync("Versions/A/framework.dylib", join(frameworks, "framework.dylib"));
    if (process.platform === "darwin") {
      const result = spawnSync("/usr/bin/ditto", [
        "-c",
        "-k",
        "--keepParent",
        fixture.appPath,
        fixture.zipPath,
      ], { encoding: "utf8" });
      expect(result.status).toBe(0);
      expect(result.stderr).toBe("");
    } else {
      writeSyntheticZip(fixture.zipPath, syntheticAppMembers(fixture.appPath));
    }
    const manifest = buildReleaseManifest(options(fixture));
    json(fixture.manifestPath, manifest);
    const { appPath: _appPath, ...downstream } = verificationOptions(fixture);

    expect(verifyReleaseManifest(downstream).bundle.identifier).toBe("com.scriptkit.app");
  });

  test("manifest binds all four actual archived application members to the verified app", () => {
    const fixture = makeFixture();
    const cases = [
      { suffix: "/script-kit-gpui", expected: "ZIP executable" },
      { suffix: "/pi", expected: "ZIP Pi sidecar" },
      { suffix: "/kit-sdk.ts", expected: "ZIP SDK" },
      { suffix: "/Info.plist", expected: "ZIP Info.plist" },
    ];

    for (const candidate of cases) {
      const members = syntheticAppMembers(fixture.appPath).map((member) =>
        member.path.endsWith(candidate.suffix)
          ? { ...member, contents: "different archived bytes" }
          : member);
      writeSyntheticZip(fixture.zipPath, members);
      expect(() => buildReleaseManifest(options(fixture))).toThrow(candidate.expected);
    }
  });

  test("publication refuses substituted ZIP members even when archive hash and size are updated", () => {
    const fixture = makeFixture();
    const substitutions = [
      { suffix: "/script-kit-gpui", expected: "ZIP executable" },
      { suffix: "/pi", expected: "ZIP Pi sidecar" },
      { suffix: "/kit-sdk.ts", expected: "ZIP SDK" },
      { suffix: "/Info.plist", expected: "ZIP Info.plist" },
    ];
    const originalMembers = syntheticAppMembers(fixture.appPath);
    const baseline = buildReleaseManifest(options(fixture));
    const { appPath: _appPath, ...downstream } = verificationOptions(fixture);

    for (const substitution of substitutions) {
      writeSyntheticZip(fixture.zipPath, originalMembers.map((member) =>
        member.path.endsWith(substitution.suffix)
          ? { ...member, contents: `${Buffer.from(member.contents)}<!--tampered-->` }
          : member));
      const manifest = structuredClone(baseline);
      manifest.artifacts[0].sha256 = hash(readFileSync(fixture.zipPath));
      manifest.artifacts[0].size_bytes = statSync(fixture.zipPath).size;
      json(fixture.manifestPath, manifest);

      expect(() => verifyReleaseManifest(downstream)).toThrow(substitution.expected);
    }
  });

  test("publication binds every signed migration, resource, signature, and file mode", () => {
    const fixture = makeFixture();
    const originalMembers = syntheticAppMembers(fixture.appPath);
    const baseline = buildReleaseManifest(options(fixture));
    const { appPath: _appPath, ...downstream } = verificationOptions(fixture);
    const cases: Array<{ members: SyntheticZipMember[]; expected: string }> = [
      {
        members: originalMembers.map((member) => member.path.endsWith("/migrate/cli.ts")
          ? { ...member, contents: "export const privateMigration = 'malicious';" }
          : member),
        expected: "complete signed application tree",
      },
      {
        members: originalMembers.map((member) => member.path.endsWith("/assets/icon.svg")
          ? { ...member, contents: "<svg>replacement icon</svg>" }
          : member),
        expected: "complete signed application tree",
      },
      {
        members: originalMembers.filter((member) => !member.path.endsWith("/migrate/cli.ts")),
        expected: "complete signed application tree",
      },
      {
        members: [...originalMembers, {
          path: "Script Kit.app/Contents/Resources/scripts/migrate/injected.ts",
          contents: "export const stolen = true;",
          unixMode: 0o100644,
        }],
        expected: "complete signed application tree",
      },
      {
        members: originalMembers.map((member) => member.path.endsWith("/script-kit-gpui")
          ? { ...member, unixMode: 0o100644 }
          : member),
        expected: "complete signed application tree",
      },
      {
        members: originalMembers.map((member) => member.path.endsWith("/_CodeSignature/CodeResources")
          ? { ...member, contents: "replacement signature envelope" }
          : member),
        expected: "complete signed application tree",
      },
      {
        members: originalMembers.filter((member) =>
          !member.path.endsWith("/_CodeSignature/CodeResources")),
        expected: "missing its signed CodeResources envelope",
      },
    ];

    for (const candidate of cases) {
      writeSyntheticZip(fixture.zipPath, candidate.members);
      const manifest = structuredClone(baseline);
      manifest.artifacts[0].sha256 = hash(readFileSync(fixture.zipPath));
      manifest.artifacts[0].size_bytes = statSync(fixture.zipPath).size;
      json(fixture.manifestPath, manifest);
      expect(() => verifyReleaseManifest(downstream)).toThrow(candidate.expected);
    }
  });

  test("publication rejects rewritten framework symlink targets and missing tree attestations", () => {
    const fixture = makeFixture();
    const frameworks = join(fixture.appPath, "Contents/Frameworks");
    mkdirSync(frameworks, { recursive: true });
    symlinkSync("Versions/A/framework.dylib", join(frameworks, "framework.dylib"));
    const originalMembers = syntheticAppMembers(fixture.appPath);
    writeSyntheticZip(fixture.zipPath, originalMembers);
    const baseline = buildReleaseManifest(options(fixture));
    const { appPath: _appPath, ...downstream } = verificationOptions(fixture);

    writeSyntheticZip(fixture.zipPath, originalMembers.map((member) =>
      member.path.endsWith("/Frameworks/framework.dylib")
        ? { ...member, contents: "../../../../external.dylib" }
        : member));
    const tampered = structuredClone(baseline);
    tampered.artifacts[0].sha256 = hash(readFileSync(fixture.zipPath));
    tampered.artifacts[0].size_bytes = statSync(fixture.zipPath).size;
    json(fixture.manifestPath, tampered);
    expect(() => verifyReleaseManifest(downstream)).toThrow("complete signed application tree");

    writeSyntheticZip(fixture.zipPath, originalMembers);
    const missing = structuredClone(baseline) as unknown as { bundle: Record<string, unknown> };
    delete missing.bundle.content_tree;
    json(fixture.manifestPath, missing);
    expect(() => verifyReleaseManifest(downstream))
      .toThrow("missing the exact signed application content-tree identity");
  });

  test("release ZIP rejects missing, duplicate, foreign-root, traversal, and local-header entries", () => {
    const fixture = makeFixture();
    const members = syntheticAppMembers(fixture.appPath);
    const cases: Array<{ members: SyntheticZipMember[]; expected: string }> = [
      { members: members.slice(1), expected: "missing its required application member" },
      { members: [...members, { ...members[0] }], expected: "duplicate or aliased" },
      {
        members: members.map((member) => ({
          ...member,
          path: member.path.replace("Script Kit.app", "Another.app"),
        })),
        expected: "unexpected top-level application root",
      },
      {
        members: [...members, { path: "Script Kit.app/../escape", contents: "escape" }],
        expected: "unsafe member path",
      },
      {
        members: members.map((member, index) => index === 0
          ? { ...member, localPath: "Script Kit.app/Contents/MacOS/other" }
          : member),
        expected: "local member path disagrees",
      },
    ];

    for (const candidate of cases) {
      writeSyntheticZip(fixture.zipPath, candidate.members);
      expect(() => buildReleaseManifest(options(fixture))).toThrow(candidate.expected);
    }
  });

  test("required ZIP members and their ancestors cannot be symlinks", () => {
    const fixture = makeFixture();
    const members = syntheticAppMembers(fixture.appPath);
    writeSyntheticZip(fixture.zipPath, members.map((member, index) => index === 0
      ? { ...member, unixMode: 0o120777 }
      : member));
    expect(() => buildReleaseManifest(options(fixture))).toThrow("not a regular file");

    writeSyntheticZip(fixture.zipPath, [
      { path: "Script Kit.app/Contents/MacOS/", contents: "outside", unixMode: 0o120777 },
      ...members,
    ]);
    expect(() => buildReleaseManifest(options(fixture)))
      .toThrow("traverses a non-directory or symlink");

    const frameworks = join(fixture.appPath, "Contents/Frameworks");
    mkdirSync(frameworks, { recursive: true });
    symlinkSync("Versions/A/lib-framework.dylib", join(frameworks, "lib-framework.dylib"));
    writeSyntheticZip(fixture.zipPath, syntheticAppMembers(fixture.appPath));
    expect(buildReleaseManifest(options(fixture)).bundle.identifier).toBe("com.scriptkit.app");
  });

  test("case-insensitive and Unicode-normalized ancestor symlinks cannot redirect extraction", () => {
    const fixture = makeFixture();
    const members = syntheticAppMembers(fixture.appPath);
    writeSyntheticZip(fixture.zipPath, [
      { path: "Script Kit.app/contents/", contents: "outside", unixMode: 0o120777 },
      ...members,
    ]);
    expect(() => buildReleaseManifest(options(fixture)))
      .toThrow("traverses a non-directory or symlink");

    const unicodeDirectory = join(fixture.appPath, "Contents/Resources/caf\u00e9");
    mkdirSync(unicodeDirectory, { recursive: true });
    writeFileSync(join(unicodeDirectory, "private.txt"), "signed Unicode resource");
    writeSyntheticZip(fixture.zipPath, [
      {
        path: "Script Kit.app/Contents/Resources/cafe\u0301/",
        contents: "outside",
        unixMode: 0o120777,
      },
      ...syntheticAppMembers(fixture.appPath),
    ]);
    expect(() => buildReleaseManifest(options(fixture)))
      .toThrow("traverses a non-directory or symlink");
  });

  test("scorecard distinguishes hidden packaged behavior from unmeasured painted latency", () => {
    const fixture = makeFixture();
    const scorecard = buildReleaseScorecard(buildReleaseManifest(options(fixture)));

    expect(scorecard.gates).toHaveLength(19);
    expect(scorecard.distributionSecurity).toEqual({
      teamIdentifier: APPLE_TEAM_ID,
      notarizationSubmissionId: NOTARIZATION_ID,
      notarizedArchiveSha256: hash(readFileSync(fixture.zipPath)),
      hardenedRuntime: true,
      stapled: true,
      gatekeeperAccepted: true,
    });
    expect(scorecard.journeys[0]).toEqual({
      id: "packaged-root-frame",
      evidenceClass: "RUNTIME_HIDDEN",
      status: "pass",
      metricKind: "semantic_frame_identity",
      measuresPaint: false,
      startsApplication: true,
      isolatedCiLaunchAuthorized: true,
      revealsWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
    });
    expect(scorecard.journeys.slice(1).map((journey) => [journey.id, journey.evidenceClass]))
      .toEqual(REQUIRED_PACKAGED_JOURNEYS.map((id) => [id, "PACKAGED_APP"]));
    expect(scorecard.paintedLatency).toEqual({
      status: "pass",
      evidenceClass: "RUNTIME_VISIBLE",
      metricKind: "PAINTED_OUTPUT",
      p50Ms: 12,
      p95Ms: 28,
      maxMs: 70,
      sampleCount: 30,
      budgetRatified: true,
      ownerVisibleAuthorization: true,
      ratifiedBudgetId: "owner-approved-release-latency-v1",
      ratificationReference: "owner-review:2026-08-22/release-latency-v1",
    });
    expect(scorecard.directSurfaceCoverage).toMatchObject({
      status: "pass",
      evidenceClass: "RUNTIME_HIDDEN",
      expectedMappings: 54,
      directProvenMappings: 54,
      transactionId: "synthetic-same-candidate-transaction",
    });
    expect(scorecard.gates.find((gate) => gate.gateId === "rust-tests")?.passed).toBe(32);
    expect(scorecard.gates.find((gate) => gate.gateId === "integration-tests")?.suiteNames)
      .toEqual([...RELEASE_INTEGRATION_SUITES].sort());
    expect(scorecard.gates.find((gate) => gate.gateId === "proof-contracts")?.assertions)
      .toBe(1112);
  });
});

describe("nonintrusive executed Rust verification", () => {
  function runVerify(args: string[], extraEnv: Record<string, string> = {}) {
    const fixture = makeFixture();
    const cargoPath = join(fixture.root, "fake-cargo");
    const logPath = join(fixture.root, "cargo.log");
    const testLogPath = join(fixture.root, "test-output.log");
    const receiptPath = join(fixture.root, "test-evidence.json");
    writeFileSync(cargoPath,
      '#!/bin/sh\nprintf "%s\\n" "$*" >> "$VERIFY_CARGO_LOG"\n' +
      'if [ "${VERIFY_FAKE_INTEGRATION:-0}" = "1" ]; then\n' +
      `  for suite in ${RELEASE_INTEGRATION_SUITES.join(" ")}; do\n` +
      '    printf "     Running tests/%s.rs (target/debug/deps/%s)\\n" "$suite" "$suite"\n' +
      '    printf "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\\n"\n' +
      '  done\n' +
      'elif [ "${VERIFY_FAKE_FIXTURE:-0}" = "1" ]; then\n' +
      '  case "$*" in\n' +
      '    *setup::tests::test_fresh_install_seeds_canonical_menu_syntax_handlers*)\n' +
      '      printf "test setup::tests::test_fresh_install_seeds_canonical_menu_syntax_handlers ... ok\\n" ;;\n' +
      '    *permissions_wizard::tests::test_snapshot_missing_required*)\n' +
      '      printf "test permissions_wizard::tests::test_snapshot_missing_required ... ok\\n" ;;\n' +
      '    *ai_reliability::model_tests::blocked_capability_produces_actionable_recovery_without_starting*)\n' +
      '      printf "test ai_reliability::model_tests::blocked_capability_produces_actionable_recovery_without_starting ... ok\\n" ;;\n' +
      '    *ai::reliability::tests::redactor_allowlists_json_masks_secrets_paths_and_bounds_output*)\n' +
      '      printf "test ai::reliability::tests::redactor_allowlists_json_masks_secrets_paths_and_bounds_output ... ok\\n" ;;\n' +
      '  esac\n' +
      '  printf "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out\\n"\n' +
      'elif [ "${VERIFY_FAKE_SUMMARY:-0}" = "1" ]; then\n' +
      '  printf "test result: ok. 7 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\\n"\n' +
      'fi\nexit "${VERIFY_FAKE_EXIT:-0}"\n',
      { mode: 0o755 });

    const recordResults = extraEnv.VERIFY_CAPTURE_RESULTS === "1";
    const fakeGit = recordResults || Boolean(extraEnv.VERIFY_FAKE_UNTRACKED_PATH) ||
      extraEnv.VERIFY_FAKE_GIT_DIRTY === "1";
    let commandPath = process.env.PATH ?? "";
    if (fakeGit) {
      writeFileSync(join(fixture.root, "git"),
        '#!/bin/sh\n' +
        'case "$3" in\n' +
        '  rev-parse) printf \'%s\\n\' "$VERIFY_FAKE_GIT_HEAD" ;;\n' +
        '  ls-files) shift 4; for candidate in "$@"; do\n' +
        '    if [ "$candidate" = "${VERIFY_FAKE_UNTRACKED_PATH:-}" ]; then exit 1; fi\n' +
        '  done; exit 0 ;;\n' +
        '  diff) [ "${VERIFY_FAKE_GIT_DIRTY:-0}" != "1" ] ;;\n' +
        '  *) exit 97 ;;\n' +
        'esac\n',
        { mode: 0o755 });
      commandPath = `${fixture.root}:${commandPath}`;
    }

    const result = Bun.spawnSync({
      cmd: ["bash", "scripts/verify.sh", "--skip-bundle", ...args],
      cwd: resolve(import.meta.dir, ".."),
      env: {
        ...process.env,
        SCRIPT_KIT_CARGO: cargoPath,
        SCRIPT_KIT_REQUIRE_CLEAN_SOURCE: "0",
        SCRIPT_KIT_VERIFY_RECEIPT: recordResults ? receiptPath : "",
        SCRIPT_KIT_SDK_TEST_RECEIPT: "",
        SCRIPT_KIT_VERIFY_TEST_LOG: recordResults ? testLogPath : "",
        VERIFY_CARGO_LOG: logPath,
        VERIFY_FAKE_GIT_HEAD: extraEnv.GITHUB_SHA ?? process.env.GITHUB_SHA ?? SOURCE_SHA,
        GITHUB_SHA: extraEnv.GITHUB_SHA ?? process.env.GITHUB_SHA ?? SOURCE_SHA,
        PATH: commandPath,
        ...extraEnv,
      },
      stdout: "pipe",
      stderr: "pipe",
    });

    const output = `${result.stdout.toString()}${result.stderr.toString()}`;
    const cargoLog = (() => {
      try { return readFileSync(logPath, "utf8"); } catch { return ""; }
    })();

    const receipt = (() => {
      try { return JSON.parse(readFileSync(receiptPath, "utf8")); } catch { return null; }
    })();

    return { ...result, output, cargoLog, receipt };
  }

  test("release test phase executes the Rust suite instead of only compiling it", () => {
    const result = runVerify(["--only", "test"]);
    expect(result.exitCode).toBe(0);
    expect(result.cargoLog).toContain("test --locked --lib");
    expect(result.cargoLog).not.toContain("--no-run");
  });

  test("integration phase executes the complete named nonintrusive behavior inventory", () => {
    const result = runVerify(["--only", "integration-tests"]);
    expect(result.exitCode).toBe(0);
    for (const suite of RELEASE_INTEGRATION_SUITES) {
      expect(result.cargoLog).toContain(`--test ${suite}`);
    }
    expect(result.cargoLog).not.toContain("--no-run");
  });

  test("integration receipt requires passing output from every named integration target", () => {
    const result = runVerify(["--only", "integration-tests"], {
      VERIFY_CAPTURE_RESULTS: "1",
      VERIFY_FAKE_INTEGRATION: "1",
    });
    expect(result.exitCode).toBe(0);
    expect(result.receipt?.gateId).toBe("integration-tests");
    expect(result.receipt?.result).toEqual({
      passed: 12,
      failed: 0,
      skipped: 0,
      suites: 6,
      suiteNames: [...RELEASE_INTEGRATION_SUITES].sort(),
    });
  });

  test.each([
    ["first-run-fixtures", "setup::tests::test_fresh_install_seeds_canonical_menu_syntax_handlers"],
    ["permissions-fixtures", "permissions_wizard::tests::test_snapshot_missing_required"],
    ["mock-ai-fixtures", "ai_reliability::model_tests::blocked_capability_produces_actionable_recovery_without_starting"],
    ["privacy-fixtures", "ai::reliability::tests::redactor_allowlists_json_masks_secrets_paths_and_bounds_output"],
  ])("%s requires its exact safe fixture and truthful one-test receipt", (phase, expectedTest) => {
    const result = runVerify(["--only", phase], {
      VERIFY_CAPTURE_RESULTS: "1",
      VERIFY_FAKE_FIXTURE: "1",
    });
    expect(result.exitCode).toBe(0);
    expect(result.cargoLog).toContain(expectedTest);
    expect(result.cargoLog).toContain("-- --exact");
    expect(result.receipt?.gateId).toBe(phase);
    expect(result.receipt?.result).toEqual({ passed: 1, failed: 0, skipped: 0 });
  });

  test("compile-only remains an explicitly named optional preflight", () => {
    const result = runVerify(["--only", "test-compile"]);
    expect(result.exitCode).toBe(0);
    expect(result.cargoLog).toContain("test --no-run --locked --lib");
  });

  test("a failing Rust behavior test propagates its failure to the release gate", () => {
    const result = runVerify(["--only", "test"], { VERIFY_FAKE_EXIT: "73" });
    expect(result.exitCode).toBe(73);
    expect(result.output).toContain("FAIL test (exit 73)");
  });

  test("recorded Rust release gate preserves actual nonzero execution counts", () => {
    const result = runVerify(["--only", "test"], {
      VERIFY_CAPTURE_RESULTS: "1",
      VERIFY_FAKE_SUMMARY: "1",
    });
    expect(result.exitCode).toBe(0);
    expect(result.receipt?.gateId).toBe("rust-tests");
    expect(result.receipt?.sourceState).toBe("clean");
    expect(result.receipt?.publishable).toBe(true);
    expect(result.receipt?.worktreeFingerprintSha256).toBeUndefined();
    expect(result.receipt?.result).toEqual({ passed: 7, failed: 0, skipped: 1 });
  });

  test("dirty tracked source cannot emit a publishable behavior receipt", () => {
    const result = runVerify(["--only", "test"], {
      VERIFY_CAPTURE_RESULTS: "1",
      VERIFY_FAKE_SUMMARY: "1",
      VERIFY_FAKE_GIT_DIRTY: "1",
    });

    expect(result.exitCode).not.toBe(0);
    expect(result.output).toContain("dirty tracked source cannot produce publishable release evidence");
    expect(result.receipt).toBeNull();
  });

  test("explicit dirty diagnostics bind reviewed owners and can never become publishable", () => {
    const result = runVerify(["--only", "test"], {
      VERIFY_CAPTURE_RESULTS: "1",
      VERIFY_FAKE_SUMMARY: "1",
      VERIFY_FAKE_GIT_DIRTY: "1",
      SCRIPT_KIT_ALLOW_DIRTY_DIAGNOSTIC_EVIDENCE: "1",
      SCRIPT_KIT_DIRTY_EVIDENCE_OWNER_PATHS:
        "scripts/release-evidence.ts:src/design_contract/mod.rs",
    });

    expect(result.exitCode).toBe(0);
    expect(result.receipt).toMatchObject({
      sourceState: "dirty",
      publishable: false,
      fingerprintScope: "DECLARED_OWNERS_NON_EXHAUSTIVE",
      worktreeOwners: [
        { path: "scripts/release-evidence.ts", sha256: expect.any(String) },
        { path: "src/design_contract/mod.rs", sha256: expect.any(String) },
      ],
    });
    expect(result.receipt.worktreeFingerprintSha256).toMatch(/^[a-f0-9]{64}$/);
  });

  test("dirty diagnostics refuse missing or potentially sensitive owner paths", () => {
    for (const owners of ["", ".env", "src/private_key.pem"]) {
      const result = runVerify(["--only", "test"], {
        VERIFY_CAPTURE_RESULTS: "1",
        VERIFY_FAKE_SUMMARY: "1",
        VERIFY_FAKE_GIT_DIRTY: "1",
        SCRIPT_KIT_ALLOW_DIRTY_DIAGNOSTIC_EVIDENCE: "1",
        SCRIPT_KIT_DIRTY_EVIDENCE_OWNER_PATHS: owners,
      });

      expect(result.exitCode).not.toBe(0);
      expect(result.receipt).toBeNull();
      expect(result.output).toContain(owners
        ? "not an approved non-sensitive source path"
        : "at least one explicitly reviewed --owner path");
    }
  });

  test.each([
    "scripts/release-evidence.test.ts",
    "scripts/devtools/actions-projection.test.ts",
    "scripts/devtools/operator-safety.test.ts",
    "scripts/devtools/driver.ts",
    "scripts/devtools/lib/client.ts",
    "scripts/devtools/lib/target-identity.ts",
    "scripts/devtools/lib/privacy.ts",
    "scripts/devtools/test-status.ts",
    "scripts/devtools/state-ownership.test.ts",
    "scripts/devtools/generated-byte-compare.test.ts",
    "scripts/agent-check.sh",
    "scripts/agentic/session.sh",
    "scripts/agentic/start-isolated.sh",
    "scripts/agentic/devtools-session.sh",
    "scripts/agentic/agent-cargo.sh",
    "scripts/agentic/cargo-cache-locks.sh",
    "scripts/agentic/cargo-build-policy.test.ts",
    "scripts/agentic/reuse-rust-test-binary.sh",
    "scripts/agentic/build-isolated-binary.sh",
    "scripts/agentic/root-search-frame-stability.test.ts",
    "tests/sdk/capability-types.fixture.ts",
    "tests/sdk/fixtures/runner-negative-case.ts",
    "tests/sdk/runner-safety.test.ts",
    "tests/protocol_batch.rs",
  ])("standalone authoritative gates reject an untracked mandatory fixture: %s", (owner) => {
    const result = runVerify(["--only", "test"], {
      VERIFY_CAPTURE_RESULTS: "1",
      VERIFY_FAKE_SUMMARY: "1",
      VERIFY_FAKE_UNTRACKED_PATH: owner,
    });

    expect(result.exitCode).not.toBe(0);
    expect(result.output).toContain(
      "publishable release evidence requires every mandatory source owner to be tracked",
    );
    expect(result.receipt).toBeNull();
  });

  test("successful compilation output without test summaries cannot emit a Rust receipt", () => {
    const result = runVerify(["--only", "test"], { VERIFY_CAPTURE_RESULTS: "1" });
    expect(result.exitCode).toBe(1);
    expect(result.output).toContain("compile-only output cannot satisfy");
    expect(result.receipt).toBeNull();
  });

  test("captured Rust test failures preserve the original command exit code", () => {
    const result = runVerify(["--only", "test"], {
      VERIFY_CAPTURE_RESULTS: "1",
      VERIFY_FAKE_EXIT: "73",
    });
    expect(result.exitCode).toBe(73);
    expect(result.output).toContain("FAIL test (exit 73)");
    expect(result.receipt).toBeNull();
  });

  test.each([
    "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
    "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
    "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
    "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
    "SCRIPT_KIT_ALLOW_LIVE_AI",
    "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
  ])("refuses %s before launching any verification command", (unsafeSetting) => {
    const result = runVerify(["--only", "test"], { [unsafeSetting]: "1" });
    expect(result.exitCode).toBe(78);
    expect(result.output).toContain(`REFUSED unsafe setting ${unsafeSetting}=1`);
    expect(result.cargoLog).toBe("");
  });

  test("committed-source mode rejects a different HEAD before executing any command", () => {
    const result = runVerify(["--only", "test"], {
      SCRIPT_KIT_REQUIRE_CLEAN_SOURCE: "1",
      GITHUB_SHA: "f".repeat(40),
    });
    expect(result.exitCode).toBe(78);
    expect(result.output).toContain("REFUSED source identity mismatch");
    expect(result.cargoLog).toBe("");
  });

  test.each([
    "scripts/devtools/consistency-catalog.md",
    "scripts/devtools/lib/operator-safety.ts",
    "scripts/devtools/lib/privacy.ts",
    "scripts/devtools/lib/evidence-class.ts",
    "scripts/devtools/lib/task-proof-policy.ts",
    "scripts/devtools/family-fixtures.ts",
    "scripts/devtools/facade-ledger.ts",
    "scripts/devtools/facade-migrations.ts",
    "scripts/devtools/safe-task-proofs.ts",
    "scripts/devtools/protected-sources.ts",
    "scripts/devtools/state-ownership.ts",
    "scripts/devtools/state-ownership.test.ts",
    "scripts/devtools/design-conflicts.ts",
    "scripts/devtools/generated-byte-compare.ts",
    "scripts/devtools/generated-byte-compare.test.ts",
    "scripts/devtools/alpha-byte-contract-harness.rs",
  ])("committed-source mode rejects an untracked required contract: %s", (requiredArtifact) => {
    const result = runVerify(["--only", "test"], {
      SCRIPT_KIT_REQUIRE_CLEAN_SOURCE: "1",
      GITHUB_SHA: SOURCE_SHA,
      VERIFY_FAKE_UNTRACKED_PATH: requiredArtifact,
    });

    expect(result.exitCode).toBe(78);
    expect(result.output).toContain(
      `REFUSED untracked or missing release contract ${requiredArtifact}`,
    );
    expect(result.cargoLog).toBe("");
  });
});

describe("nonintrusive CI release ownership and publication graph", () => {
  function workflow(name: "ci" | "release" | "perf-gates"): {
    env: Record<string, string>;
    jobs: Record<string, {
      "runs-on": string;
      needs?: string | string[];
      steps?: Array<{ name?: string; uses?: string; run?: string; if?: string }>;
    }>;
  } {
    return Bun.YAML.parse(readFileSync(
      resolve(import.meta.dir, `../.github/workflows/${name}.yml`),
      "utf8",
    )) as ReturnType<typeof workflow>;
  }

  test("every workflow refuses visible probes, screen takeover, live AI, and local app launch", () => {
    for (const name of ["ci", "release", "perf-gates"] as const) {
      const definition = workflow(name);
      expect(definition.env.SCRIPT_KIT_NONINTERACTIVE).toBe("1");
      for (const unsafeFlag of [
        "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
        "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
        "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
        "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
        "SCRIPT_KIT_ALLOW_LIVE_AI",
        "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
      ]) {
        expect(definition.env[unsafeFlag]).toBe("0");
      }
    }
  });

  test("only the two isolated hidden-frame CI commands opt into application launch", () => {
    const authorizedLaunches: Array<{ workflow: string; job: string; step: string }> = [];
    for (const name of ["ci", "release", "perf-gates"] as const) {
      for (const [job, definition] of Object.entries(workflow(name).jobs)) {
        for (const step of definition.steps ?? []) {
          if (!step.run?.includes("SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1")) continue;
          expect(step.run).toContain("root-search-frame-stability.ts");
          expect(step.run).not.toContain("--include-system");
          authorizedLaunches.push({ workflow: name, job, step: step.name ?? "unnamed" });
        }
      }
    }

    expect(authorizedLaunches).toEqual([
      {
        workflow: "release",
        job: "sign-notarize-macos",
        step: "Prove the exact signed packaged binary without showing a window",
      },
      {
        workflow: "perf-gates",
        job: "root-frame-identity",
        step: "Run hidden-window semantic frame-identity gate",
      },
    ]);
  });

  test("macOS proof job executes real platform fixtures without hidden skips", () => {
    const proof = workflow("release").jobs["validate-proof-contracts"];
    expect(proof["runs-on"]).toBe("macos-14");
    expect(proof.steps?.some((step) =>
      step.uses?.startsWith("dtolnay/rust-toolchain@"))).toBe(true);
    expect(proof.steps?.some((step) =>
      step.run?.includes("--only proof-contracts"))).toBe(true);
  });

  test("publication is downstream of exact packaged journey readiness and a blocked scorecard", () => {
    const release = workflow("release");
    const signing = release.jobs["sign-notarize-macos"];
    const steps = signing.steps ?? [];
    const readiness = steps.findIndex((step) =>
      step.name === "Block publication on unmeasured exact packaged journeys");
    const blockedScorecard = steps.findIndex((step) =>
      step.name === "Upload blocked packaged release scorecard");
    const manifest = steps.findIndex((step) => step.name === "Generate release manifest");
    expect(readiness).toBeGreaterThanOrEqual(0);
    expect(blockedScorecard).toBe(readiness + 1);
    expect(steps[blockedScorecard].if).toBe("failure()");
    expect(manifest).toBeGreaterThan(blockedScorecard);
    for (const journey of REQUIRED_PACKAGED_JOURNEYS) {
      expect(steps[manifest].run).toContain(`release-evidence/${journey}.json`);
    }
    for (const assurance of REQUIRED_PACKAGED_ASSURANCES) {
      expect(steps[manifest].run).toContain(`release-evidence/${assurance.gateId}.json`);
    }
    expect(release.jobs.release.needs).toEqual([
      "validate-release-gates",
      "sign-notarize-macos",
    ]);
  });

  test("authoritative exporter proof survives proof, signing, scorecard, and publication jobs", () => {
    const release = workflow("release");
    const proofSteps = release.jobs["validate-proof-contracts"].steps ?? [];
    const producer = proofSteps.find((step) =>
      step.name === "Prove generated design JSON and CSS match the authoritative exporter");
    expect(producer?.run).toContain("release-proof-evidence/generated-design-contracts-proof.json");
    expect(producer?.run).toContain("--result \"$RUNNER_TEMP/release-proof-evidence/generated-design-contracts-proof.json\"");

    const signingSteps = release.jobs["sign-notarize-macos"].steps ?? [];
    for (const stepName of [
      "Generate release manifest",
      "Fail closed on stale, wrong-binary, missing, or unsafe release evidence",
      "Generate honest packaged release scorecard",
    ]) {
      expect(signingSteps.find((step) => step.name === stepName)?.run)
        .toContain("--design-proof \"$ROOT/generated-design-contracts-proof.json\"");
    }

    const finalSteps = release.jobs.release.steps ?? [];
    expect(finalSteps.find((step) =>
      step.name === "Verify exact archived artifact and all mandatory release evidence")?.run)
      .toContain("--design-proof artifacts/generated-design-contracts-proof.json");
  });
});

const describeMacOS = process.platform === "darwin" ? describe : describe.skip;

describeMacOS("packaged macOS metadata and executable identity", () => {
  function inspectBundle(options: {
    identifier?: string;
    version?: string;
    executable?: string;
    expectedAppSha?: string;
    expectedSidecarSha?: string;
  }) {
    const fixture = makeFixture();
    mkdirSync(join(fixture.appPath, "Contents/Resources/assets"), { recursive: true });
    writeFileSync(join(fixture.appPath, "Contents/Info.plist"), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>${options.identifier ?? "com.scriptkit.app"}</string>
  <key>CFBundleShortVersionString</key><string>${options.version ?? "0.1.17"}</string>
  <key>CFBundleExecutable</key><string>${options.executable ?? "script-kit-gpui"}</string>
</dict></plist>`);

    const result = Bun.spawnSync({
      cmd: ["bash", "scripts/verify-macos-bundle.sh", fixture.appPath],
      cwd: resolve(import.meta.dir, ".."),
      env: {
        ...process.env,
        SCRIPT_KIT_EXPECTED_APP_SHA256: options.expectedAppSha ?? "",
        SCRIPT_KIT_EXPECTED_PI_SHA256: options.expectedSidecarSha ?? "",
      },
      stdout: "pipe",
      stderr: "pipe",
    });

    return { exitCode: result.exitCode, output: `${result.stdout}${result.stderr}` };
  }

  function inspectSyntheticAssetBundle(tamperedAsset?: string, symbolicLinkPath?: string) {
    const root = mkdtempSync(join(tmpdir(), "script-kit-bundle-assets-"));
    TEMPORARY_DIRECTORIES.push(root);
    const appPath = join(root, "Script Kit.app");
    const sourceAssets = join(root, "assets");
    const bundledAssets = join(appPath, "Contents/Resources/assets");
    const sourceScripts = join(root, "scripts");
    const bundledScripts = join(appPath, "Contents/Resources/scripts");

    mkdirSync(join(appPath, "Contents/MacOS"), { recursive: true });
    mkdirSync(sourceScripts, { recursive: true });
    writeFileSync(join(root, "Cargo.toml"),
      '[package]\nversion = "0.1.17"\nidentifier = "com.scriptkit.app"\n');
    writeFileSync(join(appPath, "Contents/MacOS/script-kit-gpui"), "synthetic-app", {
      mode: 0o755,
    });
    writeFileSync(join(appPath, "Contents/MacOS/pi"), "synthetic-pi", {
      mode: 0o755,
    });
    writeFileSync(join(appPath, "Contents/Info.plist"), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.scriptkit.app</string>
  <key>CFBundleShortVersionString</key><string>0.1.17</string>
  <key>CFBundleExecutable</key><string>script-kit-gpui</string>
</dict></plist>`);

    const assetPaths = [
      "Info.plist.ext",
      "icon.icns",
      "icon.png",
      "icon@2x.png",
      "logo.svg",
      "icons/file.svg",
      "icons/file_code.svg",
      "icons/folder.svg",
      "icons/folder_open.svg",
      "icons/settings.svg",
      "icons/magnifying_glass.svg",
      "icons/agent_chat.svg",
      "icons/ai_provider_openai.svg",
      "fonts/JetBrainsMono-Regular.ttf",
      "fonts/JetBrainsMono-Bold.ttf",
      "fonts/JetBrainsMono-Italic.ttf",
      "fonts/JetBrainsMono-BoldItalic.ttf",
      "fonts/JetBrainsMono-Medium.ttf",
      "fonts/JetBrainsMono-SemiBold.ttf",
      "fonts/LICENSE.txt",
    ];
    for (const relativePath of assetPaths) {
      const sourcePath = join(sourceAssets, relativePath);
      const bundledPath = join(bundledAssets, relativePath);
      mkdirSync(join(sourcePath, ".."), { recursive: true });
      mkdirSync(join(bundledPath, ".."), { recursive: true });
      writeFileSync(sourcePath, `authoritative:${relativePath}`);
      writeFileSync(bundledPath, `authoritative:${relativePath}`);
    }
    writeFileSync(join(appPath, "Contents/Resources/icon.icns"), "authoritative:icon.icns");

    const scriptPaths = [
      "kit-sdk.ts",
      "migrate/cli.ts",
      "migrate/pipeline.ts",
      "migrate/classify.ts",
      "migrate/agent.ts",
      "migrate/metadata.ts",
      "migrate/types.ts",
      "migrate/validators.ts",
      "migrate/compat-map.json",
      "migrate/prompts/port.md",
      "migrate/prompts/repair.md",
      "migrate/prompts/honesty.md",
    ];
    for (const relativePath of scriptPaths) {
      const sourcePath = join(sourceScripts, relativePath);
      const bundledPath = join(bundledScripts, relativePath);
      mkdirSync(join(sourcePath, ".."), { recursive: true });
      mkdirSync(join(bundledPath, ".."), { recursive: true });
      writeFileSync(sourcePath, `authoritative:${relativePath}`);
      writeFileSync(bundledPath, `authoritative:${relativePath}`);
    }

    if (tamperedAsset) {
      const tamperedPath = tamperedAsset === "Contents/Resources/icon.icns"
        ? join(appPath, tamperedAsset)
        : join(bundledAssets, tamperedAsset);
      writeFileSync(tamperedPath, "tampered asset with the same filename");
    }

    if (symbolicLinkPath) {
      const bundledPath = join(appPath, symbolicLinkPath);
      const escapedTarget = join(root, "outside-bundle-target");
      if (statSync(bundledPath).isDirectory()) {
        renameSync(bundledPath, escapedTarget);
        symlinkSync(escapedTarget, bundledPath, "dir");
      } else {
        writeFileSync(escapedTarget, readFileSync(bundledPath), {
          mode: symbolicLinkPath.startsWith("Contents/MacOS/") ? 0o755 : 0o644,
        });
        unlinkSync(bundledPath);
        symlinkSync(escapedTarget, bundledPath);
      }
    }

    const verifierPath = join(sourceScripts, "verify-macos-bundle.sh");
    writeFileSync(verifierPath,
      readFileSync(join(import.meta.dir, "verify-macos-bundle.sh"), "utf8"));
    const result = Bun.spawnSync({
      cmd: ["bash", verifierPath, appPath],
      cwd: root,
      env: {
        ...process.env,
        SCRIPT_KIT_EXPECTED_APP_SHA256: "",
        SCRIPT_KIT_EXPECTED_PI_SHA256: "",
      },
      stdout: "pipe",
      stderr: "pipe",
    });
    return { exitCode: result.exitCode, output: `${result.stdout}${result.stderr}` };
  }

  test("rejects another application's bundle identifier", () => {
    const result = inspectBundle({ identifier: "com.invalid.other" });
    expect(result.exitCode).toBe(1);
    expect(result.output).toContain("bundle_identifier_mismatch");
  });

  test("rejects a bundle version or entry point that diverges from the source", () => {
    expect(inspectBundle({ version: "9.9.9" }).output).toContain("bundle_version_mismatch");
    expect(inspectBundle({ executable: "wrong-entry" }).output).toContain("bundle_executable_mismatch");
  });

  test("rejects a packaged application or Pi sidecar with the wrong expected identity", () => {
    expect(inspectBundle({ expectedAppSha: "f".repeat(64) }).output)
      .toContain("app_identity_mismatch");
    expect(inspectBundle({ expectedSidecarSha: "f".repeat(64) }).output)
      .toContain("pi_identity_mismatch");
  });

  test("accepts an isolated synthetic bundle only when every packaged asset matches its source", () => {
    const result = inspectSyntheticAssetBundle();
    expect(result.exitCode).toBe(0);
    expect(result.output).toContain("bundle_verify resources ok");
  });

  test.each([
    "Info.plist.ext",
    "icon.icns",
    "icon.png",
    "icon@2x.png",
    "logo.svg",
    "Contents/Resources/icon.icns",
    "icons/file.svg",
    "fonts/JetBrainsMono-Regular.ttf",
    "fonts/LICENSE.txt",
  ])("rejects same-name packaged asset content tampering: %s", (assetPath) => {
    const result = inspectSyntheticAssetBundle(assetPath);
    expect(result.exitCode).toBe(1);
    expect(result.output).toContain("resource_content_mismatch");
    expect(result.output).toContain(assetPath);
  });

  test.each([
    "Contents",
    "Contents/MacOS",
    "Contents/Resources",
    "Contents/Resources/assets",
    "Contents/Resources/assets/icons",
    "Contents/Resources/assets/fonts",
    "Contents/Resources/scripts",
    "Contents/Resources/scripts/migrate",
    "Contents/Resources/scripts/migrate/prompts",
    "Contents/MacOS/script-kit-gpui",
    "Contents/MacOS/pi",
    "Contents/Info.plist",
    "Contents/Resources/icon.icns",
    "Contents/Resources/assets/logo.svg",
    "Contents/Resources/assets/icons/file.svg",
    "Contents/Resources/scripts/kit-sdk.ts",
    "Contents/Resources/scripts/migrate/cli.ts",
  ])("rejects package escape through an exact-byte symbolic link: %s", (bundledPath) => {
    const result = inspectSyntheticAssetBundle(undefined, bundledPath);
    expect(result.exitCode).toBe(1);
    expect(result.output).toContain("symbolic_link_disallowed");
    expect(result.output).toContain(bundledPath);
  });
});
