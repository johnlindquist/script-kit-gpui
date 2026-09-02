import { accessSync, constants, existsSync, lstatSync, opendirSync, readFileSync, statfsSync } from "node:fs";
import type { Stats } from "node:fs";
import * as fsPromises from "node:fs/promises";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import * as workerThreads from "node:worker_threads";

export interface AllocationObservation {
  present: boolean; complete: boolean; logicalBytes: number; allocatedBytes: number;
  entries: number; linksNotFollowed: number; errors: string[];
}
export function allocation(path: string, excluded: readonly string[] = [], seen = new Set<string>()): AllocationObservation {
  return scanAllocation(path, excluded, seen);
}

const ALLOCATION_ENTRY_LIMIT = 250_000;
const ALLOCATION_TIMEOUT_MS = 8_000;

function protectedOutputRoot(child: string): boolean {
  if (!child) return true;
  const parts = child.split("/");
  if (["pools", "agents"].includes(parts[0]!)) return parts.length <= 4;
  return ["shared", "artifacts", "runtime", ".locks"].includes(parts[0]!) && parts.length <= 2;
}

function scanAllocation(path: string, excluded: readonly string[], seen: Set<string>, expectedDevice?: number, protectOutputs = false): AllocationObservation {
  const result: AllocationObservation = { present: false, complete: true, logicalBytes: 0, allocatedBytes: 0, entries: 0, linksNotFollowed: 0, errors: [] };
  const started = performance.now();
  let device = expectedDevice;
  const visit = (current: string, child: string, depth: number): void => {
    if (excluded.some(value => child === value || child.startsWith(value + "/"))) return;
    if (++result.entries > ALLOCATION_ENTRY_LIMIT) throw new Error("inventory_entry_limit");
    if (depth > 256) throw new Error("inventory_depth_limit");
    if (performance.now() - started > ALLOCATION_TIMEOUT_MS) throw new Error("inventory_time_limit");
    let stat: Stats;
    try { stat = lstatSync(current); }
    catch (error) {
      // Only a missing scan root is an absent tree. Disappearing children are incomplete observations.
      if (!child && (error as NodeJS.ErrnoException).code === "ENOENT") { result.entries = 0; return; }
      throw error;
    }
    if (!child) result.present = true;
    device ??= Number(stat.dev);
    if (Number(stat.dev) !== device) throw new Error("inventory_unexpected_mount");
    const identity = `${stat.dev}:${stat.ino}`;
    if (!seen.has(identity)) {
      seen.add(identity);
      const bytes = Number(stat.blocks) * 512;
      if (!Number.isSafeInteger(bytes) || bytes < 0 || !Number.isSafeInteger(result.allocatedBytes + bytes)) throw new Error("inventory_invalid_allocation");
      result.allocatedBytes += bytes;
      if (!stat.isDirectory()) result.logicalBytes += Number(stat.size);
    }
    if (stat.isSymbolicLink()) {
      result.linksNotFollowed++;
      if (protectOutputs && protectedOutputRoot(child)) throw new Error("inventory_output_symlink");
      return;
    }
    if (protectOutputs && !child && !stat.isDirectory()) throw new Error("inventory_output_not_directory");
    if (!stat.isDirectory()) return;
    const directory = opendirSync(current);
    try {
      let entry;
      while ((entry = directory.readSync()) !== null) visit(join(current, entry.name), child ? `${child}/${entry.name}` : entry.name, depth + 1);
      const after = lstatSync(current);
      if (!after.isDirectory() || after.dev !== stat.dev || after.ino !== stat.ino) throw new Error("inventory_directory_changed");
    } finally { directory.closeSync(); }
  };
  try { visit(path, "", 0); }
  catch (error) { result.complete = false; result.errors.push(String(error)); }
  return result;
}

