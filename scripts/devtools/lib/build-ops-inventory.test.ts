import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import * as fs from "node:fs";
import * as fsPromises from "node:fs/promises";
import { EventEmitter } from "node:events";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import * as workerThreads from "node:worker_threads";
import { allocation, buildLimits, buildStorage, BuildResourceError, requireBuildAdmission, startBuildResourceGuard } from "./build-ops-inventory.ts";
import type { AllocationObservation, BuildResourceObservation, BuildResourceSummary } from "./build-ops-inventory.ts";

// Bun spies invoke constructor bodies as functions; this view preserves the actual module property.
interface WorkerMockBoundary {
  Worker(filename: string | URL, options?: workerThreads.WorkerOptions): workerThreads.Worker;
}
interface NumberStatfsBoundary {
  statfs(path: fs.PathLike, options?: fs.StatFsOptions & { bigint?: false }): Promise<fs.StatsFs>;
}
const workerMockBoundary = workerThreads as unknown as WorkerMockBoundary;
// The resource guard exercises only the numeric statfs overload, never bigint observations.
const numberStatfsBoundary: NumberStatfsBoundary = fsPromises;

const GiB = 1024 ** 3;
const roots: string[] = [], restore: Array<() => void> = [];
const environment = ["SCRIPT_KIT_AGENT_TARGET_BUDGET_GB", "SCRIPT_KIT_AGENT_MIN_FREE_GB", "SCRIPT_KIT_AGENT_ALLOW_LOW_DISK", "SCRIPT_KIT_NONINTERACTIVE", "CARGO_HOME", "RUSTUP_HOME", "SK_PATH", "SCCACHE_DIR"];
beforeEach(() => {
  const saved = environment.map(name => [name, process.env[name]] as const);
  restore.push(() => { for (const [name, value] of saved) { if (value === undefined) delete process.env[name]; else process.env[name] = value; } });
  process.env.SCRIPT_KIT_AGENT_TARGET_BUDGET_GB = "1";
  process.env.SCRIPT_KIT_AGENT_MIN_FREE_GB = "0";
  delete process.env.SCRIPT_KIT_AGENT_ALLOW_LOW_DISK;
});
afterEach(() => {
  for (const undo of restore.splice(0).reverse()) undo();
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});
function fixture(): string {
  const root = fs.realpathSync(fs.mkdtempSync(join(tmpdir(), "build-resource-behavior-")));
  roots.push(root); return root;
}
function file(root: string, child: string): string {
  const path = join(root, child);
  fs.mkdirSync(dirname(path), { recursive: true }); fs.writeFileSync(path, Buffer.alloc(8192, 97)); return path;
}
function volume(bytes: number) { return { bavail: bytes, bsize: 1 } as fs.StatsFs; }
function observedVolume(bytes: number): void {
  const mock = spyOn(fs, "statfsSync").mockReturnValue(volume(bytes)); restore.push(() => mock.mockRestore());
}
function refusal(fn: () => unknown): BuildResourceError {
  try { fn(); } catch (error) { expect(error).toBeInstanceOf(BuildResourceError); return error as BuildResourceError; }
  throw new Error("expected resource refusal");
}
function sample(allocatedBytes: number, errors: string[] = []): AllocationObservation {
  return { present: true, complete: errors.length === 0, logicalBytes: allocatedBytes, allocatedBytes, entries: 1, linksNotFollowed: 0, errors };
}

test("defaults describe the complete target-agent policy without raising existing limits", () => {
  delete process.env.SCRIPT_KIT_AGENT_TARGET_BUDGET_GB; delete process.env.SCRIPT_KIT_AGENT_MIN_FREE_GB;
  expect(buildLimits()).toMatchObject({ targetAgentBudgetBytes: 40 * GiB, minimumFreeBytes: 25 * GiB, compilerWorkers: 2, nativeWorkersWithinEachCompiler: 1, testWorkers: 2, hardQuota: false, automaticEviction: false });
  expect(buildLimits()).not.toHaveProperty("poolBudgetBytes");
});

