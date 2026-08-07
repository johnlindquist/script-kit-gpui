#!/usr/bin/env bun
/** Surface contract inspection. Shared transport/args/receipts live in lib/client.ts. */

import {
  type JsonObject,
  classifyEnvelopeError,
  finishReceipt,
  parseTargetArgs,
  requestId,
  responseOf,
  rpc,
  startClock,
} from "./lib/client.ts";
import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic, userContent } from "./lib/privacy.ts";
import { maybeStartAndShow, resolveTargetReceipt } from "./lib/target-identity.ts";

type SurfaceContract = {
  surfaceKind: string;
  appViewVariants: string[];
  appViewFooters: Array<{ variant: string; nativeFooterSurface: string | null }>;
  vocabulary?: {
    family?: string;
    inputOwnership?: string;
    previewRole?: string;
  };
  focusPolicy?: string;
  keyboardPolicy?: string;
  actionsPolicy?: string;
  proofPolicy?: string;
  visualPolicy?: string;
  dismissPolicy?: {
    policy: string;
    windowBlur: string;
    backdropClick: string;
    escape: string;
    cmdW: string;
  };
  automationSemanticSurface?: string;
};

const root = new URL("../..", import.meta.url);

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/surface.ts inspect --surface <SurfaceKind> [target args]",
    "",
    "Target args match scripts/devtools/targets.ts inspect, e.g. --session <name> --main --strict --start --show.",
  ].join("\n");
}

async function readContracts() {
  const payload = JSON.parse(await Bun.file(new URL("docs/ai/contracts/surface-contracts.json", root)).text()) as {
    generatedFrom: string;
    registry: string;
    entries: SurfaceContract[];
  };
  return payload;
}

function runtimeValue(snapshot: JsonObject, ...names: string[]) {
  for (const name of names) {
    const value = snapshot[name];
    if (value !== undefined) {
      return value;
    }
  }
  return null;
}

function runtimeSurface(targetReceipt: JsonObject) {
  const rawInspect = targetReceipt.rawInspect as JsonObject | undefined;
  const snapshot = (rawInspect?.snapshot as JsonObject | undefined) ?? rawInspect ?? {};
  return {
    activeFooterSurface: runtimeValue(snapshot, "activeFooterSurface", "nativeFooterSurface"),
    inputOwnerSemanticId: runtimeValue(snapshot, "inputOwnerSemanticId"),
    previewSemanticId: runtimeValue(snapshot, "previewSemanticId"),
    focusedSemanticId: runtimeValue(snapshot, "focusedSemanticId"),
    selectedSemanticId: runtimeValue(snapshot, "selectedSemanticId"),
    rowCountVisible: runtimeValue(snapshot, "rowCountVisible", "visibleRowCount"),
    rowCountTotal: runtimeValue(snapshot, "rowCountTotal", "totalRowCount"),
    filterText: runtimeValue(snapshot, "filterText", "query"),
    sourceFilter: runtimeValue(snapshot, "sourceFilter"),
    capabilities: [
      "targetIdentity",
      targetReceipt.classification === "ok" ? "strictTargetInspect" : "",
      snapshot.screenshotWidth || snapshot.screenshot_width ? "screenshotMetadata" : "",
    ].filter(Boolean),
    missingPrimitives: [
      runtimeValue(snapshot, "focusedSemanticId") == null ? "focusedSemanticId" : "",
      runtimeValue(snapshot, "selectedSemanticId") == null ? "selectedSemanticId" : "",
      runtimeValue(snapshot, "rowCountVisible", "visibleRowCount") == null ? "rowCountVisible" : "",
    ].filter(Boolean),
  };
}

