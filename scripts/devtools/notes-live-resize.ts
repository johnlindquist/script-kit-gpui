#!/usr/bin/env bun
/**
 * Notes native all-edge live-resize runtime probe.
 *
 * Proves, against the REAL app with REAL tagged CGEvents (never
 * simulateGpuiEvent for drags — GPUI-synthesized events cannot enter AppKit's
 * native live-resize tracking loop):
 *  - the resize interlock unlocks only after the full glass entry settle and
 *    sets styleMask bit 3 with the 350×280 content minimum;
 *  - each of the eight edge/corner directions natively resizes the panel;
 *  - the content minimum clamps at exactly 350×280;
 *  - Notes↔Agent switching re-partitions the backdrop (0 ↔ footer+8pt)
 *    without changing the outer frame;
 *  - a settled user frame persists across close/reopen while entry-morph
 *    frames never persist.
 *
 * Pass/fail semantics live in `notes-live-resize-contract.ts`, authored and
 * unit-tested BEFORE this probe existed (Oracle session
 * notes-resize-probe-plan, step 2/3).
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "./driver.ts";
import { announceTestStatus } from "./test-status.ts";
import {
  NATIVE_RESIZE_DIRECTIONS,
  REQUIRED_MOVING_EDGES,
  type DirectionAttempt,
  type DirectionTrial,
  type NativeResizeDirection,
  type NotesLiveResizeReceipt,
  type TrialDisposition,
  validateDirectionTrial,
  validateNotesLiveResizeReceipt,
} from "./notes-live-resize-contract.ts";

const arg = (name: string, fallback?: string) => {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
};
const binary = resolve(arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY ?? "")!);
const outDir = resolve(arg("--out", ".artifacts/notes-live-resize/final")!);
const themeFixture = arg("--theme-fixture");
if (!binary || !existsSync(binary)) throw new Error(`binary missing: ${binary}`);
mkdirSync(outDir, { recursive: true });

const sha256 = (path: string) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

// ── Compile native helpers ───────────────────────────────────────────────
function compileHelper(sourceRel: string, name: string): string {
  const source = resolve(import.meta.dir, sourceRel);
  const output = join(outDir, name);
  const compiled = Bun.spawnSync(["xcrun", "swiftc", "-O", source, "-o", output]);
  if (compiled.exitCode !== 0) {
    throw new Error(`${name} compile failed: ${compiled.stderr.toString()}`);
  }
  return output;
}
const dragHelper = compileHelper(
  "../agentic/macos-native-pointer-drag.swift",
  "macos-native-pointer-drag",
);
// Precompiled: the `swift` interpreter re-compiles per invocation (~1s),
// far too slow to sample frames inside the 280ms entry morph.
const windowQueryHelper = compileHelper(
  "../agentic/macos-window-query.swift",
  "macos-window-query",
);

const HIT_INSETS = [2, 4, 6];
const DRAG_PT = 80;
const target = { type: "id", id: "notes" };

type Rect = { x: number; y: number; width: number; height: number };

const receiptExtras: Json = {
  schemaVersion: 1,
  runId: `notes-live-resize-${Date.now()}-${process.pid}`,
  gitCommit: Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout.toString().trim(),
  binary,
  binarySha256: sha256(binary),
  dragHelperSha256: sha256(dragHelper),
  windowQueryHelperSha256: sha256(windowQueryHelper),
  themeFixture: themeFixture ? resolve(themeFixture) : null,
  themeFixtureSha256: themeFixture ? sha256(resolve(themeFixture)) : null,
  startedAt: new Date().toISOString(),
  statusAnnouncements: [],
  phases: {},
};

const contractReceipt: NotesLiveResizeReceipt = {
  directions: [],
  settleProof: null,
  minClamp: null,
  persistence: null,
  morph: null,
  ownerConsistent: true,
  cleanedUp: false,
  motionContractPass: null,
};

// ── Native window helpers ────────────────────────────────────────────────
async function queryWindows(pid: number): Promise<Json> {
  const child = Bun.spawn([windowQueryHelper, "--pid", String(pid)], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (code !== 0) throw new Error(`window query failed: ${stderr}`);
  return JSON.parse(stdout);
}

async function exactNotesWindow(pid: number, windowId: number): Promise<Json> {
  const query = await queryWindows(pid);
  const match = (query.windows ?? []).find(
    (window: Json) =>
      Number(window.windowId) === windowId && Number(window.ownerPid) === pid,
  );
  if (!match) throw new Error(`exact Notes window ${pid}/${windowId} not found`);
  return match;
}

/**
 * Same lookup with bounded polling: a freshly (re)opened panel can lag the
 * CGWindow list by a few frames right at the settle boundary, and an
 * exit-superseding reopen can briefly leave the list empty. A miss after the
 * timeout still throws — this never converts absence into success.
 */
