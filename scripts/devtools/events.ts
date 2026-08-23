#!/usr/bin/env bun
import { outputSummary, summarizeText, tryParseJson as summarizeJsonText } from "./lib/receipt-output";
import {
  assertNoninteractiveSessionCommand,
  requireSuccessfulSessionAction,
} from "./lib/operator-safety.ts";

type JsonObject = Record<string, unknown>;

type Command = "tail" | "record" | "logs" | "crashes";

type Args = {
  command: Command;
  session: string;
  limit: number;
  contains: string;
  includeMcpAudit: boolean;
  start: boolean;
  show: boolean;
  timeoutMs: number;
  childCommand: string[];
  outputPath: string;
  previewBytes: number;
  inlineFullOutput: boolean;
  externalSession: string;
  externalOutputLog: string;
  wrapperTimeoutMs: number;
  file: string;
  since: string;
  marker: string;
  level: string;
  target: string;
  cid: string;
  allSessions: boolean;
};

const actionOutputFields = [
  "stdoutSummary",
  "stderrSummary",
  "omittedBytes",
  "fingerprint",
  "stdoutJson",
  "artifactPath",
];

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/events.ts tail [--session <name>] [--limit <n>] [--contains <text>] [--include-mcp-audit]",
    "  bun scripts/devtools/events.ts record [--session <name>] [--start] [--show] [--output <path>] [--preview-bytes <n>] [--inline-full-output] [--include-mcp-audit] [--external-session <slug>] [--external-output-log <path>] [--wrapper-timeout-ms <n>] -- <devtools command...>",
    "  bun scripts/devtools/events.ts logs [--file <jsonl>] [--all-sessions] [--since <rfc3339|HH:MM:SS[.mmm]>] [--marker <text>] [--level <trace|debug|info|warn|error>] [--target <substr>] [--cid <correlation-id>] [--contains <text>] [--limit <n>]",
    "  bun scripts/devtools/events.ts crashes [--limit <n>]",
    "",
    "logs    queries the structured JSONL sinks (default ~/.scriptkit/logs/latest-session.jsonl,",
    "        --all-sessions for the append-forever script-kit-gpui.jsonl). --since accepts the",
    "        rfc3339 timestamps used in the JSONL or a bare UTC clock time matching the compact",
    "        stderr format; --level is a minimum severity; --marker keeps only entries after the",
    "        last line containing the text (pairs with dev_marker / session_start markers).",
    "crashes surfaces the newest macOS DiagnosticReports .ips files for script-kit-gpui with the",
    "        exception type, termination reason, and faulting-thread frames, so a dead app is",
    "        diagnosable in one call (the dev.sh crash watchdog is opt-in).",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  const command = argv[0] as Command | undefined;
  if (!command || !["tail", "record", "logs", "crashes"].includes(command)) {
    console.error(usage());
    process.exit(2);
  }
  const separator = argv.indexOf("--");
  const options = separator >= 0 ? argv.slice(1, separator) : argv.slice(1);
  const args: Args = {
    command,
    session: "default",
    limit: 80,
    contains: "",
    includeMcpAudit: false,
    start: false,
    show: false,
    timeoutMs: 8000,
    childCommand: separator >= 0 ? argv.slice(separator + 1) : [],
    outputPath: "",
    previewBytes: 2048,
    inlineFullOutput: false,
    externalSession: "",
    externalOutputLog: "",
    wrapperTimeoutMs: 0,
    file: "",
    since: "",
    marker: "",
    level: "",
    target: "",
    cid: "",
    allSessions: false,
  };

  for (let index = 0; index < options.length; index += 1) {
    const arg = options[index];
    if (arg === "--session") {
      args.session = options[++index] ?? args.session;
    } else if (arg === "--limit") {
      args.limit = Number(options[++index] ?? args.limit);
    } else if (arg === "--contains") {
      args.contains = options[++index] ?? "";
    } else if (arg === "--include-mcp-audit") {
      args.includeMcpAudit = true;
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--show") {
      args.show = true;
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(options[++index] ?? args.timeoutMs);
    } else if (arg === "--output") {
      args.outputPath = options[++index] ?? "";
    } else if (arg === "--preview-bytes") {
      args.previewBytes = Number(options[++index] ?? args.previewBytes);
    } else if (arg === "--inline-full-output") {
      args.inlineFullOutput = true;
    } else if (arg === "--external-session") {
      args.externalSession = options[++index] ?? "";
      if (!args.wrapperTimeoutMs) args.wrapperTimeoutMs = 120000;
    } else if (arg === "--external-output-log") {
      args.externalOutputLog = options[++index] ?? "";
    } else if (arg === "--wrapper-timeout-ms") {
      args.wrapperTimeoutMs = Number(options[++index] ?? 0);
    } else if (arg === "--file") {
      args.file = options[++index] ?? "";
    } else if (arg === "--since") {
      args.since = options[++index] ?? "";
    } else if (arg === "--marker") {
      args.marker = options[++index] ?? "";
    } else if (arg === "--level") {
      args.level = (options[++index] ?? "").toLowerCase();
    } else if (arg === "--target") {
      args.target = options[++index] ?? "";
    } else if (arg === "--cid") {
      args.cid = options[++index] ?? "";
    } else if (arg === "--all-sessions") {
      args.allSessions = true;
    }
  }

  if (command === "record" && args.childCommand.length === 0) {
    console.error(usage());
    process.exit(2);
  }

  return args;
}

