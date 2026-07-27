/**
 * Pure receipt contract for the Notes native all-edge live-resize probe
 * (`notes-live-resize.ts`).
 *
 * Authored BEFORE the runtime probe (Oracle session notes-resize-probe-plan,
 * step 2) so "pass" is defined up front and cannot be shaped opportunistically
 * around observed app behavior.
 *
 * Semantics:
 * - `evidenceValid` is false for observer/setup/environment/interference
 *   problems — invalid evidence never counts as green OR red.
 * - `productPass` is true only when every premise clause passes, including all
 *   eight native drag directions.
 * - `nonDirectionalContractPass` covers settle/policy receipts, the 350×280
 *   clamp, the Notes↔Agent backdrop partition, stable-bounds persistence,
 *   morph-frame non-persistence, owner topology, and cleanup.
 * - `landingReady = evidenceValid && nonDirectionalContractPass` — it may be
 *   true while one or more directions cleanly fail; that is the explicitly
 *   authorized fallback-retained landing branch.
 * - `recommendedRootCallDisposition` implements the pre-committed decision
 *   rule: `remove` on a clean 8/8 pass, `retain` on any clean direction
 *   failure, `undecidable` on invalid/blocked evidence.
 */

export const NATIVE_RESIZE_DIRECTIONS = [
  "L",
  "R",
  "T",
  "B",
  "TL",
  "TR",
  "BL",
  "BR",
] as const;
export type NativeResizeDirection = (typeof NATIVE_RESIZE_DIRECTIONS)[number];

export type TrialDisposition =
  | "EVALUABLE_PASS"
  | "EVALUABLE_FAIL"
  | "INVALID_INTERFERENCE"
  | "INVALID_OBSERVER"
  | "INVALID_TIMING"
  | "BLOCKED_ENVIRONMENT";

