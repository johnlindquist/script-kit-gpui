import { describe, expect, test } from "bun:test";
import {
  observeBunTestRun,
  runSafeTaskProof,
  SAFE_TASK_SPECS,
  safeTaskSpec,
} from "./safe-task-proofs.ts";
import { prepareValidatedReceipt } from "./lib/receipt-schema.ts";
import { taskProofPolicy } from "./lib/task-proof-policy.ts";

describe("catalog-bound offline consistency task proofs", () => {
  function observedProof(taskId: string) {
    return runSafeTaskProof(taskId, {
      runTests: (files, environment) => {
        expect(environment.SCRIPT_KIT_NONINTERACTIVE).toBe("1");
        expect(environment.SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH).toBe("0");
        expect(environment.SCRIPT_KIT_ALLOW_VISIBLE_PROBES).toBe("0");
        expect(environment.SCRIPT_KIT_ALLOW_LIVE_AI).toBe("0");
        expect(environment.SCRIPT_KIT_ALLOW_NATIVE_INPUT).toBe("0");
        expect(environment.SCRIPT_KIT_ALLOW_SCREEN_CAPTURE).toBe("0");
        return {
          output:
            `${files.map((path) => `${path}:`).join("\n")}\n` +
            " 5 pass\n 0 fail\n 11 expect() calls\n",
          exitCode: 0,
        };
      },
    });
  }
  test("only genuinely offline inventory and behavior obligations are authorized", () => {
    expect(SAFE_TASK_SPECS.map((spec) => spec.taskId)).toEqual([
      "RPT-001",
      "PF-001",
      "PF-002",
      "PF-003",
      "PF-009",
      "PF-011",
      "GEO-001",
      "GOV-001",
      "GOV-002",
      "GOV-003",
      "GOV-004",
      "GOV-005",
      "GOV-006",
      "GOV-007",
    ]);
    for (const taskId of ["SAFE-001", "UX-001", "WF-001", "GEO-002", "PF-012"]) {
      expect(safeTaskSpec(taskId)).toBeNull();
      expect(() => runSafeTaskProof(taskId)).toThrow("not authorized for offline proof");
    }
    expect(taskProofPolicy("PF-012")?.requirement).toBe("direct-runtime");
  });

  test("production governance behavior carries the actual owning source fingerprints", () => {
    const ownership = observedProof("GOV-001");
    expect(ownership.productionSources).toContain(
      "scripts/devtools/state-ownership.ts",
    );
    expect(ownership.productionSources).toContain(
      "crates/sk-protocol/src/command_contract.rs",
    );
    expect(ownership.productionSources).toContain(
      "crates/sk-protocol/src/ascii_search.rs",
    );
    expect(ownership.productionSources).toContain("src/scripts/search/ascii.rs");
    expect(ownership.productionSources).toContain(
      "crates/sk-protocol/src/filter_coalescer.rs",
    );
    expect(ownership.productionSources).toContain("src/filter_coalescer.rs");
    expect(ownership.productionSources).toContain(
      "src/scripts/root_search_contract.rs",
    );
    expect(ownership.productionSources).toContain(
      "src/app_impl/filtering_cache.rs",
    );
    expect(
      ownership.sourceFingerprints["crates/sk-protocol/src/command_contract.rs"],
    ).toMatch(/^[a-f0-9]{64}$/);
    expect(ownership.stateOwnership).toMatchObject({
      pass: true,
      coverageMode: "BOUNDED_NAMED_OWNERS_AND_CONSUMERS",
      sourceGraphExhaustive: false,
      provesRuntimeBehavior: false,
      externalProcessesStarted: 0,
    });
    expect(
      prepareValidatedReceipt(
        "devtools.consistency.safe-task-proof",
        ownership,
      ).exitCode,
    ).toBe(0);

    const migration = observedProof("GOV-002");
    expect(migration.productionSources).toContain(
      "scripts/devtools/facade-ledger.ts",
    );
    expect(migration.productionSources).toContain(
      "scripts/devtools/facade-migrations.ts",
    );
    expect(migration.productionSources).toContain("src/components/conversation_style.rs");
    expect(migration.productionSources).toContain(
      "src/components/inline_popup_window.rs",
    );
    expect(migration.productionSources).toContain(
      "src/ai/agent_chat/ui/popup_automation.rs",
    );
    expect(migration.testRun.suiteFiles).toEqual([
      "scripts/devtools/facade-ledger.test.ts",
      "scripts/devtools/facade-migrations.test.ts",
    ]);
    expect(migration.sourceFingerprints["src/components/conversation_style.rs"])
      .toMatch(/^[a-f0-9]{64}$/);
    expect(
      migration.sourceFingerprints["src/components/inline_popup_window.rs"],
    ).toMatch(/^[a-f0-9]{64}$/);
    expect(
      prepareValidatedReceipt(
        "devtools.consistency.safe-task-proof",
        migration,
      ).exitCode,
    ).toBe(0);
    const alpha = observedProof("GOV-003");
    expect(alpha.productionSources).toContain("src/theme/alpha.rs");
    expect(alpha.sourceFingerprints["src/theme/alpha.rs"]).toMatch(/^[a-f0-9]{64}$/);
    expect(
      prepareValidatedReceipt("devtools.consistency.safe-task-proof", alpha).exitCode,
    ).toBe(0);

    const conflicts = observedProof("GOV-005");
    expect(conflicts.productionSources).toEqual([
      "design/mockups/generated/tokens.json",
    ]);
    expect(
      conflicts.sourceFingerprints["design/mockups/generated/tokens.json"],
    ).toMatch(/^[a-f0-9]{64}$/);
  });

  test("green-looking output with zero tests, failures, no expectations, or missing suites fails", () => {
    const suite = "scripts/devtools/layout.test.ts";
    const good = `${suite}:\n 3 pass\n 0 fail\n 8 expect() calls\n`;
    expect(observeBunTestRun(good, 0, [suite]).pass).toBe(true);
    for (const [output, exitCode] of [
      [good.replace("3 pass", "0 pass"), 0],
      [good.replace("0 fail", "1 fail"), 0],
      [good.replace("8 expect() calls", "0 expect() calls"), 0],
      [good.replace(`${suite}:`, "other.test.ts:"), 0],
      [good, 2],
    ] as Array<[string, number]>) {
      expect(observeBunTestRun(output, exitCode, [suite]).pass).toBe(false);
    }
  });

  test("an observed headless geometry-role suite produces canonical UNIT_BEHAVIOR evidence", () => {
    const candidate = observedProof("GEO-001");
    const result = prepareValidatedReceipt(
      "devtools.consistency.safe-task-proof",
      candidate,
    );
    expect(result.exitCode).toBe(0);
    expect(result.receipt.disposition).toBe("EVALUABLE_PASS");
    expect(result.receipt.evidenceClass).toBe("UNIT_BEHAVIOR");
    expect(result.receipt.provesRuntimeBehavior).toBe(false);
    expect(result.receipt.taskId).toBe("GEO-001");
    expect((result.receipt.catalogBinding as Record<string, unknown>).title).toBe(
      "Name geometry by semantic layer",
    );
    expect((result.receipt.testRun as Record<string, number>).passedTestCount).toBeGreaterThan(0);
    expect((result.receipt.negativeControls as unknown[]).length).toBe(5);
  });

  test("shared task runs report and fingerprint the complete actually executed command", () => {
    const suites = [
      "scripts/devtools/layout.test.ts",
      "scripts/devtools/coverage.test.ts",
    ];
    const candidate = runSafeTaskProof("GEO-001", {
      executedTestFiles: suites,
      runTests: (executed) => ({
        output:
          `${executed.map((path) => `${path}:`).join("\n")}\n` +
          " 7 pass\n 0 fail\n 13 expect() calls\n",
        exitCode: 0,
      }),
    });
    expect(candidate.testCommand).toEqual([
      "bun",
      "test",
      ...suites.map((path) => `./${path}`),
    ]);
    expect(candidate.testRun.suiteFiles).toEqual(["scripts/devtools/layout.test.ts"]);
    expect(candidate.testRun.executedSuiteFiles).toEqual(suites);
    expect(Object.keys(candidate.sourceFingerprints)).toEqual(suites);
    expect(
      prepareValidatedReceipt("devtools.consistency.safe-task-proof", candidate).exitCode,
    ).toBe(0);

    for (const forged of [
      { ...candidate, testCommand: ["bun", "test", ...suites] },
      { ...candidate, testCommand: ["bun", "test", suites[0]] },
      {
        ...candidate,
        testRun: { ...candidate.testRun, executedSuiteFiles: [suites[0]] },
      },
      {
        ...candidate,
        sourceFingerprints: { [suites[0]]: candidate.sourceFingerprints[suites[0]] },
      },
      {
        ...candidate,
        sourceFingerprints: {
          ...candidate.sourceFingerprints,
          [suites[0]]: "a".repeat(64),
        },
      },
    ]) {
      expect(
        prepareValidatedReceipt("devtools.consistency.safe-task-proof", forged).exitCode,
      ).not.toBe(0);
    }
  });

  test("shared task commands cannot omit required suites or add unowned paths", () => {
    for (const executedTestFiles of [
      ["scripts/devtools/coverage.test.ts"],
      ["scripts/devtools/layout.test.ts", "src/theme/alpha.rs"],
      ["scripts/devtools/layout.test.ts", "scripts/devtools/layout.test.ts"],
    ]) {
      expect(() =>
        runSafeTaskProof("GEO-001", {
          executedTestFiles,
          runTests: () => ({ output: "should never run", exitCode: 0 }),
        })
      ).toThrow("observed test command");
    }
  });

  test("registry rejects fake runtime completion, wrong task section, and unsafe execution", () => {
    const baseline = observedProof("GEO-001");
    const unsafe = [
      { evidenceClass: "RUNTIME_HIDDEN" },
      { taskId: "UX-001", taskIds: ["UX-001"] },
      {
        catalogBinding: {
          ...baseline.catalogBinding,
          taskId: "GOV-007",
        },
      },
      {
        testRun: {
          ...baseline.testRun,
          passedTestCount: 0,
        },
      },
      {
        safety: {
          ...baseline.safety,
          startsApplication: true,
        },
      },
      { negativeControls: [{ id: "invalid", pass: false }] },
    ];
    for (const override of unsafe) {
      const result = prepareValidatedReceipt(
        "devtools.consistency.safe-task-proof",
        { ...baseline, ...override },
      );
      expect(result.receipt.disposition).toBe("INVALID_SCHEMA");
    }
  });

  test("a one-facade GOV-002 proof cannot omit the popup owner or behavior suite", () => {
    const complete = observedProof("GOV-002");
    const missingOwner = {
      ...complete,
      productionSources: complete.productionSources.filter(
        (path) => path !== "src/components/inline_popup_window.rs",
      ),
    };
    expect(
      prepareValidatedReceipt(
        "devtools.consistency.safe-task-proof",
        missingOwner,
      ).exitCode,
    ).not.toBe(0);

    const legacySuite = complete.testRun.suiteFiles.filter(
      (path) => path !== "scripts/devtools/facade-migrations.test.ts",
    );
    const missingPopupSuite = {
      ...complete,
      testRun: {
        ...complete.testRun,
        suiteFiles: legacySuite,
        executedSuiteFiles: legacySuite,
      },
      testCommand: ["bun", "test", ...legacySuite.map((path) => `./${path}`)],
    };
    expect(
      prepareValidatedReceipt(
        "devtools.consistency.safe-task-proof",
        missingPopupSuite,
      ).exitCode,
    ).not.toBe(0);
  });
});
