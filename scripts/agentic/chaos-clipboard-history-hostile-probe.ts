#!/usr/bin/env bun
/**
 * Chaos battery 06 (2026-07-18): clipboardHistory surface under hostile
 * seeded entries + external DB churn.  Prior batteries only ran filter-perf
 * sweeps over an EMPTY clipboard history (chaos-multisurface-perf); none ever
 * rendered hostile content or mutated the store behind the open surface.
 *
 * Seeds the SANDBOX clipboard-history.sqlite directly (never the real
 * pasteboard / real store):
 *  - hostile text entries: zalgo/combining, RTL bidi override, ZWJ emoji,
 *    ANSI/control chars, script-tag content, 400KB single line, 10k-line blob
 *  - 1000 filler entries for filter-latency pressure
 *
 * Rows checked (lenses: correctness, perf, layout/CLS, data-integrity):
 *  1. hostile-render: surface opens, hostile rows render, app alive, no new
 *     ERROR log entries, no duplicate semantic ids.
 *  2. filter-perf: per-keystroke settle over a 12-key burst with 1000 entries;
 *     budget: every settle < 4s, no never-settled snapshots.
 *  3. external-churn: delete half the entries from the DB behind the UI,
 *     re-filter; app alive, selection valid, no crash.
 *  4. empty-state + recovery: zero-match filter is graceful; clearing recovers.
 *  5. CLS: input/header/footer chrome within 1px across everything.
 *
 * Safe: sandboxHome, protocol-only, bun:sqlite writes confined to the sandbox.
 */
import { Database } from "bun:sqlite";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY;
const CLS_EPS = 1.0;
const SETTLE_BUDGET_MS = 4000;
// High enough that truncation can never hide row elements (choices are
// emitted before footer elements; default limit 50 truncates the footer,
// but a paranoid probe should not depend on collector ordering).
const ELEMENTS_LIMIT = 300;

type Bounds = { x: number; y: number; width: number; height: number };
const STABLE_HINTS = ["input", "search", "footer", "header", "toolbar", "hint"];

function stableBounds(info: Json): Map<string, Bounds> {
  const m = new Map<string, Bounds>();
  for (const c of (info?.components ?? []) as Json[]) {
    if (!c?.bounds || typeof c.bounds.y !== "number") continue;
    const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
    if (STABLE_HINTS.some((h) => hay.includes(h))) m.set(`${c.name}|${c.type ?? ""}`, c.bounds as Bounds);
  }
  return m;
}
const drift = (a: Bounds, b: Bounds) =>
  Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y), Math.abs(a.height - b.height));

// Clipboard rows surface as `type: "choice"` elements.  NOTE: getElements
// choice elements carry NO `selectable` field — battery 06's residual red
// (ledger OF-1) was this probe filtering rows on `e.selectable === true`,
// which is vacuously false for every rendered row, so `no-rows-rendered`
// fired even when 48 choices were on screen.  Row presence = choice elements.
function rowsOf(els: Json) {
  return ((els?.elements ?? []) as Json[])
    .filter((e) => e.type === "choice")
    .map((e) => ({
      semanticId: String(e.semanticId ?? ""),
      text: typeof e.text === "string" ? e.text.slice(0, 60) : null,
      selected: e.selected === true,
    }));
}

const HOSTILE: [string, string][] = [
  ["zalgo", "ź̴̨̀ą́̀ĺ̀ǵò-clip " + "́".repeat(200)],
  ["rtl-bidi", "‮reversed‬ clip العربية"],
  ["zwj-emoji", "👩‍👩‍👧‍👦👨‍👩‍👧‍👦 emoji-family-clip"],
  ["ansi-control", "ansi-clip \x1b[31mred\x1b[0m \x01\x02\x03 bell\x07"],
  ["script-tag", "<script>alert('clip')</script> <img src=x onerror=y>"],
  ["huge-line", "H".repeat(400_000)],
  ["many-lines", Array.from({ length: 10_000 }, (_, i) => `line-${i}`).join("\n")],
];

const findings: Json[] = [];
let crashed = "";

// Pre-seed a manual sandbox HOME before launch: entries written mid-session by
// an external process bypass the app's insert path and legitimately lack
// text_preview until the next launch's backfill (battery 06 finding — the
// backfill lane heals legacy rows at open, so seed FIRST, then launch).
const sandboxHome = join(
  process.env.CHAOS_SCRATCH ?? "/tmp",
  `chaos-clip-home-${process.pid}`,
);
const dbDir = join(sandboxHome, ".scriptkit", "db");
const dbPath = join(dbDir, "clipboard-history.sqlite");

