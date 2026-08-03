import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const baselineBinary = resolve("target-agent/artifacts/ux16-keyboard-feedback/script-kit-gpui");
const binary = resolve("target-agent/artifacts/ux7-selection-marker/script-kit-gpui");
const artifactDir = resolve(".artifacts/consistency/UX-007");
const receiptPath = resolve(artifactDir, "runtime-selection-marker.json");
mkdirSync(artifactDir, { recursive: true });

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(`${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`);
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

function components(layout: Json): Json[] {
  return Array.isArray(layout.components) ? layout.components as Json[] : [];
}

function mainMarkers(layout: Json): Json[] {
  return components(layout).filter((component) =>
    String(component.name ?? "").endsWith(":selection-marker")
  );
}

function actionsMarkers(layout: Json): Json[] {
  return components(layout).filter((component) =>
    String(component.name ?? "").startsWith("ActionsSelectionMarker[")
  );
}

function bounds(component: Json): Json {
  const value = component.bounds;
  assert(value && typeof value === "object", "component omitted bounds", component);
  return value as Json;
}

function assertMarkerGeometry(marker: Json, label: string): void {
  const rect = bounds(marker);
  assert(rect.width === 2, `${label} marker width changed`, marker);
  assert(rect.height === 16, `${label} marker height changed`, marker);
}

function rowGeometry(layout: Json): Json[] {
  return components(layout)
    .filter((component) => /^ListItem\[\d+\]$/.test(String(component.name ?? "")))
    .slice(0, 7)
    .map((component) => ({
      name: component.name,
      bounds: component.bounds,
      boxModel: component.boxModel,
      hitBounds: (component.visualStyle as Json | undefined)?.hitBounds,
    }));
}

function sameJson(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

async function screenshot(driver: Driver, target: Json, filename: string): Promise<Json> {
  const path = resolve(artifactDir, filename);
  const result = await driver.captureScreenshot({ target, savePath: path, timeoutMs: 10_000 });
  assert(!result.error && result.width && result.height, `screenshot failed: ${filename}`, result);
  return { path, width: result.width, height: result.height };
}

async function waitForMainMarker(
  driver: Driver,
  predicate: (marker: Json) => boolean,
  timeoutMs = 5_000,
): Promise<{ layout: Json; marker: Json }> {
  const deadline = Date.now() + timeoutMs;
  let last: Json = {};
  while (Date.now() < deadline) {
    const layout = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 10_000 });
    last = layout;
    const markers = mainMarkers(layout);
    if (markers.length === 1 && predicate(markers[0])) return { layout, marker: markers[0] };
    await Bun.sleep(25);
  }
  throw new Error(`Timed out waiting for one matching main marker\n${JSON.stringify(last, null, 2)}`);
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
  throw new Error(`Timed out waiting for window ${id} present=${present}\n${JSON.stringify(last, null, 2)}`);
}

const cleanups: Json[] = [];

async function withDriver<T>(
  executable: string,
  sessionName: string,
  run: (driver: Driver) => Promise<T>,
): Promise<T> {
  const driver = await Driver.launch({
    binary: executable,
    sessionName,
    sandboxHome: true,
    env: { SCRIPT_KIT_TEST_STATUS: "1" },
    readyTimeoutMs: 20_000,
    defaultTimeoutMs: 10_000,
  });
  try {
    return await run(driver);
  } finally {
    await driver.close();
    const ownedPids = exactExecutablePids(executable);
    const cleanup = {
      sessionName,
      binary: executable,
      ...driver.finalization,
      ownedProcessCount: ownedPids.length,
      ownedPids,
    };
    cleanups.push(cleanup);
    assert(driver.finalization.processExited, `${sessionName} process did not exit`, cleanup);
    assert(driver.finalization.streamsDrained, `${sessionName} streams did not drain`, cleanup);
    assert(driver.finalization.logWriterClosed, `${sessionName} log writer did not close`, cleanup);
    assert(ownedPids.length === 0, `${sessionName} left an owned process`, cleanup);
  }
}

