#!/usr/bin/env bun
import { Driver, type Json } from "../devtools/driver";

type Bounds = { x: number; y: number; width: number; height: number };
type ObjectJson = Record<string, unknown>;
type Check = { name: string; pass: boolean; detail: Json };

const checks: Check[] = [];
const surfaces: Record<string, Json> = {};
const embeddedWrappedText = "parity ".repeat(28);
const detachedWrappedText = "parity ".repeat(32);

function asObjects(value: unknown): ObjectJson[] {
  return Array.isArray(value)
    ? value.filter(
        (entry): entry is ObjectJson =>
          typeof entry === "object" && entry !== null && !Array.isArray(entry),
      )
    : [];
}

function check(name: string, pass: boolean, detail: Json = {}): void {
  checks.push({ name, pass, detail });
}

function near(a: number, b: number, tolerance = 1): boolean {
  return (
    Number.isFinite(a) && Number.isFinite(b) && Math.abs(a - b) <= tolerance
  );
}
function samePaintedEdges(
  modeled: Bounds,
  painted: Bounds,
  tolerance = 1,
): boolean {
  return (
    near(modeled.x, painted.x, tolerance) &&
    near(modeled.y, painted.y, tolerance) &&
    near(modeled.x + modeled.width, painted.x + painted.width, tolerance) &&
    near(modeled.y + modeled.height, painted.y + painted.height, tolerance)
  );
}

function bounds(value: unknown): Bounds | null {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    return null;
  const candidate = value as ObjectJson;
  const result = {
    x: Number(candidate.x),
    y: Number(candidate.y),
    width: Number(candidate.width),
    height: Number(candidate.height),
  };
  return Object.values(result).every(Number.isFinite) ? result : null;
}

function component(layout: Json, name: string): ObjectJson | null {
  const matches = asObjects(layout.components).filter(
    (entry) => entry.name === name,
  );
  return matches.length === 1 ? matches[0] : null;
}

function componentBounds(layout: Json, name: string): Bounds | null {
  return bounds(component(layout, name)?.bounds);
}

function paintBounds(layout: Json, selector: string): Bounds | null {
  const matches = asObjects(layout.components).filter(
    (entry) => entry.name === selector,
  );
  check(`${selector}-unique`, matches.length === 1, {
    selector,
    count: matches.length,
  });
  const entry = matches.length === 1 ? matches[0] : null;
  if (entry === null) return null;
  check(
    `${selector}-paint-time-window-measurement`,
    entry.measurementProvenance === "paint-time" &&
      entry.coordinateSpace === "window" &&
      Number.isSafeInteger(entry.measurementFrameGeneration) &&
      Number(entry.measurementFrameGeneration) > 0,
    {
      selector,
      measurementProvenance: entry.measurementProvenance ?? null,
      coordinateSpace: entry.coordinateSpace ?? null,
      measurementFrameGeneration: entry.measurementFrameGeneration ?? null,
    },
  );
  return bounds(entry.bounds);
}

function assertPositive(
  label: string,
  value: Bounds | null,
): asserts value is Bounds {
  const pass =
    value !== null &&
    value.width > 0 &&
    value.height > 0 &&
    [value.x, value.y, value.width, value.height].every(Number.isFinite);
  check(`${label}-positive-bounds`, pass, { bounds: value });
  if (!pass) throw new Error(`${label} has no positive bounds`);
}

function assertModelPaint(
  label: string,
  modeled: Bounds | null,
  painted: Bounds | null,
  tolerance = 1,
): void {
  assertPositive(`${label}-modeled`, modeled);
  assertPositive(`${label}-painted`, painted);
  check(
    `${label}-model-equals-paint`,
    samePaintedEdges(modeled, painted, tolerance),
    {
      modeled,
      painted,
      tolerancePx: tolerance,
      comparison: "window-space edges",
    },
  );
}

async function waitForSurface(driver: Driver, expected: string): Promise<void> {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    const state = await driver.getState({ timeoutMs: 10_000 });
    if (state.surfaceContract?.surfaceKind === expected) {
      const settled = await driver.waitForSettle({ timeoutMs: 10_000 });
      check(`${expected}-settled`, settled.settled, {
        elapsedMs: settled.elapsedMs,
        probes: settled.probes,
      });
      return;
    }
    await Bun.sleep(50);
  }
  throw new Error(`expected main surface ${expected}`);
}

