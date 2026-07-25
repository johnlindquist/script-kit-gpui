import { describe, expect, test } from "bun:test";
import {
  buildReport,
  classify,
  FEELS_SLOW_MS,
  iqr,
  median,
  MIN_SAMPLES,
  parseTrace,
  reportForSurface,
  SLOW_TURN_MS,
} from "./ai-phase-trace-report.ts";

function record(fields: Record<string, unknown>): string {
  return JSON.stringify({
    schemaVersion: 1,
    runId: "r",
    surface: "agent-chat",
    transport: "pi-rpc",
    seq: 1,
    ...fields,
  });
}

/** One completed turn with the given phase offsets. */
function turnLines(
  options: {
    runId?: string;
    surface?: string;
    ttfvo?: number;
    ttt: number;
    outcome?: string;
    failureCode?: string;
  },
): string[] {
  const { runId = "r", surface = "agent-chat", ttfvo = 100, ttt, outcome = "completed" } = options;
  const lines = [
    record({ runId, surface, event: "turn_start", elapsedMs: 0 }),
    record({ runId, surface, event: "first_provider_event", elapsedMs: Math.min(50, ttfvo) }),
  ];
  if (outcome === "completed") {
    lines.push(record({ runId, surface, event: "first_visible_output", elapsedMs: ttfvo }));
  }
  lines.push(
    record({
      runId,
      surface,
      event: "terminal",
      elapsedMs: ttt,
      outcome,
      isLatencySample: outcome === "completed",
      toolCalls: 0,
      ...(options.failureCode ? { failureCode: options.failureCode } : {}),
    }),
  );
  return lines;
}

describe("parseTrace", () => {
  test("splits one runId into separate turns at each turn_start", () => {
    // Pi reuses a runId across turns on the same connection, so failing to
    // split here would fuse several turns into one impossibly long sample.
    const text = [...turnLines({ ttt: 1000 }), ...turnLines({ ttt: 2000 })].join("\n");
    const { turns } = parseTrace(text);
    expect(turns).toHaveLength(2);
    expect(turns.map((turn) => turn.ttt)).toEqual([1000, 2000]);
  });

  test("measures phases RELATIVE to turn_start, not to the trace file", () => {
    // The second turn on a connection starts at a large elapsedMs. Reading the
    // raw value would report a 10-second turn that actually took 1 second.
    const text = [
      record({ event: "turn_start", elapsedMs: 10_000 }),
      record({ event: "first_visible_output", elapsedMs: 10_200 }),
      record({ event: "terminal", elapsedMs: 11_000, outcome: "completed", isLatencySample: true }),
    ].join("\n");
    const { turns } = parseTrace(text);
    expect(turns[0].ttfvo).toBe(200);
    expect(turns[0].ttt).toBe(1000);
  });

  test("tolerates a truncated final line but counts corrupt interior lines", () => {
    // A killed app truncates the last line; a spliced interior line means the
    // atomic-append contract regressed and every number is suspect.
    const good = turnLines({ ttt: 500 });
    const truncated = [...good, '{"schemaVersion":1,"ru'].join("\n");
    expect(parseTrace(truncated).corruptLines).toBe(0);

    const spliced = [good[0], '{"broken":', ...good.slice(1)].join("\n");
    expect(parseTrace(spliced).corruptLines).toBe(1);
  });
});

describe("statistics", () => {
  test("median handles even and odd counts", () => {
    expect(median([3, 1, 2])).toBe(2);
    expect(median([4, 1, 3, 2])).toBe(2.5);
    expect(median([])).toBeNull();
  });

  test("iqr reports spread so a median is never shown bare", () => {
    expect(iqr([1, 2, 3, 4, 5, 6, 7, 8])).toEqual([3, 7]);
  });
});

describe("classify", () => {
  test("refuses a verdict below the minimum sample count", () => {
    expect(classify(9_000, 8_000, MIN_SAMPLES - 1)).toBe("insufficient-data");
  });

  test("a long turn with prompt output is actually-slow, not feels-slow", () => {
    expect(classify(SLOW_TURN_MS + 1, 200, MIN_SAMPLES)).toBe("actually-slow");
  });

  test("a short turn with a long silence is feels-slow", () => {
    // This is the case the whole distinction exists for: the surface is not
    // slow, it is silent, and the repair is early feedback rather than speed.
    expect(classify(2_000, FEELS_SLOW_MS + 1, MIN_SAMPLES)).toBe("feels-slow");
  });

  test("both axes can trip together", () => {
    expect(classify(SLOW_TURN_MS + 1, FEELS_SLOW_MS + 1, MIN_SAMPLES)).toBe(
      "actually-and-feels-slow",
    );
  });

  test("fast on both axes is fast", () => {
    expect(classify(SLOW_TURN_MS - 1, FEELS_SLOW_MS - 1, MIN_SAMPLES)).toBe("fast");
  });
});