test("complete scope includes exports, pending, shared, quarantine, runtime, leases and unknown entries", () => {
  const root = fixture();
  for (const child of ["pools/agent-debug/debug/deps/output", "agents/old/debug/output", "artifacts/export/executable", "artifacts/.pending-copy/executable", "shared/compiler/cache", "runtime/task/log", ".locks/lease/owner", ".quarantine/object/payload", "unknown/payload"]) file(root, `target-agent/${child}`);
  const expected = allocation(join(root, "target-agent"));
  const before = buildStorage(root, false, true);
  expect(before.targetAgentComplete).toBe(true);
  expect(before.targetAgentAllocatedBytes).toBe(expected.allocatedBytes);
  expect(before.targetAgentAllocatedBytes).toBeGreaterThan(before.agentPoolAllocatedBytes);
  file(root, "target/dev-output"); file(root, ".test-output/evidence/payload"); file(root, "target/pi-sidecar/sidecar");
  for (const name of ["cargo", "rustup", "kit", "sccache"]) file(root, `outside/${name}/payload`);
  process.env.CARGO_HOME = join(root, "outside/cargo"); process.env.RUSTUP_HOME = join(root, "outside/rustup");
  process.env.SK_PATH = join(root, "outside/kit"); process.env.SCCACHE_DIR = join(root, "outside/sccache");
  const full = buildStorage(root, true);
  expect(full.targetAgentAllocatedBytes).toBe(before.targetAgentAllocatedBytes);
  expect(full.categories.find(item => item.category === "dev-target")!.allocatedBytes).toBeGreaterThan(0);
  expect(full.categories.find(item => item.category === "evidence-and-tasks")!.allocatedBytes).toBeGreaterThan(0);
  expect(full.external.find(item => item.category === "pi-sidecar")!.allocatedBytes).toBeGreaterThan(0);
  expect(full.external.find(item => item.category === "configured-sccache")!.allocatedBytes).toBeGreaterThan(0);
});

test("budget and pool subtotal cannot lose hardlinked bytes to diagnostic category order", () => {
  const root = fixture(), payload = file(root, "target/dev-output");
  for (const child of ["pools/agent-debug/debug/deps/output", "agents/old/debug/output", "artifacts/export/executable", "shared/cache/output"]) {
    const path = join(root, "target-agent", child); fs.mkdirSync(dirname(path), { recursive: true }); fs.linkSync(payload, path);
  }
  const full = buildStorage(root, false), budget = buildStorage(root, false, true);
  const expected = allocation(join(root, "target-agent"));
  expect(full.targetAgentAllocatedBytes).toBe(expected.allocatedBytes);
  expect(budget.targetAgentAllocatedBytes).toBe(full.targetAgentAllocatedBytes);
  expect(full.agentPoolAllocatedBytes).toBe(budget.agentPoolAllocatedBytes);
  const poolSeen = new Set<string>();
  const poolBytes = allocation(join(root, "target-agent/agents"), [], poolSeen).allocatedBytes + allocation(join(root, "target-agent/pools"), [], poolSeen).allocatedBytes;
  expect(full.agentPoolAllocatedBytes).toBe(poolBytes);
  const budgetSeen = new Set<string>();
  allocation(join(root, "target"), [], budgetSeen);
  expect(allocation(join(root, "target-agent"), [], budgetSeen).allocatedBytes).toBe(expected.allocatedBytes - fs.lstatSync(payload).blocks * 512);
});

test("missing roots are absent but permission failures and disappearing children are incomplete", () => {
  const root = fixture(); observedVolume(2 * GiB);
  expect(allocation(join(root, "missing"))).toMatchObject({ present: false, complete: true, allocatedBytes: 0 });
  expect(requireBuildAdmission(root)).toMatchObject({ complete: true, targetAgentAllocatedBytes: 0, withinLimits: true });
  const path = file(root, "target-agent/unknown/payload"), original = fs.lstatSync;
  let failurePath = join(root, "target-agent"), code = "EACCES";
  const stat = spyOn(fs, "lstatSync").mockImplementation(((candidate: fs.PathLike, ...args: unknown[]) => {
    if (String(candidate) === failurePath) throw Object.assign(new Error(code), { code });
    return Reflect.apply(original, fs, [candidate, ...args]);
  }) as typeof fs.lstatSync);
  restore.push(() => stat.mockRestore());
  expect(allocation(failurePath)).toMatchObject({ present: false, complete: false });
  expect(refusal(() => requireBuildAdmission(root)).code).toBe("resource_observation_incomplete");
  failurePath = path; code = "ENOENT";
  expect(buildStorage(root, false, true)).toMatchObject({ targetAgentComplete: false });
  expect(refusal(() => requireBuildAdmission(root)).observation.complete).toBe(false);
});

