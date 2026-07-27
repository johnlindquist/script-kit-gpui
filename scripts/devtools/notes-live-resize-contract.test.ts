import { describe, expect, test } from "bun:test";
import {
  NATIVE_RESIZE_DIRECTIONS,
  type DirectionAttempt,
  type DirectionTrial,
  type NativeResizeDirection,
  type NotesLiveResizeReceipt,
  REQUIRED_MOVING_EDGES,
  validateDirectionTrial,
  validateNotesLiveResizeReceipt,
} from "./notes-live-resize-contract.ts";

function cleanAttempt(
  direction: NativeResizeDirection,
  overrides: Partial<DirectionAttempt> = {},
): DirectionAttempt {
  const displacements = { left: 0, right: 0, top: 0, bottom: 0 };
  for (const edge of REQUIRED_MOVING_EDGES[direction]) {
    displacements[edge] = 80;
  }
  return {
    insetPt: 2,
    helperStatus: "ok",
    untaggedInputCount: 0,
    distinctFrameCount: 12,
    displacements,
    finalWidth: 420,
    finalHeight: 320,
    ownerStable: true,
    postUpStablePt: 0,
    legacyResizeStartedSeen: false,
    ...overrides,
  };
}

function passTrial(direction: NativeResizeDirection): DirectionTrial {
  return {
    direction,
    disposition: "EVALUABLE_PASS",
    attempts: [cleanAttempt(direction)],
    selectedInsetPt: 2,
  };
}

function failTrial(direction: NativeResizeDirection): DirectionTrial {
  return {
    direction,
    disposition: "EVALUABLE_FAIL",
    attempts: [
      cleanAttempt(direction, {
        displacements: { left: 0, right: 0, top: 0, bottom: 0 },
      }),
    ],
    selectedInsetPt: null,
  };
}

function baseReceipt(
  overrides: Partial<NotesLiveResizeReceipt> = {},
): NotesLiveResizeReceipt {
  return {
    directions: NATIVE_RESIZE_DIRECTIONS.map((direction) => passTrial(direction)),
    settleProof: {
      disposition: "EVALUABLE_PASS",
      phaseBefore: "entryLocked",
      phaseAfter: "enabled",
      reason: "entry_settled",
      interactionEnabled: true,
      nativeApplyOk: true,
      enabledBeforeDeadline: false,
      styleMaskAfterHasResizableBit: true,
      policyUserResizable: true,
      policyMinWidth: 350,
      policyMinHeight: 280,
      policyMaxWidth: null,
      policyMaxHeight: null,
      policyWindowMatchesPinned: true,
    },
    minClamp: {
      disposition: "EVALUABLE_PASS",
      finalContentWidth: 350,
      finalContentHeight: 280,
      leftDriftPt: 0,
      topDriftPt: 0,
      ownerStable: true,
      legacyResizeStartedSeen: false,
    },
    modePartition: {
      disposition: "EVALUABLE_PASS",
      sameWindowId: true,
      outerDeltaMaxPt: 0,
      firstInsetBefore: 0,
      firstInsetAfter: 44,
      secondInsetBefore: 44,
      secondInsetAfter: 0,
      agentStageDeficitPt: 44,
      measuredGapPt: 8,
      notesStageDeficitAfterReturnPt: 0,
    },
    persistence: {
      disposition: "EVALUABLE_PASS",
      widthDeltaPt: 0,
      heightDeltaPt: 0,
      originDeltaPt: 0,
      restoredDefaultFallback: false,
    },
    morph: {
      disposition: "EVALUABLE_PASS",
      transientFrameCaptured: true,
      closeBeforeSettle: true,
      finalMatchesSettledPt: 0,
      finalMatchesTransientOnly: false,
    },
    ownerConsistent: true,
    cleanedUp: true,
    motionContractPass: null,
    ...overrides,
  };
}

