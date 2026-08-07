#!/usr/bin/env bun
/** Keyboard policy / bindings inspection. Shared transport/args/receipts live in lib/client.ts. */

import {
  type JsonObject,
  asArray,
  classifyEnvelopeError,
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
  return "Usage:\n  bun scripts/devtools/keyboard.ts inspect [target args]";
}

function normalizeKey(value: unknown) {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function duplicateKeys(bindings: JsonObject[]) {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const binding of bindings) {
    const key = normalizeKey(binding.key);
    if (!key) continue;
    if (seen.has(key)) {
      duplicates.add(key);
    }
    seen.add(key);
  }
  return [...duplicates];
}

function nativeFooterSnapshot(activeFooter: JsonObject, bindings: JsonObject[]) {
  return {
    hostInstalled: activeFooter.nativeFooterHostInstalled ?? null,
    owner: activeFooter.owner ?? null,
    activeSurface: activeFooter.activeSurface ?? null,
    expectedSurface: activeFooter.expectedSurface ?? null,
    bindingsAvailable: bindings.length > 0,
    activationPrimitiveAvailable: false,
    missingPrimitive: "nativeFooterActivationReceipt",
    nextSafeCommand: "bun scripts/devtools/focus.ts inspect --target-id <id> --strict",
  };
}

function classify(targetReceipt: JsonObject, stateEnvelope: JsonObject, bindings: JsonObject[]) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopeError(stateEnvelope);
  if (transport !== "ok") {
    return transport;
  }
  if (bindings.length === 0) {
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
  const { args, warnings: argWarnings } = parseTargetArgs(argv.slice(1));
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }

  const clock = startClock();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "keyboard" });
  const selector = (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ?? args.target ?? { type: "focused" };
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: requestId("keyboard", "state"),
    target: selector,
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
  const state = responseOf(stateEnvelope);
  const surfaceContract = (state.surfaceContract as JsonObject | undefined) ?? {};
  const activeFooter = (state.activeFooter as JsonObject | undefined) ?? {};
  const footerBindings = asArray(activeFooter.buttons).map((button) => ({
    source: "activeFooter",
    action: button.action ?? null,
    key: button.key ?? null,
    label: button.label ?? null,
    enabled: button.enabled ?? null,
    selected: button.selected ?? null,
    disabledReason: button.actionDisabled ?? null,
  }));
  const activePopup = (state.activePopupContract as JsonObject | undefined) ?? null;
  const actionsDialog = (state.actionsDialog as JsonObject | undefined) ?? null;
  const actionsDialogShortcutParity = (actionsDialog?.shortcutParity as JsonObject | undefined)
    ?? ((actionsDialog?.actions as JsonObject | undefined)?.shortcutParity as JsonObject | undefined)
    ?? null;
  const popupActions = asArray(actionsDialog?.visibleActions).map((action) => ({
    source: "actionsDialog",
    action: action.id ?? null,
    key: action.shortcut ?? null,
    canonicalKey: action.canonicalShortcut ?? null,
    label: action.label ?? null,
    section: action.section ?? null,
    enabled: action.enabled ?? null,
    disabledReason: action.actionDisabled ?? null,
  }));
  const bindings = [...footerBindings, ...popupActions];
  const nativeFooter = nativeFooterSnapshot(activeFooter, bindings);
  const duplicates = duplicateKeys(bindings);
  const classification = classify(targetReceipt, stateEnvelope, bindings);

  emitValidatedReceipt("devtools.keyboard.inspect", finishReceipt(
    { tool: "script-kit-devtools.keyboard", command: "keyboard.inspect", session: args.session, clock },
    {
      classification,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      keyboardPolicy: surfaceContract.keyboardPolicy ?? null,
      inputOwnership: surfaceContract.inputOwnership ?? null,
      focusPolicy: surfaceContract.focusPolicy ?? null,
      activeFooter: {
        owner: activeFooter.owner ?? null,
        activeSurface: activeFooter.activeSurface ?? null,
        expectedSurface: activeFooter.expectedSurface ?? null,
        nativeFooterHostInstalled: activeFooter.nativeFooterHostInstalled ?? null,
        buttonCount: activeFooter.buttonCount ?? footerBindings.length,
        actionSlotCount: activeFooter.actionSlotCount ?? null,
        contextChipCount: activeFooter.contextChipCount ?? null,
        duplicateShortcutKeys: activeFooter.duplicateShortcutKeys ?? [],
        slotContractViolation: activeFooter.slotContractViolation ?? null,
      },
      nativeFooter,
      activePopup,
      actionsDialogShortcutParity,
      bindings,
      duplicateKeys: duplicates,
      routingPriorityResolved: duplicates.length === 0 || activePopup != null,
      routingPriority: activePopup ? "activePopupFirst" : null,
      activationProof: {
        proven: false,
        nextSafeCommand: "bun scripts/devtools/act.ts key --target-id <id> --strict",
      },
      missingPrimitives: [
        bindings.length === 0 ? "keyboardBindings" : "",
        stateEnvelope.status === "error" ? "stateResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
        nativeFooter.hostInstalled === true ? "nativeFooterActivationReceipt" : "",
      ].filter(Boolean),
      warnings: [
        ...argWarnings,
        duplicates.length > 0 ? `duplicate shortcut keys: ${duplicates.join(", ")}` : "",
        activePopup ? "popup contract is active; popup-first keyboard routing should be verified with devtools.act" : "",
      ].filter(Boolean),
      errors: diagnostic([
        ...((targetReceipt.errors as JsonObject[]) ?? []),
        ...[stateEnvelope].filter((value) => value.status === "error"),
      ]),
    },
  ));
}

await main();
