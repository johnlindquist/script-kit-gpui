#!/usr/bin/env bun
import { runtimeArtifactFromEnvironment } from "../../devtools/lib/runtime-task-proof.ts";

import { createHash } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observedWorkflowStage,
  observeWorkflowTaskTarget,
  prepareBlockedWorkflowTaskProof,
  prepareWorkflowTaskProof,
  writeWorkflowTaskProof,
} from "../../devtools/lib/workflow-task-proof.ts";
import type { RuntimeTargetObservation } from "../../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("notes-actions.private-pasteboard-archive");

const BINARY = runtimeArtifactFromEnvironment().executablePath
const OUT_DIR = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR ?? ".test-output/cons-flow-c07",
);
const RECEIPT_PATH = join(OUT_DIR, "notes-actions-receipt.json");
const NOTES_TARGET: Json = { type: "kind", kind: "notes", index: 0 };
const ACTIONS_TARGET: Json = {
  type: "kind",
  kind: "actionsDialog",
  index: 0,
};
const SYNTHETIC_NOTE =
  "C07 synthetic note\n- first synthetic item\n- second synthetic item\n[scripts](kit://scripts)";

type Obj = Record<string, any>;
type Descriptor = {
  id: string;
  label: string;
  shortcut: string | null;
  canonicalShortcut: string | null;
  enabled: boolean;
  disabledReason: string | null;
  destructive: boolean;
  confirmationRequired: boolean;
  semanticActionId: string;
};
type ProjectionReceipt = {
  mode: string;
  descriptorCount: number;
  descriptorIds: string[];
  shortcutCount: number;
  actionRowCount: number;
  actionRowsMatch: boolean;
  shortcutParity: boolean;
};
type ActivationReceipt = {
  actionId: string;
  channel: "gpui.notes" | "automation.notes";
  shortcut: string;
  key: string;
  modifiers: string[];
  beforeGeneration: number;
  afterGeneration: number;
  semanticActionId: string;
  exactlyOnce: boolean;
};

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Obj)
    : {};
}

function asArray(value: unknown): any[] {
  return Array.isArray(value) ? value : [];
}

function assert(
  condition: unknown,
  message: string,
  detail?: unknown,
): asserts condition {
  if (!condition) {
    throw new Error(
      `${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`,
    );
  }
}

