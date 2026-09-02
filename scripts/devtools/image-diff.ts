#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { readReceiptDocument, resolveReceiptDetails } from "./lib/receipt-artifact.ts";

type Args = {
  red: string;
  green: string;
  out: string;
  receiptOut: string;
  label: string;
  fuzz: string;
  redCrop: string;
  greenCrop: string;
  redCropFromReceipt: string;
  greenCropFromReceipt: string;
  redReceipt: string;
  greenReceipt: string;
  redReferenceWidth: number | null;
  greenReferenceWidth: number | null;
  requireSameSize: boolean;
  requireOsEvidence: boolean;
  requireInputHashes: boolean;
};

type Dimensions = {
  width: number;
  height: number;
};

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/image-diff.ts compare --red <before.png> --green <after.png> --out <diff.png> [--receipt-out <receipt.json>] [--label <name>] [--fuzz <percent>] [--red-crop <WxH+X+Y>] [--green-crop <WxH+X+Y>]",
    "  bun scripts/devtools/image-diff.ts compare --red <before.png> --green <after.png> --out <diff.png> --red-crop-from-receipt <inspect.json> --green-crop-from-receipt <inspect.json> --red-reference-width <logical px> --green-reference-width <logical px>",
    "  bun scripts/devtools/image-diff.ts compare --red <before.png> --green <after.png> --out <diff.png> --red-receipt <verify-shot.json> --green-receipt <verify-shot.json> [--require-os-evidence] [--require-input-hashes]",
    "",
    "Creates an ImageMagick compare mask and emits a JSON receipt with dimensions, changed-pixel count, ratio, and diff bounding box.",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  if (argv[0] !== "compare") {
    console.error(usage());
    process.exit(2);
  }

  const args: Args = {
    red: "",
    green: "",
    out: "",
    receiptOut: "",
    label: "image-diff",
    fuzz: "0%",
    redCrop: "",
    greenCrop: "",
    redCropFromReceipt: "",
    greenCropFromReceipt: "",
    redReceipt: "",
    greenReceipt: "",
    redReferenceWidth: null,
    greenReferenceWidth: null,
    requireSameSize: false,
    requireOsEvidence: false,
    requireInputHashes: false,
  };

  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--red") {
      args.red = argv[++index] ?? "";
    } else if (arg === "--green") {
      args.green = argv[++index] ?? "";
    } else if (arg === "--out") {
      args.out = argv[++index] ?? "";
    } else if (arg === "--receipt-out") {
      args.receiptOut = argv[++index] ?? "";
    } else if (arg === "--label") {
      args.label = argv[++index] ?? args.label;
    } else if (arg === "--fuzz") {
      args.fuzz = argv[++index] ?? args.fuzz;
    } else if (arg === "--red-crop") {
      args.redCrop = argv[++index] ?? "";
    } else if (arg === "--green-crop") {
      args.greenCrop = argv[++index] ?? "";
    } else if (arg === "--red-crop-from-receipt") {
      args.redCropFromReceipt = argv[++index] ?? "";
    } else if (arg === "--green-crop-from-receipt") {
      args.greenCropFromReceipt = argv[++index] ?? "";
    } else if (arg === "--red-receipt") {
      args.redReceipt = argv[++index] ?? "";
    } else if (arg === "--green-receipt") {
      args.greenReceipt = argv[++index] ?? "";
    } else if (arg === "--red-reference-width") {
      args.redReferenceWidth = Number(argv[++index] ?? "");
    } else if (arg === "--green-reference-width") {
      args.greenReferenceWidth = Number(argv[++index] ?? "");
    } else if (arg === "--require-same-size") {
      args.requireSameSize = true;
    } else if (arg === "--require-os-evidence") {
      args.requireOsEvidence = true;
    } else if (arg === "--require-input-hashes") {
      args.requireInputHashes = true;
    }
  }

  if (!args.red || !args.green || !args.out) {
    console.error(usage());
    process.exit(2);
  }
  return args;
}

