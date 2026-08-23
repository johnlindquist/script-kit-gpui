#!/usr/bin/env bun
/**
 * State-first verification matrix for filterable launcher surfaces.
 *
 * The matrix is intentionally data-first: each migrated surface declares the
 * entry command, promptType, SurfaceKind, list semantic id, and filter text
 * that must keep `getState.visibleChoiceCount` aligned with `getElements`.
 *
 * Usage:
 *   bun scripts/agentic/filterable-surface-matrix.ts --list
 *   bun scripts/agentic/filterable-surface-matrix.ts --session aurp06-filterable
 *   bun scripts/agentic/filterable-surface-matrix.ts --case current-app-commands-visible-rows
 */

import { resolve } from "path";
import {
  assertNoninteractiveSubprocess,
  inspectionSessionCleanup,
  SessionOwnershipRegistry,
} from "../devtools/lib/operator-safety.ts";

const SCHEMA_VERSION = 1;
const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const sessionOwnership = new SessionOwnershipRegistry();

export type JsonObject = Record<string, unknown>;

export type MatrixAutomationTarget =
  | {
      type: "kind";
      kind: "main" | "actionsDialog" | "promptPopup";
      index: 0;
    }
  | {
      type: "id";
      id: string;
    };

export interface SafeSurfaceInteractions {
  filter: boolean;
  selectFirstVisibleChoice: boolean;
  submit: false;
}

export interface FilterableSurfaceMatrixEntry {
  id: string;
  surface: string;
  viewName: string;
  imageLibraryName: string;
  promptType: string;
  surfaceKind: string;
  listSemanticId: string;
  entryCommand: JsonObject;
  filterText: string;
  expectedElementChromeCount: number;
  target: MatrixAutomationTarget;
  safeInteractions: SafeSurfaceInteractions;
}

interface RpcEnvelope {
  status: "ok" | "error";
  response?: JsonObject;
  error?: { code?: string; message?: string };
}

interface StepReceipt {
  name: string;
  command: JsonObject;
  response: JsonObject;
}

export interface CountObservation {
  choiceCount: number;
  visibleChoiceCount: number;
  listCount: number;
  elementsTotalCount: number;
  selectedValue: string | null;
}

const MAIN_TARGET: MatrixAutomationTarget = {
  type: "kind",
  kind: "main",
  index: 0,
};
const SAFE_NON_SUBMITTING_INTERACTIONS: SafeSurfaceInteractions = {
  filter: true,
  selectFirstVisibleChoice: false,
  submit: false,
};

