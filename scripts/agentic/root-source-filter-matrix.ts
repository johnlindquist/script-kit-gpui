#!/usr/bin/env bun
// Transport: persistent event-driven Driver (scripts/devtools/driver.ts) —
// ported from session.sh subprocess send/rpc as the migration template for
// the other matrix scripts. Fixtures, assertions, receipt shape, and the
// AI Vault poison-leak check are unchanged.
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { Driver } from "../devtools/driver";

type Json = Record<string, any>;

const repoRoot = resolve(import.meta.dir, "../..");
// Session defaults to a pid-unique name and namespaces all artifacts
// (sandbox HOME, fixtures, receipts, driver session) under it so parallel
// matrix runs on the same checkout never clobber each other. Pass
// --session for a stable artifact path when a single run is intended.
const session = argValue(
  "--session",
  `root-source-filter-matrix-${process.pid}`,
);
const query = argValue("--query", `codexsource${Date.now()}`);
const timeoutMs = Number(argValue("--timeout", "12000"));
const pollMs = Number(argValue("--poll", "50"));
const recentFilesOnly = process.argv.includes("--recent-files-only");
const outputDir = join(
  repoRoot,
  ".test-output",
  "root-source-filter-matrix",
  session,
);
const homeDir = join(outputDir, "home");
const kitDir = join(homeDir, ".scriptkit");
const dbDir = join(kitDir, "db");
const sessionRoot = join(outputDir, "sessions");
const recentDir = join(outputDir, "recent");
const recentFilePath = join(recentDir, `${query}-recent-file.txt`);
const recentDirectoryPath = join(
  recentDir,
  `${query}-legacy-recent-folder`,
);
const aiVaultPoisonStrings = [
  "POISON_TRANSCRIPT",
  "POISON_PREVIEW",
  "POISON_PROMPT",
  "POISON_ASSISTANT",
  "POISON_RESUME_COMMAND",
];
// AI Vault Codex/Claude SQL coverage lives in root-ai-vault-codex-perf.ts.
// Keep these identifiers here so the source audit ties the general source-head
// matrix to the dedicated local-provider receipt:
// codex-sql-title-match, Claude SQL source filter.

process.env.HOME = homeDir;
process.env.SK_PATH = kitDir;
process.env.SCRIPT_KIT_SESSION_DIR = sessionRoot;
process.env.SCRIPT_KIT_SESSION_READY_TIMEOUT_MS = "10000";
process.env.SCRIPT_KIT_ROOT_FILE_SEARCH_TEST_PROVIDER = JSON.stringify({
  query,
  delayMs: 0,
  results: [
    {
      path: `/tmp/${query}-file-result.txt`,
      name: `${query}-file-result.txt`,
      fileType: "document",
      size: 42,
      modified: Date.now(),
    },
  ],
});
process.env.SCRIPT_KIT_BROWSER_TABS_TEST_PROVIDER = JSON.stringify([
  {
    browser_name: "Google Chrome",
    browser_bundle_id: "com.google.Chrome",
    window_index: 1,
    tab_index: 1,
    title: `${query} browser tab`,
    url: `https://example.com/${query}/tab`,
  },
]);
process.env.SCRIPT_KIT_AI_VAULT_TEST_PROVIDER = JSON.stringify([
  {
    provider: "hermes-agent",
    providerDisplayName: "Hermes Agent",
    sessionId: "vault-source-filter",
    sourceKind: "cli",
    safeTitle: `${query} vault session`,
    workspacePath: `/tmp/${query}-workspace`,
    model: "fixture-model",
    modifiedAt: new Date().toISOString(),
    matchedField: "title",
    stableKey: "ai-vault/hermes-agent/cli/vault-source-filter",
    score: 100,
    transcript: aiVaultPoisonStrings[0],
    preview: aiVaultPoisonStrings[1],
    prompt: aiVaultPoisonStrings[2],
    assistantText: aiVaultPoisonStrings[3],
    resumeCommand: aiVaultPoisonStrings[4],
  },
]);

function argValue(name: string, fallback: string): string {
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
}

