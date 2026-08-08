#!/usr/bin/env bun
/** Scroll geometry inspection. Shared transport/args/receipts live in lib/client.ts. */

import {
  type JsonObject,
  type TargetArgs,
  classifyEnvelopeError,
  finishReceipt,
  parseTargetArgs,
  requestId,
  responseOf,
  rpc,
  run,
  serializeTargetFlags,
  startClock,
} from "./lib/client.ts";
import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic } from "./lib/privacy.ts";
import { maybeStartAndShow, resolveTargetReceipt } from "./lib/target-identity.ts";

type Rect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type NormalizedScrollMeasurement = {
  scroll: JsonObject;
  classification: string | null;
  missingPrimitive: string | null;
  listStateViewportHeight: number | null;
  effectiveViewportHeight: number | null;
  effectiveSafeViewportHeight: number | null;
  viewportMeasurementSource: string;
  viewportMeasurementWarning: string | null;
  selectedRowVisible: unknown;
  selectedRowAboveFooter: unknown;
};

export type RenderedSafeViewportMeasurement = {
  required: boolean;
  classification: "ok" | "not-ok" | "blocked-by-missing-primitive";
  selectedSemanticId: string | null;
  rowMeasurementId: string | null;
  safeViewportMeasurementId: string | null;
  rowBounds: Rect | null;
  rowVisibleBounds: Rect | null;
  safeViewportBounds: Rect | null;
  visibleRatio: number | null;
  withinSafeViewport: boolean | null;
  frameGeneration: number | null;
  frameMatches: boolean | null;
  targetDataGeneration: number | null;
  missingPrimitives: string[];
};

function usage() {
  return "Usage:\n  bun scripts/devtools/scroll.ts inspect [target args] [--require-affordance] [--require-native-list-contract] [--require-rendered-safe-viewport]";
}

function asObject(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value) ? value as JsonObject : {};
}

function asNumber(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function nodesOf(value: unknown): JsonObject[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is JsonObject => entry != null && typeof entry === "object")
    : [];
}

function rectFrom(value: unknown): Rect | null {
  const rect = asObject(value);
  const x = asNumber(rect.x);
  const y = asNumber(rect.y);
  const width = asNumber(rect.width);
  const height = asNumber(rect.height);
  return x == null || y == null || width == null || height == null
    ? null
    : { x, y, width, height };
}

function rectIntersectionRatio(bounds: Rect, clip: Rect) {
  const width = Math.max(
    0,
    Math.min(bounds.x + bounds.width, clip.x + clip.width) - Math.max(bounds.x, clip.x),
  );
  const height = Math.max(
    0,
    Math.min(bounds.y + bounds.height, clip.y + clip.height) - Math.max(bounds.y, clip.y),
  );
  const area = Math.max(0, bounds.width) * Math.max(0, bounds.height);
  return area > 0 ? (width * height) / area : 1;
}

export function notesScrollFromState(state: JsonObject): JsonObject {
  const notes = asObject(state.notes);
  const view = asObject(notes.view);
  const editorAnchor = asObject(notes.editorAnchor);
  const previewAnchor = asObject(notes.previewAnchor);
  const previewEnabled = previewAnchor.previewEnabled === true || view.previewEnabled === true;
  const owner = previewEnabled ? "notes.preview" : "notes.editor";
  const anchor = previewEnabled ? previewAnchor : editorAnchor;
  const scroll = asObject(anchor.scroll);
  const scrollHeight = asNumber(scroll.scrollHeight);
  const clientHeight = asNumber(scroll.clientHeight);
  return {
    owner,
    activeNoteId: notes.activeNoteId ?? null,
    generation: asObject(notes.generations).state ?? null,
    scrollTop: asNumber(scroll.scrollTop),
    contentHeight: scrollHeight,
    viewportHeight: clientHeight,
    safeViewportHeight: clientHeight,
    maxScrollTop: scrollHeight != null && clientHeight != null ? Math.max(0, scrollHeight - clientHeight) : null,
    selectedIndex: null,
    selectedRowTop: null,
    selectedRowBottom: null,
    selectedRowVisible: null,
    selectedRowAboveFooter: null,
    itemCount: asObject(notes.counts).noteCount ?? null,
    anchor,
  };
}

