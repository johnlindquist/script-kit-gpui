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
import { randomUUID } from "node:crypto";
import { inflateSync } from "node:zlib";
import { spawnOwnedProcess, isNativeLifecycleCandidate, validateNativeLifecycle, type NativeLifecycleObservation, type OwnedProcess } from "../agentic/owned-process.ts";
import { beginManagedTask, adoptSupervisorTask, updateManagedTask, finalizeManagedTask, createOwnedStagingDirectory, claimOutput, validateOutputTarget,
  type ManagedTask, type OwnedCleanup } from "../agentic/artifact-lifecycle.ts";
import { verifyImmutableArtifact, type ArtifactReference, type VerifiedArtifact } from "../agentic/build-artifact.ts";
import {
  clipboardCaptureFixtureCommand,
  gpuiKeyDownCommand,
  triggerActionCommand,
} from "./lib/client.ts";
import {
  assertNoninteractiveDriverLaunch,
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveUnownedSessionCommand,
  consumeOwnedEvaluationPermit, assertOwnedEvaluationCommand, ownedEvaluationEnvironment,
  OWNED_EVALUATION_GUARDS, type OwnedEvaluationPermit, type OwnedEvaluationFacts,
} from "./lib/operator-safety.ts";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
/** Ordinary human development is deterministic; proof requires an explicit reference. */
const BINARY_CANDIDATES = [join(PROJECT_ROOT, "target/debug/script-kit-gpui")];
function resolveDefaultBinary(): string {
  return process.env.SCRIPT_KIT_GPUI_BINARY ?? BINARY_CANDIDATES[0]!;
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
   * Path to the ordinary app binary. Defaults to SCRIPT_KIT_GPUI_BINARY,
   * otherwise target/debug. Evaluator launches only use a verified reference.
   */
  binary?: string;
  immutableArtifact?: ArtifactReference;
  ownedEvaluation?: OwnedEvaluationPermit;
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

export const PROTOCOL_VERSION = 2;
export const MAX_PROTOCOL_REQUEST_BYTES = 16 * 1024;
export const MAX_PROTOCOL_RESPONSE_BYTES = 6 * 1024 * 1024;
export const OWNED_RESPONSE_ENCODING = "zlib-json-base64-v1";
export const OWNED_RESPONSE_CODEC = Object.freeze({
  version: 1, encoding: OWNED_RESPONSE_ENCODING, requestField: "responseEncoding",
  responseType: "encodedResponse", delivery: "always",
  maxDecodedBytes: MAX_PROTOCOL_RESPONSE_BYTES, maxCompressedBytes: 4 * 1024 * 1024,
} as const);
const RESPONSE_ENCODING_FIELDS = ["type", "version", "encoding", "requestId", "protocolVersion", "responseType", "decodedBytes", "compressedBytes", "payload"];
const RESPONSE_BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const RESPONSE_UTF8_DECODER = new TextDecoder("utf-8", { fatal: true });
export const RESPONSE_TYPES: Readonly<Record<string, string>> = {
  getState: "stateResult", getElements: "elementsResult", getLayoutInfo: "layoutInfoResult",
  getLogs: "logsResult", listAutomationWindows: "automationWindowListResult",
  inspectAutomationWindow: "automationInspectResult",
  getAgentChatState: "agent_chatStateResult",
  getAgentChatTestProbe: "agent_chatTestProbeResult", getAiReliabilityState: "aiReliabilityStateResult",
  setAiReliabilityTestFixture: "aiReliabilityTestFixtureResult",
  performAgentChatSetupAction: "agent_chatSetupActionResult",
  waitFor: "waitForResult", batch: "batchResult",
  simulateGpuiEvent: "simulateGpuiEventResult", captureScreenshot: "screenshotResult",
  show: "windowVisibilityAck", hide: "windowVisibilityAck",
  triggerAction: "triggerActionResult", design: "designResult", captureRenderWindow: "captureRenderWindowResult",
  inspectContextPreparation: "contextPreparationProbeResult",
  openFocusedTextAgentChatWithMockData: "focusedTextAgentChatFixtureOpenResult",
  ...Object.fromEntries([
    "simulateKey", "simulateClick", "setFilter", "setInput", "setAgentChatInput", "setAgentChatTestFixture",
    "setAgentChatTranscriptScroll", "agentChatEscape", "openAgentChatKitchenSinkFixture", "openAgentChatDetachedFixture",
    "openFocusedTextAgentChatWithPiData", "openAiWithMockData", "openAi", "simulateMainHotkeyGesture",
    "openMiniAiWithMockData", "triggerBuiltin", "openNotes", "openAbout", "pushDictationResult",
    "setMenuSyntaxFormField", "openDictationOverlayFixture", "openConfirmPrompt", "showAiCommandBar",
    "injectClipboardCaptureFixture", "openCreationFeedback",
  ].map(type => [type, "externalCommandResult"])),
};

export class DriverProtocolError extends Error {
  constructor(readonly code: string, readonly requestId: string | null = null) {
    super(code); this.name = "DriverProtocolError";
  }
}
export class DriverCommandRefused extends DriverProtocolError {}
export class DriverLifecycleError extends Error {
  constructor(message: string, readonly cleanup: OwnedCleanup, options?: ErrorOptions) {
    super(message, options); this.name = "DriverLifecycleError";
  }
}
/** Normalize one response body; legacy records retain identity and bus metadata stays caller-validated. */
export function normalizeProtocolResponse(envelope: Json): Json {
  if (envelope?.type !== OWNED_RESPONSE_CODEC.responseType) return envelope;
  const invalid = (code: string): never => { throw new DriverProtocolError(code, envelope.requestId); };
  const fields = Object.keys(envelope);
  if (fields.length !== RESPONSE_ENCODING_FIELDS.length || fields.some(key => !RESPONSE_ENCODING_FIELDS.includes(key)) ||
      envelope.type !== OWNED_RESPONSE_CODEC.responseType || envelope.version !== OWNED_RESPONSE_CODEC.version ||
      envelope.encoding !== OWNED_RESPONSE_ENCODING || envelope.protocolVersion !== PROTOCOL_VERSION ||
      typeof envelope.requestId !== "string" || !envelope.requestId ||
      typeof envelope.responseType !== "string" || !envelope.responseType || envelope.responseType === OWNED_RESPONSE_CODEC.responseType ||
      !Number.isSafeInteger(envelope.decodedBytes) || envelope.decodedBytes <= 0 || envelope.decodedBytes > OWNED_RESPONSE_CODEC.maxDecodedBytes ||
      !Number.isSafeInteger(envelope.compressedBytes) || envelope.compressedBytes <= 0 || envelope.compressedBytes > OWNED_RESPONSE_CODEC.maxCompressedBytes ||
      typeof envelope.payload !== "string") invalid("response_encoding_invalid_header");
  const payload = envelope.payload as string;
  const padding = (3 - envelope.compressedBytes % 3) % 3;
  // Check canonical padding and unused bits without allocating a second full base64 string.
  if (payload.length !== Math.ceil(envelope.compressedBytes / 3) * 4 || /[^A-Za-z0-9+/=]/.test(payload) ||
      payload.indexOf("=") !== (padding ? payload.length - padding : -1) ||
      (padding !== 0 && (!payload.endsWith(padding === 2 ? "==" : "=") ||
        (RESPONSE_BASE64_ALPHABET.indexOf(payload[payload.length - padding - 1]!) & (padding === 2 ? 15 : 3)) !== 0)))
    invalid("response_encoding_invalid_base64");
  const compressed = Buffer.from(payload, "base64");
  if (compressed.length !== envelope.compressedBytes) invalid("response_encoding_invalid_base64");
  let decoded: Buffer;
  try {
    // Node's declarations omit the info:true return shape. bytesWritten rejects trailing streams/data.
    const inflated = inflateSync(compressed, { info: true, maxOutputLength: envelope.decodedBytes }) as unknown as {
      buffer: Buffer; engine: { bytesWritten: number };
    };
    if (!Buffer.isBuffer(inflated.buffer) || inflated.engine?.bytesWritten !== compressed.length)
      invalid("response_encoding_invalid_stream");
    decoded = inflated.buffer;
  } catch { return invalid("response_encoding_invalid_stream"); }
  if (decoded.length !== envelope.decodedBytes) return invalid("response_encoding_length_mismatch");
  let response: Json;
  try { response = JSON.parse(RESPONSE_UTF8_DECODER.decode(decoded)); }
  catch { return invalid("response_encoding_invalid_json"); }
  if (!response || typeof response !== "object" || Array.isArray(response) ||
      response.requestId !== envelope.requestId || response.protocolVersion !== envelope.protocolVersion ||
      response.type !== envelope.responseType) return invalid("response_encoding_identity_mismatch");
  return response;
}

export async function boundedObservation<T>(promise: Promise<T>, timeoutMs: number): Promise<
  { completed: true; value: T } | { completed: false; error: unknown }
> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise.then(value => ({ completed: true as const, value }), error => ({ completed: false as const, error })),
      new Promise<{ completed: false; error: Error }>(resolveTimeout => {
        timer = setTimeout(() => resolveTimeout({ completed: false, error: new Error("observation_deadline") }), timeoutMs);
      }),
    ]);
  } finally { if (timer !== undefined) clearTimeout(timer); }
}