function enrichedRuntimeSurface(targetReceipt: JsonObject, state: JsonObject) {
  const base = runtimeSurface(targetReceipt);
  const stateStatus = state.status === "error" ? "error" : "ok";
  const activeFooter = state.activeFooter as JsonObject | undefined;
  const surfaceContract = state.surfaceContract as JsonObject | undefined;
  const rowCountVisible = runtimeValue(state, "visibleChoiceCount") ?? base.rowCountVisible;
  const rowCountTotal = runtimeValue(state, "choiceCount") ?? base.rowCountTotal;
  const filterText = runtimeValue(state, "inputValue") ?? base.filterText;
  const activeFooterSurface = activeFooter?.activeSurface ?? activeFooter?.expectedSurface ?? base.activeFooterSurface;
  const missingPrimitives = [
    ...base.missingPrimitives.filter((missing) => missing !== "rowCountVisible"),
    rowCountVisible == null ? "rowCountVisible" : "",
    rowCountTotal == null ? "rowCountTotal" : "",
    stateStatus === "error" ? "stateResult" : "",
  ].filter(Boolean);

  return {
    ...base,
    activeFooterSurface,
    rowCountVisible,
    rowCountTotal,
    filterText,
    selectedIndex: runtimeValue(state, "selectedIndex"),
    selectedValue: runtimeValue(state, "selectedValue"),
    windowVisible: runtimeValue(state, "windowVisible"),
    isFocused: runtimeValue(state, "isFocused"),
    surfaceContract,
    activeFooter,
    capabilities: [
      ...base.capabilities,
      stateStatus === "ok" ? "stateResult" : "",
      surfaceContract ? "surfaceContract" : "",
      activeFooter ? "activeFooter" : "",
    ].filter(Boolean),
    missingPrimitives,
  };
}

function contractPayload(contracts: Awaited<ReturnType<typeof readContracts>>, surfaceKind: string) {
  const contract = contracts.entries.find((entry) => entry.surfaceKind === surfaceKind);
  if (!contract) {
    return null;
  }
  return {
    sourcePath: "docs/ai/contracts/surface-contracts.json",
    generatedFrom: contracts.generatedFrom,
    registry: contracts.registry,
    surfaceKind: contract.surfaceKind,
    appViewVariants: contract.appViewVariants,
    nativeFooterSurfaces: contract.appViewFooters
      .map((footer) => footer.nativeFooterSurface)
      .filter((footer): footer is string => Boolean(footer)),
    family: contract.vocabulary?.family ?? null,
    inputOwnership: contract.vocabulary?.inputOwnership ?? null,
    previewRole: contract.vocabulary?.previewRole ?? null,
    focusPolicy: contract.focusPolicy ?? null,
    keyboardPolicy: contract.keyboardPolicy ?? null,
    actionsPolicy: contract.actionsPolicy ?? null,
    proofPolicy: contract.proofPolicy ?? null,
    visualPolicy: contract.visualPolicy ?? null,
    dismissPolicy: contract.dismissPolicy ?? null,
    automationSemanticSurface: contract.automationSemanticSurface ?? null,
  };
}

function classify(
  targetReceipt: JsonObject,
  stateEnvelope: JsonObject,
  contract: ReturnType<typeof contractPayload>,
) {
  if (!contract) {
    return "blocked-by-unknown-surface";
  }
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopeError(stateEnvelope);
  if (transport !== "ok") {
    return transport;
  }
  if (!contract.dismissPolicy) {
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
  if (!args.expectedSurfaceKind) {
    console.error(usage());
    process.exit(2);
  }
  const surfaceKind = args.expectedSurfaceKind;

  const clock = startClock();
  const contracts = await readContracts();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "surface" });
  const stateEnvelope = await rpc(
    args.session,
    {
      type: "getState",
      requestId: requestId("surface", "state"),
      target: (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ?? args.target ?? { type: "main" },
      summaryOnly: true,
    },
    "stateResult",
    args.timeoutMs,
  );
  const state = responseOf(stateEnvelope);
  const contract = contractPayload(contracts, surfaceKind);
  const classification = classify(targetReceipt, stateEnvelope, contract);
  const runtime = enrichedRuntimeSurface(targetReceipt, state);

  emitValidatedReceipt("devtools.surface.inspect", finishReceipt(
    { tool: "script-kit-devtools.surface", command: "surface.inspect", session: args.session, clock },
    {
      classification,
      requestedSurfaceKind: surfaceKind,
      target: targetReceipt.resolvedTarget ?? null,
      requestedTarget: targetReceipt.requestedTarget ?? null,
      contract,
      runtime: {
        ...runtime,
        filterText: userContent(runtime.filterText),
        selectedValue: userContent(runtime.selectedValue),
      },
      missingPrimitives: [
        ...(contract ? [] : ["surfaceContract"]),
        ...runtime.missingPrimitives,
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
      ].filter(Boolean),
      warnings: [
        ...argWarnings,
        contract ? "" : `no contract entry for surfaceKind '${surfaceKind}' in docs/ai/contracts/surface-contracts.json`,
      ].filter(Boolean),
      errors: diagnostic([
        ...((targetReceipt.errors as JsonObject[]) ?? []),
        ...[stateEnvelope].filter((value) => value.status === "error"),
      ]),
    },
  ));
}

await main();
