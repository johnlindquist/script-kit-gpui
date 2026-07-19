#!/usr/bin/env bun
/**
 * chaos-eyeline-stability-probe.ts — OF-16 Phase A measurement matrix
 * (battery NN=23, lane L5 monkey-k-focus). MEASUREMENT ONLY — no fixes.
 *
 * Contract under measurement (round-14 brief, user intent 2026-07-18):
 *  C1 Universal anchor geometry — leading separator (x, y, height) and
 *     first-row start y IDENTICAL window-relative across all list surfaces.
 *  C2 Per-keystroke zero-shift — 30-keystroke type/delete storm: header,
 *     input, separator, footer drift 0px between consecutive keystrokes.
 *  C3 State-transition zero-shift — empty→loading→results→zero-match→results
 *     keeps separator + first-row y fixed; status renders IN the separator.
 *  C4 Cross-surface eyeline — cycling launcher→builtin→back lands on the
 *     same separator/first-row y every time.
 *
 * Anchor derivation (fail-closed; missing = violator, never fabricated):
 *  - header/input/footer/list-region bounds come from getLayoutInfo named
 *    components (MainViewHeader / MainViewInput / MainViewFooter / the
 *    topmost List-typed component).
 *  - separator: the leading sectionHeader row is the FIRST row of the list,
 *    so separator.y == list-region top. Presence/text comes from getElements
 *    (role/kind sectionHeader|leadingSeparator per OF-15 plumbing); a surface
 *    missing the leading separator is recorded as a violator, not skipped.
 *  - firstRowY/x from the `ListItem[N]` paint components (exposed on every
 *    full-width list surface); the split attachment-portal family
 *    (dictation/notes browse) exposes none — recorded as a measurability gap.
 *
 * Discipline: hidden-window (no SCREEN claim), sandboxHome, session dir
 * removed, window left hidden. Protocol prompts (select) may force-show —
 * detected, immediately re-hidden, and classified. judgedRow-style eviction
 * guard: a parallel lane's frontmost steal can reset this driver's view
 * mid-row (L3's lesson) — rows retry ≤3, persistent eviction is
 * blocked-by-environment, never a product violator.
 *
 * Receipts: .test-output/chaos-23-eyeline/ (matrix.json + per-state snaps).
 * Run: SCRIPT_KIT_GPUI_BINARY=target-agent/artifacts/monkey-focus/script-kit-gpui \
 *        bun scripts/agentic/chaos-eyeline-stability-probe.ts
 */
import { execSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  "target-agent/artifacts/monkey-focus/script-kit-gpui";
const OUT_DIR = join(process.cwd(), process.env.EYELINE_RECEIPT_DIR ?? ".test-output/chaos-23-eyeline");
mkdirSync(OUT_DIR, { recursive: true });
const EXPLORE = process.env.CHAOS_EXPLORE === "1";

// Drift tolerance: the contract says 0px. Float noise at the half-pixel
// boundary is recorded raw; violators are reported at >DRIFT_EPS with exact
// numbers (Phase B/D owns the final budget).
const DRIFT_EPS = 0.5;

type Bounds = { x: number; y: number; width: number; height: number };
type Anchors = {
  header?: Bounds;
  input?: Bounds;
  footer?: Bounds;
  listRegion?: Bounds;
  separator?: { y: number; x: number; height: number | null; present: boolean; text: string | null; source: string };
  firstRow?: Bounds | null;
  firstRowY: number | null;
  anchorGaps: string[];
};

const d = await Driver.launch({
  sandboxHome: true,
  binary: BINARY,
  sessionName: "monkey-focus-eyeline",
  readyTimeoutMs: 20_000,
  defaultTimeoutMs: 10_000,
});
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// capture plumbing
// ---------------------------------------------------------------------------
function boundsOf(c: Json): Bounds | null {
  const b = (c?.visibleBounds ?? c?.visible_bounds ?? c?.bounds) as Bounds | undefined;
  if (!b || ![b.x, b.y, b.width, b.height].every(Number.isFinite)) return null;
  return b;
}

function layoutComponents(info: Json): { name: string; type: string; bounds: Bounds }[] {
  return ((info?.components ?? []) as Json[])
    .map((c) => ({ name: String(c?.name ?? ""), type: String(c?.type ?? ""), bounds: boundsOf(c) }))
    .filter((c): c is { name: string; type: string; bounds: Bounds } => c.bounds != null);
}

function findComp(comps: { name: string; type: string; bounds: Bounds }[], name: string): Bounds | undefined {
  return comps.find((c) => c.name === name)?.bounds;
}

/** Topmost List-typed component = the list region (separator's parent). */
function findListRegion(comps: { name: string; type: string; bounds: Bounds }[]): { name: string; bounds: Bounds } | null {
  const lists = comps.filter((c) => c.type.toLowerCase() === "list");
  if (lists.length === 0) return null;
  lists.sort((a, b) => a.bounds.y - b.bounds.y);
  return { name: lists[0].name, bounds: lists[0].bounds };
}

function leadingSeparatorRow(elements: Json): { present: boolean; text: string | null; semanticId: string | null } {
  const els: Json[] = (elements?.elements ?? []) as Json[];
  const rows = els.filter((e) => {
    if (e.type === "input" || e.type === "list") return false;
    if (e.role === "footer") return false;
    return true;
  });
  if (rows.length === 0) return { present: false, text: null, semanticId: null };
  const first = rows[0];
  const hay = `${first.role ?? ""} ${first.kind ?? ""} ${first.semanticId ?? ""}`;
  const present = /sectionHeader|leadingSeparator|section:/i.test(hay);
  return { present, text: typeof first.text === "string" ? first.text.slice(0, 80) : null, semanticId: String(first.semanticId ?? "") };
}

async function captureAnchors(label: string): Promise<{ label: string; anchors: Anchors; raw: Json }> {
  const info: Json = await d.getLayoutInfo({}, { timeoutMs: 10_000 });
  const elements: Json = await d.getElements({ limit: 60, includeHeaders: true }, { timeoutMs: 10_000 });
  const comps = layoutComponents(info);
  const gaps: string[] = [];
  const header = findComp(comps, "MainViewHeader");
  const input = findComp(comps, "MainViewInput");
  const footer = findComp(comps, "MainViewFooter");
  const list = findListRegion(comps);
  if (!header) gaps.push("header-bounds-missing");
  if (!input) gaps.push("input-bounds-missing");
  if (!footer) gaps.push("footer-bounds-missing");
  if (!list) gaps.push("list-region-bounds-missing");
  const sep = leadingSeparatorRow(elements);
  if (!sep.present) gaps.push("leading-separator-missing");
  // ListItem[N] paint components expose real first-row bounds on every
  // full-width list surface (verified empirically); the split attachment-
  // portal family (dictation/notes browse) exposes none — recorded as a gap.
  const items = comps
    .filter((c) => /^ListItem\[\d+\]$/.test(c.name))
    .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
  const firstRow = items[0]?.bounds ?? null;
  if (!firstRow) gaps.push("first-row-bounds-missing(no ListItem components)");
  const listTop = list?.bounds.y ?? null;
  const anchors: Anchors = {
    header,
    input,
    footer,
    listRegion: list?.bounds,
    separator: listTop != null
      ? { y: listTop, x: list!.bounds.x, height: null, present: sep.present, text: sep.text, source: list!.name }
      : { y: -1, x: -1, height: null, present: sep.present, text: sep.text, source: "none" },
    firstRow,
    firstRowY: firstRow?.y ?? null,
    anchorGaps: gaps,
  };
  if (EXPLORE) console.error(`[explore] ${label}: ${JSON.stringify(anchors).slice(0, 400)}`);
  return { label, anchors, raw: { components: comps.map((c) => ({ name: c.name, type: c.type, bounds: c.bounds })), separatorRow: sep } };
}

const drift = (a: Bounds, b: Bounds) =>
  Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y), Math.abs(a.width - b.width), Math.abs(a.height - b.height));

