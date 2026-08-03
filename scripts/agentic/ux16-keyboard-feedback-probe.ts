import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const binary = resolve("target-agent/artifacts/ux16-keyboard-feedback/script-kit-gpui");
const artifactDir = resolve(".artifacts/consistency/UX-016");
const screenshotPath = resolve(artifactDir, "runtime-keyboard-feedback.png");
const receiptPath = resolve(artifactDir, "runtime-keyboard-feedback.json");
mkdirSync(dirname(screenshotPath), { recursive: true });

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(`${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`);
  }
}

function allStrings(value: unknown, out = new Set<string>()): Set<string> {
  if (typeof value === "string") {
    out.add(value);
  } else if (Array.isArray(value)) {
    for (const item of value) allStrings(item, out);
  } else if (value && typeof value === "object") {
    for (const item of Object.values(value as Record<string, unknown>)) allStrings(item, out);
  }
  return out;
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

function focusedSemanticIds(elementsResult: Json): string[] {
  const elements = Array.isArray(elementsResult.elements) ? elementsResult.elements : [];
  return elements
    .filter((item) => item && typeof item === "object" && (item as Record<string, unknown>).focused === true)
    .map((item) => String((item as Record<string, unknown>).semanticId ?? ""))
    .filter(Boolean);
}

function componentBounds(layout: Json, id: string): { x: number; y: number; width: number; height: number } {
  const components = Array.isArray(layout.components) ? layout.components : [];
  const component = components.find(
    (item) => item && typeof item === "object" && (item as Record<string, unknown>).name === id,
  ) as Record<string, unknown> | undefined;
  const bounds = component?.bounds as Record<string, unknown> | undefined;
  assert(
    bounds
      && typeof bounds.x === "number"
      && typeof bounds.y === "number"
      && typeof bounds.width === "number"
      && typeof bounds.height === "number",
    `runtime layout omitted bounds for ${id}`,
    component,
  );
  return bounds as { x: number; y: number; width: number; height: number };
}

async function layoutIds(driver: Driver): Promise<{ raw: Json; ids: Set<string> }> {
  const raw = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 10_000 });
  return { raw, ids: allStrings(raw) };
}

async function waitForControl(
  driver: Driver,
  id: string,
  present: boolean,
  timeoutMs = 5_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let latest: Json = {};
  while (Date.now() < deadline) {
    const snapshot = await layoutIds(driver);
    latest = snapshot.raw;
    if (snapshot.ids.has(id) === present) return latest;
    await Bun.sleep(25);
  }
  throw new Error(`Timed out waiting for ${id} present=${present}\n${JSON.stringify(latest, null, 2)}`);
}

const expectedControls = [
  "toast:ux16-runtime-a:root",
  "toast:ux16-runtime-a:action:open-local",
  "toast:ux16-runtime-a:action:open-details",
  "toast:ux16-runtime-a:dismiss",
  "toast:ux16-runtime-b:root",
  "toast:ux16-runtime-b:action:open-remote",
  "toast:ux16-runtime-b:dismiss",
];

const driver = await Driver.launch({
  binary,
  sessionName: "ux16-keyboard-feedback",
  sandboxHome: true,
  env: {
    SCRIPT_KIT_TEST_STATUS: "1",
    SCRIPT_KIT_TEST_KEYBOARD_FEEDBACK: "1",
  },
  readyTimeoutMs: 20_000,
  defaultTimeoutMs: 10_000,
});

let receipt: Record<string, unknown> = {
  classification: "RUNTIME-FAILED",
  binary,
  sessionDir: driver.sessionDir,
};

