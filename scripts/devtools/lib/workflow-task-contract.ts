type JsonObject = Record<string, unknown>;

export interface WorkflowTaskSpec {
  readonly producerOwner: string;
  readonly stageIds: readonly string[];
  readonly negativeControlIds: readonly string[];
  readonly supportingOwners: readonly string[];
}

const producerRoot = "scripts/agentic/cons-flow-ux/";

function task(
  producer: string,
  stageIds: readonly string[],
  negativeControlIds: readonly string[],
  supportingOwners: readonly string[] = [],
): WorkflowTaskSpec {
  return {
    producerOwner: `${producerRoot}${producer}.ts`,
    stageIds,
    negativeControlIds,
    supportingOwners,
  };
}

/** One executable owner and irreducible observed journey for every SAFE/WF task. */
export const WORKFLOW_TASK_PROOF_SPECS = {
  "SAFE-001": task("context-preparation-probe", [
    "accepted-sanitized-context", "failed-primary-blocked", "failed-supplemental-preserved",
  ], ["base64-context-redacted", "failed-primary-cannot-send", "no-raw-private-content"]),
  "SAFE-002": task("dictation-dismiss-targets-probe", [
    "recording-confirm-resume-discard", "processing-hide-reopen",
  ], ["recording-never-discarded-without-confirmation", "processing-never-cancelled-by-escape"]),
  "SAFE-003": task("flow-history-probe", [
    "core-lifecycle-a", "core-lifecycle-b",
  ], ["new-conversation-preserves-history", "delete-requires-explicit-confirmation"]),
  "SAFE-004": task("notes-actions-probe", [
    "shortcut-descriptors-executable", "destructive-confirmations-enforced",
  ], ["unavailable-shortcuts-cannot-activate", "destructive-action-requires-confirmation"]),
  "WF-001": task("context-lifecycle-probe", [
    "pending-context-provenance", "duplicate-context-deduplicated",
  ], ["duplicate-context-never-creates-second-chip", "raw-source-identity-redacted"]),
  "WF-002": task("entry-verbs-probe", [
    "open-draft", "preflight-refusal",
  ], ["failed-preflight-preserves-source", "open-never-submits"]),
  "WF-003": task("context-lifecycle-probe", [
    "partial-accepted-send", "fresh-thread-clears-context", "portal-cancellation-restores-context",
  ], ["immutable-receipt-cannot-be-removed", "fresh-thread-never-inherits-context"]),
  "WF-004": task("semantic-command-probe", [
    "context-role-isolated", "identity-role-isolated", "destination-role-isolated",
  ], ["context-identity-destination-cannot-interchange", "passive-inspection-never-mutates-destination"]),
  "WF-005": task("semantic-command-probe", [
    "conversation-command-descriptors", "unsupported-command-refused",
  ], ["disabled-command-cannot-activate", "destructive-command-requires-confirmation"]),
  "WF-006": task("conversation-hosts-probe", [
    "dismissal-restores-origin", "overlay-unwinds-one-layer",
  ], ["background-preserves-active-work", "close-never-destroys-draft"]),
  "WF-007": task("conversation-hosts-probe", [
    "send-stop-retry-lifecycle", "cancellation-classified-correctly",
  ], ["stop-is-cancellation-not-failure", "unavailable-recovery-cannot-activate"]),
  "WF-008": task("entry-verbs-probe", [
    "open-draft", "quick-question", "ask-cmd-enter",
  ], ["open-never-submits", "quick-question-never-inherits-context", "ask-submits-exactly-once"]),
  "WF-009": task("conversation-hosts-probe", [
    "conversation-copy-selection", "conversation-edit-capabilities",
  ], ["unsupported-copy-never-advertised", "clipboard-private-content-redacted"]),
  "WF-010": task("conversation-hosts-probe", [
    "chat-prompt-supported-callbacks", "chat-prompt-unsupported-refusal",
  ], ["unsupported-stop-never-advertised", "unsupported-retry-never-advertised"]),
  "WF-011": task("flow-history-probe", [
    "core-lifecycle-a", "core-lifecycle-b", "desk-ready",
  ], ["runtime-termination-preserves-history", "missing-engine-reports-actionable-state"]),
  "WF-012": task("notes-search-probe", [
    "shared-notes-search-results", "host-destinations-disclosed",
  ], ["failed-refresh-retains-prior-results", "no-match-never-pretends-corpus-empty"]),
  "WF-013": task("notes-today-probe", [
    "notes-mention-parity", "today-mention-parity",
  ], ["partial-reference-never-survives-deletion", "file-discovery-never-silently-disappears"], [
    "scripts/agentic/notes-spine-host-wiring-probe.ts",
    "scripts/agentic/day-page-context-roundtrip-probe.ts",
  ]),
  "WF-014": task("notes-today-probe", [
    "notes-agent-chat-return", "today-scope-matrix",
  ], ["outside-selected-range-never-staged", "context-handoff-never-auto-submits"], [
    "scripts/agentic/cons-flow-ux/notes-agent-chat-return-probe.ts",
    "scripts/agentic/day-page-agent-chat-handoff-scope-probe.ts",
  ]),
  "WF-015": task("notes-today-probe", [
    "notes-agent-chat-return", "today-agent-chat-return",
  ], ["return-never-targets-a-different-host", "unsaved-editor-state-never-discarded"], [
    "scripts/agentic/cons-flow-ux/notes-agent-chat-return-probe.ts",
    "scripts/agentic/day-agent-chat-return-probe.ts",
  ]),
  "WF-016": task("notes-handoff-probe", [
    "partial-duplicate-reuse", "primary-failure-atomic", "cart-delete-failure",
  ], ["primary-failure-never-consumes-cart", "failed-attachment-remains-retryable"]),
  "WF-017": task("notes-search-probe", [
    "standalone-open", "portal-attach", "portal-cancel-restores-origin",
  ], ["portal-attachment-never-auto-submits", "portal-cancellation-restores-draft"]),
  "WF-018": task("dictation-dismiss-targets-probe", [
    "recording-confirm-resume-discard", "processing-hide-reopen",
  ], ["legacy-destination-never-selectable", "disabled-destination-explains-refusal"]),
  "WF-019": task("dictation-dismiss-targets-probe", [
    "recording-confirm-resume-discard", "processing-hide-reopen",
  ], ["destination-selection-never-delivers", "destination-selection-never-stops-recording"]),
  "WF-020": task("dictation-delivery-probe", [
    "stale-launcher-input-refuses", "stale-prompt-input-refuses", "stale-notes-editor-refuses",
  ], ["stale-destination-never-mutates", "stale-destination-never-falls-back"]),
  "WF-021": task("dictation-delivery-probe", [
    "launcher-filter", "prompt-input", "notes-editor", "captured-day",
    "fresh-agent-chat", "existing-agent-chat", "fresh-quick-ai", "unknown-target-refuses",
  ], ["delivery-occurs-exactly-once", "unknown-destination-never-falls-back"]),
  "WF-022": task("dictation-recovery-focus-probe", [
    "stale-copy-history", "choose-destination-delivers-once",
  ], ["failed-delivery-retains-transcript", "unsupported-recovery-never-advertised"]),
  "WF-023": task("dictation-recovery-focus-probe", [
    "choose-destination-delivers-once", "microphone-picker-restores-overlay",
  ], ["stale-focus-generation-never-restored", "dismissal-never-starts-microphone"]),
  "WF-024": task("dictation-history-probe", [
    "paging-actions-portals-delete", "typed-load-failure-with-prior",
  ], ["delete-requires-explicit-confirmation", "load-failure-retains-prior-history"]),
} as const satisfies Record<string, WorkflowTaskSpec>;

