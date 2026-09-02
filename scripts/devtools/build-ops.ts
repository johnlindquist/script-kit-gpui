#!/usr/bin/env bun
import { randomUUID } from "node:crypto";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { artifactHash, ArtifactVerificationError, compilerCompatibility, verifyImmutableArtifact } from "../agentic/build-artifact.ts";
import type { ArtifactReference, ArtifactKind } from "../agentic/build-artifact.ts";
import { RETENTION_CANDIDATE_KINDS, beginManagedTask, cacheLease, canonicalJson, claimOutput, emptyOwnedCleanup, finalizeManagedTask, listManagedTasks, managedKeepSet, managedRetentionPlan, managedTaskRecordPath, pruneManagedRecords, readManagedTask, readOwnedJson, updateManagedKeepSet, updateManagedTask, validateOutputTarget } from "../agentic/artifact-lifecycle.ts";
import type { ManagedTask, OwnedCleanup, OwnedProcessIdentity, OutputClaim, RetentionCandidate, TaskIdentity } from "../agentic/artifact-lifecycle.ts";
import { spawnOwnedProcess } from "../agentic/owned-process.ts";
import type { OwnedProcess } from "../agentic/owned-process.ts";
import { boundedObservation, unknownOwnedCleanup } from "./driver.ts";
import { aggregateCleanup } from "./lib/story-contract.ts";
import { BuildResourceError, buildDependencies, buildLimits, buildStorage } from "./lib/build-ops-inventory.ts";
import type { BuildResourceReport } from "./lib/build-ops-inventory.ts";
import { finishReceipt, startClock } from "./lib/client.ts";
import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic, inferredKindForKey, productStatic, secret } from "./lib/privacy.ts";
import { NoninteractiveSafetyError } from "./lib/operator-safety.ts";

