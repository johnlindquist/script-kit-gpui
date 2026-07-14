#!/usr/bin/env bun
/**
 * scripts/agentic/input-ambiguity-probe.ts
 *
 * Runtime proof for the 2026-06-09 input-ambiguity decisions (see
 * .notes/20260609-input-ambiguity-decisions.md):
 *
 *   A1  exact alias match pins the aliased command at index 0; typing an
 *       alias (even with a trailing space) never auto-executes.
 *   A2  Tab opens the cwd picker ONLY when the main input is empty.
 *   A7  ghost predictions are disabled; backquote types a literal char.
 *   A8  Up-arrow history recall fires when input is empty at top of list,
 *       continues deeper into history once navigating, and never fires
 *       mid-query. The first Down exits history without changing the query
 *       and moves ordinary list selection; Up then moves selection back.
 *   A5  multi-line Cmd+V on the script list routes to Agent Chat.
 *
 * Key events that live in GPUI interceptors (Tab/Up/Cmd+V) are driven with
 * `simulateGpuiEvent`, which dispatches through window.dispatch_keystroke —
 * the legacy `simulateKey` stdin surface bypasses interceptors and would
 * silently test the wrong path.
 *
 * The launch is two-phase: first-run scaffolding rebuilds ~/.scriptkit in the
 * sandbox, wiping pre-seeded files, so we launch once to scaffold, close,
 * seed aliases.json + input_history.json, then relaunch for the real probe.
 *
 * Usage: bun scripts/agentic/input-ambiguity-probe.ts
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(PROJECT_ROOT, "target-agent/artifacts/input-ambiguity/script-kit-gpui");
const OUT_DIR = join(
  PROJECT_ROOT,
  `.test-output/input-ambiguity/${process.pid}`,
);
const SESSION_DIR = `/tmp/sk-input-ambiguity-${process.pid}`;

// ScriptList reports promptType "none" in stateResult.
const SCRIPT_LIST = "none";

interface StepResult {
  step: string;
  pass: boolean;
  details: Json;
}
const results: StepResult[] = [];
function record(step: string, pass: boolean, details: Json = {}) {
  results.push({ step, pass, details });
}

async function shot(driver: Driver, name: string): Promise<string | null> {
  const savePath = join(OUT_DIR, name);
  const res = await driver.captureScreenshot({
    savePath,
    target: { type: "kind", kind: "main" },
  });
  if (res.error) {
    record(`screenshot-${name}`, false, { error: res.error });
    return null;
  }
  return savePath;
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
    { type: "simulateGpuiEvent", target: { type: "kind", kind: "main" }, event },
    { expect: "simulateGpuiEventResult" },
  );
}

async function selectableRows(driver: Driver): Promise<Json[]> {
  const result = await driver.getElements({
    target: { type: "kind", kind: "main" },
    limit: 2_000,
    includeHeaders: true,
  });
  return (Array.isArray(result.elements) ? result.elements : [])
    .filter((element: Json) => element.role === "row" && element.selectable === true)
    .sort((a: Json, b: Json) => Number(a.index) - Number(b.index));
}

async function waitForSelectableRows(driver: Driver, minimum: number): Promise<Json[]> {
  const deadline = Date.now() + 4_000;
  let rows = await selectableRows(driver);
  while (rows.length < minimum && Date.now() < deadline) {
    await Bun.sleep(100);
    rows = await selectableRows(driver);
  }
  return rows;
}

async function waitForSelectedRowIndex(
  driver: Driver,
  expectedIndex: number | undefined,
): Promise<Json[]> {
  const deadline = Date.now() + 4_000;
  let rows = await selectableRows(driver);
  while (
    rows.find((row) => row.selected === true)?.index !== expectedIndex
    && Date.now() < deadline
  ) {
    await Bun.sleep(50);
    rows = await selectableRows(driver);
  }
  return rows;
}

function logMessages(response: Json): string[] {
  return (Array.isArray(response.entries) ? response.entries : [])
    .map((entry: Json) => String(entry.message ?? ""));
}

async function waitUntil(
  driver: Driver,
  predicate: (state: Json) => boolean,
  timeoutMs = 4000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let state = await driver.getState();
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(100);
    state = await driver.getState();
  }
  return state;
}

async function main() {
  mkdirSync(OUT_DIR, { recursive: true });

  // --- phase 0: scaffold the sandbox, then seed -----------------------------
  const scaffold = await Driver.launch({
    binary: BINARY,
    sessionName: "input-ambiguity-scaffold",
    sessionDir: SESSION_DIR,
    sandboxHome: true,
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 8000,
  });
  await scaffold.getState();
  await scaffold.close();

  const sk = join(SESSION_DIR, "home", ".scriptkit");
  const scriptsDir = join(sk, "plugins", "main", "scripts");
  mkdirSync(scriptsDir, { recursive: true });
  for (const suffix of ["Alpha", "Beta"]) {
    writeFileSync(
      join(scriptsDir, `launcher-history-${suffix.toLowerCase()}.ts`),
      `// Name: Launcher History ${suffix}\n// Description: deterministic launcher history fixture\n\nexport {};\n`,
    );
  }
  // Alias override: "zz" → Clipboard History builtin. "zz" fuzzy-matches
  // nothing, so the pin must use the synthetic-fallback path too.
  writeFileSync(
    join(sk, "aliases.json"),
    JSON.stringify({ "builtin/clipboard-history": "zz" }),
  );
  // Input history, most recent first.
  writeFileSync(
    join(sk, "input_history.json"),
    JSON.stringify({
      entries: ["launcher history", "launcher fixture"],
      selected_results: {},
    }),
  );

  // --- phase 1: the real probe ----------------------------------------------
  const driver = await Driver.launch({
    binary: BINARY,
    sessionName: "input-ambiguity",
    sessionDir: join(SESSION_DIR, "run"),
    sandboxHome: false,
    env: { HOME: join(SESSION_DIR, "home"), SK_PATH: sk },
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 8000,
  });

  // Save and later restore the user clipboard around the Cmd+V proof.
  const savedClipboard = await Bun.$`pbpaste`.text().catch(() => "");

  try {
    driver.send({ type: "show" });
    await driver.waitForState({ windowVisible: true });

    // === A1: alias pin ======================================================
    await driver.setFilterAndWait("zz");
    await Bun.sleep(200);
    const a1Elements = await driver.getElements();
    const a1State = await driver.getState();
    const flat = JSON.stringify(a1Elements);
    record("a1-alias-pins-clipboard-history-first", flat.includes("Clipboard History"), {
      selectedIndex: a1State.selectedIndex,
      promptType: a1State.promptType,
      visibleChoiceCount: a1State.visibleChoiceCount,
      treeMentionsClipboardHistory: flat.includes("Clipboard History"),
    });
    record(
      "a1-selected-index-0",
      a1State.selectedIndex === 0 && a1State.promptType === SCRIPT_LIST,
      { selectedIndex: a1State.selectedIndex, promptType: a1State.promptType },
    );
    const a1Shot = await shot(driver, "a1-alias-pin.png");

    // Trailing space must NOT auto-execute (view stays scriptList).
    await driver.setFilterAndWait("zz ");
    await Bun.sleep(200);
    const a1Space = await driver.getState();
    record("a1-trailing-space-does-not-execute", a1Space.promptType === SCRIPT_LIST, {
      promptType: a1Space.promptType,
      inputValue: a1Space.inputValue,
    });

    // === A2: Tab → cwd picker only when input empty =========================
    await driver.setFilterAndWait("");
    // With text typed, Tab must NOT open the picker.
    await driver.setFilterAndWait("clip");
    await gpuiKey(driver, "tab");
    await Bun.sleep(200);
    const a2Typed = await driver.getState();
    record(
      "a2-tab-with-text-stays-on-script-list",
      a2Typed.promptType === SCRIPT_LIST && a2Typed.inputValue === "clip",
      { promptType: a2Typed.promptType, inputValue: a2Typed.inputValue },
    );

    // With empty input, Tab opens the cwd picker (FileSearch surface, input "~/").
    await driver.setFilterAndWait("");
    await gpuiKey(driver, "tab");
    const a2Empty = await waitUntil(
      driver,
      (s) => s.promptType === "fileSearch" && s.inputValue === "~/",
    );
    record(
      "a2-tab-empty-opens-cwd-picker",
      a2Empty.promptType === "fileSearch" && a2Empty.inputValue === "~/",
      { promptType: a2Empty.promptType, inputValue: a2Empty.inputValue },
    );
    const a2Shot = await shot(driver, "a2-cwd-picker.png");
    // Escape back to the script list.
    await gpuiKey(driver, "escape");
    await waitUntil(driver, (s) => s.promptType === SCRIPT_LIST);
    driver.send({ type: "show" });
    await driver.waitForState({ windowVisible: true });
    await driver.setFilterAndWait("");

    // === A7: backquote types a literal char, no ghost accept ================
    await gpuiKey(driver, "`", [], "`");
    await Bun.sleep(200);
    const a7State = await driver.getState();
    record("a7-backquote-types-literal-char", a7State.inputValue === "`", {
      inputValue: a7State.inputValue,
      promptType: a7State.promptType,
    });
    await driver.setFilterAndWait("");

    // === A8: history recall ================================================
    // Mid-query first (before any history navigation exists): Up must NOT
    // recall history — the typed text stays. NOTE: this must run before the
    // recall tests because protocol setFilter bypasses the typing path that
    // resets history navigation (handle_filter_input_change), so a stale
    // history index from an earlier recall would leak into this step.
    await driver.setFilterAndWait("abc");
    await gpuiKey(driver, "up");
    await Bun.sleep(300);
    const a8Typed = await driver.getState();
    record("a8-up-mid-query-does-not-recall", a8Typed.inputValue === "abc", {
      inputValue: a8Typed.inputValue,
    });
    await driver.setFilterAndWait("");

    // Empty input at top → Up recalls most recent entry.
    await gpuiKey(driver, "up");
    const a8First = await waitUntil(driver, (s) => s.inputValue !== "");
    record("a8-up-on-empty-recalls-recent", a8First.inputValue === "launcher history", {
      inputValue: a8First.inputValue,
    });
    // Up again → continues deeper into history (the continuation fix).
    // Consecutive Ups are coalesced until the recalled filter has rendered
    // (key-repeat guard, cleared by a render ack). A live window acks within
    // one frame; the headless probe window renders lazily, so press Up
    // user-style until the guard clears (bounded retries).
    let a8Second: Json = {};
    for (let attempt = 0; attempt < 5; attempt++) {
      await gpuiKey(driver, "up");
      a8Second = await waitUntil(
        driver,
        (s) => s.inputValue === "launcher fixture",
        600,
      );
      if (a8Second.inputValue === "launcher fixture") break;
    }
    record("a8-up-again-continues-history", a8Second.inputValue === "launcher fixture", {
      inputValue: a8Second.inputValue,
    });
    const a8Shot = await shot(driver, "a8-history-recall.png");

    const recalledInput = String(a8Second.inputValue ?? "");
    const rowsBeforeDown = await waitForSelectableRows(driver, 2);
    const selectedBeforeDown = rowsBeforeDown.find((row) => row.selected === true) ?? null;
    const selectedPosition = rowsBeforeDown.findIndex((row) => row.selected === true);
    const expectedNext = selectedPosition >= 0 ? rowsBeforeDown[selectedPosition + 1] : null;
    record(
      "a8-recalled-query-has-two-selectable-rows",
      recalledInput === "launcher fixture" && rowsBeforeDown.length >= 2 && expectedNext != null,
      {
        inputValue: recalledInput,
        selectableCount: rowsBeforeDown.length,
        selectedIndex: selectedBeforeDown?.index ?? null,
        nextIndex: expectedNext?.index ?? null,
      },
    );

    await gpuiKey(driver, "down");
    const rowsAfterDown = await waitForSelectedRowIndex(driver, expectedNext?.index);
    const selectedAfterDown = rowsAfterDown.find((row) => row.selected === true) ?? null;
    const a8AfterDown = await driver.getState();
    record(
      "a8-first-down-exits-history-and-moves-list-selection",
      a8AfterDown.inputValue === recalledInput
        && selectedAfterDown?.index === expectedNext?.index,
      {
        inputBefore: recalledInput,
        inputAfter: a8AfterDown.inputValue,
        selectedIndexBefore: selectedBeforeDown?.index ?? null,
        selectedIndexAfter: selectedAfterDown?.index ?? null,
        expectedNextIndex: expectedNext?.index ?? null,
      },
    );

    const historyExitLogs = logMessages(await driver.getLogs({
      contains: "history_exit_to_list_down",
      limit: 500,
    }));
    const downRecallLogs = logMessages(await driver.getLogs({
      contains: "history_recalled",
      limit: 500,
    })).filter((message) =>
      message.includes("event=history_recalled") && message.includes("direction=down")
    );
    const clearToEmptyLogs = logMessages(await driver.getLogs({
      contains: "history_filter_render_pending_cancelled_obsolete",
      limit: 500,
    })).filter((message) =>
      message.includes("event=history_filter_render_pending_cancelled_obsolete")
        && message.includes("next_filter_len=0")
    );
    const recalledInputBytes = new TextEncoder().encode(recalledInput).byteLength;
    record(
      "a8-history-exit-structured-logs",
      historyExitLogs.length === 1
        && historyExitLogs[0].includes("history_index_after=None")
        && historyExitLogs[0].includes(`filter_len_before=${recalledInputBytes}`)
        && historyExitLogs[0].includes(`filter_len_after=${recalledInputBytes}`)
        && downRecallLogs.length === 0
        && clearToEmptyLogs.length === 0,
      { historyExitLogs, downRecallLogs, clearToEmptyLogs },
    );

    await gpuiKey(driver, "up");
    const rowsAfterListUp = await waitForSelectedRowIndex(
      driver,
      selectedBeforeDown?.index,
    );
    const selectedAfterListUp = rowsAfterListUp.find((row) => row.selected === true) ?? null;
    const a8AfterListUp = await driver.getState();
    record(
      "a8-up-after-history-exit-is-ordinary-list-navigation",
      a8AfterListUp.inputValue === recalledInput
        && selectedAfterListUp?.index === selectedBeforeDown?.index,
      {
        inputBefore: recalledInput,
        inputAfter: a8AfterListUp.inputValue,
        selectedIndexAfterDown: selectedAfterDown?.index ?? null,
        selectedIndexAfterUp: selectedAfterListUp?.index ?? null,
      },
    );
    await driver.setFilterAndWait("");

    // === A5: multi-line Cmd+V routes to Agent Chat ==========================
    await Bun.$`printf 'line one\nline two\nline three\n' | pbcopy`;
    await gpuiKey(driver, "v", ["cmd"]);
    const a5State = await waitUntil(driver, (s) => s.promptType !== SCRIPT_LIST);
    const pastedIntoFilter =
      typeof a5State.inputValue === "string" && a5State.inputValue.includes("line one");
    record(
      "a5-multiline-paste-routes-to-agent-chat",
      a5State.promptType !== SCRIPT_LIST && !pastedIntoFilter,
      { promptType: a5State.promptType, inputValue: a5State.inputValue },
    );
    const a5Shot = await shot(driver, "a5-agent-handoff.png");

    // --- receipt -------------------------------------------------------------
    const pass = results.every((r) => r.pass);
    console.log(
      JSON.stringify(
        {
          probe: "input-ambiguity",
          binary: BINARY,
          sessionDir: driver.sessionDir,
          outDir: OUT_DIR,
          screenshots: { a1Shot, a2Shot, a8Shot, a5Shot },
          pass,
          results,
        },
        null,
        2,
      ),
    );
    process.exitCode = pass ? 0 : 1;
  } finally {
    try {
      if (savedClipboard.length > 0) {
        const p = Bun.spawn(["pbcopy"], { stdin: "pipe" });
        p.stdin.write(savedClipboard);
        p.stdin.end();
        await p.exited;
      }
    } catch {
      // clipboard restore is best-effort
    }
    await driver.close();
  }
}

main();
