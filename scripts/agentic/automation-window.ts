#!/usr/bin/env bun
/**
 * scripts/agentic/automation-window.ts
 *
 * Resolve an automation target to the exact follow-on surface identity used by
 * native-input and verification helpers.
 */

import { resolve } from "path";
import { assertNoninteractiveSubprocess } from "../devtools/lib/operator-safety.ts";

const SCHEMA_VERSION = 1;
const PROJECT_ROOT = resolve(import.meta.dir, "../..");

type AutomationTargetJson =
  | { type: "focused" }
  | { type: "main" }
  | { type: "id"; id: string }
  | { type: "kind"; kind: string; index?: number }
  | { type: "titleContains"; text: string };

interface ResolveResult {
  schemaVersion: number;
  status: "ok" | "error";
  targetJson?: AutomationTargetJson;
  windowKind?: string | null;
  automationWindowId?: string | null;
  title?: string | null;
  surfaceId?: string | null;
  error?: { code: string; message: string };
}

function stderrLog(event: string, fields: Record<string, unknown> = {}): void {
  console.error(JSON.stringify({ event, ts: new Date().toISOString(), ...fields }));
}

export async function runTool(
  cmd: string[],
  label: string
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  assertNoninteractiveSubprocess(cmd);
  const proc = Bun.spawn(cmd, {
    stdout: "pipe",
    stderr: "pipe",
    cwd: PROJECT_ROOT,
  });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const exitCode = await proc.exited;
  stderrLog("tool_complete", { label, exitCode });
  return { exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
}

async function rpc(
  session: string,
  payload: Record<string, unknown>,
  expect: string,
  timeoutMs: number = 3000
): Promise<Record<string, unknown>> {
  const result = await runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "rpc",
      session,
      JSON.stringify(payload),
      "--expect",
      expect,
      "--timeout",
      String(timeoutMs),
    ],
    "rpc"
  );
  if (result.exitCode !== 0) {
    throw new Error(result.stdout || result.stderr || `RPC failed with exit code ${result.exitCode}`);
  }
  return JSON.parse(result.stdout);
}

async function listSurfaces(): Promise<Record<string, unknown>> {
  const result = await runTool(
    ["bun", "scripts/agentic/window.ts", "list"],
    "list-surfaces"
  );
  if (result.exitCode !== 0) {
    throw new Error(result.stdout || result.stderr || "window.ts list failed");
  }
  return JSON.parse(result.stdout);
}

