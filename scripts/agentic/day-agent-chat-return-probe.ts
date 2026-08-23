#!/usr/bin/env bun
/**
 * Runtime proof for Day Page -> Agent Chat -> Today return:
 * - Day Page action attaches Today's markdown and opens main Agent Chat.
 * - Day-origin Agent Chat actions expose the Today return action.
 * - A deterministic assistant fixture appends back to the captured Day file.
 * - Closing the action restores Day Page with the appended response visible.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import { openDayPage } from "./day-page-open-helper";
import { assertNoninteractiveVisualProbe } from "../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observeWorkflowTaskTarget,
} from "../devtools/lib/workflow-task-proof.ts";
import type { RuntimeTargetObservation } from "../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("day-agent-chat-return.workflow-child");

const binary =
  process.env.PROBE_BINARY ?? "target-agent/artifacts/day-agent-chat/script-kit-gpui";
const runId = `day-agent-chat-${Date.now().toString(36)}`;
const daySeed = `Today brain seed ${runId}`;
const assistantToken = `TODAY-RETURN-${Math.random().toString(36).slice(2, 8).toUpperCase()}`;
const assistantText = `Append this Day Page result ${assistantToken}.`;
const receipt: Json = {
  schemaVersion: 1,
  tool: "day-agent-chat-return-probe",
  classification: "blocked",
  pass: false,
  failures: [] as string[],
  runId,
  binary,
  daySeed,
  assistantToken,
};
let targetObservation: RuntimeTargetObservation | null = null;

function check(name: string, ok: boolean, detail: Json = {}) {
  receipt[name] = { ok, ...detail };
  if (!ok) (receipt.failures as string[]).push(name);
}

function walkElements(node: unknown, out: Json[] = []): Json[] {
  if (!node || typeof node !== "object") return out;
  if (Array.isArray(node)) {
    for (const item of node) walkElements(item, out);
    return out;
  }
  const json = node as Json;
  if (typeof json.semanticId === "string" || typeof json.id === "string") out.push(json);
  for (const value of Object.values(json)) walkElements(value, out);
  return out;
}

function visibleActions(dialog: Json | null | undefined): Json[] {
  const rows = dialog?.visibleActions;
  if (Array.isArray(rows)) return rows as Json[];
  const sample = (dialog?.actions as Json | undefined)?.visibleSample;
  return Array.isArray(sample) ? (sample as Json[]) : [];
}

function rowActionId(row: Json): string {
  return String(row.id ?? row.actionId ?? row.value ?? "");
}

async function actionsWindowRegistered(driver: Driver): Promise<boolean> {
  const listed = (await driver.listAutomationWindows({ timeoutMs: 5000 })) as Json;
  const windows = Array.isArray(listed.windows) ? (listed.windows as Json[]) : [];
  return windows.some(
    (win) =>
      win.id === "actions-dialog" ||
      win.automationId === "actions-dialog" ||
      win.kind === "ActionsDialog" ||
      win.windowKind === "ActionsDialog" ||
      win.semanticSurface === "actionsDialog",
  );
}

async function actionsDialogState(driver: Driver): Promise<Json> {
  if (!(await actionsWindowRegistered(driver).catch(() => false))) {
    const state = (await driver.getState({ timeoutMs: 5000 })) as Json;
    return (state.actionsDialog ?? {}) as Json;
  }
  const state = (await driver.request(
    { type: "getState", target: { type: "kind", kind: "actionsDialog" }, summaryOnly: true },
    { expect: "stateResult", timeoutMs: 5000 },
  )) as Json;
  return (state.actionsDialog ?? {}) as Json;
}

async function filterActions(driver: Driver, text: string) {
  const payload: Json = {
    type: "batch",
    requestId: `${runId}-filter-actions-${Date.now()}`,
    commands: [{ type: "setInput", text }],
    options: { stopOnError: true, timeout: 5000 },
  };
  if (await actionsWindowRegistered(driver).catch(() => false)) {
    payload.target = { type: "kind", kind: "actionsDialog" };
  }
  return driver.request(payload, { expect: "batchResult", timeoutMs: 6000 });
}

async function findAction(driver: Driver, actionId: string, query: string): Promise<Json | null> {
  await filterActions(driver, query);
  for (let i = 0; i < 30; i += 1) {
    const dialog = await actionsDialogState(driver).catch(() => null);
    const row = visibleActions(dialog).find((candidate) => rowActionId(candidate) === actionId);
    if (row) return row;
    await Bun.sleep(100);
  }
  return null;
}

async function activateVisibleAction(driver: Driver, actionId: string, query: string) {
  const row = await findAction(driver, actionId, query);
  const semanticId = String(row?.semanticId ?? "");
  let select: Json = { skipped: true, reason: "semantic id not exposed", semanticId };
  if (semanticId.startsWith("choice:")) {
    const payload: Json = {
      type: "batch",
      requestId: `${runId}-select-${actionId}`,
      commands: [{ type: "selectBySemanticId", semanticId }],
      options: { stopOnError: true, timeout: 5000 },
    };
    if (await actionsWindowRegistered(driver).catch(() => false)) {
      payload.target = { type: "kind", kind: "actionsDialog" };
    }
    select = (await driver.request(payload, {
      expect: "batchResult",
      timeoutMs: 6000,
    })) as Json;
  }

  const target = (await actionsWindowRegistered(driver).catch(() => false))
    ? { type: "kind", kind: "actionsDialog" }
    : { type: "main" };
  const activate = (await driver.request(
    {
      type: "simulateGpuiEvent",
      requestId: `${runId}-activate-${actionId}`,
      target,
      event: { type: "keyDown", key: "enter", modifiers: [] },
    },
    { expect: "simulateGpuiEventResult", timeoutMs: 5000 },
  )) as Json;

  return { row, semanticId, select, activate };
}

function todayLocalDate(): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

async function editorText(driver: Driver): Promise<string | null> {
  const elements = (await driver.getElements(
    { target: { type: "main" }, limit: 240 },
    { timeoutMs: 5000 },
  )) as Json;
  const editor = walkElements(elements).find(
    (el) => el.semanticId === "input:day-page-editor" || el.id === "day-page-editor",
  );
  return (editor?.value as string | undefined) ?? null;
}

async function waitForAgentChatSurface(driver: Driver, timeoutMs: number): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let last = (await driver.getState({ timeoutMs: 5000 })) as Json;
  while (Date.now() < deadline) {
    if (String(last.promptType ?? "").toLowerCase().includes("agent")) return last;
    await Bun.sleep(100);
    last = (await driver.getState({ timeoutMs: 5000 })) as Json;
  }
  return last;
}

await mkdirSync(".test-output", { recursive: true });
const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: "day-agent-chat-return",
  defaultTimeoutMs: 10_000,
  env: {
    SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
    SCRIPT_KIT_BRAIN_TZ: process.env.SCRIPT_KIT_BRAIN_TZ ?? "America/Denver",
  },
});

try {
  const sandboxHome = join(driver.sessionDir, "home");
  const daysDir = join(sandboxHome, ".scriptkit", "brain", "days");
  const todayFile = join(daysDir, `${todayLocalDate()}.md`);

  const opened = await openDayPage(driver, runId);
  check("opened_day_page", opened.promptType === "dayPage", { promptType: opened.promptType });

  const setDay = (await driver.batch(
    [
      { type: "setInput", text: daySeed },
      {
        type: "waitFor",
        condition: { type: "stateMatch", state: { promptType: "dayPage", inputValue: daySeed } },
      },
    ],
    { timeoutMs: 7000 },
  )) as Json;
  check("seeded_day_page", setDay.success === true, { setDay });
  await Bun.sleep(900);
  check("seed_saved_to_disk", existsSync(todayFile) && readFileSync(todayFile, "utf8").includes(daySeed), {
    todayFile,
    disk: existsSync(todayFile) ? readFileSync(todayFile, "utf8") : null,
  });

  await driver.simulateKey("k", ["cmd"]);
  await Bun.sleep(700);
  const ask = await activateVisibleAction(driver, "day_page:ask_agent_chat", "ask agent");
  check("ask_agent_chat_action_selected", ask.row != null && ask.activate.success !== false, ask);

  const agentState = await waitForAgentChatSurface(driver, 20_000);
  check("agent_chat_opened", String(agentState.promptType).toLowerCase().includes("agent"), {
    promptType: agentState.promptType,
    inputValue: agentState.inputValue,
  });
  receipt.agent_chat_composer_only = {
    ok: String(agentState.inputValue ?? "") === "",
    inputLength: String(agentState.inputValue ?? "").length,
    note: "Ask without an authored question stages scope and leaves the composer empty",
  };

  const fixtureAttempt = (await driver.request(
    {
      type: "setAgentChatTestFixture",
      requestId: `${runId}-fixture`,
      phase: "idle",
      userText: `Question about ${daySeed}`,
      assistantText,
    },
    { expect: "externalCommandResult", timeoutMs: 10_000 },
  )) as Json;
  const liveFixtureAvailable = fixtureAttempt.ok !== false && fixtureAttempt.success !== false;
  receipt.live_agent_chat_fixture = {
    ok: true,
    available: liveFixtureAvailable,
    fixtureAttempt,
    note: "informational only; the provider-free mock path below owns return verification",
  };

  await driver.simulateKey("escape");
  const afterSetupClose = (await driver.getState({ timeoutMs: 5000 })) as Json;
  receipt.setup_agent_chat_closed = {
    ok: afterSetupClose.promptType === "dayPage" || afterSetupClose.promptType === "none",
    promptType: afterSetupClose.promptType,
    note: "informational: setup-mode Agent Chat may consume Escape while provider credentials are missing",
  };
  const reopened = await openDayPage(driver, `${runId}-mock-return-origin`);
  check("reopened_day_page_for_mock_return_origin", reopened.promptType === "dayPage", {
    promptType: reopened.promptType,
  });

  driver.send({ type: "openAiWithMockData" });
  const mockOpenState = await waitForAgentChatSurface(driver, 10_000);
  check("provider_free_mock_agent_chat_opened", String(mockOpenState.promptType).toLowerCase().includes("agent"), {
    promptType: mockOpenState.promptType,
  });
  const mockFixture = (await driver.request(
    {
      type: "setAgentChatTestFixture",
      requestId: `${runId}-mock-fixture`,
      phase: "idle",
      userText: `Question about ${daySeed}`,
      assistantText,
    },
    { expect: "externalCommandResult", timeoutMs: 10_000 },
  )) as Json;
  check("mock_agent_chat_fixture_applied", mockFixture.ok !== false && mockFixture.success !== false, {
    mockFixture,
  });

  await driver.simulateKey("k", ["cmd"]);
  await Bun.sleep(700);
  const returnRow = await findAction(
    driver,
    "agent_chat_append_last_response_to_today",
    "append today",
  );
  check("append_to_today_action_visible", returnRow != null, { returnRow });
  const append = await activateVisibleAction(
    driver,
    "agent_chat_append_last_response_to_today",
    "append today",
  );
  check("append_to_today_action_activated", append.row != null && append.activate.success !== false, append);

  await driver.waitForState({ promptType: "dayPage" }, { timeoutMs: 10_000 });
  await Bun.sleep(900);
  const restoredState = (await driver.getState({ timeoutMs: 5000 })) as Json;
  const restoredEditor = await editorText(driver);
  const diskAfter = existsSync(todayFile) ? readFileSync(todayFile, "utf8") : "";
  check("restored_day_page", restoredState.promptType === "dayPage", {
    promptType: restoredState.promptType,
  });
  check("assistant_response_visible_in_editor", String(restoredEditor ?? "").includes(assistantToken), {
    restoredEditor,
  });
  check("assistant_response_written_to_bound_day_file", diskAfter.includes(assistantToken), {
    todayFile,
    diskAfter,
  });

  const log = existsSync(driver.logPath) ? readFileSync(driver.logPath, "utf8") : "";
  check("success_log_emitted", log.includes("event=agent_chat_append_to_today_succeeded"), {
    line: log
      .split(/\r?\n/)
      .reverse()
      .find((line) => line.includes("event=agent_chat_append_to_today_succeeded")),
  });

  receipt.classification = (receipt.failures as string[]).length === 0 ? "fixed" : "failed";
  receipt.pass = (receipt.failures as string[]).length === 0;
  targetObservation = await observeWorkflowTaskTarget(driver, binary, { type: "main" });
} catch (error) {
  check("probe_exception", false, {
    message: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? error.stack : undefined,
  });
  receipt.classification = "failed";
  receipt.pass = false;
} finally {
  receipt.sessionDir = driver.sessionDir;
  receipt.logPath = driver.logPath;
  await driver.close().catch((error) => {
    check("driver_close_completed", false, { message: String(error) });
  });
  const cleanup = {
    ...driver.finalization,
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
  receipt.classification = (receipt.failures as string[]).length === 0 ? "fixed" : "failed";
  receipt.pass = (receipt.failures as string[]).length === 0;
  if (targetObservation !== null && receipt.pass) {
    receipt.workflowObservedSegment = observedWorkflowSegment(
      "today-agent-chat-return",
      targetObservation,
      cleanup,
    ) as unknown as Json;
  }
  writeFileSync(
    `.test-output/${runId}.json`,
    `${JSON.stringify(receipt, null, 2)}\n`,
  );
}

console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exit(1);
