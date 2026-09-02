import { expect, test } from "bun:test";
import { deflateSync } from "node:zlib";
import { createHash } from "node:crypto";
import { auditRgbaPng, hashPngRegion, visitPngRgbaRows } from "./lib/png-rgba.ts";
import { selectedThemePixelRegion } from "./design.ts";
import type { Json } from "./driver.ts";

const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
function chunk(type: string, data: Uint8Array): Buffer {
  const result = Buffer.alloc(data.length + 12);
  result.writeUInt32BE(data.length, 0);
  result.write(type, 4, 4, "ascii");
  result.set(data, 8);
  let crc = 0xffffffff;
  for (const byte of result.subarray(4, result.length - 4)) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
  }
  result.writeUInt32BE((crc ^ 0xffffffff) >>> 0, result.length - 4);
  return result;
}
function png(raw: readonly number[], width: number, height: number, colorType = 6, headerOverrides: Record<number, number> = {}): Buffer {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4);
  header[8] = 8; header[9] = colorType;
  for (const [index, value] of Object.entries(headerOverrides)) header[Number(index)] = value;
  return Buffer.concat([signature, chunk("IHDR", header), chunk("IDAT", deflateSync(Buffer.from(raw))), chunk("IEND", Buffer.alloc(0))]);
}

test("RGB and RGBA decode all five scanline filters into identical straight-alpha rows", () => {
  const rgba = [10, 20, 30, 255, 50, 60, 70, 128];
  const rgbaWire = [0, ...rgba, 1, 10, 20, 30, 255, 40, 40, 40, 129,
    2, 0, 0, 0, 0, 0, 0, 0, 0, 3, 5, 10, 15, 128, 20, 20, 20, 193,
    4, 0, 0, 0, 0, 0, 0, 0, 0];
  const rgbWire = [0, 10, 20, 30, 50, 60, 70, 1, 10, 20, 30, 40, 40, 40,
    2, 0, 0, 0, 0, 0, 0, 3, 5, 10, 15, 20, 20, 20, 4, 0, 0, 0, 0, 0, 0];
  for (const [colorType, wire, expected] of [[6, rgbaWire, rgba], [2, rgbWire, [10, 20, 30, 255, 50, 60, 70, 255]]] as const) {
    const seen: number[] = [];
    const dimensions = visitPngRgbaRows(png(wire, 2, 5, colorType), (row, y) => { expect([...row]).toEqual([...expected]); seen.push(y); }, { width: 2, height: 5 });
    expect(dimensions).toEqual({ width: 2, height: 5 });
    expect(seen).toEqual([0, 1, 2, 3, 4]);
  }
});

test("Paeth chooses upper-left rather than substituting left or up", () => {
  const rows: number[][] = [];
  visitPngRgbaRows(png([0, 65, 65, 65, 80, 80, 80, 4, 241, 241, 241, 251, 251, 251], 2, 2, 2), row => rows.push([...row]));
  expect(rows[1]).toEqual([50, 50, 50, 255, 60, 60, 60, 255]);
});

test("audit preserves opacity, dark threshold, luma and quantized bucket statistics", () => {
  const audit = auditRgbaPng(png([0, 9, 0, 0, 0, 0, 0, 8, 255, 9, 0, 0, 255], 3, 1));
  expect(audit).toEqual({ sampledPixels: 3, nonBlackPixels: 1, nonTransparentPixels: 2,
    uniqueBucketCount: 2, meanLuma: (0.2126 * 9 + 0.0722 * 8 + 0.2126 * 9) / 3,
    maxLuma: 0.2126 * 9, nonBlackRatio: 1 / 3, blank: false });
  const rgb = auditRgbaPng(png([0, 0, 0, 0, 255, 255, 255], 2, 1, 2));
  expect(rgb.nonTransparentPixels).toBe(2); expect(rgb.uniqueBucketCount).toBe(2);
  expect(rgb.meanLuma).toBeCloseTo(127.5); expect(rgb.maxLuma).toBeCloseTo(255); expect(rgb.blank).toBe(false);
  expect(auditRgbaPng(png([0, 255, 0, 0, 0, 0, 255, 0, 0], 2, 1)).blank).toBe(true);
  expect(auditRgbaPng(png([0, 255, 255, 255, 255, 255, 255, 255, 255], 2, 1)).blank).toBe(true);
  const sparse = [0, ...Array.from({ length: 2000 }, (_, x) => x === 0 ? [255, 255, 255, 255] : [0, 0, 0, 255]).flat()];
  expect(auditRgbaPng(png(sparse, 2000, 1)).blank).toBe(true);
});

