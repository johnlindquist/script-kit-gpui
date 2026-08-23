#!/usr/bin/env bun
/**
 * scripts/devtools/driver.ts — persistent, event-driven protocol driver.
 *
 * Two transports share one typed protocol surface (ProtocolCore):
 *
 * - Driver.launch(): owns the app process directly. Commands are written to
 *   the app's stdin pipe and responses are matched event-driven from the
 *   app's stdout pipe (the app writes one flushed JSON line per protocol
 *   response — see src/stdin_commands/mod.rs create_stdout_response_sender).
 *   This replaces the per-command path of: bun process spawn → session.sh →
 *   FIFO forwarder → 50ms polling of protocol-responses.ndjson, which costs
 *   ~0.5-2s per command. A driver round trip is one pipe write + one pipe read.
 *
 * - Driver.attach(): connects to an ALREADY-RUNNING session.sh session by
 *   name. Commands are written to the session's input FIFO (honoring the same
 *   <session>/command.lock session.sh uses) and responses are tailed from the
 *   session's protocol-responses.ndjson. close() never kills the app — the
 *   session outlives the client. This is the cheap path for one-shot
 *   inspections against a warm app, and the sandbox-escape path: a caller
 *   outside the sandbox launches the session, a sandboxed agent attaches.
 *
 * Usage (library):
 *   import { Driver } from "./driver";
 *   const d = await Driver.launch({ sandboxHome: true });   // owns the app
 *   const a = await Driver.attach({ session: "default" });  // joins a session
 *   await d.setFilterAndWait("notes");
 *   const state = await d.getState();
 *   await d.close();
 *
 * Both transports support `await using` (Symbol.asyncDispose → close()).
 *
 * Usage (smoke checks):
 *   bun scripts/devtools/driver.ts smoke
 *   bun scripts/devtools/driver.ts attach-smoke [session]
 */

import {
  mkdirSync,
  copyFileSync,
  existsSync,
  rmdirSync,
  statSync,
  symlinkSync,
  readFileSync,
  openSync,
  readSync,
  closeSync,
  appendFileSync,
  writeFileSync,
  unlinkSync,
  watch,
  type FSWatcher,
} from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import type { Subprocess } from "bun";
import {
  clipboardCaptureFixtureCommand,
  gpuiKeyDownCommand,
  triggerActionCommand,
} from "./lib/client.ts";
import {
  assertNoninteractiveDriverLaunch,
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveUnownedSessionCommand,
} from "./lib/operator-safety.ts";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
/**
 * Both build paths produce a runnable binary: ./dev.sh owns target/debug and
 * agent-cargo.sh owns the shared agent pool. Historically the driver silently
 * defaulted to target/debug, so an agent that had just built via agent-cargo
 * verified a stale dev.sh binary. With no explicit override we now pick the
 * freshest candidate by mtime and say so on stderr.
 */
const BINARY_CANDIDATES = [
  join(PROJECT_ROOT, "target/debug/script-kit-gpui"),
  join(PROJECT_ROOT, "target-agent/pools/agent-debug/debug/script-kit-gpui"),
];

function resolveDefaultBinary(): string {
  const explicit = process.env.SCRIPT_KIT_GPUI_BINARY;
  if (explicit) return explicit;

  const found = BINARY_CANDIDATES.flatMap((path) => {
    try {
      return [{ path, mtimeMs: statSync(path).mtimeMs }];
    } catch {
      return [];
    }
  }).sort((a, b) => b.mtimeMs - a.mtimeMs);

  if (found.length === 0) return BINARY_CANDIDATES[0];
  const chosen = found[0];
  if (found.length > 1) {
    const stale = found[1];
    const ageGapSec = Math.round((chosen.mtimeMs - stale.mtimeMs) / 1000);
    console.error(
      `[driver] binary: ${chosen.path} (freshest of ${found.length} candidates; ` +
        `${stale.path} is ${ageGapSec}s older). Pass binary:/SCRIPT_KIT_GPUI_BINARY to override.`,
    );
  } else {
    console.error(`[driver] binary: ${chosen.path} (only candidate present)`);
  }
  return chosen.path;
}
const READY_MARKER_STARTUP = "STARTUP_READY ";
const READY_MARKER_APP =
  "APP_READY|main-window-ready show=false focus=false stdin-safe";
const DEFAULT_RUST_LOG =
  process.env.SCRIPT_KIT_AGENTIC_RUST_LOG ??
  "info,gpui::window=off,gpui=warn,hyper=warn,reqwest=warn";

export type Json = Record<string, any>;

/** Target-local, pixel-delta scroll input for GPUI's real event pipeline. */
export interface GpuiScrollWheelEvent {
  x: number;
  y: number;
  deltaX: number;
  deltaY: number;
  phase: "started" | "moved" | "ended";
  directPhase?:
    | "none"
    | "mayBegin"
    | "began"
    | "changed"
    | "stationary"
    | "ended"
    | "cancelled";
  momentumPhase?:
    | "none"
    | "mayBegin"
    | "began"
    | "changed"
    | "stationary"
    | "ended"
    | "cancelled";
  timestampSeconds?: number;
}