export const FILTERABLE_SURFACE_MATRIX: FilterableSurfaceMatrixEntry[] = [
  {
    id: "current-app-commands-visible-rows",
    surface: "currentAppCommands",
    viewName: "current-app-commands",
    imageLibraryName: "current-app-commands.png",
    promptType: "currentAppCommands",
    surfaceKind: "CurrentAppCommands",
    listSemanticId: "list:menu-commands",
    entryCommand: { type: "triggerBuiltin", name: "current-app-commands" },
    filterText: "workspace",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "clipboard-history-visible-rows",
    surface: "clipboardHistory",
    viewName: "clipboard-history",
    imageLibraryName: "clipboard-history.png",
    promptType: "clipboardHistory",
    surfaceKind: "ClipboardHistory",
    listSemanticId: "list:clipboard-history",
    entryCommand: { type: "triggerBuiltin", name: "clipboard-history" },
    filterText: "__aurp11_no_clipboard_match__",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "emoji-picker-visible-rows",
    surface: "emojiPicker",
    viewName: "emoji-picker",
    imageLibraryName: "emoji-picker.png",
    promptType: "emojiPicker",
    surfaceKind: "EmojiPicker",
    listSemanticId: "list:emoji-results",
    entryCommand: { type: "triggerBuiltin", name: "emoji" },
    filterText: "heart",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "app-launcher-visible-rows",
    surface: "appLauncher",
    viewName: "app-launcher",
    imageLibraryName: "app-launcher.png",
    promptType: "appLauncher",
    surfaceKind: "AppLauncher",
    listSemanticId: "list:apps",
    entryCommand: { type: "triggerBuiltin", name: "apps" },
    filterText: "__aurp16_no_app_match__",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "window-switcher-visible-rows",
    surface: "windowSwitcher",
    viewName: "window-switcher",
    imageLibraryName: "window-switcher.png",
    promptType: "windowSwitcher",
    surfaceKind: "WindowSwitcher",
    listSemanticId: "list:windows",
    entryCommand: { type: "triggerBuiltin", name: "window-switcher" },
    filterText: "__aurp16_no_window_match__",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "browser-tabs-visible-rows",
    surface: "browserTabs",
    viewName: "browser-tabs",
    imageLibraryName: "browser-tabs.png",
    promptType: "browserTabs",
    surfaceKind: "BrowserTabs",
    listSemanticId: "list:browser-tabs",
    entryCommand: { type: "triggerBuiltin", name: "browser-tabs" },
    filterText: "__aurp16_no_browser_tab_match__",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "process-manager-visible-rows",
    surface: "processManager",
    viewName: "process-manager",
    imageLibraryName: "process-manager.png",
    promptType: "processManager",
    surfaceKind: "ProcessManager",
    listSemanticId: "list:processes",
    entryCommand: { type: "triggerBuiltin", name: "process-manager" },
    filterText: "__aurp16_no_process_match__",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "settings-visible-rows",
    surface: "settings",
    viewName: "settings",
    imageLibraryName: "settings.png",
    promptType: "settings",
    surfaceKind: "Settings",
    listSemanticId: "list:settings",
    entryCommand: { type: "triggerBuiltin", builtinId: "builtin/settings" },
    filterText: "theme",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "kit-store-browse-visible-rows",
    surface: "kitStoreBrowse",
    viewName: "kit-store-browse",
    imageLibraryName: "kit-store-browse.png",
    promptType: "browseKits",
    surfaceKind: "KitStoreBrowse",
    listSemanticId: "list:kit-results",
    entryCommand: {
      type: "triggerBuiltin",
      builtinId: "builtin/browse-kit-store",
    },
    filterText: "script",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "kit-store-installed-visible-rows",
    surface: "kitStoreInstalled",
    viewName: "kit-store-installed",
    imageLibraryName: "kit-store-installed.png",
    promptType: "installedKits",
    surfaceKind: "KitStoreInstalled",
    listSemanticId: "list:installed-kits",
    entryCommand: {
      type: "triggerBuiltin",
      builtinId: "builtin/manage-installed-kits",
    },
    filterText: "",
    expectedElementChromeCount: 2,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "favorites-visible-rows",
    surface: "scriptList",
    viewName: "favorites",
    imageLibraryName: "generic-filterable-favorites.png",
    promptType: "favorites",
    surfaceKind: "GenericFilterableList",
    listSemanticId: "list:favorites",
    entryCommand: { type: "triggerBuiltin", builtinId: "builtin/favorites" },
    filterText: "__liquid_no_favorite_match__",
    expectedElementChromeCount: 3,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
  {
    id: "search-ai-presets-visible-rows",
    surface: "scriptList",
    viewName: "search-ai-presets",
    imageLibraryName: "generic-filterable-search-ai-presets.png",
    promptType: "searchAiPresets",
    surfaceKind: "GenericFilterableList",
    listSemanticId: "list:ai-presets",
    entryCommand: {
      type: "triggerBuiltin",
      builtinId: "builtin/search-ai-presets",
    },
    filterText: "coder",
    expectedElementChromeCount: 3,
    target: MAIN_TARGET,
    safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS,
  },
];

function hasFlag(flag: string): boolean {
  return process.argv.includes(flag);
}

function argValue(flag: string, fallback: string): string {
  const index = process.argv.indexOf(flag);
  if (index < 0) {
    return fallback;
  }
  return process.argv[index + 1] ?? fallback;
}

export function selectedCases(caseId: string): FilterableSurfaceMatrixEntry[] {
  if (caseId === "all") {
    return FILTERABLE_SURFACE_MATRIX;
  }
  const entry = FILTERABLE_SURFACE_MATRIX.find(
    (candidate) => candidate.id === caseId,
  );
  if (!entry) {
    throw new Error(`Unknown filterable surface matrix case: ${caseId}`);
  }
  return [entry];
}

export async function runTool(
  cmd: string[],
  label: string,
): Promise<JsonObject> {
  assertNoninteractiveSubprocess(cmd);
  const proc = Bun.spawn(cmd, {
    cwd: PROJECT_ROOT,
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = (await new Response(proc.stdout).text()).trim();
  const stderr = (await new Response(proc.stderr).text()).trim();
  const exitCode = await proc.exited;
  if (exitCode !== 0) {
    throw new Error(
      `${label} failed: ${stdout || stderr || `exit ${exitCode}`}`,
    );
  }
  try {
    return JSON.parse(stdout);
  } catch (error) {
    throw new Error(`${label} returned non-JSON output: ${stdout}`);
  }
}

export async function sessionStart(session: string): Promise<JsonObject> {
  inspectionSessionCleanup(session, null);
  if (session === "dev-watch") {
    throw new Error("Filterable surface matrix cannot claim the borrowed operator session");
  }
  const receipt = await runTool(
    ["bash", "scripts/agentic/session.sh", "start", session],
    "session.start",
  );
  sessionOwnership.rememberStart(session, receipt);
  return receipt;
}

export async function sessionStop(session: string): Promise<JsonObject> {
  const receipt = await runTool(sessionOwnership.stopCommand(session), "session.stop");
  sessionOwnership.release(session);
  return receipt;
}

export async function sendAndAwaitParse(
  session: string,
  command: JsonObject,
  timeoutMs: number,
): Promise<JsonObject> {
  const receipt = await runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "send",
      session,
      JSON.stringify(command),
      "--await-parse",
      "--timeout",
      String(timeoutMs),
    ],
    `send.${String(command.type)}`,
  );
  if (receipt.parseOutcome !== "parsed") {
    throw new Error(
      `Command ${String(command.type)} did not parse: ${JSON.stringify(receipt)}`,
    );
  }
  return receipt;
}

export async function sendCommand(
  session: string,
  command: JsonObject,
): Promise<JsonObject> {
  return runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "send",
      session,
      JSON.stringify(command),
    ],
    `send.${String(command.type)}`,
  );
}