function hashText(value: string): string {
  return createHash("sha256").update(value).digest("hex").slice(0, 24);
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const normalized = resolve(executable);
  return new TextDecoder()
    .decode(result.stdout)
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

async function runProcess(args: string[]): Promise<string> {
  const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const [stdout, code] = await Promise.all([
    new Response(child.stdout).text(),
    child.exited,
  ]);
  assert(code === 0, `private helper failed with exit ${code}`);
  return stdout;
}

const PASTEBOARD_SWIFT = String.raw`
import AppKit
import Foundation
struct ArchiveItem: Codable { let values: [String: String] }
struct Archive: Codable { let items: [ArchiveItem] }
func archive() -> Archive {
  let items = (NSPasteboard.general.pasteboardItems ?? []).map { item -> ArchiveItem in
    var values: [String: String] = [:]
    for type in item.types.sorted(by: { $0.rawValue < $1.rawValue }) {
      if let data = item.data(forType: type) { values[type.rawValue] = data.base64EncodedString() }
    }
    return ArchiveItem(values: values)
  }
  return Archive(items: items)
}
let args = CommandLine.arguments
let board = NSPasteboard.general
switch args[1] {
case "capture":
  let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
  try encoder.encode(archive()).write(to: URL(fileURLWithPath: args[2]), options: .atomic)
case "restore":
  let decoded = try JSONDecoder().decode(Archive.self, from: Data(contentsOf: URL(fileURLWithPath: args[2])))
  board.clearContents()
  let items = decoded.items.map { saved -> NSPasteboardItem in
    let item = NSPasteboardItem()
    for key in saved.values.keys.sorted() {
      if let encoded = saved.values[key], let value = Data(base64Encoded: encoded) {
        item.setData(value, forType: NSPasteboard.PasteboardType(key))
      }
    }
    return item
  }
  if !items.isEmpty { _ = board.writeObjects(items) }
default: exit(64)
}
`;

class PasteboardGuard {
  private constructor(
    private readonly root: string,
    private readonly executable: string,
    private readonly before: string,
  ) {}

  static async create(): Promise<PasteboardGuard> {
    const root = mkdtempSync(join(tmpdir(), "cons-flow-c07-pasteboard-"));
    const source = join(root, "pasteboard.swift");
    const executable = join(root, "pasteboard");
    const before = join(root, "before.json");
    writeFileSync(source, PASTEBOARD_SWIFT, { mode: 0o600 });
    await runProcess(["/usr/bin/xcrun", "swiftc", source, "-o", executable]);
    await runProcess([executable, "capture", before]);
    chmodSync(before, 0o600);
    return new PasteboardGuard(root, executable, before);
  }

  async restore(): Promise<void> {
    await runProcess([this.executable, "restore", this.before]);
    const after = join(this.root, "after.json");
    await runProcess([this.executable, "capture", after]);
    assert(
      readFileSync(after).equals(readFileSync(this.before)),
      "clipboard restoration did not preserve every pasteboard type",
    );
    rmSync(this.root, { recursive: true, force: true });
  }
}

async function poll<T>(
  label: string,
  read: () => Promise<T>,
  predicate: (value: T) => boolean,
  timeoutMs = 10_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let value = await read();
  while (!predicate(value) && Date.now() < deadline) {
    await Bun.sleep(40);
    value = await read();
  }
  assert(predicate(value), `timed out waiting for ${label}`);
  return value;
}

async function notesState(driver: Driver): Promise<Obj> {
  const result = asObj(
    await driver.request(
      { type: "getState", target: NOTES_TARGET },
      { expect: "stateResult", timeoutMs: 10_000 },
    ),
  );
  return asObj(result.notes);
}

async function descriptors(driver: Driver): Promise<Descriptor[]> {
  return asArray((await notesState(driver)).actionDescriptors).map((value) => {
    const descriptor = asObj(value);
    return {
      id: String(descriptor.id),
      label: String(descriptor.label),
      shortcut:
        typeof descriptor.shortcut === "string" ? descriptor.shortcut : null,
      canonicalShortcut:
        typeof descriptor.canonicalShortcut === "string"
          ? descriptor.canonicalShortcut
          : null,
      enabled: descriptor.enabled === true,
      disabledReason:
        typeof descriptor.disabledReason === "string"
          ? descriptor.disabledReason
          : null,
      destructive: descriptor.destructive === true,
      confirmationRequired: descriptor.confirmationRequired === true,
      semanticActionId: String(descriptor.semanticActionId),
    };
  });
}

async function notesElements(driver: Driver): Promise<Obj[]> {
  const result = asObj(
    await driver.getElements(
      { target: NOTES_TARGET, limit: 1_000 },
      { timeoutMs: 10_000 },
    ),
  );
  return asArray(result.elements).map(asObj);
}

async function actionElements(driver: Driver): Promise<Obj[]> {
  const result = asObj(
    await driver.getElements(
      { target: ACTIONS_TARGET, limit: 1_000 },
      { timeoutMs: 10_000 },
    ),
  );
  return asArray(result.elements).map(asObj);
}

async function gpuiKey(
  driver: Driver,
  key: string,
  modifiers: string[] = [],
  target: Json = NOTES_TARGET,
): Promise<void> {
  const result = asObj(
    await driver.simulateGpuiKeyDown(key, {
      target,
      modifiers,
      timeoutMs: 10_000,
    }),
  );
  assert(result.success !== false, `key dispatch failed: ${key}`, result);
  if (result.dispatchCompleted !== true) await Bun.sleep(40);
}

async function openActions(driver: Driver): Promise<Obj> {
  const result = asObj(
    await driver.request(
      {
        type: "batch",
        target: NOTES_TARGET,
        commands: [{ type: "openActions" }],
        options: { stopOnError: true, rollbackOnError: false, timeout: 8_000 },
      },
      { expect: "batchResult", timeoutMs: 10_000 },
    ),
  );
  assert(result.success === true, "failed to open Notes Actions", result);
  return poll(
    "Notes Actions open",
    () => notesState(driver),
    (state) => asObj(asObj(state.commandBars).actions).open === true,
  );
}

async function closeActions(driver: Driver): Promise<void> {
  await gpuiKey(driver, "escape");
  await poll(
    "Notes Actions close",
    () => notesState(driver),
    (state) => asObj(asObj(state.commandBars).actions).open !== true,
  );
  // The host closes synchronously, while the calibrated native popup exit
  // retires its automation target after the protected removal delay. Do not
  // dispatch the next chord into that owned exit interval.
  await poll(
    "Notes Actions native target retirement",
    async () => asArray(asObj(await driver.listAutomationWindows()).windows),
    (windows) => !windows.map(asObj).some((window) => window.kind === "actionsDialog"),
  );
  // Automation target removal precedes the protected 135ms native popup
  // removal delay; wait past that calibrated exit without changing it.
  await Bun.sleep(180);
}

async function confirmWindows(driver: Driver): Promise<Obj[]> {
  const result = asObj(await driver.listAutomationWindows());
  return asArray(result.windows)
    .map(asObj)
    .filter((window) => window.semanticSurface === "confirmDialog");
}

async function waitConfirm(driver: Driver, title: string): Promise<Obj[]> {
  return poll(
    `confirm dialog ${title}`,
    () => confirmWindows(driver),
    (windows) => windows.some((window) => window.title === title),
  );
}

async function cancelConfirm(driver: Driver): Promise<void> {
  driver.send({ type: "simulateKey", target: NOTES_TARGET, key: "escape", modifiers: [] });
  await poll(
    "confirm dialog close",
    () => confirmWindows(driver),
    (windows) => windows.length === 0,
  );
}

async function acceptConfirm(driver: Driver): Promise<void> {
  driver.send({ type: "simulateKey", target: NOTES_TARGET, key: "enter", modifiers: [] });
  await poll(
    "confirmed dialog close",
    () => confirmWindows(driver),
    (windows) => windows.length === 0,
  );
}

function execution(state: Obj): Obj {
  return asObj(state.lastActionExecution);
}

async function waitExecution(
  driver: Driver,
  actionId: string,
  beforeGeneration: number,
): Promise<Obj> {
  return poll(
    `execution ${actionId}`,
    () => notesState(driver),
    (state) => {
      const last = execution(state);
      return (
        Number(last.generation) === beforeGeneration + 1 &&
        last.actionId === actionId &&
        last.semanticActionId === `notes.action.${actionId}`
      );
    },
  );
}

function parseShortcut(shortcut: string): { key: string; modifiers: string[] } {
  const parts = shortcut.split("+");
  const key = parts.pop() ?? "";
  return { key, modifiers: parts };
}

const projections: ProjectionReceipt[] = [];
const activations: ActivationReceipt[] = [];
const activatedCanonical = new Set<string>();

async function captureProjection(
  driver: Driver,
  mode: string,
): Promise<Descriptor[]> {
  const current = await descriptors(driver);
  assert(current.length > 0, `${mode} exposed no Notes action descriptors`);

  const ids = new Set<string>();
  const semantics = new Set<string>();
  const shortcuts = new Set<string>();
  for (const descriptor of current) {
    assert(ids.add(descriptor.id), `${mode} duplicated id ${descriptor.id}`);
    assert(
      semantics.add(descriptor.semanticActionId),
      `${mode} duplicated semantic action ${descriptor.semanticActionId}`,
    );
    assert(
      descriptor.semanticActionId === `notes.action.${descriptor.id}`,
      `${mode} semantic action drifted for ${descriptor.id}`,
    );
    assert(
      descriptor.destructive === descriptor.confirmationRequired,
      `${mode} confirmation drifted for ${descriptor.id}`,
    );
    assert(
      descriptor.enabled === (descriptor.disabledReason === null),
      `${mode} disabled reason drifted for ${descriptor.id}`,
    );
    if (descriptor.canonicalShortcut) {
      assert(
        shortcuts.add(descriptor.canonicalShortcut),
        `${mode} duplicated shortcut ${descriptor.canonicalShortcut}`,
      );
    }
  }

  const state = await openActions(driver);
  const actionState = asObj(asObj(state.commandBars).actions);
  const dialog = asObj(actionState.dialog);
  const actionSummary = asObj(dialog.actions);
  const elements = await actionElements(driver);
  const rows = elements.filter((element) => element.type === "choice");
  assert(
    Number(actionState.configuredActionCount) === current.length,
    `${mode} configured action count drifted`,
    { configured: actionState.configuredActionCount, descriptors: current.length },
  );
  assert(
    Number(actionSummary.totalCount) === current.length,
    `${mode} dialog action count drifted`,
  );
  assert(rows.length === current.length, `${mode} semantic action row count drifted`, {
    rows: rows.length,
    descriptors: current.length,
  });
  for (const descriptor of current) {
    const row = rows.find((element) => element.value === descriptor.id);
    assert(row, `${mode} omitted Actions row ${descriptor.id}`);
    assert(row.text === descriptor.label, `${mode} label drifted for ${descriptor.id}`, {
      descriptor: descriptor.label,
      actionRow: row.text,
    });
  }
  const parity = asObj(actionSummary.shortcutParity);
  assert(
    Number(parity.displayedShortcutCount) === shortcuts.size &&
      Number(parity.routableShortcutCount) === shortcuts.size &&
      Number(parity.duplicateShortcutCount) === 0 &&
      asArray(parity.unroutableDisplayedShortcuts).length === 0,
    `${mode} Actions shortcut parity failed`,
    parity,
  );
  projections.push({
    mode,
    descriptorCount: current.length,
    descriptorIds: current.map((descriptor) => descriptor.id),
    shortcutCount: shortcuts.size,
    actionRowCount: rows.length,
    actionRowsMatch: true,
    shortcutParity: true,
  });
  await closeActions(driver);
  return current;
}

async function activateShortcut(
  driver: Driver,
  descriptor: Descriptor,
  keyOverride?: string,
): Promise<void> {
  assert(descriptor.shortcut, `${descriptor.id} has no advertised shortcut`);
  const parsed = parseShortcut(descriptor.shortcut);
  const key = keyOverride ?? parsed.key;
  const beforeState = await notesState(driver);
  const beforeGeneration = Number(execution(beforeState).generation ?? 0);
  await gpuiKey(driver, key, parsed.modifiers);
  const afterState = await waitExecution(driver, descriptor.id, beforeGeneration);
  const after = execution(afterState);
  activations.push({
    actionId: descriptor.id,
    channel: "gpui.notes",
    shortcut: descriptor.shortcut,
    key,
    modifiers: parsed.modifiers,
    beforeGeneration,
    afterGeneration: Number(after.generation),
    semanticActionId: String(after.semanticActionId),
    exactlyOnce: Number(after.generation) === beforeGeneration + 1,
  });
  if (descriptor.canonicalShortcut) activatedCanonical.add(descriptor.canonicalShortcut);
}

async function activateShortcutViaNotesAutomation(
  driver: Driver,
  descriptor: Descriptor,
): Promise<void> {
  assert(descriptor.shortcut, `${descriptor.id} has no advertised shortcut`);
  const parsed = parseShortcut(descriptor.shortcut);
  const beforeGeneration = Number(execution(await notesState(driver)).generation ?? 0);
  driver.send({
    type: "simulateKey",
    target: NOTES_TARGET,
    key: parsed.key,
    modifiers: parsed.modifiers,
  });
  const afterState = await waitExecution(driver, descriptor.id, beforeGeneration);
  const after = execution(afterState);
  activations.push({
    actionId: descriptor.id,
    channel: "automation.notes",
    shortcut: descriptor.shortcut,
    key: parsed.key,
    modifiers: parsed.modifiers,
    beforeGeneration,
    afterGeneration: Number(after.generation),
    semanticActionId: String(after.semanticActionId),
    exactlyOnce: Number(after.generation) === beforeGeneration + 1,
  });
  if (descriptor.canonicalShortcut) activatedCanonical.add(descriptor.canonicalShortcut);
}

async function activateActionRow(
  driver: Driver,
  actionId: string,
): Promise<number> {
  const beforeGeneration = Number(execution(await notesState(driver)).generation ?? 0);
  await openActions(driver);
  const row = (await actionElements(driver)).find(
    (element) => element.type === "choice" && element.value === actionId,
  );
  assert(row && typeof row.semanticId === "string", `missing Actions row ${actionId}`);
  const selected = asObj(
    await driver.request(
      {
        type: "batch",
        target: ACTIONS_TARGET,
        commands: [
          { type: "selectBySemanticId", semanticId: row.semanticId },
        ],
        options: { stopOnError: true, rollbackOnError: false, timeout: 8_000 },
      },
      { expect: "batchResult", timeoutMs: 10_000 },
    ),
  );
  assert(selected.success === true, `failed to select Actions row ${actionId}`, selected);
  await gpuiKey(driver, "enter", [], ACTIONS_TARGET);
  await waitExecution(driver, actionId, beforeGeneration);
  await poll(
    `Actions target retirement after ${actionId}`,
    async () => asArray(asObj(await driver.listAutomationWindows()).windows),
    (windows) => !windows.map(asObj).some((window) => window.kind === "actionsDialog"),
  );
  await Bun.sleep(180);
  return beforeGeneration;
}

async function reopenNotes(driver: Driver, label: string): Promise<void> {
  driver.send({ type: "openNotes", requestId: `${label}-close` });
  await poll(
    `${label} Notes close`,
    async () => asArray(asObj(await driver.listAutomationWindows()).windows),
    (windows) => !windows.map(asObj).some((window) => window.kind === "notes"),
    15_000,
  );
  driver.send({ type: "openNotes", requestId: `${label}-open` });
  await poll(
    `${label} Notes reopen`,
    async () => asArray(asObj(await driver.listAutomationWindows()).windows),
    (windows) => windows.map(asObj).some((window) => window.kind === "notes"),
    15_000,
  );
  await poll(
    `${label} Notes editor focus`,
    () => notesState(driver),
    (state) => asObj(state.view).focusSurface === "Editor",
    15_000,
  );
  // Opening state is available before the protected material-onset/reveal
  // lifecycle has completed. Wait beyond that entry boundary before sending a
  // real GPUI key to the fresh window.
  await Bun.sleep(300);
}

async function setNotesInput(driver: Driver, text: string): Promise<void> {
  const result = asObj(
    await driver.request(
      {
        type: "batch",
        target: NOTES_TARGET,
        commands: [{ type: "setInput", text }],
        options: { stopOnError: true, rollbackOnError: false, timeout: 8_000 },
      },
      { expect: "batchResult", timeoutMs: 10_000 },
    ),
  );
  assert(result.success === true, "failed to seed synthetic Notes text", result);
}

function descriptorById(list: Descriptor[], id: string): Descriptor {
  const descriptor = list.find((candidate) => candidate.id === id);
  assert(descriptor, `missing descriptor ${id}`);
  return descriptor;
}

async function ensureAllNotesEditor(driver: Driver): Promise<void> {
  const state = await notesState(driver);
  const view = asObj(state.view);
  if (view.viewMode === "Trash") {
    await gpuiKey(driver, "escape");
    await poll(
      "return to active Notes",
      () => notesState(driver),
      (next) => asObj(next.view).viewMode === "AllNotes",
    );
  }
  const next = await notesState(driver);
  if (asObj(next.view).previewEnabled === true) {
    await gpuiKey(driver, "p", ["shift", "cmd"]);
    await poll(
      "return to editor mode",
      () => notesState(driver),
      (value) => asObj(value.view).previewEnabled === false,
    );
  }
  if (asObj((await notesState(driver)).view).showSearch === true) {
    await gpuiKey(driver, "escape");
  }
}

function assertModeIncludes(
  mode: string,
  list: Descriptor[],
  included: string[],
  excluded: string[],
): void {
  const ids = new Set(list.map((descriptor) => descriptor.id));
  for (const id of included) assert(ids.has(id), `${mode} omitted ${id}`);
  for (const id of excluded) assert(!ids.has(id), `${mode} exposed ${id}`);
}

const receipt: Obj = {
  schemaVersion: 1,
  task: "SAFE-004",
  binarySha256: "",
  status: "FAILED",
  projections,
  activations,
  negativeControls: {},
  destructiveConfirmations: {},
  titlebar: {},
  cleanup: {},
};

let driver: Driver | null = null;
let pasteboard: PasteboardGuard | null = null;
let databaseRoot: string | null = null;
let failureFingerprint: string | null = null;
let clipboardRestored = false;
let targetObservation: RuntimeTargetObservation | null = null;

try {
  const hash = Bun.spawnSync(["shasum", "-a", "256", BINARY], {
    stdout: "pipe",
    stderr: "pipe",
  });
  assert(hash.exitCode === 0, "failed to hash stable C07 artifact");
  receipt.binarySha256 = new TextDecoder().decode(hash.stdout).trim().split(/\s+/, 1)[0];

  pasteboard = await PasteboardGuard.create();
  databaseRoot = mkdtempSync(join(tmpdir(), "cons-flow-c07-notes-"));
  driver = await Driver.launch({ immutableArtifact: runtimeArtifactFromEnvironment().reference, binary: BINARY,
  sessionName: "cons-flow-c07-notes-actions",
  sandboxHome: true,
  sharedModels: false,
  env: {
    SCRIPT_KIT_TEST_STATUS: "1",
    SCRIPT_KIT_TEST_NOTES_DB_PATH: join(databaseRoot, "notes.db"),
    SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
    SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
    SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
    SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
  },
  readyTimeoutMs: 30_000,
  defaultTimeoutMs: 15_000, });
  await driver.waitForSettle();
  driver.send({ type: "openNotes", requestId: "cons-flow-c07-open-notes" });
  await poll(
    "Notes target",
    async () => asArray(asObj(await driver!.listAutomationWindows()).windows),
    (windows) => windows.map(asObj).some((window) => window.kind === "notes"),
    15_000,
  );
  await setNotesInput(driver, SYNTHETIC_NOTE);

  const editorSelected = await captureProjection(driver, "editor-selected");
  assertModeIncludes(
    "editor-selected",
    editorSelected,
    ["delete_note", "format", "send_to_ai"],
    ["restore_note", "permanently_delete_note", "empty_trash"],
  );
  const editorElements = await notesElements(driver);
  receipt.titlebar.askAi = editorElements.some((element) =>
    String(element.semanticId ?? "").includes("notes.action.send_to_ai"),
  );
  assert(receipt.titlebar.askAi, "Ask AI titlebar command omitted descriptor semantic ID");

  await activateShortcut(driver, descriptorById(editorSelected, "toggle_preview"));
  const previewSelected = await captureProjection(driver, "preview-selected");
  assertModeIncludes(
    "preview-selected",
    previewSelected,
    ["delete_note", "send_to_ai"],
    ["find_in_note", "format", "move_list_item_up", "move_list_item_down"],
  );
  if (asObj((await notesState(driver)).view).previewEnabled === true) {
    await activateShortcut(driver, descriptorById(previewSelected, "toggle_preview"));
    await poll(
      "preview close",
      () => notesState(driver),
      (state) => asObj(state.view).previewEnabled === false,
    );
  }

  await gpuiKey(driver, "t", ["shift", "cmd"]);
  await poll(
    "empty Trash mode",
    () => notesState(driver),
    (state) =>
      asObj(state.view).viewMode === "Trash" &&
      Number(asObj(state.counts).deletedNotes) === 0,
  );
  const trashEmpty = await captureProjection(driver, "trash-empty");
  assertModeIncludes(
    "trash-empty",
    trashEmpty,
    ["empty_trash", "back_to_notes"],
    ["restore_note", "permanently_delete_note", "delete_note", "send_to_ai"],
  );
  await activateActionRow(driver, "back_to_notes");
  await poll(
    "leave empty Trash",
    () => notesState(driver),
    (state) => asObj(state.view).viewMode === "AllNotes",
  );
  await reopenNotes(driver, "after-back-to-notes");

  await setNotesInput(driver, SYNTHETIC_NOTE);
  // Read-only kit:// preview setup uses the established Notes deeplink
  // fixture route; descriptor shortcut proof below remains real GPUI dispatch.
  driver.send({
    type: "simulateKey",
    target: NOTES_TARGET,
    key: ".",
    modifiers: ["cmd"],
  });
  await poll(
    "read-only kit resource preview",
    () => notesState(driver),
    (state) => {
      const preview = asObj(state.kitResourcePreview);
      return preview.active === true && preview.readOnly === true && preview.uri === "kit://scripts";
    },
  );
  const readOnly = await captureProjection(driver, "read-only");
  assert(
    JSON.stringify(readOnly.map((descriptor) => descriptor.id)) ===
      JSON.stringify(["browse_notes", "reset_window_position"]),
    "read-only descriptor policy drifted",
    readOnly.map((descriptor) => descriptor.id),
  );
  await gpuiKey(driver, "escape");
  await poll(
    "read-only preview close",
    () => notesState(driver),
    (state) => asObj(state.kitResourcePreview).active !== true,
  );

  const beforeShiftA = Number(execution(await notesState(driver)).generation ?? 0);
  await gpuiKey(driver, "a", ["shift", "cmd"]);
  await Bun.sleep(80);
  const afterShiftA = Number(execution(await notesState(driver)).generation ?? 0);
  assert(afterShiftA === beforeShiftA, "Shift+Command+A activated a Notes action");
  receipt.negativeControls.shiftCommandA = "no-action";

  const beforePlatformDelete = Number(execution(await notesState(driver)).generation ?? 0);
  await gpuiKey(driver, "delete", ["shift", "cmd"]);
  await Bun.sleep(80);
  const afterPlatformDelete = Number(execution(await notesState(driver)).generation ?? 0);
  assert(
    afterPlatformDelete === beforePlatformDelete && (await confirmWindows(driver)).length === 0,
    "Shift+Command+Delete activated the Backspace-only delete policy",
  );
  receipt.negativeControls.shiftCommandDelete = "no-action";
  receipt.negativeControls.formatShortcut =
    descriptorById(editorSelected, "format").shortcut === null ? "none" : "unexpected";
  assert(receipt.negativeControls.formatShortcut === "none", "Format advertised a shortcut");

  const deleteDescriptor = descriptorById(editorSelected, "delete_note");
  const countsBeforeDeleteCancel = asObj((await notesState(driver)).counts);
  await activateShortcut(driver, deleteDescriptor);
  await waitConfirm(driver, "Move note to Trash");
  const countsWithDeleteConfirm = asObj((await notesState(driver)).counts);
  assert(
    countsWithDeleteConfirm.notes === countsBeforeDeleteCancel.notes &&
      countsWithDeleteConfirm.deletedNotes === countsBeforeDeleteCancel.deletedNotes,
    "Delete mutated Notes before confirmation",
  );
  await cancelConfirm(driver);
  const countsAfterDeleteCancel = asObj((await notesState(driver)).counts);
  assert(
    countsAfterDeleteCancel.notes === countsBeforeDeleteCancel.notes &&
      countsAfterDeleteCancel.deletedNotes === countsBeforeDeleteCancel.deletedNotes,
    "Delete cancellation mutated Notes",
  );
  receipt.destructiveConfirmations.delete = {
    openedBeforeMutation: true,
    cancelPreservedCounts: true,
  };

  await activateShortcut(driver, deleteDescriptor);
  await waitConfirm(driver, "Move note to Trash");
  await acceptConfirm(driver);
  await poll(
    "soft delete completion",
    () => notesState(driver),
    (state) =>
      Number(asObj(state.counts).notes) === 0 &&
      Number(asObj(state.counts).deletedNotes) === 1,
  );

  const editorNoCurrent = await captureProjection(driver, "editor-no-current");
  assertModeIncludes(
    "editor-no-current",
    editorNoCurrent,
    ["new_note", "open_trash"],
    ["delete_note", "format", "send_to_ai", "duplicate_note"],
  );

  await activateActionRow(driver, "open_trash");
  await poll(
    "selected Trash mode",
    () => notesState(driver),
    (state) =>
      asObj(state.view).viewMode === "Trash" &&
      Number(asObj(state.counts).deletedNotes) === 1,
  );
  const trashSelected = await captureProjection(driver, "trash-selected");
  assertModeIncludes(
    "trash-selected",
    trashSelected,
    ["restore_note", "permanently_delete_note", "empty_trash"],
    ["delete_note", "send_to_ai", "format"],
  );
  const trashElements = await notesElements(driver);
  receipt.titlebar.restore = trashElements.some((element) =>
    String(element.semanticId ?? "").includes("notes.action.restore_note"),
  );
  receipt.titlebar.permanentlyDelete = trashElements.some((element) =>
    String(element.semanticId ?? "").includes("notes.action.permanently_delete_note"),
  );
  assert(
    receipt.titlebar.restore && receipt.titlebar.permanentlyDelete,
    "Trash titlebar commands omitted descriptor semantic IDs",
  );

  const restore = descriptorById(trashSelected, "restore_note");
  await activateShortcutViaNotesAutomation(driver, restore);
  await poll(
    "restored note by advertised chord",
    () => notesState(driver),
    (state) =>
      asObj(state.view).viewMode === "AllNotes" &&
      Number(asObj(state.counts).notes) === 1 &&
      Number(asObj(state.counts).deletedNotes) === 0,
  );
  await setNotesInput(driver, SYNTHETIC_NOTE);

  await activateShortcut(driver, deleteDescriptor);
  await waitConfirm(driver, "Move note to Trash");
  await acceptConfirm(driver);
  await poll(
    "second soft delete completion",
    () => notesState(driver),
    (state) =>
      Number(asObj(state.counts).notes) === 0 &&
      Number(asObj(state.counts).deletedNotes) === 1,
  );
  await activateActionRow(driver, "open_trash");
  await poll(
    "selected Trash for destructive rows",
    () => notesState(driver),
    (state) => asObj(state.view).viewMode === "Trash",
  );

  const trashCounts = asObj((await notesState(driver)).counts);
  await activateActionRow(driver, "permanently_delete_note");
  await waitConfirm(driver, "Delete note permanently");
  assert(
    asObj((await notesState(driver)).counts).deletedNotes === trashCounts.deletedNotes,
    "Permanent Delete mutated Trash before confirmation",
  );
  await cancelConfirm(driver);
  assert(
    asObj((await notesState(driver)).counts).deletedNotes === trashCounts.deletedNotes,
    "Permanent Delete cancellation mutated Trash",
  );
  receipt.destructiveConfirmations.permanentlyDelete = {
    openedBeforeMutation: true,
    cancelPreservedCounts: true,
  };

  await activateActionRow(driver, "empty_trash");
  await waitConfirm(driver, "Empty Trash");
  assert(
    asObj((await notesState(driver)).counts).deletedNotes === trashCounts.deletedNotes,
    "Empty Trash mutated before confirmation",
  );
  await cancelConfirm(driver);
  assert(
    asObj((await notesState(driver)).counts).deletedNotes === trashCounts.deletedNotes,
    "Empty Trash cancellation mutated Trash",
  );
  receipt.destructiveConfirmations.emptyTrash = {
    openedBeforeMutation: true,
    cancelPreservedCounts: true,
  };

  await activateActionRow(driver, "restore_note");
  await poll(
    "restored note after destructive controls",
    () => notesState(driver),
    (state) =>
      Number(asObj(state.counts).notes) === 1 &&
      Number(asObj(state.counts).deletedNotes) === 0,
  );
  await reopenNotes(driver, "after-trash-actions");
  await setNotesInput(driver, SYNTHETIC_NOTE);

  await activateActionRow(driver, "format");
  assert(
    asObj((await notesState(driver)).view).showFormatToolbar === true,
    "Actions-only Format did not execute through the shared handler",
  );
  receipt.negativeControls.formatActionsOnlyExecuted = true;
  await reopenNotes(driver, "after-format-action");
  await setNotesInput(driver, SYNTHETIC_NOTE);

  const currentEditor = await descriptors(driver);
  const remainingEditorShortcuts = currentEditor.filter(
    (descriptor) =>
      descriptor.shortcut !== null &&
      !activatedCanonical.has(descriptor.canonicalShortcut ?? "") &&
      descriptor.id !== "delete_note",
  );
  for (const descriptor of remainingEditorShortcuts) {
    const override =
      descriptor.id === "history_back"
        ? "bracketleft"
        : descriptor.id === "history_forward"
          ? "bracketright"
          : undefined;
    if (descriptor.id === "find_in_note" || descriptor.id === "send_to_ai") {
      await activateShortcutViaNotesAutomation(driver, descriptor);
    } else {
      await activateShortcut(driver, descriptor, override);
    }
    if (descriptor.id === "browse_notes") {
      await gpuiKey(driver, "escape");
      await poll(
        "note switcher close",
        () => notesState(driver),
        (state) => asObj(asObj(state.commandBars).noteSwitcher).open !== true,
      );
    } else if (descriptor.id === "toggle_preview") {
      await gpuiKey(driver, "p", ["shift", "cmd"]);
      await poll(
        "preview restored after activation proof",
        () => notesState(driver),
        (state) => asObj(state.view).previewEnabled === false,
      );
    } else if (descriptor.id === "open_trash") {
      await activateActionRow(driver, "back_to_notes");
      await poll(
        "Trash restored after activation proof",
        () => notesState(driver),
        (state) => asObj(state.view).viewMode === "AllNotes",
      );
      await reopenNotes(driver, "after-open-trash-proof");
      await setNotesInput(driver, SYNTHETIC_NOTE);
    } else if (descriptor.id === "find_in_note") {
      await reopenNotes(driver, "after-find-proof");
      await setNotesInput(driver, SYNTHETIC_NOTE);
    }
  }
  await ensureAllNotesEditor(driver);

  const allAdvertised = new Set(
    projections.flatMap((projection) =>
      projection.descriptorIds.flatMap((id) => {
        const source = [
          editorSelected,
          previewSelected,
          trashEmpty,
          readOnly,
          editorNoCurrent,
          trashSelected,
        ]
          .flat()
          .find((descriptor) => descriptor.id === id);
        return source?.canonicalShortcut ? [source.canonicalShortcut] : [];
      }),
    ),
  );
  const missingActivations = [...allAdvertised].filter(
    (shortcut) => !activatedCanonical.has(shortcut),
  );
  assert(
    missingActivations.length === 0,
    "not every advertised Notes chord was activated",
    missingActivations,
  );

  const appLog = await Bun.file(driver.logPath).text();
  assert(!/panicked at|thread 'main' panicked/i.test(appLog), "Notes runtime panicked");
  assert(!/already mutably borrowed|double lease/i.test(appLog), "Notes runtime double-leased an entity");
  receipt.advertisedShortcutCount = allAdvertised.size;
  receipt.activatedShortcutCount = activatedCanonical.size;
  receipt.allAdvertisedShortcutsActivated = true;
  receipt.runtimePanics = 0;
  targetObservation = await observeWorkflowTaskTarget(driver, BINARY, NOTES_TARGET);
  receipt.status = "PASS";
} catch (error) {
  console.error("C07 private diagnostic:", error);
  failureFingerprint = hashText(
    error instanceof Error ? `${error.name}:${error.message}` : String(error),
  );
  receipt.failureFingerprint = failureFingerprint;
} finally {
  if (pasteboard) {
    try {
      await pasteboard.restore();
      clipboardRestored = true;
    } catch (error) {
      console.error("C07 private clipboard cleanup diagnostic:", error);
      failureFingerprint ??= hashText(
        error instanceof Error ? `${error.name}:${error.message}` : String(error),
      );
      receipt.status = "FAILED";
    }
  }
  if (driver) {
    try {
      await driver.close();
    } catch (error) {
      console.error("C07 private driver cleanup diagnostic:", error);
      failureFingerprint ??= hashText(
        error instanceof Error ? `${error.name}:${error.message}` : String(error),
      );
      receipt.status = "FAILED";
    }
    const ownedPids = exactExecutablePids(BINARY);
    receipt.cleanup = {
      processExited: driver.finalization.processExited,
      streamsDrained: driver.finalization.streamsDrained,
      logWriterClosed: driver.finalization.logWriterClosed,
      ownedProcessCount: ownedPids.length,
      forcedSignals: [],
      clipboardTouched: true,
      clipboardRestored,
      databaseRemoved: false,
    };
    if (
      !driver.finalization.processExited ||
      !driver.finalization.streamsDrained ||
      !driver.finalization.logWriterClosed ||
      ownedPids.length !== 0 ||
      !clipboardRestored
    ) {
      receipt.status = "FAILED";
    }
  }
  if (databaseRoot) {
    rmSync(databaseRoot, { recursive: true, force: true });
    receipt.cleanup.databaseRemoved = true;
  }
  if (failureFingerprint) receipt.failureFingerprint = failureFingerprint;
  mkdirSync(OUT_DIR, { recursive: true });
  writeFileSync(RECEIPT_PATH, `${JSON.stringify(receipt, null, 2)}\n`, {
    mode: 0o600,
  });
  try {
    assert(receipt.status === "PASS" && targetObservation !== null, "Notes action journey did not pass");
    const segment = observedWorkflowSegment("notes-actions", targetObservation, receipt.cleanup);
    const stages = [
      {
        id: "shortcut-descriptors-executable",
        result: {
          advertised: receipt.advertisedShortcutCount,
          activated: receipt.activatedShortcutCount,
          complete: receipt.allAdvertisedShortcutsActivated,
        },
      },
      {
        id: "destructive-confirmations-enforced",
        result: receipt.destructiveConfirmations,
      },
    ].map((stage) => observedWorkflowStage({
      ...stage,
      primitiveId: "devtools.act",
      segment,
      command: "notes.activateShortcut",
      requestId: `SAFE-004:${stage.id}`,
      pass: true,
    }));
    const confirmations = asObj(receipt.destructiveConfirmations);
    const deletePolicy = asObj(confirmations.delete);
    const prepared = prepareWorkflowTaskProof("SAFE-004", {
      producerOwner: "scripts/agentic/cons-flow-ux/notes-actions-probe.ts",
      segments: [segment],
      stages,
      negativeControls: {
        "unavailable-shortcuts-cannot-activate":
          receipt.negativeControls.shiftCommandA === "no-action" &&
          receipt.negativeControls.shiftCommandDelete === "no-action",
        "destructive-action-requires-confirmation":
          deletePolicy.openedBeforeMutation === true &&
          deletePolicy.cancelPreservedCounts === true,
      },
      safety: {
        microphoneCaptureStarted: false,
        nativeInputInjected: false,
        liveAiStarted: false,
        screenTakeoverStarted: false,
        clipboardTouched: true,
        clipboardRestored,
      },
    });
    writeWorkflowTaskProof("SAFE-004", prepared.receipt);
  } catch (error) {
    writeWorkflowTaskProof("SAFE-004", prepareBlockedWorkflowTaskProof(
      "SAFE-004",
      error instanceof Error ? error.message : String(error),
    ).receipt);
  }
}

console.log(JSON.stringify(receipt, null, 2));
if (receipt.status !== "PASS") process.exit(1);
