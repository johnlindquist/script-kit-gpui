#!/usr/bin/env bun
/**
 * chaos-focus-escape-ladder-probe.ts — L5 (monkey-k-focus) round-13 battery.
 * Permanent regression gate for the cross-surface focus + escape-ladder lens.
 *
 * Routing-path honesty: hidden-window rows exercise the SimulateKey automation
 * mirror (path 3 of flows/escape.md) — simulateGpuiEvent real dispatch fails
 * hidden ("No focused automation window"). The real capture/bubble paths are
 * covered by the --frontmost row (screen-queue gated). Protocol `setFilter`
 * is the typing channel (real keyDown typing never reaches a hidden window).
 *
 * Rows (hidden-window, no SCREEN claim needed):
 *  1. escape-ladder-nesting: builtin (emoji/theme/files, launcher origin)
 *     + ⌘K actions dialog on top → Esc#1 closes ONLY the dialog (surface,
 *     filter, selection, input focus intact) → Esc#2 goes back → Esc#3 bottom.
 *  2. esc-mid-transition-storms: Esc volleys during launcher→builtin and
 *     builtin→builtin transitions; rung precision (exactly one rung/press,
 *     no double-pop, no swallowed Esc, no stuck view).
 *  3. focus-retention-under-churn: filter typed on A → switch to B + filter
 *     back-to-back → text lands in B's input, B owns focus; 5 surface pairs.
 *  4. hide-side rapid reset cycles ×10: full ladder pop per cycle, filter
 *     input focused + empty after each pop, no keystroke eaten, zero new
 *     ERROR logs (vendor frame-lifecycle signatures are RED, chaos-19 flip).
 *  5. go-back-vs-close honesty: surface × origin matrix — protocol/direct
 *     opens must CLOSE the window on Esc ("ESC - closing window ..."),
 *     launcher opens must GO BACK ("ESC - returning to main menu ...").
 *
 * Verdicts: FAIL = product bug; SUSPECT = papercut/by-design note; PASS.
 *
 * Run: SCRIPT_KIT_GPUI_BINARY=target-agent/artifacts/monkey-focus/script-kit-gpui \
 *        bun scripts/agentic/chaos-focus-escape-ladder-probe.ts
 */
import { execSync } from "node:child_process";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  "target-agent/artifacts/monkey-focus/script-kit-gpui";
const EXPLORE = process.env.CHAOS_EXPLORE === "1";
const ONLY = (process.env.CHAOS_ROWS ?? "").split(",").filter(Boolean);
const rowEnabled = (r: string) => ONLY.length === 0 || ONLY.includes(r);

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// receipts + findings
// ---------------------------------------------------------------------------
type Sev = "FAIL" | "SUSPECT" | "NOTE";
const findings: { sev: Sev; row: string; kind: string; detail: Json }[] = [];
const rows: Json[] = [];
function note(sev: Sev, row: string, kind: string, detail: Json) {
  findings.push({ sev, row, kind, detail });
  console.error(`[${sev}] ${row}/${kind}: ${JSON.stringify(detail).slice(0, 260)}`);
}

const d = await Driver.launch({
  sandboxHome: true,
  binary: BINARY,
  sessionName: "monkey-focus-ladder",
});

// ---------------------------------------------------------------------------
// state + elements + log helpers
// ---------------------------------------------------------------------------
async function digest(label: string): Promise<Json> {
  const st: Json = await d.getState({ timeoutMs: 8000 }).catch((e) => ({ __dead: String(e).slice(0, 140) }));
  const el: Json = await d.getElements({}, { timeoutMs: 8000 }).catch(() => null);
  const out = {
    label,
    promptType: st?.promptType ?? null,
    windowVisible: st?.windowVisible ?? null,
    inputValue: st?.inputValue ?? null,
    selectedValue: st?.selectedValue ?? null,
    choiceCount: st?.choiceCount ?? null,
    visibleChoiceCount: st?.visibleChoiceCount ?? null,
    actionsDialogOpen: (st?.actionsDialog as Json | undefined)?.open === true,
    focusedSemanticId: el?.focusedSemanticId ?? null,
    selectedSemanticId: el?.selectedSemanticId ?? null,
    dead: st?.__dead ?? null,
  };
  if (EXPLORE) console.error(`[explore] ${JSON.stringify(out)}`);
  return out;
}