export function unknownOwnedCleanup(acquired: boolean): OwnedCleanup {
  return { resourcesAcquired: acquired, processExited: !acquired, processGroupExited: !acquired,
    streamsDrained: !acquired, logWriterClosed: !acquired, ownedWindowsClosed: acquired ? null : true,
    referencesFinalized: !acquired, closed: !acquired,
    survivors: acquired ? [{ kind: "process-group", identity: "unknown", observation: "unknown" }] : [],
    failureCodes: acquired ? ["cleanup_unobserved"] : [] };
}

interface Pending {
  resolve: (value: Json) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
  expectedType: string;
  protocolVersion: number;
  responseEncoding?: typeof OWNED_RESPONSE_ENCODING;
  cancelCommand?: Json;
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
  readonly protocolFaults: string[] = [];
  private readonly issuedRequestIds = new Set<string>();
  private readonly correlationNonce = randomUUID();
  protected requestLimit = 100_000;
  private responseEncoding?: typeof OWNED_RESPONSE_ENCODING;

  /** Call only after the native catalog advertises the exact bounded codec contract. */
  enableResponseEncoding(encoding: typeof OWNED_RESPONSE_ENCODING): void {
    if (encoding !== OWNED_RESPONSE_ENCODING) throw new DriverProtocolError("response_encoding_invalid");
    this.responseEncoding = encoding;
  }

