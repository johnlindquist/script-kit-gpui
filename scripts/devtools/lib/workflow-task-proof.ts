import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import {
  currentIdentity,
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  parseTaskCatalog,
} from "../consistency.ts";
import {
  prepareValidatedReceipt,
  producerIdentityForTool,
  RECEIPT_SCHEMA_VERSION,
  receiptRegistryIdentity,
} from "./receipt-schema.ts";
import {
  WORKFLOW_STAGE_PRIMITIVES,
  WORKFLOW_TASK_PRIMITIVE_ID,
  WORKFLOW_TASK_PROOF_MODE,
  WORKFLOW_TASK_PROOF_SPECS,
  workflowTaskProofSourceOwners,
  type WorkflowTaskProofId,
} from "./workflow-task-contract.ts";
import {
  observeRuntimeTaskTarget,
  verifyRuntimeBinaryProvenance,
  type RuntimeTargetObservation,
} from "./runtime-task-proof.ts";

type JsonObject = Record<string, unknown>;

export type WorkflowNegativeControls = Record<string, boolean> | Array<{
  id: string;
  pass: boolean;
  executed: boolean;
}>;

export interface WorkflowObservedSegment {
  id: string;
  runId: string;
  requestedTarget: JsonObject;
  target: JsonObject;
  transaction: JsonObject;
  binary: JsonObject;
  cleanup: JsonObject;
}

export interface WorkflowStageObservation {
  id: string;
  primitiveId: string;
  segment: WorkflowObservedSegment;
  command: string;
  requestId: string;
  result: unknown;
  pass: boolean;
}

export interface WorkflowTaskProofOptions {
  producerOwner: string;
  segments: WorkflowObservedSegment[];
  stages: JsonObject[];
  negativeControls: WorkflowNegativeControls;
  safety: JsonObject;
}

function object(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : {};
}

