#!/usr/bin/env bun
/**
 * Run the real prebuilt, non-GUI design-token exporter into a private temp
 * directory and compare its JSON/CSS bytes to the checked-in contract.
 *
 * This is exporter UNIT_BEHAVIOR evidence, not application-runtime evidence.
 * The producer never builds the binary, starts Script Kit, scans the checkout,
 * writes checked-in artifacts, or contacts an AI provider.
 */
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { verifyImmutableArtifact, type ArtifactReference, type ArtifactExpectation } from "../agentic/build-artifact.ts";
import { spawnOwnedProcess, type OwnedProcess } from "../agentic/owned-process.ts";
import { claimOutput, validateOutputTarget, createOwnedStagingDirectory, removeOwnedAuxiliaryDirectory,
  beginManagedTask, updateManagedTask, finalizeManagedTask, buildArtifactLifecycle, commitFinalReceipt, type OwnedCleanup } from "../agentic/artifact-lifecycle.ts";
import { boundedObservation, unknownOwnedCleanup, DriverLifecycleError } from "./driver.ts";
import { readArtifactReference } from "./design.ts";
import { resolveReceiptDetails } from "./lib/receipt-artifact.ts";

export const GENERATED_BYTE_COMPARE_SOURCE_PATHS = [
  "Cargo.toml",
  "src/bin/export_design_tokens.rs",
  "src/design_contract/mod.rs",
  "src/design_contract/bundle_header.rs",
  "src/design_contract/bundle_notes.rs",
  "src/design_contract/bundle_settings_day.rs",
  "src/design_contract/bundle_agent_chat.rs",
  "src/design_contract/bundle_prompts.rs",
  "src/components/conversation_style.rs",
  "src/theme/alpha.rs",
  "src/ui/chrome/tokens.rs",
] as const;

export const GENERATED_BYTE_COMPARE_OUTPUT_PATHS = [
  "design/mockups/generated/tokens.json",
  "design/mockups/generated/tokens.css",
] as const;

export const GENERATED_BYTE_COMPARE_RECEIPT_PATH =
  ".artifacts/consistency/GOV-005/generated-byte-compare.json";

export interface ExporterProcessResult {
  status: number | null;
  signal?: string | null;
  stdout?: string | Uint8Array;
  stderr?: string | Uint8Array;
  error?: Error;
}

export interface GeneratedByteCompareDependencies {
  repositoryRoot?: string;
  environment?: Record<string, string | undefined>;
}
export interface GeneratedByteCompareOptions {
  artifactReference: ArtifactReference;
  outputPath: string;
  sourcePolicy?: ArtifactExpectation["sourcePolicy"];
}

export interface GeneratedByteCompareReceiptIdentity {
  currentSourceSha?: string | null;
  currentFileSha256?: (path: string) => string | null;
}