function run(command: string, args: string[], options: { input?: string } = {}): string {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    env: process.env,
    input: options.input,
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed\nstdout=${result.stdout}\nstderr=${result.stderr}`,
    );
  }
  return result.stdout;
}

// The driver is launched in main() after fixtures are seeded so the app
// inherits the sandbox HOME/SK_PATH and test-provider env set above.
let driver: Driver;

function send(command: Json): void {
  driver.send(command);
}

async function waitForInput(input: string): Promise<Json> {
  return driver.waitForState(
    { promptType: "none", inputValue: input },
    { timeoutMs, pollIntervalMs: pollMs },
  );
}

async function getState(_tag: string): Promise<Json> {
  return driver.getState({ timeoutMs });
}

function sql(path: string, input: string) {
  run("sqlite3", [path], { input });
}

function seedFixtures() {
  rmSync(outputDir, { recursive: true, force: true });
  mkdirSync(dbDir, { recursive: true });
  mkdirSync(recentDir, { recursive: true });
  mkdirSync(join(kitDir, "plugins", "main", "scripts"), { recursive: true });
  writeFileSync(
    join(kitDir, "config.ts"),
    `export default {
  unifiedSearch: {
    files: { enabled: false, globalSearch: false, recentFiles: false, directoryBrowse: false },
    notes: { enabled: false },
    clipboardHistory: { enabled: false },
    dictationHistory: { enabled: false },
    agent_chatHistory: { enabled: false },
    aiVault: { enabled: false },
    browserTabs: { enabled: false },
    browserHistory: { enabled: false },
  },
};
`,
  );

  writeFileSync(recentFilePath, `${query} recent file body\n`);
  mkdirSync(recentDirectoryPath, { recursive: true });
  writeFileSync(
    join(kitDir, "frecency.json"),
    `${JSON.stringify({
      entries: {
        [`file/${recentDirectoryPath}`]: {
          count: 99,
          last_used: Math.floor(Date.now() / 1000),
        },
        [`dir/${recentDirectoryPath}`]: {
          count: 99,
          last_used: Math.floor(Date.now() / 1000),
        },
        [`file/${recentFilePath}`]: {
          count: 3,
          last_used: Math.floor(Date.now() / 1000) - 1,
        },
      },
    })}\n`,
  );

  const now = new Date().toISOString();
  const noteId = "11111111-1111-4111-8111-111111111111";
  sql(
    join(dbDir, "notes.sqlite"),
    `
CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0
);
CREATE VIRTUAL TABLE notes_fts USING fts5(title, content, content='notes', content_rowid='rowid');
INSERT INTO notes (id, title, content, created_at, updated_at, deleted_at, is_pinned, sort_order)
VALUES ('${noteId}', 'Welcome to Notes', 'Starter content for root source filter search', '${now}', '${now}', NULL, 0, 0);
INSERT INTO notes_fts(rowid, title, content)
SELECT rowid, title, content FROM notes WHERE id = '${noteId}';
`,
  );

  sql(
    join(dbDir, "clipboard-history.sqlite"),
    `
CREATE TABLE history (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  content_hash TEXT,
  content_type TEXT NOT NULL DEFAULT 'text',
  timestamp INTEGER NOT NULL,
  pinned INTEGER DEFAULT 0,
  ocr_text TEXT,
  text_preview TEXT,
  image_width INTEGER,
  image_height INTEGER,
  byte_size INTEGER
);
INSERT INTO history (
  id, content, content_hash, content_type, timestamp, pinned, ocr_text, text_preview, image_width, image_height, byte_size
) VALUES (
  'clip-source-filter', '${query} clipboard text', 'fixture-hash', 'text', ${Date.now()}, 0, NULL, '${query} clipboard text', NULL, NULL, ${query.length + 15}
);
`,
  );

  writeFileSync(
    join(kitDir, "dictation-history.jsonl"),
    `${JSON.stringify({
      id: "dictation-source-filter",
      timestamp: now,
      transcript: `${query} dictation transcript`,
      preview: `${query} dictation transcript`,
      target: "Main Filter",
      audio_duration_ms: 1200,
    })}\n`,
  );

  writeFileSync(
    join(kitDir, "agent_chat-history.jsonl"),
    `${JSON.stringify({
      timestamp: now,
      first_message: `${query} conversation prompt`,
      message_count: 2,
      session_id: "agent_chat-source-filter",
      title: `${query} conversation prompt`,
      preview: `${query} conversation reply`,
      search_text: `${query} conversation prompt ${query} conversation reply`,
    })}\n`,
  );

  const historyDir = join(homeDir, "Library/Application Support/Google/Chrome/Default");
  mkdirSync(historyDir, { recursive: true });
  const chromeTime = (Math.floor(Date.now() / 1000) + 11644473600) * 1000000;
  sql(
    join(historyDir, "History"),
    `
