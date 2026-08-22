import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
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

  test("non-selectable source status and disabled recovery actions never become activatable", () => {
    const status = snapshot([{
      semanticId: "source:clipboard:loading",
      type: "choice",
      role: "source-status",
      source: "clipboard",
      selectable: false,
      statusKind: "loading",
    }]);
    expect(status.nodes[0]).toMatchObject({
      action: null,
      enabled: false,
      selectable: false,
      focusable: false,
      activatable: false,
    });
    expect(classify(targetReceipt, transport, status, projection("complete", [], "action")))
      .toBe("blocked-by-missing-primitive");

    const recovery = snapshot([{
      semanticId: "recovery:retry",
      type: "button",
      role: "ai-recovery-action",
      source: "AiRecoveryCard",
      selectable: false,
      actionDisabled: "ProviderUnavailable",
    }]);
    expect(recovery.nodes[0]).toMatchObject({
      action: "activate",
      enabled: false,
      disabledReason: "ProviderUnavailable",
      activatable: false,
    });
  });

  test("production privacy descriptors retain the real observed fingerprint, never the semantic ID", () => {
    const raw = "private clipboard payload";
    const fingerprint = `sha256:${createHash("sha256").update(raw).digest("hex")}`;
    const redacted = snapshot([{
      semanticId: "choice:0:sha256-captured",
      type: "choice",
      selectable: true,
      content: {
        text: {
          contentKind: "externalContent",
          charLength: raw.length,
          byteLength: Buffer.byteLength(raw),
          fingerprint,
          rawContentReturned: false,
        },
      },
    }]);
    expect(redacted.nodes[0].content).toMatchObject({
      contentKind: "externalContent",
      length: raw.length,
      fingerprint,
      rawContentReturned: false,
      source: "production-protocol-redaction",
    });
    expect(JSON.stringify(redacted)).not.toContain(raw);
    expect(redacted.privacyViolationSemanticIds).toEqual([]);
  });

  test("malformed redaction or leaked cleartext is invalid privacy, not a synthetic safe hash", () => {
    const base = {
      semanticId: "choice:0:secret",
      type: "choice",
      content: {
        text: {
          contentKind: "secret",
          charLength: 5,
          byteLength: 5,
          fingerprint: `sha256:${"a".repeat(64)}`,
          rawContentReturned: false,
        },
      },
    };
    for (const node of [
      { ...base, text: "LEAKED_SECRET" },
      { ...base, content: { text: { ...base.content.text, rawContentReturned: true } } },
      { ...base, content: { text: { ...base.content.text, fingerprint: "synthetic" } } },
    ]) {
      const elements = snapshot([node]);
      expect(elements.privacyViolationSemanticIds).toEqual([base.semanticId]);
      expect(classify(targetReceipt, transport, elements, projection("complete", [])))
        .toBe("invalid-privacy");
      expect(JSON.stringify(elements)).not.toContain("LEAKED_SECRET");
    }
  });

  test("focus proof requires exactly one observed focused semantic node", () => {
    const none = snapshot([{ semanticId: "input:filter", type: "input", focused: false }]);
    expect(classify(targetReceipt, transport, none, projection("complete", [], "focus")))
      .toBe("blocked-by-missing-primitive");
    const duplicate = snapshot([
      { semanticId: "input:filter", type: "input", focused: true },
      { semanticId: "button:run", type: "button", focused: true },
    ]);
    expect(classify(targetReceipt, transport, duplicate, projection("complete", [], "focus")))
      .toBe("invalid-identity");
  });

  test("semantic rows alone cannot masquerade as native accessibility evidence", () => {
    const elements = snapshot([{ semanticId: "button:run", type: "button" }]);
    const semanticOnly = projection("complete", [], "ax");
    expect(semanticOnly.nativeAccessibilityObserved).toBe(false);
    expect(semanticOnly.proofAllowed).toBe(false);
    expect(classify(targetReceipt, transport, elements, semanticOnly))
      .toBe("blocked-by-missing-primitive");

    const observed = semanticProjection({
      semanticSurface: "settings",
      projectionVersion: 1,
      projectionQuality: "complete",
      reasonCodes: [],
      accessibilityProjection: {
        source: "native-appkit-accessibility",
        complete: true,
        peerCount: 1,
      },
    }, "ax");
    expect(observed.nativeAccessibilityObserved).toBe(true);
    expect(observed.proofAllowed).toBe(true);
    expect(classify(targetReceipt, transport, elements, observed)).toBe("ok");
  });
});