async function exactTarget(driver: Driver, kind: string): Promise<Json> {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    const receipt = (await driver.listAutomationWindows()) as ObjectJson;
    const matches = asObjects(receipt.windows).filter(
      (window) => window.kind === kind && window.visible === true,
    );
    if (matches.length === 1) {
      const match = matches[0];
      return typeof match.generation === "number"
        ? { type: "instance", id: match.id, generation: match.generation }
        : { type: "id", id: match.id };
    }
    await Bun.sleep(50);
  }
  throw new Error(`expected exactly one visible ${kind} target`);
}

async function waitForSettledComponentGeometry(
  driver: Driver,
  target: Json | null,
  componentName: string,
  paintSelector: string,
  expectedHeight: number,
  requirePaint = true,
): Promise<Json> {
  const deadline = Date.now() + 15_000;
  let latest: Json = {};
  while (Date.now() < deadline) {
    latest = await driver.getLayoutInfo(target === null ? {} : { target }, {
      timeoutMs: 10_000,
    });
    const modeled = componentBounds(latest, componentName);
    const painted = componentBounds(latest, paintSelector);
    if (
      modeled !== null &&
      (!requirePaint || painted !== null) &&
      near(modeled.height, expectedHeight) &&
      (!requirePaint || samePaintedEdges(modeled, painted!))
    ) {
      return latest;
    }
    await Bun.sleep(50);
  }
  return latest;
}

async function setComposer(
  driver: Driver,
  text: string,
  target: Json | null = null,
): Promise<Json> {
  if (target !== null) {
    return driver.request(
      {
        type: "batch",
        target,
        commands: [{ type: "setInput", text }],
        options: { stopOnError: true, timeout: 3000 },
        trace: "onFailure",
      },
      { expect: "batchResult", timeoutMs: 10_000 },
    );
  }
  return driver.request(
    { type: "setAgentChatInput", text, submit: false },
    {
      expect: "externalCommandResult",
      timeoutMs: 10_000,
    },
  );
}

const receipt: Json = {
  schemaVersion: 1,
  probe: "agent-chat-composer-parity",
  status: "red",
  surfaces,
  checks,
};