CREATE TABLE urls (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT,
  visit_count INTEGER NOT NULL DEFAULT 0,
  typed_count INTEGER NOT NULL DEFAULT 0,
  last_visit_time INTEGER NOT NULL DEFAULT 0
);
INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time)
VALUES (1, 'https://example.com/${query}/history', '${query} browser history', 7, 2, ${chromeTime});
`,
  );
}

const cases = [
  {
    heads: ["f:", "files:"],
    expectedFilters: ["files"],
    role: "rootFile",
    sourceName: "Files",
    stableKeyIncludes: `${query}-file-result.txt`,
    emptyStableKeyIncludes: `${query}-recent-file.txt`,
  },
  {
    heads: ["n:", "notes:"],
    query: "not",
    expectedFilters: ["notes"],
    role: "rootPassive",
    sourceName: "Notes",
    stableKey: "note/11111111-1111-4111-8111-111111111111",
    emptyStableKey: "note/11111111-1111-4111-8111-111111111111",
  },
  {
    heads: ["c:", "clipboard:"],
    expectedFilters: ["clipboard"],
    role: "rootPassive",
    sourceName: "Clipboard History",
    stableKey: "clipboard-history/clip-source-filter",
    emptyStableKey: "clipboard-history/clip-source-filter",
  },
  {
    heads: ["d:", "dictation:"],
    expectedFilters: ["dictation"],
    role: "rootPassive",
    sourceName: "Dictation History",
    stableKey: "dictation-history/dictation-source-filter",
    emptyStableKey: "dictation-history/dictation-source-filter",
  },
  {
    heads: ["ai:", "conversations:"],
    expectedFilters: ["conversations"],
    role: "rootPassive",
    sourceName: "Agent Chat Conversations",
    stableKey: "agent_chat-history/agent_chat-source-filter",
    emptyStableKey: "agent_chat-history/agent_chat-source-filter",
  },
  {
    heads: ["v:", "vault:"],
    expectedFilters: ["vault"],
    role: "rootPassive",
    sourceName: "AI Vault",
    stableKey: "ai-vault/hermes-agent/cli/vault-source-filter",
    emptyStableKey: "ai-vault/hermes-agent/cli/vault-source-filter",
  },
  {
    heads: ["h:", "history:"],
    expectedFilters: ["history"],
    role: "rootPassive",
    sourceName: "Browser History",
    stableKeyIncludes: "browser-history/",
    emptyStableKeyIncludes: "browser-history/",
  },
  {
    heads: ["t:", "tabs:"],
    expectedFilters: ["tabs"],
    role: "rootPassive",
    sourceName: "Browser Tabs",
    stableKeyIncludes: `browser-tab/com.google.Chrome/1/1/https://example.com/${query}/tab`,
    emptyStableKeyIncludes: `browser-tab/com.google.Chrome/1/1/https://example.com/${query}/tab`,
  },
];

function assertFrame(state: Json, input: string, spec: Json, expectedSearchText: string) {
  const preflight = state.mainWindowPreflight;
  if (!preflight) {
    throw new Error(`${input}: missing mainWindowPreflight in ${JSON.stringify(state)}`);
  }
  if (preflight.computedSearchText !== expectedSearchText) {
    throw new Error(`${input}: expected computedSearchText ${expectedSearchText}, got ${preflight.computedSearchText}`);
  }
  if (JSON.stringify(preflight.sourceFilters) !== JSON.stringify(spec.expectedFilters)) {
    throw new Error(`${input}: expected filters ${JSON.stringify(spec.expectedFilters)}, got ${JSON.stringify(preflight.sourceFilters)}`);
  }
  const visible = preflight.visibleResults ?? [];
  const matches = visible.filter((row: Json) => {
    if (row.role !== spec.role || row.sourceName !== spec.sourceName) {
      return false;
    }
    if (spec.stableKey) {
      return row.stableKey === spec.stableKey;
    }
    return String(row.stableKey ?? "").includes(spec.stableKeyIncludes);
  });
  if (matches.length !== 1) {
    throw new Error(`${input}: expected one ${spec.sourceName} row, got ${JSON.stringify(visible)}`);
  }
  const disallowed = visible.filter((row: Json) => row.role !== spec.role || row.sourceName !== spec.sourceName);
  if (disallowed.length > 0) {
    throw new Error(`${input}: disallowed rows survived ${JSON.stringify(disallowed)}`);
  }
}

function queryFor(spec: Json): string {
  return spec.query ?? query;
}