/** Pure, fail-closed encoder for GPUI's exact pixel-only wheel protocol. */
export function gpuiScrollWheelCommand(event: GpuiScrollWheelEvent, target?: Json): Json {
  const required = ["x", "y", "deltaX", "deltaY"] as const;
  if (required.some((field) =>
    typeof event[field] !== "number" || !Number.isFinite(event[field])
  )) {
    throw new Error("GPUI scroll-wheel coordinates and deltas must be finite CSS pixels");
  }
  if (!["started", "moved", "ended"].includes(event.phase)) {
    throw new Error("GPUI scroll-wheel phase must be started, moved, or ended");
  }
  const allowedKeys = new Set([
    "x", "y", "deltaX", "deltaY", "phase", "directPhase", "momentumPhase", "timestampSeconds",
  ]);
  if (Object.keys(event).some((field) => !allowedKeys.has(field))) {
    throw new Error("GPUI scroll-wheel events allow only reviewed pixel-delta protocol fields");
  }
  const lifecyclePhases = new Set([
    "none", "mayBegin", "began", "changed", "stationary", "ended", "cancelled",
  ]);
  for (const field of ["directPhase", "momentumPhase"] as const) {
    if (event[field] !== undefined && !lifecyclePhases.has(event[field]!)) {
      throw new Error(`GPUI scroll-wheel ${field} is not a supported lifecycle phase`);
    }
  }
  if (
    event.timestampSeconds !== undefined &&
    (!Number.isFinite(event.timestampSeconds) || event.timestampSeconds < 0)
  ) {
    throw new Error("GPUI scroll-wheel timestamps must be finite nonnegative seconds");
  }

  const command: Json = {
    type: "simulateGpuiEvent",
    event: {
      type: "scrollWheel",
      x: event.x,
      y: event.y,
      deltaX: event.deltaX,
      deltaY: event.deltaY,
      phase: event.phase,
      ...(event.directPhase === undefined ? {} : { directPhase: event.directPhase }),
      ...(event.momentumPhase === undefined ? {} : { momentumPhase: event.momentumPhase }),
      ...(event.timestampSeconds === undefined ? {} : { timestampSeconds: event.timestampSeconds }),
    },
  };
  if (target !== undefined) command.target = target;
  return command;
}

/** Stable semantic + viewport contract emitted for the currently active native list. */
export interface ActiveListScrollReceipt extends Json {
  surface: string;
  implementation: "variable_list" | "uniform_list" | "tracked_column";
  listKind: "variable_list" | "uniform_list" | "tracked_column";
  selectedIndex: number | null;
  selectedSemanticId: string | null;
  hoveredIndex: number | null;
  hoveredSemanticId: string | null;
  hoverSuppressedUntilPointerMove: boolean;
  focusedSemanticId: string | null;
  logicalScrollTop: number | null;
  scrollTopItem: number | null;
  scrollTopOffsetItems: number | null;
  scrollTopOffsetPx: number | null;
  firstVisibleIndex: number | null;
  lastVisibleIndexExclusive: number | null;
  firstVisibleSemanticId: string | null;
  lastVisibleSemanticId: string | null;
  itemCount: number;
  inputMode: "keyboard" | "mouse";
  lastInteractionSource: string;
}

let launchCounter = 0;

export interface DriverOptions {
  /**
   * Path to the app binary. Defaults to SCRIPT_KIT_GPUI_BINARY, else the
   * freshest (by mtime) of target/debug and the agent-cargo pool binary.
   */
  binary?: string;
  /**
   * Session label reported to the app (logs/protocol bus). Treated as a
   * label, not an address: the derived artifact directory is always
   * uniquified per launch so parallel loops reusing the same name never
   * clobber each other. Pass `sessionDir` to take full control.
   */
  sessionName?: string;
  /** Directory for driver artifacts (app.log, protocol bus). */
  sessionDir?: string;
  /**
   * When true, point HOME/SK_PATH/CODEX_HOME at a fresh sandbox under
   * sessionDir so the driven app never touches real user data and starts from
   * a known state.
   */
  sandboxHome?: boolean;
  /**
   * With sandboxHome, copy this deterministic theme.json fixture into the
   * fresh SK_PATH before the app starts. This lets motion/contrast probes
   * exercise an exact calibration without reading or mutating the user's
   * live theme.
   */
  themeFixturePath?: string;
  /**
   * With sandboxHome, symlink the real ~/.scriptkit/models into the sandbox
   * so the app reuses the multi-GB dictation/brain model downloads instead
   * of re-downloading into every session dir. Pass false only when a probe
   * specifically tests model-download behavior. Default true.
   */
  sharedModels?: boolean;
  /**
   * With sandboxHome, seed the sandbox with the Pi/Codex auth state live
   * Agent Chat probes need (runs scripts/agentic/seed-sandbox-home.sh:
   * APFS-clones ~/.pi plus ~/.codex/{auth.json,config.toml}). Default false —
   * leave it off unless the probe drives a live agent.
   */
  seedAgentAuth?: boolean;
  /** Extra env vars for the app process (test providers, feature flags). */
  env?: Record<string, string>;
  /** Max ms to wait for the readiness log marker. Default 10000. */
  readyTimeoutMs?: number;
  /** Default per-request timeout. Default 5000. */
  defaultTimeoutMs?: number;
  /**
   * Also mirror responses to protocol-responses.ndjson like session.sh
   * sessions do (useful for debugging with existing tooling). Default true;
   * the driver itself never reads this file.
   */
  protocolBusFile?: boolean;
}

export interface AttachOptions {
  /** Name of the running session.sh session to join. Default "default". */
  session?: string;
  /** Root of session dirs. Default SCRIPT_KIT_SESSION_DIR or /tmp/sk-agentic-sessions. */
  sessionsRoot?: string;
  /** Default per-request timeout. Default 5000. */
  defaultTimeoutMs?: number;
  /**
   * Verify the session answers a getState probe before returning. Default
   * true — attach fails fast with an actionable error instead of a hang.
   */
  verify?: boolean;
  /** Poll interval for the response-file tail fallback. Default 100ms. */
  pollIntervalMs?: number;
}

