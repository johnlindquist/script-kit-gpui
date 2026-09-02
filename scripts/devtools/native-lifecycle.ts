#!/usr/bin/env bun
import { realpathSync, watch } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { homedir } from "node:os";
import {
  assertOutputOwnership, canonicalJson, claimOutput, createOwnedStagingDirectory, isStrictDescendant,
  managedTaskRecordPath, readManagedTask, validateOutputTarget, waitForProcessesDead,
  type ManagedTask, type OutputClaim, type OwnedCleanup, type OwnedProcessIdentity, type TaskRecord,
} from "../agentic/artifact-lifecycle.ts";
import { isCompilerIdentityEnvironmentVariable, type ArtifactReference } from "../agentic/build-artifact.ts";
import { spawnOwnedProcess, validateNativeLifecycle, type NativeLifecycleObservation, type OwnedProcess } from "../agentic/owned-process.ts";
import { DriverLifecycleError, unknownOwnedCleanup, type Json } from "./driver.ts";
import { OwnedEvaluationClient, EvaluationContractError } from "./lib/owned-evaluation.ts";
import { completedFrameIssues, type AutomationInstance, type CompletedFrameIdentity, type OwnedRuntimeIdentity } from "./lib/target-identity.ts";

const FIXTURE = "main.script-list";
const HELPER_ARGUMENT = "--native-lifecycle-parent-loss-helper";
const MAX_HELPER_OUTPUT = 128 * 1024;
const MAX_HELPER_LINE = 32 * 1024;
const REPOSITORY_ROOT = resolve(import.meta.dir, "../..");
type CaseId = "explicit-end" | "stdin-eof" | "lifetime-expiry" | "parent-loss";
type ShutdownReason = NativeLifecycleObservation["result"]["shutdownReason"];
interface RuntimeStart {
  identity: OwnedRuntimeIdentity;
  processIdentity: OwnedProcessIdentity;
  managedTask: ManagedTask;
  launchNonce: string;
  policySha256: string;
  maxLifetimeMs: number;
}
interface NativeRoot extends RuntimeStart {
  target: AutomationInstance;
  frame: CompletedFrameIdentity;
}
interface Campaign {
  observations: Json[];
  assertions: { id: string; pass: boolean }[];
  cleanups: OwnedCleanup[];
}

function errorText(error: unknown): string { return String(error).slice(0, 2048); }
function cleanupFrom(error: unknown): OwnedCleanup {
  if (error instanceof DriverLifecycleError) return error.cleanup;
  const cleanup = (error as { cleanup?: OwnedCleanup } | null)?.cleanup;
  return cleanup ?? unknownOwnedCleanup(false);
}
function invalidCleanup(cleanup: OwnedCleanup, code: string): OwnedCleanup {
  return { ...cleanup, closed: false, failureCodes: [...new Set([...cleanup.failureCodes, "INVALID_CLEANUP", code])] };
}
function runtimeStart(client: OwnedEvaluationClient): RuntimeStart {
  const task = client.driver.managedTask, qualification = client.driver.qualification;
  if (!task || !qualification) throw new EvaluationContractError("native_lifecycle_launch_identity_required");
  return { identity: client.identity, processIdentity: client.driver.processIdentity, managedTask: task,
    launchNonce: qualification.launchNonce, policySha256: qualification.policySha256,
    maxLifetimeMs: qualification.limits.maxLifetimeMs };
}
async function mountNativeRoot(client: OwnedEvaluationClient, start: RuntimeStart): Promise<NativeRoot> {
  const target = await client.mount(FIXTURE);
  const frame = await client.frame(target);
  return { ...start, target, frame };
}
function readExactTask(root: string, reference: ArtifactReference, start: RuntimeStart, outputRoot: string): TaskRecord {
  if (!start?.managedTask?.identity || !isStrictDescendant(outputRoot, start.managedTask.recordPath) ||
      managedTaskRecordPath(root, start.managedTask.identity) !== start.managedTask.recordPath)
    throw new EvaluationContractError("native_lifecycle_task_identity_mismatch");
  const record = readManagedTask(start.managedTask.recordPath, start.managedTask.identity);
  if (record.identity.kind !== "runtime-run" || canonicalJson(record.ownedProcesses) !== canonicalJson([start.processIdentity]) ||
      canonicalJson(record.artifactReferences) !== canonicalJson([reference]) ||
      start.identity.manifestSha256 !== reference.manifestSha256 ||
      (["pid", "processStartTime", "processInstanceId", "sessionGeneration"] as const).some(key => start.identity[key] !== start.processIdentity[key]))
    throw new EvaluationContractError("native_lifecycle_task_process_mismatch");
  return record;
}
function validateFinal(value: unknown, start: RuntimeStart): NativeLifecycleObservation {
  return validateNativeLifecycle(value, start.processIdentity, {
    launchNonce: start.launchNonce, policySha256: start.policySha256,
    binarySha256: start.identity.binarySha256, manifestSha256: start.identity.manifestSha256,
  });
}