async function runCase(head: string, spec: Json): Promise<Json> {
  const caseQuery = queryFor(spec);
  const input = `${head} ${caseQuery}`;
  send({ type: "setFilter", text: input, requestId: `source-filter-matrix-set-${Date.now()}` });
  await waitForInput(input);

  let lastFrame: Json | null = null;
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const state = await getState(head.replace(/[^a-z0-9]+/gi, "-"));
    lastFrame = state.mainWindowPreflight;
    try {
      assertFrame(state, input, spec, caseQuery);
      return { input, preflight: state.mainWindowPreflight };
    } catch (error) {
      await Bun.sleep(pollMs);
      if (Date.now() >= deadline) {
        throw error;
      }
    }
  }
  throw new Error(`${input}: timed out, lastFrame=${JSON.stringify(lastFrame)}`);
}

async function runAttachedCase(head: string, spec: Json): Promise<Json> {
  const caseQuery = queryFor(spec);
  const input = `${head}${caseQuery}`;
  send({ type: "setFilter", text: input, requestId: `source-filter-matrix-attached-set-${Date.now()}` });
  await waitForInput(input);

  let lastFrame: Json | null = null;
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const state = await getState(`attached-${head.replace(/[^a-z0-9]+/gi, "-")}`);
    lastFrame = state.mainWindowPreflight;
    try {
      assertFrame(state, input, spec, caseQuery);
      return { input, preflight: state.mainWindowPreflight };
    } catch (error) {
      await Bun.sleep(pollMs);
      if (Date.now() >= deadline) {
        throw error;
      }
    }
  }
  throw new Error(`${input}: timed out, lastFrame=${JSON.stringify(lastFrame)}`);
}

async function runEmptyCase(head: string, spec: Json): Promise<Json> {
  const input = `${head} `;
  send({ type: "setFilter", text: input, requestId: `source-filter-matrix-empty-set-${Date.now()}` });
  await waitForInput(input);

  const emptySpec = {
    ...spec,
    stableKey: spec.emptyStableKey,
    stableKeyIncludes: spec.emptyStableKeyIncludes,
  };
  let lastFrame: Json | null = null;
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const state = await getState(`empty-${head.replace(/[^a-z0-9]+/gi, "-")}`);
    lastFrame = state.mainWindowPreflight;
    try {
      assertFrame(state, input, emptySpec, "");
      return { input, preflight: state.mainWindowPreflight };
    } catch (error) {
      await Bun.sleep(pollMs);
      if (Date.now() >= deadline) {
        throw error;
      }
    }
  }
  throw new Error(`${input}: timed out, lastFrame=${JSON.stringify(lastFrame)}`);
}

async function runRecentFilesProof(): Promise<Json> {
  const input = "files:";
  const expectedStableKey = `file/${recentFilePath}`;
  const legacyDirectoryStableKey = `file/${recentDirectoryPath}`;
  send({
    type: "setFilter",
    text: input,
    requestId: `source-filter-matrix-recent-files-${Date.now()}`,
  });
  await waitForInput(input);

  let lastEvidence: Json | null = null;
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const state = await getState("recent-files-proof");
    const elementsResponse = await driver.getElements(
      { limit: 160, includeHeaders: true },
      { timeoutMs },
    );
    const preflight = state.mainWindowPreflight ?? {};
    const visibleResults = preflight.visibleResults ?? [];
    const elements = elementsResponse.elements ?? [];
    lastEvidence = { state, elements };

    try {
      if (state.inputValue !== input) {
        throw new Error(`expected input ${input}, got ${state.inputValue}`);
      }
      if (preflight.computedSearchText !== "") {
        throw new Error(
          `expected empty computedSearchText, got ${preflight.computedSearchText}`,
        );
      }
      if (JSON.stringify(preflight.sourceFilters) !== JSON.stringify(["files"])) {
        throw new Error(
          `expected Files source filter, got ${JSON.stringify(preflight.sourceFilters)}`,
        );
      }

      const recentRows = visibleResults.filter(
        (row: Json) => row.stableKey === expectedStableKey,
      );
      if (recentRows.length !== 1) {
        throw new Error(
          `expected one recent file row, got ${JSON.stringify(visibleResults)}`,
        );
      }
      if (
        visibleResults.some(
          (row: Json) =>
            row.stableKey === legacyDirectoryStableKey ||
            JSON.stringify(row).includes("legacy-recent-folder"),
        )
      ) {
        throw new Error(
          `legacy recent directory remained visible: ${JSON.stringify(visibleResults)}`,
        );
      }

      const recentHeader = elements.find(
        (element: Json) =>
          element.role === "sectionHeader" && element.text === "Recent Files",
      );
      if (!recentHeader) {
        throw new Error(`missing Recent Files semantic header: ${JSON.stringify(elements)}`);
      }
      const expectedFileElements = elements.filter(
        (element: Json) =>
          element.role === "row" &&
          element.text === `${query}-recent-file.txt` &&
          element.value === recentFilePath &&
          element.kind === "file",
      );
      if (expectedFileElements.length !== 1) {
        throw new Error(
          `expected one truthful recent file semantic row: ${JSON.stringify(elements)}`,
        );
      }
      if (elements.some((element: Json) => JSON.stringify(element).includes(recentDirectoryPath))) {
        throw new Error(
          `semantic elements referenced legacy directory: ${JSON.stringify(elements)}`,
        );
      }

      return {
        input,
        computedSearchText: preflight.computedSearchText,
        sourceFilters: preflight.sourceFilters,
        expectedStableKey,
        recentFileOccurrences: recentRows.length,
        legacyDirectoryStableKey,
        legacyDirectoryVisible: false,
        recentFilesHeaderVisible: true,
        fileElement: expectedFileElements[0],
      };
    } catch (error) {
      if (Date.now() >= deadline) {
        throw error;
      }
      await Bun.sleep(pollMs);
    }
  }

  throw new Error(`recent Files proof timed out: ${JSON.stringify(lastEvidence)}`);
}