let driver: Driver | null = null;
try {
  driver = await Driver.launch({
    sandboxHome: true,
    sharedModels: false,
    sessionName: "agent-chat-composer-parity",
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 10_000,
    env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
  });

  await driver
    .request(
      { type: "show" },
      { expect: "externalCommandResult", timeoutMs: 10_000 },
    )
    .catch(() => driver?.send({ type: "show" }));
  await waitForSurface(driver, "ScriptList");
  const mainLayout = await driver.getLayoutInfo({}, { timeoutMs: 10_000 });
  const mainModel = componentBounds(mainLayout, "MainViewInput");
  const mainPaint = paintBounds(mainLayout, "main-view-input-shell");
  assertModelPaint("main-menu-input", mainModel, mainPaint);
  const canonicalHeight = mainModel.height;
  surfaces.mainMenu = { modeled: mainModel, painted: mainPaint };

  driver.send({
    type: "hide",
    requestId: `agent-chat-composer-parity-focused-reset-${Date.now()}`,
  });
  const hiddenBeforeFocused = await driver.waitForState(
    { windowVisible: false },
    { timeoutMs: 10_000 },
  );
  check(
    "focused-text-fixture-reveal-starts-from-hidden-main-window",
    hiddenBeforeFocused.success === true,
    { waitForState: hiddenBeforeFocused },
  );

  const focusedOpen = await driver.request(
    {
      type: "openFocusedTextAgentChatWithMockData",
      text: "Focused text fixture",
      instruction: "",
    },
    { expect: "focusedTextAgentChatFixtureOpenResult", timeoutMs: 15_000 },
  );
  check(
    "focused-text-fixture-opened",
    focusedOpen.ok === true && focusedOpen.submitted === false,
    focusedOpen,
  );
  await waitForSurface(driver, "AgentChat");
  const focusedLayout = await waitForSettledComponentGeometry(
    driver,
    { type: "main" },
    "FocusedTextMiniInputShell",
    "focused-text-mini-input-shell",
    canonicalHeight,
  );
  const focusedRow = componentBounds(focusedLayout, "FocusedTextMiniInputRow");
  const focusedModel = componentBounds(
    focusedLayout,
    "FocusedTextMiniInputShell",
  );
  const focusedPaint = paintBounds(
    focusedLayout,
    "focused-text-mini-input-shell",
  );
  assertPositive("focused-text-input-row", focusedRow);
  assertModelPaint("focused-text-input-shell", focusedModel, focusedPaint, 2);
  check(
    "focused-text-nested-shell-keeps-canonical-x-and-one-line-height",
    near(focusedModel.x, mainModel.x) &&
      near(focusedModel.width, mainModel.width) &&
      near(focusedModel.height, canonicalHeight),
    {
      mainMenu: mainModel,
      focusedText: focusedModel,
      row: focusedRow,
      tolerancePx: 1,
    },
  );
  check(
    "focused-text-painted-shell-keeps-main-menu-horizontal-edges-and-height",
    near(focusedPaint.x, mainPaint.x) &&
      near(
        focusedPaint.x + focusedPaint.width,
        mainPaint.x + mainPaint.width,
      ) &&
      near(focusedPaint.height, mainPaint.height),
    {
      mainMenu: mainPaint,
      focusedText: focusedPaint,
      tolerancePx: 1,
      comparison: "paint-time window-space edges",
    },
  );
  check(
    "focused-text-canonical-shell-is-centered-in-intentional-compact-slot",
    near(
      focusedModel.y,
      focusedRow.y + (focusedRow.height - canonicalHeight) / 2,
    ),
    { shell: focusedModel, row: focusedRow, canonicalHeight, tolerancePx: 1 },
  );
  surfaces.focusedTextMini = {
    row: focusedRow,
    modeled: focusedModel,
    painted: focusedPaint,
  };
  const focusedDismiss = await driver.simulateGpuiEvent(
    { type: "keyDown", key: "escape", modifiers: [] },
    { target: { type: "kind", kind: "main" }, timeoutMs: 10_000 },
  );
  check(
    "focused-text-dismissed-via-main-escape",
    (focusedDismiss as ObjectJson).success === true,
    focusedDismiss as ObjectJson,
  );
  await waitForSurface(driver, "ScriptList");
  await driver.send({ type: "openAiWithMockData" });
  check("embedded-fixture-opened", true, {
    command: "openAiWithMockData",
  });
  await waitForSurface(driver, "AgentChat");
  await setComposer(driver, "one line");
  const embeddedLayout = await waitForSettledComponentGeometry(
    driver,
    null,
    "MainViewInput",
    "main-view-input-shell",
    canonicalHeight,
  );
  const embeddedModel = componentBounds(embeddedLayout, "MainViewInput");
  const embeddedPaint = paintBounds(embeddedLayout, "main-view-input-shell");
  assertModelPaint("embedded-input", embeddedModel, embeddedPaint);
  check(
    "embedded-one-line-matches-main-menu-header-geometry",
    near(embeddedModel.x, mainModel.x) &&
      near(embeddedModel.y, mainModel.y) &&
      near(embeddedModel.width, mainModel.width) &&
      near(embeddedModel.height, canonicalHeight),
    { mainMenu: mainModel, embedded: embeddedModel, tolerancePx: 1 },
  );
  await setComposer(driver, embeddedWrappedText);
  const embeddedWrappedLayout = await waitForSettledComponentGeometry(
    driver,
    null,
    "MainViewInput",
    "main-view-input-shell",
    canonicalHeight * 3,
  );
  const embeddedWrappedModel = componentBounds(
    embeddedWrappedLayout,
    "MainViewInput",
  );
  const embeddedWrappedPaint = paintBounds(
    embeddedWrappedLayout,
    "main-view-input-shell",
  );
  assertModelPaint(
    "embedded-wrapped-input",
    embeddedWrappedModel,
    embeddedWrappedPaint,
  );
  check(
    "embedded-multiline-growth-uses-whole-canonical-increments",
    near(embeddedWrappedModel.height - embeddedModel.height, canonicalHeight * 2),
    {
      oneLineHeight: embeddedModel.height,
      wrappedHeight: embeddedWrappedModel.height,
      canonicalIncrement: canonicalHeight,
    },
  );

  await driver.send({ type: "openAgentChatDetachedFixture" });
  check("detached-fixture-opened", true, {
    command: "openAgentChatDetachedFixture",
  });
  const detachedTarget = await exactTarget(driver, "agentChatDetached");
  await setComposer(driver, "one line", detachedTarget);
  const detachedLayout = await waitForSettledComponentGeometry(
    driver,
    detachedTarget,
    "AgentChatComposerBar",
    "main-view-input-shell",
    canonicalHeight,
    false,
  );
  const detachedModel = componentBounds(detachedLayout, "AgentChatComposerBar");
  assertPositive("detached-input-modeled", detachedModel);
  const detachedElements = asObjects(
    (
      (await driver.getElements(
        { target: detachedTarget, limit: 200 },
        { timeoutMs: 10_000 },
      )) as ObjectJson
    ).elements,
  );
  const detachedComposer = detachedElements.filter(
    (entry) => entry.semanticId === "input:agent_chat-composer",
  );
  check(
    "detached-targeted-composer-is-present-and-focused",
    detachedComposer.length === 1 && detachedComposer[0].focused === true,
    {
      target: detachedTarget,
      count: detachedComposer.length,
      focused: detachedComposer[0]?.focused ?? null,
    },
  );
  // Target-scoped detached layouts expose the rendered view's model and
  // semantic element tree; paint-time selector measurements are main-window
  // only, so detached parity is bound to those two independent receipts.
  const mainWindowWidth = Number(mainLayout.windowWidth);
  const detachedWindowWidth = Number(detachedLayout.windowWidth);
  check(
    "detached-one-line-matches-main-menu-header-insets-origin-and-height",
    near(detachedModel.x, mainModel.x) &&
      near(detachedModel.y, mainModel.y) &&
      near(
        detachedWindowWidth - detachedModel.width,
        mainWindowWidth - mainModel.width,
      ) &&
      near(detachedModel.height, canonicalHeight),
    {
      mainMenu: mainModel,
      detached: detachedModel,
      mainWindowWidth,
      detachedWindowWidth,
      tolerancePx: 1,
    },
  );
  await setComposer(driver, detachedWrappedText, detachedTarget);
  const detachedWrappedLayout = await waitForSettledComponentGeometry(
    driver,
    detachedTarget,
    "AgentChatComposerBar",
    "main-view-input-shell",
    canonicalHeight * 3,
    false,
  );
  const detachedWrappedModel = componentBounds(
    detachedWrappedLayout,
    "AgentChatComposerBar",
  );
  assertPositive("detached-wrapped-input-modeled", detachedWrappedModel);
  const detachedGrowth = detachedWrappedModel.height - detachedModel.height;
  check(
    "detached-multiline-growth-uses-whole-canonical-increments",
    detachedGrowth > 0 &&
      near(detachedGrowth % canonicalHeight, 0),
    {
      oneLineHeight: detachedModel.height,
      wrappedHeight: detachedWrappedModel.height,
      canonicalIncrement: canonicalHeight,
      growth: detachedGrowth,
    },
  );
  surfaces.detached = {
    target: detachedTarget,
    projection: "target-scoped-model-plus-semantic-focus",
    oneLine: { modeled: detachedModel },
    multiline: { modeled: detachedWrappedModel },
  };
} catch (error) {
  receipt.error = String(error);
  check("probe-completed", false, { error: String(error) });
} finally {
  if (driver !== null) {
    try {
      await driver.close();
      check("driver-closed", true);
    } catch (error) {
      check("driver-closed", false, { error: String(error) });
    }
  }
}

receipt.status =
  checks.length > 0 && checks.every((entry) => entry.pass) ? "green" : "red";
console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.status === "green" ? 0 : 1);
