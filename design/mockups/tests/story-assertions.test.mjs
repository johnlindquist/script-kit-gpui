#!/usr/bin/env node
/**
 * Story assertion runner.
 *
 * HONESTY BOUNDARY — read this before trusting a green result.
 * ------------------------------------------------------------
 * Assertions split into what Node can prove and what it cannot:
 *
 *   actionKindsAbsent   PROVEN here (pure inspection of the action list)
 *   receiptAtChapter    PROVEN here (evaluated from the reduced state)
 *   rectEquals          NOT PROVEN here — bounding boxes require layout.
 *
 * jsdom is not a dependency of this repo (decision rule DR2), and even with it
 * jsdom does not do real CSS layout, so a "passing" rect check in Node would be
 * a lie. What IS checked for rectEquals is the necessary structural
 * precondition: every selector exists in the fixture, and no action in the
 * story can change the shell's structure (no surface swaps, no adds/removes of
 * the asserted nodes). That converts rectEquals from unproven to
 * "structurally sound, pixel proof pending a browser probe", and the runner
 * reports it as PENDING rather than counting it as a pass.
 *
 * Usage: node design/mockups/tests/story-assertions.test.mjs
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const mockups = join(here, "..");
const storiesDir = join(mockups, "stories");
const failures = [];
const pending = [];
let proven = 0;

function loadPlayer() {
  const src = readFileSync(join(storiesDir, "shared", "story-player.js"), "utf8");
  const sb = { window: {}, document: { querySelector: () => null, body: {} }, console };
  vm.createContext(sb);
  vm.runInContext(src, sb, { filename: "story-player.js" });
  return sb.window.StoryPlayer;
}

function loadStory(dir) {
  const src = readFileSync(join(storiesDir, dir, "story.js"), "utf8");
  const cap = {};
  const sb = {
    window: { StoryPlayer: { mount: (o) => { cap.story = o.story; } } },
    document: { querySelector: () => null, body: {} },
    console,
  };
  vm.createContext(sb);
  vm.runInContext(src, sb, { filename: `${dir}/story.js` });
  return cap.story;
}

const { reduce } = loadPlayer();

const dirs = readdirSync(storiesDir).filter((e) => {
  if (e === "shared" || e.startsWith(".")) return false;
  try { return statSync(join(storiesDir, e, "story.js")).isFile(); } catch { return false; }
}).sort();

for (const dir of dirs) {
  const story = loadStory(dir);
  if (!story || !story.assertions) continue;
  const chapterAt = (id) => {
    const c = (story.chapters || []).find((x) => x.id === id);
    return c ? c.at || 0 : null;
  };

  for (const as of story.assertions) {
    const label = `${dir} :: ${as.id}`;

    if (as.kind === "actionKindsAbsent") {
      const found = (story.actions || [])
        .filter((a) => (!as.surface || a.surface === as.surface) && as.kinds.includes(a.kind))
        .map((a) => a.kind);
      if (found.length) {
        failures.push(`${label}: forbidden action kind(s) present: ${[...new Set(found)].join(", ")}`);
      } else proven++;
      continue;
    }

    if (as.kind === "receiptAtChapter") {
      const at = chapterAt(as.chapter);
      if (at == null) { failures.push(`${label}: unknown chapter "${as.chapter}"`); continue; }
      // Evaluate at the END of the chapter's span, not its start. A chapter is
      // a phase, and its state is only fully realized once everything scheduled
      // inside it has run — a receipt for a streaming answer, for instance,
      // lands when the stream completes, not when it begins.
      const ordered = (story.chapters || []).map((c) => c.at || 0).sort((a, b) => a - b);
      const next = ordered.find((x) => x > at);
      const evalAt = (next != null ? next : story.durationMs || at) - 1;
      const state = reduce(story, Math.max(at, evalAt));
      const sem = (state.semantic || {})[as.surface] || {};
      const r = sem.receipt;
      if (!r) { failures.push(`${label}: no receipt in state at chapter "${as.chapter}"`); continue; }
      const actual = {
        attempted: (r.attempted || []).length,
        resolved: (r.resolved || []).length,
        failed: (r.failed || []).length,
        outcome: r.outcome,
      };
      const mismatch = Object.entries(as.expect).filter(([k, v]) => actual[k] !== v);
      if (mismatch.length) {
        failures.push(
          `${label}: receipt mismatch ${JSON.stringify(actual)} vs expected ${JSON.stringify(as.expect)}`,
        );
      } else proven++;
      continue;
    }

    if (as.kind === "rectEquals") {
      // Precondition 1: every asserted selector must exist in the fixture.
      const surface = (story.surfaces || []).find((s) => s.id === as.surface);
      const fixture = surface && (surface.fixture || surface.id);
      if (!fixture) { failures.push(`${label}: surface "${as.surface}" not declared`); continue; }
      let html;
      try {
        html = readFileSync(join(mockups, "screens", fixture, "index.html"), "utf8");
      } catch {
        failures.push(`${label}: fixture screens/${fixture}/index.html not found`);
        continue;
      }
      const missing = as.selectors.filter((sel) => {
        const attr = sel.match(/^\[([\w-]+)\]$/);
        if (attr) return !new RegExp(`\\b${attr[1]}\\b`).test(html);
        const cls = sel.match(/^\.([\w-]+)$/);
        // A class selector appears in markup as class="... name ...", not as ".name".
        if (cls) return !new RegExp(`class="[^"]*\\b${cls[1]}\\b`).test(html);
        return !html.includes(sel);
      });
      if (missing.length) {
        failures.push(`${label}: fixture is missing asserted selector(s): ${missing.join(", ")}`);
        continue;
      }
      // Precondition 2: nothing in the story may swap or hide the surface,
      // which would change geometry for reasons unrelated to mode.
      const structural = (story.actions || []).filter(
        (a) => a.surface === as.surface && ["showSurface", "hideSurface", "openOverlay", "closeOverlay"].includes(a.kind),
      );
      if (structural.length) {
        failures.push(
          `${label}: story performs structural surface actions (${structural.map((a) => a.kind).join(", ")}), so equal rects would not prove one shell`,
        );
        continue;
      }
      pending.push(
        `${label}: structurally sound (${as.selectors.length} selectors present, no surface swaps) — PIXEL PROOF PENDING a browser probe`,
      );
      continue;
    }

    if (as.kind === "fixtureTextAbsent") {
      // Content assertion: prove the fixture does not render a forbidden
      // pattern. Used for "flow titles must not leak the engine suffix",
      // which is a text fact, not a geometry fact — asserting it with
      // rectEquals would have been a name that lied about what it checked.
      const surface = (story.surfaces || []).find((s2) => s2.id === as.surface);
      const fixture = surface && (surface.fixture || surface.id);
      let html;
      try {
        html = readFileSync(join(mockups, "screens", fixture, "index.html"), "utf8");
      } catch {
        failures.push(`${label}: fixture screens/${fixture}/index.html not found`);
        continue;
      }
      // Only inspect rendered row titles, not comments explaining the rule.
      const titles = [...html.matchAll(/class="sk-list-row__name">([^<]*)</g)].map((m2) => m2[1]);
      const bad = titles.filter((t) => new RegExp(as.pattern).test(t));
      if (bad.length) {
        failures.push(`${label}: row title(s) match forbidden /${as.pattern}/: ${bad.join(", ")}`);
      } else if (!titles.length) {
        failures.push(`${label}: no row titles found in fixture — assertion would pass vacuously`);
      } else proven++;
      continue;
    }

    failures.push(`${label}: unhandled assertion kind "${as.kind}"`);
  }
}

for (const p of pending) console.log("  ⏳ " + p);

if (failures.length) {
  console.error(`✗ story assertions: ${failures.length} failure(s)`);
  for (const f of failures) console.error("  - " + f);
  process.exit(1);
}
console.log(`✓ story assertions: ${proven} proven in Node, ${pending.length} pending browser proof`);