function sha256(bytes: string | Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function failure(message: string): never {
  throw new Error("generated exporter byte comparison refused: " + message);
}

function readCurrentGitCommit(
  repositoryRoot: string,
  readFile: (path: string) => Uint8Array,
): string | null {
  let gitDirectory = resolve(repositoryRoot, ".git");
  let head: string;
  try {
    head = Buffer.from(readFile(join(gitDirectory, "HEAD")))
      .toString("utf8")
      .trim();
  } catch {
    try {
      const pointer = Buffer.from(readFile(gitDirectory))
        .toString("utf8")
        .trim();
      if (!pointer.startsWith("gitdir: ")) return null;
      gitDirectory = resolve(repositoryRoot, pointer.slice("gitdir: ".length));
      head = Buffer.from(readFile(join(gitDirectory, "HEAD")))
        .toString("utf8")
        .trim();
    } catch {
      return null;
    }
  }

  if (/^[a-f0-9]{40}$/i.test(head)) return head.toLowerCase();
  const reference = /^ref:\s+(refs\/[A-Za-z0-9._/-]+)$/.exec(head)?.[1];
  if (!reference || reference.includes("..")) return null;
  try {
    const loose = Buffer.from(readFile(join(gitDirectory, reference)))
      .toString("utf8")
      .trim();
    return /^[a-f0-9]{40}$/i.test(loose) ? loose.toLowerCase() : null;
  } catch {
    try {
      const packed = Buffer.from(readFile(join(gitDirectory, "packed-refs")))
        .toString("utf8");
      for (const line of packed.split(/\r?\n/)) {
        const match = /^([a-f0-9]{40})\s+(\S+)$/i.exec(line);
        if (match?.[2] === reference) return match[1]!.toLowerCase();
      }
    } catch {
      // Fail closed rather than asking Git or launching another subprocess.
    }
    return null;
  }
}

export function validateGeneratedByteCompareEnvironment(
  environment: Record<string, string | undefined>,
): string[] {
  const errors: string[] = [];
  if (environment.SCRIPT_KIT_NONINTERACTIVE !== "1") {
    errors.push("SCRIPT_KIT_NONINTERACTIVE=1 is required");
  }
  for (const setting of [
    "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
    "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
    "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
    "SCRIPT_KIT_ALLOW_LIVE_AI",
    "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
    "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
  ]) {
    const value = environment[setting];
    if (
      (setting === "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH" ||
        setting === "SCRIPT_KIT_ALLOW_VISIBLE_PROBES" ||
        setting === "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER" ||
        setting === "SCRIPT_KIT_ALLOW_LIVE_AI")
        ? value !== "0"
        : value !== undefined && value !== "0"
    ) {
      errors.push(setting + " must be disabled");
    }
  }
  return errors;
}

function fingerprintSources(
  repositoryRoot: string,
  readFile: (path: string) => Uint8Array,
): Record<string, string> {
  const fingerprints: Record<string, string> = {};
  for (const path of GENERATED_BYTE_COMPARE_SOURCE_PATHS) {
    try {
      fingerprints[path] = sha256(readFile(resolve(repositoryRoot, path)));
    } catch {
      failure("required exporter source is missing: " + path);
    }
  }
  return fingerprints;
}

function sameIdentity(
  left: Record<string, string>,
  right: Record<string, string>,
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

export async function generateAuthoritativeByteComparison(options: GeneratedByteCompareOptions, dependencies: GeneratedByteCompareDependencies = {}) {
  const repositoryRoot = resolve(dependencies.repositoryRoot ?? process.cwd());
  const safetyErrors = validateGeneratedByteCompareEnvironment(dependencies.environment ?? process.env);
  if (safetyErrors.length) failure(safetyErrors.join("; "));
  const artifact = verifyImmutableArtifact(repositoryRoot, options.artifactReference, { kind: "tool", packageName: "script-kit-gpui",
    targetName: "export_design_tokens", sourcePolicy: options.sourcePolicy ?? "current-content" });
  const claim = claimOutput(validateOutputTarget({ repoRoot: repositoryRoot, candidate: options.outputPath, kind: "receipt", probeId: "generated-byte-compare" }));
  const task = beginManagedTask(claim, "runtime-run", [artifact.reference]);
  let temporaryDirectory: string | undefined;
  let proc: OwnedProcess | undefined;
  let cleanup: OwnedCleanup = unknownOwnedCleanup(false);
  let execution = { exitCode: -1, stdoutSha256: sha256(""), stderrSha256: sha256("") };
  const readers: ReadableStreamDefaultReader<Uint8Array>[] = [];
  let consumers: Promise<Uint8Array>[] = [];
  const outputHashes: Record<string, string> = {};
  const generatedOutputHashes: Record<string, string> = {};
  const outputs: Array<{ path: string; checkedInSha256: string; generatedSha256: string; byteEqual: boolean; byteLength: number }> = [];
  let sourceFingerprints: Record<string, string> = {};
  let sourceSha: string | null = null;
  let error: string | undefined;
  try {
    sourceSha = readCurrentGitCommit(repositoryRoot, readFileSync);
    sourceFingerprints = fingerprintSources(repositoryRoot, readFileSync);
    const checked = Object.fromEntries(GENERATED_BYTE_COMPARE_OUTPUT_PATHS.map(path => [path, readFileSync(resolve(repositoryRoot, path))]));
    temporaryDirectory = createOwnedStagingDirectory(claim);
    const home = join(temporaryDirectory, "home"); mkdirSync(home, { mode: 0o700 });
    proc = await spawnOwnedProcess({ argv: [artifact.executablePath, temporaryDirectory], cwd: repositoryRoot, timeoutMs: 30000, maxOutputBytes: 2000000,
      env: { PATH: "/usr/bin:/bin:/usr/sbin:/sbin", HOME: home, SK_PATH: join(home, ".scriptkit"), CODEX_HOME: join(home, ".codex"),
        XDG_CONFIG_HOME: join(home, ".config"), XDG_DATA_HOME: join(home, ".local/share"), XDG_CACHE_HOME: join(home, ".cache"), TMPDIR: temporaryDirectory,
        LANG: "en_US.UTF-8", TZ: "UTC", SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0", SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
        SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0", SCRIPT_KIT_ALLOW_LIVE_AI: "0", SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0", SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0" } });
    cleanup = unknownOwnedCleanup(true); updateManagedTask(task, { state: "running", ownedProcesses: [proc.identity], source: artifact.manifest.source });
    const consume = async (stream: ReadableStream<Uint8Array>): Promise<Uint8Array> => {
      const reader = stream.getReader(); readers.push(reader); const chunks: Uint8Array[] = []; let bytes = 0;
      try { for (;;) { const next = await reader.read(); if (next.done) break; bytes += next.value.length;
        if (bytes > 1000000) throw new Error("exporter_output_limit"); chunks.push(next.value); }
        return Buffer.concat(chunks);
      } finally { reader.releaseLock(); }
    };
    consumers = [consume(proc.stdout), consume(proc.stderr)];
    const completed = await boundedObservation(Promise.all([proc.exited, ...consumers]), 33000);
    if (completed.completed === false) throw completed.error;
    execution = { exitCode: completed.value[0] as number, stdoutSha256: sha256(completed.value[1] as Uint8Array), stderrSha256: sha256(completed.value[2] as Uint8Array) };
    if (execution.exitCode !== 0) failure("non-GUI exporter failed");
    for (const path of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
      const generated = readFileSync(join(temporaryDirectory, basename(path)));
      const original = checked[path]!;
      if (!original.equals(generated)) failure("checked-in output differs from exporter bytes: " + path);
      if (!original.equals(readFileSync(resolve(repositoryRoot, path)))) failure("exporter changed checked-in output: " + path);
      outputHashes[path] = sha256(original); generatedOutputHashes[path] = sha256(generated);
      outputs.push({ path, checkedInSha256: outputHashes[path]!, generatedSha256: generatedOutputHashes[path]!, byteEqual: true, byteLength: original.length });
    }
    verifyImmutableArtifact(repositoryRoot, artifact.reference, { kind: "tool", packageName: "script-kit-gpui", targetName: "export_design_tokens", sourcePolicy: options.sourcePolicy ?? "current-content" });
    if (!sameIdentity(sourceFingerprints, fingerprintSources(repositoryRoot, readFileSync))) failure("exporter source changed during execution");
  } catch (cause) {
    error = cause instanceof Error ? cause.message : String(cause);
    if (cause && typeof cause === "object" && "cleanup" in cause) {
      // spawnOwnedProcess attaches its canonical in-process cleanup record to startup failures.
      const observedCleanup = cause.cleanup as OwnedCleanup;
      cleanup = observedCleanup;
    }
  } finally {
    if (proc) {
      const result = await boundedObservation(proc.close(), 8000); cleanup = result.completed ? result.value : unknownOwnedCleanup(true);
      const drained = await boundedObservation(Promise.allSettled(consumers), 1000);
      if (!drained.completed) await boundedObservation(Promise.allSettled(readers.map(reader => reader.cancel())), 500);
      cleanup = { ...cleanup, streamsDrained: drained.completed && drained.value.every(result => result.status === "fulfilled"), logWriterClosed: true };
      cleanup = { ...cleanup, closed: cleanup.closed && cleanup.streamsDrained };
    }
    if (temporaryDirectory && cleanup.closed) {
      try { removeOwnedAuxiliaryDirectory(claim, temporaryDirectory); }
      catch { cleanup = { ...cleanup, closed: false, failureCodes: [...cleanup.failureCodes, "exporter_output_cleanup_failed"] }; }
    }
    try { updateManagedTask(task, { result: { status: !error && cleanup.closed ? "succeeded" : "failed" } }); cleanup = finalizeManagedTask(task, cleanup).cleanup; }
    catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false }; }
  }
  const pass = !error && cleanup.closed && outputs.length === 2;
  const receipt = { schemaVersion: 2, generatedBy: "scripts/devtools/generated-byte-compare.ts", taskId: "GOV-005", evidenceClass: "UNIT_BEHAVIOR", provesRuntimeBehavior: false,
    sourceSha, sourceCoverage: { mode: "DECLARED_EXPORTER_SOURCE_OWNERS", sourceGraphExhaustive: false }, sourceFingerprints,
    binary: { ...artifact.binary, artifactReference: artifact.reference, source: artifact.manifest.source },
    outputHashes, generatedOutputHashes, outputs, byteEqual: pass, handEditedGeneratedOutput: false,
    safety: { noninteractive: true, startsApplication: false, revealsWindow: false, focusesWindow: false, drivesNativeInput: false, capturesScreen: false,
      accessesNetwork: false, usesLiveAi: false, startsExporter: true, isolatedTempOutput: true }, execution, cleanup,
    disposition: cleanup.closed ? pass ? "EVALUABLE_PASS" : "EVALUABLE_FAIL" : "INVALID_CLEANUP", pass, error };
  // Release transports copy this small proof alone. Keep its hashed-output detail
  // authoritative here rather than creating a second observation payload.
  const finalReceipt = { ...receipt, artifactLifecycle: buildArtifactLifecycle({ claim, finalizationKind: "driver-close", writersFinalized: cleanup.closed, specs: [], artifacts: [] }) };
  commitFinalReceipt(claim, finalReceipt, [], []);
  return finalReceipt;
}

function object(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

/**
 * Validate a generated-byte artifact as actual exporter evidence. Identity
 * callbacks are optional for discovery and mandatory for final governance.
 */
export function validateGeneratedByteCompareReceipt(
  candidate: unknown,
  identity: GeneratedByteCompareReceiptIdentity = {},
) {
  let receipt: Record<string, unknown>;
  try { receipt = resolveReceiptDetails(object(candidate)); }
  catch (error) { return { pass: false, errors: [error instanceof Error ? error.message : String(error)] }; }
  const errors: string[] = [];
  if (
    receipt.schemaVersion !== 2 ||
    receipt.generatedBy !== "scripts/devtools/generated-byte-compare.ts" ||
    receipt.taskId !== "GOV-005" ||
    receipt.evidenceClass !== "UNIT_BEHAVIOR" ||
    receipt.provesRuntimeBehavior !== false ||
    receipt.byteEqual !== true ||
    receipt.handEditedGeneratedOutput !== false ||
    receipt.disposition !== "EVALUABLE_PASS" ||
    receipt.pass !== true
  ) {
    errors.push("exporter receipt must be a passing GOV-005 non-GUI unit behavior proof");
  }

  const sourceSha = receipt.sourceSha;
  if (typeof sourceSha !== "string" || !/^[a-f0-9]{40}$/.test(sourceSha)) {
    errors.push("exporter receipt source commit is absent or malformed");
  } else if (
    identity.currentSourceSha !== undefined &&
    sourceSha !== identity.currentSourceSha
  ) {
    errors.push("exporter receipt source commit differs from the current checkout");
  }

  const coverage = object(receipt.sourceCoverage);
  if (
    coverage.mode !== "DECLARED_EXPORTER_SOURCE_OWNERS" ||
    coverage.sourceGraphExhaustive !== false
  ) {
    errors.push("exporter source ownership must remain bounded and explicitly declared");
  }

  const sourceFingerprints = object(receipt.sourceFingerprints);
  const expectedSources = [...GENERATED_BYTE_COMPARE_SOURCE_PATHS].sort();
  if (
    JSON.stringify(Object.keys(sourceFingerprints).sort()) !==
      JSON.stringify(expectedSources)
  ) {
    errors.push("exporter receipt must fingerprint every exact declared source owner");
  }
  for (const path of GENERATED_BYTE_COMPARE_SOURCE_PATHS) {
    const recorded = sourceFingerprints[path];
    if (typeof recorded !== "string" || !/^[a-f0-9]{64}$/.test(recorded)) {
      errors.push("missing or malformed exporter source fingerprint: " + path);
    } else if (
      identity.currentFileSha256 !== undefined &&
      identity.currentFileSha256(path) !== recorded
    ) {
      errors.push("stale exporter source fingerprint: " + path);
    }
  }

  const binary = object(receipt.binary);
  const binaryPath = binary.path;
  if (
    typeof binaryPath !== "string" ||
    binaryPath.length === 0 ||
    basename(binaryPath) !== "export_design_tokens" ||
    isAbsolute(binaryPath) ||
    binaryPath.includes("\\") ||
    binaryPath.split("/").some((part) =>
      part === "" || part === "." || part === "..",
    ) ||
    !(binaryPath.startsWith("target-agent/artifacts/") ||
      binaryPath.startsWith("target-agent/runtime/")) ||
    typeof binary.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(binary.sha256) ||
    !Number.isSafeInteger(binary.sizeBytes) ||
    Number(binary.sizeBytes) <= 0
  ) {
    errors.push("exporter binary identity is missing, malformed, or outside the repository");
  } else if (
    identity.currentFileSha256 !== undefined &&
    identity.currentFileSha256(binaryPath) !== binary.sha256
  ) {
    errors.push("exporter binary fingerprint no longer matches the proven executable");
  }

  const reference = object(binary.artifactReference);
  const source = object(binary.source);
  if (typeof reference.manifestPath !== "string" || !reference.manifestPath.startsWith("target-agent/artifacts/") ||
      typeof reference.manifestSha256 !== "string" || !/^[a-f0-9]{64}$/.test(reference.manifestSha256) ||
      binary.manifestPath !== reference.manifestPath || binary.manifestSha256 !== reference.manifestSha256 ||
      binary.sourceCommit !== source.gitHead || source.algorithm !== "reviewed-worktree-content-v1" ||
      typeof source.compilerInputSha256 !== "string" || !/^[a-f0-9]{64}$/.test(source.compilerInputSha256)) {
    errors.push("exporter requires an explicit V3 artifact and truthful recorded source identity");
  } else if (identity.currentFileSha256 !== undefined && identity.currentFileSha256(reference.manifestPath) !== reference.manifestSha256) {
    errors.push("exporter manifest fingerprint changed");
  }

  const checkedHashes = object(receipt.outputHashes);
  const generatedHashes = object(receipt.generatedOutputHashes);
  const expectedOutputs = [...GENERATED_BYTE_COMPARE_OUTPUT_PATHS].sort();
  for (const [label, values] of [
    ["checked-in", checkedHashes],
    ["generated", generatedHashes],
  ] as const) {
    if (
      JSON.stringify(Object.keys(values).sort()) !==
        JSON.stringify(expectedOutputs)
    ) {
      errors.push(label + " output hashes must contain exactly tokens.json and tokens.css");
    }
  }

  const outputs = Array.isArray(receipt.outputs) ? receipt.outputs : [];
  if (outputs.length !== GENERATED_BYTE_COMPARE_OUTPUT_PATHS.length) {
    errors.push("exporter receipt requires exactly two distinct output observations");
  }
  for (const path of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
    const checked = checkedHashes[path];
    const generated = generatedHashes[path];
    const matches = outputs
      .map((entry) => object(entry))
      .filter((entry) => entry.path === path);
    if (
      typeof checked !== "string" ||
      !/^[a-f0-9]{64}$/.test(checked) ||
      generated !== checked ||
      matches.length !== 1 ||
      matches[0]?.checkedInSha256 !== checked ||
      matches[0]?.generatedSha256 !== checked ||
      matches[0]?.byteEqual !== true ||
      !Number.isSafeInteger(matches[0]?.byteLength) ||
      Number(matches[0]?.byteLength) < 0
    ) {
      errors.push("checked-in and generated bytes disagree or are missing: " + path);
    } else if (
      identity.currentFileSha256 !== undefined &&
      identity.currentFileSha256(path) !== checked
    ) {
      errors.push("checked-in generated output changed after comparison: " + path);
    }
  }

  const safety = object(receipt.safety);
  if (
    safety.noninteractive !== true ||
    safety.startsExporter !== true ||
    safety.isolatedTempOutput !== true
  ) {
    errors.push("exporter proof must be a noninteractive isolated non-GUI execution");
  }
  for (const field of [
    "startsApplication",
    "revealsWindow",
    "focusesWindow",
    "drivesNativeInput",
    "capturesScreen",
    "accessesNetwork",
    "usesLiveAi",
  ]) {
    if (safety[field] !== false) {
      errors.push("exporter proof must prohibit " + field);
    }
  }
  const execution = object(receipt.execution);
  if (
    execution.exitCode !== 0 ||
    typeof execution.stdoutSha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(execution.stdoutSha256) ||
    typeof execution.stderrSha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(execution.stderrSha256)
  ) {
    errors.push("exporter execution status or diagnostic identity is invalid");
  }
  const cleanup = object(receipt.cleanup);
  if (
    cleanup.closed !== true || cleanup.processExited !== true || cleanup.processGroupExited !== true ||
    cleanup.streamsDrained !== true || cleanup.logWriterClosed !== true || cleanup.referencesFinalized !== true ||
    !Array.isArray(cleanup.survivors) ||
    cleanup.survivors.length !== 0
  ) {
    errors.push("exporter proof requires completed survivor-free cleanup");
  }
  return { pass: errors.length === 0, errors };
}

export function parseGeneratedByteCompareArgs(argv: readonly string[]): GeneratedByteCompareOptions {
  const values = new Map<string, string>();
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index]; const value = argv[index + 1];
    if (!flag || !["--artifact", "--out", "--source-policy"].includes(flag) || !value || value.startsWith("--") || values.has(flag))
      failure("expected --artifact <reference.json> --out <fresh-receipt> [--source-policy current-content|clean-exact-head]");
    values.set(flag, value);
  }
  const path = values.get("--artifact"); const outputPath = values.get("--out"); const sourcePolicy = values.get("--source-policy") ?? "current-content";
  if (!path || !outputPath || !["current-content", "clean-exact-head"].includes(sourcePolicy)) failure("explicit artifact, fresh output and valid policy required");
  return { artifactReference: readArtifactReference(path), outputPath, sourcePolicy: sourcePolicy as ArtifactExpectation["sourcePolicy"] };
}

if (import.meta.main) {
  if (process.argv.includes("--help")) console.log("generated-byte-compare --artifact <reference.json> --out <fresh-receipt.json> [--source-policy clean-exact-head]");
  else {
    const receipt = await generateAuthoritativeByteComparison(parseGeneratedByteCompareArgs(process.argv.slice(2)));
    console.log(JSON.stringify(receipt)); process.exitCode = receipt.pass ? 0 : 2;
  }
}
