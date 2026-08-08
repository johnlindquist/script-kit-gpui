#!/usr/bin/env bun
/** Focus/keyboard-ownership inspection. Shared transport/args/receipts live in lib/client.ts. */

import { createHash } from "node:crypto";
import {
  type JsonObject,
  asArray,
  classifyEnvelopes,
  finishReceipt,
  parseTargetArgs,
  requestId,
  responseOf,
  rpc,
  startClock,
} from "./lib/client.ts";
import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic } from "./lib/privacy.ts";
import { maybeStartAndShow, resolveTargetReceipt } from "./lib/target-identity.ts";

function usage() {
  return "Usage:\n  bun scripts/devtools/focus.ts inspect [target args] [--limit <n>] [--proof-mode focus|ax]";
}

function focusedNode(nodes: JsonObject[], focusedSemanticId: unknown) {
  const id = String(focusedSemanticId ?? "");
  return nodes.find((node) => node.semanticId === id || node.focused === true) ?? null;
}

function safeNode(node: JsonObject | null) {
  if (!node) return null;
  const content = String(node.text ?? node.value ?? "");
  let hash = 2166136261;
  for (const char of content) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return {
    semanticId: node.semanticId ?? null,
    role: node.role ?? node.type ?? null,
    type: node.type ?? null,
    selected: node.selected ?? null,
    focused: node.focused ?? null,
    index: node.index ?? null,
    content: {
      contentKind: "UserContent",
      redacted: true,
      length: content.length,
      fingerprint: (hash >>> 0).toString(16).padStart(8, "0"),
    },
  };
}

function asObject(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value) ? value as JsonObject : {};
}

function footerActionSelector(action: unknown) {
  if (typeof action !== "string" || action.length === 0) return null;
  return `${action}FooterAction:`;
}

/** Join canonical footer descriptors to AppKit's own accessibility projection. */
export function semanticAxParity(state: JsonObject, layout: JsonObject) {
  const activeFooter = asObject(state.activeFooter);
  const semanticButtons = asArray(activeFooter.buttons);
  const info = asObject(layout.info ?? layout);
  const fidelity = asObject(info.fidelity);
  const appkit = asObject(fidelity.appKit ?? fidelity.appkit);
  const axNodes = asArray(appkit.nodes);
  const peers = semanticButtons.map((button) => {
    const semanticId = typeof button.id === "string" ? button.id : null;
    const peer = semanticId == null
      ? null
      : axNodes.find((node) => node.accessibilityIdentifier === semanticId) ?? null;
    const label = typeof button.label === "string" ? button.label : "";
    const labelSha256 = createHash("sha256").update(label).digest("hex");
    const expectedSelector = footerActionSelector(button.action);
    const enabled = button.enabled === true;
    const disabledReason = button.actionDisabled ?? null;
    const errors = [
      peer == null ? "missingAxPeer" : "",
      peer && peer.accessibilityRole !== "AXButton" ? "wrongAxRole" : "",
      peer && peer.accessibilityLabelSha256 !== labelSha256 ? "labelMismatch" : "",
      peer && peer.accessibilityLabelLength !== Array.from(label).length ? "labelLengthMismatch" : "",
      peer && peer.accessibilityEnabled !== enabled ? "enabledMismatch" : "",
      peer && peer.actionSelector !== expectedSelector ? "actionMismatch" : "",
      peer && peer.accessibilityElement !== true ? "notAccessibilityElement" : "",
      peer && (peer.hidden === true || Number(peer.alpha ?? 1) <= 0) ? "hiddenAxPeer" : "",
      !enabled && disabledReason == null ? "missingDisabledReason" : "",
    ].filter(Boolean);
    return {
      semanticId,
      action: button.action ?? null,
      enabled,
      disabledReason,
      expectedSelector,
      axPeer: peer == null ? null : {
        structuralId: peer.id ?? null,
        accessibilityIdentifier: peer.accessibilityIdentifier ?? null,
        role: peer.accessibilityRole ?? null,
        labelSha256: peer.accessibilityLabelSha256 ?? null,
        labelLength: peer.accessibilityLabelLength ?? null,
        enabled: peer.accessibilityEnabled ?? null,
        focused: peer.accessibilityFocused ?? null,
        accessibilityElement: peer.accessibilityElement ?? null,
        hidden: peer.hidden ?? null,
        alpha: peer.alpha ?? null,
        actionSelector: peer.actionSelector ?? null,
        bounds: peer.screenshotFrame ?? null,
      },
      errors,
      parityPass: errors.length === 0,
    };
  });
  const duplicateAxIds = axNodes
    .map((node) => String(node.accessibilityIdentifier ?? ""))
    .filter(Boolean)
    .filter((id, index, ids) => ids.indexOf(id) !== index);
  return {
    semanticButtonCount: semanticButtons.length,
    axNodeCount: axNodes.length,
    peerCount: peers.filter((peer) => peer.axPeer != null).length,
    duplicateAxIds,
    peers,
    complete: semanticButtons.length > 0 &&
      peers.every((peer) => peer.parityPass) &&
      duplicateAxIds.length === 0,
  };
}

