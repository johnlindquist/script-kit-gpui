#!/usr/bin/env bun
/**
 * L4 monkey-perf chaos-17: scroll-perf battery for launcher long-list and
 * clipboardHistory (pre-seeded 1,000 entries). Agent-chat transcript rows are
 * covered by the existing gates (agent-chat-short-scroll-probe.ts and
 * agent-chat-heavy-markdown-scroll-proof.ts) — run those separately with the
 * same binary; this probe owns the two launcher-window surfaces.
 *
 * Wheel modes (env PROBE_WHEEL_MODE):
 *   - "gpui" (default): driver.simulateGpuiScrollWheel — REAL GPUI dispatch
 *     (PlatformInput::ScrollWheel), works on a HIDDEN window; no SCREEN claim
 *     needed. Correct field shape is REQUIRED: {x, y, deltaX, deltaY,
 *     phase: "started"|"moved"|"ended"} — `deltaX` is non-optional and phase
 *     is SimulatedTouchPhase; malformed payloads (e.g. phase "began", missing
 *     deltaX) are dropped with NO error response and the request times out
 *     (papercut, receipt 2026-07-18).
 *   - "cgevent": compiled CGEvent helper (env PROBE_SCROLL_HELPER, args
 *     <screenX> <screenY> <seconds> [pixelsPerTick]) posting real HID wheel
 *     events — needs the window SHOWN, FRONTMOST, cursor over it (ledger
 *     SCREEN claim), and NO other script-kit-gpui window on screen (all
 *     sandboxed instances share the default frame; a foreign topmost window
 *     silently swallows the stream — contention receipt 2026-07-18). Helper
 *     must post changed-phase pixel events only.
 *
 * Method (flows/perf.md): /usr/bin/sample runs WHILE the wheel stream is
 * live; draw share = top-most Window::draw subtree ticks / main-thread
 * ticks (warn > 30%, fail > 60%; healthy scroll reference ~14.5%).
 * Delivery is proven by observables (settle is never proof):
 *   - launcher: mainListScroll.scrollTop delta
 *   - clipboard: first VISIBLE choice index shift (its list has no
 *     mainListScroll field)
 */
import { Database } from "bun:sqlite";
import { mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const repoRoot = resolve(import.meta.dir, "../..");
const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(repoRoot, "target-agent/artifacts/monkey-perf/script-kit-gpui");
const MODE = process.env.PROBE_WHEEL_MODE ?? "gpui";
const HELPER = process.env.PROBE_SCROLL_HELPER ?? "";
const SCROLL_SECS = Number(process.env.PROBE_SCROLL_SECONDS ?? "5");
const session = `monkey-perf-scroll-${MODE}-${process.pid}-${Date.now().toString(36)}`;
const outDir = join(repoRoot, ".test-output", "multisurface-scroll-perf", session);
const sandboxHome = join(outDir, "home");

type Finding = { kind: string; surface: string; note: string; severity: "fail" | "warn" };
const findings: Finding[] = [];

function seedClipboard(count: number) {
  const dbDir = join(sandboxHome, ".scriptkit", "db");
  mkdirSync(dbDir, { recursive: true });
  const db = new Database(join(dbDir, "clipboard-history.sqlite"), { create: true });
  db.exec(`CREATE TABLE IF NOT EXISTS history (
    id TEXT PRIMARY KEY, content TEXT NOT NULL, content_hash TEXT,
    content_type TEXT NOT NULL DEFAULT 'text', timestamp INTEGER NOT NULL,
    pinned INTEGER DEFAULT 0, ocr_text TEXT)`);
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

/** First thread block = main thread; sum top-most Window::draw subtrees. */
function drawShare(samplePath: string): { total: number; draw: number; ratio: number } {
  const lines = readFileSync(samplePath, "utf8").split("\n");
  let total = 0;
  let draw = 0;
  let inMain = false;
  let seen = 0;
  const stack: { depth: number; isDraw: boolean }[] = [];
  for (const line of lines) {
    const tm = line.match(/^\s{4}(\d+)\s+Thread_\S+(.*)$/);
    if (tm) {
      seen++;
      inMain = seen === 1;
      if (inMain) total = Number(tm[1]);
      stack.length = 0;
      continue;
    }
    if (!inMain) continue;
    const m = line.match(/^(\s+[+!:| ]*)(\d+)\s+(.*)$/);
    if (!m) continue;
    const depth = m[1].length;
    const count = Number(m[2]);
    const frame = m[3];
    while (stack.length > 0 && stack[stack.length - 1].depth >= depth) stack.pop();
    const under = stack.some((s) => s.isDraw);
    const isDraw = frame.includes("Window::draw");
    if (isDraw && !under) draw += count;
    stack.push({ depth, isDraw: isDraw || under });
  }
  return { total, draw, ratio: total > 0 ? draw / total : NaN };
}

/** Wheel target = the LIST component's center. The clipboard surface is a
 * split view (ScriptList x 0-375, PreviewPanel x 375-750): window center
 * lands on the preview and scrolls nothing (receipt 2026-07-18). */
async function listCenter(d: Driver, fallbackX: number, fallbackY: number): Promise<{ x: number; y: number }> {
  const li: Json = await d
    .getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 8000 })
    .catch(() => ({}));
  const list = (li?.components ?? []).find((c: Json) => c.type === "list" && c.bounds);
  if (!list) return { x: fallbackX, y: fallbackY };
  return {
    x: list.bounds.x + list.bounds.width / 2,
    y: list.bounds.y + list.bounds.height / 2,
  };
}

