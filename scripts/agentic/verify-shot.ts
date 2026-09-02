#!/usr/bin/env bun
/**
 * scripts/agentic/verify-shot.ts
 *
 * Agent Chat proof bundle: state receipt + test probe + screenshot + vision prompts.
 *
 * The reliable Agent Chat verification order is:
 *   1. State receipt (getAgentChatState) — machine-readable proof
 *   2. Probe receipt (getAgentChatTestProbe) — key-route / picker-acceptance telemetry
 *   3. Screenshot capture — visual proof with pixel-content audit
 *   4. Vision checks — structured prompts for external image readers
 *
 * Usage:
 *   bun scripts/agentic/verify-shot.ts --session NAME [options]
 *
 * Options:
 *   --session NAME              Session name (default: "default")
 *   --label LABEL               Human-readable step label (default: "verify")
 *   --out PATH                  Screenshot output path (default: .test-screenshots/<label>-<ts>.png)
 *   --agent_chat-status STATUS         Assert Agent Chat status equals this value
 *   --agent_chat-picker-open           Assert picker is open
 *   --agent_chat-picker-closed         Assert picker is closed
 *   --agent_chat-input-contains STR    Assert input text contains substring
 *   --agent_chat-input-match STR       Assert input text equals exactly
 *   --agent_chat-cursor-at N           Assert cursor is at character index N
 *   --agent_chat-item-accepted         Assert lastAcceptedItem is non-null
 *   --agent_chat-accepted-label STR    Assert lastAcceptedItem.label equals STR
 *   --agent_chat-accepted-trigger STR  Assert lastAcceptedItem.trigger equals STR (@ or /)
 *   --agent_chat-accepted-via KEY      Assert probe acceptedItems[last].acceptedViaKey equals KEY (enter|tab)
 *   --agent_chat-cursor-after-accepted N  Assert probe acceptedItems[last].cursorAfter equals N
 *   --agent_chat-context-ready         Assert contextReady is true
 *   --agent_chat-no-selection          Assert hasSelection is false
 *   --agent_chat-has-selection         Assert hasSelection is true
 *   --agent_chat-no-permission         Assert hasPendingPermission is false
 *   --agent_chat-has-permission        Assert hasPendingPermission is true
 *   --agent_chat-visible-start N       Assert inputLayout.visibleStart equals N
 *   --agent_chat-visible-end N         Assert inputLayout.visibleEnd equals N
 *   --agent_chat-cursor-in-window N    Assert inputLayout.cursorInWindow equals N
 *   --agent_chat-setup-visible         Assert setup card is present (status == "setup")
 *   --agent_chat-setup-reason CODE     Assert setup.reasonCode equals CODE
 *   --agent_chat-setup-primary-action A  Assert setup.primaryAction equals A
 *   --agent_chat-setup-selected-agent ID Assert setup.selectedAgentId equals ID
 *   --agent_chat-setup-agent-picker-open Assert setup.agentPickerOpen is true
 *   --probe-tail N              Number of probe events to request (default: 20)
 *   --vision                    Emit vision checks with mustReview prompts and requiresVisionReview
 *   --emit-vision-crops         Alias for --vision
 *   --skip-screenshot           Only run state assertions, skip capture
 *   --skip-state                Only capture screenshot, skip Agent Chat state query
 *   --skip-probe                Skip Agent Chat test probe query
 *   --strict-window             Require exact target/window identity for non-Agent Chat captures
 *   --visual-source SOURCE      os|render|auto (default: os)
 *   --target-json JSON           Agent Chat window target for getAgentChatState/getAgentChatTestProbe RPCs
 *                               (same AutomationWindowTarget shape as the Rust protocol)
 *   --capture-window-id N        Exact native window ID for screencapture
 *                               (the inspected osWindowId, not automationWindowId)
 *   --request-id ID             Request ID for getAgentChatState (default: auto-generated)
 *   --json                      (default) Output JSON receipt
 *   --help                      Show this help
 *
 * Exit codes:
 *   0 = all assertions passed
 *   1 = one or more assertions failed
 *   2 = infrastructure error (session dead, capture failed, etc.)
 */

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "fs";
import { join, resolve } from "path";
import { auditRgbaPng } from "../devtools/lib/png-rgba.ts";
import {
  assertNoninteractiveSubprocess,
  NoninteractiveSafetyError,
} from "../devtools/lib/operator-safety.ts";

const SCHEMA_VERSION = 3;
const PROJECT_ROOT = resolve(import.meta.dir, "../..");

function guardedSpawn(command: string[]) {
  assertNoninteractiveSubprocess(command);
  return Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CaptureTarget {
  requestedWindowId: number | null;
  actualWindowId: number | null;
}

type CaptureRouting =
  | "cli-window-id"
  | "inspection-os-window-id"
  | "native-window-mcp"
  | "screen-rect-from-inspection"
  | "runtime-capture-window"
  | "generic-frontmost";

interface WindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

type ScreenRectSource =
  | "computer/get_native_window.bounds"
  | "computer/list_native_windows.bounds"
  | "automationInspect.resolvedBounds";

interface ScreenRectFallbackReceipt {
  attempted: boolean;
  used: boolean;
  source: ScreenRectSource | null;
  bounds: WindowBounds | null;
  nativeWindowId: number | null;
  automationResolvedBounds: WindowBounds | null;
  error: string | null;
}

interface InspectionReceipt {
  automationWindowId: string;
  windowKind: string;
  title?: string | null;
  osWindowId?: number | null;
  targetBoundsInScreenshot?: WindowBounds | null;
  resolvedBounds?: WindowBounds | null;
  screenshotWidth?: number | null;
  screenshotHeight?: number | null;
  pid?: number | null;
  pixelProbes: Array<{ x: number; y: number; r: number; g: number; b: number; a: number }>;
  warnings: string[];
}

interface CapturePlan {
  routing: CaptureRouting;
  requestedAutomationWindowId: string | null;
  requestedOsWindowId: number | null;
  inspectionOsWindowId: number | null;
  showIssuedBeforeCapture: boolean;
  screenRectFallback?: ScreenRectFallbackReceipt | null;
}

/**
 * Deterministic popup capture strategy receipt.
 *
 * For attached popups (ActionsDialog, PromptPopup), the screenshot captures
 * the parent (main) window. The `targetBounds` field specifies the exact
 * crop region within the parent screenshot where the popup lives.
 *
 * For detached windows (AgentChatDetached, Notes), the screenshot is the window
 * itself at origin (0, 0).
 */
interface PopupCaptureReceipt {
  /** The capture strategy used. */
  strategy: "parent_capture_with_crop" | "direct_window_capture" | "not_applicable";
  /** The automation window kind that determined the strategy. */
  windowKind: string | null;
  /** Crop bounds within the screenshot (null for detached or when geometry unavailable). */
  targetBounds: { x: number; y: number; width: number; height: number } | null;
  /** Whether semantic receipts (not screenshots) are the primary verification oracle. */
  semanticReceiptsArePrimary: boolean;
}

interface VerifyReceipt {
  schemaVersion: number;
  status: "pass" | "fail" | "error";
  label: string;
  timestamp: string;
  durationMs: number;
  requiresVisionReview: boolean;
  // Exact target identity — populated from inspection when --target-json is used
  resolvedTarget: {
    windowId: string;
    windowKind: string;
    title?: string | null;
    surfaceId?: string | null;
  } | null;
  /** Routed input method used during the flow that produced this proof bundle. */
  inputMethod?: "batch" | "simulateGpuiEvent" | "native";
  /** Dispatch path for routed input: exact_handle when target was an ID, window_role_fallback otherwise. */
  dispatchPath?: "exact_handle" | "window_role_fallback";
  /** Resolved window ID from the automation target (same as resolvedTarget.windowId). */
  resolvedWindowId?: string;
  /** OS-level window ID (CGWindowID) when available from inspection. */
  osWindowId?: number | null;
  inspectionOsWindowId?: number | null;
  captureRouting?: CaptureRouting;
  requestedAutomationWindowId?: string | null;
  requestedOsWindowId?: number | null;
  showIssuedBeforeCapture?: boolean;
  // Stable proof bundle fields (canonical names for machine consumption)
  state: Record<string, unknown> | null;
  probe: Record<string, unknown> | null;
  screenshot: {
    path: string | null;
    captureMethod: string | null;
    windowCaptureMethod: string | null;
    windowId: number | null;
  } | null;
  captureTarget: CaptureTarget | null;
  inspection: InspectionReceipt | null;
  /** Deterministic popup capture strategy receipt. */
  popupCapture: PopupCaptureReceipt | null;
  /** Machine-readable visual evidence classification for OS/window capture attempts. */
  visualEvidence: VisualEvidenceReceipt | null;
  /** Machine-readable visual evidence classification for GPUI app-render readback attempts. */
  renderEvidence: VisualEvidenceReceipt | null;
  visionCrops: VisionCheck[];
  // Detailed receipts (full diagnostics)
  stateReceipt: AgentChatStateResult | null;
  probeReceipt: ProbeResult | null;
  screenshotReceipt: ScreenshotResult | null;
  visionChecks: VisionCheck[];
  assertions: AssertionResult[];
  summary: string;
}

interface AgentChatStateResult {
  queried: boolean;
  snapshot: Record<string, unknown> | null;
  error: string | null;
}

interface ProbeResult {
  queried: boolean;
  snapshot: Record<string, unknown> | null;
  error: string | null;
}

interface ScreenshotResult {
  captured: boolean;
  path: string | null;
  sizeBytes: number | null;
  width: number | null;
  height: number | null;
  contentAudit: ScreenshotContentAudit | null;
  captureMethod:
    | "computer/capture_native_window"
    | "window.ts"
    | "captureWindow"
    | null;
  windowCaptureMethod: "native-xcap" | "quartz" | "screencapture" | null;
  windowFrontmost: boolean | null;
  windowFocused: boolean | null;
  windowId: number | null;
  error: string | null;
}

type VisualSource =
  | "os-window-capture"
  | "gpui-render-readback"
  | "gpui-offscreen-render";

type VisualCaptureClassification =
  | "captured"
  | "target-identity-blocked"
  | "target-not-capture-candidate"
  | "macos-windowserver-capture-blocked"
  | "blank-image-rejected"
  | "gpui-readback-unavailable"
  | "unknown-capture-failure";

type OsCaptureKind = "window-id" | "screen-rect" | "fullscreen-crop" | null;
type OsCaptureBlockerCode =
  | "none"
  | "target-identity-blocked"
  | "target-not-capture-candidate"
  | "window-id-api-blocked"
  | "screen-rect-capture-blocked"
  | "screen-recording-permission-blocked"
  | "blank-compositor-capture"
  | "occlusion-or-space-blocked"
  | "pixel-audit-failed"
  | "unknown-capture-failure";

interface CaptureAttemptReceipt {
  source:
    | "computer/capture_native_window"
    | "screencapture-window-id"
    | "screencapture-screen-rect"
    | "runtime-capture-window"
    | "gpui-render-readback";
  status: "captured" | "failed" | "blankRejected" | "skipped" | "unsupported";
  requestedWindowId: number | null;
  actualWindowId: number | null;
  errorCode: string | null;
  message: string | null;
  pixelAudit?: ScreenshotContentAudit | null;
}

interface VisualEvidenceReceipt {
  source: VisualSource;
  available: boolean;
  countsAsOsScreenshotEvidence: boolean;
  countsAsCompositorEvidence?: boolean;
  countsAsAppRenderEvidence: boolean;
  countsAsOffscreenRenderEvidence: boolean;
  classification: VisualCaptureClassification;
  captureKind?: OsCaptureKind;
  blockerCode?: OsCaptureBlockerCode;
  windowIdCaptureProof?: "pass" | "blocked" | "missing";
  compositorCaptureProof?: "pass" | "blocked" | "missing";
  attempts: CaptureAttemptReceipt[];
  path?: string | null;
  sha256?: string | null;
  width?: number | null;
  height?: number | null;
  pixelAudit?: ScreenshotContentAudit | Record<string, unknown> | null;
  target?: Record<string, unknown> | null;
  captureIdentity?: Record<string, unknown> | null;
  assertions?: Record<string, boolean>;
  limitation: string | null;
}

interface ScreenshotContentAudit {
  sampledPixels: number;
  nonBlackPixels: number;
  nonTransparentPixels: number;
  uniqueBucketCount: number;
  meanLuma: number;
  maxLuma: number;
  nonBlackRatio: number;
  blank: boolean;
}

interface VisionCheck {
  name: string;
  path: string;
  question: string;
  crop: { x: number; y: number; width: number; height: number } | null;
  expectedAnswer?: string | null;
  mustReview: boolean;
  failureMessage: string;
}

interface AssertionResult {
  name: string;
  expected: string;
  actual: string;
  passed: boolean;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function parseArgs() {
  const args = process.argv.slice(2);
  const opts: Record<string, string | boolean> = {};

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--help" || arg === "-h") {
      opts.help = true;
    } else if (arg === "--skip-screenshot") {
      opts.skipScreenshot = true;
    } else if (arg === "--skip-state") {
      opts.skipState = true;
    } else if (arg === "--agent_chat-picker-open") {
      opts.agent_chatPickerOpen = true;
    } else if (arg === "--agent_chat-picker-closed") {
      opts.agent_chatPickerClosed = true;
    } else if (arg === "--agent_chat-item-accepted") {
      opts.agent_chatItemAccepted = true;
    } else if (arg === "--agent_chat-context-ready") {
      opts.agent_chatContextReady = true;
    } else if (arg === "--agent_chat-no-selection") {
      opts.agent_chatNoSelection = true;
    } else if (arg === "--agent_chat-has-selection") {
      opts.agent_chatHasSelection = true;
    } else if (arg === "--agent_chat-no-permission") {
      opts.agent_chatNoPermission = true;
    } else if (arg === "--agent_chat-has-permission") {
      opts.agent_chatHasPermission = true;
    } else if (arg === "--agent_chat-setup-visible") {
      opts.agent_chatSetupVisible = true;
    } else if (arg === "--agent_chat-setup-agent-picker-open") {
      opts.agent_chatSetupAgentPickerOpen = true;
    } else if (arg === "--emit-vision-crops" || arg === "--vision") {
      opts.emitVisionCrops = true;
    } else if (arg === "--skip-probe") {
      opts.skipProbe = true;
    } else if (arg === "--strict-window") {
      opts.strictWindow = true;
    } else if (arg === "--json") {
      // default, no-op
    } else if (arg.startsWith("--") && i + 1 < args.length) {
      const key = arg.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      opts[key] = args[++i];
    }
  }

  return opts;
}

