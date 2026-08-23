import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  evaluateRectEquals,
  loadRectClaim,
  resolveLoopbackAssetPath,
} from "../../../design/mockups/tests/story-browser-geometry-harness.mjs";
import { currentIdentity } from "../../devtools/consistency.ts";
import {
  prepareValidatedReceipt,
  producerIdentityForTool,
  receiptSchema,
} from "../../devtools/lib/receipt-schema.ts";
import {
  buildStoryGeometryCandidate,
  evaluateStoryObservation,
  inspectStoryGeometryFixtures,
  isLoopbackStoryRequest,
  parseStoryGeometryArgs,
  STORY_GEOMETRY_IDS,
  STORY_GEOMETRY_PRIMITIVE,
  storyGeometryNegativeControls,
} from "./story-geometry-proof.mjs";

const SOURCE = currentIdentity().headCommit!;
const temporaryDirectories: string[] = [];

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

function rect(offset = 0) {
  return {
    x: 20 + offset,
    y: 30,
    width: 180,
    height: 42,
    top: 30,
    right: 200 + offset,
    bottom: 72,
    left: 20 + offset,
  };
}

function observation(storyId: string) {
  const claim = loadRectClaim(storyId);
  return {
    sourceCommit: SOURCE,
    story: { id: claim.storyId, assertionId: claim.assertionId, surfaceId: claim.surfaceId },
    browser: {
      dependency: "playwright",
      headed: true,
      observedVisible: true,
      sessionId: "synthetic-browser-session-1",
    },
    viewport: { width: 1280, height: 720, devicePixelRatio: 1 },
    fonts: {
      awaited: true,
      topLevelStatus: "loaded",
      surfaceStatus: "loaded",
      pass: true,
    },
    autoplay: {
      overrideInstalled: true,
      remainedStopped: true,
      clockBeforeFramesMs: 0,
      clockAfterFramesMs: 0,
    },
    chapters: Object.fromEntries(
      [claim.baselineChapter, ...claim.comparisonChapters].map((chapter) => [
        chapter.id,
        {
          requestedChapterId: chapter.id,
          activeChapterId: chapter.id,
          requestedSeekMs: chapter.at,
          observedTimeMs: chapter.at,
          topFrameTimestamps: [10, 20],
          surfaceFrameTimestamps: [11, 21],
          rects: Object.fromEntries(claim.selectors.map((selector) => [selector, rect()])),
          missingSelectors: [],
        },
      ]),
    ),
  };
}

function assets() {
  const bytes = readFileSync("design/mockups/stories/stories.json");
  return [{
    path: "mockups/stories/stories.json",
    sha256: createHash("sha256").update(bytes).digest("hex"),
    sizeBytes: bytes.length,
  }];
}

function candidate() {
  return buildStoryGeometryCandidate(
    STORY_GEOMETRY_IDS.map((storyId) => observation(storyId)),
    SOURCE,
    { assets: assets() },
  );
}

function validateSyntheticInteractive(receipt: Record<string, unknown>) {
  const original = process.env.SCRIPT_KIT_NONINTERACTIVE;
  process.env.SCRIPT_KIT_NONINTERACTIVE = "0";
  try {
    return prepareValidatedReceipt(STORY_GEOMETRY_PRIMITIVE, receipt);
  } finally {
    if (original === undefined) delete process.env.SCRIPT_KIT_NONINTERACTIVE;
    else process.env.SCRIPT_KIT_NONINTERACTIVE = original;
  }
}