export interface DriverStats {
  requestsSent: number;
  responsesMatched: number;
  unmatchedResponses: number;
  readyWaitMs: number;
}

interface Pending {
  resolve: (value: Json) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
  expectedType: string | null;
}

/**
 * Shared protocol surface: requestId bookkeeping, response matching, and the
 * typed helpers. Subclasses provide the transport (writeCommand) and
 * lifecycle (close).
 */
export abstract class ProtocolCore {
  readonly stats: DriverStats = {
    requestsSent: 0,
    responsesMatched: 0,
    unmatchedResponses: 0,
    readyWaitMs: 0,
  };
  readonly matchedResponses: Array<{
    requestId: string;
    expectedType: string | null;
    responseType: string | null;
  }> = [];

  protected pending = new Map<string, Pending>();
  protected requestCounter = 0;
  protected defaultTimeoutMs: number;
  protected requestIdPrefix: string;

  protected constructor(defaultTimeoutMs: number, requestIdPrefix = "drv") {
    this.defaultTimeoutMs = defaultTimeoutMs;
    this.requestIdPrefix = requestIdPrefix;
  }

  /** Transport write of one JSON command line. */
  protected abstract writeCommand(payload: Json): void;

  abstract get alive(): boolean;

  abstract close(): Promise<void>;

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }

  /** Fire-and-forget: write one command line to the transport. */
  send(command: Json): void {
    assertNoninteractiveProtocolCommand(command);
    this.writeCommand(command);
  }

  /**
   * Send a command and resolve when the response carrying the same requestId
   * arrives. Event-driven — no polling subprocesses. The optional `expect`
   * is advisory only (any typed response settles the request; callers
   * inspect `type` themselves).
   */
  request(
    command: Json,
    opts: { expect?: string; timeoutMs?: number } = {},
  ): Promise<Json> {
    assertNoninteractiveProtocolCommand(command);
    const requestId: string =
      typeof command.requestId === "string" && command.requestId.length > 0
        ? command.requestId
        : `${this.requestIdPrefix}-${process.pid}-${++this.requestCounter}`;
    const { requestId: _callerRequestId, ...commandWithoutRequestId } = command;
    const payload: Json = { requestId, ...commandWithoutRequestId };
    const timeoutMs = opts.timeoutMs ?? this.defaultTimeoutMs;

    return new Promise<Json>((resolvePromise, rejectPromise) => {
      const timer = setTimeout(() => {
        this.pending.delete(requestId);
        rejectPromise(
          new Error(
            `Timeout (${timeoutMs}ms) waiting for response to requestId '${requestId}' (${payload.type})`,
          ),
        );
      }, timeoutMs);
      this.pending.set(requestId, {
        resolve: resolvePromise,
        reject: rejectPromise,
        timer,
        expectedType: opts.expect ?? null,
      });
      this.stats.requestsSent += 1;
      try {
        this.writeCommand(payload);
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(requestId);
        rejectPromise(
          error instanceof Error ? error : new Error(String(error)),
        );
      }
    });
  }

  protected handleResponse(parsed: Json): void {
    const requestId = parsed.requestId;
    if (typeof requestId !== "string") return;
    const pending = this.pending.get(requestId);
    if (!pending) {
      this.stats.unmatchedResponses += 1;
      return;
    }
    this.pending.delete(requestId);
    clearTimeout(pending.timer);
    this.stats.responsesMatched += 1;
    this.matchedResponses.push({
      requestId,
      expectedType: pending.expectedType,
      responseType:
        typeof parsed.responseType === "string"
          ? parsed.responseType
          : typeof parsed.type === "string"
            ? parsed.type
            : null,
    });
    if (parsed.type === "externalCommandResult" && parsed.ok === false) {
      const code =
        typeof parsed.errorCode === "string" ? ` [${parsed.errorCode}]` : "";
      const message =
        typeof parsed.errorMessage === "string"
          ? parsed.errorMessage
          : "Protocol request failed";
      pending.reject(new Error(`${message}${code}`));
      return;
    }
    pending.resolve(parsed);
  }

  protected failAllPending(error: Error): void {
    for (const [, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pending.clear();
  }

  // --- typed helpers ---------------------------------------------------------

  getState(opts: { timeoutMs?: number } = {}): Promise<Json> {
    return this.request(
      { type: "getState" },
      { expect: "stateResult", ...opts },
    );
  }

  getTargetState(
    target: Json,
    opts: { timeoutMs?: number } = {},
  ): Promise<Json> {
    return this.request(
      { type: "getState", target },
      { expect: "stateResult", ...opts },
    );
  }

  async getActiveListScroll(opts: { timeoutMs?: number } = {}): Promise<ActiveListScrollReceipt> {
    const state = await this.getState(opts);
    const receipt = state.activeListScroll ?? state.mainListScroll;
    if (!receipt || typeof receipt !== "object" || Array.isArray(receipt)) {
      throw new Error("activeListScroll missing from stateResult");
    }
    return receipt as ActiveListScrollReceipt;
  }

  getElements(
    extra: Json = {},
    opts: { timeoutMs?: number } = {},
  ): Promise<Json> {
    return this.request(
      { type: "getElements", ...extra },
      { expect: "elementsResult", ...opts },
    );
  }

  getLayoutInfo(
    extra: Json = {},
    opts: { timeoutMs?: number } = {},
  ): Promise<Json> {
    return this.request(
      { type: "getLayoutInfo", ...extra },
      { expect: "layoutInfoResult", ...opts },
    );
  }

  setFilter(text: string): void {
    this.send({ type: "setFilter", text });
  }

  simulateKey(key: string, modifiers: string[] = []): void {
    this.send({ type: "simulateKey", key, modifiers });
  }

  simulateGpuiEvent(
    event: Json,
    opts: { target?: Json; timeoutMs?: number } = {},
  ): Promise<Json> {
    const command: Json = { type: "simulateGpuiEvent", event };
    if (opts.target !== undefined) command.target = opts.target;
    return this.request(command, {
      expect: "simulateGpuiEventResult",
      timeoutMs: opts.timeoutMs ?? this.defaultTimeoutMs,
    });
  }

  simulateGpuiKeyDown(
    key: string,
    opts: { text?: string; modifiers?: string[]; target?: Json; timeoutMs?: number } = {},
  ): Promise<Json> {
    return this.request(
      gpuiKeyDownCommand(key, opts.text, opts.modifiers ?? [], opts.target),
      {
        expect: "simulateGpuiEventResult",
        timeoutMs: opts.timeoutMs ?? this.defaultTimeoutMs,
      },
    );
  }

  triggerAction(
    actionId: string,
    opts: { host?: string; timeoutMs?: number } = {},
  ): Promise<Json> {
    return this.request(triggerActionCommand(actionId, opts.host), {
      expect: "triggerActionResult",
      timeoutMs: opts.timeoutMs ?? this.defaultTimeoutMs,
    });
  }

  /** Dispatch a phased, pixel-only wheel event at target-local coordinates. */
  simulateGpuiScrollWheel(
    event: GpuiScrollWheelEvent,
    opts: { target?: Json; timeoutMs?: number } = {},
  ): Promise<Json> {
    return this.request(gpuiScrollWheelCommand(event, opts.target), {
      expect: "simulateGpuiEventResult",
      timeoutMs: opts.timeoutMs ?? this.defaultTimeoutMs,
    });
  }

  async simulateGpuiClick(
    x: number,
    y: number,
    opts: { target?: Json; button?: string; timeoutMs?: number } = {},
  ): Promise<Json[]> {
    const eventTarget = opts.target;
    const timeoutMs = opts.timeoutMs;
    const button = opts.button ?? "left";
    const move = await this.simulateGpuiEvent(
      { type: "mouseMove", x, y },
      { target: eventTarget, timeoutMs },
    );
    const click = await this.simulateGpuiEvent(
      { type: "mouseClick", button, x, y },
      { target: eventTarget, timeoutMs },
    );
    return [move, click];
  }

  waitFor(
    condition: Json | string,
    opts: { timeoutMs?: number; pollIntervalMs?: number } = {},
  ): Promise<Json> {
    const timeout = opts.timeoutMs ?? this.defaultTimeoutMs;
    return this.request(
      {
        type: "waitFor",
        condition,
        timeout,
        pollInterval: opts.pollIntervalMs ?? 5,
      },
      { expect: "waitForResult", timeoutMs: timeout + 1000 },
    );
  }

  /** Wait until getState matches the given partial state. */
  waitForState(
    state: Json,
    opts: { timeoutMs?: number; pollIntervalMs?: number } = {},
  ): Promise<Json> {
    return this.waitFor({ type: "stateMatch", state }, opts);
  }

  /**
   * Wait until the observed state stops changing: resolves once `samples`
   * consecutive probes return an identical fingerprint. Use this instead of
   * hardcoded sleeps (the scattered `sleep(1500)` settle hacks) before the
   * first submit after opening a surface — it returns as soon as the surface
   * is actually stable rather than after a guessed delay.
   *
   * Returns { settled, elapsedMs, probes, lastState }. `settled: false`
   * means the timeout elapsed while state was still changing — treat that
   * as a receipt to report, not a silent pass.
   */
  async waitForSettle(
    opts: {
      /** Consecutive identical samples required. Default 3. */
      samples?: number;
      /** Delay between samples. Default 100ms. */
      intervalMs?: number;
      /** Overall deadline. Default 5000ms. */
      timeoutMs?: number;
      /** Custom probe; defaults to getState. Must return comparable JSON. */
      probe?: () => Promise<Json>;
    } = {},
  ): Promise<{
    settled: boolean;
    elapsedMs: number;
    probes: number;
    lastState: Json;
  }> {
    const required = Math.max(2, opts.samples ?? 3);
    const intervalMs = opts.intervalMs ?? 100;
    const timeoutMs = opts.timeoutMs ?? 5000;
    const probe = opts.probe ?? (() => this.getState());
    const start = performance.now();

    let lastFingerprint = "";
    let stableCount = 0;
    let probes = 0;
    let lastState: Json = {};
    while (performance.now() - start < timeoutMs) {
      lastState = await probe();
      probes += 1;
      // Every response carries its own requestId; exclude it (top-level)
      // from the fingerprint or no two probes would ever match.
      const { requestId: _requestId, ...comparable } = lastState;
      const fingerprint = JSON.stringify(comparable);
      stableCount = fingerprint === lastFingerprint ? stableCount + 1 : 1;
      lastFingerprint = fingerprint;
      if (stableCount >= required) {
        return {
          settled: true,
          elapsedMs: Math.round(performance.now() - start),
          probes,
          lastState,
        };
      }
      await Bun.sleep(intervalMs);
    }
    return {
      settled: false,
      elapsedMs: Math.round(performance.now() - start),
      probes,
      lastState,
    };
  }

  /** One round trip: setFilter + wait until the input value is applied. */
  async setFilterAndWait(
    text: string,
    opts: { timeoutMs?: number } = {},
  ): Promise<Json> {
    this.setFilter(text);
    // stdin is processed serially by the app, so by the time waitFor runs
    // the setFilter has already been applied — this usually hits the
    // already-satisfied fast path and returns immediately.
    return this.waitForState({ inputValue: text }, opts);
  }

  batch(
    commands: Json[],
    opts: { stopOnError?: boolean; timeoutMs?: number } = {},
  ): Promise<Json> {
    const timeout = opts.timeoutMs ?? this.defaultTimeoutMs;
    return this.request(
      {
        type: "batch",
        commands,
        options: { stopOnError: opts.stopOnError ?? true, timeout },
      },
      { expect: "batchResult", timeoutMs: timeout + 1000 },
    );
  }

  listAutomationWindows(opts: { timeoutMs?: number } = {}): Promise<Json> {
    return this.request(
      { type: "listAutomationWindows" },
      { expect: "automationWindowListResult", ...opts },
    );
  }

  /**
   * Fetch recent structured log entries from the app's in-process ring
   * buffer (last 500 events). Filters: limit, level (min severity),
   * target (substring), contains (message substring). Lets a probe assert
   * on log content without reading files off disk.
   */
  getLogs(
    filters: {
      limit?: number;
      level?: string;
      target?: string;
      contains?: string;
    } = {},
    opts: { timeoutMs?: number } = {},
  ): Promise<Json> {
    return this.request(
      { type: "getLogs", ...filters },
      { expect: "logsResult", ...opts },
    );
  }

  /**
   * Capture a screenshot of the app (whole main window by default, or a
   * specific automation window via `target`). Returns the screenshotResult
   * message ({ data: base64 PNG, width, height } or { error }). Pass
   * `savePath` to also decode and write the PNG to disk.
   */
  async captureScreenshot(
    opts: {
      hiDpi?: boolean;
      target?: Json;
      savePath?: string;
      timeoutMs?: number;
    } = {},
  ): Promise<Json> {
    const command: Json = { type: "captureScreenshot" };
    if (opts.hiDpi !== undefined) command.hiDpi = opts.hiDpi;
    if (opts.target !== undefined) command.target = opts.target;
    const result = (await this.request(command, {
      expect: "screenshotResult",
      timeoutMs: opts.timeoutMs ?? 10_000,
    })) as { data?: string; error?: string };
    if (opts.savePath && result.data && !result.error) {
      const { writeFileSync } = await import("node:fs");
      writeFileSync(opts.savePath, Buffer.from(result.data, "base64"));
    }
    return result as Json;
  }
}

export class Driver extends ProtocolCore {
  readonly sessionName: string;
  readonly sessionDir: string;
  readonly logPath: string;

  private proc: Subprocess<"pipe", "pipe", "pipe">;
  private logWriter: ReturnType<ReturnType<typeof Bun.file>["writer"]>;
  private readyResolve: (() => void) | null = null;
  private exited = false;
  private exitError: Error | null = null;
  private streamConsumers: Promise<void>[] = [];
  private streamError: Error | null = null;
  private closePromise: Promise<void> | null = null;
  private streamsDrained = false;
  private logWriterClosed = false;

  private constructor(
    proc: Subprocess<"pipe", "pipe", "pipe">,
    opts: Required<
      Pick<DriverOptions, "sessionName" | "sessionDir" | "defaultTimeoutMs">
    >,
  ) {
    super(opts.defaultTimeoutMs, "drv");
    this.proc = proc;
    this.sessionName = opts.sessionName;
    this.sessionDir = opts.sessionDir;
    this.logPath = join(opts.sessionDir, "app.log");
    this.logWriter = Bun.file(this.logPath).writer();
  }

  /** Attach to a running session.sh session instead of launching a process. */
  static attach(options: AttachOptions = {}): Promise<AttachedDriver> {
    return AttachedDriver.attach(options);
  }

  static async launch(options: DriverOptions = {}): Promise<Driver> {
    assertNoninteractiveDriverLaunch(options);
    const binary = options.binary ?? resolveDefaultBinary();
    if (!existsSync(binary)) {
      throw new Error(
        `Binary not found at ${binary} (candidates checked: ${BINARY_CANDIDATES.join(", ")}). ` +
          `Build one with ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui`,
      );
    }

    // Unique per launch — multiple drivers in one process, and parallel
    // processes reusing the same sessionName, must never share artifacts.
    const launchId = `${process.pid}-${++launchCounter}-${Date.now().toString(36)}`;
    const sessionName = options.sessionName ?? `driver-${launchId}`;
    const sessionDir =
      options.sessionDir ??
      join(
        "/tmp/sk-driver-sessions",
        options.sessionName ? `${sessionName}-${launchId}` : sessionName,
      );
    if (existsSync(sessionDir)) {
      throw new Error(
        `Driver session directory must be fresh and absent: ${sessionDir}`,
      );
    }
    mkdirSync(sessionDir, { recursive: true });

    const env: Record<string, string> = {
      ...(process.env as Record<string, string>),
      SCRIPT_KIT_AI_LOG: "1",
      SCRIPT_KIT_SHORTCUT_DEBUG: "1",
      RUST_LOG: DEFAULT_RUST_LOG,
      SCRIPT_KIT_AGENTIC_SESSION_NAME: sessionName,
      SCRIPT_KIT_AGENTIC_SESSION_GENERATION: `driver-${Date.now()}`,
      ...(options.env ?? {}),
    };
    if (options.protocolBusFile !== false) {
      env.SCRIPT_KIT_AGENTIC_PROTOCOL_RESPONSES_PATH = join(
        sessionDir,
        "protocol-responses.ndjson",
      );
    }
    if (options.sandboxHome) {
      const home = join(sessionDir, "home");
      const kitDir = join(home, ".scriptkit");
      mkdirSync(kitDir, { recursive: true });
      env.HOME = home;
      env.SK_PATH = kitDir;
      // Never inherit an agent runner's CODEX_HOME. The optional auth seeder
      // owns this sandbox path; without seeding it stays unauthenticated.
      env.CODEX_HOME = join(home, ".codex");
      if (options.themeFixturePath) {
        const themeFixturePath = resolve(options.themeFixturePath);
        if (!existsSync(themeFixturePath)) {
          throw new Error(`Theme fixture not found: ${themeFixturePath}`);
        }
        copyFileSync(themeFixturePath, join(kitDir, "theme.json"));
      }
      if (options.sharedModels !== false) {
        // Every model path resolves under $SK_PATH/models (dictation
        // Whisper/Parakeet, brain GGUF). Symlink the real cache so sandboxed
        // launches never re-download 1-2GB per session; downloads triggered
        // inside a sandbox land in the shared cache for future runs.
        const realModels = join(homedir(), ".scriptkit", "models");
        mkdirSync(realModels, { recursive: true });
        symlinkSync(realModels, join(kitDir, "models"));
      }
      if (options.seedAgentAuth) {
        const seed = Bun.spawnSync(
          [
            "bash",
            join(PROJECT_ROOT, "scripts/agentic/seed-sandbox-home.sh"),
            home,
          ],
          { stdout: "pipe", stderr: "pipe" },
        );
        if (seed.exitCode !== 0) {
          throw new Error(
            `seed-sandbox-home failed (exit ${seed.exitCode}): ${seed.stderr.toString().trim()}`,
          );
        }
        console.error(`[driver] ${seed.stdout.toString().trim()}`);
      }
    }

    const proc = Bun.spawn([binary], {
      cwd: PROJECT_ROOT,
      env,
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
    });

    const driver = new Driver(proc, {
      sessionName,
      sessionDir,
      defaultTimeoutMs: options.defaultTimeoutMs ?? 5000,
    });

    const readyPromise = new Promise<void>((resolveReady) => {
      driver.readyResolve = resolveReady;
    });
    const consume = (stream: ReadableStream<Uint8Array>, isStdout: boolean) =>
      driver.consumeStream(stream, isStdout).catch((error) => {
        driver.streamError =
          error instanceof Error ? error : new Error(String(error));
      });
    driver.streamConsumers = [
      consume(proc.stdout, true),
      consume(proc.stderr, false),
    ];
    proc.exited.then((code) => {
      driver.exited = true;
      driver.exitError = new Error(
        `App process exited (code ${code}) — see ${driver.logPath}`,
      );
      driver.failAllPending(driver.exitError);
      driver.readyResolve?.();
    });

    const readyStart = performance.now();
    const readyTimeoutMs = options.readyTimeoutMs ?? 10_000;
    const timedOut = await Promise.race([
      readyPromise.then(() => false),
      Bun.sleep(readyTimeoutMs).then(() => true),
    ]);
    driver.stats.readyWaitMs = Math.round(performance.now() - readyStart);
    if (driver.exited) {
      const startupError =
        driver.exitError ?? new Error("App process exited during startup");
      await driver.close();
      throw startupError;
    }
    if (timedOut) {
      // Marker not seen — fall back to a protocol probe before giving up.
      try {
        await driver.request({ type: "getState" }, { timeoutMs: 2000 });
      } catch {
        const startupError = new Error(
          `App did not become ready within ${readyTimeoutMs}ms — see ${driver.logPath}`,
        );
        await driver.close();
        throw startupError;
      }
    }
    return driver;
  }

  /**
   * Inject text through the production clipboard capture pipeline without
   * touching NSPasteboard. The payload file stays under the sandbox SK_PATH,
   * so even the 100,001-byte rejection boundary fits beneath the JSONL cap.
   */
  async injectClipboardCaptureFixture(args: {
    text: string;
    sourceBundleId?: string;
    concealedTypes?: string[];
    changeGeneration: number;
    timeoutMs?: number;
  }): Promise<Json> {
    const fixtureDir = join(this.sessionDir, "home", ".scriptkit", "devtools-fixtures");
    mkdirSync(fixtureDir, { recursive: true });
    const fixturePath = join(
      fixtureDir,
      `clipboard-${process.pid}-${Date.now()}-${args.changeGeneration}.txt`,
    );
    writeFileSync(fixturePath, args.text, { encoding: "utf8", mode: 0o600 });
    try {
      return await this.request(
        clipboardCaptureFixtureCommand({
          payloadPath: fixturePath,
          sourceBundleId: args.sourceBundleId,
          concealedTypes: args.concealedTypes,
          changeGeneration: args.changeGeneration,
        }),
        {
          expect: "externalCommandResult",
          timeoutMs: args.timeoutMs ?? this.defaultTimeoutMs,
        },
      );
    } finally {
      try {
        unlinkSync(fixturePath);
      } catch {
        // Run-scoped DB/log receipts are authoritative; payload is ephemeral.
      }
    }
  }

  // --- transport -------------------------------------------------------------

  protected writeCommand(payload: Json): void {
    if (this.exited) {
      throw this.exitError ?? new Error("App process has exited");
    }
    this.proc.stdin.write(`${JSON.stringify(payload)}\n`);
    this.proc.stdin.flush();
  }

  // --- lifecycle ---------------------------------------------------------------

  get alive(): boolean {
    return !this.exited;
  }

  /** OS pid of the app process (for `sample`/profiling). */
  get pid(): number | undefined {
    return this.proc.pid;
  }

  get finalization(): {
    processExited: boolean;
    streamsDrained: boolean;
    logWriterClosed: boolean;
  } {
    return {
      processExited: this.exited,
      streamsDrained: this.streamsDrained,
      logWriterClosed: this.logWriterClosed,
    };
  }

  close(): Promise<void> {
    this.closePromise ??= this.closeInternal();
    return this.closePromise;
  }

  private async closeInternal(): Promise<void> {
    this.failAllPending(new Error("Driver closed"));
    if (!this.exited) {
      try {
        this.proc.kill();
      } catch {
        // already gone
      }
      await Promise.race([this.proc.exited, Bun.sleep(2000)]);
      if (!this.exited) {
        try {
          this.proc.kill(9);
        } catch {
          // already gone
        }
        await Promise.race([this.proc.exited, Bun.sleep(1000)]);
      }
    }
    if (!this.exited) {
      throw new Error(
        `Driver process ${this.proc.pid} survived TERM/KILL finalization`,
      );
    }
    const outcomes = await Promise.allSettled(this.streamConsumers);
    const rejected = outcomes.find(
      (outcome): outcome is PromiseRejectedResult =>
        outcome.status === "rejected",
    );
    this.streamsDrained = rejected === undefined && this.streamError === null;
    await this.logWriter.flush();
    await this.logWriter.end();
    this.logWriterClosed = true;
    const streamFailure = this.streamError ?? rejected?.reason;
    if (streamFailure) {
      throw streamFailure instanceof Error
        ? streamFailure
        : new Error(String(streamFailure));
    }
  }

  // --- internals -----------------------------------------------------------------

  private async consumeStream(
    stream: ReadableStream<Uint8Array>,
    isStdout: boolean,
  ): Promise<void> {
    const decoder = new TextDecoder();
    let buffer = "";
    for await (const chunk of stream) {
      buffer += decoder.decode(chunk, { stream: true });
      let newlineIndex = buffer.indexOf("\n");
      while (newlineIndex >= 0) {
        const line = buffer.slice(0, newlineIndex);
        buffer = buffer.slice(newlineIndex + 1);
        this.handleLine(line, isStdout);
        newlineIndex = buffer.indexOf("\n");
      }
    }
    buffer += decoder.decode();
    if (buffer.length > 0) {
      this.handleLine(buffer, isStdout);
    }
  }

  private handleLine(line: string, isStdout: boolean): void {
    this.logWriter.write(`${line}\n`);

    if (
      this.readyResolve &&
      (line.includes(READY_MARKER_STARTUP) || line.includes(READY_MARKER_APP))
    ) {
      const resolveReady = this.readyResolve;
      this.readyResolve = null;
      resolveReady();
    }

    if (!isStdout) return;
    const trimmed = line.trimStart();
    if (!trimmed.startsWith("{")) return;

    let parsed: Json;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      return;
    }
    this.handleResponse(parsed);
  }
}

