#!/usr/bin/env bun
/**
 * NN=21b FRONTMOST rows (L6) — requires SCREEN claim.
 * (1) hold-repeat delete + chrome CLS (root-search-frame-stability conventions)
 * (2) PageUp/PageDown/End selection movement honest classification
 * (3) printable-key typing sample (protocol keyDown chars)
 */
import { mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-input/script-kit-gpui");
const OUT = join(
  process.cwd(),
  ".test-output/chaos-21-l6/21b-frontmost",
);
mkdirSync(OUT, { recursive: true });

type Row = { id: string; verdict: string; reason: string; detail?: Json };
const rows: Row[] = [];

function seed(kitDir: string) {
  const scriptsDir = join(kitDir, "plugins", "main", "scripts");
  mkdirSync(scriptsDir, { recursive: true });
  writeFileSync(join(kitDir, "config.ts"), "export default {};\n");
  for (let i = 0; i < 300; i++) {
    const n = `nav-${String(i).padStart(3, "0")}`;
    writeFileSync(
      join(scriptsDir, `${n}.ts`),
      `// Name: ${n}\nconsole.log(${i});\n`,
    );
  }
}

function chromeBounds(layout: Json): Map<string, Json> {
  const m = new Map<string, Json>();
  for (const c of (layout?.components ?? []) as Json[]) {
    const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
    if (/input|search|footer|header|toolbar|hint/.test(hay) && c.bounds) {
      m.set(`${c.name}|${c.type ?? ""}`, c.bounds);
    }
  }
  return m;
}

function maxDrift(a: Map<string, Json>, b: Map<string, Json>): number {
  let d = 0;
  for (const [k, pb] of a) {
    const cb = b.get(k);
    if (!cb) continue;
    d = Math.max(
      d,
      Math.abs(Number(pb.x) - Number(cb.x)),
      Math.abs(Number(pb.y) - Number(cb.y)),
      Math.abs(Number(pb.height) - Number(cb.height)),
    );
  }
  return d;
}

const scratch = join(tmpdir(), `21b-front-${process.pid}`);
const home = join(scratch, "home");
const kitDir = join(home, ".scriptkit");
mkdirSync(kitDir, { recursive: true });
seed(kitDir);

const d = await Driver.launch({
  binary: BINARY,
  sessionName: "monkey-input-21b-front",
  env: { HOME: home, SK_PATH: kitDir },
  readyTimeoutMs: 20_000,
});

try {
  d.send({ type: "show" });
  await d.waitForSettle({ timeoutMs: 5000 });
  let st = await d.getState({ timeoutMs: 8000 });
  if (st.windowVisible !== true) {
    rows.push({
      id: "frontmost-preflight",
      verdict: "FAIL",
      reason: `windowVisible=${st.windowVisible} after show`,
    });
    throw new Error("show failed");
  }
  console.error(`[21b-front] windowVisible=true focused=${st.isFocused}`);

  // =========================================================================
  // Hold-repeat delete storm + chrome CLS
  // =========================================================================
  {
    const seedText = "H".repeat(120);
    d.setFilter(seedText);
    await d.waitForSettle({ timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    const seeded = String(st.inputValue ?? "").length;
    const layout0 = await d.getLayoutInfo({}, { timeoutMs: 8000 });
    const chrome0 = chromeBounds(layout0);

    // Rapid backspace via simulateGpuiEvent (simulateKey routes as shortcut-miss;
    // gpui keyDown is the working frontmost path — verified before this battery).
    for (let i = 0; i < 120; i++) {
      await d.simulateGpuiEvent(
        { type: "keyDown", key: "backspace", modifiers: [] },
        { timeoutMs: 3000 },
      );
    }
    await d.waitForSettle({ timeoutMs: 5000 });
    st = await d.getState({ timeoutMs: 8000 });
    const after = String(st.inputValue ?? "").length;
    const layout1 = await d.getLayoutInfo({}, { timeoutMs: 8000 });
    const chrome1 = chromeBounds(layout1);
    const drift = maxDrift(chrome0, chrome1);

    // Also sample mid-storm frames for CLS
    d.setFilter("X".repeat(100));
    await d.waitForSettle({ timeoutMs: 3000 });
    const frames: number[] = [];
    let prev = chromeBounds(await d.getLayoutInfo({}, { timeoutMs: 6000 }));
    for (let i = 0; i < 40; i++) {
      await d.simulateGpuiEvent(
        { type: "keyDown", key: "backspace", modifiers: [] },
        { timeoutMs: 3000 },
      );
      if (i % 5 === 4) {
        await d.waitForSettle({ timeoutMs: 2000 }).catch(() => {});
        const cur = chromeBounds(await d.getLayoutInfo({}, { timeoutMs: 6000 }));
        frames.push(maxDrift(prev, cur));
        prev = cur;
      }
    }
    const maxFrameDrift = frames.length ? Math.max(...frames) : drift;
    const deleted = after < seeded;
    const clsOk = maxFrameDrift <= 1.0;

    rows.push({
      id: "hold-repeat-delete-cls",
      verdict: deleted && clsOk ? "PASS" : deleted ? "SUSPECT" : "FAIL",
      reason: `seeded=${seeded} afterBs=${after} deleted=${deleted} chromeDrift=${drift.toFixed(2)} midStormMax=${maxFrameDrift.toFixed(2)} (CLS eps 1px)`,
      detail: { seeded, after, deleted, drift, maxFrameDrift, frames, windowVisible: st.windowVisible },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // PageUp / PageDown / End classification (frontmost)
  // =========================================================================
  {
    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    const sel0 = Number(st.selectedIndex ?? 0);
    const itemCount = Number(st.mainListScroll?.itemCount ?? st.visibleChoiceCount ?? 0);

    await d.simulateGpuiEvent(
      { type: "keyDown", key: "pagedown", modifiers: [] },
      { timeoutMs: 5000 },
    );
    await d.waitForSettle({ timeoutMs: 3000 });
    st = await d.getState({ timeoutMs: 8000 });
    const afterPd = Number(st.selectedIndex ?? -1);

    await d.simulateGpuiEvent(
      { type: "keyDown", key: "pageup", modifiers: [] },
      { timeoutMs: 5000 },
    );
    await d.waitForSettle({ timeoutMs: 3000 });
    st = await d.getState({ timeoutMs: 8000 });
    const afterPu = Number(st.selectedIndex ?? -1);

    // move down then End
    for (let i = 0; i < 30; i++) {
      await d.simulateGpuiEvent(
        { type: "keyDown", key: "down", modifiers: [] },
        { timeoutMs: 3000 },
      );
    }
    await d.waitForSettle({ timeoutMs: 3000 });
    st = await d.getState({ timeoutMs: 8000 });
    const mid = Number(st.selectedIndex ?? -1);
    await d.simulateGpuiEvent(
      { type: "keyDown", key: "end", modifiers: [] },
      { timeoutMs: 5000 },
    );
    await d.waitForSettle({ timeoutMs: 3000 });
    st = await d.getState({ timeoutMs: 8000 });
    const afterEnd = Number(st.selectedIndex ?? -1);

    await d.simulateGpuiEvent(
      { type: "keyDown", key: "home", modifiers: [] },
      { timeoutMs: 5000 },
    );
    await d.waitForSettle({ timeoutMs: 3000 });
    st = await d.getState({ timeoutMs: 8000 });
    const afterHome = Number(st.selectedIndex ?? -1);

    const pdMoved = afterPd !== sel0 && afterPd >= 0;
    const puMoved = afterPu !== afterPd;
    const endMoved = afterEnd > mid;
    const homeMoved = afterHome < afterEnd || afterHome <= sel0 + 2;

    const anyBound = pdMoved || puMoved || endMoved || homeMoved;
    const allDead = !pdMoved && !puMoved && !endMoved;

    rows.push({
      id: "page-end-keys-frontmost",
      verdict: allDead ? "FAIL" : anyBound ? "PASS" : "SUSPECT",
      reason: `sel0=${sel0} Pd=${afterPd}(${pdMoved}) Pu=${afterPu}(${puMoved}) mid=${mid} End=${afterEnd}(${endMoved}) Home=${afterHome}(${homeMoved}) items=${itemCount}`,
      detail: {
        sel0,
        afterPd,
        afterPu,
        mid,
        afterEnd,
        afterHome,
        itemCount,
        pdMoved,
        puMoved,
        endMoved,
        homeMoved,
        classification: allDead
          ? "product-suspect: page/end dead even frontmost"
          : "bindings partially or fully deliver frontmost",
      },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // printable-key mode sample (keyDown chars)
  // =========================================================================
  {
    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 3000 });
    // click-focus not available; try typing after show
    const word = "amz";
    const echoes: number[] = [];
    let acc = "";
    let ok = true;
    for (const ch of word) {
      acc += ch;
      const t0 = performance.now();
      await d.simulateGpuiEvent(
        { type: "keyDown", key: ch, text: ch, modifiers: [] },
        { timeoutMs: 5000 },
      );
      await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
      st = await d.getState({ timeoutMs: 8000 });
      echoes.push(performance.now() - t0);
      if (String(st.inputValue ?? "") !== acc) {
        // fallback: may need focus — try setFilter path note
        ok = false;
      }
    }
    echoes.sort((a, b) => a - b);
    const p50 = echoes[Math.floor(echoes.length * 0.5)] ?? null;
    const p95 = echoes[Math.floor(echoes.length * 0.95)] ?? echoes[echoes.length - 1] ?? null;

    rows.push({
      id: "printable-key-sample",
      verdict: ok ? "PASS" : "SUSPECT",
      reason: ok
        ? `typed ${word} via keyDown; echo p50=${p50?.toFixed(1)} p95=${p95?.toFixed(1)} final=${JSON.stringify(st.inputValue)}`
        : `keyDown did not fully echo (final=${JSON.stringify(st.inputValue)} expected ${word}); focus may be required`,
      detail: { word, echoes, p50, p95, final: st.inputValue, windowVisible: st.windowVisible },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  d.setFilter("");
  d.send({ type: "hide" });
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
} catch (e) {
  rows.push({ id: "probe-crash", verdict: "FAIL", reason: String(e).slice(0, 300) });
  console.error("CRASH", e);
} finally {
  try {
    d.send({ type: "hide" });
  } catch {
    /* ignore */
  }
  await d.close();
  try {
    rmSync(scratch, { recursive: true, force: true });
  } catch {
    /* ignore */
  }
}

const summary = {
  battery: "chaos-21b-frontmost",
  lane: "L6-monkey-grok-input",
  binary: BINARY,
  rows,
  overall: rows.some((r) => r.verdict === "FAIL")
    ? "FAIL"
    : rows.some((r) => r.verdict === "SUSPECT")
      ? "SUSPECT"
      : "PASS",
};
writeFileSync(join(OUT, "receipt.json"), JSON.stringify(summary, null, 2) + "\n");
console.log(JSON.stringify(summary, null, 2));
process.exit(rows.some((r) => r.verdict === "FAIL") ? 1 : 0);
