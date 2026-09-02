import { afterEach, beforeAll, expect, spyOn, test } from "bun:test";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { boundedObservation, Driver, DriverCommandRefused, ProtocolCore, type Json } from "./driver";
import { createArtifactFixture } from "../agentic/build-artifact-fixture.ts";
import { verifyImmutableArtifact, type ArtifactReference } from "../agentic/build-artifact.ts";
import { claimOutput, listManagedTasks, validateOutputTarget } from "../agentic/artifact-lifecycle.ts";
import { issueOwnedEvaluationPermit } from "./lib/operator-safety.ts";
import { spawnOwnedProcess, type OwnedProcess } from "../agentic/owned-process.ts";

const cleanups: Array<() => void | Promise<void>> = [];
const DIRECT_LIFECYCLE_ENV = "SCRIPT_KIT_DRIVER_LIFECYCLE_DIRECT";
const DIRECT_LIFECYCLE_MODE_ENV = "SCRIPT_KIT_DRIVER_LIFECYCLE_MODE";
// The inner assertions retain Bun's 5s deadline. The outer runner separately
// owns interpreter startup, the child deadline, and the supervisor's close.
const directTestTimeoutMs = process.env[DIRECT_LIFECYCLE_ENV] ? 5_000 : 35_000;

// Bun 1.3.11 has two test-entry modes: the common `bun test path/to/test`
// filter form loses stdout/stderr from child processes spawned by that test,
// while `bun test ./path/to/test` consumes them correctly. Lifecycle coverage
// must not depend on command spelling, so the process/stream tests
// delegate once to Bun's direct-path mode and propagate any failure verbatim.
async function delegateToDirectPath(testName: string, mode?: "exit" | "timeout"): Promise<boolean> {
  const sourceRoot = realpathSync(join(import.meta.dir, "../.."));
  if (process.env[DIRECT_LIFECYCLE_ENV]) {
    if (process.env[DIRECT_LIFECYCLE_ENV] !== sourceRoot) throw new Error("lifecycle_private_repository_mismatch");
    return false;
  }
  const root = realpathSync(mkdtempSync(join(tmpdir(), "driver-lifecycle-repository-")));
  // Driver and the supervisor derive their repository from their source paths.
  // Copy the exact implementation, not symlinks or patched authority constants.
  for (const path of [
    "scripts/devtools/driver-lifecycle.test.ts", "scripts/devtools/driver.ts",
    "scripts/devtools/lib/client.ts", "scripts/devtools/lib/operator-safety.ts",
    "scripts/devtools/lib/evidence-class.ts", "scripts/devtools/lib/transport-errors.ts",
    "scripts/devtools/lib/build-ops-inventory.ts",
    "scripts/agentic/build-artifact-fixture.ts", "scripts/agentic/build-artifact.ts",
    "scripts/agentic/artifact-lifecycle.ts", "scripts/agentic/owned-process.ts",
    "scripts/agentic/session-supervisor.py", "scripts/agentic/cargo-cache-locks.sh",
  ]) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    copyFileSync(join(sourceRoot, path), join(root, path));
  }
  // Keep the spawnSync boundary that avoids Bun's filter-mode pipe bug. The
  // ordinary Bun runner inside it supervises and awaits the exact test child.
  const run = Bun.spawnSync([process.execPath, "-e", `
import { spawnOwnedProcess } from ${JSON.stringify(join(root, "scripts/agentic/owned-process.ts"))};
const proc = await spawnOwnedProcess({
  argv: [process.execPath, "test", "./scripts/devtools/driver-lifecycle.test.ts", "-t", ${JSON.stringify(testName)}],
  cwd: ${JSON.stringify(root)}, env: process.env,
  timeoutMs: 20_000, maxOutputBytes: 2 * 1024 * 1024,
});
const streams = [new Response(proc.stdout).text(), new Response(proc.stderr).text()];
let exitCode, output, cleanup;
try {
  exitCode = await proc.exited;
  output = await Promise.all(streams);
} finally {
  cleanup = await proc.close();
  await proc.exited;
  await Promise.allSettled(streams);
}
await Bun.write(Bun.stdout, output[0]);
await Bun.write(Bun.stderr, output[1]);
if (!cleanup.closed) await Bun.write(Bun.stderr, JSON.stringify(cleanup));
process.exitCode = exitCode || (cleanup.closed ? 0 : 1);
`], {
    cwd: root,
    env: { ...process.env, [DIRECT_LIFECYCLE_ENV]: root, [DIRECT_LIFECYCLE_MODE_ENV]: mode ?? "", SCRIPT_KIT_REPO_ROOT: root },
    stdout: "pipe", stderr: "pipe",
  });
  if (run.exitCode !== 0) {
    throw new Error(`direct-path lifecycle child failed (${run.exitCode}); private repository retained: ${root}\n${run.stdout.toString()}${run.stderr.toString()}`);
  }
  // Unknown native closure remains protected evidence even after the exact
  // child is reaped. Never force-dispose that artifact or its private metadata.
  if (mode !== "exit") rmSync(root, { recursive: true, force: true });
  return true;
}

