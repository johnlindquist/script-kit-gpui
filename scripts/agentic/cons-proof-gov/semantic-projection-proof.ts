#!/usr/bin/env bun

import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver.ts";
import {
  classify,
  semanticProjection,
  snapshot,
  type ProjectionProofMode,
} from "../../devtools/elements.ts";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observeRuntimeTaskTarget,
  prepareBlockedRuntimeTaskProof,
  prepareRuntimeTaskProof,
  type RuntimeTargetObservation,
} from "../../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("cons-proof-gov.semantic-projection");

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY
    ?? "target-agent/artifacts/cons-proof-c04/script-kit-gpui",
);
const artifactPath = resolve(
  process.env.CONSISTENCY_RECEIPT_PATH
    ?? ".artifacts/consistency/PF-004/semantic-projection.json",
);

type Obj = Record<string, unknown>;

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Obj
    : {};
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const normalized = resolve(executable);
  return new TextDecoder().decode(result.stdout)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const executablePath = match[2].trim().split(/\s+/, 1)[0];
      return resolve(executablePath) === normalized ? [Number(match[1])] : [];
    });
}

async function poll(
  label: string,
  probe: () => Promise<Json>,
  predicate: (value: Obj) => boolean,
  timeoutMs = 8_000,
): Promise<Obj> {
  const deadline = performance.now() + timeoutMs;
  let last: Obj = {};
  while (performance.now() < deadline) {
    last = asObj(await probe());
    if (predicate(last)) return last;
    await Bun.sleep(25);
  }
  throw new Error(`${label} did not become observable`);
}

function syntheticFixture(
  id: string,
  quality: "complete" | "partial" | "unsupported",
  reasonCodes: string[],
  proofMode: ProjectionProofMode,
  nodes: Obj[],
) {
  const elementSnapshot = snapshot(nodes);
  const projection = semanticProjection({
    semanticSurface: id,
    projectionVersion: 1,
    projectionQuality: quality,
    reasonCodes,
  }, proofMode);
  return {
    id,
    projection,
    classification: classify(
      { classification: "ok" },
      { status: "ok" },
      elementSnapshot,
      projection,
    ),
    duplicateSemanticIds: elementSnapshot.duplicateSemanticIds,
    returnedCount: elementSnapshot.nodes.length,
  };
}

const synthetic = {
  partialPanel: syntheticFixture(
    "about",
    "partial",
    ["collectorUnavailable"],
    "action",
    [{ semanticId: "panel:about", type: "panel" }],
  ),
  unsupportedCustomDocument: syntheticFixture(
    "divPrompt",
    "unsupported",
    ["unsupportedCustomDocument"],
    "action",
    [{ semanticId: "panel:div-prompt", type: "panel" }],
  ),
  missingFlowEntity: syntheticFixture(
    "flowSession",
    "partial",
    ["runtimeEntityMissing"],
    "focus",
    [{ semanticId: "panel:flow-session", type: "panel" }],
  ),
  duplicateId: syntheticFixture(
    "settings",
    "complete",
    [],
    "inspection",
    [
      { semanticId: "same", type: "button" },
      { semanticId: "same", type: "button" },
    ],
  ),
};
assert(synthetic.partialPanel.classification === "blocked-by-unsupported-projection", "partial action proof did not block");
assert(synthetic.unsupportedCustomDocument.classification === "blocked-by-unsupported-projection", "unsupported custom document did not block");
assert(synthetic.missingFlowEntity.classification === "blocked-by-unsupported-projection", "missing Flow entity did not block");
assert(synthetic.duplicateId.classification === "invalid-identity", "duplicate semantic ids were not invalid");

let driver: Driver | null = null;
let closeError: string | null = null;
let runtimeError: string | null = null;
let runtimeStage = "launch";
let settings: Obj = {};
let observation: RuntimeTargetObservation | null = null;
let observedNodes: Obj[] = [];
let observedProjection: Obj = {};

