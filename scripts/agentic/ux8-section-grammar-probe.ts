#!/usr/bin/env bun

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const binary = resolve("target-agent/artifacts/ux8-section-grammar/script-kit-gpui");
const artifactDir = resolve(".artifacts/consistency/UX-008");
const receiptPath = join(artifactDir, "runtime-section-grammar.json");
const sessionDir = join("/tmp", `sk-ux8-section-grammar-${process.pid}`);
const homeDir = join("/tmp", `sk-ux8-section-home-${process.pid}`);
const kitDir = join(homeDir, ".scriptkit");
mkdirSync(artifactDir, { recursive: true });
rmSync(sessionDir, { recursive: true, force: true });
rmSync(homeDir, { recursive: true, force: true });
mkdirSync(join(kitDir, "plugins", "main", "scripts"), { recursive: true });
writeFileSync(join(kitDir, "config.ts"), "export default {};\n");
writeFileSync(
  join(kitDir, "plugins", "main", "scripts", "section-grammar-alpha.ts"),
  "// Name: Section Grammar Alpha\n// Description: Deterministic UX-008 script\nawait arg('Alpha')\n",
);
writeFileSync(
  join(kitDir, "plugins", "main", "scripts", "section-grammar-beta.ts"),
  "// Name: Section Grammar Beta\n// Description: Deterministic UX-008 script\nawait arg('Beta')\n",
);

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(`${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`);
  }
}

function sha256(path: string): string {
  const result = Bun.spawnSync(["shasum", "-a", "256", path], {
    stdout: "pipe",
    stderr: "pipe",
  });
  assert(result.exitCode === 0, "failed to hash runtime binary");
  return new TextDecoder().decode(result.stdout).trim().split(/\s+/, 1)[0];
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

function components(layout: Json): Json[] {
  return Array.isArray(layout.components) ? layout.components as Json[] : [];
}

function mainMarkers(layout: Json): Json[] {
  return components(layout).filter((component) =>
    String(component.name ?? "").endsWith(":selection-marker")
  );
}

function elements(result: Json): Json[] {
  return Array.isArray(result.elements) ? result.elements as Json[] : [];
}

function sectionElements(result: Json): Json[] {
  return elements(result).filter((element) => element.role === "sectionHeader");
}

function choiceElements(result: Json): Json[] {
  return elements(result).filter((element) => element.type === "choice" && element.selectable === true);
}

function uppercase(value: string): string {
  return value.toLocaleUpperCase("en-US");
}

async function capture(driver: Driver, filename: string, target: Json = { type: "main" }): Promise<Json> {
  const path = join(artifactDir, filename);
  const shot = await driver.captureScreenshot({ target, savePath: path, timeoutMs: 10_000 });
  assert(!shot.error && shot.width && shot.height, `screenshot failed: ${filename}`, shot);
  return { path, width: shot.width, height: shot.height };
}

async function waitForWindow(driver: Driver, id: string, present: boolean, timeoutMs = 5_000): Promise<Json | null> {
  const deadline = Date.now() + timeoutMs;
  let last: Json = {};
  while (Date.now() < deadline) {
    last = await driver.listAutomationWindows({ timeoutMs: 10_000 });
    const windows = Array.isArray(last.windows) ? last.windows as Json[] : [];
    const found = windows.find((window) => window.id === id) ?? null;
    if (Boolean(found) === present) return found;
    await Bun.sleep(25);
  }
  throw new Error(`Timed out waiting for ${id} present=${present}\n${JSON.stringify(last, null, 2)}`);
}

async function waitForActionsState(driver: Driver, target: Json, timeoutMs = 5_000): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let last: Json = {};
  while (Date.now() < deadline) {
    last = await driver.request(
      { type: "getState", target, summaryOnly: true },
      { expect: "stateResult", timeoutMs: 10_000 },
    );
    if ((last.actionsDialog as Json | undefined)?.surface === "actionsDialog") return last;
    await Bun.sleep(25);
  }
  throw new Error(`Actions dialog did not become automation-ready\n${JSON.stringify(last, null, 2)}`);
}

