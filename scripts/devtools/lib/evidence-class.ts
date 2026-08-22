/**
 * Classify receipt evidence from observations, never from a CLI's name.
 *
 * A registered target and a successful RPC do not establish whether the
 * operator saw a window. Runtime proof is therefore hidden/visible only when
 * a producer observed that exact boolean on the receipt's own target.
 */

type ReceiptObject = Record<string, unknown>;

export const EVIDENCE_CLASSES = [
  "STATIC_INVENTORY",
  "UNIT_BEHAVIOR",
  "FIXTURE_CONTRACT",
  "RUNTIME_HIDDEN",
  "RUNTIME_VISIBLE",
  "RUNTIME_VISIBILITY_UNVERIFIED",
  "RUNTIME_UNSCOPED",
  "DIRECT_RUNTIME_PROOF",
  "PACKAGED_APP",
  "LIVE_AI",
  "UNCLASSIFIED",
] as const;

export type EvidenceClass = (typeof EVIDENCE_CLASSES)[number];

const supportedEvidenceClasses = new Set<string>(EVIDENCE_CLASSES);

function object(value: unknown): ReceiptObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as ReceiptObject
    : {};
}

export interface EvidenceObservation {
  evidenceClass: EvidenceClass;
  observedWindowVisible: boolean | null;
  visibilitySources: string[];
  errors: string[];
}

export function classifyReceiptEvidence(receipt: ReceiptObject): EvidenceObservation {
  const candidates: Array<[string, unknown]> = [
    ["resolvedTarget.visible", object(receipt.resolvedTarget).visible],
    ["target.visible", object(receipt.target).visible],
    ["targetBefore.visible", object(receipt.targetBefore).visible],
    ["windowVisible", receipt.windowVisible],
    ["state.windowVisible", object(receipt.state).windowVisible],
    ["runtime.windowVisible", object(receipt.runtime).windowVisible],
    ["safety.observedWindowVisible", object(receipt.safety).observedWindowVisible],
  ];
  const observations = candidates.filter(
    (candidate): candidate is [string, boolean] =>
      typeof candidate[1] === "boolean",
  );
  const distinctVisibilityValues = new Set(
    observations.map(([, visible]) => visible),
  );
  const observedWindowVisible =
    distinctVisibilityValues.size === 1 ? observations[0]![1] : null;
  const errors: string[] = [];
  if (distinctVisibilityValues.size > 1) {
    errors.push(
      `target visibility observations disagree: ${observations
        .map(([source, visible]) => `${source}=${visible}`)
        .join(", ")}`,
    );
  }

  const declared =
    typeof receipt.evidenceClass === "string" ? receipt.evidenceClass : null;
  if (declared !== null && !supportedEvidenceClasses.has(declared)) {
    errors.push(`unsupported receipt evidence class: ${declared}`);
  }

  let evidenceClass: EvidenceClass;
  if (declared !== null && supportedEvidenceClasses.has(declared)) {
    evidenceClass = declared as EvidenceClass;
  } else if (Object.keys(object(receipt.transaction)).length > 0) {
    evidenceClass = observedWindowVisible === false
      ? "RUNTIME_HIDDEN"
      : observedWindowVisible === true
        ? "RUNTIME_VISIBLE"
        : "RUNTIME_VISIBILITY_UNVERIFIED";
  } else if (
    Array.isArray(receipt.targets) ||
    Array.isArray(receipt.windows) ||
    typeof receipt.targetCount === "number"
  ) {
    evidenceClass = "RUNTIME_UNSCOPED";
  } else {
    evidenceClass = "UNCLASSIFIED";
  }

  if (evidenceClass === "RUNTIME_HIDDEN" && observedWindowVisible !== false) {
    errors.push(
      observedWindowVisible === true
        ? "hidden runtime evidence observed a visible target"
        : "hidden runtime evidence requires an observed hidden target",
    );
  }
  if (evidenceClass === "RUNTIME_VISIBLE" && observedWindowVisible !== true) {
    errors.push(
      observedWindowVisible === false
        ? "visible runtime evidence observed a hidden target"
        : "visible runtime evidence requires an observed visible target",
    );
  }

  if (
    process.env.SCRIPT_KIT_NONINTERACTIVE === "1" &&
    (observedWindowVisible === true || evidenceClass === "RUNTIME_VISIBLE")
  ) {
    errors.push(
      "noninteractive runtime evidence cannot inspect a visible target",
    );
  }

  return {
    evidenceClass,
    observedWindowVisible,
    visibilitySources: observations.map(([source]) => source),
    errors,
  };
}