// Close-contract tests wire the real Driver to an inert, private-home transport.
// This private constructor seam does not authorize or exercise application launch.
async function transportFixture(mode: "close" | "refusal" | "stream-error"): Promise<Driver> {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "driver-transport-test-")));
  cleanups.push(() => rmSync(root, { recursive: true, force: true }));
  const env: Record<string, string> = { PATH: "/usr/bin:/bin", SCRIPT_KIT_NONINTERACTIVE: "1" };
  for (const key of ["HOME", "SK_PATH", "CODEX_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "TMPDIR"]) {
    env[key] = join(root, key);
    mkdirSync(env[key]!, { mode: 0o700 });
  }
  const proc = await spawnOwnedProcess({ argv: [process.execPath, "-e", `
import { createInterface } from "node:readline";
let refused = false;
const end = () => {
  process.stdout.write(${JSON.stringify(mode === "refusal" ? "refusal cleanup stdout" : "final stdout after EOF")});
  process.stderr.write(${JSON.stringify(mode === "refusal" ? "refusal cleanup stderr" : "final stderr after EOF")});
  process.exit(0);
};
process.on("SIGTERM", end);
createInterface({ input: process.stdin }).on("line", line => {
  const command = JSON.parse(line);
  const response = refused ? { type: "stateResult", inputValue: "healthy" } :
    { type: "error", code: "stale_target_identity", message: "Owned evaluation operation refused" };
  refused = true;
  console.log(JSON.stringify({ ...response, requestId: command.requestId, protocolVersion: 2 }));
}).on("close", end);
console.log("APP_READY|main-window-ready show=false focus=false stdin-safe");
`], cwd: root, env, timeoutMs: 10_000, maxOutputBytes: 1024 * 1024 });
  const Constructor = Driver as unknown as new (proc: OwnedProcess,
    options: { sessionName: string; sessionDir: string; defaultTimeoutMs: number }) => Driver;
  const driver = new Constructor(proc, { sessionName: `transport-${mode}`, sessionDir: root, defaultTimeoutMs: 2_000 });
  const transport = driver as unknown as {
    readyResolve: (() => void) | null; exited: boolean; exitError: Error | null; streamError: Error | null;
    streamConsumers: Promise<void>[];
    consumeStream(stream: ReadableStream<Uint8Array>, stdout: boolean): Promise<void>;
    onTransportFailure(error: Error): void;
    failAllPending(error: Error): void;
  };
  const ready = new Promise<void>(resolve => { transport.readyResolve = resolve; });
  const consume = (stream: ReadableStream<Uint8Array>, stdout: boolean) => transport.consumeStream(stream, stdout).catch(cause => {
    transport.streamError = cause instanceof Error ? cause : new Error(String(cause));
    transport.onTransportFailure(transport.streamError);
  });
  transport.streamConsumers = [consume(proc.stdout, true), consume(proc.stderr, false)];
  void proc.exited.then(async code => {
    transport.exited = true;
    transport.exitError = new Error(`App process exited (${code})`);
    await boundedObservation(Promise.allSettled(transport.streamConsumers), 1500);
    transport.failAllPending(transport.exitError);
    transport.readyResolve?.();
  }, cause => transport.onTransportFailure(cause instanceof Error ? cause : new Error(String(cause))));
  const observation = await boundedObservation(ready, 2_000);
  if (!observation.completed || !driver.alive) {
    await driver.close();
    throw new Error("Inert transport did not become ready");
  }
  return driver;
}