async function mainState(driver: Driver, name: string, filter: string): Promise<Json> {
  await driver.setFilterAndWait(filter, { timeoutMs: 10_000 });
  await driver.waitForSettle({ timeoutMs: 10_000 });
  const state = await driver.getState({ target: { type: "main" } }, { timeoutMs: 10_000 });
  const count = Number(state.visibleChoiceCount ?? 0);
  if (count === 0) {
    const semantic = await driver.getElements(
      { target: { type: "main" }, limit: 160, includeHeaders: true },
      { timeoutMs: 10_000 },
    );
    assert(sectionElements(semantic).length === 0, `${name}: zero results exposed a section heading`, semantic);
    return { name, filter, count, state, semantic };
  }

  await driver.simulateGpuiKeyDown("home", { target: { type: "main" }, timeoutMs: 10_000 });
  await driver.waitForSettle({ timeoutMs: 10_000 });
  const semantic = await driver.getElements(
    { target: { type: "main" }, limit: 160, includeHeaders: true },
    { timeoutMs: 10_000 },
  );
  const headers = sectionElements(semantic);
  const choices = choiceElements(semantic);
  assert(choices.length > 0, `${name}: non-empty state omitted selectable choices`, semantic);
  assert(headers.length > 0, `${name}: non-empty grouped state omitted its semantic section`, semantic);
  assert(
    !headers.some((header) => String(header.text ?? "").trim().length === 0),
    `${name}: an empty reserved slot leaked as a semantic heading`,
    headers,
  );
  const sourceBackedHeaders = headers.filter((header) => typeof header.value === "string");
  for (const header of sourceBackedHeaders) {
    const authored = String(header.value);
    const display = String(header.text ?? "");
    assert(display === uppercase(authored), `${name}: display label is not presentation-only uppercase`, header);
  }
  const layout = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 10_000 });
  const markers = mainMarkers(layout);
  assert(markers.length === 1, `${name}: expected one selected-row marker`, layout);
  const clip = markers[0].clipBounds as Json | undefined;
  assert(clip && typeof clip.y === "number", `${name}: selected row omitted clip bounds`, markers[0]);
  const selected = choices.find((choice) => choice.selected === true);
  assert(selected, `${name}: Home did not select the first selectable row`, choices);
  assert(selected.semanticId === choices[0].semanticId, `${name}: selection is not on first selectable row`, choices);
  return {
    name,
    filter,
    count,
    firstSelectableY: clip.y,
    firstHeader: sourceBackedHeaders[0] ?? headers[0],
    selected,
    state,
  };
}

const binarySha256 = sha256(binary);
let receipt: Json = {
  classification: "RUNTIME-FAILED",
  binary,
  binarySha256,
  checks: {},
};
let driver: Driver | null = null;

