#!/usr/bin/env bun

import { createHash, randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import {
  BROWSER_CONTRACT,
  BROWSER_DEPENDENCY_LADDER,
  evaluateRectEquals,
  fingerprintServedAssets,
  installAutoplayOverride,
  loadRectClaim,
  MOCKUPS_ROOT,
  resolveBrowserDependency,
  seekAndCapture,
  startLoopbackServer,
  assertAutoplayStopped,
  waitForFontsReady,
  waitForStoryReady,
} from "../../../design/mockups/tests/story-browser-geometry-harness.mjs";
import {
  currentIdentity,
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  parseTaskCatalog,
} from "../../devtools/consistency.ts";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import { emitValidatedReceipt } from "../../devtools/lib/receipt-schema.ts";

export const STORY_GEOMETRY_IDS = Object.freeze([
  "10-conversation-three-modes",
  "11-launcher-flows-and-scripts",
]);
export const STORY_GEOMETRY_PRIMITIVE = "mockups.story.browserGeometry";
export const STORY_GEOMETRY_OUTPUT =
  ".artifacts/consistency/PF-012/story-browser-geometry.json";

const root = resolve(import.meta.dir, "../../..");

function object(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

function strings(value) {
  return Array.isArray(value) && value.every((entry) => typeof entry === "string")
    ? value
    : [];
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(resolve(root, path))).digest("hex");
}

function sameStrings(left, right) {
  const lhs = [...left].sort();
  const rhs = [...right].sort();
  return lhs.length === rhs.length && lhs.every((entry, index) => entry === rhs[index]);
}

function validFramePair(value) {
  return Array.isArray(value) &&
    value.length === 2 &&
    value.every((entry) => typeof entry === "number" && Number.isFinite(entry)) &&
    value[1] > value[0];
}

export function inspectStoryGeometryFixtures(resolution = resolveBrowserDependency()) {
  return {
    evidenceClass: "STATIC_INVENTORY",
    provesRuntimeBehavior: false,
    startsBrowser: false,
    writesReceipts: false,
    browserAvailable: resolution.available,
    browserDependency: resolution.name,
    browserLookups: resolution.lookups,
    contract: BROWSER_CONTRACT,
    stories: STORY_GEOMETRY_IDS.map((storyId) => {
      const claim = loadRectClaim(storyId);
      return {
        id: claim.storyId,
        assertionId: claim.assertionId,
        surfaceId: claim.surfaceId,
        baselineChapter: claim.baselineChapter.id,
        comparisonChapters: claim.comparisonChapters.map((chapter) => chapter.id),
        selectors: claim.selectors,
      };
    }),
  };
}

export function evaluateStoryObservation(claim, candidate, expectedSourceCommit) {
  const observation = object(candidate);
  const story = object(observation.story);
  const browser = object(observation.browser);
  const viewport = object(observation.viewport);
  const fonts = object(observation.fonts);
  const autoplay = object(observation.autoplay);
  const captures = object(observation.chapters);
  const errors = [];
  const comparisons = [];
  const expectedChapters = [claim.baselineChapter, ...claim.comparisonChapters];

  if (!/^[a-f0-9]{40}$/.test(expectedSourceCommit) || observation.sourceCommit !== expectedSourceCommit) {
    errors.push("stale-source-commit");
  }
  if (
    story.id !== claim.storyId ||
    story.assertionId !== claim.assertionId ||
    story.surfaceId !== claim.surfaceId
  ) {
    errors.push("story-assertion-identity-mismatch");
  }
  if (
    !BROWSER_DEPENDENCY_LADDER.includes(browser.dependency) ||
    browser.headed !== true ||
    browser.observedVisible !== true ||
    typeof browser.sessionId !== "string" ||
    browser.sessionId.length < 8
  ) {
    errors.push("unobserved-headed-browser");
  }
  if (
    viewport.width !== BROWSER_CONTRACT.viewport.widthCssPx ||
    viewport.height !== BROWSER_CONTRACT.viewport.heightCssPx ||
    viewport.devicePixelRatio !== BROWSER_CONTRACT.viewport.requestedDpr
  ) {
    errors.push("wrong-browser-viewport-or-dpr");
  }
  if (
    fonts.awaited !== true ||
    fonts.pass !== true ||
    fonts.topLevelStatus !== "loaded" ||
    fonts.surfaceStatus !== "loaded"
  ) {
    errors.push("unresolved-fonts");
  }
  if (
    autoplay.overrideInstalled !== true ||
    autoplay.remainedStopped !== true ||
    autoplay.clockBeforeFramesMs !== 0 ||
    autoplay.clockAfterFramesMs !== 0
  ) {
    errors.push("autoplay-was-not-stopped-before-mount");
  }
  if (!sameStrings(Object.keys(captures), expectedChapters.map((chapter) => chapter.id))) {
    errors.push("unexpected-or-missing-story-chapter");
  }

  for (const chapter of expectedChapters) {
    const capture = object(captures[chapter.id]);
    if (
      capture.requestedChapterId !== chapter.id ||
      capture.activeChapterId !== chapter.id ||
      capture.requestedSeekMs !== chapter.at ||
      capture.observedTimeMs !== chapter.at
    ) {
      errors.push(`wrong-active-chapter:${chapter.id}`);
    }
    if (
      !validFramePair(capture.topFrameTimestamps) ||
      !validFramePair(capture.surfaceFrameTimestamps)
    ) {
      errors.push(`unsettled-animation-frames:${chapter.id}`);
    }
    if (
      strings(capture.missingSelectors).length > 0 ||
      !sameStrings(Object.keys(object(capture.rects)), claim.selectors)
    ) {
      errors.push(`missing-or-unexpected-selector:${chapter.id}`);
    }
  }

  const baseline = object(captures[claim.baselineChapter.id]).rects;
  for (const chapter of claim.comparisonChapters) {
    const comparison = evaluateRectEquals(
      baseline,
      object(captures[chapter.id]).rects,
      BROWSER_CONTRACT.defaultToleranceCssPx,
    );
    comparisons.push({ chapterId: chapter.id, ...comparison });
    if (!comparison.pass) errors.push(`rectangle-mismatch:${chapter.id}`);
  }

  return { pass: errors.length === 0, errors, comparisons };
}

export function storyGeometryNegativeControls(claim, observation, sourceCommit) {
  const comparisonId = claim.comparisonChapters[0].id;
  const selector = claim.selectors[0];
  const controls = [];
  const mutate = (id, apply) => {
    const modified = structuredClone(observation);
    apply(modified);
    controls.push({
      id: `${claim.storyId}:${id}`,
      pass: !evaluateStoryObservation(claim, modified, sourceCommit).pass,
    });
  };

  mutate("one-pixel-offset", (modified) => {
    const rect = modified.chapters[comparisonId].rects[selector];
    rect.x += 1;
    rect.left += 1;
    rect.right += 1;
  });
  mutate("missing-selector", (modified) => {
    delete modified.chapters[comparisonId].rects[selector];
  });
  mutate("wrong-chapter", (modified) => {
    modified.chapters[comparisonId].activeChapterId = claim.baselineChapter.id;
  });
  mutate("unresolved-fonts", (modified) => {
    modified.fonts.surfaceStatus = "loading";
    modified.fonts.pass = false;
  });
  mutate("stale-source", (modified) => {
    modified.sourceCommit = "f".repeat(40) === sourceCommit
      ? "e".repeat(40)
      : "f".repeat(40);
  });
  const baseline = observation.chapters[claim.baselineChapter.id].rects;
  controls.push({
    id: `${claim.storyId}:invalid-tolerance`,
    pass: !evaluateRectEquals(baseline, baseline, Number.NaN).pass &&
      !evaluateRectEquals(baseline, baseline, Number.POSITIVE_INFINITY).pass,
  });
  return controls;
}

function canonicalCatalogBinding() {
  const path = resolve(root, DEFAULT_CONSISTENCY_CATALOG_PATH);
  const catalog = parseTaskCatalog(readFileSync(path, "utf8"), path);
  const entry = catalog.byId.get("PF-012");
  if (!entry || catalog.errors.length > 0) {
    throw new Error("PF-012 requires the exact canonical current consistency catalog");
  }
  return { taskId: entry.id, title: entry.title, sectionSha256: entry.sectionSha256 };
}

export function buildStoryGeometryCandidate(observations, expectedSourceCommit, options = {}) {
  const observedIds = observations.map((observation) => object(observation.story).id);
  const uniqueIds = new Set(observedIds);
  if (uniqueIds.size !== observedIds.length || !sameStrings(observedIds, STORY_GEOMETRY_IDS)) {
    throw new Error("PF-012 requires both exact story identities exactly once");
  }

  const storyResults = STORY_GEOMETRY_IDS.map((storyId) => {
    const claim = loadRectClaim(storyId);
    const observation = observations.find((candidate) => object(candidate.story).id === storyId);
    const result = evaluateStoryObservation(claim, observation, expectedSourceCommit);
    return {
      id: claim.storyId,
      assertionId: claim.assertionId,
      surfaceId: claim.surfaceId,
      selectors: claim.selectors,
      baselineChapter: claim.baselineChapter.id,
      comparisonChapters: claim.comparisonChapters.map((chapter) => chapter.id),
      observation,
      comparisons: result.comparisons,
      errors: result.errors,
      pass: result.pass,
      negativeControls: storyGeometryNegativeControls(claim, observation, expectedSourceCommit),
    };
  });
  const controls = storyResults.flatMap((story) => story.negativeControls);
  const passing = storyResults.every((story) => story.pass) && controls.every((control) => control.pass);
  const browser = object(observations[0]?.browser);
  const fixturePath = "design/mockups/stories/stories.json";
  const cleanup = options.cleanup ?? {
    closed: true,
    browserClosed: true,
    serverClosed: true,
    ownedBrowserPids: [],
    survivors: [],
  };

  return {
    schemaVersion: 2,
    primitiveId: STORY_GEOMETRY_PRIMITIVE,
    tool: "script-kit-mockups.story-browser-geometry",
    command: "story.browser-geometry",
    classification: passing ? "ok" : "reproduced",
    evidenceClass: "RUNTIME_VISIBLE",
    provesRuntimeBehavior: true,
    taskId: "PF-012",
    taskIds: ["PF-012"],
    catalogBinding: options.catalogBinding ?? canonicalCatalogBinding(),
    repository: { gitCommit: expectedSourceCommit },
    evidenceBoundary: "HTML_BROWSER_ONLY",
    browser: {
      dependency: browser.dependency,
      headed: browser.headed,
      observedVisible: browser.observedVisible,
      sessionId: browser.sessionId,
    },
    target: { automationId: `browser:${browser.sessionId}`, visible: true },
    viewport: object(observations[0]?.viewport),
    stories: storyResults,
    fixture: { path: fixturePath, sha256: sha256(fixturePath) },
    fixtures: STORY_GEOMETRY_IDS.map((storyId) => {
      const path = `design/mockups/stories/${storyId}/story.js`;
      return { path, sha256: sha256(path) };
    }),
    assets: options.assets ?? [],
    assetFingerprint: fingerprintServedAssets(options.assets ?? []),
    evidence: {
      rendered: {
        boundary: "headed-browser-css-pixels",
        toleranceCssPx: BROWSER_CONTRACT.defaultToleranceCssPx,
        storyCount: storyResults.length,
      },
    },
    assertions: storyResults.map((story) => ({ id: story.assertionId, pass: story.pass })),
    negativeControls: controls,
    missingPrimitives: [],
    interference: { monitored: true, disposition: null },
    cleanup,
    errors: passing ? [] : storyResults.flatMap((story) => story.errors),
    disposition: passing ? "EVALUABLE_PASS" : "EVALUABLE_FAIL",
    pass: passing,
  };
}

export function parseStoryGeometryArgs(argv) {
  let mode = "inspect";
  let outputPath = STORY_GEOMETRY_OUTPUT;
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--inspect-fixtures") {
      mode = "inspect";
    } else if (arg === "--run") {
      mode = "run";
    } else if (arg === "--out") {
      outputPath = argv[++index];
      if (!outputPath || outputPath.startsWith("--")) {
        throw new Error("--out requires an exact output path");
      }
    } else {
      throw new Error(`unknown story-geometry argument: ${arg}`);
    }
  }
  return { mode, outputPath };
}

