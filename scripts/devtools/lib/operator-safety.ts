/**
 * One fail-closed operator-safety boundary for headless DevTools transports.
 *
 * SCRIPT_KIT_NONINTERACTIVE=1 permits only explicitly reviewed protocol
 * inspections plus hidden-root state/filter operations. Unknown commands are
 * unsafe by default; wrapping a visible or mutating command in `batch` never
 * bypasses the policy.
 */

import { resolve } from "node:path";
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync, lstatSync, realpathSync } from "node:fs";
import { join, relative } from "node:path";
import { assertOutputOwnership, type OutputClaim } from "../../agentic/artifact-lifecycle.ts";
import { isVerifiedArtifact, type VerifiedArtifact } from "../../agentic/build-artifact.ts";

declare const ownedEvaluationBrand: unique symbol;
export interface OwnedEvaluationPermit { readonly [ownedEvaluationBrand]: true }
export interface EvaluationLimits {
  readonly maxWindows: number;
  readonly maxRequests: number;
  readonly maxFrames: number;
  readonly maxLifetimeMs: number;
  readonly maxImagePixels: number;
  readonly maxPngBytes: number;
  readonly maxRetainedImages: number;
  readonly maxLogBytes: number;
}
export const OWNED_EVALUATION_LIMITS: EvaluationLimits = Object.freeze({
  maxWindows: 8, maxRequests: 4096, maxFrames: 2048, maxLifetimeMs: 600_000,
  maxImagePixels: 4_194_304, maxPngBytes: 4_194_304, maxRetainedImages: 8,
  maxLogBytes: 8_388_608,
});
export const OWNED_EVALUATION_GUARDS = Object.freeze([
  "earlyBootstrap", "applicationEffects", "nativePlatform", "hiddenWindows",
  "localInput", "ownedStorage", "boundedProgress", "renderReadback",
] as const);
export const OWNED_EVALUATION_POLICY_SHA256 = createHash("sha256").update(JSON.stringify({
  version: 1, limits: OWNED_EVALUATION_LIMITS, guards: OWNED_EVALUATION_GUARDS,
})).digest("hex");
export interface OwnedEvaluationFacts {
  readonly artifact: VerifiedArtifact;
  readonly claim: OutputClaim;
  readonly fixtureIds: readonly string[];
  readonly limits: EvaluationLimits;
  readonly nativeGlass: "platform-default" | "disabled";
  readonly launchNonce: string;
  readonly policySha256: string;
  readonly platform: NodeJS.Platform;
  readonly architecture: string;
}
const evaluationPermits = new WeakMap<OwnedEvaluationPermit, { facts: OwnedEvaluationFacts; consumed: boolean }>();

export function issueOwnedEvaluationPermit(
  artifact: VerifiedArtifact, claim: OutputClaim, requestedFixtureIds: readonly string[],
  options: { maxLifetimeMs?: number; nativeGlass?: "platform-default" | "disabled" } = {},
): OwnedEvaluationPermit {
  if (!isVerifiedArtifact(artifact) || artifact.manifest.artifactKind !== "application" ||
      artifact.manifest.target.packageName !== "script-kit-gpui" ||
      artifact.manifest.target.targetName !== "script-kit-gpui" ||
      !artifact.manifest.target.features.includes("owned-ui-evaluation")) {
    throw new NoninteractiveSafetyError("ownedEvaluation", "verified evaluator application required");
  }
  assertOutputOwnership(claim);
  if (requestedFixtureIds.length > 512 || new Set(requestedFixtureIds).size !== requestedFixtureIds.length ||
      requestedFixtureIds.some(id => !/^[a-zA-Z0-9][a-zA-Z0-9._/-]{0,159}$/.test(id))) {
    throw new NoninteractiveSafetyError("ownedEvaluation", "invalid fixture subset");
  }
  const maxLifetimeMs = options.maxLifetimeMs === undefined ? OWNED_EVALUATION_LIMITS.maxLifetimeMs : options.maxLifetimeMs;
  if (Object.keys(options).some(key => key !== "maxLifetimeMs" && key !== "nativeGlass") ||
      !Number.isSafeInteger(maxLifetimeMs) || maxLifetimeMs <= 0 || maxLifetimeMs > OWNED_EVALUATION_LIMITS.maxLifetimeMs) {
    throw new NoninteractiveSafetyError("ownedEvaluation", "lifetime must be a positive safe integer within the existing maximum");
  }
  const nativeGlass = options.nativeGlass === undefined ? "platform-default" : options.nativeGlass;
  if (nativeGlass !== "platform-default" && nativeGlass !== "disabled") {
    throw new NoninteractiveSafetyError("ownedEvaluation", "nativeGlass must be platform-default or disabled");
  }
  const limits = maxLifetimeMs === OWNED_EVALUATION_LIMITS.maxLifetimeMs ? OWNED_EVALUATION_LIMITS :
    Object.freeze({ ...OWNED_EVALUATION_LIMITS, maxLifetimeMs });
  const policySha256 = createHash("sha256").update(JSON.stringify({ version: 1, limits, guards: OWNED_EVALUATION_GUARDS })).digest("hex");
  // Output ownership is keyed by this exact object; sealing must not clone it.
  Object.freeze(claim.plan);
  Object.freeze(claim.owner);
  Object.freeze(claim);
  const permit = Object.freeze({}) as OwnedEvaluationPermit;
  evaluationPermits.set(permit, { consumed: false, facts: Object.freeze({
    artifact, claim,
    fixtureIds: Object.freeze([...requestedFixtureIds]), limits, nativeGlass,
    launchNonce: randomUUID(), policySha256,
    platform: process.platform, architecture: process.arch,
  }) });
  return permit;
}

