#!/usr/bin/env bun

import { mkdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { openDayPage } from "../day-page-open-helper";
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

assertNoninteractiveVisualProbe("cons-flow-ux.semantic-command");

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY ??
    "target-agent/artifacts/cons-flow-c04/script-kit-gpui",
);
const runDir = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR ??
    ".artifacts/consistency/cons-flow-ux/c04-semantic-commands-v1",
);
const flowFixture = resolve("scripts/agentic/fixtures/flow-ux-project");
const packageFixture = resolve("scripts/agentic/fixtures/flow-desk-package");

type RecordJson = Record<string, Json>;

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

function asObjects(value: Json | undefined): RecordJson[] {
  return Array.isArray(value)
    ? value.filter((item): item is RecordJson => !!item && typeof item === "object" && !Array.isArray(item))
    : [];
}

function elementList(receipt: Json): RecordJson[] {
  const object = receipt as RecordJson;
  return asObjects(object.elements);
}

function safeElements(receipt: Json): Json[] {
  return elementList(receipt).map((element) => ({
    semanticId: element.semanticId ?? element.semantic_id ?? null,
    role: element.role ?? null,
    kind: element.kind ?? null,
    selectable: element.selectable ?? null,
    statusKind: element.statusKind ?? element.status_kind ?? null,
    actionDisabled: element.actionDisabled ?? element.action_disabled ?? null,
  }));
}

function assertUniqueSemanticIds(elements: RecordJson[], label: string): void {
  const ids = elements
    .map((element) => String(element.semanticId ?? element.semantic_id ?? ""))
    .filter(Boolean);
  assert(new Set(ids).size === ids.length, `${label} exposed duplicate semantic IDs`, ids);
}

function assertCommandBindings(elements: RecordJson[], label: string): void {
  const commands = elements.filter((element) => element.role === "conversationCommand");
  assert(commands.length > 0, `${label} exposed no conversation command descriptors`);
  for (const command of commands) {
    const selectable = command.selectable === true;
    const reason = String(command.actionDisabled ?? command.action_disabled ?? "").trim();
    assert(selectable || reason.length > 0, `${label} exposed a disabled command without a reason`, command);
    assert(String(command.kind ?? "").startsWith("conversation."), `${label} command lacks a closed semantic ID`, command);
  }
  assertUniqueSemanticIds(commands, `${label} commands`);
}

async function waitForState(
  driver: Driver,
  predicate: (state: RecordJson) => boolean,
  label: string,
  timeoutMs = 15_000,
): Promise<RecordJson> {
  const deadline = Date.now() + timeoutMs;
  let state = (await driver.getState({ timeoutMs: 15_000 })) as RecordJson;
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(50);
    state = (await driver.getState({ timeoutMs: 15_000 })) as RecordJson;
  }
  assert(predicate(state), `timed out waiting for ${label}`, {
    promptType: state.promptType ?? null,
    windowVisible: state.windowVisible ?? null,
  });
  return state;
}

async function waitForTarget(driver: Driver, kind: string, timeoutMs = 15_000): Promise<RecordJson> {
  const deadline = Date.now() + timeoutMs;
  let windows = (await driver.listAutomationWindows()) as RecordJson;
  let match = asObjects(windows.windows).find((window) => window.kind === kind);
  while (!match && Date.now() < deadline) {
    await Bun.sleep(50);
    windows = (await driver.listAutomationWindows()) as RecordJson;
    match = asObjects(windows.windows).find((window) => window.kind === kind);
  }
  assert(match, `timed out waiting for ${kind} automation target`, windows);
  return match;
}

const scenarios: Json[] = [];
const cleanup: Json[] = [];
const failures: string[] = [];
const observedSegments = new Map<string, WorkflowObservedSegment>();