try {
  await driver.request({ type: "show" }, { timeoutMs: 10_000 });
  await driver.waitForSettle({ timeoutMs: 10_000 });
  const beforeState = await driver.getState({ timeoutMs: 10_000 });
  const beforeElements = await driver.getElements({ target: { type: "main" } }, { timeoutMs: 10_000 });
  const initialLayout = await waitForControl(driver, expectedControls[0], true);
  const initialIds = allStrings(initialLayout);
  for (const id of expectedControls) {
    assert(initialIds.has(id), `runtime layout omitted stable control ${id}`);
  }

  const screenshot = await driver.captureScreenshot({
    target: { type: "main" },
    savePath: screenshotPath,
    timeoutMs: 10_000,
  });
  assert(!screenshot.error && screenshot.data, "runtime screenshot failed", screenshot);

  const actionBounds = componentBounds(
    initialLayout,
    "toast:ux16-runtime-a:action:open-local",
  );
  const actionResult = await driver.simulateGpuiClick(
    actionBounds.x + actionBounds.width / 2,
    actionBounds.y + actionBounds.height / 2,
    { target: { type: "main" }, timeoutMs: 10_000 },
  );
  assert(actionResult[1]?.success === true, "Toast action click did not dispatch", actionResult);
  await waitForControl(driver, "toast:ux16-runtime-a:root", false);
  const afterActionElements = await driver.getElements(
    { target: { type: "main" } },
    { timeoutMs: 10_000 },
  );

  const remainingLayout = await waitForControl(driver, "toast:ux16-runtime-b:root", true);
  const dismissBounds = componentBounds(remainingLayout, "toast:ux16-runtime-b:dismiss");
  const dismissResult = await driver.simulateGpuiClick(
    dismissBounds.x + dismissBounds.width / 2,
    dismissBounds.y + dismissBounds.height / 2,
    { target: { type: "main" }, timeoutMs: 10_000 },
  );
  assert(dismissResult[1]?.success === true, "Toast dismiss click did not dispatch", dismissResult);
  await waitForControl(driver, "toast:ux16-runtime-b:root", false);
  const afterDismissElements = await driver.getElements(
    { target: { type: "main" } },
    { timeoutMs: 10_000 },
  );
  const expectedFocus = ["input:filter"];
  assert(
    JSON.stringify(focusedSemanticIds(beforeElements)) === JSON.stringify(expectedFocus),
    "main filter was not focused before toast interaction",
    beforeElements,
  );
  assert(
    JSON.stringify(focusedSemanticIds(afterActionElements)) === JSON.stringify(expectedFocus),
    "toast action did not return focus to the prior main filter",
    afterActionElements,
  );
  assert(
    JSON.stringify(focusedSemanticIds(afterDismissElements)) === JSON.stringify(expectedFocus),
    "toast dismiss did not return focus to the prior main filter",
    afterDismissElements,
  );

  const afterState = await driver.getState({ timeoutMs: 10_000 });
  receipt = {
    classification: "RUNTIME-CONFIRMED",
    binary,
    sessionDir: driver.sessionDir,
    screenshot: {
      path: screenshotPath,
      width: screenshot.width,
      height: screenshot.height,
    },
    stableControls: expectedControls,
    duplicateMessage: "Duplicate status",
    duplicateActionLabel: "Open",
    runtimeActivation: {
      action: actionResult,
      dismiss: dismissResult,
    },
    focusContinuity: {
      before: beforeElements,
      afterAction: afterActionElements,
      afterDismiss: afterDismissElements,
    },
    state: { before: beforeState, after: afterState },
  };
} finally {
  await driver.close();
  const finalization = driver.finalization;
  const ownedPids = exactExecutablePids(binary);
  receipt.cleanup = {
    ...finalization,
    ownedProcessCount: ownedPids.length,
    ownedPids,
  };
  assert(finalization.processExited, "owned Script Kit process did not exit", finalization);
  assert(finalization.streamsDrained, "Driver streams did not drain", finalization);
  assert(finalization.logWriterClosed, "Driver log writer did not close", finalization);
  assert(ownedPids.length === 0, "owned Script Kit artifact still has running processes", ownedPids);
  await Bun.write(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
}

console.log(JSON.stringify(receipt, null, 2));
