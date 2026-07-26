/**
 * WP6 (glass-smoke-harness-max-info): runtime-contract, load, memory, and
 * interference telemetry must be fail-closed. A mislabeled artifact is
 * INVALID_SETUP, an unparseable memory-pressure state is "unavailable"
 * (never silently "normal"), thermal health without a parsed CPU speed
 * limit never passes, and scenario attribution of interference events is
 * explicitly rejected when the monitor cannot timestamp them.
 */

import { describe, expect, test } from "bun:test";
import {
  alphaFromBits,
  alphaToBitsHex,
  BOUNDARY_LOAD_LIMIT,
  checkRuntimeContract,
  interferenceStatistics,
  parseCpuSpeedLimit,
  parseMemoryPressure,
  parseMorphEnterLogs,
  parsePsLine,
  parseSwapUsage,
  parseVmStatFreeBytes,
  percentile,
  probeGpuTelemetry,
  startSampler,
  summarizeTelemetry,
  worstMemoryPressure,
} from "./glass-system-telemetry.ts";

const INSTRUMENTED_LINE =
  "event=glass_morph window=main variant=window_frame phase=enter " +
  "duration=0.28s inset=0.030 start_alpha=0.85 " +
  "start_alpha_bits=3feb333333333333 settle_duration_ns=280000000 " +
  "configured_at_host_time_ns=225190135124625 " +
  "expected_settle_deadline_ns=225190415124625 " +
  "frames=680x400->700x410->690x405";

const LEGACY_LINE =
  "event=glass_morph window=main variant=window_frame phase=enter " +
  "duration=0.28s inset=0.030 start_alpha=0.85 frames=680x400->700x410->690x405";

describe("runtime contract parsing", () => {
  test("instrumented enter lines parse with exact bits and clocks", () => {
    const rows = parseMorphEnterLogs(
      `noise\n${INSTRUMENTED_LINE}\nmore noise`,
    );
    expect(rows).toHaveLength(1);
    expect(rows[0].windowName).toBe("main");
    expect(rows[0].observedMorphStartAlphaBits).toBe("3feb333333333333");
    expect(rows[0].observedMorphStartAlpha).toBeCloseTo(0.85, 12);
    expect(rows[0].configuredDurationNs).toBe(280_000_000);
    expect(rows[0].expectedSettleDeadlineNs - rows[0].configuredAtHostTimeNs)
      .toBe(280_000_000);
  });

  test("legacy uninstrumented lines never parse — an old binary yields zero rows", () => {
    expect(parseMorphEnterLogs(LEGACY_LINE)).toHaveLength(0);
  });

  test("alpha bits roundtrip exactly", () => {
    expect(alphaToBitsHex(0.85)).toBe("3feb333333333333");
    expect(alphaFromBits(alphaToBitsHex(0.9))).toBe(0.9);
    expect(alphaToBitsHex(0)).toBe("0000000000000000");
  });
});

describe("runtime contract comparison", () => {
  const declared = { declaredMorphStartAlpha: 0.85, expectedDurationNs: 280_000_000 };

  test("matching bits and duration pass", () => {
    const result = checkRuntimeContract(
      declared,
      parseMorphEnterLogs(INSTRUMENTED_LINE),
      "main",
    );
    expect(result.pass).toBe(true);
    expect(result.alphaBitsMatch).toBe(true);
    expect(result.durationMatch).toBe(true);
    expect(result.disposition).toBe("EVALUABLE");
  });

  test("an alpha-bits mismatch is INVALID_SETUP, never a product verdict", () => {
    const result = checkRuntimeContract(
      { declaredMorphStartAlpha: 0.0, expectedDurationNs: 280_000_000 },
      parseMorphEnterLogs(INSTRUMENTED_LINE),
      "main",
    );
    expect(result.pass).toBe(false);
    expect(result.disposition).toBe("INVALID_SETUP");
    expect(result.errors.join(" ")).toContain("do not match declared");
  });

  test("zero observed lines is INVALID_SETUP — old binary cannot silently pass", () => {
    const result = checkRuntimeContract(declared, [], "main");
    expect(result.pass).toBe(false);
    expect(result.disposition).toBe("INVALID_SETUP");
    expect(result.observedLineCount).toBe(0);
  });

  test("mixed start-alpha bits within one run is a hard error", () => {
    const other = INSTRUMENTED_LINE.replace(
      "start_alpha_bits=3feb333333333333",
      "start_alpha_bits=3fec000000000000",
    );
    const result = checkRuntimeContract(
      declared,
      parseMorphEnterLogs(`${INSTRUMENTED_LINE}\n${other}`),
      "main",
    );
    expect(result.pass).toBe(false);
    expect(result.errors.join(" ")).toContain("distinct start-alpha bit patterns");
  });
});

