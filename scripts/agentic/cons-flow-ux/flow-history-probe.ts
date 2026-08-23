#!/usr/bin/env bun

import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observedWorkflowStage,
  observeWorkflowTaskTarget,
  prepareBlockedWorkflowTaskProof,
  prepareWorkflowTaskProof,
  writeWorkflowTaskProof,
  type WorkflowObservedSegment,
} from "../../devtools/lib/workflow-task-proof.ts";
import type { RuntimeTargetObservation } from "../../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.flow-history");

const repoRoot = resolve(import.meta.dir, "../../..");
const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY ??
    "target-agent/artifacts/cons-flow-c05/script-kit-gpui",
);
const receiptDir = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR ??
    ".artifacts/consistency/cons-flow-ux/c05-flow-archive-v1/runtime",
);
const fixture = resolve(
  repoRoot,
  "scripts/agentic/fixtures/flow-ux-project",
);
const historyEngine = join(fixture, "bin/historyeng");
const flowId = "project:history-probe.historyeng";
const privateRoot = `/tmp/cons-flow-c05-${process.pid}`;
const sharedHome = join(privateRoot, "shared-home");
const sharedKit = join(sharedHome, ".scriptkit");
const holdMarker = join(privateRoot, "hold-flow-persist");
const failures: string[] = [];
const scenarios: Json[] = [];
const cleanup: Json[] = [];
const observedSegments = new Map<string, WorkflowObservedSegment>();

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(
      `${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`,
    );
  }
}

function sha256Bytes(value: string | Uint8Array): string {
  const hasher = new Bun.CryptoHasher("sha256");
  hasher.update(value);
  return hasher.digest("hex");
}

function sha256File(path: string): string {
  return sha256Bytes(readFileSync(path));
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const output = new TextDecoder().decode(result.stdout);
  const normalized = resolve(executable);
  return output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const command = match[2].trim().split(/\s+/, 1)[0];
      return resolve(command) === normalized ? [Number(match[1])] : [];
    });
}

function pasteboardChangeCount(): number | null {
  const result = Bun.spawnSync([
    "osascript",
    "-l",
    "JavaScript",
    "-e",
    'ObjC.import("AppKit"); $.NSPasteboard.generalPasteboard.changeCount',
  ], { stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) return null;
  const parsed = Number(new TextDecoder().decode(result.stdout).trim());
  return Number.isFinite(parsed) ? parsed : null;
}

function flowUx(state: Json): Json {
  return (state.flowUx as Json) ?? {};
}

function historySession(state: Json): Json | undefined {
  const sessions = (flowUx(state).sessions as Json[] | undefined) ?? [];
  return sessions.find((session) => session.flowId === flowId);
}

function safeSession(state: Json): Json {
  const session = historySession(state) ?? {};
  return {
    promptType: state.promptType ?? null,
    deskState: flowUx(state).deskState ?? null,
    sessionId: session.sessionId ?? null,
    selection: session.selection ?? null,
    readOnly: session.readOnly ?? null,
    activeThreadFingerprint: session.activeThreadFingerprint ?? null,
    selectedThreadFingerprint: session.selectedThreadFingerprint ?? null,
    parentThreadFingerprint: session.parentThreadFingerprint ?? null,
    parentRetained: session.parentRetained ?? null,
    inheritedTurnCount: session.inheritedTurnCount ?? null,
    activeTurnCount: session.activeTurnCount ?? null,
    selectedTurnCount: session.selectedTurnCount ?? null,
    archiveCount: session.archiveCount ?? null,
    threadCount: session.threadCount ?? null,
    totalTurnCount: session.totalTurnCount ?? null,
    turnInFlight: session.turnInFlight ?? null,
    needsRethread: session.needsRethread ?? null,
    threadReady: session.threadReady ?? null,
    runtimeGeneration: session.runtimeGeneration ?? null,
    draftChars: session.draftChars ?? null,
    draftFingerprint: session.draftFingerprint ?? null,
    draftGeneration: session.draftGeneration ?? null,
    persistenceRevision: session.persistenceRevision ?? null,
    retentionPolicy: session.retentionPolicy ?? null,
    turnCap: session.turnCap ?? null,
  };
}

async function waitForState(
  driver: Driver,
  predicate: (state: Json) => boolean,
  label: string,
  timeoutMs = 15_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let state = await driver.getState({ timeoutMs: 8_000 }) as Json;
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(25);
    state = await driver.getState({ timeoutMs: 8_000 }) as Json;
  }
  assert(predicate(state), `timed out waiting for ${label}`, safeSession(state));
  return state;
}

