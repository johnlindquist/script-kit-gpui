#!/usr/bin/env bun

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver.ts";

const DEFAULT_BINARY =
  "/Users/johnlindquist/dev/script-kit-gpui/target-agent/artifacts/ratchet-layout-final/script-kit-gpui";
const DEFAULT_PI_BINARY =
  "/Users/johnlindquist/dev/script-kit-gpui/target/pi-sidecar/pi";
const binary = process.env.PROBE_BINARY ?? DEFAULT_BINARY;
const configuredPiBinary = process.env.SCRIPT_KIT_PI_BINARY;
const piBinary = configuredPiBinary ?? DEFAULT_PI_BINARY;
const expectedAgentProvider =
  process.env.PROBE_AGENT_PROVIDER ?? "openai-codex";
const expectedAgentModel = process.env.PROBE_AGENT_MODEL ?? "gpt-5.6-sol";
const scriptPath = resolve("tests/smoke/test-error-display.ts");
const receiptPath = resolve(
  ".test-output/script-error-agent-chat-handoff-probe.json",
);
const expectedError =
  "This is a test error message - the toast system should display this!";

type Classification =
  | "fixed"
  | "reproduced"
  | "not-reproduced"
  | "blocked-by-missing-primitive"
  | "blocked-by-unsafe-operation"
  | "agent-chat-runtime-unavailable"
  | "needs-user-info";

interface Check extends Json {
  name: string;
  pass: boolean;
}

const receipt: Json = {
  schemaVersion: 1,
  tool: "script-error-agent-chat-handoff-probe",
  intake: {
    userPath: "stdin run -> tests/smoke/test-error-display.ts -> ScriptError -> Agent Chat",
    scriptPath,
    fixtureSubstitutionUsed: false,
  },
  binary,
  piSidecar: {
    path: piBinary,
    source: configuredPiBinary ? "SCRIPT_KIT_PI_BINARY" : "repo-existing-sidecar",
    injectedIntoApp: false,
    expectedProvider: expectedAgentProvider,
    expectedModel: expectedAgentModel,
  },
  pass: false,
  classification: "needs-user-info" satisfies Classification,
  missingPrimitives: [],
  checks: [] as Check[],
  primitiveStack: [
    "Driver.launch(sandboxHome, seedAgentAuth)",
    "run",
    "app.log ScriptError + ScriptExit lifecycle markers",
    "post-script protocol route drain",
    "getLogs",
    "sandbox context bundle inspection",
    "getAgentChatState",
    "hide + getState cleanup",
  ],
};

mkdirSync(resolve(".test-output"), { recursive: true });

let driver: Driver | undefined;
let launchAttempted = false;
let runReachedScriptError = false;

function isExecutableFile(path: string): boolean {
  try {
    const stats = statSync(path);
    return stats.isFile() && (stats.mode & 0o111) !== 0;
  } catch {
    return false;
  }
}

function check(name: string, pass: boolean, detail: unknown): void {
  (receipt.checks as Check[]).push({ name, pass, detail });
}

function statePayload(response: Json): Json {
  const nested = response.state;
  return nested && typeof nested === "object" ? (nested as Json) : response;
}

function logEntries(response: Json): Json[] {
  return Array.isArray(response.entries) ? (response.entries as Json[]) : [];
}

function hasMessage(entries: Json[], needle: string): boolean {
  return entries.some((entry) => String(entry.message ?? "").includes(needle));
}

function compactState(state: Json): Json {
  return {
    promptType: state.promptType ?? null,
    windowVisible: state.windowVisible ?? null,
  };
}

function relevantAppLogLines(source: string): string[] {
  const needles = [
    "Captured stderr from buffer",
    "Script error received",
    "Toast created for script error",
    "pi_rpc_stdout_closed",
    "warm_prepare_slot_runtime_failed",
    "pi_agent_chat_warm_failed_setup",
    "pi_agent_chat_unavailable",
    "script_error_agent_chat_stage_failed",
    "=== ScriptExit message received ===",
    "Failed to write to script stdin",
    "Writer thread exiting",
  ];
  return source
    .split("\n")
    .filter((line) => needles.some((needle) => line.includes(needle)));
}

