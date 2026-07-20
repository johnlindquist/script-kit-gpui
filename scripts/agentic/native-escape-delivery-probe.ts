#!/usr/bin/env bun
/** P0.2 fail-closed audit of the native System Events helper against a pinned app. */
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  approveStagingAnchor,
  buildArtifactLifecycle,
  claimOutput,
  commitFinalReceipt,
  createOwnedStagingDirectory,
  isStrictDescendant,
  materializeAtomic,
  removeOwnedAuxiliaryDirectory,
  retainLiveSessionArtifacts,
  validateArtifact,
  validateOutputTarget,
  waitForProcessesDead,
  writeJsonArtifactAtomic,
  type ArtifactReceipt,
  type ArtifactSpec,
  type ProtocolCorrelation,
  type RetainedArtifact,
} from "./artifact-lifecycle";

type Json = Record<string, any>;
type CommandExecution = { action: Json; stdout: string; stderr: string };

const repo = resolve(import.meta.dir, "../..");
const sessionSh = join(repo, "scripts/agentic/session.sh");
const argv = process.argv.slice(2);
const value = (name: string, fallback = "") => { const index = argv.indexOf(name); return index >= 0 ? argv[index + 1] ?? fallback : fallback; };
const required = (name: string) => { const found = value(name); if (!found) throw new Error(`missing required ${name}`); return found; };
const mode = value("--mode", "p0.2-helper");
const binary = resolve(repo, required("--binary"));
const expectedSha = required("--expected-sha256").toLowerCase();
const helperRequestedPath = resolve(repo, required("--helper"));
const helper = realpathSync(helperRequestedPath);
const expectedHelperSha = required("--expected-helper-sha256").toLowerCase();
const timeoutMs = Number(value("--timeout", "10000"));
const output = resolve(value("--out", join(repo, ".test-output/native-escape-delivery-probe/p0.2-helper-receipt.json")));
const runId = `${Date.now()}-${process.pid}-${Math.random().toString(36).slice(2, 8)}`;
const outputPlan = validateOutputTarget({
  repoRoot: repo,
  candidate: output,
  kind: "receipt",
  probeId: "native-escape-delivery-probe",
});
const outputClaim = claimOutput(outputPlan, runId);
const session = `native-escape-p02-${runId}`;
const root = createOwnedStagingDirectory(
  outputClaim,
  {
    name: `sk-native-escape-p02-${runId}`,
    anchor: approveStagingAnchor(outputClaim, tmpdir()),
  },
);
const dir = join(root, session);
const failureArtifactsDir = join(root, "failure-artifacts");
const home = join(root, "home");
const kit = join(home, ".scriptkit");
const marker = `p02helper${Date.now()}${process.pid}`;
const P02_KEY_CODES: Readonly<Record<string, number>> = Object.freeze({ Escape: 53, Delete: 51, Down: 125 });
const SESSION_EVIDENCE_FILES = [
  "app.log", "responses.ndjson", "protocol-responses.ndjson", "lifecycle.ndjson",
  "pid", "generation", "fwd_pid", "supervisor_pid", "binary",
  "keep_actions_window_open", "app-exit.json",
] as const;

let rpcSequence = 0;
let ownedPid: number | null = null;
let ownedGeneration: string | null = null;
let started = false;
let fatal: unknown = null;
let unsafeOwnership = false;

chmodSync(root, 0o700);
mkdirSync(kit, { recursive: true });
Object.assign(process.env, {
  SCRIPT_KIT_SESSION_DIR: root,
  SCRIPT_KIT_GPUI_BINARY: binary,
  HOME: home,
  SK_PATH: kit,
  SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
  SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
  SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
});

const receipt: Json = {
  schemaVersion: 4,
  probe: "native-escape-delivery",
  mode,
  runId,
  classification: "blocked-by-unsafe-operation",
  intake: {
    binary,
    expectedSha256: expectedSha,
    helper: helperRequestedPath,
    expectedHelperSha256: expectedHelperSha,
    openingPath: "System Events global Cmd+;",
    protocolShowUsed: false,
    protocolSimulateKeyUsedForBehaviorProof: false,
  },
  session: { name: session, root, dir, home, kit, privateRoot: true, rootMode: "0700" },
  marker,
  binaryVerification: {},
  helperVerification: {
    requestedPath: helperRequestedPath,
    resolvedPath: helper,
    expectedSha256: expectedHelperSha,
  },
  protocolCorrelations: [],
  helperActions: [],
  rows: [],
  requiredRows: ["exact-marker", "escape-clear-visible", "escape-hide", "delete-one-char", "down-next-semantic-id"],
  optionalRows: ["ku:cmd-diagnostic"],
  cleanup: {},
  failureArtifacts: { directory: failureArtifactsDir, files: [], rawOutputs: [] },
  blockers: [],
};
let stagingDir: string | null = null;
let retained: RetainedArtifact[] = [];
let artifactSpecs: ArtifactSpec[] = [];
let artifactReceipts: ArtifactReceipt[] = [];
let writerPids: Record<string, number | null> = {
  app: null,
  supervisor: null,
  forwarder: null,
};
let writersDead: Record<string, boolean> = {};