/** Displacement of each window edge over one drag attempt, in points. */
export interface EdgeDisplacements {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export interface DirectionAttempt {
  insetPt: number;
  helperStatus: string;
  untaggedInputCount: number;
  distinctFrameCount: number;
  displacements: EdgeDisplacements;
  finalWidth: number;
  finalHeight: number;
  ownerStable: boolean;
  postUpStablePt: number;
  legacyResizeStartedSeen: boolean;
}

export interface DirectionTrial {
  direction: NativeResizeDirection;
  disposition: TrialDisposition;
  attempts: DirectionAttempt[];
  selectedInsetPt: number | null;
}

const MIN_MOVEMENT_PT = 60;
const FIXED_EDGE_TOLERANCE_PT = 1;
const MIN_DISTINCT_RECTS = 4;
const POLICY_MIN_WIDTH = 350;
const POLICY_MIN_HEIGHT = 280;
const GAP_PT = 8;
/**
 * The transparent gap must measure exactly 8pt. The tolerance is half the
 * 0.01pt reporting granularity so a measured 7.99/8.01 always fails even
 * under binary floating-point representation (8.01 - 8 ≈ 0.009999…).
 */
const GAP_TOLERANCE_PT = 0.005;
const OUTER_FRAME_TOLERANCE_PT = 1;

/** Edges that MUST move (by >= MIN_MOVEMENT_PT) for each direction. */
export const REQUIRED_MOVING_EDGES: Record<
  NativeResizeDirection,
  Array<keyof EdgeDisplacements>
> = {
  L: ["left"],
  R: ["right"],
  T: ["top"],
  B: ["bottom"],
  TL: ["left", "top"],
  TR: ["right", "top"],
  BL: ["left", "bottom"],
  BR: ["right", "bottom"],
};

/**
 * Geometry validation for ONE clean attempt. `pass` is true only for a
 * continuous native resize: enough distinct sampled frames, every intended
 * edge moving far enough, every fixed edge pinned, minimum respected, exact
 * owner unchanged, stable after mouse-up, and no legacy custom-handler route.
 */
export function validateDirectionTrial(
  direction: NativeResizeDirection,
  attempt: DirectionAttempt,
): { pass: boolean; failures: string[] } {
  const failures: string[] = [];
  const moving = REQUIRED_MOVING_EDGES[direction];
  const fixed = (
    ["left", "right", "top", "bottom"] as Array<keyof EdgeDisplacements>
  ).filter((edge) => !moving.includes(edge));

  if (attempt.helperStatus !== "ok") {
    failures.push(`helper status ${attempt.helperStatus}`);
  }
  if (attempt.untaggedInputCount !== 0) {
    failures.push(`untagged input ${attempt.untaggedInputCount}`);
  }
  if (attempt.distinctFrameCount < MIN_DISTINCT_RECTS) {
    failures.push(
      `only ${attempt.distinctFrameCount} distinct frames (< ${MIN_DISTINCT_RECTS})`,
    );
  }
  for (const edge of moving) {
    if (Math.abs(attempt.displacements[edge]) < MIN_MOVEMENT_PT) {
      failures.push(
        `moving edge ${edge} displaced ${attempt.displacements[edge]}pt (< ${MIN_MOVEMENT_PT})`,
      );
    }
  }
  for (const edge of fixed) {
    if (Math.abs(attempt.displacements[edge]) > FIXED_EDGE_TOLERANCE_PT) {
      failures.push(
        `fixed edge ${edge} drifted ${attempt.displacements[edge]}pt (> ${FIXED_EDGE_TOLERANCE_PT})`,
      );
    }
  }
  if (attempt.finalWidth < POLICY_MIN_WIDTH) {
    failures.push(`final width ${attempt.finalWidth} < ${POLICY_MIN_WIDTH}`);
  }
  if (attempt.finalHeight < POLICY_MIN_HEIGHT) {
    failures.push(`final height ${attempt.finalHeight} < ${POLICY_MIN_HEIGHT}`);
  }
  if (!attempt.ownerStable) failures.push("owner identity changed mid-trial");
  if (Math.abs(attempt.postUpStablePt) > FIXED_EDGE_TOLERANCE_PT) {
    failures.push(`post-mouse-up frame moved ${attempt.postUpStablePt}pt`);
  }
  if (attempt.legacyResizeStartedSeen) {
    failures.push("legacy resizeStarted route fired — resize was not native");
  }
  return { pass: failures.length === 0, failures };
}

export interface SettleProof {
  disposition: TrialDisposition;
  phaseBefore: string | null;
  phaseAfter: string | null;
  reason: string | null;
  interactionEnabled: boolean;
  nativeApplyOk: boolean;
  enabledBeforeDeadline: boolean;
  styleMaskAfterHasResizableBit: boolean;
  policyUserResizable: boolean;
  policyMinWidth: number | null;
  policyMinHeight: number | null;
  policyMaxWidth: number | null;
  policyMaxHeight: number | null;
  policyWindowMatchesPinned: boolean;
}

export interface MinClampProof {
  disposition: TrialDisposition;
  finalContentWidth: number;
  finalContentHeight: number;
  leftDriftPt: number;
  topDriftPt: number;
  ownerStable: boolean;
  legacyResizeStartedSeen: boolean;
}

export interface ModePartitionProof {
  disposition: TrialDisposition;
  sameWindowId: boolean;
  outerDeltaMaxPt: number;
  firstInsetBefore: number;
  firstInsetAfter: number;
  secondInsetBefore: number;
  secondInsetAfter: number;
  agentStageDeficitPt: number | null;
  measuredGapPt: number | null;
  notesStageDeficitAfterReturnPt: number | null;
}

export interface PersistenceProof {
  disposition: TrialDisposition;
  widthDeltaPt: number;
  heightDeltaPt: number;
  originDeltaPt: number;
  restoredDefaultFallback: boolean;
}

export interface MorphProof {
  disposition: TrialDisposition;
  transientFrameCaptured: boolean;
  closeBeforeSettle: boolean;
  finalMatchesSettledPt: number;
  finalMatchesTransientOnly: boolean;
}

export interface NotesLiveResizeReceipt {
  directions: DirectionTrial[];
  settleProof: SettleProof | null;
  minClamp: MinClampProof | null;
  modePartition: ModePartitionProof | null;
  persistence: PersistenceProof | null;
  morph: MorphProof | null;
  ownerConsistent: boolean;
  cleanedUp: boolean;
  /**
   * Optional external glass-motion verdict (lifecycle/rapid-toggle probes).
   * `null` = not evaluated by this receipt (does not block); `false` blocks
   * landing even when the direction fallback branch fired.
   */
  motionContractPass?: boolean | null;
}

export interface NotesLiveResizeVerdict {
  evidenceValid: boolean;
  productPass: boolean;
  nonDirectionalContractPass: boolean;
  landingReady: boolean;
  failedDirections: NativeResizeDirection[];
  recommendedRootCallDisposition: "remove" | "retain" | "undecidable";
  failures: string[];
}

const INVALID_DISPOSITIONS: TrialDisposition[] = [
  "INVALID_INTERFERENCE",
  "INVALID_OBSERVER",
  "INVALID_TIMING",
  "BLOCKED_ENVIRONMENT",
];

function proofInvalid(proof: { disposition: TrialDisposition } | null): boolean {
  return proof !== null && INVALID_DISPOSITIONS.includes(proof.disposition);
}

function validateSettleProof(proof: SettleProof | null, failures: string[]): boolean {
  if (proof === null) {
    failures.push("settle proof missing");
    return false;
  }
  if (proof.disposition !== "EVALUABLE_PASS" && proof.disposition !== "EVALUABLE_FAIL") {
    return false; // invalidity handled by evidenceValid
  }
  const ok =
    proof.phaseBefore === "entryLocked" &&
    proof.phaseAfter === "enabled" &&
    proof.reason === "entry_settled" &&
    proof.interactionEnabled &&
    proof.nativeApplyOk &&
    !proof.enabledBeforeDeadline &&
    proof.styleMaskAfterHasResizableBit &&
    proof.policyUserResizable &&
    proof.policyMinWidth === POLICY_MIN_WIDTH &&
    proof.policyMinHeight === POLICY_MIN_HEIGHT &&
    proof.policyMaxWidth === null &&
    proof.policyMaxHeight === null &&
    proof.policyWindowMatchesPinned;
  if (!ok) failures.push("settle/style-mask proof failed");
  return ok;
}

function validateMinClamp(proof: MinClampProof | null, failures: string[]): boolean {
  if (proof === null) {
    failures.push("minimum clamp proof missing");
    return false;
  }
  if (INVALID_DISPOSITIONS.includes(proof.disposition)) return false;
  if (proof.disposition === "EVALUABLE_FAIL") {
    failures.push("minimum clamp clause is an evaluable failure");
    return false;
  }
  const widthOk =
    Math.abs(proof.finalContentWidth - POLICY_MIN_WIDTH) <= 1 &&
    proof.finalContentWidth > POLICY_MIN_WIDTH - 1;
  const heightOk =
    Math.abs(proof.finalContentHeight - POLICY_MIN_HEIGHT) <= 1 &&
    proof.finalContentHeight > POLICY_MIN_HEIGHT - 1;
  const ok =
    widthOk &&
    heightOk &&
    Math.abs(proof.leftDriftPt) <= OUTER_FRAME_TOLERANCE_PT &&
    Math.abs(proof.topDriftPt) <= OUTER_FRAME_TOLERANCE_PT &&
    proof.ownerStable &&
    !proof.legacyResizeStartedSeen;
  if (!ok) {
    failures.push(
      `minimum clamp failed (content ${proof.finalContentWidth}×${proof.finalContentHeight})`,
    );
  }
  return ok;
}

function validateModePartition(
  proof: ModePartitionProof | null,
  failures: string[],
): boolean {
  if (proof === null) {
    failures.push("mode partition proof missing");
    return false;
  }
  if (INVALID_DISPOSITIONS.includes(proof.disposition)) return false;
  const insetOk =
    proof.firstInsetBefore === 0 &&
    proof.firstInsetAfter > GAP_PT &&
    proof.secondInsetBefore === proof.firstInsetAfter &&
    proof.secondInsetAfter === 0;
  const frameOk =
    proof.sameWindowId &&
    Math.abs(proof.outerDeltaMaxPt) <= OUTER_FRAME_TOLERANCE_PT;
  const stageOk =
    proof.agentStageDeficitPt !== null &&
    Math.abs(proof.agentStageDeficitPt - proof.firstInsetAfter) <= 1 &&
    proof.notesStageDeficitAfterReturnPt !== null &&
    Math.abs(proof.notesStageDeficitAfterReturnPt) <= 1;
  const gapOk =
    proof.measuredGapPt !== null &&
    Math.abs(proof.measuredGapPt - GAP_PT) < GAP_TOLERANCE_PT;
  const ok = insetOk && frameOk && stageOk && gapOk;
  if (!ok) {
    failures.push(
      `mode partition failed (insets ${proof.firstInsetBefore}→${proof.firstInsetAfter}→${proof.secondInsetAfter}, outerΔ ${proof.outerDeltaMaxPt}, gap ${proof.measuredGapPt})`,
    );
  }
  return ok;
}

function validatePersistence(
  proof: PersistenceProof | null,
  failures: string[],
): boolean {
  if (proof === null) {
    failures.push("persistence proof missing");
    return false;
  }
  if (INVALID_DISPOSITIONS.includes(proof.disposition)) return false;
  const ok =
    proof.disposition === "EVALUABLE_PASS" &&
    Math.abs(proof.widthDeltaPt) <= 1 &&
    Math.abs(proof.heightDeltaPt) <= 1 &&
    Math.abs(proof.originDeltaPt) <= 1 &&
    !proof.restoredDefaultFallback;
  if (!ok) failures.push("stable resized-bounds persistence failed");
  return ok;
}

function validateMorph(proof: MorphProof | null, failures: string[]): boolean {
  if (proof === null) {
    failures.push("morph non-persistence proof missing");
    return false;
  }
  if (INVALID_DISPOSITIONS.includes(proof.disposition)) return false;
  const ok =
    proof.transientFrameCaptured &&
    proof.closeBeforeSettle &&
    Math.abs(proof.finalMatchesSettledPt) <= 1 &&
    !proof.finalMatchesTransientOnly;
  if (!ok) failures.push("entry-morph frame persistence check failed");
  return ok;
}

export function validateNotesLiveResizeReceipt(
  receipt: NotesLiveResizeReceipt,
): NotesLiveResizeVerdict {
  const failures: string[] = [];

  // ── Evidence validity ────────────────────────────────────────────────
  let evidenceValid = true;
  const seen = new Set<NativeResizeDirection>();
  for (const trial of receipt.directions) {
    seen.add(trial.direction);
    if (INVALID_DISPOSITIONS.includes(trial.disposition)) {
      evidenceValid = false;
      failures.push(`direction ${trial.direction}: ${trial.disposition}`);
    }
    for (const attempt of trial.attempts) {
      if (attempt.untaggedInputCount > 0) {
        evidenceValid = false;
        failures.push(
          `direction ${trial.direction}: untagged input ${attempt.untaggedInputCount}`,
        );
      }
    }
  }
  for (const direction of NATIVE_RESIZE_DIRECTIONS) {
    if (!seen.has(direction)) {
      evidenceValid = false;
      failures.push(`direction ${direction} missing from receipt`);
    }
  }
  for (const proof of [
    receipt.settleProof,
    receipt.minClamp,
    receipt.modePartition,
    receipt.persistence,
    receipt.morph,
  ]) {
    if (proofInvalid(proof)) {
      evidenceValid = false;
      failures.push(`proof invalid: ${proof?.disposition}`);
    }
  }
  if (!receipt.ownerConsistent) {
    evidenceValid = false;
    failures.push("exact Notes owner was not consistent");
  }

  // ── Direction verdicts ───────────────────────────────────────────────
  const failedDirections: NativeResizeDirection[] = [];
  for (const direction of NATIVE_RESIZE_DIRECTIONS) {
    const trial = receipt.directions.find((t) => t.direction === direction);
    if (!trial) continue;
    if (trial.disposition === "EVALUABLE_FAIL") {
      failedDirections.push(direction);
    } else if (trial.disposition === "EVALUABLE_PASS") {
      // A pass claim must be backed by at least one geometrically clean
      // attempt — re-validate rather than trusting the probe's boolean.
      const backed = trial.attempts.some(
        (attempt) => validateDirectionTrial(direction, attempt).pass,
      );
      if (!backed) {
        failedDirections.push(direction);
        failures.push(
          `direction ${direction} claimed pass without a clean attempt`,
        );
      }
    }
  }
  const allDirectionsPass =
    evidenceValid &&
    failedDirections.length === 0 &&
    receipt.directions.length === NATIVE_RESIZE_DIRECTIONS.length &&
    receipt.directions.every((t) => t.disposition === "EVALUABLE_PASS");

  // ── Non-directional contract ─────────────────────────────────────────
  const settleOk = validateSettleProof(receipt.settleProof, failures);
  const minOk = validateMinClamp(receipt.minClamp, failures);
  const partitionOk = validateModePartition(receipt.modePartition, failures);
  const persistOk = validatePersistence(receipt.persistence, failures);
  const morphOk = validateMorph(receipt.morph, failures);
  if (!receipt.cleanedUp) failures.push("probe did not clean up its app instance");
  const motionOk = receipt.motionContractPass !== false;
  if (!motionOk) {
    failures.push(
      "glass motion contract failed — the direction fallback branch does not excuse motion regression",
    );
  }
  const nonDirectionalContractPass =
    settleOk &&
    minOk &&
    partitionOk &&
    persistOk &&
    morphOk &&
    receipt.cleanedUp &&
    receipt.ownerConsistent &&
    motionOk;

  const productPass = allDirectionsPass && nonDirectionalContractPass && evidenceValid;
  const landingReady = evidenceValid && nonDirectionalContractPass;
  const recommendedRootCallDisposition: NotesLiveResizeVerdict["recommendedRootCallDisposition"] =
    !evidenceValid
      ? "undecidable"
      : allDirectionsPass
        ? "remove"
        : failedDirections.length > 0
          ? "retain"
          : "undecidable";

  return {
    evidenceValid,
    productPass,
    nonDirectionalContractPass,
    landingReady,
    failedDirections,
    recommendedRootCallDisposition,
    failures,
  };
}