const domainPackages = ["sk-clipboard", "sk-protocol", "sk-storage"];
export const BUILD_ACTIONS: Readonly<Record<string, readonly string[]>> = Object.freeze({
  "app-build": ["build", "--locked", "--bin", "script-kit-gpui"],
  "libtest-build": ["test", "--locked", "--lib", "--no-run"],
  "exporter-build": ["build", "--locked", "--profile", "test", "--bin", "export_design_tokens"],
  "app-check": ["check", "--locked", "--lib", "--bin", "script-kit-gpui"],
  "app-clippy": ["clippy", "--locked", "--lib", "--bin", "script-kit-gpui", "--no-deps", "--", "-D", "warnings"],
  "lib-test": ["test", "--locked", "--lib"],
  "integration-test": ["test", "--locked", ...["ai_capability_preflight_contract", "legacy_design_variant_migration", "protocol_batch", "protocol_wait_for", "script_content_model", "window_resize_logic"].flatMap(name => ["--test", name])],
  "domain-test": ["test", "--locked", ...domainPackages.flatMap(name => ["-p", name])],
  "domain-check": ["check", "--locked", ...domainPackages.flatMap(name => ["-p", name])],
  "clipboard-test": ["test", "--locked", "-p", "sk-clipboard"],
  "protocol-test": ["test", "--locked", "-p", "sk-protocol"],
  "storage-test": ["test", "--locked", "-p", "sk-storage"],
  "publish-signed-bundle": ["publish-signed-bundle"],
});
const compilerInputOwner = "scripts/agentic/compiler-input-paths.txt";
// Exact reviewed owners only. New scripts need a reviewed contract, never a guessed test name.
const noncompilerContracts = [
  { owners: ["scripts/devtools/build-ops.ts", "scripts/agentic/build-artifact.ts", "scripts/agentic/build-artifact-fixture.ts", "scripts/agent-check.sh"], tests: ["scripts/devtools/build-ops.test.ts"] },
  { owners: ["scripts/agentic/artifact-lifecycle.ts"], tests: ["scripts/agentic/artifact-lifecycle.test.ts"] },
  { owners: ["scripts/agentic/owned-process.ts", "scripts/agentic/session-supervisor.py"], tests: ["scripts/agentic/owned-process.test.ts", "scripts/agentic/session-stop-ownership.test.ts"] },
  { owners: ["scripts/agentic/session.sh"], tests: ["scripts/agentic/session-stop-ownership.test.ts"] },
  { owners: ["scripts/agentic/agent-cargo.sh", "scripts/agentic/cargo-cache-locks.sh", "scripts/agentic/reuse-rust-test-binary.sh", compilerInputOwner], tests: ["scripts/agentic/cargo-build-policy.test.ts"] },
  { owners: ["dev.sh", "scripts/agentic/dev-cycle.sh", "scripts/agentic/dev-relaunch.sh"], tests: ["scripts/agentic/human-development-shell.test.ts"] },
  { owners: ["scripts/devtools/lib/build-ops-inventory.ts"], tests: ["scripts/devtools/lib/build-ops-inventory.test.ts"] },
  { owners: ["scripts/devtools/driver.ts"], tests: ["scripts/devtools/driver-lifecycle.test.ts", "scripts/devtools/driver-protocol.test.ts"] },
  { owners: ["scripts/devtools/lib/receipt-schema.ts"], tests: ["scripts/devtools/receipt-schema.test.ts", "scripts/devtools/receipt-output.test.ts"] },
  { owners: ["scripts/devtools/lib/privacy.ts"], tests: ["scripts/devtools/privacy.test.ts"] },
  { owners: ["scripts/devtools/lib/story-contract.ts", "scripts/devtools/stories.ts"], tests: ["scripts/devtools/story-contract.test.ts"] },
  { owners: ["scripts/devtools/design.ts", "scripts/devtools/lib/owned-evaluation.ts"], tests: ["scripts/devtools/owned-evaluation.test.ts"] },
  { owners: ["scripts/agentic/launcher-selection-stability-probe.ts", "scripts/agentic/launcher-search-contract.ts", "scripts/agentic/launcher-search-recipes.ts", "scripts/devtools/design.ts"], tests: ["scripts/agentic/launcher-selection-stability-probe.test.ts"] },
  { owners: ["scripts/agentic/launcher-search-receipt.ts", "scripts/agentic/launcher-search-contract.ts", "scripts/agentic/launcher-search-recipes.ts", "scripts/devtools/design.ts", "scripts/devtools/lib/receipt-artifact.ts"], tests: ["scripts/agentic/launcher-search-receipt.test.ts"] },
  { owners: ["scripts/devtools/lib/receipt-artifact.ts", "scripts/devtools/design.ts", "scripts/devtools/stories.ts", "scripts/devtools/lib/receipt-schema.ts", "scripts/devtools/lib/fixture-contract.ts", "scripts/devtools/lib/story-contract.ts", "scripts/devtools/lib/runtime-coverage.ts", "scripts/devtools/consistency.ts", "scripts/devtools/compare.ts", "scripts/devtools/image-diff.ts", "scripts/devtools/generated-byte-compare.ts", "scripts/release-evidence.ts", "scripts/verify.sh"], tests: ["scripts/devtools/receipt-artifact.test.ts"] },
  { owners: ["scripts/devtools/lib/fixture-contract.ts"], tests: ["scripts/devtools/lib/fixture-contract.test.ts"] },
  { owners: ["scripts/devtools/generated-byte-compare.ts"], tests: ["scripts/devtools/generated-byte-compare.test.ts"] },
  { owners: ["scripts/devtools/lib/runtime-coverage.ts"], tests: ["scripts/devtools/runtime-coverage.test.ts"] },
  { owners: ["scripts/devtools/consistency.ts"], tests: ["scripts/devtools/consistency.test.ts"] },
  { owners: ["scripts/devtools/compare.ts"], tests: ["scripts/devtools/compare.test.ts"] },
  { owners: ["scripts/devtools/image-diff.ts"], tests: ["scripts/devtools/image-diff.test.ts"] },
  { owners: ["scripts/release-evidence.ts", "scripts/verify.sh"], tests: ["scripts/release-evidence.test.ts"] },
];
interface ChangedStep { action: string; executor: "cargo" | "bun"; args: string[]; }
export interface ChangedRoute {
  domainOnly: boolean; rustRequired: boolean; selectedPackages: string[]; steps: ChangedStep[]; filterGuessing: false;
  scope: "domain-only" | "full-rust" | "noncompiler-only";
  compilerInputOwner: string; compilerInputOwnerSha256: string;
  documentationPathCount: number; unknownPathCount: number; reasons: string[]; coverageGaps: string[];
  unreviewedPathIndices: number[];
  verificationScope: "selected_checks_only";
}
function canonicalChangedPath(path: string): boolean {
  return !!path && !/[\\\x00-\x1f\x7f:]/.test(path) && !path.startsWith("/") && path.split("/").every(part => !!part && part !== "." && part !== "..");
}
export function routeChanged(paths: readonly string[], quick = false, root = resolve(import.meta.dir, "../..")): ChangedRoute {
  const normalized = paths.map(path => path.replace(/^\.\//, ""));
  if (normalized.some(path => !canonicalChangedPath(path))) throw new Error("invalid_changed_path");
  const ownerText = readFileSync(join(root, compilerInputOwner), "utf8");
  const compilerInputs = [...ownerText.trim().split("\n"), compilerInputOwner];
  if (compilerInputs.some(path => !canonicalChangedPath(path))) throw new Error("invalid_compiler_input_inventory");
  // Match reviewed file/directory roots, including a changed ancestor directory.
  const compilerPaths = normalized.filter(path => compilerInputs.some(input => path === input || path.startsWith(`${input}/`) || input.startsWith(`${path}/`)));
  const tests = new Set<string>();
  const unreviewedPathIndices: number[] = [];
  let documentationPathCount = 0, unknownPathCount = 0;
  for (const [index, path] of normalized.entries()) {
    const contracts = noncompilerContracts.filter(contract => contract.owners.includes(path) || contract.tests.includes(path));
    for (const contract of contracts) for (const test of contract.tests) tests.add(test);
    if (compilerPaths.includes(path) || contracts.length) continue;
    if (["README.md", "scripts/devtools/README.md", "CHANGELOG.md", "CONTRIBUTING.md", "AGENTS.md", "LICENSE", "GLOSSARY.md", "POLISH.md", "VISION.md", "FEATURES.md"].includes(path) ||
      path === ".notes" || path.startsWith(".notes/") || path === ".test-output" || path.startsWith(".test-output/") ||
      path.startsWith("docs/") && path.endsWith(".md")) documentationPathCount++;
    else { unknownPathCount++; unreviewedPathIndices.push(index); }
  }
  const domainOnly = normalized.length > 0 && normalized.every(path => /^crates\/sk-(clipboard|protocol|storage)(\/|$)/.test(path));
  const rustRequired = compilerPaths.length > 0 || unknownPathCount > 0 || normalized.length === 0 || domainOnly;
  const packages = domainOnly ? [...new Set(normalized.map(path => path.split("/")[1]!))].sort() : rustRequired ? domainPackages : [];
  const packageArgs = packages.flatMap(name => ["-p", name]);
  const steps: ChangedStep[] = domainOnly ? [
    { action: "domain-check", executor: "cargo", args: ["check", "--locked", ...packageArgs] },
    ...quick ? [] : [
      { action: "domain-clippy", executor: "cargo" as const, args: ["clippy", "--locked", ...packageArgs, "--all-targets", "--", "-D", "warnings"] },
      { action: "domain-test", executor: "cargo" as const, args: ["test", "--locked", ...packageArgs] },
    ],
  ] : rustRequired ? ["app-check", ...quick ? [] : ["app-clippy", "lib-test", "integration-test", "domain-test"]].map(action => ({ action, executor: "cargo" as const, args: [...BUILD_ACTIONS[action]!] })) : [];
  steps.push(...[...tests].sort().map(path => ({ action: "contract-test", executor: "bun" as const, args: ["test", "--timeout", "60000", `./${path}`] })));
  return {
    domainOnly, rustRequired, selectedPackages: packages, steps, filterGuessing: false,
    scope: domainOnly ? "domain-only" : rustRequired ? "full-rust" : "noncompiler-only",
    compilerInputOwner, compilerInputOwnerSha256: artifactHash(ownerText),
    documentationPathCount, unknownPathCount, unreviewedPathIndices,
    reasons: [
      ...!normalized.length ? ["empty_paths_require_full_rust"] : [],
      ...domainOnly ? ["exact_domain_packages"] : compilerPaths.length ? ["reviewed_compiler_inputs"] : [],
      ...unknownPathCount ? ["unknown_paths_require_full_rust"] : [],
      ...documentationPathCount ? ["reviewed_documentation_or_evidence"] : [],
      ...tests.size ? ["reviewed_noncompiler_contracts"] : [],
    ],
    coverageGaps: [
      ...documentationPathCount ? ["documentation_and_evidence_contents_not_verified"] : [],
      ...unknownPathCount ? ["unknown_paths_have_no_reviewed_noncompiler_contract"] : [],
      ...quick && rustRequired ? ["quick_omits_rust_clippy_and_tests"] : [],
    ],
    verificationScope: "selected_checks_only",
  };
}

interface Options { positional: string[]; quick: boolean; [key: string]: string | string[] | boolean; }
function parseOptions(args: string[]): Options {
  const options: Options = { positional: [], quick: false };
  const values = new Set(["features", "profile", "filter", "artifact-out", "reference", "task", "id", "generation", "after-revision", "expect-revision", "timeout-ms", "lock", "expect", "bundle", "attestation", "input"]);
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    if (arg === "--") { options.positional.push(...args.slice(i + 1)); break; }
    if (arg === "--quick") options.quick = true;
    else if (arg.startsWith("--") && values.has(arg.slice(2))) {
      const value = args[++i]; if (!value || value.startsWith("--")) throw new Error("missing_option_value");
      options[arg.slice(2)] = value;
    } else if (arg.startsWith("-")) throw new Error(`unknown_build_option:${arg}`);
    else options.positional.push(arg);
  }
  return options;
}
function integer(value: unknown, fallback: number, max: number): number {
  if (value === undefined) return fallback;
  if (typeof value !== "string" || !/^\d+$/.test(value) || !Number.isSafeInteger(Number(value)) || Number(value) < 1 || Number(value) > max) throw new Error("invalid_bounded_integer");
  return Number(value);
}
function policy(root: string, args: readonly string[]) {
  const out = spawnSync("bash", [resolve(import.meta.dir, "../agentic/agent-cargo.sh"), ...args], { cwd: root, env: { ...process.env, SCRIPT_KIT_REPO_ROOT: root, SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_AGENT_POLICY_ONLY: "1" }, encoding: "utf8", timeout: 10_000 });
  return out.status === 0 ? JSON.parse(out.stdout) : { allowed: false, reason: out.stderr.trim(), exitCode: out.status };
}
function locks(root: string) {
  const parent = join(root, "target-agent/.locks");
  return (existsSync(parent) ? readdirSync(parent).filter(name => name.endsWith(".lock")).sort() : []).map(name => cacheLease("diagnose", join(parent, name)));
}

type BuildOperationDisposition = ArtifactVerificationError["disposition"] | "BLOCKED_RESOURCE_BUDGET";
const resourceFailureCodes = new Set(["resource_budget_exceeded", "resource_free_space_below_floor", "resource_observation_incomplete", "resource_policy_conflict"]);
const managedActionSchemas = {
  prune: {
    requiredOptions: ["--expect-revision", "--input"],
    inputSchema: {
      type: "object", additionalProperties: false, required: ["candidates"],
      properties: { candidates: { type: "array", items: {
        type: "object", required: ["kind", "id", "generation", "revision", "recordSha256", "directoryDevice", "directoryInode"],
        properties: {
          kind: { enum: RETENTION_CANDIDATE_KINDS }, id: { type: "string", minLength: 1 }, generation: { type: "string", minLength: 1 },
          revision: { type: "integer", minimum: 1 }, recordSha256: { type: "string", pattern: "^[a-f0-9]{64}$" },
          directoryDevice: { type: "integer", minimum: 0 }, directoryInode: { type: "integer", minimum: 1 },
        },
      } } },
    },
    selectionPolicy: "Exact selected candidates and explicitly selected descendants; current revision required",
  },
  "keep-set": {
    requiredOptions: ["--expect-revision", "--input"],
    inputSchema: {
      type: "object", additionalProperties: false, required: ["references"],
      properties: { references: { type: "array", items: {
        type: "object", additionalProperties: false, required: ["manifestPath", "manifestSha256"],
        properties: { manifestPath: { type: "string", pattern: "^target-agent/artifacts/[^/]+/manifest\\.json$" }, manifestSha256: { type: "string", pattern: "^[a-f0-9]{64}$" } },
      } } },
    },
    selectionPolicy: "Replace exact keep references at the observed keep-set revision; historical references need not be current compiler inputs",
  },
};

export interface BuildOperationResult extends Record<string, unknown> {
  status: "succeeded" | "failed";
  cleanup: OwnedCleanup;
  ownedProcesses: readonly OwnedProcessIdentity[];
  task?: TaskIdentity;
  recordPath?: string;
  artifact?: ArtifactReference;
  artifacts?: ArtifactReference[];
  resources?: BuildResourceReport;
  failureCode?: string;
  disposition?: BuildOperationDisposition;
}

/** An operation failure must retain every resource acquired before its cause. */
export class BuildOperationError extends Error {
  readonly code: string;
  readonly disposition: BuildOperationDisposition | undefined;
  readonly result: BuildOperationResult;
  readonly cleanup: OwnedCleanup;
  constructor(cause: unknown, result: BuildOperationResult) {
    super(cause instanceof Error ? cause.message : String(cause), { cause });
    this.name = "BuildOperationError";
    this.code = cause instanceof ArtifactVerificationError ? cause.code : cause instanceof Error ? cause.message : String(cause);
    this.disposition = cause instanceof ArtifactVerificationError ? cause.disposition : result.disposition;
    this.cleanup = result.cleanup;
    this.result = { ...result, status: "failed", failureCode: this.code, ...(this.disposition ? { disposition: this.disposition } : {}), error: this.message };
  }
}

function observedCleanup(value: unknown): OwnedCleanup | undefined {
  if (!value || typeof value !== "object") return;
  const candidate = value as OwnedCleanup;
  if (![candidate.resourcesAcquired, candidate.closed, candidate.processExited, candidate.processGroupExited, candidate.streamsDrained, candidate.logWriterClosed, candidate.referencesFinalized].every(flag => typeof flag === "boolean")
    || (candidate.ownedWindowsClosed !== null && typeof candidate.ownedWindowsClosed !== "boolean")
    || !Array.isArray(candidate.failureCodes) || !candidate.failureCodes.every(code => typeof code === "string")
    || !Array.isArray(candidate.survivors) || !candidate.survivors.every(survivor => survivor && typeof survivor.kind === "string" && typeof survivor.identity === "string" && ["present", "unknown"].includes(survivor.observation))) return;
  return { ...candidate, closed: candidate.closed && candidate.processExited && candidate.processGroupExited && candidate.streamsDrained && candidate.logWriterClosed && candidate.referencesFinalized && candidate.ownedWindowsClosed !== false && candidate.survivors.length === 0 };
}

function cleanupFromError(error: unknown): OwnedCleanup | undefined {
  if (error && typeof error === "object" && "cleanup" in error) return observedCleanup(error.cleanup);
}

function unfinishedCleanup(cleanup: OwnedCleanup, code: string, kind: string, identity: string): OwnedCleanup {
  return { ...cleanup, resourcesAcquired: true, closed: false, referencesFinalized: false,
    failureCodes: [...cleanup.failureCodes, code], survivors: [...cleanup.survivors, { kind, identity, observation: "unknown" }] };
}

async function closeBuildProcess(child: OwnedProcess, streams: readonly Promise<void>[], readers: readonly ReadableStreamDefaultReader<Uint8Array>[]): Promise<OwnedCleanup> {
  const closed = await boundedObservation(Promise.resolve().then(() => child.close()), 8_000);
  const observation = closed.completed ? observedCleanup(closed.value) : undefined;
  let cleanup: OwnedCleanup = observation ? { ...observation, resourcesAcquired: true } : {
    ...unknownOwnedCleanup(true), survivors: [
      { kind: "process", identity: canonicalJson(child.identity), observation: "unknown" as const },
      { kind: "process-group", identity: `${child.identity.processGroupId}:${child.identity.sessionGeneration}`, observation: "unknown" as const },
      { kind: "supervisor", identity: `${child.identity.supervisorPid}:${child.identity.supervisorStartTime}`, observation: "unknown" as const },
    ],
  };
  const drained = await boundedObservation(Promise.allSettled(streams), 1_000);
  if (!drained.completed || drained.value.some(result => result.status === "rejected")) {
    await boundedObservation(Promise.allSettled(readers.map(reader => Promise.resolve().then(() => reader.cancel()))), 500);
    cleanup = { ...cleanup, closed: false, streamsDrained: false, failureCodes: [...cleanup.failureCodes, "stream_drain_failed"] };
  }
  return cleanup;
}

export async function executeBuildAction(root: string, action: string, args: readonly string[], timeoutMs: number): Promise<BuildOperationResult> {
  const kind: ArtifactKind | undefined = action === "app-build" ? "application" : action === "libtest-build" ? "rust-libtest" : action === "exporter-build" ? "tool" : undefined;
  const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
  Object.assign(env, { SCRIPT_KIT_REPO_ROOT: root, SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_AGENT_TIMEOUT_MS: String(timeoutMs) });
  delete env.SCRIPT_KIT_AGENT_POLICY_ONLY; delete env.SCRIPT_KIT_AGENT_RESULT_PATH;
  if (kind) env.SCRIPT_KIT_AGENT_ARTIFACT_KIND = kind; else delete env.SCRIPT_KIT_AGENT_ARTIFACT_KIND;
  let output = "", child: OwnedProcess | undefined, failure: unknown;
  let failed = false, wrapperCleanup: OwnedCleanup | undefined;
  let result: BuildOperationResult = { status: "failed", cleanup: emptyOwnedCleanup(), ownedProcesses: [] };
  const readers: ReadableStreamDefaultReader<Uint8Array>[] = [], streams: Promise<void>[] = [];
  try {
    child = await spawnOwnedProcess({ argv: ["bash", resolve(import.meta.dir, "../agentic/agent-cargo.sh"), ...args], cwd: root, env, timeoutMs: timeoutMs + 30_000, maxOutputBytes: 80 * 1024 * 1024 });
    result.ownedProcesses = [child.identity];
    const consume = async (stream: ReadableStream<Uint8Array>, stdout: boolean) => {
      const reader = stream.getReader(); readers.push(reader); const decoder = new TextDecoder();
      try {
        while (true) {
          const next = await reader.read(); if (next.done) break;
          if (stdout) { output += decoder.decode(next.value, { stream: true }); if (output.length > 8 * 1024 * 1024) throw new Error("wrapper_result_limit"); }
          else process.stderr.write(next.value);
        }
      } finally { reader.releaseLock(); }
    };
    streams.push(consume(child.stdout, true), consume(child.stderr, false));
    await Promise.all(streams);
    const exitCode = await child.exited;
    let parsed: BuildOperationResult;
    try {
      parsed = JSON.parse(output.trim());
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("wrapper_result_shape");
    } catch (error) { throw new Error("wrapper_result_unavailable", { cause: error }); }
    wrapperCleanup = observedCleanup(parsed.cleanup);
    result = { ...parsed, status: exitCode === 0 && parsed.status === "succeeded" ? "succeeded" : "failed", cleanup: result.cleanup, ownedProcesses: [child.identity, ...(parsed.ownedProcesses ?? [])] };
    if (result.task || result.recordPath || wrapperCleanup?.resourcesAcquired && wrapperCleanup.referencesFinalized) {
      try {
        if (!result.task || !result.recordPath) throw new Error("wrapper_task_identity_missing");
        const registeredPath = managedTaskRecordPath(root, result.task);
        if (resolve(result.recordPath) !== resolve(registeredPath)) throw new Error("wrapper_task_record_path_changed");
        const record = readManagedTask(registeredPath, result.task);
        result.ownedProcesses = [...result.ownedProcesses, ...record.ownedProcesses];
        if (canonicalJson(record.identity) !== canonicalJson(result.task)) throw new Error("wrapper_task_revision_changed");
        if (wrapperCleanup?.referencesFinalized && (!record.cleanup.referencesFinalized || !["closed", "protected"].includes(record.state))) throw new Error("wrapper_task_references_unfinalized");
        if (wrapperCleanup?.closed && (record.state !== "closed" || observedCleanup(record.cleanup)?.closed !== true || canonicalJson(record.cleanup) !== canonicalJson(wrapperCleanup))) throw new Error("wrapper_task_cleanup_changed");
      } catch (error) {
        wrapperCleanup = unfinishedCleanup(wrapperCleanup ?? emptyOwnedCleanup(), "wrapper_task_finalization_unproved", "managed-task", canonicalJson(result.task ?? child.identity));
        result.status = "failed";
        result.failureCode ??= "wrapper_task_finalization_unproved";
        result.finalizationErrors = [...(Array.isArray(result.finalizationErrors) ? result.finalizationErrors : []), String(error)];
      }
    }
    if (result.status === "succeeded" && kind) {
      const reference = result.artifacts?.[0];
      if (!reference || result.artifacts!.length !== 1) throw new Error("wrapper_artifact_reference_missing");
      const artifact = verifyImmutableArtifact(root, reference, { kind, packageName: "script-kit-gpui", targetName: kind === "application" ? "script-kit-gpui" : kind === "rust-libtest" ? "script_kit_gpui" : "export_design_tokens", sourcePolicy: "current-content" });
      result.artifact = artifact.reference; result.binaryPath = artifact.manifest.binaryPath;
    }
  } catch (error) {
    failed = true; failure = error;
    result.cleanup = cleanupFromError(error) ?? result.cleanup;
  } finally {
    if (child) {
      result.cleanup = await closeBuildProcess(child, streams, readers);
      result.cleanup = wrapperCleanup ? aggregateCleanup([result.cleanup, wrapperCleanup])
        : unfinishedCleanup(result.cleanup, "wrapper_cleanup_unproved", "wrapper-task", canonicalJson(child.identity));
    }
  }
  if (failed) throw new BuildOperationError(failure, result);
  if (!result.cleanup.closed) result = { ...result, status: "failed", failureCode: result.failureCode ?? "wrapper_cleanup_unproved" };
  return result;
}

async function executeNoncompilerContract(root: string, args: readonly string[], timeoutMs: number): Promise<BuildOperationResult> {
  const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
  for (const key of ["SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER", "SCRIPT_KIT_ALLOW_NATIVE_INPUT", "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE", "SCRIPT_KIT_ALLOW_VISIBLE_PROBES", "SCRIPT_KIT_ALLOW_LIVE_AI", "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH"]) {
    if (env[key] === "1") throw new NoninteractiveSafetyError("contract-test", `${key}=1 contradicts noninteractive verification`);
    env[key] = "0";
  }
  Object.assign(env, { SCRIPT_KIT_REPO_ROOT: root, SCRIPT_KIT_NONINTERACTIVE: "1", PYTHONDONTWRITEBYTECODE: "1", SCRIPT_KIT_SEARCH_FULL_STRESS: "0", SCRIPT_KIT_STORAGE_FULL_STRESS: "0", NO_COLOR: "1", FORCE_COLOR: "0" });
  let child: OwnedProcess | undefined, failure: unknown, failed = false, output = "";
  let result: BuildOperationResult = { status: "failed", cleanup: emptyOwnedCleanup(), ownedProcesses: [] };
  const readers: ReadableStreamDefaultReader<Uint8Array>[] = [], streams: Promise<void>[] = [];
  try {
    child = await spawnOwnedProcess({ argv: [process.execPath, ...args], cwd: root, env, timeoutMs: Math.min(timeoutMs, 600_000), maxOutputBytes: 8 * 1024 * 1024 });
    result.ownedProcesses = [child.identity];
    const consume = async (stream: ReadableStream<Uint8Array>, summary: boolean) => {
      const reader = stream.getReader(); readers.push(reader); const decoder = new TextDecoder();
      try {
        while (true) {
          const next = await reader.read(); if (next.done) break;
          if (summary) { output += decoder.decode(next.value, { stream: true }); if (output.length > 8 * 1024 * 1024) throw new Error("contract_result_limit"); }
          process.stderr.write(next.value);
        }
      } finally { reader.releaseLock(); }
    };
    streams.push(consume(child.stdout, false), consume(child.stderr, true));
    await Promise.all(streams);
    const exitCode = await child.exited;
    const passedTests = Number([...output.matchAll(/^\s*(\d+) pass\s*$/gm)].at(-1)?.[1] ?? 0);
    const failedTests = Number([...output.matchAll(/^\s*(\d+) fail\s*$/gm)].at(-1)?.[1] ?? 0);
    result = { ...result, status: exitCode === 0 && passedTests > 0 && failedTests === 0 ? "succeeded" : "failed", exitCode, passedTests, failedTests,
      ...(exitCode !== 0 || failedTests ? { failureCode: "noncompiler_contract_failed" } : !passedTests ? { failureCode: "noncompiler_contract_zero_tests" } : {}) };
  } catch (error) {
    failed = true; failure = error; result.cleanup = cleanupFromError(error) ?? result.cleanup;
  } finally {
    if (child) result.cleanup = await closeBuildProcess(child, streams, readers);
  }
  if (failed) throw new BuildOperationError(failure, result);
  if (!result.cleanup.closed) result = { ...result, status: "failed", failureCode: result.failureCode ?? "contract_cleanup_unproved" };
  return result;
}

function annotateBuildResult(value: unknown, key = ""): unknown {
  if (inferredKindForKey(key) === "Secret") return secret(value);
  if (key === "recordSnapshot") return diagnostic(annotateBuildResult(value));
  if (key === "error" || key === "errors" || key === "targetAgentErrors" || key === "reason" || key === "finalizationErrors") return diagnostic(value);
  if (Array.isArray(value)) return value.map(entry => annotateBuildResult(entry));
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([field, entry]) => [field, annotateBuildResult(entry, field)]));
  return value;
}