describe("validateNotesLiveResizeReceipt", () => {
  test("eight clean passes → productPass, landingReady, remove", () => {
    const verdict = validateNotesLiveResizeReceipt(baseReceipt());
    expect(verdict.evidenceValid).toBe(true);
    expect(verdict.productPass).toBe(true);
    expect(verdict.landingReady).toBe(true);
    expect(verdict.failedDirections).toEqual([]);
    expect(verdict.recommendedRootCallDisposition).toBe("remove");
  });

  test("only TR fails → landingReady, failures exactly [TR], retain", () => {
    const receipt = baseReceipt({
      directions: NATIVE_RESIZE_DIRECTIONS.map((direction) =>
        direction === "TR" ? failTrial(direction) : passTrial(direction),
      ),
    });
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.productPass).toBe(false);
    expect(verdict.landingReady).toBe(true);
    expect(verdict.failedDirections).toEqual(["TR"]);
    expect(verdict.recommendedRootCallDisposition).toBe("retain");
  });

  test("edges pass but all corners fail → exact four-corner list, retain", () => {
    const corners: NativeResizeDirection[] = ["TL", "TR", "BL", "BR"];
    const receipt = baseReceipt({
      directions: NATIVE_RESIZE_DIRECTIONS.map((direction) =>
        corners.includes(direction) ? failTrial(direction) : passTrial(direction),
      ),
    });
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.failedDirections).toEqual(corners);
    expect(verdict.recommendedRootCallDisposition).toBe("retain");
    expect(verdict.landingReady).toBe(true);
  });

  test("corners pass but L and T fail → exact partial set, retain", () => {
    const failing: NativeResizeDirection[] = ["L", "T"];
    const receipt = baseReceipt({
      directions: NATIVE_RESIZE_DIRECTIONS.map((direction) =>
        failing.includes(direction) ? failTrial(direction) : passTrial(direction),
      ),
    });
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.failedDirections).toEqual(failing);
    expect(verdict.recommendedRootCallDisposition).toBe("retain");
  });

  test("any untaggedInputCount > 0 → evidence invalid, undecidable", () => {
    const receipt = baseReceipt();
    receipt.directions[3] = {
      ...passTrial("B"),
      attempts: [cleanAttempt("B", { untaggedInputCount: 2 })],
    };
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.evidenceValid).toBe(false);
    expect(verdict.landingReady).toBe(false);
    expect(verdict.recommendedRootCallDisposition).toBe("undecidable");
  });

  test("INVALID_INTERFERENCE direction → invalid, never pass/fail", () => {
    const receipt = baseReceipt();
    receipt.directions[0] = {
      direction: "L",
      disposition: "INVALID_INTERFERENCE",
      attempts: [],
      selectedInsetPt: null,
    };
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.evidenceValid).toBe(false);
    expect(verdict.productPass).toBe(false);
    expect(verdict.failedDirections).not.toContain("L");
    expect(verdict.recommendedRootCallDisposition).toBe("undecidable");
  });

  test("motion regression blocks landing even with direction fallback", () => {
    const receipt = baseReceipt({
      directions: NATIVE_RESIZE_DIRECTIONS.map((direction) =>
        direction === "BR" ? failTrial(direction) : passTrial(direction),
      ),
      motionContractPass: false,
    });
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.landingReady).toBe(false);
  });

  test("failed persistence blocks landing", () => {
    const receipt = baseReceipt({
      persistence: {
        disposition: "EVALUABLE_FAIL",
        widthDeltaPt: 30,
        heightDeltaPt: 0,
        originDeltaPt: 0,
        restoredDefaultFallback: true,
      },
    });
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.landingReady).toBe(false);
    expect(verdict.nonDirectionalContractPass).toBe(false);
  });

  test("morph frame overwriting the settled frame blocks landing", () => {
    const receipt = baseReceipt({
      morph: {
        disposition: "EVALUABLE_FAIL",
        transientFrameCaptured: true,
        closeBeforeSettle: true,
        finalMatchesSettledPt: 24,
        finalMatchesTransientOnly: true,
      },
    });
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.landingReady).toBe(false);
  });

  test("agent inset correct but outer width drifts 1.01pt → partition fails", () => {
    const receipt = baseReceipt();
    receipt.modePartition = {
      ...receipt.modePartition!,
      outerDeltaMaxPt: 1.01,
    };
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.nonDirectionalContractPass).toBe(false);
  });

  test("measured gap 7.99 or 8.01 fails the exact-gap tolerance", () => {
    for (const gap of [7.99, 8.01]) {
      const receipt = baseReceipt();
      receipt.modePartition = { ...receipt.modePartition!, measuredGapPt: gap };
      const verdict = validateNotesLiveResizeReceipt(receipt);
      expect(verdict.nonDirectionalContractPass).toBe(false);
    }
  });

  test("content 349×280 or 350×279 fails the clamp clause", () => {
    for (const [width, height] of [
      [349, 280],
      [350, 279],
    ] as const) {
      const receipt = baseReceipt();
      receipt.minClamp = {
        ...receipt.minClamp!,
        finalContentWidth: width,
        finalContentHeight: height,
      };
      const verdict = validateNotesLiveResizeReceipt(receipt);
      expect(verdict.nonDirectionalContractPass).toBe(false);
    }
  });

  test("a claimed direction pass without a clean attempt is demoted", () => {
    const receipt = baseReceipt();
    receipt.directions[1] = {
      direction: "R",
      disposition: "EVALUABLE_PASS",
      attempts: [
        cleanAttempt("R", {
          displacements: { left: 0, right: 59.99, top: 0, bottom: 0 },
        }),
      ],
      selectedInsetPt: 2,
    };
    const verdict = validateNotesLiveResizeReceipt(receipt);
    expect(verdict.failedDirections).toEqual(["R"]);
    expect(verdict.productPass).toBe(false);
  });
});

describe("validateDirectionTrial", () => {
  test("moving-edge displacement 59.99pt fails", () => {
    const attempt = cleanAttempt("R", {
      displacements: { left: 0, right: 59.99, top: 0, bottom: 0 },
    });
    expect(validateDirectionTrial("R", attempt).pass).toBe(false);
  });

  test("fixed-edge displacement 1.01pt fails", () => {
    const attempt = cleanAttempt("R", {
      displacements: { left: 1.01, right: 80, top: 0, bottom: 0 },
    });
    expect(validateDirectionTrial("R", attempt).pass).toBe(false);
  });

  test("fewer than four distinct frames fails", () => {
    const attempt = cleanAttempt("B", { distinctFrameCount: 3 });
    expect(validateDirectionTrial("B", attempt).pass).toBe(false);
  });

  test("a bottom trial with a legacy resizeStarted receipt fails as non-native", () => {
    const attempt = cleanAttempt("B", { legacyResizeStartedSeen: true });
    expect(validateDirectionTrial("B", attempt).pass).toBe(false);
  });

  test("final size below the policy minimum fails", () => {
    const attempt = cleanAttempt("R", { finalWidth: 349 });
    expect(validateDirectionTrial("R", attempt).pass).toBe(false);
  });

  test("a fully clean corner attempt passes", () => {
    const attempt = cleanAttempt("BR");
    const verdict = validateDirectionTrial("BR", attempt);
    expect(verdict.failures).toEqual([]);
    expect(verdict.pass).toBe(true);
  });
});
