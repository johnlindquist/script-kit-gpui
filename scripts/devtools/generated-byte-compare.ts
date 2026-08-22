#!/usr/bin/env bun
/**
 * Run the real prebuilt, non-GUI design-token exporter into a private temp
 * directory and compare its JSON/CSS bytes to the checked-in contract.
 *
 * This is exporter UNIT_BEHAVIOR evidence, not application-runtime evidence.
 * The producer never builds the binary, starts Script Kit, scans the checkout,
 * writes checked-in artifacts, or contacts an AI provider.
 */
import { spawnSync } from "node:child_process";
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

export const GENERATED_BYTE_COMPARE_SOURCE_PATHS = [
  "Cargo.toml",
  "src/bin/export_design_tokens.rs",
  "src/design_contract/mod.rs",
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
  readFile?: (path: string) => Uint8Array;
  resolveRealPath?: (path: string) => string;
  fileStats?: (path: string) => {
    isFile(): boolean;
    mode: number;
    size: number;
  };
  createTemporaryDirectory?: () => string;
  removeTemporaryDirectory?: (path: string) => void;
  pathExists?: (path: string) => boolean;
  currentSourceSha?: () => string | null;
  runExporter?: (
    binaryPath: string,
    arguments_: readonly string[],
    environment: Record<string, string | undefined>,
  ) => ExporterProcessResult;
}

