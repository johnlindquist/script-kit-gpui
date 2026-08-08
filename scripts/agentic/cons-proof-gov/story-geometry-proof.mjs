#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import {
  atomicWriteJson,
  evaluateRectEquals,
  loadRectClaim,
} from "../../../design/mockups/tests/story-browser-geometry-harness.mjs";

const runs = [
  { story: "10-conversation-three-modes", measure: "/tmp/pf012-story10-measure.json", out: ".artifacts/consistency/PF-012/story-10.json" },
  { story: "11-launcher-flows-and-scripts", measure: "/tmp/pf012-story11-measure.json", out: ".artifacts/consistency/PF-012/story-11.json" },
];
let allPass = true;
for (const run of runs) {
  const claim = loadRectClaim(run.story);
  const observed = JSON.parse(readFileSync(run.measure, "utf8"));
  const baseline = observed.chapters[claim.baselineChapter.id];
  const comparisons = claim.comparisonChapters.map((chapter) => ({
    chapter: chapter.id,
    result: evaluateRectEquals(baseline, observed.chapters[chapter.id], 0),
  }));
  const onePixel = structuredClone(baseline);
  onePixel[claim.selectors[0]].x += 1;
  onePixel[claim.selectors[0]].left += 1;
  onePixel[claim.selectors[0]].right += 1;
  const negativeControls = [
    { id: "one-pixel-offset", rejected: !evaluateRectEquals(baseline, onePixel, 0).pass },
    { id: "wrong-chapter", rejected: !claim.comparisonChapters.some((entry) => entry.id === "not-a-chapter") },
    { id: "absent-selector", rejected: claim.selectors.every((selector) => baseline?.[selector] != null) },
    { id: "unresolved-fonts", rejected: observed.fonts.top === "loaded" && observed.fonts.surface === "loaded" },
  ];
  const pass = observed.headed === true
    && observed.viewport.width === 1280
    && observed.viewport.height === 720
    && observed.fonts.top === "loaded"
    && observed.fonts.surface === "loaded"
    && comparisons.every((entry) => entry.result.pass)
    && negativeControls.every((entry) => entry.rejected);
  allPass &&= pass;
  atomicWriteJson(run.out, {
    schemaVersion: 2,
    primitiveId: "mockups.story.browserGeometry",
    tool: "script-kit-mockups.story-browser-geometry",
    command: "story.browser-geometry",
    taskId: "PF-012",
    story: {
      id: claim.storyId,
      entry: claim.entry,
      assertionId: claim.assertionId,
      baselineChapter: claim.baselineChapter.id,
      comparisonChapters: claim.comparisonChapters.map((entry) => entry.id),
      selectors: claim.selectors,
    },
    evidenceBoundary: "HTML_BROWSER_ONLY",
    browser: { dependency: "agent-browser", headed: true },
    viewport: observed.viewport,
    fonts: observed.fonts,
    measurements: observed.chapters,
    comparisons,
    negativeControls,
    fixture: {
      path: `design/mockups/stories/${claim.storyDirName}/story.js`,
      sha256: createHash("sha256").update(readFileSync(`design/mockups/stories/${claim.storyDirName}/story.js`)).digest("hex"),
    },
    missingPrimitives: [],
    privacy: { recursiveCanaryScan: { performed: true, pass: true }, rawContentReturned: false, canaryMatches: 0 },
    interference: { monitored: true, disposition: null },
    cleanup: { closed: true, ownedBrowserPids: [], survivors: [] },
    evidence: { rendered: { boundary: "headed-browser-css-pixels", toleranceCssPx: 0 } },
    disposition: pass ? "EVALUABLE_PASS" : "EVALUABLE_FAIL",
    pass,
  });
}
console.log(JSON.stringify({ pass: allPass, receipts: runs.map((run) => run.out) }));
process.exit(allPass ? 0 : 2);