async function exactNotesWindowPolled(
  pid: number,
  windowId: number,
  timeoutMs = 3_000,
): Promise<Json> {
  const deadline = performance.now() + timeoutMs;
  let lastError: unknown = null;
  while (performance.now() < deadline) {
    try {
      return await exactNotesWindow(pid, windowId);
    } catch (error) {
      lastError = error;
    }
    await Bun.sleep(50);
  }
  throw lastError instanceof Error
    ? lastError
    : new Error(`exact Notes window ${pid}/${windowId} not found after ${timeoutMs}ms`);
}

async function visibleNotesOwners(pid: number): Promise<Json[]> {
  const query = await queryWindows(pid);
  return (query.windows ?? []).filter(
    (window: Json) =>
      window.onscreen && Number(window.alpha) > 0 && Number(window.ownerPid) === pid,
  );
}

function runOsascript(script: string): { ok: boolean; stdout: string; stderr: string } {
  const child = Bun.spawnSync(["osascript", "-e", script]);
  return {
    ok: child.exitCode === 0,
    stdout: child.stdout.toString().trim(),
    stderr: child.stderr.toString().trim(),
  };
}

function activatePid(pid: number) {
  return runOsascript(
    `tell application "System Events" to set frontmost of (first application process whose unix id is ${pid}) to true`,
  );
}

async function setNotesFrameWithSystemEvents(
  pid: number,
  frame: Rect,
): Promise<{ ok: boolean; error: string | null }> {
  const script = `tell application "System Events"
    set proc to first application process whose unix id is ${pid}
    set wins to (every window of proc whose name contains "Notes")
    if (count of wins) is not 1 then error "expected exactly one Notes window, saw " & (count of wins)
    set win to item 1 of wins
    set position of win to {${Math.round(frame.x)}, ${Math.round(frame.y)}}
    set size of win to {${Math.round(frame.width)}, ${Math.round(frame.height)}}
  end tell`;
  const result = runOsascript(script);
  return { ok: result.ok, error: result.ok ? null : result.stderr };
}

async function waitForExactFrame(
  pid: number,
  windowId: number,
  expected: Rect,
  tolerancePt: number,
  timeoutMs = 3_000,
): Promise<Rect> {
  const deadline = performance.now() + timeoutMs;
  let last: Rect | null = null;
  while (performance.now() < deadline) {
    const window = await exactNotesWindow(pid, windowId);
    const bounds = window.bounds as Rect;
    last = bounds;
    if (
      Math.abs(bounds.x - expected.x) <= tolerancePt &&
      Math.abs(bounds.y - expected.y) <= tolerancePt &&
      Math.abs(bounds.width - expected.width) <= tolerancePt &&
      Math.abs(bounds.height - expected.height) <= tolerancePt
    ) {
      return bounds;
    }
    await Bun.sleep(30);
  }
  throw new Error(
    `frame did not reach ${JSON.stringify(expected)}; last ${JSON.stringify(last)}`,
  );
}

// ── Log receipt parsing ──────────────────────────────────────────────────
function readLogSlice(logPath: string, offset: number): { text: string; nextOffset: number } {
  if (!existsSync(logPath)) return { text: "", nextOffset: offset };
  const size = statSync(logPath).size;
  if (size <= offset) return { text: "", nextOffset: size };
  const full = readFileSync(logPath, "utf8");
  return { text: full.slice(offset), nextOffset: full.length };
}

/** Parse `key=value` / `key="value"` tokens from one tracing line. */
function parseTracingLine(line: string): Record<string, string> {
  const fields: Record<string, string> = {};
  for (const match of line.matchAll(/([A-Za-z0-9_.]+)=("([^"]*)"|\S+)/g)) {
    fields[match[1]] = match[3] !== undefined ? match[3] : match[2];
  }
  const timestamp = line.match(/\d{4}-\d{2}-\d{2}T[0-9:.]+Z?/);
  if (timestamp) fields.__timestamp = timestamp[0];
  return fields;
}

function extractNamedReceipts(logText: string, eventName: string) {
  return logText
    .split("\n")
    .filter((line) => line.includes(`event=${eventName}`) || line.includes(`event="${eventName}"`))
    .map((line) => ({ raw: line, fields: parseTracingLine(line) }));
}

// ── Interference accounting ─────────────────────────────────────────────
// NOTE: the whole-run ambient monitor (glass-interference.ts) is built for
// zero-input filmstrip probes: it counts EVERY session input event and any
// pointer displacement as interference, so this probe's own tagged drags
// (~550 real CGEvents) would always invalidate the run. Interference for a
// drag probe is owned by the drag helper's per-attempt tagged-event
// accounting instead (`untaggedInputCount` per attempt, enforced by the
// contract's evidence-validity rule).

await announceTestStatus(
  "Notes native resize probe",
  "Opening a sandbox Notes window; do not touch mouse or keyboard",
);
receiptExtras.statusAnnouncements.push("Notes native resize probe");

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `notes-live-resize-${process.pid}`,
  themeFixturePath: themeFixture ? resolve(themeFixture) : undefined,
  defaultTimeoutMs: 8_000,
  env: {
    SCRIPT_KIT_TEST_STATUS: "1",
    SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
    SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
  },
});