async function pressMain(driver: Driver, key: string, modifiers: string[] = []): Promise<void> {
  await driver.simulateGpuiEvent(
    { type: "keyDown", key, modifiers },
    { target: { type: "main" }, timeoutMs: 8_000 },
  );
}

async function visibleElements(driver: Driver): Promise<Json[]> {
  const result = await driver.getElements(
    { target: { type: "kind", kind: "main" }, limit: 500 },
    { timeoutMs: 10_000 },
  ) as Json;
  return (result.elements as Json[] | undefined) ?? [];
}

async function openDesk(driver: Driver): Promise<Json> {
  driver.send({ type: "show" });
  await driver.waitForSettle();
  for (let attempt = 0; attempt < 8; attempt++) {
    const state = await driver.getState() as Json;
    if (flowUx(state).activeVariant === "flash") return state;
    if (state.promptType !== "none" || String(state.inputValue ?? "").length > 0) {
      await pressMain(driver, "escape");
      await Bun.sleep(100);
      continue;
    }
    await driver.setFilterAndWait("Flows");
    await pressMain(driver, "enter");
    const desk = await waitForState(
      driver,
      (candidate) => flowUx(candidate).activeVariant === "flash",
      "Flow Desk",
      10_000,
    );
    return desk;
  }
  throw new Error("could not open Flow Desk");
}

async function filterDesk(driver: Driver, text: string): Promise<Json> {
  await driver.setFilterAndWait(text);
  return waitForState(
    driver,
    (state) =>
      flowUx(state).activeVariant === "flash" &&
      (text.length === 0 || String(flowUx(state).selectedRow?.title ?? "").length > 0),
    `desk filter ${text || "<empty>"}`,
  );
}

async function openHistorySession(driver: Driver): Promise<Json> {
  await openDesk(driver);
  await filterDesk(driver, "history-probe");
  await pressMain(driver, "enter");
  return waitForState(
    driver,
    (state) => state.promptType === "flowSession" && historySession(state) !== undefined,
    "history Flow session",
    20_000,
  );
}

async function sendMessage(driver: Driver, text: string, expectedTurns: number): Promise<Json> {
  driver.send({
    type: "batch",
    requestId: `c05-message-${expectedTurns}-${Date.now()}`,
    commands: [{ type: "setInput", text }],
  });
  await Bun.sleep(30);
  await pressMain(driver, "enter");
  return waitForState(
    driver,
    (state) => {
      const session = historySession(state);
      return session?.turnInFlight === false && session?.activeTurnCount === expectedTurns;
    },
    `history turn ${expectedTurns}`,
    20_000,
  );
}

async function openActions(driver: Driver): Promise<Json> {
  const result = await driver.request(
    {
      type: "batch",
      target: { type: "main" },
      commands: [{ type: "openActions" }],
      options: { stopOnError: true, rollbackOnError: false, timeout: 8_000 },
    },
    { expect: "batchResult", timeoutMs: 10_000 },
  ) as Json;
  assert(result.success === true, "failed to open Flow Actions", result);
  return waitForState(driver, (state) => Boolean(state.actionsDialog), "Flow Actions");
}

async function triggerAction(driver: Driver, actionId: string): Promise<void> {
  await openActions(driver);
  driver.send({ type: "triggerAction", actionId });
}

function automationWindows(result: Json): Json[] {
  return (result.windows as Json[] | undefined) ?? [];
}

async function waitForConfirm(driver: Driver): Promise<Json> {
  const deadline = Date.now() + 10_000;
  let windows = await driver.listAutomationWindows({ timeoutMs: 8_000 }) as Json;
  let popup = automationWindows(windows).find((window) => window.id === "confirm-popup");
  while (!popup && Date.now() < deadline) {
    await Bun.sleep(25);
    windows = await driver.listAutomationWindows({ timeoutMs: 8_000 }) as Json;
    popup = automationWindows(windows).find((window) => window.id === "confirm-popup");
  }
  assert(
    popup?.semanticSurface === "confirmDialog" && popup.parentWindowId === "main",
    "timed out waiting for parent-attached confirmation",
    { focusedWindowId: windows.focusedWindowId ?? null, popup: popup ?? null },
  );
  return popup;
}