export async function runBuildOps(argv: string[]): Promise<void> {
  const root = realpathSync(process.env.SCRIPT_KIT_REPO_ROOT || resolve(import.meta.dir, "../.."));
  const clock = startClock();
  const [verb = "discover", subject = "", ...rest] = argv;
  let result: Record<string, any> = {}, classification = "ok", cleanup = emptyOwnedCleanup();
  let identity = { kind: "build-workspace", id: artifactHash(root), generation: artifactHash(readFileSync(import.meta.path)), revision: 1 };
  const completed: BuildOperationResult[] = [];
  let changedRoute: ChangedRoute | undefined;
  const attemptedSteps: ChangedStep[] = [];
  const retain = (operation: BuildOperationResult): BuildOperationResult => {
    completed.push(operation); cleanup = aggregateCleanup([cleanup, operation.cleanup]);
    if (operation.task) identity = operation.task;
    return operation;
  };
  try {
    const options = parseOptions([...(subject.startsWith("--") ? [subject] : []), ...rest]);
    if (verb === "discover") result = { verbs: ["discover", "inspect", "query", "act", "wait", "diagnose"], actions: Object.fromEntries(Object.entries(BUILD_ACTIONS).map(([name, args]) => [name, productStatic(args)])), managedActions: productStatic(managedActionSchemas), limits: buildLimits(), pool: "agent-debug", applicationLaunchSupported: false, cacheDeletionSupported: false, policy: policy(root, BUILD_ACTIONS["app-build"]!) };
    else if (verb === "inspect") result = { configuration: compilerCompatibility(root), policy: policy(root, BUILD_ACTIONS[String(options.task || "app-build")] ?? []), dependencies: buildDependencies(root), limits: buildLimits(), performedInstallation: false, performedBuild: false };
    else if (verb === "diagnose") result = subject === "locks" ? { locks: locks(root) } : { dependencies: buildDependencies(root), locks: locks(root), tasks: listManagedTasks(root), disk: buildStorage(root) };
    else if (verb === "query") {
      if (subject === "storage") result = buildStorage(root);
      else if (subject === "retention") result = managedRetentionPlan(root);
      else if (subject === "keep-set") result = managedKeepSet(root);
      else if (subject === "route") {
        if (Object.keys(options).some(key => !["positional", "quick", "timeout-ms"].includes(key))) throw new Error("changed_action_owns_verification_options");
        result = { ...routeChanged(options.positional, options.quick, root), performedVerification: false };
      }
      else if (subject === "jobs") result = { jobs: listManagedTasks(root) };
      else if (subject === "job") {
        const expected = { id: String(options.id), generation: String(options.generation) };
        result = { task: readManagedTask(managedTaskRecordPath(root, expected), expected) };
      } else if (subject === "artifact") {
        const reference = readOwnedJson(resolve(String(options.reference))) as unknown as ArtifactReference;
        const kind = String(options.task || "application") as ArtifactKind;
        const artifact = verifyImmutableArtifact(root, reference, { kind, packageName: "script-kit-gpui", targetName: kind === "application" ? "script-kit-gpui" : kind === "tool" ? "export_design_tokens" : "script_kit_gpui", sourcePolicy: "current-content" });
        result = { artifact: artifact.reference, manifest: artifact.manifest };
      } else throw new Error("unknown_build_query");
    } else if (verb === "act") {
      if (subject === "prune" || subject === "keep-set") {
        if (typeof options.input !== "string" || typeof options["expect-revision"] !== "string") throw new Error("managed_action_requires_input_and_revision");
        const input = readOwnedJson(resolve(options.input));
        if (subject === "prune") {
          if (!Array.isArray(input.candidates) || Object.keys(input).some(key => key !== "candidates")) throw new Error("prune_requires_candidate_selection");
          result = pruneManagedRecords(root, options["expect-revision"], input.candidates as RetentionCandidate[]);
        } else {
          if (!Array.isArray(input.references) || Object.keys(input).some(key => key !== "references")) throw new Error("keep_set_requires_references");
          result = updateManagedKeepSet(root, options["expect-revision"], input.references as ArtifactReference[]);
        }
      }
      else if (subject === "recover-lock") {
        const name = String(options.lock);
        if (!/^[A-Za-z0-9._-]+\.lock$/.test(name)) throw new Error("invalid_exact_lock_name");
        result = cacheLease("recover", join(root, "target-agent/.locks", name), [canonicalJson(readOwnedJson(resolve(String(options.expect))))]);
      } else {
        const timeout = integer(options["timeout-ms"], 1_800_000, 7_170_000);
        if (subject === "changed") {
          // Route inspection and execution share exact argv; no hidden filter/config rewrites.
          if (Object.keys(options).some(key => !["positional", "quick", "timeout-ms"].includes(key))) throw new Error("changed_action_owns_verification_options");
          changedRoute = routeChanged(options.positional, options.quick, root);
        }
        const steps: ChangedStep[] = changedRoute?.steps ?? [{ action: subject, executor: "cargo", args: [...(BUILD_ACTIONS[subject] ?? [])] }];
        if (!changedRoute && !steps[0]!.args.length) throw new Error("unknown_build_action");
        result = { status: "succeeded" };
        for (const step of steps) {
          if (changedRoute) attemptedSteps.push(step);
          if (step.executor === "bun") {
            result = retain(await executeNoncompilerContract(root, step.args, timeout));
            if (result.status !== "succeeded") break;
            continue;
          }
          const args = [...step.args];
          const separator = args.indexOf("--") < 0 ? args.length : args.indexOf("--");
          const additions = [];
          if (subject === "publish-signed-bundle") {
            for (const name of ["input", "bundle", "attestation"]) {
              if (!options[name]) throw new Error(`signed_bundle_requires_${name}`);
              additions.push(`--${name}`, String(options[name]));
            }
          }
          if (options.features) additions.push("--features", String(options.features));
          if (options.profile) { if (args.includes("--profile")) throw new Error("action_owns_profile"); additions.push("--profile", String(options.profile)); }
          if (options.filter) { if (args[0] !== "test" || !/^[A-Za-z_][A-Za-z0-9_:]*$/.test(String(options.filter))) throw new Error("invalid_reviewed_test_filter"); additions.push(String(options.filter)); }
          args.splice(separator, 0, ...additions);
          if (step.action === "lib-test") {
            if (options.reference && (options.features || options.profile)) throw new Error("reference_owns_compilation_configuration");
            const compiled = options.reference ? null : retain(await executeBuildAction(root, "libtest-build", [...BUILD_ACTIONS["libtest-build"]!, ...(options.filter ? additions.slice(0, -1) : additions)], timeout));
            if (compiled && compiled.status !== "succeeded") result = compiled;
            else {
              const reference = options.reference ? readOwnedJson(resolve(String(options.reference))) as unknown as ArtifactReference : compiled!.artifact as ArtifactReference;
              result = retain(await executePublishedLibtests(root, reference, options.filter ? String(options.filter) : undefined, timeout));
            }
          } else {
            if (options.reference) throw new Error("only_libtest_execution_accepts_reference");
            result = retain(await executeBuildAction(root, step.action, args, timeout));
          }
          if (result.status !== "succeeded") break;
        }
        if (result.status !== "succeeded") {
          classification = "reproduced";
          if (resourceFailureCodes.has(result.failureCode)) result.disposition = "BLOCKED_RESOURCE_BUDGET";
        }
        if (options["artifact-out"] && result.artifact && result.status === "succeeded") {
          const path = resolve(String(options["artifact-out"]));
          validateOutputTarget({ repoRoot: root, candidate: path, kind: "receipt", probeId: "build-reference" });
          writeFileSync(path, `${canonicalJson(result.artifact)}\n`, { flag: "wx", mode: 0o600 });
        }
      }
    } else if (verb === "wait") {
      const expected = { id: String(options.id), generation: String(options.generation) };
      const timeout = integer(options["timeout-ms"], 10_000, 3_600_000), after = options["after-revision"] === "0" ? 0 : integer(options["after-revision"], 0, Number.MAX_SAFE_INTEGER);
      const deadline = performance.now() + timeout;
      while (true) {
        const record = readManagedTask(managedTaskRecordPath(root, expected), expected);
        if (record.identity.revision > after && ["closed", "protected"].includes(record.state)) {
          result = { task: record }; identity = record.identity; cleanup = record.cleanup;
          classification = record.state === "closed" && record.result.status === "succeeded" ? "ok" : "reproduced"; break;
        }
        if (performance.now() >= deadline) throw new Error("wait_deadline_expired");
        await Bun.sleep(50);
      }
    } else throw new Error("unknown_build_verb");
  } catch (error) {
    const message = String(error);
    if (error instanceof BuildOperationError) result = retain(error.result);
    else if (error instanceof BuildResourceError) result = { status: "failed", failureCode: error.code, disposition: "BLOCKED_RESOURCE_BUDGET", resources: { scope: "target-agent", measurement: "allocated-blocks", hardQuota: false, automaticEviction: false, checks: [error.observation], monitoring: null, refusal: error.observation } };
    else {
      const observed = cleanupFromError(error);
      if (observed) cleanup = aggregateCleanup([cleanup, observed]);
      result = { ...result, status: "failed", failureCode: error instanceof ArtifactVerificationError ? error.code : message, error: message };
    }
    const disposition = error instanceof BuildResourceError ? "BLOCKED_RESOURCE_BUDGET" : error instanceof ArtifactVerificationError || error instanceof BuildOperationError ? error.disposition : undefined;
    if (disposition) result.disposition = disposition;
    else classification = /stale|changed/.test(message) ? "blocked-by-stale-generation" : /lease|storage|pressure/.test(message) ? "blocked-by-resource-budget" : /timeout|deadline/.test(message) ? "blocked-by-timeout" : "blocked-by-invalid-binary";
  }
  if (completed.length) result = { ...result, cleanup, ownedProcesses: completed.flatMap(operation => operation.ownedProcesses), ...(completed.length > 1 || subject === "changed" ? { completed } : {}) };
  if (changedRoute) result = { ...result, route: productStatic(changedRoute), attemptedSteps: productStatic(attemptedSteps),
    notExecutedSteps: productStatic(changedRoute.steps.slice(attemptedSteps.length)), performedVerification: completed.some(operation => operation.ownedProcesses.length > 0),
    noRustDecision: !changedRoute.rustRequired, selectedChecksComplete: result.status === "succeeded" && attemptedSteps.length === changedRoute.steps.length };
  if (result.disposition) classification = result.disposition === "BLOCKED_RESOURCE_BUDGET" ? "blocked-by-resource-budget" : result.disposition === "BLOCKED_STALE_GENERATION" ? "blocked-by-stale-generation" : result.disposition === "BLOCKED_SCOPE_DRIFT" ? "blocked-by-scope-drift" : "blocked-by-invalid-binary";
  if (!cleanup.closed) classification = "invalid-cleanup";
  emitValidatedReceipt("devtools.build-ops", finishReceipt({ tool: "script-kit-devtools.build-ops", command: `build-ops.${verb}`, session: "build-ops", clock }, {
    classification, ...(!cleanup.closed ? { disposition: "INVALID_CLEANUP" } : result.disposition ? { disposition: result.disposition } : {}), evidenceClass: result.passedTests > 0 && result.status === "succeeded" ? "UNIT_BEHAVIOR" : "STATIC_INVENTORY", binary: null, transaction: null,
    buildOps: { schemaVersion: 1, snapshotId: randomUUID(), observedAt: new Date().toISOString(), identity, result: verb === "query" && subject === "route" && result.status !== "failed" ? productStatic(result) : annotateBuildResult(result) },
    artifact: result.artifact ?? null, cleanup,
    safety: { startsApplication: false, revealsWindow: false, focusesWindow: false, drivesNativeInput: false, capturesScreen: false, usesLiveAi: false },
  }));
}
if (import.meta.main) await runBuildOps(process.argv.slice(2));

