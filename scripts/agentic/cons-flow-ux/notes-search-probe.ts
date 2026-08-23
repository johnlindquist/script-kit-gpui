#!/usr/bin/env bun

import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { openDayPage } from "../day-page-open-helper";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.notes-search");

const ROOT = resolve(import.meta.dir, "../../..");
const BINARY = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY ??
    join(ROOT, "target-agent/artifacts/cons-flow-c08/script-kit-gpui"),
);
const OUT_DIR = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR ?? join(ROOT, ".test-output/cons-flow-c08"),
);
const RECEIPT_PATH = join(OUT_DIR, "notes-search-receipt.json");
const MAIN: Json = { type: "kind", kind: "main", index: 0 };
const NOTES: Json = { type: "kind", kind: "notes", index: 0 };
const ACTIONS: Json = { type: "kind", kind: "actionsDialog", index: 0 };
const QUERY = "shared";
const ALPHA_ID = "00000000-0000-0000-0000-000000000081";
const BETA_ID = "00000000-0000-0000-0000-000000000082";
const DAY_DATE = "2026-07-03";
const EXPECTED_IDS = [ALPHA_ID, BETA_ID, `day:${DAY_DATE}`];

type Obj = Record<string, any>;
type CanonicalRow = {
  id: string;
  title: string;
  kind: string;
  metadataFingerprint: string;
  previewFingerprint?: string;
};

type HostReceipt = {
  host: string;
  destination: string;
  destinationProof: Obj;
  stateKind: string;
  selectedId: string | null;
  rows: CanonicalRow[];
};

