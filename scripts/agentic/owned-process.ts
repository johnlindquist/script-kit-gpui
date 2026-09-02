import { spawn, spawnSync } from "node:child_process";
import { isAbsolute, resolve } from "node:path";
import type { OwnedCleanup, OwnedProcessIdentity } from "./artifact-lifecycle.ts";

export interface OwnedProcessOptions {
  argv: readonly string[];
  cwd: string;
  env: Record<string, string>;
  timeoutMs: number;
  maxOutputBytes: number;
  ownedNative?: {
    launchNonce: string;
    binarySha256: string;
    manifestSha256: string;
    policySha256: string;
    task?: { repositoryRoot: string; recordPath: string; identity: { id: string; generation: string }; helperExecutable: string };
  };
}
export interface OwnedProcess {
  readonly pid: number;
  readonly supervisorPid: number;
  readonly identity: OwnedProcessIdentity;
  readonly stdout: ReadableStream<Uint8Array>;
  readonly stderr: ReadableStream<Uint8Array>;
  readonly exited: Promise<number>;
  readonly stdin: { write(data: string | Uint8Array): void; end(): void; flush(): Promise<void> };
  close(): Promise<OwnedCleanup>;
  readonly nativeLifecycle: NativeLifecycleObservation | null;
  /** Decoded child stdout + stderr bytes received from the supervisor, not retained log bytes. */
  readonly observedReceivedOutputBytes: number;
  readonly maxOutputBytes: number;
}

export interface NativeLifecycleObservation {
  type: "designResult";
  protocolVersion: 2;
  result: {
    operation: "end";
    schemaVersion: 1;
    policySha256: string;
    lifecycle: true;
    shutdownReason: "inputEof" | "lifetimeExpired" | "explicitEnd" | "error";
    identity: Pick<OwnedProcessIdentity, "pid" | "processStartTime" | "processInstanceId" | "sessionGeneration"> & {
      binarySha256: string; manifestSha256: string;
    };
    launchNonce: string;
    ok: boolean;
    ownedWindowsClosed: boolean;
    remainingWindows: number;
    refusedEffects: number;
    native: { installed: boolean; openedWindows: number; liveWindows: number; automationWindows: number;
      completedFrames: number; readbackImages: number; refusedOperations: number };
  };
}

function record(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
}

export function isNativeLifecycleCandidate(value: unknown): boolean {
  const envelope = record(value), result = record(envelope.result);
  return envelope.type === "designResult" && (Object.hasOwn(result, "lifecycle") || Object.hasOwn(result, "shutdownReason"));
}

/** A final event has no request authority, and all closure facts come from the exact native launch. */
export function validateNativeLifecycle(value: unknown, identity: OwnedProcessIdentity,
  expected: NonNullable<OwnedProcessOptions["ownedNative"]>): NativeLifecycleObservation {
  const envelope = record(value), result = record(envelope.result), native = record(result.native), actual = record(result.identity);
  const count = (value: unknown): number => {
    if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new Error("native_lifecycle_invalid");
    return value;
  };
  const shutdownReason = result.shutdownReason;
  if (envelope.type !== "designResult" || envelope.protocolVersion !== 2 || Object.hasOwn(envelope, "requestId") ||
      Object.hasOwn(envelope, "response") || result.operation !== "end" || result.lifecycle !== true || result.schemaVersion !== 1 ||
      result.policySha256 !== expected.policySha256 ||
      (shutdownReason !== "inputEof" && shutdownReason !== "lifetimeExpired" && shutdownReason !== "explicitEnd" && shutdownReason !== "error") ||
      result.launchNonce !== expected.launchNonce ||
      (["pid", "processStartTime", "processInstanceId", "sessionGeneration"] as const).some(key => actual[key] !== identity[key]) ||
      actual.binarySha256 !== expected.binarySha256 || actual.manifestSha256 !== expected.manifestSha256 ||
      typeof result.ok !== "boolean" || typeof result.ownedWindowsClosed !== "boolean" || typeof native.installed !== "boolean") {
    throw new Error("native_lifecycle_invalid");
  }
  const counters = { installed: native.installed, openedWindows: count(native.openedWindows), liveWindows: count(native.liveWindows),
    automationWindows: count(native.automationWindows), completedFrames: count(native.completedFrames),
    readbackImages: count(native.readbackImages), refusedOperations: count(native.refusedOperations) };
  const remainingWindows = count(result.remainingWindows), refusedEffects = count(result.refusedEffects);
  if (remainingWindows !== counters.liveWindows || counters.liveWindows > counters.openedWindows ||
      result.ownedWindowsClosed !== (result.ok && counters.installed && counters.liveWindows === 0 && counters.automationWindows === 0)) throw new Error("native_lifecycle_invalid");
  return { type: "designResult", protocolVersion: 2, result: { operation: "end", lifecycle: true, schemaVersion: 1, policySha256: expected.policySha256, shutdownReason,
    identity: { pid: identity.pid, processStartTime: identity.processStartTime, processInstanceId: identity.processInstanceId,
      sessionGeneration: identity.sessionGeneration, binarySha256: expected.binarySha256, manifestSha256: expected.manifestSha256 },
    launchNonce: expected.launchNonce, ok: result.ok, ownedWindowsClosed: result.ownedWindowsClosed, remainingWindows, refusedEffects, native: counters } };
}