export function semanticFocusGraph(nodes: JsonObject[]) {
  const candidates = nodes.filter((node) =>
    node.selectable !== false &&
    (node.type === "button" || node.type === "input" || node.focused === true)
  );
  const hiddenFocusableIds = candidates
    .filter((node) => node.hidden === true || node.visible === false)
    .map((node) => String(node.semanticId ?? ""))
    .filter(Boolean);
  const focusable = candidates.filter((node) =>
    node.hidden !== true && node.visible !== false && typeof node.semanticId === "string" && node.semanticId.length > 0
  );
  const ids = focusable.map((node) => String(node.semanticId));
  const duplicateSemanticIds = ids.filter((id, index) => ids.indexOf(id) !== index);
  const edges = focusable.map((node, index) => ({
    semanticId: String(node.semanticId),
    previous: index > 0 ? String(focusable[index - 1].semanticId) : null,
    next: index + 1 < focusable.length ? String(focusable[index + 1].semanticId) : null,
  }));
  const reciprocal = duplicateSemanticIds.length === 0 && hiddenFocusableIds.length === 0 && edges.every((edge) => {
    const previous = edge.previous == null ? null : edges.find((candidate) => candidate.semanticId === edge.previous);
    const next = edge.next == null ? null : edges.find((candidate) => candidate.semanticId === edge.next);
    return (previous == null || previous.next === edge.semanticId) &&
      (next == null || next.previous === edge.semanticId);
  });
  return {
    nodes: edges,
    reciprocal,
    duplicateSemanticIds,
    hiddenFocusableIds,
    focusedSemanticIds: nodes.filter((node) => node.focused === true).map((node) => node.semanticId),
  };
}

export function nativeFooterActivationProof(
  result: JsonObject,
  expectedSemanticId: string,
  postconditionObserved: boolean,
  expectDisabledRefusal = false,
) {
  const activation = asObject(result.nativeFooterActivation);
  const errors = [
    result.host !== "NativeFooter" ? "wrongHost" : "",
    result.actionId !== expectedSemanticId ? "wrongSemanticId" : "",
    activation.semanticId !== expectedSemanticId ? "wrongNativePeer" : "",
    activation.accessibilityRole !== "AXButton" ? "wrongAxRole" : "",
    activation.actionSelector !== activation.expectedActionSelector ? "wrongAction" : "",
    expectDisabledRefusal
      ? (activation.refusedDisabled !== true || activation.dispatched !== false || result.errorCode !== "action_disabled"
        ? "disabledActivationNotRefused"
        : "")
      : (activation.descriptorEnabled !== true || activation.appkitEnabled !== true ||
          activation.dispatched !== true || result.ok !== true
        ? "enabledActivationNotDispatched"
        : ""),
    !postconditionObserved ? (expectDisabledRefusal ? "disabledPostconditionChanged" : "missingPostcondition") : "",
  ].filter(Boolean);
  return {
    expectedSemanticId,
    expectedDisposition: expectDisabledRefusal ? "refused-disabled" : "dispatched",
    activation,
    postconditionObserved,
    errors,
    complete: errors.length === 0,
  };
}

function nativeFooterSnapshot(state: JsonObject, axParity: ReturnType<typeof semanticAxParity>) {
  const activeFooter = (state.activeFooter as JsonObject | undefined) ?? {};
  return {
    owner: activeFooter.owner ?? null,
    activeSurface: activeFooter.activeSurface ?? null,
    expectedSurface: activeFooter.expectedSurface ?? null,
    nativeFooterHostInstalled: activeFooter.nativeFooterHostInstalled ?? null,
    buttonCount: activeFooter.buttonCount ?? null,
    axParity,
    activationPrimitiveAvailable: true,
    activationStatus: "not-requested",
    activationReceipt: null,
  };
}

function classify(
  targetReceipt: JsonObject,
  stateEnvelope: JsonObject,
  elementsEnvelope: JsonObject,
  layoutEnvelope: JsonObject,
  elements: JsonObject,
  focused: JsonObject | null,
  proofMode: "focus" | "ax",
  axParity: ReturnType<typeof semanticAxParity>,
) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopes(
    proofMode === "ax" ? [stateEnvelope, elementsEnvelope, layoutEnvelope] : [stateEnvelope, elementsEnvelope],
  );
  if (transport !== "ok") {
    return transport;
  }
  if (elements.projectionQuality !== "complete") {
    return "blocked-by-unsupported-projection";
  }
  if (!focused) {
    return "blocked-by-missing-primitive";
  }
  if (proofMode === "ax" && !axParity.complete) {
    return "blocked-by-missing-primitive";
  }
  return "ok";
}