async function runCommand(command: string[], label: string): Promise<{ receipt: JsonObject; stdout: string; stderr: string; exitCode: number }> {
  assertNoninteractiveSessionCommand(command);
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (exitCode !== 0) {
    return {
      receipt: { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim() },
      stdout,
      stderr,
      exitCode,
    };
  }
  try {
    return { receipt: JSON.parse(stdout), stdout, stderr, exitCode };
  } catch {
    return {
      receipt: { status: "ok", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim() },
      stdout,
      stderr,
      exitCode,
    };
  }
}

async function run(command: string[], label: string): Promise<JsonObject> {
  return (await runCommand(command, label)).receipt;
}

async function fileSize(path: string) {
  try {
    return (await Bun.file(path).stat()).size;
  } catch {
    return 0;
  }
}

async function readLines(path: string, offset: number, limit: number, contains = "") {
  try {
    const text = (await Bun.file(path).text()).slice(offset);
    const lines = text.split(/\r?\n/).filter(Boolean);
    return lines
      .filter((line) => !contains || line.includes(contains))
      .slice(-limit)
      .map((line, index) => parseLine(line, index));
  } catch {
    return [];
  }
}

function defaultMcpAuditPath() {
  const home = Bun.env.HOME || "";
  return home ? `${home}/.scriptkit/logs/mcp-audit.jsonl` : "";
}

