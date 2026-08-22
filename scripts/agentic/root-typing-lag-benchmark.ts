#!/usr/bin/env bun
import { createHash } from "node:crypto";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { assertPerformanceContract } from "../devtools/lib/performance-contract.ts";
import {
  buildArtifactLifecycle,
  claimOutput,
  commitFinalReceipt,
  createOwnedStagingDirectory,
  materializeAtomic,
  removeOwnedAuxiliaryDirectory,
  retainLiveSessionArtifacts,
  validateArtifact,
  validateOutputTarget,
  waitForProcessesDead,
  writeJsonArtifactAtomic,
  type ArtifactReceipt,
  type ArtifactSpec,
  type ProtocolCorrelation,
  type RetainedArtifact,
} from "./artifact-lifecycle";

type Json = Record<string, any>;

const repoRoot = resolve(import.meta.dir, "../..");
const sessionScript = join(repoRoot, "scripts/agentic/session.sh");
const agentBinary = join(
  repoRoot,
  "target-agent",
  "pools",
  "agent-debug",
  "debug",
  "script-kit-gpui",
);

const session = argValue(
  "--session",
  `root-typing-lag-benchmark-${process.pid}-${Date.now()}`,
);
const outputDir = resolve(
  repoRoot,
  argValue(
    "--output-dir",
    join(repoRoot, ".test-output", "root-typing-lag-benchmark", session),
  ),
);
const outputPlan = validateOutputTarget({
  repoRoot,
  candidate: outputDir,
  kind: "directory",
  probeId: "root-typing-lag-benchmark",
});
const homeDir = join(outputDir, "home");
const kitDir = join(homeDir, ".scriptkit");
const dbDir = join(kitDir, "db");
const sessionRoot = join(outputDir, "sessions");
const chromeDir = join(
  homeDir,
  "Library/Application Support/Google/Chrome/Default",
);
const samples = Number(argValue("--samples", "6"));
const cadenceMs = Number(argValue("--cadence", "18"));
const timeoutMs = Number(argValue("--timeout", "12000"));
const pollMs = Number(argValue("--poll", "4"));
const stateProbeEvery = Number(argValue("--state-probe-every", "1"));
const enforce = process.argv.includes("--enforce");
const legacyPolling = process.argv.includes("--legacy-polling");
const hiddenDryRun = process.argv.includes("--hidden-dry-run");
const traceEnabled = !process.argv.includes("--no-trace");
const passiveRefreshOverlap = process.argv.includes(
  "--passive-refresh-overlap",
);
const forceBrowserTabFailure = process.argv.includes(
  "--force-browser-tabs-failure",
);
const inputMode = argValue("--input-mode", "setFilter");
const ratifiedBudgetId = argValue("--ratified-budget-id", "");
const visibleProbeOptIn = process.env.SCRIPT_KIT_ALLOW_VISIBLE_PROBES === "1";
const metricKind =
  inputMode === "printable-key"
    ? "protocol_simulated_gpui_key_to_state_echo"
    : "protocol_set_filter_to_state_echo";
// Calibration (OF-11, USER-RATIFICATION-PENDING): this metric observes each
// keystroke only after state is frame-published. Three quiet, fully correlated
// event-driven runs measured p50=22.15..22.61ms, establishing a structural
// floor of roughly one display frame plus dispatch in a debug build at 60Hz;
// see .test-output/chaos-21-l6/of11b-reval-r3/run{1,2,3}/ and summary.json.
// The earlier p50=10.51ms receipt at .test-output/of11-input-path-receipt.json
// was pipelined at about 87 updates/s and is not a per-keystroke calibration.
// Therefore the explicitly flagged, reversible enforced p50 gate is 25ms
// (floor plus margin); p95=50ms and max=150ms are unchanged. This re-baseline
// requires user ratification and must remain called out in final reporting.
// --legacy-polling remains report-only and must never drive an enforced gate.
const observationMode = legacyPolling
  ? "legacy_client_polling"
  : "event_driven_wait_for";
const observationPoint = legacyPolling
  ? "stateResult.inputValue"
  : "waitForResult.stateMatch.inputValue";
const measuresPaint = false;
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: bun scripts/agentic/root-typing-lag-benchmark.ts [options]

Options:
  --input-mode <setFilter|printable-key>  Input path to measure (default: setFilter)
  --session <name>                       Session name
  --output-dir <path>                    Per-run receipt, sandbox, and artifact directory
  --samples <count>                      Samples per scenario (default: 6)
  --cadence <ms>                         Target typing cadence (default: 18)
  --timeout <ms>                         Protocol timeout (default: 12000)
  --poll <ms>                            State polling interval (default: 4)
  --legacy-polling                       Report legacy client polling; never enforce its echo budget
  --hidden-dry-run                       Verify startup/protocol wiring, then stop before showing/focusing
  --describe-contract                    Print safe static metric/safety metadata without starting the app
  --state-probe-every <count>            Probe detailed state every N keys (default: 1)
  --scenarios <csv>                      Comma-separated queries
  --enforce                              Enforce only an explicitly ratified state-echo budget
  --ratified-budget-id <id>              Product-owner approval reference required by --enforce
  --no-trace                             Disable internal performance logs
  --passive-refresh-overlap              Delay the browser-tab fixture refresh
  --force-browser-tabs-failure           Force the browser-tab fixture to fail