test("mounted subtrees refuse admission rather than disappearing from the budget", () => {
  const root = fixture(), payload = file(root, "target-agent/unknown/payload"), original = fs.lstatSync;
  observedVolume(2 * GiB);
  const stat = spyOn(fs, "lstatSync").mockImplementation(((candidate: fs.PathLike, ...args: unknown[]) => {
    const value = Reflect.apply(original, fs, [candidate, ...args]);
    return String(candidate) === payload ? Object.assign(Object.create(Object.getPrototypeOf(value)), value, { dev: Number(value.dev) + 1 }) : value;
  }) as typeof fs.lstatSync);
  restore.push(() => stat.mockRestore());
  expect(buildStorage(root, false, true).targetAgentErrors.join(" ")).toContain("inventory_unexpected_mount");
  expect(refusal(() => requireBuildAdmission(root)).code).toBe("resource_observation_incomplete");
});

for (const child of ["target-agent", "target-agent/pools", "target-agent/pools/agent-debug", "target-agent/pools/agent-debug/debug", "target-agent/shared", "target-agent/artifacts", "target-agent/runtime"]) test(`protected write root ${child} cannot redirect outside accounting`, () => {
  const root = fixture(); file(root, "outside/payload"); observedVolume(2 * GiB);
  fs.mkdirSync(dirname(join(root, child)), { recursive: true }); fs.symlinkSync(join(root, "outside"), join(root, child));
  const storage = buildStorage(root, false, true);
  expect(storage.targetAgentComplete).toBe(false);
  expect(storage.targetAgentErrors.join(" ")).toContain("inventory_output_symlink");
  expect(refusal(() => requireBuildAdmission(root)).code).toBe("resource_observation_incomplete");
});

test("unprotected links are counted without traversing their targets", () => {
  const root = fixture(); file(root, "outside/payload"); fs.mkdirSync(join(root, "target-agent/unknown"), { recursive: true });
  fs.symlinkSync(join(root, "outside"), join(root, "target-agent/unknown/link"));
  const result = allocation(join(root, "target-agent"));
  expect(result).toMatchObject({ complete: true, linksNotFollowed: 1, entries: 3 });
  expect(buildStorage(root, false, true).targetAgentAllocatedBytes).toBe(result.allocatedBytes);
});

test("entry and time limits are incomplete, not successful truncated inventories", () => {
  const root = fixture(), payload = file(root, "target-agent/payload"), tree = join(root, "target-agent");
  const originalStat = fs.lstatSync, originalOpen = fs.opendirSync, treeStat = fs.lstatSync(tree), payloadStat = fs.lstatSync(payload);
  const stat = spyOn(fs, "lstatSync").mockImplementation(((path: fs.PathLike, ...args: unknown[]) => String(path) === tree ? treeStat : String(path) === payload ? payloadStat : Reflect.apply(originalStat, fs, [path, ...args])) as typeof fs.lstatSync);
  let closed = 0;
  const open = spyOn(fs, "opendirSync").mockImplementation(((path: fs.PathLike, ...args: unknown[]) => String(path) === tree ? { readSync: () => ({ name: "payload" }), closeSync: () => { closed++; } } : Reflect.apply(originalOpen, fs, [path, ...args])) as typeof fs.opendirSync);
  const clock = spyOn(performance, "now").mockReturnValue(0);
  restore.push(() => stat.mockRestore(), () => open.mockRestore(), () => clock.mockRestore());
  const entries = allocation(tree);
  expect(entries.complete).toBe(false); expect(entries.errors.join(" ")).toContain("inventory_entry_limit"); expect(closed).toBe(1);
  let now = 0; clock.mockImplementation(() => { const value = now; now += 9_000; return value; });
  const expired = allocation(tree);
  expect(expired.complete).toBe(false); expect(expired.errors.join(" ")).toContain("inventory_time_limit");
});

