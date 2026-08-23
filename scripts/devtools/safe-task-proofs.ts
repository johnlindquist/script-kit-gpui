#!/usr/bin/env bun
/**
 * Fresh, catalog-bound proof for tasks genuinely expressible by offline Bun
 * behavior tests. Runtime interaction, headed browser pixels, Rust-only
 * compiler guarantees, and generated governance ledgers remain blocked.
 */

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  parseTaskCatalog,
} from "./consistency.ts";
import { emitValidatedReceipt, RECEIPT_SCHEMA_VERSION } from "./lib/receipt-schema.ts";
import { taskProofPolicy } from "./lib/task-proof-policy.ts";
import {
  inspectCurrentStateOwnership,
  REQUIRED_STATE_OWNERSHIP_PATHS,
} from "./state-ownership.ts";

type EvidenceClass = "STATIC_INVENTORY" | "UNIT_BEHAVIOR";

export interface SafeTaskSpec {
  taskId: string;
  title: string;
  evidenceClass: EvidenceClass;
  testFiles: string[];
  productionSources?: string[];
}

export const SAFE_TASK_SPECS: readonly SafeTaskSpec[] = [
  {
    taskId: "RPT-001",
    title: "Publish evidence status and corrected inventory language",
    evidenceClass: "STATIC_INVENTORY",
    testFiles: ["scripts/devtools/surfaces-bindings.test.ts", "scripts/devtools/coverage.test.ts"],
  },
  {
    taskId: "PF-001",
    title: "Make DevTools receipt schemas executable",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/receipt-schema.test.ts"],
  },
  {
    taskId: "PF-002",
    title: "Bind each proof stack to one target and generation transaction",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/target-identity.test.ts", "scripts/devtools/runtime-coverage.test.ts"],
  },
  {
    taskId: "PF-003",
    title: "Redact generic semantic and text receipts by default",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/privacy.test.ts", "scripts/devtools/receipt-output.test.ts"],
  },
  {
    taskId: "PF-009",
    title: "Generate typed coverage bindings for all 37 kinds and 54 mappings",
    evidenceClass: "STATIC_INVENTORY",
    testFiles: ["scripts/devtools/surfaces-bindings.test.ts", "scripts/devtools/coverage.test.ts"],
  },
  {
    taskId: "PF-011",
    title: "Make glass observers fail closed without changing motion",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: [
      "scripts/devtools/glass-observers.test.ts",
      "scripts/devtools/glass-entry-motion-contract.test.ts",
    ],
  },
  {
    taskId: "GEO-001",
    title: "Name geometry by semantic layer",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/layout.test.ts"],
  },
  {
    taskId: "GOV-001",
    title: "Freeze state ownership and migrate only legitimate consumers",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/state-ownership.test.ts"],
    productionSources: [
      "scripts/devtools/state-ownership.ts",
      ...REQUIRED_STATE_OWNERSHIP_PATHS,
    ],
  },
  {
    taskId: "GOV-002",
    title: "Delete compatibility façades when callers reach zero",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: [
      "scripts/devtools/facade-ledger.test.ts",
      "scripts/devtools/facade-migrations.test.ts",
    ],
    productionSources: [
      "scripts/devtools/facade-ledger.ts",
      "scripts/devtools/facade-migrations.ts",
      "src/components/conversation_style.rs",
      "src/components/inline_popup_window.rs",
      "src/components/mod.rs",
      "src/ai/agent_chat/ui/components/transcript.rs",
      "src/ai/agent_chat/ui/view.rs",
      "src/ai/agent_chat/ui/chat_window.rs",
      "src/ai/agent_chat/ui/history_popup.rs",
      "src/ai/agent_chat/ui/popup_automation.rs",
      "src/prompts/chat/render_turns.rs",
      "src/design_contract/mod.rs",
      "src/menu_syntax/object_selector.rs",
      "design/mockups/generated/tokens.json",
    ],
  },
  {
    taskId: "GOV-003",
    title: "Introduce explicit authored alpha-byte typing",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/alpha-byte-contract.test.ts"],
    productionSources: [
      "src/theme/alpha.rs",
      "src/theme/types.rs",
      "scripts/devtools/alpha-byte-contract-harness.rs",
    ],
  },
  {
    taskId: "GOV-004",
    title: "Validate owner-map paths and fix Notes Browse",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/coverage.test.ts"],
  },
  {
    taskId: "GOV-005",
    title: "Give every generated design-contract conflict a lifecycle",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: ["scripts/devtools/design-conflicts.test.ts"],
    productionSources: ["design/mockups/generated/tokens.json"],
  },
  {
    taskId: "GOV-006",
    title: "Add a final consistency completion auditor",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: [
      "scripts/devtools/consistency.test.ts",
      "scripts/agentic/cons-flow-ux/final-workflow-audit.test.ts",
    ],
    productionSources: [
      "scripts/devtools/consistency.ts",
      "scripts/agentic/cons-flow-ux/final-workflow-audit.ts",
    ],
  },
  {
    taskId: "GOV-007",
    title: "Reconcile the protected glass veil contradiction without retuning",
    evidenceClass: "UNIT_BEHAVIOR",
    testFiles: [
      "scripts/devtools/glass-entry-motion-contract.test.ts",
      "scripts/devtools/rapid-toggle-stress.test.ts",
    ],
  },
];