Safety: visible/focused runs require SCRIPT_KIT_ALLOW_VISIBLE_PROBES=1.
This benchmark observes inputValue through event-driven waitFor by default. It does not measure paint.`);
  process.exit(0);
}
if (process.argv.includes("--describe-contract")) {
  const contract = {
    schemaVersion: 1,
    tool: "root-typing-lag-benchmark",
    evidenceClass: "STATIC_INVENTORY",
    runtimeEvidenceClass: "RUNTIME_VISIBLE",
    metricKind,
    observationPoint,
    observationMode,
    observationClass: "STATE_ECHO",
    measuresPaint: false,
    proposedBudget: {
      p50Ms: 25,
      p95Ms: 50,
      maxMs: 150,
      ratificationStatus: "USER_RATIFICATION_PENDING",
    },
    safety: {
      startsApplication: false,
      revealsWindow: false,
      drivesNativeInput: false,
      visibleRuntimeRequires: "SCRIPT_KIT_ALLOW_VISIBLE_PROBES=1",
      budgetEnforcementRequires: "--ratified-budget-id <product-owner-approval>",
    },
  };
  assertPerformanceContract(contract);
  console.log(JSON.stringify(contract, null, 2));
  process.exit(0);
}
if (process.env.SCRIPT_KIT_NONINTERACTIVE === "1") {
  throw new Error(
    "SCRIPT_KIT_NONINTERACTIVE=1 categorically refuses the root typing benchmark " +
      "before app/session launch; use only --help or --describe-contract",
  );
}
if (!hiddenDryRun && !visibleProbeOptIn) {
  throw new Error(
    "visible root typing benchmark refused before app launch; " +
      "set SCRIPT_KIT_ALLOW_VISIBLE_PROBES=1 only for an explicitly approved isolated run",
  );
}
if (!["setFilter", "printable-key"].includes(inputMode)) {
  throw new Error(`unknown --input-mode '${inputMode}'`);
}
const scenarios = argValue("--scenarios", "amz,dictat,this is the f,Hae")
  .split(",")
  .map((scenario) => scenario.trim())
  .filter(Boolean);

if (!Number.isInteger(samples) || samples <= 0) {
  throw new Error(
    `--samples must be a positive integer, got ${JSON.stringify(samples)}`,
  );
}
if (!Number.isFinite(cadenceMs) || cadenceMs < 0) {
  throw new Error(
    `--cadence must be a non-negative number, got ${JSON.stringify(cadenceMs)}`,
  );
}
if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
  throw new Error(
    `--timeout must be a positive number, got ${JSON.stringify(timeoutMs)}`,
  );
}
if (!Number.isInteger(pollMs) || pollMs <= 0) {
  throw new Error(
    `--poll must be a positive integer, got ${JSON.stringify(pollMs)}`,
  );
}
if (!Number.isInteger(stateProbeEvery) || stateProbeEvery < 0) {
  throw new Error(
    `--state-probe-every must be a non-negative integer, got ${JSON.stringify(stateProbeEvery)}`,
  );
}
if (scenarios.length === 0) {
  throw new Error("--scenarios must contain at least one non-empty query");
}
if (enforce && !traceEnabled) {
  throw new Error("--enforce requires performance tracing; remove --no-trace");
}
if (enforce && stateProbeEvery === 0) {
  throw new Error(
    "--enforce requires --state-probe-every to be greater than zero",
  );
}
if (enforce && legacyPolling) {
  throw new Error(
    "--legacy-polling is report-only and cannot be combined with --enforce",
  );
}
if (enforce && hiddenDryRun) {
  throw new Error(
    "--hidden-dry-run is diagnostic and cannot be combined with --enforce",
  );
}
if (enforce && ratifiedBudgetId.trim().length === 0) {
  throw new Error(
    "root typing state-echo budget is USER_RATIFICATION_PENDING; " +
      "--enforce requires --ratified-budget-id with an explicitly approved product-owner reference",
  );
}
assertPerformanceContract(
  {
    metricKind,
    observationPoint,
    observationClass: "STATE_ECHO",
    measuresPaint,
    runtimeEvidenceClass: "RUNTIME_VISIBLE",
    proposedBudget: {
      p50Ms: 25,
      p95Ms: 50,
      maxMs: 150,
      ratificationStatus: ratifiedBudgetId
        ? "USER_DECLARED_RATIFIED"
        : "USER_RATIFICATION_PENDING",
      approvalId: ratifiedBudgetId || null,
    },
  },
  { enforce, sampleCount: samples },
);

let sessionStatus: Json | null = null;
let mainFocusPoint: { x: number; y: number } | null = null;
let sessionOwned = false;
let ownedPid: number | null = null;
let ownedGeneration: string | null = null;
const protocolCorrelations: ProtocolCorrelation[] = [];

process.env.HOME = homeDir;
process.env.SK_PATH = kitDir;
process.env.SCRIPT_KIT_SESSION_DIR = sessionRoot;
process.env.SCRIPT_KIT_SESSION_READY_TIMEOUT_MS = "30000";
process.env.SCRIPT_KIT_STARTUP_PROFILE = "dev-fast";
process.env.SCRIPT_KIT_STARTUP_READY_LOG = "1";
process.env.SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM = "1";
process.env.SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK = "1";
if (!process.env.SCRIPT_KIT_GPUI_BINARY && fileExists(agentBinary)) {
  process.env.SCRIPT_KIT_GPUI_BINARY = agentBinary;
}
if (traceEnabled) process.env.SCRIPT_KIT_FILTER_PERF_LOG = "1";
delete process.env.SCRIPT_KIT_PREFLIGHT_DEEP_LOG;
process.env.SCRIPT_KIT_ROOT_FILE_SEARCH_TEST_PROVIDER = JSON.stringify({
  passthroughUnmatched: false,
  fixtures: scenarios.map((query) => ({
    query,
    delayMs: 0,
    results: [
      {
        path: `/tmp/root-typing-${slug(query)}.txt`,
        name: `${query} file result.txt`,
        fileType: "document",
        size: 42,
        modified: Date.now(),
      },
    ],
  })),
});
process.env.SCRIPT_KIT_BROWSER_TABS_TEST_PROVIDER = JSON.stringify(
  forceBrowserTabFailure
    ? {
        delayMs: passiveRefreshOverlap ? 350 : 0,
        fail: true,
        error: "root typing benchmark forced browser tabs failure",
        tabs: [],
      }
    : {
        delayMs: passiveRefreshOverlap ? 350 : 0,
        tabs: scenarios.map((query, index) => ({
          browser_name: "Google Chrome",
          browser_bundle_id: "com.google.Chrome",
          window_index: 1,
          tab_index: index + 1,
          title: `${query} benchmark browser tab`,
          url: `https://example.invalid/${slug(query)}/tab`,
        })),
      },
);
process.env.SCRIPT_KIT_AI_VAULT_TEST_PROVIDER = JSON.stringify(
  scenarios.map((query) => ({
    provider: "codex",
    providerDisplayName: "Codex",
    sessionId: `root-typing-${slug(query)}`,
    sourceKind: "cli",
    safeTitle: `${query} vault session`,
    workspacePath: `/tmp/root-typing-${slug(query)}-workspace`,
    model: "fixture-model",
    modifiedAt: new Date().toISOString(),
    matchedField: "title",
    stableKey: `ai-vault/codex/cli/root-typing-${slug(query)}`,
    score: 100,
  })),
);

function argValue(name: string, fallback: string): string {
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1]
    ? process.argv[index + 1]
    : fallback;
}

function slug(value: string): string {
  return (
    value
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "empty"
  );
}

