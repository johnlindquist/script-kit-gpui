#!/usr/bin/env bun
/**
 * Turn `SCRIPT_KIT_AI_TRACE_PATH` NDJSON into per-surface phase numbers and a
 * plain "actually slow vs merely feels slow" verdict.
 *
 * # Why this is a sibling of quick-ai-latency-bench.ts rather than an extension
 *
 * The retained benchmark harness measures ONE surface by driving `codex exec`
 * as a subprocess it controls, pairing A/B variants of a command it builds
 * itself. Three of the four AI surfaces cannot be driven that way at all: they
 * only exist inside the running app, behind the Pi sidecar and the flow
 * session. So the measurement SHAPE differs — this reads traces the app
 * produced, rather than commanding a subprocess.
 *
 * What it deliberately reuses is the statistical discipline that harness
 * proved necessary: medians not means, explicit sample counts, no outlier
 * trimming, and a refusal to report a number that a failed turn contaminated.
 * Yesterday an unpaired 18% "improvement" on Quick AI turned out to have a
 * paired median delta of exactly 0ms; the lesson encoded here is that a
 * latency number without n and spread beside it is not evidence.
 *
 * # The two numbers that matter
 *
 * - `ttfvo` — time to first VISIBLE output. What the user experiences as
 *   responsiveness. Reasoning/thought tokens do NOT count: they are feedback,
 *   not the answer, and counting them would flatter any surface that streams
 *   its thinking.
 * - `ttt` — total turn time, start to terminal. What the surface actually
 *   costs.
 *
 * A surface can be slow on either axis independently, which is exactly the
 * distinction the task asks for.
 */

import { readFileSync, existsSync } from "node:fs";

/** A surface is ACTUALLY slow when a whole turn takes at least this long. */
export const SLOW_TURN_MS = 5_000;

/**
 * A surface merely FEELS slow when the user stares at nothing this long.
 *
 * 1s is the long-standing interaction threshold at which a wait stops feeling
 * like a response and starts feeling like a hang. Rounded to 1.5s here to stay
 * clear of ordinary network jitter, so the label means something.
 */
export const FEELS_SLOW_MS = 1_500;

/** Below this many valid samples a median is not reportable as evidence. */
export const MIN_SAMPLES = 5;

export interface TraceRecord {
  schemaVersion: number;
  runId: string;
  surface: string;
  transport: string;
  seq: number;
  event: string;
  elapsedMs: number;
  outcome?: string;
  isLatencySample?: boolean;
  failureCode?: string;
  [key: string]: unknown;
}

export interface Turn {
  runId: string;
  surface: string;
  transport: string;
  /** Time to the first provider byte of any kind. */
  ttfpe: number | null;
  /** Time to the first user-visible answer token. */
  ttfvo: number | null;
  /** Time to the first reasoning token, when the surface streams one. */
  ttft: number | null;
  /** Total turn time. */
  ttt: number | null;
  outcome: string | null;
  failureCode: string | null;
  /** Only completed turns are valid latency samples. */
  isLatencySample: boolean;
  toolCalls: number;
}

/**
 * Parse NDJSON into turns.
 *
 * Tolerates a truncated final line (the app can be killed mid-write) but NOT
 * a corrupt interior line, which would indicate the atomic-append contract in
 * `src/ai/phase_trace.rs` has regressed and every number is suspect.
 */