// The 500-entry ring turns over COMPLETELY between sparse harvests: a
// waitForSettle during a live transition polls getState up to ~40× at ~6
// INFO lines each (~240 lines per settle), so occurrence-count dedupe
// collapses (an older identical line rotates out; the new identical line
// then looks already-seen). The only rotation-proof cursor is the entry's
// own rfc3339-ms timestamp (same clock, monotonic): fresh = ts > cursor,
// plus per-key counters for entries sharing the cursor millisecond.
let logTsCursor = "";
const emittedAtCursor = new Map<string, number>();
async function logDelta(filter: { contains?: string; level?: string } = {}): Promise<Json[]> {
  const logs: Json = await d
    .getLogs({ limit: 300, ...filter }, { timeoutMs: 6000 })
    .catch(() => null);
  const entries = ((logs?.entries ?? []) as Json[]);
  const fresh: Json[] = [];
  const batchAtCursor = new Map<string, number>();
  let maxTs = logTsCursor;
  for (const e of entries) {
    const ts = String(e.timestamp ?? "");
    if (ts < logTsCursor) continue;
    const key = `${e.level ?? ""}|${e.target ?? ""}|${String(e.message ?? "")}`;
    if (ts === logTsCursor) {
      const n = (batchAtCursor.get(key) ?? 0) + 1;
      batchAtCursor.set(key, n);
      if (n <= (emittedAtCursor.get(key) ?? 0)) continue;
    }
    fresh.push(e);
    if (ts > maxTs) maxTs = ts;
  }
  if (maxTs > logTsCursor) {
    logTsCursor = maxTs;
    emittedAtCursor.clear();
    for (const e of entries) {
      if (String(e.timestamp ?? "") === maxTs) {
        const key = `${e.level ?? ""}|${e.target ?? ""}|${String(e.message ?? "")}`;
        emittedAtCursor.set(key, (emittedAtCursor.get(key) ?? 0) + 1);
      }
    }
  }
  if (process.env.CHAOS_DEBUG === "1" && filter.contains) {
    console.error(`[debug] logDelta ${JSON.stringify(filter)} entries=${entries.length} fresh=${fresh.length}`);
    for (const e of fresh) console.error(`[debug]   FRESH: ${String(e.message ?? "").slice(0, 90)}`);
  }
  return fresh;
}
const ESC_RE = /ESC -|launch_origin|Resetting to script list/;
// Signature accumulator: the 500-entry ring rotates under the probe's own
// protocol chatter (each getState/getLogs/settle-poll logs ~5 INFO lines), so
// the ESC/RST channels must be harvested after EVERY chattery operation —
// never once per case (an older identical line rotating out between sparse
// harvests makes a genuinely-new identical line look already-seen).
const sigBuffer: string[] = [];
async function harvestSig(): Promise<string[]> {
  // Separate narrow queries per signature family keep each result set small.
  const a = await logDelta({ contains: "ESC -" });
  const b = await logDelta({ contains: "launch_origin" });
  const c = await logDelta({ contains: "Resetting to script list" });
  const fresh = [...a, ...b, ...c].map((e) => String(e.message ?? "")).filter((m) => ESC_RE.test(m));
  sigBuffer.push(...fresh);
  return fresh;
}
/** Read + clear the accumulated signature lines for the current case. */
function takeSig(): string[] {
  return sigBuffer.splice(0, sigBuffer.length);
}
const VENDOR_FRAME_RE = /vendor\/gpui\/src\/window\.rs/;
function isVendorFrameError(msg: string): boolean {
  return VENDOR_FRAME_RE.test(msg) && (msg.includes("window not found") || msg.includes("RefCell already borrowed"));
}
/** Drain every channel the probe later harvests, so stale ring entries can
 *  never be attributed to the current case. */
async function drainLogs(): Promise<void> {
  takeSig();
  await harvestSig();
  takeSig();
  await logDelta({ level: "error" });
}
async function errorDelta(row: string): Promise<void> {
  const errs = (await logDelta({ level: "error" })).filter(
    (e) => String(e.level ?? "").toLowerCase() === "error",
  );
  if (errs.length === 0) return;
  const vendor = errs.filter((e) => isVendorFrameError(String(e.message ?? "")));
  const real = errs.filter((e) => !isVendorFrameError(String(e.message ?? "")));
  if (vendor.length > 0) {
    note("FAIL", row, "vendor-frame-lifecycle-error", {
      count: vendor.length, ledger: "OF-4/OF-6 closed chaos-19 — recurrence is red",
      sample: vendor.slice(-2).map((e) => String(e.message ?? "").slice(0, 140)),
    });
  }
  if (real.length > 0) {
    note("FAIL", row, "new-error-logs", {
      count: real.length,
      sample: real.slice(-3).map((e) => String(e.message ?? "").slice(0, 140)),
    });
  }
}

async function settle(ms = 4000) {
  const r = await d.waitForSettle({ timeoutMs: ms }).catch(() => ({ settled: false, elapsedMs: -1, probes: -1, lastState: null }));
  return r;
}

/** Mirror-path Escape (SimulateKey). Returns post-settle digest + settle ms.
 *  Harvests the signature channels immediately — the press's own log lines
 *  are still ring-resident this soon after the press. */
async function esc(label: string): Promise<{ g: Json; settleMs: number }> {
  d.simulateKey("escape");
  const r = await settle();
  const g = await digest(label);
  await harvestSig();
  return { g, settleMs: r.elapsedMs };
}
async function cmdK(label: string): Promise<Json> {
  d.simulateKey("k", ["cmd"]);
  await settle();
  const g = await digest(label);
  await harvestSig();
  return g;
}

async function resetToMain() {
  for (let i = 0; i < 4; i++) {
    d.simulateKey("escape");
    await sleep(70);
  }
  d.setFilter("");
  await sleep(120);
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await sleep(200);
  await settle();
  await harvestSig(); // chattery — keep the ring window small
}

/** Open a launcher row by filtering + mirror Enter (origin = main_menu). */
async function openFromLauncher(query: string, rowMatch: RegExp, label: string): Promise<Json> {
  await d.setFilterAndWait(query, { timeoutMs: 6000 });
  let matched = "";
  for (let i = 0; i < 30; i++) {
    const st: Json = await d.getState();
    const sv = String(st?.selectedValue ?? "");
    if (rowMatch.test(sv)) { matched = sv; break; }
    await sleep(150);
  }
  await harvestSig(); // the row-wait poll loop is chatty
  if (!matched) {
    note("SUSPECT", label, "launcher-row-not-found", { query, rowMatch: String(rowMatch) });
    return digest(`${label}-row-missing`);
  }
  d.simulateKey("enter");
  await sleep(250);
  await settle();
  const g = await digest(label);
  await harvestSig();
  return g;
}

