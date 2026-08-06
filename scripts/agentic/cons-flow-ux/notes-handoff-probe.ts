#!/usr/bin/env bun
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";

const PROJECT_ROOT = resolve(import.meta.dir, "../../..");
const BINARY = resolve(
  process.env.PROBE_BINARY ??
    process.env.SCRIPT_KIT_GPUI_BINARY ??
    join(PROJECT_ROOT, "target-agent/artifacts/cons-flow-c10/script-kit-gpui"),
);
const OUT_DIR = join(PROJECT_ROOT, ".test-output", "cons-flow-c10");
const OUT_PATH = join(OUT_DIR, "notes-handoff-receipt.json");
const NOTES_TARGET: Json = { type: "kind", kind: "notes", index: 0 };
const DETACHED_TARGET: Json = { type: "kind", kind: "agentChatDetached", index: 0 };
const runId = `notes-handoff-${Date.now().toString(36)}`;

type Obj = Record<string, any>;
type ScenarioReceipt = {
  id: string;
  pass: boolean;
  failures: string[];
  facts: Obj;
  cleanup: Obj;
  databaseRemoved: boolean;
};

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Obj) : {};
}

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(`${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`);
  }
}

function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

function sqlQuote(value: string): string {
  return `'${value.replaceAll("'", "''")}'`;
}

function sqlite(dbPath: string, sql: string): string {
  const result = Bun.spawnSync(["/usr/bin/sqlite3", dbPath, sql], {
    stdout: "pipe",
    stderr: "pipe",
  });
  assert(result.exitCode === 0, "sqlite fixture command failed", {
    stderr: new TextDecoder().decode(result.stderr),
  });
  return new TextDecoder().decode(result.stdout).trim();
}

function insertCartText(
  dbPath: string,
  noteId: string,
  id: string,
  label: string,
  source: string,
  text: string,
  sortOrder: number,
) {
  const now = "2026-08-05T12:00:00Z";
  const payload = JSON.stringify({
    kind: "text",
    text,
    source,
    mimeType: "text/plain",
  });
  sqlite(
    dbPath,
    `INSERT INTO note_cart_items (id,note_id,label,payload_json,created_at,updated_at,sort_order) VALUES (${sqlQuote(id)},${sqlQuote(noteId)},${sqlQuote(label)},${sqlQuote(payload)},${sqlQuote(now)},${sqlQuote(now)},${sortOrder});`,
  );
}

