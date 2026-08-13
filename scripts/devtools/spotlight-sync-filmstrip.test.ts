import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  deltaE2000,
  exitCodeForSpotlightDisposition,
  gradeSpotlightSyncBundle,
  type JsonRecord,
  type SpotlightSyncBundle,
} from "./spotlight-sync-filmstrip-contract.ts";

const HASH = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER = 77;
const LEFT = "script-kit-footer-capsule-left";
const RIGHT = "script-kit-footer-capsule-right";

function bounds(widthScale = 1): [[number, number], [number, number]] {
  return [[100, 200], [750 * widthScale, 500]];
}

function geometryFrame(
  sequence: number,
  elapsedMs: number,
  widthScale: number,
  alpha: number,
): JsonRecord {
  return {
    sequence,
    displayTimeNs: 1_000_000_000 + elapsedMs * 1_000_000,
    windowAlpha: alpha,
    windowBounds: bounds(widthScale),
    actualWindowID: OWNER,
    expectedWindowID: OWNER,
    sha256: String(sequence).padStart(64, "0"),
  };
}

function capsule(id: string): JsonRecord {
  return {
    id,
    screenshotFrame: id === LEFT
      ? { x: 0, y: 450, width: 100, height: 50 }
      : { x: 650, y: 450, width: 100, height: 50 },
    displayedMaterialMedianRgb: [45, 45, 45],
    stageMedianRgb: [40, 40, 40],
    stageDeltaE00: 2,
    stageAbsoluteLStarDifference: 5,
    medianBoundaryLuminanceDifference: 0.050,
    p10BoundaryLuminanceDifference: 0.020,
    fractionAtLeast015: 0.90,
  };
}

function colorFrame(
  phase: "motion" | "settled",
  sequence: number | string,
  alpha: number,
): JsonRecord {
  return {
    phase,
    sequence,
    displayTimeNs: typeof sequence === "number"
      ? 1_000_000_000 + sequence * 17_000_000
      : null,
    windowAlpha: alpha,
    windowBounds: bounds(1),
    entryVisible: true,
    displayedColorEligible: true,
    displayedStageMedianRgb: [40, 40, 40],
    capsules: [capsule(LEFT), capsule(RIGHT)],
  };
}

function captureReceipt(frames: JsonRecord[]): JsonRecord {
  return {
    schemaVersion: 2,
    status: "ok",
    captureHealthPass: true,
    droppedCompleteCount: 0,
    duplicateDisplayTimeCount: 0,
    screenDamageCadenceWithinOneDisplayPeriod: true,
    maximumConsecutiveDisplayTimeGapNs: 16_700_000,
    maximumAllowedDisplayTimeGapNs: 17_000_000,
    frames,
  };
}