function hasOpt(
  opts: Record<string, string | boolean>,
  key: string
): boolean {
  return Object.prototype.hasOwnProperty.call(opts, key);
}

async function sendSessionCommand(
  session: string,
  cmd: string
): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const sessionScript = join(PROJECT_ROOT, "scripts/agentic/session.sh");
  const proc = guardedSpawn(["bash", sessionScript, "send", session, cmd]);
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;
  return { ok: code === 0, stdout: stdout.trim(), stderr: stderr.trim() };
}

function readMcpServer(): { endpoint: string; token: string } | null {
  const serverPath = `${process.env.HOME}/.scriptkit/server.json`;
  if (!existsSync(serverPath)) return null;

  try {
    const parsed = JSON.parse(readFileSync(serverPath, "utf8"));
    const token =
      parsed.token ??
      readFileSync(`${process.env.HOME}/.scriptkit/agent-token`, "utf8").trim();
    const baseUrl = String(
      parsed.url ?? `http://127.0.0.1:${parsed.port ?? 43210}`
    ).replace("http://localhost:", "http://127.0.0.1:");
    return {
      endpoint: baseUrl.endsWith("/rpc") ? baseUrl : `${baseUrl.replace(/\/$/, "")}/rpc`,
      token,
    };
  } catch {
    return null;
  }
}