function cartIds(dbPath: string, noteId: string): string[] {
  const output = sqlite(
    dbPath,
    `SELECT id FROM note_cart_items WHERE note_id=${sqlQuote(noteId)} ORDER BY sort_order,id;`,
  );
  return output ? output.split("\n").filter(Boolean) : [];
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["/bin/ps", "-axo", "pid=,command="], {
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
  accepts: (value: T) => boolean,
  timeoutMs = 15_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let last = await read();
  while (Date.now() < deadline) {
    if (accepts(last)) return last;
    await Bun.sleep(75);
    last = await read();
  }
  throw new Error(`${label} timed out: ${JSON.stringify(last)}`);
}

async function notesState(driver: Driver): Promise<Obj> {
  const state = asObj(
    await driver.request(
      { type: "getState", target: NOTES_TARGET },
      { expect: "stateResult", timeoutMs: 8000 },
    ),
  );
  return asObj(state.notes ?? state);
}

async function mainState(driver: Driver): Promise<Obj> {
  return asObj(await driver.getState({ timeoutMs: 8000 }));
}

async function chatState(driver: Driver, target: Json = { type: "main" }): Promise<Obj> {
  return asObj(
    await driver.request(
      { type: "getAgentChatState", target },
      { expect: "agent_chatStateResult", timeoutMs: 8000 },
    ),
  );
}

async function openNotes(driver: Driver, suffix: string): Promise<Obj> {
  driver.send({ type: "openNotes", requestId: `${runId}-${suffix}-open-notes` });
  await poll(
    `${suffix} Notes window`,
    async () => asObj(await driver.listAutomationWindows({ timeoutMs: 5000 })),
    (state) =>
      Array.isArray(state.windows) && state.windows.some((window: Obj) => window.kind === "notes"),
  );
  await driver.request(
    {
      type: "batch",
      requestId: `${runId}-${suffix}-seed-note`,
      target: NOTES_TARGET,
      commands: [{ type: "setInput", text: `C10 synthetic note ${suffix}` }],
      options: { stopOnError: true, timeout: 5000 },
    },
    { expect: "batchResult", timeoutMs: 7000 },
  );
  return poll(
    `${suffix} active note`,
    () => notesState(driver),
    (state) => typeof state.activeNoteId === "string" && state.activeNoteId.length > 0,
  );
}

async function commandEnterNotes(driver: Driver, priorGeneration: number): Promise<Obj> {
  driver.send({
    type: "simulateKey",
    key: "enter",
    modifiers: ["cmd"],
    target: NOTES_TARGET,
  });
  return poll(
    "Notes transactional handoff receipt",
    () => notesState(driver),
    (state) => Number(asObj(state.lastAiHandoff).generation ?? 0) > priorGeneration,
    20_000,
  );
}

function detachedFingerprint(state: Obj): string {
  return sha256(
    JSON.stringify({
      threadId: state.threadId ?? null,
      messageCount: state.messageCount ?? null,
      inputText: state.inputText ?? null,
      contextChipCount: state.contextChipCount ?? null,
      status: state.status ?? null,
    }),
  );
}

async function runScenario(
  id: string,
  extraEnv: Record<string, string>,
  body: (driver: Driver, dbPath: string, facts: Obj) => Promise<void>,
): Promise<ScenarioReceipt> {
  const databaseRoot = mkdtempSync(join(tmpdir(), `cons-flow-c10-${id}-`));
  const dbPath = join(databaseRoot, "notes.db");
  const failures: string[] = [];
  const facts: Obj = {};
  let driver: Driver | null = null;
  let cleanup: Obj = {};
  try {
    driver = await Driver.launch({
      binary: BINARY,
      sessionName: `cons-flow-c10-${id}`,
      sandboxHome: true,
      seedAgentAuth: true,
      sharedModels: false,
      readyTimeoutMs: 30_000,
      defaultTimeoutMs: 15_000,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1",
        SCRIPT_KIT_TEST_NOTES_DB_PATH: dbPath,
        SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
        SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
        SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
        ...extraEnv,
      },
    });
    await driver.waitForSettle();
    await body(driver, dbPath, facts);
  } catch (error) {
    failures.push(error instanceof Error ? error.message : String(error));
  } finally {
    if (driver) {
      await driver.close().catch((error) => failures.push(`driver.close: ${String(error)}`));
      cleanup = asObj(driver.finalization);
      if (
        cleanup.processExited !== true ||
        cleanup.streamsDrained !== true ||
        cleanup.logWriterClosed !== true
      ) {
        failures.push("incomplete Driver finalization");
      }
    }
    rmSync(databaseRoot, { recursive: true, force: true });
  }
  return {
    id,
    pass: failures.length === 0,
    failures,
    facts,
    cleanup,
    databaseRemoved: !existsSync(databaseRoot),
  };
}

const scenarios: ScenarioReceipt[] = [];