// Inert protocol/transport simulations, not evidence of real native window closure.
// The real publisher, permit, supervisor, bootstrap and Driver cleanup boundaries run.
interface LifecycleFixture {
  repositoryRoot: string;
  reference: ArtifactReference;
  pidPath: string;
  bootstrapPath: string;
  logPath(): string;
  launch(readyTimeoutMs?: number): Promise<Driver>;
}

function lifecycleFixture(mode: "exit" | "timeout"): LifecycleFixture {
  const repositoryRoot = realpathSync(join(import.meta.dir, "../.."));
  if (process.env[DIRECT_LIFECYCLE_ENV] !== repositoryRoot) throw new Error("lifecycle_requires_private_repository");
  const outputRoot = join(repositoryRoot, ".test-output");
  mkdirSync(outputRoot, { recursive: true, mode: 0o700 });
  const root = mkdtempSync(join(outputRoot, "driver-lifecycle-test-"));
  const pidPath = join(root, "child.pid");
  const sessionPath = join(root, "session-path");
  const readyPath = join(root, "child-ready");
  const bootstrapPath = join(root, "bootstrap-command.json");
  const executable = `#!${process.execPath}
import { createInterface } from "node:readline";
import { renameSync, writeFileSync } from "node:fs";
const mode = ${JSON.stringify(mode)};
const env = process.env;
writeFileSync(${JSON.stringify(pidPath)}, String(process.pid));
writeFileSync(${JSON.stringify(sessionPath)}, env.SCRIPT_KIT_OWNED_EVALUATION_ROOT);
const identity = {
  pid: process.pid,
  processStartTime: env.SCRIPT_KIT_PROCESS_START_TIME,
  processInstanceId: env.SCRIPT_KIT_PROCESS_INSTANCE_ID,
  sessionGeneration: env.SCRIPT_KIT_SESSION_GENERATION,
  binarySha256: env.SCRIPT_KIT_OWNED_EVALUATION_BINARY_SHA256,
  manifestSha256: env.SCRIPT_KIT_OWNED_EVALUATION_MANIFEST_SHA256,
};
const binding = { identity, launchNonce: env.SCRIPT_KIT_OWNED_EVALUATION_NONCE,
  policySha256: env.SCRIPT_KIT_OWNED_EVALUATION_POLICY_SHA256 };
const emit = value => process.stdout.write(JSON.stringify({ protocolVersion: 2, ...value }) + "\\n");
let ended = false;
function end(code = 0) {
  if (ended) return;
  ended = true;
  if (mode !== "exit") emit({ type: "designResult", result: { ...binding, schemaVersion: 1, operation: "end", lifecycle: true,
    shutdownReason: "inputEof", ok: true, ownedWindowsClosed: true, remainingWindows: 0, refusedEffects: 0,
    native: { installed: true, openedWindows: 1, liveWindows: 0, automationWindows: 0,
      completedFrames: 2, readbackImages: 0, refusedOperations: 0 } } });
  const prefix = mode === "exit" ? "exit trailing" : "timeout trailing";
  emit({ type: "fixtureLog", message: prefix + " stdout" });
  process.stderr.write(prefix + " stderr");
  process.exit(code);
}
createInterface({ input: process.stdin }).on("line", line => {
  const command = JSON.parse(line);
  if (command.type === "design" && command.command.operation === "bootstrap") {
    writeFileSync(${JSON.stringify(bootstrapPath)}, JSON.stringify(command));
    if (mode === "exit") end(17);
  }
}).on("close", () => end());
writeFileSync(${JSON.stringify(`${readyPath}.tmp`)}, String(process.pid));
renameSync(${JSON.stringify(`${readyPath}.tmp`)}, ${JSON.stringify(readyPath)});
`;
  const published = createArtifactFixture(repositoryRoot, {
    features: ["owned-ui-evaluation"], executable,
  });
  // Early exit has no native closure event. Preserve its protected runtime
  // evidence and referenced immutable artifact rather than force-disposing it.
  if (mode === "timeout") cleanups.push(published.dispose);
  const artifact = verifyImmutableArtifact(repositoryRoot, published.reference, {
    kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content",
  });
  const claim = claimOutput(validateOutputTarget({ repoRoot: repositoryRoot,
    candidate: join(root, "evaluation"), kind: "directory", probeId: "driver-lifecycle-test" }));
  const ownedEvaluation = issueOwnedEvaluationPermit(artifact, claim, []);
  let active: Driver | undefined;
  let pending: Promise<Driver> | undefined;
  const request = Driver.prototype.request;
  const bootstrapRequest = spyOn(Driver.prototype, "request").mockImplementation(async function (this: Driver, ...args: Parameters<Driver["request"]>) {
    if (args[0].type === "design" && args[0].command.operation === "bootstrap") {
      active = this;
      // Wait for the inert child's stdin handler, then start the unchanged real
      // request timer. A slow interpreter must not masquerade as ready timeout.
      const deadline = performance.now() + 2_000;
      while (!existsSync(readyPath)) {
        if (!this.alive || performance.now() >= deadline) throw new Error("lifecycle_fixture_ready_handshake_failed");
        await Bun.sleep(5);
      }
      expect(readFileSync(readyPath, "utf8")).toBe(readFileSync(pidPath, "utf8"));
    }
    return request.apply(this, args);
  });
  cleanups.push(() => { bootstrapRequest.mockRestore(); });
  cleanups.push(async () => {
    // Await the actual launch/close chain even when a test assertion fails.
    // Driver.close owns its exact supervisor; no PID-only kill or lease recovery.
    if (active) await Promise.allSettled([active.close()]);
    if (pending) await Promise.allSettled([pending]);
    if (active) expect(active.finalization).toMatchObject({ processExited: true, processGroupExited: true, logWriterClosed: true });
  });
  return { repositoryRoot, reference: published.reference, pidPath, bootstrapPath,
    logPath: () => join(readFileSync(sessionPath, "utf8"), "app.log"),
    launch: (readyTimeoutMs = 2_000) => pending = Driver.launch({ ownedEvaluation, readyTimeoutMs, sessionName: `driver-${mode}-test` }) };
}