function runMagick(args: string[], okExitCodes = new Set([0])) {
  const result = Bun.spawnSync(["magick", ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = new TextDecoder().decode(result.stdout).trim();
  const stderr = new TextDecoder().decode(result.stderr).trim();
  if (!okExitCodes.has(result.exitCode ?? 1)) {
    throw new Error(`magick ${args.join(" ")} failed with ${result.exitCode}: ${stderr || stdout}`);
  }
  return { stdout, stderr, exitCode: result.exitCode ?? 0 };
}

function identify(path: string): Dimensions {
  const { stdout } = runMagick(["identify", "-format", "%w %h", path]);
  const [width, height] = stdout.split(/\s+/).map(Number);
  if (!Number.isFinite(width) || !Number.isFinite(height)) {
    throw new Error(`Could not identify dimensions for ${path}: ${stdout}`);
  }
  return { width, height };
}

function sha256File(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function parseChangedPixels(metric: string): number {
  const parenthesized = metric.match(/\(([-+0-9.eE]+)\)\s*$/);
  const raw = parenthesized?.[1] ?? metric.match(/[-+0-9.eE]+/)?.[0] ?? "";
  const value = Number(raw);
  if (!Number.isFinite(value)) {
    throw new Error(`Could not parse ImageMagick AE metric: ${metric}`);
  }
  return Math.round(value);
}

function parseBoundingBox(value: string) {
  const match = value.match(/^(\d+)x(\d+)\+(-?\d+)\+(-?\d+)$/);
  if (!match) {
    return null;
  }
  return {
    width: Number(match[1]),
    height: Number(match[2]),
    x: Number(match[3]),
    y: Number(match[4]),
  };
}

function asObject(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null ? value as Record<string, unknown> : {};
}

function numberAt(source: Record<string, unknown>, path: string): number | null {
  const value = path.split(".").reduce<unknown>((current, part) => {
    if (typeof current !== "object" || current === null) return undefined;
    return (current as Record<string, unknown>)[part];
  }, source);
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

async function cropFromReceipt(path: string, imageWidth: number, referenceWidth: number | null) {
  if (!path) return null;
  if (!Number.isFinite(referenceWidth) || referenceWidth == null || referenceWidth <= 0) {
    throw new Error(`--*-crop-from-receipt requires a positive --*-reference-width`);
  }
  const receipt = resolveReceiptDetails(readReceiptDocument(path), path);
  const root = asObject(receipt);
  const boundsCandidates = [
    asObject(asObject(asObject(root.visualEvidence).captureIdentity).crop),
    asObject(asObject(root.inspection).targetBoundsInScreenshot),
    asObject(asObject(root.popupCapture).targetBounds),
    asObject(asObject(asObject(root.target).screenshotIdentity).targetBoundsInScreenshot),
    asObject(asObject(asObject(root.resolvedTarget).screenshotIdentity).targetBoundsInScreenshot),
  ];
  const bounds = boundsCandidates.find((candidate) =>
    ["x", "y", "width", "height"].every((key) => typeof candidate[key] === "number")
  ) ?? {};
  const x = typeof bounds.x === "number" ? bounds.x : null;
  const y = typeof bounds.y === "number" ? bounds.y : null;
  const width = typeof bounds.width === "number" ? bounds.width : null;
  const height = typeof bounds.height === "number" ? bounds.height : null;
  if (![x, y, width, height].every((value) => typeof value === "number" && Number.isFinite(value))) {
    throw new Error(`Could not find a supported target crop in ${path}`);
  }
  const scale = imageWidth / referenceWidth;
  return {
    crop: `${Math.round(width * scale)}x${Math.round(height * scale)}+${Math.round(x * scale)}+${Math.round(y * scale)}`,
    sourceReceipt: path,
    logicalBounds: { x, y, width, height },
    referenceWidth,
    scale,
  };
}

async function osEvidenceFromReceipt(path: string) {
  if (!path) return null;
  const receipt = resolveReceiptDetails(readReceiptDocument(path), path);
  const visualEvidence = asObject(receipt.visualEvidence);
  const pixelAudit = asObject(visualEvidence.pixelAudit);
  const screenshotEvidence = asObject(receipt.screenshotEvidence);
  const captureIdentity = asObject(visualEvidence.captureIdentity);
  return {
    receiptPath: path,
    source: visualEvidence.source ?? null,
    classification: visualEvidence.classification ?? null,
    captureKind: visualEvidence.captureKind ?? null,
    countsAsOsScreenshotEvidence: visualEvidence.countsAsOsScreenshotEvidence === true,
    countsAsCompositorEvidence: visualEvidence.countsAsCompositorEvidence === true,
    pixelAuditBlank: typeof pixelAudit.blank === "boolean" ? pixelAudit.blank : null,
    blockerCode: visualEvidence.blockerCode ?? null,
    expectedSha256:
      screenshotEvidence.sha256 ?? captureIdentity.sha256 ?? visualEvidence.sha256 ?? null,
  };
}

async function emitBlockedReceipt(args: Args, blockerCode: string, inputEvidence: Record<string, unknown>) {
  const red = asObject(inputEvidence.red);
  const green = asObject(inputEvidence.green);
  const receipt = {
    schemaVersion: 1,
    tool: "script-kit-devtools.image-diff",
    command: "image-diff.compare",
    classification: "blocked",
    blockerCode,
    proofKind: args.requireSameSize ? "capture-stability" : "visual-diff",
    sameSizeRequired: args.requireSameSize,
    label: args.label,
    redPath: args.red,
    greenPath: args.green,
    diffPath: args.out,
    inputEvidence,
    assertions: {
      redCountsAsOsScreenshotEvidence: red.countsAsOsScreenshotEvidence === true,
      greenCountsAsOsScreenshotEvidence: green.countsAsOsScreenshotEvidence === true,
      redNonBlank: red.pixelAuditBlank === false,
      greenNonBlank: green.pixelAuditBlank === false,
      cropResolvedFromReceipts: false,
      diffMaskWritten: false,
      changedPixelsMeasured: false,
      sameSizeWhenRequired: false,
    },
    errors: [blockerCode],
    timestamp: new Date().toISOString(),
  };
  const serialized = `${JSON.stringify(receipt, null, 2)}\n`;
  if (args.receiptOut) await Bun.write(args.receiptOut, serialized);
  console.log(serialized.trimEnd());
  process.exit(2);
}

async function prepareInput(path: string, crop: string, tmpDir: string, name: string) {
  if (!crop) {
    return path;
  }
  if (!/^\d+x\d+\+-?\d+\+-?\d+$/.test(crop)) {
    throw new Error(`Invalid ${name} crop ${crop}; expected WxH+X+Y`);
  }
  const out = `${tmpDir}/${name}.png`;
  runMagick([path, "-crop", crop, "+repage", out]);
  return out;
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  await Bun.write(args.out, "");
  await Bun.$`rm -f ${args.out}`;
  const tmpDir = `/tmp/script-kit-image-diff-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  await Bun.$`mkdir -p ${tmpDir}`;

  const redEvidence = await osEvidenceFromReceipt(args.redReceipt || args.redCropFromReceipt);
  const greenEvidence = await osEvidenceFromReceipt(args.greenReceipt || args.greenCropFromReceipt);
  if (args.requireOsEvidence) {
    const inputEvidence = { red: redEvidence, green: greenEvidence };
    const redOk = redEvidence?.countsAsOsScreenshotEvidence === true &&
      redEvidence.countsAsCompositorEvidence === true &&
      redEvidence.classification === "captured" &&
      redEvidence.pixelAuditBlank === false;
    const greenOk = greenEvidence?.countsAsOsScreenshotEvidence === true &&
      greenEvidence.countsAsCompositorEvidence === true &&
      greenEvidence.classification === "captured" &&
      greenEvidence.pixelAuditBlank === false;
    if (!redOk) {
      await emitBlockedReceipt(args, "red-os-evidence-missing", inputEvidence);
    }
    if (!greenOk) {
      await emitBlockedReceipt(args, "green-os-evidence-missing", inputEvidence);
    }
  }

  const sourceHashes = {
    red: sha256File(args.red),
    green: sha256File(args.green),
  };
  const inputHashes = {
    red: {
      sha256: sourceHashes.red,
      expectedSha256: redEvidence?.expectedSha256 ?? null,
      matchesReceipt: redEvidence?.expectedSha256 === sourceHashes.red,
    },
    green: {
      sha256: sourceHashes.green,
      expectedSha256: greenEvidence?.expectedSha256 ?? null,
      matchesReceipt: greenEvidence?.expectedSha256 === sourceHashes.green,
    },
  };
  if (args.requireInputHashes) {
    const inputEvidence = { red: redEvidence, green: greenEvidence, inputHashes };
    if (typeof redEvidence?.expectedSha256 !== "string") {
      await emitBlockedReceipt(args, "red-receipt-hash-missing", inputEvidence);
    }
    if (!inputHashes.red.matchesReceipt) {
      await emitBlockedReceipt(args, "red-receipt-hash-mismatch", inputEvidence);
    }
    if (typeof greenEvidence?.expectedSha256 !== "string") {
      await emitBlockedReceipt(args, "green-receipt-hash-missing", inputEvidence);
    }
    if (!inputHashes.green.matchesReceipt) {
      await emitBlockedReceipt(args, "green-receipt-hash-mismatch", inputEvidence);
    }
  }

  const sourceRedDimensions = identify(args.red);
  const sourceGreenDimensions = identify(args.green);
  const redReceiptCrop = await cropFromReceipt(args.redCropFromReceipt, sourceRedDimensions.width, args.redReferenceWidth);
  const greenReceiptCrop = await cropFromReceipt(args.greenCropFromReceipt, sourceGreenDimensions.width, args.greenReferenceWidth);
  const redCrop = args.redCrop || redReceiptCrop?.crop || "";
  const greenCrop = args.greenCrop || greenReceiptCrop?.crop || "";
  const redInput = await prepareInput(args.red, redCrop, tmpDir, "red");
  const greenInput = await prepareInput(args.green, greenCrop, tmpDir, "green");
  const redDimensions = identify(redInput);
  const greenDimensions = identify(greenInput);
  const canvas = {
    width: Math.max(redDimensions.width, greenDimensions.width),
    height: Math.max(redDimensions.height, greenDimensions.height),
  };

  const compare = runMagick(
    [
      "compare",
      "-metric",
      "AE",
      "-fuzz",
      args.fuzz,
      "-highlight-color",
      "red",
      "-lowlight-color",
      "black",
      redInput,
      greenInput,
      args.out,
    ],
    new Set([0, 1]),
  );
  const changedPixels = parseChangedPixels(compare.stderr || compare.stdout);
  const totalPixels = canvas.width * canvas.height;
  const changedPixelRatio = totalPixels > 0 ? changedPixels / totalPixels : null;
  const trim = runMagick([
    args.out,
    "-fuzz",
    "1%",
    "-trim",
    "-format",
    "%wx%h%X%Y",
    "info:",
  ]);
  const diffBoundingBox = changedPixels > 0 ? parseBoundingBox(trim.stdout) : null;
  const sameSize =
    redDimensions.width === greenDimensions.width &&
    redDimensions.height === greenDimensions.height;
  const errors = args.requireSameSize && !sameSize ? ["dimension_mismatch"] : [];
  const classification = errors.length === 0 ? "ok" : "error";

  const receipt = {
    schemaVersion: 1,
    tool: "script-kit-devtools.image-diff",
    command: "image-diff.compare",
    classification,
    proofKind: args.requireSameSize ? "capture-stability" : "visual-diff",
    sameSizeRequired: args.requireSameSize,
    label: args.label,
    redPath: args.red,
    greenPath: args.green,
    diffPath: args.out,
    fuzz: args.fuzz,
    crop: {
      red: redCrop || null,
      green: greenCrop || null,
      redSource: redReceiptCrop,
      greenSource: greenReceiptCrop,
    },
    inputEvidence: {
      red: redEvidence,
      green: greenEvidence,
    },
    inputHashes,
    dimensions: {
      red: redDimensions,
      green: greenDimensions,
      comparisonCanvas: canvas,
      sameSize,
      widthDelta: greenDimensions.width - redDimensions.width,
      heightDelta: greenDimensions.height - redDimensions.height,
    },
    changedPixels,
    totalPixels,
    changedPixelRatio,
    changedPixelPercent: changedPixelRatio == null ? null : changedPixelRatio * 100,
    diffBoundingBox,
    assertions: {
      redCountsAsOsScreenshotEvidence: args.requireOsEvidence
        ? redEvidence?.countsAsOsScreenshotEvidence === true
        : true,
      greenCountsAsOsScreenshotEvidence: args.requireOsEvidence
        ? greenEvidence?.countsAsOsScreenshotEvidence === true
        : true,
      redNonBlank: args.requireOsEvidence ? redEvidence?.pixelAuditBlank === false : true,
      greenNonBlank: args.requireOsEvidence ? greenEvidence?.pixelAuditBlank === false : true,
      cropResolvedFromReceipts: args.redCropFromReceipt || args.greenCropFromReceipt
        ? redReceiptCrop != null || greenReceiptCrop != null
        : true,
      diffMaskWritten: await Bun.file(args.out).exists(),
      changedPixelsMeasured: Number.isFinite(changedPixels),
      dimensionsMeasured: true,
      sameSizeWhenRequired: args.requireSameSize ? sameSize : true,
    },
    warnings: [
      sameSize
        ? ""
        : "red and green screenshots have different dimensions; changed-pixel ratio uses the max comparison canvas",
    ].filter(Boolean),
    errors,
    timestamp: new Date().toISOString(),
  };

  await Bun.$`rm -rf ${tmpDir}`;
  const serialized = `${JSON.stringify(receipt, null, 2)}\n`;
  if (args.receiptOut) await Bun.write(args.receiptOut, serialized);
  console.log(serialized.trimEnd());
  if (classification === "error") {
    process.exit(2);
  }
}

await main();
