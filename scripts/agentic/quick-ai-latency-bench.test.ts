/**
 * Unit tests for the paired Quick AI latency benchmark.
 *
 * These exist because the benchmark IS the acceptance authority for the
 * force-first-native-search change: if the harness can be fooled — by counting
 * a lifecycle event as a search, by dropping a slow-but-successful run, by
 * letting one arm's order dominate — then a "measured improvement" is a fiction.
 * Every test below pins one property the final verdict leans on.
 */
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  analyzeStream,
  bootstrapMedianDeltaCi95,
  buildCommand,
  collectValidPairs,
  effectivePrompt,
  evaluateKnowledgeGates,
  isValidForClass,
  loadContract,
  median,
  medianPairedDelta,
  pairOrder,
  pairedStat,
  parseCli,
  percentReduction,
  refToSource,
  scrapeConfigFlags,
  sha256,
  winCount,
  type Attempt,
  type QuickAiCommandContract,
  type StreamAnalysis,
  type TimedLine,
} from "./quick-ai-latency-bench";

// ------------------------------------------------------------------- fixtures

const ev = (atMs: number, obj: unknown): TimedLine => ({ atMs, line: JSON.stringify(obj) });

/** A live turn: boot, one search (started + completed), one answer. */
function searchStream(opts: { pass1: number; searchMs: number; pass2: number }): TimedLine[] {
  const boot = 400;
  const searchStart = boot + opts.pass1;
  const searchDone = searchStart + opts.searchMs;
  const answer = searchDone + opts.pass2;
  return [
    ev(boot, { type: "thread.started" }),
    ev(boot + 1, { type: "turn.started" }),
    ev(searchStart, {
      type: "item.started",
      item: { id: "ws_1", type: "web_search", query: "x", action: { type: "other" } },
    }),
    ev(searchDone, {
      type: "item.completed",
      item: {
        id: "ws_1",
        type: "web_search",
        query: "x",
        action: { type: "search", query: "x", queries: ["x"] },
      },
    }),
    ev(answer, {
      type: "item.completed",
      item: {
        id: "am_1",
        type: "agent_message",
        text: JSON.stringify({ answer: "yes", sources: [{ url: "https://a.example/x", title: "t" }] }),
      },
    }),
    ev(answer + 5, { type: "turn.completed", usage: {} }),
  ];
}

/** A knowledge turn: boot, no search, one answer with empty sources. */
function noSearchStream(generateMs: number): TimedLine[] {
  const boot = 900;
  return [
    ev(boot, { type: "thread.started" }),
    ev(boot + 1, { type: "turn.started" }),
    ev(boot + generateMs, {
      type: "item.completed",
      item: { id: "am_1", type: "agent_message", text: JSON.stringify({ answer: "it propagates errors", sources: [] }) },
    }),
    ev(boot + generateMs + 5, { type: "turn.completed", usage: {} }),
  ];
}

const LIVE_Q = { id: "live", text: "Did LeBron join a team yet?", needsSearch: true, forced: true };
const KNOW_Q = { id: "knowledge", text: "What does the Rust ? operator do?", needsSearch: false, forced: false };

function attempt(over: Partial<Attempt> & { analysis: StreamAnalysis }): Attempt {
  return {
    pairIndex: 0,
    arm: "baseline",
    query: "live",
    contractSource: "git:base",
    promptSha256: "p",
    commandSha256: "c",
    forcedPromptApplied: false,
    exitCode: 0,
    valid: true,
    stderrTail: "",
    ...over,
  };
}

// --------------------------------------------------------------------- medians

describe("median", () => {
  test("odd length returns the middle element", () => {
    expect(median([5, 1, 3])).toBe(3);
  });

  test("even length averages the two middle elements", () => {
    expect(median([1, 2, 3, 4])).toBe(2.5);
  });

  test("empty input is NaN rather than 0, so it cannot silently pass a gate", () => {
    expect(Number.isNaN(median([]))).toBe(true);
  });

  test("percentReduction is positive when the candidate is faster", () => {
    expect(percentReduction(4000, 3000)).toBeCloseTo(25, 6);
  });

  test("percentReduction on a zero baseline is NaN, never Infinity", () => {
    expect(Number.isNaN(percentReduction(0, 100))).toBe(true);
  });

  test("medianPairedDelta keeps the sign of the improvement", () => {
    expect(medianPairedDelta([-900, -1200, -800])).toBe(-900);
  });

  test("winCount counts only strictly negative deltas", () => {
    expect(winCount([-1, 0, 1, -5])).toBe(2);
  });
});

