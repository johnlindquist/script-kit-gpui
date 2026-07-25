#!/usr/bin/env bun
/**
 * Drive real AI turns through the real app so the shared phase trace has
 * something honest to report.
 *
 * This produces the BEFORE numbers. It deliberately does not mock the
 * provider: a fixture would measure the fixture. Where a surface cannot be
 * driven in this environment, the probe records WHY and leaves that surface
 * unmeasured rather than substituting a fixture number and calling it real.
 *
 * Usage:
 *   bun scripts/agentic/ai-phase-trace-probe.ts [--trials N] [--surfaces a,b]
 */

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver } from "../devtools/driver.ts";

const ARTIFACT_BINARY = "target-agent/artifacts/phase-trace/script-kit-gpui";

interface Args {
  trials: number;
  surfaces: string[];
  keep: boolean;
  binary?: string;
  model?: string;
  prewarm?: boolean;
  flowName?: string;
}

function parseArgs(): Args {
  const argv = process.argv.slice(2);
  const value = (flag: string) => {
    const index = argv.indexOf(flag);
    return index >= 0 ? argv[index + 1] : undefined;
  };
  const prewarmFlag = value("--prewarm");
  return {
    trials: Number(value("--trials") ?? 5),
    surfaces: (value("--surfaces") ?? "quick-ai,agent-chat").split(",").map((s) => s.trim()),
    // Without --keep the trace is wiped so a run's numbers are unambiguously
    // its own. With it, several surface runs accumulate into one receipt —
    // which is what the committed cross-surface report needs, since the
    // surfaces cannot all be driven in a single app session.
    keep: argv.includes("--keep"),
    binary: value("--binary"),
    // The repo-pinned Pi sidecar does not carry every model the app's static
    // catalog names, so a run can die on "Model ... not found" before it ever
    // reaches the code under measurement. Pinning the model here keeps that
    // environment fact out of the numbers WITHOUT editing a product default.
    model: value("--model"),
    // Pi sidecars are prewarmed. Measuring with prewarm forced off is how we
    // separate per-turn cost from one-time process spawn cost.
    prewarm: prewarmFlag === undefined ? undefined : prewarmFlag !== "off",
    flowName: value("--flow-name"),
  };
}

interface Attempt {
  surface: string;
  trial: number;
  ok: boolean;
  detail: string;
  wallMs: number;
}

function readTrace(path: string): Record<string, unknown>[] {
  if (!existsSync(path)) return [];
  return readFileSync(path, "utf8")
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .flatMap((line) => {
      try {
        return [JSON.parse(line) as Record<string, unknown>];
      } catch {
        return [];
      }
    });
}

/** Wait until a surface records a terminal event beyond the known count. */
async function waitForTerminal(
  path: string,
  surface: string,
  knownTerminals: number,
  timeoutMs: number,
): Promise<Record<string, unknown> | null> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const terminals = readTrace(path).filter(
      (record) => record.surface === surface && record.event === "terminal",
    );
    if (terminals.length > knownTerminals) return terminals[terminals.length - 1];
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  return null;
}

const QUICK_AI_QUERIES = [
  "what is the capital of Portugal",
  "who wrote the novel Dune",
  "what does the acronym HTTP stand for",
  "how many days are in a leap year",
  "what is the chemical symbol for tin",
  "what year did the Berlin Wall fall",
  "what is the largest moon of Saturn",
];

const FOCUSED_TEXT_SAMPLES = [
  "The meeting has been rescheduled to Thursday afternoon due to a conflict.",
  "Please find attached the quarterly figures for your review and approval.",
  "We regret to inform you that the shipment will arrive later than planned.",
  "Thanks for reaching out — I will look into this and get back to you soon.",
  "The build failed because a dependency was missing from the lock file.",
];