/**
 * Client attached to a running session.sh session: writes to the session
 * input FIFO under the session command.lock, tails the session's
 * protocol-responses.ndjson for matching responses. Never kills the app.
 */
export class AttachedDriver extends ProtocolCore {
  readonly sessionName: string;
  readonly sessionDir: string;
  readonly responsesPath: string;

  private fifoPath: string;
  private readOffset = 0;
  private watcher: FSWatcher | null = null;
  private pollTimer: ReturnType<typeof setInterval> | null = null;
  private closed = false;
  private lineBuffer = "";

  private constructor(opts: {
    sessionName: string;
    sessionDir: string;
    defaultTimeoutMs: number;
  }) {
    super(opts.defaultTimeoutMs, "atd");
    this.sessionName = opts.sessionName;
    this.sessionDir = opts.sessionDir;
    this.fifoPath = join(opts.sessionDir, "input");
    this.responsesPath = join(opts.sessionDir, "protocol-responses.ndjson");
  }

  static async attach(options: AttachOptions = {}): Promise<AttachedDriver> {
    const sessionName = options.session ?? "default";
    const root =
      options.sessionsRoot ??
      process.env.SCRIPT_KIT_SESSION_DIR ??
      "/tmp/sk-agentic-sessions";
    const sessionDir = join(root, sessionName);
    const fifoPath = join(sessionDir, "input");
    const pidPath = join(sessionDir, "pid");

    if (!existsSync(sessionDir) || !existsSync(fifoPath)) {
      throw new Error(
        `No running session '${sessionName}' under ${root} — start one with: bash scripts/agentic/session.sh start ${sessionName}`,
      );
    }
    const pid = Number(readFileSync(pidPath, "utf8").trim() || "0");
    if (!pid || !processAlive(pid)) {
      throw new Error(
        `Session '${sessionName}' app process (pid ${pid || "unknown"}) is not running — restart with: bash scripts/agentic/session.sh start ${sessionName}`,
      );
    }

    const attached = new AttachedDriver({
      sessionName,
      sessionDir,
      defaultTimeoutMs: options.defaultTimeoutMs ?? 5000,
    });
    // Start tailing at current EOF: earlier responses belong to other clients.
    try {
      attached.readOffset = statSync(attached.responsesPath).size;
    } catch {
      attached.readOffset = 0;
    }
    attached.startTail(options.pollIntervalMs ?? 100);

    if (options.verify !== false) {
      const readyStart = performance.now();
      try {
        await attached.request(
          { type: "getState" },
          { timeoutMs: options.defaultTimeoutMs ?? 5000 },
        );
      } catch (error) {
        await attached.close();
        throw new Error(
          `Attached to session '${sessionName}' but getState probe failed (${error instanceof Error ? error.message : error}). ` +
            `The session may be wedged — check bash scripts/agentic/session.sh health ${sessionName}`,
        );
      }
      attached.stats.readyWaitMs = Math.round(performance.now() - readyStart);
    }
    return attached;
  }

