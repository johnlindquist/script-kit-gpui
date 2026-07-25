/**
 * Quick AI latency benchmark — the paired, phase-aware before/after instrument.
 *
 * WHY PAIRED: end-to-end totals on this path are noise-dominated (identical
 * configs have ranged 4.3-15.6s). A sequential "run before, then run after"
 * comparison cannot separate a real improvement from provider drift. So every
 * trial is a PAIR: one baseline turn and one candidate turn, back to back, in
 * deterministically alternating AB/BA order. The statistic of record is the
 * median of the per-pair delta, plus its bootstrap CI.
 *
 * WHY GIT-REF CONTRACTS: the harness never hard-codes the production prompt,
 * schema, model, or `--config` flags. It extracts them from
 * `src/ai/agent_chat/codex_exec.rs` + `src/ai/agent_chat/profiles.rs`, either
 * from the worktree or from an immutable git ref (`git show <ref>:<path>`), so
 * the baseline arm is provably the pre-change product command and the harness
 * cannot drift from what ships.
 *
 * Phases timed from the `codex exec --json` NDJSON event stream:
 *   spawn -> first event                       process + boot
 *   first event -> web_search item.started     INFERENCE PASS 1 ("should I search?")
 *   web_search started -> completed            search execution
 *   search completed -> agent_message          INFERENCE PASS 2 (generation)
 *
 * Usage:
 *   bun scripts/agentic/quick-ai-latency-bench.ts --print-command
 *   bun scripts/agentic/quick-ai-latency-bench.ts --mode single --reps 6
 *   bun scripts/agentic/quick-ai-latency-bench.ts --mode aa \
 *       --baseline-ref "$BASE_HEAD" --candidate-ref "$BASE_HEAD" --reps 6
 *   bun scripts/agentic/quick-ai-latency-bench.ts --mode paired \
 *       --baseline-ref "$BASE_HEAD" --candidate-ref WORKTREE \
 *       --reps 15 --max-attempts 24 --seed 20260725 \
 *       --out .test-output/quick-ai-spec-search/paired-before-after.json
 */
import { readFileSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { execFileSync } from "node:child_process";

export const ROOT = resolve(import.meta.dir, "../..");

const EXEC_REL = "src/ai/agent_chat/codex_exec.rs";
const PROF_REL = "src/ai/agent_chat/profiles.rs";

/** Item types `parse_item` maps to `CodexItem::Forbidden` (codex_exec.rs). */
export const FORBIDDEN_ITEM_TYPES = new Set([
  "command_execution",
  "file_change",
  "mcp_tool_call",
  "collab_tool_call",
  "image_view",
  "dynamic_tool_call",
]);

/** Web actions the production budget treats as a page follow (observe_web_item). */
export const PAGE_FOLLOW_ACTIONS = new Set(["open_page", "find_in_page"]);

// -------------------------------------------------------------- source contract

export type SourceRef = { kind: "worktree" } | { kind: "git"; ref: string };

export type QuickAiCommandContract = {
  /** "worktree" or "git:<ref>" — the arm label's provenance. */
  source: string;
  model: string;
  outputSchema: string;
  outputSchemaSha256: string;
  configFlags: string[];
  basePrompt: string;
  basePromptSha256: string;
  /** Present only once package 2 lands; absent on the frozen baseline ref. */
  forceSuffix: string | null;
  forceSuffixSha256: string | null;
};

export function sha256(text: string): string {
  return new Bun.CryptoHasher("sha256").update(text, "utf8").digest("hex");
}

export function describeSource(source: SourceRef): string {
  return source.kind === "worktree" ? "worktree" : `git:${source.ref}`;
}

export function readSource(source: SourceRef, relPath: string): string {
  if (source.kind === "worktree") return readFileSync(join(ROOT, relPath), "utf8");
  return execFileSync("git", ["show", `${source.ref}:${relPath}`], {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
}

/** Extract a `NAME: &str = "…"` Rust escaped string literal. */
export function rustStr(src: string, constName: string): string {
  const m = new RegExp(`${constName}: &str = "((?:[^"\\\\]|\\\\.)*)"`, "s").exec(src);
  if (!m) throw new Error(`could not extract ${constName}`);
  // Rust allows a trailing `\` + newline line continuation; JSON does not.
  return JSON.parse(`"${m[1].replace(/\\\n\s*/g, "")}"`);
}

export function rustStrOptional(src: string, constName: string): string | null {
  try {
    return rustStr(src, constName);
  } catch {
    return null;
  }
}

/** Extract a `NAME: &str = r#"…"#` Rust raw string literal. */
export function rustRawStr(src: string, constName: string): string {
  const m = new RegExp(`${constName}: &str = r#"(.*?)"#`, "s").exec(src);
  if (!m) throw new Error(`could not extract raw ${constName}`);
  return m[1];
}

/**
 * Every `--config KEY=VALUE` pair in build_codex_exec_command, scraped from the
 * `.arg("--config").arg("…")` chain so new flags are picked up automatically.
 * `developer_instructions` is added separately (it is a runtime `format!`).
 * Values may contain escaped quotes: `.arg("model_reasoning_effort=\"low\"")`.
 */
export function scrapeConfigFlags(execSrc: string): string[] {
  const fn = /pub\(crate\) fn build_codex_exec_command[\s\S]*?\n}/.exec(execSrc)?.[0] ?? "";
  if (!fn) throw new Error("could not locate build_codex_exec_command");
  const out: string[] = [];
  const re = /\.arg\("--config"\)\s*\n\s*\.arg\("((?:[^"\\]|\\.)*)"\)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(fn)) !== null) out.push(m[1].replace(/\\"/g, '"'));
  if (out.length === 0) throw new Error("scraped zero --config flags");
  return out;
}