describe("system parsers fail closed", () => {
  test("memory pressure parses explicit levels and NEVER invents normal", () => {
    expect(parseMemoryPressure("The system memory pressure status: normal\n"))
      .toBe("normal");
    expect(parseMemoryPressure("memory pressure: critical")).toBe("critical");
    expect(parseMemoryPressure("System-wide memory free percentage: 42%\n"))
      .toBe("unavailable");
    expect(parseMemoryPressure("")).toBe("unavailable");
    expect(parseMemoryPressure("garbage output")).toBe("unavailable");
  });

  test("worst memory pressure ranks critical > warn > unavailable > normal", () => {
    expect(worstMemoryPressure(["normal", "critical"])).toBe("critical");
    expect(worstMemoryPressure(["normal", "warn"])).toBe("warn");
    expect(worstMemoryPressure(["normal", "unavailable"])).toBe("unavailable");
    expect(worstMemoryPressure(["normal", "normal"])).toBe("normal");
  });

  test("swap, therm, vm_stat, and ps parsers handle real shapes and reject garbage", () => {
    expect(parseSwapUsage("total = 2048.00M  used = 1234.50M  free = 813.50M"))
      .toBe(Math.round(1234.5 * 1024 ** 2));
    expect(parseSwapUsage("nonsense")).toBeNull();
    expect(parseCpuSpeedLimit("CPU_Speed_Limit \t= 100")).toBe(100);
    expect(parseCpuSpeedLimit("")).toBeNull();
    // Apple Silicon: pmset's explicit no-throttle note parses as nominal
    // (100); it is a positive statement, not an unknown. Anything else
    // (empty above, garbage below) stays null so thermal fails closed.
    expect(
      parseCpuSpeedLimit(
        "Note: No thermal warning level has been recorded\nNote: No performance warning level has been recorded\nNote: No CPU power status has been recorded\n",
      ),
    ).toBe(100);
    expect(parseCpuSpeedLimit("pmset: unrecognized output")).toBeNull();
    // A real numeric line always wins over the note.
    expect(
      parseCpuSpeedLimit(
        "CPU_Speed_Limit = 60\nNote: No CPU power status has been recorded\n",
      ),
    ).toBe(60);
    expect(
      parseVmStatFreeBytes(
        "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free: 1000.\n",
      ),
    ).toBe(16384 * 1000);
    expect(parseVmStatFreeBytes("")).toBeNull();
    expect(parsePsLine(" 123456 987654 12.3 4.5\n")).toEqual({
      rssBytes: 123456 * 1024,
      virtualBytes: 987654 * 1024,
      cpuPercent: 12.3,
      memPercent: 4.5,
    });
    expect(parsePsLine("")).toBeNull();
  });
});

