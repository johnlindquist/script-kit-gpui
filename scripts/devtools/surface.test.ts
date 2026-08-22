import { expect, test } from "bun:test";

test("generated surface inventory includes every AI recovery host", () => {
  const generated = Bun.spawnSync(
    ["bun", "scripts/generate-surface-contracts.ts", "--check"],
    { stdout: "pipe", stderr: "pipe" },
  );
  expect(generated.exitCode).toBe(0);
  const result = Bun.spawnSync(["bun", "scripts/devtools/surfaces.ts"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.exitCode).toBe(0);
  const report = JSON.parse(result.stdout.toString());
  expect(report.evidenceStatus).toBe("SOURCE-CONFIRMED");
  expect(report.evidenceClass).toBe("STATIC_INVENTORY");
  expect(report.runtimeCoverage.directRuntimeMappingCount).toBe(0);
  expect(report.runtimeCoverage.totalMappingCount).toBe(54);
  expect(report.runtimeCoverage.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
  expect(report.inventoryNamespaces).toEqual({
    contractKindCount: 37,
    contractMappingCount: 54,
    uniqueAppViewVariantCount: 53,
    runtimeCoverageProfileCount: 19,
    orientationAliasCount: 5,
  });
  expect(report.featureMapSource).toEqual({
    path: "FEATURE_MAP.md",
    compatibilityIndexExists: true,
    parsedEntryCount: 37,
    maintainedAtlasPath: "feature-map/index.md",
    maintainedAtlasExists: true,
    status: "maintained-atlas",
  });
  const variants = report.surfaceContracts.flatMap(
    (entry: { appViewVariants: string[] }) => entry.appViewVariants,
  );
  for (const variant of [
    "AgentChatView",
    "ChatPrompt",
    "FlowUxView",
    "FlowSessionView",
  ]) {
    expect(variants).toContain(variant);
  }
});

test("coverage reports runtime profiles as a separate inventory namespace", () => {
  const result = Bun.spawnSync(["bun", "scripts/devtools/coverage.ts"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.exitCode).toBe(0);
  const report = JSON.parse(result.stdout.toString());
  expect(report.evidenceStatus).toBe("SOURCE-CONFIRMED");
  expect(report.evidenceClass).toBe("STATIC_INVENTORY");
  expect(report.runtimeProof).toEqual({
    disposition: "NOT_EVALUATED",
    provenSurfaceCount: 0,
    note: "A Direct profile binding and a valid source-owner inventory do not prove runtime behavior.",
  });
  expect(report.registryValidation).toEqual({
    errors: [],
    validatesSourceOwners: true,
  });
  expect(report.inventoryNamespaces).toEqual({
    runtimeCoverageProfileCount: 19,
    selectedRuntimeCoverageProfileCount: 19,
    statusCounts: { supported: 0, partial: 19, missing: 0, planned: 0 },
    note: "Runtime coverage profiles are not contract kinds, contract mappings, unique AppView variants, or orientation aliases.",
  });
});
