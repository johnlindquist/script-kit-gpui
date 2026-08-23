#!/usr/bin/env bun

import { mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";
import {
  CONS_FLOW_UX_IDS,
  currentIdentity,
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  PROGRAM_IDS,
  verifyScope,
} from "../../devtools/consistency.ts";
import { validateReceipt } from "../../devtools/lib/receipt-schema.ts";

type JsonObject = Record<string, unknown>;

const root = resolve(import.meta.dir, "../../..");
const DEFAULT_RECEIPTS_ROOT = ".artifacts/consistency";
const DEFAULT_PROGRESS_PATH = ".notes/CONSISTENCY-PROGRESS.md";
const DEFAULT_OUTPUT_PATH =
  ".artifacts/consistency/cons-flow-ux/final-audit/current/lane-receipt.json";

export interface WorkflowAuditOptions {
  catalogPath: string;
  progressPath: string;
  receiptsRoot: string;
  outputPath: string;
  writeOutput: boolean;
}

export class WorkflowAuditError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WorkflowAuditError";
  }
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new WorkflowAuditError(message);
}

function object(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : {};
}

function sorted(values: Iterable<string>): string[] {
  return [...values].sort();
}

function equalStrings(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function summarizeWorkflowScope(
  scopeReceipt: JsonObject,
  expectedHead: string,
): JsonObject {
  assert(/^[a-f0-9]{40}$/.test(expectedHead), "workflow audit requires the exact current 40-character source commit");
  assert(scopeReceipt.scope === "cons-flow-ux", "workflow audit requires the canonical cons-flow-ux scope");
  assert(scopeReceipt.headCommit === expectedHead, "workflow scope source commit is stale or does not match the exact current HEAD");
  assert(scopeReceipt.catalogTaskCount === PROGRAM_IDS.size, "workflow scope requires the complete canonical 75-task catalog");
  assert(scopeReceipt.scopeTaskCount === CONS_FLOW_UX_IDS.size, "workflow scope must contain all 28 canonical SAFE/WF tasks");
  for (const fabricatedField of ["focusedMatrix", "governance", "oracleConsultCount", "productCommit"]) {
    assert(
      !Object.prototype.hasOwnProperty.call(scopeReceipt, fabricatedField),
      `workflow scope must not inherit fabricated legacy ${fabricatedField} evidence`,
    );
  }

  const validation = validateReceipt("devtools.consistency.verify-scope", scopeReceipt);
  assert(validation.valid, `workflow scope failed canonical receipt validation: ${validation.errors.join("; ")}`);
  assert(object(scopeReceipt.producerValidation).valid === true, "workflow scope producer validation did not pass");

  const dispositions = object(scopeReceipt.taskDispositions);
  const expectedIds = sorted(CONS_FLOW_UX_IDS);
  const observedIds = sorted(Object.keys(dispositions));
  assert(equalStrings(observedIds, expectedIds), "workflow scope does not contain the exact 28 canonical SAFE/WF task identities");

  const passedIds = expectedIds.filter((taskId) => dispositions[taskId] === "EVALUABLE_PASS");
  const outstandingIds = expectedIds.filter((taskId) => dispositions[taskId] !== "EVALUABLE_PASS");
  assert(scopeReceipt.scopePassedTaskCount === passedIds.length, "workflow scope passing count does not match actual task dispositions");

  const declaredMissing = Array.isArray(scopeReceipt.missingScopeTaskIds)
    ? sorted(scopeReceipt.missingScopeTaskIds.filter((value): value is string => typeof value === "string"))
    : [];
  assert(declaredMissing.every((taskId) => outstandingIds.includes(taskId)), "workflow scope reports a passing or unknown task as missing");

  const complete = outstandingIds.length === 0;
  assert(
    (scopeReceipt.disposition === "EVALUABLE_PASS" && scopeReceipt.pass === true) === complete,
    "workflow scope claims a passing verdict without all 28 canonical runtime tasks",
  );

  return {
    ...scopeReceipt,
    auditOwner: "scripts/agentic/cons-flow-ux/final-workflow-audit.ts",
    verdict: complete
      ? "PASS"
      : String(scopeReceipt.disposition).startsWith("INVALID_")
        ? "INVALID"
        : "BLOCKED",
    sourceCommit: expectedHead,
    taskCoverage: {
      expected: expectedIds.length,
      passed: passedIds.length,
      taskIds: expectedIds,
      passedTaskIds: passedIds,
      outstandingTaskIds: outstandingIds,
      missingTaskIds: declaredMissing,
    },
    runtimeProof: {
      requiredTaskCount: expectedIds.length,
      provenTaskCount: passedIds.length,
      outstandingTaskIds: outstandingIds,
    },
  };
}

export function parseWorkflowAuditArgs(argv: string[]): WorkflowAuditOptions {
  const options: WorkflowAuditOptions = {
    catalogPath: DEFAULT_CONSISTENCY_CATALOG_PATH,
    progressPath: DEFAULT_PROGRESS_PATH,
    receiptsRoot: DEFAULT_RECEIPTS_ROOT,
    outputPath: DEFAULT_OUTPUT_PATH,
    writeOutput: true,
  };
  const paths = {
    "--catalog": "catalogPath",
    "--progress": "progressPath",
    "--receipts": "receiptsRoot",
    "--out": "outputPath",
  } as const;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]!;
    if (arg === "--no-write") {
      options.writeOutput = false;
      continue;
    }
    const key = paths[arg as keyof typeof paths];
    assert(key, `unknown workflow-audit argument: ${arg}`);
    const value = argv[++index];
    assert(value && !value.startsWith("--"), `${arg} requires a path`);
    options[key] = value;
  }

  return options;
}

function writeAtomic(path: string, receipt: JsonObject): void {
  const absolute = resolve(root, path);
  const directory = dirname(absolute);
  mkdirSync(directory, { recursive: true });
  const temporary = resolve(directory, `.${basename(absolute)}.tmp-${process.pid}`);
  try {
    writeFileSync(temporary, `${JSON.stringify(receipt, null, 2)}\n`, {
      mode: 0o600,
      flag: "wx",
    });
    renameSync(temporary, absolute);
  } finally {
    rmSync(temporary, { force: true });
  }
}

export function runWorkflowAudit(options: WorkflowAuditOptions): {
  receipt: JsonObject;
  exitCode: number;
} {
  const current = currentIdentity();
  assert(current.headCommit !== null, "workflow audit cannot identify the exact current source commit");
  const result = verifyScope({
    scope: "cons-flow-ux",
    fixesPath: resolve(root, options.catalogPath),
    progressPath: resolve(root, options.progressPath),
    receiptsRoot: resolve(root, options.receiptsRoot),
    current,
  });
  const receipt = summarizeWorkflowScope(result.receipt, current.headCommit);
  if (options.writeOutput) writeAtomic(options.outputPath, receipt);
  return { receipt, exitCode: result.exitCode };
}

if (import.meta.main) {
  const argv = Bun.argv.slice(2);
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(
      "Usage: bun scripts/agentic/cons-flow-ux/final-workflow-audit.ts " +
      "[--catalog path] [--progress path] [--receipts path] [--out path] [--no-write]",
    );
  } else {
    try {
      const { receipt, exitCode } = runWorkflowAudit(parseWorkflowAuditArgs(argv));
      console.log(JSON.stringify(receipt, null, 2));
      process.exitCode = exitCode;
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 4;
    }
  }
}
