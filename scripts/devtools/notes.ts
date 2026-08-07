#!/usr/bin/env bun

import { classifyEnvelopeError } from "./lib/client.ts";
import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic } from "./lib/privacy.ts";

type JsonObject = Record<string, unknown>;

type Args = {
  command: "inspect" | "resize-compare";
  session: string;
  open: boolean;
  openActions: boolean;
  start: boolean;
  sandbox: boolean;
  sandboxPath: string;
  confirmRealNotesMutation: boolean;
  cleanup: boolean;
  timeoutMs: number;
  limit: number;
  shortLineCount: number;
  tallLineCount: number;
};

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/notes.ts inspect [--session <name>] [--open] [--open-actions] [--start] [--limit <n>]",
    "  bun scripts/devtools/notes.ts resize-compare --session <name> --start --sandbox [--short-line-count <n>] [--tall-line-count <n>]",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  if (argv[0] !== "inspect" && argv[0] !== "resize-compare") {
    console.error(usage());
    process.exit(2);
  }
  const args: Args = {
    command: argv[0],
    session: "default",
    open: false,
    openActions: false,
    start: false,
    sandbox: false,
    sandboxPath: "",
    confirmRealNotesMutation: false,
    cleanup: true,
    timeoutMs: 8000,
    limit: 80,
    shortLineCount: 2,
    tallLineCount: 80,
  };
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
    } else if (arg === "--open") {
      args.open = true;
    } else if (arg === "--open-actions") {
      args.openActions = true;
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--sandbox") {
      args.sandbox = true;
    } else if (arg === "--sandbox-path" || arg === "--sandbox-db") {
      args.sandboxPath = argv[++index] ?? "";
      args.sandbox = true;
    } else if (arg === "--confirm-real-notes-mutation") {
      args.confirmRealNotesMutation = true;
    } else if (arg === "--no-cleanup") {
      args.cleanup = false;
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
    } else if (arg === "--limit") {
      args.limit = Number(argv[++index] ?? args.limit);
    } else if (arg === "--short-lines" || arg === "--short-line-count") {
      args.shortLineCount = Number(argv[++index] ?? args.shortLineCount);
    } else if (arg === "--tall-lines" || arg === "--tall-line-count") {
      args.tallLineCount = Number(argv[++index] ?? args.tallLineCount);
    }
  }
  return args;
}

async function run(command: string[], label: string, env: Record<string, string> = {}): Promise<JsonObject> {
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe", env: { ...process.env, ...env } });
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
    return { status: "ok", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
  }
}

async function maybeOpenNotes(args: Args, env: Record<string, string> = {}) {
  if (args.start) {
    await run(["bash", "scripts/agentic/session.sh", "start", args.session], "session-start", env);
  }
  if (!args.open) {
    return null;
  }
  return run([
    "bash",
    "scripts/agentic/session.sh",
    "send",
    args.session,
    JSON.stringify({ type: "openNotes", requestId: `devtools-notes-open-${Date.now()}` }),
  ], "open-notes");
}

async function maybeOpenActions(args: Args) {
  if (!args.openActions) {
    return null;
  }
  const receipt = await run([
    "bash",
    "scripts/agentic/session.sh",
    "rpc",
    args.session,
    JSON.stringify({
      type: "batch",
      requestId: `devtools-notes-open-actions-${Date.now()}`,
      target: { type: "kind", kind: "notes" },
      commands: [{ type: "openActions" }],
      options: { stopOnError: true, rollbackOnError: false, timeout: args.timeoutMs },
      trace: "on",
    }),
    "--expect",
    "batchResult",
    "--timeout",
    String(args.timeoutMs),
  ], "open-actions");
  // Settle on the real observable instead of a fixed sleep: the Notes
  // actions command bar reports open in target-scoped state as soon as
  // the batch applies (normally the first read).
  await pollNotesState(args, "open-actions-settle", notesActionOpen);
  return receipt;
}

function notesRuntimeState(envelope: JsonObject): JsonObject {
  return ((responseOf(envelope).notes as JsonObject | undefined) ?? {});
}

function notesActionOpen(value: JsonObject): boolean {
  const commandBars = (value.commandBars as JsonObject | undefined) ?? {};
  const actions = (commandBars.actions as JsonObject | undefined) ?? {};
  return actions.open === true || ((actions.dialog as JsonObject | undefined)?.open === true);
}