describe("telemetry summary", () => {
  const edge = (cpuSpeedLimit: number | null, pressure: string) =>
    ({
      atUnixMs: 0,
      label: "edge",
      therm: {} as any,
      cpuSpeedLimit,
      memoryPressureRaw: {} as any,
      memoryPressure: pressure,
      uptime: {} as any,
      topProcesses: {} as any,
    }) as any;
  const sampler = {
    sampleCount: 4,
    intervalMs: 250,
    load1Samples: [2, 3, 4, 5],
    freeMemSamples: [100, 90, 80, 85],
    load1P50: 3,
    load1P95: 5,
    load1Maximum: 5,
    freeBytesMinimum: 80,
  };

  test("boundary load gate is preserved exactly at 6.0", () => {
    expect(BOUNDARY_LOAD_LIMIT).toBe(6.0);
    const green = summarizeTelemetry({
      pre: edge(100, "normal"),
      post: edge(100, "normal"),
      sampler,
      boundaries: [],
      preLoad1: 5.9,
      postLoad1: 6.0,
    });
    expect(green.load.boundaryPass).toBe(true);
    const red = summarizeTelemetry({
      pre: edge(100, "normal"),
      post: edge(100, "normal"),
      sampler,
      boundaries: [],
      preLoad1: 5.9,
      postLoad1: 6.01,
    });
    expect(red.load.boundaryPass).toBe(false);
  });

  test("thermal health without a parsed speed limit never passes", () => {
    const unknown = summarizeTelemetry({
      pre: edge(null, "normal"),
      post: edge(100, "normal"),
      sampler,
      boundaries: [],
      preLoad1: 1,
      postLoad1: 1,
    });
    expect(unknown.thermal.pass).toBe(false);
    const throttled = summarizeTelemetry({
      pre: edge(100, "normal"),
      post: edge(80, "normal"),
      sampler,
      boundaries: [],
      preLoad1: 1,
      postLoad1: 1,
    });
    expect(throttled.thermal.pass).toBe(false);
    expect(throttled.thermal.minimumCpuSpeedLimit).toBe(80);
  });

  test("only a positively observed critical pressure invalidates memory telemetry", () => {
    const unavailable = summarizeTelemetry({
      pre: edge(100, "unavailable"),
      post: edge(100, "normal"),
      sampler,
      boundaries: [],
      preLoad1: 1,
      postLoad1: 1,
    });
    expect(unavailable.memory.telemetryPass).toBe(true);
    expect(unavailable.memory.pressureWorst).toBe("unavailable");
    expect(unavailable.memory.pressureObservationQuality).toBe("partial");
    const critical = summarizeTelemetry({
      pre: edge(100, "normal"),
      post: edge(100, "critical"),
      sampler,
      boundaries: [],
      preLoad1: 1,
      postLoad1: 1,
    });
    expect(critical.memory.telemetryPass).toBe(false);
  });
});

describe("percentile and sampler", () => {
  test("percentile is order-insensitive and clamps", () => {
    expect(percentile([5, 1, 3], 0.5)).toBe(3);
    expect(percentile([5, 1, 3], 0.95)).toBe(5);
    expect(percentile([], 0.5)).toBe(0);
  });

  test("the in-process sampler collects real load samples", async () => {
    const handle = startSampler(50);
    await Bun.sleep(300);
    const summary = handle.stop();
    expect(summary.sampleCount).toBeGreaterThanOrEqual(3);
    expect(summary.load1Maximum).toBeGreaterThan(0);
    expect(summary.freeBytesMinimum).toBeGreaterThan(0);
  });
});

describe("gpu capability probe", () => {
  test("probe is bounded and NEVER a gate", async () => {
    const result = await probeGpuTelemetry();
    expect(["available", "unsupported", "permission-denied", "parse-unknown"])
      .toContain(result.status);
    expect(result.gate).toBe(false);
    expect(result.ioregSha256).toHaveLength(64);
  });
});

describe("interference statistics", () => {
  const intervals = [
    { name: "main-exit", startUnixMs: 1000, endUnixMs: 2000 },
    { name: "main-entry", startUnixMs: 2000, endUnixMs: 3000 },
  ];

  test("timestamped events bucket into scenario intervals", () => {
    const stats = interferenceStatistics(
      {
        untaggedInputCount: 2,
        frontmostAppChanged: false,
        pointerDeviationPx: 0,
        targetMovedExternally: false,
        eventTimestampsSupported: true,
        droppedEventCount: 0,
        events: [
          { kind: "untaggedInput", atUnixMs: 1500, atUptimeNs: 1, sampleIndex: 1 },
          { kind: "untaggedInput", atUnixMs: 2500, atUptimeNs: 2, sampleIndex: 2 },
          { kind: "untaggedInput", atUnixMs: 9999, atUptimeNs: 3, sampleIndex: 3 },
        ],
      },
      intervals,
    );
    expect(stats.scenarioAttributionSupported).toBe(true);
    expect(stats.scenarioAttribution).toEqual({
      "main-exit": 1,
      "main-entry": 1,
    });
    expect(stats.unattributedEventCount).toBe(1);
    expect(stats.causes.untaggedInput).toBe(2);
  });

  test("a receipt without timestamps explicitly rejects scenario attribution", () => {
    const stats = interferenceStatistics(
      {
        untaggedInputCount: 1,
        frontmostAppChanged: true,
        pointerDeviationPx: 5,
        targetMovedExternally: false,
      },
      intervals,
    );
    expect(stats.scenarioAttributionSupported).toBe(false);
    expect(stats.scenarioAttribution).toBeNull();
    expect(stats.scenarioAttributionRejectedReason).toContain(
      "no timestamped events",
    );
    expect(stats.causes.frontmostAppChange).toBe(1);
    expect(stats.causes.pointerDeviation).toBe(1);
  });
});