function validBundle(runOffset = 0): SpotlightSyncBundle {
  const onsetWidthScale = 1.0305 + runOffset * 0.0004;
  const onsetBlurRadius = 12 + runOffset * 0.02;
  const entryGeometry = [
    geometryFrame(0, 0, onsetWidthScale, 0.85),
    geometryFrame(1, 18, 1.012, 0.85),
    geometryFrame(2, 44, 1.012, 0.85),
    geometryFrame(3, 60, 0.999, 0.97),
    geometryFrame(4, 79, 0.987, 0.99),
    geometryFrame(5, 114, 0.995, 0.995),
    geometryFrame(6, 149, 1, 1),
  ];
  const exitGeometry = [
    geometryFrame(20, 0, 1, 1),
    geometryFrame(21, 17, 1, 0.90),
    geometryFrame(22, 34, 1, 0.75),
    geometryFrame(23, 51, 1, 0.45),
    geometryFrame(24, 68, 1, 0.10),
  ];
  const settled = [0, 1, 2].map((index) =>
    colorFrame("settled", `settled-${index}`, 1)
  );
  const entryMotion = [0, 1, 2, 3, 4].map((index) =>
    colorFrame("motion", index, 0.85 + index * 0.035)
  );
  const exitMotion = [
    colorFrame("motion", 20, 1),
    colorFrame("motion", 21, 0.90),
    colorFrame("motion", 22, 0.75),
    colorFrame("motion", 23, 0.45),
    colorFrame("motion", 24, 0.10),
  ];
  return {
    lifecycleReceiptSha256: HASH,
    lifecycle: {
      schemaVersion: 2,
      cleanedUp: true,
      interference: {
        pass: true,
        disposition: "EVALUABLE_PASS",
      },
      scenarios: [
        {
          name: "main-exit",
          exactWindowID: OWNER,
          hiddenReferencePass: true,
          filmstrip: { receipt: captureReceipt(exitGeometry) },
        },
        {
          name: "main-entry",
          exactWindowID: OWNER,
          settledCapturesPass: true,
          nativeGlassOnset: {
            present: true,
            line: "event=native_glass_entry_onset",
            supported: true,
            entryBlurRadius: onsetBlurRadius,
            entryBlurToRadius: 0,
            footerBlurRadius: onsetBlurRadius,
            footerBlurToRadius: 0,
            footerBlurScope: "per_capsule",
            footerBlurDurationNs: 44_000_000,
            footerCapsuleCount: 2,
            footerBlurredCapsuleCount: 2,
            footerEnrolled: false,
            entryBlurDurationNs: 44_000_000,
            onsetStartWidthScale: onsetWidthScale,
            tailStartWidthScale: 1.012,
            onsetGeometryDurationNs: 18_000_000,
            onsetDurationNs: 44_000_000,
            contentRootCount: 4,
            contentHoldNs: 0,
            contentFadeNs: 44_000_000,
            windowAlpha: 0.85,
            errors: [],
            pass: true,
          },
          presentationGeometry: {
            receipt: { pass: true, frames: entryGeometry },
          },
          filmstrip: { receipt: captureReceipt(entryGeometry) },
          settledLayout: {
            fidelity: {
              appKit: {
                footerContainerFrame: { x: 0, y: 450, width: 750, height: 50 },
                nodes: [
                  {
                    id: LEFT,
                    screenshotFrame: { x: 0, y: 450, width: 100, height: 50 },
                  },
                  {
                    id: RIGHT,
                    screenshotFrame: { x: 650, y: 450, width: 100, height: 50 },
                  },
                ],
              },
            },
          },
        },
      ],
    },
    entryColor: {
      schemaVersion: 2,
      lifecyclePhase: "entry",
      layoutSource: { lifecycleReceiptSha256: HASH },
      errors: [],
      summary: {
        alphaPolicy: {
          requiredMinimumVisibleEntryAlpha: 0.85,
          firstVisibleEntryAlpha: 0.85,
          minimumVisibleEntryAlpha: 0.85,
          visibleFramesBelowAlphaFloor: [],
          visibleZeroAlphaFrames: [],
          unmeasurableVisibleFrames: [],
          unmeasurableVisibleFrameCount: 0,
          pass: true,
        },
      },
      frames: [...entryMotion, ...settled],
    },
    exitColor: {
      schemaVersion: 2,
      lifecyclePhase: "exit",
      layoutSource: { lifecycleReceiptSha256: HASH },
      errors: [],
      frames: [...exitMotion, ...settled],
    },
  };
}

function clonedBundle(): SpotlightSyncBundle {
  return structuredClone(validBundle());
}

function scenario(bundle: SpotlightSyncBundle, name: string): JsonRecord {
  const lifecycle = bundle.lifecycle as JsonRecord;
  return (lifecycle.scenarios as JsonRecord[]).find((row) => row.name === name)!;
}

function frame(receipt: unknown, phase: string, sequence: number | string): JsonRecord {
  return ((receipt as JsonRecord).frames as JsonRecord[]).find(
    (row) => row.phase === phase && row.sequence === sequence,
  )!;
}

function frameCapsule(row: JsonRecord, id: string): JsonRecord {
  return (row.capsules as JsonRecord[]).find((candidate) => candidate.id === id)!;
}