let preparedLifecycleFixture: LifecycleFixture | undefined;
beforeAll(() => {
  if (!process.env[DIRECT_LIFECYCLE_ENV]) return;
  const mode = process.env[DIRECT_LIFECYCLE_MODE_ENV];
  if (mode === "exit" || mode === "timeout") preparedLifecycleFixture = lifecycleFixture(mode);
}, 5_000);

function processIsAlive(pid: number): boolean {
  try { process.kill(pid, 0); return true; } catch { return false; }
}

function expectLogWriterClosed(logPath: string): void {
  const openFiles = spawnSync("lsof", ["-p", String(process.pid), "-Fn", "--", logPath], { encoding: "utf8" });
  expect(`${openFiles.stdout}${openFiles.stderr}`).not.toContain(logPath);
}

afterEach(async () => {
  const errors: unknown[] = [];
  for (const cleanup of cleanups.splice(0).reverse()) {
    try { await cleanup(); } catch (error) { errors.push(error); }
  }
  if (errors.length) throw new AggregateError(errors, "lifecycle_fixture_cleanup_failed");
});

class RecordingProtocol extends ProtocolCore {
  lastPayload: Json | null = null;
  constructor() { super(1_000, "recording"); }
  // This recorder has no process or external transport; exercise response parsing only.
  protected authorizeCommand(_command: Json): void {}
  protected writeCommand(payload: Json): void { this.lastPayload = payload; }
  get alive(): boolean { return true; }
  async close(): Promise<void> {}
  respond(response: Json): void { this.handleResponse(response); }
}

