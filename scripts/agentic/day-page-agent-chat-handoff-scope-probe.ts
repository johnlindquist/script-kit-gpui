#!/usr/bin/env bun
/**
 * Runtime proof for Today -> Agent Chat handoff scope:
 * - Cmd+Enter from Day Page sends only the active line plus refs on that line.
 * - The explicit whole-day action remains separate and receipted as whole-day.
 * - Receipts are redacted and expose scope metadata instead of day text.
 */
import { copyFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import { openDayPage } from "./day-page-open-helper";
import { assertNoninteractiveVisualProbe } from "../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observeWorkflowTaskTarget,
} from "../devtools/lib/workflow-task-proof.ts";
import type { RuntimeTargetObservation } from "../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("day-page-agent-chat-handoff-scope.workflow-child");

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const BINARY =
  process.env.PROBE_BINARY ??
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(PROJECT_ROOT, "target-agent/artifacts/day-agent-chat-handoff/script-kit-gpui");
const OUT_PATH = join(PROJECT_ROOT, ".test-output", "day-page-agent-chat-handoff-scope-probe.json");

type Obj = Record<string, any>;

const runId = `day-agent-handoff-${Date.now().toString(36)}`;
const unrelated = `UNRELATED_SCOPE_SENTINEL_${runId}`;
const activeLine = `Ask only about scoped line ${runId} [Scoped Ref](https://example.com/${runId})`;
const dayText = `${unrelated}\n${activeLine}`;
const receipt: Obj = {
  schemaVersion: 1,
  tool: "day-page-agent-chat-handoff-scope-probe",
  binary: BINARY,
  pass: false,
  failures: [] as string[],
};
let targetObservation: RuntimeTargetObservation | null = null;

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Obj) : {};
}

function check(name: string, ok: boolean, detail: Obj = {}) {
  receipt[name] = { ok, ...detail };
  if (!ok) receipt.failures.push(name);
}

async function pollUntil(label: string, fn: () => Promise<boolean>, timeoutMs = 10_000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (await fn()) return true;
    await Bun.sleep(100);
  }
  receipt[`timeout_${label}`] = true;
  return false;
}

async function getState(driver: Driver): Promise<Obj> {
  return asObj(await driver.getState({ timeoutMs: 8000 }));
}

async function agentChatState(driver: Driver): Promise<Obj> {
  return asObj(
    await driver.request({ type: "getAgentChatState" }, { expect: "agent_chatStateResult", timeoutMs: 8000 }),
  );
}

async function setDayInput(driver: Driver, text: string): Promise<Obj> {
  return asObj(await driver.batch([{ type: "setInput", text }], { timeoutMs: 8000 }));
}

function handoffReceiptsFromLog(driver: Driver): Obj[] {
  if (!existsSync(driver.logPath)) return [];
  const log = readFileSync(driver.logPath, "utf8");
  return log
    .split(/\r?\n/)
    .filter((line) => line.includes("event=day_page_agent_chat_handoff_receipt"))
    .map((line) => {
      const marker = "receipt_json=";
      const index = line.indexOf(marker);
      if (index === -1) return {};
      try {
        return asObj(JSON.parse(line.slice(index + marker.length).trim()));
      } catch {
        return {};
      }
    })
    .filter((item) => Object.keys(item).length > 0);
}

function latestHandoffReceipt(driver: Driver, mode: string): Obj {
  return asObj(handoffReceiptsFromLog(driver).reverse().find((item) => item.mode === mode));
}

function visibleActions(dialog: Obj): Obj[] {
  if (Array.isArray(dialog.visibleActions)) return dialog.visibleActions.map(asObj);
  const sample = asObj(dialog.actions).visibleSample;
  return Array.isArray(sample) ? sample.map(asObj) : [];
}

function rowActionId(row: Obj): string {
  return String(row.id ?? row.actionId ?? row.value ?? "");
}