/** Directory events survive the task authority's atomic record replacement. No reexecution or task mutation. */
function awaitTerminalTask(task: ManagedTask, timeoutMs = 20_000): Promise<TaskRecord> {
  return new Promise((accept, reject) => {
    let settled = false;
    const watcher = watch(dirname(task.recordPath));
    const timer = setTimeout(() => finish(new EvaluationContractError("native_lifecycle_terminal_task_timeout")), timeoutMs);
    const finish = (error?: unknown, record?: TaskRecord) => {
      if (settled) return;
      settled = true; clearTimeout(timer); watcher.close();
      if (error) reject(error); else accept(record!);
    };
    const inspect = () => {
      try {
        const record = readManagedTask(task.recordPath, task.identity);
        if (record.state === "closed" || record.state === "protected") finish(undefined, record);
      } catch (error) { finish(error); }
    };
    watcher.on("change", inspect); watcher.on("error", finish);
    inspect();
  });
}
async function deadline<T>(promise: Promise<T>, timeoutMs: number, code: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([promise, new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new EvaluationContractError(code)), timeoutMs);
    })]);
  } finally { clearTimeout(timer); }
}

function retainCase(campaign: Campaign, id: CaseId, expectedReason: ShutdownReason, observation: Json,
  root: NativeRoot | undefined, lifecycle: NativeLifecycleObservation | null, task: TaskRecord | undefined,
  cleanup: OwnedCleanup): void {
  const native = lifecycle?.result.native;
  const nativeClosed = lifecycle?.result.ownedWindowsClosed === true && native?.installed === true &&
    native.liveWindows === 0 && native.automationWindows === 0 && lifecycle.result.remainingWindows === 0;
  if (!nativeClosed || cleanup.ownedWindowsClosed !== true) cleanup = invalidCleanup(cleanup, "native_closure_unproved");
  if (!cleanup.closed || !cleanup.processExited || !cleanup.processGroupExited || !cleanup.streamsDrained ||
      !cleanup.logWriterClosed || !cleanup.referencesFinalized || cleanup.survivors.length)
    cleanup = invalidCleanup(cleanup, "owned_resources_not_closed");
  const checks: Record<string, boolean> = {
    "native-root-framed": !!root && completedFrameIssues(root.target, root.frame, root.identity).length === 0,
    "native-opened-positive": !!native && native.openedWindows > 0 && native.completedFrames > 0,
    "validated-native-final": lifecycle !== null,
    "expected-shutdown-reason": lifecycle?.result.shutdownReason === expectedReason,
    "native-live-and-registry-zero": nativeClosed,
    "process-and-group-exited": cleanup.processExited && cleanup.processGroupExited,
    "streams-and-log-closed": cleanup.streamsDrained && cleanup.logWriterClosed,
    "task-and-references-finalized": task?.state === "closed" && task.cleanup.closed && cleanup.referencesFinalized,
    "zero-survivors": cleanup.survivors.length === 0 && observation.processesDead !== null,
    "cleanup-valid": cleanup.closed,
    "case-completed": observation.errors.length === 0,
  };
  for (const [name, pass] of Object.entries(checks)) campaign.assertions.push({ id: `${id}:${name}`, pass });
  campaign.cleanups.push(cleanup);
  campaign.observations.push({ caseId: id, negativeOnly: true, productionEvidence: false, fixtureId: FIXTURE,
    expectedShutdownReason: expectedReason, ...observation, initialNativeRoot: root ?? null,
    nativeLifecycle: lifecycle, terminalTask: task ?? null, cleanup });
}