function notesActiveScope(value: JsonObject): unknown {
  const shortcutRegistry = (value.shortcutRegistry as JsonObject | undefined) ?? {};
  return shortcutRegistry.activeScope ?? null;
}

function notesFocusGeneration(value: JsonObject): number | null {
  const focusTransitions = (value.focusTransitions as JsonObject | undefined) ?? {};
  return typeof focusTransitions.generation === "number" ? focusTransitions.generation : null;
}

function hasDraftSnapshot(value: JsonObject): boolean {
  const draftSnapshot = value.draftSnapshot as JsonObject | undefined;
  const draft = draftSnapshot?.draft as JsonObject | undefined;
  return (
    typeof draft?.bodyFingerprint === "string" &&
    typeof draft?.bodyByteLength === "number" &&
    draft?.selectionUnit === "utf8ByteOffset" &&
    draftSnapshot?.contentReturned === false
  );
}

function missingCoveragePrimitives(coverage: ReturnType<typeof notesCoverage>, runtimeNotes: JsonObject): string[] {
  const missing = coverage.missingRuntimePrimitives.map(String);
  return missing.filter((primitive) => {
    if (hasDraftSnapshot(runtimeNotes) && primitive === "draft snapshot fingerprint") {
      return false;
    }
    if (hasNotesAiHandoffReceipt(runtimeNotes) && primitive.includes("notes AI handoff receipt")) {
      return false;
    }
    return true;
  });
}

function shortcutSnapshot(value: JsonObject) {
  return {
    actionsOpen: notesActionOpen(value),
    activeScope: notesActiveScope(value),
    focusTransitionGeneration: notesFocusGeneration(value),
  };
}

function hasNotesAiHandoffReceipt(runtimeNotes: JsonObject): boolean {
  // Redacted notes→main handoff receipt: always present with a stable shape
  // (active:false before any handoff), identity/shape fields only.
  const lastAiHandoff = asObject(runtimeNotes.lastAiHandoff);
  if (lastAiHandoff.redacted !== true) {
    return false;
  }
  if (lastAiHandoff.active === false) {
    return true;
  }
  return (
    lastAiHandoff.active === true &&
    lastAiHandoff.destinationWindowId === "main" &&
    lastAiHandoff.destinationSurface === "agentChat" &&
    typeof lastAiHandoff.targetSemanticId === "string" &&
    typeof lastAiHandoff.targetLabelFingerprint === "string"
  );
}

function buildShortcutActivationReceipt(sendReceipt: JsonObject | null, beforeEnvelope: JsonObject | null, afterEnvelope: JsonObject | null) {
  if (!sendReceipt || !beforeEnvelope || !afterEnvelope) {
    return null;
  }
  const before = notesRuntimeState(beforeEnvelope);
  const after = notesRuntimeState(afterEnvelope);
  const beforeSnapshot = shortcutSnapshot(before);
  const afterSnapshot = shortcutSnapshot(after);
  const delivered = sendReceipt.status === "ok" || sendReceipt.sent === true;
  const opened = afterSnapshot.actionsOpen === true;
  const focusGenerationAdvanced =
    typeof beforeSnapshot.focusTransitionGeneration === "number" &&
    typeof afterSnapshot.focusTransitionGeneration === "number" &&
    afterSnapshot.focusTransitionGeneration > beforeSnapshot.focusTransitionGeneration;

  return {
    schemaVersion: 1,
    shortcut: "Cmd+K",
    channel: "protocol.batch.openActions",
    requestedOwner: "notes.actions",
    delivered,
    before: beforeSnapshot,
    after: afterSnapshot,
    assertions: {
      sendReceiptOk: delivered,
      actionsPanelOpened: opened,
      activeScopeBecameActionsPanel: afterSnapshot.activeScope === "actionsPanel",
      focusTransitionAdvanced: focusGenerationAdvanced,
    },
    // "reproduced" is reserved for a verified failure: the command was
    // delivered but the panel did not open. A failed SEND is a transport
    // problem, not a reproduced app bug — classify it fail-closed instead.
    classification: !delivered
      ? (classifyEnvelopeError(sendReceipt) === "ok"
          ? "blocked-by-response-timeout"
          : classifyEnvelopeError(sendReceipt))
      : opened
        ? "ok"
        : "reproduced",
    failure: delivered && !opened
      ? "target-scoped batch openActions did not open the Notes actions command bar"
      : null,
    receipts: {
      send: sendReceipt,
      beforeState: beforeEnvelope,
      afterState: afterEnvelope,
    },
  };
}