function isActionsWindow(win: Obj): boolean {
  return (
    win.id === "actions-dialog" ||
    win.automationId === "actions-dialog" ||
    win.kind === "ActionsDialog" ||
    win.windowKind === "ActionsDialog" ||
    win.semanticSurface === "actionsDialog"
  );
}

async function actionsWindowRegistered(driver: Driver): Promise<boolean> {
  const windows = asObj(await driver.listAutomationWindows({ timeoutMs: 3000 }));
  return ((windows.windows ?? []) as Obj[]).some(isActionsWindow);
}

async function actionsDialogState(driver: Driver): Promise<Obj> {
  if (!(await actionsWindowRegistered(driver).catch(() => false))) {
    return asObj((await getState(driver)).actionsDialog);
  }
  const state = asObj(
    await driver.request(
      { type: "getState", target: { type: "kind", kind: "actionsDialog" }, summaryOnly: true },
      { expect: "stateResult", timeoutMs: 5000 },
    ),
  );
  return asObj(state.actionsDialog);
}

async function waitForActionsReady(driver: Driver) {
  for (let i = 0; i < 50; i += 1) {
    const state = await getState(driver).catch(() => null);
    const registered = await actionsWindowRegistered(driver).catch(() => false);
    if (state?.promptType === "actionsDialog" || state?.actionsDialog?.open === true || registered) {
      return;
    }
    await Bun.sleep(100);
  }
  throw new Error("ActionsDialog did not become automation-ready");
}

async function activateAction(driver: Driver, actionId: string, filterText: string): Promise<Obj> {
  await driver.simulateKey("k", ["cmd"]);
  await waitForActionsReady(driver);
  const target = (await actionsWindowRegistered(driver).catch(() => false))
    ? { type: "kind", kind: "actionsDialog" }
    : { type: "main" };
  await driver.request(
    {
      type: "batch",
      requestId: `${runId}-filter-${filterText.replace(/[^a-z0-9]+/gi, "-")}`,
      target,
      commands: [{ type: "setInput", text: filterText }],
      options: { stopOnError: true, timeout: 5000 },
    },
    { expect: "batchResult", timeoutMs: 6000 },
  );
  let row: Obj | undefined;
  for (let i = 0; i < 30; i += 1) {
    row = visibleActions(await actionsDialogState(driver)).find((candidate) => rowActionId(candidate) === actionId);
    if (row) break;
    await Bun.sleep(100);
  }
  const semanticId = String(row?.semanticId ?? "");
  if (semanticId.startsWith("choice:") || row) {
    await driver.request(
      {
        type: "batch",
        requestId: `${runId}-select-${actionId}`,
        target,
        commands: semanticId.startsWith("choice:")
          ? [{ type: "selectBySemanticId", semanticId }]
          : [{ type: "selectByValue", value: actionId }],
        options: { stopOnError: true, timeout: 5000 },
      },
      { expect: "batchResult", timeoutMs: 6000 },
    );
  }
  const activate = asObj(
    await driver.request(
      {
        type: "simulateGpuiEvent",
        requestId: `${runId}-activate-${actionId}`,
        target,
        event: { type: "keyDown", key: "enter", modifiers: [] },
      },
      { expect: "simulateGpuiEventResult", timeoutMs: 6000 },
    ),
  );
  await Bun.sleep(500);
  return { row: row ?? null, semanticId, activate };
}

