#!/usr/bin/env bun

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

type StoryResultStatus = "pass" | "fail_closed" | "blocked_precondition" | "runtime_failure" | "timeout";

type StoryStep = {
  intent: string;
  action: string;
  receipt: string;
  observed: string;
};

type StoryResult = {
  id: string;
  recipe: string;
  story: string;
  steps: StoryStep[];
  command: string[];
  status: StoryResultStatus;
  exitCode: number | null;
  durationMs: number;
  summary: string | null;
  missingReceipt: string | null;
  failClosed: boolean;
  failureCode: string | null;
  warnings: string[];
  outputPreview: string;
};

const ALREADY_EXERCISED_THIS_THREAD = new Set([
  "visible-text-clipping-overlap-stress",
  "layout-measurement-regression-stress",
  "screenshot-semantics-visual-consistency-stress",
  "visual-contrast-readable-state-stress",
  "long-text-wrap-resize-surface-stress",
  "div-container-scroll-overflow-stress",
  "main-menu-dynamic-choice-resize-stress",
  "notes-window-resize-stress",
  "actions-command-discoverability-noop-stress",
]);

function parseArgs(argv: string[]) {
  let limit = 100;
  let maxMs = 30_000;
  let includeKnown = false;
  let dryRun = false;
  let reclassifyPath: string | null = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--limit") {
      limit = Number(argv[++i] ?? limit);
    } else if (arg === "--max-ms") {
      maxMs = Number(argv[++i] ?? maxMs);
    } else if (arg === "--include-known") {
      includeKnown = true;
    } else if (arg === "--dry-run") {
      dryRun = true;
    } else if (arg === "--reclassify") {
      reclassifyPath = argv[++i] ?? null;
    }
  }

  return { limit, maxMs, includeKnown, dryRun, reclassifyPath };
}

function humanizeRecipe(recipe: string) {
  return recipe
    .replace(/-stress$/, "")
    .replace(/-/g, " ")
    .replace(/\bagent_chat\b/g, "Agent Chat")
    .replace(/\bux\b/g, "UX")
    .replace(/\bui\b/g, "UI");
}

function storyForRecipe(recipe: string) {
  const subject = humanizeRecipe(recipe);
  return `As a Script Kit user exercising ${subject}, I expect visible state, focus, layout, ownership, and cleanup to stay coherent while I interact with the app.`;
}

function extractStressRecipes(indexSource: string) {
  const recipes = new Set<string>();
  const pattern = /case "([^"]+-stress)"/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(indexSource)) !== null) {
    recipes.add(match[1]);
  }
  return [...recipes];
}

function extractJsonObjects(output: string): any[] {
  const objects: any[] = [];

  for (let start = 0; start < output.length; start += 1) {
    if (output[start] !== "{") continue;

    let depth = 0;
    let inString = false;
    let escaped = false;

    for (let index = start; index < output.length; index += 1) {
      const char = output[index];

      if (inString) {
        if (escaped) {
          escaped = false;
        } else if (char === "\\") {
          escaped = true;
        } else if (char === "\"") {
          inString = false;
        }
        continue;
      }

      if (char === "\"") {
        inString = true;
      } else if (char === "{") {
        depth += 1;
      } else if (char === "}") {
        depth -= 1;
        if (depth === 0) {
          try {
            objects.push(JSON.parse(output.slice(start, index + 1)));
          } catch {
            // Ignore brace-balanced text that is not JSON.
          }
          start = index;
          break;
        }
      }
    }
  }

  return objects;
}

function extractJsonObject(output: string): any | null {
  const objects = extractJsonObjects(output);
  return [...objects].reverse().find((object) => object?.recipe && object?.status) ?? objects.at(-1) ?? null;
}

function failureCode(parsed: any | null) {
  return parsed?.failure?.code ?? parsed?.proofBundle?.failure?.code ?? null;
}

function stringifyValue(value: unknown, fallback: string) {
  if (typeof value === "string" && value.trim()) return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (value === null || value === undefined) return fallback;
  try {
    return JSON.stringify(value).slice(0, 500);
  } catch {
    return fallback;
  }
}

function compactOutputPreview(output: string) {
  return output.replace(/\s+/g, " ").trim().slice(0, 500);
}

function normalizeHarnessStep(step: any, index: number, recipe: string): StoryStep {
  const failure = step?.failure ?? step?.proofBundle?.failure;
  return {
    intent: stringifyValue(
      step?.intent ?? step?.name ?? step?.stepName ?? step?.label,
      `Exercise ${recipe} step ${index + 1}`,
    ),
    action: stringifyValue(step?.action ?? step?.command ?? step?.type ?? step?.event, "record harness step"),
    receipt: stringifyValue(
      step?.receipt ?? step?.receiptType ?? step?.status ?? failure?.code ?? step?.code,
      `step-${index + 1}`,
    ),
    observed: stringifyValue(
      step?.observed ?? step?.summary ?? step?.message ?? failure?.message ?? step?.actual,
      "No structured observation emitted",
    ),
  };
}

