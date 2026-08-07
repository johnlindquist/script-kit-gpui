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
    "  bun scripts/devtools/layout.ts measure [target args] [--include nodes,regions,scroll,anchors,resize,overlaps] [--limit <n>]",
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

function analyzeLayout(layout: JsonObject, targetReceipt: JsonObject) {
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
  const components = ensureRootWindowBackdropNode(rawComponents, targetBounds, viewportRect);
  const nodes = components.map((component) => {
    const bounds = rectFrom(component.bounds);
    return {
      name: component.name ?? null,
      type: component.type ?? null,
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
  const auditNodes = nodes.filter(
    (node) => node.measurementProvenance !== "paint-time",
  );
  const overlaps = [];
  for (let left = 0; left < auditNodes.length; left += 1) {
    for (
      let rightIndex = left + 1;
      rightIndex < auditNodes.length;
      rightIndex += 1
    ) {
      const a = auditNodes[left];
      const b = auditNodes[rightIndex];
      const sameSiblingBand = a.depth === b.depth && a.parent === b.parent;
      // A floating overlay (e.g. the hint-strip footer) intentionally floats over
      // content with safe-area/scroll-edge insets — the list viewport is reduced by
      // the footer height, so the geometric intersection is by design, not a clip.
      // (Oracle review tahoe-guideline-review-remaining; render_script_list reduces
      // safe_viewport_height by the footer overlay height.)
      const floatingOverlayPair =
        a.hitMetrics?.exception === "floatingFooterOverlay" ||
        b.hitMetrics?.exception === "floatingFooterOverlay";
      if (
        sameSiblingBand &&
        !floatingOverlayPair &&
        a.name &&
        b.name &&
        intersects(a.bounds, b.bounds)
      ) {
        overlaps.push({ a: a.name, b: b.name });
      }
    }
  }
  const maxBottom = auditNodes.reduce(
    (current, node) => Math.max(current, bottom(node.bounds)),
    0,
  );
  const clippedNodeCount = auditNodes.filter((node) => node.clipped).length;
  const overlapCount = overlaps.length;
  const overflowY = maxBottom > viewportRect.height;
  const nodesWithVisualStyle = auditNodes.filter((node) => node.visualStyle != null);
  const controlsWithHitFailures = auditNodes.filter((node) => {
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
  const buttonCenterDistance = buttonCenterDistanceAssertions(auditNodes);
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
    auditNodes as unknown as NodeLike[],
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
      nodeCount: auditNodes.length,
      paintMeasurementNodeCount: nodes.length - auditNodes.length,
      styledNodeCount: nodesWithVisualStyle.length,
      unstyledNodeCount: auditNodes.length - nodesWithVisualStyle.length,
      controlsWithHitFailures: controlsWithHitFailures.map((node) => ({
        name: node.name,
        type: node.type,
        hitMetrics: node.hitMetrics,
      })),
      contentGlassNodes: contentNativeMaterialNodes.map((node) => node.name),
      contentNativeMaterialNodes: contentNativeMaterialNodes.map((node) => node.name),
      glassLayerViolations,
      missingStyleNodeNames: auditNodes
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
    extras: { "--include": "string", "--limit": "number" },
  });
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  const include = extras["--include"]
    ? String(extras["--include"]).split(",").map((part) => part.trim()).filter(Boolean)
    : ["nodes", "regions", "scroll", "anchors", "resize", "overlaps"];
  const limit = extras["--limit"] ?? 200;

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
  const classification = classify(targetReceipt, layoutEnvelope, analysis);

  emitValidatedReceipt(
    "devtools.layout.measure",
    finishReceipt(
      { tool: "script-kit-devtools.layout", command: "layout.measure", session: args.session, clock },
      {
        classification,
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
        resizePressure: analysis.resizePressure,
        visualAudit: analysis.visualAudit,
        handlerForm:
          (layout.info as JsonObject | undefined)?.handlerForm ??
          layout.handlerForm ??
          null,
        missingPrimitives: [
          analysis.nodes.length === 0 ? "layoutComponents" : "",
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

await main();