function diagnosePiSidecar(sandboxHome: string): Json {
  if (!isExecutableFile(piBinary)) {
    return {
      attempted: false,
      reason: "configured Pi sidecar is not an executable file",
    };
  }

  const command = [
    piBinary,
    "--mode",
    "rpc",
    "--provider",
    expectedAgentProvider,
    "--model",
    expectedAgentModel,
  ];
  const process = Bun.spawnSync(command, {
    cwd: resolve("."),
    env: { ...processEnv(), HOME: sandboxHome },
    stdin: new Uint8Array(),
    stdout: "pipe",
    stderr: "pipe",
  });
  return {
    attempted: true,
    command,
    exitCode: process.exitCode,
    stdout: process.stdout.toString().trim(),
    stderr: process.stderr.toString().trim(),
  };
}

function processEnv(): Record<string, string> {
  return Object.fromEntries(
    Object.entries(process.env).filter(
      (entry): entry is [string, string] => typeof entry[1] === "string",
    ),
  );
}

async function waitUntil<T>(
  label: string,
  probe: () => Promise<T>,
  predicate: (value: T) => boolean,
  timeoutMs: number,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let last: T | undefined;
  while (Date.now() < deadline) {
    last = await probe();
    if (predicate(last)) return last;
    await Bun.sleep(100);
  }
  throw new Error(
    `timeout waiting for ${label}; last=${JSON.stringify(last ?? null)}`,
  );
}

function inspectContextBundle(sandboxHome: string): Json {
  const root = join(
    sandboxHome,
    ".scriptkit",
    "agent_chat",
    "script-error-context",
  );
  if (!existsSync(root)) return { root, exists: false, bundleDirectories: [] };

  const bundleDirectories = readdirSync(root)
    .map((name) => join(root, name))
    .filter((path) => statSync(path).isDirectory());
  const files = bundleDirectories.flatMap((directory) =>
    readdirSync(directory).map((name) => join(directory, name)),
  );
  const snapshotPath = files.find(
    (path) => basename(path) === basename(scriptPath),
  );
  const reportPath = files.find((path) => path.endsWith("-error-report.md"));

  return {
    root,
    exists: true,
    bundleDirectories,
    files,
    snapshotPath: snapshotPath ?? null,
    reportPath: reportPath ?? null,
    snapshot: snapshotPath ? readFileSync(snapshotPath, "utf8") : null,
    report: reportPath ? readFileSync(reportPath, "utf8") : null,
  };
}

function addMissingPrimitive(name: string): void {
  const missing = receipt.missingPrimitives as string[];
  if (!missing.includes(name)) missing.push(name);
}

function recordStructuredLogEvidence(response: Json): void {
  const entries = logEntries(response);
  const relevantEntries = entries.filter((entry) =>
    [
      "Executing script:",
      "Captured stderr from buffer",
      "Script error received",
      "Script stderr output",
      "Toast created for script error",
    ].some((needle) => String(entry.message ?? "").includes(needle)),
  );
  receipt.structuredLogs = {
    matched: response.matched ?? null,
    capacity: response.capacity ?? null,
    relevantEntries,
  };

  const logSchemaFields = [
    "timestamp",
    "level",
    "target",
    "correlation_id",
    "message",
  ];
  const schemaValid =
    relevantEntries.length > 0 &&
    relevantEntries.every((entry) =>
      logSchemaFields.every((field) => typeof entry[field] === "string"),
    );
  check("structured_log_schema", schemaValid, {
    fields: logSchemaFields,
    entryCount: relevantEntries.length,
  });
  check("script_execution_logged", hasMessage(entries, "Executing script:"), {
    message: "Executing script:",
  });
  check(
    "stderr_capture_logged",
    hasMessage(entries, "Captured stderr from buffer"),
    { message: "Captured stderr from buffer" },
  );
  check("script_error_logged", hasMessage(entries, "Script error received"), {
    message: "Script error received",
  });
  check(
    "script_error_toast_logged",
    hasMessage(entries, "Toast created for script error"),
    { message: "Toast created for script error" },
  );
}