let receipt: Json = {
  classification: "RUNTIME-FAILED",
  binary,
  baselineBinary,
  decisionBranch: "fallback-2x16-inset6-radius1-alphaFF",
};

try {
  const baseline = await withDriver(baselineBinary, "ux7-baseline", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 10_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });
    const layout = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 10_000 });
    assert(mainMarkers(layout).length === 0, "pre-UX-007 artifact unexpectedly painted a marker");
    return {
      rows: rowGeometry(layout),
      screenshot: await screenshot(driver, { type: "main" }, "main-before.png"),
    };
  });

  const main = await withDriver(binary, "ux7-main", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 10_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });

    const initial = await waitForMainMarker(driver, () => true);
    assertMarkerGeometry(initial.marker, "initial main");
    const initialName = String(initial.marker.name);
    const initialBounds = bounds(initial.marker);
    const rows = rowGeometry(initial.layout);
    assert(sameJson(rows, baseline.rows), "main row/content/hit geometry changed from the pre-marker artifact", {
      baseline: baseline.rows,
      current: rows,
    });

    const keyboardDispatch = await driver.simulateGpuiKeyDown("down", { target: { type: "main" } });
    const keyboard = await waitForMainMarker(driver, (marker) => String(marker.name) !== initialName);
    assertMarkerGeometry(keyboard.marker, "keyboard-selected main");
    assert(
      bounds(keyboard.marker).x === initialBounds.x,
      "keyboard selection changed marker horizontal placement",
      { initial: initial.marker, keyboard: keyboard.marker },
    );
    assert(
      sameJson(rowGeometry(keyboard.layout), rows),
      "keyboard selection changed non-marker row geometry",
    );

    const keyboardName = String(keyboard.marker.name);
    const keyboardClip = keyboard.marker.clipBounds as Json | undefined;
    assert(keyboardClip && typeof keyboardClip.y === "number", "marker omitted selected-row clip bounds", keyboard.marker);
    const pointerDispatch = await driver.simulateGpuiClick(
      100,
      Number(keyboardClip.y) - 22,
      { target: { type: "main" }, timeoutMs: 10_000 },
    );
    const pointer = await waitForMainMarker(driver, (marker) => String(marker.name) !== keyboardName);
    assertMarkerGeometry(pointer.marker, "pointer-selected main");
    assert(
      bounds(pointer.marker).x === bounds(keyboard.marker).x,
      "keyboard and pointer selection use different marker horizontal placement",
      { keyboard: keyboard.marker, pointer: pointer.marker },
    );
    assert(
      sameJson(rowGeometry(pointer.layout), rows),
      "pointer selection changed non-marker row geometry",
    );

    return {
      rowGeometryMatchesBaseline: true,
      initial: initial.marker,
      keyboard: { dispatch: keyboardDispatch, marker: keyboard.marker },
      pointer: { dispatch: pointerDispatch, marker: pointer.marker },
      screenshot: await screenshot(driver, { type: "main" }, "main-after.png"),
    };
  });

  const actions = await withDriver(binary, "ux7-actions", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 10_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });
    const openDispatch = await driver.simulateGpuiKeyDown("k", {
      modifiers: ["cmd"],
      target: { type: "main" },
    });
    await waitForWindow(driver, "actions-dialog", true);
    await Bun.sleep(100);
    const target = { type: "id", id: "actions-dialog" };
    const layout = await driver.getLayoutInfo({ target }, { timeoutMs: 10_000 });
    const markers = actionsMarkers(layout);
    assert(markers.length === 1, "Actions did not expose exactly one selected marker", layout);
    assertMarkerGeometry(markers[0], "Actions");
    const markerBounds = bounds(markers[0]);
    const parent = components(layout).find((component) => component.name === markers[0].parent);
    assert(parent, "Actions marker did not name its selected row parent", markers[0]);
    const parentBounds = bounds(parent);
    assert(
      markerBounds.y === Number(parentBounds.y) + (Number(parentBounds.height) - 16) / 2,
      "Actions marker is not vertically centered in its compact host row",
      { marker: markers[0], parent },
    );
    const state = await driver.getState({ target }, { timeoutMs: 10_000 });
    assert((state.actionsDialog as Json | undefined)?.selectedActionId, "Actions has no selected action", state);
    const shot = await screenshot(driver, target, "actions-selection-marker.png");
    const closeDispatch = await driver.simulateGpuiKeyDown("escape", { target });
    await waitForWindow(driver, "actions-dialog", false);
    return { openDispatch, closeDispatch, state, marker: markers[0], parent, screenshot: shot };
  });

  const builtin = await withDriver(binary, "ux7-process-manager", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 10_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });
    driver.send({ type: "triggerBuiltin", name: "process-manager" });
    await Bun.sleep(500);
    const state = await driver.getState({ timeoutMs: 10_000 });
    assert(state.promptType === "processManager", "Process Manager fixture did not open", state);
    const layout = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 10_000 });
    const markers = mainMarkers(layout);
    assert(markers.length === 1, "Process Manager did not inherit one shared marker", layout);
    assertMarkerGeometry(markers[0], "Process Manager");
    return {
      state,
      marker: markers[0],
      screenshot: await screenshot(driver, { type: "main" }, "process-manager-selection-marker.png"),
    };
  });

  const unifiedNegative = await withDriver(binary, "ux7-unified-negative", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 10_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });
    driver.send({
      type: "select",
      id: "ux7-select",
      placeholder: "Pick",
      choices: [
        { name: "One", value: "1" },
        { name: "Two", value: "2" },
      ],
      multiple: false,
    });
    await Bun.sleep(500);
    const state = await driver.getState({ timeoutMs: 10_000 });
    assert(state.promptType === "select", "Unified Select prompt did not open", state);
    const layout = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 10_000 });
    assert(mainMarkers(layout).length === 0, "Unified row family received a launcher marker", layout);
    return { state, markerCount: 0 };
  });

  const compactNegative = await withDriver(binary, "ux7-compact-negative", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 10_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });
    driver.send({ type: "openDictationOverlayFixture" });
    await waitForWindow(driver, "dictation", true);
    driver.send({ type: "openDictationMicrophonePopupFixture" });
    const popup = await waitForWindow(driver, "dictation-microphone-popup", true);
    assert(popup?.generation, "compact popup generation missing", popup);
    const target = { type: "instance", id: popup.id, generation: popup.generation };
    const elements = await driver.getElements({ target }, { timeoutMs: 10_000 });
    const semanticIds = Array.isArray(elements.elements)
      ? (elements.elements as Json[]).map((element) => element.semanticId)
      : [];
    assert(
      semanticIds.includes("choice:0:dictation-mic-row-0") && semanticIds.includes("choice:1:dictation-mic-row-1"),
      "compact microphone rows missing",
      elements,
    );
    const layout = await driver.getLayoutInfo({ target }, { timeoutMs: 10_000 });
    assert(
      components(layout).every((component) => !String(component.name ?? "").includes("selection-marker")),
      "compact row family received a launcher marker",
      layout,
    );
    const shot = await screenshot(driver, target, "compact-no-selection-marker.png");
    const closeDispatch = await driver.simulateGpuiKeyDown("escape", {
      target: { type: "id", id: "dictation" },
    });
    await waitForWindow(driver, "dictation-microphone-popup", false);
    return { target, semanticIds, markerCount: 0, closeDispatch, screenshot: shot };
  });

  receipt = {
    classification: "RUNTIME-CONFIRMED",
    binary,
    baselineBinary,
    decisionBranch: "fallback-2x16-inset6-radius1-alphaFF",
    baseline,
    main,
    actions,
    builtin,
    unifiedNegative,
    compactNegative,
  };
} catch (error) {
  receipt.error = error instanceof Error ? { message: error.message, stack: error.stack } : String(error);
} finally {
  receipt.cleanup = cleanups;
  receipt.finalOwnedProcesses = {
    baseline: exactExecutablePids(baselineBinary),
    current: exactExecutablePids(binary),
  };
  await Bun.write(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
}

console.log(JSON.stringify(receipt, null, 2));
assert(receipt.classification === "RUNTIME-CONFIRMED", "UX-007 runtime proof failed", receipt);
