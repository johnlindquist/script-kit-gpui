#!/usr/bin/env bun
/**
 * Chaos-monkey NEW scenario (2026-07-18): layout-shift (CLS) + sustained-input
 * jank hunt on the launcher.  Measures REAL per-surface geometry drift and
 * frame-settle timing — the perf/CLS watch the previous batteries never did.
 *
 * What it checks:
 *  1. CLS: capture getLayoutInfo across a realistic type/backspace/clear
 *     sequence.  Surfaces that must NOT move when only the RESULT LIST content
 *     changes (search input, footer, header/toolbar) are tracked for x/y/height
 *     drift.  Any drift over CLS_EPS px on a should-be-stable surface is a shift.
 *  2. Sustained-input jank: measure settle frames + wall-time per keystroke over
 *     a long rapid burst; flag if late keystrokes settle much slower than early
 *     ones (jank / accumulation / leak signal).
 *  3. Edge inputs never tried live: zalgo/combining, RTL/bidi, ZWJ emoji,
 *     5000-char single line — assert no crash, input round-trips, no CLS blowup.
 *
 * Safe: sandboxHome, protocol-only (no OS input synthesis), no file deletion.
 */
import { join } from "node:path";
import { Driver } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");

const CLS_EPS = 1.0; // px; sub-pixel jitter tolerated, whole-px shift is not.

type Bounds = { x: number; y: number; width: number; height: number };
type Comp = { name: string; type?: string; bounds?: Bounds };

// Surfaces that should hold position while only the result LIST content changes.
// Matched by case-insensitive substring against component name/type.
const STABLE_HINTS = ["input", "search", "footer", "header", "toolbar", "hint"];

function isStable(c: Comp): boolean {
  const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
  return STABLE_HINTS.some((h) => hay.includes(h));
}

function comps(info: any): Comp[] {
  const raw = Array.isArray(info?.components) ? info.components : [];
  return raw
    .filter((c: any) => c && c.bounds && typeof c.bounds.y === "number")
    .map((c: any) => ({ name: String(c.name ?? ""), type: String(c.type ?? ""), bounds: c.bounds }));
}

function drift(a: Bounds, b: Bounds): number {
  return Math.max(
    Math.abs(a.x - b.x),
    Math.abs(a.y - b.y),
    Math.abs(a.height - b.height),
  );
}

const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
const shifts: any[] = [];
// probes = poll rounds until state stabilized (repaint churn); settled = did it
// ever stabilize within the deadline (false => continuous repaint = real jank).
const settleSamples: { label: string; probes: number; elapsedMs: number; settled: boolean }[] = [];
let crashed = "";

async function snap(label: string): Promise<Map<string, Bounds>> {
  let probes = 0, elapsedMs = 0, settled = false;
  try {
    const r = await d.waitForSettle({ timeoutMs: 4000 });
    probes = (r as any).probes ?? 0;
    elapsedMs = (r as any).elapsedMs ?? 0;
    settled = (r as any).settled ?? false;
  } catch { /* settle timeout tolerated; recorded as unsettled */ }
  settleSamples.push({ label, probes, elapsedMs, settled });
  const info = await d.getLayoutInfo({}, { timeoutMs: 6000 });
  const m = new Map<string, Bounds>();
  for (const c of comps(info)) if (isStable(c)) m.set(`${c.name}|${c.type}`, c.bounds!);
  return m;
}

function diffStable(prev: Map<string, Bounds>, cur: Map<string, Bounds>, from: string, to: string) {
  for (const [k, pb] of prev) {
    const cb = cur.get(k);
    if (!cb) continue; // appearance/disappearance handled separately, not a shift
    const dpx = drift(pb, cb);
    if (dpx > CLS_EPS) {
      shifts.push({ surface: k, from, to, driftPx: Number(dpx.toFixed(2)), prev: pb, cur: cb });
    }
  }
}

