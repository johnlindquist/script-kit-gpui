#!/usr/bin/env bun
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";

assertNoninteractiveVisualProbe("dictation-history.system-clipboard");

const ROOT = resolve(import.meta.dir, "../../..");
const BINARY = resolve(process.env.PROBE_BINARY ?? join(ROOT, "target-agent/artifacts/cons-flow-c14/script-kit-gpui"));
const OUT_DIR = join(ROOT, ".test-output", "cons-flow-c14");
const OUT_PATH = join(OUT_DIR, "dictation-history-receipt.json");
const MAIN_TARGET: Json = { type: "kind", kind: "main", index: 0 };
const ACTIONS_TARGET: Json = { type: "kind", kind: "actionsDialog", index: 0 };

type Obj = Record<string, any>;
type Scenario = { id: string; pass: boolean; failures: string[]; facts: Obj; cleanup: Obj };
const asObj = (value: unknown): Obj => value && typeof value === "object" && !Array.isArray(value) ? value as Obj : {};
const sha256 = (value: string | Uint8Array) => createHash("sha256").update(value).digest("hex");
function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) throw new Error(`${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`);
}
function exactExecutablePids(executable: string): number[] {
  const proc = Bun.spawnSync(["/bin/ps", "-axo", "pid=,command="], { stdout: "pipe", stderr: "pipe" });
  const normalized = resolve(executable);
  return new TextDecoder().decode(proc.stdout).split("\n").map((line) => line.trim()).filter(Boolean).flatMap((line) => {
    const match = line.match(/^(\d+)\s+(.+)$/); if (!match) return [];
    return resolve(match[2].trim().split(/\s+/, 1)[0]) === normalized ? [Number(match[1])] : [];
  });
}
async function poll<T>(label: string, read: () => Promise<T>, accepts: (value: T) => boolean, timeoutMs = 15_000): Promise<T> {
  const deadline = Date.now() + timeoutMs; let last = await read();
  while (Date.now() < deadline) { if (accepts(last)) return last; await Bun.sleep(50); last = await read(); }
  throw new Error(`${label} timed out: ${JSON.stringify(last)}`);
}
async function state(driver: Driver): Promise<Obj> { return asObj(await driver.getState({ timeoutMs: 8000 })); }
async function chatState(driver: Driver): Promise<Obj> { return asObj(await driver.request({ type: "getAgentChatState" }, { timeoutMs: 8000 })); }
async function elements(driver: Driver, target: Json = MAIN_TARGET): Promise<Obj[]> {
  const value = asObj(await driver.getElements({ target, limit: 300 }, { timeoutMs: 8000 }));
  return Array.isArray(value.elements) ? value.elements.map(asObj) : [];
}
function seedHistory(driver: Driver): { path: string; ids: string[]; transcripts: string[] } {
  const kit = join(driver.sessionDir, "home", ".scriptkit");
  mkdirSync(kit, { recursive: true });
  const ids: string[] = []; const transcripts: string[] = []; const lines: string[] = [];
  for (let index = 0; index < 125; index += 1) {
    const id = `c14-${index.toString().padStart(3, "0")}`;
    const transcript = `C14_TRANSCRIPT_CANARY_${index.toString().padStart(3, "0")}`;
    ids.push(id); transcripts.push(transcript);
    lines.push(JSON.stringify({
      version: 2, id, timestamp: new Date(Date.UTC(2026, 6, 1, 0, index, 0)).toISOString(),
      transcript, preview: `History row ${index.toString().padStart(3, "0")}`,
      target_id: index % 2 === 0 ? "notes" : "agentchat",
      target_label_snapshot: index % 2 === 0 ? "Notes" : "Agent Chat",
      audio_duration_ms: 1000 + index,
    }));
  }
  const path = join(kit, "dictation-history.jsonl");
  writeFileSync(path, `${lines.join("\n")}\n`);
  return { path, ids, transcripts };
}
async function openHistory(driver: Driver): Promise<Obj> {
  await driver.setFilterAndWait("Dictation History");
  await driver.simulateKey("enter", []);
  return poll("Dictation History", () => state(driver), (value) => value.promptType === "dictationHistory");
}
async function openActions(driver: Driver): Promise<Obj[]> {
  await driver.simulateGpuiKeyDown("k", { target: MAIN_TARGET, modifiers: ["cmd"] });
  await poll("Dictation History Actions", async () => {
    const windows = asObj(await driver.listAutomationWindows({ timeoutMs: 8000 }));
    return Array.isArray(windows.windows) ? windows.windows.map(asObj) : [];
  }, (windows) => windows.some((item) => item.kind === "actionsDialog"));
  return elements(driver, ACTIONS_TARGET);
}
async function chooseAction(driver: Driver, actionId: string): Promise<void> {
  const result = asObj(await driver.triggerAction(actionId, { host: "main", timeoutMs: 8000 }));
  assert(result.ok === true || result.success === true, `triggerAction failed for ${actionId}`, result);
}
async function installCompletedTurnFixture(driver: Driver): Promise<Obj> {
  const result = asObj(await driver.request({
    type: "setAgentChatTestFixture",
    phase: "idle",
    userText: "C14 accepted request",
    assistantText: "C14 immutable sent-turn receipt",
  }, { expect: "externalCommandResult", timeoutMs: 8000 }));
  assert(result.ok !== false && result.success !== false, "completed-turn fixture failed", result);
  return poll("completed-turn fixture", () => chatState(driver), (value) => Number(value.messageCount ?? 0) === 2);
}
async function openDictationPortal(driver: Driver, token: string): Promise<Obj> {
  await driver.request({ type: "setAgentChatInput", text: token }, { timeoutMs: 8000 });
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: ".", modifiers: ["cmd"] },
    { target: MAIN_TARGET, timeoutMs: 8000 },
  );
  return poll("Dictation History portal", () => state(driver), (value) => value.promptType === "dictationHistory");
}
async function waitForConfirm(driver: Driver): Promise<Obj> {
  return poll("Dictation delete confirmation", async () => {
    const value = asObj(await driver.listAutomationWindows({ timeoutMs: 8000 }));
    const windows = Array.isArray(value.windows) ? value.windows.map(asObj) : [];
    return windows.find((item) => item.id === "confirm-popup") ?? {};
  }, (popup) => popup.id === "confirm-popup");
}
async function selectConfirm(driver: Driver, semanticId: string): Promise<void> {
  const result = asObj(await driver.request({
    type: "batch", target: { type: "id", id: "confirm-popup" },
    commands: [{ type: "selectBySemanticId", semanticId, submit: true }],
    options: { stopOnError: true, timeout: 5000 },
  }, { expect: "batchResult", timeoutMs: 8000 }));
  assert(result.success === true, "confirmation action failed", result);
}
function clipboardRead(): Uint8Array {
  const result = Bun.spawnSync(["/usr/bin/pbpaste"], { stdout: "pipe", stderr: "pipe" });
  return result.stdout;
}
function clipboardWrite(bytes: Uint8Array): void {
  const result = Bun.spawnSync(["/usr/bin/pbcopy"], { stdin: bytes, stdout: "ignore", stderr: "pipe" });
  assert(result.exitCode === 0, "clipboard restore failed", { exitCode: result.exitCode });
}
async function runScenario(id: string, body: (driver: Driver, facts: Obj) => Promise<void>, extraEnv: Record<string, string> = {}): Promise<Scenario> {
  const failures: string[] = []; const facts: Obj = {}; let cleanup: Obj = {}; let driver: Driver | null = null;
  try {
    driver = await Driver.launch({
      binary: BINARY, sessionName: `cons-flow-c14-${id}`, sandboxHome: true, sharedModels: false,
      seedAgentAuth: true, readyTimeoutMs: 30_000, defaultTimeoutMs: 15_000,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1", SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
        SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1", SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1", ...extraEnv,
      },
    });
    await driver.waitForSettle(); await body(driver, facts);
  } catch (error) { failures.push(error instanceof Error ? error.message : String(error)); }
  finally {
    if (driver) {
      await driver.close().catch((error) => failures.push(`driver.close: ${String(error)}`));
      cleanup = asObj(driver.finalization);
      if (cleanup.processExited !== true || cleanup.streamsDrained !== true || cleanup.logWriterClosed !== true) failures.push("incomplete Driver finalization");
      if (existsSync(driver.sessionDir)) rmSync(driver.sessionDir, { recursive: true, force: true });
    }
  }
  return { id, pass: failures.length === 0, failures, facts, cleanup };
}

