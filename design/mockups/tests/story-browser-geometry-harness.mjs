/**
 * PF-012 story browser-geometry harness (test-only, C08).
 *
 * Turns the two pending `rectEquals` story assertions (story 10
 * conversation.same-shell-rects, story 11
 * launcher.shell-geometry-stable-across-query) into HEADED-BROWSER pixel
 * proof. Everything here is HTML_BROWSER_ONLY evidence — it must never be
 * described as native GPUI or AppKit proof.
 *
 * Browser dependency contract (plan §3.1): the harness uses the repository's
 * EXISTING browser dependency resolved through the fixed ladder below —
 * playwright → @playwright/test → puppeteer → puppeteer-core. It never
 * installs anything and never substitutes jsdom/VM layout for browser
 * geometry. When no dependency resolves, callers must write a
 * BLOCKED_MISSING_PRIMITIVE receipt carrying the exact lookup evidence.
 *
 * Fixed browser contract: headed, 1280×720 CSS px, requested DPR 1,
 * loopback-only network, autoplay forced OFF before mount, rects measured in
 * the target surface iframe's CSS pixel space, default tolerance 0 CSS px.
 */

import { createHash } from "node:crypto";
import {
  createReadStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import http from "node:http";
import { createRequire } from "node:module";
import { dirname, extname, join, normalize, relative } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
export const MOCKUPS_ROOT = join(here, "..");
const STORIES_DIR = join(MOCKUPS_ROOT, "stories");

export const RECT_FIELDS = [
  "x",
  "y",
  "width",
  "height",
  "top",
  "right",
  "bottom",
  "left",
];

export const BROWSER_CONTRACT = {
  headed: true,
  viewport: { widthCssPx: 1280, heightCssPx: 720, requestedDpr: 1 },
  network: "loopback-only",
  autoplay: "disabled-before-mount",
  rectSpace: "target-surface-iframe-css-px",
  defaultToleranceCssPx: 0,
};

// ---------------------------------------------------------------------------
// Browser dependency resolution — fixed ladder, resolved from the REPOSITORY
// module space, hardcoded order. Never installs, never picks "whichever
// resolves first" outside this declared order.
// ---------------------------------------------------------------------------

export const BROWSER_DEPENDENCY_LADDER = [
  "playwright",
  "@playwright/test",
  "puppeteer",
  "puppeteer-core",
];

export function resolveBrowserDependency() {
  const require = createRequire(join(MOCKUPS_ROOT, "package.json"));
  const lookups = [];
  for (const name of BROWSER_DEPENDENCY_LADDER) {
    try {
      const resolved = require.resolve(name);
      let version = null;
      try {
        version = require(`${name}/package.json`).version ?? null;
      } catch {
        // version stays unknown; resolution alone selects the dependency
      }
      lookups.push({ name, resolved: true, path: resolved, version });
      return { available: true, name, version, lookups };
    } catch {
      lookups.push({ name, resolved: false, path: null, version: null });
    }
  }
  return {
    available: false,
    name: null,
    version: null,
    lookups,
    blocker:
      "repository has no usable existing headed-browser dependency (ladder: "
      + `${BROWSER_DEPENDENCY_LADDER.join(" → ")})`,
  };
}

// ---------------------------------------------------------------------------
// Claim loading — the rectEquals claims come from the STORY DEFINITIONS via
// the same VM convention as the existing tests, never reauthored by hand.
// ---------------------------------------------------------------------------

export function loadStoryDefinition(storyDirName) {
  const src = readFileSync(join(STORIES_DIR, storyDirName, "story.js"), "utf8");
  const captured = {};
  const sandbox = {
    window: { StoryPlayer: { mount: (options) => { captured.story = options.story; captured.mountOptions = options; } } },
    document: { querySelector: () => null, body: {} },
    console,
  };
  vm.createContext(sandbox);
  vm.runInContext(src, sandbox, { filename: `${storyDirName}/story.js` });
  if (!captured.story) {
    throw new Error(`story definition did not mount: ${storyDirName}`);
  }
  return captured;
}

export function loadRectClaim(storyDirName) {
  const { story, mountOptions } = loadStoryDefinition(storyDirName);
  const rectAssertions = (story.assertions ?? []).filter(
    (assertion) => assertion.kind === "rectEquals",
  );
  if (rectAssertions.length !== 1) {
    throw new Error(
      `${storyDirName}: expected exactly one rectEquals assertion, found ${rectAssertions.length}`,
    );
  }
  const assertion = rectAssertions[0];
  const chapterAt = (id) => {
    const chapter = (story.chapters ?? []).find((entry) => entry.id === id);
    if (!chapter) throw new Error(`${storyDirName}: unknown chapter "${id}"`);
    return chapter.at ?? 0;
  };
  const surface = (story.surfaces ?? []).find(
    (entry) => entry.id === assertion.surface,
  );
  if (!surface) {
    throw new Error(`${storyDirName}: surface "${assertion.surface}" not declared`);
  }
  const stories = JSON.parse(
    readFileSync(join(STORIES_DIR, "stories.json"), "utf8"),
  );
  const manifestEntry = (stories.stories ?? []).find(
    (entry) => entry.id === storyDirName,
  );
  if (!manifestEntry?.entry) {
    throw new Error(`${storyDirName}: no entry in stories.json`);
  }
  const precedingChapterOf = (id) => {
    const ordered = [...(story.chapters ?? [])].sort(
      (left, right) => (left.at ?? 0) - (right.at ?? 0),
    );
    const index = ordered.findIndex((entry) => entry.id === id);
    return index > 0 ? ordered[index - 1] : null;
  };
  return {
    storyId: story.id ?? storyDirName,
    storyDirName,
    entry: manifestEntry.entry,
    assertionId: assertion.id,
    assertionKind: "rectEquals",
    surfaceId: assertion.surface,
    selectors: [...assertion.selectors],
    baselineChapter: {
      id: assertion.baselineChapter,
      at: chapterAt(assertion.baselineChapter),
    },
    comparisonChapters: assertion.atChapters.map((id) => ({
      id,
      at: chapterAt(id),
      preceding: precedingChapterOf(id),
    })),
    mountAutoplay: mountOptions?.autoplay ?? null,
  };
}

// ---------------------------------------------------------------------------
// Loopback asset server — records every served asset (repo-relative path,
// SHA-256, byte length); refuses path escapes.
// ---------------------------------------------------------------------------

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".woff2": "font/woff2",
};