  // --- transport -------------------------------------------------------------

  protected writeCommand(payload: Json): void {
    assertNoninteractiveUnownedSessionCommand(payload, "AttachedDriver");
    if (this.closed) {
      throw new Error("AttachedDriver closed");
    }
    const line = `${JSON.stringify(payload)}\n`;
    // Honor the same per-session command lock session.sh rpc/send use so
    // concurrent writers never interleave partial lines in the FIFO.
    const lockDir = join(this.sessionDir, "command.lock");
    const deadline = performance.now() + 2000;
    let locked = false;
    while (performance.now() < deadline) {
      try {
        mkdirSync(lockDir);
        locked = true;
        break;
      } catch {
        // busy — spin briefly; lock holders release in well under 2s
        Bun.sleepSync(10);
      }
    }
    if (!locked) {
      throw new Error(`Timed out acquiring session command lock at ${lockDir}`);
    }
    try {
      appendFileSync(this.fifoPath, line);
    } finally {
      try {
        rmdirSync(lockDir);
      } catch {
        // released elsewhere
      }
    }
  }

  private startTail(pollIntervalMs: number): void {
    const drain = () => this.drainResponses();
    try {
      this.watcher = watch(this.responsesPath, { persistent: false }, drain);
    } catch {
      // File may not exist yet; the poll below will pick it up and we retry
      // the watcher on each poll tick until it attaches.
    }
    this.pollTimer = setInterval(() => {
      if (!this.watcher) {
        try {
          this.watcher = watch(
            this.responsesPath,
            { persistent: false },
            drain,
          );
        } catch {
          // still missing
        }
      }
      drain();
    }, pollIntervalMs);
    this.pollTimer.unref?.();
  }