export function consumeOwnedEvaluationPermit(permit: OwnedEvaluationPermit): OwnedEvaluationFacts {
  const entry = evaluationPermits.get(permit);
  if (!entry || entry.consumed) throw new NoninteractiveSafetyError("ownedEvaluation", "forged or consumed permit");
  assertOutputOwnership(entry.facts.claim);
  if (entry.facts.platform !== process.platform || entry.facts.architecture !== process.arch)
    throw new NoninteractiveSafetyError("ownedEvaluation", "host identity changed");
  entry.consumed = true;
  return entry.facts;
}

/** No inherited values, credentials, live preferences, model links or CI forgery. */
export function ownedEvaluationEnvironment(facts: OwnedEvaluationFacts, directory: string): Record<string, string> {
  assertOutputOwnership(facts.claim);
  const relation = relative(facts.claim.root, directory);
  if (!relation || relation.startsWith("..") || relation.startsWith("/")) throw new NoninteractiveSafetyError("ownedEvaluation", "environment directory must remain within the bound output claim");
  const directoryStat = lstatSync(directory);
  if (!directoryStat.isDirectory() || directoryStat.isSymbolicLink() || realpathSync(directory) !== resolve(directory))
    throw new NoninteractiveSafetyError("ownedEvaluation", "environment directory is not a canonical owned directory");
  const home = join(directory, "home");
  const paths = { HOME: home, SK_PATH: join(home, ".scriptkit"), CODEX_HOME: join(home, ".codex"),
    XDG_CONFIG_HOME: join(home, ".config"), XDG_DATA_HOME: join(home, ".local/share"),
    XDG_CACHE_HOME: join(home, ".cache"), TMPDIR: join(directory, "tmp") };
  for (const path of Object.values(paths)) mkdirSync(path, { recursive: true, mode: 0o700 });
  return { ...paths, PATH: "/usr/bin:/bin:/usr/sbin:/sbin", LANG: "en_US.UTF-8", TZ: "UTC",
    RUST_LOG: "warn", SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_OWNED_EVALUATION: "1",
    SCRIPT_KIT_OWNED_EVALUATION_ROOT: directory,
    SCRIPT_KIT_OWNED_EVALUATION_NONCE: facts.launchNonce,
    SCRIPT_KIT_OWNED_EVALUATION_POLICY_SHA256: facts.policySha256,
    SCRIPT_KIT_OWNED_EVALUATION_BINARY_SHA256: facts.artifact.manifest.binarySha256,
    SCRIPT_KIT_OWNED_EVALUATION_MANIFEST_SHA256: facts.artifact.reference.manifestSha256,
    SCRIPT_KIT_OWNED_EVALUATION_FIXTURES: JSON.stringify(facts.fixtureIds),
    SCRIPT_KIT_OWNED_EVALUATION_LIMITS: JSON.stringify(facts.limits),
    ...(facts.nativeGlass === "disabled" ? { SCRIPT_KIT_DEBUG_NO_GLASS: "1" } : {}),
    ...Object.fromEntries(incompatibleOptIns.map(key => [key, "0"])),
  };
}