function targetAgentAllocation(root: string): AllocationObservation {
  try {
    const stat = lstatSync(root);
    if (stat.isSymbolicLink() || !stat.isDirectory()) throw new Error("inventory_repository_not_directory");
    return scanAllocation(join(root, "target-agent"), [], new Set(), Number(stat.dev), true);
  } catch (error) {
    return { present: false, complete: false, logicalBytes: 0, allocatedBytes: 0, entries: 0, linksNotFollowed: 0, errors: [String(error)] };
  }
}
function positiveLimit(name: string, fallback: number): number {
  const value = process.env[name];
  if (value === undefined) return fallback;
  if (!/^\d+$/.test(value) || !Number.isSafeInteger(Number(value))) throw new Error(`invalid_${name}`);
  return Number(value);
}
function byteLimit(name: string, fallback: number): number {
  const bytes = positiveLimit(name, fallback) * 1024 ** 3;
  if (!Number.isSafeInteger(bytes)) throw new Error(`invalid_${name}`);
  return bytes;
}
export function buildLimits() {
  return {
    targetAgentBudgetBytes: byteLimit("SCRIPT_KIT_AGENT_TARGET_BUDGET_GB", 40),
    minimumFreeBytes: byteLimit("SCRIPT_KIT_AGENT_MIN_FREE_GB", 25),
    compilerWorkers: 2, nativeWorkersWithinEachCompiler: 1, testWorkers: 2,
    logBytesPerTask: 4 * 1024 ** 2,
    budgetScope: "all target-agent entries; dev targets, external caches and evidence outside target-agent reported separately",
    hardQuota: false as const, automaticEviction: false as const,
    deletion: "only exact finalized managed task/artifact candidates; never automatic cache eviction",
  };
}
export function buildStorage(root: string, external = true, budgetOnly = false) {
  // Budget authority must never share diagnostic inode state: a link in target/ cannot hide budgeted bytes.
  const targetAgent = targetAgentAllocation(root);
  const seen = new Set<string>(), poolSeen = new Set<string>();
  const categories = [
    ["dev-target", "target", ["pi-sidecar"], false],
    ["agent-pools", "target-agent/pools", [], true],
    ["legacy-exclusive-targets", "target-agent/agents", [], true],
    ["shared-caches", "target-agent/shared", [], false],
    ["immutable-artifacts", "target-agent/artifacts", [], false],
    ["runtime", "target-agent/runtime", [], false],
    ["evidence-and-tasks", ".test-output", [], false],
    ["legacy-evidence", ".artifacts", [], false],
    ["hidden-screenshots", ".test-screenshots", [], false],
    ["screenshots", "test-screenshots", [], false],
    ["leases", "target-agent/.locks", [], false],
    ["other-agent-storage", "target-agent", ["pools", "agents", "shared", "artifacts", "runtime", ".locks"], false],
  ] as const;
  const observations = categories.filter(([, child]) => !budgetOnly || child.startsWith("target-agent")).map(([category, child, exclude, poolBudgeted]) => ({ category, path: child, poolBudgeted,
    deletionScope: ["immutable-artifacts", "evidence-and-tasks"].includes(category) ? "finalized-managed-records-only" : "none",
    ...(!targetAgent.complete && child.startsWith("target-agent")
      ? { present: targetAgent.present, complete: false, logicalBytes: 0, allocatedBytes: 0, entries: 0, linksNotFollowed: 0, errors: [...targetAgent.errors] }
      : allocation(join(root, child), exclude, poolBudgeted ? poolSeen : seen)) }));
  const cargoHome = process.env.CARGO_HOME || join(homedir(), ".cargo");
  const kitHome = process.env.SK_PATH || join(homedir(), ".scriptkit");
  const candidates = [["cargo-registry", join(cargoHome, "registry")], ["cargo-git", join(cargoHome, "git")],
    ["toolchains", process.env.RUSTUP_HOME || join(homedir(), ".rustup")], ["pi-sidecar", join(root, "target/pi-sidecar")],
    ["models-candidate", join(kitHome, "models")], ["tools-candidate", join(kitHome, "bin")]];
  if (process.env.SCCACHE_DIR) candidates.push(["configured-sccache", resolve(process.env.SCCACHE_DIR)]);
  const externalObservations = external ? candidates.map(([category, path]) => ({ category, path, deletionScope: "never", ...allocation(path!) })) : [];
  let volume: { complete: boolean; availableBytes: number | null; reason?: string };
  try {
    const value = statfsSync(root), availableBytes = value.bavail * value.bsize;
    if (!Number.isSafeInteger(availableBytes) || availableBytes < 0) throw new Error("inventory_invalid_available");
    volume = { complete: true, availableBytes };
  }
  catch (error) { volume = { complete: false, availableBytes: null, reason: String(error) }; }
  return { categories: observations, external: externalObservations, volume,
    targetAgentAllocatedBytes: targetAgent.allocatedBytes, targetAgentComplete: targetAgent.complete, targetAgentErrors: targetAgent.errors,
    agentPoolAllocatedBytes: observations.filter(value => value.poolBudgeted).reduce((sum, value) => sum + value.allocatedBytes, 0),
    uniquePhysicalBytes: null, reclaimablePhysicalBytes: null, hardQuota: false as const, automaticEviction: false as const,
    measurement: "allocated blocks; independent target-agent budget and pool subtotal inode sets; no symlink traversal; APFS clone sharing unknown; diagnostic and external totals not additive" };
}
function executable(path: string | null): boolean {
  if (!path) return false;
  try { accessSync(path, constants.X_OK); return true; } catch { return false; }
}
export function buildDependencies(root: string) {
  const kitHome = process.env.SK_PATH || join(homedir(), ".scriptkit");
  const explicit = process.env.SCRIPT_KIT_PI_BINARY;
  const candidates = explicit ? [explicit.replace(/^~(?=\/|$)/, homedir())] : [join(root, "target/pi-sidecar/pi"), join(homedir(), "dev/pi_agent_rust/target/release/pi"), join(homedir(), "dev/pi_agent_rust/target/debug/pi")];
  const lock = existsSync(join(root, "Cargo.lock")) ? Bun.TOML.parse(readFileSync(join(root, "Cargo.lock"), "utf8")) as { package?: Array<{ name: string; version: string }> } : {};
  return {
    tools: ["bun", "python3", "git", "rustup", "cargo-watch", "sccache", "clang", "cmake", "xcrun", "cargo-bundle"].map(name => {
      const path = Bun.which(name); return { name, path, executablePresent: executable(path), health: "not-executed", version: null };
    }),
    pi: { candidates: candidates.map(path => ({ path, executablePresent: executable(path) })), health: "not-probed", owner: "scripts/prepare-pi-sidecar.sh" },
    models: [join(kitHome, "models/whisper-medium-q4_1.bin"), join(kitHome, "models/parakeet-tdt-0.6b-v3-int8")].map(path => ({ path, present: existsSync(path), validity: "not-loaded" })),
    lockedLibraries: (lock.package ?? []).filter(value => ["transcribe-rs", "whisper-rs-sys", "ort-sys", "llama-cpp-sys-2"].includes(value.name)).map(({ name, version }) => ({ name, version })),
    metal: { owner: "vendor/gpui_macos/build.rs", requiredTools: ["xcrun metal", "xcrun metallib"], nativeBuild: "not-executed" },
    nativeRuntimeProof: "not-evaluated", installsTools: false, fetchesModels: false, startsSidecars: false,
  };
}
export interface BuildResourceObservation {
  phase: string;
  complete: boolean;
  targetAgentAllocatedBytes: number;
  availableBytes: number | null;
  reserveBytes: number;
  targetAgentBudgetBytes: number;
  minimumFreeBytes: number;
  withinLimits: boolean;
  failureCodes: string[];
  errors?: string[];
  scope: "target-agent";
  measurement: "allocated-blocks";
  hardQuota: false;
  automaticEviction: false;
}

