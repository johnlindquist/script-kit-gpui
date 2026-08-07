#!/usr/bin/env bun
/** Privacy-safe text content and completed-frame glyph-fit measurement. */

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

type Rect = { x: number; y: number; width: number; height: number };
export type TextProofMode = "inspection" | "fit";

function usage() {
  return "Usage:\n  bun scripts/devtools/text.ts measure [target args] [--proof-mode inspection|fit] [--limit <n>]";
}

function asObject(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : {};
}

function asNumber(value: unknown, fallback = 0) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function rect(value: unknown): Rect {
  const source = asObject(value);
  return {
    x: asNumber(source.x),
    y: asNumber(source.y),
    width: asNumber(source.width),
    height: asNumber(source.height),
  };
}

function right(value: Rect) {
  return value.x + value.width;
}

function bottom(value: Rect) {
  return value.y + value.height;
}

function intersects(left: Rect, rightRect: Rect) {
  return left.x < right(rightRect) && right(left) > rightRect.x &&
    left.y < bottom(rightRect) && bottom(left) > rightRect.y;
}

function visibleRatio(bounds: Rect, clip: Rect) {
  const width = Math.max(0, Math.min(right(bounds), right(clip)) - Math.max(bounds.x, clip.x));
  const height = Math.max(0, Math.min(bottom(bounds), bottom(clip)) - Math.max(bounds.y, clip.y));
  const area = Math.max(0, bounds.width) * Math.max(0, bounds.height);
  return area > 0 ? (width * height) / area : 1;
}

