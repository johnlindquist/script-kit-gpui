#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { relative, resolve } from "node:path";
import { Driver } from "../../devtools/driver.ts";
import { analyzeLayout, buildMeasurementJoins } from "../../devtools/layout.ts";
import { textFitMeasurements } from "../../devtools/text.ts";
import { openDayPage } from "../day-page-open-helper.ts";

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY
    ?? "target-agent/artifacts/cons-proof-c05/script-kit-gpui",
);
const layoutPath = resolve(
  process.env.CONSISTENCY_LAYOUT_RECEIPT_PATH
    ?? ".artifacts/consistency/PF-005/layout-join.json",
);
const textPath = resolve(
  process.env.CONSISTENCY_TEXT_RECEIPT_PATH
    ?? ".artifacts/consistency/PF-006/text-fit.json",
);

type Obj = Record<string, unknown>;
type TextFit = ReturnType<typeof textFitMeasurements>[number];

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Obj
    : {};
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function sha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const normalized = resolve(executable);
  return new TextDecoder().decode(result.stdout)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const executablePath = match[2].trim().split(/\s+/, 1)[0];
      return resolve(executablePath) === normalized ? [Number(match[1])] : [];
    });
}

async function poll<T>(
  label: string,
  probe: () => Promise<T>,
  predicate: (value: T) => boolean,
  timeoutMs = 10_000,
): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  let last = await probe();
  while (performance.now() < deadline) {
    if (predicate(last)) return last;
    await Bun.sleep(35);
    last = await probe();
  }
  throw new Error(`${label} did not become observable`);
}

function infoOf(layout: unknown): Obj {
  const response = asObj(layout);
  return asObj(response.info ?? response);
}

function targetReceiptFor(layout: unknown): Obj {
  const info = infoOf(layout);
  return {
    resolvedTarget: {
      bounds: {
        x: 0,
        y: 0,
        width: Number(info.windowWidth ?? 0),
        height: Number(info.windowHeight ?? 0),
      },
    },
  };
}

function fitSummary(fits: TextFit[]) {
  return {
    count: fits.length,
    measurementIds: fits.map((fit) => fit.measurementId),
    semanticIds: [...new Set(fits.map((fit) => fit.semanticId))],
    frameGenerations: [...new Set(fits.map((fit) => fit.frameGeneration))],
    captureFrameGenerations: [...new Set(fits.map((fit) => fit.captureFrameGeneration))],
    backingScaleFactors: [...new Set(fits.map((fit) => fit.backingScaleFactor))],
    minimumVisibleRatio: fits.reduce((minimum, fit) => Math.min(minimum, fit.visibleRatio), 1),
    fontsReady: fits.every((fit) => fit.fontsReady),
    frameMatches: fits.every((fit) => fit.frameMatches),
    backingScalePresent: fits.every((fit) => typeof fit.backingScaleFactor === "number" && fit.backingScaleFactor > 0),
    occluderCount: fits.reduce((count, fit) => count + fit.occluderMeasurementIds.length, 0),
    rawContentReturned: fits.some((fit) => fit.rawContentReturned),
    fullDisplayPass: fits.every((fit) => fit.fullDisplayPass),
  };
}

function syntheticLine(overrides: Obj = {}): Obj {
  return {
    id: "fixture.line.0.0",
    kind: "textLine",
    bounds: { x: 10, y: 10, width: 180, height: 20 },
    visibleBounds: { x: 10, y: 10, width: 180, height: 20 },
    clipBounds: { x: 10, y: 10, width: 180, height: 20 },
    unionPaintBounds: { x: 10, y: 12, width: 80, height: 16 },
    paintOrder: 0,
    measurementFrameGeneration: 12,
    textHash: "fixture-content-fingerprint",
    metadata: {
      measurementId: "text:fixture:line:0:0",
      semanticId: "input:fixture",
      role: "textLineBox",
      fontFamilyFingerprint: "fixture-font-fingerprint",
      fontSize: 14,
      fontWeight: "mixedOrRendererOwned",
      lineHeight: 20,
      backingScaleFactor: 2,
      fontsReady: true,
      wrappingPolicy: "none",
      truncationPolicy: "fullDisplay",
      contentKind: "userContent",
      graphemeCount: 7,
      lineCount: 1,
      rawContentReturned: false,
    },
    ...overrides,
  };
}

function syntheticLayout(nodes: Obj[], frameGeneration = 12): Obj {
  return { fidelity: { frameGeneration, nodes } };
}

