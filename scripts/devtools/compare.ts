#!/usr/bin/env bun

import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { filePath } from "./lib/privacy.ts";

type JsonObject = Record<string, unknown>;

type Args = {
  red: string;
  green: string;
  requireFixed: boolean;
};

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/compare.ts redgreen --red <receipt.json> --green <receipt.json> [--require-fixed]",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  if (argv[0] !== "redgreen") {
    console.error(usage());
    process.exit(2);
  }
  const args: Args = { red: "", green: "", requireFixed: false };
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--red") {
      args.red = argv[++index] ?? "";
    } else if (arg === "--green") {
      args.green = argv[++index] ?? "";
    } else if (arg === "--require-fixed") {
      args.requireFixed = true;
    }
  }
  if (!args.red || !args.green) {
    console.error(usage());
    process.exit(2);
  }
  return args;
}

async function readJson(path: string): Promise<JsonObject> {
  return JSON.parse(await Bun.file(path).text()) as JsonObject;
}

function asObject(value: unknown): JsonObject {
  return typeof value === "object" && value !== null ? value as JsonObject : {};
}

function pathValue(source: JsonObject, path: string): unknown {
  return path.split(".").reduce<unknown>((current, part) => {
    if (typeof current !== "object" || current === null) {
      return undefined;
    }
    return (current as JsonObject)[part];
  }, source);
}

function compact(value: unknown) {
  if (value === undefined) {
    return null;
  }
  if (Array.isArray(value)) {
    return value.map(compact);
  }
  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(
      Object.entries(value as JsonObject)
        .filter(([, entry]) => entry !== undefined)
        .map(([key, entry]) => [key, compact(entry)]),
    );
  }
  return value;
}

function stableJson(value: unknown) {
  return JSON.stringify(compact(value));
}

function primitiveStack(receipt: JsonObject) {
  const command = String(receipt.command ?? "");
  const tool = String(receipt.tool ?? "");
  const expected = asObject(receipt.expected);
  const prePost = Array.isArray(expected.prePostReceipts) ? expected.prePostReceipts.map(String) : [];
  return [tool, command, ...prePost].filter(Boolean);
}

function targetSelector(receipt: JsonObject) {
  const requestedTarget = asObject(receipt.requestedTarget);
  return requestedTarget.selector ?? pathValue(receipt, "requestedTarget") ?? null;
}

function targetIdentity(receipt: JsonObject) {
  const target = asObject(receipt.target ?? receipt.targetAfter ?? receipt.resolvedTarget);
  return {
    automationId: target.automationId ?? target.stableTargetId ?? null,
    surfaceKind: target.surfaceKind ?? null,
    appViewVariant: target.appViewVariant ?? null,
    targetKind: target.targetKind ?? null,
  };
}

function flattenMetricNames(value: unknown, prefix = ""): string[] {
  if (value == null) {
    return [];
  }
  if (typeof value !== "object") {
    return prefix ? [prefix] : [];
  }
  if (Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }
  return Object.entries(value as JsonObject).flatMap(([key, entry]) => {
    const next = prefix ? `${prefix}.${key}` : key;
    return flattenMetricNames(entry, next);
  });
}

function metricNames(receipt: JsonObject) {
  const candidates = [
    "visibleResult",
    "resizePressure",
    "scroll",
    "textSummary",
    "keyboardOwner",
    "activeFooter",
    "targetAfter",
  ];
  return [...new Set(candidates.flatMap((path) => flattenMetricNames(pathValue(receipt, path), path)))].sort();
}

function sameStringArray(left: string[], right: string[]) {
  return stableJson(left) === stableJson(right);
}

function classify(assertions: JsonObject, args: Args, red: JsonObject, green: JsonObject) {
  if (!assertions.samePrimitiveStack || !assertions.sameTargetSelector || !assertions.metricNamesComparable) {
    return "blocked-by-missing-primitive";
  }
  if (args.requireFixed && !(red.classification !== "ok" && green.classification === "ok")) {
    return "not-reproduced";
  }
  if (red.classification !== green.classification) {
    return green.classification === "ok" ? "fixed" : "reproduced";
  }
  return "ok";
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  const red = await readJson(args.red);
  const green = await readJson(args.green);
  const redStack = primitiveStack(red);
  const greenStack = primitiveStack(green);
  const redTargetSelector = targetSelector(red);
  const greenTargetSelector = targetSelector(green);
  const redMetrics = metricNames(red);
  const greenMetrics = metricNames(green);
  const assertions = {
    samePrimitiveStack: sameStringArray(redStack, greenStack),
    sameUserPath: red.command === green.command,
    sameTargetSelector: stableJson(redTargetSelector) === stableJson(greenTargetSelector),
    targetIdentityComparable: targetIdentity(red).surfaceKind === targetIdentity(green).surfaceKind,
    metricNamesComparable: sameStringArray(redMetrics, greenMetrics),
  };

  emitValidatedReceipt("devtools.compare.redgreen", {
    schemaVersion: 2,
    tool: "script-kit-devtools.compare",
    command: "compare.redgreen",
    classification: classify(assertions, args, red, green),
    redReceiptIds: [filePath(args.red)],
    greenReceiptIds: [filePath(args.green)],
    samePrimitiveStack: assertions.samePrimitiveStack,
    sameUserPath: assertions.sameUserPath,
    sameTargetSelector: assertions.sameTargetSelector,
    targetIdentityComparable: assertions.targetIdentityComparable,
    assertions,
    primitiveStack: { red: redStack, green: greenStack },
    targetSelector: { red: redTargetSelector, green: greenTargetSelector },
    targetIdentity: { red: targetIdentity(red), green: targetIdentity(green) },
    metricNames: { red: redMetrics, green: greenMetrics },
    classificationDelta: {
      red: red.classification ?? null,
      green: green.classification ?? null,
    },
    warnings: [
      assertions.samePrimitiveStack ? "" : "red and green receipts use different primitive stacks",
      assertions.sameTargetSelector ? "" : "red and green receipts use different target selectors",
      assertions.metricNamesComparable ? "" : "red and green receipts expose different metric names",
    ].filter(Boolean),
    errors: [],
  });
}

await main();
