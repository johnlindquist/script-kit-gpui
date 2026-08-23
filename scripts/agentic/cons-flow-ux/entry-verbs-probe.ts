#!/usr/bin/env bun

import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
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

assertNoninteractiveVisualProbe("cons-flow-ux.entry-verbs");

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY ??
    "target-agent/artifacts/cons-flow-c03/script-kit-gpui",
);
const runDir = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR ??
    ".artifacts/consistency/cons-flow-ux/c03-entry-verbs-v1",
);

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

async function waitForTopState(
  driver: Driver,
  predicate: (state: Json) => boolean,
  label: string,
  timeoutMs = 12_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let state = await driver.getState({ timeoutMs: 5_000 });
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(40);
    state = await driver.getState({ timeoutMs: 5_000 });
  }
  assert(predicate(state), `timed out waiting for ${label}`, {
    promptType: state.promptType,
    windowVisible: state.windowVisible,
    inputLength: String(state.inputValue ?? "").length,
  });
  return state;
}

async function agentChatState(driver: Driver): Promise<Json> {
  return driver.request(
    { type: "getAgentChatState", target: { type: "id", id: "main" } },
    { expect: "agentChatStateResult", timeoutMs: 10_000 },
  );
}

async function waitForAgentChatState(
  driver: Driver,
  predicate: (state: Json) => boolean,
  label: string,
  timeoutMs = 12_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let state = await agentChatState(driver);
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(40);
    state = await agentChatState(driver);
  }
  assert(predicate(state), `timed out waiting for ${label}`, {
    status: state.status,
    messageCount: state.messageCount,
    inputLength: String(state.inputText ?? "").length,
    contextChipCount: state.contextChipCount,
  });
  return state;
}

async function waitForEntryReceipt(
  driver: Driver,
  feedback: "Opened" | "Asked" | "Sent" | "Added" | "Continued" | "Refused",
  timeoutMs = 12_000,
): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const response = await driver.getLogs({ limit: 500 });
    const entries = Array.isArray(response.entries) ? response.entries as Json[] : [];
    const line = entries
      .map((entry) => String(entry.message ?? ""))
      .find((message) =>
        message.includes("agent_chat_entry_completed") &&
        message.includes(`feedback=${feedback}`),
      );
    if (line) return line;
    await Bun.sleep(50);
  }
  const response = await driver.getLogs({ limit: 500 });
  throw new Error(`timed out waiting for ${feedback} entry receipt; entry log present=${JSON.stringify(response).includes("agent_chat_entry_completed")}`);
}

async function outcomeHasThreadIdentity(
  driver: Driver,
  feedback: string,
): Promise<boolean> {
  await Bun.sleep(100);
  const log = await Bun.file(resolve(driver.sessionDir, "app.log")).text();
  return log
    .split("\n")
    .some((line) =>
      !line.startsWith("{") &&
      line.includes("event=agent_chat_entry_completed") &&
      line.includes(`feedback=${feedback}`) &&
      line.includes("destinationThreadId"),
    );
}

async function gesture(driver: Driver, phase: "down" | "up", requestId: string): Promise<void> {
  const response = await driver.request(
    { type: "simulateMainHotkeyGesture", phase, requestId },
    { expect: "externalCommandResult", timeoutMs: 5_000 },
  );
  assert(response.ok === true, `gesture ${requestId} failed`, { ok: response.ok });
}

function safeState(state: Json): Json {
  return {
    status: state.status,
    uiVariant: state.uiVariant,
    inputLength: String(state.inputText ?? "").length,
    messageCount: state.messageCount,
    contextChipCount: state.contextChipCount,
    resolvedTarget: state.resolvedTarget ?? null,
  };
}

const scenarios: Json[] = [];
const failures: string[] = [];
const cleanup: Json[] = [];
const observedSegments = new Map<string, WorkflowObservedSegment>();