const modelBounds = { x: 0, y: 0, width: 100, height: 20 };
const modelNode = {
  measurementId: "layout:fixture-row",
  semanticId: "row:fixture",
  role: "rowSlot" as const,
  bounds: modelBounds,
  visibleBounds: modelBounds,
  clipBounds: modelBounds,
  measurementProvenance: "model",
  coordinateSpace: "window",
  measurementFrameGeneration: 7,
};
const layoutNegatives = {
  renderedOnePointClip: buildMeasurementJoins([
    modelNode,
    {
      ...modelNode,
      measurementProvenance: "paint-time",
      visibleBounds: { x: 0, y: 0, width: 99, height: 20 },
    },
  ])[0],
  roleMismatch: buildMeasurementJoins([
    modelNode,
    { ...modelNode, measurementProvenance: "paint-time", role: "footerNativeHost" },
  ])[0],
  staleFrame: buildMeasurementJoins([
    modelNode,
    { ...modelNode, measurementProvenance: "paint-time", measurementFrameGeneration: 8 },
  ])[0],
  renderedOnly: buildMeasurementJoins([
    { ...modelNode, measurementProvenance: "paint-time" },
  ])[0],
};
const modelOverlapWithCleanPaint = analyzeLayout({
  windowWidth: 200,
  windowHeight: 100,
  components: [
    { name: "ModelA", type: "row", measurementId: "layout:a", geometryRole: "rowSlot", bounds: { x: 0, y: 0, width: 120, height: 20 }, depth: 1, parent: "root", measurementProvenance: "model", coordinateSpace: "window", measurementFrameGeneration: 4 },
    { name: "ModelB", type: "row", measurementId: "layout:b", geometryRole: "rowSlot", bounds: { x: 100, y: 0, width: 80, height: 20 }, depth: 1, parent: "root", measurementProvenance: "model", coordinateSpace: "window", measurementFrameGeneration: 4 },
    { name: "PaintA", type: "row", measurementId: "layout:a", geometryRole: "rowSlot", bounds: { x: 0, y: 0, width: 80, height: 20 }, visibleBounds: { x: 0, y: 0, width: 80, height: 20 }, clipBounds: { x: 0, y: 0, width: 200, height: 100 }, depth: 1, parent: "root", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 4 },
    { name: "PaintB", type: "row", measurementId: "layout:b", geometryRole: "rowSlot", bounds: { x: 100, y: 0, width: 80, height: 20 }, visibleBounds: { x: 100, y: 0, width: 80, height: 20 }, clipBounds: { x: 0, y: 0, width: 200, height: 100 }, depth: 1, parent: "root", measurementProvenance: "paint-time", coordinateSpace: "window", measurementFrameGeneration: 4 },
  ],
}, { resolvedTarget: { bounds: { x: 0, y: 0, width: 200, height: 100 } } });
const clippedLine = textFitMeasurements(syntheticLayout([
  syntheticLine({ clipBounds: { x: 10, y: 12, width: 79, height: 16 } }),
]))[0];
const fontsPendingLine = textFitMeasurements(syntheticLayout([
  syntheticLine({ metadata: { ...asObj(syntheticLine().metadata), fontsReady: false } }),
]))[0];
const wrongScaleLine = textFitMeasurements(syntheticLayout([syntheticLine()]), 1)[0];
const occludedLine = textFitMeasurements(syntheticLayout([
  syntheticLine(),
  {
    id: "fixture.footer",
    kind: "element",
    unionPaintBounds: { x: 0, y: 20, width: 200, height: 20 },
    paintOrder: 1,
    metadata: { measurementId: "layout:fixture-footer" },
  },
]))[0];

let driver: Driver | null = null;
let closeError: string | null = null;
let runtimeError: string | null = null;
let runtimeStage = "launch";
let settingsProof: Obj = {};
let notesProof: Obj = {};
let dayPageProof: Obj = {};

