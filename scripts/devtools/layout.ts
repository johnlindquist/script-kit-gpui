#!/usr/bin/env bun

import {
  appleGuidelineConformance,
  type NodeLike,
} from "./apple-guideline-constants";
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
import { evidenceIntersectionRatio, isValidEvidenceRect } from "./lib/geometry-evidence.ts";
import { diagnostic } from "./lib/privacy.ts";
import { maybeStartAndShow, resolveTargetReceipt } from "./lib/target-identity.ts";

type Rect = { x: number; y: number; width: number; height: number };

async function sha256File(path: string) {
  const bytes = await Bun.file(path).arrayBuffer();
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function layoutSourceFingerprint() {
  const paths = [
    "src/app_layout/build_layout_info.rs",
    "src/app_layout/paint_measurements.rs",
    "src/protocol/types/grid_layout.rs",
    "scripts/devtools/layout.ts",
    "src/ui/chrome/tokens.rs",
  ];
  return {
    schemaVersion: 1,
    algorithm: "sha256",
    files: await Promise.all(
      paths.map(async (path) => ({
        path,
        sha256: await sha256File(path),
      })),
    ),
  };
}

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/layout.ts measure [target args] [--include nodes,regions,scroll,anchors,resize,overlaps] [--proof-mode inspection|join] [--limit <n>]",
    "",
    "Target args match scripts/devtools/targets.ts inspect, e.g. --session <name> --main --strict --surface ScriptList --start --show.",
  ].join("\n");
}

function asObject(value: unknown): JsonObject {
  return typeof value === "object" && value !== null ? value as JsonObject : {};
}

function asArray(value: unknown): JsonObject[] {
  return Array.isArray(value)
    ? value.filter(
        (entry): entry is JsonObject =>
          typeof entry === "object" && entry !== null,
      )
    : [];
}

function asNumber(value: unknown, fallback = 0) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function hasPositiveRadius(value: unknown) {
  if (typeof value === "number") {
    return Number.isFinite(value) && value > 0;
  }
  if (!value || typeof value !== "object") {
    return false;
  }
  const radii = Object.values(value as JsonObject).filter(
    (entry): entry is number => typeof entry === "number" && Number.isFinite(entry),
  );
  return radii.length > 0 && radii.every((entry) => entry > 0);
}

// Radius-bearing surfaces are container/fill-owning node TYPES, not bare text
// labels or 1px hairline dividers. This is type-based (NOT a node-name
// whitelist) and mirrors the same predicate in liquid-glass-proof.ts so the
// two audit layers agree on which nodes must carry a positive Liquid Glass
// radius. Title/count text labels and hairline dividers paint no rounded
// surface and are intentionally excluded by their non-container node type.
const RADIUS_BEARING_NODE_TYPES = new Set([
  "area",
  "button",
  "card",
  "container",
  "header",
  "input",
  "list",
  "listitem",
  "panel",
  "prompt",
  "window",
]);

function isRadiusBearingNode(node: { type: unknown }) {
  const type = String(node.type ?? "").toLowerCase();
  return RADIUS_BEARING_NODE_TYPES.has(type);
}

function rectFrom(value: unknown): Rect {
  const object =
    value && typeof value === "object" ? (value as JsonObject) : {};
  return {
    x: asNumber(object.x),
    y: asNumber(object.y),
    width: asNumber(object.width),
    height: asNumber(object.height),
  };
}

function optionalRectFrom(value: unknown): Rect | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  return rectFrom(value);
}

function right(rect: Rect) {
  return rect.x + rect.width;
}

function bottom(rect: Rect) {
  return rect.y + rect.height;
}

function intersects(a: Rect, b: Rect) {
  return a.x < right(b) && right(a) > b.x && a.y < bottom(b) && bottom(a) > b.y;
}

function clippedBy(rect: Rect, viewport: Rect) {
  return (
    rect.x < viewport.x ||
    rect.y < viewport.y ||
    right(rect) > right(viewport) ||
    bottom(rect) > bottom(viewport)
  );
}

