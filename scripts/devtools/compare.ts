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

export function targetIdentity(receipt: JsonObject) {
  const transaction = asObject(receipt.transaction);
  const target = asObject(receipt.target ?? receipt.targetAfter ?? receipt.resolvedTarget);
  return {
    automationId: transaction.automationId ?? target.automationId ?? target.stableTargetId ?? null,
    windowInstanceId: transaction.windowInstanceId ?? target.windowInstanceId ?? null,
    windowGeneration: transaction.windowGeneration ?? target.windowGeneration ?? null,
    windowKind: transaction.windowKind ?? target.windowKind ?? target.targetKind ?? null,
    hostKind: transaction.hostKind ?? target.hostKind ?? null,
    surfaceKind: transaction.surfaceKind ?? target.surfaceKind ?? null,
    semanticSurface: transaction.semanticSurface ?? target.semanticSurface ?? null,
    appViewVariant: transaction.appViewVariant ?? target.appViewVariant ?? null,
    targetGeneration: transaction.targetGeneration ?? target.targetGeneration ?? null,
    surfaceGeneration: transaction.surfaceGeneration ?? target.surfaceGeneration ?? null,
    dataGeneration: transaction.dataGeneration ?? target.dataGeneration ?? null,
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

export function comparisonBasis(receipt: JsonObject) {
  const identity = targetIdentity(receipt);
  const fixture = asObject(receipt.fixture);
  const window = asObject(receipt.window);
  const viewport = receipt.viewportRect ?? window.rect ?? null;
  return {
    fixtureId: fixture.id ?? null,
    targetSelector: targetSelector(receipt),
    windowKind: identity.windowKind,
    hostKind: identity.hostKind,
    surfaceKind: identity.surfaceKind,
    semanticSurface: identity.semanticSurface,
    appViewVariant: identity.appViewVariant,
    viewport,
    backingScaleFactor: asObject(receipt.transaction).backingScaleFactor ?? null,
    userPath: receipt.command ?? null,
    metricNames: metricNames(receipt),
  };
}

function sourceIdentity(receipt: JsonObject) {
  const binary = asObject(receipt.binary);
  const repository = asObject(receipt.repository);
  return {
    binarySha256: binary.sha256 ?? null,
    implementationFingerprint: repository.implementationFingerprint ?? null,
    gitCommit: repository.gitCommit ?? null,
  };
}

function classify(assertions: JsonObject, args: Args, red: JsonObject, green: JsonObject) {
  if (args.requireFixed && assertions.distinctWindowInstances !== true) {
    return "invalid-identity";
  }
  if (
    !assertions.samePrimitiveStack
    || !assertions.sameUserPath
    || !assertions.sameComparisonBasis
    || !assertions.metricNamesComparable
  ) {
    return "blocked-by-missing-primitive";
  }
  if (args.requireFixed && assertions.implementationChanged !== true) {
    return "invalid-binary";
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
  const redIdentity = targetIdentity(red);
  const greenIdentity = targetIdentity(green);
  const redBasis = comparisonBasis(red);
  const greenBasis = comparisonBasis(green);
  const redSource = sourceIdentity(red);
  const greenSource = sourceIdentity(green);
  const assertions = {
    samePrimitiveStack: sameStringArray(redStack, greenStack),
    sameUserPath: red.command === green.command,
    sameTargetSelector: stableJson(redTargetSelector) === stableJson(greenTargetSelector),
    sameComparisonBasis: stableJson(redBasis) === stableJson(greenBasis),
    targetIdentityComparable:
      redIdentity.windowKind === greenIdentity.windowKind
      && redIdentity.hostKind === greenIdentity.hostKind
      && redIdentity.surfaceKind === greenIdentity.surfaceKind
      && redIdentity.appViewVariant === greenIdentity.appViewVariant,
    distinctWindowInstances:
      typeof redIdentity.windowInstanceId === "string"
      && typeof greenIdentity.windowInstanceId === "string"
      && redIdentity.windowInstanceId !== greenIdentity.windowInstanceId,
    implementationChanged: stableJson(redSource) !== stableJson(greenSource),
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
    comparisonAssertions: assertions,
    assertions: [
      {
        id: "same-primitive-stack",
        required: true,
        sourceLayer: "governance",
        expected: true,
        observed: assertions.samePrimitiveStack,
        pass: assertions.samePrimitiveStack,
      },
      {
        id: "same-user-path",
        required: true,
        sourceLayer: "governance",
        expected: true,
        observed: assertions.sameUserPath,
        pass: assertions.sameUserPath,
      },
      {
        id: "same-comparison-basis",
        required: true,
        sourceLayer: "governance",
        expected: true,
        observed: assertions.sameComparisonBasis,
        pass: assertions.sameComparisonBasis,
      },
      {
        id: "metric-names-comparable",
        required: true,
        sourceLayer: "governance",
        expected: true,
        observed: assertions.metricNamesComparable,
        pass: assertions.metricNamesComparable,
      },
      ...(args.requireFixed
        ? [
            {
              id: "distinct-window-instances",
              required: true,
              sourceLayer: "governance",
              expected: true,
              observed: assertions.distinctWindowInstances,
              pass: assertions.distinctWindowInstances,
            },
            {
              id: "implementation-changed",
              required: true,
              sourceLayer: "governance",
              expected: true,
              observed: assertions.implementationChanged,
              pass: assertions.implementationChanged,
            },
          ]
        : []),
    ],
    primitiveStack: { red: redStack, green: greenStack },
    targetSelector: { red: redTargetSelector, green: greenTargetSelector },
    targetIdentity: { red: redIdentity, green: greenIdentity },
    comparisonBasis: { red: redBasis, green: greenBasis },
    sourceIdentity: { red: redSource, green: greenSource },
    metricNames: { red: redMetrics, green: greenMetrics },
    classificationDelta: {
      red: red.classification ?? null,
      green: green.classification ?? null,
    },
    warnings: [
      assertions.samePrimitiveStack ? "" : "red and green receipts use different primitive stacks",
      assertions.sameUserPath ? "" : "red and green receipts use different user paths",
      assertions.sameTargetSelector ? "" : "red and green receipts use different target selectors",
      assertions.sameComparisonBasis ? "" : "red and green receipts have different comparison bases",
      !args.requireFixed || assertions.distinctWindowInstances ? "" : "red and green reused one window instance",
      !args.requireFixed || assertions.implementationChanged ? "" : "fixed proof reused the same implementation identity",
      assertions.metricNamesComparable ? "" : "red and green receipts expose different metric names",
    ].filter(Boolean),
    errors: [],
  });
}

if (import.meta.main) await main();