function run(
  command: string,
  args: string[],
  options: { input?: string } = {},
): string {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    env: process.env,
    input: options.input,
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed\nstdout=${result.stdout}\nstderr=${result.stderr}`,
    );
  }
  return result.stdout;
}

function runSession(args: string[]): Json {
  const stdout = run(sessionScript, args).trim();
  if (!stdout)
    throw new Error(`session.sh ${args.join(" ")} produced no stdout`);
  const parsed = JSON.parse(stdout);
  if (parsed.status === "error")
    throw new Error(`session.sh ${args.join(" ")} failed: ${stdout}`);
  return parsed;
}

function stopOwnedSession(): Json {
  if (!Number.isInteger(ownedPid) || ownedPid! <= 0 || !ownedGeneration) {
    throw new Error("strict stop ownership is missing");
  }
  const args = [
    "stop",
    session,
    "--expected-pid",
    String(ownedPid),
    "--expected-generation",
    ownedGeneration,
  ];
  const result = spawnSync(sessionScript, args, {
    cwd: repoRoot,
    encoding: "utf8",
    env: process.env,
  });
  const stdout = result.stdout.trim();
  if (!stdout)
    throw new Error(`session.sh ${args.join(" ")} produced no stdout`);
  const envelope = JSON.parse(stdout);
  const ownershipExact =
    envelope.status === "ok" &&
    envelope.ownershipVerified === true &&
    envelope.expectedPid === ownedPid &&
    envelope.actualPid === ownedPid &&
    envelope.expectedGeneration === ownedGeneration &&
    envelope.actualGeneration === ownedGeneration;
  if (result.status !== 0 || !ownershipExact) {
    const error = new Error(
      `strict session stop failed: ${stdout}`,
    ) as Error & {
      stopResult?: Json;
    };
    error.stopResult = envelope;
    throw error;
  }
  return envelope;
}

function fileSize(path: string): number {
  try {
    return statSync(path).size;
  } catch {
    return 0;
  }
}

function fileExists(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function selectedBinaryPath(): string | null {
  const selected = sessionStatus?.binary ?? process.env.SCRIPT_KIT_GPUI_BINARY;
  return typeof selected === "string" && selected.length > 0
    ? resolve(repoRoot, selected)
    : null;
}

function buildProvenance(): Json {
  const binary = selectedBinaryPath();
  const gitSha = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const gitStatus = spawnSync("git", ["status", "--porcelain"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  return {
    binary,
    binarySha256:
      binary && fileExists(binary)
        ? createHash("sha256").update(readFileSync(binary)).digest("hex")
        : null,
    gitSha: gitSha.status === 0 ? gitSha.stdout.trim() : null,
    sourceDirty:
      gitStatus.status === 0 ? gitStatus.stdout.trim().length > 0 : null,
  };
}

function readFrom(path: string, offset: number): string {
  try {
    return readFileSync(path).subarray(offset).toString("utf8");
  } catch {
    return "";
  }
}

function waitUntil<T>(timeout: number, fn: () => T | null): T {
  const deadline = performance.now() + timeout;
  const sleeper = new Int32Array(new SharedArrayBuffer(4));
  while (performance.now() < deadline) {
    const value = fn();
    if (value) return value;
    Atomics.wait(sleeper, 0, 0, pollMs);
  }
  throw new Error("timed out waiting for session response");
}

function directWrite(command: Json) {
  if (!sessionStatus?.pipe) throw new Error("missing session pipe");
  appendFileSync(sessionStatus.pipe, `${JSON.stringify(command)}\n`);
}

function directRpc(command: Json, expect: string, timeout = timeoutMs): Json {
  command.requestId ??= `root-typing-rpc-${Date.now()}`;
  protocolCorrelations.push({
    requestId: command.requestId,
    expectedType: expect,
  });
  const responses = String(sessionStatus?.responses ?? "");
  const responseOffset = fileSize(responses);
  const protocolResponses = String(sessionStatus?.protocolResponses ?? "");
  const protocolOffset = fileSize(protocolResponses);
  const logPath = String(sessionStatus?.log ?? "");
  const logOffset = fileSize(logPath);
  directWrite(command);
  const envelope = waitUntil(timeout, () => {
    for (const tail of [
      readFrom(responses, responseOffset),
      readFrom(protocolResponses, protocolOffset),
    ]) {
      for (const line of tail.split("\n")) {
        if (!line.trim()) continue;
        try {
          const parsed = JSON.parse(line);
          if (parsed.requestId === command.requestId) return parsed;
        } catch {}
      }
    }
    const logTail = readFrom(logPath, logOffset);
    for (const line of logTail.split("\n")) {
      const jsonStart = line.indexOf("{");
      if (jsonStart < 0) continue;
      try {
        const parsed = JSON.parse(line.slice(jsonStart));
        if (parsed.requestId === command.requestId && parsed.type === expect) {
          return { status: "ok", responseType: expect, response: parsed };
        }
      } catch {}
    }
    return null;
  });
  if (
    envelope.kind === "protocolResponse" &&
    envelope.responseType === expect
  ) {
    return envelope.response;
  }
  if (envelope.status !== "ok" || envelope.responseType !== expect) {
    throw new Error(
      `unexpected direct rpc envelope: ${JSON.stringify(envelope)}`,
    );
  }
  return envelope.response;
}

function directSend(command: Json): number {
  const start = performance.now();
  directWrite(command);
  return performance.now() - start;
}

function showMainWindow() {
  directRpc(
    { type: "show", requestId: `root-typing-show-${Date.now()}` },
    "windowVisibilityAck",
  );
  const result = directRpc(
    {
      type: "waitFor",
      requestId: `root-typing-wait-visible-${Date.now()}`,
      condition: {
        type: "stateMatch",
        state: { promptType: "none", windowVisible: true },
      },
      timeout: timeoutMs,
      pollInterval: pollMs,
    },
    "waitForResult",
    timeoutMs + 1_000,
  );
  if (result.success !== true) {
    throw new Error(
      `main window did not become visible: ${JSON.stringify(result)}`,
    );
  }

  const windows = directRpc(
    {
      type: "listAutomationWindows",
      requestId: `root-typing-windows-${Date.now()}`,
    },
    "automationWindowListResult",
  );
  const main = Array.isArray(windows.windows)
    ? windows.windows.find((window: Json) => window.id === "main")
    : null;
  if (
    !main?.bounds ||
    !Number.isFinite(main.bounds.width) ||
    !Number.isFinite(main.bounds.height)
  ) {
    throw new Error(
      `main window bounds unavailable: ${JSON.stringify(windows)}`,
    );
  }
  mainFocusPoint = {
    x: main.bounds.width / 2,
    y: Math.max(1, main.bounds.height - 90),
  };
  if (inputMode === "printable-key") ensureFilterInputFocus("show");
}

function getState(tag: string): Json {
  return directRpc(
    { type: "getState", requestId: `root-typing-state-${tag}-${Date.now()}` },
    "stateResult",
  );
}

function waitForInputLocally(input: string, tag: string): number {
  const start = performance.now();
  const deadline = start + timeoutMs;
  let lastState: Json | null = null;
  while (performance.now() < deadline) {
    lastState = getState(`${tag}-poll`);
    if (lastState.windowVisible === true && lastState.inputValue === input) {
      return performance.now() - start;
    }
    sleepSync(Math.max(1, pollMs));
  }
  throw new Error(
    `timed out polling ${observationPoint} for ${JSON.stringify(input)}: ${JSON.stringify(
      {
        promptType: lastState?.promptType ?? null,
        inputValue: lastState?.inputValue ?? null,
        windowVisible: lastState?.windowVisible ?? null,
      },
    )}`,
  );
}

function waitForInputEventDriven(input: string, tag: string): number {
  const start = performance.now();
  const result = directRpc(
    {
      type: "waitFor",
      requestId: `root-typing-echo-${tag}-${Date.now()}`,
      condition: {
        type: "stateMatch",
        // Input echo is the measured contract. Do not couple it to promptType:
        // launcher state can legitimately transition while an empty clear is
        // already visible, which made the composite wait miss a real echo.
        state: { windowVisible: true, inputValue: input },
      },
      timeout: timeoutMs,
      pollInterval: pollMs,
    },
    "waitForResult",
    timeoutMs + 1_000,
  );
  if (result.success !== true) {
    throw new Error(
      `event-driven input observation failed: ${JSON.stringify(result)}`,
    );
  }
  return performance.now() - start;
}

function waitForInput(input: string, tag: string): number {
  return legacyPolling
    ? waitForInputLocally(input, tag)
    : waitForInputEventDriven(input, tag);
}

function ensureFilterInputFocus(tag: string) {
  if (!mainFocusPoint) throw new Error("main focus point is unavailable");
  for (const type of ["mouseDown", "mouseUp"]) {
    const dispatch = directRpc(
      {
        type: "simulateGpuiEvent",
        requestId: `root-typing-focus-${tag}-${type}-${Date.now()}`,
        target: { type: "main" },
        event: { type, ...mainFocusPoint },
      },
      "simulateGpuiEventResult",
    );
    const acknowledged =
      dispatch.dispatchCompleted === true ||
      dispatch.dispatchScheduled === true;
    if (dispatch.success !== true || !acknowledged) {
      throw new Error(
        `main filter focus dispatch failed: ${JSON.stringify(dispatch)}`,
      );
    }
  }

  const deadline = performance.now() + timeoutMs;
  let focusedSamples = 0;
  let lastElements: Json | null = null;
  while (performance.now() < deadline) {
    lastElements = directRpc(
      {
        type: "getElements",
        requestId: `root-typing-focus-elements-${tag}-${Date.now()}`,
        target: { type: "main" },
      },
      "elementsResult",
    );
    const filterInput = Array.isArray(lastElements.elements)
      ? lastElements.elements.find(
          (element: Json) => element.semanticId === "input:filter",
        )
      : null;
    const focused =
      lastElements.focusedSemanticId === "input:filter" ||
      filterInput?.focused === true;
    focusedSamples = focused ? focusedSamples + 1 : 0;
    if (focusedSamples >= 2) return;
    sleepSync(Math.max(1, pollMs));
  }
  throw new Error(
    `main filter input did not retain focus: ${JSON.stringify({
      focusedSemanticId: lastElements?.focusedSemanticId ?? null,
    })}`,
  );
}

function sleepSync(ms: number) {
  if (ms <= 0) return;
  const sleeper = new Int32Array(new SharedArrayBuffer(4));
  Atomics.wait(sleeper, 0, 0, ms);
}

function sql(path: string, input: string) {
  run("sqlite3", [path], { input });
}

function seedFixtures() {
  mkdirSync(dbDir, { recursive: true });
  mkdirSync(chromeDir, { recursive: true });
  mkdirSync(join(kitDir, "plugins", "main", "scripts"), { recursive: true });
  mkdirSync(join(kitDir, "models", "brain"), { recursive: true });
  writeFileSync(
    join(kitDir, "models", "brain", ".no-download"),
    "probe fixture\n",
  );

  const now = new Date().toISOString();
  for (const query of scenarios) {
    writeFileSync(
      join(kitDir, "plugins", "main", "scripts", `${slug(query)}.ts`),
      `// Name: ${query} script\nconsole.log("fixture");\n`,
    );
  }

  sql(
    join(dbDir, "notes.sqlite"),
    `
CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  is_pinned INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0
);
CREATE VIRTUAL TABLE notes_fts USING fts5(title, content, content='notes', content_rowid='rowid');
${scenarios
  .map(
    (query, index) =>
      `INSERT INTO notes (id, title, content, created_at, updated_at, deleted_at, is_pinned, sort_order) VALUES ('${index}3333333-3333-4333-8333-333333333333', '${query} note', '${query} note content', '${now}', '${now}', NULL, 0, ${index});`,
  )
  .join("\n")}
INSERT INTO notes_fts(rowid, title, content) SELECT rowid, title, content FROM notes;
`,
  );

  sql(
    join(dbDir, "clipboard-history.sqlite"),
    `
CREATE TABLE history (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  content_hash TEXT,
  content_type TEXT NOT NULL DEFAULT 'text',
  timestamp INTEGER NOT NULL,
  pinned INTEGER DEFAULT 0,
  ocr_text TEXT,
  text_preview TEXT,
  image_width INTEGER,
  image_height INTEGER,
  byte_size INTEGER
);
${scenarios
  .map(
    (query, index) =>
      `INSERT INTO history VALUES ('clip-root-typing-${index}', '${query} clipboard text', 'fixture-hash-${index}', 'text', ${Date.now() + index}, 0, NULL, '${query} clipboard text', NULL, NULL, ${query.length + 15});`,
  )
  .join("\n")}
`,
  );

  writeFileSync(
    join(kitDir, "dictation-history.jsonl"),
    scenarios
      .map((query) =>
        JSON.stringify({
          id: `dictation-root-typing-${slug(query)}`,
          timestamp: now,
          transcript: `${query} dictation transcript`,
          preview: `${query} dictation transcript`,
          target: "Main Filter",
          audio_duration_ms: 1200,
        }),
      )
      .join("\n") + "\n",
  );

  writeFileSync(
    join(kitDir, "agent_chat-history.jsonl"),
    scenarios
      .map((query) =>
        JSON.stringify({
          timestamp: now,
          first_message: `${query} conversation prompt`,
          message_count: 2,
          session_id: `agent_chat-root-typing-${slug(query)}`,
          title: `${query} conversation prompt`,
          preview: `${query} conversation reply`,
          search_text: `${query} conversation prompt ${query} conversation reply`,
        }),
      )
      .join("\n") + "\n",
  );

  const chromeTime = (Math.floor(Date.now() / 1000) + 11644473600) * 1000000;
  sql(
    join(chromeDir, "History"),
    `
CREATE TABLE urls (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT,
  visit_count INTEGER NOT NULL DEFAULT 0,
  typed_count INTEGER NOT NULL DEFAULT 0,
  last_visit_time INTEGER NOT NULL DEFAULT 0
);
${scenarios
  .map(
    (query, index) =>
      `INSERT INTO urls VALUES (${index + 1}, 'https://example.invalid/${slug(query)}/history', '${query} browser history', 7, 2, ${chromeTime + index});`,
  )
  .join("\n")}
`,
  );
}

