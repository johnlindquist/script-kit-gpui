import type { EvidenceClass } from "./evidence-class.ts";

export type TaskProofRequirement =
  | "static-inventory"
  | "unit-behavior"
  | "fixture-contract"
  | "direct-runtime";

export interface TaskProofPolicy {
  taskId: string;
  requirement: TaskProofRequirement;
  acceptedEvidenceClasses: readonly EvidenceClass[];
  provesRuntimeBehavior: boolean;
}

const runtimeClasses = [
  "RUNTIME_HIDDEN",
  "RUNTIME_VISIBLE",
  "PACKAGED_APP",
  "DIRECT_RUNTIME_PROOF",
] as const satisfies readonly EvidenceClass[];

function policy(taskId: string): TaskProofPolicy {
  if (taskId === "RPT-001" || taskId === "PF-009") {
    return {
      taskId,
      requirement: "static-inventory",
      acceptedEvidenceClasses: ["STATIC_INVENTORY", "UNIT_BEHAVIOR"],
      provesRuntimeBehavior: false,
    };
  }
  if (taskId === "PF-010") {
    return {
      taskId,
      requirement: "fixture-contract",
      acceptedEvidenceClasses: ["FIXTURE_CONTRACT", "UNIT_BEHAVIOR", ...runtimeClasses],
      provesRuntimeBehavior: false,
    };
  }
  if (
    taskId.startsWith("GOV-") ||
    ["PF-001", "PF-002", "PF-003", "PF-011", "GEO-001"].includes(taskId)
  ) {
    return {
      taskId,
      requirement: "unit-behavior",
      acceptedEvidenceClasses: ["UNIT_BEHAVIOR", ...runtimeClasses],
      provesRuntimeBehavior: false,
    };
  }
  return {
    taskId,
    requirement: "direct-runtime",
    acceptedEvidenceClasses: runtimeClasses,
    provesRuntimeBehavior: true,
  };
}

function range(prefix: string, total: number): string[] {
  return Array.from(
    { length: total },
    (_, index) => `${prefix}-${String(index + 1).padStart(3, "0")}`,
  );
}

export const TASK_PROOF_POLICIES: ReadonlyMap<string, TaskProofPolicy> = new Map(
  [
    "RPT-001",
    ...range("SAFE", 4),
    ...range("PF", 12),
    ...range("UX", 18),
    ...range("WF", 24),
    ...range("GEO", 9),
    ...range("GOV", 7),
  ].map((taskId) => [taskId, policy(taskId)]),
);

export function taskProofPolicy(taskId: string): TaskProofPolicy | null {
  return TASK_PROOF_POLICIES.get(taskId) ?? null;
}
