#!/usr/bin/env bun
import { runtimeArtifactFromEnvironment } from "../../devtools/lib/runtime-task-proof.ts";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observeWorkflowTaskTarget,
} from "../../devtools/lib/workflow-task-proof.ts";
import type { RuntimeTargetObservation } from "../../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.notes-agent-chat-return");

const PROJECT_ROOT = resolve(import.meta.dir, "../../..");
const BINARY = runtimeArtifactFromEnvironment().executablePath
const OUT_PATH = join(PROJECT_ROOT, ".test-output", "cons-flow-c09", "notes-agent-chat-return.json");
const NOTES_TARGET = { type: "kind", kind: "notes", index: 0 };
const runId = `notes-ai-return-${Date.now().toString(36)}`;
const outsideRange = `OUTSIDE_RANGE_${runId}`;
const liveLine = `live unsaved note ${runId}`;
const noteText = `${outsideRange}\n${liveLine}`;

type Obj = Record<string, any>;
const receipt: Obj = {
  schemaVersion: 1,
  tool: "notes-agent-chat-return-probe",
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

async function poll<T>(
  label: string,
  read: () => Promise<T>,
  accepts: (value: T) => boolean,
  timeoutMs = 12_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let last = await read();
  while (Date.now() < deadline) {
    if (accepts(last)) return last;
    await Bun.sleep(100);
    last = await read();
  }
  throw new Error(`${label} timed out: ${JSON.stringify(last)}`);
}

async function notesState(driver: Driver): Promise<Obj> {
  const result = asObj(
    await driver.request(
      { type: "getState", target: NOTES_TARGET },
      { expect: "stateResult", timeoutMs: 8000 },
    ),
  );
  return asObj(result.notes ?? result);
}

async function mainState(driver: Driver): Promise<Obj> {
  return asObj(await driver.getState({ timeoutMs: 8000 }));
}

async function agentChatState(driver: Driver): Promise<Obj> {
  return asObj(
    await driver.request(
      { type: "getAgentChatState" },
      { expect: "agent_chatStateResult", timeoutMs: 8000 },
    ),
  );
}

async function gpuiKey(driver: Driver, target: Json, key: string, modifiers: string[] = []) {
  return asObj(
    await driver.simulateGpuiKeyDown(key, {
      target,
      modifiers,
      timeoutMs: 10_000,
    }),
  );
}

const driver = await Driver.launch({ immutableArtifact: runtimeArtifactFromEnvironment().reference, binary: BINARY,
sessionName: "cons-flow-c09-notes-ai-return",
sandboxHome: true,
seedAgentAuth: true,
defaultTimeoutMs: 12_000,
env: {
  SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
  SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
  SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
  SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
}, });

try {
  driver.send({ type: "openNotes", requestId: `${runId}-open-notes` });
  await poll(
    "Notes target",
    async () => asObj(await driver.listAutomationWindows({ timeoutMs: 5000 })),
    (state) =>
      Array.isArray(state.windows) &&
      state.windows.some((window: Obj) => window.kind === "notes"),
  );
  const set = asObj(
    await driver.request(
      {
        type: "batch",
        requestId: `${runId}-set-note`,
        target: NOTES_TARGET,
        commands: [{ type: "setInput", text: noteText }],
        options: { stopOnError: true, timeout: 5000 },
      },
      { expect: "batchResult", timeoutMs: 7000 },
    ),
  );
  check("live_note_seeded", set.success === true, { success: set.success ?? null });

  await gpuiKey(driver, NOTES_TARGET, "left", ["shift"]);
  const before = await poll(
    "Notes selection",
    () => notesState(driver),
    (state) => Number(asObj(state.editor).selectionLength ?? 0) === 1,
  );
  const beforeDraft = asObj(before.draftSnapshot);
  const beforeView = asObj(before.view);
  check("notes_selection_and_live_snapshot_captured", Number(asObj(before.editor).selectionLength) === 1, {
    activeNoteId: before.activeNoteId ?? null,
    bodyLength: asObj(beforeDraft.draft).bodyLength ?? null,
    bodyFingerprint: asObj(beforeDraft.draft).bodyFingerprint ?? null,
    selectionRange: asObj(before.editor).selectionRange ?? null,
  });

  driver.simulateKey("enter", ["cmd"]);
  const afterCommand = await poll(
    "Notes AI handoff receipt",
    () => notesState(driver),
    (state) => asObj(state.lastAiHandoff).active === true,
    10_000,
  );
  check("notes_command_enter_dispatched", asObj(afterCommand.lastAiHandoff).active === true, {
    lastAiHandoff: afterCommand.lastAiHandoff ?? null,
  });

  const opened = await poll(
    "main Agent Chat",
    () => mainState(driver),
    (state) => String(state.promptType ?? "").toLowerCase().includes("agent"),
    20_000,
  );
  const chat = await agentChatState(driver).catch((error) => ({ error: String(error) }));
  check("main_agent_chat_opened_composer_only", Number(chat.messageCount ?? 0) === 0, {
    promptType: opened.promptType ?? null,
    messageCount: chat.messageCount ?? null,
    inputLength: String(chat.inputText ?? "").length,
    contextChipCount: chat.contextChipCount ?? null,
  });

  const during = await notesState(driver);
  const handoff = asObj(during.lastAiHandoff);
  check(
    "whole_live_note_scope_receipted",
    handoff.status === "staged" &&
      handoff.scope === "wholeNote" &&
      handoff.contentLength === noteText.length &&
      handoff.stagingOutcome === "composerOnly" &&
      handoff.returnRoute === "notes",
    { handoff },
  );
  check("notes_window_stayed_open", during.activeNoteId != null, {
    notesInstanceId: handoff.notesInstanceId ?? null,
    activeNoteId: during.activeNoteId ?? null,
  });

  const escape = await gpuiKey(driver, { type: "main" }, "escape");
  check("agent_chat_escape_dispatched", escape.success !== false, { escape });
  const restored = await poll(
    "Notes focus restore",
    () => notesState(driver),
    (state) => asObj(state.view).focusSurface === "Editor",
    10_000,
  );
  const afterDraft = asObj(asObj(restored.draftSnapshot).draft);
  check(
    "notes_host_state_preserved_on_return",
    handoff.notesInstanceId === asObj(restored.lastAiHandoff).notesInstanceId &&
      afterDraft.bodyLength === asObj(beforeDraft.draft).bodyLength &&
      afterDraft.bodyFingerprint === asObj(beforeDraft.draft).bodyFingerprint &&
      JSON.stringify(asObj(restored.editor).selectionRange) ===
        JSON.stringify(asObj(before.editor).selectionRange) &&
      asObj(restored.view).viewMode === beforeView.viewMode,
    {
      before: {
        bodyLength: asObj(beforeDraft.draft).bodyLength ?? null,
        bodyFingerprint: asObj(beforeDraft.draft).bodyFingerprint ?? null,
        selectionRange: asObj(before.editor).selectionRange ?? null,
        viewMode: beforeView.viewMode ?? null,
      },
      after: {
        bodyLength: afterDraft.bodyLength ?? null,
        bodyFingerprint: afterDraft.bodyFingerprint ?? null,
        selectionRange: asObj(restored.editor).selectionRange ?? null,
        viewMode: asObj(restored.view).viewMode ?? null,
        focusSurface: asObj(restored.view).focusSurface ?? null,
      },
    },
  );
  check("receipts_do_not_expose_note_text", !JSON.stringify(handoff).includes(outsideRange), {
    redacted: handoff.redacted ?? null,
  });

  const returnLogObserved = await poll(
    "Notes return log",
    async () => Bun.file(driver.logPath).text(),
    (logs) => logs.includes("event=notes_ai_return_restored"),
    3000,
  );
  check(
    "exact_notes_return_log_emitted",
    returnLogObserved.includes("event=notes_ai_return_restored") &&
      !returnLogObserved.includes("event=notes_ai_return_ignored"),
    { logPath: driver.logPath },
  );
  targetObservation = await observeWorkflowTaskTarget(driver, BINARY, NOTES_TARGET);
} catch (error) {
  check("probe_exception", false, {
    message: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? error.stack : null,
  });
} finally {
  await driver.close().catch((error) => {
    check("driver_close_completed", false, { message: String(error) });
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
  receipt.sessionDir = driver.sessionDir;
  receipt.logPath = driver.logPath;
  receipt.pass = receipt.failures.length === 0;
  if (targetObservation !== null && receipt.pass) {
    receipt.workflowObservedSegment = observedWorkflowSegment(
      "notes-agent-chat-return",
      targetObservation,
      cleanup,
    );
  }
  mkdirSync(join(PROJECT_ROOT, ".test-output", "cons-flow-c09"), { recursive: true });
  await Bun.write(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
}

console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exitCode = 1;
