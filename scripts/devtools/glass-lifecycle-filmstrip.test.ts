import { describe, expect, test } from "bun:test";
import {
  expandCaptureBounds,
  validateDetachedExitLifecycle,
  validateFilmstripCapture,
  type FilmstripIdentity,
} from "./glass-lifecycle-filmstrip-contract.ts";

const identity: FilmstripIdentity = {
  runId: "glass-run",
  gitCommit: "abc123",
  binarySha256: "b".repeat(64),
  pid: 42,
  windowId: 77,
};

function validReceipt() {
  return {
    schemaVersion: 2,
    status: "ok",
    captureHealthPass: true,
    runID: identity.runId,
    gitCommit: identity.gitCommit,
    binarySHA256: identity.binarySha256,
    pid: identity.pid,
    windowID: identity.windowId,
    displayID: 1,
    refreshRateHz: 120,
    captureScale: 2,
    pixelFormat: "BGRA",
    receivedSampleCount: 1,
    accountedSampleCount: 1,
    completeSampleCount: 1,
    copiedCompleteCount: 1,
    encodedCompleteCount: 1,
    incompleteSampleCount: 0,
    incompleteRenderableSampleCount: 0,
    missingDisplayTimeCount: 0,
    droppedCompleteCount: 0,
    duplicateDisplayTimeCount: 0,
    lateFrameCount: 0,
    maximumConsecutiveDisplayTimeGapNs: 0,
    maximumAllowedDisplayTimeGapNs: 9_333_333,
    screenDamageCadenceWithinOneDisplayPeriod: true,
    frames: [{
      expectedWindowID: identity.windowId,
      actualWindowID: identity.windowId,
      displayTimeNs: 100,
      sha256: "a".repeat(64),
    }],
  };
}

describe("loss-accounted lifecycle filmstrip", () => {
  test("capture crop encloses the full 106% entry morph", () => {
    const expanded = expandCaptureBounds({
      x: 381,
      y: 166,
      width: 750,
      height: 501,
    });
    expect(expanded).toEqual({
      x: 351,
      y: 145,
      width: 810,
      height: 542,
    });
    expect(expanded.x).toBeLessThanOrEqual(358);
    expect(expanded.x + expanded.width).toBeGreaterThanOrEqual(358 + 795);
    expect(expanded.y).toBeLessThanOrEqual(160);
    expect(expanded.y + expanded.height).toBeGreaterThanOrEqual(160 + 492);
  });

  test("accepts an exact complete immutable capture", () => {
    expect(validateFilmstripCapture(validReceipt(), identity)).toEqual([]);
  });

  test("accepts an explicitly accounted unchanged sample", () => {
    const receipt = validReceipt();
    receipt.receivedSampleCount = 2;
    receipt.accountedSampleCount = 2;
    receipt.incompleteSampleCount = 1;
    expect(validateFilmstripCapture(receipt, identity)).toEqual([]);
  });

  test("rejects an unaccounted or untimed sample", () => {
    const receipt = validReceipt();
    receipt.receivedSampleCount = 2;
    receipt.missingDisplayTimeCount = 1;
    const errors = validateFilmstripCapture(receipt, identity);
    expect(errors).toContain("received sample accounting mismatch");
    expect(errors).toContain("one or more samples lack display time");
  });

  test("rejects a copied but unencoded complete frame", () => {
    const receipt = validReceipt();
    receipt.copiedCompleteCount = 2;
    expect(validateFilmstripCapture(receipt, identity)).toContain(
      "encoded complete count mismatch",
    );
  });

  test("records content-damage cadence without misclassifying it as capture loss", () => {
    const receipt = validReceipt();
    receipt.lateFrameCount = 1;
    receipt.maximumConsecutiveDisplayTimeGapNs = 9_333_334;
    receipt.screenDamageCadenceWithinOneDisplayPeriod = false;
    expect(validateFilmstripCapture(receipt, identity)).toEqual([]);
  });

  test("rejects an actual CGWindowID mismatch", () => {
    const receipt = validReceipt();
    receipt.frames[0].actualWindowID = 78;
    expect(validateFilmstripCapture(receipt, identity)).toContain(
      "frame 0 actual CGWindowID mismatch",
    );
  });
});