async function firstVisibleChoiceIndex(d: Driver): Promise<number | null> {
  const els: Json = await d
    .getElements({ target: { type: "main" }, limit: 10 }, { timeoutMs: 5000 })
    .catch(() => ({}));
  const choices = (els?.elements ?? []).filter((e: Json) => e.type === "choice");
  if (!choices.length) return null;
  return Math.min(...choices.map((c: Json) => c.index ?? 0));
}

/** cgevent mode only: a foreign on-screen script-kit-gpui window swallows the
 * HID wheel stream. */
function foreignWindowsOnScreen(ownPid: number | undefined): Json[] {
  const q = Bun.spawnSync({
    cmd: ["swift", join(repoRoot, "scripts/agentic/macos-window-query.swift")],
    stdout: "pipe",
    stderr: "pipe",
  });
  try {
    const parsed = JSON.parse(q.stdout.toString());
    return (parsed.windows ?? []).filter(
      (w: Json) => w.onscreen === true && w.ownerPid !== ownPid,
    );
  } catch {
    return [];
  }
}

// --- run ---------------------------------------------------------------------

if (!["gpui", "cgevent"].includes(MODE)) {
  console.error(`FATAL: unknown PROBE_WHEEL_MODE '${MODE}'`);
  process.exit(2);
}
if (MODE === "cgevent" && (!HELPER || !existsSync(HELPER))) {
  console.error(`FATAL: cgevent mode needs PROBE_SCROLL_HELPER (got ${HELPER || "unset"})`);
  process.exit(2);
}
if (!existsSync(BINARY)) {
  console.error(`FATAL: binary not found at ${BINARY}`);
  process.exit(2);
}
mkdirSync(outDir, { recursive: true });
seedClipboard(1000);

const d = await Driver.launch({
  binary: BINARY,
  sessionName: session,
  sessionDir: join(outDir, "driver"),
  env: { HOME: sandboxHome, SK_PATH: join(sandboxHome, ".scriptkit") },
});

const surfaces: { name: string; trigger: string }[] = [
  { name: "launcher-long-list", trigger: "mainList" },
  { name: "clipboard-1k", trigger: "clipboardHistory" },
];
const results: Json[] = [];

