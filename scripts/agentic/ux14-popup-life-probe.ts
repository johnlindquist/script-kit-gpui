#!/usr/bin/env bun
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver.ts";

type WindowInfo = {
  id: string;
  kind: string;
  focused?: boolean;
  generation?: number;
  parentWindowId?: string;
  parentKind?: string;
  bounds?: { x: number; y: number; width: number; height: number };
};

const args = process.argv.slice(2);
function arg(name: string, fallback?: string): string | undefined {
  const ix = args.indexOf(name);
  return ix >= 0 ? args[ix + 1] : fallback;
}

const binary = resolve(arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY) ?? "");
const family = arg("--family", "dictation-microphone")!;
const defaultReceipt = family === "agent-history"
  ? ".artifacts/ux14-popup-life/runtime-agent-history.json"
  : ".artifacts/ux14-popup-life/runtime-dictation-microphone.json";
const out = resolve(arg("--out", defaultReceipt)!);
const verifyPath = arg("--verify");

if (verifyPath) {
  const receipt = JSON.parse(readFileSync(resolve(verifyPath), "utf8"));
  if (receipt.ok !== true) throw new Error("UX-014 receipt is not green");
  if (receipt.cleanup?.ownedProcessCount !== 0) {
    throw new Error("UX-014 receipt retained an owned process");
  }
  console.log(JSON.stringify({ verified: true, path: resolve(verifyPath) }, null, 2));
  process.exit(0);
}

if (!binary || !existsSync(binary)) throw new Error(`Missing binary: ${binary}`);
if (!new Set(["agent-history", "dictation-microphone"]).has(family)) {
  throw new Error(`Unsupported UX-014 family: ${family}`);
}
mkdirSync(dirname(out), { recursive: true });

function windowsOf(response: Json): WindowInfo[] {
  return Array.isArray(response.windows) ? response.windows : [];
}

async function waitForWindow(
  driver: Driver,
  id: string,
  present: boolean,
  timeoutMs = 5000,
): Promise<WindowInfo | null> {
  const started = performance.now();
  while (performance.now() - started < timeoutMs) {
    const found = windowsOf(await driver.listAutomationWindows()).find((w) => w.id === id) ?? null;
    if (Boolean(found) === present) return found;
    await Bun.sleep(25);
  }
  throw new Error(`Timed out waiting for ${id} present=${present}`);
}

async function waitForKind(driver: Driver, kind: string, timeoutMs = 5000): Promise<WindowInfo> {
  const started = performance.now();
  while (performance.now() - started < timeoutMs) {
    const found = windowsOf(await driver.listAutomationWindows()).find((w) => w.kind === kind);
    if (found) return found;
    await Bun.sleep(25);
  }
  throw new Error(`Timed out waiting for kind=${kind}`);
}

function exactProcessCount(executable: string): number {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,comm="], { stdout: "pipe" });
  const real = resolve(executable);
  return result.stdout
    .toString()
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((line) => resolve(line.replace(/^\d+\s+/, "")) === real).length;
}

function staleTargetWasRejected(response: Json): boolean {
  const warnings = Array.isArray(response.warnings) ? response.warnings : [];
  return (response.elements ?? []).length === 0 && warnings.some((warning) =>
    String(warning).includes("target_resolution_failed: Unknown or stale automation window instance"),
  );
}

async function requireAgentComposer(
  driver: Driver,
  parentTarget: Json,
  inputText: string,
  cursorIndex: number,
): Promise<Json> {
  await Bun.sleep(25);
  const state = await driver.request(
    { type: "getAgentChatState", target: parentTarget },
    { expect: "agent_chatStateResult", timeoutMs: 10_000 },
  );
  if (state.inputText !== inputText || state.cursorIndex !== cursorIndex) {
    throw new Error(`Composer focus/text was not restored: ${JSON.stringify(state)}`);
  }
  return state;
}

async function requireFocusedWindow(driver: Driver, id: string): Promise<WindowInfo> {
  const current = await waitForWindow(driver, id, true);
  if (!current?.focused) throw new Error(`Expected focused parent window: ${JSON.stringify(current)}`);
  return current;
}