export interface BuildResourceSummary {
  sampleCount: number;
  maximumSampledAllocatedBytes: number | null;
  minimumSampledAvailableBytes: number | null;
  complete: boolean;
  trigger: BuildResourceObservation | null;
  transientFailureCount?: number;
  workerClosed?: boolean;
  workerThreadId?: number;
  callbackFailed?: boolean;
  initialObservationIncluded?: boolean;
}

export interface BuildResourceReport {
  scope: "target-agent";
  measurement: "allocated-blocks";
  hardQuota: false;
  automaticEviction: false;
  checks: BuildResourceObservation[];
  monitoring: BuildResourceSummary | null;
  refusal: BuildResourceObservation | null;
}

export class BuildResourceError extends Error {
  readonly exitCode = 75;
  constructor(readonly code: string, readonly observation: BuildResourceObservation) {
    super(`${code}: ${observation.phase}`);
    this.name = "BuildResourceError";
  }
}

interface ResourcePolicy {
  targetAgentBudgetBytes: number;
  minimumFreeBytes: number;
  conflict: boolean;
}
function resourcePolicy(): ResourcePolicy {
  try { return { ...buildLimits(), conflict: process.env.SCRIPT_KIT_AGENT_ALLOW_LOW_DISK === "1" }; }
  catch { return { targetAgentBudgetBytes: 0, minimumFreeBytes: 0, conflict: true }; }
}