  protected authorizeCommand(command: Json): void { assertNoninteractiveProtocolCommand(command); }
  protected onTransportFailure(_error: Error): void {}
  protected fault(code: string): void {
    if (this.protocolFaults.length < 128) this.protocolFaults.push(code);
  }

  protected onNativeLifecycle(_envelope: Json): void { this.fault("unexpected_native_lifecycle"); }

  private payload(command: Json): Json {
    this.authorizeCommand(command);
    const protocolVersion = command.protocolVersion ?? PROTOCOL_VERSION;
    if (protocolVersion !== 1 && protocolVersion !== PROTOCOL_VERSION)
      throw new DriverProtocolError("unsupported_protocol_version");
    const explicitEncoding = Object.hasOwn(command, "responseEncoding");
    const responseEncoding = explicitEncoding ? command.responseEncoding : this.responseEncoding;
    if ((explicitEncoding || responseEncoding !== undefined) && responseEncoding !== OWNED_RESPONSE_ENCODING)
      throw new DriverProtocolError("response_encoding_invalid");
    const payload = { ...command, protocolVersion, ...(responseEncoding === undefined ? {} : { responseEncoding }) };
    if (Buffer.byteLength(JSON.stringify(payload)) + 1 > MAX_PROTOCOL_REQUEST_BYTES)
      throw new DriverProtocolError("stdin_line_too_long");
    return payload;
  }

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
    this.writeCommand(this.payload(command));
  }

  /** Correlate one unique request with its exact terminal type and protocol version. */
  request(command: Json, opts: { expect?: string; timeoutMs?: number } = {}): Promise<Json> {
    const known = RESPONSE_TYPES[String(command.type)];
    const expectedType = opts.expect ?? known;
    if (!expectedType) throw new DriverProtocolError("expected_response_type_required");
    if (known && opts.expect && known !== opts.expect)
      throw new DriverProtocolError("expected_response_type_conflicts_with_protocol");
    if (command.requestId !== undefined && (typeof command.requestId !== "string" || !command.requestId || command.requestId.length > 192))
      throw new DriverProtocolError("invalid_request_id");
    const requestId: string = command.requestId ?? `${this.requestIdPrefix}-${this.correlationNonce}-${++this.requestCounter}`;
    if (this.issuedRequestIds.has(requestId)) throw new DriverProtocolError("request_id_reused", requestId);
    if (this.issuedRequestIds.size >= this.requestLimit) throw new DriverProtocolError("request_budget_exhausted");
    const timeoutMs = opts.timeoutMs ?? this.defaultTimeoutMs;
    if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0 || timeoutMs > 600_000)
      throw new DriverProtocolError("invalid_request_timeout", requestId);
    let deadlineUnixMs: number | undefined;
    if (command.type === "simulateGpuiEvent") {
      deadlineUnixMs = Math.min(command.deadlineUnixMs ?? Number.MAX_SAFE_INTEGER, Date.now() + timeoutMs);
      if (!Number.isSafeInteger(deadlineUnixMs) || deadlineUnixMs < 0)
        throw new DriverProtocolError("invalid_dispatch_deadline", requestId);
    }
    const payload = this.payload({ ...command, requestId, ...(deadlineUnixMs === undefined ? {} : { deadlineUnixMs }) });
    this.issuedRequestIds.add(requestId);
    return new Promise<Json>((resolvePromise, rejectPromise) => {
      const pending: Pending = {
        resolve: resolvePromise, reject: rejectPromise, expectedType, protocolVersion: PROTOCOL_VERSION,
        responseEncoding: payload.responseEncoding,
        cancelCommand: command.type === "simulateGpuiEvent"
          ? { type: "cancelGpuiEvent", requestId, protocolVersion: PROTOCOL_VERSION } : undefined,
        timer: setTimeout(() => {
          if (this.pending.get(requestId) !== pending) return;
          this.pending.delete(requestId);
          // Cancellation only revokes an already-authorized action. It grants no input authority.
          if (pending.cancelCommand) {
            try { this.writeCommand(pending.cancelCommand); } catch { /* transport already unavailable */ }
          }
          const error = new DriverProtocolError("response_timeout", requestId);
          this.onTransportFailure(error);
          rejectPromise(error);
        }, timeoutMs),
      };
      this.pending.set(requestId, pending);
      this.stats.requestsSent += 1;
      try { this.writeCommand(payload); }
      catch (cause) {
        clearTimeout(pending.timer); this.pending.delete(requestId);
        const error = cause instanceof Error ? cause : new Error(String(cause));
        this.onTransportFailure(error); rejectPromise(error);
      }
    });
  }

  protected handleResponse(envelope: Json): void {
    if (!envelope || typeof envelope !== "object" || Array.isArray(envelope)) { this.fault("non_object_response"); return; }
    let parsed = envelope;
    if (Object.hasOwn(envelope, "response")) {
      if (isNativeLifecycleCandidate(envelope.response)) { this.onNativeLifecycle(envelope); return; }
      const nested = envelope.response;
      if (!nested || typeof nested !== "object" || Array.isArray(nested) ||
          envelope.requestId !== nested.requestId ||
          envelope.responseType !== (nested.type === OWNED_RESPONSE_CODEC.responseType ? nested.responseType : nested.type) ||
          envelope.protocolVersion !== nested.protocolVersion) {
        this.fault("nested_response_identity_mismatch"); return;
      }
      parsed = nested;
    }
    if (isNativeLifecycleCandidate(parsed)) { this.onNativeLifecycle(parsed); return; }
    const requestId = parsed.requestId;
    if (typeof requestId !== "string" || !requestId) { this.fault("missing_response_request_id"); return; }
    const pending = this.pending.get(requestId);
    if (!pending) { this.stats.unmatchedResponses += 1; return; }
    try {
      if (parsed.type === OWNED_RESPONSE_CODEC.responseType) {
        if (pending.responseEncoding !== OWNED_RESPONSE_ENCODING) throw new DriverProtocolError("response_encoding_unrequested", requestId);
        parsed = normalizeProtocolResponse(parsed);
      } else if (pending.responseEncoding !== undefined) throw new DriverProtocolError("response_encoding_missing", requestId);
    } catch (cause) {
      const error = cause instanceof DriverProtocolError ? cause : new DriverProtocolError("response_encoding_invalid", requestId);
      this.fault(error.code);
      this.pending.delete(requestId); clearTimeout(pending.timer); pending.reject(error);
      this.onTransportFailure(error);
      return;
    }
    if (isNativeLifecycleCandidate(parsed)) { this.onNativeLifecycle(parsed); return; }
    if (parsed.protocolVersion !== pending.protocolVersion) { this.fault("response_protocol_version_mismatch"); return; }
    const refusalCode = parsed.type === "externalCommandResult" && parsed.ok === false && typeof parsed.errorCode === "string"
      ? parsed.errorCode
      : parsed.type === "error" && typeof parsed.code === "string" && typeof parsed.message === "string"
        ? parsed.code
        : undefined;
    if (parsed.type !== pending.expectedType && refusalCode === undefined) { this.fault("wrong_response_type"); return; }
    // An acceptance ticket is never terminal proof. A later deferred completion may settle this request.
    if (parsed.type === "simulateGpuiEventResult") {
      if (parsed.dispatchScheduled === true) {
        if (parsed.dispatchCompleted === true) this.fault("invalid_dispatch_completion");
        return;
      }
      if (parsed.success === true) {
        if (parsed.dispatchCompleted !== true || parsed.dispatchScheduled !== false ||
            typeof parsed.wasDeferred !== "boolean" || parsed.activationProof !== "not_observed") {
          this.fault("invalid_dispatch_completion"); return;
        }
      } else if (parsed.success !== false || parsed.dispatchCompleted !== false ||
          parsed.dispatchScheduled !== false || typeof parsed.errorCode !== "string") {
        this.fault("invalid_dispatch_completion"); return;
      }
    }
    this.pending.delete(requestId); clearTimeout(pending.timer);
    this.stats.responsesMatched += 1;
    if (this.matchedResponses.length < this.requestLimit)
      this.matchedResponses.push({ requestId, expectedType: pending.expectedType, responseType: parsed.type });
    if (refusalCode !== undefined) { pending.reject(new DriverCommandRefused(refusalCode, requestId)); return; }
    pending.resolve(parsed);
  }

  protected failAllPending(error: Error): void {
    for (const [, pending] of this.pending) {
      if (pending.cancelCommand) {
        try { this.writeCommand(pending.cancelCommand); } catch { /* transport already unavailable */ }
      }
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

  private proc: OwnedProcess;
  private logWriter: Bun.FileSink;
  private readyResolve: (() => void) | null = null;
  private exited = false;
  private exitError: Error | null = null;
  private streamConsumers: Promise<void>[] = [];
  private streamError: Error | null = null;
  private closePromise: Promise<void> | null = null;
  private streamsDrained = false;
  private logWriterClosed = false;
  private readers: ReadableStreamDefaultReader<Uint8Array>[] = [];
  private logBytes = 0;
  private maxLogBytes = 8_388_608;
  private permit?: OwnedEvaluationPermit;
  private facts?: OwnedEvaluationFacts;
  private task?: ManagedTask;
  private cleanup: OwnedCleanup = unknownOwnedCleanup(true);
  private windowsClosed: boolean | null = null;
  private nativeObservation: NativeLifecycleObservation | null = null;
  private nativeLifecycleFailure = false;
  private inputClosed = false;
  private readonly lifecycleSignal = Promise.withResolvers<void>();
  verifiedArtifact?: VerifiedArtifact;
  qualification: Json | null = null;

  private constructor(
    proc: OwnedProcess,
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

  get observedReceivedOutputBytes(): number { return this.proc.observedReceivedOutputBytes; }
  get maxOutputBytes(): number { return this.proc.maxOutputBytes; }

  /** Attach to a running session.sh session instead of launching a process. */
  static attach(options: AttachOptions = {}): Promise<AttachedDriver> {
    return AttachedDriver.attach(options);
  }

  static async launch(options: DriverOptions = {}): Promise<Driver> {
    const facts = options.ownedEvaluation ? consumeOwnedEvaluationPermit(options.ownedEvaluation) : undefined;
    if (facts && (options.binary || options.env || options.sessionDir || options.themeFixturePath || options.seedAgentAuth || options.sharedModels))
      throw new DriverProtocolError("owned_evaluation_launch_override");
    if (!facts) assertNoninteractiveDriverLaunch(options);
    const artifact = facts?.artifact ?? (options.immutableArtifact ? verifyImmutableArtifact(PROJECT_ROOT, options.immutableArtifact,
      { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" }) : undefined);
    if (facts && options.immutableArtifact && (options.immutableArtifact.manifestPath !== facts.artifact.reference.manifestPath ||
      options.immutableArtifact.manifestSha256 !== facts.artifact.reference.manifestSha256))
      throw new DriverProtocolError("permit_artifact_mismatch");
    if (artifact && options.binary && resolve(options.binary) !== artifact.executablePath)
      throw new DriverProtocolError("artifact_binary_override");
    const binary = artifact?.executablePath ?? options.binary ?? resolveDefaultBinary();
    if (!existsSync(binary)) throw new Error(`Binary not found: ${binary}`);
    const launchId = `${process.pid}-${++launchCounter}-${randomUUID()}`;
    const sessionName = options.sessionName ?? `driver-${launchId}`;
    let driver: Driver | undefined;
    let proc: OwnedProcess | undefined;
    let task: ManagedTask | undefined;
    let cleanup = unknownOwnedCleanup(false);
    try {
      const taskClaim = artifact ? claimOutput(validateOutputTarget({ repoRoot: PROJECT_ROOT,
        candidate: facts ? join(facts.claim.root, `runtime-${launchId}`) : join("/tmp/sk-driver-sessions", `runtime-${launchId}`),
        kind: "directory", probeId: "driver-runtime" })) : undefined;
      if (taskClaim) task = beginManagedTask(taskClaim, "runtime-run", [artifact!.reference]);
      const sessionDir = facts ? createOwnedStagingDirectory(taskClaim!, { name: "session" }) :
        options.sessionDir ?? join("/tmp/sk-driver-sessions", `${sessionName}-${launchId}`);
      if (!facts) {
        if (existsSync(sessionDir)) throw new Error(`Driver session directory must be fresh: ${sessionDir}`);
        mkdirSync(sessionDir, { recursive: true, mode: 0o700 });
      }
      const env: Record<string, string> = facts ? ownedEvaluationEnvironment(facts, sessionDir) : {
        ...(process.env as Record<string, string>), SCRIPT_KIT_AI_LOG: "1", SCRIPT_KIT_SHORTCUT_DEBUG: "1",
        RUST_LOG: DEFAULT_RUST_LOG, ...(options.env ?? {}),
      };
      env.SCRIPT_KIT_AGENTIC_SESSION_NAME = sessionName;
      // The supervisor installs the authoritative session/process instance identifiers.
      if (!facts && options.protocolBusFile !== false)
        env.SCRIPT_KIT_AGENTIC_PROTOCOL_RESPONSES_PATH = join(sessionDir, "protocol-responses.ndjson");
      if (!facts && options.sandboxHome) {
        const home = join(sessionDir, "home");
        const kitDir = join(home, ".scriptkit");
        mkdirSync(kitDir, { recursive: true, mode: 0o700 });
        env.HOME = home; env.SK_PATH = kitDir; env.CODEX_HOME = join(home, ".codex");
        if (options.themeFixturePath) copyFileSync(resolve(options.themeFixturePath), join(kitDir, "theme.json"));
        if (options.sharedModels !== false) {
          const realModels = join(homedir(), ".scriptkit", "models");
          mkdirSync(realModels, { recursive: true }); symlinkSync(realModels, join(kitDir, "models"));
        }
        if (options.seedAgentAuth) {
          const seed = Bun.spawnSync(["bash", join(PROJECT_ROOT, "scripts/agentic/seed-sandbox-home.sh"), home], { stdout: "pipe", stderr: "pipe" });
          if (seed.exitCode !== 0) throw new Error(`seed-sandbox-home failed: ${seed.stderr.toString()}`);
        }
      }
      proc = await spawnOwnedProcess({ argv: facts ? [binary, "--owned-ui-evaluation"] : [binary], cwd: PROJECT_ROOT,
        env, timeoutMs: facts ? facts.limits.maxLifetimeMs + 3000 : 3_600_000, maxOutputBytes: facts ? 64 * 1024 * 1024 : 128 * 1024 * 1024,
        ...(facts ? { ownedNative: { launchNonce: facts.launchNonce, policySha256: facts.policySha256,
          binarySha256: artifact!.manifest.binarySha256, manifestSha256: artifact!.reference.manifestSha256,
          task: { repositoryRoot: PROJECT_ROOT, recordPath: task!.recordPath, identity: task!.identity, helperExecutable: process.execPath } } } : {}) });
      cleanup = unknownOwnedCleanup(true);
      driver = new Driver(proc, { sessionName, sessionDir, defaultTimeoutMs: options.defaultTimeoutMs ?? 5000 });
      driver.permit = options.ownedEvaluation; driver.facts = facts; driver.task = task; driver.verifiedArtifact = artifact;
      driver.requestLimit = facts?.limits.maxRequests ?? 100_000;
      driver.maxLogBytes = facts?.limits.maxLogBytes ?? 8_388_608;
      if (task) {
        if (facts) adoptSupervisorTask(task, proc.identity);
        updateManagedTask(task, { state: "running", ownedProcesses: [proc.identity], source: artifact!.manifest.source });
      }
      const active = driver;
      const ready = new Promise<void>(resolveReady => { active.readyResolve = resolveReady; });
      const consume = (stream: ReadableStream<Uint8Array>, stdout: boolean) => active.consumeStream(stream, stdout).catch(cause => {
        active.streamError = cause instanceof Error ? cause : new Error(String(cause));
        active.onTransportFailure(active.streamError);
      });
      active.streamConsumers = [consume(proc.stdout, true), consume(proc.stderr, false)];
      void proc.exited.then(async code => {
        active.exited = true; active.exitError = new Error(`App process exited (${code})`);
        // A terminal supervisor frame can overtake the consumer's queued native replies.
        await boundedObservation(Promise.allSettled(active.streamConsumers), 1500);
        active.failAllPending(active.exitError); active.readyResolve?.();
      }, cause => active.onTransportFailure(cause instanceof Error ? cause : new Error(String(cause))));
      const start = performance.now();
      if (facts) {
        const response = await active.request({ type: "design", command: { operation: "bootstrap",
          launchNonce: facts.launchNonce, policySha256: facts.policySha256 } }, { timeoutMs: options.readyTimeoutMs ?? 10_000 });
        const report = response.result;
        const identity = report?.identity;
        if (report?.operation !== "bootstrap" || report.ok !== true || report.launchNonce !== facts.launchNonce ||
          report.policySha256 !== facts.policySha256 || !identity ||
          identity.pid !== proc.pid || identity.processStartTime !== proc.identity.processStartTime ||
          identity.processInstanceId !== proc.identity.processInstanceId || identity.sessionGeneration !== proc.identity.sessionGeneration ||
          identity.binarySha256 !== artifact!.manifest.binarySha256 || identity.manifestSha256 !== artifact!.reference.manifestSha256 ||
          OWNED_EVALUATION_GUARDS.some(guard => report.guards?.[guard] !== true) ||
          Object.entries(facts.limits).some(([key, value]) => report.limits?.[key] !== value))
          throw new DriverProtocolError("owned_evaluation_qualification_mismatch");
        active.qualification = report;
      } else {
        const observation = await boundedObservation(ready, options.readyTimeoutMs ?? 10_000);
        if (!observation.completed) {
          try { await active.request({ type: "getState" }, { timeoutMs: 2000 }); }
          catch (cause) { throw new Error(`App did not become ready within ${options.readyTimeoutMs ?? 10_000}ms`, { cause }); }
        }
      }
      active.stats.readyWaitMs = Math.round(performance.now() - start);
      if (active.exited) throw active.exitError;
      return active;
    } catch (cause) {
      if (driver) { await boundedObservation(driver.close(), 22_000); cleanup = driver.finalization; }
      else if (proc) {
        const observed = await boundedObservation(proc.close(), 18_000);
        cleanup = observed.completed ? observed.value : unknownOwnedCleanup(true);
      } else if (cause && typeof cause === "object" && "cleanup" in cause) cleanup = (cause as DriverLifecycleError).cleanup;
      if (task && !driver) {
        try { cleanup = finalizeManagedTask(task, cleanup).cleanup; }
        catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "task_finalization_failed"] }; }
      }
      throw new DriverLifecycleError("Driver launch failed", cleanup, { cause });
    }
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
    if (this.exited || this.closePromise || this.inputClosed) throw this.exitError ?? new Error("Driver input closed");
    this.proc.stdin.write(`${JSON.stringify(payload)}\n`);
    void this.proc.stdin.flush().catch(cause => this.onTransportFailure(cause instanceof Error ? cause : new Error(String(cause))));
  }

  protected authorizeCommand(command: Json): void {
    if (this.permit) assertOwnedEvaluationCommand(this.permit, command);
    else super.authorizeCommand(command);
  }

  protected onTransportFailure(error: Error): void {
    this.exitError = error;
    this.failAllPending(error);
    void this.close().catch(() => {}); // cleanup evidence remains on finalization
  }

  get processIdentity() { return this.proc.identity; }
  get nativeLifecycle(): NativeLifecycleObservation | null { return this.nativeObservation ? structuredClone(this.nativeObservation) : null; }
  /** Identity projection only; this is not the in-memory task mutation capability. */
  get managedTask(): ManagedTask | null {
    return this.task ? Object.freeze({ recordPath: this.task.recordPath, identity: this.task.identity }) : null;
  }

  protected onNativeLifecycle(envelope: Json): void {
    try {
      if (!this.facts || this.nativeObservation || this.nativeLifecycleFailure) throw new Error("native_lifecycle_unexpected_or_duplicate");
      this.nativeObservation = validateNativeLifecycle(envelope, this.proc.identity, {
        launchNonce: this.facts.launchNonce, policySha256: this.facts.policySha256,
        binarySha256: this.facts.artifact.manifest.binarySha256, manifestSha256: this.facts.artifact.reference.manifestSha256,
      });
      this.windowsClosed = this.nativeObservation.result.ownedWindowsClosed;
    } catch (error) {
      this.nativeLifecycleFailure = true;
      this.windowsClosed = false;
      this.fault(error instanceof Error ? error.message : "native_lifecycle_invalid");
    }
    this.lifecycleSignal.resolve();
  }

  async awaitNativeLifecycle(timeoutMs = 5000): Promise<NativeLifecycleObservation> {
    if (!this.facts) throw new DriverProtocolError("owned_native_launch_required");
    if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 10_000) throw new DriverProtocolError("invalid_lifecycle_timeout");
    const observed = await boundedObservation(this.lifecycleSignal.promise, timeoutMs);
    if (!observed.completed || !this.nativeObservation || this.nativeLifecycleFailure) throw new DriverProtocolError("native_lifecycle_unproved");
    return structuredClone(this.nativeObservation);
  }

  /** Close only the owned child's stdin; supervisor ownership and output draining remain live. */
  async closeInput(timeoutMs = 5000): Promise<NativeLifecycleObservation> {
    if (!this.facts) throw new DriverProtocolError("owned_native_launch_required");
    if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 10_000) throw new DriverProtocolError("invalid_lifecycle_timeout");
    if (!this.inputClosed) { this.inputClosed = true; this.proc.stdin.end(); }
    return this.awaitNativeLifecycle(timeoutMs);
  }

  // --- lifecycle ---------------------------------------------------------------

  get alive(): boolean {
    return !this.exited;
  }

  /** OS pid of the app process (for `sample`/profiling). */
  get pid(): number | undefined {
    return this.proc.pid;
  }

  get finalization(): OwnedCleanup { return this.cleanup; }

  close(): Promise<void> {
    this.closePromise ??= this.closeInternal();
    return this.closePromise;
  }

  private async closeInternal(): Promise<void> {
    this.failAllPending(new Error("Driver closed"));
    const processResult = await boundedObservation(this.proc.close(), 18_000);
    let cleanup = processResult.completed ? processResult.value : unknownOwnedCleanup(true);
    const drain = await boundedObservation(Promise.allSettled(this.streamConsumers), 1500);
    this.streamsDrained = drain.completed && drain.value.every(result => result.status === "fulfilled") && !this.streamError;
    if (!this.streamsDrained) await boundedObservation(Promise.allSettled(this.readers.map(reader => reader.cancel())), 500);
    const flush = await boundedObservation(Promise.resolve().then(() => this.logWriter.flush()), 500);
    const end = await boundedObservation(Promise.resolve().then(() => this.logWriter.end()), 500);
    this.logWriterClosed = flush.completed && end.completed;
    cleanup = { ...cleanup, streamsDrained: cleanup.streamsDrained && this.streamsDrained,
      logWriterClosed: cleanup.logWriterClosed && this.logWriterClosed,
      ownedWindowsClosed: this.facts ? this.windowsClosed : cleanup.ownedWindowsClosed,
      failureCodes: [...cleanup.failureCodes, ...(!this.streamsDrained ? ["streams_not_drained"] : []),
        ...(!this.logWriterClosed ? ["log_not_closed"] : []), ...(this.facts && this.windowsClosed !== true ? ["windows_not_observed_closed"] : []),
        ...(this.nativeLifecycleFailure ? ["native_lifecycle_invalid"] : [])],
    };
    cleanup = { ...cleanup, closed: cleanup.closed && cleanup.streamsDrained && cleanup.logWriterClosed && (!this.facts || this.windowsClosed === true) };
    this.cleanup = cleanup;
    if (this.task) {
      try {
        if (this.nativeObservation) updateManagedTask(this.task, { result: { nativeLifecycle: this.nativeObservation } });
        this.cleanup = finalizeManagedTask(this.task, { ...cleanup, referencesFinalized: this.facts ? cleanup.closed : true }).cleanup;
      }
      catch { this.cleanup = { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "task_finalization_failed"] }; }
    }
    if (!this.cleanup.closed) throw new DriverLifecycleError("INVALID_CLEANUP", this.cleanup);
  }

  // --- internals -----------------------------------------------------------------

  private async consumeStream(stream: ReadableStream<Uint8Array>, isStdout: boolean): Promise<void> {
    const decoder = new TextDecoder();
    const reader = stream.getReader();
    this.readers.push(reader);
    let buffer = "";
    try {
      for (;;) {
        const next = await reader.read();
        if (next.done) break;
        buffer += decoder.decode(next.value, { stream: true });
        let newline = buffer.indexOf("\n");
        while (newline >= 0) {
          this.handleLine(buffer.slice(0, newline), isStdout);
          buffer = buffer.slice(newline + 1); newline = buffer.indexOf("\n");
        }
        if (Buffer.byteLength(buffer) > MAX_PROTOCOL_RESPONSE_BYTES) throw new DriverProtocolError("response_line_too_long");
      }
      buffer += decoder.decode();
      if (buffer.length) this.handleLine(buffer, isStdout);
    } finally { reader.releaseLock(); }
  }

  private handleLine(line: string, isStdout: boolean): void {
    const bytes = Buffer.byteLength(line) + 1;
    if (bytes > MAX_PROTOCOL_RESPONSE_BYTES) throw new DriverProtocolError("response_line_too_long");
    if (this.logBytes + bytes <= this.maxLogBytes) {
      this.logWriter.write(`${line}\n`); this.logBytes += bytes;
    }

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
      throw new DriverProtocolError("malformed_json_response");
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