function hitMetrics(component: JsonObject, bounds: Rect) {
  const style =
    component.visualStyle && typeof component.visualStyle === "object"
      ? (component.visualStyle as JsonObject)
      : null;
  const hitBounds = optionalRectFrom(style?.hitBounds) ?? bounds;
  const visualBounds = optionalRectFrom(style?.visualBounds) ?? bounds;
  const exception =
    typeof style?.exception === "string" ? style.exception : null;
  return {
    hitBounds,
    visualBounds,
    hitWidth: hitBounds.width,
    hitHeight: hitBounds.height,
    visualWidth: visualBounds.width,
    visualHeight: visualBounds.height,
    minHitPass: hitBounds.width >= 28 && hitBounds.height >= 28,
    minVisualPass: visualBounds.width >= 20 && visualBounds.height >= 20,
    preferredHitPass: hitBounds.width >= 44 && hitBounds.height >= 44,
    exception,
  };
}

function center(rect: Rect) {
  return {
    x: rect.x + rect.width / 2,
    y: rect.y + rect.height / 2,
  };
}

function distance(a: { x: number; y: number }, b: { x: number; y: number }) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function buttonCenterDistanceAssertions(nodes: Array<{
  name: unknown;
  type: unknown;
  parent: unknown;
  hitMetrics: ReturnType<typeof hitMetrics>;
}>) {
  const buttons = nodes.filter((node) => String(node.type ?? "") === "button");
  const failures = [];
  for (let left = 0; left < buttons.length; left += 1) {
    for (let rightIndex = left + 1; rightIndex < buttons.length; rightIndex += 1) {
      const a = buttons[left];
      const b = buttons[rightIndex];
      if (a.parent !== b.parent) continue;
      if (a.hitMetrics.exception || b.hitMetrics.exception) continue;
      const centerDistance = distance(
        center(a.hitMetrics.hitBounds),
        center(b.hitMetrics.hitBounds),
      );
      if (centerDistance < 60) {
        failures.push({
          a: a.name,
          b: b.name,
          centerDistance,
          required: 60,
          source: "apple-documented",
        });
      }
    }
  }
  return {
    source: "apple-documented",
    requiredCenterDistance: 60,
    failures,
  };
}

function liquidGlassWindowBackdropVisualStyle(bounds: Rect): JsonObject {
  return {
    chromeLayer: "windowBackdrop",
    materialSource: "nativeWindowBackdrop",
    tokenSource: "window.backdrop",
    cornerRadius: {
      topLeft: 22,
      topRight: 22,
      bottomRight: 22,
      bottomLeft: 22,
    },
    visualBounds: bounds,
    hitBounds: bounds,
  };
}

function ensureRootWindowBackdropNode(
  components: JsonObject[],
  targetBounds: Rect,
  viewportRect: Rect,
) {
  const alreadyHasBackdrop = components.some((component) => {
    const style = asObject(component.visualStyle);
    return component.name === "Window" || style.chromeLayer === "windowBackdrop";
  });
  if (alreadyHasBackdrop) {
    return components;
  }

  const bounds = {
    x: 0,
    y: 0,
    width: viewportRect.width || targetBounds.width,
    height: viewportRect.height || targetBounds.height,
  };
  const windowNode: JsonObject = {
    name: "Window",
    type: "container",
    bounds,
    depth: 0,
    parent: null,
    children: components
      .filter((component) => component.parent == null && component.name)
      .map((component) => component.name),
    explanation:
      "Root native window backdrop metadata derived from strict target bounds; target layout provider describes the content subtree.",
    visualStyle: liquidGlassWindowBackdropVisualStyle(bounds),
  };

  return [
    windowNode,
    ...components.map((component) => {
      if (component.parent != null || component.name === "Window") {
        return component;
      }
      return {
        ...component,
        parent: "Window",
        depth: asNumber(component.depth, 0) + 1,
      };
    }),
  ];
}

export type GeometryRole =
  | "windowBackdrop"
  | "mainHeaderChrome"
  | "contextZone"
  | "inputControl"
  | "contentViewport"
  | "rowSlot"
  | "sectionSlot"
  | "footerNativeHost"
  | "footerRail"
  | "footerActionRow"
  | "footerActionSlot"
  | "keycapInnerFrame"
  | "popupShell"
  | "popupAnchor"
  | "textLineBox"
  | "glyphBounds"
  | "focusRing"
  | "other";

