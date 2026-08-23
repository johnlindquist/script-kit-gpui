import { createHash } from "node:crypto";
import { describe, expect, test } from "bun:test";
import {
  nativeFooterActivationProof,
  semanticAxParity,
  semanticFocusGraph,
} from "./focus.ts";

const label = "Run";
const labelSha256 = createHash("sha256").update(label).digest("hex");
const state = {
  activeFooter: {
    owner: "native",
    buttons: [{
      id: "footer-action:run",
      action: "run",
      label,
      enabled: true,
      actionDisabled: null,
    }],
  },
};
const layout = {
  fidelity: {
    appKit: {
      nodes: [{
        id: "script-kit-footer-button-run",
        accessibilityIdentifier: "footer-action:run",
        accessibilityRole: "AXButton",
        accessibilityLabelSha256: labelSha256,
        accessibilityLabelLength: label.length,
        accessibilityEnabled: true,
        accessibilityFocused: false,
        accessibilityElement: true,
        actionSelector: "runFooterAction:",
        hidden: false,
        alpha: 1,
        screenshotFrame: { x: 600, y: 450, width: 80, height: 32 },
      }],
    },
  },
};

describe("semantic to AppKit accessibility parity", () => {
  test("joins the canonical footer descriptor to its native AX peer", () => {
    const parity = semanticAxParity(state, layout);
    expect(parity.complete).toBe(true);
    expect(parity.peerCount).toBe(1);
    expect(parity.peers[0].parityPass).toBe(true);
    expect(parity.peers[0].axPeer?.structuralId).toBe("script-kit-footer-button-run");
  });

  test("fails closed for a missing peer, wrong action, or enabled mismatch", () => {
    const missing = semanticAxParity(state, { fidelity: { appKit: { nodes: [] } } });
    const wrong = structuredClone(layout);
    wrong.fidelity.appKit.nodes[0].actionSelector = "actionsFooterAction:";
    wrong.fidelity.appKit.nodes[0].accessibilityEnabled = false;
    const mismatch = semanticAxParity(state, wrong);
    expect(missing.complete).toBe(false);
    expect(missing.peers[0].errors).toContain("missingAxPeer");
    expect(mismatch.complete).toBe(false);
    expect(mismatch.peers[0].errors).toContain("actionMismatch");
    expect(mismatch.peers[0].errors).toContain("enabledMismatch");
  });

  test("rejects hidden or non-element AX peers", () => {
    const hidden = structuredClone(layout);
    hidden.fidelity.appKit.nodes[0].hidden = true;
    hidden.fidelity.appKit.nodes[0].accessibilityElement = false;
    const parity = semanticAxParity(state, hidden);
    expect(parity.complete).toBe(false);
    expect(parity.peers[0].errors).toContain("hiddenAxPeer");
    expect(parity.peers[0].errors).toContain("notAccessibilityElement");
  });

  test("requires explicitly observed finite visibility and positive native geometry", () => {
    const variants: Array<[string, Record<string, unknown>, string]> = [
      ["missing alpha", { alpha: undefined }, "invalidAxVisibility"],
      ["NaN alpha", { alpha: NaN }, "invalidAxVisibility"],
      ["infinite alpha", { alpha: Infinity }, "invalidAxVisibility"],
      ["negative alpha", { alpha: -1 }, "invalidAxVisibility"],
      ["over-opaque alpha", { alpha: 2 }, "invalidAxVisibility"],
      ["missing hidden observation", { hidden: undefined }, "invalidAxVisibility"],
      ["missing bounds", { screenshotFrame: undefined }, "invalidAxGeometry"],
      ["zero-area bounds", { screenshotFrame: { x: 1, y: 2, width: 0, height: 32 } }, "invalidAxGeometry"],
      ["negative-area bounds", { screenshotFrame: { x: 1, y: 2, width: 80, height: -1 } }, "invalidAxGeometry"],
    ];

    for (const [name, overrides, reason] of variants) {
      const candidate = structuredClone(layout);
      Object.assign(candidate.fidelity.appKit.nodes[0], overrides);
      const parity = semanticAxParity(state, candidate);
      expect(parity.complete, name).toBe(false);
      expect(parity.peers[0].errors, name).toContain(reason);
    }
  });

  test("rejects duplicate native structural owners and raw accessibility labels", () => {
    const duplicate = structuredClone(layout);
    duplicate.fidelity.appKit.nodes.push({
      ...structuredClone(duplicate.fidelity.appKit.nodes[0]),
      accessibilityIdentifier: "footer-action:other",
    });
    const ambiguous = semanticAxParity(state, duplicate);
    expect(ambiguous.complete).toBe(false);
    expect(ambiguous.duplicateAxStructuralIds).toEqual(["script-kit-footer-button-run"]);

    const exposed = structuredClone(layout);
    Object.assign(exposed.fidelity.appKit.nodes[0], { accessibilityLabel: "Run" });
    const privateLabel = semanticAxParity(state, exposed);
    expect(privateLabel.complete).toBe(false);
    expect(privateLabel.peers[0].errors).toContain("rawAccessibilityLabelReturned");
  });

  test("disabled semantics require a reason and a disabled AX peer", () => {
    const disabledState = structuredClone(state);
    disabledState.activeFooter.buttons[0].enabled = false;
    disabledState.activeFooter.buttons[0].actionDisabled = "Nothing selected";
    const disabledLayout = structuredClone(layout);
    disabledLayout.fidelity.appKit.nodes[0].accessibilityEnabled = false;
    expect(semanticAxParity(disabledState, disabledLayout).complete).toBe(true);

    disabledState.activeFooter.buttons[0].actionDisabled = null;
    const invalid = semanticAxParity(disabledState, disabledLayout);
    expect(invalid.complete).toBe(false);
    expect(invalid.peers[0].errors).toContain("missingDisabledReason");
  });

  test("duplicate semantic owners cannot share one native AX peer", () => {
    const duplicate = structuredClone(state);
    duplicate.activeFooter.buttons.push(structuredClone(duplicate.activeFooter.buttons[0]));
    const parity = semanticAxParity(duplicate, layout);
    expect(parity.peerCount).toBe(2);
    expect(parity.duplicateSemanticIds).toEqual(["footer-action:run"]);
    expect(parity.complete).toBe(false);
  });

  test("native AX parity needs a real semantic action and accessibility name", () => {
    const noAction = structuredClone(state);
    delete (noAction.activeFooter.buttons[0] as Record<string, unknown>).action;
    const actionParity = semanticAxParity(noAction, layout);
    expect(actionParity.complete).toBe(false);
    expect(actionParity.peers[0].errors).toContain("missingSemanticAction");

    const noName = structuredClone(state);
    noName.activeFooter.buttons[0].label = "";
    const labelParity = semanticAxParity(noName, layout);
    expect(labelParity.complete).toBe(false);
    expect(labelParity.peers[0].errors).toContain("missingAccessibilityLabel");
  });
});