mkdirSync(OUT_DIR, { recursive: true });
const clipboardBefore = clipboardRead();
const scenarios: Scenario[] = [];
try {
  scenarios.push(await runScenario("paging-actions-portals-delete", async (driver, facts) => {
    const seeded = seedHistory(driver);
    let current = await openHistory(driver);
    assert(current.choiceCount === 125 && current.visibleChoiceCount === 100, "initial count is not Showing 100 of 125", current);
    let semantic = await elements(driver);
    assert(semantic.some((item) => item.semanticId === "button:dictation-history-load-more"), "Load More semantic action missing");

    for (let index = 0; index < 5; index += 1) await driver.simulateGpuiKeyDown("down", { target: MAIN_TARGET });
    current = await state(driver);
    const selectedValue = String(current.selectedValue ?? "");
    const selectedFingerprintBefore = sha256(selectedValue);
    const selectedIndexBefore = current.selectedIndex;
    const scrollAnchorBefore = {
      firstVisibleSemanticId: current.activeListScroll?.firstVisibleSemanticId ?? null,
      logicalScrollTop: current.activeListScroll?.logicalScrollTop ?? null,
    };
    const selectedNumber = Number(selectedValue.match(/(\d{3})$/)?.[1] ?? -1);
    assert(selectedNumber >= 0 && selectedNumber < seeded.transcripts.length, "selected History identity is not recognizable");
    const selectedTranscript = seeded.transcripts[selectedNumber];
    await driver.simulateGpuiKeyDown("enter", { target: MAIN_TARGET, modifiers: ["cmd"] });
    const copied = await poll("History clipboard copy", async () => clipboardRead(), (bytes) => sha256(bytes) === sha256(selectedTranscript));
    assert(sha256(copied) === sha256(selectedTranscript), "Command+Enter did not copy the selected transcript");

    const load = asObj(await driver.request({
      type: "batch", target: MAIN_TARGET,
      commands: [{ type: "selectBySemanticId", semanticId: "button:dictation-history-load-more", submit: true }],
      options: { stopOnError: true, timeout: 5000 },
    }, { expect: "batchResult", timeoutMs: 8000 }));
    assert(load.success === true, "Load More action failed", load);
    current = await poll("expanded History", () => state(driver), (value) => value.visibleChoiceCount === 125);
    assert(current.choiceCount === 125, "expanded total changed", current);
    assert(current.selectedIndex === selectedIndexBefore, "Load More changed selected index", current);
    assert(sha256(String(current.selectedValue ?? "")) === selectedFingerprintBefore, "Load More changed selected row identity", current);
    assert(current.activeListScroll?.firstVisibleSemanticId === scrollAnchorBefore.firstVisibleSemanticId, "Load More changed the first visible row anchor", current.activeListScroll);
    assert(current.activeListScroll?.logicalScrollTop === scrollAnchorBefore.logicalScrollTop, "Load More changed the logical scroll anchor", current.activeListScroll);

    let actionRows = await openActions(driver);
    const standaloneLabels = actionRows.map((item) => String(item.text ?? ""));
    for (const label of ["Paste to Frontmost App", "Add to Agent Chat", "Copy Transcript", "Delete from History"]) {
      assert(standaloneLabels.some((value) => value.includes(label)), `standalone Actions missing ${label}`);
    }
    assert(!standaloneLabels.some((value) => /\b(Ask|Send)\b/.test(value)), "History Actions advertise a submitting verb", standaloneLabels);
    await chooseAction(driver, "dictation_history_add_to_agent_chat");
    const selectedHistoryLabel = `Dictation: History row ${selectedNumber.toString().padStart(3, "0")}`;
    const chat = await poll("composer-only History add", () => chatState(driver), (value) => {
      const parts = Array.isArray(value.contextParts) ? value.contextParts.map(asObj) : [];
      return Number(value.messageCount ?? -1) === 0
        && parts.some((part) => String(part.label ?? "").includes(selectedHistoryLabel));
    });
    const addContextCount = Number(chat.contextChipCount ?? 0);
    assert(addContextCount >= 1, "Add to Agent Chat did not stage History context", chat);
    assert(Number(chat.messageCount ?? 0) === 0, "Add to Agent Chat submitted a turn", chat);

    const selectedHistoryId = seeded.ids[selectedNumber];
    const selectedHistoryToken = `@dictation:${selectedHistoryId}`;
    await openDictationPortal(driver, selectedHistoryToken);
    actionRows = await openActions(driver);
    const portalLabels = actionRows.map((item) => String(item.text ?? ""));
    assert(!portalLabels.some((value) => value.includes("Paste to Frontmost App")), "portal exposed Paste", portalLabels);
    assert(!portalLabels.some((value) => value.includes("Add to Agent Chat")), "portal exposed redundant Add", portalLabels);
    assert(portalLabels.some((value) => value.includes("Copy Transcript")), "portal omitted Copy", portalLabels);
    await chooseAction(driver, "dictation_history_delete");
    await waitForConfirm(driver);
    const confirmRows = await elements(driver, { type: "id", id: "confirm-popup" });
    const pendingWarning = confirmRows.some((item) =>
      `${String(item.text ?? "")} ${String(item.value ?? "")}`.includes("staged in Agent Chat")
    );
    assert(pendingWarning, "pending staged attachment did not produce the stronger delete warning", confirmRows);
    await selectConfirm(driver, "button:1:cancel");
    const afterCancelledDelete = readFileSync(seeded.path, "utf8");
    assert(afterCancelledDelete.includes(`"id":"${seeded.ids[selectedNumber]}"`), "cancelled deletion removed the staged History row");

    await driver.simulateGpuiKeyDown("escape", { target: MAIN_TARGET });
    const restored = await poll("portal cancel restore", () => chatState(driver), (value) => String(value.inputText ?? "") === selectedHistoryToken);
    assert(Number(restored.contextChipCount ?? 0) === addContextCount && Number(restored.messageCount ?? 0) === 0, "portal cancel changed composer context or messages", restored);
    assert(restored.hasSelection === chat.hasSelection, "portal cancel changed composer selection state", restored);
    const restoredElements = await elements(driver);
    assert(restoredElements.some((item) => item.semanticId === "input:agent-chat-composer" && item.focused === true), "portal cancel did not restore composer focus", restoredElements);

    const completedTurn = await installCompletedTurnFixture(driver);
    assert(Number(completedTurn.messageCount ?? 0) === 2, "sent-turn fixture did not install immutable receipts", completedTurn);

    await openDictationPortal(driver, "@dictation");
    for (let index = 0; index < 6; index += 1) await driver.simulateGpuiKeyDown("down", { target: MAIN_TARGET });
    await poll("portal row 118 selection", () => state(driver), (value) => String(value.selectedValue ?? "").includes("History row 118"));
    await driver.simulateGpuiKeyDown("enter", { target: MAIN_TARGET });
    const attached = await poll("portal transcript attach", () => chatState(driver), (value) => {
      const parts = Array.isArray(value.contextParts) ? value.contextParts.map(asObj) : [];
      return Number(value.messageCount ?? -1) === 2
        && parts.some((part) =>
          String(part.label ?? "").includes("History row 118")
            && part.provenance === "attachmentPortal"
        );
    });
    assert(Number(attached.messageCount ?? 0) === 2, "Attach Transcript mutated sent turns", attached);

    await openDictationPortal(driver, "@dictation");
    for (let index = 0; index < 6; index += 1) await driver.simulateGpuiKeyDown("down", { target: MAIN_TARGET });
    await poll("pending portal row 118 selection", () => state(driver), (value) => String(value.selectedValue ?? "").includes("History row 118"));
    await openActions(driver); await chooseAction(driver, "dictation_history_delete");
    await waitForConfirm(driver); await selectConfirm(driver, "button:0:confirm");
    await Bun.sleep(100);
    const remaining = readFileSync(seeded.path, "utf8");
    assert(!remaining.includes('"id":"c14-118"'), "confirmed deletion retained selected History row");
    await driver.simulateGpuiKeyDown("escape", { target: MAIN_TARGET });
    const afterDeleteChat = await poll("sent-turn restore after deletion", () => chatState(driver), (value) => Number(value.messageCount ?? 0) === 2);
    assert(Number(afterDeleteChat.messageCount ?? 0) === 2, "History deletion erased immutable sent-turn receipts", afterDeleteChat);

    facts.initial = { total: 125, visible: 100, countLabel: "Showing 100 of 125", hasLoadMore: true };
    facts.expanded = { total: 125, visible: 125, selectedIndexPreserved: true, selectedFingerprintPreserved: true, scrollAnchorPreserved: true };
    facts.copy = { length: copied.length, fingerprint: sha256(copied), clipboardRestored: true };
    facts.actions = { standalone: ["Paste", "AddToAgentChat", "Copy", "Delete"], portal: ["Copy", "Delete"], submittingVerbCount: 0 };
    facts.agentChat = { addContextCount, attachContextCount: attached.contextChipCount, intendedResourceStaged: true, messageCount: 0 };
    facts.deletion = { confirmationRequired: true, pendingWarning: true, cancelledRowPreserved: true, confirmedRowRemoved: true, sentMessageCountBefore: 2, sentMessageCountAfter: afterDeleteChat.messageCount, sentReceiptBoundaryUnaffected: Number(afterDeleteChat.messageCount ?? 0) === 2 };
    facts.portalCancel = { draftRestored: true, selectionStateRestored: true, focusRestored: true, contextCount: restored.contextChipCount, messageCount: restored.messageCount };
  }));

  scenarios.push(await runScenario("typed-load-failure-with-prior", async (driver, facts) => {
    const seeded = seedHistory(driver);
    const ready = await openHistory(driver);
    assert(ready.choiceCount === 125 && ready.visibleChoiceCount === 100, "prior successful page was not established", ready);
    const selectedBefore = String(ready.selectedValue ?? "");
    rmSync(seeded.path, { force: true });
    mkdirSync(seeded.path);

    const failed = await poll("retained page after load failure", () => state(driver), (value) =>
      value.choiceCount === 125
        && value.visibleChoiceCount === 100
        && String(value.selectedValue ?? "") === selectedBefore
    );
    const visible = await elements(driver);
    assert(visible.some((item) => item.semanticId === "status:dictation-history-load-failed" && String(item.text ?? "").includes("could not be loaded")), "load failure status was not projected", visible);
    assert(visible.some((item) => item.semanticId === "list:dictation-history" && String(item.text ?? "").includes("100 items")), "prior History row count disappeared during load failure", visible);
    assert(visible.some((item) => item.type === "choice" && String(item.text ?? "").startsWith("History row ")), "prior History rows disappeared during load failure", visible);
    facts.failureState = { typed: "Failed", renderedAsEmpty: false, priorTotal: failed.choiceCount, priorVisible: failed.visibleChoiceCount, selectedIdentityPreserved: true };
  }));
} finally {
  clipboardWrite(clipboardBefore);
}