function bounded<T>(input: Promise<T>, ms: number): Promise<T | undefined> {
  const { promise, resolve: settle } = Promise.withResolvers<T | undefined>();
  const timer = setTimeout(() => settle(undefined), ms);
  input.then(value => { clearTimeout(timer); settle(value); }, () => { clearTimeout(timer); settle(undefined); });
  return promise;
}

/** Standard pipes have one descriptor owner. stdin carries lifetime/input frames; stdout carries control/output frames. */
export async function spawnOwnedProcess(options: OwnedProcessOptions): Promise<OwnedProcess> {
  if (!options.argv.length || options.argv.some(value => !value || value.includes("\0"))
    || !Number.isSafeInteger(options.timeoutMs) || options.timeoutMs < 1 || options.timeoutMs > 7_200_000
    || !Number.isSafeInteger(options.maxOutputBytes) || options.maxOutputBytes < 1 || options.maxOutputBytes > 268_435_456) {
    throw new Error("invalid_owned_process_options");
  }
  if (options.ownedNative && (
    !/^[a-f0-9-]{36}$/.test(options.ownedNative.launchNonce) ||
    [options.ownedNative.binarySha256, options.ownedNative.manifestSha256, options.ownedNative.policySha256].some(value => !/^[a-f0-9]{64}$/.test(value)) ||
    (options.ownedNative.task && (!isAbsolute(options.ownedNative.task.helperExecutable) || !isAbsolute(options.ownedNative.task.recordPath))))) {
    throw new Error("owned_native_options_invalid");
  }
  // Isolated Python ignores PYTHONDONTWRITEBYTECODE, so infrastructure must pass -B explicitly.
  const python = spawnSync("python3", ["-I", "-S", "-B", "-c", "import sys; print(sys.executable)"], {
    cwd: options.cwd, env: options.env, encoding: "utf8", timeout: 5_000, maxBuffer: 4_096,
  });
  const interpreter = python.stdout?.trim() ?? "";
  if (python.error || python.status !== 0 || !isAbsolute(interpreter) || interpreter.includes("\n")) {
    throw new Error("supervisor_python_resolution_failed", { cause: python.error ?? python.stderr });
  }
  // Detached supervisors survive an outer process-group stop long enough to reap their own children.
  const child = spawn(interpreter, ["-I", "-S", "-B", resolve(import.meta.dir, "session-supervisor.py"), "--request-owned"], {
    cwd: options.cwd, env: options.env, detached: true, stdio: ["pipe", "pipe", "pipe"],
  });
  let identity: OwnedProcessIdentity | undefined;
  let observedCleanup: OwnedCleanup | undefined;
  let nativeLifecycle: NativeLifecycleObservation | null = null;
  let protocolFailure: string | undefined;
  let supervisorFailureCodes: string[] = [];
  let discardingOutput = false, closingOutput = false, outputClosed = false, inputEnded = false, nativeExited = false;
  let outputBytes = 0, buffer = "";
  const controllers: ReadableStreamDefaultController<Uint8Array>[] = [];
  const resume = () => {
    if (discardingOutput || closingOutput || controllers.every(controller => (controller.desiredSize ?? 0) > 0)) child.stdout!.resume();
  };
  const streams = [0, 1].map(index => new ReadableStream<Uint8Array>({
    start(controller) { controllers[index] = controller; },
    pull() { resume(); },
    cancel() { discardingOutput = true; cancel(); resume(); },
  }, { highWaterMark: 64 * 1024, size: bytes => bytes.byteLength }));
  const finishOutput = (error?: Error) => {
    if (outputClosed) return;
    outputClosed = true;
    for (const controller of controllers) {
      try { if (error) controller.error(error); else controller.close(); } catch { /* A cancelled reader is already closed. */ }
    }
  };
  const { promise: started, resolve: resolveStarted } = Promise.withResolvers<OwnedProcessIdentity | undefined>();
  const { promise: exited, resolve: resolveExited } = Promise.withResolvers<number>();
  const { promise: supervisorExit, resolve: resolveSupervisorExit } = Promise.withResolvers<number | null>();
  const { promise: supervisorClosed, resolve: resolveSupervisorClosed } = Promise.withResolvers<boolean>();
  const cancel = () => {
    if (!child.stdin!.destroyed && !child.stdin!.writableEnded && !observedCleanup) child.stdin!.end('{"event":"close"}\n');
  };
  const fail = (message: string) => {
    protocolFailure ??= message;
    discardingOutput = true;
    cancel(); resume(); finishOutput(new Error(message));
    resolveStarted(undefined); resolveExited(70);
  };
  child.once("error", error => { fail(error.message); resolveSupervisorExit(null); resolveSupervisorClosed(false); });
  // Exit may precede buffered frames; only stdout EOF can rule out a started event.
  child.once("exit", code => { nativeExited = true; resolveSupervisorExit(code); });
  child.once("close", () => resolveSupervisorClosed(true));
  child.stdin!.on("error", () => { if (!observedCleanup && !nativeExited) fail("lifetime_pipe_error"); });
  child.stderr!.on("data", () => fail("supervisor_stderr_output"));
  child.stderr!.on("error", () => fail("supervisor_stderr_error"));
  child.stdout!.setEncoding("utf8");
  child.stdout!.on("data", (chunk: string) => {
    if (protocolFailure) return;
    try {
      buffer += chunk;
      let newline: number;
      while ((newline = buffer.indexOf("\n")) !== -1) {
        if (newline > 128 * 1024) throw new Error("supervisor_control_limit");
        const event = JSON.parse(buffer.slice(0, newline));
        buffer = buffer.slice(newline + 1);
        if (event.event === "started") {
          if (identity || event.identity?.supervisorPid !== child.pid
            || !Number.isSafeInteger(event.identity.pid) || event.identity.pid <= 0
            || event.identity.processGroupId !== event.identity.pid
            || ["processStartTime", "processInstanceId", "supervisorStartTime", "sessionGeneration"].some(key => typeof event.identity[key] !== "string" || !event.identity[key])) {
            throw new Error("supervisor_identity_invalid");
          }
          identity = Object.freeze(event.identity);
          resolveStarted(identity);
        } else if (event.event === "output") {
          if (!identity || observedCleanup || !["stdout", "stderr"].includes(event.channel)
            || typeof event.data !== "string" || event.data.length > 87_384
            || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(event.data)) throw new Error("supervisor_output_invalid");
          const bytes = Buffer.from(event.data, "base64");
          outputBytes += bytes.length;
          if (bytes.length > 65_536 || outputBytes > options.maxOutputBytes) throw new Error("process_output_limit");
          if (!discardingOutput) {
            controllers[event.channel === "stdout" ? 0 : 1]!.enqueue(bytes);
            if (!closingOutput && controllers.some(controller => (controller.desiredSize ?? 0) <= 0)) child.stdout!.pause();
          }
        } else if (event.event === "done") {
          if (observedCleanup || !event.cleanup || !Number.isInteger(event.exitCode)) throw new Error("supervisor_completion_invalid");
          if (Array.isArray(event.cleanup.failureCodes) && event.cleanup.failureCodes.every((value: unknown) => typeof value === "string"))
            supervisorFailureCodes = event.cleanup.failureCodes;
          if (options.ownedNative) {
            if (!identity) throw new Error("supervisor_identity_missing");
            if (event.nativeLifecycle !== null && event.nativeLifecycle !== undefined)
              nativeLifecycle = validateNativeLifecycle(event.nativeLifecycle, identity, options.ownedNative);
            if (event.cleanup.ownedWindowsClosed === true && (!nativeLifecycle?.result.ownedWindowsClosed || event.nativeLifecycleFailure))
              throw new Error("supervisor_native_closure_unproved");
          }
          observedCleanup = event.cleanup;
          finishOutput(); resolveExited(event.exitCode);
        } else throw new Error("supervisor_event_invalid");
      }
      if (buffer.length > 128 * 1024) throw new Error("supervisor_control_limit");
    } catch (error) { fail(String(error)); }
  });
  child.stdout!.on("error", () => fail("control_stream_error"));
  child.stdout!.on("end", () => {
    if (!observedCleanup || buffer) fail("control_eof_without_cleanup");
    resolveStarted(undefined);
  });
  child.stdin!.write(`${JSON.stringify(options)}\n`);
  let closing: Promise<OwnedCleanup> | undefined;
  const close = (): Promise<OwnedCleanup> => closing ??= (async () => {
    // Drain final output without consumer backpressure; maxOutputBytes still bounds retention.
    closingOutput = true;
    cancel(); resume();
    const status = await bounded(supervisorExit, 9_000);
    if (status === undefined) { child.kill("SIGTERM"); await bounded(supervisorExit, 7_000); }
    child.stdin!.destroy();
    const closed = await bounded(supervisorClosed, 1_000);
    const cleanup = observedCleanup;
    finishOutput();
    if (cleanup && status !== undefined && closed && !protocolFailure) return cleanup;
    child.stdout!.destroy(); child.stderr!.destroy();
    return {
      resourcesAcquired: identity !== undefined, processExited: cleanup?.processExited ?? false,
      processGroupExited: cleanup?.processGroupExited ?? false, streamsDrained: cleanup?.streamsDrained ?? false,
      logWriterClosed: true, ownedWindowsClosed: null, referencesFinalized: !options.ownedNative?.task, closed: false,
      survivors: cleanup?.survivors.length ? cleanup.survivors : [{ kind: "process-group", identity: String(identity?.processGroupId ?? "unobserved"), observation: "unknown" }],
      failureCodes: [...new Set([...(cleanup?.failureCodes ?? supervisorFailureCodes), protocolFailure ?? "supervisor_finalization_unproved"])],
    };
  })();
  // Enclose the supervisor's 20s metadata handoff, without changing the child lifetime.
  const actual = await bounded(started, options.ownedNative?.task ? 25_000 : 5_000);
  if (!actual) {
    const cleanup = await close();
    throw Object.assign(new Error(protocolFailure ?? "owned_process_start_failed"), { cleanup });
  }
  return {
    pid: actual.pid, supervisorPid: actual.supervisorPid, identity: actual,
    stdout: streams[0]!, stderr: streams[1]!, exited, close,
    get observedReceivedOutputBytes() { return outputBytes; },
    maxOutputBytes: options.maxOutputBytes,
    get nativeLifecycle() { return nativeLifecycle ? structuredClone(nativeLifecycle) : null; },
    stdin: {
      write(data) {
        if (inputEnded || child.stdin!.destroyed || child.stdin!.writableEnded) throw new Error("owned_stdin_closed");
        const bytes = typeof data === "string" ? Buffer.from(data) : Buffer.from(data.buffer, data.byteOffset, data.byteLength);
        for (let offset = 0; offset < bytes.length; offset += 65_536) child.stdin!.write(`${JSON.stringify({ event: "stdin", data: bytes.subarray(offset, offset + 65_536).toString("base64") })}\n`);
      },
      end() { if (!inputEnded) { inputEnded = true; if (!child.stdin!.destroyed && !child.stdin!.writableEnded && !observedCleanup) child.stdin!.write('{"event":"stdin-end"}\n'); } },
      async flush() {
        if (!child.stdin!.writableNeedDrain) return;
        const { promise, resolve: settle } = Promise.withResolvers<boolean>();
        child.stdin!.once("drain", () => settle(true)); child.stdin!.once("error", () => settle(false));
        const drained = await bounded(promise, 1_000);
        if (!drained) throw new Error("owned_stdin_flush_timeout");
      },
    },
  };
}