async function waitForNotesTarget(args: Args) {
  const deadline = Date.now() + args.timeoutMs;
  let last: JsonObject = {};
  while (Date.now() < deadline) {
    last = await run([
      "bun",
      "scripts/devtools/targets.ts",
      "inspect",
      "--session",
      args.session,
      "--target-kind",
      "notes",
      "--strict",
    ], "targets.inspect");
    if (last.classification === "ok") {
      return last;
    }
    await Bun.sleep(25);
  }
  return last;
}

/**
 * Poll target-scoped Notes runtime state until `predicate` passes.
 * Replaces fixed settle sleeps with the actual observable; bounded by
 * `timeoutMs` and non-fatal (returns the last envelope either way — the
 * caller's subsequent receipts are the authoritative failure point).
 */
async function pollNotesState(
  args: Args,
  label: string,
  predicate: (runtimeNotes: JsonObject) => boolean,
  timeoutMs = 3000,
): Promise<JsonObject | null> {
  const deadline = Date.now() + timeoutMs;
  let last: JsonObject | null = null;
  while (Date.now() < deadline) {
    last = await rpc(args.session, {
      type: "getState",
      requestId: requestId(label),
      target: { type: "kind", kind: "notes", index: 0 },
    }, "stateResult", timeoutMs);
    if (predicate(notesRuntimeState(last))) {
      return last;
    }
    await Bun.sleep(25);
  }
  return last;
}