function assertOwnedExpectedTarget(target: ProtocolCommand | null, value: unknown, commandType: string): void {
  const expected = object(value);
  const revisions = ["windowGeneration", "targetGeneration", "surfaceGeneration", "dataGeneration", "presentationRevision", "themeRevision", "frameGeneration"];
  if (!target || target.type !== "instance" || typeof target.id !== "string" || !target.id ||
      !Number.isSafeInteger(target.generation) || Number(target.generation) <= 0 || !expected ||
      expected.windowId !== target.id || expected.windowGeneration !== target.generation ||
      typeof expected.appViewVariant !== "string" || !expected.appViewVariant ||
      Object.keys(expected).some(key => !["windowId", "appViewVariant", ...revisions].includes(key)) ||
      revisions.some(key => !Number.isSafeInteger(expected[key]) || Number(expected[key]) < 0))
    throw new NoninteractiveSafetyError(commandType, "exact target expectation required");
}

export function assertOwnedEvaluationCommand(permit: OwnedEvaluationPermit, command: ProtocolCommand): void {
  const entry = evaluationPermits.get(permit);
  if (!entry?.consumed) throw new NoninteractiveSafetyError("ownedEvaluation", "launch capability not consumed");
  const type = String(command.type);
  if (type === "design") {
    const body = object(command.command);
    if (!body || !["bootstrap", "catalog", "mount", "captureFrame", "acknowledgeFrames", "applyTheme", "revertTheme", "unmount", "end", "diagnose", "fixtureControl", "probeSafety", "sdkPrompt"].includes(String(body.operation)))
      throw new NoninteractiveSafetyError(type, "unknown evaluator operation");
    if (body.operation === "mount" && !entry.facts.fixtureIds.includes(String(body.fixtureId)))
      throw new NoninteractiveSafetyError(type, "fixture outside sealed subset");
    if (body.operation === "sdkPrompt" && !entry.facts.fixtureIds.includes("sdk.arg-roundtrip.v1"))
      throw new NoninteractiveSafetyError(type, "SDK fixture outside sealed subset");
    if (body.operation === "captureFrame") {
      const target = object(body.target);
      if (!target || target.type !== "instance" || typeof target.id !== "string" || !target.id ||
          !Number.isSafeInteger(target.generation) || Number(target.generation) <= 0)
        throw new NoninteractiveSafetyError(type, "exact mounted instance required");
      if (typeof body.includeImage !== "boolean" || Object.keys(body).some(key => !["operation", "target", "includeImage", "scheduled", "frameCursor"].includes(key)))
        throw new NoninteractiveSafetyError(type, "invalid atomic capture command");
      if (Object.hasOwn(body, "scheduled")) {
        const scheduled = object(body.scheduled);
        if (!scheduled || Object.keys(scheduled).some(key => !["expected", "afterFrameGeneration", "afterNotificationEpoch"].includes(key)) ||
            !Number.isSafeInteger(scheduled.afterFrameGeneration) || Number(scheduled.afterFrameGeneration) < 0 ||
            !Number.isSafeInteger(scheduled.afterNotificationEpoch) || Number(scheduled.afterNotificationEpoch) < 0)
          throw new NoninteractiveSafetyError(type, "invalid scheduled capture command");
        assertOwnedExpectedTarget(target, scheduled.expected, type);
      }
    }
    if (body.operation === "acknowledgeFrames") {
      if (!entry.facts.fixtureIds.includes("main-search-contract") ||
          Object.keys(body).some(key => !["operation", "target", "expected", "cursor"].includes(key)))
        throw new NoninteractiveSafetyError(type, "frame acknowledgement outside sealed fixture");
      assertOwnedExpectedTarget(object(body.target), body.expected, type);
      const cursor = object(body.cursor);
      if (!cursor || Object.keys(cursor).some(key => !["traceGeneration", "afterFrameGeneration"].includes(key)) ||
          [cursor.traceGeneration, cursor.afterFrameGeneration].some(value => !Number.isSafeInteger(value) || Number(value) < 0))
        throw new NoninteractiveSafetyError(type, "invalid frame acknowledgement cursor");
    }
    if (body.operation === "fixtureControl" && object(body.control)?.family === "search") {
      if (!entry.facts.fixtureIds.includes("main-search-contract") ||
          Object.keys(body).some(key => !["operation", "target", "expected", "control"].includes(key)))
        throw new NoninteractiveSafetyError(type, "search control outside sealed fixture");
      assertOwnedExpectedTarget(object(body.target), body.expected, type);
      const control = object(body.control)!;
      const operation = control.operation;
      const field = operation === "prepare" ? "scenario" : operation === "release" ? "runIds" : operation === "advance" ? "milliseconds" : null;
      if (!field || Object.keys(control).some(key => !["family", "operation", field].includes(key)) ||
          (operation === "prepare" && (typeof control.scenario !== "string" || !/^[a-z][a-z0-9-]{0,63}$/.test(control.scenario))) ||
          (operation === "release" && (!Array.isArray(control.runIds) || control.runIds.length < 1 || control.runIds.length > 128 ||
            new Set(control.runIds).size !== control.runIds.length || control.runIds.some(id => !Number.isSafeInteger(id) || Number(id) <= 0))) ||
          (operation === "advance" && (!Number.isSafeInteger(control.milliseconds) || Number(control.milliseconds) < 0 || Number(control.milliseconds) > 1000)))
        throw new NoninteractiveSafetyError(type, "invalid search control command");
    }
    return;
  }
  if (!["getState", "getElements", "getLayoutInfo", "getLogs", "getAgentChatState", "listAutomationWindows",
    "simulateGpuiEvent", "batch", "waitFor", "captureRenderWindow"].includes(type))
    throw new NoninteractiveSafetyError(type, "operation is not evaluator-local");
  if (["getLogs", "listAutomationWindows"].includes(type)) return;
  const target = object(command.target ?? object(command.request)?.target);
  if (!target || target.type !== "instance" || typeof target.id !== "string" ||
      !Number.isSafeInteger(target.generation) || Number(target.generation) <= 0)
    throw new NoninteractiveSafetyError(type, "exact mounted instance required");
  if (type === "simulateGpuiEvent" && /^(mouse|scroll)/.test(String(object(command.event)?.type))) {
    assertOwnedExpectedTarget(target, command.expected, type);
    const frame = object(command.expectedFrame); const requested = object(frame?.requestedTarget); const frameTarget = object(frame?.target);
    if (!frame || !requested || !frameTarget || Object.keys(frame).some(key => !["pid", "processStartTime", "processInstanceId", "sessionGeneration", "binarySha256", "manifestSha256", "requestedTarget", "target", "nativeWindowId"].includes(key)) ||
        Object.keys(requested).some(key => !["type", "id", "generation"].includes(key)) ||
        requested.type !== "instance" || requested.id !== target.id || requested.generation !== target.generation ||
        !Number.isSafeInteger(frame.pid) || Number(frame.pid) <= 0 ||
        ["processStartTime", "processInstanceId", "sessionGeneration"].some(key => typeof frame[key] !== "string" || !frame[key]) ||
        ["binarySha256", "manifestSha256"].some(key => typeof frame[key] !== "string" || !/^[a-f0-9]{64}$/.test(String(frame[key]))) ||
        Number(frameTarget.frameGeneration) <= 0 || Object.keys(object(command.expected)!).some(key => frameTarget[key] !== object(command.expected)![key]))
      throw new NoninteractiveSafetyError(type, "exact completed pointer frame required");
    assertOwnedExpectedTarget(target, frameTarget, type);
  }
  if (type === "captureRenderWindow") {
    const request = object(command.request);
    if (request && Object.hasOwn(request, "probes")) {
      assertOwnedExpectedTarget(target, request.expected, type);
      if (Object.keys(request).some(key => !["target", "expected", "hiDpi", "includeImage", "probes"].includes(key)) ||
          Number(object(request.expected)!.frameGeneration) <= 0 ||
          request.hiDpi !== true || typeof request.includeImage !== "boolean" || !Array.isArray(request.probes) ||
          request.probes.length < 1 || request.probes.length > 64 || request.probes.some(value => {
            const probe = object(value);
            return !probe || Object.keys(probe).some(key => key !== "x" && key !== "y") ||
              [probe.x, probe.y].some(coordinate => !Number.isSafeInteger(coordinate) || Number(coordinate) < 0 || Number(coordinate) > 0xffffffff);
          })) throw new NoninteractiveSafetyError(type, "invalid retained pixel probes");
    }
  }
}