export function parseTrace(text: string): {
  turns: Turn[];
  corruptLines: number;
  /**
   * Surfaces whose turns overlapped under one run id, so their phase records
   * cannot be attributed to individual turns. Their numbers are refused rather
   * than reported.
   */
  ambiguousSurfaces: Set<string>;
} {
  const lines = text.split("\n").filter((line) => line.trim().length > 0);
  const records: TraceRecord[] = [];
  let corruptLines = 0;
  lines.forEach((line, index) => {
    try {
      records.push(JSON.parse(line) as TraceRecord);
    } catch {
      // A broken LAST line is a kill mid-write; anywhere else means spliced
      // records, which is a real defect worth surfacing loudly.
      if (index !== lines.length - 1) corruptLines += 1;
    }
  });

  const byRun = new Map<string, TraceRecord[]>();
  for (const record of records) {
    // runId is reused across turns on the same connection/session, so key by
    // runId AND the turn_start boundary. Records are appended in order.
    const list = byRun.get(record.runId) ?? [];
    list.push(record);
    byRun.set(record.runId, list);
  }

  const turns: Turn[] = [];
  const ambiguousSurfaces = new Set<string>();
  for (const [runId, all] of byRun) {
    // Overlap check BEFORE splitting, because splitting is what hides the
    // problem. Grouping by runId and cutting at turn_start is only valid while
    // a run's turns are sequential. When they are not — the focused-text
    // variation fan-out runs up to three Mini turns at once — a second
    // turn_start arrives before the first terminal, and every later record is
    // filed under whichever group happens to be open. The medians that come
    // out are confident and meaningless.
    //
    // This silently produced a full Mini row in a committed receipt, so the
    // analyzer now refuses that surface instead of averaging the wreckage.
    // Ambiguity is specifically OVERLAP: a turn_start arriving before the
    // previous turn under this id terminated. Only then can a record belong to
    // more than one turn.
    //
    // Two things that look similar are deliberately NOT ambiguous:
    //   - a reused id whose turns are strictly SEQUENTIAL. Ordering resolves
    //     those, which is exactly what splitting at turn_start does.
    //   - a lone turn that never terminated. It is attributable, merely
    //     incomplete, and isLatencySample already keeps it out of the medians.
    // Both were flagged by earlier versions of this check, which withheld good
    // numbers from Text and from any trace whose last turn was still in flight
    // when the app closed.
    let open = 0;
    let overlapped = false;
    for (const record of all) {
      if (record.event === "turn_start") {
        open += 1;
        if (open > 1) overlapped = true;
      }
      if (record.event === "terminal") open -= 1;
    }
    if (overlapped) {
      const surface = all[0]?.surface ?? "unknown";
      ambiguousSurfaces.add(surface);
    }

    // Split into turns at each turn_start.
    const groups: TraceRecord[][] = [];
    for (const record of all) {
      if (record.event === "turn_start" || groups.length === 0) groups.push([]);
      groups[groups.length - 1].push(record);
    }
    for (const group of groups) {
      const at = (event: string) =>
        group.find((record) => record.event === event)?.elapsedMs ?? null;
      const start = at("turn_start") ?? 0;
      const terminal = group.find((record) => record.event === "terminal");
      const rel = (value: number | null) => (value === null ? null : value - start);
      turns.push({
        runId,
        surface: group[0]?.surface ?? "unknown",
        transport: group[0]?.transport ?? "unknown",
        ttfpe: rel(at("first_provider_event")),
        ttfvo: rel(at("first_visible_output")),
        ttft: rel(at("first_thought")),
        ttt: rel(terminal?.elapsedMs ?? null),
        outcome: (terminal?.outcome as string) ?? null,
        failureCode: (terminal?.failureCode as string) ?? null,
        isLatencySample: terminal?.isLatencySample === true,
        toolCalls: Number(terminal?.toolCalls ?? 0),
      });
    }
  }
  return { turns, corruptLines, ambiguousSurfaces };
}

export function median(values: number[]): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
}

/** Interquartile range. Reported instead of stddev because these are medians. */
export function iqr(values: number[]): [number, number] | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const pick = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
  return [pick(0.25), pick(0.75)];
}

export type Verdict =
  | "actually-slow"
  | "feels-slow"
  | "actually-and-feels-slow"
  | "fast"
  | "insufficient-data"
  | "ambiguous-trace";

export interface SurfaceReport {
  surface: string;
  transport: string;
  totalTurns: number;
  validSamples: number;
  failedTurns: number;
  cancelledTurns: number;
  failureCodes: Record<string, number>;
  medianTtfvo: number | null;
  medianTtfpe: number | null;
  medianTtt: number | null;
  iqrTtt: [number, number] | null;
  iqrTtfvo: [number, number] | null;
  /** Fraction of the turn the user spent looking at nothing. */
  deadAirRatio: number | null;
  verdict: Verdict;
  evidence: string;
}