test("semantic focus graph has reciprocal forward and backward edges", () => {
  const graph = semanticFocusGraph([
    { semanticId: "input:filter", type: "input", selectable: true, focused: true },
    { semanticId: "row:one", type: "button", selectable: true },
    { semanticId: "footer-action:run", type: "button", selectable: true },
  ]);
  expect(graph.reciprocal).toBe(true);
  expect(graph.nodes[0]).toEqual({ semanticId: "input:filter", previous: null, next: "row:one" });
  expect(graph.nodes[2]).toEqual({ semanticId: "footer-action:run", previous: "row:one", next: null });
});

test("focus graph rejects hidden focusables and duplicate identities", () => {
  const graph = semanticFocusGraph([
    { semanticId: "input:filter", type: "input", selectable: true },
    { semanticId: "footer-action:run", type: "button", selectable: true, hidden: true },
    { semanticId: "input:filter", type: "button", selectable: true },
  ]);
  expect(graph.reciprocal).toBe(false);
  expect(graph.hiddenFocusableIds).toEqual(["footer-action:run"]);
  expect(graph.duplicateSemanticIds).toEqual(["input:filter"]);
});

test("focus graph rejects multiple owners or a focused non-focusable node", () => {
  const multiple = semanticFocusGraph([
    { semanticId: "input:filter", type: "input", focused: true },
    { semanticId: "footer-action:run", type: "button", focused: true },
  ]);
  expect(multiple.focusedSemanticIds).toEqual([
    "input:filter",
    "footer-action:run",
  ]);
  expect(multiple.reciprocal).toBe(false);

  const inaccessible = semanticFocusGraph([
    { semanticId: "footer-action:disabled", type: "button", selectable: false, focused: true },
  ]);
  expect(inaccessible.focusedSemanticIds).toEqual(["footer-action:disabled"]);
  expect(inaccessible.reciprocal).toBe(false);
});