const clipboardRestored = sha256(clipboardRead()) === sha256(clipboardBefore);
const primaryCopyFacts = asObj(asObj(scenarios.find((scenario) => scenario.id === "paging-actions-portals-delete")?.facts).copy);
if (Object.keys(primaryCopyFacts).length > 0) primaryCopyFacts.clipboardRestored = clipboardRestored;
if (!clipboardRestored) scenarios.push({ id: "clipboard-cleanup", pass: false, failures: ["clipboard text fingerprint was not restored"], facts: {}, cleanup: {} });
const exactArtifactOwnedProcessCount = exactExecutablePids(BINARY).length;
if (exactArtifactOwnedProcessCount !== 0) scenarios.push({ id: "cleanup", pass: false, failures: [`${exactArtifactOwnedProcessCount} exact artifact processes remain`], facts: {}, cleanup: {} });
const failures = scenarios.flatMap((scenario) => scenario.failures.map((failure) => `${scenario.id}: ${failure}`));
const receipt = {
  schemaVersion: 1, probe: "cons-flow-c14-dictation-history", binarySha256: sha256(readFileSync(BINARY)),
  pass: failures.length === 0, failures, scenarios, exactArtifactOwnedProcessCount,
  privacy: { rawTranscriptPresent: false, clipboardContentPresent: false, externalAppLabelPresent: false },
};
writeFileSync(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exit(1);
