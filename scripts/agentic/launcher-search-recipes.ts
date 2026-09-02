import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { OwnedEvaluationClient, EvaluationContractError, OWNED_SEARCH_CACHE_SOURCES, isOwnedSearchSourceCacheReadiness, type OwnedSearchSourceCacheReadiness, type OwnedAction, type SearchProviderObservation, type SearchProviderRun, type SearchFixtureObservation, type SearchFixturePreparation, type SearchSourcePlan, type ScheduledCapture, type PixelProbeResult, type OwnedFrameCapture, type OwnedCopySinkObservation, type OwnedFrameCursor } from "../devtools/lib/owned-evaluation.ts";
import { DriverCommandRefused, DriverProtocolError, DriverLifecycleError, unknownOwnedCleanup, type Json } from "../devtools/driver.ts";
import type { AutomationInstance, AutomationTargetSnapshot } from "../devtools/lib/target-identity.ts";
import type { ArtifactReference } from "./build-artifact.ts";
import type { OutputClaim, OwnedCleanup } from "./artifact-lifecycle.ts";
import { aggregateCleanup } from "../devtools/lib/story-contract.ts";
import { nativeSafetyProbeAssertions, type RuntimeJourneyReceipt } from "../devtools/design.ts";
import { SEARCH_CASES, SEARCH_PROVIDERS, SEARCH_FIXTURE_ID, searchContractSpec, searchAssertionApplicability, searchScheduleComparisonGroup, partitionSearchSchedules, accountSearchCoverage, type SearchCase, type SearchSchedule, type SearchScheduleResult, type SearchCoverage, type SearchProvider, type SearchTerminalOutcome } from "./launcher-search-contract.ts";
import { NoninteractiveSafetyError } from "../devtools/lib/operator-safety.ts";

const ROOT = resolve(import.meta.dir, "../..");
export interface SearchShardEvidence {
  version: 1; caseSetHash: string; shard: number; scheduleIds: string[];
  results: SearchScheduleResult[]; effects: Json[]; cleanup: OwnedCleanup;
}
export interface SearchShardEvidenceReference { artifactId: string; shard: number; scheduleIds: string[] }
export interface SearchRecipeOptions {
  caseId?: string; shard?: number; retainShard?: (evidence: SearchShardEvidence) => SearchShardEvidenceReference;
}
export interface SearchJourneyReceipt extends RuntimeJourneyReceipt {
  coverage: Omit<SearchCoverage, "results"> & { results: (SearchScheduleResult & { evidenceReference?: { artifactId: string; shard: number; scheduleId: string } })[] };
  caseSetHash: string; shardReferences: SearchShardEvidenceReference[];
}
export interface SearchQueryStamp { lifetime: number; revision: number; scopeRevision: number }
export interface SearchCommittedRow {
  semanticId: string; stableKey: string; contentFingerprint: string; groupedIndex: number;
  selectableOrdinal: number | null; subjectKind: string; selectable: boolean; activatable: boolean;
}
export interface SearchDispatchObservation {
  query: SearchQueryStamp; stableKey: string; contentFingerprint: string;
  status: "dispatchRequested" | "refused" | "completed" | "pendingConfirmation"; reason: string | null;
}
export interface SearchObservation {
  version: 1; query: SearchQueryStamp; computedQuery: SearchQueryStamp; pending: boolean;
  resultRevision: number; selectionRevision: number; viewportRevision: number;
  selectionIntent: { kind: "automaticTop" | "automaticAnchor" | "explicitAnchor"; semanticId?: string | null };
  selectionArmed: boolean;
  viewportIntent: "followSelection" | "userControlled"; reconciliationReason: string | null;
  selectedSemanticId: string | null; selectedOrdinal: number | null;
  rawInput: string; computedInput: string; selectedIndex: number | null;
  committedRows: SearchCommittedRow[]; publication: Json | null;
  providers: Json; publicationError: unknown; preflight?: Json;
  sourceCacheReadiness?: readonly OwnedSearchSourceCacheReadiness[];
  dispatch?: SearchDispatchObservation | null;
}
interface SearchSnapshot { state: Json; elements: Json; search: SearchObservation; frame?: OwnedFrameCapture }
export interface SearchRuntime { client: OwnedEvaluationClient; target: AutomationInstance; safety: Json }
export interface SearchOrderReceipt { key: string; order: string; fingerprint: string; expectedOrders: readonly string[] }
interface SearchCaseEvidence {
  phases: Json[]; actions: Json[]; orderComparisons: SearchOrderReceipt[];
  safetyReference: string; resourceUse?: Json; failure?: Json; counterexample?: Json;
  framePool: SearchFramePool;
}
const acknowledgedSearchFrames = new WeakMap<SearchRuntime, { client: OwnedEvaluationClient; target: AutomationInstance; cursor: OwnedFrameCursor }>();
function acknowledgedFrameCursor(runtime: SearchRuntime): OwnedFrameCursor | undefined {
  const acknowledged = acknowledgedSearchFrames.get(runtime);
  if (acknowledged && (acknowledged.client !== runtime.client || acknowledged.target.id !== runtime.target.id || acknowledged.target.generation !== runtime.target.generation))
    throw new EvaluationContractError("frame_cursor_binding_mismatch");
  return acknowledged?.cursor;
}
export async function retireSearchRuntime(runtime: SearchRuntime, effects: Json[]): Promise<void> {
  const cursor = acknowledgedFrameCursor(runtime);
  const state = await runtime.client.inspect(runtime.target, cursor);
  const observed = { id: "search-runtime-retirement", requestedCursor: cursor ?? null,
    target: identity(state), frameEvidence: state.frameEvidence, unmounted: false };
  effects.push(observed);
  await runtime.client.unmount(runtime.target, observed.target);
  observed.unmounted = true;
  acknowledgedSearchFrames.delete(runtime);
}
const TERMINAL = ["completed", "failed", "unavailable", "stale-discarded", "cancelled"];
const SOURCE_TERMINAL_STATES: Readonly<Record<string, readonly [string, string]>> = { success: ["completed", "success"], empty: ["completed", "empty"], failed: ["failed", "error"], unavailable: ["unavailable", "unavailable"], disconnected: ["failed", "disconnected"] };
function digest(value: unknown): string { return createHash("sha256").update(JSON.stringify(value) ?? "\u0000undefined").digest("hex"); }
function identity(state: Json): AutomationTargetSnapshot {
  if (!state.targetIdentity) throw new EvaluationContractError("missing-capability:target-identity");
  return state.targetIdentity;
}
function requireIssues(issues: readonly string[]): void { if (issues.length) throw new EvaluationContractError(issues[0]!, issues.slice(1)); }
function sameQuery(a: SearchQueryStamp, b: SearchQueryStamp): boolean { return a.lifetime === b.lifetime && a.revision === b.revision && a.scopeRevision === b.scopeRevision; }
function rowId(row: Json): string { return row.semanticId ?? row.id; }
function collectorRows(elements: Json): Json[] {
  if (!Array.isArray(elements.elements)) throw new EvaluationContractError("missing-capability:semantic-rows");
  return elements.elements.filter((row: Json) => row.role === "row" || row.elementType === "choice");
}
function selected(search: SearchObservation): SearchCommittedRow | undefined {
  return search.committedRows.find(row => row.semanticId === search.selectedSemanticId);
}
export function dispatchBindingIssues(search: SearchObservation): string[] {
  const row = selected(search); const dispatch = search.dispatch;
  if (!row?.activatable) return dispatch === null ? [] : ["dispatch-without-current-subject"];
  if (!dispatch) return ["missing-current-subject-dispatch"];
  const issues: string[] = [];
  if (search.pending || !sameQuery(dispatch.query, search.query) || dispatch.stableKey !== row.stableKey || dispatch.contentFingerprint !== row.contentFingerprint)
    issues.push("stale-dispatch-subject");
  if (!["refused", "completed", "pendingConfirmation"].includes(dispatch.status)) issues.push("dispatch-not-terminal");
  return issues;
}
export function copySinkIssues(before: OwnedCopySinkObservation | null | undefined, after: OwnedCopySinkObservation | null | undefined, text: string): string[] {
  const revision = before === null ? 0 : before?.receipt?.revision;
  const issues: string[] = [];
  if (!Number.isSafeInteger(revision) || revision! < 0) issues.push("missing-prior-copy-sink-revision");
  if (!after || after.receipt?.destination !== "ownedProcessLocal") return [...issues, "missing-owned-copy-completion"];
  if (!Number.isSafeInteger(after.receipt.revision) || after.receipt.revision !== revision! + 1) issues.push("owned-copy-count-did-not-advance-once");
  if (after.text !== text || after.receipt.byteLength !== Buffer.byteLength(text) || after.receipt.sha256 !== createHash("sha256").update(text).digest("hex"))
    issues.push("owned-copy-value-disagreement");
  return issues;
}
function admissionReady(run: SearchProviderRun): boolean {
  return (run.kind === "worker" && run.state === "held") || (run.kind === "sourceChange" && run.state === "awaiting-admission");
}
export function currentSourceResolution(search: SearchObservation, observation: SearchProviderObservation, source: SearchProvider, afterRunId = 0): { status: "admitted" | "settled"; owner: Json; run: SearchProviderRun } | undefined {
  const ownership = search.providers;
  if (search.pending || !sameQuery(search.query, search.computedQuery) || ownership?.version !== 1 ||
      !Array.isArray(ownership.runs) || !Array.isArray(ownership.desired)) return undefined;
  const owner = ownership.runs.find((run: Json) => run.source === source);
  if (!owner || (owner.queryBound !== false && (!owner.consumer || !sameQuery(owner.consumer, search.query))) ||
      ownership.desired.some((desired: Json) => desired.source === source && (owner.queryBound === false || desired.query && sameQuery(desired.query, search.query)))) return undefined;
  const run = observation.runs.findLast(run => run.id > afterRunId && run.source === source && run.kind !== "sourceChange" && run.generation === owner.generation);
  if (!run) return undefined;
  if (owner.terminal === null && admissionReady(run)) return { status: "admitted", owner, run };
  const terminal = SOURCE_TERMINAL_STATES[owner.terminal];
  return terminal && run.state === terminal[0] && run.outcome === terminal[1] ? { status: "settled", owner, run } : undefined;
}
export function currentSourceCache(search: SearchObservation, source: SearchProvider, afterRunId = 0): OwnedSearchSourceCacheReadiness | undefined {
  if (afterRunId !== 0 || search.pending || !sameQuery(search.query, search.computedQuery) || !Array.isArray(search.sourceCacheReadiness)) return undefined;
  let current: OwnedSearchSourceCacheReadiness | undefined;
  for (const cache of search.sourceCacheReadiness) if (cache?.source === source) {
    if (current || !isOwnedSearchSourceCacheReadiness(cache) || !sameQuery(cache.query, search.query)) return undefined;
    current = cache;
  }
  return current;
}
export function providerObservationIssues(value: SearchProviderObservation | undefined): string[] {
  if (!value || value.version !== 1 || !Array.isArray(value.runs) || !Array.isArray(value.pendingRunIds)) return ["missing-capability:provider-observation"];
  const issues: string[] = [];
  if (!Number.isSafeInteger(value.displayUnixMs)) issues.push("missing-capability:provider-display-clock");
  if (value.retired || value.overflow) issues.push("provider-observation-retired-or-overflowed");
  if (new Set(value.runs.map(run => run.id)).size !== value.runs.length) issues.push("duplicate-provider-run");
  for (const run of value.runs) if (!Number.isSafeInteger(run.id) || run.id <= 0 || !SEARCH_PROVIDERS.includes(run.source as SearchProvider) || typeof run.query !== "string" ||
      !Number.isSafeInteger(run.generation) || run.generation < 0 || !["worker", "sourceChange", "synchronousRead"].includes(run.kind) ||
      !["awaiting-admission", "reading", "held", "released", "delivered", ...TERMINAL].includes(run.state) ||
      (run.kind === "sourceChange" ? run.publicationPolicy !== null || run.outcome != null || run.resultCount != null :
        !["visible", "cache-only", "visible-synchronous"].includes(run.publicationPolicy ?? ""))) issues.push("invalid-provider-run");
  for (const id of value.pendingRunIds) if (!value.runs.some(run => run.id === id)) issues.push("unknown-pending-run");
  return issues;
}
export function sourcePlanIssues(plans: readonly SearchSourcePlan[] | undefined): string[] {
  if (!Array.isArray(plans) || plans.length !== SEARCH_PROVIDERS.length || new Set(plans.map(plan => plan.source)).size !== SEARCH_PROVIDERS.length) return ["missing-capability:complete-source-plans"];
  return plans.some(plan => !SEARCH_PROVIDERS.includes(plan.source as SearchProvider) || typeof plan.input !== "string" || plan.input.length > 4096 ||
    !["root", "directory", "spine"].includes(plan.scope) || !["query-bound", "catalogue", "synchronous"].includes(plan.workKind) ||
    Object.keys(plan).some(key => !["source", "input", "scope", "workKind"].includes(key))) ? ["invalid-source-plan"] : [];
}
export function heldProviderIssues(before: SearchProviderRun, after: SearchProviderRun | undefined, beforeFingerprint: string, afterFingerprint: string): string[] {
  const issues: string[] = [];
  if (!after || after.id !== before.id || after.source !== before.source || after.query !== before.query || after.generation !== before.generation) issues.push("held-run-identity-changed");
  if (before.state !== "held" || after?.state !== "held") issues.push("held-completion-escaped");
  if (beforeFingerprint !== afterFingerprint) issues.push("held-provider-published");
  return issues;
}
export function searchObservationIssues(search: SearchObservation | undefined, elements?: Json): string[] {
  if (!search || search.version !== 1 || !Array.isArray(search.committedRows) || !search.query || !search.computedQuery) return ["missing-capability:search-observation"];
  const issues: string[] = [];
  for (const stamp of [search.query, search.computedQuery]) if ([stamp.lifetime, stamp.revision, stamp.scopeRevision].some(value => !Number.isSafeInteger(value) || value < 0)) issues.push("invalid-query-stamp");
  if (search.pending !== !sameQuery(search.query, search.computedQuery)) issues.push("query-current-stamp-disagreement");
  if (typeof search.selectionArmed !== "boolean") issues.push("missing-capability:selection-arming");
  if ([search.resultRevision, search.selectionRevision, search.viewportRevision].some(value => !Number.isSafeInteger(value) || value < 0)) issues.push("invalid-owner-revision");
  const selectable = search.committedRows.filter(row => row.selectable);
  if (new Set(search.committedRows.map(row => row.semanticId)).size !== search.committedRows.length ||
      search.committedRows.some(row => !/^main-list-row:v2:[a-f0-9]{64}$/.test(row.semanticId) || typeof row.stableKey !== "string" || !row.stableKey || typeof row.contentFingerprint !== "string" || !/^[a-f0-9]{64}$/.test(row.contentFingerprint) ||
        row.semanticId !== `main-list-row:v2:${createHash("sha256").update(row.stableKey).digest("hex")}`)) issues.push("invalid-canonical-row-identity");
  if (selectable.some((row, index) => row.selectableOrdinal !== index) || search.committedRows.some(row => !row.selectable && row.selectableOrdinal !== null)) issues.push("eligibility-ordinal-disagreement");
  const current = selected(search);
  if (search.selectedSemanticId === null ? search.selectedOrdinal !== null : !current?.selectable || current.selectableOrdinal !== search.selectedOrdinal) issues.push("effective-selection-disagreement");
  if (!search.selectionArmed && current) issues.push("unarmed-effective-selection");
  if (!["automaticTop", "automaticAnchor", "explicitAnchor"].includes(search.selectionIntent?.kind) ||
      (search.selectionArmed && search.selectionIntent.kind !== "automaticTop" && (search.selectionIntent.semanticId ?? null) !== (current?.semanticId ?? null))) issues.push("explicit-anchor-disagreement");
  if (!["followSelection", "userControlled"].includes(search.viewportIntent)) issues.push("invalid-viewport-intent");
  if (elements) {
    const rows = collectorRows(elements); const marked = rows.filter(row => row.selected);
    if (marked.length !== (current ? 1 : 0) || (current && rowId(marked[0]!) !== current.semanticId)) issues.push("semantic-selection-disagreement");
    for (const row of rows) {
      const canonical = search.committedRows.find(item => item.semanticId === rowId(row));
      if (!canonical || canonical.selectable !== (row.selectable !== false) || (canonical.selectable && canonical.selectableOrdinal !== row.index)) issues.push("collector-projection-disagreement");
    }
  }
  return [...new Set(issues)];
}
export function naturalEvidenceIssues(capture: OwnedFrameCapture, expected: ScheduledCapture, retainedFrames?: SearchFramePool): string[] {
  const issues: string[] = []; const evidence = capture.frameEvidence;
  if (!evidence?.scheduledCapability) issues.push("missing-capability:scheduled-frame");
  if (evidence?.mode !== "scheduled" || capture.frame.target.frameGeneration <= expected.afterFrameGeneration ||
      !Number.isSafeInteger(evidence?.notificationEpoch) || evidence!.notificationEpoch <= expected.afterNotificationEpoch) issues.push("missing-scheduled-frame");
  if (evidence?.traceOverflow !== false) issues.push("frame-trace-overflow-or-missing");
  const currentFrame = canonicalFrameJson(capture.frame);
  const matchesCurrent = (stamp: Json) => Number.isSafeInteger(stamp?.traceGeneration) && stamp.traceGeneration > 0 &&
    stamp.traceGeneration === evidence?.traceGeneration && stamp.frame && canonicalFrameJson(stamp.frame) === currentFrame;
  if (!Array.isArray(evidence?.completedFrames) || (!evidence.completedFrames.some(matchesCurrent) &&
      !retainedFrames?.frames.some((_entry, index) => {
        try { return matchesCurrent(reconstructSearchFrameFacts(retainedFrames, index)); }
        catch { return false; }
      }))) issues.push("completed-stamp-missing");
  if (capture.snapshot.status !== "captured" || !capture.snapshot.capture?.width || !capture.snapshot.capture?.height) issues.push("qualified-readback-missing");
  if (Object.keys(expected.expected).some(key => key !== "frameGeneration" && capture.frame.target[key as keyof AutomationTargetSnapshot] !== expected.expected[key as keyof AutomationTargetSnapshot])) issues.push("frame-state-stale");
  if (evidence?.nativeWindow?.visible !== false || evidence?.nativeWindowActive !== false) issues.push("owned-hidden-window-not-proven");
  return issues;
}
export function selectionPixelIssues(samples: readonly PixelProbeResult[] | undefined, color: number): string[] {
  if (!samples?.length || !Number.isSafeInteger(color) || color < 0 || color > 0xffffff) return ["missing-capability:selection-pixels"];
  const expected = [color >>> 16 & 255, color >>> 8 & 255, color & 255];
  return samples.every(pixel => pixel.a === 255 && [pixel.r, pixel.g, pixel.b].every((v, i) => Math.abs(v - expected[i]!) <= 2)) ? [] : ["selected-marker-pixels-disagree"];
}
function rankingFacts(search: SearchObservation): Json {
  return { rawInput: search.rawInput, computedInput: search.computedInput, rows: search.committedRows.map(row => ({ semanticId: row.semanticId, contentFingerprint: row.contentFingerprint,
    selectableOrdinal: row.selectableOrdinal, subjectKind: row.subjectKind, selectable: row.selectable, activatable: row.activatable })) };
}
export function rankingFingerprint(search: SearchObservation): string {
  return digest(rankingFacts(search));
}
// Arrival order may change display order, but not the available result set.
function candidateFingerprint(search: SearchObservation): string {
  return digest({ rawInput: search.rawInput, computedInput: search.computedInput,
    rows: search.committedRows.map(({ semanticId, contentFingerprint, subjectKind, selectable, activatable }) =>
      ({ semanticId, contentFingerprint, subjectKind, selectable, activatable })).sort((a, b) => a.semanticId.localeCompare(b.semanticId)) });
}
function selectionFingerprint(search: SearchObservation): string {
  return digest({ rows: rankingFingerprint(search), selected: search.selectedSemanticId, intent: search.selectionIntent, armed: search.selectionArmed,
    resultRevision: search.resultRevision, selectionRevision: search.selectionRevision, viewportRevision: search.viewportRevision, viewportIntent: search.viewportIntent });
}
export function paintBindingIssues(search: SearchObservation, bindings: readonly Json[] | undefined): string[] {
  if (!Array.isArray(bindings)) return ["missing-capability:paint-bindings"];
  const issues: string[] = []; const current = selected(search);
  const painted = bindings.filter(binding => binding.kind === "mainSearchRow");
  for (const binding of painted) {
    const row = search.committedRows.find(row => row.semanticId === binding.id);
    if (!row || binding.metadata?.stableKey !== row.stableKey || binding.metadata?.contentFingerprint !== row.contentFingerprint ||
        binding.metadata?.selected !== (row.semanticId === search.selectedSemanticId) || binding.metadata?.activatable !== row.activatable) issues.push("paint-row-projection-disagreement");
  }
  const preview = bindings.filter(binding => binding.kind === "mainSearchPreview");
  for (const binding of preview) if (!current || binding.metadata?.stableKey !== current.stableKey || binding.metadata?.contentFingerprint !== current.contentFingerprint ||
      !sameQuery(binding.metadata?.query ?? {}, search.computedQuery)) issues.push("paint-preview-subject-disagreement");
  if (!current && painted.some(binding => binding.metadata?.selected)) issues.push("empty-selection-painted");
  if (painted.filter(binding => binding.metadata?.selected).length > 1 || preview.length > 1) issues.push("multiple-painted-selected-subjects");
  return [...new Set(issues)];
}
function recipeFailure(error: unknown): Json {
  // Launch wrappers carry cleanup; their cause carries the operational failure.
  if (error instanceof DriverLifecycleError && error.cause !== undefined) error = error.cause;
  const name = error instanceof NoninteractiveSafetyError ? "NoninteractiveSafetyError" : error instanceof DriverProtocolError ? "DriverProtocolError" :
    error instanceof Error && ["TypeError", "ReferenceError", "RangeError", "SyntaxError"].includes(error.name) ? error.name : "Error";
  const code = error instanceof EvaluationContractError || error instanceof DriverProtocolError ? error.code :
    error instanceof NoninteractiveSafetyError ? "noninteractive-safety-refused" : `search-recipe-unexpected-${name}`;
  return { code, name, diagnostic: { message: error instanceof Error ? error.message.slice(0, 256) : typeof error,
    stack: error instanceof Error ? error.stack?.slice(0, 512) : undefined } };
}

function frameFacts(frame: Json): Json {
  const search = frame.search;
  return { frame: frame.frame, mode: frame.mode, cause: frame.cause, traceGeneration: frame.traceGeneration,
    notificationEpoch: frame.notificationEpoch, invalidationEpoch: frame.invalidationEpoch, notificationCause: frame.notificationCause,
    localInputFocused: frame.localInputFocused, nativeWindow: frame.nativeWindow, nativeWindowActive: frame.nativeWindowActive,
    search: search ? { query: search.query, computedQuery: search.computedQuery, pending: search.pending,
      resultRevision: search.resultRevision, selectionRevision: search.selectionRevision, viewportRevision: search.viewportRevision,
      selectionIntent: search.selectionIntent, selectionArmed: search.selectionArmed, viewportIntent: search.viewportIntent, selectedSemanticId: search.selectedSemanticId,
      selectedOrdinal: search.selectedOrdinal, publication: search.publication, dispatch: search.dispatch,
      rankingFingerprint: rankingFingerprint(search), providersFingerprint: digest(search.providers) } : null,
    fileSearch: frame.fileSearch, paintFailures: frame.paintFailures, pixelEvidence: frame.pixelEvidence,
    pixelEvidenceComplete: frame.pixelEvidenceComplete, pendingResources: frame.pendingResources, failedResources: frame.failedResources,
    paintBindings: frame.paintBindings?.map((binding: Json) => binding.kind === "mainSearch" ?
      { ...binding, metadata: undefined } : binding) };
}

