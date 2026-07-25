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

import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { Driver } from "../devtools/driver.ts";

const ARTIFACT_BINARY = "target-agent/artifacts/phase-trace/script-kit-gpui";

interface Args {
  trials: number;
  surfaces: string[];
  keep: boolean;
}

function parseArgs(): Args {
  const argv = process.argv.slice(2);
  const value = (flag: string) => {
    const index = argv.indexOf(flag);
    return index >= 0 ? argv[index + 1] : undefined;
  };
  return {
    trials: Number(value("--trials") ?? 5),
    surfaces: (value("--surfaces") ?? "quick-ai,agent-chat").split(",").map((s) => s.trim()),
    // Without --keep the trace is wiped so a run's numbers are unambiguously
    // its own. With it, several surface runs accumulate into one receipt —
    // which is what the committed cross-surface report needs, since the
    // surfaces cannot all be driven in a single app session.
    keep: argv.includes("--keep"),
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

async function main() {
  const args = parseArgs();
  const outDir = join(process.cwd(), ".notes/oracle/ai-phase-trace-all");
  mkdirSync(outDir, { recursive: true });
  const tracePath = join(outDir, "phase-trace.ndjson");
  if (!args.keep) rmSync(tracePath, { force: true });

  const binary = existsSync(ARTIFACT_BINARY) ? ARTIFACT_BINARY : undefined;
  const attempts: Attempt[] = [];

  const driver = await Driver.launch({
    sessionName: `ai-phase-trace-${process.pid}`,
    binary,
    sandboxHome: true,
    seedAgentAuth: true,
    env: {
      SCRIPT_KIT_AI_TRACE_PATH: tracePath,
      SCRIPT_KIT_CODEX_BIN: process.env.SCRIPT_KIT_CODEX_BIN ?? "codex",
    },
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 40_000,
  });

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