/**
 * The operational rule, stated so it is computed rather than judged.
 *
 * The two axes are independent on purpose. A surface that answers in 800ms but
 * shows nothing for 700ms of it is not "slow" in any measurable sense, yet it
 * feels unresponsive — and the fix for that (early feedback) is completely
 * different from the fix for a genuinely long turn (less work, faster model).
 * Collapsing them into one score would point at the wrong repair.
 */
export function classify(medianTtt: number | null, medianTtfvo: number | null, validSamples: number): Verdict {
  if (validSamples < MIN_SAMPLES || medianTtt === null) return "insufficient-data";
  const actually = medianTtt >= SLOW_TURN_MS;
  const feels = medianTtfvo !== null && medianTtfvo >= FEELS_SLOW_MS;
  if (actually && feels) return "actually-and-feels-slow";
  if (actually) return "actually-slow";
  if (feels) return "feels-slow";
  return "fast";
}

export function reportForSurface(
  surface: string,
  turns: Turn[],
  ambiguousSurfaces: ReadonlySet<string> = new Set(),
): SurfaceReport {
  const mine = turns.filter((turn) => turn.surface === surface);
  const valid = mine.filter((turn) => turn.isLatencySample);
  const ttts = valid.map((turn) => turn.ttt).filter((value): value is number => value !== null);
  const ttfvos = valid.map((turn) => turn.ttfvo).filter((value): value is number => value !== null);
  const ttfpes = valid.map((turn) => turn.ttfpe).filter((value): value is number => value !== null);

  const failureCodes: Record<string, number> = {};
  for (const turn of mine) {
    if (turn.outcome === "failed") {
      const code = turn.failureCode ?? "Unclassified";
      failureCodes[code] = (failureCodes[code] ?? 0) + 1;
    }
  }

  const medianTtt = median(ttts);
  const medianTtfvo = median(ttfvos);
  // An ambiguous trace outranks every other verdict, including
  // insufficient-data: the samples exist, they are simply not attributable to
  // individual turns, so reporting their median would be worse than reporting
  // nothing. Refusing is the whole point — the previous version happily
  // published a Mini median built from three interleaved turns.
  const ambiguous = ambiguousSurfaces.has(surface);
  const verdict: Verdict = ambiguous
    ? "ambiguous-trace"
    : classify(medianTtt, medianTtfvo, valid.length);
  const deadAirRatio =
    medianTtt !== null && medianTtfvo !== null && medianTtt > 0 ? medianTtfvo / medianTtt : null;

  let evidence: string;
  switch (verdict) {
    case "ambiguous-trace":
      evidence = `turns overlapped under a shared runId, so phases cannot be attributed to individual turns. ${mine.length} turn(s) seen; numbers withheld. Fix the transport to mint a per-turn runId (src/ai/phase_trace.rs begin_at), then re-measure.`;
      break;
    case "insufficient-data":
      evidence = `only ${valid.length} valid sample(s); ${MIN_SAMPLES} required. ${mine.length} turn(s) seen, ${mine.length - valid.length} not usable as latency samples.`;
      break;
    case "actually-and-feels-slow":
      evidence = `median turn ${medianTtt}ms (>= ${SLOW_TURN_MS}ms) AND ${medianTtfvo}ms of dead air before any answer text (>= ${FEELS_SLOW_MS}ms).`;
      break;
    case "actually-slow":
      evidence = `median turn ${medianTtt}ms (>= ${SLOW_TURN_MS}ms); first output at ${medianTtfvo}ms, so the wait is real work, not silence.`;
      break;
    case "feels-slow":
      evidence = `median turn only ${medianTtt}ms, but nothing visible for ${medianTtfvo}ms (>= ${FEELS_SLOW_MS}ms) — ${deadAirRatio !== null ? Math.round(deadAirRatio * 100) : "?"}% of the turn is dead air.`;
      break;
    default:
      evidence = `median turn ${medianTtt}ms with first output at ${medianTtfvo}ms; under both thresholds.`;
  }

  return {
    surface,
    transport: mine[0]?.transport ?? "unknown",
    totalTurns: mine.length,
    validSamples: valid.length,
    failedTurns: mine.filter((turn) => turn.outcome === "failed").length,
    cancelledTurns: mine.filter((turn) => turn.outcome === "cancelled").length,
    failureCodes,
    medianTtfvo,
    medianTtfpe: median(ttfpes),
    medianTtt,
    iqrTtt: iqr(ttts),
    iqrTtfvo: iqr(ttfvos),
    deadAirRatio,
    verdict,
    evidence,
  };
}

