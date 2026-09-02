import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative } from "node:path";
import { afterAll, afterEach, describe, expect, test } from "bun:test";
import {
  CONS_FLOW_UX_IDS,
  currentIdentity,
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  parseProgressSections,
  parseTaskCatalog,
  verifyTask,
} from "./consistency.ts";
import { validateReceipt } from "./lib/receipt-schema.ts";
import {
  WORKFLOW_TASK_PRIMITIVE_ID,
  WORKFLOW_TASK_PROOF_SPECS,
  workflowTaskProofSourceOwners,
  type WorkflowTaskProofId,
} from "./lib/workflow-task-contract.ts";
import { createArtifactFixture } from "../agentic/build-artifact-fixture.ts";
import { verifyImmutableArtifact } from "../agentic/build-artifact.ts";
import {
  observedWorkflowSegment,
  observedWorkflowStage,
  prepareBlockedWorkflowTaskProof,
  prepareWorkflowTaskProof,
  writeWorkflowTaskProof,
  type WorkflowTaskProofOptions,
} from "./lib/workflow-task-proof.ts";

type JsonObject = Record<string, unknown>;

const binaryPath = "scripts/devtools/lib/workflow-task-proof.ts";
const binarySha = createHash("sha256").update(readFileSync(binaryPath)).digest("hex");
const head = currentIdentity().headCommit!;
const temporaryDirectories: string[] = [];
let signedBinary: JsonObject | undefined;
let disposeArtifactFixture: (() => void) | undefined;

afterAll(() => disposeArtifactFixture?.());

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