test("focus graph requires one actual visible focused owner", () => {
  const empty = semanticFocusGraph([]);
  expect(empty.nodes).toEqual([]);
  expect(empty.reciprocal).toBe(false);

  const unowned = semanticFocusGraph([
    { semanticId: "input:filter", type: "input", selectable: true },
  ]);
  expect(unowned.focusedSemanticIds).toEqual([]);
  expect(unowned.reciprocal).toBe(false);

  const explicitlyUnfocusable = semanticFocusGraph([
    { semanticId: "input:filter", type: "input", focusable: false, focused: true },
  ]);
  expect(explicitlyUnfocusable.reciprocal).toBe(false);
});

describe("native footer activation proof", () => {
  const enabledResult = {
    host: "NativeFooter",
    actionId: "footer-action:actions",
    ok: true,
    nativeFooterActivation: {
      semanticId: "footer-action:actions",
      accessibilityRole: "AXButton",
      actionSelector: "actionsFooterAction:",
      expectedActionSelector: "actionsFooterAction:",
      descriptorEnabled: true,
      appkitEnabled: true,
      refusedDisabled: false,
      dispatched: true,
      errorCode: null,
    },
  };

  test("requires the exact native host, action, and observed postcondition", () => {
    expect(nativeFooterActivationProof(enabledResult, "footer-action:actions", true).complete).toBe(true);
    const wrongHost = structuredClone(enabledResult);
    wrongHost.host = "MainList";
    const invalid = nativeFooterActivationProof(wrongHost, "footer-action:actions", true);
    expect(invalid.complete).toBe(false);
    expect(invalid.errors).toContain("wrongHost");
    expect(nativeFooterActivationProof(enabledResult, "footer-action:actions", false).errors).toContain(
      "missingPostcondition",
    );
  });

  test("requires disabled controls to refuse before dispatch and preserve state", () => {
    const disabled = structuredClone(enabledResult);
    disabled.ok = false;
    disabled.errorCode = "action_disabled";
    disabled.nativeFooterActivation.descriptorEnabled = false;
    disabled.nativeFooterActivation.appkitEnabled = false;
    disabled.nativeFooterActivation.refusedDisabled = true;
    disabled.nativeFooterActivation.dispatched = false;
    disabled.nativeFooterActivation.errorCode = "action_disabled";
    expect(nativeFooterActivationProof(disabled, "footer-action:actions", true, true).complete).toBe(true);
    disabled.nativeFooterActivation.dispatched = true;
    expect(nativeFooterActivationProof(disabled, "footer-action:actions", true, true).errors).toContain(
      "disabledActivationNotRefused",
    );
  });

  test("cannot trust missing or self-consistently forged native selectors", () => {
    const missing = structuredClone(enabledResult);
    delete (missing.nativeFooterActivation as Record<string, unknown>).actionSelector;
    delete (missing.nativeFooterActivation as Record<string, unknown>).expectedActionSelector;
    expect(nativeFooterActivationProof(missing, "footer-action:actions", true).errors)
      .toContain("wrongAction");

    const forged = structuredClone(enabledResult);
    forged.nativeFooterActivation.actionSelector = "unreviewedDangerousAction:";
    forged.nativeFooterActivation.expectedActionSelector = "unreviewedDangerousAction:";
    expect(nativeFooterActivationProof(forged, "footer-action:actions", true).errors)
      .toContain("wrongAction");

    const empty = structuredClone(enabledResult);
    empty.actionId = "";
    empty.nativeFooterActivation.semanticId = "";
    expect(nativeFooterActivationProof(empty, "", true).errors)
      .toContain("invalidExpectedSemanticId");
  });

  test("disabled refusal requires both owners to be disabled and an unsuccessful dispatch", () => {
    const disguised = structuredClone(enabledResult);
    Object.assign(disguised, { errorCode: "action_disabled" });
    Object.assign(disguised.nativeFooterActivation, {
      refusedDisabled: true,
      dispatched: false,
      errorCode: "action_disabled",
    });
    expect(nativeFooterActivationProof(disguised, "footer-action:actions", true, true).errors)
      .toContain("disabledActivationNotRefused");
  });
});