function parseLine(line: string, index: number) {
  const parsed = tryJson(line);
  const parsedFields = (parsed?.fields && typeof parsed.fields === "object" ? parsed.fields : {}) as JsonObject;
  const eventType = stringValue(parsedFields.event_type)
    ?? stringValue(parsed?.event_type)
    ?? /event_type=([^ ]+)/.exec(line)?.[1]
    ?? /\bevent=([^ ]+)/.exec(line)?.[1]
    ?? (typeof parsed?.method === "string" ? "mcp_audit" : null);
  const correlationId = stringValue(parsed?.correlation_id)
    ?? stringValue(parsedFields.correlation_id)
    ?? /cid=([^ ]+)/.exec(line)?.[1]
    ?? null;
  const commandType = stringValue(parsed?.method)
    ?? stringValue(parsedFields.command_type)
    ?? stringValue(parsed?.command_type)
    ?? /command_type=([^ ]+)/.exec(line)?.[1]
    ?? null;
  // Compact format is HH:MM:SS.mmm|L|C|... ; older logs may still carry the
  // pre-2026-07 SS.mmm prefix, so accept both.
  const compactLevel = /^(?:\d{2}:\d{2}:)?\d{2}\.\d{3}\|([iwedt])\|/.exec(line)?.[1] ?? null;
  const level = stringValue(parsed?.level)?.toLowerCase()
    ?? (parsed?.success === false ? "error" : null)
    ?? (parsed?.success === true ? "info" : null)
    ?? /\b(INFO|WARN|ERROR|DEBUG|TRACE)\b/.exec(line)?.[1]?.toLowerCase()
    ?? compactLevelName(compactLevel);
  return {
    index,
    kind: parsed ? "json" : "log",
    eventType,
    correlationId,
    commandType,
    level,
    line,
    parsed,
  };
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function compactLevelName(level: string | null) {
  if (level === "i") return "info";
  if (level === "w") return "warn";
  if (level === "e") return "error";
  if (level === "d") return "debug";
  if (level === "t") return "trace";
  return null;
}

function tryJson(line: string) {
  try {
    return JSON.parse(line) as JsonObject;
  } catch {
    return null;
  }
}

function countEvents(lines: ReturnType<typeof parseLine>[]) {
  const byType: Record<string, number> = {};
  const warnings: string[] = [];
  for (const entry of lines) {
    const key = entry.eventType ?? entry.commandType ?? entry.level ?? entry.kind;
    byType[key] = (byType[key] ?? 0) + 1;
    if (entry.level === "warn" || entry.level === "error" || entry.line.includes("WARN") || entry.line.includes("ERROR")) {
      warnings.push(entry.line);
    }
  }
  return { byType, warningCount: warnings.length, warnings: warnings.slice(-10) };
}

function lineText(lines: Array<{ line?: string }>) {
  return lines.map((entry) => String(entry.line ?? "")).join("\n");
}

function classifyFailureSignals(child: JsonObject | null, appEvents: ReturnType<typeof parseLine>[], responseEvents: ReturnType<typeof parseLine>[]) {
  const text = [
    JSON.stringify(child ?? {}),
    lineText(appEvents),
    lineText(responseEvents),
  ].join("\n");
  const openAiSeen = /command_type=openAi|"type":"openAi"|openAi/.test(text);
  const agent_chatNotAgentChat = /notAgentChat|not_agent_chat|status.?[:=].?notAgentChat/i.test(text);
  const piWarmSeen = /pi warm|warm pi|prepare.*pi|acquire.*pi|Pi Text|openAi/i.test(text);
  const responseTimeout = /response_timeout|timeout|parseOutcome.?[:=].?timeout/i.test(text);
  return {
    openAi: {
      seen: openAiSeen,
      parsedEventSeen: /event_type=stdin_command_parsed.*command_type=openAi/.test(text),
    },
    agent_chat: {
      statusNotAgentChatSeen: agent_chatNotAgentChat,
      nextReceipt: "getAgentChatState(target ai or focused AgentChat target)",
    },
    piWarm: {
      signalsSeen: piWarmSeen,
      nextReceipt: "events.record around openAi plus getAgentChatState after transition",
    },
    timeout: {
      responseTimeoutSeen: responseTimeout,
    },
    likelyOwners: [
      agent_chatNotAgentChat ? "agent_chat-chat-core" : "",
      piWarmSeen ? "sdk-script-execution" : "",
      openAiSeen ? "protocol-automation" : "",
    ].filter(Boolean),
    recommendedNext: [
      openAiSeen && agent_chatNotAgentChat ? "Prove target identity first: targets.inspect --target-kind agentChatDetached|main and getAgentChatState." : "",
      piWarmSeen ? "Separate warm Pi acquire failure from Agent Chat routing failure in the receipt." : "",
      responseTimeout ? "Use compact output artifact path; do not rely on transcript tail." : "",
    ].filter(Boolean),
  };
}

function compactActionReceipt(receipt: JsonObject | null, inlineFullOutput: boolean) {
  if (!receipt || inlineFullOutput) return receipt;
  const safety = (receipt.safety as JsonObject | undefined) ?? {};
  const submitGate = (receipt.submitGate as JsonObject | undefined) ?? {};
  const targetBefore = (receipt.targetBefore as JsonObject | undefined) ?? null;
  const targetAfter = (receipt.targetAfter as JsonObject | undefined) ?? null;
  return {
    schemaVersion: receipt.schemaVersion ?? null,
    tool: receipt.tool ?? null,
    command: receipt.command ?? null,
    classification: receipt.classification ?? null,
    session: receipt.session ?? null,
    proofIntent: receipt.proofIntent ?? null,
    preflightOnly: receipt.preflightOnly ?? null,
    actionKind: receipt.actionKind ?? null,
    blockedAction: receipt.blockedAction ?? null,
    submitGate: {
      gateName: submitGate.gateName ?? null,
      key: submitGate.key ?? null,
      modifiers: submitGate.modifiers ?? [],
      allowSubmit: submitGate.allowSubmit ?? null,
      allowSubmitReason: submitGate.allowSubmitReason ?? null,
      submitIntent: submitGate.submitIntent ?? null,
      selectedSemanticId: submitGate.selectedSemanticId ?? null,
      selectedActionId: submitGate.selectedActionId ?? null,
      target: submitGate.target ?? null,
    },
    safety: {
      channel: safety.channel ?? null,
      destructive: safety.destructive ?? null,
      submitAllowed: safety.submitAllowed ?? null,
      submitIntent: safety.submitIntent ?? null,
      allowSubmitReason: safety.allowSubmitReason ?? null,
      nativeEscalation: safety.nativeEscalation ?? null,
      submitAttempted: safety.submitAttempted ?? null,
      errors: safety.errors ?? [],
      warnings: safety.warnings ?? [],
    },
    submitLifecycle: receipt.submitLifecycle ?? null,
    postActionLifecycle: receipt.postActionLifecycle ?? null,
    actionReceipt: receipt.actionReceipt ?? null,
    targetBefore: targetBefore
      ? {
        automationId: targetBefore.automationId ?? null,
        targetKind: targetBefore.targetKind ?? null,
        surfaceKind: targetBefore.surfaceKind ?? null,
        appViewVariant: targetBefore.appViewVariant ?? null,
        nativeFooterSurface: targetBefore.nativeFooterSurface ?? null,
        strictTargetMatch: targetBefore.strictTargetMatch ?? null,
      }
      : null,
    targetAfter: targetAfter
      ? {
        automationId: targetAfter.automationId ?? null,
        targetKind: targetAfter.targetKind ?? null,
        surfaceKind: targetAfter.surfaceKind ?? null,
        appViewVariant: targetAfter.appViewVariant ?? null,
        nativeFooterSurface: targetAfter.nativeFooterSurface ?? null,
        strictTargetMatch: targetAfter.strictTargetMatch ?? null,
      }
      : null,
    rawActionReceiptOmitted: true,
  };
}

function parsedSummary(parsed: JsonObject | null) {
  if (!parsed) return null;
  return {
    type: parsed.type ?? null,
    requestId: parsed.requestId ?? null,
    tool: parsed.tool ?? null,
    command: parsed.command ?? null,
    classification: parsed.classification ?? null,
    status: parsed.status ?? null,
    json: summarizeJsonText(JSON.stringify(parsed)),
  };
}

function compactEventEntries(entries: ReturnType<typeof parseLine>[], previewBytes: number, inlineFullOutput: boolean) {
  if (inlineFullOutput) return entries;
  return entries.map((entry) => ({
    index: entry.index,
    kind: entry.kind,
    eventType: entry.eventType,
    correlationId: entry.correlationId,
    commandType: entry.commandType,
    level: entry.level,
    line: summarizeText(entry.line, previewBytes).preview,
    lineSummary: summarizeText(entry.line, previewBytes),
    parsed: parsedSummary(entry.parsed),
  }));
}

async function sessionStatus(args: Args) {
  if (args.start) {
    const started = await run(
      ["bash", "scripts/agentic/session.sh", "start", args.session],
      "session-start",
    );
    requireSuccessfulSessionAction(args.session, "start", started);
  }
  if (args.show) {
    const shown = await run(
      ["bash", "scripts/agentic/session.sh", "send", args.session, JSON.stringify({ type: "show" }), "--await-parse", "--timeout", String(args.timeoutMs)],
      "session-show",
    );
    requireSuccessfulSessionAction(args.session, "show", shown);
  }
  return run(["bash", "scripts/agentic/session.sh", "status", args.session], "session-status");
}

function classify(status: JsonObject, child: JsonObject | null, appLines: unknown[], responseLines: unknown[]) {
  if (child?.status === "error" && String((child.error as JsonObject | undefined)?.code ?? "").includes("external_wrapper_timeout")) {
    return "blocked-by-external-wrapper-timeout";
  }
  if (status.status !== "ok" || status.healthy === false) {
    return "blocked-by-target-ambiguity";
  }
  if (child?.status === "error") {
    return "blocked-by-timeout";
  }
  if (appLines.length === 0 && responseLines.length === 0) {
    return "blocked-by-missing-primitive";
  }
  return "ok";
}

// --- logs: query the structured JSONL sinks ---------------------------------

const LEVEL_RANK: Record<string, number> = { trace: 0, debug: 1, info: 2, warn: 3, error: 4 };

/** Accepts the JSONL rfc3339 timestamps or a bare UTC clock time (compact stderr format). */
function parseSince(since: string): number | null {
  const clock = /^(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,3}))?$/.exec(since);
  if (clock) {
    const now = new Date();
    const candidate = Date.UTC(
      now.getUTCFullYear(),
      now.getUTCMonth(),
      now.getUTCDate(),
      Number(clock[1]),
      Number(clock[2]),
      Number(clock[3]),
      Number((clock[4] ?? "0").padEnd(3, "0")),
    );
    // A clock time later than "now" means the span started yesterday.
    return candidate > now.getTime() ? candidate - 24 * 3600 * 1000 : candidate;
  }
  const parsed = Date.parse(since);
  return Number.isNaN(parsed) ? null : parsed;
}

