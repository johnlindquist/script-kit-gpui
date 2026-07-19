#!/usr/bin/env bun
/**
 * OF-20 candidate: attribute 14KB filter-settle latency watch
 * (NN=25 OF-dictation-14kb-filter-settle — maxSettle ~5265ms vs 4s budget).
 *
 * Method (manager round-59):
 *  - hidden-window sandbox
 *  - deliver ~10–14KB single-line filter via pushDictationResult (same path
 *    as the watch item) AND a control via setFilter
 *  - sample getLogs WHILE settle runs (contains: filter / PERF / SEARCH —
 *    NOT target:, per module-path lesson)
 *  - name cost center; NO fix
 *  - verdict fork: hot-path cost → finding; structural one-off → budget call
 *
 * Enable SCRIPT_KIT_FILTER_PERF_LOG so FILTER_PERF traces fire.
 *
 * Run:
 *   SCRIPT_KIT_GPUI_BINARY=target-agent/artifacts/monkey-input/script-kit-gpui \
 *     bun scripts/agentic/of20-14kb-filter-settle-attribute.ts
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-input/script-kit-gpui");
const OUT = join(
  process.cwd(),
  process.env.OF20_OUTPUT_DIR ?? ".test-output/of20-14kb-filter-settle",
);
mkdirSync(OUT, { recursive: true });

// Match NN=25 watch: ~10KB word storm + 14KB H-line under stdin cap
const STDIN_SAFE = 14_000;
const PAYLOADS: Array<{ name: string; text: string; via: "pushDictation" | "setFilter" }> = [
  {
    name: "dictation-10k-words",
    text: ("word ".repeat(2_000)).trim(), // ~9999 chars — the watch hotspot
    via: "pushDictation",
  },
  {
    name: "dictation-14k-H",
    text: "H".repeat(STDIN_SAFE),
    via: "pushDictation",
  },
  {
    name: "setFilter-10k-words",
    text: ("word ".repeat(2_000)).trim(),
    via: "setFilter",
  },
  {
    name: "setFilter-small-control",
    text: "hello world control",
    via: "setFilter",
  },
];

type LogSnap = {
  tMs: number;
  count: number;
  entries: Array<{ target?: string; message?: string; level?: string }>;
};

function extractMs(msg: string): number | null {
  const m =
    msg.match(/in ([\d.]+)ms/i) ||
    msg.match(/total=([\d.]+)ms/i) ||
    msg.match(/took ([\d.]+)ms/i) ||
    msg.match(/elapsed[=:]?\s*([\d.]+)ms/i) ||
    msg.match(/([\d.]+)ms/);
  return m ? parseFloat(m[1]) : null;
}

function classifyEntry(message: string): string {
  const m = message;
  if (/SEARCH_TOTAL/i.test(m)) return "search_total";
  if (/PASSIVE_SOURCE_DONE/i.test(m)) return "passive_source";
  if (/FILTER_PERF|filter apply|set_filter|filter_input/i.test(m)) return "filter_pipeline";
  if (/RENDER_PERF|render/i.test(m)) return "render";
  if (/PREVIEW_PERF|preview/i.test(m)) return "preview";
  if (/dictation/i.test(m)) return "dictation";
  if (/GROUP_DONE|SEARCH/i.test(m)) return "search_group";
  return "other";
}

async function pullPerfLogs(d: Driver, contains: string, limit = 80): Promise<LogSnap["entries"]> {
  // Module-path lesson: use contains, NOT target.
  const r = (await d
    .getLogs({ contains, limit }, { timeoutMs: 4000 })
    .catch(() => ({ entries: [] }))) as Json;
  const entries = ((r as any).entries ?? (r as any).logs ?? []) as LogSnap["entries"];
  return entries.map((e) => ({
    target: e.target,
    message: typeof e.message === "string" ? e.message.slice(0, 400) : String(e.message ?? "").slice(0, 400),
    level: e.level,
  }));
}

function mergeUnique(
  into: Map<string, { target?: string; message: string; firstT: number; lastT: number; count: number }>,
  entries: LogSnap["entries"],
  tMs: number,
) {
  for (const e of entries) {
    const msg = e.message ?? "";
    if (!msg) continue;
    // keep PERF-ish / filter-ish only
    if (
      !/PERF|SEARCH|filter|Filter|PASSIVE|RENDER|PREVIEW|GROUP_DONE|dictation/i.test(msg) &&
      !/PERF|filter|search/i.test(e.target ?? "")
    ) {
      continue;
    }
    const key = `${e.target ?? ""}|${msg.slice(0, 160)}`;
    const prev = into.get(key);
    if (prev) {
      prev.count += 1;
      prev.lastT = tMs;
    } else {
      into.set(key, {
        target: e.target,
        message: msg,
        firstT: tMs,
        lastT: tMs,
        count: 1,
      });
    }
  }
}

const receipt: Json = {
  schemaVersion: 1,
  tool: "of20-14kb-filter-settle-attribute",
  binary: BINARY,
  env: {
    SCRIPT_KIT_FILTER_PERF_LOG: "1",
    note: "contains-filter sampling (not target:) while settle runs",
  },
  cases: [] as Json[],
  verdict: null as Json,
};

const d = await Driver.launch({
  binary: BINARY,
  sandboxHome: true,
  sessionName: `of20-14kb-${process.pid}`,
  readyTimeoutMs: 25_000,
  defaultTimeoutMs: 12_000,
  env: {
    SCRIPT_KIT_FILTER_PERF_LOG: "1",
    SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
  },
});
receipt.sessionDir = d.sessionDir;

try {
  await Bun.sleep(400);
  try {
    await d.waitForSettle({ timeoutMs: 5000 });
  } catch {}

  for (const payload of PAYLOADS) {
    // clear between cases
    d.setFilter("");
    await Bun.sleep(200);
    try {
      await d.waitForSettle({ timeoutMs: 3000 });
    } catch {}

    // baseline ring snapshot keys
    const seen = new Map<
      string,
      { target?: string; message: string; firstT: number; lastT: number; count: number }
    >();
    const timeline: LogSnap[] = [];
    const t0 = performance.now();

    if (payload.via === "pushDictation") {
      d.send({
        type: "pushDictationResult",
        transcript: payload.text,
        target: "mainWindowFilter",
      });
    } else {
      d.setFilter(payload.text);
    }

    // Sample WHILE settle runs — poll logs + state until stable or budget
    const SETTLE_BUDGET = 12_000;
    let settled = false;
    let settleElapsed = 0;
    let probes = 0;
    let lastFp = "";
    let stable = 0;
    const required = 3;
    let filterLenEnd = 0;
    let alive = true;

    while (performance.now() - t0 < SETTLE_BUDGET) {
      probes += 1;
      const tMs = Math.round(performance.now() - t0);

      // Parallel-ish: state + multi-contains log pulls
      const [st, logsFilter, logsPerf, logsSearch] = await Promise.all([
        d.getState({ timeoutMs: 5000 }).catch(() => null) as Promise<Json | null>,
        pullPerfLogs(d, "filter", 100),
        pullPerfLogs(d, "PERF", 100),
        pullPerfLogs(d, "SEARCH", 100),
      ]);

      if (st == null) {
        alive = false;
        break;
      }
      const diag = (st as any)?.filterInputDiagnostics;
      const fv =
        (diag && typeof diag.canonicalFilterText === "string"
          ? diag.canonicalFilterText
          : String((st as any)?.inputValue ?? (st as any)?.filter ?? "")) || "";
      filterLenEnd = fv.length;
      const fp = JSON.stringify({
        f: fv.length,
        v: (st as any)?.visibleChoiceCount ?? null,
        p: (st as any)?.promptType ?? null,
        s: (st as any)?.selectedIndex ?? null,
      });
      if (fp === lastFp) stable += 1;
      else {
        stable = 1;
        lastFp = fp;
      }
      if (stable >= required) {
        settled = true;
        settleElapsed = tMs;
        break;
      }

      mergeUnique(seen, logsFilter, tMs);
      mergeUnique(seen, logsPerf, tMs);
      mergeUnique(seen, logsSearch, tMs);
      timeline.push({
        tMs,
        count: seen.size,
        entries: [...logsFilter, ...logsPerf, ...logsSearch]
          .filter((e) => /PERF|SEARCH|filter|PASSIVE|RENDER|PREVIEW|GROUP/i.test(e.message ?? ""))
          .slice(0, 12),
      });

      await Bun.sleep(80);
    }
    if (!settled) settleElapsed = Math.round(performance.now() - t0);

    // final log harvest
    const finalLogs = [
      ...(await pullPerfLogs(d, "filter", 150)),
      ...(await pullPerfLogs(d, "PERF", 150)),
      ...(await pullPerfLogs(d, "SEARCH", 150)),
      ...(await pullPerfLogs(d, "PASSIVE", 80)),
      ...(await pullPerfLogs(d, "RENDER", 80)),
    ];
    mergeUnique(seen, finalLogs, Math.round(performance.now() - t0));

    const perfLines = [...seen.values()].map((v) => {
      const bucket = classifyEntry(v.message);
      const ms = extractMs(v.message);
      return {
        bucket,
        ms,
        count: v.count,
        firstT: v.firstT,
        lastT: v.lastT,
        target: v.target ?? null,
        message: v.message.slice(0, 280),
      };
    });

    // Aggregate cost by bucket (sum of extracted ms where available)
    const byBucket: Record<string, { count: number; sumMs: number; maxMs: number; samples: string[] }> =
      {};
    for (const p of perfLines) {
      const b = p.bucket;
      if (!byBucket[b]) byBucket[b] = { count: 0, sumMs: 0, maxMs: 0, samples: [] };
      byBucket[b].count += p.count;
      if (p.ms != null) {
        byBucket[b].sumMs += p.ms;
        byBucket[b].maxMs = Math.max(byBucket[b].maxMs, p.ms);
      }
      if (byBucket[b].samples.length < 4) byBucket[b].samples.push(p.message.slice(0, 160));
    }

    // Cost center heuristic
    const ranked = Object.entries(byBucket)
      .map(([k, v]) => ({ bucket: k, ...v }))
      .sort((a, b) => b.maxMs - a.maxMs || b.sumMs - a.sumMs);

    const top = ranked[0] ?? null;
    const searchMax = byBucket.search_total?.maxMs ?? byBucket.search_group?.maxMs ?? 0;
    const passiveMax = byBucket.passive_source?.maxMs ?? 0;
    const filterMax = byBucket.filter_pipeline?.maxMs ?? 0;
    const renderMax = byBucket.render?.maxMs ?? 0;

    const wall = settleElapsed;
    // If search_total or passive sources dominate wall, hot-path; if wall >> sum of
    // reported ms and few lines, likely settle-loop/structural
    const reportedCap = Math.max(searchMax, passiveMax, filterMax, renderMax);
    const coverage = wall > 0 ? reportedCap / wall : 0;

    let caseVerdict: string;
    let costCenter: string;
    if (wall < 4000 && payload.name.includes("control")) {
      caseVerdict = "control_ok";
      costCenter = "n/a-control";
    } else if (searchMax >= 2000 || (searchMax > 0 && searchMax >= wall * 0.4)) {
      caseVerdict = "hot_path_search";
      costCenter = "main-list search (SEARCH_TOTAL / query scoring over huge filter text)";
    } else if (passiveMax >= 2000 || passiveMax >= wall * 0.4) {
      caseVerdict = "hot_path_passive_sources";
      costCenter = "passive root sources (PASSIVE_SOURCE_DONE under large query_len)";
    } else if (filterMax >= 2000 || filterMax >= wall * 0.4) {
      caseVerdict = "hot_path_filter_pipeline";
      costCenter = "filter apply pipeline (FILTER_PERF)";
    } else if (renderMax >= 2000 || renderMax >= wall * 0.4) {
      caseVerdict = "hot_path_render";
      costCenter = "render/layout (RENDER_PERF)";
    } else if (wall >= 4000 && reportedCap < 500 && perfLines.length < 3) {
      caseVerdict = "structural_or_instrumentation_gap";
      costCenter =
        "wall-clock settle without matching PERF cost — settle fingerprint loop / missing instrumentation / one-shot layout";
    } else if (wall >= 4000 && coverage < 0.25) {
      caseVerdict = "structural_one_off_or_uninstrumented";
      costCenter = top
        ? `weak PERF coverage (max reported ${reportedCap.toFixed(0)}ms vs wall ${wall}ms); top bucket=${top.bucket}`
        : `no PERF attribution for wall ${wall}ms`;
    } else if (wall >= 4000) {
      caseVerdict = "mixed_hot_path";
      costCenter = top
        ? `${top.bucket} (max ${top.maxMs.toFixed(0)}ms) + residual uninstrumented`
        : "unknown mixed";
    } else {
      caseVerdict = "within_budget";
      costCenter = top ? top.bucket : "fast-path";
    }

    const caseReceipt: Json = {
      name: payload.name,
      via: payload.via,
      textLen: payload.text.length,
      wallSettleMs: wall,
      settled,
      probes,
      alive,
      filterLenEnd,
      budgetMs: 4000,
      overBudget: wall > 4000,
      costCenter,
      caseVerdict,
      byBucket,
      rankedBuckets: ranked.slice(0, 8),
      perfLineCount: perfLines.length,
      perfLines: perfLines
        .sort((a, b) => (b.ms ?? 0) - (a.ms ?? 0))
        .slice(0, 40),
      timelineSample: timeline.filter((_, i) => i % 3 === 0).slice(0, 12),
      coverageReportedMaxOverWall: Number(coverage.toFixed(3)),
    };
    (receipt.cases as Json[]).push(caseReceipt);
    writeFileSync(join(OUT, `case-${payload.name}.json`), JSON.stringify(caseReceipt, null, 2));
    console.error(
      JSON.stringify({
        case: payload.name,
        wall,
        settled,
        costCenter,
        caseVerdict,
        top: ranked[0] ?? null,
      }),
    );
  }

  // Overall verdict fork across huge cases
  const huge = (receipt.cases as Json[]).filter(
    (c) => Number(c.textLen) >= 8000 && c.via !== "n/a",
  );
  const hot = huge.filter((c) => String(c.caseVerdict).startsWith("hot_path"));
  const structural = huge.filter((c) =>
    String(c.caseVerdict).includes("structural"),
  );
  const over = huge.filter((c) => c.overBudget === true);

  let fork: "finding_hot_path" | "budget_reality_call" | "mixed" | "cleared";
  let summary: string;
  if (over.length === 0) {
    fork = "cleared";
    summary = "All large cases settled within 4s budget this run — watch may be load-sensitive.";
  } else if (hot.length >= Math.ceil(over.length / 2)) {
    fork = "finding_hot_path";
    const centers = [...new Set(hot.map((c) => String(c.costCenter)))];
    summary =
      `Hot-path cost dominates over-budget settles. Cost center(s): ${centers.join(" | ")}. ` +
      `Recommend finding + fix plan (no fix this unit).`;
  } else if (structural.length >= Math.ceil(over.length / 2)) {
    fork = "budget_reality_call";
    summary =
      "Over-budget wall without proportional PERF hot-path — structural one-off / settle-loop / " +
      "instrumentation gap. Manager budget-vs-reality call (no product fix proposed).";
  } else {
    fork = "mixed";
    summary =
      "Mixed signals across cases — see per-case cost centers; manager triage between hot-path fix vs budget.";
  }

  receipt.verdict = {
    fork,
    summary,
    overBudgetCases: over.map((c) => ({
      name: c.name,
      wallSettleMs: c.wallSettleMs,
      costCenter: c.costCenter,
      caseVerdict: c.caseVerdict,
      topBucket: (c.rankedBuckets as Json[])?.[0] ?? null,
    })),
    control: (receipt.cases as Json[]).find((c) => String(c.name).includes("control")) ?? null,
    recommendation:
      fork === "finding_hot_path"
        ? "Promote to OF-20 FINDING: optimize named cost center; keep probe as red→green lock."
        : fork === "budget_reality_call"
          ? "Do not open product fix; call budget-reality (raise settle budget for huge-filter OR document acceptable)."
          : "See receipts; no fix in this unit.",
  };
} finally {
  await d.close();
}

writeFileSync(join(OUT, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(JSON.stringify({ fork: (receipt.verdict as any)?.fork, summary: (receipt.verdict as any)?.summary, out: OUT }, null, 2));
process.exit(0);