function legacyFingerprint(value: string) {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

/** Convert semantic descriptors without reconstructing or returning authored bytes. */
export function textRows(nodes: JsonObject[]) {
  return nodes.flatMap((node) => {
    const content = asObject(node.content);
    if (Object.keys(content).length > 0) {
      const charLength = asNumber(content.charLength ?? content.textLength, 0);
      if (charLength <= 0) return [];
      return [{
        semanticId: node.semanticId ?? null,
        role: node.role ?? node.type ?? null,
        contentKind: content.kind ?? "userContent",
        rawContentReturned: content.rawContentReturned === true,
        redacted: true,
        textLength: charLength,
        byteLength: asNumber(content.byteLength, charLength),
        lineCount: asNumber(content.lineCount, 1),
        selected: node.selected ?? null,
        focused: node.focused ?? null,
        fingerprint: content.fingerprint ?? null,
      }];
    }

    // Legacy product-static values remain measurable, but are immediately
    // reduced to length/fingerprint and never copied into the receipt.
    const legacy = typeof node.text === "string"
      ? node.text
      : typeof node.value === "string"
      ? node.value
      : "";
    if (!legacy) return [];
    return [{
      semanticId: node.semanticId ?? null,
      role: node.role ?? node.type ?? null,
      contentKind: "productStatic",
      rawContentReturned: false,
      redacted: true,
      textLength: legacy.length,
      byteLength: new TextEncoder().encode(legacy).length,
      lineCount: legacy.split(/\r\n|\r|\n/).length,
      selected: node.selected ?? null,
      focused: node.focused ?? null,
      fingerprint: legacyFingerprint(legacy),
    }];
  });
}

/** Extract shaped line boxes and exact glyph paint unions from one completed frame. */
export function textFitMeasurements(
  layout: JsonObject,
  expectedBackingScaleFactor?: number | null,
) {
  const info = asObject(layout.info ?? layout);
  const fidelity = asObject(info.fidelity);
  const frameGeneration = asNumber(fidelity.frameGeneration, -1);
  const nodes = asArray(fidelity.nodes);
  const lines = nodes.filter((node) => node.kind === "textLine");

  return lines.map((node) => {
    const metadata = asObject(node.metadata);
    const lineBoxBounds = rect(node.bounds);
    const glyphBounds = rect(node.unionPaintBounds);
    const clipBounds = rect(node.clipBounds);
    const visibleBounds = rect(node.visibleBounds);
    const nodePaintOrder = asNumber(node.paintOrder, -1);
    const occluderMeasurementIds = nodes
      .filter((candidate) => candidate !== node)
      .filter((candidate) => asNumber(candidate.paintOrder, -1) > nodePaintOrder)
      .filter((candidate) => intersects(glyphBounds, rect(candidate.unionPaintBounds)))
      .map((candidate) => {
        const candidateMetadata = asObject(candidate.metadata);
        return String(candidateMetadata.measurementId ?? candidate.id ?? "unknown");
      });
    const ratio = visibleRatio(glyphBounds, clipBounds);
    const measurementFrameGeneration = asNumber(node.measurementFrameGeneration, -1);
    const fontsReady = metadata.fontsReady === true;
    const frameMatches = frameGeneration >= 0 && measurementFrameGeneration === frameGeneration;
    const backingScaleFactor = asNumber(metadata.backingScaleFactor, -1);
    const backingScaleMatches = expectedBackingScaleFactor == null ||
      (backingScaleFactor > 0 && Math.abs(backingScaleFactor - expectedBackingScaleFactor) < 0.001);
    const truncationPolicy = String(metadata.truncationPolicy ?? "unknown");
    const fullDisplayPass =
      truncationPolicy === "fullDisplay" &&
      ratio >= 0.999 &&
      occluderMeasurementIds.length === 0 &&
      fontsReady &&
      frameMatches &&
      backingScaleMatches;

    return {
      measurementId: metadata.measurementId ?? `text:${String(node.id ?? "unknown")}`,
      semanticId: metadata.semanticId ?? null,
      role: metadata.role ?? "textLineBox",
      lineBoxBounds,
      glyphBounds,
      textContainerBounds: lineBoxBounds,
      visibleBounds,
      clipBounds,
      wrappingPolicy: metadata.wrappingPolicy ?? "unknown",
      truncationPolicy,
      visibleRatio: ratio,
      occluderMeasurementIds,
      fontFamilyFingerprint: metadata.fontFamilyFingerprint ?? null,
      fontSize: metadata.fontSize ?? null,
      fontWeight: metadata.fontWeight ?? null,
      lineHeight: metadata.lineHeight ?? null,
      backingScaleFactor: backingScaleFactor > 0 ? backingScaleFactor : null,
      backingScaleMatches,
      fontsReady,
      frameGeneration: measurementFrameGeneration,
      captureFrameGeneration: frameGeneration,
      frameMatches,
      contentKind: metadata.contentKind ?? "userContent",
      graphemeCount: metadata.graphemeCount ?? null,
      lineCount: metadata.lineCount ?? 1,
      contentFingerprint: node.textHash ?? null,
      rawContentReturned: metadata.rawContentReturned === true,
      fullDisplayPass,
    };
  });
}

export function classifyTextProof(
  targetReceipt: JsonObject,
  stateEnvelope: JsonObject,
  elementsEnvelope: JsonObject,
  layoutEnvelope: JsonObject,
  rows: ReturnType<typeof textRows>,
  fits: ReturnType<typeof textFitMeasurements>,
  proofMode: TextProofMode,
) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopes([stateEnvelope, elementsEnvelope, layoutEnvelope]);
  if (transport !== "ok") return transport;
  if (rows.length === 0) return "blocked-by-missing-primitive";
  if (proofMode === "fit") {
    if (fits.length === 0) return "blocked-by-missing-primitive";
    if (fits.some((fit) => !fit.fullDisplayPass || fit.rawContentReturned)) return "not-ok";
  }
  return "ok";
}

