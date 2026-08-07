import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const roots: string[] = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function receipt(
  classification: string,
  windowInstanceId: string,
  binarySha256: string,
  extra: Record<string, unknown> = {},
) {
  return {
    schemaVersion: 2,
    tool: "script-kit-devtools.layout",
    command: "layout.measure",
    classification,
    requestedTarget: { selector: { type: "main" } },
    transaction: {
      automationId: "main",
      windowInstanceId,
      windowGeneration: Number(windowInstanceId.split("@")[1]),
      windowKind: "Main",
      hostKind: "mainWindow",
      surfaceKind: "ScriptList",
      semanticSurface: "scriptList",
      appViewVariant: "ScriptList",
      backingScaleFactor: 2,
    },
    repository: {
      gitCommit: binarySha256.slice(0, 8),
      implementationFingerprint: binarySha256,
    },
    binary: { sha256: binarySha256 },
    fixture: { id: "pf002-layout-fixture" },
    window: { rect: { x: 0, y: 0, width: 800, height: 600 } },
    textSummary: { inputLength: 0, inputFingerprint: "none" },
    ...extra,
  };
}

function runCompare(red: Record<string, unknown>, green: Record<string, unknown>) {
  const root = mkdtempSync(join(tmpdir(), "pf002-compare-"));
  roots.push(root);
  const redPath = join(root, "red.json");
  const greenPath = join(root, "green.json");
  writeFileSync(redPath, JSON.stringify(red));
  writeFileSync(greenPath, JSON.stringify(green));
  const result = Bun.spawnSync([
    "bun",
    "scripts/devtools/compare.ts",
    "redgreen",
    "--red",
    redPath,
    "--green",
    greenPath,
    "--require-fixed",
  ], { stdout: "pipe", stderr: "pipe" });
  return {
    exitCode: result.exitCode,
    receipt: JSON.parse(new TextDecoder().decode(result.stdout)),
  };
}

describe("PF-002 red/green comparison basis", () => {
  test("accepts comparable user paths on distinct instances and implementations", () => {
    const result = runCompare(
      receipt("reproduced", "main@1", "a".repeat(64)),
      receipt("ok", "main@2", "b".repeat(64)),
    );
    expect(result.exitCode).toBe(0);
    expect(result.receipt.disposition).toBe("EVALUABLE_PASS");
    expect(result.receipt.comparisonAssertions.sameComparisonBasis).toBe(true);
    expect(result.receipt.comparisonAssertions.distinctWindowInstances).toBe(true);
    expect(result.receipt.comparisonAssertions.implementationChanged).toBe(true);
  });

  test("rejects a reopened proof that reuses the old instance identity", () => {
    const result = runCompare(
      receipt("reproduced", "main@1", "a".repeat(64)),
      receipt("ok", "main@1", "b".repeat(64)),
    );
    expect(result.exitCode).toBe(4);
    expect(result.receipt.disposition).toBe("INVALID_IDENTITY");
  });

  test("rejects unlike hosts even when both receipts say ScriptList", () => {
    const result = runCompare(
      receipt("reproduced", "main@1", "a".repeat(64)),
      receipt("ok", "main@2", "b".repeat(64), {
        transaction: {
          automationId: "main",
          windowInstanceId: "main@2",
          windowGeneration: 2,
          windowKind: "Main",
          hostKind: "detachedWindow",
          surfaceKind: "ScriptList",
          semanticSurface: "scriptList",
          appViewVariant: "ScriptList",
          backingScaleFactor: 2,
        },
      }),
    );
    expect(result.exitCode).toBe(3);
    expect(result.receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(result.receipt.comparisonAssertions.sameComparisonBasis).toBe(false);
  });
});