export async function startLoopbackServer(root = MOCKUPS_ROOT) {
  const served = new Map();
  const server = http.createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    const filePath = resolveLoopbackAssetPath(root, url.pathname);
    if (!filePath || !existsSync(filePath)
      || statSync(filePath).isDirectory()) {
      response.writeHead(404).end("not found");
      return;
    }
    const bytes = readFileSync(filePath);
    served.set(relative(root, filePath), {
      path: relative(join(root, ".."), filePath).replace(/^\.\.\//, ""),
      sha256: createHash("sha256").update(bytes).digest("hex"),
      sizeBytes: bytes.length,
    });
    response.writeHead(200, {
      "content-type": MIME[extname(filePath)] ?? "application/octet-stream",
    });
    response.end(bytes);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  return {
    server,
    port: address.port,
    origin: `http://127.0.0.1:${address.port}`,
    servedAssets: () =>
      [...served.values()].sort((left, right) =>
        left.path.localeCompare(right.path)
      ),
    close: () =>
      new Promise((resolve) => server.close(() => resolve(undefined))),
  };
}

export function resolveLoopbackAssetPath(root, pathname) {
  let relPath;
  try {
    relPath = decodeURIComponent(pathname).replace(/^\/+/, "");
  } catch {
    return null;
  }
  if (relPath.includes("\0")) return null;
  const normalizedRoot = normalize(root);
  const filePath = normalize(join(normalizedRoot, relPath));
  const lexical = relative(normalizedRoot, filePath);
  if (!lexical || lexical === ".." || lexical.startsWith("../")) return null;
  if (existsSync(filePath)) {
    const physical = relative(realpathSync(normalizedRoot), realpathSync(filePath));
    if (!physical || physical === ".." || physical.startsWith("../")) return null;
  }
  return filePath;
}

export function fingerprintServedAssets(assets) {
  return createHash("sha256")
    .update(
      assets
        .map((asset) => `${asset.path}\n${asset.sha256}\n${asset.sizeBytes}`)
        .join("\n"),
    )
    .digest("hex");
}

// ---------------------------------------------------------------------------
// In-page steps (framework-agnostic: they receive a playwright-style `page`)
// ---------------------------------------------------------------------------

/**
 * Install a test-only setter that wraps StoryPlayer.mount and forces
 * `autoplay: false` BEFORE page scripts execute. A plain ?autoplay=0 is
 * insufficient: the player gives explicit options.autoplay precedence and
 * stories 10/11 mount with autoplay: true.
 */
export async function installAutoplayOverride(page) {
  await page.addInitScript(() => {
    let storyPlayer;
    Object.defineProperty(window, "StoryPlayer", {
      configurable: true,
      get() {
        return storyPlayer;
      },
      set(next) {
        if (next && typeof next.mount === "function") {
          const originalMount = next.mount.bind(next);
          next.mount = (options) => {
            window.__SK_AUTOPLAY_FORCED_OFF__ = true;
            return originalMount({ ...options, autoplay: false });
          };
        }
        storyPlayer = next;
      },
    });
  });
}

export async function waitForStoryReady(page, surfaceId) {
  await page.waitForFunction((id) => {
    const api = window.__SK_STORY__;
    const surface = document.querySelector(`[data-story-surface="${id}"]`);
    if (!api || !surface) return false;
    if (surface.tagName === "IFRAME") {
      try {
        return surface.contentDocument?.readyState === "complete";
      } catch {
        return false;
      }
    }
    return true;
  }, surfaceId);
  // Match the player's post-load readiness margin.
  await page.waitForTimeout(30);
}

export async function waitForFontsReady(page, surfaceId, { awaitFonts = true } = {}) {
  if (!awaitFonts) {
    // Test-only bypass used by the fonts-bypass negative control. A skipped
    // fonts wait is an OBSERVER defect, never a geometry verdict.
    return {
      awaited: false,
      topLevelStatus: null,
      surfaceStatus: null,
      pass: false,
      reason: "fonts wait bypassed by test option",
    };
  }
  return page.evaluate(async (id) => {
    const surface = document.querySelector(`[data-story-surface="${id}"]`);
    if (!document.fonts) {
      return {
        awaited: false,
        topLevelStatus: null,
        surfaceStatus: null,
        pass: false,
        reason: "top-level FontFaceSet unavailable",
      };
    }
    await document.fonts.ready;
    let surfaceStatus = null;
    if (surface?.tagName === "IFRAME") {
      const doc = surface.contentDocument;
      if (!doc?.fonts) {
        return {
          awaited: true,
          topLevelStatus: document.fonts.status,
          surfaceStatus: null,
          pass: false,
          reason: "surface FontFaceSet unavailable",
        };
      }
      await doc.fonts.ready;
      surfaceStatus = doc.fonts.status;
    }
    return {
      awaited: true,
      topLevelStatus: document.fonts.status,
      surfaceStatus,
      pass: document.fonts.status === "loaded" && surfaceStatus === "loaded",
      reason: null,
    };
  }, surfaceId);
}

/** Prove the clock stays at zero across two animation frames pre-seek. */
export async function assertAutoplayStopped(page) {
  return page.evaluate(async () => {
    const api = window.__SK_STORY__;
    if (!api) throw new Error("window.__SK_STORY__ is missing");
    const before = api.getTime();
    await new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(resolve))
    );
    const after = api.getTime();
    return {
      overrideInstalled: window.__SK_AUTOPLAY_FORCED_OFF__ === true,
      clockBeforeFramesMs: before,
      clockAfterFramesMs: after,
      remainedStopped: before === 0 && after === 0,
    };
  });
}