export function mainListScrollFromState(state: JsonObject) {
  return asObject(state.mainListScroll);
}

export function activeListScrollFromState(state: JsonObject) {
  const active = asObject(state.activeListScroll);
  return Object.keys(active).length > 0 ? active : mainListScrollFromState(state);
}

export const NATIVE_LIST_CONTRACT_FIELDS = [
  "surface", "implementation", "listKind", "selectedIndex", "selectedSemanticId",
  "hoveredIndex", "hoveredSemanticId", "hoverSuppressedUntilPointerMove",
  "focusedSemanticId", "logicalScrollTop", "scrollTopItem", "scrollTopOffsetItems",
  "scrollTopOffsetPx", "firstVisibleIndex", "lastVisibleIndexExclusive",
  "firstVisibleSemanticId", "lastVisibleSemanticId", "itemCount", "contentHeight",
  "viewportHeight", "safeViewportHeight", "maxScrollTop", "selectedRowVisible",
  "selectedRowWithinSafeViewport", "inputMode", "lastInteractionSource",
] as const;

export function inspectNativeListContract(scroll: JsonObject, required: boolean) {
  const missingFields = NATIVE_LIST_CONTRACT_FIELDS
    .filter((field) => !Object.prototype.hasOwnProperty.call(scroll, field))
    .map((field) => `activeListScroll.${field}`);
  return {
    required,
    present: Object.keys(scroll).length > 0,
    complete: missingFields.length === 0,
    missingFields,
    classification: required && missingFields.length > 0
      ? "blocked-by-missing-primitive"
      : "ok",
  };
}

export const MAIN_LIST_SCROLL_AFFORDANCE_FIELDS = [
  "atTop",
  "atBottom",
  "topFadeActive",
  "topFadeProgress",
  "topFadeAlpha",
  "overscrollOffsetPx",
  "overscrollMaxOffsetPx",
  "overscrollEdge",
  "overscrollPhase",
  "generation",
  "lastTouchPhase",
  "lastSettleReason",
  "reducedMotion",
] as const;

export function inspectMainListScrollAffordance(
  scroll: JsonObject,
  required: boolean,
) {
  const raw = scroll.affordance;
  const present = raw != null && typeof raw === "object" && !Array.isArray(raw);
  const affordance = present ? raw as JsonObject : null;
  const missingFields = MAIN_LIST_SCROLL_AFFORDANCE_FIELDS
    .filter((field) => affordance == null || !Object.prototype.hasOwnProperty.call(affordance, field))
    .map((field) => `mainListScroll.affordance.${field}`);
  return {
    required,
    present,
    complete: present && missingFields.length === 0,
    missingFields,
    affordance,
    classification: required && missingFields.length > 0
      ? "blocked-by-missing-primitive"
      : "ok",
  };
}

async function layoutMeasurement(args: TargetArgs): Promise<JsonObject | null> {
  const layoutReceipt = await run(
    ["bun", "scripts/devtools/layout.ts", "measure", ...serializeTargetFlags(args)],
    "layout.measure",
  );
  return layoutReceipt.status === "error" ? null : layoutReceipt;
}

function layoutNodeBounds(layoutReceipt: JsonObject, name: string): Rect | null {
  const nodes = nodesOf(layoutReceipt.nodes);
  const node = nodes.find((entry) => entry.name === name);
  return rectFrom(node?.bounds);
}