// ---------------------------------------------------------------------------
// judgedRow-style eviction guard (parallel-lane frontmost steal resets the
// driver's view mid-row; retry ≤3, persistent = blocked-by-environment)
// ---------------------------------------------------------------------------
class Evicted extends Error {}
const snaps: Record<string, Json> = {};
const violators: Json[] = [];
function violator(dim: string, surface: string, detail: Json) {
  violators.push({ dim, surface, ...detail });
  console.error(`[VIOLATOR] ${dim} ${surface}: ${JSON.stringify(detail).slice(0, 220)}`);
}
/** Assert the driver's view inside a row — a mismatch means eviction. */
async function requireView(view: string, tag: string) {
  const st: Json = await d.getState({ timeoutMs: 10_000 });
  if (String(st.promptType ?? "none") !== view) {
    throw new Evicted(`${tag}: view reset mid-row: expected ${view}, got ${st.promptType}`);
  }
}
async function judgedRow(name: string, fn: () => Promise<void>) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await d.getState({ timeoutMs: 10_000 }); // liveness — throws if dead
      await fn();
      snaps[`${name}__attempts`] = { count: attempt };
      return;
    } catch (e) {
      if (e instanceof Evicted) {
        snaps[`${name}__evicted_${attempt}`] = { error: String(e).slice(0, 160) };
        continue;
      }
      throw e;
    }
  }
  snaps[`${name}__blocked_by_environment`] = { note: "view reset on every attempt (parallel-lane steal); row not judged" };
}

async function settle(ms = 4000) {
  return d.waitForSettle({ timeoutMs: ms }).catch(() => ({ settled: false, elapsedMs: -1, probes: -1, lastState: null }));
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
}
async function openBuiltin(trigger: string, view: string): Promise<boolean> {
  d.send({ type: "triggerBuiltin", name: trigger });
  const start = performance.now();
  while (performance.now() - start < 7000) {
    const st: Json = await d.getState({ timeoutMs: 8000 });
    if (String(st.promptType ?? "") === view) return true;
    await settle(600);
  }
  return false;
}

// ---------------------------------------------------------------------------
// surface/state matrix definition
// ---------------------------------------------------------------------------
type Cell = { surface: string; state: string; run: () => Promise<void> };
const MATRIX: { id: string; view: string; label: string; cells: Cell[] }[] = [
  {
    id: "launcher", view: "none", label: "main menu (ScriptList)",
    cells: [
      { surface: "launcher", state: "empty", run: async () => { d.setFilter(""); await settle(); } },
      { surface: "launcher", state: "query", run: async () => { d.setFilter("abc"); await settle(); } },
      { surface: "launcher", state: "zero-match", run: async () => { d.setFilter("zzqq0011-nomatch"); await settle(); } },
      // NN26-F2 (round-47 manager add): root brain: passive recall — first
      // item reported at y=58 with no semantic leading separator.
      { surface: "brain-passive", state: "brain-head", run: async () => { d.setFilter("brain:"); await settle(); } },
      { surface: "brain-passive", state: "passive-recall", run: async () => { d.setFilter("brain: focus"); await settle(); } },
    ],
  },
  { id: "file-search", view: "fileSearch", label: "file search", cells: [] }, // custom: results/empty/loading/zero-match
  { id: "clipboard", view: "clipboardHistory", label: "clipboard history", cells: [
      { surface: "clipboard", state: "results", run: async () => { d.setFilter("a"); await settle(); } },
      { surface: "clipboard", state: "zero-match", run: async () => { d.setFilter("zzqq0011-nomatch"); await settle(); } },
    ] },
  { id: "emoji", view: "emojiPicker", label: "emoji picker", cells: [
      { surface: "emoji", state: "results", run: async () => { d.setFilter("cat"); await settle(); } },
      { surface: "emoji", state: "zero-match", run: async () => { d.setFilter("zzqq0011-nomatch"); await settle(); } },
    ] },
  { id: "apps", view: "appLauncher", label: "app launcher", cells: [
      { surface: "apps", state: "results", run: async () => { d.setFilter("a"); await settle(); } },
      { surface: "apps", state: "zero-match", run: async () => { d.setFilter("zzqq0011-nomatch"); await settle(); } },
    ] },
  { id: "settings", view: "settings", label: "settings", cells: [
      { surface: "settings", state: "results", run: async () => { d.setFilter("a"); await settle(); } },
      { surface: "settings", state: "zero-match", run: async () => { d.setFilter("zzqq0011-nomatch"); await settle(); } },
    ] },
  { id: "theme", view: "themeChooser", label: "theme chooser", cells: [
      { surface: "theme", state: "results", run: async () => { d.setFilter("dark"); await settle(); } },
      { surface: "theme", state: "zero-match", run: async () => { d.setFilter("zzqq0011-nomatch"); await settle(); } },
    ] },
  { id: "dictation", view: "dictationHistory", label: "dictation history", cells: [
      { surface: "dictation", state: "results", run: async () => { d.setFilter("a"); await settle(); } },
      { surface: "dictation", state: "zero-match", run: async () => { d.setFilter("zzqq0011-nomatch"); await settle(); } },
    ] },
];
const TRIGGERS: Record<string, string> = {
  "file-search": "files", clipboard: "clipboardHistory", emoji: "emoji",
  apps: "apps", settings: "settings", theme: "choose-theme", dictation: "dictationHistory",
};