test("ProtocolCore rejects a correlated response with the wrong terminal type", async () => {
  const protocol = new RecordingProtocol();
  const pending = protocol.request({ type: "getState" }, { expect: "stateResult" });
  const requestId = protocol.lastPayload?.requestId;
  protocol.respond({ requestId, protocolVersion: 2, type: "wrongResult" });
  await expect(pending).rejects.toThrow("response_timeout");
  expect(protocol.matchedResponses).toEqual([]);
});

test("ProtocolCore rejects a request-correlated malformed-payload response", async () => {
  const protocol = new RecordingProtocol();
  const pending = protocol.request({ type: "simulateGpuiEvent", event: {} }, { expect: "simulateGpuiEventResult" });
  const requestId = protocol.lastPayload?.requestId;
  protocol.respond({ type: "externalCommandResult", requestId, protocolVersion: 2, command: "simulateGpuiEvent",
    ok: false, errorCode: "invalid_payload", errorMessage: "missing field `deltaX`" });
  await expect(pending).rejects.toThrow("invalid_payload");
});

test("Driver.close drains trailing stdout/stderr before resolving", async () => {
  if (await delegateToDirectPath("Driver.close drains trailing stdout/stderr before resolving")) return;
  const driver = await transportFixture("close");
  const beforeClose = driver.observedReceivedOutputBytes;
  expect(driver.maxOutputBytes).toBe(1024 * 1024);
  await driver.close();
  expect(driver.alive).toBe(false);
  expect(driver.finalization).toMatchObject({ processExited: true, processGroupExited: true,
    streamsDrained: true, logWriterClosed: true, closed: true, survivors: [] });
  const log = readFileSync(driver.logPath, "utf8");
  expect(log).toContain("final stdout after EOF");
  expect(log).toContain("final stderr after EOF");
  expect(driver.observedReceivedOutputBytes - beforeClose).toBe(Buffer.byteLength("final stdout after EOFfinal stderr after EOF"));
  await expect(driver.close()).resolves.toBeUndefined();
}, directTestTimeoutMs);

test("Driver keeps its transport and cleanup healthy after an evaluator refusal", async () => {
  if (await delegateToDirectPath("Driver keeps its transport and cleanup healthy after an evaluator refusal")) return;
  const driver = await transportFixture("refusal");
  const command = { type: "getState" };
  try {
    await expect(driver.request(command)).rejects.toBeInstanceOf(DriverCommandRefused);
    expect(driver.alive).toBe(true);
    expect((await driver.request(command)).inputValue).toBe("healthy");
  } finally { await driver.close(); }
  expect(driver.finalization).toMatchObject({ processExited: true, processGroupExited: true,
    streamsDrained: true, logWriterClosed: true, closed: true, survivors: [], failureCodes: ["request_closed"] });
  const log = readFileSync(driver.logPath, "utf8");
  expect(log).toContain("refusal cleanup stdout");
  expect(log).toContain("refusal cleanup stderr");
  await expect(driver.close()).resolves.toBeUndefined();
}, directTestTimeoutMs);