export type MeasurementJoin = {
  measurementId: string;
  semanticId: string | null;
  role: GeometryRole;
  coordinateSpace: string;
  intended: {
    contractId: string;
    sourcePath: string;
    sourceSymbol: string;
    relation: string;
  } | null;
  model: { bounds: Rect; generation: number } | null;
  rendered: {
    bounds: Rect;
    visibleBounds: Rect;
    clipBounds: Rect;
    frameGeneration: number;
    source: "paint-time" | "appkit" | "browser";
  } | null;
  comparability:
    | "Comparable"
    | "RoleMismatch"
    | "SemanticMismatch"
    | "CoordinateSpaceMismatch"
    | "StaleGeneration"
    | "DuplicateMeasurement"
    | "InvalidProvenance"
    | "InvalidGeometry"
    | "ModelOnly"
    | "RenderedOnly";
  delta: Rect | null;
  tolerance: Rect | null;
  classification:
    | "Match"
    | "Clipped"
    | "OutOfTolerance"
    | "ModelOverlap"
    | "RenderedOverlap"
    | "NotComparable";
};

function intendedGeometry(role: GeometryRole) {
  if (role === "other") return null;
  return {
    contractId: `geometry-role:${role}`,
    sourcePath: "src/protocol/types/grid_layout.rs",
    sourceSymbol: "GeometryRole",
    relation: "Compare equal roles in one coordinate space and capture generation; preserve containment between distinct roles.",
  };
}

function rectDelta(model: Rect, rendered: Rect): Rect {
  return {
    x: rendered.x - model.x,
    y: rendered.y - model.y,
    width: rendered.width - model.width,
    height: rendered.height - model.height,
  };
}

function layerOverlaps(nodes: Array<{ name: unknown; measurementId: string; role: GeometryRole; bounds: Rect; depth: unknown; parent: unknown; hitMetrics: ReturnType<typeof hitMetrics> }>) {
  const overlaps: Array<{ a: unknown; b: unknown; aMeasurementId: string; bMeasurementId: string }> = [];
  for (let left = 0; left < nodes.length; left += 1) {
    for (let rightIndex = left + 1; rightIndex < nodes.length; rightIndex += 1) {
      const a = nodes[left];
      const b = nodes[rightIndex];
      const sameSiblingBand = a.depth != null && b.depth != null &&
        a.parent != null && b.parent != null &&
        a.depth === b.depth && a.parent === b.parent;
      const floatingOverlayPair =
        a.hitMetrics?.exception === "floatingFooterOverlay" ||
        b.hitMetrics?.exception === "floatingFooterOverlay";
      if (sameSiblingBand && !floatingOverlayPair && a.name && b.name && intersects(a.bounds, b.bounds)) {
        overlaps.push({ a: a.name, b: b.name, aMeasurementId: a.measurementId, bMeasurementId: b.measurementId });
      }
    }
  }
  return overlaps;
}

