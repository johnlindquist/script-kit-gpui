#!/usr/bin/env bun
/**
 * L4 monkey-perf battery: per-keystroke latency p50/p95 + sample-while-typing
 * draw share across three filterable surfaces:
 *
 *   - mainList (launcher, fresh sandbox index)
 *   - clipboardHistory (pre-seeded 1,000 entries — perf under real load;
 *     chaos-multisurface-perf only ever filtered an EMPTY history)
 *   - emojiPicker
 *
 * Method (per flows/perf.md + root-typing-lag-benchmark.ts conventions):
 *   - Real dispatch: simulateGpuiEvent keyDown per character (NOT setFilter),
 *     target {type:"main"}.
 *   - Latency = keyDown send → getState inputValue echo (4ms poll). This is
 *     the protocol/state echo, not paint — same observation point as the
 *     enforced root-typing-lag-benchmark (p50 ≤ 20ms, p95 ≤ 50ms).
 *   - /usr/bin/sample runs WHILE keystrokes are being driven (sampling an
 *     idle window proves nothing); typing loops continue until sample exits.
 *   - Draw share = sum of top gpui::window::Window::draw subtree ticks /
 *     main-thread total ticks, raw counts AND ratio. Budget: healthy typing
 *     draw share measured ~15% (2026-07-02 opt-level fix); warn > 30%,
 *     fail > 60%.
 *   - App-side attribution: getLogs target=PERF "Search '…' took Xms".
 *
 * Safe: pre-seeded scratch HOME (never real ~/.scriptkit), protocol-only,
 * unique session name, cleanup gate (hide + close) in finally.
 *
 * NOTE: requires the window SHOWN (ledger SCREEN claim): hidden-window
 * keyDown dispatch returns dispatchScheduled=true but never completes and
 * the filter never echoes (verified 2026-07-18). Do not run in parallel
 * with other show/screen-level probes.
 */
import { Database } from "bun:sqlite";
import { mkdirSync, rmSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const repoRoot = resolve(import.meta.dir, "../..");
const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(repoRoot, "target-agent/artifacts/monkey-perf/script-kit-gpui");
const session = `monkey-perf-keystroke-${process.pid}-${Date.now().toString(36)}`;
const outDir = join(repoRoot, ".test-output", "multisurface-keystroke-latency", session);
const sandboxHome = join(outDir, "home");
const dbDir = join(sandboxHome, ".scriptkit", "db");
const SAMPLE_SECS = Number(process.env.PROBE_SAMPLE_SECS ?? "5");
const MAX_KEYS_PER_SURFACE = 220;
const MIN_KEYS_PER_SURFACE = 40;
const ECHO_TIMEOUT_MS = 3000;
const POLL_MS = 4;
const INTER_KEY_MS = 18;

type Finding = { kind: string; surface: string; note: string; severity: "fail" | "warn" };
const findings: Finding[] = [];

function seedClipboard(count: number) {
  mkdirSync(dbDir, { recursive: true });
  const db = new Database(join(dbDir, "clipboard-history.sqlite"), { create: true });
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
    for (let i = 0; i < count; i++) {
      ins.run(
        `perf-${String(i).padStart(4, "0")}`,
        `perf clip entry ${i} lorem-${i % 97} ipsum dolor sit amet consectetur`,
        now - i,
      );
    }
  });
  tx();
  db.close();
}

function percentile(values: number[], p: number): number {
  if (values.length === 0) return NaN;
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1);
  return sorted[Math.max(0, idx)];
}

/** Parse a `sample` call-tree file: main-thread total ticks + summed ticks of
 * top-most Window::draw subtrees (nested draw lines under a draw ancestor are
 * already counted by the ancestor). */
function drawShare(samplePath: string): { total: number; draw: number; ratio: number } {
  const text = readFileSync(samplePath, "utf8");
  const lines = text.split("\n");
  let total = 0;
  let draw = 0;
  let inMainThread = false;
  // Ancestor stack of {depth, isDraw} for the current call-tree walk.
  const stack: { depth: number; isDraw: boolean }[] = [];
  for (const line of lines) {
    const threadMatch = line.match(/^\s{4}(\d+)\s+Thread_\S+(.*)$/);
    if (threadMatch) {
      // sample lists the main thread first ("Thread_N: main"); stop at the
      // second thread block.
      if (total === 0) {
        total = Number(threadMatch[1]);
        inMainThread = true;
      } else {
        inMainThread = false;
      }
      stack.length = 0;
      continue;
    }
    if (!inMainThread) continue;
    const m = line.match(/^(\s+[+!:| ]*)(\d+)\s+(.*)$/);
    if (!m) continue;
    const depth = m[1].length;
    const count = Number(m[2]);
    const frame = m[3];
    while (stack.length > 0 && stack[stack.length - 1].depth >= depth) stack.pop();
    const underDraw = stack.some((s) => s.isDraw);
    const isDraw = frame.includes("Window::draw");
    if (isDraw && !underDraw) draw += count;
    stack.push({ depth, isDraw: isDraw || underDraw });
  }
  return { total, draw, ratio: total > 0 ? draw / total : NaN };
}

