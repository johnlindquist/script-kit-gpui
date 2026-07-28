#!/usr/bin/env node
/**
 * Gating test for continuous DevTools-style user-story mockups.
 * Asserts:
 * - the manifest count matches the number of story directories on disk
 *   (previously hardcoded to 9, which is the same class of lie that
 *   build-manifest.mjs was written to eliminate)
 * - each story embeds real screen fixtures via embed=story iframes
 * - parent HTML has no cloned .sk-window app DOM / no .sk-footer-btn
 * - story.js mounts StoryPlayer with continuous action kinds
 * - type actions yield ≥3 distinct prefixes via StoryPlayer.reduce
 * - index links all nine by title
 * - lint-mockups.mjs is green
 */
import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import vm from "node:vm";

const __dirname = dirname(fileURLToPath(import.meta.url));
const storiesRoot = join(__dirname, "..", "stories");
const failures = [];
const assert = (c, m) => {
  if (!c) failures.push(m);
};

const manifest = JSON.parse(readFileSync(join(storiesRoot, "stories.json"), "utf8"));
assert(manifest.schemaVersion === 2, `schemaVersion 2 required, got ${manifest.schemaVersion}`);
const storyDirCount = readdirSync(storiesRoot).filter((e) => {
  if (e === "shared" || e.startsWith(".")) return false;
  try {
    return statSync(join(storiesRoot, e)).isDirectory() && statSync(join(storiesRoot, e, "story.js")).isFile();
  } catch {
    return false;
  }
}).length;
assert(
  manifest.count === storyDirCount,
  `manifest count ${manifest.count} != ${storyDirCount} story directories on disk`,
);
assert(manifest.architecture === "continuous-iframe-timeline", "architecture marker missing");
assert(!!manifest.buildId, "buildId required");

const indexHtml = readFileSync(join(storiesRoot, "index.html"), "utf8");
assert(!indexHtml.includes(".sk-footer-btn"), "index must not mention sk-footer-btn");

// Load StoryPlayer.reduce in VM
const playerCode = readFileSync(join(storiesRoot, "shared", "story-player.js"), "utf8");
const sandbox = { console, requestAnimationFrame: () => 1, cancelAnimationFrame: () => {}, document: { hidden: false } };
sandbox.globalThis = sandbox;
sandbox.window = sandbox;
vm.runInNewContext(playerCode, sandbox);
assert(sandbox.StoryPlayer && typeof sandbox.StoryPlayer.reduce === "function", "StoryPlayer.reduce missing");
assert(playerCode.includes("requestAnimationFrame"), "player must use rAF");
assert(!playerCode.includes("setInterval("), "player must not use setInterval slideshow");

const CONTINUOUS = new Set(["type", "walkSelection", "streamText", "setTerminalLines", "setLines"]);

for (const story of manifest.stories) {
  const entry = join(storiesRoot, story.entry);
  assert(existsSync(entry), `missing ${story.entry}`);
  const html = readFileSync(entry, "utf8");
  assert(indexHtml.includes(story.title), `index missing title ${story.title}`);
  assert(
    indexHtml.includes(`./${story.id}/index.html`),
    `index missing href ${story.id}`,
  );
  // No full-scene slideshow
  assert(!html.includes('data-scene="'), `${story.id}: must not use data-scene slideshow`);
  assert(!html.includes("sk-footer-btn"), `${story.id}: forbidden .sk-footer-btn`);
  assert(!html.includes('class="sk-window"'), `${story.id}: parent must not clone .sk-window`);
  // Real fixtures
  const iframes = [...html.matchAll(/data-story-surface="([^"]+)"[^>]*src="([^"]+)"/g)];
  assert(iframes.length >= 1, `${story.id}: needs ≥1 iframe surface`);
  for (const [, id, src] of iframes) {
    assert(src.includes("screens/"), `${story.id}: surface ${id} must point at screens/`);
    assert(src.includes("embed=story"), `${story.id}: surface ${id} must use ?embed=story`);
    // resolve path
    const rel = src.split("?")[0];
    const abs = join(dirname(entry), rel);
    assert(existsSync(abs), `${story.id}: broken fixture ${src} -> ${abs}`);
  }
  // story.js continuous actions
  const jsPath = join(storiesRoot, story.id, "story.js");
  assert(existsSync(jsPath), `${story.id}: missing story.js`);
  const js = readFileSync(jsPath, "utf8");
  assert(js.includes("StoryPlayer.mount"), `${story.id}: must mount StoryPlayer`);
  // Extract story object from the shipped story.js (object literal, not JSON)
  let def = null;
  try {
    def = new Function(
      js
        .replace(/\(function\s*\(\)\s*\{/, "")
        .replace(/window\.StoryPlayer\.mount\([\s\S]*$/m, "return story;")
        .replace(/\}\)\(\);\s*$/, ""),
    )();
  } catch (e) {
    failures.push(`${story.id}: cannot evaluate story.js (${e.message})`);
    continue;
  }
  assert(def && def.actions, `${story.id}: story definition missing actions`);
  const kinds = new Set((def.actions || []).map((a) => a.kind));
  const hasContinuous = [...kinds].some((k) => CONTINUOUS.has(k));
  assert(hasContinuous, `${story.id}: needs continuous action (type/walkSelection/stream/…), got ${[...kinds]}`);

  // type prefixes ≥3 distinct when type action present
  const typeActs = (def.actions || []).filter((a) => a.kind === "type" && a.text && a.text.length >= 3);
  if (typeActs.length) {
    const act = typeActs[0];
    const prefixes = new Set();
    for (let p = 0; p <= 1.001; p += 0.1) {
      prefixes.add(sandbox.StoryPlayer.typePrefix(act.text, p));
    }
    assert(prefixes.size >= 3, `${story.id}: type action must yield ≥3 prefixes, got ${prefixes.size}`);
  }

  // reduce produces changing digests over time
  const digests = new Set();
  for (let t = 0; t <= (def.durationMs || 5000); t += 200) {
    const st = sandbox.StoryPlayer.reduce(def, t);
    digests.add(JSON.stringify(st.semantic));
  }
  assert(digests.size >= 3, `${story.id}: reduce(t) must change semantic state (≥3 samples), got ${digests.size}`);
}

// Resolve shared assets
for (const f of [
  "shared/story-player.js",
  "shared/surface-adapters.js",
  "shared/story-shell.css",
]) {
  assert(existsSync(join(storiesRoot, f)), `missing ${f}`);
}

const lint = spawnSync(process.execPath, [join(__dirname, "lint-mockups.mjs")], {
  encoding: "utf8",
});
assert(lint.status === 0, `lint-mockups failed:\n${lint.stdout}\n${lint.stderr}`);

if (failures.length) {
  console.error(`✗ stories inventory: ${failures.length} failure(s)`);
  failures.forEach((f) => console.error("  -", f));
  process.exit(1);
}
console.log(
  `✓ stories inventory v2: ${manifest.stories.length} continuous iframe stories, reduce+type gates, lint green`,
);