function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

export interface BunTestObservation {
  pass: boolean;
  exitCode: number;
  passedTestCount: number;
  failedTestCount: number;
  expectationCount: number;
  suiteFiles: string[];
  executedSuiteFiles?: string[];
  outputSha256: string;
  errors: string[];
}

export function observeBunTestRun(
  output: string,
  exitCode: number,
  expectedFiles: readonly string[],
): BunTestObservation {
  const passed = /^\s*(\d+) pass\s*$/m.exec(output);
  const failed = /^\s*(\d+) fail\s*$/m.exec(output);
  const expectations = /^\s*(\d+) expect\(\) calls\s*$/m.exec(output);
  const errors: string[] = [];
  if (exitCode !== 0) errors.push(`bun test exited ${exitCode}`);
  if (!passed || Number(passed[1]) <= 0) errors.push("no executed passing behavior tests");
  if (!failed || Number(failed[1]) !== 0) errors.push("behavior test failures are present or unreported");
  if (!expectations || Number(expectations[1]) <= 0) {
    errors.push("behavior test expectation count is missing or empty");
  }
  const suiteFiles = expectedFiles.filter((path) => output.includes(`${path}:`));
  for (const path of expectedFiles) {
    if (!suiteFiles.includes(path)) errors.push(`requested suite did not execute: ${path}`);
  }
  return {
    pass: errors.length === 0,
    exitCode,
    passedTestCount: Number(passed?.[1] ?? 0),
    failedTestCount: Number(failed?.[1] ?? -1),
    expectationCount: Number(expectations?.[1] ?? 0),
    suiteFiles,
    outputSha256: sha256(output),
    errors,
  };
}

function testRunnerNegativeControls(expectedFiles: readonly string[]) {
  const healthy =
    `${expectedFiles.map((path) => `${path}:`).join("\n")}\n` +
    " 3 pass\n 0 fail\n 8 expect() calls\n";
  const mutations: Array<[string, string, number, string]> = [
    ["zero-executed-tests", healthy.replace("3 pass", "0 pass"), 0, "no executed"],
    ["one-failed-test", healthy.replace("0 fail", "1 fail"), 0, "failures"],
    ["nonzero-test-process", healthy, 3, "exited 3"],
    ["missing-suite-identity", healthy.replace(`${expectedFiles[0]}:`, "other.test.ts:"), 0, "did not execute"],
    ["missing-expectations", healthy.replace("8 expect() calls", "0 expect() calls"), 0, "expectation count"],
  ];
  return mutations.map(([id, output, exitCode, reason]) => {
    const result = observeBunTestRun(output, exitCode, expectedFiles);
    return {
      id,
      expectedFailure: reason,
      pass: !result.pass && result.errors.some((error) => error.includes(reason)),
    };
  });
}

function protectedGlassAssertions(taskId: string) {
  if (taskId !== "GOV-007" && taskId !== "PF-011") return [];
  const tokens = readFileSync("src/ui/chrome/tokens.rs", "utf8");
  const guidance = readFileSync("AGENTS.md", "utf8");
  const expectedVeil =
    /pub const LIQUID_GLASS_CAPSULE_VEIL_ALPHA:\s*f32\s*=\s*0\.0\s*;/.test(tokens);
  const guidanceMatches = /capsule veil\s+`0\.0`/.test(guidance);
  return [
    { id: "protected-production-capsule-veil-zero", pass: expectedVeil },
    { id: "operator-guidance-matches-protected-veil", pass: guidanceMatches },
  ];
}

export function safeTaskSpec(taskId: string): SafeTaskSpec | null {
  return SAFE_TASK_SPECS.find((spec) => spec.taskId === taskId) ?? null;
}

