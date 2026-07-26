/**
 * WP11 (glass-smoke-harness-max-info): direct locks over the lifecycle
 * filmstrip CONTRACT module — the pure decision layer every capture probe
 * shares. The invariant family: profiles resolve to exact scenario sets,
 * deferred analysis can never look green, ANALYSIS_PENDING never becomes a
 * pass, interference always dominates, and timing intervals must be
 * monotone and non-overlapping.
 */

import { describe, expect, test } from "bun:test";
import {
  computeLifecycleDisposition,
  filmstripVerdict,
  LEGACY_FULL_SCENARIO_ORDER,
  parseAnalysisMode,
  resolveScenarioNames,
  SCENARIO_CAPTURE_DURATIONS_MS,
  validateScenarioTimingIntervals,
} from "./glass-lifecycle-filmstrip-contract.ts";

describe("scenario profiles", () => {
  test("full and extended resolve to the exact legacy five in order", () => {
    expect(resolveScenarioNames("full")).toEqual([
      ...LEGACY_FULL_SCENARIO_ORDER,
    ]);
    expect(resolveScenarioNames("extended")).toEqual([
      ...LEGACY_FULL_SCENARIO_ORDER,
    ]);
  });

  test("entry-color is exactly main-exit then main-entry", () => {
    expect(resolveScenarioNames("entry-color")).toEqual([
      "main-exit",
      "main-entry",
    ]);
  });

  test("an unknown profile throws instead of silently substituting", () => {
    expect(() => resolveScenarioNames("bogus" as any)).toThrow();
  });

  test("every scenario has a locked capture duration", () => {
    for (const name of LEGACY_FULL_SCENARIO_ORDER) {
      expect(SCENARIO_CAPTURE_DURATIONS_MS[name]).toBeGreaterThan(0);
    }
  });
});

describe("analysis modes and verdicts", () => {
  test("analysis mode parsing fails closed", () => {
    expect(parseAnalysisMode("inline")).toBe("inline");
    expect(parseAnalysisMode("deferred")).toBe("deferred");
    expect(() => parseAnalysisMode("later" as any)).toThrow();
  });

  test("a deferred filmstrip can never report pass", () => {
    const verdict = filmstripVerdict({
      captureErrorCount: 0,
      analysisMode: "deferred",
      metricsExitCode: null,
      metricsPass: null,
    });
    expect(verdict.capturePass).toBe(true);
    expect(verdict.analysisState).toBe("pending");
    expect(verdict.pass).toBe(false);
  });

  test("inline pass requires clean capture AND green metrics", () => {
    const green = filmstripVerdict({
      captureErrorCount: 0,
      analysisMode: "inline",
      metricsExitCode: 0,
      metricsPass: true,
    });
    expect(green.pass).toBe(true);
    const badMetrics = filmstripVerdict({
      captureErrorCount: 0,
      analysisMode: "inline",
      metricsExitCode: 1,
      metricsPass: false,
    });
    expect(badMetrics.pass).toBe(false);
    const badCapture = filmstripVerdict({
      captureErrorCount: 2,
      analysisMode: "inline",
      metricsExitCode: 0,
      metricsPass: true,
    });
    expect(badCapture.capturePass).toBe(false);
    expect(badCapture.pass).toBe(false);
  });
});

describe("lifecycle dispositions", () => {
  test("interference dominates everything", () => {
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: "INVALID_INTERFERENCE",
        analysisState: "inline",
        pass: true,
        hasObserverError: false,
      }),
    ).toBe("INVALID_INTERFERENCE");
  });

  test("pending analysis is ANALYSIS_PENDING, never green", () => {
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: "EVALUABLE_PASS",
        analysisState: "pending",
        pass: false,
        hasObserverError: false,
      }),
    ).toBe("ANALYSIS_PENDING");
  });

  test("observer errors are INVALID_OBSERVER, not product failures", () => {
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: "EVALUABLE_PASS",
        analysisState: "inline",
        pass: false,
        hasObserverError: true,
      }),
    ).toBe("INVALID_OBSERVER");
  });

  test("evaluable outcomes resolve by pass", () => {
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: "EVALUABLE_PASS",
        analysisState: "inline",
        pass: true,
        hasObserverError: false,
      }),
    ).toBe("EVALUABLE_PASS");
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: "EVALUABLE_PASS",
        analysisState: "inline",
        pass: false,
        hasObserverError: false,
      }),
    ).toBe("EVALUABLE_FAIL");
  });
});

describe("scenario timing intervals", () => {
  test("monotone non-overlapping intervals validate", () => {
    expect(
      validateScenarioTimingIntervals([
        { name: "main-exit", startedAtMs: 0, finishedAtMs: 10 },
        { name: "main-entry", startedAtMs: 10, finishedAtMs: 30 },
      ]),
    ).toEqual([]);
  });

  test("overlapping or reversed intervals are rejected", () => {
    expect(
      validateScenarioTimingIntervals([
        { name: "main-exit", startedAtMs: 0, finishedAtMs: 20 },
        { name: "main-entry", startedAtMs: 10, finishedAtMs: 30 },
      ]).length,
    ).toBeGreaterThan(0);
    expect(
      validateScenarioTimingIntervals([
        { name: "main-exit", startedAtMs: 10, finishedAtMs: 5 },
      ]).length,
    ).toBeGreaterThan(0);
  });
});