function syntheticBinary(unsigned = false): JsonObject {
  if (unsigned) return { path: binaryPath, sha256: binarySha, sourceCommit: head };
  if (!signedBinary) {
    // Publish once; each candidate owns a deep copy of the same immutable artifact identity.
    // Production proof verification still checks current source and provenance on every call.
    const fixture = createArtifactFixture(process.cwd(), { existingRepository: true, executable: readFileSync(binaryPath, "utf8") });
    disposeArtifactFixture = fixture.dispose;
    const artifact = verifyImmutableArtifact(process.cwd(), fixture.reference, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" });
    signedBinary = { ...artifact.binary, artifactReference: artifact.reference };
  }
  return structuredClone(signedBinary);
}

function segment(id = "observed-session", unsigned = false) {
  const bounds = { x: 0, y: 0, width: 800, height: 600 };
  const transaction = {
    transactionId: `proof:${id}`,
    runId: `run:${id}`,
    pid: process.pid,
    processStartTime: "2026-08-22T00:00:00.000Z",
    binarySha256: binarySha,
    automationId: "main",
    windowInstanceId: "main@1",
    windowGeneration: 1,
    windowKind: "Main",
    hostKind: "mainWindow",
    surfaceKind: "ScriptList",
    semanticSurface: "scriptList",
    appViewVariant: "ScriptList",
    bounds,
    targetGeneration: 1,
    surfaceGeneration: 1,
    dataGeneration: 1,
  };
  return observedWorkflowSegment(id, {
    requestedTarget: { selector: { type: "main" } },
    target: {
      automationId: "main",
      windowInstanceId: "main@1",
      targetGeneration: 1,
      surfaceGeneration: 1,
      dataGeneration: 1,
      pid: process.pid,
      visible: false,
      bounds,
    },
    transaction,
    binary: syntheticBinary(unsigned),
  }, {
    processExited: true,
    streamsDrained: true,
    logWriterClosed: true,
    ownedProcessCount: 0,
    closeError: null,
    clipboardTouched: false,
  });
}

function options(taskId: WorkflowTaskProofId, unsigned = false): WorkflowTaskProofOptions {
  const spec = WORKFLOW_TASK_PROOF_SPECS[taskId];
  const observed = segment(`session-${taskId.toLowerCase()}`, unsigned);
  return {
    producerOwner: spec.producerOwner,
    segments: [observed],
    stages: spec.stageIds.map((id, index) => observedWorkflowStage({
      id,
      primitiveId: "devtools.act",
      segment: observed,
      command: "synthetic.observed-action",
      requestId: `${taskId}:request:${index}`,
      result: { id, changed: true },
      pass: true,
    })),
    negativeControls: Object.fromEntries(
      spec.negativeControlIds.map((id) => [id, true]),
    ),
    safety: {
      microphoneCaptureStarted: false,
      nativeInputInjected: false,
      liveAiStarted: false,
      screenTakeoverStarted: false,
      clipboardTouched: false,
    },
  };
}

function mutate(taskId: WorkflowTaskProofId, update: (value: WorkflowTaskProofOptions) => void) {
  const value = options(taskId);
  update(value);
  return () => prepareWorkflowTaskProof(taskId, value);
}

describe("source-bound canonical safety and workflow task proofs", () => {
  test("an unsigned existing executable cannot borrow current HEAD as build provenance", () => {
    expect(() => prepareWorkflowTaskProof("SAFE-001", options("SAFE-001", true)))
      .toThrow();
  });

  test("all 28 tasks have one exact executable owner and task-specific observed stages", () => {
    expect(new Set(Object.keys(WORKFLOW_TASK_PROOF_SPECS))).toEqual(new Set(CONS_FLOW_UX_IDS));
    for (const [taskId, spec] of Object.entries(WORKFLOW_TASK_PROOF_SPECS)) {
      expect(spec.producerOwner.startsWith("scripts/agentic/cons-flow-ux/")).toBe(true);
      expect(spec.stageIds.length).toBeGreaterThanOrEqual(2);
      expect(new Set(spec.stageIds).size).toBe(spec.stageIds.length);
      expect(spec.negativeControlIds.length).toBeGreaterThanOrEqual(2);
      expect(new Set(spec.negativeControlIds).size).toBe(spec.negativeControlIds.length);
      expect(workflowTaskProofSourceOwners(taskId as WorkflowTaskProofId)).toContain(spec.producerOwner);
    }
    expect(workflowTaskProofSourceOwners("WF-013")).toContain(
      "scripts/agentic/day-page-context-roundtrip-probe.ts",
    );
    expect(workflowTaskProofSourceOwners("WF-014")).toContain(
      "scripts/agentic/day-page-agent-chat-handoff-scope-probe.ts",
    );
    expect(workflowTaskProofSourceOwners("WF-015")).toContain(
      "scripts/agentic/day-agent-chat-return-probe.ts",
    );
  });

  test.each(Object.keys(WORKFLOW_TASK_PROOF_SPECS) as WorkflowTaskProofId[])(
    "%s accepts only its canonical source-bound synthetic journey",
    (taskId) => {
      const directory = mkdtempSync(join(tmpdir(), `workflow-task-proof-${taskId}-`));
      temporaryDirectories.push(directory);
      const catalog = parseTaskCatalog(
        readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
        DEFAULT_CONSISTENCY_CATALOG_PATH,
      );
      const progress = parseProgressSections(readFileSync(".notes/CONSISTENCY-PROGRESS.md", "utf8"));
      const prepared = prepareWorkflowTaskProof(taskId, options(taskId));
      expect(prepared.exitCode).toBe(0);
      expect(prepared.receipt.primitiveId).toBe(WORKFLOW_TASK_PRIMITIVE_ID);
      expect(validateReceipt(WORKFLOW_TASK_PRIMITIVE_ID, prepared.receipt).valid).toBe(true);
      const taskDirectory = join(directory, taskId);
      mkdirSync(taskDirectory, { recursive: true });
      writeFileSync(join(taskDirectory, "proof.json"), JSON.stringify(prepared.receipt));
      const actual = verifyTask({
        taskId,
        scope: "cons-flow-ux",
        receiptsRoot: directory,
        catalog,
        progress,
        current: currentIdentity(),
      });
      expect(actual.exitCode, JSON.stringify({
        taskId,
        disposition: actual.receipt.disposition,
        errors: actual.receipt.errors,
        staleReasons: actual.receipt.staleReasons,
      })).toBe(0);
      expect(actual.receipt.disposition).toBe("EVALUABLE_PASS");
    },
  );

  test("another workflow producer cannot claim the same catalog obligation", () => {
    expect(mutate("SAFE-001", (value) => {
      value.producerOwner = WORKFLOW_TASK_PROOF_SPECS["SAFE-002"].producerOwner;
    })).toThrow("exact reviewed runtime owner");
  });

  test.each<[
    string,
    (value: WorkflowTaskProofOptions) => void,
    string,
  ]>([
    ["missing", (value) => { value.stages.pop(); }, "required stage: cart-delete-failure"],
    ["duplicate", (value) => {
      value.stages[1] = { ...value.stages[0]! };
    }, "unique stable identities"],
    ["failed", (value) => { value.stages[0]!.pass = false; }, "observed registered target transaction"],
  ])("%s actual journey stages cannot pass", (_name, update, expectedError) => {
    expect(mutate("WF-016", update)).toThrow(expectedError);
  });

  test.each<[
    string,
    (value: WorkflowTaskProofOptions) => void,
    string,
  ]>([
    ["missing", (value) => {
      delete (value.negativeControls as Record<string, boolean>)["delete-requires-explicit-confirmation"];
    }, "required adversarial control: delete-requires-explicit-confirmation"],
    ["failed", (value) => {
      (value.negativeControls as Record<string, boolean>)["delete-requires-explicit-confirmation"] = false;
    }, "required adversarial control: delete-requires-explicit-confirmation"],
    ["unexecuted", (value) => {
      value.negativeControls = WORKFLOW_TASK_PROOF_SPECS["SAFE-003"].negativeControlIds.map((control) => ({
        id: control,
        pass: true,
        executed: control !== "delete-requires-explicit-confirmation",
      }));
    }, "required adversarial control: delete-requires-explicit-confirmation"],
    ["duplicate", (value) => {
      const controls = WORKFLOW_TASK_PROOF_SPECS["SAFE-003"].negativeControlIds.map((control) => ({
        id: control,
        pass: true,
        executed: true,
      }));
      value.negativeControls = [...controls, { ...controls[0]! }];
    }, "workflow negative controls must have unique stable identities"],
  ])("%s adversarial controls cannot pass", (_name, update, expectedError) => {
    expect(mutate("SAFE-003", update)).toThrow(expectedError);
  });

  test.each<[
    string,
    (value: WorkflowTaskProofOptions) => void,
    string,
  ]>([
    ["foreign session", (value) => {
      value.stages[0]!.segmentId = "foreign-session";
    }, "observed registered target transaction"],
    ["foreign process", (value) => {
      (value.stages[0]!.transaction as JsonObject).pid = -1;
    }, "target transaction"],
    ["foreign data generation", (value) => {
      (value.stages[0]!.transaction as JsonObject).dataGeneration = 100;
    }, "target transaction"],
    ["unregistered primitive", (value) => {
      value.stages[0]!.primitiveId = "devtools.fake.success";
    }, "observed registered target transaction"],
    ["reused request", (value) => {
      (value.stages[1]!.observation as JsonObject).requestId =
        (value.stages[0]!.observation as JsonObject).requestId;
    }, "unique actual command/result observation"],
    ["forged result", (value) => {
      (value.stages[0]!.observation as JsonObject).resultSha256 = "forged";
    }, "unique actual command/result observation"],
  ])("stages reject %s", (_name, update, expectedError) => {
    expect(mutate("WF-020", update)).toThrow(expectedError);
  });

  test("missing or mismatched source-bound binary and runtime generation fail closed", () => {
    expect(mutate("WF-021", (value) => {
      value.segments[0]!.binary.sha256 = "0".repeat(64);
    })).toThrow("current-source binary bytes");
    expect(mutate("WF-021", (value) => {
      value.segments[0]!.target.dataGeneration = 100;
    })).toThrow("matching target/process identity");
  });

  test.each([
    "microphoneCaptureStarted",
    "nativeInputInjected",
    "liveAiStarted",
    "screenTakeoverStarted",
    "clipboardTouched",
  ] as const)("unsafe %s refuses proof", (field) => {
    expect(mutate("WF-024", (value) => { value.safety[field] = true; }))
      .toThrow("microphone, input, AI, desktop, and clipboard safety");
  });

  test("unowned or incomplete process cleanup cannot become a valid segment", () => {
    const observed = segment();
    expect(() => observedWorkflowSegment("bad-cleanup", observed, {
      ...observed.cleanup,
      ownedProcessCount: 1,
    })).toThrow("owned-process/privacy cleanup");
    expect(mutate("WF-022", (value) => {
      value.segments[0]!.cleanup.processExited = false;
    })).toThrow("safe owned-process cleanup");
  });

  test("private observed bytes become result fingerprints, never serialized cleartext", () => {
    const observed = segment();
    const stage = observedWorkflowStage({
      id: "private-context",
      primitiveId: "devtools.elements.snapshot",
      segment: observed,
      command: "inspect.private-context",
      requestId: "private-request",
      result: { rawContent: "PRIVATE_WORKFLOW_CANARY_DO_NOT_PERSIST" },
      pass: true,
    });
    expect(JSON.stringify(stage)).not.toContain("PRIVATE_WORKFLOW_CANARY_DO_NOT_PERSIST");
    expect((stage.observation as JsonObject).resultSha256).toMatch(/^[a-f0-9]{64}$/);
  });

  test("unobserved workflows remain typed blocked instead of fabricated green", () => {
    const prepared = prepareBlockedWorkflowTaskProof("SAFE-004", "no actual user journey executed");
    expect(prepared.exitCode).toBe(3);
    expect(prepared.receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(prepared.receipt.pass).toBe(false);
    expect(JSON.stringify(prepared.receipt)).not.toContain("no actual user journey executed");
  });

  test("canonical publication cannot overwrite a different obligation", () => {
    const directory = mkdtempSync(join(tmpdir(), "workflow-task-publish-"));
    temporaryDirectories.push(directory);
    const prepared = prepareWorkflowTaskProof("SAFE-001", options("SAFE-001"));
    expect(() => writeWorkflowTaskProof("SAFE-002", prepared.receipt, directory))
      .toThrow("unrelated or unregistered workflow receipt");
    const path = writeWorkflowTaskProof("SAFE-001", prepared.receipt, directory);
    expect(path).toBe(join(directory, "SAFE-001", "workflow-proof.json"));
    expect(JSON.parse(readFileSync(path, "utf8")).taskId).toBe("SAFE-001");
  });

  test("stale workflow producer bytes cannot survive independent final auditing", () => {
    const directory = mkdtempSync(join(tmpdir(), "workflow-task-stale-"));
    temporaryDirectories.push(directory);
    const taskId = "WF-024";
    const prepared = prepareWorkflowTaskProof(taskId, options(taskId));
    writeWorkflowTaskProof(taskId, prepared.receipt, directory);
    const catalog = parseTaskCatalog(readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"));
    const progress = parseProgressSections(readFileSync(".notes/CONSISTENCY-PROGRESS.md", "utf8"));
    const actual = verifyTask({
      taskId,
      scope: "cons-flow-ux",
      receiptsRoot: directory,
      catalog,
      progress,
      current: currentIdentity({
        fileSha256: (path) => path === WORKFLOW_TASK_PROOF_SPECS[taskId].producerOwner
          ? "0".repeat(64)
          : createHash("sha256").update(readFileSync(path)).digest("hex"),
      }),
    });
    expect(actual.exitCode).toBe(3);
    expect(actual.receipt.disposition).toBe("BLOCKED_STALE_GENERATION");
    expect(JSON.stringify(actual.receipt)).toContain("stale-workflow-proof-source");
  });

  test("composed Notes/Today proof cannot hide a stale actual child owner", () => {
    const directory = mkdtempSync(join(tmpdir(), "workflow-task-stale-child-"));
    temporaryDirectories.push(directory);
    const taskId = "WF-013";
    const prepared = prepareWorkflowTaskProof(taskId, options(taskId));
    writeWorkflowTaskProof(taskId, prepared.receipt, directory);
    const current = currentIdentity({
      fileSha256: (path) => path === "scripts/agentic/day-page-context-roundtrip-probe.ts"
        ? "0".repeat(64)
        : createHash("sha256").update(readFileSync(path)).digest("hex"),
    });
    const result = verifyTask({
      taskId,
      scope: "cons-flow-ux",
      receiptsRoot: directory,
      catalog: parseTaskCatalog(readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8")),
      progress: parseProgressSections(readFileSync(".notes/CONSISTENCY-PROGRESS.md", "utf8")),
      current,
    });
    expect(result.exitCode).toBe(3);
    expect(JSON.stringify(result.receipt)).toContain("stale-workflow-proof-source");
  });
});
