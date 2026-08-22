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
    "  bun scripts/devtools/elements.ts snapshot [target args] [--limit <n>] [--proof-mode inspection|action|focus|ax]",
    "",
    "Target args match scripts/devtools/targets.ts inspect, e.g. --session <name> --main --strict --surface ScriptList --start --show.",
  ].join("\n");
}

function nodeLabel(node: JsonObject) {
  return node.text ?? node.value ?? null;
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

function inferredAction(node: JsonObject): string | null {
  if (typeof node.action === "string") return node.action;
  if (node.type === "button") return "activate";
  if (node.type === "choice" && node.selectable !== false) return "select";
  if (node.type === "input") return "edit";
  return null;
}

function productionContentMeasurement(node: JsonObject) {
  const descriptors = node.content && typeof node.content === "object" &&
      !Array.isArray(node.content)
    ? node.content as JsonObject
    : null;
  const candidate = descriptors?.text ?? descriptors?.value;
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
    return {
      measurement: contentMeasurement(nodeLabel(node)),
      privacyViolation: descriptors !== null,
    };
  }
  const content = candidate as JsonObject;
  const valid =
    typeof content.contentKind === "string" &&
    Number.isSafeInteger(content.charLength) && Number(content.charLength) >= 0 &&
    Number.isSafeInteger(content.byteLength) && Number(content.byteLength) >= 0 &&
    typeof content.fingerprint === "string" &&
    /^sha256:[a-f0-9]{64}$/.test(content.fingerprint) &&
    content.rawContentReturned === false &&
    node.text == null && node.value == null;
  return {
    measurement: valid
      ? {
        contentKind: content.contentKind,
        redacted: true,
        length: content.charLength,
        byteLength: content.byteLength,
        lineCount: null,
        fingerprint: content.fingerprint,
        rawContentReturned: false,
        source: "production-protocol-redaction",
      }
      : contentMeasurement(null),
    privacyViolation: !valid,
  };
}

export function snapshot(nodes: JsonObject[]) {
  const ids = nodes.map((node) => String(node.semanticId ?? "")).filter(Boolean);
  const seen = new Set<string>();
  const duplicateSemanticIds = ids.filter((id) => {
    if (seen.has(id)) {
      return true;
    }
    seen.add(id);
    return false;
  });

  const privacyViolationSemanticIds: string[] = [];
  const projectedNodes = nodes.map((node) => {
      const action = inferredAction(node);
      const disabledReason = node.actionDisabled ?? null;
      const explicitlyDisabled = node.enabled === false ||
        ((node.type === "choice" || node.type === "button") && node.selectable === false);
      const enabled = !explicitlyDisabled && disabledReason == null;
      const selectable = node.selectable ?? node.type === "choice";
      const focusable = typeof node.focusable === "boolean"
        ? node.focusable
        : node.focused === true || node.type === "input" ||
          (enabled && selectable !== false && action != null);
      const activatable = enabled && action != null && node.type !== "input" &&
        selectable !== false;
      const semanticId = node.semanticId ?? null;
      const content = productionContentMeasurement(node);
      if (content.privacyViolation) {
        privacyViolationSemanticIds.push(String(semanticId ?? "(missing-semantic-id)"));
      }
      return {
        semanticId,
        measurementId: semanticId == null ? null : `semantic:${String(semanticId)}`,
        role: node.role ?? node.type ?? null,
        type: node.type ?? null,
        content: content.measurement,
        selected: node.selected ?? null,
        focused: node.focused ?? null,
        index: node.index ?? null,
        owner: node.source ?? null,
        action,
        enabled,
        disabledReason,
        focusable,
        selectable,
        activatable,
        style: node.style ?? null,
        bounds: node.bounds ?? null,
      };
    });

  return {
    nodes: projectedNodes,
    duplicateSemanticIds: [...new Set(duplicateSemanticIds)],
    missingSemanticIdCount: nodes.length - ids.length,
    missingBoundsCount: nodes.filter((node) => node.bounds == null).length,
    focusedSemanticIds: projectedNodes
      .filter((node) => node.focused === true)
      .map((node) => String(node.semanticId ?? ""))
      .filter(Boolean),
    privacyViolationSemanticIds,
  };
}

export type ProjectionProofMode = "inspection" | "action" | "focus" | "ax";

export function semanticProjection(elements: JsonObject, proofMode: ProjectionProofMode) {
  const quality = String(elements.projectionQuality ?? "unsupported");
  const reasonCodes = Array.isArray(elements.reasonCodes)
    ? elements.reasonCodes.map(String)
    : elements.projectionQuality == null
      ? ["collectorUnavailable"]
      : [];
  const complete = quality === "complete";
  const accessibility = elements.accessibilityProjection &&
      typeof elements.accessibilityProjection === "object" &&
      !Array.isArray(elements.accessibilityProjection)
    ? elements.accessibilityProjection as JsonObject
    : null;
  const nativeAccessibilityObserved =
    accessibility?.source === "native-appkit-accessibility" &&
    accessibility.complete === true &&
    Number.isSafeInteger(accessibility.peerCount) &&
    Number(accessibility.peerCount) > 0;
  return {
    semanticSurface: elements.semanticSurface ?? null,
    version: elements.projectionVersion ?? null,
    quality,
    reasonCodes,
    proofMode,
    complete,
    proofAllowed: complete &&
      (proofMode !== "ax" || nativeAccessibilityObserved),
    nativeAccessibilityObserved,
    limitationsExplicit: complete || reasonCodes.length > 0,
  };
}

