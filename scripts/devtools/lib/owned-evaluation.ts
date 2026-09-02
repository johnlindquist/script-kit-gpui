import { isDeepStrictEqual } from "node:util";
import { Driver, DriverCommandRefused, DriverLifecycleError, OWNED_RESPONSE_CODEC, OWNED_RESPONSE_ENCODING, type Json } from "../driver.ts";
import { type ArtifactReference, type ArtifactExpectation, verifyImmutableArtifact } from "../../agentic/build-artifact.ts";
import { type OutputClaim, type OwnedCleanup } from "../../agentic/artifact-lifecycle.ts";
import { issueOwnedEvaluationPermit, OWNED_EVALUATION_LIMITS, type EvaluationLimits } from "./operator-safety.ts";
import { completedFrameIssues, evaluatorTargetIssues, declaredTransitionIssues,
  type AutomationInstance, type AutomationTargetSnapshot, type CompletedFrameIdentity, type OwnedRuntimeIdentity } from "./target-identity.ts";

export type LiveThemeTokenId =
  | "theme.colors.accent.selected" | "theme.colors.background.main" | "theme.colors.background.searchBox"
  | "theme.colors.ui.error" | "theme.opacity.hover" | "theme.opacity.selected" | "theme.opacity.textStrong"
  | "theme.opacity.textMutedAlpha" | "theme.opacity.textHint" | "theme.opacity.textPlaceholder" | "theme.opacity.textIcon";
export interface LiveThemeEdit { readonly tokenId: LiveThemeTokenId; readonly value: number }
export type ObservedEffect =
  | { kind: "stateChanged"; owner: string; revision: number }
  | { kind: "submissionDelivered"; owner: string; receiptId: string; promptInstanceId: string; deliveryCount: number }
  | { kind: "popupOpened" | "popupClosed" | "rootClosed"; target: AutomationInstance }
  | { kind: "noOp"; reason: string }
  | { kind: "refused"; code: string };
export interface ScopedActionReceipt {
  readonly requestId: string;
  readonly operationId: string;
  readonly before: AutomationTargetSnapshot;
  readonly after?: AutomationTargetSnapshot;
  readonly dispatchCompleted: boolean;
  readonly wasDeferred: boolean;
  readonly effect: ObservedEffect;
}
export interface FixtureDescriptor {
  readonly id: string;
  readonly family: string;
  readonly root: "main" | "notes" | "agentChat" | "dictation" | "secondary";
  readonly owner: string;
  readonly parentFixtureId?: string;
  readonly proofBoundary: "owned-production-runtime";
  readonly nativeExclusions: readonly string[];
  readonly factoryOwners?: readonly string[];
  readonly appViewVariant?: string;
  readonly presentationOwner?: string;
  readonly surfaceVariant?: string;
  readonly expectedSemanticSurface: string;
  readonly requiredSemanticIds: readonly string[];
}
export interface ThemeInvalidation {
  readonly target: AutomationInstance;
  readonly revision: number;
  readonly cause: "themePublication";
  readonly invalidationEpoch: number;
}
export interface ThemePublication {
  readonly operation: "applyTheme" | "revertTheme";
  readonly ok: true;
  readonly revision: number;
  readonly previousRevision: number;
  readonly invalidations: readonly ThemeInvalidation[];
  readonly resolved: readonly LiveThemeEdit[];
  readonly phaseDurationsMs: Readonly<Record<string, number>>;
}
export type OwnedAction =
  | { type: "key"; key: string; text?: string; modifiers?: string[] }
  | { type: "setInput"; text: string }
  | { type: "select"; semanticId: string; submit?: boolean }
  | { type: "openActions" }
  | { type: "gpuiEvent"; event: Json; frame?: CompletedFrameIdentity };
export type FixtureControl =
  | ({ family: "agentChat" } & (
      { operation: "submit" | "mutateInputBeforePaint"; text: string } |
      { operation: "retry" | "stop" | "holdDrain" | "retainDrain" | "openHistory" | "openSlashPicker" | "openProfilePicker" } |
      { operation: "emitText"; turnGeneration: number; text: string } |
      { operation: "complete" | "fail" | "releaseDrain"; turnGeneration: number }))
  | ({ family: "flow"; sessionId: number } & (
      { operation: "submit"; text: string } |
      { operation: "emitText"; messageId: string; text: string } |
      { operation: "complete" | "fail"; messageId: string } |
      { operation: "retry" | "stop" | "background" | "resume" }))
  | ({ family: "sdkChat" } & (
      { operation: "submit"; text: string } |
      { operation: "emitText"; messageId: string; text: string } |
      { operation: "complete" | "fail"; messageId: string } |
      { operation: "retry" | "stop" }))
  | ({ family: "dictation" } & (
      { operation: "begin" | "retarget"; destination: "mainFilter" | "mainPrompt" | "notes" | "agentChat" | "dayPage" } |
      { operation: "recording"; text: string; bars: readonly [number, number, number, number, number, number, number, number, number] } |
      { operation: "confirm" | "resume" | "transcribe" | "deliver" | "openMicrophonePicker" }))
  | { family: "notes"; operation: "toggleTask"; markerStart: number; markerEnd: number; checked: boolean }
  | ({ family: "search" } & ({ operation: "prepare"; scenario: string } |
      { operation: "release"; runIds: readonly number[] } | { operation: "advance"; milliseconds: number }))
  | { family: "theme"; operation: "armSaveFailure" | "clearSaveFailure" | "malformedReload" }
  | { family: "fault"; operation: "suppressThemeNotification"; target: AutomationInstance };
export interface AgentChatFixtureObservation {
  threadId: string; transcriptGeneration: number; turnId: number; streamGeneration: number;
  status: "idle" | "streaming" | "waitingforpermission" | "error"; startedTurns: number;
  localStreamCancelled: boolean; remoteCancelRequested: boolean; remoteCancelAcknowledged: boolean;
}
export interface FlowFixtureObservation { sessionId: number; draftText: string; expectedMessageId: string; status: string }
export interface DictationFixtureObservation { phase: string; deliveryOutcome?: "delivered" | "staleTarget" | "refused"; generation: number }
export interface ThemeFixtureObservation {
  family: "theme"; operation: "armSaveFailure" | "clearSaveFailure" | "malformedReload";
  path: "theme.json"; revision?: number; blockerPresent?: boolean;
  originalFileSha256?: string | null; restoredFileSha256?: string | null;
  reloadError?: string; beforeRevision?: number; afterRevision?: number;
  beforeThemeSha256?: string; afterThemeSha256?: string; malformedFileSha256?: string;
  ordinaryRestartProven: false;
}

export const NATIVE_SAFETY_PROBES = [
  "invalidShow", "invalidFocus", "invalidDialog", "invalidTabbing", "invalidOversize",
  "nativeActivation", "nativeIme", "globalPointer", "clipboardRead", "clipboardWrite", "directAppActivation",
  "process", "provider", "credentials", "device", "openExternal", "notification",
  "blankReadback", "failedReadback", "missingRequiredImage", "missingRequiredSvg", "oversizedImage",
  "duplicateSemanticIdentity", "duplicateMeasurementIdentity",
  "deferredDispatch",
] as const;
export type NativeSafetyProbe = typeof NATIVE_SAFETY_PROBES[number];
export interface NativeSafetyProbeResult {
  operation: "probeSafety"; ok: true; probe: NativeSafetyProbe;
  negativeOnly: true; productionEvidence: false; target: AutomationInstance; targetIdentity: AutomationTargetSnapshot;
  implementationGap: string | null; before: Json; after: Json; observation: Json;
  windowStateUnchanged: boolean; ownedCopyUnchanged: boolean; elapsedMs: number;
}

export type SdkPromptCommand =
  | { operation: "begin"; fixtureId: "sdk.arg-roundtrip.v1"; message: Json; channel: "connected" | "full" | "disconnected" }
  | { operation: "drain" | "releaseCapacity" | "close" };
export interface SdkPromptResult {
  operation: "sdkPrompt"; ok: true; fixtureId?: "sdk.arg-roundtrip.v1"; closed?: boolean;
  completion: { completed: boolean; retired: boolean; error: string | null;
    receipt: { prompt: { id: string; generation: number }; sequence: number; outcome: Json } | null };
  messages: Json[]; forwarded: number; capacityHeld?: boolean;
}

