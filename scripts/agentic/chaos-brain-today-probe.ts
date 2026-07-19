#!/usr/bin/env bun
/**
 * NN=26 Brain + Today runtime chaos battery.
 *
 * All state lives below Driver's sandbox HOME. The probe deliberately mutates
 * the sandbox brain DB/day files while the app is browsing them, but never
 * touches the user's real Brain or Notes stores.
 */
import { Database } from "bun:sqlite";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import { openDayPage, tapMainHotkey } from "./day-page-open-helper";

const root = resolve(import.meta.dir, "../..");
const arg = (name: string, fallback: string) => {
  const i = process.argv.indexOf(name);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
};
const session = arg(
  "--session",
  `chaos-brain-today-${Date.now()}-${process.pid}`,
);
const binary =
  process.env.PROBE_BINARY ??
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(root, "target-agent/artifacts/chaos-brain-today/script-kit-gpui");
const outputDir = resolve(
  arg("--output-dir", join(root, ".test-output", "chaos-brain-today", session)),
);
const receiptPath = join(outputDir, "receipt.json");
mkdirSync(outputDir, { recursive: true });

type Obj = Record<string, any>;
const marker = `brainToday${Date.now()}${process.pid}`;
const receipt: Obj = {
  schemaVersion: 1,
  tool: "chaos-brain-today-probe",
  session,
  binary,
  marker,
  rows: [],
  failures: [],
};
const rows = receipt.rows as Obj[];
const failures = receipt.failures as string[];