  private drainResponses(): void {
    if (this.closed) return;
    let size: number;
    try {
      size = statSync(this.responsesPath).size;
    } catch {
      return;
    }
    if (size < this.readOffset) {
      // File rotated/truncated — start over from the top.
      this.readOffset = 0;
      this.lineBuffer = "";
    }
    if (size === this.readOffset) return;

    const length = size - this.readOffset;
    const buffer = Buffer.alloc(length);
    let fd: number;
    try {
      fd = openSync(this.responsesPath, "r");
    } catch {
      return;
    }
    try {
      const read = readSync(fd, buffer, 0, length, this.readOffset);
      this.readOffset += read;
      this.lineBuffer += buffer.subarray(0, read).toString("utf8");
    } finally {
      closeSync(fd);
    }

    let newlineIndex = this.lineBuffer.indexOf("\n");
    while (newlineIndex >= 0) {
      const line = this.lineBuffer.slice(0, newlineIndex).trim();
      this.lineBuffer = this.lineBuffer.slice(newlineIndex + 1);
      if (line.startsWith("{")) {
        try {
          this.handleResponse(JSON.parse(line));
        } catch {
          // partial/garbled line — skip
        }
      }
      newlineIndex = this.lineBuffer.indexOf("\n");
    }
  }

  // --- lifecycle ---------------------------------------------------------------

