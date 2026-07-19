#!/usr/bin/env bun
/**
 * Chaos-monkey NEW scenario (2026-07-18, battery 05): root-launcher DIRECTORY
 * BROWSE under LIVE filesystem churn.  Prior batteries fuzzed filter text and
 * builtin churn; none ever mutated the filesystem UNDER an open browse surface.
 *
 * The root launcher treats `/abs/path/` queries as directory browses (readdir,
 * not mdfind — see `looks_like_root_directory_browse_query`), so rows track a
 * real directory that this probe churns (create/delete/rename) between
 * keystrokes.  Reality-vs-intent checks:
 *
 *  1. baseline: browse a seeded dir → rows appear, chrome layout captured.
 *  2. churn rounds: add/remove/rename files while typing child fragments and
 *     arrowing.  After every settle: app alive, no duplicate semantic ids,
 *     selection (if any) points at a row that actually exists, stable chrome
 *     (input/header/footer) holds within 1px (CLS).
 *  3. delete-under-cursor: delete the file backing the selected row, refresh,
 *     assert selection coerces to a live row (no stale/phantom selection).
 *  4. dir-vanish: rm -rf the browsed directory mid-browse, keep typing —
 *     assert no crash, graceful empty/fallback state, and full recovery
 *     (normal filter round-trips afterward).
 *
 * Safe: sandboxHome, protocol-only, churn confined to a scratchpad temp dir.
 */
import { mkdirSync, writeFileSync, rmSync, renameSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY; // undefined => driver auto-picks freshest
const CLS_EPS = 1.0;

const CHURN_ROOT = join(
  process.env.CHAOS_SCRATCH ?? "/tmp",
  `chaos-dir-browse-${process.pid}`,
);

type Bounds = { x: number; y: number; width: number; height: number };
const STABLE_HINTS = ["input", "search", "footer", "header", "toolbar", "hint"];

function stableBounds(info: Json): Map<string, Bounds> {
  const m = new Map<string, Bounds>();
  for (const c of (info?.components ?? []) as Json[]) {
    if (!c?.bounds || typeof c.bounds.y !== "number") continue;
    const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
    if (STABLE_HINTS.some((h) => hay.includes(h))) {
      m.set(`${c.name}|${c.type ?? ""}`, c.bounds as Bounds);
    }
  }
  return m;
}

function drift(a: Bounds, b: Bounds): number {
  return Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y), Math.abs(a.height - b.height));
}

interface Row {
  semanticId: string;
  text: string | null;
  selectable: boolean;
  selected: boolean;
}

function rowsOf(elementsResult: Json): Row[] {
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
      text: typeof e.text === "string" ? e.text.slice(0, 80) : null,
      selectable: e.selectable === true,
      selected: e.selected === true,
    }));
}

const findings: Json[] = [];
let crashed = "";
let fileSeq = 0;

function seedFile(name?: string): string {
  const fname = name ?? `churn-${String(fileSeq++).padStart(3, "0")}.txt`;
  writeFileSync(join(CHURN_ROOT, fname), `chaos ${fname}\n`);
  return fname;
}

mkdirSync(CHURN_ROOT, { recursive: true });
for (let i = 0; i < 30; i++) seedFile();

const d = await Driver.launch({ sandboxHome: true, ...(BINARY ? { binary: BINARY } : {}) });

async function settleSnap(label: string) {
  let settled = false, probes = 0, elapsedMs = 0;
  try {
    const r: Json = await d.waitForSettle({ timeoutMs: 4000 });
    settled = r?.settled ?? false;
    probes = r?.probes ?? 0;
    elapsedMs = r?.elapsedMs ?? 0;
  } catch { /* recorded as unsettled */ }
  const [elements, layout] = await Promise.all([
    d.getElements({}, { timeoutMs: 6000 }),
    d.getLayoutInfo({}, { timeoutMs: 6000 }),
  ]);
  const rows = rowsOf(elements);
  // Duplicate semantic ids = broken row identity under churn.
  const seen = new Set<string>(), dupes = new Set<string>();
  for (const r of rows) {
    if (r.semanticId && seen.has(r.semanticId)) dupes.add(r.semanticId);
    seen.add(r.semanticId);
  }
  if (dupes.size > 0) {
    findings.push({ kind: "duplicate-semantic-ids", label, dupes: [...dupes].slice(0, 6) });
  }
  // A selected row must exist in the row set (it is BY DEFINITION in the set —
  // the real check is that selection exists at all when selectable rows do,
  // and that a selected row is actually selectable).
  const selectedRows = rows.filter((r) => r.selected);
  for (const s of selectedRows) {
    if (!s.selectable) {
      findings.push({ kind: "selected-unselectable-row", label, id: s.semanticId, text: s.text });
    }
  }
  return { label, settled, probes, elapsedMs, rows, selectedRows, stable: stableBounds(layout) };
}

function diffChrome(prev: { label: string; stable: Map<string, Bounds> }, cur: { label: string; stable: Map<string, Bounds> }) {
  for (const [k, pb] of prev.stable) {
    const cb = cur.stable.get(k);
    if (!cb) continue;
    const dpx = drift(pb, cb);
    if (dpx > CLS_EPS) {
      findings.push({
        kind: "chrome-layout-shift", surface: k, from: prev.label, to: cur.label,
        driftPx: Number(dpx.toFixed(2)),
      });
    }
  }
}

const snaps: Awaited<ReturnType<typeof settleSnap>>[] = [];