export function buildMeasurementJoins(nodes: Array<{
  measurementId: string;
  semanticId: string | null;
  role: GeometryRole;
  bounds: Rect;
  visibleBounds: Rect | null;
  clipBounds: Rect | null;
  measurementProvenance: unknown;
  coordinateSpace: unknown;
  measurementFrameGeneration: unknown;
}>): MeasurementJoin[] {
  const groups = new Map<string, typeof nodes>();
  for (const node of nodes) {
    const group = groups.get(node.measurementId) ?? [];
    group.push(node);
    groups.set(node.measurementId, group);
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([measurementId, group]) => {
    const modelNodes = group.filter((node) => node.measurementProvenance === "model");
    const renderedNodes = group.filter((node) => node.measurementProvenance === "paint-time");
    const invalidProvenance = group.some((node) =>
      node.measurementProvenance !== "model" &&
      node.measurementProvenance !== "paint-time"
    );
    const modelNode = modelNodes[0] ?? null;
    const renderedNode = renderedNodes[0] ?? null;
    const role = (modelNode?.role ?? renderedNode?.role ?? "other") as GeometryRole;
    const modelGeneration = asNumber(modelNode?.measurementFrameGeneration, -1);
    const renderedGeneration = asNumber(renderedNode?.measurementFrameGeneration, -1);
    const modelSpace = String(modelNode?.coordinateSpace ?? "unknown");
    const renderedSpace = String(renderedNode?.coordinateSpace ?? "unknown");
    let comparability: MeasurementJoin["comparability"];
    if (invalidProvenance) comparability = "InvalidProvenance";
    else if (modelNodes.length > 1 || renderedNodes.length > 1) comparability = "DuplicateMeasurement";
    else if (!modelNode) comparability = "RenderedOnly";
    else if (!renderedNode) comparability = "ModelOnly";
    else if (modelNode.role !== renderedNode.role || role === "other") comparability = "RoleMismatch";
    else if (modelNode.semanticId !== renderedNode.semanticId) comparability = "SemanticMismatch";
    else if (
      modelSpace === "unknown" ||
      renderedSpace === "unknown" ||
      modelSpace.trim().length === 0 ||
      modelSpace !== renderedSpace
    ) comparability = "CoordinateSpaceMismatch";
    else if (
      !Number.isSafeInteger(modelGeneration) ||
      !Number.isSafeInteger(renderedGeneration) ||
      modelGeneration < 0 ||
      renderedGeneration < 0 ||
      modelGeneration !== renderedGeneration
    ) comparability = "StaleGeneration";
    else if (
      !isValidEvidenceRect(modelNode.bounds) ||
      !isValidEvidenceRect(renderedNode.bounds) ||
      !isValidEvidenceRect(renderedNode.visibleBounds, true) ||
      !isValidEvidenceRect(renderedNode.clipBounds, true)
    ) comparability = "InvalidGeometry";
    else comparability = "Comparable";
    const delta = modelNode && renderedNode &&
      isValidEvidenceRect(modelNode.bounds) && isValidEvidenceRect(renderedNode.bounds)
      ? rectDelta(modelNode.bounds, renderedNode.bounds)
      : null;
    const tolerance = { x: 1, y: 1, width: 1, height: 1 };
    const outsideTolerance = delta != null && Object.entries(delta).some(([key, value]) => Math.abs(value) > tolerance[key as keyof Rect]);
    const renderedVisibleRatio = renderedNode &&
      isValidEvidenceRect(renderedNode.bounds) &&
      isValidEvidenceRect(renderedNode.visibleBounds, true) &&
      isValidEvidenceRect(renderedNode.clipBounds, true)
      ? Math.min(
        evidenceIntersectionRatio(renderedNode.bounds, renderedNode.visibleBounds),
        evidenceIntersectionRatio(renderedNode.bounds, renderedNode.clipBounds),
      )
      : 1;
    const classification: MeasurementJoin["classification"] = comparability !== "Comparable"
      ? "NotComparable"
      : renderedVisibleRatio < 0.999
      ? "Clipped"
      : outsideTolerance
      ? "OutOfTolerance"
      : "Match";
    return {
      measurementId,
      semanticId: modelNode?.semanticId ?? renderedNode?.semanticId ?? null,
      role,
      coordinateSpace: modelNode ? modelSpace : renderedSpace,
      intended: intendedGeometry(role),
      model: modelNode ? { bounds: modelNode.bounds, generation: modelGeneration } : null,
      rendered: renderedNode ? {
        bounds: renderedNode.bounds,
        visibleBounds: renderedNode.visibleBounds ?? renderedNode.bounds,
        clipBounds: renderedNode.clipBounds ?? renderedNode.bounds,
        frameGeneration: renderedGeneration,
        source: "paint-time",
      } : null,
      comparability,
      delta,
      tolerance,
      classification,
    };
  });
}