export interface SearchFrameReference { frameRef: number }
type SearchFrameFact = "search" | "pixelEvidence" | "nativeWindow";
export interface SearchFramePool {
  version: 1;
  frames: { facts: Json; paintBindingRefs?: number[]; factRefs?: Partial<Record<SearchFrameFact, number>>; ownerRef?: number }[];
  paintBindings: { binding: Json; metadataRef?: number }[];
  metadata: Json[];
}
function canonicalFrameJson(value: Json): string {
  const encoded = JSON.stringify(value, (_key, entry) => entry && typeof entry === "object" && !Array.isArray(entry) ?
    Object.fromEntries(Object.keys(entry).sort().map(key => [key, entry[key]])) : entry);
  if (encoded === undefined) throw new EvaluationContractError("invalid-frame-evidence-json");
  return encoded;
}
function framePoolIdentity(facts: Json): string {
  const owner = facts?.frame; const target = owner?.target;
  if (typeof owner?.processInstanceId !== "string" || typeof owner.sessionGeneration !== "string" || typeof target?.windowId !== "string" ||
      !Number.isSafeInteger(target.windowGeneration) || target.windowGeneration < 1 || !Number.isSafeInteger(target.frameGeneration) || target.frameGeneration < 1)
    throw new EvaluationContractError("invalid-pooled-frame-identity");
  return JSON.stringify([owner.processInstanceId, owner.sessionGeneration, target.windowId, target.windowGeneration, target.frameGeneration]);
}
function poolIndex(value: unknown, length: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value >= length) throw new EvaluationContractError("dangling-frame-evidence-reference");
  return value;
}
const FRAME_OWNER_FIELDS = ["binarySha256", "manifestSha256", "pid", "processInstanceId", "processStartTime", "sessionGeneration", "requestedTarget"] as const;
const FRAME_INLINE_FIELDS = ["processInstanceId", "sessionGeneration", "requestedTarget", "target"] as const;
function isFrameOwnerMetadata(value: Json): boolean {
  return !!value && typeof value === "object" && !Array.isArray(value) &&
    FRAME_OWNER_FIELDS.every(key => Object.hasOwn(value, key) && value[key] !== undefined) &&
    Object.keys(value).every(key => key === "nativeWindowId" || FRAME_OWNER_FIELDS.includes(key as typeof FRAME_OWNER_FIELDS[number])) &&
    typeof value.processInstanceId === "string" && typeof value.sessionGeneration === "string" &&
    value.requestedTarget?.type === "instance" && typeof value.requestedTarget.id === "string" &&
    Number.isSafeInteger(value.requestedTarget.generation) && value.requestedTarget.generation > 0;
}
function isFrameFact(field: string, value: Json): boolean {
  if (field === "pixelEvidence") return Array.isArray(value) && value.every(sample => sample && typeof sample === "object" &&
    typeof sample.kind === "string" && sample.probe && ["x", "y", "r", "g", "b", "a"].every(key => typeof sample.probe[key] === "number" && Number.isFinite(sample.probe[key])));
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  if (field === "nativeWindow") return typeof value.visible === "boolean" &&
    Object.keys(value).every(key => ["visible", "appActive", "key", "miniaturized", "nativeWindowId"].includes(key));
  return field === "search" && Object.hasOwn(value, "selectedSemanticId") && value.query && typeof value.query === "object" &&
    !Array.isArray(value.query) && ["lifetime", "revision", "scopeRevision"].every(key => Number.isSafeInteger(value.query[key]) && value.query[key] >= 0);
}
/** Resolves only this frame's facts; shared metadata stays immutable and paint bindings remain indexed. */
export function reconstructSearchFrameFacts(pool: SearchFramePool, index: number): Json {
  const entry = pool.frames[poolIndex(index, pool.frames.length)]!;
  if (!entry?.facts || typeof entry.facts !== "object" || Array.isArray(entry.facts) || Object.hasOwn(entry.facts, "paintBindings") ||
      Object.keys(entry).some(key => !["facts", "paintBindingRefs", "factRefs", "ownerRef"].includes(key)))
    throw new EvaluationContractError("conflicting-frame-evidence-reference");
  const facts = { ...entry.facts };
  if (Object.hasOwn(entry, "ownerRef")) {
    const owner = pool.metadata[poolIndex(entry.ownerRef, pool.metadata.length)]!; const inline = facts.frame;
    if (!isFrameOwnerMetadata(owner) || !inline || typeof inline !== "object" || Array.isArray(inline) ||
        Object.keys(inline).length !== FRAME_INLINE_FIELDS.length || Object.keys(inline).some(key => !FRAME_INLINE_FIELDS.includes(key as typeof FRAME_INLINE_FIELDS[number])) ||
        inline.processInstanceId !== owner.processInstanceId || inline.sessionGeneration !== owner.sessionGeneration ||
        canonicalFrameJson(inline.requestedTarget) !== canonicalFrameJson(owner.requestedTarget))
      throw new EvaluationContractError("invalid-frame-owner-reference");
    facts.frame = { ...owner, ...inline };
  }
  if (Object.hasOwn(entry, "factRefs")) {
    if (!entry.factRefs || typeof entry.factRefs !== "object" || Array.isArray(entry.factRefs))
      throw new EvaluationContractError("invalid-frame-fact-reference");
    for (const [field, ref] of Object.entries(entry.factRefs)) {
      if (!["search", "pixelEvidence", "nativeWindow"].includes(field) || Object.hasOwn(facts, field))
        throw new EvaluationContractError("conflicting-frame-evidence-reference");
      const value = pool.metadata[poolIndex(ref, pool.metadata.length)]!;
      if (!isFrameFact(field, value)) throw new EvaluationContractError("invalid-frame-fact-reference");
      facts[field] = value;
    }
  }
  return facts;
}
export function validateSearchFramePool(pool: SearchFramePool, references: readonly SearchFrameReference[] = []): void {
  if (pool?.version !== 1 || !Array.isArray(pool.frames) || !Array.isArray(pool.paintBindings) || !Array.isArray(pool.metadata))
    throw new EvaluationContractError("invalid-frame-evidence-pool");
  const identities = new Set<string>();
  for (let index = 0; index < pool.frames.length; index++) {
    const entry = pool.frames[index]!;
    const identity = framePoolIdentity(reconstructSearchFrameFacts(pool, index));
    if (identities.has(identity)) throw new EvaluationContractError("conflicting-frame-evidence-reference");
    identities.add(identity);
    if (Object.hasOwn(entry, "paintBindingRefs")) {
      if (!Array.isArray(entry.paintBindingRefs)) throw new EvaluationContractError("invalid-frame-evidence-pool");
      for (const ref of entry.paintBindingRefs) poolIndex(ref, pool.paintBindings.length);
    }
  }
  for (const entry of pool.paintBindings) {
    if (!entry?.binding || typeof entry.binding !== "object" || Array.isArray(entry.binding) || Object.hasOwn(entry.binding, "metadata") ||
        Object.keys(entry).some(key => !["binding", "metadataRef"].includes(key))) throw new EvaluationContractError("conflicting-frame-evidence-reference");
    if (Object.hasOwn(entry, "metadataRef")) poolIndex(entry.metadataRef, pool.metadata.length);
  }
  for (const reference of references) {
    if (!reference || Object.keys(reference).length !== 1 || !Object.hasOwn(reference, "frameRef")) throw new EvaluationContractError("invalid-frame-evidence-reference");
    poolIndex(reference.frameRef, pool.frames.length);
  }
}
export function reconstructSearchFrame(pool: SearchFramePool, reference: SearchFrameReference): Json {
  validateSearchFramePool(pool, [reference]);
  const entry = pool.frames[reference.frameRef]!;
  const facts = reconstructSearchFrameFacts(pool, reference.frameRef);
  if (entry.paintBindingRefs) facts.paintBindings = entry.paintBindingRefs.map(ref => {
    const item = pool.paintBindings[ref]!;
    return Object.hasOwn(item, "metadataRef") ? { ...item.binding, metadata: pool.metadata[item.metadataRef!] } : { ...item.binding };
  });
  return facts;
}
function searchObservationOrigin(phases: readonly Json[], index: number): Json | undefined {
  if (!Array.isArray(phases) || !Number.isSafeInteger(index) || index < 0 || index >= phases.length ||
      !phases[index] || typeof phases[index] !== "object" || Array.isArray(phases[index]))
    throw new EvaluationContractError("invalid-search-observation-phase");
  const phase = phases[index]!;
  if (!Object.hasOwn(phase, "observationRef")) return undefined;
  if (Object.keys(phase).length !== 6 || Object.keys(phase).some(key => !["id", "observationRef", "reusedObservation", "providerRuns", "providerRunsAreChanges", "completedFrames"].includes(key)) ||
      typeof phase.id !== "string" || !phase.id || phase.reusedObservation !== true || phase.providerRunsAreChanges !== true ||
      !Array.isArray(phase.providerRuns) || phase.providerRuns.length !== 0 || !Array.isArray(phase.completedFrames) || phase.completedFrames.length !== 0)
    throw new EvaluationContractError("invalid-search-observation-reference");
  if (!Number.isSafeInteger(phase.observationRef) || phase.observationRef < 0 || phase.observationRef >= index)
    throw new EvaluationContractError("dangling-search-observation-reference");
  const origin = phases[phase.observationRef];
  if (!origin || typeof origin !== "object" || Array.isArray(origin) || Object.hasOwn(origin, "observationRef") ||
      typeof origin.id !== "string" || !origin.id || typeof origin.reusedObservation !== "boolean" ||
      ![origin.targetIdentity, origin.query, origin.computedQuery].every(value => value && typeof value === "object" && !Array.isArray(value)) ||
      !Number.isSafeInteger(origin.providerRunCount) || origin.providerRunCount < 0 ||
      !Array.isArray(origin.providerRuns) || typeof origin.providerRunsAreChanges !== "boolean" || !Array.isArray(origin.completedFrames) ||
      !["pending", "resultRevision", "selectionRevision", "viewportRevision", "selectedSemanticId", "selectedOrdinal", "selectionIntent", "selectionArmed", "viewportIntent",
        "reconciliationReason", "publication", "rankingFingerprint", "selectionFingerprint", "providerRunsHash", "traceGeneration"].every(key => Object.hasOwn(origin, key)))
    throw new EvaluationContractError("invalid-search-observation-origin");
  return origin;
}
export function validateSearchObservationPhases(phases: readonly Json[]): void {
  if (!Array.isArray(phases)) throw new EvaluationContractError("invalid-search-observation-phases");
  for (let index = 0; index < phases.length; index++) searchObservationOrigin(phases, index);
}
export function reconstructSearchObservationPhase(phases: readonly Json[], index: number): Json {
  const origin = searchObservationOrigin(phases, index);
  const phase = { ...origin, ...phases[index] };
  delete phase.observationRef;
  return phase;
}
/** The caller validates the frame pool once; each compact pixel record resolves in constant time. */
export function reconstructSearchCapturePixels(pool: SearchFramePool, phase: Json): Json | undefined {
  const pixels = phase.pixels;
  if (!pixels || typeof pixels !== "object" || Array.isArray(pixels) || !Object.hasOwn(pixels, "nativeSamplesFrame")) return pixels;
  const reference = pixels.nativeSamplesFrame;
  if (Object.keys(pixels).some(key => !["nativeSamplesFrame", "sampled"].includes(key)) ||
      !reference || typeof reference !== "object" || Array.isArray(reference) || Object.keys(reference).length !== 1 ||
      !Number.isSafeInteger(reference.frameRef) || reference.frameRef < 0 || reference.frameRef >= pool.frames.length ||
      !phase.frameEvidence || Object.keys(phase.frameEvidence).length !== 1 || phase.frameEvidence.frameRef !== reference.frameRef ||
      (Object.hasOwn(pixels, "sampled") && !Array.isArray(pixels.sampled)))
    throw new EvaluationContractError("invalid-search-pixel-reference");
  const frame = reconstructSearchFrameFacts(pool, reference.frameRef);
  if (!Array.isArray(frame.pixelEvidence)) throw new EvaluationContractError("invalid-search-pixel-origin");
  return { frameGeneration: frame.frame.target.frameGeneration, nativeSamples: frame.pixelEvidence,
    captureHash: phase.capture?.sha256, ...(Object.hasOwn(pixels, "sampled") ? { sampled: pixels.sampled } : {}) };
}
/** Numeric wire identities survive privacy rewriting; content hashes are internal only. */
export class SearchFrameStore {
  readonly pool: SearchFramePool = { version: 1, frames: [], paintBindings: [], metadata: [] };
  private readonly frames = new Map<string, { index: number; hash: string }>();
  private readonly bindings = new Map<string, number>();
  private readonly metadata = new Map<string, number>();
  private intern(value: Json, table: Json[], index: Map<string, number>): number {
    const encoded = canonicalFrameJson(value); const previous = index.get(encoded);
    if (previous !== undefined) return previous;
    const id = table.length; table.push(JSON.parse(encoded)); index.set(encoded, id); return id;
  }
  private retain(facts: Json): SearchFrameReference {
    const identity = framePoolIdentity(facts); const hash = createHash("sha256").update(canonicalFrameJson(facts)).digest("hex");
    const previous = this.frames.get(identity);
    if (previous) {
      if (previous.hash !== hash) throw new EvaluationContractError("conflicting-frame-evidence-reference");
      return { frameRef: previous.index };
    }
    const { paintBindings, ...rest } = facts;
    if (paintBindings !== undefined && !Array.isArray(paintBindings)) throw new EvaluationContractError("invalid-frame-paint-bindings");
    const refs = paintBindings?.map((item: Json) => {
      const { metadata, ...binding } = item;
      const entry = metadata === undefined ? { binding } : { binding, metadataRef: this.intern(metadata, this.pool.metadata, this.metadata) };
      return this.intern(entry, this.pool.paintBindings, this.bindings);
    });
    const factRefs: Partial<Record<SearchFrameFact, number>> = {};
    for (const field of ["search", "pixelEvidence", "nativeWindow"] as const) if (isFrameFact(field, rest[field])) {
      factRefs[field] = this.intern(rest[field], this.pool.metadata, this.metadata); delete rest[field];
    }
    const { target, ...owner } = rest.frame;
    const ownerRef = isFrameOwnerMetadata(owner) ? this.intern(owner, this.pool.metadata, this.metadata) : undefined;
    if (ownerRef !== undefined) rest.frame = { processInstanceId: owner.processInstanceId, sessionGeneration: owner.sessionGeneration, requestedTarget: owner.requestedTarget, target };
    const entry = { facts: JSON.parse(canonicalFrameJson(rest)), ...(refs === undefined ? {} : { paintBindingRefs: refs }),
      ...(Object.keys(factRefs).length ? { factRefs } : {}), ...(ownerRef === undefined ? {} : { ownerRef }) };
    const id = this.pool.frames.length; this.pool.frames.push(entry); this.frames.set(identity, { index: id, hash });
    return { frameRef: id };
  }
  retainWithin<T>(frames: readonly Json[], document: (refs: SearchFrameReference[]) => T, maximumBytes: number): T {
    const checkpoint = [this.pool.frames.length, this.pool.paintBindings.length, this.pool.metadata.length] as const;
    try {
      const result = document(frames.map(frame => this.retain(frame)));
      if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 0 || Buffer.byteLength(JSON.stringify(result)) > maximumBytes)
        throw new EvaluationContractError("search-evidence-byte-bound");
      return result;
    } catch (error) {
      this.pool.frames.length = checkpoint[0]; this.pool.paintBindings.length = checkpoint[1]; this.pool.metadata.length = checkpoint[2];
      for (const [key, entry] of this.frames) if (entry.index >= checkpoint[0]) this.frames.delete(key);
      for (const [key, id] of this.bindings) if (id >= checkpoint[1]) this.bindings.delete(key);
      for (const [key, id] of this.metadata) if (id >= checkpoint[2]) this.metadata.delete(key);
      throw error;
    }
  }
}

