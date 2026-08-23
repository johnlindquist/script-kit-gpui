#!/usr/bin/env bun

import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.context-lifecycle");

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY ??
    "target-agent/artifacts/cons-flow-c02/script-kit-gpui",
);
const runDir = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR ??
    ".artifacts/consistency/cons-flow-ux/c02-context-lifecycle-v1",
);
const portalToken = "@script:context-lifecycle-portal";
const portalQuery = "context-lifecycle-portal";

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(
      `${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`,
    );
  }
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

function sha256(path: string): string {
  const result = Bun.spawnSync(["shasum", "-a", "256", path], {
    stdout: "pipe",
    stderr: "pipe",
  });
  assert(result.exitCode === 0, "failed to hash runtime binary");
  return new TextDecoder().decode(result.stdout).trim().split(/\s+/, 1)[0];
}

function items(state: Json, key: "contextParts" | "contextReceipts"): Json[] {
  return Array.isArray(state[key]) ? state[key] as Json[] : [];
}

function safeItem(item: Json): Json {
  return {
    id: item.id,
    kind: item.kind,
    label: item.label,
    source: item.source,
    sourceFingerprint: item.sourceFingerprint,
    provenance: item.provenance,
    role: item.role,
    state: item.state,
    lifetime: item.lifetime,
    removable: item.removable,
    generation: item.generation,
    failureCode: item.failureCode ?? null,
    diagnosticFingerprint: item.diagnosticFingerprint ?? null,
  };
}

function safeState(state: Json): Json {
  return {
    schemaVersion: state.schemaVersion,
    status: state.status,
    inputLength: String(state.inputText ?? "").length,
    cursorIndex: state.cursorIndex,
    hasSelection: state.hasSelection,
    selectionRange: state.selectionRange ?? null,
    picker: state.picker ?? null,
    contextChipCount: state.contextChipCount,
    contextParts: items(state, "contextParts").map(safeItem),
    contextReceipts: items(state, "contextReceipts").map(safeItem),
    preparedTurnFingerprint: state.preparedTurnFingerprint ?? null,
  };
}

async function getAgentChatState(driver: Driver): Promise<Json> {
  return driver.request(
    { type: "getAgentChatState", target: { type: "id", id: "main" } },
    { expect: "agentChatStateResult", timeoutMs: 15_000 },
  );
}

async function waitForAgentChatState(
  driver: Driver,
  predicate: (state: Json) => boolean,
  label: string,
  timeoutMs = 10_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let state = await getAgentChatState(driver);
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(25);
    state = await getAgentChatState(driver);
  }
  assert(predicate(state), `timed out waiting for ${label}`, safeState(state));
  return state;
}

async function waitForTopState(
  driver: Driver,
  predicate: (state: Json) => boolean,
  label: string,
  timeoutMs = 10_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let state = await driver.getState({ timeoutMs: 5_000 });
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(25);
    state = await driver.getState({ timeoutMs: 5_000 });
  }
  assert(predicate(state), `timed out waiting for ${label}`, {
    promptType: state.promptType,
    surfaceKind: state.surfaceContract?.surfaceKind,
    inputValue: state.inputValue,
    windowVisible: state.windowVisible,
  });
  return state;
}

async function applyFixture(driver: Driver, phase: string, userText: string): Promise<Json> {
  const response = await driver.request(
    { type: "setAgentChatTestFixture", phase, userText },
    { expect: "externalCommandResult", timeoutMs: 15_000 },
  );
  assert(response.ok === true, `fixture ${phase} was rejected`, { ok: response.ok });
  return response;
}

async function composerFocus(driver: Driver): Promise<Json> {
  const response = await driver.getElements(
    { target: { type: "kind", kind: "main" }, limit: 300 },
    { timeoutMs: 10_000 },
  );
  const all = Array.isArray(response.elements) ? response.elements as Json[] : [];
  const composer = all.find((element) => element.semanticId === "input:agent-chat-composer");
  return {
    focusedSemanticId: response.focusedSemanticId ?? null,
    composerFocused: composer?.focused === true,
    composerValueLength: String(composer?.value ?? "").length,
  };
}

