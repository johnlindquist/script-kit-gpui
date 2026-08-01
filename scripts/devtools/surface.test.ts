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
  expect(report.inventoryNamespaces).toEqual({
    contractKindCount: 37,
    contractMappingCount: 54,
    uniqueAppViewVariantCount: 53,
    runtimeCoverageProfileCount: 11,
    orientationAliasCount: 4,
  });
  expect(report.featureMapSource).toEqual({
    path: "FEATURE_MAP.md",
    compatibilityIndexExists: true,
    parsedEntryCount: 0,
    maintainedAtlasPath: "feature-map/index.md",
    maintainedAtlasExists: false,
    status: "compatibility-index-points-to-missing-atlas",
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
  expect(report.inventoryNamespaces).toEqual({
    runtimeCoverageProfileCount: 11,
    selectedRuntimeCoverageProfileCount: 11,
    statusCounts: { supported: 1, partial: 9, missing: 0, planned: 1 },
    note: "Runtime coverage profiles are not contract kinds, contract mappings, unique AppView variants, or orientation aliases.",
  });
});
