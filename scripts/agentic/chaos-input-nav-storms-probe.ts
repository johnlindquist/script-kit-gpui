#!/usr/bin/env bun
/**
 * Chaos-21 L6 (monkey-grok-input): input/nav storms.
 * Grow one judged row at a time; run after each add.
 * Driver APIs only as used in chaos-cls-perf-probe.ts / chaos-interaction-stress.ts
 * and scripts/devtools/driver.ts (setFilter, setFilterAndWait, waitForSettle,
 * getState, simulateKey, getLogs, getLayoutInfo, close).
 */
import { mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-input/script-kit-gpui");

const CYCLES = Number(process.env.CHAOS_INPUT_NAV_CYCLES ?? "30");
const SEED_COUNT = Number(process.env.CHAOS_INPUT_NAV_SEED_COUNT ?? "1000");
const DIR_DEPTH = Number(process.env.CHAOS_INPUT_NAV_DIR_DEPTH ?? "10");
const DIR_ENTRIES = Number(process.env.CHAOS_INPUT_NAV_DIR_ENTRIES ?? "40");
const ROW_ONLY = process.env.CHAOS_INPUT_NAV_ROW_ONLY ?? ""; // e.g. "3" to run only row 3 after smoke

type Row = {
  id: string;
  verdict: "PASS" | "SUSPECT" | "FAIL" | "ENV";
  reason: string;
  detail?: Json;
};
const rows: Row[] = [];

function isNoise(msg: string): boolean {
  return (
    /captureScreenshot|automation window|screenshot/i.test(msg) ||
    (/RefCell already borrowed/i.test(msg) && /vendor\/gpui\/src\/window\.rs/i.test(msg)) ||
    (/window not found/i.test(msg) && /vendor\/gpui\/src\/window\.rs/i.test(msg))
  );
}

async function errorKeys(d: Driver): Promise<Set<string>> {
  try {
    const r = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
    const es: any[] = (r as any).entries ?? (r as any).logs ?? [];
    return new Set(
      es
        .map((e) => `${e.target ?? ""}|${e.message ?? ""}`)
        .filter((k) => !isNoise(k)),
    );
  } catch {
    return new Set();
  }
}

async function warnKeys(d: Driver): Promise<string[]> {
  try {
    const r = await d.getLogs({ level: "warn", limit: 400 }, { timeoutMs: 5000 });
    const es: any[] = (r as any).entries ?? (r as any).logs ?? [];
    return es.map((e) => `${e.target ?? ""}|${e.message ?? ""}`);
  } catch {
    return [];
  }
}

function selectionOk(state: Json): boolean {
  const vis = Number(state.visibleChoiceCount ?? 0);
  const sel = state.selectedIndex ?? state.selectedRow ?? null;
  if (sel == null) return true;
  const n = Number(sel);
  if (!Number.isFinite(n) || n < 0) return false;
  if (vis > 0 && n >= vis) return false;
  return true;
}

function filterText(state: Json): string {
  const diag = state?.filterInputDiagnostics;
  if (diag && typeof diag.canonicalFilterText === "string") return diag.canonicalFilterText;
  return String(state?.inputValue ?? "");
}

function seedScripts(kitDir: string, count: number) {
  const scriptsDir = join(kitDir, "plugins", "main", "scripts");
  mkdirSync(scriptsDir, { recursive: true });
  writeFileSync(join(kitDir, "config.ts"), "export default {};\n");
  for (let i = 0; i < count; i++) {
    const name = `chaos-nav-${String(i).padStart(4, "0")}`;
    writeFileSync(
      join(scriptsDir, `${name}.ts`),
      `// Name: ${name}\n// Description: chaos input nav seed ${i}\nconsole.log(${i});\n`,
    );
  }
  for (const label of ["test-alpha", "test-beta", "script-gamma", "script-delta"]) {
    writeFileSync(
      join(scriptsDir, `${label}.ts`),
      `// Name: ${label}\nconsole.log("${label}");\n`,
    );
  }
}

function seedDeepTree(root: string, depth: number, entries: number): string {
  let cur = root;
  mkdirSync(cur, { recursive: true });
  for (let d = 0; d < depth; d++) {
    for (let i = 0; i < entries; i++) {
      writeFileSync(join(cur, `file-d${d}-e${String(i).padStart(3, "0")}.txt`), `d=${d} e=${i}\n`);
    }
    const next = join(cur, `level-${d}`);
    mkdirSync(next, { recursive: true });
    cur = next;
  }
  writeFileSync(join(cur, "leaf.txt"), "deep-leaf\n");
  return cur;
}

const scratch = join(tmpdir(), `chaos-input-nav-${process.pid}-${Date.now().toString(36)}`);
const home = join(scratch, "home");
const kitDir = join(home, ".scriptkit");
mkdirSync(kitDir, { recursive: true });
seedScripts(kitDir, SEED_COUNT);
const deepRoot = join(home, "deep-fixture");
const deepLeaf = seedDeepTree(deepRoot, DIR_DEPTH, DIR_ENTRIES);

console.error(`[chaos-21] binary=${BINARY} seed=${SEED_COUNT} deep=${DIR_DEPTH}x${DIR_ENTRIES}`);

const d = await Driver.launch({
  binary: BINARY,
  sessionName: "monkey-input-nav",
  // Pre-seeded HOME (scripts present at menu-cache startup). Same pattern as
  // chaos-frecency-input-history-probe.ts — not sandboxHome:true empty home.
  env: { HOME: home, SK_PATH: kitDir },
  readyTimeoutMs: 20_000,
  defaultTimeoutMs: 8_000,
});

const want = (n: number) => !ROW_ONLY || ROW_ONLY.split(",").includes(String(n));

try {
  await d.getState({ timeoutMs: 8000 });
  d.setFilter("");
  await d.waitForSettle({ timeoutMs: 4000 });

  // =========================================================================
  // Row 1: Rapid type/delete alternation
  // =========================================================================
  if (want(1)) {
    const before = await errorKeys(d);
    let textMismatch = 0;
    let selBad = 0;
    const receipts: Json[] = [];
    const t0 = performance.now();

    for (let c = 0; c < CYCLES; c++) {
      const t5 = c % 2 === 0 ? "zzzzq" : "scrip";
      const afterDel = t5.slice(0, 2);
      const t4 = c % 2 === 0 ? "xyzw" : "test";
      const mid = afterDel + t4;
      d.setFilter(t5);
      d.setFilter(afterDel);
      d.setFilter(mid);
      d.setFilter("");
      await d.waitForSettle({ timeoutMs: 4000 });
      const st = await d.getState({ timeoutMs: 8000 });
      if (String(st.inputValue ?? "") !== "") {
        textMismatch++;
        if (textMismatch <= 3) receipts.push({ cycle: c, got: st.inputValue });
      }
      if (!selectionOk(st)) {
        selBad++;
        receipts.push({ cycle: c, sel: st.selectedIndex, vis: st.visibleChoiceCount });
      }
      if (c % 10 === 0) {
        receipts.push({
          cycle: c,
          input: st.inputValue,
          vis: st.visibleChoiceCount,
          sel: st.selectedIndex,
        });
      }
    }

    await d.setFilterAndWait("alpha", { timeoutMs: 4000 });
    let st = await d.getState({ timeoutMs: 8000 });
    if (String(st.inputValue ?? "") !== "alpha") textMismatch++;
    await d.setFilterAndWait("al", { timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    if (String(st.inputValue ?? "") !== "al") textMismatch++;
    await d.setFilterAndWait("albeta", { timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    if (String(st.inputValue ?? "") !== "albeta") textMismatch++;
    await d.setFilterAndWait("", { timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    if (String(st.inputValue ?? "") !== "") textMismatch++;

    const after = await errorKeys(d);
    const newErrs = [...after].filter((k) => !before.has(k)).slice(0, 6);
    const elapsedMs = Math.round(performance.now() - t0);
    const ok = textMismatch === 0 && selBad === 0 && newErrs.length === 0;
    rows.push({
      id: "rapid-type-delete-alternation",
      verdict: ok ? "PASS" : "FAIL",
      reason: ok
        ? `${CYCLES} cycles exact text+sel; ${elapsedMs}ms`
        : `mismatches=${textMismatch} selBad=${selBad} newErrs=${newErrs.length}`,
      detail: { cycles: CYCLES, textMismatch, selBad, newErrs, elapsedMs, receipts },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // Row 2: Sigil grammar rapid switching
  // =========================================================================
  if (want(2)) {
    const before = await errorKeys(d);
    const cases: { input: string; expectStickyForbidden: boolean; label: string }[] = [
      { input: "plain fuzzy search", expectStickyForbidden: true, label: "plain" },
      { input: "+todo Buy milk", expectStickyForbidden: false, label: "capture-plus" },
      { input: ":type:script", expectStickyForbidden: false, label: "refine-colon" },
      { input: "+todo #work body", expectStickyForbidden: false, label: "capture-tag" },
      { input: "!ps", expectStickyForbidden: false, label: "run-bang" },
      { input: "/review", expectStickyForbidden: true, label: "slash" },
      { input: "@current", expectStickyForbidden: true, label: "at" },
      { input: "C# tutorial", expectStickyForbidden: true, label: "plain-hash" },
      { input: "Decision: ship", expectStickyForbidden: true, label: "plain-colon" },
      { input: "not-a-target: stuff", expectStickyForbidden: true, label: "unknown-colon" },
    ];
    const transitions: Json[] = [];
    let textBad = 0;
    let sticky = 0;

    for (const c of cases) {
      d.setFilter(c.input);
      await d.waitForSettle({ timeoutMs: 4000 });
      const st = await d.getState({ timeoutMs: 8000 });
      const got = String(st.inputValue ?? "");
      const kind =
        st.menuSyntaxMainHint && typeof st.menuSyntaxMainHint.kind === "string"
          ? st.menuSyntaxMainHint.kind
          : null;
      if (got !== c.input) {
        textBad++;
        transitions.push({ label: c.label, expected: c.input, got, fail: "text" });
      } else if (
        c.expectStickyForbidden &&
        kind &&
        (String(kind).startsWith("Capture") || String(kind).startsWith("Command"))
      ) {
        sticky++;
        transitions.push({ label: c.label, input: c.input, kind, fail: "sticky" });
      } else {
        transitions.push({ label: c.label, input: c.input, got, kind, vis: st.visibleChoiceCount });
      }
    }

    for (const s of ["hello", "hel+lo", "hello", "+hello", "hello", "!x", "x", ":type:", "type:"]) {
      d.setFilter(s);
      await d.waitForSettle({ timeoutMs: 4000 });
      const st = await d.getState({ timeoutMs: 8000 });
      const got = String(st.inputValue ?? "");
      const kind =
        st.menuSyntaxMainHint && typeof st.menuSyntaxMainHint.kind === "string"
          ? st.menuSyntaxMainHint.kind
          : null;
      if (got !== s) textBad++;
      if (
        (s === "hello" || s === "hel+lo" || s === "x" || s === "type:") &&
        kind &&
        (String(kind).startsWith("Capture") || String(kind).startsWith("Command"))
      ) {
        sticky++;
      }
      transitions.push({ mid: true, input: s, got, kind });
    }

    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 4000 });
    const after = await errorKeys(d);
    const newErrs = [...after].filter((k) => !before.has(k)).slice(0, 6);
    const ok = textBad === 0 && sticky === 0 && newErrs.length === 0;
    rows.push({
      id: "sigil-grammar-rapid-switch",
      verdict: ok ? "PASS" : textBad || sticky ? "FAIL" : "SUSPECT",
      reason: ok
        ? `all switches text-exact; sticky=0; newErrs=0`
        : `textBad=${textBad} sticky=${sticky} newErrs=${newErrs.length}`,
      detail: { transitions, textBad, sticky, newErrs },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // Row 3: Deep navigation (1k list + dir-browse tree)
  // =========================================================================
  if (want(3)) {
    const before = await errorKeys(d);
    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 4000 });
    let st = await d.getState({ timeoutMs: 8000 });
    const sel0 = Number(st.selectedIndex ?? 0);
    const vis0 = Number(st.visibleChoiceCount ?? 0);
    const itemCount = Number(
      st?.mainListScroll?.itemCount ?? st?.choiceCount ?? st?.visibleChoiceCount ?? 0,
    );

    // Down × 200
    for (let i = 0; i < 200; i++) d.simulateKey("down");
    await d.waitForSettle({ timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    const selAfterDown = Number(st.selectedIndex ?? -1);
    const scrollAfterDown = st.mainListScroll ?? null;
    const selOkDown = selectionOk(st);
    const expectedDown = Math.min(sel0 + 200, Math.max(0, itemCount - 1));
    // Allow off-by-one from header/unselectable rows; require substantial move
    const downMoved = selAfterDown > sel0 || itemCount <= 1;
    const downArithmeticOk =
      selOkDown &&
      (Math.abs(selAfterDown - expectedDown) <= 2 ||
        (selAfterDown >= sel0 + 150 && itemCount > 200));

    // jump-to-end (End), else PageDown storm
    d.simulateKey("end");
    await d.waitForSettle({ timeoutMs: 4000 });
    let stEnd = await d.getState({ timeoutMs: 8000 });
    let selEnd = Number(stEnd.selectedIndex ?? -1);
    if (!(selEnd > selAfterDown)) {
      for (let i = 0; i < 30; i++) d.simulateKey("pagedown");
      await d.waitForSettle({ timeoutMs: 4000 });
      stEnd = await d.getState({ timeoutMs: 8000 });
      selEnd = Number(stEnd.selectedIndex ?? -1);
    }

    // Up × 200 from current
    const selBeforeUp = selEnd;
    for (let i = 0; i < 200; i++) d.simulateKey("up");
    await d.waitForSettle({ timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    const selAfterUp = Number(st.selectedIndex ?? -1);
    const selOkUp = selectionOk(st);
    const expectedUp = Math.max(0, selBeforeUp - 200);
    const upArithmeticOk =
      selOkUp &&
      (Math.abs(selAfterUp - expectedUp) <= 2 ||
        (selBeforeUp - selAfterUp >= 150 && selBeforeUp > 200));

    // selected row within visible bounds (mainListScroll + layout)
    let occlusion: Json = { checked: false };
    try {
      const layout = await d.getLayoutInfo({}, { timeoutMs: 8000 });
      const comps: any[] = layout?.components ?? [];
      const list = comps.find((c) =>
        /scriptlist|list:results|MainList/i.test(`${c.name ?? ""}|${c.type ?? ""}`),
      );
      const selected = comps.find(
        (c) => c.selected === true || /ListItem\[/.test(String(c.name ?? "")),
      );
      const scrollVis = st.mainListScroll?.selectedRowVisible;
      if (list?.bounds && selected?.bounds) {
        const lb = list.bounds;
        const sb = selected.bounds;
        const visible =
          sb.y >= lb.y - 1 && sb.y + sb.height <= lb.y + lb.height + 1;
        occlusion = {
          checked: true,
          layoutVisible: visible,
          selectedRowVisible: scrollVis ?? null,
          selectedRowWithinSafeViewport: st.mainListScroll?.selectedRowWithinSafeViewport ?? null,
          list: lb,
          selected: sb,
        };
      } else {
        occlusion = {
          checked: true,
          selectedRowVisible: scrollVis ?? null,
          selectedRowWithinSafeViewport: st.mainListScroll?.selectedRowWithinSafeViewport ?? null,
          note: "layout match incomplete; used mainListScroll",
        };
      }
    } catch (e) {
      occlusion = { checked: false, error: String(e).slice(0, 160) };
    }

    // Dir-browse deep fixture
    const browseReceipts: Json[] = [];
    let browseBad = 0;
    let path = deepRoot;
    for (let dlevel = 0; dlevel < Math.min(DIR_DEPTH, 6); dlevel++) {
      const q = path.endsWith("/") ? path : `${path}/`;
      d.setFilter(q);
      await d.waitForSettle({ timeoutMs: 4000 });
      const bst = await d.getState({ timeoutMs: 8000 });
      const got = filterText(bst);
      const vis = Number(bst.visibleChoiceCount ?? 0);
      const inputOk = got === q || got === path || got.endsWith("/");
      browseReceipts.push({
        level: dlevel,
        queryTail: q.slice(-90),
        inputOk,
        vis,
        sel: bst.selectedIndex,
      });
      if (!inputOk || (dlevel === 0 && vis === 0)) browseBad++;
      path = join(path, `level-${dlevel}`);
    }

    // Path-escape probe (must not crash)
    d.setFilter(`${deepRoot}/../../../etc/`);
    await d.waitForSettle({ timeoutMs: 4000 });
    const escapeSt = await d.getState({ timeoutMs: 8000 });
    browseReceipts.push({
      escapeProbe: true,
      input: filterText(escapeSt).slice(0, 120),
      vis: escapeSt.visibleChoiceCount,
      alive: true,
    });

    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 4000 });
    const afterBrowse = await d.getState({ timeoutMs: 8000 });
    if (filterText(afterBrowse) !== "") {
      browseBad++;
    }

    const after = await errorKeys(d);
    const newErrs = [...after].filter((k) => !before.has(k)).slice(0, 6);
    const thinList = vis0 < 50 && itemCount < 50;
    const rowVisibleOk =
      occlusion.selectedRowVisible === true ||
      occlusion.selectedRowVisible === null ||
      occlusion.layoutVisible === true ||
      occlusion.checked === false;

    const hardFail = !selOkDown || !selOkUp || newErrs.length > 0;
    const soft =
      !downMoved ||
      !downArithmeticOk ||
      !upArithmeticOk ||
      browseBad > 2 ||
      !rowVisibleOk;

    const verdict: Row["verdict"] = hardFail
      ? "FAIL"
      : thinList
        ? "ENV"
        : soft
          ? "SUSPECT"
          : "PASS";

    rows.push({
      id: "deep-navigation",
      verdict,
      reason: `sel0=${sel0}→down200=${selAfterDown}(exp~${expectedDown})→end=${selEnd}→up200=${selAfterUp}(exp~${expectedUp}); vis0=${vis0} items=${itemCount}; browseBad=${browseBad}; downMoved=${downMoved}`,
      detail: {
        sel0,
        selAfterDown,
        expectedDown,
        selEnd,
        selBeforeUp,
        selAfterUp,
        expectedUp,
        vis0,
        itemCount,
        scrollAfterDown,
        occlusion,
        browseReceipts,
        browseBad,
        newErrs,
        thinList,
        deepLeaf,
        downArithmeticOk,
        upArithmeticOk,
      },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // Row 4: chunked large-paste + mass-delete recovery (+ 16KiB by-design)
  // =========================================================================
  if (want(4)) {
    const beforeErr = await errorKeys(d);
    const beforeWarn = await warnKeys(d);
    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 4000 });
    const emptySt = await d.getState({ timeoutMs: 8000 });
    const fullVis = Number(emptySt.visibleChoiceCount ?? 0);

    // 4a: single setFilter >16KiB — by-design drop; expect WARN
    const over = "X".repeat(20_000);
    d.setFilter(over);
    await d.waitForSettle({ timeoutMs: 4000 });
    const overSt = await d.getState({ timeoutMs: 8000 });
    const overLen = filterText(overSt).length;
    const afterWarn = await warnKeys(d);
    const newWarns = afterWarn.filter((k) => !beforeWarn.includes(k));
    const capWarn = newWarns.filter(
      (w) =>
        /16\s*\*?\s*KiB|16384|MAX_STDIN|stdin.*cap|command.*too large|payload.*too large|input.*too large|byte/i.test(
          w,
        ),
    );
    // Also accept any new WARN after the oversized setFilter as evidence if message is size-related
    const overDropped = overLen < 20_000;

    // 4b: chunked ≤8KiB successive setFilters (each message under cap)
    // Build observed max that still echoes, then shrink.
    const chunkSizes = [4_000, 8_000, 12_000, 15_000, 16_000];
    const chunkReceipts: Json[] = [];
    for (const n of chunkSizes) {
      d.setFilter("P".repeat(n));
      await d.waitForSettle({ timeoutMs: 4000 });
      const st = await d.getState({ timeoutMs: 8000 });
      const got = filterText(st).length;
      chunkReceipts.push({ n, got, ok: got === n, vis: st.visibleChoiceCount });
    }

    // Mass-delete recovery: shrink from 8KiB → empty
    const shrinkSteps = [8000, 4000, 1000, 100, 0];
    const shrinkReceipts: Json[] = [];
    for (const n of shrinkSteps) {
      const t0 = performance.now();
      d.setFilter(n === 0 ? "" : "Q".repeat(n));
      await d.waitForSettle({ timeoutMs: 4000 });
      const st = await d.getState({ timeoutMs: 8000 });
      shrinkReceipts.push({
        n,
        ms: Math.round(performance.now() - t0),
        inputLen: filterText(st).length,
        vis: st.visibleChoiceCount,
      });
    }

    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 4000 });
    const recovered = await d.getState({ timeoutMs: 8000 });
    const recoveredInput = filterText(recovered);
    const recoveredVis = Number(recovered.visibleChoiceCount ?? 0);
    const listRecovered =
      recoveredInput === "" &&
      (fullVis === 0 || recoveredVis >= Math.max(1, Math.floor(fullVis * 0.5)));

    // Chunked under-cap: at least 8KiB must echo
    const chunk8 = chunkReceipts.find((r) => r.n === 8000);
    const chunk8Ok = chunk8 && chunk8.ok === true;
    const underCapOk = chunkReceipts
      .filter((r) => Number(r.n) <= 16000)
      .every((r) => r.ok === true || Number(r.n) === 16000); // 16000 may be edge

    const afterErr = await errorKeys(d);
    const newErrs = [...afterErr].filter((k) => !beforeErr.has(k)).slice(0, 6);

    const byDesignCap = overDropped; // 20k does not fully echo
    const capDocumented = capWarn.length > 0 || byDesignCap;

    const hardFail = !listRecovered || !chunk8Ok || newErrs.length > 0;
    const soft = !capDocumented || !underCapOk;

    rows.push({
      id: "chunked-paste-mass-delete",
      verdict: hardFail ? "FAIL" : soft ? "SUSPECT" : "PASS",
      reason: `over20kLen=${overLen} capWarns=${capWarn.length} chunk8Ok=${chunk8Ok} listRecovered=${listRecovered} vis ${fullVis}→${recoveredVis}`,
      detail: {
        overLen,
        overDropped,
        capWarn: capWarn.slice(0, 6),
        newWarnsSample: newWarns.slice(0, 8),
        chunkReceipts,
        shrinkReceipts,
        fullVis,
        recoveredVis,
        recoveredInput,
        listRecovered,
        newErrs,
        byDesign: "MAX_STDIN_COMMAND_BYTES 16KiB — single setFilter >16KiB drops",
      },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }
} catch (e) {
  rows.push({ id: "probe-crash", verdict: "FAIL", reason: String(e).slice(0, 300) });
  console.error(`  [FAIL] probe-crash — ${String(e).slice(0, 200)}`);
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

const fail = rows.filter((r) => r.verdict === "FAIL").length;
const summary = {
  battery: "chaos-21-input-nav-storms",
  lane: "L6-monkey-grok-input",
  binary: BINARY,
  seedCount: SEED_COUNT,
  rows,
  overall: fail > 0 ? "FAIL" : rows.some((r) => r.verdict === "SUSPECT") ? "SUSPECT" : "PASS",
};
console.log(JSON.stringify(summary, null, 2));
process.exit(fail > 0 ? 1 : 0);