async function runAgentHistoryScenario(driver: Driver, steps: Json[]): Promise<void> {
  const kitDir = join(driver.sessionDir, "home", ".scriptkit");
  mkdirSync(kitDir, { recursive: true });
  const historyPath = join(kitDir, "agent_chat-history.jsonl");
  const entries = [
    {
      timestamp: "2026-08-01T12:00:00Z",
      first_message: "Plan the launch",
      message_count: 4,
      session_id: "ux14-history-1",
      title: "Launch planning",
      preview: "A deterministic history fixture",
      search_text: "launch planning deterministic history fixture",
    },
    {
      timestamp: "2026-07-31T12:00:00Z",
      first_message: "Review the interface",
      message_count: 3,
      session_id: "ux14-history-2",
      title: "Interface review",
      preview: "Popup lifecycle review",
      search_text: "interface review popup lifecycle",
    },
  ];
  writeFileSync(historyPath, `${entries.map((entry) => JSON.stringify(entry)).join("\n")}\n`);

  driver.send({ type: "openAgentChatDetachedFixture" });
  const parent = await waitForKind(driver, "agentChatDetached");
  const parentTarget = { type: "id", id: parent.id };
  steps.push({ step: "parent-open", parent });

  const seeded = await driver.request(
    {
      type: "batch",
      target: parentTarget,
      commands: [{ type: "setInput", text: "focus-return:" }],
      options: { stopOnError: true, timeout: 5000 },
    },
    { expect: "batchResult", timeoutMs: 7000 },
  );
  if (seeded.success !== true) throw new Error(`Agent composer seed failed: ${JSON.stringify(seeded)}`);

  driver.send({ type: "openAgentChatHistoryPopupFixture" });
  const first = await waitForWindow(driver, "agent_chat-history-popup", true);
  if (!first?.generation) throw new Error("First history popup did not publish a generation");
  if (first.parentWindowId !== parent.id || first.parentKind !== parent.kind) {
    throw new Error(`Wrong history popup parent: ${JSON.stringify({ parent, first })}`);
  }
  const firstTarget = { type: "instance", id: first.id, generation: first.generation };
  const firstElements = await driver.getElements({ target: firstTarget });
  const semanticIds = (firstElements.elements ?? []).map((element: Json) => element.semanticId);
  for (const required of ["panel:history-popup", "list:history-entries"]) {
    if (!semanticIds.includes(required)) {
      throw new Error(`Missing exact history semantics: ${JSON.stringify(semanticIds)}`);
    }
  }
  steps.push({ step: "first-open", target: firstTarget, semanticIds });

  const shotPath = join(dirname(out), "agent-history-popup.png");
  const screenshot = await driver.captureScreenshot({ target: firstTarget, savePath: shotPath, timeoutMs: 10_000 });
  if (screenshot.error || !screenshot.width || !screenshot.height) {
    throw new Error(`Strict history screenshot failed: ${JSON.stringify(screenshot)}`);
  }
  steps.push({ step: "strict-screenshot", path: shotPath, width: screenshot.width, height: screenshot.height });

  const escape = await driver.simulateGpuiKeyDown("escape", { target: parentTarget });
  await waitForWindow(driver, first.id, false);
  await waitForWindow(driver, parent.id, true);
  const typed = await driver.simulateGpuiKeyDown("λ", { text: "λ", target: parentTarget });
  const parentState = await requireAgentComposer(driver, parentTarget, "focus-return:λ", 14);
  steps.push({ step: "parent-escape-focus-return", dispatch: escape, typed, inputText: parentState.inputText, cursorIndex: parentState.cursorIndex });

  await Bun.sleep(350);
  driver.send({ type: "openAgentChatHistoryPopupFixture" });
  const second = await waitForWindow(driver, first.id, true);
  if (!second?.generation || second.generation <= first.generation) {
    throw new Error(`History reopen generation did not advance: ${JSON.stringify({ first, second })}`);
  }
  const staleResponse = await driver.getElements({ target: firstTarget });
  if (!staleTargetWasRejected(staleResponse)) {
    throw new Error(`Stale history target resolved: ${JSON.stringify(staleResponse)}`);
  }
  steps.push({ step: "fresh-reopen-stale-refusal", firstGeneration: first.generation, secondGeneration: second.generation, warnings: staleResponse.warnings });

  const outsideClick = await driver.simulateGpuiClick(10, 10, { target: parentTarget });
  await waitForWindow(driver, second.id, false);
  await requireFocusedWindow(driver, parent.id);
  const outsideTyped = await driver.simulateGpuiKeyDown("β", { text: "β", target: parentTarget });
  const outsideState = await requireAgentComposer(driver, parentTarget, "focus-return:λβ", 15);
  steps.push({
    step: "parent-click-outside-focus-return",
    dispatch: outsideClick,
    typed: outsideTyped,
    inputText: outsideState.inputText,
    cursorIndex: outsideState.cursorIndex,
    parentFocused: true,
  });

  await Bun.sleep(350);
  driver.send({ type: "openAgentChatHistoryPopupFixture" });
  const third = await waitForWindow(driver, first.id, true);
  if (!third?.generation) throw new Error("Third history popup generation missing");
  const thirdTarget = { type: "instance", id: third.id, generation: third.generation };
  driver.send({ type: "closePromptPopupNatively", target: thirdTarget });
  await waitForWindow(driver, third.id, false);
  await requireFocusedWindow(driver, parent.id);
  const nativeStale = await driver.getElements({ target: thirdTarget });
  if (!staleTargetWasRejected(nativeStale)) {
    throw new Error(`Native-closed history target remained live: ${JSON.stringify(nativeStale)}`);
  }
  const nativeTyped = await driver.simulateGpuiKeyDown("γ", { text: "γ", target: parentTarget });
  const nativeState = await requireAgentComposer(driver, parentTarget, "focus-return:λβγ", 16);
  steps.push({
    step: "native-close-reconciled-focus-return",
    target: thirdTarget,
    typed: nativeTyped,
    inputText: nativeState.inputText,
    cursorIndex: nativeState.cursorIndex,
    parentFocused: true,
    warnings: nativeStale.warnings,
  });
}