function requestId(prefix: string) {
  return `devtools-notes-${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function rpc(session: string, payload: JsonObject, expect: string, timeoutMs: number) {
  return run([
    "bash",
    "scripts/agentic/session.sh",
    "rpc",
    session,
    JSON.stringify(payload),
    "--expect",
    expect,
    "--timeout",
    String(timeoutMs),
  ], String(payload.type ?? "rpc"));
}

function responseOf(envelope: JsonObject): JsonObject {
  return (envelope.response as JsonObject | undefined) ?? envelope;
}

function asArray(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.filter((entry): entry is JsonObject => typeof entry === "object" && entry !== null) : [];
}

function asObject(value: unknown): JsonObject {
  return value && typeof value === "object" ? value as JsonObject : {};
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function notesLines(count: number, prefix = "DevTools notes resize line"): string {
  return Array.from({ length: count }, (_, index) =>
    `${prefix} ${String(index + 1).padStart(2, "0")}`
  ).join("\n");
}

function notesCoverage(coverage: JsonObject) {
  const notes = asArray(coverage.surfaces).find((surface) => surface.id === "notes") ?? {};
  return {
    status: notes.status ?? null,
    sourceFiles: notes.sourceFiles ?? [],
    features: notes.features ?? [],
    shortcuts: notes.shortcuts ?? [],
    supportedNow: notes.supportedNow ?? [],
    missingRuntimePrimitives: notes.missingRuntimePrimitives ?? [],
    regressionRecipeRole: notes.regressionRecipeRole ?? null,
  };
}

function textFingerprint(value: string) {
  let hash = 2166136261;
  for (const char of value) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function redactNestedReceipt(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(redactNestedReceipt);
  }
  if (!value || typeof value !== "object") {
    return value;
  }
  const next: JsonObject = {};
  for (const [key, entry] of Object.entries(value as JsonObject)) {
    if ((key === "value" || key === "text" || key === "label" || key === "commandFingerprint") && typeof entry === "string") {
      next[key] = {
        redacted: true,
        length: entry.length,
        fingerprint: textFingerprint(entry),
        contentReturned: false,
      };
    } else {
      next[key] = redactNestedReceipt(entry);
    }
  }
  return next;
}

function notesState(elements: JsonObject, focus: JsonObject, text: JsonObject, runtimeState: JsonObject, layout: JsonObject) {
  const nodes = asArray(elements.nodes);
  const editor = nodes.find((node) => node.semanticId === "input:notes-editor") ?? null;
  const editorText = String(editor?.label ?? editor?.text ?? editor?.value ?? "");
  const runtimeNotes = (runtimeState.notes as JsonObject | undefined) ?? {};
  const runtimeEditor = (runtimeNotes.editor as JsonObject | undefined) ?? {};
  const layoutRegions = asArray(layout.regions);
  const editorRegion = layoutRegions.find((region) => {
    const name = String(region.name ?? "");
    return name === "NotesEditor" || name === "NotesPreview";
  }) ?? null;
  return {
    panelPresent: nodes.some((node) => node.semanticId === "panel:notes-window"),
    editorPresent: Boolean(editor),
    editorFocused: focus.focusedSemanticId === "input:notes-editor" || editor?.focused === true,
    focusedSemanticId: focus.focusedSemanticId ?? elements.focusedSemanticId ?? null,
    editorStyle: editor?.style ?? (editor?.raw as JsonObject | undefined)?.style ?? null,
    activeNoteId: runtimeNotes.activeNoteId ?? null,
    dirtyState: runtimeNotes.dirtyState ?? null,
    editorTextLength: runtimeEditor.textLength ?? editorText.length,
    editorFingerprint: runtimeEditor.textFingerprint ?? (editorText ? textFingerprint(editorText) : null),
    selectionRange: runtimeEditor.selectionRange ?? null,
    cursorLine: runtimeEditor.cursorLine ?? null,
    draftSnapshot: runtimeNotes.draftSnapshot ?? null,
    editorAnchor: runtimeNotes.editorAnchor ?? null,
    previewAnchor: runtimeNotes.previewAnchor ?? null,
    view: runtimeNotes.view ?? null,
    commandBars: runtimeNotes.commandBars ?? null,
    lastAiHandoff: runtimeNotes.lastAiHandoff ?? null,
    shortcutRegistry: runtimeNotes.shortcutRegistry ?? null,
    focusTransitions: runtimeNotes.focusTransitions ?? null,
    ghostAutocomplete: runtimeNotes.ghostAutocomplete ?? null,
    autosize: runtimeNotes.autosize ?? null,
    generations: runtimeNotes.generations ?? null,
    lastAutosizeTransition: runtimeNotes.lastAutosizeTransition ?? null,
    layout: {
      classification: layout.classification ?? null,
      componentCount: layout.componentCount ?? null,
      editorRegion: editorRegion ? { name: editorRegion.name ?? null, bounds: editorRegion.bounds ?? null } : null,
      pressure: layout.pressure ?? null,
      viewport: layout.viewport ?? null,
    },
    storage: runtimeNotes.storage ?? null,
    counts: runtimeNotes.counts ?? null,
    redacted: runtimeNotes.redacted ?? null,
    footerTexts: text.footerTexts ?? [],
  };
}

function classify(target: JsonObject, elements: JsonObject, requiredMissing: string[]) {
  if (target.classification !== "ok") {
    return target.classification ?? "blocked-by-target-ambiguity";
  }
  if (elements.classification !== "ok") {
    return elements.classification ?? "blocked-by-missing-primitive";
  }
  if (asArray(elements.nodes).length === 0) {
    return "blocked-by-missing-primitive";
  }
  if (requiredMissing.length > 0) {
    return "blocked-by-missing-primitive";
  }
  return "ok";
}

function unsafeResizeReceipt(args: Args, classification: string, reason: string, extra: JsonObject = {}) {
  return {
    schemaVersion: 2,
    tool: "script-kit-devtools.notes",
    command: "notes.resize-compare",
    classification,
    session: args.session,
    safety: {
      mutatesNotesEditor: true,
      requiresSandbox: true,
      sandboxRequested: args.sandbox,
      confirmRealNotesMutation: args.confirmRealNotesMutation,
      startedByTool: args.start,
    },
    reason,
    ...extra,
  };
}

function emitUnsafeResizeReceipt(
  args: Args,
  classification: string,
  reason: string,
  extra: JsonObject = {},
) {
  emitValidatedReceipt(
    "devtools.notes.resizeCompare",
    unsafeResizeReceipt(args, classification, reason, extra),
  );
}

async function sessionStatus(session: string) {
  return run(["bash", "scripts/agentic/session.sh", "status", session], "session-status");
}

async function stopSession(session: string) {
  return run(["bash", "scripts/agentic/session.sh", "stop", session], "session-stop");
}

async function getNotesState(args: Args, notesTargetId: string, label: string) {
  return rpc(args.session, {
    type: "getState",
    requestId: requestId(label),
    target: { type: "id", id: notesTargetId },
  }, "stateResult", args.timeoutMs);
}

async function inspectNotesTarget(args: Args) {
  return run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--target-kind",
    "notes",
    "--strict",
  ], "targets.inspect");
}

function targetHeight(target: JsonObject): number | null {
  const bounds = asObject((target.resolvedTarget as JsonObject | undefined)?.bounds);
  return asNumber(bounds.height);
}

function targetWidth(target: JsonObject): number | null {
  const bounds = asObject((target.resolvedTarget as JsonObject | undefined)?.bounds);
  return asNumber(bounds.width);
}

async function setNotesInput(args: Args, text: string, label: string) {
  const receipt = await rpc(args.session, {
    type: "batch",
    requestId: requestId(label),
    target: { type: "kind", kind: "notes", index: 0 },
    commands: [{ type: "setInput", text }],
    options: { stopOnError: true, rollbackOnError: false, timeout: args.timeoutMs },
    trace: "on",
  }, "batchResult", args.timeoutMs);
  // Autosize settle on the real observable: wait until the resolved notes
  // window height is stable across two consecutive reads instead of a
  // fixed sleep, so the follow-up grow/shrink measurements never read a
  // mid-resize frame.
  await waitForStableNotesHeight(args);
  return receipt;
}

async function waitForStableNotesHeight(args: Args, maxReads = 6): Promise<void> {
  let previous: number | null = null;
  for (let read = 0; read < maxReads; read += 1) {
    const target = await inspectNotesTarget(args);
    const height = targetHeight(target);
    if (height != null && previous != null && height === previous) {
      return;
    }
    previous = height;
    await Bun.sleep(25);
  }
}

async function runResizeCompare(args: Args) {
  if (!args.sandbox && !args.confirmRealNotesMutation) {
    emitUnsafeResizeReceipt(args, "blocked-by-real-data-risk", "notes.resize-compare mutates the Notes editor and requires --sandbox or --confirm-real-notes-mutation.");
    return;
  }
  if (args.confirmRealNotesMutation) {
    emitUnsafeResizeReceipt(args, "blocked-by-real-data-risk", "Real Notes mutation is intentionally not implemented for this DevTools primitive yet. Use --sandbox.");
    return;
  }
  if (!args.start) {
    emitUnsafeResizeReceipt(args, "blocked-by-unsafe-operation", "notes.resize-compare requires --start so the app can be launched with SCRIPT_KIT_TEST_NOTES_DB_PATH.");
    return;
  }

  const beforeStatus = await sessionStatus(args.session);
  if (beforeStatus.alive === true) {
    emitUnsafeResizeReceipt(args, "blocked-by-unsafe-operation", "Refusing to reuse a running session for sandboxed Notes mutation. Use a fresh session name or stop it first.", { beforeStatus });
    return;
  }

  const sandboxPath = args.sandboxPath || `/tmp/sk-devtools-notes-${args.session}-${Date.now()}/notes.sqlite`;
  const sandboxEnv = { SCRIPT_KIT_TEST_NOTES_DB_PATH: sandboxPath };

  const startedByTool = true;
  let cleanupReceipt: JsonObject | null = null;
  const receipts: JsonObject = { beforeStatus };
  let result: JsonObject | null = null;

  try {
    const openReceipt = await maybeOpenNotes({ ...args, open: true }, sandboxEnv);
    receipts.open = openReceipt;
    const target = await waitForNotesTarget(args);
    receipts.target = target;
    const notesTargetId = String((target.resolvedTarget as JsonObject | undefined)?.automationId ?? "notes");
    const beforeStateEnvelope = await getNotesState(args, notesTargetId, "resize-before-state");
    receipts.beforeState = beforeStateEnvelope;
    const beforeRuntime = notesRuntimeState(beforeStateEnvelope);
    const storage = asObject(beforeRuntime.storage);
    if (storage.testSandbox !== true) {
      result = unsafeResizeReceipt(args, "blocked-by-real-data-risk", "Resolved Notes runtime is not using the sandbox notes store; refusing setInput mutation.", {
        sandboxPath,
        storage,
        receipts: redactNestedReceipt(receipts),
      });
      return;
    }

    const beforeTarget = await inspectNotesTarget(args);
    receipts.beforeTarget = beforeTarget;
    const tallText = notesLines(args.tallLineCount);
    const growBatch = await setNotesInput(args, tallText, "resize-grow");
    const afterGrowTarget = await inspectNotesTarget(args);
    const afterGrowState = await getNotesState(args, notesTargetId, "resize-after-grow-state");
    receipts.growBatch = growBatch;
    receipts.afterGrowTarget = afterGrowTarget;
    receipts.afterGrowState = afterGrowState;

    const shortText = notesLines(args.shortLineCount, "DevTools notes restored line");
    const shrinkBatch = await setNotesInput(args, shortText, "resize-shrink");
    const afterShrinkTarget = await inspectNotesTarget(args);
    const afterShrinkState = await getNotesState(args, notesTargetId, "resize-after-shrink-state");
    receipts.shrinkBatch = shrinkBatch;
    receipts.afterShrinkTarget = afterShrinkTarget;
    receipts.afterShrinkState = afterShrinkState;

    const beforeHeight = targetHeight(beforeTarget);
    const afterGrowHeight = targetHeight(afterGrowTarget);
    const afterShrinkHeight = targetHeight(afterShrinkTarget);
    const beforeWidth = targetWidth(beforeTarget);
    const afterGrowWidth = targetWidth(afterGrowTarget);
    const afterShrinkWidth = targetWidth(afterShrinkTarget);
    const growDeltaPx = beforeHeight != null && afterGrowHeight != null ? afterGrowHeight - beforeHeight : null;
    const shrinkDeltaPx = afterGrowHeight != null && afterShrinkHeight != null ? afterGrowHeight - afterShrinkHeight : null;
    const heightGrewForTallContent = growDeltaPx != null && growDeltaPx > 0;
    const heightShrankForShortContent = shrinkDeltaPx != null && shrinkDeltaPx > 0;
    const widthStable = beforeWidth != null
      && afterGrowWidth != null
      && afterShrinkWidth != null
      && Math.abs(afterGrowWidth - beforeWidth) <= 1
      && Math.abs(afterShrinkWidth - beforeWidth) <= 1;
    const growBatchSucceeded = responseOf(growBatch).success === true;
    const shrinkBatchSucceeded = responseOf(shrinkBatch).success === true;
    const beforeView = asObject(beforeRuntime.view);
    const afterGrowView = asObject(notesRuntimeState(afterGrowState).view);
    const afterShrinkView = asObject(notesRuntimeState(afterShrinkState).view);
    const beforeAutosize = asObject(beforeRuntime.autosize);
    const afterGrowAutosize = asObject(notesRuntimeState(afterGrowState).autosize);
    const afterShrinkAutosize = asObject(notesRuntimeState(afterShrinkState).autosize);
    const beforeAutosizeGeneration = asNumber(beforeAutosize.generation);
    const afterGrowAutosizeGeneration = asNumber(afterGrowAutosize.generation);
    const afterShrinkAutosizeGeneration = asNumber(afterShrinkAutosize.generation);
    const generationOrderValid =
      beforeAutosizeGeneration != null &&
      afterGrowAutosizeGeneration != null &&
      afterShrinkAutosizeGeneration != null &&
      afterGrowAutosizeGeneration > beforeAutosizeGeneration &&
      afterShrinkAutosizeGeneration > afterGrowAutosizeGeneration;
    const autoSizingStayedEnabled =
      (beforeAutosize.enabled === true || beforeView.autoSizingEnabled === true) &&
      (afterGrowAutosize.enabled === true || afterGrowView.autoSizingEnabled === true) &&
      (afterShrinkAutosize.enabled === true || afterShrinkView.autoSizingEnabled === true);
    const fixed =
      growBatchSucceeded &&
      shrinkBatchSucceeded &&
      heightGrewForTallContent &&
      heightShrankForShortContent &&
      widthStable &&
      generationOrderValid &&
      autoSizingStayedEnabled;

    result = {
      schemaVersion: 2,
      tool: "script-kit-devtools.notes",
      command: "notes.resize-compare",
      classification: fixed ? "ok" : "reproduced",
      session: args.session,
      target: afterShrinkTarget.resolvedTarget ?? null,
      safety: {
        mutatesNotesEditor: true,
        sandboxRequired: true,
        sandboxConfirmed: true,
        mutationMode: "sandbox-db",
        envName: "SCRIPT_KIT_TEST_NOTES_DB_PATH",
        sandboxNotesDbPath: sandboxPath,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
        openedSystemSettings: false,
        mutatedTcc: false,
      },
      resizeCompare: {
        commandId: "devtools.notes.resizeCompare",
        shortLineCount: args.shortLineCount,
        tallLineCount: args.tallLineCount,
        shortTextFingerprint: textFingerprint(shortText),
        tallTextFingerprint: textFingerprint(tallText),
        beforeHeight,
        afterGrowHeight,
        afterShrinkHeight,
        growDeltaPx,
        shrinkDeltaPx,
        beforeWidth,
        afterGrowWidth,
        afterShrinkWidth,
        heightGrewForTallContent,
        heightShrankForShortContent,
        widthStable,
        growBatchSucceeded,
        shrinkBatchSucceeded,
        autoSizingStayedEnabled,
        generationOrderValid,
        autosize: {
          before: beforeAutosize,
          afterGrow: afterGrowAutosize,
          afterShrink: afterShrinkAutosize,
        },
        beforeView,
        afterGrowView,
        afterShrinkView,
      },
      assertions: [
        { name: "sandbox db active", pass: true },
        { name: "grow batch succeeded", pass: growBatchSucceeded },
        { name: "shrink batch succeeded", pass: shrinkBatchSucceeded },
        { name: "height grew after tall content", pass: heightGrewForTallContent, expected: "> 0", actual: growDeltaPx },
        { name: "height shrank after short content", pass: heightShrankForShortContent, expected: "> 0", actual: shrinkDeltaPx },
        { name: "width stayed stable", pass: widthStable, expected: "delta <= 1px", actual: { beforeWidth, afterGrowWidth, afterShrinkWidth } },
        { name: "autosize generation advanced", pass: generationOrderValid, expected: "before < grow < shrink", actual: { beforeAutosizeGeneration, afterGrowAutosizeGeneration, afterShrinkAutosizeGeneration } },
        { name: "auto sizing stayed enabled", pass: autoSizingStayedEnabled },
        { name: "raw note content redacted", pass: true },
      ],
      missingPrimitives: fixed ? [] : ["auto-resize before/after compare"],
      roadmapMissingPrimitives: [
        "preview scroll handle populated content bounds under mounted markdown preview",
        "Agent Chat embedded origin receipts",
        "portal session provenance",
        "remaining Notes shortcut activation parity receipts beyond Cmd+Shift+P",
      ],
      receipts: redactNestedReceipt(receipts),
      cleanup: null,
    };
  } finally {
    if (args.cleanup && startedByTool) {
      cleanupReceipt = await stopSession(args.session);
    }
    if (result) {
      result.cleanup = cleanupReceipt;
      emitValidatedReceipt("devtools.notes.resizeCompare", result);
    }
  }
}

async function runInspect(args: Args) {
  const openReceipt = await maybeOpenNotes(args);
  const target = await waitForNotesTarget(args);
  const notesTargetId = String((target.resolvedTarget as JsonObject | undefined)?.automationId ?? "notes");
  const shortcutBeforeEnvelope = args.openActions ? await rpc(args.session, {
    type: "getState",
    requestId: requestId("shortcut-before"),
    target: { type: "id", id: notesTargetId },
  }, "stateResult", args.timeoutMs) : null;
  const openActionsReceipt = await maybeOpenActions(args);
  const shortcutAfterEnvelope = args.openActions ? await rpc(args.session, {
    type: "getState",
    requestId: requestId("shortcut-after"),
    target: { type: "id", id: notesTargetId },
  }, "stateResult", args.timeoutMs) : null;
  const shortcutActivation = buildShortcutActivationReceipt(
    openActionsReceipt,
    shortcutBeforeEnvelope,
    shortcutAfterEnvelope,
  );
  const targetArgs = ["--session", args.session, "--target-id", notesTargetId, "--strict"];
  const elements = await run(["bun", "scripts/devtools/elements.ts", "snapshot", ...targetArgs, "--limit", String(args.limit)], "elements.snapshot");
  const focus = await run(["bun", "scripts/devtools/focus.ts", "inspect", ...targetArgs], "focus.inspect");
  const text = await run(["bun", "scripts/devtools/text.ts", "measure", ...targetArgs, "--limit", String(args.limit)], "text.measure");
  const layout = await run(["bun", "scripts/devtools/layout.ts", "measure", ...targetArgs, "--include", "nodes,regions,scroll,resize,overlaps"], "layout.measure");
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: requestId("state"),
    target: { type: "id", id: notesTargetId },
  }, "stateResult", args.timeoutMs);
  const runtimeState = responseOf(stateEnvelope);
  const coverageRaw = await run(["bun", "scripts/devtools/coverage.ts", "--surface", "notes"], "coverage.notes");
  const coverage = notesCoverage(coverageRaw);
  const state = notesState(elements, focus, text, runtimeState, layout);
  const runtimeNotes = (runtimeState.notes as JsonObject | undefined) ?? {};
  const editorAnchor = asObject(state.editorAnchor);
  const editorScroll = asObject(editorAnchor.scroll);
  const previewAnchor = asObject(state.previewAnchor);
  const requiredMissing = [
    ...new Set([
      ...(Array.isArray(elements.missingPrimitives) ? elements.missingPrimitives.map(String) : []),
      ...(Array.isArray(text.missingPrimitives) ? text.missingPrimitives.map(String) : []),
      state.activeNoteId == null ? "active note id" : "",
      state.dirtyState == null ? "dirty state" : "",
      state.selectionRange == null ? "cursor and selection ranges" : "",
      state.draftSnapshot == null ? "draft snapshot fingerprint" : "",
      editorAnchor.scrollMetricsAvailable !== true ? "editor scroll metrics" : "",
      editorAnchor.scrollTopAvailable !== true ? "editor scrollTopAvailable" : "",
      editorAnchor.scrollHeightAvailable !== true ? "editor scrollHeightAvailable" : "",
      editorAnchor.clientHeightAvailable !== true ? "editor clientHeightAvailable" : "",
      editorScroll.scrollTop == null ? "editor scrollTop" : "",
      editorScroll.scrollHeight == null ? "editor scrollHeight" : "",
      editorScroll.clientHeight == null ? "editor clientHeight" : "",
      previewAnchor.previewEnabled === true && previewAnchor.scrollMetricsAvailable !== true
        ? "preview scroll metrics"
        : "",
      state.layout.editorRegion == null ? "notes editor layout region" : "",
      state.storage == null ? "note store generation and sandbox identity" : "",
      state.focusTransitions == null ? "notes focus owner transition timeline" : "",
    ].filter(Boolean)),
  ];
  const roadmapMissing = [
    ...new Set([
      ...missingCoveragePrimitives(coverage, runtimeNotes),
      previewAnchor.previewEnabled === true && previewAnchor.scrollMetricsAvailable !== true
        ? "preview scroll metrics"
        : "",
      hasNotesAiHandoffReceipt(runtimeNotes) ? "" : "notes AI handoff receipt",
      "portal session provenance",
      state.commandBars == null ? "notes command bar runtime state" : "",
      state.shortcutRegistry == null ? "notes shortcut registry" : "",
    ].filter(Boolean)),
  ];

  emitValidatedReceipt("devtools.notes.inspect", {
    schemaVersion: 2,
    tool: "script-kit-devtools.notes",
    command: "notes.inspect",
    classification: classify(target, elements, requiredMissing),
    session: args.session,
    openReceipt: diagnostic(openReceipt),
    openActionsReceipt: diagnostic(openActionsReceipt),
    shortcutActivation: diagnostic(shortcutActivation),
    availableActions: {
      togglePreview: {
        channel: "protocol.batch.togglePreview",
        command: "togglePreview",
        target: { type: "kind", kind: "notes" },
      },
      askAiHandoff: {
        channel: "protocol.simulateKey",
        command: "cmd+enter",
        target: { type: "kind", kind: "notes" },
        note: "Opens the MAIN window's Agent Chat with the selected note staged as @note context; Notes stays open.",
      },
    },
    target: target.resolvedTarget ?? null,
    requestedTarget: target.requestedTarget ?? { selector: { type: "kind", kind: "notes" } },
    notesState: state,
    runtimeState: diagnostic(runtimeState),
    coverage,
    receipts: diagnostic({
      target,
      elements,
      focus,
      text,
      layout,
      state: stateEnvelope,
    }),
    missingPrimitives: requiredMissing,
    roadmapMissingPrimitives: roadmapMissing,
    warnings: [
      ...(Array.isArray(elements.warnings) ? elements.warnings : []),
      ...(Array.isArray(focus.warnings) ? focus.warnings : []),
      ...(Array.isArray(text.warnings) ? text.warnings : []),
      requiredMissing.length > 0 ? `Notes inspection remains fail-closed until required primitives are available: ${requiredMissing.join(", ")}.` : "",
      roadmapMissing.length > 0 ? `Notes inspection roadmap primitives still missing: ${roadmapMissing.join(", ")}.` : "",
    ].filter(Boolean),
    errors: diagnostic(
      [target, elements, focus, text].filter((receipt) => receipt.status === "error"),
    ),
  });
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  if (args.command === "resize-compare") {
    await runResizeCompare(args);
    return;
  }
  await runInspect(args);
}

await main();