export function loadContract(source: SourceRef): QuickAiCommandContract {
  const execSrc = readSource(source, EXEC_REL);
  const profSrc = readSource(source, PROF_REL);
  const model = /QUICK_AI_PI_MODEL: &str = "([^"]+)"/.exec(profSrc)?.[1];
  if (!model) throw new Error("could not extract QUICK_AI_PI_MODEL");
  const basePrompt = rustStr(profSrc, "QUICK_AI_APPEND_SYSTEM_PROMPT");
  const outputSchema = rustRawStr(execSrc, "QUICK_AI_OUTPUT_SCHEMA");
  const forceSuffix = rustStrOptional(profSrc, "QUICK_AI_FORCE_FIRST_SEARCH_SUFFIX");
  return {
    source: describeSource(source),
    model,
    outputSchema,
    outputSchemaSha256: sha256(outputSchema),
    configFlags: scrapeConfigFlags(execSrc),
    basePrompt,
    basePromptSha256: sha256(basePrompt),
    forceSuffix,
    forceSuffixSha256: forceSuffix === null ? null : sha256(forceSuffix),
  };
}

/**
 * Mirrors `quick_ai_developer_instructions`: ModelDecides returns the base
 * prompt byte-for-byte; ForceFirstNativeSearch appends the suffix after a blank
 * line. An arm whose contract has no suffix always uses the base prompt, which
 * is how the frozen baseline ref stays byte-identical to shipped HEAD.
 */
export function effectivePrompt(contract: QuickAiCommandContract, forced: boolean): string {
  if (!forced || contract.forceSuffix === null) return contract.basePrompt;
  return `${contract.basePrompt}\n\n${contract.forceSuffix}`;
}

export type BuiltCommand = {
  argv: string[];
  prompt: string;
  promptSha256: string;
  /** Hash of the whole arg vector with the scratch path elided — arm equality proof. */
  commandSha256: string;
  forcedPromptApplied: boolean;
};

export function buildCommand(
  contract: QuickAiCommandContract,
  query: string,
  scratchDir: string,
  schemaPath: string,
  forced: boolean,
): BuiltCommand {
  const prompt = effectivePrompt(contract, forced);
  const argv = [
    "codex",
    "--search",
    "--model",
    contract.model,
    "--sandbox",
    "read-only",
    "--cd",
    scratchDir,
    "--disable",
    "plugins",
  ];
  for (const f of contract.configFlags) argv.push("--config", f);
  argv.push("--config", `developer_instructions=${JSON.stringify(prompt)}`);
  argv.push(
    "exec",
    "--ephemeral",
    "--ignore-user-config",
    "--ignore-rules",
    "--skip-git-repo-check",
    "--output-schema",
    schemaPath,
    "--json",
    query,
  );
  const canonical = argv.map((a) => (a === scratchDir || a === schemaPath ? "<SCRATCH>" : a));
  return {
    argv,
    prompt,
    promptSha256: sha256(prompt),
    commandSha256: sha256(JSON.stringify(canonical)),
    forcedPromptApplied: forced && contract.forceSuffix !== null,
  };
}

// ------------------------------------------------------------------- query set
// Two classes, because 0f42c5930 made the no-search path legal: a live-fact
// question that must search, and a knowledge question that must not.
//
// `forced` declares which class the Rust classifier is contracted to force.
// The Rust classification matrix in `quick_ai_search_plan.rs` is the authority
// that the classifier agrees with these declarations; the runtime traces in
// `quick-ai-fastest-search-probe.ts` prove it in the real app. This harness
// only needs the declaration to build the two arms' commands.
export type BenchQuery = {
  id: string;
  text: string;
  needsSearch: boolean;
  forced: boolean;
};

export const QUERIES: BenchQuery[] = [
  { id: "live", text: "Did LeBron join a team yet?", needsSearch: true, forced: true },
  { id: "knowledge", text: "What does the Rust ? operator do?", needsSearch: false, forced: false },
];