scenarios.push(
  await runScenario("partial-duplicate-reuse", {}, async (driver, dbPath, facts) => {
    driver.send({ type: "openAgentChatDetachedFixture", requestId: `${runId}-detached` });
    await poll(
      "detached Agent Chat",
      async () => asObj(await driver.listAutomationWindows()),
      (state) =>
        Array.isArray(state.windows) &&
        state.windows.some((window: Obj) => window.kind === "agentChatDetached"),
    );
    const detachedBefore = await chatState(driver, DETACHED_TARGET);
    const detachedBeforeFingerprint = detachedFingerprint(detachedBefore);

    const notes = await openNotes(driver, "partial");
    const noteId = String(notes.activeNoteId);
    const acceptedSource = `test://accepted/${runId}`;
    const acceptedText = `accepted-${runId}`;
    const acceptedId = `${runId}-accepted`;
    const failedId = `${runId}-failed`;
    insertCartText(dbPath, noteId, acceptedId, "Accepted fixture", acceptedSource, acceptedText, 0);
    insertCartText(
      dbPath,
      noteId,
      failedId,
      "Refused fixture",
      "test://notes-handoff-refuse",
      `refused-${runId}`,
      1,
    );

    const first = await commandEnterNotes(driver, 0);
    const firstHandoff = asObj(first.lastAiHandoff);
    assert(firstHandoff.status === "stagedPartial", "first handoff was not partial", firstHandoff);
    assert(firstHandoff.supplementalAcceptedCount === 1, "accepted row not receipted", firstHandoff);
    assert(firstHandoff.supplementalFailedCount === 1, "failed row not receipted", firstHandoff);
    assert(firstHandoff.cartConsumedCount === 1, "accepted row was not consumed", firstHandoff);
    assert(
      JSON.stringify(cartIds(dbPath, noteId)) === JSON.stringify([failedId]),
      "failed cart row was not retained",
      cartIds(dbPath, noteId),
    );
    const firstChat = await chatState(driver);
    const firstMain = await mainState(driver);
    assert(
      String(firstMain.promptType ?? "").toLowerCase().includes("agent"),
      "unsuitable main host did not open a proper embedded Agent Chat",
      firstMain,
    );
    assert(Number(firstChat.messageCount ?? 0) === 0, "Notes handoff auto-submitted a turn", firstChat);
    assert(Number(firstChat.contextChipCount ?? 0) >= 2, "first visible chat omitted staged context", firstChat);
    const firstLog = await Bun.file(driver.logPath).text();
    assert(
      firstLog.includes("event=notes_ai_handoff_retry_completed"),
      "re-entrant main-window handoff did not complete its bounded immutable retry",
      { logPath: driver.logPath },
    );

    const preservedDraft = `C10 preserved draft ${runId}`;
    await driver.request(
      {
        type: "batch",
        requestId: `${runId}-preserve-draft`,
        target: { type: "main" },
        commands: [{ type: "setInput", text: preservedDraft }],
        options: { stopOnError: true, timeout: 5000 },
      },
      { expect: "batchResult", timeoutMs: 7000 },
    );
    const notesBeforeDuplicate = await notesState(driver);
    const duplicateId = `${runId}-duplicate`;
    insertCartText(
      dbPath,
      noteId,
      duplicateId,
      "Accepted fixture",
      acceptedSource,
      acceptedText,
      2,
    );
    const contextCountBefore = Number(firstChat.contextChipCount ?? 0);
    const second = await commandEnterNotes(
      driver,
      Number(asObj(notesBeforeDuplicate.lastAiHandoff).generation ?? 0),
    );
    const secondHandoff = asObj(second.lastAiHandoff);
    assert(secondHandoff.status === "stagedPartial", "second handoff was not partial", secondHandoff);
    assert(secondHandoff.supplementalDuplicateCount === 1, "duplicate row not receipted", secondHandoff);
    assert(secondHandoff.supplementalFailedCount === 1, "failed row not retained on retry", secondHandoff);
    assert(secondHandoff.cartConsumedCount === 1, "duplicate row was not consumed", secondHandoff);
    assert(
      JSON.stringify(cartIds(dbPath, noteId)) === JSON.stringify([failedId]),
      "duplicate consumption removed the failed row",
      cartIds(dbPath, noteId),
    );
    const secondChat = await chatState(driver);
    assert(secondChat.inputText === preservedDraft, "reused main chat draft changed", secondChat);
    assert(Number(secondChat.messageCount ?? 0) === 0, "second handoff auto-submitted", secondChat);
    assert(
      firstChat.threadId == null || secondChat.threadId === firstChat.threadId,
      "reused main handoff replaced the existing thread",
      { firstThread: firstChat.threadId ?? null, secondThread: secondChat.threadId ?? null },
    );
    assert(
      Number(secondChat.contextChipCount ?? 0) === contextCountBefore,
      "canonical duplicate created another context chip",
      { before: contextCountBefore, after: secondChat.contextChipCount },
    );

    const detachedAfter = await chatState(driver, DETACHED_TARGET);
    const detachedAfterFingerprint = detachedFingerprint(detachedAfter);
    assert(
      detachedAfterFingerprint === detachedBeforeFingerprint,
      "Notes handoff mutated detached Agent Chat",
      { detachedBeforeFingerprint, detachedAfterFingerprint },
    );

    facts.noteIdentityFingerprint = sha256(noteId).slice(0, 24);
    facts.first = {
      status: firstHandoff.status,
      accepted: firstHandoff.supplementalAcceptedCount,
      failed: firstHandoff.supplementalFailedCount,
      consumed: firstHandoff.cartConsumedCount,
    };
    facts.second = {
      status: secondHandoff.status,
      duplicate: secondHandoff.supplementalDuplicateCount,
      failed: secondHandoff.supplementalFailedCount,
      consumed: secondHandoff.cartConsumedCount,
      draftLength: String(secondChat.inputText ?? "").length,
      messageCount: secondChat.messageCount,
    };
    facts.failedCartRowsRetained = cartIds(dbPath, noteId).length;
    facts.mainThreadPreserved = true;
    facts.immutableRetryObserved = true;
    facts.firstVisibleContextStaged = true;
    facts.detachedUnchanged = true;
  }),
);