/** GPUI focus does not land on the filter input from `show` alone under
 * simulated dispatch (mouse-down focus transfer is the real path) — click the
 * input's center and verify focusedSemanticId, mirroring
 * root-typing-lag-benchmark's ensureFilterInputFocus. */
async function focusFilter(d: Driver, surface: string): Promise<boolean> {
  const windows: Json = await d
    .request({ type: "listAutomationWindows" }, { expect: "automationWindowListResult", timeoutMs: 5000 })
    .catch(() => ({}));
  const main = Array.isArray(windows?.windows)
    ? windows.windows.find((w: Json) => w.id === "main")
    : null;
  if (!main?.bounds) {
    findings.push({ kind: "main-bounds-unavailable", surface, note: "listAutomationWindows returned no main bounds", severity: "warn" });
    return false;
  }
  // Filter input lives at the bottom of the launcher shell; same point the
  // enforced root-typing-lag-benchmark clicks (width/2, height-90).
  const px = main.bounds.width / 2;
  const py = Math.max(1, main.bounds.height - 90);
  for (const type of ["mouseDown", "mouseUp"]) {
    await d.simulateGpuiEvent({ type, x: px, y: py }, { target: { type: "main" }, timeoutMs: 3000 }).catch(() => {});
  }
  const deadline = performance.now() + 3000;
  let focusedSamples = 0;
  let lastFocused: string | null = null;
  while (performance.now() < deadline) {
    const check: Json = await d
      .getElements({ target: { type: "main" } }, { timeoutMs: 3000 })
      .catch(() => ({}));
    lastFocused = check?.focusedSemanticId ?? null;
    const focused = typeof lastFocused === "string" && lastFocused.startsWith("input:");
    focusedSamples = focused ? focusedSamples + 1 : 0;
    if (focusedSamples >= 2) return true;
    await Bun.sleep(20);
  }
  // Not a finding by itself: clipboardHistory/emojiPicker auto-focus their
  // InputState without exposing input:* focusedSemanticId. echo-timeout
  // findings catch real focus loss.
  console.error(`[info] ${surface}: focusedSemanticId=${JSON.stringify(lastFocused)} after focus click (typing echo is the real gate)`);
  return false;
}

async function typeUntil(
  d: Driver,
  surface: string,
  query: string,
  sampleProc: ReturnType<typeof Bun.spawn> | null,
): Promise<number[]> {
  const latencies: number[] = [];
  let keys = 0;
  const sampleAlive = () => sampleProc !== null && sampleProc.exitCode === null;
  while (keys < MAX_KEYS_PER_SURFACE && (keys < MIN_KEYS_PER_SURFACE || sampleAlive())) {
    // Clear filter between rounds (not measured).
    d.setFilter("");
    const cleared = await d
      .waitFor({ type: "stateMatch", state: { inputValue: "" } }, { timeoutMs: 3000 })
      .then(() => true)
      .catch(() => false);
    if (!cleared) {
      findings.push({ kind: "filter-clear-stuck", surface, note: "inputValue would not clear between rounds", severity: "fail" });
      break;
    }
    let prefix = "";
    for (const ch of query) {
      prefix += ch;
      const t0 = performance.now();
      try {
        const dispatch = await d.simulateGpuiEvent(
          { type: "keyDown", key: ch, text: ch, modifiers: [] },
          { target: { type: "main" }, timeoutMs: ECHO_TIMEOUT_MS },
        );
        if (dispatch?.success !== true) {
          findings.push({ kind: "dispatch-rejected", surface, note: `keyDown '${ch}': ${JSON.stringify(dispatch)}`, severity: "fail" });
          keys++;
          continue;
        }
        // Poll until the state echoes the typed prefix.
        let echoed = false;
        while (performance.now() - t0 < ECHO_TIMEOUT_MS) {
          const st: Json = await d.getState({ timeoutMs: ECHO_TIMEOUT_MS });
          if (st?.inputValue === prefix) { echoed = true; break; }
          await Bun.sleep(POLL_MS);
        }
        const ms = performance.now() - t0;
        if (!echoed) {
          findings.push({ kind: "echo-timeout", surface, note: `key '${ch}' (prefix '${prefix}') no state echo within ${ECHO_TIMEOUT_MS}ms`, severity: "fail" });
        } else {
          latencies.push(ms);
        }
      } catch (e) {
        findings.push({ kind: "dispatch-error", surface, note: `keyDown '${ch}': ${e}`, severity: "fail" });
      }
      keys++;
      await Bun.sleep(INTER_KEY_MS);
    }
  }
  return latencies;
}