// ------------------------------------------------------------------- pair order

describe("pairOrder", () => {
  test("alternates so provider warm-up cannot favour one arm", () => {
    expect(pairOrder(0, 0)).toEqual(["baseline", "candidate"]);
    expect(pairOrder(1, 0)).toEqual(["candidate", "baseline"]);
    expect(pairOrder(2, 0)).toEqual(["baseline", "candidate"]);
  });

  test("an odd seed flips which parity starts but keeps the order balanced", () => {
    expect(pairOrder(0, 1)).toEqual(["candidate", "baseline"]);
    expect(pairOrder(1, 1)).toEqual(["baseline", "candidate"]);
  });

  test("over 16 pairs each arm goes first exactly half the time, for either seed", () => {
    for (const seed of [0, 1, 20260725]) {
      const firsts = Array.from({ length: 16 }, (_, i) => pairOrder(i, seed)[0]);
      expect(firsts.filter((a) => a === "baseline").length).toBe(8);
      expect(firsts.filter((a) => a === "candidate").length).toBe(8);
    }
  });
});

// -------------------------------------------------------------- phase extraction

describe("analyzeStream", () => {
  test("extracts all four phases from a real-shaped search stream", () => {
    const a = analyzeStream(searchStream({ pass1: 3000, searchMs: 2600, pass2: 800 }), 6800);
    expect(a.phases.spawnToFirstEventMs).toBe(400);
    expect(a.phases.firstEventToSearchStartMs).toBe(3000);
    expect(a.phases.searchStartToSearchCompleteMs).toBe(2600);
    expect(a.phases.searchCompleteToAnswerMs).toBe(800);
    expect(a.phases.totalMs).toBe(6800);
  });

  test("a no-search stream has null search phases and measures generation from boot", () => {
    const a = analyzeStream(noSearchStream(4400), 5400);
    expect(a.phases.firstEventToSearchStartMs).toBeNull();
    expect(a.phases.searchStartToSearchCompleteMs).toBeNull();
    // PASS 2 falls back to first-event when no search ran: this is the generate phase.
    expect(a.phases.searchCompleteToAnswerMs).toBe(4400);
    expect(a.distinctSearchItemCount).toBe(0);
  });

  test("counts DISTINCT search items, not lifecycle events", () => {
    // The old substring harness counted this as two searches because
    // item.started and item.completed both contain `"type":"web_search"`.
    const a = analyzeStream(searchStream({ pass1: 100, searchMs: 100, pass2: 100 }), 1000);
    expect(a.distinctSearchItemCount).toBe(1);
    expect(a.searchActionCount).toBe(1);
  });

  test("two distinct search item ids count as two searches", () => {
    const lines = [
      ev(10, { type: "thread.started" }),
      ev(20, { type: "item.completed", item: { id: "ws_1", type: "web_search", action: { type: "search", query: "a" } } }),
      ev(30, { type: "item.completed", item: { id: "ws_2", type: "web_search", action: { type: "search", query: "b" } } }),
    ];
    const a = analyzeStream(lines, 100);
    expect(a.distinctSearchItemCount).toBe(2);
    expect(a.searchActionCount).toBe(2);
  });

  test("open_page and find_in_page are page follows, not searches", () => {
    for (const type of ["open_page", "find_in_page"]) {
      const lines = [
        ev(10, { type: "thread.started" }),
        ev(20, { type: "item.completed", item: { id: "ws_1", type: "web_search", action: { type: "search", query: "a" } } }),
        ev(30, { type: "item.completed", item: { id: "ws_2", type: "web_search", action: { type, url: "https://x.example" } } }),
      ];
      const a = analyzeStream(lines, 100);
      expect(a.pageFollowActionCount).toBe(1);
      expect(a.searchActionCount).toBe(1);
    }
  });

  test("forbidden item types are counted so a tool-gate escape cannot pass as valid", () => {
    const lines = [
      ev(10, { type: "thread.started" }),
      ev(20, { type: "item.completed", item: { id: "cmd_1", type: "command_execution" } }),
    ];
    expect(analyzeStream(lines, 100).forbiddenItemCount).toBe(1);
  });

  test("counts events before the first search — the PASS 1 deliberation footprint", () => {
    const lines = [
      ev(10, { type: "thread.started" }),
      ev(20, { type: "turn.started" }),
      ev(30, { type: "item.completed", item: { id: "r_1", type: "reasoning" } }),
      ev(40, { type: "item.started", item: { id: "ws_1", type: "web_search", action: { type: "other" } } }),
    ];
    expect(analyzeStream(lines, 100).eventsBeforeFirstSearch).toBe(3);
  });

  test("parses the structured answer and its source count", () => {
    const a = analyzeStream(searchStream({ pass1: 10, searchMs: 10, pass2: 10 }), 100);
    expect(a.structuredAnswerParsed).toBe(true);
    expect(a.structuredSourceCount).toBe(1);
    expect(a.completedAgentMessageCount).toBe(1);
  });

  test("an empty sources array is honest, not a parse failure", () => {
    const a = analyzeStream(noSearchStream(100), 1100);
    expect(a.structuredAnswerParsed).toBe(true);
    expect(a.structuredSourceCount).toBe(0);
  });

  test("turn.failed and error events mark the turn as failed", () => {
    for (const type of ["turn.failed", "error"]) {
      const lines = [ev(10, { type: "thread.started" }), ev(20, { type, message: "boom", error: { message: "boom" } })];
      expect(analyzeStream(lines, 100).turnFailed).toBe(true);
    }
  });

  test("malformed lines are counted, not thrown, so one bad line cannot abort a run", () => {
    const lines: TimedLine[] = [{ atMs: 10, line: "{not json" }, ev(20, { type: "thread.started" })];
    const a = analyzeStream(lines, 100);
    expect(a.parseErrorCount).toBe(1);
    // Boot is measured to the first PARSEABLE protocol event: unparseable bytes
    // are not evidence the model started working, so crediting them would
    // understate boot and overstate PASS 1.
    expect(a.phases.spawnToFirstEventMs).toBe(20);
  });
});