function fingerprint(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

function fingerprintFile(path: string): string {
  return fingerprint(readFileSync(path));
}

function sourceCommit(): string {
  const head = currentIdentity().headCommit;
  if (!head || !/^[a-f0-9]{40}$/.test(head)) {
    throw new Error("workflow proof requires the exact current Git source commit");
  }
  return head;
}

function catalogBinding(taskId: WorkflowTaskProofId): JsonObject {
  const catalog = parseTaskCatalog(
    readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
    DEFAULT_CONSISTENCY_CATALOG_PATH,
  );
  const task = catalog.byId.get(taskId);
  if (!task || catalog.errors.length > 0) {
    throw new Error(`${taskId} does not resolve to one exact current catalog obligation`);
  }
  return {
    catalogPath: DEFAULT_CONSISTENCY_CATALOG_PATH,
    taskId,
    title: task.title,
    sectionSha256: task.sectionSha256,
  };
}

export const observeWorkflowTaskTarget = observeRuntimeTaskTarget;

/** Capture identity while the real driver is live; finalize only after owned cleanup. */
export function observedWorkflowSegment(
  id: string,
  observation: RuntimeTargetObservation,
  cleanupValue: unknown,
): WorkflowObservedSegment {
  const transaction = object(observation.transaction);
  const target = object(observation.target);
  const cleanup = object(cleanupValue);
  if (
    typeof id !== "string" || id.length === 0 ||
    typeof transaction.runId !== "string" || transaction.runId.length === 0 ||
    typeof target.visible !== "boolean"
  ) {
    throw new Error("workflow segment requires an actual named live target and driver transaction");
  }
  if (
    cleanup.processExited !== true || cleanup.streamsDrained !== true ||
    cleanup.logWriterClosed !== true || cleanup.ownedProcessCount !== 0 ||
    cleanup.closeError != null ||
    (cleanup.clipboardTouched === true && cleanup.clipboardRestored !== true)
  ) {
    throw new Error(`workflow segment ${id} cannot pass without exact owned-process/privacy cleanup`);
  }
  return {
    id,
    runId: transaction.runId,
    requestedTarget: object(observation.requestedTarget),
    target,
    transaction,
    binary: object(observation.binary),
    cleanup,
  };
}

/** Preserve only a fingerprint of the genuinely observed result, never its private bytes. */
export function observedWorkflowStage(observation: WorkflowStageObservation): JsonObject {
  if (!WORKFLOW_STAGE_PRIMITIVES.has(observation.primitiveId)) {
    throw new Error(`${observation.id} does not use a reviewed registered production primitive`);
  }
  if (
    observation.pass !== true ||
    typeof observation.command !== "string" || observation.command.length === 0 ||
    typeof observation.requestId !== "string" || observation.requestId.length === 0 ||
    observation.result == null
  ) {
    throw new Error(`${observation.id} has no passing independently observed command result`);
  }
  return {
    id: observation.id,
    primitiveId: observation.primitiveId,
    segmentId: observation.segment.id,
    runId: observation.segment.runId,
    transaction: { ...observation.segment.transaction },
    pass: true,
    observation: {
      command: observation.command,
      requestId: observation.requestId,
      resultSha256: fingerprint(JSON.stringify(observation.result)),
    },
  };
}

function controls(value: WorkflowNegativeControls): JsonObject[] {
  return Array.isArray(value)
    ? value.map((control) => ({ ...control }))
    : Object.entries(value).map(([id, pass]) => ({ id, pass, executed: true }));
}

/** Promote only the exact owner, actual observed segments, stages, controls, and source. */
export function prepareWorkflowTaskProof(
  taskId: WorkflowTaskProofId,
  options: WorkflowTaskProofOptions,
) {
  const spec = WORKFLOW_TASK_PROOF_SPECS[taskId];
  if (!spec) throw new Error(`unknown SAFE/WF workflow task: ${taskId}`);
  if (options.producerOwner !== spec.producerOwner) {
    throw new Error(`${taskId} requires its exact reviewed runtime owner: ${spec.producerOwner}`);
  }
  const primary = options.segments[0];
  if (!primary) throw new Error(`${taskId} has no directly observed runtime segment`);
  const binary = object(primary.binary);
  const head = sourceCommit();
  if (
    typeof binary.path !== "string" ||
    typeof binary.sha256 !== "string" ||
    binary.sourceCommit !== head ||
    fingerprintFile(binary.path) !== binary.sha256
  ) {
    throw new Error(`${taskId} requires independently verified current-source binary bytes`);
  }
  try {
    verifyRuntimeBinaryProvenance(binary.path as string, binary);
  } catch (error) {
    throw new Error(`${taskId} requires verified build provenance: ${String(error)}`);
  }
  if (options.segments.some((segment) => object(segment.binary).sha256 !== binary.sha256)) {
    throw new Error(`${taskId} workflow segments cannot mix executable identities`);
  }

  const sourceOwners = workflowTaskProofSourceOwners(taskId);
  const sourceFingerprints = Object.fromEntries(
    sourceOwners.map((owner) => [owner, fingerprintFile(owner)]),
  );
  const producerIdentity = producerIdentityForTool("script-kit-devtools.workflow-proof");
  const prepared = prepareValidatedReceipt(WORKFLOW_TASK_PRIMITIVE_ID, {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    tool: "script-kit-devtools.workflow-proof",
    command: "workflow.prove",
    classification: "ok",
    taskId,
    taskIds: [taskId],
    catalogBinding: catalogBinding(taskId),
    requestedTarget: primary.requestedTarget,
    target: primary.target,
    transaction: primary.transaction,
    binary,
    repository: {
      gitCommit: head,
      implementationFingerprint: producerIdentity.fingerprint,
      producerSourceFingerprint: producerIdentity.fingerprint,
    },
    workflowTaskProof: {
      taskId,
      proofMode: WORKFLOW_TASK_PROOF_MODE,
      producerOwner: spec.producerOwner,
      sourceOwners,
      stages: options.stages,
      observedSegments: options.segments.map((segment) => ({
        id: segment.id,
        runId: segment.runId,
        target: segment.target,
        transaction: segment.transaction,
        cleanup: segment.cleanup,
      })),
      safety: options.safety,
    },
    sourceFingerprints,
    negativeControls: controls(options.negativeControls),
    cleanup: {
      closed: true,
      ownedPids: [],
      ownedSessions: [],
      ownedBrowserPids: [],
      survivors: [],
      processExited: true,
      streamsDrained: true,
      logWriterClosed: true,
      ownedProcessCount: 0,
      clipboardTouched: options.safety.clipboardTouched === true,
      clipboardRestored: options.safety.clipboardRestored === true,
    },
    missingPrimitives: [],
    errors: [],
    warnings: [],
  });
  if (!prepared.validation.valid || prepared.receipt.disposition !== "EVALUABLE_PASS") {
    throw new Error(`${taskId} observed workflow proof is invalid: ${prepared.validation.errors.join("; ")}`);
  }
  (prepared.receipt.producerValidation as JsonObject).registryFingerprint =
    receiptRegistryIdentity().registryFingerprint;
  return prepared;
}

export function prepareBlockedWorkflowTaskProof(
  taskId: WorkflowTaskProofId,
  reason: string,
) {
  const spec = WORKFLOW_TASK_PROOF_SPECS[taskId];
  if (!spec) throw new Error(`unknown SAFE/WF workflow task: ${taskId}`);
  return prepareValidatedReceipt(WORKFLOW_TASK_PRIMITIVE_ID, {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    tool: "script-kit-devtools.workflow-proof",
    command: "workflow.prove",
    classification: "blocked-by-missing-primitive",
    taskId,
    taskIds: [taskId],
    catalogBinding: catalogBinding(taskId),
    missingPrimitives: ["completedObservedWorkflowJourney"],
    workflowFailure: {
      producerOwner: spec.producerOwner,
      reasonSha256: fingerprint(reason),
    },
  });
}

/** Publish only to the canonical authoritative task directory after real execution. */
export function writeWorkflowTaskProof(
  taskId: WorkflowTaskProofId,
  receipt: JsonObject,
  receiptsRoot = ".artifacts/consistency",
): string {
  if (receipt.taskId !== taskId || receipt.primitiveId !== WORKFLOW_TASK_PRIMITIVE_ID) {
    throw new Error(`${taskId} cannot publish an unrelated or unregistered workflow receipt`);
  }
  const path = resolve(receiptsRoot, taskId, "workflow-proof.json");
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.tmp-${process.pid}`;
  writeFileSync(temporary, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600, flag: "wx" });
  renameSync(temporary, path);
  return path;
}