function row(name: string, pass: boolean, detail: Obj = {}) {
  rows.push({ name, pass, ...detail });
  if (!pass) failures.push(name);
}
function localDate(offsetDays = 0): string {
  const d = new Date();
  d.setDate(d.getDate() + offsetDays);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate(),
  ).padStart(2, "0")}`;
}
function visibleRows(state: Json): Json[] {
  return Array.isArray(state?.mainWindowPreflight?.visibleResults)
    ? state.mainWindowPreflight.visibleResults
    : [];
}
async function waitUntil(fn: () => Promise<boolean>, timeoutMs = 10_000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    if (await fn()) return true;
    await Bun.sleep(100);
  }
  return false;
}
async function setDay(driver: Driver, text: string) {
  return (await driver.batch(
    [
      { type: "setInput", text },
      {
        type: "waitFor",
        condition: {
          type: "stateMatch",
          state: { promptType: "dayPage", inputValue: text },
        },
      },
    ],
    { timeoutMs: 30_000 },
  )) as Json;
}
async function launcher(driver: Driver) {
  const state = (await driver.getState({ timeoutMs: 8_000 })) as Json;
  if (state.promptType === "dayPage") {
    await tapMainHotkey(driver, session, "hide-day-page");
    await driver.waitForState({ windowVisible: false }, { timeoutMs: 8_000 });
    await tapMainHotkey(driver, session, "show-launcher");
  }
  await driver.waitForState(
    { windowVisible: true, promptType: "none" },
    { timeoutMs: 10_000 },
  );
  await Bun.sleep(350);
}
async function search(driver: Driver, query: string) {
  await driver.setFilterAndWait(query, { timeoutMs: 10_000 });
  let state: Json = {};
  await waitUntil(async () => {
    state = (await driver.getState({ timeoutMs: 8_000 })) as Json;
    return (
      state?.mainWindowPreflight?.rootPassiveFrame?.brain?.refreshing !== true
    );
  });
  return state;
}
async function eyeline(driver: Driver, label: string) {
  const [state, elements, layout] = await Promise.all([
    driver.getState({ timeoutMs: 8_000 }) as Promise<Json>,
    driver.getElements(
      { target: { type: "main" }, limit: 120 },
      { timeoutMs: 8_000 },
    ) as Promise<Json>,
    driver.getLayoutInfo(
      { target: { type: "main" } },
      { timeoutMs: 8_000 },
    ) as Promise<Json>,
  ]);
  const flat: Json[] = [];
  const walk = (value: any) => {
    if (!value || typeof value !== "object") return;
    if (value.semanticId || value.role || value.type) flat.push(value);
    for (const child of Object.values(value)) {
      if (Array.isArray(child)) child.forEach(walk);
      else if (child && typeof child === "object") walk(child);
    }
  };
  walk(elements);
  const separator = flat.find((el) =>
    /sectionHeader|leadingSeparator|leading-separator/i.test(
      `${el.role ?? ""}|${el.kind ?? ""}|${el.semanticId ?? ""}`,
    ),
  );
  return {
    label,
    promptType: state.promptType,
    inputBytes: Buffer.byteLength(String(state.inputValue ?? "")),
    separator: separator
      ? {
          semanticId: separator.semanticId,
          role: separator.role,
          text: separator.text,
        }
      : null,
    firstVisibleRow: visibleRows(state)[0] ?? null,
    layoutAnchors: (layout?.components ?? [])
      .filter((node: Json) =>
        /main-view-(header|input|main)|ScriptList|ListItem\[0\]/.test(
          String(node.name ?? ""),
        ),
      )
      .map((node: Json) => ({ name: node.name, bounds: node.bounds })),
  };
}

const driver = await Driver.launch({
  binary,
  sessionName: session,
  sandboxHome: true,
  defaultTimeoutMs: 10_000,
  readyTimeoutMs: 18_000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

try {
  const home = join(driver.sessionDir, "home");
  const brainDir = join(home, ".scriptkit", "brain");
  const daysDir = join(brainDir, "days");
  const dbPath = join(home, ".scriptkit", "db", "brain.sqlite");
  mkdirSync(daysDir, { recursive: true });
  receipt.sessionDir = driver.sessionDir;
  receipt.sandboxPaths = { brainDir, daysDir, dbPath };

  // Row 1: rapid Today capture storm, including hostile Unicode and a 100 KiB
  // terminal capture. Only the last value should survive autosave.
  await openDayPage(driver, session);
  const hostile = `rtl \u202e${marker}\u202c zwj 👩🏽‍💻 nul-like \\0 combining e\u0301`;
  const captures = [
    `${marker} capture-0`,
    `${marker} capture-1\n${hostile}`,
    `${marker} capture-2\n${"x".repeat(1_024)}`,
    `${marker} capture-final\n${hostile}\n${"Z".repeat(2_048)}`,
  ];
  for (const text of captures) await setDay(driver, text);
  await Bun.sleep(1_200);
  const todayPath = join(daysDir, `${localDate()}.md`);
  const todayDisk = existsSync(todayPath)
    ? readFileSync(todayPath, "utf8")
    : "";
  row("inbox_capture_storm", todayDisk === captures.at(-1), {
    captures: captures.length,
    finalBytes: Buffer.byteLength(captures.at(-1)!),
    diskBytes: Buffer.byteLength(todayDisk),
  });

  // Row 2: recall immediately while another capture replaces the bound day.
  // This is intentionally raced around the locked instant-index edge.
  await launcher(driver);
  const recallPromise = search(driver, `brain: ${marker}`);
  await Bun.sleep(20);
  writeFileSync(todayPath, `${marker} concurrent external capture`);
  const concurrent = await recallPromise;
  row(
    "capture_while_recall",
    visibleRows(concurrent).some((r) => JSON.stringify(r).includes(marker)),
    { visible: visibleRows(concurrent).slice(0, 8) },
  );

  // Row 3: populate Brain Inbox externally while the empty launcher is live,
  // including hostile and huge titles, then churn one row away mid-refresh.
  await driver.setFilterAndWait("zz-no-results");
  await driver.setFilterAndWait("");
  const dbReady = await waitUntil(async () => existsSync(dbPath), 6_000);
  let inboxError: string | null = null;
  let inboxCount = 0;
  let inboxImmediateCount = 0;
  if (dbReady) {
    try {
      const db = new Database(dbPath);
      const insert = db.prepare(
        `INSERT OR REPLACE INTO brain_inbox
         (kind,title,detail,source,source_id,dedupe_hash,created_at,resolved_at)
         VALUES (?,?,?,?,?,?,?,NULL)`,
      );
      for (let i = 0; i < 24; i++) {
        insert.run(
          i % 3 === 0 ? "question" : "commitment",
          `${marker} inbox ${i} ${i === 23 ? hostile + " Q".repeat(20_000) : ""}`,
          `${hostile} detail ${i}`,
          "capture",
          `${marker}-${i}`,
          `${marker}-hash-${i}`,
          Math.floor(Date.now() / 1000) + i,
        );
      }
      db.close();
      await driver.setFilterAndWait("x");
      const refresh = driver.setFilterAndWait("");
      await Bun.sleep(10);
      const churn = new Database(dbPath);
      churn
        .query("DELETE FROM brain_inbox WHERE source_id = ?")
        .run(`${marker}-23`);
      churn.close();
      await refresh;
      await Bun.sleep(1_000);
      const elements = (await driver.getElements(
        { target: { type: "main" }, limit: 180 },
        { timeoutMs: 8_000 },
      )) as Json;
      inboxImmediateCount = (
        JSON.stringify(elements)
          .toLowerCase()
          .match(/brain.?inbox/g) ?? []
      ).length;
      await Bun.sleep(30_500);
      await driver.setFilterAndWait("ttl-refresh");
      await driver.setFilterAndWait("");
      await Bun.sleep(500);
      const postTtlElements = (await driver.getElements(
        { target: { type: "main" }, limit: 180 },
        { timeoutMs: 8_000 },
      )) as Json;
      inboxCount = (
        JSON.stringify(postTtlElements)
          .toLowerCase()
          .match(/brain.?inbox/g) ?? []
      ).length;
    } catch (error) {
      inboxError = String(error);
    }
  }
  row(
    "brain_inbox_live_churn",
    dbReady && inboxError === null && inboxCount > 0,
    {
      dbReady,
      inboxImmediateCount,
      inboxCount,
      documentedSnapshotTtlMs: 30_000,
      inboxError,
    },
  );

  // Row 4: recall while the backing day is replaced and then deleted. The UI
  // may return zero hits, but must settle, remain usable, and recover on write.
  writeFileSync(todayPath, `${marker} churn-v1`);
  const before = await search(driver, `brain: ${marker}`);
  writeFileSync(todayPath, `${marker} churn-v2 hostile ${hostile}`);
  rmSync(todayPath, { force: true });
  const afterDelete = await search(driver, `brain: ${marker} churn-v2`);
  writeFileSync(todayPath, `${marker} churn-v3 recovered`);
  await driver.setFilterAndWait("brain: recovery-nudge");
  const recovered = await search(driver, `brain: ${marker}`);
  row(
    "recall_under_store_write_delete",
    Boolean(before) && Boolean(afterDelete) && Boolean(recovered),
    {
      beforeRows: visibleRows(before).length,
      afterDeleteRows: visibleRows(afterDelete).length,
      recoveredRows: visibleRows(recovered).length,
      finalPromptType: recovered.promptType,
    },
  );

  // Row 5: Today daily-driver boundary navigation and reopen survival.
  const yesterday = localDate(-1);
  const tomorrow = localDate(1);
  const yesterdayText = `${marker} yesterday boundary`;
  const tomorrowText = `${marker} tomorrow boundary`;
  writeFileSync(join(daysDir, `${yesterday}.md`), yesterdayText);
  writeFileSync(join(daysDir, `${tomorrow}.md`), tomorrowText);
  await driver.setFilterAndWait("");
  await openDayPage(driver, `${session}-boundary`);
  await driver.simulateKey("p", ["cmd"]);
  await Bun.sleep(500);
  for (const ch of yesterday) await driver.simulateKey(ch, []);
  await Bun.sleep(300);
  await driver.simulateKey("enter", []);
  const onYesterday = await waitUntil(async () => {
    const s = (await driver.getState()) as Json;
    return s.promptType === "dayPage" && s.inputValue === yesterdayText;
  });
  await driver.simulateKey("escape", []);
  const backToday = await waitUntil(
    async () => ((await driver.getState()) as Json).promptType === "dayPage",
  );
  await launcher(driver);
  await openDayPage(driver, `${session}-reopen`);
  const reopened = (await driver.getState()) as Json;
  row(
    "today_boundary_navigation_reopen",
    onYesterday && backToday && reopened.promptType === "dayPage",
    {
      yesterday,
      tomorrow,
      onYesterday,
      backToday,
      reopenedInputBytes: Buffer.byteLength(String(reopened.inputValue ?? "")),
    },
  );

  // Row 6: hostile clean external edit mid-view must appear without corrupting
  // bytes, then rapid Today -> actions -> Today -> launcher transitions settle.
  const hostileExternal = `${marker} external-mid-view\n${hostile}\n${"Y".repeat(32_768)}`;
  const beforeHostile = (await driver.getState()) as Json;
  if (beforeHostile.inputValue === yesterdayText) {
    await driver.simulateKey("escape", []);
    await waitUntil(async () => {
      const state = (await driver.getState()) as Json;
      return (
        state.promptType === "dayPage" && state.inputValue !== yesterdayText
      );
    });
  }
  await setDay(driver, `${marker} clean-before-external`);
  await Bun.sleep(1_000);
  writeFileSync(todayPath, hostileExternal);
  await driver.simulateKey("arrowDown", []);
  const externalObserved = await waitUntil(async () => {
    const s = (await driver.getState()) as Json;
    return s.inputValue === hostileExternal;
  });
  const todayEye = await eyeline(driver, "today-hostile");
  await driver.simulateKey("k", ["cmd"]);
  await Bun.sleep(150);
  await driver.simulateKey("escape", []);
  await driver.simulateKey("x", []);
  await driver.simulateKey("escape", []);
  await launcher(driver);
  await driver.setFilterAndWait(`brain: ${marker}`);
  const brainEye = await eyeline(driver, "brain-recall");
  const settled = (await driver.getState()) as Json;
  row(
    "today_hostile_edit_and_spine_storm",
    externalObserved && settled.promptType === "none",
    {
      externalObserved,
      finalPromptType: settled.promptType,
      finalWindowVisible: settled.windowVisible,
    },
  );
  receipt.eyeline = { today: todayEye, brain: brainEye };

  receipt.classification = failures.length === 0 ? "pass" : "fail";
} catch (error) {
  receipt.classification = "fail";
  receipt.error =
    error instanceof Error ? (error.stack ?? error.message) : String(error);
  failures.push("probe_threw");
} finally {
  receipt.loadavg = {
    final: Bun.spawnSync(["sysctl", "-n", "vm.loadavg"])
      .stdout.toString()
      .trim(),
  };
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  await driver.close().catch(() => {});
  console.log(
    JSON.stringify(
      { classification: receipt.classification, failures, receiptPath },
      null,
      2,
    ),
  );
}

process.exit(failures.length === 0 ? 0 : 1);