export interface ScheduledCapture {
  expected: AutomationTargetSnapshot;
  afterFrameGeneration: number;
  afterNotificationEpoch: number;
}
export interface OwnedFrameCursor { readonly traceGeneration: number; readonly afterFrameGeneration: number }
export interface OwnedFrameAcknowledgement {
  operation: "acknowledgeFrames"; ok: true; target: AutomationInstance; expected: AutomationTargetSnapshot;
  acknowledgedCursor: OwnedFrameCursor; retiredFrames: number; retainedFrames: number; retainedTraceBytes: number;
}
export interface PixelProbe { x: number; y: number }
export interface PixelProbeResult extends PixelProbe { r: number; g: number; b: number; a: number }
export const OWNED_SEARCH_PROVIDER_SOURCES = ["files", "directory", "brain-lexical", "brain-semantic", "tabs", "history",
  "windows", "icons", "notes", "todos", "clipboard", "dictation", "conversations", "spine", "brain-inbox", "scripts",
  "apps", "skills", "validation", "flow-roster"] as const;
export type OwnedSearchProviderSource = typeof OWNED_SEARCH_PROVIDER_SOURCES[number];
export interface OwnedSearchQueryStamp { readonly lifetime: number; readonly revision: number; readonly scopeRevision: number }
export interface OwnedSearchProviderCondition {
  readonly type: "searchProvider"; readonly source: OwnedSearchProviderSource;
  readonly query: OwnedSearchQueryStamp; readonly afterRunId: number;
  readonly acceptCached?: boolean;
}
export interface OwnedFileSearchStreamCondition {
  readonly type: "fileSearchStream"; readonly generation: number; readonly query: string;
}
export interface OwnedFileSearchStreamObservation {
  readonly generation: number; readonly query: string; readonly directory: string | null; readonly showHidden: boolean;
  readonly phase: typeof FILE_SEARCH_STREAM_PHASES[number];
  readonly loading: boolean; readonly resultCount: number; readonly visibleCount: number; readonly failure: string | null;
}
const FILE_SEARCH_STREAM_WAIT = {
  version: 1, conditionType: "fileSearchStream", identityFields: ["generation", "query"],
  terminalPhases: ["completed", "failed", "cancelled", "unavailable"],
} as const;
const FILE_SEARCH_STREAM_PHASES = ["accepted", "running", ...FILE_SEARCH_STREAM_WAIT.terminalPhases] as const;
export interface OwnedFileSearchPreviewCondition {
  readonly type: "fileSearchPreview"; readonly generation: number; readonly query: string; readonly workSequence: number;
}
export interface OwnedFileSearchPreviewObservation {
  readonly version: 1; readonly generation: number; readonly query: string; readonly workSequence: number;
  readonly phase: "held"; readonly path: string; readonly decoded: boolean; readonly contentHash: string | null;
  readonly logicalTimeMs: number; readonly dueAtMs: number;
}
const FILE_SEARCH_PREVIEW_WAIT = {
  version: 1, conditionType: "fileSearchPreview", identityFields: ["generation", "query", "workSequence"], phase: "held",
} as const;
export interface OwnedSearchProviderOwner {
  readonly source: OwnedSearchProviderSource; readonly generation: number; readonly workQuery: string; readonly workScope: string;
  readonly consumer: OwnedSearchQueryStamp | null; readonly publicationPolicy: "visible" | "cache-only" | "visible-synchronous";
  readonly queryBound: boolean; readonly terminal: "success" | "empty" | "failed" | "unavailable" | "disconnected" | "cancelled" | "staleDiscarded" | null;
}
export const OWNED_SEARCH_CACHE_SOURCES = ["tabs", "files", "directory", "history", "notes", "todos", "clipboard", "dictation", "conversations", "windows"] as const;
export interface OwnedSearchSourceCacheReadiness {
  readonly source: typeof OWNED_SEARCH_CACHE_SOURCES[number]; readonly query: OwnedSearchQueryStamp;
  readonly cacheIdentity: string; readonly cacheStateRevision: number | null; readonly rowCount: number;
}
export interface OwnedSearchProviderWaitObservation {
  readonly version: 1; readonly source: OwnedSearchProviderSource; readonly query: OwnedSearchQueryStamp; readonly afterRunId: number;
  readonly status: "admitted" | "blocked" | "settled" | "cached"; readonly owner: OwnedSearchProviderOwner | null;
  readonly run: SearchProviderRun | null; readonly blockers: readonly { owner: OwnedSearchProviderOwner; run: SearchProviderRun }[];
  readonly pendingDesired: boolean; readonly availabilityReason: string;
  readonly cache?: OwnedSearchSourceCacheReadiness;
}

export interface SearchProviderRun {
  id: number; kind: "worker" | "sourceChange" | "synchronousRead";
  source: string; query: string; generation: number; state: string; publicationPolicy: string | null; outcome?: string | null;
  plannedResponse?: string; resultCount?: number | null; payloadPhase?: number;
  admissionApplied?: boolean; capabilityRefusal?: string | null; originAdmissionId?: number | null;
  payloadPrepared?: boolean; pendingDelivery?: boolean; deliveryDueAtMs?: number | null;
  deliveryAttempted?: boolean; senderDropped?: boolean;
}
export interface SearchProviderObservation {
  version: number; scenario: string; logicalTimeMs: number; displayUnixMs: number; retired: boolean; overflow: boolean;
  runs: SearchProviderRun[]; pendingRunIds: number[];
  pendingSourceChanges?: readonly { source: string; dueAtMs: number }[];
  pendingPreviewCompletions?: readonly OwnedFileSearchPreviewObservation[];
  retiredGate?: SearchProviderObservation | null;
}
export interface SearchSourcePlan {
  source: string; input: string; scope: "root" | "directory" | "spine";
  workKind: "query-bound" | "catalogue" | "synchronous";
}
export interface SearchFixtureObservation {
  searchProviders: SearchProviderObservation;
  pendingForegroundTasks?: number; pendingBackgroundTasks?: number; pendingEffects?: number;
  pendingDirtyWindows?: number; hasPendingTasksOrTimers?: boolean;
}
export interface SearchFixturePreparation extends SearchFixtureObservation {
  suggestedInput: string; sourcePlans: readonly SearchSourcePlan[];
  fileViewInputs: { full: string; mini: string; preview: string };
}
export interface OwnedCopySinkObservation {
  text: string;
  receipt: { destination: "ownedProcessLocal"; byteLength: number; sha256: string; revision: number };
}
export interface OwnedQueryOptions { includeImage?: boolean; includeHeaders?: boolean }
export type DesignCommand =
  | { operation: "bootstrap"; launchNonce: string; policySha256: string }
  | { operation: "catalog" }
  | { operation: "mount"; fixtureId: string; parent?: AutomationInstance }
  | { operation: "captureFrame"; target: AutomationInstance; includeImage: boolean; scheduled?: ScheduledCapture; frameCursor?: OwnedFrameCursor }
  | { operation: "acknowledgeFrames"; target: AutomationInstance; expected: AutomationTargetSnapshot; cursor: OwnedFrameCursor }
  | { operation: "applyTheme"; expectedRevision: number; edits: readonly LiveThemeEdit[] }
  | { operation: "revertTheme"; expectedRevision: number }
  | { operation: "unmount"; target: AutomationInstance; expected: AutomationTargetSnapshot }
  | { operation: "fixtureControl"; target: AutomationInstance; expected: AutomationTargetSnapshot; control: FixtureControl }
  | { operation: "sdkPrompt"; target: AutomationInstance; expected: AutomationTargetSnapshot; command: SdkPromptCommand }
  | { operation: "probeSafety"; target: AutomationInstance; expected: AutomationTargetSnapshot; probe: NativeSafetyProbe }
  | { operation: "diagnose" }
  | { operation: "end" };