export type WorkflowTaskProofId = keyof typeof WORKFLOW_TASK_PROOF_SPECS;

export const WORKFLOW_TASK_PRIMITIVE_ID = "devtools.consistency.workflow-task-proof";
export const WORKFLOW_TASK_PROOF_MODE = "observed-user-journey";

export const WORKFLOW_STAGE_PRIMITIVES = new Set([
  "devtools.elements.snapshot",
  "devtools.layout.measure",
  "devtools.text.measure",
  "devtools.focus.inspect",
  "devtools.scroll.inspect",
  "devtools.keyboard.inspect",
  "devtools.actions.inspect",
  "devtools.act",
  "devtools.notes.inspect",
  "devtools.dictation.inspect",
  "devtools.dictation.deliverFixture",
  "devtools.inspect.orchestrate",
]);

function object(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : {};
}

function stringObjects(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.map(object) : [];
}

const fingerprint = (value: unknown): boolean =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);

export function workflowTaskProofSourceOwners(taskId: WorkflowTaskProofId): string[] {
  return [
    "scripts/devtools/lib/receipt-schema.ts",
    "scripts/agentic/compiler-input-paths.txt",
    "scripts/devtools/lib/runtime-task-proof.ts",
    "scripts/devtools/lib/workflow-task-contract.ts",
    "scripts/devtools/lib/workflow-task-proof.ts",
    WORKFLOW_TASK_PROOF_SPECS[taskId].producerOwner,
    ...WORKFLOW_TASK_PROOF_SPECS[taskId].supportingOwners,
  ];
}

