import { afterEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  buildRuntimeCoverageScorecard,
  discoverRuntimeCoverageReceipts,
  type RuntimeProofReceipt,
} from "./lib/runtime-coverage.ts";
import { prepareValidatedReceipt } from "./lib/receipt-schema.ts";
import { runBindingsPipeline, type CoverageBindingRecord } from "./surfaces.ts";

const SOURCE = "f".repeat(40);
const BINARY = "a".repeat(64);
const roots: string[] = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

async function aboutBinding(): Promise<CoverageBindingRecord> {
  const pipeline = await runBindingsPipeline();
  const binding = pipeline.build.set.bindings.find(
    (candidate) => candidate.bindingId === "About::About@MainWindow",
  );
  if (!binding) throw new Error("About binding is required");
  return binding;
}

function runtimeReceipt(
  primitiveId: string,
  overrides: Record<string, unknown> = {},
): RuntimeProofReceipt {
  const transaction = {
    transactionId: "proof:about-one-frame",
    runId: "runtime-coverage-test",
    pid: 42,
    processStartTime: "Fri Aug 7 00:00:00 2026",
    binarySha256: BINARY,
    automationId: "main",
    windowInstanceId: "main@1",
    windowGeneration: 1,
    windowKind: "Main",
    hostKind: "mainWindow",
    surfaceKind: "About",
    semanticSurface: "about",
    appViewVariant: "About",
    bounds: { x: 0, y: 0, width: 800, height: 600 },
    targetGeneration: 1,
    surfaceGeneration: 1,
    dataGeneration: 1,
  };
  const variants: Record<string, Record<string, unknown>> = {
    "devtools.targets.inspect": {
      tool: "script-kit-devtools.targets",
      command: "targets.inspect",
      requestedTarget: { selector: { type: "main" } },
      resolvedTarget: {
        automationId: "main",
        visible: false,
        bounds: { x: 0, y: 0, width: 800, height: 600 },
      },
    },
    "devtools.surface.inspect": {
      tool: "script-kit-devtools.surface",
      command: "surface.inspect",
      requestedTarget: { selector: { type: "main" } },
      target: { automationId: "main", visible: false },
      contract: { surfaceKind: "About" },
      runtime: { capabilities: [], missingPrimitives: [] },
    },
    "devtools.layout.measure": {
      tool: "script-kit-devtools.layout",
      command: "layout.measure",
      requestedTarget: { selector: { type: "main" } },
      target: {
        automationId: "main",
        visible: false,
        bounds: { x: 0, y: 0, width: 800, height: 600 },
      },
      proofMode: "inspection",
      window: { rect: { x: 0, y: 0, width: 800, height: 600 } },
      regions: [],
      resizePressure: { windowCanGrow: true },
      pressure: { pressureScore: 0 },
      truthLayers: {
        model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        rendered: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        joins: [],
      },
    },
  };
  // Synthetic unit inputs exercise acceptance rules only; no application or
  // owned runtime resources are acquired, and these are not runtime evidence.
  const candidate = {
    schemaVersion: 2,
    classification: "ok",
    evidenceClass: "RUNTIME_HIDDEN",
    repository: { gitCommit: SOURCE },
    binary: { sha256: BINARY },
    transaction,
    durationMs: 12,
    missingPrimitives: [],
    errors: [],
    cleanup: { resourcesAcquired: false, closed: true, survivors: [] },
    ...variants[primitiveId],
    ...overrides,
  };
  const prepared = prepareValidatedReceipt(primitiveId, candidate);
  if (prepared.exitCode !== 0) {
    throw new Error(
      `invalid test receipt for ${primitiveId}: ${JSON.stringify(prepared.validation.errors)}`,
    );
  }
  return { path: `${primitiveId}.json`, receipt: prepared.receipt };
}

function completeAboutReceipts(): RuntimeProofReceipt[] {
  return [
    "devtools.targets.inspect",
    "devtools.surface.inspect",
    "devtools.layout.measure",
  ].map((primitiveId) => runtimeReceipt(primitiveId));
}