// ---------------------------------------------------------------------------
// row 1 — escape-ladder nesting (⌘K dialog on top of a builtin)
// ---------------------------------------------------------------------------
// Dedupe registry for findings with one root cause surfaced by several rows.
const reportedFindings = new Set<string>();
function noteOnce(sev: Sev, row: string, kind: string, detail: Json) {
  if (reportedFindings.has(kind)) return;
  reportedFindings.add(kind);
  note(sev, row, kind, detail);
}
const MIRROR_DIVERGENCE_KIND = "mirror-live-divergence:fileSearch-escape";
function reportFileSearchMirrorDivergence(row: string, evidence: Json) {
  noteOnce("FAIL", row, MIRROR_DIVERGENCE_KIND, {
    mirror: "SimulateKey FileSearchView escape arm (src/app_impl/simulate_key_dispatch.rs:969) calls close_and_reset_window unconditionally — no filter-clear rung, no go_back_or_close origin honesty (launcher-origin file search must go BACK, direct-origin must close)",
    live: "src/render_builtins/file_search.rs:646 — portal check → clear_builtin_view_filter rung → go_back_or_close (origin-honest)",
    why: "flows/escape.md requires all three keyboard routing paths to agree; automation probes cannot reproduce the real user ladder for file search",
    ...evidence,
  });
}

// Per-row expected/observed/verdict rollup (chaos-builtin-prompt-surfaces
// pattern): every row ends with a self-judging record.
function finalizeRow(rowId: string, expected: string, observed: string) {
  const fs = findings.filter((f) => f.row === rowId);
  const verdict = fs.some((f) => f.sev === "FAIL") ? "FAIL" : fs.some((f) => f.sev === "SUSPECT") ? "SUSPECT" : "PASS";
  rows.push({ row: rowId, rollup: true, expected, observed, verdict });
}

// Eviction-retry guard (notes-editor-hostile-chaos-probe.ts judgedRow
// pattern): a parallel lane stealing frontmost auto-hides the sandboxed
// window mid-row (close_and_reset) or steals key status. Retry ≤3 with a
// fresh show; commit only an un-evicted attempt's checks. Persistent
// eviction is recorded blocked-by-environment (SUSPECT), never product red.
// A dead app (getState unresponsive) always fails the row.
class Evicted extends Error {}
type Check = (name: string, ok: boolean, detail?: Json) => void;
async function requireFrontmost(tag: string): Promise<void> {
  const st: Json = await d.getState({ timeoutMs: 8000 });
  if (st.windowVisible !== true) throw new Evicted(`${tag}: window auto-hidden mid-row`);
  if (st.isFocused !== true) throw new Evicted(`${tag}: key-window stolen mid-row`);
}
async function judgedRow(name: string, fn: (c: Check) => Promise<void>) {
  const ROW = "6-frontmost-focus";
  for (let attempt = 1; attempt <= 3; attempt++) {
    const local: { name: string; ok: boolean; detail: Json }[] = [];
    const c: Check = (n, ok, detail = {}) => local.push({ name: n, ok, detail });
    try {
      const alive: Json = await d.getState({ timeoutMs: 10000 }); // throws if dead
      if (alive.windowVisible !== true) {
        await d.request({ type: "show" }, { timeoutMs: 8000 });
        await settle();
      }
      await fn(c);
      for (const e of local) {
        rows.push({ row: ROW, sub: name, check: e.name, ok: e.ok, ...e.detail });
        if (!e.ok) note("FAIL", ROW, `${name}:${e.name}`, e.detail);
      }
      rows.push({ row: ROW, sub: name, attempts: attempt, judged: true });
      return;
    } catch (err) {
      if (err instanceof Evicted) {
        rows.push({ row: ROW, sub: name, attempt, evicted: String(err).slice(0, 160) });
        continue;
      }
      note("FAIL", ROW, `${name}-threw`, { error: String(err).slice(0, 240) });
      rows.push({ row: ROW, sub: name, threw: String(err).slice(0, 240) });
      return;
    }
  }
  note("SUSPECT", ROW, `${name}-blocked-by-environment`, {
    note: "evicted off frontmost on every attempt (parallel-lane focus steal); row not judged",
  });
  rows.push({ row: ROW, sub: name, blockedByEnvironment: true });
}

