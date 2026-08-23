#!/usr/bin/env bun
/** Fail-closed macOS input helper. Native keyboard delivery is System Events only. */
import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  assertNoninteractiveSubprocess,
  NoninteractiveSafetyError,
} from "../devtools/lib/operator-safety.ts";

export const SCHEMA_VERSION = 4;
export const NATIVE_SETTLE_MS = 50;
const SESSION_SH = fileURLToPath(new URL("./session.sh", import.meta.url));
const WINDOW_TS = fileURLToPath(new URL("./window.ts", import.meta.url));
const SOURCE_PATH = realpathSync(fileURLToPath(import.meta.url));
export const SOURCE_PROVENANCE = Object.freeze({
  path: SOURCE_PATH,
  sha256: createHash("sha256").update(readFileSync(SOURCE_PATH)).digest("hex"),
});

export type CapabilityMethod = "directBatch" | "gpuiDispatch" | "accessibility" | "quartz";
export type ActualInputMethod =
  | "protocol.batch.setInput"
  | "protocol.simulateGpuiEvent.keyDown"
  | "native.systemEvents.keyCode"
  | "native.systemEvents.keystroke"
  | "native.cliclick.click";
export type DeliveryScope = "injector" | "ingress" | "postcondition";

export interface DeliveryEvidence {
  injectorAccepted: boolean;
  ingressVerified: boolean;
  postconditionVerified: boolean;
  deliveryScope: DeliveryScope;
  delivered: boolean;
  settleMs: number;
  settleIsProof: false;
}

export interface FrontmostApplicationIdentity {
  pid: number;
  bundleId: string;
  name: string;
}

export interface NonactivationEvidence {
  before: FrontmostApplicationIdentity;
  after: FrontmostApplicationIdentity;
  targetPid: number;
  baselineIsExternal: boolean;
  unchanged: boolean;
  verified: boolean;
}

export interface PassiveKeyboardReadinessInput {
  expectedPid: number | null;
  statusPid: number | null;
  pidFilePid: number | null;
  expectedGeneration: string | null;
  generationFile: string | null;
  requestedKind: string | null;
  protocolTargetType: string | null;
  surfaceId: string | null;
  targetWindowId: number | null;
  protocolRequestId: string | null;
  protocolExpectedType: string | null;
  protocolExactCorrelation: boolean;
  windowVisible: boolean;
  protocolFocused: boolean;
  promptType: string | null;
  surfaceKind: string | null;
  automationSemanticSurface: string | null;
  inputOwnership: string | null;
  focusPolicy: string | null;
  keyboardPolicy: string | null;
  axPid: number | null;
  axFocusedWindowPresent: boolean;
  axFocusedWindowId: number | null;
}

export interface PassiveKeyboardReadiness {
  ready: boolean;
  failures: string[];
  expectedPid: number | null;
  statusPid: number | null;
  pidFilePid: number | null;
  expectedGeneration: string | null;
  generationFile: string | null;
  target: {
    requestedKind: string | null;
    protocolTargetType: string | null;
    surfaceId: string | null;
    windowId: number | null;
    exact: boolean;
  };
  protocol: {
    requestId: string | null;
    expectedType: string | null;
    exactCorrelation: boolean;
    windowVisible: boolean;
    isFocused: boolean;
    promptType: string | null;
    surfaceKind: string | null;
    automationSemanticSurface: string | null;
    inputOwnership: string | null;
    focusPolicy: string | null;
    keyboardPolicy: string | null;
  };
  accessibility: {
    pid: number | null;
    focusedWindowPresent: boolean;
    focusedWindowId: number | null;
    targetWindowId: number | null;
    exactWindowMatch: boolean;
    requiredForReadiness: false;
  };
  osFrontmostRequired: false;
}

export type NativeKeyPlan =
  | { kind: "keyCode"; key: string; keyCode: number; modifiers: string[]; actualMethod: "native.systemEvents.keyCode"; script: string }
  | { kind: "keystroke"; key: string; keyCode: null; modifiers: string[]; actualMethod: "native.systemEvents.keystroke"; script: string };

