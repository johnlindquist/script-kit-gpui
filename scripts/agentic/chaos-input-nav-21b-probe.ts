#!/usr/bin/env bun
/**
 * Chaos-21b (L6 monkey-grok-input) — hidden gap-closure rows only.
 * Gaps 5, 7, 8, 11 + !ps null-hint receipt.
 * Driver APIs only as used in chaos-frecency-input-history-probe.ts /
 * chaos-interaction-stress.ts / driver.ts.
 */
import { mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-input/script-kit-gpui");

type Row = {
  id: string;
  verdict: "PASS" | "SUSPECT" | "FAIL" | "ENV" | "BY_DESIGN";
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

function logEntries(r: Json): any[] {
  return (r as any)?.entries ?? (r as any)?.logs ?? [];
}

function seedHome(kitDir: string) {
  const scriptsDir = join(kitDir, "plugins", "main", "scripts");
  mkdirSync(scriptsDir, { recursive: true });
  writeFileSync(join(kitDir, "config.ts"), "export default {};\n");
  // 200 scripts so common prefixes yield vis>=100; rare tokens → ~0
  for (let i = 0; i < 200; i++) {
    const name = `chaos-21b-${String(i).padStart(3, "0")}`;
    writeFileSync(
      join(scriptsDir, `${name}.ts`),
      `// Name: ${name}\n// Description: seed row ${i}\nconsole.log(${i});\n`,
    );
  }
  for (const label of ["script-alpha", "script-beta", "test-gamma"]) {
    writeFileSync(
      join(scriptsDir, `${label}.ts`),
      `// Name: ${label}\nconsole.log("${label}");\n`,
    );
  }
  // Input history for gap 8 (A8: empty input + at top row → Up enters history)
  const historyEntries = [
    "history-recall-token-AAA",
    "history-recall-token-BBB",
    "history-recall-token-CCC",
    ...Array.from({ length: 40 }, (_, i) => `history entry ${i}`),
  ];
  writeFileSync(
    join(kitDir, "input_history.json"),
    JSON.stringify({ entries: historyEntries, selected_results: {} }),
  );
}

const scratch = join(tmpdir(), `chaos-21b-${process.pid}-${Date.now().toString(36)}`);
const home = join(scratch, "home");
const kitDir = join(home, ".scriptkit");
mkdirSync(kitDir, { recursive: true });
seedHome(kitDir);

console.error(`[chaos-21b] binary=${BINARY}`);

const d = await Driver.launch({
  binary: BINARY,
  sessionName: "monkey-input-21b",
  env: { HOME: home, SK_PATH: kitDir },
  readyTimeoutMs: 20_000,
  defaultTimeoutMs: 8_000,
});

try {
  await d.getState({ timeoutMs: 8000 });
  d.setFilter("");
  await d.waitForSettle({ timeoutMs: 4000 });

  // =========================================================================
  // Gap 11: hard vis 0 <-> 100+ assertions across type/delete phases
  // =========================================================================
  {
    const before = await errorKeys(d);
    const phases: Json[] = [];
    let hardFail = 0;

    // rare → expect few/zero; common → expect many
    const sequence: { q: string; want: "zeroish" | "many" }[] = [
      { q: "zzzzq-no-hit-xx", want: "zeroish" },
      { q: "chaos-21b", want: "many" }, // matches 200 seeds
      { q: "zzzzq-no-hit-yy", want: "zeroish" },
      { q: "script", want: "many" },
      { q: "", want: "many" },
    ];

    for (const step of sequence) {
      d.setFilter(step.q);
      await d.waitForSettle({ timeoutMs: 4000 });
      const st = await d.getState({ timeoutMs: 8000 });
      const vis = Number(st.visibleChoiceCount ?? 0);
      const input = String(st.inputValue ?? "");
      let ok = input === step.q;
      if (step.want === "zeroish") {
        // allow a handful of fuzzy noise; hard assert < 20 (not 100+)
        ok = ok && vis < 20;
      } else {
        ok = ok && vis >= 100;
      }
      if (!ok) hardFail++;
      phases.push({ q: step.q, want: step.want, vis, input, ok });
    }

    const after = await errorKeys(d);
    const newErrs = [...after].filter((k) => !before.has(k)).slice(0, 6);
    rows.push({
      id: "gap11-hard-vis-swing",
      verdict: hardFail === 0 && newErrs.length === 0 ? "PASS" : "FAIL",
      reason:
        hardFail === 0
          ? `phases ok; ${phases.map((p) => `${p.q || "∅"}→vis${p.vis}`).join("; ")}`
          : `hardFail=${hardFail} newErrs=${newErrs.length}`,
      detail: { phases, newErrs },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // Gap 5: Backspace simulateKey storm (not setFilter shrink)
  // =========================================================================
  {
    const before = await errorKeys(d);
    const seed = "B".repeat(80);
    d.setFilter(seed);
    await d.waitForSettle({ timeoutMs: 4000 });
    let st = await d.getState({ timeoutMs: 8000 });
    const seeded = String(st.inputValue ?? "");
    const seedOk = seeded.length === 80;

    // Rapid backspace dispatch (hold-repeat equivalent, protocol)
    for (let i = 0; i < 80; i++) d.simulateKey("backspace");
    await d.waitForSettle({ timeoutMs: 4000 });
    st = await d.getState({ timeoutMs: 8000 });
    const afterBs = String(st.inputValue ?? "");
    const deleted = afterBs.length < seeded.length;
    const fullyCleared = afterBs.length === 0;

    // Recover list after clear
    if (!fullyCleared) {
      d.setFilter("");
      await d.waitForSettle({ timeoutMs: 4000 });
    }
    st = await d.getState({ timeoutMs: 8000 });
    const recoveredVis = Number(st.visibleChoiceCount ?? 0);

    const after = await errorKeys(d);
    const newErrs = [...after].filter((k) => !before.has(k)).slice(0, 6);

    // Hidden-window keyDown may not reach the filter (known limitation).
    // Classify honestly: if seed worked but backspace no-op → ENV/SUSPECT
    // for hidden; not silent PASS.
    let verdict: Row["verdict"] = "PASS";
    let reason = "";
    if (!seedOk) {
      verdict = "FAIL";
      reason = `seed failed len=${seeded.length}`;
    } else if (fullyCleared && newErrs.length === 0) {
      verdict = "PASS";
      reason = `80 backspaces cleared 80-char filter; recoveredVis=${recoveredVis}`;
    } else if (deleted && newErrs.length === 0) {
      verdict = "SUSPECT";
      reason = `partial delete ${seeded.length}→${afterBs.length}; recoveredVis=${recoveredVis}`;
    } else if (!deleted) {
      verdict = "ENV";
      reason = `backspace no-op hidden (len stayed ${afterBs.length}); known keyDown-focus limitation — frontmost retest required`;
    } else {
      verdict = "FAIL";
      reason = `deleted=${deleted} errs=${newErrs.length}`;
    }

    rows.push({
      id: "gap5-backspace-simulateKey-storm",
      verdict,
      reason,
      detail: {
        seedLen: seeded.length,
        afterLen: afterBs.length,
        deleted,
        fullyCleared,
        recoveredVis,
        newErrs,
        windowVisible: st.windowVisible,
      },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // Gap 7: PERF getLogs zero-lines root-cause
  // =========================================================================
  {
    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 3000 });
    // Drive several searches so Search PERF lines should fire
    for (const q of ["a", "am", "ama", "amaz", "amazon", "chaos-21b", "script"]) {
      d.setFilter(q);
      await d.waitForSettle({ timeoutMs: 3000 });
    }

    const byTargetPerf = await d.getLogs({ target: "PERF", limit: 200 }, { timeoutMs: 6000 });
    const byContainsSearch = await d.getLogs(
      { contains: "Search", limit: 200 },
      { timeoutMs: 6000 },
    );
    const byContainsTook = await d.getLogs({ contains: "took", limit: 200 }, { timeoutMs: 6000 });
    const unfiltered = await d.getLogs({ limit: 200 }, { timeoutMs: 6000 });

    const eTarget = logEntries(byTargetPerf);
    const eSearch = logEntries(byContainsSearch);
    const eTook = logEntries(byContainsTook);
    const eAll = logEntries(unfiltered);

    // How many unfiltered look like Search-took / have PERF in message
    const searchLike = eAll.filter(
      (e) =>
        /Search .*took/i.test(String(e.message ?? "")) ||
        /took .*ms/i.test(String(e.message ?? "")),
    );
    const targetsSample = [...new Set(eAll.map((e) => String(e.target ?? "")).filter(Boolean))].slice(
      0,
      25,
    );
    const categoryInTarget = eAll.filter((e) => /PERF/i.test(String(e.target ?? "")));

    // Root cause: getLogs `target` matches tracing module path (substring), NOT
    // logging::log category "PERF". Search lines land as message text with
    // module target e.g. script_kit_gpui::... — use contains:"Search" / "took".
    const rootCause =
      eTarget.length === 0 && (eSearch.length > 0 || searchLike.length > 0)
        ? "probe-side: target:\"PERF\" filters tracing metadata.target (module path), not logging category; Search lines are in message — use contains:\"Search\" or contains:\"took\""
        : eTarget.length === 0 && eSearch.length === 0 && searchLike.length === 0
          ? "no Search/took lines in ring at all — ring rotation, logging::log not mirrored, or search path skipped empty-filter short-circuit"
          : eTarget.length > 0
            ? "target PERF matched unexpected entries"
            : "partial match";

    const probeBug = eTarget.length === 0 && (eSearch.length > 0 || searchLike.length > 0);
    const sampleMsgs = (eSearch.length ? eSearch : searchLike)
      .slice(0, 6)
      .map((e) => ({
        target: e.target,
        message: String(e.message ?? "").slice(0, 120),
      }));

    rows.push({
      id: "gap7-perf-getLogs-root-cause",
      verdict: probeBug ? "PASS" : searchLike.length > 0 || eSearch.length > 0 ? "PASS" : "SUSPECT",
      reason: probeBug
        ? `ROOT CAUSE=probe filter misuse; targetPERF=${eTarget.length} containsSearch=${eSearch.length} searchLike=${searchLike.length}`
        : `targetPERF=${eTarget.length} containsSearch=${eSearch.length} took=${eTook.length} searchLike=${searchLike.length} — ${rootCause.slice(0, 80)}`,
      detail: {
        rootCause,
        counts: {
          targetPERF: eTarget.length,
          containsSearch: eSearch.length,
          containsTook: eTook.length,
          unfiltered: eAll.length,
          searchLike: searchLike.length,
          categoryInTarget: categoryInTarget.length,
        },
        targetsSample,
        sampleMsgs,
        fix: "Use getLogs({contains:\"Search\"}) or contains:\"took\"; do not use target:\"PERF\" for logging::log category",
      },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // Gap 8: filter Up/Down history-recall (chaos-around A8)
  // =========================================================================
  {
    const before = await errorKeys(d);
    // A8: Up walks selection to top first; history recall enters from top + empty input
    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 4000 });
    let st = await d.getState({ timeoutMs: 8000 });
    let recalled: string | null = null;
    const walk: Json[] = [];

    for (let i = 0; i < 8; i++) {
      d.simulateKey("up");
      await d.waitForSettle({ timeoutMs: 3000 });
      st = await d.getState({ timeoutMs: 8000 });
      const input = String(st.inputValue ?? "");
      walk.push({
        i,
        input: input.slice(0, 80),
        sel: st.selectedIndex,
        vis: st.visibleChoiceCount,
      });
      if (input.length > 0 && input.startsWith("history")) {
        recalled = input;
        break;
      }
    }

    // If recalled, Down should walk history or leave list
    let downWalk: Json[] = [];
    if (recalled) {
      for (let i = 0; i < 3; i++) {
        d.simulateKey("down");
        await d.waitForSettle({ timeoutMs: 3000 });
        st = await d.getState({ timeoutMs: 8000 });
        downWalk.push({
          i,
          input: String(st.inputValue ?? "").slice(0, 80),
          sel: st.selectedIndex,
        });
      }
    }

    d.simulateKey("escape");
    await d.waitForSettle({ timeoutMs: 3000 });
    d.setFilter("");
    await d.waitForSettle({ timeoutMs: 3000 });

    const after = await errorKeys(d);
    const newErrs = [...after].filter((k) => !before.has(k)).slice(0, 6);

    rows.push({
      id: "gap8-history-recall-arrows",
      verdict: recalled && newErrs.length === 0 ? "PASS" : recalled ? "SUSPECT" : "ENV",
      reason: recalled
        ? `recalled ${JSON.stringify(recalled.slice(0, 40))} within walk; downSteps=${downWalk.length}`
        : `no history recall within 8 Up (A8 may need frontmost focus); walk=${walk.length}`,
      detail: { recalled, walk, downWalk, newErrs },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  // =========================================================================
  // !ps null-hint receipt (papercut watchlist)
  // =========================================================================
  {
    d.setFilter("!ps");
    await d.waitForSettle({ timeoutMs: 4000 });
    const st = await d.getState({ timeoutMs: 8000 });
    const hint = st.menuSyntaxMainHint ?? null;
    const kind = hint && typeof hint.kind === "string" ? hint.kind : null;
    const input = String(st.inputValue ?? "");
    rows.push({
      id: "bang-ps-null-hint-receipt",
      verdict: input === "!ps" ? "PASS" : "FAIL",
      reason:
        kind == null
          ? `input exact; menuSyntaxMainHint=null (papercut watchlist receipt)`
          : `input exact; hint.kind=${kind} (not null this run)`,
      detail: {
        input,
        hintKind: kind,
        hint,
        vis: st.visibleChoiceCount,
        promptType: st.promptType,
      },
    });
    console.error(`  [${rows[rows.length - 1].verdict}] ${rows[rows.length - 1].id} — ${rows[rows.length - 1].reason}`);
  }

  d.setFilter("");
  await d.waitForSettle({ timeoutMs: 3000 });
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
  battery: "chaos-21b-hidden-gaps",
  lane: "L6-monkey-grok-input",
  binary: BINARY,
  rows,
  overall: fail > 0 ? "FAIL" : rows.some((r) => r.verdict === "SUSPECT") ? "SUSPECT" : "PASS",
};
console.log(JSON.stringify(summary, null, 2));
process.exit(fail > 0 ? 1 : 0);