function defaultJsonlPath(allSessions: boolean) {
  const home = Bun.env.HOME || "";
  if (!home) return "";
  return allSessions
    ? `${home}/.scriptkit/logs/script-kit-gpui.jsonl`
    : `${home}/.scriptkit/logs/latest-session.jsonl`;
}

async function cmdLogs(args: Args) {
  const path = args.file || defaultJsonlPath(args.allSessions);
  const warnings: string[] = [];
  const sinceMs = args.since ? parseSince(args.since) : null;
  if (args.since && sinceMs === null) {
    warnings.push(`--since '${args.since}' is not rfc3339 or HH:MM:SS[.mmm]; ignoring it`);
  }
  const minLevel = args.level ? LEVEL_RANK[args.level] : null;
  if (args.level && minLevel === undefined) {
    warnings.push(`--level '${args.level}' is not one of trace|debug|info|warn|error; ignoring it`);
  }

  let rawLines: string[] = [];
  try {
    rawLines = (await Bun.file(path).text()).split(/\r?\n/).filter(Boolean);
  } catch {
    warnings.push(`could not read ${path}`);
  }

  if (args.marker) {
    const markerIndex = rawLines.findLastIndex((line) => line.includes(args.marker));
    if (markerIndex >= 0) {
      rawLines = rawLines.slice(markerIndex + 1);
    } else {
      warnings.push(`marker '${args.marker}' not found; returning the unmarked span`);
    }
  }

  const entries: JsonObject[] = [];
  let unparsedLines = 0;
  for (const line of rawLines) {
    const parsed = tryJson(line);
    if (!parsed) {
      unparsedLines += 1;
      continue;
    }
    if (sinceMs !== null && typeof parsed.timestamp === "string") {
      const ts = Date.parse(parsed.timestamp);
      if (!Number.isNaN(ts) && ts < sinceMs) continue;
    }
    if (minLevel !== null && minLevel !== undefined) {
      const rank = LEVEL_RANK[String(parsed.level ?? "").toLowerCase()];
      if (rank !== undefined && rank < minLevel) continue;
    }
    if (args.target && !String(parsed.target ?? "").includes(args.target)) continue;
    if (args.cid && String(parsed.correlation_id ?? "") !== args.cid) continue;
    if (args.contains && !line.includes(args.contains)) continue;
    entries.push(parsed);
  }
  const matched = entries.length;
  const kept = entries.slice(-args.limit);
  if (matched > kept.length) {
    warnings.push(`matched ${matched} entries; returning the last ${kept.length} (raise --limit for more)`);
  }
  if (unparsedLines > 0) {
    warnings.push(`${unparsedLines} non-JSON lines skipped`);
  }

  console.log(JSON.stringify({
    schemaVersion: 1,
    tool: "script-kit-devtools.events",
    command: "events.logs",
    classification: kept.length > 0 ? "ok" : "blocked-by-missing-primitive",
    source: {
      path,
      allSessions: args.allSessions,
      since: args.since || null,
      sinceMs,
      marker: args.marker || null,
      level: args.level || null,
      target: args.target || null,
      cid: args.cid || null,
      contains: args.contains || null,
      limit: args.limit,
    },
    totalLines: rawLines.length,
    matched,
    entries: kept,
    warnings,
  }, null, 2));
}