async function runDirectCase(campaign: Campaign, repositoryRoot: string, reference: ArtifactReference,
  claim: OutputClaim, id: Exclude<CaseId, "parent-loss">, reason: ShutdownReason): Promise<void> {
  let client: OwnedEvaluationClient | undefined, start: RuntimeStart | undefined, root: NativeRoot | undefined;
  let lifecycle: NativeLifecycleObservation | null = null, task: TaskRecord | undefined;
  let cleanup = unknownOwnedCleanup(false), processesDead: Record<string, boolean> | null = null;
  const errors: string[] = [];
  let endResponse: Json | null = null;
  try {
    client = await OwnedEvaluationClient.launch(repositoryRoot, reference, claim, [FIXTURE], "current-content",
      id === "lifetime-expiry" ? { maxLifetimeMs: 3000 } : {});
    start = runtimeStart(client);
    root = await mountNativeRoot(client, start);
    readExactTask(repositoryRoot, reference, start, claim.root);
    if (id === "explicit-end") {
      endResponse = await client.design({ operation: "end" });
      lifecycle = await client.driver.awaitNativeLifecycle(10_000);
    } else if (id === "stdin-eof") lifecycle = await client.driver.closeInput(10_000);
    else lifecycle = await client.driver.awaitNativeLifecycle(10_000);
  } catch (error) { errors.push(errorText(error)); if (!client) cleanup = cleanupFrom(error); }
  finally {
    if (client) {
      try { cleanup = await client.close(); }
      catch (error) { errors.push(errorText(error)); cleanup = client.cleanup; }
      lifecycle = client.driver.nativeLifecycle;
    }
    if (start) {
      try {
        task = readExactTask(repositoryRoot, reference, start, claim.root);
        if (lifecycle) lifecycle = validateFinal(lifecycle, start);
        if (canonicalJson(task.result.nativeLifecycle ?? null) !== canonicalJson(lifecycle))
          throw new EvaluationContractError("native_lifecycle_retained_final_mismatch");
      } catch (error) { errors.push(errorText(error)); cleanup = invalidCleanup(cleanup, "task_or_native_identity_unproved"); }
      try { processesDead = await waitForProcessesDead({ native: start.processIdentity.pid, supervisor: start.processIdentity.supervisorPid }); }
      catch (error) {
        errors.push(errorText(error));
        cleanup = invalidCleanup({ ...cleanup, survivors: [...cleanup.survivors,
          { kind: "native-process-or-supervisor", identity: canonicalJson(start.processIdentity), observation: "unknown" }] }, "native_process_death_unproved");
      }
    }
  }
  retainCase(campaign, id, reason, { errors, endResponse, launch: start ?? null, processesDead }, root, lifecycle, task, cleanup);
}

async function consumeHelperOutput(stream: ReadableStream<Uint8Array>, onMessage: (message: Json) => void): Promise<void> {
  const reader = stream.getReader(), decoder = new TextDecoder();
  let pending = "", bytes = 0;
  try {
    for (;;) {
      const next = await reader.read();
      if (next.done) break;
      bytes += next.value.byteLength;
      if (bytes > MAX_HELPER_OUTPUT) throw new EvaluationContractError("native_helper_output_limit");
      pending += decoder.decode(next.value, { stream: true });
      let newline: number;
      while ((newline = pending.indexOf("\n")) !== -1) {
        if (newline > MAX_HELPER_LINE) throw new EvaluationContractError("native_helper_line_limit");
        const message = JSON.parse(pending.slice(0, newline)); pending = pending.slice(newline + 1);
        if (!message || message.schemaVersion !== 1 || message.negativeOnly !== true || message.productionEvidence !== false ||
            !["nativeLifecycleStarted", "nativeLifecycleReady", "nativeLifecycleFinished"].includes(message.type))
          throw new EvaluationContractError("native_helper_protocol_invalid");
        onMessage(message);
      }
      if (pending.length > MAX_HELPER_LINE) throw new EvaluationContractError("native_helper_line_limit");
    }
    if (pending || decoder.decode()) throw new EvaluationContractError("native_helper_output_truncated");
  } finally { reader.releaseLock(); }
}