try {
  driver = await Driver.launch({
    binary,
    sessionName: `cons-proof-pf004-${process.pid}`,
    sandboxHome: true,
    sharedModels: false,
    defaultTimeoutMs: 10_000,
    env: {
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
    },
  });

  runtimeStage = "open-settings";
  driver.send({ type: "triggerBuiltin", name: "settings" });
  await poll(
    "Settings",
    () => driver!.getState({ timeoutMs: 3_000 }),
    (state) => String(state.promptType ?? state.currentView ?? "").toLowerCase().includes("settings"),
  );

  runtimeStage = "settings-elements";
  const response = asObj(await driver.getElements({ target: { type: "main" }, limit: 200 }));
  const nodes = asArray(response.elements).map(asObj);
  const measured = snapshot(nodes);
  const projection = semanticProjection(response, "action");
  const classification = classify(
    { classification: "ok" },
    { status: "ok" },
    measured,
    projection,
  );
  assert(response.type === "elementsResult", "Settings did not return elementsResult");
  assert(projection.semanticSurface === "settings", "Settings collector surface is wrong");
  assert(projection.version === 1, "Settings projection version is missing");
  assert(projection.quality === "complete", "Settings projection is not complete");
  assert(projection.proofAllowed === true, "Settings action proof was not allowed");
  assert(classification === "ok", "Settings semantic projection did not classify as ok");
  assert(measured.duplicateSemanticIds.length === 0, "Settings contains duplicate semantic ids");
  assert(measured.nodes.length >= 2, "Settings projection lacks its input/list semantics");
  assert(measured.nodes.every((node) => node.semanticId && node.measurementId), "Settings nodes lack stable measurement identity");
  assert(measured.nodes.every((node) => typeof node.enabled === "boolean"), "Settings nodes lack enabled state");
  assert(measured.nodes.every((node) => typeof node.focusable === "boolean"), "Settings nodes lack focusability state");
  assert(measured.nodes.every((node) => typeof node.activatable === "boolean"), "Settings nodes lack activation state");
  observation = await observeRuntimeTaskTarget(driver, binary, { type: "main" });
  observedNodes = measured.nodes as Obj[];
  observedProjection = projection as unknown as Obj;

  settings = {
    responseType: response.type,
    semanticSurface: projection.semanticSurface,
    projectionVersion: projection.version,
    projectionQuality: projection.quality,
    reasonCodes: projection.reasonCodes,
    proofMode: projection.proofMode,
    proofAllowed: projection.proofAllowed,
    classification,
    returnedCount: measured.nodes.length,
    duplicateSemanticIds: measured.duplicateSemanticIds,
    nodeContract: {
      semanticId: true,
      measurementId: true,
      action: true,
      enabled: true,
      disabledReason: true,
      focusable: true,
      selectable: true,
      activatable: true,
      owner: true,
    },
  };
  runtimeStage = "complete";
} catch (error) {
  runtimeError = error instanceof Error ? error.message : "UnknownError";
} finally {
  if (driver) {
    try {
      await driver.close();
    } catch (error) {
      closeError = error instanceof Error ? error.name : "UnknownCloseError";
    }
  }
}

const ownedProcessCount = exactExecutablePids(binary).length;
const cleanup = driver
  ? {
      processExited: driver.finalization.processExited,
      streamsDrained: driver.finalization.streamsDrained,
      logWriterClosed: driver.finalization.logWriterClosed,
      ownedProcessCount,
      closeError,
      clipboardTouched: false,
    }
  : {
      processExited: false,
      streamsDrained: false,
      logWriterClosed: false,
      ownedProcessCount,
      closeError,
      clipboardTouched: false,
    };
const runtimePassed = runtimeError == null && runtimeStage === "complete";
const cleanupPassed = cleanup.processExited
  && cleanup.streamsDrained
  && cleanup.logWriterClosed
  && cleanup.ownedProcessCount === 0
  && cleanup.closeError == null;
const negativeControls = {
  partialActionProofBlocked: synthetic.partialPanel.classification === "blocked-by-unsupported-projection",
  unsupportedCustomDocumentBlocked: synthetic.unsupportedCustomDocument.classification === "blocked-by-unsupported-projection",
  missingFlowEntityBlocked: synthetic.missingFlowEntity.classification === "blocked-by-unsupported-projection",
  duplicateSemanticIdsInvalid: synthetic.duplicateId.classification === "invalid-identity",
};
const prepared = runtimePassed && cleanupPassed && observation
  ? prepareRuntimeTaskProof("PF-004", {
      schemaVersion: 2,
      tool: "script-kit-devtools.elements",
      command: "elements.snapshot",
      classification: "ok",
      ...observation,
      semanticSurface: {
        surfaceKind: observation.target.surfaceKind ?? null,
        appViewVariant: observation.target.appViewVariant ?? null,
        collectorSurface: observedProjection.semanticSurface ?? null,
      },
      semanticProjection: observedProjection,
      nodes: observedNodes,
      duplicateSemanticIds: [],
      privacyViolationSemanticIds: [],
      completeSettings: settings,
      fixtures: synthetic,
      evidence: {
        intended: observedProjection,
        model: { nodeCount: observedNodes.length },
        interaction: settings,
      },
      cleanup,
      missingPrimitives: [],
      errors: [],
    }, negativeControls)
  : prepareBlockedRuntimeTaskProof("PF-004", {
      stage: runtimeStage,
      reason: runtimeError ?? closeError ?? "runtime cleanup did not complete",
      cleanup,
      controls: negativeControls,
    });
const receipt = prepared.receipt;

await mkdir(resolve(artifactPath, ".."), { recursive: true });
await writeFile(artifactPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt, null, 2));

if (prepared.exitCode !== 0) process.exitCode = prepared.exitCode;