// --- crashes: surface the newest DiagnosticReports .ips ---------------------

type IpsFrame = { image: string; symbol: string; offset: number | null };

function parseIpsReport(text: string): JsonObject {
  const newline = text.indexOf("\n");
  const header = tryJson(newline >= 0 ? text.slice(0, newline) : text) ?? {};
  const body = newline >= 0 ? tryJson(text.slice(newline + 1)) : null;
  if (!body) {
    return { header, parseError: "could not parse .ips body JSON" };
  }
  const images = Array.isArray(body.usedImages) ? (body.usedImages as JsonObject[]) : [];
  const faultingIndex = typeof body.faultingThread === "number" ? body.faultingThread : 0;
  const threads = Array.isArray(body.threads) ? (body.threads as JsonObject[]) : [];
  const faulting = threads[faultingIndex] ?? null;
  const frames: IpsFrame[] = [];
  if (faulting && Array.isArray(faulting.frames)) {
    for (const rawFrame of (faulting.frames as JsonObject[]).slice(0, 8)) {
      const imageIndex = typeof rawFrame.imageIndex === "number" ? rawFrame.imageIndex : -1;
      const image = images[imageIndex];
      frames.push({
        image: String(image?.name ?? image?.path ?? `image#${imageIndex}`),
        symbol: String(rawFrame.symbol ?? `+${rawFrame.imageOffset ?? "?"}`),
        offset: typeof rawFrame.symbolLocation === "number" ? rawFrame.symbolLocation : null,
      });
    }
  }
  const exception = (body.exception ?? {}) as JsonObject;
  const termination = (body.termination ?? {}) as JsonObject;
  return {
    procName: body.procName ?? header.app_name ?? null,
    crashTime: body.captureTime ?? header.timestamp ?? null,
    exceptionType: exception.type ?? null,
    signal: exception.signal ?? null,
    exceptionSubtype: exception.subtype ?? null,
    terminationIndicator: termination.indicator ?? null,
    terminationReasons: termination.reasons ?? null,
    faultingThread: faultingIndex,
    faultingThreadName: faulting?.name ?? faulting?.queue ?? null,
    topFrames: frames,
  };
}