/** One owner-bound schedule. Effects always use the sealed production client. */
class SearchRecipe {
  readonly result: SearchScheduleResult;
  readonly evidence: SearchCaseEvidence;
  readonly started = performance.now();
  readonly requestsStarted: number;
  steps = 0; logicalMilliseconds = 0; captures = 0;
  prepared!: SearchFixturePreparation;
  latest!: SearchSnapshot;
  lastFrame?: OwnedFrameCapture;
  retainedState?: Json;
  recordedObservation?: { state: Json; elements: Json; phaseIndex: number };
  actionAuthority?: { after: AutomationTargetSnapshot; requestId: string; operationId: string };
  readonly recordedProviderRuns = new Map<number, string>();
  readonly frameStore = new SearchFrameStore();
  constructor(readonly runtime: SearchRuntime, readonly contract: SearchCase, readonly schedule: SearchSchedule) {
    this.requestsStarted = runtime.client.driver.stats.requestsSent;
    this.evidence = { phases: [], actions: [], orderComparisons: [], safetyReference: runtime.safety.id, framePool: this.frameStore.pool };
    this.result = { id: schedule.id, caseId: contract.id, status: "failed", executed: false, issues: [], assertions: [], notApplicableAssertions: schedule.notApplicableAssertions, evidence: this.evidence };
  }
  get client(): OwnedEvaluationClient { return this.runtime.client; }
  get target(): AutomationInstance { return this.runtime.target; }
  get frameCursor(): OwnedFrameCursor | undefined { return acknowledgedFrameCursor(this.runtime); }
  resetFrameCursor(): void { acknowledgedSearchFrames.delete(this.runtime); this.retainedState = undefined; this.recordedObservation = undefined; this.actionAuthority = undefined; }
  acknowledgeFrames(trace: Json): void {
    acknowledgedSearchFrames.set(this.runtime, { client: this.client, target: { ...this.target },
      cursor: { traceGeneration: trace.traceGeneration, afterFrameGeneration: trace.latestFrameGeneration } });
  }
  async inspect(id: string, reuseRetained = false): Promise<Json> {
    const reusedObservation = reuseRetained && this.retainedState !== undefined;
    const state = reusedObservation ? this.retainedState! : await this.request(() => this.client.inspect(this.target, this.frameCursor));
    this.recordFramePage(id, state.frameEvidence, [], (_extra, completedFrames) => ({ completedFrames, reusedObservation }));
    this.retainedState = state;
    return state;
  }
  async unmount(): Promise<void> {
    const target = this.target; const state = await this.inspect("before-unmount");
    await this.request(() => this.client.unmount(target, identity(state)));
    this.resetFrameCursor();
  }
  recordFramePage(id: string, trace: Json, additionalFrames: readonly Json[], describe: (extra: SearchFrameReference[], completed: SearchFrameReference[]) => Json, captureTrace?: Json): void {
    if (captureTrace && ["traceGeneration", "afterFrameGeneration", "latestFrameGeneration"].some(key => captureTrace[key] !== trace[key]))
      throw new EvaluationContractError("frame_cursor_response_mismatch");
    const completed = [...(captureTrace ? this.completedFrames(captureTrace, `${id}:capture-history`) : []), ...this.completedFrames(trace, id)];
    try {
      this.recordFrames(id, [...additionalFrames, ...completed.map(frameFacts)], refs => ({
        traceGeneration: trace.traceGeneration, afterFrameGeneration: trace.afterFrameGeneration, latestFrameGeneration: trace.latestFrameGeneration,
        ...describe(refs.slice(0, additionalFrames.length), refs.slice(additionalFrames.length)) }));
    } catch (error) {
      this.evidence.failure = { unretainedFramePage: { target: { ...this.target }, traceGeneration: trace.traceGeneration,
        afterFrameGeneration: trace.afterFrameGeneration, latestFrameGeneration: trace.latestFrameGeneration,
        frameGenerations: [...additionalFrames, ...completed].map(frame => frame.frame.target.frameGeneration), acknowledged: false } };
      throw error;
    }
    this.acknowledgeFrames(trace);
  }
  async request<T>(operation: () => Promise<T>): Promise<T> {
    const limits = this.schedule.bounds;
    if (++this.steps > limits.steps || this.client.driver.stats.requestsSent - this.requestsStarted >= limits.requests || performance.now() - this.started > limits.wallMilliseconds)
      throw new EvaluationContractError("search-case-resource-bound");
    this.retainedState = undefined;
    this.recordedObservation = undefined;
    this.actionAuthority = undefined;
    const value = await operation();
    if (this.client.driver.stats.requestsSent - this.requestsStarted > limits.requests || performance.now() - this.started > limits.wallMilliseconds)
      throw new EvaluationContractError("search-case-resource-bound");
    return value;
  }
  assert(id: string, pass: boolean): void {
    const existing = this.result.assertions.find(assertion => assertion.id === id);
    if (existing) existing.pass &&= pass; else this.result.assertions.push({ id, pass });
    if (!pass && !this.evidence.counterexample && this.latest) {
      const search = this.latest.search;
      const rows = search.committedRows.filter(row => row.semanticId === search.selectedSemanticId || row.selectableOrdinal === 0);
      const beforeBytes = Buffer.byteLength(JSON.stringify(this.result)); let bytes = 0;
      try {
        const retained = this.frameStore.retainWithin(this.lastFrame?.frameEvidence ? [frameFacts(this.lastFrame.frameEvidence)] : [], refs => {
          const counterexample = { assertion: id, targetIdentity: identity(this.latest.state),
            search: { ...search, providers: undefined, preflight: undefined, committedRows: rows },
            collector: collectorRows(this.latest.elements).filter(row => row.selected || row.index === 0),
            providerRuns: this.latest.state.searchProviders.runs.filter((run: SearchProviderRun) => this.schedule.providers.includes(run.source as SearchProvider)),
            frame: refs[0], preflight: { selectedResultKey: this.latest.state.mainWindowPreflight?.selectedResultKey, enterAction: this.latest.state.mainWindowPreflight?.enterAction } };
          const document = { ...this.result, evidence: { ...this.evidence, counterexample } };
          bytes = Buffer.byteLength(JSON.stringify(document)) - beforeBytes;
          if (bytes > 32768) throw new EvaluationContractError("search-counterexample-byte-bound");
          return document;
        }, this.schedule.bounds.retainedBytes);
        this.evidence.counterexample = retained.evidence.counterexample;
      } catch (error) {
        if (!(error instanceof EvaluationContractError) || !["search-evidence-byte-bound", "search-counterexample-byte-bound"].includes(error.code)) throw error;
        this.evidence.counterexample = { assertion: id, targetIdentity: identity(this.latest.state), omittedBytes: bytes, failure: "counterexample-evidence-bound" };
        this.result.issues.push("search-counterexample-byte-bound");
      }
    }
  }
  record(id: string, facts: Json): void { this.recordFrames(id, [], () => facts); }
  recordFrames(id: string, frames: readonly Json[], describe: (refs: SearchFrameReference[]) => Json): void {
    const retained = this.frameStore.retainWithin(frames, refs => ({ ...this.result, evidence: { ...this.evidence,
      phases: [...this.evidence.phases, { id, ...describe(refs) }] } }), this.schedule.bounds.retainedBytes - 40960);
    this.evidence.phases.push(retained.evidence.phases.at(-1)!);
  }
  completedFrames(trace: Json, id: string): Json[] {
    if (!trace || trace.traceOverflow !== false || !Array.isArray(trace.completedFrames) ||
        !Number.isSafeInteger(trace.traceGeneration) || trace.traceGeneration < 1 ||
        !Number.isSafeInteger(trace.latestFrameGeneration) || trace.latestFrameGeneration < 0 ||
        (trace.afterFrameGeneration !== null && (!Number.isSafeInteger(trace.afterFrameGeneration) || trace.afterFrameGeneration < 0)))
      throw new EvaluationContractError("frame_cursor_response_mismatch");
    const cursor = this.frameCursor;
    if (cursor && cursor.traceGeneration !== trace.traceGeneration) throw new EvaluationContractError("frame_cursor_stale");
    if (cursor && trace.latestFrameGeneration < cursor.afterFrameGeneration) throw new EvaluationContractError("frame_cursor_response_mismatch");
    this.assert(`${id}:bounded-complete-trace`, true);
    const completed = trace.completedFrames.filter((frame: Json) => frame.frame.target.frameGeneration > (cursor?.afterFrameGeneration ?? 0));
    for (const frame of completed) {
      this.assert(`${id}:intermediate-frame-${frame.frame.target.frameGeneration}`, frame.paintFailures?.length === 0 && frame.pixelEvidenceComplete === true &&
        (frame.search ? searchObservationIssues(frame.search).length === 0 && paintBindingIssues(frame.search, frame.paintBindings).length === 0 :
          !!frame.fileSearch || (this.contract.id === "eligibility-calculator" && frame.frame.target.appViewVariant === "AgentChatView")));
    }
    return completed;
  }
  async control(command: { operation: "prepare"; scenario: string } | { operation: "release"; runIds: readonly number[] } | { operation: "advance"; milliseconds: number }): Promise<SearchFixtureObservation> {
    const authority = command.operation === "advance" ? this.actionAuthority : undefined;
    const expected = authority?.after ?? identity(await this.inspect(`before-search-${command.operation}`, command.operation !== "prepare"));
    if (authority) this.record("before-search-advance", { targetIdentity: expected, requestId: authority.requestId,
      operationId: authority.operationId, reusedActionReceipt: true, deferredFrameDrain: true });
    const response = await this.request(() => this.client.design({ operation: "fixtureControl", target: this.target, expected, control: { family: "search", ...command } }));
    if (response.operation !== "fixtureControl" || !response.ok) throw new EvaluationContractError("search-control-result-required");
    if (command.operation === "prepare") this.resetFrameCursor();
    const observation = response.observation as SearchFixtureObservation;
    requireIssues(providerObservationIssues(observation.searchProviders));
    return observation;
  }
  async prepare(scenario = this.contract.fixture): Promise<SearchSnapshot> {
    // The prepare-only metadata is validated below; release/advance do not carry it.
    this.prepared = await this.control({ operation: "prepare", scenario }) as SearchFixturePreparation;
    this.result.executed = true; this.lastFrame = undefined;
    this.recordedProviderRuns.clear();
    requireIssues(sourcePlanIssues(this.prepared.sourcePlans));
    const inputs = this.prepared.fileViewInputs;
    if (typeof this.prepared.suggestedInput !== "string" || !this.prepared.suggestedInput || this.prepared.suggestedInput.length > 4096 ||
        !inputs || Object.keys(inputs).some(key => !["full", "mini", "preview"].includes(key)) ||
        [inputs.full, inputs.mini, inputs.preview].some(input => typeof input !== "string" || !input || input.length > 4096) ||
        !inputs.full.startsWith("/") || this.plan("directory").input !== `files: ${inputs.mini}` || !inputs.mini.startsWith("~/") || !inputs.preview.startsWith(inputs.mini))
      throw new EvaluationContractError("missing-capability:compiled-file-view-inputs");
    this.record("prepare", { scenario, sourcePlans: this.prepared.sourcePlans, fileViewInputs: inputs, providers: { ...this.prepared.searchProviders, retiredGate: undefined } });
    for (const run of this.prepared.searchProviders.runs) this.recordedProviderRuns.set(run.id, JSON.stringify(run));
    return this.capture("prepared-forced-baseline", false);
  }
  plan(source: SearchProvider): SearchSourcePlan {
    const plan = this.prepared.sourcePlans.find(plan => plan.source === source);
    if (!plan) throw new EvaluationContractError(`missing-capability:source-plan:${source}`);
    return plan;
  }
  async observe(id: string, reuseRetained = false): Promise<SearchSnapshot> {
    const reusedObservation = reuseRetained && this.retainedState !== undefined && this.retainedState === this.latest?.state;
    const previous = this.latest;
    const state = await this.inspect(`${id}:frames`, reusedObservation);
    const elements = reusedObservation ? previous.elements : await this.request(() => this.client.query(this.target, "elements"));
    requireIssues(providerObservationIssues(state.searchProviders));
    if (state.windowVisible !== false) throw new EvaluationContractError("hidden-window-required");
    const snapshot = { state, elements, search: state.searchObservation as SearchObservation };
    this.latest = snapshot;
    requireIssues(searchObservationIssues(snapshot.search, elements));
    this.assert(`${id}:collector-version`, elements.projectionVersion === 2);
    const trace = state.frameEvidence;
    const completed = this.completedFrames(trace, id);
    const runs: SearchProviderRun[] = state.searchProviders.runs;
    const providerRunsAreChanges = this.recordedProviderRuns.size > 0;
    const changedRuns = runs.filter(run => {
      const encoded = JSON.stringify(run);
      if (this.recordedProviderRuns.get(run.id) === encoded) return false;
      this.recordedProviderRuns.set(run.id, encoded); return true;
    });
    this.assert(`${id}:provider-runs-retained`, this.recordedProviderRuns.size === runs.length);
    const observationRef = reusedObservation && changedRuns.length === 0 && completed.length === 0 &&
      this.recordedObservation?.state === state && this.recordedObservation.elements === elements ? this.recordedObservation.phaseIndex : undefined;
    if (observationRef !== undefined) this.record(id, { observationRef, reusedObservation: true, providerRuns: [], providerRunsAreChanges: true, completedFrames: [] });
    else {
      const phaseIndex = this.evidence.phases.length;
      const noteSubtitles = snapshot.search.committedRows.filter(row => row.stableKey.startsWith("note/")).map(row => ({
        semanticId: row.semanticId, value: elements.elements.find((element: Json) => rowId(element) === row.semanticId)?.content?.value ?? null }));
      const expectedNoteSubtitleFingerprint = state.searchProviders.expectedNoteSubtitleFingerprint;
      if (noteSubtitles.length && (typeof expectedNoteSubtitleFingerprint !== "string" || !/^sha256:[a-f0-9]{64}$/.test(expectedNoteSubtitleFingerprint))) {
        throw new EvaluationContractError("missing-capability:notes-display-clock");
      }
      this.recordFrames(id, completed.map(frameFacts), refs => ({ targetIdentity: identity(state), reusedObservation, query: snapshot.search.query, computedQuery: snapshot.search.computedQuery,
        pending: snapshot.search.pending, resultRevision: snapshot.search.resultRevision,
        selectionRevision: snapshot.search.selectionRevision, viewportRevision: snapshot.search.viewportRevision,
        selectedSemanticId: snapshot.search.selectedSemanticId, selectedOrdinal: snapshot.search.selectedOrdinal,
        selectionIntent: snapshot.search.selectionIntent, selectionArmed: snapshot.search.selectionArmed, viewportIntent: snapshot.search.viewportIntent,
        reconciliationReason: snapshot.search.reconciliationReason, publication: snapshot.search.publication,
        rankingFingerprint: rankingFingerprint(snapshot.search), selectionFingerprint: selectionFingerprint(snapshot.search),
        notesDisplayClock: noteSubtitles.length ? { expectedFingerprint: expectedNoteSubtitleFingerprint, rows: noteSubtitles } : undefined,
        providerRuns: changedRuns, providerRunsAreChanges, providerRunCount: runs.length, providerRunsHash: digest(runs), traceGeneration: trace?.traceGeneration,
        scroll: this.contract.inputRoute === "gpui-scroll" ? state.mainListScroll : undefined,
        completedFrames: refs }));
      this.recordedObservation = { state, elements, phaseIndex };
      if (noteSubtitles.length) this.assert(`${id}:notes-use-held-display-clock`, noteSubtitles.every(row =>
        row.value?.rawContentReturned === false && row.value.fingerprint === expectedNoteSubtitleFingerprint));
    }
    const preflight = state.mainWindowPreflight;
    this.assert(`${id}:preflight-selection`, Boolean(preflight) && (preflight.selectedResultKey ?? null) === (selected(snapshot.search)?.stableKey ?? null));
    this.retainedState = state;
    return snapshot;
  }
  async capture(id: string, scheduled = true): Promise<SearchSnapshot> {
    if (++this.captures > this.schedule.bounds.frames) throw new EvaluationContractError("search-case-frame-bound");
    const authority = scheduled ? this.actionAuthority : undefined;
    const before = authority ? undefined : await this.observe(`${id}:state`, true);
    let expectation: ScheduledCapture | undefined;
    if (scheduled) {
      if (!this.lastFrame?.frameEvidence?.scheduledCapability) throw new EvaluationContractError("missing-capability:scheduled-frame");
      expectation = { expected: authority?.after ?? identity(before!.state), afterFrameGeneration: this.lastFrame.frame.target.frameGeneration,
        afterNotificationEpoch: this.lastFrame.frameEvidence.notificationEpoch };
    }
    const frame = await this.request(() => this.client.captureFrame(this.target, false, expectation, this.frameCursor));
    const search = frame.state.searchObservation as SearchObservation;
    this.latest = { state: frame.state, elements: frame.elements, search, frame };
    requireIssues(searchObservationIssues(search, frame.elements));
    if (expectation) for (const issue of naturalEvidenceIssues(frame, expectation, this.frameStore.pool)) this.assert(`${id}:${issue}`, false);
    const authorityFields: readonly (keyof SearchObservation)[] = ["query", "computedQuery", "pending", "resultRevision", "selectionRevision", "viewportRevision", "selectionIntent", "selectionArmed", "viewportIntent", "selectedSemanticId", "selectedOrdinal", "publication", "committedRows"];
    this.assert(`${id}:draw-owner-join`, authorityFields.every(field => JSON.stringify(frame.frameEvidence?.search?.[field]) === JSON.stringify(search[field])));
    this.assert(`${id}:native-paint-join`, frame.frameEvidence?.paintFailures?.length === 0 && frame.frameEvidence?.pixelEvidenceComplete === true);
    for (const issue of paintBindingIssues(search, frame.frameEvidence?.paintBindings)) this.assert(`${id}:${issue}`, false);
    this.assert(`${id}:filter-local-focus`, frame.frameEvidence?.localInputFocused === true);
    this.lastFrame = frame;
    const nativeSamples = frame.frameEvidence?.pixelEvidence;
    if (!Array.isArray(nativeSamples)) throw new EvaluationContractError("missing-capability:native-pixel-evidence");
    this.assert(`${id}:single-painted-marker`, nativeSamples.filter((sample: Json) => sample.kind === "selectionMarker").length <= 1);
    let pixels: Json = {};
    if (nativeSamples.length) {
      const probes = nativeSamples.map((sample: Json) => ({ x: sample.probe.x, y: sample.probe.y }));
      const sampled = await this.request(() => this.client.probePixels(this.target, frame.frame.target, probes));
      this.assert(`${id}:retained-pixel-join`, sampled.pixelProbes?.length === nativeSamples.length && sampled.pixelProbes.every((pixel, index) =>
        ["x", "y", "r", "g", "b", "a"].every(channel => pixel[channel as keyof PixelProbeResult] === nativeSamples[index].probe[channel])));
      for (const sample of nativeSamples) if (sample.kind === "selectionMarker") {
        this.assert(`${id}:marker-subject`, sample.semanticId === search.selectedSemanticId);
        const a = sample.visibleBounds; const b = sample.bounds;
        if (a.x <= b.x && a.y <= b.y && a.x + a.width >= b.x + b.width && a.y + a.height >= b.y + b.height)
          for (const issue of selectionPixelIssues([sample.probe], frame.frameEvidence!.search.selectionMarkerColor)) this.assert(`${id}:${issue}`, false);
      }
      pixels = { ...pixels, sampled: sampled.pixelProbes };
    }
    this.recordFramePage(id, frame.state.frameEvidence, [frameFacts(frame.frameEvidence!)], (extra, completedFrames) => ({
      frameEvidence: extra[0], completedFrames, capture: frame.snapshot.capture, pixels: { nativeSamplesFrame: extra[0], ...pixels } }), frame.frameEvidence);
    // Flow control is needed only under pressure, not after every capture. Keep
    // three quarters of native capacity available without spending a request per frame.
    const { retainedTraceBytes, maxRetainedTraceBytes } = frame.state.frameEvidence;
    if (!Number.isSafeInteger(retainedTraceBytes) || retainedTraceBytes < 1 || !Number.isSafeInteger(maxRetainedTraceBytes) ||
        maxRetainedTraceBytes < 1 || retainedTraceBytes > maxRetainedTraceBytes)
      throw new EvaluationContractError("missing-capability:frame-retention-pressure");
    if (retainedTraceBytes >= maxRetainedTraceBytes / 4) {
      // The page is losslessly retained. Keep this exact capture as the next
      // scheduled baseline, even when the read-page cursor is newer.
      const acknowledgement = await this.request(() => this.client.acknowledgeFrames(this.target, frame.frame.target,
        { traceGeneration: frame.frameEvidence!.traceGeneration, afterFrameGeneration: frame.frame.target.frameGeneration }));
      this.record(`${id}:frame-acknowledgement`, acknowledgement);
    }
    this.retainedState = frame.state;
    // A completed action binds the draw directly. Its full decoded bundle is retained
    // before these assertions consume the capture's actual state and collector.
    if (authority) this.latest = { ...await this.observe(`${id}:state`, true), frame };
    return this.latest;
  }
  async advance(milliseconds: number): Promise<void> {
    if (!Number.isSafeInteger(milliseconds) || milliseconds < 0 || this.logicalMilliseconds + milliseconds > this.schedule.bounds.logicalMilliseconds)
      throw new EvaluationContractError("search-case-clock-bound");
    this.logicalMilliseconds += milliseconds;
    for (let remaining = milliseconds; remaining > 0; remaining -= Math.min(1000, remaining)) await this.control({ operation: "advance", milliseconds: Math.min(1000, remaining) });
  }
  async action(action: OwnedAction, expected = identity(this.latest.state)): Promise<Json> {
    const response = await this.request(() => this.client.act(this.target, action, expected));
    const receipt = response.actionReceipt ?? response.results?.[0]?.actionReceipt;
    this.evidence.actions.push({ action, receipt });
    if (receipt?.dispatchCompleted === true && receipt.after) this.actionAuthority = {
      after: receipt.after, requestId: receipt.requestId, operationId: receipt.operationId };
    return response;
  }
  async input(text: string, route: SearchCase["inputRoute"] = "setInput", commit = true): Promise<SearchSnapshot> {
    await this.observe("before-input", true);
    const changed = this.latest.search.rawInput !== text;
    if (route === "gpui-text" || route === "gpui-keyboard") {
      await this.action({ type: "key", key: "a", modifiers: ["cmd"] });
      await this.observe("input-selected");
      await this.action({ type: "key", key: text[0] ?? "backspace", ...(text ? { text } : {}) });
    } else await this.action({ type: "setInput", text });
    if (commit) { await this.advance(250); return changed ? this.capture("input-committed") : this.observe("input-unchanged"); }
    return this.observe("raw-input-pending");
  }
  runs(source?: SearchProvider): SearchProviderRun[] {
    // observe() validated this immutable provider snapshot before storing it.
    const observation = this.latest.state.searchProviders as SearchProviderObservation;
    const runs = observation.runs;
    return source ? runs.filter(run => run.source === source) : runs;
  }
  async release(runs: readonly SearchProviderRun[], label = "provider-publication"): Promise<SearchSnapshot> {
    if (!runs.length || runs.some(run => !admissionReady(run))) throw new EvaluationContractError("exact-pending-source-admissions-required");
    const before = this.latest;
    await this.control({ operation: "release", runIds: runs.map(run => run.id) });
    let state = await this.observe(`${label}:released`);
    for (let tick = 0; tick < 8 && runs.some(run => !TERMINAL.includes(this.runs().find(current => current.id === run.id)?.state ?? "")); tick++) {
      await this.advance(25); state = await this.observe(`${label}:terminal-${tick}`);
    }
    const terminal = runs.map(run => this.runs().find(current => current.id === run.id));
    this.assert(`${label}:all-exact-runs-terminal`, terminal.every(run => run && TERMINAL.includes(run.state)));
    for (const run of terminal) if (run?.kind === "sourceChange") {
      this.assert(`${label}:source-change-applied`, run.admissionApplied === true);
      const reads = this.runs(run.source as SearchProvider).filter(read => read.kind === "synchronousRead" && read.originAdmissionId === run.id);
      if (this.plan(run.source as SearchProvider).input === state.search.rawInput) this.assert(`${label}:synchronous-owner-read`, reads.length > 0 && reads.every(read => TERMINAL.includes(read.state)));
      this.record(`${label}:synchronous-read`, { executionClass: "compiled-source-change-and-synchronous-owner-read", admission: run, reads });
    }
    this.record(`${label}:completion`, { releasedRunIds: runs.map(run => run.id), terminal });
    const published = state.search.publication?.sequence !== before.search.publication?.sequence;
    if (published) state = await this.capture(label);
    else {
      this.assert(`${label}:no-unattributed-result-change`, rankingFingerprint(before.search) === rankingFingerprint(state.search) && before.search.resultRevision === state.search.resultRevision);
      const baselineGeneration = identity(before.state).frameGeneration;
      const frames = this.frameStore.pool.frames.map((_entry, index) => reconstructSearchFrameFacts(this.frameStore.pool, index)).filter(frame => frame.traceGeneration === state.state.frameEvidence.traceGeneration &&
        frame.frame.target.windowId === this.target.id && frame.frame.target.windowGeneration === this.target.generation && frame.frame.target.frameGeneration > baselineGeneration);
      this.assert(`${label}:no-hidden-list-publication`, frames.every((frame: Json) => frame.search?.resultRevision === before.search.resultRevision));
    }
    this.assertIntent(`${label}:selection-policy`, before.search, state.search);
    return state;
  }
  assertIntent(id: string, before: SearchObservation, after: SearchObservation): void {
    const old = selected(before); const current = selected(after);
    if (!before.pending && sameQuery(before.query, after.query) && before.selectionArmed) this.assert(`${id}:arming-preserved`, after.selectionArmed);
    if (after.pending) this.assert(id, sameQuery(before.computedQuery, after.computedQuery) && selectionFingerprint(before) === selectionFingerprint(after));
    else if (!after.selectionArmed) this.assert(id, !current);
    else if (!before.pending && sameQuery(before.query, after.query) && old) {
      const retained = after.committedRows.some(row => row.stableKey === old.stableKey && row.selectable);
      this.assert(id, retained ? current?.stableKey === old.stableKey : !current);
    } else if (after.selectionIntent.kind === "automaticTop") this.assert(id, !current);
    else this.assert(id, !current || current.selectable);
  }
  async anchor(ordinal = this.latest.search.selectedOrdinal ?? 0): Promise<SearchSnapshot> {
    if (!this.latest.search.selectionArmed) {
      await this.action({ type: "key", key: "down" }); await this.capture("source-first-down-armed");
      this.assert("source-first-down-armed", this.latest.search.selectionArmed && this.latest.search.selectedOrdinal === 0);
    }
    if (this.latest.search.selectionIntent.kind === "explicitAnchor" && this.latest.search.selectedOrdinal === ordinal) return this.observe("explicit-anchor-current", true);
    const row = this.latest.search.committedRows.find(row => row.selectableOrdinal === ordinal && row.selectable);
    if (!row) throw new EvaluationContractError("missing-capability:deliberate-row");
    await this.action({ type: "select", semanticId: row.semanticId });
    return this.capture("explicit-anchor");
  }
  async refuse(action: () => Promise<unknown>, id: string, codes?: readonly string[]): Promise<string | undefined> {
    try { await this.request(action); }
    catch (error) {
      if (!(error instanceof DriverCommandRefused || error instanceof EvaluationContractError)) throw error;
      this.assert(id, codes ? codes.includes(error.code) : /stale|retired|unknown|not_found|not_mounted|query_pending|query_not_current/.test(error.code));
      this.record(id, { refusal: error.code }); return error.code;
    }
    this.assert(id, false); return undefined;
  }
  async pointer(kind: "mouseClick" | "mouseDown" | "mouseUp" | "mouseMove", semanticId: string, frame = this.lastFrame!): Promise<Json> {
    const binding = frame.frameEvidence?.paintBindings?.find((item: Json) => item.kind === "mainSearchRow" && item.id === semanticId);
    const bounds = binding?.visibleBounds;
    if (!bounds || bounds.width <= 0 || bounds.height <= 0) throw new EvaluationContractError("missing-capability:row-paint-hit-bounds");
    return this.action({ type: "gpuiEvent", frame: frame.frame, event: { type: kind, button: "left", x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 } }, frame.frame.target);
  }
}