type ProtocolCommand = Record<string, unknown>;

export type InspectionSessionCleanup = {
  required: boolean;
  createdSession: boolean;
  command: string | null;
  ownership?: { pid: number; generation: string };
};

const safeSessionIdentity = /^[A-Za-z0-9][A-Za-z0-9_.-]*$/;

export function requireSuccessfulSessionAction(
  session: string,
  action: "start" | "show",
  receipt: Record<string, unknown>,
): Record<string, unknown> {
  if (!safeSessionIdentity.test(session)) {
    throw new Error("DevTools session lifecycle requires one safe session identity");
  }

  if (receipt.status !== "ok") {
    const parsedError = object(receipt.parsedError);
    const directError = object(receipt.error);
    const nestedError = object(parsedError?.error);
    const failureCode = directError?.code ?? nestedError?.code;
    const detail = typeof failureCode === "string" ? ` (${failureCode})` : "";
    throw new Error(`DevTools session ${action} failed for ${session}${detail}`);
  }

  if (receipt.session !== session) {
    throw new Error(`DevTools session ${action} failed for ${session}: session identity mismatch`);
  }

  if (action === "start" && receipt.ready !== true) {
    throw new Error(`DevTools session start failed for ${session}: session is not ready`);
  }

  if (action === "start" && typeof receipt.resumed !== "boolean") {
    throw new Error(`DevTools session start failed for ${session}: session ownership is unknown`);
  }

  return receipt;
}

