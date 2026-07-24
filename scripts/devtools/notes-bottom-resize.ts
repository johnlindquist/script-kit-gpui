#!/usr/bin/env bun

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { Driver, type Json } from "./driver.ts";
import { validateNotesBottomResizeReceipt } from "./notes-bottom-resize-contract.ts";
import { announceTestStatus } from "./test-status.ts";

const arg = (name: string, fallback?: string) => {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
};
const binary = resolve(
  arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY ?? "")!,
);
const outDir = resolve(
  arg("--out", ".artifacts/notes-bottom-resize/final")!,
);
const themeFixture = arg("--theme-fixture");
if (!binary || !existsSync(binary)) throw new Error(`binary missing: ${binary}`);
mkdirSync(outDir, { recursive: true });

const helperSource = resolve(
  import.meta.dir,
  "../agentic/macos-native-pointer-drag.swift",
);
const helper = join(outDir, "macos-native-pointer-drag");
const compile = Bun.spawnSync([
  "xcrun",
  "swiftc",
  "-O",
  helperSource,
  "-o",
  helper,
]);
if (compile.exitCode !== 0) {
  throw new Error(`pointer helper compile failed: ${compile.stderr.toString()}`);
}

type Rect = { x: number; y: number; width: number; height: number };
type Region = {
  group: string;
  index: number;
  elementId: string;
  bounds: Rect;
};

const sha256 = (path: string) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
const target = { type: "id", id: "notes" };
const receipt: Json = {
  schemaVersion: 1,
  runId: `notes-bottom-resize-${Date.now()}-${process.pid}`,
  gitCommit: Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout
    .toString()
    .trim(),
  binary,
  binarySha256: sha256(binary),
  pointerHelperSha256: sha256(helper),
  themeFixture: themeFixture ? resolve(themeFixture) : null,
  themeFixtureSha256: themeFixture ? sha256(resolve(themeFixture)) : null,
  startedAt: new Date().toISOString(),
  statusAnnouncements: [],
  edgeTrial: null,
  shrinkTrial: null,
  buttonTrials: [],
  screenshots: [],
  pass: false,
};

function notesState(envelope: Json): Json {
  return envelope?.notes ?? envelope ?? {};
}

async function waitForNotesReady(driver: Driver, timeoutMs = 8_000) {
  const deadline = performance.now() + timeoutMs;
  let last: Json = null;
  while (performance.now() < deadline) {
    try {
      last = notesState(await driver.getTargetState(target, { timeoutMs: 2_000 }));
      const reveal = last?.entryReveal;
      const footer = last?.footerHitRegions;
      const configured = Number(reveal?.nativeConfiguredAtUnixMs);
      const settleMs = Number(reveal?.settleDurationMs);
      const elapsedEnough = Number.isFinite(configured)
        && Number.isFinite(settleMs)
        && Date.now() >= configured + settleMs;
      if (
        reveal?.bodyVisible === true
        && elapsedEnough
        && footer?.nativeWindowNumber > 0
        && Array.isArray(footer?.regions)
      ) {
        return last;
      }
    } catch {}
    await Bun.sleep(25);
  }
  throw new Error(`Notes did not expose settled resize state: ${JSON.stringify(last)}`);
}

async function ensureSelectedNote(driver: Driver) {
  let state = await waitForNotesReady(driver);
  if (state?.selectedNote?.id) return state;
  await driver.simulateGpuiKeyDown("n", {
    modifiers: ["cmd"],
    target,
    timeoutMs: 5_000,
  });
  await driver.request(
    {
      type: "batch",
      target,
      commands: [{ type: "setInput", text: "Bottom resize sandbox fixture" }],
      options: { stopOnError: true, rollbackOnError: false, timeout: 5_000 },
    },
    { expect: "batchResult", timeoutMs: 6_000 },
  );
  const deadline = performance.now() + 5_000;
  while (performance.now() < deadline) {
    state = await waitForNotesReady(driver);
    if (
      state?.selectedNote?.id
      && Array.isArray(state?.footerHitRegions?.regions)
      && state.footerHitRegions.regions.length > 0
    ) return state;
    await Bun.sleep(40);
  }
  throw new Error("sandbox note/footer buttons did not become available");
}

