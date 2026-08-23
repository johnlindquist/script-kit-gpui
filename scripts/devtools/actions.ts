#!/usr/bin/env bun

import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic, userContent } from "./lib/privacy.ts";
import { classifyTransportError } from "./lib/transport-errors.ts";
import {
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveSubprocess,
  requireSuccessfulSessionAction,
} from "./lib/operator-safety.ts";

type JsonObject = Record<string, unknown>;
type Rect = { x: number; y: number; width: number; height: number };

type Args = {
  session: string;
  target?: JsonObject;
  openTarget?: JsonObject;
  timeoutMs: number;
  open: boolean;
  start: boolean;
  keepOpen: boolean;
  proveHover: boolean;
  proveClickSelect: boolean;
  proveClickActivate: boolean;
  proveSemanticFreshness: boolean;
  proveCloseCleanup: boolean;
  proveShortcutOpenFreshness: boolean;
  proveShortcutCloseCleanup: boolean;
  proveEscapeCloseCleanup: boolean;
  sampleMs: number;
  intervalMs: number;
  inspectTargetForwarded: string[];
  hasExplicitInspectTarget: boolean;
  openTargetForwarded: string[];
  hasExplicitOpenTarget: boolean;
};

const DEFAULT_INSPECT_TARGET = ["--target-kind", "actionsDialog", "--strict", "--surface", "ActionsDialog"];
const DEFAULT_OPEN_TARGET = ["--show", "--main", "--strict", "--surface", "ScriptList"];

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/actions.ts inspect [--session <name>] [--start] [--keep-open] [--open] [--open-target-kind <kind>] [--prove-hover] [--prove-click-select] [--prove-click-activate] [--prove-semantic-freshness] [--prove-close-cleanup] [--prove-shortcut-open-freshness] [--prove-shortcut-close-cleanup] [--prove-escape-close-cleanup] [--sample-ms <ms>] [--interval-ms <ms>] [target args]",
    "",
    "Target args match scripts/devtools/targets.ts inspect. Defaults to --target-kind actionsDialog --strict --surface ActionsDialog.",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  if (argv[0] !== "inspect") {
    console.error(usage());
    process.exit(2);
  }

  const args: Args = {
    session: "default",
    timeoutMs: 8000,
    open: false,
    start: false,
    keepOpen: false,
    proveHover: false,
    proveClickSelect: false,
    proveClickActivate: false,
    proveSemanticFreshness: false,
    proveCloseCleanup: false,
    proveShortcutOpenFreshness: false,
    proveShortcutCloseCleanup: false,
    proveEscapeCloseCleanup: false,
    sampleMs: 900,
    intervalMs: 50,
    inspectTargetForwarded: [],
    hasExplicitInspectTarget: false,
    openTargetForwarded: [],
    hasExplicitOpenTarget: false,
  };
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
    } else if (arg === "--open") {
      args.open = true;
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--keep-open") {
      args.keepOpen = true;
    } else if (arg === "--prove-hover") {
      args.proveHover = true;
    } else if (arg === "--prove-click-select") {
      args.proveClickSelect = true;
    } else if (arg === "--prove-click-activate") {
      args.proveClickActivate = true;
    } else if (arg === "--prove-semantic-freshness") {
      args.proveSemanticFreshness = true;
    } else if (arg === "--prove-close-cleanup") {
      args.proveCloseCleanup = true;
    } else if (arg === "--prove-shortcut-open-freshness") {
      args.proveShortcutOpenFreshness = true;
    } else if (arg === "--prove-shortcut-close-cleanup") {
      args.proveShortcutCloseCleanup = true;
    } else if (arg === "--prove-escape-close-cleanup") {
      args.proveEscapeCloseCleanup = true;
    } else if (arg === "--open-target-id") {
      args.openTarget = { type: "id", id: argv[++index] ?? "" };
      args.hasExplicitOpenTarget = true;
      args.openTargetForwarded.push("--target-id", String(args.openTarget.id ?? ""));
    } else if (arg === "--open-target-kind") {
      const kind = argv[++index] ?? "main";
      args.openTarget = { type: "kind", kind };
      args.hasExplicitOpenTarget = true;
      args.openTargetForwarded.push("--target-kind", kind);
    } else if (arg === "--open-target-index") {
      const value = Number(argv[++index] ?? 0);
      if (!args.openTarget || args.openTarget.type !== "kind") {
        throw new Error("--open-target-index requires --open-target-kind first");
      }
      args.openTarget.index = value;
      args.hasExplicitOpenTarget = true;
      args.openTargetForwarded.push("--target-index", String(value));
    } else if (arg === "--open-target-title") {
      args.openTarget = { type: "titleContains", text: argv[++index] ?? "" };
      args.hasExplicitOpenTarget = true;
      args.openTargetForwarded.push("--target-title", String(args.openTarget.text ?? ""));
    } else if (arg === "--open-focused") {
      args.openTarget = { type: "focused" };
      args.hasExplicitOpenTarget = true;
      args.openTargetForwarded.push("--focused");
    } else if (arg === "--open-main") {
      args.openTarget = { type: "main" };
      args.hasExplicitOpenTarget = true;
      args.openTargetForwarded.push("--main");
    } else if (arg === "--target-id") {
      args.target = { type: "id", id: argv[++index] ?? "" };
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--target-id", String(args.target.id ?? ""));
    } else if (arg === "--target-kind") {
      const kind = argv[++index] ?? "actionsDialog";
      args.target = { type: "kind", kind };
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--target-kind", kind);
    } else if (arg === "--target-index") {
      const value = Number(argv[++index] ?? 0);
      if (!args.target || args.target.type !== "kind") {
        throw new Error("--target-index requires --target-kind first");
      }
      args.target.index = value;
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--target-index", String(value));
    } else if (arg === "--target-title") {
      args.target = { type: "titleContains", text: argv[++index] ?? "" };
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--target-title", String(args.target.text ?? ""));
    } else if (arg === "--focused") {
      args.target = { type: "focused" };
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--focused");
    } else if (arg === "--main") {
      args.target = { type: "main" };
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--main");
    } else if (arg === "--strict") {
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--strict");
    } else if (arg === "--surface") {
      args.hasExplicitInspectTarget = true;
      args.inspectTargetForwarded.push("--surface", argv[++index] ?? "");
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
    } else if (arg === "--sample-ms") {
      args.sampleMs = Number(argv[++index] ?? args.sampleMs);
    } else if (arg === "--interval-ms") {
      args.intervalMs = Number(argv[++index] ?? args.intervalMs);
    }
  }
  return args;
}

function inspectForwarded(args: Args): string[] {
  return [
    "--session",
    args.session,
    ...(args.hasExplicitInspectTarget ? args.inspectTargetForwarded : DEFAULT_INSPECT_TARGET),
    "--timeout",
    String(args.timeoutMs),
  ];
}

function openForwarded(args: Args): string[] {
  return [
    "--session",
    args.session,
    ...(args.hasExplicitOpenTarget ? args.openTargetForwarded : DEFAULT_OPEN_TARGET),
    "--timeout",
    String(args.timeoutMs),
  ];
}

export async function runActionsSubprocess(
  command: string[],
  label: string,
  env?: Record<string, string>,
): Promise<JsonObject> {
  assertNoninteractiveSubprocess(command, env ?? {});
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe", env: env ? { ...process.env, ...env } : process.env });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (exitCode !== 0) {
    return { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
  }
  try {
    return JSON.parse(stdout);
  } catch {
    return { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim(), error: "invalid_json_output" };
  }
}

const run = runActionsSubprocess;