export interface OwnedFrameCapture {
  operation: "captureFrame"; ok: true; frame: CompletedFrameIdentity; snapshot: RenderCapture;
  state: Json; elements: Json; layout: Json; phaseDurationsMs: Readonly<Record<string, number>>; frameEvidence?: Json;
  frameHistoryBundle?: { version: 1; captureFrameCount: number; stateFrameCount: number };
}
export type DesignResult =
  | { operation: "bootstrap"; ok: true; identity: OwnedRuntimeIdentity; launchNonce: string; policySha256: string;
      guards: Readonly<Record<string, boolean>>; limits: EvaluationLimits }
  | { operation: "catalog"; ok: true; fixtures: readonly FixtureDescriptor[]; targets: readonly AutomationTargetSnapshot[];
      operations: readonly string[]; safetyProbes: readonly NativeSafetyProbe[]; settings: Readonly<Record<string, unknown>>; runtimeQualified: boolean;
      searchFixtures?: { fixtureId: string; version: number; scenarios: readonly { id: string }[]; providers: readonly string[] };
      responseEncoding?: typeof OWNED_RESPONSE_CODEC;
      fileSearchStreamWait?: typeof FILE_SEARCH_STREAM_WAIT;
      fileSearchPreviewWait?: typeof FILE_SEARCH_PREVIEW_WAIT;
      frameCursor?: { version: 1; operation: "getState"; captureFrame?: true;
        captureHistoryBundle?: { version: 1; requiresFrameCursor: true; pageScope: "captureBundle"; decodedScope: "complete" };
        searchMetadataRef?: { version: 1; paintBindingIndex: true } };
      frameAcknowledgement?: { version: 1; operation: "acknowledgeFrames"; retainsCursorFrame: true; readCursorsArePassive: true; draws: false };
      searchProviderWait?: { version: 1; conditionType: "searchProvider"; sources: readonly OwnedSearchProviderSource[];
        statuses: readonly ["admitted", "blocked", "settled", "cached"]; sourceChange: "explicitFixtureControl";
        acceptCached: true; cacheAfterRunId: 0; cacheSources: readonly OwnedSearchSourceCacheReadiness["source"][] } }
  | { operation: "mount"; ok: true; fixtureId: string; target: AutomationTargetSnapshot }
  | OwnedFrameCapture
  | OwnedFrameAcknowledgement
  | ThemePublication
  | { operation: "unmount"; ok: true; target: AutomationInstance; closed: boolean }
  | { operation: "fixtureControl"; ok: true; actionReceipt?: ScopedActionReceipt;
      observation: AgentChatFixtureObservation | FlowFixtureObservation | DictationFixtureObservation | ThemeFixtureObservation | SearchFixtureObservation | { suppressed: AutomationInstance } }
  | SdkPromptResult
  | NativeSafetyProbeResult
  | { operation: "end"; ok: true; ownedWindowsClosed: boolean; remainingWindows: number }
  | { operation: "diagnose"; ok: true; identity: OwnedRuntimeIdentity; targets: readonly AutomationTargetSnapshot[];
      refusedEffects: number; completedFixtureEffects: number; pendingEffects: number; framesCompleted: number }
  | { operation: DesignCommand["operation"]; ok: false; error: { code: string; message: string }; revision?: number };

export interface RenderCapture {
  readonly source: "gpuiRenderReadback";
  readonly scope: "liveAutomationWindowRenderReadback";
  readonly status: "captured" | "unsupported" | "targetNotFound" | "blankImageRejected" | "captureFailed";
  readonly frameIdentity: CompletedFrameIdentity;
  readonly correlationId?: string;
  readonly capture?: { width: number; height: number; hiDpi: boolean; pngBase64?: string; sha256?: string };
  readonly limitation: string;
  readonly phaseDurationsMs?: Readonly<Record<string, number>>;
  readonly pixelProbes?: readonly PixelProbeResult[];
  readonly scaleFactor?: number;
}
export class EvaluationContractError extends Error {
  constructor(readonly code: string, readonly details: readonly string[] = []) { super([code, ...details].join(": ")); }
}

export function validateThemeEdits(value: unknown): readonly LiveThemeEdit[] {
  const allowed: Record<LiveThemeTokenId, "color" | "opacity"> = {
    "theme.colors.accent.selected": "color", "theme.colors.background.main": "color",
    "theme.colors.background.searchBox": "color", "theme.colors.ui.error": "color",
    "theme.opacity.hover": "opacity", "theme.opacity.selected": "opacity", "theme.opacity.textStrong": "opacity",
    "theme.opacity.textMutedAlpha": "opacity", "theme.opacity.textHint": "opacity", "theme.opacity.textPlaceholder": "opacity",
    "theme.opacity.textIcon": "opacity",
  };
  if (!Array.isArray(value) || !value.length || value.length > 16) throw new EvaluationContractError("invalid_theme_edits");
  const seen = new Set<string>();
  for (const edit of value) {
    if (!edit || typeof edit !== "object" || Object.keys(edit).some(key => key !== "tokenId" && key !== "value") ||
        !Object.hasOwn(allowed, edit.tokenId) || seen.has(edit.tokenId) || typeof edit.value !== "number" || !Number.isFinite(edit.value))
      throw new EvaluationContractError("invalid_theme_edit");
    seen.add(edit.tokenId);
    if (allowed[edit.tokenId as LiveThemeTokenId] === "color" ? !Number.isSafeInteger(edit.value) || edit.value < 0 || edit.value > 0xffffff : edit.value < 0 || edit.value > 1)
      throw new EvaluationContractError("theme_value_out_of_range");
  }
  // Cross-token ladders and effective inherited values remain Rust theme authority.
  return value as LiveThemeEdit[];
}

export function publicationCausalityIssues(publication: ThemePublication,
  targets: readonly AutomationInstance[], beforeEpochs?: Readonly<Record<string, number>>): string[] {
  const issues: string[] = [];
  if (!Number.isSafeInteger(publication.revision) || publication.revision <= publication.previousRevision) issues.push("publication_revision_not_advanced");
  for (const target of targets) {
    const matches = publication.invalidations.filter(item => item.target.id === target.id && item.target.generation === target.generation);
    if (matches.length !== 1 || matches[0]!.cause !== "themePublication" || matches[0]!.revision !== publication.revision ||
        !Number.isSafeInteger(matches[0]!.invalidationEpoch) || matches[0]!.invalidationEpoch <= (beforeEpochs?.[`${target.id}:${target.generation}`] ?? -1))
      issues.push(`publication_not_delivered:${target.id}:${target.generation}`);
  }
  return issues;
}

function validateFrameCursor(cursor: OwnedFrameCursor): OwnedFrameCursor {
  if (!cursor || typeof cursor !== "object" || Array.isArray(cursor) ||
      Object.keys(cursor).some(key => key !== "traceGeneration" && key !== "afterFrameGeneration") ||
      !Number.isSafeInteger(cursor.traceGeneration) || cursor.traceGeneration < 1 ||
      !Number.isSafeInteger(cursor.afterFrameGeneration) || cursor.afterFrameGeneration < 0)
    throw new EvaluationContractError("frame_cursor_invalid");
  return { traceGeneration: cursor.traceGeneration, afterFrameGeneration: cursor.afterFrameGeneration };
}

function validateFramePage(trace: Json, cursor: OwnedFrameCursor, target: AutomationInstance, runtime: OwnedRuntimeIdentity, scope?: "captureBundle"): void {
  if (trace?.traceGeneration !== cursor.traceGeneration || trace?.afterFrameGeneration !== cursor.afterFrameGeneration ||
      !Number.isSafeInteger(trace?.latestFrameGeneration) || trace.latestFrameGeneration < cursor.afterFrameGeneration ||
      trace.traceOverflow !== false || !Array.isArray(trace.completedFrames) ||
      (scope ? trace.historyScope !== scope : Object.hasOwn(trace, "historyScope")))
    throw new EvaluationContractError("frame_cursor_response_mismatch");
  let previous = cursor.afterFrameGeneration;
  for (const stamp of trace.completedFrames) {
    const generation = stamp?.frame?.target?.frameGeneration;
    if (stamp?.traceGeneration !== cursor.traceGeneration || !Number.isSafeInteger(generation) || generation <= previous ||
        generation > trace.latestFrameGeneration || completedFrameIssues(target, stamp.frame, runtime).length)
      throw new EvaluationContractError("frame_cursor_response_mismatch");
    previous = generation;
  }
  // Native negative-readback isolation can advance completed generation without a retained stamp.
}

function restoreSearchMetadataRefs(page: Json | undefined, optedIn: boolean): void {
  if (!page || typeof page !== "object" || Array.isArray(page)) return;
  const restore = (stamp: Json): void => {
    if (!stamp || typeof stamp !== "object" || Array.isArray(stamp) || !Object.hasOwn(stamp, "searchMetadataRef")) return;
    const index = stamp.searchMetadataRef;
    const bindings = stamp.paintBindings;
    if (!optedIn || Object.hasOwn(stamp, "search") || !stamp.frame || typeof stamp.frame !== "object" || Array.isArray(stamp.frame) ||
        !Number.isSafeInteger(index) || index < 0 || !Array.isArray(bindings) || index >= bindings.length)
      throw new EvaluationContractError("frame_cursor_response_mismatch");
    const binding = bindings[index];
    if (binding?.kind !== "mainSearch" || binding.id !== "main-search" || !binding.metadata ||
        typeof binding.metadata !== "object" || Array.isArray(binding.metadata) ||
        bindings.some((candidate: Json, position: number) => position !== index && candidate?.kind === "mainSearch" && candidate.id === "main-search"))
      throw new EvaluationContractError("frame_cursor_response_mismatch");
    stamp.search = binding.metadata;
    delete stamp.searchMetadataRef;
  };
  restore(page);
  if (Array.isArray(page.completedFrames)) for (const stamp of page.completedFrames) restore(stamp);
}

