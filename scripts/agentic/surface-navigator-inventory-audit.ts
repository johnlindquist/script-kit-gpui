#!/usr/bin/env bun
/**
 * Runtime audit that keeps the Surface Navigator matrices aligned with live
 * screenshotable surface contracts.
 */

import {
  FILTERABLE_SURFACE_MATRIX,
  getStateAndElements,
  selectedCases,
  sendAndAwaitParse,
  sessionStart,
  sessionStop,
  waitForPromptType,
  type FilterableSurfaceMatrixEntry,
  type JsonObject,
} from "./filterable-surface-matrix";
import { ATTACHED_POPUP_SURFACE_MATRIX } from "./attached-popup-surface-matrix";

const DEFAULT_SESSION = "surface-navigator-inventory";
const TIMEOUT_MS = 5000;

interface LiveSurfaceProbe {
  id: string;
  command: JsonObject;
  expectedPromptType: string;
  screenshotable: boolean;
  notes: string;
}

export const SURFACE_NAVIGATOR_EXEMPTIONS: Record<string, string> = {
  fileSearch:
    "File Search has mini/full portal semantics and is captured by file-search specific proofs before promotion.",
  windowSwitcher:
    "Window Switcher depends on host window inventory and is intentionally kept out of the screenshot library matrix.",
  sdkReference:
    "SDK Reference is a split documentation browser with resource-specific proof coverage.",
  quickTerminal:
    "Quick Terminal is PTY/canvas-like child content, not a filterable screenshot matrix target.",
  webcam:
    "Webcam requires camera permission and media-state proof rather than generic screenshot capture.",
  scriptList:
    "Generic scriptList is a host/default semantic surface, not a dedicated screenshotable built-in surface.",
};

const LIVE_TRIGGER_BUILTINS: LiveSurfaceProbe[] = [
  {
    id: "current-app-commands",
    command: { type: "triggerBuiltin", name: "current-app-commands" },
    expectedPromptType: "currentAppCommands",
    screenshotable: true,
    notes: "mini filterable list",
  },
  {
    id: "clipboard-history",
    command: { type: "triggerBuiltin", name: "clipboard-history" },
    expectedPromptType: "clipboardHistory",
    screenshotable: true,
    notes: "filterable list with preview",
  },
  {
    id: "emoji",
    command: { type: "triggerBuiltin", name: "emoji" },
    expectedPromptType: "emojiPicker",
    screenshotable: true,
    notes: "mini filterable list",
  },
  {
    id: "apps",
    command: { type: "triggerBuiltin", name: "apps" },
    expectedPromptType: "appLauncher",
    screenshotable: true,
    notes: "mini filterable list",
  },
  {
    id: "process-manager",
    command: { type: "triggerBuiltin", name: "process-manager" },
    expectedPromptType: "processManager",
    screenshotable: true,
    notes: "mini filterable list",
  },
  {
    id: "settings",
    command: { type: "triggerBuiltin", builtinId: "builtin/settings" },
    expectedPromptType: "settings",
    screenshotable: true,
    notes: "mini filterable list",
  },
  {
    id: "browse-kit-store",
    command: { type: "triggerBuiltin", builtinId: "builtin/browse-kit-store" },
    expectedPromptType: "browseKits",
    screenshotable: true,
    notes: "Kit Store browse list",
  },
  {
    id: "manage-installed-kits",
    command: { type: "triggerBuiltin", builtinId: "builtin/manage-installed-kits" },
    expectedPromptType: "installedKits",
    screenshotable: true,
    notes: "Kit Store installed list",
  },
  {
    id: "file-search",
    command: { type: "triggerBuiltin", name: "file-search" },
    expectedPromptType: "fileSearch",
    screenshotable: false,
    notes: "portal-specific surface",
  },
  {
    id: "window-switcher",
    command: { type: "triggerBuiltin", name: "window-switcher" },
    expectedPromptType: "windowSwitcher",
    screenshotable: false,
    notes: "inventory-dependent surface",
  },
  {
    id: "sdk-reference",
    command: { type: "triggerBuiltin", builtinId: "builtin/sdk-reference" },
    expectedPromptType: "sdkReference",
    screenshotable: false,
    notes: "resource browser",
  },
];

function surfaceContractFrom(state: JsonObject): JsonObject {
  const contract = state.surfaceContract;
  if (!contract || typeof contract !== "object" || Array.isArray(contract)) {
    throw new Error(`stateResult missing surfaceContract: ${JSON.stringify(state)}`);
  }
  return contract as JsonObject;
}