/**
 * The measured pre-change baseline on 29dc1658 (2026-07-25, n=6). Metadata for
 * the receipt only — NEVER an input to candidate statistics.
 */
export const REFERENCE_BASELINE = {
  head: "29dc1658afb80467d0a9c087298b7728983746d7",
  measuredAt: "2026-07-25",
  liveTotalMedianMs: 7201,
  livePass1MedianMs: 3006,
  liveSearchMedianMs: 2666,
  livePass2MedianMs: 846,
  knowledgeTotalMedianMs: 5604,
} as const;

// ------------------------------------------------------------- stream analysis

export type PhaseTimes = {
  spawnToFirstEventMs: number | null;
  firstEventToSearchStartMs: number | null;
  searchStartToSearchCompleteMs: number | null;
  searchCompleteToAnswerMs: number | null;
  totalMs: number;
};

export type StreamAnalysis = {
  phases: PhaseTimes;
  distinctSearchItemCount: number;
  searchActionCount: number;
  pageFollowActionCount: number;
  forbiddenItemCount: number;
  eventsBeforeFirstSearch: number;
  completedAgentMessageCount: number;
  answerText: string | null;
  structuredAnswerParsed: boolean;
  structuredSourceCount: number | null;
  turnFailed: boolean;
  parseErrorCount: number;
};

export type TimedLine = { atMs: number; line: string };

/**
 * Structural NDJSON analysis. Deliberately not `line.includes(...)`: the old
 * substring form miscounted a search per lifecycle event and could not see
 * action types or item ids at all.
 */
export function analyzeStream(lines: TimedLine[], totalMs: number): StreamAnalysis {
  let firstEventMs: number | null = null;
  let searchStartMs: number | null = null;
  let searchCompleteMs: number | null = null;
  let answerMs: number | null = null;
  const searchItemIds = new Set<string>();
  let searchActionCount = 0;
  let pageFollowActionCount = 0;
  let forbiddenItemCount = 0;
  let eventsBeforeFirstSearch = 0;
  const agentMessageIds = new Set<string>();
  let answerText: string | null = null;
  let turnFailed = false;
  let parseErrorCount = 0;

  for (const { atMs, line } of lines) {
    if (!line.trim()) continue;
    let event: any;
    try {
      event = JSON.parse(line);
    } catch {
      parseErrorCount += 1;
      continue;
    }
    if (firstEventMs === null) firstEventMs = atMs;
    // Counted at the END of the iteration so the search event that terminates
    // the deliberation is not itself counted as part of it.
    const searchAlreadySeen = searchStartMs !== null;

    const type = typeof event?.type === "string" ? event.type : "";
    if (type === "turn.failed" || type === "error") turnFailed = true;

    const item = event?.item;
    const itemType = typeof item?.type === "string" ? item.type : "";
    const itemId = typeof item?.id === "string" ? item.id : "";

    if (itemType === "web_search") {
      if (itemId) searchItemIds.add(itemId);
      if (searchStartMs === null) searchStartMs = atMs;
      const actionType = typeof item?.action?.type === "string" ? item.action.type : "other";
      if (type === "item.completed") {
        if (actionType === "search") searchActionCount += 1;
        if (PAGE_FOLLOW_ACTIONS.has(actionType)) pageFollowActionCount += 1;
        if (searchCompleteMs === null) searchCompleteMs = atMs;
      }
    } else if (FORBIDDEN_ITEM_TYPES.has(itemType)) {
      forbiddenItemCount += 1;
    } else if (
      (itemType === "agent_message" || itemType === "assistant_message") &&
      type === "item.completed"
    ) {
      const text = typeof item?.text === "string" ? item.text : "";
      if (text.trim() && itemId) {
        if (agentMessageIds.size === 0) {
          answerMs = atMs;
          answerText = text;
        }
        agentMessageIds.add(itemId);
      }
    }

    if (!searchAlreadySeen && searchStartMs === null) eventsBeforeFirstSearch += 1;
  }

  let structuredAnswerParsed = false;
  let structuredSourceCount: number | null = null;
  if (answerText !== null) {
    try {
      const parsed = JSON.parse(answerText);
      if (parsed && typeof parsed === "object") {
        structuredAnswerParsed = true;
        structuredSourceCount = Array.isArray(parsed.sources) ? parsed.sources.length : 0;
      }
    } catch {
      structuredAnswerParsed = false;
    }
  }

  const sub = (a: number | null, b: number | null) =>
    a !== null && b !== null ? a - b : null;

  return {
    phases: {
      spawnToFirstEventMs: firstEventMs,
      firstEventToSearchStartMs: sub(searchStartMs, firstEventMs),
      searchStartToSearchCompleteMs: sub(searchCompleteMs, searchStartMs),
      searchCompleteToAnswerMs: sub(answerMs, searchCompleteMs ?? firstEventMs),
      totalMs,
    },
    distinctSearchItemCount: searchItemIds.size,
    searchActionCount,
    pageFollowActionCount,
    forbiddenItemCount,
    eventsBeforeFirstSearch,
    completedAgentMessageCount: agentMessageIds.size,
    answerText,
    structuredAnswerParsed,
    structuredSourceCount,
    turnFailed,
    parseErrorCount,
  };
}

