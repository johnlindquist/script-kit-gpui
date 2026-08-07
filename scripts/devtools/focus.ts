#!/usr/bin/env bun
/** Focus/keyboard-ownership inspection. Shared transport/args/receipts live in lib/client.ts. */

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
  return "Usage:\n  bun scripts/devtools/focus.ts inspect [target args] [--limit <n>]";
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

function nativeFooterSnapshot(state: JsonObject) {
  const activeFooter = (state.activeFooter as JsonObject | undefined) ?? {};
  return {
    owner: activeFooter.owner ?? null,
    activeSurface: activeFooter.activeSurface ?? null,
    expectedSurface: activeFooter.expectedSurface ?? null,
    nativeFooterHostInstalled: activeFooter.nativeFooterHostInstalled ?? null,
    buttonCount: activeFooter.buttonCount ?? null,
    activationPrimitiveAvailable: false,
    missingPrimitive: "nativeFooterActivationReceipt",
  };
}

function classify(targetReceipt: JsonObject, stateEnvelope: JsonObject, elementsEnvelope: JsonObject, focused: JsonObject | null) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopes([stateEnvelope, elementsEnvelope]);
  if (transport !== "ok") {
    return transport;
  }
  if (!focused) {
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
  const { args, extras, warnings } = parseTargetArgs(argv.slice(1), { extras: { "--limit": "number" } });
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  const limit = extras["--limit"] ?? 100;

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
  const state = responseOf(stateEnvelope);
  const elements = responseOf(elementsEnvelope);
  const nodes = asArray(elements.elements);
  const focusedSemanticId = elements.focusedSemanticId ?? null;
  const selectedSemanticId = elements.selectedSemanticId ?? null;
  const focused = focusedNode(nodes, focusedSemanticId);
  const classification = classify(targetReceipt, stateEnvelope, elementsEnvelope, focused);
  const nativeFooter = nativeFooterSnapshot(state);

  emitValidatedReceipt("devtools.focus.inspect", finishReceipt(
    { tool: "script-kit-devtools.focus", command: "focus.inspect", session: args.session, clock },
    {
      classification,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      windowFocused: state.isFocused ?? null,
      windowVisible: state.windowVisible ?? null,
      focusedSemanticId,
      selectedSemanticId,
      focusedNode: safeNode(focused),
      selectedNode: safeNode(nodes.find((node) => node.semanticId === selectedSemanticId) ?? null),
      activeFooter: state.activeFooter ?? null,
      nativeFooter,
      submitDiagnostics: diagnostic(state.submitDiagnostics ?? null),
      receipts: {
        target: { classification: targetReceipt.classification ?? null },
        state: { status: stateEnvelope.status ?? "ok" },
        elements: { status: elementsEnvelope.status ?? "ok" },
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
        stateEnvelope.status === "error" ? "stateResult" : "",
        elementsEnvelope.status === "error" ? "elementsResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
        nativeFooter.nativeFooterHostInstalled === true ? "nativeFooterActivationReceipt" : "",
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

await main();