export interface InputResult extends DeliveryEvidence {
  method: CapabilityMethod;
  capabilityMethod: CapabilityMethod;
  actualMethod: ActualInputMethod;
  keyCode: number | null;
  key?: string;
  modifiers?: string[];
  text?: string;
  focusCheckRequested: boolean;
  focusVerified: boolean;
  focusEnforced: false;
  activationAttempted: false;
  focusMutationAttempted: false;
  focusVerificationMode: "passive" | "not-requested";
  keyboardReadiness?: PassiveKeyboardReadiness;
  nonactivation?: NonactivationEvidence;
  receipt: {
    target: string | null;
    chosenMethod: CapabilityMethod;
    actualMethod: ActualInputMethod;
    keyCode: number | null;
    fallbackReasons: Array<{ method: CapabilityMethod; reason: string }>;
  };
  focusEvidence?: {
    focusCheckRequested: boolean;
    focusVerified: boolean;
    focusEnforced: false;
    activationAttempted: false;
    focusMutationAttempted: false;
    focusVerificationMode: "passive";
    keyboardReadiness: PassiveKeyboardReadiness;
    nonactivation: NonactivationEvidence;
  };
}

export const SYSTEM_EVENTS_KEY_CODES: Readonly<Record<string, number>> = Object.freeze({
  enter: 36, return: 36, tab: 48, space: 49, delete: 51, backspace: 51,
  escape: 53, esc: 53, command: 55, cmd: 55, shift: 56, capslock: 57,
  option: 58, alt: 58, control: 59, ctrl: 59, rightshift: 60,
  rightoption: 61, rightalt: 61, rightcontrol: 62, function: 63, fn: 63,
  f17: 64, keypaddecimal: 65, keypadmultiply: 67, keypadplus: 69,
  keypadclear: 71, volumeup: 72, volumedown: 73, mute: 74,
  keypaddivide: 75, keypadenter: 76, keypadminus: 78, f18: 79, f19: 80,
  keypadequals: 81, keypad0: 82, keypad1: 83, keypad2: 84, keypad3: 85,
  keypad4: 86, keypad5: 87, keypad6: 88, keypad7: 89, f20: 90,
  keypad8: 91, keypad9: 92, jisyen: 93, jisunderscore: 94, keypadcomma: 95,
  f5: 96, f6: 97, f7: 98, f3: 99, f8: 100, f9: 101, jiseisu: 102,
  f11: 103, jiskana: 104, f13: 105, f16: 106, f14: 107, f10: 109,
  f12: 111, f15: 113, help: 114, home: 115, pageup: 116,
  forwarddelete: 117, f4: 118, end: 119, f2: 120, pagedown: 121,
  f1: 122, left: 123, right: 124, down: 125, up: 126,
});

const MODIFIER_ALIASES: Readonly<Record<string, string>> = Object.freeze({
  cmd: "command down", command: "command down", shift: "shift down",
  alt: "option down", option: "option down", ctrl: "control down", control: "control down",
});