async function queryWindows(pid: number) {
  const process = Bun.spawn([
    "swift",
    resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
    "--pid",
    String(pid),
  ], { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([
    new Response(process.stdout).text(),
    new Response(process.stderr).text(),
    process.exited,
  ]);
  if (code !== 0) throw new Error(`window query failed: ${stderr}`);
  return JSON.parse(stdout);
}

async function exactWindow(pid: number, windowId: number): Promise<Json> {
  const query = await queryWindows(pid);
  const match = (query.windows ?? []).find(
    (window: Json) => Number(window.windowId) === windowId
      && Number(window.ownerPid) === pid,
  );
  if (!match) {
    throw new Error(`exact Notes window ${pid}/${windowId} not found`);
  }
  return match;
}

function widestSafeBottomSegment(width: number, regions: Region[]) {
  const intervals = regions
    .map((region) => [
      Math.max(6, region.bounds.x - 1),
      Math.min(width - 6, region.bounds.x + region.bounds.width + 1),
    ] as [number, number])
    .sort((a, b) => a[0] - b[0]);
  const merged: Array<[number, number]> = [];
  for (const interval of intervals) {
    const last = merged.at(-1);
    if (last && interval[0] <= last[1]) last[1] = Math.max(last[1], interval[1]);
    else merged.push([...interval]);
  }
  const gaps: Array<[number, number]> = [];
  let cursor = 6;
  for (const [start, end] of merged) {
    if (start > cursor) gaps.push([cursor, start]);
    cursor = Math.max(cursor, end);
  }
  if (cursor < width - 6) gaps.push([cursor, width - 6]);
  const widest = gaps.sort((a, b) => (b[1] - b[0]) - (a[1] - a[0]))[0];
  if (!widest || widest[1] - widest[0] < 12) {
    throw new Error("no safe bottom-edge segment outside floating buttons");
  }
  return { start: widest[0], end: widest[1], x: (widest[0] + widest[1]) / 2 };
}

async function runDrag(
  pid: number,
  windowId: number,
  start: { x: number; y: number },
  end: { x: number; y: number },
  label: string,
) {
  const path = join(outDir, `${label}.json`);
  const tag = Date.now() * 1_000 + Math.floor(Math.random() * 900);
  const process = Bun.spawn([
    helper,
    "--pid",
    String(pid),
    "--window-id",
    String(windowId),
    "--start",
    String(start.x),
    String(start.y),
    "--end",
    String(end.x),
    String(end.y),
    "--duration-ms",
    "600",
    "--steps",
    "48",
    "--event-user-data",
    String(tag),
    "--out",
    path,
  ], { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([
    new Response(process.stdout).text(),
    new Response(process.stderr).text(),
    process.exited,
  ]);
  const result = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null;
  if (code !== 0 || !result) {
    throw new Error(`native drag ${label} failed: ${stderr || stdout}`);
  }
  return { path, sha256: sha256(path), result };
}

function invariantFingerprint(state: Json) {
  return {
    selectedNoteId: state?.selectedNote?.id ?? null,
    contentFingerprint: state?.selectedNote?.contentFingerprint ?? null,
    surfaceMode: state?.view?.surfaceMode ?? null,
    preview: state?.view?.previewEnabled ?? null,
    focusMode: state?.view?.focusMode ?? null,
    currentFocusSurface:
      state?.shortcutRegistry?.currentFocusSurface ?? null,
  };
}

function boundsDistinct(samples: Json[]) {
  return new Set(
    samples
      .map((sample) => sample?.bounds?.height)
      .filter((value) => Number.isFinite(value))
      .map((value) => Number(value).toFixed(2)),
  ).size;
}

async function capture(driver: Driver, name: string) {
  const path = join(outDir, `${name}.png`);
  const result = await driver.captureScreenshot({
    hiDpi: true,
    target,
    savePath: path,
    timeoutMs: 10_000,
  });
  if (result.error || !existsSync(path)) {
    throw new Error(`screenshot ${name} failed: ${result.error ?? "missing file"}`);
  }
  const entry = { name, path, sha256: sha256(path) };
  receipt.screenshots.push(entry);
  return entry;
}

await announceTestStatus(
  "Notes resize setup",
  "Opening a sandbox Notes window and measuring its floating-button bounds",
);
receipt.statusAnnouncements.push("Notes resize setup");

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `notes-bottom-resize-${process.pid}`,
  themeFixturePath: themeFixture ? resolve(themeFixture) : undefined,
  defaultTimeoutMs: 8_000,
  env: {
    SCRIPT_KIT_TEST_STATUS: "1",
    SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
    SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
  },
});

try {
  receipt.pid = driver.pid;
  await driver.request({ type: "show" }, { timeoutMs: 5_000 }).catch(() => null);
  driver.send({ type: "openNotes", requestId: "notes-bottom-resize-open" });
  let state = await ensureSelectedNote(driver);
  const beforeFingerprint = invariantFingerprint(state);
  receipt.initialFingerprint = beforeFingerprint;
  const windowId = Number(state.entryReveal.nativeWindowNumber);
  const nativeBefore = await exactWindow(driver.pid, windowId);
  receipt.initialNotesWindowId = windowId;
  receipt.initialBounds = nativeBefore.bounds;
  receipt.footerHitRegions = state.footerHitRegions;
  await capture(driver, "before");

  const regions = state.footerHitRegions.regions as Region[];
  const height = Number(nativeBefore.bounds.height);
  const bottomRegions = regions.filter(
    (region) => region.bounds.y + region.bounds.height >= height - 6,
  );
  const safeSegment = widestSafeBottomSegment(
    Number(nativeBefore.bounds.width),
    bottomRegions,
  );
  const safeStart = {
    x: Number(nativeBefore.bounds.x) + safeSegment.x,
    // Four points inside the GPUI content view: far enough from AppKit's
    // ordinary frame hit-test that this proves the explicit GPUI handoff.
    y: Number(nativeBefore.bounds.y) + height - 4,
  };
  const safeEnd = { x: safeStart.x, y: safeStart.y + 80 };

  await announceTestStatus(
    "Notes bottom-edge drag",
    "Dragging the empty bottom edge; the window should resize continuously",
  );
  receipt.statusAnnouncements.push("Notes bottom-edge drag");
  const edge = await runDrag(driver.pid, windowId, safeStart, safeEnd, "edge-drag");
  await Bun.sleep(350);
  state = notesState(await driver.getTargetState(target));
  const edgeInitial = edge.result.initialBounds as Rect;
  const edgeFinal = edge.result.finalBounds as Rect;
  const edgePass =
    edge.result.status === "ok"
    && edge.result.untaggedInputCount === 0
    && boundsDistinct(edge.result.samples) >= 4
    && edgeFinal.height - edgeInitial.height >= 60
    && Math.abs(edgeFinal.x - edgeInitial.x) <= 1
    && Math.abs(edgeFinal.y - edgeInitial.y) <= 1
    && Math.abs(edgeFinal.width - edgeInitial.width) <= 1
    && state?.bottomResize?.route === "resizeStarted"
    && state?.view?.autoSizingEnabled === false;
  receipt.edgeTrial = {
    ...edge,
    safeSegment,
    start: safeStart,
    end: safeEnd,
    distinctHeights: boundsDistinct(edge.result.samples),
    route: state?.bottomResize,
    autoSizingEnabledAfter: state?.view?.autoSizingEnabled,
    pass: edgePass,
  };
  await capture(driver, "after-edge-resize");

  const grownWindow = await exactWindow(driver.pid, windowId);
  const grownRegions = state.footerHitRegions.regions as Region[];
  const shrinkSegment = widestSafeBottomSegment(
    Number(grownWindow.bounds.width),
    grownRegions,
  );
  const shrinkStart = {
    x: Number(grownWindow.bounds.x) + shrinkSegment.x,
    y: Number(grownWindow.bounds.y) + Number(grownWindow.bounds.height) - 4,
  };
  const shrinkEnd = { x: shrinkStart.x, y: shrinkStart.y - 40 };
  await announceTestStatus(
    "Notes bottom-edge shrink",
    "Dragging the empty bottom edge upward; the window should shrink continuously",
  );
  receipt.statusAnnouncements.push("Notes bottom-edge shrink");
  const shrink = await runDrag(
    driver.pid,
    windowId,
    shrinkStart,
    shrinkEnd,
    "edge-drag-shrink",
  );
  await Bun.sleep(350);
  state = notesState(await driver.getTargetState(target));
  const shrinkInitial = shrink.result.initialBounds as Rect;
  const shrinkFinal = shrink.result.finalBounds as Rect;
  const shrinkPass =
    shrink.result.status === "ok"
    && shrink.result.untaggedInputCount === 0
    && boundsDistinct(shrink.result.samples) >= 4
    && shrinkInitial.height - shrinkFinal.height >= 30
    && Math.abs(shrinkFinal.x - shrinkInitial.x) <= 1
    && Math.abs(shrinkFinal.y - shrinkInitial.y) <= 1
    && Math.abs(shrinkFinal.width - shrinkInitial.width) <= 1
    && state?.bottomResize?.route === "resizeStarted"
    && Number(state?.bottomResize?.beforeSize?.height) === shrinkInitial.height;
  receipt.shrinkTrial = {
    ...shrink,
    safeSegment: shrinkSegment,
    start: shrinkStart,
    end: shrinkEnd,
    distinctHeights: boundsDistinct(shrink.result.samples),
    route: state?.bottomResize,
    pass: shrinkPass,
  };
  await capture(driver, "after-shrink-resize");

  const resizedWindow = await exactWindow(driver.pid, windowId);
  const resizedRegions = state.footerHitRegions.regions as Region[];
  receipt.resizedFooterHitRegions = state.footerHitRegions;
  for (const region of resizedRegions) {
    await announceTestStatus(
      `Floating button ${region.index + 1}/${resizedRegions.length}`,
      `Dragging ${region.elementId}; the Notes frame must not resize`,
    );
    receipt.statusAnnouncements.push(
      `Floating button ${region.index + 1}/${resizedRegions.length}`,
    );
    const beforeState = notesState(await driver.getTargetState(target));
    const frame = await exactWindow(driver.pid, windowId);
    const localY = Math.min(
      region.bounds.y + region.bounds.height - 1,
      Number(frame.bounds.height) - 1,
    );
    const start = {
      x: Number(frame.bounds.x) + region.bounds.x + region.bounds.width / 2,
      y: Number(frame.bounds.y) + localY,
    };
    const end = { x: start.x, y: start.y - 60 };
    const trial = await runDrag(
      driver.pid,
      windowId,
      start,
      end,
      `button-${region.index}`,
    );
    await Bun.sleep(120);
    const afterState = notesState(await driver.getTargetState(target));
    const initial = trial.result.initialBounds as Rect;
    const final = trial.result.finalBounds as Rect;
    const noFrameChange = ["x", "y", "width", "height"].every(
      (key) => Math.abs(Number(final[key]) - Number(initial[key])) <= 1,
    );
    const noAction = JSON.stringify(invariantFingerprint(afterState))
      === JSON.stringify(invariantFingerprint(beforeState));
    receipt.buttonTrials.push({
      region,
      start,
      end,
      ...trial,
      route: afterState?.bottomResize,
      noFrameChange,
      noAction,
      pass: trial.result.untaggedInputCount === 0
        && noFrameChange
        && noAction
        && afterState?.bottomResize?.route !== "resizeStarted",
    });
  }
  await capture(driver, "after-button-drags");

  await announceTestStatus(
    "Notes resize persistence",
    "Closing and reopening Notes; the manually resized bounds should restore",
  );
  receipt.statusAnnouncements.push("Notes resize persistence");
  driver.send({ type: "openNotes", requestId: "notes-bottom-resize-close" });
  await Bun.sleep(850);
  driver.send({ type: "openNotes", requestId: "notes-bottom-resize-reopen" });
  const reopened = await waitForNotesReady(driver);
  const restoredWindowId = Number(reopened.entryReveal.nativeWindowNumber);
  const restoredWindow = await exactWindow(driver.pid, restoredWindowId);
  receipt.restoredNotesWindowId = restoredWindowId;
  receipt.persistence = {
    expected: resizedWindow.bounds,
    restored: restoredWindow.bounds,
    pass:
      Math.abs(Number(restoredWindow.bounds.width) - Number(resizedWindow.bounds.width)) <= 1
      && Math.abs(Number(restoredWindow.bounds.height) - Number(resizedWindow.bounds.height)) <= 1,
  };
  receipt.topology = {
    beforeWindowId: windowId,
    restoredWindowId,
    visibleNotesOwners: (await queryWindows(driver.pid)).windows.filter(
      (window: Json) => window.onscreen && window.alpha > 0,
    ),
  };
  await capture(driver, "after-reopen");

  const allButtonsPass = receipt.buttonTrials.length === resizedRegions.length
    && receipt.buttonTrials.every((trial: Json) => trial.pass === true);
  const noInterference =
    receipt.edgeTrial.result.untaggedInputCount === 0
    && receipt.shrinkTrial.result.untaggedInputCount === 0
    && receipt.buttonTrials.every(
      (trial: Json) => trial.result.untaggedInputCount === 0,
    );
  receipt.finalFingerprint = invariantFingerprint(reopened);
  receipt.pass = edgePass
    && shrinkPass
    && allButtonsPass
    && receipt.persistence.pass === true
    && noInterference
    && JSON.stringify(receipt.finalFingerprint)
      === JSON.stringify(beforeFingerprint);
  receipt.disposition = noInterference
    ? receipt.pass
      ? "EVALUABLE_PASS"
      : "EVALUABLE_FAIL"
    : "INVALID_INTERFERENCE";
} catch (error) {
  receipt.error = String(error);
  receipt.disposition = "INVALID_OBSERVER";
  receipt.pass = false;
} finally {
  driver.send({ type: "openNotes", requestId: "notes-bottom-resize-cleanup" });
  await Bun.sleep(350);
  await driver.close();
  receipt.cleanedUp = !driver.alive;
  receipt.finishedAt = new Date().toISOString();
  receipt.pass = receipt.pass === true && receipt.cleanedUp === true;
  receipt.disposition = receipt.disposition === "INVALID_INTERFERENCE"
    ? "INVALID_INTERFERENCE"
    : receipt.pass === true
    ? "EVALUABLE_PASS"
    : receipt.disposition ?? "EVALUABLE_FAIL";
  receipt.contractValidation = validateNotesBottomResizeReceipt(receipt);
  receipt.pass = receipt.pass === true && receipt.contractValidation.pass === true;
  const receiptPath = join(outDir, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({
    receiptPath,
    pass: receipt.pass,
    disposition: receipt.disposition,
    binarySha256: receipt.binarySha256,
  }, null, 2));
}

process.exit(receipt.pass ? 0 : 2);
