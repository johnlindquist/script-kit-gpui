#!/usr/bin/env bun
/**
 * scripts/agentic/window.ts
 *
 * Window discovery, focus, activation, and screenshot capture for
 * Script Kit GPUI agentic testing. Single authority for window focus
 * and capture — other scripts (macos-input.ts) delegate here.
 *
 * Usage:
 *   bun scripts/agentic/window.ts find   [--title SUBSTR] [--json]
 *   bun scripts/agentic/window.ts focus  [--title SUBSTR] [--retry N] [--settle-ms MS] [--json]
 *   bun scripts/agentic/window.ts status [--json]
 *   bun scripts/agentic/window.ts capture PATH [--title SUBSTR] [--retry N] [--activate-first] [--settle-ms MS] [--window-id N] [--json]
 *
 * All structured output is JSON on stdout. Diagnostics go to stderr as NDJSON.
 */

import { existsSync, statSync } from "fs";
import {
  assertNoninteractiveSubprocess,
  NoninteractiveSafetyError,
} from "../devtools/lib/operator-safety.ts";

const SCHEMA_VERSION = 2;
const APP_NAME = "Script Kit";
const DEFAULT_TITLE_SUBSTR = "";
const DEFAULT_RETRY = 1;
const DEFAULT_SETTLE_MS = 300;
const DEFAULT_CAPTURE_RETRY = 1;
const DEFAULT_CAPTURE_SETTLE_MS = 200;

// The process/owner name reported by CGWindowListCopyWindowInfo for the
// Script Kit GPUI binary. This is the executable name ("script-kit-gpui"),
// NOT the user-visible display name ("Script Kit") — the two differ because
// the app uses NSApp.setActivationPolicy(.accessory) and ships no Info.plist
// CFBundleName alias at enumeration time. Any `--title "Script Kit"` filter
// the caller supplies must transparently match this owner name.
const QUARTZ_OWNER_NAME = "script-kit-gpui";

// Layer threshold at which a Script Kit window is considered "above normal
// windows" and therefore frontmost-enough for CGEvent delivery. Main panel
// configured at NSFloatingWindowLevel (3); Quartz reports layer=101 in
// practice because the WindowServer maps NSFloatingWindowLevel into its
// own layer namespace. Detached popups may report other elevated layers
// (≥20 for most panel subtypes). Anything ≥ 3 is a non-desktop, non-normal
// window — treat it as "frontmost" for our purposes.
const PANEL_FRONTMOST_LAYER_MIN = 3;

// Resolved swift helper path (sibling file).
const MACOS_WINDOW_QUERY_SWIFT = new URL(
  "./macos-window-query.swift",
  import.meta.url
).pathname;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface Envelope<T> {
  schemaVersion: number;
  status: "ok" | "error";
  command: string;
  data?: T;
  error?: { code: string; message: string };
}

interface WindowInfo {
  windowId: number;
  ownerName: string;
  title: string;
  layer: number;
  bounds: { x: number; y: number; width: number; height: number } | null;
  frontmost: boolean;
  focused: boolean;
  visible: boolean;
}

interface FindResult {
  windows: WindowInfo[];
  appRunning: boolean;
}

interface FocusResult {
  activated: boolean;
  frontmost: boolean;
  focused: boolean;
  keyboardReady: boolean;
  osFrontmostProcess: string | null;
  attempts: number;
  settleMs: number;
  windowId: number;
}

interface StatusResult {
  appRunning: boolean;
  frontmost: boolean;
  keyboardReady: boolean;
  osFrontmostProcess: string | null;
  visibleWindows: number;
  windows: WindowInfo[];
}

/** Stable automation-surface identity for Agent Chat-relevant windows. */
interface AutomationSurface {
  /** Stable identity: "main", "agent_chat", "actions", "notes", "ai", or window title. */
  surfaceId: string;
  /** Window kind hint: "main", "popup", "panel", or "unknown". */
  kind: string;
  windowId: number;
  title: string;
  frontmost: boolean;
  focused: boolean;
  visible: boolean;
  bounds: { x: number; y: number; width: number; height: number } | null;
}