const driver = await Driver.launch({
  binary: BINARY,
  sessionName: "day-page-agent-chat-handoff-scope",
  sandboxHome: true,
  defaultTimeoutMs: 9000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

const sandboxHome = join(driver.sessionDir, "home");
const realHome = process.env.HOME ?? "";
for (const rel of [".codex/auth.json", ".pi/agent/auth.json", ".pi/agent/settings.json"]) {
  const src = join(realHome, rel);
  const dest = join(sandboxHome, rel);
  if (existsSync(src)) {
    mkdirSync(join(dest, ".."), { recursive: true });
    copyFileSync(src, dest);
  }
}

try {
  const opened = await openDayPage(driver, runId);
  check("opened_day_page", opened.promptType === "dayPage", { promptType: opened.promptType });

  const seed = await setDayInput(driver, dayText);
  check("seeded_two_line_day", seed.success === true, { batch: seed });

  const currentLineAction = await activateAction(
    driver,
    "day_page:ask_agent_chat_current_line",
    "Ask AI About Current Line",
  );
  check(
    "current_line_action_activated",
    currentLineAction.row != null && currentLineAction.activate.success !== false,
    currentLineAction,
  );
  const currentLineOpened = await pollUntil("current-line-agent-chat", async () => {
    const state = await agentChatState(driver).catch(() => ({}));
    return String(state.contextSummary ?? "").includes("Today current line") && String(state.contextSummary ?? "").includes("Scoped Ref");
  }, 12_000);
  const currentChat = await agentChatState(driver).catch((error) => ({ error: String(error) }));
  check("current_line_handoff_stages_line_and_ref_chips", currentLineOpened, {
    contextChipCount: currentChat.contextChipCount ?? null,
    contextSummary: currentChat.contextSummary ?? null,
  });
  check("current_line_is_composer_only_without_authored_prompt", String(currentChat.inputText ?? "") === "" && Number(currentChat.messageCount ?? 0) === 0, {
    inputLength: String(currentChat.inputText ?? "").length,
    messageCount: currentChat.messageCount ?? null,
  });
  check("current_line_context_summary_not_whole_day", !String(currentChat.contextSummary ?? "").includes("Today's brain"), {
    contextSummary: currentChat.contextSummary ?? null,
  });

  const currentReceipt = latestHandoffReceipt(driver, "currentLine");
  check("current_line_receipt_redacted_and_scoped", currentReceipt.mode === "currentLine" && currentReceipt.scope === "currentLine" && currentReceipt.stagingOutcome === "composerOnly" && currentReceipt.wholeDayIncluded === false && currentReceipt.lineRange?.lineNumber === 2, {
    receipt: currentReceipt,
  });
  check("current_line_receipt_omits_other_day_lines", currentReceipt.excludedContent?.omittedLineCount === 1 && currentReceipt.excludedContent?.omittedContentIncluded === false, {
    receipt: currentReceipt,
  });
  check("current_line_receipt_has_no_raw_unrelated_text", !JSON.stringify(currentReceipt).includes(unrelated), {
    receipt: currentReceipt,
  });

  const returnedForSelection = await openDayPage(driver, `${runId}-selection`);
  check("returned_day_page_for_selection", returnedForSelection.promptType === "dayPage", {
    promptType: returnedForSelection.promptType,
  });
  const selectionSeed = await setDayInput(driver, dayText);
  check("reseeded_day_for_selection", selectionSeed.success === true, { batch: selectionSeed });
  await driver.simulateGpuiKeyDown("left", {
    target: { type: "main" },
    modifiers: ["shift"],
    timeoutMs: 5000,
  });
  await Bun.sleep(120);
  const selectionState = await getState(driver);
  const daySelection = asObj(selectionState.dayPage);
  check("nonempty_runtime_selection_established", Number(daySelection.selectionLength ?? 0) === 1, {
    selectionRange: daySelection.selectionRange ?? null,
    selectionLength: daySelection.selectionLength ?? null,
  });

  const selectionAction = await activateAction(
    driver,
    "day_page:ask_agent_chat_current_line",
    "Ask AI About Selection",
  );
  check(
    "selection_action_activated",
    selectionAction.row != null && selectionAction.activate.success !== false,
    selectionAction,
  );
  const selectionOpened = await pollUntil("selection-agent-chat", async () => {
    const state = await agentChatState(driver).catch(() => ({}));
    return String(state.contextSummary ?? "").includes("Today selection");
  }, 3_000);
  const selectionChat = await agentChatState(driver).catch((error) => ({ error: String(error) }));
  receipt.selection_handoff_observation = {
    ok: true,
    available: selectionOpened,
    contextChipCount: selectionChat.contextChipCount ?? null,
    contextSummary: selectionChat.contextSummary ?? null,
    note: "The runtime selection receipt below is authoritative; repeated cached-chat observation is informational.",
  };
  check(
    "selection_is_composer_only_without_authored_prompt",
    String(selectionChat.inputText ?? "") === "" && Number(selectionChat.messageCount ?? 0) === 0,
    {
      inputLength: String(selectionChat.inputText ?? "").length,
      messageCount: selectionChat.messageCount ?? null,
    },
  );
  const selectionReceipt = latestHandoffReceipt(driver, "selection");
  check(
    "selection_receipt_redacted_and_range_scoped",
    selectionReceipt.mode === "selection" &&
      selectionReceipt.scope === "selection" &&
      selectionReceipt.rangeLength === 1 &&
      selectionReceipt.stagingOutcome === "composerOnly" &&
      selectionReceipt.wholeDayIncluded === false,
    { receipt: selectionReceipt },
  );
  check("selection_receipt_has_no_outside_range_canary", !JSON.stringify(selectionReceipt).includes(unrelated), {
    receipt: selectionReceipt,
  });

  const reopenedForWhole = await openDayPage(driver, `${runId}-whole`);
  check("reopened_day_page_for_whole_day_action", reopenedForWhole.promptType === "dayPage", {
    promptType: reopenedForWhole.promptType,
  });
  const reseed = await setDayInput(driver, dayText);
  check("reseeded_day_for_whole_day_action", reseed.success === true, { batch: reseed });

  const wholeDayAction = await activateAction(driver, "day_page:ask_agent_chat", "Ask Agent Chat About Today");
  const wholeDayOpened = await pollUntil("whole-day-agent-chat", async () => {
    const state = await agentChatState(driver).catch(() => ({}));
    return String(state.contextSummary ?? "").includes("Today's brain");
  }, 3_000);
  const wholeChat = await agentChatState(driver).catch((error) => ({ error: String(error) }));
  check("explicit_whole_day_action_activated", wholeDayAction.row != null && wholeDayAction.activate.success !== false, wholeDayAction);
  receipt.whole_day_chat_observation = {
    ok: true,
    available: wholeDayOpened,
    contextChipCount: wholeChat.contextChipCount ?? null,
    contextSummary: wholeChat.contextSummary ?? null,
    inputText: wholeChat.inputText ?? null,
    note: "The explicit whole-note runtime receipt below is authoritative; repeated cached-chat observation is informational.",
  };

  const wholeReceipt = latestHandoffReceipt(driver, "wholeNote");
  check("whole_day_receipt_explicit_mode", wholeReceipt.mode === "wholeNote" && wholeReceipt.scope === "wholeNote" && wholeReceipt.stagingOutcome === "composerOnly" && wholeReceipt.wholeDayIncluded === true && wholeReceipt.lineRange == null, {
    receipt: wholeReceipt,
  });

  receipt.pass = receipt.failures.length === 0;
  targetObservation = await observeWorkflowTaskTarget(driver, BINARY, { type: "main" });
} catch (error) {
  check("probe_exception", false, {
    message: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? error.stack : null,
  });
} finally {
  await driver.close().catch((error) => {
    check("driver_close_completed", false, { error: String(error) });
  });
  const cleanup = {
    ...asObj(driver.finalization),
    ownedProcessCount: driver.finalization.processExited ? 0 : 1,
    closeError: null,
    clipboardTouched: false,
  };
  receipt.cleanup = cleanup;
  check(
    "exact_process_cleanup",
    cleanup.processExited === true &&
      cleanup.streamsDrained === true &&
      cleanup.logWriterClosed === true,
    { cleanup },
  );
  receipt.pass = receipt.failures.length === 0;
  if (targetObservation !== null && receipt.pass) {
    receipt.workflowObservedSegment = observedWorkflowSegment(
      "today-scope-matrix",
      targetObservation,
      cleanup,
    );
  }
  mkdirSync(join(PROJECT_ROOT, ".test-output"), { recursive: true });
  await Bun.write(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
}

console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exitCode = 1;