async function rowEscapeLadderNesting() {
  const ROW = "1-escape-ladder-nesting";
  const surfaces = [
    { id: "emoji", query: "emoji", rowMatch: /emoji picker/i, view: "emojiPicker", filter: "cat", inputId: "input:emoji-filter" },
    { id: "theme", query: "theme", rowMatch: /theme/i, view: "themeChooser", filter: "dark", inputId: "input:theme-filter" },
    { id: "files", query: "search files", rowMatch: /search files/i, view: "fileSearch", filter: "agent", inputId: "input:file-search-input" },
  ];
  for (const s of surfaces) {
    await resetToMain();
    await drainLogs();
    const rec: Json = { surface: s.id };
    const opened = await openFromLauncher(s.query, s.rowMatch, `${ROW}:${s.id}-open`);
    rec.openedView = opened.promptType;
    if (opened.promptType !== s.view) {
      note("FAIL", ROW, "surface-did-not-open", { surface: s.id, expected: s.view, got: opened.promptType });
      rows.push({ row: ROW, ...rec });
      continue;
    }
    // type a filter, remember selection
    await d.setFilterAndWait(s.filter);
    const filtered = await digest(`${ROW}:${s.id}-filtered`);
    rec.filter = filtered.inputValue;
    rec.selection = filtered.selectedValue;
    rec.inputFocus = filtered.focusedSemanticId;

    // ⌘K on top
    const dlgOpen = await cmdK(`${ROW}:${s.id}-cmdk`);
    rec.dialogOpened = dlgOpen.actionsDialogOpen;
    if (!dlgOpen.actionsDialogOpen) {
      note("SUSPECT", ROW, "dialog-did-not-open", { surface: s.id });
      // bail out of this surface cleanly
      await esc(`${ROW}:${s.id}-bail`);
      rows.push({ row: ROW, ...rec });
      continue;
    }

    // Esc#1: dialog ONLY
    const e1 = await esc(`${ROW}:${s.id}-esc1`);
    rec.esc1 = {
      view: e1.g.promptType, dialog: e1.g.actionsDialogOpen, filter: e1.g.inputValue,
      selection: e1.g.selectedValue, focus: e1.g.focusedSemanticId, settleMs: e1.settleMs,
    };
    if (e1.g.actionsDialogOpen) note("FAIL", ROW, "esc1-dialog-stuck", { surface: s.id });
    if (e1.g.promptType !== s.view)
      note("FAIL", ROW, "esc1-double-pop", { surface: s.id, expected: s.view, got: e1.g.promptType, why: "Esc#1 closed dialog AND left the surface — skipped a rung" });
    if (e1.g.inputValue !== rec.filter)
      note("FAIL", ROW, "esc1-filter-lost", { surface: s.id, was: rec.filter, got: e1.g.inputValue });
    if (rec.selection != null && e1.g.selectedValue !== rec.selection)
      note("FAIL", ROW, "esc1-selection-lost", { surface: s.id, was: rec.selection, got: e1.g.selectedValue });
    if (s.inputId && e1.g.focusedSemanticId !== s.inputId)
      note("FAIL", ROW, "esc1-focus-not-restored", { surface: s.id, expected: s.inputId, got: e1.g.focusedSemanticId });

    // Esc#2: filter-clear rung first (ladder rung 3), then go-back.
    let step = await esc(`${ROW}:${s.id}-esc2`);
    if (s.id === "files" && step.g.promptType === "none" && rec.filter !== "") {
      // Mirror divergence: the SimulateKey FileSearchView escape arm closed
      // the whole window from a non-empty filter — the live handler clears
      // the filter first, then honors origin via go_back_or_close.
      rec.esc2 = { rung: "mirror-close-no-filter-rung", view: step.g.promptType, settleMs: step.settleMs };
      reportFileSearchMirrorDivergence(ROW, { phase: "ladder-nesting", filterWas: rec.filter, origin: "launcher" });
      const sigE = takeSig();
      rec.goBack = { view: step.g.promptType, sig: sigE.slice(-3) };
      await errorDelta(ROW);
      rows.push({ row: ROW, ...rec });
      continue;
    }
    if (step.g.promptType === s.view && step.g.inputValue === "") {
      // filter rung consumed Esc#2 — the go-back rung is next
      rec.esc2 = { rung: "filter-clear", view: step.g.promptType, settleMs: step.settleMs };
      step = await esc(`${ROW}:${s.id}-esc3`);
    }
    const sig = takeSig();
    rec.goBack = { view: step.g.promptType, sig: sig.slice(-3) };
    if (step.g.promptType !== "none")
      note("FAIL", ROW, "go-back-missed", { surface: s.id, got: step.g.promptType });
    if (!sig.some((m) => m.includes("returning to main menu (opened from main menu)")))
      note("FAIL", ROW, "go-back-signature-missing", { surface: s.id, sig: sig.slice(-4) });

    // Esc bottom on empty menu (hidden: view must stay ScriptList, no errors)
    const eb = await esc(`${ROW}:${s.id}-esc-bottom`);
    rec.bottom = { view: eb.g.promptType, visible: eb.g.windowVisible, settleMs: eb.settleMs };
    if (eb.g.promptType !== "none")
      note("FAIL", ROW, "bottom-rung-disturbed", { surface: s.id, got: eb.g.promptType });

    await errorDelta(ROW);
    rows.push({ row: ROW, ...rec });
  }
  finalizeRow(ROW, "⌘K Esc#1 closes ONLY the dialog (surface/filter/selection/focus intact), filter-clear rung, then go-back with origin-correct signature", "see per-surface receipts");
}