export function inspectionSessionCleanup(
  session: string,
  startReceipt: Record<string, unknown> | null,
): InspectionSessionCleanup {
  if (!safeSessionIdentity.test(session)) {
    throw new Error("DevTools inspection requires one safe session identity");
  }

  const borrowed: InspectionSessionCleanup = {
    required: false,
    createdSession: false,
    command: null,
  };
  if (startReceipt === null) return borrowed;

  if (
    startReceipt.status !== "ok" ||
    startReceipt.session !== session ||
    startReceipt.ready !== true ||
    typeof startReceipt.resumed !== "boolean"
  ) {
    throw new Error("DevTools inspection cannot claim a failed, unready, or mismatched session");
  }
  if (startReceipt.resumed) return borrowed;
  if (session === "dev-watch") {
    throw new Error("DevTools inspection cannot claim the borrowed operator session");
  }

  const pid = startReceipt.pid;
  const generation = startReceipt.sessionGeneration;
  if (
    typeof pid !== "number" ||
    !Number.isSafeInteger(pid) ||
    pid <= 0 ||
    typeof generation !== "string" ||
    !safeSessionIdentity.test(generation)
  ) {
    throw new Error("DevTools inspection requires the exact owned session PID and generation");
  }

  return {
    required: true,
    createdSession: true,
    ownership: { pid, generation },
    command:
      `scripts/agentic/session.sh stop ${session}` +
      ` --expected-pid ${pid} --expected-generation ${generation}`,
  };
}

export class SessionOwnershipRegistry {
  private readonly owned = new Map<string, { pid: number; generation: string }>();

  rememberStart(
    session: string,
    startReceipt: Record<string, unknown>,
    options: { allowPendingReadiness?: boolean } = {},
  ): InspectionSessionCleanup {
    // A process can be unready yet still need exact owned cleanup if its
    // readiness wait fails. This never mutates or upgrades its actual receipt.
    const ownershipReceipt = options.allowPendingReadiness && startReceipt.ready === false
      ? { ...startReceipt, ready: true }
      : startReceipt;
    const cleanup = inspectionSessionCleanup(session, ownershipReceipt);
    if (cleanup.createdSession && cleanup.ownership) {
      this.owned.set(session, cleanup.ownership);
    } else {
      this.owned.delete(session);
    }
    return cleanup;
  }

  isOwned(session: string): boolean {
    return this.owned.has(session);
  }

  stopCommand(session: string): string[] {
    const ownership = this.owned.get(session);
    if (!ownership) {
      throw new Error(`DevTools session is not owned by this invocation: ${session}`);
    }
    return [
      "bash",
      "scripts/agentic/session.sh",
      "stop",
      session,
      "--expected-pid",
      String(ownership.pid),
      "--expected-generation",
      ownership.generation,
    ];
  }

  release(session: string): void {
    this.owned.delete(session);
  }
}

export const NONINTERACTIVE_SAFE_COMMAND_TYPES = [
  "getState",
  "getElements",
  "getLayoutInfo",
  "getLogs",
  "listAutomationWindows",
  "getAiReliabilityState",
  "getAgentChatState",
  "waitFor",
  "setFilter",
  "setInput",
  "hide",
] as const;

const safeCommandTypes = new Set<string>(NONINTERACTIVE_SAFE_COMMAND_TYPES);
const ownedSandboxMutationTypes = new Set([
  "setFilter",
  "setInput",
  "hide",
  "batch",
]);
const incompatibleOptIns = [
  "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
  "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
  "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
  "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
  "SCRIPT_KIT_ALLOW_LIVE_AI",
] as const;
const immutableLaunchAuthority = [
  "CI",
  "SCRIPT_KIT_NONINTERACTIVE",
  "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
  ...incompatibleOptIns,
] as const;