/**
 * A turn is VALID for its class when it produced a parseable structured answer,
 * did not fail, and honoured the class's search contract. A successful but very
 * slow turn is valid and MUST remain in the statistics — no outlier trimming.
 */
export function isValidForClass(a: StreamAnalysis, query: BenchQuery, exitCode: number): boolean {
  if (exitCode !== 0) return false;
  if (a.turnFailed) return false;
  if (!a.structuredAnswerParsed) return false;
  if (a.forbiddenItemCount !== 0) return false;
  if (a.pageFollowActionCount !== 0) return false;
  if (query.needsSearch) {
    if (a.distinctSearchItemCount !== 1) return false;
    if (a.searchActionCount !== 1) return false;
    if (a.phases.searchStartToSearchCompleteMs === null) return false;
  } else if (a.distinctSearchItemCount !== 0) {
    return false;
  }
  return true;
}

// -------------------------------------------------------------------- statistics

export function median(values: number[]): number {
  if (values.length === 0) return NaN;
  const s = [...values].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 === 1 ? s[m] : (s[m - 1] + s[m]) / 2;
}

export function medianPairedDelta(deltas: number[]): number {
  return median(deltas);
}

export function percentReduction(baselineMedian: number, candidateMedian: number): number {
  if (!Number.isFinite(baselineMedian) || baselineMedian === 0) return NaN;
  return ((baselineMedian - candidateMedian) / baselineMedian) * 100;
}

/** Candidate wins a pair when its delta (candidate - baseline) is negative. */
export function winCount(deltas: number[]): number {
  return deltas.filter((d) => d < 0).length;
}

/** Deterministic 32-bit PRNG (mulberry32) so bootstrap output is reproducible. */
export function makeRng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export type Ci95 = { lower: number; upper: number; iterations: number; seed: number };

export function bootstrapMedianDeltaCi95(
  deltas: number[],
  seed: number,
  iterations = 10_000,
): Ci95 {
  if (deltas.length === 0) {
    return { lower: NaN, upper: NaN, iterations, seed };
  }
  const rng = makeRng(seed);
  const medians: number[] = [];
  const n = deltas.length;
  for (let i = 0; i < iterations; i += 1) {
    const sample: number[] = new Array(n);
    for (let j = 0; j < n; j += 1) sample[j] = deltas[Math.floor(rng() * n)];
    medians.push(median(sample));
  }
  medians.sort((a, b) => a - b);
  const at = (p: number) => medians[Math.min(medians.length - 1, Math.max(0, Math.floor(p * medians.length)))];
  return { lower: at(0.025), upper: at(0.975), iterations, seed };
}

/**
 * Deterministic balanced order: even pairs run baseline-first, odd pairs run
 * candidate-first. A seed may flip which parity starts, but the sequence stays
 * balanced so provider warm-up cannot systematically favour one arm.
 */
export function pairOrder(pairIndex: number, seed: number): ["baseline" | "candidate", "baseline" | "candidate"] {
  const flip = (seed >>> 0) % 2 === 1;
  const baselineFirst = (pairIndex % 2 === 0) !== flip;
  return baselineFirst ? ["baseline", "candidate"] : ["candidate", "baseline"];
}

// ----------------------------------------------------------------- measurement

export type Attempt = {
  pairIndex: number;
  arm: "baseline" | "candidate";
  query: string;
  contractSource: string;
  promptSha256: string;
  commandSha256: string;
  forcedPromptApplied: boolean;
  exitCode: number;
  analysis: StreamAnalysis;
  valid: boolean;
  stderrTail: string;
};

export type MeasureDeps = {
  spawn: (argv: string[]) => {
    stdout: ReadableStream<Uint8Array>;
    stderr: ReadableStream<Uint8Array>;
    exited: Promise<number>;
  };
  now: () => number;
};

async function drain(stream: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stream.getReader();
  const dec = new TextDecoder();
  let out = "";
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    out += dec.decode(value, { stream: true });
  }
  return out;
}