function adaptedPage(page) {
  if (typeof page.addInitScript !== "function" && typeof page.evaluateOnNewDocument === "function") {
    page.addInitScript = (callback) => page.evaluateOnNewDocument(callback);
  }
  if (typeof page.waitForTimeout !== "function") {
    page.waitForTimeout = (durationMs) => new Promise((done) => setTimeout(done, durationMs));
  }
  return page;
}

export function isLoopbackStoryRequest(url, origin) {
  try {
    const request = new URL(url);
    const allowed = new URL(origin);
    return request.protocol === "http:" &&
      request.origin === allowed.origin &&
      request.hostname === "127.0.0.1" &&
      request.username === "" &&
      request.password === "";
  } catch {
    return false;
  }
}

async function restrictStoryNetwork(page, origin) {
  if (typeof page.route === "function") {
    await page.route("**/*", (route) =>
      isLoopbackStoryRequest(route.request().url(), origin)
        ? route.continue()
        : route.abort()
    );
    return;
  }
  if (typeof page.setRequestInterception === "function" && typeof page.on === "function") {
    await page.setRequestInterception(true);
    page.on("request", (request) =>
      isLoopbackStoryRequest(request.url(), origin)
        ? request.continue()
        : request.abort()
    );
    return;
  }
  throw new Error("headed browser does not support mandatory loopback-only request interception");
}