export class NoninteractiveSafetyError extends Error {
  readonly commandType: string;

  constructor(commandType: string, reason: string) {
    super(
      `SCRIPT_KIT_NONINTERACTIVE=1 refused ${commandType}: ${reason}; ` +
        "the operator's screen, focus, input, credentials, and live data must remain untouched",
    );
    this.name = "NoninteractiveSafetyError";
    this.commandType = commandType;
  }
}

export function assertNoninteractiveVisualProbe(probe: string): void {
  if (process.env.SCRIPT_KIT_NONINTERACTIVE !== "1") return;
  assertNoIncompatibleOptIns(process.env);
  throw new NoninteractiveSafetyError(
    probe,
    "visible windows, native pointer or keyboard input, screen capture, system clipboard access, and native-helper compilation are forbidden; use a reviewed synthetic or grade-only mode",
  );
}

function object(value: unknown): ProtocolCommand | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as ProtocolCommand)
    : null;
}

function assertNoIncompatibleOptIns(
  environment: Record<string, string | undefined>,
): void {
  for (const key of incompatibleOptIns) {
    if (environment[key] === "1") {
      throw new NoninteractiveSafetyError(
        "configuration",
        `${key}=1 contradicts noninteractive execution`,
      );
    }
  }
}

function assertSafeCommand(
  command: ProtocolCommand,
  depth: number,
  ownsIsolatedCiSandbox: boolean,
): void {
  const commandType =
    typeof command.type === "string" ? command.type : "(missing command type)";
  if (depth > 8) {
    throw new NoninteractiveSafetyError(
      commandType,
      "nested batch depth exceeds the reviewed safety boundary",
    );
  }

  if (commandType !== "batch" && !safeCommandTypes.has(commandType)) {
    throw new NoninteractiveSafetyError(
      commandType,
      "the command can reveal windows, focus controls, inject input, capture pixels, run actions, or is unreviewed",
    );
  }

  const options = object(command.options);
  if (
    command.noResponse === true ||
    command.no_response === true ||
    options?.noResponse === true ||
    options?.no_response === true
  ) {
    throw new NoninteractiveSafetyError(
      commandType,
      "noResponse suppresses a reply, not window reveal or AI staging; silent mutations are forbidden",
    );
  }
  for (const modifier of [
    "submit",
    "open",
    "reveal",
    "focus",
    "activate",
    "show",
    "visible",
    "windowVisible",
    "capture",
    "screenshot",
    "nativeInput",
    "liveAi",
  ]) {
    if (command[modifier] === true || options?.[modifier] === true) {
      throw new NoninteractiveSafetyError(
        commandType,
        `the ${modifier}=true modifier can submit, reveal, or focus a user surface`,
      );
    }
  }

  for (const container of ["operations", "actions", "steps", "messages", "requests", "payload", "command"]) {
    if (command[container] !== undefined || options?.[container] !== undefined) {
      throw new NoninteractiveSafetyError(
        commandType,
        `the alternate ${container} command container is unreviewed`,
      );
    }
  }
  if (options?.commands !== undefined ||
    (commandType !== "batch" && command.commands !== undefined)) {
    throw new NoninteractiveSafetyError(
      commandType,
      "nested commands are allowed only in the explicit batch.commands array",
    );
  }

  const requestedTarget = object(command.requestedTarget);
  for (const target of [
    object(command.target),
    object(options?.target),
    object(command.selector),
    object(options?.selector),
    object(requestedTarget?.selector),
  ]) {
    const nestedSelector = object(target?.selector);
    if (
      target?.type === "focused" ||
      nestedSelector?.type === "focused" ||
      target?.focused === true ||
      target?.windowFocused === true
    ) {
      throw new NoninteractiveSafetyError(
        commandType,
        "focused-window selectors are not allowed; use an explicit hidden/main target",
      );
    }
    if (target?.visible === true || target?.windowVisible === true) {
      throw new NoninteractiveSafetyError(
        commandType,
        "visible-window selectors contradict hidden-only execution",
      );
    }
  }

  if (commandType === "batch") {
    if (!Array.isArray(command.commands)) {
      throw new NoninteractiveSafetyError(
        commandType,
        "batch commands must be an explicit array",
      );
    }
    for (const [index, candidate] of command.commands.entries()) {
      const nested = object(candidate);
      if (!nested) {
        throw new NoninteractiveSafetyError(
          commandType,
          `batch command ${index} is not a typed protocol object`,
        );
      }
      try {
        assertSafeCommand(nested, depth + 1, ownsIsolatedCiSandbox);
      } catch (error) {
        if (error instanceof NoninteractiveSafetyError) {
          throw new NoninteractiveSafetyError(
            commandType,
            `nested command ${index} is unsafe (${error.commandType})`,
          );
        }
        throw error;
      }
    }
    if (!ownsIsolatedCiSandbox) {
      throw new NoninteractiveSafetyError(
        commandType,
        "mutating protocol batches require an explicitly owned isolated CI sandbox",
      );
    }
    return;
  }

  if (commandType === "waitFor") {
    const condition = command.condition;
    const conditionObject = object(condition);
    const expectedState = object(conditionObject?.state);
    if (
      condition === "windowVisible" ||
      conditionObject?.type === "windowVisible" ||
      expectedState?.windowVisible === true
    ) {
      throw new NoninteractiveSafetyError(
        commandType,
        "waiting for a visible window contradicts hidden-only execution",
      );
    }
    if (
      condition === "windowFocused" ||
      conditionObject?.type === "windowFocused" ||
      expectedState?.windowFocused === true ||
      expectedState?.isFocused === true ||
      expectedState?.focused === true
    ) {
      throw new NoninteractiveSafetyError(
        commandType,
        "waiting for a focused window contradicts noninteractive execution",
      );
    }
  }

  if (ownedSandboxMutationTypes.has(commandType) && !ownsIsolatedCiSandbox) {
    throw new NoninteractiveSafetyError(
      commandType,
      "mutating an existing operator session is forbidden without an explicitly owned isolated CI sandbox",
    );
  }
}

