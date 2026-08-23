#!/usr/bin/env bun
/**
 * scripts/agentic/index.ts
 *
 * Thin orchestrator over the lower-level agentic helpers.
 * Orchestrates common multi-step flows without hiding the underlying
 * proof receipts from each tool.
 *
 * Usage:
 *   bun scripts/agentic/index.ts <recipe> [--session NAME] [--key enter|tab] [--vision]
 *     [--target-json '{"type":"kind","kind":"agentChatDetached","index":0}'] [--surface agent_chat]
 *
 * Recipes:
 *   agent_chat-accept             Full Agent Chat picker accept; choose key with --key enter|tab
 *   agent_chat-enter-accept       Compatibility alias for --key enter
 *   agent_chat-tab-accept         Compatibility alias for --key tab
 *   agent_chat-detached-accept    One-command detached Agent Chat proof: resolve → accept → identity check
 *   agent_chat-open               Open Agent Chat and verify it reaches ready state
 *   agent_chat-setup-recovery     Recovery from Agent Chat setup state; select agent with --select-agent ID
 *   surface-navigate       Navigate known surfaces, safely interact, and capture image-library screenshots
 *   scenario               Run a replayable scenario with proof bundle (--scenario NAME --index N)
 *   vision-loop            Materialize visionCrops from a receipt into crop files + manifest
 *   check-leaks            Check for memory leaks using xctrace Leaks template
 *   preflight              Check all prerequisites (session, window, permissions)
 *   permission-assistant-drag-preflight-stress
 *                         Fail-closed Permission Assistant drag/no-TCC proof
 *   quick-terminal-pty-apply-back-stress
 *                         Fail-closed Quick Terminal PTY apply-back proof
 *   mcp-context-resource-attachment-identity-stress
 *                         Fail-closed MCP context resource identity proof
 *   settings-theme-hot-reload-stress
 *                         Fail-closed Settings/theme hot-reload proof
 *   file-search-drag-out-identity-stress
 *                         Fail-closed File Search drag-out identity proof
 *   scriptlet-bundle-execution-matrix-stress
 *                         Fail-closed scriptlet bundle execution matrix proof
 *   tray-global-hotkey-menu-mutation-stress
 *                         Fail-closed tray menu/global hotkey mutation proof
 *   multi-window-resize-monitor-restoration-stress
 *                         Fail-closed multi-window resize/monitor restoration proof
 *   agent_chat-targeted-dictation-delivery-stress
 *                         Fail-closed Agent Chat-targeted dictation delivery proof
 *   clipboard-share-trust-install-stress
 *                         Fail-closed clipboard share trust install proof
 *   clipboard-share-watcher-stale-replay-stress
 *                         Fail-closed clipboard watcher stale/replay proof
 *   permission-share-cross-prompt-focus-stress
 *                         Fail-closed permission/share cross-prompt focus proof
 *   visible-text-clipping-overlap-stress
 *                         Pass-now main-window visible text clipping/overlap visual proof
 *   layout-measurement-regression-stress
 *                         Pass-now main-window layout measurement regression proof
 *   screenshot-semantics-visual-consistency-stress
 *                         Pass-now screenshot-to-semantics consistency proof
 *   modal-stack-arbitration-stress
 *                         Fail-closed stacked modal key arbitration proof
 *   cross-surface-export-provenance-stress
 *                         Fail-closed cross-surface export provenance proof
 *   dev-session-recovery-stale-target-stress
 *                         Pass-now stale target recovery proof
 *   menu-syntax-ambiguity-diagnostics-stress
 *                         Fail-closed menu syntax ambiguity diagnostics proof
 *   ime-composition-input-boundary-stress
 *                         Fail-closed IME composition boundary proof
 *   accessibility-selected-text-fallback-stress
 *                         Fail-closed selected-text fallback proof
 *   display-migration-visual-bounds-stress
 *                         Fail-closed display migration visual bounds proof
 *   native-picker-external-return-focus-stress
 *                         Fail-closed native picker/external return focus proof
 *   drag-cancel-payload-scope-stress
 *                         Fail-closed drag cancellation payload scope proof
 *   runtime-appearance-churn-focused-input-stress
 *                         Fail-closed focused input appearance churn proof
 *   power-resume-window-generation-stress
 *                         Fail-closed power resume window generation proof
 *   menu-tray-notification-modal-interruption-stress
 *                         Fail-closed menu/tray/notification modal interruption proof
 *   stream-progress-cancel-visual-stability-stress
 *                         Fail-closed stream/progress cancellation visual stability proof
 *   dictation-media-permission-readiness-churn-stress
 *                         Fail-closed dictation/media permission readiness churn proof
 *   animation-frame-capture-determinism-stress
 *                         Fail-closed animation frame capture determinism proof
 *   accessibility-tree-semantic-parity-stress
 *                         Fail-closed accessibility tree semantic parity proof
 *   rtl-bidi-emoji-text-rendering-stress
 *                         Fail-closed RTL/bidi/emoji text rendering proof
 *   high-volume-virtualized-list-stability-stress
 *                         Fail-closed high-volume virtualized list stability proof
 *   input-modality-transition-ownership-stress
 *                         Fail-closed input-device modality transition ownership proof
 *   multi-context-attachment-dedupe-provenance-stress
 *                         Fail-closed multi-context attachment dedupe/provenance proof
 *   visual-contrast-readable-state-stress
 *                         Pass-now theme contrast/readable-state proof
 *   empty-error-retry-state-ux-stress
 *                         Fail-closed empty/error/retry state UX proof
 *   form-validation-inline-recovery-stress
 *                         Fail-closed form validation inline recovery proof
 *   navigation-back-stack-history-stress
 *                         Fail-closed navigation/back-stack history proof
 *   long-text-wrap-resize-surface-stress
 *                         Pass-now long text wrapping/resizing UX proof
 *   div-container-scroll-overflow-stress
 *                         Pass-now DivPrompt tall-container overflow layout proof
 *   main-menu-dynamic-choice-resize-stress
 *                         Pass-now dynamic choice-count window resize proof
 *   notes-window-resize-stress
 *                         Pass-now sandboxed Notes window grow/shrink resize proof
 *   notes-delete-confirm-route-proof
 *                         Pass-now sandboxed Notes delete confirm-popup route proof
 *   actions-command-discoverability-noop-stress
 *                         Pass-now actions popup row measurement proof
 *   dense-list-detail-preview-readability-stress
 *                         Fail-closed dense list/detail preview readability proof
 *   toast-notification-queue-lifecycle-stress
 *                         Fail-closed toast/notification queue lifecycle proof
 *   destructive-confirm-modal-safety-stress
 *                         Fail-closed destructive confirm dry-run safety proof
 *   confirm-modal-style-preview-proof
 *                         Fail-closed: Confirm Modal style-preview scenario owner is missing
 *   sdk-confirm-runtime-proof
 *                         Pass-now SDK confirm script route proof
 *   loading-skeleton-progress-restoration-stress
 *                         Fail-closed loading skeleton/progress restoration proof
 *   icon-image-fallback-redaction-stress
 *                         Fail-closed icon/image fallback redaction proof
 *   footer-status-persistence-stress
 *                         Fail-closed footer/status persistence proof
 *   keyboard-hint-label-parity-stress
 *                         Fail-closed keyboard hint label parity proof
 *   row-state-parity-without-pointer-stress
 *                         Fail-closed row state parity proof without native pointer input
 *   quiet-chrome-card-nesting-stress
 *                         Fail-closed quiet chrome/card nesting proof
 *   scroll-shadow-sticky-header-density-stress
 *                         Fail-closed scroll shadow/sticky header/density proof
 *   popup-focus-keycap-visual-semantics-stress
 *                         Fail-closed popup focus/keycap visual semantics proof
 *   reduced-motion-animation-disable-stress
 *                         Fail-closed reduced-motion animation disable proof
 *   command-search-highlighting-accessory-badges-stress
 *                         Fail-closed command search highlight/badge proof
 *   clipboard-copy-visual-feedback-stress
 *                         Fail-closed fixture-scoped copy visual feedback proof
 *   portal-cancel-return-state-restoration-stress
 *                         Fail-closed portal cancel/back return restoration proof
 *   tooltip-hover-focus-affordance-stress
 *                         Fail-closed tooltip hover/focus affordance proof
 *   shortcut-recorder-cancel-layering-stress
 *                         Fail-closed shortcut recorder cancel/layering proof
 *   inline-popover-anchor-resize-stress
 *                         Fail-closed inline popover anchor/resize proof
 *   disabled-footer-hit-target-refusal-stress
 *                         Fail-closed disabled footer hit-target refusal proof
 *   mini-full-transition-layout-continuity-stress
 *                         Fail-closed mini/full transition visual layout continuity proof
 *   filter-input-decoration-chip-layout-stress
 *                         Fail-closed filter input decoration chip layout proof
 *   focus-ring-viewport-integrity-stress
 *                         Fail-closed focus ring viewport integrity proof
 *   warning-banner-action-dismiss-semantics-stress
 *                         Fail-closed warning banner action/dismiss semantics proof
 *   select-prompt-multiselect-keyboard-state-stress
 *                         Fail-closed SelectPrompt keyboard multi-selection state proof
 *   file-search-preview-sanitization-stress
 *                         Fail-closed File Search safe preview sanitization proof
 *   hotkey-prompt-transient-capture-cancel-stress
 *                         Fail-closed HotkeyPrompt transient capture/cancel proof
 *   process-manager-sort-detail-panel-stability-stress
 *                         Fail-closed Process Manager sort/header/detail panel proof
 *   env-prompt-redacted-status-error-recovery-stress
 *                         Fail-closed EnvPrompt redacted status/error recovery proof
 *   command-palette-breadcrumb-route-stack-stress
 *                         Fail-closed command palette breadcrumb route-stack proof
 *   root-source-chip-action-semantics-stress
 *                         Fail-closed root source-chip action semantics proof
 *   recent-history-dedupe-root-grouping-stress
 *                         Fail-closed recent/history dedupe root grouping proof
 *   inline-attachment-preview-chip-stability-stress
 *                         Fail-closed inline attachment preview chip proof
 *   window-title-status-semantics-stress
 *                         Fail-closed window title/status semantics proof
 *   menu-syntax-capture-validation-chip-stress
 *                         Fail-closed menu syntax capture validation chip proof
 *   agent_chat-footer-activity-indicator-stress
 *                         Fail-closed Agent Chat footer activity indicator proof
 *   agent_chat-model-history-popover-visual-state-stress
 *                         Fail-closed Agent Chat model/history popover visual state proof
 *   agent_chat-context-insertion-preview-parity-stress
 *                         Fail-closed Agent Chat context insertion preview parity proof
 *   agent_chat-composer-token-keyboard-edit-parity-stress
 *                         Fail-closed Agent Chat composer token keyboard edit parity proof
 *   agent_chat-transcript-stream-retry-virtualization-stress
 *                         Fail-closed Agent Chat transcript stream retry virtualization proof
 *   agent_chat-plugin-skill-entry-thread-affinity-stress
 *                         Fail-closed Agent Chat plugin skill entry thread affinity proof
 *   notes-cart-agent_chat-handoff-dedupe-stress
 *                         Fail-closed Notes cart Agent Chat handoff dedupe proof
 *   root-file-source-filter-pagination-footer-stress
 *                         Fail-closed root file source-filter pagination footer proof
 *   file-search-directory-breadcrumb-restoration-stress
 *                         Fail-closed File Search breadcrumb restoration proof
 *   emoji-picker-skin-tone-category-ux-stress
 *                         Fail-closed Emoji Picker skin-tone/category UX proof
 *   root-window-source-filter-activation-refusal-stress
 *                         Fail-closed root Windows source-filter activation refusal proof
 *   notes-markdown-preview-scroll-sync-stress
 *                         Fail-closed Notes markdown preview scroll/focus sync proof
 *   quick-terminal-ansi-scrollback-search-stress
 *                         Fail-closed Quick Terminal ANSI scrollback search proof
 *   script-output-inspector-folding-recovery-stress
 *                         Fail-closed script output inspector folding recovery proof
 *   app-launcher-icon-grid-keyboard-navigation-stress
 *                         Fail-closed App Launcher icon-grid keyboard navigation proof
 *   browser-history-time-grouped-privacy-stress
 *                         Fail-closed Browser History time-grouped privacy proof
 *   settings-preferences-search-reset-preview-stress
 *                         Fail-closed Settings preferences search/reset preview proof
 *   settings-preferences-readonly-detail-panel-stress
 *                         Fail-closed Settings read-only detail panel proof
 *   design-picker-preview-restore-visual-stress
 *                         Fail-closed Design Picker preview restore visual proof
 *   dictation-history-transcript-preview-redaction-stress
 *                         Fail-closed Dictation History transcript preview redaction proof
 *   help                   Show this help
 *
 * Target threading:
 *   --target-json JSON   Agent Chat window target for all RPCs (getAgentChatState, getAgentChatTestProbe,
 *                        resetAgentChatTestProbe, waitFor). Reused consistently across all steps.
 *   --surface SURFACE    Automation surface for native input focus (main, agent_chat, actions, notes, ai).
 *                        Must match the --target-json window so focus and proof stay on the same surface.
 *
 * All output is JSON on stdout. Each recipe returns the underlying
 * tool receipts so the agent can inspect proof at every step.
 */

import { resolve } from "path";
import { assertNoninteractiveSubprocess } from "../devtools/lib/operator-safety.ts";
import {
  runAgentChatPortalRoundTripOriginStressScenario,
  runActionsCapturedSubjectFrameStressScenario,
  runBrowserTabsCacheIdentityStressScenario,
  runClipboardHistoryPortalRangeStressScenario,
  runCurrentAppCommandsFrontmostStressScenario,
  runDetachedAgentChatTargetThreadingStressScenario,
  runDropPromptNativeDropPrivacyStressScenario,
  runActionsDialogExactIdScenario,
  runDetachedAgentChatExactIdScenario,
  runMainWindowExactIdScenario,
  runNotesAgentChatDelayedActionOriginStressScenario,
  runPathPromptFilesystemEdgeStressScenario,
  runPermissionPreflightReadonlyScenario,
  runPermissionAssistantDragPreflightStressScenario,
  runPromptPopupExactIdScenario,
  runQuickTerminalPtyApplyBackStressScenario,
  runMcpContextResourceAttachmentIdentityStressScenario,
  runSettingsThemeHotReloadStressScenario,
  runFileSearchDragOutIdentityStressScenario,
  runScriptletBundleExecutionMatrixStressScenario,
  runTrayGlobalHotkeyMenuMutationStressScenario,
  runMultiWindowResizeMonitorRestorationStressScenario,
  runAgentChatTargetedDictationDeliveryStressScenario,
  runClipboardShareTrustInstallStressScenario,
  runClipboardShareWatcherStaleReplayStressScenario,
  runPermissionShareCrossPromptFocusStressScenario,
  runVisibleTextClippingOverlapStressScenario,
  runLayoutMeasurementRegressionStressScenario,
  runScreenshotSemanticsVisualConsistencyStressScenario,
  runModalStackArbitrationStressScenario,
  runCrossSurfaceExportProvenanceStressScenario,
  runDevSessionRecoveryStaleTargetStressScenario,
  runMenuSyntaxAmbiguityDiagnosticsStressScenario,
  runImeCompositionInputBoundaryStressScenario,
  runAccessibilitySelectedTextFallbackStressScenario,
  runDisplayMigrationVisualBoundsStressScenario,
  runNativePickerExternalReturnFocusStressScenario,
  runDragCancelPayloadScopeStressScenario,
  runRuntimeAppearanceChurnFocusedInputStressScenario,
  runPowerResumeWindowGenerationStressScenario,
  runMenuTrayNotificationModalInterruptionStressScenario,
  runStreamProgressCancelVisualStabilityStressScenario,
  runDictationMediaPermissionReadinessChurnStressScenario,
  runAnimationFrameCaptureDeterminismStressScenario,
  runAccessibilityTreeSemanticParityStressScenario,
  runRtlBidiEmojiTextRenderingStressScenario,
  runHighVolumeVirtualizedListStabilityStressScenario,
  runInputModalityTransitionOwnershipStressScenario,
  runMultiContextAttachmentDedupeProvenanceStressScenario,
  runVisualContrastReadableStateStressScenario,
  runEmptyErrorRetryStateUxStressScenario,
  runFormValidationInlineRecoveryStressScenario,
  runNavigationBackStackHistoryStressScenario,
  runLongTextWrapResizeSurfaceStressScenario,
  runDivContainerScrollOverflowStressScenario,
  runMainMenuDynamicChoiceResizeStressScenario,
  runNotesWindowResizeStressScenario,
  runNotesDeleteConfirmRouteProofScenario,
  runActionsCommandDiscoverabilityNoopStressScenario,
  runDenseListDetailPreviewReadabilityStressScenario,
  runToastNotificationQueueLifecycleStressScenario,
  runDestructiveConfirmModalSafetyStressScenario,
  runSdkConfirmRuntimeProofScenario,
  runLoadingSkeletonProgressRestorationStressScenario,
  runIconImageFallbackRedactionStressScenario,
  runFooterStatusPersistenceStressScenario,
  runKeyboardHintLabelParityStressScenario,
  runRowStateParityWithoutPointerStressScenario,
  runQuietChromeCardNestingStressScenario,
  runScrollShadowStickyHeaderDensityStressScenario,
  runPopupFocusKeycapVisualSemanticsStressScenario,
  runReducedMotionAnimationDisableStressScenario,
  runCommandSearchHighlightingAccessoryBadgesStressScenario,
  runClipboardCopyVisualFeedbackStressScenario,
  runPortalCancelReturnStateRestorationStressScenario,
  runTooltipHoverFocusAffordanceStressScenario,
  runShortcutRecorderCancelLayeringStressScenario,
  runInlinePopoverAnchorResizeStressScenario,
  runDisabledFooterHitTargetRefusalStressScenario,
  runMiniFullTransitionLayoutContinuityStressScenario,
  runFilterInputDecorationChipLayoutStressScenario,
  runFocusRingViewportIntegrityStressScenario,
  runHotkeyPromptTransientCaptureCancelStressScenario,
  runProcessManagerSortDetailPanelStabilityStressScenario,
  runEnvPromptRedactedStatusErrorRecoveryStressScenario,
  runCommandPaletteBreadcrumbRouteStackStressScenario,
  runRootSourceChipActionSemanticsStressScenario,
  runRecentHistoryDedupeRootGroupingStressScenario,
  runInlineAttachmentPreviewChipStabilityStressScenario,
  runWindowTitleStatusSemanticsStressScenario,
  runMenuSyntaxCaptureValidationChipStressScenario,
  runAgentChatFooterActivityIndicatorStressScenario,
  runAgentChatModelHistoryPopoverVisualStateStressScenario,
  runAgentChatContextInsertionPreviewParityStressScenario,
  runAgentChatComposerTokenKeyboardEditParityStressScenario,
  runAgentChatTranscriptStreamRetryVirtualizationStressScenario,
  runAgentChatPluginSkillEntryThreadAffinityStressScenario,
  runNotesCartAgentChatHandoffDedupeStressScenario,
  runRootFileSourceFilterPaginationFooterStressScenario,
  runFileSearchDirectoryBreadcrumbRestorationStressScenario,
  runEmojiPickerSkinToneCategoryUxStressScenario,
  runRootWindowSourceFilterActivationRefusalStressScenario,
  runNotesMarkdownPreviewScrollSyncStressScenario,
  runQuickTerminalAnsiScrollbackSearchStressScenario,
  runScriptOutputInspectorFoldingRecoveryStressScenario,
  runAppLauncherIconGridKeyboardNavigationStressScenario,
  runBrowserHistoryTimeGroupedPrivacyStressScenario,
  runSettingsPreferencesSearchResetPreviewStressScenario,
  runSettingsPreferencesReadonlyDetailPanelStressScenario,
  runDesignPickerPreviewRestoreVisualStressScenario,
  runDictationHistoryTranscriptPreviewRedactionStressScenario,
  runWarningBannerActionDismissSemanticsStressScenario,
  runSelectPromptMultiselectKeyboardStateStressScenario,
  runFileSearchPreviewSanitizationStressScenario,
  runScreenshotIdentityAgentChatContextStressScenario,
  runScrollSelectionReanchorStressScenario,
  runShortcutRecorderFocusCaptureStressScenario,
  runTemplatePromptAutomationParityStressScenario,
} from "./scenario";

const SCHEMA_VERSION = 1;
const PROJECT_ROOT = resolve(import.meta.dir, "../..");

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/**
 * Wire-compatible Agent Chat window target. Same shape as Rust `AutomationWindowTarget`.
 * One target object must be reused for every RPC in a single verification run.
 */
type AutomationTargetJson =
  | { type: "focused" }
  | { type: "main" }
  | { type: "id"; id: string }
  | { type: "kind"; kind: string; index?: number }
  | { type: "titleContains"; text: string };

interface RecipeReceipt {
  schemaVersion: number;
  recipe: string;
  status: "pass" | "fail" | "error";
  failClosed?: boolean;
  failureMode?: string;
  missingReceipt?: string;
  reasonCode?: string;
  linearIssue?: string;
  steps: StepReceipt[];
  summary: string;
  /** When --vision is requested, the final verify-shot proof bundle is surfaced here unchanged. */
  proofBundle?: unknown;
}

type SurfaceProofKind = "main" | "actionsDialog" | "promptPopup" | "agentChatDetached";

interface SurfaceProofUsage {
  stateFirst: true;
  usedGetState: boolean;
  usedGetElements: boolean;
  usedInspect: boolean;
  usedWaitFor: boolean;
  usedBatch: boolean;
  usedGpuiEvent: boolean;
  usedScreenshot: false;
  usedNativeInput: false;
  usedShow: false;
  usedFixedSleepMs: 0;
}

interface SurfaceProofCapabilities {
  state: boolean;
  elements: boolean;
  inspect: boolean;
  waitFor: boolean;
  batch: string[];
  gpuiEvent: boolean;
  nativeInputRequired: false;
  screenshotRequired: false;
}

/** Input delivery method chosen by the routing logic. */
type RoutedInputMethod = "batch" | "simulateGpuiEvent" | "native";
type RoutedInputMode = "auto" | "force-native" | "force-batch" | "force-gpui";

interface RoutedInputMetadata {
  inputMethod: RoutedInputMethod;
  resolvedWindowId?: string;
  dispatchPath?: "exact_handle" | "window_role_fallback";
}

interface StepReceipt {
  name: string;
  status: "pass" | "fail" | "error" | "skipped";
  output: unknown;
  durationMs: number;
  /** Present on steps that deliver input to a target surface. */
  inputMethod?: RoutedInputMethod;
  /** Present when inputMethod is "batch" or "simulateGpuiEvent". */
  resolvedWindowId?: string;
  /** Present when inputMethod is "batch" or "simulateGpuiEvent". */
  dispatchPath?: "exact_handle" | "window_role_fallback";
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function runTool(
  cmd: string[],
  _label: string
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  assertNoninteractiveSubprocess(cmd);
  const proc = Bun.spawn(cmd, {
    stdout: "pipe",
    stderr: "pipe",
    cwd: PROJECT_ROOT,
  });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const exitCode = await proc.exited;
  return { exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return { raw: text };
  }
}

async function step(
  name: string,
  fn: () => Promise<{ exitCode: number; stdout: string }>
): Promise<StepReceipt> {
  const start = Date.now();
  try {
    const { exitCode, stdout } = await fn();
    return {
      name,
      status: exitCode === 0 ? "pass" : exitCode === 2 ? "error" : "fail",
      output: parseJson(stdout),
      durationMs: Date.now() - start,
    };
  } catch (e: any) {
    return {
      name,
      status: "error",
      output: { error: e.message ?? String(e) },
      durationMs: Date.now() - start,
    };
  }
}

/**
 * Send a protocol command via session.sh rpc and return structured result.
 * Surfaces the full waitForResult / batchResult trace receipt on failure.
 */
async function rpc(
  session: string,
  jsonCmd: string,
  opts: { expect?: string; timeout?: number } = {}
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const args = [
    "bash",
    "scripts/agentic/session.sh",
    "rpc",
    session,
    jsonCmd,
  ];
  if (opts.expect) {
    args.push("--expect", opts.expect);
  }
  if (opts.timeout) {
    args.push("--timeout", String(opts.timeout));
  }
  return runTool(args, "rpc");
}

/**
 * Build a JSON command string, injecting `target` when present.
 */
function buildCmd(
  base: Record<string, unknown>,
  target?: AutomationTargetJson
): string {
  if (target) {
    return JSON.stringify({ ...base, target });
  }
  return JSON.stringify(base);
}

/**
 * Build native-input args with session, optional --surface, and optional --ensure-focus.
 * Always passes --session so macos-input.ts uses the capability ladder
 * (directBatch → gpuiDispatch → native fallback).
 *
 * When `skipEnsureFocus` is true, the `--ensure-focus` flag is omitted so
 * macos-input.ts will not attempt OS-level focus enforcement before trying
 * protocol-level and GPUI dispatch paths. This is the correct mode for
 * detached Agent Chat and picker targets that don't need foreground keyboard focus.
 */
function nativeInputArgs(
  command: string,
  value: string,
  session: string,
  surface?: string,
  opts?: { skipEnsureFocus?: boolean }
): string[] {
  const args = [
    "bun",
    "scripts/agentic/macos-input.ts",
    command,
    value,
  ];
  if (!opts?.skipEnsureFocus) {
    args.push("--ensure-focus");
  }
  args.push("--session", session);
  if (surface) {
    args.push("--target", surface);
  }
  return args;
}

/**
 * Build verify-shot args with optional --target-json.
 */
function verifyArgs(
  base: string[],
  target?: AutomationTargetJson
): string[] {
  if (target) {
    return [...base, "--target-json", JSON.stringify(target)];
  }
  return base;
}

/**
 * Post-show settle: wait until the window reports focused (frontmost)
 * via server-side waitFor instead of a fixed wall-clock sleep. Returns
 * as soon as the condition is met. Non-fatal by design — the follow-up
 * command is the authoritative failure point, matching the old sleep's
 * proceed-anyway semantics.
 */
async function settleWindowFocus(
  session: string,
  timeoutMs = 2000
): Promise<void> {
  try {
    await rpc(
      session,
      JSON.stringify({
        type: "waitFor",
        requestId: `w-window-focused-${Date.now()}`,
        condition: "windowFocused",
        timeout: timeoutMs,
        pollInterval: 25,
      }),
      { expect: "waitForResult", timeout: timeoutMs + 3000 }
    );
  } catch {
    // best-effort settle; proceed like the old fixed sleep did
  }
}

/**
 * Fire-and-forget send via session.sh send.
 */
async function send(
  session: string,
  jsonCmd: string
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  return runTool(
    ["bash", "scripts/agentic/session.sh", "send", session, jsonCmd],
    "send"
  );
}

/**
 * Decide how to deliver input to a target surface.
 *
 * Decision rule:
 *   - agentChatDetached, actionsDialog, promptPopup → "batch" (protocol-level, no OS focus needed)
 *   - Exact ID targets → "batch"
 *   - main/focused/unspecified → "native" (OS-level input via macos-input.ts)
 */
function chooseInputMethod(
  target?: AutomationTargetJson,
  mode: RoutedInputMode = "auto"
): RoutedInputMethod {
  if (mode === "force-native") return "native";
  if (mode === "force-batch") return "batch";
  if (mode === "force-gpui") return "simulateGpuiEvent";
  if (!target) return "native";
  if (target.type === "id") return "batch";
  if (target.type === "kind") {
    if (
      target.kind === "agentChatDetached" ||
      target.kind === "actionsDialog" ||
      target.kind === "promptPopup"
    ) {
      return "batch";
    }
  }
  return "native";
}

/**
 * Send text via protocol-level batch setInput command.
 * Returns the RPC result with routing metadata.
 */
async function batchSetInput(
  session: string,
  text: string,
  target: AutomationTargetJson
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const cmd = buildCmd(
    {
      type: "batch",
      requestId: `txn-setInput-${Date.now()}`,
      commands: [
        { type: "setInput", text },
      ],
      trace: "onFailure",
    },
    target
  );
  return rpc(session, cmd, { expect: "batchResult", timeout: 5000 });
}

/**
 * Send a key via simulateGpuiEvent when batch cannot express the input.
 * Returns the RPC result with routing metadata.
 */
async function gpuiKeyDispatch(
  session: string,
  key: string,
  target: AutomationTargetJson,
  modifiers: string[] = []
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const cmd = buildCmd(
    {
      type: "simulateGpuiEvent",
      requestId: `gpui-key-${key}-${Date.now()}`,
      event: { type: "keyDown", key, modifiers },
    },
    target
  );
  return rpc(session, cmd, { expect: "simulateGpuiEventResult", timeout: 5000 });
}

/**
 * Build a routed step: choose batch/GPUI/native based on target, execute, and
 * attach inputMethod metadata to the StepReceipt.
 */
async function routedInputStep(
  name: string,
  kind: "type" | "key",
  value: string,
  session: string,
  opts: {
    target?: AutomationTargetJson;
    surface?: string;
    modifiers?: string[];
    inputMode?: RoutedInputMode;
  } = {}
): Promise<StepReceipt> {
  const method = chooseInputMethod(opts.target, opts.inputMode ?? "auto");
  const start = Date.now();

  try {
    let result: { exitCode: number; stdout: string; stderr: string };
    let resolvedWindowId: string | undefined;
    let dispatchPath: "exact_handle" | "window_role_fallback" | undefined;

    if (method === "batch" && kind === "type" && opts.target) {
      result = await batchSetInput(session, value, opts.target);
      resolvedWindowId = opts.target.type === "id" ? opts.target.id : undefined;
      dispatchPath = opts.target.type === "id" ? "exact_handle" : "window_role_fallback";
    } else if (method === "batch" && kind === "key" && opts.target) {
      // batch cannot express arbitrary keys; fall through to simulateGpuiEvent
      result = await gpuiKeyDispatch(session, value, opts.target, opts.modifiers);
      resolvedWindowId = opts.target.type === "id" ? opts.target.id : undefined;
      dispatchPath = opts.target.type === "id" ? "exact_handle" : "window_role_fallback";
      // Override method to reflect actual dispatch
      return {
        name,
        status: result.exitCode === 0 ? "pass" : result.exitCode === 2 ? "error" : "fail",
        output: parseJson(result.stdout),
        durationMs: Date.now() - start,
        inputMethod: "simulateGpuiEvent",
        resolvedWindowId,
        dispatchPath,
      };
    } else {
      // Native fallback: use macos-input.ts
      const isNonMainTarget = opts.target && !isMainLikeTarget(opts.target);
      const args = nativeInputArgs(kind, value, session, opts.surface, {
        skipEnsureFocus: opts.inputMode === "force-native" ? false : isNonMainTarget,
      });
      result = await runTool(args, name);
      return {
        name,
        status: result.exitCode === 0 ? "pass" : result.exitCode === 2 ? "error" : "fail",
        output: parseJson(result.stdout),
        durationMs: Date.now() - start,
        inputMethod: "native",
      };
    }

    return {
      name,
      status: result.exitCode === 0 ? "pass" : result.exitCode === 2 ? "error" : "fail",
      output: parseJson(result.stdout),
      durationMs: Date.now() - start,
      inputMethod: method,
      resolvedWindowId,
      dispatchPath,
    };
  } catch (e: any) {
    return {
      name,
      status: "error",
      output: { error: e.message ?? String(e) },
      durationMs: Date.now() - start,
      inputMethod: method,
    };
  }
}

function parseTargetJson(raw: string | undefined): AutomationTargetJson | undefined {
  if (!raw) return undefined;
  try {
    return JSON.parse(raw) as AutomationTargetJson;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`Invalid --target-json: ${reason}`);
  }
}

