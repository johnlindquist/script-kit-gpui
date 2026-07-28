#!/usr/bin/env node
/**
 * Story schema validation (v2 legacy + v3).
 *
 * Enforces the invariants that keep stories usable as evidence:
 *  - action kinds are known (a typo must fail loudly, not silently no-op)
 *  - v3 stories declare covers/effort so coverage can be audited
 *  - chapters are ordered and inside the duration
 *  - AT MOST ONE queued turn, always at position 1 (the product decision is
 *    "one visible pending turn"; a story that shows two would be drawing a
 *    behavior the product rejected)
 *  - assertions declare a known kind and the selectors they need
 *
 * Usage: node design/mockups/tests/story-schema.test.mjs
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const storiesDir = join(here, "..", "stories");
const failures = [];

const LEGACY_KINDS = new Set([
  "showSurface", "hideSurface", "openOverlay", "closeOverlay", "ensureRows",
  "type", "setText", "setSelection", "walkSelection", "setFooterState",
  "setLines", "streamText", "appendMessage", "setSendState", "setTerminalLines",
  "pressKey", "pause",
]);
// v3 idempotent replacement kinds. Any add*/remove* kind is a schema error by
// construction: mutation verbs cannot survive arbitrary seeking.
const V3_KINDS = new Set([
  "setConversationMode", "setTurnState", "setQueuedTurn", "setContextGrants",
  "setContextReceipt", "setListRows", "setNotice", "setDocumentState",
  "setArtifactState",
]);
const ALL_KINDS = new Set([...LEGACY_KINDS, ...V3_KINDS]);
const ASSERTION_KINDS = new Set(["rectEquals", "actionKindsAbsent", "receiptAtChapter"]);
const EFFORTS = new Set(["new-design", "reskin", "hybrid", "comparison"]);

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

const dirs = readdirSync(storiesDir).filter((e) => {
  if (e === "shared" || e.startsWith(".")) return false;
  try { return statSync(join(storiesDir, e)).isDirectory() && statSync(join(storiesDir, e, "story.js")).isFile(); }
  catch { return false; }
}).sort();

let v3Count = 0;

for (const dir of dirs) {
  let s;
  try { s = loadStory(dir); } catch (e) { failures.push(`${dir}: ${e.message}`); continue; }
  if (!s) { failures.push(`${dir}: no story mounted`); continue; }
  const tag = (m) => failures.push(`${dir}: ${m}`);

  if (s.id !== dir) tag(`id "${s.id}" does not match directory`);
  if (!s.durationMs || s.durationMs <= 0) tag("missing positive durationMs");

  const chapters = s.chapters || [];
  if (!chapters.length) tag("no chapters");
  let prev = -1;
  for (const c of chapters) {
    if (!c.id || !c.label) tag(`chapter missing id/label: ${JSON.stringify(c)}`);
    const at = c.at || 0;
    if (at < prev) tag(`chapters out of order at "${c.id}" (${at} after ${prev})`);
    if (at > s.durationMs) tag(`chapter "${c.id}" at ${at}ms exceeds duration ${s.durationMs}ms`);
    prev = at;
  }

  const actions = s.actions || [];
  for (const a of actions) {
    if (!ALL_KINDS.has(a.kind)) tag(`unknown action kind "${a.kind}"`);
    if (/^(add|remove|append|delete)[A-Z]/.test(a.kind) && !LEGACY_KINDS.has(a.kind)) {
      tag(`mutation-verb action kind "${a.kind}" — v3 requires idempotent set*`);
    }
    if ((a.at || 0) > s.durationMs) tag(`action "${a.kind}" at ${a.at}ms exceeds duration`);
  }

  // One visible pending turn — the settled product rule.
  for (const a of actions.filter((x) => x.kind === "setQueuedTurn")) {
    if (a.turn && a.turn.position !== 1) {
      tag(`setQueuedTurn position must be 1, saw ${a.turn.position}`);
    }
    if (Array.isArray(a.turn)) tag("setQueuedTurn must carry one turn or null, never an array");
  }

  if (s.storyVersion === 3) {
    v3Count++;
    if (!Array.isArray(s.covers) || !s.covers.length) tag("v3 story must declare covers[]");
    if (!EFFORTS.has(s.effort)) tag(`v3 story effort must be one of ${[...EFFORTS].join("|")}`);
    for (const as of s.assertions || []) {
      if (!as.id) tag("assertion missing id");
      if (!ASSERTION_KINDS.has(as.kind)) tag(`unknown assertion kind "${as.kind}"`);
      if (as.kind === "rectEquals") {
        if (!as.baselineChapter) tag(`${as.id}: rectEquals needs baselineChapter`);
        if (!(as.selectors || []).length) tag(`${as.id}: rectEquals needs selectors`);
        const ids = chapters.map((c) => c.id);
        for (const ch of [as.baselineChapter, ...(as.atChapters || [])]) {
          if (!ids.includes(ch)) tag(`${as.id}: references unknown chapter "${ch}"`);
        }
      }
    }
    for (const sn of s.snapshots || []) {
      if (!chapters.some((c) => c.id === sn.chapter)) {
        tag(`snapshot references unknown chapter "${sn.chapter}"`);
      }
    }
  }
}

if (failures.length) {
  console.error(`✗ story schema: ${failures.length} problem(s)`);
  for (const f of failures) console.error("  - " + f);
  process.exit(1);
}
console.log(`✓ story schema: ${dirs.length} stories valid (${v3Count} at schema v3)`);
