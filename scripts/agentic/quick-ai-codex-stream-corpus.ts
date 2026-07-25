#!/usr/bin/env bun
/**
 * Capture a corpus of REAL `codex exec --json` streams using the exact
 * production Quick AI command shape, then classify each stream against the
 * production one-search policy.
 *
 * Why this exists: `src/ai/agent_chat/codex_exec.rs` admits exactly one
 * `web_search` item id per turn. Codex reports a page visit as a SEPARATE
 * `web_search` item whose action is `other` and whose `query` is the URL. Any
 * turn where the model searches and then opens a page therefore trips
 * `WebBudgetDecision::Stop` and ends in a recovery card instead of an answer.
 *
 * The same page-visit items are the ONLY carrier of source provenance: a
 * `search` action's completed item contains queries and nothing else, so
 * `structured_urls` stays empty on a search-only turn.
 *
 * This script measures how often that happens on ordinary questions, so the
 * policy decision rests on observed model behavior rather than assumption.
 *
 * Usage: bun scripts/agentic/quick-ai-codex-stream-corpus.ts [--reps 1]
 * Receipt: /tmp/quick-ai-stream-corpus/results.json (+ one .ndjson per run)
 */
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const OUT = "/tmp/quick-ai-stream-corpus";
mkdirSync(OUT, { recursive: true });

const MODEL = "gpt-5.3-codex-spark";

/**
 * Read the production constants straight out of the Rust so this harness
 * cannot drift from `build_codex_exec_command`. A run without
 * `developer_instructions` is NOT the production command: the system prompt is
 * what tells the model to search exactly once and never open a page.
 */
function extractRustStr(file: string, name: string): string {
  const src = require("node:fs").readFileSync(file, "utf8") as string;
  const raw = src.match(new RegExp(`${name}: &str = r#"([\\s\\S]*?)"#;`));
  if (raw) return raw[1];
  const plain = src.match(new RegExp(`${name}: &str = "((?:[^"\\\\]|\\\\.)*)";`));
  if (!plain) throw new Error(`could not extract ${name}`);
  return JSON.parse(`"${plain[1]}"`);
}

const PROMPT = extractRustStr("src/ai/agent_chat/profiles.rs", "QUICK_AI_APPEND_SYSTEM_PROMPT");
const SCHEMA = extractRustStr("src/ai/agent_chat/codex_exec.rs", "QUICK_AI_OUTPUT_SCHEMA");
const SCHEMA_PATH = join(OUT, "quick-ai-output-schema.json");
writeFileSync(SCHEMA_PATH, SCHEMA);

const argOf = (f: string) => {
  const i = process.argv.indexOf(f);
  return i >= 0 ? process.argv[i + 1] : undefined;
};
const REPS = Number(argOf("--reps") ?? 1);

/** Ordinary Quick AI questions. Mixed: some need a page, some do not. */
const QUERIES: Array<{ key: string; text: string }> = [
  { key: "rust-release", text: "What is the latest stable Rust release? Give the version, release date, and one official source URL." },
  { key: "bun-version", text: "What is the current Bun version?" },
  { key: "no-web", text: "What does the Rust `?` operator do? One sentence." },
  { key: "weather-ish", text: "Who won the most recent Formula 1 race?" },
  { key: "docs-lookup", text: "What is the default value of macOS NSWindow collectionBehavior?" },
  { key: "price", text: "What is the current price of the Anthropic Claude API per million input tokens for Sonnet?" },
];

function args(query: string) {
  return [
    "--search",
    "--model", MODEL,
    "--sandbox", "read-only",
    "--cd", OUT,
    "--disable", "plugins",
    "--config", "skills.bundled.enabled=false",
    "--config", 'model_reasoning_effort="low"',
    "--config", 'tools.web_search.context_size="low"',
    "--config", `developer_instructions=${JSON.stringify(PROMPT)}`,
    "exec", "--ephemeral", "--ignore-user-config", "--ignore-rules", "--skip-git-repo-check",
    "--output-schema", SCHEMA_PATH,
    "--json", query,
  ];
}

type Run = {
  key: string;
  rep: number;
  exitCode: number | null;
  totalMs: number;
  lines: string[];
};

function runOnce(key: string, rep: number, query: string): Promise<Run> {
  return new Promise((resolve) => {
    const t0 = performance.now();
    // stdin MUST be `ignore`; with an inherited tty codex blocks on
    // "Reading additional input from stdin..." and never emits an event.
    const child = spawn("codex", args(query), { stdio: ["ignore", "pipe", "pipe"], cwd: OUT });
    let buf = "";
    const lines: string[] = [];
    child.stdout.on("data", (d) => {
      buf += d.toString();
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i).trim();
        buf = buf.slice(i + 1);
        if (line) lines.push(line);
      }
    });
    child.on("close", (exitCode) => {
      resolve({ key, rep, exitCode, totalMs: Math.round(performance.now() - t0), lines });
    });
    child.on("error", () => {
      resolve({ key, rep, exitCode: -1, totalMs: Math.round(performance.now() - t0), lines });
    });
  });
}

const isUrl = (s: string) => /^https?:\/\/\S+$/i.test(s.trim());

/**
 * Replay the production admission rules from `observe_web_item`. Kept
 * deliberately literal so a divergence from the Rust is visible as a diff.
 */