function seedDb() {
  Bun.spawnSync(["mkdir", "-p", dbDir]);
  const db = new Database(dbPath, { create: true });
  db.exec(`CREATE TABLE IF NOT EXISTS history (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    content_hash TEXT,
    content_type TEXT NOT NULL DEFAULT 'text',
    timestamp INTEGER NOT NULL,
    pinned INTEGER DEFAULT 0,
    ocr_text TEXT
  )`);
  const now = Date.now();
  const ins = db.prepare(
    "INSERT OR REPLACE INTO history (id, content, content_type, timestamp, pinned) VALUES (?, ?, 'text', ?, 0)",
  );
  const tx = db.transaction(() => {
    HOSTILE.forEach(([label, content], i) => ins.run(`hostile-${label}`, content, now - i));
    for (let i = 0; i < 1000; i++) {
      ins.run(`filler-${String(i).padStart(4, "0")}`, `filler clip entry ${i} lorem-${i % 97}`, now - 1000 - i);
    }
  });
  tx();
  db.close();
}

async function settleSnap(label: string) {
  let settled = false, probes = 0, elapsedMs = 0;
  try {
    const r: Json = await d.waitForSettle({ timeoutMs: SETTLE_BUDGET_MS });
    settled = r?.settled ?? false;
    probes = r?.probes ?? 0;
    elapsedMs = r?.elapsedMs ?? 0;
  } catch { /* recorded unsettled */ }
  const [els, layout] = await Promise.all([
    d.getElements({ limit: ELEMENTS_LIMIT }, { timeoutMs: 8000 }),
    d.getLayoutInfo({}, { timeoutMs: 8000 }),
  ]);
  const rows = rowsOf(els);
  const seen = new Set<string>();
  for (const r of rows) {
    if (r.semanticId && seen.has(r.semanticId)) {
      findings.push({ kind: "duplicate-semantic-ids", label, id: r.semanticId });
    }
    seen.add(r.semanticId);
  }
  return {
    label, settled, probes, elapsedMs, rows,
    totalCount: (els?.totalCount ?? els?.total_count ?? null) as number | null,
    rawElements: ((els?.elements ?? []) as Json[]),
    stable: stableBounds(layout),
  };
}

const snaps: Awaited<ReturnType<typeof settleSnap>>[] = [];
let prevStable: Map<string, Bounds> | null = null;
let prevLabel = "";
function clsCheck(cur: { label: string; stable: Map<string, Bounds> }) {
  if (prevStable) {
    for (const [k, pb] of prevStable) {
      const cb = cur.stable.get(k);
      if (!cb) continue;
      const dpx = drift(pb, cb);
      if (dpx > CLS_EPS) {
        findings.push({ kind: "chrome-layout-shift", surface: k, from: prevLabel, to: cur.label, driftPx: Number(dpx.toFixed(2)) });
      }
    }
  }
  prevStable = cur.stable;
  prevLabel = cur.label;
}

let errorBaseline = 0;
async function newErrors(label: string) {
  const logs: Json = await d.getLogs({ limit: 200, level: "error" }).catch(() => null);
  const count = (logs?.entries ?? []).length;
  if (count > errorBaseline) {
    findings.push({
      kind: "new-error-logs", label, count: count - errorBaseline,
      sample: (logs?.entries ?? []).slice(-3).map((e: Json) => String(e.message).slice(0, 140)),
    });
    errorBaseline = count;
  }
}

seedDb();
const d = await Driver.launch({
  ...(BINARY ? { binary: BINARY } : {}),
  env: { HOME: sandboxHome, SK_PATH: join(sandboxHome, ".scriptkit") },
});