// -------------------------------------------------------------------- validity

describe("isValidForClass", () => {
  test("a slow but successful live turn is VALID — no outlier trimming", () => {
    const a = analyzeStream(searchStream({ pass1: 9000, searchMs: 5000, pass2: 1500 }), 15_900);
    expect(isValidForClass(a, LIVE_Q, 0)).toBe(true);
  });

  test("a live turn with zero searches is invalid for its class", () => {
    const a = analyzeStream(noSearchStream(1000), 2000);
    expect(isValidForClass(a, LIVE_Q, 0)).toBe(false);
  });

  test("a knowledge turn that searched is invalid for its class", () => {
    const a = analyzeStream(searchStream({ pass1: 10, searchMs: 10, pass2: 10 }), 100);
    expect(isValidForClass(a, KNOW_Q, 0)).toBe(false);
  });

  test("a nonzero exit code invalidates the turn", () => {
    const a = analyzeStream(searchStream({ pass1: 10, searchMs: 10, pass2: 10 }), 100);
    expect(isValidForClass(a, LIVE_Q, 1)).toBe(false);
  });

  test("a page follow invalidates the turn on both classes", () => {
    const a = analyzeStream(
      [
        ev(10, { type: "thread.started" }),
        ev(20, { type: "item.completed", item: { id: "ws_1", type: "web_search", action: { type: "search", query: "a" } } }),
        ev(30, { type: "item.completed", item: { id: "ws_2", type: "web_search", action: { type: "open_page", url: "https://x.example" } } }),
        ev(40, { type: "item.completed", item: { id: "am", type: "agent_message", text: '{"answer":"a","sources":[]}' } }),
      ],
      100,
    );
    expect(isValidForClass(a, LIVE_Q, 0)).toBe(false);
  });

  test("an unparseable answer invalidates the turn", () => {
    const a = analyzeStream(
      [
        ev(10, { type: "thread.started" }),
        ev(20, { type: "item.completed", item: { id: "ws_1", type: "web_search", action: { type: "search", query: "a" } } }),
        ev(30, { type: "item.completed", item: { id: "am", type: "agent_message", text: "here is prose, not JSON" } }),
      ],
      100,
    );
    expect(isValidForClass(a, LIVE_Q, 0)).toBe(false);
  });
});