function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(
    sorted.length - 1,
    Math.ceil((p / 100) * sorted.length) - 1,
  );
  return Number(sorted[index].toFixed(3));
}

function stats(values: number[]) {
  return {
    count: values.length,
    p50Ms: percentile(values, 50),
    p95Ms: percentile(values, 95),
    maxMs: Number(Math.max(0, ...values).toFixed(3)),
  };
}

function hash(value: unknown): string {
  return createHash("sha256")
    .update(JSON.stringify(value))
    .digest("hex")
    .slice(0, 16);
}

function setFilter(text: string, tag: string) {
  const sendMs = directSend({
    type: "setFilter",
    text,
    requestId: `root-typing-set-${tag}-${Date.now()}`,
  });
  const echoWaitMs = waitForInput(text, tag);
  return {
    text,
    metricKind: "protocol_set_filter_to_state_echo",
    observationPoint,
    measuresPaint,
    sendMs: Number(sendMs.toFixed(3)),
    inputEchoMs: Number((sendMs + echoWaitMs).toFixed(3)),
  };
}

function printableKey(next: string, tag: string) {
  const key = next.at(-1);
  if (!key) throw new Error("printable-key requires one appended character");
  const started = performance.now();
  const dispatch = directRpc(
    {
      type: "simulateGpuiEvent",
      requestId: `root-typing-printable-key-${tag}-${Date.now()}`,
      target: { type: "main" },
      event: { type: "keyDown", key, text: key, modifiers: [] },
    },
    "simulateGpuiEventResult",
  );
  const protocolRoundTripMs = performance.now() - started;
  const dispatchAcknowledged =
    dispatch.dispatchCompleted === true || dispatch.dispatchScheduled === true;
  if (dispatch.success !== true || !dispatchAcknowledged) {
    throw new Error(
      `printable key dispatch failed: ${JSON.stringify(dispatch)}`,
    );
  }
  const echoWaitMs = waitForInput(next, tag);
  return {
    text: next,
    metricKind: "protocol_simulated_gpui_key_to_state_echo",
    observationPoint,
    measuresPaint,
    sendMs: Number(protocolRoundTripMs.toFixed(3)),
    protocolRoundTripMs: Number(protocolRoundTripMs.toFixed(3)),
    dispatchPath: dispatch.dispatchPath ?? null,
    resolvedWindowId: dispatch.resolvedWindowId ?? null,
    dispatchCompleted: dispatch.dispatchCompleted,
    dispatchScheduled: dispatch.dispatchScheduled,
    activationProof: dispatch.activationProof ?? null,
    inputEchoMs: Number((protocolRoundTripMs + echoWaitMs).toFixed(3)),
  };
}

