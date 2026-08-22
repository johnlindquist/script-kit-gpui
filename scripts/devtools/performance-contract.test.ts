import { describe, expect, test } from "bun:test";
import {
  assertPerformanceContract,
  PERFORMANCE_OBSERVATION_CLASSES,
  validatePerformanceContract,
  type PerformanceObservationContract,
} from "./lib/performance-contract.ts";

function stateEcho(
  overrides: Partial<PerformanceObservationContract> = {},
): PerformanceObservationContract {
  return {
    metricKind: "protocol_set_filter_to_state_echo",
    observationClass: "STATE_ECHO",
    observationPoint: "waitForResult.stateMatch.inputValue",
    measuresPaint: false,
    runtimeEvidenceClass: "RUNTIME_HIDDEN",
    ...overrides,
  };
}

describe("truthful performance observation contracts", () => {
  test("the taxonomy distinguishes all seven independently meaningful observation layers", () => {
    expect(PERFORMANCE_OBSERVATION_CLASSES).toEqual([
      "STATE_ECHO",
      "SEMANTIC_FRAME",
      "FRAME_CALLBACK_PROXY",
      "COMPOSITOR_PAINT",
      "SCREEN_CAPTURE",
      "PROVIDER_EVENT_STREAM",
      "FIRST_VISIBLE_OUTPUT",
    ]);
    expect(assertPerformanceContract(stateEcho()).pass).toBe(true);
  });

  test("state echoes, semantic frames, frame callbacks, and provider events are not paint", () => {
    for (const observationClass of [
      "STATE_ECHO",
      "SEMANTIC_FRAME",
      "FRAME_CALLBACK_PROXY",
      "PROVIDER_EVENT_STREAM",
    ]) {
      const result = validatePerformanceContract(
        stateEcho({
          observationClass,
          measuresPaint: true,
          runtimeEvidenceClass:
            observationClass === "PROVIDER_EVENT_STREAM" ? "LIVE_AI" : "RUNTIME_HIDDEN",
        }),
      );
      expect(result.pass).toBe(false);
      expect(result.errors.join("\n")).toContain("does not measure composited or captured paint");
    }
  });

  test("hidden execution cannot claim compositor paint, screenshots, or visible output", () => {
    for (const observationClass of [
      "COMPOSITOR_PAINT",
      "SCREEN_CAPTURE",
      "FIRST_VISIBLE_OUTPUT",
    ]) {
      const result = validatePerformanceContract(
        stateEcho({
          observationClass,
          measuresPaint: observationClass !== "FIRST_VISIBLE_OUTPUT",
        }),
      );
      expect(result.pass).toBe(false);
      expect(result.errors.join("\n")).toContain("requires visible execution");
    }
  });

  test("provider streams cannot masquerade as hidden app interaction or paint", () => {
    const wrongClass = validatePerformanceContract(
      stateEcho({ observationClass: "PROVIDER_EVENT_STREAM" }),
    );
    expect(wrongClass.errors).toContain(
      "provider-event performance proof requires the LIVE_AI evidence class",
    );
    const actualProvider = validatePerformanceContract(
      stateEcho({
        observationClass: "PROVIDER_EVENT_STREAM",
        runtimeEvidenceClass: "LIVE_AI",
      }),
    );
    expect(actualProvider.pass).toBe(true);
  });

  test("thresholds cannot be enforced without owner ratification, reference, and observed samples", () => {
    const pending = stateEcho({
      proposedBudget: {
        p50Ms: 25,
        p95Ms: 50,
        maxMs: 150,
        ratificationStatus: "USER_RATIFICATION_PENDING",
      },
    });
    expect(validatePerformanceContract(pending).pass).toBe(true);
    const unratified = validatePerformanceContract(pending, {
      enforce: true,
      sampleCount: 0,
    });
    expect(unratified.errors).toEqual([
      "performance threshold enforcement requires an owner-ratified budget",
      "performance threshold enforcement requires an explicit approval reference",
      "performance threshold enforcement requires at least one observed sample",
    ]);

    const ratified = validatePerformanceContract(
      {
        ...pending,
        budgetRatification: {
          status: "USER_DECLARED_RATIFIED",
          approvalId: "product-owner-2026-08-21",
        },
      },
      { enforce: true, sampleCount: 12 },
    );
    expect(ratified.pass).toBe(true);
    expect(ratified.thresholdEnforced).toBe(true);
  });

  test("negative or invented ratification labels cannot masquerade as owner approval", () => {
    for (const status of [
      "UNRATIFIED",
      "NOT_RATIFIED",
      "SELF_RATIFIED",
      "USER_RATIFICATION_PENDING",
      "USER_DECLARED_RATIFIED_PENDING",
      "USER_DECLARED_RATIFIED_EXTRA",
      "user_declared_ratified",
    ]) {
      const result = validatePerformanceContract(
        stateEcho({
          proposedBudget: {
            p50Ms: 25,
            p95Ms: 50,
            maxMs: 150,
            ratificationStatus: status,
            approvalId: "counterfeit-approval-reference",
          },
        }),
        { enforce: true, sampleCount: 12 },
      );

      expect(result.pass, status).toBe(false);
      expect(result.thresholdEnforced, status).toBe(false);
      expect(result.errors, status).toContain(
        "performance threshold enforcement requires an owner-ratified budget",
      );
    }
  });

  test("enforcement cannot omit, invent, or fractionally count observed samples", () => {
    const ratified = stateEcho({
      proposedBudget: {
        p50Ms: 25,
        p95Ms: 50,
        maxMs: 150,
      },
      budgetRatification: {
        status: "USER_DECLARED_RATIFIED",
        approvalId: "product-owner-2026-08-21",
      },
    });
    for (const sampleCount of [
      undefined,
      0,
      -1,
      0.5,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.MAX_SAFE_INTEGER + 1,
    ]) {
      const result = validatePerformanceContract(
        ratified,
        sampleCount === undefined
          ? { enforce: true }
          : { enforce: true, sampleCount },
      );

      expect(result.pass, String(sampleCount)).toBe(false);
      expect(result.thresholdEnforced, String(sampleCount)).toBe(false);
      expect(result.errors, String(sampleCount)).toContain(
        "performance threshold enforcement requires at least one observed sample",
      );
    }
    expect(validatePerformanceContract(ratified).pass).toBe(true);
    expect(
      validatePerformanceContract(ratified, { enforce: true, sampleCount: 1 }).pass,
    ).toBe(true);
  });

  test("threshold ordering, unknown observations, and missing measurement points fail closed", () => {
    const result = validatePerformanceContract(
      stateEcho({
        metricKind: "",
        observationClass: "MAGIC_FASTNESS",
        observationPoint: "",
        proposedBudget: { p50Ms: 100, p95Ms: 50, maxMs: 40 },
      }),
    );
    expect(result.errors).toContain(
      "unsupported performance observation class: MAGIC_FASTNESS",
    );
    expect(result.errors).toContain(
      "performance contract requires a nonempty metric identity",
    );
    expect(result.errors).toContain(
      "performance contract requires explicit observation points",
    );
    expect(result.errors).toContain(
      "performance thresholds must satisfy p50 <= p95 <= max",
    );
    expect(() => assertPerformanceContract(stateEcho({ measuresPaint: true }))).toThrow(
      "invalid performance observation contract",
    );
  });
});