async function callMcpTool(
  name: string,
  args: Record<string, unknown>
): Promise<Record<string, any>> {
  if (process.env.SCRIPT_KIT_NONINTERACTIVE === "1") {
    throw new NoninteractiveSafetyError(
      `mcp.${name}`,
      "native-window and render capture may not contact MCP during noninteractive verification",
    );
  }
  const server = readMcpServer();
  if (!server) {
    throw new Error("MCP discovery file is unavailable");
  }

  const response = await fetch(server.endpoint, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${server.token}`,
    },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: `verify-shot-native-capture-${Date.now()}-${Math.random()}`,
      method: "tools/call",
      params: { name, arguments: args },
    }),
  });

  const json = await response.json();
  if (json.error) {
    throw new Error(JSON.stringify(json.error));
  }
  const text = json.result?.content?.find(
    (item: Record<string, unknown>) => item.type === "text"
  )?.text;
  if (typeof text !== "string" || text.length === 0) {
    throw new Error(`MCP tool ${name} returned no text content`);
  }
  return JSON.parse(text);
}

async function captureNativeWindowViaMcp(
  nativeWindowId: number,
  outPath: string
): Promise<{
  windowId: number;
  frontmost: boolean | null;
  focused: boolean | null;
  sizeBytes: number;
} | null> {
  const list = await callMcpTool("computer/list_native_windows", {
    includeHidden: true,
    includeBackground: true,
  });

  const rows: Array<{ app: Record<string, any>; window: Record<string, any> }> =
    [];
  for (const appEntry of list.apps ?? []) {
    const app = appEntry.app ?? appEntry;
    for (const window of appEntry.windows ?? []) {
      if (window.nativeWindowId === nativeWindowId) {
        rows.push({ app, window });
      }
    }
  }

  const selected =
    rows.find(
      ({ window }) =>
        window.observation?.captureSelectionCandidate?.status === "candidate"
    ) ?? rows[0];
  if (!selected) {
    throw new Error(
      `Native window ${nativeWindowId} was not listed by computer/list_native_windows`
    );
  }

  const capture = await callMcpTool("computer/capture_native_window", {
    pid: selected.app.pid,
    nativeWindowId,
    expectedBundleId: selected.app.bundleId,
    includeImage: true,
    hiDpi: false,
  });
  if (capture.status !== "captured" || !capture.capture?.pngBase64) {
    throw new Error(
      `computer/capture_native_window did not return image data: status=${capture.status ?? "unknown"} error=${capture.error?.code ?? "none"}`
    );
  }

  const decoded = Buffer.from(capture.capture.pngBase64, "base64");
  writeFileSync(outPath, decoded);
  return {
    windowId: nativeWindowId,
    frontmost: selected.window.observation?.frontmost ?? null,
    focused: selected.window.observation?.focused ?? null,
    sizeBytes: decoded.byteLength,
  };
}

async function resolveNativeWindowIdFromInspection(
  inspection: InspectionReceipt | null
): Promise<number | null> {
  if (
    !inspection ||
    typeof inspection.pid !== "number" ||
    inspection.pid <= 0 ||
    !inspection.resolvedBounds
  ) {
    return null;
  }

  const list = await callMcpTool("computer/list_native_windows", {
    includeHidden: true,
    includeBackground: true,
  });
  const expectedWidth = Math.round(inspection.resolvedBounds.width);
  const expectedHeight = Math.round(inspection.resolvedBounds.height);
  const matches: Array<Record<string, any>> = [];

  for (const appEntry of list.apps ?? []) {
    const app = appEntry.app ?? appEntry;
    if (app.pid !== inspection.pid) {
      continue;
    }
    for (const window of appEntry.windows ?? []) {
      const bounds = window.bounds ?? {};
      const width = Math.round(Number(bounds.width));
      const height = Math.round(Number(bounds.height));
      if (width === expectedWidth && height === expectedHeight) {
        matches.push(window);
      }
    }
  }

  const selected =
    matches.find(
      (window) =>
        window.observation?.captureSelectionCandidate?.status === "candidate"
    ) ?? matches[0];

  return typeof selected?.nativeWindowId === "number"
    ? selected.nativeWindowId
    : null;
}

function inspectionScreenRect(inspection: InspectionReceipt | null): WindowBounds | null {
  const bounds = inspection?.resolvedBounds;
  if (
    !bounds ||
    !Number.isFinite(bounds.x) ||
    !Number.isFinite(bounds.y) ||
    !Number.isFinite(bounds.width) ||
    !Number.isFinite(bounds.height) ||
    bounds.width <= 0 ||
    bounds.height <= 0
  ) {
    return null;
  }

  return {
    x: Math.floor(bounds.x),
    y: Math.floor(bounds.y),
    width: Math.max(1, Math.round(bounds.width)),
    height: Math.max(1, Math.round(bounds.height)),
  };
}

function toWindowBounds(value: unknown): WindowBounds | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const bounds = value as Record<string, unknown>;
  const x = Number(bounds.x);
  const y = Number(bounds.y);
  const width = Number(bounds.width);
  const height = Number(bounds.height);
  if (
    !Number.isFinite(x) ||
    !Number.isFinite(y) ||
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width <= 0 ||
    height <= 0
  ) {
    return null;
  }
  return {
    x: Math.floor(x),
    y: Math.floor(y),
    width: Math.max(1, Math.round(width)),
    height: Math.max(1, Math.round(height)),
  };
}

async function nativeScreenRectFromWindowId(
  nativeWindowId: number,
  expectedPid: number | null
): Promise<{ bounds: WindowBounds; source: ScreenRectSource } | null> {
  const fromGetNative = await callMcpTool("computer/get_native_window", {
    nativeWindowId,
  });
  const app = asRecord(fromGetNative.app);
  if (
    expectedPid != null &&
    typeof app?.pid === "number" &&
    app.pid !== expectedPid
  ) {
    throw new Error(
      `native window ${nativeWindowId} pid mismatch: expected ${expectedPid}, got ${app.pid}`
    );
  }
  const getNativeBounds = toWindowBounds(asRecord(fromGetNative.window)?.bounds);
  if (getNativeBounds) {
    return {
      source: "computer/get_native_window.bounds",
      bounds: getNativeBounds,
    };
  }

  const list = await callMcpTool("computer/list_native_windows", {
    includeHidden: true,
    includeBackground: true,
  });
  for (const appEntry of list.apps ?? []) {
    const listedApp = appEntry.app ?? appEntry;
    if (
      expectedPid != null &&
      typeof listedApp?.pid === "number" &&
      listedApp.pid !== expectedPid
    ) {
      continue;
    }
    for (const window of appEntry.windows ?? []) {
      if (window.nativeWindowId !== nativeWindowId) {
        continue;
      }
      const bounds = toWindowBounds(window.bounds);
      if (bounds) {
        return {
          source: "computer/list_native_windows.bounds",
          bounds,
        };
      }
    }
  }

  return null;
}

function newScreenRectFallbackReceipt(
  inspection: InspectionReceipt | null,
  nativeWindowId: number | null
): ScreenRectFallbackReceipt {
  return {
    attempted: false,
    used: false,
    source: null,
    bounds: null,
    nativeWindowId,
    automationResolvedBounds: inspectionScreenRect(inspection),
    error: null,
  };
}

function asRecord(value: unknown): Record<string, any> | null {
  return typeof value === "object" && value !== null
    ? (value as Record<string, any>)
    : null;
}

async function captureScreenRectFromInspection(
  inspection: InspectionReceipt | null,
  outPath: string,
  settleMs: number,
  nativeWindowId: number | null
): Promise<
  | { ok: true; bounds: WindowBounds; receipt: ScreenRectFallbackReceipt }
  | { ok: false; error: string; receipt: ScreenRectFallbackReceipt }
> {
  const receipt = newScreenRectFallbackReceipt(inspection, nativeWindowId);
  receipt.attempted = true;
  let bounds: WindowBounds | null = null;
  if (nativeWindowId != null && nativeWindowId > 0) {
    try {
      const nativeBounds = await nativeScreenRectFromWindowId(
        nativeWindowId,
        typeof inspection?.pid === "number" ? inspection.pid : null
      );
      if (nativeBounds) {
        bounds = nativeBounds.bounds;
        receipt.source = nativeBounds.source;
      }
    } catch (error) {
      receipt.error = error instanceof Error ? error.message : String(error);
    }
  }
  if (!bounds) {
    bounds = inspectionScreenRect(inspection);
    if (bounds) {
      receipt.source = "automationInspect.resolvedBounds";
    }
  }
  if (!bounds) {
    receipt.error =
      receipt.error ?? "inspection did not include usable resolvedBounds";
    return {
      ok: false,
      error: receipt.error,
      receipt,
    };
  }
  receipt.bounds = bounds;

  if (settleMs > 0) {
    await Bun.sleep(settleMs);
  }

  const proc = guardedSpawn([
    "screencapture",
    "-x",
    "-o",
    "-R",
    `${bounds.x},${bounds.y},${bounds.width},${bounds.height}`,
    outPath,
  ]);
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;
  if (code !== 0 || !existsSync(outPath)) {
    receipt.error = stderr.trim() || stdout.trim() || `screencapture -R exited ${code}`;
    return {
      ok: false,
      error: receipt.error,
      receipt,
    };
  }

  receipt.used = true;
  return { ok: true, bounds, receipt };
}

function classifyOsCaptureBlocker(
  screenshotResult: ScreenshotResult | null,
  capturePlan: CapturePlan | null
): VisualCaptureClassification {
  if (screenshotResult?.captured && screenshotResult.contentAudit?.blank === false) {
    return "captured";
  }
  const error = screenshotResult?.error ?? "";
  if (error.includes("Strict window capture requires")) {
    return "target-identity-blocked";
  }
  if (error.includes("blankImageRejected") || screenshotResult?.contentAudit?.blank) {
    return "blank-image-rejected";
  }
  if (
    capturePlan?.screenRectFallback?.attempted ||
    error.includes("could not create image from window") ||
    error.includes("could not create image from rect") ||
    error.includes("screenRect=")
  ) {
    return "macos-windowserver-capture-blocked";
  }
  if (error.includes("notCaptureCandidate") || error.includes("capture candidate")) {
    return "target-not-capture-candidate";
  }
  return "unknown-capture-failure";
}

function osCaptureKind(
  screenshotResult: ScreenshotResult | null,
  capturePlan: CapturePlan | null
): OsCaptureKind {
  if (!screenshotResult?.captured) return null;
  if (capturePlan?.routing === "screen-rect-from-inspection" || capturePlan?.screenRectFallback?.used) {
    return "screen-rect";
  }
  if (
    screenshotResult.captureMethod === "computer/capture_native_window" ||
    screenshotResult.windowCaptureMethod === "quartz" ||
    screenshotResult.windowCaptureMethod === "native-xcap"
  ) {
    return "window-id";
  }
  return null;
}

function osCaptureBlockerCode(
  screenshotResult: ScreenshotResult | null,
  capturePlan: CapturePlan | null,
  classification: VisualCaptureClassification
): OsCaptureBlockerCode {
  if (classification === "captured") return "none";
  const error = screenshotResult?.error ?? "";
  if (classification === "target-identity-blocked") return "target-identity-blocked";
  if (classification === "target-not-capture-candidate") return "target-not-capture-candidate";
  if (classification === "blank-image-rejected") return "blank-compositor-capture";
  if (error.includes("Screen Recording") || error.includes("screen recording")) {
    return "screen-recording-permission-blocked";
  }
  if (capturePlan?.screenRectFallback?.attempted && capturePlan.screenRectFallback.used !== true) {
    return "screen-rect-capture-blocked";
  }
  if (error.includes("could not create image from rect") || error.includes("screenRect=")) {
    return "screen-rect-capture-blocked";
  }
  if (error.includes("could not create image from window")) {
    return "window-id-api-blocked";
  }
  if (error.includes("notCaptureCandidate") || error.includes("not_capture_candidate")) {
    return "target-not-capture-candidate";
  }
  return "unknown-capture-failure";
}

function buildOsVisualEvidence(
  screenshotResult: ScreenshotResult | null,
  capturePlan: CapturePlan | null
): VisualEvidenceReceipt | null {
  if (!screenshotResult && !capturePlan) {
    return null;
  }
  const classification = classifyOsCaptureBlocker(screenshotResult, capturePlan);
  const captured = classification === "captured";
  const captureKind = osCaptureKind(screenshotResult, capturePlan);
  const blockerCode = osCaptureBlockerCode(screenshotResult, capturePlan, classification);
  const requestedWindowId = capturePlan?.requestedOsWindowId ?? null;
  const actualWindowId = screenshotResult?.windowId ?? null;
  const attempts: CaptureAttemptReceipt[] = [];
  if (requestedWindowId != null) {
    attempts.push({
      source: "computer/capture_native_window",
      status: screenshotResult?.captureMethod === "computer/capture_native_window"
        ? captured ? "captured" : "failed"
        : screenshotResult?.error?.includes("blankImageRejected")
          ? "blankRejected"
          : "failed",
      requestedWindowId,
      actualWindowId,
      errorCode: screenshotResult?.error?.includes("blankImageRejected")
        ? "blankImageRejected"
        : captured ? null : "capture_failed",
      message: screenshotResult?.error ?? null,
      pixelAudit: screenshotResult?.contentAudit ?? null,
    });
    attempts.push({
      source: "screencapture-window-id",
      status: captured && screenshotResult?.windowCaptureMethod === "quartz"
        ? "captured"
        : screenshotResult?.error?.includes("could not create image from window")
          ? "failed"
          : "skipped",
      requestedWindowId,
      actualWindowId,
      errorCode: screenshotResult?.error?.includes("could not create image from window")
        ? "screencapture_window_failed"
        : null,
      message: screenshotResult?.error ?? null,
      pixelAudit: screenshotResult?.contentAudit ?? null,
    });
  }
  if (capturePlan?.screenRectFallback?.attempted) {
    attempts.push({
      source: "screencapture-screen-rect",
      status: captured && screenshotResult?.windowCaptureMethod === "screencapture"
        ? "captured"
        : "failed",
      requestedWindowId,
      actualWindowId,
      errorCode: capturePlan.screenRectFallback.error
        ? "screencapture_rect_failed"
        : captured ? null : "capture_failed",
      message: capturePlan.screenRectFallback.error ?? screenshotResult?.error ?? null,
      pixelAudit: screenshotResult?.contentAudit ?? null,
    });
  }
  if (capturePlan?.routing === "runtime-capture-window") {
    attempts.push({
      source: "runtime-capture-window",
      status: captured ? "captured" : "failed",
      requestedWindowId,
      actualWindowId,
      errorCode: captured ? null : "capture_failed",
      message: screenshotResult?.error ?? null,
      pixelAudit: screenshotResult?.contentAudit ?? null,
    });
  }

  const windowIdCaptureFailed = attempts.some((attempt) =>
    attempt.source === "screencapture-window-id" &&
    attempt.status === "failed"
  );
  const screenRectCaptureAttempted = capturePlan?.screenRectFallback?.attempted === true;
  const screenRectCaptureFailed = attempts.some((attempt) =>
    attempt.source === "screencapture-screen-rect" &&
    attempt.status === "failed"
  );

  return {
    source: "os-window-capture",
    available: captured,
    countsAsOsScreenshotEvidence: captured,
    countsAsCompositorEvidence: captured && captureKind != null,
    countsAsAppRenderEvidence: false,
    countsAsOffscreenRenderEvidence: false,
    classification,
    captureKind,
    blockerCode,
    windowIdCaptureProof: captureKind === "window-id"
      ? "pass"
      : windowIdCaptureFailed
        ? "blocked"
        : "missing",
    compositorCaptureProof: captured
      ? "pass"
      : attempts.length > 0 || blockerCode !== "target-identity-blocked"
        ? "blocked"
        : "missing",
    attempts,
    path: screenshotResult?.path ?? null,
    width: screenshotResult?.width ?? null,
    height: screenshotResult?.height ?? null,
    pixelAudit: screenshotResult?.contentAudit ?? null,
    captureIdentity: {
      requested: {
        automationWindowId: capturePlan?.requestedAutomationWindowId ?? null,
        osWindowId: requestedWindowId,
      },
      nativeWindow: {
        nativeWindowId: actualWindowId,
        osWindowId: actualWindowId,
      },
      crop: capturePlan?.screenRectFallback?.bounds ?? null,
      identityAssertions: {
        targetJsonProvided: capturePlan?.requestedAutomationWindowId != null,
        osWindowIdMatched: requestedWindowId != null && actualWindowId === requestedWindowId,
        boundsResolved: capturePlan?.screenRectFallback?.bounds != null,
        nonBlankPixels: screenshotResult?.contentAudit?.blank === false,
      },
    },
    assertions: {
      targetIdentity: requestedWindowId != null || capturePlan?.requestedAutomationWindowId != null,
      countsAsOsScreenshotEvidence: captured,
      countsAsCompositorEvidence: captured && captureKind != null,
      nonBlankPixels: screenshotResult?.contentAudit?.blank === false,
      windowIdCaptureAttempted: requestedWindowId != null,
      windowIdCaptureFailed,
      screenRectCaptureAttempted,
      screenRectCaptureFailed,
    },
    limitation: captured
      ? null
      : "macOS WindowServer screenshot capture did not produce usable pixels; do not count this as OS screenshot proof.",
  };
}

async function captureRenderReadbackViaMcp(
  target: Record<string, unknown>,
  outPath: string,
  inspection: InspectionReceipt | null
): Promise<VisualEvidenceReceipt> {
  try {
    const capture = await callMcpTool("computer/capture_render_window", {
      target,
      includeImage: true,
      hiDpi: false,
    });
    const status = String(capture.status ?? "unknown");
    const errorCode = capture.error?.code ?? capture.errorCode ?? null;
    const message = capture.error?.message ?? capture.message ?? null;
    const image = capture.capture?.pngBase64;
    if (status === "captured" && typeof image === "string" && image.length > 0) {
      const decoded = Buffer.from(image, "base64");
      writeFileSync(outPath, decoded);
    }
    const localPixelAudit = existsSync(outPath) ? auditPngContent(outPath) : null;
    const nonBlank = localPixelAudit != null && localPixelAudit.blank === false;
    const captured = status === "captured" && nonBlank;
    return {
      source: "gpui-render-readback",
      available: captured,
      countsAsOsScreenshotEvidence: false,
      countsAsAppRenderEvidence: captured,
      countsAsOffscreenRenderEvidence: false,
      classification: status !== "captured"
        ? "gpui-readback-unavailable"
        : nonBlank ? "captured" : "blank-image-rejected",
      attempts: [{
        source: "gpui-render-readback",
        status: captured
          ? "captured"
          : status === "unsupported" || errorCode === "runtime_unavailable" || errorCode === "unknown_tool"
            ? "unsupported"
            : status === "captured"
              ? "blankRejected"
            : "failed",
        requestedWindowId: null,
        actualWindowId: null,
        errorCode: status === "captured" && !nonBlank ? "render_blank_image_rejected" : errorCode,
        message: status === "captured" && !nonBlank
          ? "GPUI render readback produced a blank or black image"
          : message,
        pixelAudit: localPixelAudit ?? (capture.capture?.pixelAudit ?? null),
      }],
      path: captured ? outPath : null,
      sha256: capture.capture?.sha256 ?? null,
      width: capture.capture?.width ?? null,
      height: capture.capture?.height ?? null,
      pixelAudit: localPixelAudit ?? (capture.capture?.pixelAudit ?? null),
      target: {
        requested: target,
        resolvedWindowId: inspection?.automationWindowId ?? null,
        windowKind: inspection?.windowKind ?? null,
        osWindowId: inspection?.osWindowId ?? null,
        resolvedBounds: inspection?.resolvedBounds ?? null,
      },
      assertions: {
        targetIdentity: inspection?.automationWindowId != null,
        nonBlankPixels: nonBlank,
        dimensionsMeasured: capture.capture?.width != null && capture.capture?.height != null,
        notOsScreenshot: true,
      },
      limitation: capture.limitation
        ?? "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor, native blur, occlusion, shadow, screen recording permission, or actual screen pixels.",
    };
  } catch (error) {
    return {
      source: "gpui-render-readback",
      available: false,
      countsAsOsScreenshotEvidence: false,
      countsAsAppRenderEvidence: false,
      countsAsOffscreenRenderEvidence: false,
      classification: "gpui-readback-unavailable",
      attempts: [{
        source: "gpui-render-readback",
        status: "failed",
        requestedWindowId: null,
        actualWindowId: null,
        errorCode: "mcp_call_failed",
        message: error instanceof Error ? error.message : String(error),
      }],
      path: null,
      target: {
        requested: target,
        resolvedWindowId: inspection?.automationWindowId ?? null,
        windowKind: inspection?.windowKind ?? null,
        osWindowId: inspection?.osWindowId ?? null,
        resolvedBounds: inspection?.resolvedBounds ?? null,
      },
      assertions: {
        targetIdentity: inspection?.automationWindowId != null,
        nonBlankPixels: false,
        dimensionsMeasured: false,
        notOsScreenshot: true,
      },
      limitation: "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor, native blur, occlusion, shadow, screen recording permission, or actual screen pixels.",
    };
  }
}

async function queryAgentChatState(
  session: string,
  requestId: string,
  target?: Record<string, unknown>
): Promise<AgentChatStateResult> {
  const sessionScript = join(PROJECT_ROOT, "scripts/agentic/session.sh");
  const payload: Record<string, unknown> = {
    type: "getAgentChatState",
    requestId,
  };
  if (target) {
    payload.target = target;
  }
  const cmd = JSON.stringify(payload);

  const proc = guardedSpawn([
    "bash",
    sessionScript,
    "rpc",
    session,
    cmd,
    "--expect",
    "agent_chatStateResult",
    "--timeout",
    "3000",
  ]);
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;

  let parsed: Record<string, unknown> | null = null;
  try {
    parsed = JSON.parse(stdout) as Record<string, unknown>;
  } catch {
    parsed = null;
  }

  if (code !== 0) {
    const errorMessage =
      parsed &&
      typeof parsed.error === "object" &&
      parsed.error != null &&
      typeof (parsed.error as Record<string, unknown>).message === "string"
        ? String((parsed.error as Record<string, unknown>).message)
        : stderr.trim() || stdout.trim() || "RPC failed";
    return {
      queried: true,
      snapshot: null,
      error: `Failed to query getAgentChatState: ${errorMessage}`,
    };
  }

  const response = parsed?.response;
  if (!response || typeof response !== "object") {
    return {
      queried: true,
      snapshot: null,
      error: "RPC completed but did not return an agent_chatStateResult payload",
    };
  }

  if ((response as Record<string, unknown>).type !== "agent_chatStateResult") {
    return {
      queried: true,
      snapshot: null,
      error: "RPC returned an unexpected response type",
    };
  }

  return {
    queried: true,
    snapshot: response as Record<string, unknown>,
    error: null,
  };
}

async function queryAgentChatTestProbe(
  session: string,
  requestId: string,
  tail: number,
  target?: Record<string, unknown>
): Promise<ProbeResult> {
  const sessionScript = join(PROJECT_ROOT, "scripts/agentic/session.sh");
  const payload: Record<string, unknown> = {
    type: "getAgentChatTestProbe",
    requestId,
    tail,
  };
  if (target) {
    payload.target = target;
  }
  const cmd = JSON.stringify(payload);

  const proc = guardedSpawn([
    "bash",
    sessionScript,
    "rpc",
    session,
    cmd,
    "--expect",
    "agent_chatTestProbeResult",
    "--timeout",
    "3000",
  ]);
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;

  let parsed: Record<string, unknown> | null = null;
  try {
    parsed = JSON.parse(stdout) as Record<string, unknown>;
  } catch {
    parsed = null;
  }

  if (code !== 0) {
    const errorMessage =
      parsed &&
      typeof parsed.error === "object" &&
      parsed.error != null &&
      typeof (parsed.error as Record<string, unknown>).message === "string"
        ? String((parsed.error as Record<string, unknown>).message)
        : stderr.trim() || stdout.trim() || "RPC failed";
    return {
      queried: true,
      snapshot: null,
      error: `Failed to query getAgentChatTestProbe: ${errorMessage}`,
    };
  }

  const response = parsed?.response;
  if (!response || typeof response !== "object") {
    return {
      queried: true,
      snapshot: null,
      error: "RPC completed but did not return an agent_chatTestProbeResult payload",
    };
  }

  if ((response as Record<string, unknown>).type !== "agent_chatTestProbeResult") {
    return {
      queried: true,
      snapshot: null,
      error: "RPC returned an unexpected response type",
    };
  }

  return {
    queried: true,
    snapshot: response as Record<string, unknown>,
    error: null,
  };
}

async function queryInspection(
  session: string,
  requestId: string,
  target?: Record<string, unknown>
): Promise<InspectionReceipt | null> {
  const sessionScript = join(PROJECT_ROOT, "scripts/agentic/session.sh");
  const payload: Record<string, unknown> = {
    type: "inspectAutomationWindow",
    requestId,
  };
  if (target) {
    payload.target = target;
  }
  const cmd = JSON.stringify(payload);

  const proc = guardedSpawn([
    "bash",
    sessionScript,
    "rpc",
    session,
    cmd,
    "--expect",
    "automationInspectResult",
    "--timeout",
    "5000",
  ]);
  const stdout = await new Response(proc.stdout).text();
  const code = await proc.exited;

  if (code !== 0) {
    return null;
  }

  let parsed: Record<string, unknown> | null = null;
  try {
    parsed = JSON.parse(stdout) as Record<string, unknown>;
  } catch {
    return null;
  }

  const response = (parsed?.response ?? parsed) as Record<string, unknown> | null;
  if (!response) {
    return null;
  }

  return {
    automationWindowId: String(response.windowId ?? ""),
    windowKind: String(response.windowKind ?? ""),
    title: (response.title as string) ?? null,
    osWindowId: (response.osWindowId as number) ?? null,
    targetBoundsInScreenshot:
      (response.targetBoundsInScreenshot as InspectionReceipt["targetBoundsInScreenshot"]) ?? null,
    resolvedBounds:
      (response.resolvedBounds as InspectionReceipt["resolvedBounds"]) ?? null,
    screenshotWidth: (response.screenshotWidth as number) ?? null,
    screenshotHeight: (response.screenshotHeight as number) ?? null,
    pid: (response.pid as number) ?? null,
    pixelProbes: (response.pixelProbes as InspectionReceipt["pixelProbes"]) ?? [],
    warnings: (response.warnings as string[]) ?? [],
  };
}

function diag(event: string, data: Record<string, unknown> = {}): void {
  console.error(JSON.stringify({ event, ...data }));
}

function hasAgentChatAssertions(opts: Record<string, string | boolean>): boolean {
  return [
    "agent_chatStatus",
    "agent_chatPickerOpen",
    "agent_chatPickerClosed",
    "agent_chatInputContains",
    "agent_chatInputMatch",
    "agent_chatCursorAt",
    "agent_chatItemAccepted",
    "agent_chatAcceptedLabel",
    "agent_chatAcceptedTrigger",
    "agent_chatAcceptedVia",
    "agent_chatCursorAfterAccepted",
    "agent_chatContextReady",
    "agent_chatNoSelection",
    "agent_chatHasSelection",
    "agent_chatNoPermission",
    "agent_chatHasPermission",
    "agent_chatVisibleStart",
    "agent_chatVisibleEnd",
    "agent_chatCursorInWindow",
  ].some((key) => hasOpt(opts, key));
}

function shouldQueryProbe(
  opts: Record<string, string | boolean>,
  skipProbe: boolean
): boolean {
  return (
    !skipProbe &&
    (hasOpt(opts, "agent_chatAcceptedVia") || hasOpt(opts, "agent_chatCursorAfterAccepted"))
  );
}

function isAttachedPopupWindowKind(kind: string | null | undefined): boolean {
  return ["ActionsDialog", "PromptPopup", "actionsDialog", "promptPopup"].includes(
    String(kind ?? "")
  );
}

async function getImageDimensions(
  filePath: string
): Promise<{ width: number | null; height: number | null }> {
  try {
    const proc = guardedSpawn(["sips", "-g", "pixelWidth", "-g", "pixelHeight", filePath]);
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    const wMatch = out.match(/pixelWidth:\s*(\d+)/);
    const hMatch = out.match(/pixelHeight:\s*(\d+)/);
    return {
      width: wMatch ? parseInt(wMatch[1], 10) : null,
      height: hMatch ? parseInt(hMatch[1], 10) : null,
    };
  } catch {
    return { width: null, height: null };
  }
}


function auditPngContent(filePath: string): ScreenshotContentAudit {
  return auditRgbaPng(readFileSync(filePath));
}

async function captureScreenshot(
  session: string,
  outPath: string,
  label: string,
  opts: Record<string, string | boolean>,
  inspection: InspectionReceipt | null,
  targetJson?: Record<string, unknown>,
  captureWindowId?: number
): Promise<{ result: ScreenshotResult; plan: CapturePlan }> {
  let captureMethod:
    | "computer/capture_native_window"
    | "window.ts"
    | "captureWindow"
    | null = null;
  let windowCaptureMethod: "native-xcap" | "quartz" | "screencapture" | null = null;
  let windowFrontmost: boolean | null = null;
  let windowFocused: boolean | null = null;
  let windowId: number | null = null;
  let nativeCaptureFailure: string | null = null;
  let screenRectFailure: string | null = null;
  let screenRectFallback: ScreenRectFallbackReceipt | null = null;
  const strictWindowProof = opts.strictWindow === true || hasAgentChatAssertions(opts);
  const inspectionOsWindowId =
    typeof inspection?.osWindowId === "number" && inspection.osWindowId > 0
      ? inspection.osWindowId
      : null;
  const requestedAutomationWindowId = inspection?.automationWindowId ?? null;
  let requestedOsWindowId =
    typeof captureWindowId === "number" && captureWindowId > 0
      ? captureWindowId
      : inspectionOsWindowId;
  if (requestedOsWindowId == null && targetJson) {
    requestedOsWindowId = await resolveNativeWindowIdFromInspection(inspection);
    if (requestedOsWindowId != null) {
      diag("verify_shot_native_window_resolved_from_pid", {
        label,
        automationWindowId: requestedAutomationWindowId,
        pid: inspection?.pid ?? null,
        requestedOsWindowId,
      });
    }
  }
  let captureRouting: CaptureRouting =
    requestedOsWindowId != null
      ? captureWindowId != null
        ? "cli-window-id"
        : "inspection-os-window-id"
      : "generic-frontmost";
  let showIssuedBeforeCapture = false;

  if (opts.strictWindow === true && !targetJson && captureWindowId == null) {
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: null,
        width: null,
        height: null,
        contentAudit: null,
        captureMethod: null,
        windowCaptureMethod: null,
        windowFrontmost: null,
        windowFocused: null,
        windowId: null,
        error:
          "Strict window capture requires --target-json or --capture-window-id; refusing generic frontmost capture",
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
      },
    };
  }

  if (
    targetJson &&
    captureWindowId != null &&
    inspectionOsWindowId != null &&
    captureWindowId !== inspectionOsWindowId
  ) {
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: null,
        width: null,
        height: null,
        contentAudit: null,
        captureMethod: null,
        windowCaptureMethod: null,
        windowFrontmost: null,
        windowFocused: null,
        windowId: null,
        error: `Capture ID mismatch: cli --capture-window-id=${captureWindowId} but inspection resolved osWindowId=${inspectionOsWindowId}`,
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
      },
    };
  }

  if (targetJson && requestedOsWindowId == null) {
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: null,
        width: null,
        height: null,
        contentAudit: null,
        captureMethod: null,
        windowCaptureMethod: null,
        windowFrontmost: null,
        windowFocused: null,
        windowId: null,
        error:
          "Targeted screenshot capture requires a native osWindowId, but inspection did not return one",
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
      },
    };
  }

  const skipShowForAttachedPopup =
    targetJson != null &&
    requestedOsWindowId != null &&
    isAttachedPopupWindowKind(inspection?.windowKind);
  if (!skipShowForAttachedPopup) {
    const showCommand = await sendSessionCommand(session, JSON.stringify({ type: "show" }));
    if (!showCommand.ok) {
      return {
        result: {
          captured: false,
          path: outPath,
          sizeBytes: null,
          width: null,
          height: null,
          contentAudit: null,
          captureMethod: null,
          windowCaptureMethod: null,
          windowFrontmost: null,
          windowFocused: null,
          windowId: null,
          error: `Failed to send show before capture: ${showCommand.stderr || showCommand.stdout || "unknown error"}`,
        },
        plan: {
          routing: captureRouting,
          requestedAutomationWindowId,
          requestedOsWindowId,
          inspectionOsWindowId,
          showIssuedBeforeCapture,
        },
      };
    }
    showIssuedBeforeCapture = true;
    await Bun.sleep(500);
  }

  if (requestedOsWindowId != null && requestedOsWindowId > 0) {
    try {
      const nativeCapture = await captureNativeWindowViaMcp(
        requestedOsWindowId,
        outPath
      );
      if (nativeCapture) {
        captureRouting = "native-window-mcp";
        captureMethod = "computer/capture_native_window";
        windowCaptureMethod = "native-xcap";
        windowFrontmost = nativeCapture.frontmost;
        windowFocused = nativeCapture.focused;
        windowId = nativeCapture.windowId;
      }
    } catch (error) {
      nativeCaptureFailure = error instanceof Error ? error.message : String(error);
      diag("verify_shot_native_capture_fallback", {
        label,
        requestedOsWindowId,
        error: nativeCaptureFailure,
      });
    }
  }

  // Use the window.ts helper for reliable capture
  let stdout = "";
  let stderr = "";
  let code = captureMethod === "computer/capture_native_window" ? 0 : 1;
  if (captureMethod !== "computer/capture_native_window") {
    const windowScript = join(PROJECT_ROOT, "scripts/agentic/window.ts");
    const captureArgs = [
      "bun",
      windowScript,
      "capture",
      outPath,
      "--activate-first",
      "--retry",
      "2",
      "--settle-ms",
      "200",
    ];
    // Thread exact window ID when provided
    if (requestedOsWindowId && requestedOsWindowId > 0) {
      captureArgs.push("--window-id", String(requestedOsWindowId));
    }
    const proc = guardedSpawn(captureArgs);
    stdout = await new Response(proc.stdout).text();
    stderr = await new Response(proc.stderr).text();
    code = await proc.exited;
  }

  if (captureMethod === "computer/capture_native_window") {
    // Exact native capture already wrote outPath and populated identity fields.
  } else if (code === 0) {
    captureMethod = "window.ts";
    try {
      const envelope = JSON.parse(stdout) as {
        data?: {
          method?: "quartz" | "screencapture";
          frontmost?: boolean;
          focused?: boolean;
          windowId?: number;
        };
      };
      windowCaptureMethod = envelope?.data?.method ?? null;
      windowFrontmost = envelope?.data?.frontmost ?? null;
      windowFocused = envelope?.data?.focused ?? null;
      windowId = envelope?.data?.windowId ?? null;
    } catch {
      // leave parsed fields null
    }
  } else if (!strictWindowProof) {
    captureRouting = "runtime-capture-window";
    // Fallback: use session-based captureWindow (only when no Agent Chat assertions).
    // The runtime captureWindow handler now routes through the resolver-driven
    // capture path (capture_window_by_title_via_resolver), which translates the
    // title to an AutomationWindowTarget and emits the same structured log
    // sequence as the protocol captureScreenshot path:
    //   automation.capture_screenshot.title_compatibility
    //   automation.capture_screenshot.targeted
    //   automation.capture_screenshot.candidate_selected (or ambiguous_candidate)
    // An empty title resolves to AutomationWindowTarget::Main.
    captureMethod = "captureWindow";
    const captureCmd = JSON.stringify({
      type: "captureWindow",
      title: inspection?.title ?? "",
      path: outPath,
    });
    const { ok, stderr: sessErr } = await sendSessionCommand(
      session,
      captureCmd
    );
    await Bun.sleep(1000);

    if (!ok || !existsSync(outPath)) {
      return {
        result: {
          captured: false,
          path: outPath,
          sizeBytes: null,
          width: null,
          height: null,
          contentAudit: null,
          captureMethod,
          windowCaptureMethod,
          windowFrontmost,
          windowFocused,
          windowId,
          error: `Capture failed. window.ts: ${stderr.trim()}. session captureWindow: ${sessErr}`,
        },
        plan: {
          routing: captureRouting,
          requestedAutomationWindowId,
          requestedOsWindowId,
          inspectionOsWindowId,
          showIssuedBeforeCapture,
        },
      };
    }
  } else {
    if (targetJson && (inspectionScreenRect(inspection) || requestedOsWindowId != null)) {
      captureRouting = "screen-rect-from-inspection";
      captureMethod = "window.ts";
      windowCaptureMethod = "screencapture";
      windowId = requestedOsWindowId;
    }
    const rectCapture = await captureScreenRectFromInspection(
      inspection,
      outPath,
      200,
      requestedOsWindowId
    );
    screenRectFallback = rectCapture.receipt;
    if (rectCapture.ok && requestedOsWindowId != null) {
      diag("verify_shot_screen_rect_fallback", {
        label,
        reason: "strict-window-helper-failed",
        requestedOsWindowId,
        inspectionOsWindowId,
        source: rectCapture.receipt.source,
        bounds: rectCapture.bounds,
      });
    } else {
      screenRectFailure =
        rectCapture.ok
          ? "screen-rect fallback captured but no target-bound native window id was available"
          : rectCapture.error;
      // Strict mode: window.ts failed and we have Agent Chat assertions — do not fall back to generic frontmost capture.
      const primaryFailure = stderr.trim() || stdout.trim() || "window.ts capture failed";
      const failureParts = [
        `primary=${primaryFailure}`,
        nativeCaptureFailure ? `native=${nativeCaptureFailure}` : null,
        screenRectFailure ? `screenRect=${screenRectFailure}` : null,
      ].filter(Boolean);
      return {
        result: {
          captured: false,
          path: outPath,
          sizeBytes: null,
          width: null,
          height: null,
          contentAudit: null,
          captureMethod: "window.ts",
          windowCaptureMethod,
          windowFrontmost,
          windowFocused,
          windowId,
          error: `Strict window capture failed: ${failureParts.join("; ")}`,
        },
        plan: {
          routing: captureRouting,
          requestedAutomationWindowId,
          requestedOsWindowId,
          inspectionOsWindowId,
          showIssuedBeforeCapture,
          screenRectFallback,
        },
      };
    }
  }

  diag("verify_shot_capture_receipt", {
    label,
    captureMethod,
    windowCaptureMethod,
    windowFrontmost,
    windowFocused,
    windowId,
  });

  // Strict window proof: require quartz method, frontmost, and valid windowId
  if (
    strictWindowProof &&
    (captureRouting === "screen-rect-from-inspection"
      ? windowId == null || windowId <= 0
      : (windowCaptureMethod !== "quartz" && windowCaptureMethod !== "native-xcap") ||
        (windowCaptureMethod !== "native-xcap" && windowFrontmost !== true) ||
        windowId == null ||
        windowId <= 0)
  ) {
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: null,
        width: null,
        height: null,
        contentAudit: null,
        captureMethod,
        windowCaptureMethod,
        windowFrontmost,
        windowFocused,
        windowId,
        error: `Strict window capture required quartz/frontmost/windowId; got method=${windowCaptureMethod ?? "null"} frontmost=${String(windowFrontmost)} windowId=${String(windowId)}`,
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
      },
    };
  }

  if (
    strictWindowProof &&
    requestedOsWindowId != null &&
    windowId != null &&
    windowId !== requestedOsWindowId
  ) {
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: null,
        width: null,
        height: null,
        contentAudit: null,
        captureMethod,
        windowCaptureMethod,
        windowFrontmost,
        windowFocused,
        windowId,
        error: `Strict window capture targeted osWindowId=${requestedOsWindowId} but captured windowId=${windowId}`,
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
      },
    };
  }

  // Wait for file write
  await Bun.sleep(300);

  if (!existsSync(outPath)) {
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: null,
        width: null,
        height: null,
        contentAudit: null,
        captureMethod,
        windowCaptureMethod,
        windowFrontmost,
        windowFocused,
        windowId,
        error: "Screenshot file not created after capture",
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
      },
    };
  }

  let stats = statSync(outPath);
  let dims = await getImageDimensions(outPath);
  let contentAudit: ScreenshotContentAudit;
  try {
    contentAudit = auditPngContent(outPath);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: stats.size,
        width: dims.width,
        height: dims.height,
        contentAudit: null,
        captureMethod,
        windowCaptureMethod,
        windowFrontmost,
        windowFocused,
        windowId,
        error: `Screenshot pixel audit failed: ${reason}`,
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
      },
    };
  }

  diag("verify_shot_pixel_audit", {
    label,
    ...contentAudit,
  });

  if (
    contentAudit.blank &&
    strictWindowProof &&
    captureRouting !== "screen-rect-from-inspection" &&
    requestedOsWindowId != null
  ) {
    if (targetJson && (inspectionScreenRect(inspection) || requestedOsWindowId != null)) {
      captureRouting = "screen-rect-from-inspection";
      captureMethod = "window.ts";
      windowCaptureMethod = "screencapture";
      windowId = requestedOsWindowId;
    }
    const rectCapture = await captureScreenRectFromInspection(
      inspection,
      outPath,
      200,
      requestedOsWindowId
    );
    screenRectFallback = rectCapture.receipt;
    if (rectCapture.ok) {
      stats = statSync(outPath);
      dims = await getImageDimensions(outPath);
      try {
        contentAudit = auditPngContent(outPath);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        return {
          result: {
            captured: false,
            path: outPath,
            sizeBytes: stats.size,
            width: dims.width,
            height: dims.height,
            contentAudit: null,
            captureMethod,
            windowCaptureMethod,
            windowFrontmost,
            windowFocused,
            windowId,
            error: `Screenshot pixel audit failed after screen-rect fallback: ${reason}`,
          },
          plan: {
            routing: captureRouting,
            requestedAutomationWindowId,
            requestedOsWindowId,
            inspectionOsWindowId,
            showIssuedBeforeCapture,
            screenRectFallback,
          },
        };
      }
      diag("verify_shot_screen_rect_fallback", {
        label,
        reason: "blank-primary-capture",
        requestedOsWindowId,
        inspectionOsWindowId,
        source: rectCapture.receipt.source,
        bounds: rectCapture.bounds,
        contentAudit,
      });
    } else {
      screenRectFailure = rectCapture.error;
    }
  }

  if (contentAudit.blank) {
    return {
      result: {
        captured: false,
        path: outPath,
        sizeBytes: stats.size,
        width: dims.width,
        height: dims.height,
        contentAudit,
        captureMethod,
        windowCaptureMethod,
        windowFrontmost,
        windowFocused,
        windowId,
        error: [
          "Screenshot pixel audit rejected a blank/black image. Check macOS Screen Recording permission or capture timing before using this PNG as visual proof.",
          nativeCaptureFailure ? `native=${nativeCaptureFailure}` : null,
          screenRectFailure ? `screenRect=${screenRectFailure}` : null,
        ].filter(Boolean).join(" "),
      },
      plan: {
        routing: captureRouting,
        requestedAutomationWindowId,
        requestedOsWindowId,
        inspectionOsWindowId,
        showIssuedBeforeCapture,
        screenRectFallback,
      },
    };
  }

  return {
    result: {
      captured: true,
      path: outPath,
      sizeBytes: stats.size,
      width: dims.width,
      height: dims.height,
      contentAudit,
      captureMethod,
      windowCaptureMethod,
      windowFrontmost,
      windowFocused,
      windowId,
      error: null,
    },
    plan: {
      routing: captureRouting,
      requestedAutomationWindowId,
      requestedOsWindowId,
      inspectionOsWindowId,
      showIssuedBeforeCapture,
      screenRectFallback,
    },
  };
}

function runAssertions(
  snapshot: Record<string, unknown> | null,
  opts: Record<string, string | boolean>
): AssertionResult[] {
  const results: AssertionResult[] = [];

  if (!snapshot) {
    // If state was expected but missing, every assertion fails
    if (hasOpt(opts, "agent_chatStatus")) {
      results.push({
        name: "agent_chat-status",
        expected: String(opts.agent_chatStatus),
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatPickerOpen) {
      results.push({
        name: "agent_chat-picker-open",
        expected: "true",
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatPickerClosed) {
      results.push({
        name: "agent_chat-picker-closed",
        expected: "true",
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatInputContains")) {
      results.push({
        name: "agent_chat-input-contains",
        expected: `contains "${opts.agent_chatInputContains}"`,
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatInputMatch")) {
      results.push({
        name: "agent_chat-input-match",
        expected: String(opts.agent_chatInputMatch),
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatCursorAt")) {
      results.push({
        name: "agent_chat-cursor-at",
        expected: String(opts.agent_chatCursorAt),
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatItemAccepted) {
      results.push({
        name: "agent_chat-item-accepted",
        expected: "non-null",
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatContextReady) {
      results.push({
        name: "agent_chat-context-ready",
        expected: "true",
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatAcceptedLabel")) {
      results.push({
        name: "agent_chat-accepted-label",
        expected: String(opts.agent_chatAcceptedLabel),
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatAcceptedTrigger")) {
      results.push({
        name: "agent_chat-accepted-trigger",
        expected: String(opts.agent_chatAcceptedTrigger),
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatNoSelection) {
      results.push({
        name: "agent_chat-no-selection",
        expected: "false",
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatHasSelection) {
      results.push({
        name: "agent_chat-has-selection",
        expected: "true",
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatNoPermission) {
      results.push({
        name: "agent_chat-no-permission",
        expected: "false",
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatHasPermission) {
      results.push({
        name: "agent_chat-has-permission",
        expected: "true",
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatVisibleStart")) {
      results.push({
        name: "agent_chat-visible-start",
        expected: String(opts.agent_chatVisibleStart),
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatVisibleEnd")) {
      results.push({
        name: "agent_chat-visible-end",
        expected: String(opts.agent_chatVisibleEnd),
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatCursorInWindow")) {
      results.push({
        name: "agent_chat-cursor-in-window",
        expected: String(opts.agent_chatCursorInWindow),
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatSetupVisible) {
      results.push({
        name: "agent_chat-setup-visible",
        expected: "true",
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatSetupReason")) {
      results.push({
        name: "agent_chat-setup-reason",
        expected: String(opts.agent_chatSetupReason),
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatSetupPrimaryAction")) {
      results.push({
        name: "agent_chat-setup-primary-action",
        expected: String(opts.agent_chatSetupPrimaryAction),
        actual: "<no state>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatSetupSelectedAgent")) {
      results.push({
        name: "agent_chat-setup-selected-agent",
        expected: String(opts.agent_chatSetupSelectedAgent),
        actual: "<no state>",
        passed: false,
      });
    }
    if (opts.agent_chatSetupAgentPickerOpen) {
      results.push({
        name: "agent_chat-setup-agent-picker-open",
        expected: "true",
        actual: "<no state>",
        passed: false,
      });
    }
    return results;
  }

  // Status assertion
  if (hasOpt(opts, "agent_chatStatus")) {
    const actual = String(snapshot.status ?? "<missing>");
    results.push({
      name: "agent_chat-status",
      expected: String(opts.agent_chatStatus),
      actual,
      passed: actual === String(opts.agent_chatStatus),
    });
  }

  // Picker open assertion
  if (opts.agent_chatPickerOpen) {
    const picker = snapshot.picker as Record<string, unknown> | null;
    const actual = picker ? String(picker.open ?? false) : "false";
    results.push({
      name: "agent_chat-picker-open",
      expected: "true",
      actual,
      passed: actual === "true",
    });
  }

  // Picker closed assertion
  if (opts.agent_chatPickerClosed) {
    const picker = snapshot.picker as Record<string, unknown> | null;
    const isOpen = picker ? picker.open === true : false;
    results.push({
      name: "agent_chat-picker-closed",
      expected: "true",
      actual: String(!isOpen),
      passed: !isOpen,
    });
  }

  // Input contains assertion
  if (hasOpt(opts, "agent_chatInputContains")) {
    const inputText = String(snapshot.inputText ?? "");
    const substring = String(opts.agent_chatInputContains);
    results.push({
      name: "agent_chat-input-contains",
      expected: `contains "${substring}"`,
      actual: `"${inputText}"`,
      passed: inputText.includes(substring),
    });
  }

  // Input match assertion
  if (hasOpt(opts, "agent_chatInputMatch")) {
    const inputText = String(snapshot.inputText ?? "");
    const expected = String(opts.agent_chatInputMatch);
    results.push({
      name: "agent_chat-input-match",
      expected: `"${expected}"`,
      actual: `"${inputText}"`,
      passed: inputText === expected,
    });
  }

  // Cursor position assertion
  if (hasOpt(opts, "agent_chatCursorAt")) {
    const cursorIndex = Number(snapshot.cursorIndex ?? -1);
    const expected = Number(opts.agent_chatCursorAt);
    results.push({
      name: "agent_chat-cursor-at",
      expected: String(expected),
      actual: String(cursorIndex),
      passed: cursorIndex === expected,
    });
  }

  // Item accepted assertion
  if (opts.agent_chatItemAccepted) {
    const item = snapshot.lastAcceptedItem;
    results.push({
      name: "agent_chat-item-accepted",
      expected: "non-null",
      actual: item ? "present" : "null",
      passed: item != null,
    });
  }

  // Context ready assertion
  if (opts.agent_chatContextReady) {
    const ready = snapshot.contextReady === true;
    results.push({
      name: "agent_chat-context-ready",
      expected: "true",
      actual: String(ready),
      passed: ready,
    });
  }

  // Accepted item label assertion
  if (hasOpt(opts, "agent_chatAcceptedLabel")) {
    const item = snapshot.lastAcceptedItem as Record<string, unknown> | null;
    const actual = item ? String(item.label ?? "<missing>") : "<no item>";
    const expected = String(opts.agent_chatAcceptedLabel);
    results.push({
      name: "agent_chat-accepted-label",
      expected,
      actual,
      passed: actual === expected,
    });
  }

  // Accepted item trigger assertion
  if (hasOpt(opts, "agent_chatAcceptedTrigger")) {
    const item = snapshot.lastAcceptedItem as Record<string, unknown> | null;
    const actual = item ? String(item.trigger ?? "<missing>") : "<no item>";
    const expected = String(opts.agent_chatAcceptedTrigger);
    results.push({
      name: "agent_chat-accepted-trigger",
      expected,
      actual,
      passed: actual === expected,
    });
  }

  // Selection assertions
  if (opts.agent_chatNoSelection) {
    const hasSel = snapshot.hasSelection === true;
    results.push({
      name: "agent_chat-no-selection",
      expected: "false",
      actual: String(hasSel),
      passed: !hasSel,
    });
  }

  if (opts.agent_chatHasSelection) {
    const hasSel = snapshot.hasSelection === true;
    results.push({
      name: "agent_chat-has-selection",
      expected: "true",
      actual: String(hasSel),
      passed: hasSel,
    });
  }

  // Permission assertions
  if (opts.agent_chatNoPermission) {
    const hasPerm = snapshot.hasPendingPermission === true;
    results.push({
      name: "agent_chat-no-permission",
      expected: "false",
      actual: String(hasPerm),
      passed: !hasPerm,
    });
  }

  if (opts.agent_chatHasPermission) {
    const hasPerm = snapshot.hasPendingPermission === true;
    results.push({
      name: "agent_chat-has-permission",
      expected: "true",
      actual: String(hasPerm),
      passed: hasPerm,
    });
  }

  // Input layout assertions
  const layout = snapshot.inputLayout as Record<string, unknown> | null;

  if (hasOpt(opts, "agent_chatVisibleStart")) {
    const expected = Number(opts.agent_chatVisibleStart);
    const actual = layout ? Number(layout.visibleStart ?? -1) : -1;
    results.push({
      name: "agent_chat-visible-start",
      expected: String(expected),
      actual: layout ? String(actual) : "<no layout>",
      passed: actual === expected,
    });
  }

  if (hasOpt(opts, "agent_chatVisibleEnd")) {
    const expected = Number(opts.agent_chatVisibleEnd);
    const actual = layout ? Number(layout.visibleEnd ?? -1) : -1;
    results.push({
      name: "agent_chat-visible-end",
      expected: String(expected),
      actual: layout ? String(actual) : "<no layout>",
      passed: actual === expected,
    });
  }

  if (hasOpt(opts, "agent_chatCursorInWindow")) {
    const expected = Number(opts.agent_chatCursorInWindow);
    const actual = layout ? Number(layout.cursorInWindow ?? -1) : -1;
    results.push({
      name: "agent_chat-cursor-in-window",
      expected: String(expected),
      actual: layout ? String(actual) : "<no layout>",
      passed: actual === expected,
    });
  }

  // Agent Chat setup assertions
  const setup = snapshot.setup as Record<string, unknown> | null;

  if (opts.agent_chatSetupVisible) {
    results.push({
      name: "agent_chat-setup-visible",
      expected: "true",
      actual: setup ? "true" : "false",
      passed: setup != null,
    });
  }

  if (hasOpt(opts, "agent_chatSetupReason")) {
    const expected = String(opts.agent_chatSetupReason);
    const actual = setup ? String(setup.reasonCode ?? "<missing>") : "<no setup>";
    results.push({
      name: "agent_chat-setup-reason",
      expected,
      actual,
      passed: actual === expected,
    });
  }

  if (hasOpt(opts, "agent_chatSetupPrimaryAction")) {
    const expected = String(opts.agent_chatSetupPrimaryAction);
    const actual = setup ? String(setup.primaryAction ?? "<missing>") : "<no setup>";
    results.push({
      name: "agent_chat-setup-primary-action",
      expected,
      actual,
      passed: actual === expected,
    });
  }

  if (hasOpt(opts, "agent_chatSetupSelectedAgent")) {
    const expected = String(opts.agent_chatSetupSelectedAgent);
    const actual = setup ? String(setup.selectedAgentId ?? "<none>") : "<no setup>";
    results.push({
      name: "agent_chat-setup-selected-agent",
      expected,
      actual,
      passed: actual === expected,
    });
  }

  if (opts.agent_chatSetupAgentPickerOpen) {
    const actual = setup ? String(setup.agentPickerOpen ?? false) : "false";
    results.push({
      name: "agent_chat-setup-agent-picker-open",
      expected: "true",
      actual,
      passed: actual === "true",
    });
  }

  return results;
}

function runProbeAssertions(
  probeSnapshot: Record<string, unknown> | null,
  opts: Record<string, string | boolean>
): AssertionResult[] {
  const results: AssertionResult[] = [];

  const needsProbe =
    hasOpt(opts, "agent_chatAcceptedVia") || hasOpt(opts, "agent_chatCursorAfterAccepted");
  if (!needsProbe) return results;

  if (!probeSnapshot) {
    if (hasOpt(opts, "agent_chatAcceptedVia")) {
      results.push({
        name: "agent_chat-accepted-via",
        expected: String(opts.agent_chatAcceptedVia),
        actual: "<no probe>",
        passed: false,
      });
    }
    if (hasOpt(opts, "agent_chatCursorAfterAccepted")) {
      results.push({
        name: "agent_chat-cursor-after-accepted",
        expected: String(opts.agent_chatCursorAfterAccepted),
        actual: "<no probe>",
        passed: false,
      });
    }
    return results;
  }

  // Extract the last accepted item from the probe
  const acceptedItems = probeSnapshot.acceptedItems as
    | Record<string, unknown>[]
    | undefined;
  const lastAccepted =
    acceptedItems && acceptedItems.length > 0
      ? acceptedItems[acceptedItems.length - 1]
      : null;

  if (hasOpt(opts, "agent_chatAcceptedVia")) {
    const expected = String(opts.agent_chatAcceptedVia);
    const actual = lastAccepted
      ? String(lastAccepted.acceptedViaKey ?? "<missing>")
      : "<no accepted items>";
    results.push({
      name: "agent_chat-accepted-via",
      expected,
      actual,
      passed: actual === expected,
    });
  }

  if (hasOpt(opts, "agent_chatCursorAfterAccepted")) {
    const expected = Number(opts.agent_chatCursorAfterAccepted);
    const actual = lastAccepted
      ? Number(lastAccepted.cursorAfter ?? -1)
      : -1;
    results.push({
      name: "agent_chat-cursor-after-accepted",
      expected: String(expected),
      actual: lastAccepted ? String(actual) : "<no accepted items>",
      passed: actual === expected,
    });
  }

  return results;
}

function buildVisionChecks(
  screenshotResult: ScreenshotResult | null,
  opts: Record<string, string | boolean>
): VisionCheck[] {
  if (!opts.emitVisionCrops) return [];
  if (!screenshotResult?.captured || !screenshotResult.path) return [];

  const checks: VisionCheck[] = [];
  const imgWidth = screenshotResult.width ?? 998;
  const imgHeight = screenshotResult.height ?? 712;

  // Composer line check — bottom region of the window where input lives
  const composerHeight = 56;
  const composerY = Math.max(0, imgHeight - composerHeight - 40);

  // Caret placement after insertion — mustReview because state alone
  // cannot prove the caret is *visually* positioned correctly
  if (hasOpt(opts, "agent_chatAcceptedVia") || opts.agent_chatItemAccepted) {
    checks.push({
      name: "composer-caret",
      path: screenshotResult.path,
      question:
        "Is the caret (blinking cursor) immediately after the accepted picker text with no gap or misalignment?",
      crop: {
        x: 0,
        y: composerY,
        width: imgWidth,
        height: composerHeight,
      },
      expectedAnswer: "yes",
      mustReview: true,
      failureMessage:
        "Caret is not visually aligned after Agent Chat insertion. The cursor may have jumped or the inserted text may be clipped.",
    });
  }

  // Picker dismissal check — mustReview because a stale picker overlay
  // can linger visually even when state reports it closed
  if (opts.agent_chatPickerClosed || hasOpt(opts, "agent_chatAcceptedVia")) {
    checks.push({
      name: "picker-dismissed",
      path: screenshotResult.path,
      question:
        "Is the inline mention/slash picker dropdown fully dismissed (no floating list visible)?",
      crop: {
        x: 0,
        y: Math.max(0, composerY - 200),
        width: imgWidth,
        height: 200,
      },
      expectedAnswer: "yes",
      mustReview: true,
      failureMessage:
        "Picker overlay is still visually present despite state reporting it closed. Possible render stale frame.",
    });
  }

  // Single-line composer stability — mustReview because layout metrics
  // can report correct values while the visual input jumps or clips
  if (hasOpt(opts, "agent_chatAcceptedVia") || opts.agent_chatItemAccepted) {
    checks.push({
      name: "single-line-stability",
      path: screenshotResult.path,
      question:
        "Does the single-line composer remain visually stable — no clipped leading text, no vertical shift, no layout jump?",
      crop: {
        x: 0,
        y: composerY,
        width: imgWidth,
        height: composerHeight + 20,
      },
      expectedAnswer: "yes",
      mustReview: true,
      failureMessage:
        "Single-line composer shifted, clipped, or jumped after picker acceptance or cursor movement.",
    });
  }

  // Picker visibility check (for picker-open assertions)
  if (opts.agent_chatPickerOpen) {
    checks.push({
      name: "picker-visible",
      path: screenshotResult.path,
      question:
        "Is the inline mention/slash picker dropdown visible with selectable rows?",
      crop: {
        x: 0,
        y: Math.max(0, composerY - 200),
        width: imgWidth,
        height: 200,
      },
      expectedAnswer: "yes",
      mustReview: true,
      failureMessage:
        "Picker dropdown is not visible despite state reporting it open.",
    });
  }

  return checks;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

const opts = parseArgs();

if (opts.help) {
  const jsonFlag = process.argv.includes("--json");
  if (jsonFlag) {
    const helpJson = {
      schemaVersion: 1,
      script: "verify-shot",
      commands: [
        { name: "run", description: "Collect state/probe/screenshot proof", flags: ["--session", "--label", "--out", "--target-json", "--capture-window-id", "--strict-window", "--visual-source", "--vision", "--skip-screenshot", "--skip-state", "--skip-probe", "--json"] },
      ],
      contracts: [
        "popup-capture-receipts",
        "detached-proof-contract",
        "screenshot-pixel-content-audit",
        "structured-visual-evidence-tiers",
      ],
      receipts: [
        "popupCapture.strategy",
        "popupCapture.targetBounds",
        "popupCapture.semanticReceiptsArePrimary",
        "screenshotReceipt.contentAudit",
      ],
    };
    console.log(JSON.stringify(helpJson, null, 2));
    process.exit(0);
  }
  console.log(`Usage: bun scripts/agentic/verify-shot.ts --session NAME [options]

Agent Chat proof bundle: state receipt + test probe + screenshot + vision prompts.

Options:
  --session NAME              Session name (default: "default")
  --label LABEL               Human-readable step label (default: "verify")
  --out PATH                  Screenshot output path (default: .test-screenshots/<label>-<ts>.png)
  --agent_chat-status STATUS         Assert Agent Chat status equals this value
  --agent_chat-picker-open           Assert picker is open
  --agent_chat-picker-closed         Assert picker is closed
  --agent_chat-input-contains STR    Assert input text contains substring
  --agent_chat-input-match STR       Assert input text equals exactly
  --agent_chat-cursor-at N           Assert cursor is at character index N
  --agent_chat-item-accepted         Assert lastAcceptedItem is non-null
  --agent_chat-accepted-label STR    Assert lastAcceptedItem.label equals STR
  --agent_chat-accepted-trigger STR  Assert lastAcceptedItem.trigger equals STR (@ or /)
  --agent_chat-accepted-via KEY      Assert probe acceptedItems[last].acceptedViaKey (enter|tab)
  --agent_chat-cursor-after-accepted N  Assert probe acceptedItems[last].cursorAfter equals N
  --agent_chat-context-ready         Assert contextReady is true
  --agent_chat-no-selection          Assert hasSelection is false
  --agent_chat-has-selection         Assert hasSelection is true
  --agent_chat-no-permission         Assert hasPendingPermission is false
  --agent_chat-has-permission        Assert hasPendingPermission is true
  --agent_chat-visible-start N       Assert inputLayout.visibleStart equals N
  --agent_chat-visible-end N         Assert inputLayout.visibleEnd equals N
  --agent_chat-cursor-in-window N    Assert inputLayout.cursorInWindow equals N
  --agent_chat-setup-visible         Assert setup card is present (status == "setup")
  --agent_chat-setup-reason CODE     Assert setup.reasonCode equals CODE
  --agent_chat-setup-primary-action A  Assert setup.primaryAction equals A
  --agent_chat-setup-selected-agent ID Assert setup.selectedAgentId equals ID
  --agent_chat-setup-agent-picker-open Assert setup.agentPickerOpen is true
  --probe-tail N              Number of probe events to request (default: 20)
  --vision                    Emit vision checks with mustReview prompts and requiresVisionReview
  --emit-vision-crops         Alias for --vision
  --skip-screenshot           Only run state assertions, skip capture
  --skip-state                Only capture screenshot, skip state query
  --skip-probe                Skip Agent Chat test probe query
  --strict-window             Require --target-json or --capture-window-id and forbid generic fallback
  --visual-source SOURCE      os|render|auto (default: os). auto tries GPUI app-render readback after OS capture is blocked.
  --target-json JSON          Agent Chat window target for getAgentChatState/getAgentChatTestProbe RPCs
  --capture-window-id N       Exact native window ID for screencapture
                              (the inspected osWindowId, not automationWindowId)
  --request-id ID             Request ID for getAgentChatState (auto-generated)

Verification order (Agent Chat golden path):
  1. State receipt (getAgentChatState) — machine-readable proof
  2. Probe receipt (getAgentChatTestProbe) — key-route/picker-acceptance telemetry
  3. Screenshot capture — visual proof with metadata + pixel-content audit
  4. Vision checks — structured prompts for external image readers
  5. Assertions check state + probe fields

Exit 0 = all assertions pass. Exit 1 = assertion failure. Exit 2 = infra error.`);
  process.exit(0);
}

if (
  process.env.SCRIPT_KIT_NONINTERACTIVE === "1" &&
  opts.skipScreenshot !== true
) {
  console.log(JSON.stringify({
    schemaVersion: SCHEMA_VERSION,
    status: "error",
    error: {
      code: "NONINTERACTIVE_SAFETY_REFUSED",
      message:
        "OS screenshots, app-render readback, window activation, and MCP capture are forbidden during noninteractive verification; pass --skip-screenshot for passive proof.",
    },
  }, null, 2));
  process.exit(2);
}

const startTime = Date.now();
const session = String(opts.session ?? "default");
const label = String(opts.label ?? "verify");
const requestId =
  String(opts.requestId ?? `verify-${label}-${Date.now()}`);
const skipScreenshot = opts.skipScreenshot === true;
const skipState = opts.skipState === true;
const skipProbe = opts.skipProbe === true;
const probeTail = Number(opts.probeTail ?? 20);
const captureWindowId = typeof opts.captureWindowId === "string"
  ? (() => {
      const parsed = parseInt(opts.captureWindowId, 10);
      return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
    })()
  : undefined;
const visualSource = typeof opts.visualSource === "string"
  ? String(opts.visualSource)
  : "os";
if (!["os", "render", "auto"].includes(visualSource)) {
  console.error(`Invalid --visual-source ${visualSource}; expected os, render, or auto`);
  process.exit(2);
}
const wantsOsVisual = visualSource === "os" || visualSource === "auto";
const wantsRenderVisual = visualSource === "render" || visualSource === "auto";

// Parse --target-json for Agent Chat window targeting
let targetJson: Record<string, unknown> | undefined;
if (typeof opts.targetJson === "string") {
  try {
    targetJson = JSON.parse(opts.targetJson) as Record<string, unknown>;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.error(`Invalid --target-json: ${reason}`);
    process.exit(2);
  }
}

// Determine output path
let outPath: string;
if (opts.out) {
  outPath = resolve(String(opts.out));
} else {
  const screenshotDir = join(PROJECT_ROOT, ".test-screenshots");
  if (!skipScreenshot && !existsSync(screenshotDir)) {
    mkdirSync(screenshotDir, { recursive: true });
  }
  outPath = join(
    screenshotDir,
    `${label}-${Date.now()}.png`
  );
}

// Step 1: Query Agent Chat state (unless skipped)
let stateResult: AgentChatStateResult | null = null;
if (!skipState) {
  stateResult = await queryAgentChatState(session, requestId, targetJson);
}

// Step 2: Query Agent Chat test probe only when probe assertions are requested
let probeResult: ProbeResult | null = null;
const needsProbe = shouldQueryProbe(opts, skipProbe);
if (needsProbe) {
  const probeRequestId = `${requestId}-probe`;
  probeResult = await queryAgentChatTestProbe(session, probeRequestId, probeTail, targetJson);
  diag("verify_shot_probe_loaded", {
    label,
    queried: probeResult.queried,
    hasSnapshot: probeResult.snapshot != null,
    acceptedVia: hasOpt(opts, "agent_chatAcceptedVia")
      ? String(opts.agent_chatAcceptedVia)
      : null,
    cursorAfterAccepted: hasOpt(opts, "agent_chatCursorAfterAccepted")
      ? String(opts.agent_chatCursorAfterAccepted)
      : null,
    error: probeResult.error ?? null,
  });
}

// Step 2b: Query inspection when --target-json or --vision is present
let inspection: InspectionReceipt | null = null;
if (targetJson || opts.emitVisionCrops) {
  const inspectRequestId = `${requestId}-inspect`;
  inspection = await queryInspection(session, inspectRequestId, targetJson);
  diag("verify_shot_inspection_loaded", {
    label,
    automationWindowId: inspection?.automationWindowId ?? null,
    windowKind: inspection?.windowKind ?? null,
    osWindowId: inspection?.osWindowId ?? null,
    probeCount: inspection?.pixelProbes.length ?? 0,
    warningCount: inspection?.warnings.length ?? 0,
  });
}

// Step 3: Capture screenshot (unless skipped)
let screenshotResult: ScreenshotResult | null = null;
let capturePlan: CapturePlan | null = null;
let visualEvidence: VisualEvidenceReceipt | null = null;
let renderEvidence: VisualEvidenceReceipt | null = null;
if (!skipScreenshot && wantsOsVisual) {
  const captureOutcome = await captureScreenshot(
    session,
    outPath,
    label,
    opts,
    inspection,
    targetJson,
    captureWindowId
  );
  screenshotResult = captureOutcome.result;
  capturePlan = captureOutcome.plan;
  visualEvidence = buildOsVisualEvidence(screenshotResult, capturePlan);
}
if (!skipScreenshot && targetJson && wantsRenderVisual) {
  if (
    visualSource === "render" ||
    (visualSource === "auto" && visualEvidence?.available === false)
  ) {
    const renderOutPath = visualSource === "render"
      ? outPath
      : outPath.replace(/\.png$/i, ".render.png");
    renderEvidence = await captureRenderReadbackViaMcp(targetJson, renderOutPath, inspection);
  }
}

// Step 4: Run assertions against state + probe
const stateAssertions = runAssertions(stateResult?.snapshot ?? null, opts);
const probeAssertions = runProbeAssertions(
  probeResult?.snapshot ?? null,
  opts
);
const assertions = [...stateAssertions, ...probeAssertions];

// Step 5: Build vision checks
const visionChecks = buildVisionChecks(screenshotResult, opts);

// Log assertion evaluation
for (const a of assertions) {
  diag("verify_shot_assertion", {
    label,
    assertion: a.name,
    expected: a.expected,
    actual: a.actual,
    passed: a.passed,
  });
}

// Build receipt
const hasMustReviewItems = visionChecks.some((v) => v.mustReview);

// Compute deterministic popup capture strategy receipt
let popupCapture: PopupCaptureReceipt | null = null;
let popupCaptureInfraError = false;
if (inspection) {
  const wk = inspection.windowKind;
  const attachedPopupKinds = ["ActionsDialog", "PromptPopup", "actionsDialog", "promptPopup"];
  const detachedKinds = ["AgentChatDetached", "Notes", "agentChatDetached", "notes"];
  const isAttached = attachedPopupKinds.includes(wk);
  const isDetached = detachedKinds.includes(wk);

  // Parse targetBoundsInScreenshot from inspection if available
  let targetBounds: PopupCaptureReceipt["targetBounds"] = null;
  if (inspection.targetBoundsInScreenshot) {
    const tb = inspection.targetBoundsInScreenshot as {
      x: number; y: number; width: number; height: number;
    };
    targetBounds = { x: tb.x, y: tb.y, width: tb.width, height: tb.height };
  }

  if (isAttached) {
    if (!targetBounds) {
      // Fail loud: attached popup without crop bounds is not a valid proof.
      // The Rust layer should have already failed, but enforce here too.
      diag("automation.capture_screenshot.parent_crop_failed", {
        windowKind: wk,
        automationWindowId: inspection.automationWindowId,
        reason: "attached popup has no targetBoundsInScreenshot — parent identity or popup bounds missing",
      });
      popupCapture = {
        strategy: "parent_capture_with_crop",
        windowKind: wk,
        targetBounds: null,
        semanticReceiptsArePrimary: true,
      };
      // Mark the receipt as an error since we cannot verify this popup.
      // The receipt will show strategy=parent_capture_with_crop but null bounds,
      // and the overall status will be "error" because hasInfraError will be set.
      popupCaptureInfraError = true;
      if (!stateResult?.error && !screenshotResult?.error) {
        // Force infrastructure error when crop bounds are missing for attached popup
        if (screenshotResult) {
          screenshotResult.error = `Attached popup ${wk} captured without crop bounds — cannot verify popup region`;
          screenshotResult.captured = false;
        }
      }
    } else {
      diag("automation.capture_screenshot.parent_crop_selected", {
        windowKind: wk,
        automationWindowId: inspection.automationWindowId,
        targetBounds,
      });
      popupCapture = {
        strategy: "parent_capture_with_crop",
        windowKind: wk,
        targetBounds,
        semanticReceiptsArePrimary: true,
      };
    }
  } else if (isDetached) {
    popupCapture = {
      strategy: "direct_window_capture",
      windowKind: wk,
      targetBounds: null,
      semanticReceiptsArePrimary: true,
    };
  } else {
    popupCapture = {
      strategy: "not_applicable",
      windowKind: wk,
      targetBounds: null,
      semanticReceiptsArePrimary: false,
    };
  }
}

const allPassed = assertions.every((a) => a.passed);
const osInfraError = wantsOsVisual && screenshotResult && !screenshotResult.captured && !skipScreenshot;
const renderInfraError =
  visualSource === "render" && (!renderEvidence || renderEvidence.available !== true);
const hasInfraError =
  (stateResult?.error && !skipState) ||
  (probeResult?.error && needsProbe) ||
  osInfraError ||
  renderInfraError ||
  popupCaptureInfraError;

// Build resolvedTarget from inspection when available
const resolvedTarget: VerifyReceipt["resolvedTarget"] = inspection
  ? {
      windowId: inspection.automationWindowId,
      windowKind: inspection.windowKind,
      title: inspection.title ?? null,
      surfaceId: null,
    }
  : null;

// Infer dispatch path from target-json shape
const dispatchPath: VerifyReceipt["dispatchPath"] = targetJson
  ? (targetJson as { type: string }).type === "id"
    ? "exact_handle"
    : "window_role_fallback"
  : undefined;

const receipt: VerifyReceipt = {
  schemaVersion: SCHEMA_VERSION,
  status: hasInfraError ? "error" : allPassed ? "pass" : "fail",
  label,
  timestamp: new Date().toISOString(),
  durationMs: Date.now() - startTime,
  requiresVisionReview: hasMustReviewItems,
  resolvedTarget,
  ...(dispatchPath ? { dispatchPath } : {}),
  ...(resolvedTarget ? { resolvedWindowId: resolvedTarget.windowId } : {}),
  ...(inspection?.osWindowId != null ? { osWindowId: inspection.osWindowId } : {}),
  ...(capturePlan?.inspectionOsWindowId != null
    ? { inspectionOsWindowId: capturePlan.inspectionOsWindowId }
    : {}),
  ...(capturePlan?.routing ? { captureRouting: capturePlan.routing } : {}),
  ...(capturePlan ? { requestedAutomationWindowId: capturePlan.requestedAutomationWindowId } : {}),
  ...(capturePlan ? { requestedOsWindowId: capturePlan.requestedOsWindowId } : {}),
  ...(capturePlan ? { showIssuedBeforeCapture: capturePlan.showIssuedBeforeCapture } : {}),
  ...(capturePlan?.screenRectFallback
    ? { screenRectFallback: capturePlan.screenRectFallback }
    : {}),
  // Stable proof bundle fields
  state: stateResult?.snapshot ?? null,
  probe: probeResult?.snapshot ?? null,
  screenshot: screenshotResult
    ? {
        path: screenshotResult.path,
        captureMethod: screenshotResult.captureMethod,
        windowCaptureMethod: screenshotResult.windowCaptureMethod,
        windowId: screenshotResult.windowId,
      }
    : null,
  captureTarget: screenshotResult
    ? {
        requestedWindowId: capturePlan?.requestedOsWindowId ?? null,
        actualWindowId: screenshotResult.windowId,
      }
    : null,
  inspection,
  popupCapture,
  visualEvidence,
  renderEvidence,
  visionCrops: visionChecks,
  // Detailed receipts
  stateReceipt: stateResult,
  probeReceipt: probeResult,
  screenshotReceipt: screenshotResult,
  visionChecks,
  assertions,
  summary: buildSummary(assertions, stateResult, probeResult, screenshotResult),
};

console.log(JSON.stringify(receipt, null, 2));

if (hasInfraError) {
  process.exit(2);
} else {
  process.exit(allPassed ? 0 : 1);
}

function buildSummary(
  assertions: AssertionResult[],
  state: AgentChatStateResult | null,
  probe: ProbeResult | null,
  screenshot: ScreenshotResult | null
): string {
  const parts: string[] = [];

  if (state) {
    if (state.error) {
      parts.push(`state: ERROR (${state.error})`);
    } else {
      parts.push("state: queried");
    }
  }

  if (probe) {
    if (probe.error) {
      parts.push(`probe: ERROR (${probe.error})`);
    } else {
      parts.push("probe: queried");
    }
  }

  if (screenshot) {
    if (screenshot.captured) {
      parts.push(`screenshot: ${screenshot.path} (${screenshot.sizeBytes}B)`);
    } else {
      parts.push(`screenshot: FAILED (${screenshot.error})`);
    }
  }

  if (assertions.length > 0) {
    const passed = assertions.filter((a) => a.passed).length;
    const failed = assertions.filter((a) => !a.passed).length;
    parts.push(`assertions: ${passed} passed, ${failed} failed`);
    for (const a of assertions.filter((a) => !a.passed)) {
      parts.push(`  FAIL ${a.name}: expected ${a.expected}, got ${a.actual}`);
    }
  }

  return parts.join(" | ");
}