interface ListResult {
  surfaces: AutomationSurface[];
  appRunning: boolean;
  focusedSurfaceId: string | null;
}

interface CaptureResult {
  path: string;
  windowId: number;
  attempts: number;
  method: "quartz" | "screencapture";
  frontmost: boolean;
  focused: boolean;
  sizeBytes: number;
  width: number | null;
  height: number | null;
}

// ---------------------------------------------------------------------------
// Structured stderr logging
// ---------------------------------------------------------------------------

function stderrLog(event: string, fields: Record<string, unknown> = {}): void {
  console.error(JSON.stringify({ event, ts: new Date().toISOString(), ...fields }));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function envelope<T>(command: string, data: T): Envelope<T> {
  return { schemaVersion: SCHEMA_VERSION, status: "ok", command, data };
}

function errorEnvelope(
  command: string,
  code: string,
  message: string
): Envelope<never> {
  return {
    schemaVersion: SCHEMA_VERSION,
    status: "error",
    command,
    error: { code, message },
  };
}

async function runOsascript(script: string): Promise<string> {
  const command = ["osascript", "-e", script];
  assertNoninteractiveSubprocess(command);
  const proc = Bun.spawn(command, {
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;
  if (code !== 0) {
    const msg = stderr.trim() || `osascript exit ${code}`;
    if (
      msg.includes("not allowed assistive access") ||
      msg.includes("is not allowed to send keystrokes") ||
      msg.includes("(-1743)")
    ) {
      throw Object.assign(new Error(msg), { code: "ACCESSIBILITY_DENIED" });
    }
    if (msg.includes("(-600)") || msg.includes("Application isn't running")) {
      throw Object.assign(new Error(msg), { code: "APP_NOT_RUNNING" });
    }
    throw Object.assign(new Error(msg), { code: "OSASCRIPT_ERROR" });
  }
  return stdout.trim();
}

async function runCommand(
  cmd: string[],
  label: string
): Promise<string> {
  assertNoninteractiveSubprocess(cmd);
  const proc = Bun.spawn(cmd, {
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;
  if (code !== 0) {
    throw Object.assign(new Error(stderr.trim() || `${label} exit ${code}`), {
      code: "COMMAND_FAILED",
    });
  }
  return stdout.trim();
}

async function currentFrontmostProcessName(): Promise<string | null> {
  try {
    const name = await runOsascript(
      'tell application "System Events" to get name of first application process whose frontmost is true'
    );
    return name || null;
  } catch (e: any) {
    stderrLog("frontmost_process_check_failed", { error: e.message });
    return null;
  }
}

function isScriptKitFrontmostProcess(name: string | null): boolean {
  return name
    ? /^(script-kit-gpui|Script Kit|Script Kit GPUI)$/i.test(name)
    : false;
}

// ---------------------------------------------------------------------------
// Window ID resolution (Quartz CGWindowListCopyWindowInfo via Swift helper)
// ---------------------------------------------------------------------------

/**
 * Shape returned by `scripts/agentic/macos-window-query.swift` per window.
 * Kept loose (record) because the swift script is the single source of truth
 * for the exact field names — window.ts is a consumer.
 */
interface QuartzWindowRecord {
  windowId: number;
  ownerName: string;
  ownerPid: number;
  title: string;
  layer: number;
  alpha: number;
  onscreen: boolean;
  storeType: number;
  bounds: { x: number; y: number; width: number; height: number };
}

/**
 * Normalize a string for fuzzy matching: lowercase, strip non-alphanumerics.
 * This lets `--title "Script Kit"` match an owner name of "script-kit-gpui",
 * which is required because macOS `accessory` apps surface their executable
 * name (not the display name) in CGWindowListCopyWindowInfo.
 */
function normalizeForMatch(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "");
}

/**
 * Return true if either the window title or the owner process name matches
 * the user-supplied title substring. Matching is done on a normalized form
 * (lowercase, alphanumerics only) so "Script Kit" matches "script-kit-gpui".
 */
function windowMatchesTitle(
  w: QuartzWindowRecord,
  titleSubstr: string
): boolean {
  if (titleSubstr.length === 0) return true;
  const needle = normalizeForMatch(titleSubstr);
  if (needle.length === 0) return true;
  const title = normalizeForMatch(w.title ?? "");
  const owner = normalizeForMatch(w.ownerName ?? "");
  return title.includes(needle) || owner.includes(needle);
}

function isFooterOverlayRecord(w: QuartzWindowRecord): boolean {
  const title = (w.title ?? "").toLowerCase();
  return (
    title.includes("footer overlay") ||
    (w.bounds.height <= 80 && w.bounds.width >= 300)
  );
}

/**
 * Call the Swift CGWindowList helper. Returns an empty list on any error
 * (logged to stderr). Prefers swift over python because macOS ships swift
 * but not PyObjC — see the swift file's header for the full rationale.
 */
async function runSwiftWindowQuery(): Promise<QuartzWindowRecord[]> {
  try {
    const command = ["swift", MACOS_WINDOW_QUERY_SWIFT, "--owner", QUARTZ_OWNER_NAME];
    assertNoninteractiveSubprocess(command);
    const proc = Bun.spawn(
      command,
      { stdout: "pipe", stderr: "pipe" }
    );
    const stdout = await new Response(proc.stdout).text();
    const stderr = await new Response(proc.stderr).text();
    const code = await proc.exited;
    if (code !== 0) {
      stderrLog("window_query_swift_nonzero_exit", {
        exitCode: code,
        stderr: stderr.slice(0, 400),
      });
      return [];
    }
    const parsed = JSON.parse(stdout);
    if (parsed?.status !== "ok" || !Array.isArray(parsed.windows)) {
      stderrLog("window_query_swift_bad_envelope", {
        status: parsed?.status,
        errorCode: parsed?.error?.code,
      });
      return [];
    }
    return parsed.windows as QuartzWindowRecord[];
  } catch (e: any) {
    stderrLog("window_query_swift_error", { message: e.message });
    return [];
  }
}

/**
 * Resolve the CGWindowID for a Script Kit window.
 * Returns the numeric ID (>0) or 0 if no window found.
 * Prefers the Swift Quartz helper for reliable IDs that screencapture -l
 * accepts, falls back to AX.
 */
async function resolveWindowId(titleSubstr: string): Promise<{ windowId: number; method: "quartz" | "ax" }> {
  const windows = await runSwiftWindowQuery();
  if (windows.length > 0) {
    // Prefer panel windows (layer >= PANEL_FRONTMOST_LAYER_MIN, onscreen)
    // over off-screen or desktop-level windows, so screencapture -l picks
    // the live visible panel when multiple app instances exist.
    const ranked = [...windows].sort((a, b) => {
      const aScore =
        (a.onscreen ? 100 : 0) +
        (a.layer >= PANEL_FRONTMOST_LAYER_MIN ? 10 : 0) +
        (isFooterOverlayRecord(a) ? -1000 : 0) +
        a.bounds.height;
      const bScore =
        (b.onscreen ? 100 : 0) +
        (b.layer >= PANEL_FRONTMOST_LAYER_MIN ? 10 : 0) +
        (isFooterOverlayRecord(b) ? -1000 : 0) +
        b.bounds.height;
      return bScore - aScore;
    });
    for (const w of ranked) {
      if (w.windowId > 0 && windowMatchesTitle(w, titleSubstr)) {
        return { windowId: w.windowId, method: "quartz" };
      }
    }
  }

  // Fallback: AXIdentifier
  try {
    const script = `
tell application "System Events"
    set appProcs to every process whose name contains "Script Kit"
  if (count of appProcs) > 0 then
    set p to item 1 of appProcs
    set wList to every window of p
    repeat with w in wList
      set wName to name of w
      try
        set wid to value of attribute "AXIdentifier" of w
        return (wid as text) & "|||" & wName
      on error
        return ""
      end try
    end repeat
  end if
  return ""
end tell`;
    const wid = await runOsascript(script);
    if (wid) {
      const [idText, name = ""] = wid.split("|||");
      const parsed = parseInt(idText, 10);
      const lowerTitle = titleSubstr.toLowerCase();
      const matchesTitle =
        lowerTitle.length === 0 || name.toLowerCase().includes(lowerTitle);
      if (matchesTitle && parsed > 0) {
        return { windowId: parsed, method: "ax" };
      }
    }
  } catch {
    // fall through
  }

  return { windowId: 0, method: "quartz" };
}

// ---------------------------------------------------------------------------
// Quartz-based window enumeration (richer than System Events)
// ---------------------------------------------------------------------------

async function enumerateQuartzWindows(titleSubstr: string): Promise<WindowInfo[]> {
  const raw = await runSwiftWindowQuery();
  if (raw.length === 0) return [];
  return raw
    .filter((w) => windowMatchesTitle(w, titleSubstr))
    .map((w) => ({
      windowId: w.windowId,
      ownerName: w.ownerName,
      title: w.title,
      layer: w.layer,
      bounds: {
        x: w.bounds.x,
        y: w.bounds.y,
        width: w.bounds.width,
        height: w.bounds.height,
      },
      // `frontmost` is computed from panel-presence in checkFrontmost +
      // findWindows. For NonactivatingPanels the NSApp.frontmost boolean is
      // always false (the app has .accessory activation policy), so we treat
      // a visible elevated-layer panel as frontmost-for-input-purposes.
      frontmost: w.onscreen && w.layer >= PANEL_FRONTMOST_LAYER_MIN,
      focused: w.onscreen && w.layer >= PANEL_FRONTMOST_LAYER_MIN,
      visible:
        w.onscreen &&
        w.bounds.width > 0 &&
        w.bounds.height > 0 &&
        w.alpha > 0,
    }));
}

// ---------------------------------------------------------------------------
// Frontmost / focused checks
// ---------------------------------------------------------------------------

async function checkFrontmost(): Promise<{ frontmost: boolean; focused: boolean }> {
  // Script Kit GPUI runs with NSApp.setActivationPolicy(.accessory) and
  // presents its main surface as a NonactivatingPanel (WindowKind::PopUp at
  // NSFloatingWindowLevel). Under that configuration, the traditional
  // "process is frontmost" check via System Events will ALWAYS return false
  // — an accessory app cannot become NSApp.frontmost by design. For
  // agentic CGEvent delivery the question we actually care about is "is the
  // panel visible and above normal windows?", which we derive from
  // Quartz: any Script Kit window with `onscreen=true` and
  // `layer >= PANEL_FRONTMOST_LAYER_MIN`.
  const panels = await runSwiftWindowQuery();
  const panelPresent = panels.some(
    (w) => w.onscreen && w.layer >= PANEL_FRONTMOST_LAYER_MIN && w.alpha > 0
  );
  return { frontmost: panelPresent, focused: panelPresent };
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

async function findWindows(titleSubstr: string): Promise<FindResult> {
  let appRunning = false;
  try {
    const ps = await runCommand(["pgrep", "-f", "script-kit-gpui"], "pgrep");
    appRunning = ps.trim().length > 0;
  } catch {
    appRunning = false;
  }

  const windows = await enumerateQuartzWindows(titleSubstr);
  // Per-window frontmost/focused is already populated in
  // enumerateQuartzWindows from onscreen + layer, so we do NOT clobber
  // windows[0] from a bulk checkFrontmost() call. Off-screen helper
  // windows (tray items, secondary panels) must remain frontmost=false.

  // Surface the panel-visible windows first so downstream consumers (focus,
  // capture, surface classification) see the live main panel at index 0.
  windows.sort((a, b) => {
    const score = (w: WindowInfo) =>
      (w.visible ? 100 : 0) +
      (w.layer >= PANEL_FRONTMOST_LAYER_MIN ? 10 : 0) +
      (w.frontmost ? 1 : 0);
    return score(b) - score(a);
  });

  return { windows, appRunning: appRunning || windows.length > 0 };
}

async function focusWindow(
  titleSubstr: string,
  retryCount: number,
  settleMs: number
): Promise<FocusResult> {
  let lastFrontmost = false;
  let lastFocused = false;
  let windowId = 0;
  let osFrontmostProcess: string | null = null;
  let keyboardReady = false;

  for (let attempt = 1; attempt <= retryCount; attempt++) {
    stderrLog("window_focus_attempt", { attempt, retryCount, titleSubstr });

    // Early-out: the Script Kit main window is a NonactivatingPanel owned
    // by an accessory-policy app, so AppleScript-based activation
    // (`set frontmost of process`) is a no-op at best and an error at
    // worst. If a panel is already visible and above normal windows, we
    // are already "frontmost" in the CGEvent-delivery sense. Skip the
    // activation attempt entirely in that case.
    const preCheck = await checkFrontmost();
    if (preCheck.frontmost) {
      lastFrontmost = preCheck.frontmost;
      lastFocused = preCheck.focused;
      const resolved = await resolveWindowId(titleSubstr);
      windowId = resolved.windowId;
      osFrontmostProcess = await currentFrontmostProcessName();
      keyboardReady = isScriptKitFrontmostProcess(osFrontmostProcess);
      stderrLog("window_focus_panel_already_visible", {
        attempt,
        windowId,
        osFrontmostProcess,
        keyboardReady,
      });
      break;
    }

    try {
      // Activation path for the (increasingly unlikely) case where the
      // app is NOT an accessory / NOT a NonactivatingPanel. Both "Script
      // Kit" (display name) and "script-kit-gpui" (executable name) are
      // matched so future rebuilds don't regress this.
      await runOsascript(`
tell application "System Events"
  set appProcs to every process whose (name contains "Script Kit") or (name contains "script-kit-gpui")
  if (count of appProcs) > 0 then
    set frontmost of item 1 of appProcs to true
  else
    error "Script Kit process not found"
  end if
end tell`);
    } catch (e: any) {
      if (e.code === "ACCESSIBILITY_DENIED") throw e;
      stderrLog("window_focus_activate_failed", { attempt, error: e.message });
      if (attempt < retryCount) {
        await Bun.sleep(settleMs);
        continue;
      }
      throw Object.assign(new Error(`Failed to activate after ${retryCount} attempts: ${e.message}`), {
        code: "ACTIVATION_FAILED",
      });
    }

    // Settle delay
    await Bun.sleep(settleMs);

    // Check state after settle
    const state = await checkFrontmost();
    lastFrontmost = state.frontmost;
    lastFocused = state.focused;
    osFrontmostProcess = await currentFrontmostProcessName();
    keyboardReady = isScriptKitFrontmostProcess(osFrontmostProcess);

    // Resolve window ID
    const resolved = await resolveWindowId(titleSubstr);
    windowId = resolved.windowId;

    stderrLog("window_focus_result", {
      attempt,
      frontmost: lastFrontmost,
      focused: lastFocused,
      windowId,
      osFrontmostProcess,
      keyboardReady,
    });

    if (lastFrontmost) break;
  }

  return {
    activated: true,
    frontmost: lastFrontmost,
    focused: lastFocused,
    keyboardReady,
    osFrontmostProcess,
    attempts: retryCount,
    settleMs,
    windowId,
  };
}

/**
 * Classify a Script Kit window into a stable automation surface identity.
 * Uses title heuristics to assign surface IDs that agents can target.
 */
function classifyWindow(w: WindowInfo): AutomationSurface {
  const titleLower = (w.title ?? "").toLowerCase();
  const bounds = w.bounds;
  const isFooterOverlay =
    titleLower.includes("footer overlay") ||
    (bounds !== null && bounds.height <= 80 && bounds.width >= 300);
  let surfaceId = "main";
  let kind = "popup";

  if (isFooterOverlay) {
    surfaceId = "footer";
    kind = "popup";
  } else if (titleLower.includes("agent_chat") || titleLower.includes("chat")) {
    surfaceId = "agent_chat";
    kind = "panel";
  } else if (titleLower.includes("actions") || titleLower.includes("⌘k")) {
    surfaceId = "actions";
    kind = "popup";
  } else if (titleLower.includes("notes")) {
    surfaceId = "notes";
    kind = "panel";
  } else if (titleLower.includes("ai")) {
    surfaceId = "ai";
    kind = "panel";
  } else if (w.layer > 100) {
    // PopUp windows at elevated levels are likely main or detached surfaces
    surfaceId = "main";
    kind = "popup";
  }

  return {
    surfaceId,
    kind,
    windowId: w.windowId,
    title: w.title,
    frontmost: w.frontmost,
    focused: w.focused,
    visible: w.visible,
    bounds: w.bounds,
  };
}

async function listSurfaces(titleSubstr: string): Promise<ListResult> {
  const findResult = await findWindows(titleSubstr);
  const surfaces = findResult.windows
    .map(classifyWindow)
    .sort((a, b) => {
      const score = (s: AutomationSurface) => {
        let value = 0;
        if (s.surfaceId === "main") value += 1000;
        if (s.surfaceId === "footer") value -= 1000;
        if (s.visible) value += 100;
        if (s.frontmost) value += 10;
        if (s.focused) value += 1;
        value += s.bounds?.height ?? 0;
        return value;
      };
      return score(b) - score(a);
    });

  // Deduplicate by surfaceId — keep the first (frontmost) window per ID
  const seen = new Set<string>();
  const deduped: AutomationSurface[] = [];
  for (const s of surfaces) {
    // If two windows have the same surfaceId, disambiguate with index
    const key = seen.has(s.surfaceId) ? `${s.surfaceId}:${s.windowId}` : s.surfaceId;
    if (seen.has(s.surfaceId)) {
      s.surfaceId = key;
    }
    seen.add(s.surfaceId);
    deduped.push(s);
  }

  const focusedSurface =
    deduped.find((s) => s.surfaceId === "main" && (s.focused || s.frontmost)) ??
    deduped.find((s) => s.focused || s.frontmost);
  return {
    surfaces: deduped,
    appRunning: findResult.appRunning,
    focusedSurfaceId: focusedSurface?.surfaceId ?? null,
  };
}

async function getStatus(): Promise<StatusResult> {
  const findResult = await findWindows("");
  const osFrontmostProcess = await currentFrontmostProcessName();
  return {
    appRunning: findResult.appRunning,
    frontmost: findResult.windows.some((w) => w.frontmost),
    keyboardReady: isScriptKitFrontmostProcess(osFrontmostProcess),
    osFrontmostProcess,
    visibleWindows: findResult.windows.filter((w) => w.visible).length,
    windows: findResult.windows,
  };
}

async function captureWindow(
  path: string,
  titleSubstr: string,
  retryCount: number,
  activateFirst: boolean,
  settleMs: number,
  exactWindowId?: number
): Promise<CaptureResult> {
  let windowId = exactWindowId ?? 0;
  let captureMethod: "quartz" | "screencapture" = "screencapture";
  let attempts = 0;
  let lastFrontmost = false;
  let lastFocused = false;

  for (let attempt = 1; attempt <= retryCount; attempt++) {
    attempts = attempt;
    stderrLog("window_capture_attempt", { attempt, retryCount, path, titleSubstr, activateFirst, exactWindowId: exactWindowId ?? null });

    // Optionally focus first
    if (activateFirst || attempt > 1) {
      const focusResult = await focusWindow(titleSubstr, 1, settleMs);
      lastFrontmost = focusResult.frontmost;
      lastFocused = focusResult.focused;
      // Prefer exact window ID if provided; fall back to focus-resolved ID
      if (!exactWindowId) {
        windowId = focusResult.windowId;
      }
    } else {
      // Just resolve ID without activating
      if (!exactWindowId) {
        const resolved = await resolveWindowId(titleSubstr);
        windowId = resolved.windowId;
      }
      const state = await checkFrontmost();
      lastFrontmost = state.frontmost;
      lastFocused = state.focused;
    }

    // Try screencapture with window ID
    if (windowId > 0) {
      const command = ["screencapture", "-l", String(windowId), path];
      assertNoninteractiveSubprocess(command);
      const proc = Bun.spawn(command, {
        stdout: "pipe",
        stderr: "pipe",
      });
      const stderr = await new Response(proc.stderr).text();
      const code = await proc.exited;

      if (code === 0 && existsSync(path)) {
        captureMethod = "quartz";
        stderrLog("window_capture_success", { attempt, windowId, method: "quartz", path });
        break;
      }
      stderrLog("window_capture_screencapture_l_failed", { attempt, windowId, exitCode: code, stderr: stderr.trim() });
    }

    if (exactWindowId && exactWindowId > 0) {
      if (attempt === retryCount) {
        throw Object.assign(
          new Error(
            `Targeted window capture failed for exact window ID ${exactWindowId} after ${retryCount} attempts`
          ),
          { code: "CAPTURE_FAILED" }
        );
      }
      await Bun.sleep(settleMs);
      continue;
    }

    // Fallback: full-screen capture (non-interactive)
    if (attempt === retryCount) {
      const command = ["screencapture", "-o", "-x", path];
      assertNoninteractiveSubprocess(command);
      const proc2 = Bun.spawn(command, {
        stdout: "pipe",
        stderr: "pipe",
      });
      const stderr2 = await new Response(proc2.stderr).text();
      const code2 = await proc2.exited;
      if (code2 === 0 && existsSync(path)) {
        captureMethod = "screencapture";
        stderrLog("window_capture_fallback_success", { attempt, method: "screencapture", path });
        break;
      }
      throw Object.assign(
        new Error(`screencapture failed after ${retryCount} attempts: ${stderr2.trim()}`),
        { code: "CAPTURE_FAILED" }
      );
    }

    // Settle before retry
    await Bun.sleep(settleMs);
  }

  // Verify file
  if (!existsSync(path)) {
    throw Object.assign(new Error(`Screenshot file not created at ${path}`), {
      code: "CAPTURE_FAILED",
    });
  }

  const stats = statSync(path);
  let width: number | null = null;
  let height: number | null = null;
  try {
    const sipsOut = await runCommand(
      ["sips", "-g", "pixelWidth", "-g", "pixelHeight", path],
      "sips"
    );
    const wMatch = sipsOut.match(/pixelWidth:\s*(\d+)/);
    const hMatch = sipsOut.match(/pixelHeight:\s*(\d+)/);
    if (wMatch) width = parseInt(wMatch[1], 10);
    if (hMatch) height = parseInt(hMatch[1], 10);
  } catch {
    // dimensions unknown
  }

  // Do not return windowId: 0 for a claimed successful capture
  // If we used the fallback method, windowId may be 0 — that's still valid
  // because the fallback captures the full screen, not a specific window.
  // But we report it honestly.

  return {
    path,
    windowId,
    attempts,
    method: captureMethod,
    frontmost: lastFrontmost,
    focused: lastFocused,
    sizeBytes: stats.size,
    width,
    height,
  };
}

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

function parseIntArg(args: string[], flag: string, fallback: number): number {
  const idx = args.indexOf(flag);
  if (idx >= 0 && args[idx + 1]) {
    const v = parseInt(args[idx + 1], 10);
    return isNaN(v) ? fallback : v;
  }
  return fallback;
}

function hasFlag(args: string[], flag: string): boolean {
  return args.includes(flag);
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const args = process.argv.slice(2);
const command = args[0] ?? "status";
const titleIdx = args.indexOf("--title");
const titleSubstr =
  titleIdx >= 0 && args[titleIdx + 1] ? args[titleIdx + 1] : DEFAULT_TITLE_SUBSTR;

function emit(data: Envelope<any>) {
  console.log(JSON.stringify(data, null, 2));
}

try {
  if (
    process.env.SCRIPT_KIT_NONINTERACTIVE === "1" &&
    command !== "help" &&
    command !== "--help"
  ) {
    throw new NoninteractiveSafetyError(
      `window.${command}`,
      "native window focus, AppleScript, Swift discovery, and screen capture are forbidden during noninteractive verification",
    );
  }
  switch (command) {
    case "list": {
      const result = await listSurfaces(titleSubstr);
      emit(envelope("list", result));
      process.exit(result.surfaces.length > 0 ? 0 : 1);
      break;
    }

    case "find": {
      const result = await findWindows(titleSubstr);
      emit(envelope("find", result));
      process.exit(result.windows.length > 0 ? 0 : 1);
      break;
    }

    case "focus": {
      const retry = parseIntArg(args, "--retry", DEFAULT_RETRY);
      const settleMs = parseIntArg(args, "--settle-ms", DEFAULT_SETTLE_MS);
      const result = await focusWindow(titleSubstr, retry, settleMs);
      emit(envelope("focus", result));
      process.exit(result.frontmost ? 0 : 1);
      break;
    }

    case "status": {
      const result = await getStatus();
      emit(envelope("status", result));
      process.exit(result.appRunning ? 0 : 1);
      break;
    }

    case "capture": {
      const capturePath = args[1];
      if (!capturePath || capturePath.startsWith("--")) {
        emit(
          errorEnvelope(
            "capture",
            "MISSING_PATH",
            "Usage: window.ts capture <path> [--title SUBSTR] [--retry N] [--activate-first] [--settle-ms MS] [--window-id N]"
          )
        );
        process.exit(1);
        break;
      }
      const retry = parseIntArg(args, "--retry", DEFAULT_CAPTURE_RETRY);
      const activateFirst = hasFlag(args, "--activate-first");
      const settleMs = parseIntArg(args, "--settle-ms", DEFAULT_CAPTURE_SETTLE_MS);
      const exactWindowId = parseIntArg(args, "--window-id", 0);
      const result = await captureWindow(capturePath, titleSubstr, retry, activateFirst, settleMs, exactWindowId > 0 ? exactWindowId : undefined);
      emit(envelope("capture", result));
      process.exit(0);
      break;
    }

    case "help":
    case "--help": {
      console.log(`Usage: bun scripts/agentic/window.ts <command> [options]

Commands:
  list    [--title SUBSTR]                                 List automation surfaces with stable IDs
  find    [--title SUBSTR]                                 Find Script Kit windows (raw Quartz)
  focus   [--title SUBSTR] [--retry N] [--settle-ms MS]    Activate and focus window
  status                                                    App running + window status
  capture PATH [--title SUBSTR] [--retry N]                Focus + screencapture window
          [--activate-first] [--settle-ms MS]

Options:
  --title SUBSTR       Filter windows by title substring
  --retry N            Number of focus/capture attempts (default: 1 for focus, 1 for capture)
  --settle-ms MS       Delay after activation before checking state (default: 300 focus, 200 capture)
  --activate-first     Activate window before capture attempt (default: only on retry)
  --json               (accepted for compatibility, output is always JSON)

Output:
  Schema version ${SCHEMA_VERSION} JSON envelopes on stdout.
  Structured NDJSON diagnostics on stderr.
  Exit 0 = success, 1 = not found or failed.

Window ID resolution:
  Uses Quartz CGWindowListCopyWindowInfo (preferred) with AXIdentifier fallback.
  screencapture -l <windowId> for targeted capture; full-screen fallback if ID unavailable.

Permission requirements:
  - System Events access (Automation permission in Privacy & Security)
  - Screen Recording permission for 'capture' command

Remediation:
  System Preferences → Privacy & Security → Automation → allow Terminal/IDE
  System Preferences → Privacy & Security → Screen Recording → allow Terminal/IDE`);
      process.exit(0);
      break;
    }

    default:
      emit(
        errorEnvelope("unknown", "UNKNOWN_COMMAND", `Unknown command: ${command}`)
      );
      process.exit(1);
  }
} catch (e: any) {
  const code = e instanceof NoninteractiveSafetyError
    ? "NONINTERACTIVE_SAFETY_REFUSED"
    : e.code ?? "UNKNOWN_ERROR";
  let message = e.message ?? String(e);

  if (code === "ACCESSIBILITY_DENIED") {
    message +=
      "\n\nRemediation: System Preferences → Privacy & Security → Accessibility → enable your terminal/IDE app.";
  }

  emit(errorEnvelope(command, code, message));
  process.exit(1);
}