describe("fail-closed browser story geometry", () => {
  test("fixture inspection is static, honest, and never claims browser runtime", () => {
    const inspected = inspectStoryGeometryFixtures({
      available: false,
      name: null,
      lookups: [{ name: "playwright", resolved: false }],
    });

    expect(inspected.evidenceClass).toBe("STATIC_INVENTORY");
    expect(inspected.provesRuntimeBehavior).toBe(false);
    expect(inspected.startsBrowser).toBe(false);
    expect(inspected.writesReceipts).toBe(false);
    expect(inspected.browserAvailable).toBe(false);
    expect(inspected.stories.map((story: { id: string }) => story.id)).toEqual(
      [...STORY_GEOMETRY_IDS],
    );
  });

  test("default CLI mode remains passive and browser execution requires an explicit request", () => {
    expect(parseStoryGeometryArgs([]).mode).toBe("inspect");
    expect(parseStoryGeometryArgs(["--run"]).mode).toBe("run");
    expect(parseStoryGeometryArgs(["--inspect-fixtures", "--out", "/tmp/synthetic"]).outputPath)
      .toBe("/tmp/synthetic");
    expect(() => parseStoryGeometryArgs(["--out"])).toThrow("requires an exact output path");
    expect(() => parseStoryGeometryArgs(["--from-tmp"])).toThrow("unknown story-geometry");
  });

  test("browser requests remain bound to their exact reviewed loopback origin", () => {
    const origin = "http://127.0.0.1:41523";
    expect(isLoopbackStoryRequest(`${origin}/stories/story.js`, origin)).toBe(true);
    for (const rejected of [
      "https://127.0.0.1:41523/stories/story.js",
      "http://127.0.0.1:41524/stories/story.js",
      "http://localhost:41523/stories/story.js",
      "https://example.com/stories/story.js",
      "http://127.0.0.1.example.com:41523/stories/story.js",
      "http://user:secret@127.0.0.1:41523/stories/story.js",
      "file:///etc/passwd",
    ]) {
      expect(isLoopbackStoryRequest(rejected, origin)).toBe(false);
    }
  });

  test("loopback asset resolution refuses encoded traversal and symlinked external files", () => {
    const directory = mkdtempSync(join(tmpdir(), "story-loopback-root-"));
    temporaryDirectories.push(directory);
    const hosted = join(directory, "hosted");
    const outside = join(directory, "outside.txt");
    mkdirSync(hosted);
    writeFileSync(outside, "synthetic private bytes");
    writeFileSync(join(hosted, "safe.txt"), "synthetic public bytes");
    symlinkSync(outside, join(hosted, "escape.txt"));

    expect(resolveLoopbackAssetPath(hosted, "/safe.txt")).toBe(join(hosted, "safe.txt"));
    expect(resolveLoopbackAssetPath(hosted, "/%2e%2e/outside.txt")).toBeNull();
    expect(resolveLoopbackAssetPath(hosted, "/escape.txt")).toBeNull();
    expect(resolveLoopbackAssetPath(hosted, "/%zz")).toBeNull();
  });

  test("zero-tolerance comparison accepts identical complete rectangle maps", () => {
    const result = evaluateRectEquals({ ".row": rect() }, { ".row": rect() }, 0);
    expect(result.pass).toBe(true);
    expect(result.failures).toEqual([]);
    expect(result.deltas[".row"].right).toBe(0);
  });

  test("empty, missing, additional, and non-object selectors cannot pass geometry", () => {
    expect(evaluateRectEquals({}, {}, 0).pass).toBe(false);
    expect(evaluateRectEquals({ ".row": rect() }, {}, 0).pass).toBe(false);
    expect(evaluateRectEquals({ ".row": rect() }, {
      ".row": rect(),
      ".forged": rect(),
    }, 0).pass).toBe(false);
    expect(evaluateRectEquals(null, { ".row": rect() }, 0).pass).toBe(false);
    expect(evaluateRectEquals({ ".row": rect() }, null, 0).pass).toBe(false);
  });

  test("NaN, infinity, missing, negative, and coerced tolerance never hide pixel drift", () => {
    for (const tolerance of [NaN, Infinity, -1, undefined, "1"]) {
      expect(evaluateRectEquals({ ".row": rect() }, {
        ".row": rect(1),
      }, tolerance).pass).toBe(false);
    }
  });

  test("missing, string-coerced, infinite, and impossible rectangle fields fail", () => {
    for (const mutated of [
      { ...rect(), x: undefined },
      { ...rect(), x: "20" },
      { ...rect(), x: Infinity },
      { ...rect(), width: -180 },
      { ...rect(), right: 201 },
      { ...rect(), bottom: 71 },
    ]) {
      expect(evaluateRectEquals({ ".row": rect() }, { ".row": mutated }, 0).pass)
        .toBe(false);
    }
  });

  test("both real story claims accept complete settled synthetic observations as unit behavior only", () => {
    for (const storyId of STORY_GEOMETRY_IDS) {
      const result = evaluateStoryObservation(loadRectClaim(storyId), observation(storyId), SOURCE);
      expect(result.pass).toBe(true);
      expect(result.comparisons).toHaveLength(2);
      expect(result.errors).toEqual([]);
    }
  });

  test("one-pixel drift and omitted selectors are measured, not inferred from fixture text", () => {
    const claim = loadRectClaim(STORY_GEOMETRY_IDS[0]);
    const pixel = observation(claim.storyId);
    const selector = claim.selectors[0];
    const chapter = claim.comparisonChapters[0].id;
    pixel.chapters[chapter].rects[selector] = rect(1);
    expect(evaluateStoryObservation(claim, pixel, SOURCE).errors)
      .toContain(`rectangle-mismatch:${chapter}`);

    const missing = observation(claim.storyId);
    delete missing.chapters[chapter].rects[selector];
    expect(evaluateStoryObservation(claim, missing, SOURCE).errors)
      .toContain(`missing-or-unexpected-selector:${chapter}`);
  });

  test("wrong active chapter, unsettled frames, and wrong viewport never count as proof", () => {
    const claim = loadRectClaim(STORY_GEOMETRY_IDS[0]);
    const chapter = claim.comparisonChapters[0].id;
    const wrongChapter = observation(claim.storyId);
    wrongChapter.chapters[chapter].activeChapterId = claim.baselineChapter.id;
    expect(evaluateStoryObservation(claim, wrongChapter, SOURCE).errors)
      .toContain(`wrong-active-chapter:${chapter}`);

    const unsettled = observation(claim.storyId);
    unsettled.chapters[chapter].topFrameTimestamps = [12, 12];
    expect(evaluateStoryObservation(claim, unsettled, SOURCE).errors)
      .toContain(`unsettled-animation-frames:${chapter}`);

    const viewport = observation(claim.storyId);
    viewport.viewport.devicePixelRatio = 2;
    expect(evaluateStoryObservation(claim, viewport, SOURCE).errors)
      .toContain("wrong-browser-viewport-or-dpr");
  });

  test("fonts, autoplay, browser visibility, fixture identity, and source freshness are mandatory", () => {
    const claim = loadRectClaim(STORY_GEOMETRY_IDS[0]);
    const mutations = [
      [(value: any) => { value.fonts.surfaceStatus = "loading"; }, "unresolved-fonts"],
      [(value: any) => { value.autoplay.remainedStopped = false; }, "autoplay-was-not-stopped-before-mount"],
      [(value: any) => { value.browser.observedVisible = false; }, "unobserved-headed-browser"],
      [(value: any) => { value.story.assertionId = "invented"; }, "story-assertion-identity-mismatch"],
      [(value: any) => { value.sourceCommit = "b".repeat(40); }, "stale-source-commit"],
    ] as const;
    for (const [mutate, expected] of mutations) {
      const modified = observation(claim.storyId);
      mutate(modified);
      expect(evaluateStoryObservation(claim, modified, SOURCE).errors).toContain(expected);
    }
  });

  test("every adversarial control executes against both actual fixture claims", () => {
    for (const storyId of STORY_GEOMETRY_IDS) {
      const claim = loadRectClaim(storyId);
      const controls = storyGeometryNegativeControls(claim, observation(storyId), SOURCE);
      expect(controls).toHaveLength(6);
      expect(controls.every((control: { pass: boolean }) => control.pass)).toBe(true);
    }
  });

  test("a partial or duplicated story cannot satisfy the two-story obligation", () => {
    const first = observation(STORY_GEOMETRY_IDS[0]);
    expect(() => buildStoryGeometryCandidate([first], SOURCE)).toThrow("both exact story identities");
    expect(() => buildStoryGeometryCandidate([first, first], SOURCE))
      .toThrow("both exact story identities");
  });

  test("the browser producer has a canonical registered owner and reviewed schema", () => {
    expect(receiptSchema(STORY_GEOMETRY_PRIMITIVE)?.tool)
      .toBe("script-kit-mockups.story-browser-geometry");
    expect(producerIdentityForTool("script-kit-mockups.story-browser-geometry").producerPath)
      .toEndWith("scripts/agentic/cons-proof-gov/story-geometry-proof.mjs");
  });

  test("two complete observed stories validate only as explicit visible runtime evidence", () => {
    const prepared = validateSyntheticInteractive(candidate());
    expect(prepared.exitCode).toBe(0);
    expect(prepared.receipt.evidenceClass).toBe("RUNTIME_VISIBLE");
    expect(prepared.receipt.taskIds).toEqual(["PF-012"]);
    expect((prepared.receipt.stories as unknown[])).toHaveLength(2);
    expect((prepared.receipt.negativeControls as unknown[])).toHaveLength(12);
    expect(prepared.receipt.producerValidation).toMatchObject({ valid: true });
  });

  test("the canonical registry rejects fake backend, hidden browser, stale fixture, and missing assets", () => {
    const base = candidate();
    for (const forged of [
      { ...base, browser: { ...base.browser, dependency: "agent-browser" } },
      { ...base, browser: { ...base.browser, observedVisible: false } },
      { ...base, target: { ...base.target, visible: false } },
      { ...base, fixture: { ...base.fixture, sha256: "a".repeat(64) } },
      { ...base, fixtures: base.fixtures.slice(0, 1) },
      { ...base, assets: [] },
      { ...base, assetFingerprint: "b".repeat(64) },
      { ...base, stories: base.stories.slice(0, 1) },
      { ...base, negativeControls: base.negativeControls.slice(0, -1) },
      { ...base, cleanup: { ...base.cleanup, browserClosed: false } },
    ]) {
      expect(validateSyntheticInteractive(forged).exitCode).not.toBe(0);
    }
  });

  test("green-looking browser runtime proof remains invalid in noninteractive mode", () => {
    const original = process.env.SCRIPT_KIT_NONINTERACTIVE;
    process.env.SCRIPT_KIT_NONINTERACTIVE = "1";
    try {
      const prepared = prepareValidatedReceipt(STORY_GEOMETRY_PRIMITIVE, candidate());
      expect(prepared.exitCode).not.toBe(0);
      expect(prepared.validation.errors.join(" ")).toContain("noninteractive runtime evidence");
    } finally {
      if (original === undefined) delete process.env.SCRIPT_KIT_NONINTERACTIVE;
      else process.env.SCRIPT_KIT_NONINTERACTIVE = original;
    }
  });

  test("strict browser invocation refuses before dependency discovery, output mutation, or launch", () => {
    const directory = mkdtempSync(join(tmpdir(), "story-geometry-safe-"));
    temporaryDirectories.push(directory);
    const output = join(directory, "never-created.json");
    const result = Bun.spawnSync([
      process.execPath,
      "scripts/agentic/cons-proof-gov/story-geometry-proof.mjs",
      "--run",
      "--out",
      output,
    ], {
      cwd: process.cwd(),
      env: {
        ...process.env,
        SCRIPT_KIT_NONINTERACTIVE: "1",
        SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
        SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
        SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
        SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
        SCRIPT_KIT_ALLOW_LIVE_AI: "0",
        SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
      },
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode).toBe(4);
    expect(new TextDecoder().decode(result.stderr)).toContain("NONINTERACTIVE=1 refused");
    expect(existsSync(output)).toBe(false);
  });
});