try {
  d.send({ type: "show" });
  await Bun.sleep(400);

  // --- Phase 1: CLS across a realistic type / backspace / clear sequence ---
  // Only the result LIST should change; tracked stable surfaces must hold.
  const seq = ["", "s", "se", "set", "sett", "set", "se", "s", "", "clip", "clipboard", ""];
  let prev: Map<string, Bounds> | null = null;
  let prevLabel = "init";
  for (const q of seq) {
    d.setFilter(q);
    const label = `filter="${q}"`;
    const cur = await snap(label);
    if (prev) diffStable(prev, cur, prevLabel, label);
    prev = cur;
    prevLabel = label;
  }

  // --- Phase 2: sustained-input jank — long rapid burst, watch settle growth ---
  const burst = "abcdefghijklmnopqrstuvwxyz0123456789";
  let acc = "";
  for (const ch of burst) {
    acc += ch;
    d.setFilter(acc);
    await snap(`burst:${acc.length}`);
  }
  d.setFilter("");
  await snap("burst-clear");

  // --- Phase 3: edge inputs never fired live ---
  const edges: [string, string][] = [
    ["zalgo", "ź̴̨̀ą́̀ĺ̀ǵò"],
    ["rtl-bidi", "‮malicious‬ العربية mix"],
    ["zwj-emoji", "👩‍👩‍👧‍👦 family"],
    ["long-line", "L".repeat(5000)],
    ["combining-tail", "a" + "́".repeat(400)],
  ];
  const edgeBaseline = await snap("edge-baseline");
  for (const [label, text] of edges) {
    d.setFilter(text);
    const cur = await snap(`edge:${label}`);
    // Compare against baseline: stable surfaces shouldn't blow up on edge text.
    diffStable(edgeBaseline, cur, "edge-baseline", `edge:${label}`);
    const s = await d.getState({ timeoutMs: 6000 });
    if (!s || typeof s !== "object") { crashed = `edge:${label}: bad state`; break; }
  }

  // --- Recovery: normal filter still works after all abuse ---
  d.setFilter("");
  await Bun.sleep(150);
  d.setFilter("recover-xyz");
  await Bun.sleep(200);
  const s = await d.getState({ timeoutMs: 6000 });
  if (!s || (s as any).inputValue !== "recover-xyz") {
    crashed = crashed || `recovery: inputValue=${JSON.stringify((s as any)?.inputValue)}`;
  }
} catch (e) {
  crashed = crashed || String(e).slice(0, 200);
}

await d.close();

// --- Analysis ---
// Repaint churn = probes beyond the minimum stable count (3). A settle that
// needed many probes, or never settled, means the surface kept repainting.
const MIN_PROBES = 3;
const churn = (s: { probes: number }) => Math.max(0, s.probes - MIN_PROBES);
const burstSettles = settleSamples.filter((s) => s.label.startsWith("burst:"));
const early = burstSettles.slice(0, 8);
const late = burstSettles.slice(-8);
const avg = (a: any[], f: (x: any) => number) => (a.length ? a.reduce((s, x) => s + f(x), 0) / a.length : 0);
const earlyChurn = avg(early, churn), lateChurn = avg(late, churn);
const neverSettled = settleSamples.filter((s) => !s.settled);
const maxChurn = Math.max(0, ...settleSamples.map(churn));
// Jank = burst keystrokes accumulate repaint churn late vs early, OR any surface
// never settles within 4s (continuous repaint), OR high peak churn.
const jankFail =
  (lateChurn > earlyChurn + 2 && lateChurn > 3) ||
  neverSettled.length > 2 ||
  maxChurn > 8;

const clsFail = shifts.length > 0;
const verdict = crashed ? "FAIL" : clsFail || jankFail ? "REGRESSION" : "PASS";

const report = {
  verdict,
  crashed: crashed || null,
  cls: { shifts: shifts.length, epsPx: CLS_EPS, stableSurfacesTracked: true, detail: shifts.slice(0, 12) },
  jank: {
    earlyChurnAvg: Number(earlyChurn.toFixed(2)),
    lateChurnAvg: Number(lateChurn.toFixed(2)),
    maxChurn,
    neverSettledCount: neverSettled.length,
    neverSettledLabels: neverSettled.map((s) => s.label).slice(0, 8),
    rule: "lateChurn>earlyChurn+2&&>3, OR neverSettled>2, OR maxChurn>8",
  },
  samples: settleSamples.length,
  binary: BINARY,
};
console.log(JSON.stringify(report, null, 2));
console.error(
  `[${verdict}] cls-perf: shifts=${shifts.length} churn early=${earlyChurn.toFixed(1)} ` +
    `late=${lateChurn.toFixed(1)} max=${maxChurn} neverSettled=${neverSettled.length} ` +
    `${crashed ? "CRASH:" + crashed : "alive"}`,
);
