#!/usr/bin/env bun
/** WP10 fail-closed native-list matrix. Attaches to an externally launched session. */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { Driver, type ActiveListScrollReceipt, type Json } from "../devtools/driver.ts";

export type FixtureProfile = "empty" | "short" | "long" | "mixed-height";

export interface NativeListMatrixEntry {
  surface: string;
  implementation: "variable_list" | "uniform_list" | "tracked_column";
  entryCommand: Json | null;
  fixtureProfiles: readonly FixtureProfile[];
  routeBlocker?: string;
}

const allProfiles = ["empty", "short", "long", "mixed-height"] as const;
const LAYOUT_NAMES: Record<string, string> = {
  script_list: "ScriptList",
  app_launcher: "AppLauncher",
  browser_tabs: "BrowserTabs",
  current_app_commands: "CurrentAppCommands",
  tips: "Tips",
  window_switcher: "WindowSwitcher",
  clipboard_history: "ClipboardHistory",
  process_manager: "ProcessManager",
  kit_store_browse: "BrowseKits",
  kit_store_installed: "InstalledKits",
  browser_history: "BrowserHistory",
  notes_browse: "NotesBrowse",
  dictation_history: "DictationHistory",
  agent_chat_history: "AgentChatHistory",
};

export const NATIVE_LIST_MATRIX: readonly NativeListMatrixEntry[] = [
  { surface: "script_list", implementation: "variable_list", entryCommand: { type: "triggerBuiltin", name: "mainList" }, fixtureProfiles: allProfiles, routeBlocker: "dev-style long/mixed fixture route is absent from the current working tree" },
  { surface: "app_launcher", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", name: "apps" }, fixtureProfiles: allProfiles },
  { surface: "browser_tabs", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", name: "browser-tabs" }, fixtureProfiles: allProfiles },
  { surface: "current_app_commands", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", name: "current-app-commands" }, fixtureProfiles: allProfiles },
  { surface: "tips", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", builtinId: "builtin/tips" }, fixtureProfiles: allProfiles },
  { surface: "window_switcher", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", name: "window-switcher" }, fixtureProfiles: allProfiles },
  { surface: "clipboard_history", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", name: "clipboard-history" }, fixtureProfiles: allProfiles },
  { surface: "process_manager", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", name: "process-manager" }, fixtureProfiles: allProfiles },
  { surface: "kit_store_browse", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", builtinId: "builtin/browse-kit-store" }, fixtureProfiles: allProfiles },
  { surface: "kit_store_installed", implementation: "uniform_list", entryCommand: { type: "triggerBuiltin", builtinId: "builtin/manage-installed-kits" }, fixtureProfiles: allProfiles },
  { surface: "browser_history", implementation: "tracked_column", entryCommand: null, fixtureProfiles: allProfiles, routeBlocker: "triggerBuiltin registry has no Browser History route" },
  { surface: "notes_browse", implementation: "tracked_column", entryCommand: null, fixtureProfiles: allProfiles, routeBlocker: "triggerBuiltin registry has no Notes Browse route" },
  { surface: "dictation_history", implementation: "tracked_column", entryCommand: { type: "triggerBuiltin", builtinId: "builtin/dictation-history" }, fixtureProfiles: allProfiles },
  { surface: "agent_chat_history", implementation: "tracked_column", entryCommand: null, fixtureProfiles: allProfiles, routeBlocker: "triggerBuiltin registry has no Agent Chat History route" },
] as const;

export const REQUIRED_NATIVE_LIST_FIELDS = [
  "surface", "implementation", "listKind", "selectedIndex", "selectedSemanticId",
  "hoveredIndex", "hoveredSemanticId", "hoverSuppressedUntilPointerMove",
  "focusedSemanticId", "logicalScrollTop", "scrollTopItem", "scrollTopOffsetItems",
  "scrollTopOffsetPx", "firstVisibleIndex", "lastVisibleIndexExclusive",
  "firstVisibleSemanticId", "lastVisibleSemanticId", "itemCount", "contentHeight",
  "viewportHeight", "safeViewportHeight", "maxScrollTop", "selectedRowVisible",
  "selectedRowWithinSafeViewport", "inputMode", "lastInteractionSource",
] as const;

function arg(name: string): string | null {
  const index = process.argv.indexOf(name);
  return index < 0 ? null : process.argv[index + 1] ?? null;
}

function currentHead(): string | null {
  const result = Bun.spawnSync(["git", "rev-parse", "HEAD"], { stdout: "pipe", stderr: "pipe" });
  return result.exitCode === 0 ? result.stdout.toString().trim() : null;
}

function compact(scroll: ActiveListScrollReceipt): Json {
  return Object.fromEntries(REQUIRED_NATIVE_LIST_FIELDS.map((field) => [field, scroll[field] ?? null]));
}

function missingFields(scroll: ActiveListScrollReceipt): string[] {
  return REQUIRED_NATIVE_LIST_FIELDS.filter((field) => !Object.prototype.hasOwnProperty.call(scroll, field));
}

function viewportChanged(before: ActiveListScrollReceipt, after: ActiveListScrollReceipt): boolean {
  return before.scrollTopItem !== after.scrollTopItem
    || before.scrollTopOffsetPx !== after.scrollTopOffsetPx
    || before.firstVisibleSemanticId !== after.firstVisibleSemanticId;
}

function invariant(before: ActiveListScrollReceipt, after: ActiveListScrollReceipt): Json {
  return {
    selectionStable: before.selectedSemanticId === after.selectedSemanticId,
    focusStable: before.focusedSemanticId === after.focusedSemanticId,
    inputModeStable: before.inputMode === after.inputMode,
    viewportChanged: viewportChanged(before, after),
  };
}

async function settleActive(driver: Awaited<ReturnType<typeof Driver.attach>>) {
  return driver.waitForSettle({
    samples: 2,
    timeoutMs: 5_000,
    probe: () => driver.getActiveListScroll(),
  });
}

async function exerciseSurface(
  driver: Awaited<ReturnType<typeof Driver.attach>>,
  entry: NativeListMatrixEntry,
): Promise<Json> {
  if (!entry.entryCommand) {
    return { surface: entry.surface, classification: "blocked-by-missing-primitive", missingPrimitive: `deterministicRoute:${entry.surface}`, detail: entry.routeBlocker };
  }
  driver.send(entry.entryCommand);
  const opened = await settleActive(driver);
  if (!opened.settled) {
    return { surface: entry.surface, classification: "blocked-by-session-lifecycle", opened };
  }
  const before = await driver.getActiveListScroll();
  const missing = missingFields(before);
  if (before.surface !== entry.surface || before.implementation !== entry.implementation || missing.length > 0) {
    return {
      surface: entry.surface,
      classification: "blocked-by-missing-primitive",
      missingPrimitive: before.surface !== entry.surface ? `deterministicRoute:${entry.surface}` : "activeListScroll",
      expected: { surface: entry.surface, implementation: entry.implementation },
      observed: compact(before),
      missingFields: missing,
    };
  }

  const result: Json = {
    surface: entry.surface,
    fixtureProfiles: entry.fixtureProfiles,
    initial: compact(before),
    dispatchReceipts: [],
    transitions: [],
    blockers: [],
  };
  if (entry.routeBlocker) result.blockers.push({ missingPrimitive: `fixtureRoute:${entry.surface}`, detail: entry.routeBlocker });
  if (before.itemCount < 12 || !(Number(before.maxScrollTop) > 0)) {
    result.blockers.push({ missingPrimitive: `deterministicLongFixture:${entry.surface}`, observedItemCount: before.itemCount, maxScrollTop: before.maxScrollTop ?? null });
    result.classification = "blocked-by-missing-primitive";
    return result;
  }

  const layout = await driver.getLayoutInfo({ target: { type: "main" } });
  const components = Array.isArray(layout.components) ? layout.components : [];
  const list = components.find((component: Json) => component.name === LAYOUT_NAMES[entry.surface]);
  const bounds = list?.bounds as Json | undefined;
  if (!bounds || ![bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite)) {
    result.blockers.push({ missingPrimitive: `activeListViewportBounds:${entry.surface}` });
    result.classification = "blocked-by-missing-primitive";
    return result;
  }
  const point = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  const target = { type: "main" };
  const direct = [
    { phase: "started", directPhase: "began", deltaY: 0, timestampSeconds: 1 },
    { phase: "moved", directPhase: "changed", deltaY: -7.5, timestampSeconds: 1.01 },
    { phase: "ended", directPhase: "ended", deltaY: 0, timestampSeconds: 1.02 },
  ] as const;
  for (const event of direct) {
    result.dispatchReceipts.push(await driver.simulateGpuiScrollWheel({ ...point, deltaX: 0, momentumPhase: "none", ...event }, { target }));
  }
  await settleActive(driver);
  const afterDirect = await driver.getActiveListScroll();
  result.transitions.push({ source: "direct-pixel", before: compact(before), after: compact(afterDirect), assertions: invariant(before, afterDirect) });

  const momentum = [
    { phase: "started", momentumPhase: "began", deltaY: 0, timestampSeconds: 2 },
    { phase: "moved", momentumPhase: "changed", deltaY: -9.25, timestampSeconds: 2.01 },
    { phase: "ended", momentumPhase: "ended", deltaY: 0, timestampSeconds: 2.02 },
  ] as const;
  for (const event of momentum) {
    result.dispatchReceipts.push(await driver.simulateGpuiScrollWheel({ ...point, deltaX: 0, directPhase: "none", ...event }, { target }));
  }
  await settleActive(driver);
  const afterMomentum = await driver.getActiveListScroll();
  result.transitions.push({ source: "momentum", before: compact(afterDirect), after: compact(afterMomentum), assertions: invariant(afterDirect, afterMomentum) });

  const selectedBeforePointer = afterMomentum.selectedSemanticId;
  result.dispatchReceipts.push(await driver.simulateGpuiEvent({ type: "mouseMove", ...point }, { target }));
  await settleActive(driver);
  const afterPointer = await driver.getActiveListScroll();
  result.transitions.push({ source: "pointer-move", after: compact(afterPointer), assertions: { selectionStable: selectedBeforePointer === afterPointer.selectedSemanticId } });

  result.dispatchReceipts.push(await driver.simulateGpuiEvent({ type: "keyDown", key: "down", modifiers: [] }, { target }));
  await settleActive(driver);
  const afterDown = await driver.getActiveListScroll();
  result.transitions.push({ source: "keyboard-down", after: compact(afterDown), assertions: { selected: afterDown.selectedSemanticId !== afterPointer.selectedSemanticId, revealed: afterDown.selectedRowWithinSafeViewport === true } });
  result.dispatchReceipts.push(await driver.simulateGpuiEvent({ type: "keyDown", key: "end", modifiers: [] }, { target }));
  await settleActive(driver);
  const afterEnd = await driver.getActiveListScroll();
  result.transitions.push({ source: "keyboard-end", after: compact(afterEnd), assertions: { endpoint: afterEnd.selectedIndex === afterEnd.itemCount - 1, revealed: afterEnd.selectedRowWithinSafeViewport === true } });

  await driver.setFilterAndWait("__wp10_no_match__");
  const filtered = await driver.getActiveListScroll();
  driver.send(entry.entryCommand);
  await settleActive(driver);
  const refreshed = await driver.getActiveListScroll();
  result.transitions.push({ source: "filter-refresh", filtered: compact(filtered), refreshed: compact(refreshed) });

  result.blockers.push({ missingPrimitive: "lineDeltaRuntimeTransport", coveredBy: "tracked_scroll_column_behavior_tests" });
  result.blockers.push({ missingPrimitive: `scrollbarThumbBoundsAndDrag:${entry.surface}` });
  result.blockers.push({ missingPrimitive: `rowBoundsForClick:${entry.surface}` });
  result.classification = result.blockers.length > 0 ? "blocked-by-missing-primitive" : "reproduced";
  return result;
}

async function main() {
  const session = arg("--session");
  if (!session || !process.argv.includes("--all-migrated-surfaces")) {
    throw new Error("Usage: bun scripts/agentic/builtin-native-scroll-probe.ts --session <name> --all-migrated-surfaces [--output <json>]");
  }
  const output = resolve(arg("--output") ?? ".notes/oracle/native-mouse-scroll-behavior/wp10-runtime-receipt.json");
  const receipt: Json = {
    workPackage: 10,
    requestedBaseCommit: "3d6289abf",
    observedHeadWhenReceiptGenerated: currentHead(),
    session,
    classification: "blocked-by-session-lifecycle",
    matrix: NATIVE_LIST_MATRIX,
    behaviorCoverage: {
      deterministicProfiles: allProfiles,
      transports: ["direct-pixel", "line", "direct-terminal", "momentum", "scrollbar"],
      interactions: ["stationary-pointer", "pointer-move", "click", "keyboard-up-down-endpoints", "filter", "refresh"],
      lineCoverageOwner: "tracked_scroll_column_behavior_tests",
    },
    surfaces: [],
    cleanup: null,
  };
  let driver: Awaited<ReturnType<typeof Driver.attach>> | null = null;
  try {
    driver = await Driver.attach({ session, defaultTimeoutMs: 10_000 });
    driver.send({ type: "show" });
    await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
    for (const entry of NATIVE_LIST_MATRIX) receipt.surfaces.push(await exerciseSurface(driver, entry));
    receipt.classification = receipt.surfaces.every((surface: Json) => surface.classification === "reproduced")
      ? "reproduced"
      : "blocked-by-missing-primitive";
  } catch (error) {
    receipt.error = error instanceof Error ? error.message : String(error);
    receipt.surfaces = NATIVE_LIST_MATRIX.map((entry) => ({
      surface: entry.surface,
      classification: "blocked-by-session-lifecycle",
      missingPrimitive: "runningExternalDriverSession",
      routeBlocker: entry.routeBlocker ?? null,
    }));
  } finally {
    if (driver) {
      driver.send({ type: "hide" });
      try {
        receipt.cleanup = await driver.waitForState({ windowVisible: false }, { timeoutMs: 5_000 });
      } catch (error) {
        receipt.cleanup = { classification: "blocked-by-session-lifecycle", error: error instanceof Error ? error.message : String(error) };
      }
      await driver.close();
    }
    mkdirSync(dirname(output), { recursive: true });
    writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
    console.log(JSON.stringify(receipt, null, 2));
  }
}

if (import.meta.main) await main();