try {
  driver = await Driver.launch({
    binary,
    sessionName: "ux8-section-grammar",
    sessionDir,
    sandboxHome: false,
    env: {
      HOME: homeDir,
      SK_PATH: kitDir,
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
    },
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 10_000,
  });
  await driver.request({ type: "show" }, { timeoutMs: 10_000 });
  await driver.waitForSettle({ timeoutMs: 10_000 });

  const states = [
    await mainState(driver, "empty-filter", ""),
    await mainState(driver, "text-filter-same-family", "section"),
    await mainState(driver, "text-filter-changed-label", "notes"),
    await mainState(driver, "structured-type-filter", "type:script"),
  ];
  for (const state of states) {
    assert(state.count > 0, `${state.name}: deterministic filter unexpectedly returned zero`, state);
  }
  const yValues = states.map((state) => state.firstSelectableY);
  assert(new Set(yValues).size === 1, "first selectable row moved across non-empty query states", states);
  assert(
    states.some((state) =>
      typeof state.firstHeader.value === "string" && state.firstHeader.text !== state.firstHeader.value
    ),
    "runtime fixture did not prove source/display label separation",
    states,
  );
  const mainShot = await capture(driver, "main-uppercase-stable-slot.png");

  const zero = await mainState(driver, "zero-results", "type:script __ux8_no_match_8f6c__");
  assert(zero.count === 0, "zero-result control unexpectedly matched", zero);
  const returned = await mainState(driver, "return-to-results", "");
  assert(returned.firstSelectableY === yValues[0], "first row moved after zero-result round trip", returned);

  const openActions = await driver.simulateGpuiKeyDown("k", {
    modifiers: ["cmd"],
    target: { type: "main" },
    timeoutMs: 10_000,
  });
  await waitForWindow(driver, "actions-dialog", true);
  const actionsTarget = { type: "id", id: "actions-dialog" };
  const actionsState = await waitForActionsState(driver, actionsTarget);
  await Bun.sleep(350);
  const actionsDialog = actionsState.actionsDialog as Json | undefined;
  const rowGeometry = actionsDialog?.rowGeometry as Json | undefined;
  const sections = Array.isArray(rowGeometry?.sections) ? rowGeometry.sections as Json[] : [];
  assert(sections.length > 0, "Actions header-mode fixture exposed no sections", actionsState);
  for (const section of sections) {
    assert(section.displayLabel === uppercase(String(section.semanticLabel)), "Actions display label is not uppercase", section);
    assert(section.labelTier === "strong", "Actions section label is not strong", section);
    assert(section.countTier === "muted" && section.iconTier === "muted", "Actions metadata tiers are not muted", section);
  }
  const actionsShot = await capture(driver, "actions-uppercase-sections.png", actionsTarget);
  await driver.simulateGpuiKeyDown("escape", { target: actionsTarget, timeoutMs: 10_000 });
  await waitForWindow(driver, "actions-dialog", false);

  driver.send({ type: "openDictationOverlayFixture" });
  await waitForWindow(driver, "dictation", true);
  driver.send({ type: "openDictationMicrophonePopupFixture" });
  const popup = await waitForWindow(driver, "dictation-microphone-popup", true);
  assert(popup?.generation, "compact popup generation missing", popup);
  const compactTarget = { type: "instance", id: popup.id, generation: popup.generation };
  const compactElements = await driver.getElements({ target: compactTarget, limit: 80 }, { timeoutMs: 10_000 });
  assert(sectionElements(compactElements).length === 0, "compact no-header popup acquired a section heading", compactElements);
  const compactShot = await capture(driver, "compact-popup-remains-headerless.png", compactTarget);
  await driver.simulateGpuiKeyDown("escape", { target: { type: "id", id: "dictation" }, timeoutMs: 10_000 });
  await waitForWindow(driver, "dictation-microphone-popup", false);

  receipt = {
    classification: "RUNTIME-CONFIRMED",
    binary,
    binarySha256,
    checks: {
      main: {
        states,
        zero,
        returned,
        firstSelectableY: yValues[0],
        screenshot: mainShot,
      },
      actions: {
        openDispatch: openActions,
        sections,
        screenshot: actionsShot,
      },
      compactNegative: {
        target: compactTarget,
        sectionCount: 0,
        screenshot: compactShot,
      },
    },
  };
} catch (error) {
  receipt.error = error instanceof Error ? { message: error.message, stack: error.stack } : String(error);
} finally {
  if (driver) {
    await driver.close();
    receipt.cleanup = {
      ...driver.finalization,
      ownedProcessCount: exactExecutablePids(binary).length,
      ownedPids: exactExecutablePids(binary),
    };
  }
  rmSync(sessionDir, { recursive: true, force: true });
  rmSync(homeDir, { recursive: true, force: true });
  await Bun.write(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
}

console.log(JSON.stringify(receipt, null, 2));
assert(receipt.classification === "RUNTIME-CONFIRMED", "UX-008 runtime proof failed", receipt);
assert(receipt.cleanup?.processExited === true, "UX-008 process did not exit", receipt.cleanup);
assert(receipt.cleanup?.streamsDrained === true, "UX-008 streams did not drain", receipt.cleanup);
assert(receipt.cleanup?.logWriterClosed === true, "UX-008 log writer did not close", receipt.cleanup);
assert(receipt.cleanup?.ownedProcessCount === 0, "UX-008 left an owned process", receipt.cleanup);