function classify(lines: string[]) {
  let admittedId: string | null = null;
  let searchStarted = false;
  let searchCompleted = false;
  let stopped = false;
  let stopReason: string | null = null;
  const pageVisitUrls: string[] = [];
  const searchItemIds = new Set<string>();
  let answer: string | null = null;

  for (const raw of lines) {
    let ev: any;
    try { ev = JSON.parse(raw); } catch { continue; }
    const item = ev?.item;
    if (item?.type === "agent_message" && ev.type === "item.completed") answer = item.text;
    if (item?.type !== "web_search") continue;
    searchItemIds.add(item.id);
    const action = item.action ?? { type: "other" };
    const isPageFollow =
      action.type === "open_page" ||
      action.type === "find_in_page" ||
      (action.type === "other" && isUrl(item.query ?? ""));
    if (isPageFollow && isUrl(item.query ?? "")) pageVisitUrls.push(item.query);
    if (stopped) continue;

    if (admittedId === null) {
      if (isPageFollow) { stopped = true; stopReason = "page-follow-before-any-search"; continue; }
      admittedId = item.id;
      if (action.type === "search") { searchStarted = true; }
    } else if (admittedId !== item.id) {
      stopped = true;
      stopReason = isPageFollow ? "second-item-page-follow" : "second-item-distinct-id";
      continue;
    } else if (isPageFollow) {
      stopped = true; stopReason = "admitted-item-became-page-follow"; continue;
    } else if (action.type === "search") {
      searchStarted = true;
    }
    if (searchStarted && ev.type === "item.completed") searchCompleted = true;
  }

  let answerSources: string[] = [];
  let answerText: string | null = null;
  if (answer) {
    try {
      const parsed = JSON.parse(answer);
      answerSources = Array.isArray(parsed?.sources) ? parsed.sources : [];
      answerText = typeof parsed?.answer === "string" ? parsed.answer : null;
    } catch { answerText = answer; }
  }

  // `structured_urls` is only written for an item that was NOT stopped, and
  // every URL-bearing item is classified as a page follow, which stops. So in
  // production this list is provably always empty — which is exactly why the
  // host-verification branch of the provenance gate is unreachable.
  const provenanceUrls = stopped ? [] : pageVisitUrls;
  const provenanceHosts = new Set(provenanceUrls.map((u) => new URL(u).host.toLowerCase()));
  const citedHosts = answerSources.filter(isUrl).map((u) => new URL(u).host.toLowerCase());
  const uncitedSources = citedHosts.filter((h) => !provenanceHosts.has(h));

  // Mirror `render_final_answer`: schema sources not already present in the
  // answer text are appended as `Source: <url>` lines BEFORE the provenance
  // gate looks for URLs. Checking the raw answer instead would report false
  // protocol failures.
  let rendered = answerText ?? "";
  for (const source of answerSources.filter(isUrl)) {
    if (!rendered.includes(source)) rendered += `\n\nSource: ${source}`;
  }
  // Model `enforce_answer_provenance` so the reported outcome is what a user
  // would actually see, not just whether the model produced text.
  const answerUrls = rendered.match(/https?:\/\/\S+/gi) ?? [];
  let outcome: string;
  let gateFailure: string | null = null;
  if (stopped) {
    outcome = "RECOVERY-CARD";
  } else if (!answerText) {
    outcome = "NO-ANSWER";
  } else if (provenanceUrls.length === 0) {
    const honestEmpty = answerSources.length === 0;
    if (!searchCompleted || (answerUrls.length === 0 && !honestEmpty)) {
      outcome = "PROTOCOL-FAILURE";
      gateFailure = "quick_ai_structured_sources_unavailable";
    } else {
      outcome = "ANSWER";
    }
  } else {
    outcome = "ANSWER";
  }

  return {
    webItemCount: searchItemIds.size,
    pageVisitCount: pageVisitUrls.length,
    searchCompleted,
    policyStopped: stopped,
    stopReason,
    productionOutcome: outcome,
    gateFailure,
    answerSources,
    provenanceUrls,
    /** Sources the answer cites that no observed page visit backs. */
    unverifiedSourceHosts: uncitedSources,
  };
}

const results: any[] = [];
for (const q of QUERIES) {
  for (let rep = 1; rep <= REPS; rep++) {
    const run = await runOnce(q.key, rep, q.text);
    writeFileSync(join(OUT, `${q.key}-${rep}.ndjson`), run.lines.join("\n"));
    const c = classify(run.lines);
    results.push({ key: q.key, rep, totalMs: run.totalMs, exitCode: run.exitCode, ...c });
    console.error(
      `[corpus] ${q.key} rep${rep}: ${c.productionOutcome.padEnd(17)} ` +
        `items=${c.webItemCount} pageVisits=${c.pageVisitCount} ` +
        `stop=${c.stopReason ?? "-"} unverifiedCites=${c.unverifiedSourceHosts.length} ${run.totalMs}ms`,
    );
  }
}

const total = results.length;
const recovery = results.filter((r) => r.productionOutcome === "RECOVERY-CARD").length;
const unverified = results.filter((r) => r.unverifiedSourceHosts.length > 0).length;
const out = {
  schemaVersion: 1,
  model: MODEL,
  total,
  recoveryCardRate: `${recovery}/${total}`,
  unverifiedCitationRate: `${unverified}/${total}`,
  results,
};
writeFileSync(join(OUT, "results.json"), JSON.stringify(out, null, 2));
console.error(`\nrecovery-card outcomes: ${recovery}/${total}`);
console.error(`answers citing an unvisited host: ${unverified}/${total}`);
console.error(`receipt: ${join(OUT, "results.json")}`);
console.log(JSON.stringify(out, null, 2));