async function resolveTarget(
  session: string,
  targetJson: AutomationTargetJson
): Promise<ResolveResult> {
  stderrLog("resolve_start", { session, targetJson });

  let state: Record<string, unknown>;
  try {
    state = await rpc(
      session,
      {
        type: "getAgentChatState",
        requestId: `resolve-${Date.now()}`,
        target: targetJson,
      },
      "agent_chatStateResult"
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    stderrLog("resolve_rpc_failed", { error: message });
    return {
      schemaVersion: SCHEMA_VERSION,
      status: "error",
      targetJson,
      error: { code: "rpc_failed", message },
    };
  }

  // session.sh rpc returns { response: { type: "agent_chatStateResult", resolvedTarget: {...}, ... } }
  const response = (state as { response?: Record<string, unknown> }).response;
  const resolvedTarget = (response?.resolvedTarget as Record<string, unknown>) ?? null;
  const windowKind = (resolvedTarget?.windowKind as string) ?? null;
  const automationWindowId = resolvedTarget?.windowId != null
    ? String(resolvedTarget.windowId)
    : null;
  const title = (resolvedTarget?.windowTitle as string) ?? (resolvedTarget?.title as string) ?? null;

  stderrLog("resolve_agent_chat_state", { windowKind, automationWindowId, title });

  let surfaceId: string | null = null;
  try {
    const surfaceEnvelope = await listSurfaces();
    const surfaces =
      (surfaceEnvelope as { data?: { surfaces?: Array<Record<string, unknown>> } }).data?.surfaces ??
      [];

    if (title) {
      const titleMatch = surfaces.find(
        (surface) => typeof surface.title === "string" && surface.title === title
      );
      if (titleMatch) {
        surfaceId = (titleMatch.surfaceId as string) ?? null;
      }
    }

    if (!surfaceId && automationWindowId) {
      const windowIdMatch = surfaces.find(
        (surface) => String(surface.windowId) === String(automationWindowId)
      );
      if (windowIdMatch) {
        surfaceId = (windowIdMatch.surfaceId as string) ?? null;
      }
    }

    stderrLog("resolve_surface_match", { surfaceId, surfaceCount: surfaces.length });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    stderrLog("resolve_surface_list_failed", { error: message });
  }

  return {
    schemaVersion: SCHEMA_VERSION,
    status: "ok",
    targetJson,
    windowKind,
    automationWindowId,
    title,
    surfaceId,
  };
}

// ---------------------------------------------------------------------------
// Inspect types
// ---------------------------------------------------------------------------

interface InspectAutomationWindowResult {
  type: "automationInspectResult";
  requestId: string;
  schemaVersion: number;
  windowId: string;
  windowKind: string;
  title?: string | null;
  elements: Array<Record<string, unknown>>;
  totalCount: number;
  focusedSemanticId?: string | null;
  selectedSemanticId?: string | null;
  screenshotWidth?: number | null;
  screenshotHeight?: number | null;
  pixelProbes: Array<{ x: number; y: number; r: number; g: number; b: number; a: number }>;
  osWindowId?: number | null;
  warnings: string[];
}

/**
 * Popup semantics assessment: did the surface return real semantic elements
 * or only the panel-only fallback?
 */
interface PopupSemantics {
  /** True if the surface returned real input/list/choice elements. */
  hasRealElements: boolean;
  /** True if panel_only_* warning was present (fallback mode). */
  panelOnly: boolean;
  /** Specific warning codes from the surface collector. */
  warnings: string[];
  /** Whether batch mutation (setInput/selectByValue) is expected to work. */
  batchMutationAvailable: boolean;
}

interface AutomationWindowInspectEnvelope {
  schemaVersion: number;
  status: "ok" | "error";
  targetJson?: AutomationTargetJson;
  surfaceId?: string | null;
  automationWindowId?: string | null;
  osWindowId?: number | null;
  inspect?: InspectAutomationWindowResult;
  /** Popup semantics assessment for the inspected surface. */
  popupSemantics?: PopupSemantics;
  error?: { code: string; message: string };
}

async function inspectTarget(
  session: string,
  targetJson: AutomationTargetJson,
  probes: Array<{ x: number; y: number }>
): Promise<AutomationWindowInspectEnvelope> {
  stderrLog("inspect_start", { session, targetJson, probeCount: probes.length });

  const requestId = `inspect-${Date.now()}`;
  const payload: Record<string, unknown> = {
    type: "inspectAutomationWindow",
    requestId,
    target: targetJson,
  };
  if (probes.length > 0) {
    payload.probes = probes;
  }

  let response: Record<string, unknown>;
  try {
    const rpcResult = await rpc(session, payload, "automationInspectResult", 5000);
    response = (rpcResult as { response?: Record<string, unknown> }).response ?? rpcResult;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    stderrLog("inspect_rpc_failed", { error: message });
    return {
      schemaVersion: SCHEMA_VERSION,
      status: "error",
      targetJson,
      error: { code: "rpc_failed", message },
    };
  }

  const inspect = response as unknown as InspectAutomationWindowResult;
  const automationWindowId = inspect.windowId || null;
  const windowKind = inspect.windowKind || null;
  const osWindowId = inspect.osWindowId ?? null;

  // Also resolve surface ID for native input threading
  let surfaceId: string | null = null;
  try {
    const surfaceEnvelope = await listSurfaces();
    const surfaces =
      (surfaceEnvelope as { data?: { surfaces?: Array<Record<string, unknown>> } }).data?.surfaces ??
      [];

    if (inspect.title) {
      const titleMatch = surfaces.find(
        (surface) => typeof surface.title === "string" && surface.title === inspect.title
      );
      if (titleMatch) {
        surfaceId = (titleMatch.surfaceId as string) ?? null;
      }
    }

    if (!surfaceId && automationWindowId) {
      const windowIdMatch = surfaces.find(
        (surface) => String(surface.windowId) === String(automationWindowId)
      );
      if (windowIdMatch) {
        surfaceId = (windowIdMatch.surfaceId as string) ?? null;
      }
    }

    stderrLog("inspect_surface_match", { surfaceId, surfaceCount: surfaces.length });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    stderrLog("inspect_surface_list_failed", { error: message });
  }

  stderrLog("inspect_complete", {
    automationWindowId,
    windowKind,
    surfaceId,
    osWindowId,
    screenshotWidth: inspect.screenshotWidth ?? null,
    screenshotHeight: inspect.screenshotHeight ?? null,
    probeCount: inspect.pixelProbes?.length ?? 0,
    warningCount: inspect.warnings?.length ?? 0,
  });

  // Compute popup semantics from the inspect result
  const inspectWarnings: string[] = (inspect.warnings ?? []).map(String);
  const panelOnly = inspectWarnings.some((w) => w.startsWith("panel_only_"));
  const hasRealElements = (inspect.elements ?? []).some(
    (el: Record<string, unknown>) =>
      el.elementType === "Input" || el.elementType === "List" || el.elementType === "Choice"
  );
  const popupKinds = ["actionsDialog", "promptPopup"];
  const isPopupTarget =
    (targetJson.type === "kind" && popupKinds.includes(targetJson.kind)) ||
    (windowKind != null && popupKinds.includes(windowKind));
  const batchMutationAvailable = isPopupTarget && hasRealElements && !panelOnly;

  const popupSemantics: PopupSemantics = {
    hasRealElements,
    panelOnly,
    warnings: inspectWarnings,
    batchMutationAvailable,
  };

  return {
    schemaVersion: SCHEMA_VERSION,
    status: "ok",
    targetJson,
    surfaceId,
    automationWindowId,
    osWindowId,
    inspect,
    popupSemantics,
  };
}

// ---------------------------------------------------------------------------
// Arg parsing
// ---------------------------------------------------------------------------

function parseArgs() {
  const args = process.argv.slice(2);
  const subcmd = args[0] ?? "help";

  const sessionIdx = args.indexOf("--session");
  const session = sessionIdx >= 0 && args[sessionIdx + 1] ? args[sessionIdx + 1] : "default";

  const kindIdx = args.indexOf("--kind");
  const kind = kindIdx >= 0 && args[kindIdx + 1] ? args[kindIdx + 1] : "agentChatDetached";

  const indexIdx = args.indexOf("--index");
  const index = indexIdx >= 0 && args[indexIdx + 1] ? Number(args[indexIdx + 1]) : 0;

  const idIdx = args.indexOf("--id");
  const id = idIdx >= 0 && args[idIdx + 1] ? args[idIdx + 1] : undefined;

  const titleIdx = args.indexOf("--title");
  const titleText = titleIdx >= 0 && args[titleIdx + 1] ? args[titleIdx + 1] : undefined;

  // Collect --probe x,y pairs
  const probes: Array<{ x: number; y: number }> = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--probe" && args[i + 1]) {
      const parts = args[i + 1].split(",");
      if (parts.length === 2) {
        const x = parseInt(parts[0], 10);
        const y = parseInt(parts[1], 10);
        if (!isNaN(x) && !isNaN(y)) {
          probes.push({ x, y });
        }
      }
    }
  }

  return { subcmd, session, kind, index, id, titleText, probes };
}

