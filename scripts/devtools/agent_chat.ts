#!/usr/bin/env bun

import { inspectAiReliabilityFixture } from "./ai_reliability_cli.ts";
import { assertNoninteractiveSessionCommand } from "./lib/operator-safety.ts";

type JsonObject = Record<string, unknown>;

type Args = {
  command: "open-detached-placeholder" | "open-kitchen-sink" | "inspect";
  session: string;
  start: boolean;
  show: boolean;
  timeoutMs: number;
  fixture?: string;
};

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/agent_chat.ts open-detached-placeholder [--session <name>] [--start] [--show] [--timeout <ms>]",
    "  bun scripts/devtools/agent_chat.ts open-kitchen-sink [--session <name>] [--start] [--show] [--timeout <ms>]",
    "  bun scripts/devtools/agent_chat.ts inspect --fixture image-2-search-budget [--strict]",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  if (
    argv[0] !== "open-detached-placeholder" &&
    argv[0] !== "open-kitchen-sink" &&
    argv[0] !== "inspect"
  ) {
    console.error(usage());
    process.exit(2);
  }
  const args: Args = {
    command: argv[0],
    session: "default",
    start: false,
    show: false,
    timeoutMs: 8000,
  };
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--show") {
      args.show = true;
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
    } else if (arg === "--fixture") {
      args.fixture = argv[++index];
    }
  }
  return args;
}

async function inspectEmbeddedAgentChatTarget(args: Args) {
  let targetReceipt: JsonObject | null = null;
  for (let attempt = 0; attempt < 6; attempt += 1) {
    targetReceipt = await run([
      "bun",
      "scripts/devtools/targets.ts",
      "inspect",
      "--session",
      args.session,
      "--target-kind",
      "main",
      "--surface",
      "AgentChat",
      "--strict",
      "--timeout",
      String(args.timeoutMs),
    ], "targets.inspect.embeddedAgentChat");
    if (targetReceipt.classification === "ok") {
      break;
    }
    await Bun.sleep(50);
  }
  return targetReceipt;
}

async function run(command: string[], label: string): Promise<JsonObject> {
  assertNoninteractiveSessionCommand(command);
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  let parsed: JsonObject | null = null;
  try {
    parsed = JSON.parse(stdout);
  } catch {
    parsed = null;
  }
  if (exitCode !== 0) {
    return { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim(), parsedError: parsed };
  }
  return parsed ?? { status: "ok", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
}

async function maybeStartAndShow(args: Args) {
  if (args.start) {
    await run(["bash", "scripts/agentic/session.sh", "start", args.session], "session-start");
  }
  if (args.show) {
    await run([
      "bash",
      "scripts/agentic/session.sh",
      "send",
      args.session,
      JSON.stringify({ type: "show" }),
      "--await-parse",
      "--timeout",
      String(args.timeoutMs),
    ], "session-show");
  }
}

async function inspectDetachedTarget(args: Args) {
  let targetReceipt: JsonObject | null = null;
  for (let attempt = 0; attempt < 6; attempt += 1) {
    targetReceipt = await run([
      "bun",
      "scripts/devtools/targets.ts",
      "inspect",
      "--session",
      args.session,
      "--target-kind",
      "agentChatDetached",
      "--surface",
      "AgentChat",
      "--strict",
      "--timeout",
      String(args.timeoutMs),
    ], "targets.inspect.agentChatDetached");
    if (targetReceipt.classification === "ok") {
      break;
    }
    await Bun.sleep(50);
  }
  return targetReceipt;
}

async function openDetachedPlaceholder(args: Args) {
  await maybeStartAndShow(args);
  const requestId = `devtools-agent_chat-detached-fixture-${Date.now()}`;
  const openReceipt = await run([
    "bash",
    "scripts/agentic/session.sh",
    "send",
    args.session,
    JSON.stringify({
      type: "openAgentChatDetachedFixture",
      requestId,
    }),
    "--await-parse",
    "--timeout",
    String(args.timeoutMs),
  ], "openAgentChatDetachedFixture");
  const targetReceipt = await inspectDetachedTarget(args);
  const resolvedTarget = targetReceipt?.resolvedTarget as JsonObject | undefined;
  const classification = openReceipt.status === "error"
    ? "blocked-by-timeout"
    : targetReceipt?.classification === "ok"
      ? "ok"
      : "blocked-by-target-ambiguity";

  console.log(JSON.stringify({
    schemaVersion: 1,
    tool: "script-kit-devtools.agent_chat",
    command: "agent_chat.openDetachedPlaceholder",
    classification,
    session: args.session,
    requestId,
    safety: {
      providerRequired: false,
      liveThreadRequired: false,
      fixtureOnly: true,
    },
    target: resolvedTarget ?? null,
    resolvedTarget: resolvedTarget ?? null,
    openReceipt,
    targetReceipt,
    errors: [openReceipt, targetReceipt].filter((receipt) => receipt?.status === "error"),
  }, null, 2));
}

async function openKitchenSink(args: Args) {
  await maybeStartAndShow(args);
  const requestId = `devtools-agent_chat-kitchen-sink-${Date.now()}`;
  const openReceipt = await run([
    "bash",
    "scripts/agentic/session.sh",
    "rpc",
    args.session,
    JSON.stringify({
      type: "openAgentChatKitchenSinkFixture",
      requestId,
    }),
    "--timeout",
    String(args.timeoutMs),
  ], "openAgentChatKitchenSinkFixture");
  const targetReceipt = await inspectEmbeddedAgentChatTarget(args);
  const resolvedTarget = targetReceipt?.resolvedTarget as JsonObject | undefined;
  const classification = openReceipt.status === "error"
    ? "blocked-by-timeout"
    : targetReceipt?.classification === "ok"
      ? "ok"
      : "blocked-by-target-ambiguity";

  console.log(JSON.stringify({
    schemaVersion: 1,
    tool: "script-kit-devtools.agent_chat",
    command: "agent_chat.openKitchenSink",
    classification,
    session: args.session,
    requestId,
    safety: {
      providerRequired: false,
      liveThreadRequired: false,
      fixtureOnly: true,
    },
    target: resolvedTarget ?? null,
    resolvedTarget: resolvedTarget ?? null,
    openReceipt,
    targetReceipt,
    errors: [openReceipt, targetReceipt].filter((receipt) => receipt?.status === "error"),
  }, null, 2));
}

const args = parseArgs(Bun.argv.slice(2));
if (args.command === "inspect") {
  if (!args.fixture) {
    console.error("--fixture is required");
    process.exit(2);
  }
  await inspectAiReliabilityFixture(
    "script-kit-devtools.agent_chat",
    args.fixture,
    "quickAi",
    Bun.argv.includes("--strict"),
  );
} else if (args.command === "open-kitchen-sink") {
  await openKitchenSink(args);
} else {
  await openDetachedPlaceholder(args);
}