function applyTypedInput(next: string, tag: string) {
  if (inputMode === "printable-key") return printableKey(next, tag);
  return setFilter(next, tag);
}

function clearInput(tag: string) {
  let state = getState(`${tag}-before-clear`);
  if (state.promptType !== "none" || state.windowVisible !== true) {
    showMainWindow();
    state = getState(`${tag}-after-show`);
  }
  if (state.inputValue === "") return;

  if (inputMode === "setFilter") {
    setFilter("", `${tag}-clear`);
    return;
  }

  ensureFilterInputFocus(`${tag}-before-clear`);

  const dispatch = directRpc(
    {
      type: "simulateGpuiEvent",
      requestId: `root-typing-clear-${tag}-${Date.now()}`,
      target: { type: "main" },
      event: { type: "keyDown", key: "escape", modifiers: [] },
    },
    "simulateGpuiEventResult",
  );
  const dispatchAcknowledged =
    dispatch.dispatchCompleted === true || dispatch.dispatchScheduled === true;
  if (dispatch.success !== true || !dispatchAcknowledged) {
    throw new Error(
      `printable input clear dispatch failed: ${JSON.stringify(dispatch)}`,
    );
  }
  waitForInput("", tag);
}

function typeScenario(query: string, sampleIndex: number) {
  clearInput(`${slug(query)}-${sampleIndex}-clear`);
  const events = [];
  let current = "";
  let cadenceOverrunMaxMs = 0;
  for (let index = 0; index < query.length; index += 1) {
    current += query[index];
    if (inputMode === "printable-key") {
      ensureFilterInputFocus(`${slug(query)}-${sampleIndex}-${index}`);
    }
    const tickStarted = performance.now();
    const event = applyTypedInput(
      current,
      `${slug(query)}-${sampleIndex}-${index}`,
    );
    const echoElapsed = performance.now() - tickStarted;
    const state =
      stateProbeEvery > 0 && index % stateProbeEvery === 0
        ? getState(`${slug(query)}-${sampleIndex}-${index}`)
        : null;
    const elapsed = performance.now() - tickStarted;
    cadenceOverrunMaxMs = Math.max(
      cadenceOverrunMaxMs,
      echoElapsed - cadenceMs,
    );
    events.push({
      index,
      expected: current,
      expectedLength: current.length,
      inputMode,
      ...event,
      computedMatchesInput: state
        ? state.mainWindowPreflight?.computedSearchText === current
        : null,
      visibleResultCount:
        state?.mainWindowPreflight?.visibleResults?.length ?? null,
      preflightFingerprint: state
        ? hash(state.mainWindowPreflight?.visibleResults ?? [])
        : null,
    });
    if (elapsed < cadenceMs) sleepSync(cadenceMs - elapsed);
  }
  return {
    kind: "typing",
    query,
    sampleIndex,
    cadenceOverrunMaxMs: Number(cadenceOverrunMaxMs.toFixed(3)),
    events,
  };
}

function duplicateEmptyInput(sampleIndex: number) {
  clearInput(`empty-${sampleIndex}-clear`);
  const observeEmptySet = (tag: string) => {
    const started = performance.now();
    const sendMs = directSend({
      type: "setFilter",
      text: "",
      requestId: `root-typing-set-${tag}-${Date.now()}`,
    });
    const state = getState(tag);
    return {
      state,
      receipt: {
        text: "",
        metricKind: "protocol_set_filter_to_state_echo",
        observationPoint,
        measuresPaint,
        sendMs: Number(sendMs.toFixed(3)),
        inputEchoMs: Number((performance.now() - started).toFixed(3)),
      },
    };
  };
  const first = observeEmptySet(`empty-${sampleIndex}-first`);
  const second = observeEmptySet(`empty-${sampleIndex}-second`);
  return {
    kind: "duplicate-empty",
    sampleIndex,
    first: first.receipt,
    second: second.receipt,
    inputValue: second.state.inputValue,
    computedSearchText:
      second.state.mainWindowPreflight?.computedSearchText ?? null,
  };
}

function maxLogLineBytes(log: string): number {
  return Math.max(
    0,
    ...log
      .split("\n")
      .filter((line) => {
        if (line.includes('"type":"stateResult"')) return false;
        if (line.includes('"type":"elementsResult"')) return false;
        if (line.includes('"type":"layoutInfoResult"')) return false;
        return true;
      })
      .map((line) => Buffer.byteLength(line)),
  );
}

