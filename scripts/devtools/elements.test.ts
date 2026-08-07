import { describe, expect, test } from "bun:test";
import {
  classify,
  semanticProjection,
  snapshot,
  type ProjectionProofMode,
} from "./elements.ts";

const targetReceipt = { classification: "ok" };
const transport = { status: "ok" };

function projection(
  quality: "complete" | "partial" | "unsupported",
  reasonCodes: string[],
  mode: ProjectionProofMode = "inspection",
) {
  return semanticProjection({
    semanticSurface: "settings",
    projectionVersion: 1,
    projectionQuality: quality,
    reasonCodes,
  }, mode);
}

describe("PF-004 semantic projection", () => {
  test("complete projection exposes action, focus, ownership, and measurement facts", () => {
    const elements = snapshot([{
      semanticId: "choice:0:theme",
      type: "choice",
      role: "settings-row",
      source: "settings",
      selectable: true,
      selected: true,
      focused: false,
      actionDisabled: null,
    }]);
    const complete = projection("complete", [], "action");

    expect(classify(targetReceipt, transport, elements, complete)).toBe("ok");
    expect(complete.proofAllowed).toBe(true);
    expect(elements.nodes[0]).toMatchObject({
      semanticId: "choice:0:theme",
      measurementId: "semantic:choice:0:theme",
      owner: "settings",
      action: "select",
      enabled: true,
      disabledReason: null,
      focusable: true,
      selectable: true,
      activatable: true,
    });
  });

  test.each(["inspection", "action", "focus", "ax"] as ProjectionProofMode[])(
    "partial projection remains inspectable but blocks %s proof",
    (mode) => {
      const elements = snapshot([{ semanticId: "panel:about", type: "panel" }]);
      const partial = projection("partial", ["collectorUnavailable"], mode);
      expect(partial.limitationsExplicit).toBe(true);
      expect(partial.proofAllowed).toBe(false);
      expect(classify(targetReceipt, transport, elements, partial))
        .toBe("blocked-by-unsupported-projection");
    },
  );

  test("unsupported custom documents carry a typed reason and never pass", () => {
    const elements = snapshot([{ semanticId: "panel:div-prompt", type: "panel" }]);
    const unsupported = projection("unsupported", ["unsupportedCustomDocument"], "action");
    expect(unsupported.limitationsExplicit).toBe(true);
    expect(classify(targetReceipt, transport, elements, unsupported))
      .toBe("blocked-by-unsupported-projection");
  });

  test("incomplete projections without reason codes fail closed", () => {
    const elements = snapshot([{ semanticId: "panel:unknown", type: "panel" }]);
    const partial = projection("partial", [], "inspection");
    expect(partial.limitationsExplicit).toBe(false);
    expect(classify(targetReceipt, transport, elements, partial))
      .toBe("blocked-by-unsupported-projection");
  });

  test("duplicate semantic ids are invalid identity, not partial proof", () => {
    const elements = snapshot([
      { semanticId: "same", type: "button" },
      { semanticId: "same", type: "button" },
    ]);
    expect(classify(targetReceipt, transport, elements, projection("complete", [])))
      .toBe("invalid-identity");
    expect(elements.duplicateSemanticIds).toEqual(["same"]);
  });
});
