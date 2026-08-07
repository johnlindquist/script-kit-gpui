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

  test("fit classification requires measurements from one completed frame", () => {
    const rows = textRows([{ semanticId: "input:notes-editor", content: { kind: "userContent", charLength: 7, byteLength: 7, lineCount: 1, fingerprint: "f", rawContentReturned: false } }]);
    const fit = textFitMeasurements(layout([line]));
    const stale = textFitMeasurements(layout([{ ...line, measurementFrameGeneration: 11 }]));
    expect(classifyTextProof({ classification: "ok" }, { status: "ok" }, { status: "ok" }, { status: "ok" }, rows, fit, "fit")).toBe("ok");
    expect(classifyTextProof({ classification: "ok" }, { status: "ok" }, { status: "ok" }, { status: "ok" }, rows, stale, "fit")).toBe("not-ok");
  });
});