function parseArgs() {
  const args = process.argv.slice(2);
  const recipe = args[0] ?? "help";
  const sessionIdx = args.indexOf("--session");
  const session =
    sessionIdx >= 0 && args[sessionIdx + 1] ? args[sessionIdx + 1] : "default";
  const keyIdx = args.indexOf("--key");
  const key =
    keyIdx >= 0 &&
    (args[keyIdx + 1] === "enter" || args[keyIdx + 1] === "tab")
      ? (args[keyIdx + 1] as "enter" | "tab")
      : "enter";
  const vision = args.includes("--vision");
  const selectAgentIdx = args.indexOf("--select-agent");
  const selectAgent =
    selectAgentIdx >= 0 && args[selectAgentIdx + 1]
      ? args[selectAgentIdx + 1]
      : undefined;
  const targetJsonIdx = args.indexOf("--target-json");
  const targetJson = parseTargetJson(
    targetJsonIdx >= 0 ? args[targetJsonIdx + 1] : undefined
  );
  const surfaceIdx = args.indexOf("--surface");
  const surface =
    surfaceIdx >= 0 && args[surfaceIdx + 1] ? args[surfaceIdx + 1] : undefined;
  const json = args.includes("--json");
  const kindIdx = args.indexOf("--kind");
  const kind = kindIdx >= 0 && args[kindIdx + 1] ? args[kindIdx + 1] : undefined;
  const indexIdx = args.indexOf("--index");
  const rawIndex = indexIdx >= 0 ? args[indexIdx + 1] : undefined;
  if (rawIndex != null) {
    const parsedIndex = Number(rawIndex);
    if (!Number.isInteger(parsedIndex) || parsedIndex < 0) {
      throw new Error(`Invalid --index: expected non-negative integer, got ${rawIndex}`);
    }
  }
  const index = rawIndex != null ? Number(rawIndex) : undefined;
  const minTargetsIdx = args.indexOf("--min-targets");
  const rawMinTargets = minTargetsIdx >= 0 ? args[minTargetsIdx + 1] : undefined;
  const minTargets =
    rawMinTargets != null && Number.isInteger(Number(rawMinTargets))
      ? Number(rawMinTargets)
      : undefined;
  const familyIdx = args.indexOf("--family");
  const family =
    familyIdx >= 0 && args[familyIdx + 1] ? args[familyIdx + 1] : undefined;
  const familiesIdx = args.indexOf("--families");
  const families =
    familiesIdx >= 0 && args[familiesIdx + 1]
      ? args[familiesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const driftIdx = args.indexOf("--drift");
  const drift = driftIdx >= 0 && args[driftIdx + 1] ? args[driftIdx + 1] : undefined;
  const hostIdx = args.indexOf("--host");
  const originIdx = args.indexOf("--origin");
  const host =
    originIdx >= 0 && args[originIdx + 1]
      ? args[originIdx + 1]
      : hostIdx >= 0 && args[hostIdx + 1] ? args[hostIdx + 1] : undefined;
  const portalIdx = args.indexOf("--portal");
  const portal = portalIdx >= 0 && args[portalIdx + 1] ? args[portalIdx + 1] : undefined;
  const selectionIdx = args.indexOf("--selection");
  const selection =
    selectionIdx >= 0 && args[selectionIdx + 1] ? args[selectionIdx + 1] : undefined;
  const queryIdx = args.indexOf("--query");
  const query = queryIdx >= 0 && args[queryIdx + 1] ? args[queryIdx + 1] : undefined;
  const kindsIdx = args.indexOf("--kinds");
  const kinds =
    kindsIdx >= 0 && args[kindsIdx + 1]
      ? args[kindsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const chordIdx = args.indexOf("--chord");
  const chord = chordIdx >= 0 && args[chordIdx + 1] ? args[chordIdx + 1] : undefined;
  const actionIdx = args.indexOf("--action");
  const action = actionIdx >= 0 && args[actionIdx + 1] ? args[actionIdx + 1] : undefined;
  const templateIdx = args.indexOf("--template");
  const template =
    templateIdx >= 0 && args[templateIdx + 1] ? args[templateIdx + 1] : undefined;
  const fieldIdx = args.indexOf("--field");
  const field = fieldIdx >= 0 && args[fieldIdx + 1] ? args[fieldIdx + 1] : undefined;
  const valueIdx = args.indexOf("--value");
  const value = valueIdx >= 0 && args[valueIdx + 1] ? args[valueIdx + 1] : undefined;
  const forcedValueIdx = args.indexOf("--forced-value");
  const forcedValue =
    forcedValueIdx >= 0 && args[forcedValueIdx + 1] ? args[forcedValueIdx + 1] : undefined;
  const aliasIdx = args.indexOf("--alias");
  const alias = aliasIdx >= 0 && args[aliasIdx + 1] ? args[aliasIdx + 1] : undefined;
  const expectedAppIdx = args.indexOf("--expected-app");
  const expectedApp =
    expectedAppIdx >= 0 && args[expectedAppIdx + 1] ? args[expectedAppIdx + 1] : undefined;
  const sourceIdx = args.indexOf("--source");
  const source = sourceIdx >= 0 && args[sourceIdx + 1] ? args[sourceIdx + 1] : undefined;
  const mutationIdx = args.indexOf("--mutation");
  const mutation =
    mutationIdx >= 0 && args[mutationIdx + 1] ? args[mutationIdx + 1] : undefined;
  const fileNameIdx = args.indexOf("--file-name");
  const fileName =
    fileNameIdx >= 0 && args[fileNameIdx + 1] ? args[fileNameIdx + 1] : undefined;
  const sizeIdx = args.indexOf("--size");
  const rawSize = sizeIdx >= 0 && args[sizeIdx + 1] ? Number(args[sizeIdx + 1]) : undefined;
  const size = Number.isFinite(rawSize) ? rawSize : undefined;
  const portalIdIdx = args.indexOf("--portal-id");
  const portalId = portalIdIdx >= 0 && args[portalIdIdx + 1] ? args[portalIdIdx + 1] : undefined;
  const rangeIdx = args.indexOf("--range");
  const range = rangeIdx >= 0 && args[rangeIdx + 1] ? args[rangeIdx + 1] : undefined;
  const paneIdx = args.indexOf("--pane");
  const pane = paneIdx >= 0 && args[paneIdx + 1] ? args[paneIdx + 1] : undefined;
  const bundleIdIdx = args.indexOf("--bundle-id");
  const bundleId =
    bundleIdIdx >= 0 && args[bundleIdIdx + 1] ? args[bundleIdIdx + 1] : undefined;
  const commandIdx = args.indexOf("--command");
  const command =
    commandIdx >= 0 && args[commandIdx + 1] ? args[commandIdx + 1] : undefined;
  const resourceUriIdx = args.indexOf("--resource-uri");
  const resourceUri =
    resourceUriIdx >= 0 && args[resourceUriIdx + 1] ? args[resourceUriIdx + 1] : undefined;
  const profileIdx = args.indexOf("--profile");
  const profile =
    profileIdx >= 0 && args[profileIdx + 1] ? args[profileIdx + 1] : undefined;
  const themeBeforeIdx = args.indexOf("--theme-before");
  const themeBefore =
    themeBeforeIdx >= 0 && args[themeBeforeIdx + 1] ? args[themeBeforeIdx + 1] : undefined;
  const themeAfterIdx = args.indexOf("--theme-after");
  const themeAfter =
    themeAfterIdx >= 0 && args[themeAfterIdx + 1] ? args[themeAfterIdx + 1] : undefined;
  const configKeyIdx = args.indexOf("--config-key");
  const configKey =
    configKeyIdx >= 0 && args[configKeyIdx + 1] ? args[configKeyIdx + 1] : undefined;
  const dropTargetIdx = args.indexOf("--drop-target");
  const dropTarget =
    dropTargetIdx >= 0 && args[dropTargetIdx + 1] ? args[dropTargetIdx + 1] : undefined;
  const scriptletIdIdx = args.indexOf("--scriptlet-id");
  const scriptletId =
    scriptletIdIdx >= 0 && args[scriptletIdIdx + 1] ? args[scriptletIdIdx + 1] : undefined;
  const cancelAfterMsIdx = args.indexOf("--cancel-after-ms");
  const rawCancelAfterMs =
    cancelAfterMsIdx >= 0 && args[cancelAfterMsIdx + 1]
      ? Number(args[cancelAfterMsIdx + 1])
      : undefined;
  const cancelAfterMs = Number.isFinite(rawCancelAfterMs) ? rawCancelAfterMs : undefined;
  const sandboxConfig = args.includes("--sandbox-config");
  const loopsIdx = args.indexOf("--loops");
  const rawLoops = loopsIdx >= 0 ? args[loopsIdx + 1] : undefined;
  const loops =
    rawLoops != null && Number.isInteger(Number(rawLoops)) && Number(rawLoops) > 0
      ? Number(rawLoops)
      : undefined;
  const surfacesIdx = args.indexOf("--surfaces");
  const surfaces =
    surfacesIdx >= 0 && args[surfacesIdx + 1]
      ? args[surfacesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const groupIdx = args.indexOf("--group");
  const group = groupIdx >= 0 && args[groupIdx + 1] ? args[groupIdx + 1] : undefined;
  const caseIdx = args.indexOf("--case");
  const caseId = caseIdx >= 0 && args[caseIdx + 1] ? args[caseIdx + 1] : undefined;
  const destinationIdx = args.indexOf("--destination");
  const destination =
    destinationIdx >= 0 && args[destinationIdx + 1] ? args[destinationIdx + 1] : undefined;
  const exportModeIdx = args.indexOf("--export-mode");
  const exportMode =
    exportModeIdx >= 0 && args[exportModeIdx + 1] ? args[exportModeIdx + 1] : undefined;
  const entryIdx = args.indexOf("--entry");
  const entry = entryIdx >= 0 && args[entryIdx + 1] ? args[entryIdx + 1] : undefined;
  const restartModeIdx = args.indexOf("--restart-mode");
  const restartMode =
    restartModeIdx >= 0 && args[restartModeIdx + 1] ? args[restartModeIdx + 1] : undefined;
  const monitorProfileIdx = args.indexOf("--monitor-profile");
  const monitorProfile =
    monitorProfileIdx >= 0 && args[monitorProfileIdx + 1]
      ? args[monitorProfileIdx + 1]
      : undefined;
  const transcriptIdx = args.indexOf("--transcript");
  const transcript =
    transcriptIdx >= 0 && args[transcriptIdx + 1] ? args[transcriptIdx + 1] : undefined;
  const fixtureIdIdx = args.indexOf("--fixture-id");
  const fixtureId =
    fixtureIdIdx >= 0 && args[fixtureIdIdx + 1] ? args[fixtureIdIdx + 1] : undefined;
  const shareKindIdx = args.indexOf("--share-kind");
  const shareKind =
    shareKindIdx >= 0 && args[shareKindIdx + 1] ? args[shareKindIdx + 1] : undefined;
  const acceptModeIdx = args.indexOf("--accept-mode");
  const acceptMode =
    acceptModeIdx >= 0 && args[acceptModeIdx + 1] ? args[acceptModeIdx + 1] : undefined;
  const countIdx = args.indexOf("--count");
  const rawCount = countIdx >= 0 && args[countIdx + 1] ? Number(args[countIdx + 1]) : undefined;
  const count = Number.isFinite(rawCount) ? rawCount : undefined;
  const burstMsIdx = args.indexOf("--burst-ms");
  const rawBurstMs =
    burstMsIdx >= 0 && args[burstMsIdx + 1] ? Number(args[burstMsIdx + 1]) : undefined;
  const burstMs = Number.isFinite(rawBurstMs) ? rawBurstMs : undefined;
  const fromDisplayIdx = args.indexOf("--from-display");
  const fromDisplay =
    fromDisplayIdx >= 0 && args[fromDisplayIdx + 1] ? args[fromDisplayIdx + 1] : undefined;
  const toDisplayIdx = args.indexOf("--to-display");
  const toDisplay =
    toDisplayIdx >= 0 && args[toDisplayIdx + 1] ? args[toDisplayIdx + 1] : undefined;
  const handoffIdx = args.indexOf("--handoff");
  const handoff =
    handoffIdx >= 0 && args[handoffIdx + 1] ? args[handoffIdx + 1] : undefined;
  const foreignAppIdx = args.indexOf("--foreign-app");
  const foreignApp =
    foreignAppIdx >= 0 && args[foreignAppIdx + 1] ? args[foreignAppIdx + 1] : undefined;
  const hoverTargetIdx = args.indexOf("--hover-target");
  const hoverTarget =
    hoverTargetIdx >= 0 && args[hoverTargetIdx + 1] ? args[hoverTargetIdx + 1] : undefined;
  const cancelIdx = args.indexOf("--cancel");
  const cancel = cancelIdx >= 0 && args[cancelIdx + 1] ? args[cancelIdx + 1] : undefined;
  const churnIdx = args.indexOf("--churn");
  const churn =
    churnIdx >= 0 && args[churnIdx + 1]
      ? args[churnIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const cyclesIdx = args.indexOf("--cycles");
  const rawCycles = cyclesIdx >= 0 && args[cyclesIdx + 1] ? Number(args[cyclesIdx + 1]) : undefined;
  const cycles = Number.isFinite(rawCycles) ? rawCycles : undefined;
  const eventIdx = args.indexOf("--event");
  const event = eventIdx >= 0 && args[eventIdx + 1] ? args[eventIdx + 1] : undefined;
  const activeSurfaceIdx = args.indexOf("--active-surface");
  const activeSurface =
    activeSurfaceIdx >= 0 && args[activeSurfaceIdx + 1]
      ? args[activeSurfaceIdx + 1]
      : undefined;
  const interruptionsIdx = args.indexOf("--interruptions");
  const interruptions =
    interruptionsIdx >= 0 && args[interruptionsIdx + 1]
      ? args[interruptionsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const updatesIdx = args.indexOf("--updates");
  const rawUpdates = updatesIdx >= 0 && args[updatesIdx + 1] ? Number(args[updatesIdx + 1]) : undefined;
  const updates = Number.isFinite(rawUpdates) ? rawUpdates : undefined;
  const cancelAtIdx = args.indexOf("--cancel-at");
  const rawCancelAt =
    cancelAtIdx >= 0 && args[cancelAtIdx + 1] ? Number(args[cancelAtIdx + 1]) : undefined;
  const cancelAt = Number.isFinite(rawCancelAt) ? rawCancelAt : undefined;
  const targetIdx = args.indexOf("--target");
  const target = targetIdx >= 0 && args[targetIdx + 1] ? args[targetIdx + 1] : undefined;
  const framesIdx = args.indexOf("--frames");
  const rawFrames = framesIdx >= 0 && args[framesIdx + 1] ? Number(args[framesIdx + 1]) : undefined;
  const frames = Number.isFinite(rawFrames) ? rawFrames : undefined;
  const intervalMsIdx = args.indexOf("--interval-ms");
  const rawIntervalMs =
    intervalMsIdx >= 0 && args[intervalMsIdx + 1] ? Number(args[intervalMsIdx + 1]) : undefined;
  const intervalMs = Number.isFinite(rawIntervalMs) ? rawIntervalMs : undefined;
  const textIdx = args.indexOf("--text");
  const text = textIdx >= 0 && args[textIdx + 1] ? args[textIdx + 1] : undefined;
  const fixtureCountIdx = args.indexOf("--fixture-count");
  const rawFixtureCount =
    fixtureCountIdx >= 0 && args[fixtureCountIdx + 1] ? Number(args[fixtureCountIdx + 1]) : undefined;
  const fixtureCount = Number.isFinite(rawFixtureCount) ? rawFixtureCount : undefined;
  const filterCyclesIdx = args.indexOf("--filter-cycles");
  const rawFilterCycles =
    filterCyclesIdx >= 0 && args[filterCyclesIdx + 1] ? Number(args[filterCyclesIdx + 1]) : undefined;
  const filterCycles = Number.isFinite(rawFilterCycles) ? rawFilterCycles : undefined;
  const scrollCyclesIdx = args.indexOf("--scroll-cycles");
  const rawScrollCycles =
    scrollCyclesIdx >= 0 && args[scrollCyclesIdx + 1] ? Number(args[scrollCyclesIdx + 1]) : undefined;
  const scrollCycles = Number.isFinite(rawScrollCycles) ? rawScrollCycles : undefined;
  const interleaveIdx = args.indexOf("--interleave");
  const interleave =
    interleaveIdx >= 0 && args[interleaveIdx + 1]
      ? args[interleaveIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const originsIdx = args.indexOf("--origins");
  const origins =
    originsIdx >= 0 && args[originsIdx + 1]
      ? args[originsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const destinationsIdx = args.indexOf("--destinations");
  const destinations =
    destinationsIdx >= 0 && args[destinationsIdx + 1]
      ? args[destinationsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const reorderCyclesIdx = args.indexOf("--reorder-cycles");
  const rawReorderCycles =
    reorderCyclesIdx >= 0 && args[reorderCyclesIdx + 1] ? Number(args[reorderCyclesIdx + 1]) : undefined;
  const reorderCycles = Number.isFinite(rawReorderCycles) ? rawReorderCycles : undefined;
  const themesIdx = args.indexOf("--themes");
  const themes =
    themesIdx >= 0 && args[themesIdx + 1]
      ? args[themesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const scaleFactorsIdx = args.indexOf("--scale-factors");
  const scaleFactors =
    scaleFactorsIdx >= 0 && args[scaleFactorsIdx + 1]
      ? args[scaleFactorsIdx + 1].split(",").map((s) => Number(s.trim())).filter(Number.isFinite)
      : undefined;
  const statesIdx = args.indexOf("--states");
  const states =
    statesIdx >= 0 && args[statesIdx + 1]
      ? args[statesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const retryCyclesIdx = args.indexOf("--retry-cycles");
  const rawRetryCycles =
    retryCyclesIdx >= 0 && args[retryCyclesIdx + 1] ? Number(args[retryCyclesIdx + 1]) : undefined;
  const retryCycles = Number.isFinite(rawRetryCycles) ? rawRetryCycles : undefined;
  const fieldsIdx = args.indexOf("--fields");
  const fields =
    fieldsIdx >= 0 && args[fieldsIdx + 1]
      ? args[fieldsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const invalidIdx = args.indexOf("--invalid");
  const invalid =
    invalidIdx >= 0 && args[invalidIdx + 1]
      ? args[invalidIdx + 1].split(",").map((s) => s.trim())
      : undefined;
  const validIdx = args.indexOf("--valid");
  const valid =
    validIdx >= 0 && args[validIdx + 1]
      ? args[validIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const originSurfaceIdx = args.indexOf("--origin");
  const originSurface =
    originSurfaceIdx >= 0 && args[originSurfaceIdx + 1] ? args[originSurfaceIdx + 1] : undefined;
  const transitionsIdx = args.indexOf("--transitions");
  const transitions =
    transitionsIdx >= 0 && args[transitionsIdx + 1]
      ? args[transitionsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const widthsIdx = args.indexOf("--widths");
  const widths =
    widthsIdx >= 0 && args[widthsIdx + 1]
      ? args[widthsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const fixtureIdx = args.indexOf("--fixture");
  const fixture =
    fixtureIdx >= 0 && args[fixtureIdx + 1] ? args[fixtureIdx + 1] : undefined;
  const fixturesIdx = args.indexOf("--fixtures");
  const fixtures =
    fixturesIdx >= 0 && args[fixturesIdx + 1]
      ? args[fixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const pathsIdx = args.indexOf("--paths");
  const paths =
    pathsIdx >= 0 && args[pathsIdx + 1]
      ? args[pathsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const dryRunOnly = args.includes("--dry-run-only");
  const chromeIdx = args.indexOf("--chrome");
  const chrome = chromeIdx >= 0 && args[chromeIdx + 1] ? args[chromeIdx + 1] : undefined;
  const scrollPositionsIdx = args.indexOf("--scroll-positions");
  const scrollPositions =
    scrollPositionsIdx >= 0 && args[scrollPositionsIdx + 1]
      ? args[scrollPositionsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const densityIdx = args.indexOf("--density");
  const density =
    densityIdx >= 0 && args[densityIdx + 1]
      ? args[densityIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const hostsIdx = args.indexOf("--hosts");
  const hosts =
    hostsIdx >= 0 && args[hostsIdx + 1]
      ? args[hostsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const selectionCyclesIdx = args.indexOf("--selection-cycles");
  const rawSelectionCycles =
    selectionCyclesIdx >= 0 && args[selectionCyclesIdx + 1] ? Number(args[selectionCyclesIdx + 1]) : undefined;
  const selectionCycles = Number.isFinite(rawSelectionCycles) ? rawSelectionCycles : undefined;
  const resizeCyclesIdx = args.indexOf("--resize-cycles");
  const rawResizeCycles =
    resizeCyclesIdx >= 0 && args[resizeCyclesIdx + 1] ? Number(args[resizeCyclesIdx + 1]) : undefined;
  const resizeCycles = Number.isFinite(rawResizeCycles) ? rawResizeCycles : undefined;
  const itemCountIdx = args.indexOf("--item-count");
  const rawItemCount =
    itemCountIdx >= 0 && args[itemCountIdx + 1] ? Number(args[itemCountIdx + 1]) : undefined;
  const itemCount = Number.isFinite(rawItemCount) ? rawItemCount : undefined;
  const smallCountIdx = args.indexOf("--small-count");
  const rawSmallCount =
    smallCountIdx >= 0 && args[smallCountIdx + 1] ? Number(args[smallCountIdx + 1]) : undefined;
  const smallCount = Number.isFinite(rawSmallCount) ? rawSmallCount : undefined;
  const largeCountIdx = args.indexOf("--large-count");
  const rawLargeCount =
    largeCountIdx >= 0 && args[largeCountIdx + 1] ? Number(args[largeCountIdx + 1]) : undefined;
  const largeCount = Number.isFinite(rawLargeCount) ? rawLargeCount : undefined;
  const shortLineCountIdx = args.indexOf("--short-line-count");
  const rawShortLineCount =
    shortLineCountIdx >= 0 && args[shortLineCountIdx + 1] ? Number(args[shortLineCountIdx + 1]) : undefined;
  const shortLineCount = Number.isFinite(rawShortLineCount) ? rawShortLineCount : undefined;
  const tallLineCountIdx = args.indexOf("--tall-line-count");
  const rawTallLineCount =
    tallLineCountIdx >= 0 && args[tallLineCountIdx + 1] ? Number(args[tallLineCountIdx + 1]) : undefined;
  const tallLineCount = Number.isFinite(rawTallLineCount) ? rawTallLineCount : undefined;
  const pasteboardScopeIdx = args.indexOf("--pasteboard-scope");
  const pasteboardScope =
    pasteboardScopeIdx >= 0 && args[pasteboardScopeIdx + 1] ? args[pasteboardScopeIdx + 1] : undefined;
  const noSystemPasteboard = args.includes("--no-system-pasteboard");
  const cancelMethodsIdx = args.indexOf("--cancel-methods");
  const cancelMethods =
    cancelMethodsIdx >= 0 && args[cancelMethodsIdx + 1]
      ? args[cancelMethodsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noNativePicker = args.includes("--no-native-picker");
  const targetsIdx = args.indexOf("--targets");
  const targets =
    targetsIdx >= 0 && args[targetsIdx + 1]
      ? args[targetsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const inputModesIdx = args.indexOf("--input-modes");
  const inputModes =
    inputModesIdx >= 0 && args[inputModesIdx + 1]
      ? args[inputModesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noNativePointer = args.includes("--no-native-pointer");
  const noConfigWrite = args.includes("--no-config-write");
  const noNativeInput = args.includes("--no-native-input");
  const localFixtureOnly = args.includes("--local-fixture-only");
  const noSubmit = args.includes("--no-submit");
  const stepsIdx = args.indexOf("--steps");
  const proofSteps =
    stepsIdx >= 0 && args[stepsIdx + 1]
      ? args[stepsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const queriesIdx = args.indexOf("--queries");
  const queries =
    queriesIdx >= 0 && args[queriesIdx + 1]
      ? args[queriesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const choicesIdx = args.indexOf("--choices");
  const rawChoices = choicesIdx >= 0 && args[choicesIdx + 1] ? Number(args[choicesIdx + 1]) : undefined;
  const choices = Number.isFinite(rawChoices) ? rawChoices : undefined;
  const selectionStepsIdx = args.indexOf("--selection-steps");
  const selectionSteps =
    selectionStepsIdx >= 0 && args[selectionStepsIdx + 1]
      ? args[selectionStepsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const previewFixturesIdx = args.indexOf("--preview-fixtures");
  const previewFixtures =
    previewFixturesIdx >= 0 && args[previewFixturesIdx + 1]
      ? args[previewFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noQuickLook = args.includes("--no-quick-look");
  const chordsIdx = args.indexOf("--chords");
  const chords =
    chordsIdx >= 0 && args[chordsIdx + 1]
      ? args[chordsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const sortKeysIdx = args.indexOf("--sort-keys");
  const sortKeys =
    sortKeysIdx >= 0 && args[sortKeysIdx + 1]
      ? args[sortKeysIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const statusFixturesIdx = args.indexOf("--status-fixtures");
  const statusFixtures =
    statusFixturesIdx >= 0 && args[statusFixturesIdx + 1]
      ? args[statusFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noProcessKill = args.includes("--no-process-kill");
  const noGlobalHotkeyRegistration = args.includes("--no-global-hotkey-registration");
  const noSecretWrite = args.includes("--no-secret-write");
  const drillPathIdx = args.indexOf("--drill-path");
  const drillPath =
    drillPathIdx >= 0 && args[drillPathIdx + 1]
      ? args[drillPathIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const filterIdx = args.indexOf("--filter");
  const filter = filterIdx >= 0 && args[filterIdx + 1] ? args[filterIdx + 1] : undefined;
  const backMethodsIdx = args.indexOf("--back-methods");
  const backMethods =
    backMethodsIdx >= 0 && args[backMethodsIdx + 1]
      ? args[backMethodsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const chipActionsIdx = args.includes("--chip-actions")
    ? args.indexOf("--chip-actions")
    : args.indexOf("--actions");
  const chipActions =
    chipActionsIdx >= 0 && args[chipActionsIdx + 1]
      ? args[chipActionsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const sourcesIdx = args.indexOf("--sources");
  const sources =
    sourcesIdx >= 0 && args[sourcesIdx + 1]
      ? args[sourcesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noNetwork = args.includes("--no-network");
  const casesIdx = args.indexOf("--cases");
  const cases =
    casesIdx >= 0 && args[casesIdx + 1]
      ? args[casesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noScreenCapture = args.includes("--no-screen-capture");
  const activityFixturesIdx = args.indexOf("--activity-fixtures");
  const activityFixtures =
    activityFixturesIdx >= 0 && args[activityFixturesIdx + 1]
      ? args[activityFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const agentFixtureIdx = args.indexOf("--agent-fixture");
  const agentFixture =
    agentFixtureIdx >= 0 && args[agentFixtureIdx + 1] ? args[agentFixtureIdx + 1] : undefined;
  const insertModesIdx = args.indexOf("--insert-modes");
  const insertModes =
    insertModesIdx >= 0 && args[insertModesIdx + 1]
      ? args[insertModesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noSecurityPrompts = args.includes("--no-security-prompts");
  const providersIdx = args.indexOf("--providers");
  const providers =
    providersIdx >= 0 && args[providersIdx + 1]
      ? args[providersIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const tokenKindsIdx = args.indexOf("--token-kinds");
  const tokenKinds =
    tokenKindsIdx >= 0 && args[tokenKindsIdx + 1]
      ? args[tokenKindsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const editStepsIdx = args.indexOf("--edit-steps");
  const editSteps =
    editStepsIdx >= 0 && args[editStepsIdx + 1]
      ? args[editStepsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const messageCountIdx = args.indexOf("--message-count");
  const rawMessageCount =
    messageCountIdx >= 0 && args[messageCountIdx + 1]
      ? Number(args[messageCountIdx + 1])
      : undefined;
  const messageCount = Number.isFinite(rawMessageCount) ? rawMessageCount : undefined;
  const streamChunksIdx = args.indexOf("--stream-chunks");
  const rawStreamChunks =
    streamChunksIdx >= 0 && args[streamChunksIdx + 1]
      ? Number(args[streamChunksIdx + 1])
      : undefined;
  const streamChunks = Number.isFinite(rawStreamChunks) ? rawStreamChunks : undefined;
  const errorFixturesIdx = args.indexOf("--error-fixtures");
  const errorFixtures =
    errorFixturesIdx >= 0 && args[errorFixturesIdx + 1]
      ? args[errorFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const retryPathsIdx = args.indexOf("--retry-paths");
  const retryPaths =
    retryPathsIdx >= 0 && args[retryPathsIdx + 1]
      ? args[retryPathsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const skillIdIdx = args.indexOf("--skill-id");
  const skillId =
    skillIdIdx >= 0 && args[skillIdIdx + 1] ? args[skillIdIdx + 1] : undefined;
  const entryPathsIdx = args.indexOf("--entry-paths");
  const entryPaths =
    entryPathsIdx >= 0 && args[entryPathsIdx + 1]
      ? args[entryPathsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const notesIdx = args.indexOf("--notes");
  const notes =
    notesIdx >= 0 && args[notesIdx + 1]
      ? args[notesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const cartItemsIdx = args.indexOf("--cart-items");
  const cartItems =
    cartItemsIdx >= 0 && args[cartItemsIdx + 1]
      ? args[cartItemsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const handoffPathsIdx = args.indexOf("--handoff-paths");
  const handoffPaths =
    handoffPathsIdx >= 0 && args[handoffPathsIdx + 1]
      ? args[handoffPathsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const sandboxNotesStore = args.includes("--sandbox-notes-store");
  const pageSizeIdx = args.indexOf("--page-size");
  const rawPageSize =
    pageSizeIdx >= 0 && args[pageSizeIdx + 1] ? Number(args[pageSizeIdx + 1]) : undefined;
  const pageSize = Number.isFinite(rawPageSize) ? rawPageSize : undefined;
  const providerDelaysIdx = args.indexOf("--provider-delays");
  const providerDelays =
    providerDelaysIdx >= 0 && args[providerDelaysIdx + 1]
      ? args[providerDelaysIdx + 1].split(",").map((s) => Number(s.trim())).filter(Number.isFinite)
      : undefined;
  const startDirIdx = args.indexOf("--start-dir");
  const startDir =
    startDirIdx >= 0 && args[startDirIdx + 1] ? args[startDirIdx + 1] : undefined;
  const navigationStepsIdx = args.indexOf("--navigation-steps");
  const navigationSteps =
    navigationStepsIdx >= 0 && args[navigationStepsIdx + 1]
      ? args[navigationStepsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const categoriesIdx = args.indexOf("--categories");
  const categories =
    categoriesIdx >= 0 && args[categoriesIdx + 1]
      ? args[categoriesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const skinTonesIdx = args.indexOf("--skin-tones");
  const skinTones =
    skinTonesIdx >= 0 && args[skinTonesIdx + 1]
      ? args[skinTonesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const windowStatesIdx = args.indexOf("--window-states");
  const windowStates =
    windowStatesIdx >= 0 && args[windowStatesIdx + 1]
      ? args[windowStatesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const windowProviderIdx = args.indexOf("--window-provider");
  const windowProvider =
    windowProviderIdx >= 0 && args[windowProviderIdx + 1]
      ? args[windowProviderIdx + 1]
      : undefined;
  const noWindowActivation = args.includes("--no-window-activation");
  const markdownFixturesIdx = args.indexOf("--markdown-fixtures");
  const markdownFixtures =
    markdownFixturesIdx >= 0 && args[markdownFixturesIdx + 1]
      ? args[markdownFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noExternalServices = args.includes("--no-external-services");
  const transcriptFixturesIdx = args.indexOf("--transcript-fixtures");
  const transcriptFixtures =
    transcriptFixturesIdx >= 0 && args[transcriptFixturesIdx + 1]
      ? args[transcriptFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const searchQueriesIdx = args.indexOf("--search-queries");
  const searchQueries =
    searchQueriesIdx >= 0 && args[searchQueriesIdx + 1]
      ? args[searchQueriesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const terminalFixtureIdx = args.indexOf("--terminal-fixture");
  const terminalFixture =
    terminalFixtureIdx >= 0 && args[terminalFixtureIdx + 1]
      ? args[terminalFixtureIdx + 1]
      : undefined;
  const noShellCommand = args.includes("--no-shell-command");
  const outputFixturesIdx = args.indexOf("--output-fixtures");
  const outputFixtures =
    outputFixturesIdx >= 0 && args[outputFixturesIdx + 1]
      ? args[outputFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const viewStepsIdx = args.indexOf("--view-steps");
  const viewSteps =
    viewStepsIdx >= 0 && args[viewStepsIdx + 1]
      ? args[viewStepsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const scriptFixtureIdx = args.indexOf("--script-fixture");
  const scriptFixture =
    scriptFixtureIdx >= 0 && args[scriptFixtureIdx + 1]
      ? args[scriptFixtureIdx + 1]
      : undefined;
  const noHandlerSpawn = args.includes("--no-handler-spawn");
  const appsIdx = args.indexOf("--apps");
  const apps =
    appsIdx >= 0 && args[appsIdx + 1]
      ? args[appsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const gridStatesIdx = args.indexOf("--grid-states");
  const gridStates =
    gridStatesIdx >= 0 && args[gridStatesIdx + 1]
      ? args[gridStatesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noAppLaunch = args.includes("--no-app-launch");
  const browserFixturesIdx = args.indexOf("--browser-fixtures");
  const browserFixtures =
    browserFixturesIdx >= 0 && args[browserFixturesIdx + 1]
      ? args[browserFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const timeBucketsIdx = args.indexOf("--time-buckets");
  const timeBuckets =
    timeBucketsIdx >= 0 && args[timeBucketsIdx + 1]
      ? args[timeBucketsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const privacyModesIdx = args.indexOf("--privacy-modes");
  const privacyModes =
    privacyModesIdx >= 0 && args[privacyModesIdx + 1]
      ? args[privacyModesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const browserProviderIdx = args.indexOf("--browser-provider");
  const browserProvider =
    browserProviderIdx >= 0 && args[browserProviderIdx + 1]
      ? args[browserProviderIdx + 1]
      : undefined;
  const noBrowserActivation = args.includes("--no-browser-activation");
  const preferenceFixturesIdx = args.indexOf("--preference-fixtures");
  const preferenceFixtures =
    preferenceFixturesIdx >= 0 && args[preferenceFixturesIdx + 1]
      ? args[preferenceFixturesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const controlTypesIdx = args.indexOf("--control-types");
  const controlTypes =
    controlTypesIdx >= 0 && args[controlTypesIdx + 1]
      ? args[controlTypesIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const resetPathsIdx = args.indexOf("--reset-paths");
  const resetPaths =
    resetPathsIdx >= 0 && args[resetPathsIdx + 1]
      ? args[resetPathsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const sectionsIdx = args.indexOf("--sections");
  const sections =
    sectionsIdx >= 0 && args[sectionsIdx + 1]
      ? args[sectionsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noSystemSettings = args.includes("--no-system-settings");
  const noTccMutation = args.includes("--no-tcc-mutation");
  const catalogIdsIdx = args.indexOf("--catalog-ids");
  const catalogIds =
    catalogIdsIdx >= 0 && args[catalogIdsIdx + 1]
      ? args[catalogIdsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const previewStepsIdx = args.indexOf("--preview-steps");
  const previewSteps =
    previewStepsIdx >= 0 && args[previewStepsIdx + 1]
      ? args[previewStepsIdx + 1].split(",").map((s) => s.trim()).filter(Boolean)
      : undefined;
  const noDesignWrite = args.includes("--no-design-write");
  const dictationFixtureIdx = args.indexOf("--dictation-fixture");
  const dictationFixture =
    dictationFixtureIdx >= 0 && args[dictationFixtureIdx + 1]
      ? args[dictationFixtureIdx + 1]
      : undefined;
  const noMicrophone = args.includes("--no-microphone");
  const noMediaCapture = args.includes("--no-media-capture");
  return {
    recipe,
    session,
    key,
    vision,
    selectAgent,
    targetJson,
    surface,
    json,
    kind,
    index,
    minTargets,
    family,
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
    fileName,
    size,
    portalId,
    range,
    pane,
    bundleId,
    command,
    resourceUri,
    profile,
    themeBefore,
    themeAfter,
    configKey,
    dropTarget,
    scriptletId,
    cancelAfterMs,
    sandboxConfig,
    loops,
    surfaces,
    group,
    caseId,
    destination,
    exportMode,
    entry,
    restartMode,
    monitorProfile,
    transcript,
    fixtureId,
    shareKind,
    acceptMode,
    count,
    burstMs,
    fromDisplay,
    toDisplay,
    handoff,
    foreignApp,
    hoverTarget,
    cancel,
    churn,
    cycles,
    event,
    activeSurface,
    interruptions,
    updates,
    cancelAt,
    target,
    frames,
    intervalMs,
    text,
    fixtureCount,
    filterCycles,
    scrollCycles,
    interleave,
    origins,
    destinations,
    reorderCycles,
    themes,
    scaleFactors,
    states,
    retryCycles,
    fields,
    invalid,
    valid,
    originSurface,
    transitions,
    widths,
    fixture,
    fixtures,
    paths,
    dryRunOnly,
    chrome,
    scrollPositions,
    density,
    hosts,
    selectionCycles,
    resizeCycles,
    itemCount,
    smallCount,
    largeCount,
    shortLineCount,
    tallLineCount,
    pasteboardScope,
    noSystemPasteboard,
    cancelMethods,
    noNativePicker,
    targets,
    inputModes,
    noNativePointer,
    noConfigWrite,
    noNativeInput,
    localFixtureOnly,
    noSubmit,
    proofSteps,
    queries,
    choices,
    selectionSteps,
    previewFixtures,
    noQuickLook,
    chords,
    sortKeys,
    statusFixtures,
    noProcessKill,
    noGlobalHotkeyRegistration,
    noSecretWrite,
    drillPath,
    filter,
    backMethods,
    chipActions,
    sources,
    noNetwork,
    cases,
    noScreenCapture,
    activityFixtures,
    agentFixture,
    insertModes,
    noSecurityPrompts,
    providers,
    tokenKinds,
    editSteps,
    messageCount,
    streamChunks,
    errorFixtures,
    retryPaths,
    skillId,
    entryPaths,
    notes,
    cartItems,
    handoffPaths,
    sandboxNotesStore,
    pageSize,
    providerDelays,
    startDir,
    navigationSteps,
    categories,
    skinTones,
    windowStates,
    windowProvider,
    noWindowActivation,
    markdownFixtures,
    noExternalServices,
    transcriptFixtures,
    searchQueries,
    terminalFixture,
    noShellCommand,
    outputFixtures,
    viewSteps,
    scriptFixture,
    noHandlerSpawn,
    apps,
    gridStates,
    noAppLaunch,
    browserFixtures,
    timeBuckets,
    privacyModes,
    browserProvider,
    noBrowserActivation,
    preferenceFixtures,
    controlTypes,
    resetPaths,
    sections,
    noSystemSettings,
    noTccMutation,
    catalogIds,
    previewSteps,
    noDesignWrite,
    dictationFixture,
    noMicrophone,
    noMediaCapture,
  };
}

// ---------------------------------------------------------------------------
// Recipes
// ---------------------------------------------------------------------------

async function recipePreflight(session: string): Promise<RecipeReceipt> {
  const steps: StepReceipt[] = [];

  // Check session health via session.sh status
  const sessionStatusStep = await step("session-status", () =>
    runTool(
      ["bash", "scripts/agentic/session.sh", "status", session],
      "session-status"
    )
  );

  // Parse the session JSON and enforce health invariants
  const sessionJson = sessionStatusStep.output as Record<string, unknown> | null;
  if (sessionJson && typeof sessionJson === "object" && !("raw" in sessionJson)) {
    const status = sessionJson.status as string | undefined;
    const alive = sessionJson.alive as boolean | undefined;
    const forwarderAlive = sessionJson.forwarderAlive as boolean | undefined;
    const healthy = sessionJson.healthy as boolean | undefined;

    if (
      status === "not_found" ||
      alive === false ||
      forwarderAlive === false ||
      healthy === false
    ) {
      const issues = (sessionJson.issues as string[]) ?? [];
      sessionStatusStep.status = "fail";
      sessionStatusStep.output = {
        ...sessionJson,
        _preflightVerdict: "unhealthy",
        _failReasons: [
          ...(status === "not_found" ? ["status:not_found"] : []),
          ...(alive === false ? ["alive:false"] : []),
          ...(forwarderAlive === false ? ["forwarderAlive:false"] : []),
          ...(healthy === false ? ["healthy:false"] : []),
          ...issues.map((i: string) => `issue:${i}`),
        ],
      };
    }
  }
  steps.push(sessionStatusStep);

  // Check session health via session-state.ts (cross-validates)
  const sessionStateStep = await step("session-state", () =>
    runTool(
      ["bun", "scripts/agentic/session-state.ts", "--session", session],
      "session-state"
    )
  );
  // session-state.ts already exits non-zero when unhealthy, so step() maps that
  steps.push(sessionStateStep);

  // Check window status
  steps.push(
    await step("window-status", () =>
      runTool(["bun", "scripts/agentic/window.ts", "status"], "window-status")
    )
  );

  // Check native input prerequisites
  steps.push(
    await step("input-check", () =>
      runTool(
        ["bun", "scripts/agentic/macos-input.ts", "check"],
        "input-check"
      )
    )
  );

  const allPass = steps.every((s) => s.status === "pass");
  return {
    schemaVersion: SCHEMA_VERSION,
    recipe: "preflight",
    status: allPass ? "pass" : "fail",
    steps,
    summary: allPass
      ? "All prerequisites met"
      : `Failed: ${steps
          .filter((s) => s.status !== "pass")
          .map((s) => s.name)
          .join(", ")}`,
  };
}

async function recipeSurfaceProofPreflight(session: string): Promise<StepReceipt[]> {
  const steps: StepReceipt[] = [];

  const sessionStatusStep = await step("session-status", () =>
    runTool(
      ["bash", "scripts/agentic/session.sh", "status", session],
      "session-status"
    )
  );

  const sessionJson = sessionStatusStep.output as Record<string, unknown> | null;
  if (sessionJson && typeof sessionJson === "object" && !("raw" in sessionJson)) {
    const status = sessionJson.status as string | undefined;
    const alive = sessionJson.alive as boolean | undefined;
    const forwarderAlive = sessionJson.forwarderAlive as boolean | undefined;
    const healthy = sessionJson.healthy as boolean | undefined;

    if (
      status === "not_found" ||
      alive === false ||
      forwarderAlive === false ||
      healthy === false
    ) {
      const issues = (sessionJson.issues as string[]) ?? [];
      sessionStatusStep.status = "fail";
      sessionStatusStep.output = {
        ...sessionJson,
        _surfaceProofVerdict: "unhealthy",
        _failReasons: [
          ...(status === "not_found" ? ["status:not_found"] : []),
          ...(alive === false ? ["alive:false"] : []),
          ...(forwarderAlive === false ? ["forwarderAlive:false"] : []),
          ...(healthy === false ? ["healthy:false"] : []),
          ...issues.map((i: string) => `issue:${i}`),
        ],
      };
    }
  }
  steps.push(sessionStatusStep);

  steps.push(
    await step("session-state", () =>
      runTool(
        ["bun", "scripts/agentic/session-state.ts", "--session", session],
        "session-state"
      )
    )
  );

  return steps;
}

function inferSurfaceClass(kind: SurfaceProofKind): "main" | "attachedPopup" | "detached" {
  switch (kind) {
    case "main":
      return "main";
    case "actionsDialog":
    case "promptPopup":
      return "attachedPopup";
    case "agentChatDetached":
      return "detached";
  }
}

function exactTargetFromSurfaceProofBundle(
  bundle: any,
  fallbackKind: SurfaceProofKind,
  index: number
): AutomationTargetJson {
  if (fallbackKind === "main") return { type: "main" };
  const windowId = bundle?.resolvedTarget?.windowId;
  if (windowId) return { type: "id", id: String(windowId) };
  return { type: "kind", kind: fallbackKind, index };
}

function responsePayload(output: unknown): unknown {
  if (output && typeof output === "object" && "response" in output) {
    return (output as Record<string, unknown>).response;
  }
  return output;
}

function surfaceProofUsage(bundle: any, stateStep: StepReceipt, elementsStep: StepReceipt): SurfaceProofUsage {
  const stepTypes = Array.isArray(bundle?.steps)
    ? bundle.steps.map((s: any) => String(s?.type ?? ""))
    : [];

  return {
    stateFirst: true,
    usedGetState: stateStep.status === "pass",
    usedGetElements: elementsStep.status === "pass",
    usedInspect: stepTypes.includes("inspect"),
    usedWaitFor: stepTypes.includes("waitFor"),
    usedBatch: false,
    usedGpuiEvent: stepTypes.includes("simulateGpuiEvent"),
    usedScreenshot: false,
    usedNativeInput: false,
    usedShow: false,
    usedFixedSleepMs: 0,
  };
}

function surfaceProofCapabilities(
  kind: SurfaceProofKind,
  stateStep: StepReceipt,
  elementsStep: StepReceipt
): SurfaceProofCapabilities {
  const attachedPopup = kind === "actionsDialog" || kind === "promptPopup";
  return {
    state: stateStep.status === "pass",
    elements: elementsStep.status === "pass",
    inspect: true,
    waitFor: true,
    batch: attachedPopup ? ["setInput"] : [],
    gpuiEvent: kind === "agentChatDetached",
    nativeInputRequired: false,
    screenshotRequired: false,
  };
}

async function collectSurfaceStateAndElements(
  session: string,
  target: AutomationTargetJson,
  kind: SurfaceProofKind
): Promise<{ stateStep: StepReceipt; elementsStep: StepReceipt }> {
  const stamp = Date.now();
  const stateStep =
    kind === "agentChatDetached"
      ? await step("surface.getAgentChatState", () =>
          rpc(
            session,
            buildCmd(
              {
                type: "getAgentChatState",
                requestId: `surface-proof-${kind}-state-${stamp}`,
              },
              target
            ),
            { expect: "agent_chatStateResult", timeout: 5000 }
          )
        )
      : await step("surface.getState", () =>
          rpc(
            session,
            buildCmd(
              {
                type: "getState",
                requestId: `surface-proof-${kind}-state-${stamp}`,
              },
              target
            ),
            { expect: "stateResult", timeout: 5000 }
          )
        );

  const elementsStep = await step("surface.getElements", () =>
    rpc(
      session,
      buildCmd(
        {
          type: "getElements",
          requestId: `surface-proof-${kind}-elements-${stamp}`,
          limit: 500,
        },
        target
      ),
      { expect: "elementsResult", timeout: 5000 }
    )
  );

  return { stateStep, elementsStep };
}

async function recipeSurfaceProof(
  session: string,
  opts: { kind?: SurfaceProofKind; index?: number } = {}
): Promise<RecipeReceipt> {
  const kind = opts.kind ?? "main";
  const index = opts.index ?? 0;
  const preflightSteps = await recipeSurfaceProofPreflight(session);
  let bundle;

  if (!preflightSteps.every((s) => s.status === "pass")) {
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: "surface-proof",
      status: "fail",
      steps: preflightSteps,
      summary: `Cannot proceed: ${preflightSteps
        .filter((s) => s.status !== "pass")
        .map((s) => s.name)
        .join(", ")}`,
    };
  }

  switch (kind) {
    case "main":
      bundle = await runMainWindowExactIdScenario(session);
      break;
    case "actionsDialog":
      bundle = await runActionsDialogExactIdScenario(session, index);
      break;
    case "promptPopup":
      bundle = await runPromptPopupExactIdScenario(session, index);
      break;
    case "agentChatDetached":
      bundle = await runDetachedAgentChatExactIdScenario(session, index);
      break;
  }

  const target = exactTargetFromSurfaceProofBundle(bundle, kind, index);
  const { stateStep, elementsStep } = await collectSurfaceStateAndElements(
    session,
    target,
    kind
  );
  const initialWindowId = bundle.resolvedTarget.windowId;
  const finalWindowId = target.type === "id" ? target.id : initialWindowId;
  const proofBundle = {
    ...bundle,
    targetIdentity: {
      stable: initialWindowId === finalWindowId,
      initialWindowId,
      finalWindowId,
    },
    usage: surfaceProofUsage(bundle, stateStep, elementsStep),
    capabilities: surfaceProofCapabilities(kind, stateStep, elementsStep),
    state: responsePayload(stateStep.output),
    elements: responsePayload(elementsStep.output),
    steps: [
      ...bundle.steps,
      {
        type: "getState",
        at: new Date().toISOString(),
        request: { target },
        response: responsePayload(stateStep.output),
      },
      {
        type: "getElements",
        at: new Date().toISOString(),
        request: { target },
        response: responsePayload(elementsStep.output),
      },
    ],
  };
  const allPass =
    bundle.warnings.length === 0 &&
    stateStep.status === "pass" &&
    elementsStep.status === "pass" &&
    proofBundle.targetIdentity.stable;

  console.error(
    JSON.stringify({
      event: "surface_proof_complete",
      recipe: "surface-proof",
      kind,
      scenario: bundle.scenario,
      surfaceClass: inferSurfaceClass(kind),
      warningCount: bundle.warnings.length,
      resolvedWindowId: bundle.resolvedTarget.windowId,
      surfaceId: bundle.resolvedTarget.surfaceId ?? null,
      usedScreenshot: false,
      usedNativeInput: false,
    })
  );

  return {
    schemaVersion: SCHEMA_VERSION,
    recipe: "surface-proof",
    status: allPass ? "pass" : "fail",
    steps: [
      ...preflightSteps,
      {
        name: "scenario",
        status: bundle.warnings.length === 0 ? "pass" : "fail",
        output: bundle,
        durationMs: 0,
      },
      stateStep,
      elementsStep,
    ],
    summary:
      allPass
        ? `State-first ${inferSurfaceClass(kind)} proof succeeded for ${kind}`
        : `Scenario warnings: ${bundle.warnings.join(", ")}`,
    proofBundle,
  };
}

/**
 * Returns true when the target is the main window (or no target specified).
 * Non-main targets (e.g., agentChatDetached) should skip show/triggerBuiltin.
 */
function isMainLikeTarget(target?: AutomationTargetJson): boolean {
  if (!target) return true;
  if (target.type === "main" || target.type === "focused") return true;
  return false;
}

async function recipeAgentChatOpen(
  session: string,
  opts: { target?: AutomationTargetJson } = {}
): Promise<RecipeReceipt> {
  const steps: StepReceipt[] = [];

  if (isMainLikeTarget(opts.target)) {
    // 1. Show window
    steps.push(
      await step("show", () => send(session, '{"type":"show"}'))
    );

    // macOS focus settle: the window needs to become frontmost after
    // show before triggerBuiltin can target it — wait on the actual
    // windowFocused condition instead of a fixed sleep.
    await settleWindowFocus(session);

    // 2. Trigger Agent Chat
    steps.push(
      await step("trigger-agent_chat", () =>
        send(session, '{"type":"triggerBuiltin","name":"tab-ai"}')
      )
    );
  } else {
    // Non-main target: skip show/triggerBuiltin — the detached Agent Chat
    // surface is assumed to already exist. We only wait/verify.
    steps.push({
      name: "skip-main-open",
      status: "pass",
      output: {
        skipped: true,
        reason: "non-main Agent Chat target supplied; assuming detached target already exists",
        target: opts.target,
      },
      durationMs: 0,
    });
  }

  // 3. Wait for Agent Chat to be ready using waitFor instead of fixed sleep
  steps.push(
    await step("wait-agent_chat-ready", () =>
      rpc(
        session,
        buildCmd(
          {
            type: "waitFor",
            requestId: "w-agent_chat-ready",
            condition: { type: "agent_chatReady" },
            timeout: 8000,
            pollInterval: 25,
            trace: "onFailure",
          },
          opts.target
        ),
        { expect: "waitForResult", timeout: 10000 }
      )
    )
  );

  // 4. State-only verification: no screenshot, no probe
  steps.push(
    await step("verify-agent_chat-ready", () =>
      runTool(
        verifyArgs(
          [
            "bun",
            "scripts/agentic/verify-shot.ts",
            "--session",
            session,
            "--label",
            "agent_chat-open",
            "--skip-screenshot",
            "--skip-probe",
            "--agent_chat-context-ready",
          ],
          opts.target
        ),
        "verify-ready"
      )
    )
  );

  const allPass = steps.every((s) => s.status === "pass");
  return {
    schemaVersion: SCHEMA_VERSION,
    recipe: "agent_chat-open",
    status: allPass
      ? "pass"
      : steps.some((s) => s.status === "error")
        ? "error"
        : "fail",
    steps,
    summary: allPass
      ? "Agent Chat opened and context ready"
      : `Failed at: ${steps
          .filter((s) => s.status !== "pass")
          .map((s) => s.name)
          .join(", ")}`,
  };
}

async function recipeAgentChatPickerAccept(
  session: string,
  acceptKey: "enter" | "tab",
  opts: {
    emitVision?: boolean;
    target?: AutomationTargetJson;
    surface?: string;
    captureWindowId?: number;
    inputMode?: RoutedInputMode;
  } = {}
): Promise<RecipeReceipt> {
  const steps: StepReceipt[] = [];

  // 1. Open Agent Chat first
  const openResult = await recipeAgentChatOpen(session, { target: opts.target });
  steps.push({
    name: "agent_chat-open",
    status: openResult.status,
    output: openResult,
    durationMs: openResult.steps.reduce((sum, s) => sum + s.durationMs, 0),
  });

  if (openResult.status !== "pass") {
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: `agent_chat-${acceptKey}-accept`,
      status: "error",
      steps,
      summary: "Cannot proceed: Agent Chat open failed",
    };
  }

  // 2. Reset probe before native interaction to avoid stale accepted items
  steps.push(
    await step("reset-probe", () =>
      send(
        session,
        buildCmd(
          {
            type: "resetAgentChatTestProbe",
            requestId: `reset-${acceptKey}-${Date.now()}`,
          },
          opts.target
        )
      )
    )
  );

  // 3. Type / to open the supported slash picker.
  //    For non-main targets (detached Agent Chat, popups), route through batch/GPUI first
  //    so the flow succeeds even when the human user types in another app.
  const typeAtStep = await routedInputStep("type-slash-trigger", "type", "/", session, {
    target: opts.target,
    surface: opts.surface,
    inputMode: opts.inputMode,
  });
  steps.push(typeAtStep);

  if (typeAtStep.status !== "pass") {
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: `agent_chat-${acceptKey}-accept`,
      status: "fail",
      steps,
      summary: `Input failed at type-slash-trigger (method: ${typeAtStep.inputMethod ?? "unknown"})`,
    };
  }

  // 4. Wait for picker to open using waitFor instead of fixed sleep
  steps.push(
    await step("wait-picker-open", () =>
      rpc(
        session,
        buildCmd(
          {
            type: "waitFor",
            requestId: `w-picker-open-${acceptKey}`,
            condition: { type: "agent_chatPickerOpen" },
            timeout: 3000,
            pollInterval: 25,
            trace: "onFailure",
          },
          opts.target
        ),
        { expect: "waitForResult", timeout: 5000 }
      )
    )
  );

  // 5. State-only verification for picker: no screenshot, no probe
  steps.push(
    await step("verify-picker-open", () =>
      runTool(
        verifyArgs(
          [
            "bun",
            "scripts/agentic/verify-shot.ts",
            "--session",
            session,
            "--label",
            "picker-open",
            "--skip-screenshot",
            "--skip-probe",
            "--agent_chat-picker-open",
          ],
          opts.target
        ),
        "verify-picker"
      )
    )
  );

  // 6. Accept with key (routed: batch/GPUI for non-main, native for main)
  const acceptKeyStep = await routedInputStep(`accept-${acceptKey}`, "key", acceptKey, session, {
    target: opts.target,
    surface: opts.surface,
    inputMode: opts.inputMode,
  });
  steps.push(acceptKeyStep);

  if (acceptKeyStep.status !== "pass") {
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: `agent_chat-${acceptKey}-accept`,
      status: "fail",
      steps,
      summary: `Input failed at accept-${acceptKey} (method: ${acceptKeyStep.inputMethod ?? "unknown"})`,
    };
  }

  // 7. Wait for key-specific acceptance proof (not generic agent_chatItemAccepted)
  steps.push(
    await step("wait-accepted-via-key", () =>
      rpc(
        session,
        buildCmd(
          {
            type: "waitFor",
            requestId: `w-accepted-via-${acceptKey}`,
            condition: { type: "agent_chatAcceptedViaKey", key: acceptKey },
            timeout: 3000,
            pollInterval: 25,
            trace: "onFailure",
          },
          opts.target
        ),
        { expect: "waitForResult", timeout: 5000 }
      )
    )
  );

  // 8. Final proof: screenshot + probe assertion (the only screenshot in the recipe)
  const finalVerifyBase = [
    "bun",
    "scripts/agentic/verify-shot.ts",
    "--session",
    session,
    "--label",
    `${acceptKey}-accepted`,
    "--agent_chat-picker-closed",
    "--agent_chat-item-accepted",
    "--agent_chat-accepted-via",
    acceptKey,
    ...(opts.emitVision ? ["--vision"] : []),
    ...(opts.captureWindowId != null ? ["--capture-window-id", String(opts.captureWindowId)] : []),
  ];
  steps.push(
    await step("verify-accepted", () =>
      runTool(verifyArgs(finalVerifyBase, opts.target), "verify-accepted")
    )
  );

  const allPass = steps.every((s) => s.status === "pass");

  // Extract the verify-accepted step's proof bundle for top-level access
  const verifyStep = steps.find((s) => s.name === "verify-accepted");
  const proofBundle =
    opts.emitVision && verifyStep?.output ? verifyStep.output : undefined;

  return {
    schemaVersion: SCHEMA_VERSION,
    recipe: `agent_chat-${acceptKey}-accept`,
    status: allPass
      ? "pass"
      : steps.some((s) => s.status === "error")
        ? "error"
        : "fail",
    steps,
    summary: allPass
      ? `Agent Chat picker accepted via ${acceptKey}`
      : `Failed at: ${steps
          .filter((s) => s.status !== "pass")
          .map((s) => s.name)
          .join(", ")}`,
    ...(proofBundle ? { proofBundle } : {}),
  };
}

async function recipeAgentChatSetupRecovery(
  session: string,
  selectAgent?: string
): Promise<RecipeReceipt> {
  const steps: StepReceipt[] = [];

  // 1. Show window
  steps.push(
    await step("show", () => send(session, '{"type":"show"}'))
  );

  // Post-show focus settle on the real windowFocused condition.
  await settleWindowFocus(session);

  // 2. Trigger Agent Chat
  steps.push(
    await step("trigger-agent_chat", () =>
      send(session, '{"type":"triggerBuiltin","name":"tab-ai"}')
    )
  );

  // 3. Wait for setup card to appear (or agent_chatReady if no setup needed)
  const waitSetupStep = await step("wait-setup-visible", () =>
    rpc(
      session,
      JSON.stringify({
        type: "waitFor",
        requestId: "w-setup-visible",
        condition: { type: "agent_chatSetupVisible" },
        timeout: 8000,
        pollInterval: 25,
        trace: "onFailure",
      }),
      { expect: "waitForResult", timeout: 10000 }
    )
  );
  steps.push(waitSetupStep);

  if (waitSetupStep.status !== "pass") {
    // Setup card never appeared — might already be ready or error
    const verifyStep = await step("verify-no-setup", () =>
      runTool(
        [
          "bun",
          "scripts/agentic/verify-shot.ts",
          "--session",
          session,
          "--label",
          "setup-not-found",
          "--skip-screenshot",
          "--skip-probe",
          "--agent_chat-status",
          "setup",
        ],
        "verify-no-setup"
      )
    );
    steps.push(verifyStep);
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-setup-recovery",
      status: "fail",
      steps,
      summary:
        "Setup card did not appear — Agent Chat may already be ready or failed to open",
    };
  }

  // 4. State-only verification of setup card
  steps.push(
    await step("verify-setup-visible", () =>
      runTool(
        [
          "bun",
          "scripts/agentic/verify-shot.ts",
          "--session",
          session,
          "--label",
          "setup",
          "--skip-screenshot",
          "--skip-probe",
          "--agent_chat-setup-visible",
        ],
        "verify-setup"
      )
    )
  );

  // 5. If --select-agent provided, drive the setup recovery flow
  if (selectAgent) {
    // 5a. Open agent picker
    steps.push(
      await step("open-setup-agent-picker", () =>
        rpc(
          session,
          JSON.stringify({
            type: "performAgentChatSetupAction",
            requestId: "a-open-picker",
            action: "openAgentPicker",
          }),
          { expect: "agent_chatSetupActionResult", timeout: 5000 }
        )
      )
    );

    // 5b. Wait for picker to open
    steps.push(
      await step("wait-agent-picker-open", () =>
        rpc(
          session,
          JSON.stringify({
            type: "waitFor",
            requestId: "w-agent-picker-open",
            condition: { type: "agent_chatSetupAgentPickerOpen" },
            timeout: 3000,
            pollInterval: 25,
            trace: "onFailure",
          }),
          { expect: "waitForResult", timeout: 5000 }
        )
      )
    );

    // 5c. Select the agent
    steps.push(
      await step("select-setup-agent", () =>
        rpc(
          session,
          JSON.stringify({
            type: "performAgentChatSetupAction",
            requestId: "a-select-agent",
            action: "selectAgent",
            agentId: selectAgent,
          }),
          { expect: "agent_chatSetupActionResult", timeout: 5000 }
        )
      )
    );

    // 5d. Wait for selected-agent confirmation
    steps.push(
      await step("wait-selected-agent", () =>
        rpc(
          session,
          JSON.stringify({
            type: "waitFor",
            requestId: "w-selected-agent",
            condition: { type: "agent_chatSetupSelectedAgent", agentId: selectAgent },
            timeout: 3000,
            pollInterval: 25,
            trace: "onFailure",
          }),
          { expect: "waitForResult", timeout: 5000 }
        )
      )
    );

    // 5e. Wait for Agent Chat to become ready after agent selection
    steps.push(
      await step("wait-ready", () =>
        rpc(
          session,
          JSON.stringify({
            type: "waitFor",
            requestId: "w-ready-after-select",
            condition: { type: "agent_chatReady" },
            timeout: 8000,
            pollInterval: 25,
            trace: "onFailure",
          }),
          { expect: "waitForResult", timeout: 10000 }
        )
      )
    );
  }

  // 6. Final verification — assert expected Agent Chat status based on flow
  const verifyArgs = [
    "bun",
    "scripts/agentic/verify-shot.ts",
    "--session",
    session,
    "--label",
    selectAgent ? "setup-recovered" : "setup-final",
    "--skip-probe",
    "--agent_chat-status",
    selectAgent ? "idle" : "setup",
  ];
  steps.push(
    await step("verify-final", () =>
      runTool(verifyArgs, "verify-final")
    )
  );

  const allPass = steps.every((s) => s.status === "pass");

  // Extract final Agent Chat state from the verify-final step for the receipt
  const verifyFinalStep = steps.find((s) => s.name === "verify-final");
  const finalState =
    verifyFinalStep?.output &&
    typeof verifyFinalStep.output === "object" &&
    !("raw" in (verifyFinalStep.output as Record<string, unknown>))
      ? (verifyFinalStep.output as Record<string, unknown>).state
      : null;
  const finalSetup =
    finalState && typeof finalState === "object"
      ? (finalState as Record<string, unknown>).setup
      : null;

  // Log recipe completion as single-line JSON on stderr
  console.error(
    JSON.stringify({
      event: "agent_chat_setup_recovery_complete",
      finalStatus:
        finalState && typeof finalState === "object"
          ? (finalState as Record<string, unknown>).status
          : null,
      finalReasonCode:
        finalSetup && typeof finalSetup === "object"
          ? (finalSetup as Record<string, unknown>).reasonCode
          : null,
      selectedAgent: selectAgent ?? null,
    })
  );

  return {
    schemaVersion: SCHEMA_VERSION,
    recipe: "agent_chat-setup-recovery",
    status: allPass
      ? "pass"
      : steps.some((s) => s.status === "error")
        ? "error"
        : "fail",
    steps,
    summary: allPass
      ? selectAgent
        ? `Agent Chat setup recovered via ${selectAgent}`
        : "Agent Chat setup card rendered"
      : `Failed at: ${steps
          .filter((s) => s.status !== "pass")
          .map((s) => s.name)
          .join(", ")}`,
  };
}

/**
 * Resolved identity for a detached Agent Chat window.
 * Threaded through the entire recipe so proof stays coherent.
 */
interface DetachedResolved {
  targetJson: AutomationTargetJson;
  surfaceId: string | null;
  automationWindowId: number | null;
  osWindowId: number | null;
}

async function recipeAgentChatDetachedAccept(
  session: string,
  acceptKey: "enter" | "tab",
  opts: {
    emitVision?: boolean;
    kind?: string;
    index?: number;
  } = {}
): Promise<RecipeReceipt> {
  const steps: StepReceipt[] = [];
  const kind = opts.kind ?? "agentChatDetached";
  const index = opts.index ?? 0;

  // 1. Promote to exact target — resolve the kind-based target to an exact ID
  //    first, then inspect. This ensures all subsequent RPCs use the exact ID
  //    and never re-resolve by kind.
  const inspectStep = await step("promote-exact-target", () =>
    runTool(
      [
        "bun",
        "scripts/agentic/automation-window.ts",
        "inspect",
        "--session",
        session,
        "--kind",
        kind,
        "--index",
        String(index),
      ],
      "promote-exact-target"
    )
  );
  steps.push(inspectStep);

  if (inspectStep.status !== "pass") {
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-detached-accept",
      status: "error",
      steps,
      summary: "Cannot proceed: exact target promotion failed",
    };
  }

  // Extract identity from the inspect envelope and promote to exact ID
  const inspectOutput = inspectStep.output as Record<string, unknown>;
  const surfaceId = (inspectOutput.surfaceId as string) ?? null;
  const rawWindowId = inspectOutput.automationWindowId;
  const parsedWindowId =
    typeof rawWindowId === "number"
      ? rawWindowId
      : rawWindowId != null
        ? Number(rawWindowId)
        : null;
  const automationWindowId =
    typeof parsedWindowId === "number" &&
    Number.isFinite(parsedWindowId) &&
    parsedWindowId > 0
      ? parsedWindowId
      : null;
  const osWindowId =
    typeof inspectOutput.osWindowId === "number" && inspectOutput.osWindowId > 0
      ? inspectOutput.osWindowId
      : null;

  // Promote: if we got an automation window ID from inspect, use exact ID
  // targeting. Detached proof flows MUST have an exact target — no kind fallback.
  if (automationWindowId == null) {
    console.error(
      JSON.stringify({
        event: "agentic.proof_flow.exact_target_required",
        fromKind: kind,
        fromIndex: index,
        reason: "automationWindowId not available from inspect; detached proof requires exact target",
      })
    );
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-detached-accept",
      status: "error",
      steps,
      summary: "Cannot proceed: detached Agent Chat proof requires exact automationWindowId but inspect returned none",
    };
  }

  const targetJson: AutomationTargetJson = { type: "id", id: String(automationWindowId) };
  console.error(
    JSON.stringify({
      event: "agentic.promote_exact_target",
      fromKind: kind,
      fromIndex: index,
      promotedTargetJson: targetJson,
      automationWindowId,
      surfaceId,
      osWindowId,
    })
  );

  const resolved: DetachedResolved = {
    targetJson,
    surfaceId,
    automationWindowId,
    osWindowId,
  };

  if (osWindowId == null) {
    console.error(
      JSON.stringify({
        event: "agentic.proof_flow.os_window_id_required",
        automationWindowId,
        surfaceId,
        reason: "detached Agent Chat proof requires osWindowId for exact screenshot routing",
      })
    );
    steps.push({
      name: "require-os-window-id",
      status: "fail",
      output: {
        error: "detached Agent Chat proof requires osWindowId from inspect output",
        resolved,
      },
      durationMs: 0,
    });
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-detached-accept",
      status: "error",
      steps,
      summary: "Cannot proceed: detached Agent Chat proof requires osWindowId for exact screenshot routing",
    };
  }

  // 2. Emit structured identity bundle log on stderr
  console.error(
    JSON.stringify({
      event: "agent_chat_final_identity_bundle",
      automationWindowId,
      surfaceId,
      osWindowId,
    })
  );

  // 3. Delegate to the standard picker-accept recipe with resolved identity threaded through.
  //    Use osWindowId (native CGWindowID from inspect) for strict capture proof.
  const captureWindowId = osWindowId ?? undefined;
  const acceptResult = await recipeAgentChatPickerAccept(session, acceptKey, {
    emitVision: opts.emitVision,
    target: targetJson,
    surface: surfaceId ?? undefined,
    captureWindowId: captureWindowId ?? undefined,
    inputMode: "force-native",
  });

  // Incorporate accept steps (skip the wrapper — flatten the inner steps for transparency)
  for (const s of acceptResult.steps) {
    steps.push(s);
  }

  // 4. Validate proof receipt chain — detached proof requires a real proof bundle
  const proofBundle = acceptResult.proofBundle as Record<string, unknown> | undefined;

  if (!proofBundle) {
    console.error(
      JSON.stringify({
        event: "agentic.proof_flow.receipt_missing",
        recipe: "agent_chat-detached-accept",
        automationWindowId,
        reason: "acceptResult.proofBundle is absent; detached proof requires a real proof bundle",
      })
    );
    steps.push({
      name: "proof-receipt-check",
      status: "fail",
      output: {
        error: "proofBundle missing from accept result",
        resolved,
      },
      durationMs: 0,
    });
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-detached-accept",
      status: "error",
      steps,
      summary: "Detached Agent Chat proof failed: proofBundle missing from accept result",
    };
  }

  const captureTarget = proofBundle.captureTarget as Record<string, unknown> | undefined;
  if (!captureTarget) {
    console.error(
      JSON.stringify({
        event: "agentic.proof_flow.receipt_missing",
        recipe: "agent_chat-detached-accept",
        automationWindowId,
        reason: "proofBundle.captureTarget is absent; detached proof requires capture identity",
      })
    );
    steps.push({
      name: "proof-receipt-check",
      status: "fail",
      output: {
        error: "proofBundle.captureTarget missing",
        resolved,
        proofBundle,
      },
      durationMs: 0,
    });
    return {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-detached-accept",
      status: "error",
      steps,
      summary: "Detached Agent Chat proof failed: proofBundle.captureTarget missing",
    };
  }

  // Validate identity alignment — requested vs actual window
  let identityMismatch = false;
  const requestedId = captureTarget.requestedWindowId;
  const actualId = captureTarget.actualWindowId;
  if (requestedId != null && actualId != null && requestedId !== actualId) {
    identityMismatch = true;
    console.error(
      JSON.stringify({
        event: "agentic.proof_flow.capture_identity_mismatch",
        recipe: "agent_chat-detached-accept",
        automationWindowId,
        requestedWindowId: requestedId,
        actualWindowId: actualId,
      })
    );
  }

  if (identityMismatch) {
    steps.push({
      name: "identity-check",
      status: "fail",
      output: {
        error: "captureTarget identity mismatch",
        resolved,
        proofBundle,
      },
      durationMs: 0,
    });
  } else {
    steps.push({
      name: "proof-receipt-check",
      status: "pass",
      output: { resolved, captureTarget },
      durationMs: 0,
    });
    steps.push({
      name: "identity-check",
      status: "pass",
      output: { resolved },
      durationMs: 0,
    });
  }

  const allPass = !identityMismatch && acceptResult.status === "pass";

  // Attach resolvedTarget only when a real proof bundle exists
  const mergedProofBundle: Record<string, unknown> = {
    ...proofBundle,
    resolvedTarget: resolved,
  };

  return {
    schemaVersion: SCHEMA_VERSION,
    recipe: "agent_chat-detached-accept",
    status: allPass
      ? "pass"
      : identityMismatch || steps.some((s) => s.status === "fail")
        ? "fail"
        : steps.some((s) => s.status === "error")
          ? "error"
          : "fail",
    steps,
    summary: allPass
      ? `Detached Agent Chat picker accepted via ${acceptKey} (window ${automationWindowId})`
      : identityMismatch
        ? "Identity mismatch: captureTarget.requestedWindowId != actualWindowId"
        : `Failed at: ${steps
            .filter((s) => s.status !== "pass")
            .map((s) => s.name)
            .join(", ")}`,
    proofBundle: mergedProofBundle,
  };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const {
  recipe,
  session,
  key,
  vision,
  selectAgent,
  targetJson,
  surface,
  kind,
  index,
  minTargets,
  family,
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
  fileName,
  size,
  portalId,
  range,
  pane,
  bundleId,
  command,
  resourceUri,
  profile,
  themeBefore,
  themeAfter,
  configKey,
  dropTarget,
  scriptletId,
  cancelAfterMs,
  sandboxConfig,
  loops,
  surfaces,
  group,
  caseId,
  destination,
  exportMode,
  entry,
  restartMode,
  monitorProfile,
  transcript,
  fixtureId,
  shareKind,
  acceptMode,
  count,
  burstMs,
  fromDisplay,
  toDisplay,
  handoff,
  foreignApp,
  hoverTarget,
  cancel,
  churn,
  cycles,
  event,
  activeSurface,
  interruptions,
  updates,
  cancelAt,
  target,
  frames,
  intervalMs,
  text,
  fixtureCount,
  filterCycles,
  scrollCycles,
  interleave,
  origins,
  destinations,
  reorderCycles,
  themes,
  scaleFactors,
  states,
  retryCycles,
  fields,
  invalid,
  valid,
  originSurface,
  transitions,
  widths,
  fixture,
  fixtures,
  paths,
  dryRunOnly,
  chrome,
  scrollPositions,
  density,
  hosts,
  selectionCycles,
  resizeCycles,
  itemCount,
  smallCount,
  largeCount,
  shortLineCount,
  tallLineCount,
  pasteboardScope,
  noSystemPasteboard,
  cancelMethods,
  noNativePicker,
  targets,
  inputModes,
  noNativePointer,
  noConfigWrite,
  noNativeInput,
  localFixtureOnly,
  noSubmit,
  proofSteps,
  queries,
  choices,
  selectionSteps,
  previewFixtures,
  noQuickLook,
  chords,
  sortKeys,
  statusFixtures,
  noProcessKill,
  noGlobalHotkeyRegistration,
  noSecretWrite,
  drillPath,
  filter,
  backMethods,
  chipActions,
  sources,
  noNetwork,
  cases,
  noScreenCapture,
  activityFixtures,
  agentFixture,
  insertModes,
  noSecurityPrompts,
  providers,
  tokenKinds,
  editSteps,
  messageCount,
  streamChunks,
  errorFixtures,
  retryPaths,
  skillId,
  entryPaths,
  notes,
  cartItems,
  handoffPaths,
  sandboxNotesStore,
  pageSize,
  providerDelays,
  startDir,
  navigationSteps,
  categories,
  skinTones,
  windowStates,
  windowProvider,
  noWindowActivation,
  markdownFixtures,
  noExternalServices,
  transcriptFixtures,
  searchQueries,
  terminalFixture,
  noShellCommand,
  outputFixtures,
  viewSteps,
  scriptFixture,
  noHandlerSpawn,
  apps,
  gridStates,
  noAppLaunch,
  browserFixtures,
  timeBuckets,
  privacyModes,
  browserProvider,
  noBrowserActivation,
  preferenceFixtures,
  controlTypes,
  resetPaths,
  sections,
  noSystemSettings,
  noTccMutation,
  catalogIds,
  previewSteps,
  noDesignWrite,
  dictationFixture,
  noMicrophone,
  noMediaCapture,
} = parseArgs();

let result: RecipeReceipt;

switch (recipe) {
  case "preflight":
    result = await recipePreflight(session);
    break;

  case "agent_chat-open":
    result = await recipeAgentChatOpen(session, { target: targetJson });
    break;

  case "agent_chat-accept":
    result = await recipeAgentChatPickerAccept(session, key, {
      emitVision: vision,
      target: targetJson,
      surface,
    });
    break;

  case "agent_chat-enter-accept":
    result = await recipeAgentChatPickerAccept(session, "enter", {
      emitVision: vision,
      target: targetJson,
      surface,
    });
    break;

  case "agent_chat-tab-accept":
    result = await recipeAgentChatPickerAccept(session, "tab", {
      emitVision: vision,
      target: targetJson,
      surface,
    });
    break;

  case "agent_chat-detached-accept":
    result = await recipeAgentChatDetachedAccept(session, key, {
      emitVision: vision,
      kind: kind ?? "agentChatDetached",
      index: index ?? 0,
    });
    break;

  case "check-leaks": {
    const target = targetJson ?? { type: "main" };
    // 1. Get PID
    const targetReceipt = await step("resolve-pid", async () => {
      return runTool(["bun", "scripts/devtools/targets.ts", "inspect", "--target-json", JSON.stringify(target)], "inspect");
    });
    const pid = (targetReceipt.output as any)?.resolvedTarget?.pid;
    if (!pid) {
      result = {
        schemaVersion: SCHEMA_VERSION,
        recipe: "check-leaks",
        status: "error",
        steps: [targetReceipt],
        summary: "Could not resolve PID for target",
      };
      break;
    }
    // 2. Start recording Leaks
    const traceFile = `leaks_${Date.now()}.trace`;
    const recordReceipt = await step("record-leaks", async () => {
      return runTool(["bun", "scripts/devtools/perf.ts", "record", "--pid", String(pid), "--template", "Leaks", "--output", traceFile, "--duration", "20"], "record");
    });
    // 3. Analyze
    const analyzeReceipt = await step("analyze-leaks", async () => {
      return runTool(["bun", "scripts/devtools/perf.ts", "analyze", "--input", traceFile], "analyze");
    });
    const leaks = (analyzeReceipt.output as any)?.summary?.leakCount ?? 0;
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "check-leaks",
      status: leaks === 0 ? "pass" : "fail",
      steps: [targetReceipt, recordReceipt, analyzeReceipt],
      summary: leaks === 0 ? "No leaks detected" : `${leaks} leaks detected`,
    };
    break;
  }

  case "agent_chat-detached-target-threading-stress": {
    const proofBundle = await runDetachedAgentChatTargetThreadingStressScenario({
      session,
      kind: kind ?? "agentChatDetached",
      index: index ?? 0,
      minTargets: minTargets ?? 2,
      key,
      vision,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-detached-target-threading-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Detached Agent Chat target identity stayed stable across native input, Agent Chat state, waitFor, and strict capture"
          : `Detached Agent Chat target threading stress failed: ${JSON.stringify(proofBundle.failure ?? proofBundle.targetThread?.driftFailures ?? [])}`,
      proofBundle,
    };
    break;
  }

  case "notes-agent_chat-delayed-action-origin-stress": {
    const proofBundle = await runNotesAgentChatDelayedActionOriginStressScenario({
      session,
      drift: drift ?? "generation",
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "notes-agent_chat-delayed-action-origin-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Notes Agent Chat delayed action origin receipt stayed valid"
          : "Notes Agent Chat delayed action origin stress failed closed; app-side origin/generation receipt is missing",
      proofBundle,
    };
    break;
  }

  case "file-portal-origin-roundtrip": {
    const proofBundle = await runAgentChatPortalRoundTripOriginStressScenario({
      session,
      host: host ?? "agent_chat",
      portal: portal ?? "file-search",
      selection: selection ?? "file",
      query: query ?? "AGENTS.md",
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "file-portal-origin-roundtrip",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Agent Chat portal round-trip preserved origin and accepted context-part identity"
          : "Agent Chat portal round-trip origin stress failed closed; app-side portal origin/context receipts are missing",
      proofBundle,
    };
    break;
  }

  case "permission-privacy-preflight": {
    const proofBundle = await runPermissionPreflightReadonlyScenario({ session, kinds });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "permission-privacy-preflight",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Read-only permission preflight completed without opening System Settings or mutating permissions"
          : "Read-only permission preflight failed closed without mutating OS permission state",
      proofBundle,
    };
    break;
  }

  case "shortcut-recorder-focus-capture": {
    const proofBundle = await runShortcutRecorderFocusCaptureStressScenario({
      session,
      chord: chord ?? "cmd+shift+7",
      action: action ?? "test-agentic-shortcut",
      surface: surface ?? "shortcuts",
      sandboxConfig: Boolean(sandboxConfig),
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "shortcut-recorder-focus-capture",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Shortcut recorder captured the native chord on the exact focused recorder surface"
          : "Shortcut recorder focus/capture stress failed closed; recorder receipts are missing",
      proofBundle,
    };
    break;
  }

  case "template-prompt-automation-parity-stress": {
    const proofBundle = await runTemplatePromptAutomationParityStressScenario({
      session,
      template,
      field,
      value,
      forcedValue,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "template-prompt-automation-parity-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "TemplatePrompt automation parity proved state, elements, actions, submit, cancel, and forceSubmit"
          : "TemplatePrompt automation parity stress failed; inspect proofBundle.templatePrompt and failure",
      proofBundle,
    };
    break;
  }

  case "current-app-commands-frontmost-stress": {
    const proofBundle = await runCurrentAppCommandsFrontmostStressScenario({
      session,
      query,
      alias,
      expectedApp,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "current-app-commands-frontmost-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Current App Commands preserved the frontmost snapshot and shared filtering semantics"
          : "Current App Commands frontmost stress failed closed; frontmost/filter/action receipts are missing",
      proofBundle,
    };
    break;
  }

  case "actions-captured-subject-frame-stress": {
    const proofBundle = await runActionsCapturedSubjectFrameStressScenario({
      session,
      source,
      action,
      mutation,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "actions-captured-subject-frame-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Actions dialog executed the captured subject after frame drift and restored focus"
          : "Actions captured-subject frame stress failed closed; captured subject/frame receipts are missing",
      proofBundle,
    };
    break;
  }

  case "drop-prompt-native-drop-privacy-stress": {
    const proofBundle = await runDropPromptNativeDropPrivacyStressScenario({
      session,
      fileName,
      size,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "drop-prompt-native-drop-privacy-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "DropPrompt native drop receipts stayed redacted"
          : "DropPrompt native drop privacy stress failed closed; native drop injection receipt is missing",
      proofBundle,
    };
    break;
  }

  case "path-prompt-filesystem-edge-stress": {
    const proofBundle = await runPathPromptFilesystemEdgeStressScenario({ session });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "path-prompt-filesystem-edge-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "PathPrompt filesystem edge receipts passed for missing, empty, file-start, and permission-denied cases"
          : "PathPrompt filesystem edge stress failed; inspect proofBundle.pathPrompt",
      proofBundle,
    };
    break;
  }

  case "screenshot-identity-agent_chat-context-stress": {
    const proofBundle = await runScreenshotIdentityAgentChatContextStressScenario({
      session,
      source,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "screenshot-identity-agent_chat-context-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary:
        proofBundle.status === "pass"
          ? "Screenshot identity matched capture, state, and Agent Chat context receipts"
          : "Screenshot identity Agent Chat context stress failed closed; context identity receipt is missing",
      proofBundle,
    };
    break;
  }

  case "clipboard-history-portal-range-stress": {
    const proofBundle = await runClipboardHistoryPortalRangeStressScenario({ session, portalId, range });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "clipboard-history-portal-range-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Clipboard history portal range stress failed closed; portal range receipts are missing",
      proofBundle,
    };
    break;
  }

  case "browser-tabs-cache-identity-stress": {
    const proofBundle = await runBrowserTabsCacheIdentityStressScenario({ session, source });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "browser-tabs-cache-identity-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Browser tabs/history cache identity stress failed closed; cache identity receipts are missing",
      proofBundle,
    };
    break;
  }

  case "scroll-selection-reanchor-stress": {
    const proofBundle = await runScrollSelectionReanchorStressScenario({
      session,
      surfaces: kinds,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "scroll-selection-reanchor-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Scroll selection reanchor stress failed closed; cross-surface reanchor receipts are missing",
      proofBundle,
    };
    break;
  }

  case "permission-assistant-drag-preflight-stress": {
    const proofBundle = await runPermissionAssistantDragPreflightStressScenario({
      session,
      pane,
      bundleId,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "permission-assistant-drag-preflight-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Permission Assistant drag/preflight stress failed closed; passive panel and no-TCC-mutation receipts are missing",
      proofBundle,
    };
    break;
  }

  case "quick-terminal-pty-apply-back-stress": {
    const proofBundle = await runQuickTerminalPtyApplyBackStressScenario({
      session,
      command,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "quick-terminal-pty-apply-back-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Quick Terminal PTY apply-back stress failed closed; PTY/apply-back lifecycle receipts are missing",
      proofBundle,
    };
    break;
  }

  case "mcp-context-resource-attachment-identity-stress": {
    const proofBundle = await runMcpContextResourceAttachmentIdentityStressScenario({
      session,
      resourceUri,
      profile,
      source,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "mcp-context-resource-attachment-identity-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "MCP context resource identity stress failed closed; resource/context-part identity receipts are missing",
      proofBundle,
    };
    break;
  }

  case "settings-theme-hot-reload-stress": {
    const proofBundle = await runSettingsThemeHotReloadStressScenario({
      session,
      themeBefore,
      themeAfter,
      configKey,
      sandboxConfig,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "settings-theme-hot-reload-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Settings/theme hot-reload stress failed closed; config identity, token fingerprint, repaint, and cleanup receipts are missing",
      proofBundle,
    };
    break;
  }

  case "file-search-drag-out-identity-stress": {
    const proofBundle = await runFileSearchDragOutIdentityStressScenario({
      session,
      query,
      fileName,
      dropTarget,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "file-search-drag-out-identity-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "File Search drag-out identity stress failed closed; selected file, drag payload, host refusal, privacy, and return receipts are missing",
      proofBundle,
    };
    break;
  }

  case "scriptlet-bundle-execution-matrix-stress": {
    const proofBundle = await runScriptletBundleExecutionMatrixStressScenario({
      session,
      scriptletId,
      bundleId,
      cancelAfterMs,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "scriptlet-bundle-execution-matrix-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Scriptlet bundle execution matrix stress failed closed; scriptlet id, bundle hash, isolation, output, and cancellation receipts are missing",
      proofBundle,
    };
    break;
  }

  case "tray-global-hotkey-menu-mutation-stress": {
    const proofBundle = await runTrayGlobalHotkeyMenuMutationStressScenario({
      session,
      loops,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "tray-global-hotkey-menu-mutation-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Tray/global-hotkey menu mutation stress failed closed; tray generation, duplicate detection, route identity, and cleanup receipts are missing",
      proofBundle,
    };
    break;
  }

  case "multi-window-resize-monitor-restoration-stress": {
    const proofBundle = await runMultiWindowResizeMonitorRestorationStressScenario({
      session,
      surfaces,
      monitorProfile,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "multi-window-resize-monitor-restoration-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Multi-window resize/monitor restoration stress failed closed; window identity, bounds, scale/rem, restore order, and clobber receipts are missing",
      proofBundle,
    };
    break;
  }

  case "agent_chat-targeted-dictation-delivery-stress": {
    const proofBundle = await runAgentChatTargetedDictationDeliveryStressScenario({
      session,
      kind,
      index,
      transcript,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "agent_chat-targeted-dictation-delivery-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Agent Chat-targeted dictation delivery stress failed closed; transcript generation, target identity, cursor range, wrong-window guard, and passive setup receipts are missing",
      proofBundle,
    };
    break;
  }

  case "clipboard-share-trust-install-stress": {
    const proofBundle = await runClipboardShareTrustInstallStressScenario({
      session,
      fixtureId,
      shareKind,
      acceptMode,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "clipboard-share-trust-install-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Clipboard share trust install stress failed closed; share URI, trust prompt, no-install-before-trust, and clipboard restore receipts are missing",
      proofBundle,
    };
    break;
  }

  case "clipboard-share-watcher-stale-replay-stress": {
    const proofBundle = await runClipboardShareWatcherStaleReplayStressScenario({
      session,
      fixtureId,
      shareKind,
      count,
      burstMs,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "clipboard-share-watcher-stale-replay-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Clipboard watcher stale/replay stress failed closed; generation ordering, stale rejection, replay, duplicate, and cleanup receipts are missing",
      proofBundle,
    };
    break;
  }

  case "permission-share-cross-prompt-focus-stress": {
    const proofBundle = await runPermissionShareCrossPromptFocusStressScenario({
      session,
      fixtureId,
      shareKind,
      pane,
      bundleId,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "permission-share-cross-prompt-focus-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Permission/share cross-prompt focus stress failed closed; prompt priority, focus routing, no activation leak, and cleanup receipts are missing",
      proofBundle,
    };
    break;
  }

  case "visible-text-clipping-overlap-stress": {
    const proofBundle = await runVisibleTextClippingOverlapStressScenario({
      session,
      surfaces,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "visible-text-clipping-overlap-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Visible text clipping/overlap stress measured text widths, layout bounds, fit, truncation, overlap, and cleanup without relying on screenshots"
        : "Visible text clipping/overlap stress failed; inspect proofBundle.visibleTextLayoutAudit for missing or failing measurement receipts",
      proofBundle,
    };
    break;
  }

  case "layout-measurement-regression-stress": {
    const proofBundle = await runLayoutMeasurementRegressionStressScenario({
      session,
      surfaces,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "layout-measurement-regression-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Layout measurement regression stress collected layout component bounds, input/list/content/window geometry, ownership receipts, and cleanup without relying on screenshots"
        : "Layout measurement regression stress failed; inspect proofBundle.layoutMeasurementRegression for missing measurement receipts",
      proofBundle,
    };
    break;
  }

  case "screenshot-semantics-visual-consistency-stress": {
    const proofBundle = await runScreenshotSemanticsVisualConsistencyStressScenario({
      session,
      group,
      caseId,
      surface,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "screenshot-semantics-visual-consistency-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Screenshot, capture identity, state, elements, selected row, focus, footer, and semantic visible text receipts agree"
        : "Screenshot-to-semantics consistency failed; inspect proofBundle.visualConsistency.failures",
      proofBundle,
    };
    break;
  }

  case "modal-stack-arbitration-stress": {
    const proofBundle = await runModalStackArbitrationStressScenario({ session, host });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "modal-stack-arbitration-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Modal stack arbitration stress failed closed; topmost-owner key routing and parent focus/selection receipts are missing",
      proofBundle,
    };
    break;
  }

  case "cross-surface-export-provenance-stress": {
    const proofBundle = await runCrossSurfaceExportProvenanceStressScenario({
      session,
      source,
      destination,
      exportMode,
      query,
      range,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "cross-surface-export-provenance-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Cross-surface export provenance stress failed closed; provenance, redaction, destination insertion, and stale-source receipts are missing",
      proofBundle,
    };
    break;
  }

  case "dev-session-recovery-stale-target-stress": {
    const proofBundle = await runDevSessionRecoveryStaleTargetStressScenario({
      session,
      entry,
      kind,
      index,
      restartMode,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "dev-session-recovery-stale-target-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Dev/session restart recovery blocked stale-target input and re-resolved the exact target"
        : "Dev/session restart recovery failed; inspect proofBundle.sessionRecovery",
      proofBundle,
    };
    break;
  }

  case "menu-syntax-ambiguity-diagnostics-stress": {
    const proofBundle = await runMenuSyntaxAmbiguityDiagnosticsStressScenario({
      session,
      query,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "menu-syntax-ambiguity-diagnostics-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Menu syntax ambiguity diagnostics stress failed closed; parse diagnostics, skipped fragments, selection identity, and execution guard receipts are missing",
      proofBundle,
    };
    break;
  }

  case "ime-composition-input-boundary-stress": {
    const proofBundle = await runImeCompositionInputBoundaryStressScenario({ session });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "ime-composition-input-boundary-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "IME composition input boundary stress failed closed; composition lifecycle, premature action guards, and committed text receipts are missing",
      proofBundle,
    };
    break;
  }

  case "accessibility-selected-text-fallback-stress": {
    const proofBundle = await runAccessibilitySelectedTextFallbackStressScenario({ session });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "accessibility-selected-text-fallback-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Accessibility selected-text fallback stress failed closed; permission, stale-context, redaction, and fallback receipts are missing",
      proofBundle,
    };
    break;
  }

  case "display-migration-visual-bounds-stress": {
    const proofBundle = await runDisplayMigrationVisualBoundsStressScenario({
      session,
      surfaces,
      fromDisplay,
      toDisplay,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "display-migration-visual-bounds-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Display migration visual bounds stress failed closed; display, text bounds, focus/selection, and screenshot semantic alignment receipts are missing",
      proofBundle,
    };
    break;
  }

  case "native-picker-external-return-focus-stress": {
    const proofBundle = await runNativePickerExternalReturnFocusStressScenario({
      session,
      origin: host,
      handoff,
      foreignApp,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "native-picker-external-return-focus-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Native picker external return focus stress failed closed; origin return, focus/selection/cursor restore, and stale/foreign event receipts are missing",
      proofBundle,
    };
    break;
  }

  case "drag-cancel-payload-scope-stress": {
    const proofBundle = await runDragCancelPayloadScopeStressScenario({
      session,
      source,
      hoverTarget,
      cancel,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "drag-cancel-payload-scope-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Drag cancel payload scope stress failed closed; drag session, cancel cleanup, and side-effect boundary receipts are missing",
      proofBundle,
    };
    break;
  }

  case "runtime-appearance-churn-focused-input-stress": {
    const proofBundle = await runRuntimeAppearanceChurnFocusedInputStressScenario({
      session,
      surface,
      churn,
      cycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "runtime-appearance-churn-focused-input-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Runtime appearance churn focused input stress failed closed; focused input continuity, layout metrics, and renderer token receipts are missing",
      proofBundle,
    };
    break;
  }

  case "power-resume-window-generation-stress": {
    const proofBundle = await runPowerResumeWindowGenerationStressScenario({
      session,
      surface,
      event,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "power-resume-window-generation-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Power resume window generation stress failed closed; stale pre-sleep target rejection and post-wake revalidation receipts are missing",
      proofBundle,
    };
    break;
  }

  case "menu-tray-notification-modal-interruption-stress": {
    const proofBundle = await runMenuTrayNotificationModalInterruptionStressScenario({
      session,
      host,
      activeSurface,
      interruptions,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "menu-tray-notification-modal-interruption-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Menu/tray/notification modal interruption stress failed closed; active modal focus, wrong-surface rejection, and interruption receipts are missing",
      proofBundle,
    };
    break;
  }

  case "stream-progress-cancel-visual-stability-stress": {
    const proofBundle = await runStreamProgressCancelVisualStabilityStressScenario({
      session,
      surface,
      updates,
      cancelAt,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "stream-progress-cancel-visual-stability-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Stream progress cancel visual stability stress failed closed; stream identity, monotonic progress, cancel ordering, stale chunk rejection, and screenshot revalidation receipts are missing",
      proofBundle,
    };
    break;
  }

  case "dictation-media-permission-readiness-churn-stress": {
    const proofBundle = await runDictationMediaPermissionReadinessChurnStressScenario({
      session,
      target,
      churn,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "dictation-media-permission-readiness-churn-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Dictation media permission readiness churn stress failed closed; passive setup, readiness generation, target identity, and no auto-submit receipts are missing",
      proofBundle,
    };
    break;
  }

  case "animation-frame-capture-determinism-stress": {
    const proofBundle = await runAnimationFrameCaptureDeterminismStressScenario({
      session,
      surfaces,
      frames,
      intervalMs,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "animation-frame-capture-determinism-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Animation frame capture determinism stress failed closed; frame identity, per-frame receipts, occlusion, and stale-frame rejection receipts are missing",
      proofBundle,
    };
    break;
  }

  case "accessibility-tree-semantic-parity-stress": {
    const proofBundle = await runAccessibilityTreeSemanticParityStressScenario({
      session,
      surfaces,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "accessibility-tree-semantic-parity-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Accessibility tree semantic parity stress failed closed; role, label, focus order, activation, AX tree, and screenshot-to-semantics receipts are missing",
      proofBundle,
    };
    break;
  }

  case "rtl-bidi-emoji-text-rendering-stress": {
    const proofBundle = await runRtlBidiEmojiTextRenderingStressScenario({
      session,
      surface,
      text,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "rtl-bidi-emoji-text-rendering-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "RTL/bidi/emoji text rendering stress failed closed; direction run, grapheme, cursor, selection, filter, and layout receipts are missing",
      proofBundle,
    };
    break;
  }

  case "high-volume-virtualized-list-stability-stress": {
    const proofBundle = await runHighVolumeVirtualizedListStabilityStressScenario({
      session,
      surface,
      fixtureCount,
      filterCycles,
      scrollCycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "high-volume-virtualized-list-stability-stress",
      status: proofBundle.status,
      steps: proofBundle.steps as StepReceipt[],
      summary: "High-volume virtualized list stability stress failed closed; row identity, reanchor, scroll/filter generation, and screenshot-to-semantics receipts are missing",
      proofBundle,
    };
    break;
  }

  case "input-modality-transition-ownership-stress": {
    const proofBundle = await runInputModalityTransitionOwnershipStressScenario({
      session,
      surface,
      interleave,
      cycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "input-modality-transition-ownership-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Input modality transition ownership stress failed closed; modality generation, hover/focus/selection, scroll, shortcut, and activation receipts are missing",
      proofBundle,
    };
    break;
  }

  case "multi-context-attachment-dedupe-provenance-stress": {
    const proofBundle = await runMultiContextAttachmentDedupeProvenanceStressScenario({
      session,
      origins,
      destinations,
      reorderCycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "multi-context-attachment-dedupe-provenance-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Multi-context attachment dedupe/provenance stress failed closed; cross-host origin, dedupe, provenance, reorder, privacy, and stale rejection receipts are missing",
      proofBundle,
    };
    break;
  }

  case "visual-contrast-readable-state-stress": {
    const proofBundle = await runVisualContrastReadableStateStressScenario({
      session,
      surfaces,
      themes,
      scaleFactors,
      states,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "visual-contrast-readable-state-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Visual contrast readable-state stress collected theme token foreground/background contrast ratios, visible main-window semantics, and cleanup without relying on screenshots"
        : "Visual contrast readable-state stress failed; inspect proofBundle.visualContrastReadableState for missing contrast or visible-surface receipts",
      proofBundle,
    };
    break;
  }

  case "empty-error-retry-state-ux-stress": {
    const proofBundle = await runEmptyErrorRetryStateUxStressScenario({
      session,
      surfaces,
      query,
      retryCycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "empty-error-retry-state-ux-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      reasonCode: proofBundle.reasonCode,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Empty/error/retry state UX stress failed closed; empty, loading, error, retry, and recovery receipts are missing",
      proofBundle,
    };
    break;
  }

  case "form-validation-inline-recovery-stress": {
    const proofBundle = await runFormValidationInlineRecoveryStressScenario({
      session,
      surface,
      fields,
      invalid,
      valid,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "form-validation-inline-recovery-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      reasonCode: proofBundle.reasonCode,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Form validation inline recovery stress failed closed; inline errors, first invalid focus, input preservation, valid edit recovery, and submit guard receipts are missing",
      proofBundle,
    };
    break;
  }

  case "navigation-back-stack-history-stress": {
    const proofBundle = await runNavigationBackStackHistoryStressScenario({
      session,
      origin: originSurface,
      surfaces,
      transitions,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "navigation-back-stack-history-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      reasonCode: proofBundle.reasonCode,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Navigation/back-stack history stress failed closed; route stack, actions discoverability, no-op affordance, return-to-origin, and stale state receipts are missing",
      proofBundle,
    };
    break;
  }

  case "long-text-wrap-resize-surface-stress": {
    const proofBundle = await runLongTextWrapResizeSurfaceStressScenario({
      session,
      surfaces,
      widths,
      fixtures,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "long-text-wrap-resize-surface-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Long text wrapping/resizing UX stress measured visible text bounds, widths, overlap, footer/input collision fields, and cleanup without relying on screenshots"
        : "Long text wrapping/resizing UX stress failed; inspect proofBundle.longTextWrapResizeSurface for missing or failing measurement receipts",
      proofBundle,
    };
    break;
  }

  case "div-container-scroll-overflow-stress": {
    const proofBundle = await runDivContainerScrollOverflowStressScenario({
      session,
      itemCount,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "div-container-scroll-overflow-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Div container overflow stress opened a real DivPrompt, measured DivContent bounds, proved tall content requires scrolling, and cleaned up without screenshots"
        : "Div container overflow stress failed; inspect proofBundle.divContainerScrollOverflow for missing DivContent or overflow receipts",
      proofBundle,
    };
    break;
  }

  case "main-menu-dynamic-choice-resize-stress": {
    const proofBundle = await runMainMenuDynamicChoiceResizeStressScenario({
      session,
      smallCount,
      largeCount,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "main-menu-dynamic-choice-resize-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Main menu dynamic choice resize stress measured small and large choice prompts, visible counts, height growth, stable width, and cleanup"
        : "Main menu dynamic choice resize stress failed; inspect proofBundle.mainMenuDynamicChoiceResize for missing count or bounds receipts",
      proofBundle,
    };
    break;
  }

  case "notes-window-resize-stress": {
    const proofBundle = await runNotesWindowResizeStressScenario({
      session,
      shortLineCount,
      tallLineCount,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "notes-window-resize-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Notes window resize stress used a sandbox notes DB, drove targeted Notes setInput, measured grow/shrink bounds, and cleaned up without screenshots"
        : "Notes window resize stress failed; inspect proofBundle.notesWindowResize for missing sandbox, bounds, grow, shrink, or cleanup receipts",
      proofBundle,
    };
    break;
  }

  case "notes-delete-confirm-route-proof": {
    const proofBundle = await runNotesDeleteConfirmRouteProofScenario({
      session,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "notes-delete-confirm-route-proof",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Notes delete confirm route used a sandbox Notes DB, real Notes shortcuts, attached confirm-popup, cancel button, and DB mutation check"
        : "Notes delete confirm route failed; inspect proofBundle.notesDeleteConfirmRoute for missing popup identity, button, focus, cancel, or sandbox DB receipts",
      proofBundle,
    };
    break;
  }

  case "actions-command-discoverability-noop-stress": {
    const proofBundle = await runActionsCommandDiscoverabilityNoopStressScenario({
      session,
      hosts,
      states,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "actions-command-discoverability-noop-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: proofBundle.status === "pass"
        ? "Actions command discoverability stress opened real Cmd-K action popups, measured visible action rows, and closed without executing actions"
        : "Actions command discoverability stress failed; inspect proofBundle.actionsCommandDiscoverabilityNoop for missing popup action receipts",
      proofBundle,
    };
    break;
  }

  case "dense-list-detail-preview-readability-stress": {
    const proofBundle = await runDenseListDetailPreviewReadabilityStressScenario({
      session,
      surfaces,
      query,
      filterCycles,
      selectionCycles,
      resizeCycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "dense-list-detail-preview-readability-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Dense list/detail preview readability stress failed closed; row-preview identity, metadata chip, footer, filter, selection, and resize receipts are missing",
      proofBundle,
    };
    break;
  }

  case "toast-notification-queue-lifecycle-stress": {
    const proofBundle = await runToastNotificationQueueLifecycleStressScenario({
      session,
      surface,
      fixtures,
      cycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "toast-notification-queue-lifecycle-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Toast notification queue lifecycle stress failed closed; queue, bridge, duplicate, autohide, bounds, stale rejection, and no-action receipts are missing",
      proofBundle,
    };
    break;
  }

  case "destructive-confirm-modal-safety-stress": {
    const proofBundle = await runDestructiveConfirmModalSafetyStressScenario({
      session,
      host,
      fixture,
      paths,
      dryRunOnly,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "destructive-confirm-modal-safety-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Destructive confirm modal safety stress failed closed; dry-run prompt identity, Enter/Escape, restore, stale rejection, and no-system-command receipts are missing",
      proofBundle,
    };
    break;
  }

  case "confirm-modal-style-preview-proof": {
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "confirm-modal-style-preview-proof",
      status: "error",
      failClosed: true,
      failureMode: "missing_scenario_owner",
      missingReceipt: "confirm_modal_style_preview_scenario",
      reasonCode: "missing_confirm_modal_style_preview_scenario",
      steps: [{
        name: "confirm-modal-style-preview-owner",
        status: "error",
        output: {
          code: "missing_confirm_modal_style_preview_scenario",
          message: "The requested scenario owner is not implemented.",
        },
        durationMs: 0,
      }],
      summary: "Confirm Modal style preview cannot run because its scenario owner is missing; no runtime interaction was attempted.",
    };
    break;
  }

  case "sdk-confirm-runtime-proof": {
    const proofBundle = await runSdkConfirmRuntimeProofScenario({
      session,
      cancel: "escape",
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "sdk-confirm-runtime-proof",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "SDK confirm runtime proof drives global confirm() through the script run route and verifies shared ConfirmPrompt cancellation returns boolean false",
      proofBundle,
    };
    break;
  }

  case "loading-skeleton-progress-restoration-stress": {
    const proofBundle = await runLoadingSkeletonProgressRestorationStressScenario({
      session,
      surfaces,
      fixture,
      cycles,
    });
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe: "loading-skeleton-progress-restoration-stress",
      status: proofBundle.status,
      failClosed: proofBundle.failClosed,
      failureMode: proofBundle.failureMode,
      missingReceipt: proofBundle.missingReceipt,
      linearIssue: proofBundle.linearIssue,
      steps: proofBundle.steps as StepReceipt[],
      summary: "Loading skeleton/progress restoration stress failed closed; request/result generation, skeleton, progress, stale rejection, and restore receipts are missing",
      proofBundle,
    };
    break;
  }

  case "icon-image-fallback-redaction-stress": {
    const proofBundle = await runIconImageFallbackRedactionStressScenario({ session, surfaces, fixtures });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "icon-image-fallback-redaction-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Icon/image fallback redaction stress failed closed; fallback, redaction, stale image, accessible label, and no-leak receipts are missing", proofBundle };
    break;
  }

  case "footer-status-persistence-stress": {
    const proofBundle = await runFooterStatusPersistenceStressScenario({ session, surfaces, transitions });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "footer-status-persistence-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Footer/status persistence stress failed closed; owner, generation, transition persistence, duplicate rejection, and stale status receipts are missing", proofBundle };
    break;
  }

  case "keyboard-hint-label-parity-stress": {
    const proofBundle = await runKeyboardHintLabelParityStressScenario({ session, surfaces, families });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "keyboard-hint-label-parity-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Keyboard hint label parity stress failed closed; footer, row, tooltip, catalog, glyph, disabled parity, and activation owner receipts are missing", proofBundle };
    break;
  }

  case "row-state-parity-without-pointer-stress": {
    const proofBundle = await runRowStateParityWithoutPointerStressScenario({ session, surfaces, states });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "row-state-parity-without-pointer-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Row state parity without pointer stress failed closed; selected/focused/hover paint, precedence, stale rejection, and no-native-pointer receipts are missing", proofBundle };
    break;
  }

  case "quiet-chrome-card-nesting-stress": {
    const proofBundle = await runQuietChromeCardNestingStressScenario({ session, surfaces, chrome });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "quiet-chrome-card-nesting-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Quiet chrome/card nesting stress failed closed; layer token, card depth, duplicate border, opaque fill, and stale chrome receipts are missing", proofBundle };
    break;
  }

  case "scroll-shadow-sticky-header-density-stress": {
    const proofBundle = await runScrollShadowStickyHeaderDensityStressScenario({ session, surfaces, scrollPositions, density });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "scroll-shadow-sticky-header-density-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Scroll shadow/sticky header/density stress failed closed; scroll bounds, sticky header, shadow token, density, and footer-safe viewport receipts are missing", proofBundle };
    break;
  }

  case "popup-focus-keycap-visual-semantics-stress": {
    const proofBundle = await runPopupFocusKeycapVisualSemanticsStressScenario({ session, surfaces });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "popup-focus-keycap-visual-semantics-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Popup focus/keycap visual semantics stress failed closed; focus/keycap parity, glyph, topmost, stale rejection, and no-execution receipts are missing", proofBundle };
    break;
  }

  case "reduced-motion-animation-disable-stress": {
    const proofBundle = await runReducedMotionAnimationDisableStressScenario({ session, surfaces, fixture });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "reduced-motion-animation-disable-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Reduced-motion animation disable stress failed closed; fixture policy, animation generation, stable frame, and no-TCC receipts are missing", proofBundle };
    break;
  }

  case "command-search-highlighting-accessory-badges-stress": {
    const proofBundle = await runCommandSearchHighlightingAccessoryBadgesStressScenario({ session, hosts, query });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "command-search-highlighting-accessory-badges-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Command search highlighting/accessory badges stress failed closed; highlight ranges, badge ordering, action-catalog parity, and stale rejection receipts are missing", proofBundle };
    break;
  }

  case "clipboard-copy-visual-feedback-stress": {
    const proofBundle = await runClipboardCopyVisualFeedbackStressScenario({ session, hosts, fixture, pasteboardScope, noSystemPasteboard });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "clipboard-copy-visual-feedback-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Clipboard copy visual feedback stress failed closed; fixture pasteboard, visible copied state, toast, redaction, and stale copy receipts are missing", proofBundle };
    break;
  }

  case "portal-cancel-return-state-restoration-stress": {
    const proofBundle = await runPortalCancelReturnStateRestorationStressScenario({ session, origins, portal, query, cancelMethods, fixture, noNativePicker });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "portal-cancel-return-state-restoration-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Portal cancel return state restoration stress failed closed; origin draft, cursor, selection, filter, scroll, and no-insert receipts are missing", proofBundle };
    break;
  }

  case "tooltip-hover-focus-affordance-stress": {
    const proofBundle = await runTooltipHoverFocusAffordanceStressScenario({ session, surfaces, targets, fixture, inputModes, noNativePointer });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "tooltip-hover-focus-affordance-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Tooltip hover/focus affordance stress failed closed; protocol hover, keyboard focus, placement, dismissal, and no-cover receipts are missing", proofBundle };
    break;
  }

  case "shortcut-recorder-cancel-layering-stress": {
    const proofBundle = await runShortcutRecorderCancelLayeringStressScenario({ session, surface, action, cancelMethods, inputModes, sandboxConfig, noConfigWrite });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "shortcut-recorder-cancel-layering-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Shortcut recorder cancel/layering stress failed closed; modal layering, cancel paths, config unchanged, and focus restore receipts are missing", proofBundle };
    break;
  }

  case "inline-popover-anchor-resize-stress": {
    const proofBundle = await runInlinePopoverAnchorResizeStressScenario({ session, families, widths, fixture, inputModes, noNativeInput });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "inline-popover-anchor-resize-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Inline popover anchor/resize stress failed closed; anchor, resize, clipping, z-order, keyboard, and capture receipts are missing", proofBundle };
    break;
  }

  case "disabled-footer-hit-target-refusal-stress": {
    const proofBundle = await runDisabledFooterHitTargetRefusalStressScenario({ session, surfaces, fixtures, inputModes, noNativePointer, dryRunOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "disabled-footer-hit-target-refusal-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Disabled footer hit-target refusal stress failed closed; disabled reason, refused activation, no submit, and state preservation receipts are missing", proofBundle };
    break;
  }

  case "mini-full-transition-layout-continuity-stress": {
    const proofBundle = await runMiniFullTransitionLayoutContinuityStressScenario({ session, surfaces, transitions, fixture, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "mini-full-transition-layout-continuity-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Mini/full transition layout continuity stress failed closed; bounds, rem/scale, focus ring, footer, clipping, and capture receipts are missing", proofBundle };
    break;
  }

  case "filter-input-decoration-chip-layout-stress": {
    const proofBundle = await runFilterInputDecorationChipLayoutStressScenario({ session, surfaces, queries, widths, scaleFactors, fixture, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noConfigWrite, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "filter-input-decoration-chip-layout-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Filter input decoration chip layout stress failed closed; chip ranges, bounds, overlap, clipping, cursor, and stale decoration receipts are missing", proofBundle };
    break;
  }

  case "focus-ring-viewport-integrity-stress": {
    const proofBundle = await runFocusRingViewportIntegrityStressScenario({ session, surfaces, fixture, inputModes, steps: proofSteps, noNativeInput, noNativePointer, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "focus-ring-viewport-integrity-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Focus ring viewport integrity stress failed closed; focus bounds, viewport clipping, footer/popup occlusion, tab order, and no-submit receipts are missing", proofBundle };
    break;
  }

  case "warning-banner-action-dismiss-semantics-stress": {
    const proofBundle = await runWarningBannerActionDismissSemanticsStressScenario({ session, surface, fixtures, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noConfigWrite, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "warning-banner-action-dismiss-semantics-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Warning banner action/dismiss semantics stress failed closed; banner state, action-vs-dismiss, contrast, and obstruction receipts are missing", proofBundle };
    break;
  }

  case "select-prompt-multiselect-keyboard-state-stress": {
    const proofBundle = await runSelectPromptMultiselectKeyboardStateStressScenario({ session, surface, fixture, choices, selectionSteps, inputModes, noNativeInput, noNativePointer, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "select-prompt-multiselect-keyboard-state-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "SelectPrompt multiselect keyboard state stress failed closed; checked rows, state parity, filter preservation, and no-submit receipts are missing", proofBundle };
    break;
  }

  case "file-search-preview-sanitization-stress": {
    const proofBundle = await runFileSearchPreviewSanitizationStressScenario({ session, surface, fixture, previewFixtures, selectionCycles, filterCycles, inputModes, noNativeInput, noNativePointer, noNativePicker, noQuickLook, noSystemPasteboard, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "file-search-preview-sanitization-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "File Search preview sanitization stress failed closed; preview identity, redaction, fallback, no external handoff, and stale preview receipts are missing", proofBundle };
    break;
  }

  case "hotkey-prompt-transient-capture-cancel-stress": {
    const proofBundle = await runHotkeyPromptTransientCaptureCancelStressScenario({ session, surface, fixture, chords, cancelMethods, inputModes, noNativeInput, noNativePointer, noConfigWrite, noGlobalHotkeyRegistration, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "hotkey-prompt-transient-capture-cancel-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "HotkeyPrompt transient capture/cancel stress failed closed; capture, cancel, config fingerprint, global hotkey, and focus restore receipts are missing", proofBundle };
    break;
  }

  case "process-manager-sort-detail-panel-stability-stress": {
    const proofBundle = await runProcessManagerSortDetailPanelStabilityStressScenario({ session, surface, fixture, sortKeys, selectionCycles, filterCycles, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noProcessKill, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "process-manager-sort-detail-panel-stability-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Process Manager sort/detail panel stability stress failed closed; sort headers, section rows, detail source, kill disable, and stale detail receipts are missing", proofBundle };
    break;
  }

  case "env-prompt-redacted-status-error-recovery-stress": {
    const proofBundle = await runEnvPromptRedactedStatusErrorRecoveryStressScenario({ session, surface, fixture, statusFixtures, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noConfigWrite, noSecretWrite, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "env-prompt-redacted-status-error-recovery-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "EnvPrompt redacted status/error recovery stress failed closed; redaction, inline error, disabled submit, valid recovery, and no-secret-write receipts are missing", proofBundle };
    break;
  }

  case "command-palette-breadcrumb-route-stack-stress": {
    const proofBundle = await runCommandPaletteBreadcrumbRouteStackStressScenario({ session, host, fixture, drillPath, filter, backMethods, inputModes, noNativeInput, noNativePointer, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "command-palette-breadcrumb-route-stack-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Command palette breadcrumb route-stack stress failed closed; breadcrumb, drill-down, Escape/back, parent restore, and no-execution receipts are missing", proofBundle };
    break;
  }

  case "root-source-chip-action-semantics-stress": {
    const proofBundle = await runRootSourceChipActionSemanticsStressScenario({ session, queries, actions: chipActions, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noConfigWrite, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "root-source-chip-action-semantics-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Root source-chip action semantics stress failed closed; chip remove/clear/exclude, decorations, status-chip refusal, and stale action receipts are missing", proofBundle };
    break;
  }

  case "recent-history-dedupe-root-grouping-stress": {
    const proofBundle = await runRecentHistoryDedupeRootGroupingStressScenario({ session, fixture, sources, query, cycles, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "recent-history-dedupe-root-grouping-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Recent/history dedupe root grouping stress failed closed; source grouping, dedupe keys, metadata-only rows, stale passive publish, and selection stability receipts are missing", proofBundle };
    break;
  }

  case "inline-attachment-preview-chip-stability-stress": {
    const proofBundle = await runInlineAttachmentPreviewChipStabilityStressScenario({ session, hosts, fixture, origins, chipActions, inputModes, noNativeInput, noNativePointer, noNativePicker, noScreenCapture, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "inline-attachment-preview-chip-stability-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Inline attachment preview chip stability stress failed closed; chip focus/preview/remove/reorder, redaction, overflow, and no-leak receipts are missing", proofBundle };
    break;
  }

  case "window-title-status-semantics-stress": {
    const proofBundle = await runWindowTitleStatusSemanticsStressScenario({ session, surfaces, states, transitions, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "window-title-status-semantics-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Window title/status semantics stress failed closed; native title, semantic title, status generation, parity, and stale title/status receipts are missing", proofBundle };
    break;
  }

  case "menu-syntax-capture-validation-chip-stress": {
    const proofBundle = await runMenuSyntaxCaptureValidationChipStressScenario({ session, fixture, cases, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "menu-syntax-capture-validation-chip-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Menu syntax capture validation chip stress failed closed; status chips, missing/malformed/unresolved validation, no-submit, and no-payload receipts are missing", proofBundle };
    break;
  }

  case "agent_chat-footer-activity-indicator-stress": {
    const proofBundle = await runAgentChatFooterActivityIndicatorStressScenario({ session, hosts, fixture, activityFixtures, inputModes, agentFixture, noNativeInput, noNativePointer, noSecurityPrompts, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "agent_chat-footer-activity-indicator-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Agent Chat footer activity indicator stress failed closed; footer dot status, activity transitions, repaint generation, and no-security-prompt receipts are missing", proofBundle };
    break;
  }

  case "agent_chat-model-history-popover-visual-state-stress": {
    const proofBundle = await runAgentChatModelHistoryPopoverVisualStateStressScenario({ session, families, fixture, states, selectionCycles, filterCycles, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "agent_chat-model-history-popover-visual-state-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Agent Chat model/history popover visual state stress failed closed; row visual states, badges, filtered/loading/error recovery, and redacted preview receipts are missing", proofBundle };
    break;
  }

  case "agent_chat-context-insertion-preview-parity-stress": {
    const proofBundle = await runAgentChatContextInsertionPreviewParityStressScenario({ session, sources, destination, fixture, selectionCycles, filterCycles, insertModes, inputModes, noNativeInput, noNativePointer, noNativePicker, noQuickLook, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "agent_chat-context-insertion-preview-parity-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Agent Chat context insertion preview parity stress failed closed; selected-row preview, accepted context URI, inserted token preview, and stale drift receipts are missing", proofBundle };
    break;
  }

  case "agent_chat-composer-token-keyboard-edit-parity-stress": {
    const proofBundle = await runAgentChatComposerTokenKeyboardEditParityStressScenario({ session, hosts, fixture, tokenKinds, editSteps, inputModes, noNativeInput, noNativePointer, noNativePicker, noScreenCapture, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "agent_chat-composer-token-keyboard-edit-parity-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Agent Chat composer token keyboard edit parity stress failed closed; atomic delete, reorder, cursor, metadata preservation, and stale composer receipts are missing", proofBundle };
    break;
  }

  case "agent_chat-transcript-stream-retry-virtualization-stress": {
    const proofBundle = await runAgentChatTranscriptStreamRetryVirtualizationStressScenario({ session, hosts, fixture, messageCount, streamChunks, errorFixtures, retryPaths, scrollPositions, inputModes, agentFixture, noNativeInput, noNativePointer, noSecurityPrompts, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "agent_chat-transcript-stream-retry-virtualization-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Agent Chat transcript stream retry virtualization stress failed closed; stream ordering, virtualized rows, retry recovery, and stale chunk receipts are missing", proofBundle };
    break;
  }

  case "agent_chat-plugin-skill-entry-thread-affinity-stress": {
    const proofBundle = await runAgentChatPluginSkillEntryThreadAffinityStressScenario({ session, hosts, fixture, skillId, entryPaths, inputModes, agentFixture, noNativeInput, noNativePointer, noSecurityPrompts, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "agent_chat-plugin-skill-entry-thread-affinity-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Agent Chat plugin skill entry thread affinity stress failed closed; skill entry, target thread, slash token, and stale thread receipts are missing", proofBundle };
    break;
  }

  case "notes-cart-agent_chat-handoff-dedupe-stress": {
    const proofBundle = await runNotesCartAgentChatHandoffDedupeStressScenario({ session, fixture, notes, cartItems, handoffPaths, inputModes, agentFixture, noNativeInput, noNativePointer, noNativePicker, noScreenCapture, noSystemPasteboard, noNetwork, noSubmit, sandboxNotesStore, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "notes-cart-agent_chat-handoff-dedupe-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Notes cart Agent Chat handoff dedupe stress failed closed; cart dedupe, handoff, cancel, stale note, and no-leak receipts are missing", proofBundle };
    break;
  }

  case "root-file-source-filter-pagination-footer-stress": {
    const proofBundle = await runRootFileSourceFilterPaginationFooterStressScenario({ session, fixture, queries, pageSize, providerDelays, selectionSteps, inputModes, noNativeInput, noNativePointer, noNativePicker, noQuickLook, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "root-file-source-filter-pagination-footer-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Root file source-filter pagination footer stress failed closed; page generation, selected row, footer-safe scroll, and stale page receipts are missing", proofBundle };
    break;
  }

  case "file-search-directory-breadcrumb-restoration-stress": {
    const proofBundle = await runFileSearchDirectoryBreadcrumbRestorationStressScenario({ session, fixture, startDir, queries, navigationSteps, inputModes, noNativeInput, noNativePointer, noNativePicker, noQuickLook, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "file-search-directory-breadcrumb-restoration-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "File Search directory breadcrumb restoration stress failed closed; breadcrumb, filter, selection, preview, and stale directory receipts are missing", proofBundle };
    break;
  }

  case "emoji-picker-skin-tone-category-ux-stress": {
    const proofBundle = await runEmojiPickerSkinToneCategoryUxStressScenario({ session, fixture, categories, queries, skinTones, steps: proofSteps, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "emoji-picker-skin-tone-category-ux-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Emoji Picker skin-tone/category UX stress failed closed; category, palette, variant, grapheme, and stale palette receipts are missing", proofBundle };
    break;
  }

  case "root-window-source-filter-activation-refusal-stress": {
    const proofBundle = await runRootWindowSourceFilterActivationRefusalStressScenario({ session, fixture, queries, windowStates, selectionSteps, inputModes, windowProvider, noNativeInput, noNativePointer, noWindowActivation, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "root-window-source-filter-activation-refusal-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Root Window source-filter activation refusal stress failed closed; window rows, selection, dry-run activation, and no-focus-steal receipts are missing", proofBundle };
    break;
  }

  case "notes-markdown-preview-scroll-sync-stress": {
    const proofBundle = await runNotesMarkdownPreviewScrollSyncStressScenario({ session, fixture, notes, markdownFixtures, editSteps, inputModes, sandboxNotesStore, noNativeInput, noNativePointer, noNativePicker, noSystemPasteboard, noNetwork, noExternalServices, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "notes-markdown-preview-scroll-sync-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Notes markdown preview scroll sync stress failed closed; preview block, editor focus, scroll, split, and stale preview receipts are missing", proofBundle };
    break;
  }

  case "quick-terminal-ansi-scrollback-search-stress": {
    const proofBundle = await runQuickTerminalAnsiScrollbackSearchStressScenario({ session, fixture, transcriptFixtures, searchQueries, scrollPositions, inputModes, terminalFixture, noShellCommand, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noExternalServices, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "quick-terminal-ansi-scrollback-search-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Quick Terminal ANSI scrollback search stress failed closed; ANSI, wide text, search hit, hyperlink redaction, and no-shell receipts are missing", proofBundle };
    break;
  }

  case "script-output-inspector-folding-recovery-stress": {
    const proofBundle = await runScriptOutputInspectorFoldingRecoveryStressScenario({ session, fixture, outputFixtures, viewSteps, inputModes, scriptFixture, noHandlerSpawn, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noExternalServices, noProcessKill, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "script-output-inspector-folding-recovery-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Script output inspector folding recovery stress failed closed; stream, folding, retry, scroll, and no-spawn receipts are missing", proofBundle };
    break;
  }

  case "app-launcher-icon-grid-keyboard-navigation-stress": {
    const proofBundle = await runAppLauncherIconGridKeyboardNavigationStressScenario({ session, fixture, apps, gridStates, navigationSteps, inputModes, noNativeInput, noNativePointer, noAppLaunch, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "app-launcher-icon-grid-keyboard-navigation-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "App Launcher icon-grid keyboard navigation stress failed closed; icon bounds, selection, neighbor map, preview, and no-launch receipts are missing", proofBundle };
    break;
  }

  case "browser-history-time-grouped-privacy-stress": {
    const proofBundle = await runBrowserHistoryTimeGroupedPrivacyStressScenario({ session, fixture, browserFixtures, timeBuckets, queries, privacyModes, inputModes, browserProvider, noBrowserActivation, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "browser-history-time-grouped-privacy-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Browser History time-grouped privacy stress failed closed; bucket, redaction, favicon fallback, selection, and no-activation receipts are missing", proofBundle };
    break;
  }

  case "settings-preferences-search-reset-preview-stress": {
    const proofBundle = await runSettingsPreferencesSearchResetPreviewStressScenario({ session, fixture, preferenceFixtures, controlTypes, queries, resetPaths, inputModes, sandboxConfig, noConfigWrite, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "settings-preferences-search-reset-preview-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Settings preferences search/reset preview stress failed closed; controls, dirty state, reset preview, and no-write receipts are missing", proofBundle };
    break;
  }

  case "settings-preferences-readonly-detail-panel-stress": {
    const proofBundle = await runSettingsPreferencesReadonlyDetailPanelStressScenario({ session, fixture, sections, queries, navigationSteps, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noExternalServices, noSecurityPrompts, noSystemSettings, noTccMutation, noConfigWrite, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "settings-preferences-readonly-detail-panel-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Settings read-only detail panel stress failed closed; row bounds, detail panel, config fingerprint, and no-write receipts are missing", proofBundle };
    break;
  }

  case "design-picker-preview-restore-visual-stress": {
    const proofBundle = await runDesignPickerPreviewRestoreVisualStressScenario({ session, fixture, catalogIds, previewSteps, inputModes, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noExternalServices, noSecurityPrompts, noSystemSettings, noTccMutation, noConfigWrite, noDesignWrite, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "design-picker-preview-restore-visual-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Design Picker preview restore visual stress failed closed; token fingerprints, preview restore, and no-write receipts are missing", proofBundle };
    break;
  }

  case "dictation-history-transcript-preview-redaction-stress": {
    const proofBundle = await runDictationHistoryTranscriptPreviewRedactionStressScenario({ session, fixture, transcriptFixtures, queries, selectionCycles, viewSteps, inputModes, dictationFixture, noMicrophone, noMediaCapture, noNativeInput, noNativePointer, noSystemPasteboard, noNetwork, noExternalServices, noSecurityPrompts, noSystemSettings, noTccMutation, noSubmit, dryRunOnly, localFixtureOnly });
    result = { schemaVersion: SCHEMA_VERSION, recipe: "dictation-history-transcript-preview-redaction-stress", status: proofBundle.status, failClosed: proofBundle.failClosed, failureMode: proofBundle.failureMode, missingReceipt: proofBundle.missingReceipt, linearIssue: proofBundle.linearIssue, steps: proofBundle.steps as StepReceipt[], summary: "Dictation History transcript preview redaction stress failed closed; preview, redaction, fallback, and no-media receipts are missing", proofBundle };
    break;
  }

  case "agent_chat-setup-recovery":
    result = await recipeAgentChatSetupRecovery(session, selectAgent);
    break;

  case "surface-proof":
    result = await recipeSurfaceProof(session, {
      kind: (kind as SurfaceProofKind | undefined) ?? "main",
      index: index ?? 0,
    });
    break;

  case "scenario": {
    const scenarioName = kind ?? "";
    // Also accept --scenario as an alias for --kind
    const scenarioArg = process.argv.indexOf("--scenario");
    const resolvedScenario =
      scenarioArg >= 0 && process.argv[scenarioArg + 1]
        ? process.argv[scenarioArg + 1]
        : scenarioName;

    switch (resolvedScenario) {
      case "main-window-exact-id": {
        const bundle = await runMainWindowExactIdScenario(session);
        console.log(JSON.stringify(bundle, null, 2));
        process.exit(bundle.warnings.length > 0 ? 1 : 0);
      }
      case "actions-dialog-exact-id": {
        const bundle = await runActionsDialogExactIdScenario(session, index ?? 0);
        console.log(JSON.stringify(bundle, null, 2));
        process.exit(bundle.warnings.length > 0 ? 1 : 0);
      }
      case "prompt-popup-exact-id": {
        const bundle = await runPromptPopupExactIdScenario(session, index ?? 0);
        console.log(JSON.stringify(bundle, null, 2));
        process.exit(bundle.warnings.length > 0 ? 1 : 0);
      }
      case "detached-agent_chat-exact-id": {
        const bundle = await runDetachedAgentChatExactIdScenario(session, index ?? 0);
        console.log(JSON.stringify(bundle, null, 2));
        process.exit(bundle.warnings.length > 0 ? 1 : 0);
      }
      default:
        result = {
          schemaVersion: SCHEMA_VERSION,
          recipe: "scenario",
          status: "error",
          steps: [],
          summary:
            `Unknown scenario: ${resolvedScenario}. Available: main-window-exact-id, actions-dialog-exact-id, prompt-popup-exact-id, detached-agent_chat-exact-id`,
        };
    }
    break;
  }

  case "vision-loop": {
    // Delegate to the standalone vision-loop.ts script.
    // Expects --receipt and --out-dir to be passed after the recipe name.
    const vlArgs = process.argv.slice(3); // everything after "vision-loop"
    const command = ["bun", "scripts/agentic/vision-loop.ts", ...vlArgs];
    assertNoninteractiveSubprocess(command);
    const proc = Bun.spawn(
      command,
      { stdout: "pipe", stderr: "pipe", cwd: PROJECT_ROOT }
    );
    const vlStdout = await new Response(proc.stdout).text();
    const vlStderr = await new Response(proc.stderr).text();
    const vlExit = await proc.exited;
    if (vlStderr) process.stderr.write(vlStderr);
    process.stdout.write(vlStdout);
    process.exit(vlExit);
    break;
  }

  case "surface-navigate": {
    const navArgs = process.argv.slice(3);
    const command = ["bun", "scripts/agentic/surface-navigator.ts", ...navArgs];
    assertNoninteractiveSubprocess(command);
    const proc = Bun.spawn(
      command,
      { stdout: "pipe", stderr: "pipe", cwd: PROJECT_ROOT }
    );
    const navStdout = await new Response(proc.stdout).text();
    const navStderr = await new Response(proc.stderr).text();
    const navExit = await proc.exited;
    if (navStderr) process.stderr.write(navStderr);
    process.stdout.write(navStdout);
    process.exit(navExit);
    break;
  }

  case "help":
  case "--help": {
    const jsonFlag = process.argv.includes("--json");
    if (jsonFlag) {
      const helpJson = {
        schemaVersion: 1,
        script: "index",
        commands: [
          { name: "preflight", description: "Check prerequisites (session, window, permissions)", flags: ["--session", "--json"] },
          { name: "agent_chat-open", description: "Open Agent Chat and verify ready state", flags: ["--session", "--target-json", "--json"] },
          { name: "agent_chat-accept", description: "Full Agent Chat picker accept", flags: ["--session", "--key", "--vision", "--target-json", "--surface", "--json"] },
          { name: "agent_chat-enter-accept", description: "Compatibility alias for --key enter", flags: ["--session", "--vision", "--target-json", "--surface", "--json"] },
          { name: "agent_chat-tab-accept", description: "Compatibility alias for --key tab", flags: ["--session", "--vision", "--target-json", "--surface", "--json"] },
          { name: "agent_chat-detached-accept", description: "One-command detached Agent Chat proof: resolve, accept, identity check", flags: ["--session", "--kind", "--index", "--key", "--vision", "--json"] },
          { name: "agent_chat-detached-target-threading-stress", description: "Multi-window detached Agent Chat proof with exact target threading, native input, and strict capture identity", flags: ["--session", "--kind", "--index", "--min-targets", "--key", "--vision", "--json"] },
          { name: "notes-agent_chat-delayed-action-origin-stress", description: "Fail-closed Notes Agent Chat delayed-action origin/generation stress receipt", flags: ["--session", "--drift", "--json"] },
          { name: "file-portal-origin-roundtrip", description: "Fail-closed Agent Chat portal origin/context-part round-trip receipt", flags: ["--session", "--origin", "--portal", "--selection", "--query", "--json"] },
          { name: "permission-privacy-preflight", description: "Read-only permission preflight that never opens System Settings or mutates OS permissions", flags: ["--session", "--kinds", "--json"] },
          { name: "shortcut-recorder-focus-capture", description: "Fail-closed native shortcut recorder focus/capture receipt", flags: ["--session", "--surface", "--action", "--chord", "--sandbox-config", "--json"] },
          { name: "template-prompt-automation-parity-stress", description: "State-first TemplatePrompt state/elements/actions/submit/cancel/forceSubmit parity receipt", flags: ["--session", "--template", "--field", "--value", "--forced-value", "--json"] },
          { name: "current-app-commands-frontmost-stress", description: "Fail-closed Do in Current App frontmost snapshot and shared filtering receipt", flags: ["--session", "--alias", "--query", "--expected-app", "--json"] },
          { name: "actions-captured-subject-frame-stress", description: "Fail-closed root actions captured-subject and source-frame stability receipt", flags: ["--session", "--source", "--action", "--mutation", "--json"] },
          { name: "drop-prompt-native-drop-privacy-stress", description: "Fail-closed DropPrompt native drop privacy/redaction receipt", flags: ["--session", "--file-name", "--size", "--json"] },
          { name: "path-prompt-filesystem-edge-stress", description: "State-first PathPrompt filesystem edge receipt helper", flags: ["--session", "--json"] },
          { name: "screenshot-identity-agent_chat-context-stress", description: "Fail-closed screenshotIdentity to Agent Chat context threading receipt", flags: ["--session", "--source", "--json"] },
          { name: "clipboard-history-portal-range-stress", description: "Fail-closed Clipboard History portal host/range receipt", flags: ["--session", "--portal-id", "--range", "--json"] },
          { name: "browser-tabs-cache-identity-stress", description: "Fail-closed browser tabs/history cache identity receipt", flags: ["--session", "--source", "--json"] },
          { name: "scroll-selection-reanchor-stress", description: "Fail-closed cross-surface scroll selection reanchor receipt", flags: ["--session", "--kinds", "--json"] },
          { name: "permission-assistant-drag-preflight-stress", description: "Fail-closed Permission Assistant passive drag-source and no-TCC-mutation receipt", flags: ["--session", "--pane", "--bundle-id", "--json"] },
          { name: "quick-terminal-pty-apply-back-stress", description: "Fail-closed Quick Terminal PTY readiness/output/apply-back cleanup receipt", flags: ["--session", "--command", "--json"] },
          { name: "mcp-context-resource-attachment-identity-stress", description: "Fail-closed MCP context resource URI/profile/context-part identity receipt", flags: ["--session", "--resource-uri", "--profile", "--source", "--json"] },
          { name: "settings-theme-hot-reload-stress", description: "Fail-closed Settings/theme config identity, token fingerprint, repaint, and cleanup receipt", flags: ["--session", "--theme-before", "--theme-after", "--config-key", "--sandbox-config", "--json"] },
          { name: "file-search-drag-out-identity-stress", description: "Fail-closed File Search selected URI, drag payload, host refusal, privacy, and return receipt", flags: ["--session", "--query", "--file-name", "--drop-target", "--json"] },
          { name: "scriptlet-bundle-execution-matrix-stress", description: "Fail-closed scriptlet id, bundle hash, args/env isolation, output, cancellation, and bleed receipt", flags: ["--session", "--scriptlet-id", "--bundle-id", "--cancel-after-ms", "--json"] },
          { name: "tray-global-hotkey-menu-mutation-stress", description: "Fail-closed tray menu/global-hotkey mutation receipt for section order, update state, action identity, duplicate guards, and hotkey route", flags: ["--session", "--loops", "--json"] },
          { name: "multi-window-resize-monitor-restoration-stress", description: "Fail-closed multi-window monitor/scale/resize restoration receipt for main, attached popup, detached Agent Chat, and Notes windows", flags: ["--session", "--surfaces", "--monitor-profile", "--json"] },
          { name: "agent_chat-targeted-dictation-delivery-stress", description: "Fail-closed Agent Chat-targeted dictation delivery receipt for target identity, transcript generation, cursor insertion range, wrong-window guard, and passive setup", flags: ["--session", "--kind", "--index", "--transcript", "--json"] },
          { name: "clipboard-share-trust-install-stress", description: "Fail-closed clipboard share trust/install receipt for prompt identity, package fingerprint, accept/refuse, install gate, and clipboard restoration", flags: ["--session", "--fixture-id", "--share-kind", "--accept-mode", "--json"] },
          { name: "clipboard-share-watcher-stale-replay-stress", description: "Fail-closed clipboard share watcher stale/replay receipt for generation ordering, stale rejection, prompt replacement, and duplicate install guard", flags: ["--session", "--fixture-id", "--share-kind", "--count", "--burst-ms", "--json"] },
          { name: "permission-share-cross-prompt-focus-stress", description: "Fail-closed Permission Assistant/share trust prompt focus receipt for prompt priority, window identity, no Settings activation leak, and cleanup", flags: ["--session", "--fixture-id", "--share-kind", "--pane", "--bundle-id", "--json"] },
          { name: "visible-text-clipping-overlap-stress", description: "Pass-now main-window visible text width, bounds, overlap, and intentional truncation receipt", flags: ["--session", "--surfaces", "--json"] },
          { name: "layout-measurement-regression-stress", description: "Pass-now main-window bounds, scroll/input/footer ownership, and layout measurement receipt", flags: ["--session", "--surfaces", "--json"] },
          { name: "screenshot-semantics-visual-consistency-stress", description: "Pass-now strict screenshot capture plus state/elements semantic consistency proof", flags: ["--session", "--group", "--case", "--surface", "--json"] },
          { name: "modal-stack-arbitration-stress", description: "Fail-closed stacked modal topmost-owner key routing and parent focus/selection restoration receipt", flags: ["--session", "--host", "--json"] },
          { name: "cross-surface-export-provenance-stress", description: "Fail-closed cross-surface export provenance, redaction, destination insertion, and stale-source receipt", flags: ["--session", "--source", "--destination", "--export-mode", "--query", "--range", "--json"] },
          { name: "dev-session-recovery-stale-target-stress", description: "Pass-now stale target session-epoch rejection, exact re-resolution, no stale input, and cleanup receipt", flags: ["--session", "--entry", "--kind", "--index", "--restart-mode", "--json"] },
          { name: "menu-syntax-ambiguity-diagnostics-stress", description: "Fail-closed menu syntax parse diagnostics, skipped fragments, selection identity, and no-execute guard receipt", flags: ["--session", "--query", "--json"] },
          { name: "ime-composition-input-boundary-stress", description: "Fail-closed IME composition lifecycle, premature action guard, and committed text receipt", flags: ["--session", "--json"] },
          { name: "accessibility-selected-text-fallback-stress", description: "Fail-closed selected-text permission fallback, stale-context rejection, redaction, and safe-disable receipt", flags: ["--session", "--json"] },
          { name: "display-migration-visual-bounds-stress", description: "Fail-closed display migration visual/text bounds, focus/selection, capture identity, and wrong-display rejection receipt", flags: ["--session", "--surfaces", "--from-display", "--to-display", "--json"] },
          { name: "native-picker-external-return-focus-stress", description: "Fail-closed native picker/external handoff origin return, focus/selection/cursor restore, and stale/foreign event receipt", flags: ["--session", "--origin", "--handoff", "--foreign-app", "--json"] },
          { name: "drag-cancel-payload-scope-stress", description: "Fail-closed drag cancellation payload scope, hover/drop cleanup, origin restoration, and side-effect boundary receipt", flags: ["--session", "--source", "--hover-target", "--cancel", "--json"] },
          { name: "runtime-appearance-churn-focused-input-stress", description: "Fail-closed focused prompt/Agent Chat appearance churn receipt for scale/font/theme changes", flags: ["--session", "--surface", "--churn", "--cycles", "--json"] },
          { name: "power-resume-window-generation-stress", description: "Fail-closed power resume generation, stale target refusal, and post-wake revalidation receipt", flags: ["--session", "--surface", "--event", "--json"] },
          { name: "menu-tray-notification-modal-interruption-stress", description: "Fail-closed tray/menu/notification interruption active modal focus ownership receipt", flags: ["--session", "--host", "--active-surface", "--interruptions", "--json"] },
          { name: "stream-progress-cancel-visual-stability-stress", description: "Fail-closed stream/progress monotonic repaint, cancellation ordering, stale chunk, and focus return receipt", flags: ["--session", "--surface", "--updates", "--cancel-at", "--json"] },
          { name: "dictation-media-permission-readiness-churn-stress", description: "Fail-closed dictation/media passive setup, readiness generation, target identity, and no auto-submit receipt", flags: ["--session", "--target", "--churn", "--json"] },
          { name: "animation-frame-capture-determinism-stress", description: "Fail-closed animation frame sampling, per-frame state/screenshot, occlusion, and stale-frame rejection receipt", flags: ["--session", "--surfaces", "--frames", "--interval-ms", "--json"] },
          { name: "accessibility-tree-semantic-parity-stress", description: "Fail-closed accessibility role, label, focus order, activation, AX tree, and screenshot-to-semantics parity receipt", flags: ["--session", "--surfaces", "--json"] },
          { name: "rtl-bidi-emoji-text-rendering-stress", description: "Fail-closed RTL/bidi/emoji grapheme, cursor, selection, truncation, and filter semantics receipt", flags: ["--session", "--surface", "--text", "--json"] },
          { name: "high-volume-virtualized-list-stability-stress", description: "Fail-closed virtualized row identity, selection reanchor, scroll/filter generation, and screenshot-to-semantics receipt", flags: ["--session", "--surface", "--fixture-count", "--filter-cycles", "--scroll-cycles", "--json"] },
          { name: "input-modality-transition-ownership-stress", description: "Fail-closed input-device modality transition hover/focus/selection, scroll, shortcut, and activation ownership receipt", flags: ["--session", "--surface", "--interleave", "--cycles", "--json"] },
          { name: "multi-context-attachment-dedupe-provenance-stress", description: "Fail-closed multi-context attachment dedupe, provenance, ordering, and privacy receipt", flags: ["--session", "--origins", "--destinations", "--reorder-cycles", "--json"] },
          { name: "visual-contrast-readable-state-stress", description: "Pass-now theme token contrast/readable-state receipt plus visible main-window semantics", flags: ["--session", "--surfaces", "--themes", "--scale-factors", "--states", "--json"] },
          { name: "empty-error-retry-state-ux-stress", description: "Fail-closed empty/loading/error/retry/recovery UX state receipt", flags: ["--session", "--surfaces", "--query", "--retry-cycles", "--json"] },
          { name: "form-validation-inline-recovery-stress", description: "Fail-closed form validation inline error recovery and submit guard receipt", flags: ["--session", "--surface", "--fields", "--invalid", "--valid", "--json"] },
          { name: "navigation-back-stack-history-stress", description: "Fail-closed navigation/back-stack history restoration and stale state receipt", flags: ["--session", "--origin", "--surfaces", "--transitions", "--json"] },
          { name: "long-text-wrap-resize-surface-stress", description: "Pass-now main-window long text bounds, width, overlap, footer/input collision, and cleanup receipt", flags: ["--session", "--surfaces", "--widths", "--fixtures", "--json"] },
          { name: "div-container-scroll-overflow-stress", description: "Pass-now real DivPrompt tall-container overflow layout and cleanup receipt", flags: ["--session", "--item-count", "--json"] },
          { name: "main-menu-dynamic-choice-resize-stress", description: "Pass-now small/large choice-count window height resize receipt", flags: ["--session", "--small-count", "--large-count", "--json"] },
          { name: "notes-window-resize-stress", description: "Pass-now sandboxed Notes window grow/shrink resize receipt", flags: ["--session", "--short-line-count", "--tall-line-count", "--json"] },
          { name: "notes-delete-confirm-route-proof", description: "Pass-now sandboxed Notes delete shortcut confirm-popup cancel receipt", flags: ["--session", "--json"] },
          { name: "actions-command-discoverability-noop-stress", description: "Pass-now real Cmd-K action popup row measurement with safe no-execution guards", flags: ["--session", "--hosts", "--states", "--json"] },
          { name: "dense-list-detail-preview-readability-stress", description: "Fail-closed UX stress for dense list/detail preview readability during filter, selection, and resize churn", flags: ["--session", "--surfaces", "--query", "--filter-cycles", "--selection-cycles", "--resize-cycles", "--json"] },
          { name: "toast-notification-queue-lifecycle-stress", description: "Fail-closed UX stress for toast queue, notification bridge, duplicate collapse, autohide, dismiss, bounds, and stale rejection", flags: ["--session", "--surface", "--fixtures", "--cycles", "--json"] },
          { name: "destructive-confirm-modal-safety-stress", description: "Fail-closed UX stress for destructive confirm dry-run identity, Enter/Escape safety, parent restore, and no real system command", flags: ["--session", "--host", "--fixture", "--paths", "--dry-run-only", "--json"] },
          { name: "confirm-modal-style-preview-proof", description: "Fail-closed: the Confirm Modal style-preview scenario owner is missing", flags: ["--session", "--json"] },
          { name: "sdk-confirm-runtime-proof", description: "Pass-now SDK confirm script route proof through shared ConfirmPrompt and Escape false result", flags: ["--session", "--json"] },
          { name: "loading-skeleton-progress-restoration-stress", description: "Fail-closed UX stress for local loading skeleton/progress generations, stale rejection, activation blocking, and restoration", flags: ["--session", "--surfaces", "--fixture", "--cycles", "--json"] },
          { name: "icon-image-fallback-redaction-stress", description: "Fail-closed UX stress for icon/image fallback, source redaction, stale image rejection, and accessible labels", flags: ["--session", "--surfaces", "--fixtures", "--json"] },
          { name: "footer-status-persistence-stress", description: "Fail-closed UX stress for footer/status owner, generation, transition persistence, duplicate rejection, and stale status", flags: ["--session", "--surfaces", "--transitions", "--json"] },
          { name: "keyboard-hint-label-parity-stress", description: "Fail-closed UX stress for footer, row, tooltip, and action catalog shortcut hint parity", flags: ["--session", "--surfaces", "--families", "--json"] },
          { name: "row-state-parity-without-pointer-stress", description: "Fail-closed UX receipt contract for row selected/focused/hover state parity without native pointer input", flags: ["--session", "--surfaces", "--states", "--json"] },
          { name: "quiet-chrome-card-nesting-stress", description: "Fail-closed UX receipt contract for quiet chrome/card nesting and visual token budgets", flags: ["--session", "--surfaces", "--chrome", "--json"] },
          { name: "scroll-shadow-sticky-header-density-stress", description: "Fail-closed UX receipt contract for scroll shadows, sticky headers, and density drift", flags: ["--session", "--surfaces", "--scroll-positions", "--density", "--json"] },
          { name: "popup-focus-keycap-visual-semantics-stress", description: "Fail-closed UX receipt contract for popup focus, keycap visual semantics, shortcut glyphs, and parent preservation", flags: ["--session", "--surfaces", "--json"] },
          { name: "reduced-motion-animation-disable-stress", description: "Fail-closed UX receipt contract for fixture-only reduced motion and disabled animation semantics", flags: ["--session", "--surfaces", "--fixture", "--json"] },
          { name: "command-search-highlighting-accessory-badges-stress", description: "Fail-closed UX receipt contract for command search highlighting, accessory badges, and action-catalog parity", flags: ["--session", "--hosts", "--query", "--json"] },
          { name: "clipboard-copy-visual-feedback-stress", description: "Fail-closed UX receipt contract for fixture-scoped copy visual feedback and pasteboard isolation", flags: ["--session", "--hosts", "--fixture", "--pasteboard-scope", "--no-system-pasteboard", "--json"] },
          { name: "portal-cancel-return-state-restoration-stress", description: "Fail-closed UX receipt contract for portal cancel/back origin restoration without insertion", flags: ["--session", "--origins", "--portal", "--query", "--cancel-methods", "--fixture", "--no-native-picker", "--json"] },
          { name: "tooltip-hover-focus-affordance-stress", description: "Fail-closed UX receipt contract for tooltip hover/focus affordances and keyboard fallback", flags: ["--session", "--surfaces", "--targets", "--fixture", "--input-modes", "--no-native-pointer", "--json"] },
          { name: "shortcut-recorder-cancel-layering-stress", description: "Fail-closed UX receipt contract for shortcut recorder cancel paths and modal layering", flags: ["--session", "--surface", "--action", "--cancel-methods", "--input-modes", "--sandbox-config", "--no-config-write", "--json"] },
          { name: "inline-popover-anchor-resize-stress", description: "Fail-closed UX receipt contract for inline popover anchoring, resizing, clipping, and keyboard fallback", flags: ["--session", "--families", "--widths", "--fixture", "--input-modes", "--no-native-input", "--json"] },
          { name: "disabled-footer-hit-target-refusal-stress", description: "Fail-closed UX receipt contract for disabled footer hit-target refusal and no-submit proof", flags: ["--session", "--surfaces", "--fixtures", "--input-modes", "--no-native-pointer", "--dry-run-only", "--json"] },
          { name: "mini-full-transition-layout-continuity-stress", description: "Fail-closed visual receipt contract for mini/full transition layout continuity", flags: ["--session", "--surfaces", "--transitions", "--fixture", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--local-fixture-only", "--json"] },
          { name: "filter-input-decoration-chip-layout-stress", description: "Fail-closed visual receipt contract for filter input decoration chip layout and clipping", flags: ["--session", "--surfaces", "--queries", "--widths", "--scale-factors", "--fixture", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-config-write", "--local-fixture-only", "--json"] },
          { name: "focus-ring-viewport-integrity-stress", description: "Fail-closed visual receipt contract for focus ring viewport bounds and occlusion", flags: ["--session", "--surfaces", "--fixture", "--input-modes", "--steps", "--no-native-input", "--no-native-pointer", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "warning-banner-action-dismiss-semantics-stress", description: "Fail-closed UX receipt contract for warning banner action/dismiss semantics", flags: ["--session", "--surface", "--fixtures", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-config-write", "--local-fixture-only", "--json"] },
          { name: "select-prompt-multiselect-keyboard-state-stress", description: "Fail-closed UX receipt contract for SelectPrompt keyboard-only multi-selection state parity", flags: ["--session", "--surface", "--fixture", "--choices", "--selection-steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "file-search-preview-sanitization-stress", description: "Fail-closed UX receipt contract for File Search safe preview sanitization", flags: ["--session", "--surface", "--fixture", "--preview-fixtures", "--selection-cycles", "--filter-cycles", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-quick-look", "--no-system-pasteboard", "--local-fixture-only", "--json"] },
          { name: "hotkey-prompt-transient-capture-cancel-stress", description: "Fail-closed UX receipt contract for HotkeyPrompt transient capture/cancel semantics", flags: ["--session", "--surface", "--fixture", "--chords", "--cancel-methods", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-config-write", "--no-global-hotkey-registration", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "process-manager-sort-detail-panel-stability-stress", description: "Fail-closed UX receipt contract for Process Manager sort/header/detail panel stability", flags: ["--session", "--surface", "--fixture", "--sort-keys", "--selection-cycles", "--filter-cycles", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-process-kill", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "env-prompt-redacted-status-error-recovery-stress", description: "Fail-closed UX receipt contract for EnvPrompt redacted status/error recovery", flags: ["--session", "--surface", "--fixture", "--status-fixtures", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-config-write", "--no-secret-write", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "command-palette-breadcrumb-route-stack-stress", description: "Fail-closed UX receipt contract for command palette breadcrumb route-stack restoration", flags: ["--session", "--host", "--fixture", "--drill-path", "--filter", "--back-methods", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "root-source-chip-action-semantics-stress", description: "Fail-closed UX receipt contract for root source-chip actions and status-chip refusal", flags: ["--session", "--queries", "--actions", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-config-write", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "recent-history-dedupe-root-grouping-stress", description: "Fail-closed UX receipt contract for recent/history dedupe and root grouping stability", flags: ["--session", "--fixture", "--sources", "--query", "--cycles", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "inline-attachment-preview-chip-stability-stress", description: "Fail-closed UX receipt contract for inline attachment preview chip stability", flags: ["--session", "--hosts", "--fixture", "--origins", "--chip-actions", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-screen-capture", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "window-title-status-semantics-stress", description: "Fail-closed UX receipt contract for window title and visible status semantics", flags: ["--session", "--surfaces", "--states", "--transitions", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "menu-syntax-capture-validation-chip-stress", description: "Fail-closed UX receipt contract for menu syntax capture validation chips", flags: ["--session", "--fixture", "--cases", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "agent_chat-footer-activity-indicator-stress", description: "Fail-closed UX receipt contract for Agent Chat footer activity indicator transitions", flags: ["--session", "--hosts", "--fixture", "--activity-fixtures", "--input-modes", "--agent-fixture", "--no-native-input", "--no-native-pointer", "--no-security-prompts", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "agent_chat-model-history-popover-visual-state-stress", description: "Fail-closed UX receipt contract for Agent Chat model/history popover visual states", flags: ["--session", "--families", "--fixture", "--states", "--selection-cycles", "--filter-cycles", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "agent_chat-context-insertion-preview-parity-stress", description: "Fail-closed UX receipt contract for Agent Chat context insertion selected-row preview parity", flags: ["--session", "--sources", "--destination", "--fixture", "--selection-cycles", "--filter-cycles", "--insert-modes", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-quick-look", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "agent_chat-composer-token-keyboard-edit-parity-stress", description: "Fail-closed UX receipt contract for Agent Chat composer token keyboard delete/reorder parity", flags: ["--session", "--hosts", "--fixture", "--token-kinds", "--edit-steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-screen-capture", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "agent_chat-transcript-stream-retry-virtualization-stress", description: "Fail-closed UX receipt contract for Agent Chat transcript streaming, virtualization, and retry recovery", flags: ["--session", "--hosts", "--fixture", "--message-count", "--stream-chunks", "--error-fixtures", "--retry-paths", "--scroll-positions", "--input-modes", "--agent-fixture", "--no-native-input", "--no-native-pointer", "--no-security-prompts", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "agent_chat-plugin-skill-entry-thread-affinity-stress", description: "Fail-closed UX receipt contract for Agent Chat plugin skill entry thread affinity", flags: ["--session", "--hosts", "--fixture", "--skill-id", "--entry-paths", "--input-modes", "--agent-fixture", "--no-native-input", "--no-native-pointer", "--no-security-prompts", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "notes-cart-agent_chat-handoff-dedupe-stress", description: "Fail-closed UX receipt contract for Notes cart Agent Chat handoff dedupe", flags: ["--session", "--fixture", "--notes", "--cart-items", "--handoff-paths", "--input-modes", "--agent-fixture", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-screen-capture", "--no-system-pasteboard", "--no-network", "--no-submit", "--sandbox-notes-store", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "root-file-source-filter-pagination-footer-stress", description: "Fail-closed UX receipt contract for root Files source-filter pagination near the footer", flags: ["--session", "--fixture", "--queries", "--page-size", "--provider-delays", "--selection-steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-quick-look", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "file-search-directory-breadcrumb-restoration-stress", description: "Fail-closed UX receipt contract for File Search directory breadcrumb restoration", flags: ["--session", "--fixture", "--start-dir", "--queries", "--navigation-steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-quick-look", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "emoji-picker-skin-tone-category-ux-stress", description: "Fail-closed UX receipt contract for Emoji Picker skin-tone palette and category stability", flags: ["--session", "--fixture", "--categories", "--queries", "--skin-tones", "--steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "root-window-source-filter-activation-refusal-stress", description: "Fail-closed UX receipt contract for root Windows source-filter activation refusal", flags: ["--session", "--fixture", "--queries", "--window-states", "--selection-steps", "--input-modes", "--window-provider", "--no-native-input", "--no-native-pointer", "--no-window-activation", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "notes-markdown-preview-scroll-sync-stress", description: "Fail-closed UX receipt contract for Notes markdown preview scroll and focus sync", flags: ["--session", "--fixture", "--notes", "--markdown-fixtures", "--edit-steps", "--input-modes", "--sandbox-notes-store", "--no-native-input", "--no-native-pointer", "--no-native-picker", "--no-system-pasteboard", "--no-network", "--no-external-services", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "quick-terminal-ansi-scrollback-search-stress", description: "Fail-closed UX receipt contract for Quick Terminal ANSI scrollback search readability", flags: ["--session", "--fixture", "--transcript-fixtures", "--search-queries", "--scroll-positions", "--input-modes", "--terminal-fixture", "--no-shell-command", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-external-services", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "script-output-inspector-folding-recovery-stress", description: "Fail-closed UX receipt contract for script output inspector folding and retry recovery", flags: ["--session", "--fixture", "--output-fixtures", "--view-steps", "--input-modes", "--script-fixture", "--no-handler-spawn", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-external-services", "--no-process-kill", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "app-launcher-icon-grid-keyboard-navigation-stress", description: "Fail-closed UX receipt contract for App Launcher icon-grid keyboard navigation", flags: ["--session", "--fixture", "--apps", "--grid-states", "--navigation-steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-app-launch", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "browser-history-time-grouped-privacy-stress", description: "Fail-closed UX receipt contract for Browser History time grouping and privacy redaction", flags: ["--session", "--fixture", "--browser-fixtures", "--time-buckets", "--queries", "--privacy-modes", "--input-modes", "--browser-provider", "--no-browser-activation", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "settings-preferences-search-reset-preview-stress", description: "Fail-closed UX receipt contract for Settings preferences search and reset preview", flags: ["--session", "--fixture", "--preference-fixtures", "--control-types", "--queries", "--reset-paths", "--input-modes", "--sandbox-config", "--no-config-write", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "settings-preferences-readonly-detail-panel-stress", description: "Fail-closed UX receipt contract for Settings read-only detail panel navigation", flags: ["--session", "--fixture", "--sections", "--queries", "--navigation-steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-external-services", "--no-security-prompts", "--no-system-settings", "--no-tcc-mutation", "--no-config-write", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "design-picker-preview-restore-visual-stress", description: "Fail-closed visual receipt contract for Design Picker preview restore", flags: ["--session", "--fixture", "--catalog-ids", "--preview-steps", "--input-modes", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-external-services", "--no-security-prompts", "--no-system-settings", "--no-tcc-mutation", "--no-config-write", "--no-design-write", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "dictation-history-transcript-preview-redaction-stress", description: "Fail-closed UX receipt contract for Dictation History transcript preview redaction", flags: ["--session", "--fixture", "--transcript-fixtures", "--queries", "--selection-cycles", "--view-steps", "--input-modes", "--dictation-fixture", "--no-microphone", "--no-media-capture", "--no-native-input", "--no-native-pointer", "--no-system-pasteboard", "--no-network", "--no-external-services", "--no-security-prompts", "--no-system-settings", "--no-tcc-mutation", "--no-submit", "--dry-run-only", "--local-fixture-only", "--json"] },
          { name: "agent_chat-setup-recovery", description: "Recovery from Agent Chat setup state", flags: ["--session", "--select-agent", "--json"] },
          { name: "surface-proof", description: "Seconds-first proof for main / attached popup / detached surfaces", flags: ["--session", "--kind", "--index", "--json"] },
          { name: "surface-navigate", description: "Warm-session state-first navigation, safe interaction, and strict screenshot capture for known surfaces", flags: ["--session", "--group", "--case", "--interact", "--capture", "--out-dir", "--manifest", "--fresh-per-case", "--keep-session", "--json"] },
          { name: "scenario", description: "Run a replayable scenario with proof bundle", flags: ["--session", "--scenario", "--index", "--json"] },
          { name: "vision-loop", description: "Materialize visionCrops from receipt", flags: ["--receipt", "--out-dir"] },
          { name: "help", description: "Show help (--json for machine-readable)", flags: ["--json"] },
        ],
        contracts: [
          "surface-proof-contract",
          "detached-proof-contract",
          "no-focus-input-ladder",
          "popup-capture-receipts",
        ],
        receipts: [
          "inputMethod",
          "resolvedWindowId",
          "dispatchPath",
          "resolvedTarget.windowId",
          "proofBundle.usage",
          "proofBundle.capabilities",
          "proofBundle.state",
          "proofBundle.elements",
          "proofBundle.targetIdentity",
          "proofBundle.targetThread",
          "proofBundle.peerWindows",
          "proofBundle.captureTarget",
          "proofBundle.popupCases",
          "proofBundle.origin",
          "proofBundle.portal",
          "proofBundle.permissions",
          "proofBundle.shortcut",
          "proofBundle.templatePrompt",
          "proofBundle.currentAppCommands",
          "proofBundle.actionsCapturedSubject",
          "proofBundle.dropPrompt",
          "proofBundle.pathPrompt",
          "proofBundle.screenshotIdentity",
          "proofBundle.clipboardPortal",
          "proofBundle.browserCache",
          "proofBundle.scrollSelection",
          "proofBundle.permissionAssistant",
          "proofBundle.quickTerminal",
          "proofBundle.mcpContextResource",
          "proofBundle.settingsThemeHotReload",
          "proofBundle.fileSearchDragOut",
          "proofBundle.scriptletBundleExecution",
          "proofBundle.trayMenuMutation",
          "proofBundle.multiWindowRestore",
          "proofBundle.agent_chatDictationDelivery",
          "proofBundle.clipboardShareTrust",
          "proofBundle.clipboardShareReplay",
          "proofBundle.permissionShareCrossPrompt",
          "proofBundle.visibleTextAudit",
          "proofBundle.visibleTextLayoutAudit",
          "proofBundle.layoutMeasurement",
          "proofBundle.layoutMeasurementRegression",
          "proofBundle.visualConsistency",
          "proofBundle.screenshotSemanticsConsistency",
          "proofBundle.modalStackArbitration",
          "proofBundle.crossSurfaceExport",
          "proofBundle.sessionRecovery",
          "proofBundle.menuSyntaxAmbiguity",
          "proofBundle.imeCompositionBoundary",
          "proofBundle.accessibilitySelectedTextFallback",
          "proofBundle.displayMigrationVisualBounds",
          "proofBundle.nativePickerExternalReturnFocus",
          "proofBundle.dragCancelPayloadScope",
          "proofBundle.runtimeAppearanceChurnFocusedInput",
          "proofBundle.powerResumeWindowGeneration",
          "proofBundle.menuTrayNotificationModalInterruption",
          "proofBundle.streamProgressCancelVisualStability",
          "proofBundle.dictationMediaPermissionReadinessChurn",
          "proofBundle.animationFrameCaptureDeterminism",
          "proofBundle.clipboardCopyVisualFeedback",
          "proofBundle.portalCancelReturnStateRestoration",
          "proofBundle.tooltipHoverFocusAffordance",
          "proofBundle.shortcutRecorderCancelLayeringReceipt",
          "proofBundle.inlinePopoverAnchorResizeReceipt",
          "proofBundle.disabledFooterHitTargetRefusalReceipt",
          "proofBundle.miniFullTransitionLayoutContinuityReceipt",
          "proofBundle.filterInputDecorationChipLayoutReceipt",
          "proofBundle.focusRingViewportIntegrityReceipt",
          "proofBundle.warningBannerActionDismissSemanticsReceipt",
          "proofBundle.selectPromptMultiselectKeyboardStateReceipt",
          "proofBundle.fileSearchPreviewSanitizationReceipt",
          "proofBundle.hotkeyPromptTransientCaptureCancelReceipt",
          "proofBundle.processManagerSortDetailPanelStabilityReceipt",
          "proofBundle.envPromptRedactedStatusErrorRecoveryReceipt",
          "proofBundle.commandPaletteBreadcrumbRouteStackReceipt",
          "proofBundle.rootSourceChipActionSemanticsReceipt",
          "proofBundle.recentHistoryDedupeRootGroupingReceipt",
          "proofBundle.inlineAttachmentPreviewChipStabilityReceipt",
          "proofBundle.windowTitleStatusSemanticsReceipt",
          "proofBundle.menuSyntaxCaptureValidationChipReceipt",
          "proofBundle.agent_chatFooterActivityIndicatorReceipt",
          "proofBundle.agent_chatModelHistoryPopoverVisualStateReceipt",
          "proofBundle.agent_chatContextInsertionPreviewParityReceipt",
          "proofBundle.delayedAction",
        ],
        routing: {
          description: "Non-main targets are exact-id threaded. Attached popups prefer batch/simulateGpuiEvent. Detached Agent Chat target-threading stress forces native input with focus enforcement.",
          methods: ["batch", "simulateGpuiEvent", "native", "force-native", "force-batch", "force-gpui"],
          nonMainTargets: ["agentChatDetached", "actionsDialog", "promptPopup"],
        },
      };
      console.log(JSON.stringify(helpJson, null, 2));
      process.exit(0);
    }
    console.log(`Usage: bun scripts/agentic/index.ts <recipe> [--session NAME] [--key enter|tab] [--vision]
  [--target-json '{"type":"kind","kind":"agentChatDetached","index":0}'] [--surface agent_chat]
  [--kind KIND] [--index N] [--select-agent ID] [--scenario NAME]

Recipes:
  preflight              Check prerequisites (session, window, permissions)
  agent_chat-open               Open Agent Chat and verify ready state
  agent_chat-accept             Full Agent Chat picker accept; choose key with --key enter|tab
  agent_chat-enter-accept       Compatibility alias for --key enter
  agent_chat-tab-accept         Compatibility alias for --key tab
  agent_chat-detached-accept    One-command detached Agent Chat proof: resolve → accept → identity check
  agent_chat-detached-target-threading-stress
                         Multi-window detached Agent Chat proof with exact target threading
  notes-agent_chat-delayed-action-origin-stress
                         Fail-closed Notes Agent Chat delayed-action origin/generation stress
  file-portal-origin-roundtrip
                         Fail-closed Agent Chat portal origin/context-part round-trip stress
  permission-privacy-preflight
                         Read-only permission preflight; never opens Settings or mutates TCC
  shortcut-recorder-focus-capture
                         Fail-closed shortcut recorder focus/capture stress
  template-prompt-automation-parity-stress
                         State-first TemplatePrompt state/elements/actions/forceSubmit parity
  current-app-commands-frontmost-stress
                         Fail-closed Do in Current App frontmost/filtering/action snapshot
  actions-captured-subject-frame-stress
                         Fail-closed root actions captured-subject frame stability
  drop-prompt-native-drop-privacy-stress
                         Fail-closed DropPrompt native drop privacy/redaction proof
  path-prompt-filesystem-edge-stress
                         State-first PathPrompt filesystem edge proof
  screenshot-identity-agent_chat-context-stress
                         Fail-closed screenshotIdentity to Agent Chat context proof
  clipboard-history-portal-range-stress
                         Fail-closed Clipboard History portal host/range proof
  browser-tabs-cache-identity-stress
                         Fail-closed browser cache identity/dedupe proof
  scroll-selection-reanchor-stress
                         Fail-closed cross-surface scroll selection proof
  permission-assistant-drag-preflight-stress
                         Fail-closed Permission Assistant passive drag/no-TCC proof
  quick-terminal-pty-apply-back-stress
                         Fail-closed Quick Terminal PTY apply-back lifecycle proof
  mcp-context-resource-attachment-identity-stress
                         Fail-closed MCP context resource identity proof
  settings-theme-hot-reload-stress
                         Fail-closed Settings/theme hot-reload proof
  file-search-drag-out-identity-stress
                         Fail-closed File Search drag-out identity proof
  scriptlet-bundle-execution-matrix-stress
                         Fail-closed scriptlet bundle execution matrix proof
  tray-global-hotkey-menu-mutation-stress
                         Fail-closed tray/global hotkey menu mutation proof
  multi-window-resize-monitor-restoration-stress
                         Fail-closed multi-window resize/monitor restoration proof
  agent_chat-targeted-dictation-delivery-stress
                         Fail-closed Agent Chat-targeted dictation delivery proof
  clipboard-share-trust-install-stress
                         Fail-closed clipboard share trust install proof
  clipboard-share-watcher-stale-replay-stress
                         Fail-closed clipboard watcher stale/replay proof
  permission-share-cross-prompt-focus-stress
                         Fail-closed permission/share cross-prompt focus proof
  visible-text-clipping-overlap-stress
                         Pass-now main-window visible text clipping/overlap visual proof
  layout-measurement-regression-stress
                         Pass-now main-window layout measurement regression proof
  screenshot-semantics-visual-consistency-stress
                         Pass-now screenshot-to-semantics consistency proof
  modal-stack-arbitration-stress
                         Fail-closed stacked modal key arbitration proof
  cross-surface-export-provenance-stress
                         Fail-closed cross-surface export provenance proof
  dev-session-recovery-stale-target-stress
                         Pass-now stale target recovery proof
  menu-syntax-ambiguity-diagnostics-stress
                         Fail-closed menu syntax ambiguity diagnostics proof
  ime-composition-input-boundary-stress
                         Fail-closed IME composition boundary proof
  accessibility-selected-text-fallback-stress
                         Fail-closed selected-text fallback proof
  display-migration-visual-bounds-stress
                         Fail-closed display migration visual bounds proof
  native-picker-external-return-focus-stress
                         Fail-closed native picker/external return focus proof
  drag-cancel-payload-scope-stress
                         Fail-closed drag cancellation payload scope proof
  runtime-appearance-churn-focused-input-stress
                         Fail-closed focused input appearance churn proof
  power-resume-window-generation-stress
                         Fail-closed power resume window generation proof
  menu-tray-notification-modal-interruption-stress
                         Fail-closed menu/tray/notification modal interruption proof
  stream-progress-cancel-visual-stability-stress
                         Fail-closed stream/progress cancellation visual stability proof
  dictation-media-permission-readiness-churn-stress
                         Fail-closed dictation/media permission readiness churn proof
  animation-frame-capture-determinism-stress
                         Fail-closed animation frame capture determinism proof
  accessibility-tree-semantic-parity-stress
                         Fail-closed accessibility tree semantic parity proof
  rtl-bidi-emoji-text-rendering-stress
                         Fail-closed RTL/bidirectional/emoji text rendering proof
  high-volume-virtualized-list-stability-stress
                         Fail-closed high-volume virtualized list stability proof
  input-modality-transition-ownership-stress
                         Fail-closed input-device modality transition ownership proof
  multi-context-attachment-dedupe-provenance-stress
                         Fail-closed multi-context attachment dedupe/provenance proof
  visual-contrast-readable-state-stress
                         Pass-now theme contrast/readable-state proof
  empty-error-retry-state-ux-stress
                         Fail-closed empty/error/retry state UX proof
  form-validation-inline-recovery-stress
                         Fail-closed form validation inline recovery proof
  navigation-back-stack-history-stress
                         Fail-closed navigation/back-stack history proof
  long-text-wrap-resize-surface-stress
                         Pass-now long text wrapping/resizing UX proof
  div-container-scroll-overflow-stress
                         Pass-now DivPrompt tall-container overflow layout proof
  main-menu-dynamic-choice-resize-stress
                         Pass-now dynamic choice-count window resize proof
  notes-window-resize-stress
                         Pass-now sandboxed Notes window grow/shrink resize proof
  notes-delete-confirm-route-proof
                         Pass-now sandboxed Notes delete confirm-popup route proof
  actions-command-discoverability-noop-stress
                         Pass-now action popup row measurement proof
  dense-list-detail-preview-readability-stress
                         Fail-closed dense list/detail preview readability proof
  toast-notification-queue-lifecycle-stress
                         Fail-closed toast/notification queue lifecycle proof
  destructive-confirm-modal-safety-stress
                         Fail-closed destructive confirm dry-run safety proof
  confirm-modal-style-preview-proof
                         Fail-closed: Confirm Modal style-preview scenario owner is missing
  sdk-confirm-runtime-proof
                         Pass-now SDK confirm script route proof
  loading-skeleton-progress-restoration-stress
                         Fail-closed loading skeleton/progress restoration proof
  icon-image-fallback-redaction-stress
                         Fail-closed icon/image fallback redaction proof
  footer-status-persistence-stress
                         Fail-closed footer/status persistence proof
  keyboard-hint-label-parity-stress
                         Fail-closed keyboard hint label parity proof
  row-state-parity-without-pointer-stress
                         Fail-closed row selected/focused/hover state parity proof
  quiet-chrome-card-nesting-stress
                         Fail-closed quiet chrome/card nesting proof
  scroll-shadow-sticky-header-density-stress
                         Fail-closed scroll shadow/sticky header/density proof
  popup-focus-keycap-visual-semantics-stress
                         Fail-closed popup focus/keycap semantics proof
  reduced-motion-animation-disable-stress
                         Fail-closed reduced-motion animation disable proof
  command-search-highlighting-accessory-badges-stress
                         Fail-closed command search highlight/badge proof
  agent_chat-setup-recovery     Recovery from Agent Chat setup; select agent with --select-agent ID
  surface-proof          Seconds-first proof for main / attached popup / detached surfaces
  surface-navigate       Warm-session navigation, safe interaction, and strict screenshots for known surfaces
  scenario               Run a replayable scenario with proof bundle output
  vision-loop            Materialize visionCrops from receipt (pass --receipt, --out-dir)
  help                   Show this help (--json for machine-readable output)

Target threading:
  --target-json JSON   Agent Chat window target for all RPCs (reused across all steps)
  --surface SURFACE    Automation surface for native input focus (main, agent_chat, etc.)
  --kind KIND          Target kind for agent_chat-detached-accept (default: agentChatDetached)
  --index N            Target kind index for agent_chat-detached-accept (default: 0)
  --scenario NAME      Scenario name for the scenario recipe

Input routing (non-main targets):
  attached popups → batch/simulateGpuiEvent first, no OS focus unless required
  detached Agent Chat target-threading stress → force-native with OS focus enforcement
  main, focused, unspecified → native (macos-input.ts with OS focus enforcement)

Available scenarios:
  main-window-exact-id    Resolve exact main target, inspect, getElements
  actions-dialog-exact-id Resolve exact attached ActionsDialog target, inspect, waitFor
  prompt-popup-exact-id   Resolve exact attached PromptPopup target, inspect, waitFor
  detached-agent_chat-exact-id  Resolve exact detached Agent Chat target, inspect, GPUI event, inspect again
  file-portal-origin-roundtrip
                         Emit fail-closed portal origin/context receipt requirements
  permission-privacy-preflight
                         Run read-only permission prerequisite receipts
  shortcut-recorder-focus-capture
                         Emit fail-closed shortcut recorder receipt requirements
  template-prompt-automation-parity-stress
                         Run TemplatePrompt automation parity proof
  current-app-commands-frontmost-stress
                         Emit fail-closed Current App Commands frontmost requirements
  actions-captured-subject-frame-stress
                         Emit fail-closed captured-subject frame requirements
  drop-prompt-native-drop-privacy-stress
                         Emit fail-closed DropPrompt native drop privacy requirements
  path-prompt-filesystem-edge-stress
                         Run PathPrompt filesystem edge helper
  screenshot-identity-agent_chat-context-stress
                         Emit fail-closed screenshot identity context requirements
  clipboard-history-portal-range-stress
                         Emit fail-closed Clipboard History portal range requirements
  browser-tabs-cache-identity-stress
                         Emit fail-closed browser cache identity requirements
  scroll-selection-reanchor-stress
                         Emit fail-closed scroll selection reanchor requirements
  permission-assistant-drag-preflight-stress
                         Emit fail-closed Permission Assistant drag/preflight requirements
  quick-terminal-pty-apply-back-stress
                         Emit fail-closed Quick Terminal apply-back requirements
  mcp-context-resource-attachment-identity-stress
                         Emit fail-closed MCP context resource identity requirements
  settings-theme-hot-reload-stress
                         Emit fail-closed Settings/theme hot-reload requirements
  file-search-drag-out-identity-stress
                         Emit fail-closed File Search drag-out identity requirements
  scriptlet-bundle-execution-matrix-stress
                         Emit fail-closed scriptlet bundle execution requirements
  tray-global-hotkey-menu-mutation-stress
                         Emit fail-closed tray/global hotkey mutation requirements
  multi-window-resize-monitor-restoration-stress
                         Emit fail-closed multi-window resize/monitor requirements
  agent_chat-targeted-dictation-delivery-stress
                         Emit fail-closed Agent Chat dictation targeting requirements
  clipboard-share-trust-install-stress
                         Emit fail-closed clipboard share trust install requirements
  clipboard-share-watcher-stale-replay-stress
                         Emit fail-closed clipboard watcher stale/replay requirements
  permission-share-cross-prompt-focus-stress
                         Emit fail-closed permission/share cross-prompt focus requirements
  visible-text-clipping-overlap-stress
                         Measure main-window visible text clipping/overlap
  layout-measurement-regression-stress
                         Measure main-window layout geometry
  screenshot-semantics-visual-consistency-stress
                         Run strict screenshot/semantics visual consistency proof
  modal-stack-arbitration-stress
                         Emit fail-closed stacked modal arbitration requirements
  cross-surface-export-provenance-stress
                         Emit fail-closed cross-surface export provenance requirements
  dev-session-recovery-stale-target-stress
                         Run pass-now stale target session recovery proof
  menu-syntax-ambiguity-diagnostics-stress
                         Emit fail-closed menu syntax ambiguity requirements
  ime-composition-input-boundary-stress
                         Emit fail-closed IME composition requirements
  accessibility-selected-text-fallback-stress
                         Emit fail-closed selected-text fallback requirements
  display-migration-visual-bounds-stress
                         Emit fail-closed display migration visual bounds requirements
  native-picker-external-return-focus-stress
                         Emit fail-closed native picker/external return focus requirements
  drag-cancel-payload-scope-stress
                         Emit fail-closed drag cancellation payload scope requirements
  runtime-appearance-churn-focused-input-stress
                         Emit fail-closed focused input appearance churn requirements
  power-resume-window-generation-stress
                         Emit fail-closed power resume window generation requirements
  menu-tray-notification-modal-interruption-stress
                         Emit fail-closed menu/tray/notification interruption requirements
  stream-progress-cancel-visual-stability-stress
                         Emit fail-closed stream/progress cancellation visual stability requirements
  dictation-media-permission-readiness-churn-stress
                         Emit fail-closed dictation/media readiness churn requirements
  animation-frame-capture-determinism-stress
                         Emit fail-closed animation frame capture determinism requirements
  accessibility-tree-semantic-parity-stress
                         Emit fail-closed accessibility tree semantic parity requirements
  rtl-bidi-emoji-text-rendering-stress
                         Emit fail-closed RTL/bidirectional/emoji text rendering requirements
  high-volume-virtualized-list-stability-stress
                         Emit fail-closed high-volume virtualized list stability requirements
  input-modality-transition-ownership-stress
                         Emit fail-closed input-device modality transition requirements
  multi-context-attachment-dedupe-provenance-stress
                         Emit fail-closed multi-context attachment dedupe/provenance requirements
  visual-contrast-readable-state-stress
                         Collect theme contrast/readable-state receipts
  empty-error-retry-state-ux-stress
                         Emit fail-closed empty/error/retry state UX requirements
  form-validation-inline-recovery-stress
                         Emit fail-closed form validation inline recovery requirements
  navigation-back-stack-history-stress
                         Emit fail-closed navigation/back-stack history requirements
  long-text-wrap-resize-surface-stress
                         Emit fail-closed long text wrapping/resizing UX requirements
  actions-command-discoverability-noop-stress
                         Measure real Cmd-K action picker rows
  dense-list-detail-preview-readability-stress
                         Emit fail-closed dense list/detail preview readability requirements
  toast-notification-queue-lifecycle-stress
                         Emit fail-closed toast/notification queue lifecycle requirements
  destructive-confirm-modal-safety-stress
                         Emit fail-closed destructive confirm dry-run safety requirements
  confirm-modal-style-preview-proof
                         Fail closed because the Confirm Modal style-preview scenario owner is missing
  sdk-confirm-runtime-proof
                         Prove SDK global confirm() reaches shared ConfirmPrompt
  loading-skeleton-progress-restoration-stress
                         Emit fail-closed loading skeleton/progress restoration requirements
  icon-image-fallback-redaction-stress
                         Emit fail-closed icon/image fallback redaction requirements
  footer-status-persistence-stress
                         Emit fail-closed footer/status persistence requirements
  keyboard-hint-label-parity-stress
                         Emit fail-closed keyboard hint label parity requirements
  row-state-parity-without-pointer-stress
                         Emit fail-closed row state parity requirements
  quiet-chrome-card-nesting-stress
                         Emit fail-closed quiet chrome/card nesting requirements
  scroll-shadow-sticky-header-density-stress
                         Emit fail-closed scroll shadow/sticky header/density requirements
  popup-focus-keycap-visual-semantics-stress
                         Emit fail-closed popup focus/keycap visual semantics requirements
  reduced-motion-animation-disable-stress
                         Emit fail-closed reduced-motion animation disable requirements
  command-search-highlighting-accessory-badges-stress
                         Emit fail-closed command search highlighting/accessory badge requirements
  clipboard-copy-visual-feedback-stress
                         Emit fail-closed clipboard copy visual feedback requirements
  portal-cancel-return-state-restoration-stress
                         Emit fail-closed portal cancel/back return restoration requirements
  tooltip-hover-focus-affordance-stress
                         Emit fail-closed tooltip hover/focus affordance requirements
  shortcut-recorder-cancel-layering-stress
                         Emit fail-closed shortcut recorder cancel/layering requirements
  inline-popover-anchor-resize-stress
                         Emit fail-closed inline popover anchor/resize requirements
  disabled-footer-hit-target-refusal-stress
                         Emit fail-closed disabled footer hit-target refusal requirements
  mini-full-transition-layout-continuity-stress
                         Emit fail-closed mini/full transition layout continuity requirements
  filter-input-decoration-chip-layout-stress
                         Emit fail-closed filter input decoration chip layout requirements
  focus-ring-viewport-integrity-stress
                         Emit fail-closed focus ring viewport integrity requirements
  warning-banner-action-dismiss-semantics-stress
                         Emit fail-closed warning banner action/dismiss semantics requirements
  select-prompt-multiselect-keyboard-state-stress
                         Emit fail-closed SelectPrompt keyboard multi-selection state requirements
  file-search-preview-sanitization-stress
                         Emit fail-closed File Search safe preview sanitization requirements
  hotkey-prompt-transient-capture-cancel-stress
                         Emit fail-closed HotkeyPrompt transient capture/cancel requirements
  process-manager-sort-detail-panel-stability-stress
                         Emit fail-closed Process Manager sort/header/detail panel requirements
  env-prompt-redacted-status-error-recovery-stress
                         Emit fail-closed EnvPrompt redacted status/error recovery requirements
  command-palette-breadcrumb-route-stack-stress
                         Emit fail-closed command palette breadcrumb route-stack requirements
  root-source-chip-action-semantics-stress
                         Emit fail-closed root source-chip action semantics requirements
  recent-history-dedupe-root-grouping-stress
                         Emit fail-closed recent/history dedupe root grouping requirements
  inline-attachment-preview-chip-stability-stress
                         Emit fail-closed inline attachment preview chip requirements
  window-title-status-semantics-stress
                         Emit fail-closed window title/status semantics requirements
  menu-syntax-capture-validation-chip-stress
                         Emit fail-closed menu syntax capture validation chip requirements
  agent_chat-footer-activity-indicator-stress
                         Emit fail-closed Agent Chat footer activity indicator requirements
  agent_chat-model-history-popover-visual-state-stress
                         Emit fail-closed Agent Chat model/history popover visual state requirements
  agent_chat-context-insertion-preview-parity-stress
                         Emit fail-closed Agent Chat context insertion preview parity requirements
  agent_chat-composer-token-keyboard-edit-parity-stress
                         Emit fail-closed Agent Chat composer token keyboard edit parity requirements
  agent_chat-transcript-stream-retry-virtualization-stress
                         Emit fail-closed Agent Chat transcript stream retry virtualization requirements
  agent_chat-plugin-skill-entry-thread-affinity-stress
                         Emit fail-closed Agent Chat plugin skill entry thread affinity requirements
  notes-cart-agent_chat-handoff-dedupe-stress
                         Emit fail-closed Notes cart Agent Chat handoff dedupe requirements
  root-file-source-filter-pagination-footer-stress
                         Emit fail-closed root Files source-filter pagination footer requirements
  file-search-directory-breadcrumb-restoration-stress
                         Emit fail-closed File Search breadcrumb restoration requirements
  emoji-picker-skin-tone-category-ux-stress
                         Emit fail-closed Emoji Picker skin-tone/category UX requirements
  root-window-source-filter-activation-refusal-stress
                         Emit fail-closed root Windows source-filter activation refusal requirements
  notes-markdown-preview-scroll-sync-stress
                         Emit fail-closed Notes markdown preview scroll sync requirements
  quick-terminal-ansi-scrollback-search-stress
                         Emit fail-closed Quick Terminal ANSI scrollback search requirements
  script-output-inspector-folding-recovery-stress
                         Emit fail-closed script output inspector folding recovery requirements
  app-launcher-icon-grid-keyboard-navigation-stress
                         Emit fail-closed App Launcher icon-grid keyboard navigation requirements
  browser-history-time-grouped-privacy-stress
                         Emit fail-closed Browser History time grouping and privacy requirements
  settings-preferences-search-reset-preview-stress
                         Emit fail-closed Settings preferences search/reset preview requirements
  settings-preferences-readonly-detail-panel-stress
                         Emit fail-closed Settings read-only detail panel requirements
  design-picker-preview-restore-visual-stress
                         Emit fail-closed Design Picker preview restore requirements
  dictation-history-transcript-preview-redaction-stress
                         Emit fail-closed Dictation History transcript preview redaction requirements

Examples:
  bun scripts/agentic/index.ts surface-proof --session default --kind main
  bun scripts/agentic/index.ts surface-navigate --session default --group filterable-main --case all --interact safe --capture --fresh-per-case --out-dir .notes/image-library --manifest .notes/image-library/manifest.json --json
  bun scripts/agentic/index.ts surface-navigate --session popup --group attached-popup --case actions-dialog-attached-popup --capture --fresh-per-case --json
  bun scripts/agentic/index.ts surface-proof --session default --kind promptPopup --index 0
  bun scripts/agentic/index.ts surface-proof --session default --kind agentChatDetached --index 0
  bun scripts/agentic/index.ts agent_chat-accept --session default --key enter
  bun scripts/agentic/index.ts agent_chat-accept --session default --key tab --vision
  bun scripts/agentic/index.ts agent_chat-accept --session default --key enter \\
    --target-json '{"type":"kind","kind":"agentChatDetached","index":0}' --surface agent_chat --vision
  bun scripts/agentic/index.ts agent_chat-detached-accept --session default --kind agentChatDetached --index 0 --key enter --vision
  bun scripts/agentic/index.ts agent_chat-detached-target-threading-stress --session default --kind agentChatDetached --index 0 --min-targets 2 --key enter --vision --json
  bun scripts/agentic/index.ts notes-agent_chat-delayed-action-origin-stress --session default --drift generation --json
  bun scripts/agentic/index.ts file-portal-origin-roundtrip --session default --host agent_chat --portal file-search --json
  bun scripts/agentic/index.ts permission-privacy-preflight --session default --json
  bun scripts/agentic/index.ts shortcut-recorder-focus-capture --session default --chord cmd+shift+7 --json
  bun scripts/agentic/index.ts template-prompt-automation-parity-stress --session default --template 'Hello {{name}}' --field name --value Ada --forced-value forced-template-result --json
  bun scripts/agentic/index.ts current-app-commands-frontmost-stress --session default --alias 'Do in Current Command' --query 'close tab' --json
  bun scripts/agentic/index.ts actions-captured-subject-frame-stress --session default --source root-file --action quick-look --mutation filter-selection-cache-frame --json
  bun scripts/agentic/index.ts drop-prompt-native-drop-privacy-stress --session default --file-name agentic-drop.txt --size 12 --json
  bun scripts/agentic/index.ts path-prompt-filesystem-edge-stress --session default --json
  bun scripts/agentic/index.ts screenshot-identity-agent_chat-context-stress --session default --source tab-ai-screenshot --json
  bun scripts/agentic/index.ts clipboard-history-portal-range-stress --session default --portal-id 'kit://clipboard-history?id=agentic' --range composer:0..0 --json
  bun scripts/agentic/index.ts browser-tabs-cache-identity-stress --session default --source browser-tabs --json
  bun scripts/agentic/index.ts scroll-selection-reanchor-stress --session default --kinds clipboard,browser-history,current-app-commands,file-search --json
  bun scripts/agentic/index.ts permission-assistant-drag-preflight-stress --session default --pane Accessibility --bundle-id com.scriptkit.app --json
  bun scripts/agentic/index.ts quick-terminal-pty-apply-back-stress --session default --command 'printf agentic-pty-apply-back' --json
  bun scripts/agentic/index.ts mcp-context-resource-attachment-identity-stress --session default --resource-uri kit://context/agentic-loop-six --profile agentic-test --source mcp-resource --json
  bun scripts/agentic/index.ts settings-theme-hot-reload-stress --session default --theme-before script-kit-dark --theme-after script-kit-light --config-key theme --sandbox-config --json
  bun scripts/agentic/index.ts file-search-drag-out-identity-stress --session default --query AGENTS.md --file-name AGENTS.md --drop-target host-refusal-fixture --json
  bun scripts/agentic/index.ts scriptlet-bundle-execution-matrix-stress --session default --scriptlet-id alpha --bundle-id agentic-loop-seven-bundle --cancel-after-ms 50 --json
  bun scripts/agentic/index.ts tray-global-hotkey-menu-mutation-stress --session default --loops 5 --json
  bun scripts/agentic/index.ts multi-window-resize-monitor-restoration-stress --session default --surfaces main,actionsDialog,agentChatDetached,notes --monitor-profile scale-bounds-drift --json
  bun scripts/agentic/index.ts agent_chat-targeted-dictation-delivery-stress --session default --kind agentChatDetached --index 0 --transcript 'agentic loop eight dictation' --json
  bun scripts/agentic/index.ts clipboard-share-trust-install-stress --session default --fixture-id agentic-loop-nine --share-kind script --accept-mode both --json
  bun scripts/agentic/index.ts clipboard-share-watcher-stale-replay-stress --session default --fixture-id agentic-loop-nine --share-kind script --count 3 --burst-ms 25 --json
  bun scripts/agentic/index.ts permission-share-cross-prompt-focus-stress --session default --fixture-id agentic-loop-nine --share-kind script --pane Accessibility --bundle-id com.scriptkit.app --json
  bun scripts/agentic/index.ts visible-text-clipping-overlap-stress --session default --surfaces main,actionsDialog,agentChatDetached --json
  bun scripts/agentic/index.ts layout-measurement-regression-stress --session default --surfaces main,actionsDialog,agentChatDetached --json
  bun scripts/agentic/index.ts screenshot-semantics-visual-consistency-stress --session default --group filterable-main --case clipboard-history-visible-rows --json
  bun scripts/agentic/index.ts modal-stack-arbitration-stress --session default --json
  bun scripts/agentic/index.ts cross-surface-export-provenance-stress --session default --source file-search --destination agent_chat-composer --export-mode copy --query AGENTS.md --json
  bun scripts/agentic/index.ts dev-session-recovery-stale-target-stress --session default --entry clipboard-history-actions --kind actionsDialog --restart-mode stop-start --json
  bun scripts/agentic/index.ts menu-syntax-ambiguity-diagnostics-stress --session default --query '>open @file !bad ~AGENTS.md' --json
  bun scripts/agentic/index.ts ime-composition-input-boundary-stress --session default --json
  bun scripts/agentic/index.ts accessibility-selected-text-fallback-stress --session default --json
  bun scripts/agentic/index.ts display-migration-visual-bounds-stress --session default --surfaces main,actionsDialog,promptPopup,agentChatDetached,notes --from-display primary --to-display external --json
  bun scripts/agentic/index.ts native-picker-external-return-focus-stress --session default --origin agent_chat --handoff file-picker --foreign-app Finder --json
  bun scripts/agentic/index.ts drag-cancel-payload-scope-stress --session default --source file-search --hover-target drop-prompt --cancel escape --json
  bun scripts/agentic/index.ts runtime-appearance-churn-focused-input-stress --session default --surface agent_chat-composer --churn scale,font,theme --cycles 6 --json
  bun scripts/agentic/index.ts power-resume-window-generation-stress --session default --surface main --event sleep-wake --json
  bun scripts/agentic/index.ts menu-tray-notification-modal-interruption-stress --session default --host agentChatChat --active-surface actionsDialog --interruptions tray-menu,app-menu,notification --json
  bun scripts/agentic/index.ts stream-progress-cancel-visual-stability-stress --session default --surface agent_chat-composer --updates 40 --cancel-at 25 --json
  bun scripts/agentic/index.ts dictation-media-permission-readiness-churn-stress --session default --target agent_chat-composer --churn microphone-permission,model-readiness --json
  bun scripts/agentic/index.ts animation-frame-capture-determinism-stress --session default --surfaces main,actionsDialog,promptPopup --frames 6 --interval-ms 80 --json
  bun scripts/agentic/index.ts accessibility-tree-semantic-parity-stress --session default --surfaces main,actionsDialog,promptPopup --json
  bun scripts/agentic/index.ts rtl-bidi-emoji-text-rendering-stress --session default --surface agent_chat-composer --text 'abc שלום 👩🏽‍💻 é مرحبا 123' --json
  bun scripts/agentic/index.ts high-volume-virtualized-list-stability-stress --session default --surface clipboard-history --fixture-count 5000 --filter-cycles 8 --scroll-cycles 12 --json
  bun scripts/agentic/index.ts input-modality-transition-ownership-stress --session default --surface main --interleave pointer-hover,keyboard-nav,trackpad-scroll,wheel-scroll,shortcut --cycles 8 --json
  bun scripts/agentic/index.ts multi-context-attachment-dedupe-provenance-stress --session default --origins file,screenshot,selected-text,mcp-resource,clipboard-snippet --destinations agent_chat-composer,notes --reorder-cycles 3 --json
  bun scripts/agentic/index.ts visual-contrast-readable-state-stress --session default --surfaces main,actionsDialog,promptPopup,agent_chat-composer,notes --themes light,dark --scale-factors 1,1.25,1.5 --states active,inactive,disabled,focused,error,loading --json
  bun scripts/agentic/index.ts empty-error-retry-state-ux-stress --session default --surfaces main,clipboard-history,emoji-picker,file-search --query 'agentic-loop-eighteen-no-results-zzzz' --json
  bun scripts/agentic/index.ts form-validation-inline-recovery-stress --session default --surface fields-prompt --fields email,required-text,number --invalid email:not-an-email,required-text:,number:not-a-number --valid email:ada@example.com,required-text:Ada,number:42 --json
  bun scripts/agentic/index.ts navigation-back-stack-history-stress --session default --origin main --surfaces clipboard-history,emoji-picker,file-search,actionsDialog --transitions triggerBuiltin,cmd-k,escape,back --json
  bun scripts/agentic/index.ts long-text-wrap-resize-surface-stress --session default --surfaces main,clipboard-history,emoji-picker,file-search,actionsDialog --widths mini,narrow,full --fixtures long-name,long-path,long-description,multiline-snippet --json
  bun scripts/agentic/index.ts div-container-scroll-overflow-stress --session default --item-count 80 --json
  bun scripts/agentic/index.ts main-menu-dynamic-choice-resize-stress --session default --small-count 3 --large-count 15 --json
  bun scripts/agentic/index.ts notes-window-resize-stress --session default --short-line-count 2 --tall-line-count 80 --json
  bun scripts/agentic/index.ts notes-delete-confirm-route-proof --session default --json
  bun scripts/agentic/index.ts actions-command-discoverability-noop-stress --session default --hosts main,clipboard-history,emoji-picker,file-search,app-launcher --states actionable,disabled,no-op --json
  bun scripts/agentic/index.ts dense-list-detail-preview-readability-stress --session default --surfaces file-search,sdk-reference,script-template-catalog --query agentic-loop-nineteen-preview --filter-cycles 4 --selection-cycles 8 --resize-cycles 3 --json
  bun scripts/agentic/index.ts toast-notification-queue-lifecycle-stress --session default --surface main --fixtures success,duplicate,persistent,dismiss,autohide --cycles 3 --json
  bun scripts/agentic/index.ts destructive-confirm-modal-safety-stress --session default --host main --fixture agentic-destructive-dry-run --paths cancel,confirm,stale-confirm --dry-run-only --json
  bun scripts/agentic/index.ts confirm-modal-style-preview-proof --session default --json
  bun scripts/agentic/index.ts sdk-confirm-runtime-proof --session default --json
  bun scripts/agentic/index.ts loading-skeleton-progress-restoration-stress --session default --surfaces sdk-reference,script-template-catalog --fixture delayed-local --cycles 4 --json
  bun scripts/agentic/index.ts icon-image-fallback-redaction-stress --session default --surfaces app-launcher,file-search,clipboard-history --fixtures missing-file,corrupt-png,private-local-path,data-uri-redacted --json
  bun scripts/agentic/index.ts footer-status-persistence-stress --session default --surfaces main,clipboard-history,emoji-picker,file-search,actionsDialog --transitions filter,selection,cmd-k,escape,clear-filter --json
  bun scripts/agentic/index.ts keyboard-hint-label-parity-stress --session default --surfaces main,clipboard-history,emoji-picker,file-search,actionsDialog,menuSyntaxTriggerPicker --families footer,row-accessory,tooltip,action-catalog --json
  bun scripts/agentic/index.ts row-state-parity-without-pointer-stress --session default --surfaces main,clipboard-history,emoji-picker,file-search,actionsDialog --states selected,focused,hovered,selected-hovered --json
  bun scripts/agentic/index.ts quiet-chrome-card-nesting-stress --session default --surfaces main,clipboard-history,emoji-picker,file-search,actionsDialog,promptPopup --chrome quiet --json
  bun scripts/agentic/index.ts scroll-shadow-sticky-header-density-stress --session default --surfaces clipboard-history,emoji-picker,file-search,app-launcher,actionsDialog --scroll-positions top,middle,bottom --density compact,default --json
  bun scripts/agentic/index.ts popup-focus-keycap-visual-semantics-stress --session default --surfaces actionsDialog,confirmPrompt --json
  bun scripts/agentic/index.ts reduced-motion-animation-disable-stress --session default --surfaces main,actionsDialog,menuSyntaxTriggerPicker --fixture reduced-motion --json
  bun scripts/agentic/index.ts command-search-highlighting-accessory-badges-stress --session default --hosts main,actionsDialog,app-launcher,menuSyntaxTriggerPicker --query agentic-loop-twenty-three --json
  bun scripts/agentic/index.ts clipboard-copy-visual-feedback-stress --session default --hosts file-search,actionsDialog,app-launcher --fixture agentic-copy-preview --pasteboard-scope fixture --no-system-pasteboard --json
  bun scripts/agentic/index.ts portal-cancel-return-state-restoration-stress --session default --origins agent_chat-composer,notes --portal file-search --query AGENTS.md --cancel-methods escape,back --fixture repo-file --no-native-picker --json
  bun scripts/agentic/index.ts tooltip-hover-focus-affordance-stress --session default --surfaces main,actionsDialog,app-launcher --targets truncated-row,disabled-action,footer-button --fixture agentic-tooltips --input-modes protocol-hover,keyboard-focus --no-native-pointer --json
  bun scripts/agentic/index.ts shortcut-recorder-cancel-layering-stress --session default --surface shortcuts --action test-agentic-shortcut --cancel-methods escape,cmd-w,backdrop,parent-click --input-modes protocol-key,protocol-click --sandbox-config --no-config-write --json
  bun scripts/agentic/index.ts inline-popover-anchor-resize-stress --session default --families agent_chat-slash,menu-syntax-colon --widths mini,narrow,full --fixture agentic-inline-popover --input-modes protocol-key,protocol-resize --no-native-input --json
  bun scripts/agentic/index.ts disabled-footer-hit-target-refusal-stress --session default --surfaces drop-prompt,fields-prompt,path-prompt --fixtures empty-drop,invalid-fields,missing-path --input-modes enter,footer-shortcut,protocol-footer-click --no-native-pointer --dry-run-only --json
  bun scripts/agentic/index.ts mini-full-transition-layout-continuity-stress --session default --surfaces main,mini-prompt,fields-prompt,actionsDialog --transitions mini-to-full,full-to-mini,hide-show,return-to-origin --fixture agentic-mini-full-layout --input-modes protocol-key,protocol-resize --no-native-input --no-native-pointer --no-system-pasteboard --local-fixture-only --json
  bun scripts/agentic/index.ts filter-input-decoration-chip-layout-stress --session default --surfaces main --queries 'f: AGENTS.md,c: agentic,~/script,:actions,;note,!command,literal\\:chip' --widths mini,narrow,full --scale-factors 1,1.25,1.5 --fixture agentic-filter-input-decorations --input-modes protocol-set-filter,protocol-resize --no-native-input --no-native-pointer --no-system-pasteboard --no-config-write --local-fixture-only --json
  bun scripts/agentic/index.ts focus-ring-viewport-integrity-stress --session default --surfaces main,actionsDialog,fields-prompt,path-prompt --fixture agentic-focus-rings --input-modes protocol-key,simulate-gpui-event --steps tab,shift-tab,up,down,escape --no-native-input --no-native-pointer --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts warning-banner-action-dismiss-semantics-stress --session default --surface main --fixtures warning,actionable,dismissible,error --input-modes protocol-hover,protocol-click,protocol-key --no-native-input --no-native-pointer --no-system-pasteboard --no-config-write --local-fixture-only --json
  bun scripts/agentic/index.ts select-prompt-multiselect-keyboard-state-stress --session default --surface select-prompt --fixture agentic-multiselect --choices 24 --selection-steps space,cmd-a,filter-preserve,clear-filter,range-toggle,escape-restore --input-modes protocol-key,batch --no-native-input --no-native-pointer --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts file-search-preview-sanitization-stress --session default --surface file-search --fixture agentic-safe-preview --preview-fixtures text,binary,large-text,missing-file,private-path,unsupported-kind --selection-cycles 8 --filter-cycles 4 --input-modes protocol-set-filter,protocol-key,batch --no-native-input --no-native-pointer --no-native-picker --no-quick-look --no-system-pasteboard --local-fixture-only --json
  bun scripts/agentic/index.ts hotkey-prompt-transient-capture-cancel-stress --session default --surface hotkey-prompt --fixture agentic-transient-hotkey --chords cmd+shift+7,ctrl+space --cancel-methods escape,cmd-w --input-modes protocol-key,simulate-key --no-native-input --no-native-pointer --no-config-write --no-global-hotkey-registration --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts process-manager-sort-detail-panel-stability-stress --session default --surface process-manager --fixture agentic-process-table --sort-keys name,cpu,memory,pid --selection-cycles 8 --filter-cycles 4 --input-modes protocol-click,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-process-kill --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts env-prompt-redacted-status-error-recovery-stress --session default --surface env-prompt --fixture agentic-env-status --status-fixtures missing-secret,parse-error,masked-existing,valid-edit --input-modes protocol-set-input,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-config-write --no-secret-write --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts command-palette-breadcrumb-route-stack-stress --session default --host main --fixture agentic-actions-breadcrumbs --drill-path parent-action,child-action --filter 'switch' --back-methods escape,breadcrumb-click --input-modes protocol-key,protocol-click,batch --no-native-input --no-native-pointer --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts root-source-chip-action-semantics-stress --session default --queries 'f: AGENTS.md,c: agentic,n: welcome,-c: noise' --actions remove-chip,clear-all,toggle-exclude,open-chip-actions --input-modes protocol-click,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-config-write --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts recent-history-dedupe-root-grouping-stress --session default --fixture agentic-root-recents --sources files,notes,clipboard,dictation,agent_chat-history --query agentic-loop-29-dupe --cycles 6 --input-modes protocol-set-filter,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-network --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts inline-attachment-preview-chip-stability-stress --session default --hosts agent_chat-composer,notes --fixture agentic-inline-attachments --origins local-file,fixture-image,fixture-text,script-resource --chip-actions focus,preview,remove,reorder,overflow --input-modes protocol-set-input,protocol-click,batch --no-native-input --no-native-pointer --no-native-picker --no-screen-capture --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts window-title-status-semantics-stress --session default --surfaces main,agent_chat-composer,actionsDialog,promptPopup,notes --states idle,busy,error,dirty,ready --transitions triggerBuiltin,cmd-k,escape,hide-show --input-modes protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts menu-syntax-capture-validation-chip-stress --session default --fixture agentic-capture-validation --cases missing-body-date,missing-date,ready,malformed-url,unresolved-date,dynamic-schema --input-modes protocol-set-filter,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts agent_chat-footer-activity-indicator-stress --session default --hosts agent_chat-composer,notes --fixture agentic-agent_chat-footer-activity --activity-fixtures context-capture,tool-call,plan-update,permission-wait,cancelled,idle-recovered --input-modes protocol-state,batch --agent-fixture scripted-local --no-native-input --no-native-pointer --no-security-prompts --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts agent_chat-model-history-popover-visual-state-stress --session default --families model-selector,local-history --fixture agentic-agent_chat-popover-visual-state --states idle,filtered,empty,loading,current-selection,error-recovered --selection-cycles 8 --filter-cycles 4 --input-modes protocol-set-input,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts agent_chat-context-insertion-preview-parity-stress --session default --sources file-search,browser-history,dictation-history,notes --destination agent_chat-composer --fixture agentic-context-preview-parity --selection-cycles 6 --filter-cycles 4 --insert-modes protocol-accept,batch --input-modes protocol-set-filter,protocol-key,batch --no-native-input --no-native-pointer --no-native-picker --no-quick-look --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts agent_chat-composer-token-keyboard-edit-parity-stress --session default --hosts agent_chat-composer,notes --fixture agentic-agent_chat-composer-tokens --token-kinds mention,slash,pasted-text,pasted-image,skill-file --edit-steps backspace-delete,delete-forward,range-remove,move-token-left,move-token-right,cursor-around-token --input-modes protocol-key,batch --no-native-input --no-native-pointer --no-native-picker --no-screen-capture --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts agent_chat-transcript-stream-retry-virtualization-stress --session default --hosts agent_chat-composer,notes --fixture agentic-agent_chat-transcript-stream --message-count 160 --stream-chunks 48 --error-fixtures tool-error,agent-error,model-timeout,cancelled --retry-paths retry-same-draft,retry-edited-draft,retry-after-scroll --scroll-positions top,middle,bottom,near-active --input-modes protocol-state,protocol-key,batch --agent-fixture scripted-local --no-native-input --no-native-pointer --no-security-prompts --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts agent_chat-plugin-skill-entry-thread-affinity-stress --session loop33 --hosts embedded,detached --fixture agentic-plugin-skill-entry --skill-id new-script --entry-paths main-menu,source-filter,cmd-enter --input-modes protocol-set-filter,batch --agent-fixture scripted-local --no-native-input --no-native-pointer --no-security-prompts --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts notes-cart-agent_chat-handoff-dedupe-stress --session loop33 --fixture agentic-notes-cart --notes note-a,note-b --cart-items duplicate-link,local-snippet,repo-file,unchecked-task --handoff-paths open-agent_chat,switch-note,cancel,consume --input-modes protocol-click,protocol-key,batch --agent-fixture scripted-local --no-native-input --no-native-pointer --no-native-picker --no-screen-capture --no-system-pasteboard --no-network --no-submit --sandbox-notes-store --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts root-file-source-filter-pagination-footer-stress --session loop33 --fixture agentic-root-file-pages --queries 'f: ,f:s,files: AGENTS' --page-size 12 --provider-delays 0,150,450 --selection-steps near-bottom,next-page,filter-tighten,clear-filter --input-modes protocol-set-filter,protocol-key,batch --no-native-input --no-native-pointer --no-native-picker --no-quick-look --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts file-search-directory-breadcrumb-restoration-stress --session default --fixture agentic-file-tree-breadcrumbs --start-dir repo-root --queries 'AGENTS,src,missing' --navigation-steps enter-directory,breadcrumb-parent,back,forward,filter-tighten,clear-filter --input-modes protocol-set-filter,protocol-click,protocol-key,batch --no-native-input --no-native-pointer --no-native-picker --no-quick-look --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts emoji-picker-skin-tone-category-ux-stress --session default --fixture agentic-emoji-skin-tone --categories people,symbols,flags --queries 'woman technologist,thumbs up,flag' --skin-tones default,medium-dark --steps category-click,skin-tone-open,variant-select,filter-tighten,escape-palette,clear-filter --input-modes protocol-set-filter,protocol-click,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts root-window-source-filter-activation-refusal-stress --session default --fixture agentic-window-rows --queries 'w: ,w: safari,windows: terminal' --window-states focused,minimized,offscreen,duplicate-title --selection-steps filter,sort-z-order,actions,enter-refused,clear-filter --input-modes protocol-set-filter,protocol-key,batch --window-provider fixture-only --no-native-input --no-native-pointer --no-window-activation --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts notes-markdown-preview-scroll-sync-stress --session default --fixture agentic-notes-markdown-preview --notes note-a,note-b --markdown-fixtures headings,checklist,code-block,table,long-link,image-placeholder --edit-steps protocol-insert,protocol-delete,protocol-scroll,preview-toggle,split-resize,switch-note --input-modes protocol-set-input,protocol-click,protocol-key,batch --sandbox-notes-store --no-native-input --no-native-pointer --no-native-picker --no-system-pasteboard --no-network --no-external-services --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts quick-terminal-ansi-scrollback-search-stress --session default --fixture agentic-terminal-ansi-scrollback --transcript-fixtures ansi-colors,wide-emoji,combining-marks,long-lines,hyperlinks,stderr-block,prompt-continuation --search-queries error,emoji,url,long --scroll-positions top,middle,bottom,search-hit --input-modes protocol-set-input,protocol-key,batch --terminal-fixture scripted-local --no-shell-command --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-external-services --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts script-output-inspector-folding-recovery-stress --session default --fixture agentic-script-output-inspector --output-fixtures stdout-long,stderr-long,ansi-stacktrace,json-lines,progress-rewrite,exit-error,exit-success --view-steps run-fixture,filter-output,fold-stderr,expand-stack,clear-filter,retry-dry-run --input-modes protocol-set-filter,protocol-click,protocol-key,batch --script-fixture local-noop --no-handler-spawn --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-external-services --no-process-kill --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts app-launcher-icon-grid-keyboard-navigation-stress --session default --fixture agentic-app-launcher-grid --apps 'Calculator,Script Kit,Safari,Terminal,Very Long Application Name' --grid-states grid,list,filtered,empty,resized --navigation-steps right,down,left,up,home,end,filter,clear-filter --input-modes protocol-key,protocol-set-filter,batch --no-native-input --no-native-pointer --no-app-launch --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts browser-history-time-grouped-privacy-stress --session default --fixture agentic-browser-history-time-groups --browser-fixtures same-domain,private-url,long-title,favicon-missing,duplicate-visit --time-buckets today,yesterday,last-week,older --queries docs,private,missing --privacy-modes redacted-url,favicon-fallback,title-only --input-modes protocol-set-filter,protocol-key,batch --browser-provider fixture-only --no-browser-activation --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts settings-preferences-search-reset-preview-stress --session default --fixture agentic-settings-preferences --preference-fixtures theme,font-size,ui-scale,launch-at-login,agent-profile --control-types toggle,select,slider,text,reset-button --queries theme,font,missing --reset-paths single-setting,section-reset,cancel-reset --input-modes protocol-set-filter,protocol-click,protocol-key,batch --sandbox-config --no-config-write --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts settings-preferences-readonly-detail-panel-stress --session default --fixture agentic-settings-preferences-readonly --sections appearance,windowing,history,advanced --queries theme,mini,history,missing --navigation-steps filter,section-click,detail-focus,clear-filter,escape-restore --input-modes protocol-set-filter,protocol-click,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-external-services --no-security-prompts --no-system-settings --no-tcc-mutation --no-config-write --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts design-picker-preview-restore-visual-stress --session default --fixture agentic-design-picker-preview --catalog-ids default,compact,high-contrast --preview-steps open-picker,preview-next,preview-previous,filter,escape-restore,cmd-w-restore --input-modes protocol-set-filter,protocol-click,protocol-key,batch --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-external-services --no-security-prompts --no-system-settings --no-tcc-mutation --no-config-write --no-design-write --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts dictation-history-transcript-preview-redaction-stress --session default --fixture agentic-dictation-history-preview --transcript-fixtures short,long,redacted,missing-audio,emoji --queries meeting,error,emoji,missing --selection-cycles 6 --view-steps filter,preview,expand-redacted,clear-filter,escape-restore --input-modes protocol-set-filter,protocol-click,protocol-key,batch --dictation-fixture saved-local --no-microphone --no-media-capture --no-native-input --no-native-pointer --no-system-pasteboard --no-network --no-external-services --no-security-prompts --no-system-settings --no-tcc-mutation --no-submit --dry-run-only --local-fixture-only --json
  bun scripts/agentic/index.ts scenario --session default --scenario main-window-exact-id
  bun scripts/agentic/index.ts scenario --session default --scenario actions-dialog-exact-id --index 0
  bun scripts/agentic/index.ts scenario --session default --scenario prompt-popup-exact-id --index 0
  bun scripts/agentic/index.ts scenario --session default --scenario detached-agent_chat-exact-id --index 0
  bun scripts/agentic/index.ts agent_chat-setup-recovery --session default --select-agent opencode --json
  bun scripts/agentic/index.ts help --json`);
    process.exit(0);
    break;
  }

  default:
    result = {
      schemaVersion: SCHEMA_VERSION,
      recipe,
      status: "error",
      steps: [],
      summary: `Unknown recipe: ${recipe}. Run with 'help' for options.`,
    };
    break;
}

console.log(JSON.stringify(result!, null, 2));
process.exit(
  result!.status === "pass" ? 0 : result!.status === "error" ? 2 : 1
);