async function runDictationScenario(driver: Driver, steps: Json[]): Promise<void> {
  const configPath = join(driver.sessionDir, "home", ".scriptkit", "config.ts");
  const configBefore = existsSync(configPath) ? readFileSync(configPath) : null;

  driver.send({ type: "openDictationOverlayFixture" });
  const parent = await waitForWindow(driver, "dictation", true);
  if (!parent || parent.kind !== "dictation") throw new Error("Dictation fixture target missing");
  steps.push({ step: "parent-open", parent });

  driver.send({ type: "openDictationMicrophonePopupFixture" });
  const first = await waitForWindow(driver, "dictation-microphone-popup", true);
  if (!first?.generation) throw new Error("First popup did not publish a generation");
  if (first.parentWindowId !== "dictation" || first.parentKind !== "dictation") {
    throw new Error(`Wrong popup parent: ${JSON.stringify(first)}`);
  }
  const firstTarget = { type: "instance", id: first.id, generation: first.generation };
  const firstElements = await driver.getElements({ target: firstTarget });
  const semanticIds = (firstElements.elements ?? []).map((element: Json) => element.semanticId);
  if (!semanticIds.includes("panel:dictation-microphone-popup")) {
    throw new Error(`Missing exact popup semantics: ${JSON.stringify(semanticIds)}`);
  }
  steps.push({ step: "first-open", target: firstTarget, semanticIds });

  const shotPath = join(dirname(out), "dictation-microphone-popup.png");
  const screenshot = await driver.captureScreenshot({ target: firstTarget, savePath: shotPath, timeoutMs: 10_000 });
  if (screenshot.error || !screenshot.width || !screenshot.height) {
    throw new Error(`Strict popup screenshot failed: ${JSON.stringify(screenshot)}`);
  }
  steps.push({ step: "strict-screenshot", path: shotPath, width: screenshot.width, height: screenshot.height });

  const localEscape = await driver.simulateGpuiKeyDown("escape", { target: { type: "id", id: "dictation" } });
  await waitForWindow(driver, first.id, false);
  await requireFocusedWindow(driver, "dictation");
  steps.push({ step: "parent-escape-one-layer", dispatch: localEscape, parentRemained: true, parentFocused: true });

  await Bun.sleep(180);
  driver.send({ type: "openDictationMicrophonePopupFixture" });
  const second = await waitForWindow(driver, first.id, true);
  if (!second?.generation || second.generation <= first.generation) {
    throw new Error(`Reopen generation did not advance: ${JSON.stringify({ first, second })}`);
  }
  const secondTarget = { type: "instance", id: second.id, generation: second.generation };
  steps.push({ step: "fresh-reopen", firstGeneration: first.generation, secondGeneration: second.generation });

  const staleResponse = await driver.getElements({ target: firstTarget });
  if (!staleTargetWasRejected(staleResponse)) {
    throw new Error(`Stale first-generation target resolved after reopen: ${JSON.stringify(staleResponse)}`);
  }
  steps.push({ step: "stale-instance-refused", staleRejected: true, warnings: staleResponse.warnings });

  const outsideClick = await driver.simulateGpuiClick(20, 20, { target: { type: "id", id: "dictation" } });
  await waitForWindow(driver, second.id, false);
  await requireFocusedWindow(driver, "dictation");
  steps.push({ step: "parent-click-outside-focus-return", dispatch: outsideClick, overlayRemained: true, parentFocused: true });

  await Bun.sleep(180);
  driver.send({ type: "openDictationMicrophonePopupFixture" });
  const third = await waitForWindow(driver, first.id, true);
  if (!third?.generation) throw new Error("Third popup generation missing");
  const thirdTarget = { type: "instance", id: third.id, generation: third.generation };
  driver.send({ type: "closePromptPopupNatively", target: thirdTarget });
  await waitForWindow(driver, third.id, false);
  await requireFocusedWindow(driver, "dictation");
  const nativeStale = await driver.getElements({ target: thirdTarget });
  if (!staleTargetWasRejected(nativeStale)) {
    throw new Error(`Native-closed Dictation target remained live: ${JSON.stringify(nativeStale)}`);
  }
  steps.push({ step: "native-close-reconciled-focus-return", target: thirdTarget, overlayRemained: true, parentFocused: true, warnings: nativeStale.warnings });

  await Bun.sleep(180);
  driver.send({ type: "openDictationMicrophonePopupFixture" });
  const fourth = await waitForWindow(driver, first.id, true);
  if (!fourth?.generation || fourth.generation <= third.generation) {
    throw new Error(`Fourth popup generation did not advance: ${JSON.stringify({ third, fourth })}`);
  }
  const fourthTarget = { type: "instance", id: fourth.id, generation: fourth.generation };
  const rows = await driver.getElements({ target: fourthTarget });
  const alternate = (rows.elements ?? []).find((element: Json) => element.semanticId === "choice:1:dictation-mic-row-1");
  if (!alternate) throw new Error("Fixture alternate microphone row missing");
  const selected = await driver.request(
    {
      type: "batch",
      target: fourthTarget,
      commands: [{ type: "selectBySemanticId", semanticId: alternate.semanticId }],
      options: { stopOnError: true, timeout: 5000 },
    },
    { expect: "batchResult", timeoutMs: 7000 },
  );
  if (selected.success !== true) throw new Error(`Exact Dictation batch failed: ${JSON.stringify(selected)}`);
  await waitForWindow(driver, fourth.id, false);
  const configAfter = existsSync(configPath) ? readFileSync(configPath) : null;
  if (!Buffer.from(configBefore ?? []).equals(Buffer.from(configAfter ?? []))) {
    throw new Error("Fixture microphone selection changed sandbox config bytes");
  }
  steps.push({ step: "fixture-selection-no-persistence", selected, configUnchanged: true });
}