async function resolveSource(recipe: SearchRecipe, source: SearchProvider, label: string, afterRunId = 0, acceptCached = false): Promise<{ status: "admitted" | "settled"; run: SearchProviderRun } | { status: "cached"; run: null; cache: OwnedSearchSourceCacheReadiness }> {
  if (acceptCached && afterRunId !== 0) throw new EvaluationContractError("search_provider_condition_invalid");
  const query = recipe.latest.search.query;
  for (let attempt = 0; attempt < 4; attempt++) {
    if (recipe.latest.search.pending || !sameQuery(query, recipe.latest.search.query) || !sameQuery(query, recipe.latest.search.computedQuery))
      throw new EvaluationContractError("source-admission-query-changed");
    if (recipe.plan(source).workKind === "synchronous") {
      const admission = recipe.runs(source).find(run => run.id > afterRunId && run.kind === "sourceChange" && admissionReady(run));
      if (admission) return { status: "admitted", run: admission };
    }
    const retained = currentSourceResolution(recipe.latest.search, recipe.latest.state.searchProviders, source, afterRunId);
    if (retained) {
      recipe.record(`${label}:current-owner`, { source, query, afterRunId, ...retained, reusedObservation: true });
      return retained;
    }
    const cached = acceptCached ? currentSourceCache(recipe.latest.search, source, afterRunId) : undefined;
    if (cached) {
      recipe.record(`${label}:current-cache`, { source, query, afterRunId, status: "cached", owner: null, run: null, cache: cached, reusedObservation: true });
      return { status: "cached", run: null, cache: cached };
    }
    if (acceptCached) {
      const ownership = recipe.latest.search.providers;
      const owner = ownership?.runs?.find((run: Json) => run.source === source);
      const currentDesired = ownership?.desired?.some((desired: Json) => desired.source === source && desired.query && sameQuery(desired.query, query));
      if (owner?.queryBound === true && owner.terminal === null && (!owner.consumer || !sameQuery(owner.consumer, query)) && !currentDesired) {
        const blocker = recipe.runs(source).findLast(run => run.kind === "worker" && run.generation === owner.generation && run.state === "held");
        if (blocker) {
          recipe.record(`${label}:retired-owner-drain`, { source, query, owner, run: blocker });
          await recipe.release([blocker], `${label}:retired-owner-${attempt}`);
          const terminal = recipe.runs(source).find(run => run.id === blocker.id);
          recipe.assert(`${label}:retired-owner-not-current`, terminal?.state === "stale-discarded" && sameQuery(query, recipe.latest.search.query));
          continue;
        }
      }
    }
    const condition = { type: "searchProvider", source, query, afterRunId, ...(acceptCached ? { acceptCached: true } : {}) };
    recipe.record(`${label}:admission-request`, { condition, target: recipe.target,
      owner: recipe.latest.search.providers?.runs?.find((owner: Json) => owner.source === source) ?? null,
      desired: recipe.latest.search.providers?.desired?.find((desired: Json) => desired.source === source) ?? null,
      gateRuns: recipe.runs(source), cache: currentSourceCache(recipe.latest.search, source) ?? null });
    const response = await recipe.request(() => recipe.client.wait(recipe.target, condition,
      Math.max(1, Math.min(5000, recipe.schedule.bounds.wallMilliseconds - (performance.now() - recipe.started)))));
    const proof = response.searchProvider as Json;
    recipe.record(`${label}:admission-wait`, { observation: proof });
    if (!proof || proof.version !== 1 || proof.source !== source || !proof.query || !sameQuery(proof.query, query) || proof.afterRunId !== afterRunId ||
        !["admitted", "blocked", "settled", "cached"].includes(proof.status)) throw new EvaluationContractError("invalid-source-admission-proof");
    await recipe.observe(`${label}:admission-observed`);
    if (!sameQuery(query, recipe.latest.search.query) || recipe.latest.search.pending) throw new EvaluationContractError("source-admission-query-changed");
    if (proof.status === "cached") {
      const cache = acceptCached ? currentSourceCache(recipe.latest.search, source, afterRunId) : undefined;
      if (!cache || proof.owner !== null || proof.run !== null || !Array.isArray(proof.blockers) || proof.blockers.length ||
          !isOwnedSearchSourceCacheReadiness(proof.cache) || canonicalFrameJson(cache) !== canonicalFrameJson(proof.cache))
        throw new EvaluationContractError("source-cache-readiness-changed");
      return { status: "cached", run: null, cache };
    }
    if (proof.status !== "blocked") {
      const current = currentSourceResolution(recipe.latest.search, recipe.latest.state.searchProviders, source, afterRunId);
      if (!current || current.status !== proof.status || current.run.id !== proof.run?.id || current.run.generation !== proof.run?.generation ||
          current.owner.generation !== proof.owner?.generation) throw new EvaluationContractError("source-admission-owner-changed");
      return current;
    }
    if (proof.pendingDesired !== true || !Array.isArray(proof.blockers) || !proof.blockers.length || new Set(proof.blockers.map((blocker: Json) => blocker.run?.id)).size !== proof.blockers.length)
      throw new EvaluationContractError("exact-source-blockers-required");
    const blockers = proof.blockers.map((blocker: Json) => {
      const run = recipe.runs().find(run => run.id === blocker.run?.id);
      const owner = recipe.latest.search.providers?.runs?.find((owner: Json) => owner.source === run?.source);
      if (!run || run.kind !== "worker" || !admissionReady(run) || run.source !== blocker.run.source || run.generation !== blocker.run.generation ||
          !owner || owner.generation !== run.generation || canonicalFrameJson(owner) !== canonicalFrameJson(blocker.owner))
        throw new EvaluationContractError("source-blocker-owner-changed");
      return run;
    });
    recipe.record(`${label}:blocked-owner-drain`, { source, query, blockers: proof.blockers });
    await recipe.release(blockers, `${label}:blocked-owner-${attempt}`);
  }
  throw new EvaluationContractError(`native-source-admission-not-settled:${source}`);
}
async function finishSource(recipe: SearchRecipe, source: SearchProvider, label: string, afterId = 0): Promise<SearchSnapshot> {
  const resolution = await resolveSource(recipe, source, label, afterId);
  if (resolution.status === "settled") return recipe.latest;
  if (resolution.status === "cached") throw new EvaluationContractError("unexpected-source-cache-resolution");
  if (recipe.plan(source).workKind === "synchronous" && resolution.run.kind !== "sourceChange") throw new EvaluationContractError("synchronous-source-admission-required");
  return recipe.release([resolution.run], label);
}
async function awaitWork(recipe: SearchRecipe, source: SearchProvider, afterId = 0): Promise<SearchProviderRun> {
  const resolution = await resolveSource(recipe, source, `await-source:${source}`, afterId);
  if (resolution.status !== "admitted") throw new EvaluationContractError(`missing-capability:pending-source-admission:${source}`);
  return resolution.run;
}
async function awaitSourceRefresh(recipe: SearchRecipe, source: SearchProvider, initialId: number): Promise<SearchProviderRun> {
  const before = recipe.latest.search;
  for (let drain = 0; drain < 4; drain++) {
    if (!recipe.runs(source).some(run => run.id > initialId && admissionReady(run))) {
      const observation = recipe.latest.state.searchProviders as SearchProviderObservation;
      const pending = observation.pendingSourceChanges?.find(change => change.source === source);
      if (!pending) throw new EvaluationContractError(`missing-capability:compiled-source-change:${source}`);
      await recipe.advance(Math.max(1, pending.dueAtMs - observation.logicalTimeMs));
      await recipe.observe("source-change-admitted");
    }
    const run = await awaitWork(recipe, source, initialId);
    recipe.assert("source-change-preserves-query", sameQuery(before.query, recipe.latest.search.query));
    if (run.payloadPhase === 1) return run;
    if (run.payloadPhase !== 0) throw new EvaluationContractError(`missing-capability:phase-one-source-refresh:${source}`);
    await recipe.release([run], "queued-phase-zero-native-completion");
    const terminal = recipe.runs(source).find(current => current.id === run.id);
    recipe.assert("queued-phase-zero-completed-or-retired", terminal?.state === "completed" || terminal?.state === "stale-discarded");
    initialId = run.id;
  }
  throw new EvaluationContractError(`native-phase-zero-drain-not-settled:${source}`);
}
async function followupSource(recipe: SearchRecipe, source: SearchProvider, initialId: number): Promise<SearchSnapshot> {
  return recipe.release([await awaitSourceRefresh(recipe, source, initialId)], "source-refresh-publication");
}
async function stageSourceRefresh(recipe: SearchRecipe, source: SearchProvider, label: string): Promise<SearchProviderRun> {
  if (source === "icons") await finishSource(recipe, "windows", `${label}:window-baseline`);
  let seed = await awaitWork(recipe, source);
  if (seed.kind === "sourceChange") {
    const baseline = recipe.runs(source).filter(run => run.kind === "synchronousRead" && run.originAdmissionId == null && run.payloadPhase === 0 && run.state === "completed");
    if (!baseline.length) throw new EvaluationContractError(`missing-capability:synchronous-success-baseline:${source}`);
    recipe.record(`${label}:synchronous-success-baseline`, { providers: baseline });
    return seed;
  }
  for (let drain = 0; drain < 4; drain++) {
    if (seed.payloadPhase !== 0) throw new EvaluationContractError(`missing-capability:native-phase-zero-baseline:${source}`);
    await recipe.release([seed], `${label}:native-success-baseline`);
    const completed = recipe.runs(source).find(run => run.id === seed.id);
    if (completed?.state === "completed" && ["success", "empty"].includes(completed.outcome ?? "")) {
      recipe.assert(`${label}:real-success-baseline`, true);
      return awaitSourceRefresh(recipe, source, seed.id);
    }
    if (completed?.state !== "stale-discarded") throw new EvaluationContractError(`missing-capability:native-success-baseline:${source}`);
    recipe.record(`${label}:superseded-baseline-work`, { provider: completed });
    seed = await awaitWork(recipe, source, seed.id);
  }
  throw new EvaluationContractError(`native-baseline-drain-not-settled:${source}`);
}

async function arrivalRecipe(recipe: SearchRecipe): Promise<void> {
  const id = recipe.contract.id;
  await recipe.prepare();
  const initial = await recipe.input(recipe.prepared.suggestedInput, recipe.contract.inputRoute);
  const first = selected(initial.search);
  if (!first) throw new EvaluationContractError("missing-capability:initial-selected-row");
  if (id === "keyboard-anchor-arrival") {
    for (const key of ["down", "up"]) {
      const action = await recipe.action({ type: "key", key });
      recipe.assert(`${key}-dispatched`, action.actionReceipt?.dispatchCompleted === true);
      await recipe.capture(`${key}-selected`);
    }
    recipe.assert("down-up-dispatched", recipe.result.assertions.filter(assertion => ["down-dispatched", "up-dispatched"].includes(assertion.id)).every(assertion => assertion.pass));
  } else if (["semantic-anchor-current-first", "same-input-noop", "stale-agent-target"].includes(id)) await recipe.anchor();
  else if (id === "click-anchor-arrival" || id === "pointer-down-publication-up") {
    const row = initial.search.committedRows.find(row => row.selectable && row.semanticId !== first.semanticId);
    if (!row) throw new EvaluationContractError("missing-capability:unselected-row");
    const action = await recipe.pointer(id === "click-anchor-arrival" ? "mouseClick" : "mouseDown", row.semanticId);
    if (id === "click-anchor-arrival") {
      recipe.assert("unselected-row-click", action.actionReceipt?.dispatchCompleted === true);
      await recipe.capture("clicked-row");
    }
  }
  const before = await recipe.observe("before-release");
  const anchor = selected(before.search)!;
  const pointerFrame = recipe.lastFrame!;
  const run = await awaitWork(recipe, "tabs");
  const beforeFingerprint = selectionFingerprint(before.search);
  await recipe.advance(50);
  const held = await recipe.observe("held-provider");
  requireIssues(heldProviderIssues(run, recipe.runs().find(current => current.id === run.id), beforeFingerprint, selectionFingerprint(held.search)));
  if (id === "same-input-noop") {
    await recipe.action({ type: "setInput", text: recipe.prepared.suggestedInput });
    const unchanged = await recipe.observe("same-input-noop");
    recipe.assert("selection-unchanged", selectionFingerprint(unchanged.search) === beforeFingerprint);
    recipe.assert("no-worker-duplication", digest(unchanged.state.searchProviders.runs) === digest(held.state.searchProviders.runs));
    recipe.assert("no-revision-churn", sameQuery(unchanged.search.query, held.search.query) &&
      unchanged.search.selectionRevision === held.search.selectionRevision && unchanged.search.resultRevision === held.search.resultRevision && unchanged.search.viewportRevision === held.search.viewportRevision);
  }
  const after = await recipe.release([run]);
  const current = selected(after.search);
  const arrived = after.search.committedRows.some(row => row.selectable && !initial.search.committedRows.some(old => old.stableKey === row.stableKey));
  if (id === "automatic-higher-arrival") {
    recipe.assert("late-arrival-visible", arrived);
    recipe.assert("automatic-anchor-preserved", current?.stableKey === anchor.stableKey);
    const markerBefore = pointerFrame.frameEvidence?.pixelEvidence?.find((sample: Json) => sample.kind === "selectionMarker");
    const markerAfter = recipe.lastFrame?.frameEvidence?.pixelEvidence?.find((sample: Json) => sample.kind === "selectionMarker");
    recipe.assert("selected-position-preserved", Boolean(markerBefore && markerAfter) &&
      markerAfter.semanticId === anchor.semanticId && Math.abs(markerAfter.bounds.y - markerBefore.bounds.y) <= 0.5);
  } else if (id === "keyboard-anchor-arrival") {
    recipe.assert("late-arrival-visible", arrived); recipe.assert("anchor-preserved", current?.stableKey === anchor.stableKey);
    recipe.assert("ordinal-preserved", current?.selectableOrdinal === anchor.selectableOrdinal && after.search.publication?.source === "tabs" && after.search.publication?.sourceGeneration === run.generation);
  } else if (id === "semantic-anchor-current-first") {
    recipe.assert("same-index-deliberate-intent", anchor.selectableOrdinal === 0 && before.search.selectionIntent.kind === "explicitAnchor");
    recipe.assert("owner-revision-advanced", before.search.selectionRevision > initial.search.selectionRevision);
    recipe.assert("anchor-preserved", current?.stableKey === anchor.stableKey);
  } else if (id === "click-anchor-arrival") recipe.assert("anchor-preserved", current?.stableKey === anchor.stableKey);
  else if (id === "pointer-down-publication-up") {
    const row = initial.search.committedRows.find(row => row.selectable && row.semanticId !== first.semanticId)!;
    await recipe.refuse(() => recipe.pointer("mouseUp", row.semanticId, pointerFrame), "gesture-subject-preserved-or-refused");
    const unchanged = await recipe.observe("stale-pointer-up-refused");
    recipe.assert("no-rebound-activation", selectionFingerprint(unchanged.search) === selectionFingerprint(after.search));
  } else if (id === "stale-agent-target") {
    await recipe.refuse(() => recipe.client.act(recipe.target, { type: "select", semanticId: anchor.semanticId }, identity(before.state)), "stale-operation-refused-before-effect");
    const unchanged = await recipe.observe("stale-agent-refused");
    recipe.assert("current-state-unchanged", selectionFingerprint(unchanged.search) === selectionFingerprint(after.search));
  }
  if (recipe.contract.assertions.includes("filter-local-focus")) recipe.assert("filter-local-focus", recipe.lastFrame?.frameEvidence?.localInputFocused === true);
  if (id === "automatic-higher-arrival") {
    try { await recipe.action({ type: "key", key: "enter" }); }
    catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; }
    const dispatched = await recipe.observe("late-arrival-enter");
    recipe.assert("enter-keeps-visible-subject", dispatchBindingIssues({ ...before.search, dispatch: dispatched.search.dispatch }).length === 0);
    recipe.record("late-arrival-enter-subject", { visibleSubject: anchor, dispatch: dispatched.search.dispatch });
  }
}

async function filesRecipe(recipe: SearchRecipe): Promise<void> {
  for (const deliberate of recipe.contract.selectionIntent === "both" ? [false, true] : [false]) {
    await recipe.prepare();
    const input = recipe.contract.id === "explicit-files-publish" ? recipe.prepared.suggestedInput : recipe.plan("files").input;
    await recipe.input(input);
    const work = await awaitWork(recipe, "files");
    if (deliberate) await recipe.anchor();
    const before = recipe.latest;
    if (recipe.contract.id === "pending-files-reuse") {
      await recipe.input(`files: ${input}`);
      const attached = await awaitWork(recipe, "files");
      recipe.assert("compatible-work-reused", attached.id === work.id && attached.generation === work.generation);
      recipe.assert("no-worker-duplication", recipe.runs("files").length === before.state.searchProviders.runs.filter((run: SearchProviderRun) => run.source === "files").length);
      recipe.assert("current-attachment-authority", !recipe.latest.search.pending && recipe.latest.search.query.revision > before.search.query.revision);
    }
    const after = await recipe.release([await awaitWork(recipe, "files")], "file-terminal");
    const terminal = recipe.runs("files").find(run => run.id === work.id)!;
    if (recipe.contract.id === "implicit-files-cache-only") {
      recipe.assert("terminal-cache-settled", terminal.state === "completed" && terminal.publicationPolicy === "cache-only" && !after.state.searchProviders.pendingRunIds.includes(work.id));
      recipe.assert("rows-selection-viewport-unchanged", selectionFingerprint(before.search) === selectionFingerprint(after.search));
      recipe.assert("no-list-republication", before.search.publication?.sequence === after.search.publication?.sequence);
    } else {
      recipe.assert("completion-time-visible-policy", terminal.publicationPolicy === "visible");
      recipe.assert("file-rows-published", after.state.mainWindowPreflight.visibleResults.some((row: Json) => row.role === "rootFile") && after.search.resultRevision > before.search.resultRevision);
    }
  }
}

async function pendingQueryRecipe(recipe: SearchRecipe): Promise<void> {
  for (const source of recipe.contract.providers) {
    await recipe.prepare(); await recipe.input(recipe.plan(source).input);
    const old = await awaitWork(recipe, source); const before = recipe.latest;
    const pending = await recipe.input(`${recipe.plan(source).input}x`, "gpui-text", false);
    recipe.assert("controlled-coalescer", pending.search.pending && !sameQuery(pending.search.query, pending.search.computedQuery));
    if (recipe.contract.id === "query-aba") {
      await recipe.input(recipe.plan(source).input, "gpui-text", false);
      recipe.assert("aba-intent-distinct", recipe.latest.search.query.revision > before.search.query.revision && recipe.latest.search.rawInput === before.search.rawInput);
    } else {
      let refusal: string | undefined; let actionReceipt: Json | undefined;
      try { actionReceipt = (await recipe.action({ type: "key", key: "enter" })).actionReceipt; }
      catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; refusal = error.code; }
      const dispatched = await recipe.observe("raw-query-enter-flushed");
      recipe.assert("enter-uses-current-query", !dispatched.search.pending && sameQuery(dispatched.search.query, pending.search.query) &&
        sameQuery(dispatched.search.query, dispatched.search.computedQuery) && dispatchBindingIssues(dispatched.search).length === 0 &&
        (dispatched.search.dispatch !== null || Boolean(refusal) || actionReceipt?.effect?.kind === "noOp"));
      recipe.record("raw-query-dispatch", { source, queryBefore: before.search.query, requestedQuery: pending.search.query,
        dispatch: dispatched.search.dispatch, refusal, actionReceipt });
    }
    const after = await recipe.release([old], "old-query-completion");
    const terminal = recipe.runs().find(run => run.id === old.id)!;
    const retired = ["stale-discarded", "cancelled"].includes(terminal.state) || terminal.publicationPolicy === "cache-only";
    if (recipe.contract.id === "query-aba") recipe.assert("old-a-authority-retired", retired && !(after.search.publication?.source === old.source && after.search.publication?.sourceGeneration === old.generation));
    else recipe.assert("old-completion-not-new-intent", retired && (after.search.pending || after.search.computedInput !== before.search.computedInput));
    await recipe.advance(250); await recipe.observe("coalescer-current-query");
    recipe.assert("coalescer-current-query", !recipe.latest.search.pending);
    const painted = recipe.lastFrame?.frameEvidence?.search;
    if (!painted || !sameQuery(painted.computedQuery, recipe.latest.search.computedQuery) || selectionFingerprint(painted) !== selectionFingerprint(recipe.latest.search))
      await recipe.capture("coalescer-current-query-painted");
  }
}

async function sentenceTypingRecipe(recipe: SearchRecipe): Promise<void> {
  const scenario = recipe.schedule.recipe;
  if (scenario.kind !== "sentence") throw new EvaluationContractError("sentence-schedule-required");
  await recipe.prepare(scenario.fixture);
  if (recipe.prepared.suggestedInput !== scenario.input) throw new EvaluationContractError("compiled-sentence-corpus-mismatch");
  recipe.record("sentence-scenario", { fixture: scenario.fixture, input: scenario.input, profile: scenario.profile, entry: scenario.entry,
    inputRoute: "individual-gpui-key-events", syntheticProviderData: true });
  // A real matching catalogue is present before typing; later sources must not
  // steal selection or publish a retired query over these results.
  for (const source of ["scripts", "validation", "apps", "skills", "flow-roster"] as const)
    await finishSource(recipe, source, `sentence-seed-${source}`);
  const characters = Array.from(scenario.input);
  let expectedInput = ""; let keyEvents = 0; let releaseEvents = 0;
  const releasedWorkers = new Set<number>();
  const check = (before?: SearchObservation, changed = false): void => {
    const current = recipe.latest.search;
    recipe.assert("character-input-preserved", current.rawInput === expectedInput);
    recipe.assert("query-authority-preserved", current.pending === !sameQuery(current.query, current.computedQuery) &&
      (!before || (changed ? current.query.revision > before.query.revision : sameQuery(current.query, before.query))));
    if (!current.pending) {
      recipe.assert("character-input-preserved", current.computedInput === expectedInput);
      if (current.selectionArmed && current.selectionIntent.kind === "automaticAnchor")
        recipe.assert("completion-selection-policy", current.selectedSemanticId === current.selectionIntent.semanticId);
    }
  };
  const key = async (keyName: string, nextInput: string, text?: string, delay = 0): Promise<void> => {
    const before = recipe.latest.search; const changed = nextInput !== expectedInput;
    expectedInput = nextInput;
    await recipe.action({ type: "key", key: keyName, ...(text === undefined ? {} : { text }) });
    keyEvents++;
    if (delay) await recipe.advance(delay);
    await recipe.capture(`sentence-key-${keyEvents}`);
    check(before, changed);
    recipe.assert("natural-sentence-frames", recipe.lastFrame?.frameEvidence?.mode === "scheduled");
  };
  const settle = async (label: string): Promise<void> => {
    await recipe.advance(250); await recipe.observe(label);
    recipe.assert(`${label}:coalescer-current`, !recipe.latest.search.pending);
    const painted = recipe.lastFrame?.frameEvidence?.search;
    if (!painted || !sameQuery(painted.computedQuery, recipe.latest.search.computedQuery) ||
        selectionFingerprint(painted) !== selectionFingerprint(recipe.latest.search)) await recipe.capture(`${label}:painted`);
    check();
  };
  const release = async (runs: readonly SearchProviderRun[], label: string): Promise<void> => {
    if (!runs.length) return;
    const before = recipe.latest.search;
    for (const run of runs) if (run.kind === "worker") releasedWorkers.add(run.id);
    await recipe.release(runs, label); releaseEvents++;
    check(before);
    recipe.assertIntent("completion-selection-policy", before, recipe.latest.search);
  };
  const pending = (): SearchProviderRun[] => recipe.runs().filter(admissionReady);
  const partialRelease = async (label: string, reverse = false): Promise<void> => {
    const runs = pending(); if (reverse) runs.reverse();
    await release(runs.slice(0, 4), label);
  };
  // Caret movement redraws InputState, not the root search scene. Observe its
  // real scheduled frames; insertion/deletion still require the root-notified
  // capture above and must prove the exact middle-of-sentence edit below.
  const moveCursor = async (direction: "left" | "right"): Promise<void> => {
    const before = recipe.latest.search;
    const beforeFrame = identity(recipe.latest.state).frameGeneration!;
    const action = await recipe.action({ type: "key", key: direction });
    keyEvents++;
    await recipe.observe(`sentence-cursor-${direction}-${keyEvents}`);
    check(before);
    recipe.assert("sentence-cursor-dispatched", action.actionReceipt?.dispatchCompleted === true);
    recipe.assert("natural-sentence-frames", recipe.latest.state.frameEvidence.completedFrames.some((frame: Json) =>
      frame.mode === "scheduled" && frame.frame.target.frameGeneration > beforeFrame &&
      frame.localInputFocused === true && frame.nativeWindowActive === false));
  };
  let initialCharacters = 0;
  if (scenario.entry === "caret-prefix") {
    const prefixLength = characters.findIndex(character => character !== " ");
    if (prefixLength < 1) throw new EvaluationContractError("invalid-sentence-caret-prefix");
    const first = characters[prefixLength]!;
    await key(first, first, first);
    await moveCursor("left");
    for (let index = 0; index < prefixLength; index++) await key("space", " ".repeat(index + 1) + first, " ");
    await moveCursor("right");
    initialCharacters = prefixLength + 1;
    recipe.assert("sentence-caret-prefix-preserved", expectedInput === characters.slice(0, initialCharacters).join("") &&
      identity(recipe.latest.state).appViewVariant === "ScriptList");
    recipe.record("sentence-caret-prefix-entry", { prefixCodePoints: prefixLength, initialCharacters,
      input: expectedInput, targetIdentity: identity(recipe.latest.state) });
  }
  const midpoint = Math.floor(characters.length / 2);
  const firstPause = Math.max(1, characters.findIndex((character, index) => character === " " && index >= initialCharacters));
  let wordPauses = 0;
  let abaBefore: SearchQueryStamp | undefined;
  for (const [index, character] of characters.entries()) {
    if (index < initialCharacters) continue;
    const delay = scenario.profile === "paced" ? 30 + (index * 37 % 71) : 0;
    await key(character === " " ? "space" : character, expectedInput + character, character, delay);
    if (scenario.profile === "word-pauses" && character === " " && ++wordPauses % 2 === 0) {
      await settle(`sentence-word-pause-${index}`); await partialRelease(`sentence-word-arrival-${index}`);
    }
    if (scenario.profile === "reverse-completions" && index === firstPause) await settle("sentence-first-query");
    if (index === midpoint) {
      if (scenario.profile === "burst" || scenario.profile === "paced" || scenario.profile === "reverse-completions")
        await partialRelease("sentence-midword-arrival", scenario.profile === "reverse-completions");
      if (scenario.profile === "correction-aba") {
        await settle("sentence-before-typo"); abaBefore = { ...recipe.latest.search.query };
        const original = expectedInput;
        await key("x", original + "x", "x"); await settle("sentence-typo-query");
        await key("backspace", original); await settle("sentence-restored-query");
        recipe.assert("sentence-aba-revision-distinct", recipe.latest.search.query.revision > abaBefore.revision);
        await partialRelease("sentence-aba-old-completions", true);
      }
    }
  }
  await settle("sentence-final-input");
  if (scenario.profile === "cursor-edit") {
    for (let move = 0; move < 4; move++) await moveCursor("left");
    const original = expectedInput; const split = Array.from(original);
    const edited = [...split.slice(0, -4), "x", ...split.slice(-4)].join("");
    await key("x", edited, "x"); await settle("sentence-cursor-edited");
    await partialRelease("sentence-cursor-old-completions", true);
    await key("backspace", original); await settle("sentence-cursor-restored");
    for (let move = 0; move < 4; move++) await moveCursor("right");
  }
  let anchor: string | null = null;
  if (scenario.profile === "deliberate-selection") {
    await key("down", expectedInput);
    anchor = recipe.latest.search.selectedSemanticId;
    recipe.assert("sentence-deliberate-selection", anchor !== null && recipe.latest.search.selectionIntent.kind === "explicitAnchor");
  }
  for (let round = 0; round < 24; round++) {
    await recipe.observe(`sentence-final-drain-${round}`);
    const runs = pending();
    if (!runs.length) break;
    if (scenario.profile === "reverse-completions" || scenario.profile === "correction-aba") runs.reverse();
    await release(runs.slice(0, 8), `sentence-final-completions-${round}`);
  }
  recipe.assert("sentence-all-observed-admissions-drained", pending().length === 0);
  await settle("sentence-final-settled");
  const current = recipe.latest.search;
  recipe.assert("matching-source-results", current.committedRows.some(row => row.selectable && !row.stableKey.startsWith("fallback/")));
  recipe.assert("asynchronous-completions-observed", recipe.runs().filter(run => releasedWorkers.has(run.id) &&
    ["completed", "stale-discarded"].includes(run.state)).length >= 2);
  recipe.assert("character-input-preserved", expectedInput === scenario.input && keyEvents >= characters.length);
  if (anchor !== null) recipe.assert("sentence-deliberate-anchor-preserved", current.selectedSemanticId === anchor);
  recipe.record("sentence-complete", { fixture: scenario.fixture, profile: scenario.profile, keyEvents, releaseEvents,
    codePoints: characters.length, rawInput: current.rawInput, query: current.query, computedQuery: current.computedQuery,
    completedWorkers: recipe.runs().filter(run => releasedWorkers.has(run.id)).map(run => ({ id: run.id, source: run.source, state: run.state })) });
}