// ------------------------------------------------------------------ pairing

describe("collectValidPairs", () => {
  const good = analyzeStream(searchStream({ pass1: 3000, searchMs: 2600, pass2: 800 }), 6800);
  const fast = analyzeStream(searchStream({ pass1: 400, searchMs: 2600, pass2: 800 }), 4200);

  test("keeps a pair only when BOTH arms are valid", () => {
    const attempts = [
      attempt({ pairIndex: 0, arm: "baseline", analysis: good, valid: true }),
      attempt({ pairIndex: 0, arm: "candidate", analysis: fast, valid: true }),
      attempt({ pairIndex: 1, arm: "baseline", analysis: good, valid: true }),
      attempt({ pairIndex: 1, arm: "candidate", analysis: fast, valid: false }),
    ];
    const pairs = collectValidPairs(attempts, "live");
    expect(pairs.map((p) => p.pairIndex)).toEqual([0]);
  });

  test("a failed attempt is retained in attempts but excluded from paired stats", () => {
    const attempts = [
      attempt({ pairIndex: 0, arm: "baseline", analysis: good, valid: true }),
      attempt({ pairIndex: 0, arm: "candidate", analysis: fast, valid: false, exitCode: 1 }),
    ];
    expect(attempts.length).toBe(2);
    expect(collectValidPairs(attempts, "live").length).toBe(0);
  });

  test("does not mix query classes", () => {
    const attempts = [
      attempt({ pairIndex: 0, arm: "baseline", query: "live", analysis: good }),
      attempt({ pairIndex: 0, arm: "candidate", query: "knowledge", analysis: fast }),
    ];
    expect(collectValidPairs(attempts, "live").length).toBe(0);
  });

  test("paired delta points the right way when the candidate is faster", () => {
    const attempts = [
      attempt({ pairIndex: 0, arm: "baseline", analysis: good }),
      attempt({ pairIndex: 0, arm: "candidate", analysis: fast }),
      attempt({ pairIndex: 1, arm: "baseline", analysis: good }),
      attempt({ pairIndex: 1, arm: "candidate", analysis: fast }),
    ];
    const stat = pairedStat(collectValidPairs(attempts, "live"), "firstEventToSearchStartMs", 7);
    expect(stat.medianPairedDeltaMs).toBe(-2600);
    expect(stat.baselineMedianMs).toBe(3000);
    expect(stat.candidateMedianMs).toBe(400);
    expect(stat.candidateWins).toBe(2);
    expect(stat.percentReduction).toBeCloseTo(86.67, 1);
  });

  test("a phase that is null on either arm drops out of that phase's stat only", () => {
    const noSearch = analyzeStream(noSearchStream(4400), 5400);
    const attempts = [
      attempt({ pairIndex: 0, arm: "baseline", analysis: good }),
      attempt({ pairIndex: 0, arm: "candidate", analysis: noSearch }),
    ];
    const pairs = collectValidPairs(attempts, "live");
    expect(pairedStat(pairs, "firstEventToSearchStartMs", 7).n).toBe(0);
    expect(pairedStat(pairs, "totalMs", 7).n).toBe(1);
  });
});

// ------------------------------------------------------------------- bootstrap