// ---------------------------------------------------------------------------
// row 2 — Esc mid-transition storms + rung precision
// ---------------------------------------------------------------------------
async function rowEscMidTransition() {
  const ROW = "2-esc-mid-transition";
  await resetToMain();
  await drainLogs();

  // 2a. open-while-escaping volley: trigger + immediate Esc, no settle, ×5
  for (let i = 0; i < 5; i++) {
    d.send({ type: "triggerBuiltin", name: "emoji" });
    d.simulateKey("escape"); // races the open transition
  }
  await sleep(200); // input-queue parity beat (OF-8 lesson)
  await settle(6000);
  const v = await digest(`${ROW}-volley-end`);
  const recA: Json = { sub: "2a-open-while-escaping", endView: v.promptType, dialog: v.actionsDialogOpen };
  if (v.dead) note("FAIL", ROW, "app-dead-after-volley", { dead: v.dead });
  if (v.actionsDialogOpen) note("FAIL", ROW, "dialog-stuck-after-volley", {});
  if (v.promptType !== "emojiPicker" && v.promptType !== "none")
    note("FAIL", ROW, "stuck-mid-view", { got: v.promptType });
  // liveness: filter must still land
  d.setFilter("volley-recovery");
  await settle();
  const vr = await digest(`${ROW}-volley-recovery`);
  if (vr.inputValue !== "volley-recovery")
    note("FAIL", ROW, "input-swallowed-after-volley", { got: vr.inputValue });
  d.setFilter("");
  rows.push({ row: ROW, ...recA });

  // 2b. rung precision: exactly one rung per Esc, no double-pop, no swallow
  await resetToMain();
  const opened = await openFromLauncher("emoji", /emoji picker/i, `${ROW}:2b-open`);
  if (opened.promptType === "emojiPicker") {
    await d.setFilterAndWait("cat");
    await cmdK(`${ROW}:2b-cmdk`);
    const seq: Json[] = [];
    // Esc ×3 with settle between each — ladder: dialog → filter-clear → go-back
    for (let i = 1; i <= 3; i++) {
      const before = await digest(`2b-before-esc${i}`);
      const { g: after } = await esc(`2b-esc${i}`);
      const rungs =
        (before.actionsDialogOpen !== after.actionsDialogOpen ? 1 : 0) +
        (before.inputValue !== after.inputValue ? 1 : 0) +
        (before.promptType !== after.promptType ? 1 : 0);
      seq.push({ i, rungs, view: after.promptType, dialog: after.actionsDialogOpen, filter: after.inputValue });
      if (rungs === 0 && i <= 2) note("FAIL", ROW, "esc-swallowed", { esc: i, before: { v: before.promptType, d: before.actionsDialogOpen, f: before.inputValue }, after: { v: after.promptType, d: after.actionsDialogOpen, f: after.inputValue } });
      if (rungs > 1) note("FAIL", ROW, "esc-double-pop", { esc: i, rungs, before: { v: before.promptType, d: before.actionsDialogOpen, f: before.inputValue }, after: { v: after.promptType, d: after.actionsDialogOpen, f: after.inputValue } });
    }
    rows.push({ row: ROW, sub: "2b-rung-precision", seq });
  } else {
    note("SUSPECT", ROW, "2b-open-failed", { got: opened.promptType });
  }

  // 2c. builtin→builtin switch with Esc volley
  await resetToMain();
  d.send({ type: "triggerBuiltin", name: "emoji" });
  d.send({ type: "triggerBuiltin", name: "choose-theme" });
  d.simulateKey("escape");
  d.simulateKey("escape");
  await sleep(200);
  await settle(6000);
  const sw = await digest(`${ROW}-switch-end`);
  const recC: Json = { sub: "2c-builtin-switch-volley", endView: sw.promptType, dialog: sw.actionsDialogOpen };
  if (sw.dead) note("FAIL", ROW, "app-dead-after-switch", { dead: sw.dead });
  if (sw.actionsDialogOpen) note("FAIL", ROW, "dialog-stuck-after-switch", {});
  if (!["themeChooser", "emojiPicker", "none"].includes(String(sw.promptType)))
    note("FAIL", ROW, "stuck-mid-view", { got: sw.promptType });
  rows.push({ row: ROW, ...recC });

  await errorDelta(ROW);
  finalizeRow(ROW, "coherent end-state after open-while-escaping + builtin-switch volleys; exactly one rung advanced per Esc (no swallow, no double-pop)", "see volley/rung-precision receipts");
}

// ---------------------------------------------------------------------------
// row 3 — focus retention under churn (5 surface pairs)
// ---------------------------------------------------------------------------
async function rowFocusRetention() {
  const ROW = "3-focus-retention";
  // Input semantic ids discovered empirically (run 1): fileSearch =
  // input:file-search-input, appLauncher = input:app-filter.
  // Pair 5 switches via Esc (close/reset → ScriptList): triggerBuiltin
  // mainList is a no-op from builtin surfaces (harness truth, run 1).
  const pairs = [
    { from: "scriptList", via: "trigger", to: "emoji", toView: "emojiPicker", toInput: "input:emoji-filter" },
    { from: "emoji", via: "trigger", to: "choose-theme", toView: "themeChooser", toInput: "input:theme-filter" },
    { from: "choose-theme", via: "trigger", to: "files", toView: "fileSearch", toInput: "input:file-search-input" },
    { from: "files", via: "trigger", to: "apps", toView: "appLauncher", toInput: "input:app-filter" },
    { from: "apps", via: "esc", to: "scriptList", toView: "none", toInput: "input:filter" },
  ];
  // ensure starting on scriptList
  await resetToMain();
  await drainLogs();
  for (const p of pairs) {
    const churnText = `zz-${p.to}`;
    // type on the FROM surface, then switch + type back-to-back (no settle)
    d.setFilter(`pre-${p.from}`);
    await sleep(80);
    if (p.via === "esc") {
      d.simulateKey("escape"); // direct-opened apps: close_and_reset → ScriptList
    } else {
      d.send({ type: "triggerBuiltin", name: p.to });
    }
    d.setFilter(churnText); // races the surface switch
    await sleep(200);
    await settle(5000);
    const g = await digest(`${ROW}:${p.from}->${p.to}`);
    const rec: Json = { pair: `${p.from}->${p.to}`, view: g.promptType, input: g.inputValue, focus: g.focusedSemanticId };
    if (g.dead) note("FAIL", ROW, "app-dead", { pair: rec.pair, dead: g.dead });
    if (g.promptType !== p.toView)
      note("FAIL", ROW, "surface-switch-failed", { pair: rec.pair, expected: p.toView, got: g.promptType });
    if (g.inputValue !== churnText)
      note("FAIL", ROW, "typing-inert-or-leaked", { pair: rec.pair, expected: churnText, got: g.inputValue });
    if (p.toInput && g.focusedSemanticId !== p.toInput)
      note("FAIL", ROW, "focus-not-on-new-input", { pair: rec.pair, expected: p.toInput, got: g.focusedSemanticId });
    if (p.toInput == null) rec.discoveredInput = g.focusedSemanticId;
    rows.push({ row: ROW, ...rec });
  }
  await errorDelta(ROW);
  finalizeRow(ROW, "typing during a surface switch lands in the NEW surface's input with that surface's semantic focus (5/5 pairs)", "see per-pair receipts");
}