async function changingRowsRecipe(recipe: SearchRecipe): Promise<void> {
  for (const source of recipe.contract.providers) {
    await recipe.prepare(); await recipe.input(recipe.plan(source).input);
    const beforeKeys = new Set(recipe.latest.search.committedRows.map(row => row.semanticId));
    const initialRun = await awaitWork(recipe, source); const first = await recipe.release([initialRun], "initial-source-publication");
    const initialKeys = new Set(first.search.committedRows.map(row => row.stableKey));
    const candidate = first.search.committedRows.find(row => row.selectable && !beforeKeys.has(row.semanticId));
    if (!candidate) throw new EvaluationContractError("missing-capability:refresh-anchor");
    await recipe.anchor(candidate.selectableOrdinal!);
    const before = recipe.latest; const anchor = selected(before.search)!;
    const after = await followupSource(recipe, source, initialRun.id);
    const same = after.search.committedRows.find(row => row.stableKey === anchor.stableKey);
    if (recipe.contract.id === "selected-row-removal") {
      recipe.assert("anchor-removed-no-fallback", !same && after.search.reconciliationReason === "anchor_removed" && !selected(after.search));
      try { await recipe.action({ type: "key", key: "enter" }); }
      catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; }
      const entered = await recipe.observe("removed-target-enter");
      recipe.assert("removed-target-enter-inert", !selected(entered.search) && entered.search.dispatch === null);
      await recipe.refuse(() => recipe.client.act(recipe.target, { type: "select", semanticId: anchor.semanticId }, identity(before.state)), "old-target-refused");
    } else if (recipe.contract.id === "metadata-same-identity") {
      recipe.assert("content-revision-advanced", Boolean(same) && same!.contentFingerprint !== anchor.contentFingerprint && after.search.resultRevision > before.search.resultRevision);
      recipe.assert("new-content-painted", recipe.lastFrame?.frameEvidence?.paintBindings?.some((binding: Json) => binding.id === same?.semanticId && binding.metadata?.contentFingerprint === same?.contentFingerprint));
      recipe.assert("anchor-preserved", selected(after.search)?.stableKey === anchor.stableKey);
    } else {
      recipe.assert("real-list-replacement", first.search.committedRows.length === after.search.committedRows.length && after.search.committedRows.some(row => !initialKeys.has(row.stableKey)));
      recipe.assert("row-preview-footer-agree", paintBindingIssues(after.search, recipe.lastFrame?.frameEvidence?.paintBindings).length === 0 && (after.state.mainWindowPreflight.selectedResultKey ?? null) === (selected(after.search)?.stableKey ?? null));
    }
    recipe.record("row-change-source-covered", { source, initialRunId: initialRun.id, selectedBefore: anchor, selectedAfter: selected(after.search) });
  }
}

async function enterSource(recipe: SearchRecipe, source: SearchProvider): Promise<SearchSnapshot> {
  const plan = recipe.plan(source);
  if (recipe.latest.search.rawInput !== plan.input) return recipe.input(plan.input);
  return recipe.observe("source-input-current", true);
}
async function normalizeSources(recipe: SearchRecipe, sources: readonly SearchProvider[]): Promise<SearchSnapshot> {
  const canonical = [...sources].sort()[0]!;
  await enterSource(recipe, canonical);
  // Catalogue work can change these rows even when its fixture entry query differs.
  const active = sources.filter(source =>
    (recipe.plan(source).input === recipe.plan(canonical).input && recipe.plan(source).scope === recipe.plan(canonical).scope) ||
    recipe.latest.search.providers?.runs?.some((owner: Json) => owner.source === source && owner.queryBound === false));
  const complete = (snapshot: SearchSnapshot) => active.every(source => {
    const current = currentSourceResolution(snapshot.search, snapshot.state.searchProviders, source);
    return current ? current.status === "settled" : Boolean(currentSourceCache(snapshot.search, source));
  });
  for (let drain = 0; drain < 4; drain++) {
    const held: SearchProviderRun[] = [];
    for (const source of active) {
      const resolution = await resolveSource(recipe, source, `normalized-source:${source}:${drain}`, 0, true);
      if (resolution.status === "admitted") held.push(resolution.run);
    }
    if (!held.length) {
      const final = await recipe.observe("normalized-final-ranking", complete(recipe.latest));
      if (complete(final)) return final;
      continue;
    }
    await recipe.release(held, `normalized-current-source-publication-${drain}`);
  }
  throw new EvaluationContractError("native-source-normalization-not-settled");
}
async function anchorWhenEligible(recipe: SearchRecipe, phase: string): Promise<boolean> {
  const eligible = recipe.latest.search.committedRows.some(row => row.selectable);
  if (eligible) await recipe.anchor();
  recipe.record(phase, { requestedIntent: "explicitAnchor", status: eligible ? "executed" : "notApplicable", proof: eligible && recipe.latest.search.selectionIntent.kind === "explicitAnchor",
    cause: eligible ? null : "noEligibleSubject", query: recipe.latest.search.query, selectionArmed: recipe.latest.search.selectionArmed,
    actualIntent: recipe.latest.search.selectionIntent, selectedSemanticId: recipe.latest.search.selectedSemanticId });
  return eligible;
}
async function orderedSources(recipe: SearchRecipe, sources: readonly SearchProvider[], deliberate: boolean, sameTurn: boolean): Promise<SearchSnapshot> {
  const plans = sources.map(source => recipe.plan(source));
  const compatible = plans.every(plan => plan.input === plans[0]!.input && plan.scope === plans[0]!.scope);
  await enterSource(recipe, sources[0]!);
  let anchorReady = !deliberate || await anchorWhenEligible(recipe, "order-initial-intent");
  const before = recipe.latest;
  if (sameTurn && compatible) {
    const runs: SearchProviderRun[] = [];
    for (const source of sources) runs.push(await awaitWork(recipe, source));
    const completedBefore = recipe.frameStore.pool.frames.length;
    await recipe.release(runs, "atomic-source-admissions");
    recipe.record("same-turn-observed", { executionClass: "atomic-source-admission", runIds: runs.map(run => run.id),
      completedFrames: Array.from({ length: recipe.frameStore.pool.frames.length - completedBefore }, (_, index) => ({ frameRef: completedBefore + index })),
      sourceKinds: plans.map(plan => ({ source: plan.source, workKind: plan.workKind })) });
    recipe.assert("same-turn-completion", runs.length >= 2 && runs.every(run => TERMINAL.includes(recipe.runs().find(current => current.id === run.id)?.state ?? "")));
  } else {
    const retained: SearchProviderRun[] = [];
    for (const source of sources) {
      if (recipe.latest.search.rawInput !== recipe.plan(source).input) {
        const previous = recipe.latest.search;
        retained.push(...recipe.runs().filter(run => admissionReady(run) && sources.includes(run.source as SearchProvider)));
        await enterSource(recipe, source);
        recipe.assert(`scope-transition:${source}`, !sameQuery(previous.query, recipe.latest.search.query));
        anchorReady = !deliberate || await anchorWhenEligible(recipe, `order-scope-intent:${source}`);
      }
      if (!sameTurn) {
        await finishSource(recipe, source, `ordered-source:${source}`);
        if (!anchorReady) anchorReady = await anchorWhenEligible(recipe, `order-first-eligible:${source}`);
      }
      else retained.push(...recipe.runs(source).filter(admissionReady));
    }
    if (sameTurn) {
      const retainedIds = new Set(retained.map(run => run.id));
      const unique = recipe.runs().filter(run => retainedIds.has(run.id) && admissionReady(run));
      if (!unique.length) throw new EvaluationContractError("missing-capability:scope-retirement-admissions");
      if (recipe.schedule.structuralNotApplicable) {
        recipe.assert("single-physical-owner-observed", unique.length === 1 && sources.some(source => !recipe.runs(source).some(run => run.kind === "worker")));
        recipe.record("single-owner-queued-desired", { requestedFactor: "same-turn", availableAdmissions: unique, requestedSources: sources, sourcePlans: plans, query: recipe.latest.search.query });
      }
      await recipe.release(unique, recipe.schedule.structuralNotApplicable ? "single-owner-sequential-drain" : "atomic-incompatible-scope-completions");
      for (const source of sources) if (!recipe.runs(source).some(run => run.kind !== "synchronousRead" && TERMINAL.includes(run.state))) {
        const next = await awaitWork(recipe, source);
        recipe.record("bounded-latest-desired-drain", { executionClass: "serial-native-owner-after-atomic-admission", source, provider: next });
        await recipe.release([next], `queued-source:${source}`);
      }
      if (!recipe.schedule.structuralNotApplicable) recipe.assert("same-turn-completion", unique.length >= 2 && unique.every(run => TERMINAL.includes(recipe.runs().find(current => current.id === run.id)?.state ?? "")));
      recipe.record("incompatible-scopes", { executionClass: "scope-retirement-and-cache-policy", sourcePlans: plans,
        terminal: unique.map(run => recipe.runs().find(current => current.id === run.id)) });
    }
  }
  if (!anchorReady) await anchorWhenEligible(recipe, "order-first-eligible-after-batch");
  recipe.assert("every-intermediate-intent", recipe.result.assertions.filter(assertion => assertion.id.endsWith(":selection-policy")).every(assertion => assertion.pass));
  recipe.assert("ordered-source-admissions-observed", sources.every(source => recipe.runs(source).length > 0));
  const final = await normalizeSources(recipe, sources);
  const ranking = rankingFacts(final.search);
  recipe.record("source-order-terminal", { initialQuery: before.search.query, finalQuery: final.search.query,
    compatible, sourceOrder: sources, intent: deliberate ? "deliberate-when-eligible" : "automatic", displayUnixMs: final.state.searchProviders.displayUnixMs, fingerprint: digest(ranking), ranking });
  return final;
}
async function retiredOwnerRecipe(recipe: SearchRecipe, source: SearchProvider = "tabs"): Promise<void> {
  const oldTarget = recipe.target; const before = recipe.latest; const frame = recipe.lastFrame!;
  const run = await awaitWork(recipe, source);
  if (run.kind === "worker") {
    await recipe.control({ operation: "release", runIds: [run.id] });
    let pending = recipe.runs(source).find(current => current.id === run.id);
    for (let tick = 0; tick < 32 && !(pending?.payloadPrepared && pending.pendingDelivery); tick++) {
      await recipe.advance(16); await recipe.observe(`retirement-payload-prepared-${tick}`);
      pending = recipe.runs(source).find(current => current.id === run.id);
    }
    if (!pending?.payloadPrepared || !pending.pendingDelivery || pending.state !== "released") throw new EvaluationContractError("missing-capability:held-native-delivery-before-retirement");
    recipe.record("native-delivery-held-before-retirement", { provider: pending });
  }
  await recipe.unmount();
  recipe.runtime.target = await recipe.request(() => recipe.client.mount(SEARCH_FIXTURE_ID));
  await recipe.prepare("all-providers");
  const after = recipe.latest;
  recipe.assert("new-window-lifetime", before.search.query.lifetime !== after.search.query.lifetime &&
    (oldTarget.id !== recipe.target.id || oldTarget.generation !== recipe.target.generation));
  await recipe.refuse(() => recipe.client.act(oldTarget, { type: "key", key: "down" }, identity(before.state)), "retired-action-refused");
  await recipe.refuse(() => recipe.client.design({ operation: "captureFrame", target: oldTarget, includeImage: false,
    scheduled: { expected: frame.frame.target, afterFrameGeneration: frame.frame.target.frameGeneration, afterNotificationEpoch: frame.frameEvidence!.notificationEpoch } }), "retired-frame-refused");
  await recipe.refuse(() => recipe.control({ operation: "release", runIds: [run.id] }), "retired-provider-refused");
  await recipe.advance(64); await recipe.observe("retired-native-delivery");
  let retired: SearchProviderRun | undefined = recipe.latest.state.searchProviders.retiredGate?.runs.find((current: SearchProviderRun) => current.id === run.id);
  for (let tick = 0; tick < 8 && !TERMINAL.includes(retired?.state ?? ""); tick++) {
    await recipe.advance(16); await recipe.observe(`retired-native-receiver-${tick}`);
    retired = recipe.latest.state.searchProviders.retiredGate?.runs.find((current: SearchProviderRun) => current.id === run.id);
  }
  const retiredProof = run.kind === "worker" ? retired?.state === "stale-discarded" && retired.deliveryAttempted === true && retired.pendingDelivery === false :
    retired?.state === "cancelled" && retired.admissionApplied === false && retired.outcome == null &&
    !recipe.latest.state.searchProviders.retiredGate?.runs.some((current: SearchProviderRun) => current.originAdmissionId === run.id);
  recipe.assert("retired-native-owner-proof", retiredProof);
  recipe.record("timing:after-owner-retirement:completion", { executionClass: run.kind === "worker" ? "actual-native-delivery-to-retired-owner" : "synchronous-admission-retired-without-read", provider: retired });
  const unchanged = recipe.latest;
  recipe.assert("old-provider-action-frame-refused", retiredProof && recipe.result.assertions.filter(assertion => assertion.id.startsWith("retired-") && assertion.id.endsWith("-refused")).length === 3 &&
    recipe.result.assertions.filter(assertion => assertion.id.startsWith("retired-") && assertion.id.endsWith("-refused")).every(assertion => assertion.pass) && selectionFingerprint(unchanged.search) === selectionFingerprint(after.search));
  await recipe.input(recipe.plan(source).input);
}
async function timingRecipe(recipe: SearchRecipe): Promise<void> {
  if (recipe.schedule.recipe.kind !== "timing") throw new EvaluationContractError("timing-schedule-required");
  const { timing } = recipe.schedule.recipe; const source = recipe.schedule.providers[0]!;
  const scenario = timing === "after-owner-retirement" ? "owner-retirement" : timing === "after-deliberate-selection" ? "metadata" : recipe.contract.fixture;
  await recipe.prepare(scenario);
  await recipe.input(recipe.plan(source).input);
  let run = await awaitWork(recipe, source);
  if (timing === "before-initial-commit" && recipe.plan(source).input === "") {
    if (!recipe.prepared.suggestedInput) throw new EvaluationContractError("missing-capability:nonstructural-query-baseline");
    await recipe.input(recipe.prepared.suggestedInput);
  }
  const before = recipe.latest;
  if (timing === "before-initial-commit") {
    await recipe.advance(250); await recipe.observe("old-native-work-parked");
    await recipe.input(`${before.search.rawInput}x`, "gpui-text", false);
    recipe.assert("before-initial-commit-observed", recipe.latest.search.pending && !sameQuery(before.search.query, recipe.latest.search.query));
    recipe.record("before-initial-commit-admission", { executionClass: run.kind === "sourceChange" ? "source-change-before-new-query-commit" : "old-native-work-before-new-query-commit",
      observedRun: run, computedQuery: recipe.latest.search.computedQuery, newQuery: recipe.latest.search.query });
  } else if (timing === "after-deliberate-selection") {
    run = await stageSourceRefresh(recipe, source, "timing-deliberate"); await recipe.anchor();
  }
  else if (timing === "after-superseding-query") await recipe.input(`${recipe.plan(source).input}x`, "gpui-text");
  else if (timing === "after-owner-retirement") await retiredOwnerRecipe(recipe, source);
  if (timing !== "after-owner-retirement") await recipe.release([run], `timing:${timing}`);
  if (timing === "before-initial-commit") { await recipe.advance(250); await recipe.observe("initial-coalescer-completed"); }
  const final = await normalizeSources(recipe, [source]);
  const fingerprint = candidateFingerprint(final.search);
  await recipe.prepare(timing === "after-owner-retirement" ? recipe.contract.fixture : scenario); await enterSource(recipe, source);
  if (timing === "after-deliberate-selection") await recipe.release([await stageSourceRefresh(recipe, source, "timing-reference")], "timing-reference-publication");
  else await finishSource(recipe, source, "timing-reference-publication");
  const reference = await normalizeSources(recipe, [source]);
  recipe.assert("final-candidates-equal", fingerprint === candidateFingerprint(reference.search));
  recipe.assert("all-provider-timings", recipe.evidence.phases.some(phase => phase.id === `timing:${timing}:completion`));
  recipe.assert("every-intermediate-intent", recipe.result.assertions.filter(assertion => assertion.id.endsWith(":selection-policy")).every(assertion => assertion.pass));
}
async function providerOrdersRecipe(recipe: SearchRecipe): Promise<void> {
  if (recipe.schedule.recipe.kind === "timing") return timingRecipe(recipe);
  if (recipe.schedule.recipe.kind === "cohort") return cohortRecipe(recipe);
  const primary = recipe.schedule.recipe.kind === "primary";
  for (const deliberate of [false, true]) {
    await recipe.prepare();
    const sources = recipe.schedule.providers;
    const final = await orderedSources(recipe, sources, deliberate, recipe.schedule.recipe.kind === "same-turn");
    if (primary) {
      const fingerprint = candidateFingerprint(final.search);
      await recipe.prepare();
      const reversed = await orderedSources(recipe, [...sources].reverse(), deliberate, false);
      recipe.assert("final-candidates-equal", fingerprint === candidateFingerprint(reversed.search));
    } else if (recipe.schedule.structuralNotApplicable) {
      recipe.record("serial-owner-proof-not-atomic", { deliberate, fingerprint: rankingFingerprint(final.search), sourceOrder: sources });
    } else {
      const canonical = [...sources].sort();
      const atomicEligible = !searchAssertionApplicability(recipe.contract, { kind: "same-turn" }, canonical).structuralNotApplicable;
      recipe.evidence.orderComparisons.push({ key: `pair:${canonical.join("+")}:${deliberate ? "deliberate-when-eligible" : "automatic"}`,
        order: recipe.schedule.recipe.kind === "same-turn" ? "same-turn" : sources.join("-then-"), fingerprint: candidateFingerprint(final.search),
        expectedOrders: [canonical.join("-then-"), [...canonical].reverse().join("-then-"), ...(atomicEligible ? ["same-turn"] : [])] });
    }
  }
}
async function cohortRecipe(recipe: SearchRecipe): Promise<void> {
  if (recipe.schedule.recipe.kind !== "cohort") throw new EvaluationContractError("cohort-schedule-required");
  const { cohort, order } = recipe.schedule.recipe;
  const events: Record<string, SearchProvider> = { "tab-hoist": "tabs", "brain-replacement": "brain-semantic", "files-handoff": "files",
    "catalogue-arrival": "scripts", "passive-arrival": "notes", "budget-competition": "history",
    "selected-removal": "tabs", replacement: "notes", "other-source-arrival": "todos",
    "source-change": "spine", "scope-change": "directory", "compatible-work-reuse": "files" };
  for (const deliberate of [false, true]) {
    await recipe.prepare(cohort === 1 ? "passive-budget" : cohort === 2 ? "cohort-removal-replacement" : "all-providers");
    await enterSource(recipe, recipe.schedule.providers[0]!);
    let anchorReady = !deliberate;
    if (cohort === 2) {
      const before = new Set(recipe.latest.search.committedRows.map(row => row.semanticId));
      await finishSource(recipe, "tabs", "cohort-initial-tabs");
      const tab = recipe.latest.search.committedRows.find(row => row.selectable && !before.has(row.semanticId));
      await finishSource(recipe, "notes", "cohort-initial-notes");
      if (!tab) throw new EvaluationContractError("missing-capability:cohort-removal-subject");
      if (deliberate) await recipe.anchor(recipe.latest.search.committedRows.find(row => row.semanticId === tab.semanticId)!.selectableOrdinal!);
      anchorReady = true;
      await recipe.advance(16); await recipe.observe("cohort-refreshes-admitted");
    } else if (deliberate) anchorReady = await anchorWhenEligible(recipe, "cohort-initial-intent");
    for (const event of order) {
      const source = events[event]!;
      const previousQuery = recipe.latest.search.query;
      if (event === "files-handoff" || event === "compatible-work-reuse") {
        await enterSource(recipe, "files"); const work = await awaitWork(recipe, "files");
        await recipe.input(`files: ${recipe.plan("files").input}`);
        recipe.assert(`${event}:same-native-work`, (await awaitWork(recipe, "files")).id === work.id);
      } else await enterSource(recipe, source);
      if (deliberate && !sameQuery(previousQuery, recipe.latest.search.query)) anchorReady = await anchorWhenEligible(recipe, `cohort-scope-intent:${event}`);
      await finishSource(recipe, source, `cohort:${event}`);
      if (!anchorReady) anchorReady = await anchorWhenEligible(recipe, `cohort-first-eligible:${event}`);
      recipe.record(`cohort-event:${event}`, { source, query: recipe.latest.search.query, publication: recipe.latest.search.publication,
        selection: recipe.latest.search.selectionIntent, fingerprint: rankingFingerprint(recipe.latest.search) });
    }
    const final = await normalizeSources(recipe, recipe.schedule.providers);
    const otherOrders = searchContractSpec().schedules.filter(schedule => schedule.recipe.kind === "cohort" && schedule.recipe.cohort === cohort)
      .map(schedule => schedule.recipe.kind === "cohort" ? schedule.recipe.order.join("-then-") : "");
    const ranking = rankingFacts(final.search); const fingerprint = candidateFingerprint(final.search);
    recipe.record("cohort-order-terminal", { cohort, order, intent: deliberate ? "deliberate-when-eligible" : "automatic", fingerprint, ranking });
    recipe.evidence.orderComparisons.push({ key: `cohort:${cohort}:${deliberate ? "deliberate-when-eligible" : "automatic"}`, order: order.join("-then-"),
      fingerprint, expectedOrders: otherOrders });
  }
  recipe.assert("every-intermediate-intent", recipe.result.assertions.filter(assertion => assertion.id.endsWith(":selection-policy")).every(assertion => assertion.pass));
  recipe.assert("cohort-events-observed", recipe.schedule.recipe.order.every(event => recipe.evidence.phases.some(phase => phase.id === `cohort-event:${event}`)));
}

