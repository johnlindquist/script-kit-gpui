import { describe, expect, test } from "bun:test";
import { classifyTextProof, textFitMeasurements, textRows } from "./text.ts";

const line = {
  id: "notes-editor.line.0.0",
  kind: "textLine",
  bounds: { x: 10, y: 10, width: 180, height: 20 },
  visibleBounds: { x: 10, y: 10, width: 180, height: 20 },
  clipBounds: { x: 10, y: 10, width: 180, height: 20 },
  unionPaintBounds: { x: 10, y: 12, width: 80, height: 16 },
  paintOrder: 0,
  measurementFrameGeneration: 12,
  textHash: "fixture-fingerprint",
  metadata: {
    measurementId: "text:notes-editor:line:0:0",
    semanticId: "input:notes-editor",
    role: "textLineBox",
    fontFamilyFingerprint: "font-fingerprint",
    fontSize: 14,
    fontWeight: "mixedOrRendererOwned",
    lineHeight: 20,
    backingScaleFactor: 2,
    fontsReady: true,
    wrappingPolicy: "none",
    truncationPolicy: "fullDisplay",
    contentKind: "userContent",
    graphemeCount: 7,
    lineCount: 1,
    rawContentReturned: false,
  },
};

function layout(nodes: unknown[]) {
  return { fidelity: { frameGeneration: 12, nodes } };
}

describe("privacy-safe shaped text fit", () => {
  test("complete shaped line proves full display without raw text", () => {
    const fits = textFitMeasurements(layout([line]));
    expect(fits).toHaveLength(1);
    expect(fits[0].fullDisplayPass).toBe(true);
    expect(fits[0].visibleRatio).toBe(1);
    expect(fits[0].contentFingerprint).toBe("fixture-fingerprint");
    expect(JSON.stringify(fits)).not.toContain("authored fixture text");
  });

  test("one-point glyph clip and fonts-not-ready both fail closed", () => {
    const clipped = textFitMeasurements(layout([{ ...line, clipBounds: { x: 10, y: 12, width: 79, height: 16 } }]))[0];
    const fontsPending = textFitMeasurements(layout([{ ...line, metadata: { ...line.metadata, fontsReady: false } }]))[0];
    const wrongScale = textFitMeasurements(layout([line]), 1)[0];
    expect(clipped.visibleRatio).toBeLessThan(1);
    expect(clipped.fullDisplayPass).toBe(false);
    expect(fontsPending.fullDisplayPass).toBe(false);
    expect(wrongScale.backingScaleMatches).toBe(false);
    expect(wrongScale.fullDisplayPass).toBe(false);
  });

  test("later intersecting paint is reported as an occluder", () => {
    const occluder = {
      id: "native-footer",
      kind: "element",
      unionPaintBounds: { x: 0, y: 20, width: 200, height: 20 },
      paintOrder: 1,
      metadata: { measurementId: "layout:native-footer" },
    };
    const fit = textFitMeasurements(layout([line, occluder]))[0];
    expect(fit.occluderMeasurementIds).toEqual(["layout:native-footer"]);
    expect(fit.fullDisplayPass).toBe(false);
  });

  test("missing, zero-area, non-finite, and negative glyph geometry never proves display", () => {
    for (const unionPaintBounds of [
      undefined,
      { x: 10, y: 12, width: 0, height: 0 },
      { x: 10, y: 12, width: NaN, height: 16 },
      { x: 10, y: 12, width: 80, height: -1 },
    ]) {
      const fit = textFitMeasurements(layout([{ ...line, unionPaintBounds }]), 2)[0];
      expect(fit.geometryValid).toBe(false);
      expect(fit.fullDisplayPass).toBe(false);
    }
  });

  test("raw private content cannot be labeled a passing full-display measurement", () => {
    const fit = textFitMeasurements(layout([{
      ...line,
      metadata: { ...line.metadata, rawContentReturned: true },
    }]), 2)[0];

    expect(fit.rawContentReturned).toBe(true);
    expect(fit.fullDisplayPass).toBe(false);
  });

  test("visible bounds and clip bounds independently constrain rendered glyphs", () => {
    const fit = textFitMeasurements(layout([{
      ...line,
      visibleBounds: { x: 10, y: 12, width: 40, height: 16 },
      clipBounds: { x: 10, y: 10, width: 180, height: 20 },
    }]), 2)[0];

    expect(fit.visibleRatio).toBe(0.5);
    expect(fit.fullDisplayPass).toBe(false);
  });

  test("real measurement identity, glyph fingerprint, fonts, and paint order are mandatory", () => {
    for (const candidate of [
      { ...line, textHash: null },
      { ...line, paintOrder: undefined },
      { ...line, metadata: { ...line.metadata, measurementId: null } },
      { ...line, metadata: { ...line.metadata, semanticId: null } },
      { ...line, metadata: { ...line.metadata, fontFamilyFingerprint: null } },
      { ...line, metadata: { ...line.metadata, fontSize: 0 } },
      { ...line, metadata: { ...line.metadata, graphemeCount: undefined } },
    ]) {
      const fit = textFitMeasurements(layout([candidate]), 2)[0];
      expect(fit.fullDisplayPass).toBe(false);
    }
  });

  test("fit classification requires measurements from one completed frame", () => {
    const rows = textRows([{ semanticId: "input:notes-editor", content: { kind: "userContent", charLength: 7, byteLength: 7, lineCount: 1, fingerprint: "f", rawContentReturned: false } }]);
    const fit = textFitMeasurements(layout([line]));
    const stale = textFitMeasurements(layout([{ ...line, measurementFrameGeneration: 11 }]));
    expect(classifyTextProof({ classification: "ok" }, { status: "ok" }, { status: "ok" }, { status: "ok" }, rows, fit, "fit")).toBe("ok");
    expect(classifyTextProof({ classification: "ok" }, { status: "ok" }, { status: "ok" }, { status: "ok" }, rows, stale, "fit")).toBe("not-ok");
  });
});