function restoreCaptureFrameHistories(result: OwnedFrameCapture, cursor: OwnedFrameCursor,
  target: AutomationInstance, runtime: OwnedRuntimeIdentity): void {
  const bundle = result.frameHistoryBundle;
  const capture = result.frameEvidence;
  const state = result.state?.frameEvidence;
  if (!bundle || typeof bundle !== "object" || Array.isArray(bundle) || bundle.version !== 1 ||
      Object.keys(bundle).length !== 3 || Object.keys(bundle).some(key => !["version", "captureFrameCount", "stateFrameCount"].includes(key)) ||
      !Number.isSafeInteger(bundle.captureFrameCount) || bundle.captureFrameCount < 0 ||
      !Number.isSafeInteger(bundle.stateFrameCount) || bundle.stateFrameCount < 0)
    throw new EvaluationContractError("frame_cursor_response_mismatch");
  validateFramePage(capture!, cursor, target, runtime, "captureBundle");
  validateFramePage(state, cursor, target, runtime, "captureBundle");
  if (!isDeepStrictEqual(capture!.frame, result.frame) || capture!.traceGeneration !== cursor.traceGeneration ||
      completedFrameIssues(target, capture!.frame, runtime).length)
    throw new EvaluationContractError("frame_cursor_response_mismatch");
  // FrameStamp is the complete current projection, not a reduced search-specific view.
  // Keep every stamp field; only the existing history-page annotations are removed.
  const current = { ...capture } as Json;
  for (const key of ["completedFrames", "afterFrameGeneration", "latestFrameGeneration", "traceOverflow", "maxCompletedStamps",
      "maxRetainedTraceBytes", "scheduledCapability", "transientPixelsRetained", "transientPixelEvidence", "historyScope"])
    delete current[key];
  const union = new Map<number, Json>([[current.frame.target.frameGeneration, current]]);
  for (const page of [capture!, state]) for (const stamp of page.completedFrames) {
    const generation = stamp.frame.target.frameGeneration;
    if (union.has(generation)) throw new EvaluationContractError("frame_cursor_response_mismatch");
    union.set(generation, stamp);
  }
  const ordered = [...union.values()].sort((left, right) => left.frame.target.frameGeneration - right.frame.target.frameGeneration);
  const captureFrames = ordered.filter(stamp => stamp.frame.target.frameGeneration > capture!.afterFrameGeneration &&
    stamp.frame.target.frameGeneration <= capture!.latestFrameGeneration);
  const stateFrames = ordered.filter(stamp => stamp.frame.target.frameGeneration > state.afterFrameGeneration &&
    stamp.frame.target.frameGeneration <= state.latestFrameGeneration);
  if (captureFrames.length !== bundle.captureFrameCount || stateFrames.length !== bundle.stateFrameCount)
    throw new EvaluationContractError("frame_cursor_response_mismatch");
  capture!.completedFrames = captureFrames;
  state.completedFrames = stateFrames;
  delete capture!.historyScope;
  delete state.historyScope;
  delete result.frameHistoryBundle;
}

function validSearchQuery(value: Json): value is OwnedSearchQueryStamp {
  return !!value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length === 3 &&
    Object.keys(value).every(key => ["lifetime", "revision", "scopeRevision"].includes(key)) &&
    [value.lifetime, value.revision, value.scopeRevision].every(value => Number.isSafeInteger(value) && value >= 0);
}
function validateSearchProviderCondition(value: Json): OwnedSearchProviderCondition {
  if (!value || typeof value !== "object" || Array.isArray(value) || value.type !== "searchProvider" ||
      Object.keys(value).some(key => !["type", "source", "query", "afterRunId", "acceptCached"].includes(key)) ||
      !OWNED_SEARCH_PROVIDER_SOURCES.includes(value.source) || !validSearchQuery(value.query) ||
      !Number.isSafeInteger(value.afterRunId) || value.afterRunId < 0 ||
      (Object.hasOwn(value, "acceptCached") && typeof value.acceptCached !== "boolean") ||
      (value.acceptCached === true && value.afterRunId !== 0))
    throw new EvaluationContractError("search_provider_condition_invalid");
  return { type: "searchProvider", source: value.source, query: { ...value.query }, afterRunId: value.afterRunId,
    ...(value.acceptCached === undefined ? {} : { acceptCached: value.acceptCached }) };
}
function validateFileSearchStreamCondition(value: Json): OwnedFileSearchStreamCondition {
  if (!value || typeof value !== "object" || Array.isArray(value) || value.type !== "fileSearchStream" ||
      Object.keys(value).some(key => !["type", "generation", "query"].includes(key)) ||
      !Number.isSafeInteger(value.generation) || value.generation <= 0 || typeof value.query !== "string")
    throw new EvaluationContractError("file_search_stream_condition_invalid");
  return { type: "fileSearchStream", generation: value.generation, query: value.query };
}
export function isOwnedFileSearchStreamObservation(value: Json): value is OwnedFileSearchStreamObservation {
  return !!value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length === 9 &&
    Object.keys(value).every(key => ["generation", "query", "directory", "showHidden", "phase", "loading", "resultCount", "visibleCount", "failure"].includes(key)) &&
    Number.isSafeInteger(value.generation) && value.generation >= 0 && typeof value.query === "string" &&
    (value.directory === null || typeof value.directory === "string") && typeof value.showHidden === "boolean" &&
    FILE_SEARCH_STREAM_PHASES.includes(value.phase) && typeof value.loading === "boolean" &&
    Number.isSafeInteger(value.resultCount) && value.resultCount >= 0 &&
    Number.isSafeInteger(value.visibleCount) && value.visibleCount >= 0 &&
    (value.failure === null || typeof value.failure === "string");
}
function validateFileSearchStreamObservation(value: Json, condition: OwnedFileSearchStreamCondition): void {
  if (!isOwnedFileSearchStreamObservation(value) || value.generation !== condition.generation || value.query !== condition.query ||
      !FILE_SEARCH_STREAM_WAIT.terminalPhases.some(phase => phase === value.phase) || value.loading !== false)
    throw new EvaluationContractError("file_search_stream_wait_contract_mismatch");
}
function validateFileSearchPreviewCondition(value: Json): OwnedFileSearchPreviewCondition {
  if (!value || typeof value !== "object" || Array.isArray(value) || value.type !== "fileSearchPreview" ||
      Object.keys(value).some(key => !["type", "generation", "query", "workSequence"].includes(key)) ||
      !Number.isSafeInteger(value.generation) || value.generation <= 0 || typeof value.query !== "string" ||
      !Number.isSafeInteger(value.workSequence) || value.workSequence <= 0)
    throw new EvaluationContractError("file_search_preview_condition_invalid");
  return { type: "fileSearchPreview", generation: value.generation, query: value.query, workSequence: value.workSequence };
}
export function isOwnedFileSearchPreviewObservation(value: Json): value is OwnedFileSearchPreviewObservation {
  return !!value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length === 10 &&
    Object.keys(value).every(key => ["version", "generation", "query", "workSequence", "phase", "path", "decoded", "contentHash", "logicalTimeMs", "dueAtMs"].includes(key)) &&
    value.version === 1 && value.phase === "held" && Number.isSafeInteger(value.generation) && value.generation > 0 &&
    typeof value.query === "string" && Number.isSafeInteger(value.workSequence) && value.workSequence > 0 &&
    typeof value.path === "string" && value.path.length > 0 && typeof value.decoded === "boolean" &&
    (value.decoded ? typeof value.contentHash === "string" && /^[a-f0-9]{64}$/.test(value.contentHash) : value.contentHash === null) &&
    Number.isSafeInteger(value.logicalTimeMs) && value.logicalTimeMs >= 0 &&
    Number.isSafeInteger(value.dueAtMs) && value.dueAtMs > value.logicalTimeMs;
}
function validateFileSearchPreviewObservation(value: Json, condition: OwnedFileSearchPreviewCondition): void {
  if (!isOwnedFileSearchPreviewObservation(value) || value.generation !== condition.generation || value.query !== condition.query || value.workSequence !== condition.workSequence)
    throw new EvaluationContractError("file_search_preview_wait_contract_mismatch");
}
function validProviderOwner(owner: Json): owner is OwnedSearchProviderOwner {
  return !!owner && typeof owner === "object" && !Array.isArray(owner) && OWNED_SEARCH_PROVIDER_SOURCES.includes(owner.source) &&
    Number.isSafeInteger(owner.generation) && owner.generation >= 0 && typeof owner.workQuery === "string" && typeof owner.workScope === "string" &&
    (owner.consumer === null || validSearchQuery(owner.consumer)) && typeof owner.queryBound === "boolean" &&
    ["visible", "cache-only", "visible-synchronous"].includes(owner.publicationPolicy) &&
    (owner.terminal === null || ["success", "empty", "failed", "unavailable", "disconnected", "cancelled", "staleDiscarded"].includes(owner.terminal));
}
function validProviderRun(run: Json, owner: OwnedSearchProviderOwner): run is SearchProviderRun {
  return !!run && typeof run === "object" && !Array.isArray(run) && Number.isSafeInteger(run.id) && run.id > 0 &&
    run.source === owner.source && run.generation === owner.generation && typeof run.query === "string" &&
    ["worker", "synchronousRead"].includes(run.kind) && typeof run.state === "string" &&
    (run.publicationPolicy === null || typeof run.publicationPolicy === "string");
}
export function isOwnedSearchSourceCacheReadiness(value: Json): value is OwnedSearchSourceCacheReadiness {
  return !!value && typeof value === "object" && !Array.isArray(value) &&
    Object.keys(value).length === 5 && Object.keys(value).every(key =>
      ["source", "query", "cacheIdentity", "cacheStateRevision", "rowCount"].includes(key)) &&
    OWNED_SEARCH_CACHE_SOURCES.includes(value.source) && validSearchQuery(value.query) &&
    typeof value.cacheIdentity === "string" && value.cacheIdentity.length > 0 &&
    Number.isSafeInteger(value.rowCount) && value.rowCount >= 0 &&
    (["files", "directory"].includes(value.source) ? value.cacheStateRevision === null :
      Number.isSafeInteger(value.cacheStateRevision) && value.cacheStateRevision >= 0);
}