const receipt: Obj = {
  schemaVersion: 1,
  probe: "cons-flow-c08-notes-search",
  binary: BINARY,
  artifactSha256: createHash("sha256").update(readFileSync(BINARY)).digest("hex"),
  status: "FAILED",
  hosts: {},
  stateMatrix: {},
  activation: {},
  portalRestoration: {},
  comparisons: {},
  cleanup: {},
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

function fingerprint(value: string): string {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(value)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return `fnv1a64:${hash.toString(16).padStart(16, "0")}`;
}

function failureFingerprint(error: unknown): string {
  const text = error instanceof Error ? `${error.name}:${error.message}` : String(error);
  return createHash("sha256").update(text).digest("hex").slice(0, 24);
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
  assert(predicate(value), `timed out waiting for ${label}`, value);
  return value;
}

function seedCorpus(root: string): { brain: string; db: string } {
  const brain = join(root, "brain");
  const notes = join(brain, "notes");
  const days = join(brain, "days");
  const db = join(root, "notes.sqlite");
  mkdirSync(notes, { recursive: true });
  mkdirSync(days, { recursive: true });

  writeFileSync(
    join(notes, "c08-shared-alpha.md"),
    `---\nid: ${ALPHA_ID}\ncreated: 2026-07-05T10:00:00Z\nupdated: 2026-07-05T10:00:00Z\npinned: true\n---\n\n# C08 Shared Alpha\n\n${QUERY} preview alpha\n`,
  );
  writeFileSync(
    join(notes, "c08-shared-beta.md"),
    `---\nid: ${BETA_ID}\ncreated: 2026-07-04T10:00:00Z\nupdated: 2026-07-04T10:00:00Z\n---\n\n# C08 Shared Beta\n\n${QUERY} preview beta\n`,
  );
  const dayPath = join(days, `${DAY_DATE}.md`);
  writeFileSync(dayPath, `${QUERY} preview day\nsecond line\n`);
  const dayMtime = new Date("2026-07-03T10:00:00Z");
  utimesSync(dayPath, dayMtime, dayMtime);
  return { brain, db };
}

function runSqlite(db: string, sql: string, failureMessage: string): void {
  const result = Bun.spawnSync(["/usr/bin/sqlite3", db, sql], {
    stdout: "pipe",
    stderr: "pipe",
  });
  assert(
    result.exitCode === 0,
    failureMessage,
    new TextDecoder().decode(result.stderr),
  );
}

function seedNotesDatabase(db: string): void {
  const sql = `
    INSERT OR REPLACE INTO notes
      (id, title, content, created_at, updated_at, deleted_at, is_pinned, sort_order, file_slug, content_hash)
    VALUES
      ('${ALPHA_ID}', 'C08 Shared Alpha', '# C08 Shared Alpha\n\n${QUERY} preview alpha',
       '2026-07-05T10:00:00Z', '2026-07-05T10:00:00Z', NULL, 1, 0,
       'c08-shared-alpha', NULL),
      ('${BETA_ID}', 'C08 Shared Beta', '# C08 Shared Beta\n\n${QUERY} preview beta',
       '2026-07-04T10:00:00Z', '2026-07-04T10:00:00Z', NULL, 0, 0,
       'c08-shared-beta', NULL);
  `;
  runSqlite(db, sql, "failed to seed Notes database");
}

function clearNotesDatabase(db: string): void {
  runSqlite(db, "DELETE FROM notes;", "failed to clear Notes database");
}

async function listWindows(driver: Driver): Promise<Obj[]> {
  const result = asObj(await driver.listAutomationWindows({ timeoutMs: 8_000 }));
  return asArray(result.windows).map(asObj);
}

function notesWindow(windows: Obj[]): Obj | undefined {
  return windows.find((window) => {
    const kind = String(window.kind ?? window.windowKind ?? "").toLowerCase();
    const id = String(window.id ?? window.automationId ?? "").toLowerCase();
    return (kind === "notes" || id.includes("notes-window")) && !id.includes("actions");
  });
}

function actionsWindow(windows: Obj[]): Obj | undefined {
  return windows.find((window) => {
    const kind = String(window.kind ?? window.windowKind ?? "").toLowerCase();
    const id = String(window.id ?? window.automationId ?? "").toLowerCase();
    return kind === "actionsdialog" || id === "actions-dialog";
  });
}

async function mainState(driver: Driver): Promise<Obj> {
  return asObj(await driver.getState({ timeoutMs: 8_000 }));
}

async function targetState(driver: Driver, target: Json): Promise<Obj> {
  return asObj(
    await driver.request(
      { type: "getState", target },
      { expect: "stateResult", timeoutMs: 8_000 },
    ),
  );
}

async function mainElements(driver: Driver): Promise<Obj[]> {
  const result = asObj(
    await driver.getElements({ target: MAIN, limit: 1_000 }, { timeoutMs: 8_000 }),
  );
  return asArray(result.elements).map(asObj);
}

async function actionElements(driver: Driver): Promise<Obj[]> {
  const result = asObj(
    await driver.getElements({ target: ACTIONS, limit: 1_000 }, { timeoutMs: 8_000 }),
  );
  return asArray(result.elements).map(asObj);
}

async function showMain(driver: Driver, label: string): Promise<void> {
  driver.send({ type: "show", requestId: `c08-${label}-show` });
  await poll(
    `${label} main visible`,
    () => listWindows(driver),
    (windows) => windows.some((window) => String(window.id) === "main" && window.visible === true),
  );
}

async function ensureNotesClosed(driver: Driver, label: string): Promise<void> {
  if (!notesWindow(await listWindows(driver))) return;
  driver.send({ type: "openNotes", requestId: `c08-${label}-notes-close` });
  await poll(
    `${label} Notes closed`,
    () => listWindows(driver),
    (windows) => !notesWindow(windows),
  );
}

async function openNotes(driver: Driver, label: string): Promise<void> {
  await ensureNotesClosed(driver, `${label}-pre`);
  driver.send({ type: "openNotes", requestId: `c08-${label}-notes-open` });
  await poll(
    `${label} Notes open`,
    () => listWindows(driver),
    (windows) => notesWindow(windows)?.visible === true,
  );
}

async function closeActions(driver: Driver, label: string): Promise<void> {
  if (!actionsWindow(await listWindows(driver))) return;
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "escape", modifiers: [] },
    { target: ACTIONS, timeoutMs: 8_000 },
  );
  await poll(
    `${label} Actions close`,
    () => listWindows(driver),
    (windows) => !actionsWindow(windows),
  );
  await Bun.sleep(180);
}

async function openActions(
  driver: Driver,
  target: Json,
  label: string,
): Promise<void> {
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "p", modifiers: ["cmd"] },
    { target, timeoutMs: 8_000 },
  );
  await poll(
    `${label} Actions open`,
    () => listWindows(driver),
    (windows) => actionsWindow(windows)?.visible === true,
  );
}