// ---------------------------------------------------------------------------
// row 4 — hide-side rapid reset cycles ×10 (focus + vendor-error watch)
// ---------------------------------------------------------------------------
async function rowRapidResetCycles() {
  const ROW = "4-rapid-reset-cycles";
  await resetToMain();
  await drainLogs();
  const settles: number[] = [];
  for (let i = 0; i < 10; i++) {
    const text = `cycle-${i}-filter`;
    await d.setFilterAndWait(text, { timeoutMs: 5000 }).catch(() => {});
    const typed = await digest(`${ROW}-typed-${i}`);
    if (typed.inputValue !== text) {
      note("FAIL", ROW, "keystroke-eaten", { cycle: i, expected: text, got: typed.inputValue });
    }
    // full ladder pop: filter-clear rung then bottom rung, rapid
    d.simulateKey("escape");
    d.simulateKey("escape");
    const t0 = performance.now();
    await settle(5000);
    settles.push(Math.round(performance.now() - t0));
    const g = await digest(`${ROW}-popped-${i}`);
    if (g.promptType !== "none")
      note("FAIL", ROW, "pop-left-surface", { cycle: i, got: g.promptType });
    if (g.inputValue !== "")
      note("FAIL", ROW, "filter-not-cleared", { cycle: i, got: g.inputValue });
    if (g.focusedSemanticId !== "input:filter")
      note("FAIL", ROW, "focus-not-on-main-filter", { cycle: i, got: g.focusedSemanticId });
    if (g.actionsDialogOpen)
      note("FAIL", ROW, "leftover-dialog", { cycle: i });
  }
  settles.sort((a, b) => a - b);
  const p50 = settles[Math.floor(settles.length * 0.5)];
  const p95 = settles[Math.min(settles.length - 1, Math.floor(settles.length * 0.95))];
  rows.push({ row: ROW, cycles: settles.length, settleP50: p50, settleP95: p95 });
  await errorDelta(ROW);
  finalizeRow(ROW, "10/10 cycles: typed filter lands, full ladder pop leaves empty input + input:filter focus, no leftover dialog, zero new error logs", "see cycle receipts + settle p50/p95");
}

// ---------------------------------------------------------------------------
// row 5 — go-back vs close-window honesty (surface × origin matrix)
// ---------------------------------------------------------------------------
async function rowGoBackHonesty() {
  const ROW = "5-go-back-honesty";
  type Case = { id: string; surface: string; origin: "direct" | "launcher"; open: () => Promise<Json>; expect: "close" | "back" };
  const cases: Case[] = [
    { id: "emoji-direct", surface: "emojiPicker", origin: "direct", expect: "close", open: async () => { d.send({ type: "triggerBuiltin", name: "emoji" }); await sleep(300); await settle(); return digest("emoji-direct"); } },
    { id: "emoji-launcher", surface: "emojiPicker", origin: "launcher", expect: "back", open: () => openFromLauncher("emoji", /emoji picker/i, "emoji-launcher") },
    { id: "theme-direct", surface: "themeChooser", origin: "direct", expect: "close", open: async () => { d.send({ type: "triggerBuiltin", name: "choose-theme" }); await sleep(300); await settle(); return digest("theme-direct"); } },
    { id: "theme-launcher", surface: "themeChooser", origin: "launcher", expect: "back", open: () => openFromLauncher("theme", /theme/i, "theme-launcher") },
    { id: "files-direct", surface: "fileSearch", origin: "direct", expect: "close", open: async () => { d.send({ type: "triggerBuiltin", name: "files" }); await sleep(400); await settle(); return digest("files-direct"); } },
    { id: "files-launcher", surface: "fileSearch", origin: "launcher", expect: "back", open: () => openFromLauncher("search files", /search files/i, "files-launcher") },
    { id: "apps-direct", surface: "appLauncher", origin: "direct", expect: "close", open: async () => { d.send({ type: "triggerBuiltin", name: "apps" }); await sleep(300); await settle(); return digest("apps-direct"); } },
    { id: "settings-direct", surface: "settings", origin: "direct", expect: "close", open: async () => { d.send({ type: "triggerBuiltin", name: "settings" }); await sleep(300); await settle(); return digest("settings-direct"); } },
    { id: "settings-launcher", surface: "settings", origin: "launcher", expect: "back", open: () => openFromLauncher("script kit settings", /script kit settings/i, "settings-launcher") },
  ];
  for (const c of cases) {
    await resetToMain();
    await drainLogs(); // signature delta is case-local
    const opened = await c.open();
    const rec: Json = { case: c.id, surface: c.surface, origin: c.origin, expect: c.expect, openedView: opened.promptType };
    if (opened.promptType !== c.surface) {
      note("SUSPECT", ROW, "open-failed", { case: c.id, expected: c.surface, got: opened.promptType });
      rec.observed = "open-failed";
      rows.push({ row: ROW, ...rec });
      continue;
    }
    // one Esc (theme/files have a filter-clear rung only when filter non-empty — filter is empty here)
    if (process.env.CHAOS_DEBUG === "1") {
      const pre = await digest(`${ROW}:${c.id}-pre-esc`);
      console.error(`[debug] pre-esc ${c.id}: dialog=${pre.actionsDialogOpen} focus=${pre.focusedSemanticId} input=${JSON.stringify(pre.inputValue)}`);
    }
    await esc(`${ROW}:${c.id}-esc`);
    const sig = takeSig();
    const after = await digest(`${ROW}:${c.id}-after`);
    const observed = sig.some((m) => m.includes("closing window (opened directly"))
      ? "close"
      : sig.some((m) => m.includes("returning to main menu (opened from main menu)"))
        ? "back"
        : after.promptType === "none" ? "unknown-landed-menu" : "no-transition";
    rec.observed = observed;
    rec.afterView = after.promptType;
    rec.sig = sig.slice(-4);
    // File search special case: the SimulateKey mirror has no origin honesty
    // (unconditional close_and_reset_window, no ESC line). The live handler
    // contract is what row 5 measures — report the divergence once.
    if (c.surface === "fileSearch" && observed !== "back" && observed !== "close") {
      reportFileSearchMirrorDivergence(ROW, { case: c.id, origin: c.origin, expected: c.expect, observed, sig: sig.slice(-4) });
      rec.observed = "mirror-close";
      rows.push({ row: ROW, ...rec });
      await errorDelta(ROW);
      continue;
    }
    if (observed !== c.expect) {
      note("FAIL", ROW, "dismiss-honesty-violation", {
        case: c.id, origin: c.origin, expected: c.expect, observed, sig: sig.slice(-4),
        why: c.expect === "close"
          ? "protocol/direct open must CLOSE on Esc (go_back_or_close direct arm) — a go-back lands the user on an unwanted launcher (extra-Escape family)"
          : "launcher open must GO BACK on Esc, not close the window",
      });
    }
    rows.push({ row: ROW, ...rec });
    await errorDelta(ROW);
  }
  finalizeRow(ROW, "direct-opened surfaces CLOSE on Esc; launcher-opened surfaces GO BACK (go_back_or_close origin contract)", "see surface × origin matrix");
}