function requestId(prefix: string) {
  return `devtools-actions-${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function rpc(session: string, payload: JsonObject, expect: string, timeoutMs: number) {
  assertNoninteractiveProtocolCommand(payload);
  return run(["bash", "scripts/agentic/session.sh", "rpc", session, JSON.stringify(payload), "--expect", expect, "--timeout", String(timeoutMs)], String(payload.type ?? "rpc"));
}

export async function maybeStartSession(
  args: Pick<Args, "start" | "session" | "keepOpen">,
) {
  if (!args.start) return null;
  const env = args.keepOpen ? { SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN: "1" } : undefined;
  const started = await run(
    ["bash", "scripts/agentic/session.sh", "start", args.session],
    "session.start",
    env,
  );
  return requireSuccessfulSessionAction(args.session, "start", started);
}

async function maybeOpenParentTarget(args: Args) {
  const normalizedOpenKind =
    typeof args.openTarget?.kind === "string" ? args.openTarget.kind.toLowerCase() : "";
  if (!args.open || normalizedOpenKind !== "notes") return null;
  const receipt = await run([
    "bash",
    "scripts/agentic/session.sh",
    "send",
    args.session,
    JSON.stringify({ type: "openNotes", requestId: requestId("open-notes-parent") }),
    "--await-parse",
    "--timeout",
    String(args.timeoutMs),
  ], "parent.open.notes");
  // Settle on the real observable instead of a fixed sleep: poll strict
  // target resolution until the Notes window is addressable (normally the
  // first attempt).
  await waitForNotesParentTarget(args);
  return receipt;
}

async function waitForNotesParentTarget(args: Args): Promise<void> {
  const deadline = Date.now() + Math.min(args.timeoutMs, 5000);
  while (Date.now() < deadline) {
    const receipt = await run([
      "bun",
      "scripts/devtools/targets.ts",
      "inspect",
      "--session",
      args.session,
      "--target-kind",
      "notes",
      "--strict",
    ], "targets.inspect.notes-parent");
    if (receipt.classification === "ok") {
      return;
    }
    await Bun.sleep(25);
  }
}

function responseOf(envelope: JsonObject): JsonObject {
  return (envelope.response as JsonObject | undefined) ?? envelope;
}

function asArray(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.filter((entry): entry is JsonObject => typeof entry === "object" && entry !== null) : [];
}

function asNumber(value: unknown, fallback = 0) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function rectFrom(value: unknown): Rect | null {
  if (!value || typeof value !== "object") return null;
  const object = value as JsonObject;
  return {
    x: asNumber(object.x),
    y: asNumber(object.y),
    width: asNumber(object.width),
    height: asNumber(object.height),
  };
}

function edgeClipping(child: Rect | null, parent: Rect | null) {
  if (!child || !parent) {
    return { available: false, top: null, right: null, bottom: null, left: null };
  }
  return {
    available: true,
    top: child.y < parent.y,
    right: child.x + child.width > parent.x + parent.width,
    bottom: child.y + child.height > parent.y + parent.height,
    left: child.x < parent.x,
  };
}

function shortcutTokens(shortcut: unknown) {
  if (typeof shortcut !== "string" || shortcut.trim().length === 0) return [];
  return shortcut
    .split(/(?=[⌘⇧⌃⌥↑↓←→↵⌫⇥⎋␣])|\+/)
    .map((token) => token.trim())
    .filter(Boolean);
}

export function visibleActionRows(actionsDialog: JsonObject | null) {
  const actions = (actionsDialog?.actions as JsonObject | undefined) ?? {};
  const rows = Array.isArray(actionsDialog?.visibleActions)
    ? asArray(actionsDialog?.visibleActions)
    : asArray(actions.visibleSample);
  return rows.map((action, index) => {
    const shortcut = action.shortcut ?? null;
    const disabledReason = action.actionDisabled ?? action.disabledReason ?? null;
    const declaredEnabled = typeof action.enabled === "boolean"
      ? action.enabled
      : null;
    const enabled = disabledReason == null ? declaredEnabled : false;
    return {
      index,
      id: action.id ?? null,
      label: action.label ?? action.title ?? null,
      section: action.section ?? null,
      shortcut,
      shortcutTokens: shortcutTokens(shortcut),
      destructive: action.destructive ?? false,
      enabled,
      disabledReason,
      stateConsistent: !(declaredEnabled === true && disabledReason != null),
      activatable: enabled === true && typeof action.id === "string" &&
        action.id.length > 0,
    };
  });
}

export function auditActionProjection(rows: ReturnType<typeof visibleActionRows>) {
  const ids = rows
    .map((row) => typeof row.id === "string" ? row.id : "")
    .filter(Boolean);
  const duplicateActionIds = ids.filter((id, index) => ids.indexOf(id) !== index);
  const missingActionIds = rows
    .filter((row) => typeof row.id !== "string" || row.id.length === 0)
    .map((row) => row.index);
  const unknownAvailabilityIds = rows
    .filter((row) => row.enabled == null)
    .map((row) => row.id ?? `(row:${row.index})`);
  const contradictoryAvailabilityIds = rows
    .filter((row) => row.stateConsistent !== true)
    .map((row) => row.id ?? `(row:${row.index})`);
  return {
    rowCount: rows.length,
    activatableActionIds: rows
      .filter((row) => row.activatable)
      .map((row) => String(row.id)),
    duplicateActionIds: [...new Set(duplicateActionIds)],
    missingActionIds,
    unknownAvailabilityIds,
    contradictoryAvailabilityIds,
    complete:
      duplicateActionIds.length === 0 &&
      missingActionIds.length === 0 &&
      unknownAvailabilityIds.length === 0 &&
      contradictoryAvailabilityIds.length === 0,
  };
}

export function groupActionSections(rows: ReturnType<typeof visibleActionRows>) {
  const sections = new Map<string, { title: string; rowCount: number; firstIndex: number; lastIndex: number }>();
  rows.forEach((row, index) => {
    const title = typeof row.section === "string" && row.section ? row.section : "default";
    const existing = sections.get(title);
    if (existing) {
      existing.rowCount += 1;
      existing.lastIndex = index;
    } else {
      sections.set(title, { title, rowCount: 1, firstIndex: index, lastIndex: index });
    }
  });
  return [...sections.values()];
}

function rowSemanticId(row: JsonObject) {
  return typeof row.semanticId === "string" ? row.semanticId : null;
}

function visibleActionRowsFromGeometry(rowGeometry: JsonObject | null) {
  return asArray(rowGeometry?.rows).filter((row) => {
    const semanticId = rowSemanticId(row);
    return row.kind === "action"
      && row.visible === true
      && typeof semanticId === "string"
      && semanticId.startsWith("choice:")
      && Boolean(rectFrom(row.bounds) ?? rectFrom(row.rect));
  });
}

function pickHoverProofRow(rowGeometry: JsonObject | null) {
  return visibleActionRowsFromGeometry(rowGeometry)[0] ?? null;
}

function selectedRowOf(rowGeometry: JsonObject | null) {
  return ((rowGeometry?.selectedRow as JsonObject | undefined) ?? null);
}

function selectedRowSemanticId(rowGeometry: JsonObject | null) {
  const selectedRow = selectedRowOf(rowGeometry);
  return selectedRow ? rowSemanticId(selectedRow) : null;
}

function pickClickSelectProofRow(rowGeometry: JsonObject | null) {
  const selectedSemanticId = selectedRowSemanticId(rowGeometry);
  return visibleActionRowsFromGeometry(rowGeometry)
    .find((row) => rowSemanticId(row) !== selectedSemanticId) ?? null;
}

const CLICK_ACTIVATE_ALLOWED_ACTION_IDS = new Set(["toggle_info"]);

function pickClickActivateProofRow(rowGeometry: JsonObject | null) {
  return visibleActionRowsFromGeometry(rowGeometry).find((row) => {
    const actionId = typeof row.actionId === "string" ? row.actionId : "";
    return CLICK_ACTIVATE_ALLOWED_ACTION_IDS.has(actionId);
  }) ?? null;
}

function centerOfRect(rect: Rect) {
  return {
    x: rect.x + rect.width / 2,
    y: rect.y + rect.height / 2,
  };
}

function findParentTarget(target: JsonObject, windows: JsonObject[]) {
  const parentId = target.parentAutomationId ?? target.openerAutomationId;
  return windows.find((window) => window.automationId === parentId) ?? null;
}

async function maybeOpenActions(args: Args) {
  if (!args.open) return null;
  const openCommand = [
      "bun",
      "scripts/devtools/act.ts",
      "open-actions",
      ...openForwarded(args),
    ];
  return run([
    ...openCommand,
  ], "actions.open");
}

function isActionsDialogTarget(receipt: JsonObject) {
  const target = receipt.resolvedTarget as JsonObject | undefined;
  return receipt.classification === "ok"
    && target?.automationId === "actions-dialog"
    && target?.targetKind === "ActionsDialog";
}

async function waitForActionsDialogTarget(args: Args, forwarded: string[]) {
  const startedAt = Date.now();
  const deadline = startedAt + args.timeoutMs;
  const attempts: JsonObject[] = [];

  while (Date.now() < deadline) {
    const receipt = await run(
      ["bun", "scripts/devtools/targets.ts", "inspect", ...forwarded],
      "targets.inspect.actionsDialog.ready",
    );
    attempts.push({
      elapsedMs: Date.now() - startedAt,
      classification: receipt.classification ?? null,
      requestedTarget: receipt.requestedTarget ?? null,
      resolvedTarget: receipt.resolvedTarget ?? null,
      errors: receipt.errors ?? [],
    });
    if (isActionsDialogTarget(receipt)) {
      return { status: "ok", receipt, attempts };
    }
    await Bun.sleep(50);
  }

  return {
    status: "error",
    classification: "blocked-by-target-ambiguity",
    reason: "actionsDialog target did not become observable after openActions",
    attempts,
  };
}

function classify(targetReceipt: JsonObject, stateEnvelope: JsonObject, missing: string[]) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  if (stateEnvelope.status === "error") {
    return classifyTransportError(stateEnvelope);
  }
  if (missing.length > 0) {
    return "blocked-by-missing-primitive";
  }
  return "ok";
}

function firstNonOkClassification(classifications: Array<unknown>): string {
  for (const classification of classifications) {
    if (typeof classification === "string" && classification !== "ok") {
      return classification;
    }
  }
  return "ok";
}

function hoveredRowOf(rowGeometry: JsonObject | null) {
  return ((rowGeometry?.hoveredRow as JsonObject | undefined) ?? null);
}

function hoveredRowSemanticId(rowGeometry: JsonObject | null) {
  const hoveredRow = hoveredRowOf(rowGeometry);
  const row = (hoveredRow?.row as JsonObject | undefined) ?? null;
  return row ? rowSemanticId(row) : null;
}

function mouseArmedRowOf(rowGeometry: JsonObject | null) {
  return ((rowGeometry?.mouseArmedRow as JsonObject | undefined) ?? null);
}

function mouseArmedRowSemanticId(rowGeometry: JsonObject | null) {
  const armedRow = mouseArmedRowOf(rowGeometry);
  const row = (armedRow?.row as JsonObject | undefined) ?? null;
  return row ? rowSemanticId(row) : null;
}

function stateActionsDialog(envelope: JsonObject) {
  const state = responseOf(envelope);
  return (state.actionsDialog as JsonObject | undefined) ?? null;
}

function dispatchSuccess(receipt: JsonObject) {
  return receipt.status !== "error" && responseOf(receipt).success === true;
}

function dispatchPath(receipt: JsonObject) {
  const response = responseOf(receipt);
  return response.dispatchPath ?? receipt.dispatchPath ?? null;
}

function hoverProofBlocked(reason: string, extras: JsonObject = {}) {
  return {
    classification: "blocked-by-missing-primitive",
    command: "actions.hoverProof",
    reason,
    missingPrimitives: ["target-scoped ActionsDialog hover proof"],
    safety: {
      noNativeEscalation: true,
      submitAttempted: false,
      activationAttempted: false,
    },
    ...extras,
  };
}

function clickSelectProofBlocked(reason: string, extras: JsonObject = {}) {
  return {
    classification: "blocked-by-missing-primitive",
    command: "actions.clickSelectProof",
    reason,
    missingPrimitives: ["target-scoped ActionsDialog first-click selection proof"],
    safety: {
      noNativeEscalation: true,
      submitAttempted: false,
      activationAttempted: false,
      activationObserved: false,
    },
    ...extras,
  };
}

function clickActivateProofBlocked(reason: string, extras: JsonObject = {}) {
  return {
    classification: "blocked-by-missing-primitive",
    command: "actions.clickActivateProof",
    reason,
    missingPrimitives: ["target-scoped ActionsDialog second-click activation proof"],
    safety: {
      noNativeEscalation: true,
      submitAttempted: true,
      activationAttempted: true,
      destructiveActionAllowed: false,
    },
    ...extras,
  };
}

function closeCleanupProofBlocked(reason: string, extras: JsonObject = {}) {
  return {
    classification: "blocked-by-stale-view",
    command: "actions.closeCleanupProof",
    reason,
    missingPrimitives: ["target-scoped ActionsDialog close cleanup proof"],
    safety: {
      noNativeEscalation: true,
      submitAttempted: false,
      activationAttempted: false,
    },
    ...extras,
  };
}

function shortcutOpenFreshnessProofBlocked(
  classification: "blocked-by-missing-primitive" | "blocked-by-stale-view" | "blocked-by-target-ambiguity",
  reason: string,
  extras: JsonObject = {},
) {
  return {
    classification,
    command: "actions.shortcutOpenFreshnessProof",
    reason,
    missingPrimitives: ["target-scoped ActionsDialog Cmd+K shortcut-open first-frame freshness proof"],
    safety: {
      noNativeEscalation: true,
      shortcutDispatched: false,
      submitAttempted: false,
      activationAttempted: false,
    },
    ...extras,
  };
}

function shortcutCloseCleanupProofBlocked(
  classification: "blocked-by-missing-primitive" | "blocked-by-stale-view" | "blocked-by-target-ambiguity",
  reason: string,
  extras: JsonObject = {},
) {
  return {
    classification,
    command: "actions.shortcutCloseCleanupProof",
    reason,
    missingPrimitives: ["target-scoped ActionsDialog Cmd+K shortcut-close cleanup proof"],
    safety: {
      noNativeEscalation: true,
      shortcutDispatched: false,
      submitAttempted: false,
      activationAttempted: false,
    },
    ...extras,
  };
}

function escapeCloseCleanupProofBlocked(
  classification: "blocked-by-missing-primitive" | "blocked-by-stale-view" | "blocked-by-target-ambiguity",
  reason: string,
  extras: JsonObject = {},
) {
  return {
    classification,
    command: "actions.escapeCloseCleanupProof",
    reason,
    missingPrimitives: ["target-scoped ActionsDialog Escape close cleanup proof"],
    safety: {
      noNativeEscalation: true,
      shortcutDispatched: false,
      submitAttempted: false,
      activationAttempted: false,
    },
    ...extras,
  };
}

async function dispatchLeftClick(
  args: Args,
  point: { x: number; y: number },
  prefix: string,
) {
  const eventTarget = { type: "kind", kind: "actionsDialog" };
  const mouseDown = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId(`${prefix}-down`),
      target: eventTarget,
      event: { type: "mouseDown", x: point.x, y: point.y, button: "left" },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );
  const mouseUp = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId(`${prefix}-up`),
      target: eventTarget,
      event: { type: "mouseUp", x: point.x, y: point.y, button: "left" },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );
  return { mouseDown, mouseUp };
}

async function runHoverProof(
  args: Args,
  selector: JsonObject,
  target: JsonObject,
  rowGeometry: JsonObject | null,
) {
  if (rowGeometry?.hoverRowAvailable !== true) {
    return hoverProofBlocked("rowGeometry.hoverRowAvailable is not true", { rowGeometry });
  }

  const requestedRow = pickHoverProofRow(rowGeometry);
  const requestedSemanticId = requestedRow ? rowSemanticId(requestedRow) : null;
  const requestedBounds = requestedRow ? (rectFrom(requestedRow.bounds) ?? rectFrom(requestedRow.rect)) : null;
  if (!requestedRow || !requestedSemanticId || !requestedBounds) {
    return hoverProofBlocked("no visible action row with usable bounds", { requestedRow });
  }

  const point = centerOfRect(requestedBounds);
  const eventTarget = { type: "kind", kind: "actionsDialog" };
  const beforeHoveredRow = hoveredRowOf(rowGeometry);
  const simulateReceipt = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId("hover"),
      target: eventTarget,
      event: { type: "mouseMove", x: point.x, y: point.y },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );

  if (simulateReceipt.status === "error" || responseOf(simulateReceipt).success === false) {
    return hoverProofBlocked("simulateGpuiEvent did not dispatch", {
      requestedRow,
      point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
      receipts: { simulateGpuiEvent: simulateReceipt },
    });
  }

  const startedAt = Date.now();
  const attempts: JsonObject[] = [];
  let stateAfter: JsonObject | null = null;
  let actionsDialogAfter: JsonObject | null = null;
  let rowGeometryAfter: JsonObject | null = null;
  while (Date.now() - startedAt < Math.min(args.timeoutMs, 1500)) {
    stateAfter = await rpc(
      args.session,
      { type: "getState", requestId: requestId("hover-state"), target: selector, summaryOnly: true },
      "stateResult",
      args.timeoutMs,
    );
    actionsDialogAfter = stateActionsDialog(stateAfter);
    rowGeometryAfter = (actionsDialogAfter?.rowGeometry as JsonObject | undefined) ?? null;
    const hoveredSemanticId = hoveredRowSemanticId(rowGeometryAfter);
    attempts.push({
      elapsedMs: Date.now() - startedAt,
      hoveredSemanticId,
      hoveredRow: hoveredRowOf(rowGeometryAfter),
    });
    if (hoveredSemanticId === requestedSemanticId) break;
    await Bun.sleep(50);
  }

  const popupStillOpen = Boolean(actionsDialogAfter);
  const targetStable = target.automationId === "actions-dialog" && target.targetKind === "ActionsDialog";
  const hoveredRequestedRow = hoveredRowSemanticId(rowGeometryAfter) === requestedSemanticId;
  const assertions = {
    rowBoundsAvailable: Boolean(requestedBounds && requestedBounds.width > 0 && requestedBounds.height > 0),
    hoveredRequestedRow,
    popupStillOpen,
    targetStable,
  };

  if (!hoveredRequestedRow || !popupStillOpen || !targetStable) {
    return hoverProofBlocked("hover did not update to requested row", {
      requestedRow,
      point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
      before: { hoveredRow: beforeHoveredRow },
      after: { hoveredRow: hoveredRowOf(rowGeometryAfter) },
      assertions,
      attempts,
      receipts: { simulateGpuiEvent: simulateReceipt, stateAfter },
    });
  }

  return {
    classification: "ok",
    command: "actions.hoverProof",
    safety: {
      noNativeEscalation: true,
      submitAttempted: false,
      activationAttempted: false,
    },
    requestedRow: {
      semanticId: requestedSemanticId,
      visualIndex: requestedRow.visualIndex ?? null,
      kind: requestedRow.kind ?? null,
      actionId: requestedRow.actionId ?? null,
      bounds: requestedBounds,
    },
    point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
    before: { hoveredRow: beforeHoveredRow },
    after: { hoveredRow: hoveredRowOf(rowGeometryAfter) },
    assertions,
    attempts,
    receipts: { simulateGpuiEvent: simulateReceipt, stateAfter },
  };
}

async function runClickSelectProof(
  args: Args,
  selector: JsonObject,
  target: JsonObject,
  rowGeometry: JsonObject | null,
) {
  if (rowGeometry?.rowBoundsAvailable !== true) {
    return clickSelectProofBlocked("rowGeometry.rowBoundsAvailable is not true", { rowGeometry });
  }

  const requestedRow = pickClickSelectProofRow(rowGeometry);
  const requestedSemanticId = requestedRow ? rowSemanticId(requestedRow) : null;
  const requestedBounds = requestedRow
    ? (rectFrom(requestedRow.innerBounds) ?? rectFrom(requestedRow.bounds) ?? rectFrom(requestedRow.rect))
    : null;
  const beforeSelectedSemanticId = selectedRowSemanticId(rowGeometry);
  if (!requestedRow || !requestedSemanticId || !requestedBounds) {
    return clickSelectProofBlocked("no visible non-selected action row with usable bounds", {
      before: { selectedSemanticId: beforeSelectedSemanticId, selectedRow: selectedRowOf(rowGeometry) },
      requestedRow,
    });
  }

  const point = centerOfRect(requestedBounds);
  const eventTarget = { type: "kind", kind: "actionsDialog" };
  const mouseMove = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId("click-select-move"),
      target: eventTarget,
      event: { type: "mouseMove", x: point.x, y: point.y },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );
  const mouseDown = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId("click-select-down"),
      target: eventTarget,
      event: { type: "mouseDown", x: point.x, y: point.y, button: "left" },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );
  const mouseUp = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId("click-select-up"),
      target: eventTarget,
      event: { type: "mouseUp", x: point.x, y: point.y, button: "left" },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );

  if (!dispatchSuccess(mouseDown) || !dispatchSuccess(mouseUp) || dispatchPath(mouseDown) !== "exact_handle" || dispatchPath(mouseUp) !== "exact_handle") {
    return clickSelectProofBlocked("mouse down/up did not dispatch through exact actions dialog handle", {
      requestedRow,
      point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
      receipts: { mouseMove, mouseDown, mouseUp },
    });
  }

  const startedAt = Date.now();
  const attempts: JsonObject[] = [];
  let stateAfter: JsonObject | null = null;
  let actionsDialogAfter: JsonObject | null = null;
  let rowGeometryAfter: JsonObject | null = null;
  while (Date.now() - startedAt < Math.min(args.timeoutMs, 1500)) {
    stateAfter = await rpc(
      args.session,
      { type: "getState", requestId: requestId("click-select-state"), target: selector, summaryOnly: true },
      "stateResult",
      args.timeoutMs,
    );
    actionsDialogAfter = stateActionsDialog(stateAfter);
    rowGeometryAfter = (actionsDialogAfter?.rowGeometry as JsonObject | undefined) ?? null;
    const selectedSemanticId = selectedRowSemanticId(rowGeometryAfter);
    const armedSemanticId = mouseArmedRowSemanticId(rowGeometryAfter);
    attempts.push({
      elapsedMs: Date.now() - startedAt,
      selectedSemanticId,
      armedSemanticId,
      selectedRow: selectedRowOf(rowGeometryAfter),
      mouseArmedRow: mouseArmedRowOf(rowGeometryAfter),
    });
    if (selectedSemanticId === requestedSemanticId && armedSemanticId === requestedSemanticId) break;
    await Bun.sleep(50);
  }

  const afterSelectedSemanticId = selectedRowSemanticId(rowGeometryAfter);
  const afterArmedSemanticId = mouseArmedRowSemanticId(rowGeometryAfter);
  const popupStillOpen = Boolean(actionsDialogAfter);
  const targetStable = target.automationId === "actions-dialog" && target.targetKind === "ActionsDialog";
  const assertions = {
    rowBoundsAvailable: Boolean(requestedBounds && requestedBounds.width > 0 && requestedBounds.height > 0),
    clickedDifferentRow: beforeSelectedSemanticId !== requestedSemanticId,
    selectionChanged: beforeSelectedSemanticId !== afterSelectedSemanticId,
    selectedRequestedRow: afterSelectedSemanticId === requestedSemanticId,
    mouseArmedRequestedRow: afterArmedSemanticId === requestedSemanticId,
    popupStillOpen,
    targetStable,
    activationObserved: !popupStillOpen,
  };

  if (!assertions.clickedDifferentRow || !assertions.selectionChanged || !assertions.selectedRequestedRow || !assertions.mouseArmedRequestedRow || !popupStillOpen || !targetStable) {
    return clickSelectProofBlocked("first click did not select and arm the requested row", {
      requestedRow,
      point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
      before: {
        selectedSemanticId: beforeSelectedSemanticId,
        selectedRow: selectedRowOf(rowGeometry),
        mouseArmedRow: mouseArmedRowOf(rowGeometry),
      },
      after: {
        selectedSemanticId: afterSelectedSemanticId,
        selectedRow: selectedRowOf(rowGeometryAfter),
        mouseArmedRow: mouseArmedRowOf(rowGeometryAfter),
      },
      assertions,
      attempts,
      receipts: { mouseMove, mouseDown, mouseUp, stateAfter },
    });
  }

  return {
    classification: "ok",
    command: "actions.clickSelectProof",
    safety: {
      noNativeEscalation: true,
      submitAttempted: false,
      activationAttempted: false,
      activationObserved: false,
    },
    requestedRow: {
      semanticId: requestedSemanticId,
      visualIndex: requestedRow.visualIndex ?? null,
      kind: requestedRow.kind ?? null,
      actionId: requestedRow.actionId ?? null,
      bounds: requestedBounds,
    },
    point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
    before: {
      selectedSemanticId: beforeSelectedSemanticId,
      selectedRow: selectedRowOf(rowGeometry),
      mouseArmedRow: mouseArmedRowOf(rowGeometry),
    },
    after: {
      selectedSemanticId: afterSelectedSemanticId,
      selectedRow: selectedRowOf(rowGeometryAfter),
      mouseArmedRow: mouseArmedRowOf(rowGeometryAfter),
    },
    assertions,
    attempts,
    receipts: { mouseMove, mouseDown, mouseUp, stateAfter },
  };
}

async function runClickActivateProof(
  args: Args,
  selector: JsonObject,
  target: JsonObject,
  rowGeometry: JsonObject | null,
) {
  if (rowGeometry?.rowBoundsAvailable !== true) {
    return clickActivateProofBlocked("rowGeometry.rowBoundsAvailable is not true", { rowGeometry });
  }

  const requestedRow = pickClickActivateProofRow(rowGeometry);
  const requestedSemanticId = requestedRow ? rowSemanticId(requestedRow) : null;
  const requestedBounds = requestedRow
    ? (rectFrom(requestedRow.innerBounds) ?? rectFrom(requestedRow.bounds) ?? rectFrom(requestedRow.rect))
    : null;
  const beforeSelectedSemanticId = selectedRowSemanticId(rowGeometry);
  const beforeArmedSemanticId = mouseArmedRowSemanticId(rowGeometry);
  if (!requestedRow || !requestedSemanticId || !requestedBounds) {
    return clickActivateProofBlocked("no safe visible action row with usable bounds", {
      before: {
        selectedSemanticId: beforeSelectedSemanticId,
        selectedRow: selectedRowOf(rowGeometry),
        mouseArmedRow: mouseArmedRowOf(rowGeometry),
      },
      requestedRow,
    });
  }
  if (beforeSelectedSemanticId === requestedSemanticId || beforeArmedSemanticId === requestedSemanticId) {
    return clickActivateProofBlocked("safe row is already selected or armed before first click", {
      before: {
        selectedSemanticId: beforeSelectedSemanticId,
        selectedRow: selectedRowOf(rowGeometry),
        mouseArmedRow: mouseArmedRowOf(rowGeometry),
      },
      requestedRow,
    });
  }

  const point = centerOfRect(requestedBounds);
  const eventTarget = { type: "kind", kind: "actionsDialog" };
  const mouseMove = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId("click-activate-move"),
      target: eventTarget,
      event: { type: "mouseMove", x: point.x, y: point.y },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );
  const firstClick = await dispatchLeftClick(args, point, "click-activate-first");

  if (
    !dispatchSuccess(firstClick.mouseDown)
    || !dispatchSuccess(firstClick.mouseUp)
    || dispatchPath(firstClick.mouseDown) !== "exact_handle"
    || dispatchPath(firstClick.mouseUp) !== "exact_handle"
  ) {
    return clickActivateProofBlocked("first click did not dispatch through exact actions dialog handle", {
      requestedRow,
      point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
      receipts: { mouseMove, ...firstClick },
    });
  }

  const startedAt = Date.now();
  const attempts: JsonObject[] = [];
  let stateAfterArm: JsonObject | null = null;
  let actionsDialogAfterArm: JsonObject | null = null;
  let rowGeometryAfterArm: JsonObject | null = null;
  while (Date.now() - startedAt < Math.min(args.timeoutMs, 1500)) {
    stateAfterArm = await rpc(
      args.session,
      { type: "getState", requestId: requestId("click-activate-arm-state"), target: selector, summaryOnly: true },
      "stateResult",
      args.timeoutMs,
    );
    actionsDialogAfterArm = stateActionsDialog(stateAfterArm);
    rowGeometryAfterArm = (actionsDialogAfterArm?.rowGeometry as JsonObject | undefined) ?? null;
    const selectedSemanticId = selectedRowSemanticId(rowGeometryAfterArm);
    const armedSemanticId = mouseArmedRowSemanticId(rowGeometryAfterArm);
    attempts.push({
      elapsedMs: Date.now() - startedAt,
      selectedSemanticId,
      armedSemanticId,
      selectedRow: selectedRowOf(rowGeometryAfterArm),
      mouseArmedRow: mouseArmedRowOf(rowGeometryAfterArm),
    });
    if (selectedSemanticId === requestedSemanticId && armedSemanticId === requestedSemanticId) break;
    await Bun.sleep(50);
  }

  const afterArmSelectedSemanticId = selectedRowSemanticId(rowGeometryAfterArm);
  const afterArmArmedSemanticId = mouseArmedRowSemanticId(rowGeometryAfterArm);
  const firstClickSelectedRequestedRow = afterArmSelectedSemanticId === requestedSemanticId;
  const firstClickArmedRequestedRow = afterArmArmedSemanticId === requestedSemanticId;
  if (!firstClickSelectedRequestedRow || !firstClickArmedRequestedRow || !actionsDialogAfterArm) {
    return clickActivateProofBlocked("first click did not select and arm the requested row", {
      requestedRow,
      point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
      before: {
        selectedSemanticId: beforeSelectedSemanticId,
        selectedRow: selectedRowOf(rowGeometry),
        mouseArmedRow: mouseArmedRowOf(rowGeometry),
      },
      firstClick: {
        receipts: { mouseMove, mouseDown: firstClick.mouseDown, mouseUp: firstClick.mouseUp },
        stateAfterArm,
      },
      assertions: {
        firstClickSelectedRequestedRow,
        firstClickArmedRequestedRow,
      },
      attempts,
    });
  }

  const secondClick = await dispatchLeftClick(args, point, "click-activate-second");
  const secondClickDispatchedExactHandle = dispatchSuccess(secondClick.mouseDown)
    && dispatchSuccess(secondClick.mouseUp)
    && dispatchPath(secondClick.mouseDown) === "exact_handle"
    && dispatchPath(secondClick.mouseUp) === "exact_handle";

  const sourceTarget = await run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--timeout",
    String(args.timeoutMs),
  ], "targets.inspect.actionsDialog.afterActivation");
  const parentAutomationId = target.parentAutomationId ?? target.openerAutomationId ?? "main";
  const parentTarget = await run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--target-id",
    String(parentAutomationId),
    "--strict",
    "--timeout",
    String(args.timeoutMs),
  ], "targets.inspect.parent.afterActivation");

  const sourceClosed = sourceTarget.classification !== "ok";
  const parentResolved = (parentTarget.resolvedTarget as JsonObject | undefined) ?? {};
  const parentLive = parentTarget.classification === "ok" && Boolean(parentResolved.automationId);
  const assertions = {
    rowBoundsAvailable: Boolean(requestedBounds && requestedBounds.width > 0 && requestedBounds.height > 0),
    requestedSafeAction: requestedRow.actionId === "toggle_info",
    firstClickSelectedRequestedRow,
    firstClickArmedRequestedRow,
    secondClickDispatchedExactHandle,
    sourceClosed,
    parentLive,
  };

  if (!Object.values(assertions).every(Boolean)) {
    return clickActivateProofBlocked("second click did not activate only the requested actions dialog row", {
      requestedRow,
      point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
      before: {
        selectedSemanticId: beforeSelectedSemanticId,
        selectedRow: selectedRowOf(rowGeometry),
        mouseArmedRow: mouseArmedRowOf(rowGeometry),
      },
      firstClick: {
        receipts: { mouseMove, mouseDown: firstClick.mouseDown, mouseUp: firstClick.mouseUp },
        stateAfterArm,
      },
      secondClick: { receipts: secondClick },
      after: { sourceTarget, parentTarget },
      assertions,
      attempts,
    });
  }

  return {
    classification: "ok",
    command: "actions.clickActivateProof",
    safety: {
      noNativeEscalation: true,
      submitAttempted: true,
      activationAttempted: true,
      destructiveActionAllowed: false,
    },
    requestedRow: {
      semanticId: requestedSemanticId,
      visualIndex: requestedRow.visualIndex ?? null,
      kind: requestedRow.kind ?? null,
      actionId: requestedRow.actionId ?? null,
      bounds: requestedBounds,
    },
    point: { ...point, coordinateSpace: "popupLogicalPx", target: eventTarget },
    before: {
      selectedSemanticId: beforeSelectedSemanticId,
      selectedRow: selectedRowOf(rowGeometry),
      mouseArmedRow: mouseArmedRowOf(rowGeometry),
    },
    firstClick: {
      receipts: { mouseMove, mouseDown: firstClick.mouseDown, mouseUp: firstClick.mouseUp },
      stateAfterArm,
    },
    secondClick: { receipts: secondClick },
    after: { sourceTarget, parentTarget },
    assertions,
    attempts,
  };
}

function panelOnlyFallbackWarnings(receipt: JsonObject) {
  const warnings = Array.isArray(receipt.warnings) ? receipt.warnings : [];
  return warnings.filter((warning): warning is string =>
    typeof warning === "string" && warning.startsWith("panel_only_")
  );
}

async function runSemanticFreshnessProof(
  args: Args,
  forwarded: string[],
  expectedSelectedSemanticId: string | null,
) {
  const elementsAfter = await run(
    ["bun", "scripts/devtools/elements.ts", "snapshot", ...forwarded],
    "elements.snapshot.semanticFreshness",
  );
  const selectedNode = asArray(elementsAfter.nodes).find((node) =>
    node.semanticId === elementsAfter.selectedSemanticId && node.selected === true
  ) ?? null;
  const fallbackWarnings = panelOnlyFallbackWarnings(elementsAfter);
  const target = (elementsAfter.target as JsonObject | undefined) ?? {};
  const assertions = {
    expectedSelectedSemanticIdAvailable: typeof expectedSelectedSemanticId === "string",
    elementsSelectedMatchesRowGeometry: elementsAfter.selectedSemanticId === expectedSelectedSemanticId,
    selectedNodeMatches: selectedNode != null && selectedNode.semanticId === expectedSelectedSemanticId,
    noPanelOnlyFallback: fallbackWarnings.length === 0,
    targetStable: target.automationId === "actions-dialog",
  };
  const ok = Object.values(assertions).every(Boolean);
  return {
    classification: ok ? "ok" : "blocked-by-stale-view",
    command: "actions.semanticFreshnessProof",
    expected: {
      selectedSemanticId: expectedSelectedSemanticId,
      source: "rowGeometry.selectedRow.semanticId",
    },
    elements: {
      selectedSemanticId: elementsAfter.selectedSemanticId ?? null,
      focusedSemanticId: elementsAfter.focusedSemanticId ?? null,
      selectedNode,
      panelOnlyFallbackWarnings: fallbackWarnings,
    },
    assertions,
    receipts: { elementsAfter },
  };
}

async function runCloseCleanupProof(
  args: Args,
  clickActivateProof: JsonObject | null,
) {
  const activationProofAvailable = clickActivateProof?.classification === "ok";
  if (!activationProofAvailable) {
    return closeCleanupProofBlocked("close cleanup proof requires a green clickActivateProof", {
      clickActivateProof,
    });
  }

  const sourceTarget = await run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--timeout",
    String(args.timeoutMs),
  ], "targets.inspect.actionsDialog.closeCleanup");
  const elementsAfterClose = await run([
    "bun",
    "scripts/devtools/elements.ts",
    "snapshot",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--timeout",
    String(args.timeoutMs),
  ], "elements.snapshot.actionsDialog.closeCleanup");
  const staleDispatch = await rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId("close-cleanup-stale-event"),
      target: { type: "kind", kind: "actionsDialog" },
      event: { type: "mouseMove", x: 1, y: 1 },
    },
    "simulateGpuiEventResult",
    args.timeoutMs,
  );
  const staleDispatchResponse = responseOf(staleDispatch);
  const parentTarget = ((clickActivateProof.after as JsonObject | undefined)?.parentTarget as JsonObject | undefined) ?? null;
  const staleDispatchFailed = staleDispatchResponse.success === false || staleDispatch.success === false;
  const assertions = {
    activationProofAvailable,
    sourceTargetGone: sourceTarget.classification !== "ok",
    elementsNotFresh: elementsAfterClose.classification !== "ok" || asNumber(elementsAfterClose.returnedCount, 0) === 0,
    staleEventRefused: staleDispatchFailed,
    noExactHandleDispatchAfterClose: dispatchPath(staleDispatch) !== "exact_handle",
    parentLive: parentTarget?.classification === "ok",
  };
  const ok = Object.values(assertions).every(Boolean);

  return {
    classification: ok ? "ok" : "blocked-by-stale-view",
    command: "actions.closeCleanupProof",
    assertions,
    receipts: {
      sourceTarget,
      elementsAfterClose,
      staleDispatch,
      parentTarget,
    },
  };
}

function numericValues(samples: JsonObject[], path: (sample: JsonObject) => unknown) {
  return samples
    .map(path)
    .filter((value): value is number => typeof value === "number" && Number.isFinite(value));
}

function monotonicNonDecreasing(values: number[]) {
  return values.every((value, index) => index === 0 || value >= values[index - 1]);
}

function targetOfSample(sample: JsonObject) {
  return (sample.target as JsonObject | undefined) ?? {};
}

function stateActionsOfSample(sample: JsonObject) {
  return (sample.actionsDialog as JsonObject | undefined) ?? {};
}

function summarizeShortcutGenerations(samples: JsonObject[]) {
  const targetGeneration = numericValues(samples, (sample) => targetOfSample(sample).targetGeneration);
  const surfaceGeneration = numericValues(samples, (sample) => targetOfSample(sample).surfaceGeneration);
  const dataGeneration = numericValues(samples, (sample) => targetOfSample(sample).dataGeneration);
  const attachedPopupGeneration = numericValues(samples, (sample) => {
    const actionsDialog = stateActionsOfSample(sample);
    const attachedPopup = (actionsDialog.attachedPopup as JsonObject | undefined) ?? {};
    return attachedPopup.generation;
  });
  return {
    targetGeneration,
    surfaceGeneration,
    dataGeneration,
    attachedPopupGeneration,
    fieldsChecked: [
      targetGeneration.length >= 2 ? "targetGeneration" : "",
      surfaceGeneration.length >= 2 ? "surfaceGeneration" : "",
      dataGeneration.length >= 2 ? "dataGeneration" : "",
      attachedPopupGeneration.length >= 2 ? "attachedPopupGeneration" : "",
    ].filter(Boolean),
    missingOrSingleSampleFields: [
      targetGeneration.length < 2 ? "targetGeneration" : "",
      surfaceGeneration.length < 2 ? "surfaceGeneration" : "",
      dataGeneration.length < 2 ? "dataGeneration" : "",
    ].filter(Boolean),
    generationMonotonicWhenAvailable: monotonicNonDecreasing(targetGeneration)
      && monotonicNonDecreasing(surfaceGeneration)
      && monotonicNonDecreasing(dataGeneration)
      && monotonicNonDecreasing(attachedPopupGeneration),
  };
}

function actionSampleFresh(sample: JsonObject) {
  const target = targetOfSample(sample);
  return target.automationId === "actions-dialog"
    && target.targetKind === "ActionsDialog"
    && target.strictTargetMatch === true;
}

function parentStillMainScriptList(sample: JsonObject) {
  const target = targetOfSample(sample);
  return (target.parentAutomationId ?? target.parentWindowId ?? target.openerAutomationId) === "main";
}

function chromeContractOk(sample: JsonObject) {
  const actionsDialog = stateActionsOfSample(sample);
  const runtimeAudit = (actionsDialog.runtimeAudit as JsonObject | undefined) ?? null;
  const violations = asArray(actionsDialog.runtimeAuditViolations);
  const status = typeof actionsDialog.runtimeAuditStatus === "string"
    ? actionsDialog.runtimeAuditStatus
    : runtimeAudit
      ? violations.length === 0 ? "ok" : "violation"
      : "unavailable";
  return status === "ok";
}

function actionsDialogFooterless(sample: JsonObject) {
  const keyboard = (sample.keyboard as JsonObject | undefined) ?? {};
  const activeFooter = (keyboard.activeFooter as JsonObject | undefined) ?? {};
  const nativeFooter = (keyboard.nativeFooter as JsonObject | undefined) ?? {};
  return activeFooter.owner == null
    || activeFooter.owner === "main"
    || nativeFooter.hostInstalled === false
    || asArray(nativeFooter.bindings).length === 0;
}

function noMainFooterLeak(sample: JsonObject) {
  const mainKeyboard = (sample.mainKeyboard as JsonObject | undefined) ?? {};
  const activeFooter = (mainKeyboard.activeFooter as JsonObject | undefined) ?? {};
  const owner = activeFooter.owner;
  const expectedSurface = activeFooter.expectedSurface;
  const activeSurface = activeFooter.activeSurface;
  const buttonCount = asNumber(activeFooter.buttonCount, 0);
  const surfaceMatches = expectedSurface == null || activeSurface == null || activeSurface === expectedSurface;
  return ((owner == null || owner === "main" || owner === "native") && surfaceMatches)
    || (owner === "popup" && buttonCount === 0 && surfaceMatches);
}

function mainStillScriptList(targetReceipt: JsonObject) {
  const target = (targetReceipt.resolvedTarget as JsonObject | undefined) ?? {};
  return target.automationId === "main"
    && target.targetKind === "Main"
    && target.strictTargetMatch === true
    && target.surfaceKind === "ScriptList";
}

function noActivePopup(sample: JsonObject) {
  const mainState = (sample.mainState as JsonObject | undefined) ?? {};
  return mainState.activePopupContract == null;
}

async function inspectMainTarget(args: Args, label: string) {
  return run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--main",
    "--strict",
    "--surface",
    "ScriptList",
    "--timeout",
    String(args.timeoutMs),
  ], label);
}

async function getMainState(args: Args, label: string) {
  return rpc(
    args.session,
    { type: "getState", requestId: requestId(label), target: { type: "main" }, summaryOnly: true },
    "stateResult",
    args.timeoutMs,
  );
}

async function inspectMainKeyboard(args: Args, label: string) {
  return run([
    "bun",
    "scripts/devtools/keyboard.ts",
    "inspect",
    "--session",
    args.session,
    "--main",
    "--strict",
    "--surface",
    "ScriptList",
    "--timeout",
    String(args.timeoutMs),
  ], label);
}

async function inspectActionsTarget(args: Args, label: string) {
  return run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--timeout",
    String(args.timeoutMs),
  ], label);
}

async function inspectActionsTargetFast(args: Args, label: string) {
  return run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--timeout",
    String(Math.min(args.timeoutMs, 1000)),
  ], label);
}

async function snapshotActionsElementsFast(args: Args, label: string) {
  return run([
    "bun",
    "scripts/devtools/elements.ts",
    "snapshot",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--timeout",
    String(Math.min(args.timeoutMs, 1000)),
  ], label);
}

async function staleActionsMouseMove(args: Args, label: string) {
  return rpc(
    args.session,
    {
      type: "simulateGpuiEvent",
      requestId: requestId(label),
      target: { type: "kind", kind: "actionsDialog" },
      event: { type: "mouseMove", x: 1, y: 1 },
    },
    "simulateGpuiEventResult",
    Math.min(args.timeoutMs, 1000),
  );
}

async function getActionsState(args: Args, label: string) {
  return rpc(
    args.session,
    { type: "getState", requestId: requestId(label), target: { type: "kind", kind: "actionsDialog" }, summaryOnly: true },
    "stateResult",
    args.timeoutMs,
  );
}

async function inspectActionsKeyboard(args: Args, label: string) {
  return run([
    "bun",
    "scripts/devtools/keyboard.ts",
    "inspect",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--timeout",
    String(args.timeoutMs),
  ], label);
}

async function runShortcutOpenFreshnessProof(args: Args) {
  const beforeMainTarget = await inspectMainTarget(args, "targets.inspect.main.beforeShortcut");
  const beforeMainState = await getMainState(args, "shortcut-open-main-before");
  const beforeMainKeyboard = await inspectMainKeyboard(args, "keyboard.inspect.main.beforeShortcut");

  const beforeResolved = (beforeMainTarget.resolvedTarget as JsonObject | undefined) ?? {};
  if (beforeMainTarget.classification !== "ok" || beforeResolved.automationId !== "main") {
    return shortcutOpenFreshnessProofBlocked("blocked-by-target-ambiguity", "main ScriptList target was not ready before Cmd+K", {
      before: { mainTarget: beforeMainTarget, mainState: beforeMainState, mainKeyboard: beforeMainKeyboard },
    });
  }

  const keyReceipt = await run([
    "bun",
    "scripts/devtools/act.ts",
    "key",
    "--session",
    args.session,
    "--show",
    "--main",
    "--strict",
    "--surface",
    "ScriptList",
    "--key",
    "k",
    "--modifiers",
    "cmd",
    "--timeout",
    String(args.timeoutMs),
  ], "act.key.cmdK.openActions");

  const startedAt = Date.now();
  const samples: JsonObject[] = [];
  while (Date.now() - startedAt < args.sampleMs || samples.length < 3) {
    const targetReceipt = await inspectActionsTarget(args, "targets.inspect.actionsDialog.shortcutOpen");
    if (targetReceipt.classification === "ok") {
      const stateEnvelope = await getActionsState(args, "shortcut-open-actions-state");
      const keyboard = await inspectActionsKeyboard(args, "keyboard.inspect.actionsDialog.shortcutOpen");
      const mainState = await getMainState(args, "shortcut-open-main-state");
      const mainKeyboard = await inspectMainKeyboard(args, "keyboard.inspect.main.shortcutOpen");
      const state = responseOf(stateEnvelope);
      samples.push({
        elapsedMs: Date.now() - startedAt,
        target: targetReceipt.resolvedTarget ?? null,
        actionsDialog: state.actionsDialog ?? null,
        keyboard,
        mainState: responseOf(mainState),
        mainKeyboard,
        receipts: { target: targetReceipt, state: stateEnvelope, keyboard, mainState, mainKeyboard },
      });
    } else {
      samples.push({
        elapsedMs: Date.now() - startedAt,
        target: targetReceipt.resolvedTarget ?? null,
        errors: targetReceipt.errors ?? [],
        receipts: { target: targetReceipt },
      });
    }
    if (Date.now() - startedAt >= args.sampleMs && samples.length >= 3) break;
    await Bun.sleep(args.intervalMs);
  }

  const firstObservableFrame = samples.find(actionSampleFresh) ?? null;
  const generation = summarizeShortcutGenerations(samples.filter(actionSampleFresh));
  const keyResponse = responseOf(keyReceipt);
  const assertions = {
    cmdKDispatched: keyReceipt.status !== "error" && keyResponse.success !== false,
    openedViaShortcut: firstObservableFrame != null,
    firstObservableFrameTargetStable: firstObservableFrame != null && actionSampleFresh(firstObservableFrame),
    everySampleTargetStable: samples.filter(actionSampleFresh).length >= 3 && samples.filter(actionSampleFresh).every(actionSampleFresh),
    parentStillMainScriptList: samples.filter(actionSampleFresh).every(parentStillMainScriptList),
    attachedPopupGenerationAvailable: generation.targetGeneration.length >= 2 || generation.attachedPopupGeneration.length >= 2,
    generationMonotonicWhenAvailable: generation.generationMonotonicWhenAvailable,
    noStalePopupGeneration: generation.targetGeneration.length === 0 || new Set(generation.targetGeneration).size <= generation.targetGeneration.length,
    chromeContractOk: samples.filter(actionSampleFresh).every(chromeContractOk),
    actionsDialogFooterless: samples.filter(actionSampleFresh).every(actionsDialogFooterless),
    noFooterOwnershipLeak: samples.filter(actionSampleFresh).every(noMainFooterLeak),
  };
  const missingGeneration = !assertions.attachedPopupGenerationAvailable;
  const ok = Object.values(assertions).every(Boolean);
  const classification = ok
    ? "ok"
    : missingGeneration
      ? "blocked-by-missing-primitive"
      : "blocked-by-stale-view";

  return {
    classification,
    command: "actions.shortcutOpenFreshnessProof",
    assertions,
    generation,
    firstObservableFrame,
    samplesAfterShortcut: samples,
    safety: {
      noNativeEscalation: true,
      shortcutDispatched: true,
      submitAttempted: false,
      activationAttempted: false,
    },
    receipts: {
      beforeMainTarget,
      beforeMainState,
      beforeMainKeyboard,
      keyReceipt,
    },
  };
}

async function runCloseKeyCleanupProof(
  args: Args,
  options: {
    command: string;
    closeKey: string;
    closeModifiers: string[];
    closeAssertion: string;
    blocker: (
      classification: "blocked-by-missing-primitive" | "blocked-by-stale-view",
      reason: string,
      extras?: JsonObject,
    ) => JsonObject;
  },
) {
  const beforeMainTarget = await inspectMainTarget(args, "targets.inspect.main.beforeShortcutClose");
  const beforeMainState = await getMainState(args, "shortcut-close-main-before");
  const beforeMainKeyboard = await inspectMainKeyboard(args, "keyboard.inspect.main.beforeShortcutClose");
  if (beforeMainTarget.classification !== "ok" || !mainStillScriptList(beforeMainTarget)) {
    return options.blocker("blocked-by-stale-view", "main ScriptList target was not ready before opening ActionsDialog", {
      before: { beforeMainTarget, beforeMainState, beforeMainKeyboard },
    });
  }

  const openKeyReceipt = await run([
    "bun",
    "scripts/devtools/act.ts",
    "key",
    "--session",
    args.session,
    "--show",
    "--main",
    "--strict",
    "--surface",
    "ScriptList",
    "--key",
    "k",
    "--modifiers",
    "cmd",
    "--timeout",
    String(args.timeoutMs),
  ], "act.key.cmdK.openActionsForClose");

  let openedTarget: JsonObject | null = null;
  const openAttempts: JsonObject[] = [];
  const openStartedAt = Date.now();
  while (Date.now() - openStartedAt < args.timeoutMs) {
    const receipt = await inspectActionsTarget(args, "targets.inspect.actionsDialog.beforeShortcutClose");
    openAttempts.push(receipt);
    if (receipt.classification === "ok") {
      openedTarget = receipt;
      break;
    }
    await Bun.sleep(args.intervalMs);
  }

  if (!openedTarget) {
    return options.blocker("blocked-by-stale-view", "ActionsDialog did not open before close key dispatch", {
      receipts: { beforeMainTarget, beforeMainState, beforeMainKeyboard, openKeyReceipt },
      openAttempts,
    });
  }

  const closeKeyReceipt = await run([
    "bun",
    "scripts/devtools/act.ts",
    "key",
    "--session",
    args.session,
    "--target-kind",
    "actionsDialog",
    "--strict",
    "--surface",
    "ActionsDialog",
    "--key",
    options.closeKey,
    ...(options.closeModifiers.length > 0 ? ["--modifiers", options.closeModifiers.join(",")] : []),
    "--timeout",
    String(args.timeoutMs),
  ], `act.key.${options.closeKey}.closeActions`);

  const closeStartedAt = Date.now();
  const samplesAfterClose: JsonObject[] = [];
  while (Date.now() - closeStartedAt < args.sampleMs || samplesAfterClose.length < 3) {
    const sourceTarget = await inspectActionsTargetFast(args, "targets.inspect.actionsDialog.afterShortcutClose");
    const elementsAfterClose = await snapshotActionsElementsFast(args, "elements.snapshot.actionsDialog.afterShortcutClose");
    const staleDispatch = await staleActionsMouseMove(args, "shortcut-close-stale-event");
    const mainTarget = await inspectMainTarget(args, "targets.inspect.main.afterShortcutClose");
    const mainState = await getMainState(args, "shortcut-close-main-after");
    const mainKeyboard = await inspectMainKeyboard(args, "keyboard.inspect.main.afterShortcutClose");
    samplesAfterClose.push({
      elapsedMs: Date.now() - closeStartedAt,
      sourceTarget,
      elementsAfterClose,
      staleDispatch,
      mainTarget,
      mainState: responseOf(mainState),
      mainKeyboard,
    });
    if (Date.now() - closeStartedAt >= args.sampleMs && samplesAfterClose.length >= 3) break;
    await Bun.sleep(args.intervalMs);
  }

  const assertions = {
    cmdKOpenDispatched: openKeyReceipt.status !== "error" && responseOf(openKeyReceipt).success !== false,
    openedViaShortcut: openedTarget.classification === "ok",
    [options.closeAssertion]: closeKeyReceipt.status !== "error" && responseOf(closeKeyReceipt).success !== false,
    sourceTargetGone: samplesAfterClose.every((sample) =>
      (sample.sourceTarget as JsonObject | undefined)?.classification !== "ok"
    ),
    elementsNotFresh: samplesAfterClose.every((sample) => {
      const elementsAfterClose = (sample.elementsAfterClose as JsonObject | undefined) ?? {};
      return elementsAfterClose.classification !== "ok" || asNumber(elementsAfterClose.returnedCount, 0) === 0;
    }),
    staleEventRefused: samplesAfterClose.every((sample) => {
      const staleDispatch = (sample.staleDispatch as JsonObject | undefined) ?? {};
      const response = responseOf(staleDispatch);
      return response.success === false || staleDispatch.success === false;
    }),
    noExactHandleDispatchAfterClose: samplesAfterClose.every((sample) => dispatchPath((sample.staleDispatch as JsonObject | undefined) ?? {}) !== "exact_handle"),
    parentLiveAfterClose: samplesAfterClose.every((sample) => (sample.mainTarget as JsonObject | undefined)?.classification === "ok"),
    parentScriptListStableAfterClose: samplesAfterClose.every((sample) => mainStillScriptList((sample.mainTarget as JsonObject | undefined) ?? {})),
    noActivePopupAfterClose: samplesAfterClose.every(noActivePopup),
    noFooterOwnershipLeakAfterClose: samplesAfterClose.every(noMainFooterLeak),
  };
  const ok = Object.values(assertions).every(Boolean);
  return {
    classification: ok ? "ok" : "blocked-by-stale-view",
    command: options.command,
    assertions,
    openedTarget: (openedTarget.resolvedTarget as JsonObject | undefined) ?? null,
    samplesAfterClose,
    safety: {
      noNativeEscalation: true,
      shortcutDispatched: true,
      submitAttempted: false,
      activationAttempted: false,
    },
    receipts: {
      beforeMainTarget,
      beforeMainState,
      beforeMainKeyboard,
      openKeyReceipt,
      closeKeyReceipt,
      openAttempts,
    },
  };
}

async function runShortcutCloseCleanupProof(args: Args) {
  return runCloseKeyCleanupProof(args, {
    command: "actions.shortcutCloseCleanupProof",
    closeKey: "k",
    closeModifiers: ["cmd"],
    closeAssertion: "cmdKCloseDispatched",
    blocker: shortcutCloseCleanupProofBlocked,
  });
}

async function runEscapeCloseCleanupProof(args: Args) {
  return runCloseKeyCleanupProof(args, {
    command: "actions.escapeCloseCleanupProof",
    closeKey: "escape",
    closeModifiers: [],
    closeAssertion: "escapeCloseDispatched",
    blocker: escapeCloseCleanupProofBlocked,
  });
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  const forwarded = inspectForwarded(args);
  const startReceipt = await maybeStartSession(args);
  const parentOpenReceipt = await maybeOpenParentTarget(args);
  const openReceipt = await maybeOpenActions(args);
  const targetReadiness = args.open
    ? await waitForActionsDialogTarget(args, forwarded)
    : null;
  const targetReceipt = targetReadiness?.status === "ok"
    ? (targetReadiness.receipt as JsonObject)
    : await run(["bun", "scripts/devtools/targets.ts", "inspect", ...forwarded], "targets.inspect");

  const targetsList = await run(
    ["bun", "scripts/devtools/targets.ts", "list", "--session", args.session, "--timeout", String(args.timeoutMs)],
    "targets.list",
  );
  const selector: JsonObject = ((targetReceipt.requestedTarget as JsonObject | undefined)?.selector as JsonObject | undefined) ?? args.target ?? { type: "kind", kind: "actionsDialog" };
  const stateEnvelope = await rpc(
    args.session,
    { type: "getState", requestId: requestId("state"), target: selector, summaryOnly: true },
    "stateResult",
    args.timeoutMs,
  );
  const elements = await run(["bun", "scripts/devtools/elements.ts", "snapshot", ...forwarded], "elements.snapshot");
  const layout = await run(["bun", "scripts/devtools/layout.ts", "measure", ...forwarded], "layout.measure");
  const keyboard = await run(["bun", "scripts/devtools/keyboard.ts", "inspect", ...forwarded], "keyboard.inspect");

  const state = responseOf(stateEnvelope);
  const actionsDialog = (state.actionsDialog as JsonObject | undefined) ?? null;
  const attachedPopup = (actionsDialog?.attachedPopup as JsonObject | undefined) ?? null;
  const attachedGeometry = (attachedPopup?.geometry as JsonObject | undefined) ?? {};
  const rowGeometry = (actionsDialog?.rowGeometry as JsonObject | undefined) ?? null;
  const runtimeAudit = (actionsDialog?.runtimeAudit as JsonObject | undefined) ?? null;
  const runtimeAuditViolations = asArray(actionsDialog?.runtimeAuditViolations);
  const runtimeAuditStatus = typeof actionsDialog?.runtimeAuditStatus === "string"
    ? actionsDialog.runtimeAuditStatus
    : runtimeAudit
      ? runtimeAuditViolations.length === 0 ? "ok" : "violation"
      : "unavailable";
  const target: JsonObject = (targetReceipt.resolvedTarget as JsonObject | undefined) ?? {};
  const windows = asArray(targetsList.targets ?? targetsList.windows);
  const parent = findParentTarget(target, windows);
  const popupRect = rectFrom(attachedGeometry.popupRect) ?? rectFrom(target.bounds);
  const parentRect = rectFrom(attachedGeometry.parentRect) ?? rectFrom(parent?.bounds);
  const anchorRect = rectFrom(attachedGeometry.anchorRect);
  const rows = visibleActionRows(actionsDialog);
  const actionProjection = auditActionProjection(rows);
  const sections = groupActionSections(rows);
  const clipping = edgeClipping(popupRect, parentRect);
  const dialogRoute = (actionsDialog?.route as JsonObject | undefined) ?? {};
  const targetRouteStack = asArray(target.routeStack);
  const routeStack = targetRouteStack.length > 0 ? targetRouteStack : asArray(dialogRoute.stack);
  const routeStateAvailable = Boolean(actionsDialog && Object.prototype.hasOwnProperty.call(actionsDialog, "route"));
  const shortcutRows = rows.filter((row) => row.shortcut);
  const runtimeShortcutLayout = (rowGeometry?.shortcutLayout as JsonObject | undefined) ?? null;
  const destructiveRows = rows.filter((row) => row.destructive);
  const disabledRows = rows.filter((row) => row.enabled === false || row.disabledReason);
  const disabledReasonBoundsRequired = disabledRows.length > 0;
  const sectionBoundsAvailable = rowGeometry?.sectionBoundsAvailable === true;
  const hoverRowAvailable = rowGeometry?.hoverRowAvailable === true;
  const shortcutBoundsAvailable = rowGeometry?.shortcutBoundsAvailable === true;
  const disabledReasonBoundsAvailable = rowGeometry?.disabledReasonBoundsAvailable === true;
  const missing = [
    actionsDialog ? "" : "actionsDialogState",
    routeStateAvailable ? "" : "route stack",
    popupRect ? "" : "popup rect",
    parentRect ? "" : "parent target rect",
    anchorRect ? "" : "anchor rect",
    sectionBoundsAvailable ? "" : "section bounds",
    hoverRowAvailable ? "" : "hover row",
    shortcutBoundsAvailable ? "" : "shortcut layout bounds",
    disabledReasonBoundsRequired && !disabledReasonBoundsAvailable ? "disabled reason bounds" : "",
    actionProjection.complete ? "" : "complete executable action projection",
  ].filter(Boolean);
  const classification = classify(targetReceipt, stateEnvelope, missing);
  const hoverProof = args.proveHover
    ? await runHoverProof(args, selector, target, rowGeometry)
    : null;
  const clickSelectProof = args.proveClickSelect
    ? await runClickSelectProof(args, selector, target, rowGeometry)
    : null;
  const clickActivateProof = args.proveClickActivate
    ? await runClickActivateProof(args, selector, target, rowGeometry)
    : null;
  const clickSelectProofAfter = ((clickSelectProof as JsonObject | null)?.after as JsonObject | undefined) ?? null;
  const semanticExpectedSelectedId = typeof clickSelectProofAfter?.selectedSemanticId === "string"
    ? clickSelectProofAfter.selectedSemanticId
    : selectedRowSemanticId(rowGeometry);
  const semanticFreshnessProof = args.proveSemanticFreshness
    ? await runSemanticFreshnessProof(args, forwarded, semanticExpectedSelectedId)
    : null;
  const closeCleanupProof = args.proveCloseCleanup
    ? await runCloseCleanupProof(args, clickActivateProof)
    : null;
  const shortcutOpenFreshnessProof = args.proveShortcutOpenFreshness
    ? await runShortcutOpenFreshnessProof(args)
    : null;
  const shortcutCloseCleanupProof = args.proveShortcutCloseCleanup
    ? await runShortcutCloseCleanupProof(args)
    : null;
  const escapeCloseCleanupProof = args.proveEscapeCloseCleanup
    ? await runEscapeCloseCleanupProof(args)
    : null;
  const finalClassification = firstNonOkClassification([
    classification,
    hoverProof?.classification,
    clickSelectProof?.classification,
    clickActivateProof?.classification,
    semanticFreshnessProof?.classification,
    closeCleanupProof?.classification,
    shortcutOpenFreshnessProof?.classification,
    shortcutCloseCleanupProof?.classification,
    escapeCloseCleanupProof?.classification,
  ]);

  emitValidatedReceipt("devtools.actions.inspect", {
    schemaVersion: 2,
    tool: "script-kit-devtools.actions",
    command: "actions.inspect",
    classification: finalClassification,
    session: args.session,
    requestedTarget: targetReceipt.requestedTarget ?? { selector },
    target,
    transaction: targetReceipt.transaction,
    parentTarget: parent,
    startReceipt: diagnostic(startReceipt),
    parentOpenReceipt: diagnostic(parentOpenReceipt),
    openReceipt: diagnostic(openReceipt),
    targetReadiness,
    popupState: {
      open: Boolean(actionsDialog),
      host: actionsDialog?.host ?? null,
      contextTitle: userContent(actionsDialog?.contextTitle ?? null),
      contextStableKey: actionsDialog?.contextStableKey ?? null,
      contextSource: actionsDialog?.contextSource ?? null,
      selectedActionId: ((actionsDialog?.selection as JsonObject | undefined)?.actionId ?? actionsDialog?.selectedActionId) ?? null,
    },
    attachedPopup,
    routeStack,
    routeId: target.routeId ?? dialogRoute.currentRouteId ?? null,
    rowGeometry,
    hoverProof,
    clickSelectProof,
    clickActivateProof,
    semanticFreshnessProof,
    closeCleanupProof,
    shortcutOpenFreshnessProof,
    shortcutCloseCleanupProof,
    escapeCloseCleanupProof,
    chromeContract: {
      source: "actionsDialog.automationState.runtimeAudit",
      status: runtimeAuditStatus,
      audit: runtimeAudit,
      violations: runtimeAuditViolations,
    },
    geometry: {
      layoutPrimitive: "getLayoutInfo(actionsDialog)",
      popupRect,
      parentRect,
      anchorRect,
      clippingEdges: clipping,
      placement: {
        parentAutomationId: attachedPopup?.parentAutomationId ?? target.parentAutomationId ?? null,
        parentWindowId: target.parentWindowId ?? attachedPopup?.parentAutomationId ?? target.parentAutomationId ?? null,
        openerAutomationId: target.openerAutomationId ?? null,
        screenId: target.screenId ?? attachedGeometry.displayId ?? attachedPopup?.displayId ?? null,
        pinnedEdge: attachedGeometry.pinnedEdge ?? null,
        position: attachedGeometry.position ?? attachedPopup?.position ?? null,
        generation: attachedPopup?.generation ?? null,
        stale: attachedPopup?.stale ?? null,
      },
      layoutPressure: (layout.resizePressure as JsonObject | undefined) ?? null,
      overlapPairs: layout.overlaps ?? [],
    },
    sections: {
      count: sectionBoundsAvailable ? asArray(rowGeometry?.sections).length : sections.length,
      rows: sectionBoundsAvailable ? asArray(rowGeometry?.sections) : sections,
      boundsAvailable: sectionBoundsAvailable,
    },
    actions: {
      visibleCount: rows.length,
      rows,
      destructiveRows,
      disabledRows,
      projection: actionProjection,
    },
    shortcutLayout: {
      primitive: "runtime shortcut layout bounds",
      rowCount: shortcutBoundsAvailable ? asArray(runtimeShortcutLayout?.rows).length : shortcutRows.length,
      rows: shortcutBoundsAvailable ? asArray(runtimeShortcutLayout?.rows) : shortcutRows.map((row) => ({
        id: row.id,
        label: row.label,
        shortcut: row.shortcut,
        shortcutTokens: row.shortcutTokens,
        tokenCount: row.shortcutTokens.length,
        bounds: null,
      })),
      boundsAvailable: shortcutBoundsAvailable,
      runtimeBoundsAvailable: shortcutBoundsAvailable,
      measurementSource: runtimeShortcutLayout?.measurementSource ?? null,
      stopReason: runtimeShortcutLayout?.stopReason ?? (shortcutBoundsAvailable ? null : "runtime shortcut layout unavailable"),
    },
    receipts: diagnostic({
      target: targetReceipt,
      state: stateEnvelope,
      elements,
      layout,
      keyboard,
    }),
    missingPrimitives: missing,
    recommendedNext: [
      disabledReasonBoundsRequired && !disabledReasonBoundsAvailable
        ? "Expose disabled reason text bounds when a visible ActionsDialog route renders disabled explanations."
        : "",
      "Add safe click/hover receipts for action rows before native pointer escalation.",
    ].filter(Boolean),
    proofMode: {
      keepOpen: args.keepOpen,
      keepOpenEnv: args.keepOpen ? "SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN=1" : null,
      openTarget: args.openTarget ?? null,
    },
    warnings: [
      missing.length > 0 ? "Actions popup inspection is fail-closed until anchor/section/shortcut geometry is first-class." : "",
      clipping.available && (clipping.top || clipping.right || clipping.bottom || clipping.left) ? "popup rect clips outside parent rect" : "",
    ].filter(Boolean),
    errors: diagnostic(
      [targetReceipt, stateEnvelope, elements, layout, keyboard].filter(
        (receipt) => receipt.status === "error",
      ),
    ),
  });
}

if (import.meta.main) await main();