async function main() {
  const argv = Bun.argv.slice(2);
  if (argv[0] !== "measure") {
    if (argv.includes("--help") || argv.includes("-h")) {
      console.log(usage());
      process.exit(0);
    }
    console.error(usage());
    process.exit(2);
  }
  const { args, extras, warnings: argWarnings } = parseTargetArgs(argv.slice(1), {
    extras: { "--limit": "number", "--proof-mode": "string" },
  });
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  const limit = extras["--limit"] ?? 120;
  const proofMode: TextProofMode = extras["--proof-mode"] === "fit" ? "fit" : "inspection";

  const clock = startClock();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "text" });
  const selector = (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ??
    args.target ?? { type: "focused" };
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: requestId("text", "state"),
    target: selector,
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
  const elementsEnvelope = await rpc(args.session, {
    type: "getElements",
    requestId: requestId("text", "elements"),
    target: selector,
    limit,
  }, "elementsResult", args.timeoutMs);
  const layoutEnvelope = await rpc(args.session, {
    type: "getLayoutInfo",
    requestId: requestId("text", "layout"),
    target: selector,
  }, "layoutInfoResult", args.timeoutMs);
  const state = responseOf(stateEnvelope);
  const elements = responseOf(elementsEnvelope);
  const layout = responseOf(layoutEnvelope);
  const nodes = asArray(elements.elements);
  const rows = textRows(nodes);
  const transaction = asObject(targetReceipt.transaction);
  const expectedBackingScaleFactor = typeof transaction.backingScaleFactor === "number"
    ? transaction.backingScaleFactor
    : null;
  const fits = textFitMeasurements(layout, expectedBackingScaleFactor);
  const inputValue = typeof state.inputValue === "string" ? state.inputValue : "";
  const selectedValue = typeof state.selectedValue === "string" ? state.selectedValue : "";
  const footerButtons = asArray((state.activeFooter as JsonObject | undefined)?.buttons);
  const footerTexts = footerButtons.map((button) => ({
    action: button.action ?? null,
    key: button.key ?? null,
    label: button.label ?? null,
    labelLength: typeof button.label === "string" ? button.label.length : null,
  }));
  const classification = classifyTextProof(
    targetReceipt,
    stateEnvelope,
    elementsEnvelope,
    layoutEnvelope,
    rows,
    fits,
    proofMode,
  );

  emitValidatedReceipt("devtools.text.measure", finishReceipt(
    { tool: "script-kit-devtools.text", command: "text.measure", session: args.session, clock },
    {
      classification,
      proofMode,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      transaction: targetReceipt.transaction,
      textSummary: {
        contentKind: "userContent",
        rawContentReturned: false,
        inputLength: inputValue.length,
        inputFingerprint: legacyFingerprint(inputValue),
        selectedLength: selectedValue.length,
        selectedFingerprint: legacyFingerprint(selectedValue),
        textNodeCount: rows.length,
        longestTextLength: rows.reduce((max, row) => Math.max(max, row.textLength), 0),
      },
      rows,
      textFits: fits,
      footerTexts,
      missingPrimitives: [
        rows.length === 0 ? "textNodes" : "",
        rows.some((row) => row.semanticId == null) ? "semanticId" : "",
        proofMode === "fit" && fits.length === 0 ? "shapedGlyphBounds" : "",
        proofMode === "fit" && fits.some((fit) => !fit.frameMatches) ? "sameFrameTextLayout" : "",
        proofMode === "fit" && fits.some((fit) => !fit.backingScaleMatches) ? "backingScaleFactor" : "",
        stateEnvelope.status === "error" ? "stateResult" : "",
        elementsEnvelope.status === "error" ? "elementsResult" : "",
        layoutEnvelope.status === "error" ? "layoutInfoResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
      ].filter(Boolean),
      warnings: [
        ...argWarnings,
        proofMode === "inspection" ? "Inspection mode does not claim glyph fit." : "",
      ].filter(Boolean),
      errors: diagnostic([
        ...((targetReceipt.errors as JsonObject[]) ?? []),
        ...[stateEnvelope, elementsEnvelope, layoutEnvelope].filter((value) => value.status === "error"),
      ]),
    },
  ));
}

if (import.meta.main) await main();