describe("target-scoped runtime coverage scorecard", () => {
  test("a static binding never counts as runtime surface coverage", async () => {
    const binding = await aboutBinding();
    const scorecard = buildRuntimeCoverageScorecard([binding], []);
    expect(binding.evidenceGrade).toBe("Derived");
    expect(scorecard.directRuntimeMappingCount).toBe(0);
    expect(scorecard.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(scorecard.mappings[0]?.missingPrimitiveIds).toEqual(
      binding.requiredPrimitiveIds,
    );
  });

  test("matching fresh primitives prove a mapping regardless of its static binding grade", async () => {
    const binding = await aboutBinding();
    const scorecard = buildRuntimeCoverageScorecard(
      [binding],
      completeAboutReceipts(),
      { sourceCommit: SOURCE, binarySha256: BINARY },
    );
    expect(scorecard.disposition).toBe("EVALUABLE_PASS");
    expect(scorecard.rejectedReceipts).toEqual([]);
    expect(scorecard.evidenceClass).toBe("DIRECT_RUNTIME_PROOF");
    expect(scorecard.directRuntimeMappingCount).toBe(1);
    expect(scorecard.totalRuntimeDurationMs).toBe(36);
    expect(scorecard.mappings[0]?.transactionId).toBe("proof:about-one-frame");
    expect(scorecard.mappings[0]?.missingPrimitiveIds).toEqual([]);
  });

  test("primitive proofs from separate transactions cannot be assembled into one fake frame", async () => {
    const binding = await aboutBinding();
    const receipts = completeAboutReceipts();
    (receipts[1]!.receipt.transaction as Record<string, unknown>).transactionId =
      "proof:another-frame";
    const scorecard = buildRuntimeCoverageScorecard([binding], receipts);
    expect(scorecard.directRuntimeMappingCount).toBe(0);
    expect(scorecard.mappings[0]?.missingPrimitiveIds.length).toBeGreaterThan(0);
  });

  test("static, wrong-target, stale-source, and stale-binary receipts cannot pass", async () => {
    const binding = await aboutBinding();

    const staticReceipts = completeAboutReceipts();
    staticReceipts[0]!.receipt.evidenceClass = "STATIC_INVENTORY";
    expect(
      buildRuntimeCoverageScorecard([binding], staticReceipts)
        .directRuntimeMappingCount,
    ).toBe(0);

    const wrongTarget = completeAboutReceipts();
    (wrongTarget[0]!.receipt.transaction as Record<string, unknown>).appViewVariant =
      "SettingsView";
    expect(
      buildRuntimeCoverageScorecard([binding], wrongTarget)
        .directRuntimeMappingCount,
    ).toBe(0);

    expect(
      buildRuntimeCoverageScorecard([binding], completeAboutReceipts(), {
        sourceCommit: "0".repeat(40),
      }).directRuntimeMappingCount,
    ).toBe(0);
    expect(
      buildRuntimeCoverageScorecard([binding], completeAboutReceipts(), {
        binarySha256: "b".repeat(64),
      }).directRuntimeMappingCount,
    ).toBe(0);
  });

  test("privacy violations and incomplete cleanup remain explicit blockers", async () => {
    const binding = await aboutBinding();
    const receipts = completeAboutReceipts();
    (receipts[0]!.receipt.privacy as Record<string, unknown>).rawContentReturned =
      true;
    (receipts[1]!.receipt.cleanup as Record<string, unknown>).closed = false;
    const scorecard = buildRuntimeCoverageScorecard([binding], receipts);
    expect(scorecard.privacyViolationCount).toBe(1);
    expect(scorecard.acceptedReceiptCount).toBe(1);
    expect(scorecard.rejectedReceipts.map((receipt) => receipt.reason)).toEqual([
      "runtime receipt has an incomplete or failing recursive privacy scan",
      "runtime receipt has unclosed cleanup or surviving owned processes",
    ]);
    expect(scorecard.directRuntimeMappingCount).toBe(0);
  });

  test("archived, baseline, and negative receipts never become current evidence", () => {
    const root = mkdtempSync(join(tmpdir(), "runtime-coverage-test-"));
    roots.push(root);
    const receipt = runtimeReceipt("devtools.layout.measure").receipt;
    writeFileSync(join(root, "current.json"), JSON.stringify(receipt));
    for (const archive of ["attempts", "invalid", "history", "baseline", "negative"]) {
      mkdirSync(join(root, archive));
      writeFileSync(join(root, archive, "old.json"), JSON.stringify(receipt));
    }
    const discovered = discoverRuntimeCoverageReceipts(root);
    expect(discovered).toHaveLength(1);
    expect(discovered[0]?.path).toEndWith("current.json");
  });
});