test("Driver.close rejects a recorded stream-consumer failure after closing the log", async () => {
  if (await delegateToDirectPath("Driver.close rejects a recorded stream-consumer failure after closing the log")) return;
  const driver = await transportFixture("stream-error");
  const internals = driver as unknown as { streamError: Error | null };
  internals.streamError = new Error("injected stream-consumer failure");
  await expect(driver.close()).rejects.toThrow("INVALID_CLEANUP");
  expect(driver.alive).toBe(false);
  expect(driver.finalization).toMatchObject({ processExited: true, streamsDrained: false,
    logWriterClosed: true, closed: false, failureCodes: expect.arrayContaining(["streams_not_drained"]) });
  expect(readFileSync(driver.logPath, "utf8")).toContain("final stdout after EOF");
}, directTestTimeoutMs);

test("Driver.launch preserves unknown native closure when a child exits before readiness", async () => {
  if (await delegateToDirectPath("Driver.launch preserves unknown native closure when a child exits before readiness", "exit")) return;
  const fixture = preparedLifecycleFixture;
  if (!fixture) throw new Error("lifecycle_fixture_not_prepared");
  await expect(fixture.launch()).rejects.toMatchObject({
    cause: expect.objectContaining({ message: expect.stringContaining("App process exited") }),
    cleanup: expect.objectContaining({ closed: false, processExited: true, processGroupExited: true,
      streamsDrained: true, logWriterClosed: true, ownedWindowsClosed: null, referencesFinalized: false,
      failureCodes: expect.arrayContaining(["windows_not_observed_closed", "native_references_not_finalized"]) }),
  });
  expect(existsSync(fixture.pidPath)).toBe(true);
  expect(processIsAlive(Number(readFileSync(fixture.pidPath, "utf8")))).toBe(false);
  expect(JSON.parse(readFileSync(fixture.bootstrapPath, "utf8"))).toMatchObject({ type: "design", command: { operation: "bootstrap" } });
  const logPath = fixture.logPath();
  const log = readFileSync(logPath, "utf8");
  expect(log).toContain("exit trailing stdout");
  expect(log).toContain("exit trailing stderr");
  expectLogWriterClosed(logPath);
  const runtimeTasks = listManagedTasks(fixture.repositoryRoot).filter(({ record }) =>
    record?.identity.kind === "runtime-run" && record.artifactReferences.some(reference =>
      reference.manifestSha256 === fixture.reference.manifestSha256));
  expect(runtimeTasks).toHaveLength(1);
  expect(runtimeTasks[0]!.record).toMatchObject({ state: "protected",
    cleanup: { closed: false, referencesFinalized: false, ownedWindowsClosed: null } });
  expect(existsSync(join(fixture.repositoryRoot, fixture.reference.manifestPath))).toBe(true);
}, directTestTimeoutMs);

test("Driver.launch finalizes a readiness timeout before rejecting", async () => {
  if (await delegateToDirectPath("Driver.launch finalizes a readiness timeout before rejecting", "timeout")) return;
  const fixture = preparedLifecycleFixture;
  if (!fixture) throw new Error("lifecycle_fixture_not_prepared");
  // Evaluator readiness is the qualified bootstrap reply, not an APP_READY log line.
  await expect(fixture.launch(200)).rejects.toMatchObject({
    cause: expect.objectContaining({ message: expect.stringContaining("response_timeout") }),
    cleanup: expect.objectContaining({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true, logWriterClosed: true }),
  });
  expect(existsSync(fixture.pidPath)).toBe(true);
  expect(processIsAlive(Number(readFileSync(fixture.pidPath, "utf8")))).toBe(false);
  expect(JSON.parse(readFileSync(fixture.bootstrapPath, "utf8"))).toMatchObject({ type: "design", command: { operation: "bootstrap" } });
  const logPath = fixture.logPath();
  const log = readFileSync(logPath, "utf8");
  expect(log).toContain("timeout trailing stdout");
  expect(log).toContain("timeout trailing stderr");
  expectLogWriterClosed(logPath);
}, directTestTimeoutMs);