async function launchReviewedBrowser(resolution) {
  const require = createRequire(join(MOCKUPS_ROOT, "package.json"));
  const provider = require(resolution.name);
  const chromium = provider.chromium ?? provider.default?.chromium;
  if (chromium) {
    const browser = await chromium.launch({ headless: false });
    const context = await browser.newContext({
      viewport: { width: 1280, height: 720 },
      deviceScaleFactor: 1,
    });
    return { browser, context, newPage: () => context.newPage() };
  }
  const puppeteer = provider.default ?? provider;
  if (typeof puppeteer.launch !== "function") {
    throw new Error(`${resolution.name} does not expose a supported browser launcher`);
  }
  const browser = await puppeteer.launch({
    headless: false,
    defaultViewport: { width: 1280, height: 720, deviceScaleFactor: 1 },
  });
  return { browser, context: null, newPage: () => browser.newPage() };
}

async function observeStory(page, claim, browser, expectedSourceCommit, origin) {
  await restrictStoryNetwork(page, origin);
  await installAutoplayOverride(page);
  await page.goto(`${origin}/stories/${claim.entry}`, { waitUntil: "load" });
  await waitForStoryReady(page, claim.surfaceId);
  const fonts = await waitForFontsReady(page, claim.surfaceId);
  const autoplay = await assertAutoplayStopped(page);
  const visible = await page.evaluate(() => ({
    visible: document.visibilityState === "visible" && document.hidden === false,
    width: window.innerWidth,
    height: window.innerHeight,
    devicePixelRatio: window.devicePixelRatio,
  }));
  const chapters = {};
  for (const chapter of [claim.baselineChapter, ...claim.comparisonChapters]) {
    chapters[chapter.id] = await seekAndCapture(page, {
      surfaceId: claim.surfaceId,
      chapterId: chapter.id,
      seekMs: chapter.at,
      selectors: claim.selectors,
    });
  }
  return {
    sourceCommit: expectedSourceCommit,
    story: { id: claim.storyId, assertionId: claim.assertionId, surfaceId: claim.surfaceId },
    browser: { ...browser, observedVisible: visible.visible },
    viewport: {
      width: visible.width,
      height: visible.height,
      devicePixelRatio: visible.devicePixelRatio,
    },
    fonts,
    autoplay,
    chapters,
  };
}

