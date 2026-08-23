/**
 * One fail-closed operator-safety boundary for headless DevTools transports.
 *
 * SCRIPT_KIT_NONINTERACTIVE=1 permits only explicitly reviewed protocol
 * inspections plus hidden-root state/filter operations. Unknown commands are
 * unsafe by default; wrapping a visible or mutating command in `batch` never
 * bypasses the policy.
 */

import { resolve } from "node:path";

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
    "visible windows, native pointer or keyboard input, screen capture, and native-helper compilation are forbidden; use a reviewed synthetic or grade-only mode",
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