export function assertNoninteractiveProtocolCommand(
  command: ProtocolCommand,
  options: {
    noninteractive?: boolean;
    environment?: Record<string, string | undefined>;
  } = {},
): void {
  const environment = options.environment ?? process.env;
  const parentNoninteractive = process.env.SCRIPT_KIT_NONINTERACTIVE === "1";
  if (parentNoninteractive) {
    if (
      options.noninteractive === false ||
      (
        Object.prototype.hasOwnProperty.call(environment, "SCRIPT_KIT_NONINTERACTIVE") &&
        environment.SCRIPT_KIT_NONINTERACTIVE !== "1"
      )
    ) {
      throw new NoninteractiveSafetyError(
        "configuration",
        "protocol options cannot override immutable parent safety authority",
      );
    }
    assertNoIncompatibleOptIns(process.env);
  }
  const noninteractive =
    parentNoninteractive ||
    (options.noninteractive ?? environment.SCRIPT_KIT_NONINTERACTIVE === "1");
  if (!noninteractive) return;
  assertNoIncompatibleOptIns(environment);
  const ownsIsolatedCiSandbox =
    process.env.SCRIPT_KIT_NONINTERACTIVE === "1" &&
    process.env.CI === "true" &&
    process.env.SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH === "1";
  assertSafeCommand(command, 0, ownsIsolatedCiSandbox);
}

/** Existing sessions/FIFOs have no trustworthy ownership proof, even in CI. */
export function assertNoninteractiveUnownedSessionCommand(
  command: ProtocolCommand,
  transport: string = "session",
): void {
  if (process.env.SCRIPT_KIT_NONINTERACTIVE !== "1") return;
  const commandType =
    typeof command.type === "string" ? command.type : "(missing command type)";
  if (ownedSandboxMutationTypes.has(commandType)) {
    throw new NoninteractiveSafetyError(
      `${transport}.${commandType}`,
      "an unowned existing session permits only side-effect-free read-only inspection",
    );
  }
  assertNoninteractiveProtocolCommand(command);
}

