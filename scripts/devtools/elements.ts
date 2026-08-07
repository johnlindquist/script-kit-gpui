#!/usr/bin/env bun
/** Target-scoped semantic element snapshot. Shared transport/args/receipts live in lib/client.ts. */

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
  return [
    "Usage:",
    "  bun scripts/devtools/elements.ts snapshot [target args] [--limit <n>]",
    "",
    "Target args match scripts/devtools/targets.ts inspect, e.g. --session <name> --main --strict --surface ScriptList --start --show.",
  ].join("\n");
}

function nodeLabel(node: JsonObject) {
  return node.text ?? node.value ?? node.semanticId ?? null;
}

function contentMeasurement(value: unknown) {
  const text = typeof value === "string" ? value : value == null ? "" : String(value);
  let hash = 2166136261;
  for (const char of text) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }
  return {
    contentKind: "UserContent",
    redacted: true,
    length: text.length,
    lineCount: text.length ? text.split(/\r\n|\r|\n/).length : 0,
    fingerprint: (hash >>> 0).toString(16).padStart(8, "0"),
  };
}

function snapshot(nodes: JsonObject[]) {
  const ids = nodes.map((node) => String(node.semanticId ?? "")).filter(Boolean);
  const seen = new Set<string>();
  const duplicateSemanticIds = ids.filter((id) => {
    if (seen.has(id)) {
      return true;
    }
    seen.add(id);
    return false;
  });

  return {
    nodes: nodes.map((node) => ({
      semanticId: node.semanticId ?? null,
      role: node.role ?? node.type ?? null,
      type: node.type ?? null,
      content: contentMeasurement(nodeLabel(node)),
      selected: node.selected ?? null,
      focused: node.focused ?? null,
      index: node.index ?? null,
      owner: node.source ?? null,
      style: node.style ?? null,
      actions: [],
      bounds: node.bounds ?? null,
    })),
    duplicateSemanticIds: [...new Set(duplicateSemanticIds)],
    missingSemanticIdCount: nodes.length - ids.length,
    missingBoundsCount: nodes.filter((node) => node.bounds == null).length,
  };
}

function classify(targetReceipt: JsonObject, elementsEnvelope: JsonObject, elementSnapshot: ReturnType<typeof snapshot>) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopeError(elementsEnvelope);
  if (transport !== "ok") {
    return transport;
  }
  if (elementSnapshot.missingSemanticIdCount > 0 || elementSnapshot.duplicateSemanticIds.length > 0) {
    return "blocked-by-missing-primitive";
  }
  return "ok";
}

async function main() {
  const argv = Bun.argv.slice(2);
  if (argv[0] !== "snapshot") {
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
  const limit = extras["--limit"] ?? 200;

  const clock = startClock();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "elements" });
  const selector = (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ?? args.target ?? { type: "focused" };
  const elementsEnvelope = await rpc(args.session, {
    type: "getElements",
    requestId: requestId("elements", "snapshot"),
    target: selector,
    limit,
  }, "elementsResult", args.timeoutMs);
  const elements = responseOf(elementsEnvelope);
  const nodes = asArray(elements.elements);
  const elementSnapshot = snapshot(nodes);
  const classification = classify(targetReceipt, elementsEnvelope, elementSnapshot);

  emitValidatedReceipt("devtools.elements.snapshot", finishReceipt(
    { tool: "script-kit-devtools.elements", command: "elements.snapshot", session: args.session, clock },
    {
      classification,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      semanticSurface: {
        surfaceKind: (targetReceipt.resolvedTarget as JsonObject | undefined)?.surfaceKind ?? null,
        appViewVariant: (targetReceipt.resolvedTarget as JsonObject | undefined)?.appViewVariant ?? null,
      },
      totalCount: elements.totalCount ?? nodes.length,
      returnedCount: nodes.length,
      truncated: elements.truncated ?? false,
      focusedSemanticId: elements.focusedSemanticId ?? null,
      selectedSemanticId: elements.selectedSemanticId ?? null,
      duplicateSemanticIds: elementSnapshot.duplicateSemanticIds,
      missingPrimitives: [
        elementSnapshot.missingSemanticIdCount > 0 ? "semanticId" : "",
        elementSnapshot.missingBoundsCount > 0 ? "elementBounds" : "",
        elementsEnvelope.status === "error" ? "elementsResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
      ].filter(Boolean),
      nodes: elementSnapshot.nodes,
      warnings: [
        ...warnings,
        ...(Array.isArray(elements.warnings) ? elements.warnings : []),
        elementSnapshot.missingBoundsCount > 0 ? "getElements does not expose bounds yet; use devtools.layout.measure for geometry." : "",
      ].filter(Boolean),
      errors: diagnostic(
        [...((targetReceipt.errors as JsonObject[]) ?? []), elementsEnvelope].filter(
          (value) => value.status === "error",
        ),
      ),
    },
  ));
}

await main();