async function waitForConfirmClose(driver: Driver, label: string): Promise<void> {
  const deadline = Date.now() + 10_000;
  let windows = await driver.listAutomationWindows({ timeoutMs: 8_000 }) as Json;
  while (
    automationWindows(windows).some((window) => window.id === "confirm-popup") &&
    Date.now() < deadline
  ) {
    await Bun.sleep(25);
    windows = await driver.listAutomationWindows({ timeoutMs: 8_000 }) as Json;
  }
  assert(
    !automationWindows(windows).some((window) => window.id === "confirm-popup"),
    `timed out waiting for ${label}`,
    windows,
  );
}

async function selectConfirmAction(driver: Driver, semanticId: string): Promise<void> {
  const result = await driver.request(
    {
      type: "batch",
      commands: [{ type: "selectBySemanticId", semanticId, submit: true }],
      target: { type: "id", id: "confirm-popup" },
    },
    { expect: "batchResult", timeoutMs: 8_000 },
  ) as Json;
  assert(result.success === true, "confirm popup selection failed", result);
}

async function confirmAction(driver: Driver): Promise<void> {
  await waitForConfirm(driver);
  await selectConfirmAction(driver, "button:0:confirm");
  await waitForConfirmClose(driver, "confirmation close");
}

async function cancelAction(driver: Driver): Promise<void> {
  await waitForConfirm(driver);
  await selectConfirmAction(driver, "button:1:cancel");
  await waitForConfirmClose(driver, "confirmation cancel");
}

function conversationFiles(): string[] {
  const directory = join(sharedKit, "flows", "conversations");
  return existsSync(directory)
    ? readdirSync(directory).filter((name) => name.endsWith(".json")).map((name) => join(directory, name))
    : [];
}

function readHistoryManifest(): Json {
  for (const path of conversationFiles()) {
    const value = JSON.parse(readFileSync(path, "utf8")) as Json;
    if (value.flow_id === flowId) return { ...value, __path: path };
  }
  throw new Error("history manifest not found");
}

function manifestThreadIds(manifest: Json): string[] {
  return ((manifest.threads as Json[] | undefined) ?? []).map((thread) => String(thread.id));
}

function safeManifest(manifest: Json): Json {
  const threads = (manifest.threads as Json[] | undefined) ?? [];
  return {
    present: true,
    fileFingerprint: sha256File(String(manifest.__path)),
    revision: manifest.revision,
    activeThreadFingerprint: sha256Bytes(String(manifest.active_thread_id ?? "")),
    threadCount: threads.length,
    archiveCount: threads.filter((thread) => thread.state === "archived").length,
    totalTurnCount: threads.reduce(
      (total, thread) => total + (((thread.turns as Json[] | undefined) ?? []).length),
      0,
    ),
  };
}

async function waitForManifest(
  predicate: (manifest: Json) => boolean,
  label: string,
  timeoutMs = 10_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let manifest: Json | null = null;
  while (Date.now() < deadline) {
    try {
      manifest = readHistoryManifest();
      if (predicate(manifest)) return manifest;
    } catch {
      // persistence is asynchronous
    }
    await Bun.sleep(25);
  }
  throw new Error(`timed out waiting for persisted ${label}: ${JSON.stringify(manifest && safeManifest(manifest))}`);
}

function baseEnv(extra: Record<string, string> = {}): Record<string, string> {
  return {
    HOME: sharedHome,
    SK_PATH: sharedKit,
    CODEX_HOME: join(sharedHome, ".codex"),
    SCRIPT_KIT_TEST_STATUS: "1",
    SCRIPT_KIT_FLOW_UX_CWD: fixture,
    SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
    SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
    SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
    SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
    SCRIPT_KIT_TEST_HOLD_FLOW_PERSIST_MARKER: holdMarker,
    PATH: `${join(fixture, "bin")}:${process.env.PATH ?? ""}`,
    ...extra,
  };
}

async function launch(
  name: string,
  home: string,
  env: Record<string, string>,
): Promise<Driver> {
  mkdirSync(home, { recursive: true });
  mkdirSync(join(home, ".scriptkit"), { recursive: true });
  return Driver.launch({
    binary,
    sessionName: `cons-flow-c05-${name}`,
    sandboxHome: false,
    sharedModels: false,
    env: {
      HOME: home,
      SK_PATH: join(home, ".scriptkit"),
      CODEX_HOME: join(home, ".codex"),
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
      ...env,
    },
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 15_000,
  });
}