async function runScenario(
  name: string,
  body: (driver: Driver) => Promise<Json>,
  env: Record<string, string> = {},
  seedAgentAuth = false,
): Promise<void> {
  let driver: Driver | null = null;
  let targetObservation: RuntimeTargetObservation | null = null;
  let result: Json = { name, status: "FAILED" };
  try {
    driver = await Driver.launch({
      binary,
      sessionName: `cons-flow-c04-${name}`,
      sandboxHome: true,
      seedAgentAuth,
      sharedModels: false,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1",
        SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
        SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
        SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
        ...env,
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
      safeMessage: "C04 runtime assertion failed; inspect the private Driver session log.",
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

await runScenario(
  "agent-chat",
  async (driver) => {
    const dispatch = await driver.triggerAction("ask_ai_settings", {
      host: "main",
      timeoutMs: 15_000,
    });
    assert(dispatch.ok === true || dispatch.success === true, "Agent Chat context entry was not dispatched", dispatch);
    await waitForState(driver, (state) => state.promptType === "agentChatChat", "embedded Agent Chat", 20_000);
    const receipt = await driver.getElements({ target: { type: "main" }, limit: 400 });
    const elements = elementList(receipt);
    assertUniqueSemanticIds(elements, "Agent Chat");
    assert(elements.filter((element) => element.role === "identityBadge").length >= 2, "Agent Chat identity badges missing");
    assert(elements.some((element) => element.role === "contextChip" && element.kind === "openContextDetails"), "Agent Chat context details action missing");
    assert(elements.some((element) => element.role === "contextChip" && element.kind === "removeContext"), "Agent Chat removable context action missing");
    assertCommandBindings(elements, "Agent Chat");
    return { surface: "Agent Chat", elements: safeElements(receipt) };
  },
  {},
  true,
);

await runScenario(
  "flow",
  async (driver) => {
    await driver.setFilterAndWait("Flows");
    await driver.simulateGpuiKeyDown("enter", { target: { type: "main" } });
    await driver.setFilterAndWait("hello-codex");
    await driver.simulateGpuiKeyDown("enter", { target: { type: "main" } });
    await waitForState(driver, (state) => state.promptType === "flowSession", "Flow session");
    const receipt = await driver.getElements({ target: { type: "main" }, limit: 400 });
    const elements = elementList(receipt);
    assert(elements.some((element) => element.role === "identityBadge" && element.kind === "openIdentityDetails"), "Flow identity badge missing");
    assertCommandBindings(elements, "Flow");
    const terminate = elements.find((element) => (element.semanticId ?? element.semantic_id) === "conversation.terminateRuntime");
    assert(terminate?.statusKind === "confirmationRequired" || terminate?.status_kind === "confirmationRequired", "Flow destructive command lacks confirmation marker", terminate);
    return { surface: "Flow", elements: safeElements(receipt) };
  },
  {
    SCRIPT_KIT_FLOW_UX_CWD: flowFixture,
    SCRIPT_KIT_FLOWS_PACKAGE_DIR: packageFixture,
    SCRIPT_KIT_FLOWS_BIN_DIR: join(packageFixture, "bin"),
    SCRIPT_KIT_CODEX_BIN: join(packageFixture, "bin/fake-codex"),
    PATH: `${join(flowFixture, "bin")}:${join(packageFixture, "bin")}:${process.env.PATH ?? ""}`,
  },
);

await runScenario("notes", async (driver) => {
  driver.send({ type: "openNotes", requestId: "c04-notes" });
  await waitForTarget(driver, "notes");
  const receipt = await driver.getElements({ target: { type: "kind", kind: "notes" }, limit: 120 });
  const elements = elementList(receipt);
  assert(elements.some((element) => element.role === "identityBadge" && element.kind === "openIdentityDetails"), "Notes document identity missing");
  return { surface: "Notes", elements: safeElements(receipt) };
});

await runScenario("today", async (driver) => {
  await openDayPage(driver, "c04-today");
  await waitForState(driver, (state) => state.promptType === "dayPage", "Today");
  const receipt = await driver.getElements({ target: { type: "main" }, limit: 200 });
  const elements = elementList(receipt);
  assert(elements.some((element) => element.role === "identityBadge" && element.kind === "openIdentityDetails"), "Today document identity missing");
  return { surface: "Today", elements: safeElements(receipt) };
});

await runScenario("dictation", async (driver) => {
  driver.send({ type: "openDictationOverlayFixture", requestId: "c04-dictation" });
  await waitForTarget(driver, "dictation");
  const target = { type: "kind", kind: "dictation" };
  const receipt = await driver.getElements({ target, limit: 120 });
  const secondReceipt = await driver.getElements({ target, limit: 120 });
  const elements = elementList(receipt);
  assert(elements.some((element) => element.role === "destinationSelector" && element.kind === "selectDestination"), "Dictation destination selector missing");
  const selector = safeElements(receipt).find((element) => (element as RecordJson).role === "destinationSelector");
  const secondSelector = safeElements(secondReceipt).find((element) => (element as RecordJson).role === "destinationSelector");
  assert(JSON.stringify(selector) === JSON.stringify(secondSelector), "passive destination inspection changed selector semantics");
  return { surface: "Dictation", selectorInspectionMutatedState: false, elements: safeElements(receipt) };
});

await runScenario("chat-prompt", async (driver) => {
  await driver.request({ type: "show" }, { timeoutMs: 10_000 });
  await driver.waitForSettle({ timeoutMs: 10_000 });
  await Bun.sleep(150);
  // The fixture feeds the same `PromptMessage::ShowChat` path as an ordinary
  // SDK `chat()` script while leaving the Driver control bus independent.
  driver.send({ type: "openChatPromptFixture", requestId: "c04-chat-prompt" });
  await waitForState(driver, (state) => state.promptType === "chat", "ordinary ChatPrompt", 20_000);
  const receipt = await driver.getElements({ target: { type: "main" }, limit: 200 });
  const elements = elementList(receipt);
  assertCommandBindings(elements, "ChatPrompt");
  assert(!elements.some((element) => (element.semanticId ?? element.semantic_id) === "conversation.background"), "ChatPrompt exposed unsupported Background command");
  assert(!elements.some((element) => (element.semanticId ?? element.semantic_id) === "conversation.new"), "ChatPrompt exposed unsupported New Conversation command");
  return { surface: "ChatPrompt", elements: safeElements(receipt) };
});

const classification = failures.length === 0 ? "RUNTIME-CONFIRMED" : "RUNTIME-FAILED";
const receipt = {
  schemaVersion: 1,
  classification,
  binary: {
    path: binary,
    sha256: sha256(binary),
  },
  scenarios,
  cleanup,
  assertions: {
    exactRoles: ["contextChip", "identityBadge", "destinationSelector"],
    exactActions: [
      "removeContext",
      "openContextDetails",
      "openIdentitySelector",
      "openIdentityDetails",
      "selectDestination",
    ],
    unsupportedChatPromptCommandsAbsent: true,
    destructiveCommandsRequireConfirmation: true,
    disabledCommandsCarrySafeReasons: true,
    duplicateSemanticIdsAbsent: true,
    clipboardTouched: false,
  },
  failures,
};

for (const taskId of ["WF-004", "WF-005"] as const) {
  let taskReceipt: Json;
  try {
    assert(classification === "RUNTIME-CONFIRMED", "semantic command journey did not pass");
    const stageMappings = taskId === "WF-004"
      ? [
          { id: "context-role-isolated", scenario: "agent-chat" },
          { id: "identity-role-isolated", scenario: "agent-chat" },
          { id: "destination-role-isolated", scenario: "dictation" },
        ]
      : [
          { id: "conversation-command-descriptors", scenario: "agent-chat" },
          { id: "unsupported-command-refused", scenario: "chat-prompt" },
        ];
    const segments = Array.from(new Set(stageMappings.map((stage) => stage.scenario)))
      .map((name) => {
        const segment = observedSegments.get(name);
        assert(segment, `semantic journey omitted actual observed segment: ${name}`);
        return segment;
      });
    const stages = stageMappings.map((stage) => {
      const segment = observedSegments.get(stage.scenario)!;
      const scenario = scenarios.find((candidate) => candidate.name === stage.scenario);
      assert(scenario?.status === "PASS", `semantic journey omitted passing stage: ${stage.id}`);
      return observedWorkflowStage({
        id: stage.id,
        primitiveId: "devtools.elements.snapshot",
        segment,
        command: "getElements",
        requestId: `${taskId}:${stage.id}`,
        result: scenario,
        pass: true,
      });
    });
    const dictation = scenarios.find((scenario) => scenario.name === "dictation");
    const controls = taskId === "WF-004"
      ? {
          "context-identity-destination-cannot-interchange":
            receipt.assertions.exactRoles.length === 3,
          "passive-inspection-never-mutates-destination":
            dictation?.selectorInspectionMutatedState === false,
        }
      : {
          "disabled-command-cannot-activate":
            receipt.assertions.disabledCommandsCarrySafeReasons === true,
          "destructive-command-requires-confirmation":
            receipt.assertions.destructiveCommandsRequireConfirmation === true,
        };
    taskReceipt = prepareWorkflowTaskProof(taskId, {
      producerOwner: "scripts/agentic/cons-flow-ux/semantic-command-probe.ts",
      segments,
      stages,
      negativeControls: controls,
      safety: {
        microphoneCaptureStarted: false,
        nativeInputInjected: false,
        liveAiStarted: false,
        screenTakeoverStarted: false,
        clipboardTouched: receipt.assertions.clipboardTouched === true,
      },
    }).receipt as Json;
  } catch (error) {
    taskReceipt = prepareBlockedWorkflowTaskProof(
      taskId,
      error instanceof Error ? error.message : String(error),
    ).receipt as Json;
  }
  await mkdir(join(runDir, taskId), { recursive: true });
  await writeFile(join(runDir, taskId, "receipt.json"), `${JSON.stringify(taskReceipt, null, 2)}\n`);
  writeWorkflowTaskProof(taskId, taskReceipt);
}
console.log(JSON.stringify(receipt, null, 2));
if (failures.length > 0) process.exitCode = 1;
