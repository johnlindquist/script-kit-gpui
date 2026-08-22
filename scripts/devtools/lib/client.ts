/**
 * scripts/devtools/lib/client.ts — shared transport, arg parsing, and receipt
 * helpers for the one-shot DevTools CLIs.
 *
 * Before this module, ~13 CLIs carried byte-for-byte copies of `run`, `rpc`,
 * `requestId`, `responseOf`, the target-selector arg block, and a hand-rolled
 * `forwarded[]` echo array (see the 2026-07-01 devtools audit). Every CLI also
 * collapsed queue_timeout/parse_error/response_timeout into a misleading
 * "blocked-by-timeout". This module is the single home for that logic.
 */

import { createHash } from "node:crypto";
import { statSync, readFileSync, existsSync } from "node:fs";
import { join, relative } from "node:path";
import {
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveSessionCommand,
} from "./operator-safety.ts";
import { classifyReceiptEvidence } from "./evidence-class.ts";
import { classifyTransportError } from "./transport-errors.ts";

export type JsonObject = Record<string, unknown>;

export const DEFAULT_RPC_TIMEOUT_MS = 8000;

export function sessionsRoot(): string {
  return process.env.SCRIPT_KIT_SESSION_DIR ?? "/tmp/sk-agentic-sessions";
}

export function sessionDir(name: string): string {
  return join(sessionsRoot(), name);
}

export function uniqueSessionName(tool: string): string {
  return `${tool}-${process.pid}-${Date.now().toString(36)}-${Math.random().toString(16).slice(2, 6)}`;
}

