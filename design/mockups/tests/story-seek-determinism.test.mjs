#!/usr/bin/env node
/**
 * Seek determinism: seek(t) MUST equal play-through-to(t).
 *
 * WHY THIS EXISTS
 * ---------------
 * The player previously kept a `msgOnce` cache OUTSIDE reduce(), reset only at
 * t===0, and `sem.appendMessage` was a single overwritten slot. So the DOM after
 * jumping to a chapter could differ from the DOM after playing into it, and
 * seeking backward left later messages on screen. A storyboard that renders
 * differently depending on how you arrived at a moment cannot be evidence for
 * "we covered every scenario".
 *
 * This test pins the invariant at the state layer: for every story, at every
 * chapter boundary, the reduced state reached by a direct jump must be deeply
 * equal to the state reached by stepping through at several granularities.
 * Because applyState() is now a pure function of reduce() with no caches, state
 * equality is the meaningful guarantee.
 *
 * Usage: node design/mockups/tests/story-seek-determinism.test.mjs
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const storiesDir = join(here, "..", "stories");
const failures = [];

/** Load the player once into a sandbox and return its reduce(). */
function loadPlayer() {
  const src = readFileSync(join(storiesDir, "shared", "story-player.js"), "utf8");
  const sandbox = { window: {}, document: { querySelector: () => null, body: {} }, console };
  vm.createContext(sandbox);
  vm.runInContext(src, sandbox, { filename: "story-player.js" });
  const player = sandbox.window.StoryPlayer;
  if (!player || typeof player.reduce !== "function") {
    throw new Error("story-player.js did not export StoryPlayer.reduce");
  }
  return player;
}

function loadStory(dir) {
  const src = readFileSync(join(storiesDir, dir, "story.js"), "utf8");
  const captured = {};
  const sandbox = {
    window: { StoryPlayer: { mount: (o) => { captured.story = o.story; } } },
    document: { querySelector: () => null, body: {} },
    console,
  };
  vm.createContext(sandbox);
  vm.runInContext(src, sandbox, { filename: `${dir}/story.js` });
  return captured.story;
}

function storyDirs() {
  return readdirSync(storiesDir)
    .filter((e) => {
      if (e === "shared" || e.startsWith(".")) return false;
      const p = join(storiesDir, e);
      try { return statSync(p).isDirectory() && statSync(join(p, "story.js")).isFile(); }
      catch { return false; }
    })
    .sort();
}

/** Stable serialization so key insertion order never masks a difference. */
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") {
    const out = {};
    for (const k of Object.keys(value).sort()) out[k] = canonical(value[k]);
    return out;
  }
  return value;
}

const digest = (state) => JSON.stringify(canonical(state));

const { reduce } = loadPlayer();

/** Walk from 0 to target in fixed steps, returning the final reduced state.
 *  reduce() is pure, so this models "played through" without a clock. */
function playThrough(story, target, step) {
  let state = reduce(story, 0);
  for (let t = step; t < target; t += step) state = reduce(story, t);
  return reduce(story, target);
}

const dirs = storyDirs();
let checks = 0;

for (const dir of dirs) {
  let story;
  try {
    story = loadStory(dir);
  } catch (err) {
    failures.push(`${dir}: failed to load — ${err.message}`);
    continue;
  }
  if (!story) { failures.push(`${dir}: story.js did not mount a story`); continue; }

  const marks = [
    0,
    ...(story.chapters || []).map((c) => c.at || 0),
    Math.max(0, (story.durationMs || 0) - 1),
    story.durationMs || 0,
  ];

  for (const t of [...new Set(marks)].sort((a, b) => a - b)) {
    const direct = digest(reduce(story, t));
    for (const step of [17, 100, 613]) {
      const stepped = digest(playThrough(story, t, step));
      checks++;
      if (direct !== stepped) {
        failures.push(
          `${dir} @ t=${t}ms (step ${step}ms): seek(t) !== play-through-to(t)\n` +
            `      seek:    ${direct.slice(0, 220)}\n` +
            `      stepped: ${stepped.slice(0, 220)}`,
        );
      }
    }
    // Backward seek must also converge: arriving from the END must equal
    // arriving from the start. This is the case msgOnce used to break.
    const fromEnd = (() => {
      reduce(story, story.durationMs || 0);
      return digest(reduce(story, t));
    })();
    checks++;
    if (direct !== fromEnd) {
      failures.push(`${dir} @ t=${t}ms: backward seek from end !== forward seek`);
    }
  }
}

// Structural guarantees the refactor exists to provide.
const playerSrc = readFileSync(join(storiesDir, "shared", "story-player.js"), "utf8");
const adapterSrc = readFileSync(join(storiesDir, "shared", "surface-adapters.js"), "utf8");
if (/\bmsgOnce\b/.test(playerSrc)) failures.push("story-player.js still references msgOnce");
if (/Math\.random\s*\(/.test(playerSrc)) failures.push("story-player.js uses Math.random");
if (/Math\.random\s*\(/.test(adapterSrc)) failures.push("surface-adapters.js uses Math.random");

if (failures.length) {
  console.error(`✗ seek determinism: ${failures.length} failure(s)`);
  for (const f of failures) console.error("  - " + f);
  process.exit(1);
}
console.log(`✓ seek determinism: ${checks} checks across ${dirs.length} stories; no caches, no random ids`);