function resourceObservation(policy: ResourcePolicy, allocated: (Pick<AllocationObservation, "complete" | "allocatedBytes"> & Partial<Pick<AllocationObservation, "errors">>) | null, available: number | null, phase: string, reserveBytes = 0): BuildResourceObservation {
  const validReserve = Number.isSafeInteger(reserveBytes) && reserveBytes >= 0;
  const validAvailable = available !== null && Number.isSafeInteger(available) && available >= 0;
  const failureCodes: string[] = [];
  if (policy.conflict || !validReserve) failureCodes.push("resource_policy_conflict");
  const reserve = validReserve ? reserveBytes : 0;
  if (allocated && allocated.allocatedBytes > policy.targetAgentBudgetBytes - reserve) failureCodes.push("resource_budget_exceeded");
  if (validAvailable && available - reserve < policy.minimumFreeBytes) failureCodes.push("resource_free_space_below_floor");
  const complete = allocated?.complete === true && validAvailable;
  if (!complete) failureCodes.push("resource_observation_incomplete");
  return { phase, complete, targetAgentAllocatedBytes: allocated?.allocatedBytes ?? 0,
    availableBytes: validAvailable ? available : null, reserveBytes: reserve,
    targetAgentBudgetBytes: policy.targetAgentBudgetBytes, minimumFreeBytes: policy.minimumFreeBytes,
    withinLimits: failureCodes.length === 0, failureCodes, errors: allocated?.errors,
    scope: "target-agent", measurement: "allocated-blocks", hardQuota: false, automaticEviction: false };
}

export function requireBuildAdmission(root: string, options: { phase?: string; reserveBytes?: number } = {}): BuildResourceObservation {
  const policy = resourcePolicy(), allocated = targetAgentAllocation(root);
  let available: number | null = null;
  try { const value = statfsSync(root); available = value.bavail * value.bsize; } catch { /* Incomplete observations refuse below. */ }
  const observation = resourceObservation(policy, allocated, available, options.phase ?? "preflight", options.reserveBytes ?? 0);
  if (!observation.withinLimits) throw new BuildResourceError(observation.failureCodes[0]!, observation);
  return observation;
}

const ALLOCATION_WORKER = "build-resource-allocation";
// The request owns this worker and its only protocol. No process-wide watcher or persisted sample log.
if (!workerThreads.isMainThread && workerThreads.workerData?.role === ALLOCATION_WORKER) {
  workerThreads.parentPort!.on("message", (id: number) => {
    workerThreads.parentPort!.postMessage({ id, allocation: targetAgentAllocation(workerThreads.workerData.root) });
  });
}

