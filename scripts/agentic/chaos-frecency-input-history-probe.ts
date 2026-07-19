#!/usr/bin/env bun
/**
 * Chaos battery 13 (2026-07-18, lane L1): frecency + input-history resilience.
 * chaos-corrupt-state.ts already covers input_history corruption shapes
 * (invalid JSON, wrong shape, dir-as-file, 100k entries) — this battery covers
 * the UNCOVERED edges:
 *
 *  A. corrupt/truncated ~/.scriptkit/frecency.json at launch — launcher must
 *     come up, rows must render (frecency load is `.ok()`-swallowed at
 *     startup; a corrupt file must not take the launcher down with it).
 *  B. hostile input-history RECALL: 10k entries (load truncates to 100) with
 *     a 400KB first entry + zalgo/RTL entries — Up-arrow recall must not
 *     crash, must land the entry in the input (or a coherent alternative),
 *     must settle inside budget, and Escape must recover a usable launcher.
 *  C. CLS: launcher chrome within 1px across recall/recovery.
 *  D. no new ERROR log entries at any phase.
 *
 * Safe: pre-seeded scratch HOME (never the real ~/.scriptkit), hidden-window
 * protocol only, unique session per run.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY;
const CLS_EPS = 1.0;

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

const findings: Json[] = [];
let crashed = "";

const sandboxHome = join(
  process.env.CHAOS_SCRATCH ?? "/tmp",
  `chaos-frecency-home-${process.pid}`,
);
const kitDir = join(sandboxHome, ".scriptkit");
mkdirSync(kitDir, { recursive: true });

// Row A seed: truncated mid-object frecency JSON (a real crash-during-write
// shape from the pre-write_atomic era).
writeFileSync(
  join(kitDir, "frecency.json"),
  '{"entries":{"script/foo.ts":{"count":5,"last_used":1752800000,"sco',
);

// Row B seed: 10k entries; index 0 is a 400KB line, 1-2 are zalgo/RTL.
const hugeEntry = "H".repeat(400_000);
const entries = [
  hugeEntry,
  "ź̴̨̀ą́̀ĺ̀ǵò-history " + "́".repeat(120),
  "‮reversed‬ history العربية",
  ...Array.from({ length: 10_000 }, (_, i) => `history entry ${i} tok-${i % 53}`),
];
writeFileSync(
  join(kitDir, "input_history.json"),
  JSON.stringify({ entries, selected_results: {} }),
);

const d = await Driver.launch({
  ...(BINARY ? { binary: BINARY } : {}),
  env: { HOME: sandboxHome, SK_PATH: kitDir },
});

let errorBaseline = 0;
// Ledger OF-4/OF-6 CLOSED (chaos-19, 2026-07-18): the vendor gpui
// on_request_frame retry patch landed, so "window not found" / "RefCell
// already borrowed" frame-callback errors must no longer appear. These
// signatures are now a RED bug kind (vendor-frame-lifecycle-error): a
// recurrence means the vendor patch regressed or a new lifecycle race.
function isKnownVendorFrameNoise(msg: string): boolean {
  return (
    /vendor\/gpui\/src\/window\.rs/.test(msg) &&
    (msg.includes("window not found") || msg.includes("RefCell already borrowed"))
  );
}
async function newErrors(label: string) {
  const logs: Json = await d.getLogs({ limit: 300, level: "error" }).catch(() => null);
  const entries = ((logs?.entries ?? []) as Json[]);
  const count = entries.length;
  if (count > errorBaseline) {
    const fresh = entries.slice(errorBaseline);
    const vendorNoise = fresh.filter((e) => isKnownVendorFrameNoise(String(e.message)));
    const real = fresh.filter((e) => !isKnownVendorFrameNoise(String(e.message)));
    if (vendorNoise.length > 0) {
      findings.push({
        kind: "vendor-frame-lifecycle-error", label, count: vendorNoise.length, ledger: "OF-4-closed-chaos-19",
        sample: vendorNoise.slice(-2).map((e) => String(e.message).slice(0, 140)),
      });
    }
    if (real.length > 0) {
      findings.push({
        kind: "new-error-logs", label, count: real.length,
        sample: real.slice(-3).map((e) => String(e.message).slice(0, 140)),
      });
    }
    errorBaseline = count;
  }
}

try {
  d.send({ type: "show" });
  await Bun.sleep(400);
  await d.waitForSettle({ timeoutMs: 8000 }).catch(() => {});

  // --- Row A: launcher survives corrupt frecency ---
  const st0: Json = await d.getState({ timeoutMs: 8000 });
  if (!st0 || typeof st0 !== "object") crashed = "no state after launch over corrupt frecency";
  const els0: Json = await d.getElements({ limit: 300 }, { timeoutMs: 8000 });
  const rows0 = ((els0?.elements ?? []) as Json[]).filter((e) => e.type === "choice");
  if (rows0.length === 0) {
    findings.push({ kind: "launcher-empty-over-corrupt-frecency", note: "no choice rows after launch" });
  }
  // Row A2 (battery 14 rescue lock, runtime level): the corrupt frecency
  // file must be preserved as a `.corrupt` sidecar by the startup load —
  // never silently destroyed by a later fresh-start save.
  {
    const sidecar = Bun.file(join(kitDir, "frecency.json.corrupt"));
    if (!(await sidecar.exists())) {
      findings.push({ kind: "corrupt-frecency-not-rescued", note: "no frecency.json.corrupt sidecar after launch" });
    }
  }
  // Error-log baseline AFTER launch: startup noise from the seeded corruption
  // is expected to be at most a warn; anything after this point is new.
  {
    const logs: Json = await d.getLogs({ limit: 300, level: "error" }).catch(() => null);
    errorBaseline = (logs?.entries ?? []).length;
  }
  const layoutBefore = stableBounds(await d.getLayoutInfo({}, { timeoutMs: 8000 }));

  // --- Row B: huge/hostile history recall ---
  // A8 contract: Up walks the selection to the top of the list first; history
  // recall only ENTERS from the top row with an empty input. Walk up to 4
  // presses — recall must fire within them (battery 14 fixed the SimulateKey
  // dispatch to honor this contract at all; before, recall was unreachable
  // from automation).
  let recallLen = 0;
  let pressesUsed = 0;
  for (let i = 1; i <= 4 && recallLen === 0; i++) {
    d.simulateKey("up");
    pressesUsed = i;
    await d.waitForSettle({ timeoutMs: 8000 }).catch(() => {});
    const st: Json = await d.getState({ timeoutMs: 15000 });
    if (!st || typeof st !== "object") {
      crashed = "state unavailable during history recall walk";
      break;
    }
    recallLen = String(st?.inputValue ?? "").length;
  }
  findings.push({ kind: "note-huge-recall", inputLen: recallLen, pressesUsed });
  if (!crashed && recallLen === 0) {
    findings.push({ kind: "history-recall-swallowed", note: "no recall within 4 Up presses over seeded history" });
  } else if (!crashed && recallLen !== 400_000) {
    findings.push({ kind: "note-recall-unexpected-length", recallLen });
  }
  await newErrors("huge-recall");

  // Recall the zalgo + RTL entries.
  d.simulateKey("up");
  await d.waitForSettle({ timeoutMs: 8000 }).catch(() => {});
  d.simulateKey("up");
  await d.waitForSettle({ timeoutMs: 8000 }).catch(() => {});
  const st2: Json = await d.getState({ timeoutMs: 8000 });
  if (!st2 || typeof st2 !== "object") crashed = crashed || "state unavailable after hostile recalls";
  await newErrors("hostile-recall");

  // --- Recovery: clear and prove the launcher is usable ---
  d.simulateKey("escape");
  await Bun.sleep(250);
  d.send({ type: "show" });
  await Bun.sleep(250);
  await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});
  d.setFilter("frecency-probe-recovery");
  await Bun.sleep(300);
  const st3: Json = await d.getState({ timeoutMs: 8000 });
  if (st3?.inputValue !== "frecency-probe-recovery") {
    findings.push({ kind: "recovery-input-swallowed", inputValue: st3?.inputValue ?? null });
  }
  d.setFilter("");
  await newErrors("recovery");

  // --- Row C: chrome CLS across the storm ---
  {
    const layoutAfter = stableBounds(await d.getLayoutInfo({}, { timeoutMs: 8000 }));
    for (const [k, pb] of layoutBefore) {
      const cb = layoutAfter.get(k);
      if (!cb) continue;
      const dpx = drift(pb, cb);
      if (dpx > CLS_EPS) {
        findings.push({ kind: "chrome-layout-shift", surface: k, driftPx: Number(dpx.toFixed(2)) });
      }
    }
  }

  d.send({ type: "hide" });
  await Bun.sleep(250);
} catch (e) {
  crashed = crashed || String(e).slice(0, 200);
} finally {
  await d.close();
  Bun.spawnSync(["rm", "-rf", sandboxHome]);
}

const bugKinds = [
  "launcher-empty-over-corrupt-frecency", "corrupt-frecency-not-rescued", "history-recall-swallowed",
  "recovery-input-swallowed", "chrome-layout-shift", "new-error-logs", "vendor-frame-lifecycle-error",
];
const bugFindings = findings.filter((f) => bugKinds.includes(String(f.kind)));
const verdict = crashed ? "FAIL" : bugFindings.length > 0 ? "REGRESSION" : "PASS";
console.log(JSON.stringify({ verdict, crashed: crashed || null, findings, binary: BINARY ?? "auto" }, null, 2));
console.error(`[${verdict}] frecency-input-history: findings=${findings.length} ${crashed ? "CRASH:" + crashed : "alive"}`);
process.exit(verdict === "FAIL" ? 1 : 0);