test("decode rejects unsupported, malformed, truncated and over-inflated data", () => {
  const good = png([0, 10, 20, 30, 255], 1, 1);
  const corrupt = Buffer.from(good); corrupt[corrupt.length - 1] ^= 1;
  for (const bytes of [Buffer.alloc(20), good.subarray(0, good.length - 1), good.subarray(0, good.length - 12), corrupt,
    png([0, 10, 20, 30, 255], 0, 1), png([0, 10, 20, 30, 255], 1, 1, 6, { 8: 16 }),
    png([0, 10, 20, 30, 255], 1, 1, 3), png([0, 10, 20, 30, 255], 1, 1, 6, { 10: 1 }),
    png([0, 10, 20, 30, 255], 1, 1, 6, { 11: 1 }), png([0, 10, 20, 30, 255], 1, 1, 6, { 12: 1 }),
    png([5, 10, 20, 30, 255], 1, 1), png([0, 10, 20], 1, 1), png([0, 10, 20, 30, 255, 99], 1, 1)]) {
    expect(() => visitPngRgbaRows(bytes, () => {})).toThrow();
  }
  const oversized = Buffer.from(good); oversized.writeUInt32BE(0xffffffff, 8);
  expect(() => visitPngRgbaRows(oversized, () => {})).toThrow("Invalid PNG chunk");
});

test("declared dimensions reject forged PNG bounds before inflation or visiting", () => {
  const forged = png([], 0x7fffffff, 1);
  let visits = 0;
  expect(() => visitPngRgbaRows(forged, () => { visits++; }, { width: 750, height: 520 })).toThrow("dimensions differ");
  for (const dimensions of [{ width: 0, height: 1 }, { width: NaN, height: 1 }, { width: 1.5, height: 1 }]) {
    expect(() => visitPngRgbaRows(forged, () => { visits++; }, dimensions)).toThrow("Invalid declared");
  }
  expect(visits).toBe(0);
});

test("multiple IDAT chunks decode, but transparency, duplicate header and nonconsecutive IDAT fail closed", () => {
  const original = png([0, 10, 20, 30, 255], 1, 1);
  const header = original.subarray(8, 33);
  const compressed = deflateSync(Buffer.from([0, 10, 20, 30, 255]));
  const first = chunk("IDAT", compressed.subarray(0, 4));
  const last = chunk("IDAT", compressed.subarray(4));
  const end = chunk("IEND", Buffer.alloc(0));
  const split = Buffer.concat([signature, header, first, last, end]);
  expect(auditRgbaPng(split)).toEqual(auditRgbaPng(original));
  for (const parts of [[header, header, first, last], [header, first, chunk("tEXt", Buffer.from("name\0value")), last],
    [header, chunk("tRNS", Buffer.alloc(6)), first, last], [header, chunk("BADX", Buffer.alloc(0)), first, last]]) {
    expect(() => visitPngRgbaRows(Buffer.concat([signature, ...parts, end]), () => {})).toThrow();
  }
});

test("region hash consumes only declared RGBA pixels and changes/restores with their content", () => {
  const dimensions = { width: 3, height: 2 }; const region = { x: 1, y: 1, width: 1, height: 1 };
  const baseline = [0, ...Array(12).fill(255), 0, ...Array(12).fill(255)];
  const outsideEdit = [...baseline]; outsideEdit[1] = 0;
  const insideEdit = [...baseline]; insideEdit[18] = 0;
  const base = hashPngRegion(png(baseline, 3, 2), dimensions, region);
  expect(base).toEqual({ sha256: createHash("sha256").update(Buffer.from([255, 255, 255, 255])).digest("hex"), sampledPixels: 1, opaquePixels: 1 });
  expect(hashPngRegion(png(outsideEdit, 3, 2), dimensions, region)).toEqual(base);
  expect(hashPngRegion(png(insideEdit, 3, 2), dimensions, region).sha256).not.toBe(base.sha256);
  expect(hashPngRegion(png(baseline, 3, 2), dimensions, region)).toEqual(base);
  for (const invalid of [{ ...region, x: -1 }, { ...region, y: 2 }, { ...region, width: 3 }, { ...region, width: 0 }, { ...region, x: 0.5 }])
    expect(() => hashPngRegion(png(baseline, 3, 2), dimensions, invalid)).toThrow("Invalid PNG pixel region");
});