function appleScriptString(value: string): string {
  return `"${value.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

function boundedTail(value: string, limit = 2_000): string {
  return value.length <= limit ? value : value.slice(-limit);
}

export function parseJsonDocuments(raw: string): unknown[] {
  const trimmed = raw.trim();
  if (!trimmed) throw new Error("empty_json_output");
  try {
    return [JSON.parse(trimmed)];
  } catch {}

  const documents: unknown[] = [];
  let start = -1;
  const stack: string[] = [];
  let inString = false;
  let escaped = false;
  for (let index = 0; index < raw.length; index += 1) {
    const char = raw[index]!;
    if (start < 0) {
      if (char === "{" || char === "[") {
        start = index;
        stack.push(char);
      }
      continue;
    }
    if (inString) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === '"') inString = false;
      continue;
    }
    if (char === '"') {
      inString = true;
      continue;
    }
    if (char === "{" || char === "[") stack.push(char);
    else if (char === "}" || char === "]") {
      const opening = stack.pop();
      if ((opening === "{" && char !== "}") || (opening === "[" && char !== "]") || !opening) {
        throw new Error("invalid_json_document");
      }
      if (stack.length === 0) {
        const source = raw.slice(start, index + 1);
        try {
          documents.push(JSON.parse(source));
        } catch {
          throw new Error("invalid_json_document");
        }
        start = -1;
        inString = false;
        escaped = false;
      }
    }
  }
  if (start >= 0 || stack.length > 0 || inString) throw new Error("truncated_json_document");
  if (documents.length === 0) throw new Error("no_json_documents");
  return documents;
}

export function selectExactlyOne<T>(documents: unknown[], predicate: (document: any) => boolean, label: string): T {
  const matches = documents.filter(predicate);
  if (matches.length !== 1) throw new Error(`${label}:${matches.length === 0 ? "no_exact_match" : "ambiguous_exact_match"}`);
  return matches[0] as T;
}

export interface SessionRpcResult {
  requestId: string;
  expectedType: string;
  envelope: any;
  response: any;
  correlation: {
    sessionExact: true;
    outerRequestIdExact: true;
    outerResponseTypeExact: true;
    innerRequestIdExact: true;
    innerTypeExact: true;
    exact: true;
  };
}

export function selectExactRpcEnvelope(documents: unknown[], session: string, requestId: string, expectedType: string): SessionRpcResult {
  const envelope = selectExactlyOne<any>(documents, (candidate) => candidate?.status === "ok"
    && candidate?.session === session
    && candidate?.requestId === requestId
    && candidate?.responseType === expectedType
    && candidate?.response?.requestId === requestId
    && candidate?.response?.type === expectedType, "rpc_envelope");
  return {
    requestId,
    expectedType,
    envelope,
    response: envelope.response,
    correlation: {
      sessionExact: true,
      outerRequestIdExact: true,
      outerResponseTypeExact: true,
      innerRequestIdExact: true,
      innerTypeExact: true,
      exact: true,
    },
  };
}

export function normalizeModifiers(modifiers: string[]): string[] {
  const normalized: string[] = [];
  for (const modifier of modifiers) {
    const value = MODIFIER_ALIASES[modifier.toLowerCase()];
    if (!value) throw hardError("UNKNOWN_KEY", `Unknown modifier: ${modifier}`);
    if (!normalized.includes(value)) normalized.push(value);
  }
  return normalized;
}

export function planNativeKey(key: string, modifiers: string[] = []): NativeKeyPlan {
  const normalized = key.toLowerCase().replace(/[ _-]/g, "");
  const usingParts = normalizeModifiers(modifiers);
  const using = usingParts.length ? ` using {${usingParts.join(", ")}}` : "";
  const keyCode = SYSTEM_EVENTS_KEY_CODES[normalized];
  if (keyCode !== undefined) {
    return { kind: "keyCode", key, keyCode, modifiers: usingParts, actualMethod: "native.systemEvents.keyCode",
      script: `tell application "System Events" to key code ${keyCode}${using}` };
  }
  if ([...key].length === 1) {
    return { kind: "keystroke", key, keyCode: null, modifiers: usingParts, actualMethod: "native.systemEvents.keystroke",
      script: `tell application "System Events" to keystroke ${appleScriptString(key)}${using}` };
  }
  throw hardError("UNKNOWN_KEY", `Unknown key: ${key}. Use one literal character or a named System Events key.`);
}

export function planInputRoute(command: "key" | "type", forceNative: boolean, hasSession: boolean, hasTarget: boolean): ActualInputMethod[] {
  if (forceNative) return command === "key"
    ? ["native.systemEvents.keyCode", "native.systemEvents.keystroke"]
    : ["native.systemEvents.keystroke"];
  if (command === "key") return hasSession
    ? ["protocol.simulateGpuiEvent.keyDown", "native.systemEvents.keyCode", "native.systemEvents.keystroke"]
    : ["native.systemEvents.keyCode", "native.systemEvents.keystroke"];
  return hasSession && hasTarget
    ? ["protocol.batch.setInput", "protocol.simulateGpuiEvent.keyDown", "native.systemEvents.keystroke"]
    : hasSession ? ["protocol.simulateGpuiEvent.keyDown", "native.systemEvents.keystroke"] : ["native.systemEvents.keystroke"];
}

export function evaluateDeliveryEvidence(input: Omit<DeliveryEvidence, "delivered" | "settleIsProof">): DeliveryEvidence {
  return { ...input, delivered: input.injectorAccepted || input.ingressVerified || input.postconditionVerified, settleIsProof: false };
}

export function evaluateNonactivation(before: FrontmostApplicationIdentity, after: FrontmostApplicationIdentity, targetPid: number): NonactivationEvidence {
  const baselineIsExternal = before.pid > 0 && before.pid !== targetPid;
  const unchanged = before.pid === after.pid && before.bundleId === after.bundleId;
  return { before, after, targetPid, baselineIsExternal, unchanged, verified: baselineIsExternal && unchanged };
}

export function evaluatePassiveKeyboardReadiness(input: PassiveKeyboardReadinessInput): PassiveKeyboardReadiness {
  const failures: string[] = [];
  const targetExact = input.requestedKind === "main" && input.protocolTargetType === "main"
    && input.surfaceId === "main" && Number.isInteger(input.targetWindowId) && input.targetWindowId! > 0;
  const protocolExact = input.protocolRequestId != null && input.protocolExpectedType === "stateResult"
    && input.protocolExactCorrelation && input.windowVisible && input.protocolFocused
    && input.promptType === "none" && input.surfaceKind === "ScriptList"
    && input.automationSemanticSurface === "scriptList" && input.inputOwnership === "LauncherFilter"
    && input.focusPolicy === "LauncherFilterFocus" && input.keyboardPolicy === "LauncherListKeyboard";
  const exactWindowMatch = input.axPid === input.expectedPid && input.axFocusedWindowPresent
    && Number.isInteger(input.axFocusedWindowId) && input.axFocusedWindowId! > 0
    && input.axFocusedWindowId === input.targetWindowId;
  if (!Number.isInteger(input.expectedPid) || input.expectedPid! <= 0) failures.push("missing_expected_pid");
  if (input.statusPid !== input.expectedPid || input.pidFilePid !== input.expectedPid) failures.push("pid_mismatch");
  if (!input.expectedGeneration || input.generationFile !== input.expectedGeneration) failures.push("generation_mismatch");
  if (!targetExact) failures.push("strict_main_target_required");
  if (!protocolExact) failures.push("launcher_keyboard_policy_not_exact");
  return {
    ready: failures.length === 0,
    failures,
    expectedPid: input.expectedPid,
    statusPid: input.statusPid,
    pidFilePid: input.pidFilePid,
    expectedGeneration: input.expectedGeneration,
    generationFile: input.generationFile,
    target: { requestedKind: input.requestedKind, protocolTargetType: input.protocolTargetType, surfaceId: input.surfaceId, windowId: input.targetWindowId, exact: targetExact },
    protocol: {
      requestId: input.protocolRequestId, expectedType: input.protocolExpectedType, exactCorrelation: input.protocolExactCorrelation,
      windowVisible: input.windowVisible, isFocused: input.protocolFocused, promptType: input.promptType,
      surfaceKind: input.surfaceKind, automationSemanticSurface: input.automationSemanticSurface,
      inputOwnership: input.inputOwnership, focusPolicy: input.focusPolicy, keyboardPolicy: input.keyboardPolicy,
    },
    accessibility: {
      pid: input.axPid, focusedWindowPresent: input.axFocusedWindowPresent, focusedWindowId: input.axFocusedWindowId,
      targetWindowId: input.targetWindowId, exactWindowMatch, requiredForReadiness: false,
    },
    osFrontmostRequired: false,
  };
}

function hardError(code: "ACCESSIBILITY_DENIED" | "UNKNOWN_KEY" | "KEY_INJECTOR_FAILED" | "FOCUS_NOT_CONFIRMED", message: string, evidence?: unknown): Error & { code: string; evidence?: unknown } {
  return Object.assign(new Error(message), { code, evidence });
}

async function runProcess(command: string[], env = process.env): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  assertNoninteractiveSubprocess(command, env);
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe", env });
  const [stdout, stderr, exitCode] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  return { stdout, stderr, exitCode };
}

let rpcCounter = 0;
async function sessionRpc(session: string, payload: Record<string, unknown>, expectedType: string): Promise<SessionRpcResult> {
  if (Object.prototype.hasOwnProperty.call(payload, "requestId")) throw new Error("caller_supplied_request_id_rejected");
  const requestId = `macos-input-${process.pid}-${++rpcCounter}`;
  const result = await runProcess(["bash", SESSION_SH, "rpc", session, JSON.stringify({ ...payload, requestId }), "--expect", expectedType, "--timeout", "4000"]);
  if (result.exitCode !== 0) throw new Error(`session_rpc_failed stdout=${boundedTail(result.stdout)} stderr=${boundedTail(result.stderr)}`);
  try {
    return selectExactRpcEnvelope(parseJsonDocuments(result.stdout), session, requestId, expectedType);
  } catch (error) {
    throw new Error(`${error instanceof Error ? error.message : String(error)} stdout=${boundedTail(result.stdout)} stderr=${boundedTail(result.stderr)}`);
  }
}

function selectSessionEnvelope(raw: string, session: string, statuses: string[]): any {
  return selectExactlyOne<any>(parseJsonDocuments(raw), (candidate) => candidate?.session === session && statuses.includes(candidate?.status), "session_envelope");
}

export async function observeFrontmostApplication(): Promise<FrontmostApplicationIdentity> {
  const script = `ObjC.import('AppKit'); const app=$.NSWorkspace.sharedWorkspace.frontmostApplication; JSON.stringify({pid:Number(app.processIdentifier),bundleId:ObjC.unwrap(app.bundleIdentifier)||'',name:ObjC.unwrap(app.localizedName)||''});`;
  const result = await runProcess(["osascript", "-l", "JavaScript", "-e", script]);
  if (result.exitCode !== 0) throw hardError("FOCUS_NOT_CONFIRMED", `frontmost_application_unavailable:${boundedTail(result.stderr)}`);
  const identity = selectExactlyOne<any>(parseJsonDocuments(result.stdout), (candidate) => Number.isInteger(candidate?.pid)
    && typeof candidate?.bundleId === "string" && typeof candidate?.name === "string", "frontmost_application");
  return { pid: identity.pid, bundleId: identity.bundleId, name: identity.name };
}

async function inspectPassiveKeyboardReadiness(session: string, target: string, expectedPid: number | null, expectedGeneration: string | null): Promise<PassiveKeyboardReadiness> {
  const statusResult = await runProcess(["bash", SESSION_SH, "status", session]);
  if (statusResult.exitCode !== 0) throw hardError("FOCUS_NOT_CONFIRMED", `session_status_failed:${boundedTail(statusResult.stderr || statusResult.stdout)}`);
  const status = selectSessionEnvelope(statusResult.stdout, session, ["ok"]);
  const sessionRoot = process.env.SCRIPT_KIT_SESSION_DIR ?? "/tmp/script-kit-agent-sessions";
  const dir = `${sessionRoot}/${session}`;
  const pidFilePid = existsSync(`${dir}/pid`) ? Number(readFileSync(`${dir}/pid`, "utf8").trim()) : null;
  const generationFile = existsSync(`${dir}/generation`) ? readFileSync(`${dir}/generation`, "utf8").trim() : null;
  const stateRpc = await sessionRpc(session, { type: "getState", target: { type: "main" } }, "stateResult");
  const state = stateRpc.response;
  const windowsResult = await runProcess(["bun", WINDOW_TS, "list"]);
  if (windowsResult.exitCode !== 0) throw hardError("FOCUS_NOT_CONFIRMED", `window_list_failed:${boundedTail(windowsResult.stderr || windowsResult.stdout)}`);
  const windows = selectExactlyOne<any>(parseJsonDocuments(windowsResult.stdout), (candidate) => candidate?.status === "ok" && Array.isArray(candidate?.data?.surfaces), "window_list");
  const mains = windows.data.surfaces.filter((surface: any) => surface?.surfaceId === "main" && Number.isInteger(Number(surface?.windowId)) && Number(surface.windowId) > 0);
  const main = mains.length === 1 ? mains[0] : null;
  const axResult = expectedPid ? await runProcess(["osascript", "-e", `tell application "System Events" to tell first application process whose unix id is ${expectedPid} to get value of attribute "AXWindowNumber" of focused window`])
    : { exitCode: 1, stdout: "", stderr: "missing pid" };
  const axWindowId = axResult.exitCode === 0 && Number.isInteger(Number(axResult.stdout.trim())) ? Number(axResult.stdout.trim()) : null;
  const contract = state.surfaceContract ?? {};
  return evaluatePassiveKeyboardReadiness({
    expectedPid,
    statusPid: Number(status.pid) || null,
    pidFilePid,
    expectedGeneration,
    generationFile,
    requestedKind: target || null,
    protocolTargetType: "main",
    surfaceId: main?.surfaceId ?? null,
    targetWindowId: main ? Number(main.windowId) : null,
    protocolRequestId: stateRpc.requestId,
    protocolExpectedType: stateRpc.expectedType,
    protocolExactCorrelation: stateRpc.correlation.exact,
    windowVisible: state.windowVisible === true,
    protocolFocused: state.isFocused === true,
    promptType: state.promptType ?? null,
    surfaceKind: contract.surfaceKind ?? null,
    automationSemanticSurface: contract.automationSemanticSurface ?? null,
    inputOwnership: contract.inputOwnership ?? null,
    focusPolicy: contract.focusPolicy ?? null,
    keyboardPolicy: contract.keyboardPolicy ?? null,
    axPid: axResult.exitCode === 0 ? expectedPid : null,
    axFocusedWindowPresent: axWindowId != null && axWindowId > 0,
    axFocusedWindowId: axWindowId,
  });
}

function accessibilityFailure(stderr: string): boolean {
  return /not allowed assistive access|-1743|not authorized/i.test(stderr);
}

async function injectSystemEvents(script: string): Promise<void> {
  const result = await runProcess(["osascript", "-e", script]);
  if (result.exitCode !== 0) {
    if (accessibilityFailure(result.stderr)) throw hardError("ACCESSIBILITY_DENIED", result.stderr || "Accessibility denied");
    throw hardError("KEY_INJECTOR_FAILED", result.stderr || `osascript exited ${result.exitCode}`);
  }
  await Bun.sleep(NATIVE_SETTLE_MS);
}

function deliveryFor(actualMethod: ActualInputMethod, postconditionVerified = false): DeliveryEvidence {
  if (actualMethod.startsWith("native.")) return evaluateDeliveryEvidence({ injectorAccepted: true, ingressVerified: false, postconditionVerified: false, deliveryScope: "injector", settleMs: NATIVE_SETTLE_MS });
  if (actualMethod === "protocol.batch.setInput") return evaluateDeliveryEvidence({ injectorAccepted: false, ingressVerified: true, postconditionVerified, deliveryScope: postconditionVerified ? "postcondition" : "ingress", settleMs: 0 });
  return evaluateDeliveryEvidence({ injectorAccepted: false, ingressVerified: true, postconditionVerified: false, deliveryScope: "ingress", settleMs: 0 });
}

export function resultFor(actualMethod: ActualInputMethod, capabilityMethod: CapabilityMethod, fields: Partial<InputResult> = {}, fallbackReasons: InputResult["receipt"]["fallbackReasons"] = [], postconditionVerified = false): InputResult {
  const keyCode = fields.keyCode ?? null;
  return {
    ...deliveryFor(actualMethod, postconditionVerified),
    method: capabilityMethod,
    capabilityMethod,
    actualMethod,
    keyCode,
    focusCheckRequested: false,
    focusVerified: false,
    focusEnforced: false,
    activationAttempted: false,
    focusMutationAttempted: false,
    focusVerificationMode: "not-requested",
    receipt: { target: null, chosenMethod: capabilityMethod, actualMethod, keyCode, fallbackReasons },
    ...fields,
  } as InputResult;
}

async function nativeKey(key: string, modifiers: string[], focusFields: Partial<InputResult>): Promise<InputResult> {
  const plan = planNativeKey(key, modifiers);
  await injectSystemEvents(plan.script);
  return resultFor(plan.actualMethod, "accessibility", { key, keyCode: plan.keyCode, modifiers, ...focusFields });
}

async function nativeType(text: string, focusFields: Partial<InputResult>): Promise<InputResult> {
  await injectSystemEvents(`tell application "System Events" to keystroke ${appleScriptString(text)}`);
  return resultFor("native.systemEvents.keystroke", "accessibility", { text, keyCode: null, ...focusFields });
}

async function nativeClick(x: number, y: number): Promise<InputResult & { x: number; y: number }> {
  const cliclick = ["/opt/homebrew/bin/cliclick", "/usr/local/bin/cliclick"].find((path) => existsSync(path));
  if (!cliclick) throw hardError("KEY_INJECTOR_FAILED", "cliclick is required for pointer delivery");
  const result = await runProcess([cliclick, `c:${x},${y}`]);
  if (result.exitCode !== 0) throw hardError("KEY_INJECTOR_FAILED", result.stderr || `cliclick exited ${result.exitCode}`);
  await Bun.sleep(NATIVE_SETTLE_MS);
  return resultFor("native.cliclick.click", "quartz", { x, y } as Partial<InputResult>) as InputResult & { x: number; y: number };
}

function protocolTarget(target: string): Record<string, unknown> {
  return target === "main" ? { type: "main" } : { type: "kind", kind: target };
}

async function gpuiKey(session: string, target: string, key: string, modifiers: string[]): Promise<boolean> {
  const rpc = await sessionRpc(session, { type: "simulateGpuiEvent", target: protocolTarget(target), event: { type: "keyDown", key, modifiers } }, "simulateGpuiEventResult");
  return rpc.response.success === true;
}

async function batchType(session: string, target: string, text: string): Promise<{ accepted: boolean; postconditionVerified: boolean }> {
  const batch = await sessionRpc(session, { type: "batch", target: protocolTarget(target), commands: [{ type: "setInput", text }], options: { stopOnError: true, timeout: 3000 } }, "batchResult");
  if (batch.response.success !== true) return { accepted: false, postconditionVerified: false };
  const state = (await sessionRpc(session, { type: "getState", target: protocolTarget(target) }, "stateResult")).response;
  const diagnostics = state.filterInputDiagnostics ?? {};
  const postconditionVerified = target === "main" && state.inputValue === text
    && diagnostics.canonicalFilterText === text && diagnostics.computedFilterText === text
    && diagnostics.rawVisualInputValue === text && diagnostics.pendingFilterSync === false;
  return { accepted: true, postconditionVerified };
}

async function sendKeyWithLadder(key: string, modifiers: string[], session: string, target: string, forceNative: boolean, focusFields: Partial<InputResult>): Promise<InputResult> {
  if (!forceNative && session && await gpuiKey(session, target, key, modifiers)) {
    return resultFor("protocol.simulateGpuiEvent.keyDown", "gpuiDispatch", { key, modifiers, keyCode: null, ...focusFields });
  }
  const result = await nativeKey(key, modifiers, focusFields);
  result.receipt.target = target;
  result.receipt.fallbackReasons = forceNative
    ? [{ method: "gpuiDispatch", reason: "force_native_requested" }]
    : [{ method: "gpuiDispatch", reason: session ? "dispatch_rejected" : "no_session" }];
  return result;
}

async function sendTypeWithLadder(text: string, session: string, target: string, forceNative: boolean, focusFields: Partial<InputResult>): Promise<InputResult> {
  if (!forceNative && session && target) {
    const batch = await batchType(session, target, text);
    if (batch.accepted) return resultFor("protocol.batch.setInput", "directBatch", { text, keyCode: null, ...focusFields }, [], batch.postconditionVerified);
  }
  if (!forceNative && session) {
    let ok = true;
    for (const char of text) if (!(await gpuiKey(session, target, char, []))) { ok = false; break; }
    if (ok) return resultFor("protocol.simulateGpuiEvent.keyDown", "gpuiDispatch", { text, keyCode: null, ...focusFields });
  }
  const result = await nativeType(text, focusFields);
  result.receipt.target = target;
  result.receipt.fallbackReasons = forceNative
    ? [{ method: "gpuiDispatch", reason: "force_native_requested" }]
    : [{ method: "gpuiDispatch", reason: session ? "dispatch_rejected" : "no_session" }];
  return result;
}

export async function main(argv = process.argv.slice(2)): Promise<number> {
  const command = argv[0] ?? "help";
  const flag = (name: string) => argv.includes(name);
  const value = (name: string, fallback = "") => { const index = argv.indexOf(name); return index >= 0 ? argv[index + 1] ?? fallback : fallback; };
  const session = value("--session");
  const target = value("--target", "main");
  const forceNative = flag("--force-native") || flag("--no-gpui-dispatch");
  const ensureFocus = flag("--ensure-focus");
  const emit = (body: any) => console.log(JSON.stringify({ ...body, source: SOURCE_PROVENANCE }, null, 2));
  try {
    if (
      process.env.SCRIPT_KIT_NONINTERACTIVE === "1" &&
      (command === "key" || command === "type" || command === "click")
    ) {
      throw new NoninteractiveSafetyError(
        `macos-input.${command}`,
        "native keyboard, text, and pointer delivery are forbidden during noninteractive verification",
      );
    }
    let focusFields: Partial<InputResult> = {};
    let before: FrontmostApplicationIdentity | null = null;
    let readiness: PassiveKeyboardReadiness | null = null;
    if (ensureFocus && (command === "key" || command === "type")) {
      if (!session) throw hardError("FOCUS_NOT_CONFIRMED", "--ensure-focus requires --session for exact ownership verification");
      const expectedPid = Number(value("--expected-pid")) || null;
      const expectedGeneration = value("--expected-generation") || null;
      readiness = await inspectPassiveKeyboardReadiness(session, target, expectedPid, expectedGeneration);
      before = await observeFrontmostApplication();
      if (!readiness.ready) throw hardError("FOCUS_NOT_CONFIRMED", readiness.failures.join(","), { keyboardReadiness: readiness, frontmostBefore: before });
      focusFields = {
        focusCheckRequested: true, focusVerified: true, focusEnforced: false,
        activationAttempted: false, focusMutationAttempted: false, focusVerificationMode: "passive",
        keyboardReadiness: readiness,
      };
    }

    let result: InputResult;
    if (command === "key") {
      const key = argv[1];
      if (!key) throw hardError("UNKNOWN_KEY", "Missing key");
      const modifiers = value("--modifiers").split(",").map((part) => part.trim()).filter(Boolean);
      result = await sendKeyWithLadder(key, modifiers, session, target, forceNative, focusFields);
    } else if (command === "type") {
      const text = argv[1];
      if (text === undefined) throw hardError("KEY_INJECTOR_FAILED", "Missing text");
      result = await sendTypeWithLadder(text, session, target, forceNative, focusFields);
    } else if (command === "click") {
      const x = Number(argv[1]);
      const y = Number(argv[2]);
      if (!Number.isFinite(x) || !Number.isFinite(y)) throw hardError("KEY_INJECTOR_FAILED", "click requires numeric x and y");
      result = await nativeClick(x, y);
    } else if (command === "check") {
      emit({ schemaVersion: SCHEMA_VERSION, status: "ok", command, data: { osascript: existsSync("/usr/bin/osascript"), systemEventsOnlyForKeyboard: true } });
      return 0;
    } else if (command === "help" || command === "--help") {
      emit({ schemaVersion: SCHEMA_VERSION, status: "ok", command: "help", data: { commands: ["key", "type", "click", "check"], forceNativeBypassesProtocolFor: ["key", "type"], deliveredMeans: "delivery_proved_at_deliveryScope" } });
      return 0;
    } else throw hardError("KEY_INJECTOR_FAILED", `Unknown command: ${command}`);

    if (ensureFocus && before && readiness) {
      const after = await observeFrontmostApplication();
      const nonactivation = evaluateNonactivation(before, after, readiness.expectedPid ?? 0);
      const focusEvidence = {
        focusCheckRequested: true, focusVerified: true, focusEnforced: false as const,
        activationAttempted: false as const, focusMutationAttempted: false as const,
        focusVerificationMode: "passive" as const, keyboardReadiness: readiness, nonactivation,
      };
      result.nonactivation = nonactivation;
      result.focusEvidence = focusEvidence;
      if (!nonactivation.verified) throw hardError("FOCUS_NOT_CONFIRMED", "nonactivation_not_verified", focusEvidence);
    }
    if (session) result.receipt.target = target;
    emit({ schemaVersion: SCHEMA_VERSION, status: "ok", command, data: result });
    return result.delivered ? 0 : 1;
  } catch (error: any) {
    const code = error instanceof NoninteractiveSafetyError
      ? "NONINTERACTIVE_SAFETY_REFUSED"
      : ["ACCESSIBILITY_DENIED", "UNKNOWN_KEY", "KEY_INJECTOR_FAILED", "FOCUS_NOT_CONFIRMED"].includes(error?.code)
        ? error.code
        : "KEY_INJECTOR_FAILED";
    emit({ schemaVersion: SCHEMA_VERSION, status: "error", command, error: { code, message: error?.message ?? String(error), evidence: error?.evidence ?? null } });
    return 1;
  }
}

if (import.meta.main) process.exit(await main());