export async function executePublishedLibtests(root: string, reference: ArtifactReference, filter: string | undefined, timeoutMs: number): Promise<BuildOperationResult> {
  if (filter && !/^[A-Za-z_][A-Za-z0-9_:]*$/.test(filter)) throw new Error("invalid_reviewed_test_filter");
  const artifact = verifyImmutableArtifact(root, reference, { kind: "rust-libtest", packageName: "script-kit-gpui", targetName: "script_kit_gpui", sourcePolicy: "current-content" });
  const id = `libtest-${randomUUID()}`;
  let claim: OutputClaim | undefined, task: ManagedTask | undefined, child: OwnedProcess | undefined;
  let cleanup = emptyOwnedCleanup(), passed = 0, failed = 0, summaries = 0, retained = 0;
  let result: BuildOperationResult = { status: "failed", artifact: reference, cleanup, ownedProcesses: [] };
  let log: number | undefined, failure: unknown, operationFailed = false;
  const finalizationErrors: string[] = [];
  const readers: ReadableStreamDefaultReader<Uint8Array>[] = [], streams: Promise<void>[] = [];
  try {
    claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/managed-tasks", id), kind: "directory", probeId: "published-libtest" }), id);
    cleanup = { ...cleanup, resourcesAcquired: true };
    result.outputClaim = { root: claim.root, owner: claim.owner };
    task = beginManagedTask(claim, "runtime-run", [reference]);
    result.task = task.identity; result.recordPath = task.recordPath;
    const env: Record<string, string> = { PATH: `${dirname(process.execPath)}:/usr/bin:/bin:/usr/sbin:/sbin`, LANG: "C.UTF-8", RUST_TEST_THREADS: "1", SCRIPT_KIT_NONINTERACTIVE: "1", RUST_BACKTRACE: "1" };
    for (const key of ["HOME", "SK_PATH", "CODEX_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "TMPDIR", "TMP", "TEMP"]) {
      env[key] = join(claim.root, key.toLowerCase()); mkdirSync(env[key]!, { mode: 0o700 });
    }
    for (const key of ["SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER", "SCRIPT_KIT_ALLOW_NATIVE_INPUT", "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE", "SCRIPT_KIT_ALLOW_VISIBLE_PROBES", "SCRIPT_KIT_ALLOW_LIVE_AI", "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH", "SCRIPT_KIT_SEARCH_FULL_STRESS", "SCRIPT_KIT_STORAGE_FULL_STRESS"]) env[key] = "0";
    log = openSync(join(claim.root, "libtest.log"), "wx", 0o600);
    child = await spawnOwnedProcess({ argv: [artifact.executablePath, ...(filter ? [filter] : []), "--test-threads=1"], cwd: root, env, timeoutMs, maxOutputBytes: 64 * 1024 * 1024 });
    result.ownedProcesses = [child.identity];
    updateManagedTask(task, { state: "running", source: artifact.manifest.source, effectiveConfiguration: artifact.manifest.effectiveConfiguration, ownedProcesses: result.ownedProcesses.slice() });
    const consume = async (stream: ReadableStream<Uint8Array>) => {
      const reader = stream.getReader(); readers.push(reader); const decoder = new TextDecoder(); let pending = "";
      try {
        while (true) {
          const next = await reader.read(); if (next.done) break;
          const bytes = next.value.subarray(0, Math.max(0, 4 * 1024 * 1024 - retained));
          if (bytes.length) { writeFileSync(log!, bytes); retained += bytes.length; }
          process.stderr.write(next.value);
          pending += decoder.decode(next.value, { stream: true });
          if (pending.length > 8 * 1024 * 1024) throw new Error("libtest_line_limit");
          let newline: number;
          while ((newline = pending.indexOf("\n")) >= 0) {
            const line = pending.slice(0, newline); pending = pending.slice(newline + 1);
            const summary = /test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;/.exec(line);
            if (summary) { summaries++; passed += Number(summary[1]); failed += Number(summary[2]); }
          }
        }
      } finally { reader.releaseLock(); }
    };
    child.stdin.end();
    streams.push(consume(child.stdout), consume(child.stderr));
    await Promise.all(streams);
    const exitCode = await child.exited;
    result = { ...result, status: exitCode === 0 && summaries > 0 && passed > 0 && failed === 0 ? "succeeded" : "failed", exitCode, binaryPath: artifact.manifest.binaryPath };
  } catch (error) {
    operationFailed = true; failure = error;
    const observed = cleanupFromError(error);
    if (observed) cleanup = aggregateCleanup([cleanup, observed]);
  } finally {
    if (child) cleanup = await closeBuildProcess(child, streams, readers);
    if (log !== undefined) {
      try { closeSync(log); }
      catch (error) {
        cleanup = { ...cleanup, resourcesAcquired: true, closed: false, logWriterClosed: false,
          failureCodes: [...cleanup.failureCodes, "log_close_failed"], survivors: [...cleanup.survivors, { kind: "log-writer", identity: join(claim!.root, "libtest.log"), observation: "unknown" }] };
        finalizationErrors.push(String(error));
      }
    }
    result = { ...result, cleanup, passedTests: passed, failedTests: failed, testSummaries: summaries, retainedLogBytes: retained };
    if (operationFailed) result = new BuildOperationError(failure, result).result;
    if (!cleanup.closed) result.status = "failed";
    if (task) {
      try { result.task = updateManagedTask(task, { state: "finalizing", result }).identity; }
      catch (error) {
        cleanup = unfinishedCleanup(cleanup, "task_result_finalization_failed", "managed-task", canonicalJson(task.identity));
        finalizationErrors.push(String(error));
      }
      // Result persistence must not prevent the independent reference/terminal-state finalizer.
      try {
        const finalized = finalizeManagedTask(task, cleanup);
        cleanup = finalized.cleanup; result.task = finalized.identity;
      } catch (error) {
        cleanup = unfinishedCleanup(cleanup, "task_finalization_failed", "managed-task", canonicalJson(task.identity));
        finalizationErrors.push(String(error));
      }
    } else if (claim) {
      cleanup = unfinishedCleanup(cleanup, "task_acquisition_unproved", "output-claim", canonicalJson(claim.owner));
    }
    result = { ...result, cleanup, ...(finalizationErrors.length ? { finalizationErrors } : {}) };
  }
  if (finalizationErrors.length && !operationFailed) { operationFailed = true; failure = new Error("libtest_finalization_failed"); }
  if (operationFailed) throw new BuildOperationError(failure, result);
  if (!cleanup.closed) result.status = "failed";
  return result;
}