export function normalizeScriptListScrollMeasurement(
  scroll: JsonObject,
  layoutReceipt: JsonObject | null,
): NormalizedScrollMeasurement {
  const listStateViewportHeight = asNumber(scroll.viewportHeight);
  const listStateSafeViewportHeight = asNumber(scroll.safeViewportHeight);
  const rawSelectedRowVisible = scroll.selectedRowVisible;
  const rawSelectedRowAboveFooter = scroll.selectedRowAboveFooter;

  if (listStateViewportHeight == null || listStateViewportHeight > 0) {
    return {
      scroll,
      classification: null,
      missingPrimitive: null,
      listStateViewportHeight,
      effectiveViewportHeight: listStateViewportHeight,
      effectiveSafeViewportHeight: listStateSafeViewportHeight,
      viewportMeasurementSource: "listState",
      viewportMeasurementWarning: null,
      selectedRowVisible: rawSelectedRowVisible ?? null,
      selectedRowAboveFooter: rawSelectedRowAboveFooter ?? null,
    };
  }

  const listBounds = layoutReceipt ? layoutNodeBounds(layoutReceipt, "ScriptList") : null;
  const selectedIndex = asNumber(scroll.selectedIndex);
  const selectedRowBounds =
    selectedIndex == null || !layoutReceipt
      ? null
      : layoutNodeBounds(layoutReceipt, `ListItem[${selectedIndex}]`);
  const footerBounds = layoutReceipt ? layoutNodeBounds(layoutReceipt, "MainViewFooter") : null;

  if (!listBounds) {
    return {
      scroll: {
        ...scroll,
        selectedRowVisible: null,
        selectedRowAboveFooter: null,
      },
      classification: "blocked-by-missing-primitive",
      missingPrimitive: "scriptListBounds",
      listStateViewportHeight,
      effectiveViewportHeight: null,
      effectiveSafeViewportHeight: null,
      viewportMeasurementSource: "listState",
      viewportMeasurementWarning: "listStateViewportUnmeasured",
      selectedRowVisible: null,
      selectedRowAboveFooter: null,
    };
  }

  const effectiveViewportHeight = listBounds.height;
  const effectiveSafeViewportHeight = footerBounds
    ? Math.max(0, footerBounds.y - listBounds.y)
    : effectiveViewportHeight;
  const selectedRowVisible = selectedRowBounds
    ? selectedRowBounds.y >= listBounds.y
      && selectedRowBounds.y + selectedRowBounds.height <= listBounds.y + listBounds.height
    : rawSelectedRowVisible ?? null;
  const selectedRowAboveFooter = selectedRowBounds
    ? selectedRowBounds.y + selectedRowBounds.height <= listBounds.y + effectiveSafeViewportHeight
    : rawSelectedRowAboveFooter ?? null;

  return {
    scroll: {
      ...scroll,
      viewportHeight: effectiveViewportHeight,
      safeViewportHeight: effectiveSafeViewportHeight,
      selectedRowTop: selectedRowBounds ? selectedRowBounds.y - listBounds.y : null,
      selectedRowBottom: selectedRowBounds
        ? selectedRowBounds.y + selectedRowBounds.height - listBounds.y
        : null,
      selectedRowVisible,
      selectedRowAboveFooter,
    },
    classification: null,
    missingPrimitive: null,
    listStateViewportHeight,
    effectiveViewportHeight,
    effectiveSafeViewportHeight,
    viewportMeasurementSource: "layout",
    viewportMeasurementWarning: selectedRowBounds
      ? "listStateViewportUnmeasured"
      : "listStateViewportUnmeasured;selectedRowBoundsUnavailable",
    selectedRowVisible,
    selectedRowAboveFooter,
  };
}