async function main() {
  const argv = Bun.argv.slice(2);
  if (argv[0] !== "inspect") {
    if (argv.includes("--help") || argv.includes("-h")) {
      console.log(usage());
      process.exit(0);
    }
    console.error(usage());
    process.exit(2);
  }
  const { args, extras, warnings } = parseTargetArgs(argv.slice(1), {
    extras: { "--limit": "number", "--proof-mode": "string" },
  });
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  const limit = extras["--limit"] ?? 100;
  const proofMode: "focus" | "ax" = extras["--proof-mode"] === "ax" ? "ax" : "focus";

  const clock = startClock();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "focus" });
  const selector = (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ?? args.target ?? { type: "focused" };
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: requestId("focus", "state"),
    target: selector,
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
  const elementsEnvelope = await rpc(args.session, {
    type: "getElements",
    requestId: requestId("focus", "elements"),
    target: selector,
    limit,
  }, "elementsResult", args.timeoutMs);
  const layoutEnvelope = proofMode === "ax"
    ? await rpc(args.session, {
        type: "getLayoutInfo",
        requestId: requestId("focus", "layout"),
        target: selector,
      }, "layoutInfoResult", args.timeoutMs)
    : { status: "skipped" };
  const state = responseOf(stateEnvelope);
  const elements = responseOf(elementsEnvelope);
  const layout = responseOf(layoutEnvelope);
  const nodes = asArray(elements.elements);
  const focusedSemanticId = elements.focusedSemanticId ?? null;
  const selectedSemanticId = elements.selectedSemanticId ?? null;
  const focused = focusedNode(nodes, focusedSemanticId);
  const axParity = semanticAxParity(state, layout);
  const focusGraph = semanticFocusGraph(nodes);
  const classification = classify(
    targetReceipt,
    stateEnvelope,
    elementsEnvelope,
    layoutEnvelope,
    elements,
    focused,
    proofMode,
    axParity,
  );
  const nativeFooter = nativeFooterSnapshot(state, axParity);

  emitValidatedReceipt("devtools.focus.inspect", finishReceipt(
    { tool: "script-kit-devtools.focus", command: "focus.inspect", session: args.session, clock },
    {
      classification,
      proofMode,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      transaction: targetReceipt.transaction,
      windowFocused: state.isFocused ?? null,
      windowVisible: state.windowVisible ?? null,
      focusedSemanticId,
      selectedSemanticId,
      focusedNode: safeNode(focused),
      selectedNode: safeNode(nodes.find((node) => node.semanticId === selectedSemanticId) ?? null),
      activeFooter: state.activeFooter ?? null,
      nativeFooter,
      focusGraph,
      semanticProjection: {
        semanticSurface: elements.semanticSurface ?? null,
        version: elements.projectionVersion ?? null,
        quality: elements.projectionQuality ?? null,
        reasonCodes: elements.reasonCodes ?? [],
        proofMode,
        proofAllowed: elements.projectionQuality === "complete" &&
          (proofMode !== "ax" || axParity.complete),
      },
      submitDiagnostics: diagnostic(state.submitDiagnostics ?? null),
      receipts: {
        target: { classification: targetReceipt.classification ?? null },
        state: { status: stateEnvelope.status ?? "ok" },
        elements: { status: elementsEnvelope.status ?? "ok" },
        layout: { status: layoutEnvelope.status ?? "skipped" },
      },
      keyboardOwner: {
        inputLength: typeof state.inputValue === "string" ? state.inputValue.length : 0,
        inputFingerprint: typeof state.inputValue === "string"
          ? safeNode({ value: state.inputValue })?.content
          : null,
        promptType: state.promptType ?? null,
        surfaceKind: (state.surfaceContract as JsonObject | undefined)?.surfaceKind ?? null,
        inputOwnership: (state.surfaceContract as JsonObject | undefined)?.inputOwnership ?? null,
        keyboardPolicy: (state.surfaceContract as JsonObject | undefined)?.keyboardPolicy ?? null,
      },
      missingPrimitives: [
        !focused ? "focusedSemanticId" : "",
        elements.projectionQuality === "complete" ? "" : "completeSemanticProjection",
        stateEnvelope.status === "error" ? "stateResult" : "",
        elementsEnvelope.status === "error" ? "elementsResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
        proofMode === "ax" && !axParity.complete ? "semanticAxParity" : "",
        proofMode === "ax" && !focusGraph.reciprocal ? "reciprocalFocusGraph" : "",
        proofMode === "ax" && layoutEnvelope.status === "error" ? "layoutInfoResult" : "",
      ].filter(Boolean),
      warnings: [
        ...warnings,
        state.isFocused === false ? "window is visible but not focused" : "",
        focusedSemanticId == null ? "focused semantic id missing" : "",
      ].filter(Boolean),
      errors: diagnostic([
        ...((targetReceipt.errors as JsonObject[]) ?? []),
        ...[stateEnvelope, elementsEnvelope].filter((value) => value.status === "error"),
      ]),
    },
  ));
}

if (import.meta.main) await main();