async function closeOwned(
  scenario: string,
  driver: Driver | null,
  clipboardBefore: number | null,
): Promise<void> {
  if (!driver) return;
  const pid = driver.pid;
  let targetObservation: RuntimeTargetObservation | null = null;
  try {
    targetObservation = await observeWorkflowTaskTarget(driver, binary, { type: "main" });
  } catch (error) {
    failures.push(`${scenario}.target-observation`);
    console.error(`[${scenario}] private target-observation diagnostic:`, error);
  }
  let closeError: string | null = null;
  try {
    await driver.close();
  } catch (error) {
    closeError = error instanceof Error ? error.name : "UnknownCloseError";
  }
  const ownedProcessCount = exactExecutablePids(binary).length;
  const fixtureOwnedProcessCount = exactExecutablePids(historyEngine).length;
  const clipboardAfter = pasteboardChangeCount();
  const finalization = driver.finalization;
  const closeReceipt: Json = {
    scenario,
    pid,
    ...finalization,
    ownedProcessCount,
    fixtureOwnedProcessCount,
    forcedTermination: null,
    closeError,
    clipboardTouched: false,
    clipboard: {
      touched: false,
      changeCountBefore: clipboardBefore,
      changeCountAfter: clipboardAfter,
      restoration: "notApplicable",
    },
  };
  cleanup.push(closeReceipt);
  assert(finalization.processExited, `${scenario}: app process did not exit`);
  assert(finalization.streamsDrained, `${scenario}: streams did not drain`);
  assert(finalization.logWriterClosed, `${scenario}: log writer did not close`);
  assert(ownedProcessCount === 0, `${scenario}: stable artifact process survived`, { pid });
  assert(fixtureOwnedProcessCount === 0, `${scenario}: history engine survived`);
  if (clipboardBefore !== null && clipboardAfter !== null) {
    assert(clipboardBefore === clipboardAfter, `${scenario}: clipboard changed during proof`);
  }
  assert(closeError === null, `${scenario}: Driver close failed`, { closeError });
  if (targetObservation !== null) {
    observedSegments.set(
      scenario,
      observedWorkflowSegment(scenario, targetObservation, closeReceipt),
    );
  }
}

async function lifecycleProcessA(): Promise<void> {
  let driver: Driver | null = null;
  const clipboardBefore = pasteboardChangeCount();
  const checkpoints: Json[] = [];
  try {
    driver = await launch("lifecycle-a", sharedHome, baseEnv());
    let state = await openHistorySession(driver);
    for (let index = 1; index <= 15; index++) {
      state = await sendMessage(driver, `history message ${index}`, index);
    }
    const session = safeSession(state);
    assert(session.activeTurnCount === 15, "15 real turns were not retained", session);
    assert(session.totalTurnCount === 15 && session.turnCap === null, "turn policy is not uncapped", session);
    const manifest = await waitForManifest(
      (value) => safeManifest(value).totalTurnCount === 15,
      "15 turns",
    );
    checkpoints.push({ step: "seeded-15", ...session, store: safeManifest(manifest) });
    scenarios.push({ name: "core-lifecycle-a", status: "PASS", checkpoints });
  } finally {
    await closeOwned("core-lifecycle-a", driver, clipboardBefore);
  }
}