scenarios.push(
  await runScenario(
    "primary-failure-atomic",
    { SCRIPT_KIT_TEST_NOTES_PRIMARY_STAGE_FAIL: "1" },
    async (driver, dbPath, facts) => {
      const notes = await openNotes(driver, "primary-failure");
      const noteId = String(notes.activeNoteId);
      const rowId = `${runId}-primary-failure-row`;
      insertCartText(
        dbPath,
        noteId,
        rowId,
        "Atomic fixture",
        `test://atomic/${runId}`,
        `atomic-${runId}`,
        0,
      );
      const after = await commandEnterNotes(driver, 0);
      const handoff = asObj(after.lastAiHandoff);
      assert(handoff.status === "failed", "primary refusal did not fail", handoff);
      assert(handoff.cartConsumedCount === 0, "primary refusal consumed a cart row", handoff);
      assert(
        JSON.stringify(cartIds(dbPath, noteId)) === JSON.stringify([rowId]),
        "primary refusal changed persisted cart",
        cartIds(dbPath, noteId),
      );
      const main = await mainState(driver);
      assert(
        !String(main.promptType ?? "").toLowerCase().includes("agent"),
        "primary refusal replaced source with Agent Chat",
        main,
      );
      facts.status = handoff.status;
      facts.consumed = handoff.cartConsumedCount;
      facts.cartRowsRetained = 1;
      facts.sourcePreserved = true;
    },
  ),
);

scenarios.push(
  await runScenario(
    "cart-delete-failure",
    { SCRIPT_KIT_TEST_NOTES_CART_DELETE_FAIL: "1" },
    async (driver, dbPath, facts) => {
      const notes = await openNotes(driver, "delete-failure");
      const noteId = String(notes.activeNoteId);
      const rowId = `${runId}-delete-failure-row`;
      insertCartText(
        dbPath,
        noteId,
        rowId,
        "Delete failure fixture",
        `test://delete-failure/${runId}`,
        `delete-failure-${runId}`,
        0,
      );
      const after = await commandEnterNotes(driver, 0);
      const handoff = asObj(after.lastAiHandoff);
      assert(
        handoff.status === "stagedCartRetained",
        "cart-delete failure did not report retained staging",
        handoff,
      );
      assert(handoff.supplementalAcceptedCount === 1, "attachment was not staged", handoff);
      assert(handoff.cartConsumedCount === 0, "failed delete reported consumption", handoff);
      assert(
        JSON.stringify(cartIds(dbPath, noteId)) === JSON.stringify([rowId]),
        "cart-delete failure removed the row",
        cartIds(dbPath, noteId),
      );
      const chat = await chatState(driver);
      assert(Number(chat.messageCount ?? 0) === 0, "cart-delete scenario auto-submitted", chat);
      facts.status = handoff.status;
      facts.accepted = handoff.supplementalAcceptedCount;
      facts.consumed = handoff.cartConsumedCount;
      facts.cartRowsRetained = 1;
      facts.messageCount = chat.messageCount;
    },
  ),
);

const pids = exactExecutablePids(BINARY);
const failures = scenarios.flatMap((scenario) => scenario.failures.map((failure) => `${scenario.id}: ${failure}`));
if (pids.length > 0) failures.push(`owned executable processes remain: ${pids.join(",")}`);
const receipt = {
  schemaVersion: 1,
  task: "WF-016",
  binary: {
    pathFingerprint: sha256(BINARY).slice(0, 24),
    sha256: sha256(readFileSync(BINARY)),
  },
  pass: failures.length === 0 && scenarios.every((scenario) => scenario.pass),
  failures,
  scenarios,
  exactArtifactOwnedProcessCount: pids.length,
  privacy: {
    rawNoteTextReturned: false,
    rawAttachmentTextReturned: false,
    rawUriReturned: false,
  },
};
mkdirSync(OUT_DIR, { recursive: true });
await Bun.write(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exitCode = 1;