describe("Spotlight-sync filmstrip contract", () => {
  test("accepts three dense synchronized retuned entry and exit receipts", () => {
    for (const runOffset of [-1, 0, 1]) {
      const grade = gradeSpotlightSyncBundle(validBundle(runOffset));
      expect(grade.disposition).toBe("EVALUABLE_PASS");
      expect(grade.pass).toBe(true);
      expect(grade.failures).toEqual([]);
      expect(grade.coverage.entryOnsetReceiptPresent).toBe(true);
      expect(grade.coverage.entryDistinctWidthCount).toBeGreaterThanOrEqual(5);
      expect(grade.coverage.entryOnsetTailAtMs).toBe(18);
      expect(grade.coverage.entryOnsetTailWidthScale).toBeCloseTo(1.012, 6);
      expect(grade.coverage.capsuleIds).toEqual([LEFT, RIGHT]);
      expect(grade.measurements.onset).toMatchObject({
        entryBlurToRadius: 0,
        footerBlurRadius: 12 + runOffset * 0.02,
        footerBlurToRadius: 0,
        footerBlurScope: "per_capsule",
        footerBlurDurationNs: 44_000_000,
        footerCapsuleCount: 2,
        footerBlurredCapsuleCount: 2,
        footerEnrolled: false,
      });
      expect(grade.measurements.entry[0]?.capsules[0]?.geometry).toHaveProperty(
        "screenshotFrame",
      );
    }
  });

  test("locks the Spotlight soft-materialize start at exactly 1.0305 ± 0.006", () => {
    // Durable anti-redrift guard: the first visible frame is the measured
    // 103.05% soft-materialize photon (2026-08-13 retune). 1.012 — the old
    // wide-start-only first frame — is an explicit FAILURE case now, so the
    // onset stretch cannot silently regress; 1.000 remains the signature of
    // the queued-move stomp.
    const cases: Array<[number, boolean]> = [
      [1.000, false],
      [1.012, false],
      [1.0244, false],
      [1.0245, true],
      [1.0305, true],
      [1.0365, true],
      [1.0366, false],
    ];
    for (const [scale, shouldPass] of cases) {
      const bundle = clonedBundle();
      const entry = scenario(bundle, "main-entry");
      for (const frames of [
        entry.presentationGeometry?.receipt?.frames,
        entry.filmstrip?.receipt?.frames,
      ]) {
        if (Array.isArray(frames) && frames[0]) {
          (frames[0] as JsonRecord).windowBounds = bounds(scale);
        }
      }
      const grade = gradeSpotlightSyncBundle(bundle);
      const wideFailure = grade.failures.find(
        (failure) => failure.metric === "entry.geometry.firstVisibleWidthScale",
      );
      if (shouldPass) {
        expect(wideFailure).toBeUndefined();
      } else {
        expect(wideFailure).toMatchObject({ kind: "product", phase: "entry" });
      }
    }
  });

  test("rejects the stale eight-point full-entry onset blur", () => {
  const bundle = clonedBundle();
  const onset = scenario(bundle, "main-entry").nativeGlassOnset as JsonRecord;
  onset.entryBlurRadius = 8;
  onset.entryBlurDurationNs = 149_000_000;
  onset.pass = false;
  onset.errors = ["stale onset blur"];
  const grade = gradeSpotlightSyncBundle(bundle);
  expect(grade.disposition).toBe("EVALUABLE_FAIL");
  expect(grade.failures).toContainEqual(expect.objectContaining({
    kind: "product",
    phase: "entry",
    metric: "entry.onset.blurFromRadius",
  }));
  expect(grade.failures).toContainEqual(expect.objectContaining({
    kind: "product",
    phase: "entry",
    metric: "entry.onset.blurDurationMs",
  }));
});

test("rejects stale, container-scoped, or incomplete footer onset defocus", () => {
  const bundle = clonedBundle();
  const onset = scenario(bundle, "main-entry").nativeGlassOnset as JsonRecord;
  onset.footerBlurRadius = 0;
  onset.footerBlurScope = "container";
  onset.footerBlurDurationNs = 149_000_000;
  onset.footerBlurredCapsuleCount = 1;
  onset.footerEnrolled = true;
  onset.pass = false;
  onset.errors = ["footer effects"];
  const grade = gradeSpotlightSyncBundle(bundle);
  expect(grade.disposition).toBe("EVALUABLE_FAIL");
  for (const metric of [
    "entry.onset.footerBlurRadius",
    "entry.onset.footerBlurScope",
    "entry.onset.footerBlurDurationMs",
    "entry.onset.footerBlurredCapsuleCount",
    "entry.onset.footerEnrolled",
  ]) {
    expect(grade.failures).toContainEqual(expect.objectContaining({
      kind: "product",
      phase: "entry",
      metric,
    }));
  }
});

test("names the exact entry frame and capsule for a brightness excursion", () => {
    const bundle = clonedBundle();
    const row = frame(bundle.entryColor, "motion", 2);
    const left = frameCapsule(row, LEFT);
    left.stageDeltaE00 = 7;
    left.stageAbsoluteLStarDifference = 18;
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("EVALUABLE_FAIL");
    expect(grade.failures).toContainEqual(expect.objectContaining({
      kind: "product",
      phase: "entry",
      sequence: 2,
      capsuleId: LEFT,
      metric: "entry.capsule.stageRelationDriftDeltaE00",
    }));
    expect(grade.failures).toContainEqual(expect.objectContaining({
      sequence: 2,
      capsuleId: LEFT,
      metric: "entry.capsule.stageLStarRelationDrift",
    }));
  });

  test("names the exact entry frame that violates the visible alpha floor", () => {
    const bundle = clonedBundle();
    const policy = (((bundle.entryColor as JsonRecord).summary as JsonRecord)
      .alphaPolicy as JsonRecord);
    policy.minimumVisibleEntryAlpha = 0.40;
    policy.visibleFramesBelowAlphaFloor = [1];
    policy.pass = false;
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("EVALUABLE_FAIL");
    expect(grade.failures).toContainEqual(expect.objectContaining({
      kind: "product",
      phase: "entry",
      sequence: 1,
      metric: "entry.alpha.visibleMinimum",
    }));
  });

  test("rejects main and capsule color movement after settle", () => {
    const bundle = clonedBundle();
    const last = frame(bundle.entryColor, "settled", "settled-2");
    last.displayedStageMedianRgb = [75, 75, 75];
    frameCapsule(last, RIGHT).displayedMaterialMedianRgb = [90, 90, 90];
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("EVALUABLE_FAIL");
    expect(grade.failures).toContainEqual(expect.objectContaining({
      phase: "settled",
      sequence: "settled-2",
      metric: "steady.main.displayedDeltaE00",
    }));
    expect(grade.failures).toContainEqual(expect.objectContaining({
      phase: "settled",
      sequence: "settled-2",
      capsuleId: RIGHT,
      metric: "steady.capsule.displayedDeltaE00",
    }));
  });

  test("names the exact exit frame that violates fixed-frame fade-only", () => {
    const bundle = clonedBundle();
    const exit = scenario(bundle, "main-exit");
    const rows = ((exit.filmstrip as JsonRecord).receipt as JsonRecord)
      .frames as JsonRecord[];
    rows[2]!.windowBounds = [[100, 200], [754, 500]];
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("EVALUABLE_FAIL");
    expect(grade.failures).toContainEqual(expect.objectContaining({
      phase: "exit",
      sequence: 22,
      metric: "exit.geometry.widthDriftPoints",
    }));
  });

  test("fails closed when the interference monitor invalidates the run", () => {
    const bundle = clonedBundle();
    (bundle.lifecycle as JsonRecord).interference = {
      pass: false,
      disposition: "INVALID_INTERFERENCE",
    };
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("INVALID_INTERFERENCE");
    expect(grade.pass).toBe(false);
    expect(exitCodeForSpotlightDisposition(grade.disposition)).toBe(4);
  });

  test("classifies an under-resolved entry as observer failure", () => {
    const bundle = clonedBundle();
    const entry = scenario(bundle, "main-entry");
    ((entry.presentationGeometry as JsonRecord).receipt as JsonRecord).frames = (
      ((entry.presentationGeometry as JsonRecord).receipt as JsonRecord)
        .frames as JsonRecord[]
    ).slice(0, 2);
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("INVALID_OBSERVER");
    expect(grade.failures).toContainEqual(expect.objectContaining({
      kind: "observer",
      metric: "entry.geometry.frameCount",
    }));
  });

  test("names the exact capsule frame whose rendered boundary disappears", () => {
    // Lower-tail gates (p10/fraction) apply only on SETTLED entry frames —
    // motion and exit frames sample the window/desktop seam on edge-flush
    // capsules (see validateColorCoverage). The median gate remains the
    // washout catch on every comparable frame.
    const bundle = clonedBundle();
    const settledRow = frame(bundle.entryColor, "settled", "settled-1");
    const settledRight = frameCapsule(settledRow, RIGHT);
    settledRight.fractionAtLeast015 = 0.25;
    const exitRow = frame(bundle.exitColor, "motion", 21);
    const exitRight = frameCapsule(exitRow, RIGHT);
    exitRight.medianBoundaryLuminanceDifference = 0.004;
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("EVALUABLE_FAIL");
    expect(grade.failures).toContainEqual(expect.objectContaining({
      kind: "product",
      phase: "entry",
      sequence: "settled-1",
      capsuleId: RIGHT,
      metric: "entry.capsule.fractionAtLeast015",
    }));
    expect(grade.failures).toContainEqual(expect.objectContaining({
      kind: "product",
      phase: "exit",
      sequence: 21,
      capsuleId: RIGHT,
      metric: "exit.capsule.medianBoundaryLuminanceDifference",
    }));
    // Seam-scoped: a low exit p10 alone must NOT fail the run.
    const seamBundle = clonedBundle();
    const seamRight = frameCapsule(frame(seamBundle.exitColor, "motion", 21), RIGHT);
    seamRight.p10BoundaryLuminanceDifference = 0.003;
    expect(gradeSpotlightSyncBundle(seamBundle).pass).toBe(true);
  });

  test("grades both floating edges against the zero-inset contract", () => {
    const bundle = clonedBundle();
    const entry = scenario(bundle, "main-entry");
    const appKit = (((entry.settledLayout as JsonRecord).fidelity as JsonRecord)
      .appKit as JsonRecord);
    const nodes = appKit.nodes as JsonRecord[];
    (nodes[0]!.screenshotFrame as JsonRecord).x = 14;
    (nodes[1]!.screenshotFrame as JsonRecord).x = 636;
    const grade = gradeSpotlightSyncBundle(bundle);
    expect(grade.disposition).toBe("EVALUABLE_FAIL");
    expect(grade.failures).toContainEqual(expect.objectContaining({
      sequence: "settled-layout",
      capsuleId: LEFT,
      metric: "settled.edgeFlush.leftInsetPxAt1x",
    }));
    expect(grade.failures).toContainEqual(expect.objectContaining({
      sequence: "settled-layout",
      capsuleId: RIGHT,
      metric: "settled.edgeFlush.rightInsetPxAt1x",
    }));
  });

  test("grade-only runner emits one final receipt bound to the imported evidence", async () => {
    const directory = mkdtempSync(join(tmpdir(), "spotlight-sync-grade-only-"));
    try {
      const bundle = validBundle();
      const lifecyclePath = join(directory, "lifecycle.json");
      const entryPath = join(directory, "entry.json");
      const exitPath = join(directory, "exit.json");
      const outPath = join(directory, "receipt.json");
      writeFileSync(lifecyclePath, `${JSON.stringify(bundle.lifecycle, null, 2)}\n`);
      const lifecycleSha256 = createHash("sha256")
        .update(readFileSync(lifecyclePath))
        .digest("hex");
      for (const receipt of [bundle.entryColor, bundle.exitColor] as JsonRecord[]) {
        (receipt.layoutSource as JsonRecord).lifecycleReceiptSha256 = lifecycleSha256;
      }
      writeFileSync(entryPath, `${JSON.stringify(bundle.entryColor, null, 2)}\n`);
      writeFileSync(exitPath, `${JSON.stringify(bundle.exitColor, null, 2)}\n`);
      const child = Bun.spawn([
        "bun",
        resolve(import.meta.dir, "spotlight-sync-filmstrip.ts"),
        "--grade-only",
        "--lifecycle-receipt",
        lifecyclePath,
        "--entry-color-receipt",
        entryPath,
        "--exit-color-receipt",
        exitPath,
        "--out",
        outPath,
      ], { stdout: "pipe", stderr: "pipe" });
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
        child.exited,
      ]);
      expect(stderr).toBe("");
      expect(exitCode).toBe(0);
      expect(JSON.parse(stdout).pass).toBe(true);
      const receipt = JSON.parse(readFileSync(outPath, "utf8"));
      expect(receipt.kind).toBe("spotlight-sync-filmstrip-receipt");
      expect(receipt.pass).toBe(true);
      expect(receipt.commands).toEqual({});
      expect(receipt.sourceReceipts.lifecycle.sha256).toBe(lifecycleSha256);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  test("CIEDE2000 is zero for identical colors and symmetric", () => {
    expect(deltaE2000([40, 42, 46], [40, 42, 46])).toBeCloseTo(0, 10);
    expect(deltaE2000([40, 42, 46], [80, 70, 60])).toBeCloseTo(
      deltaE2000([80, 70, 60], [40, 42, 46]),
      10,
    );
  });
});