async function perfSearchTimes(d: Driver): Promise<number[]> {
  // SCRIPT_KIT_FILTER_PERF_LOG=1 emits: [RENDER_GET_RESULTS] filter='q' items=N results=M took=Xms
  const logs: Json = await d
    .getLogs({ limit: 500 }, { timeoutMs: 5000 })
    .catch(() => ({ entries: [] }));
  const entries: any[] = logs?.entries ?? logs?.logs ?? [];
  const out: number[] = [];
  for (const e of entries) {
    const m = String(e.message ?? "").match(/\[RENDER_GET_RESULTS\] filter='[^']+' .*took=([\d.]+)ms/);
    if (m) out.push(parseFloat(m[1]));
  }
  return out;
}

// --- run ---------------------------------------------------------------------

mkdirSync(outDir, { recursive: true });
seedClipboard(1000);

if (!existsSync(BINARY)) {
  console.error(`FATAL: binary not found at ${BINARY}`);
  process.exit(2);
}

const d = await Driver.launch({
  binary: BINARY,
  sessionName: session,
  sessionDir: join(outDir, "driver"),
  env: {
    HOME: sandboxHome,
    SK_PATH: join(sandboxHome, ".scriptkit"),
    // App-side search timing ("Search '…' took Xms") is gated behind this.
    SCRIPT_KIT_FILTER_PERF_LOG: "1",
  },
});

const surfaces: { name: string; trigger: string; query: string }[] = [
  { name: "launcher", trigger: "mainList", query: "settings" },
  { name: "clipboard-1k", trigger: "clipboardHistory", query: "lorem" },
  { name: "emoji", trigger: "emojiPicker", query: "smile" },
];