function recordContextBundle(bundle: Json): void {
  const source = readFileSync(scriptPath, "utf8");
  const report = String(bundle.report ?? "");
  receipt.contextBundle = {
    root: bundle.root,
    bundleDirectories: bundle.bundleDirectories,
    files: bundle.files,
    snapshotPath: bundle.snapshotPath,
    reportPath: bundle.reportPath,
  };
  check(
    "single_persisted_context_bundle",
    (bundle.bundleDirectories as string[]).length === 1,
    { bundleDirectories: bundle.bundleDirectories },
  );
  check("persisted_script_snapshot_matches", bundle.snapshot === source, {
    snapshotPath: bundle.snapshotPath,
    expectedBytes: Buffer.byteLength(source),
    actualBytes: Buffer.byteLength(String(bundle.snapshot ?? "")),
  });
  check(
    "persisted_error_report_complete",
    report.includes("# Script Failure Report") &&
      report.includes(scriptPath) &&
      report.includes(expectedError) &&
      report.includes("## Exit Code") &&
      report.includes("## Stderr") &&
      report.includes("## Stack Trace"),
    {
      reportPath: bundle.reportPath,
      hasTitle: report.includes("# Script Failure Report"),
      hasScriptPath: report.includes(scriptPath),
      hasExpectedError: report.includes(expectedError),
      hasExitCode: report.includes("## Exit Code"),
      hasStderr: report.includes("## Stderr"),
      hasStackTrace: report.includes("## Stack Trace"),
    },
  );
}

function recordAgentChatRuntimeUnavailable(appLog: string): void {
  const sidecarDiagnostic = diagnosePiSidecar(String(receipt.sandboxHome));
  receipt.agentChat = {
    status: "setup",
    source: "app-log",
    setupTitle: "Pi Agent Chat is unavailable",
  };
  receipt.agentChatRuntime = {
    available: false,
    status: "setup",
    appEvents: relevantAppLogLines(appLog).filter((line) =>
      [
        "pi_rpc_stdout_closed",
        "warm_prepare_slot_runtime_failed",
        "pi_agent_chat_warm_failed_setup",
        "pi_agent_chat_unavailable",
        "script_error_agent_chat_stage_failed",
      ].some((needle) => line.includes(needle)),
    ),
    sidecarDiagnostic,
  };
  check("agent_chat_runtime_ready", false, receipt.agentChatRuntime);
  receipt.classification = "agent-chat-runtime-unavailable";
  receipt.blocker = {
    code: "agent-chat-runtime-unavailable",
    missingPrimitive: null,
    detail:
      sidecarDiagnostic.stderr ||
      "The injected Pi sidecar exited before RPC model readiness; see appEvents.",
  };
}