  get alive(): boolean {
    if (this.closed) return false;
    try {
      const pid = Number(
        readFileSync(join(this.sessionDir, "pid"), "utf8").trim() || "0",
      );
      return Boolean(pid) && processAlive(pid);
    } catch {
      return false;
    }
  }

  /** Detach only — the session and app keep running. */
  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    this.failAllPending(new Error("AttachedDriver closed"));
    this.watcher?.close();
    this.watcher = null;
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }
}

function processAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

// --- CLI smoke checks -------------------------------------------------------------

if (import.meta.main) {
  const mode = process.argv[2] ?? "smoke";
  if (mode === "smoke") {
    const started = performance.now();
    const driver = await Driver.launch({ sandboxHome: true });
    const launchedMs = Math.round(performance.now() - started);

    const rpcStart = performance.now();
    const state = await driver.getState();
    const stateMs = Math.round(performance.now() - rpcStart);

    const filterStart = performance.now();
    await driver.setFilterAndWait("smoke");
    const filterMs = Math.round(performance.now() - filterStart);

    await driver.close();
    console.log(
      JSON.stringify(
        {
          schemaVersion: 1,
          status: "ok",
          launchMs: launchedMs,
          readyWaitMs: driver.stats.readyWaitMs,
          getStateMs: stateMs,
          setFilterAndWaitMs: filterMs,
          promptType: state.promptType ?? null,
          inputValueAfterFilter: "smoke",
          stats: driver.stats,
          log: driver.logPath,
        },
        null,
        2,
      ),
    );
  } else if (mode === "attach-smoke") {
    const session = process.argv[3] ?? "default";
    const started = performance.now();
    const attached = await Driver.attach({ session });
    const attachMs = Math.round(performance.now() - started);

    const rpcStart = performance.now();
    const state = await attached.getState();
    const stateMs = Math.round(performance.now() - rpcStart);

    const secondStart = performance.now();
    await attached.getState();
    const secondStateMs = Math.round(performance.now() - secondStart);

    await attached.close();
    console.log(
      JSON.stringify(
        {
          schemaVersion: 1,
          status: "ok",
          session,
          attachMs,
          readyWaitMs: attached.stats.readyWaitMs,
          getStateMs: stateMs,
          secondGetStateMs: secondStateMs,
          promptType: state.promptType ?? null,
          stats: attached.stats,
          responsesPath: attached.responsesPath,
        },
        null,
        2,
      ),
    );
  } else {
    console.error(
      "Usage: bun scripts/devtools/driver.ts smoke | attach-smoke [session]",
    );
    process.exit(2);
  }
}