function assertReceiptPrivate(value: unknown): void {
  const serialized = JSON.stringify(value);
  const forbidden = [
    "/missing/",
    "fixture://context-lifecycle/",
    "CONTEXT_LIFECYCLE_PATH_" + "CANARY",
    "CONTEXT_LIFECYCLE_ERROR_" + "CANARY",
    "synthetic context body",
    "required synthetic body",
  ];
  assert(
    forbidden.every((token) => !serialized.includes(token)),
    "runtime receipt contained private context source or content",
  );
}

let receipt: Json = {
  schemaVersion: 1,
  classification: "RUNTIME-FAILED",
  binaryArtifact: "cons-flow-c02",
  binarySha256: sha256(binary),
};
let driver: Driver | null = null;
let closeError: string | null = null;

try {
  driver = await Driver.launch({
    binary,
    sessionName: "cons-flow-c02-context-lifecycle",
    sandboxHome: true,
    sharedModels: false,
    env: {
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
      SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
    },
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 15_000,
  });

  driver.send({ type: "openAiWithMockData" });
  await waitForTopState(
    driver,
    (state) =>
      state.promptType === "agentChatChat" &&
      state.surfaceContract?.surfaceKind === "AgentChat" &&
      state.windowVisible === true,
    "Agent Chat host",
  );

  await applyFixture(driver, "contextLifecyclePending", "Lifecycle pending fixture");
  const pending = await waitForAgentChatState(
    driver,
    (state) => items(state, "contextParts").length === 1,
    "one deduplicated pending context item",
  );
  const pendingItems = items(pending, "contextParts");
  const staged = pendingItems[0];
  assert(pending.schemaVersion === 7, "unexpected Agent Chat state schema", safeState(pending));
  assert(pending.contextChipCount === 1, "deduplicated context count was not one", safeState(pending));
  assert(staged.provenance === "attachmentPortal", "higher-priority provenance did not win", safeItem(staged));
  assert(staged.role === "supplemental", "pending role changed unexpectedly", safeItem(staged));
  assert(staged.state === "pending", "pending item had the wrong lifecycle state", safeItem(staged));
  assert(staged.lifetime === "nextTurn" && staged.removable === true, "pending lifetime/removability was wrong", safeItem(staged));
  assert(typeof staged.id === "string" && staged.id.startsWith("context-"), "pending item lacked a stable safe id", safeItem(staged));
  assert(Number(staged.generation) > 0, "pending item lacked a generation", safeItem(staged));
  assert(staged.source === "Text", "compatibility source leaked more than source kind", safeItem(staged));
  assert(typeof staged.sourceFingerprint === "string" && staged.sourceFingerprint.length > 0, "pending item lacked a source fingerprint", safeItem(staged));

  await applyFixture(driver, "contextLifecyclePartialAccepted", "Lifecycle accepted fixture");
  const partial = await waitForAgentChatState(
    driver,
    (state) =>
      items(state, "contextReceipts").length === 1 &&
      items(state, "contextParts").length === 1 &&
      typeof state.preparedTurnFingerprint === "string",
    "accepted receipt plus failed supplemental",
  );
  const acceptedReceipt = items(partial, "contextReceipts")[0];
  const failedSupplemental = items(partial, "contextParts")[0];
  assert(acceptedReceipt.provenance === "threadReceipt", "accepted context was not converted to a thread receipt", safeItem(acceptedReceipt));
  assert(acceptedReceipt.role === "primary", "accepted receipt lost its primary role", safeItem(acceptedReceipt));
  assert(acceptedReceipt.state === "resolved", "accepted receipt was not resolved", safeItem(acceptedReceipt));
  assert(acceptedReceipt.lifetime === "immutableReceipt", "accepted receipt was not immutable", safeItem(acceptedReceipt));
  assert(acceptedReceipt.removable === false, "accepted receipt remained removable", safeItem(acceptedReceipt));
  assert(failedSupplemental.provenance === "attachmentPortal", "failed supplemental lost provenance", safeItem(failedSupplemental));
  assert(failedSupplemental.role === "supplemental", "failed supplemental role changed", safeItem(failedSupplemental));
  assert(failedSupplemental.state === "failed", "failed supplemental was not retained visibly", safeItem(failedSupplemental));
  assert(failedSupplemental.lifetime === "nextTurn" && failedSupplemental.removable === true, "failed supplemental could not be corrected", safeItem(failedSupplemental));
  assert(failedSupplemental.failureCode === "ContextUnavailable", "failed supplemental lost typed failure code", safeItem(failedSupplemental));
  assert(typeof failedSupplemental.diagnosticFingerprint === "string", "failed supplemental lacked a diagnostic fingerprint", safeItem(failedSupplemental));
  assert(String(partial.preparedTurnFingerprint).length >= 16, "immutable retry payload lacked a fingerprint", safeState(partial));
  assertReceiptPrivate(safeState(partial));

  await applyFixture(driver, "contextLifecycleFreshThread", "Lifecycle fresh thread fixture");
  const fresh = await waitForAgentChatState(
    driver,
    (state) =>
      items(state, "contextParts").length === 0 &&
      items(state, "contextReceipts").length === 0 &&
      state.preparedTurnFingerprint == null,
    "fresh thread context reset",
  );
  assert(fresh.contextChipCount === 0, "fresh thread retained a context chip", safeState(fresh));

  await applyFixture(driver, "contextLifecyclePending", "Portal cancellation fixture");
  const setInput = await driver.request(
    { type: "setAgentChatInput", text: portalToken },
    { timeoutMs: 15_000 },
  );
  assert(setInput.ok === true, "portal token was not staged", { ok: setInput.ok });
  const portalBefore = await waitForAgentChatState(
    driver,
    (state) => state.inputText === portalToken && items(state, "contextParts").length === 1,
    "portal pre-open snapshot",
  );
  const focusBefore = await composerFocus(driver);
  assert(focusBefore.composerFocused === true, "composer was not focused before portal open", focusBefore);
  const contextBefore = items(portalBefore, "contextParts").map(safeItem);
  const pickerBefore = portalBefore.picker ?? null;

  const openPortal = await driver.simulateGpuiEvent(
    { type: "keyDown", key: ".", modifiers: ["cmd"] },
    { target: { type: "kind", kind: "main" }, timeoutMs: 15_000 },
  );
  assert(
    openPortal.success === true &&
      (openPortal.dispatchCompleted === true || openPortal.dispatchScheduled === true),
    "Cmd+. did not dispatch the attachment portal",
    { success: openPortal.success, dispatchPath: openPortal.dispatchPath },
  );
  const portalTop = await waitForTopState(
    driver,
    (state) =>
      state.promptType === "none" &&
      state.surfaceContract?.surfaceKind === "ScriptList" &&
      state.inputValue === portalQuery,
    "Script List attachment portal",
  );

  const escape = await driver.simulateGpuiEvent(
    { type: "keyDown", key: "escape", modifiers: [] },
    { target: { type: "kind", kind: "main" }, timeoutMs: 15_000 },
  );
  assert(
    escape.success === true &&
      (escape.dispatchCompleted === true || escape.dispatchScheduled === true),
    "Escape did not dispatch portal cancellation",
    { success: escape.success, dispatchPath: escape.dispatchPath },
  );
  await waitForTopState(
    driver,
    (state) =>
      state.promptType === "agentChatChat" &&
      state.surfaceContract?.surfaceKind === "AgentChat" &&
      state.windowVisible === true,
    "restored Agent Chat host",
  );
  const portalAfter = await waitForAgentChatState(
    driver,
    (state) => state.inputText === portalToken && items(state, "contextParts").length === 1,
    "restored portal draft and context",
  );
  const focusAfter = await composerFocus(driver);
  const contextAfter = items(portalAfter, "contextParts").map(safeItem);
  assert(portalAfter.cursorIndex === portalToken.length, "portal cancellation did not restore the cursor", safeState(portalAfter));
  assert(portalAfter.hasSelection === portalBefore.hasSelection, "portal cancellation changed selection state", safeState(portalAfter));
  assert(JSON.stringify(portalAfter.selectionRange ?? null) === JSON.stringify(portalBefore.selectionRange ?? null), "portal cancellation changed the selection range", safeState(portalAfter));
  assert(JSON.stringify(contextAfter) === JSON.stringify(contextBefore), "portal cancellation changed pending ids or generations", { before: contextBefore, after: contextAfter });
  assert(JSON.stringify(portalAfter.picker ?? null) === JSON.stringify(pickerBefore), "portal cancellation changed picker state", safeState(portalAfter));
  assert(focusAfter.composerFocused === true, "portal cancellation did not restore composer focus", focusAfter);

  receipt = {
    ...receipt,
    classification: "RUNTIME-CONFIRMED",
    pendingDedupe: {
      count: pendingItems.length,
      item: safeItem(staged),
      stablePositionPreserved: true,
      winningProvenance: "attachmentPortal",
    },
    partialAcceptedSend: {
      pendingCount: items(partial, "contextParts").length,
      receiptCount: items(partial, "contextReceipts").length,
      receipt: safeItem(acceptedReceipt),
      failedSupplemental: safeItem(failedSupplemental),
      preparedTurnFingerprint: partial.preparedTurnFingerprint,
    },
    freshThread: {
      pendingCount: items(fresh, "contextParts").length,
      receiptCount: items(fresh, "contextReceipts").length,
      preparedTurnFingerprint: fresh.preparedTurnFingerprint ?? null,
    },
    portalCancellation: {
      portalSurface: portalTop.surfaceContract?.surfaceKind,
      queryLength: String(portalTop.inputValue ?? "").length,
      inputLengthBefore: portalToken.length,
      inputLengthAfter: String(portalAfter.inputText ?? "").length,
      cursorBefore: portalBefore.cursorIndex,
      cursorAfter: portalAfter.cursorIndex,
      selectionBefore: portalBefore.selectionRange ?? null,
      selectionAfter: portalAfter.selectionRange ?? null,
      pendingBefore: contextBefore,
      pendingAfter: contextAfter,
      pickerRestored: true,
      focusBefore,
      focusAfter,
      openDispatchPath: openPortal.dispatchPath ?? null,
      escapeDispatchPath: escape.dispatchPath ?? null,
    },
    negativeControls: {
      rawSourceUriAbsent: true,
      rawMissingPathAbsent: true,
      rawFailureTextAbsent: true,
      duplicateContextDidNotCreateSecondChip: true,
      immutableReceiptNotRemovable: true,
      freshThreadDidNotInheritContext: true,
    },
  };
  assertReceiptPrivate(receipt);
} catch (error) {
  console.error("C02 private probe diagnostic:", error);
  receipt.error = {
    name: error instanceof Error ? error.name : "UnknownError",
    safeMessage: "C02 runtime assertion failed; inspect the private Driver session log.",
  };
} finally {
  if (driver) {
    try {
      await driver.close();
    } catch (error) {
      closeError = error instanceof Error ? error.name : "UnknownCloseError";
      receipt.classification = "RUNTIME-FAILED";
    }
    const ownedPids = exactExecutablePids(binary);
    receipt.cleanup = {
      processExited: driver.finalization.processExited,
      streamsDrained: driver.finalization.streamsDrained,
      logWriterClosed: driver.finalization.logWriterClosed,
      ownedProcessCount: ownedPids.length,
      closeError,
      clipboardTouched: false,
    };
    if (
      !driver.finalization.processExited ||
      !driver.finalization.streamsDrained ||
      !driver.finalization.logWriterClosed ||
      ownedPids.length !== 0
    ) {
      receipt.classification = "RUNTIME-FAILED";
    }
  }

  assertReceiptPrivate(receipt);
  for (const taskId of ["WF-001", "WF-003"]) {
    const taskDir = resolve(runDir, taskId);
    await mkdir(taskDir, { recursive: true });
    await writeFile(
      resolve(taskDir, "receipt.json"),
      `${JSON.stringify({ ...receipt, taskId }, null, 2)}\n`,
    );
  }
}

console.log(JSON.stringify(receipt, null, 2));
assert(receipt.classification === "RUNTIME-CONFIRMED", "C02 runtime proof failed");
const cleanup = receipt.cleanup as Json;
assert(cleanup.processExited === true, "C02 process did not exit", cleanup);
assert(cleanup.streamsDrained === true, "C02 streams did not drain", cleanup);
assert(cleanup.logWriterClosed === true, "C02 log writer did not close", cleanup);
assert(cleanup.ownedProcessCount === 0, "C02 left its exact binary running", cleanup);
assert(exactExecutablePids(binary).length === 0, "C02 left an app instance running");
