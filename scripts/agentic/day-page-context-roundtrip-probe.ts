#!/usr/bin/env bun
/**
 * Runtime proof for Day Page @context parity:
 * - typing @con in the Day Page editor swaps to the main menu context list
 * - accepting the main-menu row returns to the same Day Page line with @here
 * - Cmd+Enter does not open the deprecated Day prompt-builder/Agent handoff
 */
import { Driver, type Json } from "../devtools/driver";
import { openDayPage } from "./day-page-open-helper";
import { assertNoninteractiveVisualProbe } from "../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observeWorkflowTaskTarget,
} from "../devtools/lib/workflow-task-proof.ts";
import type { RuntimeTargetObservation } from "../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("day-page-context-roundtrip.workflow-child");

const BINARY =
  process.env.PROBE_BINARY ??
  "target-agent/artifacts/day-page-context/script-kit-gpui";

const receipts: Record<string, Json> = {};
const failures: string[] = [];
let targetObservation: RuntimeTargetObservation | null = null;
const runId = `${Date.now()}-${Math.random().toString(36).slice(2)}`;

function check(name: string, ok: boolean, detail: Json = {}) {
  receipts[name] = { ok, ...detail };
  if (!ok) failures.push(name);
}

function walkElements(node: unknown, out: Json[] = []): Json[] {
  if (!node || typeof node !== "object") return out;
  if (Array.isArray(node)) {
    for (const item of node) walkElements(item, out);
    return out;
  }
  const json = node as Json;
  if (typeof json.semanticId === "string" || typeof json.id === "string") {
    out.push(json);
  }
  for (const value of Object.values(json)) walkElements(value, out);
  return out;
}

function gpuiKey(
  driver: Driver,
  key: string,
  modifiers: string[] = [],
  text?: string,
): Promise<Json> {
  const event: Json = { type: "keyDown", key, modifiers };
  if (text !== undefined) event.text = text;
  return driver.request(
    {
      type: "simulateGpuiEvent",
      target: { type: "kind", kind: "main" },
      event,
    },
    { expect: "simulateGpuiEventResult", timeoutMs: 5000 },
  );
}

async function typeText(driver: Driver, text: string) {
  for (const ch of text) {
    const key = ch === " " ? "space" : ch;
    const result = await gpuiKey(driver, key, [], ch);
    if (result.success !== true) {
      check(`typed_${ch}_dispatch`, false, { result });
      return false;
    }
    await Bun.sleep(35);
  }
  return true;
}

async function editorText(driver: Driver): Promise<string | null> {
  const elements = (await driver.getElements(
    { target: { type: "main" }, limit: 160 },
    { timeoutMs: 5000 },
  )) as Json;
  const editor = walkElements(elements).find(
    (el) =>
      el.semanticId === "input:day-page-editor" || el.id === "day-page-editor",
  );
  return (editor?.value as string | undefined) ?? null;
}

function firstMatchingLog(log: string, needle: string): string | null {
  return log.split("\n").find((line) => line.includes(needle)) ?? null;
}

function contextReceiptLines(log: string, status?: string): string[] {
  return log
    .split("\n")
    .filter(
      (line) =>
        line.includes("day_page_context_round_trip_receipt") &&
        line.includes('"receiptKind":"dayPage.contextRoundTrip"') &&
        line.includes('"redacted":true') &&
        (status === undefined || line.includes(`"status":"${status}"`)),
    );
}

async function appLogText(driver: Driver): Promise<string> {
  return await Bun.file(`${driver.sessionDir}/app.log`).text();
}

const deprecatedMarkdownHandoffEvent = [
  "day_page",
  "markdown_reference",
  "handoff",
].join("_");
const deprecatedLineHandoffSource = ["day_page", "line", "handoff"].join("_");

function localOverlayIds(elements: Json): string[] {
  return walkElements(elements)
    .map((el) => String(el.semanticId ?? el.id ?? ""))
    .filter((id) => {
      const lower = id.toLowerCase();
      return (
        lower.includes("day-page-spine") ||
        lower.includes("day-spine") ||
        lower.includes("ready-to-send") ||
        lower.includes("prompt-builder") ||
        lower === "prompt-compiler"
      );
    });
}

