#!/usr/bin/env node
// Generate stories/stories.json from the canonical story.js definitions.
//
// WHY THIS EXISTS
// ---------------
// stories.json was hand-maintained and had already drifted from the real
// definitions. Measured 2026-07-27 against 01-run-script-with-arg:
//
//   durationMs   manifest 7500   actual 9000
//   chapters     manifest omitted the "narrow" chapter
//   actionKinds  manifest omitted "ensureRows"
//
// A storyboard inventory that claims coverage while misreporting its own
// contents cannot be evidence. Duplicated fields (count, chapters, durationMs,
// surfaces, actionKinds) are now DERIVED, never authored.
//
// Each story.js is executed in a sandbox with a stubbed StoryPlayer so the
// definition object can be captured without a DOM.
//
// Usage:
//   node design/mockups/stories/build-manifest.mjs          # write
//   node design/mockups/stories/build-manifest.mjs --check  # verify only (CI)

import { readdirSync, readFileSync, writeFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const manifestPath = join(here, "stories.json");
const checkOnly = process.argv.includes("--check");

/** Execute a story.js in a sandbox and return its `story` definition. */
function loadStory(dir) {
  const file = join(here, dir, "story.js");
  const src = readFileSync(file, "utf8");
  const captured = {};
  const sandbox = {
    window: { StoryPlayer: { mount: (opts) => { captured.mounted = opts; } } },
    document: { querySelector: () => null, body: {} },
    console,
  };
  vm.createContext(sandbox);
  try {
    vm.runInContext(src, sandbox, { filename: file, timeout: 5000 });
  } catch (err) {
    throw new Error(`${dir}/story.js failed to evaluate: ${err.message}`);
  }
  const story = captured.mounted?.story;
  if (!story) throw new Error(`${dir}/story.js did not call StoryPlayer.mount({story})`);
  return story;
}

function storyDirs() {
  return readdirSync(here)
    .filter((e) => {
      if (e === "shared" || e.startsWith(".")) return false;
      const p = join(here, e);
      return statSync(p).isDirectory() && existsSafe(join(p, "story.js"));
    })
    .sort();
}

function existsSafe(p) {
  try { statSync(p); return true; } catch { return false; }
}

/** Read authored-only metadata (title/blurb/entry/covers/effort) from the
 *  previous manifest so this generator never invents prose. */
function priorMeta() {
  try {
    const prev = JSON.parse(readFileSync(manifestPath, "utf8"));
    return new Map((prev.stories || []).map((s) => [s.id, s]));
  } catch {
    return new Map();
  }
}

const prior = priorMeta();
const dirs = storyDirs();
const stories = [];
const problems = [];

for (const dir of dirs) {
  let story;
  try {
    story = loadStory(dir);
  } catch (err) {
    problems.push(err.message);
    continue;
  }
  if (story.id !== dir) {
    problems.push(`${dir}/story.js declares id "${story.id}" — must match its directory`);
  }
  const p = prior.get(story.id) || {};
  const surfaces = (story.surfaces || []).map((s) => (typeof s === "string" ? s : s.id));
  const entry = `${dir}/index.html`;

  stories.push({
    id: story.id,
    // Authored prose is preserved from the prior manifest; the generator
    // never fabricates a title or blurb.
    title: p.title ?? story.title ?? story.id,
    blurb: p.blurb ?? story.blurb ?? "",
    entry,
    // Optional schema-v3 authored fields, carried through when present.
    ...(story.storyVersion ? { storyVersion: story.storyVersion } : {}),
    ...(story.covers ?? p.covers ? { covers: story.covers ?? p.covers } : {}),
    ...(story.effort ?? p.effort ? { effort: story.effort ?? p.effort } : {}),
    // --- everything below is DERIVED; do not hand-edit ---
    chapters: (story.chapters || []).map((c) => ({ id: c.id, label: c.label, at: c.at })),
    surfaces: [...new Set(surfaces)].sort(),
    durationMs: story.durationMs,
    actionKinds: [...new Set((story.actions || []).map((a) => a.kind))].sort(),
    ...(story.assertions ? { assertions: story.assertions.map((a) => a.id) } : {}),
  });
}

if (problems.length) {
  console.error("✗ story manifest build failed:");
  for (const p of problems) console.error("  - " + p);
  process.exit(1);
}

const prevRaw = (() => { try { return JSON.parse(readFileSync(manifestPath, "utf8")); } catch { return {}; } })();
const manifest = {
  schemaVersion: prevRaw.schemaVersion ?? 2,
  buildId: prevRaw.buildId ?? "story-mockup-realism-v2",
  architecture: prevRaw.architecture ?? "continuous-iframe-timeline",
  generated: "derived from story.js definitions by build-manifest.mjs — do not hand-edit derived fields",
  count: stories.length,
  stories,
};

const next = JSON.stringify(manifest, null, 2) + "\n";
const current = (() => { try { return readFileSync(manifestPath, "utf8"); } catch { return ""; } })();

if (checkOnly) {
  if (next !== current) {
    console.error("✗ stories.json is stale — run: node design/mockups/stories/build-manifest.mjs");
    // Report the specific drifting stories to make the failure actionable.
    const cur = prevRaw.stories || [];
    for (const s of stories) {
      const c = cur.find((x) => x.id === s.id);
      if (!c) { console.error(`  + ${s.id} missing from manifest`); continue; }
      if (c.durationMs !== s.durationMs)
        console.error(`  ~ ${s.id} durationMs: manifest ${c.durationMs} vs actual ${s.durationMs}`);
      const cc = (c.chapters || []).map((x) => x.id).join(",");
      const sc = s.chapters.map((x) => x.id).join(",");
      if (cc !== sc) console.error(`  ~ ${s.id} chapters: manifest [${cc}] vs actual [${sc}]`);
      const ck = (c.actionKinds || []).slice().sort().join(",");
      const sk = s.actionKinds.join(",");
      if (ck !== sk) console.error(`  ~ ${s.id} actionKinds: manifest [${ck}] vs actual [${sk}]`);
    }
    process.exit(1);
  }
  console.log(`✓ stories.json matches ${stories.length} story definitions`);
} else {
  writeFileSync(manifestPath, next);
  console.log(`✓ wrote stories.json from ${stories.length} story definitions`);
}