async function setActionsQuery(driver: Driver, query: string): Promise<Obj> {
  const result = asObj(
    await driver.request(
      {
        type: "batch",
        target: ACTIONS,
        commands: [{ type: "setInput", text: query }],
        options: { stopOnError: true, rollbackOnError: false, timeout: 8_000 },
      },
      { expect: "batchResult", timeoutMs: 10_000 },
    ),
  );
  assert(result.success === true, "Actions query failed", result);
  return poll(
    `Actions query ${query}`,
    () => targetState(driver, ACTIONS),
    (state) => Number(asObj(asObj(state.actionsDialog).search).textLength) === query.length,
  );
}

function popupRows(stateResult: Obj, elements: Obj[]): CanonicalRow[] {
  const dialog = asObj(stateResult.actionsDialog);
  const summaries = asArray(asObj(dialog.actions).visibleSample).map(asObj);
  const titles = new Map(
    elements
      .filter((element) => String(element.semanticId ?? "").startsWith("choice:"))
      .map((element) => [String(element.value ?? ""), String(element.text ?? "")]),
  );
  return summaries
    .filter((summary) => String(summary.id ?? "").startsWith("note_"))
    .map((summary) => {
      const actionId = String(summary.id);
      return {
        id: actionId.slice("note_".length),
        title: titles.get(actionId) ?? "",
        kind: actionId.startsWith("note_day:") ? "day" : "note",
        metadataFingerprint: String(summary.valueFingerprint ?? ""),
      };
    });
}

async function capturePopupHost(
  driver: Driver,
  host: string,
  destination: string,
): Promise<HostReceipt> {
  const state = await setActionsQuery(driver, QUERY);
  const elements = await actionElements(driver);
  const dialog = asObj(state.actionsDialog);
  const rows = popupRows(state, elements);
  const observedContextFingerprint = String(asObj(dialog.context).titleFingerprint ?? "");
  assert(
    observedContextFingerprint === fingerprint(destination),
    `${host} destination context is not truthful`,
    { destination, observedContextFingerprint, dialog },
  );
  assert(rows.length === EXPECTED_IDS.length, `${host} row count mismatch`, rows);
  return {
    host,
    destination,
    destinationProof: {
      visibleContextFingerprint: observedContextFingerprint,
      expectedContextFingerprint: fingerprint(destination),
      parentHost: asObj(dialog.attachedPopup).host ?? null,
    },
    stateKind: "ready",
    selectedId: String(asObj(dialog.selection).actionId ?? "").replace(/^note_/, "") || null,
    rows,
  };
}

function browseRows(elements: Obj[]): CanonicalRow[] {
  const metadata = new Map(
    elements
      .filter((element) => element.role === "resultMetadata")
      .map((element) => [
        String(element.semanticId).replace(/:metadata$/, ""),
        element,
      ]),
  );
  return elements
    .filter((element) => element.role === "result")
    .map((element) => {
      const semanticId = String(element.semanticId);
      const detail = asObj(metadata.get(semanticId));
      return {
        id: String(element.value ?? ""),
        title: String(element.text ?? ""),
        kind: String(element.kind ?? ""),
        metadataFingerprint: fingerprint(String(detail.value ?? "")),
        previewFingerprint: fingerprint(String(detail.text ?? "")),
      };
    });
}

async function captureBrowseHost(
  driver: Driver,
  host: string,
  destination: string,
): Promise<HostReceipt> {
  const elements = await mainElements(driver);
  const state = asObj(elements.find((element) => element.semanticId === "notes-search-state"));
  const action = asObj(
    elements.find(
      (element) => element.role === "action" && String(element.source) === "notes",
    ),
  );
  assert(action.text === destination, `${host} destination action is not truthful`, action);
  return {
    host,
    destination,
    destinationProof: {
      semanticAction: action.semanticId ?? null,
      visibleText: action.text ?? null,
      enabled: !action.actionDisabled,
    },
    stateKind: String(state.statusKind ?? ""),
    selectedId: typeof action.value === "string" ? action.value : null,
    rows: browseRows(elements),
  };
}