const { subcmd, session, kind, index, id, titleText, probes } = parseArgs();

function buildTargetJson(): AutomationTargetJson {
  if (id) {
    return { type: "id", id };
  } else if (titleText) {
    return { type: "titleContains", text: titleText };
  } else if (kind === "main") {
    return { type: "main" };
  } else if (kind === "focused") {
    return { type: "focused" };
  } else {
    return { type: "kind", kind, index };
  }
}

if (import.meta.main) switch (subcmd) {
  case "resolve": {
    const targetJson = buildTargetJson();
    const result = await resolveTarget(session, targetJson);
    console.log(JSON.stringify(result, null, 2));
    process.exit(result.status === "ok" ? 0 : 1);
    break;
  }

  case "inspect": {
    const targetJson = buildTargetJson();
    const result = await inspectTarget(session, targetJson, probes);
    console.log(JSON.stringify(result, null, 2));
    process.exit(result.status === "ok" ? 0 : 1);
    break;
  }

  case "help":
  case "--help": {
    console.log(`Usage: bun scripts/agentic/automation-window.ts <command> [options]

Commands:
  resolve    Resolve an automation target to an exact surface identity
  inspect    Inspect an automation window (screenshot dims, pixel probes, elements)
  help       Show this help

Options:
  --session NAME    Session name (default: "default")
  --kind KIND       Target kind: agentChatDetached | actionsDialog | promptPopup | notes | main | focused
  --index N         Kind index (default: 0)
  --id WINDOW_ID    Resolve by exact automation window ID
  --title TEXT      Resolve by titleContains target
  --probe X,Y       Pixel probe coordinate (repeatable, inspect only)
`);
    process.exit(0);
  }

  default:
    console.error(`Unknown command: ${subcmd}`);
    process.exit(2);
}