const driver = await Driver.launch({
  binary: BINARY,
  sandboxHome: true,
  sessionName: "day-page-context-roundtrip",
  defaultTimeoutMs: 8000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

try {
  const initial = await openDayPage(driver, runId);
  check("opened_day_page", initial.promptType === "dayPage", {
    promptType: initial.promptType,
  });

  const prefix = `Day Page context roundtrip ${runId} `;
  const seed = (await driver.batch(
    [
      { type: "setInput", text: prefix },
      {
        type: "waitFor",
        condition: {
          type: "stateMatch",
          state: { promptType: "dayPage", inputValue: prefix },
        },
      },
    ],
    { timeoutMs: 5000 },
  )) as Json;
  check("seeded_day_page_prefix", seed.success === true, { batch: seed });

  const typed = await typeText(driver, "@con");
  check("typed_context_alias_through_gpui", typed, {});

  const menuWait = (await driver.waitFor(
    {
      type: "stateMatch",
      state: { promptType: "none", inputValue: "@con" },
    },
    { timeoutMs: 5000 },
  )) as Json;
  const menuState = (await driver.getState({ timeoutMs: 5000 })) as Json;
  check("typing_swapped_to_main_menu", menuState.promptType === "none", {
    promptType: menuState.promptType,
    inputValue: menuState.inputValue ?? null,
    wait: menuWait,
  });
  check("main_menu_filter_is_context_alias", menuState.inputValue === "@con", {
    inputValue: menuState.inputValue ?? null,
  });

  const menuElements = (await driver.getElements(
    { target: { type: "main" }, limit: 220 },
    { timeoutMs: 5000 },
  )) as Json;
  const menuFlat = walkElements(menuElements);
  const contextRow = menuFlat.find(
    (el) =>
      el.semanticId === "spine:@:builtin:here" ||
      el.semanticId === "choice:0:what-i-m-looking-at" ||
      String(el.semanticId ?? "").includes("what-i-m-looking-at") ||
      (typeof el.label === "string" && el.label.includes("What I")),
  );
  const localDaySpineIds = menuFlat
    .map((el) => String(el.semanticId ?? el.id ?? ""))
    .filter((id) => id.includes("day-page-spine"));
  check("main_menu_context_row_visible", Boolean(contextRow), {
    selectedSemanticId: menuElements.selectedSemanticId ?? null,
    contextRow: contextRow ?? null,
    sampleIds: menuFlat
      .map((el) => el.semanticId ?? el.id)
      .filter(Boolean)
      .slice(0, 40),
  });
  check(
    "day_page_local_spine_not_rendered_in_main_menu",
    localDaySpineIds.length === 0,
    {
      localDaySpineIds,
    },
  );

  driver.simulateKey("enter");
  await Bun.sleep(900);
  const afterAccept = (await driver.getState({ timeoutMs: 5000 })) as Json;
  const completedLine = `${prefix}[What I’m Looking At](kit://context?profile=minimal) `;
  check("accept_returns_to_day_page", afterAccept.promptType === "dayPage", {
    promptType: afterAccept.promptType,
    inputValue: afterAccept.inputValue ?? null,
  });
  const afterAcceptText = await editorText(driver);
  check(
    "accepted_context_spliced_into_original_line",
    afterAcceptText === completedLine,
    {
      afterAcceptText,
      expected: completedLine,
    },
  );
  const afterAcceptDayPageState = (afterAccept.dayPage ?? {}) as Json;
  const ledger = (afterAcceptDayPageState.contextReferenceLedger ?? {}) as Json;
  const lastRoundTripReceipt =
    (afterAcceptDayPageState.lastContextRoundTripReceipt ?? {}) as Json;
  check(
    "context_reference_ledger_rebuilt_from_markdown",
    ledger.markdownReferenceCount === 1 && ledger.aliasCount === 1,
    {
      ledger,
    },
  );
  check(
    "automation_exposes_completed_roundtrip_receipt",
    lastRoundTripReceipt.status === "completed" &&
      lastRoundTripReceipt.redacted === true,
    {
      lastRoundTripReceipt,
    },
  );

  const afterAcceptLog = await appLogText(driver);
  const pendingReceiptsAfterAccept = contextReceiptLines(
    afterAcceptLog,
    "pending",
  );
  const completedReceiptsAfterAccept = contextReceiptLines(
    afterAcceptLog,
    "completed",
  );
  const completedReceiptText = completedReceiptsAfterAccept.join("\n");
  check(
    "context_roundtrip_pending_receipt_redacted",
    pendingReceiptsAfterAccept.length >= 1,
    {
      pendingReceipts: pendingReceiptsAfterAccept.slice(-2),
    },
  );
  check(
    "context_roundtrip_completed_receipt_redacted",
    completedReceiptsAfterAccept.length >= 1,
    {
      completedReceipts: completedReceiptsAfterAccept.slice(-2),
    },
  );
  check(
    "context_roundtrip_completed_receipt_hashes_visible_reference",
    completedReceiptText.includes('"visibleReferenceHash":"sha256:'),
    {
      completedReceipts: completedReceiptsAfterAccept.slice(-2),
    },
  );
  check(
    "context_roundtrip_receipts_do_not_log_raw_context",
    !completedReceiptText.includes("What I’m Looking At") &&
      !completedReceiptText.includes("kit://context?profile=minimal"),
    {
      completedReceipts: completedReceiptsAfterAccept.slice(-2),
    },
  );

  const afterAcceptElements = (await driver.getElements(
    { target: { type: "main" }, limit: 220 },
    { timeoutMs: 5000 },
  )) as Json;
  check(
    "day_page_has_no_local_prompt_builder_after_context_accept",
    localOverlayIds(afterAcceptElements).length === 0,
    {
      localOverlayIds: localOverlayIds(afterAcceptElements),
    },
  );

  await gpuiKey(driver, "left");
  await gpuiKey(driver, "backspace");
  await Bun.sleep(500);
  const afterAtomicDelete = (await driver.getState({
    timeoutMs: 5000,
  })) as Json;
  const afterAtomicDeleteText = await editorText(driver);
  const atomicDeleteLedger = (((afterAtomicDelete.dayPage ?? {}) as Json)
    .contextReferenceLedger ?? {}) as Json;
  check(
    "markdown_context_reference_deletes_atomically",
    typeof afterAtomicDeleteText === "string" &&
      !afterAtomicDeleteText.includes("[What I’m Looking At]") &&
      !afterAtomicDeleteText.includes("kit://context?profile=minimal"),
    {
      afterAtomicDeleteText,
    },
  );
  check(
    "context_reference_ledger_clears_after_atomic_delete",
    atomicDeleteLedger.markdownReferenceCount === 0 &&
      atomicDeleteLedger.aliasCount === 0,
    {
      atomicDeleteLedger,
    },
  );

  const restoreAcceptedLine = (await driver.batch(
    [
      { type: "setInput", text: completedLine },
      {
        type: "waitFor",
        condition: {
          type: "stateMatch",
          state: { promptType: "dayPage", inputValue: completedLine },
        },
      },
    ],
    { timeoutMs: 5000 },
  )) as Json;
  check(
    "restored_accepted_line_for_cmd_enter",
    restoreAcceptedLine.success === true,
    {
      batch: restoreAcceptedLine,
    },
  );

  driver.simulateKey("enter", ["cmd"]);
  await Bun.sleep(900);
  const afterSubmit = (await driver.getState({ timeoutMs: 5000 })) as Json;
  const afterSubmitText = await editorText(driver);
  const afterSubmitElements = (await driver.getElements(
    { target: { type: "main" }, limit: 220 },
    { timeoutMs: 5000 },
  )) as Json;
  const appLog = await Bun.file(`${driver.sessionDir}/app.log`).text();
  const deprecatedStartedLine = firstMatchingLog(
    appLog,
    `event=${deprecatedMarkdownHandoffEvent}_started`,
  );
  const deprecatedSubmitLine = firstMatchingLog(
    appLog,
    deprecatedMarkdownHandoffEvent,
  );
  const deprecatedLineHandoff = firstMatchingLog(
    appLog,
    deprecatedLineHandoffSource,
  );
  const promptCompilerIds = localOverlayIds(afterSubmitElements);
  const footerButtons = Array.isArray(afterSubmit.activeFooter?.buttons)
    ? (afterSubmit.activeFooter.buttons as Json[])
    : [];

  check(
    "cmd_enter_opens_agent_chat_from_day_page",
    afterSubmit.promptType === "agentChatChat",
    {
      promptType: afterSubmit.promptType,
    },
  );
  check("cmd_enter_leaves_day_page_surface", afterSubmitText === null, {
    afterSubmitText,
  });
  check(
    "cmd_enter_does_not_render_prompt_builder",
    promptCompilerIds.length === 0,
    {
      localOverlayIds: promptCompilerIds,
    },
  );
  check(
    "cmd_enter_does_not_emit_deprecated_day_handoff_logs",
    !deprecatedStartedLine && !deprecatedSubmitLine && !deprecatedLineHandoff,
    {
      deprecatedStartedLine,
      deprecatedSubmitLine,
      deprecatedLineHandoff,
    },
  );
  check(
    "day_footer_has_no_agent_button",
    footerButtons.every((button) => button.action !== "ai"),
    {
      footerButtons,
    },
  );

  const reopenedAfterAgentChat = await openDayPage(driver, `${runId}-cancel`);
  check(
    "reopened_day_page_after_agent_chat",
    reopenedAfterAgentChat.promptType === "dayPage",
    {
      promptType: reopenedAfterAgentChat.promptType,
    },
  );

  const cancelPrefix = `Day Page context cancel ${runId} `;
  const cancelSeed = (await driver.batch(
    [
      { type: "setInput", text: cancelPrefix },
      {
        type: "waitFor",
        condition: {
          type: "stateMatch",
          state: { promptType: "dayPage", inputValue: cancelPrefix },
        },
      },
    ],
    { timeoutMs: 5000 },
  )) as Json;
  check("seeded_cancel_prefix", cancelSeed.success === true, {
    batch: cancelSeed,
  });
  await typeText(driver, "@file");
  const cancelMenuState = (await driver.waitFor(
    {
      type: "stateMatch",
      state: { promptType: "none", inputValue: "@file" },
    },
    { timeoutMs: 5000 },
  )) as Json;
  check("cancel_path_swapped_to_main_menu", cancelMenuState.success === true, {
    cancelMenuState,
  });
  driver.simulateKey("escape");
  await Bun.sleep(500);
  const afterCancel = (await driver.getState({ timeoutMs: 5000 })) as Json;
  const afterCancelText = await editorText(driver);
  const afterCancelLog = await appLogText(driver);
  const cancelledReceipts = contextReceiptLines(afterCancelLog, "cancelled");
  check(
    "escape_cancels_back_to_day_page",
    afterCancel.promptType === "dayPage",
    {
      promptType: afterCancel.promptType,
      afterCancelText,
    },
  );
  check(
    "context_roundtrip_cancelled_receipt_redacted",
    cancelledReceipts.length >= 1,
    {
      cancelledReceipts: cancelledReceipts.slice(-2),
    },
  );

  targetObservation = await observeWorkflowTaskTarget(driver, BINARY, { type: "main" });
} finally {
  await driver.close();
  const cleanup = {
    ...driver.finalization,
    ownedProcessCount: driver.finalization.processExited ? 0 : 1,
    closeError: null,
    clipboardTouched: false,
  };
  const pass = failures.length === 0 && cleanup.processExited && cleanup.streamsDrained && cleanup.logWriterClosed;
  const workflowObservedSegment = targetObservation !== null && pass
    ? observedWorkflowSegment("today-mention-parity", targetObservation, cleanup)
    : null;
  console.log(JSON.stringify({
    pass,
    failures,
    sessionDir: driver.sessionDir,
    screenshotProof: "not-used-semantic-devtools-only",
    receipts,
    cleanup,
    workflowObservedSegment,
  }, null, 2));
  if (!pass) process.exitCode = 1;
}
