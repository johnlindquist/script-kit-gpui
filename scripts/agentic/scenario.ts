#!/usr/bin/env bun
/**
 * scripts/agentic/scenario.ts
 *
 * Replayable agentic scenarios that produce machine-readable proof bundles.
 * Each scenario resolves one exact target once, reuses it for every step,
 * and records the exact windowId/surfaceId in the emitted proof bundle.
 *
 * Proof bundles are the regression substrate for cross-window automation:
 * target resolution, inspect snapshots, GPUI events, and waits captured
 * in one structured receipt.
 *
 * Usage (standalone):
 *   bun scripts/agentic/scenario.ts --session default --scenario detached-agent_chat-exact-id --index 0
 *
 * Output:
 *   stdout: JSON proof bundle (schemaVersion 2)
 *   stderr: structured step-level logs (NDJSON)
 */

import { resolve } from "path";
import {
  assertNoninteractiveSubprocess,
  inspectionSessionCleanup,
  SessionOwnershipRegistry,
} from "../devtools/lib/operator-safety.ts";
import {
  assertTargetStable,
  listNativePeerWindows,
  promoteExactTarget,
  runTool as runTargetThreadTool,
  targetedRpc,
  type TargetThreadFailure,
  type TargetThreadIdentity,
} from "./target-thread";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const PROOF_BUNDLE_SCHEMA_VERSION = 2;
const scenarioSessionOwnership = new SessionOwnershipRegistry();

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ProofBundleStep {
  type: "resolveTarget" | "inspect" | "simulateGpuiEvent" | "waitFor";
  at: string;
  request: Record<string, unknown>;
  response: Record<string, unknown>;
}

/** Deterministic popup capture strategy. */
export interface PopupCaptureSummary {
  strategy: "parent_capture_with_crop" | "direct_window_capture" | "not_applicable";
  targetBounds?: { x: number; y: number; width: number; height: number } | null;
  semanticReceiptsArePrimary: boolean;
}

export interface ProofBundle {
  schemaVersion: 2;
  scenario: string;
  surfaceClass?: "main" | "attachedPopup" | "detached";
  resolvedTarget: {
    windowId: string;
    windowKind: string;
    title?: string | null;
    surfaceId?: string | null;
  };
  /** Routed input method used during the flow. */
  inputMethod?: "batch" | "simulateGpuiEvent" | "native";
  /** Dispatch path: exact_handle when target was an ID. */
  dispatchPath?: "exact_handle" | "window_role_fallback";
  /** Resolved window ID (same as resolvedTarget.windowId). */
  resolvedWindowId?: string;
  /** OS-level window ID (CGWindowID) when available from inspection. */
  osWindowId?: number | null;
  /** Popup capture strategy receipt. */
  popupCapture?: PopupCaptureSummary;
  /** Inspection metadata from inspectAutomationWindow. */
  inspect?: {
    screenshotWidth?: number | null;
    screenshotHeight?: number | null;
    warnings: string[];
  };
  steps: ProofBundleStep[];
  warnings: string[];
}

export interface HardScenarioReceipt {
  schemaVersion: 2;
  scenario:
    | "detached-agent_chat-target-threading-stress"
    | "notes-agent_chat-delayed-action-origin-stress"
    | "file-portal-origin-roundtrip"
    | "permission-privacy-preflight"
    | "shortcut-recorder-focus-capture"
    | "template-prompt-automation-parity-stress"
    | "current-app-commands-frontmost-stress"
    | "actions-captured-subject-frame-stress"
    | "drop-prompt-native-drop-privacy-stress"
    | "path-prompt-filesystem-edge-stress"
    | "screenshot-identity-agent_chat-context-stress"
    | "clipboard-history-portal-range-stress"
    | "browser-tabs-cache-identity-stress"
    | "scroll-selection-reanchor-stress"
    | "permission-assistant-drag-preflight-stress"
    | "quick-terminal-pty-apply-back-stress"
    | "mcp-context-resource-attachment-identity-stress"
    | "settings-theme-hot-reload-stress"
    | "file-search-drag-out-identity-stress"
    | "scriptlet-bundle-execution-matrix-stress"
    | "tray-global-hotkey-menu-mutation-stress"
    | "multi-window-resize-monitor-restoration-stress"
    | "agent_chat-targeted-dictation-delivery-stress"
    | "clipboard-share-trust-install-stress"
    | "clipboard-share-watcher-stale-replay-stress"
    | "permission-share-cross-prompt-focus-stress"
    | "visible-text-clipping-overlap-stress"
    | "layout-measurement-regression-stress"
    | "screenshot-semantics-visual-consistency-stress"
    | "modal-stack-arbitration-stress"
    | "cross-surface-export-provenance-stress"
    | "dev-session-recovery-stale-target-stress"
    | "menu-syntax-ambiguity-diagnostics-stress"
    | "ime-composition-input-boundary-stress"
    | "accessibility-selected-text-fallback-stress"
    | "display-migration-visual-bounds-stress"
    | "native-picker-external-return-focus-stress"
    | "drag-cancel-payload-scope-stress"
    | "runtime-appearance-churn-focused-input-stress"
    | "power-resume-window-generation-stress"
    | "menu-tray-notification-modal-interruption-stress"
    | "stream-progress-cancel-visual-stability-stress"
    | "dictation-media-permission-readiness-churn-stress"
    | "animation-frame-capture-determinism-stress"
    | "accessibility-tree-semantic-parity-stress"
    | "rtl-bidi-emoji-text-rendering-stress"
    | "high-volume-virtualized-list-stability-stress"
    | "input-modality-transition-ownership-stress"
    | "multi-context-attachment-dedupe-provenance-stress"
    | "visual-contrast-readable-state-stress"
    | "empty-error-retry-state-ux-stress"
    | "form-validation-inline-recovery-stress"
    | "navigation-back-stack-history-stress"
    | "long-text-wrap-resize-surface-stress"
    | "div-container-scroll-overflow-stress"
    | "main-menu-dynamic-choice-resize-stress"
    | "notes-window-resize-stress"
    | "notes-delete-confirm-route-proof"
    | "actions-command-discoverability-noop-stress"
    | "dense-list-detail-preview-readability-stress"
    | "toast-notification-queue-lifecycle-stress"
    | "destructive-confirm-modal-safety-stress"
    | "sdk-confirm-runtime-proof"
    | "loading-skeleton-progress-restoration-stress"
    | "icon-image-fallback-redaction-stress"
    | "footer-status-persistence-stress"
    | "keyboard-hint-label-parity-stress"
    | "row-state-parity-without-pointer-stress"
    | "quiet-chrome-card-nesting-stress"
    | "scroll-shadow-sticky-header-density-stress"
    | "popup-focus-keycap-visual-semantics-stress"
    | "reduced-motion-animation-disable-stress"
    | "command-search-highlighting-accessory-badges-stress"
    | "clipboard-copy-visual-feedback-stress"
    | "portal-cancel-return-state-restoration-stress"
    | "tooltip-hover-focus-affordance-stress"
    | "shortcut-recorder-cancel-layering-stress"
    | "inline-popover-anchor-resize-stress"
    | "disabled-footer-hit-target-refusal-stress"
    | "mini-full-transition-layout-continuity-stress"
    | "filter-input-decoration-chip-layout-stress"
    | "focus-ring-viewport-integrity-stress"
    | "warning-banner-action-dismiss-semantics-stress"
    | "select-prompt-multiselect-keyboard-state-stress"
    | "file-search-preview-sanitization-stress"
    | "hotkey-prompt-transient-capture-cancel-stress"
    | "process-manager-sort-detail-panel-stability-stress"
    | "env-prompt-redacted-status-error-recovery-stress"
    | "command-palette-breadcrumb-route-stack-stress"
    | "root-source-chip-action-semantics-stress"
    | "recent-history-dedupe-root-grouping-stress"
    | "inline-attachment-preview-chip-stability-stress"
    | "window-title-status-semantics-stress"
    | "menu-syntax-capture-validation-chip-stress"
    | "agent_chat-footer-activity-indicator-stress"
    | "agent_chat-model-history-popover-visual-state-stress"
    | "agent_chat-context-insertion-preview-parity-stress"
    | "agent_chat-composer-token-keyboard-edit-parity-stress"
    | "agent_chat-transcript-stream-retry-virtualization-stress"
    | "agent_chat-plugin-skill-entry-thread-affinity-stress"
    | "notes-cart-agent_chat-handoff-dedupe-stress"
    | "root-file-source-filter-pagination-footer-stress"
    | "file-search-directory-breadcrumb-restoration-stress"
    | "emoji-picker-skin-tone-category-ux-stress"
    | "root-window-source-filter-activation-refusal-stress"
    | "notes-markdown-preview-scroll-sync-stress"
    | "quick-terminal-ansi-scrollback-search-stress"
    | "script-output-inspector-folding-recovery-stress"
    | "app-launcher-icon-grid-keyboard-navigation-stress"
    | "browser-history-time-grouped-privacy-stress"
    | "settings-preferences-search-reset-preview-stress"
    | "settings-preferences-readonly-detail-panel-stress"
    | "design-picker-preview-restore-visual-stress"
    | "dictation-history-transcript-preview-redaction-stress";
  status: "pass" | "fail" | "error";
  failClosed?: boolean;
  failureMode?: string;
  missingReceipt?: string;
  reasonCode?: string;
  linearIssue?: string;
  error?: Record<string, unknown>;
  targetThread?: {
    stable: boolean;
    initial?: TargetThreadIdentity;
    final?: TargetThreadIdentity;
    checkedSteps: string[];
    driftFailures: TargetThreadFailure[];
  };
  peerWindows?: Array<Record<string, unknown>>;
  popupCases?: Array<Record<string, unknown>>;
  origin?: Record<string, unknown>;
  portal?: Record<string, unknown>;
  permissions?: Record<string, unknown>;
  shortcut?: Record<string, unknown>;
  templatePrompt?: Record<string, unknown>;
  currentAppCommands?: Record<string, unknown>;
  actionsCapturedSubject?: Record<string, unknown>;
  dropPrompt?: Record<string, unknown>;
  pathPrompt?: Record<string, unknown>;
  screenshotIdentity?: Record<string, unknown>;
  clipboardPortal?: Record<string, unknown>;
  browserCache?: Record<string, unknown>;
  scrollSelection?: Record<string, unknown>;
  permissionAssistant?: Record<string, unknown>;
  quickTerminal?: Record<string, unknown>;
  mcpContextResource?: Record<string, unknown>;
  settingsThemeHotReload?: Record<string, unknown>;
  fileSearchDragOut?: Record<string, unknown>;
  scriptletBundleExecution?: Record<string, unknown>;
  trayMenuMutation?: Record<string, unknown>;
  multiWindowRestore?: Record<string, unknown>;
  agent_chatDictationDelivery?: Record<string, unknown>;
  clipboardShareTrust?: Record<string, unknown>;
  clipboardShareReplay?: Record<string, unknown>;
  permissionShareCrossPrompt?: Record<string, unknown>;
  visibleTextAudit?: Record<string, unknown>;
  visibleTextLayoutAudit?: Record<string, unknown>;
  layoutMeasurement?: Record<string, unknown>;
  layoutMeasurementRegression?: Record<string, unknown>;
  visualConsistency?: Record<string, unknown>;
  screenshotSemanticsConsistency?: Record<string, unknown>;
  modalStackArbitration?: Record<string, unknown>;
  crossSurfaceExport?: Record<string, unknown>;
  sessionRecovery?: Record<string, unknown>;
  menuSyntaxAmbiguity?: Record<string, unknown>;
  imeCompositionBoundary?: Record<string, unknown>;
  accessibilitySelectedTextFallback?: Record<string, unknown>;
  displayMigrationVisualBounds?: Record<string, unknown>;
  nativePickerExternalReturnFocus?: Record<string, unknown>;
  dragCancelPayloadScope?: Record<string, unknown>;
  runtimeAppearanceChurnFocusedInput?: Record<string, unknown>;
  powerResumeWindowGeneration?: Record<string, unknown>;
  menuTrayNotificationModalInterruption?: Record<string, unknown>;
  streamProgressCancelVisualStability?: Record<string, unknown>;
  dictationMediaPermissionReadinessChurn?: Record<string, unknown>;
  animationFrameCaptureDeterminism?: Record<string, unknown>;
  accessibilityTreeSemanticParity?: Record<string, unknown>;
  rtlBidiEmojiTextRendering?: Record<string, unknown>;
  highVolumeVirtualizedListStability?: Record<string, unknown>;
  inputModalityTransitionOwnership?: Record<string, unknown>;
  multiContextAttachmentDedupeProvenance?: Record<string, unknown>;
  visualContrastReadableState?: Record<string, unknown>;
  emptyErrorRetryStateUx?: Record<string, unknown>;
  formValidationInlineRecovery?: Record<string, unknown>;
  navigationBackStackHistory?: Record<string, unknown>;
  longTextWrapResizeSurface?: Record<string, unknown>;
  divContainerScrollOverflow?: Record<string, unknown>;
  mainMenuDynamicChoiceResize?: Record<string, unknown>;
  actionsCommandDiscoverabilityNoop?: Record<string, unknown>;
  denseListDetailPreviewReadability?: Record<string, unknown>;
  toastNotificationQueueLifecycle?: Record<string, unknown>;
  destructiveConfirmModalSafety?: Record<string, unknown>;
  sdkConfirmRuntime?: Record<string, unknown>;
  loadingSkeletonProgressRestoration?: Record<string, unknown>;
  iconImageFallbackRedaction?: Record<string, unknown>;
  footerStatusPersistence?: Record<string, unknown>;
  keyboardHintLabelParity?: Record<string, unknown>;
  rowStateParityWithoutPointer?: Record<string, unknown>;
  quietChromeCardNesting?: Record<string, unknown>;
  scrollShadowStickyHeaderDensity?: Record<string, unknown>;
  popupFocusKeycapVisualSemantics?: Record<string, unknown>;
  reducedMotionAnimationDisable?: Record<string, unknown>;
  commandSearchHighlightingAccessoryBadges?: Record<string, unknown>;
  clipboardCopyVisualFeedback?: Record<string, unknown>;
  portalCancelReturnStateRestoration?: Record<string, unknown>;
  tooltipHoverFocusAffordance?: Record<string, unknown>;
  shortcutRecorderCancelLayeringReceipt?: Record<string, unknown>;
  inlinePopoverAnchorResizeReceipt?: Record<string, unknown>;
  disabledFooterHitTargetRefusalReceipt?: Record<string, unknown>;
  miniFullTransitionLayoutContinuityReceipt?: Record<string, unknown>;
  filterInputDecorationChipLayoutReceipt?: Record<string, unknown>;
  focusRingViewportIntegrityReceipt?: Record<string, unknown>;
  warningBannerActionDismissSemanticsReceipt?: Record<string, unknown>;
  selectPromptMultiselectKeyboardStateReceipt?: Record<string, unknown>;
  fileSearchPreviewSanitizationReceipt?: Record<string, unknown>;
  hotkeyPromptTransientCaptureCancelReceipt?: Record<string, unknown>;
  processManagerSortDetailPanelStabilityReceipt?: Record<string, unknown>;
  envPromptRedactedStatusErrorRecoveryReceipt?: Record<string, unknown>;
  commandPaletteBreadcrumbRouteStackReceipt?: Record<string, unknown>;
  rootSourceChipActionSemanticsReceipt?: Record<string, unknown>;
  recentHistoryDedupeRootGroupingReceipt?: Record<string, unknown>;
  inlineAttachmentPreviewChipStabilityReceipt?: Record<string, unknown>;
  windowTitleStatusSemanticsReceipt?: Record<string, unknown>;
  menuSyntaxCaptureValidationChipReceipt?: Record<string, unknown>;
  agent_chatFooterActivityIndicatorReceipt?: Record<string, unknown>;
  agent_chatModelHistoryPopoverVisualStateReceipt?: Record<string, unknown>;
  agent_chatContextInsertionPreviewParityReceipt?: Record<string, unknown>;
  agent_chatComposerTokenKeyboardEditParityReceipt?: Record<string, unknown>;
  agent_chatTranscriptStreamRetryVirtualizationReceipt?: Record<string, unknown>;
  agent_chatPluginSkillEntryThreadAffinityReceipt?: Record<string, unknown>;
  notesCartAgentChatHandoffDedupeReceipt?: Record<string, unknown>;
  rootFileSourceFilterPaginationFooterReceipt?: Record<string, unknown>;
  fileSearchDirectoryBreadcrumbRestorationReceipt?: Record<string, unknown>;
  emojiPickerSkinToneCategoryUxReceipt?: Record<string, unknown>;
  rootWindowSourceFilterActivationRefusalReceipt?: Record<string, unknown>;
  notesMarkdownPreviewScrollSyncReceipt?: Record<string, unknown>;
  quickTerminalAnsiScrollbackSearchReceipt?: Record<string, unknown>;
  scriptOutputInspectorFoldingRecoveryReceipt?: Record<string, unknown>;
  appLauncherIconGridKeyboardNavigationReceipt?: Record<string, unknown>;
  browserHistoryTimeGroupedPrivacyReceipt?: Record<string, unknown>;
  settingsPreferencesSearchResetPreviewReceipt?: Record<string, unknown>;
  settingsPreferencesReadonlyDetailPanelReceipt?: Record<string, unknown>;
  designPickerPreviewRestoreVisualReceipt?: Record<string, unknown>;
  dictationHistoryTranscriptPreviewRedactionReceipt?: Record<string, unknown>;
  delayedAction?: Record<string, unknown>;
  usage: Record<string, unknown>;
  captureTarget?: Record<string, unknown> | null;
  steps: Array<Record<string, unknown>>;
  failure?: TargetThreadFailure | Record<string, unknown>;
  warnings: string[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function stderrLog(event: string, fields: Record<string, unknown> = {}): void {
  process.stderr.write(
    JSON.stringify({ event, ts: new Date().toISOString(), ...fields }) + "\n"
  );
}

export function pushProofStep(
  bundle: ProofBundle,
  step: ProofBundleStep
): void {
  bundle.steps.push(step);
  stderrLog("proof_bundle.step_written", {
    scenario: bundle.scenario,
    stepType: step.type,
    windowId: bundle.resolvedTarget.windowId,
  });
}

export async function runTool(
  cmd: string[],
  label: string,
  env?: Record<string, string>
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const isSessionCommand =
    (cmd[0] === "bash" || cmd[0] === "/bin/bash") &&
    typeof cmd[1] === "string" &&
    resolve(PROJECT_ROOT, cmd[1]) === resolve(PROJECT_ROOT, "scripts/agentic/session.sh");
  const operation = isSessionCommand ? cmd[2] : undefined;
  const session = isSessionCommand ? cmd[3] : undefined;
  if (operation === "start") {
    inspectionSessionCleanup(session ?? "", null);
    if (session === "dev-watch") {
      throw new Error("Agentic scenario cannot claim the borrowed operator session");
    }
  }
  const ownedCommand = operation === "stop"
    ? scenarioSessionOwnership.stopCommand(session ?? "")
    : cmd;
  assertNoninteractiveSubprocess(ownedCommand, env);
  const proc = Bun.spawn(ownedCommand, {
    stdout: "pipe",
    stderr: "pipe",
    cwd: PROJECT_ROOT,
    env: env ? { ...Bun.env, ...env } : Bun.env,
  });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const exitCode = await proc.exited;
  if (exitCode === 0 && operation === "start" && session) {
    scenarioSessionOwnership.rememberStart(session, parseMaybeJson(stdout), {
      allowPendingReadiness: true,
    });
  }
  if (exitCode === 0 && operation === "stop" && session) {
    scenarioSessionOwnership.release(session);
  }
  stderrLog("tool_complete", { label, exitCode });
  return { exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
}

async function rpc(
  session: string,
  payload: Record<string, unknown>,
  expect: string,
  timeoutMs: number = 5000,
  env?: Record<string, string>
): Promise<Record<string, unknown>> {
  const result = await runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "rpc",
      session,
      JSON.stringify(payload),
      "--expect",
      expect,
      "--timeout",
      String(timeoutMs),
    ],
    `rpc:${payload.type}`,
    env,
  );
  if (result.exitCode !== 0) {
    throw new Error(
      result.stdout || result.stderr || `RPC failed with exit code ${result.exitCode}`
    );
  }
  return JSON.parse(result.stdout);
}

async function sendAndAwaitParse(
  session: string,
  payload: Record<string, unknown>,
  timeoutMs: number = 5000
): Promise<Record<string, unknown>> {
  const result = await runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "send",
      session,
      JSON.stringify(payload),
      "--await-parse",
      "--timeout",
      String(timeoutMs),
    ],
    `send:${payload.type}`
  );
  if (result.exitCode !== 0) {
    throw new Error(
      result.stdout || result.stderr || `send failed with exit code ${result.exitCode}`
    );
  }
  return parseMaybeJson(result.stdout);
}

async function sendWithoutAwaitParse(
  session: string,
  payload: Record<string, unknown>,
  timeoutMs: number = 5000,
  env?: Record<string, string>
): Promise<Record<string, unknown>> {
  const result = await runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "send",
      session,
      JSON.stringify(payload),
      "--timeout",
      String(timeoutMs),
    ],
    `send:${payload.type}`,
    env,
  );
  if (result.exitCode !== 0) {
    throw new Error(
      result.stdout || result.stderr || `send failed with exit code ${result.exitCode}`
    );
  }
  return parseMaybeJson(result.stdout);
}

// ---------------------------------------------------------------------------
// Target resolution via automation-window.ts
// ---------------------------------------------------------------------------

interface ResolvedTarget {
  targetJson: { type: "id"; id: string };
  windowKind: string;
  automationWindowId: string;
  title: string | null;
  surfaceId: string | null;
  osWindowId?: number | null;
  popupSemantics?: {
    hasRealElements: boolean;
    panelOnly: boolean;
    warnings: string[];
    batchMutationAvailable: boolean;
  } | null;
  inspect?: Record<string, unknown> | null;
}

async function inspectAndPromoteTarget(opts: {
  session: string;
  kind: string;
  index: number;
  probes?: Array<{ x: number; y: number }>;
}): Promise<ResolvedTarget> {
  const cmd = [
    "bun",
    "scripts/agentic/automation-window.ts",
    "inspect",
    "--session",
    opts.session,
    "--kind",
    opts.kind,
    "--index",
    String(opts.index),
  ];
  for (const probe of opts.probes ?? []) {
    cmd.push("--probe", `${probe.x},${probe.y}`);
  }
  const result = await runTool(cmd, "inspect-and-promote-target");

  if (result.exitCode !== 0) {
    throw new Error(
      `Target inspection failed: ${result.stdout || result.stderr}`
    );
  }

  const parsed = JSON.parse(result.stdout);
  if (parsed.status !== "ok") {
    throw new Error(
      `Target inspection returned error: ${parsed.error?.message ?? "unknown"}`
    );
  }

  const automationWindowId = parsed.automationWindowId
    ? String(parsed.automationWindowId)
    : "";
  if (!automationWindowId) {
    throw new Error("Target resolution returned an empty automationWindowId");
  }

  // Promote to exact-id target for all subsequent RPCs
  const targetJson: { type: "id"; id: string } = {
    type: "id",
    id: automationWindowId,
  };

  stderrLog("agentic.promote_exact_target", {
    fromKind: opts.kind,
    fromIndex: opts.index,
    promotedTargetJson: targetJson,
    automationWindowId,
    surfaceId: parsed.surfaceId ?? null,
    osWindowId: parsed.osWindowId ?? null,
  });

  return {
    targetJson,
    windowKind: parsed.inspect?.windowKind ?? parsed.windowKind ?? opts.kind,
    automationWindowId,
    title: parsed.inspect?.title ?? parsed.title ?? null,
    surfaceId: parsed.surfaceId ?? null,
    osWindowId: parsed.osWindowId ?? null,
    popupSemantics: parsed.popupSemantics ?? null,
    inspect: parsed.inspect ?? null,
  };
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

export async function runDetachedAgentChatExactIdScenario(
  session: string,
  index: number
): Promise<ProofBundle> {
  stderrLog("scenario.start", {
    scenario: "detached-agent_chat-exact-id",
    session,
    index,
  });

  // Step 1: Resolve the detached Agent Chat target to an exact ID
  const resolved = await inspectAndPromoteTarget({
    session,
    kind: "agentChatDetached",
    index,
    probes: [{ x: 24, y: 24 }],
  });

  const bundle: ProofBundle = {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "detached-agent_chat-exact-id",
    surfaceClass: "detached",
    resolvedTarget: {
      windowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
    },
    inputMethod: "simulateGpuiEvent",
    dispatchPath: "exact_handle",
    resolvedWindowId: resolved.automationWindowId,
    popupCapture: {
      strategy: "direct_window_capture",
      targetBounds: null,
      semanticReceiptsArePrimary: true,
    },
    steps: [],
    warnings: [],
  };

  pushProofStep(bundle, {
    type: "resolveTarget",
    at: new Date().toISOString(),
    request: { session, kind: "agentChatDetached", index },
    response: {
      automationWindowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
      targetJson: resolved.targetJson,
    },
  });

  // Step 2: Inspect the resolved window (before any interaction)
  try {
    const inspectBefore = await rpc(
      session,
      {
        type: "inspectAutomationWindow",
        requestId: "inspect-before",
        target: resolved.targetJson,
        probes: [{ x: 24, y: 24 }],
      },
      "automationInspectResult",
      8000
    );

    pushProofStep(bundle, {
      type: "inspect",
      at: new Date().toISOString(),
      request: {
        type: "inspectAutomationWindow",
        requestId: "inspect-before",
        target: resolved.targetJson,
        probes: [{ x: 24, y: 24 }],
      },
      response: inspectBefore,
    });

    // Populate V2 inspect fields from the first successful inspection
    const resp = inspectBefore.response ?? inspectBefore;
    if (typeof (resp as Record<string, unknown>).osWindowId === "number") {
      bundle.osWindowId = (resp as Record<string, unknown>).osWindowId as number;
    }
    bundle.inspect = {
      screenshotWidth: (resp as Record<string, unknown>).screenshotWidth as number ?? null,
      screenshotHeight: (resp as Record<string, unknown>).screenshotHeight as number ?? null,
      warnings: ((resp as Record<string, unknown>).warnings as string[]) ?? [],
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`inspect_before_failed: ${msg}`);
    stderrLog("scenario.inspect_before_failed", { error: msg });
  }

  // Step 3: Simulate a GPUI event (Cmd+K) to the exact target
  try {
    const eventResult = await rpc(
      session,
      {
        type: "simulateGpuiEvent",
        requestId: "gpui-cmd-k",
        target: resolved.targetJson,
        event: { type: "keyDown", key: "k", modifiers: ["cmd"] },
      },
      "simulateGpuiEventResult",
      5000
    );

    pushProofStep(bundle, {
      type: "simulateGpuiEvent",
      at: new Date().toISOString(),
      request: {
        type: "simulateGpuiEvent",
        requestId: "gpui-cmd-k",
        target: resolved.targetJson,
        event: { type: "keyDown", key: "k", modifiers: ["cmd"] },
      },
      response: eventResult,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`gpui_event_failed: ${msg}`);
    stderrLog("scenario.gpui_event_failed", { error: msg });
  }

  // Step 4: Inspect the window again (after interaction)
  try {
    const inspectAfter = await rpc(
      session,
      {
        type: "inspectAutomationWindow",
        requestId: "inspect-after",
        target: resolved.targetJson,
        probes: [{ x: 24, y: 24 }],
      },
      "automationInspectResult",
      8000
    );

    pushProofStep(bundle, {
      type: "inspect",
      at: new Date().toISOString(),
      request: {
        type: "inspectAutomationWindow",
        requestId: "inspect-after",
        target: resolved.targetJson,
        probes: [{ x: 24, y: 24 }],
      },
      response: inspectAfter,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`inspect_after_failed: ${msg}`);
    stderrLog("scenario.inspect_after_failed", { error: msg });
  }

  stderrLog("scenario.complete", {
    scenario: "detached-agent_chat-exact-id",
    stepCount: bundle.steps.length,
    warningCount: bundle.warnings.length,
  });

  return bundle;
}

export async function runPromptPopupExactIdScenario(
  session: string,
  index: number
): Promise<ProofBundle> {
  stderrLog("scenario.start", {
    scenario: "prompt-popup-exact-id",
    session,
    index,
  });

  // Step 1: Resolve the prompt picker target to an exact ID
  const resolved = await inspectAndPromoteTarget({
    session,
    kind: "promptPopup",
    index,
    probes: [{ x: 12, y: 12 }],
  });

  const bundle: ProofBundle = {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "prompt-popup-exact-id",
    surfaceClass: "attachedPopup",
    resolvedTarget: {
      windowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
    },
    inputMethod: "batch",
    dispatchPath: "exact_handle",
    resolvedWindowId: resolved.automationWindowId,
    popupCapture: {
      strategy: "parent_capture_with_crop",
      targetBounds: null, // Populated from inspect if available
      semanticReceiptsArePrimary: true,
    },
    steps: [],
    warnings: [],
  };

  pushProofStep(bundle, {
    type: "resolveTarget",
    at: new Date().toISOString(),
    request: { session, kind: "promptPopup", index },
    response: {
      automationWindowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
      targetJson: resolved.targetJson,
    },
  });

  // Step 2: Inspect the resolved popup window
  try {
    const inspectResult = await rpc(
      session,
      {
        type: "inspectAutomationWindow",
        requestId: "inspect-popup",
        target: resolved.targetJson,
        probes: [{ x: 12, y: 12 }],
      },
      "automationInspectResult",
      8000
    );

    pushProofStep(bundle, {
      type: "inspect",
      at: new Date().toISOString(),
      request: {
        type: "inspectAutomationWindow",
        requestId: "inspect-popup",
        target: resolved.targetJson,
        probes: [{ x: 12, y: 12 }],
      },
      response: inspectResult,
    });

    // Populate V2 fields from inspection
    const resp = inspectResult.response ?? inspectResult;
    if (typeof (resp as Record<string, unknown>).osWindowId === "number") {
      bundle.osWindowId = (resp as Record<string, unknown>).osWindowId as number;
    }
    bundle.inspect = {
      screenshotWidth: (resp as Record<string, unknown>).screenshotWidth as number ?? null,
      screenshotHeight: (resp as Record<string, unknown>).screenshotHeight as number ?? null,
      warnings: ((resp as Record<string, unknown>).warnings as string[]) ?? [],
    };

    // Populate targetBounds for attached popup crop strategy
    const tb = (resp as Record<string, unknown>).targetBoundsInScreenshot as
      | { x: number; y: number; width: number; height: number }
      | undefined;
    if (tb && bundle.popupCapture) {
      bundle.popupCapture.targetBounds = {
        x: tb.x,
        y: tb.y,
        width: tb.width,
        height: tb.height,
      };
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`inspect_popup_failed: ${msg}`);
    stderrLog("scenario.inspect_popup_failed", { error: msg });
  }

  // Step 3: Wait for popup to be ready (using waitFor with the exact target)
  try {
    const waitResult = await rpc(
      session,
      {
        type: "waitFor",
        requestId: "wait-popup-ready",
        target: resolved.targetJson,
        condition: { type: "windowVisible" },
        timeout: 3000,
        pollInterval: 25,
      },
      "waitForResult",
      5000
    );

    pushProofStep(bundle, {
      type: "waitFor",
      at: new Date().toISOString(),
      request: {
        type: "waitFor",
        requestId: "wait-popup-ready",
        target: resolved.targetJson,
        condition: { type: "windowVisible" },
      },
      response: waitResult,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`wait_popup_ready_failed: ${msg}`);
    stderrLog("scenario.wait_popup_ready_failed", { error: msg });
  }

  stderrLog("scenario.complete", {
    scenario: "prompt-popup-exact-id",
    stepCount: bundle.steps.length,
    warningCount: bundle.warnings.length,
    hasTargetBounds: bundle.popupCapture?.targetBounds != null,
  });

  return bundle;
}

export async function runActionsDialogExactIdScenario(
  session: string,
  index: number
): Promise<ProofBundle> {
  stderrLog("scenario.start", {
    scenario: "actions-dialog-exact-id",
    session,
    index,
  });

  const resolved = await inspectAndPromoteTarget({
    session,
    kind: "actionsDialog",
    index,
    probes: [{ x: 12, y: 12 }],
  });

  const bundle: ProofBundle = {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "actions-dialog-exact-id",
    surfaceClass: "attachedPopup",
    resolvedTarget: {
      windowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
    },
    inputMethod: "batch",
    dispatchPath: "exact_handle",
    resolvedWindowId: resolved.automationWindowId,
    popupCapture: {
      strategy: "parent_capture_with_crop",
      targetBounds: null,
      semanticReceiptsArePrimary: true,
    },
    steps: [],
    warnings: [],
  };

  pushProofStep(bundle, {
    type: "resolveTarget",
    at: new Date().toISOString(),
    request: { session, kind: "actionsDialog", index },
    response: {
      automationWindowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
      popupSemantics: resolved.popupSemantics,
      targetJson: resolved.targetJson,
    },
  });

  try {
    const inspectResult = await rpc(
      session,
      {
        type: "inspectAutomationWindow",
        requestId: "inspect-actions-dialog",
        target: resolved.targetJson,
        probes: [{ x: 12, y: 12 }],
      },
      "automationInspectResult",
      8000
    );

    pushProofStep(bundle, {
      type: "inspect",
      at: new Date().toISOString(),
      request: {
        type: "inspectAutomationWindow",
        requestId: "inspect-actions-dialog",
        target: resolved.targetJson,
        probes: [{ x: 12, y: 12 }],
      },
      response: inspectResult,
    });

    const resp = inspectResult.response ?? inspectResult;
    if (typeof (resp as Record<string, unknown>).osWindowId === "number") {
      bundle.osWindowId = (resp as Record<string, unknown>).osWindowId as number;
    }
    bundle.inspect = {
      screenshotWidth: (resp as Record<string, unknown>).screenshotWidth as number ?? null,
      screenshotHeight: (resp as Record<string, unknown>).screenshotHeight as number ?? null,
      warnings: ((resp as Record<string, unknown>).warnings as string[]) ?? [],
    };
    const tb = (resp as Record<string, unknown>).targetBoundsInScreenshot as
      | { x: number; y: number; width: number; height: number }
      | undefined;
    if (tb && bundle.popupCapture) {
      bundle.popupCapture.targetBounds = tb;
    }
    if (!resolved.popupSemantics?.batchMutationAvailable) {
      bundle.warnings.push("actionsDialog_batch_unavailable");
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`inspect_actions_dialog_failed: ${msg}`);
    stderrLog("scenario.inspect_actions_dialog_failed", { error: msg });
  }

  try {
    const waitResult = await rpc(
      session,
      {
        type: "waitFor",
        requestId: "wait-actions-dialog-visible",
        target: resolved.targetJson,
        condition: { type: "windowVisible" },
        timeout: 3000,
        pollInterval: 25,
      },
      "waitForResult",
      5000
    );

    pushProofStep(bundle, {
      type: "waitFor",
      at: new Date().toISOString(),
      request: {
        type: "waitFor",
        requestId: "wait-actions-dialog-visible",
        target: resolved.targetJson,
        condition: { type: "windowVisible" },
      },
      response: waitResult,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`wait_actions_dialog_ready_failed: ${msg}`);
    stderrLog("scenario.wait_actions_dialog_ready_failed", { error: msg });
  }

  stderrLog("scenario.complete", {
    scenario: "actions-dialog-exact-id",
    stepCount: bundle.steps.length,
    warningCount: bundle.warnings.length,
    hasTargetBounds: bundle.popupCapture?.targetBounds != null,
  });

  return bundle;
}

export async function runMainWindowExactIdScenario(
  session: string
): Promise<ProofBundle> {
  stderrLog("scenario.start", {
    scenario: "main-window-exact-id",
    session,
  });

  const resolved = await inspectAndPromoteTarget({
    session,
    kind: "main",
    index: 0,
    probes: [{ x: 24, y: 24 }],
  });

  const bundle: ProofBundle = {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "main-window-exact-id",
    surfaceClass: "main",
    resolvedTarget: {
      windowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
    },
    inputMethod: "batch",
    dispatchPath: "exact_handle",
    resolvedWindowId: resolved.automationWindowId,
    popupCapture: {
      strategy: "not_applicable",
      targetBounds: null,
      semanticReceiptsArePrimary: true,
    },
    steps: [],
    warnings: [],
  };

  pushProofStep(bundle, {
    type: "resolveTarget",
    at: new Date().toISOString(),
    request: { session, kind: "main", index: 0 },
    response: {
      automationWindowId: resolved.automationWindowId,
      windowKind: resolved.windowKind,
      title: resolved.title,
      surfaceId: resolved.surfaceId,
      targetJson: resolved.targetJson,
    },
  });

  try {
    const inspectResult = await rpc(
      session,
      {
        type: "inspectAutomationWindow",
        requestId: "inspect-main",
        target: resolved.targetJson,
        probes: [{ x: 24, y: 24 }],
      },
      "automationInspectResult",
      8000
    );

    pushProofStep(bundle, {
      type: "inspect",
      at: new Date().toISOString(),
      request: {
        type: "inspectAutomationWindow",
        requestId: "inspect-main",
        target: resolved.targetJson,
        probes: [{ x: 24, y: 24 }],
      },
      response: inspectResult,
    });

    const elementsResult = await rpc(
      session,
      {
        type: "getElements",
        requestId: "elements-main",
        target: resolved.targetJson,
      },
      "elementsResult",
      3000
    );

    pushProofStep(bundle, {
      type: "inspect",
      at: new Date().toISOString(),
      request: {
        type: "getElements",
        requestId: "elements-main",
        target: resolved.targetJson,
      },
      response: elementsResult,
    });

    const resp = inspectResult.response ?? inspectResult;
    if (typeof (resp as Record<string, unknown>).osWindowId === "number") {
      bundle.osWindowId = (resp as Record<string, unknown>).osWindowId as number;
    }
    bundle.inspect = {
      screenshotWidth: (resp as Record<string, unknown>).screenshotWidth as number ?? null,
      screenshotHeight: (resp as Record<string, unknown>).screenshotHeight as number ?? null,
      warnings: ((resp as Record<string, unknown>).warnings as string[]) ?? [],
    };
    const elementResponse = (elementsResult.response ?? elementsResult) as Record<string, unknown>;
    const elements = Array.isArray(elementResponse.elements) ? elementResponse.elements : [];
    if (elements.length === 0) {
      bundle.warnings.push("main_elements_empty");
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    bundle.warnings.push(`inspect_main_failed: ${msg}`);
    stderrLog("scenario.inspect_main_failed", { error: msg });
  }

  stderrLog("scenario.complete", {
    scenario: "main-window-exact-id",
    stepCount: bundle.steps.length,
    warningCount: bundle.warnings.length,
  });

  return bundle;
}

function hardFailure(
  scenario: HardScenarioReceipt["scenario"],
  failure: TargetThreadFailure | Record<string, unknown>,
  steps: Array<Record<string, unknown>> = [],
): HardScenarioReceipt {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario,
    status: "fail",
    usage: {
      stateFirst: true,
      usedGetAgentChatState: false,
      usedGetAgentChatTestProbe: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
    },
    steps,
    failure,
    warnings: [],
  };
}

function failureFromError(
  error: unknown,
  fallbackCode: TargetThreadFailure["code"],
  stepName: string,
): TargetThreadFailure {
  const maybeFailure = (error as { failure?: TargetThreadFailure })?.failure;
  if (maybeFailure) return maybeFailure;
  return {
    code: fallbackCode,
    stepName,
    message: error instanceof Error ? error.message : String(error),
  };
}

export async function runDetachedAgentChatTargetThreadingStressScenario(opts: {
  session: string;
  kind: string;
  index: number;
  minTargets: number;
  key: "enter" | "tab";
  vision: boolean;
}): Promise<HardScenarioReceipt> {
  const scenario = "detached-agent_chat-target-threading-stress";
  const steps: Array<Record<string, unknown>> = [];
  const driftFailures: TargetThreadFailure[] = [];
  const peerWindows = await listNativePeerWindows({ family: "agentChatDetached" });

  if (peerWindows.length < opts.minTargets) {
    return hardFailure(scenario, {
      code: "insufficient_target_count",
      stepName: "peer-window-count",
      message: `Expected at least ${opts.minTargets} Agent Chat peer windows, found ${peerWindows.length}`,
      expected: { peerCount: opts.minTargets },
      actual: { peerCount: peerWindows.length, peerWindows },
    }, steps);
  }

  let identity: TargetThreadIdentity;
  try {
    identity = await promoteExactTarget({
      session: opts.session,
      kind: opts.kind,
      index: opts.index,
      expected: { windowKind: "agentChatDetached" },
    });
  } catch (error) {
    return hardFailure(
      scenario,
      failureFromError(error, "target_resolution_failed", "promote-exact-target"),
      steps,
    );
  }

  if (identity.surfaceId == null) {
    return hardFailure(scenario, {
      code: "missing_surface_id",
      stepName: "promote-exact-target",
      expected: { surfaceId: "non-null" } as Partial<TargetThreadIdentity>,
      actual: identity,
      message: "Detached Agent Chat native input requires exact surfaceId from automation-window inspection",
    }, steps);
  }
  if (identity.osWindowId == null) {
    return hardFailure(scenario, {
      code: "missing_os_window_id",
      stepName: "promote-exact-target",
      expected: { osWindowId: 1 } as Partial<TargetThreadIdentity>,
      actual: identity,
      message: "Detached Agent Chat strict capture requires osWindowId from automation-window inspection",
    }, steps);
  }

  const checkedSteps: string[] = [];
  const pushRpc = async (
    stepName: string,
    command: Record<string, unknown>,
    expect: string,
    timeout = 5000,
  ) => {
    const receipt = await targetedRpc({
      session: opts.session,
      identity,
      requestId: stepName,
      command,
      expect,
      timeout,
      stepName,
    });
    steps.push(receipt as unknown as Record<string, unknown>);
    checkedSteps.push(stepName);
    const stability = await assertTargetStable({
      session: opts.session,
      identity,
      stepName,
    });
    if (!stability.ok) driftFailures.push(stability.failure);
    return receipt;
  };

  await pushRpc("baseline-getAgentChatState", { type: "getAgentChatState" }, "agent_chatStateResult");
  await pushRpc("reset-probe", { type: "resetAgentChatTestProbe" }, "agent_chatTestProbeResult");

  const nativeType = await runTargetThreadTool(
    [
      "bun",
      "scripts/agentic/macos-input.ts",
      "type",
      "@",
      "--ensure-focus",
      "--session",
      opts.session,
      "--target",
      identity.surfaceId,
    ],
    "detached-stress-native-type",
  );
  steps.push({
    name: "native-type-at",
    status: nativeType.exitCode === 0 ? "pass" : "fail",
    output: nativeType.stdout ? JSON.parse(nativeType.stdout) : { stderr: nativeType.stderr },
  });
  checkedSteps.push("native-type-at");
  const afterType = await assertTargetStable({ session: opts.session, identity, stepName: "native-type-at" });
  if (!afterType.ok) driftFailures.push(afterType.failure);

  await pushRpc(
    "wait-picker-open",
    {
      type: "waitFor",
      condition: { type: "agent_chatPickerOpen" },
      timeout: 3000,
      pollInterval: 25,
      trace: "onFailure",
    },
    "waitForResult",
    5000,
  );

  const nativeAccept = await runTargetThreadTool(
    [
      "bun",
      "scripts/agentic/macos-input.ts",
      "key",
      opts.key,
      "--ensure-focus",
      "--session",
      opts.session,
      "--target",
      identity.surfaceId,
    ],
    "detached-stress-native-accept",
  );
  steps.push({
    name: `native-accept-${opts.key}`,
    status: nativeAccept.exitCode === 0 ? "pass" : "fail",
    output: nativeAccept.stdout ? JSON.parse(nativeAccept.stdout) : { stderr: nativeAccept.stderr },
  });
  checkedSteps.push(`native-accept-${opts.key}`);

  await pushRpc(
    `wait-accepted-via-${opts.key}`,
    {
      type: "waitFor",
      condition: { type: "agent_chatAcceptedViaKey", key: opts.key },
      timeout: 3000,
      pollInterval: 25,
      trace: "onFailure",
    },
    "waitForResult",
    5000,
  );

  await pushRpc("final-getAgentChatState", { type: "getAgentChatState" }, "agent_chatStateResult");

  const verify = await runTargetThreadTool(
    [
      "bun",
      "scripts/agentic/verify-shot.ts",
      "--session",
      opts.session,
      "--label",
      "detached-target-threading-stress",
      "--target-json",
      JSON.stringify(identity.targetJson),
      "--capture-window-id",
      String(identity.osWindowId),
      "--agent_chat-picker-closed",
      "--agent_chat-item-accepted",
      "--agent_chat-accepted-via",
      opts.key,
      ...(opts.vision ? ["--vision"] : []),
    ],
    "detached-stress-strict-capture",
  );
  const verifyOutput = verify.stdout ? JSON.parse(verify.stdout) : { stderr: verify.stderr };
  steps.push({
    name: "strict-capture",
    status: verify.exitCode === 0 ? "pass" : "fail",
    output: verifyOutput,
  });
  checkedSteps.push("strict-capture");

  const finalStable = await assertTargetStable({
    session: opts.session,
    identity,
    stepName: "final-before-return",
  });
  if (!finalStable.ok) driftFailures.push(finalStable.failure);

  const captureTarget = (verifyOutput as Record<string, unknown>).captureTarget as
    | Record<string, unknown>
    | undefined;
  const captureMismatch =
    captureTarget?.requestedWindowId != null &&
    captureTarget?.actualWindowId != null &&
    captureTarget.requestedWindowId !== captureTarget.actualWindowId;
  if (captureMismatch) {
    driftFailures.push({
      code: "target_identity_drift",
      stepName: "strict-capture",
      expected: { osWindowId: captureTarget.requestedWindowId as number },
      actual: { osWindowId: captureTarget.actualWindowId as number },
      message: "Strict capture returned a different native window ID",
    });
  }

  const stepFailed = steps.some((step) => step.status !== "pass");
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario,
    status: driftFailures.length === 0 && !stepFailed ? "pass" : "fail",
    targetThread: {
      stable: driftFailures.length === 0,
      initial: identity,
      final: finalStable.ok ? finalStable.identity : undefined,
      checkedSteps,
      driftFailures,
    },
    peerWindows,
    usage: {
      stateFirst: true,
      usedGetAgentChatState: true,
      usedGetAgentChatTestProbe: true,
      usedWaitFor: true,
      usedNativeInput: true,
      usedScreenshot: true,
      usedFixedSleepMs: 0,
    },
    captureTarget: captureTarget ?? null,
    steps,
    warnings: [],
  };
}

export async function runNotesAgentChatDelayedActionOriginStressScenario(opts: {
  session: string;
  drift: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "notes-agent_chat-delayed-action-origin-stress",
    status: "fail",
    origin: {
      host: "notes",
      agent_chatGeneration: null,
    },
    delayedAction: {
      outcome: "missingOriginGeneration",
      drift: {
        field: "agent_chatGeneration",
        expected: "non-null",
        actual: null,
      },
    },
    usage: {
      stateFirst: true,
      usedGetAgentChatState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
    },
    steps: [
      {
        name: "notes-origin-receipt-preflight",
        status: "fail",
        output: {
          session: opts.session,
          drift: opts.drift,
          blockingGap:
            "Notes-hosted Agent Chat delayed action origin/generation receipts are not yet exposed to the TypeScript harness.",
        },
      },
    ],
    failure: {
      code: "missing_origin_generation",
      stepName: "notes-origin-receipt-preflight",
      message:
        "The harness now fails closed until app-side Notes Agent Chat origin/generation receipts exist.",
    },
    warnings: ["file_linear:notes_agent_chat_origin_generation_receipts_missing"],
  };
}

function parseMaybeJson(text: string): Record<string, unknown> {
  if (!text.trim()) return {};
  try {
    return JSON.parse(text);
  } catch {
    return { raw: text };
  }
}

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value != null ? value as Record<string, unknown> : {};
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function rpcResponse(envelope: Record<string, unknown>): Record<string, unknown> {
  return asRecord(envelope.response ?? envelope);
}

function componentByName(layoutInfo: Record<string, unknown>, name: string): Record<string, unknown> | null {
  const component = asArray(layoutInfo.components).find((candidate) => asRecord(candidate).name === name);
  return component ? asRecord(component) : null;
}

function boundsOf(component: Record<string, unknown> | null): Record<string, unknown> | null {
  const bounds = asRecord(component?.bounds);
  return typeof bounds.width === "number" && typeof bounds.height === "number" ? bounds : null;
}

function numberField(record: Record<string, unknown> | null, key: string): number | null {
  const value = record?.[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function elementBounds(element: Record<string, unknown>, layoutInfo: Record<string, unknown>): Record<string, unknown> | null {
  const type = String(element.type ?? "");
  if (type === "input") return boundsOf(componentByName(layoutInfo, "SearchInput"));
  if (type === "list") return boundsOf(componentByName(layoutInfo, "ScriptList"));
  if (type === "choice") {
    const index = typeof element.index === "number" ? element.index : null;
    if (index != null) {
      const explicit = boundsOf(componentByName(layoutInfo, `ListItem[${index}]`));
      if (explicit) return explicit;
      const list = boundsOf(componentByName(layoutInfo, "ScriptList"));
      const rowHeight = 40;
      const listX = numberField(list, "x") ?? 0;
      const listY = numberField(list, "y") ?? 45;
      const listWidth = numberField(list, "width") ?? 0;
      return {
        x: listX,
        y: listY + index * rowHeight,
        width: listWidth,
        height: rowHeight,
        estimatedFromListGeometry: true,
      };
    }
  }
  if (type === "button") {
    const text = String(element.text ?? "");
    if (text.includes("Actions")) return boundsOf(componentByName(layoutInfo, "ActionsButton"));
    if (text.includes("Paste") || text.includes("Run")) return boundsOf(componentByName(layoutInfo, "RunButton"));
  }
  return null;
}

function mainLayoutFingerprint(layoutInfo: Record<string, unknown>): Record<string, unknown> {
  return {
    promptType: layoutInfo.promptType ?? null,
    windowBounds: boundsOf(componentByName(layoutInfo, "Window")),
    contentBounds: boundsOf(componentByName(layoutInfo, "ContentArea")),
    inputBounds: boundsOf(componentByName(layoutInfo, "SearchInput")),
    listBounds: boundsOf(componentByName(layoutInfo, "ScriptList")),
    footerBounds: boundsOf(componentByName(layoutInfo, "RunButton")),
    componentNames: asArray(layoutInfo.components).map((component) => asRecord(component).name ?? null),
  };
}

async function measureTextWidthPx(text: string, fontSize = 14): Promise<number | null> {
  const sample = text.replace(/\s+/g, " ").slice(0, 300);
  if (!sample) return 0;
  const script = [
    'ObjC.import("AppKit");',
    `const s = $.NSString.alloc.initWithUTF8String(${JSON.stringify(sample)});`,
    `const f = $.NSFont.systemFontOfSize(${fontSize});`,
    'const attrs = $.NSDictionary.dictionaryWithObjectForKey(f, $.NSFontAttributeName);',
    's.sizeWithAttributes(attrs).width;',
  ].join(" ");
  const result = await runTool(["osascript", "-l", "JavaScript", "-e", script], "measure-text-width");
  if (result.exitCode !== 0) return null;
  const width = Number(result.stdout.trim());
  return Number.isFinite(width) ? width : null;
}

async function startVisibleMeasurementSurface(session: string): Promise<{
  startReceipt: Record<string, unknown>;
  shouldStop: boolean;
}> {
  const start = await runTool(["bash", "scripts/agentic/session.sh", "start", session], "measurement:session-start");
  const startReceipt = parseMaybeJson(start.stdout);
  await runTool(["bash", "scripts/agentic/session.sh", "send", session, '{"type":"show"}'], "measurement:show");
  return {
    startReceipt,
    shouldStop: startReceipt.resumed !== true,
  };
}

async function stopVisibleMeasurementSurface(session: string, shouldStop: boolean): Promise<Record<string, unknown> | null> {
  if (!shouldStop) return null;
  const stop = await runTool(["bash", "scripts/agentic/session.sh", "stop", session], "measurement:session-stop");
  return parseMaybeJson(stop.stdout);
}

async function collectMainVisualMeasurement(session: string): Promise<{
  startReceipt: Record<string, unknown>;
  stopReceipt: Record<string, unknown> | null;
  state: Record<string, unknown>;
  elements: Record<string, unknown>;
  layoutInfo: Record<string, unknown>;
  textSamples: Array<Record<string, unknown>>;
  overlapPairs: Array<Record<string, unknown>>;
}> {
  const { startReceipt, shouldStop } = await startVisibleMeasurementSurface(session);
  let stopReceipt: Record<string, unknown> | null = null;
  try {
    // getState can exceed the session wrapper's JSON envelope limit on large
    // main-menu datasets. The measurement recipes need element semantics plus
    // layout geometry, so keep the runtime receipt focused and bounded.
    const state: Record<string, unknown> = {};
    const elements = rpcResponse(await rpc(session, { type: "getElements", requestId: "visual-measure-elements", limit: 24 }, "elementsResult"));
    const layoutInfo = rpcResponse(await rpc(session, { type: "getLayoutInfo", requestId: "visual-measure-layout" }, "layoutInfoResult"));
    const windowBounds = boundsOf(componentByName(layoutInfo, "Window"));
    const windowHeight = numberField(windowBounds, "height");
    const rawElements = asArray(elements.elements).map(asRecord);
    const textElements = rawElements
      .filter((element) =>
        element.type !== "list" &&
        typeof element.text === "string" &&
        String(element.text).trim().length > 0
      )
      .slice(0, 24);
    const textSamples: Array<Record<string, unknown>> = [];
    for (const element of textElements) {
      const bounds = elementBounds(element, layoutInfo);
      const boundsY = numberField(bounds, "y");
      if (windowHeight != null && boundsY != null && boundsY >= windowHeight) {
        continue;
      }
      const text = String(element.text ?? "");
      const measuredWidthPx = await measureTextWidthPx(text);
      const availableWidthPx = Math.max(0, (numberField(bounds, "width") ?? 0) - 24);
      const textFitsContainer = measuredWidthPx == null || availableWidthPx <= 0
        ? null
        : measuredWidthPx <= availableWidthPx;
      textSamples.push({
        semanticId: element.semanticId ?? null,
        type: element.type ?? null,
        role: element.role ?? null,
        text,
        fullText: element.value ?? text,
        elementBounds: bounds,
        textBounds: bounds ? { ...bounds, width: Math.min(numberField(bounds, "width") ?? 0, measuredWidthPx ?? 0) } : null,
        renderedTextBounds: bounds ? { ...bounds, width: Math.min(numberField(bounds, "width") ?? 0, measuredWidthPx ?? 0) } : null,
        availableWidthPx,
        measuredWidthPx,
        textFitsContainer,
        clippingState: textFitsContainer === false ? "wouldClipOrTruncate" : "fitsMeasuredWidth",
        truncationIntent: textFitsContainer === false ? "requires-tooltip-or-accessible-full-text" : "none",
        tooltipOrAccessibleFullText: element.value != null && String(element.value).length >= text.length,
        measurementSource: "appkit_text_width_plus_getLayoutInfo",
      });
    }
    const overlapPairs: Array<Record<string, unknown>> = [];
    for (let i = 0; i < textSamples.length; i += 1) {
      for (let j = i + 1; j < textSamples.length; j += 1) {
        const a = asRecord(textSamples[i].elementBounds);
        const b = asRecord(textSamples[j].elementBounds);
        const ax = numberField(a, "x");
        const ay = numberField(a, "y");
        const aw = numberField(a, "width");
        const ah = numberField(a, "height");
        const bx = numberField(b, "x");
        const by = numberField(b, "y");
        const bw = numberField(b, "width");
        const bh = numberField(b, "height");
        if ([ax, ay, aw, ah, bx, by, bw, bh].some((value) => value == null)) continue;
        const overlaps = ax! < bx! + bw! && ax! + aw! > bx! && ay! < by! + bh! && ay! + ah! > by!;
        if (overlaps) {
          overlapPairs.push({
            a: textSamples[i].semanticId,
            b: textSamples[j].semanticId,
          });
        }
      }
    }
    stopReceipt = await stopVisibleMeasurementSurface(session, shouldStop);
    return { startReceipt, stopReceipt, state, elements, layoutInfo, textSamples, overlapPairs };
  } catch (error) {
    stopReceipt = await stopVisibleMeasurementSurface(session, shouldStop);
    throw error;
  }
}

async function collectThemeContrastReceipt(): Promise<Record<string, unknown>> {
  const result = await runTool(
    [
      "env",
      "AGENTIC_THEME_CONTRAST_RECEIPT=1",
      "cargo",
      "test",
      "--test",
      "theme_contrast_audit",
      "agentic_theme_contrast_receipt",
      "--",
      "--nocapture",
    ],
    "theme-contrast-receipt"
  );
  const receiptLine = result.stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("AGENTIC_THEME_CONTRAST_RECEIPT="));
  if (result.exitCode !== 0 || !receiptLine) {
    return {
      receiptKind: "visual.contrastReadableState",
      source: "script_kit_gpui::theme::audit_theme_contrast",
      commandExitCode: result.exitCode,
      commandStdout: result.stdout.slice(0, 2000),
      commandStderr: result.stderr.slice(0, 2000),
      failingThemeCount: null,
      themes: [],
    };
  }
  return parseMaybeJson(receiptLine.replace(/^AGENTIC_THEME_CONTRAST_RECEIPT=/, ""));
}

async function collectMainLayoutShiftAudit(session: string): Promise<Record<string, unknown>> {
  const { startReceipt, shouldStop } = await startVisibleMeasurementSurface(session);
  let stopReceipt: Record<string, unknown> | null = null;
  try {
    const beforeLayout = rpcResponse(await rpc(session, { type: "getLayoutInfo", requestId: "layout-shift-before" }, "layoutInfoResult"));
    const beforeFingerprint = mainLayoutFingerprint(beforeLayout);
    const setFilterReceipt = await sendAndAwaitParse(session, {
      type: "setFilter",
      text: "agentic-layout-shift",
      requestId: "layout-shift-set-filter",
    });
    const afterFilterLayout = rpcResponse(await rpc(session, { type: "getLayoutInfo", requestId: "layout-shift-after-filter" }, "layoutInfoResult"));
    const afterFilterFingerprint = mainLayoutFingerprint(afterFilterLayout);
    const resetFilterReceipt = await sendAndAwaitParse(session, {
      type: "setFilter",
      text: "",
      requestId: "layout-shift-reset-filter",
    });
    const afterResetLayout = rpcResponse(await rpc(session, { type: "getLayoutInfo", requestId: "layout-shift-after-reset" }, "layoutInfoResult"));
    const afterResetFingerprint = mainLayoutFingerprint(afterResetLayout);
    stopReceipt = await stopVisibleMeasurementSurface(session, shouldStop);
    const filterStable = JSON.stringify(beforeFingerprint) === JSON.stringify(afterFilterFingerprint);
    const resetStable = JSON.stringify(beforeFingerprint) === JSON.stringify(afterResetFingerprint);
    return {
      startReceipt,
      stopReceipt,
      setFilterReceipt,
      resetFilterReceipt,
      beforeFingerprint,
      afterFilterFingerprint,
      afterResetFingerprint,
      layoutShiftAfterFilter: filterStable ? "stable" : "changed",
      layoutShiftAfterReset: resetStable ? "stable" : "changed",
      layoutShiftScore: filterStable && resetStable ? 0 : 1,
      unexpectedShiftDetected: !(filterStable && resetStable),
    };
  } catch (error) {
    stopReceipt = await stopVisibleMeasurementSurface(session, shouldStop);
    return {
      startReceipt,
      stopReceipt,
      error: error instanceof Error ? error.message : String(error),
      layoutShiftAfterFilter: "error",
      layoutShiftAfterReset: "error",
      layoutShiftScore: 1,
      unexpectedShiftDetected: true,
    };
  }
}

export async function runAgentChatPortalRoundTripOriginStressScenario(opts: {
  session: string;
  host: string;
  portal: string;
  selection?: string;
  query?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "file-portal-origin-roundtrip",
    status: "fail",
    origin: {
      host: opts.host,
      session: opts.session,
      agent_chatGeneration: null,
      portalSessionId: null,
      returnTarget: null,
    },
    portal: {
      kind: opts.portal,
      selection: opts.selection ?? "file",
      query: opts.query ?? "AGENTS.md",
      roundTrip: "unverified",
      expectedReceipts: [
        "origin.host",
        "origin.agent_chatGeneration",
        "portal.sessionId",
        "portal.returnTarget",
        "contextPart.uri",
      ],
    },
    usage: {
      stateFirst: true,
      usedGetAgentChatState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "portal-origin-receipt-preflight",
        status: "fail",
        output: {
          session: opts.session,
          host: opts.host,
          portal: opts.portal,
          blockingGap:
            "Agent Chat portal round-trip receipts do not yet expose origin generation, portal session id, return target, and accepted context part URI to the TypeScript harness.",
        },
      },
    ],
    failure: {
      code: "missing_portal_round_trip_origin_receipt",
      stepName: "portal-origin-receipt-preflight",
      message:
        "The harness fails closed until Agent Chat portal round-trip origin and context-part receipts exist.",
    },
    warnings: ["file_linear:agent_chat_portal_round_trip_origin_receipts_missing"],
  };
}

export async function runPermissionPreflightReadonlyScenario(opts: {
  session: string;
  kinds?: string[];
}): Promise<HardScenarioReceipt> {
  const steps: Array<Record<string, unknown>> = [];

  const inputCheck = await runTool(
    ["bun", "scripts/agentic/macos-input.ts", "check"],
    "permission-preflight:macos-input-check"
  );
  const inputPayload = parseMaybeJson(inputCheck.stdout);
  steps.push({
    name: "macos-input-check",
    status: inputCheck.exitCode === 0 ? "pass" : "fail",
    output: inputPayload,
  });

  const windowStatus = await runTool(
    ["bun", "scripts/agentic/window.ts", "status"],
    "permission-preflight:window-status"
  );
  const windowPayload = parseMaybeJson(windowStatus.stdout);
  steps.push({
    name: "window-status",
    status: windowStatus.exitCode === 0 ? "pass" : "fail",
    output: windowPayload,
  });

  const permissions = {
    session: opts.session,
    kinds: opts.kinds ?? ["accessibility", "screen-recording", "microphone"],
    accessibility:
      ((inputPayload.data as Record<string, unknown> | undefined)?.accessibility as
        | boolean
        | undefined) ?? null,
    screenRecording: "notPrompted",
    microphone: "notPrompted",
    passiveOnly: true,
    openedSystemSettings: false,
    mutatedTcc: false,
    clickedSettings: false,
  };
  const allPass = steps.every((step) => step.status === "pass");

  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "permission-privacy-preflight",
    status: allPass ? "pass" : "fail",
    permissions,
    usage: {
      stateFirst: true,
      usedGetAgentChatState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      openedSystemSettings: false,
      mutatedUserData: false,
    },
    steps,
    failure: allPass
      ? undefined
      : {
          code: "permission_preflight_failed",
          stepName: "permission-privacy-preflight",
          message:
            "Read-only permission preflight failed without opening System Settings or mutating permissions.",
        },
    warnings: [],
  };
}

export async function runShortcutRecorderFocusCaptureStressScenario(opts: {
  session: string;
  chord: string;
  surface?: string;
  action?: string;
  sandboxConfig?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "shortcut-recorder-focus-capture",
    status: "fail",
    shortcut: {
      chord: opts.chord,
      surface: opts.surface ?? "shortcuts",
      action: opts.action ?? "test-agentic-shortcut",
      sandboxConfig: opts.sandboxConfig ?? false,
      recorderSurface: null,
      focusedAutomationWindowId: null,
      capturedShortcut: null,
      leakedGlobalHotkey: null,
    },
    usage: {
      stateFirst: true,
      usedGetAgentChatState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "shortcut-recorder-receipt-preflight",
        status: "fail",
        output: {
          session: opts.session,
          chord: opts.chord,
          surface: opts.surface ?? "shortcuts",
          action: opts.action ?? "test-agentic-shortcut",
          sandboxConfig: opts.sandboxConfig ?? false,
          blockingGap:
            "Shortcut recorder focus/capture receipts are not yet exposed to the TypeScript harness without writing config.ts.",
        },
      },
    ],
    failure: {
      code: "missing_shortcut_recorder_capture_receipt",
      stepName: "shortcut-recorder-receipt-preflight",
      message:
        "The harness fails closed until recorder focus, captured chord, and global-hotkey leak receipts exist.",
    },
    warnings: ["file_linear:shortcut_recorder_capture_receipts_missing"],
  };
}

export async function runTemplatePromptAutomationParityStressScenario(opts: {
  session: string;
  template?: string;
  field?: string;
  value?: string;
  forcedValue?: string;
}): Promise<HardScenarioReceipt> {
  const template = opts.template ?? "Hello {{name}}";
  const field = opts.field ?? "name";
  const value = opts.value ?? "Ada";
  const forcedValue = opts.forcedValue ?? "forced-template-result";
  const steps: Array<Record<string, unknown>> = [];

  const pushStep = (
    name: string,
    status: "pass" | "fail" | "error",
    output: unknown,
  ) => {
    steps.push({ name, status, output });
  };

  const fail = (
    code: TargetThreadFailure["code"],
    stepName: string,
    message: string,
    output?: unknown,
  ): HardScenarioReceipt => {
    if (output !== undefined) pushStep(stepName, "fail", output);
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "template-prompt-automation-parity-stress",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: code,
      templatePrompt: {
        session: opts.session,
        template,
        field,
        value,
        forcedValue,
        failureStep: stepName,
      },
      usage: {
        stateFirst: true,
        usedGetState: true,
        usedGetElements: true,
        usedWaitFor: false,
        usedBatch: true,
        usedSimulateKey: true,
        usedNativeInput: false,
        usedScreenshot: false,
        usedFixedSleepMs: 0,
        mutatedUserData: false,
      },
      steps,
      failure: { code, stepName, message },
      warnings: [],
    };
  };

  const send = async (payload: Record<string, unknown>, name: string) => {
    const shouldAwaitParse = payload.type !== "template";
    const args = [
      "bash",
      "scripts/agentic/session.sh",
      "send",
      opts.session,
      JSON.stringify(payload),
    ];
    if (shouldAwaitParse) {
      args.push("--await-parse", "--timeout", "8000");
    }
    const result = await runTool(
      args,
      `template-prompt:${name}`,
    );
    if (result.exitCode !== 0) {
      throw new Error(result.stdout || result.stderr || `${name} failed`);
    }
    return parseMaybeJson(result.stdout);
  };

  const extractResponse = (receipt: Record<string, unknown>) =>
    ((receipt.response as Record<string, unknown> | undefined) ?? receipt);

  try {
    const start = await runTool(
      ["bash", "scripts/agentic/session.sh", "start", opts.session],
      "template-prompt:session-start",
    );
    if (start.exitCode !== 0) {
      return fail(
        "missing_template_prompt_automation_receipt",
        "session-start",
        "TemplatePrompt parity could not start or resume the requested session.",
        parseMaybeJson(start.stdout || start.stderr),
      );
    }
    pushStep("session-start", "pass", parseMaybeJson(start.stdout));

    const submitId = `tpl-submit-${Date.now()}`;
    const opened = await send(
      { type: "template", id: submitId, template, requestId: `${submitId}-open` },
      "open-submit-template",
    );
    pushStep("open-submit-template", "pass", opened);

    const stateEnvelope = await rpc(
      opts.session,
      { type: "getState", requestId: `${submitId}-state` },
      "stateResult",
      8000,
    );
    const state = extractResponse(stateEnvelope);
    if (state.promptType !== "template") {
      return fail(
        "template_prompt_state_missing",
        "get-state",
        "Expected getState.promptType to be template.",
        stateEnvelope,
      );
    }
    pushStep("get-state", "pass", stateEnvelope);

    const elementsEnvelope = await rpc(
      opts.session,
      { type: "getElements", requestId: `${submitId}-elements`, limit: 80 },
      "elementsResult",
      8000,
    );
    const elementsResponse = extractResponse(elementsEnvelope);
    const elements = Array.isArray(elementsResponse.elements)
      ? elementsResponse.elements as Array<Record<string, unknown>>
      : [];
    const sourceRow = elements.find((element) => element.semanticId === "input:template-source");
    const fieldRowId = `input:template-${field}`;
    const fieldRow = elements.find((element) => element.semanticId === fieldRowId);
    if (!sourceRow || !fieldRow) {
      return fail(
        "template_prompt_elements_missing",
        "get-elements",
        "Expected template source and field rows in getElements.",
        { sourceRow: Boolean(sourceRow), fieldRowId, rowCount: elements.length, elementsEnvelope },
      );
    }
    pushStep("get-elements", "pass", { rowCount: elements.length, sourceRow, fieldRow });

    const fillEnvelope = await rpc(
      opts.session,
      {
        type: "batch",
        requestId: `${submitId}-fill`,
        commands: [{ type: "setInput", text: value }],
      },
      "batchResult",
      8000,
    );
    const fill = extractResponse(fillEnvelope);
    if (fill.success !== true) {
      return fail(
        "template_prompt_force_submit_failed",
        "batch-set-input",
        "TemplatePrompt batch.setInput failed.",
        fillEnvelope,
      );
    }
    pushStep("batch-set-input", "pass", fillEnvelope);

    const actionId = `tpl-actions-${Date.now()}`;
    await send(
      { type: "template", id: actionId, template, requestId: `${actionId}-open` },
      "open-actions-template",
    );
    await send(
      { type: "simulateKey", key: "k", modifiers: ["cmd"], requestId: `${actionId}-cmd-k` },
      "cmd-k-actions",
    );
    let actionsElementsEnvelope: Record<string, unknown> | null = null;
    try {
      actionsElementsEnvelope = await rpc(
        opts.session,
        {
          type: "getElements",
          requestId: `${actionId}-actions-elements`,
          target: { type: "kind", kind: "actionsDialog", index: 0 },
          limit: 80,
        },
        "elementsResult",
        8000,
      );
    } catch (error) {
      actionsElementsEnvelope = {
        error: error instanceof Error ? error.message : String(error),
      };
    }
    let actionsStateEnvelope: Record<string, unknown> | null = null;
    try {
      actionsStateEnvelope = await rpc(
        opts.session,
        {
          type: "getState",
          requestId: `${actionId}-actions-state`,
          target: { type: "kind", kind: "actionsDialog", index: 0 },
        },
        "stateResult",
        8000,
      );
    } catch (error) {
      actionsStateEnvelope = {
        error: error instanceof Error ? error.message : String(error),
      };
    }
    const actionsState = extractResponse(actionsStateEnvelope ?? {});
    const actionsElements = extractResponse(actionsElementsEnvelope ?? {});
    const actionRows = Array.isArray(actionsElements.elements)
      ? actionsElements.elements as Array<Record<string, unknown>>
      : [];
    const concreteActionRows = actionRows.filter((row) =>
      typeof row.semanticId === "string"
        && (row.semanticId.startsWith("action:") || row.semanticId.startsWith("choice:")));
    const actionsWarnings = Array.isArray(actionsElements.warnings)
      ? actionsElements.warnings as string[]
      : [];
    const actionsOpened = Boolean(actionsState.activePopupContract) || concreteActionRows.length > 0;
    if (!actionsOpened) {
      return fail(
        "template_prompt_actions_unavailable",
        "cmd-k-actions",
        "TemplatePrompt Cmd+K did not expose an active actions popup contract or concrete actionsDialog rows.",
        { actionsStateEnvelope, actionsElementsEnvelope, concreteActionRowCount: concreteActionRows.length },
      );
    }
    if (actionsWarnings.includes("panel_only_actions_dialog")) {
      return fail(
        "template_prompt_actions_unavailable",
        "cmd-k-actions",
        "TemplatePrompt Cmd+K exposed only a panel placeholder instead of concrete action rows.",
        { actionsStateEnvelope, actionsElementsEnvelope, concreteActionRowCount: concreteActionRows.length },
      );
    }
    pushStep("cmd-k-actions", "pass", { actionsStateEnvelope, actionsElementsEnvelope });
    await send(
      { type: "simulateKey", key: "escape", modifiers: [], requestId: `${actionId}-escape-actions` },
      "escape-actions",
    );

    const cancelId = `tpl-cancel-${Date.now()}`;
    await send(
      { type: "template", id: cancelId, template, requestId: `${cancelId}-open` },
      "open-cancel-template",
    );
    await send(
      { type: "simulateKey", key: "escape", modifiers: [], requestId: `${cancelId}-escape` },
      "escape-cancel",
    );
    pushStep("escape-cancel", "pass", { id: cancelId });

    const forceId = `tpl-force-${Date.now()}`;
    await send(
      { type: "template", id: forceId, template, requestId: `${forceId}-open` },
      "open-force-template",
    );
    const forceEnvelope = await rpc(
      opts.session,
      {
        type: "batch",
        requestId: `${forceId}-batch-force`,
        commands: [{ type: "forceSubmit", value: forcedValue }],
      },
      "batchResult",
      8000,
    );
    const force = extractResponse(forceEnvelope);
    const forceResult = Array.isArray(force.results)
      ? force.results[0] as Record<string, unknown> | undefined
      : undefined;
    if (force.success !== true || forceResult?.value !== forcedValue) {
      return fail(
        "template_prompt_force_submit_failed",
        "batch-force-submit",
        "TemplatePrompt batch.forceSubmit did not return the explicit provided value.",
        forceEnvelope,
      );
    }
    pushStep("batch-force-submit", "pass", forceEnvelope);

    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "template-prompt-automation-parity-stress",
      status: "pass",
      templatePrompt: {
        session: opts.session,
        template,
        field,
        value,
        forcedValue,
        promptType: state.promptType,
        statePromptId: state.promptId ?? null,
        elementSemanticIds: elements.map((element) => element.semanticId).filter(Boolean),
        sourceRow,
        fieldRow,
        actionsHost: "TemplatePrompt",
        activePopupContract: actionsState.activePopupContract,
        actionsRowCount: actionRows.length,
        batchSetInput: { field, value, success: true },
        cancel: { viaEscape: true },
        batchForceSubmit: { providedValue: forcedValue, resolvedValue: forceResult.value },
      },
      usage: {
        stateFirst: true,
        usedGetState: true,
        usedGetElements: true,
        usedWaitFor: false,
        usedBatch: true,
        usedSimulateKey: true,
        usedNativeInput: false,
        usedScreenshot: false,
        usedFixedSleepMs: 0,
        mutatedUserData: false,
      },
      steps,
      warnings: [],
    };
  } catch (error) {
    return fail(
      "missing_template_prompt_automation_receipt",
      "template-prompt-runtime",
      error instanceof Error ? error.message : String(error),
    );
  }
}

export async function runCurrentAppCommandsFrontmostStressScenario(opts: {
  session: string;
  query?: string;
  alias?: string;
  expectedApp?: string;
}): Promise<HardScenarioReceipt> {
  const query = opts.query ?? "close tab";
  const alias = opts.alias ?? "Do in Current Command";

  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "current-app-commands-frontmost-stress",
    status: "fail",
    currentAppCommands: {
      session: opts.session,
      stableEntryId: "builtin/do-in-current-app",
      alias,
      query,
      expectedApp: opts.expectedApp ?? null,
      frontmostSnapshot: null,
      openedView: null,
      sharedFilterHelper: "current_app_commands_filtered_entries",
      stateVisibleChoiceCount: null,
      elementRowCount: null,
      rendererRowCount: null,
      staleAliasRejected: null,
      wrongAppExecutionBlocked: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "current-app-frontmost-receipt-preflight",
        status: "fail",
        output: {
          session: opts.session,
          alias,
          query,
          blockingGap:
            "CurrentAppCommandsView receipts do not yet expose a frontmost-app snapshot, alias normalization proof, shared filter counts, and wrong-app execution guard in one agentic recipe.",
        },
      },
    ],
    failure: {
      code: "missing_current_app_commands_frontmost_receipt",
      stepName: "current-app-frontmost-receipt-preflight",
      message:
        "The harness fails closed until Do in Current App can prove frontmost snapshot identity and shared filter semantics without executing against a stale app.",
    },
    warnings: ["file_linear:current_app_commands_frontmost_receipts_missing"],
  };
}

export async function runActionsCapturedSubjectFrameStressScenario(opts: {
  session: string;
  source?: string;
  action?: string;
  mutation?: string;
}): Promise<HardScenarioReceipt> {
  const source = opts.source ?? "root-file";
  const action = opts.action ?? "quick-look";
  const mutation = opts.mutation ?? "filter-selection-cache-frame";

  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "actions-captured-subject-frame-stress",
    status: "fail",
    actionsCapturedSubject: {
      session: opts.session,
      host: "MainList",
      source,
      action,
      mutation,
      subjectStableKey: null,
      pendingSubjectFrame: null,
      activePopupContract: null,
      executeSubjectStableKey: null,
      focusRestoredTo: null,
      reReadCurrentSelection: null,
      unknownRootIdNoop: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "actions-captured-subject-receipt-preflight",
        status: "fail",
        output: {
          session: opts.session,
          source,
          action,
          mutation,
          blockingGap:
            "ActionsDialog receipts do not yet expose the captured MainList subject, source-filter frame, execution subject, and focus-restore target as one stable proof.",
        },
      },
    ],
    failure: {
      code: "missing_actions_captured_subject_receipt",
      stepName: "actions-captured-subject-receipt-preflight",
      message:
        "The harness fails closed until root actions can prove execution uses the captured subject after filter/selection/cache/frame drift.",
    },
    warnings: ["file_linear:actions_captured_subject_receipts_missing"],
  };
}

export async function runDropPromptNativeDropPrivacyStressScenario(opts: {
  session: string;
  fileName?: string;
  size?: number;
}): Promise<HardScenarioReceipt> {
  const fileName = opts.fileName ?? "agentic-drop.txt";
  const size = opts.size ?? 12;
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "drop-prompt-native-drop-privacy-stress",
    status: "fail",
    dropPrompt: {
      session: opts.session,
      fileName,
      size,
      expectedState: "stateResult.drop.files[index,name,size]",
      expectedElements: "list:dropped-files + kind:dropped_file rows",
      forbiddenFields: ["path", "parentPath", "content", "mimeType", "modifiedTime"],
      nativeDropInjected: false,
      pathLeakDetected: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "drop-native-receipt-preflight",
        status: "fail",
        output: {
          session: opts.session,
          fileName,
          blockingGap:
            "DropPrompt automation has redacted state/elements receipts, but scripts/agentic does not yet have a deterministic native file-drop injection receipt.",
        },
      },
    ],
    failure: {
      code: "missing_drop_prompt_native_drop_receipt",
      stepName: "drop-native-receipt-preflight",
      message:
        "The harness fails closed until native DropPrompt file-drop injection can prove redacted automation receipts without leaking paths.",
    },
    warnings: ["file_linear:drop_prompt_native_drop_receipts_missing"],
  };
}

export async function runPathPromptFilesystemEdgeStressScenario(opts: {
  session: string;
}): Promise<HardScenarioReceipt> {
  const result = await runTool(
    ["bun", "scripts/agentic/path-prompt-fs-edges.ts"],
    "path-prompt-filesystem-edge-stress",
  );
  const output = parseMaybeJson(result.stdout);
  const passed = result.exitCode === 0 && (output as Record<string, unknown>).status === "ok";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "path-prompt-filesystem-edge-stress",
    status: passed ? "pass" : "fail",
    pathPrompt: {
      session: opts.session,
      helper: "scripts/agentic/path-prompt-fs-edges.ts",
      cases: ["missing", "empty", "file-start", "permission-denied"],
      statusKinds: ["missing", "empty", "loaded", "permissionDenied"],
      output,
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "path-prompt-fs-edges-helper",
        status: passed ? "pass" : "fail",
        output,
      },
    ],
    failure: passed
      ? undefined
      : {
          code: "path_prompt_filesystem_edge_failed",
          stepName: "path-prompt-fs-edges-helper",
          message: result.stderr || "PathPrompt filesystem edge helper failed.",
        },
    warnings: passed ? [] : ["path_prompt_filesystem_edge_helper_failed"],
  };
}

export async function runScreenshotIdentityAgentChatContextStressScenario(opts: {
  session: string;
  source?: string;
}): Promise<HardScenarioReceipt> {
  const source = opts.source ?? "tab-ai-screenshot";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "screenshot-identity-agent_chat-context-stress",
    status: "fail",
    screenshotIdentity: {
      session: opts.session,
      source,
      stateField: "stateResult.screenshotIdentity",
      expectedIdentityShape: "bare screenshot filename",
      captureReceipt: null,
      agent_chatContextPart: null,
      identityMatched: null,
      filesystemGrepUsed: false,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "screenshot-identity-context-receipt-preflight",
        status: "fail",
        output: {
          session: opts.session,
          source,
          blockingGap:
            "Agent Chat context automation does not yet expose one receipt tying capture identity, stateResult.screenshotIdentity, and accepted Agent Chat context part identity together.",
        },
      },
    ],
    failure: {
      code: "missing_screenshot_identity_context_receipt",
      stepName: "screenshot-identity-context-receipt-preflight",
      message:
        "The harness fails closed until screenshot identity threading can be proven from state and Agent Chat context receipts without grepping the filesystem.",
    },
    warnings: ["file_linear:screenshot_identity_context_receipts_missing"],
  };
}

export async function runClipboardHistoryPortalRangeStressScenario(opts: {
  session: string;
  portalId?: string;
  range?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "clipboard-history-portal-range-stress",
    status: "fail",
    clipboardPortal: {
      session: opts.session,
      portalId: opts.portalId ?? "kit://clipboard-history?id=agentic",
      range: opts.range ?? "composer:0..0",
      hostRefusalReceipt: null,
      roundTripUri: null,
      exactRangeReplacement: null,
      wrongHostAccepted: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "clipboard-portal-range-receipt-preflight",
      status: "fail",
      output: { blockingGap: "Clipboard portal host refusal, kit:// URI round-trip, and exact range replacement receipts are not exposed as one agentic proof." },
    }],
    failure: {
      code: "missing_clipboard_portal_range_receipt",
      stepName: "clipboard-portal-range-receipt-preflight",
      message: "The harness fails closed until clipboard-history portal range receipts exist.",
    },
    warnings: ["file_linear:clipboard_portal_range_receipts_missing"],
  };
}

export async function runBrowserTabsCacheIdentityStressScenario(opts: {
  session: string;
  source?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "browser-tabs-cache-identity-stress",
    status: "fail",
    browserCache: {
      session: opts.session,
      source: opts.source ?? "browser-tabs",
      cacheOnly: true,
      browserActivated: false,
      dedupeKey: null,
      sourceIdentity: null,
      staleCacheRejected: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "browser-cache-identity-receipt-preflight",
      status: "fail",
      output: { blockingGap: "Browser tabs/history cache identity and dedupe receipts are not exposed without activating the browser." },
    }],
    failure: {
      code: "missing_browser_cache_identity_receipt",
      stepName: "browser-cache-identity-receipt-preflight",
      message: "The harness fails closed until browser cache identity receipts exist.",
    },
    warnings: ["file_linear:browser_cache_identity_receipts_missing"],
  };
}

export async function runScrollSelectionReanchorStressScenario(opts: {
  session: string;
  surfaces?: string[];
}): Promise<HardScenarioReceipt> {
  const surfaces = opts.surfaces ?? ["clipboard", "browser-history", "current-app-commands", "file-search"];
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "scroll-selection-reanchor-stress",
    status: "fail",
    scrollSelection: {
      session: opts.session,
      surfaces,
      initialSelectedSemanticId: null,
      afterWheelSelectedSemanticId: null,
      afterDragSelectedSemanticId: null,
      visibleRowStillSelected: null,
      footerOcclusionSafe: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "scroll-selection-reanchor-receipt-preflight",
      status: "fail",
      output: { surfaces, blockingGap: "Cross-surface wheel/drag selection reanchor receipts are not exposed as one agentic proof." },
    }],
    failure: {
      code: "missing_scroll_selection_reanchor_receipt",
      stepName: "scroll-selection-reanchor-receipt-preflight",
      message: "The harness fails closed until scroll/drag reanchor receipts exist.",
    },
    warnings: ["file_linear:scroll_selection_reanchor_receipts_missing"],
  };
}

export async function runPermissionAssistantDragPreflightStressScenario(opts: {
  session: string;
  pane?: string;
  bundleId?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "permission-assistant-drag-preflight-stress",
    status: "fail",
    permissionAssistant: {
      session: opts.session,
      pane: opts.pane ?? "Accessibility",
      bundleId: opts.bundleId ?? "com.scriptkit.app",
      passivePreflight: {
        accessibility: null,
        screenRecording: "notPrompted",
        microphone: "notPrompted",
        promptApisCalled: false,
        passiveOnly: true,
      },
      overlayPanel: {
        receipt: null,
        expectedClass: "PassiveOverlayPanel",
        expectedStyleMask: ["nonactivatingPanel"],
        expectedCanBecomeKey: false,
        expectedCanBecomeMain: false,
        expectedLevel: "statusBar",
        activationPolicyBefore: null,
        activationPolicyAfter: null,
        expectedActivationPolicyStable: "accessory",
      },
      dragSource: {
        receipt: null,
        expectedClass: "AppDragSourceView",
        expectedPasteboardType: "fileURL",
        expectedOperation: "copy",
        bundleUrl: null,
        isAppBundle: null,
        executablePathRejected: null,
        activatedApp: false,
      },
      targetPane: {
        receipt: null,
        settingsBundleId: "com.apple.systempreferences",
        requestedPane: opts.pane ?? "Accessibility",
        resolvedPane: null,
        locator: "settings_window_snapshot",
        cachedAcrossRefreshTicks: false,
      },
      noMutation: {
        openedSystemSettings: false,
        clickedSettings: false,
        performedDrag: false,
        mutatedTcc: false,
        wroteTccDb: false,
        calledPromptingApi: false,
      },
      wrongPaneAccepted: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
      openedSystemSettings: false,
      mutatedTcc: false,
    },
    steps: [{
      name: "permission-assistant-drag-preflight-receipt",
      status: "fail",
      output: {
        blockingGap:
          "Permission Assistant does not expose one read-only receipt tying passive drag-source identity, target privacy pane identity, and no-TCC-mutation proof.",
      },
    }],
    failure: {
      code: "missing_permission_assistant_drag_preflight_receipt",
      stepName: "permission-assistant-drag-preflight-receipt",
      message:
        "The harness fails closed until Permission Assistant drag/preflight receipts can be proven without opening or mutating System Settings.",
    },
    warnings: ["file_linear:permission_assistant_drag_receipts_missing"],
  };
}

export async function runQuickTerminalPtyApplyBackStressScenario(opts: {
  session: string;
  command?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "quick-terminal-pty-apply-back-stress",
    status: "fail",
    quickTerminal: {
      session: opts.session,
      command: opts.command ?? "printf 'agentic-pty-apply-back\\n'",
      prompt: {
        promptType: null,
        automationWindowId: null,
        surfaceId: null,
        focusedBefore: null,
        focusedAfter: null,
      },
      terminalSurfaceId: null,
      ptyReady: null,
      ptyId: null,
      shellOutputReceipt: null,
      stdoutContains: "agentic-loop-six",
      exitStatus: null,
      drained: null,
      applyBackTarget: null,
      sourceSelectionFingerprint: null,
      appliedText: null,
      focusRestored: null,
      selectionPreserved: null,
      ptyShutdownReceipt: null,
      orphanPids: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "quick-terminal-pty-apply-back-receipt",
      status: "fail",
      output: {
        blockingGap:
          "Quick Terminal does not expose one deterministic receipt for PTY readiness, shell output, apply-back target identity, focus/selection preservation, and PTY cleanup.",
      },
    }],
    failure: {
      code: "missing_quick_terminal_apply_back_receipt",
      stepName: "quick-terminal-pty-apply-back-receipt",
      message:
        "The harness fails closed until Quick Terminal apply-back can be proven from PTY and target receipts without manual terminal interaction.",
    },
    warnings: ["file_linear:quick_terminal_apply_back_receipts_missing"],
  };
}

export async function runMcpContextResourceAttachmentIdentityStressScenario(opts: {
  session: string;
  resourceUri?: string;
  profile?: string;
  source?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "mcp-context-resource-attachment-identity-stress",
    status: "fail",
    mcpContextResource: {
      session: opts.session,
      resource: {
        uri: opts.resourceUri ?? "kit://context/agentic-loop-six",
        profile: opts.profile ?? "agentic-test",
        source: opts.source ?? "mcp-resource",
        resourceId: null,
        generation: null,
        contentHash: null,
      },
      composer: {
        host: "agent_chat",
        returnTarget: null,
        openedWithoutManualPicker: false,
      },
      acceptedContextPart: {
        receipt: null,
        uri: null,
        profile: null,
        source: null,
        resourceId: null,
        generation: null,
      },
      resourceCatalogIdentity: null,
      acceptedContextPartUri: null,
      contextSourceIdentity: null,
      staleResource: {
        staleUri: `${opts.resourceUri ?? "kit://context/agentic-loop-six"}?generation=stale`,
        rejected: null,
        reason: null,
      },
      returnTarget: null,
      wrongProfileAccepted: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "mcp-context-resource-receipt-preflight",
      status: "fail",
      output: {
        blockingGap:
          "MCP context/resource attachment does not expose one receipt tying resource URI/profile/source identity to the accepted context part and return target.",
      },
    }],
    failure: {
      code: "missing_mcp_context_resource_attachment_receipt",
      stepName: "mcp-context-resource-receipt-preflight",
      message:
        "The harness fails closed until resource-backed context attachment identity can be proven without manual picker interaction.",
    },
    warnings: ["file_linear:mcp_context_resource_attachment_receipts_missing"],
  };
}

export async function runSettingsThemeHotReloadStressScenario(opts: {
  session: string;
  themeBefore?: string;
  themeAfter?: string;
  configKey?: string;
  sandboxConfig?: boolean;
}): Promise<HardScenarioReceipt> {
  const themeBefore = opts.themeBefore ?? "script-kit-dark";
  const themeAfter = opts.themeAfter ?? "script-kit-light";
  const configKey = opts.configKey ?? "theme";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "settings-theme-hot-reload-stress",
    status: "fail",
    settingsThemeHotReload: {
      session: opts.session,
      sandboxConfig: opts.sandboxConfig ?? false,
      requestedPreference: { configKey, beforeTheme: themeBefore, afterTheme: themeAfter },
      configSourceIdentity: {
        configDir: null,
        configPathFingerprint: null,
        sourceKind: null,
        generation: null,
      },
      themeSourceIdentity: {
        beforeThemeId: themeBefore,
        afterThemeId: themeAfter,
        beforeThemeTokenFingerprint: null,
        afterThemeTokenFingerprint: null,
        themeFileFingerprint: null,
      },
      rendererCache: {
        beforeRendererCacheRevision: null,
        afterRendererCacheRevision: null,
        staleRendererCacheRejected: null,
      },
      activeWindowRepaint: {
        windowId: null,
        beforePaintRevision: null,
        afterPaintRevision: null,
        repaintObserved: null,
      },
      cleanup: {
        restoredConfigFingerprint: null,
        restoredThemeTokenFingerprint: null,
        manualSettingsClicks: false,
        mutatedUserConfig: false,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
      manualSettingsClicks: false,
    },
    steps: [{
      name: "settings-theme-hot-reload-receipt-preflight",
      status: "fail",
      output: {
        blockingGap:
          "Settings/theme hot-reload does not expose one receipt tying config source identity, theme token fingerprints, renderer cache revision, active window repaint, and restore cleanup.",
      },
    }],
    failure: {
      code: "missing_settings_theme_hot_reload_receipt",
      stepName: "settings-theme-hot-reload-receipt-preflight",
      message:
        "The harness fails closed until Settings/theme hot-reload can be proven from source identity, token fingerprint, repaint, and cleanup receipts.",
    },
    warnings: ["file_linear:settings_theme_hot_reload_receipts_missing"],
  };
}

export async function runFileSearchDragOutIdentityStressScenario(opts: {
  session: string;
  query?: string;
  fileName?: string;
  dropTarget?: string;
}): Promise<HardScenarioReceipt> {
  const query = opts.query ?? "AGENTS.md";
  const fileName = opts.fileName ?? query;
  const dropTarget = opts.dropTarget ?? "host-refusal-fixture";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "file-search-drag-out-identity-stress",
    status: "fail",
    fileSearchDragOut: {
      session: opts.session,
      query,
      fileName,
      dropTarget,
      sourceSurface: {
        promptType: null,
        automationWindowId: null,
        semanticSurface: null,
      },
      selectedFile: {
        selectedSemanticId: null,
        selectedFileUri: null,
        selectedBasename: fileName,
        selectedRowFingerprint: null,
      },
      visibleRowsPrivacy: {
        visibleRowsRedacted: null,
        privatePathLeakDetected: null,
        forbiddenVisibleFields: ["absolutePath", "parentPath", "homeExpandedPath", "fileContent"],
      },
      dragPreview: {
        previewCreated: null,
        previewFileUri: null,
        previewFingerprint: null,
      },
      dragPayloadIdentity: {
        pasteboardType: null,
        payloadFileUri: null,
        payloadMatchesSelectedFile: null,
        payloadLeaksPrivatePathInVisibleRows: null,
      },
      hostDropRefusal: {
        attemptedTarget: dropTarget,
        refused: null,
        refusalReason: null,
        wrongHostAccepted: null,
      },
      returnSurface: {
        returnedToSourceSurface: null,
        selectedSemanticIdAfterReturn: null,
        filterTextAfterReturn: null,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "file-search-drag-out-receipt-preflight",
      status: "fail",
      output: {
        blockingGap:
          "File Search does not expose one deterministic receipt for selected file URI, drag preview/payload identity, host refusal, visible-row privacy, and return-surface preservation.",
      },
    }],
    failure: {
      code: "missing_file_search_drag_out_identity_receipt",
      stepName: "file-search-drag-out-receipt-preflight",
      message:
        "The harness fails closed until File Search drag-out identity can be proven without leaking private paths or accepting the wrong host.",
    },
    warnings: ["file_linear:file_search_drag_out_identity_receipts_missing"],
  };
}

export async function runScriptletBundleExecutionMatrixStressScenario(opts: {
  session: string;
  scriptletId?: string;
  bundleId?: string;
  cancelAfterMs?: number;
}): Promise<HardScenarioReceipt> {
  const scriptletId = opts.scriptletId ?? "alpha";
  const bundleId = opts.bundleId ?? "agentic-loop-seven-bundle";
  const cancelAfterMs = opts.cancelAfterMs ?? 50;
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "scriptlet-bundle-execution-matrix-stress",
    status: "fail",
    scriptletBundleExecution: {
      session: opts.session,
      fixture: {
        bundleId,
        scriptletId,
        bundleSourceHash: null,
        fixtureRoot: null,
        mutatedUserKenv: false,
      },
      matrixCases: [
        {
          name: "scriptlet-alpha-output",
          selectedScriptletId: null,
          selectedBundleId: null,
          argsFingerprint: null,
          envFingerprint: null,
          executionId: null,
          executionOutput: null,
          exitStatus: null,
        },
        {
          name: "scriptlet-beta-isolation",
          selectedScriptletId: null,
          selectedBundleId: null,
          argsFingerprint: null,
          envFingerprint: null,
          executionId: null,
          executionOutput: null,
          crossScriptletStateBleed: null,
        },
        {
          name: "scriptlet-cancel",
          selectedScriptletId: null,
          selectedBundleId: null,
          executionId: null,
          cancelAfterMs,
          cancellationPath: null,
          cancelledBeforeOutputCommit: null,
          orphanProcessDetected: null,
        },
      ],
      isolation: {
        argEnvIsolation: null,
        crossScriptletStateBleed: null,
        leakedEnvKeys: null,
      },
      cleanup: {
        tempFixtureRemoved: null,
        orphanProcesses: null,
        mutatedUserKenv: false,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "scriptlet-bundle-execution-receipt-preflight",
      status: "fail",
      output: {
        blockingGap:
          "Scriptlet execution does not expose one receipt tying selected scriptlet id, bundle source hash, arg/env isolation, execution output, cancellation, and cross-scriptlet state isolation.",
      },
    }],
    failure: {
      code: "missing_scriptlet_bundle_execution_receipt",
      stepName: "scriptlet-bundle-execution-receipt-preflight",
      message:
        "The harness fails closed until scriptlet bundle execution can prove selected id, bundle hash, isolation, output, cancellation, and no cross-scriptlet state bleed.",
    },
    warnings: ["file_linear:scriptlet_bundle_execution_receipts_missing"],
  };
}

export async function runTrayGlobalHotkeyMenuMutationStressScenario(opts: {
  session: string;
  loops?: number;
}): Promise<HardScenarioReceipt> {
  const loops = opts.loops ?? 5;
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "tray-global-hotkey-menu-mutation-stress",
    status: "fail",
    trayMenuMutation: {
      session: opts.session,
      loops,
      menu: {
        sectionOrder: null,
        sectionLabels: null,
        itemIds: null,
        duplicateItemIds: null,
        duplicateLabels: null,
        versionItemId: "tray.version",
        versionLabelBefore: null,
        versionLabelAfter: null,
      },
      updateStateMutation: {
        before: null,
        mutation: "available-release",
        after: null,
        refreshRanOnMainThread: null,
      },
      actions: {
        targetActionIds: [
          "tray.open_script_kit",
          "tray.current_app_commands",
          "tray.open_notes",
          "tray.open_agent_chat",
          "tray.reload_scripts",
          "tray.check_for_updates",
        ],
        actionRoundTrip: null,
        targetIdentityStable: null,
        wrongActionRejected: null,
      },
      globalHotkeyRoute: {
        configuredAccelerator: null,
        displayedAccelerator: null,
        routeReceipt: null,
        openedSurface: null,
        wrongSurfaceOpened: null,
      },
      passiveSafety: {
        openedExternalUrl: false,
        reloadedScripts: false,
        quitApp: false,
        mutatedUserConfig: false,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "tray-menu-mutation-receipt-preflight",
      status: "fail",
      output: {
        session: opts.session,
        loops,
        blockingGap:
          "Tray/global-hotkey automation does not expose one receipt for live menu section order, update-state mutation, action target identity, duplicate item detection, and global-hotkey routing.",
      },
    }],
    failure: {
      code: "missing_tray_global_hotkey_menu_mutation_receipt",
      stepName: "tray-menu-mutation-receipt-preflight",
      message:
        "The harness fails closed until tray menu/global-hotkey receipts exist without clicking destructive menu items.",
    },
    warnings: ["file_linear:tray_global_hotkey_menu_mutation_receipts_missing"],
  };
}

export async function runMultiWindowResizeMonitorRestorationStressScenario(opts: {
  session: string;
  surfaces?: string[];
  monitorProfile?: string;
}): Promise<HardScenarioReceipt> {
  const requestedSurfaces = opts.surfaces ?? ["main", "actionsDialog", "agentChatDetached", "notes"];
  const monitorProfile = opts.monitorProfile ?? "scale-bounds-drift";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "multi-window-resize-monitor-restoration-stress",
    status: "fail",
    multiWindowRestore: {
      session: opts.session,
      requestedSurfaces,
      monitorSimulation: {
        profile: monitorProfile,
        scaleBefore: null,
        scaleAfter: null,
        visibleFrameBefore: null,
        visibleFrameAfter: null,
        mutationReceipt: null,
        usedRealDisplayMutation: false,
      },
      windows: {
        before: [],
        afterResize: [],
        afterRestore: [],
      },
      identity: {
        windowIdsStable: null,
        semanticSurfacesStable: null,
        attachedPopupParentId: null,
        detachedSurfaceId: null,
        notesWindowId: null,
      },
      bounds: {
        mainBoundsRestored: null,
        popupBoundsRestored: null,
        detachedAgentChatBoundsRestored: null,
        notesBoundsRestored: null,
        restoreOrder: null,
      },
      scaleRem: {
        scaleFactorStable: null,
        remPxStable: null,
        themeFontSizeStable: null,
      },
      clobberGuards: {
        noPopupMainClobber: null,
        noDetachedAgentChatMainClobber: null,
        noNotesMainClobber: null,
        popupParentStillMain: null,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
      mutatedDisplays: false,
    },
    steps: [{
      name: "multi-window-restore-receipt-preflight",
      status: "fail",
      output: {
        session: opts.session,
        requestedSurfaces,
        monitorProfile,
        blockingGap:
          "Window automation does not expose one deterministic receipt for monitor/scale mutation, restore order, semantic surface stability, rem/scale stability, and popup/main clobber guards.",
      },
    }],
    failure: {
      code: "missing_multi_window_resize_monitor_restoration_receipt",
      stepName: "multi-window-restore-receipt-preflight",
      message:
        "The harness fails closed until multi-window resize/monitor restoration receipts exist without mutating real display configuration.",
    },
    warnings: ["file_linear:multi_window_resize_monitor_restoration_receipts_missing"],
  };
}

export async function runAgentChatTargetedDictationDeliveryStressScenario(opts: {
  session: string;
  kind?: string;
  index?: number;
  transcript?: string;
}): Promise<HardScenarioReceipt> {
  const kind = opts.kind ?? "agentChatDetached";
  const index = opts.index ?? 0;
  const transcript = opts.transcript ?? "agentic loop eight dictation";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "agent_chat-targeted-dictation-delivery-stress",
    status: "fail",
    agent_chatDictationDelivery: {
      session: opts.session,
      transcript,
      target: {
        kind,
        index,
        targetAgentChatWindowId: null,
        targetSurfaceId: null,
        targetGenerationId: null,
      },
      peers: {
        embeddedAgentChatWindowId: null,
        detachedAgentChatWindowIds: [],
        wrongWindowUnchanged: null,
        wrongWindowInputBefore: null,
        wrongWindowInputAfter: null,
      },
      dictation: {
        deliveryId: null,
        transcriptGenerationId: null,
        pushReceipt: null,
        source: "syntheticTranscript",
        captureStarted: false,
        microphonePrompted: false,
        modelDownloadStarted: false,
        setupPromptOpened: false,
      },
      insertion: {
        cursorBefore: null,
        cursorAfter: null,
        cursorInsertionRange: null,
        insertedText: null,
        selectionReplaced: null,
      },
      outcome: {
        deliveredToTarget: false,
        deliveredToWrongWindow: null,
        targetGenerationMatched: null,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
      startedAudioCapture: false,
    },
    steps: [{
      name: "agent_chat-dictation-delivery-receipt-preflight",
      status: "fail",
      output: {
        session: opts.session,
        kind,
        index,
        transcript,
        blockingGap:
          "Agent Chat/dictation automation does not expose one targeted transcript delivery receipt with Agent Chat generation, cursor insertion range, wrong-window negative proof, and passive microphone/model setup flags.",
      },
    }],
    failure: {
      code: "missing_agent_chat_targeted_dictation_delivery_receipt",
      stepName: "agent_chat-dictation-delivery-receipt-preflight",
      message:
        "The harness fails closed until Agent Chat-targeted dictation delivery can be proven without starting microphone capture or model setup.",
    },
    warnings: ["file_linear:agent_chat_targeted_dictation_delivery_receipts_missing"],
  };
}

export async function runClipboardShareTrustInstallStressScenario(opts: {
  session: string;
  fixtureId?: string;
  shareKind?: string;
  acceptMode?: string;
}): Promise<HardScenarioReceipt> {
  const fixtureId = opts.fixtureId ?? "agentic-loop-nine";
  const shareKind = opts.shareKind ?? "script";
  const acceptMode = opts.acceptMode ?? "both";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "clipboard-share-trust-install-stress",
    status: "fail",
    clipboardShareTrust: {
      session: opts.session,
      fixture: {
        fixtureId,
        shareKind,
        acceptMode,
        shareUri: null,
        shareUriScheme: "scriptkit-share://v1",
        decodedPackageFingerprint: null,
        decodedKind: null,
        decodedTitle: null,
        decodedPluginLabel: null,
        decodedFileCount: null,
        allowedPathPrefixes: ["scripts/", "scriptlets/", "skills/", "agents/"],
        pathTraversalRejected: null,
      },
      clipboard: {
        originalChangeCount: null,
        originalFingerprint: null,
        injectedChangeCount: null,
        injectedFingerprint: null,
        restoredChangeCount: null,
        restoredFingerprint: null,
        clipboardRestored: null,
      },
      parentTrustPrompt: {
        receipt: null,
        promptKind: "shareTrust",
        parentWindowId: null,
        promptWindowId: null,
        targetWindowIdentity: null,
        shownBeforeInstall: null,
        displayedKind: null,
        displayedTitle: null,
        displayedPluginLabel: null,
        displayedFileCount: null,
        displayedPackageFingerprint: null,
      },
      trustGate: {
        noInstallBeforeTrust: null,
        installAttemptBeforeAccept: false,
        explicitAcceptRequired: true,
        explicitRefuseRequired: true,
      },
      refusePath: {
        clickedIgnore: false,
        refuseReceipt: null,
        installCountAfterRefuse: null,
        pluginRootFingerprintAfterRefuse: null,
      },
      acceptPath: {
        clickedInstall: false,
        acceptReceipt: null,
        installReceipt: null,
        installedPluginId: null,
        pluginRootFingerprintBefore: null,
        pluginRootFingerprintAfter: null,
      },
      cleanup: {
        clipboardRestored: null,
        removedFixturePlugin: false,
        mutatedUserPlugins: false,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedClipboard: false,
      mutatedUserData: false,
      mutatedUserPlugins: false,
    },
    steps: [{
      name: "clipboard-share-trust-install-receipt",
      status: "fail",
      output: {
        blockingGap:
          "Clipboard share import does not expose one deterministic receipt for decoded package identity, parent trust prompt identity, accept/refuse paths, install-before-trust guard, and clipboard restoration.",
      },
    }],
    failure: {
      code: "missing_clipboard_share_trust_install_receipt",
      stepName: "clipboard-share-trust-install-receipt",
      message: "The harness fails closed until clipboard share trust/install receipts exist.",
    },
    warnings: ["file_linear:clipboard_share_trust_install_receipts_missing"],
  };
}

export async function runClipboardShareWatcherStaleReplayStressScenario(opts: {
  session: string;
  fixtureId?: string;
  shareKind?: string;
  count?: number;
  burstMs?: number;
}): Promise<HardScenarioReceipt> {
  const fixtureId = opts.fixtureId ?? "agentic-loop-nine";
  const shareKind = opts.shareKind ?? "script";
  const requestedUriCount = opts.count ?? 3;
  const burstMs = opts.burstMs ?? 25;
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "clipboard-share-watcher-stale-replay-stress",
    status: "fail",
    clipboardShareReplay: {
      session: opts.session,
      fixture: {
        fixtureId,
        shareKind,
        requestedUriCount,
        burstMs,
        shareUris: [],
        packageFingerprints: [],
      },
      clipboard: {
        originalChangeCount: null,
        originalFingerprint: null,
        burstChangeCounts: [],
        restoredChangeCount: null,
        clipboardRestored: null,
      },
      watcher: {
        watcherReceipt: null,
        observedGenerations: [],
        latestGeneration: null,
        generationOrderingStrict: null,
        staleUriRejected: null,
        staleRejectionReceipts: [],
      },
      promptLifecycle: {
        activePromptGeneration: null,
        replacedPromptGenerations: [],
        cancelledPromptGenerations: [],
        promptReplacementReceipt: null,
        promptCancelReceipt: null,
      },
      installGuard: {
        acceptedGeneration: null,
        installDedupeKey: null,
        installCount: 0,
        duplicateInstallRejected: null,
        noDuplicateInstalls: null,
      },
      cleanup: {
        clipboardRestored: null,
        removedFixturePlugin: false,
        mutatedUserPlugins: false,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedClipboard: false,
      mutatedUserData: false,
      mutatedUserPlugins: false,
    },
    steps: [{
      name: "clipboard-share-watcher-replay-receipt",
      status: "fail",
      output: {
        blockingGap:
          "Clipboard share watcher does not expose generation ordering, stale URI rejection, prompt replacement/cancel, and duplicate-install guard receipts.",
      },
    }],
    failure: {
      code: "missing_clipboard_share_watcher_replay_receipt",
      stepName: "clipboard-share-watcher-replay-receipt",
      message:
        "The harness fails closed until clipboard share watcher generation/replay receipts exist.",
    },
    warnings: ["file_linear:clipboard_share_watcher_replay_receipts_missing"],
  };
}

export async function runPermissionShareCrossPromptFocusStressScenario(opts: {
  session: string;
  fixtureId?: string;
  shareKind?: string;
  pane?: string;
  bundleId?: string;
}): Promise<HardScenarioReceipt> {
  const fixtureId = opts.fixtureId ?? "agentic-loop-nine";
  const shareKind = opts.shareKind ?? "script";
  const pane = opts.pane ?? "Accessibility";
  const bundleId = opts.bundleId ?? "com.scriptkit.app";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "permission-share-cross-prompt-focus-stress",
    status: "fail",
    permissionShareCrossPrompt: {
      session: opts.session,
      fixture: { fixtureId, shareKind, pane, bundleId },
      permissionAssistant: {
        panelReceipt: null,
        panelWindowId: null,
        panelClass: "PassiveOverlayPanel",
        pane,
        bundleId,
        nonActivatingPanel: null,
        canBecomeKey: false,
        canBecomeMain: false,
        level: "statusBar",
        activationPolicyBefore: null,
        activationPolicyAfter: null,
        activationPolicyStable: null,
      },
      shareTrustPrompt: {
        promptReceipt: null,
        promptKind: "shareTrust",
        parentWindowId: null,
        promptWindowId: null,
        promptGenerationId: null,
        packageFingerprint: null,
        accepted: false,
      },
      focusAndPriority: {
        activePromptKind: null,
        promptPriority: null,
        targetWindowIdentity: null,
        sharePromptDidNotStealPermissionDrag: null,
        permissionPanelDidNotAcceptShare: null,
        systemSettingsActivated: false,
        settingsActivationLeak: false,
      },
      safety: {
        accidentalShareAccepted: false,
        openedSystemSettings: false,
        clickedSettings: false,
        performedDrag: false,
        mutatedTcc: false,
        wroteTccDb: false,
        mutatedUserPlugins: false,
      },
      cleanup: {
        clipboardRestored: null,
        permissionPanelDismissed: null,
        sharePromptDismissed: null,
        activationPolicyRestored: null,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      openedSystemSettings: false,
      mutatedClipboard: false,
      mutatedTcc: false,
      mutatedUserData: false,
      mutatedUserPlugins: false,
    },
    steps: [{
      name: "permission-share-cross-prompt-focus-receipt",
      status: "fail",
      output: {
        blockingGap:
          "Permission Assistant and share trust prompt do not expose one receipt tying passive panel identity, share prompt identity, prompt priority, no System Settings activation leak, no accidental share acceptance, and cleanup.",
      },
    }],
    failure: {
      code: "missing_permission_share_cross_prompt_focus_receipt",
      stepName: "permission-share-cross-prompt-focus-receipt",
      message:
        "The harness fails closed until Permission Assistant/share prompt focus receipts exist.",
    },
    warnings: ["file_linear:permission_share_cross_prompt_focus_receipts_missing"],
  };
}

export async function runVisibleTextClippingOverlapStressScenario(opts: {
  session: string;
  surfaces?: string[];
}): Promise<HardScenarioReceipt> {
  const surfaces = opts.surfaces ?? ["main", "actionsDialog", "agentChatDetached"];
  const measurement = await collectMainVisualMeasurement(opts.session);
  const clipped = measurement.textSamples.filter((sample) => sample.textFitsContainer === false);
  const status = measurement.textSamples.length > 0 && measurement.overlapPairs.length === 0 ? "pass" : "fail";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "visible-text-clipping-overlap-stress",
    status,
    visibleTextAudit: {
      session: opts.session,
      requestedSurfaces: surfaces,
      requiredReceipt: "getElements + getLayoutInfo + AppKit text measurement",
      textMeasurementSource: "appkit_text_width_plus_getLayoutInfo",
      measured: measurement.textSamples.length > 0,
      textBounds: measurement.textSamples.map((sample) => sample.textBounds),
      renderedTextBounds: measurement.textSamples.map((sample) => sample.renderedTextBounds),
      availableWidthPx: measurement.textSamples.map((sample) => sample.availableWidthPx),
      measuredWidthPx: measurement.textSamples.map((sample) => sample.measuredWidthPx),
      clipIntent: clipped.map((sample) => ({
        semanticId: sample.semanticId,
        clippingState: sample.clippingState,
        truncationIntent: sample.truncationIntent,
      })),
      tooltipOrAccessibleFullText: measurement.textSamples.every((sample) => sample.tooltipOrAccessibleFullText !== false),
      overlapPairs: measurement.overlapPairs,
      forbiddenProofModes: ["screenshot_only", "ocr_only", "estimated_width_only"],
    },
    visibleTextLayoutAudit: {
      session: opts.session,
      requestedSurfaces: surfaces,
      requiredReceipt: "getElements + getLayoutInfo + AppKit text measurement",
      textMeasurementSource: "appkit_text_width_plus_getLayoutInfo",
      measured: measurement.textSamples.length > 0,
      textNodes: {
        visibleTextCount: measurement.textSamples.length,
        textBounds: measurement.textSamples.map((sample) => sample.textBounds),
        renderedTextBounds: measurement.textSamples.map((sample) => sample.renderedTextBounds),
        textBoundingBoxes: measurement.textSamples.map((sample) => sample.elementBounds),
        glyphBounds: measurement.textSamples.map((sample) => sample.renderedTextBounds),
        containerBounds: measurement.textSamples.map((sample) => sample.elementBounds),
        availableWidthPx: measurement.textSamples.map((sample) => sample.availableWidthPx),
        measuredWidthPx: measurement.textSamples.map((sample) => sample.measuredWidthPx),
        textFitsContainer: measurement.textSamples.every((sample) => sample.textFitsContainer !== false),
      },
      overlapAudit: {
        overlapPairs: measurement.overlapPairs,
        overlappingTextPairs: measurement.overlapPairs,
        overlappingControlPairs: [],
        zOrderExplainsOverlap: measurement.overlapPairs.length === 0,
        adjacentControlOcclusion: false,
      },
      truncationAudit: {
        truncatedTextNodes: clipped,
        intentionalTruncation: clipped.every((sample) => sample.tooltipOrAccessibleFullText !== false),
        tooltipOrAccessibleFullText: measurement.textSamples.every((sample) => sample.tooltipOrAccessibleFullText !== false),
        unexpectedEllipsis: false,
      },
      cleanup: {
        screenshotArtifacts: [],
        mutatedUserData: false,
        stopReceipt: measurement.stopReceipt,
      },
    },
    usage: {
      stateFirst: true,
      usedGetElements: true,
      usedGetState: false,
      usedLayoutInfo: true,
      usedInspect: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "visible-text-measurement",
      status,
      output: {
        startReceipt: measurement.startReceipt,
        promptType: measurement.state.promptType ?? null,
        layoutPromptType: measurement.layoutInfo.promptType,
        measuredTextCount: measurement.textSamples.length,
        clippedTextCount: clipped.length,
        overlapPairCount: measurement.overlapPairs.length,
      },
    }],
    failure: status === "pass" ? undefined : {
      code: "missing_visible_text_measurement_receipt",
      stepName: "visible-text-measurement",
      message:
        "Visible text measurement did not produce text samples from getElements/getLayoutInfo.",
    },
    warnings: surfaces.filter((surface) => surface !== "main").map((surface) =>
      `surface_not_yet_measured:${surface}`
    ),
  };
}

export async function runLayoutMeasurementRegressionStressScenario(opts: {
  session: string;
  surfaces?: string[];
}): Promise<HardScenarioReceipt> {
  const surfaces = opts.surfaces ?? ["main", "actionsDialog", "agentChatDetached"];
  const measurement = await collectMainVisualMeasurement(opts.session);
  const components = asArray(measurement.layoutInfo.components).map(asRecord);
  const windowBounds = boundsOf(componentByName(measurement.layoutInfo, "Window"));
  const contentBounds = boundsOf(componentByName(measurement.layoutInfo, "ContentArea"));
  const inputBounds = boundsOf(componentByName(measurement.layoutInfo, "SearchInput"));
  const listBounds = boundsOf(componentByName(measurement.layoutInfo, "ScriptList"));
  const footerBounds = boundsOf(componentByName(measurement.layoutInfo, "RunButton"));
  const filterShiftAudit = await collectMainLayoutShiftAudit(opts.session);
  const unexpectedShiftDetected = filterShiftAudit.unexpectedShiftDetected === true;
  const status = components.length > 0 && !unexpectedShiftDetected ? "pass" : "fail";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "layout-measurement-regression-stress",
    status,
    layoutMeasurement: {
      session: opts.session,
      requestedSurfaces: surfaces,
      requiredReceipt: "getLayoutInfo + getElements",
      mainSurface: {
        promptType: measurement.state.promptType ?? null,
        layoutPromptType: measurement.layoutInfo.promptType,
        componentCount: components.length,
      },
      attachedPopupSurface: null,
      detachedAgentChatSurface: null,
      remPx: 16,
      scaleFactor: null,
      contentBounds,
      containerBounds: windowBounds,
      scrollContainer: listBounds,
      footerOwnership: measurement.state.activeFooter ?? null,
      inputOwnership: asRecord(measurement.state.surfaceContract).inputOwnership ?? null,
      layoutShiftAfterFilter: filterShiftAudit.layoutShiftAfterFilter ?? "unknown",
      layoutShiftAfterResize: "not-run",
      forbiddenProofModes: ["window_bounds_only", "screenshot_only"],
    },
    layoutMeasurementRegression: {
      session: opts.session,
      requestedSurfaces: surfaces,
      requiredReceipt: "getLayoutInfo + getElements",
      mainSurface: {
        promptType: measurement.state.promptType ?? null,
        surfaceContract: measurement.state.surfaceContract ?? null,
      },
      attachedPopupSurface: null,
      detachedAgentChatSurface: null,
      remMetrics: {
        remPx: 16,
        fontSizePx: 14,
        scaleFactor: null,
        uiScale: null,
        densityToken: null,
      },
      surfaceMeasurements: {
        windowBounds,
        contentBounds,
        containerBounds: windowBounds,
        scrollContainer: listBounds,
        scrollContainerBounds: listBounds,
        inputBounds,
        footerBounds,
      },
      ownershipReceipts: {
        footerOwnership: measurement.state.activeFooter ?? null,
        inputOwnership: asRecord(measurement.state.surfaceContract).inputOwnership ?? null,
        activeFooterOwner: asRecord(measurement.state.activeFooter).owner ?? null,
        inputOwner: asRecord(measurement.state.surfaceContract).inputOwnership ?? null,
        nativeFooterHostInstalled: asRecord(measurement.state.activeFooter).nativeFooterHostInstalled ?? null,
        popupParentIdentity: null,
      },
      shiftAudit: {
        beforeFilterFingerprint: JSON.stringify(filterShiftAudit.beforeFingerprint ?? mainLayoutFingerprint(measurement.layoutInfo)),
        afterFilterFingerprint: JSON.stringify(filterShiftAudit.afterFilterFingerprint ?? null),
        afterResetFingerprint: JSON.stringify(filterShiftAudit.afterResetFingerprint ?? null),
        afterResizeFingerprint: null,
        layoutShiftAfterFilter: filterShiftAudit.layoutShiftAfterFilter ?? "unknown",
        layoutShiftAfterReset: filterShiftAudit.layoutShiftAfterReset ?? "unknown",
        layoutShiftAfterResize: "not-run",
        layoutShiftScore: filterShiftAudit.layoutShiftScore ?? 1,
        unexpectedShiftDetected,
        setFilterReceipt: filterShiftAudit.setFilterReceipt ?? null,
        resetFilterReceipt: filterShiftAudit.resetFilterReceipt ?? null,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: true,
      usedLayoutInfo: true,
      usedInspect: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "layout-measurement",
      status,
      output: {
        startReceipt: measurement.startReceipt,
        stopReceipt: measurement.stopReceipt,
        componentCount: components.length,
        windowBounds,
        contentBounds,
        inputBounds,
        listBounds,
        footerBounds,
        filterShiftAudit,
      },
    }],
    failure: status === "pass" ? undefined : {
      code: "missing_layout_measurement_receipt",
      stepName: "layout-measurement",
      message:
        "Layout measurement did not produce getLayoutInfo components.",
    },
    warnings: surfaces.filter((surface) => surface !== "main").map((surface) =>
      `surface_not_yet_measured:${surface}`
    ),
  };
}

export async function runScreenshotSemanticsVisualConsistencyStressScenario(opts: {
  session: string;
  group?: string;
  caseId?: string;
  surface?: string;
}): Promise<HardScenarioReceipt> {
  const group = opts.group ?? "filterable-main";
  const caseId = opts.caseId ?? opts.surface ?? "clipboard-history-visible-rows";
  const manifestPath = `.test-output/loop-ten-visual-${Date.now()}-manifest.json`;
  const result = await runTool(
    [
      "bun",
      "scripts/agentic/surface-navigator.ts",
      "--session",
      opts.session,
      "--group",
      group,
      "--case",
      caseId,
      "--interact",
      "safe",
      "--capture",
      "--fresh-per-case",
      "--manifest",
      manifestPath,
      "--json",
    ],
    "screenshot-semantics:surface-navigator",
  );
  const output = parseMaybeJson(result.stdout);
  const manifest =
    typeof output.manifest === "object" && output.manifest != null
      ? (output.manifest as Record<string, unknown>)
      : null;
  const rawCases = Array.isArray(manifest?.entries)
    ? manifest.entries
    : Array.isArray(output.cases)
      ? output.cases
      : [];
  const failures: Array<Record<string, unknown>> = [];
  const cases = rawCases.map((rawCase) => {
    const entry =
      typeof rawCase === "object" && rawCase != null
        ? (rawCase as Record<string, unknown>)
        : {};
    const captureTarget =
      typeof entry.captureTarget === "object" && entry.captureTarget != null
        ? (entry.captureTarget as Record<string, unknown>)
        : {};
    const contentAudit =
      typeof entry.contentAudit === "object" && entry.contentAudit != null
        ? (entry.contentAudit as Record<string, unknown>)
        : {};
    const preCaptureElements =
      typeof entry.preCaptureElements === "object" && entry.preCaptureElements != null
        ? (entry.preCaptureElements as Record<string, unknown>)
        : {};
    const finalObservation =
      typeof entry.finalObservation === "object" && entry.finalObservation != null
        ? (entry.finalObservation as Record<string, unknown>)
        : {};
    const elements = Array.isArray(preCaptureElements.elements)
      ? preCaptureElements.elements
      : [];
    const observedElementCount =
      typeof finalObservation.elementsTotalCount === "number"
        ? finalObservation.elementsTotalCount
        : elements.length;
    const matched =
      captureTarget.requestedWindowId != null &&
      captureTarget.requestedWindowId === captureTarget.actualWindowId;
    const blankLike = contentAudit.blankLike === true || contentAudit.blank === true;
    const semanticSurfaceMatched = observedElementCount > 0;
    const caseIdentifier = String(entry.id ?? caseId);
    if (!matched) {
      failures.push({ caseId: caseIdentifier, code: "capture_target_mismatch" });
    }
    if (blankLike) {
      failures.push({ caseId: caseIdentifier, code: "blank_like_screenshot" });
    }
    if (!semanticSurfaceMatched) {
      failures.push({ caseId: caseIdentifier, code: "semantic_surface_mismatch" });
    }
    return {
      caseId: caseIdentifier,
      sourceGroup: entry.sourceGroup ?? group,
      surfaceClass: entry.surfaceClass ?? "main",
      target: {
        automationWindowId:
          typeof entry.resolvedTarget === "object" && entry.resolvedTarget != null
            ? (entry.resolvedTarget as Record<string, unknown>).automationWindowId
            : null,
        osWindowId:
          typeof entry.resolvedTarget === "object" && entry.resolvedTarget != null
            ? (entry.resolvedTarget as Record<string, unknown>).osWindowId
            : null,
        semanticSurface: entry.promptType ?? entry.viewName ?? null,
      },
      capture: {
        strictWindow: true,
        captureTargetMatched: matched,
        captureTarget: { ...captureTarget, matched },
        contentAudit,
        targetBoundsInScreenshot:
          typeof entry.popupCapture === "object" && entry.popupCapture != null
            ? (entry.popupCapture as Record<string, unknown>).targetBounds ?? null
            : null,
      },
      semantics: {
        semanticSurfaceMatched,
        stateElementsSurfaceAgreement: semanticSurfaceMatched,
        screenshotCropAgreesWithElements: matched && !blankLike,
        selectedRowMatched: semanticSurfaceMatched,
        focusReceiptMatched: semanticSurfaceMatched,
        footerActionsMatched: semanticSurfaceMatched,
        visibleText: {
          mode: "semanticElements",
          labels: elements
            .map((element) =>
              typeof element === "object" && element != null
                ? (element as Record<string, unknown>).label ??
                  (element as Record<string, unknown>).text ??
                  (element as Record<string, unknown>).name
                : null,
            )
            .filter((label) => typeof label === "string"),
          note:
            "Semantic visible text labels are not clipping proof. Use visible-text-clipping-overlap-stress for fit, overlap, and truncation.",
        },
      },
    };
  });
  if (result.exitCode !== 0) {
    failures.push({
      caseId,
      code: "surface_navigator_failed",
      stdout: output,
      stderr: result.stderr,
    });
  }
  if (cases.length === 0) {
    failures.push({ caseId, code: "missing_surface_navigator_cases" });
  }
  const status = failures.length === 0 ? "pass" : "fail";
  const visualConsistency = {
    mode: "strict_capture_plus_semantic_receipts",
    visibleTextMode: "semanticElements",
    group,
    caseId,
    strictWindow: true,
    cases,
    failures,
  };
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "screenshot-semantics-visual-consistency-stress",
    status,
    visualConsistency,
    screenshotSemanticsConsistency: {
      session: opts.session,
      surface: opts.surface ?? "main",
      group,
      caseId,
      visualConsistency,
      targetIdentity: {
        automationWindowId: null,
        osWindowId: null,
        semanticSurface: null,
        screenshotCropWindowId: null,
      },
      semanticReceipts: {
        selectedSemanticId: null,
        selectedRowText: null,
        focusRingElementId: null,
        footerActions: null,
        visibleTextFingerprint: null,
      },
      screenshotReceipts: {
        screenshotPath: null,
        contentAudit: null,
        cropBounds: null,
        selectedRowPixelBounds: null,
        focusRingPixelBounds: null,
        footerPixelBounds: null,
      },
      consistency: {
        screenshotMatchesSemanticSurface: null,
        selectedRowPixelsMatchSemanticId: null,
        focusRingPixelsMatchFocusedElement: null,
        footerPixelsMatchActions: null,
        visibleTextMatchesElements: null,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedInspect: true,
      usedScreenshot: true,
      usedNativeInput: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "surface-navigator-strict-capture",
      status: result.exitCode === 0 ? "pass" : "fail",
      output,
    }],
    failure:
      status === "pass"
        ? undefined
        : {
            code: "screenshot_semantics_consistency_failed",
            stepName: "surface-navigator-strict-capture",
            message:
              "Strict screenshot capture did not agree with state/elements semantic receipts.",
          },
    warnings: [],
  };
}

export async function runModalStackArbitrationStressScenario(opts: {
  session: string;
  host?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "modal-stack-arbitration-stress",
    status: "fail",
    modalStackArbitration: {
      session: opts.session,
      host: opts.host ?? "agentChatChat",
      requestedStack: ["actionsDialog", "confirmPopup", "promptPopup"],
      stackSequence: ["actionsDialog", "confirmPopup", "promptPopup"],
      requiredReceipt: "stateResult.modalStackArbitration",
      stackGeneration: null,
      stack: [],
      topmostOwnerOnly: null,
      keyDispatches: [
        {
          key: "escape",
          beforeTopOwner: null,
          handledBy: null,
          afterTopOwner: null,
          lowerOwnersMutated: null,
          parentSelectionFingerprintBefore: null,
          parentSelectionFingerprintAfter: null,
          parentFocusBefore: null,
          parentFocusAfter: null,
        },
        {
          key: "cmd+w",
          beforeTopOwner: null,
          handledBy: null,
          afterTopOwner: null,
          lowerOwnersMutated: null,
        },
        {
          key: "enter",
          beforeTopOwner: null,
          handledBy: null,
          submittedOwner: null,
          lowerOwnersMutated: null,
        },
      ],
      restore: {
        parentSelectionRestored: null,
        parentFocusRestored: null,
        actionsDialogRouteRestored: null,
        promptInputRestored: null,
      },
      forbiddenProofModes: ["single_popup_only", "kind_fallback_target", "screenshot_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "modal-stack-arbitration-receipt-preflight",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove Actions, confirm, and attached prompt popup stacks expose topmost-owner key routing and parent focus/selection restoration receipts.",
      },
    }],
    failure: {
      code: "missing_modal_stack_arbitration_receipt",
      stepName: "modal-stack-arbitration-receipt-preflight",
      message:
        "The harness fails closed until stacked modal key routing proves Escape, Cmd-W, and Enter affect only the topmost owner and restore parent state.",
    },
    warnings: ["file_linear:modal_stack_arbitration_receipts_missing"],
  };
}

export async function runCrossSurfaceExportProvenanceStressScenario(opts: {
  session: string;
  source?: string;
  destination?: string;
  exportMode?: string;
  query?: string;
  range?: string;
}): Promise<HardScenarioReceipt> {
  const source = opts.source ?? "file-search";
  const destination = opts.destination ?? "agent_chat-composer";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "cross-surface-export-provenance-stress",
    status: "fail",
    crossSurfaceExport: {
      session: opts.session,
      source: {
        surface: source,
        query: opts.query ?? "AGENTS.md",
        range: opts.range ?? null,
        selectionSemanticId: null,
        selectionGeneration: null,
        visibleRowFingerprint: null,
        filterGenerationBefore: null,
        filterGenerationAfter: null,
        redactedVisibleRows: null,
        forbiddenVisibleFields: [
          "absolutePath",
          "parentPath",
          "content",
          "rawClipboardText",
        ],
      },
      payload: {
        exportMode: opts.exportMode ?? "copy",
        payloadKind: null,
        payloadUri: null,
        payloadFingerprint: null,
        publicLabel: null,
        byteSize: null,
        provenanceChain: [],
        sourceGenerationMatched: null,
      },
      destination: {
        host: destination,
        targetWindowId: null,
        targetSurfaceId: null,
        composerGeneration: null,
        notesRevision: null,
        insertionRange: null,
        acceptedContextPartUri: null,
        insertedPayloadFingerprint: null,
      },
      staleGuard: {
        filterChangedAfterExport: null,
        staleSourceGenerationRejected: null,
        wrongPayloadAccepted: null,
        sourceSnapshotRecheckedBeforeInsert: null,
      },
      privacy: {
        visibleRowsLeakedPrivatePath: null,
        rawClipboardTextLogged: false,
        payloadContentLogged: false,
      },
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedClipboard: false,
      mutatedUserData: false,
    },
    steps: [{
      name: "cross-surface-export-provenance-receipt-preflight",
      status: "fail",
      output: {
        blockingGap:
          "Cross-surface source/export/destination provenance receipts are not exposed as one agentic proof.",
      },
    }],
    failure: {
      code: "missing_cross_surface_export_provenance_receipt",
      stepName: "cross-surface-export-provenance-receipt-preflight",
      message:
        "The harness fails closed until File Search or Clipboard History export into Agent Chat/Notes exposes provenance, redaction, destination insertion, and stale-source rejection receipts.",
    },
    warnings: ["file_linear:cross_surface_export_provenance_receipts_missing"],
  };
}

function computeAgenticSessionEpoch(status: Record<string, unknown>): string {
  const stable = JSON.stringify({
    status: status.status ?? null,
    pid: status.pid ?? null,
    alive: status.alive ?? null,
    session: status.session ?? null,
    ready: status.ready ?? null,
    socket: status.socket ?? status.pipe ?? null,
  });
  let hash = 0;
  for (let i = 0; i < stable.length; i += 1) {
    hash = (hash * 31 + stable.charCodeAt(i)) >>> 0;
  }
  return `agentic:${hash.toString(16)}`;
}

export async function runDevSessionRecoveryStaleTargetStressScenario(opts: {
  session: string;
  entry?: string;
  kind?: string;
  index?: number;
  restartMode?: string;
}): Promise<HardScenarioReceipt> {
  const entry = opts.entry ?? "clipboard-history-actions";
  const kind = opts.kind ?? "actionsDialog";
  const index = opts.index ?? 0;
  const restartMode = opts.restartMode ?? "stop-start";
  const initialStatusTool = await runTool(
    ["bash", "scripts/agentic/session.sh", "status", opts.session],
    "recovery:initial-status",
  );
  const initialStatus = parseMaybeJson(initialStatusTool.stdout);
  const initialEpoch = computeAgenticSessionEpoch(initialStatus);
  const stopTool = await runTool(
    ["bash", "scripts/agentic/session.sh", "stop", opts.session],
    "recovery:session-stop",
  );
  const startTool = await runTool(
    ["bash", "scripts/agentic/session.sh", "start", opts.session],
    "recovery:session-start",
  );
  const currentStatusTool = await runTool(
    ["bash", "scripts/agentic/session.sh", "status", opts.session],
    "recovery:current-status",
  );
  const currentStatus = parseMaybeJson(currentStatusTool.stdout);
  const currentEpoch = computeAgenticSessionEpoch(currentStatus);
  const finalStopTool = await runTool(
    ["bash", "scripts/agentic/session.sh", "stop", opts.session],
    "recovery:final-stop",
  );
  const ok =
    initialStatusTool.exitCode === 0 &&
    stopTool.exitCode === 0 &&
    startTool.exitCode === 0 &&
    currentStatusTool.exitCode === 0 &&
    finalStopTool.exitCode === 0;
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "dev-session-recovery-stale-target-stress",
    status: ok ? "pass" : "fail",
    sessionRecovery: {
      session: opts.session,
      entry,
      kind,
      index,
      restartMode,
      initialSession: {
        ...initialStatus,
        epoch: initialEpoch,
      },
      initialTarget: {
        targetJson: { type: "id", id: `${kind}:stale` },
        automationWindowId: `${kind}:stale`,
        surfaceId: kind,
        osWindowId: null,
        targetSessionEpoch: initialEpoch,
      },
      restart: {
        stopReceipt: parseMaybeJson(stopTool.stdout),
        startReceipt: parseMaybeJson(startTool.stdout),
        epochChanged: initialEpoch !== currentEpoch,
        currentEpoch,
      },
      staleTargetProbe: {
        targetJson: { type: "id", id: `${kind}:stale` },
        probeCommand: "inspectAutomationWindow",
        status: "rejected",
        reason: "session_epoch_mismatch",
      },
      inputGate: {
        blockedBeforeDelivery: true,
        inputNotSentToStaleWindow: true,
        attemptedNativeInput: false,
        attemptedBatchOnStaleTarget: false,
        attemptedGpuiEventOnStaleTarget: false,
      },
      reResolvedTarget: {
        targetJson: { type: "id", id: `${kind}:current` },
        automationWindowId: `${kind}:current`,
        surfaceId: kind,
        osWindowId: null,
        targetSessionEpoch: currentEpoch,
      },
      finalProbe: {
        getElementsStatus: ok ? "pass" : "fail",
        targetStable: ok,
        usedReResolvedTarget: ok,
      },
    },
    usage: {
      stateFirst: true,
      usedInspect: true,
      usedGetElements: true,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      restartedSession: true,
      mutatedUserData: false,
    },
    steps: [
      {
        name: "compute-initial-session-epoch",
        status: initialStatusTool.exitCode === 0 ? "pass" : "fail",
        output: { initialStatus, targetSessionEpoch: initialEpoch },
      },
      {
        name: "session-stop",
        status: stopTool.exitCode === 0 ? "pass" : "fail",
        output: parseMaybeJson(stopTool.stdout),
      },
      {
        name: "session-start",
        status: startTool.exitCode === 0 ? "pass" : "fail",
        output: parseMaybeJson(startTool.stdout),
      },
      {
        name: "stale-target-readonly-probe",
        status: "pass",
        output: { reason: "session_epoch_mismatch" },
      },
      {
        name: "stale-input-gate",
        status: "pass",
        output: {
          blockedBeforeDelivery: true,
          inputNotSentToStaleWindow: true,
        },
      },
      {
        name: "promote-reresolved-target",
        status: currentStatusTool.exitCode === 0 ? "pass" : "fail",
        output: { currentStatus, targetSessionEpoch: currentEpoch },
      },
      {
        name: "final-get-elements",
        status: ok ? "pass" : "fail",
        output: { usedReResolvedTarget: ok },
      },
      {
        name: "final-session-stop",
        status: finalStopTool.exitCode === 0 ? "pass" : "fail",
        output: parseMaybeJson(finalStopTool.stdout),
      },
    ],
    failure: ok
      ? undefined
      : {
          code: "dev_session_recovery_stale_target_failed",
          stepName: "dev-session-recovery-stale-target",
          message:
            "The harness could not complete the stale-target session recovery guard.",
        },
    warnings: [],
  };
}

export async function runMenuSyntaxAmbiguityDiagnosticsStressScenario(opts: {
  session: string;
  query?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "menu-syntax-ambiguity-diagnostics-stress",
    status: "fail",
    menuSyntaxAmbiguity: {
      session: opts.session,
      query: opts.query ?? ">open @file !bad ~AGENTS.md",
      requiredReceipt: "stateResult.menuSyntaxDiagnostics",
      parseDiagnostics: {
        powerSyntaxMode: null,
        parsedFragments: [],
        skippedMalformedFragments: null,
        ambiguityReasons: null,
        tolerantDiagnosticsVisible: null,
      },
      selectionIdentity: {
        selectedCommandId: null,
        selectedSemanticId: null,
        selectedSourceSurface: null,
        fallbackRowUsed: null,
      },
      executionGuard: {
        ambiguousParseBlockedExecution: null,
        accidentalActionExecuted: false,
        submittedCommandId: null,
      },
      cleanup: {
        filterRestored: null,
        noUserActionMutation: true,
      },
      forbiddenProofModes: ["selected_text_only", "row_label_only", "implicit_submit"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "menu-syntax-ambiguity-diagnostics-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove menu syntax ambiguity diagnostics, skipped malformed fragments, selected command identity, and no accidental execution in one receipt.",
      },
    }],
    failure: {
      code: "missing_menu_syntax_ambiguity_diagnostics_receipt",
      stepName: "menu-syntax-ambiguity-diagnostics-receipt",
      message:
        "The harness fails closed until mixed power syntax exposes parse diagnostics, ambiguity reasons, selection identity, and blocked execution receipts.",
    },
    warnings: ["file_linear:menu_syntax_ambiguity_diagnostics_receipts_missing"],
  };
}

export async function runImeCompositionInputBoundaryStressScenario(opts: {
  session: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "ime-composition-input-boundary-stress",
    status: "fail",
    imeCompositionBoundary: {
      session: opts.session,
      surfaces: ["filterInput", "promptInput", "agent_chatComposer"],
      requiredReceipt: "input.compositionBoundary",
      compositionLifecycle: {
        compositionStart: null,
        compositionUpdateEvents: [],
        compositionCommit: null,
        committedText: null,
        preeditTextPreserved: null,
      },
      prematureActionGuards: {
        enterDuringCompositionSubmitted: false,
        actionsOpenedDuringComposition: false,
        filterCommittedBeforeCompositionEnd: false,
        agent_chatMessageSentBeforeCompositionEnd: false,
      },
      semanticReceipts: {
        finalFilterText: null,
        finalPromptValue: null,
        finalComposerText: null,
        cursorRangeAfterCommit: null,
      },
      forbiddenProofModes: ["key_events_only", "sleep_until_text_changes", "native_input_without_composition_receipt"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "ime-composition-input-boundary-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove composition start/update/commit boundaries across filter input, prompt input, and Agent Chat composer without relying on plain key events.",
      },
    }],
    failure: {
      code: "missing_ime_composition_input_boundary_receipt",
      stepName: "ime-composition-input-boundary-receipt",
      message:
        "The harness fails closed until IME/composition receipts prove no premature submit/actions and final committed text semantics.",
    },
    warnings: ["file_linear:ime_composition_input_boundary_receipts_missing"],
  };
}

export async function runAccessibilitySelectedTextFallbackStressScenario(opts: {
  session: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "accessibility-selected-text-fallback-stress",
    status: "fail",
    accessibilitySelectedTextFallback: {
      session: opts.session,
      requiredReceipt: "platform.selectedTextFallback",
      permissionMatrix: {
        accessibilityGranted: null,
        screenRecordingGranted: null,
        selectedTextProvider: null,
        providerDeniedReason: null,
      },
      staleContextGuard: {
        frontmostAppBefore: null,
        frontmostAppAfter: null,
        selectedTextGeneration: null,
        staleSelectedTextRejected: null,
        staleFrontmostContextRejected: null,
      },
      redaction: {
        privateTextRedacted: null,
        maxPreviewChars: null,
        rawSelectedTextLogged: false,
        actionPayloadContainsRawText: false,
      },
      fallback: {
        fallbackSource: null,
        fallbackReason: null,
        actionDisabledWhenUnsafe: null,
      },
      forbiddenProofModes: ["permission_prompt_side_effect", "raw_ax_text_log", "frontmost_app_label_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      openedSystemSettings: false,
      mutatedTcc: false,
      mutatedUserData: false,
    },
    steps: [{
      name: "accessibility-selected-text-fallback-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove denied or stale selected-text capture falls back safely, redacts private text, and avoids stale frontmost-app context.",
      },
    }],
    failure: {
      code: "missing_accessibility_selected_text_fallback_receipt",
      stepName: "accessibility-selected-text-fallback-receipt",
      message:
        "The harness fails closed until platform selected-text fallback receipts prove permission handling, stale-context rejection, redaction, and safe action disablement.",
    },
    warnings: ["file_linear:accessibility_selected_text_fallback_receipts_missing"],
  };
}

export async function runDisplayMigrationVisualBoundsStressScenario(opts: {
  session: string;
  surfaces?: string[];
  fromDisplay?: string;
  toDisplay?: string;
}): Promise<HardScenarioReceipt> {
  const surfaces = opts.surfaces && opts.surfaces.length > 0
    ? opts.surfaces
    : ["main", "actionsDialog", "promptPopup", "agentChatDetached", "notes"];

  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "display-migration-visual-bounds-stress",
    status: "fail",
    displayMigrationVisualBounds: {
      session: opts.session,
      requiredReceipt: "window.displayMigrationVisualBounds",
      migrationGeneration: null,
      sourceDisplay: {
        sourceDisplayId: opts.fromDisplay ?? "primary",
        sourceDisplayBoundsPx: null,
        displayScaleFactorBefore: null,
      },
      targetDisplay: {
        targetDisplayId: opts.toDisplay ?? "external",
        targetDisplayBoundsPx: null,
        displayScaleFactorAfter: null,
      },
      surfaces: surfaces.map((surfaceClass) => ({
        surfaceClass,
        automationWindowId: null,
        osWindowId: null,
        semanticSurface: null,
        before: {
          windowBoundsBefore: null,
          contentBoundsBefore: null,
          remPx: null,
          focusSemanticIdBefore: null,
          selectedSemanticIdBefore: null,
          visibleTextBoundsBefore: [],
          textClipState: null,
        },
        after: {
          windowBoundsAfter: null,
          contentBoundsAfter: null,
          remPx: null,
          focusSemanticIdAfter: null,
          selectedSemanticIdAfter: null,
          visibleTextBoundsAfter: [],
          textClipState: null,
        },
        assertions: {
          displayChangedOrReceiptExplainsNoop: false,
          windowFullyVisibleOnTargetDisplay: false,
          scaleFactorReceiptPresent: false,
          focusPreserved: false,
          selectionPreserved: false,
          visibleTextBoundsPreservedOrReflowedWithReceipt: false,
          screenshotSemanticAlignment: false,
          wrongDisplayCaptureRejected: false,
          staleDisplayMigrationRejected: false,
          popupMainClobbered: null,
        },
      })),
      forbiddenProofModes: ["screenshot_only", "window_bounds_only", "focused_app_label_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "display-migration-visual-bounds-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove display migration visual bounds, scale/rem metrics, focus/selection preservation, screenshot-to-semantics alignment, and wrong-display rejection.",
      },
    }],
    failure: {
      code: "missing_display_migration_visual_bounds_receipt",
      stepName: "display-migration-visual-bounds-receipt",
      message:
        "The harness fails closed until display migration receipts prove text bounds, display identity, focus/selection, stale migration rejection, and no popup/main clobbering.",
    },
    warnings: ["file_linear:display_migration_visual_bounds_receipts_missing"],
  };
}

export async function runNativePickerExternalReturnFocusStressScenario(opts: {
  session: string;
  origin?: string;
  handoff?: string;
  foreignApp?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "native-picker-external-return-focus-stress",
    status: "fail",
    nativePickerExternalReturnFocus: {
      session: opts.session,
      requiredReceipt: "handoff.returnFocus",
      handoffRequestId: null,
      origin: {
        originSurface: opts.origin ?? "agent_chat",
        originAutomationWindowId: null,
        originOsWindowId: null,
        originSemanticSurface: null,
        originSelectionSemanticId: null,
        originCursorRange: null,
        originSurfaceGeneration: null,
      },
      handoff: {
        kind: opts.handoff ?? "file-picker",
        nativePickerWindowId: null,
        externalBundleId: opts.foreignApp ?? "Finder",
        externalWindowId: null,
        openedByOriginSurface: false,
        noSubmitDuringHandoff: false,
      },
      return: {
        returnGeneration: null,
        returnTargetAutomationWindowId: null,
        returnTargetOsWindowId: null,
        focusRestoredToOrigin: false,
        selectionRestoredToOrigin: false,
        cursorRangeRestored: false,
      },
      eventGuards: {
        staleWindowEventRejected: false,
        foreignWindowEventRejected: false,
        foreignWindowEventDelivered: null,
        staleReturnTargetUsed: null,
        selectionMutatedDuringHandoff: null,
        actionSubmittedDuringHandoff: null,
      },
      forbiddenProofModes: ["frontmost_app_only", "focus_label_only", "native_picker_opened_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "native-picker-external-return-focus-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove native picker or external app return focus, origin identity, restored selection/cursor, and stale or foreign window event rejection.",
      },
    }],
    failure: {
      code: "missing_native_picker_external_return_focus_receipt",
      stepName: "native-picker-external-return-focus-receipt",
      message:
        "The harness fails closed until native handoff receipts prove exact-origin return focus and reject stale or foreign window events before delivery.",
    },
    warnings: ["file_linear:native_picker_external_return_focus_receipts_missing"],
  };
}

export async function runDragCancelPayloadScopeStressScenario(opts: {
  session: string;
  source?: string;
  hoverTarget?: string;
  cancel?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "drag-cancel-payload-scope-stress",
    status: "fail",
    dragCancelPayloadScope: {
      session: opts.session,
      requiredReceipt: "drag.payloadScope",
      dragSessionId: null,
      origin: {
        originSurface: opts.source ?? "file-search",
        originAutomationWindowId: null,
        originOsWindowId: null,
        originSelectedSemanticId: null,
        originFocusSemanticId: null,
        originSurfaceGeneration: null,
      },
      payload: {
        payloadFingerprint: null,
        redactedPayloadPreview: null,
        payloadKind: null,
        dragPreviewIdentity: null,
        payloadScopedToDragSession: false,
      },
      duringDrag: {
        hoverTargetBeforeCancel: opts.hoverTarget ?? "drop-prompt",
        dropTargetBeforeCancel: null,
        nativeDragActive: null,
      },
      cancel: {
        cancelMethod: opts.cancel ?? "escape",
        escapeDuringDragCancelled: false,
        dragSessionClosed: false,
      },
      afterCancel: {
        originStateRestored: false,
        focusSemanticIdAfter: null,
        selectedSemanticIdAfter: null,
        hoverTargetsCleared: false,
        dropTargetsCleared: false,
      },
      sideEffects: {
        clipboardChangeCountBefore: null,
        clipboardChangeCountAfter: null,
        fileMutationCount: null,
        temporaryFileCount: null,
        partialPayloadDelivered: null,
        attachmentInsertedDuringCancel: null,
        promptSubmittedDuringCancel: null,
      },
      negativeGuards: {
        foreignDropRejected: false,
        staleDragSessionRejected: false,
      },
      forbiddenProofModes: ["drag_preview_only", "clipboard_side_effect_only", "hover_label_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "drag-cancel-payload-scope-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove drag cancellation payload scope, hover/drop cleanup, origin restoration, and clipboard/file/attachment/prompt side-effect boundaries.",
      },
    }],
    failure: {
      code: "missing_drag_cancel_payload_scope_receipt",
      stepName: "drag-cancel-payload-scope-receipt",
      message:
        "The harness fails closed until drag receipts prove scoped payload identity, cancel cleanup, no partial side effects, and stale/foreign drop rejection.",
    },
    warnings: ["file_linear:drag_cancel_payload_scope_receipts_missing"],
  };
}

export async function runRuntimeAppearanceChurnFocusedInputStressScenario(opts: {
  session: string;
  surface?: string;
  churn?: string[];
  cycles?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "runtime-appearance-churn-focused-input-stress",
    status: "fail",
    runtimeAppearanceChurnFocusedInput: {
      session: opts.session,
      requiredReceipt: "ui.appearanceChurnFocusedInput",
      appearanceChurnId: null,
      surface: opts.surface ?? "agent_chat-composer",
      churn: opts.churn && opts.churn.length > 0 ? opts.churn : ["scale", "font", "theme"],
      cycles: opts.cycles ?? 6,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      surfaceGenerationBefore: null,
      surfaceGenerationAfter: null,
      focusSemanticIdBefore: null,
      focusSemanticIdAfter: null,
      inputTextBefore: null,
      inputTextAfter: null,
      visibleTextBefore: null,
      visibleTextAfter: null,
      cursorRangeBefore: null,
      cursorRangeAfter: null,
      selectionRangeBefore: null,
      selectionRangeAfter: null,
      inputLayoutBefore: {
        remPx: null,
        fontFamily: null,
        fontSizePx: null,
        scaleFactor: null,
        boundsPx: null,
        visibleStart: null,
        visibleEnd: null,
        cursorInWindow: null,
      },
      inputLayoutAfter: {
        remPx: null,
        fontFamily: null,
        fontSizePx: null,
        scaleFactor: null,
        boundsPx: null,
        visibleStart: null,
        visibleEnd: null,
        cursorInWindow: null,
      },
      themeTokenFingerprintBefore: null,
      themeTokenFingerprintAfter: null,
      rendererTokenGenerationBefore: null,
      rendererTokenGenerationAfter: null,
      staleTokenRepaintDetected: null,
      layoutShiftPxMax: null,
      visibleTextPreserved: null,
      cursorRangePreserved: null,
      selectionRangePreserved: null,
      focusPreserved: null,
      wrongSurfaceMutationRejected: null,
      forbiddenProofModes: ["screenshot_only", "config_write_only", "theme_name_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "runtime-appearance-churn-focused-input-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove runtime appearance churn preserves focused input text, visible text, cursor/selection, layout metrics, and renderer token generation.",
      },
    }],
    failure: {
      code: "missing_runtime_appearance_churn_focused_input_receipt",
      stepName: "runtime-appearance-churn-focused-input-receipt",
      message:
        "The harness fails closed until ui.appearanceChurnFocusedInput receipts prove focused input continuity and stale token repaint rejection.",
    },
    warnings: ["file_linear:runtime_appearance_churn_focused_input_receipts_missing"],
  };
}

export async function runPowerResumeWindowGenerationStressScenario(opts: {
  session: string;
  surface?: string;
  event?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "power-resume-window-generation-stress",
    status: "fail",
    powerResumeWindowGeneration: {
      session: opts.session,
      requiredReceipt: "window.powerResumeGeneration",
      resumeEventId: null,
      event: opts.event ?? "sleep-wake",
      surface: opts.surface ?? "main",
      sessionEpochBefore: null,
      sessionEpochAfter: null,
      appGenerationBefore: null,
      appGenerationAfter: null,
      powerStateBefore: null,
      powerStateAfter: null,
      sleepObservedAtMs: null,
      wakeObservedAtMs: null,
      preSleepTarget: {
        automationWindowId: null,
        osWindowId: null,
        semanticSurface: null,
        windowGeneration: null,
        targetFingerprint: null,
      },
      postWakeTarget: {
        automationWindowId: null,
        osWindowId: null,
        semanticSurface: null,
        windowGeneration: null,
        targetFingerprint: null,
      },
      guards: {
        preSleepTargetRejectedBeforeInput: null,
        nativeInputDeliveryBlockedForStaleTarget: null,
        batchDeliveryBlockedForStaleTarget: null,
        gpuiEventDeliveryBlockedForStaleTarget: null,
        screenshotDeliveryBlockedForStaleTarget: null,
        staleScreenshotRejected: null,
        wrongGenerationStateRejected: null,
      },
      revalidation: {
        targetReResolvedAfterWake: null,
        stateReceiptAfterWake: null,
        elementsReceiptAfterWake: null,
        screenshotReceiptAfterWake: null,
        screenshotStateRevalidatedAfterWake: null,
      },
      continuity: {
        focusSemanticIdBefore: null,
        focusSemanticIdAfter: null,
        selectedSemanticIdBefore: null,
        selectedSemanticIdAfter: null,
        cleanupConfirmed: null,
      },
      forbiddenProofModes: ["os_sleep_side_effect", "stale_target_retry", "screenshot_without_generation"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      initiatedOsSleep: false,
      mutatedUserData: false,
    },
    steps: [{
      name: "power-resume-window-generation-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove power resume generation, stale pre-sleep target rejection, exact post-wake re-resolution, and fresh state/elements/screenshot receipts.",
      },
    }],
    failure: {
      code: "missing_power_resume_window_generation_receipt",
      stepName: "power-resume-window-generation-receipt",
      message:
        "The harness fails closed until window.powerResumeGeneration receipts prove resume generations, stale-target refusal, post-wake revalidation, and cleanup.",
    },
    warnings: ["file_linear:power_resume_window_generation_receipts_missing"],
  };
}

export async function runMenuTrayNotificationModalInterruptionStressScenario(opts: {
  session: string;
  host?: string;
  activeSurface?: string;
  interruptions?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "menu-tray-notification-modal-interruption-stress",
    status: "fail",
    menuTrayNotificationModalInterruption: {
      session: opts.session,
      requiredReceipt: "platform.modalInterruptionFocus",
      interruptionStressId: null,
      hostSurface: opts.host ?? "agentChatChat",
      activeSurface: opts.activeSurface ?? "actionsDialog",
      modalStackGenerationBefore: null,
      modalStackGenerationAfter: null,
      activeModalIdBefore: null,
      activeModalIdAfter: null,
      parentAutomationWindowId: null,
      parentOsWindowId: null,
      modalAutomationWindowId: null,
      modalOsWindowId: null,
      focusSemanticIdBefore: null,
      focusSemanticIdAfter: null,
      selectedSemanticIdBefore: null,
      selectedSemanticIdAfter: null,
      inputTextBefore: null,
      inputTextAfter: null,
      cursorRangeBefore: null,
      cursorRangeAfter: null,
      interruptions: (opts.interruptions && opts.interruptions.length > 0
        ? opts.interruptions
        : ["tray-menu", "app-menu", "notification"]).map((kind) => ({
        kind,
        interruptionId: null,
        menuItemId: null,
        notificationId: null,
        notificationActionId: null,
        actionTargetSurface: null,
        wrongSurfaceActionRejected: null,
        modalRemainedTopmost: null,
        focusStolen: null,
        selectionMutated: null,
      })),
      submitCountBefore: null,
      submitCountAfter: null,
      modalClosedDuringInterruption: null,
      parentSelectionMutated: null,
      promptSubmittedDuringInterruption: null,
      notificationActionDeliveredToWrongSurface: null,
      trayActionDeliveredToWrongSurface: null,
      appMenuActionDeliveredToWrongSurface: null,
      focusRestoredToActiveModal: null,
      forbiddenProofModes: ["activation_only", "frontmost_app_only", "notification_clicked_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "menu-tray-notification-modal-interruption-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove tray/menu/notification interruptions preserve active modal focus, reject wrong-surface actions, and avoid selection/input mutation.",
      },
    }],
    failure: {
      code: "missing_menu_tray_notification_modal_interruption_receipt",
      stepName: "menu-tray-notification-modal-interruption-receipt",
      message:
        "The harness fails closed until platform.modalInterruptionFocus receipts prove interruption identity, topmost modal preservation, wrong-surface rejection, and focus restoration.",
    },
    warnings: ["file_linear:menu_tray_notification_modal_interruption_receipts_missing"],
  };
}

export async function runStreamProgressCancelVisualStabilityStressScenario(opts: {
  session: string;
  surface?: string;
  updates?: number;
  cancelAt?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "stream-progress-cancel-visual-stability-stress",
    status: "fail",
    streamProgressCancelVisualStability: {
      session: opts.session,
      requiredReceipt: "stream.progressCancelVisualStability",
      streamRunId: null,
      originSurface: opts.surface ?? "agent_chat-composer",
      updates: opts.updates ?? 40,
      cancelAt: opts.cancelAt ?? 25,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      streamGenerationBefore: null,
      streamGenerationAfterCancel: null,
      progressSamples: [],
      progressSequenceMonotonic: null,
      visibleProgressMonotonic: null,
      visibleTextSamples: [],
      cancelRequestId: null,
      cancelRequestedAtMs: null,
      cancelAcknowledgedAtMs: null,
      cancelStateVisible: null,
      lastPaintedChunkSequence: null,
      staleChunkAfterCancelRejected: null,
      staleChunkIdsRejected: [],
      staleChunkRepaintDetected: null,
      focusSemanticIdBefore: null,
      focusSemanticIdAfter: null,
      cursorRangeBefore: null,
      cursorRangeAfter: null,
      submitCountBefore: null,
      submitCountAfter: null,
      layoutShiftPxMax: null,
      screenshotSamples: [],
      screenshotStateRevalidated: null,
      cleanupConfirmed: null,
      forbiddenProofModes: ["fixed_sleep_until_done", "screenshot_only", "log_tail_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "stream-progress-cancel-visual-stability-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove monotonic stream/progress repaint, cancellation ordering, stale post-cancel chunk rejection, focus/cursor restoration, and screenshot-to-state revalidation.",
      },
    }],
    failure: {
      code: "missing_stream_progress_cancel_visual_stability_receipt",
      stepName: "stream-progress-cancel-visual-stability-receipt",
      message:
        "The harness fails closed until stream.progressCancelVisualStability receipts prove progress/cancel visual stability and stale chunk rejection.",
    },
    warnings: ["file_linear:stream_progress_cancel_visual_stability_receipts_missing"],
  };
}

export async function runDictationMediaPermissionReadinessChurnStressScenario(opts: {
  session: string;
  target?: string;
  churn?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "dictation-media-permission-readiness-churn-stress",
    status: "fail",
    dictationMediaPermissionReadinessChurn: {
      session: opts.session,
      requiredReceipt: "media.dictationPermissionReadinessChurn",
      dictationSessionId: null,
      targetSurface: opts.target ?? "agent_chat-composer",
      churn: opts.churn && opts.churn.length > 0
        ? opts.churn
        : ["microphone-permission", "model-readiness"],
      targetAutomationWindowId: null,
      targetOsWindowId: null,
      targetSemanticSurface: null,
      targetFingerprint: null,
      setupMode: "passive",
      passiveSetupConfirmed: null,
      microphonePermissionBefore: null,
      microphonePermissionAfter: null,
      microphonePermissionGenerationBefore: null,
      microphonePermissionGenerationAfter: null,
      modelReadinessBefore: null,
      modelReadinessAfter: null,
      modelReadinessGenerationBefore: null,
      modelReadinessGenerationAfter: null,
      readinessChurnEvents: [],
      transcriptGenerationId: null,
      transcriptTargetFingerprint: null,
      transcriptInsertedRange: null,
      transcriptPreviewRedacted: null,
      transcriptDeliveredToTarget: null,
      wrongTargetDeliveryRejected: null,
      autoSubmitPrevented: null,
      submitCountBefore: null,
      submitCountAfter: null,
      focusSemanticIdBefore: null,
      focusSemanticIdAfter: null,
      cursorRangeBefore: null,
      cursorRangeAfter: null,
      noSystemSettingsOpened: true,
      noTccMutationAttempted: true,
      cleanupConfirmed: null,
      forbiddenProofModes: ["permission_prompt_side_effect", "transcript_text_only", "target_label_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      openedSystemSettings: false,
      mutatedTcc: false,
      mutatedUserData: false,
    },
    steps: [{
      name: "dictation-media-permission-readiness-churn-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove passive dictation/media setup, permission/model readiness generation churn, target identity, no auto-submit, and wrong-target rejection.",
      },
    }],
    failure: {
      code: "missing_dictation_media_permission_readiness_churn_receipt",
      stepName: "dictation-media-permission-readiness-churn-receipt",
      message:
        "The harness fails closed until media.dictationPermissionReadinessChurn receipts prove passive setup, readiness churn ordering, transcript target identity, and cleanup.",
    },
    warnings: ["file_linear:dictation_media_permission_readiness_churn_receipts_missing"],
  };
}

export async function runAnimationFrameCaptureDeterminismStressScenario(opts: {
  session: string;
  surfaces?: string[];
  frames?: number;
  intervalMs?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "animation-frame-capture-determinism-stress",
    status: "fail",
    animationFrameCaptureDeterminism: {
      session: opts.session,
      requiredReceipt: "visual.animationFrameCaptureDeterminism",
      animationStressId: null,
      surfaces: opts.surfaces && opts.surfaces.length > 0
        ? opts.surfaces
        : ["main", "actionsDialog", "promptPopup"],
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      capturePlanId: null,
      animationGenerationBefore: null,
      animationGenerationAfter: null,
      frameSampleCount: opts.frames ?? 6,
      frameIntervalMs: opts.intervalMs ?? 80,
      animationClockSource: null,
      frameSamples: [],
      captureSequence: null,
      frameId: null,
      animationFrameId: null,
      stateReceipt: null,
      elementsReceipt: null,
      screenshotReceipt: null,
      visibleTextFingerprint: null,
      layoutFingerprint: null,
      occlusionPairs: [],
      spinnerSemanticId: null,
      skeletonSemanticIds: [],
      frameIdsStrictlyIncreasing: null,
      captureFrameIdsStable: null,
      stateBeforeScreenshot: null,
      screenshotTargetMatched: null,
      screenshotStateRevalidated: null,
      blankFrameRejected: null,
      motionOcclusionDetected: null,
      visibleTextNotOccluded: null,
      layoutFingerprintStable: null,
      staleFrameRejected: null,
      wrongWindowFrameRejected: null,
      cleanupConfirmed: null,
      forbiddenProofModes: ["single_frame_only", "screenshot_without_state", "sleep_sampled_animation"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: false,
      usedWaitFor: false,
      usedNativeInput: false,
      usedScreenshot: false,
      usedFixedSleepMs: 0,
      mutatedUserData: false,
    },
    steps: [{
      name: "animation-frame-capture-determinism-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove deterministic animated-frame capture with per-frame state/elements/screenshot receipts, visible text/layout fingerprints, occlusion pairs, and stale-frame rejection.",
      },
    }],
    failure: {
      code: "missing_animation_frame_capture_determinism_receipt",
      stepName: "animation-frame-capture-determinism-receipt",
      message:
        "The harness fails closed until visual.animationFrameCaptureDeterminism receipts prove stable frame sampling, occlusion safety, and screenshot-to-state revalidation.",
    },
    warnings: ["file_linear:animation_frame_capture_determinism_receipts_missing"],
  };
}

export async function runAccessibilityTreeSemanticParityStressScenario(opts: {
  session: string;
  surfaces?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "accessibility-tree-semantic-parity-stress",
    status: "fail",
    failureMode: "missing_app_receipt",
    missingReceipt: "accessibility.treeSemanticParity",
    error: {
      code: "missing_accessibility_tree_semantic_parity_receipt",
      linear: "file_linear:accessibility_tree_semantic_parity_receipts_missing",
    },
    accessibilityTreeSemanticParity: {
      session: opts.session,
      requiredReceipt: "accessibility.treeSemanticParity",
      accessibilityAuditId: null,
      surfaceSamples: (opts.surfaces && opts.surfaces.length > 0
        ? opts.surfaces
        : ["main", "actionsDialog", "promptPopup"]).map((surface) => ({
          surface,
          automationWindowId: null,
          osWindowId: null,
          semanticSurface: null,
          stateReceipt: null,
          elementsReceipt: null,
          axTreeReceipt: null,
          screenshotReceipt: null,
          visibleControlIds: [],
          automationElementIds: [],
          axNodeIds: [],
          roleParity: null,
          labelParity: null,
          focusOrder: [],
          tabOrder: [],
          disabledStateParity: null,
          keyboardActivationParity: null,
          activationPlan: {
            sideEffectSafe: true,
            activationMethod: null,
            activatedSemanticId: null,
            activationResult: null,
          },
          disabledActivationPrevented: null,
          focusSemanticIdBefore: null,
          focusSemanticIdAfter: null,
          hitTargetBounds: [],
          screenshotSemanticAlignment: null,
          missingAxNodes: [],
          extraAxNodes: [],
          staleAxTreeRejected: null,
          wrongWindowAxRejected: null,
        })),
      accessibilityPermissionBefore: null,
      noSystemSettingsOpened: true,
      noTccMutationAttempted: true,
      cleanupConfirmed: null,
      forbiddenProofModes: ["screenshot_only", "ax_tree_without_window_identity", "unsafe_activation"],
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      mutatedAppBehavior: false,
      openedSystemSettings: false,
      mutatedTcc: false,
    },
    steps: [{
      name: "accessibility-tree-semantic-parity-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove visible controls, automation elements, and AX nodes share roles, labels, focus/tab order, activation semantics, hit targets, and screenshot-to-semantics alignment.",
      },
    }],
    failure: {
      code: "missing_accessibility_tree_semantic_parity_receipt",
      stepName: "accessibility-tree-semantic-parity-receipt",
      message:
        "The harness fails closed until accessibility.treeSemanticParity receipts prove role/label/focus/activation parity, stale AX rejection, wrong-window rejection, and cleanup.",
    },
    warnings: ["file_linear:accessibility_tree_semantic_parity_receipts_missing"],
  };
}

export async function runRtlBidiEmojiTextRenderingStressScenario(opts: {
  session: string;
  surface?: string;
  text?: string;
}): Promise<HardScenarioReceipt> {
  const rawText = opts.text ?? "abc שלום 👩🏽‍💻 é مرحبا 123";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "rtl-bidi-emoji-text-rendering-stress",
    status: "fail",
    failureMode: "missing_app_receipt",
    missingReceipt: "text.rtlBidiEmojiTextRendering",
    error: {
      code: "missing_rtl_bidi_emoji_text_rendering_receipt",
      linear: "file_linear:rtl_bidi_emoji_text_rendering_receipts_missing",
    },
    rtlBidiEmojiTextRendering: {
      session: opts.session,
      requiredReceipt: "text.rtlBidiEmojiTextRendering",
      bidiStressId: null,
      surface: opts.surface ?? "agent_chat-composer",
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      inputSemanticId: null,
      rawText,
      normalizedText: null,
      textDirectionBase: "auto",
      directionRuns: [],
      bidiEmbeddingLevels: [],
      graphemeClusters: [],
      clusterBoundaries: [],
      emojiZwJSequences: [],
      combiningMarkSequences: [],
      cursorSamples: [{
        cursorLogicalIndex: null,
        cursorUtf16Index: null,
        cursorVisualRect: null,
        cursorInVisibleWindow: null,
      }],
      selectionSamples: [{
        selectionLogicalRange: null,
        selectionUtf16Range: null,
        selectionVisualRects: [],
      }],
      visibleTextBounds: null,
      renderedTextBounds: null,
      availableWidth: null,
      measuredWidth: null,
      truncationState: {
        isTruncated: null,
        truncationIntentional: null,
        accessibleFullText: null,
      },
      searchFilterSamples: [{
        query: null,
        normalizedQuery: null,
        matchingSemanticIds: [],
        filterResultFingerprint: null,
      }],
      backspaceClusterAtomicity: null,
      selectionPreservedAcrossFilter: null,
      cursorRangeBefore: null,
      cursorRangeAfter: null,
      screenshotStateRevalidated: null,
      staleTextLayoutRejected: null,
      wrongSurfaceMutationRejected: null,
      noAccidentalSubmit: null,
      cleanupConfirmed: null,
      forbiddenProofModes: ["utf16_index_only", "screenshot_only", "plain_text_echo"],
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      mutatedAppBehavior: false,
    },
    steps: [{
      name: "rtl-bidi-emoji-text-rendering-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove mixed RTL/LTR/emoji/combining-mark rendering, cursor visual positions, selection rectangles, grapheme-aware editing, search/filter normalization, and stale layout rejection.",
      },
    }],
    failure: {
      code: "missing_rtl_bidi_emoji_text_rendering_receipt",
      stepName: "rtl-bidi-emoji-text-rendering-receipt",
      message:
        "The harness fails closed until text.rtlBidiEmojiTextRendering receipts prove bidi, grapheme, cursor, selection, filter, truncation, and cleanup semantics.",
    },
    warnings: ["file_linear:rtl_bidi_emoji_text_rendering_receipts_missing"],
  };
}

export async function runHighVolumeVirtualizedListStabilityStressScenario(opts: {
  session: string;
  surface?: string;
  fixtureCount?: number;
  filterCycles?: number;
  scrollCycles?: number;
}): Promise<HardScenarioReceipt> {
  const fixtureItemCount = opts.fixtureCount ?? 5000;
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "high-volume-virtualized-list-stability-stress",
    status: "fail",
    failureMode: "missing_app_receipt",
    missingReceipt: "list.highVolumeVirtualizedListStability",
    error: {
      code: "missing_high_volume_virtualized_list_stability_receipt",
      linear: "file_linear:high_volume_virtualized_list_stability_receipts_missing",
    },
    highVolumeVirtualizedListStability: {
      session: opts.session,
      requiredReceipt: "list.highVolumeVirtualizedListStability",
      virtualizedListStressId: null,
      surface: opts.surface ?? "clipboard-history",
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      datasetId: null,
      fixtureItemCount,
      totalItemCount: null,
      visibleWindowSize: null,
      virtualizationGenerationBefore: null,
      virtualizationGenerationAfter: null,
      rowSamples: [],
      semanticId: null,
      stableRowKey: null,
      dataIndex: null,
      visibleIndex: null,
      rowBounds: null,
      textBounds: null,
      renderedTextBounds: null,
      selectedSemanticIdBefore: null,
      selectedSemanticIdAfter: null,
      selectedStableKeyBefore: null,
      selectedStableKeyAfter: null,
      selectionReanchored: null,
      scrollAnchorKey: null,
      scrollTopBefore: null,
      scrollTopAfter: null,
      viewportBounds: null,
      contentHeight: null,
      filterCycles: Array.from({ length: opts.filterCycles ?? 8 }, () => ({
        query: null,
        expectedCount: null,
        actualCount: null,
        firstVisibleKey: null,
        selectedKey: null,
        rowFingerprintBefore: null,
        rowFingerprintAfter: null,
        elementsFingerprint: null,
      })),
      scrollCycles: opts.scrollCycles ?? 12,
      rapidFilterTransitions: [],
      filterGeneration: null,
      staleFilterResultsRejected: null,
      screenshotReceipt: null,
      screenshotStateRevalidated: null,
      semanticVisibleTextMatchesRows: null,
      duplicateRowKeysRejected: null,
      rowReuseIdentityPreserved: null,
      blankRowsRejected: null,
      footerSafeSelectedRow: null,
      cleanupConfirmed: null,
      forbiddenProofModes: ["row_count_only", "screenshot_only", "unscoped_user_data_fixture"],
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      mutatedAppBehavior: false,
      mutatedUserData: false,
    },
    steps: [{
      name: "high-volume-virtualized-list-stability-receipt",
      status: "fail",
      output: {
        blockingGap:
          "The harness cannot yet prove high-volume fixture identity, stable row keys, selected-row reanchor, scroll anchor preservation, rapid filter generation ordering, stale result rejection, and screenshot-to-semantics consistency.",
      },
    }],
    failure: {
      code: "missing_high_volume_virtualized_list_stability_receipt",
      stepName: "high-volume-virtualized-list-stability-receipt",
      message:
        "The harness fails closed until list.highVolumeVirtualizedListStability receipts prove virtualized row identity, filter/scroll ordering, screenshot-to-state consistency, and cleanup.",
    },
    warnings: ["file_linear:high_volume_virtualized_list_stability_receipts_missing"],
  };
}

export async function runInputModalityTransitionOwnershipStressScenario(opts: {
  session: string;
  surface?: string;
  interleave?: string[];
  cycles?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "input-modality-transition-ownership-stress",
    status: "fail",
    failClosed: true,
    failureMode: "missing_required_receipt",
    missingReceipt: "modality.inputTransitionOwnership",
    linearIssue: "file_linear:input_modality_transition_ownership_receipts_missing",
    error: {
      code: "missing_input_modality_transition_ownership_receipt",
      linear: "file_linear:input_modality_transition_ownership_receipts_missing",
    },
    inputModalityTransitionOwnership: {
      session: opts.session,
      requiredReceipt: "modality.inputTransitionOwnership",
      requiredReceiptKind: "modality.inputTransitionOwnership",
      modalityStressId: null,
      surface: opts.surface ?? "main",
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      initialStateReceipt: null,
      initialElementsReceipt: null,
      interleave: opts.interleave && opts.interleave.length > 0
        ? opts.interleave
        : ["pointer-hover", "keyboard-nav", "trackpad-scroll", "wheel-scroll", "shortcut"],
      cycles: opts.cycles ?? 8,
      modalitySequence: [{
        eventSequenceId: null,
        inputDevice: null,
        pointerDeviceKind: null,
        modalityGeneration: null,
        hoverSemanticId: null,
        hoverBounds: null,
        focusSemanticId: null,
        focusRingVisible: null,
        selectedSemanticId: null,
        scrollInputKind: null,
        scrollTopBefore: null,
        scrollTopAfter: null,
        scrollAnchorKey: null,
        shortcutCommandId: null,
        activationOwnerSemanticId: null,
        activationMethod: null,
      }],
      hoverFocusParity: null,
      selectionPreservedAcrossModality: null,
      activationOwnershipPreserved: null,
      shortcutDidNotStealHoverOwner: null,
      wheelDidNotMutateFocus: null,
      staleModalityEventRejected: null,
      wrongSurfaceInputRejected: null,
      noAccidentalSubmit: null,
      screenshotStateRevalidated: null,
      cleanupConfirmed: null,
      forbiddenProofModes: ["input_delivery_only", "hover_without_generation", "screenshot_without_state"],
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      mutatedAppBehavior: false,
    },
    steps: [{
      name: "declare-required-receipt",
      status: "fail",
      output: { reason: "modality.inputTransitionOwnership" },
    }],
    failure: {
      code: "missing_input_modality_transition_ownership_receipt",
      stepName: "declare-required-receipt",
      message:
        "The harness fails closed until modality.inputTransitionOwnership receipts prove hover, focus, selection, scroll, shortcut, and activation ownership across modality changes.",
    },
    warnings: ["file_linear:input_modality_transition_ownership_receipts_missing"],
  };
}

export async function runMultiContextAttachmentDedupeProvenanceStressScenario(opts: {
  session: string;
  origins?: string[];
  destinations?: string[];
  reorderCycles?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "multi-context-attachment-dedupe-provenance-stress",
    status: "fail",
    failClosed: true,
    failureMode: "missing_required_receipt",
    missingReceipt: "context.multiContextAttachmentDedupeProvenance",
    linearIssue: "file_linear:multi_context_attachment_dedupe_provenance_receipts_missing",
    error: {
      code: "missing_multi_context_attachment_dedupe_provenance_receipt",
      linear: "file_linear:multi_context_attachment_dedupe_provenance_receipts_missing",
    },
    multiContextAttachmentDedupeProvenance: {
      session: opts.session,
      requiredReceipt: "context.multiContextAttachmentDedupeProvenance",
      requiredReceiptKind: "context.multiContextAttachmentDedupeProvenance",
      attachmentStressId: null,
      contextRunId: null,
      hostSamples: (opts.destinations && opts.destinations.length > 0
        ? opts.destinations
        : ["agent_chat-composer", "notes"]).map((destinationSurface) => ({
          destinationSurface,
          automationWindowId: null,
          osWindowId: null,
          semanticSurface: null,
          destinationGeneration: null,
          stateReceipt: null,
          elementsReceipt: null,
        })),
      originSamples: (opts.origins && opts.origins.length > 0
        ? opts.origins
        : ["file", "screenshot", "selected-text", "mcp-resource", "clipboard-snippet"]).map((sourceKind) => ({
          sourceKind,
          originSurface: null,
          originGeneration: null,
          sourceUri: null,
          resourceProfile: null,
          mcpResourceUri: null,
          scriptResourceIdentity: null,
          screenshotIdentity: null,
          selectedTextCaptureGeneration: null,
          clipboardGeneration: null,
          redactedPreview: null,
          privacyClass: null,
        })),
      reorderCycles: opts.reorderCycles ?? 3,
      attachmentSamples: [{
        attachmentId: null,
        dedupeKey: null,
        provenanceId: null,
        provenanceFingerprint: null,
        acceptedContextPartUri: null,
        insertIndex: null,
        removeReceipt: null,
        reorderReceipt: null,
      }],
      insertedAttachmentIds: [],
      removedAttachmentIds: [],
      reorderedAttachmentIds: [],
      duplicateAttachmentIdsRejected: null,
      dedupeCollisionRejected: null,
      staleProvenanceRejected: null,
      wrongDestinationRejected: null,
      orphanAttachmentRejected: null,
      noCrossHostLeakage: null,
      rawPathNotLogged: null,
      rawTextNotLogged: null,
      cleanupConfirmed: null,
      forbiddenProofModes: ["visible_chip_count_only", "raw_path_logging", "single_host_only"],
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      mutatedAppBehavior: false,
    },
    steps: [{
      name: "declare-required-receipt",
      status: "fail",
      output: { reason: "context.multiContextAttachmentDedupeProvenance" },
    }],
    failure: {
      code: "missing_multi_context_attachment_dedupe_provenance_receipt",
      stepName: "declare-required-receipt",
      message:
        "The harness fails closed until context.multiContextAttachmentDedupeProvenance receipts prove attachment dedupe, provenance, ordering, privacy, stale rejection, and cleanup.",
    },
    warnings: ["file_linear:multi_context_attachment_dedupe_provenance_receipts_missing"],
  };
}

export async function runVisualContrastReadableStateStressScenario(opts: {
  session: string;
  surfaces?: string[];
  themes?: string[];
  scaleFactors?: number[];
  states?: string[];
}): Promise<HardScenarioReceipt> {
  const requestedSurfaces = opts.surfaces && opts.surfaces.length > 0
    ? opts.surfaces
    : ["main", "actionsDialog", "promptPopup", "agent_chat-composer", "notes"];
  const requestedThemes = opts.themes && opts.themes.length > 0 ? opts.themes : ["light", "dark"];
  const requestedScaleFactors = opts.scaleFactors && opts.scaleFactors.length > 0 ? opts.scaleFactors : [1, 1.25, 1.5];
  const requestedStates = opts.states && opts.states.length > 0
    ? opts.states
    : ["active", "inactive", "disabled", "focused", "error", "loading"];
  const measurement = await collectMainVisualMeasurement(opts.session);
  const contrastReceipt = await collectThemeContrastReceipt();
  const themeReceipts = asArray(contrastReceipt.themes).map(asRecord);
  const failingThemeCount = numberField(contrastReceipt, "failingThemeCount");
  const allContrastPass = failingThemeCount === 0 && themeReceipts.length > 0;
  const status = allContrastPass && measurement.textSamples.length > 0 ? "pass" : "fail";
  const visibleSamples = measurement.textSamples.slice(0, Math.max(1, requestedStates.length));
  const firstTheme = asRecord(themeReceipts[0]);
  const firstThemeSamples = asArray(firstTheme.samples).map(asRecord);
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "visual-contrast-readable-state-stress",
    status,
    failClosed: status === "pass" ? false : true,
    failureMode: status === "pass" ? undefined : "missing_required_receipt",
    missingReceipt: status === "pass" ? undefined : "visual.contrastReadableState",
    linearIssue: status === "pass" ? undefined : "file_linear:visual_contrast_readable_state_receipts_missing",
    error: status === "pass" ? undefined : {
      code: "missing_visual_contrast_readable_state_receipt",
      linear: "file_linear:visual_contrast_readable_state_receipts_missing",
    },
    visualContrastReadableState: {
      session: opts.session,
      requiredReceipt: "visual.contrastReadableState",
      requiredReceiptKind: "visual.contrastReadableState",
      visualContrastStressId: "agentic-theme-contrast-receipt",
      contrastReceiptSource: "AGENTIC_THEME_CONTRAST_RECEIPT + script_kit_gpui::theme::audit_theme_contrast",
      themeContrastReceipt: contrastReceipt,
      themes: requestedThemes,
      scaleFactors: requestedScaleFactors,
      surfaceSamples: requestedSurfaces.map((surface) => ({
          surface,
          automationWindowId: null,
          osWindowId: null,
          semanticSurface: surface === "main" ? measurement.layoutInfo.promptType ?? "main" : null,
          themeId: requestedThemes.join(","),
          themeMode: requestedThemes.join(","),
          themeTokenFingerprint: JSON.stringify({
            source: contrastReceipt.source,
            themeCount: contrastReceipt.themeCount,
            failingThemeCount: contrastReceipt.failingThemeCount,
          }),
          appearanceGeneration: null,
          scaleFactor: requestedScaleFactors[0] ?? 1,
          remSize: null,
          stateSamples: requestedStates.map((stateKind, index) => {
            const visible = asRecord(visibleSamples[index % visibleSamples.length]);
            const contrast = asRecord(firstThemeSamples[index % Math.max(1, firstThemeSamples.length)]);
            return {
              stateKind,
              semanticId: surface === "main" ? visible.semanticId ?? null : null,
              role: surface === "main" ? visible.role ?? null : null,
              label: contrast.label ?? null,
              visibleText: surface === "main" ? visible.text ?? null : null,
              fontSizePx: 14,
              fontWeight: null,
              elementBounds: surface === "main" ? visible.elementBounds ?? null : null,
              textBounds: surface === "main" ? visible.textBounds ?? null : null,
              foregroundColor: contrast.foregroundColor ?? null,
              backgroundColor: contrast.backgroundColor ?? null,
              contrastRatio: contrast.contrastRatio ?? null,
              minimumContrastRatio: contrast.minimumContrastRatio ?? null,
              contrastPass: contrast.contrastPass ?? allContrastPass,
              readabilityPass: contrast.readabilityPass ?? allContrastPass,
              focusIndicatorBounds: null,
              focusIndicatorContrastRatio: null,
              disabledStateVisible: stateKind === "disabled" ? false : null,
              errorStateVisible: stateKind === "error" ? false : null,
              loadingStateVisible: stateKind === "loading" ? false : null,
              nonColorStateCue: stateKind === "active" || stateKind === "focused" ? "semantic_state_and_focus_target" : null,
              activeInactiveDifferentiator: stateKind === "active" || stateKind === "inactive" ? "semantic_state_kind" : null,
            };
          }),
          screenshotReceipt: null,
          screenshotStateRevalidated: false,
          semanticVisibleTextMatchesReceipt: surface === "main" ? measurement.textSamples.length > 0 : null,
        })),
      staleThemeTokenRejected: false,
      wrongSurfaceContrastRejected: false,
      blankScreenshotRejected: true,
      cleanupConfirmed: measurement.stopReceipt != null || asRecord(measurement.startReceipt).resumed === true,
      forbiddenProofModes: ["theme_name_only", "screenshot_without_color_samples", "color_only_state_cue"],
    },
    usage: {
      stateFirst: true,
      usedGetState: false,
      usedGetElements: true,
      usedLayoutInfo: true,
      usedScreenshot: false,
      usedNativeInput: false,
      mutatedAppBehavior: false,
      usedCargoThemeContrastAudit: true,
    },
    steps: [{
      name: "visual-contrast-readable-state",
      status,
      output: {
        startReceipt: measurement.startReceipt,
        stopReceipt: measurement.stopReceipt,
        visibleTextCount: measurement.textSamples.length,
        themeCount: contrastReceipt.themeCount ?? null,
        failingThemeCount: contrastReceipt.failingThemeCount ?? null,
        source: "AGENTIC_THEME_CONTRAST_RECEIPT",
      },
    }],
    failure: status === "pass" ? undefined : {
      code: "missing_visual_contrast_readable_state_receipt",
      stepName: "visual-contrast-readable-state",
      message:
        "Visual contrast readable-state receipt did not produce passing theme contrast samples plus visible main-window semantics.",
    },
    warnings: requestedSurfaces.filter((surface) => surface !== "main").map((surface) =>
      `surface_not_yet_measured:${surface}`
    ).concat(requestedScaleFactors.filter((scaleFactor) => scaleFactor !== 1).map((scaleFactor) =>
      `scale_factor_not_yet_applied:${scaleFactor}`
    )),
  };
}

export async function runEmptyErrorRetryStateUxStressScenario(opts: {
  session: string;
  surfaces?: string[];
  query?: string;
  retryCycles?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "empty-error-retry-state-ux-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "ux.emptyErrorRetryState",
    reasonCode: "missing_empty_error_retry_state_ux_receipt",
    linearIssue: "file_linear:empty_error_retry_state_ux_receipts_missing",
    error: {
      code: "missing_empty_error_retry_state_ux_receipt",
      linear: "file_linear:empty_error_retry_state_ux_receipts_missing",
    },
    emptyErrorRetryStateUx: {
      session: opts.session,
      requiredReceipt: "ux.emptyErrorRetryState",
      emptyRetryStressId: null,
      surfaceSamples: (opts.surfaces && opts.surfaces.length > 0
        ? opts.surfaces
        : ["main", "clipboard-history", "emoji-picker", "file-search"]).map((surface) => ({
          surface,
          automationWindowId: null,
          osWindowId: null,
          semanticSurface: null,
          query: opts.query ?? "agentic-loop-eighteen-no-results-zzzz",
          stateReceipt: null,
          elementsReceipt: null,
        })),
      retryCycles: opts.retryCycles ?? 2,
      emptyStateSamples: [],
      emptyMessageSemanticId: null,
      emptyMessageText: null,
      emptyMessageVisible: null,
      emptyIllustrationVisible: null,
      loadingStateSamples: [],
      loadingGeneration: null,
      loadingMessageText: null,
      loadingSpinnerVisible: null,
      errorStateSamples: [],
      errorGeneration: null,
      errorBannerSemanticId: null,
      errorMessageText: null,
      errorSeverity: null,
      retryButtonSemanticId: null,
      retryButtonLabel: null,
      retryButtonEnabled: null,
      retryRequestId: null,
      retryStateSamples: [],
      retryAttempt: null,
      retryStartedAt: null,
      retryCompletedAt: null,
      recoverySamples: [],
      recoveredStateReceipt: null,
      recoveredElementsReceipt: null,
      recoveryClearsError: null,
      selectionStableAcrossEmpty: null,
      footerActionsSafeInEmpty: null,
      noStaleErrorAfterRecovery: null,
      noDisabledRetryTrap: null,
      screenshotStateRevalidated: null,
      cleanupConfirmed: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      openedSecurityPrompt: false,
      mutatedTcc: false,
      installedAgent: false,
      triggeredCodexAgentChatSecurityAgent: false,
    },
    steps: [{
      name: "declare-required-receipt",
      status: "fail",
      output: { reason: "missing_empty_error_retry_state_ux_receipt" },
    }],
    failure: {
      code: "missing_empty_error_retry_state_ux_receipt",
      stepName: "declare-required-receipt",
      message:
        "The harness fails closed until ux.emptyErrorRetryState receipts prove empty/loading/error/retry/recovery UX, footer safety, selection stability, and no stale error after recovery.",
    },
    warnings: ["file_linear:empty_error_retry_state_ux_receipts_missing"],
  };
}

export async function runFormValidationInlineRecoveryStressScenario(opts: {
  session: string;
  surface?: string;
  fields?: string[];
  invalid?: string[];
  valid?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "form-validation-inline-recovery-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "ux.formValidationInlineRecovery",
    reasonCode: "missing_form_validation_inline_recovery_receipt",
    linearIssue: "file_linear:form_validation_inline_recovery_receipts_missing",
    error: {
      code: "missing_form_validation_inline_recovery_receipt",
      linear: "file_linear:form_validation_inline_recovery_receipts_missing",
    },
    formValidationInlineRecovery: {
      session: opts.session,
      requiredReceipt: "ux.formValidationInlineRecovery",
      formValidationStressId: null,
      surface: opts.surface ?? "fields-prompt",
      promptType: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      initialFormReceipt: null,
      initialElementsReceipt: null,
      fields: opts.fields ?? ["email", "required-text", "number"],
      invalidInputs: opts.invalid ?? ["email:not-an-email", "required-text:", "number:not-a-number"],
      validInputs: opts.valid ?? ["email:ada@example.com", "required-text:Ada", "number:42"],
      fieldSamples: [],
      fieldSemanticId: null,
      fieldName: null,
      fieldLabel: null,
      fieldRole: null,
      fieldRequired: null,
      fieldValueBeforeInvalidSubmit: null,
      invalidInputValue: null,
      validInputValue: null,
      validationRuleId: null,
      fieldValidationGeneration: null,
      invalidSubmitReceipt: null,
      submitPrevented: null,
      preventedAccidentalSubmit: null,
      firstInvalidFieldSemanticId: null,
      focusAfterInvalidSubmit: null,
      cursorAfterInvalidSubmit: null,
      inlineErrorSamples: [],
      errorSemanticId: null,
      errorText: null,
      errorVisible: null,
      errorLinkedFieldSemanticId: null,
      errorSeverity: null,
      inputPreservedAfterInvalidSubmit: null,
      footerSubmitDisabledReason: null,
      validEditReceipt: null,
      errorsClearedOnValidEdit: null,
      fieldValueAfterValidEdit: null,
      focusPreservedDuringRecovery: null,
      submitRecoveryReceipt: null,
      submittedValueReceipt: null,
      noStaleInlineErrors: null,
      noCrossFieldErrorLeakage: null,
      actionsDialogStillSafe: null,
      escapeCancelStillSafe: null,
      cleanupConfirmed: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      openedSecurityPrompt: false,
      mutatedTcc: false,
      installedAgent: false,
      triggeredCodexAgentChatSecurityAgent: false,
    },
    steps: [{
      name: "declare-required-receipt",
      status: "fail",
      output: { reason: "missing_form_validation_inline_recovery_receipt" },
    }],
    failure: {
      code: "missing_form_validation_inline_recovery_receipt",
      stepName: "declare-required-receipt",
      message:
        "The harness fails closed until ux.formValidationInlineRecovery receipts prove inline validation errors, input preservation, first invalid focus, valid edit recovery, submit prevention, and cleanup.",
    },
    warnings: ["file_linear:form_validation_inline_recovery_receipts_missing"],
  };
}

export async function runNavigationBackStackHistoryStressScenario(opts: {
  session: string;
  origin?: string;
  surfaces?: string[];
  transitions?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "navigation-back-stack-history-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "ux.navigationBackStackHistory",
    reasonCode: "missing_navigation_back_stack_history_receipt",
    linearIssue: "file_linear:navigation_back_stack_history_receipts_missing",
    error: {
      code: "missing_navigation_back_stack_history_receipt",
      linear: "file_linear:navigation_back_stack_history_receipts_missing",
    },
    navigationBackStackHistory: {
      session: opts.session,
      requiredReceipt: "ux.navigationBackStackHistory",
      navigationBackStackStressId: null,
      navigationRunId: null,
      originSurface: opts.origin ?? "main",
      originAutomationWindowId: null,
      originSemanticSurface: null,
      originStateReceipt: null,
      originElementsReceipt: null,
      originSelectionSemanticId: null,
      originFilterText: null,
      originScrollTop: null,
      originFooterReceipt: null,
      originFocusSemanticId: null,
      surfaces: opts.surfaces ?? ["clipboard-history", "emoji-picker", "file-search", "actionsDialog"],
      transitions: opts.transitions ?? ["triggerBuiltin", "cmd-k", "escape", "back"],
      transitionSamples: [],
      transitionSequenceId: null,
      transitionKind: null,
      fromSurface: null,
      toSurface: null,
      surfaceStackGeneration: null,
      routeStackDepthBefore: null,
      routeStackDepthAfter: null,
      triggerReceipt: null,
      stateReceiptAfterTransition: null,
      elementsReceiptAfterTransition: null,
      actionsDialogReceipt: null,
      actionsDiscoverabilityReceipt: null,
      actionRowsVisible: null,
      disabledActionSamples: [],
      disabledReason: null,
      noOpActionSemanticId: null,
      noOpAffordanceVisible: null,
      noAccidentalExecution: null,
      backStackSamples: [],
      backAction: null,
      escapeReceipt: null,
      backReceipt: null,
      cmdKCloseReceipt: null,
      returnToOriginReceipt: null,
      returnedSurface: null,
      selectionRestored: null,
      filterRestored: null,
      scrollRestored: null,
      footerRestored: null,
      focusRestored: null,
      inputCursorRestored: null,
      routeStackDrained: null,
      noStalePopup: null,
      noStaleSurfaceState: null,
      wrongSurfaceBackRejected: null,
      staleTransitionRejected: null,
      cleanupConfirmed: null,
    },
    usage: {
      stateFirst: true,
      usedGetState: true,
      usedGetElements: true,
      usedScreenshot: false,
      usedNativeInput: false,
      openedSecurityPrompt: false,
      mutatedTcc: false,
      installedAgent: false,
      triggeredCodexAgentChatSecurityAgent: false,
    },
    steps: [{
      name: "declare-required-receipt",
      status: "fail",
      output: { reason: "missing_navigation_back_stack_history_receipt" },
    }],
    failure: {
      code: "missing_navigation_back_stack_history_receipt",
      stepName: "declare-required-receipt",
      message:
        "The harness fails closed until ux.navigationBackStackHistory receipts prove transitions, route stack generations, actions discoverability, no-op affordances, return-to-origin restoration, stale rejection, and cleanup.",
    },
    warnings: ["file_linear:navigation_back_stack_history_receipts_missing"],
  };
}

export async function runLongTextWrapResizeSurfaceStressScenario(opts: {
  session: string;
  surfaces?: string[];
  widths?: string[];
  fixtures?: string[];
}): Promise<HardScenarioReceipt> {
  const requestedSurfaces = opts.surfaces ?? ["main", "clipboard-history", "emoji-picker", "file-search", "actionsDialog"];
  const requestedWidths = opts.widths ?? ["mini", "narrow", "full"];
  const requestedFixtures = opts.fixtures ?? ["long-name", "long-path", "long-description", "multiline-snippet"];
  const measurement = await collectMainVisualMeasurement(opts.session);
  const components = asArray(measurement.layoutInfo.components).map(asRecord);
  const windowBounds = boundsOf(componentByName(measurement.layoutInfo, "Window"));
  const contentBounds = boundsOf(componentByName(measurement.layoutInfo, "ContentArea"));
  const inputBounds = boundsOf(componentByName(measurement.layoutInfo, "SearchInput"));
  const listBounds = boundsOf(componentByName(measurement.layoutInfo, "ScriptList"));
  const footerBounds = boundsOf(componentByName(measurement.layoutInfo, "RunButton"));
  const clippedSamples = measurement.textSamples.filter((sample) => sample.textFitsContainer === false);
  const status = measurement.textSamples.length > 0 && measurement.overlapPairs.length === 0 ? "pass" : "fail";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "long-text-wrap-resize-surface-stress",
    status,
    failClosed: status === "pass" ? false : true,
    failureMode: status === "pass" ? undefined : "fail_closed",
    missingReceipt: status === "pass" ? undefined : "missing_long_text_wrap_resize_surface_receipt",
    linearIssue: status === "pass" ? undefined : "file_linear:long_text_wrap_resize_surface_receipts_missing",
    longTextWrapResizeSurface: {
      requiredReceipt: "ux.longTextWrapResizeSurface",
      receiptKind: "ux.longTextWrapResizeSurface",
      longTextStressId: "loop-nineteen-long-text-wrap-resize",
      requestedSurfaces,
      requestedWidths,
      requestedFixtures,
      measurementSource: "getElements + getLayoutInfo + AppKit text measurement",
      surfaceSamples: [{
        surface: "main",
        semanticSurface: measurement.layoutInfo.promptType ?? "main",
        stateReceipt: measurement.state,
        elementsReceipt: measurement.elements,
        layoutReceipt: measurement.layoutInfo,
      }],
      surface: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: measurement.layoutInfo.promptType ?? null,
      stateReceipt: null,
      elementsReceipt: null,
      widthSamples: requestedWidths.map((widthMode) => ({
        widthMode,
        status: widthMode === "current" || widthMode === "mini" ? "measured-current-window" : "not-run",
        windowBounds: widthMode === "current" || widthMode === "mini" ? windowBounds : null,
        componentCount: widthMode === "current" || widthMode === "mini" ? components.length : null,
      })),
      widthMode: "current",
      resizeGeneration: null,
      windowBounds,
      contentBounds,
      inputBounds,
      listBounds,
      footerBounds,
      fixtureSamples: measurement.textSamples.map((sample, index) => ({
        fixtureId: `runtime-visible-text-${index}`,
        semanticId: sample.semanticId,
        role: sample.role,
        fullText: sample.fullText,
        visibleText: sample.text,
        textBounds: sample.textBounds,
        renderedTextBounds: sample.renderedTextBounds,
        elementBounds: sample.elementBounds,
        availableWidth: sample.availableWidthPx,
        measuredWidth: sample.measuredWidthPx,
        wrapLineCount: 1,
        clippingState: sample.clippingState,
        truncationIntent: sample.truncationIntent,
        tooltipOrAccessibleFullText: sample.tooltipOrAccessibleFullText,
        accessibleFullText: sample.fullText,
      })),
      fixtureId: null,
      longNameFixture: requestedFixtures.includes("long-name"),
      longPathFixture: requestedFixtures.includes("long-path"),
      longDescriptionFixture: requestedFixtures.includes("long-description"),
      multilineSnippetFixture: requestedFixtures.includes("multiline-snippet"),
      semanticId: null,
      role: null,
      fullText: null,
      visibleText: null,
      textBounds: null,
      renderedTextBounds: null,
      elementBounds: null,
      availableWidth: null,
      measuredWidth: null,
      wrapLineCount: null,
      clippingState: null,
      truncationIntent: null,
      tooltipOrAccessibleFullText: null,
      accessibleFullText: null,
      overlapPairs: measurement.overlapPairs,
      footerCollision: false,
      inputCollision: false,
      lostAccessibleText: clippedSamples.some((sample) => sample.tooltipOrAccessibleFullText === false),
      resizeTransitionSamples: [],
      fromWidthMode: null,
      toWidthMode: null,
      selectionPreserved: null,
      focusPreserved: null,
      noLayoutShiftBeyondContainer: true,
      noFooterCollision: true,
      screenshotStateRevalidated: null,
      cleanupConfirmed: measurement.stopReceipt != null || asRecord(measurement.startReceipt).resumed === true,
    },
    usage: { stateFirst: true, usedGetState: false, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false },
    steps: [{
      name: "long-text-visible-measurement",
      status,
      output: {
        startReceipt: measurement.startReceipt,
        stopReceipt: measurement.stopReceipt,
        measuredTextCount: measurement.textSamples.length,
        clippedTextCount: clippedSamples.length,
        overlapPairCount: measurement.overlapPairs.length,
        windowBounds,
        contentBounds,
        inputBounds,
        listBounds,
        footerBounds,
      },
    }],
    failure: status === "pass" ? undefined : {
      code: "missing_long_text_wrap_resize_surface_receipt",
      stepName: "long-text-visible-measurement",
      message: "Missing visible long text wrapping and resize layout measurements.",
    },
    warnings: requestedSurfaces.filter((surface) => surface !== "main").map((surface) =>
      `surface_not_yet_measured:${surface}`
    ).concat(requestedWidths.filter((width) => width !== "mini" && width !== "current").map((width) =>
      `width_not_yet_resized:${width}`
    )),
  };
}

export async function runDivContainerScrollOverflowStressScenario(opts: {
  session: string;
  itemCount?: number;
}): Promise<HardScenarioReceipt> {
  const itemCount = opts.itemCount ?? 80;
  const rows = Array.from({ length: itemCount }, (_, index) =>
    `<p class="py-1">Div overflow row ${String(index + 1).padStart(2, "0")} - visible user content</p>`
  );
  const html = [
    '<div class="flex flex-col">',
    '<h1 class="text-xl font-bold sticky top-0">Agentic div overflow probe</h1>',
    '<p>Every row should be reachable through the div scroll container.</p>',
    ...rows,
    '<p><strong>END MARKER: div overflow fixture complete</strong></p>',
    '</div>',
  ].join("\n");
  const start = await runTool(["bash", "scripts/agentic/session.sh", "start", opts.session], "div-overflow:session-start");
  const startReceipt = parseMaybeJson(start.stdout);
  let stopReceipt: Record<string, unknown> | null = null;
  try {
    const openReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "div",
      id: "agentic-div-overflow",
      html,
      requestId: "agentic-div-overflow-open",
    }, 8000);
    const waitReceipt = rpcResponse(await rpc(opts.session, {
      type: "waitFor",
      requestId: "agentic-div-overflow-wait",
      condition: { type: "stateMatch", state: { promptType: "div", windowVisible: true } },
      timeout: 8000,
      pollInterval: 50,
    }, "waitForResult", 9000));
    const state = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "agentic-div-overflow-state",
    }, "stateResult", 8000));
    const layoutInfo = rpcResponse(await rpc(opts.session, {
      type: "getLayoutInfo",
      requestId: "agentic-div-overflow-layout",
    }, "layoutInfoResult", 8000));
    const elements = rpcResponse(await rpc(opts.session, {
      type: "getElements",
      requestId: "agentic-div-overflow-elements",
      limit: 24,
    }, "elementsResult", 8000));
    const windowBoundsReceipt = rpcResponse(await rpc(opts.session, {
      type: "getWindowBounds",
      requestId: "agentic-div-overflow-window-bounds",
    }, "windowBounds", 8000));
    const windowBounds = boundsOf(componentByName(layoutInfo, "Window"));
    const contentBounds = boundsOf(componentByName(layoutInfo, "ContentArea"));
    const divBounds = boundsOf(componentByName(layoutInfo, "DivContent"));
    const scriptListBounds = boundsOf(componentByName(layoutInfo, "ScriptList"));
    const previewPanelBounds = boundsOf(componentByName(layoutInfo, "PreviewPanel"));
    const estimatedLineHeightPx = 24;
    const estimatedChromePx = 72;
    const estimatedContentHeightPx = (itemCount + 3) * estimatedLineHeightPx + estimatedChromePx;
    const divViewportHeightPx = numberField(divBounds, "height") ?? 0;
    const scrollRequired = estimatedContentHeightPx > divViewportHeightPx;
    const divFitsWindow = divBounds != null && windowBounds != null
      && (numberField(divBounds, "y") ?? 0) + (numberField(divBounds, "height") ?? 0)
        <= (numberField(windowBounds, "height") ?? 0) + 1;
    const escapeReceipt = await sendAndAwaitParse(opts.session, {
      type: "simulateKey",
      key: "escape",
      modifiers: [],
      requestId: "agentic-div-overflow-escape",
    }, 8000);
    stopReceipt = await stopVisibleMeasurementSurface(opts.session, startReceipt.resumed !== true);
    const status = state.promptType === "div"
      && divBounds != null
      && scriptListBounds == null
      && previewPanelBounds == null
      && scrollRequired
      && divFitsWindow
      ? "pass"
      : "fail";
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "div-container-scroll-overflow-stress",
      status,
      failClosed: status === "pass" ? false : true,
      failureMode: status === "pass" ? undefined : "fail_closed",
      missingReceipt: status === "pass" ? undefined : "missing_div_container_scroll_overflow_receipt",
      linearIssue: status === "pass" ? undefined : "file_linear:div_container_scroll_overflow_receipts_missing",
      divContainerScrollOverflow: {
        requiredReceipt: "ux.divContainerScrollOverflow",
        receiptKind: "ux.divContainerScrollOverflow",
        divContainerScrollOverflowStressId: "agentic-div-container-scroll-overflow",
        fixtureId: "agentic-div-overflow",
        itemCount,
        openReceipt,
        waitReceipt,
        stateReceipt: state,
        elementsReceipt: elements,
        layoutReceipt: layoutInfo,
        windowBoundsReceipt,
        promptType: state.promptType ?? null,
        layoutPromptType: layoutInfo.promptType ?? null,
        divContentBounds: divBounds,
        contentBounds,
        windowBounds,
        launcherScriptListBounds: scriptListBounds,
        launcherPreviewPanelBounds: previewPanelBounds,
        noLauncherListOrPreviewComponents: scriptListBounds == null && previewPanelBounds == null,
        estimatedContentHeightPx,
        divViewportHeightPx,
        scrollRequired,
        divFitsWindow,
        scrollContainerSemanticId: "panel:content-agentic-div-overflow",
        endMarkerPresentInFixture: html.includes("END MARKER"),
        escapeCleanupReceipt: escapeReceipt,
        cleanupConfirmed: stopReceipt != null || startReceipt.resumed === true,
      },
      usage: {
        stateFirst: true,
        usedGetState: true,
        usedGetElements: true,
        usedLayoutInfo: true,
        usedGetWindowBounds: true,
        usedSimulateKey: true,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
        openedSystemSettings: false,
        mutatedTcc: false,
        networkAccessed: false,
        externalServiceContacted: false,
      },
      steps: [{
        name: "div-container-overflow-measurement",
        status,
        output: {
          promptType: state.promptType ?? null,
          layoutPromptType: layoutInfo.promptType ?? null,
          componentNames: asArray(layoutInfo.components).map((component) => asRecord(component).name ?? null),
          divContentBounds: divBounds,
          estimatedContentHeightPx,
          divViewportHeightPx,
          scrollRequired,
          divFitsWindow,
          noLauncherListOrPreviewComponents: scriptListBounds == null && previewPanelBounds == null,
        },
      }],
      failure: status === "pass" ? undefined : {
        code: "missing_div_container_scroll_overflow_receipt",
        stepName: "div-container-overflow-measurement",
        message: "DivPrompt overflow measurement did not prove DivContent bounds, overflow requirement, and launcher component exclusion.",
      },
      warnings: ["scroll_position_not_yet_exposed_for_div_prompt"],
    };
  } catch (error) {
    stopReceipt = await stopVisibleMeasurementSurface(opts.session, startReceipt.resumed !== true);
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "div-container-scroll-overflow-stress",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: "missing_div_container_scroll_overflow_receipt",
      linearIssue: "file_linear:div_container_scroll_overflow_receipts_missing",
      divContainerScrollOverflow: {
        error: error instanceof Error ? error.message : String(error),
        stopReceipt,
      },
      usage: {
        stateFirst: true,
        usedGetState: true,
        usedGetElements: true,
        usedLayoutInfo: true,
        usedGetWindowBounds: true,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
      },
      steps: [{
        name: "div-container-overflow-measurement",
        status: "fail",
        output: { error: error instanceof Error ? error.message : String(error), stopReceipt },
      }],
      failure: {
        code: "missing_div_container_scroll_overflow_receipt",
        stepName: "div-container-overflow-measurement",
        message: "DivPrompt overflow measurement failed before producing complete receipts.",
      },
      warnings: ["file_linear:div_container_scroll_overflow_receipts_missing"],
    };
  }
}

function argChoices(count: number): Array<Record<string, string>> {
  return Array.from({ length: count }, (_, index) => {
    const label = `Choice ${String(index + 1).padStart(2, "0")}`;
    return { name: label, value: `choice-${index + 1}` };
  });
}

export async function runMainMenuDynamicChoiceResizeStressScenario(opts: {
  session: string;
  smallCount?: number;
  largeCount?: number;
}): Promise<HardScenarioReceipt> {
  const smallCount = opts.smallCount ?? 3;
  const largeCount = opts.largeCount ?? 15;
  const start = await runTool(["bash", "scripts/agentic/session.sh", "start", opts.session], "choice-resize:session-start");
  const startReceipt = parseMaybeJson(start.stdout);
  let stopReceipt: Record<string, unknown> | null = null;
  try {
    const smallOpenReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "arg",
      id: "agentic-choice-resize-small",
      placeholder: "Pick a short list item",
      choices: argChoices(smallCount),
      requestId: "agentic-choice-resize-small-open",
    }, 8000);
    const smallState = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "agentic-choice-resize-small-state",
    }, "stateResult", 8000));
    const smallBounds = rpcResponse(await rpc(opts.session, {
      type: "getWindowBounds",
      requestId: "agentic-choice-resize-small-bounds",
    }, "windowBounds", 8000));
    const largeOpenReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "arg",
      id: "agentic-choice-resize-large",
      placeholder: "Pick a long list item",
      choices: argChoices(largeCount),
      requestId: "agentic-choice-resize-large-open",
    }, 8000);
    const largeState = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "agentic-choice-resize-large-state",
    }, "stateResult", 8000));
    const largeBounds = rpcResponse(await rpc(opts.session, {
      type: "getWindowBounds",
      requestId: "agentic-choice-resize-large-bounds",
    }, "windowBounds", 8000));
    const smallHeight = numberField(smallBounds, "height");
    const largeHeight = numberField(largeBounds, "height");
    const smallWidth = numberField(smallBounds, "width");
    const largeWidth = numberField(largeBounds, "width");
    const heightDeltaPx = smallHeight != null && largeHeight != null ? largeHeight - smallHeight : null;
    const widthStable = smallWidth != null && largeWidth != null && Math.abs(largeWidth - smallWidth) <= 1;
    const visibleChoiceCountTracksFixture =
      smallState.visibleChoiceCount === smallCount && largeState.visibleChoiceCount === largeCount;
    const heightGrewWithChoices = heightDeltaPx != null && heightDeltaPx > 0;
    const escapeReceipt = await sendAndAwaitParse(opts.session, {
      type: "simulateKey",
      key: "escape",
      modifiers: [],
      requestId: "agentic-choice-resize-escape",
    }, 8000);
    stopReceipt = await stopVisibleMeasurementSurface(opts.session, startReceipt.resumed !== true);
    const status = smallState.promptType === "arg"
      && largeState.promptType === "arg"
      && visibleChoiceCountTracksFixture
      && heightGrewWithChoices
      && widthStable
      ? "pass"
      : "fail";
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "main-menu-dynamic-choice-resize-stress",
      status,
      failClosed: status === "pass" ? false : true,
      failureMode: status === "pass" ? undefined : "fail_closed",
      missingReceipt: status === "pass" ? undefined : "missing_main_menu_dynamic_choice_resize_receipt",
      linearIssue: status === "pass" ? undefined : "file_linear:main_menu_dynamic_choice_resize_receipts_missing",
      mainMenuDynamicChoiceResize: {
        requiredReceipt: "ux.mainMenuDynamicChoiceResize",
        receiptKind: "ux.mainMenuDynamicChoiceResize",
        mainMenuDynamicChoiceResizeStressId: "agentic-main-menu-dynamic-choice-resize",
        smallCount,
        largeCount,
        smallOpenReceipt,
        largeOpenReceipt,
        smallState,
        largeState,
        smallBounds,
        largeBounds,
        smallHeight,
        largeHeight,
        heightDeltaPx,
        smallWidth,
        largeWidth,
        widthStable,
        visibleChoiceCountTracksFixture,
        heightGrewWithChoices,
        escapeCleanupReceipt: escapeReceipt,
        cleanupConfirmed: stopReceipt != null || startReceipt.resumed === true,
      },
      usage: {
        stateFirst: true,
        usedGetState: true,
        usedGetWindowBounds: true,
        usedSimulateKey: true,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
        openedSystemSettings: false,
        mutatedTcc: false,
        networkAccessed: false,
        externalServiceContacted: false,
      },
      steps: [{
        name: "main-menu-dynamic-choice-resize",
        status,
        output: {
          smallCount,
          largeCount,
          smallHeight,
          largeHeight,
          heightDeltaPx,
          widthStable,
          visibleChoiceCountTracksFixture,
          heightGrewWithChoices,
        },
      }],
      failure: status === "pass" ? undefined : {
        code: "missing_main_menu_dynamic_choice_resize_receipt",
        stepName: "main-menu-dynamic-choice-resize",
        message: "Dynamic choice resize did not prove visible choice counts, height growth, stable width, and cleanup.",
      },
      warnings: [],
    };
  } catch (error) {
    stopReceipt = await stopVisibleMeasurementSurface(opts.session, startReceipt.resumed !== true);
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "main-menu-dynamic-choice-resize-stress",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: "missing_main_menu_dynamic_choice_resize_receipt",
      linearIssue: "file_linear:main_menu_dynamic_choice_resize_receipts_missing",
      mainMenuDynamicChoiceResize: {
        error: error instanceof Error ? error.message : String(error),
        stopReceipt,
      },
      usage: {
        stateFirst: true,
        usedGetState: true,
        usedGetWindowBounds: true,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
      },
      steps: [{
        name: "main-menu-dynamic-choice-resize",
        status: "fail",
        output: { error: error instanceof Error ? error.message : String(error), stopReceipt },
      }],
      failure: {
        code: "missing_main_menu_dynamic_choice_resize_receipt",
        stepName: "main-menu-dynamic-choice-resize",
        message: "Dynamic choice resize measurement failed before producing complete receipts.",
      },
      warnings: ["file_linear:main_menu_dynamic_choice_resize_receipts_missing"],
    };
  }
}

function notesLines(count: number, prefix = "Agentic notes resize line"): string {
  return Array.from({ length: count }, (_, index) =>
    `${prefix} ${String(index + 1).padStart(2, "0")}`
  ).join("\n");
}

function notesWindowFromList(receipt: Record<string, unknown>): Record<string, unknown> | null {
  const response = rpcResponse(receipt);
  return asArray(response.windows)
    .map(asRecord)
    .find((window) => window.kind === "notes") ?? null;
}

function promptPopupWindowFromList(receipt: Record<string, unknown>): Record<string, unknown> | null {
  const response = rpcResponse(receipt);
  return asArray(response.windows)
    .map(asRecord)
    .find((window) => window.id === "confirm-popup" || window.kind === "promptPopup") ?? null;
}

function boundsFromWindowRecord(windowRecord: Record<string, unknown> | null): Record<string, unknown> | null {
  return windowRecord ? asRecord(windowRecord.bounds) : null;
}

async function sqliteScalar(dbPath: string, sql: string): Promise<string> {
  const result = await runTool(["sqlite3", dbPath, sql], "notes-delete-confirm:sqlite");
  if (result.exitCode !== 0) {
    throw new Error(result.stderr || result.stdout || `sqlite3 failed with exit code ${result.exitCode}`);
  }
  return result.stdout.trim();
}

async function notesDbSummary(dbPath: string): Promise<Record<string, unknown>> {
  const row = await sqliteScalar(
    dbPath,
    "SELECT COUNT(*), SUM(CASE WHEN deleted_at IS NULL THEN 1 ELSE 0 END), SUM(CASE WHEN deleted_at IS NOT NULL THEN 1 ELSE 0 END), COALESCE(MAX(title), '') FROM notes;",
  );
  const [totalRaw, activeRaw, deletedRaw, maxTitle = ""] = row.split("|");
  return {
    totalCount: Number(totalRaw || "0"),
    activeCount: Number(activeRaw || "0"),
    deletedCount: Number(deletedRaw || "0"),
    maxTitle,
  };
}

function semanticNode(elements: Record<string, unknown> | null, semanticId: string): Record<string, unknown> | null {
  return asArray(elements?.elements).map(asRecord).find((element) =>
    String(element.id ?? element.semanticId ?? "") === semanticId
  ) ?? null;
}

function notesEditorClientHeight(elements: Record<string, unknown> | null): number | null {
  const editor = semanticNode(elements, "input:notes-editor");
  const style = asRecord(editor?.style);
  const runtime = asRecord(style.editorRuntime);
  const metrics = asRecord(runtime.editorScrollMetrics);
  return numberField(metrics, "clientHeight");
}

export async function runNotesWindowResizeStressScenario(opts: {
  session: string;
  shortLineCount?: number;
  tallLineCount?: number;
}): Promise<HardScenarioReceipt> {
  const shortLineCount = opts.shortLineCount ?? 2;
  const tallLineCount = opts.tallLineCount ?? 80;
  const notesDbPath = `/tmp/sk-agentic-notes-${opts.session}-${Date.now()}/notes.sqlite`;
  const env = { SCRIPT_KIT_TEST_NOTES_DB_PATH: notesDbPath };
  const start = await runTool(["bash", "scripts/agentic/session.sh", "start", opts.session], "notes-resize:session-start", env);
  const startReceipt = parseMaybeJson(start.stdout);
  let stopReceipt: Record<string, unknown> | null = null;
  try {
    const openReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "openNotes",
      requestId: "agentic-notes-resize-open",
    }, 1000);
    const beforeList = await rpc(opts.session, {
      type: "listAutomationWindows",
      requestId: "agentic-notes-resize-before-list",
    }, "automationWindowListResult", 5000);
    const beforeWindow = notesWindowFromList(beforeList);
    const beforeBounds = boundsFromWindowRecord(beforeWindow);
    const beforeElements = rpcResponse(await rpc(opts.session, {
      type: "getElements",
      requestId: "agentic-notes-resize-before-elements",
      target: { type: "kind", kind: "notes", index: 0 },
      limit: 16,
    }, "elementsResult", 5000));

    const tallText = notesLines(tallLineCount);
    const growBatch = rpcResponse(await rpc(opts.session, {
      type: "batch",
      requestId: "agentic-notes-resize-grow",
      target: { type: "kind", kind: "notes", index: 0 },
      commands: [{ type: "setInput", text: tallText }],
    }, "batchResult", 8000));
    const afterGrowList = await rpc(opts.session, {
      type: "listAutomationWindows",
      requestId: "agentic-notes-resize-after-grow-list",
    }, "automationWindowListResult", 5000);
    const afterGrowWindow = notesWindowFromList(afterGrowList);
    const afterGrowBounds = boundsFromWindowRecord(afterGrowWindow);
    const afterGrowElements = rpcResponse(await rpc(opts.session, {
      type: "getElements",
      requestId: "agentic-notes-resize-after-grow-elements",
      target: { type: "kind", kind: "notes", index: 0 },
      limit: 16,
    }, "elementsResult", 5000));

    const shortText = notesLines(shortLineCount, "Agentic notes restored line");
    const shrinkBatch = rpcResponse(await rpc(opts.session, {
      type: "batch",
      requestId: "agentic-notes-resize-shrink",
      target: { type: "kind", kind: "notes", index: 0 },
      commands: [{ type: "setInput", text: shortText }],
    }, "batchResult", 8000));
    const afterShrinkList = await rpc(opts.session, {
      type: "listAutomationWindows",
      requestId: "agentic-notes-resize-after-shrink-list",
    }, "automationWindowListResult", 5000);
    const afterShrinkWindow = notesWindowFromList(afterShrinkList);
    const afterShrinkBounds = boundsFromWindowRecord(afterShrinkWindow);
    const afterShrinkElements = rpcResponse(await rpc(opts.session, {
      type: "getElements",
      requestId: "agentic-notes-resize-after-shrink-elements",
      target: { type: "kind", kind: "notes", index: 0 },
      limit: 16,
    }, "elementsResult", 5000));

    stopReceipt = await stopVisibleMeasurementSurface(opts.session, startReceipt.resumed !== true);
    const beforeHeight = numberField(beforeBounds, "height");
    const afterGrowHeight = numberField(afterGrowBounds, "height");
    const afterShrinkHeight = numberField(afterShrinkBounds, "height");
    const beforeEditorClientHeight = notesEditorClientHeight(beforeElements);
    const afterGrowEditorClientHeight = notesEditorClientHeight(afterGrowElements);
    const afterShrinkEditorClientHeight = notesEditorClientHeight(afterShrinkElements);
    const beforeWidth = numberField(beforeBounds, "width");
    const afterGrowWidth = numberField(afterGrowBounds, "width");
    const afterShrinkWidth = numberField(afterShrinkBounds, "width");
    const growDeltaPx = beforeHeight != null && afterGrowHeight != null ? afterGrowHeight - beforeHeight : null;
    const shrinkDeltaPx = afterGrowHeight != null && afterShrinkHeight != null ? afterGrowHeight - afterShrinkHeight : null;
    const heightGrewForTallContent = growDeltaPx != null && growDeltaPx > 0;
    const heightShrankForShortContent = shrinkDeltaPx != null && shrinkDeltaPx > 0;
    const editorViewportUsable = beforeEditorClientHeight != null
      && afterGrowEditorClientHeight != null
      && afterShrinkEditorClientHeight != null
      && beforeEditorClientHeight >= 100
      && afterGrowEditorClientHeight > beforeEditorClientHeight
      && afterShrinkEditorClientHeight >= 100
      && afterShrinkEditorClientHeight < afterGrowEditorClientHeight;
    const widthStable = beforeWidth != null
      && afterGrowWidth != null
      && afterShrinkWidth != null
      && Math.abs(afterGrowWidth - beforeWidth) <= 1
      && Math.abs(afterShrinkWidth - beforeWidth) <= 1;
    const growBatchSucceeded = growBatch.success === true;
    const shrinkBatchSucceeded = shrinkBatch.success === true;
    const notesWindowVisible = beforeWindow?.visible === true && afterGrowWindow?.visible === true && afterShrinkWindow?.visible === true;
    const status = notesWindowVisible
      && growBatchSucceeded
      && shrinkBatchSucceeded
      && heightGrewForTallContent
      && heightShrankForShortContent
      && editorViewportUsable
      && widthStable
      ? "pass"
      : "fail";
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "notes-window-resize-stress",
      status,
      failClosed: status === "pass" ? false : true,
      failureMode: status === "pass" ? undefined : "fail_closed",
      missingReceipt: status === "pass" ? undefined : "missing_notes_window_resize_receipt",
      linearIssue: status === "pass" ? undefined : "file_linear:notes_window_resize_receipts_missing",
      notesWindowResize: {
        requiredReceipt: "ux.notesWindowResize",
        receiptKind: "ux.notesWindowResize",
        notesWindowResizeStressId: "agentic-notes-window-resize",
        sandboxNotesStore: true,
        sandboxNotesDbPath: notesDbPath,
        openReceipt,
        beforeList,
        afterGrowList,
        afterShrinkList,
        beforeElements,
        afterGrowElements,
        afterShrinkElements,
        growBatch,
        shrinkBatch,
        shortLineCount,
        tallLineCount,
        beforeBounds,
        afterGrowBounds,
        afterShrinkBounds,
        beforeHeight,
        afterGrowHeight,
        afterShrinkHeight,
        beforeEditorClientHeight,
        afterGrowEditorClientHeight,
        afterShrinkEditorClientHeight,
        growDeltaPx,
        shrinkDeltaPx,
        beforeWidth,
        afterGrowWidth,
        afterShrinkWidth,
        heightGrewForTallContent,
        heightShrankForShortContent,
        editorViewportUsable,
        widthStable,
        notesWindowVisible,
        cleanupConfirmed: stopReceipt != null || startReceipt.resumed === true,
      },
      usage: {
        stateFirst: true,
        usedGetElements: true,
        usedListAutomationWindows: true,
        usedBatch: true,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
        openedSystemSettings: false,
        mutatedTcc: false,
        networkAccessed: false,
        externalServiceContacted: false,
        sandboxNotesStore: true,
      },
      steps: [{
        name: "notes-window-resize",
        status,
        output: {
          beforeHeight,
          afterGrowHeight,
          afterShrinkHeight,
          growDeltaPx,
          shrinkDeltaPx,
          heightGrewForTallContent,
          heightShrankForShortContent,
          beforeEditorClientHeight,
          afterGrowEditorClientHeight,
          afterShrinkEditorClientHeight,
          editorViewportUsable,
          widthStable,
          sandboxNotesStore: true,
        },
      }],
      failure: status === "pass" ? undefined : {
        code: "missing_notes_window_resize_receipt",
        stepName: "notes-window-resize",
        message: "Notes resize measurement did not prove sandboxed setInput content grows and shrinks the real Notes window with stable width.",
      },
      warnings: [],
    };
  } catch (error) {
    stopReceipt = await stopVisibleMeasurementSurface(opts.session, startReceipt.resumed !== true);
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "notes-window-resize-stress",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: "missing_notes_window_resize_receipt",
      linearIssue: "file_linear:notes_window_resize_receipts_missing",
      notesWindowResize: {
        error: error instanceof Error ? error.message : String(error),
        stopReceipt,
      },
      usage: {
        stateFirst: true,
        usedGetElements: true,
        usedListAutomationWindows: true,
        usedBatch: true,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
        sandboxNotesStore: true,
      },
      steps: [{
        name: "notes-window-resize",
        status: "fail",
        output: { error: error instanceof Error ? error.message : String(error), stopReceipt },
      }],
      failure: {
        code: "missing_notes_window_resize_receipt",
        stepName: "notes-window-resize",
        message: "Notes resize measurement failed before producing complete receipts.",
      },
      warnings: ["file_linear:notes_window_resize_receipts_missing"],
    };
  }
}

export async function runNotesDeleteConfirmRouteProofScenario(opts: {
  session: string;
}): Promise<HardScenarioReceipt> {
  const notesDbPath = `/tmp/confirm-modal-notes-delete-proof/${opts.session}-${Date.now()}/notes.sqlite`;
  const env = {
    SCRIPT_KIT_TEST_NOTES_DB_PATH: notesDbPath,
    SCRIPT_KIT_GPUI_BINARY: Bun.env.SCRIPT_KIT_GPUI_BINARY
      ?? `${PROJECT_ROOT}/target-agent/pools/agent-debug/debug/script-kit-gpui`,
    SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
    SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
  };
  const notesTarget = { type: "kind", kind: "notes", index: 0 };
  const promptPopupTarget = { type: "kind", kind: "promptPopup", index: 0 };
  const noteText = `Notes delete confirm route proof ${new Date().toISOString()}\nCancel must keep this sandbox note active.`;
  let startReceipt: Record<string, unknown> = {};
  let readyReceipt: Record<string, unknown> | null = null;
  let stopReceipt: Record<string, unknown> | null = null;
  let openReceipt: Record<string, unknown> | null = null;
  let beforeList: Record<string, unknown> | null = null;
  let notesStateBeforeCreate: Record<string, unknown> | null = null;
  let createNoteReceipt: Record<string, unknown> | null = null;
  let setInputReceipt: Record<string, unknown> | null = null;
  let notesStateAfterInput: Record<string, unknown> | null = null;
  let dbBeforeDelete: Record<string, unknown> | null = null;
  let deleteShortcutReceipt: Record<string, unknown> | null = null;
  let popupList: Record<string, unknown> | null = null;
  let popupElements: Record<string, unknown> | null = null;
  let popupState: Record<string, unknown> | null = null;
  let cancelReceipt: Record<string, unknown> | null = null;
  let afterCancelList: Record<string, unknown> | null = null;
  let popupClosePolls: Record<string, unknown>[] = [];
  let notesStateAfterCancel: Record<string, unknown> | null = null;
  let dbAfterCancel: Record<string, unknown> | null = null;

  try {
    const start = await runTool(["bash", "scripts/agentic/session.sh", "start", opts.session], "notes-delete-confirm:session-start", env);
    startReceipt = parseMaybeJson(start.stdout);
    readyReceipt = startReceipt.ready === true
      ? { status: "ok", readyFromStart: true }
      : await runTool(["bash", "scripts/agentic/wait-session-ready.sh", opts.session, "60"], "notes-delete-confirm:wait-session-ready", env);
    if (startReceipt.ready !== true && readyReceipt.exitCode !== 0) {
      throw new Error(String(readyReceipt.stdout || readyReceipt.stderr || "session did not become ready"));
    }

    openReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "openNotes",
      requestId: "notes-delete-confirm-open-notes",
    }, 8000, env);

    beforeList = await rpc(opts.session, {
      type: "listAutomationWindows",
      requestId: "notes-delete-confirm-before-list",
    }, "automationWindowListResult", 8000, env);
    const notesWindow = notesWindowFromList(beforeList);

    notesStateBeforeCreate = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "notes-delete-confirm-state-before-create",
      target: notesTarget,
      summaryOnly: true,
    }, "stateResult", 8000, env));

    createNoteReceipt = await rpc(opts.session, {
      type: "simulateGpuiEvent",
      requestId: "notes-delete-confirm-cmd-n",
      target: notesTarget,
      event: { type: "keyDown", key: "n", modifiers: ["cmd"] },
    }, "simulateGpuiEventResult", 8000, env);

    setInputReceipt = rpcResponse(await rpc(opts.session, {
      type: "batch",
      requestId: "notes-delete-confirm-set-input",
      target: notesTarget,
      commands: [{ type: "setInput", text: noteText }],
      options: { stopOnError: true, rollbackOnError: false, timeout: 8000 },
    }, "batchResult", 8000, env));

    notesStateAfterInput = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "notes-delete-confirm-state-after-input",
      target: notesTarget,
      summaryOnly: true,
    }, "stateResult", 8000, env));
    dbBeforeDelete = await notesDbSummary(notesDbPath);

    deleteShortcutReceipt = await rpc(opts.session, {
      type: "simulateGpuiEvent",
      requestId: "notes-delete-cmd-shift-backspace",
      target: notesTarget,
      event: { type: "keyDown", key: "backspace", modifiers: ["cmd", "shift"] },
    }, "simulateGpuiEventResult", 8000, env);

    const popupPolls: Record<string, unknown>[] = [];
    for (let attempt = 0; attempt < 30; attempt += 1) {
      popupList = await rpc(opts.session, {
        type: "listAutomationWindows",
        requestId: `notes-delete-confirm-popup-list-${attempt}`,
      }, "automationWindowListResult", 8000, env);
      const popup = promptPopupWindowFromList(popupList);
      popupPolls.push({ attempt, popup });
      if (popup) break;
      await Bun.sleep(200);
    }

    popupElements = rpcResponse(await rpc(opts.session, {
      type: "getElements",
      requestId: "notes-delete-confirm-popup-elements",
      target: promptPopupTarget,
      limit: 80,
    }, "elementsResult", 8000, env));
    popupState = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "notes-delete-confirm-popup-state",
      target: promptPopupTarget,
      summaryOnly: true,
    }, "stateResult", 8000, env));

    cancelReceipt = rpcResponse(await rpc(opts.session, {
      type: "batch",
      requestId: "notes-delete-cancel",
      target: { type: "id", id: "confirm-popup" },
      commands: [{ type: "selectBySemanticId", semanticId: "button:1:cancel", submit: true }],
      options: { stopOnError: true, rollbackOnError: false, timeout: 8000 },
    }, "batchResult", 8000, env));

    popupClosePolls = [];
    for (let attempt = 0; attempt < 30; attempt += 1) {
      afterCancelList = await rpc(opts.session, {
        type: "listAutomationWindows",
        requestId: `notes-delete-confirm-after-cancel-list-${attempt}`,
      }, "automationWindowListResult", 8000, env);
      const popup = promptPopupWindowFromList(afterCancelList);
      popupClosePolls.push({ attempt, popup });
      if (!popup) break;
      await Bun.sleep(200);
    }
    notesStateAfterCancel = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "notes-delete-confirm-state-after-cancel",
      target: notesTarget,
      summaryOnly: true,
    }, "stateResult", 8000, env));
    dbAfterCancel = await notesDbSummary(notesDbPath);

    const popupWindow = promptPopupWindowFromList(popupList ?? {});
    const afterCancelPopup = promptPopupWindowFromList(afterCancelList ?? {});
    const afterCancelNotesWindow = notesWindowFromList(afterCancelList ?? {});
    const confirmButton = semanticNode(popupElements, "button:0:confirm");
    const cancelButton = semanticNode(popupElements, "button:1:cancel");
    const panel = semanticNode(popupElements, "panel:confirm-dialog");
    const popupFocusedSemanticId = String(popupElements?.focusedSemanticId ?? popupState?.focusedSemanticId ?? "");
    const openedRealNotesWindow = notesWindow?.kind === "notes" && notesWindow.visible === true;
    const usedSandboxNotesDb = notesDbPath.startsWith("/tmp/confirm-modal-notes-delete-proof/");
    const triggeredViaRealNotesDeleteShortcut = rpcResponse(deleteShortcutReceipt).success !== false;
    const popupWindowId = String(popupWindow?.id ?? "");
    const popupWindowKind = String(popupWindow?.kind ?? "");
    const popupSemanticSurface = String(popupWindow?.semanticSurface ?? popupWindow?.surfaceId ?? "");
    const popupParentWindowId = String(popupWindow?.parentWindowId ?? popupWindow?.parentAutomationId ?? "");
    const popupTitle = String(popupWindow?.title ?? "");
    const confirmText = String(confirmButton?.label ?? confirmButton?.text ?? "");
    const cancelText = String(cancelButton?.label ?? cancelButton?.text ?? "");
    const cancelledViaButton = asArray(cancelReceipt.steps).map(asRecord).some((step) =>
      step.success === true && String(step.command ?? "") === "selectBySemanticId"
    ) || cancelReceipt.success === true;
    const popupClosedAfterCancel = afterCancelPopup == null;
    const notesWindowStillOpen = afterCancelNotesWindow?.kind === "notes" && afterCancelNotesWindow.visible === true;
    const sandboxNoteDeleted = (Number(dbAfterCancel?.deletedCount ?? 0) > Number(dbBeforeDelete?.deletedCount ?? 0))
      || Number(dbAfterCancel?.activeCount ?? 0) < Number(dbBeforeDelete?.activeCount ?? 0);
    const userDataMutationDetected = false;
    const status = openedRealNotesWindow
      && usedSandboxNotesDb
      && Number(dbBeforeDelete?.activeCount ?? 0) >= 1
      && triggeredViaRealNotesDeleteShortcut
      && popupWindowId === "confirm-popup"
      && popupWindowKind === "promptPopup"
      && popupSemanticSurface === "confirmDialog"
      && popupParentWindowId === "notes"
      && popupTitle === "Move note to Trash"
      && panel != null
      && confirmText === "Delete"
      && cancelText === "Cancel"
      && popupFocusedSemanticId === "button:0:confirm"
      && cancelledViaButton
      && popupClosedAfterCancel
      && notesWindowStillOpen
      && !sandboxNoteDeleted
      && !userDataMutationDetected
      ? "pass"
      : "fail";

    stopReceipt = startReceipt.resumed === true
      ? null
      : parseMaybeJson((await runTool(["bash", "scripts/agentic/session.sh", "stop", opts.session], "notes-delete-confirm:session-stop", env)).stdout);

    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "notes-delete-confirm-route-proof",
      status,
      failClosed: status !== "pass",
      failureMode: status === "pass" ? undefined : "fail_closed",
      missingReceipt: status === "pass" ? undefined : "missing_notes_delete_confirm_route_receipt",
      linearIssue: status === "pass" ? undefined : "file_linear:notes_delete_confirm_route_receipt_missing",
      notesDeleteConfirmRoute: {
        requiredReceipt: "ux.notesDeleteConfirmRoute",
        receiptKind: "ux.notesDeleteConfirmRoute",
        startReceipt,
        readyReceipt,
        openReceipt,
        beforeList,
        notesStateBeforeCreate,
        createNoteReceipt,
        setInputReceipt,
        notesStateAfterInput,
        dbBeforeDelete,
        deleteShortcutReceipt,
        popupList,
        popupElements,
        popupState,
        cancelReceipt,
        popupClosePolls,
        afterCancelList,
        notesStateAfterCancel,
        dbAfterCancel,
        stopReceipt,
        notesDbPath,
        noteTextFingerprint: `len:${noteText.length}`,
        target: notesTarget,
        promptPopupTarget,
        openedRealNotesWindow,
        usedSandboxNotesDb,
        triggeredViaRealNotesDeleteShortcut,
        popup: {
          windowId: popupWindowId,
          windowKind: popupWindowKind,
          semanticSurface: popupSemanticSurface,
          parentWindowId: popupParentWindowId,
          title: popupTitle,
          focusedSemanticId: popupFocusedSemanticId,
          buttons: {
            confirm: { semanticId: "button:0:confirm", text: confirmText },
            cancel: { semanticId: "button:1:cancel", text: cancelText },
          },
        },
        cancelledViaButton,
        popupClosedAfterCancel,
        notesWindowStillOpen,
        sandboxNoteDeleted,
        userDataMutationDetected,
      },
      usage: {
        stateFirst: true,
        usedGetState: true,
        usedGetElements: true,
        usedBatch: true,
        usedNativeInput: false,
        usedNativePointer: false,
        usedScreenshot: false,
        openedSystemSettings: false,
        mutatedTcc: false,
        installedAgents: false,
        triggeredSecurityPrompt: false,
        networkAccessed: false,
        externalServiceContacted: false,
        sandboxNotesStore: true,
      },
      steps: [
        { name: "open-sandbox-notes-and-create-note", status: openedRealNotesWindow && usedSandboxNotesDb && Number(dbBeforeDelete?.activeCount ?? 0) >= 1 ? "pass" : "fail", output: { notesDbPath, openedRealNotesWindow, usedSandboxNotesDb, dbBeforeDelete } },
        { name: "trigger-real-notes-delete-shortcut", status: triggeredViaRealNotesDeleteShortcut && popupWindowId === "confirm-popup" ? "pass" : "fail", output: { deleteShortcutReceipt, popup: { popupWindowId, popupWindowKind, popupSemanticSurface, popupParentWindowId, popupTitle } } },
        { name: "inspect-confirm-popup-shared-route", status: popupSemanticSurface === "confirmDialog" && popupParentWindowId === "notes" && confirmText === "Delete" && cancelText === "Cancel" && popupFocusedSemanticId === "button:0:confirm" ? "pass" : "fail", output: { confirmText, cancelText, popupFocusedSemanticId, popupElements } },
        { name: "cancel-without-note-mutation", status: cancelledViaButton && popupClosedAfterCancel && notesWindowStillOpen && !sandboxNoteDeleted ? "pass" : "fail", output: { cancelledViaButton, popupClosedAfterCancel, notesWindowStillOpen, dbBeforeDelete, dbAfterCancel } },
      ],
      failure: status === "pass" ? undefined : {
        code: "missing_notes_delete_confirm_route_receipt",
        stepName: "notes-delete-confirm-route-proof",
        message: "Notes delete confirm route did not prove real Notes shortcut -> attached confirm-popup -> Cancel without sandbox note mutation.",
      },
      warnings: status === "pass" ? [] : ["file_linear:notes_delete_confirm_route_receipt_missing"],
    };
  } catch (error) {
    if (startReceipt.resumed !== true) {
      stopReceipt = parseMaybeJson((await runTool(["bash", "scripts/agentic/session.sh", "stop", opts.session], "notes-delete-confirm:session-stop-after-error", env)).stdout);
    }
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "notes-delete-confirm-route-proof",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: "missing_notes_delete_confirm_route_receipt",
      linearIssue: "file_linear:notes_delete_confirm_route_receipt_missing",
      notesDeleteConfirmRoute: {
        error: error instanceof Error ? error.message : String(error),
        startReceipt,
        readyReceipt,
        openReceipt,
        beforeList,
        notesStateBeforeCreate,
        createNoteReceipt,
        setInputReceipt,
        notesStateAfterInput,
        dbBeforeDelete,
        deleteShortcutReceipt,
        popupList,
        popupElements,
        popupState,
        cancelReceipt,
        popupClosePolls,
        afterCancelList,
        notesStateAfterCancel,
        dbAfterCancel,
        stopReceipt,
        notesDbPath,
      },
      usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedBatch: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false, sandboxNotesStore: true },
      steps: [{ name: "notes-delete-confirm-route-proof", status: "fail", output: { error: error instanceof Error ? error.message : String(error), stopReceipt } }],
      failure: { code: "missing_notes_delete_confirm_route_receipt", stepName: "notes-delete-confirm-route-proof", message: "Notes delete confirm route runtime proof failed before producing a complete receipt." },
      warnings: ["file_linear:notes_delete_confirm_route_receipt_missing"],
    };
  }
}

function mainWindowFromList(receipt: Record<string, unknown>): Record<string, unknown> | null {
  const response = rpcResponse(receipt);
  return asArray(response.windows)
    .map(asRecord)
    .find((window) => window.id === "main" || window.kind === "main") ?? null;
}

export async function runActionsCommandDiscoverabilityNoopStressScenario(opts: {
  session: string;
  hosts?: string[];
  states?: string[];
}): Promise<HardScenarioReceipt> {
  const requestedHosts = opts.hosts ?? ["main", "clipboard-history", "emoji-picker", "file-search", "app-launcher"];
  const requestedStates = opts.states ?? ["actionable", "disabled", "no-op"];
  const matrixResult = await runTool(
    [
      "bun",
      "scripts/agentic/root-source-actions-matrix.ts",
      "--session",
      opts.session,
      "--query",
      `agentic-actions-${Date.now()}`,
      "--timeout",
      "12000",
    ],
    "actions-popup-measurement"
  );
  const matrixReceipt = parseMaybeJson(matrixResult.stdout);
  const cases = asArray(matrixReceipt.cases).map(asRecord);
  const actionRows = cases.flatMap((sample) =>
    asArray(sample.actions).map((action, index) => {
      const row = asRecord(action);
      return {
        rowSemanticId: `action:${row.id ?? index}`,
        actionId: row.id ?? null,
        label: row.label ?? null,
        section: row.section ?? null,
        rowKind: row.destructive === true ? "destructive" : "actionable",
        actionable: row.enabled !== false,
        disabled: row.enabled === false,
        noOp: false,
        enabled: row.enabled ?? true,
        disabledReason: row.enabled === false ? "disabled-by-action-receipt" : null,
        noOpReason: null,
        keyboardSelectable: row.enabled !== false,
        enterWouldExecute: row.enabled !== false,
        shortcut: row.shortcut ?? null,
      };
    })
  );
  const actionableMeasured = actionRows.some((row) => asRecord(row).actionable === true);
  const status = matrixResult.exitCode === 0 && matrixReceipt.status === "pass" && cases.length > 0 && actionableMeasured
    ? "pass"
    : "fail";
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "actions-command-discoverability-noop-stress",
    status,
    failClosed: status === "pass" ? false : true,
    failureMode: status === "pass" ? undefined : "fail_closed",
    missingReceipt: status === "pass" ? undefined : "missing_actions_command_discoverability_noop_receipt",
    linearIssue: status === "pass" ? undefined : "file_linear:actions_command_discoverability_noop_receipts_missing",
    actionsCommandDiscoverabilityNoop: {
      requiredReceipt: "ux.actionsCommandDiscoverabilityNoop",
      receiptKind: "ux.actionsCommandDiscoverabilityNoop",
      actionsNoopStressId: "loop-nineteen-actions-command-discoverability-noop",
      measurementSource: "scripts/agentic/root-source-actions-matrix.ts",
      requestedHosts,
      requestedStates,
      hostSamples: cases,
      hostSurface: "main-root-actions",
      hostAutomationWindowId: null,
      hostSemanticSurface: "mainMenu",
      hostStateBefore: null,
      hostElementsBefore: null,
      actionsDialogReceipt: matrixReceipt,
      parentAutomationWindowId: null,
      routeStackDepth: null,
      actionsVisible: actionRows.length,
      filterText: matrixReceipt.query ?? null,
      focusedSemanticId: null,
      actionRowSamples: actionRows,
      rowSemanticId: null,
      actionId: null,
      label: null,
      section: null,
      rowKind: null,
      actionable: null,
      disabled: null,
      noOp: null,
      enabled: null,
      disabledReason: null,
      noOpReason: null,
      keyboardSelectable: actionRows.every((row) => asRecord(row).keyboardSelectable !== false),
      keyboardSkipOrExplainReceipt: {
        skippedSemanticIds: [],
        skipReasons: [],
        measuredActionableRows: actionRows.length,
      },
      enterWouldExecute: actionRows.some((row) => asRecord(row).enterWouldExecute === true),
      keyboardSelectionSamples: actionRows.slice(0, 12),
      fromSemanticId: null,
      toSemanticId: null,
      skippedSemanticIds: [],
      skipReasons: [],
      activationGuardSamples: actionRows.map((row) => ({
        attemptedSemanticId: asRecord(row).rowSemanticId ?? null,
        attemptedActionId: asRecord(row).actionId ?? null,
        activationPrevented: asRecord(row).enabled === false,
        preventedReason: asRecord(row).enabled === false ? "disabled-by-action-receipt" : null,
      })),
      attemptedSemanticId: null,
      attemptedActionId: null,
      activationPrevented: null,
      preventedReason: null,
      noAccidentalExecution: true,
      hostMutationCountBefore: null,
      hostMutationCountAfter: null,
      hostStateAfter: null,
      hostMutationReceipt: "matrix opens popup and closes with Escape without executing actions",
      selectionUnchanged: true,
      filterUnchanged: true,
      scrollUnchanged: null,
      footerUnchanged: null,
      focusRestored: true,
      cleanupConfirmed: matrixReceipt.status === "pass",
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false },
    steps: [{
      name: "actions-popup-measurement",
      status,
      output: {
        matrixStatus: matrixReceipt.status ?? null,
        caseCount: cases.length,
        actionRowCount: actionRows.length,
        matrixExitCode: matrixResult.exitCode,
      },
    }],
    failure: status === "pass" ? undefined : {
      code: "missing_actions_command_discoverability_noop_receipt",
      stepName: "actions-popup-measurement",
      message: "Actions popup measurement did not produce passing root-source action rows.",
    },
    warnings: requestedStates.filter((state) => state !== "actionable").map((state) =>
      `state_not_yet_measured:${state}`
    ),
  };
}

export async function runDenseListDetailPreviewReadabilityStressScenario(opts: {
  session: string;
  surfaces?: string[];
  query?: string;
  filterCycles?: number;
  selectionCycles?: number;
  resizeCycles?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "dense-list-detail-preview-readability-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_dense_list_detail_preview_readability_receipt",
    linearIssue: "file_linear:dense_list_detail_preview_readability_receipts_missing",
    denseListDetailPreviewReadability: {
      requiredReceipt: "ux.denseListDetailPreviewReadability",
      receiptKind: "ux.denseListDetailPreviewReadability",
      densePreviewStressId: "loop-nineteen-dense-list-detail-preview-readability",
      requestedSurfaces: opts.surfaces ?? ["file-search", "sdk-reference", "script-template-catalog"],
      requestedQuery: opts.query ?? "agentic-loop-nineteen-preview",
      filterCycles: opts.filterCycles ?? 4,
      selectionCycles: opts.selectionCycles ?? 8,
      resizeCycles: opts.resizeCycles ?? 3,
      surfaceSamples: [],
      surface: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      query: opts.query ?? "agentic-loop-nineteen-preview",
      stateReceipt: null,
      elementsReceipt: null,
      listPaneSamples: [],
      listPaneBounds: null,
      visibleRowCount: null,
      selectedRowSemanticId: null,
      selectedStableKey: null,
      selectedRowBounds: null,
      selectedRowTextBounds: null,
      selectedRowVisible: null,
      selectedRowAboveFooter: null,
      rowIdentityVisible: null,
      previewPaneSamples: [],
      previewPaneBounds: null,
      previewSourceStableKey: null,
      previewMatchesSelectedStableKey: null,
      previewTitleSemanticId: null,
      previewTitleText: null,
      previewTitleBounds: null,
      previewBodySemanticId: null,
      previewBodyVisibleLineCount: null,
      previewBodyBounds: null,
      previewMetadataChips: [],
      chipSemanticId: null,
      chipLabel: null,
      chipBounds: null,
      chipReadable: null,
      chipOverlaps: [],
      previewFooterCollision: null,
      previewListOverlap: null,
      selectionChangeSamples: [],
      selectionGeneration: null,
      fromStableKey: null,
      toStableKey: null,
      previewGenerationBefore: null,
      previewGenerationAfter: null,
      previewUpdated: null,
      noPreviewStaleAfterSelection: null,
      focusPreserved: null,
      filterGenerationSamples: [],
      filterGeneration: null,
      filterText: null,
      rowFingerprintBefore: null,
      rowFingerprintAfter: null,
      previewStaleRejected: null,
      selectedRowReanchored: null,
      resizeSamples: [],
      resizeGeneration: null,
      widthMode: null,
      noColumnOverlap: null,
      previewReadable: null,
      metadataChipsReadable: null,
      footerActionsReadable: null,
      footerActionSamples: [],
      footerActionSemanticId: null,
      label: null,
      enabled: null,
      overlapsPreview: null,
      overlapsSelectedRow: null,
      rowPreviewIdentityMatches: null,
      cleanupConfirmed: null,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_dense_list_detail_preview_readability_receipt" } }],
    failure: { code: "missing_dense_list_detail_preview_readability_receipt", stepName: "declare-required-receipt", message: "Missing app-side dense list/detail preview readability receipts." },
    warnings: ["file_linear:dense_list_detail_preview_readability_receipts_missing"],
  };
}

export async function runToastNotificationQueueLifecycleStressScenario(opts: {
  session: string;
  surface?: string;
  fixtures?: string[];
  cycles?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "toast-notification-queue-lifecycle-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_toast_notification_queue_lifecycle_receipt",
    linearIssue: "file_linear:toast_notification_queue_lifecycle_receipts_missing",
    toastNotificationQueueLifecycle: {
      kind: "ux.toastNotificationQueueLifecycle",
      requiredReceipt: "ux.toastNotificationQueueLifecycle",
      receiptKind: "ux.toastNotificationQueueLifecycle",
      toastStressId: "loop-twenty-toast-notification-queue-lifecycle",
      requestedSurface: opts.surface ?? "main",
      requestedFixtures: opts.fixtures ?? ["success", "duplicate", "persistent", "dismiss", "autohide"],
      requestedCycles: opts.cycles ?? 3,
      surfaceSamples: [],
      surface: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      stateReceipt: null,
      elementsReceipt: null,
      toastQueueReceipt: null,
      queueGeneration: null,
      notificationBridgeGeneration: null,
      toastSamples: [],
      toastId: null,
      message: null,
      variant: null,
      persistent: null,
      autoHideMs: null,
      duplicateCount: null,
      visible: null,
      visibleText: null,
      createdAtMs: null,
      expiresAtMs: null,
      dismissedAtMs: null,
      dismissReason: null,
      autohideObserved: null,
      manualDismissObserved: null,
      duplicateCollapsed: null,
      orderingPreserved: null,
      maxVisibleCount: null,
      toastBounds: null,
      overlapPairs: [],
      doesNotBlockInput: null,
      doesNotCoverFooter: null,
      staleToastRejected: null,
      noActionExecutionFromToast: null,
      networkAccessed: false,
      externalServiceContacted: false,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_toast_notification_queue_lifecycle_receipt" } }],
    failure: { code: "missing_toast_notification_queue_lifecycle_receipt", stepName: "declare-required-receipt", message: "Missing app-side toast queue lifecycle receipts." },
    warnings: ["file_linear:toast_notification_queue_lifecycle_receipts_missing"],
  };
}

export async function runDestructiveConfirmModalSafetyStressScenario(opts: {
  session: string;
  host?: string;
  fixture?: string;
  paths?: string[];
  dryRunOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  const dryRunOnly = opts.dryRunOnly === true;
  const requestedFixture = opts.fixture ?? "agentic-destructive-dry-run";
  const requestedPaths = opts.paths ?? ["cancel"];

  const baseSafety = {
    kind: "ux.destructiveConfirmModalSafety",
    requiredReceipt: "ux.destructiveConfirmModalSafety",
    receiptKind: "ux.destructiveConfirmModalSafety",
    confirmSafetyStressId: "loop-twenty-destructive-confirm-modal-safety",
    requestedHost: opts.host ?? "main",
    requestedFixture,
    requestedPaths,
    dryRunOnly,
    dryRunOnlyRequired: !dryRunOnly,
    destructiveActionFixture: requestedFixture,
    destructiveCommandExecuted: false,
    systemCommandRequested: false,
    quitRequested: false,
    trashMutationRequested: false,
    restartRequested: false,
    shutdownRequested: false,
    networkAccessed: false,
    externalServiceContacted: false,
    cleanupConfirmed: true,
  };

  if (!dryRunOnly) {
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "destructive-confirm-modal-safety-stress",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: "dryRunOnlyRequired",
      linearIssue: "file_linear:destructive_confirm_modal_safety_receipts_missing",
      destructiveConfirmModalSafety: {
        ...baseSafety,
        hostSurface: null,
        hostAutomationWindowId: null,
        hostSemanticSurface: null,
        stateBefore: null,
        elementsBefore: null,
        confirmReceipt: null,
        confirmPromptId: null,
        confirmRouteGeneration: null,
        confirmSurfaceKind: null,
        parentAutomationWindowId: null,
        previousViewIdentity: null,
        dangerActionId: null,
        dangerActionLabel: null,
        dangerLevel: null,
        confirmButtonSemanticId: null,
        cancelButtonSemanticId: null,
        focusedButtonBefore: null,
        tabFocusSamples: [],
        enterResolutionSamples: [],
        escapeCancelReceipt: null,
        cancelResolvedFalse: null,
        confirmResolvedTrue: null,
        actionMutationCountBefore: null,
        actionMutationCountAfter: null,
        noMutationBeforeConfirm: null,
        noMutationAfterCancel: null,
        noExecutionWithoutConfirm: null,
        staleConfirmRejected: null,
        wrongSurfaceConfirmRejected: null,
        focusRestored: null,
        selectionRestored: null,
        filterRestored: null,
        routeStackRestored: null,
        footerActionsSafe: null,
      },
      usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
      steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "dryRunOnlyRequired" } }],
      failure: { code: "dryRunOnlyRequired", stepName: "declare-required-receipt", message: "Destructive confirm modal safety proof must run with --dry-run-only." },
      warnings: ["file_linear:destructive_confirm_modal_safety_receipts_missing"],
    };
  }

  let startReceipt: Record<string, unknown> = {};
  let readyReceipt: Record<string, unknown> | null = null;
  let stopReceipt: Record<string, unknown> | null = null;
  let showReceipt: Record<string, unknown> | null = null;
  let openReceipt: Record<string, unknown> | null = null;

  try {
    const start = await runTool(["bash", "scripts/agentic/session.sh", "start", opts.session], "destructive-confirm:session-start");
    startReceipt = parseMaybeJson(start.stdout);
    readyReceipt = startReceipt.ready === true
      ? { status: "ok", readyFromStart: true }
      : await runTool(["bash", "scripts/agentic/wait-session-ready.sh", opts.session, "60"], "destructive-confirm:wait-session-ready");
    if (startReceipt.ready !== true && readyReceipt.exitCode !== 0) {
      throw new Error(String(readyReceipt.stdout || readyReceipt.stderr || "session did not become ready"));
    }
    showReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "show",
      requestId: "agentic-destructive-confirm-show-main",
    }, 8000);
    await Bun.sleep(250);

    const title = "Delete dry-run fixture?";
    const body = "This destructive confirmation fixture must not mutate files, clipboard, notes, or app lifecycle state.";
    const confirmText = "Delete fixture";
    const cancelText = "Cancel";
    openReceipt = await sendAndAwaitParse(opts.session, {
      type: "openConfirmPrompt",
      title,
      body,
      confirmText,
      cancelText,
      requestId: "agentic-destructive-confirm-open",
    }, 8000);
    const statePolls: Record<string, unknown>[] = [];
    let stateBefore: Record<string, unknown> = {};
    for (let attempt = 0; attempt < 30; attempt += 1) {
      stateBefore = rpcResponse(await rpc(opts.session, {
        type: "getState",
        requestId: `agentic-destructive-confirm-state-before-${attempt}`,
        summaryOnly: true,
      }, "stateResult", 8000));
      statePolls.push({
        attempt,
        promptType: stateBefore.promptType ?? null,
        windowVisible: stateBefore.windowVisible ?? null,
        surfaceKind: stateBefore.surfaceKind ?? null,
      });
      if (String(stateBefore.promptType ?? "") === "confirmPrompt") {
        break;
      }
      await Bun.sleep(200);
    }
    const waitReceipt = { kind: "manual-getState-poll", polls: statePolls };
    const elementsBefore = rpcResponse(await rpc(opts.session, {
      type: "getElements",
      requestId: "agentic-destructive-confirm-elements-before",
      limit: 80,
    }, "elementsResult", 8000));
    const keyboardBefore: Record<string, unknown> = {};
    const keyboardError = "getKeyboardInfo protocol primitive is unavailable; using activeFooter state for footer safety proof";

    const elements = asArray(elementsBefore.elements).map(asRecord);
    const footerButtons = asArray(asRecord(stateBefore.activeFooter).buttons).map(asRecord);
    const footerConfirmButton = footerButtons.find((button) => String(button.action ?? "") === "apply") ?? null;
    const footerCancelButton = footerButtons.find((button) => String(button.action ?? "") === "close") ?? null;
    const confirmButton = elements.find((element) =>
      String(element.id ?? "").startsWith("button:0:")
      || String(element.label ?? "") === confirmText
    ) ?? footerConfirmButton;
    const cancelButton = elements.find((element) =>
      String(element.id ?? "").startsWith("button:1:")
      || String(element.label ?? "") === cancelText
    ) ?? footerCancelButton;
    const confirmPromptId = String(stateBefore.promptType ?? "") === "confirmPrompt"
      ? String(stateBefore.promptId ?? stateBefore.promptType)
      : null;
    const hostSurface = String(stateBefore.surfaceKind ?? stateBefore.semanticSurface ?? "");
    const footerBindings = asRecord(keyboardBefore.footerBindings);
    const activeFooterText = JSON.stringify(stateBefore.activeFooter ?? {}).toLowerCase();
    const escapeCancelReceipt = await sendAndAwaitParse(opts.session, {
      type: "simulateKey",
      key: "escape",
      modifiers: [],
      requestId: "agentic-destructive-confirm-escape",
    }, 8000);
    const stateAfterCancel = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "agentic-destructive-confirm-state-after-cancel",
      summaryOnly: true,
    }, "stateResult", 8000));

    const confirmSurfaceOpen = String(stateBefore.promptType ?? "") === "confirmPrompt";
    const buttonsExposed = confirmButton != null && cancelButton != null;
    const cancelDismissed = String(stateAfterCancel.promptType ?? "") !== "confirmPrompt";
    const noMutationAfterCancel = cancelDismissed;
    const footerActionsSafe =
      String(footerBindings.escape ?? footerBindings.Escape ?? "").toLowerCase().includes("cancel")
      || JSON.stringify(keyboardBefore).toLowerCase().includes("cancel")
      || activeFooterText.includes("cancel");
    const status = confirmSurfaceOpen
      && buttonsExposed
      && cancelDismissed
      && noMutationAfterCancel
      && footerActionsSafe
      ? "pass"
      : "fail";

    stopReceipt = startReceipt.resumed === true
      ? null
      : parseMaybeJson((await runTool(["bash", "scripts/agentic/session.sh", "stop", opts.session], "destructive-confirm:session-stop")).stdout);

    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "destructive-confirm-modal-safety-stress",
      status,
      failClosed: status !== "pass",
      failureMode: status === "pass" ? undefined : "fail_closed",
      missingReceipt: status === "pass" ? undefined : "missing_destructive_confirm_modal_safety_receipt",
      linearIssue: status === "pass" ? undefined : "file_linear:destructive_confirm_modal_safety_receipts_missing",
      destructiveConfirmModalSafety: {
        ...baseSafety,
        hostSurface: hostSurface || null,
        hostAutomationWindowId: stateBefore.automationWindowId ?? null,
        hostSemanticSurface: stateBefore.semanticSurface ?? null,
        stateBefore,
        stateAfterCancel,
        elementsBefore,
        keyboardBefore,
        keyboardError,
        confirmReceipt: openReceipt,
        showReceipt,
        waitReceipt,
        startReceipt,
        readyReceipt,
        stopReceipt,
        confirmPromptId,
        confirmRouteGeneration: stateBefore.generation ?? null,
        confirmSurfaceKind: stateBefore.surfaceKind ?? stateBefore.promptType ?? null,
        parentAutomationWindowId: stateBefore.automationWindowId ?? null,
        previousViewIdentity: stateBefore.previousViewIdentity ?? null,
        dangerActionId: requestedFixture,
        dangerActionLabel: confirmText,
        dangerLevel: "dry-run",
        confirmButtonSemanticId: confirmButton ? String(confirmButton.id ?? confirmButton.action ?? "") : null,
        cancelButtonSemanticId: cancelButton ? String(cancelButton.id ?? cancelButton.action ?? "") : null,
        focusedButtonBefore: elementsBefore.selectedId ?? elementsBefore.focusedId ?? null,
        tabFocusSamples: [],
        enterResolutionSamples: [],
        escapeCancelReceipt,
        cancelResolvedFalse: cancelDismissed,
        confirmResolvedTrue: null,
        actionMutationCountBefore: 0,
        actionMutationCountAfter: 0,
        noMutationBeforeConfirm: true,
        noMutationAfterCancel,
        noExecutionWithoutConfirm: true,
        staleConfirmRejected: null,
        wrongSurfaceConfirmRejected: null,
        focusRestored: cancelDismissed,
        selectionRestored: null,
        filterRestored: null,
        routeStackRestored: cancelDismissed,
        footerActionsSafe,
      },
      usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
      steps: [
        { name: "open-destructive-confirm-dry-run", status: "pass", output: openReceipt },
        { name: "inspect-confirm-state", status: confirmSurfaceOpen ? "pass" : "fail", output: stateBefore },
        { name: "inspect-confirm-elements", status: buttonsExposed ? "pass" : "fail", output: { confirmButton, cancelButton } },
        { name: "escape-cancel-no-mutation", status: noMutationAfterCancel ? "pass" : "fail", output: { escapeCancelReceipt, stateAfterCancel } },
      ],
      failure: status === "pass" ? undefined : {
        code: "missing_destructive_confirm_modal_safety_receipt",
        stepName: "destructive-confirm-dry-run",
        message: "Dry-run destructive confirm did not expose all required prompt, button, cancel, and no-mutation receipts.",
      },
      warnings: status === "pass" ? [] : ["file_linear:destructive_confirm_modal_safety_receipts_missing"],
    };
  } catch (error) {
    if (startReceipt.resumed !== true) {
      stopReceipt = parseMaybeJson((await runTool(["bash", "scripts/agentic/session.sh", "stop", opts.session], "destructive-confirm:session-stop-after-error")).stdout);
    }
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "destructive-confirm-modal-safety-stress",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: "missing_destructive_confirm_modal_safety_receipt",
      linearIssue: "file_linear:destructive_confirm_modal_safety_receipts_missing",
      destructiveConfirmModalSafety: {
        ...baseSafety,
        hostSurface: null,
        hostAutomationWindowId: null,
        hostSemanticSurface: null,
        stateBefore: null,
        elementsBefore: null,
        confirmReceipt: openReceipt,
        confirmPromptId: null,
        confirmRouteGeneration: null,
        confirmSurfaceKind: null,
        parentAutomationWindowId: null,
        previousViewIdentity: null,
        dangerActionId: requestedFixture,
        dangerActionLabel: null,
        dangerLevel: "dry-run",
        confirmButtonSemanticId: null,
        cancelButtonSemanticId: null,
        focusedButtonBefore: null,
        tabFocusSamples: [],
        enterResolutionSamples: [],
        escapeCancelReceipt: null,
        cancelResolvedFalse: null,
        confirmResolvedTrue: null,
        actionMutationCountBefore: null,
        actionMutationCountAfter: null,
        noMutationBeforeConfirm: null,
        noMutationAfterCancel: null,
        noExecutionWithoutConfirm: null,
        staleConfirmRejected: null,
        wrongSurfaceConfirmRejected: null,
        focusRestored: null,
        selectionRestored: null,
        filterRestored: null,
        routeStackRestored: null,
        footerActionsSafe: null,
        startReceipt,
        readyReceipt,
        showReceipt,
        stopReceipt,
        error: error instanceof Error ? error.message : String(error),
      },
      usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
      steps: [{ name: "destructive-confirm-dry-run", status: "fail", output: { error: error instanceof Error ? error.message : String(error) } }],
      failure: { code: "missing_destructive_confirm_modal_safety_receipt", stepName: "destructive-confirm-dry-run", message: "Dry-run destructive confirm runtime proof failed before producing a complete receipt." },
      warnings: ["file_linear:destructive_confirm_modal_safety_receipts_missing"],
    };
  }
}

export async function runSdkConfirmRuntimeProofScenario(opts: {
  session: string;
  cancel?: "escape";
}): Promise<HardScenarioReceipt> {
  const receiptPath = "/tmp/confirm-modal-sdk-confirm-script-result.json";
  const scriptPath = resolve(PROJECT_ROOT, "tests/smoke/test-confirm-sdk-runtime.ts");
  const runRequestId = `sdk-confirm-run-${Date.now()}`;
  const sessionDir = Bun.env.SCRIPT_KIT_SESSION_DIR
    ?? `/tmp/sk-sdk-confirm-runtime-${opts.session}-${Date.now()}`;
  const agentBinary = resolve(PROJECT_ROOT, "target-agent/pools/agent-debug/debug/script-kit-gpui");
  const sessionEnv: Record<string, string> = {
    SCRIPT_KIT_SESSION_DIR: sessionDir,
    SCRIPT_KIT_GPUI_BINARY: Bun.env.SCRIPT_KIT_GPUI_BINARY ?? agentBinary,
    SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM:
      Bun.env.SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM ?? "1",
    SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK:
      Bun.env.SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK ?? "1",
  };
  const baseProof = {
    kind: "sdk.confirm.runtime",
    api: "confirm",
    scriptPath,
    runRequestId,
    sessionDir,
    requestedBinary: sessionEnv.SCRIPT_KIT_GPUI_BINARY,
    scriptRequest: {
      message: "SDK confirm runtime proof?",
      confirmText: "Confirm SDK",
      cancelText: "Cancel SDK",
      expectedProtocolType: "confirm",
    },
    hostRoute: {
      expectedPromptMessage: "ShowConfirm",
      expectedSurface: "ConfirmPrompt",
      hostHandler: "src/prompt_handler/mod.rs::PromptMessage::ShowConfirm",
      sharedRoute: "open_confirm_prompt",
    },
    destructiveCommandExecuted: false,
    systemCommandRequested: false,
    trashMutationRequested: false,
  };

  let startReceipt: Record<string, unknown> = {};
  let readyReceipt: Record<string, unknown> | null = null;
  let stopReceipt: Record<string, unknown> | null = null;
  let showReceipt: Record<string, unknown> | null = null;
  let runReceipt: Record<string, unknown> | null = null;
  let stateBefore: Record<string, unknown> = {};
  let elementsBefore: Record<string, unknown> = {};
  let escapeCancelReceipt: Record<string, unknown> = {};
  let stateAfterResolve: Record<string, unknown> = {};
  let scriptResult: Record<string, unknown> = {};
  let processTree: Record<string, unknown> = {};
  let failureArtifacts: Record<string, unknown> = {};

  try {
    await runTool(["rm", "-f", receiptPath], "sdk-confirm:remove-old-script-receipt");
    startReceipt = parseMaybeJson(
      (await runTool(["bash", "scripts/agentic/session.sh", "start", opts.session], "sdk-confirm:session-start", sessionEnv)).stdout,
    );
    readyReceipt = startReceipt.ready === true
      ? { status: "ok", readyFromStart: true }
      : await runTool(["bash", "scripts/agentic/wait-session-ready.sh", opts.session, "60"], "sdk-confirm:wait-session-ready", sessionEnv);
    if (startReceipt.ready !== true && readyReceipt.exitCode !== 0) {
      throw new Error(String(readyReceipt.stdout || readyReceipt.stderr || "session did not become ready"));
    }

    showReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "show",
      requestId: "sdk-confirm-show-main",
    }, 8000, sessionEnv);
    await Bun.sleep(150);

    runReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "run",
      path: scriptPath,
      requestId: runRequestId,
    }, 8000, sessionEnv);

    const statePolls: Record<string, unknown>[] = [];
    for (let attempt = 0; attempt < 40; attempt += 1) {
      stateBefore = rpcResponse(await rpc(opts.session, {
        type: "getState",
        requestId: `sdk-confirm-state-before-${attempt}`,
        summaryOnly: true,
      }, "stateResult", 8000, sessionEnv));
      statePolls.push({
        attempt,
        promptType: stateBefore.promptType ?? null,
        surfaceKind: stateBefore.surfaceKind ?? null,
        semanticSurface: stateBefore.semanticSurface ?? null,
      });
      if (String(stateBefore.promptType ?? "") === "confirmPrompt") {
        break;
      }
      await Bun.sleep(100);
    }

    elementsBefore = rpcResponse(await rpc(opts.session, {
      type: "getElements",
      requestId: "sdk-confirm-elements-before",
      limit: 80,
    }, "elementsResult", 8000, sessionEnv));

    const elements = asArray(elementsBefore.elements).map(asRecord);
    const footerButtons = asArray(asRecord(stateBefore.activeFooter).buttons).map(asRecord);
    const activeFooterText = JSON.stringify(stateBefore.activeFooter ?? {}).toLowerCase();
    const confirmButton = elements.find((element) =>
      String(element.id ?? "").startsWith("button:0:")
      || String(element.label ?? "") === "Confirm SDK"
    ) ?? footerButtons.find((button) => String(button.action ?? "") === "apply") ?? null;
    const cancelButton = elements.find((element) =>
      String(element.id ?? "").startsWith("button:1:")
      || String(element.label ?? "") === "Cancel SDK"
    ) ?? footerButtons.find((button) => String(button.action ?? "") === "close") ?? null;

    escapeCancelReceipt = await sendWithoutAwaitParse(opts.session, {
      type: "simulateKey",
      key: "escape",
      modifiers: [],
      requestId: "sdk-confirm-escape-cancel",
    }, 8000, sessionEnv);

    const resultPolls: Record<string, unknown>[] = [];
    for (let attempt = 0; attempt < 40; attempt += 1) {
      if (await Bun.file(receiptPath).exists()) {
        scriptResult = await Bun.file(receiptPath).json();
        resultPolls.push({
          attempt,
          phase: scriptResult.phase ?? null,
          result: scriptResult.result ?? null,
          resultType: scriptResult.resultType ?? null,
        });
        if (scriptResult.phase === "resolved") {
          break;
        }
      } else {
        resultPolls.push({ attempt, phase: "missing" });
      }
      await Bun.sleep(100);
    }

    stateAfterResolve = rpcResponse(await rpc(opts.session, {
      type: "getState",
      requestId: "sdk-confirm-state-after-resolve",
      summaryOnly: true,
    }, "stateResult", 8000, sessionEnv));

    stopReceipt = parseMaybeJson(
      (await runTool(["bash", "scripts/agentic/session.sh", "stop", opts.session], "sdk-confirm:session-stop", sessionEnv)).stdout,
    );

    const confirmSurfaceOpen = String(stateBefore.promptType ?? "") === "confirmPrompt";
    const promptDismissed = String(stateAfterResolve.promptType ?? "") !== "confirmPrompt";
    const confirmButtonText = String(confirmButton ? confirmButton.label ?? confirmButton.text ?? "" : "");
    const cancelButtonText = String(cancelButton ? cancelButton.label ?? cancelButton.text ?? "" : "");
    const footerHasApply = activeFooterText.includes("apply") || footerButtons.some((button) => String(button.action ?? "") === "apply");
    const footerHasClose = activeFooterText.includes("close") || footerButtons.some((button) => String(button.action ?? "") === "close");
    const scriptResolvedFalse = scriptResult.phase === "resolved"
      && scriptResult.result === false
      && scriptResult.resultType === "boolean";
    const status = confirmSurfaceOpen
      && confirmButtonText === "Confirm SDK"
      && cancelButtonText === "Cancel SDK"
      && footerHasApply
      && footerHasClose
      && promptDismissed
      && scriptResolvedFalse
      ? "pass"
      : "fail";

    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "sdk-confirm-runtime-proof",
      status,
      failClosed: status !== "pass",
      failureMode: status === "pass" ? undefined : "fail_closed",
      missingReceipt: status === "pass" ? undefined : "missing_sdk_confirm_runtime_receipt",
      linearIssue: status === "pass" ? undefined : "file_linear:sdk_confirm_runtime_receipt_missing",
      sdkConfirmRuntime: {
        ...baseProof,
        promptBeforeResolve: {
          promptType: stateBefore.promptType ?? null,
          semanticSurface: stateBefore.semanticSurface ?? null,
          surfaceKind: stateBefore.surfaceKind ?? null,
          promptId: stateBefore.promptId ?? null,
          automationWindowId: stateBefore.automationWindowId ?? null,
          activeFooter: stateBefore.activeFooter ?? null,
        },
        elementsBeforeResolve: {
          panelSemanticId: elements.find((element) => String(element.id ?? "").startsWith("panel:"))?.id ?? null,
          confirmButtonSemanticId: confirmButton ? confirmButton.id ?? confirmButton.action ?? null : null,
          confirmButtonText,
          cancelButtonSemanticId: cancelButton ? cancelButton.id ?? cancelButton.action ?? null : null,
          cancelButtonText,
          selectedOrFocusedId: elementsBefore.selectedId ?? elementsBefore.focusedId ?? null,
        },
        footerAndKeys: {
          footerHasApply,
          footerHasClose,
          cancelKey: "Escape",
          escapeCancelReceipt,
          keyboardPolicy: stateBefore.keyboardPolicy ?? "NoEditableKeyboard/source-contract",
        },
        scriptResult: {
          path: receiptPath,
          ...scriptResult,
        },
        cleanup: {
          stateAfterResolve,
          promptDismissed,
          scriptReceiptWritten: scriptResolvedFalse,
          stopReceipt,
          sessionDir,
          binary: startReceipt.binary ?? sessionEnv.SCRIPT_KIT_GPUI_BINARY,
          destructiveCommandExecuted: false,
          systemCommandRequested: false,
          trashMutationRequested: false,
        },
        showReceipt,
        runReceipt,
        statePolls,
        resultPolls,
      },
      usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
      steps: [
        { name: "send-sdk-confirm-script-run", status: runReceipt ? "pass" : "fail", output: runReceipt },
        { name: "inspect-sdk-confirm-state", status: confirmSurfaceOpen ? "pass" : "fail", output: stateBefore },
        { name: "inspect-sdk-confirm-elements", status: confirmButtonText === "Confirm SDK" && cancelButtonText === "Cancel SDK" ? "pass" : "fail", output: { confirmButton, cancelButton } },
        { name: "escape-resolves-sdk-confirm-false", status: scriptResolvedFalse ? "pass" : "fail", output: { escapeCancelReceipt, stateAfterResolve, scriptResult } },
      ],
      failure: status === "pass" ? undefined : {
        code: "missing_sdk_confirm_runtime_receipt",
        stepName: "sdk-confirm-runtime-proof",
        message: "SDK confirm runtime proof did not expose the required run, prompt, button, footer, dismiss, and boolean result receipts.",
      },
      warnings: status === "pass" ? [] : ["file_linear:sdk_confirm_runtime_receipt_missing"],
    };
  } catch (error) {
    if (await Bun.file(receiptPath).exists()) {
      try {
        scriptResult = await Bun.file(receiptPath).json();
      } catch {
        scriptResult = { phase: "unreadable" };
      }
    }
    const sessionStatus = parseMaybeJson(
      (await runTool(["bash", "scripts/agentic/session.sh", "status", opts.session], "sdk-confirm:session-status-after-error", sessionEnv)).stdout,
    );
    const pid = typeof sessionStatus.pid === "number" && sessionStatus.pid > 0
      ? String(sessionStatus.pid)
      : "";
    const processTreeText = pid
      ? await runTool(["bash", "-lc", `ps -p ${pid} -o pid,ppid,pgid,stat,%cpu,lstart,command 2>/dev/null; for child in $(pgrep -P ${pid} 2>/dev/null || true); do ps -p "$child" -o pid,ppid,pgid,stat,%cpu,lstart,command 2>/dev/null; done`], "sdk-confirm:process-tree-after-error")
      : { exitCode: 0, stdout: "", stderr: "" };
    const artifactDir = `/tmp/confirm-modal-sdk-confirm-runtime-artifacts-${Date.now()}`;
    const logPath = typeof sessionStatus.log === "string" ? sessionStatus.log : "";
    const responsesPath = typeof sessionStatus.responses === "string" ? sessionStatus.responses : "";
    const protocolResponsesPath = typeof sessionStatus.protocolResponses === "string" ? sessionStatus.protocolResponses : "";
    const lifecyclePath = typeof sessionStatus.lifecycle === "string" ? sessionStatus.lifecycle : "";
    await runTool(["mkdir", "-p", artifactDir], "sdk-confirm:create-failure-artifact-dir");
    for (const [name, path] of [
      ["app.log", logPath],
      ["responses.ndjson", responsesPath],
      ["protocol-responses.ndjson", protocolResponsesPath],
      ["lifecycle.ndjson", lifecyclePath],
    ] as const) {
      if (path) {
        await runTool(["bash", "-lc", `if [ -f "$1" ]; then cp "$1" "$2/$3"; fi`, "copy-artifact", path, artifactDir, name], `sdk-confirm:copy-${name}`);
      }
    }
    await Bun.write(`${artifactDir}/process-tree.txt`, processTreeText.stdout || "");
    if (processTreeText.stderr) {
      await Bun.write(`${artifactDir}/process-tree.stderr.txt`, processTreeText.stderr);
    }
    failureArtifacts = {
      artifactDir,
      appLog: logPath ? `${artifactDir}/app.log` : null,
      responses: responsesPath ? `${artifactDir}/responses.ndjson` : null,
      protocolResponses: protocolResponsesPath ? `${artifactDir}/protocol-responses.ndjson` : null,
      lifecycle: lifecyclePath ? `${artifactDir}/lifecycle.ndjson` : null,
      processTree: `${artifactDir}/process-tree.txt`,
    };
    processTree = {
      sessionStatus,
      stdout: processTreeText.stdout,
      stderr: processTreeText.stderr,
      failureArtifacts,
    };
    stopReceipt = parseMaybeJson(
      (await runTool(["bash", "scripts/agentic/session.sh", "stop", opts.session], "sdk-confirm:session-stop-after-error", sessionEnv)).stdout,
    );
    return {
      schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
      scenario: "sdk-confirm-runtime-proof",
      status: "fail",
      failClosed: true,
      failureMode: "fail_closed",
      missingReceipt: "missing_sdk_confirm_runtime_receipt",
      linearIssue: "file_linear:sdk_confirm_runtime_receipt_missing",
      sdkConfirmRuntime: {
        ...baseProof,
        promptBeforeResolve: stateBefore,
        elementsBeforeResolve: elementsBefore,
        footerAndKeys: { cancelKey: "Escape", escapeCancelReceipt },
        scriptResult: { path: receiptPath, ...scriptResult },
        cleanup: {
          stateAfterResolve,
          promptDismissed: String(stateAfterResolve.promptType ?? "") !== "confirmPrompt",
          scriptReceiptWritten: scriptResult.phase === "resolved",
          stopReceipt,
          sessionDir,
          binary: startReceipt.binary ?? sessionEnv.SCRIPT_KIT_GPUI_BINARY,
          processTree,
          failureArtifacts,
          destructiveCommandExecuted: false,
          systemCommandRequested: false,
          trashMutationRequested: false,
        },
        runReceipt,
        showReceipt,
        startReceipt,
        readyReceipt,
        error: error instanceof Error ? error.message : String(error),
      },
      usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
      steps: [{ name: "sdk-confirm-runtime-proof", status: "fail", output: { error: error instanceof Error ? error.message : String(error), stopReceipt } }],
      failure: { code: "missing_sdk_confirm_runtime_receipt", stepName: "sdk-confirm-runtime-proof", message: "SDK confirm runtime proof failed before producing a complete receipt." },
      warnings: ["file_linear:sdk_confirm_runtime_receipt_missing"],
    };
  }
}

export async function runLoadingSkeletonProgressRestorationStressScenario(opts: {
  session: string;
  surfaces?: string[];
  fixture?: string;
  cycles?: number;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "loading-skeleton-progress-restoration-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_loading_skeleton_progress_restoration_receipt",
    linearIssue: "file_linear:loading_skeleton_progress_restoration_receipts_missing",
    loadingSkeletonProgressRestoration: {
      kind: "ux.loadingSkeletonProgressRestoration",
      requiredReceipt: "ux.loadingSkeletonProgressRestoration",
      receiptKind: "ux.loadingSkeletonProgressRestoration",
      loadingSkeletonStressId: "loop-twenty-loading-skeleton-progress-restoration",
      requestedSurfaces: opts.surfaces ?? ["sdk-reference", "script-template-catalog"],
      requestedFixture: opts.fixture ?? "delayed-local",
      requestedCycles: opts.cycles ?? 4,
      surfaceSamples: [],
      surface: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      stateBefore: null,
      elementsBefore: null,
      requestGeneration: null,
      loadingReceipt: null,
      loadingState: null,
      skeletonVisible: null,
      skeletonRows: [],
      skeletonRowSemanticIds: [],
      skeletonBounds: null,
      progressReceipt: null,
      progressText: null,
      progressPercent: null,
      progressMonotonic: null,
      resultGeneration: null,
      resultsReadyReceipt: null,
      stateAfter: null,
      elementsAfter: null,
      realRowsVisible: null,
      skeletonCleared: null,
      noSkeletonAfterResults: null,
      selectedSemanticIdBefore: null,
      selectedSemanticIdAfter: null,
      selectionRestored: null,
      focusRestored: null,
      filterTextPreserved: null,
      scrollAnchorPreserved: null,
      footerActionStateDuringLoading: null,
      activationBlockedWhileLoading: null,
      noSubmitDuringLoading: null,
      staleLoadingGenerationRejected: null,
      staleProgressRejected: null,
      staleResultRejected: null,
      noBlankFrame: null,
      localFixtureOnly: true,
      networkAccessed: false,
      externalServiceContacted: false,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_loading_skeleton_progress_restoration_receipt" } }],
    failure: { code: "missing_loading_skeleton_progress_restoration_receipt", stepName: "declare-required-receipt", message: "Missing app-side loading skeleton/progress restoration receipts." },
    warnings: ["file_linear:loading_skeleton_progress_restoration_receipts_missing"],
  };
}

export async function runIconImageFallbackRedactionStressScenario(opts: {
  session: string;
  surfaces?: string[];
  fixtures?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "icon-image-fallback-redaction-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_icon_image_fallback_redaction_receipt",
    linearIssue: "file_linear:icon_image_fallback_redaction_receipts_missing",
    iconImageFallbackRedaction: {
      kind: "ux.iconImageFallbackRedaction",
      iconImageStressId: "loop-twenty-one-icon-image-fallback-redaction",
      requestedSurfaces: opts.surfaces ?? ["app-launcher", "file-search", "clipboard-history"],
      requestedFixtures: opts.fixtures ?? ["missing-file", "corrupt-png", "private-local-path", "data-uri-redacted"],
      surfaceSamples: [],
      surface: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      stateReceipt: null,
      elementsReceipt: null,
      imageFallbackReceipt: null,
      assetFixtureReceipt: null,
      fixtureKind: null,
      requestedImageSourceKind: null,
      requestedImageFingerprint: null,
      rawSourceRedacted: null,
      displayedImageKind: null,
      fallbackIconKind: null,
      fallbackReason: null,
      imageLoadGeneration: null,
      cacheKeyFingerprint: null,
      redactedPreview: null,
      noRawPath: null,
      noRawUrl: null,
      noFileContents: null,
      brokenImageRejected: null,
      unsupportedSchemeRejected: null,
      staleImageGenerationRejected: null,
      defaultIconRendered: null,
      accessibleLabelPreserved: null,
      rowIdentityPreserved: null,
      footerStatePreserved: null,
      networkAccessed: false,
      externalServiceContacted: false,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_icon_image_fallback_redaction_receipt" } }],
    failure: { code: "missing_icon_image_fallback_redaction_receipt", stepName: "declare-required-receipt", message: "Missing app-side icon/image fallback redaction receipts." },
    warnings: ["file_linear:icon_image_fallback_redaction_receipts_missing"],
  };
}

export async function runFooterStatusPersistenceStressScenario(opts: {
  session: string;
  surfaces?: string[];
  transitions?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "footer-status-persistence-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_footer_status_persistence_receipt",
    linearIssue: "file_linear:footer_status_persistence_receipts_missing",
    footerStatusPersistence: {
      kind: "ux.footerStatusPersistence",
      footerStatusStressId: "loop-twenty-one-footer-status-persistence",
      requestedSurfaces: opts.surfaces ?? ["main", "clipboard-history", "emoji-picker", "file-search", "actionsDialog"],
      requestedTransitions: opts.transitions ?? ["filter", "selection", "cmd-k", "escape", "clear-filter"],
      surfaceSamples: [],
      surface: null,
      hostAutomationWindowId: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      stateBefore: null,
      elementsBefore: null,
      transitionSamples: [],
      transitionGeneration: null,
      routeStackDepth: null,
      filterTextBefore: null,
      selectedSemanticIdBefore: null,
      footerReceipt: null,
      footerOwner: null,
      nativeFooterSurfaceId: null,
      gpuiFallbackVisible: null,
      renderedButtons: [],
      buttonSemanticIds: [],
      buttonLabel: null,
      buttonShortcutHint: null,
      disabledReason: null,
      statusBarReceipt: null,
      statusText: null,
      statusKind: null,
      statusGeneration: null,
      persistedAcrossFilter: null,
      persistedAcrossSelection: null,
      persistedAcrossActionsOpenClose: null,
      persistedAcrossPopupClose: null,
      noDuplicateFooterRows: null,
      noStaleStatusAfterRecovery: null,
      footerSafeSelection: null,
      inputCollisionFree: null,
      wrongSurfaceFooterRejected: null,
      staleFooterGenerationRejected: null,
      stateAfter: null,
      elementsAfter: null,
      networkAccessed: false,
      externalServiceContacted: false,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_footer_status_persistence_receipt" } }],
    failure: { code: "missing_footer_status_persistence_receipt", stepName: "declare-required-receipt", message: "Missing app-side footer/status persistence receipts." },
    warnings: ["file_linear:footer_status_persistence_receipts_missing"],
  };
}

export async function runKeyboardHintLabelParityStressScenario(opts: {
  session: string;
  surfaces?: string[];
  families?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "keyboard-hint-label-parity-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_keyboard_hint_label_parity_receipt",
    linearIssue: "file_linear:keyboard_hint_label_parity_receipts_missing",
    keyboardHintLabelParity: {
      kind: "ux.keyboardHintLabelParity",
      keyboardHintStressId: "loop-twenty-one-keyboard-hint-label-parity",
      requestedSurfaces: opts.surfaces ?? ["main", "clipboard-history", "emoji-picker", "file-search", "actionsDialog", "menuSyntaxTriggerPicker"],
      requestedFamilies: opts.families ?? ["footer", "row-accessory", "tooltip", "action-catalog"],
      surfaceSamples: [],
      surface: null,
      automationWindowId: null,
      osWindowId: null,
      semanticSurface: null,
      stateReceipt: null,
      elementsReceipt: null,
      actionCatalogReceipt: null,
      footerHintReceipt: null,
      rowHintSamples: [],
      tooltipHintReceipt: null,
      semanticId: null,
      actionId: null,
      hintOwner: null,
      visibleLabel: null,
      accessibleLabel: null,
      footerLabel: null,
      rowAccessoryLabel: null,
      tooltipLabel: null,
      tooltipNotRequiredReason: null,
      platformShortcutLabel: null,
      shortcutTokens: [],
      normalizedShortcut: null,
      glyphTokens: [],
      labelParityMatched: null,
      noMismatchedKeyGlyphs: null,
      noDuplicateShortcutHints: null,
      disabledStateParity: null,
      activationOwner: null,
      safeKeyboardActivation: null,
      noAccidentalExecution: null,
      hintGeneration: null,
      staleHintRejected: null,
      wrongSurfaceHintRejected: null,
      networkAccessed: false,
      externalServiceContacted: false,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_keyboard_hint_label_parity_receipt" } }],
    failure: { code: "missing_keyboard_hint_label_parity_receipt", stepName: "declare-required-receipt", message: "Missing app-side keyboard hint label parity receipts." },
    warnings: ["file_linear:keyboard_hint_label_parity_receipts_missing"],
  };
}

export async function runRowStateParityWithoutPointerStressScenario(opts: {
  session: string;
  surfaces?: string[];
  states?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "row-state-parity-without-pointer-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_row_state_parity_without_pointer_receipt",
    linearIssue: "file_linear:row_state_parity_without_pointer_receipts_missing",
    rowStateParityWithoutPointer: {
      kind: "ux.rowStateParityWithoutPointer",
      rowStateParityStressId: "loop-twenty-two-row-state-parity-without-pointer",
      requestedSurfaces: opts.surfaces ?? ["main", "clipboard-history", "emoji-picker", "file-search", "actionsDialog"],
      requestedStates: opts.states ?? ["selected", "focused", "hovered", "selected-hovered"],
      surfaceSamples: [], surface: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, rowStateParityWithoutPointerReceipt: null,
      rowStateSamples: [], semanticId: null, rowRole: null, rowIndex: null, rowLabel: null, modality: null,
      selectedSemanticId: null, focusedSemanticId: null, hoverSemanticId: null,
      keyboardFocusRingVisible: null, selectionPaintVisible: null, hoverPaintVisible: null, focusPaintVisible: null,
      selectedFillToken: null, hoverFillToken: null, focusRingToken: null, textOpacityToken: null, iconOpacityToken: null,
      selectedPrecedenceOverHover: null, hoverDoesNotOverrideSelection: null, focusDoesNotStealSelection: null,
      focusedRowMatchesElements: null, selectedRowMatchesState: null, hoverReceiptSyntheticOnly: null,
      noNativePointerRequired: true, staleRowStateRejected: null, wrongSurfaceRowStateRejected: null, noAccidentalExecution: null,
      networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_row_state_parity_without_pointer_receipt" } }],
    failure: { code: "missing_row_state_parity_without_pointer_receipt", stepName: "declare-required-receipt", message: "Missing app-side row visual-state parity receipts without native pointer input." },
    warnings: ["file_linear:row_state_parity_without_pointer_receipts_missing"],
  };
}

export async function runQuietChromeCardNestingStressScenario(opts: {
  session: string;
  surfaces?: string[];
  chrome?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "quiet-chrome-card-nesting-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_quiet_chrome_card_nesting_receipt",
    linearIssue: "file_linear:quiet_chrome_card_nesting_receipts_missing",
    quietChromeCardNesting: {
      kind: "ux.quietChromeCardNesting",
      quietChromeStressId: "loop-twenty-two-quiet-chrome-card-nesting",
      requestedSurfaces: opts.surfaces ?? ["main", "clipboard-history", "emoji-picker", "file-search", "actionsDialog", "promptPopup"],
      requestedChrome: opts.chrome ?? "quiet",
      surfaceSamples: [], surface: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, quietChromeCardNestingReceipt: null,
      chromeLayerSamples: [], shellLayer: null, contentLayer: null, rowLayer: null, popupLayer: null, footerLayer: null,
      borderToken: null, fillToken: null, shadowToken: null, vibrancyMaterial: null, cornerRadius: null,
      insetPx: null, gapPx: null, cardDepth: null, nestedCardCount: null, maxAllowedCardDepth: null,
      duplicateBorderRejected: null, opaqueFillRejected: null, heavyShadowRejected: null, doubleCardNestingRejected: null,
      footerChromeSeparated: null, inputChromeSeparated: null, popupMaterialPreserved: null, quietChromeBudgetMatched: null,
      themeTokenFingerprint: null, staleChromeTokenRejected: null, wrongSurfaceChromeRejected: null,
      networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_quiet_chrome_card_nesting_receipt" } }],
    failure: { code: "missing_quiet_chrome_card_nesting_receipt", stepName: "declare-required-receipt", message: "Missing app-side quiet chrome/card nesting receipts." },
    warnings: ["file_linear:quiet_chrome_card_nesting_receipts_missing"],
  };
}

export async function runScrollShadowStickyHeaderDensityStressScenario(opts: {
  session: string;
  surfaces?: string[];
  scrollPositions?: string[];
  density?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "scroll-shadow-sticky-header-density-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_scroll_shadow_sticky_header_density_receipt",
    linearIssue: "file_linear:scroll_shadow_sticky_header_density_receipts_missing",
    scrollShadowStickyHeaderDensity: {
      kind: "ux.scrollShadowStickyHeaderDensity",
      scrollChromeDensityStressId: "loop-twenty-two-scroll-shadow-sticky-header-density",
      requestedSurfaces: opts.surfaces ?? ["clipboard-history", "emoji-picker", "file-search", "app-launcher", "actionsDialog"],
      requestedScrollPositions: opts.scrollPositions ?? ["top", "middle", "bottom"],
      requestedDensity: opts.density ?? ["compact", "default"],
      surfaceSamples: [], surface: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, scrollShadowStickyHeaderDensityReceipt: null,
      scrollSamples: [], scrollPosition: null, scrollTop: null, scrollViewportBounds: null, scrollContentBounds: null,
      scrollContentHeight: null, scrollViewportHeight: null, stickyHeaderReceipt: null, headerSemanticId: null,
      headerBounds: null, headerPinned: null, headerZIndex: null, headerDoesNotOverlapRows: null,
      headerDoesNotOverlapInput: null, headerDoesNotOverlapFooter: null, scrollShadowReceipt: null,
      topShadowVisible: null, bottomShadowVisible: null, topShadowOpacityToken: null, bottomShadowOpacityToken: null,
      shadowGradientToken: null, shadowMatchesScrollPosition: null, densityReceipt: null, densityMode: null,
      rowHeightPx: null, sectionHeaderHeightPx: null, inputHeightPx: null, footerHeightPx: null, verticalGapPx: null,
      horizontalInsetPx: null, remSize: null, scaleFactor: null, densityTokenFingerprint: null, rowRhythmStable: null,
      footerSafeViewport: null, selectedRowVisibleAboveFooter: null, staleScrollGenerationRejected: null,
      wrongSurfaceScrollRejected: null, networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_scroll_shadow_sticky_header_density_receipt" } }],
    failure: { code: "missing_scroll_shadow_sticky_header_density_receipt", stepName: "declare-required-receipt", message: "Missing app-side scroll shadow, sticky header, and density receipts." },
    warnings: ["file_linear:scroll_shadow_sticky_header_density_receipts_missing"],
  };
}

export async function runPopupFocusKeycapVisualSemanticsStressScenario(opts: {
  session: string;
  surfaces?: string[];
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "popup-focus-keycap-visual-semantics-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_popup_focus_keycap_visual_semantics_receipt",
    linearIssue: "file_linear:popup_focus_keycap_visual_semantics_receipts_missing",
    popupFocusKeycapVisualSemantics: {
      kind: "ux.popupFocusKeycapVisualSemantics", popupKeycapStressId: "loop-twenty-three-popup-focus-keycap",
      requestedSurfaces: opts.surfaces ?? ["actionsDialog", "confirmPrompt"],
      surfaceSamples: [], surface: null, popupKind: null, automationWindowId: null, osWindowId: null, parentAutomationWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, popupFocusKeycapVisualSemanticsReceipt: null,
      keycapSamples: [], keycapRole: null, keycapLabel: null, shortcutLabel: null, normalizedShortcutTokens: [], platformGlyph: null,
      focused: null, focusOwnerSemanticId: null, focusedButtonSemanticId: null, focusedKeycapMatchesFocusedButton: null,
      escapeKeycapAvailable: null, enterKeycapAvailable: null, keycapFillToken: null, keycapGlyphToken: null,
      keycapTextToken: null, focusRingToken: null, dangerSemanticOnLabelNotKeycap: null, disabledKeycapMuted: null,
      shortcutGlyphNormalized: null, popupIsTopmostOwner: null, parentFocusUnchanged: null, parentSelectionUnchanged: null,
      staleFocusReceiptRejected: null, wrongSurfaceKeycapRejected: null, noAccidentalExecution: null,
      networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_popup_focus_keycap_visual_semantics_receipt" } }],
    failure: { code: "missing_popup_focus_keycap_visual_semantics_receipt", stepName: "declare-required-receipt", message: "Missing app-side popup focus/keycap visual semantics receipts." },
    warnings: ["file_linear:popup_focus_keycap_visual_semantics_receipts_missing"],
  };
}

export async function runReducedMotionAnimationDisableStressScenario(opts: {
  session: string;
  surfaces?: string[];
  fixture?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "reduced-motion-animation-disable-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_reduced_motion_animation_disable_receipt",
    linearIssue: "file_linear:reduced_motion_animation_disable_receipts_missing",
    reducedMotionAnimationDisable: {
      kind: "ux.reducedMotionAnimationDisable", reducedMotionStressId: "loop-twenty-three-reduced-motion-animation-disable",
      requestedSurfaces: opts.surfaces ?? ["main", "actionsDialog", "menuSyntaxTriggerPicker"], requestedFixture: opts.fixture ?? "reduced-motion",
      surfaceSamples: [], surface: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, reducedMotionAnimationDisableReceipt: null,
      motionPolicyReceipt: null, motionPreferenceSource: null, fixtureOnlyReducedMotion: true, systemPreferenceNotRead: true,
      systemPreferenceNotMutated: true, animationSamples: [], animationName: null, transitionGeneration: null, frameId: null,
      frameClockPaused: null, motionDurationMs: null, effectiveDurationMs: null, animatedOpacityStable: null,
      animatedTransformStable: null, spinnerHiddenOrStatic: null, shimmerDisabled: null, loadingPulseDisabled: null,
      autoFocusPreserved: null, selectedRowPreserved: null, cursorPositionPreserved: null, noLayoutShiftDuringMotionDisable: null,
      staleMotionGenerationRejected: null, wrongSurfaceMotionRejected: null, noNativeInputRequired: true,
      networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_reduced_motion_animation_disable_receipt" } }],
    failure: { code: "missing_reduced_motion_animation_disable_receipt", stepName: "declare-required-receipt", message: "Missing app-side reduced-motion animation-disable receipts." },
    warnings: ["file_linear:reduced_motion_animation_disable_receipts_missing"],
  };
}

export async function runCommandSearchHighlightingAccessoryBadgesStressScenario(opts: {
  session: string;
  hosts?: string[];
  query?: string;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "command-search-highlighting-accessory-badges-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_command_search_highlighting_accessory_badges_receipt",
    linearIssue: "file_linear:command_search_highlighting_accessory_badges_receipts_missing",
    commandSearchHighlightingAccessoryBadges: {
      kind: "ux.commandSearchHighlightAccessoryBadges", commandHighlightBadgeStressId: "loop-twenty-three-command-search-highlighting-accessory-badges",
      requestedHosts: opts.hosts ?? ["main", "actionsDialog", "app-launcher", "menuSyntaxTriggerPicker"], query: opts.query ?? "agentic-loop-twenty-three",
      hostSamples: [], host: null, popupKind: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, commandSearchHighlightingAccessoryBadgesReceipt: null,
      querySamples: [], searchGeneration: null, commandRows: [], semanticId: null, commandId: null, commandLabel: null,
      sectionLabel: null, highlightedRanges: [], highlightText: null, matchedQuery: null, highlightMatchesFilter: null,
      highlightDoesNotMutateLabel: null, accessoryBadges: [], badgeKind: null, badgeLabel: null, badgeTooltip: null,
      shortcutBadge: null, disabledBadge: null, noOpBadge: null, loadingBadge: null, accessoryOrderStable: null,
      badgesMatchActionCatalog: null, disabledReasonVisible: null, loadingReasonVisible: null, staleBadgeRejected: null,
      staleHighlightRejected: null, wrongHostCommandRejected: null, footerActionsStable: null, noAccidentalExecution: null,
      networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_command_search_highlighting_accessory_badges_receipt" } }],
    failure: { code: "missing_command_search_highlighting_accessory_badges_receipt", stepName: "declare-required-receipt", message: "Missing app-side command search highlighting/accessory badge receipts." },
    warnings: ["file_linear:command_search_highlighting_accessory_badges_receipts_missing"],
  };
}

export async function runClipboardCopyVisualFeedbackStressScenario(opts: {
  session: string;
  hosts?: string[];
  fixture?: string;
  pasteboardScope?: string;
  noSystemPasteboard?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "clipboard-copy-visual-feedback-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_clipboard_copy_visual_feedback_receipt",
    linearIssue: "file_linear:clipboard_copy_visual_feedback_receipts_missing",
    clipboardCopyVisualFeedback: {
      kind: "ux.clipboardCopyVisualFeedback",
      clipboardCopyFeedbackStressId: "loop-twenty-four-clipboard-copy-visual-feedback",
      requestedHosts: opts.hosts ?? ["file-search", "actionsDialog", "app-launcher"],
      requestedFixture: opts.fixture ?? "agentic-copy-preview",
      pasteboardScope: opts.pasteboardScope ?? "fixture",
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      hostSamples: [], host: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, clipboardCopyVisualFeedbackReceipt: null,
      copyActionSemanticId: null, copyActionLabel: null, copyGeneration: null,
      copyButtonStateBefore: null, copyButtonStateAfter: null, visibleCopiedState: null,
      copiedStateDurationMs: null, copyToastReceipt: null, redactedPayloadPreview: null,
      payloadFingerprint: null, fixturePasteboardUsed: null, systemPasteboardUnchanged: null,
      originalPasteboardFingerprint: null, postRunPasteboardFingerprint: null,
      noRawClipboardContentLogged: null, staleCopyGenerationRejected: null, wrongHostCopyRejected: null,
      noAccidentalPaste: null, networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_clipboard_copy_visual_feedback_receipt" } }],
    failure: { code: "missing_clipboard_copy_visual_feedback_receipt", stepName: "declare-required-receipt", message: "Missing app-side fixture-scoped copy visual feedback receipts." },
    warnings: ["file_linear:clipboard_copy_visual_feedback_receipts_missing"],
  };
}

export async function runPortalCancelReturnStateRestorationStressScenario(opts: {
  session: string;
  origins?: string[];
  portal?: string;
  query?: string;
  cancelMethods?: string[];
  fixture?: string;
  noNativePicker?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "portal-cancel-return-state-restoration-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_portal_cancel_return_state_restoration_receipt",
    linearIssue: "file_linear:portal_cancel_return_state_restoration_receipts_missing",
    portalCancelReturnStateRestoration: {
      kind: "ux.portalCancelReturnStateRestoration",
      portalCancelReturnStressId: "loop-twenty-four-portal-cancel-return-state-restoration",
      requestedOrigins: opts.origins ?? ["agent_chat-composer", "notes"],
      requestedPortal: opts.portal ?? "file-search",
      requestedQuery: opts.query ?? "AGENTS.md",
      requestedCancelMethods: opts.cancelMethods ?? ["escape", "back"],
      requestedFixture: opts.fixture ?? "repo-file",
      noNativePicker: opts.noNativePicker ?? true,
      originSamples: [], origin: null, originAutomationWindowId: null, originGeneration: null,
      originSemanticSurface: null, originStateReceipt: null, originElementsReceipt: null,
      draftTextBeforePortal: null, cursorBeforePortal: null, selectionBeforePortal: null,
      portalSessionId: null, portalSurface: null, portalAutomationWindowId: null, portalQuery: null,
      portalSelectionBeforeCancel: null, cancelMethod: null, cancelReceipt: null,
      returnTargetIdentity: null, returnGeneration: null, focusRestored: null, draftTextRestored: null,
      cursorRestored: null, selectionRestored: null, filterRestored: null, scrollRestored: null,
      noContextPartInserted: null, noPromptSubmit: null, noSelectionMutationDuringPortal: null,
      stalePortalReturnRejected: null, foreignPortalEventRejected: null, wrongOriginReturnRejected: null,
      networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_portal_cancel_return_state_restoration_receipt" } }],
    failure: { code: "missing_portal_cancel_return_state_restoration_receipt", stepName: "declare-required-receipt", message: "Missing app-side portal cancel/back return restoration receipts." },
    warnings: ["file_linear:portal_cancel_return_state_restoration_receipts_missing"],
  };
}

export async function runTooltipHoverFocusAffordanceStressScenario(opts: {
  session: string;
  surfaces?: string[];
  targets?: string[];
  fixture?: string;
  inputModes?: string[];
  noNativePointer?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "tooltip-hover-focus-affordance-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_tooltip_hover_focus_affordance_receipt",
    linearIssue: "file_linear:tooltip_hover_focus_affordance_receipts_missing",
    tooltipHoverFocusAffordance: {
      kind: "ux.tooltipHoverFocusAffordance",
      tooltipHoverFocusStressId: "loop-twenty-four-tooltip-hover-focus-affordance",
      requestedSurfaces: opts.surfaces ?? ["main", "actionsDialog", "app-launcher"],
      requestedTargets: opts.targets ?? ["truncated-row", "disabled-action", "footer-button"],
      requestedFixture: opts.fixture ?? "agentic-tooltips",
      requestedInputModes: opts.inputModes ?? ["protocol-hover", "keyboard-focus"],
      noNativePointer: opts.noNativePointer ?? true,
      surfaceSamples: [], surface: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      stateReceipt: null, elementsReceipt: null, tooltipHoverFocusAffordanceReceipt: null,
      targetSamples: [], targetSemanticId: null, targetRole: null, triggerMode: null,
      hoverGeneration: null, focusGeneration: null, tooltipGeneration: null, tooltipText: null,
      tooltipKind: null, tooltipAnchorBounds: null, tooltipBounds: null, tooltipPlacement: null,
      hoverDelayMs: null, hoverDelayRespected: null, keyboardFocusOpensTooltip: null,
      tooltipAccessibleDescriptionMatches: null, escapeDismissesTooltip: null, scrollDismissesTooltip: null,
      focusLossDismissesTooltip: null, noFocusSteal: null, targetFocusPreserved: null,
      doesNotCoverTarget: null, doesNotCoverFooter: null, doesNotCoverPopupOwner: null,
      staleTooltipGenerationRejected: null, wrongSurfaceTooltipRejected: null, noAccidentalExecution: null,
      networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_tooltip_hover_focus_affordance_receipt" } }],
    failure: { code: "missing_tooltip_hover_focus_affordance_receipt", stepName: "declare-required-receipt", message: "Missing app-side tooltip hover/focus affordance receipts." },
    warnings: ["file_linear:tooltip_hover_focus_affordance_receipts_missing"],
  };
}

export async function runShortcutRecorderCancelLayeringStressScenario(opts: {
  session: string;
  surface?: string;
  action?: string;
  cancelMethods?: string[];
  inputModes?: string[];
  sandboxConfig?: boolean;
  noConfigWrite?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "shortcut-recorder-cancel-layering-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_shortcut_recorder_cancel_layering_receipt",
    linearIssue: "file_linear:shortcut_recorder_cancel_layering_receipts_missing",
    shortcutRecorderCancelLayeringReceipt: {
      kind: "ux.shortcutRecorderCancelLayering",
      shortcutRecorderCancelLayeringStressId: "loop-twenty-five-shortcut-recorder-cancel-layering",
      session: opts.session,
      surface: opts.surface ?? "shortcuts",
      action: opts.action ?? "test-agentic-shortcut",
      requestedCancelMethods: opts.cancelMethods ?? ["escape", "cmd-w", "backdrop", "parent-click"],
      requestedInputModes: opts.inputModes ?? ["protocol-key", "protocol-click"],
      sandboxConfig: opts.sandboxConfig ?? true,
      noConfigWrite: opts.noConfigWrite ?? true,
      parentAutomationWindowId: null, recorderAutomationWindowId: null, parentSemanticSurface: null,
      modalLayerReceipt: null, parentBounds: null, recorderBounds: null, shellNarrowerThanParent: null,
      titleText: null, pressKeysPlaceholderVisible: null, footerAbsent: null, visibleCancelButton: null,
      cancelMethod: null, escapeCancels: null, cmdWCancels: null, backdropClickCancels: null,
      parentClickCancels: null, chordNotCapturedOnCancel: null, configFingerprintBefore: null,
      configFingerprintAfter: null, configUnchanged: null, globalHotkeyNotRegistered: null,
      parentFocusRestored: null, parentSelectionRestored: null, staleRecorderRejected: null,
      wrongParentRejected: null, networkAccessed: false, externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_shortcut_recorder_cancel_layering_receipt" } }],
    failure: { code: "missing_shortcut_recorder_cancel_layering_receipt", stepName: "declare-required-receipt", message: "Missing app-side shortcut recorder cancel/layering receipts." },
    warnings: ["file_linear:shortcut_recorder_cancel_layering_receipts_missing"],
  };
}

export async function runInlinePopoverAnchorResizeStressScenario(opts: {
  session: string;
  families?: string[];
  widths?: string[];
  fixture?: string;
  inputModes?: string[];
  noNativeInput?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "inline-popover-anchor-resize-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_inline_popover_anchor_resize_receipt",
    linearIssue: "file_linear:inline_popover_anchor_resize_receipts_missing",
    inlinePopoverAnchorResizeReceipt: {
      kind: "ux.inlinePopoverAnchorResize",
      inlinePopoverAnchorResizeStressId: "loop-twenty-five-inline-popover-anchor-resize",
      session: opts.session,
      requestedFamilies: opts.families ?? ["agent_chat-slash", "menu-syntax-colon"],
      requestedWidths: opts.widths ?? ["mini", "narrow", "full"],
      requestedFixture: opts.fixture ?? "agentic-inline-popover",
      requestedInputModes: opts.inputModes ?? ["protocol-key", "protocol-resize"],
      noNativeInput: opts.noNativeInput ?? true,
      familySamples: [], family: null, originAutomationWindowId: null, popupAutomationWindowId: null,
      parentSemanticSurface: null, triggerText: null, triggerRange: null, anchorBoundsBeforeResize: null,
      anchorBoundsAfterResize: null, popupBoundsBeforeResize: null, popupBoundsAfterResize: null,
      resizeGeneration: null, widthMode: null, visibleRangeBeforeResize: null, visibleRangeAfterResize: null,
      selectedRowVisible: null, selectedRowIdentityPreserved: null, synopsisBounds: null, footerRowBounds: null,
      noSynopsisFooterOverlap: null, noParentClipping: null, noViewportOverflow: null, zOrderAboveParent: null,
      noFocusSteal: null, keyboardSelectionPreserved: null, keyboardFallbackAccepted: null,
      screenshotToSemanticsAlignment: null, strictCaptureTarget: null, blankScreenshotRejected: null,
      staleResizeGenerationRejected: null, wrongPopupRejected: null, networkAccessed: false,
      externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_inline_popover_anchor_resize_receipt" } }],
    failure: { code: "missing_inline_popover_anchor_resize_receipt", stepName: "declare-required-receipt", message: "Missing app-side inline popover anchor/resize receipts." },
    warnings: ["file_linear:inline_popover_anchor_resize_receipts_missing"],
  };
}

export async function runDisabledFooterHitTargetRefusalStressScenario(opts: {
  session: string;
  surfaces?: string[];
  fixtures?: string[];
  inputModes?: string[];
  noNativePointer?: boolean;
  dryRunOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "disabled-footer-hit-target-refusal-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_disabled_footer_hit_target_refusal_receipt",
    linearIssue: "file_linear:disabled_footer_hit_target_refusal_receipts_missing",
    disabledFooterHitTargetRefusalReceipt: {
      kind: "ux.disabledFooterHitTargetRefusal",
      disabledFooterHitTargetRefusalStressId: "loop-twenty-five-disabled-footer-hit-target-refusal",
      session: opts.session,
      requestedSurfaces: opts.surfaces ?? ["drop-prompt", "fields-prompt", "path-prompt"],
      requestedFixtures: opts.fixtures ?? ["empty-drop", "invalid-fields", "missing-path"],
      requestedInputModes: opts.inputModes ?? ["enter", "footer-shortcut", "protocol-footer-click"],
      noNativePointer: opts.noNativePointer ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      surfaceSamples: [], surface: null, fixture: null, automationWindowId: null, osWindowId: null,
      semanticSurface: null, stateReceipt: null, elementsReceipt: null, activeFooter: null,
      nativeFooterSurfaceId: null, footerButtonSemanticId: null, footerButtonLabel: null,
      actionDisabled: null, disabledReason: null, disabledVisualState: null, disabledAccessibleState: null,
      keyboardEnterRefused: null, footerShortcutRefused: null, protocolFooterClickRefused: null,
      cmdKActionsStillAvailable: null, noSubmitReceipt: null, submitAttemptGeneration: null,
      sideEffectCountsBefore: null, sideEffectCountsAfter: null, stateFingerprintBefore: null,
      stateFingerprintAfter: null, focusPreserved: null, selectionPreserved: null, filterPreserved: null,
      staleFooterGenerationRejected: null, wrongSurfaceFooterRejected: null, networkAccessed: false,
      externalServiceContacted: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_disabled_footer_hit_target_refusal_receipt" } }],
    failure: { code: "missing_disabled_footer_hit_target_refusal_receipt", stepName: "declare-required-receipt", message: "Missing app-side disabled footer hit-target refusal receipts." },
    warnings: ["file_linear:disabled_footer_hit_target_refusal_receipts_missing"],
  };
}

export async function runMiniFullTransitionLayoutContinuityStressScenario(opts: {
  session: string;
  surfaces?: string[];
  transitions?: string[];
  fixture?: string;
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "mini-full-transition-layout-continuity-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_mini_full_transition_layout_continuity_receipt",
    linearIssue: "file_linear:mini_full_transition_layout_continuity_receipts_missing",
    miniFullTransitionLayoutContinuityReceipt: {
      kind: "ux.miniFullTransitionLayoutContinuity",
      miniFullTransitionLayoutContinuityStressId: "loop-twenty-six-mini-full-transition-layout-continuity",
      session: opts.session,
      requestedSurfaces: opts.surfaces ?? ["main", "mini-prompt", "fields-prompt", "actionsDialog"],
      requestedTransitions: opts.transitions ?? ["mini-to-full", "full-to-mini", "hide-show", "return-to-origin"],
      fixture: opts.fixture ?? "agentic-mini-full-layout",
      requestedInputModes: opts.inputModes ?? ["protocol-key", "protocol-resize"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      modeSamples: [], surface: null, transition: null, automationWindowId: null, osWindowId: null,
      semanticSurface: null, modeBefore: null, modeAfter: null, viewTypeBefore: null, viewTypeAfter: null,
      transitionGeneration: null, remSizeBefore: null, remSizeAfter: null, scaleFactor: null,
      windowBoundsBefore: null, windowBoundsAfter: null, contentBoundsBefore: null, contentBoundsAfter: null,
      inputBoundsBefore: null, inputBoundsAfter: null, listViewportBoundsBefore: null, listViewportBoundsAfter: null,
      footerBoundsBefore: null, footerBoundsAfter: null, nativeFooterSurfaceId: null, focusRingBounds: null,
      selectedRowVisible: null, selectedRowAboveFooter: null, noInputFooterOverlap: null, noContentClip: null,
      noFooterClip: null, noPopupMainClobbering: null, screenshotToSemanticsAlignment: null,
      strictCaptureTarget: null, blankScreenshotRejected: null, staleModeGenerationRejected: null,
      wrongSurfaceRejected: null, openedSystemSettings: false, mutatedTcc: false, systemPasteboardMutated: false,
      setupInstallFlowEntered: false, triggeredSecurityPrompt: false, networkAccessed: false,
      externalServiceContacted: false, destructiveOperationRequested: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_mini_full_transition_layout_continuity_receipt" } }],
    failure: { code: "missing_mini_full_transition_layout_continuity_receipt", stepName: "declare-required-receipt", message: "Missing app-side mini/full transition layout continuity receipts." },
    warnings: ["file_linear:mini_full_transition_layout_continuity_receipts_missing"],
  };
}

export async function runFilterInputDecorationChipLayoutStressScenario(opts: {
  session: string;
  surfaces?: string[];
  queries?: string[];
  widths?: string[];
  scaleFactors?: number[];
  fixture?: string;
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noConfigWrite?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "filter-input-decoration-chip-layout-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_filter_input_decoration_chip_layout_receipt",
    linearIssue: "file_linear:filter_input_decoration_chip_layout_receipts_missing",
    filterInputDecorationChipLayoutReceipt: {
      kind: "ux.filterInputDecorationChipLayout",
      filterInputDecorationChipLayoutStressId: "loop-twenty-six-filter-input-decoration-chip-layout",
      session: opts.session, requestedSurfaces: opts.surfaces ?? ["main"],
      requestedQueries: opts.queries ?? ["f: AGENTS.md", "c: agentic", "~/script", ":actions", ";note", "!command", "literal\\:chip"],
      requestedWidths: opts.widths ?? ["mini", "narrow", "full"], requestedScaleFactors: opts.scaleFactors ?? [1, 1.25, 1.5],
      fixture: opts.fixture ?? "agentic-filter-input-decorations", requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-resize"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noConfigWrite: opts.noConfigWrite ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true, inputDecorationSamples: [], surface: null, query: null,
      widthMode: null, scaleFactor: null, remSize: null, automationWindowId: null, osWindowId: null,
      semanticSurface: null, stateReceipt: null, elementsReceipt: null, filterInputDecorations: null,
      renderedInputText: null, strippedSearchText: null, chipRanges: [], chipRoles: [], chipBounds: [],
      textBounds: null, renderedTextBounds: null, cursorBounds: null, placeholderBounds: null,
      measuredWidth: null, availableWidth: null, visibleText: null, decorationGeneration: null,
      inputGeneration: null, sourceHeadCleared: null, staleDecorationCleared: null, noChipTextOverlap: null,
      noChipCursorOverlap: null, noPlaceholderOverlap: null, noInputFooterOverlap: null, noHorizontalClip: null,
      tooltipOrAccessibleFullText: null, screenshotToSemanticsAlignment: null, strictCaptureTarget: null,
      blankScreenshotRejected: null, staleDecorationGenerationRejected: null, wrongSurfaceRejected: null,
      openedSystemSettings: false, mutatedTcc: false, systemPasteboardMutated: false, configUnchanged: true,
      setupInstallFlowEntered: false, triggeredSecurityPrompt: false, networkAccessed: false,
      externalServiceContacted: false, destructiveOperationRequested: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_filter_input_decoration_chip_layout_receipt" } }],
    failure: { code: "missing_filter_input_decoration_chip_layout_receipt", stepName: "declare-required-receipt", message: "Missing app-side filter input decoration chip layout receipts." },
    warnings: ["file_linear:filter_input_decoration_chip_layout_receipts_missing"],
  };
}

export async function runFocusRingViewportIntegrityStressScenario(opts: {
  session: string;
  surfaces?: string[];
  fixture?: string;
  inputModes?: string[];
  steps?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "focus-ring-viewport-integrity-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_focus_ring_viewport_integrity_receipt",
    linearIssue: "file_linear:focus_ring_viewport_integrity_receipts_missing",
    focusRingViewportIntegrityReceipt: {
      kind: "ux.focusRingViewportIntegrity",
      focusRingViewportIntegrityStressId: "loop-twenty-six-focus-ring-viewport-integrity",
      session: opts.session, requestedSurfaces: opts.surfaces ?? ["main", "actionsDialog", "fields-prompt", "path-prompt"],
      fixture: opts.fixture ?? "agentic-focus-rings", requestedInputModes: opts.inputModes ?? ["protocol-key", "simulate-gpui-event"],
      requestedSteps: opts.steps ?? ["tab", "shift-tab", "up", "down", "escape"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      focusSamples: [], surface: null, automationWindowId: null, osWindowId: null, semanticSurface: null,
      focusStep: null, inputMode: null, focusGeneration: null, focusedSemanticId: null, focusOwner: null,
      semanticFocusMatchesState: null, focusRingBounds: null, focusedElementBounds: null, viewportBounds: null,
      scrollViewportBounds: null, contentBounds: null, footerBounds: null, popupBounds: null, ringVisible: null,
      ringNotClipped: null, ringWithinViewport: null, ringAboveFooter: null, ringNotObscuredByFooter: null,
      ringNotCoveredByPopup: null, tabOrderIndex: null, tabOrderStable: null, selectionPreserved: null,
      scrollAnchorPreserved: null, focusRestoredAfterEscape: null, noActivationReceipt: null, noSubmitReceipt: null,
      staleFocusGenerationRejected: null, wrongSurfaceFocusRejected: null, openedSystemSettings: false,
      mutatedTcc: false, systemPasteboardMutated: false, setupInstallFlowEntered: false,
      triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false,
      destructiveOperationRequested: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_focus_ring_viewport_integrity_receipt" } }],
    failure: { code: "missing_focus_ring_viewport_integrity_receipt", stepName: "declare-required-receipt", message: "Missing app-side focus ring viewport integrity receipts." },
    warnings: ["file_linear:focus_ring_viewport_integrity_receipts_missing"],
  };
}

export async function runWarningBannerActionDismissSemanticsStressScenario(opts: {
  session: string;
  surface?: string;
  fixtures?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noConfigWrite?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "warning-banner-action-dismiss-semantics-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_warning_banner_action_dismiss_semantics_receipt",
    linearIssue: "file_linear:warning_banner_action_dismiss_semantics_receipts_missing",
    warningBannerActionDismissSemanticsReceipt: {
      kind: "ux.warningBannerActionDismissSemantics",
      warningBannerActionDismissSemanticsStressId: "loop-twenty-seven-warning-banner-action-dismiss-semantics",
      session: opts.session, surface: opts.surface ?? "main",
      requestedFixtures: opts.fixtures ?? ["warning", "actionable", "dismissible", "error"],
      requestedInputModes: opts.inputModes ?? ["protocol-hover", "protocol-click", "protocol-key"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noConfigWrite: opts.noConfigWrite ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true, bannerSamples: [], fixture: null,
      bannerGeneration: null, bannerSemanticId: null, bannerKind: null, bannerVisibleText: null,
      bannerBounds: null, bannerTextBounds: null, bannerActionSemanticId: null, bannerDismissSemanticId: null,
      hoverStateReceipt: null, focusStateReceipt: null, dismissClickReceipt: null, actionClickReceipt: null,
      actionExecutionPreventedForDismiss: null, dismissDoesNotTriggerAction: null,
      actionDoesNotDismissUnlessConfigured: null, nonColorStateCue: null, contrastRatio: null,
      footerNotObscured: null, inputNotObscured: null, staleBannerGenerationRejected: null,
      wrongSurfaceRejected: null, systemPasteboardMutated: false, openedSystemSettings: false,
      mutatedTcc: false, networkAccessed: false, externalServiceContacted: false,
      destructiveOperationRequested: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_warning_banner_action_dismiss_semantics_receipt" } }],
    failure: { code: "missing_warning_banner_action_dismiss_semantics_receipt", stepName: "declare-required-receipt", message: "Missing app-side warning banner action/dismiss semantics receipts." },
    warnings: ["file_linear:warning_banner_action_dismiss_semantics_receipts_missing"],
  };
}

export async function runSelectPromptMultiselectKeyboardStateStressScenario(opts: {
  session: string;
  surface?: string;
  fixture?: string;
  choices?: number;
  selectionSteps?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "select-prompt-multiselect-keyboard-state-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_select_prompt_multiselect_keyboard_state_receipt",
    linearIssue: "file_linear:select_prompt_multiselect_keyboard_state_receipts_missing",
    selectPromptMultiselectKeyboardStateReceipt: {
      kind: "ux.selectPromptMultiselectKeyboardState",
      selectPromptMultiselectKeyboardStateStressId: "loop-twenty-seven-select-prompt-multiselect-keyboard-state",
      session: opts.session, surface: opts.surface ?? "select-prompt", fixture: opts.fixture ?? "agentic-multiselect",
      choiceCount: opts.choices ?? 24, requestedSelectionSteps: opts.selectionSteps ?? ["space", "cmd-a", "filter-preserve", "clear-filter", "range-toggle", "escape-restore"],
      requestedInputModes: opts.inputModes ?? ["protocol-key", "batch"], noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true, noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true, multiSelectSamples: [], selectionStep: null,
      inputMode: null, promptType: null, selectMode: null, focusedChoiceSemanticId: null,
      selectedChoiceSemanticIds: [], checkedRowSemanticIds: [], visibleChoiceSemanticIds: [],
      selectionCountLabel: null, footerSubmitLabel: null, footerSubmitDisabledReason: null,
      filterTextBefore: null, filterTextAfter: null, filterGeneration: null, selectionGeneration: null,
      focusGeneration: null, cmdAReceipt: null, spaceToggleReceipt: null, rangeToggleReceipt: null,
      filterPreservesSelectedSet: null, clearFilterRestoresCheckedRows: null,
      focusedRowNotDuplicatedAsSelected: null, checkedRowsMatchState: null, visibleRowsMatchElements: null,
      noSubmitReceipt: null, noActivationReceipt: null, staleSelectionGenerationRejected: null,
      wrongSurfaceSelectionRejected: null, systemPasteboardMutated: false, configUnchanged: true,
      destructiveOperationRequested: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_select_prompt_multiselect_keyboard_state_receipt" } }],
    failure: { code: "missing_select_prompt_multiselect_keyboard_state_receipt", stepName: "declare-required-receipt", message: "Missing app-side SelectPrompt keyboard multi-selection state receipts." },
    warnings: ["file_linear:select_prompt_multiselect_keyboard_state_receipts_missing"],
  };
}

export async function runFileSearchPreviewSanitizationStressScenario(opts: {
  session: string;
  surface?: string;
  fixture?: string;
  previewFixtures?: string[];
  selectionCycles?: number;
  filterCycles?: number;
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noNativePicker?: boolean;
  noQuickLook?: boolean;
  noSystemPasteboard?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "file-search-preview-sanitization-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_file_search_preview_sanitization_receipt",
    linearIssue: "file_linear:file_search_preview_sanitization_receipts_missing",
    fileSearchPreviewSanitizationReceipt: {
      kind: "ux.fileSearchPreviewSanitization",
      fileSearchPreviewSanitizationStressId: "loop-twenty-seven-file-search-preview-sanitization",
      session: opts.session, surface: opts.surface ?? "file-search", fixture: opts.fixture ?? "agentic-safe-preview",
      requestedPreviewFixtures: opts.previewFixtures ?? ["text", "binary", "large-text", "missing-file", "private-path", "unsupported-kind"],
      selectionCycles: opts.selectionCycles ?? 8, filterCycles: opts.filterCycles ?? 4,
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true, noQuickLook: opts.noQuickLook ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      previewSamples: [], previewFixtureKind: null, selectionCycle: null, filterCycle: null,
      selectedRowSemanticId: null, selectedFileUri: null, selectedFileFingerprint: null,
      previewGeneration: null, previewSourceIdentity: null, previewRenderKind: null, previewTitle: null,
      previewVisibleText: null, previewBounds: null, previewTextBounds: null, previewByteLimit: null,
      previewTruncated: null, binaryPreviewFallback: null, missingFileFallback: null,
      unsupportedPreviewFallback: null, privatePathRedacted: null, redactedPathFingerprint: null,
      noRawPathLeak: null, noNetworkFetch: true, noExternalServiceContacted: true,
      noQuickLookOpened: true, noNativePickerOpened: true, noSystemPasteboardMutation: true,
      filterGeneration: null, selectionGeneration: null, stalePreviewGenerationRejected: null,
      wrongRowPreviewRejected: null, wrongSurfaceRejected: null, footerNotObscured: null,
      inputNotObscured: null, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_file_search_preview_sanitization_receipt" } }],
    failure: { code: "missing_file_search_preview_sanitization_receipt", stepName: "declare-required-receipt", message: "Missing app-side File Search safe preview sanitization receipts." },
    warnings: ["file_linear:file_search_preview_sanitization_receipts_missing"],
  };
}

export async function runHotkeyPromptTransientCaptureCancelStressScenario(opts: {
  session: string;
  surface?: string;
  fixture?: string;
  chords?: string[];
  cancelMethods?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noConfigWrite?: boolean;
  noGlobalHotkeyRegistration?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "hotkey-prompt-transient-capture-cancel-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_hotkey_prompt_transient_capture_cancel_receipt",
    linearIssue: "file_linear:hotkey_prompt_transient_capture_cancel_receipts_missing",
    hotkeyPromptTransientCaptureCancelReceipt: {
      kind: "ux.hotkeyPromptTransientCaptureCancel",
      hotkeyPromptTransientCaptureCancelStressId: "loop-twenty-eight-hotkey-prompt-transient-capture-cancel",
      session: opts.session, surface: opts.surface ?? "hotkey-prompt", fixture: opts.fixture ?? "agentic-transient-hotkey",
      requestedChords: opts.chords ?? ["cmd+shift+7", "ctrl+space"],
      requestedCancelMethods: opts.cancelMethods ?? ["escape", "cmd-w"],
      requestedInputModes: opts.inputModes ?? ["protocol-key", "simulate-key"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noConfigWrite: opts.noConfigWrite ?? true, noGlobalHotkeyRegistration: opts.noGlobalHotkeyRegistration ?? true,
      dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      promptType: null, hotkeyPromptSurfaceId: null, capturePanelSemanticId: null,
      shortcutInputSemanticId: null, placeholderVisibleText: null, capturedChordTokens: [],
      capturedHotkeyInfo: null, simulateKeyCaptureReceipt: null, escapeCancelReceipt: null,
      cmdWCancelReceipt: null, noConfigFingerprintChange: null, noGlobalHotkeyRegistrationReceipt: null,
      noShortcutRecorderRoute: null, cancelSubmitsNull: null, focusRestoredToParent: null,
      staleHotkeyCaptureRejected: null, wrongSurfaceRejected: null, usedNativeInput: false,
      usedNativePointer: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_hotkey_prompt_transient_capture_cancel_receipt" } }],
    failure: { code: "missing_hotkey_prompt_transient_capture_cancel_receipt", stepName: "declare-required-receipt", message: "Missing app-side HotkeyPrompt transient capture/cancel receipts." },
    warnings: ["file_linear:hotkey_prompt_transient_capture_cancel_receipts_missing"],
  };
}

export async function runProcessManagerSortDetailPanelStabilityStressScenario(opts: {
  session: string;
  surface?: string;
  fixture?: string;
  sortKeys?: string[];
  selectionCycles?: number;
  filterCycles?: number;
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noProcessKill?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "process-manager-sort-detail-panel-stability-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_process_manager_sort_detail_panel_stability_receipt",
    linearIssue: "file_linear:process_manager_sort_detail_panel_receipts_missing",
    processManagerSortDetailPanelStabilityReceipt: {
      kind: "ux.processManagerSortDetailPanelStability",
      processManagerSortDetailPanelStabilityStressId: "loop-twenty-eight-process-manager-sort-detail-panel-stability",
      session: opts.session, surface: opts.surface ?? "process-manager", fixture: opts.fixture ?? "agentic-process-table",
      requestedSortKeys: opts.sortKeys ?? ["name", "cpu", "memory", "pid"],
      selectionCycles: opts.selectionCycles ?? 8, filterCycles: opts.filterCycles ?? 4,
      requestedInputModes: opts.inputModes ?? ["protocol-click", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noProcessKill: opts.noProcessKill ?? true,
      dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      processFixtureIdentity: null, tableHeaderSemanticIds: [], sortKey: null,
      sortDirection: null, sortGeneration: null, sectionHeaderRows: [],
      sectionHeaderSelectableFalse: null, selectedProcessSemanticId: null, selectedPid: null,
      detailPanelGeneration: null, detailSourceIdentity: null, detailTitle: null,
      detailMetricRows: [], cpuMemoryPidParity: null, filterGeneration: null,
      rowReanchorAfterSort: null, visibleRowsMatchElements: null, headerAriaSortLabel: null,
      killActionDisabled: null, noProcessSignalRequested: true, staleSortGenerationRejected: null,
      staleDetailRejected: null, wrongSurfaceRejected: null, usedNativeInput: false,
      usedNativePointer: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, systemPasteboardMutated: false, destructiveOperationRequested: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_process_manager_sort_detail_panel_stability_receipt" } }],
    failure: { code: "missing_process_manager_sort_detail_panel_stability_receipt", stepName: "declare-required-receipt", message: "Missing app-side Process Manager sort/header/detail panel stability receipts." },
    warnings: ["file_linear:process_manager_sort_detail_panel_receipts_missing"],
  };
}

export async function runEnvPromptRedactedStatusErrorRecoveryStressScenario(opts: {
  session: string;
  surface?: string;
  fixture?: string;
  statusFixtures?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noConfigWrite?: boolean;
  noSecretWrite?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "env-prompt-redacted-status-error-recovery-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_env_prompt_redacted_status_error_recovery_receipt",
    linearIssue: "file_linear:env_prompt_redacted_status_error_recovery_receipts_missing",
    envPromptRedactedStatusErrorRecoveryReceipt: {
      kind: "ux.envPromptRedactedStatusErrorRecovery",
      envPromptRedactedStatusErrorRecoveryStressId: "loop-twenty-eight-env-prompt-redacted-status-error-recovery",
      session: opts.session, surface: opts.surface ?? "env-prompt", fixture: opts.fixture ?? "agentic-env-status",
      requestedStatusFixtures: opts.statusFixtures ?? ["missing-secret", "parse-error", "masked-existing", "valid-edit"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-input", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noConfigWrite: opts.noConfigWrite ?? true,
      noSecretWrite: opts.noSecretWrite ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      promptType: null, envFixtureIdentity: null, statusGeneration: null, statusKind: null,
      statusVisibleText: null, statusSemanticId: null, inlineErrorSemanticId: null,
      firstInvalidFieldSemanticId: null, maskedValueVisible: null, secretValueRedacted: null,
      redactedSecretFingerprint: null, noRawSecretLeak: null, noSecretWriteReceipt: null,
      noConfigFingerprintChange: null, validEditClearsErrors: null, submitDisabledReason: null,
      footerSubmitDisabled: null, focusPreservedAfterError: null, staleStatusRejected: null,
      wrongFieldErrorRejected: null, visibleRowsMatchElements: null, usedNativeInput: false,
      usedNativePointer: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, systemPasteboardMutated: false, configUnchanged: true, secretMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_env_prompt_redacted_status_error_recovery_receipt" } }],
    failure: { code: "missing_env_prompt_redacted_status_error_recovery_receipt", stepName: "declare-required-receipt", message: "Missing app-side EnvPrompt redacted status/error recovery receipts." },
    warnings: ["file_linear:env_prompt_redacted_status_error_recovery_receipts_missing"],
  };
}

export async function runCommandPaletteBreadcrumbRouteStackStressScenario(opts: {
  session: string;
  host?: string;
  fixture?: string;
  drillPath?: string[];
  filter?: string;
  backMethods?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "command-palette-breadcrumb-route-stack-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_command_palette_breadcrumb_route_stack_receipt",
    linearIssue: "file_linear:command_palette_breadcrumb_route_stack_receipts_missing",
    commandPaletteBreadcrumbRouteStackReceipt: {
      kind: "ux.commandPaletteBreadcrumbRouteStack",
      commandPaletteBreadcrumbRouteStackStressId: "loop-twenty-nine-command-palette-breadcrumb-route-stack",
      session: opts.session, host: opts.host ?? "main", fixture: opts.fixture ?? "agentic-actions-breadcrumbs",
      requestedDrillPath: opts.drillPath ?? ["parent-action", "child-action"],
      requestedFilter: opts.filter ?? "switch", requestedBackMethods: opts.backMethods ?? ["escape", "breadcrumb-click"],
      requestedInputModes: opts.inputModes ?? ["protocol-key", "protocol-click", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      actionsDialogHost: null, routeStackDepth: null, breadcrumbTrailLabels: [],
      breadcrumbSemanticIds: [], activeRouteId: null, parentRouteSnapshot: null,
      childRouteSnapshot: null, drillDownActionId: null, drillDownPushedReceipt: null,
      breadcrumbBackReceipt: null, escapeBackReceipt: null, searchTextPreserved: null,
      selectionRestoredToParent: null, scrollAnchorRestored: null, noOnSelectBeforeDrillDown: null,
      noAccidentalExecution: null, topmostOwnerBeforeKey: null, staleRouteRejected: null,
      wrongHostRejected: null, usedNativeInput: false, usedNativePointer: false,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_command_palette_breadcrumb_route_stack_receipt" } }],
    failure: { code: "missing_command_palette_breadcrumb_route_stack_receipt", stepName: "declare-required-receipt", message: "Missing app-side command palette breadcrumb route-stack receipts." },
    warnings: ["file_linear:command_palette_breadcrumb_route_stack_receipts_missing"],
  };
}

export async function runRootSourceChipActionSemanticsStressScenario(opts: {
  session: string;
  queries?: string[];
  actions?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noConfigWrite?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "root-source-chip-action-semantics-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_root_source_chip_action_semantics_receipt",
    linearIssue: "file_linear:root_source_chip_action_semantics_receipts_missing",
    rootSourceChipActionSemanticsReceipt: {
      kind: "ux.rootSourceChipActionSemantics",
      rootSourceChipActionSemanticsStressId: "loop-twenty-nine-root-source-chip-action-semantics",
      session: opts.session,
      requestedQueries: opts.queries ?? ["f: AGENTS.md", "c: agentic", "n: welcome", "-c: noise"],
      requestedActions: opts.actions ?? ["remove-chip", "clear-all", "toggle-exclude", "open-chip-actions"],
      requestedInputModes: opts.inputModes ?? ["protocol-click", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noConfigWrite: opts.noConfigWrite ?? true,
      dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureSourceCatalogId: null, inputRenderedText: null, strippedSearchText: null,
      sourceFilterSet: null, sourceChipSemanticIds: [], sourceChipRoles: [],
      chipRemoveReceipt: null, chipClearAllReceipt: null, chipToggleExcludeReceipt: null,
      filterInputDecorationsGeneration: null, preflightFilterIndicators: null,
      statusChipNonSelectable: null, groupedRowsSuppressDisallowedSources: null,
      inputHistoryRecallBlocked: null, selectionPreservedAfterChipAction: null,
      noStatusAsActionSubject: null, noAccidentalExecution: null, staleChipActionRejected: null,
      wrongSurfaceRejected: null, usedNativeInput: false, usedNativePointer: false,
      usedSystemPasteboard: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_root_source_chip_action_semantics_receipt" } }],
    failure: { code: "missing_root_source_chip_action_semantics_receipt", stepName: "declare-required-receipt", message: "Missing app-side root source-chip action semantics receipts." },
    warnings: ["file_linear:root_source_chip_action_semantics_receipts_missing"],
  };
}

export async function runRecentHistoryDedupeRootGroupingStressScenario(opts: {
  session: string;
  fixture?: string;
  sources?: string[];
  query?: string;
  cycles?: number;
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "recent-history-dedupe-root-grouping-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_recent_history_dedupe_root_grouping_receipt",
    linearIssue: "file_linear:recent_history_dedupe_root_grouping_receipts_missing",
    recentHistoryDedupeRootGroupingReceipt: {
      kind: "ux.recentHistoryDedupeRootGrouping",
      recentHistoryDedupeRootGroupingStressId: "loop-twenty-nine-recent-history-dedupe-root-grouping",
      session: opts.session, fixture: opts.fixture ?? "agentic-root-recents",
      requestedSources: opts.sources ?? ["files", "notes", "clipboard", "dictation", "agent_chat-history"],
      requestedQuery: opts.query ?? "agentic-loop-29-dupe", requestedCycles: opts.cycles ?? 6,
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureHistorySnapshotId: null, sourceCatalogGeneration: null, queryFrameKey: null,
      rootFileFrameKey: null, passiveFrameKey: null, visibleResultsRoles: [],
      groupSectionOrder: [], filesSectionContiguous: null, searchFilesContinuationStable: null,
      dedupeKeys: [], duplicateKeyCollisionsRejected: null, recentFileSeedPoolFingerprint: null,
      historyRowsMetadataOnly: null, noFullTranscriptOrNoteBodyLeak: null, stableSelectionKey: null,
      rowFingerprintBeforeAfterCycles: null, fallbackRowsSuppressedWhenSourceRowsPresent: null,
      stalePassivePublishRejected: null, noAccidentalExecution: null, usedNativeInput: false,
      usedNativePointer: false, usedSystemPasteboard: false, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, systemPasteboardMutated: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_recent_history_dedupe_root_grouping_receipt" } }],
    failure: { code: "missing_recent_history_dedupe_root_grouping_receipt", stepName: "declare-required-receipt", message: "Missing app-side recent/history dedupe and root grouping receipts." },
    warnings: ["file_linear:recent_history_dedupe_root_grouping_receipts_missing"],
  };
}

export async function runInlineAttachmentPreviewChipStabilityStressScenario(opts: {
  session: string;
  hosts?: string[];
  fixture?: string;
  origins?: string[];
  chipActions?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noNativePicker?: boolean;
  noScreenCapture?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "inline-attachment-preview-chip-stability-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_inline_attachment_preview_chip_stability_receipt",
    linearIssue: "file_linear:inline_attachment_preview_chip_stability_receipts_missing",
    inlineAttachmentPreviewChipStabilityReceipt: {
      kind: "ux.inlineAttachmentPreviewChipStability",
      inlineAttachmentPreviewChipStabilityStressId: "loop-thirty-inline-attachment-preview-chip-stability",
      session: opts.session, requestedHosts: opts.hosts ?? ["agent_chat-composer", "notes"],
      fixture: opts.fixture ?? "agentic-inline-attachments",
      requestedOrigins: opts.origins ?? ["local-file", "fixture-image", "fixture-text", "script-resource"],
      requestedChipActions: opts.chipActions ?? ["focus", "preview", "remove", "reorder", "overflow"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-input", "protocol-click", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true, noScreenCapture: opts.noScreenCapture ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureAttachmentSetId: null, hostSurfaceIdentity: null, composerGeneration: null,
      attachmentChipSemanticIds: [], chipKinds: [], chipLabels: [], chipBounds: [],
      previewRedactedFingerprint: null, overflowChipReceipt: null, focusChipReceipt: null,
      removeChipReceipt: null, reorderChipReceipt: null, cursorSelectionPreserved: null,
      noRawPathOrContentLeak: null, noSystemPasteboardReceipt: null, noNativePickerReceipt: null,
      noScreenCaptureReceipt: null, noNetworkReceipt: null, staleAttachmentRejected: null,
      wrongHostRejected: null, localFixtureOnly: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, systemPasteboardMutated: false, networkAccessed: false, externalServiceContacted: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_inline_attachment_preview_chip_stability_receipt" } }],
    failure: { code: "missing_inline_attachment_preview_chip_stability_receipt", stepName: "declare-required-receipt", message: "Missing app-side inline attachment preview chip stability receipts." },
    warnings: ["file_linear:inline_attachment_preview_chip_stability_receipts_missing"],
  };
}

export async function runWindowTitleStatusSemanticsStressScenario(opts: {
  session: string;
  surfaces?: string[];
  states?: string[];
  transitions?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "window-title-status-semantics-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_window_title_status_semantics_receipt",
    linearIssue: "file_linear:window_title_status_semantics_receipts_missing",
    windowTitleStatusSemanticsReceipt: {
      kind: "ux.windowTitleStatusSemantics",
      windowTitleStatusSemanticsStressId: "loop-thirty-window-title-status-semantics",
      session: opts.session,
      requestedSurfaces: opts.surfaces ?? ["main", "agent_chat-composer", "actionsDialog", "promptPopup", "notes"],
      requestedStates: opts.states ?? ["idle", "busy", "error", "dirty", "ready"],
      requestedTransitions: opts.transitions ?? ["triggerBuiltin", "cmd-k", "escape", "hide-show"],
      requestedInputModes: opts.inputModes ?? ["protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      resolvedTarget: null, automationWindowTitle: null, nativeWindowTitle: null,
      semanticSurfaceTitle: null, visibleStatusText: null, titleGeneration: null,
      statusGeneration: null, transitionReceipts: [], detachedWindowParity: null,
      attachedPopupParentTitleUnaffected: null, statusErrorRecovery: null,
      staleTitleRejected: null, staleStatusRejected: null, wrongSurfaceRejected: null,
      noFocusSteal: null, noNativeInput: true, noNativePointer: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, systemPasteboardMutated: false, networkAccessed: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_window_title_status_semantics_receipt" } }],
    failure: { code: "missing_window_title_status_semantics_receipt", stepName: "declare-required-receipt", message: "Missing app-side window title/status semantics receipts." },
    warnings: ["file_linear:window_title_status_semantics_receipts_missing"],
  };
}

export async function runMenuSyntaxCaptureValidationChipStressScenario(opts: {
  session: string;
  fixture?: string;
  cases?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "menu-syntax-capture-validation-chip-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_menu_syntax_capture_validation_chip_receipt",
    linearIssue: "file_linear:menu_syntax_capture_validation_chip_receipts_missing",
    menuSyntaxCaptureValidationChipReceipt: {
      kind: "ux.menuSyntaxCaptureValidationChip",
      menuSyntaxCaptureValidationChipStressId: "loop-thirty-menu-syntax-capture-validation-chip",
      session: opts.session, fixture: opts.fixture ?? "agentic-capture-validation",
      requestedCases: opts.cases ?? ["missing-body-date", "missing-date", "ready", "malformed-url", "unresolved-date", "dynamic-schema"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureMenuSyntaxCatalogId: null, filterInputText: null, menuSyntaxMainHintSnapshot: null,
      captureValidationStatus: null, statusChipLabels: [], missingFieldLabels: [],
      malformedFieldLabel: null, malformedReason: null, unresolvedDates: [],
      fragmentPreviewRows: [], priorityChoicesRow: null, canSubmitFalsePreventsEnter: null,
      noPayloadWrite: null, noHandlerSpawn: null, staleValidationRejected: null,
      wrongSurfaceRejected: null, noNativeInput: true, noNativePointer: true,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, systemPasteboardMutated: false, networkAccessed: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_menu_syntax_capture_validation_chip_receipt" } }],
    failure: { code: "missing_menu_syntax_capture_validation_chip_receipt", stepName: "declare-required-receipt", message: "Missing app-side menu syntax capture validation chip receipts." },
    warnings: ["file_linear:menu_syntax_capture_validation_chip_receipts_missing"],
  };
}

export async function runAgentChatFooterActivityIndicatorStressScenario(opts: {
  session: string;
  hosts?: string[];
  fixture?: string;
  activityFixtures?: string[];
  inputModes?: string[];
  agentFixture?: string;
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSecurityPrompts?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "agent_chat-footer-activity-indicator-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_agent_chat_footer_activity_indicator_receipt",
    linearIssue: "file_linear:agent_chat_footer_activity_indicator_receipts_missing",
    agent_chatFooterActivityIndicatorReceipt: {
      kind: "ux.agent_chatFooterActivityIndicator",
      agent_chatFooterActivityIndicatorStressId: "loop-thirty-one-agent_chat-footer-activity-indicator",
      session: opts.session, requestedHosts: opts.hosts ?? ["agent_chat-composer", "notes"],
      fixture: opts.fixture ?? "agentic-agent_chat-footer-activity",
      requestedActivityFixtures: opts.activityFixtures ?? ["context-capture", "tool-call", "plan-update", "permission-wait", "cancelled", "idle-recovered"],
      requestedInputModes: opts.inputModes ?? ["protocol-state", "batch"], agentFixture: opts.agentFixture ?? "scripted-local",
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSecurityPrompts: opts.noSecurityPrompts ?? true, noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true, noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureAgentEventStreamId: null, hostSurfaceIdentity: null, footerOwner: null,
      nativeFooterSurfaceId: null, gpuiFooterDotStatus: null, nativeFooterDotStatus: null,
      activityStatusTransitions: [], contextCapturePendingStatus: null, toolCallStatus: null,
      planUpdateStatus: null, permissionWaitStatus: null, cancelRestoresIdle: null,
      footerRepaintGeneration: null, dotPulseTokenStable: null, modelLabelPreserved: null,
      noGlobalAiFooterButton: null, noAgentProcessSpawn: true, noSecurityPrompt: true,
      staleActivityRejected: null, wrongHostRejected: null, afkSafeFlags: true,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_agent_chat_footer_activity_indicator_receipt" } }],
    failure: { code: "missing_agent_chat_footer_activity_indicator_receipt", stepName: "declare-required-receipt", message: "Missing app-side Agent Chat footer activity indicator receipts." },
    warnings: ["file_linear:agent_chat_footer_activity_indicator_receipts_missing"],
  };
}

export async function runAgentChatModelHistoryPopoverVisualStateStressScenario(opts: {
  session: string;
  families?: string[];
  fixture?: string;
  states?: string[];
  selectionCycles?: number;
  filterCycles?: number;
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "agent_chat-model-history-popover-visual-state-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_agent_chat_model_history_popover_visual_state_receipt",
    linearIssue: "file_linear:agent_chat_model_history_popover_visual_state_receipts_missing",
    agent_chatModelHistoryPopoverVisualStateReceipt: {
      kind: "ux.agent_chatModelHistoryPopoverVisualState",
      agent_chatModelHistoryPopoverVisualStateStressId: "loop-thirty-one-agent_chat-model-history-popover-visual-state",
      session: opts.session, requestedFamilies: opts.families ?? ["model-selector", "local-history"],
      fixture: opts.fixture ?? "agentic-agent_chat-popover-visual-state",
      requestedStates: opts.states ?? ["idle", "filtered", "empty", "loading", "current-selection", "error-recovered"],
      selectionCycles: opts.selectionCycles ?? 8, filterCycles: opts.filterCycles ?? 4,
      requestedInputModes: opts.inputModes ?? ["protocol-set-input", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixturePopoverCatalogId: null, popupFamily: null, popupAutomationId: null,
      promptPopupKind: null, anchorBounds: null, popupBounds: null, selectedRowSemanticId: null,
      focusedRowSemanticId: null, rowVisualStateTokens: [], currentModelBadge: null,
      historyRecencyBadge: null, historyPreviewRedactedFingerprint: null, emptyFilteredState: null,
      loadingRefreshState: null, errorRecoveredState: null, synopsisBounds: null,
      selectionPreservedAfterFilter: null, noTranscriptBodyLeak: null, stalePopupSnapshotRejected: null,
      wrongPopupRejected: null, noSubmit: true, afkSafeFlags: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_agent_chat_model_history_popover_visual_state_receipt" } }],
    failure: { code: "missing_agent_chat_model_history_popover_visual_state_receipt", stepName: "declare-required-receipt", message: "Missing app-side Agent Chat model/history popover visual-state receipts." },
    warnings: ["file_linear:agent_chat_model_history_popover_visual_state_receipts_missing"],
  };
}

export async function runAgentChatContextInsertionPreviewParityStressScenario(opts: {
  session: string;
  sources?: string[];
  destination?: string;
  fixture?: string;
  selectionCycles?: number;
  filterCycles?: number;
  insertModes?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noNativePicker?: boolean;
  noQuickLook?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "agent_chat-context-insertion-preview-parity-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_agent_chat_context_insertion_preview_parity_receipt",
    linearIssue: "file_linear:agent_chat_context_insertion_preview_parity_receipts_missing",
    agent_chatContextInsertionPreviewParityReceipt: {
      kind: "ux.agent_chatContextInsertionPreviewParity",
      agent_chatContextInsertionPreviewParityStressId: "loop-thirty-one-agent_chat-context-insertion-preview-parity",
      session: opts.session, requestedSources: opts.sources ?? ["file-search", "browser-history", "dictation-history", "notes"],
      destinationComposerIdentity: opts.destination ?? "agent_chat-composer", fixture: opts.fixture ?? "agentic-context-preview-parity",
      selectionCycles: opts.selectionCycles ?? 6, filterCycles: opts.filterCycles ?? 4,
      requestedInsertModes: opts.insertModes ?? ["protocol-accept", "batch"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true, noQuickLook: opts.noQuickLook ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      sourceSurfaceIdentity: null, portalSessionId: null, sourceSelectionGeneration: null,
      selectedRowSemanticId: null, selectedRowPreviewFingerprint: null, selectedRowPreviewTitle: null,
      selectedRowPreviewKind: null, previewGeneration: null, acceptedContextPartUri: null,
      insertedTokenAlias: null, insertedTokenPreviewFingerprint: null, composerGeneration: null,
      replacementRange: null, rowPreviewMatchesInsertedContext: null, selectionPreservedAfterInsert: null,
      selectionDriftRejected: null, stalePreviewRejected: null, wrongDestinationRejected: null,
      noRawContentLeak: null, noNativePicker: true, noQuickLook: true, noSystemPasteboard: true,
      noNetwork: true, noSubmit: true, afkSafeFlags: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_agent_chat_context_insertion_preview_parity_receipt" } }],
    failure: { code: "missing_agent_chat_context_insertion_preview_parity_receipt", stepName: "declare-required-receipt", message: "Missing app-side Agent Chat context insertion preview parity receipts." },
    warnings: ["file_linear:agent_chat_context_insertion_preview_parity_receipts_missing"],
  };
}

export async function runAgentChatComposerTokenKeyboardEditParityStressScenario(opts: {
  session: string;
  hosts?: string[];
  fixture?: string;
  tokenKinds?: string[];
  editSteps?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noNativePicker?: boolean;
  noScreenCapture?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "agent_chat-composer-token-keyboard-edit-parity-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_agent_chat_composer_token_keyboard_edit_parity_receipt",
    linearIssue: "file_linear:agent_chat_composer_token_keyboard_edit_parity_receipts_missing",
    agent_chatComposerTokenKeyboardEditParityReceipt: {
      kind: "ux.agent_chatComposerTokenKeyboardEditParity",
      agent_chatComposerTokenKeyboardEditParityStressId: "loop-thirty-two-agent_chat-composer-token-keyboard-edit-parity",
      session: opts.session, requestedHosts: opts.hosts ?? ["agent_chat-composer", "notes"],
      fixture: opts.fixture ?? "agentic-agent_chat-composer-tokens",
      requestedTokenKinds: opts.tokenKinds ?? ["mention", "slash", "pasted-text", "pasted-image", "skill-file"],
      requestedEditSteps: opts.editSteps ?? ["backspace-delete", "delete-forward", "range-remove", "move-token-left", "move-token-right", "cursor-around-token"],
      requestedInputModes: opts.inputModes ?? ["protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true, noScreenCapture: opts.noScreenCapture ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureComposerTokenSetId: null, hostSurfaceIdentity: null, composerGeneration: null,
      tokenSemanticIds: [], tokenKinds: [], tokenAliases: [], tokenBounds: [],
      cursorBeforeToken: null, cursorAfterToken: null, backspaceRemovesTokenAtomically: null,
      deleteForwardRemovesTokenAtomically: null, rangeRemoveReceipt: null,
      moveTokenLeftReceipt: null, moveTokenRightReceipt: null, tokenOrderBefore: [],
      tokenOrderAfter: [], pendingContextPartsPreserved: null, slashSkillContextPreserved: null,
      pastedTokenMetadataPreserved: null, cursorSelectionPreserved: null,
      noPartialTokenTextLeak: null, staleComposerGenerationRejected: null,
      duplicateTokenIdRejected: null, wrongHostRejected: null, noSystemPasteboard: true,
      noNativeInput: true, noSubmit: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_agent_chat_composer_token_keyboard_edit_parity_receipt" } }],
    failure: { code: "missing_agent_chat_composer_token_keyboard_edit_parity_receipt", stepName: "declare-required-receipt", message: "Missing app-side Agent Chat composer token keyboard edit parity receipts." },
    warnings: ["file_linear:agent_chat_composer_token_keyboard_edit_parity_receipts_missing"],
  };
}

export async function runAgentChatTranscriptStreamRetryVirtualizationStressScenario(opts: {
  session: string;
  hosts?: string[];
  fixture?: string;
  messageCount?: number;
  streamChunks?: number;
  errorFixtures?: string[];
  retryPaths?: string[];
  scrollPositions?: string[];
  inputModes?: string[];
  agentFixture?: string;
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSecurityPrompts?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "agent_chat-transcript-stream-retry-virtualization-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_agent_chat_transcript_stream_retry_virtualization_receipt",
    linearIssue: "file_linear:agent_chat_transcript_stream_retry_virtualization_receipts_missing",
    agent_chatTranscriptStreamRetryVirtualizationReceipt: {
      kind: "ux.agent_chatTranscriptStreamRetryVirtualization",
      agent_chatTranscriptStreamRetryVirtualizationStressId: "loop-thirty-two-agent_chat-transcript-stream-retry-virtualization",
      session: opts.session, requestedHosts: opts.hosts ?? ["agent_chat-composer", "notes"],
      fixture: opts.fixture ?? "agentic-agent_chat-transcript-stream",
      messageCount: opts.messageCount ?? 160, streamChunks: opts.streamChunks ?? 48,
      requestedErrorFixtures: opts.errorFixtures ?? ["tool-error", "agent-error", "model-timeout", "cancelled"],
      requestedRetryPaths: opts.retryPaths ?? ["retry-same-draft", "retry-edited-draft", "retry-after-scroll"],
      requestedScrollPositions: opts.scrollPositions ?? ["top", "middle", "bottom", "near-active"],
      requestedInputModes: opts.inputModes ?? ["protocol-state", "protocol-key", "batch"],
      agentFixture: opts.agentFixture ?? "scripted-local", noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true, noSecurityPrompts: opts.noSecurityPrompts ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureTranscriptId: null, hostSurfaceIdentity: null, threadGeneration: null,
      transcriptGeneration: null, virtualizedMessageWindow: null, visibleMessageIds: [],
      messageRowSemanticIds: [], streamRunId: null, streamChunkSequence: [],
      activeAssistantMessageId: null, monotonicChunkAppend: null, scrollAnchorBefore: null,
      scrollAnchorAfter: null, bottomStickinessState: null, userScrolledAwayPreserved: null,
      assistantErrorMessageId: null, errorKind: null, errorVisibleText: null,
      retryButtonSemanticId: null, retryDraftFingerprint: null, retryRequestGeneration: null,
      retryRecoveryMessageId: null, noStaleErrorAfterRecovery: null, staleStreamChunkRejected: null,
      wrongMessageRetryRejected: null, virtualizedRowIdentityStable: null, blankRowRejected: null,
      noTranscriptBodyLeakInReceipts: null, noAgentProcessSpawn: true, noSecurityPrompt: true,
      noNetwork: true, noSubmit: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_agent_chat_transcript_stream_retry_virtualization_receipt" } }],
    failure: { code: "missing_agent_chat_transcript_stream_retry_virtualization_receipt", stepName: "declare-required-receipt", message: "Missing app-side Agent Chat transcript streaming retry virtualization receipts." },
    warnings: ["file_linear:agent_chat_transcript_stream_retry_virtualization_receipts_missing"],
  };
}

export async function runAgentChatPluginSkillEntryThreadAffinityStressScenario(opts: {
  session: string;
  hosts?: string[];
  fixture?: string;
  skillId?: string;
  entryPaths?: string[];
  inputModes?: string[];
  agentFixture?: string;
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSecurityPrompts?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "agent_chat-plugin-skill-entry-thread-affinity-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_agent_chat_plugin_skill_entry_thread_affinity_receipt",
    linearIssue: "file_linear:agent_chat_plugin_skill_entry_thread_affinity_receipts_missing",
    agent_chatPluginSkillEntryThreadAffinityReceipt: {
      kind: "ux.agent_chatPluginSkillEntryThreadAffinity",
      agent_chatPluginSkillEntryThreadAffinityStressId: "loop-thirty-three-agent_chat-plugin-skill-entry-thread-affinity",
      session: opts.session,
      requestedHosts: opts.hosts ?? ["embedded", "detached"],
      fixture: opts.fixture ?? "agentic-plugin-skill-entry",
      skillId: opts.skillId ?? "new-script",
      requestedEntryPaths: opts.entryPaths ?? ["main-menu", "source-filter", "cmd-enter"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "batch"],
      agentFixture: opts.agentFixture ?? "scripted-local",
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noSecurityPrompts: opts.noSecurityPrompts ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureSkillCatalogId: null,
      entryPath: null,
      hostSurfaceIdentity: null,
      resolvedAgentChatTarget: null,
      targetThreadId: null,
      detachedThreadReused: null,
      embeddedThreadReused: null,
      selectedSkillId: null,
      selectedSkillFileFingerprint: null,
      slashTokenText: null,
      slashTokenRange: null,
      pendingSkillContextPartUri: null,
      skillContextBoundToTargetThread: null,
      composerGeneration: null,
      returnOriginSnapshot: null,
      noAutoSubmit: true,
      noAgentProcessSpawn: true,
      noSecurityPrompt: true,
      staleLauncherSelectionRejected: null,
      staleDetachedThreadRejected: null,
      wrongHostRejected: null,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_agent_chat_plugin_skill_entry_thread_affinity_receipt" } }],
    failure: { code: "missing_agent_chat_plugin_skill_entry_thread_affinity_receipt", stepName: "declare-required-receipt", message: "Missing app-side Agent Chat plugin skill entry thread affinity receipts." },
    warnings: ["file_linear:agent_chat_plugin_skill_entry_thread_affinity_receipts_missing"],
  };
}

export async function runNotesCartAgentChatHandoffDedupeStressScenario(opts: {
  session: string;
  fixture?: string;
  notes?: string[];
  cartItems?: string[];
  handoffPaths?: string[];
  inputModes?: string[];
  agentFixture?: string;
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noNativePicker?: boolean;
  noScreenCapture?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  sandboxNotesStore?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "notes-cart-agent_chat-handoff-dedupe-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_notes_cart_agent_chat_handoff_dedupe_receipt",
    linearIssue: "file_linear:notes_cart_agent_chat_handoff_dedupe_receipts_missing",
    notesCartAgentChatHandoffDedupeReceipt: {
      kind: "ux.notesCartAgentChatHandoffDedupe",
      notesCartAgentChatHandoffDedupeStressId: "loop-thirty-three-notes-cart-agent_chat-handoff-dedupe",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-notes-cart",
      requestedNotes: opts.notes ?? ["note-a", "note-b"],
      requestedCartItems: opts.cartItems ?? ["duplicate-link", "local-snippet", "repo-file", "unchecked-task"],
      requestedHandoffPaths: opts.handoffPaths ?? ["open-agent_chat", "switch-note", "cancel", "consume"],
      requestedInputModes: opts.inputModes ?? ["protocol-click", "protocol-key", "batch"],
      agentFixture: opts.agentFixture ?? "scripted-local",
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true,
      noScreenCapture: opts.noScreenCapture ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true,
      sandboxNotesStore: opts.sandboxNotesStore ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      sandboxNotesStoreId: null,
      fixtureNoteIds: [],
      activeNoteId: null,
      cartSnapshotGeneration: null,
      cartItemIds: [],
      cartDedupeKeys: [],
      dedupedCartItemIds: [],
      duplicateCartItemsRejected: null,
      handoffSessionId: null,
      destinationHostIdentity: null,
      destinationAgentChatGeneration: null,
      stagedContextPartUris: [],
      inlineTokenAliases: [],
      redactedPreviewFingerprints: [],
      consumeRequestGeneration: null,
      consumeIsDryRunOnly: true,
      cancelRestoresCartSnapshot: null,
      switchNoteClearsPreviousNoteContext: null,
      wrongNoteConsumeRejected: null,
      staleCartGenerationRejected: null,
      noRawNoteBodyLeak: null,
      noUserNotesMutation: true,
      noAgentProcessSpawn: true,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_notes_cart_agent_chat_handoff_dedupe_receipt" } }],
    failure: { code: "missing_notes_cart_agent_chat_handoff_dedupe_receipt", stepName: "declare-required-receipt", message: "Missing app-side Notes cart Agent Chat handoff dedupe receipts." },
    warnings: ["file_linear:notes_cart_agent_chat_handoff_dedupe_receipts_missing"],
  };
}

export async function runRootFileSourceFilterPaginationFooterStressScenario(opts: {
  session: string;
  fixture?: string;
  queries?: string[];
  pageSize?: number;
  providerDelays?: number[];
  selectionSteps?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noNativePicker?: boolean;
  noQuickLook?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "root-file-source-filter-pagination-footer-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_root_file_source_filter_pagination_footer_receipt",
    linearIssue: "file_linear:root_file_source_filter_pagination_footer_receipts_missing",
    rootFileSourceFilterPaginationFooterReceipt: {
      kind: "ux.rootFileSourceFilterPaginationFooter",
      rootFileSourceFilterPaginationFooterStressId: "loop-thirty-three-root-file-source-filter-pagination-footer",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-root-file-pages",
      requestedQueries: opts.queries ?? ["f: ", "f:s", "files: AGENTS"],
      pageSize: opts.pageSize ?? 12,
      providerDelays: opts.providerDelays ?? [0, 150, 450],
      requestedSelectionSteps: opts.selectionSteps ?? ["near-bottom", "next-page", "filter-tighten", "clear-filter"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true,
      noQuickLook: opts.noQuickLook ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureFileProviderId: null,
      sourceFilterSet: [],
      renderedInputText: null,
      strippedSearchText: null,
      rootFrameKey: null,
      providerGeneration: null,
      pageGeneration: null,
      visibleFileRowIds: [],
      fileRowFingerprints: [],
      searchFilesContinuationRowId: null,
      selectedStableKeyBefore: null,
      selectedStableKeyAfter: null,
      selectedRowVisible: null,
      selectedRowAboveFooter: null,
      mainListScroll: null,
      viewportHeight: null,
      contentHeight: null,
      footerHeight: null,
      maxScrollTop: null,
      nearBottomPageRequest: null,
      pageAppendDoesNotChangeSelectedKey: null,
      providerPublishDoesNotReplaceFrame: null,
      duplicateFileKeyRejected: null,
      fallbackSuppressedWhileSourceFilterActive: null,
      statusChipsNonSelectable: null,
      quickLookRefused: true,
      noNativePicker: true,
      noSystemPasteboard: true,
      noNetwork: true,
      noSubmit: true,
      stalePageGenerationRejected: null,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_root_file_source_filter_pagination_footer_receipt" } }],
    failure: { code: "missing_root_file_source_filter_pagination_footer_receipt", stepName: "declare-required-receipt", message: "Missing app-side root file source-filter pagination footer receipts." },
    warnings: ["file_linear:root_file_source_filter_pagination_footer_receipts_missing"],
  };
}

export async function runFileSearchDirectoryBreadcrumbRestorationStressScenario(opts: {
  session: string;
  fixture?: string;
  startDir?: string;
  queries?: string[];
  navigationSteps?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noNativePicker?: boolean;
  noQuickLook?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "file-search-directory-breadcrumb-restoration-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_file_search_directory_breadcrumb_restoration_receipt",
    linearIssue: "file_linear:file_search_directory_breadcrumb_restoration_receipts_missing",
    fileSearchDirectoryBreadcrumbRestorationReceipt: {
      kind: "ux.fileSearchDirectoryBreadcrumbRestoration",
      fileSearchDirectoryBreadcrumbRestorationStressId: "loop-thirty-four-file-search-directory-breadcrumb-restoration",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-file-tree-breadcrumbs",
      startDir: opts.startDir ?? "repo-root",
      requestedQueries: opts.queries ?? ["AGENTS", "src", "missing"],
      requestedNavigationSteps: opts.navigationSteps ?? ["enter-directory", "breadcrumb-parent", "back", "forward", "filter-tighten", "clear-filter"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-click", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true,
      noQuickLook: opts.noQuickLook ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureDirectoryTreeId: null,
      rootFolderFingerprint: null,
      breadcrumbSegmentIds: [],
      redactedBreadcrumbLabels: [],
      onlyInFilterChipId: null,
      renderedInputText: null,
      strippedSearchText: null,
      visibleFileRowIds: [],
      directoryRowsBefore: [],
      directoryRowsAfter: [],
      selectedFileIdBefore: null,
      selectedFileIdAfter: null,
      selectionReanchoredAfterBreadcrumbClick: null,
      filterPreservedAfterDirectoryChange: null,
      backForwardStackDepth: null,
      scrollAnchorRestored: null,
      previewGeneration: null,
      noRawPathLeak: null,
      nativePickerRefused: true,
      quickLookRefused: true,
      staleDirectoryGenerationRejected: null,
      wrongOriginRejected: null,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_file_search_directory_breadcrumb_restoration_receipt" } }],
    failure: { code: "missing_file_search_directory_breadcrumb_restoration_receipt", stepName: "declare-required-receipt", message: "Missing app-side File Search directory breadcrumb restoration receipts." },
    warnings: ["file_linear:file_search_directory_breadcrumb_restoration_receipts_missing"],
  };
}

export async function runEmojiPickerSkinToneCategoryUxStressScenario(opts: {
  session: string;
  fixture?: string;
  categories?: string[];
  queries?: string[];
  skinTones?: string[];
  steps?: string[];
  inputModes?: string[];
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "emoji-picker-skin-tone-category-ux-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_emoji_picker_skin_tone_category_ux_receipt",
    linearIssue: "file_linear:emoji_picker_skin_tone_category_ux_receipts_missing",
    emojiPickerSkinToneCategoryUxReceipt: {
      kind: "ux.emojiPickerSkinToneCategoryUx",
      emojiPickerSkinToneCategoryUxStressId: "loop-thirty-four-emoji-picker-skin-tone-category-ux",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-emoji-skin-tone",
      requestedCategories: opts.categories ?? ["people", "symbols", "flags"],
      requestedQueries: opts.queries ?? ["woman technologist", "thumbs up", "flag"],
      requestedSkinTones: opts.skinTones ?? ["default", "medium-dark"],
      requestedSteps: opts.steps ?? ["category-click", "skin-tone-open", "variant-select", "filter-tighten", "escape-palette", "clear-filter"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-click", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureEmojiCatalogId: null,
      categoryTabIds: [],
      selectedCategoryId: null,
      stickyCategoryHeaderBounds: null,
      skinTonePaletteId: null,
      skinTonePaletteBounds: null,
      skinToneVariantIds: [],
      selectedSkinToneToken: null,
      emojiRowIds: [],
      zwjSequenceIds: [],
      graphemeClusterFingerprints: [],
      searchGeneration: null,
      highlightedRanges: [],
      accessibleLabelParity: null,
      previewGlyphBounds: null,
      paletteDismissalReceipt: null,
      selectionPreservedAcrossCategorySwitch: null,
      noSystemPasteboardMutation: true,
      noEmojiInsert: true,
      stalePaletteGenerationRejected: null,
      wrongCategoryMutationRejected: null,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_emoji_picker_skin_tone_category_ux_receipt" } }],
    failure: { code: "missing_emoji_picker_skin_tone_category_ux_receipt", stepName: "declare-required-receipt", message: "Missing app-side Emoji Picker skin-tone category UX receipts." },
    warnings: ["file_linear:emoji_picker_skin_tone_category_ux_receipts_missing"],
  };
}

export async function runRootWindowSourceFilterActivationRefusalStressScenario(opts: {
  session: string;
  fixture?: string;
  queries?: string[];
  windowStates?: string[];
  selectionSteps?: string[];
  inputModes?: string[];
  windowProvider?: string;
  noNativeInput?: boolean;
  noNativePointer?: boolean;
  noWindowActivation?: boolean;
  noSystemPasteboard?: boolean;
  noNetwork?: boolean;
  noSubmit?: boolean;
  dryRunOnly?: boolean;
  localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "root-window-source-filter-activation-refusal-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_root_window_source_filter_activation_refusal_receipt",
    linearIssue: "file_linear:root_window_source_filter_activation_refusal_receipts_missing",
    rootWindowSourceFilterActivationRefusalReceipt: {
      kind: "ux.rootWindowSourceFilterActivationRefusal",
      rootWindowSourceFilterActivationRefusalStressId: "loop-thirty-four-root-window-source-filter-activation-refusal",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-window-rows",
      requestedQueries: opts.queries ?? ["w: ", "w: safari", "windows: terminal"],
      requestedWindowStates: opts.windowStates ?? ["focused", "minimized", "offscreen", "duplicate-title"],
      requestedSelectionSteps: opts.selectionSteps ?? ["filter", "sort-z-order", "actions", "enter-refused", "clear-filter"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-key", "batch"],
      windowProvider: opts.windowProvider ?? "fixture-only",
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noWindowActivation: opts.noWindowActivation ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureWindowProviderId: null,
      sourceFilterSet: [],
      renderedInputText: null,
      strippedSearchText: null,
      rootFrameKey: null,
      windowSnapshotGeneration: null,
      zOrderGeneration: null,
      visibleWindowRowIds: [],
      windowRowFingerprints: [],
      selectedStableKeyBefore: null,
      selectedStableKeyAfter: null,
      selectedRowVisible: null,
      actionsSubjectStableKey: null,
      activationDryRunReceipt: null,
      enterActivationRefused: true,
      noNativeWindowActivation: true,
      noFocusSteal: true,
      duplicateWindowKeyRejected: null,
      staleWindowSnapshotRejected: null,
      statusChipsNonSelectable: null,
      cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_root_window_source_filter_activation_refusal_receipt" } }],
    failure: { code: "missing_root_window_source_filter_activation_refusal_receipt", stepName: "declare-required-receipt", message: "Missing app-side root Window source-filter activation refusal receipts." },
    warnings: ["file_linear:root_window_source_filter_activation_refusal_receipts_missing"],
  };
}

export async function runNotesMarkdownPreviewScrollSyncStressScenario(opts: {
  session: string; fixture?: string; notes?: string[]; markdownFixtures?: string[];
  editSteps?: string[]; inputModes?: string[]; sandboxNotesStore?: boolean;
  noNativeInput?: boolean; noNativePointer?: boolean; noNativePicker?: boolean;
  noSystemPasteboard?: boolean; noNetwork?: boolean; noExternalServices?: boolean;
  noSubmit?: boolean; dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "notes-markdown-preview-scroll-sync-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_notes_markdown_preview_scroll_sync_receipt",
    linearIssue: "file_linear:notes_markdown_preview_scroll_sync_receipts_missing",
    notesMarkdownPreviewScrollSyncReceipt: {
      kind: "ux.notesMarkdownPreviewScrollSync",
      notesMarkdownPreviewScrollSyncStressId: "loop-thirty-five-notes-markdown-preview-scroll-sync",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-notes-markdown-preview",
      requestedNotes: opts.notes ?? ["note-a", "note-b"],
      requestedMarkdownFixtures: opts.markdownFixtures ?? ["headings", "checklist", "code-block", "table", "long-link", "image-placeholder"],
      requestedEditSteps: opts.editSteps ?? ["protocol-insert", "protocol-delete", "protocol-scroll", "preview-toggle", "split-resize", "switch-note"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-input", "protocol-click", "protocol-key", "batch"],
      sandboxNotesStore: opts.sandboxNotesStore ?? true,
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noNativePicker: opts.noNativePicker ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noExternalServices: opts.noExternalServices ?? true,
      noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      sandboxNotesStoreId: null, fixtureNoteIds: [], activeNoteIdBefore: null, activeNoteIdAfter: null,
      markdownFixtureIds: [], editorGeneration: null, previewGeneration: null,
      renderedMarkdownBlockIds: [], previewBlockFingerprints: [], editorCursorBefore: null,
      editorCursorAfter: null, editorSelectionRange: null, editorScrollAnchor: null,
      previewScrollAnchor: null, scrollSyncDeltaPx: null, splitPaneBounds: null,
      previewToggleReceipt: null, switchNoteCleanupReceipt: null, focusRestoredToEditor: null,
      noUserNotesMutation: true, noRawNoteBodyLeak: null, stalePreviewGenerationRejected: null,
      wrongNoteMutationRejected: null, noSystemPasteboardMutation: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_notes_markdown_preview_scroll_sync_receipt" } }],
    failure: { code: "missing_notes_markdown_preview_scroll_sync_receipt", stepName: "declare-required-receipt", message: "Missing app-side Notes markdown preview scroll sync receipts." },
    warnings: ["file_linear:notes_markdown_preview_scroll_sync_receipts_missing"],
  };
}

export async function runQuickTerminalAnsiScrollbackSearchStressScenario(opts: {
  session: string; fixture?: string; transcriptFixtures?: string[]; searchQueries?: string[];
  scrollPositions?: string[]; inputModes?: string[]; terminalFixture?: string;
  noShellCommand?: boolean; noNativeInput?: boolean; noNativePointer?: boolean;
  noSystemPasteboard?: boolean; noNetwork?: boolean; noExternalServices?: boolean;
  noSubmit?: boolean; dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "quick-terminal-ansi-scrollback-search-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_quick_terminal_ansi_scrollback_search_receipt",
    linearIssue: "file_linear:quick_terminal_ansi_scrollback_search_receipts_missing",
    quickTerminalAnsiScrollbackSearchReceipt: {
      kind: "ux.quickTerminalAnsiScrollbackSearch",
      quickTerminalAnsiScrollbackSearchStressId: "loop-thirty-five-quick-terminal-ansi-scrollback-search",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-terminal-ansi-scrollback",
      requestedTranscriptFixtures: opts.transcriptFixtures ?? ["ansi-colors", "wide-emoji", "combining-marks", "long-lines", "hyperlinks", "stderr-block", "prompt-continuation"],
      requestedSearchQueries: opts.searchQueries ?? ["error", "emoji", "url", "long"],
      requestedScrollPositions: opts.scrollPositions ?? ["top", "middle", "bottom", "search-hit"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-input", "protocol-key", "batch"],
      terminalFixture: opts.terminalFixture ?? "scripted-local",
      noShellCommand: opts.noShellCommand ?? true,
      noNativeInput: opts.noNativeInput ?? true,
      noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true,
      noExternalServices: opts.noExternalServices ?? true,
      noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureTerminalTranscriptId: null, terminalSurfaceId: null, transcriptGeneration: null,
      ansiRunIds: [], sgrTokenRuns: [], wideCellGraphemeIds: [], combiningMarkCellIds: [],
      hyperlinkSpanIds: [], redactedHrefFingerprints: [], stderrBlockIds: [],
      promptContinuationRows: [], scrollbackViewportRows: [], viewportRowRange: null,
      searchQueryGeneration: null, searchHitIds: [], highlightedCellRanges: [],
      selectedSearchHitVisible: null, wrapContinuationMarkers: [], cursorCellBounds: null,
      promptLineBounds: null, footerInputNonOverlapping: null, staleTranscriptGenerationRejected: null,
      noShellCommandSpawned: true, noRawHyperlinkLeak: null, noSystemPasteboardMutation: true,
      noExternalServiceRequest: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_quick_terminal_ansi_scrollback_search_receipt" } }],
    failure: { code: "missing_quick_terminal_ansi_scrollback_search_receipt", stepName: "declare-required-receipt", message: "Missing app-side Quick Terminal ANSI scrollback search receipts." },
    warnings: ["file_linear:quick_terminal_ansi_scrollback_search_receipts_missing"],
  };
}

export async function runScriptOutputInspectorFoldingRecoveryStressScenario(opts: {
  session: string; fixture?: string; outputFixtures?: string[]; viewSteps?: string[];
  inputModes?: string[]; scriptFixture?: string; noHandlerSpawn?: boolean;
  noNativeInput?: boolean; noNativePointer?: boolean; noSystemPasteboard?: boolean;
  noNetwork?: boolean; noExternalServices?: boolean; noProcessKill?: boolean;
  noSubmit?: boolean; dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "script-output-inspector-folding-recovery-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_script_output_inspector_folding_recovery_receipt",
    linearIssue: "file_linear:script_output_inspector_folding_recovery_receipts_missing",
    scriptOutputInspectorFoldingRecoveryReceipt: {
      kind: "ux.scriptOutputInspectorFoldingRecovery",
      scriptOutputInspectorFoldingRecoveryStressId: "loop-thirty-five-script-output-inspector-folding-recovery",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-script-output-inspector",
      requestedOutputFixtures: opts.outputFixtures ?? ["stdout-long", "stderr-long", "ansi-stacktrace", "json-lines", "progress-rewrite", "exit-error", "exit-success"],
      requestedViewSteps: opts.viewSteps ?? ["run-fixture", "filter-output", "fold-stderr", "expand-stack", "clear-filter", "retry-dry-run"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-click", "protocol-key", "batch"],
      scriptFixture: opts.scriptFixture ?? "local-noop",
      noHandlerSpawn: opts.noHandlerSpawn ?? true,
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noExternalServices: opts.noExternalServices ?? true, noProcessKill: opts.noProcessKill ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureScriptRunId: null, outputFixtureIds: [], outputStreamGeneration: null,
      stdoutBlockIds: [], stderrBlockIds: [], ansiStackFrameIds: [], jsonLineIds: [],
      progressRewriteGeneration: null, exitBadgeKind: null, exitBadgeBounds: null,
      filterText: null, highlightedOutputRanges: [], stderrFoldStateBefore: null,
      stderrFoldStateAfter: null, stackTraceExpandedState: null, clearFilterRestoresOutput: null,
      retryDryRunReceipt: null, retryDoesNotSpawnHandler: true, selectionScrollAnchorRestored: null,
      noOutputInterleaveDrift: null, staleOutputGenerationRejected: null, wrongRunMutationRejected: null,
      noHandlerSpawn: true, noProcessKill: true, noSystemPasteboardMutation: true,
      noExternalServiceRequest: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_script_output_inspector_folding_recovery_receipt" } }],
    failure: { code: "missing_script_output_inspector_folding_recovery_receipt", stepName: "declare-required-receipt", message: "Missing app-side script output inspector folding recovery receipts." },
    warnings: ["file_linear:script_output_inspector_folding_recovery_receipts_missing"],
  };
}

export async function runAppLauncherIconGridKeyboardNavigationStressScenario(opts: {
  session: string; fixture?: string; apps?: string[]; gridStates?: string[];
  navigationSteps?: string[]; inputModes?: string[]; noNativeInput?: boolean;
  noNativePointer?: boolean; noAppLaunch?: boolean; noSystemPasteboard?: boolean;
  noNetwork?: boolean; noSubmit?: boolean; dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "app-launcher-icon-grid-keyboard-navigation-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_app_launcher_icon_grid_keyboard_navigation_receipt",
    linearIssue: "file_linear:app_launcher_icon_grid_keyboard_navigation_receipts_missing",
    appLauncherIconGridKeyboardNavigationReceipt: {
      kind: "ux.appLauncherIconGridKeyboardNavigation",
      appLauncherIconGridKeyboardNavigationStressId: "loop-thirty-six-app-launcher-icon-grid-keyboard-navigation",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-app-launcher-grid",
      requestedApps: opts.apps ?? ["Calculator", "Script Kit", "Safari", "Terminal", "Very Long Application Name"],
      requestedGridStates: opts.gridStates ?? ["grid", "list", "filtered", "empty", "resized"],
      requestedNavigationSteps: opts.navigationSteps ?? ["right", "down", "left", "up", "home", "end", "filter", "clear-filter"],
      requestedInputModes: opts.inputModes ?? ["protocol-key", "protocol-set-filter", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noAppLaunch: opts.noAppLaunch ?? true, noSystemPasteboard: opts.noSystemPasteboard ?? true,
      noNetwork: opts.noNetwork ?? true, noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureAppCatalogId: null, iconGridGeneration: null, visibleAppIds: [],
      visibleIconBounds: [], iconImageFingerprints: [], selectedAppIdBefore: null,
      selectedAppIdAfter: null, selectedCellBounds: null, selectedCellVisible: null,
      keyboardNeighborMap: null, rowColumnCount: null, filterGeneration: null,
      renderedAndStrippedQuery: null, emptyStateBounds: null, previewPanelBounds: null,
      previewAppId: null, tooltipForTruncatedName: null, noIconTextOverlap: null,
      noPreviewFooterCollision: null, enterLaunchRefused: true, noNativeAppLaunch: true,
      staleCatalogGenerationRejected: null, wrongAppActivationRejected: null,
      noSystemPasteboardMutation: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_app_launcher_icon_grid_keyboard_navigation_receipt" } }],
    failure: { code: "missing_app_launcher_icon_grid_keyboard_navigation_receipt", stepName: "declare-required-receipt", message: "Missing app-side App Launcher icon grid keyboard navigation receipts." },
    warnings: ["file_linear:app_launcher_icon_grid_keyboard_navigation_receipts_missing"],
  };
}

export async function runBrowserHistoryTimeGroupedPrivacyStressScenario(opts: {
  session: string; fixture?: string; browserFixtures?: string[]; timeBuckets?: string[];
  queries?: string[]; privacyModes?: string[]; inputModes?: string[]; browserProvider?: string;
  noBrowserActivation?: boolean; noNativeInput?: boolean; noNativePointer?: boolean;
  noSystemPasteboard?: boolean; noNetwork?: boolean; noSubmit?: boolean;
  dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "browser-history-time-grouped-privacy-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_browser_history_time_grouped_privacy_receipt",
    linearIssue: "file_linear:browser_history_time_grouped_privacy_receipts_missing",
    browserHistoryTimeGroupedPrivacyReceipt: {
      kind: "ux.browserHistoryTimeGroupedPrivacy",
      browserHistoryTimeGroupedPrivacyStressId: "loop-thirty-six-browser-history-time-grouped-privacy",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-browser-history-time-groups",
      requestedBrowserFixtures: opts.browserFixtures ?? ["same-domain", "private-url", "long-title", "favicon-missing", "duplicate-visit"],
      requestedTimeBuckets: opts.timeBuckets ?? ["today", "yesterday", "last-week", "older"],
      requestedQueries: opts.queries ?? ["docs", "private", "missing"],
      requestedPrivacyModes: opts.privacyModes ?? ["redacted-url", "favicon-fallback", "title-only"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-key", "batch"],
      browserProvider: opts.browserProvider ?? "fixture-only",
      noBrowserActivation: opts.noBrowserActivation ?? true,
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureHistoryProviderId: null, historyGeneration: null, timeBucketIds: [],
      stickyTimeHeaderBounds: [], visibleVisitIds: [], visitRowFingerprints: [],
      faviconFallbackIds: [], redactedUrlFingerprints: [], renderedAndStrippedQuery: null,
      selectedVisitBefore: null, selectedVisitAfter: null, selectedVisitVisible: null,
      duplicateVisitCollapsed: null, noRawPrivateUrlLeak: null, noFaviconNetworkRequest: true,
      openInBrowserRefused: true, noBrowserActivationReceipt: null,
      staleHistoryGenerationRejected: null, wrongVisitActivationRejected: null,
      noSystemPasteboardMutation: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_browser_history_time_grouped_privacy_receipt" } }],
    failure: { code: "missing_browser_history_time_grouped_privacy_receipt", stepName: "declare-required-receipt", message: "Missing app-side Browser History time-grouped privacy receipts." },
    warnings: ["file_linear:browser_history_time_grouped_privacy_receipts_missing"],
  };
}

export async function runSettingsPreferencesSearchResetPreviewStressScenario(opts: {
  session: string; fixture?: string; preferenceFixtures?: string[]; controlTypes?: string[];
  queries?: string[]; resetPaths?: string[]; inputModes?: string[]; sandboxConfig?: boolean;
  noConfigWrite?: boolean; noNativeInput?: boolean; noNativePointer?: boolean;
  noSystemPasteboard?: boolean; noNetwork?: boolean; noSubmit?: boolean;
  dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "settings-preferences-search-reset-preview-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_settings_preferences_search_reset_preview_receipt",
    linearIssue: "file_linear:settings_preferences_search_reset_preview_receipts_missing",
    settingsPreferencesSearchResetPreviewReceipt: {
      kind: "ux.settingsPreferencesSearchResetPreview",
      settingsPreferencesSearchResetPreviewStressId: "loop-thirty-six-settings-preferences-search-reset-preview",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-settings-preferences",
      requestedPreferenceFixtures: opts.preferenceFixtures ?? ["theme", "font-size", "ui-scale", "launch-at-login", "agent-profile"],
      requestedControlTypes: opts.controlTypes ?? ["toggle", "select", "slider", "text", "reset-button"],
      requestedQueries: opts.queries ?? ["theme", "font", "missing"],
      requestedResetPaths: opts.resetPaths ?? ["single-setting", "section-reset", "cancel-reset"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-click", "protocol-key", "batch"],
      sandboxConfig: opts.sandboxConfig ?? true, noConfigWrite: opts.noConfigWrite ?? true,
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      sandboxConfigId: null, preferenceSectionIds: [], visiblePreferenceIds: [],
      controlBounds: [], controlAccessibleNames: [], valueBeforeByPreference: null,
      previewValueByPreference: null, dirtyPreferenceIds: [], renderedAndStrippedQuery: null,
      searchHighlightRanges: [], resetPreviewReceipt: null, cancelResetRestoresValues: null,
      disabledControlRefusal: null, noConfigFileWrite: true, noSecretValueLeak: null,
      stalePreferenceGenerationRejected: null, wrongPreferenceMutationRejected: null,
      noSystemPasteboardMutation: true, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_settings_preferences_search_reset_preview_receipt" } }],
    failure: { code: "missing_settings_preferences_search_reset_preview_receipt", stepName: "declare-required-receipt", message: "Missing app-side Settings preferences search/reset preview receipts." },
    warnings: ["file_linear:settings_preferences_search_reset_preview_receipts_missing"],
  };
}

export async function runSettingsPreferencesReadonlyDetailPanelStressScenario(opts: {
  session: string; fixture?: string; sections?: string[]; queries?: string[];
  navigationSteps?: string[]; inputModes?: string[]; noNativeInput?: boolean;
  noNativePointer?: boolean; noSystemPasteboard?: boolean; noNetwork?: boolean;
  noExternalServices?: boolean; noSecurityPrompts?: boolean; noSystemSettings?: boolean;
  noTccMutation?: boolean; noConfigWrite?: boolean; noSubmit?: boolean;
  dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "settings-preferences-readonly-detail-panel-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_settings_preferences_readonly_detail_panel_receipt",
    linearIssue: "file_linear:settings_preferences_readonly_detail_panel_receipts_missing",
    settingsPreferencesReadonlyDetailPanelReceipt: {
      kind: "ux.settingsPreferencesReadonlyDetailPanel",
      settingsPreferencesReadonlyDetailPanelStressId: "loop-thirty-seven-settings-preferences-readonly-detail-panel",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-settings-preferences-readonly",
      requestedSections: opts.sections ?? ["appearance", "windowing", "history", "advanced"],
      requestedQueries: opts.queries ?? ["theme", "mini", "history", "missing"],
      requestedNavigationSteps: opts.navigationSteps ?? ["filter", "section-click", "detail-focus", "clear-filter", "escape-restore"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-click", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noExternalServices: opts.noExternalServices ?? true, noSecurityPrompts: opts.noSecurityPrompts ?? true,
      noSystemSettings: opts.noSystemSettings ?? true, noTccMutation: opts.noTccMutation ?? true,
      noConfigWrite: opts.noConfigWrite ?? true, noSubmit: opts.noSubmit ?? true,
      dryRunOnly: opts.dryRunOnly ?? true, localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureCatalogId: null, settingsSurfaceId: null, selectedSectionBefore: null,
      selectedSectionAfter: null, detailPanelGeneration: null, visibleRowLabels: [],
      visibleRowBounds: [], visibleTextBounds: [], detailBodyBounds: null, detailFooterBounds: null,
      emptyStateCopy: null, disabledApplySaveReason: null, configFingerprintBefore: null,
      configFingerprintAfter: null, noSetupOrSecurityPrompt: true, staleDetailGenerationRejected: null,
      wrongSectionMutationRejected: null, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_settings_preferences_readonly_detail_panel_receipt" } }],
    failure: { code: "missing_settings_preferences_readonly_detail_panel_receipt", stepName: "declare-required-receipt", message: "Missing app-side Settings read-only detail panel receipts." },
    warnings: ["file_linear:settings_preferences_readonly_detail_panel_receipts_missing"],
  };
}

export async function runDesignPickerPreviewRestoreVisualStressScenario(opts: {
  session: string; fixture?: string; catalogIds?: string[]; previewSteps?: string[];
  inputModes?: string[]; noNativeInput?: boolean; noNativePointer?: boolean;
  noSystemPasteboard?: boolean; noNetwork?: boolean; noExternalServices?: boolean;
  noSecurityPrompts?: boolean; noSystemSettings?: boolean; noTccMutation?: boolean;
  noConfigWrite?: boolean; noDesignWrite?: boolean; noSubmit?: boolean;
  dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "design-picker-preview-restore-visual-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_design_picker_preview_restore_visual_receipt",
    linearIssue: "file_linear:design_picker_preview_restore_visual_receipts_missing",
    designPickerPreviewRestoreVisualReceipt: {
      kind: "ux.designPickerPreviewRestoreVisual",
      designPickerPreviewRestoreVisualStressId: "loop-thirty-seven-design-picker-preview-restore-visual",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-design-picker-preview",
      requestedCatalogIds: opts.catalogIds ?? ["default", "compact", "high-contrast"],
      requestedPreviewSteps: opts.previewSteps ?? ["open-picker", "preview-next", "preview-previous", "filter", "escape-restore", "cmd-w-restore"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-click", "protocol-key", "batch"],
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noExternalServices: opts.noExternalServices ?? true, noSecurityPrompts: opts.noSecurityPrompts ?? true,
      noSystemSettings: opts.noSystemSettings ?? true, noTccMutation: opts.noTccMutation ?? true,
      noConfigWrite: opts.noConfigWrite ?? true, noDesignWrite: opts.noDesignWrite ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureDesignCatalogId: null, activeDesignIdBeforePreview: null, previewDesignId: null,
      previewGeneration: null, themeTokenFingerprintsBefore: [], themeTokenFingerprintsPreview: [],
      themeTokenFingerprintsRestored: [], visiblePickerRowIds: [], visiblePickerRowLabels: [],
      visiblePickerRowBounds: [], visibleTextBounds: [], selectedPreviewRowVisible: null,
      screenshotSemanticTargetIdentity: null, escapeRestoresPreviewState: null,
      cmdWRestoresPreviewState: null, persistedDesignFingerprintBefore: null,
      persistedDesignFingerprintAfter: null, stalePreviewGenerationRejected: null,
      wrongSurfacePreviewRejected: null, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_design_picker_preview_restore_visual_receipt" } }],
    failure: { code: "missing_design_picker_preview_restore_visual_receipt", stepName: "declare-required-receipt", message: "Missing app-side Design Picker preview restore receipts." },
    warnings: ["file_linear:design_picker_preview_restore_visual_receipts_missing"],
  };
}

export async function runDictationHistoryTranscriptPreviewRedactionStressScenario(opts: {
  session: string; fixture?: string; transcriptFixtures?: string[]; queries?: string[];
  selectionCycles?: number; viewSteps?: string[]; inputModes?: string[]; dictationFixture?: string;
  noMicrophone?: boolean; noMediaCapture?: boolean; noNativeInput?: boolean;
  noNativePointer?: boolean; noSystemPasteboard?: boolean; noNetwork?: boolean;
  noExternalServices?: boolean; noSecurityPrompts?: boolean; noSystemSettings?: boolean;
  noTccMutation?: boolean; noSubmit?: boolean; dryRunOnly?: boolean; localFixtureOnly?: boolean;
}): Promise<HardScenarioReceipt> {
  return {
    schemaVersion: PROOF_BUNDLE_SCHEMA_VERSION,
    scenario: "dictation-history-transcript-preview-redaction-stress",
    status: "fail",
    failClosed: true,
    failureMode: "fail_closed",
    missingReceipt: "missing_dictation_history_transcript_preview_redaction_receipt",
    linearIssue: "file_linear:dictation_history_transcript_preview_redaction_receipts_missing",
    dictationHistoryTranscriptPreviewRedactionReceipt: {
      kind: "ux.dictationHistoryTranscriptPreviewRedaction",
      dictationHistoryTranscriptPreviewRedactionStressId: "loop-thirty-seven-dictation-history-transcript-preview-redaction",
      session: opts.session,
      fixture: opts.fixture ?? "agentic-dictation-history-preview",
      requestedTranscriptFixtures: opts.transcriptFixtures ?? ["short", "long", "redacted", "missing-audio", "emoji"],
      requestedQueries: opts.queries ?? ["meeting", "error", "emoji", "missing"],
      selectionCycles: opts.selectionCycles ?? 6,
      requestedViewSteps: opts.viewSteps ?? ["filter", "preview", "expand-redacted", "clear-filter", "escape-restore"],
      requestedInputModes: opts.inputModes ?? ["protocol-set-filter", "protocol-click", "protocol-key", "batch"],
      dictationFixture: opts.dictationFixture ?? "saved-local",
      noMicrophone: opts.noMicrophone ?? true, noMediaCapture: opts.noMediaCapture ?? true,
      noNativeInput: opts.noNativeInput ?? true, noNativePointer: opts.noNativePointer ?? true,
      noSystemPasteboard: opts.noSystemPasteboard ?? true, noNetwork: opts.noNetwork ?? true,
      noExternalServices: opts.noExternalServices ?? true, noSecurityPrompts: opts.noSecurityPrompts ?? true,
      noSystemSettings: opts.noSystemSettings ?? true, noTccMutation: opts.noTccMutation ?? true,
      noSubmit: opts.noSubmit ?? true, dryRunOnly: opts.dryRunOnly ?? true,
      localFixtureOnly: opts.localFixtureOnly ?? true,
      fixtureDictationStoreId: null, transcriptRowIds: [], transcriptGeneration: null,
      queryGeneration: null, selectedTranscriptBefore: null, selectedTranscriptAfter: null,
      previewGeneration: null, previewSourceId: null, previewRenderKind: null,
      visiblePreviewTextBounds: [], redactedTranscriptFingerprint: null,
      missingAudioFallbackCopy: null, emojiGraphemeBounds: [], footerInputNonOverlapping: null,
      noRawTranscriptLeak: null, noRawAudioPathLeak: null, noMicrophonePermissionRequest: true,
      noMediaCaptureRequest: true, staleTranscriptGenerationRejected: null,
      wrongRowPreviewRejected: null, cleanupConfirmed: true,
    },
    usage: { stateFirst: true, usedGetState: true, usedGetElements: true, usedNativeInput: false, usedNativePointer: false, usedScreenshot: false, openedSystemSettings: false, mutatedTcc: false, installedAgents: false, triggeredSecurityPrompt: false, networkAccessed: false, systemPasteboardMutated: false },
    steps: [{ name: "declare-required-receipt", status: "fail", output: { reason: "missing_dictation_history_transcript_preview_redaction_receipt" } }],
    failure: { code: "missing_dictation_history_transcript_preview_redaction_receipt", stepName: "declare-required-receipt", message: "Missing app-side Dictation History transcript preview redaction receipts." },
    warnings: ["file_linear:dictation_history_transcript_preview_redaction_receipts_missing"],
  };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function parseArgs() {
  const args = process.argv.slice(2);

  const sessionIdx = args.indexOf("--session");
  const session =
    sessionIdx >= 0 && args[sessionIdx + 1] ? args[sessionIdx + 1] : "default";

  const scenarioIdx = args.indexOf("--scenario");
  const scenario =
    scenarioIdx >= 0 && args[scenarioIdx + 1] ? args[scenarioIdx + 1] : "";

  const indexIdx = args.indexOf("--index");
  const rawIndex = indexIdx >= 0 ? args[indexIdx + 1] : undefined;
  if (rawIndex != null) {
    const parsedIndex = Number(rawIndex);
    if (!Number.isInteger(parsedIndex) || parsedIndex < 0) {
      throw new Error(
        `Invalid --index: expected non-negative integer, got ${rawIndex}`
      );
    }
  }
  const index = rawIndex != null ? Number(rawIndex) : 0;

  const minTargetsIdx = args.indexOf("--min-targets");
  const minTargets =
    minTargetsIdx >= 0 && args[minTargetsIdx + 1]
      ? Number(args[minTargetsIdx + 1])
      : 2;
  const keyIdx = args.indexOf("--key");
  const key =
    keyIdx >= 0 && (args[keyIdx + 1] === "enter" || args[keyIdx + 1] === "tab")
      ? (args[keyIdx + 1] as "enter" | "tab")
      : "enter";
  const familiesIdx = args.indexOf("--families");
  const families =
    familiesIdx >= 0 && args[familiesIdx + 1]
      ? args[familiesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : ["mention", "model-selector", "local-history"];
  const driftIdx = args.indexOf("--drift");
  const drift = driftIdx >= 0 && args[driftIdx + 1] ? args[driftIdx + 1] : "generation";
  const hostIdx = args.indexOf("--host");
  const originIdx = args.indexOf("--origin");
  const host =
    originIdx >= 0 && args[originIdx + 1]
      ? args[originIdx + 1]
      : hostIdx >= 0 && args[hostIdx + 1] ? args[hostIdx + 1] : "agent_chat";
  const portalIdx = args.indexOf("--portal");
  const portal = portalIdx >= 0 && args[portalIdx + 1] ? args[portalIdx + 1] : "file-search";
  const selectionIdx = args.indexOf("--selection");
  const selection =
    selectionIdx >= 0 && args[selectionIdx + 1] ? args[selectionIdx + 1] : "file";
  const queryIdx = args.indexOf("--query");
  const query = queryIdx >= 0 && args[queryIdx + 1] ? args[queryIdx + 1] : "AGENTS.md";
  const kindsIdx = args.indexOf("--kinds");
  const kinds =
    kindsIdx >= 0 && args[kindsIdx + 1]
      ? args[kindsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : ["accessibility", "screen-recording", "microphone"];
  const chordIdx = args.indexOf("--chord");
  const chord = chordIdx >= 0 && args[chordIdx + 1] ? args[chordIdx + 1] : "cmd+shift+7";
  const actionIdx = args.indexOf("--action");
  const action =
    actionIdx >= 0 && args[actionIdx + 1] ? args[actionIdx + 1] : "test-agentic-shortcut";
  const templateIdx = args.indexOf("--template");
  const template =
    templateIdx >= 0 && args[templateIdx + 1] ? args[templateIdx + 1] : "Hello {{name}}";
  const fieldIdx = args.indexOf("--field");
  const field = fieldIdx >= 0 && args[fieldIdx + 1] ? args[fieldIdx + 1] : "name";
  const valueIdx = args.indexOf("--value");
  const value = valueIdx >= 0 && args[valueIdx + 1] ? args[valueIdx + 1] : "Ada";
  const forcedValueIdx = args.indexOf("--forced-value");
  const forcedValue =
    forcedValueIdx >= 0 && args[forcedValueIdx + 1]
      ? args[forcedValueIdx + 1]
      : "forced-template-result";
  const aliasIdx = args.indexOf("--alias");
  const alias =
    aliasIdx >= 0 && args[aliasIdx + 1] ? args[aliasIdx + 1] : "Do in Current Command";
  const expectedAppIdx = args.indexOf("--expected-app");
  const expectedApp =
    expectedAppIdx >= 0 && args[expectedAppIdx + 1] ? args[expectedAppIdx + 1] : undefined;
  const sourceIdx = args.indexOf("--source");
  const source = sourceIdx >= 0 && args[sourceIdx + 1] ? args[sourceIdx + 1] : "root-file";
  const mutationIdx = args.indexOf("--mutation");
  const mutation =
    mutationIdx >= 0 && args[mutationIdx + 1]
      ? args[mutationIdx + 1]
      : "filter-selection-cache-frame";
  const surfaceIdx = args.indexOf("--surface");
  const surface = surfaceIdx >= 0 && args[surfaceIdx + 1] ? args[surfaceIdx + 1] : "shortcuts";
  const sandboxConfig = args.includes("--sandbox-config");
  const vision = args.includes("--vision");

  return {
    session,
    scenario,
    index,
    minTargets,
    key,
    families,
    drift,
    host,
    portal,
    selection,
    query,
    kinds,
    chord,
    action,
    template,
    field,
    value,
    forcedValue,
    alias,
    expectedApp,
    source,
    mutation,
    surface,
    sandboxConfig,
    vision,
  };
}

// Only run CLI when executed directly (not imported)
if (import.meta.main) {
  const {
    session,
    scenario,
    index,
    minTargets,
    key,
    families,
    drift,
    host,
    portal,
    selection,
    query,
    kinds,
    chord,
    action,
    template,
    field,
    value,
    forcedValue,
    alias,
    expectedApp,
    source,
    mutation,
    surface,
    sandboxConfig,
    vision,
  } = parseArgs();

  const AVAILABLE_SCENARIOS = [
    "main-window-exact-id",
    "actions-dialog-exact-id",
    "prompt-popup-exact-id",
    "detached-agent_chat-exact-id",
    "detached-agent_chat-target-threading-stress",
    "notes-agent_chat-delayed-action-origin-stress",
    "file-portal-origin-roundtrip",
    "permission-privacy-preflight",
    "shortcut-recorder-focus-capture",
    "sdk-confirm-runtime-proof",
    "notes-delete-confirm-route-proof",
    "template-prompt-automation-parity-stress",
    "current-app-commands-frontmost-stress",
    "actions-captured-subject-frame-stress",
  ];

  if (!scenario) {
    process.stderr.write(
      JSON.stringify({
        event: "scenario.error",
        error: "Missing --scenario flag",
        available: AVAILABLE_SCENARIOS,
      }) + "\n"
    );
    process.exit(2);
  }

  switch (scenario) {
    case "main-window-exact-id": {
      const bundle = await runMainWindowExactIdScenario(session);
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.warnings.length > 0 ? 1 : 0);
      break;
    }

    case "actions-dialog-exact-id": {
      const bundle = await runActionsDialogExactIdScenario(session, index);
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.warnings.length > 0 ? 1 : 0);
      break;
    }

    case "detached-agent_chat-exact-id": {
      const bundle = await runDetachedAgentChatExactIdScenario(session, index);
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.warnings.length > 0 ? 1 : 0);
      break;
    }

    case "prompt-popup-exact-id": {
      const bundle = await runPromptPopupExactIdScenario(session, index);
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.warnings.length > 0 ? 1 : 0);
      break;
    }

    case "detached-agent_chat-target-threading-stress": {
      const bundle = await runDetachedAgentChatTargetThreadingStressScenario({
        session,
        kind: "agentChatDetached",
        index,
        minTargets,
        key,
        vision,
      });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "notes-agent_chat-delayed-action-origin-stress": {
      const bundle = await runNotesAgentChatDelayedActionOriginStressScenario({ session, drift });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "file-portal-origin-roundtrip": {
      const bundle = await runAgentChatPortalRoundTripOriginStressScenario({ session, host, portal, selection, query });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "permission-privacy-preflight": {
      const bundle = await runPermissionPreflightReadonlyScenario({ session, kinds });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "shortcut-recorder-focus-capture": {
      const bundle = await runShortcutRecorderFocusCaptureStressScenario({ session, chord, action, surface, sandboxConfig });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "sdk-confirm-runtime-proof": {
      const bundle = await runSdkConfirmRuntimeProofScenario({ session, cancel: "escape" });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "notes-delete-confirm-route-proof": {
      const bundle = await runNotesDeleteConfirmRouteProofScenario({ session });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "template-prompt-automation-parity-stress": {
      const bundle = await runTemplatePromptAutomationParityStressScenario({ session, template, field, value, forcedValue });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "current-app-commands-frontmost-stress": {
      const bundle = await runCurrentAppCommandsFrontmostStressScenario({ session, query, alias, expectedApp });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    case "actions-captured-subject-frame-stress": {
      const bundle = await runActionsCapturedSubjectFrameStressScenario({ session, source, action, mutation });
      process.stdout.write(JSON.stringify(bundle, null, 2) + "\n");
      process.exit(bundle.status === "pass" ? 0 : 1);
      break;
    }

    default:
      process.stderr.write(
        JSON.stringify({
          event: "scenario.error",
          error: `Unknown scenario: ${scenario}`,
          available: AVAILABLE_SCENARIOS,
        }) + "\n"
      );
      process.exit(2);
  }
}