const PROVIDER_TERMINAL_RUNS: Readonly<Record<string, readonly [string, string]>> = {
  success: ["completed", "success"], empty: ["completed", "empty"], failed: ["failed", "error"],
  unavailable: ["unavailable", "unavailable"], disconnected: ["failed", "disconnected"],
};
function validateSearchProviderObservation(value: Json, condition: OwnedSearchProviderCondition): void {
  if (value?.version !== 1 || value.source !== condition.source || !validSearchQuery(value.query) ||
      value.query.lifetime !== condition.query.lifetime || value.query.revision !== condition.query.revision ||
      value.query.scopeRevision !== condition.query.scopeRevision || value.afterRunId !== condition.afterRunId ||
      !Array.isArray(value.blockers) || typeof value.pendingDesired !== "boolean" ||
      (value.owner !== null && (!validProviderOwner(value.owner) || value.owner.source !== condition.source)))
    throw new EvaluationContractError("search_provider_wait_contract_mismatch");
  if (value.status === "cached") {
    if (condition.acceptCached !== true || condition.afterRunId !== 0 || value.owner !== null || value.run !== null ||
        value.blockers.length || value.availabilityReason !== "sourceCacheReuse" || !isOwnedSearchSourceCacheReadiness(value.cache) ||
        value.cache.source !== condition.source || value.cache.query.lifetime !== condition.query.lifetime ||
        value.cache.query.revision !== condition.query.revision || value.cache.query.scopeRevision !== condition.query.scopeRevision)
      throw new EvaluationContractError("search_provider_wait_contract_mismatch");
    return;
  }
  if (value.cache !== undefined) throw new EvaluationContractError("search_provider_wait_contract_mismatch");
  if (value.status === "blocked") {
    const lanes = condition.source === "files" || condition.source === "directory" ? ["files", "directory"] : [condition.source];
    const ids = new Set<number>();
    if (!value.pendingDesired || value.run !== null || value.availabilityReason !== "pendingReplacement" ||
        value.blockers.length < 1 || value.blockers.length > lanes.length || value.blockers.some((blocker: Json) => {
          if (!validProviderOwner(blocker?.owner) || !lanes.includes(blocker.owner.source) || blocker.owner.terminal !== null ||
              !validProviderRun(blocker.run, blocker.owner) || blocker.run.kind !== "worker" || blocker.run.state !== "held" ||
              blocker.run.outcome != null || blocker.run.capabilityRefusal != null || ids.has(blocker.run.id)) return true;
          ids.add(blocker.run.id); return false;
        })) throw new EvaluationContractError("search_provider_wait_contract_mismatch");
    return;
  }
  const owner = value.owner as OwnedSearchProviderOwner | null;
  if (!owner || value.pendingDesired || value.blockers.length || !validProviderRun(value.run, owner) ||
      value.run.id <= condition.afterRunId || (owner.queryBound && (!owner.consumer ||
        owner.consumer.lifetime !== condition.query.lifetime || owner.consumer.revision !== condition.query.revision ||
        owner.consumer.scopeRevision !== condition.query.scopeRevision)))
    throw new EvaluationContractError("search_provider_wait_contract_mismatch");
  if (value.status === "admitted") {
    if (owner.terminal !== null || value.run.kind !== "worker" || value.run.state !== "held" || value.run.outcome != null ||
        value.run.capabilityRefusal != null || value.availabilityReason !== "heldCurrentRun")
      throw new EvaluationContractError("search_provider_wait_contract_mismatch");
  } else if (value.status === "settled") {
    const terminal = owner.terminal && Object.hasOwn(PROVIDER_TERMINAL_RUNS, owner.terminal) ? PROVIDER_TERMINAL_RUNS[owner.terminal] : undefined;
    if (!terminal || value.run.state !== terminal[0] || value.run.outcome !== terminal[1] || value.availabilityReason !== terminal[1])
      throw new EvaluationContractError("search_provider_wait_contract_mismatch");
  } else throw new EvaluationContractError("search_provider_wait_contract_mismatch");
}

/** One transport and one process owner. This object never attaches to an operator session. */
export class OwnedEvaluationClient {
  private readonly mounted = new Map<string, AutomationTargetSnapshot>();
  private closePromise?: Promise<OwnedCleanup>;
  private frames = 0;
  private images = 0;
  lastFramePhaseDurationsMs: Readonly<Record<string, number>> = {};
  private constructor(readonly driver: Driver) {}

  static async launch(repositoryRoot: string, reference: ArtifactReference, claim: OutputClaim,
    fixtureIds: readonly string[], sourcePolicy: ArtifactExpectation["sourcePolicy"] = "current-content",
    options: { readonly maxLifetimeMs?: number; readonly nativeGlass?: "platform-default" | "disabled" } = {}): Promise<OwnedEvaluationClient> {
    const artifact = verifyImmutableArtifact(repositoryRoot, reference, {
      kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy,
    });
    const permit = issueOwnedEvaluationPermit(artifact, claim, fixtureIds, options);
    const driver = await Driver.launch({ immutableArtifact: reference, ownedEvaluation: permit, sessionName: "owned-evaluation" });
    return new OwnedEvaluationClient(driver);
  }
  get identity(): OwnedRuntimeIdentity { return this.driver.qualification!.identity as OwnedRuntimeIdentity; }
  get cleanup(): OwnedCleanup { return this.driver.finalization; }
  get targets(): readonly AutomationTargetSnapshot[] { return [...this.mounted.values()]; }
  private reconcileTargets(targets: readonly AutomationTargetSnapshot[]): void {
    if (!Array.isArray(targets)) throw new EvaluationContractError("invalid_target_catalog");
    const current = new Map<string, AutomationTargetSnapshot>();
    for (const target of targets) {
      if (typeof target?.windowId !== "string" || current.has(target.windowId))
        throw new EvaluationContractError("invalid_target_catalog");
      const issues = evaluatorTargetIssues({ type: "instance", id: target.windowId, generation: target.windowGeneration }, target);
      if (issues.length) throw new EvaluationContractError("invalid_target_catalog", issues);
      current.set(target.windowId, target);
    }
    // Validate the complete runtime snapshot before retiring any cached lifetime.
    this.mounted.clear();
    for (const target of current.values()) this.mounted.set(`${target.windowId}:${target.windowGeneration}`, target);
  }

