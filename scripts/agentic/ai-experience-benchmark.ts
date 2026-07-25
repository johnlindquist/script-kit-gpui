/**
 * AI experience benchmark: drive every AI surface through its real user entry
 * path and record phase timings from live protocol state.
 *
 * Quick AI additionally emits an in-app NDJSON phase trace via
 * SCRIPT_KIT_QUICK_AI_TRACE_PATH (src/ai/agent_chat/codex_exec.rs TraceSink),
 * giving exact elapsedMs for spawn_started / first_protocol_event /
 * turn_started / search_permit_reserved / search_completed / terminal.
 * No other AI surface has an equivalent instrument -- that gap is a finding.
 *
 * Usage:
 *   bun scripts/agentic/ai-experience-benchmark.ts                  # all surfaces
 *   bun scripts/agentic/ai-experience-benchmark.ts --surface quickAi --reps 3
 *   bun scripts/agentic/ai-experience-benchmark.ts --explore        # dump shapes
 *
 * Emits one JSON receipt on stdout; progress lines go to stderr.
 */
import { Driver, type Json } from "../devtools/driver.ts";
import { existsSync, readFileSync, rmSync } from "node:fs";

const ARGS = process.argv.slice(2);
const EXPLORE = ARGS.includes("--explore");
const argOf = (flag: string): string | null => {
  const i = ARGS.indexOf(flag);
  return i >= 0 ? (ARGS[i + 1] ?? null) : null;
};
const ONLY = argOf("--surface");
const REPS = Number(argOf("--reps") ?? "1");
const QUERY = argOf("--query") ?? "Did LeBron join a team yet?";
const POLL_MS = 120;
const TURN_TIMEOUT_MS = 60_000;
const SETTLE_MS = 3000;

/** Fields that actually move during a turn (schemaVersion 6). */
function progressKey(s: Json): string {
  return JSON.stringify([
    s.status,
    s.messageCount,
    s.awaitingFirstAssistantText,
    s.hasPendingPermission,
    typeof s.inputText === "string" ? s.inputText.length : null,
  ]);
}

async function readState(d: Driver): Promise<Json | null> {
  try {
    return await d.request({ type: "getAgentChatState" }, { timeoutMs: 8000 });
  } catch {
    return null;
  }
}

async function trackTurn(d: Driver, t0: number) {
  const phases: { t: number; key: string }[] = [];
  let lastKey = "";
  let lastChange = performance.now();
  let final: Json | null = null;
  let firstChangeMs: number | null = null;
  while (performance.now() - t0 < TURN_TIMEOUT_MS) {
    const s = await readState(d);
    if (s) {
      final = s;
      const k = progressKey(s);
      if (k !== lastKey) {
        const t = Math.round(performance.now() - t0);
        if (lastKey !== "" && firstChangeMs === null) firstChangeMs = t;
        phases.push({ t, key: k });
        lastKey = k;
        lastChange = performance.now();
      } else if (phases.length > 1 && performance.now() - lastChange > SETTLE_MS) {
        break;
      }
    }
    await Bun.sleep(POLL_MS);
  }
  return { phases, final, firstChangeMs };
}

function readTrace(path: string): Json[] {
  if (!existsSync(path)) return [];
  return readFileSync(path, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      try { return JSON.parse(l) as Json; } catch { return null; }
    })
    .filter((x): x is Json => x !== null);
}

type Spec = {
  open: (d: Driver) => Promise<void>;
  submit: (d: Driver) => Promise<void>;
  backend: string;
};