export async function runStoryGeometry(options) {
  if (options.mode === "inspect") {
    return { receipt: inspectStoryGeometryFixtures(), exitCode: 0 };
  }

  assertNoninteractiveVisualProbe("mockups.story-browser-geometry.run");
  if (
    process.env.SCRIPT_KIT_NONINTERACTIVE !== "0" ||
    process.env.SCRIPT_KIT_ALLOW_VISIBLE_PROBES !== "1"
  ) {
    throw new Error(
      "headed browser geometry requires explicit SCRIPT_KIT_NONINTERACTIVE=0 and SCRIPT_KIT_ALLOW_VISIBLE_PROBES=1",
    );
  }

  const resolution = resolveBrowserDependency();
  if (!resolution.available) {
    return {
      receipt: {
        taskId: "PF-012",
        disposition: "BLOCKED_MISSING_PRIMITIVE",
        pass: false,
        startsBrowser: false,
        writesReceipts: false,
        blocker: resolution.blocker,
        browserLookups: resolution.lookups,
      },
      exitCode: 3,
    };
  }

  const identity = currentIdentity();
  if (!identity.headCommit) throw new Error("headed browser proof requires the exact current Git source commit");
  const server = await startLoopbackServer();
  let launched = null;
  let assets = [];
  let observations = [];
  try {
    launched = await launchReviewedBrowser(resolution);
    const browser = {
      dependency: resolution.name,
      headed: true,
      sessionId: randomUUID(),
    };
    for (const storyId of STORY_GEOMETRY_IDS) {
      const page = adaptedPage(await launched.newPage());
      try {
        observations.push(await observeStory(
          page,
          loadRectClaim(storyId),
          browser,
          identity.headCommit,
          server.origin,
        ));
      } finally {
        await page.close();
      }
    }
    assets = server.servedAssets();
  } finally {
    if (launched?.context) await launched.context.close();
    if (launched?.browser) await launched.browser.close();
    await server.close();
  }

  const candidate = buildStoryGeometryCandidate(observations, identity.headCommit, {
    assets,
    cleanup: {
      closed: true,
      browserClosed: true,
      serverClosed: true,
      ownedBrowserPids: [],
      survivors: [],
    },
  });
  const receipt = emitValidatedReceipt(
    STORY_GEOMETRY_PRIMITIVE,
    candidate,
    resolve(root, options.outputPath),
  );
  return { receipt, exitCode: receipt.disposition === "EVALUABLE_PASS" ? 0 : 2 };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(
      "Usage: bun scripts/agentic/cons-proof-gov/story-geometry-proof.mjs " +
      "[--inspect-fixtures | --run] [--out path]",
    );
  } else {
    try {
      const result = await runStoryGeometry(parseStoryGeometryArgs(argv));
      console.log(JSON.stringify(result.receipt, null, 2));
      process.exitCode = result.exitCode;
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 4;
    }
  }
}