function parsePerfLogs(logPath: string) {
  const log = readFileSync(logPath, "utf8");
  const numbers = (regex: RegExp) =>
    [...log.matchAll(regex)].map((match) => Number(match[1]));
  const handlerDurations = numbers(
    /handle_filter_input_change took ([0-9.]+)ms/g,
  );
  const applyDurations = numbers(/APPLY_FILTER_DONE in ([0-9.]+)ms/g);
  const groupDurations = numbers(/GROUP_DONE '?[^'\n]*'? in ([0-9.]+)ms/g);
  const searchDurations = numbers(
    /SEARCH_TOTAL[^:]*: sort=[0-9.]+ms total=([0-9.]+)ms/g,
  );
  const refreshStarted = (
    log.match(/root_passive_snapshot_refresh_started/g) ?? []
  ).length;
  const refreshFailed = (
    log.match(/root_passive_snapshot_refresh_failed/g) ?? []
  ).length;
  const preflightDeepLineCount = (
    log.match(/visible_row_fingerprint":"(?:[^"]{512,})/g) ?? []
  ).length;
  const passiveSources = [
    ...log.matchAll(
      /\[PASSIVE_SOURCE_DONE\] source=([a-z_]+) query_len=([0-9]+) explicit=(true|false) in ([0-9.]+)ms -> ([0-9]+) hits/g,
    ),
  ].map((match) => ({
    source: match[1],
    queryLen: Number(match[2]),
    explicit: match[3] === "true",
    ms: Number(match[4]),
    hits: Number(match[5]),
  }));
  const passiveDurations = passiveSources.map((entry) => entry.ms);
  const implicitPassiveDurations = passiveSources
    .filter((entry) => !entry.explicit)
    .map((entry) => entry.ms);
  const slowestPassiveSources = [...passiveSources]
    .sort((a, b) => b.ms - a.ms)
    .slice(0, 10);
  return {
    applyFilterDone: stats(applyDurations),
    groupDone: stats(groupDurations),
    searchTotal: stats(searchDurations),
    handlerSlow: stats(handlerDurations),
    handlerSlowCount: handlerDurations.length,
    browserTabsRefreshStartCount: refreshStarted,
    browserTabsRefreshFailedCount: refreshFailed,
    passiveSources: {
      all: stats(passiveDurations),
      implicit: stats(implicitPassiveDurations),
      count: passiveSources.length,
      slowest: slowestPassiveSources,
    },
    preflightDeepLineCount,
    maxLogLineBytes: maxLogLineBytes(log),
  };
}

async function runBenchmark() {
  const beforeStatus = runSession(["status", session]);
  if (beforeStatus.alive === true || beforeStatus.healthy === true) {
    throw new Error(
      `refusing to reuse running session ${JSON.stringify(session)}`,
    );
  }
  seedFixtures();
  const startStatus = runSession(["start", session]);
  if (startStatus.resumed === true) {
    throw new Error(
      `refusing to claim resumed session ${JSON.stringify(session)}`,
    );
  }
  ownedPid = Number(startStatus.pid);
  ownedGeneration =
    typeof startStatus.sessionGeneration === "string"
      ? startStatus.sessionGeneration
      : null;
  if (!Number.isInteger(ownedPid) || ownedPid! <= 0 || !ownedGeneration) {
    throw new Error(
      `fresh session did not report ownership: ${JSON.stringify(startStatus)}`,
    );
  }
  sessionOwned = true;
  const liveStatus = runSession(["status", session]);
  sessionStatus = { ...startStatus, ...liveStatus };
  if (startStatus.ready !== true) {
    throw new Error(
      `owned session did not reach protocol readiness: ${JSON.stringify(startStatus)}`,
    );
  }

  if (hiddenDryRun) {
    const hiddenState = getState("hidden-dry-run");
    if (hiddenState.windowVisible !== false) {
      throw new Error(
        `hidden dry-run unexpectedly found a visible window: ${JSON.stringify(hiddenState)}`,
      );
    }
    throw new Error(
      `hidden dry-run reached expected show/focus boundary (${observationMode}); frontmost validation required`,
    );
  }

  showMainWindow();
  setFilter(scenarios[0] ?? "warm", "warm");

  const typingReceipts = [];
  for (let sampleIndex = 0; sampleIndex < samples; sampleIndex += 1) {
    for (const query of scenarios) {
      typingReceipts.push(typeScenario(query, sampleIndex));
    }
  }

  const emptyReceipts = [];
  if (inputMode === "setFilter") {
    for (let sampleIndex = 0; sampleIndex < samples; sampleIndex += 1) {
      emptyReceipts.push(duplicateEmptyInput(sampleIndex));
    }
  }

  const events = typingReceipts.flatMap((receipt) => receipt.events);
  const expectedEventCount =
    samples * scenarios.reduce((total, scenario) => total + scenario.length, 0);
  const stateObservationCount = events.filter(
    (event) => event.computedMatchesInput !== null,
  ).length;
  const computedMismatchCount = events.filter(
    (event) => event.computedMatchesInput === false,
  ).length;
  const emptyMismatchCount = emptyReceipts.filter(
    (receipt) => receipt.inputValue !== "" || receipt.computedSearchText !== "",
  ).length;
  const summary: Json = {
    typing: {
      inputEcho: stats(events.map((event) => event.inputEchoMs)),
      send: stats(events.map((event) => event.sendMs)),
      cadenceMs,
      cadenceOverrunMaxMs: Number(
        Math.max(
          0,
          ...typingReceipts.map((receipt) => receipt.cadenceOverrunMaxMs),
        ).toFixed(3),
      ),
      expectedEventCount,
      stateObservationCount,
      computedMismatchCount,
    },
    duplicateEmpty: {
      applicable: inputMode === "setFilter",
      inputEcho: stats(
        emptyReceipts.flatMap((receipt) => [
          receipt.first.inputEchoMs,
          receipt.second.inputEchoMs,
        ]),
      ),
      mismatchCount: emptyMismatchCount,
    },
  };

  return {
    schemaVersion: 4,
    status: "pending-finalization",
    behavior: { status: "fail", failure: null },
    executionMode: enforce ? "gate" : "diagnostic",
    thresholdStatus: "not-evaluated",
    scenarios,
    samples,
    cadenceMs,
    inputMode,
    metricKind,
    observationPoint,
    observationMode,
    observationClass: "STATE_ECHO",
    measuresPaint,
    budgetRatification: {
      status: ratifiedBudgetId
        ? "USER_DECLARED_RATIFIED"
        : "USER_RATIFICATION_PENDING",
      approvalId: ratifiedBudgetId || null,
    },
    traceEnabled,
    passiveRefreshOverlap,
    forceBrowserTabFailure,
    hiddenDryRun,
    enforce,
    outputDir,
    provenance: buildProvenance(),
    session: { name: session },
    summary,
    typingReceipts,
    emptyReceipts,
  };
}

function evaluateFinalizedBehavior(
  receipt: Json,
  durableLogPath: string,
): void {
  if (!receipt.summary) return;
  const summary = receipt.summary;
  summary.perfLogs = parsePerfLogs(durableLogPath);
  const failures: string[] = [];
  const events = receipt.typingReceipts.flatMap((item: Json) => item.events);
  const expectedEventCount = summary.typing.expectedEventCount;
  const stateObservationCount = summary.typing.stateObservationCount;
  if (events.length !== expectedEventCount) {
    failures.push(
      `typing event count ${events.length} != expected ${expectedEventCount}`,
    );
  }
  if (!legacyPolling && summary.typing.inputEcho.p50Ms > 25)
    failures.push("typing inputEcho p50 > 25ms");
  if (!legacyPolling && summary.typing.inputEcho.p95Ms > 50)
    failures.push("typing inputEcho p95 > 50ms");
  if (!legacyPolling && summary.typing.inputEcho.maxMs > 150)
    failures.push("typing inputEcho max > 150ms");
  if (summary.typing.cadenceOverrunMaxMs > 75)
    failures.push("typing cadence overrun max > 75ms");
  if (summary.typing.computedMismatchCount !== 0)
    failures.push("computedSearchText mismatch");
  if (summary.duplicateEmpty.mismatchCount !== 0)
    failures.push("duplicate empty final mismatch");
  if (summary.perfLogs.handlerSlowCount !== 0)
    failures.push("handler slow logs present");
  if (summary.perfLogs.groupDone.p95Ms > 35)
    failures.push("GROUP_DONE p95 > 35ms");
  if (summary.perfLogs.searchTotal.p95Ms > 15)
    failures.push("SEARCH_TOTAL p95 > 15ms");
  if (summary.perfLogs.passiveSources.all.maxMs > 20)
    failures.push("passive source max > 20ms");
  if (summary.perfLogs.passiveSources.implicit.maxMs > 12)
    failures.push("implicit passive source max > 12ms");
  if (summary.perfLogs.maxLogLineBytes > 2048)
    failures.push("max log line bytes > 2048");
  if (summary.perfLogs.preflightDeepLineCount !== 0)
    failures.push("deep preflight lines present");
  if (enforce && stateObservationCount === 0)
    failures.push("no semantic state observations");
  if (enforce && summary.perfLogs.groupDone.count === 0)
    failures.push("no GROUP_DONE observations");
  if (enforce && summary.perfLogs.searchTotal.count === 0)
    failures.push("no SEARCH_TOTAL observations");
  if (enforce && summary.perfLogs.passiveSources.count === 0) {
    failures.push("no passive-source observations");
  }

  receipt.thresholds = {
    inputEchoP50Ms: 25,
    inputEchoP95Ms: 50,
    inputEchoMaxMs: 150,
    inputEchoEnforced: enforce && !legacyPolling,
    observationClass: "STATE_ECHO",
    measuresPaint: false,
    ratificationStatus: ratifiedBudgetId
      ? "USER_DECLARED_RATIFIED"
      : "USER_RATIFICATION_PENDING",
    ratificationReference: ratifiedBudgetId || null,
    cadenceOverrunMaxMs: 75,
    groupDoneP95Ms: 35,
    searchTotalP95Ms: 15,
    passiveSourceMaxMs: 20,
    implicitPassiveSourceMaxMs: 12,
    maxLogLineBytes: 2048,
    failures,
  };
  receipt.thresholdStatus = failures.length === 0 ? "pass" : "fail";
  receipt.behavior = {
    status:
      failures.length === 0 ? "pass" : enforce ? "fail" : "diagnostic-warning",
    failure: failures.length === 0 ? null : failures.join("; "),
  };
}

function sessionArtifactSpecs(): ArtifactSpec[] {
  return [
    {
      id: "app-log",
      sourceName: "app.log",
      required: true,
      mediaType: "text/plain",
      kind: "text",
      acceptedTextMarkers: ["STARTUP_READY ", "APP_READY|"],
    },
    {
      id: "protocol-responses",
      sourceName: "protocol-responses.ndjson",
      required: true,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      correlations: protocolCorrelations,
    },
    {
      id: "lifecycle",
      sourceName: "lifecycle.json",
      required: true,
      mediaType: "application/json",
      kind: "json",
    },
    {
      id: "responses",
      sourceName: "responses.ndjson",
      required: false,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      requireNonEmpty: false,
    },
    {
      id: "raw-lifecycle",
      sourceName: "lifecycle.ndjson",
      required: false,
      mediaType: "application/x-ndjson",
      kind: "ndjson",
      requireNonEmpty: false,
    },
    {
      id: "app-exit",
      sourceName: "app-exit.json",
      required: false,
      mediaType: "application/json",
      kind: "json",
      requireNonEmpty: false,
    },
  ];
}

function readSessionPid(name: "supervisor_pid" | "fwd_pid"): number | null {
  try {
    const pid = Number(
      readFileSync(join(sessionRoot, session, name), "utf8").trim(),
    );
    return Number.isInteger(pid) && pid > 0 ? pid : null;
  } catch {
    return null;
  }
}

function verifyCurrentOwnership(): Json {
  const sessionDir = join(sessionRoot, session);
  const actualPid = Number(
    readFileSync(join(sessionDir, "pid"), "utf8").trim(),
  );
  const actualGeneration = readFileSync(
    join(sessionDir, "generation"),
    "utf8",
  ).trim();
  const exact = actualPid === ownedPid && actualGeneration === ownedGeneration;
  if (!exact) {
    throw new Error(
      `ownership mismatch before retention: ${JSON.stringify({
        expectedPid: ownedPid,
        actualPid,
        expectedGeneration: ownedGeneration,
        actualGeneration,
      })}`,
    );
  }
  return {
    expectedPid: ownedPid,
    actualPid,
    expectedGeneration: ownedGeneration,
    actualGeneration,
    exact,
  };
}

async function main() {
  const claim = claimOutput(outputPlan);
  let receipt: Json = {
    schemaVersion: 4,
    status: "error",
    behavior: { status: "fail", failure: null },
    executionMode: enforce ? "gate" : "diagnostic",
    thresholdStatus: "not-evaluated",
    scenarios,
    samples,
    cadenceMs,
    inputMode,
    metricKind,
    observationPoint,
    observationMode,
    observationClass: "STATE_ECHO",
    measuresPaint,
    budgetRatification: {
      status: ratifiedBudgetId
        ? "USER_DECLARED_RATIFIED"
        : "USER_RATIFICATION_PENDING",
      approvalId: ratifiedBudgetId || null,
    },
    traceEnabled,
    passiveRefreshOverlap,
    forceBrowserTabFailure,
    hiddenDryRun,
    enforce,
    outputDir,
    provenance: buildProvenance(),
    session: { name: session },
  };
  let runError: string | null = null;
  let cleanupError: string | null = null;
  let stopResult: Json | null = null;
  let afterStatus: Json | null = null;
  let hiddenState: Json | null = null;
  let stagingDir: string | null = null;
  let retained: RetainedArtifact[] = [];
  let writers: Record<string, number | null> = {
    app: null,
    supervisor: null,
    forwarder: null,
  };
  let writersDead: Record<string, boolean> = {};
  const specs = sessionArtifactSpecs();
  const artifacts: ArtifactReceipt[] = [];
  try {
    receipt = await runBenchmark();
  } catch (error) {
    runError = error instanceof Error ? error.message : String(error);
    receipt.behavior = { status: "fail", failure: runError };
  } finally {
    if (sessionOwned) {
      try {
        directRpc(
          { type: "hide", requestId: `root-typing-cleanup-hide-${Date.now()}` },
          "windowVisibilityAck",
        );
        const hidden = directRpc(
          {
            type: "waitFor",
            requestId: `root-typing-cleanup-hidden-${Date.now()}`,
            condition: { type: "stateMatch", state: { windowVisible: false } },
            timeout: timeoutMs,
            pollInterval: pollMs,
          },
          "waitForResult",
          timeoutMs + 1_000,
        );
        if (hidden.success !== true) {
          throw new Error(
            `window did not become hidden: ${JSON.stringify(hidden)}`,
          );
        }
        hiddenState = getState("cleanup-hidden");
        if (hiddenState.windowVisible !== false) {
          throw new Error(
            `cleanup state remained visible: ${JSON.stringify(hiddenState)}`,
          );
        }
      } catch (error) {
        cleanupError = error instanceof Error ? error.message : String(error);
      }
    }

    if (sessionOwned) {
      try {
        receipt.ownershipBeforeStop = verifyCurrentOwnership();
        writers = {
          app: ownedPid,
          supervisor: readSessionPid("supervisor_pid"),
          forwarder: readSessionPid("fwd_pid"),
        };
        stagingDir = createOwnedStagingDirectory(claim);
        retained = retainLiveSessionArtifacts(
          claim,
          join(sessionRoot, session),
          stagingDir,
          specs.filter((spec) => spec.id !== "lifecycle"),
        );
        stopResult = stopOwnedSession();
        writersDead = await waitForProcessesDead(writers, { timeoutMs });
        afterStatus = runSession(["status", session]);
        if (afterStatus.status !== "not_found" || afterStatus.alive !== false) {
          throw new Error(
            `final session status is not not_found: ${JSON.stringify(afterStatus)}`,
          );
        }
      } catch (error) {
        stopResult =
          (error as Error & { stopResult?: Json }).stopResult ?? stopResult;
        const stopError =
          error instanceof Error ? error.message : String(error);
        cleanupError = cleanupError
          ? `${cleanupError}; finalization: ${stopError}`
          : `finalization: ${stopError}`;
      }
    }

    const writersFinalized =
      sessionOwned &&
      stopResult?.status === "ok" &&
      stopResult?.ownershipVerified === true &&
      afterStatus?.status === "not_found" &&
      Object.values(writers).every(
        (pid) => Number.isInteger(pid) && pid! > 0,
      ) &&
      writersDead.app === true &&
      writersDead.supervisor === true &&
      writersDead.forwarder === true;
    if (writersFinalized) {
      try {
        for (const retainedArtifact of retained) {
          const spec = specs.find(
            (candidate) => candidate.id === retainedArtifact.id,
          )!;
          materializeAtomic(claim, {
            sourceRoot: stagingDir!,
            sourceName: spec.destinationName ?? spec.sourceName,
            destinationName: spec.destinationName ?? spec.sourceName,
          });
        }
        writeJsonArtifactAtomic(claim, "lifecycle.json", {
          schemaVersion: 1,
          probeId: "root-typing-lag-benchmark",
          runId: claim.owner.runId,
          finalizationKind: "strict-session-stop",
          hidden: hiddenState?.windowVisible === false,
          app: { pid: writers.app, dead: writersDead.app === true },
          supervisor: {
            pid: writers.supervisor,
            dead: writersDead.supervisor === true,
          },
          forwarder: {
            pid: writers.forwarder,
            dead: writersDead.forwarder === true,
          },
          ownership: receipt.ownershipBeforeStop,
          stop: {
            wasRunning: stopResult.wasRunning,
            forcedKill: stopResult.forcedKill ?? null,
            finalStatus: afterStatus.status,
          },
          completedAt: new Date().toISOString(),
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        cleanupError = cleanupError
          ? `${cleanupError}; artifacts: ${message}`
          : `artifacts: ${message}`;
      }
    }

    for (const spec of specs) {
      artifacts.push(
        validateArtifact(
          join(claim.artifactsRoot, spec.destinationName ?? spec.sourceName),
          spec,
          claim.artifactsRoot,
        ),
      );
    }
    const durableLog = artifacts.find((artifact) => artifact.id === "app-log");
    if (!runError && durableLog?.readable) {
      try {
        evaluateFinalizedBehavior(receipt, durableLog.path);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        receipt.behavior = { status: "fail", failure: message };
      }
    }
    receipt.artifactLifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "strict-session-stop",
      writersFinalized,
      specs,
      artifacts,
    });
    receipt.cleanup = {
      attempted: sessionOwned,
      hidden: hiddenState?.windowVisible === false,
      hiddenState,
      stopped: writersFinalized,
      result: stopResult,
      finalStatus: afterStatus?.status ?? null,
      writers,
      writersDead,
      error: cleanupError,
    };
    receipt.provenance = buildProvenance();
    receipt.session = {
      name: session,
      pid: ownedPid,
      generation: ownedGeneration,
    };
    const lifecycleValid =
      receipt.artifactLifecycle.allRequiredValid === true &&
      receipt.artifactLifecycle.allRecordedPathsReadable === true;
    const behaviorAcceptable =
      receipt.behavior.status === "pass" ||
      (!enforce && receipt.behavior.status === "diagnostic-warning");
    receipt.status =
      lifecycleValid &&
      hiddenState?.windowVisible === false &&
      cleanupError === null &&
      behaviorAcceptable
        ? receipt.behavior.status
        : "error";
    receipt.failure =
      [
        receipt.behavior.failure,
        cleanupError,
        lifecycleValid ? null : "artifact lifecycle validation failed",
      ]
        .filter(Boolean)
        .join("; ") || null;
    if (receipt.status === "pass" && stagingDir) {
      try {
        removeOwnedAuxiliaryDirectory(claim, stagingDir);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        receipt.status = "error";
        receipt.failure = `staging removal failed: ${message}`;
      }
    }
    if (receipt.status !== "pass") {
      const paths = [claim.root];
      if (stagingDir && existsSync(stagingDir)) paths.push(stagingDir);
      const liveSessionDir = join(sessionRoot, session);
      if (existsSync(liveSessionDir)) paths.push(liveSessionDir);
      receipt.failurePreservation = {
        outputRootPreserved: true,
        sessionRootPreserved: existsSync(liveSessionDir),
        stagingPreserved: Boolean(stagingDir && existsSync(stagingDir)),
        paths,
        reason: receipt.failure ?? "probe failed",
      };
    }
    commitFinalReceipt(claim, receipt, specs, artifacts);
    console.log(JSON.stringify(receipt, null, 2));
    if (
      receipt.status === "error" ||
      (enforce && receipt.behavior.status !== "pass")
    ) {
      process.exitCode = 1;
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
