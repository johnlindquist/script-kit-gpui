import { describe, expect, test } from "bun:test";
import { analyzeLayout, buildMeasurementJoins } from "./layout.ts";

const bounds = { x: 0, y: 0, width: 100, height: 20 };

function node(overrides: Record<string, unknown> = {}) {
  return {
    measurementId: "layout:row",
    semanticId: "row:1",
    role: "rowSlot" as const,
    bounds,
    visibleBounds: bounds,
    clipBounds: bounds,
    measurementProvenance: "model",
    coordinateSpace: "window",
    measurementFrameGeneration: 7,
    ...overrides,
  };
}

describe("model/rendered measurement joins", () => {
  test("joins equal roles in one coordinate space and frame", () => {
    const joins = buildMeasurementJoins([
      node(),
      node({ measurementProvenance: "paint-time" }),
    ]);
    expect(joins).toHaveLength(1);
    expect(joins[0].comparability).toBe("Comparable");
    expect(joins[0].classification).toBe("Match");
    expect(joins[0].intended?.contractId).toBe("geometry-role:rowSlot");
  });

  test("rendered clipping remains visible when the model fits", () => {
    const joins = buildMeasurementJoins([
      node(),
      node({
        measurementProvenance: "paint-time",
        visibleBounds: { x: 0, y: 0, width: 99, height: 20 },
      }),
    ]);
    expect(joins[0].comparability).toBe("Comparable");
    expect(joins[0].classification).toBe("Clipped");
  });

  test("unlike roles and stale frames are never compared", () => {
    const roleMismatch = buildMeasurementJoins([
      node(),
      node({ measurementProvenance: "paint-time", role: "footerNativeHost" }),
    ])[0];
    const stale = buildMeasurementJoins([
      node(),
      node({ measurementProvenance: "paint-time", measurementFrameGeneration: 8 }),
    ])[0];
    expect(roleMismatch.comparability).toBe("RoleMismatch");
    expect(roleMismatch.classification).toBe("NotComparable");
    expect(stale.comparability).toBe("StaleGeneration");
    expect(stale.classification).toBe("NotComparable");
  });

  test("duplicate paint measurements cannot hide a later drifting or clipped renderer", () => {
    const duplicate = buildMeasurementJoins([
      node(),
      node({ measurementProvenance: "paint-time" }),
      node({
        measurementProvenance: "paint-time",
        bounds: { ...bounds, x: 50 },
      }),
    ])[0];

    expect(duplicate.comparability).toBe("DuplicateMeasurement");
    expect(duplicate.classification).toBe("NotComparable");
  });

  test("different semantic owners and missing coordinate spaces are never comparable", () => {
    const wrongOwner = buildMeasurementJoins([
      node(),
      node({ measurementProvenance: "paint-time", semanticId: "row:other" }),
    ])[0];
    const missingSpace = buildMeasurementJoins([
      node({ coordinateSpace: undefined }),
      node({ measurementProvenance: "paint-time", coordinateSpace: undefined }),
    ])[0];

    expect(wrongOwner.comparability).toBe("SemanticMismatch");
    expect(missingSpace.comparability).toBe("CoordinateSpaceMismatch");
    expect(wrongOwner.classification).toBe("NotComparable");
    expect(missingSpace.classification).toBe("NotComparable");
  });

  test("missing, zero-area, negative, and non-finite geometry cannot produce a Match", () => {
    for (const rendered of [
      node({ measurementProvenance: "paint-time", bounds: { ...bounds, x: NaN } }),
      node({ measurementProvenance: "paint-time", bounds: { ...bounds, width: Infinity } }),
      node({ measurementProvenance: "paint-time", bounds: { ...bounds, width: 0 } }),
      node({ measurementProvenance: "paint-time", bounds: { ...bounds, height: -1 } }),
      node({ measurementProvenance: "paint-time", visibleBounds: null }),
      node({ measurementProvenance: "paint-time", clipBounds: null }),
    ]) {
      const joined = buildMeasurementJoins([node(), rendered])[0];
      expect(joined.comparability).toBe("InvalidGeometry");
      expect(joined.classification).toBe("NotComparable");
    }
  });

  test("clip bounds remain authoritative even when a producer falsely reports full visibility", () => {
    const joined = buildMeasurementJoins([
      node(),
      node({
        measurementProvenance: "paint-time",
        visibleBounds: bounds,
        clipBounds: { ...bounds, width: 99 },
      }),
    ])[0];

    expect(joined.comparability).toBe("Comparable");
    expect(joined.classification).toBe("Clipped");
  });

  test("unknown provenance and untyped roles cannot impersonate model truth", () => {
    const unknown = buildMeasurementJoins([
      node({ measurementProvenance: "fabricated-observer" }),
      node({ measurementProvenance: "paint-time" }),
    ])[0];
    const untyped = buildMeasurementJoins([
      node({ role: "other" }),
      node({ measurementProvenance: "paint-time", role: "other" }),
    ])[0];

    expect(unknown.comparability).toBe("InvalidProvenance");
    expect(untyped.comparability).toBe("RoleMismatch");
  });

  test("rendered overlaps are audited independently of the model", () => {
    const layout = {
      windowWidth: 200,
      windowHeight: 100,
      components: [
        { name: "ModelA", type: "row", measurementId: "layout:a", geometryRole: "rowSlot", bounds: { x: 0, y: 0, width: 80, height: 20 }, depth: 1, parent: "root", measurementProvenance: "model", coordinateSpace: "window", measurementFrameGeneration: 4 },
        { name: "ModelB", type: "row", measurementId: "layout:b", geometryRole: "rowSlot", bounds: { x: 100, y: 0, width: 80, height: 20 }, depth: 1, parent: "root", measurementProvenance: "model", coordinateSpace: "window", measurementFrameGeneration: 4 },
        { name: "PaintA", type: "row", measurementId: "layout:a", geometryRole: "rowSlot", bounds: { x: 0, y: 0, width: 120, height: 20 }, visibleBounds: { x: 0, y: 0, width: 120, height: 20 }, clipBounds: { x: 0, y: 0, width: 200, height: 100 }, depth: 1, parent: "root", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 4 },
        { name: "PaintB", type: "row", measurementId: "layout:b", geometryRole: "rowSlot", bounds: { x: 100, y: 0, width: 80, height: 20 }, visibleBounds: { x: 100, y: 0, width: 80, height: 20 }, clipBounds: { x: 0, y: 0, width: 200, height: 100 }, depth: 1, parent: "root", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 4 },
      ],
    };
    const analysis = analyzeLayout(layout, { resolvedTarget: { bounds: { x: 0, y: 0, width: 200, height: 100 } } });
    expect(analysis.truthLayers.model.overlapCount).toBe(0);
    expect(analysis.truthLayers.rendered.overlapCount).toBe(1);
  });

  test("model overlaps remain visible when completed paint fits", () => {
    const layout = {
      windowWidth: 200,
      windowHeight: 100,
      components: [
        { name: "ModelA", type: "row", measurementId: "layout:a", geometryRole: "rowSlot", bounds: { x: 0, y: 0, width: 120, height: 20 }, depth: 1, parent: "root", measurementProvenance: "model", coordinateSpace: "window", measurementFrameGeneration: 4 },
        { name: "ModelB", type: "row", measurementId: "layout:b", geometryRole: "rowSlot", bounds: { x: 100, y: 0, width: 80, height: 20 }, depth: 1, parent: "root", measurementProvenance: "model", coordinateSpace: "window", measurementFrameGeneration: 4 },
        { name: "PaintA", type: "row", measurementId: "layout:a", geometryRole: "rowSlot", bounds: { x: 0, y: 0, width: 80, height: 20 }, visibleBounds: { x: 0, y: 0, width: 80, height: 20 }, clipBounds: { x: 0, y: 0, width: 200, height: 100 }, depth: 1, parent: "root", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 4 },
        { name: "PaintB", type: "row", measurementId: "layout:b", geometryRole: "rowSlot", bounds: { x: 100, y: 0, width: 80, height: 20 }, visibleBounds: { x: 100, y: 0, width: 80, height: 20 }, clipBounds: { x: 0, y: 0, width: 200, height: 100 }, depth: 1, parent: "root", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 4 },
      ],
    };
    const analysis = analyzeLayout(layout, { resolvedTarget: { bounds: { x: 0, y: 0, width: 200, height: 100 } } });
    expect(analysis.truthLayers.model.overlapCount).toBe(1);
    expect(analysis.truthLayers.rendered.overlapCount).toBe(0);
  });
});