async function runParentLoss(campaign: Campaign, repositoryRoot: string, reference: ArtifactReference, claim: OutputClaim): Promise<void> {
  let helper: OwnedProcess | undefined, start: RuntimeStart | undefined, root: NativeRoot | undefined;
  let lifecycle: NativeLifecycleObservation | null = null, task: TaskRecord | undefined;
  let cleanup = unknownOwnedCleanup(false), helperCleanup = unknownOwnedCleanup(false);
  let processesDead: Record<string, boolean> | null = null, helperExitCode: number | null = null;
  let stdout: Promise<void> | undefined, stderr: Promise<string> | undefined;
  let readinessAccepted = false, terminationRequested = false;
  const errors: string[] = [], messages: Json[] = [];
  const childRoot = join(claim.root, `native-parent-loss-${randomUUID()}`);
  const ready = Promise.withResolvers<Json>();
  // Handle an early EOF before the bounded readiness await has attached its consumer.
  void ready.promise.catch(() => {});
  try {
    assertOutputOwnership(claim);
    const sandbox = createOwnedStagingDirectory(claim, { name: `native-helper-${randomUUID()}` });
    const rustup = Bun.which("rustup");
    if (!rustup) throw new EvaluationContractError("native_helper_toolchain_unavailable");
    // Retain the compiler identity for current-content verification, not the operator environment.
    // The native child still receives its separate credential-free ownedEvaluationEnvironment.
    const rustupHome = realpathSync(process.env.RUSTUP_HOME ?? join(homedir(), ".rustup"));
    const cargoHome = resolve(process.env.CARGO_HOME ?? join(homedir(), ".cargo"));
    const compilerEnvironment: Record<string, string> = {};
    for (const [name, value] of Object.entries(process.env)) {
      if (typeof value === "string" && isCompilerIdentityEnvironmentVariable(name)) compilerEnvironment[name] = value;
    }
    // Fixed source entrypoint and shared compiler allowlist; no inherited credentials or runtime configuration.
    helper = await spawnOwnedProcess({ argv: [process.execPath, import.meta.path, HELPER_ARGUMENT,
      repositoryRoot, childRoot, reference.manifestPath, reference.manifestSha256], cwd: repositoryRoot,
      env: { ...compilerEnvironment, PATH: `${dirname(realpathSync(rustup))}:/usr/bin:/bin:/usr/sbin:/sbin`, RUSTUP_HOME: rustupHome, CARGO_HOME: cargoHome,
        LANG: "en_US.UTF-8", TZ: "UTC", HOME: sandbox,
        TMPDIR: sandbox, XDG_CONFIG_HOME: sandbox, XDG_DATA_HOME: sandbox, XDG_CACHE_HOME: sandbox,
        SK_PATH: sandbox, CODEX_HOME: sandbox, SCRIPT_KIT_NONINTERACTIVE: "1" },
      timeoutMs: 30_000, maxOutputBytes: MAX_HELPER_OUTPUT });
    stderr = new Response(helper.stderr).text();
    void stderr.catch(() => {});
    stdout = consumeHelperOutput(helper.stdout, message => {
      messages.push(message);
      if (messages.length > 3) throw new EvaluationContractError("native_helper_message_limit");
      if (message.type === "nativeLifecycleStarted") {
        if (start) throw new EvaluationContractError("native_helper_duplicate_start");
        start = message.launch as RuntimeStart;
      } else if (message.type === "nativeLifecycleReady") ready.resolve(message);
      else {
        cleanup = message.cleanup as OwnedCleanup;
        ready.reject(new EvaluationContractError("native_helper_finished_before_termination"));
      }
    });
    void stdout.then(() => ready.reject(new EvaluationContractError("native_helper_eof_before_readiness")), error => ready.reject(error));
    const message = await deadline(ready.promise, 20_000, "native_helper_readiness_timeout");
    root = message.root as NativeRoot;
    const { target: _target, frame: _frame, ...readyStart } = root;
    if (!start || canonicalJson(readyStart) !== canonicalJson(start) ||
        message.helper?.pid !== helper.pid || message.helper.processStartTime !== helper.identity.processStartTime ||
        message.helper.processInstanceId !== helper.identity.processInstanceId || message.helper.sessionGeneration !== helper.identity.sessionGeneration ||
        root.processIdentity.pid === helper.pid || root.processIdentity.supervisorPid === helper.supervisorPid ||
        completedFrameIssues(root.target, root.frame, root.identity).length)
      throw new EvaluationContractError("native_helper_readiness_identity_mismatch");
    const running = readExactTask(repositoryRoot, reference, start, childRoot);
    if (running.state !== "running" || running.result.nativeLifecycle)
      throw new EvaluationContractError("native_helper_not_live_at_readiness");
    readinessAccepted = true;
    // Only the owned helper group is stopped. The native supervisor owns a separate session.
    terminationRequested = true;
    helperCleanup = await helper.close();
    helperExitCode = await helper.exited;
    if (helperExitCode !== -15 && helperExitCode !== -9) throw new EvaluationContractError("native_helper_parent_death_not_observed");
  } catch (error) { errors.push(errorText(error)); if (!helper) helperCleanup = cleanupFrom(error); }
  finally {
    if (helper) {
      if (!terminationRequested) {
        // A failed readiness orchestration is not permission to manufacture parent loss.
        // Let the helper's native lifetime/finally close normally; its owner also has a hard deadline.
        try { await deadline(helper.exited, 35_000, "native_helper_bounded_exit_timeout"); }
        catch (error) { errors.push(errorText(error)); }
      }
      try { helperCleanup = await helper.close(); helperExitCode = await helper.exited; }
      catch (error) { errors.push(errorText(error)); helperCleanup = invalidCleanup(helperCleanup, "native_helper_cleanup_failed"); }
    }
    if (stdout) {
      try { await stdout; } catch (error) { errors.push(errorText(error)); }
    }
    if (stderr) {
      try { const text = await stderr; if (text) errors.push(`native_helper_stderr:${text.slice(0, 2048)}`); }
      catch (error) { errors.push(errorText(error)); }
    }
    if (start) {
      try {
        readExactTask(repositoryRoot, reference, start, childRoot);
        await awaitTerminalTask(start.managedTask);
        task = readExactTask(repositoryRoot, reference, start, childRoot);
        cleanup = task.cleanup;
        lifecycle = validateFinal(task.result.nativeLifecycle, start);
      } catch (error) { errors.push(errorText(error)); cleanup = invalidCleanup(task?.cleanup ?? cleanup, "supervisor_native_finalization_unproved"); }
      try {
        processesDead = await waitForProcessesDead({ native: start.processIdentity.pid, nativeSupervisor: start.processIdentity.supervisorPid,
          helper: helper?.pid, helperSupervisor: helper?.supervisorPid });
      } catch (error) {
        errors.push(errorText(error));
        cleanup = invalidCleanup({ ...cleanup, survivors: [...cleanup.survivors,
          { kind: "native-process-or-supervisor", identity: canonicalJson(start.processIdentity), observation: "unknown" }] }, "native_process_death_unproved");
      }
    }
  }
  if (!helperCleanup.closed) helperCleanup = invalidCleanup(helperCleanup, "native_helper_cleanup_unproved");
  campaign.cleanups.push(helperCleanup);
  campaign.assertions.push({ id: "parent-loss:exact-ready-helper-terminated", pass: readinessAccepted && terminationRequested &&
    (helperExitCode === -15 || helperExitCode === -9) });
  campaign.assertions.push({ id: "parent-loss:helper-cleanup-valid", pass: helperCleanup.closed && helperCleanup.survivors.length === 0 });
  // This branch has no task mutation capability; only the surviving native supervisor can commit this terminal record.
  campaign.assertions.push({ id: "parent-loss:supervisor-finalized-exact-task", pass: readinessAccepted && terminationRequested &&
    (helperExitCode === -15 || helperExitCode === -9) && !!task && ["closed", "protected"].includes(task.state) &&
    typeof task.result.exitCode === "number" && lifecycle?.result.shutdownReason === "inputEof" });
  retainCase(campaign, "parent-loss", "inputEof", { errors, launch: start ?? null, processesDead,
    helper: helper?.identity ?? null, helperExitCode, helperCleanup, readinessAccepted, terminationRequested, messages }, root, lifecycle, task, cleanup);
}