export function assertNoninteractiveDriverLaunch(
  options: {
    sandboxHome?: boolean;
    seedAgentAuth?: boolean;
    env?: Record<string, string>;
  },
): void {
  if (process.env.SCRIPT_KIT_NONINTERACTIVE !== "1") return;
  const environment = { ...process.env, ...(options.env ?? {}) };
  assertNoIncompatibleOptIns(environment);
  if (options.sandboxHome !== true) {
    throw new NoninteractiveSafetyError(
      "Driver.launch",
      "hidden execution requires an isolated sandboxHome",
    );
  }
  if (options.seedAgentAuth === true) {
    throw new NoninteractiveSafetyError(
      "Driver.launch",
      "copying live AI credentials into a verification sandbox is forbidden",
    );
  }
  for (const key of immutableLaunchAuthority) {
    if (
      Object.prototype.hasOwnProperty.call(options.env ?? {}, key) &&
      options.env?.[key] !== process.env[key]
    ) {
      throw new NoninteractiveSafetyError(
        "Driver.launch",
        `${key} cannot override immutable parent safety authority`,
      );
    }
  }
  if (environment.SCRIPT_KIT_NONINTERACTIVE !== "1") {
    throw new NoninteractiveSafetyError(
      "Driver.launch",
      "the launched child must retain SCRIPT_KIT_NONINTERACTIVE=1",
    );
  }
  if (process.env.SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH !== "1") {
    throw new NoninteractiveSafetyError(
      "Driver.launch",
      "starting an application requires SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1 in explicitly approved isolated CI",
    );
  }
  if (process.env.CI !== "true") {
    throw new NoninteractiveSafetyError(
      "Driver.launch",
      "starting an application requires CI=true; operator-local app launches are forbidden",
    );
  }
}

export function assertNoninteractiveSessionCommand(command: string[]): void {
  if (process.env.SCRIPT_KIT_NONINTERACTIVE !== "1") return;
  assertNoIncompatibleOptIns(process.env);
  const executable = command[0];
  const script = command[1];
  const canonicalSessionScript = resolve(process.cwd(), "scripts/agentic/session.sh");
  if (
    (executable !== "bash" && executable !== "/bin/bash") ||
    typeof script !== "string" ||
    resolve(script) !== canonicalSessionScript
  ) {
    throw new NoninteractiveSafetyError(
      "subprocess",
      "only the reviewed bash scripts/agentic/session.sh RPC, send, or status transport may start",
    );
  }
  const operation = command[2];
  const session = command[3];
  if (typeof session !== "string" || !/^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(session)) {
    throw new NoninteractiveSafetyError(
      `session.${operation ?? "unknown"}`,
      "the session name must be an explicit reviewed identifier",
    );
  }
  if (operation === "status") {
    if (command.length !== 4) {
      throw new NoninteractiveSafetyError(
        "session.status",
        "session status does not accept unreviewed trailing arguments",
      );
    }
    return;
  }
  if (operation !== "send" && operation !== "rpc") {
    throw new NoninteractiveSafetyError(
      `session.${operation ?? "unknown"}`,
      "session lifecycle mutation is forbidden; attach only to an explicitly owned hidden sandbox",
    );
  }
  const encodedPayload = command[4];
  let payload: ProtocolCommand | null = null;
  try {
    payload = object(JSON.parse(encodedPayload ?? ""));
  } catch {
    payload = null;
  }
  if (!payload) {
    throw new NoninteractiveSafetyError(
      `session.${operation}`,
      "the protocol payload must be a readable typed JSON object",
    );
  }
  assertNoninteractiveUnownedSessionCommand(payload, `session.${operation}`);
  for (let index = 5; index < command.length; index += 1) {
    const flag = command[index];
    if (flag === "--await-parse" && operation === "send") continue;
    if (flag === "--expect" || flag === "--timeout") {
      const value = command[++index];
      if (
        typeof value !== "string" ||
        value.length === 0 ||
        value.startsWith("--") ||
        (flag === "--timeout" && !/^\d+$/.test(value))
      ) {
        throw new NoninteractiveSafetyError(
          `session.${operation}`,
          `${flag} requires a valid explicit value`,
        );
      }
      continue;
    }
    throw new NoninteractiveSafetyError(
      `session.${operation}`,
      `unreviewed session transport argument: ${flag ?? "(missing)"}`,
    );
  }
}

export function assertNoninteractiveSubprocess(
  command: string[],
  overrides: Record<string, string | undefined> = {},
): void {
  if (process.env.SCRIPT_KIT_NONINTERACTIVE !== "1") return;

  assertNoIncompatibleOptIns(process.env);
  for (const key of immutableLaunchAuthority) {
    if (
      Object.prototype.hasOwnProperty.call(overrides, key) &&
      overrides[key] !== process.env[key]
    ) {
      throw new NoninteractiveSafetyError(
        "subprocess",
        `${key} cannot override immutable parent safety authority`,
      );
    }
  }
  assertNoIncompatibleOptIns({ ...process.env, ...overrides });
  assertNoninteractiveSessionCommand(command);
}