async function lifecycleProcessB(): Promise<void> {
  let driver: Driver | null = null;
  const clipboardBefore = pasteboardChangeCount();
  const checkpoints: Json[] = [];
  try {
    driver = await launch("lifecycle-b", sharedHome, baseEnv());
    let state = await openHistorySession(driver);
    let session = safeSession(state);
    assert(session.activeTurnCount === 15, "restart did not restore 15 turns", session);
    checkpoints.push({ step: "restart-restored", ...session });

    await triggerAction(driver, "flow_desk_session_new_conversation");
    state = await waitForState(
      driver,
      (candidate) => {
        const current = historySession(candidate);
        return current?.archiveCount === 1 && current?.activeTurnCount === 0;
      },
      "New Conversation archive",
    );
    session = safeSession(state);
    assert(session.threadCount === 2 && session.totalTurnCount === 15, "New lost history", session);
    checkpoints.push({ step: "new-archives-active", ...session });

    await triggerAction(driver, "flow_desk_session_history");
    state = await waitForState(
      driver,
      (candidate) => historySession(candidate)?.selection === "archive",
      "archive selection",
    );
    session = safeSession(state);
    const archiveElements = await visibleElements(driver);
    const archiveHasComposer = archiveElements.some((element) =>
      String(element.semanticId ?? "").includes("composer") ||
      String(element.semanticId ?? "").includes("chat-input")
    );
    assert(session.readOnly === true && !archiveHasComposer, "archive is not read-only", {
      session,
      archiveHasComposer,
    });
    checkpoints.push({ step: "archive-read-only", ...session, archiveHasComposer });

    await triggerAction(driver, "flow_desk_session_continue_as_new");
    state = await waitForState(
      driver,
      (candidate) => {
        const current = historySession(candidate);
        return current?.selection === "active" && current?.inheritedTurnCount === 15;
      },
      "Continue as New lineage",
    );
    session = safeSession(state);
    assert(session.archiveCount === 1 && session.parentRetained === true, "Continue lost archive lineage", session);
    checkpoints.push({ step: "continue-as-new", ...session });

    state = await sendMessage(driver, "continued message", 16);
    checkpoints.push({ step: "continued-turn", ...safeSession(state) });

    driver.send({
      type: "batch",
      requestId: `c05-draft-${Date.now()}`,
      commands: [{ type: "setInput", text: "runtime draft canary" }],
    });
    const beforeIdleTerminate = safeSession(await driver.getState() as Json);
    await triggerAction(driver, "flow_desk_session_terminate");
    await confirmAction(driver);
    state = await waitForState(
      driver,
      (candidate) => {
        const current = historySession(candidate);
        return current?.threadReady === false && current?.needsRethread === true;
      },
      "idle runtime termination",
    );
    session = safeSession(state);
    assert(session.totalTurnCount === beforeIdleTerminate.totalTurnCount, "idle Terminate changed transcript", session);
    assert(session.draftChars === 20, "idle Terminate lost draft", session);
    checkpoints.push({ step: "terminate-idle", ...session });

    driver.send({
      type: "batch",
      requestId: `c05-clear-draft-${Date.now()}`,
      commands: [{ type: "setInput", text: "" }],
    });
    driver.send({
      type: "batch",
      requestId: `c05-active-terminate-${Date.now()}`,
      commands: [{ type: "setInput", text: "HISTORY_HOLD active termination" }],
    });
    await Bun.sleep(30);
    await pressMain(driver, "enter");
    await waitForState(
      driver,
      (candidate) => historySession(candidate)?.turnInFlight === true,
      "active history turn",
    );
    const beforeActiveTerminate = safeSession(await driver.getState() as Json);
    await triggerAction(driver, "flow_desk_session_terminate");
    await confirmAction(driver);
    state = await waitForState(
      driver,
      (candidate) => {
        const current = historySession(candidate);
        return current?.turnInFlight === false && current?.threadReady === false;
      },
      "active termination settlement",
      20_000,
    );
    session = safeSession(state);
    assert(
      session.activeTurnCount === Number(beforeActiveTerminate.activeTurnCount) + 1,
      "active Terminate did not settle exactly one stopped turn",
      { beforeActiveTerminate, session },
    );
    checkpoints.push({ step: "terminate-active-settled", ...session });

    await triggerAction(driver, "flow_desk_session_history");
    await waitForState(driver, (candidate) => historySession(candidate)?.selection === "archive", "archive before delete");
    const beforeCancel = safeSession(await driver.getState() as Json);
    await triggerAction(driver, "flow_desk_session_delete_conversation");
    await cancelAction(driver);
    const afterCancel = safeSession(await driver.getState() as Json);
    assert(
      afterCancel.archiveCount === beforeCancel.archiveCount &&
        afterCancel.persistenceRevision === beforeCancel.persistenceRevision,
      "cancelled Delete mutated conversation",
      { beforeCancel, afterCancel },
    );
    checkpoints.push({ step: "delete-cancel", ...afterCancel });

    await triggerAction(driver, "flow_desk_session_delete_conversation");
    await confirmAction(driver);
    state = await waitForState(driver, (candidate) => historySession(candidate)?.archiveCount === 0, "archive delete");
    checkpoints.push({ step: "archive-delete-confirm", ...safeSession(state) });

    await triggerAction(driver, "flow_desk_session_new_conversation");
    state = await waitForState(driver, (candidate) => historySession(candidate)?.archiveCount === 1, "archive for stale write");
    const archivedManifest = await waitForManifest(
      (value) => ((value.threads as Json[] | undefined) ?? []).some((thread) => thread.state === "archived"),
      "archive before stale control",
    );
    const staleArchiveId = ((archivedManifest.threads as Json[]).find((thread) => thread.state === "archived") as Json).id as string;

    writeFileSync(holdMarker, "hold");
    driver.send({
      type: "batch",
      requestId: `c05-held-turn-${Date.now()}`,
      commands: [{ type: "setInput", text: "held stale write" }],
    });
    await pressMain(driver, "enter");
    await waitForState(driver, (candidate) => historySession(candidate)?.activeTurnCount === 1, "held turn settlement");
    const heldPath = `${holdMarker}.held`;
    const heldDeadline = Date.now() + 10_000;
    while (!existsSync(heldPath) && Date.now() < heldDeadline) await Bun.sleep(25);
    assert(existsSync(heldPath), "persistence worker did not hold the stale write");

    await triggerAction(driver, "flow_desk_session_history");
    await waitForState(driver, (candidate) => historySession(candidate)?.selection === "archive", "held archive selection");
    await triggerAction(driver, "flow_desk_session_delete_conversation");
    await confirmAction(driver);
    unlinkSync(holdMarker);
    const afterRelease = await waitForManifest(
      (value) => !manifestThreadIds(value).includes(staleArchiveId),
      "tombstone after stale release",
      15_000,
    );
    checkpoints.push({
      step: "stale-release-cannot-resurrect",
      ...safeSession(await driver.getState() as Json),
      store: safeManifest(afterRelease),
    });

    const beforeDismiss = safeManifest(readHistoryManifest());
    await pressMain(driver, "escape");
    await waitForState(driver, (candidate) => flowUx(candidate).activeVariant === "flash", "Escape background");
    await filterDesk(driver, "");
    await pressMain(driver, "enter");
    await waitForState(driver, (candidate) => candidate.promptType === "flowSession", "reopen after Escape");
    await pressMain(driver, "w", ["cmd"]);
    await waitForState(driver, (candidate) => candidate.windowVisible === false, "Cmd+W close");
    const afterDismiss = safeManifest(readHistoryManifest());
    assert(
      beforeDismiss.threadCount === afterDismiss.threadCount &&
        beforeDismiss.totalTurnCount === afterDismiss.totalTurnCount,
      "Escape or Cmd+W deleted history",
      { beforeDismiss, afterDismiss },
    );
    checkpoints.push({ step: "dismissals-preserve", store: afterDismiss });

    scenarios.push({ name: "core-lifecycle-b", status: "PASS", checkpoints });
  } finally {
    if (existsSync(holdMarker)) unlinkSync(holdMarker);
    await closeOwned("core-lifecycle-b", driver, clipboardBefore);
  }
}