try {
  driver = await Driver.launch({
    binary,
    sessionName: `cons-proof-c05-${process.pid}`,
    sandboxHome: true,
    sharedModels: false,
    defaultTimeoutMs: 12_000,
    env: {
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_FIDELITY_CAPTURE: "agent-chat",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
    },
  });

  runtimeStage = "settings-layout-join";
  driver.send({ type: "triggerBuiltin", name: "settings" });
  await poll(
    "Settings",
    async () => asObj(await driver!.getState({ timeoutMs: 4_000 })),
    (state) => String(state.promptType ?? state.currentView ?? "").toLowerCase().includes("settings"),
  );
  await driver.waitForSettle({ timeoutMs: 8_000 });
  const settingsLayout = await poll(
    "Settings comparable geometry",
    () => driver!.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 5_000 }),
    (layout) => {
      const analysis = analyzeLayout(asObj(layout), targetReceiptFor(layout));
      return analysis.truthLayers.comparableJoinCount > 0;
    },
  );
  const settingsAnalysis = analyzeLayout(asObj(settingsLayout), targetReceiptFor(settingsLayout));
  const mainHeaderJoin = settingsAnalysis.truthLayers.joins.find(
    (join) => join.measurementId === "layout:main-view-header",
  );
  settingsProof = {
    promptType: settingsAnalysis.promptType,
    modelNodeCount: settingsAnalysis.truthLayers.model.nodeCount,
    renderedNodeCount: settingsAnalysis.truthLayers.rendered.nodeCount,
    comparableJoinCount: settingsAnalysis.truthLayers.comparableJoinCount,
    renderedClippedNodeCount: settingsAnalysis.truthLayers.rendered.clippedNodeCount,
    outOfToleranceJoinCount: settingsAnalysis.truthLayers.joins.filter(
      (join) => join.classification === "OutOfTolerance",
    ).length,
    modelPaintAgreement: settingsAnalysis.truthLayers.joins
      .filter((join) => join.comparability === "Comparable")
      .every((join) => join.classification === "Match"),
    comparableJoins: settingsAnalysis.truthLayers.joins.filter(
      (join) => join.comparability === "Comparable",
    ),
    mainHeaderJoin: mainHeaderJoin ?? null,
    unjoinedMeasurementIds: settingsAnalysis.truthLayers.unjoinedMeasurementIds,
  };
  assert(mainHeaderJoin != null, "Settings lacks the canonical MainViewHeader join");
  assert(mainHeaderJoin.comparability === "Comparable", "Settings header model/paint join is not comparable");
  assert(settingsAnalysis.truthLayers.rendered.clippedNodeCount === 0, "Settings rendered geometry is clipped");
  assert(settingsAnalysis.truthLayers.rendered.overlapCount === 0, "Settings rendered geometry overlaps");

  runtimeStage = "notes-text-fit";
  driver.send({ type: "openNotes", requestId: "pf006-open-notes" });
  await poll(
    "Notes target",
    () => driver!.listAutomationWindows(),
    (response) => asArray(asObj(response).windows).some((entry) => String(asObj(entry).kind ?? "").toLowerCase() === "notes"),
  );
  await driver.request({
    type: "batch",
    target: { type: "kind", kind: "notes", index: 0 },
    commands: [{ type: "setInput", text: "# Capture heading\nBody line for shaped proof" }],
    options: { stopOnError: true, rollbackOnError: false, timeout: 8_000 },
  }, { expect: "batchResult", timeoutMs: 10_000 });
  const notesLayout = await poll(
    "Notes shaped text",
    () => driver!.getLayoutInfo({ target: { type: "kind", kind: "notes", index: 0 } }, { timeoutMs: 5_000 }),
    (layout) => textFitMeasurements(asObj(layout)).some((fit) => fit.semanticId === "input:notes-editor"),
  );
  const notesFits = textFitMeasurements(asObj(notesLayout)).filter(
    (fit) => fit.semanticId === "input:notes-editor",
  );
  const notesSummary = fitSummary(notesFits);
  assert(notesSummary.count >= 2, "Notes did not expose heading/body shaped lines");
  assert(notesSummary.fullDisplayPass, "Notes shaped glyphs are clipped, occluded, stale, or font-pending");
  assert(notesSummary.rawContentReturned === false, "Notes text proof returned raw content");
  notesProof = notesSummary;

  // Toggle the same owned Notes window closed before moving the main window to Today.
  driver.send({ type: "openNotes", requestId: "pf006-close-notes" });
  await poll(
    "Notes cleanup before Today",
    () => driver!.listAutomationWindows(),
    (response) => !asArray(asObj(response).windows).some((entry) => String(asObj(entry).kind ?? "").toLowerCase() === "notes"),
  );

  runtimeStage = "day-page-text-fit";
  await openDayPage(driver, `pf006-${process.pid}`);
  await driver.batch([
    { type: "setInput", text: "# Today capture\nBody line for shaped proof" },
  ], { timeoutMs: 8_000 });
  const dayLayout = await poll(
    "Day Page shaped text",
    () => driver!.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 5_000 }),
    (layout) => textFitMeasurements(asObj(layout)).some((fit) => fit.semanticId === "input:day-page-editor"),
  );
  const dayFits = textFitMeasurements(asObj(dayLayout)).filter(
    (fit) => fit.semanticId === "input:day-page-editor",
  );
  const daySummary = fitSummary(dayFits);
  assert(daySummary.count >= 2, "Day Page did not expose heading/body shaped lines");
  assert(daySummary.fullDisplayPass, "Day Page shaped glyphs are clipped, occluded, stale, or font-pending");
  assert(daySummary.rawContentReturned === false, "Day Page text proof returned raw content");
  dayPageProof = daySummary;
  runtimeStage = "complete";
} catch (error) {
  runtimeError = error instanceof Error ? error.message : "UnknownError";
} finally {
  if (driver) {
    try {
      await driver.close();
    } catch (error) {
      closeError = error instanceof Error ? error.name : "UnknownCloseError";
    }
  }
}