async function setBrowseQuery(driver: Driver, query: string): Promise<HostReceipt> {
  await driver.setFilterAndWait(query, { timeoutMs: 10_000 });
  await poll(
    `Notes Browse query ${query}`,
    () => mainElements(driver),
    (elements) => {
      const input = elements.find((element) => element.semanticId === "input:notes-browse-filter");
      return input?.value === query;
    },
  );
  const elements = await mainElements(driver);
  const action = asObj(elements.find((element) => element.role === "action"));
  return captureBrowseHost(driver, "notesBrowse", String(action.text ?? ""));
}

async function stageLauncher(driver: Driver, label: string): Promise<void> {
  await ensureNotesClosed(driver, `${label}-pre`);
  await showMain(driver, label);
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const state = await mainState(driver);
    if (state.promptType === "none") break;
    driver.simulateKey("escape");
    await Bun.sleep(120);
  }
}

async function openStandaloneBrowse(driver: Driver, label: string): Promise<void> {
  await stageLauncher(driver, label);
  await driver.setFilterAndWait("Search Notes", { timeoutMs: 10_000 });
  const elements = asObj(
    await driver.getElements({ target: MAIN, limit: 1_000 }, { timeoutMs: 8_000 }),
  );
  const searchNotesRow = asArray(elements.elements)
    .map(asObj)
    .find(
      (element) =>
        element.value === "builtin/search-notes" ||
        element.text === "Search Notes" ||
        String(element.semanticId ?? "").includes("builtin/search-notes"),
    );
  const semanticId = String(searchNotesRow?.semanticId ?? "");
  assert(semanticId.length > 0, "Search Notes launcher row has no semantic id", {
    state: await mainState(driver),
    elements,
  });
  const activation = asObj(
    await driver.batch(
      [{ type: "selectBySemanticId", semanticId, submit: true }],
      { timeoutMs: 8_000, stopOnError: true },
    ),
  );
  assert(activation.success === true, "Search Notes launcher selection failed", activation);
  const submit = asObj(
    await driver.simulateGpuiEvent(
      { type: "keyDown", key: "enter", modifiers: [] },
      { target: MAIN, timeoutMs: 8_000 },
    ),
  );
  assert(submit.success !== false, "Search Notes launcher submit failed", submit);
  await poll(
    `${label} standalone Notes Browse`,
    () => mainState(driver),
    (state) => state.promptType === "notesBrowse",
  );
}

async function openEmbeddedAgentChat(driver: Driver, label: string): Promise<void> {
  await stageLauncher(driver, `${label}-agent-chat`);
  await driver.setFilterAndWait("Agent Chat", { timeoutMs: 10_000 });
  const elements = asObj(
    await driver.getElements({ target: MAIN, limit: 1_000 }, { timeoutMs: 8_000 }),
  );
  const row = asArray(elements.elements)
    .map(asObj)
    .find(
      (element) =>
        element.text === "Agent Chat" &&
        (element.kind === "built-in" ||
          String(element.semanticId ?? "").includes("ai-chat")),
    );
  const semanticId = String(row?.semanticId ?? "");
  assert(semanticId.length > 0, `${label} Agent Chat launcher row missing`, elements);
  const selected = asObj(
    await driver.batch(
      [{ type: "selectBySemanticId", semanticId }],
      { timeoutMs: 8_000, stopOnError: true },
    ),
  );
  assert(selected.success === true, `${label} Agent Chat selection failed`, selected);
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "enter", modifiers: [] },
    { target: MAIN, timeoutMs: 8_000 },
  );
  await poll(
    `${label} Agent Chat ready`,
    () => mainState(driver),
    (state) => state.promptType === "agentChatChat",
  );
}

async function openNotesPortalFromAgentChat(
  driver: Driver,
  draft: string,
  label: string,
): Promise<{ before: Obj; beforeElements: Obj[] }> {
  await poll(
    `${label} current Agent Chat ready`,
    () => mainState(driver),
    (state) => state.promptType === "agentChatChat",
  );
  const setInput = asObj(
    await driver.request({ type: "setAgentChatInput", text: draft }, { timeoutMs: 10_000 }),
  );
  assert(setInput.ok === true, `${label} Agent Chat draft failed`, setInput);
  const before = await poll(
    `${label} Agent Chat draft ready`,
    () => driver.request({ type: "getAgentChatState" }, { timeoutMs: 8_000 }).then(asObj),
    (state) => state.inputText === draft,
  );
  const beforeElements = await mainElements(driver);
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: ".", modifiers: ["cmd"] },
    { target: MAIN, timeoutMs: 10_000 },
  );
  await poll(
    `${label} Notes portal ready`,
    () => mainState(driver),
    (state) => state.promptType === "notesBrowse",
  );
  return { before, beforeElements };
}