/**
 * Seek to a chapter time, wait two top-level AND two surface-frame rAF
 * callbacks, then capture every selector's getBoundingClientRect inside the
 * target surface document (CSS px of that iframe).
 */
export async function seekAndCapture(page, { surfaceId, chapterId, seekMs, selectors }) {
  return page.evaluate(
    async ({ surfaceId, chapterId, seekMs, selectors }) => {
      const api = window.__SK_STORY__;
      if (!api) throw new Error("window.__SK_STORY__ is missing");
      api.pause();
      api.seek(seekMs);
      const topFrames = await new Promise((resolve) => {
        requestAnimationFrame((first) => {
          requestAnimationFrame((second) => resolve([first, second]));
        });
      });
      const surface = document.querySelector(
        `[data-story-surface="${surfaceId}"]`,
      );
      const doc = surface?.tagName === "IFRAME"
        ? surface.contentDocument
        : surface?.ownerDocument;
      if (!doc) throw new Error(`surface document missing: ${surfaceId}`);
      const surfaceFrames = await new Promise((resolve) => {
        const win = doc.defaultView;
        win.requestAnimationFrame((first) => {
          win.requestAnimationFrame((second) => resolve([first, second]));
        });
      });
      const observedTimeMs = api.getTime();
      const activeChapter = [...(api.getState()?.story?.chapters
        ?? window.__SK_STORY_DEF__?.chapters ?? [])]
        .filter((chapter) => (chapter.at || 0) <= observedTimeMs)
        .sort((left, right) => (left.at || 0) - (right.at || 0))
        .at(-1) ?? null;
      const rects = {};
      const missingSelectors = [];
      for (const selector of selectors) {
        const element = doc.querySelector(selector);
        if (!element) {
          missingSelectors.push(selector);
          continue;
        }
        const rect = element.getBoundingClientRect();
        rects[selector] = Object.fromEntries(
          ["x", "y", "width", "height", "top", "right", "bottom", "left"].map(
            (field) => [field, rect[field]],
          ),
        );
      }
      return {
        requestedChapterId: chapterId,
        activeChapterId: activeChapter?.id ?? null,
        requestedSeekMs: seekMs,
        observedTimeMs,
        topFrameTimestamps: topFrames,
        surfaceFrameTimestamps: surfaceFrames,
        rects,
        missingSelectors,
      };
    },
    { surfaceId, chapterId, seekMs, selectors },
  );
}

