#!/usr/bin/env bun
/**
 * OF-15 AUDIT ONLY — leading separator + floating status coverage.
 * Verifies promptType/surfaceKind before snapping (rejects false ScriptList snaps).
 * includeHeaders:true for sectionHeader rows.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-input/script-kit-gpui");
const OUT_DIR = join(
  process.cwd(),
  process.env.OF15_RECEIPT_DIR ?? ".test-output/of15-leading-separator-audit-v2",
);
mkdirSync(OUT_DIR, { recursive: true });

type RowInfo = {
  semanticId: string;
  role: string | null;
  kind: string | null;
  text: string | null;
  selectable: boolean;
};

function rowsOf(elementsResult: Json): RowInfo[] {
  const elements: Json[] = elementsResult?.elements ?? [];
  return elements
    .filter((e) => {
      if (e.semanticId === "input:filter" || e.semanticId === "list:results") return false;
      if (e.type === "input" || e.type === "list") return false;
      if (e.role === "footer") return false;
      return true;
    })
    .map((e) => ({
      semanticId: String(e.semanticId ?? ""),
      role: (e.role as string) ?? null,
      kind: (e.kind as string) ?? null,
      text: typeof e.text === "string" ? e.text.slice(0, 80) : null,
      selectable: e.selectable === true,
    }));
}

function leadingSeparator(rows: RowInfo[]) {
  if (rows.length === 0) return { present: false, reason: "empty-list", first: null as RowInfo | null, headerCount: 0 };
  const first = rows[0];
  const present =
    first.role === "sectionHeader" ||
    first.kind === "sectionHeader" ||
    /sectionHeader|section:/.test(first.semanticId);
  return {
    present,
    reason: present ? "first-row-sectionHeader" : "first-row-not-sectionHeader",
    first,
    headerCount: rows.filter((r) => r.role === "sectionHeader").length,
  };
}

function floatingSignals(rows: RowInfo[], layout: Json) {
  const hits: string[] = [];
  for (const r of rows) {
    if (/indexing|loading preview|loading…|loading\.\.\.|searching files/i.test(r.text ?? "")) {
      // Status is correct when it is carried by the persistent leading row;
      // only status text outside that shared row is floating chrome.
      if (r.role !== "sectionHeader" && r.kind !== "leadingSeparator") {
        hits.push(`row:${r.semanticId}:${r.text}`);
      }
    }
  }
  for (const c of (layout?.components ?? []) as Json[]) {
    const hay = `${c.name ?? ""} ${c.type ?? ""} ${c.text ?? ""}`;
    if (/indexing|loading.?badge|status.?pill|pill|badge/i.test(hay)) {
      hits.push(`layout:${c.name}:${String(c.text ?? "").slice(0, 40)}`);
    }
  }
  return { textHits: hits.slice(0, 20), firstLooksStatus: false };
}

const table: Json[] = [];
const d = await Driver.launch({
  binary: BINARY,
  sandboxHome: true,
  sessionName: "of15-v2",
  readyTimeoutMs: 20_000,
  defaultTimeoutMs: 10_000,
});

async function recoverMain() {
  for (let i = 0; i < 5; i++) {
    d.simulateKey("escape");
    await d.waitForSettle({ timeoutMs: 1200 }).catch(() => {});
  }
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  d.setFilter("");
  await d.waitForSettle({ timeoutMs: 2000 }).catch(() => {});
}

async function waitPrompt(
  pred: (st: Json) => boolean,
  timeoutMs = 6000,
): Promise<Json> {
  const start = performance.now();
  let st = await d.getState({ timeoutMs: 8000 });
  while (!pred(st) && performance.now() - start < timeoutMs) {
    await d.waitForSettle({ timeoutMs: 500 }).catch(() => {});
    st = await d.getState({ timeoutMs: 8000 });
  }
  return st;
}

async function snap(
  surfaceId: string,
  note: string,
  expect?: { promptType?: string | string[]; notScriptList?: boolean },
) {
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  const state = await d.getState({ timeoutMs: 10_000 });
  const pt = String(state.promptType ?? "");
  const sk = String(state.surfaceContract?.surfaceKind ?? "");
  let openOk = true;
  let openNote = "ok";
  if (expect?.promptType) {
    const allowed = Array.isArray(expect.promptType) ? expect.promptType : [expect.promptType];
    if (!allowed.includes(pt)) {
      openOk = false;
      openNote = `expected promptType in ${JSON.stringify(allowed)} got ${pt}`;
    }
  }
  if (expect?.notScriptList && (pt === "none" || sk === "ScriptList")) {
    openOk = false;
    openNote = `still on main list pt=${pt} sk=${sk}`;
  }

  const elements = await d.getElements(
    { limit: 150, includeHeaders: true },
    { timeoutMs: 10_000 },
  );
  const layout = await d.getLayoutInfo({}, { timeoutMs: 10_000 });
  const rows = rowsOf(elements);
  const lead = leadingSeparator(rows);
  const floating = floatingSignals(rows, layout);

  const receipt = {
    surfaceId,
    note,
    openOk,
    openNote,
    promptType: pt,
    surfaceKind: sk,
    windowVisible: state.windowVisible ?? null,
    inputValue: state.inputValue ?? null,
    visibleChoiceCount: state.visibleChoiceCount ?? null,
    leadingSeparator: lead,
    floatingStatus: floating,
    rowPreview: rows.slice(0, 10),
    rowCount: rows.length,
    layoutNames: ((layout?.components ?? []) as Json[])
      .map((c) => String(c.name ?? ""))
      .slice(0, 30),
  };
  const path = join(OUT_DIR, `${surfaceId.replace(/[^a-zA-Z0-9_-]+/g, "_")}.json`);
  writeFileSync(path, JSON.stringify(receipt, null, 2) + "\n");

  const row = {
    surface: surfaceId,
    openOk,
    openNote,
    promptType: pt,
    surfaceKind: sk,
    leadingSeparator: openOk ? lead.present : null,
    leadingReason: openOk ? lead.reason : "surface-not-opened",
    floatingHits: floating.textHits.length,
    floatingSample: floating.textHits.slice(0, 4),
    firstRowRole: lead.first?.role ?? null,
    firstRowText: lead.first?.text ?? null,
    receipt: path,
    note,
  };
  table.push(row);
  const tag = !openOk ? "OPEN-FAIL" : lead.present ? "LEAD-OK" : "LEAD-MISS";
  console.error(
    `  [${tag}] ${surfaceId} pt=${pt} lead=${lead.present} float=${floating.textHits.length} first=${lead.first?.role}/${lead.first?.text ?? ""}`,
  );
  return receipt;
}

try {
  await d.getState({ timeoutMs: 8000 });
  await recoverMain();

  // Launcher states (valid ScriptList)
  await snap("launcher-empty", "main empty", { promptType: "none" });
  d.setFilter("abc");
  await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
  await snap("launcher-query-abc", "main abc", { promptType: "none" });
  d.setFilter("zzzz-no-match-xx");
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  await snap("launcher-zero-match", "main zero", { promptType: "none" });
  d.setFilter("");
  await d.waitForSettle({ timeoutMs: 2000 }).catch(() => {});

  // Builtins — chaos-builtin trigger names
  const builtins: { id: string; trigger: string; view: string; filter?: string }[] = [
    { id: "file-search", trigger: "files", view: "fileSearch", filter: "abc" },
    { id: "file-search-empty", trigger: "files", view: "fileSearch", filter: "" },
    { id: "clipboard-history", trigger: "clipboardHistory", view: "clipboardHistory" },
    { id: "emoji-picker", trigger: "emoji", view: "emojiPicker" },
    { id: "app-launcher", trigger: "apps", view: "appLauncher" },
    { id: "settings", trigger: "settings", view: "settings" },
    { id: "theme-chooser", trigger: "choose-theme", view: "themeChooser" },
    { id: "dictation-history", trigger: "dictationHistory", view: "dictationHistory" },
  ];

  for (const b of builtins) {
    await recoverMain();
    d.send({ type: "triggerBuiltin", name: b.trigger });
    const st = await waitPrompt((s) => String(s.promptType ?? "") === b.view, 7000);
    if (String(st.promptType ?? "") !== b.view) {
      // launcher fallback: type name + enter top builtin row
      d.setFilter(b.trigger === "choose-theme" ? "theme" : b.trigger);
      await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
      d.simulateKey("enter");
      await waitPrompt((s) => String(s.promptType ?? "") === b.view, 7000);
    }
    if (b.filter !== undefined) {
      d.setFilter(b.filter);
      await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});
    }
    await snap(b.id, `builtin ${b.trigger}`, { promptType: b.view });
  }

  // Brain / notes / day — best-effort triggers
  for (const b of [
    { id: "brain", trigger: "brain", views: ["brain", "brainBrowse", "notes", "none"] },
    { id: "notes-browse", trigger: "notes", views: ["notes", "notesBrowse", "none"] },
  ]) {
    await recoverMain();
    d.send({ type: "triggerBuiltin", name: b.trigger });
    await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
    await snap(b.id, `trigger ${b.trigger}`, { notScriptList: false });
  }

  // Script prompts (may force-show)
  const prompts: { id: string; view: string; msg: Json }[] = [
    {
      id: "prompt-arg",
      view: "arg",
      msg: {
        type: "arg",
        id: "of15-arg",
        placeholder: "Pick",
        choices: [
          { name: "One", value: "1" },
          { name: "Two", value: "2" },
        ],
      },
    },
    {
      id: "prompt-select",
      view: "select",
      msg: {
        type: "select",
        id: "of15-select",
        placeholder: "Select",
        choices: [
          { name: "A", value: "a" },
          { name: "B", value: "b" },
        ],
      },
    },
    {
      id: "prompt-mini",
      view: "mini",
      msg: {
        type: "mini",
        id: "of15-mini",
        placeholder: "Mini",
        choices: [
          { name: "M1", value: "m1" },
          { name: "M2", value: "m2" },
        ],
      },
    },
    {
      id: "prompt-micro",
      view: "micro",
      msg: {
        type: "micro",
        id: "of15-micro",
        placeholder: "Micro",
        choices: [
          { name: "U1", value: "u1" },
          { name: "U2", value: "u2" },
        ],
      },
    },
  ];

  for (const p of prompts) {
    await recoverMain();
    d.send(p.msg);
    await waitPrompt((s) => String(s.promptType ?? "") === p.view, 7000);
    await snap(p.id, `protocol ${p.msg.type}`, { promptType: p.view });
  }

  await recoverMain();
  await snap("launcher-final", "post-audit main", { promptType: "none" });
} catch (e) {
  table.push({ surface: "audit-crash", error: String(e).slice(0, 300) });
  console.error("CRASH", e);
} finally {
  try {
    d.send({ type: "hide" });
  } catch {
    /* ignore */
  }
  await d.close();
}

const opened = table.filter((r) => r.openOk === true);
const summary = {
  finding: "OF-15",
  lane: "L6-monkey-grok-input",
  binary: BINARY,
  outDir: OUT_DIR,
  contract:
    "Leading separator REQUIRED all lists; transient status in/as separator never floating (9bd506f5e)",
  table,
  counts: {
    total: table.length,
    opened: opened.length,
    openFail: table.filter((r) => r.openOk === false).length,
    leadOk: opened.filter((r) => r.leadingSeparator === true).length,
    leadMiss: opened.filter((r) => r.leadingSeparator === false).length,
    floatingAny: opened.filter((r) => Number(r.floatingHits ?? 0) > 0).length,
  },
};
writeFileSync(join(OUT_DIR, "coverage-table.json"), JSON.stringify(summary, null, 2) + "\n");
console.log(JSON.stringify(summary, null, 2));