export function analyzeLayout(layout: JsonObject, targetReceipt: JsonObject) {
  const info = (layout.info as JsonObject | undefined) ?? layout;
  const rawComponents = asArray(info.components);
  const targetBounds = rectFrom(
    (targetReceipt.resolvedTarget as JsonObject | undefined)?.bounds,
  );
  const viewportRect = {
    x: 0,
    y: 0,
    width: asNumber(info.windowWidth, targetBounds.width),
    height: asNumber(info.windowHeight, targetBounds.height),
  };
  const fidelity = asObject(info.fidelity);
  const fidelityNodes = asArray(fidelity.nodes).map((node) => {
    const metadata = asObject(node.metadata);
    return {
      name: node.id,
      type: node.kind,
      semanticId: metadata.semanticId ?? null,
      measurementId: metadata.measurementId ?? `paint:${String(node.id ?? "unknown")}`,
      geometryRole: metadata.role ?? (node.kind === "textLine" ? "textLineBox" : "other"),
      bounds: node.bounds,
      visibleBounds: node.visibleBounds,
      clipBounds: node.clipBounds,
      measurementProvenance: node.measurementProvenance ?? "paint-time",
      coordinateSpace: node.coordinateSpace ?? "window",
      measurementFrameGeneration: node.measurementFrameGeneration ?? fidelity.frameGeneration,
      depth: null,
      parent: node.parentId ?? null,
      children: [],
      fidelityMetadata: metadata,
      unionPaintBounds: node.unionPaintBounds,
      primitiveCount: node.primitiveCount,
      paintOrder: node.paintOrder,
      textHash: node.textHash,
      textLayoutHash: node.textLayoutHash,
    } as JsonObject;
  });
  const components = [
    ...ensureRootWindowBackdropNode(rawComponents, targetBounds, viewportRect),
    ...fidelityNodes,
  ];
  const nodes = components.map((component) => {
    const bounds = rectFrom(component.bounds);
    return {
      name: component.name ?? null,
      type: component.type ?? null,
      semanticId: typeof component.semanticId === "string" ? component.semanticId : null,
      measurementId: typeof component.measurementId === "string"
        ? component.measurementId
        : `layout:${String(component.name ?? "unknown")}`,
      role: String(component.geometryRole ?? "other") as GeometryRole,
      bounds,
      depth: component.depth ?? null,
      parent: component.parent ?? null,
      children: component.children ?? [],
      explanation: component.explanation ?? null,
      visualStyle: component.visualStyle ?? null,
      measurementProvenance: component.measurementProvenance ?? null,
      coordinateSpace: component.coordinateSpace ?? null,
      visibleBounds: component.visibleBounds ? rectFrom(component.visibleBounds) : null,
      clipBounds: component.clipBounds ? rectFrom(component.clipBounds) : null,
      measurementFrameGeneration: component.measurementFrameGeneration ?? null,
      hitMetrics: hitMetrics(component, bounds),
      clipped: clippedBy(bounds, viewportRect),
      raw: component,
    };
  });
  const modelNodes = nodes.filter(
    (node) => node.measurementProvenance !== "paint-time",
  );
  const renderedNodes = nodes.filter(
    (node) => node.measurementProvenance === "paint-time",
  );
  // Model and completed-frame paint are audited independently. A passing model
  // can no longer hide clipped paint, and a passing paint layer cannot erase a
  // stale or contradictory model.
  const modelOverlaps = layerOverlaps(modelNodes);
  const renderedOverlaps = layerOverlaps(renderedNodes);
  const overlaps = modelOverlaps;
  const joins = buildMeasurementJoins(nodes);
  const maxBottom = modelNodes.reduce(
    (current, node) => Math.max(current, bottom(node.bounds)),
    0,
  );
  const clippedNodeCount = modelNodes.filter((node) => node.clipped).length;
  const renderedClippedNodeCount = renderedNodes.filter((node) =>
    evidenceIntersectionRatio(node.bounds, node.visibleBounds ?? node.bounds) < 0.999
  ).length;
  const overlapCount = modelOverlaps.length;
  const renderedOverlapCount = renderedOverlaps.length;
  const overflowY = maxBottom > viewportRect.height;
  const nodesWithVisualStyle = modelNodes.filter((node) => node.visualStyle != null);
  const controlsWithHitFailures = modelNodes.filter((node) => {
    const type = String(node.type ?? "");
    return (
      ["button", "input"].includes(type) &&
      !node.hitMetrics.minHitPass &&
      node.hitMetrics.exception == null
    );
  });
  const nativeMaterialSources = new Set([
    "NSGlassEffectView",
    "NSVisualEffectView",
    "nativeWindowBackdrop",
  ]);
  const contentNativeMaterialNodes = nodesWithVisualStyle.filter((node) => {
    const style = node.visualStyle as JsonObject;
    const materialSource = String(style.materialSource ?? "");
    return (
      style.chromeLayer === "content" &&
      nativeMaterialSources.has(materialSource)
    );
  });
  const glassLayerViolations = contentNativeMaterialNodes.map((node) => ({
    name: node.name,
    chromeLayer: (node.visualStyle as JsonObject).chromeLayer ?? null,
    materialSource: (node.visualStyle as JsonObject).materialSource ?? null,
  }));
  const buttonCenterDistance = buttonCenterDistanceAssertions(modelNodes);
  const hardcodedColorNodes = nodesWithVisualStyle.filter((node) => {
    const style = node.visualStyle as JsonObject;
    return style.usesSemanticThemeToken === false || style.colorSource === "hardcoded";
  });
  const cornerRadiusFailures = nodesWithVisualStyle.filter((node) => {
    if (!isRadiusBearingNode(node)) return false;
    const style = node.visualStyle as JsonObject;
    return !hasPositiveRadius(style.cornerRadius) && !hasPositiveRadius(style.radius);
  });
  // Apple-documented numeric conformance: concentric-radius, control padding,
  // and capsule-radius deviations against Apple's published FORMULAS/constants
  // (provenance-tagged in apple-guideline-constants.ts). Replaces the previous
  // null radius-constants placeholder with real per-node deviation math.
  const backingScaleFactor =
    typeof info.backingScaleFactor === "number" ? info.backingScaleFactor : null;
  const appleConformance = appleGuidelineConformance(
    modelNodes as unknown as NodeLike[],
    backingScaleFactor,
  );
  return {
    promptType: info.promptType ?? null,
    timestamp: info.timestamp ?? null,
    viewportRect,
    windowRect: targetBounds,
    regions: nodes.map((node) => ({
      name: node.name,
      type: node.type,
      bounds: node.bounds,
    })),
    nodes,
    overlaps,
    truthLayers: {
      model: {
        nodeCount: modelNodes.length,
        clippedNodeCount,
        overlapCount,
        overlaps: modelOverlaps,
      },
      rendered: {
        nodeCount: renderedNodes.length,
        clippedNodeCount: renderedClippedNodeCount,
        overlapCount: renderedOverlapCount,
        overlaps: renderedOverlaps,
      },
      joins,
      comparableJoinCount: joins.filter((join) => join.comparability === "Comparable").length,
      unjoinedMeasurementIds: joins
        .filter((join) => join.comparability === "ModelOnly" || join.comparability === "RenderedOnly")
        .map((join) => join.measurementId),
    },
    resizePressure: {
      windowCanGrow: null,
      overflowY,
      desiredContentHeight: maxBottom,
      measuredContentHeight: viewportRect.height,
      clippedNodeCount,
      overlapCount,
      pressureScore: clippedNodeCount + overlapCount + (overflowY ? 1 : 0),
    },
    visualAudit: {
      nodeCount: modelNodes.length,
      paintMeasurementNodeCount: nodes.length - modelNodes.length,
      styledNodeCount: nodesWithVisualStyle.length,
      unstyledNodeCount: modelNodes.length - nodesWithVisualStyle.length,
      controlsWithHitFailures: controlsWithHitFailures.map((node) => ({
        name: node.name,
        type: node.type,
        hitMetrics: node.hitMetrics,
      })),
      contentGlassNodes: contentNativeMaterialNodes.map((node) => node.name),
      contentNativeMaterialNodes: contentNativeMaterialNodes.map((node) => node.name),
      glassLayerViolations,
      missingStyleNodeNames: modelNodes
        .filter((node) => node.visualStyle == null)
        .map((node) => node.name),
      chromeLayers: Object.fromEntries(
        Object.entries(
          nodesWithVisualStyle.reduce<Record<string, number>>((acc, node) => {
            const layer = String(
              (node.visualStyle as JsonObject).chromeLayer ?? "unknown",
            );
            acc[layer] = (acc[layer] ?? 0) + 1;
            return acc;
          }, {}),
        ).sort(([a], [b]) => a.localeCompare(b)),
      ),
      guidelineAssertions: {
        appleDocumented: {
          hitTargets: {
            source: "apple-documented",
            macosMinimumHitSize: { width: 28, height: 28 },
            macosMinimumVisualSize: { width: 20, height: 20 },
            failures: controlsWithHitFailures.map((node) => ({
              name: node.name,
              type: node.type,
              hitMetrics: node.hitMetrics,
            })),
          },
          buttonCenterDistance,
          materialLayering: {
            source: "apple-documented",
            contentGlassNodes: contentNativeMaterialNodes.map((node) => node.name),
            contentNativeMaterialNodes: contentNativeMaterialNodes.map((node) => node.name),
            glassLayerViolations,
          },
          colorAdaptivity: {
            source: "apple-documented",
            hardcodedColorNodes: hardcodedColorNodes.map((node) => node.name),
          },
          safeAreaLayout: {
            source: "apple-documented",
            clippedNodeCount,
            overflowY,
          },
          // Apple does NOT publish a width-based "exact panel radius constant";
          // window radius is style-dependent / system-owned. We encode Apple's
          // documented FORMULAS (capsule = h/2, concentric child = parent − inset)
          // and compute per-node deviations instead. Exact window radius resolves
          // via a native baseline probe (metric macos.window.toolbarRadius.nativeBaseline, roadmap).
          cornerGeometry: {
            source: "apple-documented-and-derived",
            backingScaleFactor: appleConformance.backingScaleFactor,
            constants: appleConformance.constants.filter((m) =>
              m.category === "cornerRadius" || m.category === "concentricity",
            ),
            deviations: appleConformance.deviations.filter((d) =>
              d.metricId.startsWith("shape.") || d.metricId.startsWith("macos.window."),
            ),
            failures: appleConformance.failures.filter((d) => d.metricId.startsWith("shape.")),
            nearMisses: appleConformance.nearMisses.filter((d) => d.metricId.startsWith("shape.")),
          },
          padding: {
            source: "apple-documented-and-derived",
            constants: appleConformance.constants.filter((m) => m.category === "padding"),
            deviations: appleConformance.deviations.filter((d) => {
              const m = appleConformance.constants.find((c) => c.id === d.metricId);
              return m?.category === "padding";
            }),
            unmeasured: appleConformance.unmeasured,
            failures: appleConformance.failures.filter((d) => {
              const m = appleConformance.constants.find((c) => c.id === d.metricId);
              return m?.category === "padding";
            }),
          },
          spacing: {
            source: "apple-documented",
            constants: appleConformance.constants.filter((m) => m.category === "spacing"),
            buttonCenterDistance,
          },
          typography: {
            source: "apple-documented-and-measured-native",
            constants: appleConformance.constants.filter((m) => m.category === "typography"),
            deviations: appleConformance.deviations.filter((d) => d.metricId.startsWith("typography.")),
            unmeasured: appleConformance.unmeasured.filter((d) => d.metricId.startsWith("typography.")),
            failures: appleConformance.failures.filter((d) => d.metricId.startsWith("typography.")),
            nearMisses: appleConformance.nearMisses.filter((d) => d.metricId.startsWith("typography.")),
          },
          conformanceScore: appleConformance.score,
        },
        projectLocal: {
          // DEMOTED to a smoke test: "every radius-bearing surface has SOME
          // positive radius". Numeric Apple alignment now lives in
          // appleDocumented.cornerGeometry above (concentric deviation math).
          cornerRadiusTokens: {
            source: "project-local",
            note: "smoke-only: positive-radius presence; numeric Apple comparison is appleDocumented.cornerGeometry",
            failures: cornerRadiusFailures.map((node) => node.name),
          },
          paddingTokens: {
            source: "project-local",
            minimumPanelPadding: 16,
            minimumCompactRowHorizontalPadding: 10,
          },
          spacingTokens: {
            source: "project-local",
            minimumFooterActionGap: 8,
          },
          windowBackdropPolicy: {
            source: "project-local",
            contentGlassNodeCount: contentNativeMaterialNodes.length,
          },
          themeTokenUsage: {
            source: "project-local",
            hardcodedColorNodes: hardcodedColorNodes.map((node) => node.name),
          },
          renderReadbackPixelThresholds: {
            source: "project-local",
            minimumNonBlackRatio: 0.01,
          },
        },
      },
    },
  };
}