async function cmdCrashes(args: Args) {
  const home = Bun.env.HOME || "";
  const dir = `${home}/Library/Logs/DiagnosticReports`;
  const warnings: string[] = [];
  let names: string[] = [];
  try {
    const { readdirSync } = await import("node:fs");
    names = readdirSync(dir).filter((name) => /^script-kit-gpui.*\.ips$/.test(name));
  } catch {
    warnings.push(`could not read ${dir}`);
  }
  const { statSync } = await import("node:fs");
  const limit = Math.min(args.limit, 10);
  const candidates = names
    .map((name) => {
      const path = `${dir}/${name}`;
      try {
        return { path, modifiedMs: statSync(path).mtimeMs };
      } catch {
        return null;
      }
    })
    .filter((entry): entry is { path: string; modifiedMs: number } => entry !== null)
    .sort((a, b) => b.modifiedMs - a.modifiedMs)
    .slice(0, limit);

  const reports: JsonObject[] = [];
  for (const candidate of candidates) {
    try {
      const parsed = parseIpsReport(await Bun.file(candidate.path).text());
      reports.push({
        path: candidate.path,
        modified: new Date(candidate.modifiedMs).toISOString(),
        ...parsed,
      });
    } catch (error) {
      warnings.push(`failed to parse ${candidate.path}: ${error}`);
    }
  }
  if (reports.length === 0) {
    warnings.push("no script-kit-gpui .ips crash reports found (a missing report does NOT rule out a crash — check `events.ts logs --level error` and the app.log tail too)");
  }

  console.log(JSON.stringify({
    schemaVersion: 1,
    tool: "script-kit-devtools.events",
    command: "events.crashes",
    classification: reports.length > 0 ? "ok" : "not-reproduced",
    source: { dir, matchedFiles: names.length, limit },
    reports,
    warnings,
  }, null, 2));
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  if (args.command === "logs") {
    await cmdLogs(args);
    return;
  }
  if (args.command === "crashes") {
    await cmdCrashes(args);
    return;
  }
  const status = await sessionStatus(args);
  const logPath = String(status.log ?? "");
  const responsesPath = String(status.responses ?? "");
  const mcpAuditPath = args.includeMcpAudit ? defaultMcpAuditPath() : "";
  const startedAt = new Date().toISOString();
  const logOffset = args.command === "record" ? await fileSize(logPath) : 0;
  const responsesOffset = args.command === "record" ? await fileSize(responsesPath) : 0;
  const mcpAuditOffset = args.command === "record" && mcpAuditPath ? await fileSize(mcpAuditPath) : 0;

  let child: JsonObject | null = null;
  let actionOutput: Awaited<ReturnType<typeof outputSummary>> | null = null;
  if (args.command === "record") {
    const childRun = await runCommand(args.childCommand, "recorded-command");
    child = childRun.receipt;
    actionOutput = await outputSummary("recorded-command", childRun.stdout, childRun.stderr, {
      outputPath: args.outputPath || null,
      previewBytes: args.previewBytes,
      inlineFullOutput: args.inlineFullOutput,
    });
  }

  const appEvents = await readLines(logPath, logOffset, args.limit, args.contains);
  const responseEvents = await readLines(responsesPath, responsesOffset, args.limit, args.contains);
  const mcpAuditEvents = mcpAuditPath
    ? await readLines(mcpAuditPath, mcpAuditOffset, args.limit, args.contains)
    : [];
  const endedAt = new Date().toISOString();
  const summary = {
    appLog: countEvents(appEvents),
    responses: countEvents(responseEvents),
    mcpAudit: countEvents(mcpAuditEvents),
  };
  const externalSession = args.externalSession
    ? {
      slug: args.externalSession,
      outputLogPath: args.externalOutputLog || `~/.oracle/sessions/${args.externalSession}/output.log`,
      wrapperTimeoutMs: args.wrapperTimeoutMs || 120000,
      wrapperTimeoutIsNotBrowserSessionFailure: true,
      reattachInstruction: "read output.log and preserve slug/path in the report",
    }
    : null;

  console.log(JSON.stringify({
    schemaVersion: 1,
    tool: "script-kit-devtools.events",
    command: args.command === "record" ? "events.record" : "events.tail",
    classification: classify(status, child, appEvents, responseEvents),
    session: args.session,
    startedAt,
    endedAt,
    source: {
      logPath,
      responsesPath,
      logOffset,
      responsesOffset,
      mcpAuditPath: mcpAuditPath || null,
      mcpAuditOffset,
      includeMcpAudit: args.includeMcpAudit,
      limit: args.limit,
      contains: args.contains || null,
    },
    outputPolicy: {
      previewBytes: args.previewBytes,
      inlineFullOutput: args.inlineFullOutput,
      artifactPath: actionOutput?.artifactPath ?? null,
      actionOutputFields,
    },
    actionOutput,
    failureSignals: classifyFailureSignals(child, appEvents, responseEvents),
    externalSession,
    recordedCommand: args.command === "record" ? args.childCommand : null,
    actionReceipt: compactActionReceipt(child, args.inlineFullOutput),
    eventSummary: summary,
    events: {
      appLog: compactEventEntries(appEvents, args.previewBytes, args.inlineFullOutput),
      responses: compactEventEntries(responseEvents, args.previewBytes, args.inlineFullOutput),
      mcpAudit: compactEventEntries(mcpAuditEvents, args.previewBytes, args.inlineFullOutput),
    },
    warnings: [
      ...summary.appLog.warnings,
      ...summary.responses.warnings,
      ...summary.mcpAudit.warnings,
      appEvents.length === 0 && responseEvents.length === 0 ? "no session events matched the requested span" : "",
    ].filter(Boolean),
    errors: [
      status.status !== "ok" ? status : null,
      child?.status === "error" ? child : null,
    ].filter(Boolean),
  }, null, 2));
}

await main();