test("publication reserve needs both total-budget and free-space headroom, with inclusive boundaries", () => {
  const root = fixture(); file(root, "target-agent/unknown/payload");
  const allocated = buildStorage(root, false, true).targetAgentAllocatedBytes, reserveBytes = GiB - allocated;
  observedVolume(reserveBytes);
  expect(requireBuildAdmission(root, { phase: "before-copy", reserveBytes })).toMatchObject({ phase: "before-copy", reserveBytes, withinLimits: true, targetAgentAllocatedBytes: allocated, scope: "target-agent", measurement: "allocated-blocks", hardQuota: false, automaticEviction: false });
  const over = refusal(() => requireBuildAdmission(root, { phase: "before-copy", reserveBytes: reserveBytes + 1 }));
  expect(over).toMatchObject({ code: "resource_budget_exceeded", exitCode: 75 });
  expect(over.observation.failureCodes).toEqual(["resource_budget_exceeded", "resource_free_space_below_floor"]);
});

test("free-space floor includes reserve and refuses unavailable or invalid observations", () => {
  const root = fixture(); process.env.SCRIPT_KIT_AGENT_MIN_FREE_GB = "1";
  const stat = spyOn(fs, "statfsSync").mockReturnValue(volume(GiB)); restore.push(() => stat.mockRestore());
  expect(requireBuildAdmission(root).withinLimits).toBe(true);
  expect(refusal(() => requireBuildAdmission(root, { reserveBytes: 1 })).code).toBe("resource_free_space_below_floor");
  stat.mockImplementation(() => { throw new Error("EIO"); });
  expect(refusal(() => requireBuildAdmission(root)).code).toBe("resource_observation_incomplete");
  stat.mockReturnValue(volume(Number.NaN));
  expect(refusal(() => requireBuildAdmission(root)).observation.availableBytes).toBeNull();
});

for (const interactive of ["0", "1"]) test(`managed low-disk bypass is a policy conflict with NONINTERACTIVE=${interactive}`, () => {
  const root = fixture(); observedVolume(2 * GiB);
  process.env.SCRIPT_KIT_NONINTERACTIVE = interactive; process.env.SCRIPT_KIT_AGENT_ALLOW_LOW_DISK = "1";
  expect(refusal(() => requireBuildAdmission(root)).code).toBe("resource_policy_conflict");
});

test("invalid limits and reserves fail with a typed policy refusal", () => {
  const root = fixture(); observedVolume(2 * GiB);
  for (const reserveBytes of [-1, Number.NaN, Number.POSITIVE_INFINITY, 0.5]) expect(refusal(() => requireBuildAdmission(root, { reserveBytes })).code).toBe("resource_policy_conflict");
  for (const value of ["bad", "-1", String(Number.MAX_SAFE_INTEGER)]) {
    process.env.SCRIPT_KIT_AGENT_TARGET_BUDGET_GB = value;
    expect(refusal(() => requireBuildAdmission(root)).code).toBe("resource_policy_conflict");
  }
});

// Only module-boundary filesystem/worker/timer substitutes; no production test mode or real-disk growth.
class FixtureWorker extends EventEmitter {
  readonly threadId = 71;
  readonly requests: number[] = [];
  terminated = 0;
  unreferenced = false;
  termination: Promise<number> = Promise.resolve(0);
  postMessage(id: number): void { this.requests.push(id); }
  terminate(): Promise<number> { this.terminated++; return this.termination; }
  unref(): this { this.unreferenced = true; return this; }
  answer(value: AllocationObservation): void { this.emit("message", { id: this.requests.at(-1), allocation: value }); }
}
async function microtasks(): Promise<void> { for (let i = 0; i < 8; i++) await Promise.resolve(); }
function guardFixture() {
  const root = fixture(), worker = new FixtureWorker(), refusals: BuildResourceError[] = [];
  let now = 0, next = 0;
  const timers = new Map<number, { at: number; callback: () => void }>();
  const timer = spyOn(globalThis, "setTimeout").mockImplementation(((callback: () => void, ms = 0) => {
    const id = ++next; timers.set(id, { at: now + ms, callback }); return id;
  }) as unknown as typeof setTimeout);
  const clear = spyOn(globalThis, "clearTimeout").mockImplementation(id => { timers.delete(Number(id)); });
  const clock = spyOn(performance, "now").mockImplementation(() => now);
  const constructor = spyOn(workerMockBoundary, "Worker").mockImplementation(function () { return worker as unknown as workerThreads.Worker; });
  const free = spyOn(numberStatfsBoundary, "statfs").mockResolvedValue(volume(2 * GiB));
  restore.push(() => timer.mockRestore(), () => clear.mockRestore(), () => clock.mockRestore(), () => constructor.mockRestore(), () => free.mockRestore());
  const advance = async (ms: number): Promise<void> => {
    const end = now + ms;
    await microtasks();
    for (let i = 0; i < 1_000; i++) {
      const next = [...timers].filter(([, timer]) => timer.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!next) { now = end; await microtasks(); return; }
      now = next[1].at; timers.delete(next[0]); next[1].callback(); await microtasks();
    }
    throw new Error("unbounded fixture timers");
  };
  return { root, worker, refusals, timers, free, advance, start: (initial?: BuildResourceObservation) => startBuildResourceGuard(root, error => refusals.push(error), initial) };
}