async function terminalRecipe(recipe: SearchRecipe): Promise<void> {
  const outcomes: readonly SearchTerminalOutcome[] = recipe.schedule.recipe.kind === "terminal" ? [recipe.schedule.recipe.outcome] : ["error", "unavailable", "disconnect"];
  if (!recipe.schedule.terminalIntents) throw new EvaluationContractError("missing-terminal-intent-contract");
  recipe.record("terminal-intent-applicability", recipe.schedule.terminalIntents);
  for (const source of recipe.schedule.providers) for (const outcome of outcomes) for (const intent of recipe.schedule.terminalIntents.required) {
    const deliberate = intent === "explicitAnchor"; let terminal: SearchProviderRun | undefined;
    await recipe.prepare(outcome); await enterSource(recipe, source);
    const run = await stageSourceRefresh(recipe, source, `terminal:${source}:${outcome}`);
    if (deliberate) await recipe.anchor();
    const before = recipe.latest; const plan = recipe.plan(source);
    recipe.assert("terminal-observed-intent", before.search.selectionIntent.kind === intent);
    if (plan.workKind === "synchronous" && outcome === "disconnect") {
      await recipe.refuse(() => recipe.control({ operation: "release", runIds: [run.id] }), "synchronous-disconnect-refused", ["synchronous_source_has_no_worker"]);
      const after = await recipe.observe("synchronous-no-worker");
      terminal = recipe.runs(source).find(current => current.id === run.id);
      recipe.assert("error-unavailable-disconnect-distinct", terminal?.kind === "sourceChange" && terminal.payloadPhase === 1 && terminal.outcome == null &&
        terminal.capabilityRefusal === "synchronous_source_has_no_worker" && terminal.admissionApplied === false);
      recipe.assert("last-good-or-empty-policy", rankingFingerprint(before.search) === rankingFingerprint(after.search));
      recipe.record("terminal-capability-negative", { source, requestedOutcome: outcome, deliberate, executionClass: "unsupported-operation",
        beforeFingerprint: rankingFingerprint(before.search), afterFingerprint: rankingFingerprint(after.search), provider: recipe.runs(source).find(current => current.id === run.id) });
    } else {
      const after = await recipe.release([run], `terminal:${source}:${outcome}`);
      terminal = plan.workKind === "synchronous" ? recipe.runs(source).find(current => current.kind === "synchronousRead" && current.originAdmissionId === run.id) : recipe.runs(source).find(current => current.id === run.id);
      recipe.assert("error-unavailable-disconnect-distinct", terminal?.outcome === (outcome === "disconnect" ? "disconnected" : outcome) &&
        terminal?.state === (outcome === "unavailable" ? "unavailable" : "failed") && terminal.payloadPhase === 1 && terminal.kind === (plan.workKind === "synchronous" ? "synchronousRead" : "worker"));
      const previousSubjects = before.search.committedRows.filter(row => row.selectable);
      const currentSubjects = after.search.committedRows.filter(row => row.selectable);
      recipe.assert("last-good-or-empty-policy", !currentSubjects.length || currentSubjects.length === previousSubjects.length && currentSubjects.every((row, index) =>
        row.stableKey === previousSubjects[index]!.stableKey && row.contentFingerprint === previousSubjects[index]!.contentFingerprint && row.activatable === previousSubjects[index]!.activatable));
      recipe.record("terminal-outcome", { deliberate, executionClass: plan.workKind === "synchronous" ? "synchronous-owner-read-error" : "native-provider-terminal", provider: terminal });
    }
    recipe.assert("no-source-broadening", recipe.latest.search.rawInput === plan.input && sameQuery(before.search.query, recipe.latest.search.query));
    if (!terminal) throw new EvaluationContractError("missing-native-terminal-receipt");
    const observedIntent = before.search.selectionIntent.kind === "explicitAnchor" ? "explicitAnchor" : "automatic";
    (recipe.result.terminalReceipts ??= []).push({ source, requestedOutcome: outcome, intent: observedIntent,
      query: before.search.query, selectionArmed: before.search.selectionArmed, selectedSemanticId: before.search.selectedSemanticId, provider: terminal });
    recipe.record("terminal-proof-link", { receiptIndex: recipe.result.terminalReceipts.length - 1, source, requestedOutcome: outcome, intent: observedIntent, runId: terminal.id });
  }
}

async function directoryRecipe(recipe: SearchRecipe): Promise<void> {
  await recipe.prepare(); const directory = recipe.plan("directory").input;
  await recipe.input(`${directory}exa`, "gpui-text");
  const original = await awaitWork(recipe, "directory"); const firstScope = recipe.latest.search.query;
  await recipe.input(`${directory}.`, "gpui-text");
  const scopeChanged = recipe.latest.search.query.scopeRevision > firstScope.scopeRevision;
  recipe.assert("fragment-scope-correct", scopeChanged);
  if (!scopeChanged) throw new EvaluationContractError("directory-hidden-scope-transition-required");
  await recipe.release([original], "retired-directory-fragment");
  const hidden = await awaitWork(recipe, "directory", original.id);
  await recipe.input(directory, "gpui-text");
  const current = recipe.latest;
  await recipe.release([hidden], "retired-hidden-directory");
  recipe.assert("stale-directory-rejected", [original, hidden].every(run => ["stale-discarded", "cancelled"].includes(recipe.runs().find(item => item.id === run.id)?.state ?? "")));
  recipe.assert("hidden-option-rejected", sameQuery(current.search.query, recipe.latest.search.query) &&
    rankingFingerprint(current.search) === rankingFingerprint(recipe.latest.search) &&
    current.search.selectedSemanticId === recipe.latest.search.selectedSemanticId &&
    current.search.selectionArmed === recipe.latest.search.selectionArmed &&
    JSON.stringify(current.search.selectionIntent) === JSON.stringify(recipe.latest.search.selectionIntent) &&
    viewportFingerprint(current) === viewportFingerprint(recipe.latest));
  const active = await awaitWork(recipe, "directory", hidden.id);
  await recipe.release([active], "current-directory");
  const files = recipe.latest.state.mainWindowPreflight.visibleResults.filter((row: Json) => row.role === "rootFile");
  recipe.assert("hidden-option-rejected", files.length > 0 && files.every((row: Json) => typeof row.stableKey === "string" && !row.stableKey.slice(row.stableKey.lastIndexOf("/") + 1).startsWith(".")));
  recipe.assert("selection-valid", searchObservationIssues(recipe.latest.search, recipe.latest.elements).length === 0);
}
async function brainRecipe(recipe: SearchRecipe): Promise<void> {
  for (const deliberate of [false, true]) {
    await recipe.prepare(); await enterSource(recipe, "brain-lexical");
    await finishSource(recipe, "brain-lexical", "lexical-source-change");
    if (deliberate) await recipe.anchor();
    const before = recipe.latest;
    const after = await finishSource(recipe, "brain-semantic", "semantic-source-publication");
    recipe.assert("different-semantic-batch-consumed", rankingFingerprint(before.search) !== rankingFingerprint(after.search) && after.search.resultRevision > before.search.resultRevision);
    recipe.assertIntent("selection-policy", before.search, after.search);
  }
}
async function passiveBudgetRecipe(recipe: SearchRecipe): Promise<void> {
  const fingerprints: string[] = [];
  for (const deliberate of [false, true]) for (const reverse of [false, true]) {
    const phaseStart = recipe.evidence.phases.length;
    await recipe.prepare(); await enterSource(recipe, "notes");
    const oldRows = new Set(recipe.latest.search.committedRows.map(row => row.semanticId));
    await finishSource(recipe, "notes", "initial-passive-notes");
    const candidates = recipe.latest.search.committedRows.filter(row => row.selectable && !oldRows.has(row.semanticId));
    if (!candidates.length) throw new EvaluationContractError("missing-capability:passive-budget-anchor");
    if (deliberate) await recipe.anchor(candidates.at(-1)!.selectableOrdinal!);
    const anchor = selected(recipe.latest.search)!;
    const order = recipe.contract.providers.filter(source => source !== "notes");
    for (const source of reverse ? [...order].reverse() : order) await finishSource(recipe, source, `budget-arrival:${source}`);
    fingerprints.push(candidateFingerprint(recipe.latest.search));
    const retained = recipe.latest.search.committedRows.some(row => row.stableKey === anchor.stableKey && row.selectable);
    if (deliberate) recipe.assert("cap-removal-explicit", !retained && !selected(recipe.latest.search) &&
      recipe.evidence.phases.slice(phaseStart).some((phase, index) => (searchObservationOrigin(recipe.evidence.phases, phaseStart + index) ?? phase).reconciliationReason === "anchor_removed"));
    else recipe.assert("budget-automatic-anchor", retained ? selected(recipe.latest.search)?.stableKey === anchor.stableKey : !selected(recipe.latest.search));
    recipe.record("source-budget-terminal", { deliberate, reverse, fingerprint: fingerprints.at(-1), preflight: recipe.latest.state.mainWindowPreflight.rootPassiveFrame });
  }
  recipe.assert("source-budget-deterministic", fingerprints.length === 4 && fingerprints.every(fingerprint => fingerprint === fingerprints[0]));
}
async function emptyRecipe(recipe: SearchRecipe): Promise<void> {
  await recipe.prepare(); await recipe.input(recipe.plan("windows").input);
  await recipe.release([await awaitWork(recipe, "windows"), await awaitWork(recipe, "icons")], "empty-source-terminal");
  const before = recipe.latest;
  recipe.assert("no-selection", before.search.selectedSemanticId === null && before.search.selectedOrdinal === null);
  recipe.assert("no-marker", !(recipe.lastFrame?.layout.components ?? []).some((node: Json) => node.name?.endsWith(":selection-marker")));
  recipe.assert("no-preview", !(recipe.lastFrame?.frameEvidence?.paintBindings ?? []).some((binding: Json) => binding.kind === "mainSearchPreview"));
  const effect = await recipe.action({ type: "key", key: "enter" });
  const after = await recipe.observe("empty-enter");
  recipe.assert("no-row-submission", effect.actionReceipt?.effect?.kind === "noOp" && after.state.mainWindowPreflight.enterAction === null && selectionFingerprint(before.search) === selectionFingerprint(after.search));
}
async function sourceUnarmedRecipe(recipe: SearchRecipe): Promise<void> {
  await recipe.prepare(); await recipe.input(recipe.plan("spine").input, "gpui-keyboard");
  const run = await awaitWork(recipe, "spine");
  await recipe.control({ operation: "release", runIds: [run.id] }); await recipe.advance(25);
  await recipe.capture("source-recents-unarmed");
  recipe.assert("empty-source-unarmed", recipe.latest.search.selectedSemanticId === null && recipe.latest.search.committedRows.some(row => row.selectable) && recipe.latest.state.mainWindowPreflight.enterAction === null);
  await recipe.action({ type: "key", key: "down" }); await recipe.capture("source-first-down");
  recipe.assert("first-down-chooses-first", recipe.latest.search.selectedOrdinal === 0 && recipe.latest.search.selectionIntent.kind === "explicitAnchor");
}
async function calculatorRecipe(recipe: SearchRecipe): Promise<void> {
  await recipe.prepare("eligibility"); await recipe.input(recipe.prepared.suggestedInput, "gpui-keyboard");
  const oldValidation = await awaitWork(recipe, "validation");
  await recipe.release([await awaitWork(recipe, "scripts")], "eligibility-script-corpus");
  await recipe.release([oldValidation], "eligibility-retired-validation-corpus");
  recipe.assert("validation-rejects-old-corpus", recipe.runs("validation").find(run => run.id === oldValidation.id)?.state === "stale-discarded");
  const validation = await awaitWork(recipe, "validation", oldValidation.id);
  recipe.assert("validation-restarts-for-current-corpus", validation.id !== oldValidation.id && validation.generation > oldValidation.generation);
  await recipe.release([validation], "eligibility-current-validation-corpus");
  recipe.assert("validation-accepts-current-corpus", recipe.runs("validation").find(run => run.id === validation.id)?.state === "completed");
  const initial = recipe.latest;
  const count = initial.search.committedRows.filter(row => row.selectable).length;
  const headers = (await recipe.request(() => recipe.client.query(recipe.target, "elements", { includeHeaders: true }))).elements
    .filter((node: Json) => node.kind === "sectionHeader");
  recipe.assert("headers-inert", headers.length > 0 && headers.every((node: Json) => node.type === "panel" && node.role === "sectionHeader" &&
    node.selectable === false && node.selected === false && node.index == null) && collectorRows(initial.elements).filter(row => row.selectable !== false).length === count);
  recipe.record("observed-section-headers", { elements: headers });
  for (let index = 1; index < count; index++) {
    await recipe.action({ type: "key", key: "down" }); await recipe.capture(`eligibility-navigation-${index}`);
    recipe.assert("navigation-count-preflight-paint-submit-agree", recipe.latest.search.selectedOrdinal === index &&
      recipe.latest.state.mainWindowPreflight.selectedResultKey === selected(recipe.latest.search)?.stableKey && selected(recipe.latest.search)?.selectable === true);
  }
  await recipe.prepare("eligibility-portal");
  await recipe.input(recipe.prepared.suggestedInput, "gpui-keyboard");
  await recipe.release([await awaitWork(recipe, "scripts")], "reserved-slot-portal");
  const chrome = await recipe.request(() => recipe.client.query(recipe.target, "elements", { includeHeaders: true }));
  const slot = chrome.elements.find((node: Json) => node.semanticId === "main-list-reserved-slot" && node.kind === "reservedSectionSlot");
  const slotPaint = recipe.lastFrame?.frameEvidence?.paintBindings?.find((binding: Json) => binding.kind === "mainSearchReservedSlot" && binding.id === "main-list-reserved-slot");
  if (!slot || typeof slot.value !== "string" || !slotPaint) throw new EvaluationContractError("missing-capability:actual-reserved-section-slot");
  const slotMetadata: Json = JSON.parse(slot.value);
  const expectedSlot = { groupedIndex: 0, selectableOrdinal: null, selectable: false, activatable: false, selected: false };
  recipe.assert("reserved-slots-inert", slot.type === "panel" && slot.role === "presentation" && slot.selectable === false && slot.selected === false &&
    slot.index == null && slot.actionDisabled === "presentationOnly" &&
    Object.entries(expectedSlot).every(([key, value]) => slotMetadata?.[key] === value && slotPaint.metadata?.[key] === value) && slotPaint.visibleBounds?.width > 0 && slotPaint.visibleBounds?.height > 0 &&
    !recipe.latest.search.committedRows.some(row => row.semanticId === slot.semanticId) && selected(recipe.latest.search)?.groupedIndex === 1 && recipe.latest.search.selectedOrdinal === 0);
  recipe.record("observed-reserved-section-slot", { element: slot, paint: slotPaint });
  const slotSelection = selectionFingerprint(recipe.latest.search); let slotRefusal: string | undefined;
  try { await recipe.action({ type: "select", semanticId: slot.semanticId }); }
  catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; slotRefusal = error.code; }
  await recipe.observe("reserved-slot-action-refused");
  recipe.assert("reserved-slots-inert", !!slotRefusal && selectionFingerprint(recipe.latest.search) === slotSelection);
  await recipe.action({ type: "key", key: "up" }); await recipe.observe("reserved-slot-navigation-skipped");
  recipe.assert("navigation-count-preflight-paint-submit-agree", recipe.latest.search.selectedOrdinal === 0 && selected(recipe.latest.search)?.groupedIndex === 1 &&
    selected(recipe.latest.search)?.activatable === true && recipe.latest.state.mainWindowPreflight.selectedResultKey === selected(recipe.latest.search)?.stableKey);
  await recipe.action({ type: "key", key: "escape" });
  const portalState = await recipe.inspect("reserved-slot-portal-cancelled");
  if (identity(portalState).appViewVariant !== "AgentChatView" || portalState.windowVisible !== false)
    throw new EvaluationContractError("context-portal-return-host-required");
  // Chat dismissal is not a search contract; restore the launcher through the owned fixture.
  await recipe.prepare("eligibility");
  await recipe.input("2+2", "gpui-keyboard");
  const calculator = selected(recipe.latest.search);
  recipe.assert("calculator-subject", calculator?.subjectKind === "calculator" && recipe.latest.state.mainWindowPreflight.enterAction?.kind === "copyCalculator");
  recipe.assert("navigation-count-preflight-paint-submit-agree", recipe.latest.state.mainWindowPreflight.selectedResultKey === calculator?.stableKey &&
    paintBindingIssues(recipe.latest.search, recipe.lastFrame?.frameEvidence?.paintBindings).length === 0);
  const before = recipe.latest;
  const copiedText = before.state.mainWindowPreflight.enterAction?.subject;
  if (typeof copiedText !== "string") throw new EvaluationContractError("missing-capability:calculator-copy-value");
  let refusal: string | undefined; let actionReceipt: Json | undefined;
  try { actionReceipt = (await recipe.action({ type: "key", key: "enter" })).actionReceipt; }
  catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; refusal = error.code; }
  const completed = await recipe.observe("calculator-effect-completion");
  const dispatch = completed.search.dispatch;
  recipe.assert("navigation-count-preflight-paint-submit-agree", dispatchBindingIssues({ ...before.search, dispatch }).length === 0 &&
    dispatch?.status === "completed" && dispatch.reason === null && actionReceipt?.dispatchCompleted === true && !refusal);
  for (const issue of copySinkIssues(before.state.copySink, completed.state.copySink, copiedText)) recipe.assert(`calculator:${issue}`, false);
  recipe.record("calculator-effect-completion", { dispatch, actionReceipt, refusal, copySinkBefore: before.state.copySink,
    copySink: completed.state.copySink, preflight: before.state.mainWindowPreflight.enterAction });
}
function viewportFingerprint(snapshot: SearchSnapshot): string {
  const scroll = snapshot.state.mainListScroll;
  if (!scroll || !Number.isFinite(scroll.scrollTopItem) || !Number.isFinite(scroll.scrollTopOffsetPx)) throw new EvaluationContractError("missing-capability:viewport-position");
  return digest({ item: scroll.scrollTopItem, offset: scroll.scrollTopOffsetPx, intent: snapshot.search.viewportIntent });
}
async function scrollRecipe(recipe: SearchRecipe): Promise<void> {
  for (const deliberate of recipe.contract.selectionIntent === "both" ? [false, true] : [true]) {
    await recipe.prepare(); await recipe.input(recipe.plan("tabs").input);
    const run = await awaitWork(recipe, "tabs"); await recipe.release([run], "deep-list-initial");
    if (deliberate) await recipe.anchor(Math.min(8, recipe.latest.search.committedRows.filter(row => row.selectable).length - 1));
    const before = recipe.latest;
    const scroll = async (scrollbar: boolean) => {
      const selectionBefore = recipe.latest.search;
      const frame = recipe.lastFrame!;
      const binding = frame.frameEvidence?.paintBindings?.find((binding: Json) => scrollbar ? binding.kind === "scrollbar" && binding.id === "launcher-main-scrollbar:vertical:track" : binding.kind === "mainSearchRow" && binding.visibleBounds?.width > 0 && binding.visibleBounds?.height > 0);
      const bounds = binding?.visibleBounds;
      if (!bounds || bounds.width <= 0 || bounds.height <= 0) throw new EvaluationContractError("missing-capability:scroll-paint-geometry");
      const x = bounds.x + bounds.width / 2; const y = bounds.y + bounds.height / 2;
      if (scrollbar) {
        const thumb = frame.frameEvidence?.paintBindings?.find((binding: Json) => binding.kind === "scrollbar" && binding.id === "launcher-main-scrollbar:vertical:thumb");
        if (!thumb?.visibleBounds) throw new EvaluationContractError("missing-capability:scrollbar-thumb-paint");
        const targetY = thumb.visibleBounds.y + thumb.visibleBounds.height < bounds.y + bounds.height - 4 ? bounds.y + bounds.height - 2 : bounds.y + 2;
        await recipe.action({ type: "gpuiEvent", frame: frame.frame, event: { type: "mouseClick", button: "left", x, y: targetY } }, frame.frame.target);
      }
      else await recipe.action({ type: "gpuiEvent", frame: frame.frame, event: { type: "scrollWheel", x, y, deltaX: 0, deltaY: -180, phase: "moved" } }, frame.frame.target);
      await recipe.capture(scrollbar ? "scrollbar-scrolled" : "wheel-scrolled");
      recipe.assert("no-hover-or-selection-change", recipe.latest.search.selectedSemanticId === selectionBefore.selectedSemanticId &&
        recipe.latest.search.selectionRevision === selectionBefore.selectionRevision && recipe.latest.state.mainListScroll.hoveredIndex == null &&
        recipe.latest.state.mainListScroll.hoveredSemanticId == null && recipe.latest.state.mainListScroll.hoverSuppressedUntilPointerMove === true);
    };
    await scroll(false); const wheel = recipe.latest; await scroll(true); const moved = recipe.latest;
    recipe.assert("wheel-and-scrollbar", viewportFingerprint(before) !== viewportFingerprint(wheel) && viewportFingerprint(wheel) !== viewportFingerprint(moved) && moved.search.viewportIntent === "userControlled");
    if (recipe.contract.id === "late-reveal-retired") {
      const requestReveal = async (assertion: string): Promise<SearchSnapshot> => {
        const previousSequence = recipe.latest.state.mainListScroll.pendingReveal?.sequence ?? 0;
        const pending = await recipe.anchor(recipe.latest.search.selectedOrdinal === 0 ? 1 : 0);
        const ticket = pending.state.mainListScroll.pendingReveal;
        recipe.assert(assertion, Boolean(ticket) && ticket.sequence > previousSequence && sameQuery(ticket.query, pending.search.query) &&
          ticket.resultRevision === pending.search.resultRevision && ticket.selectionRevision === pending.search.selectionRevision &&
          ticket.viewportRevision === pending.search.viewportRevision && ticket.surfaceGeneration === identity(pending.state).surfaceGeneration);
        return pending;
      };
      const pending = await requestReveal("reveal-ticket-observed");
      const nextInput = recipe.plan("tabs").input.toUpperCase();
      if (nextInput === recipe.plan("tabs").input) throw new EvaluationContractError("missing-capability:scrollable-query-transition");
      await recipe.input(nextInput, "gpui-text", false);
      await recipe.advance(25); await recipe.capture("new-query-before-old-reveal");
      const newQuery = recipe.latest;
      recipe.assert("scrollable-new-query-committed", !newQuery.search.pending && !sameQuery(pending.search.query, newQuery.search.query) && newQuery.search.committedRows.filter(row => row.selectable).length > 10);
      await recipe.advance(250); await recipe.observe("old-reveal-retired");
      recipe.assert("old-reveal-cannot-scroll-new-query", viewportFingerprint(newQuery) === viewportFingerprint(recipe.latest) && !recipe.latest.state.mainListScroll.pendingReveal);
      await recipe.input(recipe.plan("tabs").input);
      await requestReveal("wheel-reveal-ticket-observed");
      await scroll(false); const manual = recipe.latest;
      await recipe.advance(250); await recipe.observe("old-reveal-after-wheel");
      recipe.assert("old-reveal-cannot-undo-wheel", viewportFingerprint(manual) === viewportFingerprint(recipe.latest) && recipe.latest.search.viewportIntent === "userControlled");
    } else {
      await followupSource(recipe, "tabs", run.id);
      const anchors = [moved, recipe.latest].map(snapshot => {
        const row = snapshot.frame?.frameEvidence?.paintBindings?.find((binding: Json) => binding.kind === "mainSearchRow" && binding.visibleBounds?.height > 0);
        if (!row?.metadata?.stableKey || !Number.isFinite(row.bounds?.y)) throw new EvaluationContractError("missing-capability:painted-viewport-anchor");
        return { key: row.metadata.stableKey, y: row.bounds.y };
      });
      recipe.assert("viewport-anchor-preserved", anchors[0]!.key === anchors[1]!.key && anchors[0]!.y === anchors[1]!.y && recipe.latest.search.viewportIntent === "userControlled");
      recipe.record("viewport-source-refresh", { deliberate, anchors, scrollBefore: moved.state.mainListScroll, scrollAfter: recipe.latest.state.mainListScroll });
    }
  }
}
async function fileCapture(recipe: SearchRecipe, label: string): Promise<OwnedFrameCapture> {
  const state = await recipe.inspect(`${label}:state`, true);
  if (state.promptType !== "fileSearch" || state.fileSearch?.version !== 1) throw new EvaluationContractError("missing-capability:file-view-selection-observation");
  const previous = recipe.lastFrame!;
  const expected = { expected: identity(state), afterFrameGeneration: previous.frame.target.frameGeneration, afterNotificationEpoch: previous.frameEvidence!.notificationEpoch };
  if (++recipe.captures > recipe.schedule.bounds.frames) throw new EvaluationContractError("search-case-frame-bound");
  const frame = await recipe.request(() => recipe.client.captureFrame(recipe.target, false, expected, recipe.frameCursor));
  for (const issue of naturalEvidenceIssues(frame, expected, recipe.frameStore.pool)) recipe.assert(`${label}:${issue}`, false);
  recipe.assert(`${label}:native-file-paint`, frame.frameEvidence?.paintFailures?.length === 0 && frame.frameEvidence?.pixelEvidenceComplete === true);
  recipe.assert(`${label}:file-owner-paint-join`, ["query", "selectionMode", "presentation", "selectedPath", "selectedOrdinal", "rows"]
    .every(field => JSON.stringify(frame.frameEvidence?.fileSearch?.[field]) === JSON.stringify(frame.state.fileSearch?.[field])));
  recipe.lastFrame = frame;
  recipe.recordFramePage(label, frame.state.frameEvidence, [frameFacts(frame.frameEvidence!)], (extra, completedFrames) => ({
    frameEvidence: extra[0], completedFrames, capture: frame.snapshot.capture }), frame.frameEvidence);
  recipe.retainedState = frame.state;
  return frame;
}
async function enterFileView(recipe: SearchRecipe, presentation: "Full" | "Mini", query: string): Promise<OwnedFrameCapture> {
  if (presentation === "Mini") await recipe.action({ type: "key", key: "~", text: "~" });
  else {
    await recipe.input("Search Files");
    const row = recipe.latest.search.committedRows.find(row => row.stableKey === "builtin/file-search");
    if (!row) throw new EvaluationContractError("missing-capability:file-search-builtin");
    await recipe.action({ type: "select", semanticId: row.semanticId, submit: true });
  }
  let state = await recipe.inspect("entered-file-view");
  if (state.promptType !== "fileSearch") throw new EvaluationContractError("file-view-route-not-entered");
  await recipe.action({ type: "setInput", text: query }, identity(state));
  state = await recipe.inspect("file-view-source-admitted");
  const stream = state.fileSearch?.stream;
  if (!stream || !Number.isSafeInteger(stream.generation) || stream.generation <= 0 || stream.query !== query)
    throw new EvaluationContractError("missing-capability:file-search-stream-state");
  const condition = { generation: stream.generation, query };
  const terminal = await recipe.request(() => recipe.client.waitForFileSearchStream(recipe.target, condition,
    Math.max(1, Math.min(5000, Math.floor(recipe.schedule.bounds.wallMilliseconds - (performance.now() - recipe.started))))));
  recipe.record("file-view-source-terminal", { fileSearchStream: terminal.fileSearchStream, targetIdentity: terminal.targetIdentity });
  if (terminal.fileSearchStream.phase !== "completed" || terminal.fileSearchStream.loading || terminal.fileSearchStream.failure !== null)
    throw new EvaluationContractError("file-view-source-not-completed");
  await recipe.refuse(() => recipe.client.waitForFileSearchStream(recipe.target, { ...condition, generation: condition.generation + 1 }),
    "file-view-stale-generation-refused", ["file_search_stream_generation_stale"]);
  await recipe.refuse(() => recipe.client.waitForFileSearchStream(recipe.target, { ...condition, query: `${query}x` }),
    "file-view-stale-query-refused", ["file_search_stream_query_stale"]);
  state = await recipe.inspect("file-view-stale-waits-refused");
  recipe.assert("file-view-stale-waits-preserve-stream", state.fileSearch?.stream?.generation === condition.generation &&
    state.fileSearch.stream.query === query && state.fileSearch.stream.phase === "completed");
  return fileCapture(recipe, `file-view-${presentation}`);
}
async function previewRecipe(recipe: SearchRecipe): Promise<void> {
  await recipe.prepare();
  const firstFrame = await enterFileView(recipe, "Full", recipe.prepared.fileViewInputs.preview);
  let state = firstFrame.state;
  const first = state.fileSearch;
  if (first.rows.length !== 2 || !first.selectedPath?.endsWith(".png")) throw new EvaluationContractError("missing-capability:compiled-preview-images");
  const waitForDecode = async (observed: Json, label: string): Promise<Json> => {
    const condition = { generation: observed.fileSearch?.stream?.generation, query: observed.fileSearch?.stream?.query,
      workSequence: observed.fileSearch?.preview?.workSequence };
    const waited = await recipe.request(() => recipe.client.waitForFileSearchPreview(recipe.target, condition,
      Math.max(1, Math.min(5000, Math.floor(recipe.schedule.bounds.wallMilliseconds - (performance.now() - recipe.started))))));
    const held = waited.fileSearchPreview;
    recipe.record(label, { fileSearchPreview: held, targetIdentity: waited.targetIdentity });
    if (!held.decoded || held.path !== observed.fileSearch.selectedPath) throw new EvaluationContractError("preview-decode-not-held-for-current-subject");
    return held;
  };
  const firstHeld = await waitForDecode(state, "preview-first-decoder-held");
  await recipe.refuse(() => recipe.client.waitForFileSearchPreview(recipe.target,
    { generation: firstHeld.generation, query: firstHeld.query, workSequence: firstHeld.workSequence + 1 }),
    "preview-stale-work-refused", ["file_search_preview_work_stale"]);
  state = await recipe.inspect("preview-first-held");
  const pendingBefore = state.searchProviders.pendingPreviewCompletions;
  const firstSequence = firstHeld.workSequence;
  recipe.assert("preview-work-pending-before-retarget", pendingBefore?.some((work: Json) => work.workSequence === firstSequence && work.decoded));
  await recipe.action({ type: "key", key: "down" }, identity(state));
  const secondFrame = await fileCapture(recipe, "preview-retargeted-before-completion");
  state = secondFrame.state;
  const second = state.fileSearch;
  recipe.assert("preview-selection-changed", first.selectedPath !== second.selectedPath);
  const secondHeld = await waitForDecode(state, "preview-second-decoder-held");
  state = await recipe.inspect("preview-both-decoders-held");
  const secondSequence = secondHeld.workSequence;
  recipe.assert("two-real-preview-decodes-held", firstSequence !== secondSequence && [firstHeld, secondHeld].every(held =>
    state.searchProviders.pendingPreviewCompletions?.some((work: Json) => work.workSequence === held.workSequence &&
      work.generation === held.generation && work.query === held.query && work.path === held.path && work.decoded && work.contentHash === held.contentHash)));
  recipe.assert("preview-gate-time-not-advanced", firstHeld.logicalTimeMs === secondHeld.logicalTimeMs);
  await recipe.advance(Math.max(firstHeld.dueAtMs, secondHeld.dueAtMs) - secondHeld.logicalTimeMs);
  const finished = await fileCapture(recipe, "preview-old-completion-fenced");
  const file = finished.state.fileSearch;
  const binding = finished.frameEvidence?.paintBindings?.find((binding: Json) => binding.kind === "fileSearchPreviewImage");
  recipe.assert("preview-current-selected-subject", file.selectedPath === second.selectedPath && binding?.metadata?.path === file.selectedPath &&
    binding?.metadata?.loadState === "ready" && /^[a-f0-9]{64}$/.test(binding?.metadata?.contentHash ?? ""));
  const logicalWidth = finished.layout.windowWidth, logicalHeight = finished.layout.windowHeight;
  const pixelWidth = finished.snapshot.capture?.width, pixelHeight = finished.snapshot.capture?.height;
  const scaleX = pixelWidth! / logicalWidth, scaleY = pixelHeight! / logicalHeight;
  recipe.assert("preview-current-selected-subject", Number.isSafeInteger(firstSequence) && Number.isSafeInteger(secondSequence) && firstSequence !== secondSequence &&
    Array.isArray(file.previewWork) && file.previewWork.some((work: Json) => work.sequence === firstSequence && work.status === "discarded") &&
    file.previewWork.some((work: Json) => work.sequence === secondSequence && work.status === "installed"));
  if (!binding?.visibleBounds || ![logicalWidth, logicalHeight, pixelWidth, pixelHeight, scaleX, scaleY].every(value => Number.isFinite(value) && value > 0) ||
      Math.abs(pixelWidth! - logicalWidth * scaleY) > 1 || Math.abs(pixelHeight! - logicalHeight * scaleX) > 1)
    throw new EvaluationContractError("missing-capability:preview-image-pixel-geometry");
  recipe.record("preview-pixel-coordinate-space", { logicalWidth, logicalHeight, pixelWidth, pixelHeight, scaleX, scaleY });
  const bounds = binding.visibleBounds;
  const probes = [0.25, 0.5, 0.75].map(part => ({ x: Math.floor((bounds.x + bounds.width * part) * scaleX), y: Math.floor((bounds.y + bounds.height / 2) * scaleY) }));
  const pixels = await recipe.request(() => recipe.client.probePixels(recipe.target, finished.frame.target, probes));
  const colours: Record<string, readonly number[]> = { "example.invalid-preview-0.png": [210, 48, 52], "example.invalid-preview-1.png": [36, 148, 92] };
  const colour = colours[file.selectedPath.split("/").at(-1)];
  recipe.assert("preview-current-selected-subject", Boolean(colour) && pixels.pixelProbes?.length === 3 && pixels.pixelProbes.every(pixel => pixel.a === 255 && [pixel.r, pixel.g, pixel.b].every((channel, index) => Math.abs(channel - colour![index]!) <= 2)));
  recipe.record("preview-decoder-fence", { pendingBefore, work: file.previewWork, selectedPath: file.selectedPath, image: binding?.metadata, pixels: pixels.pixelProbes });
  await recipe.action({ type: "key", key: "escape" }, finished.frame.target);
}
async function fileViewRecipe(recipe: SearchRecipe): Promise<void> {
  const views: Json[] = [];
  for (const presentation of ["Mini", "Full"] as const) {
    await recipe.prepare("directory-browse");
    const frame = await enterFileView(recipe, presentation, presentation === "Mini" ? recipe.prepared.fileViewInputs.mini : recipe.prepared.fileViewInputs.full);
    const initial = frame.state.fileSearch;
    recipe.assert("full-mini-file-view", initial.presentation === presentation && initial.selectionMode === "AutoFirst" &&
      initial.rows.length > 1 && initial.rows.every((row: Json) => row.path.startsWith(recipe.prepared.fileViewInputs.full)));
    await recipe.request(() => recipe.client.act(recipe.target, { type: "key", key: "down" }, frame.frame.target));
    const chosen = await fileCapture(recipe, "file-view-deliberate-selection");
    await recipe.advance(250);
    const after = await recipe.inspect("file-view-after-selection");
    recipe.assert("auto-first-user-locked-isolated", initial.selectedOrdinal === 0 && chosen.state.fileSearch.selectedOrdinal === 1 &&
      chosen.state.fileSearch.selectedPath !== initial.selectedPath && chosen.state.fileSearch.selectionMode === "UserLockedPath" &&
      chosen.state.fileSearch.selectedPath === after.fileSearch.selectedPath && after.fileSearch.selectionMode === "UserLockedPath");
    views.push({ presentation, initial, chosen: chosen.state.fileSearch, after: after.fileSearch });
    await recipe.request(() => recipe.client.act(recipe.target, { type: "key", key: "escape" }, identity(after)));
  }
  recipe.record("file-view-isolation", { views });
}