try {
  d.send({ type: "show" });
  await Bun.sleep(400);
  await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});

  // --- Phase 1: baseline browse ---
  d.setFilter(`${CHURN_ROOT}/`);
  const baseline = await settleSnap("baseline-browse");
  snaps.push(baseline);
  if (baseline.rows.filter((r) => r.selectable).length === 0) {
    // Not a soft warning: every later phase is vacuous without browse rows.
    crashed = `baseline browse of ${CHURN_ROOT}/ produced 0 selectable rows`;
    throw new Error(crashed);
  }

  // --- Phase 2: churn rounds ---
  const fragments = ["c", "ch", "chu", "chur", "churn-0", "churn-01", "churn", ""];
  let prev = baseline;
  for (let round = 0; round < 12; round++) {
    // fs churn between keystrokes
    seedFile(); seedFile();
    const existing = readdirSync(CHURN_ROOT);
    if (existing.length > 3) {
      rmSync(join(CHURN_ROOT, existing[round % existing.length]), { force: true });
    }
    const renTarget = readdirSync(CHURN_ROOT)[0];
    if (renTarget) {
      renameSync(join(CHURN_ROOT, renTarget), join(CHURN_ROOT, `ren-${round}-${renTarget}`.slice(0, 60)));
    }
    d.simulateKey("down");
    d.simulateKey("down");
    d.setFilter(`${CHURN_ROOT}/${fragments[round % fragments.length]}`);
    const cur = await settleSnap(`churn-round-${round}`);
    snaps.push(cur);
    diffChrome(prev, cur);
    prev = cur;
    const state = await d.getState({ timeoutMs: 6000 });
    if (!state || typeof state !== "object") {
      crashed = `churn-round-${round}: getState returned ${JSON.stringify(state).slice(0, 80)}`;
      throw new Error(crashed);
    }
  }

  // --- Phase 3: delete the file under the cursor ---
  d.setFilter(`${CHURN_ROOT}/`);
  const preDelete = await settleSnap("pre-delete-under-cursor");
  snaps.push(preDelete);
  const sel = preDelete.selectedRows[0] ?? preDelete.rows.find((r) => r.selectable);
  if (sel?.text) {
    // Row text carries the file name; find a churn file matching it.
    const match = readdirSync(CHURN_ROOT).find((f) => sel.text!.includes(f));
    if (match) {
      rmSync(join(CHURN_ROOT, match), { force: true });
      // Same-directory listings revalidate on a 2s TTL (ROOT_FILE_BROWSE_REFRESH_TTL);
      // wait past it, then re-request and give the async readdir time to publish.
      await Bun.sleep(2200);
      d.setFilter(`${CHURN_ROOT}/x`);
      await Bun.sleep(150);
      d.setFilter(`${CHURN_ROOT}/`);
      await Bun.sleep(400);
      const postDelete = await settleSnap("post-delete-under-cursor");
      snaps.push(postDelete);
      const stale = postDelete.rows.find((r) => r.text?.includes(match));
      if (stale) {
        findings.push({ kind: "stale-row-after-delete", file: match, rowId: stale.semanticId });
      }
      const sel2 = postDelete.selectedRows[0];
      if (sel2 && sel2.text?.includes(match)) {
        findings.push({ kind: "selection-on-deleted-file", file: match, rowId: sel2.semanticId });
      }
    }
  }

  // --- Phase 4: directory vanishes mid-browse ---
  d.setFilter(`${CHURN_ROOT}/chu`);
  await settleSnap("pre-vanish");
  rmSync(CHURN_ROOT, { recursive: true, force: true });
  d.setFilter(`${CHURN_ROOT}/churn`);
  const vanished = await settleSnap("post-vanish");
  snaps.push(vanished);
  const ghost = vanished.rows.filter((r) => r.text?.includes("churn-") || r.text?.includes("ren-"));
  if (ghost.length > 0) {
    findings.push({
      kind: "ghost-rows-after-dir-delete", count: ghost.length,
      sample: ghost.slice(0, 4).map((r) => ({ id: r.semanticId, text: r.text })),
    });
  }

  // --- Recovery ---
  d.setFilter("");
  await Bun.sleep(150);
  d.setFilter("recover-churn");
  await Bun.sleep(250);
  const s = await d.getState({ timeoutMs: 6000 });
  if ((s as Json)?.inputValue !== "recover-churn") {
    crashed = crashed || `recovery failed: inputValue=${JSON.stringify((s as Json)?.inputValue)}`;
  }
} catch (e) {
  crashed = crashed || String(e).slice(0, 200);
} finally {
  await d.close();
  rmSync(CHURN_ROOT, { recursive: true, force: true });
}

const neverSettled = snaps.filter((s) => !s.settled);
const bugFindings = findings.filter((f) =>
  ["duplicate-semantic-ids", "selection-on-deleted-file", "chrome-layout-shift", "selected-unselectable-row"].includes(
    String(f.kind),
  ),
);
const verdict = crashed ? "FAIL" : bugFindings.length > 0 ? "REGRESSION" : "PASS";

const report = {
  verdict,
  crashed: crashed || null,
  churnRoot: CHURN_ROOT,
  snaps: snaps.map((s) => ({
    label: s.label, settled: s.settled, probes: s.probes, elapsedMs: s.elapsedMs,
    rowCount: s.rows.length, selected: s.selectedRows.map((r) => r.semanticId).slice(0, 2),
  })),
  neverSettled: neverSettled.map((s) => s.label),
  findings,
  binary: BINARY ?? "auto",
};
console.log(JSON.stringify(report, null, 2));
console.error(
  `[${verdict}] dir-browse-churn: snaps=${snaps.length} findings=${findings.length} ` +
    `neverSettled=${neverSettled.length} ${crashed ? "CRASH:" + crashed : "alive"}`,
);
process.exit(verdict === "FAIL" ? 1 : 0);