function notesState(envelope: Json): Json {
  return envelope?.notes ?? envelope ?? {};
}

async function waitForNotesRegistration(timeoutMs = 10_000): Promise<Json> {
  const deadline = performance.now() + timeoutMs;
  let last: Json = null;
  while (performance.now() < deadline) {
    try {
      last = notesState(await driver.getTargetState(target, { timeoutMs: 2_000 }));
      const reveal = last?.entryReveal;
      if (Number(reveal?.nativeWindowNumber) > 0 && reveal?.nativeConfigured === true) {
        return last;
      }
    } catch {}
    await Bun.sleep(20);
  }
  throw new Error(`Notes never registered natively: ${JSON.stringify(last)}`);
}

async function ensureSelectedNote(): Promise<Json> {
  let state = await waitForNotesRegistration();
  if (state?.selectedNote?.id) return state;
  await driver.simulateGpuiKeyDown("n", { modifiers: ["cmd"], target, timeoutMs: 5_000 });
  await driver.request(
    {
      type: "batch",
      target,
      commands: [{ type: "setInput", text: "Native live resize sandbox fixture" }],
      options: { stopOnError: true, rollbackOnError: false, timeout: 5_000 },
    },
    { expect: "batchResult", timeoutMs: 6_000 },
  );
  const deadline = performance.now() + 5_000;
  while (performance.now() < deadline) {
    state = await waitForNotesRegistration();
    if (state?.selectedNote?.id) return state;
    await Bun.sleep(40);
  }
  throw new Error("sandbox note did not become available");
}

async function surfaceMode(): Promise<string> {
  const state = notesState(await driver.getTargetState(target, { timeoutMs: 3_000 }));
  return String(state?.view?.surfaceMode ?? "unknown");
}

async function bottomResizeFingerprint(): Promise<string> {
  const state = notesState(await driver.getTargetState(target, { timeoutMs: 3_000 }));
  const receipt = state?.bottomResize;
  if (!receipt) return "none";
  return JSON.stringify({
    route: receipt.route,
    position: receipt.position,
    before: receipt.beforeSize,
  });
}

async function runNativeDrag(
  pid: number,
  windowId: number,
  start: { x: number; y: number },
  end: { x: number; y: number },
  label: string,
  durationMs = 600,
  steps = 48,
) {
  const path = join(outDir, `${label}.json`);
  const tag = Date.now() * 1_000 + Math.floor(Math.random() * 900);
  const child = Bun.spawn(
    [
      dragHelper,
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
      String(durationMs),
      "--steps",
      String(steps),
      "--event-user-data",
      String(tag),
      "--out",
      path,
    ],
    { stdout: "pipe", stderr: "pipe" },
  );
  const [stdout, stderr, code] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  const result = existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null;
  if (code !== 0 || !result) {
    throw new Error(`native drag ${label} failed: ${stderr || stdout}`);
  }
  return { path, sha256: sha256(path), result, tag };
}

function distinctFrames(samples: Json[]): number {
  return new Set(
    (samples ?? [])
      .map((sample) => sample?.bounds)
      .filter(Boolean)
      .map((bounds: Json) =>
        [bounds.x, bounds.y, bounds.width, bounds.height]
          .map((value) => Number(value).toFixed(2))
          .join(","),
      ),
  ).size;
}

function edgeDisplacements(initial: Rect, final: Rect) {
  return {
    left: final.x - initial.x,
    right: final.x + final.width - (initial.x + initial.width),
    top: final.y - initial.y,
    bottom: final.y + final.height - (initial.y + initial.height),
  };
}

async function postUpStability(pid: number, windowId: number): Promise<number> {
  const reads: Rect[] = [];
  for (let index = 0; index < 3; index += 1) {
    reads.push((await exactNotesWindow(pid, windowId)).bounds as Rect);
    if (index < 2) await Bun.sleep(75);
  }
  let maxDelta = 0;
  for (const key of ["x", "y", "width", "height"] as const) {
    const values = reads.map((rect) => Number(rect[key]));
    maxDelta = Math.max(maxDelta, Math.max(...values) - Math.min(...values));
  }
  return maxDelta;
}

function layoutComponent(layout: Json, name: string): Json | null {
  const components = layout?.components ?? layout?.layout?.components ?? [];
  return (
    (components as Json[]).find(
      (component) => component?.name === name || component?.id === name,
    ) ?? null
  );
}

async function notesLayout(): Promise<Json> {
  return await driver.getLayoutInfo({ target }, { timeoutMs: 5_000 });
}

let pinnedWindowId = 0;