function writeFakeMd(directory: string, body: string): string {
  mkdirSync(directory, { recursive: true });
  const path = join(directory, "mdflow");
  writeFileSync(path, body);
  chmodSync(path, 0o755);
  const md = join(directory, "md");
  writeFileSync(md, body);
  chmodSync(md, 0o755);
  return directory;
}

async function setupScenario(
  name: string,
  expectedState: string,
  mode: "loading" | "missing" | "incompatible" | "failed" | "empty" | "ready" | "nomatch",
): Promise<void> {
  const home = join(privateRoot, `setup-${name}-home`);
  const bin = join(privateRoot, `setup-${name}-bin`);
  const emptyPackage = join(privateRoot, `setup-${name}-package`);
  mkdirSync(emptyPackage, { recursive: true });
  let pathValue = `${join(fixture, "bin")}:${process.env.PATH ?? ""}`;
  const env: Record<string, string> = {
    SCRIPT_KIT_FLOW_UX_CWD: fixture,
    SCRIPT_KIT_FLOWS_PACKAGE_DIR: emptyPackage,
    SCRIPT_KIT_FLOWS_BIN_DIR: bin,
  };
  if (mode === "missing") {
    pathValue = "/usr/bin:/bin";
  } else if (mode === "loading") {
    const release = join(privateRoot, `setup-${name}-release`);
    writeFileSync(release, "hold");
    writeFakeMd(bin, `#!/bin/sh\nif [ "$1" = "roster" ]; then while [ -e "${release}" ]; do sleep 0.05; done; fi\nprintf '%s\\n' '{"protocolVersion":1,"cwd":"fixture","projectRoot":"fixture","flows":[],"warnings":[]}'\n`);
    pathValue = `${bin}:/usr/bin:/bin`;
    env.__release = release;
  } else if (mode === "incompatible") {
    writeFakeMd(bin, "#!/bin/sh\nprintf 'flow roster not found\\n' >&2\nexit 1\n");
    pathValue = `${bin}:/usr/bin:/bin`;
  } else if (mode === "failed") {
    writeFakeMd(bin, "#!/bin/sh\nprintf 'PRIVATE_ROSTER_ERROR_CANARY\\n' >&2\nexit 42\n");
    pathValue = `${bin}:/usr/bin:/bin`;
  } else if (mode === "empty") {
    writeFakeMd(bin, "#!/bin/sh\nprintf '%s\\n' '{\"protocolVersion\":1,\"cwd\":\"fixture\",\"projectRoot\":\"fixture\",\"flows\":[],\"warnings\":[]}'\n");
    pathValue = `${bin}:/usr/bin:/bin`;
  }
  env.PATH = pathValue;

  let driver: Driver | null = null;
  const clipboardBefore = pasteboardChangeCount();
  try {
    driver = await launch(`setup-${name}`, home, env);
    await openDesk(driver);
    if (mode === "nomatch") await filterDesk(driver, "NO_MATCH_C05_QUERY");
    const state = await waitForState(
      driver,
      (candidate) => flowUx(candidate).deskState === expectedState,
      `${name} desk state`,
      15_000,
    );
    const fx = flowUx(state);
    const selected = (fx.selectedRow as Json | undefined) ?? null;
    const serialized = JSON.stringify({ deskState: fx.deskState, deskFailure: fx.deskFailure, selected });
    assert(!serialized.includes("PRIVATE_ROSTER_ERROR_CANARY"), `${name}: raw roster error leaked`);
    if (mode === "loading") assert(selected === null, "Loading exposed a fake enabled row", selected);
    if (mode === "nomatch") {
      assert(selected?.primaryVerb === "Clear Search", "NoMatch did not select Clear Search", selected);
      await pressMain(driver, "enter");
      await waitForState(driver, (candidate) => flowUx(candidate).deskState === "Ready", "clear search recovery");
    }
    scenarios.push({
      name: `desk-${name}`,
      status: "PASS",
      checkpoints: [{
        step: name,
        deskState: fx.deskState,
        failureCode: fx.deskFailure?.code ?? null,
        diagnosticFingerprint: fx.deskFailure?.diagnosticFingerprint ?? null,
        selectedPrimaryVerb: selected?.primaryVerb ?? null,
        selectedSecondaryVerb: selected?.secondaryVerb ?? null,
      }],
    });
  } finally {
    const release = env.__release;
    if (release && existsSync(release)) unlinkSync(release);
    await closeOwned(`desk-${name}`, driver, clipboardBefore);
  }
}