export function classify(
  targetReceipt: JsonObject,
  elementsEnvelope: JsonObject,
  elementSnapshot: ReturnType<typeof snapshot>,
  projection: ReturnType<typeof semanticProjection>,
) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopeError(elementsEnvelope);
  if (transport !== "ok") {
    return transport;
  }
  if (elementSnapshot.duplicateSemanticIds.length > 0) {
    return "invalid-identity";
  }
  if (elementSnapshot.privacyViolationSemanticIds.length > 0) {
    return "invalid-privacy";
  }
  if (elementSnapshot.focusedSemanticIds.length > 1) {
    return "invalid-identity";
  }
  if (elementSnapshot.missingSemanticIdCount > 0) {
    return "blocked-by-missing-primitive";
  }
  if (!projection.complete || !projection.limitationsExplicit) {
    return "blocked-by-unsupported-projection";
  }
  if (projection.proofMode === "action" &&
      !elementSnapshot.nodes.some((node) => node.activatable)) {
    return "blocked-by-missing-primitive";
  }
  if (projection.proofMode === "focus" &&
      elementSnapshot.focusedSemanticIds.length !== 1) {
    return "blocked-by-missing-primitive";
  }
  if (projection.proofMode === "ax" && !projection.nativeAccessibilityObserved) {
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
  const { args, extras, warnings } = parseTargetArgs(argv.slice(1), {
    extras: { "--limit": "number", "--proof-mode": "string" },
  });
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  const limit = extras["--limit"] ?? 200;
  const requestedProofMode = String(extras["--proof-mode"] ?? "inspection");
  if (!["inspection", "action", "focus", "ax"].includes(requestedProofMode)) {
    console.error(usage());
    process.exit(64);
  }
  const proofMode = requestedProofMode as ProjectionProofMode;

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
  const projection = semanticProjection(elements, proofMode);
  const classification = classify(targetReceipt, elementsEnvelope, elementSnapshot, projection);

  emitValidatedReceipt("devtools.elements.snapshot", finishReceipt(
    { tool: "script-kit-devtools.elements", command: "elements.snapshot", session: args.session, clock },
    {
      classification,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      transaction: targetReceipt.transaction,
      semanticSurface: {
        surfaceKind: (targetReceipt.resolvedTarget as JsonObject | undefined)?.surfaceKind ?? null,
        appViewVariant: (targetReceipt.resolvedTarget as JsonObject | undefined)?.appViewVariant ?? null,
        collectorSurface: projection.semanticSurface,
      },
      semanticProjection: projection,
      totalCount: elements.totalCount ?? nodes.length,
      returnedCount: nodes.length,
      truncated: elements.truncated ?? false,
      focusedSemanticId: elements.focusedSemanticId ?? null,
      selectedSemanticId: elements.selectedSemanticId ?? null,
      duplicateSemanticIds: elementSnapshot.duplicateSemanticIds,
      privacyViolationSemanticIds: elementSnapshot.privacyViolationSemanticIds,
      missingPrimitives: [
        elementSnapshot.missingSemanticIdCount > 0 ? "semanticId" : "",
        elementSnapshot.missingBoundsCount > 0 ? "elementBounds" : "",
        projection.complete ? "" : "completeSemanticProjection",
        projection.limitationsExplicit ? "" : "projectionReasonCodes",
        proofMode === "action" && !elementSnapshot.nodes.some((node) => node.activatable)
          ? "enabledSemanticAction"
          : "",
        proofMode === "focus" && elementSnapshot.focusedSemanticIds.length !== 1
          ? "uniqueFocusedSemanticNode"
          : "",
        proofMode === "ax" && !projection.nativeAccessibilityObserved
          ? "nativeAccessibilityProjection"
          : "",
        elementsEnvelope.status === "error" ? "elementsResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
      ].filter(Boolean),
      nodes: elementSnapshot.nodes,
      warnings: [
        ...warnings,
        ...(Array.isArray(elements.warnings) ? elements.warnings : []),
        elementSnapshot.missingBoundsCount > 0 ? "getElements does not expose bounds yet; use devtools.layout.measure for geometry." : "",
        projection.complete ? "" : `semantic projection is ${projection.quality}: ${projection.reasonCodes.join(",")}`,
      ].filter(Boolean),
      errors: diagnostic(
        [...((targetReceipt.errors as JsonObject[]) ?? []), elementsEnvelope].filter(
          (value) => value.status === "error",
        ),
      ),
    },
  ));
}

if (import.meta.main) await main();