describe("bootstrapMedianDeltaCi95", () => {
  const deltas = [-1200, -900, -1400, -700, -1100, -1300, -800, -1000, -950, -1250, -1050, -880, -1150, -990, -1320];

  test("is deterministic for a given seed", () => {
    const a = bootstrapMedianDeltaCi95(deltas, 20260725, 2000);
    const b = bootstrapMedianDeltaCi95(deltas, 20260725, 2000);
    expect(a).toEqual(b);
  });

  test("a different seed gives a different but similar interval", () => {
    const a = bootstrapMedianDeltaCi95(deltas, 1, 2000);
    const b = bootstrapMedianDeltaCi95(deltas, 2, 2000);
    expect(Number.isNaN(a.upper)).toBe(false);
    expect(Math.abs(a.upper - b.upper)).toBeLessThan(400);
  });

  test("a consistently negative delta set puts the CI upper bound below zero", () => {
    expect(bootstrapMedianDeltaCi95(deltas, 20260725, 4000).upper).toBeLessThan(0);
  });

  test("a delta set straddling zero does NOT put the upper bound below zero", () => {
    const mixed = [-100, 200, -50, 300, -20, 150, 80, -300, 40, 90, -10, 210, -70, 120, 30];
    expect(bootstrapMedianDeltaCi95(mixed, 20260725, 4000).upper).toBeGreaterThanOrEqual(0);
  });

  test("an empty delta set is NaN, so a zero-pair run cannot pass the CI gate", () => {
    const ci = bootstrapMedianDeltaCi95([], 1, 100);
    expect(Number.isNaN(ci.upper)).toBe(true);
  });
});

// -------------------------------------------------------------- source contract

describe("source contract extraction", () => {
  test("scrapes every production --config flag including escaped-quote values", () => {
    const flags = scrapeConfigFlags(
      readFileSync(resolve(import.meta.dir, "../../src/ai/agent_chat/codex_exec.rs"), "utf8"),
    );
    expect(flags).toContain("skills.bundled.enabled=false");
    expect(flags).toContain('model_reasoning_effort="low"');
    expect(flags).toContain('tools.web_search.context_size="low"');
    expect(flags).toContain("features.shell_tool=false");
    expect(flags).toContain("mcp_servers={}");
    expect(flags).toContain("features.enable_mcp_apps=false");
    expect(flags).toContain("features.apps=false");
    expect(flags).toContain("features.tool_search=false");
    // The deprecated connectors flag is deliberately absent (Codex answers it
    // with a deprecation error item).
    expect(flags.some((f) => f.includes("connectors"))).toBe(false);
  });

  test("loads the worktree contract with real hashes", () => {
    const c = loadContract({ kind: "worktree" });
    expect(c.model.length).toBeGreaterThan(0);
    expect(c.basePrompt.length).toBeGreaterThan(200);
    expect(c.basePromptSha256).toBe(sha256(c.basePrompt));
    expect(c.configFlags.length).toBeGreaterThanOrEqual(8);
  });

  test("refToSource treats WORKTREE and a missing ref as the worktree", () => {
    expect(refToSource(null)).toEqual({ kind: "worktree" });
    expect(refToSource("WORKTREE")).toEqual({ kind: "worktree" });
    expect(refToSource("29dc1658")).toEqual({ kind: "git", ref: "29dc1658" });
  });
});

// ---------------------------------------------------------------- prompt/command

const FAKE: QuickAiCommandContract = {
  source: "test",
  model: "gpt-5.3-codex-spark",
  outputSchema: '{"type":"object"}',
  outputSchemaSha256: sha256('{"type":"object"}'),
  configFlags: ["features.shell_tool=false", "mcp_servers={}"],
  basePrompt: "BASE PROMPT",
  basePromptSha256: sha256("BASE PROMPT"),
  forceSuffix: "FORCE SUFFIX",
  forceSuffixSha256: sha256("FORCE SUFFIX"),
};