async function main() {
  const args = parseArgs();
  const outDir = join(process.cwd(), ".notes/oracle/ai-phase-trace-all");
  mkdirSync(outDir, { recursive: true });
  const tracePath = join(outDir, "phase-trace.ndjson");
  if (!args.keep) rmSync(tracePath, { force: true });

  const binary =
    args.binary ?? (existsSync(ARTIFACT_BINARY) ? ARTIFACT_BINARY : undefined);
  const attempts: Attempt[] = [];

  const env: Record<string, string> = {
    SCRIPT_KIT_AI_TRACE_PATH: tracePath,
    SCRIPT_KIT_CODEX_BIN: process.env.SCRIPT_KIT_CODEX_BIN ?? "codex",
  };
  if (args.prewarm === false) env.SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM = "1";
  if (args.prewarm === true) env.SCRIPT_KIT_ENABLE_AGENT_CHAT_HOT_PREWARM = "1";

  const driver = await Driver.launch({
    sessionName: `ai-phase-trace-${process.pid}`,
    binary,
    sandboxHome: true,
    seedAgentAuth: true,
    env,
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 40_000,
  });

  // The sandbox HOME only exists once the driver has built it, so the model
  // override is written immediately after launch and before any AI surface is
  // opened. `config.ts` did not exist at process start, so the loader has no
  // memoised entry to serve instead — the first profile resolution reads this.
  if (args.model) {
    writeFileSync(
      join(driver.sessionDir, "home", ".scriptkit", "config.ts"),
      `export default ${JSON.stringify({ ai: { selectedModelId: args.model } }, null, 2)}\n`,
    );
  }

  // Flows are discovered by `md roster --json` run in the app's cwd, and that
  // cwd is the Spine default `$HOME/.scriptkit` — NOT the process working
  // directory. So the fixture flow is seeded into the sandbox, where the app
  // actually looks.
  //
  // It must be a fixture, never one of the repo's own flows/**: those are real
  // delegation briefs, and "measuring" one would set an agent loose on this
  // checkout. This one is explicitly read-only.
  if (args.surfaces.includes("flow")) {
    const flowsDir = join(driver.sessionDir, "home", ".scriptkit", "flows");
    mkdirSync(flowsDir, { recursive: true });
    writeFileSync(
      join(flowsDir, "ping.md"),
      [
        "---",
        "name: ping",
        "description: Minimal read-only flow used to measure flow turn latency.",
        "engine: codex",
        "---",
        "",
        "Reply with exactly the word: pong",
        "",
        "Do not read, create, modify, or delete any files. Do not run commands.",
        "",
      ].join("\n"),
    );
  }

  try {
    await driver.waitForSettle();
    driver.send({ type: "show" });
    await driver.waitForState({ windowVisible: true }, { timeoutMs: 15_000 });

    // ---- Quick AI: launcher Tab-with-text. The one proven entry path. ----
    if (args.surfaces.includes("quick-ai")) {
      for (let trial = 1; trial <= args.trials; trial += 1) {
        const query = QUICK_AI_QUERIES[(trial - 1) % QUICK_AI_QUERIES.length];
        const before = readTrace(tracePath).filter(
          (record) => record.surface === "quick-ai" && record.event === "terminal",
        ).length;
        const started = performance.now();
        try {
          // Escape dismisses the launcher, so the window must be re-shown
          // before the next trial. Without the explicit `show` the filter is
          // set on a hidden window and the Tab press reaches nothing — which
          // presents as a turn that never starts rather than as an error.
          driver.simulateKey("escape");
          driver.send({ type: "show" });
          await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
          await driver.waitForSettle();
          await driver.setFilterAndWait(query, { timeoutMs: 15_000 });
          await driver.simulateGpuiEvent(
            { type: "keyDown", key: "tab" },
            { target: { type: "kind", kind: "main" }, timeoutMs: 15_000 },
          );
          const terminal = await waitForTerminal(tracePath, "quick-ai", before, 90_000);
          attempts.push({
            surface: "quick-ai",
            trial,
            ok: terminal?.outcome === "completed",
            detail: terminal
              ? `outcome=${terminal.outcome} elapsedMs=${terminal.elapsedMs}`
              : "no terminal record within 90s",
            wallMs: Math.round(performance.now() - started),
          });
        } catch (error) {
          attempts.push({
            surface: "quick-ai",
            trial,
            ok: false,
            detail: `driver error: ${(error as Error).message}`,
            wallMs: Math.round(performance.now() - started),
          });
        }
      }
    }

    // ---- Agent Chat: Cmd+Enter is the universal AI entry from the launcher.
    // With an empty filter it opens a clean chat; the composer then needs the
    // prompt typed and submitted. If this path does not produce a turn in this
    // environment, that is recorded rather than papered over with a fixture.
    if (args.surfaces.includes("agent-chat")) {
      for (let trial = 1; trial <= args.trials; trial += 1) {
        const before = readTrace(tracePath).filter(
          (record) => record.surface === "agent-chat" && record.event === "terminal",
        ).length;
        const started = performance.now();
        try {
          // Escape dismisses the launcher, so the window must be re-shown
          // before the next trial. Without the explicit `show` the filter is
          // set on a hidden window and the Tab press reaches nothing — which
          // presents as a turn that never starts rather than as an error.
          driver.simulateKey("escape");
          driver.send({ type: "show" });
          await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
          await driver.waitForSettle();
          await driver.setFilterAndWait(QUICK_AI_QUERIES[trial % QUICK_AI_QUERIES.length], {
            timeoutMs: 15_000,
          });
          await driver.simulateGpuiEvent(
            { type: "keyDown", key: "enter", modifiers: ["cmd"] },
            { target: { type: "kind", kind: "main" }, timeoutMs: 15_000 },
          );
          await driver.waitForSettle();
          await driver.simulateGpuiEvent(
            { type: "keyDown", key: "enter" },
            { target: { type: "kind", kind: "main" }, timeoutMs: 15_000 },
          );
          const terminal = await waitForTerminal(tracePath, "agent-chat", before, 120_000);
          attempts.push({
            surface: "agent-chat",
            trial,
            ok: terminal?.outcome === "completed",
            detail: terminal
              ? `outcome=${terminal.outcome} elapsedMs=${terminal.elapsedMs}`
              : "no terminal record within 120s",
            wallMs: Math.round(performance.now() - started),
          });
        } catch (error) {
          attempts.push({
            surface: "agent-chat",
            trial,
            ok: false,
            detail: `driver error: ${(error as Error).message}`,
            wallMs: Math.round(performance.now() - started),
          });
        }
      }
    }
    // ---- Text / Mini: the focused-text rewrite surface, driven against real
    // Pi rather than the deterministic mock twin. A non-empty `instruction` is
    // what makes the fixture actually submit (`requested_submit =
    // instruction_length > 0`), so an empty one would open the window and
    // measure nothing. One submit fans out into variation turns; the primary
    // traces as `text` and the auxiliary ones as `mini`, which is why both
    // surfaces are collected from the same drive.
    if (args.surfaces.includes("text") || args.surfaces.includes("mini")) {
      for (let trial = 1; trial <= args.trials; trial += 1) {
        const beforeText = readTrace(tracePath).filter(
          (record) => record.surface === "text" && record.event === "terminal",
        ).length;
        const started = performance.now();
        try {
          driver.simulateKey("escape");
          driver.send({ type: "show" });
          await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
          await driver.waitForSettle();
          driver.send({
            type: "openFocusedTextAgentChatWithPiData",
            text: FOCUSED_TEXT_SAMPLES[(trial - 1) % FOCUSED_TEXT_SAMPLES.length],
            instruction: "Rewrite this more concisely.",
          });
          const terminal = await waitForTerminal(tracePath, "text", beforeText, 120_000);
          attempts.push({
            surface: "text",
            trial,
            ok: terminal?.outcome === "completed",
            detail: terminal
              ? `outcome=${terminal.outcome} elapsedMs=${terminal.elapsedMs}`
              : "no terminal record within 120s",
            wallMs: Math.round(performance.now() - started),
          });
        } catch (error) {
          attempts.push({
            surface: "text",
            trial,
            ok: false,
            detail: `driver error: ${(error as Error).message}`,
            wallMs: Math.round(performance.now() - started),
          });
        }
      }
    }
    // ---- Flow: launch a flow from the launcher, then converse. Enter on a
    // flow row opens the conversation (SessionTransport::CodexThread), which
    // is the `codex app-server` path the trace instruments; the MAIN input
    // then acts as the composer, so the second Enter submits a turn.
    //
    // The flow driven here is the seeded read-only fixture, never one of the
    // repo's own flows/**: those are real delegation briefs, and "measuring"
    // one would set an agent loose on this checkout.
    if (args.surfaces.includes("flow")) {
      for (let trial = 1; trial <= args.trials; trial += 1) {
        const before = readTrace(tracePath).filter(
          (record) => record.surface === "flow" && record.event === "terminal",
        ).length;
        const started = performance.now();
        try {
          driver.simulateKey("escape");
          driver.send({ type: "show" });
          await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
          await driver.waitForSettle();
          await driver.setFilterAndWait(args.flowName ?? "ping", { timeoutMs: 15_000 });
          await driver.simulateGpuiEvent(
            { type: "keyDown", key: "enter" },
            { target: { type: "kind", kind: "main" }, timeoutMs: 15_000 },
          );
          await driver.waitForSettle();
          await driver.setFilterAndWait("Reply with exactly: pong", { timeoutMs: 15_000 });
          await driver.simulateGpuiEvent(
            { type: "keyDown", key: "enter" },
            { target: { type: "kind", kind: "main" }, timeoutMs: 15_000 },
          );
          const terminal = await waitForTerminal(tracePath, "flow", before, 180_000);
          attempts.push({
            surface: "flow",
            trial,
            ok: terminal?.outcome === "completed",
            detail: terminal
              ? `outcome=${terminal.outcome} elapsedMs=${terminal.elapsedMs}`
              : "no terminal record within 180s",
            wallMs: Math.round(performance.now() - started),
          });
        } catch (error) {
          attempts.push({
            surface: "flow",
            trial,
            ok: false,
            detail: `driver error: ${(error as Error).message}`,
            wallMs: Math.round(performance.now() - started),
          });
        }
      }
    }
  } finally {
    driver.send({ type: "hide" });
    await driver.close();
  }

  console.log(JSON.stringify({ tracePath, attempts }, null, 2));
  const succeeded = attempts.filter((attempt) => attempt.ok).length;
  console.log(`\nPROBE_OK=${succeeded}/${attempts.length} trace=${tracePath}`);
  if (succeeded === 0) process.exit(1);
}

main().catch((error) => {
  console.error(`PROBE_FAILED ${(error as Error).stack}`);
  process.exit(1);
});