export function runSafeTaskProof(
  taskId: string,
  options: {
    executedTestFiles?: readonly string[];
    runTests?: (
      testFiles: readonly string[],
      environment: Record<string, string | undefined>,
    ) => { output: string; exitCode: number };
  } = {},
) {
  const spec = safeTaskSpec(taskId);
  if (!spec) {
    throw new Error(
      `${taskId} is not authorized for offline proof; runtime interaction, ` +
        "headed-browser pixels, Rust-only compiler guarantees, or governance ledgers remain blocked",
    );
  }
  const policy = taskProofPolicy(taskId);
  if (!policy || policy.provesRuntimeBehavior || !policy.acceptedEvidenceClasses.includes(spec.evidenceClass)) {
    throw new Error(`${taskId} cannot be discharged by ${spec.evidenceClass}`);
  }

  const catalog = parseTaskCatalog(
    readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
    DEFAULT_CONSISTENCY_CATALOG_PATH,
  );
  const entry = catalog.byId.get(taskId);
  if (!entry || catalog.errors.length > 0 || entry.title !== spec.title) {
    throw new Error(`${taskId} does not match its exact canonical task ID/title/section`);
  }
  const reviewedWorkflowSuite =
    "scripts/agentic/cons-flow-ux/final-workflow-audit.test.ts";
  const reviewedWorkflowOwner =
    "scripts/agentic/cons-flow-ux/final-workflow-audit.ts";
  const ownedBehaviorSuite = (path: string): boolean =>
    path.endsWith(".test.ts") && existsSync(path) && (
      path.startsWith("scripts/devtools/") ||
      (taskId === "GOV-006" && path === reviewedWorkflowSuite)
    );
  for (const path of spec.testFiles) {
    if (!ownedBehaviorSuite(path)) {
      throw new Error(`offline proof suite is not an existing reviewed behavior owner: ${path}`);
    }
  }
  for (const path of spec.productionSources ?? []) {
    if (
      !existsSync(path) ||
      !(path.startsWith("src/") || path.startsWith("scripts/devtools/") ||
        path.startsWith("crates/sk-protocol/src/") ||
        path.startsWith("design/mockups/generated/") ||
        (taskId === "GOV-006" && path === reviewedWorkflowOwner)) ||
      path.includes("..")
    ) {
      throw new Error(`offline proof production source is missing or outside its reviewed owner: ${path}`);
    }
  }
  const executedTestFiles = [...(options.executedTestFiles ?? spec.testFiles)];
  if (
    executedTestFiles.length === 0 ||
    new Set(executedTestFiles).size !== executedTestFiles.length ||
    spec.testFiles.some((path) => !executedTestFiles.includes(path)) ||
    executedTestFiles.some((path) => !ownedBehaviorSuite(path))
  ) {
    throw new Error(
      "the observed test command must contain every required task suite exactly once and only existing reviewed behavior suites",
    );
  }

  const environment = {
    ...process.env,
    SCRIPT_KIT_NONINTERACTIVE: "1",
    SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
    SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
    SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
    SCRIPT_KIT_ALLOW_LIVE_AI: "0",
    SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
    SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
  };
  const rootedTestFiles = executedTestFiles.map((path) => `./${path}`);
  const testResult = options.runTests
    ? options.runTests(executedTestFiles, environment)
    : (() => {
        const result = Bun.spawnSync([process.execPath, "test", ...rootedTestFiles], {
          cwd: process.cwd(),
          env: environment,
          stdout: "pipe",
          stderr: "pipe",
        });
        return {
          output: `${result.stdout.toString()}\n${result.stderr.toString()}`,
          exitCode: result.exitCode,
        };
      })();
  const executedRun = observeBunTestRun(
    testResult.output,
    testResult.exitCode,
    executedTestFiles,
  );
  const observed = {
    ...executedRun,
    suiteFiles: spec.testFiles.filter((path) => executedRun.suiteFiles.includes(path)),
    executedSuiteFiles: executedRun.suiteFiles,
  };
  const negativeControls = testRunnerNegativeControls(spec.testFiles);
  const stateOwnership = taskId === "GOV-001"
    ? inspectCurrentStateOwnership()
    : undefined;
  const assertions = [
    { id: "focused-behavior-tests-passed", pass: observed.pass },
    { id: "catalog-obligation-title-exact", pass: entry.title === spec.title },
    ...(stateOwnership
      ? [
          { id: "bounded-canonical-state-owners-and-consumers-pass", pass: stateOwnership.pass },
          {
            id: "state-ownership-inventory-is-honestly-bounded",
            pass: stateOwnership.sourceGraphExhaustive === false &&
              stateOwnership.externalProcessesStarted === 0 &&
              stateOwnership.provesRuntimeBehavior === false,
          },
          {
            id: "all-sanctioned-ownership-exceptions-preserved",
            pass: Object.values(stateOwnership.sanctionedExceptions).every(Boolean),
          },
        ]
      : []),
    ...protectedGlassAssertions(taskId),
  ];
  const errors = [
    ...observed.errors,
    ...assertions.filter((assertion) => !assertion.pass).map((assertion) => assertion.id),
    ...negativeControls.filter((negative) => !negative.pass).map((negative) => negative.id),
    ...(stateOwnership?.failures.map((failure) => `state-ownership:${failure}`) ?? []),
  ];

  return {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    primitiveId: "devtools.consistency.safe-task-proof",
    tool: "script-kit-devtools.safe-task-proofs",
    command: "safe-task-proofs.verify",
    evidenceClass: spec.evidenceClass,
    provesRuntimeBehavior: false,
    classification: errors.length === 0 ? "ok" : "reproduced",
    taskId,
    taskIds: [taskId],
    catalogBinding: {
      catalogPath: DEFAULT_CONSISTENCY_CATALOG_PATH,
      taskId: entry.id,
      title: entry.title,
      sectionSha256: entry.sectionSha256,
    },
    testCommand: ["bun", "test", ...rootedTestFiles],
    testRun: observed,
    productionSources: [...(spec.productionSources ?? [])],
    sourceFingerprints: Object.fromEntries(
      [...executedTestFiles, ...(spec.productionSources ?? [])]
        .map((path) => [path, sha256(readFileSync(path))]),
    ),
    ...(stateOwnership ? { stateOwnership } : {}),
    assertions,
    negativeControls,
    safety: {
      noninteractive: true,
      startsApplication: false,
      revealsWindow: false,
      focusesWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
      accessesNetwork: false,
      usesLiveAi: false,
    },
    errors,
  };
}