export interface GeneratedByteCompareOptions {
  binaryPath: string;
  sourceSha: string;
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

export function generateAuthoritativeByteComparison(
  options: GeneratedByteCompareOptions,
  dependencies: GeneratedByteCompareDependencies = {},
) {
  if (!options.binaryPath || basename(options.binaryPath) !== "export_design_tokens") {
    failure("an explicit existing export_design_tokens binary path is required");
  }
  if (!/^[a-f0-9]{40}$/i.test(options.sourceSha)) {
    failure("--source-sha must be the exact 40-character source commit");
  }

  const repositoryRoot = resolve(dependencies.repositoryRoot ?? process.cwd());
  const environment = dependencies.environment ?? process.env;
  const safetyErrors = validateGeneratedByteCompareEnvironment(environment);
  if (safetyErrors.length > 0) failure(safetyErrors.join("; "));

  const readFile = dependencies.readFile ??
    ((path: string) => readFileSync(path));
  const realPath = dependencies.resolveRealPath ?? realpathSync;
  const fileStats = dependencies.fileStats ?? statSync;
  const sourceSha = options.sourceSha.toLowerCase();
  const currentSourceSha = dependencies.currentSourceSha ??
    (() => readCurrentGitCommit(repositoryRoot, readFile));
  if (currentSourceSha() !== sourceSha) {
    failure("the supplied source commit does not match the current checkout");
  }

  let binaryPath: string;
  let binaryStats: ReturnType<typeof fileStats>;
  try {
    binaryPath = realPath(resolve(repositoryRoot, options.binaryPath));
    binaryStats = fileStats(binaryPath);
  } catch {
    failure("the explicit exporter binary is missing");
  }
  const binaryRelativePath = relative(repositoryRoot, binaryPath);
  if (
    basename(binaryPath) !== "export_design_tokens" ||
    binaryRelativePath === "" ||
    binaryRelativePath.startsWith(".." + "/") ||
    binaryRelativePath === ".." ||
    isAbsolute(binaryRelativePath)
  ) {
    failure("the exporter binary must stay inside this repository");
  }
  if (!binaryStats.isFile() || (binaryStats.mode & 0o111) === 0) {
    failure("the exporter binary must be an executable regular file");
  }

  const binaryBytes = readFile(binaryPath);
  const binarySha256 = sha256(binaryBytes);
  const sourceFingerprints = fingerprintSources(repositoryRoot, readFile);
  const checkedInBytes = new Map<string, Uint8Array>();
  for (const path of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
    try {
      checkedInBytes.set(path, readFile(resolve(repositoryRoot, path)));
    } catch {
      failure("checked-in generated output is missing: " + path);
    }
  }

  const makeTemporaryDirectory = dependencies.createTemporaryDirectory ??
    (() => mkdtempSync(join(tmpdir(), "script-kit-exporter-byte-")));
  const removeTemporaryDirectory = dependencies.removeTemporaryDirectory ??
    ((path: string) => rmSync(path, { recursive: true, force: true }));
  const pathExists = dependencies.pathExists ?? existsSync;
  const runExporter = dependencies.runExporter ??
    ((binary: string, arguments_: readonly string[], childEnvironment) =>
      spawnSync(binary, [...arguments_], {
        cwd: repositoryRoot,
        env: childEnvironment,
        encoding: "buffer",
        timeout: 30_000,
        maxBuffer: 2_000_000,
      }));
  const temporaryDirectory = makeTemporaryDirectory();
  const temporaryRelation = relative(repositoryRoot, temporaryDirectory);
  if (
    !isAbsolute(temporaryDirectory) ||
    temporaryRelation === "" ||
    (!temporaryRelation.startsWith(".." + "/") &&
      temporaryRelation !== ".." &&
      !isAbsolute(temporaryRelation))
  ) {
    removeTemporaryDirectory(temporaryDirectory);
    failure("exporter output must use an isolated external temporary directory");
  }

  let execution: ExporterProcessResult | undefined;
  let outputHashes: Record<string, string> | undefined;
  let generatedOutputHashes: Record<string, string> | undefined;
  let outputs:
    | Array<{
      path: string;
      checkedInSha256: string;
      generatedSha256: string;
      byteEqual: true;
      byteLength: number;
    }>
    | undefined;
  try {
    execution = runExporter(binaryPath, [temporaryDirectory], {
      ...environment,
      SCRIPT_KIT_NONINTERACTIVE: "1",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
      SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
      SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
      SCRIPT_KIT_ALLOW_LIVE_AI: "0",
      SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
      SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
    });
    if (
      execution.error ||
      execution.status !== 0 ||
      execution.signal !== undefined && execution.signal !== null
    ) {
      failure("the non-GUI exporter failed before completing byte comparison");
    }

    outputs = [];
    outputHashes = {};
    generatedOutputHashes = {};
    for (const path of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
      let generatedBytes: Uint8Array;
      try {
        generatedBytes = readFile(join(temporaryDirectory, basename(path)));
      } catch {
        failure("authoritative exporter did not produce " + basename(path));
      }
      const checkedBytes = checkedInBytes.get(path)!;
      if (!Buffer.from(checkedBytes).equals(Buffer.from(generatedBytes))) {
        failure("checked-in output differs from exporter bytes: " + path);
      }
      const currentCheckedBytes = readFile(resolve(repositoryRoot, path));
      if (!Buffer.from(checkedBytes).equals(Buffer.from(currentCheckedBytes))) {
        failure("the exporter changed checked-in generated output: " + path);
      }
      const checkedSha = sha256(checkedBytes);
      const generatedSha = sha256(generatedBytes);
      outputHashes[path] = checkedSha;
      generatedOutputHashes[path] = generatedSha;
      outputs.push({
        path,
        checkedInSha256: checkedSha,
        generatedSha256: generatedSha,
        byteEqual: true,
        byteLength: checkedBytes.byteLength,
      });
    }

    if (sha256(readFile(binaryPath)) !== binarySha256) {
      failure("the exporter binary changed during execution");
    }
    if (!sameIdentity(sourceFingerprints, fingerprintSources(repositoryRoot, readFile))) {
      failure("an exporter source owner changed during execution");
    }
    if (currentSourceSha() !== sourceSha) {
      failure("the checkout source commit changed during execution");
    }
  } finally {
    removeTemporaryDirectory(temporaryDirectory);
  }

  if (pathExists(temporaryDirectory)) {
    failure("the exporter temporary directory survived cleanup");
  }

  const receipt = {
    schemaVersion: 1 as const,
    generatedBy: "scripts/devtools/generated-byte-compare.ts" as const,
    taskId: "GOV-005" as const,
    evidenceClass: "UNIT_BEHAVIOR" as const,
    provesRuntimeBehavior: false as const,
    sourceSha,
    sourceCoverage: {
      mode: "DECLARED_EXPORTER_SOURCE_OWNERS" as const,
      sourceGraphExhaustive: false as const,
    },
    sourceFingerprints,
    binary: {
      path: binaryRelativePath,
      sha256: binarySha256,
      sizeBytes: binaryStats.size,
    },
    outputHashes: outputHashes!,
    generatedOutputHashes: generatedOutputHashes!,
    outputs: outputs!,
    byteEqual: true as const,
    handEditedGeneratedOutput: false as const,
    safety: {
      noninteractive: true as const,
      startsApplication: false as const,
      revealsWindow: false as const,
      focusesWindow: false as const,
      drivesNativeInput: false as const,
      capturesScreen: false as const,
      accessesNetwork: false as const,
      usesLiveAi: false as const,
      startsExporter: true as const,
      isolatedTempOutput: true as const,
    },
    execution: {
      exitCode: execution!.status,
      stdoutSha256: sha256(execution!.stdout ?? ""),
      stderrSha256: sha256(execution!.stderr ?? ""),
    },
    cleanup: { closed: true as const, survivors: [] as string[] },
    disposition: "EVALUABLE_PASS" as const,
    pass: true as const,
  };
  const selfValidation = validateGeneratedByteCompareReceipt(receipt, {
    currentSourceSha: sourceSha,
    currentFileSha256(path) {
      try {
        return sha256(readFile(resolve(repositoryRoot, path)));
      } catch {
        return null;
      }
    },
  });
  if (!selfValidation.pass) {
    failure("completed exporter receipt failed identity validation: " +
      selfValidation.errors.join("; "));
  }
  return receipt;
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
  const receipt = object(candidate);
  const errors: string[] = [];
  if (
    receipt.schemaVersion !== 1 ||
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
    cleanup.closed !== true ||
    !Array.isArray(cleanup.survivors) ||
    cleanup.survivors.length !== 0
  ) {
    errors.push("exporter proof requires completed survivor-free cleanup");
  }
  return { pass: errors.length === 0, errors };
}

export function parseGeneratedByteCompareArgs(argv: readonly string[]): {
  binaryPath: string;
  sourceSha: string;
  outputPath?: string;
} {
  const values = new Map<string, string>();
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (
      (flag !== "--binary" && flag !== "--source-sha" && flag !== "--out") ||
      value === undefined ||
      value.startsWith("--") ||
      values.has(flag)
    ) {
      failure("expected --binary <export_design_tokens> --source-sha <40-hex> [--out <receipt>]");
    }
    values.set(flag, value);
  }
  const binaryPath = values.get("--binary");
  const sourceSha = values.get("--source-sha");
  if (!binaryPath || !sourceSha) {
    failure("both --binary and --source-sha are required");
  }
  return { binaryPath, sourceSha, outputPath: values.get("--out") };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  if (argv.length === 1 && (argv[0] === "--help" || argv[0] === "-h")) {
    console.log(
      "Usage: bun scripts/devtools/generated-byte-compare.ts " +
        "--binary <prebuilt/export_design_tokens> --source-sha <40-hex> " +
        "[--out .artifacts/consistency/GOV-005/generated-byte-compare.json]",
    );
    process.exit(0);
  }
  try {
    const arguments_ = parseGeneratedByteCompareArgs(argv);
    if (arguments_.outputPath !== undefined) {
      const expected = resolve(GENERATED_BYTE_COMPARE_RECEIPT_PATH);
      if (resolve(arguments_.outputPath) !== expected) {
        failure("receipt output must be exactly " + GENERATED_BYTE_COMPARE_RECEIPT_PATH);
      }
    }
    const receipt = generateAuthoritativeByteComparison(arguments_);
    if (arguments_.outputPath !== undefined) {
      const path = resolve(arguments_.outputPath);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, JSON.stringify(receipt, null, 2) + "\n");
    }
    console.log(JSON.stringify(receipt, null, 2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(2);
  }
}