function contractSurface(state: JsonObject): string {
  const value = surfaceContractFrom(state).automationSemanticSurface;
  if (typeof value !== "string") {
    throw new Error(`surfaceContract missing automationSemanticSurface`);
  }
  return value;
}

function warningsFrom(elements: JsonObject): string[] {
  return Array.isArray(elements.warnings) ? (elements.warnings as string[]) : [];
}

function matrixSurfaces(): Set<string> {
  return new Set([
    ...FILTERABLE_SURFACE_MATRIX.map((entry) => entry.surface),
    ...ATTACHED_POPUP_SURFACE_MATRIX.map((entry) => entry.viewName),
  ]);
}

async function probeLiveSurface(
  session: string,
  probe: LiveSurfaceProbe,
): Promise<{ probe: LiveSurfaceProbe; state: JsonObject; surface: string }> {
  await sendAndAwaitParse(session, probe.command, TIMEOUT_MS);
  const deadline = Date.now() + TIMEOUT_MS;
  let state: JsonObject | null = null;
  while (Date.now() < deadline) {
    const response = await getState(session);
    state = response;
    if (response.promptType === probe.expectedPromptType) {
      return { probe, state: response, surface: contractSurface(response) };
    }
    await Bun.sleep(50);
  }
  throw new Error(
    `${probe.id}: expected promptType ${probe.expectedPromptType}, got ${JSON.stringify(state)}`,
  );
}

async function getState(session: string): Promise<JsonObject> {
  const { rpc } = await import("./filterable-surface-matrix");
  return rpc(
    session,
    { type: "getState", requestId: `inventory-state-${Date.now()}` },
    "stateResult",
    TIMEOUT_MS,
  );
}

async function auditMatrixCase(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
): Promise<{ id: string; surface: string; warnings: string[] }> {
  await sendAndAwaitParse(session, entry.entryCommand, TIMEOUT_MS);
  await waitForPromptType(session, entry, TIMEOUT_MS);
  const snapshot = await getStateAndElements(session, entry, TIMEOUT_MS, "inventory");
  return {
    id: entry.id,
    surface: entry.surface,
    warnings: warningsFrom(snapshot.elements),
  };
}

async function main(): Promise<void> {
  const session = process.argv.includes("--session")
    ? process.argv[process.argv.indexOf("--session") + 1]
    : DEFAULT_SESSION;
  const keepSession = process.argv.includes("--keep-session");
  const json = process.argv.includes("--json");
  const failures: string[] = [];
  const live: { id: string; surface: string; screenshotable: boolean; exempt: boolean }[] = [];
  const matrix = matrixSurfaces();

  await sessionStart(session);
  try {
    for (const probe of LIVE_TRIGGER_BUILTINS) {
      const result = await probeLiveSurface(session, probe);
      const exempt = result.surface in SURFACE_NAVIGATOR_EXEMPTIONS;
      live.push({
        id: probe.id,
        surface: result.surface,
        screenshotable: probe.screenshotable,
        exempt,
      });
      if (probe.screenshotable && !matrix.has(result.surface) && !exempt) {
        failures.push(`${probe.id}: live surface ${result.surface} is missing from matrices`);
      }
    }

    const liveSurfaces = new Set(live.map((entry) => entry.surface));
    for (const entry of FILTERABLE_SURFACE_MATRIX) {
      if (!liveSurfaces.has(entry.surface)) {
        failures.push(`${entry.id}: matrix surface ${entry.surface} no longer appears live`);
      }
      const audit = await auditMatrixCase(session, entry);
      if (audit.warnings.includes("collector_used_current_view_fallback")) {
        failures.push(`${entry.id}: getElements used collector fallback`);
      }
    }
  } finally {
    if (!keepSession) {
      await sessionStop(session);
    }
  }

  const receipt = {
    schemaVersion: 1,
    status: failures.length === 0 ? "pass" : "fail",
    live,
    matrixSurfaces: [...matrix].sort(),
    exemptions: SURFACE_NAVIGATOR_EXEMPTIONS,
    failures,
  };

  process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  if (!json && failures.length > 0) {
    for (const failure of failures) {
      console.error(failure);
    }
  }
  process.exit(failures.length === 0 ? 0 : 1);
}

await main();
