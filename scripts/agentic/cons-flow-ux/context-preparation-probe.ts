#!/usr/bin/env bun

import { mkdir, unlink } from "node:fs/promises";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observedWorkflowStage,
  observeWorkflowTaskTarget,
  prepareBlockedWorkflowTaskProof,
  prepareWorkflowTaskProof,
  writeWorkflowTaskProof,
} from "../../devtools/lib/workflow-task-proof.ts";
import type { RuntimeTargetObservation } from "../../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.context-preparation");

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY ??
    "target-agent/artifacts/cons-flow-safe001/script-kit-gpui",
);
const artifactDir = resolve(
  ".artifacts/consistency/cons-flow-ux/safe001-canonical-v2/SAFE-001",
);
const receiptPath = join(artifactDir, "receipt.json");
await mkdir(artifactDir, { recursive: true });
try {
  await unlink(receiptPath);
} catch {}

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(
      `${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`,
    );
  }
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const output = new TextDecoder().decode(result.stdout);
  const normalized = resolve(executable);
  return output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const command = match[2].trim().split(/\s+/, 1)[0];
      return resolve(command) === normalized ? [Number(match[1])] : [];
    });
}

function sha256(path: string): string {
  const result = Bun.spawnSync(["shasum", "-a", "256", path], {
    stdout: "pipe",
    stderr: "pipe",
  });
  assert(result.exitCode === 0, "failed to hash runtime binary");
  return new TextDecoder().decode(result.stdout).trim().split(/\s+/, 1)[0];
}

function asJson(value: unknown): Json {
  return value && typeof value === "object" ? (value as Json) : {};
}

function assertReceiptIsPrivate(value: unknown): void {
  const serialized = JSON.stringify(value);
  for (const forbidden of [
    "SAFE001_BASE64_CANARY",
    "SAFE001_NONBINARY_SURVIVES",
    "SAFE001_RAW_PATH_CANARY",
    "rawContent",
    "finalUserContent",
    "promptPrefix",
  ]) {
    assert(!serialized.includes(forbidden), `runtime receipt leaked ${forbidden}`);
  }
}

const cleanups: Json[] = [];
let targetObservation: RuntimeTargetObservation | null = null;
let receipt: Json = {
  schemaVersion: 1,
  taskId: "SAFE-001",
  classification: "RUNTIME-FAILED",
  binaryArtifact: "cons-flow-safe001",
  binarySha256: sha256(binary),
};

const driver = await Driver.launch({
  binary,
  sessionName: "safe001-context-preparation",
  sandboxHome: true,
  env: {
    SCRIPT_KIT_TEST_STATUS: "1",
    SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
  },
  readyTimeoutMs: 30_000,
  defaultTimeoutMs: 15_000,
});