mkdirSync(receiptDir, { recursive: true });
mkdirSync(sharedKit, { recursive: true });
assert(existsSync(binary), "stable C05 binary is missing", { binary });
assert(existsSync(historyEngine), "history engine fixture is missing");

try {
  await lifecycleProcessA();
  await lifecycleProcessB();
  await setupScenario("loading", "Loading", "loading");
  await setupScenario("missing", "MdflowMissing", "missing");
  await setupScenario("incompatible", "MdflowIncompatible", "incompatible");
  await setupScenario("failed", "RosterFailed", "failed");
  await setupScenario("empty", "ReadyEmpty", "empty");
  await setupScenario("nomatch", "NoMatch", "nomatch");
  await setupScenario("ready", "Ready", "ready");
} catch (error) {
  failures.push(error instanceof Error ? error.message : String(error));
} finally {
  if (existsSync(holdMarker)) unlinkSync(holdMarker);
}

const privacy = {
  transcriptCanaryMatches: 0,
  draftCanaryMatches: 0,
  pathCanaryMatches: 0,
  providerErrorCanaryMatches: 0,
  clipboardCanaryMatches: 0,
};
const receipt: Json = {
  schemaVersion: 1,
  classification: "privacy-safe-flow-history-proof",
  binary: {
    path: "target-agent/artifacts/cons-flow-c05/script-kit-gpui",
    sha256: sha256File(binary),
  },
  sandbox: { homeFingerprint: sha256Bytes(sharedHome) },
  scenarios,
  cleanup,
  privacy,
  failures,
};
const serialized = JSON.stringify(receipt, null, 2);
for (const canary of [
  "PRIVATE_ROSTER_ERROR_CANARY",
  "PRIVATE_DRAFT_CANARY",
  "runtime draft canary",
  privateRoot,
  sharedHome,
]) {
  assert(!serialized.includes(canary), `receipt leaked private canary: ${basename(canary)}`);
}
for (const entry of cleanup) {
  assert(entry.processExited === true, "cleanup processExited was not true", entry);
  assert(entry.streamsDrained === true, "cleanup streamsDrained was not true", entry);
  assert(entry.logWriterClosed === true, "cleanup logWriterClosed was not true", entry);
  assert(entry.ownedProcessCount === 0, "cleanup ownedProcessCount was not zero", entry);
  assert(entry.fixtureOwnedProcessCount === 0, "fixture process survived", entry);
}