function classify(
  targetReceipt: JsonObject,
  layoutEnvelope: JsonObject,
  analysis: ReturnType<typeof analyzeLayout>,
  proofMode: "inspection" | "join",
) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopeError(layoutEnvelope);
  if (transport !== "ok") {
    return transport;
  }
  if (analysis.nodes.length === 0) {
    return "blocked-by-missing-primitive";
  }
  if (proofMode === "join") {
    if (analysis.truthLayers.rendered.nodeCount === 0 || analysis.truthLayers.comparableJoinCount === 0) {
      return "blocked-by-missing-primitive";
    }
    const comparableJoinsAgree = analysis.truthLayers.joins
      .filter((join) => join.comparability === "Comparable")
      .every((join) => join.classification === "Match");
    if (
      analysis.truthLayers.model.clippedNodeCount > 0 ||
      analysis.truthLayers.model.overlapCount > 0 ||
      analysis.truthLayers.rendered.clippedNodeCount > 0 ||
      analysis.truthLayers.rendered.overlapCount > 0 ||
      !comparableJoinsAgree
    ) {
      return "not-ok";
    }
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
    extras: { "--include": "string", "--limit": "number", "--proof-mode": "string" },
  });
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  const include = extras["--include"]
    ? String(extras["--include"]).split(",").map((part) => part.trim()).filter(Boolean)
    : ["nodes", "regions", "scroll", "anchors", "resize", "overlaps"];
  const limit = extras["--limit"] ?? 200;
  const proofMode = extras["--proof-mode"] === "join" ? "join" : "inspection";

  const clock = startClock();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "layout" });
  const selector = (targetReceipt.requestedTarget as JsonObject | undefined)
    ?.selector ??
    args.target ?? { type: "focused" };
  const layoutEnvelope = await rpc(
    args.session,
    {
      type: "getLayoutInfo",
      requestId: requestId("layout", "measure"),
      target: selector,
      options: {
        include,
        limit,
      },
    },
    "layoutInfoResult",
    args.timeoutMs,
  );
  const layout = responseOf(layoutEnvelope);
  const analysis = analyzeLayout(layout, targetReceipt);
  const classification = classify(targetReceipt, layoutEnvelope, analysis, proofMode);

  emitValidatedReceipt(
    "devtools.layout.measure",
    finishReceipt(
      { tool: "script-kit-devtools.layout", command: "layout.measure", session: args.session, clock },
      {
        classification,
        proofMode,
        include,
        limit,
        requestedTarget: targetReceipt.requestedTarget ?? { selector },
        target: targetReceipt.resolvedTarget ?? null,
        transaction: targetReceipt.transaction,
        layoutEvidenceFreshness: {
          schemaVersion: 1,
          generatedAt: new Date().toISOString(),
          auditSchema: "panel-radius-v2",
          requiredPanelRadiusNodes: ["MainViewMain", "ScriptList", "PreviewPanel"],
          sourceFingerprint: await layoutSourceFingerprint(),
        },
        promptType: analysis.promptType,
        timestamp: analysis.timestamp,
        componentCount: analysis.nodes.length,
        window: {
          rect: analysis.windowRect,
          visible:
            (targetReceipt.resolvedTarget as JsonObject | undefined)?.visible ??
            null,
          focused:
            (targetReceipt.resolvedTarget as JsonObject | undefined)?.focused ??
            null,
        },
        viewport: {
          clientWidth: analysis.viewportRect.width,
          clientHeight: analysis.viewportRect.height,
          contentWidth: analysis.viewportRect.width,
          contentHeight: analysis.resizePressure.desiredContentHeight,
          scrollWidth: analysis.viewportRect.width,
          scrollHeight: analysis.resizePressure.desiredContentHeight,
          canScrollX: false,
          canScrollY: analysis.resizePressure.overflowY,
          scrollTop: null,
          maxScrollTop: Math.max(
            0,
            analysis.resizePressure.desiredContentHeight -
              analysis.viewportRect.height,
          ),
          overflowPolicyY: analysis.resizePressure.overflowY
            ? "auto"
            : "hidden",
        },
        pressure: {
          overflowY: analysis.resizePressure.overflowY,
          hiddenContentHeight: Math.max(
            0,
            analysis.resizePressure.desiredContentHeight -
              analysis.viewportRect.height,
          ),
          clippedNodeCount: analysis.resizePressure.clippedNodeCount,
          overlapCount: analysis.resizePressure.overlapCount,
          footerOverlapCount: analysis.overlaps.filter(
            (entry) =>
              String(entry.a).includes("Footer") ||
              String(entry.b).includes("Footer"),
          ).length,
          inputOverlapCount: analysis.overlaps.filter(
            (entry) =>
              String(entry.a).includes("Input") ||
              String(entry.b).includes("Input"),
          ).length,
          pressureScore: analysis.resizePressure.pressureScore,
        },
        viewportRect: analysis.viewportRect,
        windowRect: analysis.windowRect,
        regions: analysis.regions,
        nodes: analysis.nodes.map((node) => ({
          ...node,
          raw: diagnostic(node.raw),
        })),
        overlaps: analysis.overlaps,
        truthLayers: analysis.truthLayers,
        resizePressure: analysis.resizePressure,
        visualAudit: analysis.visualAudit,
        handlerForm:
          (layout.info as JsonObject | undefined)?.handlerForm ??
          layout.handlerForm ??
          null,
        missingPrimitives: [
          analysis.nodes.length === 0 ? "layoutComponents" : "",
          proofMode === "join" && analysis.truthLayers.rendered.nodeCount === 0
            ? "renderedMeasurements"
            : "",
          proofMode === "join" && analysis.truthLayers.comparableJoinCount === 0
            ? "comparableMeasurementJoin"
            : "",
          layoutEnvelope.status === "error" ? "layoutInfoResult" : "",
          targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
        ].filter(Boolean),
        warnings: [
          ...argWarnings,
          analysis.resizePressure.overflowY
            ? "content exceeds measured viewport height"
            : "",
          analysis.resizePressure.overlapCount > 0
            ? "layout components overlap"
            : "",
        ].filter(Boolean),
        errors: diagnostic([
          ...((targetReceipt.errors as JsonObject[]) ?? []),
          ...[layoutEnvelope].filter((value) => value.status === "error"),
        ]),
      },
    ),
  );
}

if (import.meta.main) await main();