async function openNotesPortal(
  driver: Driver,
  draft: string,
  label: string,
): Promise<{ before: Obj; beforeElements: Obj[] }> {
  await openEmbeddedAgentChat(driver, label);
  return openNotesPortalFromAgentChat(driver, draft, label);
}

async function waitForAgentChat(driver: Driver, label: string): Promise<Obj> {
  await poll(
    `${label} Agent Chat restored`,
    () => mainState(driver),
    (state) => state.promptType === "agentChatChat",
  );
  return asObj(
    await driver.request({ type: "getAgentChatState" }, { timeoutMs: 8_000 }),
  );
}

async function firstBrowseRowPoint(driver: Driver): Promise<{ x: number; y: number; layout: Obj }> {
  const layout = asObj(
    await driver.getLayoutInfo({ target: MAIN }, { timeoutMs: 8_000 }),
  );
  const components = asArray(layout.components).map(asObj);
  const header = asObj(components.find((component) => component.name === "MainViewHeader"));
  const windowBounds = asObj(layout.windowBounds);
  const headerBottom = Number(asObj(header.bounds).y ?? 0) + Number(asObj(header.bounds).height ?? 76);
  const width = Number(windowBounds.width ?? 750);
  return { x: Math.max(80, width * 0.25), y: headerBottom + 48, layout };
}

async function pointerActivateFirstRow(driver: Driver, label: string): Promise<Obj> {
  const destination = label.includes("portal") ? "Attach Note" : "Open in Notes Window";
  const initial = await captureBrowseHost(driver, label, destination);
  assert(initial.rows.length >= 2, `${label} needs at least two rows for second-click proof`, initial);
  const point = await firstBrowseRowPoint(driver);
  const selectSecond = asObj(
    await driver.simulateGpuiEvent(
      { type: "mouseClick", x: point.x, y: point.y + 44, button: "left" },
      { target: MAIN, timeoutMs: 8_000 },
    ),
  );
  const before = await poll(
    `${label} second row selected`,
    () => captureBrowseHost(driver, label, destination),
    (host) => host.selectedId === host.rows[1]?.id,
  );
  const first = asObj(
    await driver.simulateGpuiEvent(
      { type: "mouseClick", x: point.x, y: point.y, button: "left" },
      { target: MAIN, timeoutMs: 8_000 },
    ),
  );
  const afterFirst = await poll(
    `${label} first click selects first row`,
    () => captureBrowseHost(driver, label, before.destination),
    (host) => host.selectedId === host.rows[0]?.id,
  );
  const second = asObj(
    await driver.simulateGpuiEvent(
      { type: "mouseClick", x: point.x, y: point.y, button: "left" },
      { target: MAIN, timeoutMs: 8_000 },
    ),
  );
  return {
    point: { x: point.x, y: point.y },
    selectSecondDispatch: selectSecond,
    firstDispatch: first,
    secondDispatch: second,
    selectedAfterFirst: afterFirst.selectedId,
  };
}

function assertCanonicalParity(hosts: HostReceipt[]): void {
  for (const host of hosts) {
    assert(
      JSON.stringify(host.rows.map((row) => row.id)) === JSON.stringify(EXPECTED_IDS),
      `${host.host} canonical IDs/order mismatch`,
      host,
    );
  }
  const baseline = hosts[0].rows;
  for (const host of hosts.slice(1)) {
    assert(
      JSON.stringify(host.rows.map(({ id, title, kind, metadataFingerprint }) => ({
        id,
        title,
        kind,
        metadataFingerprint,
      }))) ===
        JSON.stringify(baseline.map(({ id, title, kind, metadataFingerprint }) => ({
          id,
          title,
          kind,
          metadataFingerprint,
        }))),
      `${host.host} canonical row metadata differs from ${hosts[0].host}`,
      { baseline, observed: host.rows },
    );
  }
}