/** Join the selected semantic row to completed-frame paint and its rendered safe viewport. */
export function renderedSafeViewportMeasurement(
  scroll: JsonObject,
  layoutReceipt: JsonObject | null,
  required: boolean,
): RenderedSafeViewportMeasurement {
  const selectedSemanticId = typeof scroll.selectedSemanticId === "string"
    ? scroll.selectedSemanticId
    : null;
  const nodes = nodesOf(layoutReceipt?.nodes);
  const renderedNodes = nodes.filter((node) => node.measurementProvenance === "paint-time");
  const row = selectedSemanticId == null
    ? null
    : renderedNodes.find((node) => node.name === `list-row:${selectedSemanticId}`) ?? null;
  const safeViewport = renderedNodes.find((node) => node.name === "main-view-main") ?? null;
  const rowBounds = rectFrom(row?.bounds);
  const rowVisibleBounds = rectFrom(row?.visibleBounds);
  const safeViewportBounds = rectFrom(safeViewport?.visibleBounds ?? safeViewport?.bounds);
  const rowFrame = asNumber(row?.measurementFrameGeneration);
  const viewportFrame = asNumber(safeViewport?.measurementFrameGeneration);
  const frameMatches = rowFrame == null || viewportFrame == null ? null : rowFrame === viewportFrame;
  const visibleRatio = rowBounds && rowVisibleBounds && safeViewportBounds
    ? Math.min(
        rectIntersectionRatio(rowBounds, rowVisibleBounds),
        rectIntersectionRatio(rowBounds, safeViewportBounds),
      )
    : null;
  const withinSafeViewport = visibleRatio == null ? null : visibleRatio >= 0.999;
  const transaction = asObject(layoutReceipt?.transaction);
  const targetDataGeneration = asNumber(transaction.dataGeneration);
  const missingPrimitives = [
    selectedSemanticId == null ? "selectedSemanticId" : "",
    rowBounds == null ? "renderedSelectedRowBounds" : "",
    rowVisibleBounds == null ? "renderedSelectedRowVisibleBounds" : "",
    safeViewportBounds == null ? "renderedSafeViewportBounds" : "",
    frameMatches == null ? "renderedFrameGeneration" : "",
    targetDataGeneration == null ? "targetDataGeneration" : "",
  ].filter(Boolean);
  const classification = required && missingPrimitives.length > 0
    ? "blocked-by-missing-primitive"
    : required && (!withinSafeViewport || frameMatches !== true)
    ? "not-ok"
    : "ok";

  return {
    required,
    classification,
    selectedSemanticId,
    rowMeasurementId: typeof row?.measurementId === "string" ? row.measurementId : null,
    safeViewportMeasurementId: typeof safeViewport?.measurementId === "string"
      ? safeViewport.measurementId
      : null,
    rowBounds,
    rowVisibleBounds,
    safeViewportBounds,
    visibleRatio,
    withinSafeViewport,
    frameGeneration: rowFrame,
    frameMatches,
    targetDataGeneration,
    missingPrimitives,
  };
}