/** Independently validate observations; a passing flag or prose summary is never proof. */
export function workflowTaskProofErrors(receipt: JsonObject): string[] {
  const taskId = receipt.taskId;
  if (typeof taskId !== "string" || !(taskId in WORKFLOW_TASK_PROOF_SPECS)) {
    return ["workflow task proof requires one canonical SAFE/WF task identity"];
  }
  const id = taskId as WorkflowTaskProofId;
  const spec = WORKFLOW_TASK_PROOF_SPECS[id];
  const proof = object(receipt.workflowTaskProof);
  const errors: string[] = [];
  if (proof.taskId !== id || proof.proofMode !== WORKFLOW_TASK_PROOF_MODE) {
    errors.push("workflow proof does not bind the exact task and observed journey mode");
  }
  if (proof.producerOwner !== spec.producerOwner) {
    errors.push("workflow proof does not bind its exact executable producer owner");
  }

  const expectedOwners = workflowTaskProofSourceOwners(id);
  const sourceOwners = Array.isArray(proof.sourceOwners)
    ? proof.sourceOwners.filter((owner): owner is string => typeof owner === "string")
    : [];
  const sourceFingerprints = object(receipt.sourceFingerprints);
  if (
    sourceOwners.length !== expectedOwners.length ||
    new Set(sourceOwners).size !== expectedOwners.length ||
    expectedOwners.some((owner) => !sourceOwners.includes(owner)) ||
    Object.keys(sourceFingerprints).length !== expectedOwners.length ||
    expectedOwners.some((owner) => !fingerprint(sourceFingerprints[owner]))
  ) {
    errors.push("workflow proof requires exact reviewed schema, contract, adapter, and producer owners");
  }

  const repository = object(receipt.repository);
  const binary = object(receipt.binary);
  const artifactReference = object(binary.artifactReference);
  const rootTransaction = object(receipt.transaction);
  const binaryPath = typeof binary.path === "string" ? binary.path : "";
  const manifestPath = typeof artifactReference.manifestPath === "string" ? artifactReference.manifestPath : "";
  if (
    !fingerprint(binary.sha256) || binary.sha256 !== rootTransaction.binarySha256 ||
    typeof binary.sourceCommit !== "string" || !/^[a-f0-9]{40,64}$/.test(binary.sourceCommit) ||
    typeof repository.gitCommit !== "string" || !/^[a-f0-9]{40,64}$/.test(repository.gitCommit) ||
    typeof binary.sourceDirty !== "boolean" ||
    !Number.isSafeInteger(binary.sizeBytes) || Number(binary.sizeBytes) <= 0 ||
    !/^target-agent\/artifacts\/[A-Za-z0-9][A-Za-z0-9._-]*\/manifest\.json$/.test(manifestPath) ||
    !fingerprint(artifactReference.manifestSha256) ||
    binary.manifestPath !== manifestPath || binary.manifestSha256 !== artifactReference.manifestSha256 ||
    !binaryPath.startsWith(manifestPath.slice(0, -"manifest.json".length)) ||
    binaryPath.includes("\\") || binaryPath.split("/").some((part) => !part || part === "." || part === "..") ||
    binary.provenance !== undefined
  ) {
    errors.push("workflow proof requires one exact source-bound binary, explicit immutable artifact reference, and root transaction");
  }
  // Build source and observation HEAD remain distinct; the producer and auditor independently
  // verify current-content compatibility through the explicit immutable artifact reference.

  const safety = object(proof.safety);
  if (
    safety.microphoneCaptureStarted !== false ||
    safety.nativeInputInjected !== false ||
    safety.liveAiStarted !== false ||
    safety.screenTakeoverStarted !== false ||
    !(safety.clipboardTouched === false || safety.clipboardRestored === true)
  ) {
    errors.push("workflow proof lacks observed microphone, input, AI, desktop, and clipboard safety");
  }

  const segments = stringObjects(proof.observedSegments);
  const segmentIds = segments.map((segment) => String(segment.id ?? ""));
  if (
    segments.length === 0 || segmentIds.some((value) => value.length === 0) ||
    new Set(segmentIds).size !== segmentIds.length
  ) {
    errors.push("workflow proof requires uniquely identified actual observed process segments");
  }
  const segmentsById = new Map(segments.map((segment) => [String(segment.id ?? ""), segment]));
  for (const segment of segments) {
    const target = object(segment.target);
    const transaction = object(segment.transaction);
    const cleanup = object(segment.cleanup);
    if (
      typeof segment.runId !== "string" || segment.runId.length === 0 ||
      segment.runId !== transaction.runId ||
      !Number.isSafeInteger(transaction.pid) || Number(transaction.pid) <= 0 ||
      transaction.binarySha256 !== binary.sha256 ||
      typeof target.visible !== "boolean" ||
      target.pid !== transaction.pid ||
      target.automationId !== transaction.automationId ||
      target.windowInstanceId !== transaction.windowInstanceId ||
      target.windowGeneration !== undefined && target.windowGeneration !== transaction.windowGeneration ||
      target.targetGeneration !== transaction.targetGeneration ||
      target.surfaceGeneration !== transaction.surfaceGeneration ||
      target.dataGeneration !== transaction.dataGeneration
    ) {
      errors.push(`workflow segment ${String(segment.id ?? "missing")} lacks actual matching target/process identity`);
    }
    if (
      cleanup.processExited !== true || cleanup.streamsDrained !== true ||
      cleanup.logWriterClosed !== true || cleanup.ownedProcessCount !== 0 ||
      cleanup.closeError != null ||
      (cleanup.clipboardTouched === true && cleanup.clipboardRestored !== true)
    ) {
      errors.push(`workflow segment ${String(segment.id ?? "missing")} did not complete safe owned-process cleanup`);
    }
  }

  const stages = stringObjects(proof.stages);
  const stageIds = stages.map((stage) => String(stage.id ?? ""));
  if (new Set(stageIds).size !== stageIds.length) {
    errors.push("workflow journey stages must have unique stable identities");
  }
  for (const requiredStage of spec.stageIds) {
    if (!stageIds.includes(requiredStage)) {
      errors.push(`workflow journey did not execute required stage: ${requiredStage}`);
    }
  }
  const requestIds = new Set<string>();
  for (const stage of stages) {
    const segment = segmentsById.get(String(stage.segmentId ?? ""));
    const observation = object(stage.observation);
    const transaction = object(stage.transaction);
    const segmentTransaction = object(segment?.transaction);
    if (
      stage.pass !== true || !WORKFLOW_STAGE_PRIMITIVES.has(String(stage.primitiveId ?? "")) ||
      !segment || stage.runId !== segment.runId ||
      transaction.runId !== segmentTransaction.runId ||
      transaction.transactionId !== segmentTransaction.transactionId ||
      transaction.pid !== segmentTransaction.pid ||
      transaction.processStartTime !== segmentTransaction.processStartTime ||
      transaction.automationId !== segmentTransaction.automationId ||
      transaction.windowInstanceId !== segmentTransaction.windowInstanceId ||
      transaction.windowGeneration !== segmentTransaction.windowGeneration ||
      transaction.targetGeneration !== segmentTransaction.targetGeneration ||
      transaction.surfaceGeneration !== segmentTransaction.surfaceGeneration ||
      transaction.dataGeneration !== segmentTransaction.dataGeneration ||
      transaction.binarySha256 !== segmentTransaction.binarySha256
    ) {
      errors.push(`workflow stage ${String(stage.id ?? "missing")} is not bound to an observed registered target transaction`);
    }
    if (
      typeof observation.command !== "string" || observation.command.length === 0 ||
      typeof observation.requestId !== "string" || observation.requestId.length === 0 ||
      requestIds.has(String(observation.requestId ?? "")) ||
      !fingerprint(observation.resultSha256)
    ) {
      errors.push(`workflow stage ${String(stage.id ?? "missing")} lacks a unique actual command/result observation`);
    }
    requestIds.add(String(observation.requestId ?? ""));
  }

  const controls = stringObjects(receipt.negativeControls);
  const controlIds = controls.map((control) => String(control.id ?? ""));
  if (new Set(controlIds).size !== controlIds.length) {
    errors.push("workflow negative controls must have unique stable identities");
  }
  for (const requiredControl of spec.negativeControlIds) {
    const observed = controls.find((control) => control.id === requiredControl);
    if (!observed || observed.pass !== true || observed.executed !== true) {
      errors.push(`workflow journey did not execute required adversarial control: ${requiredControl}`);
    }
  }
  return errors;
}