const matrix: Record<string, { label?: string; anchors: Anchors; gaps: string[] }> = {};
let crashed = "";

try {
  const loadavg = execSync("sysctl -n vm.loadavg").toString().trim();
  snaps["loadavg-start"] = { loadavg };
  await sleep(400);
  await settle(6000);

  // ── C1: surface × state anchor matrix ────────────────────────────────────
  for (const surf of MATRIX) {
    await resetToMain();
    if (surf.id !== "launcher") {
      const opened = await openBuiltin(TRIGGERS[surf.id], surf.view);
      if (!opened) {
        matrix[`${surf.id}:open`] = { anchors: { anchorGaps: ["open-failed"], firstRowY: null }, gaps: ["open-failed"] };
        violator("C1", surf.id, { issue: "surface-open-failed", view: surf.view });
        continue;
      }
    }
    if (surf.id === "file-search") {
      // loading: snap immediately after trigger (async stream may still be empty)
      await judgedRow("c1-file-search-loading", async () => {
        await requireView("fileSearch", "c1-file-search-loading");
        const cap = await captureAnchors("file-search:loading");
        matrix["file-search:loading"] = { anchors: cap.anchors, gaps: cap.anchors.anchorGaps };
        writeFileSync(join(OUT_DIR, "file-search_loading.json"), JSON.stringify(cap.raw, null, 2));
      });
      await settle(6000);
      for (const cell of [
        { surface: "file-search", state: "results", filter: "agent" },
        { surface: "file-search", state: "zero-match", filter: "zzqq0011-nomatch" },
        { surface: "file-search", state: "empty", filter: "" },
      ]) {
        await judgedRow(`c1-${cell.surface}-${cell.state}`, async () => {
          await requireView("fileSearch", `c1-${cell.surface}-${cell.state}`);
          d.setFilter(cell.filter);
          await settle();
          const cap = await captureAnchors(`${cell.surface}:${cell.state}`);
          matrix[`${cell.surface}:${cell.state}`] = { anchors: cap.anchors, gaps: cap.anchors.anchorGaps };
          writeFileSync(join(OUT_DIR, `${cell.surface}_${cell.state}.json`), JSON.stringify(cap.raw, null, 2));
        });
      }
      continue;
    }
    for (const cell of surf.cells) {
      await judgedRow(`c1-${cell.surface}-${cell.state}`, async () => {
        await requireView(surf.view, `c1-${cell.surface}-${cell.state}`);
        await cell.run();
        const cap = await captureAnchors(`${cell.surface}:${cell.state}`);
        matrix[`${cell.surface}:${cell.state}`] = { anchors: cap.anchors, gaps: cap.anchors.anchorGaps };
        writeFileSync(join(OUT_DIR, `${cell.surface}_${cell.state}.json`), JSON.stringify(cap.raw, null, 2));
      });
    }
  }

  // select prompt (protocol; may force-show — detect + re-hide immediately)
  {
    await resetToMain();
    d.send({ type: "select", id: "eyeline-select", placeholder: "Select", choices: [{ name: "A", value: "a" }, { name: "B", value: "b" }] });
    const start = performance.now();
    let opened = false;
    while (performance.now() - start < 7000 && !opened) {
      const st: Json = await d.getState({ timeoutMs: 8000 });
      opened = String(st.promptType ?? "") === "select";
      if (!opened) await settle(600);
    }
    const vis = (await d.getState({ timeoutMs: 8000 })).windowVisible;
    if (vis === true) {
      snaps["select-force-show"] = { note: "protocol select forced the window visible — re-hidden; observability note" };
      d.send({ type: "hide" });
      await settle();
    }
    if (opened) {
      await judgedRow("c1-select-results", async () => {
        await requireView("select", "c1-select-results");
        const cap = await captureAnchors("select:results");
        matrix["select:results"] = { anchors: cap.anchors, gaps: cap.anchors.anchorGaps };
        writeFileSync(join(OUT_DIR, "select_results.json"), JSON.stringify(cap.raw, null, 2));
      });
    } else {
      matrix["select:open"] = { anchors: { anchorGaps: ["open-failed"], firstRowY: null }, gaps: ["open-failed"] };
    }
    await resetToMain();
  }

  // ── C2: 30-keystroke storms (type 15 + delete 15), anchors per keystroke ──
  const stormResults: Record<string, Json> = {};
  const stormSurfs = [
    { id: "launcher", view: "none", trigger: null },
    { id: "file-search", view: "fileSearch", trigger: "files" },
    { id: "emoji", view: "emojiPicker", trigger: "emoji" },
    { id: "theme", view: "themeChooser", trigger: "choose-theme" },
    { id: "clipboard", view: "clipboardHistory", trigger: "clipboardHistory" },
  ];
  for (const s of stormSurfs) {
    await resetToMain();
    if (s.trigger) {
      const opened = await openBuiltin(s.trigger, s.view);
      if (!opened) { stormResults[s.id] = { openFailed: true }; violator("C2", s.id, { issue: "surface-open-failed" }); continue; }
    }
    await judgedRow(`c2-storm-${s.id}`, async () => {
      await requireView(s.view, `c2-storm-${s.id}`);
      const seq = "abcdefghijklmno".split("");
      let acc = "";
      const keys: string[] = [];
      for (const ch of seq) { acc += ch; keys.push(acc); }
      for (let i = acc.length - 1; i >= 0; i--) keys.push(acc.slice(0, i));
      // 30 keystrokes: 15 type + 15 delete
      let prev: Anchors | null = null;
      const drifts: Json[] = [];
      for (let k = 0; k < keys.length; k++) {
        d.setFilter(keys[k]);
        // wait for the echo so the capture reflects THIS keystroke's frame
        await d.waitForState({ inputValue: keys[k] }, { timeoutMs: 4000 }).catch(() => {});
        const cap = await captureAnchors(`${s.id}:storm:${k}`);
        const a = cap.anchors;
        if (prev) {
          const per: Json = { k, filter: keys[k] };
          for (const anchorName of ["header", "input", "footer", "listRegion"] as const) {
            const pb = prev[anchorName], cb = a[anchorName];
            if (pb && cb) {
              const dd = drift(pb, cb);
              per[anchorName] = Number(dd.toFixed(3));
              if (dd > DRIFT_EPS) violator("C2", s.id, { anchor: anchorName, keystroke: k, filter: keys[k], driftPx: Number(dd.toFixed(3)) });
            }
          }
          if (prev.firstRow && a.firstRow) {
            const dd = drift(prev.firstRow, a.firstRow);
            per.firstRow = Number(dd.toFixed(3));
            if (dd > DRIFT_EPS) violator("C2", s.id, { anchor: "firstRow", keystroke: k, filter: keys[k], driftPx: Number(dd.toFixed(3)) });
          }
          const sepPrev = prev.separator?.y ?? null;
          const sepCur = a.separator?.y ?? null;
          if (sepPrev != null && sepCur != null && sepPrev >= 0 && sepCur >= 0) {
            const dd = Math.abs(sepCur - sepPrev);
            per.separatorY = Number(dd.toFixed(3));
            if (dd > DRIFT_EPS) violator("C2", s.id, { anchor: "separatorY", keystroke: k, filter: keys[k], driftPx: Number(dd.toFixed(3)) });
          }
          drifts.push(per);
        }
        prev = a;
      }
      const maxOf = (name: string) => Math.max(0, ...drifts.map((r) => Number(r[name] ?? 0)));
      stormResults[s.id] = {
        keystrokes: keys.length,
        maxDriftPx: { header: maxOf("header"), input: maxOf("input"), footer: maxOf("footer"), listRegion: maxOf("listRegion"), separatorY: maxOf("separatorY"), firstRow: maxOf("firstRow") },
        overEps: drifts.filter((r) => ["header", "input", "footer", "listRegion", "separatorY", "firstRow"].some((n) => Number(r[n] ?? 0) > DRIFT_EPS)).length,
        detail: drifts,
      };
    });
  }
  writeFileSync(join(OUT_DIR, "c2-storm-drift.json"), JSON.stringify(stormResults, null, 2));

  // ── C3: state-transition zero-shift (file search loading is the async case) ──
  const c3: Record<string, Json> = {};
  {
    await resetToMain();
    await judgedRow("c3-file-search-transitions", async () => {
      const frames: Json[] = [];
      d.send({ type: "triggerBuiltin", name: "files" });
      // sample frames during async load (loading → results)
      for (let i = 0; i < 8; i++) {
        const cap = await captureAnchors(`c3:load:${i}`);
        const st: Json = await d.getState({ timeoutMs: 8000 });
        frames.push({ i, promptType: st.promptType, visibleChoiceCount: st.visibleChoiceCount, sepY: cap.anchors.separator?.y, sepPresent: cap.anchors.separator?.present, sepText: cap.anchors.separator?.text, gaps: cap.anchors.anchorGaps });
        if (String(st.promptType ?? "") === "fileSearch" && Number(st.visibleChoiceCount ?? 0) > 0) break;
        await settle(400);
      }
      await requireView("fileSearch", "c3-file-search-transitions");
      // results → zero-match → results
      for (const [label, f] of [["zero-match", "zzqq0011-nomatch"], ["results-again", "agent"]] as const) {
        d.setFilter(f);
        await settle();
        const cap = await captureAnchors(`c3:${label}`);
        frames.push({ label, sepY: cap.anchors.separator?.y, sepPresent: cap.anchors.separator?.present, gaps: cap.anchors.anchorGaps });
      }
      const ys = frames.map((f) => Number(f.sepY ?? -1)).filter((y) => y >= 0);
      const spread = ys.length ? Math.max(...ys) - Math.min(...ys) : null;
      c3["file-search"] = { frames, sepYSpreadPx: spread };
      if (spread != null && spread > DRIFT_EPS) violator("C3", "file-search", { anchor: "separatorY", spreadPx: Number(spread.toFixed(3)), frames: frames.length });
      const missingSep = frames.filter((f) => f.sepPresent === false).length;
      if (missingSep > 0) violator("C3", "file-search", { anchor: "separator-presence", framesMissingSeparator: missingSep, of: frames.length });
    });
    await resetToMain();
  }

  // ── C4: cross-surface cycling (launcher → builtin → back, ×2) ────────────
  const c4: Record<string, Json> = {};
  {
    const cycleSurfs = [
      { id: "file-search", trigger: "files", view: "fileSearch" },
      { id: "emoji", trigger: "emoji", view: "emojiPicker" },
      { id: "theme", trigger: "choose-theme", view: "themeChooser" },
      { id: "settings", trigger: "settings", view: "settings" },
    ];
    for (let round = 0; round < 2; round++) {
      for (const s of cycleSurfs) {
        await resetToMain();
        await judgedRow(`c4-${s.id}-r${round}`, async () => {
          const opened = await openBuiltin(s.trigger, s.view);
          if (!opened) throw new Evicted("open failed");
          await requireView(s.view, `c4-${s.id}-r${round}`);
          const cap = await captureAnchors(`c4:${s.id}:r${round}`);
          const back = async () => { for (let i = 0; i < 3; i++) { d.simulateKey("escape"); await sleep(70); } await settle(); };
          await back();
          const backCap = await captureAnchors(`c4:back:r${round}`);
          c4[`${s.id}:r${round}`] = {
            sepY: cap.anchors.separator?.y, sepX: cap.anchors.separator?.x, listRegion: cap.anchors.listRegion,
            firstRow: cap.anchors.firstRow,
            backSepY: backCap.anchors.separator?.y, gaps: cap.anchors.anchorGaps,
          };
        });
      }
    }
    writeFileSync(join(OUT_DIR, "c4-cycling.json"), JSON.stringify(c4, null, 2));
    // cross-round consistency per surface
    for (const s of cycleSurfs) {
      const a = c4[`${s.id}:r0`], b = c4[`${s.id}:r1`];
      if (a && b && typeof a.sepY === "number" && typeof b.sepY === "number" && Math.abs(a.sepY - b.sepY) > DRIFT_EPS) {
        violator("C4", s.id, { anchor: "separatorY", r0: a.sepY, r1: b.sepY, driftPx: Number(Math.abs(a.sepY - b.sepY).toFixed(3)) });
      }
    }
  }

  // ── C1 cross-surface analysis (reference = launcher geometry) ────────────
  const ref = matrix["launcher:query"]?.anchors;
  const c1Table: Json[] = [];
  for (const [key, cell] of Object.entries(matrix)) {
    const a = cell.anchors;
    const row: Json = { cell: key, sepY: a.separator?.y ?? null, sepX: a.separator?.x ?? null, sepPresent: a.separator?.present ?? null, listTop: a.listRegion?.y ?? null, firstRow: a.firstRow ?? null, gaps: cell.gaps };
    if (ref?.separator && a.separator && typeof a.separator.y === "number" && a.separator.y >= 0 && typeof ref.separator.y === "number" && ref.separator.y >= 0) {
      row.vsLauncherSepY = Number((a.separator.y - ref.separator.y).toFixed(3));
      if (Math.abs(row.vsLauncherSepY) > DRIFT_EPS) violator("C1", key, { anchor: "separatorY", vsLauncherPx: row.vsLauncherSepY });
    }
    if (ref?.firstRow && a.firstRow) {
      row.vsLauncherFirstRow = {
        dy: Number((a.firstRow.y - ref.firstRow.y).toFixed(3)),
        dx: Number((a.firstRow.x - ref.firstRow.x).toFixed(3)),
        dwidth: Number((a.firstRow.width - ref.firstRow.width).toFixed(3)),
        dheight: Number((a.firstRow.height - ref.firstRow.height).toFixed(3)),
      };
      if (Math.abs(row.vsLauncherFirstRow.dy) > DRIFT_EPS) violator("C1", key, { anchor: "firstRowY", vsLauncherPx: row.vsLauncherFirstRow.dy });
      if (Math.abs(row.vsLauncherFirstRow.dx) > DRIFT_EPS) violator("C1", key, { anchor: "firstRowX", vsLauncherPx: row.vsLauncherFirstRow.dx });
      if (Math.abs(row.vsLauncherFirstRow.dwidth) > DRIFT_EPS) violator("C1", key, { anchor: "firstRowWidth", vsLauncherPx: row.vsLauncherFirstRow.dwidth });
      if (Math.abs(row.vsLauncherFirstRow.dheight) > DRIFT_EPS) violator("C1", key, { anchor: "firstRowHeight", vsLauncherPx: row.vsLauncherFirstRow.dheight });
    }
    for (const g of cell.gaps) violator("C1", key, { anchor: "measurability", gap: g });
    c1Table.push(row);
  }
  writeFileSync(join(OUT_DIR, "c1-matrix.json"), JSON.stringify(c1Table, null, 2));
  snaps["c3"] = c3;
} catch (e) {
  crashed = String(e).slice(0, 240);
} finally {
  for (let i = 0; i < 3; i++) {
    d.simulateKey("escape");
    await sleep(60);
  }
  d.send({ type: "hide" });
  await sleep(200);
  let windowVisible: unknown = "unknown";
  try {
    windowVisible = (await d.getState({ timeoutMs: 4000 })).windowVisible;
  } catch {}
  const sessionDir = (d as unknown as { sessionDir?: string }).sessionDir;
  await d.close();
  if (sessionDir) {
    try { execSync(`rm -rf ${JSON.stringify(sessionDir)}`); } catch {}
  }
  const summary = {
    phase: "OF-16 Phase A measurement matrix (NN=23)",
    crashed: crashed || null,
    windowVisible,
    driftEpsPx: DRIFT_EPS,
    matrixCells: Object.keys(matrix).length,
    violatorCount: violators.length,
    violators,
    snaps,
    binary: BINARY,
  };
  writeFileSync(join(OUT_DIR, "summary.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  console.error(`[${crashed ? "FAIL" : "DONE"}] eyeline-phaseA: cells=${Object.keys(matrix).length} violators=${violators.length} ${crashed ? "CRASH:" + crashed : ""}`);
  process.exit(crashed ? 1 : 0);
}