export async function rpc(
  session: string,
  command: JsonObject,
  expect: string,
  timeoutMs: number,
): Promise<JsonObject> {
  const envelope = (await runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "rpc",
      session,
      JSON.stringify(command),
      "--expect",
      expect,
      "--timeout",
      String(timeoutMs),
    ],
    `rpc.${String(command.type)}`,
  )) as RpcEnvelope;
  if (envelope.status !== "ok" || !envelope.response) {
    throw new Error(
      `RPC ${String(command.type)} failed: ${JSON.stringify(envelope)}`,
    );
  }
  return envelope.response;
}

function numberField(source: JsonObject, key: string): number {
  const value = source[key];
  if (typeof value !== "number") {
    throw new Error(`Expected numeric ${key}, got ${JSON.stringify(value)}`);
  }
  return value;
}

function stringField(source: JsonObject, key: string): string {
  const value = source[key];
  if (typeof value !== "string") {
    throw new Error(`Expected string ${key}, got ${JSON.stringify(value)}`);
  }
  return value;
}

function objectField(source: JsonObject, key: string): JsonObject {
  const value = source[key];
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Expected object ${key}, got ${JSON.stringify(value)}`);
  }
  return value as JsonObject;
}

export function elementsFrom(response: JsonObject): JsonObject[] {
  const elements = response.elements;
  if (!Array.isArray(elements)) {
    throw new Error("elementsResult response must contain an elements array");
  }
  return elements as JsonObject[];
}

function listCountFromElements(
  response: JsonObject,
  listSemanticId: string,
): number {
  const list = elementsFrom(response).find(
    (element) => element.semanticId === listSemanticId,
  );
  if (!list) {
    throw new Error(`Missing list element ${listSemanticId}`);
  }
  const text = stringField(list, "text");
  const match = text.match(/^(\d+) items?$/);
  if (!match) {
    throw new Error(
      `List ${listSemanticId} text is not an item count: ${text}`,
    );
  }
  return Number(match[1]);
}

export function observeCounts(
  entry: FilterableSurfaceMatrixEntry,
  state: JsonObject,
  elements: JsonObject,
): CountObservation {
  const promptType = stringField(state, "promptType");
  if (promptType !== entry.promptType) {
    throw new Error(
      `${entry.id}: expected promptType ${entry.promptType}, got ${promptType}`,
    );
  }
  const surfaceContract = objectField(state, "surfaceContract");
  const surfaceKind = stringField(surfaceContract, "surfaceKind");
  if (surfaceKind !== entry.surfaceKind) {
    throw new Error(
      `${entry.id}: expected surfaceContract.surfaceKind ${entry.surfaceKind}, got ${surfaceKind}`,
    );
  }
  const automationSemanticSurface = stringField(
    surfaceContract,
    "automationSemanticSurface",
  );
  if (automationSemanticSurface !== entry.surface) {
    throw new Error(
      `${entry.id}: expected surfaceContract.automationSemanticSurface ${entry.surface}, got ${automationSemanticSurface}`,
    );
  }

  const choiceCount = numberField(state, "choiceCount");
  const visibleChoiceCount = numberField(state, "visibleChoiceCount");
  const listCount = listCountFromElements(elements, entry.listSemanticId);
  const elementsTotalCount = numberField(elements, "totalCount");

  if (visibleChoiceCount > choiceCount) {
    throw new Error(
      `${entry.id}: visibleChoiceCount ${visibleChoiceCount} exceeds choiceCount ${choiceCount}`,
    );
  }
  if (listCount !== visibleChoiceCount) {
    throw new Error(
      `${entry.id}: list count ${listCount} differs from visibleChoiceCount ${visibleChoiceCount}`,
    );
  }
  if (
    elementsTotalCount <
    visibleChoiceCount + entry.expectedElementChromeCount
  ) {
    throw new Error(
      `${entry.id}: elements totalCount ${elementsTotalCount} is smaller than visible rows plus required chrome`,
    );
  }

  return {
    choiceCount,
    visibleChoiceCount,
    listCount,
    elementsTotalCount,
    selectedValue:
      typeof state.selectedValue === "string" ? state.selectedValue : null,
  };
}

export async function enterFilterableSurface(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  timeoutMs: number,
): Promise<JsonObject> {
  try {
    return await sendAndAwaitParse(session, entry.entryCommand, timeoutMs);
  } catch (error) {
    if (entry.entryCommand.type !== "triggerBuiltin") {
      throw error;
    }
    const message = error instanceof Error ? error.message : String(error);
    if (!message.includes('"parseOutcome":"timeout"')) {
      throw error;
    }
    await waitForPromptType(session, entry, timeoutMs);
    return {
      schemaVersion: 1,
      status: "ok",
      sent: true,
      parseOutcome: "timeout",
      acceptedByState: true,
      expectedPromptType: entry.promptType,
    };
  }
}

export async function waitForPromptType(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  timeoutMs: number,
): Promise<JsonObject> {
  const deadline = Date.now() + timeoutMs;
  let lastState: JsonObject | null = null;
  while (Date.now() < deadline) {
    const command = {
      type: "getState",
      requestId: `${entry.id}-wait-state-${Date.now()}`,
    };
    const state = await rpc(
      session,
      command,
      "stateResult",
      Math.min(timeoutMs, 1000),
    );
    lastState = state;
    if (state.promptType === entry.promptType) {
      return state;
    }
    await Bun.sleep(50);
  }
  throw new Error(
    `${entry.id}: expected promptType ${entry.promptType}, last state ${JSON.stringify(lastState)}`,
  );
}

function stateInputText(state: JsonObject): string {
  if (typeof state.inputValue === "string") {
    return state.inputValue;
  }
  const decorations = state.filterInputDecorations;
  if (
    decorations &&
    typeof decorations === "object" &&
    !Array.isArray(decorations) &&
    typeof (decorations as JsonObject).text === "string"
  ) {
    return (decorations as JsonObject).text as string;
  }
  return "";
}

export async function setFilterAndWaitForState(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  text: string,
  requestId: string,
  timeoutMs: number,
): Promise<JsonObject> {
  const command = {
    type: "setFilter",
    text,
    requestId,
  };
  await sendCommand(session, command);

  const deadline = Date.now() + timeoutMs;
  let lastState: JsonObject | null = null;
  while (Date.now() < deadline) {
    const state = await rpc(
      session,
      {
        type: "getState",
        requestId: `${requestId}-state-${Date.now()}`,
        target: entry.target,
      },
      "stateResult",
      Math.min(timeoutMs, 1000),
    );
    lastState = state;
    if (stateInputText(state) === text) {
      return state;
    }
    await Bun.sleep(50);
  }
  throw new Error(
    `${entry.id}: setFilter ${JSON.stringify(text)} was not reflected in state, last state ${JSON.stringify(lastState)}`,
  );
}

export async function getStateAndElements(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  timeoutMs: number,
  requestLabel = "snapshot",
  targetOverride: MatrixAutomationTarget = entry.target,
): Promise<{
  state: JsonObject;
  elements: JsonObject;
  observation: CountObservation;
}> {
  const stateCommand = {
    type: "getState",
    requestId: `${entry.id}-${requestLabel}-state`,
    target: targetOverride,
  };
  const elementsCommand = {
    type: "getElements",
    requestId: `${entry.id}-${requestLabel}-elements`,
    target: targetOverride,
    limit: 500,
  };
  const state = await rpc(session, stateCommand, "stateResult", timeoutMs);
  const elements = await rpc(
    session,
    elementsCommand,
    "elementsResult",
    timeoutMs,
  );
  return {
    state,
    elements,
    observation: observeCounts(entry, state, elements),
  };
}

export async function runEntry(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  timeoutMs: number,
): Promise<JsonObject> {
  const steps: StepReceipt[] = [];

  await enterFilterableSurface(session, entry, timeoutMs);
  if (entry.filterText !== "") {
    await setFilterAndWaitForState(
      session,
      entry,
      "",
      `${entry.id}-reset-filter`,
      timeoutMs,
    );
  }

  const emptyStateCommand = {
    type: "getState",
    requestId: `${entry.id}-empty-state`,
    target: entry.target,
  };
  const emptyElementsCommand = {
    type: "getElements",
    requestId: `${entry.id}-empty-elements`,
    target: entry.target,
    limit: 500,
  };
  const emptySnapshot = await getStateAndElements(
    session,
    entry,
    timeoutMs,
    "empty",
  );
  const emptyState = emptySnapshot.state;
  const emptyElements = emptySnapshot.elements;
  steps.push({
    name: "empty.getState",
    command: emptyStateCommand,
    response: emptyState,
  });
  steps.push({
    name: "empty.getElements",
    command: emptyElementsCommand,
    response: emptyElements,
  });

  if (entry.filterText !== "") {
    await setFilterAndWaitForState(
      session,
      entry,
      entry.filterText,
      `${entry.id}-set-filter`,
      timeoutMs,
    );
  }

  const filteredStateCommand = {
    type: "getState",
    requestId: `${entry.id}-filtered-state`,
    target: entry.target,
  };
  const filteredElementsCommand = {
    type: "getElements",
    requestId: `${entry.id}-filtered-elements`,
    target: entry.target,
    limit: 500,
  };
  const filteredSnapshot = await getStateAndElements(
    session,
    entry,
    timeoutMs,
    "filtered",
  );
  const filteredState = filteredSnapshot.state;
  const filteredElements = filteredSnapshot.elements;
  steps.push({
    name: "filtered.getState",
    command: filteredStateCommand,
    response: filteredState,
  });
  steps.push({
    name: "filtered.getElements",
    command: filteredElementsCommand,
    response: filteredElements,
  });

  const empty = emptySnapshot.observation;
  const filtered = filteredSnapshot.observation;

  return {
    id: entry.id,
    status: "pass",
    surface: entry.surface,
    observations: { empty, filtered },
    steps,
  };
}

async function main(): Promise<void> {
  if (hasFlag("--list")) {
    process.stdout.write(
      `${JSON.stringify({
        schemaVersion: SCHEMA_VERSION,
        matrix: FILTERABLE_SURFACE_MATRIX,
      })}\n`,
    );
    return;
  }

  const session = argValue("--session", "filterable-surface-matrix");
  const timeoutMs = Number(argValue("--timeout", "5000"));
  const caseId = argValue("--case", "all");
  const keepSession = hasFlag("--keep-session");
  const cases = selectedCases(caseId);
  let startedSession = false;

  try {
    const startReceipt = await sessionStart(session);
    startedSession = startReceipt.resumed !== true;
    const caseReceipts = [];
    for (const entry of cases) {
      caseReceipts.push(await runEntry(session, entry, timeoutMs));
    }
    if (!keepSession && startedSession) {
      await sessionStop(session);
    }
    process.stdout.write(
      `${JSON.stringify({
        schemaVersion: SCHEMA_VERSION,
        status: "pass",
        session,
        startedSession,
        cases: caseReceipts,
      })}\n`,
    );
  } catch (error) {
    if (startedSession && !keepSession) {
      await sessionStop(session).catch(() => undefined);
    }
    process.stdout.write(
      `${JSON.stringify({
        schemaVersion: SCHEMA_VERSION,
        status: "fail",
        session,
        error: error instanceof Error ? error.message : String(error),
      })}\n`,
    );
    process.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