try {
  driver.send({
    type: "openAgentChatDetachedFixture",
    requestId: "safe001-agent-chat-fixture",
  });
  await Bun.sleep(300);

  const acceptedResponse = await driver.request(
    { type: "inspectContextPreparation", fixtureId: "acceptedOversizedJson" },
    { expect: "contextPreparationProbeResult", timeoutMs: 15_000 },
  );
  const accepted = asJson(acceptedResponse.receipt);
  const acceptedPreparation = asJson(accepted.preparation);
  const acceptedContext = asJson(acceptedPreparation.context);
  const acceptedPrivate = asJson(accepted.privateChecks);
  assert(accepted.classification === "runtimeConfirmed", "accepted fixture was not confirmed", accepted);
  assert(acceptedPreparation.decision === "ready", "accepted fixture was not ready", accepted);
  assert(acceptedContext.attempted === 1 && acceptedContext.resolved === 1);
  assert(acceptedContext.failed === 0);
  assert(acceptedPrivate.nonbinaryFieldPreserved === true);
  assert(acceptedPrivate.base64CanaryAbsent === true);
  assert(acceptedPrivate.binaryOmissionMarkerPresent === true);
  assert(acceptedPrivate.receiptRawCanariesAbsent === true);
  assert(Number(acceptedPrivate.payloadChars) < 101_000, "prepared payload was not bounded", acceptedPrivate);
  assertReceiptIsPrivate(accepted);

  const primaryResponse = await driver.request(
    { type: "inspectContextPreparation", fixtureId: "missingPrimary" },
    { expect: "contextPreparationProbeResult", timeoutMs: 15_000 },
  );
  const primary = asJson(primaryResponse.receipt);
  const primaryPreparation = asJson(primary.preparation);
  const primaryContext = asJson(primaryPreparation.context);
  const primaryPrivate = asJson(primary.privateChecks);
  assert(primaryPreparation.decision === "blocked", "failed primary did not block", primary);
  assert(primaryContext.primaryFailed === 1);
  assert(primaryPrivate.canSendMessage === false);
  assert(
    primaryPreparation.userError ===
      "This context could not be prepared. Retry or remove it before sending.",
    "failed primary exposed unexpected copy",
    primaryPreparation,
  );
  assertReceiptIsPrivate(primary);

  const supplementalResponse = await driver.request(
    { type: "inspectContextPreparation", fixtureId: "missingSupplemental" },
    { expect: "contextPreparationProbeResult", timeoutMs: 15_000 },
  );
  const supplemental = asJson(supplementalResponse.receipt);
  const supplementalPreparation = asJson(supplemental.preparation);
  const supplementalContext = asJson(supplementalPreparation.context);
  const supplementalPrivate = asJson(supplemental.privateChecks);
  assert(
    supplementalPreparation.decision === "partial",
    "failed supplemental did not preserve authored send",
    supplemental,
  );
  assert(supplementalContext.supplementalFailed === 1);
  assert(supplementalPrivate.canSendMessage === true);
  assert(
    supplementalPreparation.userError ===
      "One attachment could not be added. The remaining message is ready.",
    "failed supplemental exposed unexpected copy",
    supplementalPreparation,
  );
  assertReceiptIsPrivate(supplemental);

  const elements = await driver.getElements(
    { target: { type: "kind", kind: "agentChatDetached" }, limit: 300 },
    { timeoutMs: 15_000 },
  );
  assertReceiptIsPrivate(elements);
  targetObservation = await observeWorkflowTaskTarget(driver, binary, {
    type: "kind",
    kind: "agentChatDetached",
    index: 0,
  });

  receipt = {
    ...receipt,
    classification: "RUNTIME-CONFIRMED",
    agentChatFixtureOpened: true,
    accepted: {
      decision: acceptedPreparation.decision,
      attempted: acceptedContext.attempted,
      resolved: acceptedContext.resolved,
      failed: acceptedContext.failed,
      payloadChars: acceptedPrivate.payloadChars,
      binaryFieldsStripped: acceptedPrivate.base64CanaryAbsent,
      nonbinaryFieldPreserved: acceptedPrivate.nonbinaryFieldPreserved,
      bounded: true,
    },
    primaryFailure: {
      decision: primaryPreparation.decision,
      primaryFailed: primaryContext.primaryFailed,
      acceptedSend: primaryPrivate.canSendMessage,
      safeUserError: true,
    },
    supplementalFailure: {
      decision: supplementalPreparation.decision,
      supplementalFailed: supplementalContext.supplementalFailed,
      acceptedSend: supplementalPrivate.canSendMessage,
      safeUserError: true,
    },
    negativeControls: {
      base64CanaryAbsentAtModelBoundary: true,
      zeroResolvedPrimaryCannotSend: true,
      serializedReceiptsContainNoRawContentOrSourceIdentity: true,
      visibleAgentChatSemanticsContainNoPreparationCanary: true,
    },
  };
} catch (error) {
  console.error("SAFE-001 private probe diagnostic:", error);
  receipt.error = {
    name: error instanceof Error ? error.name : "UnknownError",
    safeMessage: "SAFE-001 runtime assertion failed; inspect the private Driver session log.",
  };
} finally {
  await driver.close();
  const ownedPids = exactExecutablePids(binary);
  const cleanup: Json = {
    sessionName: "safe001-context-preparation",
    processExited: driver.finalization.processExited,
    streamsDrained: driver.finalization.streamsDrained,
    logWriterClosed: driver.finalization.logWriterClosed,
    ownedProcessCount: ownedPids.length,
    ownedPids,
    clipboardTouched: false,
    closeError: null,
  };
  cleanups.push(cleanup);
  receipt.cleanup = cleanups;
  receipt.finalOwnedProcessCount = ownedPids.length;

  let serialized = JSON.stringify(receipt, null, 2);
  const forbiddenCanaries = [
    "SAFE001_BASE64_CANARY",
    "SAFE001_NONBINARY_SURVIVES",
    "SAFE001_RAW_PATH_CANARY",
    "rawContent",
    "finalUserContent",
    "promptPrefix",
  ];
  const leaked = forbiddenCanaries.filter((canary) => serialized.includes(canary));
  if (leaked.length > 0) {
    receipt.classification = "RUNTIME-FAILED";
    receipt.privacyViolationCount = leaked.length;
    receipt.error = {
      name: "ReceiptPrivacyViolation",
      safeMessage: "The generated receipt contained forbidden private data and was suppressed.",
    };
    serialized = JSON.stringify(receipt, null, 2);
  }
  try {
    if (receipt.classification !== "RUNTIME-CONFIRMED" || targetObservation === null) {
      throw new Error("the actual sanitized-context journey did not complete");
    }
    const segment = observedWorkflowSegment(
      "safe001-context-preparation",
      targetObservation,
      cleanup,
    );
    const stages = [
      {
        id: "accepted-sanitized-context",
        requestId: "acceptedOversizedJson",
        result: receipt.accepted,
      },
      {
        id: "failed-primary-blocked",
        requestId: "missingPrimary",
        result: receipt.primaryFailure,
      },
      {
        id: "failed-supplemental-preserved",
        requestId: "missingSupplemental",
        result: receipt.supplementalFailure,
      },
    ].map((stage) => observedWorkflowStage({
      ...stage,
      primitiveId: "devtools.inspect.orchestrate",
      segment,
      command: "inspectContextPreparation",
      pass: true,
    }));
    const observedControls = asJson(receipt.negativeControls);
    receipt = prepareWorkflowTaskProof("SAFE-001", {
      producerOwner: "scripts/agentic/cons-flow-ux/context-preparation-probe.ts",
      segments: [segment],
      stages,
      negativeControls: {
        "base64-context-redacted": observedControls.base64CanaryAbsentAtModelBoundary === true,
        "failed-primary-cannot-send": observedControls.zeroResolvedPrimaryCannotSend === true,
        "no-raw-private-content":
          observedControls.serializedReceiptsContainNoRawContentOrSourceIdentity === true &&
          observedControls.visibleAgentChatSemanticsContainNoPreparationCanary === true,
      },
      safety: {
        microphoneCaptureStarted: false,
        nativeInputInjected: false,
        liveAiStarted: false,
        screenTakeoverStarted: false,
        clipboardTouched: false,
      },
    }).receipt as Json;
  } catch (error) {
    receipt = prepareBlockedWorkflowTaskProof(
      "SAFE-001",
      error instanceof Error ? error.message : String(error),
    ).receipt as Json;
  }
  assertReceiptIsPrivate(receipt);
  serialized = JSON.stringify(receipt, null, 2);
  await Bun.write(receiptPath, `${serialized}\n`);
  writeWorkflowTaskProof("SAFE-001", receipt);
}

console.log(JSON.stringify(receipt, null, 2));
assert(receipt.disposition === "EVALUABLE_PASS", "SAFE-001 runtime proof failed", receipt);
assert(cleanups.length === 1);
assert(cleanups.every((cleanup) => cleanup.processExited === true));
assert(cleanups.every((cleanup) => cleanup.streamsDrained === true));
assert(cleanups.every((cleanup) => cleanup.logWriterClosed === true));
assert(cleanups.every((cleanup) => cleanup.ownedProcessCount === 0));
assert(exactExecutablePids(binary).length === 0, "SAFE-001 left an app instance running");