function classify(
  targetReceipt: JsonObject,
  stateEnvelope: JsonObject,
  scroll: JsonObject,
  measurementClassification?: string | null,
  affordanceClassification?: string | null,
  nativeContractClassification?: string | null,
  nativeContractRequired = false,
  renderedClassification?: string | null,
) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopeError(stateEnvelope);
  if (transport !== "ok") {
    return transport;
  }
  if (measurementClassification) {
    return measurementClassification;
  }
  if (affordanceClassification && affordanceClassification !== "ok") {
    return affordanceClassification;
  }
  if (nativeContractClassification && nativeContractClassification !== "ok") {
    return nativeContractClassification;
  }
  if (renderedClassification && renderedClassification !== "ok") {
    return renderedClassification;
  }
  if (Object.keys(scroll).length === 0
    || (!nativeContractRequired && (scroll.scrollTop == null || scroll.viewportHeight == null))) {
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
  const { args, extras, warnings: argWarnings } = parseTargetArgs(argv.slice(1), {
    extras: {
      "--require-affordance": "boolean",
      "--require-native-list-contract": "boolean",
      "--require-rendered-safe-viewport": "boolean",
    },
  });
  const requireAffordance = extras["--require-affordance"] === true;
  const requireNativeListContract = extras["--require-native-list-contract"] === true;
  const requireRenderedSafeViewport = extras["--require-rendered-safe-viewport"] === true;
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }

  const clock = startClock();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "scroll" });
  const selector = (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ?? args.target ?? { type: "focused" };
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: requestId("scroll", "state"),
    target: selector,
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
  const state = responseOf(stateEnvelope);
  const resolved = asObject(targetReceipt.resolvedTarget);
  const resolvedKind = String(resolved.targetKind ?? "").toLowerCase();
  const resolvedSurface = String(resolved.semanticSurface ?? resolved.surfaceKind ?? "").toLowerCase();
  const isNotesTarget = resolvedKind === "notes" || resolvedSurface === "notes";
  const rawScroll = isNotesTarget ? notesScrollFromState(state) : activeListScrollFromState(state);
  const isScriptList = !isNotesTarget && (rawScroll.surface == null || rawScroll.surface === "script_list");
  const renderedLayout = !isNotesTarget && requireRenderedSafeViewport
    ? await layoutMeasurement(args)
    : null;
  const normalized: NormalizedScrollMeasurement = isNotesTarget || !isScriptList
    ? {
        scroll: rawScroll,
        classification: null,
        missingPrimitive: null,
        listStateViewportHeight: asNumber(rawScroll.viewportHeight),
        effectiveViewportHeight: asNumber(rawScroll.viewportHeight),
        effectiveSafeViewportHeight: asNumber(rawScroll.safeViewportHeight),
        viewportMeasurementSource: isNotesTarget ? "listState" : "activeListScroll",
        viewportMeasurementWarning: null,
        selectedRowVisible: rawScroll.selectedRowVisible ?? null,
        selectedRowAboveFooter: rawScroll.selectedRowAboveFooter ?? null,
      }
    : normalizeScriptListScrollMeasurement(
        rawScroll,
        asNumber(rawScroll.viewportHeight) != null && (asNumber(rawScroll.viewportHeight) ?? 0) <= 0
          ? renderedLayout ?? await layoutMeasurement(args)
          : null,
      );
  const scroll = normalized.scroll;
  const renderedSafeViewport = renderedSafeViewportMeasurement(
    scroll,
    renderedLayout,
    requireRenderedSafeViewport,
  );
  const contentHeight = asNumber(scroll.contentHeight);
  const viewportHeight = asNumber(scroll.viewportHeight);
  const maxScrollTop = asNumber(scroll.maxScrollTop);
  const scrollTop = asNumber(scroll.scrollTop);
  const canScrollY = maxScrollTop != null ? maxScrollTop > 0 : contentHeight != null && viewportHeight != null ? contentHeight > viewportHeight : null;
  const affordanceInspection = inspectMainListScrollAffordance(
    isNotesTarget ? {} : scroll,
    requireAffordance,
  );
  const nativeContractInspection = inspectNativeListContract(
    isNotesTarget ? {} : scroll,
    requireNativeListContract,
  );
  const classification = classify(
    targetReceipt,
    stateEnvelope,
    scroll,
    normalized.classification,
    affordanceInspection.classification,
    nativeContractInspection.classification,
    requireNativeListContract,
    renderedSafeViewport.classification,
  );

  emitValidatedReceipt("devtools.scroll.inspect", finishReceipt(
    { tool: "script-kit-devtools.scroll", command: "scroll.inspect", session: args.session, clock },
    {
      classification,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      transaction: targetReceipt.transaction,
      scroll: {
        surface: scroll.surface ?? null,
        implementation: scroll.implementation ?? null,
        listKind: scroll.listKind ?? null,
        logicalScrollTop: scroll.logicalScrollTop ?? null,
        scrollTop,
        scrollTopItem: scroll.scrollTopItem ?? null,
        scrollTopOffset: scroll.scrollTopOffset ?? null,
        scrollTopOffsetItems: scroll.scrollTopOffsetItems ?? null,
        scrollTopOffsetPx: scroll.scrollTopOffsetPx ?? null,
        firstVisibleIndex: scroll.firstVisibleIndex ?? null,
        lastVisibleIndexExclusive: scroll.lastVisibleIndexExclusive ?? null,
        firstVisibleSemanticId: scroll.firstVisibleSemanticId ?? null,
        lastVisibleSemanticId: scroll.lastVisibleSemanticId ?? null,
        contentHeight,
        viewportHeight,
        listStateViewportHeight: normalized.listStateViewportHeight,
        effectiveViewportHeight: normalized.effectiveViewportHeight,
        viewportMeasurementSource: normalized.viewportMeasurementSource,
        viewportMeasurementWarning: normalized.viewportMeasurementWarning,
        safeViewportHeight: scroll.safeViewportHeight ?? null,
        effectiveSafeViewportHeight: normalized.effectiveSafeViewportHeight,
        headerOverlayHeight: scroll.headerOverlayHeight ?? null,
        headerGlassStripHeight: scroll.headerGlassStripHeight ?? null,
        footerHeight: scroll.footerHeight ?? null,
        footerOverlayHeight: scroll.footerOverlayHeight ?? null,
        footerRevealClearanceHeight: scroll.footerRevealClearanceHeight ?? null,
        footerOverlayTotalPadding: scroll.footerOverlayTotalPadding ?? null,
        maxScrollTop,
        canScrollY,
        selectedIndex: scroll.selectedIndex ?? state.selectedIndex ?? null,
        selectedSemanticId: scroll.selectedSemanticId ?? null,
        selectedStableKey: scroll.selectedStableKey ?? null,
        selectedRowTop: scroll.selectedRowTop ?? null,
        selectedRowBottom: scroll.selectedRowBottom ?? null,
        selectedRowVisible: scroll.selectedRowVisible ?? null,
        selectedRowAboveFooter: scroll.selectedRowAboveFooter ?? null,
        selectedRowWithinSafeViewport: scroll.selectedRowWithinSafeViewport ?? null,
        hoveredIndex: scroll.hoveredIndex ?? null,
        hoveredSemanticId: scroll.hoveredSemanticId ?? null,
        hoverSuppressedUntilPointerMove: scroll.hoverSuppressedUntilPointerMove ?? null,
        inputMode: scroll.inputMode ?? null,
        focusedSemanticId: scroll.focusedSemanticId ?? null,
        lastInteractionSource: scroll.lastInteractionSource ?? null,
        performance: scroll.performance ?? null,
        itemCount: scroll.itemCount ?? state.visibleChoiceCount ?? null,
        owner: scroll.owner ?? (isNotesTarget ? "notes.unknown" : "main.list"),
        activeNoteId: scroll.activeNoteId ?? null,
        generation: scroll.generation ?? null,
        affordance: affordanceInspection.affordance,
      },
      affordanceRequirement: {
        required: affordanceInspection.required,
        present: affordanceInspection.present,
        complete: affordanceInspection.complete,
        missingFields: affordanceInspection.missingFields,
      },
      nativeListContractRequirement: nativeContractInspection,
      renderedSafeViewport,
      missingPrimitive: normalized.missingPrimitive,
      resizePressure: {
        overflowY: canScrollY,
        hiddenContentHeight: contentHeight != null && viewportHeight != null ? Math.max(0, contentHeight - viewportHeight) : null,
        selectedRowOutsideSafeViewport:
          scroll.selectedRowWithinSafeViewport === false
          || scroll.selectedRowVisible === false
          || scroll.selectedRowAboveFooter === false,
      },
      missingPrimitives: [
        Object.keys(scroll).length === 0
          || (!requireNativeListContract && (scroll.scrollTop == null || scroll.viewportHeight == null))
          ? isNotesTarget ? "notesScrollMetrics" : "mainListScroll"
          : "",
        stateEnvelope.status === "error" ? "stateResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
        normalized.missingPrimitive ?? "",
        ...(requireAffordance ? affordanceInspection.missingFields : []),
        ...(requireNativeListContract ? nativeContractInspection.missingFields : []),
        ...(requireRenderedSafeViewport ? renderedSafeViewport.missingPrimitives : []),
      ].filter(Boolean),
      warnings: argWarnings,
      errors: diagnostic([
        ...((targetReceipt.errors as JsonObject[]) ?? []),
        ...[stateEnvelope].filter((value) => value.status === "error"),
      ]),
    },
  ));
}

if (import.meta.main) {
  await main();
}