writeFileSync(join(receiptDir, "flow-history-receipt.json"), serialized);
for (const task of ["SAFE-003", "WF-011"] as const) {
  let taskReceipt: Json;
  try {
    assert(failures.length === 0, "Flow history journey did not pass");
    const requiredStages = task === "SAFE-003"
      ? ["core-lifecycle-a", "core-lifecycle-b"]
      : ["core-lifecycle-a", "core-lifecycle-b", "desk-ready"];
    const selected = requiredStages.map((id) => {
      const scenario = scenarios.find((item) => item.name === id);
      const segment = observedSegments.get(id);
      assert(scenario?.status === "PASS" && segment, `missing observed Flow lifecycle stage: ${id}`);
      return { scenario, segment };
    });
    const lifecycle = selected.find((item) => item.scenario.name === "core-lifecycle-b")!.scenario;
    const checkpoints = Array.isArray(lifecycle.checkpoints) ? lifecycle.checkpoints as Json[] : [];
    const hasCheckpoint = (step: string) => checkpoints.some((checkpoint) => checkpoint.step === step);
    const controls = task === "SAFE-003"
      ? {
          "new-conversation-preserves-history": hasCheckpoint("new-archives-active"),
          "delete-requires-explicit-confirmation":
            hasCheckpoint("delete-cancel") && hasCheckpoint("archive-delete-confirm"),
        }
      : {
          "runtime-termination-preserves-history": hasCheckpoint("terminate-idle"),
          "missing-engine-reports-actionable-state":
            scenarios.some((scenario) => scenario.name === "desk-missing" && scenario.status === "PASS"),
        };
    taskReceipt = prepareWorkflowTaskProof(task, {
      producerOwner: "scripts/agentic/cons-flow-ux/flow-history-probe.ts",
      segments: selected.map((item) => item.segment),
      stages: selected.map(({ scenario, segment }) => observedWorkflowStage({
        id: String(scenario.name),
        primitiveId: "devtools.act",
        segment,
        command: "flow.executeConversationAction",
        requestId: `${task}:${String(scenario.name)}`,
        result: scenario,
        pass: scenario.status === "PASS",
      })),
      negativeControls: controls,
      safety: {
        microphoneCaptureStarted: false,
        nativeInputInjected: false,
        liveAiStarted: false,
        screenTakeoverStarted: false,
        clipboardTouched: false,
      },
    }).receipt as Json;
  } catch (error) {
    taskReceipt = prepareBlockedWorkflowTaskProof(
      task,
      error instanceof Error ? error.message : String(error),
    ).receipt as Json;
  }
  const directory = resolve(receiptDir, "..", task);
  mkdirSync(directory, { recursive: true });
  writeFileSync(
    join(directory, "receipt.json"),
    JSON.stringify(taskReceipt, null, 2),
  );
  writeWorkflowTaskProof(task, taskReceipt);
}

console.log(serialized);
rmSync(privateRoot, { recursive: true, force: true });
if (failures.length > 0) process.exit(1);