async function runScenario(
  name: string,
  body: (driver: Driver) => Promise<Json>,
  options: { seedAgentAuth?: boolean; env?: Record<string, string> } = {},
): Promise<void> {
  let driver: Driver | null = null;
  let targetObservation: RuntimeTargetObservation | null = null;
  let result: Json = { name, status: "FAILED" };
  try {
    driver = await Driver.launch({
      binary,
      sessionName: `cons-flow-c03-${name}`,
      sandboxHome: true,
      seedAgentAuth: options.seedAgentAuth ?? true,
      sharedModels: false,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1",
        SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
        SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
        SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
        ...(options.env ?? {}),
      },
      readyTimeoutMs: 30_000,
      defaultTimeoutMs: 15_000,
    });
    await driver.waitForSettle();
    result = { name, status: "PASS", ...(await body(driver)) };
    targetObservation = await observeWorkflowTaskTarget(driver, binary, { type: "main" });
  } catch (error) {
    console.error(`[${name}] private diagnostic:`, error);
    failures.push(name);
    result = {
      name,
      status: "FAILED",
      errorName: error instanceof Error ? error.name : "UnknownError",
      safeMessage: "C03 runtime assertion failed; inspect the private Driver session log.",
    };
  } finally {
    if (driver) {
      let closeError: string | null = null;
      try {
        await driver.close();
      } catch (error) {
        closeError = error instanceof Error ? error.name : "UnknownCloseError";
        failures.push(`${name}.cleanup`);
      }
      const ownedPids = exactExecutablePids(binary);
      const closeReceipt = {
        name,
        processExited: driver.finalization.processExited,
        streamsDrained: driver.finalization.streamsDrained,
        logWriterClosed: driver.finalization.logWriterClosed,
        ownedProcessCount: ownedPids.length,
        closeError,
        clipboardTouched: false,
      };
      cleanup.push(closeReceipt);
      if (
        !closeReceipt.processExited ||
        !closeReceipt.streamsDrained ||
        !closeReceipt.logWriterClosed ||
        closeReceipt.ownedProcessCount !== 0
      ) {
        failures.push(`${name}.cleanup`);
      } else if (targetObservation !== null) {
        observedSegments.set(
          name,
          observedWorkflowSegment(name, targetObservation, closeReceipt),
        );
      }
    }
    scenarios.push(result);
  }
}

await runScenario("open-draft", async (driver) => {
  const dispatch = await driver.triggerAction("ask_ai_settings", { host: "main", timeoutMs: 10_000 });
  assert(dispatch.ok === true || dispatch.success === true, "ask_ai_settings action was not dispatched", dispatch);
  await waitForTopState(driver, (state) => state.promptType === "agentChatChat", "Open draft Agent Chat");
  const state = await waitForAgentChatState(
    driver,
    (snapshot) => String(snapshot.inputText ?? "").length > 0,
    "staged settings draft",
  );
  const receiptLine = await waitForEntryReceipt(driver, "Opened");
  assert(Number(state.messageCount ?? 0) === 0, "Open constructed a submitted turn", safeState(state));
  assert(!receiptLine.includes("Sent"), "opened-only result claimed Sent");
  return {
    visibleVerb: "Open",
    state: safeState(state),
    outcomeHasThreadIdentity: await outcomeHasThreadIdentity(driver, "Opened"),
    submission: "notRequested",
  };
});

await runScenario("ask-cmd-enter", async (driver) => {
  await driver.setFilterAndWait("c03 ask runtime canary");
  const dispatch = await driver.simulateGpuiKeyDown("enter", {
    modifiers: ["cmd"],
    target: { type: "kind", kind: "main" },
    timeoutMs: 10_000,
  });
  assert(dispatch.success === true, "Cmd+Enter did not dispatch", dispatch);
  await waitForTopState(driver, (state) => state.promptType === "agentChatChat", "Ask Agent Chat");
  const state = await waitForAgentChatState(
    driver,
    (snapshot) => Number(snapshot.messageCount ?? 0) === 1,
    "exactly one submitted Ask turn",
  );
  await waitForEntryReceipt(driver, "Asked");
  assert(Number(state.messageCount ?? 0) === 1, "Ask did not produce exactly one submitted turn", safeState(state));
  return {
    visibleVerb: "Ask",
    state: safeState(state),
    outcomeHasThreadIdentity: await outcomeHasThreadIdentity(driver, "Asked"),
    submission: "accepted",
  };
});

await runScenario("quick-question", async (driver) => {
  await gesture(driver, "down", "c03-open-down");
  await Bun.sleep(30);
  await gesture(driver, "up", "c03-open-up");
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 8_000 });
  await Bun.sleep(400);
  await gesture(driver, "down", "c03-double-one-down");
  await gesture(driver, "up", "c03-double-one-up");
  await Bun.sleep(60);
  await gesture(driver, "down", "c03-double-two-down");
  await gesture(driver, "up", "c03-double-two-up");
  await waitForTopState(driver, (state) => state.promptType === "agentChatChat", "Quick Question Agent Chat");
  const state = await waitForAgentChatState(
    driver,
    (snapshot) => snapshot.status !== "setup",
    "Quick Question live state",
  );
  await waitForEntryReceipt(driver, "Opened");
  assert(String(state.inputText ?? "") === "", "Quick Question composer was not empty", safeState(state));
  assert(Number(state.contextChipCount ?? 0) === 0, "Quick Question inherited context", safeState(state));
  assert(Number(state.messageCount ?? 0) === 0, "Quick Question submitted a turn", safeState(state));
  return {
    visibleVerb: "Open",
    state: safeState(state),
    outcomeHasThreadIdentity: await outcomeHasThreadIdentity(driver, "Opened"),
    submission: "notRequested",
  };
});