export function compareSearchOrders(results: readonly SearchScheduleResult[]): void {
  const schedules = searchContractSpec().schedules;
  const groups = new Map<string, { expected: readonly string[]; records: { result: SearchScheduleResult; receipt: SearchOrderReceipt }[] }>();
  for (const result of results) {
    const evidence = result.evidence as SearchCaseEvidence | undefined;
    const comparisons = Array.isArray(evidence?.orderComparisons) ? evidence.orderComparisons : [];
    const schedule = schedules.find(schedule => schedule.id === result.id);
    if (schedule?.structuralNotApplicable) {
      if (comparisons.length) result.issues.push("inapplicable-atomic-comparison");
      continue;
    }
    const groupId = schedule ? searchScheduleComparisonGroup(schedule) : null;
    if (!groupId) {
      if (comparisons.length) result.issues.push("unexpected-order-comparison");
      continue;
    }
    if (!result.executed) continue;
    const orderOf = (schedule: SearchSchedule): string => schedule.recipe.kind === "same-turn" ? "same-turn" : schedule.recipe.kind === "cohort" ? schedule.recipe.order.join("-then-") : schedule.providers.join("-then-");
    const expected = schedules.filter(schedule => !schedule.structuralNotApplicable && searchScheduleComparisonGroup(schedule) === groupId).map(orderOf);
    const requiredKeys = [`${groupId}:automatic`, `${groupId}:deliberate-when-eligible`];
    if (comparisons.length !== 2 || requiredKeys.some(key => !comparisons.some(receipt => receipt?.key === key))) result.issues.push("missing-intent-order-comparison");
    for (const receipt of comparisons) {
      if (!receipt || !requiredKeys.includes(receipt.key) || receipt.order !== orderOf(schedule!) || !/^[a-f0-9]{64}$/.test(receipt.fingerprint) ||
          !Array.isArray(receipt.expectedOrders) || receipt.expectedOrders.length !== expected.length || expected.some(order => !receipt.expectedOrders.includes(order))) {
        result.issues.push("invalid-order-comparison-receipt"); continue;
      }
      const group = groups.get(receipt.key) ?? { expected, records: [] };
      if (group.records.some(record => record.receipt.order === receipt.order)) { result.issues.push("duplicate-order-comparison"); continue; }
      group.records.push({ result, receipt }); groups.set(receipt.key, group);
    }
  }
  for (const [key, group] of groups) {
    const complete = group.expected.length === group.records.length && group.expected.every(order => group.records.some(record => record.receipt.order === order));
    const equal = complete && group.records.every(record => record.receipt.fingerprint === group.records[0]!.receipt.fingerprint);
    for (const { result } of group.records) {
      const existing = result.assertions.find(assertion => assertion.id === "final-candidates-equal");
      if (existing) existing.pass &&= equal; else result.assertions.push({ id: "final-candidates-equal", pass: equal });
      if (!complete) result.issues.push(`missing-order-comparison:${key}`);
      else if (!equal) result.issues.push(`provider-order-candidates-diverged:${key}`);
    }
  }
}
function finalizeResult(result: SearchScheduleResult, schedule: SearchSchedule): void {
  if (schedule.structuralNotApplicable) {
    result.auxiliaryExecution ??= { kind: "singlePhysicalOwnerDrain", executed: result.executed,
      pass: result.executed && !result.issues.length && result.assertions.length > 0 && result.assertions.every(assertion => assertion.pass), assertions: result.assertions };
    result.assertions = []; result.executed = false;
    result.status = result.auxiliaryExecution.pass && !result.issues.length ? "notApplicable" : "failed";
    return;
  }
  for (const id of schedule.assertions) if (!result.assertions.some(assertion => assertion.id === id)) result.issues.push(`uncovered-assertion:${id}`);
  result.issues = [...new Set(result.issues)];
  result.status = result.executed && !result.issues.length && result.assertions.length > 0 && result.assertions.every(assertion => assertion.pass) ? "passed" : "failed";
}
export async function runSearchSchedule(runtime: SearchRuntime, contract: SearchCase, schedule: SearchSchedule): Promise<SearchScheduleResult> {
  const recipe = new SearchRecipe(runtime, contract, schedule);
  try {
    switch (contract.id) {
      case "automatic-higher-arrival": case "keyboard-anchor-arrival": case "semantic-anchor-current-first":
      case "click-anchor-arrival": case "pointer-down-publication-up": case "same-input-noop": case "stale-agent-target":
        await arrivalRecipe(recipe); break;
      case "publisher-orders": await providerOrdersRecipe(recipe); break;
      case "passive-budget": await passiveBudgetRecipe(recipe); break;
      case "raw-query-before-commit": case "query-aba": await pendingQueryRecipe(recipe); break;
      case "implicit-files-cache-only": case "explicit-files-publish": case "pending-files-reuse": await filesRecipe(recipe); break;
      case "directory-scope": await directoryRecipe(recipe); break;
      case "brain-lexical-semantic": await brainRecipe(recipe); break;
      case "provider-terminal-errors": await terminalRecipe(recipe); break;
      case "empty-inert-rows": await emptyRecipe(recipe); break;
      case "selected-row-removal": case "metadata-same-identity": case "same-count-replacement": await changingRowsRecipe(recipe); break;
      case "eligibility-calculator": await calculatorRecipe(recipe); break;
      case "source-unarmed-down": await sourceUnarmedRecipe(recipe); break;
      case "scroll-pending-refresh": case "late-reveal-retired": await scrollRecipe(recipe); break;
      case "preview-stale-completion": await previewRecipe(recipe); break;
      case "retired-window-lifetime":
        for (const deliberate of [false, true]) {
          await recipe.prepare(); await recipe.input(recipe.plan("tabs").input);
          if (deliberate) await recipe.anchor();
          await retiredOwnerRecipe(recipe);
        }
        break;
      case "tilde-file-view-isolation": await fileViewRecipe(recipe); break;
      case "sentence-typing": await sentenceTypingRecipe(recipe); break;
      default: throw new EvaluationContractError("unknown-search-case");
    }
  } catch (error) {
    const failure = recipeFailure(error);
    recipe.evidence.failure = { ...recipe.evidence.failure, ...failure, lastCompletedPhase: recipe.evidence.phases.at(-1)?.id ?? "not-started" };
    recipe.result.issues.push(failure.code);
  }
  if (contract.id === "pointer-down-publication-up" && recipe.result.executed) {
    try {
      const retired = runtime.target;
      await recipe.unmount();
      runtime.target = await recipe.request(() => runtime.client.mount(SEARCH_FIXTURE_ID));
      recipe.record("retired-unfinished-pointer-gesture", { retired, replacement: runtime.target });
    } catch (error) {
      const failure = recipeFailure(error);
      recipe.result.issues.push(failure.code);
      recipe.evidence.failure = { ...recipe.evidence.failure, ...failure, lastCompletedPhase: recipe.evidence.phases.at(-1)?.id ?? "not-started" };
    }
  }
  recipe.evidence.resourceUse = { requests: runtime.client.driver.stats.requestsSent - recipe.requestsStarted,
    steps: recipe.steps, captures: recipe.captures, logicalMilliseconds: recipe.logicalMilliseconds,
    wallMilliseconds: Math.ceil(performance.now() - recipe.started) };
  try {
    validateSearchObservationPhases(recipe.evidence.phases);
    validateSearchFramePool(recipe.evidence.framePool, [...recipe.evidence.phases.flatMap(phase => [...(phase.completedFrames ?? []), ...(phase.frameEvidence ? [phase.frameEvidence] : [])]),
      ...(recipe.evidence.counterexample?.frame ? [recipe.evidence.counterexample.frame] : [])]);
    for (const phase of recipe.evidence.phases) reconstructSearchCapturePixels(recipe.evidence.framePool, phase);
  } catch (error) { recipe.result.issues.push(recipeFailure(error).code); }
  if (Buffer.byteLength(JSON.stringify(recipe.result)) > schedule.bounds.retainedBytes) recipe.result.issues.push("search-evidence-byte-bound");
  if (!recipe.evidence.orderComparisons.length) finalizeResult(recipe.result, schedule);
  return recipe.result;
}