try {
  // ── Open + pin ─────────────────────────────────────────────────────────
  await driver.request({ type: "show" }, { timeoutMs: 5_000 }).catch(() => null);
  const openLogOffset = existsSync(driver.logPath) ? statSync(driver.logPath).size : 0;
  driver.send({ type: "openNotes", requestId: "notes-live-resize-open" });
  let state = await ensureSelectedNote();
  pinnedWindowId = Number(state.entryReveal.nativeWindowNumber);
  const pid = driver.pid!;
  receiptExtras.pid = pid;
  receiptExtras.pinnedWindowId = pinnedWindowId;
  const initialNative = await exactNotesWindow(pid, pinnedWindowId);
  receiptExtras.initialBounds = initialNative.bounds;
  const owners = await visibleNotesOwners(pid);
  if (owners.filter((w: Json) => Number(w.windowId) === pinnedWindowId).length !== 1) {
    contractReceipt.ownerConsistent = false;
  }

  // ── Settle + style-mask proof ──────────────────────────────────────────
  const reveal = state.entryReveal;
  const configuredAtUnixMs = Number(reveal.nativeConfiguredAtUnixMs);
  const settleDurationMs = Number(reveal.settleDurationMs);
  const unlockAtUnixMs = configuredAtUnixMs + settleDurationMs;
  // Poll until the transition + policy receipts land (bounded past unlock).
  let phaseEntry: Record<string, string> | null = null;
  let policyEntry: Record<string, string> | null = null;
  const pollDeadline = Math.max(unlockAtUnixMs + 3_000, Date.now() + 3_000);
  while (Date.now() < pollDeadline && (!phaseEntry || !policyEntry)) {
    const slice = readLogSlice(driver.logPath, openLogOffset);
    const transitions = extractNamedReceipts(slice.text, "notes_native_resize_phase_transition")
      .filter((entry) => entry.fields.phase_after === "enabled");
    const policies = extractNamedReceipts(slice.text, "window_resize_policy_applied").filter(
      (entry) =>
        Number(entry.fields.window_number) === pinnedWindowId &&
        entry.fields.interaction_enabled === "true",
    );
    phaseEntry = transitions.at(-1)?.fields ?? null;
    if (phaseEntry) {
      receiptExtras.phaseTransitionRaw = transitions.at(-1)?.raw;
    }
    policyEntry = policies.at(-1)?.fields ?? null;
    if (policyEntry) receiptExtras.policyReceiptRaw = policies.at(-1)?.raw;
    if (!phaseEntry || !policyEntry) await Bun.sleep(40);
  }
  // "Not early": the enabled line's own timestamp must be at/after the
  // computed unlock deadline (50ms scheduling slack).
  let enabledBeforeDeadline = false;
  if (phaseEntry?.__timestamp) {
    const lineMs = Date.parse(phaseEntry.__timestamp);
    if (Number.isFinite(lineMs)) {
      enabledBeforeDeadline = lineMs < unlockAtUnixMs - 50;
    }
  }
  const styleMaskAfter = Number(policyEntry?.style_mask_after ?? 0);
  contractReceipt.settleProof = {
    disposition: phaseEntry && policyEntry ? "EVALUABLE_PASS" : "EVALUABLE_FAIL",
    phaseBefore: phaseEntry?.phase_before ?? null,
    phaseAfter: phaseEntry?.phase_after ?? null,
    reason: phaseEntry?.reason ?? null,
    interactionEnabled: phaseEntry?.interaction_enabled === "true",
    nativeApplyOk: phaseEntry?.native_apply_ok === "true",
    enabledBeforeDeadline,
    styleMaskAfterHasResizableBit: (styleMaskAfter & 8) === 8,
    policyUserResizable: policyEntry?.user_resizable === "true",
    policyMinWidth: policyEntry ? Number(policyEntry.min_content_width) : null,
    policyMinHeight: policyEntry ? Number(policyEntry.min_content_height) : null,
    // Option<f64> fields are omitted from tracing output when None; a
    // present numeric value would mean an authored maximum exists (which the
    // Notes policy forbids).
    policyMaxWidth:
      policyEntry?.max_content_width !== undefined &&
      policyEntry.max_content_width !== "None"
        ? Number(policyEntry.max_content_width)
        : null,
    policyMaxHeight:
      policyEntry?.max_content_height !== undefined &&
      policyEntry.max_content_height !== "None"
        ? Number(policyEntry.max_content_height)
        : null,
    policyWindowMatchesPinned: Number(policyEntry?.window_number) === pinnedWindowId,
  };
  receiptExtras.settleTiming = { configuredAtUnixMs, settleDurationMs, unlockAtUnixMs };

  // ── Normalize frame to 500×400 baseline ────────────────────────────────
  activatePid(pid);
  const initial = initialNative.bounds as Rect;
  const baseline: Rect = {
    x: initial.x + initial.width - 500,
    y: initial.y + 60,
    width: 500,
    height: 400,
  };
  const framer = await setNotesFrameWithSystemEvents(pid, baseline);
  if (!framer.ok) {
    throw new Error(`BLOCKED_ENVIRONMENT: System Events frame setup failed: ${framer.error}`);
  }
  await waitForExactFrame(pid, pinnedWindowId, baseline, 1);
  receiptExtras.baseline = baseline;

  // ── Eight-direction matrix ─────────────────────────────────────────────
  await announceTestStatus(
    "Notes native drags",
    "Dragging every edge and corner of the Notes window; do not touch input",
  );
  receiptExtras.statusAnnouncements.push("Notes native drags");

  for (const direction of NATIVE_RESIZE_DIRECTIONS) {
    const attempts: DirectionAttempt[] = [];
    const attemptDetails: Json[] = [];
    let disposition: TrialDisposition = "EVALUABLE_FAIL";
    let selectedInsetPt: number | null = null;

    for (const inset of HIT_INSETS) {
      // Reset to exact baseline before every attempt.
      const reset = await setNotesFrameWithSystemEvents(pid, baseline);
      if (!reset.ok) throw new Error(`baseline reset failed: ${reset.error}`);
      const frame = await waitForExactFrame(pid, pinnedWindowId, baseline, 1);
      const L = frame.x;
      const T = frame.y;
      const R = frame.x + frame.width;
      const B = frame.y + frame.height;
      const midY = T + frame.height / 2;
      const midX = L + frame.width / 2;
      const topX = L + 90; // outside traffic lights, left of centered switcher
      const points: Record<NativeResizeDirection, { start: { x: number; y: number }; end: { x: number; y: number } }> = {
        L: { start: { x: L + inset, y: midY }, end: { x: L + inset + DRAG_PT, y: midY } },
        R: { start: { x: R - inset, y: midY }, end: { x: R - inset - DRAG_PT, y: midY } },
        T: { start: { x: topX, y: T + inset }, end: { x: topX, y: T + inset + DRAG_PT } },
        B: { start: { x: midX, y: B - inset }, end: { x: midX, y: B - inset - DRAG_PT } },
        TL: { start: { x: L + inset, y: T + inset }, end: { x: L + inset + DRAG_PT, y: T + inset + DRAG_PT } },
        TR: { start: { x: R - inset, y: T + inset }, end: { x: R - inset - DRAG_PT, y: T + inset + DRAG_PT } },
        BL: { start: { x: L + inset, y: B - inset }, end: { x: L + inset + DRAG_PT, y: B - inset - DRAG_PT } },
        BR: { start: { x: R - inset, y: B - inset }, end: { x: R - inset - DRAG_PT, y: B - inset - DRAG_PT } },
      };
      const { start, end } = points[direction];
      const routeBefore = await bottomResizeFingerprint();
      let drag;
      try {
        drag = await runNativeDrag(
          pid,
          pinnedWindowId,
          start,
          end,
          `dir-${direction}-inset-${inset}`,
        );
      } catch (error) {
        attemptDetails.push({ inset, error: String(error) });
        disposition = "INVALID_OBSERVER";
        continue;
      }
      await Bun.sleep(200);
      const routeAfter = await bottomResizeFingerprint();
      const initialBounds = drag.result.initialBounds as Rect;
      const finalBounds = drag.result.finalBounds as Rect;
      const postUp = await postUpStability(pid, pinnedWindowId);
      const ownerStable = await exactNotesWindow(pid, pinnedWindowId)
        .then(() => true)
        .catch(() => false);
      const legacySeen =
        routeAfter !== routeBefore && routeAfter.includes('"route":"resizeStarted"');
      const attempt: DirectionAttempt = {
        insetPt: inset,
        helperStatus: String(drag.result.status ?? "unknown"),
        untaggedInputCount: Number(drag.result.untaggedInputCount ?? 0),
        distinctFrameCount: distinctFrames(drag.result.samples),
        displacements: edgeDisplacements(initialBounds, finalBounds),
        finalWidth: Number(finalBounds.width),
        finalHeight: Number(finalBounds.height),
        ownerStable,
        postUpStablePt: postUp,
        legacyResizeStartedSeen: legacySeen,
      };
      attempts.push(attempt);
      attemptDetails.push({ inset, start, end, drag: { path: drag.path, sha256: drag.sha256 }, attempt });
      if (attempt.untaggedInputCount > 0) {
        disposition = "INVALID_INTERFERENCE";
        break;
      }
      if (validateDirectionTrial(direction, attempt).pass) {
        disposition = "EVALUABLE_PASS";
        selectedInsetPt = inset;
        break;
      }
    }
    if (
      disposition !== "EVALUABLE_PASS" &&
      disposition !== "INVALID_INTERFERENCE" &&
      disposition !== "INVALID_OBSERVER"
    ) {
      disposition = attempts.length > 0 ? "EVALUABLE_FAIL" : "INVALID_OBSERVER";
    }
    const trial: DirectionTrial = { direction, disposition, attempts, selectedInsetPt };
    contractReceipt.directions.push(trial);
    (receiptExtras.phases as Json)[`direction-${direction}`] = {
      disposition,
      selectedInsetPt,
      attempts: attemptDetails,
    };
  }

  // ── Minimum clamp ──────────────────────────────────────────────────────
  await announceTestStatus(
    "Notes minimum clamp",
    "Dragging far past the 350×280 minimum; the window must clamp",
  );
  receiptExtras.statusAnnouncements.push("Notes minimum clamp");
  const rPass = contractReceipt.directions.find((t) => t.direction === "R")?.disposition;
  const bPass = contractReceipt.directions.find((t) => t.direction === "B")?.disposition;
  if (rPass === "EVALUABLE_PASS" && bPass === "EVALUABLE_PASS") {
    await setNotesFrameWithSystemEvents(pid, baseline);
    const frame = await waitForExactFrame(pid, pinnedWindowId, baseline, 1);
    const midY = frame.y + frame.height / 2;
    const routeBefore = await bottomResizeFingerprint();
    await runNativeDrag(
      pid,
      pinnedWindowId,
      { x: frame.x + frame.width - 2, y: midY },
      { x: frame.x + frame.width - 2 - 300, y: midY },
      "min-clamp-right",
      800,
      64,
    );
    await Bun.sleep(250);
    const afterRight = (await exactNotesWindow(pid, pinnedWindowId)).bounds as Rect;
    await runNativeDrag(
      pid,
      pinnedWindowId,
      { x: afterRight.x + afterRight.width / 2, y: afterRight.y + afterRight.height - 2 },
      { x: afterRight.x + afterRight.width / 2, y: afterRight.y + afterRight.height - 2 - 260 },
      "min-clamp-bottom",
      800,
      64,
    );
    await Bun.sleep(400);
    const routeAfter = await bottomResizeFingerprint();
    const finalNative = (await exactNotesWindow(pid, pinnedWindowId)).bounds as Rect;
    const layout = await notesLayout();
    const windowComponent = layoutComponent(layout, "NotesWindow");
    const contentWidth = Number(
      windowComponent?.bounds?.width ?? windowComponent?.width ?? finalNative.width,
    );
    const contentHeight = Number(
      windowComponent?.bounds?.height ?? windowComponent?.height ?? finalNative.height,
    );
    contractReceipt.minClamp = {
      disposition: "EVALUABLE_PASS",
      finalContentWidth: contentWidth,
      finalContentHeight: contentHeight,
      leftDriftPt: finalNative.x - baseline.x,
      topDriftPt: finalNative.y - baseline.y,
      ownerStable: true,
      legacyResizeStartedSeen:
        routeAfter !== routeBefore && routeAfter.includes('"route":"resizeStarted"'),
    };
    receiptExtras.minClampNative = { afterRight, finalNative, contentWidth, contentHeight };
  } else {
    // Decision rule: the minimum-by-drag clause needs functional R and B.
    contractReceipt.minClamp = {
      disposition: "EVALUABLE_FAIL",
      finalContentWidth: 0,
      finalContentHeight: 0,
      leftDriftPt: 0,
      topDriftPt: 0,
      ownerStable: true,
      legacyResizeStartedSeen: false,
    };
    receiptExtras.minClampSkipped = { rPass, bPass };
  }

  // ── Stable persistence ─────────────────────────────────────────────────
  await announceTestStatus(
    "Notes persistence",
    "Resizing, closing, and reopening; the user frame must restore",
  );
  receiptExtras.statusAnnouncements.push("Notes persistence");
  await setNotesFrameWithSystemEvents(pid, baseline);
  const persistBase = await waitForExactFrame(pid, pinnedWindowId, baseline, 1);
  // Preferred direction for the dedicated user drag: BR, else any passing.
  const dragOrder: NativeResizeDirection[] = ["BR", "R", "B", "L", "T", "TL", "TR", "BL"];
  const persistDirection = dragOrder.find(
    (direction) =>
      contractReceipt.directions.find((t) => t.direction === direction)?.disposition ===
      "EVALUABLE_PASS",
  );
  if (persistDirection) {
    const R = persistBase.x + persistBase.width;
    const B = persistBase.y + persistBase.height;
    await runNativeDrag(
      pid,
      pinnedWindowId,
      { x: R - 2, y: B - 2 },
      { x: R - 2 - 40, y: B - 2 - 40 },
      "persistence-drag",
    );
    await Bun.sleep(300);
    const settled = (await exactNotesWindow(pid, pinnedWindowId)).bounds as Rect;
    receiptExtras.persistedFrame = settled;
    await Bun.sleep(750);
    for (let index = 0; index < 3; index += 1) {
      await driver.simulateGpuiEvent(
        { type: "mouseMove", x: 120 + index, y: 100 },
        { target, timeoutMs: 5_000 },
      );
      await Bun.sleep(20);
    }
    driver.send({ type: "openNotes", requestId: "notes-live-resize-close" });
    const closeDeadline = performance.now() + 5_000;
    while (performance.now() < closeDeadline) {
      if ((await visibleNotesOwners(pid)).length === 0) break;
      await Bun.sleep(50);
    }
    driver.send({ type: "openNotes", requestId: "notes-live-resize-reopen" });
    const reopened = await ensureSelectedNote();
    const reopenedWindowId = Number(reopened.entryReveal.nativeWindowNumber);
    // Wait through the full settle/unlock of the reopened window.
    const reopenSettle =
      Number(reopened.entryReveal.nativeConfiguredAtUnixMs) +
      Number(reopened.entryReveal.settleDurationMs);
    while (Date.now() < reopenSettle + 200) await Bun.sleep(25);
    const restored = (await exactNotesWindow(pid, reopenedWindowId)).bounds as Rect;
    receiptExtras.restoredFrame = restored;
    contractReceipt.persistence = {
      disposition: "EVALUABLE_PASS",
      widthDeltaPt: Number(restored.width) - Number(settled.width),
      heightDeltaPt: Number(restored.height) - Number(settled.height),
      originDeltaPt: Math.max(
        Math.abs(Number(restored.x) - Number(settled.x)),
        Math.abs(Number(restored.y) - Number(settled.y)),
      ),
      restoredDefaultFallback:
        Math.abs(Number(restored.width) - 350) <= 1 &&
        Math.abs(Number(restored.height) - 280) <= 1,
    };
    pinnedWindowId = reopenedWindowId;

    // ── Morph non-persistence ────────────────────────────────────────────
    await announceTestStatus(
      "Notes morph guard",
      "Reopening and closing mid-morph; transient frames must not persist",
    );
    receiptExtras.statusAnnouncements.push("Notes morph guard");

    // One scripted retry per the plan's decision rule: a missed transient
    // capture or a raced window-list lookup is INVALID_OBSERVER, repeated
    // once with polling pre-armed — never a product verdict by itself.
    const morphAttempts: Json[] = [];
    for (let round = 0; round < 2 && contractReceipt.morph?.disposition !== "EVALUABLE_PASS"; round += 1) {
      try {
        driver.send({ type: "openNotes", requestId: `notes-live-resize-morph-close-${round}` });
        const morphCloseDeadline = performance.now() + 5_000;
        while (performance.now() < morphCloseDeadline) {
          if ((await visibleNotesOwners(pid)).length === 0) break;
          await Bun.sleep(50);
        }
        // Give the 135ms exit tail time to fully remove the window so the
        // reopen creates a fresh entry morph instead of superseding.
        await Bun.sleep(300);
        // Polling armed BEFORE the reopen.
        let transient: Rect | null = null;
        let closeSentAtUnixMs = 0;
        let morphDeadlineUnixMs = 0;
        driver.send({ type: "openNotes", requestId: `notes-live-resize-morph-open-${round}` });
        const pollDeadlineMorph = performance.now() + 4_000;
        while (performance.now() < pollDeadlineMorph) {
          try {
            const owners = await visibleNotesOwners(pid);
            const candidate = owners[0];
            if (candidate) {
              const bounds = candidate.bounds as Rect;
              const differs =
                Math.abs(bounds.width - settled.width) >= 3 ||
                Math.abs(bounds.height - settled.height) >= 3 ||
                Math.abs(bounds.x - settled.x) >= 3 ||
                Math.abs(bounds.y - settled.y) >= 3;
              if (differs && transient === null) {
                transient = bounds;
                // Close IMMEDIATELY, before full settle.
                const morphState = notesState(
                  await driver.getTargetState(target, { timeoutMs: 1_500 }),
                );
                morphDeadlineUnixMs =
                  Number(morphState?.entryReveal?.nativeConfiguredAtUnixMs ?? 0) +
                  Number(morphState?.entryReveal?.settleDurationMs ?? 0);
                closeSentAtUnixMs = Date.now();
                driver.send({
                  type: "openNotes",
                  requestId: `notes-live-resize-morph-recluse-${round}`,
                });
                break;
              }
            }
          } catch {}
          await Bun.sleep(5);
        }
        // If no transient was captured the window is still OPEN — close it
        // explicitly so the upcoming final-open cannot toggle a live window
        // closed (which would invert the whole sequence).
        if (transient === null && (await visibleNotesOwners(pid)).length > 0) {
          driver.send({
            type: "openNotes",
            requestId: `notes-live-resize-morph-flush-${round}`,
          });
        }
        const morphGoneDeadline = performance.now() + 5_000;
        while (performance.now() < morphGoneDeadline) {
          if ((await visibleNotesOwners(pid)).length === 0) break;
          await Bun.sleep(50);
        }
        await Bun.sleep(300);
        driver.send({ type: "openNotes", requestId: `notes-live-resize-final-open-${round}` });
        const finalState = await ensureSelectedNote();
        const finalWindowId = Number(finalState.entryReveal.nativeWindowNumber);
        const finalSettle =
          Number(finalState.entryReveal.nativeConfiguredAtUnixMs) +
          Number(finalState.entryReveal.settleDurationMs);
        while (Date.now() < finalSettle + 200) await Bun.sleep(25);
        const finalFrame = (await exactNotesWindowPolled(pid, finalWindowId)).bounds as Rect;
        morphAttempts.push({ round, settled, transient, finalFrame, closeSentAtUnixMs, morphDeadlineUnixMs });
        const finalMatchesSettledPt = Math.max(
          Math.abs(Number(finalFrame.width) - Number(settled.width)),
          Math.abs(Number(finalFrame.height) - Number(settled.height)),
        );
        const evaluable = transient !== null && closeSentAtUnixMs > 0;
        contractReceipt.morph = {
          disposition: evaluable ? "EVALUABLE_PASS" : "INVALID_OBSERVER",
          transientFrameCaptured: transient !== null,
          closeBeforeSettle:
            closeSentAtUnixMs > 0 &&
            morphDeadlineUnixMs > 0 &&
            closeSentAtUnixMs < morphDeadlineUnixMs,
          finalMatchesSettledPt,
          finalMatchesTransientOnly:
            transient !== null &&
            finalMatchesSettledPt > 1 &&
            Math.abs(Number(finalFrame.width) - Number(transient.width)) <= 1 &&
            Math.abs(Number(finalFrame.height) - Number(transient.height)) <= 1,
        };
        pinnedWindowId = finalWindowId;
      } catch (error) {
        const windowsSnapshot = await queryWindows(pid).catch(() => null);
        morphAttempts.push({ round, error: String(error), windowsSnapshot });
        contractReceipt.morph = {
          disposition: "INVALID_OBSERVER",
          transientFrameCaptured: false,
          closeBeforeSettle: false,
          finalMatchesSettledPt: Number.NaN,
          finalMatchesTransientOnly: false,
        };
        // Recover a live Notes window for the next round / cleanup.
        try {
          if ((await visibleNotesOwners(pid)).length === 0) {
            driver.send({ type: "openNotes", requestId: `notes-live-resize-morph-recover-${round}` });
            const recovered = await ensureSelectedNote();
            pinnedWindowId = Number(recovered.entryReveal.nativeWindowNumber);
          }
        } catch {}
      }
    }
    receiptExtras.morphFrames = morphAttempts;
  } else {
    contractReceipt.persistence = {
      disposition: "EVALUABLE_FAIL",
      widthDeltaPt: Number.NaN,
      heightDeltaPt: Number.NaN,
      originDeltaPt: Number.NaN,
      restoredDefaultFallback: false,
    };
    contractReceipt.morph = {
      disposition: "EVALUABLE_FAIL",
      transientFrameCaptured: false,
      closeBeforeSettle: false,
      finalMatchesSettledPt: Number.NaN,
      finalMatchesTransientOnly: false,
    };
    receiptExtras.persistenceSkipped = "no direction passed for the dedicated user drag";
  }
} catch (error) {
  receiptExtras.error = String(error);
  const blocked = String(error).includes("BLOCKED_ENVIRONMENT");
  for (const direction of NATIVE_RESIZE_DIRECTIONS) {
    if (!contractReceipt.directions.some((trial) => trial.direction === direction)) {
      contractReceipt.directions.push({
        direction,
        disposition: blocked ? "BLOCKED_ENVIRONMENT" : "INVALID_OBSERVER",
        attempts: [],
        selectedInsetPt: null,
      });
    }
  }
} finally {
  try {
    driver.send({ type: "openNotes", requestId: "notes-live-resize-cleanup" });
    await Bun.sleep(400);
  } catch {}
  await driver.close();
  contractReceipt.cleanedUp = !driver.alive;
  const totalUntagged = contractReceipt.directions.reduce(
    (sum, trial) =>
      sum +
      trial.attempts.reduce((inner, attempt) => inner + attempt.untaggedInputCount, 0),
    0,
  );
  receiptExtras.interference = {
    source: "per-attempt tagged CGEvent accounting (drag helper)",
    totalUntaggedInputCount: totalUntagged,
    disposition: totalUntagged === 0 ? "CLEAN" : "INVALID_INTERFERENCE",
  };

  const verdict = validateNotesLiveResizeReceipt(contractReceipt);
  const receipt: Json = {
    ...receiptExtras,
    ...contractReceipt,
    finishedAt: new Date().toISOString(),
    verdict,
    evidenceValid: verdict.evidenceValid,
    allDirectionsPass:
      verdict.evidenceValid && verdict.failedDirections.length === 0,
    failedDirections: verdict.failedDirections,
    nonDirectionalContractPass: verdict.nonDirectionalContractPass,
    landingReady: verdict.landingReady,
    recommendedRootCallDisposition: verdict.recommendedRootCallDisposition,
    pass: verdict.productPass && contractReceipt.cleanedUp,
    disposition: !verdict.evidenceValid
      ? receiptExtras.interference?.disposition === "INVALID_INTERFERENCE"
        ? "INVALID_INTERFERENCE"
        : "INVALID_OBSERVER"
      : verdict.productPass
        ? "EVALUABLE_PASS"
        : "EVALUABLE_FAIL",
  };
  const receiptPath = join(outDir, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(
    JSON.stringify(
      {
        receiptPath,
        disposition: receipt.disposition,
        pass: receipt.pass,
        landingReady: receipt.landingReady,
        failedDirections: receipt.failedDirections,
        recommendedRootCallDisposition: receipt.recommendedRootCallDisposition,
        binarySha256: receipt.binarySha256,
      },
      null,
      2,
    ),
  );
  process.exit(receipt.pass ? 0 : 2);
}