await runScenario(
  "preflight-refusal",
  async (driver) => {
    await driver.setFilterAndWait("preserve this exact launcher draft");
    const before = await driver.getState({ timeoutMs: 5_000 });
    const dispatch = await driver.triggerAction("ask_ai_settings", { host: "main", timeoutMs: 10_000 });
    assert(dispatch.ok === true || dispatch.success === true, "preflight action was not dispatched", dispatch);
    await Bun.sleep(750);
    const after = await driver.getState({ timeoutMs: 5_000 });
    assert(after.promptType === before.promptType, "failed preflight replaced the source view", { before: before.promptType, after: after.promptType });
    assert(after.inputValue === before.inputValue, "failed preflight changed the launcher draft", {
      beforeLength: String(before.inputValue ?? "").length,
      afterLength: String(after.inputValue ?? "").length,
    });
    return {
      visibleVerb: "Refused",
      sourceViewPreserved: true,
      draftLength: String(after.inputValue ?? "").length,
    };
  },
  {
    seedAgentAuth: false,
    env: { SCRIPT_KIT_TEST_AGENT_CHAT_PREFLIGHT_REFUSAL: "1" },
  },
);

const receipt: Json = {
  schemaVersion: 1,
  classification: failures.length === 0 ? "RUNTIME-CONFIRMED" : "RUNTIME-FAILED",
  binaryArtifact: "cons-flow-c03",
  binarySha256: sha256(binary),
  scenarios,
  negativeControls: {
    openDidNotSubmit: scenarios.some((item) => item.name === "open-draft" && item.status === "PASS"),
    quickQuestionDidNotSubmitOrInheritContext: scenarios.some((item) => item.name === "quick-question" && item.status === "PASS"),
    askProducedExactlyOneTurn: scenarios.some((item) => item.name === "ask-cmd-enter" && item.status === "PASS"),
    failedPreflightPreservedSource: scenarios.some((item) => item.name === "preflight-refusal" && item.status === "PASS"),
    receiptContainsNoAuthoredText: true,
  },
  cleanup,
  failures,
};

for (const taskId of ["WF-002", "WF-008"] as const) {
  let taskReceipt: Json;
  try {
    assert(receipt.classification === "RUNTIME-CONFIRMED", "entry-verb journey did not pass");
    const stageIds = taskId === "WF-002"
      ? ["open-draft", "preflight-refusal"]
      : ["open-draft", "quick-question", "ask-cmd-enter"];
    const segments = stageIds.map((id) => {
      const segment = observedSegments.get(id);
      assert(segment, `entry-verb journey omitted actual observed segment: ${id}`);
      return segment;
    });
    const stages = stageIds.map((id, index) => {
      const scenario = scenarios.find((entry) => entry.name === id);
      assert(scenario?.status === "PASS", `entry-verb journey omitted passing stage: ${id}`);
      return observedWorkflowStage({
        id,
        primitiveId: "devtools.act",
        segment: segments[index]!,
        command: "agentChat.entryAction",
        requestId: `${taskId}:${id}`,
        result: scenario,
        pass: true,
      });
    });
    const observedControls = receipt.negativeControls as Json;
    const controls = taskId === "WF-002"
      ? {
          "failed-preflight-preserves-source": observedControls.failedPreflightPreservedSource === true,
          "open-never-submits": observedControls.openDidNotSubmit === true,
        }
      : {
          "open-never-submits": observedControls.openDidNotSubmit === true,
          "quick-question-never-inherits-context":
            observedControls.quickQuestionDidNotSubmitOrInheritContext === true,
          "ask-submits-exactly-once": observedControls.askProducedExactlyOneTurn === true,
        };
    taskReceipt = prepareWorkflowTaskProof(taskId, {
      producerOwner: "scripts/agentic/cons-flow-ux/entry-verbs-probe.ts",
      segments,
      stages,
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
      taskId,
      error instanceof Error ? error.message : String(error),
    ).receipt as Json;
  }
  const taskDir = resolve(runDir, taskId);
  await mkdir(taskDir, { recursive: true });
  await writeFile(resolve(taskDir, "receipt.json"), `${JSON.stringify(taskReceipt, null, 2)}\n`);
  writeWorkflowTaskProof(taskId, taskReceipt);
}

console.log(JSON.stringify(receipt, null, 2));
assert(receipt.classification === "RUNTIME-CONFIRMED", "C03 runtime proof failed", receipt);
assert(exactExecutablePids(binary).length === 0, "C03 left an app instance running");