test("guard samples bounded summaries, admits one scan in flight, and closes timers and worker once", async () => {
  const f = guardFixture(), guard = f.start();
  await microtasks(); expect(f.worker.requests).toHaveLength(1);
  await f.advance(5_000); expect(f.worker.requests).toHaveLength(1);
  f.worker.answer(sample(8192)); await f.advance(5_000);
  expect(f.worker.requests).toHaveLength(2); f.worker.answer(sample(16_384));
  const stopping = guard.stop(); expect(guard.stop()).toBe(stopping);
  const summary = await stopping;
  expect(summary).toMatchObject({ complete: true, workerClosed: true, trigger: null, maximumSampledAllocatedBytes: 16_384, minimumSampledAvailableBytes: 2 * GiB });
  expect(summary.sampleCount).toBeGreaterThan(2); expect(f.refusals).toEqual([]);
  expect(f.worker.terminated).toBe(1); expect(f.timers.size).toBe(0);
  const count = summary.sampleCount; await f.advance(60_000);
  expect(summary.sampleCount).toBe(count); expect(f.worker.requests).toHaveLength(2);
});

for (const breach of ["allocation", "free", "incomplete"] as const) test(`guard requests exact one-shot cancellation for ${breach} refusal`, async () => {
  const f = guardFixture();
  if (breach === "free") { process.env.SCRIPT_KIT_AGENT_MIN_FREE_GB = "1"; f.free.mockResolvedValue(volume(GiB - 1)); }
  const guard = f.start();
  f.worker.answer(breach === "allocation" ? sample(GiB + 1) : breach === "incomplete" ? sample(0, ["EACCES"]) : sample(8192));
  await microtasks();
  const code = breach === "allocation" ? "resource_budget_exceeded" : breach === "free" ? "resource_free_space_below_floor" : "resource_observation_incomplete";
  expect(f.refusals).toHaveLength(1); expect(f.refusals[0]).toBeInstanceOf(BuildResourceError);
  expect(f.refusals[0]).toMatchObject({ code, exitCode: 75, observation: { phase: "in-build", withinLimits: false, hardQuota: false, automaticEviction: false } });
  if (breach === "incomplete") expect(f.refusals[0]!.observation.errors).toEqual(["EACCES"]);
  const summary = await guard.stop(); expect(summary.trigger).toBe(f.refusals[0]!.observation);
  expect(summary).toMatchObject({ complete: false, workerClosed: true });
  await f.advance(60_000); expect(f.refusals).toHaveLength(1); expect(f.timers.size).toBe(0); expect(f.worker.terminated).toBe(1);
});

test("one transient allocation race retries without masking persistent incompleteness", async () => {
  const f = guardFixture(), guard = f.start();
  f.worker.answer(sample(0, ["ENOENT"])); await microtasks();
  expect(f.refusals).toEqual([]); await f.advance(250); expect(f.worker.requests).toHaveLength(2);
  f.worker.answer(sample(4096)); await f.advance(10_000); expect(f.worker.requests).toHaveLength(3);
  f.worker.answer(sample(0, ["ESTALE"])); await f.advance(250); expect(f.worker.requests).toHaveLength(4);
  f.worker.answer(sample(0, ["ESTALE"])); await microtasks();
  expect(f.refusals.map(error => error.code)).toEqual(["resource_observation_incomplete"]);
  expect(await guard.stop()).toMatchObject({ transientFailureCount: 2, complete: false, workerClosed: true });
  expect(f.timers.size).toBe(0);
});

