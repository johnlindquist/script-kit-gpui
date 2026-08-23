import { createHash } from "node:crypto";
import { existsSync, lstatSync, readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import {
  currentIdentity,
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  parseTaskCatalog,
} from "../consistency.ts";
import type { Driver, Json } from "../driver.ts";
import { nativeFooterActivationProof } from "../focus.ts";
import { classifyReceiptEvidence } from "./evidence-class.ts";
import {
  prepareValidatedReceipt,
  producerIdentityForTool,
  RECEIPT_SCHEMA_VERSION,
  receiptRegistryIdentity,
  receiptSchema,
  RUNTIME_TASK_PROOF_SPECS,
  type RuntimeTaskProofId,
} from "./receipt-schema.ts";
import {
  proofTransactionIdentity,
  strictTransactionMissingFields,
  targetIdentity,
  type ProofTransactionIdentity,
} from "./target-identity.ts";

type Obj = Record<string, unknown>;

export type RuntimeNegativeControls = Record<string, boolean> | Array<{ id: string; pass: boolean }>;

export type RuntimeTargetObservation = {
  requestedTarget: Obj;
  target: Obj;
  transaction: Obj;
  binary: Obj;
};

function asObject(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Obj : {};
}

function sha256File(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function currentGitHead(): string {
  const head = currentIdentity().headCommit;
  if (!head || !/^[a-f0-9]{40}$/.test(head)) {
    throw new Error("runtime task proof requires the exact current Git source commit");
  }
  return head;
}

const COMPILER_INPUT_PATH_OWNER = "scripts/agentic/compiler-input-paths.txt";
const compilerTreeCache = new Map<string, string>();

function reviewedCompilerInputPaths(): string[] {
  const paths = readFileSync(COMPILER_INPUT_PATH_OWNER, "utf8")
    .split(/\r?\n/)
    .filter(Boolean);
  if (
    paths.length === 0 ||
    new Set(paths).size !== paths.length ||
    paths.some((path) => path.startsWith("/") || path.split("/").includes(".."))
  ) {
    throw new Error("verified build provenance requires one valid reviewed compiler-input owner");
  }
  return paths;
}

/** Git tree objects are immutable; identical reviewed trees safely survive docs-only commits. */
export function reviewedCompilerInputFingerprint(commit = currentGitHead()): string {
  if (!/^[a-f0-9]{40}$/.test(commit)) {
    throw new Error("reviewed compiler-input tree requires one exact Git commit");
  }
  const ownerHash = sha256File(COMPILER_INPUT_PATH_OWNER);
  const key = `${commit}:${ownerHash}`;
  const cached = compilerTreeCache.get(key);
  if (cached) return cached;
  const result = Bun.spawnSync([
    "git",
    "-C",
    process.cwd(),
    "ls-tree",
    "-r",
    commit,
    "--",
    ...reviewedCompilerInputPaths(),
  ], { stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0 || result.stdout.byteLength === 0) {
    throw new Error("verified build provenance cannot independently resolve its reviewed compiler-input tree");
  }
  const fingerprint = createHash("sha256").update(result.stdout).digest("hex");
  compilerTreeCache.set(key, fingerprint);
  return fingerprint;
}

function requireCleanCompilerInputs() {
  const result = Bun.spawnSync([
    "git",
    "-C",
    process.cwd(),
    "status",
    "--porcelain",
    "--untracked-files=all",
    "--",
    ...reviewedCompilerInputPaths(),
  ], { stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0 || result.stdout.byteLength !== 0) {
    throw new Error("verified build provenance rejects uncommitted current compiler-input sources");
  }
}

/** Bind one actual executable to the manifest written by its owned build/export. */
export function verifyRuntimeBinaryProvenance(binaryPath: string, expected: Obj = {}): Obj {
  const repositoryRoot = realpathSync(process.cwd());
  const resolvedBinary = resolve(repositoryRoot, binaryPath);
  const repositoryRelative = relative(repositoryRoot, resolvedBinary);
  if (
    repositoryRelative.startsWith("../") || repositoryRelative.startsWith("/") ||
    !(repositoryRelative.startsWith("target-agent/artifacts/") ||
      repositoryRelative.startsWith("target-agent/runtime/"))
  ) {
    throw new Error("verified build provenance requires one owned exported or staged runtime binary");
  }
  let canonicalBinary: string;
  try {
    canonicalBinary = realpathSync(resolvedBinary);
  } catch {
    throw new Error("verified build provenance cannot independently observe its runtime binary");
  }
  if (canonicalBinary !== resolvedBinary || lstatSync(resolvedBinary).isSymbolicLink()) {
    throw new Error("verified build provenance cannot follow a binary symlink");
  }

  const candidates = [
    `${resolvedBinary}.provenance.json`,
    join(dirname(resolvedBinary), "manifest.json"),
  ].filter((candidate) => existsSync(candidate));
  if (candidates.length !== 1) {
    throw new Error("verified build provenance requires exactly one independently observed artifact manifest");
  }
  const manifestPath = candidates[0]!;
  if (lstatSync(manifestPath).isSymbolicLink() || realpathSync(manifestPath) !== manifestPath) {
    throw new Error("verified build provenance cannot follow an artifact manifest symlink");
  }
  const manifestBytes = readFileSync(manifestPath);
  let manifest: Obj;
  try {
    manifest = asObject(JSON.parse(manifestBytes.toString("utf8")));
  } catch {
    throw new Error("verified build provenance manifest is not valid JSON");
  }
  const binaryStat = statSync(resolvedBinary);
  const binaryHash = sha256File(resolvedBinary);
  if (
    manifest.schemaVersion !== 2 ||
    manifest.binaryPath !== repositoryRelative ||
    manifest.binarySha256 !== binaryHash ||
    manifest.sizeBytes !== binaryStat.size ||
    typeof manifest.compilerInputSha256 !== "string" ||
    !/^[a-f0-9]{64}$/.test(manifest.compilerInputSha256) ||
    typeof manifest.profile !== "string" || !/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(manifest.profile) ||
    typeof manifest.requiresExactGitHead !== "boolean" ||
    manifest.profile === "release" && manifest.requiresExactGitHead !== true ||
    typeof manifest.pool !== "string" || !/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(manifest.pool) ||
    typeof manifest.source !== "string" ||
    !(manifest.source.startsWith("target-agent/pools/") ||
      manifest.source.startsWith("target-agent/agents/")) ||
    manifest.source.split("/").includes("..") ||
    typeof manifest.builtAt !== "string" || Number.isNaN(Date.parse(manifest.builtAt))
  ) {
    throw new Error("verified build provenance manifest does not match its exact owned executable bytes");
  }
  if (manifest.rustDirty !== false) {
    throw new Error("verified build provenance rejects uncommitted compiler-input sources");
  }
  const head = currentGitHead();
  if (manifest.compilerInputSha256 !== reviewedCompilerInputFingerprint(head)) {
    throw new Error("verified build provenance executable was not built from the current reviewed compiler-input tree");
  }
  requireCleanCompilerInputs();
  if (manifest.gitHead !== head) {
    if (manifest.requiresExactGitHead === true) {
      throw new Error("release, CI, and explicit Git tracking require the exact build commit");
    }
    if (typeof manifest.gitHead !== "string" || !/^[a-f0-9]{40}$/.test(manifest.gitHead)) {
      throw new Error("verified build provenance executable was not built from a valid source commit");
    }
    const ancestor = Bun.spawnSync([
      "git",
      "-C",
      process.cwd(),
      "merge-base",
      "--is-ancestor",
      manifest.gitHead,
      head,
    ], { stdout: "pipe", stderr: "pipe" });
    if (
      ancestor.exitCode !== 0 ||
      reviewedCompilerInputFingerprint(manifest.gitHead) !== manifest.compilerInputSha256
    ) {
      throw new Error("verified build provenance executable was not built from an equivalent current source commit");
    }
  }

  const manifestRelative = relative(repositoryRoot, manifestPath);
  const manifestHash = createHash("sha256").update(manifestBytes).digest("hex");
  if (
    (expected.path !== undefined && expected.path !== repositoryRelative) ||
    (expected.sha256 !== undefined && expected.sha256 !== binaryHash) ||
    (expected.sourceCommit !== undefined && expected.sourceCommit !== head)
  ) {
    throw new Error("verified build provenance identity does not match the observed executable");
  }
  if (Object.keys(expected).length > 0) {
    const declaredManifest = asObject(expected.provenance);
    if (
      declaredManifest.path !== manifestRelative ||
      declaredManifest.sha256 !== manifestHash ||
      declaredManifest.builtGitHead !== manifest.gitHead ||
      declaredManifest.compilerInputSha256 !== manifest.compilerInputSha256 ||
      declaredManifest.profile !== manifest.profile ||
      declaredManifest.requiresExactGitHead !== manifest.requiresExactGitHead
    ) {
      throw new Error("verified build provenance manifest identity does not match its observed receipt");
    }
  }

  return {
    path: repositoryRelative,
    sha256: binaryHash,
    sizeBytes: binaryStat.size,
    modifiedAt: new Date(binaryStat.mtimeMs).toISOString(),
    sourceCommit: head,
    pinned: Boolean(process.env.SCRIPT_KIT_GPUI_BINARY),
    provenance: {
      path: manifestRelative,
      sha256: manifestHash,
      schemaVersion: manifest.schemaVersion,
      pool: manifest.pool,
      builtGitHead: manifest.gitHead,
      compilerInputSha256: manifest.compilerInputSha256,
      profile: manifest.profile,
      requiresExactGitHead: manifest.requiresExactGitHead,
      rustDirty: false,
    },
  };
}

function canonicalTaskBinding(taskId: RuntimeTaskProofId) {
  const catalog = parseTaskCatalog(
    readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
    DEFAULT_CONSISTENCY_CATALOG_PATH,
  );
  const entry = catalog.byId.get(taskId);
  if (!entry || catalog.errors.length > 0) {
    throw new Error(`${taskId} does not resolve to exactly one current canonical catalog section`);
  }
  return {
    catalogPath: DEFAULT_CONSISTENCY_CATALOG_PATH,
    taskId,
    title: entry.title,
    sectionSha256: entry.sectionSha256,
  };
}

export function runtimeTaskProofSourceOwners(taskId: RuntimeTaskProofId): string[] {
  const spec = RUNTIME_TASK_PROOF_SPECS[taskId];
  return [
    "scripts/devtools/lib/runtime-task-proof.ts",
    COMPILER_INPUT_PATH_OWNER,
    "scripts/devtools/lib/receipt-schema.ts",
    spec.productionOwner,
    spec.runtimeProducer,
  ];
}

function normalizeNegativeControls(value: RuntimeNegativeControls): Array<{ id: string; pass: boolean }> {
  return Array.isArray(value)
    ? value.map((control) => ({ id: control.id, pass: control.pass }))
    : Object.entries(value).map(([id, pass]) => ({ id, pass }));
}

function assertNegativeControls(taskId: RuntimeTaskProofId, controls: RuntimeNegativeControls) {
  const normalized = normalizeNegativeControls(controls);
  const ids = normalized.map((control) => control.id);
  if (ids.some((id) => typeof id !== "string" || id.length === 0) || new Set(ids).size !== ids.length) {
    throw new Error(`${taskId} runtime proof requires uniquely identified executed negative controls`);
  }
  if (normalized.some((control) => control.pass !== true)) {
    throw new Error(`${taskId} runtime proof contains a failed or unexecuted negative control`);
  }
  const missing = RUNTIME_TASK_PROOF_SPECS[taskId].negativeControlIds
    .filter((id) => !ids.includes(id));
  if (missing.length > 0) {
    throw new Error(`${taskId} runtime proof is missing required negative controls: ${missing.join(", ")}`);
  }
  return normalized;
}

function assertCleanup(cleanupValue: unknown): Obj {
  const cleanup = asObject(cleanupValue);
  const scenarios = ["enabled", "disabled"]
    .map((name) => asObject(cleanup[name]))
    .filter((scenario) => Object.keys(scenario).length > 0);
  const observed = scenarios.length > 0 ? scenarios : [cleanup];
  if (
    observed.some((entry) =>
      entry.processExited !== true || entry.streamsDrained !== true ||
      entry.logWriterClosed !== true || entry.ownedProcessCount !== 0 ||
      entry.closeError != null || entry.clipboardTouched !== false
    ) ||
    (cleanup.ownedProcessCount != null && cleanup.ownedProcessCount !== 0)
  ) {
    throw new Error("runtime task proof requires observed process, stream, session, and privacy cleanup");
  }
  return {
    ...cleanup,
    closed: true,
    ownedPids: [],
    ownedSessions: [],
    ownedBrowserPids: [],
    survivors: [],
  };
}

function assertTaskSpecificObservation(taskId: RuntimeTaskProofId, candidate: Obj) {
  if (taskId === "PF-006") {
    for (const surface of ["notes", "dayPage"]) {
      const summary = asObject(candidate[surface]);
      if (
        !Number.isSafeInteger(summary.count) || Number(summary.count) < 2 ||
        summary.fullDisplayPass !== true || summary.rawContentReturned !== false
      ) {
        throw new Error(`${taskId} requires actual private-safe full-display ${surface} glyph evidence`);
      }
    }
    return;
  }

  if (taskId === "PF-007") {
    const evidence = asObject(candidate.activationEvidence);
    for (const [name, disabled] of [["enabled", false], ["disabled", true]] as const) {
      const proof = asObject(evidence[name]);
      const observed = nativeFooterActivationProof({
        host: proof.host,
        actionId: proof.actionId,
        ok: proof.resultOk,
        errorCode: proof.resultErrorCode,
        nativeFooterActivation: proof.activation,
      }, String(proof.expectedSemanticId ?? ""), proof.postconditionObserved === true, disabled);
      if (proof.complete !== true || observed.complete !== true) {
        throw new Error(`${taskId} requires observed ${name} native activation and its real postcondition`);
      }
    }
    return;
  }

  if (taskId === "PF-008") {
    const selected = asObject(candidate.selectedRow);
    const transaction = asObject(selected.transaction);
    const before = asObject(transaction.before);
    const after = asObject(transaction.after);
    const target = asObject(candidate.transaction);
    const selectedSemanticId = asObject(candidate.scroll).selectedSemanticId;
    const observedFingerprint = typeof selectedSemanticId === "string"
      ? createHash("sha256").update(selectedSemanticId).digest("hex")
      : null;
    if (
      selected.selectionChanged !== true ||
      selected.semanticIdReturnedRaw !== false ||
      selected.semanticIdSha256 !== observedFingerprint ||
      transaction.stableWindowInstance !== true ||
      transaction.stableTargetGeneration !== true ||
      transaction.stableSurfaceGeneration !== true ||
      transaction.dataGenerationAdvanced !== true ||
      transaction.dataGenerationPresent !== true ||
      typeof before.windowInstanceId !== "string" ||
      before.windowInstanceId !== after.windowInstanceId ||
      after.windowInstanceId !== target.windowInstanceId ||
      before.targetGeneration !== after.targetGeneration ||
      after.targetGeneration !== target.targetGeneration ||
      before.surfaceGeneration !== after.surfaceGeneration ||
      after.surfaceGeneration !== target.surfaceGeneration ||
      !Number.isSafeInteger(before.dataGeneration) ||
      !Number.isSafeInteger(after.dataGeneration) ||
      Number(after.dataGeneration) <= Number(before.dataGeneration) ||
      after.dataGeneration !== target.dataGeneration
    ) {
      throw new Error(`${taskId} requires an actual exact-target selection and advancing data-generation transaction`);
    }
  }
}

/** Capture identity from the actual live target; never infer a window or process. */
export async function observeRuntimeTaskTarget(
  driver: Driver,
  binaryPath: string,
  selector: Json = { type: "main" },
): Promise<RuntimeTargetObservation> {
  const verifiedBinary = verifyRuntimeBinaryProvenance(binaryPath);
  const windows = asObject(await driver.listAutomationWindows({ timeoutMs: 5_000 }));
  const inspection = asObject(await driver.request({
    type: "inspectAutomationWindow",
    target: selector,
    hiDpi: false,
    probes: [],
  }, { expect: "automationInspectResult", timeoutMs: 8_000 }));
  const identity = targetIdentity(
    { target: selector as Obj, strict: true, expectedSurfaceKind: "" },
    asObject(inspection.snapshot ?? inspection),
    windows,
  );
  const target = identity.resolvedTarget as Obj;
  if (target.strictTargetMatch !== true || asObject(target.strictTargetMismatch).automationId) {
    throw new Error("runtime task proof requires one strictly resolved observed target");
  }
  if (typeof target.visible !== "boolean") {
    throw new Error("runtime task proof requires an independently observed target visibility");
  }
  if (target.pid !== driver.pid) {
    throw new Error("runtime task proof target process does not match its actual owned driver");
  }

  const transaction = proofTransactionIdentity(driver.sessionName, target) as unknown as Obj;
  transaction.binarySha256 = verifiedBinary.sha256;
  const missing = strictTransactionMissingFields(transaction as ProofTransactionIdentity);
  if (missing.length > 0) {
    throw new Error(`runtime task proof target lacks strict transaction identity: ${missing.join(", ")}`);
  }
  return {
    requestedTarget: identity.requestedTarget as Obj,
    target,
    transaction,
    binary: verifiedBinary,
  };
}

/** Bind an actually observed primitive to its one exact runtime catalog obligation. */
export function prepareRuntimeTaskProof(
  taskId: RuntimeTaskProofId,
  candidate: Obj,
  executedControls: RuntimeNegativeControls,
) {
  const spec = RUNTIME_TASK_PROOF_SPECS[taskId];
  if (!spec) throw new Error(`unknown or offline-only runtime task: ${taskId}`);
  const definition = receiptSchema(spec.primitiveId);
  if (!definition) throw new Error(`${taskId} has no registered runtime primitive`);
  if (
    (candidate.primitiveId != null && candidate.primitiveId !== spec.primitiveId) ||
    candidate.tool !== definition.tool ||
    !definition.commands.includes(String(candidate.command ?? ""))
  ) {
    throw new Error(`${taskId} requires its actual ${spec.primitiveId} production primitive`);
  }
  if (candidate.classification !== "ok") {
    throw new Error(`${taskId} has no passing directly observed runtime result`);
  }
  const observedMode = spec.primitiveId === "devtools.elements.snapshot"
    ? asObject(candidate.semanticProjection).proofMode
    : spec.primitiveId === "devtools.scroll.inspect"
      ? (asObject(candidate.renderedSafeViewport).required === true
        ? "rendered-safe-viewport"
        : null)
      : candidate.proofMode;
  if (observedMode !== spec.proofMode) {
    throw new Error(`${taskId} requires actual ${spec.proofMode} runtime observation`);
  }
  assertTaskSpecificObservation(taskId, candidate);

  const target = asObject(candidate.target);
  if (typeof target.visible !== "boolean") {
    throw new Error(`${taskId} requires observed hidden or visible runtime target identity`);
  }
  const observedEvidence = classifyReceiptEvidence(candidate);
  if (!["RUNTIME_HIDDEN", "RUNTIME_VISIBLE", "PACKAGED_APP", "DIRECT_RUNTIME_PROOF"]
      .includes(observedEvidence.evidenceClass) || observedEvidence.errors.length > 0) {
    throw new Error(`${taskId} cannot promote ${observedEvidence.evidenceClass} into direct runtime evidence`);
  }

  const head = currentGitHead();
  const existingRepository = asObject(candidate.repository);
  if (existingRepository.gitCommit != null && existingRepository.gitCommit !== head) {
    throw new Error(`${taskId} runtime evidence source commit does not match current HEAD`);
  }
  const binary = asObject(candidate.binary);
  const transaction = asObject(candidate.transaction);
  if (
    typeof binary.path !== "string" || binary.path.length === 0 ||
    typeof binary.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(binary.sha256) ||
    binary.sourceCommit !== head || transaction.binarySha256 !== binary.sha256
  ) {
    throw new Error(`${taskId} requires one current-source binary and matching proof transaction`);
  }
  let currentBinarySha: string;
  try {
    currentBinarySha = sha256File(binary.path);
  } catch {
    throw new Error(`${taskId} runtime binary cannot be independently observed`);
  }
  if (currentBinarySha !== binary.sha256) {
    throw new Error(`${taskId} runtime binary bytes do not match their observed fingerprint`);
  }
  try {
    verifyRuntimeBinaryProvenance(binary.path as string, binary);
  } catch (error) {
    throw new Error(`${taskId} requires verified build provenance: ${String(error)}`);
  }

  const catalogBinding = canonicalTaskBinding(taskId);
  const existingBinding = asObject(candidate.catalogBinding);
  if (
    Object.keys(existingBinding).length > 0 &&
    (existingBinding.taskId !== taskId || existingBinding.title !== catalogBinding.title ||
      existingBinding.sectionSha256 !== catalogBinding.sectionSha256)
  ) {
    throw new Error(`${taskId} runtime receipt attempted to reuse a different catalog obligation`);
  }

  const negativeControls = assertNegativeControls(taskId, executedControls);
  const cleanup = assertCleanup(candidate.cleanup);
  const sourceFingerprints = Object.fromEntries(
    runtimeTaskProofSourceOwners(taskId).map((path) => [path, sha256File(path)]),
  );
  const producerIdentity = producerIdentityForTool(definition.tool);
  const prepared = prepareValidatedReceipt(spec.primitiveId, {
    ...candidate,
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    primitiveId: spec.primitiveId,
    taskId,
    taskIds: [taskId],
    catalogBinding,
    runtimeTaskProof: {
      sourceOwners: runtimeTaskProofSourceOwners(taskId),
      productionOwner: spec.productionOwner,
      runtimeProducer: spec.runtimeProducer,
      proofMode: spec.proofMode,
    },
    sourceFingerprints,
    repository: {
      ...existingRepository,
      gitCommit: head,
      implementationFingerprint: producerIdentity.fingerprint,
      producerSourceFingerprint: producerIdentity.fingerprint,
    },
    negativeControls,
    cleanup,
  });
  if (!prepared.validation.valid || prepared.receipt.disposition !== "EVALUABLE_PASS") {
    throw new Error(`${taskId} runtime primitive is invalid: ${prepared.validation.errors.join("; ")}`);
  }
  (prepared.receipt.producerValidation as Obj).registryFingerprint =
    receiptRegistryIdentity().registryFingerprint;
  return prepared;
}

/** Preserve an honest typed block when the runtime never completed its observation. */
export function prepareBlockedRuntimeTaskProof(
  taskId: RuntimeTaskProofId,
  details: {
    stage: string;
    reason?: string | null;
    cleanup?: unknown;
    controls?: RuntimeNegativeControls;
  },
) {
  const spec = RUNTIME_TASK_PROOF_SPECS[taskId];
  if (!spec) throw new Error(`unknown or offline-only runtime task: ${taskId}`);
  const definition = receiptSchema(spec.primitiveId);
  if (!definition) throw new Error(`${taskId} has no registered runtime primitive`);
  const prepared = prepareValidatedReceipt(spec.primitiveId, {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    tool: definition.tool,
    command: definition.commands[0],
    classification: "blocked-by-missing-primitive",
    taskId,
    taskIds: [taskId],
    catalogBinding: canonicalTaskBinding(taskId),
    missingPrimitives: ["completedObservedRuntimeTransaction"],
    runtimeFailure: {
      stage: details.stage,
      reasonSha256: details.reason
        ? createHash("sha256").update(details.reason).digest("hex")
        : null,
    },
    negativeControls: normalizeNegativeControls(details.controls ?? {}),
    cleanup: {
      ...asObject(details.cleanup),
      closed: false,
    },
    errors: [],
    warnings: [],
  });
  if (prepared.receipt.disposition !== "BLOCKED_MISSING_PRIMITIVE") {
    throw new Error(`${taskId} could not produce an honest typed runtime block`);
  }
  return prepared;
}