const cleanup = driver
  ? {
      processExited: driver.finalization.processExited,
      streamsDrained: driver.finalization.streamsDrained,
      logWriterClosed: driver.finalization.logWriterClosed,
      ownedProcessCount: exactExecutablePids(binary).length,
      closeError,
      clipboardTouched: false,
    }
  : {
      processExited: false,
      streamsDrained: false,
      logWriterClosed: false,
      ownedProcessCount: exactExecutablePids(binary).length,
      closeError,
      clipboardTouched: false,
    };
const cleanupPassed = cleanup.processExited && cleanup.streamsDrained &&
  cleanup.logWriterClosed && cleanup.ownedProcessCount === 0 && cleanup.closeError == null;
const runtimePassed = runtimeError == null && runtimeStage === "complete";
const artifact = {
  executable: relative(process.cwd(), binary),
  sha256: sha256(binary),
};
const layoutNegativeControls = {
  renderedOnePointClipDetected: layoutNegatives.renderedOnePointClip.classification === "Clipped",
  roleMismatchNotComparable: layoutNegatives.roleMismatch.comparability === "RoleMismatch" &&
    layoutNegatives.roleMismatch.classification === "NotComparable",
  staleFrameNotComparable: layoutNegatives.staleFrame.comparability === "StaleGeneration" &&
    layoutNegatives.staleFrame.classification === "NotComparable",
  renderedOnlyNotComparable: layoutNegatives.renderedOnly.comparability === "RenderedOnly" &&
    layoutNegatives.renderedOnly.classification === "NotComparable",
  modelSiblingOverlapNotHiddenByCleanPaint:
    modelOverlapWithCleanPaint.truthLayers.model.overlapCount === 1 &&
    modelOverlapWithCleanPaint.truthLayers.rendered.overlapCount === 0,
};
const textNegativeControls = {
  onePointGlyphClipFails: clippedLine.fullDisplayPass === false && clippedLine.visibleRatio < 1,
  footerOcclusionFails: occludedLine.fullDisplayPass === false && occludedLine.occluderMeasurementIds.length === 1,
  fontsNotReadyFails: fontsPendingLine.fullDisplayPass === false && fontsPendingLine.fontsReady === false,
  backingScaleMismatchFails: wrongScaleLine.fullDisplayPass === false && wrongScaleLine.backingScaleMatches === false,
};
const layoutReceipt = {
  schemaVersion: 2,
  taskId: "PF-005",
  classification: runtimePassed && cleanupPassed && Object.values(layoutNegativeControls).every(Boolean)
    ? "RUNTIME-CONFIRMED"
    : "RUNTIME-FAILED",
  artifact,
  intendedContract: {
    source: "src/protocol/types/grid_layout.rs::GeometryRole",
    invariant: "Compare only equal roles in one coordinate space and capture generation; keep model and rendered layers independent.",
  },
  settings: settingsProof,
  negativeControls: layoutNegativeControls,
  cleanup,
  runtimeStage,
  runtimeError,
};
const textReceipt = {
  schemaVersion: 2,
  taskId: "PF-006",
  classification: runtimePassed && cleanupPassed && Object.values(textNegativeControls).every(Boolean)
    ? "RUNTIME-CONFIRMED"
    : "RUNTIME-FAILED",
  artifact,
  notes: notesProof,
  dayPage: dayPageProof,
  negativeControls: textNegativeControls,
  privacy: {
    rawContentReturned: false,
    fixtureCanaryMatches: JSON.stringify({ notesProof, dayPageProof }).includes("Capture heading") ? 1 : 0,
  },
  cleanup,
  runtimeStage,
  runtimeError,
};

await mkdir(resolve(layoutPath, ".."), { recursive: true });
await mkdir(resolve(textPath, ".."), { recursive: true });
await writeFile(layoutPath, `${JSON.stringify(layoutReceipt, null, 2)}\n`);
await writeFile(textPath, `${JSON.stringify(textReceipt, null, 2)}\n`);
console.log(JSON.stringify({ layout: layoutReceipt, text: textReceipt }, null, 2));

if (layoutReceipt.classification !== "RUNTIME-CONFIRMED" ||
  textReceipt.classification !== "RUNTIME-CONFIRMED" ||
  textReceipt.privacy.fixtureCanaryMatches !== 0) {
  process.exitCode = 1;
}