try {
  await Bun.sleep(600);
  await d.waitForSettle({ timeoutMs: 8000 }).catch(() => {});

  let sx = 0;
  let sy = 0;
  const windows: Json = await d.request(
    { type: "listAutomationWindows" },
    { expect: "automationWindowListResult", timeoutMs: 5000 },
  );
  const main = Array.isArray(windows?.windows)
    ? windows.windows.find((w: Json) => w.id === "main")
    : null;
  if (!main?.bounds) throw new Error("main window bounds unavailable");
  const localX = main.bounds.width / 2;
  const localY = main.bounds.height / 2;

  if (MODE === "cgevent") {
    // Screen coordinates + frontmost + exclusivity requirements.
    d.send({ type: "show" });
    await Bun.sleep(500);
    await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});
    const foreign = foreignWindowsOnScreen(d.pid);
    if (foreign.length > 0) {
      findings.push({ kind: "screen-contention", surface: "global", note: `foreign script-kit-gpui windows on screen (pids ${[...new Set(foreign.map((w: Json) => w.ownerPid))].join(",")}) — wheel delivery would be swallowed; aborting battery`, severity: "fail" });
      throw new Error("screen contention: foreign script-kit-gpui window on screen");
    }
    sx = main.bounds.x + localX;
    sy = main.bounds.y + localY;
  }

  for (const s of surfaces) {
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

    const wheelPoint = await listCenter(d, localX, localY);
    const errBaseline = await d
      .getLogs({ limit: 300, level: "error" }, { timeoutMs: 5000 })
      .then((l: Json) => (l?.entries ?? []).length)
      .catch(() => 0);
    const before: Json = await d.getState({ timeoutMs: 5000 }).catch(() => ({}));
    const scrollBefore = before?.mainListScroll?.scrollTop ?? 0;
    const selectedBefore = before?.selectedIndex ?? 0;
    const firstChoiceBefore = await firstVisibleChoiceIndex(d);

    const samplePath = join(outDir, `sample-${s.name}.txt`);
    const sampler = Bun.spawn(
      ["/usr/bin/sample", String(d.pid), String(SCROLL_SECS), "-file", samplePath],
      { stdout: "ignore", stderr: "ignore" },
    );

    let wheelEvents = 0;
    let helperMeta: Json = null;
    if (MODE === "cgevent") {
      const scroller = Bun.spawnSync({
        cmd: [HELPER, String(main.bounds.x + wheelPoint.x), String(main.bounds.y + wheelPoint.y), String(SCROLL_SECS)],
        stdout: "pipe",
        stderr: "pipe",
      });
      wheelEvents = Number((scroller.stdout.toString().match(/posted (\d+)/) ?? [])[1] ?? 0);
      helperMeta = { exitCode: scroller.exitCode, stderr: scroller.stderr.toString().trim() || null };
      if (scroller.exitCode !== 0) {
        findings.push({ kind: "helper-failed", surface: s.name, note: `exit=${scroller.exitCode} ${helperMeta.stderr ?? ""}`, severity: "fail" });
      }
    } else {
      // GPUI dispatch stream at ~16ms cadence while the sampler runs.
      const deadline = performance.now() + SCROLL_SECS * 1000;
      await d
        .simulateGpuiScrollWheel(
          { x: wheelPoint.x, y: wheelPoint.y, deltaX: 0, deltaY: -40, phase: "started" },
          { target: { type: "main" }, timeoutMs: 3000 },
        )
        .catch(() => {});
      wheelEvents++;
      while (performance.now() < deadline) {
        await d
          .simulateGpuiScrollWheel(
            { x: wheelPoint.x, y: wheelPoint.y, deltaX: 0, deltaY: -40, phase: "moved" },
            { target: { type: "main" }, timeoutMs: 3000 },
          )
          .catch(() => {});
        wheelEvents++;
        await Bun.sleep(16);
      }
      await d
        .simulateGpuiScrollWheel(
          { x: wheelPoint.x, y: wheelPoint.y, deltaX: 0, deltaY: 0, phase: "ended" },
          { target: { type: "main" }, timeoutMs: 3000 },
        )
        .catch(() => {});
    }
    await sampler.exited;
    await Bun.sleep(300);

    const after: Json = await d.getState({ timeoutMs: 8000 }).catch(() => ({}));
    const scrollAfter = after?.mainListScroll?.scrollTop ?? 0;
    const selectedAfter = after?.selectedIndex ?? 0;
    const firstChoiceAfter = await firstVisibleChoiceIndex(d);
    const share = existsSync(samplePath) ? drawShare(samplePath) : null;
    const errNow = await d
      .getLogs({ limit: 300, level: "error" }, { timeoutMs: 5000 })
      .then((l: Json) => (l?.entries ?? []).length)
      .catch(() => errBaseline);

    const row: Json = {
      surface: s.name,
      mode: MODE,
      wheelPoint,
      itemCount:
        s.trigger === "mainList"
          ? after?.mainListScroll?.itemCount ?? null
          : after?.visibleChoiceCount ?? after?.choiceCount ?? null,
      wheelEvents,
      scrollDeliveredPx: scrollAfter - scrollBefore,
      selectedIndexDelta: selectedAfter - selectedBefore,
      firstVisibleChoice: { before: firstChoiceBefore, after: firstChoiceAfter },
      drawShare: share
        ? { drawTicks: share.draw, mainThreadTicks: share.total, ratio: Number(share.ratio.toFixed(3)) }
        : null,
      drawTicksPerWheelEvent: share && wheelEvents > 0 ? Number((share.draw / wheelEvents).toFixed(2)) : null,
      newErrorLogs: Math.max(0, errNow - errBaseline),
      helper: helperMeta,
      samplePath,
    };
    results.push(row);

    // Delivery proof per surface observable (settle is never proof).
    // Launcher: scrollTop. Clipboard: its list has no scroll field and
    // getElements/getLayoutInfo enumerate from the data head regardless of
    // scroll — but wheel moves selectedIndex (receipt: 40 events -> sel 0->63).
    const delivered =
      row.scrollDeliveredPx > 0 ||
      row.selectedIndexDelta > 0 ||
      (firstChoiceBefore != null && firstChoiceAfter != null && firstChoiceAfter > firstChoiceBefore);
    if (!delivered) {
      findings.push({ kind: "wheel-not-delivered", surface: s.name, note: `scrollTop ${scrollBefore} -> ${scrollAfter}, selectedIndex ${selectedBefore} -> ${selectedAfter}, firstVisibleChoice ${firstChoiceBefore} -> ${firstChoiceAfter} after ${wheelEvents} events (row invalid, not green)`, severity: "fail" });
    }
    if (share && Number.isFinite(share.ratio)) {
      if (share.ratio > 0.6) findings.push({ kind: "draw-share-over-budget", surface: s.name, note: `draw share ${(share.ratio * 100).toFixed(0)}% > 60%`, severity: "fail" });
      else if (share.ratio > 0.3) findings.push({ kind: "draw-share-elevated", surface: s.name, note: `draw share ${(share.ratio * 100).toFixed(0)}% > 30%`, severity: "warn" });
    }
    if (row.newErrorLogs > 0) {
      findings.push({ kind: "new-error-logs", surface: s.name, note: `${row.newErrorLogs} new error-level log entries during scroll`, severity: "warn" });
    }
    d.simulateKey("home");
    await Bun.sleep(200);
  }
} finally {
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
  probe: "multisurface-scroll-perf-probe",
  session,
  mode: MODE,
  binary: BINARY,
  helper: MODE === "cgevent" ? HELPER : null,
  scrollSecs: SCROLL_SECS,
  budgets: { drawShareWarn: 0.3, drawShareFail: 0.6 },
  results,
  findings,
};
writeFileSync(join(outDir, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");

for (const r of results) {
  console.error(
    `${String(r.surface).padEnd(20)} mode=${r.mode} items=${r.itemCount} wheel=${r.wheelEvents} delivered=${r.scrollDeliveredPx}px selDelta=${r.selectedIndexDelta} ` +
      `firstChoice ${r.firstVisibleChoice.before}->${r.firstVisibleChoice.after} ` +
      `drawShare=${r.drawShare ? (r.drawShare.ratio * 100).toFixed(0) + "% (" + r.drawShare.drawTicks + "/" + r.drawShare.mainThreadTicks + ")" : "n/a"} ` +
      `drawTicks/wheel=${r.drawTicksPerWheelEvent ?? "n/a"} newErrors=${r.newErrorLogs}`,
  );
}
const fails = findings.filter((f) => f.severity === "fail");
const warns = findings.filter((f) => f.severity === "warn");
for (const f of findings.slice(0, 30)) console.error(`  [${f.severity.toUpperCase()}] ${f.surface}: ${f.kind} — ${f.note}`);
console.error(`receipt: ${join(outDir, "receipt.json")}`);
console.log(`[${fails.length ? "FAIL" : warns.length ? "SUSPECT" : "PASS"}] multisurface-scroll-perf(${MODE}): surfaces=${results.length}/2 fails=${fails.length} warns=${warns.length}`);
process.exit(fails.length ? 1 : 0);