function extractStorySteps(
  recipe: string,
  command: string[],
  parsed: any | null,
  status: StoryResultStatus,
  exitCode: number | null,
  output: string,
): StoryStep[] {
  const harnessSteps = parsed?.steps ?? parsed?.proofBundle?.steps;
  const steps: StoryStep[] = Array.isArray(harnessSteps)
    ? harnessSteps.map((step, index) => normalizeHarnessStep(step, index, recipe))
    : [];
  const code = failureCode(parsed);
  const missingReceipt = parsed?.missingReceipt ?? parsed?.proofBundle?.missingReceipt ?? code;
  const summary = stringifyValue(parsed?.summary ?? parsed?.proofBundle?.summary, compactOutputPreview(output) || "No output");

  if (steps.length === 0) {
    steps.push(
      {
        intent: `Run ${recipe} through the real agentic stress harness`,
        action: command.join(" "),
        receipt: exitCode === null ? "process-timeout-or-killed" : `exitCode:${exitCode}`,
        observed: compactOutputPreview(output) || "Harness produced no stdout/stderr",
      },
      {
        intent: "Collect structured proof output from the harness",
        action: "parse stdout/stderr JSON into recipe, status, proofBundle, failure, and warnings",
        receipt: missingReceipt ? `missingReceipt:${missingReceipt}` : `parsedStatus:${parsed?.status ?? "none"}`,
        observed: summary,
      },
    );
  }

  steps.push({
    intent: "Classify the user-story outcome without treating missing receipts as green",
    action: "classify(parsed, exitCode, timedOut, outputPreview)",
    receipt: `status:${status}`,
    observed: `failClosed=${isFailClosed(parsed, output)} warnings=${(parsed?.proofBundle?.warnings ?? parsed?.warnings ?? []).length}`,
  });

  return steps;
}

function isFailClosed(parsed: any | null, outputPreview = "") {
  const summary = String(parsed?.summary ?? outputPreview);
  const code = failureCode(parsed) ?? "";
  const warnings = parsed?.proofBundle?.warnings ?? parsed?.warnings ?? [];
  return parsed?.failClosed === true
    || parsed?.proofBundle?.failClosed === true
    || parsed?.failureMode === "fail_closed"
    || parsed?.proofBundle?.failureMode === "fail_closed"
    || summary.includes("failed closed")
    || code.startsWith("missing_")
    || warnings.some((warning: string) => warning.startsWith("file_linear:"));
}

function classify(parsed: any | null, exitCode: number | null, timedOut: boolean, outputPreview = ""): StoryResultStatus {
  const topLevelStatus = outputPreview.match(/^\s*\{[\s\S]{0,240}?\"recipe\"\s*:\s*\"[^\"]+\"\s*,\s*\"status\"\s*:\s*\"(pass|fail)\"/)?.[1];
  if (timedOut) return "timeout";
  if (failureCode(parsed) === "insufficient_target_count") return "blocked_precondition";
  if (topLevelStatus === "pass") return "pass";
  if (topLevelStatus !== "fail" && parsed?.status === "pass") return "pass";
  if (isFailClosed(parsed, outputPreview)) return "fail_closed";
  if (exitCode === 0 && parsed?.status === "pass") return "pass";
  return "runtime_failure";
}

async function runStory(recipe: string, index: number, maxMs: number): Promise<StoryResult> {
  const session = `story-audit-${Date.now()}-${String(index + 1).padStart(3, "0")}`;
  const command = ["bun", "scripts/agentic/index.ts", recipe, "--session", session, "--json"];
  const started = Date.now();
  const proc = Bun.spawn(command, {
    stdout: "pipe",
    stderr: "pipe",
    env: {
      ...Bun.env,
      SCRIPT_KIT_AGENTIC_AUDIT: "100-user-stories",
    },
  });

  let timedOut = false;
  const timeout = setTimeout(() => {
    timedOut = true;
    proc.kill();
  }, maxMs);

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited.catch(() => null),
  ]);
  clearTimeout(timeout);

  const output = `${stdout}\n${stderr}`.trim();
  const parsed = extractJsonObject(output);
  const status = classify(parsed, exitCode, timedOut, output);

  return {
    id: `ux-story-${String(index + 1).padStart(3, "0")}`,
    recipe,
    story: storyForRecipe(recipe),
    steps: extractStorySteps(recipe, command, parsed, status, exitCode, output),
    command,
    status,
    exitCode,
    durationMs: Date.now() - started,
    summary: parsed?.summary ?? null,
    missingReceipt: parsed?.missingReceipt ?? parsed?.proofBundle?.missingReceipt ?? failureCode(parsed),
    failClosed: isFailClosed(parsed, output),
    failureCode: failureCode(parsed),
    warnings: parsed?.proofBundle?.warnings ?? parsed?.warnings ?? [],
    outputPreview: output.slice(0, 1600),
  };
}