export async function measureOnce(
  contract: QuickAiCommandContract,
  query: BenchQuery,
  arm: "baseline" | "candidate",
  pairIndex: number,
  scratchRoot: string,
  deps: MeasureDeps,
): Promise<Attempt> {
  // Unique scratch dir per attempt so neither arm can reuse the other's state.
  const scratchDir = join(scratchRoot, `${arm}-${query.id}-p${pairIndex}`);
  mkdirSync(scratchDir, { recursive: true });
  const schemaPath = join(scratchDir, "quick-ai-output-schema.json");
  writeFileSync(schemaPath, contract.outputSchema);

  const built = buildCommand(contract, query.text, scratchDir, schemaPath, query.forced);
  const t0 = deps.now();
  const proc = deps.spawn(built.argv);
  const stderrPromise = drain(proc.stderr);

  const lines: TimedLine[] = [];
  const reader = proc.stdout.getReader();
  const dec = new TextDecoder();
  let buf = "";
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buf += dec.decode(value, { stream: true });
    let nl: number;
    while ((nl = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, nl);
      buf = buf.slice(nl + 1);
      if (line.trim()) lines.push({ atMs: deps.now() - t0, line });
    }
  }
  if (buf.trim()) lines.push({ atMs: deps.now() - t0, line: buf });

  const exitCode = await proc.exited;
  const totalMs = deps.now() - t0;
  const stderrText = await stderrPromise;
  const analysis = analyzeStream(lines, totalMs);
  rmSync(scratchDir, { recursive: true, force: true });

  return {
    pairIndex,
    arm,
    query: query.id,
    contractSource: contract.source,
    promptSha256: built.promptSha256,
    commandSha256: built.commandSha256,
    forcedPromptApplied: built.forcedPromptApplied,
    exitCode,
    analysis,
    valid: isValidForClass(analysis, query, exitCode),
    stderrTail: stderrText.slice(-500),
  };
}

// ------------------------------------------------------------------ aggregation

export type PhaseKey = keyof Omit<PhaseTimes, never>;

export const PHASE_KEYS: PhaseKey[] = [
  "spawnToFirstEventMs",
  "firstEventToSearchStartMs",
  "searchStartToSearchCompleteMs",
  "searchCompleteToAnswerMs",
  "totalMs",
];

export type PairedStat = {
  n: number;
  baselineMedianMs: number;
  candidateMedianMs: number;
  medianPairedDeltaMs: number;
  percentReduction: number;
  ci95: Ci95;
  candidateWins: number;
};

export type ValidPair = { pairIndex: number; baseline: Attempt; candidate: Attempt };

/**
 * A pair contributes to the statistics only when BOTH arms produced a valid
 * turn — that is what makes the delta a like-for-like comparison. Invalid
 * attempts stay in `attempts` for the receipt; they are never silently dropped.
 */
export function collectValidPairs(attempts: Attempt[], queryId: string): ValidPair[] {
  const byPair = new Map<number, { baseline?: Attempt; candidate?: Attempt }>();
  for (const a of attempts) {
    if (a.query !== queryId) continue;
    const slot = byPair.get(a.pairIndex) ?? {};
    slot[a.arm] = a;
    byPair.set(a.pairIndex, slot);
  }
  const out: ValidPair[] = [];
  for (const [pairIndex, slot] of [...byPair.entries()].sort((a, b) => a[0] - b[0])) {
    if (slot.baseline?.valid && slot.candidate?.valid) {
      out.push({ pairIndex, baseline: slot.baseline, candidate: slot.candidate });
    }
  }
  return out;
}

export function pairedStat(pairs: ValidPair[], phase: PhaseKey, seed: number): PairedStat {
  const usable = pairs.filter(
    (p) =>
      p.baseline.analysis.phases[phase] !== null && p.candidate.analysis.phases[phase] !== null,
  );
  const baselineValues = usable.map((p) => p.baseline.analysis.phases[phase] as number);
  const candidateValues = usable.map((p) => p.candidate.analysis.phases[phase] as number);
  const deltas = usable.map(
    (p) =>
      (p.candidate.analysis.phases[phase] as number) -
      (p.baseline.analysis.phases[phase] as number),
  );
  const bm = median(baselineValues);
  const cm = median(candidateValues);
  return {
    n: usable.length,
    baselineMedianMs: Math.round(bm),
    candidateMedianMs: Math.round(cm),
    medianPairedDeltaMs: Math.round(medianPairedDelta(deltas)),
    percentReduction: Number(percentReduction(bm, cm).toFixed(2)),
    ci95: bootstrapMedianDeltaCi95(deltas, seed),
    candidateWins: winCount(deltas),
  };
}

// -------------------------------------------------------------- acceptance gate

export type GateResult = { id: string; pass: boolean; detail: string };