describe("exact detached-owner lifecycle", () => {
  const active = {
    schemaVersion: 2,
    nativeWindowNumber: 77,
    exitMode: "DetachedRegionsFadeOnly",
    originalFrame: [10, 20, 300, 200],
    currentFrame: [10, 20, 300, 200],
    currentAlpha: 0.8,
    commonContentViewFilterCount: 0,
    glassHostAttached: true,
    requestHostTimeNs: 1_000_000_000,
    expectedRemovalDeadlineNs: 1_135_000_000,
    cancelledAtHostTimeNs: null,
    committedAtHostTimeNs: null,
    history: [{ event: "ticketBegin", hostTimeNs: 1_000_000_000 }],
  };

  test("accepts a fixed-frame filter-free active exit", () => {
    expect(validateDetachedExitLifecycle(active, 77, "exiting")).toEqual([]);
  });

  test("rejects geometry drift, a filter, and early host teardown", () => {
    const errors = validateDetachedExitLifecycle({
      ...active,
      currentFrame: [10.3, 20, 300, 200],
      commonContentViewFilterCount: 1,
      glassHostAttached: false,
    }, 77, "exiting");
    expect(errors).toContain("native exit frame moved by more than 0.5 device pixel");
    expect(errors).toContain("common content-view filter must remain absent");
    expect(errors).toContain("native glass host detached before current exit resolved");
  });

  test("requires cancellation and restored alpha on reopen", () => {
    expect(validateDetachedExitLifecycle({
      ...active,
      currentFrame: [100, 200, 300, 200],
      currentAlpha: 1,
      cancelledAtHostTimeNs: 1_040_000_000,
      history: [
        ...active.history,
        { event: "ticketCancel", hostTimeNs: 1_040_000_000 },
      ],
    }, 77, "cancelled")).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// WP3 (glass-smoke-harness-max-info): scenario profiles, analysis modes,
// timing intervals, and the locked capture durations.
// ---------------------------------------------------------------------------

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
  test("default full profile is exactly the legacy order, each scenario once", () => {
    const names = resolveScenarioNames("full");
    expect(names).toEqual([
      "main-exit",
      "main-entry",
      "notes-entry",
      "notes-close-before-settle-reopen",
      "dictation-exit-reopen",
    ]);
    expect(names).toEqual([...LEGACY_FULL_SCENARIO_ORDER]);
    expect(new Set(names).size).toBe(names.length);
  });

  test("entry-color expands main-entry's hidden-reference dependency", () => {
    // The displayed-color metric needs main-exit's explicit post-exit
    // background reference, so requesting entry color ALWAYS includes it.
    expect(resolveScenarioNames("entry-color")).toEqual([
      "main-exit",
      "main-entry",
    ]);
  });

  test("extended keeps the legacy five as an exact prefix", () => {
    const names = resolveScenarioNames("extended");
    expect(names.slice(0, 5)).toEqual([...LEGACY_FULL_SCENARIO_ORDER]);
  });

  test("unknown profile fails closed", () => {
    expect(() => resolveScenarioNames("speedy" as never)).toThrow(
      "unknown scenario profile",
    );
  });

  test("locked legacy capture durations are unchanged", () => {
    expect(SCENARIO_CAPTURE_DURATIONS_MS).toEqual({
      "main-exit": 200,
      "main-entry": 700,
      "notes-entry": 800,
      "notes-close-before-settle-reopen": 950,
      "dictation-exit-reopen": 900,
    });
  });
});

describe("analysis modes", () => {
  test("inline is the default; unknown modes fail closed", () => {
    expect(parseAnalysisMode(undefined)).toBe("inline");
    expect(parseAnalysisMode("inline")).toBe("inline");
    expect(parseAnalysisMode("deferred")).toBe("deferred");
    expect(() => parseAnalysisMode("lazy")).toThrow("unknown analysis mode");
  });

  test("deferred mode can never produce a passing filmstrip", () => {
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

  test("inline mode with missing or failed metric analysis remains red", () => {
    expect(
      filmstripVerdict({
        captureErrorCount: 0,
        analysisMode: "inline",
        metricsExitCode: 1,
        metricsPass: false,
      }).pass,
    ).toBe(false);
    expect(
      filmstripVerdict({
        captureErrorCount: 0,
        analysisMode: "inline",
        metricsExitCode: null,
        metricsPass: null,
      }).pass,
    ).toBe(false);
    expect(
      filmstripVerdict({
        captureErrorCount: 0,
        analysisMode: "inline",
        metricsExitCode: 0,
        metricsPass: true,
      }).pass,
    ).toBe(true);
  });

  test("a pending capture receipt can never be EVALUABLE_PASS", () => {
    // Even a hypothetically green pass flag cannot escape ANALYSIS_PENDING.
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: null,
        analysisState: "pending",
        pass: true,
        hasObserverError: false,
      }),
    ).toBe("ANALYSIS_PENDING");
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: "INVALID_INTERFERENCE",
        analysisState: "pending",
        pass: false,
        hasObserverError: false,
      }),
    ).toBe("INVALID_INTERFERENCE");
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: null,
        analysisState: "inline",
        pass: true,
        hasObserverError: false,
      }),
    ).toBe("EVALUABLE_PASS");
    expect(
      computeLifecycleDisposition({
        interferenceDisposition: null,
        analysisState: "inline",
        pass: false,
        hasObserverError: true,
      }),
    ).toBe("INVALID_OBSERVER");
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

  test("overlap and backwards intervals are rejected", () => {
    const errors = validateScenarioTimingIntervals([
      { name: "main-exit", startedAtMs: 0, finishedAtMs: 20 },
      { name: "main-entry", startedAtMs: 10, finishedAtMs: 5 },
    ]);
    expect(errors).toContain("main-entry: interval runs backwards");
    expect(errors).toContain("main-entry: started before main-exit finished");
  });
});
