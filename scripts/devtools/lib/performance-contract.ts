/**
 * Observation taxonomy for performance claims. A fast protocol echo, a
 * semantic frame, provider output, and an actually composited frame answer
 * different product questions; one must never silently stand in for another.
 */

export const PERFORMANCE_OBSERVATION_CLASSES = [
  "STATE_ECHO",
  "SEMANTIC_FRAME",
  "FRAME_CALLBACK_PROXY",
  "COMPOSITOR_PAINT",
  "SCREEN_CAPTURE",
  "PROVIDER_EVENT_STREAM",
  "FIRST_VISIBLE_OUTPUT",
] as const;

export type PerformanceObservationClass =
  (typeof PERFORMANCE_OBSERVATION_CLASSES)[number];

const knownClasses = new Set<string>(PERFORMANCE_OBSERVATION_CLASSES);
const paintClasses = new Set<string>(["COMPOSITOR_PAINT", "SCREEN_CAPTURE"]);
const visibleOnlyClasses = new Set<string>([
  "COMPOSITOR_PAINT",
  "SCREEN_CAPTURE",
  "FIRST_VISIBLE_OUTPUT",
]);
const ownerRatifiedStatuses = new Set<string>([
  "USER_DECLARED_RATIFIED",
]);

export interface PerformanceBudget {
  p50Ms?: number;
  p95Ms?: number;
  maxMs?: number;
  ratificationStatus?: string;
  approvalId?: string | null;
  ratificationReference?: string | null;
}

export interface PerformanceObservationContract {
  metricKind: string;
  observationClass: string;
  observationPoint?: string;
  observationPoints?: readonly string[];
  measuresPaint: boolean;
  evidenceClass?: string;
  runtimeEvidenceClass?: string;
  proposedBudget?: PerformanceBudget;
  budgetRatification?: {
    status?: string;
    approvalId?: string | null;
  };
  safety?: Record<string, unknown>;
}

export interface PerformanceContractValidation {
  pass: boolean;
  observationClass: PerformanceObservationClass | null;
  measuresPaint: boolean;
  runtimeEvidenceClass: string | null;
  thresholdEnforced: boolean;
  errors: string[];
}

export function validatePerformanceContract(
  contract: PerformanceObservationContract,
  options: { enforce?: boolean; sampleCount?: number } = {},
): PerformanceContractValidation {
  const errors: string[] = [];
  const observationClass = knownClasses.has(contract.observationClass)
    ? contract.observationClass as PerformanceObservationClass
    : null;
  if (observationClass === null) {
    errors.push(`unsupported performance observation class: ${contract.observationClass}`);
  }
  if (typeof contract.metricKind !== "string" || contract.metricKind.length === 0) {
    errors.push("performance contract requires a nonempty metric identity");
  }
  const points = [
    ...(typeof contract.observationPoint === "string"
      ? [contract.observationPoint]
      : []),
    ...(Array.isArray(contract.observationPoints)
      ? contract.observationPoints
      : []),
  ];
  if (points.length === 0 || points.some((point) => point.trim().length === 0)) {
    errors.push("performance contract requires explicit observation points");
  }
  if (typeof contract.measuresPaint !== "boolean") {
    errors.push("performance contract must explicitly declare whether it measures paint");
  } else if (observationClass !== null) {
    const actuallyObservesPaint = paintClasses.has(observationClass);
    if (contract.measuresPaint !== actuallyObservesPaint) {
      errors.push(
        actuallyObservesPaint
          ? `${observationClass} is a painted-frame observation and cannot hide that fact`
          : `${observationClass} does not measure composited or captured paint`,
      );
    }
  }

  const runtimeEvidenceClass = contract.runtimeEvidenceClass ?? contract.evidenceClass ?? null;
  if (
    observationClass !== null &&
    visibleOnlyClasses.has(observationClass) &&
    (runtimeEvidenceClass === "RUNTIME_HIDDEN" ||
      process.env.SCRIPT_KIT_NONINTERACTIVE === "1")
  ) {
    errors.push(
      `${observationClass} requires visible execution and cannot be proven from hidden/noninteractive evidence`,
    );
  }
  if (
    observationClass === "PROVIDER_EVENT_STREAM" &&
    runtimeEvidenceClass !== "LIVE_AI"
  ) {
    errors.push("provider-event performance proof requires the LIVE_AI evidence class");
  }
  if (
    observationClass !== "PROVIDER_EVENT_STREAM" &&
    runtimeEvidenceClass === "LIVE_AI"
  ) {
    errors.push("LIVE_AI evidence cannot stand in for an application interaction metric");
  }

  const budget = contract.proposedBudget ?? {};
  const values = [budget.p50Ms, budget.p95Ms, budget.maxMs]
    .filter((value): value is number => value !== undefined);
  if (values.some((value) => !Number.isFinite(value) || value <= 0)) {
    errors.push("performance threshold values must be finite positive milliseconds");
  }
  if (
    (budget.p50Ms !== undefined && budget.p95Ms !== undefined && budget.p50Ms > budget.p95Ms) ||
    (budget.p95Ms !== undefined && budget.maxMs !== undefined && budget.p95Ms > budget.maxMs)
  ) {
    errors.push("performance thresholds must satisfy p50 <= p95 <= max");
  }

  if (options.enforce) {
    const ratificationStatus =
      contract.budgetRatification?.status ?? budget.ratificationStatus ?? "UNRATIFIED";
    const approvalId =
      contract.budgetRatification?.approvalId ??
      budget.approvalId ??
      budget.ratificationReference ??
      null;
    if (!ownerRatifiedStatuses.has(ratificationStatus)) {
      errors.push("performance threshold enforcement requires an owner-ratified budget");
    }
    if (typeof approvalId !== "string" || approvalId.trim().length === 0) {
      errors.push("performance threshold enforcement requires an explicit approval reference");
    }
    if (values.length === 0) {
      errors.push("performance threshold enforcement requires declared threshold values");
    }
    if (
      typeof options.sampleCount !== "number" ||
      !Number.isSafeInteger(options.sampleCount) ||
      options.sampleCount <= 0
    ) {
      errors.push("performance threshold enforcement requires at least one observed sample");
    }
  }

  return {
    pass: errors.length === 0,
    observationClass,
    measuresPaint: contract.measuresPaint === true,
    runtimeEvidenceClass,
    thresholdEnforced: options.enforce === true && errors.length === 0,
    errors,
  };
}

export function assertPerformanceContract(
  contract: PerformanceObservationContract,
  options: { enforce?: boolean; sampleCount?: number } = {},
): PerformanceContractValidation {
  const validation = validatePerformanceContract(contract, options);
  if (!validation.pass) {
    throw new Error(`invalid performance observation contract: ${validation.errors.join("; ")}`);
  }
  return validation;
}