test("free observation retries are bounded and do not retain stale available-space claims", async () => {
  const f = guardFixture(); f.free.mockRejectedValue(Object.assign(new Error("ESTALE"), { code: "ESTALE" }));
  const guard = f.start(); f.worker.answer(sample(4096)); await microtasks();
  expect(f.refusals).toEqual([]); await f.advance(250);
  expect(f.refusals).toHaveLength(1); expect(f.refusals[0]!.observation.availableBytes).toBeNull();
  expect(await guard.stop()).toMatchObject({ transientFailureCount: 1, complete: false, workerClosed: true });
  expect(f.free).toHaveBeenCalledTimes(2); expect(f.timers.size).toBe(0);
});

test("a hung scan is cancelled on deadline, never overlapped by another allocation scan", async () => {
  const f = guardFixture(), guard = f.start();
  const deadline = Number(process.env.SCRIPT_KIT_AGENT_INVENTORY_TIMEOUT_MS ?? 8_000) + 1_000;
  await f.advance(deadline - 1);
  expect(f.refusals).toEqual([]); expect(f.worker.requests).toHaveLength(1);
  await f.advance(1);
  expect(f.worker.requests).toHaveLength(1); expect(f.refusals.map(error => error.code)).toEqual(["resource_observation_incomplete"]);
  expect(await guard.stop()).toMatchObject({ complete: false, workerClosed: true }); expect(f.timers.size).toBe(0);
});

test("stop during in-flight observations is bounded and ignores late completions", async () => {
  const f = guardFixture(), free = Promise.withResolvers<fs.StatsFs>();
  f.free.mockImplementation(() => free.promise);
  const guard = f.start(), summary = await guard.stop();
  expect(summary).toMatchObject({ complete: false, workerClosed: true, sampleCount: 0 });
  free.resolve(volume(0)); f.worker.answer(sample(GiB + 1)); await microtasks(); await f.advance(60_000);
  expect(f.refusals).toEqual([]); expect(summary.sampleCount).toBe(0); expect(f.timers.size).toBe(0);
});

test("failed worker termination has bounded stop and cannot claim complete monitoring", async () => {
  const f = guardFixture(); f.worker.termination = Promise.withResolvers<number>().promise;
  const guard = f.start(); f.worker.answer(sample(4096)); await microtasks();
  let result: BuildResourceSummary | undefined;
  const stopping = guard.stop().then(value => { result = value; });
  await f.advance(999); expect(result).toBeUndefined(); await f.advance(1); await stopping;
  expect(result).toMatchObject({ complete: false, workerClosed: false, workerThreadId: f.worker.threadId });
  expect(f.worker.terminated).toBe(1); expect(f.worker.unreferenced).toBe(true); expect(f.timers.size).toBe(0);
});

test("guard policy refusal schedules no worker or filesystem sample", async () => {
  const f = guardFixture(); process.env.SCRIPT_KIT_AGENT_ALLOW_LOW_DISK = "1";
  const guard = f.start(); await microtasks();
  expect(f.refusals.map(error => error.code)).toEqual(["resource_policy_conflict"]);
  expect(f.worker.requests).toHaveLength(0); expect(f.free).not.toHaveBeenCalled();
  expect(await guard.stop()).toMatchObject({ complete: false, workerClosed: true }); expect(f.timers.size).toBe(0);
});

test("a stalled free-space query refuses on its deadline without launching a second query", async () => {
  const f = guardFixture(); f.free.mockImplementation(() => Promise.withResolvers<fs.StatsFs>().promise);
  const guard = f.start(); f.worker.answer(sample(4096)); await f.advance(2_000);
  expect(f.refusals.map(error => error.code)).toEqual(["resource_observation_incomplete"]);
  expect(f.free).toHaveBeenCalledTimes(1);
  expect(await guard.stop()).toMatchObject({ complete: false, workerClosed: true }); expect(f.timers.size).toBe(0);
});

