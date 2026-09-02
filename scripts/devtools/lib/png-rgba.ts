import { inflateSync } from "node:zlib";
import { createHash } from "node:crypto";

export interface PngDimensions { width: number; height: number }
export interface PixelRegion extends PngDimensions { x: number; y: number }
export interface ScreenshotContentAudit {
  sampledPixels: number;
  nonBlackPixels: number;
  nonTransparentPixels: number;
  uniqueBucketCount: number;
  meanLuma: number;
  maxLuma: number;
  nonBlackRatio: number;
  blank: boolean;
}

const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}
function paethPredictor(a: number, b: number, c: number): number {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}
function validDimensions(value: PngDimensions): boolean {
  return Number.isSafeInteger(value.width) && value.width > 0 &&
    Number.isSafeInteger(value.height) && value.height > 0 &&
    Number.isSafeInteger(value.width * value.height * 4 + value.height);
}

/** Visits top-down, straight-alpha RGBA rows. The read-only row is reused: consume it synchronously.
 * Only the bounded inflated scanlines and O(width) working rows are allocated, never an RGBA frame.
 * A visitor must discard partial results if decoding throws. Native callers supply exact capture dimensions.
 */
export function visitPngRgbaRows(bytes: Uint8Array, visit: (rgba: Uint8Array, y: number) => void,
  declared?: PngDimensions): PngDimensions {
  if (declared && !validDimensions(declared)) throw new Error("Invalid declared PNG dimensions");
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (!signature.every((value, index) => bytes[index] === value)) throw new Error("Screenshot is not a PNG file");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let offset = 8;
  let width = 0;
  let height = 0;
  let colorType = 0;
  let sawHeader = false;
  let sawEnd = false;
  let idatEnded = false;
  const idatParts: Uint8Array[] = [];
  while (offset < bytes.length) {
    if (bytes.length - offset < 12) throw new Error("Truncated PNG chunk");
    const length = view.getUint32(offset);
    const type = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (dataEnd + 4 > bytes.length) throw new Error(`Invalid PNG chunk ${type}`);
    if (crc32(bytes.subarray(offset + 4, dataEnd)) !== view.getUint32(dataEnd)) throw new Error(`Invalid PNG checksum ${type}`);
    if (!sawHeader && type !== "IHDR") throw new Error("PNG missing IHDR first chunk");
    if (type === "IHDR") {
      if (sawHeader || length !== 13) throw new Error("Invalid PNG IHDR");
      sawHeader = true;
      width = view.getUint32(dataStart);
      height = view.getUint32(dataStart + 4);
      if (!validDimensions({ width, height })) throw new Error("PNG missing dimensions");
      if (declared && (width !== declared.width || height !== declared.height)) throw new Error("PNG dimensions differ from declared capture");
      const bitDepth = bytes[dataStart + 8];
      colorType = bytes[dataStart + 9]!;
      if (bitDepth !== 8 || (colorType !== 6 && colorType !== 2))
        throw new Error(`Unsupported PNG format for audit: bitDepth=${bitDepth} colorType=${colorType}`);
      if (bytes[dataStart + 10] !== 0 || bytes[dataStart + 11] !== 0 || bytes[dataStart + 12] !== 0)
        throw new Error("Unsupported PNG compression, filter method or interlace");
    } else if (type === "IDAT") {
      if (idatEnded) throw new Error("Nonconsecutive PNG IDAT chunks");
      idatParts.push(bytes.subarray(dataStart, dataEnd));
    } else if (type === "IEND") {
      if (length !== 0 || !idatParts.length || dataEnd + 4 !== bytes.length) throw new Error("Invalid PNG IEND");
      sawEnd = true;
    } else {
      if (idatParts.length) idatEnded = true;
      // RGB transparency needs a different alpha contract; never silently ignore it.
      if (type === "tRNS" || (type !== "PLTE" && (bytes[offset + 4]! & 0x20) === 0))
        throw new Error(`Unsupported PNG chunk ${type}`);
      if (type === "PLTE" && (idatParts.length || length === 0 || length > 768 || length % 3 !== 0))
        throw new Error("Invalid PNG PLTE");
    }
    offset = dataEnd + 4;
  }
  if (!sawHeader || !sawEnd) throw new Error("PNG missing IHDR or IEND");
  const bytesPerPixel = colorType === 6 ? 4 : 3;
  const rowBytes = width * bytesPerPixel;
  const expected = height * (rowBytes + 1);
  const compressed = idatParts.length === 1 ? idatParts[0]! : Buffer.concat(idatParts);
  const inflated = inflateSync(compressed, { maxOutputLength: expected });
  if (inflated.length !== expected) throw new Error(`PNG pixel data length mismatch: ${inflated.length} != ${expected}`);
  let previous = new Uint8Array(rowBytes);
  let current = new Uint8Array(rowBytes);
  const rgbRow = colorType === 2 ? new Uint8Array(width * 4) : undefined;
  let readOffset = 0;
  for (let y = 0; y < height; y++) {
    const filter = inflated[readOffset++];
    for (let x = 0; x < rowBytes; x++) {
      const raw = inflated[readOffset++]!;
      const left = x >= bytesPerPixel ? current[x - bytesPerPixel]! : 0;
      const up = previous[x]!;
      const upLeft = x >= bytesPerPixel ? previous[x - bytesPerPixel]! : 0;
      let value: number;
      if (filter === 0) value = raw;
      else if (filter === 1) value = raw + left;
      else if (filter === 2) value = raw + up;
      else if (filter === 3) value = raw + Math.floor((left + up) / 2);
      else if (filter === 4) value = raw + paethPredictor(left, up, upLeft);
      else throw new Error(`Unsupported PNG filter ${filter}`);
      current[x] = value & 0xff;
    }
    if (rgbRow) {
      for (let x = 0, out = 0; x < rowBytes; x += 3, out += 4) {
        rgbRow[out] = current[x]!;
        rgbRow[out + 1] = current[x + 1]!;
        rgbRow[out + 2] = current[x + 2]!;
        rgbRow[out + 3] = 255;
      }
    }
    visit(rgbRow ?? current, y);
    const swap = previous;
    previous = current;
    current = swap;
  }
  return { width, height };
}