const fixtureRoot = mkdtempSync(join(tmpdir(), "cons-flow-c08-"));
const seeded = seedCorpus(fixtureRoot);
const emptyRoot = mkdtempSync(join(tmpdir(), "cons-flow-c08-empty-"));
const emptyBrain = join(emptyRoot, "brain");
const emptyDb = join(emptyRoot, "notes.sqlite");
mkdirSync(join(emptyBrain, "notes"), { recursive: true });
mkdirSync(join(emptyBrain, "days"), { recursive: true });

let driver: Driver | null = null;
let emptyDriver: Driver | null = null;
let failure: string | null = null;

try {
  driver = await Driver.launch({
    binary: BINARY,
    sessionName: "cons-flow-c08-notes-search",
    sandboxHome: true,
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 10_000,
    env: {
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_TEST_NOTES_BRAIN_PATH: seeded.brain,
      SCRIPT_KIT_TEST_NOTES_DB_PATH: seeded.db,
      SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
    },
  });

  await openNotes(driver, "notes-schema-init");
  await ensureNotesClosed(driver, "notes-schema-init");
  await Bun.sleep(300);
  seedNotesDatabase(seeded.db);
  await Bun.sleep(300);
  seedNotesDatabase(seeded.db);

  await openNotes(driver, "notes-window");
  await openActions(driver, NOTES, "Notes Window switcher");
  const notesHost = await capturePopupHost(driver, "notesWindow", "Open in Notes");
  receipt.hosts.notesWindow = notesHost;
  await closeActions(driver, "Notes Window switcher");
  await ensureNotesClosed(driver, "after-notes-window");

  await showMain(driver, "day-page");
  const dayOpened = asObj(await openDayPage(driver, "cons-flow-c08-day-page"));
  assert(dayOpened.promptType === "dayPage", "Day Page did not open", dayOpened);
  await openActions(driver, MAIN, "Day Page switcher");
  const dayHost = await capturePopupHost(driver, "dayPage", "Open Here");
  receipt.hosts.dayPage = dayHost;
  await closeActions(driver, "Day Page switcher");

  await openStandaloneBrowse(driver, "standalone");
  let standaloneHost = await setBrowseQuery(driver, QUERY);
  standaloneHost.host = "standalone";
  assert(standaloneHost.destination === "Open in Notes Window", "standalone verb mismatch", standaloneHost);
  receipt.hosts.standalone = standaloneHost;

  const portalDraft = `Review @note:"${QUERY}"`;
  await showMain(driver, "portal-cancel");
  const cancelOrigin = await openNotesPortal(driver, portalDraft, "portal-cancel");
  const portalHost = await captureBrowseHost(driver, "agentChatPortal", "Attach Note");
  receipt.hosts.agentChatPortal = portalHost;
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "down", modifiers: [] },
    { target: MAIN, timeoutMs: 8_000 },
  );
  const portalSelection = await captureBrowseHost(driver, "agentChatPortal", "Attach Note");
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "escape", modifiers: [] },
    { target: MAIN, timeoutMs: 8_000 },
  );
  const restored = await waitForAgentChat(driver, "portal cancel");
  const restoredElements = await mainElements(driver);
  const restoredComposer = asObj(
    restoredElements.find((element) => element.semanticId === "input:agent-chat-composer"),
  );
  assert(restored.inputText === cancelOrigin.before.inputText, "portal cancel lost draft/query", {
    before: cancelOrigin.before,
    restored,
  });
  assert(restored.cursorIndex === cancelOrigin.before.cursorIndex, "portal cancel lost cursor selection", {
    before: cancelOrigin.before,
    restored,
  });
  assert(restored.contextChipCount === cancelOrigin.before.contextChipCount, "portal cancel changed context", {
    before: cancelOrigin.before,
    restored,
  });
  assert(restoredComposer.focused === true, "portal cancel did not restore composer focus", restoredComposer);
  receipt.portalRestoration = {
    draftRestored: true,
    queryRestored: restored.inputText === portalDraft,
    cursorRestored: true,
    contextRestored: true,
    focusRestored: true,
    portalSelectedIdBeforeCancel: portalSelection.selectedId,
  };

  const attachOrigin = await openNotesPortalFromAgentChat(driver, portalDraft, "portal-enter");
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "enter", modifiers: [] },
    { target: MAIN, timeoutMs: 8_000 },
  );
  const attached = await waitForAgentChat(driver, "portal Enter attach");
  const enterPortalParts = asArray(attached.contextParts)
    .map(asObj)
    .filter((part) => part.provenance === "attachmentPortal" && part.targetKind === "note");
  assert(
    enterPortalParts.length === 1,
    "portal Enter did not attach exactly one note",
    { before: attachOrigin.before, attached },
  );
  receipt.activation.portalEnter = {
    contextBefore: attachOrigin.before.contextChipCount,
    contextAfter: attached.contextChipCount,
    portalPartCount: enterPortalParts.length,
    attachedExactlyOnce: true,
  };

  const pointerOrigin = await openNotesPortalFromAgentChat(driver, portalDraft, "portal-pointer");
  const portalPointer = await pointerActivateFirstRow(driver, "agentChat-portal-pointer");
  const pointerAttached = await waitForAgentChat(driver, "portal second-click attach");
  const pointerPortalParts = asArray(pointerAttached.contextParts)
    .map(asObj)
    .filter((part) => part.provenance === "attachmentPortal" && part.targetKind === "note");
  assert(
    pointerPortalParts.length === 1,
    "portal second click did not attach exactly one note",
    { before: pointerOrigin.before, pointerAttached },
  );
  receipt.activation.portalSecondClick = {
    ...portalPointer,
    contextBefore: pointerOrigin.before.contextChipCount,
    contextAfter: pointerAttached.contextChipCount,
    portalPartCount: pointerPortalParts.length,
    attachedExactlyOnce: true,
  };

  await openStandaloneBrowse(driver, "standalone-states");
  await setBrowseQuery(driver, QUERY);
  const noMatch = await setBrowseQuery(driver, "c08-no-such-note");
  assert(noMatch.stateKind === "noMatch" && noMatch.rows.length === 0, "NoMatch became another state", noMatch);
  receipt.stateMatrix.noMatch = {
    stateKind: noMatch.stateKind,
    rowCount: noMatch.rows.length,
    destination: noMatch.destination,
  };

  standaloneHost = await setBrowseQuery(driver, QUERY);
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "down", modifiers: [] },
    { target: MAIN, timeoutMs: 8_000 },
  );
  const selectedBeforeRefresh = await captureBrowseHost(
    driver,
    "standalone",
    "Open in Notes Window",
  );
  const stableSelection = await setBrowseQuery(driver, "c08");
  assert(
    stableSelection.selectedId === selectedBeforeRefresh.selectedId,
    "selection was preserved by stale index instead of stable ID",
    { selectedBeforeRefresh, stableSelection },
  );
  receipt.stateMatrix.stableSelection = {
    selectedId: stableSelection.selectedId,
    expectedId: selectedBeforeRefresh.selectedId,
  };

  await setBrowseQuery(driver, QUERY);
  const loading = await setBrowseQuery(driver, "__notes_search_loading__");
  assert(
    loading.stateKind === "loading" &&
      JSON.stringify(loading.rows.map((row) => row.id)) === JSON.stringify(EXPECTED_IDS),
    "Loading did not retain the prior snapshot",
    loading,
  );
  receipt.stateMatrix.loading = {
    stateKind: loading.stateKind,
    retainedIds: loading.rows.map((row) => row.id),
  };

  await setBrowseQuery(driver, QUERY);
  const failed = await setBrowseQuery(driver, "__notes_search_failure__");
  assert(
    failed.stateKind === "failed" &&
      JSON.stringify(failed.rows.map((row) => row.id)) === JSON.stringify(EXPECTED_IDS),
    "Failed refresh did not retain the prior snapshot",
    failed,
  );
  receipt.stateMatrix.failed = {
    stateKind: failed.stateKind,
    retainedIds: failed.rows.map((row) => row.id),
    notEmpty: failed.rows.length > 0,
  };

  await setBrowseQuery(driver, QUERY);
  await driver.simulateGpuiEvent(
    { type: "keyDown", key: "enter", modifiers: [] },
    { target: MAIN, timeoutMs: 8_000 },
  );
  const enterNotesWindows = await poll(
    "standalone Enter opens Notes",
    () => listWindows(driver!),
    (windows) => notesWindow(windows)?.visible === true,
  );
  receipt.activation.standaloneEnter = {
    openedNotes: true,
    notesWindowId: notesWindow(enterNotesWindows)?.id ?? null,
  };
  await ensureNotesClosed(driver, "standalone-enter");

  await openStandaloneBrowse(driver, "standalone-pointer");
  await setBrowseQuery(driver, QUERY);
  const standalonePointer = await pointerActivateFirstRow(driver, "standalone-pointer");
  await poll(
    "standalone second click opens Notes",
    () => listWindows(driver!),
    (windows) => notesWindow(windows)?.visible === true,
  );
  receipt.activation.standaloneSecondClick = {
    ...standalonePointer,
    openedNotes: true,
  };
  await ensureNotesClosed(driver, "standalone-pointer");

  const hosts = [notesHost, dayHost, standaloneHost, portalHost];
  assertCanonicalParity(hosts);
  receipt.comparisons = {
    sameOrderedIds: true,
    sameTitles: true,
    sameKinds: true,
    sameMetadataFingerprints: true,
    orderedIds: EXPECTED_IDS,
    destinationVerbs: hosts.map((host) => ({ host: host.host, verb: host.destination })),
  };

  emptyDriver = await Driver.launch({
    binary: BINARY,
    sessionName: "cons-flow-c08-notes-search-empty",
    sandboxHome: true,
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 10_000,
    env: {
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_TEST_NOTES_BRAIN_PATH: emptyBrain,
      SCRIPT_KIT_TEST_NOTES_DB_PATH: emptyDb,
      SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
    },
  });
  await openNotes(emptyDriver, "empty-schema-init");
  await ensureNotesClosed(emptyDriver, "empty-schema-init");
  await Bun.sleep(300);
  clearNotesDatabase(emptyDb);
  await Bun.sleep(300);
  clearNotesDatabase(emptyDb);
  await openStandaloneBrowse(emptyDriver, "empty-corpus");
  const readyEmpty = await captureBrowseHost(
    emptyDriver,
    "standaloneEmpty",
    "Open in Notes Window",
  );
  assert(
    readyEmpty.stateKind === "readyEmpty" && readyEmpty.rows.length === 0,
    "empty corpus did not produce ReadyEmpty",
    readyEmpty,
  );
  receipt.stateMatrix.readyEmpty = {
    stateKind: readyEmpty.stateKind,
    rowCount: readyEmpty.rows.length,
  };

  const appLog = await Bun.file(driver.logPath).text();
  const emptyLog = await Bun.file(emptyDriver.logPath).text();
  assert(!/panicked at|thread 'main' panicked/i.test(`${appLog}\n${emptyLog}`), "runtime panicked");
  receipt.runtimePanics = 0;
  receipt.status = "PASS";
} catch (error) {
  console.error("C08 private diagnostic:", error);
  failure = failureFingerprint(error);
  receipt.failureFingerprint = failure;
} finally {
  const finalizations: Obj[] = [];
  for (const active of [emptyDriver, driver]) {
    if (!active) continue;
    try {
      await active.close();
    } catch (error) {
      console.error("C08 private driver cleanup diagnostic:", error);
      failure ??= failureFingerprint(error);
      receipt.failureFingerprint = failure;
      receipt.status = "FAILED";
    }
    finalizations.push({
      processExited: active.finalization.processExited,
      streamsDrained: active.finalization.streamsDrained,
      logWriterClosed: active.finalization.logWriterClosed,
    });
  }
  const ownedPids = exactExecutablePids(BINARY);
  rmSync(fixtureRoot, { recursive: true, force: true });
  rmSync(emptyRoot, { recursive: true, force: true });
  receipt.cleanup = {
    sessions: finalizations,
    processExited: finalizations.every((item) => item.processExited === true),
    streamsDrained: finalizations.every((item) => item.streamsDrained === true),
    logWriterClosed: finalizations.every((item) => item.logWriterClosed === true),
    ownedProcessCount: ownedPids.length,
    forcedSignals: [],
    databaseRemoved: true,
  };
  if (
    !receipt.cleanup.processExited ||
    !receipt.cleanup.streamsDrained ||
    !receipt.cleanup.logWriterClosed ||
    ownedPids.length !== 0
  ) {
    receipt.status = "FAILED";
  }
  mkdirSync(OUT_DIR, { recursive: true });
  writeFileSync(RECEIPT_PATH, `${JSON.stringify(receipt, null, 2)}\n`, {
    mode: 0o600,
  });
}

console.log(JSON.stringify(receipt, null, 2));
if (receipt.status !== "PASS") process.exit(1);