async function main() {
  const startedAt = performance.now();
  seedFixtures();
  driver = await Driver.launch({
    sessionName: session,
    sessionDir: join(sessionRoot, session),
    readyTimeoutMs: 10_000,
  });

  const results: Json[] = [];
  let recentFilesProof: Json | null = null;
  let finalState: Json | null = null;
  try {
    await driver.getState({ timeoutMs });
    if (!recentFilesOnly) {
      for (const spec of cases) {
        for (const head of spec.heads) {
          results.push(await runCase(head, spec));
          send({ type: "setFilter", text: "", requestId: `source-filter-matrix-reset-${Date.now()}` });
          await waitForInput("");
          results.push(await runAttachedCase(head, spec));
          send({ type: "setFilter", text: "", requestId: `source-filter-matrix-attached-reset-${Date.now()}` });
          await waitForInput("");
          results.push(await runEmptyCase(head, spec));
          send({ type: "setFilter", text: "", requestId: `source-filter-matrix-empty-reset-${Date.now()}` });
          await waitForInput("");
        }
      }
    }
    recentFilesProof = await runRecentFilesProof();
    await driver.simulateGpuiEvent(
      { type: "keyDown", key: "escape", modifiers: [] },
      { timeoutMs },
    );
    send({
      type: "hide",
      requestId: `source-filter-matrix-cleanup-${Date.now()}`,
    });
    await driver.waitForState(
      { windowVisible: false },
      { timeoutMs, pollIntervalMs: pollMs },
    );
    finalState = await driver.getState({ timeoutMs });
  } finally {
    try {
      if (finalState?.windowVisible !== false) {
        send({
          type: "hide",
          requestId: `source-filter-matrix-finally-cleanup-${Date.now()}`,
        });
        await driver.waitForState(
          { windowVisible: false },
          { timeoutMs, pollIntervalMs: pollMs },
        );
        finalState = await driver.getState({ timeoutMs });
      }
    } finally {
      // Close before reading artifacts so the driver's log writer is flushed.
      await driver.close();
    }
  }

  const logPath = driver.logPath;
  const responsesPath = join(driver.sessionDir, "protocol-responses.ndjson");
  const receipt = {
    schemaVersion: 1,
    status: "pass",
    session,
    transport: "driver",
    durationMs: Math.round(performance.now() - startedAt),
    query,
    homeDir,
    cases: results,
    recentFilesProof,
    finalState,
    logExcerpt: readFileSync(logPath, "utf8").split("\n").slice(-80),
    responsesPath,
  };
  const receiptText = `${JSON.stringify(receipt, null, 2)}\n`;
  const leakHaystack = `${receiptText}\n${readFileSync(logPath, "utf8")}\n${readFileSync(responsesPath, "utf8")}`;
  const leakedPoison = aiVaultPoisonStrings.filter((value) => leakHaystack.includes(value));
  if (leakedPoison.length > 0) {
    throw new Error(`AI Vault receipt leaked poison metadata: ${leakedPoison.join(", ")}`);
  }
  writeFileSync(join(outputDir, "receipt.json"), receiptText);
  process.stdout.write(receiptText);
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
  process.exit(1);
});