/** Real native negative evidence only; receipt commitment remains with the design CLI. */
export async function runNativeLifecycleCampaign(repositoryRoot: string, reference: ArtifactReference, claim: OutputClaim): Promise<Campaign> {
  if (resolve(repositoryRoot) !== REPOSITORY_ROOT || claim.plan.repoRoot !== REPOSITORY_ROOT)
    throw new EvaluationContractError("native_lifecycle_repository_mismatch");
  assertOutputOwnership(claim);
  const campaign: Campaign = { observations: [], assertions: [], cleanups: [] };
  await runDirectCase(campaign, repositoryRoot, reference, claim, "explicit-end", "explicitEnd");
  await runDirectCase(campaign, repositoryRoot, reference, claim, "stdin-eof", "inputEof");
  await runDirectCase(campaign, repositoryRoot, reference, claim, "lifetime-expiry", "lifetimeExpired");
  await runParentLoss(campaign, repositoryRoot, reference, claim);
  return campaign;
}

function emitHelper(message: Json): void {
  const line = JSON.stringify({ schemaVersion: 1, negativeOnly: true, productionEvidence: false, ...message });
  if (Buffer.byteLength(line) > MAX_HELPER_LINE) throw new EvaluationContractError("native_helper_line_limit");
  process.stdout.write(`${line}\n`);
}
async function runParentLossHelper(): Promise<void> {
  const [argument, repositoryRoot, outputRoot, manifestPath, manifestSha256, ...extra] = process.argv.slice(2);
  if (argument !== HELPER_ARGUMENT || extra.length || repositoryRoot !== REPOSITORY_ROOT || !outputRoot ||
      !manifestPath || !manifestSha256 || !/^[a-f0-9]{64}$/.test(manifestSha256) || process.env.SCRIPT_KIT_NONINTERACTIVE !== "1")
    throw new EvaluationContractError("native_lifecycle_helper_arguments_invalid");
  let client: OwnedEvaluationClient | undefined;
  const errors: string[] = [];
  let cleanup = unknownOwnedCleanup(false);
  try {
    // No extra outer task: Driver binds the sole artifact reference before native exec.
    const claim = claimOutput(validateOutputTarget({ repoRoot: repositoryRoot, candidate: outputRoot,
      kind: "directory", probeId: "native-lifecycle-parent-loss" }));
    client = await OwnedEvaluationClient.launch(repositoryRoot, { manifestPath, manifestSha256 }, claim,
      [FIXTURE], "current-content", { maxLifetimeMs: 10_000 });
    const launch = runtimeStart(client);
    emitHelper({ type: "nativeLifecycleStarted", launch });
    const root = await mountNativeRoot(client, launch);
    emitHelper({ type: "nativeLifecycleReady", root, helper: { pid: process.pid,
      processStartTime: process.env.SCRIPT_KIT_PROCESS_START_TIME,
      processInstanceId: process.env.SCRIPT_KIT_PROCESS_INSTANCE_ID,
      sessionGeneration: process.env.SCRIPT_KIT_SESSION_GENERATION } });
    // If readiness delivery or parent termination fails, native lifetime still closes the root normally.
    await client.driver.awaitNativeLifecycle(10_000);
  } catch (error) { errors.push(errorText(error)); if (!client) cleanup = cleanupFrom(error); }
  finally {
    if (client) {
      try { cleanup = await client.close(); }
      catch (error) { errors.push(errorText(error)); cleanup = client.cleanup; }
    }
    emitHelper({ type: "nativeLifecycleFinished", cleanup, errors });
  }
  if (errors.length || !cleanup.closed) process.exitCode = 1;
}

if (import.meta.main) {
  try { await runParentLossHelper(); }
  catch (error) { process.stderr.write(`${errorText(error)}\n`); process.exitCode = 1; }
}