export function runAllSafeTaskProofs() {
  const suites = [...new Set(
    SAFE_TASK_SPECS.flatMap((spec) => spec.testFiles),
  )];
  const environment = {
    ...process.env,
    SCRIPT_KIT_NONINTERACTIVE: "1",
    SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
    SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
    SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
    SCRIPT_KIT_ALLOW_LIVE_AI: "0",
    SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
    SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
  };
  const result = Bun.spawnSync(
    [process.execPath, "test", ...suites.map((path) => `./${path}`)],
    {
    cwd: process.cwd(),
    env: environment,
    stdout: "pipe",
    stderr: "pipe",
    },
  );
  const shared = {
    output: `${result.stdout.toString()}\n${result.stderr.toString()}`,
    exitCode: result.exitCode,
  };
  return SAFE_TASK_SPECS.map((spec) =>
    runSafeTaskProof(spec.taskId, {
      executedTestFiles: suites,
      runTests: () => shared,
    })
  );
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(
      "Usage: bun scripts/devtools/safe-task-proofs.ts <TASK-ID> [--out <receipt.json>] | --list | --all",
    );
  } else if (argv.includes("--list")) {
    console.log(JSON.stringify({
      evidenceClass: "STATIC_INVENTORY",
      safeTaskIds: SAFE_TASK_SPECS.map((spec) => spec.taskId),
      doesNotProveRuntimeBehavior: true,
    }, null, 2));
  } else if (argv.includes("--all")) {
    for (const receipt of runAllSafeTaskProofs()) {
      emitValidatedReceipt(
        "devtools.consistency.safe-task-proof",
        receipt,
        join(".artifacts/consistency", receipt.taskId, "offline-behavior.json"),
      );
    }
  } else {
    const taskId = argv[0];
    if (!taskId || taskId.startsWith("--")) {
      console.error("a safe canonical task ID is required");
      process.exit(64);
    }
    const outIndex = argv.indexOf("--out");
    const outputPath = outIndex >= 0 ? argv[outIndex + 1] : undefined;
    if (outIndex >= 0 && !outputPath) {
      console.error("--out requires a receipt path");
      process.exit(64);
    }
    const receipt = runSafeTaskProof(taskId);
    emitValidatedReceipt(
      "devtools.consistency.safe-task-proof",
      receipt,
      outputPath ?? join(".artifacts/consistency", taskId, "offline-behavior.json"),
    );
  }
}