export function startBuildResourceGuard(root: string, onRefusal: (error: BuildResourceError) => void, initialObservation?: BuildResourceObservation): { stop(): Promise<BuildResourceSummary> } {
  const policy = resourcePolicy();
  const validInitial = initialObservation != null && initialObservation.complete === true && initialObservation.withinLimits === true
    && initialObservation.reserveBytes === 0 && initialObservation.targetAgentBudgetBytes === policy.targetAgentBudgetBytes
    && initialObservation.minimumFreeBytes === policy.minimumFreeBytes && initialObservation.scope === "target-agent"
    && initialObservation.measurement === "allocated-blocks" && initialObservation.hardQuota === false && initialObservation.automaticEviction === false
    && Array.isArray(initialObservation.failureCodes) && initialObservation.failureCodes.length === 0
    && Number.isSafeInteger(initialObservation.targetAgentAllocatedBytes) && initialObservation.targetAgentAllocatedBytes >= 0
    && resourceObservation(policy, { complete: true, allocatedBytes: initialObservation.targetAgentAllocatedBytes }, initialObservation.availableBytes, initialObservation.phase).withinLimits;
  if (initialObservation !== undefined && !validInitial) policy.conflict = true;
  // This is the caller's exact lease-owned preflight, not a synthetic successful worker sample.
  const initial = validInitial ? { complete: true, allocatedBytes: initialObservation!.targetAgentAllocatedBytes } : null;
  const summary: BuildResourceSummary = { sampleCount: initial ? 1 : 0, maximumSampledAllocatedBytes: initial?.allocatedBytes ?? null,
    minimumSampledAvailableBytes: initial ? initialObservation!.availableBytes : null,
    complete: true, trigger: null, transientFailureCount: 0, workerClosed: true, callbackFailed: false, initialObservationIncluded: initial !== null };
  const timers = new Set<NodeJS.Timeout>();
  let stopped = false, stopPromise: Promise<BuildResourceSummary> | undefined;
  let worker: workerThreads.Worker | undefined, workerExited = false;
  let allocated: AllocationObservation | null = null, available: number | null = initial ? initialObservation!.availableBytes : null;
  let scanId = 0, pendingScan = 0, scanStarted = 0, scanFailures = 0, freeFailures = 0;
  let scanDeadline: NodeJS.Timeout | undefined;

  const cancel = (timer: NodeJS.Timeout | undefined): void => {
    if (timer !== undefined) { clearTimeout(timer); timers.delete(timer); }
  };
  const later = (fn: () => void, ms: number): NodeJS.Timeout => {
    const timer = setTimeout(() => { timers.delete(timer); if (!stopped) fn(); }, ms);
    timers.add(timer); return timer;
  };
  const stop = (): Promise<BuildResourceSummary> => {
    if (stopPromise) return stopPromise;
    stopped = true;
    for (const timer of timers) clearTimeout(timer);
    timers.clear();
    if (!(allocated ?? initial)?.complete || available === null) summary.complete = false;
    const owned = worker;
    stopPromise = (async () => {
      if (owned && !workerExited) {
        const timedOut = Promise.withResolvers<false>();
        const deadline = setTimeout(() => timedOut.resolve(false), 1_000);
        try {
          summary.workerClosed = await Promise.race([
            Promise.resolve(owned.terminate()).then(() => true, () => false),
            timedOut.promise,
          ]);
        } catch { summary.workerClosed = false; }
        finally { clearTimeout(deadline); }
      }
      if (owned) {
        owned.removeAllListeners();
        // A failed termination is reported, never represented as observed closure.
        if (!summary.workerClosed) { owned.on("error", () => {}); owned.unref(); }
      }
      if (!summary.workerClosed) summary.complete = false;
      return summary;
    })();
    return stopPromise;
  };
  const refuse = (observation: BuildResourceObservation): void => {
    if (stopped || summary.trigger) return;
    summary.complete = false; summary.trigger = observation;
    // Stop scheduling before the owner's close callback, which may itself call stop().
    const closing = stop();
    try { onRefusal(new BuildResourceError(observation.failureCodes[0]!, observation)); }
    catch { summary.callbackFailed = true; }
    void closing;
  };
  const check = (forceIncomplete = false): void => {
    const observation = resourceObservation(policy, allocated ?? initial, available, "in-build");
    // While the initial two independent samples are pending, known breaches still refuse immediately.
    if (forceIncomplete || observation.failureCodes.some(code => code !== "resource_observation_incomplete")) refuse(observation);
  };
  const transient = (errors: readonly string[]): boolean => errors.length > 0 && errors.every(error => /\b(ENOENT|ESTALE|EAGAIN|EBUSY)\b|inventory_directory_changed/.test(error));
  const failedAllocation = (errors: string[]): void => {
    allocated = { present: false, complete: false, logicalBytes: 0, allocatedBytes: 0, entries: 0, linksNotFollowed: 0, errors };
    summary.sampleCount++;
    check(true);
  };
  const scan = (): void => {
    if (stopped || pendingScan || !worker) return;
    pendingScan = ++scanId; scanStarted = performance.now();
    scanDeadline = later(() => { pendingScan = 0; failedAllocation(["inventory_time_limit"]); }, ALLOCATION_TIMEOUT_MS + 1_000);
    try { worker.postMessage(pendingScan); }
    catch (error) { cancel(scanDeadline); pendingScan = 0; failedAllocation([String(error)]); }
  };
  const sampleFree = (): void => {
    if (stopped) return;
    const started = performance.now();
    let settled = false;
    const deadline = later(() => {
      settled = true; available = null; summary.sampleCount++; check(true);
    }, 2_000);
    const failed = (error: unknown): void => {
      if (stopped || settled) return;
      settled = true; cancel(deadline); available = null; summary.sampleCount++;
      if (transient([String(error)]) && freeFailures++ === 0) {
        summary.transientFailureCount = (summary.transientFailureCount ?? 0) + 1; later(sampleFree, 250);
      } else check(true);
    };
    try {
      void fsPromises.statfs(root).then(value => {
        if (stopped || settled) return;
        const bytes = value.bavail * value.bsize;
        if (!Number.isSafeInteger(bytes) || bytes < 0) { failed(new Error("inventory_invalid_available")); return; }
        settled = true; cancel(deadline); freeFailures = 0; available = bytes; summary.sampleCount++;
        summary.minimumSampledAvailableBytes = Math.min(summary.minimumSampledAvailableBytes ?? bytes, bytes);
        check();
        if (!stopped) later(sampleFree, Math.max(0, 1_000 - (performance.now() - started)));
      }, failed);
    } catch (error) { failed(error); }
  };

  if (policy.conflict) {
    queueMicrotask(() => { if (!stopped) check(true); });
  } else {
    try {
      worker = new workerThreads.Worker(new URL(import.meta.url), { workerData: { role: ALLOCATION_WORKER, root: resolve(root) } });
      summary.workerClosed = false;
      const captureThreadId = (): void => {
        const threadId = worker!.threadId;
        if (Number.isSafeInteger(threadId) && threadId > 0) summary.workerThreadId = threadId;
      };
      captureThreadId();
      worker.once("online", captureThreadId);
      worker.on("message", (message: { id?: number; allocation?: AllocationObservation }) => {
        if (stopped) return;
        if (!pendingScan || message?.id !== pendingScan) { failedAllocation(["inventory_worker_protocol"]); return; }
        cancel(scanDeadline); pendingScan = 0;
        const sample = message.allocation;
        if (!sample || typeof sample.complete !== "boolean" || !Array.isArray(sample.errors)
          || !sample.errors.every(error => typeof error === "string")
          || !Number.isSafeInteger(sample.allocatedBytes) || sample.allocatedBytes < 0) {
          failedAllocation(["inventory_worker_protocol"]); return;
        }
        allocated = sample; summary.sampleCount++;
        summary.maximumSampledAllocatedBytes = Math.max(summary.maximumSampledAllocatedBytes ?? sample.allocatedBytes, sample.allocatedBytes);
        check();
        if (stopped) return;
        if (!sample.complete && transient(sample.errors) && scanFailures++ === 0) {
          summary.transientFailureCount = (summary.transientFailureCount ?? 0) + 1; later(scan, 250); return;
        }
        if (!sample.complete) { check(true); return; }
        scanFailures = 0;
        if (!stopped) later(scan, Math.max(0, 10_000 - (performance.now() - scanStarted)));
      });
      worker.on("error", error => { if (!stopped) failedAllocation([String(error)]); });
      worker.on("exit", () => {
        workerExited = true; summary.workerClosed = true;
        if (!stopped) failedAllocation(["inventory_worker_exited"]);
      });
      scan(); sampleFree();
    } catch (error) {
      queueMicrotask(() => { if (!stopped) failedAllocation([String(error)]); });
    }
  }
  return { stop };
}
if (workerThreads.isMainThread && import.meta.main && process.argv[2] === "admission") {
  try { requireBuildAdmission(resolve(process.argv[3]!)); } catch (error) { console.error(String(error)); process.exitCode = 75; }
}