/** The statistical acceptance bar Oracle set for the live query, verbatim. */
export function evaluateLiveGates(pairs: ValidPair[], seed: number): GateResult[] {
  const pass1 = pairedStat(pairs, "firstEventToSearchStartMs", seed);
  const total = pairedStat(pairs, "totalMs", seed);
  const searchExec = pairedStat(pairs, "searchStartToSearchCompleteMs", seed);
  const pass2 = pairedStat(pairs, "searchCompleteToAnswerMs", seed);
  const g = (id: string, pass: boolean, detail: string): GateResult => ({ id, pass, detail });
  const regressionBound = (baseline: number, absMs: number, pct: number) =>
    Math.max(absMs, (baseline * pct) / 100);
  return [
    g("validPairs>=15", pairs.length >= 15, `validPairs=${pairs.length}`),
    g(
      "exactlyOneSearchBothArms",
      pairs.every(
        (p) =>
          p.baseline.analysis.distinctSearchItemCount === 1 &&
          p.candidate.analysis.distinctSearchItemCount === 1,
      ),
      "distinctSearchItemCount==1 on every valid turn",
    ),
    g(
      "noPageFollowExcessOrForbidden",
      pairs.every(
        (p) =>
          p.baseline.analysis.pageFollowActionCount === 0 &&
          p.candidate.analysis.pageFollowActionCount === 0 &&
          p.baseline.analysis.forbiddenItemCount === 0 &&
          p.candidate.analysis.forbiddenItemCount === 0 &&
          p.baseline.analysis.searchActionCount === 1 &&
          p.candidate.analysis.searchActionCount === 1,
      ),
      "no page follow, excess search, or forbidden item",
    ),
    g("pass1Reduction>=25pct", pass1.percentReduction >= 25, `pass1 reduction=${pass1.percentReduction}%`),
    g(
      "pass1Absolute>=750ms",
      pass1.baselineMedianMs - pass1.candidateMedianMs >= 750,
      `pass1 median ${pass1.baselineMedianMs} -> ${pass1.candidateMedianMs}`,
    ),
    g("pass1PairedDelta<=-750ms", pass1.medianPairedDeltaMs <= -750, `pass1 paired delta=${pass1.medianPairedDeltaMs}ms`),
    g("pass1Ci95Upper<0", pass1.ci95.upper < 0, `pass1 CI95 upper=${Math.round(pass1.ci95.upper)}ms`),
    g("pass1Wins>=11of15", pass1.candidateWins >= 11, `pass1 wins=${pass1.candidateWins}/${pass1.n}`),
    g("totalReduction>=10pct", total.percentReduction >= 10, `total reduction=${total.percentReduction}%`),
    g(
      "totalAbsolute>=700ms",
      total.baselineMedianMs - total.candidateMedianMs >= 700,
      `total median ${total.baselineMedianMs} -> ${total.candidateMedianMs}`,
    ),
    g("totalCi95Upper<0", total.ci95.upper < 0, `total CI95 upper=${Math.round(total.ci95.upper)}ms`),
    g("totalWins>=9of15", total.candidateWins >= 9, `total wins=${total.candidateWins}/${total.n}`),
    g(
      "searchExecNoRegression",
      searchExec.candidateMedianMs - searchExec.baselineMedianMs <=
        regressionBound(searchExec.baselineMedianMs, 300, 15),
      `searchExec ${searchExec.baselineMedianMs} -> ${searchExec.candidateMedianMs}`,
    ),
    g(
      "pass2NoRegression",
      pass2.candidateMedianMs - pass2.baselineMedianMs <=
        regressionBound(pass2.baselineMedianMs, 250, 15),
      `pass2 ${pass2.baselineMedianMs} -> ${pass2.candidateMedianMs}`,
    ),
  ];
}

/**
 * A/A self-check: both arms are the SAME contract, so the harness must find no
 * systematic winner. If it does, the pairing/ordering/phase parsing is biased
 * and no A/B verdict from it can be trusted. "No systematic advantage" is
 * expressed as: the 95% bootstrap CI for the paired delta CONTAINS zero.
 */
export function evaluateAaGates(pairs: ValidPair[], seed: number): GateResult[] {
  const g = (id: string, pass: boolean, detail: string): GateResult => ({ id, pass, detail });
  const out: GateResult[] = [
    g("validPairs>=4", pairs.length >= 4, `validPairs=${pairs.length}`),
    g(
      "commandHashesIdentical",
      pairs.every((p) => p.baseline.commandSha256 === p.candidate.commandSha256),
      "both arms build a byte-identical command",
    ),
    g(
      "promptHashesIdentical",
      pairs.every((p) => p.baseline.promptSha256 === p.candidate.promptSha256),
      "both arms send a byte-identical developer prompt",
    ),
  ];
  for (const phase of ["firstEventToSearchStartMs", "totalMs"] as PhaseKey[]) {
    const stat = pairedStat(pairs, phase, seed);
    if (stat.n === 0) continue;
    const straddlesZero = stat.ci95.lower <= 0 && stat.ci95.upper >= 0;
    out.push(
      g(
        `${phase}:ci95ContainsZero`,
        straddlesZero,
        `CI95 [${Math.round(stat.ci95.lower)}, ${Math.round(stat.ci95.upper)}]ms, ` +
          `delta=${stat.medianPairedDeltaMs}ms, wins=${stat.candidateWins}/${stat.n}`,
      ),
    );
  }
  return out;
}