const SURFACES: Record<string, Spec> = {
  quickAi: {
    backend: "codex exec (subprocess per turn)",
    open: async (d) => { await d.setFilterAndWait(QUERY); },
    submit: async (d) => { d.simulateKey("tab"); },
  },
  agentChat: {
    backend: "Pi sidecar (General profile)",
    // OpenAi is a unit variant with no requestId -> never replies. Fire and poll.
    open: async (d) => { d.send({ type: "openAi" }); await waitForAgentSurface(d); },
    submit: async (d) => {
      await d.request(
        { type: "setAgentChatInput", text: QUERY, submit: true },
        { timeoutMs: 10_000 },
      );
    },
  },
  text: {
    backend: "Pi sidecar (Text profile)",
    open: async (d) => {
      await d.request({ type: "openFocusedTextAgentChatWithPiData" }, { expect: "externalCommandResult", timeoutMs: 20_000 });
      await waitForAgentSurface(d);
    },
    submit: async (d) => {
      await d.request(
        { type: "setAgentChatInput", text: QUERY, submit: true },
        { timeoutMs: 10_000 },
      );
    },
  },
  // Mini AI renders into the SAME window id ("main") and semanticSurface
  // ("agentChatChat") as Agent Chat -- verified via listAutomationWindows --
  // so it takes the same composer command, not setAiInput.
  miniAi: {
    backend: "Pi sidecar (mini AI variant, shares agentChatChat surface)",
    open: async (d) => { d.send({ type: "openMiniAi" }); await waitForAgentSurface(d); },
    submit: async (d) => {
      await d.request(
        { type: "setAgentChatInput", text: QUERY, submit: true },
        { timeoutMs: 10_000 },
      );
    },
  },
};

/** Poll until getAgentChatState stops reporting notAgentChat (surface is live). */
async function waitForAgentSurface(d: Driver, timeoutMs = 20_000): Promise<boolean> {
  const start = performance.now();
  while (performance.now() - start < timeoutMs) {
    const s = await readState(d);
    if (s && s.status && s.status !== "notAgentChat") return true;
    await Bun.sleep(150);
  }
  return false;
}

async function runOnce(name: string, spec: Spec, rep: number): Promise<Json> {
  const tracePath = `/tmp/ai-bench-trace/${name}-${process.pid}-${rep}.ndjson`;
  try { rmSync(tracePath, { force: true }); } catch { /* ignore */ }
  const d = await Driver.launch({
    sandboxHome: true,
    seedAgentAuth: true,
    env: { SCRIPT_KIT_QUICK_AI_TRACE_PATH: tracePath },
  });
  const rec: Json = { surface: name, rep, backend: spec.backend, pid: d.pid };
  try {
    await d.waitForSettle();
    const tOpen = performance.now();
    await spec.open(d);
    rec.openMs = Math.round(performance.now() - tOpen);

    if (EXPLORE) {
      rec.state = await readState(d);
      rec.windows = await d.listAutomationWindows({ timeoutMs: 8000 });
      return rec;
    }

    const t0 = performance.now();
    await spec.submit(d);
    const { phases, final, firstChangeMs } = await trackTurn(d, t0);
    rec.firstStateChangeMs = firstChangeMs;
    rec.totalMs = phases.length ? phases[phases.length - 1].t : null;
    rec.stateTransitions = phases.length;
    rec.finalStatus = final?.status ?? null;
    rec.finalMessageCount = final?.messageCount ?? null;
    rec.trace = readTrace(tracePath).map((e) => ({ event: e.event, elapsedMs: e.elapsedMs }));
  } catch (err) {
    rec.error = String(err);
  } finally {
    try {
      const lg = await d.getLogs({ limit: 80 });
      const entries = Array.isArray(lg?.entries) ? (lg.entries as Json[]) : [];
      rec.aiLogs = entries
        .filter((e) => typeof e.message === "string" &&
          /quick_ai|agent_chat_turn|pi_|codex_quick/.test(e.message as string))
        .slice(-12)
        .map((e) => e.message);
    } catch { /* best effort */ }
    await d.close();
  }
  return rec;
}

const out: Json = { schemaVersion: 1, query: QUERY, reps: REPS, results: [] };
for (const [name, spec] of Object.entries(SURFACES)) {
  if (ONLY && ONLY !== name) continue;
  for (let r = 1; r <= REPS; r++) {
    const rec = await runOnce(name, spec, r);
    (out.results as Json[]).push(rec);
    console.error(
      `[bench] ${name} rep${r}: open=${rec.openMs}ms first=${rec.firstStateChangeMs ?? "-"}ms ` +
      `total=${rec.totalMs ?? "n/a"}ms transitions=${rec.stateTransitions ?? 0} ` +
      `trace=${Array.isArray(rec.trace) ? (rec.trace as Json[]).length : 0} ${rec.error ?? ""}`,
    );
  }
}
console.log(JSON.stringify(out, null, 2));