async function runtimeSafety(runtime: SearchRuntime): Promise<Json> {
  const { client, target } = runtime;
  const command = async (control: { operation: "prepare"; scenario: string } | { operation: "advance"; milliseconds: number } | { operation: "release"; runIds: readonly number[] }) => {
    const state = await client.inspect(target);
    return client.design({ operation: "fixtureControl", target, expected: identity(state), control: { family: "search", ...control } });
  };
  await command({ operation: "prepare", scenario: "tab-domain-hoist" });
  const before = await client.inspect(target);
  await client.act(target, { type: "setInput", text: "example.invalid" }, identity(before));
  await command({ operation: "advance", milliseconds: 250 });
  const state = await client.inspect(target);
  requireIssues(providerObservationIssues(state.searchProviders));
  const observation = state.searchProviders as SearchProviderObservation;
  const runs = observation.runs;
  const held = runs.find(run => run.kind === "worker" && run.state === "held");
  if (!held) throw new EvaluationContractError("missing-capability:safety-held-worker");
  let atomicRefusal: string | undefined;
  try { await command({ operation: "release", runIds: [held.id, Number.MAX_SAFE_INTEGER] }); }
  catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; atomicRefusal = error.code; }
  const atomicAfter = await client.inspect(target);
  const atomicHeld = atomicAfter.searchProviders.runs.find((run: SearchProviderRun) => run.id === held.id);
  if (!atomicRefusal || atomicHeld?.state !== "held") throw new EvaluationContractError("non-atomic-source-admission-refusal");
  const baseline = await client.captureFrame(target, false);
  const canonicalPreflight = baseline.state.mainWindowPreflight;
  const framePreflight = baseline.frameEvidence?.search?.preflight;
  if (!canonicalPreflight || typeof canonicalPreflight !== "object" || Array.isArray(canonicalPreflight) ||
      !Object.hasOwn(canonicalPreflight, "selectedResultKey") || !Object.hasOwn(canonicalPreflight, "enterAction") ||
      !framePreflight || typeof framePreflight !== "object" || Array.isArray(framePreflight) || !baseline.state.searchObservation ||
      Object.hasOwn(baseline.state.searchObservation, "preflight") || canonicalFrameJson(framePreflight) !== canonicalFrameJson(canonicalPreflight))
    throw new EvaluationContractError("canonical-preflight-observation-required");
  let missingNotification: string | undefined;
  try { await client.captureFrame(target, false, { expected: baseline.frame.target, afterFrameGeneration: baseline.frame.target.frameGeneration,
    afterNotificationEpoch: baseline.frameEvidence!.notificationEpoch }); }
  catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; missingNotification = error.code; }
  if (missingNotification !== "scheduled_frame_notification_missing") throw new EvaluationContractError("missing-frame-negative-unexpected-result");
  const cursorBefore = await client.inspect(target);
  const frameCursor: OwnedFrameCursor = { traceGeneration: cursorBefore.frameEvidence.traceGeneration,
    afterFrameGeneration: cursorBefore.frameEvidence.latestFrameGeneration };
  await client.inspect(target, frameCursor);
  const cursorRefusals: Json[] = [];
  for (const operation of ["getState", "captureFrame"] as const) for (const [id, requested, expectedCode] of [
    ["stale", { ...frameCursor, traceGeneration: frameCursor.traceGeneration + 1 }, "frame_cursor_stale"],
    ["future", { ...frameCursor, afterFrameGeneration: Number.MAX_SAFE_INTEGER }, "frame_cursor_future"],
    ["null", null, "frame_cursor_invalid"],
    ["unknown-field", { ...frameCursor, unexpected: true }, "frame_cursor_invalid"],
    ["fractional", { ...frameCursor, afterFrameGeneration: 0.5 }, "frame_cursor_invalid"],
  ] as const) {
    let code: string | undefined;
    // Deliberate malformed wire probes must reach the native parser, not the client-side validator.
    try {
      const response = await client.driver.request(operation === "getState" ? { type: "getState", target, frameCursor: requested } :
        { type: "design", command: { operation: "captureFrame", target, includeImage: false, frameCursor: requested } });
      if (operation === "captureFrame" && response.result?.operation === operation && response.result.ok === false && typeof response.result.error?.code === "string")
        code = response.result.error.code;
    } catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; code = error.code; }
    if (code !== expectedCode) throw new EvaluationContractError("frame-cursor-negative-unexpected-result", [operation, id, code ?? "accepted"]);
    cursorRefusals.push({ operation, id, code, requested });
  }
  const searchProviderRefusals: Json[] = [];
  const sourceCondition = { type: "searchProvider", source: held.source, query: cursorBefore.searchObservation.query, afterRunId: 0 };
  for (const [id, requestedTarget, condition, expectedCode] of [
    ...(["lifetime", "revision", "scopeRevision"] as const).map(field => [`stale-${field}`, target,
      { ...sourceCondition, query: { ...sourceCondition.query, [field]: sourceCondition.query[field] + 1 } }, "search_provider_query_stale"] as const),
    ["stale-target", { ...target, generation: target.generation + 1 }, sourceCondition, "stale_window_generation"],
    ["unknown-condition-field", target, { ...sourceCondition, unexpected: true }, "search_provider_condition_invalid"],
    ["unknown-query-field", target, { ...sourceCondition, query: { ...sourceCondition.query, unexpected: true } }, "search_provider_condition_invalid"],
    ["null-query", target, { ...sourceCondition, query: null }, "search_provider_condition_invalid"],
    ["fractional-run-id", target, { ...sourceCondition, afterRunId: 0.5 }, "search_provider_condition_invalid"],
    ["null-cache-opt-in", target, { ...sourceCondition, acceptCached: null }, "search_provider_condition_invalid"],
    ["nonboolean-cache-opt-in", target, { ...sourceCondition, acceptCached: "true" }, "search_provider_condition_invalid"],
    ["cache-opt-in-with-run-bound", target, { ...sourceCondition, acceptCached: true, afterRunId: held.id }, "search_provider_condition_invalid"],
  ] as const) {
    let code: string | undefined;
    try {
      const response = await client.driver.request({ type: "waitFor", target: requestedTarget, condition, timeout: 0 });
      if (response.type === "waitForResult" && response.success === false && typeof response.error?.code === "string") code = response.error.code;
    } catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; code = error.code; }
    if (code !== expectedCode) throw new EvaluationContractError("search-provider-wait-negative-unexpected-result", [id, code ?? "accepted"]);
    searchProviderRefusals.push({ id, target: requestedTarget, condition, code });
  }
  const cursorAfter = await client.inspect(target, frameCursor);
  const cursorAuthority = (value: Json) => ({ search: value.searchObservation, providers: value.searchProviders, target: identity(value),
    frame: { traceGeneration: value.frameEvidence.traceGeneration, latestFrameGeneration: value.frameEvidence.latestFrameGeneration,
      notificationEpoch: value.frameEvidence.notificationEpoch } });
  if (digest(cursorAuthority(cursorBefore)) !== digest(cursorAuthority(cursorAfter)))
    throw new EvaluationContractError("read-only-negative-controls-mutated-search-authority");
  const probes: Json[] = [];
  for (const probe of ["blankReadback", "failedReadback", "deferredDispatch"] as const) {
    const result = await client.probeSafety(target, probe);
    const assertions = nativeSafetyProbeAssertions(result);
    if (assertions.some(assertion => !assertion.pass)) throw new EvaluationContractError("native-safety-negative-failed");
    probes.push({ probe, assertions, negativeOnly: true, productionEvidence: false, observation: result.observation, before: result.before, after: result.after });
  }
  // Cancellation controls above require an idle owner. Retirement proof below
  // deliberately types again, so its pending work must not enter those controls.
  // The binary has no Rust test harness; exercise its trace contract natively.
  const acknowledgementBefore = await client.inspect(target);
  const beforeTrace = acknowledgementBefore.frameEvidence;
  const retainedFrames: Json[] = beforeTrace.completedFrames;
  if (retainedFrames.length < 2) throw new EvaluationContractError("frame-acknowledgement-history-required");
  const keptFrame = retainedFrames.at(-1)!;
  const acknowledgedCursor = { traceGeneration: beforeTrace.traceGeneration, afterFrameGeneration: keptFrame.frame.target.frameGeneration };
  const acknowledgementRefusals: Json[] = [];
  const refuseAcknowledgement = async (id: string, cursor: OwnedFrameCursor, expected: AutomationTargetSnapshot, expectedCode: string) => {
    let code: string | undefined;
    try { await client.design({ operation: "acknowledgeFrames", target, expected, cursor }); }
    catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; code = error.code; }
    if (code !== expectedCode) throw new EvaluationContractError("frame-acknowledgement-negative-unexpected-result", [id, code ?? "accepted"]);
    acknowledgementRefusals.push({ id, cursor, expected, code });
  };
  for (const [id, cursor, expectedCode] of [
    ["stale", { ...acknowledgedCursor, traceGeneration: acknowledgedCursor.traceGeneration + 1 }, "frame_cursor_stale"],
    ["future", { ...acknowledgedCursor, afterFrameGeneration: Number.MAX_SAFE_INTEGER }, "invalid_frame_acknowledgement_expectation"],
    ["unknown", { ...acknowledgedCursor, afterFrameGeneration: beforeTrace.retiredBeforeFrameGeneration }, "frame_cursor_unknown"],
  ] as const) await refuseAcknowledgement(id, cursor, identity(acknowledgementBefore), expectedCode);
  await refuseAcknowledgement("stale-target", acknowledgedCursor,
    { ...identity(acknowledgementBefore), dataGeneration: identity(acknowledgementBefore).dataGeneration + 1 }, "stale_target_identity");
  await refuseAcknowledgement("unobserved-frame", acknowledgedCursor,
    { ...identity(acknowledgementBefore), frameGeneration: acknowledgedCursor.afterFrameGeneration - 1 }, "invalid_frame_acknowledgement_expectation");
  const afterRefusals = await client.inspect(target);
  if (digest(afterRefusals.frameEvidence) !== digest(beforeTrace) || digest(cursorAuthority(afterRefusals)) !== digest(cursorAuthority(acknowledgementBefore)))
    throw new EvaluationContractError("frame-acknowledgement-refusal-mutated-trace");
  const acknowledged = await client.acknowledgeFrames(target, identity(afterRefusals), acknowledgedCursor);
  const afterAcknowledgement = await client.inspect(target);
  const afterTrace = afterAcknowledgement.frameEvidence;
  if (acknowledged.retiredFrames !== retainedFrames.length - 1 || acknowledged.retainedFrames !== 1 ||
      afterTrace.completedFrames.length !== 1 || digest(afterTrace.completedFrames[0]) !== digest(keptFrame) ||
      afterTrace.retiredBeforeFrameGeneration !== acknowledgedCursor.afterFrameGeneration ||
      afterTrace.retainedTraceBytes !== acknowledged.retainedTraceBytes || afterTrace.retainedTraceBytes >= beforeTrace.retainedTraceBytes ||
      digest(cursorAuthority(afterAcknowledgement)) !== digest(cursorAuthority(acknowledgementBefore)))
    throw new EvaluationContractError("frame-acknowledgement-retention-mismatch");
  const repeated = await client.acknowledgeFrames(target, identity(afterAcknowledgement), acknowledgedCursor);
  if (repeated.retiredFrames !== 0 || repeated.retainedFrames !== 1 || repeated.retainedTraceBytes !== acknowledged.retainedTraceBytes)
    throw new EvaluationContractError("frame-acknowledgement-not-idempotent");
  await refuseAcknowledgement("retired", { ...acknowledgedCursor, afterFrameGeneration: beforeTrace.retiredBeforeFrameGeneration },
    identity(afterAcknowledgement), "frame_cursor_retired");
  const afterRepeated = await client.inspect(target);
  if (digest(afterRepeated.frameEvidence) !== digest(afterTrace)) throw new EvaluationContractError("frame-acknowledgement-repeat-mutated-trace");
  const edited = await client.act(target, { type: "key", key: "x", text: "x" }, identity(afterRepeated));
  if (edited.actionReceipt?.dispatchCompleted !== true) throw new EvaluationContractError("frame-acknowledgement-next-action-undispatched");
  const nextFrame = await client.captureFrame(target, false, { expected: edited.actionReceipt.after,
    afterFrameGeneration: acknowledgedCursor.afterFrameGeneration, afterNotificationEpoch: keptFrame.notificationEpoch });
  const frameAcknowledgement = { version: 1, refusals: acknowledgementRefusals, acknowledged, repeated,
    before: { frameGenerations: retainedFrames.map(frame => frame.frame.target.frameGeneration), retainedTraceBytes: beforeTrace.retainedTraceBytes },
    after: { frameGenerations: afterTrace.completedFrames.map((frame: Json) => frame.frame.target.frameGeneration), retainedTraceBytes: afterTrace.retainedTraceBytes },
    authorityUnchanged: true, retainedBaselineSchedulesNextFrame: nextFrame.frame };
  return { id: runtime.safety.id, executionClass: "once-per-owned-runtime-safety", atomicRefusal, heldRunId: held.id,
    missingNotification, preflight: { stateLocation: "mainWindowPreflight", stateDuplicateAbsent: true, capturedFrameComplete: true,
      capturedFrameGeneration: baseline.frame.target.frameGeneration, preflightKeys: Object.keys(canonicalPreflight).sort() },
    frameCursor: { requested: frameCursor, refusals: cursorRefusals, authorityUnchanged: true,
      before: cursorAuthority(cursorBefore), after: cursorAuthority(cursorAfter) },
    searchProviderWait: { refusals: searchProviderRefusals, authorityReference: "frameCursor", authorityUnchanged: true }, frameAcknowledgement, probes, productionEvidence: false };
}
export async function runSearchJourney(reference: ArtifactReference, claim: OutputClaim, options: SearchRecipeOptions = {}): Promise<SearchJourneyReceipt> {
  const spec = searchContractSpec(); requireIssues(spec.issues);
  if (options.caseId && !SEARCH_CASES.some(contract => contract.id === options.caseId)) throw new EvaluationContractError("unknown-search-case");
  const selected = spec.schedules.filter(schedule => !options.caseId || schedule.caseId === options.caseId);
  const shards = partitionSearchSchedules(selected);
  if (options.shard !== undefined && (!Number.isSafeInteger(options.shard) || options.shard < 0 || options.shard >= shards.length)) throw new EvaluationContractError("invalid-search-shard");
  const requested = options.shard === undefined ? shards : [shards[options.shard]!];
  const requestedIds = new Set(requested.flatMap(shard => shard.schedules.map(schedule => schedule.id)));
  const results: SearchJourneyReceipt["coverage"]["results"] = spec.schedules.filter(schedule => !requestedIds.has(schedule.id)).map(schedule => ({ id: schedule.id, caseId: schedule.caseId,
    status: "blocked", executed: false, issues: ["outside-requested-shard"], assertions: [], notApplicableAssertions: schedule.notApplicableAssertions }));
  const receipt: RuntimeJourneyReceipt = { id: "launcher-ranking-provider", proofLevel: "owned-production-runtime", pass: false,
    assertions: [], frames: [], effects: [], fixtureIds: [SEARCH_FIXTURE_ID], cleanup: unknownOwnedCleanup(false) };
  const cleanups: OwnedCleanup[] = []; const shardReferences: SearchShardEvidenceReference[] = [];
  for (const shard of requested) {
    let client: OwnedEvaluationClient | undefined;
    let observedReceivedOutputBytesAfterSafety: number | null = null;
    const shardResults: SearchScheduleResult[] = []; const effects: Json[] = []; const shardCleanups: OwnedCleanup[] = [];
    try {
      client = await OwnedEvaluationClient.launch(ROOT, reference, claim, [SEARCH_FIXTURE_ID]);
      const catalog = await client.discover();
      if (catalog.frameCursor?.version !== 1 || catalog.frameCursor.operation !== "getState" || catalog.frameCursor.captureFrame !== true)
        throw new EvaluationContractError("missing-capability:frame-cursor");
      if (catalog.frameAcknowledgement?.version !== 1 || catalog.frameAcknowledgement.operation !== "acknowledgeFrames" ||
          catalog.frameAcknowledgement.retainsCursorFrame !== true || catalog.frameAcknowledgement.readCursorsArePassive !== true ||
          catalog.frameAcknowledgement.draws !== false)
        throw new EvaluationContractError("missing-capability:frame-acknowledgement");
      if (catalog.searchProviderWait?.version !== 1 || catalog.searchProviderWait.conditionType !== "searchProvider" ||
          catalog.searchProviderWait.sourceChange !== "explicitFixtureControl" || SEARCH_PROVIDERS.some(source => !catalog.searchProviderWait!.sources.includes(source)) ||
          catalog.searchProviderWait.acceptCached !== true || catalog.searchProviderWait.cacheAfterRunId !== 0 ||
          !Array.isArray(catalog.searchProviderWait.cacheSources) || catalog.searchProviderWait.cacheSources.length !== OWNED_SEARCH_CACHE_SOURCES.length ||
          OWNED_SEARCH_CACHE_SOURCES.some(source => !catalog.searchProviderWait!.cacheSources.includes(source)) ||
          (["admitted", "blocked", "settled", "cached"] as const).some(status => !catalog.searchProviderWait!.statuses.includes(status)))
        throw new EvaluationContractError("missing-capability:current-source-admission-wait");
      if (catalog.fileSearchStreamWait?.version !== 1 || catalog.fileSearchStreamWait.conditionType !== "fileSearchStream" ||
          catalog.fileSearchStreamWait.identityFields.length !== 2 || !catalog.fileSearchStreamWait.identityFields.includes("generation") ||
          !catalog.fileSearchStreamWait.identityFields.includes("query") || catalog.fileSearchStreamWait.terminalPhases.length !== 4 ||
          (["completed", "failed", "cancelled", "unavailable"] as const).some(phase => !catalog.fileSearchStreamWait!.terminalPhases.includes(phase)))
        throw new EvaluationContractError("missing-capability:file-search-stream-wait");
      if (catalog.fileSearchPreviewWait?.version !== 1)
        throw new EvaluationContractError("missing-capability:file-search-preview-wait");
      if (catalog.searchFixtures?.fixtureId !== SEARCH_FIXTURE_ID || catalog.searchFixtures.version !== 1 || SEARCH_PROVIDERS.some(source => !catalog.searchFixtures!.providers.includes(source)))
        throw new EvaluationContractError("missing-capability:complete-search-fixture-catalog");
      const runtime: SearchRuntime = { client, target: await client.mount(SEARCH_FIXTURE_ID), safety: { id: `search-runtime-${shard.index}` } };
      runtime.safety = await runtimeSafety(runtime); effects.push(runtime.safety);
      observedReceivedOutputBytesAfterSafety = client.driver.observedReceivedOutputBytes;
      for (const schedule of shard.schedules) {
        const contract = SEARCH_CASES.find(contract => contract.id === schedule.caseId)!;
        if (!catalog.searchFixtures.scenarios.some(scenario => scenario.id === contract.fixture)) {
          shardResults.push({ id: schedule.id, caseId: contract.id, status: "failed", executed: false, issues: ["missing-capability:compiled-source-fixture"], assertions: [], notApplicableAssertions: schedule.notApplicableAssertions });
        } else shardResults.push(await runSearchSchedule(runtime, contract, schedule));
      }
      await retireSearchRuntime(runtime, effects);
    } catch (error) {
      if (error instanceof DriverLifecycleError) shardCleanups.push(error.cleanup);
      const failure = recipeFailure(error); effects.push({ id: "search-shard-failure", shard: shard.index, ...failure });
      for (const schedule of shard.schedules) if (!shardResults.some(result => result.id === schedule.id)) shardResults.push({ id: schedule.id, caseId: schedule.caseId,
        status: "failed", executed: false, issues: [failure.code], assertions: [], notApplicableAssertions: schedule.notApplicableAssertions });
    } finally {
      if (client) {
        try { shardCleanups.push(await client.close()); } catch { shardCleanups.push(client.cleanup); }
        effects.push({ id: "search-runtime-output", shard: shard.index, observation: "after-close-attempt",
          scheduleIds: shard.schedules.map(schedule => schedule.id), observedReceivedOutputBytesAfterSafety,
          observedReceivedOutputBytes: client.driver.observedReceivedOutputBytes, maxOutputBytes: client.driver.maxOutputBytes,
          maxRetainedLogBytes: spec.admission.maxLogBytes, streamsDrained: client.cleanup.streamsDrained, cleanupClosed: client.cleanup.closed });
      }
    }
    const cleanup = shardCleanups.length ? aggregateCleanup(shardCleanups) : unknownOwnedCleanup(false);
    cleanups.push(cleanup);
    compareSearchOrders(shardResults);
    for (const result of shardResults) finalizeResult(result, shard.schedules.find(schedule => schedule.id === result.id)!);
    let retentionFailed = false;
    if (options.retainShard) {
      const scheduleIds = shard.schedules.map(schedule => schedule.id);
      try {
        const retained = options.retainShard({ version: 1, caseSetHash: spec.caseSetHash, shard: shard.index,
          scheduleIds: [...scheduleIds], results: shardResults, effects, cleanup });
        if (!retained || typeof retained.artifactId !== "string" || !retained.artifactId || retained.shard !== shard.index ||
            !Array.isArray(retained.scheduleIds) || retained.scheduleIds.length !== scheduleIds.length ||
            retained.scheduleIds.some((id, index) => id !== scheduleIds[index]) ||
            Object.keys(retained).some(key => !["artifactId", "shard", "scheduleIds"].includes(key)))
          throw new EvaluationContractError("invalid-search-shard-reference");
        const evidenceReference = { artifactId: retained.artifactId, shard: retained.shard, scheduleIds: [...retained.scheduleIds] };
        shardReferences.push(evidenceReference);
        for (const result of shardResults) {
          const { evidence: _evidence, ...summary } = result;
          results.push({ ...summary, evidenceReference: { artifactId: retained.artifactId, shard: retained.shard, scheduleId: result.id } });
        }
        receipt.effects.push({ id: "search-shard-evidence", evidenceReference, cleanupClosed: cleanup.closed });
      } catch (error) {
        retentionFailed = true; receipt.error = "search-shard-retention-failed";
        receipt.effects.push({ id: "search-shard-retention-failure", shard: shard.index, scheduleIds, ...recipeFailure(error) });
        for (const result of shardResults) {
          const { evidence: _evidence, ...summary } = result;
          results.push({ ...summary, status: "failed", issues: [...new Set([...summary.issues, "search-shard-retention-failed"])] });
        }
      }
    } else {
      results.push(...shardResults); receipt.effects.push(...effects);
    }
    if (!cleanup.closed) receipt.error = "INVALID_CLEANUP";
    if (retentionFailed || !cleanup.closed) break;
  }
  const coverage = accountSearchCoverage(spec.schedules, results);
  receipt.cleanup = cleanups.length ? aggregateCleanup(cleanups) : unknownOwnedCleanup(false);
  receipt.assertions = coverage.results.filter(result => result.status !== "notApplicable").map(result => ({ id: result.id, pass: result.status === "passed" }));
  receipt.pass = coverage.complete && receipt.cleanup.closed && !receipt.error;
  if (!receipt.pass) receipt.error ??= coverage.failed ? "search-contract-failed" : "search-contract-uncovered";
  return { ...receipt, coverage, caseSetHash: spec.caseSetHash, shardReferences };
}