export function auditRgbaPng(bytes: Uint8Array): ScreenshotContentAudit {
  let sampledPixels = 0;
  let nonBlackPixels = 0;
  let nonTransparentPixels = 0;
  let lumaSum = 0;
  let maxLuma = 0;
  const buckets = new Set<string>();
  visitPngRgbaRows(bytes, current => {
    for (let x = 0; x < current.length; x += 4) {
      const r = current[x]!;
      const g = current[x + 1]!;
      const b = current[x + 2]!;
      const a = current[x + 3]!;
      sampledPixels += 1;
      if (a > 0) nonTransparentPixels += 1;
      if (a > 0 && (r > 8 || g > 8 || b > 8)) nonBlackPixels += 1;
      const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      lumaSum += luma;
      maxLuma = Math.max(maxLuma, luma);
      buckets.add(`${r >> 5}:${g >> 5}:${b >> 5}:${a === 0 ? 0 : 1}`);
    }
  });
  const meanLuma = sampledPixels > 0 ? lumaSum / sampledPixels : 0;
  const uniqueBucketCount = buckets.size;
  const nonBlackRatio = sampledPixels > 0 ? nonBlackPixels / sampledPixels : 0;
  const solidLike = uniqueBucketCount <= 1;
  const sparseDarkCaptureLike = meanLuma < 5.0 && nonBlackRatio < 0.001;
  const darkEmptyLike = (uniqueBucketCount <= 2 || sparseDarkCaptureLike) && meanLuma < 5.0 &&
    nonBlackRatio < 0.001 && (maxLuma < 16.0 || sparseDarkCaptureLike);
  const blank = sampledPixels === 0 || nonTransparentPixels === 0 || solidLike || darkEmptyLike;
  return { sampledPixels, nonBlackPixels, nonTransparentPixels, uniqueBucketCount, meanLuma, maxLuma, nonBlackRatio, blank };
}

export function hashPngRegion(bytes: Uint8Array, dimensions: PngDimensions, region: PixelRegion) {
  if (!validDimensions(dimensions) || !validDimensions(region) ||
      !Number.isSafeInteger(region.x) || !Number.isSafeInteger(region.y) || region.x < 0 || region.y < 0 ||
      region.x + region.width > dimensions.width || region.y + region.height > dimensions.height)
    throw new Error("Invalid PNG pixel region");
  const hash = createHash("sha256");
  let opaquePixels = 0;
  visitPngRgbaRows(bytes, (row, y) => {
    if (y < region.y || y >= region.y + region.height) return;
    const start = region.x * 4;
    const end = (region.x + region.width) * 4;
    hash.update(row.subarray(start, end));
    for (let x = start + 3; x < end; x += 4) if (row[x] === 255) opaquePixels++;
  }, dimensions);
  return { sha256: hash.digest("hex"), sampledPixels: region.width * region.height, opaquePixels };
}