describe("reportForSurface", () => {
  test("failed turns are counted but never enter the latency medians", () => {
    // Pi died in ~800ms in a sandbox home. Without this exclusion a broken
    // surface reports as the fastest one in the table.
    const text = [
      ...Array.from({ length: 5 }, (_, index) =>
        turnLines({ runId: `ok-${index}`, ttt: 4_000 }),
      ).flat(),
      ...turnLines({ runId: "bad", ttt: 800, outcome: "failed", failureCode: "RuntimeClosed" }),
    ].join("\n");
    const { turns } = parseTrace(text);
    const report = reportForSurface("agent-chat", turns);

    expect(report.totalTurns).toBe(6);
    expect(report.validSamples).toBe(5);
    expect(report.failedTurns).toBe(1);
    expect(report.failureCodes).toEqual({ RuntimeClosed: 1 });
    expect(report.medianTtt).toBe(4_000);
  });

  test("cancelled turns are excluded too and reported separately", () => {
    const text = [
      ...turnLines({ runId: "c", ttt: 300, outcome: "cancelled" }),
      ...Array.from({ length: 5 }, (_, index) =>
        turnLines({ runId: `ok-${index}`, ttt: 2_000 }),
      ).flat(),
    ].join("\n");
    const report = reportForSurface("agent-chat", parseTrace(text).turns);
    expect(report.cancelledTurns).toBe(1);
    expect(report.validSamples).toBe(5);
    expect(report.medianTtt).toBe(2_000);
  });

  test("dead-air ratio surfaces a silent-but-quick turn", () => {
    const text = Array.from({ length: 5 }, (_, index) =>
      turnLines({ runId: `q-${index}`, ttfvo: 1_800, ttt: 2_000 }),
    )
      .flat()
      .join("\n");
    const report = reportForSurface("agent-chat", parseTrace(text).turns);
    expect(report.verdict).toBe("feels-slow");
    expect(report.deadAirRatio).toBeCloseTo(0.9, 2);
  });

  test("a surface with no turns reports insufficient-data, never fast", () => {
    // The dangerous failure: an unwired surface writes nothing, and silence
    // must never be read as speed.
    const report = reportForSurface("flow", parseTrace("").turns);
    expect(report.verdict).toBe("insufficient-data");
    expect(report.medianTtt).toBeNull();
  });
});

describe("buildReport", () => {
  test("separates surfaces that share a transport", () => {
    // Agent Chat, Text, and Mini all ride pi-rpc. Pooling them would hide a
    // slow surface behind two fast ones.
    const text = [
      ...Array.from({ length: 5 }, (_, index) =>
        turnLines({ runId: `a-${index}`, surface: "agent-chat", ttt: 1_000 }),
      ).flat(),
      ...Array.from({ length: 5 }, (_, index) =>
        turnLines({ runId: `m-${index}`, surface: "mini", ttt: 9_000 }),
      ).flat(),
    ].join("\n");
    const { surfaces } = buildReport(text);
    const agentChat = surfaces.find((surface) => surface.surface === "agent-chat")!;
    const mini = surfaces.find((surface) => surface.surface === "mini")!;
    expect(agentChat.medianTtt).toBe(1_000);
    expect(agentChat.verdict).toBe("fast");
    expect(mini.medianTtt).toBe(9_000);
    expect(mini.verdict).toBe("actually-slow");
  });

  test("always reports every known surface, including unmeasured ones", () => {
    const { surfaces } = buildReport("");
    expect(surfaces.map((surface) => surface.surface)).toEqual([
      "quick-ai",
      "agent-chat",
      "text",
      "mini",
      "flow",
    ]);
    expect(surfaces.every((surface) => surface.verdict === "insufficient-data")).toBe(true);
  });
});