/** The knowledge-class holdout gate: identical commands, zero searches. */
export function evaluateKnowledgeGates(
  pairs: ValidPair[],
  baseline: QuickAiCommandContract,
  candidate: QuickAiCommandContract,
  seed: number,
): GateResult[] {
  const total = pairedStat(pairs, "totalMs", seed);
  const g = (id: string, pass: boolean, detail: string): GateResult => ({ id, pass, detail });
  const bound = Math.max(500, (total.baselineMedianMs * 10) / 100);
  return [
    g("validPairs>=15", pairs.length >= 15, `validPairs=${pairs.length}`),
    g(
      "commandHashesIdentical",
      pairs.every((p) => p.baseline.commandSha256 === p.candidate.commandSha256),
      "baseline and candidate command hashes match",
    ),
    g(
      "candidateUsesBasePromptHash",
      pairs.every((p) => p.candidate.promptSha256 === candidate.basePromptSha256),
      `candidate prompt == candidate base prompt (${candidate.basePromptSha256.slice(0, 12)})`,
    ),
    g(
      "baselineUsesBasePromptHash",
      pairs.every((p) => p.baseline.promptSha256 === baseline.basePromptSha256),
      `baseline prompt == baseline base prompt (${baseline.basePromptSha256.slice(0, 12)})`,
    ),
    g(
      "zeroSearchesEveryRun",
      pairs.every(
        (p) =>
          p.baseline.analysis.distinctSearchItemCount === 0 &&
          p.candidate.analysis.distinctSearchItemCount === 0,
      ),
      "zero native searches on both arms",
    ),
    g(
      "answerHasNoSourceUrl",
      pairs.every(
        (p) =>
          (p.baseline.analysis.structuredSourceCount ?? 0) === 0 &&
          (p.candidate.analysis.structuredSourceCount ?? 0) === 0,
      ),
      "structured sources empty on both arms",
    ),
    g(
      "candidateNoLatencyRegression",
      total.candidateMedianMs - total.baselineMedianMs <= bound,
      `total ${total.baselineMedianMs} -> ${total.candidateMedianMs} (bound ${Math.round(bound)}ms)`,
    ),
  ];
}

// ------------------------------------------------------------------------- main

type Cli = {
  mode: "single" | "paired" | "aa";
  reps: number;
  maxAttempts: number;
  seed: number;
  baselineRef: string | null;
  candidateRef: string | null;
  out: string | null;
  label: string;
  printCommand: boolean;
};

export function parseCli(args: string[]): Cli {
  const arg = (f: string): string | null => {
    const i = args.indexOf(f);
    return i >= 0 ? (args[i + 1] ?? null) : null;
  };
  const mode = (arg("--mode") ?? "single") as Cli["mode"];
  if (!["single", "paired", "aa"].includes(mode)) throw new Error(`unknown --mode ${mode}`);
  return {
    mode,
    reps: Number(arg("--reps") ?? (mode === "single" ? "6" : "15")),
    maxAttempts: Number(arg("--max-attempts") ?? "24"),
    seed: Number(arg("--seed") ?? "20260725"),
    baselineRef: arg("--baseline-ref"),
    candidateRef: arg("--candidate-ref"),
    out: arg("--out"),
    label: arg("--label") ?? "before",
    printCommand: args.includes("--print-command"),
  };
}

export function refToSource(ref: string | null): SourceRef {
  if (ref === null || ref === "WORKTREE" || ref === "worktree") return { kind: "worktree" };
  return { kind: "git", ref };
}

const liveDeps: MeasureDeps = {
  spawn: (argv) => {
    const proc = Bun.spawn(argv, { stdout: "pipe", stderr: "pipe", stdin: "ignore" });
    return { stdout: proc.stdout, stderr: proc.stderr, exited: proc.exited };
  },
  now: () => performance.now(),
};