const results: Json[] = [];
try {
  await Bun.sleep(500);
  await d.waitForSettle({ timeoutMs: 8000 }).catch(() => {});
  // Real keyDown dispatch requires a shown window (SCREEN claim held):
  // hidden-window dispatch is scheduled but never completes.
  d.send({ type: "show" });
  await Bun.sleep(400);
  await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});

  for (const s of surfaces) {
    // Re-assert visibility (a prior surface's teardown may have hidden us).
    const vis: Json = await d.getState({ timeoutMs: 5000 }).catch(() => ({}));
    if (vis?.windowVisible !== true) {
      d.send({ type: "show" });
      await Bun.sleep(300);
    }
    // Open the surface (retry once — trigger can race startup load).
    // mainList renders as promptType "none" with choices; builtins report
    // their own promptType.
    let opened = false;
    for (let attempt = 0; attempt < 2 && !opened; attempt++) {
      d.send({ type: "triggerBuiltin", name: s.trigger });
      for (let i = 0; i < 10; i++) {
        await Bun.sleep(200);
        const st: Json = await d.getState({ timeoutMs: 8000 }).catch(() => ({}));
        if (
          s.trigger === "mainList"
            ? st?.promptType === "none" && (st?.choiceCount ?? 0) > 0
            : st?.promptType === s.trigger
        ) {
          opened = true;
          break;
        }
      }
    }
    if (!opened) {
      findings.push({ kind: "surface-never-opened", surface: s.name, note: `triggerBuiltin ${s.trigger} did not transition`, severity: "fail" });
      continue;
    }
    await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});
    await focusFilter(d, s.name);

    const searchBaseline = (await perfSearchTimes(d)).length;
    const samplePath = join(outDir, `sample-${s.name}.txt`);
    const sampleProc = d.pid
      ? Bun.spawn(["/usr/bin/sample", String(d.pid), String(SAMPLE_SECS), "-file", samplePath], {
          stdout: "ignore",
          stderr: "ignore",
        })
      : null;
    if (!sampleProc) {
      findings.push({ kind: "no-pid", surface: s.name, note: "driver exposed no pid; sample skipped", severity: "warn" });
    }

    const latencies = await typeUntil(d, s.name, s.query, sampleProc);
    if (sampleProc) await sampleProc.exited;

    const appSearch = (await perfSearchTimes(d)).slice(searchBaseline);
    const share = sampleProc && existsSync(samplePath) ? drawShare(samplePath) : null;

    const row: Json = {
      surface: s.name,
      keysMeasured: latencies.length,
      echo: {
        p50Ms: Number(percentile(latencies, 50).toFixed(1)),
        p95Ms: Number(percentile(latencies, 95).toFixed(1)),
        maxMs: Number(Math.max(...latencies, 0).toFixed(1)),
      },
      appSearch: {
        count: appSearch.length,
        p50Ms: appSearch.length ? Number(percentile(appSearch, 50).toFixed(1)) : null,
        p95Ms: appSearch.length ? Number(percentile(appSearch, 95).toFixed(1)) : null,
      },
      drawShare: share
        ? { drawTicks: share.draw, mainThreadTicks: share.total, ratio: Number(share.ratio.toFixed(3)) }
        : null,
      samplePath: share ? samplePath : null,
      rawLatenciesMs: latencies.map((v) => Number(v.toFixed(1))),
    };
    results.push(row);

    if (latencies.length >= 10) {
      if (row.echo.p95Ms > 50) findings.push({ kind: "echo-p95-over-budget", surface: s.name, note: `p95=${row.echo.p95Ms}ms > 50ms`, severity: "fail" });
      if (row.echo.p50Ms > 20) findings.push({ kind: "echo-p50-over-budget", surface: s.name, note: `p50=${row.echo.p50Ms}ms > 20ms`, severity: "warn" });
    } else {
      findings.push({ kind: "insufficient-samples", surface: s.name, note: `only ${latencies.length} measured keys`, severity: "fail" });
    }
    if (share && Number.isFinite(share.ratio)) {
      if (share.ratio > 0.6) findings.push({ kind: "draw-share-over-budget", surface: s.name, note: `draw share ${(share.ratio * 100).toFixed(0)}% > 60%`, severity: "fail" });
      else if (share.ratio > 0.3) findings.push({ kind: "draw-share-elevated", surface: s.name, note: `draw share ${(share.ratio * 100).toFixed(0)}% > 30%`, severity: "warn" });
    }

    // No escape between surfaces (escape on the main list hides the window);
    // the next triggerBuiltin transitions directly.
    d.setFilter("");
    await Bun.sleep(150);
  }
} finally {
  // Cleanup gate: hide, verify, close.
  try {
    d.simulateKey("escape");
    await Bun.sleep(100);
    d.send({ type: "hide" });
    await Bun.sleep(200);
    const st: Json = await d.getState({ timeoutMs: 3000 }).catch(() => ({}));
    if (st?.windowVisible === true) {
      findings.push({ kind: "window-left-visible", surface: "global", note: "windowVisible still true after hide", severity: "fail" });
    }
  } catch {}
  await d.close();
}

const receipt = {
  probe: "multisurface-keystroke-latency-probe",
  session,
  binary: BINARY,
  sampleSecs: SAMPLE_SECS,
  budgets: { echoP50Ms: 20, echoP95Ms: 50, drawShareWarn: 0.3, drawShareFail: 0.6 },
  observationPoint: "stateResult.inputValue (state echo, not paint)",
  results,
  findings,
};
writeFileSync(join(outDir, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");

for (const r of results) {
  console.error(
    `${String(r.surface).padEnd(14)} keys=${String(r.keysMeasured).padStart(3)} ` +
      `echo p50=${r.echo.p50Ms}ms p95=${r.echo.p95Ms}ms max=${r.echo.maxMs}ms ` +
      `appSearch p95=${r.appSearch.p95Ms ?? "n/a"}ms ` +
      `drawShare=${r.drawShare ? (r.drawShare.ratio * 100).toFixed(0) + "% (" + r.drawShare.drawTicks + "/" + r.drawShare.mainThreadTicks + ")" : "n/a"}`,
  );
}
const fails = findings.filter((f) => f.severity === "fail");
const warns = findings.filter((f) => f.severity === "warn");
for (const f of findings.slice(0, 40)) console.error(`  [${f.severity.toUpperCase()}] ${f.surface}: ${f.kind} — ${f.note}`);
console.error(`receipt: ${join(outDir, "receipt.json")}`);
const verdict = fails.length ? "FAIL" : warns.length ? "SUSPECT" : "PASS";
console.log(`[${verdict}] multisurface-keystroke-latency: surfaces=${results.length}/3 fails=${fails.length} warns=${warns.length}`);
process.exit(fails.length ? 1 : 0);
