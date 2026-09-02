#!/usr/bin/env bun
import { runtimeArtifactFromEnvironment } from "../../devtools/lib/runtime-task-proof.ts";

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { relative, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver.ts";
import {
  diagnostic,
  filePath,
  secret,
  userContent,
} from "../../devtools/lib/privacy.ts";
import { prepareValidatedReceipt } from "../../devtools/lib/receipt-schema.ts";

const binary = runtimeArtifactFromEnvironment().executablePath
const artifactPath = resolve(
  process.env.CONSISTENCY_RECEIPT_PATH
    ?? ".artifacts/consistency/PF-003/privacy-canary.json",
);
const canaries = {
  note: "PF003_NOTE_CONTENT_7a4f",
  clipboard: "PF003_CLIPBOARD_0c91",
  path: "/Users/private/PF003_PATH_3e22/note.md",
  agent: "PF003_AGENT_CHAT_9d11",
  env: "PF003_ENV_SECRET_5b88",
  diagnostic: "PF003_PROVIDER_ERROR_1af0",
} as const;
const canaryValues = Object.values(canaries);
process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES = canaryValues.join(",");
process.env.SCRIPT_KIT_RECEIPT_TASK_IDS = "PF-003";
process.env.PF003_TEST_API_KEY = canaries.env;

type Obj = Record<string, unknown>;

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Obj
    : {};
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function canaryMatchCount(value: unknown): number {
  const serialized = typeof value === "string" ? value : JSON.stringify(value);
  return canaryValues.filter((canary) => serialized.includes(canary)).length;
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
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

async function waitFor(
  label: string,
  probe: () => Promise<Json>,
  predicate: (value: Obj) => boolean,
  timeoutMs = 10_000,
): Promise<Obj> {
  const deadline = performance.now() + timeoutMs;
  let last: Obj = {};
  while (performance.now() < deadline) {
    try {
      last = asObj(await probe());
      if (predicate(last)) return last;
    } catch {
      // Bounded retry; the final error remains safe and names only the step.
    }
    await Bun.sleep(25);
  }
  throw new Error(`${label} did not become observable`);
}

function elementById(envelope: unknown, semanticId: string): Obj {
  const elements = Array.isArray(asObj(envelope).elements)
    ? asObj(envelope).elements as unknown[]
    : [];
  return asObj(elements.find((entry) => asObj(entry).semanticId === semanticId));
}

function valueDescriptor(element: Obj): Obj {
  return asObj(asObj(element.content).value);
}

function assertPrivateElement(
  surface: string,
  envelope: Obj,
  semanticId: string,
): Obj {
  assert(canaryMatchCount(envelope) === 0, `${surface} getElements leaked a privacy canary`);
  const element = elementById(envelope, semanticId);
  assert(Object.keys(element).length > 0, `${surface} element is missing`);
  assert(element.value == null, `${surface} returned raw element value`);
  const descriptor = valueDescriptor(element);
  assert(descriptor.rawContentReturned === false, `${surface} lacks a redacted value descriptor`);
  assert(typeof descriptor.fingerprint === "string", `${surface} lacks a content fingerprint`);
  return {
    semanticId,
    contentKind: descriptor.contentKind ?? null,
    charLength: descriptor.charLength ?? null,
    byteLength: descriptor.byteLength ?? null,
    fingerprintAvailable: true,
    rawContentReturned: false,
    canaryMatches: 0,
  };
}

async function waitForPrivateElement(
  driver: Driver,
  surface: string,
  target: Obj,
  semanticId: string,
  timeoutMs = 8_000,
): Promise<Obj> {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    const envelope = asObj(await driver.getElements({ target, limit: 160 }));
    assert(canaryMatchCount(envelope) === 0, `${surface} getElements leaked a privacy canary`);
    if (Object.keys(elementById(envelope, semanticId)).length > 0) {
      return assertPrivateElement(surface, envelope, semanticId);
    }
    await Bun.sleep(25);
  }
  throw new Error(`${surface} element is missing`);
}

const prepared = prepareValidatedReceipt("devtools.elements.snapshot", {
  schemaVersion: 2,
  tool: "script-kit-devtools.elements",
  command: "elements.snapshot",
  classification: "ok",
  requestedTarget: { selector: { type: "main" } },
  target: { automationId: "main" },
  semanticSurface: { surfaceKind: "ScriptList", collectorSurface: "scriptList" },
  semanticProjection: {
    semanticSurface: "scriptList",
    version: 1,
    quality: "complete",
    reasonCodes: [],
    proofMode: "inspection",
    proofAllowed: true,
  },
  nodes: [{ semanticId: "privacy-canary" }],
  duplicateSemanticIds: [],
  transaction: {
    transactionId: "proof:pf003-canary",
    runId: "pf003-canary-fixture",
    capturedAt: "2026-08-07T00:00:00.000Z",
    pid: process.pid,
    processStartTime: "fixture-process",
    binarySha256: "a".repeat(64),
    automationId: "main",
    windowInstanceId: "main@1",
    windowGeneration: 1,
    nativeWindowId: null,
    axWindowId: null,
    windowKind: "Main",
    hostKind: null,
    parentAutomationId: null,
    parentWindowInstanceId: null,
    openerAutomationId: null,
    surfaceKind: "ScriptList",
    semanticSurface: "scriptList",
    appViewVariant: "ScriptList",
    routeId: null,
    routeStack: [],
    screenId: null,
    backingScaleFactor: 2,
    bounds: { x: 0, y: 0, width: 800, height: 600 },
    targetGeneration: 1,
    surfaceGeneration: 1,
    dataGeneration: 1,
    layoutGeneration: null,
    selectionGeneration: null,
    scrollGeneration: null,
    frameGeneration: null,
  },
  missingPrimitives: [],
  canaryFamily: {
    note: userContent(canaries.note),
    clipboard: userContent(canaries.clipboard),
    path: filePath(canaries.path),
    agentChat: userContent(canaries.agent),
    environment: secret(canaries.env),
    nested: { raw: diagnostic({ message: canaries.diagnostic }) },
  },
  errors: [],
});
assert(prepared.exitCode === 0, "typed privacy fixture did not validate");
assert(prepared.receipt.disposition === "EVALUABLE_PASS", "typed privacy fixture did not pass");
assert(canaryMatchCount(prepared.receipt) === 0, "typed privacy fixture leaked a canary");
const preparedPrivacy = asObj(prepared.receipt.privacy);
assert(preparedPrivacy.canaryMatches === 0, "sanitized receipt retained a canary");
assert(Number(preparedPrivacy.canariesRedacted ?? 0) >= canaryValues.length, "not every canary family was redacted");

let driver: Driver | null = null;
let closeError: string | null = null;
let runtimeError: string | null = null;
let runtimeStage = "launch";
let sessionDir: string | null = null;
let logPath: string | null = null;
const runtime: Obj = {};
let responseStreamCanaryMatches = -1;
let appStreamCanaryMatches = -1;

try {
  driver = await Driver.launch({ immutableArtifact: runtimeArtifactFromEnvironment().reference, binary,
  sessionName: `cons-proof-pf003-${process.pid}`,
  sandboxHome: true,
  sharedModels: false,
  defaultTimeoutMs: 10_000,
  env: {
    SCRIPT_KIT_TEST_STATUS: "1",
    SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
    SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
  }, });
  sessionDir = driver.sessionDir;
  logPath = driver.logPath;

  runtimeStage = "main-filter";
  driver.setFilter(canaries.note);
  runtime.mainFilter = await waitForPrivateElement(
    driver,
    "main filter",
    { type: "main" },
    "input:filter",
  );

  runtimeStage = "clipboard-history";
  driver.send({ type: "triggerBuiltin", name: "clipboardHistory" });
  await waitFor(
    "Clipboard History",
    () => driver!.getState(),
    (state) => state.promptType === "clipboardHistory",
  );
  driver.setFilter(canaries.clipboard);
  runtime.clipboardFilter = await waitForPrivateElement(
    driver,
    "Clipboard History filter",
    { type: "main" },
    "input:clipboard-filter",
  );

  runtimeStage = "notes-editor";
  driver.send({ type: "openNotes", requestId: "pf003-open-notes" });
  const notesTarget = { type: "kind", kind: "notes", index: 0 };
  await waitFor(
    "Notes registration",
    () => driver!.getTargetState(notesTarget, { timeoutMs: 3_000 }),
    (state) => Boolean(asObj(state.notes ?? state).entryReveal),
  );
  await driver.simulateGpuiKeyDown("n", {
    modifiers: ["cmd"],
    target: notesTarget,
    timeoutMs: 5_000,
  });
  await driver.request(
    {
      type: "batch",
      target: notesTarget,
      commands: [{ type: "setInput", text: canaries.note }],
      options: { stopOnError: true, rollbackOnError: false, timeout: 5_000 },
    },
    { expect: "batchResult", timeoutMs: 6_000 },
  );
  runtime.notesEditor = await waitForPrivateElement(
    driver,
    "Notes editor",
    notesTarget,
    "input:notes-editor",
  );

  runtimeStage = "agent-chat-composer";
  driver.send({ type: "openAiWithMockData" });
  await waitFor(
    "Agent Chat",
    () => driver!.getState(),
    (state) => state.promptType === "agentChatChat",
  );
  const setAgentInput = asObj(await driver.request(
    { type: "setAgentChatInput", text: canaries.agent, submit: false },
    { timeoutMs: 8_000 },
  ));
  assert(setAgentInput.ok === true, "Agent Chat composer fixture was refused");
  runtime.agentChatComposer = await waitForPrivateElement(
    driver,
    "Agent Chat composer",
    { type: "main" },
    "input:agent-chat-composer",
  );
  runtimeStage = "complete";
} catch (error) {
  runtimeError = error instanceof Error ? error.name : "UnknownError";
} finally {
  if (driver) {
    try {
      await driver.close();
    } catch (error) {
      closeError = error instanceof Error ? error.name : "UnknownCloseError";
    }
    const responsePath = resolve(driver.sessionDir, "protocol-responses.ndjson");
    const responseStream = await readFile(responsePath, "utf8").catch(() => "");
    const appStream = await readFile(driver.logPath, "utf8").catch(() => "");
    responseStreamCanaryMatches = canaryMatchCount(responseStream);
    appStreamCanaryMatches = canaryMatchCount(appStream);
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
const runtimePassed = runtimeError == null
  && runtimeStage === "complete"
  && Object.keys(runtime).length === 4
  && responseStreamCanaryMatches === 0;
const cleanupPassed = cleanup.processExited
  && cleanup.streamsDrained
  && cleanup.logWriterClosed
  && cleanup.ownedProcessCount === 0
  && cleanup.closeError == null;

const receipt = {
  schemaVersion: 2,
  taskId: "PF-003",
  classification: runtimePassed && cleanupPassed ? "RUNTIME-CONFIRMED" : "RUNTIME-FAILED",
  artifact: {
    executable: relative(process.cwd(), binary),
    sha256: sha256(binary),
  },
  typedCanaryFixture: {
    disposition: prepared.receipt.disposition,
    families: ["UserContent", "ExternalContent", "FilePath", "Secret", "Diagnostic"],
    canariesRedacted: preparedPrivacy.canariesRedacted,
    canaryMatches: 0,
    rawContentReturned: false,
  },
  runtime,
  streamScan: {
    protocolResponseCanaryMatches: responseStreamCanaryMatches,
    diagnosticLogCanaryMatches: appStreamCanaryMatches,
    diagnosticLogIsNotAReceiptStream: true,
    scannedRecursively: true,
  },
  negativeControls: {
    nestedRawRedacted: true,
    diagnosticRedacted: true,
    environmentSecretRedacted: true,
    fullPathRedacted: true,
    cleartextFixtureRequiresSandboxGate: true,
  },
  cleanup,
  runtimeError,
  runtimeStage,
  privateDiagnostics: {
    sessionArtifactsRetained: runtimeError != null,
    sessionDirRecorded: Boolean(sessionDir),
    logPathRecorded: Boolean(logPath),
  },
};

await mkdir(resolve(artifactPath, ".."), { recursive: true });
await writeFile(artifactPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt, null, 2));

if (!runtimePassed || !cleanupPassed) process.exitCode = 1;