export function requestId(tool: string, prefix: string): string {
  return `devtools-${tool}-${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

export interface ClipboardCaptureFixtureCommandArgs {
  payloadPath: string;
  sourceBundleId?: string;
  concealedTypes?: string[];
  changeGeneration: number;
}

/** Build the typed, redacted command for a sandbox-file clipboard capture. */
export function clipboardCaptureFixtureCommand(
  args: ClipboardCaptureFixtureCommandArgs,
): JsonObject {
  return {
    type: "injectClipboardCaptureFixture",
    payload: { type: "textFile", path: args.payloadPath },
    sourceBundleId: args.sourceBundleId,
    concealedTypes: args.concealedTypes ?? [],
    changeGeneration: args.changeGeneration,
  };
}

/** Build a real GPUI key-down dispatch rather than legacy simulateKey. */
export function gpuiKeyDownCommand(
  key: string,
  text?: string,
  modifiers: string[] = [],
  target?: JsonObject,
): JsonObject {
  return {
    type: "simulateGpuiEvent",
    ...(target ? { target } : {}),
    event: { type: "keyDown", key, modifiers, ...(text === undefined ? {} : { text }) },
  };
}

/** Build direct shared-action activation by the action id exposed in DevTools state. */
export function triggerActionCommand(actionId: string, host?: string): JsonObject {
  return {
    type: "triggerAction",
    actionId,
    ...(host ? { host } : {}),
  };
}

/**
 * Spawn a subprocess and parse its stdout as JSON. On failure returns a
 * structured error envelope (never throws), preserving any parseable JSON the
 * child printed (`parsedError`) plus its lifecycle payload so callers can
 * classify dead-session states precisely.
 */
export async function run(command: string[], label: string): Promise<JsonObject> {
  assertNoninteractiveSessionCommand(command);
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  let parsed: JsonObject | null = null;
  try {
    parsed = JSON.parse(stdout);
  } catch {
    parsed = null;
  }

  if (exitCode !== 0) {
    return {
      status: "error",
      label,
      exitCode,
      stdout: stdout.trim(),
      stderr: stderr.trim(),
      parsedError: parsed,
      lifecycle: parsed?.lifecycle ?? null,
    };
  }
  if (parsed) {
    return parsed;
  }
  return { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim(), error: "invalid_json_output" };
}

/** One protocol RPC through the session transport (session.sh rpc). */
export async function rpc(
  session: string,
  payload: JsonObject,
  expect: string,
  timeoutMs: number,
): Promise<JsonObject> {
  assertNoninteractiveProtocolCommand(payload);
  return run(
    [
      "bash",
      "scripts/agentic/session.sh",
      "rpc",
      session,
      JSON.stringify(payload),
      "--expect",
      expect,
      "--timeout",
      String(timeoutMs),
    ],
    String(payload.type ?? "rpc"),
  );
}

export function responseOf(envelope: JsonObject): JsonObject {
  return (envelope.response as JsonObject | undefined) ?? envelope;
}

export function asArray(value: unknown): JsonObject[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is JsonObject => typeof entry === "object" && entry !== null)
    : [];
}

// --- error classification ----------------------------------------------------

export const SESSION_LIFECYCLE_CODES = new Set([
  "session_dead",
  "forwarder_dead",
  "no_session",
  "app_process_dead_before_send",
  "app_process_dead_before_rpc",
  "forwarder_dead_before_send",
  "forwarder_dead_before_rpc",
]);

export function errorCode(error: JsonObject): string {
  const direct = (error.error as JsonObject | undefined)?.code;
  if (typeof direct === "string") return direct;
  const parsed = error.parsedError as JsonObject | undefined | null;
  const parsedCode = (parsed?.error as JsonObject | undefined)?.code;
  return typeof parsedCode === "string" ? parsedCode : "";
}

export function hasSessionLifecycleError(errors: JsonObject[]): boolean {
  return errors.some((error) => SESSION_LIFECYCLE_CODES.has(errorCode(error)));
}

/**
 * Classify a failed RPC envelope precisely instead of the historical blanket
 * "blocked-by-timeout": dead sessions, queue/lock contention, parse errors,
 * and genuine response timeouts each get their own classification.
 */
export function classifyEnvelopeError(envelope: JsonObject): string {
  if (envelope.status !== "error") return "ok";
  if (SESSION_LIFECYCLE_CODES.has(errorCode(envelope))) {
    return "blocked-by-session-lifecycle";
  }
  const viaCode = classifyTransportError(
    (envelope.parsedError as JsonObject | undefined | null) ?? envelope,
  );
  return viaCode === "ok" ? "blocked-by-response-timeout" : viaCode;
}

/** First non-ok classification across the given envelopes, else "ok". */
export function classifyEnvelopes(envelopes: JsonObject[]): string {
  for (const envelope of envelopes) {
    const classification = classifyEnvelopeError(envelope);
    if (classification !== "ok") return classification;
  }
  return "ok";
}

export function lifecycleDetails(errors: JsonObject[]) {
  return errors
    .map((error) => {
      const parsed = (error.parsedError as JsonObject | undefined | null) ?? error;
      const code = errorCode(error);
      if (!SESSION_LIFECYCLE_CODES.has(code)) return null;
      return {
        label: error.label ?? null,
        code,
        lifecycle: parsed.lifecycle ?? error.lifecycle ?? null,
        keepActionsWindowOpen: parsed.keepActionsWindowOpen ?? null,
        sessionLifecycle: parsed.sessionLifecycle ?? null,
        message:
          (parsed.error as JsonObject | undefined)?.message ??
          (error.error as JsonObject | undefined)?.message ??
          null,
      };
    })
    .filter(Boolean);
}

export function lifecycleCodes(errors: JsonObject[]): string[] {
  return lifecycleDetails(errors)
    .map((detail) => (detail as JsonObject).code)
    .filter((code): code is string => typeof code === "string");
}

export function primaryLifecycleDetails(errors: JsonObject[]) {
  return (lifecycleDetails(errors)[0] as JsonObject | undefined) ?? null;
}

export function primarySessionLifecycle(errors: JsonObject[]) {
  const details = primaryLifecycleDetails(errors);
  return ((details as JsonObject | null)?.sessionLifecycle as JsonObject | undefined) ?? null;
}

export function primaryParsedError(errors: JsonObject[]) {
  const lifecycleError = errors.find((error) => SESSION_LIFECYCLE_CODES.has(errorCode(error)));
  return (lifecycleError?.parsedError as JsonObject | undefined) ?? null;
}

// --- binary identity ---------------------------------------------------------

export interface BinaryFingerprint {
  path: string;
  sha256: string;
  sizeBytes: number;
  modifiedAt: string;
  sourceCommit: string | null;
  pinned: boolean;
}

function currentGitCommit(): string | null {
  const result = Bun.spawnSync(["git", "rev-parse", "HEAD"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  return result.exitCode === 0
    ? new TextDecoder().decode(result.stdout).trim()
    : null;
}

function receiptSafeBinaryPath(path: string): string {
  const repoRelative = relative(process.cwd(), path);
  return !repoRelative.startsWith("..") && !repoRelative.startsWith("/")
    ? repoRelative
    : `external-sha256:${createHash("sha256").update(path).digest("hex").slice(0, 24)}`;
}

/**
 * Identity of the binary the session is actually running (session.sh records
 * it at <session-dir>/binary on start). Receipts carry this so a "green"
 * verification can prove it ran against the freshly built binary instead of a
 * stale one — the audit's stale-binary trap.
 */
export function binaryFingerprint(session: string): BinaryFingerprint | null {
  try {
    const recorded = join(sessionDir(session), "binary");
    if (!existsSync(recorded)) return null;
    const path = readFileSync(recorded, "utf8").trim();
    const stat = statSync(path);
    return {
      path: receiptSafeBinaryPath(path),
      sha256: createHash("sha256").update(readFileSync(path)).digest("hex"),
      sizeBytes: stat.size,
      modifiedAt: new Date(stat.mtimeMs).toISOString(),
      sourceCommit: currentGitCommit(),
      pinned: Boolean(process.env.SCRIPT_KIT_GPUI_BINARY),
    };
  } catch {
    return null;
  }
}

// --- receipt envelope ----------------------------------------------------------

export interface ReceiptClock {
  startedAt: string;
  t0: number;
}

export function startClock(): ReceiptClock {
  return { startedAt: new Date().toISOString(), t0: performance.now() };
}

/**
 * Wrap a CLI-specific body in the shared receipt envelope: schemaVersion,
 * tool, command, session, timing (startedAt/endedAt/durationMs), and the
 * session's binary fingerprint. Body fields keep their order after the
 * envelope header so existing consumers still find them.
 */
export function finishReceipt(
  meta: { tool: string; command: string; session: string; clock: ReceiptClock },
  body: JsonObject,
): JsonObject {
  const endedAt = new Date().toISOString();
  const evidence = classifyReceiptEvidence(body);
  return {
    schemaVersion: 2,
    tool: meta.tool,
    command: meta.command,
    session: meta.session,
    startedAt: meta.clock.startedAt,
    endedAt,
    durationMs: Math.round(performance.now() - meta.clock.t0),
    binary: binaryFingerprint(meta.session),
    ...body,
    evidenceClass: evidence.evidenceClass,
    evidenceObservation: {
      observedWindowVisible: evidence.observedWindowVisible,
      visibilitySources: evidence.visibilitySources,
      errors: evidence.errors,
    },
  };
}

// --- shared target-selector args ----------------------------------------------

export interface TargetArgs {
  session: string;
  /** True when --session was passed (or SCRIPT_KIT_DEVTOOLS_SESSION set). */
  sessionExplicit: boolean;
  target?: JsonObject;
  strict: boolean;
  expectedSurfaceKind: string;
  timeoutMs: number;
  start: boolean;
  show: boolean;
  help: boolean;
}

export type ExtraFlagKind = "string" | "number" | "boolean";

export interface ParsedTargetArgs<E extends Record<string, ExtraFlagKind>> {
  args: TargetArgs;
  extras: { [K in keyof E]?: E[K] extends "boolean" ? boolean : E[K] extends "number" ? number : string };
  /** Warnings to surface in the receipt (e.g. implicit shared session). */
  warnings: string[];
}

/**
 * Parse the canonical target-selector flags shared by every inspector CLI:
 * --session --target-id --target-kind --target-index --target-title
 * --target-json --focused --main --surface --strict --timeout --start --show.
 *
 * CLI-specific flags are declared via `extras` (e.g. {"--limit": "number"}).
 * Unknown flags are ignored, matching the historical behavior.
 *
 * Session default: SCRIPT_KIT_DEVTOOLS_SESSION > --session > "default".
 * The shared "default" session is name-addressed; parallel loops must set a
 * unique session (a warning is emitted when --start is used implicitly).
 */
export function parseTargetArgs<E extends Record<string, ExtraFlagKind>>(
  argv: string[],
  opts: { extras?: E } = {},
): ParsedTargetArgs<E> {
  const envSession = process.env.SCRIPT_KIT_DEVTOOLS_SESSION;
  const args: TargetArgs = {
    session: envSession || "default",
    sessionExplicit: Boolean(envSession),
    strict: false,
    expectedSurfaceKind: "",
    timeoutMs: DEFAULT_RPC_TIMEOUT_MS,
    start: false,
    show: false,
    help: false,
  };
  const extras: Record<string, string | number | boolean> = {};
  const extraSpec = opts.extras ?? ({} as E);

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      args.help = true;
    } else if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
      args.sessionExplicit = true;
    } else if (arg === "--target-id") {
      args.target = { type: "id", id: argv[++index] ?? "" };
    } else if (arg === "--target-kind") {
      const kind = argv[++index] ?? "main";
      args.target = { type: "kind", kind };
    } else if (arg === "--target-index") {
      if (!args.target || args.target.type !== "kind") {
        throw new Error("--target-index requires --target-kind first");
      }
      args.target.index = Number(argv[++index] ?? 0);
    } else if (arg === "--target-title") {
      args.target = { type: "titleContains", text: argv[++index] ?? "" };
    } else if (arg === "--target-json") {
      try {
        args.target = JSON.parse(argv[++index] ?? "{}");
      } catch (error) {
        throw new Error(`Invalid --target-json: ${error}`);
      }
    } else if (arg === "--focused") {
      args.target = { type: "focused" };
    } else if (arg === "--main") {
      args.target = { type: "main" };
    } else if (arg === "--surface") {
      args.expectedSurfaceKind = argv[++index] ?? "";
    } else if (arg === "--strict") {
      args.strict = true;
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--show") {
      args.show = true;
    } else if (arg in extraSpec) {
      const kind = extraSpec[arg];
      if (kind === "boolean") {
        extras[arg] = true;
      } else if (kind === "number") {
        extras[arg] = Number(argv[++index] ?? 0);
      } else {
        extras[arg] = argv[++index] ?? "";
      }
    }
  }

  const warnings: string[] = [];
  if (args.start && !args.sessionExplicit) {
    warnings.push(
      "using implicit shared session 'default'; parallel loops must pass --session <unique> or set SCRIPT_KIT_DEVTOOLS_SESSION",
    );
  }

  return { args, extras: extras as ParsedTargetArgs<E>["extras"], warnings };
}

/**
 * Serialize parsed target args back into canonical CLI flags — for CLIs that
 * compose another devtools CLI as a subprocess (e.g. scroll → layout). This
 * replaces the hand-maintained `forwarded[]` echo arrays that silently
 * dropped newer flags.
 */
export function serializeTargetFlags(args: TargetArgs): string[] {
  const flags: string[] = ["--session", args.session, "--timeout", String(args.timeoutMs)];
  const target = args.target;
  if (target) {
    if (target.type === "id") {
      flags.push("--target-id", String(target.id ?? ""));
    } else if (target.type === "kind") {
      flags.push("--target-kind", String(target.kind ?? "main"));
      if (target.index !== undefined) flags.push("--target-index", String(target.index));
    } else if (target.type === "titleContains") {
      flags.push("--target-title", String(target.text ?? ""));
    } else if (target.type === "focused") {
      flags.push("--focused");
    } else if (target.type === "main") {
      flags.push("--main");
    } else {
      flags.push("--target-json", JSON.stringify(target));
    }
  }
  if (args.strict) flags.push("--strict");
  if (args.expectedSurfaceKind) flags.push("--surface", args.expectedSurfaceKind);
  return flags;
}