test("the real request-owned worker scans a tiny fixture and closes its thread", async () => {
  const root = fixture(); file(root, "target-agent/unknown/payload");
  const expected = allocation(join(root, "target-agent")).allocatedBytes;
  const OriginalWorker = workerThreads.Worker, received = Promise.withResolvers<void>();
  const constructor = spyOn(workerMockBoundary, "Worker").mockImplementation(function (filename: string | URL, options?: workerThreads.WorkerOptions) {
    const worker = new OriginalWorker(filename, options);
    worker.once("message", () => received.resolve());
    worker.once("error", received.reject);
    return worker;
  });
  restore.push(() => constructor.mockRestore());
  const free = spyOn(numberStatfsBoundary, "statfs").mockResolvedValue(volume(2 * GiB)); restore.push(() => free.mockRestore());
  const errors: BuildResourceError[] = [], guard = startBuildResourceGuard(root, error => errors.push(error));
  const deadline = setTimeout(() => received.reject(new Error("fixture_worker_deadline")), 5_000);
  try {
    await received.promise;
    expect(await guard.stop()).toMatchObject({ complete: true, workerClosed: true, maximumSampledAllocatedBytes: expected, minimumSampledAvailableBytes: 2 * GiB });
    expect(errors).toEqual([]);
  } finally { clearTimeout(deadline); await guard.stop(); }
});

test("fast operations retain real preflight evidence while cancelling an unfinished first scan", async () => {
  const f = guardFixture(); file(f.root, "target-agent/unknown/payload"); observedVolume(2 * GiB);
  const initial = requireBuildAdmission(f.root, { phase: "preflight" });
  const free = Promise.withResolvers<fs.StatsFs>(); f.free.mockImplementation(() => free.promise);
  const guard = f.start(initial), summary = await guard.stop();
  expect(summary).toMatchObject({ complete: true, workerClosed: true, initialObservationIncluded: true, sampleCount: 1,
    maximumSampledAllocatedBytes: initial.targetAgentAllocatedBytes, minimumSampledAvailableBytes: initial.availableBytes });
  expect(f.worker.requests).toHaveLength(1); expect(f.worker.terminated).toBe(1);
  free.resolve(volume(0)); await microtasks();
  expect(summary.sampleCount).toBe(1); expect(f.refusals).toEqual([]); expect(f.timers.size).toBe(0);
});

for (const patch of [
  { complete: false }, { withinLimits: false }, { reserveBytes: 1 }, { targetAgentBudgetBytes: 2 * GiB },
  { minimumFreeBytes: GiB }, { targetAgentAllocatedBytes: GiB + 1 }, { targetAgentAllocatedBytes: -1 },
  { availableBytes: null }, { failureCodes: ["resource_observation_incomplete"] },
] satisfies Partial<BuildResourceObservation>[]) test(`guard rejects unproved or policy-mismatched seed ${JSON.stringify(patch)}`, async () => {
  const f = guardFixture(); observedVolume(2 * GiB);
  const initial = { ...requireBuildAdmission(f.root), ...patch };
  const guard = f.start(initial); await microtasks();
  expect(f.refusals.map(error => error.code)).toEqual(["resource_policy_conflict"]);
  expect(await guard.stop()).toMatchObject({ complete: false, initialObservationIncluded: false });
  expect(f.worker.requests).toHaveLength(0); expect(f.free).not.toHaveBeenCalled(); expect(f.timers.size).toBe(0);
});

test("preflight seed cannot conceal a later failed sample or failed worker closure", async () => {
  const f = guardFixture(); observedVolume(2 * GiB);
  const guard = f.start(requireBuildAdmission(f.root)); f.worker.termination = Promise.withResolvers<number>().promise;
  f.worker.answer(sample(0, ["EACCES"])); await f.advance(1_000);
  const summary = await guard.stop();
  expect(summary).toMatchObject({ complete: false, workerClosed: false, workerThreadId: f.worker.threadId, initialObservationIncluded: true });
  expect(f.refusals.map(error => error.code)).toEqual(["resource_observation_incomplete"]); expect(f.timers.size).toBe(0);
});