export const KNOWN_SURFACES = ["quick-ai", "agent-chat", "text", "mini", "flow"] as const;

export function buildReport(text: string) {
  const { turns, corruptLines, ambiguousSurfaces } = parseTrace(text);
  const surfaces = KNOWN_SURFACES.map((surface) =>
    reportForSurface(surface, turns, ambiguousSurfaces),
  );
  return { turns, corruptLines, surfaces, ambiguousSurfaces };
}

function formatMs(value: number | null): string {
  return value === null ? "     —" : `${String(Math.round(value)).padStart(6)}`;
}

function main() {
  const path = process.argv[2] ?? process.env.SCRIPT_KIT_AI_TRACE_PATH;
  if (!path) {
    console.error("usage: bun scripts/agentic/ai-phase-trace-report.ts <trace.ndjson>");
    console.error("   or: SCRIPT_KIT_AI_TRACE_PATH=<path> bun scripts/agentic/ai-phase-trace-report.ts");
    process.exit(2);
  }
  if (!existsSync(path)) {
    console.error(`NO_TRACE_FILE path=${path}`);
    console.error("Nothing has written a trace yet. Run the app with SCRIPT_KIT_AI_TRACE_PATH set.");
    process.exit(3);
  }

  const { turns, corruptLines, surfaces } = buildReport(readFileSync(path, "utf8"));

  console.log(`# AI phase trace report`);
  console.log(`trace=${path}`);
  console.log(`turns=${turns.length} corruptLines=${corruptLines}`);
  if (corruptLines > 0) {
    console.log(
      `WARNING: ${corruptLines} corrupt interior line(s). The single-write_all append contract in src/ai/phase_trace.rs may have regressed; treat these numbers as unreliable.`,
    );
  }
  console.log("");
  console.log("surface     transport          n  valid  ttfpe  ttfvo    ttt  verdict");
  console.log("---------------------------------------------------------------------------");
  for (const report of surfaces) {
    // Withheld means withheld. Printing the medians beside "ambiguous-trace"
    // would leave them on screen to be copied into a summary, which is exactly
    // how the bad Mini row travelled in the first place.
    const withheld = report.verdict === "ambiguous-trace";
    const cell = (value: number | null) => (withheld ? "     —" : formatMs(value));
    console.log(
      [
        report.surface.padEnd(11),
        report.transport.padEnd(17),
        String(report.totalTurns).padStart(2),
        String(report.validSamples).padStart(6),
        cell(report.medianTtfpe),
        cell(report.medianTtfvo),
        cell(report.medianTtt),
        ` ${report.verdict}`,
      ].join(" "),
    );
  }
  console.log("");
  console.log("## Verdict evidence");
  for (const report of surfaces) {
    console.log(`- ${report.surface}: ${report.evidence}`);
    if (report.failedTurns > 0) {
      console.log(
        `    failed=${report.failedTurns} codes=${JSON.stringify(report.failureCodes)} (excluded from latency)`,
      );
    }
  }
  console.log("");
  console.log(
    `Thresholds: actually-slow when median turn >= ${SLOW_TURN_MS}ms; feels-slow when median time-to-first-visible-output >= ${FEELS_SLOW_MS}ms; both need n >= ${MIN_SAMPLES}.`,
  );

  // An ambiguous surface is NOT measured. Counting it was how the scoreboard
  // read 4/5 while one of those four was unusable.
  const measured = surfaces.filter(
    (report) =>
      report.verdict !== "insufficient-data" && report.verdict !== "ambiguous-trace",
  );
  console.log(`\nMEASURED_SURFACES=${measured.length}/${surfaces.length}`);
}

if (import.meta.main) main();