describe("overlapping turns under one runId", () => {
  /**
   * The defect that made a committed Mini row unusable.
   *
   * Splitting at `turn_start` is only valid while a run's turns are sequential.
   * The focused-text variation fan-out opens several Mini turns at once under
   * the constant id `"pi-isolated"`, so their phases interleave and every
   * median built from them describes a turn nobody ran.
   */
  const overlapping = [
    record({ runId: "pi-isolated", surface: "mini", event: "turn_start", elapsedMs: 0 }),
    // Second turn opens BEFORE the first terminates — the whole problem.
    record({ runId: "pi-isolated", surface: "mini", event: "turn_start", elapsedMs: 10 }),
    record({
      runId: "pi-isolated",
      surface: "mini",
      event: "first_visible_output",
      elapsedMs: 900,
    }),
    record({
      runId: "pi-isolated",
      surface: "mini",
      event: "terminal",
      elapsedMs: 1000,
      outcome: "completed",
      isLatencySample: true,
      toolCalls: 0,
    }),
    record({
      runId: "pi-isolated",
      surface: "mini",
      event: "terminal",
      elapsedMs: 1100,
      outcome: "completed",
      isLatencySample: true,
      toolCalls: 0,
    }),
  ].join("\n");

  test("are flagged as ambiguous rather than averaged", () => {
    const { ambiguousSurfaces } = parseTrace(overlapping);
    expect(ambiguousSurfaces.has("mini")).toBe(true);
  });

  test("produce an ambiguous-trace verdict that withholds the median", () => {
    const { surfaces } = buildReport(overlapping);
    const mini = surfaces.find((surface) => surface.surface === "mini");
    expect(mini?.verdict).toBe("ambiguous-trace");
    expect(mini?.evidence).toContain("overlapped");
  });

  test("ambiguity outranks a would-be verdict, so a bad median never escapes", () => {
    // Enough samples that classify() would happily return a real verdict.
    const many = Array.from({ length: MIN_SAMPLES + 2 }, () =>
      turnLines({ runId: "shared", surface: "mini", ttt: 1000, ttfvo: 900 }),
    ).flat();
    // One overlap anywhere in the surface is enough to distrust its numbers.
    many.splice(
      1,
      0,
      record({ runId: "shared", surface: "mini", event: "turn_start", elapsedMs: 5 }),
    );
    const { surfaces } = buildReport(many.join("\n"));
    const mini = surfaces.find((surface) => surface.surface === "mini");
    expect(mini?.verdict).toBe("ambiguous-trace");
  });

  test("a sequential surface in the same trace keeps its real verdict", () => {
    // The refusal must be scoped to the broken surface, or one bad surface
    // would erase every good number in the file.
    const text = [
      overlapping,
      // Unique id per turn, which is what the transport mints now.
      ...Array.from({ length: MIN_SAMPLES }, (_unused, index) =>
        turnLines({
          runId: `pi-1#${index}`,
          surface: "text",
          ttt: 4000,
          ttfvo: 3800,
        }).join("\n"),
      ),
    ].join("\n");
    const { surfaces } = buildReport(text);
    expect(surfaces.find((s) => s.surface === "mini")?.verdict).toBe("ambiguous-trace");
    expect(surfaces.find((s) => s.surface === "text")?.verdict).toBe("feels-slow");
  });

  test("a lone unterminated turn is NOT ambiguous", () => {
    // Regression: the first version of this guard flagged any unterminated
    // turn, which meant every trace whose last turn was still in flight when
    // the app closed got its whole surface withheld. A single turn under its
    // own id is perfectly attributable — it is just an incomplete sample, and
    // isLatencySample already excludes it from the medians.
    const text = [
      record({ runId: "pi-1#1", surface: "text", event: "turn_start", elapsedMs: 0 }),
      record({ runId: "pi-1#1", surface: "text", event: "first_visible_output", elapsedMs: 50 }),
    ].join("\n");
    const { ambiguousSurfaces } = parseTrace(text);
    expect(ambiguousSurfaces.has("text")).toBe(false);
  });

  test("a reused id with SEQUENTIAL turns is not ambiguous", () => {
    // Regression: an over-strict version flagged any reused id, which withheld
    // Text's perfectly good numbers. Sequential turns are resolved by ordering
    // — that is precisely what splitting at turn_start does. Only overlap is
    // unattributable.
    const text = [
      ...turnLines({ runId: "pi-1", surface: "text", ttt: 1000 }),
      ...turnLines({ runId: "pi-1", surface: "text", ttt: 1200 }),
    ].join("\n");
    const { ambiguousSurfaces, turns } = parseTrace(text);
    expect(ambiguousSurfaces.has("text")).toBe(false);
    expect(turns).toHaveLength(2);
  });
});