describe("effectivePrompt / buildCommand", () => {
  test("ModelDecides is byte-identical to the base prompt", () => {
    expect(effectivePrompt(FAKE, false)).toBe("BASE PROMPT");
  });

  test("forced appends the suffix after a blank line, keeping base as a prefix", () => {
    const p = effectivePrompt(FAKE, true);
    expect(p).toBe("BASE PROMPT\n\nFORCE SUFFIX");
    expect(p.startsWith(FAKE.basePrompt)).toBe(true);
  });

  test("a contract without the suffix cannot be forced — the frozen baseline arm", () => {
    const noSuffix = { ...FAKE, forceSuffix: null, forceSuffixSha256: null };
    expect(effectivePrompt(noSuffix, true)).toBe("BASE PROMPT");
    const built = buildCommand(noSuffix, "q", "/tmp/s", "/tmp/s/schema.json", true);
    expect(built.forcedPromptApplied).toBe(false);
    expect(built.promptSha256).toBe(noSuffix.basePromptSha256);
  });

  test("forcing changes ONLY the developer_instructions argument value", () => {
    const base = buildCommand(FAKE, "q", "/tmp/s", "/tmp/s/schema.json", false);
    const forced = buildCommand(FAKE, "q", "/tmp/s", "/tmp/s/schema.json", true);
    expect(base.argv.length).toBe(forced.argv.length);
    const diffs = base.argv
      .map((a, i) => (a === forced.argv[i] ? null : i))
      .filter((i): i is number => i !== null);
    expect(diffs.length).toBe(1);
    expect(base.argv[diffs[0]].startsWith("developer_instructions=")).toBe(true);
    expect(base.commandSha256).not.toBe(forced.commandSha256);
  });

  test("the raw user query is passed through unmodified as the final argument", () => {
    const built = buildCommand(FAKE, "Did LeBron join a team yet?", "/tmp/s", "/tmp/s/schema.json", true);
    expect(built.argv[built.argv.length - 1]).toBe("Did LeBron join a team yet?");
  });

  test("the command hash elides the scratch path so two arms can be compared", () => {
    const a = buildCommand(FAKE, "q", "/tmp/one", "/tmp/one/schema.json", false);
    const b = buildCommand(FAKE, "q", "/tmp/two", "/tmp/two/schema.json", false);
    expect(a.commandSha256).toBe(b.commandSha256);
  });
});

// ----------------------------------------------------------------------- gates

describe("knowledge holdout gates", () => {
  const noSearch = analyzeStream(noSearchStream(4400), 5400);
  const pairs = Array.from({ length: 15 }, (_, i) => ({
    pairIndex: i,
    baseline: attempt({ pairIndex: i, arm: "baseline" as const, query: "knowledge", analysis: noSearch, promptSha256: FAKE.basePromptSha256, commandSha256: "same" }),
    candidate: attempt({ pairIndex: i, arm: "candidate" as const, query: "knowledge", analysis: noSearch, promptSha256: FAKE.basePromptSha256, commandSha256: "same" }),
  }));

  test("an unchanged knowledge path passes every holdout gate", () => {
    const gates = evaluateKnowledgeGates(pairs, FAKE, FAKE, 20260725);
    const failed = gates.filter((g) => !g.pass);
    expect(failed.map((g) => `${g.id}: ${g.detail}`)).toEqual([]);
  });

  test("a knowledge turn that searched fails the zero-search gate", () => {
    const searched = analyzeStream(searchStream({ pass1: 100, searchMs: 100, pass2: 100 }), 1000);
    const bad = pairs.map((p) => ({
      ...p,
      candidate: { ...p.candidate, analysis: searched },
    }));
    const gates = evaluateKnowledgeGates(bad, FAKE, FAKE, 20260725);
    expect(gates.find((g) => g.id === "zeroSearchesEveryRun")?.pass).toBe(false);
  });

  test("a changed knowledge command fails the identical-hash gate", () => {
    const bad = pairs.map((p) => ({ ...p, candidate: { ...p.candidate, commandSha256: "different" } }));
    const gates = evaluateKnowledgeGates(bad, FAKE, FAKE, 20260725);
    expect(gates.find((g) => g.id === "commandHashesIdentical")?.pass).toBe(false);
  });

  test("a forced prompt on the knowledge path fails the base-prompt-hash gate", () => {
    const bad = pairs.map((p) => ({ ...p, candidate: { ...p.candidate, promptSha256: sha256("BASE PROMPT\n\nFORCE SUFFIX") } }));
    const gates = evaluateKnowledgeGates(bad, FAKE, FAKE, 20260725);
    expect(gates.find((g) => g.id === "candidateUsesBasePromptHash")?.pass).toBe(false);
  });
});

// ------------------------------------------------------------------------- cli