export function evaluateRectEquals(expected, actual, toleranceCssPx) {
  const deltas = {};
  const failures = [];
  const expectedRects = expected && typeof expected === "object" && !Array.isArray(expected)
    ? expected
    : null;
  const actualRects = actual && typeof actual === "object" && !Array.isArray(actual)
    ? actual
    : null;
  if (!Number.isFinite(toleranceCssPx) || toleranceCssPx < 0) {
    failures.push({ selector: null, field: "tolerance", reason: "invalid-tolerance" });
  }
  if (!expectedRects || !actualRects) {
    failures.push({ selector: null, field: "rects", reason: "invalid-rectangle-map" });
    return { toleranceCssPx, deltas, failures, pass: false };
  }
  const expectedSelectors = Object.keys(expectedRects).sort();
  const actualSelectors = Object.keys(actualRects).sort();
  if (expectedSelectors.length === 0) {
    failures.push({ selector: null, field: "selectors", reason: "empty-selector-set" });
  }
  for (const selector of expectedSelectors.filter((entry) => !actualSelectors.includes(entry))) {
    failures.push({ selector, field: "selectors", reason: "missing-selector" });
  }
  for (const selector of actualSelectors.filter((entry) => !expectedSelectors.includes(entry))) {
    failures.push({ selector, field: "selectors", reason: "unexpected-selector" });
  }
  for (const selector of expectedSelectors) {
    deltas[selector] = {};
    for (const field of RECT_FIELDS) {
      const expectedValue = expectedRects?.[selector]?.[field];
      const actualValue = actualRects?.[selector]?.[field];
      if (
        typeof expectedValue !== "number" ||
        typeof actualValue !== "number" ||
        !Number.isFinite(expectedValue) ||
        !Number.isFinite(actualValue)
      ) {
        deltas[selector][field] = null;
        failures.push({ selector, field, reason: "invalid-rectangle-coordinate" });
        continue;
      }
      const delta = actualValue - expectedValue;
      deltas[selector][field] = delta;
      if (!Number.isFinite(delta) || Math.abs(delta) > toleranceCssPx) {
        failures.push({ selector, field, delta, toleranceCssPx });
      }
    }
    for (const [kind, rect] of [["baseline", expectedRects[selector]], ["observed", actualRects[selector]]]) {
      if (!rect || typeof rect !== "object") continue;
      if (
        rect.width < 0 ||
        rect.height < 0 ||
        rect.x !== rect.left ||
        rect.y !== rect.top ||
        rect.right !== rect.left + rect.width ||
        rect.bottom !== rect.top + rect.height
      ) {
        failures.push({ selector, field: "geometry", reason: `${kind}-rectangle-inconsistent` });
      }
    }
  }
  return { toleranceCssPx, deltas, failures, pass: failures.length === 0 };
}

// ---------------------------------------------------------------------------
// Receipts
// ---------------------------------------------------------------------------

export function atomicWriteJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  const staging = `${path}.tmp-${process.pid}`;
  writeFileSync(staging, `${JSON.stringify(value, null, 2)}\n`);
  renameSync(staging, path);
}

export function baseReceipt(claim, runId) {
  return {
    schemaVersion: 2,
    primitiveId: "mockups.story.browserGeometry",
    tool: "script-kit-mockups.story-browser-geometry",
    command: "story.browser-geometry",
    receiptId: `pf012-${claim.storyDirName}-${runId}`,
    runId,
    taskIds: ["PF-012"],
    startedAt: new Date().toISOString(),
    story: {
      id: claim.storyId,
      entry: claim.entry,
      assertionId: claim.assertionId,
      assertionKind: claim.assertionKind,
      surfaceId: claim.surfaceId,
    },
    evidenceBoundary: "HTML_BROWSER_ONLY",
  };
}

export function blockedReceipt(claim, runId, resolution) {
  return {
    ...baseReceipt(claim, runId),
    browser: {
      dependency: null,
      version: null,
      headed: null,
      browserPid: null,
      lookups: resolution.lookups,
    },
    viewport: null,
    assets: null,
    autoplay: null,
    fonts: null,
    assertion: null,
    negativeControls: [],
    cleanup: {
      serverClosed: true,
      browserClosed: true,
      ownedBrowserPids: [],
      survivors: [],
      closed: true,
    },
    endedAt: new Date().toISOString(),
    blocker: resolution.blocker,
    disposition: "BLOCKED_MISSING_PRIMITIVE",
    pass: false,
  };
}