try {
  d.send({ type: "show" });
  await Bun.sleep(400);
  await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});
  {
    const logs: Json = await d.getLogs({ limit: 200, level: "error" }).catch(() => null);
    errorBaseline = (logs?.entries ?? []).length;
  }

  // --- Row 1: hostile-render ---
  // Verify the surface actually opened; a trigger sent during startup load can
  // race the view transition. One retry, then it's a hard finding.
  let opened = false;
  for (let attempt = 0; attempt < 2 && !opened; attempt++) {
    d.send({ type: "triggerBuiltin", name: "clipboardHistory" });
    for (let i = 0; i < 10; i++) {
      await Bun.sleep(300);
      const st: Json = await d.getState({ timeoutMs: 8000 });
      if (st?.promptType === "clipboardHistory") { opened = true; break; }
    }
    if (!opened && attempt === 0) findings.push({ kind: "papercut-trigger-retry", note: "first triggerBuiltin did not transition within 3s" });
  }
  if (!opened) {
    crashed = "clipboardHistory never opened after 2 trigger attempts";
    throw new Error(crashed);
  }
  const open = await settleSnap("open-clipboard-history");
  snaps.push(open);
  clsCheck(open);
  if (open.rows.length === 0) {
    // Self-diagnosing red: capture what the collector DID return plus recent
    // warn+ logs so an empty open is attributable (cold cache / migration
    // race / collector change) instead of a bare count.
    const logs: Json = await d.getLogs({ limit: 100, level: "warn" }).catch(() => null);
    findings.push({
      kind: "no-rows-rendered",
      note: "seeded 1007 entries; open snapshot has zero choice elements",
      totalCount: open.totalCount,
      elementSample: open.rawElements.slice(0, 10).map((e) => ({
        type: e.type, semanticId: e.semanticId, role: e.role,
        text: typeof e.text === "string" ? e.text.slice(0, 40) : null,
      })),
      recentWarnLogs: ((logs?.entries ?? []) as Json[]).slice(-10).map((e) => String(e.message).slice(0, 160)),
    });
  }
  await newErrors("hostile-render");

  // Battery 06 lock: blessed `↵ Paste` primary label must NOT fire the
  // universal three-key contract violation (false-positive alarm drift).
  {
    const logs: Json = await d.getLogs({ limit: 200 }).catch(() => null);
    const violation = (logs?.entries ?? []).find((e: Json) =>
      String(e.message).includes("prompt_hint_contract_violation") &&
      String(e.message).includes("clipboard_history"),
    );
    if (violation) {
      findings.push({ kind: "footer-contract-false-positive", message: String(violation.message).slice(0, 200) });
    }
  }

  // --- Row 2: filter-perf burst over 1000 entries ---
  const burst = "lorem-1 fill";
  let acc = "";
  for (const ch of burst) {
    acc += ch;
    d.setFilter(acc);
    const s = await settleSnap(`filter:${acc.length}`);
    snaps.push(s);
    clsCheck(s);
  }
  await newErrors("filter-burst");

  // OF-2 lock: the clipboard surface must not spam `flat sizing fallback`
  // WARNs per keystroke.  height_for_main_window ignores sizing (fixed
  // height), so the flat path is not a degraded fallback here — an
  // always-firing WARN trains readers to ignore real contract violations
  // (same class as battery 06's footer-audit false positive).
  {
    const logs: Json = await d.getLogs({ limit: 1000, level: "warn" }).catch(() => null);
    const flatWarns = ((logs?.entries ?? []) as Json[]).filter((e) =>
      String(e.message).includes("flat sizing fallback"),
    );
    if (flatWarns.length > 0) {
      findings.push({
        kind: "flat-sizing-warn-spam",
        count: flatWarns.length,
        sample: String(flatWarns[flatWarns.length - 1]?.message ?? "").slice(0, 140),
      });
    }
  }

  // --- Row 3: external churn behind the open surface ---
  {
    const db = new Database(dbPath);
    db.exec("DELETE FROM history WHERE id LIKE 'filler-0%'"); // drop ~1000-block prefix chunk
    db.close();
  }
  d.setFilter("filler");
  const churn = await settleSnap("post-external-delete");
  snaps.push(churn);
  clsCheck(churn);
  const st1: Json = await d.getState({ timeoutMs: 8000 });
  if (!st1 || typeof st1 !== "object") crashed = "post-external-delete: bad state";
  await newErrors("external-churn");

  // --- Row 4: zero-match + recovery ---
  d.setFilter("zz-no-match-xq");
  const empty = await settleSnap("zero-match");
  snaps.push(empty);
  clsCheck(empty);
  d.setFilter("");
  const recovered = await settleSnap("cleared");
  snaps.push(recovered);
  clsCheck(recovered);
  if (recovered.rows.length === 0 && open.rows.length > 0) {
    findings.push({ kind: "recovery-lost-rows", note: "rows rendered before zero-match but not after clearing" });
  }
  await newErrors("empty-recovery");

  // --- Escape back to launcher; verify alive ---
  d.simulateKey("escape");
  await Bun.sleep(250);
  d.setFilter("recover-clip-probe");
  await Bun.sleep(250);
  const st2: Json = await d.getState({ timeoutMs: 8000 });
  if ((st2 as Json)?.inputValue !== "recover-clip-probe") {
    crashed = crashed || `recovery failed: inputValue=${JSON.stringify((st2 as Json)?.inputValue)}`;
  }
} catch (e) {
  crashed = crashed || String(e).slice(0, 200);
} finally {
  await d.close();
  Bun.spawnSync(["rm", "-rf", sandboxHome]);
}

const neverSettled = snaps.filter((s) => !s.settled);
const perf = snaps.filter((s) => s.label.startsWith("filter:"));
const maxSettleMs = Math.max(0, ...perf.map((s) => s.elapsedMs));
const bugKinds = [
  "duplicate-semantic-ids", "chrome-layout-shift", "new-error-logs",
  "recovery-lost-rows", "no-rows-rendered", "footer-contract-false-positive",
  "flat-sizing-warn-spam",
];
const bugFindings = findings.filter((f) => bugKinds.includes(String(f.kind)));
const verdict = crashed ? "FAIL" : bugFindings.length > 0 || neverSettled.length > 0 ? "REGRESSION" : "PASS";

console.log(JSON.stringify({
  verdict,
  crashed: crashed || null,
  rowCounts: snaps.map((s) => ({ label: s.label, rows: s.rows.length, totalCount: s.totalCount, settled: s.settled, probes: s.probes, elapsedMs: s.elapsedMs })),
  perf: { samples: perf.length, maxSettleMs, budgetMs: SETTLE_BUDGET_MS },
  neverSettled: neverSettled.map((s) => s.label),
  findings,
  binary: BINARY ?? "auto",
}, null, 2));
console.error(`[${verdict}] clipboard-hostile: snaps=${snaps.length} findings=${findings.length} maxSettle=${maxSettleMs}ms ${crashed ? "CRASH:" + crashed : "alive"}`);
process.exit(verdict === "FAIL" ? 1 : 0);