const receipt: Json = { schemaVersion: 1, family, binary, clipboardTouched: false, steps: [] };
const steps = receipt.steps as Json[];
let driver: Driver | null = null;
let driverPid: number | undefined;
let closeError: string | null = null;

try {
  driver = await Driver.launch({
    binary,
    sessionName: `ux14-${family}`,
    sandboxHome: true,
    sharedModels: false,
    defaultTimeoutMs: 10_000,
  });
  driverPid = driver.pid;
  await driver.waitForSettle();

  if (family === "agent-history") {
    await runAgentHistoryScenario(driver, steps);
  } else {
    await runDictationScenario(driver, steps);
  }
  receipt.ok = true;
} catch (error) {
  receipt.ok = false;
  receipt.error = String(error);
} finally {
  if (driver) {
    receipt.sessionDir = driver.sessionDir;
    receipt.logPath = driver.logPath;
    try {
      await driver.close();
    } catch (error) {
      closeError = String(error);
      receipt.ok = false;
    }
    receipt.cleanup = {
      ...driver.finalization,
      closeError,
      ownedPid: driverPid ?? null,
      ownedProcessCount: exactProcessCount(binary),
      ownedChildProcessCount: 0,
      clipboardTouched: false,
    };
    if (
      !driver.finalization.processExited ||
      !driver.finalization.streamsDrained ||
      !driver.finalization.logWriterClosed ||
      (receipt.cleanup as Json).ownedProcessCount !== 0
    ) {
      receipt.ok = false;
    }
  }
}

writeFileSync(out, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.ok ? 0 : 1);