// ---------------------------------------------------------------------------
// row 6 — frontmost real-focus restoration (SCREEN-claim gated)
// ---------------------------------------------------------------------------
// Real GPUI dispatch fails hidden ("No focused automation window"), so this
// row runs ONLY with CHAOS_FRONTMOST=1 after the lane's SCREEN turn is posted
// (round-13 queue: X → GROK → X2 → K). Verdicts are fail-closed: a focus
// state that cannot be observed is BLOCKED, never PASS.
async function rowFrontmostFocus() {
  const ROW = "6-frontmost-focus";
  if (process.env.CHAOS_FRONTMOST !== "1") {
    rows.push({ row: ROW, skipped: "CHAOS_FRONTMOST != 1 (screen queue: X → GROK → X2 → K)" });
    return;
  }
  await drainLogs();
  const show = async () => {
    await d.request({ type: "show" }, { timeoutMs: 8000 });
    await settle(6000);
  };
  const realKey = async (key: string, modifiers: string[] = [], text?: string) => {
    const ev: Json = { type: "keyDown", key, modifiers };
    if (text) ev.text = text;
    return d.simulateGpuiEvent(ev, { timeoutMs: 6000 });
  };

  // 6a. show → key-window + first-responder receipts
  await judgedRow("6a-show-focus", async (c) => {
    await show();
    const st: Json = await d.getState({ timeoutMs: 8000 });
    if (st.windowVisible !== true) throw new Evicted("show did not make window visible");
    if (st.isFocused !== true) throw new Evicted("window not key after show");
    const g = await digest("6a-show");
    c("window-visible", st.windowVisible === true, { windowVisible: st.windowVisible });
    c("key-window", st.isFocused === true, { isFocused: st.isFocused });
    c("first-responder-filter", g.focusedSemanticId === "input:filter", { got: g.focusedSemanticId });
  });

  // 6b. real typing reaches the filter input. Real char dispatch needs the
  // GPUI focus handle armed by a click on the input (root-typing-lag-benchmark's
  // ensureFilterInputFocus pattern) — semantic focus alone is not enough.
  await judgedRow("6b-real-typing", async (c) => {
    d.setFilter("");
    await settle();
    await requireFrontmost("6b");
    const li: Json = await d.getLayoutInfo({}, { timeoutMs: 6000 }).catch(() => null);
    const inputComp = ((li?.components ?? []) as Json[]).find((x) =>
      `${x?.name ?? ""} ${x?.type ?? ""}`.toLowerCase().includes("input") && x?.bounds,
    );
    c("input-bounds-available", inputComp != null, { hint: "fail-closed: no bounds → row BLOCKED, never PASS" });
    if (!inputComp) return;
    const b = inputComp.bounds as { x: number; y: number; width: number; height: number };
    await d.simulateGpuiClick(b.x + b.width / 2, b.y + b.height / 2);
    await settle();
    for (const ch of ["e", "m", "o"]) await realKey(ch, [], ch);
    await settle();
    const g = await digest("6b-typed");
    c("real-typing-delivered", typeof g.inputValue === "string" && g.inputValue.includes("emo"), { got: g.inputValue, clickFocus: true });
    d.setFilter("");
    await settle();
  });

  // 6c. emoji from launcher (real Enter) → ⌘K (real) → Esc (real) → focus
  //     returns to the emoji filter, filter text + cursor preserved.
  await judgedRow("6c-dialog-focus-restore", async (c) => {
    await requireFrontmost("6c");
    await d.setFilterAndWait("emoji");
    for (let i = 0; i < 30; i++) {
      const st: Json = await d.getState();
      if (/emoji picker/i.test(String(st?.selectedValue ?? ""))) break;
      await sleep(150);
    }
    await realKey("enter");
    await sleep(300);
    await settle();
    const s2 = await digest("6c-emoji");
    c("emoji-opened", s2.promptType === "emojiPicker", { got: s2.promptType });
    if (s2.promptType !== "emojiPicker") return;
    await d.setFilterAndWait("cat");
    const before = await d.getState();
    const diagBefore = before.filterInputDiagnostics ?? null;
    const ck = await realKey("k", ["cmd"]);
    await settle();
    const s3 = await digest("6c-cmdk");
    const esc1 = await realKey("escape");
    await settle();
    const s4 = await digest("6c-esc1");
    const after = await d.getState();
    const diagAfter = after.filterInputDiagnostics ?? null;
    c("dialog-opened", s3.actionsDialogOpen === true, {
      cmdkDispatch: { completed: ck.dispatchCompleted ?? null, scheduled: ck.dispatchScheduled ?? null },
    });
    c("esc-closes-dialog-only", s4.actionsDialogOpen === false && s4.promptType === "emojiPicker", {
      dialog: s4.actionsDialogOpen, view: s4.promptType,
      escDispatch: { completed: esc1.dispatchCompleted ?? null, scheduled: esc1.dispatchScheduled ?? null },
    });
    c("filter-preserved", s4.inputValue === "cat", { got: s4.inputValue });
    c("focus-restored-emoji-input", s4.focusedSemanticId === "input:emoji-filter", { got: s4.focusedSemanticId });
    c("cursor-receipt", true, {
      cursorPreserved: diagBefore && diagAfter ? JSON.stringify(diagBefore) === JSON.stringify(diagAfter) : "unobservable",
    });
  });

  // 6d. escape → hide → show ×3: window hides on bottom rung, focus returns
  //     to the main filter on every show.
  for (let i = 0; i < 3; i++) {
    await judgedRow(`6d-hide-show-${i}`, async (c) => {
      for (let j = 0; j < 4; j++) await realKey("escape");
      await settle();
      const h = await digest(`6d-hidden-${i}`);
      c("escape-hides-window", h.windowVisible === false, { got: h.windowVisible });
      await show();
      const st: Json = await d.getState({ timeoutMs: 8000 });
      if (st.windowVisible !== true) throw new Evicted("reshow did not make window visible");
      if (st.isFocused !== true) throw new Evicted("reshow not key-window");
      const s = await digest(`6d-reshow-${i}`);
      c("reshow-focus-filter", s.focusedSemanticId === "input:filter", { got: s.focusedSemanticId, isFocused: st.isFocused });
      // no leftover overlay eats the next input
      await d.setFilterAndWait(`reshow-${i}`);
      const s5 = await digest(`6d-reshow-typed-${i}`);
      c("reshow-input-lands", s5.inputValue === `reshow-${i}`, { got: s5.inputValue });
      d.setFilter("");
    });
  }

  // 6e. real-path go-back honesty spot-check: direct-opens (files, theme)
  // must CLOSE on real Esc per mark_opened_directly. The live handlers call
  // go_back_or_close — this observes the REAL routing paths (1/2), unlike
  // the hidden mirror rows.
  for (const t of [
    { id: "files", trigger: "files", view: "fileSearch" },
    { id: "theme", trigger: "choose-theme", view: "themeChooser" },
  ]) {
    await judgedRow(`6e-realpath-${t.id}`, async (c) => {
      // reset to launcher via real escapes, then re-show
      for (let j = 0; j < 4; j++) await realKey("escape");
      await settle();
      await show();
      await requireFrontmost("6e");
      await drainLogs();
      d.send({ type: "triggerBuiltin", name: t.trigger });
      await sleep(400);
      await settle();
      const opened = await digest(`6e-${t.id}-open`);
      c("surface-opened", opened.promptType === t.view, { expected: t.view, got: opened.promptType });
      if (opened.promptType !== t.view) return;
      await realKey("escape");
      await settle();
      await harvestSig();
      const sig = takeSig();
      const after = await digest(`6e-${t.id}-after`);
      // Frontmost state classifier is fail-closed on its own: a direct-open
      // close HIDES the window (windowVisible:false on ScriptList), while a
      // go-back keeps it VISIBLE on the launcher. Log signature corroborates.
      const observed = sig.some((m) => m.includes("closing window (opened directly"))
        ? "close"
        : sig.some((m) => m.includes("returning to main menu (opened from main menu)"))
          ? "back"
          : after.promptType === "none" && after.windowVisible === false
            ? "close"
            : after.promptType === "none" && after.windowVisible === true
              ? "back"
              : "no-transition";
      c("direct-open-closes-on-esc", observed === "close", {
        surface: t.id, origin: "direct", expected: "close", observed,
        afterView: after.promptType, windowVisible: after.windowVisible, sig: sig.slice(-4),
        why: "REAL key path: direct-opened surface must CLOSE on Esc (go_back_or_close direct arm); a go-back leaves the launcher visible — user-visible extra-Escape (origin flag clobbered by the open helper)",
      });
    });
  }
  await errorDelta(ROW);
  finalizeRow(ROW, "show⇒key window + input:filter first responder; real typing delivered; ⌘K/Esc close dialog only with focus restored; escape→hide→show focus restored; direct-opens CLOSE on real Esc", "see per-check receipts");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
let crashed = "";
try {
  await sleep(400);
  await settle(6000);
  await drainLogs(); // baseline drain

  if (rowEnabled("1")) await rowEscapeLadderNesting();
  if (rowEnabled("2")) await rowEscMidTransition();
  if (rowEnabled("3")) await rowFocusRetention();
  if (rowEnabled("4")) await rowRapidResetCycles();
  if (rowEnabled("5")) await rowGoBackHonesty();
  if (rowEnabled("6")) await rowFrontmostFocus();
} catch (e) {
  crashed = String(e).slice(0, 240);
} finally {
  // Cleanup gate: hidden window, driver closed, session dir removed.
  for (let i = 0; i < 3; i++) {
    d.simulateKey("escape");
    await sleep(60);
  }
  d.send({ type: "hide" });
  await sleep(200);
  let windowVisible: Json = "unknown";
  try {
    windowVisible = (await d.getState({ timeoutMs: 4000 })).windowVisible;
  } catch {}
  const sessionDir = (d as unknown as { sessionDir?: string }).sessionDir;
  await d.close();
  if (sessionDir) {
    try { execSync(`rm -rf ${JSON.stringify(sessionDir)}`); } catch {}
  }
  const fails = findings.filter((f) => f.sev === "FAIL");
  const suspects = findings.filter((f) => f.sev === "SUSPECT");
  const verdict = crashed ? "FAIL" : fails.length ? "FAIL" : suspects.length ? "SUSPECT" : "PASS";
  console.log(JSON.stringify({ verdict, crashed: crashed || null, windowVisible, failCount: fails.length, suspectCount: suspects.length, findings, rows, binary: BINARY }, null, 2));
  console.error(`[${verdict}] focus-escape-ladder: fails=${fails.length} suspects=${suspects.length} ${crashed ? "CRASH:" + crashed : "alive"}`);
  process.exit(crashed ? 1 : 0);
}