async function main() {
  const { limit, maxMs, includeKnown, dryRun, reclassifyPath } = parseArgs(Bun.argv.slice(2));

  if (reclassifyPath) {
    const existing = JSON.parse(await readFile(reclassifyPath, "utf8"));
    const stories = existing.stories.map((story: StoryResult) => {
      const parsed = extractJsonObject(story.outputPreview);
      const status = classify(parsed, story.exitCode, story.status === "timeout", story.outputPreview);
      return {
        ...story,
        steps: story.steps?.length
          ? story.steps
          : extractStorySteps(story.recipe, story.command, parsed, status, story.exitCode, story.outputPreview),
        status,
        summary: story.summary ?? parsed?.summary ?? null,
        missingReceipt: story.missingReceipt ?? parsed?.missingReceipt ?? parsed?.proofBundle?.missingReceipt ?? failureCode(parsed),
        failClosed: isFailClosed(parsed, story.outputPreview),
        failureCode: story.failureCode ?? failureCode(parsed),
        warnings: story.warnings?.length ? story.warnings : parsed?.proofBundle?.warnings ?? parsed?.warnings ?? [],
      };
    });
    const statusCounts = stories.reduce<Record<string, number>>((acc, story: StoryResult) => {
      acc[story.status] = (acc[story.status] ?? 0) + 1;
      return acc;
    }, {});
    const normalized = {
      ...existing,
      reclassifiedAt: new Date().toISOString(),
      statusCounts,
      stories,
    };
    const outPath = reclassifyPath.replace(/\.json$/, ".normalized.json");
    await writeFile(outPath, `${JSON.stringify(normalized, null, 2)}\n`);
    console.log(JSON.stringify({ artifactPath: outPath, statusCounts }, null, 2));
    return;
  }
  const indexSource = await readFile("scripts/agentic/index.ts", "utf8");
  const candidates = extractStressRecipes(indexSource)
    .filter((recipe) => includeKnown || !ALREADY_EXERCISED_THIS_THREAD.has(recipe))
    .slice(0, limit);

  if (!Number.isSafeInteger(limit) || limit <= 0 || candidates.length < limit) {
    throw new Error(`Nonempty exact story selection required; found ${candidates.length}, requested ${limit}`);
  }

  const startedAt = new Date().toISOString();
  const results: StoryResult[] = [];

  if (!dryRun) {
    for (const [index, recipe] of candidates.entries()) {
      const result = await runStory(recipe, index, maxMs);
      results.push(result);
      console.error(`${result.id} ${result.status} ${recipe}`);
    }
  }

  const statusCounts = results.reduce<Record<string, number>>((acc, result) => {
    acc[result.status] = (acc[result.status] ?? 0) + 1;
    return acc;
  }, {});

  const artifact = {
    schemaVersion: 1,
    audit: "agentic-100-user-story-ux-audit",
    startedAt,
    completedAt: new Date().toISOString(),
    requestedStoryCount: limit,
    selectedStoryCount: candidates.length,
    skippedAlreadyExercisedThisThread: includeKnown ? [] : [...ALREADY_EXERCISED_THIS_THREAD],
    dryRun,
    maxMs,
    statusCounts,
    stories: dryRun
      ? candidates.map((recipe, index) => ({
          id: `ux-story-${String(index + 1).padStart(3, "0")}`,
          recipe,
          story: storyForRecipe(recipe),
          steps: [
            {
              intent: `Run ${recipe} through the real agentic stress harness`,
              action: `bun scripts/agentic/index.ts ${recipe} --session <generated> --json`,
              receipt: "dry-run",
              observed: "Dry run selected this recipe without executing it",
            },
            {
              intent: "Collect structured proof output from the harness",
              action: "parse stdout/stderr JSON into recipe, status, proofBundle, failure, and warnings",
              receipt: "dry-run",
              observed: "No runtime receipt in dry-run mode",
            },
          ],
        }))
      : results,
  };

  await mkdir(".test-output", { recursive: true });
  const artifactPath = join(".test-output", `agentic-100-user-story-audit-${startedAt.replace(/[:.]/g, "-")}.json`);
  await writeFile(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`);
  console.log(JSON.stringify({ ...artifact, artifactPath }, null, 2));
  if (!dryRun && (results.length !== candidates.length || results.length === 0 || results.some(result => result.status !== "pass")))
    process.exitCode = 1;
}

await main();