  async design(command: DesignCommand): Promise<DesignResult> {
    const response = await this.driver.request({ type: "design", command }, command.operation === "probeSafety" ? { timeoutMs: 8000 } : undefined);
    const result = response.result as DesignResult;
    if (!result || result.operation !== command.operation || typeof result.ok !== "boolean")
      throw new EvaluationContractError("design_result_contract_mismatch");
    if (result.ok === false) throw new DriverCommandRefused(result.error.code, response.requestId);
    if (result.operation === "captureFrame" && (typeof response.requestId !== "string" || !response.requestId ||
        result.snapshot?.correlationId !== response.requestId))
      throw new EvaluationContractError("capture_response_correlation_mismatch");
    return result;
  }
  async discover() {
    const result = await this.design({ operation: "catalog" });
    if (result.operation !== "catalog" || !result.ok) throw new EvaluationContractError("catalog_result_required");
    if (!Array.isArray(result.fixtures) || new Set(result.fixtures.map(fixture => fixture.id)).size !== result.fixtures.length)
      throw new EvaluationContractError("invalid_fixture_catalog");
    if (Object.hasOwn(result, "responseEncoding") && !isDeepStrictEqual(result.responseEncoding, OWNED_RESPONSE_CODEC))
      throw new EvaluationContractError("response_encoding_capability_mismatch");
    if (Object.hasOwn(result, "fileSearchStreamWait") && !isDeepStrictEqual(result.fileSearchStreamWait, FILE_SEARCH_STREAM_WAIT))
      throw new EvaluationContractError("file_search_stream_capability_mismatch");
    if (Object.hasOwn(result, "fileSearchPreviewWait") && !isDeepStrictEqual(result.fileSearchPreviewWait, FILE_SEARCH_PREVIEW_WAIT))
      throw new EvaluationContractError("file_search_preview_capability_mismatch");
    this.reconcileTargets(result.targets);
    if (result.responseEncoding) this.driver.enableResponseEncoding(OWNED_RESPONSE_ENCODING);
    return result;
  }
  async mount(fixtureId: string, parent?: AutomationInstance): Promise<AutomationInstance> {
    const result = await this.design({ operation: "mount", fixtureId, ...(parent ? { parent } : {}) });
    if (result.operation !== "mount" || !result.ok || result.fixtureId !== fixtureId) throw new EvaluationContractError("mount_result_required");
    const target: AutomationInstance = { type: "instance", id: result.target.windowId, generation: result.target.windowGeneration };
    const issues = evaluatorTargetIssues(target, result.target);
    if (issues.length) throw new EvaluationContractError("invalid_mounted_identity", issues);
    this.mounted.set(`${target.id}:${target.generation}`, result.target);
    return target;
  }
  async inspect(target: AutomationInstance, frameCursor?: OwnedFrameCursor): Promise<Json> {
    const cursor = frameCursor === undefined ? undefined : validateFrameCursor(frameCursor);
    const state = await this.driver.request({ type: "getState", target, ...(cursor ? { frameCursor: cursor } : {}) });
    restoreSearchMetadataRefs(state.frameEvidence, cursor !== undefined);
    const identity = (state.targetIdentity ?? state.surfaceContract?.targetIdentity) as AutomationTargetSnapshot;
    const issues = evaluatorTargetIssues(target, identity);
    if (issues.length) throw new EvaluationContractError("invalid_state_identity", issues);
    if (state.frameHistoryBundle !== undefined || Object.hasOwn(state.frameEvidence ?? {}, "historyScope"))
      throw new EvaluationContractError("frame_cursor_response_mismatch");
    if (cursor) validateFramePage(state.frameEvidence, cursor, target, this.identity);
    this.mounted.set(`${target.id}:${target.generation}`, identity);
    return state;
  }
  async query(target: AutomationInstance, facet: "elements" | "layout" | "text" | "frame", options: OwnedQueryOptions = {}): Promise<Json> {
    if (facet === "frame") return this.captureFrame(target, options.includeImage ?? false);
    const response = await this.driver.request(facet === "layout" ? { type: "getLayoutInfo", target } : { type: "getElements", target, includeHeaders: options.includeHeaders ?? false });
    if (facet === "text") return { ...response, elements: (response.elements ?? []).filter((element: Json) => typeof element.text === "string" || typeof element.label === "string") };
    return response;
  }
  async act(target: AutomationInstance, action: OwnedAction, expected?: AutomationTargetSnapshot): Promise<Json> {
    // Explicit observations are authority, not hints. In particular, never heal a stale target.
    const before = expected ?? (await this.inspect(target)).targetIdentity ?? this.mounted.get(`${target.id}:${target.generation}`)!;
    const expectedIssues = evaluatorTargetIssues(target, before);
    if (expectedIssues.length) throw new EvaluationContractError("invalid_action_expectation", expectedIssues);
    if (action.type === "gpuiEvent" && /^(mouse|scroll)/.test(action.event.type)) {
      const frameIssues = completedFrameIssues(target, action.frame!, this.identity, before);
      if (!action.frame || action.frame.target.frameGeneration !== before.frameGeneration || frameIssues.length)
        throw new EvaluationContractError("coordinate_action_frame_required", frameIssues);
    }
    const command = action.type === "key" || action.type === "gpuiEvent" ? {
      type: "simulateGpuiEvent", target, expected: before,
      ...(action.type === "gpuiEvent" && /^(mouse|scroll)/.test(action.event.type) ? { expectedFrame: action.frame } : {}),
      event: action.type === "gpuiEvent" ? action.event :
        { type: "keyDown", key: action.key, text: action.text, modifiers: action.modifiers ?? [] },
    } : {
      // One effect per bound request; this does not claim multi-command atomicity.
      type: "batch", target, expected: before, commands: [action.type === "select" ?
        { type: "selectBySemanticId", semanticId: action.semanticId, submit: action.submit ?? false } : action],
      options: { stopOnError: true, timeout: 5000 },
    };
    const result = await this.driver.request(command);
    const receipt = (result.actionReceipt ?? result.results?.[0]?.actionReceipt) as ScopedActionReceipt | undefined;
    if (!receipt || receipt.requestId !== result.requestId || !receipt.operationId)
      throw new EvaluationContractError("completed_action_receipt_required");
    const issues = evaluatorTargetIssues(target, receipt.before);
    if (receipt.after) issues.push(...declaredTransitionIssues(receipt.before, receipt.after,
      ["appViewVariant", "targetGeneration", "surfaceGeneration", "dataGeneration", "presentationRevision", "frameGeneration"]));
    if (issues.length) throw new EvaluationContractError("action_identity_mismatch", issues);
    if (receipt.effect.kind === "refused") throw new DriverCommandRefused(receipt.effect.code, result.requestId);
    const coordinateAction = action.type === "gpuiEvent" && /^(mouse|scroll)/.test(action.event.type);
    if (Object.keys(before).some(key => (key !== "frameGeneration" || coordinateAction) &&
        before[key as keyof AutomationTargetSnapshot] !== receipt.before[key as keyof AutomationTargetSnapshot]))
      throw new EvaluationContractError("action_expectation_rebound");
    if (result.type === "batchResult" && (result.success !== true || result.results?.length !== 1 || result.results[0]?.success !== true))
      throw new EvaluationContractError("single_action_batch_not_completed");
    if (!receipt.dispatchCompleted) throw new EvaluationContractError("completed_action_receipt_required");
    if (receipt.after) this.mounted.set(`${target.id}:${target.generation}`, receipt.after);
    else if (["submissionDelivered", "popupClosed", "rootClosed"].includes(receipt.effect.kind)) this.mounted.delete(`${target.id}:${target.generation}`);
    return result;
  }
  async wait(target: AutomationInstance, condition: Json, timeoutMs = 5000): Promise<Json> {
    const providerCondition = condition?.type === "searchProvider" ? validateSearchProviderCondition(condition) : undefined;
    const streamCondition = condition?.type === "fileSearchStream" ? validateFileSearchStreamCondition(condition) : undefined;
    const previewCondition = condition?.type === "fileSearchPreview" ? validateFileSearchPreviewCondition(condition) : undefined;
    const ownedCondition = providerCondition ?? streamCondition ?? previewCondition;
    const conditionName = providerCondition ? "search_provider" : streamCondition ? "file_search_stream" : "file_search_preview";
    const previous = ownedCondition ? this.mounted.get(`${target.id}:${target.generation}`) : undefined;
    if (ownedCondition && (!previous || evaluatorTargetIssues(target, previous).length))
      throw new EvaluationContractError("target_not_mounted");
    if (ownedCondition && (!Number.isSafeInteger(timeoutMs) || timeoutMs < 0 || timeoutMs > OWNED_EVALUATION_LIMITS.maxLifetimeMs))
      throw new EvaluationContractError(`${conditionName}_condition_invalid`);
    const response = await this.driver.request({ type: "waitFor", target, condition: ownedCondition ?? condition, timeout: timeoutMs, pollInterval: 5 }, { timeoutMs: timeoutMs + 500 });
    if (response.success !== true) throw new EvaluationContractError("wait_condition_not_observed");
    if (ownedCondition) {
      const observed = response.targetIdentity as AutomationTargetSnapshot;
      const issues = evaluatorTargetIssues(target, observed);
      if (response.type !== "waitForResult" || !Number.isSafeInteger(response.elapsed) || response.elapsed < 0 || issues.length ||
          observed.targetGeneration !== previous!.targetGeneration || observed.surfaceGeneration !== previous!.surfaceGeneration ||
          observed.appViewVariant !== previous!.appViewVariant || observed.frameGeneration < previous!.frameGeneration ||
          ((streamCondition || previewCondition) && (observed.dataGeneration < previous!.dataGeneration ||
            observed.presentationRevision < previous!.presentationRevision || observed.themeRevision < previous!.themeRevision)))
        throw new EvaluationContractError(`${conditionName}_wait_identity_mismatch`, issues);
      if (providerCondition) validateSearchProviderObservation(response.searchProvider, providerCondition);
      if (streamCondition) validateFileSearchStreamObservation(response.fileSearchStream, streamCondition);
      if (previewCondition) validateFileSearchPreviewObservation(response.fileSearchPreview, previewCondition);
      this.mounted.set(`${target.id}:${target.generation}`, observed);
    }
    if (condition.type === "completedFrame") {
      const frame = response.frameIdentity as CompletedFrameIdentity;
      const issues = completedFrameIssues(target, frame, this.identity);
      if (issues.length) throw new EvaluationContractError("invalid_completed_frame", issues);
      this.mounted.set(`${target.id}:${target.generation}`, frame.target);
      this.lastFramePhaseDurationsMs = response.phaseDurationsMs ?? {};
    }
    return response;
  }
  async waitForFileSearchStream(target: AutomationInstance,
    condition: Omit<OwnedFileSearchStreamCondition, "type">, timeoutMs = 5000): Promise<Json> {
    return this.wait(target, { ...condition, type: "fileSearchStream" }, timeoutMs);
  }
  async waitForFileSearchPreview(target: AutomationInstance,
    condition: Omit<OwnedFileSearchPreviewCondition, "type">, timeoutMs = 5000): Promise<Json> {
    return this.wait(target, { ...condition, type: "fileSearchPreview" }, timeoutMs);
  }
  async frame(target: AutomationInstance): Promise<CompletedFrameIdentity> {
    if (++this.frames > OWNED_EVALUATION_LIMITS.maxFrames) throw new EvaluationContractError("frame_budget_exhausted");
    const expected = this.mounted.get(`${target.id}:${target.generation}`);
    if (!expected) throw new EvaluationContractError("target_not_mounted");
    // Omitted expected asks the owned runtime to observe its current identity
    // atomically; explicit expected on wait() still enforces snapshot freshness.
    const response = await this.wait(target, { type: "completedFrame", afterFrameGeneration: expected.frameGeneration });
    const frame = response.frameIdentity as CompletedFrameIdentity;
    const issues = completedFrameIssues(target, frame, this.identity);
    if (issues.length || frame.target.frameGeneration <= expected.frameGeneration) throw new EvaluationContractError("invalid_completed_frame", issues);
    this.mounted.set(`${target.id}:${target.generation}`, frame.target);
    return frame;
  }
  async captureFrame(target: AutomationInstance, includeImage = true, scheduled?: ScheduledCapture, frameCursor?: OwnedFrameCursor): Promise<OwnedFrameCapture> {
    const cursor = frameCursor === undefined ? undefined : validateFrameCursor(frameCursor);
    if (++this.frames > OWNED_EVALUATION_LIMITS.maxFrames) throw new EvaluationContractError("frame_budget_exhausted");
    if (includeImage && this.images >= OWNED_EVALUATION_LIMITS.maxRetainedImages) throw new EvaluationContractError("retained_image_budget_exhausted");
    const previous = this.mounted.get(`${target.id}:${target.generation}`);
    if (!previous) throw new EvaluationContractError("target_not_mounted");
    if (scheduled && (evaluatorTargetIssues(target, scheduled.expected).length ||
        !Number.isSafeInteger(scheduled.afterFrameGeneration) || scheduled.afterFrameGeneration < 0 ||
        !Number.isSafeInteger(scheduled.afterNotificationEpoch) || scheduled.afterNotificationEpoch < 0))
      throw new EvaluationContractError("invalid_scheduled_capture_expectation");
    const result = await this.design({ operation: "captureFrame", target, includeImage, ...(scheduled ? { scheduled } : {}), ...(cursor ? { frameCursor: cursor } : {}) });
    if (result.operation !== "captureFrame" || !result.ok) throw new EvaluationContractError("capture_frame_result_required");
    restoreSearchMetadataRefs(result.frameEvidence, cursor !== undefined);
    restoreSearchMetadataRefs(result.state?.frameEvidence, cursor !== undefined);
    const issues = completedFrameIssues(target, result.frame, this.identity, scheduled?.expected);
    if (scheduled && (result.frameEvidence?.mode !== "scheduled" ||
        !Number.isSafeInteger(result.frameEvidence?.notificationEpoch) ||
        result.frameEvidence!.notificationEpoch <= scheduled.afterNotificationEpoch)) issues.push("scheduled_notification_not_observed");
    if (issues.length || result.frame.target.frameGeneration <= (scheduled?.afterFrameGeneration ?? previous.frameGeneration))
      throw new EvaluationContractError("invalid_completed_frame", issues);
    this.validateCapture(target, result.snapshot, result.frame.target, includeImage);
    if (result.snapshot.frameIdentity.nativeWindowId !== result.frame.nativeWindowId)
      throw new EvaluationContractError("invalid_frame_capture", ["frame_native_window_mismatch"]);
    for (const [facet, type] of [["state", "stateResult"], ["elements", "elementsResult"], ["layout", "layoutInfoResult"]] as const) {
      const observation = result[facet];
      if (observation?.requestId !== `${result.snapshot.correlationId}:${facet}`)
        throw new EvaluationContractError("capture_response_correlation_mismatch", [facet]);
      const observed = observation?.targetIdentity as AutomationTargetSnapshot;
      const observationIssues = evaluatorTargetIssues(target, observed);
      if (observation?.type !== type || observationIssues.length ||
          Object.keys(result.frame.target).some(key => observed[key as keyof AutomationTargetSnapshot] !== result.frame.target[key as keyof AutomationTargetSnapshot]))
        throw new EvaluationContractError("capture_observation_identity_mismatch", [facet, ...observationIssues]);
    }
    if (!result.phaseDurationsMs || typeof result.phaseDurationsMs !== "object" || Array.isArray(result.phaseDurationsMs) ||
        Object.values(result.phaseDurationsMs).some(ms => typeof ms !== "number" || !Number.isFinite(ms) || ms < 0))
      throw new EvaluationContractError("invalid_capture_frame_timing");
    if (cursor) {
      restoreCaptureFrameHistories(result, cursor, target, this.identity);
      validateFramePage(result.frameEvidence!, cursor, target, this.identity);
      if (result.frameEvidence!.latestFrameGeneration !== result.frame.target.frameGeneration)
        throw new EvaluationContractError("frame_cursor_response_mismatch");
    }
    if (!cursor && (result.frameHistoryBundle !== undefined || Object.hasOwn(result.frameEvidence ?? {}, "historyScope") ||
        Object.hasOwn(result.state.frameEvidence ?? {}, "historyScope")))
      throw new EvaluationContractError("frame_cursor_response_mismatch");
    if (cursor || scheduled) {
      validateFramePage(result.state.frameEvidence, cursor ?? validateFrameCursor({ traceGeneration: result.frameEvidence?.traceGeneration,
        afterFrameGeneration: scheduled!.afterFrameGeneration }), target, this.identity);
      if (result.state.frameEvidence.latestFrameGeneration !== result.frame.target.frameGeneration)
        throw new EvaluationContractError("frame_cursor_response_mismatch");
    }
    this.mounted.set(`${target.id}:${target.generation}`, result.frame.target);
    this.lastFramePhaseDurationsMs = result.phaseDurationsMs;
    if (includeImage) this.images += 1;
    return result;
  }
  async acknowledgeFrames(target: AutomationInstance, expected: AutomationTargetSnapshot, frameCursor: OwnedFrameCursor): Promise<OwnedFrameAcknowledgement> {
    const cursor = validateFrameCursor(frameCursor);
    if (evaluatorTargetIssues(target, expected).length || cursor.afterFrameGeneration > expected.frameGeneration)
      throw new EvaluationContractError("invalid_frame_acknowledgement_expectation");
    if (!this.mounted.has(`${target.id}:${target.generation}`)) throw new EvaluationContractError("target_not_mounted");
    const result = await this.design({ operation: "acknowledgeFrames", target, expected, cursor });
    if (result.operation !== "acknowledgeFrames" || !result.ok || !isDeepStrictEqual(result.target, target) ||
        !isDeepStrictEqual(result.expected, expected) || !isDeepStrictEqual(result.acknowledgedCursor, cursor) ||
        !Number.isSafeInteger(result.retiredFrames) || result.retiredFrames < 0 ||
        !Number.isSafeInteger(result.retainedFrames) || result.retainedFrames < 1 ||
        !Number.isSafeInteger(result.retainedTraceBytes) || result.retainedTraceBytes < 1)
      throw new EvaluationContractError("frame_acknowledgement_response_mismatch");
    return result;
  }
  async capture(target: AutomationInstance, includeImage = true): Promise<Json> {
    if (includeImage && this.images >= OWNED_EVALUATION_LIMITS.maxRetainedImages) throw new EvaluationContractError("retained_image_budget_exhausted");
    const expected = this.mounted.get(`${target.id}:${target.generation}`);
    if (!expected || expected.frameGeneration <= 0) throw new EvaluationContractError("completed_frame_required");
    // The completed scene is already bounded. Keep its native pixels instead of resampling them.
    const response = await this.driver.request({ type: "captureRenderWindow", request: { target, expected, hiDpi: true, includeImage } });
    this.validateCapture(target, response.snapshot as RenderCapture, expected, includeImage);
    if (includeImage) this.images += 1;
    return response;
  }
  async probePixels(target: AutomationInstance, expected: AutomationTargetSnapshot, probes: readonly PixelProbe[]): Promise<RenderCapture> {
    if (evaluatorTargetIssues(target, expected).length || expected.frameGeneration <= 0 ||
        !probes.length || probes.length > 64 || probes.some(p => !Number.isSafeInteger(p.x) || !Number.isSafeInteger(p.y) || p.x < 0 || p.y < 0))
      throw new EvaluationContractError("invalid_pixel_probe_request");
    const response = await this.driver.request({ type: "captureRenderWindow", request: { target, expected, hiDpi: true, includeImage: false, probes } });
    const snapshot = response.snapshot as RenderCapture;
    this.validateCapture(target, snapshot, expected, false);
    if (snapshot.pixelProbes?.length !== probes.length || snapshot.pixelProbes.some((p, i) =>
        p.x !== probes[i]!.x || p.y !== probes[i]!.y || p.x >= snapshot.capture!.width || p.y >= snapshot.capture!.height ||
        [p.r, p.g, p.b, p.a].some(value => !Number.isSafeInteger(value) || value < 0 || value > 255)))
      throw new EvaluationContractError("invalid_pixel_probe_result");
    return snapshot;
  }
  private validateCapture(target: AutomationInstance, snapshot: RenderCapture, expected: AutomationTargetSnapshot, includeImage: boolean): void {
    if (!snapshot || snapshot.status !== "captured" || snapshot.source !== "gpuiRenderReadback" || snapshot.scope !== "liveAutomationWindowRenderReadback")
      throw new EvaluationContractError("qualified_readback_failed", [snapshot?.status ?? "missing_snapshot"]);
    const issues = completedFrameIssues(target, snapshot.frameIdentity, this.identity, expected);
    if (snapshot.frameIdentity?.target?.frameGeneration !== expected.frameGeneration) issues.push("frame_generation_mismatch");
    if (snapshot.frameIdentity?.target?.appViewVariant !== expected.appViewVariant) issues.push("frame_surface_mismatch");
    const capture = snapshot.capture;
    if (!capture || !Number.isSafeInteger(capture.width) || !Number.isSafeInteger(capture.height) || capture.width <= 0 || capture.height <= 0 ||
        capture.width * capture.height > OWNED_EVALUATION_LIMITS.maxImagePixels) issues.push("invalid_capture_dimensions");
    if (capture?.hiDpi !== true) issues.push("native_resolution_readback_required");
    const data = capture?.pngBase64;
    if (includeImage && (typeof data !== "string" || !data.length || Buffer.byteLength(data, "base64") > OWNED_EVALUATION_LIMITS.maxPngBytes)) issues.push("invalid_capture_bytes");
    if (issues.length) throw new EvaluationContractError("invalid_frame_capture", issues);
  }
  async probeSafety(target: AutomationInstance, probe: NativeSafetyProbe): Promise<NativeSafetyProbeResult> {
    const expected = (await this.inspect(target)).targetIdentity as AutomationTargetSnapshot;
    const result = await this.design({ operation: "probeSafety", target, expected, probe });
    if (result.operation !== "probeSafety" || !result.ok || result.probe !== probe ||
        result.negativeOnly !== true || result.productionEvidence !== false ||
        result.target?.id !== target.id || result.target?.generation !== target.generation)
      throw new EvaluationContractError("native_probe_result_required");
    const issues = evaluatorTargetIssues(target, result.targetIdentity);
    if (issues.length) throw new EvaluationContractError("native_probe_identity_mismatch", issues);
    this.mounted.set(`${target.id}:${target.generation}`, result.targetIdentity);
    return result;
  }
  async applyTheme(expectedRevision: number, edits: readonly LiveThemeEdit[]): Promise<ThemePublication> {
    const result = await this.design({ operation: "applyTheme", expectedRevision, edits: validateThemeEdits(edits) });
    if (result.operation !== "applyTheme" || !result.ok) throw new EvaluationContractError("theme_publication_required");
    return result;
  }
  async revertTheme(expectedRevision: number): Promise<ThemePublication> {
    const result = await this.design({ operation: "revertTheme", expectedRevision });
    if (result.operation !== "revertTheme" || !result.ok) throw new EvaluationContractError("theme_revert_required");
    return result;
  }
  async unmount(target: AutomationInstance, expected?: AutomationTargetSnapshot): Promise<void> {
    if (!this.mounted.has(`${target.id}:${target.generation}`)) throw new EvaluationContractError("target_not_mounted");
    const before = expected ?? (await this.inspect(target)).targetIdentity ?? this.mounted.get(`${target.id}:${target.generation}`)!;
    const issues = evaluatorTargetIssues(target, before);
    if (issues.length) throw new EvaluationContractError("invalid_unmount_expectation", issues);
    const result = await this.design({ operation: "unmount", target, expected: before });
    if (result.operation !== "unmount" || !result.ok || !result.closed || result.target.id !== target.id || result.target.generation !== target.generation)
      throw new EvaluationContractError("unmount_not_observed");
    this.mounted.delete(`${target.id}:${target.generation}`);
  }
  async diagnose() {
    const result = await this.design({ operation: "diagnose" });
    if (result.operation !== "diagnose" || !result.ok) throw new EvaluationContractError("diagnose_result_required");
    this.reconcileTargets(result.targets);
    return result;
  }
  close(): Promise<OwnedCleanup> {
    this.closePromise ??= this.closeOwned();
    return this.closePromise;
  }
  private async closeOwned(): Promise<OwnedCleanup> {
    try {
      if (this.driver.alive && !this.driver.nativeLifecycle) {
        const result = await this.design({ operation: "end" });
        if (result.operation !== "end" || !result.ok || !result.ownedWindowsClosed || result.remainingWindows !== 0)
          throw new EvaluationContractError("native_end_not_closed");
        await this.driver.awaitNativeLifecycle();
      }
    } finally { await this.driver.close(); }
    if (!this.driver.finalization.closed) throw new DriverLifecycleError("INVALID_CLEANUP", this.driver.finalization);
    return this.driver.finalization;
  }
}