function selectedFixture() {
  const state: Json = { mainListScroll: { selectedSemanticId: "script:fixture", selectedRowVisible: true, selectedRowTop: 84, selectedRowBottom: 128 } };
  const rowBounds = { x: 20, y: 84, width: 350, height: 44 };
  const markerBounds = { x: 34, y: 98, width: 2, height: 16 };
  const layout: Json = { windowWidth: 750, windowHeight: 520, components: [
    { name: "list-row:script:fixture", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 7,
      bounds: rowBounds, visibleBounds: rowBounds, clipBounds: rowBounds },
    { name: "script:fixture:selection-marker", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 7,
      bounds: markerBounds, visibleBounds: markerBounds, clipBounds: rowBounds },
  ] };
  return { state, layout };
}

test("selected marker region maps actual window-space paint into capture pixels, not list-relative offsets", () => {
  const { state, layout } = selectedFixture();
  const selected = selectedThemePixelRegion(state, layout, { width: 750, height: 520 }, 7);
  expect(selected.region).toEqual({ x: 34, y: 102, width: 2, height: 8 });
  const hiDpi = selectedThemePixelRegion(state, layout, { width: 1500, height: 1040 }, 7);
  expect(hiDpi.region).toEqual({ x: 68, y: 204, width: 4, height: 16 });
  // Local paint coordinates are authoritative even when a list has a window-space origin.
  state.mainListScroll.selectedRowTop = 0; state.mainListScroll.selectedRowBottom = 44;
  expect(selectedThemePixelRegion(state, layout, { width: 750, height: 520 }, 7)).toEqual(selected);
  layout.components[1].bounds = { x: 34.25, y: 98.25, width: 2, height: 16 };
  layout.components[1].visibleBounds = layout.components[1].bounds;
  expect(selectedThemePixelRegion(state, layout, { width: 1500, height: 1040 }, 7).region)
    .toEqual({ x: 69, y: 205, width: 3, height: 15 });
});

test("selected marker proof rejects stale, ambiguous, clipped, hidden and invalid geometry", () => {
  const mutations: Array<(state: Json, layout: Json) => void> = [
    state => { state.mainListScroll.selectedRowVisible = false; },
    state => { state.mainListScroll.selectedSemanticId = "script:other"; },
    (_state, layout) => { layout.components[1].measurementFrameGeneration = 6; },
    (_state, layout) => { layout.components[1].measurementProvenance = "model"; },
    (_state, layout) => { layout.components[1].coordinateSpace = "screen"; },
    (_state, layout) => { layout.components.push(layout.components[1]); },
    (_state, layout) => { layout.components[1].visibleBounds = { x: 34, y: 98, width: 1, height: 16 }; },
    (_state, layout) => { layout.components[1].bounds = { x: 400, y: 98, width: 2, height: 16 }; layout.components[1].visibleBounds = layout.components[1].bounds; },
    (_state, layout) => { layout.components[1].bounds.width = NaN; },
    (_state, layout) => { layout.windowWidth = 0; },
  ];
  for (const mutate of mutations) {
    const { state, layout } = selectedFixture(); mutate(state, layout);
    expect(() => selectedThemePixelRegion(state, layout, { width: 750, height: 520 }, 7)).toThrow();
  }
  const { state, layout } = selectedFixture();
  for (const dimensions of [{ width: 0, height: 520 }, { width: 750.5, height: 520 }, { width: 4096, height: 4096 }])
    expect(() => selectedThemePixelRegion(state, layout, dimensions, 7)).toThrow("theme_pixel_dimensions_invalid");
});