async function main(): Promise<void> {
  const cli = parseCli(process.argv.slice(2));
  const scratchRoot = "/tmp/quick-ai-latency-bench";
  mkdirSync(scratchRoot, { recursive: true });

  if (cli.printCommand) {
    const contract = loadContract(refToSource(cli.candidateRef));
    const built = buildCommand(contract, "<QUERY>", scratchRoot, "<SCHEMA>", false);
    console.log(
      JSON.stringify(
        {
          model: contract.model,
          configFlags: contract.configFlags,
          promptChars: contract.basePrompt.length,
          basePromptSha256: contract.basePromptSha256,
          forceSuffixPresent: contract.forceSuffix !== null,
          forceSuffixSha256: contract.forceSuffixSha256,
          schemaChars: contract.outputSchema.length,
          commandSha256: built.commandSha256,
          command: built.argv,
        },
        null,
        2,
      ),
    );
    return;
  }

  const baselineSource = refToSource(cli.mode === "single" ? cli.candidateRef : cli.baselineRef);
  const candidateSource = refToSource(cli.candidateRef);
  const baseline = loadContract(baselineSource);
  const candidate = loadContract(candidateSource);

  console.error(
    `[bench] mode=${cli.mode} baseline=${baseline.source} candidate=${candidate.source} ` +
      `reps=${cli.reps} seed=${cli.seed}\n` +
      `[bench] baselinePrompt=${baseline.basePromptSha256.slice(0, 12)} ` +
      `candidatePrompt=${candidate.basePromptSha256.slice(0, 12)} ` +
      `forceSuffix=${candidate.forceSuffix === null ? "absent" : "present"}`,
  );

  const attempts: Attempt[] = [];

  if (cli.mode === "single") {
    for (const q of QUERIES) {
      for (let r = 0; r < cli.reps; r += 1) {
        const a = await measureOnce(candidate, q, "candidate", r, scratchRoot, liveDeps);
        attempts.push(a);
        logAttempt(a);
      }
    }
  } else {
    for (const q of QUERIES) {
      let validPairs = 0;
      let pairIndex = 0;
      let attemptedPairs = 0;
      while (validPairs < cli.reps && attemptedPairs < cli.maxAttempts) {
        const order = pairOrder(pairIndex, cli.seed);
        const results: Attempt[] = [];
        for (const arm of order) {
          const contract = arm === "baseline" ? baseline : candidate;
          const a = await measureOnce(contract, q, arm, pairIndex, scratchRoot, liveDeps);
          attempts.push(a);
          logAttempt(a);
          results.push(a);
        }
        attemptedPairs += 1;
        if (results.every((a) => a.valid)) validPairs += 1;
        else console.error(`[bench] pair ${pairIndex} (${q.id}) invalid — rerunning the whole pair`);
        pairIndex += 1;
      }
      if (validPairs < cli.reps) {
        console.error(
          `[bench] WARNING ${q.id}: only ${validPairs}/${cli.reps} valid pairs after ` +
            `${attemptedPairs} attempted pairs (max ${cli.maxAttempts})`,
        );
      }
    }
  }

  const perQuery: Record<string, unknown> = {};
  for (const q of QUERIES) {
    const pairs = collectValidPairs(attempts, q.id);
    const stats: Record<string, PairedStat> = {};
    for (const phase of PHASE_KEYS) stats[phase] = pairedStat(pairs, phase, cli.seed);
    const gates =
      cli.mode === "single"
        ? []
        : cli.mode === "aa"
          ? evaluateAaGates(pairs, cli.seed)
          : q.needsSearch
            ? evaluateLiveGates(pairs, cli.seed)
            : evaluateKnowledgeGates(pairs, baseline, candidate, cli.seed);
    perQuery[q.id] = {
      needsSearch: q.needsSearch,
      forced: q.forced,
      attemptCount: attempts.filter((a) => a.query === q.id).length,
      validPairCount: pairs.length,
      stats,
      gates,
      gatesPassed: gates.every((x) => x.pass),
    };
  }

  const receipt = {
    schemaVersion: 2,
    mode: cli.mode,
    label: cli.label,
    seed: cli.seed,
    reps: cli.reps,
    maxAttempts: cli.maxAttempts,
    baselineContract: contractReceipt(baseline),
    candidateContract: contractReceipt(candidate),
    referenceBaseline: REFERENCE_BASELINE,
    perQuery,
    attempts,
  };

  if (cli.out) {
    mkdirSync(dirname(cli.out), { recursive: true });
    writeFileSync(cli.out, JSON.stringify(receipt, null, 2));
    console.error(`[bench] receipt -> ${cli.out}`);
  }
  console.log(JSON.stringify({ ...receipt, attempts: `${attempts.length} attempts (see --out)` }, null, 2));
}

function contractReceipt(c: QuickAiCommandContract) {
  return {
    source: c.source,
    model: c.model,
    configFlags: c.configFlags,
    basePromptChars: c.basePrompt.length,
    basePromptSha256: c.basePromptSha256,
    forceSuffixPresent: c.forceSuffix !== null,
    forceSuffixSha256: c.forceSuffixSha256,
    outputSchemaSha256: c.outputSchemaSha256,
  };
}

function logAttempt(a: Attempt): void {
  const p = a.analysis.phases;
  console.error(
    `[bench] ${a.arm.padEnd(9)} ${a.query.padEnd(9)} pair${String(a.pairIndex).padStart(2)} ` +
      `total=${Math.round(p.totalMs)}ms boot=${fmt(p.spawnToFirstEventMs)} ` +
      `pass1=${fmt(p.firstEventToSearchStartMs)} search=${fmt(p.searchStartToSearchCompleteMs)} ` +
      `pass2=${fmt(p.searchCompleteToAnswerMs)} nSearch=${a.analysis.distinctSearchItemCount} ` +
      `forced=${a.forcedPromptApplied} valid=${a.valid}`,
  );
}

const fmt = (v: number | null) => (v === null ? "-" : String(Math.round(v)));

if (import.meta.main) {
  await main();
}