try {
  if (!isExecutableFile(binary)) {
    throw new Error(`PROBE_BINARY is not executable or does not exist: ${binary}`);
  }

  const injectedPiBinary = isExecutableFile(piBinary) ? piBinary : undefined;
  (receipt.piSidecar as Json).injectedIntoApp = Boolean(injectedPiBinary);

  launchAttempted = true;
  driver = await Driver.launch({
    binary,
    sessionName: "script-error-agent-chat-handoff-probe",
    sandboxHome: true,
    seedAgentAuth: true,
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 15_000,
    env: {
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
      ...(injectedPiBinary
        ? { SCRIPT_KIT_PI_BINARY: injectedPiBinary }
        : {}),
    },
  });
  receipt.sessionDir = driver.sessionDir;
  receipt.sandboxHome = join(driver.sessionDir, "home");

  const initialSettle = await driver.waitForSettle({ timeoutMs: 10_000 });
  const initialState = statePayload(initialSettle.lastState ?? {});
  receipt.initialSettle = {
    settled: initialSettle.settled,
    elapsedMs: initialSettle.elapsedMs,
    probes: initialSettle.probes,
    lastState: compactState(initialState),
  };
  check("initial_state_settled", initialSettle.settled, receipt.initialSettle);

  driver.send({
    type: "run",
    path: scriptPath,
    requestId: "plan04-script-error-run",
  });

  const appLog = await waitUntil(
    "file-log ScriptError and ScriptExit markers",
    async () =>
      existsSync(driver!.logPath) ? readFileSync(driver!.logPath, "utf8") : "",
    (source) =>
      source.includes("Script error received") &&
      source.includes("=== ScriptExit message received ==="),
    30_000,
  );
  runReachedScriptError = true;
  receipt.fileLogEvidence = {
    path: driver.logPath,
    lifecycleComplete: true,
    relevantLines: relevantAppLogLines(appLog),
  };

  const bundle = await waitUntil(
    "persisted sandbox script-error context bundle",
    async () => inspectContextBundle(String(receipt.sandboxHome)),
    (value) =>
      value.exists === true &&
      typeof value.snapshot === "string" &&
      typeof value.report === "string",
    10_000,
  );
  recordContextBundle(bundle);

  if (
    appLog.includes("pi_agent_chat_unavailable") ||
    appLog.includes("script_error_agent_chat_stage_failed")
  ) {
    recordAgentChatRuntimeUnavailable(appLog);
    throw new Error(
      "Agent Chat runtime was unavailable after the real ScriptError handoff; see agentChatRuntime",
    );
  }

  // On this pinned artifact the completed script can leave its writer route
  // installed until the next protocol message observes the closed pipe. Drain
  // that stale route with a fire-and-forget state probe before making any
  // request whose response is part of the receipt. Newer binaries that route
  // immediately answer the drain request instead, which is also acceptable.
  const drainRequestId = "plan04-post-script-writer-drain";
  const writerExitCount = appLog.split("Writer thread exiting").length - 1;
  driver.send({ type: "getState", requestId: drainRequestId });
  const drainLog = await waitUntil(
    "post-script protocol route drain",
    async () => readFileSync(driver!.logPath, "utf8"),
    (source) =>
      source.split("Writer thread exiting").length - 1 > writerExitCount ||
      source.includes(`\"requestId\":\"${drainRequestId}\"`),
    5_000,
  );
  receipt.postScriptProtocolDrain = {
    staleWriterClosed:
      drainLog.split("Writer thread exiting").length - 1 > writerExitCount,
    drainRequestAnswered: drainLog.includes(
      `\"requestId\":\"${drainRequestId}\"`,
    ),
  };

  const scriptErrorLogs = await waitUntil(
    "structured ScriptError log",
    () => driver!.getLogs({ limit: 500 }),
    (logs) => hasMessage(logEntries(logs), "Script error received"),
    10_000,
  );
  recordStructuredLogEvidence(scriptErrorLogs);

  const agentChatResponse = await waitUntil(
    "Agent Chat script-error handoff",
    () =>
      driver!.request(
        { type: "getAgentChatState" },
        { expect: "agentChatStateResult", timeoutMs: 15_000 },
      ),
    (response) => String(statePayload(response).status ?? "") !== "notAgentChat",
    30_000,
  );
  const agentChat = statePayload(agentChatResponse);
  receipt.agentChat = agentChatResponse;

  if (agentChat.status === "setup") {
    const latestAppLog = readFileSync(driver.logPath, "utf8");
    recordAgentChatRuntimeUnavailable(latestAppLog);
    throw new Error(
      `Agent Chat setup blocked the script-error handoff: ${JSON.stringify(agentChat.setup ?? null)}`,
    );
  }

  receipt.agentChatRuntime = {
    available: true,
    status: agentChat.status,
  };
  check("agent_chat_runtime_ready", true, receipt.agentChatRuntime);

  for (const [field, primitive] of [
    ["messageCount", "getAgentChatState.messageCount"],
    ["contextChipCount", "getAgentChatState.contextChipCount"],
    ["contextSummary", "getAgentChatState.contextSummary"],
  ] as const) {
    if (!(field in agentChat)) addMissingPrimitive(primitive);
  }

  const contextSummary = String(agentChat.contextSummary ?? "");
  check("agent_chat_live_surface", agentChat.status !== "notAgentChat", {
    status: agentChat.status,
    resolvedTarget: agentChat.resolvedTarget ?? null,
  });
  check("handoff_submitted_user_message", Number(agentChat.messageCount) >= 1, {
    messageCount: agentChat.messageCount,
    status: agentChat.status,
    inputTextCleared: agentChat.inputText === "",
  });
  check("handoff_has_two_context_parts", Number(agentChat.contextChipCount) === 2, {
    contextChipCount: agentChat.contextChipCount,
    contextSummary,
  });
  check(
    "handoff_context_labels",
    contextSummary.includes("test-error-display.ts") &&
      contextSummary.includes("test-error-display-error-report.md"),
    { contextSummary },
  );

  const failedChecks = (receipt.checks as Check[])
    .filter((item) => !item.pass)
    .map((item) => item.name);
  receipt.failedChecks = failedChecks;
  receipt.pass =
    failedChecks.length === 0 &&
    (receipt.missingPrimitives as string[]).length === 0;
  receipt.classification = receipt.pass
    ? "fixed"
    : (receipt.missingPrimitives as string[]).length > 0
      ? "blocked-by-missing-primitive"
      : "reproduced";
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  receipt.error = message;
  if (receipt.classification === "agent-chat-runtime-unavailable") {
    // The setup branch above already captured exact app events and a
    // same-binary sidecar diagnostic. Keep that fail-closed classification.
  } else if (/did not become ready|Timeout .*getState/i.test(message)) {
    receipt.classification = "blocked-by-unsafe-operation";
    receipt.blocker = {
      code: "blocked-by-sandbox",
      missingPrimitive: null,
      detail:
        "Driver launch never reached readiness and its fallback getState timed out; run the same probe outside the seatbelt sandbox.",
    };
  } else if (/unknown command.*getLogs|unsupported.*getLogs/i.test(message)) {
    receipt.classification = "blocked-by-missing-primitive";
    addMissingPrimitive(
      "getLogs structured entries with timestamp, level, target, correlation_id, and message",
    );
  } else if (
    /unknown command.*getAgentChatState|unsupported.*getAgentChatState/i.test(
      message,
    )
  ) {
    receipt.classification = "blocked-by-missing-primitive";
    addMissingPrimitive(
      "getAgentChatState with messageCount, contextChipCount, and contextSummary",
    );
  } else if (/setup|auth|credential|Pi Agent Chat is unavailable/i.test(message)) {
    receipt.classification = "needs-user-info";
  } else if (runReachedScriptError) {
    receipt.classification = "reproduced";
  } else {
    receipt.classification = "not-reproduced";
  }
} finally {
  if (driver) {
    try {
      driver.simulateKey("escape");
      driver.send({ type: "hide" });
      const hideWait = await driver.waitForState(
        { windowVisible: false },
        { timeoutMs: 5_000 },
      );
      if (!("structuredLogs" in receipt)) {
        try {
          const postHideLogs = await driver.getLogs(
            { limit: 500 },
            { timeoutMs: 5_000 },
          );
          recordStructuredLogEvidence(postHideLogs);
        } catch (error) {
          const detail = error instanceof Error ? error.message : String(error);
          receipt.postHideStructuredLogError = detail;
          check("structured_log_query_after_cleanup", false, { detail });
        }
      }
      const finalState = await driver.getState({ timeoutMs: 5_000 });
      const hideVerified = finalState.windowVisible === false;
      receipt.cleanup = {
        hideVerified,
        hideWait: compactState(statePayload(hideWait)),
        finalState: compactState(finalState),
      };
      check("app_hidden_after_probe", hideVerified, {
        windowVisible: finalState.windowVisible ?? null,
      });
      if (!hideVerified) {
        receipt.pass = false;
        if (receipt.classification === "fixed") receipt.classification = "reproduced";
      }
    } catch (error) {
      receipt.cleanup = {
        hideVerified: false,
        error: error instanceof Error ? error.message : String(error),
      };
      receipt.pass = false;
      if (receipt.classification === "fixed") receipt.classification = "reproduced";
    }
    await driver.close();
  } else {
    receipt.cleanup = {
      hideVerified: false,
      launchAttempted,
      driverHandleAcquired: false,
      driverLaunchOwnsFailureCleanup: true,
    };
  }

  receipt.failedChecks = (receipt.checks as Check[])
    .filter((item) => !item.pass)
    .map((item) => item.name);
  if ((receipt.failedChecks as string[]).length > 0) receipt.pass = false;
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify(receipt, null, 2));
}

process.exit(receipt.pass ? 0 : 1);