function shaBytes(bytes: string | Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function sha(path: string): string {
  return shaBytes(readFileSync(path));
}

function boundedTail(value: string, limit = 2_000): string {
  return value.length <= limit ? value : value.slice(-limit);
}

function alive(pid: number): boolean {
  try { process.kill(pid, 0); return true; } catch { return false; }
}

function command(executable: string, args: string[], allowFailure = false): CommandExecution {
  const result = spawnSync(executable, args, {
    cwd: repo,
    env: process.env,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    timeout: Math.max(timeoutMs, 30_000),
  });
  const stdout = result.stdout ?? "";
  const stderr = result.stderr ?? "";
  const action = {
    executable,
    args,
    exitCode: result.status,
    signal: result.signal,
    ok: result.status === 0,
    stdout: { bytes: Buffer.byteLength(stdout), sha256: shaBytes(stdout), tail: boundedTail(stdout.trim()) },
    stderr: { bytes: Buffer.byteLength(stderr), sha256: shaBytes(stderr), tail: boundedTail(stderr.trim()) },
  };
  if (!action.ok && !allowFailure) throw new Error(`${executable}_failed:${boundedTail(stderr || stdout)}`);
  return { action, stdout, stderr };
}

function parseJsonDocuments(raw: string): unknown[] {
  const trimmed = raw.trim();
  if (!trimmed) throw new Error("empty_json_output");
  try { return [JSON.parse(trimmed)]; } catch {}
  const documents: unknown[] = [];
  const stack: string[] = [];
  let start = -1;
  let inString = false;
  let escaped = false;
  for (let index = 0; index < raw.length; index += 1) {
    const char = raw[index]!;
    if (start < 0) {
      if (char === "{" || char === "[") { start = index; stack.push(char); }
      continue;
    }
    if (inString) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === '"') inString = false;
      continue;
    }
    if (char === '"') { inString = true; continue; }
    if (char === "{" || char === "[") stack.push(char);
    else if (char === "}" || char === "]") {
      const opening = stack.pop();
      if ((opening === "{" && char !== "}") || (opening === "[" && char !== "]") || !opening) throw new Error("invalid_json_document");
      if (stack.length === 0) {
        try { documents.push(JSON.parse(raw.slice(start, index + 1))); }
        catch { throw new Error("invalid_json_document"); }
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

function selectExactlyOne(documents: unknown[], predicate: (document: any) => boolean, label: string): Json {
  const matches = documents.filter(predicate) as Json[];
  if (matches.length !== 1) throw new Error(`${label}:${matches.length === 0 ? "no_exact_match" : "ambiguous_exact_match"}`);
  return matches[0]!;
}

function recordRawOutput(label: string, raw: string): Json {
  mkdirSync(failureArtifactsDir, { recursive: true, mode: 0o700 });
  const safe = label.replace(/[^A-Za-z0-9_.-]+/g, "-");
  const path = join(failureArtifactsDir, `${safe}.txt`);
  writeFileSync(path, raw);
  const metadata = { label, path, bytes: Buffer.byteLength(raw), sha256: shaBytes(raw) };
  receipt.failureArtifacts.rawOutputs.push(metadata);
  return metadata;
}

function runSession(args: string[], predicate: (envelope: Json) => boolean, label: string): Json {
  const execution = command("bash", [sessionSh, ...args], true);
  let documents: unknown[];
  try { documents = parseJsonDocuments(execution.stdout); }
  catch (error) {
    recordRawOutput(`${label}-stdout`, execution.stdout);
    throw new Error(`${label}:${error instanceof Error ? error.message : String(error)} stdout=${boundedTail(execution.stdout)} stderr=${boundedTail(execution.stderr)}`);
  }
  let envelope: Json;
  try {
    envelope = selectExactlyOne(documents, predicate, label);
  } catch (error) {
    recordRawOutput(`${label}-stdout`, execution.stdout);
    throw error;
  }
  if (!execution.action.ok || envelope.status === "error") {
    recordRawOutput(`${label}-stdout`, execution.stdout);
    throw new Error(`${label}:session_command_failed stdout=${boundedTail(execution.stdout)} stderr=${boundedTail(execution.stderr)}`);
  }
  return { action: execution.action, envelope };
}

function protocolResponseOffset(): number {
  const path = join(dir, "protocol-responses.ndjson");
  return existsSync(path) ? statSync(path).size : 0;
}

function newProtocolResponseBytes(offset: number): string {
  const path = join(dir, "protocol-responses.ndjson");
  if (!existsSync(path)) return "";
  const bytes = readFileSync(path);
  if (bytes.length < offset) throw new Error("protocol_response_file_truncated");
  return bytes.subarray(offset).toString("utf8");
}

function rpc(payload: Json, exactType: string): Json {
  if (Object.prototype.hasOwnProperty.call(payload, "requestId")) throw new Error("caller_supplied_request_id_rejected");
  const requestId = `p02-${runId}-${String(++rpcSequence).padStart(6, "0")}`;
  const offset = protocolResponseOffset();
  const execution = command("bash", [sessionSh, "rpc", session, JSON.stringify({ ...payload, requestId }), "--expect", exactType, "--timeout", String(timeoutMs)], true);
  let stdoutDocuments: unknown[] = [];
  try { stdoutDocuments = parseJsonDocuments(execution.stdout); }
  catch { recordRawOutput(`rpc-${requestId}-stdout`, execution.stdout); }
  const wrapperMatches = stdoutDocuments.filter((candidate: any) => candidate?.status === "ok"
    && candidate?.session === session
    && candidate?.requestId === requestId
    && candidate?.responseType === exactType
    && candidate?.response?.requestId === requestId
    && candidate?.response?.type === exactType) as Json[];
  if (wrapperMatches.length > 1) {
    recordRawOutput(`rpc-${requestId}-duplicate-stdout`, execution.stdout);
    throw new Error(`rpc_envelope:ambiguous_exact_match:${requestId}`);
  }
  if (wrapperMatches.length === 1) {
    if (!execution.action.ok) throw new Error(`rpc_command_failed:${requestId}:${boundedTail(execution.stderr || execution.stdout)}`);
    receipt.protocolCorrelations.push({
      requestId, expectedType: exactType, source: "session.stdout",
      outerRequestIdExact: true, outerResponseTypeExact: true,
      innerRequestIdExact: true, innerTypeExact: true, exact: true,
    });
    return wrapperMatches[0]!.response;
  }

  const rawBytes = newProtocolResponseBytes(offset);
  if (!rawBytes.trim()) {
    recordRawOutput(`rpc-${requestId}-missing-stdout`, execution.stdout);
    throw new Error(`rpc_response:no_exact_match:${requestId} stdout=${boundedTail(execution.stdout)} stderr=${boundedTail(execution.stderr)}`);
  }
  let fileDocuments: unknown[];
  try { fileDocuments = parseJsonDocuments(rawBytes); }
  catch (error) {
    recordRawOutput(`rpc-${requestId}-protocol-responses`, rawBytes);
    throw new Error(`rpc_response_file:${error instanceof Error ? error.message : String(error)}:${requestId}`);
  }
  const rawMatches = fileDocuments.map((candidate: any) => candidate?.response ?? candidate)
    .filter((candidate: any) => candidate?.requestId === requestId && candidate?.type === exactType) as Json[];
  if (rawMatches.length !== 1) {
    recordRawOutput(`rpc-${requestId}-protocol-responses`, rawBytes);
    recordRawOutput(`rpc-${requestId}-unmatched-stdout`, execution.stdout);
    throw new Error(`rpc_raw_response:${rawMatches.length === 0 ? "no_exact_match" : "ambiguous_exact_match"}:${requestId}`);
  }
  receipt.protocolCorrelations.push({
    requestId, expectedType: exactType, source: "protocol-responses.ndjson",
    outerRequestIdExact: null, outerResponseTypeExact: null,
    innerRequestIdExact: true, innerTypeExact: true, exact: true,
  });
  return rawMatches[0]!;
}

const getState = () => rpc({ type: "getState", target: { type: "main" } }, "stateResult");
const getElements = () => rpc({ type: "getElements", target: { type: "main" }, limit: 500 }, "elementsResult");

function digest(state: Json): Json {
  const diagnostics = state.filterInputDiagnostics ?? {};
  const contract = state.surfaceContract ?? {};
  return {
    type: state.type ?? null,
    promptType: state.promptType ?? null,
    semanticSurface: state.semanticSurface ?? null,
    surfaceKind: contract.surfaceKind ?? null,
    automationSemanticSurface: contract.automationSemanticSurface ?? null,
    inputOwnership: contract.inputOwnership ?? null,
    focusPolicy: contract.focusPolicy ?? null,
    keyboardPolicy: contract.keyboardPolicy ?? null,
    windowVisible: state.windowVisible ?? null,
    isFocused: state.isFocused ?? null,
    inputValue: state.inputValue ?? null,
    canonicalFilterText: diagnostics.canonicalFilterText ?? null,
    computedFilterText: diagnostics.computedFilterText ?? null,
    rawVisualInputValue: diagnostics.rawVisualInputValue ?? null,
    pendingFilterSync: diagnostics.pendingFilterSync ?? null,
    selectedIndex: state.selectedIndex ?? state.mainWindowPreflight?.selectedIndex ?? null,
    selectedValue: state.selectedValue ?? null,
    selectedResultKey: state.mainWindowPreflight?.selectedResultKey ?? null,
  };
}

function strictMain(state: Json, expectedInput?: string, visible = true): boolean {
  const current = digest(state);
  const inputOk = expectedInput === undefined || [current.inputValue, current.canonicalFilterText, current.computedFilterText, current.rawVisualInputValue]
    .every((item) => item === expectedInput) && current.pendingFilterSync === false;
  return current.type === "stateResult" && current.promptType === "none" && current.surfaceKind === "ScriptList"
    && current.automationSemanticSurface === "scriptList"
    && current.inputOwnership === "LauncherFilter" && current.focusPolicy === "LauncherFilterFocus"
    && current.keyboardPolicy === "LauncherListKeyboard"
    && current.windowVisible === visible && (visible ? current.isFocused === true : true) && inputOk;
}

async function poll(predicate: (state: Json) => boolean, label: string, limit = timeoutMs): Promise<Json> {
  const deadline = Date.now() + limit;
  let last = getState();
  while (!predicate(last) && Date.now() < deadline) { await Bun.sleep(25); last = getState(); }
  if (!predicate(last)) throw new Error(`${label}:${JSON.stringify(digest(last))}`);
  return last;
}

function ownership(label: string): Json {
  const pid = existsSync(join(dir, "pid")) ? Number(readFileSync(join(dir, "pid"), "utf8").trim()) : null;
  const generation = existsSync(join(dir, "generation")) ? readFileSync(join(dir, "generation"), "utf8").trim() : null;
  const exact = pid === ownedPid && generation === ownedGeneration && Boolean(ownedPid && alive(ownedPid));
  const proof = {
    label,
    expectedPid: ownedPid,
    actualPid: pid,
    expectedGeneration: ownedGeneration,
    actualGeneration: generation,
    live: Boolean(pid && alive(pid)),
    exact,
  };
  if (!exact) {
    unsafeOwnership = true;
    throw new Error(`ownership_mismatch:${label}:${JSON.stringify(proof)}`);
  }
  return proof;
}

function observeFrontmostApplication(): Json {
  const script = `ObjC.import('AppKit'); const app=$.NSWorkspace.sharedWorkspace.frontmostApplication; JSON.stringify({pid:Number(app.processIdentifier),bundleId:ObjC.unwrap(app.bundleIdentifier)||'',name:ObjC.unwrap(app.localizedName)||''});`;
  const execution = command("osascript", ["-l", "JavaScript", "-e", script], true);
  if (!execution.action.ok) throw new Error(`frontmost_application_failed:${boundedTail(execution.stderr || execution.stdout)}`);
  return selectExactlyOne(parseJsonDocuments(execution.stdout), (candidate) => Number.isInteger(candidate?.pid)
    && typeof candidate?.bundleId === "string" && typeof candidate?.name === "string", "frontmost_application");
}

function nonactivation(before: Json, after: Json): Json {
  const baselineIsExternal = before.pid > 0 && before.pid !== ownedPid;
  const unchanged = before.pid === after.pid && before.bundleId === after.bundleId;
  return { before, after, targetPid: ownedPid, baselineIsExternal, unchanged, verified: baselineIsExternal && unchanged };
}

function sameFrontmost(left: Json, right: Json): boolean {
  return left?.pid === right?.pid && left?.bundleId === right?.bundleId && left?.name === right?.name;
}

function verifyHelperHash(label: string): Json {
  const current = existsSync(helper) ? sha(helper) : null;
  const proof = { label, path: helper, sha256: current, matchesExpected: current === expectedHelperSha };
  if (!proof.matchesExpected) throw new Error(`helper_hash_mismatch:${label}:${current ?? "missing"}`);
  return proof;
}

function helperEnvelope(stdout: string, kind: "key" | "type"): Json {
  return selectExactlyOne(parseJsonDocuments(stdout), (candidate) => candidate?.schemaVersion === 4
    && candidate?.status === "ok" && candidate?.command === kind, "helper_envelope");
}

function helperAction(kind: "key" | "type", input: string): Json {
  const own = ownership(`helper-${kind}-${input}`);
  const hashBefore = verifyHelperHash(`before-${kind}-${input}`);
  const frontmostBefore = observeFrontmostApplication();
  const execution = command("bun", [
    helper, kind, input, "--force-native", "--ensure-focus", "--session", session,
    "--target", "main", "--expected-pid", String(ownedPid),
    "--expected-generation", String(ownedGeneration),
  ], true);
  const frontmostAfter = observeFrontmostApplication();
  const probeNonactivation = nonactivation(frontmostBefore, frontmostAfter);
  let envelope: Json;
  try { envelope = helperEnvelope(execution.stdout, kind); }
  catch (error) {
    recordRawOutput(`helper-${kind}-${input}-stdout`, execution.stdout);
    const failed = { ownership: own, hashBefore, action: execution.action, probeNonactivation, helperEvidenceValid: false };
    receipt.helperActions.push(failed);
    throw new Error(`helper_envelope_invalid:${error instanceof Error ? error.message : String(error)}`);
  }
  const data = envelope.data ?? {};
  const focusEvidence = data.focusEvidence ?? {};
  const readiness = data.keyboardReadiness ?? focusEvidence.keyboardReadiness ?? {};
  const helperNonactivation = focusEvidence.nonactivation ?? {};
  const expectedMethod = kind === "type" ? "native.systemEvents.keystroke" : "native.systemEvents.keyCode";
  const expectedKeyCode = kind === "type" ? null : P02_KEY_CODES[input];
  const sourceExact = envelope.source?.path === helper && envelope.source?.sha256 === expectedHelperSha;
  const helperNonactivationMatchesProbe = sameFrontmost(helperNonactivation.before, frontmostBefore)
    && sameFrontmost(helperNonactivation.after, frontmostAfter);
  const helperEvidenceValid = execution.action.ok === true
    && sourceExact
    && data.actualMethod === expectedMethod && data.receipt?.actualMethod === expectedMethod
    && data.keyCode === expectedKeyCode && data.receipt?.keyCode === expectedKeyCode
    && data.capabilityMethod === "accessibility" && data.method === "accessibility"
    && data.injectorAccepted === true && data.ingressVerified === false && data.postconditionVerified === false
    && data.deliveryScope === "injector" && data.delivered === true
    && data.settleMs === 50 && data.settleIsProof === false
    && data.focusCheckRequested === true && data.focusVerified === true && data.focusEnforced === false
    && data.activationAttempted === false && data.focusMutationAttempted === false
    && data.focusVerificationMode === "passive"
    && readiness.ready === true && readiness.target?.exact === true && readiness.target?.surfaceId === "main"
    && readiness.accessibility?.requiredForReadiness === false && readiness.protocol?.exactCorrelation === true
    && helperNonactivation.verified === true && data.nonactivation?.verified === true
    && helperNonactivationMatchesProbe && probeNonactivation.verified === true;
  const action = {
    ownership: own,
    hashBefore,
    action: execution.action,
    envelope,
    probeNonactivation,
    helperNonactivationMatchesProbe,
    helperEvidenceValid,
  };
  receipt.helperActions.push(action);
  if (!helperEvidenceValid) throw new Error(`helper_evidence_invalid:${kind}:${input}`);
  return action;
}

async function openGlobal(label: string): Promise<Json> {
  const before = observeFrontmostApplication();
  const execution = command("osascript", ["-e", 'tell application "System Events" to keystroke ";" using command down'], true);
  await Bun.sleep(50);
  const after = observeFrontmostApplication();
  const proof = nonactivation(before, after);
  const result = { label, action: execution.action, injectorAccepted: execution.action.ok === true, nonactivation: proof };
  if (!execution.action.ok || !proof.verified) throw new Error(`global_open_nonactivation_failed:${label}`);
  return result;
}

function addRow(id: string, before: Json, action: Json, after: Json, ingressVerified: boolean, postconditionVerified: boolean, extra: Json = {}): void {
  const deliveryEvidence = {
    injectorAccepted: action.helperEvidenceValid === true,
    ingressVerified,
    postconditionVerified,
    deliveryScope: postconditionVerified ? "postcondition" : ingressVerified ? "ingress" : "injector",
    delivered: action.helperEvidenceValid === true || ingressVerified || postconditionVerified,
    settleMs: 50,
    settleIsProof: false,
  };
  receipt.rows.push({
    id,
    required: true,
    before: digest(before),
    action,
    after: digest(after),
    deliveryEvidence,
    passed: action.helperEvidenceValid === true && ingressVerified && postconditionVerified,
    ...extra,
  });
}

function flatten(input: any, output: Json[] = []): Json[] {
  if (Array.isArray(input)) for (const item of input) flatten(item, output);
  else if (input && typeof input === "object") {
    if (typeof input.semanticId === "string") output.push(input);
    for (const nested of Object.values(input)) if (nested && typeof nested === "object") flatten(nested, output);
  }
  return output;
}

async function stableElements(): Promise<Json> {
  const deadline = Date.now() + 3_000;
  let last = getElements();
  let previous = "";
  let same = 0;
  while (Date.now() < deadline) {
    const ids = flatten(last.elements ?? []).map((node) => `${node.semanticId}|${node.selected === true}|${node.disabled === true}`);
    const current = JSON.stringify({ ids, selectedSemanticId: last.selectedSemanticId ?? null });
    if (current === previous) same += 1; else same = 1;
    if (same >= 3) return last;
    previous = current;
    await Bun.sleep(25);
    last = getElements();
  }
  throw new Error("getElements_did_not_stabilize");
}

function selectionPlan(elements: Json): Json {
  const seen = new Set<string>();
  const rows = flatten(elements.elements ?? []).filter((node) => {
    const id = String(node.semanticId ?? "");
    if (!id.startsWith("choice:") || node.disabled === true || node.visible === false || seen.has(id)) return false;
    seen.add(id);
    return true;
  });
  const selected = String(elements.selectedSemanticId ?? rows.find((row) => row.selected === true)?.semanticId ?? "");
  const index = rows.findIndex((row) => row.semanticId === selected);
  if (index < 0 || index + 1 >= rows.length) throw new Error(`cannot_derive_exact_next_selectable:${JSON.stringify({ selected, ids: rows.map((row) => row.semanticId) })}`);
  return { selectedBefore: selected, expectedNext: rows[index + 1]!.semanticId, selectableIds: rows.map((row) => row.semanticId) };
}

function nativeArtifactSpecs(): ArtifactSpec[] {
  const correlations: ProtocolCorrelation[] = receipt.protocolCorrelations.map((item: Json) => ({
    requestId: item.requestId,
    expectedType: item.expectedType,
  }));
  return [
    {
      id: "app-log",
      sourceName: "app.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
      acceptedTextMarkers: ["STARTUP_READY ", "APP_READY|"],
    },
    {
      id: "responses",
      sourceName: "responses.ndjson",
      required: true,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      correlations: correlations.map((correlation) => ({
        ...correlation,
        requireNestedResponse: true,
      })),
    },
    {
      id: "protocol-responses",
      sourceName: "protocol-responses.ndjson",
      required: true,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      correlations,
    },
    {
      id: "lifecycle",
      sourceName: "lifecycle.json",
      required: true,
      mediaType: "application/json",
      kind: "json",
    },
    ...SESSION_EVIDENCE_FILES.filter((name) => ![
      "app.log",
      "responses.ndjson",
      "protocol-responses.ndjson",
    ].includes(name)).map((name): ArtifactSpec => ({
      id: `raw-${name}`,
      sourceName: name,
      required: false,
      mediaType: name.endsWith(".json")
        ? "application/json"
        : name.endsWith(".ndjson")
          ? "application/x-ndjson"
          : "text/plain",
      kind: name.endsWith(".json") ? "json" : name.endsWith(".ndjson") ? "ndjson" : "text",
      requireNonEmpty: false,
    })),
  ];
}

function sessionPid(name: "supervisor_pid" | "fwd_pid"): number | null {
  try {
    const pid = Number(readFileSync(join(dir, name), "utf8").trim());
    return Number.isInteger(pid) && pid > 0 ? pid : null;
  } catch {
    return null;
  }
}

try {
  if (mode !== "p0.2-helper") throw new Error(`unsupported_mode:${mode}`);
  if (!/^[a-f0-9]{64}$/.test(expectedSha)) throw new Error("expected_binary_sha_must_be_lowercase_64_hex");
  if (!/^[a-f0-9]{64}$/.test(expectedHelperSha)) throw new Error("expected_helper_sha_must_be_lowercase_64_hex");
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) throw new Error("timeout_must_be_positive");
  if ((statSync(root).mode & 0o777) !== 0o700) throw new Error("private_session_root_mode_not_0700");
  if (!existsSync(binary) || !statSync(binary).isFile() || (statSync(binary).mode & 0o111) === 0) throw new Error(`pinned_executable_missing:${binary}`);
  if (!existsSync(helper) || !statSync(helper).isFile()) throw new Error(`helper_source_missing:${helper}`);

  const beforeHash = sha(binary);
  receipt.binaryVerification.before = { sha256: beforeHash, matchesExpected: beforeHash === expectedSha };
  if (beforeHash !== expectedSha) throw new Error(`binary_hash_mismatch:${beforeHash}`);
  const helperBefore = verifyHelperHash("probe-start");
  receipt.helperVerification.before = { sha256: helperBefore.sha256, matchesExpected: helperBefore.matchesExpected };

  const beforeStatus = runSession(["status", session], (envelope) => envelope?.status === "not_found" && envelope?.session === session, "status-before");
  receipt.session.statusBefore = beforeStatus.envelope;
  const start = runSession(["start", session], (envelope) => envelope?.status === "ok" && envelope?.session === session, "start");
  started = true;
  receipt.session.start = start.envelope;
  ownedPid = Number(start.envelope.pid);
  ownedGeneration = typeof start.envelope.sessionGeneration === "string" ? start.envelope.sessionGeneration : null;
  if (start.envelope.resumed !== false || !Number.isInteger(ownedPid) || ownedPid! <= 0 || !ownedGeneration) throw new Error(`invalid_start_ownership:${JSON.stringify(start.envelope)}`);
  receipt.session.ownershipAfterStart = ownership("after-start");

  const open = await openGlobal("initial-open");
  const opened = await poll((state) => strictMain(state, "", true), "global-Cmd-semicolon-open");
  receipt.open = { ...open, state: digest(opened), postconditionVerified: strictMain(opened, "", true) };

  const markerBefore = opened;
  const markerAction = helperAction("type", marker);
  const markerAfter = await poll((state) => strictMain(state, marker, true), "exact-marker");
  const markerPostcondition = strictMain(markerAfter, marker, true);
  addRow("exact-marker", markerBefore, markerAction, markerAfter, markerPostcondition, markerPostcondition, { exactMarker: marker });

  const escape1 = helperAction("key", "Escape");
  const afterEscape1 = await poll((state) => strictMain(state, "", true), "first-Escape-clears-visible");
  const escape1Postcondition = strictMain(afterEscape1, "", true);
  addRow("escape-clear-visible", markerAfter, escape1, afterEscape1, escape1Postcondition, escape1Postcondition);

  const escape2 = helperAction("key", "Escape");
  const afterEscape2 = await poll((state) => strictMain(state, "", false), "second-Escape-hides");
  const escape2Postcondition = strictMain(afterEscape2, "", false);
  addRow("escape-hide", afterEscape1, escape2, afterEscape2, escape2Postcondition, escape2Postcondition);

  const reopen = await openGlobal("reopen-after-hide");
  const reopened = await poll((state) => strictMain(state, "", true), "reopen-after-hide");
  receipt.reopen = { ...reopen, state: digest(reopened), postconditionVerified: strictMain(reopened, "", true) };
  const secondMarkerAction = helperAction("type", marker);
  const secondMarker = await poll((state) => strictMain(state, marker, true), "second-exact-marker");
  receipt.secondMarker = { action: secondMarkerAction, state: digest(secondMarker), postconditionVerified: strictMain(secondMarker, marker, true) };
  if (!receipt.secondMarker.postconditionVerified) throw new Error("second_marker_setup_failed");

  const deleteAction = helperAction("key", "Delete");
  const deleteExpected = [...marker].slice(0, -1).join("");
  const afterDelete = await poll((state) => strictMain(state, deleteExpected, true), "Delete-removes-one-Unicode-scalar");
  const deletePostcondition = strictMain(afterDelete, deleteExpected, true);
  addRow("delete-one-char", secondMarker, deleteAction, afterDelete, deletePostcondition, deletePostcondition, { expectedInput: deleteExpected, removedUnicodeScalars: 1 });

  const clearAction = helperAction("key", "Escape");
  const cleared = await poll((state) => strictMain(state, "", true), "clear-before-Down");
  receipt.downPreparation = { action: clearAction, state: digest(cleared), postconditionVerified: strictMain(cleared, "", true) };
  if (!receipt.downPreparation.postconditionVerified) throw new Error("down_preparation_failed");
  const elementsBefore = await stableElements();
  const plan = selectionPlan(elementsBefore);
  const downAction = helperAction("key", "Down");
  const deadline = Date.now() + 3_000;
  let elementsAfter = await stableElements();
  let selectedAfter = String(elementsAfter.selectedSemanticId ?? flatten(elementsAfter.elements ?? []).find((row) => row.selected === true)?.semanticId ?? "");
  while (selectedAfter !== plan.expectedNext && Date.now() < deadline) {
    elementsAfter = await stableElements();
    selectedAfter = String(elementsAfter.selectedSemanticId ?? flatten(elementsAfter.elements ?? []).find((row) => row.selected === true)?.semanticId ?? "");
  }
  const afterDown = getState();
  const downPostcondition = selectedAfter === plan.expectedNext && strictMain(afterDown, "", true);
  addRow("down-next-semantic-id", cleared, downAction, afterDown, downPostcondition, downPostcondition, {
    selectionPlan: plan,
    selectedAfter,
    elementsResponseType: elementsAfter.type,
  });

  const cliclick = ["/opt/homebrew/bin/cliclick", "/usr/local/bin/cliclick"].find(existsSync);
  const optional = cliclick ? command(cliclick, ["ku:cmd"], true).action : { ok: false, blocker: "cliclick_not_found" };
  receipt.rows.push({ id: "ku:cmd-diagnostic", required: false, diagnosticOnly: true, action: optional, passed: optional.ok === true });
} catch (error) {
  fatal = error;
  const message = boundedTail(error instanceof Error ? error.message : String(error));
  receipt.blockers.push(message);
  receipt.blockerKind = receipt.session.start?.ready === false && message.includes("response_timeout")
    ? "blocked-by-sandbox"
    : message.includes("response_timeout") ? "blocked-by-session-lifecycle" : "runtime-blocker";
} finally {
  if (started && ownedPid) {
    try {
      receipt.cleanup.ownershipBeforeHide = ownership("immediately-before-hide");
      const current = getState();
      if (current.windowVisible === true) {
        const hide = rpc({ type: "hide" }, "windowVisibilityAck");
        const hidden = await poll((state) => state.windowVisible === false, "cleanup-hide");
        receipt.cleanup.hide = { response: hide, hiddenState: digest(hidden), postconditionVerified: hidden.windowVisible === false };
      } else {
        receipt.cleanup.hide = { skipped: true, reason: "already_hidden", postconditionVerified: true };
      }
    } catch (error) {
      const message = boundedTail(error instanceof Error ? error.message : String(error));
      receipt.cleanup.hideError = message;
      receipt.blockers.push(message);
    }

    if (!unsafeOwnership) {
      try {
        receipt.cleanup.ownershipBeforeStop = ownership("immediately-before-stop");
        writerPids = {
          app: ownedPid,
          supervisor: sessionPid("supervisor_pid"),
          forwarder: sessionPid("fwd_pid"),
        };
        artifactSpecs = nativeArtifactSpecs();
        stagingDir = createOwnedStagingDirectory(outputClaim, {
          name: "retained-artifacts",
          anchor: approveStagingAnchor(outputClaim, root),
        });
        retained = retainLiveSessionArtifacts(
          outputClaim,
          dir,
          stagingDir,
          artifactSpecs.filter((spec) => spec.id !== "lifecycle"),
        );
        receipt.cleanup.retention = {
          completed: true,
          retainedIds: retained.map((artifact) => artifact.id),
          sameInodesVerified: true,
        };
        receipt.cleanup.stopSafety = {
          p01StrictStopAvailable: true,
          privateSessionRoot: true,
          rootMode: "0700",
          uniqueSessionName: true,
          ownershipCheckedImmediatelyBeforeStop: true,
          ownershipExact: receipt.cleanup.ownershipBeforeStop.exact === true,
          atomicOwnershipCheckAndStop: true,
          strategy: "session-stop-expected-pid-generation",
        };
        const stop = runSession(
          [
            "stop",
            session,
            "--expected-pid",
            String(ownedPid),
            "--expected-generation",
            String(ownedGeneration),
          ],
          (envelope) => envelope?.status === "ok"
            && envelope?.session === session
            && typeof envelope?.wasRunning === "boolean"
            && envelope?.ownershipVerified === true
            && envelope?.expectedPid === ownedPid
            && envelope?.actualPid === ownedPid
            && envelope?.expectedGeneration === ownedGeneration
            && envelope?.actualGeneration === ownedGeneration,
          "stop",
        );
        receipt.cleanup.stop = stop;
        receipt.cleanup.wasRunning = stop.envelope.wasRunning === true;
        receipt.cleanup.forcedKill = stop.envelope.forcedKill ?? null;
        writersDead = await waitForProcessesDead(writerPids, { timeoutMs });
        receipt.cleanup.pidDead = writersDead.app === true;
        receipt.cleanup.supervisorDead = writersDead.supervisor === true;
        receipt.cleanup.forwarderDead = writersDead.forwarder === true;
        const afterStatus = runSession(["status", session], (envelope) => envelope?.status === "not_found" && envelope?.session === session, "status-after-stop");
        receipt.cleanup.statusAfterStop = afterStatus.envelope;
        receipt.cleanup.notFound = afterStatus.envelope.status === "not_found";
      } catch (error) {
        const message = boundedTail(error instanceof Error ? error.message : String(error));
        receipt.cleanup.stopError = message;
        receipt.blockers.push(message);
      }
    } else {
      receipt.cleanup.stop = { skipped: true, reason: "ownership_mismatch_no_name_based_stop_issued" };
      receipt.cleanup.stopSafety = {
        p01StrictStopAvailable: true,
        privateSessionRoot: true,
        rootMode: "0700",
        uniqueSessionName: true,
        ownershipCheckedImmediatelyBeforeStop: false,
        ownershipExact: false,
        atomicOwnershipCheckAndStop: true,
        strategy: "session-stop-expected-pid-generation",
      };
    }
  }

  try {
    const afterHash = existsSync(binary) ? sha(binary) : null;
    receipt.binaryVerification.after = {
      sha256: afterHash,
      matchesExpected: afterHash === expectedSha,
      unchanged: afterHash === receipt.binaryVerification.before?.sha256,
    };
  } catch (error) { receipt.blockers.push(boundedTail(error instanceof Error ? error.message : String(error))); }
  try {
    const afterHash = existsSync(helper) ? sha(helper) : null;
    receipt.helperVerification.after = {
      sha256: afterHash,
      matchesExpected: afterHash === expectedHelperSha,
      unchanged: afterHash === receipt.helperVerification.before?.sha256,
    };
  } catch (error) { receipt.blockers.push(boundedTail(error instanceof Error ? error.message : String(error))); }

  const writersFinalized = receipt.cleanup.stop?.envelope?.ownershipVerified === true
    && receipt.cleanup.notFound === true
    && Object.values(writerPids).every((pid) => Number.isInteger(pid) && pid! > 0)
    && writersDead.app === true
    && writersDead.supervisor === true
    && writersDead.forwarder === true;
  if (writersFinalized) {
    try {
      for (const retainedArtifact of retained) {
        const spec = artifactSpecs.find((candidate) => candidate.id === retainedArtifact.id)!;
        materializeAtomic(outputClaim, {
          sourceRoot: stagingDir!,
          sourceName: spec.destinationName ?? spec.sourceName,
          destinationName: spec.destinationName ?? spec.sourceName,
        });
      }
      writeJsonArtifactAtomic(
        outputClaim,
        "lifecycle.json",
        {
          schemaVersion: 1,
          probeId: "native-escape-delivery-probe",
          runId: outputClaim.owner.runId,
          finalizationKind: "strict-session-stop",
          hidden: receipt.cleanup.hide?.postconditionVerified === true,
          app: { pid: writerPids.app, dead: writersDead.app === true },
          supervisor: { pid: writerPids.supervisor, dead: writersDead.supervisor === true },
          forwarder: { pid: writerPids.forwarder, dead: writersDead.forwarder === true },
          ownership: receipt.cleanup.ownershipBeforeStop,
          stop: {
            wasRunning: receipt.cleanup.wasRunning,
            forcedKill: receipt.cleanup.forcedKill,
            finalStatus: receipt.cleanup.statusAfterStop?.status ?? null,
          },
          completedAt: new Date().toISOString(),
        },
      );
    } catch (error) {
      const message = `artifact_materialization_failed:${boundedTail(error instanceof Error ? error.message : String(error))}`;
      receipt.cleanup.artifactError = message;
      receipt.blockers.push(message);
    }
  }
  if (artifactSpecs.length === 0) artifactSpecs = nativeArtifactSpecs();
  artifactReceipts = artifactSpecs.map((spec) =>
    validateArtifact(
      join(outputClaim.artifactsRoot, spec.destinationName ?? spec.sourceName),
      spec,
      outputClaim.artifactsRoot,
    )
  );
  receipt.artifactLifecycle = buildArtifactLifecycle({
    claim: outputClaim,
    finalizationKind: "strict-session-stop",
    writersFinalized,
    specs: artifactSpecs,
    artifacts: artifactReceipts,
  });

  const requiredRows = receipt.rows.filter((row: Json) => row.required === true);
  const requiredIds = requiredRows.map((row: Json) => row.id);
  const allRequired = JSON.stringify(requiredIds) === JSON.stringify(receipt.requiredRows)
    && requiredRows.every((row: Json) => row.passed === true
      && row.deliveryEvidence?.injectorAccepted === true
      && row.deliveryEvidence?.ingressVerified === true
      && row.deliveryEvidence?.postconditionVerified === true
      && row.deliveryEvidence?.deliveryScope === "postcondition"
      && row.deliveryEvidence?.delivered === true
      && row.deliveryEvidence?.settleMs === 50
      && row.deliveryEvidence?.settleIsProof === false);
  const exactOwnership = receipt.session.ownershipAfterStart?.exact === true
    && receipt.cleanup.ownershipBeforeHide?.exact === true
    && receipt.cleanup.ownershipBeforeStop?.exact === true;
  const allHelperActions = receipt.helperActions.length === 7
    && receipt.helperActions.every((action: Json) => action.helperEvidenceValid === true
      && action.probeNonactivation?.verified === true
      && action.envelope?.data?.focusEvidence?.nonactivation?.verified === true);
  const exactCorrelations = receipt.protocolCorrelations.length > 0
    && receipt.protocolCorrelations.every((correlation: Json) => correlation.exact === true
      && correlation.innerRequestIdExact === true && correlation.innerTypeExact === true);
  const openProofs = receipt.open?.nonactivation?.verified === true && receipt.open?.postconditionVerified === true
    && receipt.reopen?.nonactivation?.verified === true && receipt.reopen?.postconditionVerified === true;
  const hashesExact = receipt.binaryVerification.after?.matchesExpected === true
    && receipt.binaryVerification.after?.unchanged === true
    && receipt.helperVerification.after?.matchesExpected === true
    && receipt.helperVerification.after?.unchanged === true;
  const cleanupExact = exactOwnership
    && receipt.cleanup.hide?.postconditionVerified === true
    && receipt.cleanup.wasRunning === true
    && receipt.cleanup.forcedKill === false
    && receipt.cleanup.pidDead === true
    && receipt.cleanup.supervisorDead === true
    && receipt.cleanup.forwarderDead === true
    && receipt.cleanup.notFound === true
    && receipt.cleanup.stopSafety?.privateSessionRoot === true
    && receipt.cleanup.stopSafety?.rootMode === "0700"
    && receipt.cleanup.stopSafety?.ownershipCheckedImmediatelyBeforeStop === true
    && receipt.cleanup.stopSafety?.ownershipExact === true
    && receipt.cleanup.stopSafety?.atomicOwnershipCheckAndStop === true
    && receipt.cleanup.stop?.envelope?.ownershipVerified === true
    && receipt.cleanup.stop?.envelope?.expectedPid === ownedPid
    && receipt.cleanup.stop?.envelope?.actualPid === ownedPid
    && receipt.cleanup.stop?.envelope?.expectedGeneration === ownedGeneration
    && receipt.cleanup.stop?.envelope?.actualGeneration === ownedGeneration;

  const artifactsExact = receipt.artifactLifecycle.allRequiredValid === true
    && receipt.artifactLifecycle.allRecordedPathsReadable === true;
  receipt.ok = fatal === null && receipt.blockers.length === 0 && allRequired && allHelperActions
    && exactCorrelations && openProofs && hashesExact && cleanupExact && artifactsExact;
  receipt.classification = receipt.ok ? "fixed" : "blocked-by-unsafe-operation";
  receipt.summary = {
    requiredRowsFound: requiredRows.length,
    requiredRowsExpected: receipt.requiredRows.length,
    requiredIds,
    allRequired,
    allHelperActions,
    exactCorrelations,
    openProofs,
    exactOwnership,
    hashesExact,
    cleanupExact,
    artifactsExact,
  };
  if (receipt.ok) {
    const durablePathsOutsidePrivateRoot = [
      outputClaim.receiptPath,
      ...receipt.artifactLifecycle.recordedPaths,
    ].every((path: string) => !isStrictDescendant(root, resolve(path)) && resolve(path) !== resolve(root));
    if (!durablePathsOutsidePrivateRoot) {
      receipt.ok = false;
      receipt.classification = "blocked-by-unsafe-operation";
      receipt.blockers.push("durable_artifact_inside_private_root");
    }
  }
  if (receipt.ok) {
    const start = receipt.session.start;
    receipt.session = {
      name: session,
      pid: ownedPid,
      generation: ownedGeneration,
      resumed: start?.resumed ?? null,
      ready: start?.ready ?? null,
      readyWaitMs: start?.readyWaitMs ?? null,
      readyMarker: start?.readyMarker ?? null,
    };
    delete receipt.failureArtifacts;
    try {
      removeOwnedAuxiliaryDirectory(outputClaim, root);
      receipt.cleanup.privateRunRootRemoved = true;
    } catch (error) {
      receipt.ok = false;
      receipt.classification = "blocked-by-unsafe-operation";
      receipt.blockers.push(
        `private_root_removal_failed:${boundedTail(error instanceof Error ? error.message : String(error))}`,
      );
    }
  }
  if (!receipt.ok) {
    const preservedPaths = [outputClaim.root, root];
    if (stagingDir && existsSync(stagingDir)) preservedPaths.push(stagingDir);
    if (existsSync(dir)) preservedPaths.push(dir);
    receipt.failurePreservation = {
      outputRootPreserved: true,
      sessionRootPreserved: existsSync(dir),
      stagingPreserved: Boolean(stagingDir && existsSync(stagingDir)),
      paths: preservedPaths,
      reason: receipt.blockers.join("; ") || "native probe failed",
    };
    receipt.cleanup.preservedSessionRoot = root;
    receipt.cleanup.failureArtifactsPreserved = existsSync(failureArtifactsDir);
  }
  receipt.completedAt = new Date().toISOString();
  commitFinalReceipt(outputClaim, receipt, artifactSpecs, artifactReceipts);
}

console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.ok ? 0 : 1);