describe("parseCli", () => {
  test("defaults to single mode with 6 reps", () => {
    const cli = parseCli([]);
    expect(cli.mode).toBe("single");
    expect(cli.reps).toBe(6);
  });

  test("paired mode defaults to 15 reps and 24 max attempts", () => {
    const cli = parseCli(["--mode", "paired"]);
    expect(cli.reps).toBe(15);
    expect(cli.maxAttempts).toBe(24);
  });

  test("rejects an unknown mode instead of silently running single", () => {
    expect(() => parseCli(["--mode", "bogus"])).toThrow();
  });

  test("recognizes safe help and contract inspection without treating either as a live run", () => {
    expect(parseCli(["--help"]).help).toBe(true);
    expect(parseCli(["-h"]).help).toBe(true);
    expect(parseCli(["--describe-contract"]).describeContract).toBe(true);
  });

  test("unknown switches, missing values, and invalid counts fail closed", () => {
    expect(() => parseCli(["--future-live-mode"])).toThrow("unknown option");
    expect(() => parseCli(["--mode"])).toThrow("requires a value");
    expect(() => parseCli(["--reps", "0"])).toThrow("positive integer");
    expect(() => parseCli(["--max-attempts", "nope"])).toThrow("positive integer");
    expect(() => parseCli(["--seed", "-1"])).toThrow();
  });

  test("reads refs, seed, and out path", () => {
    const cli = parseCli(["--mode", "paired", "--baseline-ref", "abc123", "--candidate-ref", "WORKTREE", "--seed", "7", "--out", "/tmp/x.json"]);
    expect(cli.baselineRef).toBe("abc123");
    expect(cli.candidateRef).toBe("WORKTREE");
    expect(cli.seed).toBe(7);
    expect(cli.out).toBe("/tmp/x.json");
  });
});

describe("Quick AI benchmark provider-call safety", () => {
  function invoke(
    args: string[],
    extraEnvironment: Record<string, string> = {},
  ) {
    const environment = {
      ...process.env,
      SCRIPT_KIT_ALLOW_LIVE_AI: "0",
      SCRIPT_KIT_NONINTERACTIVE: "0",
      ...extraEnvironment,
    };
    return Bun.spawnSync(
      [process.execPath, "scripts/agentic/quick-ai-latency-bench.ts", ...args],
      {
        cwd: resolve(import.meta.dir, "../.."),
        env: environment,
        stdout: "pipe",
        stderr: "pipe",
      },
    );
  }

  test("help cannot fall through to the six-repetition paid benchmark", () => {
    const result = invoke(["--help"]);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("Safe inspection");
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_ALLOW_LIVE_AI=1");
    expect(result.stderr.toString()).not.toContain("[bench] mode=");
  });

  test("contract inspection is static, network-free, and never claims paint", () => {
    const result = invoke(["--describe-contract"]);
    expect(result.exitCode).toBe(0);
    const contract = JSON.parse(result.stdout.toString());
    expect(contract.evidenceClass).toBe("STATIC_INVENTORY");
    expect(contract.runtimeEvidenceClass).toBe("LIVE_AI");
    expect(contract.metricKind).toBe("quick_ai_provider_event_phases");
    expect(contract.observationClass).toBe("PROVIDER_EVENT_STREAM");
    expect(contract.measuresPaint).toBe(false);
    expect(contract.safety.startsProviderProcess).toBe(false);
    expect(contract.safety.makesNetworkRequest).toBe(false);
  });

  test("default invocation refuses before provider startup without explicit approval", () => {
    const result = invoke([]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("refused before provider startup");
    expect(result.stderr.toString()).not.toContain("[bench] mode=");
  });

  test("noninteractive mode refuses provider calls even if live AI was opted in", () => {
    const result = invoke(["--mode", "single", "--reps", "1"], {
      SCRIPT_KIT_ALLOW_LIVE_AI: "1",
      SCRIPT_KIT_NONINTERACTIVE: "1",
    });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("categorically refuses live Quick AI");
    expect(result.stderr.toString()).not.toContain("[bench] mode=");
  });

  test("print-command remains safe under noninteractive mode", () => {
    const result = invoke(["--print-command"], {
      SCRIPT_KIT_NONINTERACTIVE: "1",
    });
    expect(result.exitCode).toBe(0);
    const command = JSON.parse(result.stdout.toString());
    expect(command.command[0]).toBe("codex");
    expect(command.commandSha256).toHaveLength(64);
  });
});
