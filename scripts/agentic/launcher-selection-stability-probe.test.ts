import { expect, spyOn, test } from "bun:test";
import { SEARCH_CASES, SEARCH_PROVIDERS, SEARCH_SENTENCE_PROFILES, searchContractSpec, searchInventoryIssues, partitionSearchSchedules, searchScheduleComparisonGroup, accountSearchCoverage, type SearchScheduleResult, type SearchSchedule, type SearchTerminalReceipt } from "./launcher-search-contract.ts";
import { heldProviderIssues, naturalEvidenceIssues, providerObservationIssues, selectionPixelIssues, sourcePlanIssues, searchObservationIssues, paintBindingIssues, rankingFingerprint, compareSearchOrders, dispatchBindingIssues, copySinkIssues, SearchFrameStore, reconstructSearchFrame, reconstructSearchFrameFacts, validateSearchFramePool, reconstructSearchObservationPhase, validateSearchObservationPhases, reconstructSearchCapturePixels, runSearchSchedule, runSearchJourney, retireSearchRuntime, currentSourceResolution, currentSourceCache, type SearchRuntime, type SearchObservation, type SearchOrderReceipt, type SearchShardEvidence, type SearchShardEvidenceReference } from "./launcher-search-recipes.ts";
import { OwnedEvaluationClient, OWNED_SEARCH_CACHE_SOURCES, type OwnedFrameCapture, type SearchProviderRun, type ScheduledCapture, type OwnedCopySinkObservation, type OwnedFrameCursor } from "../devtools/lib/owned-evaluation.ts";
import type { AutomationTargetSnapshot, CompletedFrameIdentity } from "../devtools/lib/target-identity.ts";
import { createHash } from "node:crypto";
import { annotateOwnedEvidence } from "../devtools/design.ts";
import { sanitizeReceipt } from "../devtools/lib/privacy.ts";
import { DriverCommandRefused, DriverLifecycleError, DriverProtocolError, type Json } from "../devtools/driver.ts";
import { emptyOwnedCleanup } from "./artifact-lifecycle.ts";

function terminalResult(schedule: SearchSchedule): SearchScheduleResult {
  const outcomes = schedule.recipe.kind === "terminal" ? [schedule.recipe.outcome] : ["error", "unavailable", "disconnect"] as const;
  const terminalReceipts = schedule.providers.flatMap(source => outcomes.flatMap(requestedOutcome => schedule.terminalIntents!.required.map<SearchTerminalReceipt>(intent => {
    const synchronous = source === "brain-lexical" || source === "brain-inbox";
    const refused = synchronous && requestedOutcome === "disconnect";
    return { source, requestedOutcome, intent, query: { lifetime: 1, revision: 3, scopeRevision: 0 }, selectionArmed: true,
      selectedSemanticId: `main-list-row:v2:${"a".repeat(64)}`, provider: { id: 2, source, query: "example.invalid", generation: 2, payloadPhase: 1,
        kind: refused ? "sourceChange" : synchronous ? "synchronousRead" : "worker", publicationPolicy: refused ? null : synchronous ? "visible-synchronous" : "visible",
        state: refused ? "awaiting-admission" : requestedOutcome === "unavailable" ? "unavailable" : "failed",
        outcome: refused ? null : requestedOutcome === "disconnect" ? "disconnected" : requestedOutcome,
        ...(refused ? { admissionApplied: false, capabilityRefusal: "synchronous_source_has_no_worker" } : synchronous ? { originAdmissionId: 1 } : {}) } };
  })));
  return { id: schedule.id, caseId: schedule.caseId, status: "passed", executed: true, issues: [], terminalReceipts,
    assertions: schedule.assertions.map(id => ({ id, pass: true })), notApplicableAssertions: schedule.notApplicableAssertions };
}

test("finite inventory covers all cases, each provider timing, both pair orders, same turn and cohort permutations", () => {
  const spec = searchContractSpec();
  expect(spec.issues).toEqual([]); expect(spec.cases).toHaveLength(29);
  expect(spec.schedules).toHaveLength(1006);
  for (const provider of SEARCH_PROVIDERS) {
    for (const timing of ["before-initial-commit", "after-initial-commit", "after-deliberate-selection", "after-superseding-query", "after-owner-retirement"])
      expect(spec.schedules.some(s => s.id === `publisher-orders/${provider}/${timing}`)).toBe(true);
  }
  for (const first of SEARCH_PROVIDERS) for (const second of SEARCH_PROVIDERS) if (first !== second)
    expect(spec.schedules.some(s => s.id === `publisher-orders/${first}-then-${second}`)).toBe(true);
  expect(spec.schedules.filter(s => s.id.endsWith("/same-turn"))).toHaveLength(SEARCH_PROVIDERS.length * (SEARCH_PROVIDERS.length - 1) / 2);
  for (let cohort = 0; cohort < 4; cohort++) expect(spec.schedules.filter(s => s.id.includes(`/cohort-${cohort}/`))).toHaveLength(6);
  expect(searchContractSpec().caseSetHash).toBe(spec.caseSetHash);
  expect(spec.reduction.count).toBe(0);
});

test("every natural sentence retains all seven typing and overlapping-completion profiles", () => {
  const spec = searchContractSpec();
  const sentences = spec.schedules.filter(schedule => schedule.recipe.kind === "sentence");
  expect(sentences).toHaveLength(224);
  const inputs = new Map<string, string>();
  for (const schedule of sentences) {
    if (schedule.recipe.kind !== "sentence") throw new Error("sentence recipe required");
    const recipe = schedule.recipe;
    inputs.set(recipe.fixture, recipe.input);
    expect(recipe.entry).toBe(recipe.input.startsWith(" ") ? "caret-prefix" : "forward");
    expect(schedule.events).toContain(recipe.entry);
    expect(schedule.caseId).toBe("sentence-typing");
    expect(schedule.events).toContain("character-input");
    expect(schedule.events).toContain("assert-every-frame");
    expect(schedule.structuralNotApplicable).toBeNull();
    expect(schedule.bounds.frames).toBeGreaterThan(Array.from(recipe.input).length + 16);
    expect(recipe.input.trim().split(/\s+/).length).toBeGreaterThanOrEqual(4);
  }
  expect(inputs.size).toBe(32);
  for (const [fixture, input] of inputs) {
    const schedules = sentences.filter(schedule => schedule.recipe.kind === "sentence" && schedule.recipe.fixture === fixture);
    expect(schedules.map(schedule => schedule.recipe.kind === "sentence" && schedule.recipe.profile)).toEqual([...SEARCH_SENTENCE_PROFILES]);
    expect(schedules.every(schedule => schedule.recipe.kind === "sentence" && schedule.recipe.input === input)).toBe(true);
  }
  expect([...inputs.values()].some(input => input.normalize("NFC") !== input)).toBe(true);
  expect([...inputs.values()].some(input => Array.from(input).length !== input.length)).toBe(true);
  expect([...inputs.values()].some(input => input.trim() !== input)).toBe(true);
  const missing = sentences.find(schedule => schedule.recipe.kind === "sentence" && schedule.recipe.profile === "correction-aba")!;
  expect(searchInventoryIssues(SEARCH_CASES, spec.schedules.filter(schedule => schedule.id !== missing.id))).toContain(`missing-schedule:${missing.id}`);
});

test("missing case, missing assertions and skipped schedules cannot become coverage success", () => {
  const spec = searchContractSpec();
  expect(searchInventoryIssues(SEARCH_CASES.slice(1), spec.schedules)).toContain("complete-case-inventory-required");
  const missingAssertion = SEARCH_CASES.map((c, index) => index ? c : { ...c, assertions: c.assertions.slice(1) });
  expect(searchInventoryIssues(missingAssertion, spec.schedules)).toContain("missing-assertion:automatic-higher-arrival:late-arrival-visible");
  expect(searchInventoryIssues(SEARCH_CASES, spec.schedules.slice(1))).toContain("missing-schedule:automatic-higher-arrival/primary");
  const absent = accountSearchCoverage(spec.schedules, []);
  expect(absent.generated).toBe(spec.schedules.length); expect(absent.blocked).toBe(spec.schedules.length);
  expect(absent.executed).toBe(0); expect(absent.complete).toBe(false);
  const first = spec.schedules[0]!;
  expect(() => accountSearchCoverage([first], [{ id: first.id, caseId: first.caseId, status: "passed", executed: true, assertions: [{ id: "only-one", pass: true }], issues: [] }])).toThrow("unsupported-pass-claim");
  const assertions = SEARCH_CASES[0]!.assertions.map(id => ({ id, pass: true }));
  expect(() => accountSearchCoverage([first], [{ id: first.id, caseId: first.caseId, status: "passed", executed: false, assertions, issues: [] }])).toThrow("unsupported-pass-claim");
  expect(accountSearchCoverage([first], [{ id: first.id, caseId: first.caseId, status: "passed", executed: true, assertions, issues: [] }]).complete).toBe(false);
});
test("inapplicable criteria are non-proof and a single-owner drain is not an executed atomic factor", () => {
  const spec = searchContractSpec();
  const order = spec.schedules.find(schedule => schedule.recipe.kind === "order")!;
  const orderResult: SearchScheduleResult = { id: order.id, caseId: order.caseId, status: "passed", executed: true, issues: [],
    assertions: order.assertions.map(id => ({ id, pass: true })), notApplicableAssertions: order.notApplicableAssertions };
  expect(order.notApplicableAssertions.some(assertion => assertion.id === "same-turn-completion" && assertion.proof === false)).toBe(true);
  expect(accountSearchCoverage([order], [orderResult]).passed).toBe(1);
  expect(() => accountSearchCoverage([order], [{ ...orderResult, assertions: [...orderResult.assertions, { id: "same-turn-completion", pass: true }] }])).toThrow("unsupported-pass-claim");
  expect(() => accountSearchCoverage([order], [{ ...orderResult, notApplicableAssertions: [] }])).toThrow("unsupported-pass-claim");
  const structural = spec.schedules.filter(schedule => schedule.structuralNotApplicable);
  expect(structural).toHaveLength(1);
  const atomic = structural[0]!;
  const auxiliary: SearchScheduleResult = { id: atomic.id, caseId: atomic.caseId, status: "notApplicable", executed: false, assertions: [], issues: [],
    notApplicableAssertions: atomic.notApplicableAssertions, auxiliaryExecution: { kind: "singlePhysicalOwnerDrain", executed: true, pass: true,
      assertions: [{ id: "single-physical-owner-observed", pass: true }] } };
  const partial = accountSearchCoverage([atomic], [auxiliary]);
  expect(partial.generated).toBe(1); expect(partial.executed).toBe(0); expect(partial.auxiliaryExecuted).toBe(1);
  expect(partial.notApplicable).toBe(1); expect(partial.reduced).toBe(0); expect(partial.complete).toBe(false);
  expect(() => accountSearchCoverage([atomic], [{ ...auxiliary, executed: true }])).toThrow("unsupported-inapplicability-claim");
  expect(() => accountSearchCoverage([atomic], [{ ...auxiliary, auxiliaryExecution: undefined }])).toThrow("unsupported-inapplicability-claim");
  const complete = spec.schedules.map<SearchScheduleResult>(schedule => schedule.structuralNotApplicable ? auxiliary : schedule.terminalIntents ? terminalResult(schedule) : {
    id: schedule.id, caseId: schedule.caseId, status: "passed", executed: true, issues: [],
    assertions: schedule.assertions.map(id => ({ id, pass: true })), notApplicableAssertions: schedule.notApplicableAssertions,
  });
  const coverage = accountSearchCoverage(spec.schedules, complete);
  expect(coverage.complete).toBe(true); expect(coverage.generated).toBe(1006); expect(coverage.executed).toBe(1005);
  expect(coverage.eligible).toBe(1005); expect(coverage.caseCriteria.every(criterion => criterion.complete)).toBe(true);
  expect(accountSearchCoverage(spec.schedules, complete.slice(1)).complete).toBe(false);
});
test("terminal case coverage joins native receipts across all providers, outcomes and both intents", () => {
  const schedules = searchContractSpec().schedules.filter(schedule => schedule.terminalIntents);
  const primary = schedules.find(schedule => schedule.recipe.kind === "primary")!;
  expect(primary.terminalIntents!.required).toEqual(["automatic"]);
  expect(primary.terminalIntents!.notApplicable).toEqual([{ intent: "explicitAnchor", status: "notApplicable", proof: false, cause: "separateTerminalIntentSchedules" }]);
  const primaryOnly = accountSearchCoverage([primary], [terminalResult(primary)]);
  expect(primaryOnly.terminalCoverage.proved).toBe(60); expect(primaryOnly.terminalCoverage.complete).toBe(false);
  expect(primaryOnly.caseCriteria.find(item => item.caseId === primary.caseId)!.complete).toBe(false);
  const results = schedules.map(terminalResult); const coverage = accountSearchCoverage(schedules, results);
  expect(coverage.terminalCoverage.required).toBe(120); expect(coverage.terminalCoverage.proved).toBe(120);
  expect(coverage.terminalCoverage.complete).toBe(true);
  expect(coverage.terminalCoverage.factors.filter(factor => factor.intent === "explicitAnchor").every(factor => factor.scheduleIds.length === 1 && factor.scheduleIds[0] !== primary.id)).toBe(true);
  expect(accountSearchCoverage(schedules, results.slice(0, -1)).terminalCoverage.proved).toBe(119);
  const factor = schedules.find(schedule => schedule.recipe.kind === "terminal" && schedule.providers[0] === "tabs")!;
  const missing = terminalResult(factor); missing.terminalReceipts!.pop();
  expect(() => accountSearchCoverage([factor], [missing])).toThrow("missing-terminal-intent-proof");
  const unarmed = terminalResult(factor); unarmed.terminalReceipts![1]!.selectionArmed = false;
  expect(() => accountSearchCoverage([factor], [unarmed])).toThrow("unsupported-terminal-intent-proof");
  const oldPayload = terminalResult(factor); oldPayload.terminalReceipts![0]!.provider.payloadPhase = 0;
  expect(() => accountSearchCoverage([factor], [oldPayload])).toThrow("unsupported-terminal-intent-proof");
  const sync = schedules.find(schedule => schedule.recipe.kind === "terminal" && schedule.recipe.outcome === "disconnect" && schedule.providers[0] === "brain-lexical")!;
  const fakeTerminal = terminalResult(sync); fakeTerminal.terminalReceipts![0]!.provider.kind = "worker";
  expect(() => accountSearchCoverage([sync], [fakeTerminal])).toThrow("unsupported-terminal-intent-proof");
});

const held: SearchProviderRun = { id: 31, kind: "worker", source: "tabs", query: "example.invalid", generation: 7, state: "held", publicationPolicy: "visible" };
test("held gate validates exact run ownership and fingerprint sensitivity without claiming runtime proof", () => {
  expect(heldProviderIssues(held, { ...held }, "rows-a", "rows-a")).toEqual([]);
  expect(heldProviderIssues(held, { ...held, id: 32 }, "rows-a", "rows-a")).toContain("held-run-identity-changed");
  expect(heldProviderIssues(held, { ...held, state: "completed" }, "rows-a", "rows-a")).toContain("held-completion-escaped");
  expect(heldProviderIssues(held, held, "rows-a", "rows-b")).toContain("held-provider-published");
  expect(providerObservationIssues({ version: 1, scenario: "tab-domain-hoist", logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false, runs: [held], pendingRunIds: [32] })).toContain("unknown-pending-run");
  expect(providerObservationIssues({ version: 1, scenario: "tab-domain-hoist", logicalTimeMs: 0, displayUnixMs: Number.NaN, retired: false, overflow: false, runs: [], pendingRunIds: [] })).toContain("missing-capability:provider-display-clock");
});
test("current source resolution binds the native consumer and generation rather than the last gate run", () => {
  const search = searchState(); const owner = { source: "tabs", generation: held.generation, workQuery: held.query, workScope: "",
    consumer: { ...search.query }, publicationPolicy: "visible", queryBound: true, terminal: null };
  search.providers = { version: 1, runs: [owner], desired: [] };
  const observation = { version: 1, scenario: "all-providers", logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false,
    runs: [held, { ...held, id: held.id + 1, generation: held.generation + 1 }], pendingRunIds: [held.id, held.id + 1] };
  expect(currentSourceResolution(search, observation, "tabs")).toMatchObject({ status: "admitted", run: { id: held.id } });
  expect(currentSourceResolution(search, observation, "tabs", held.id)).toBeUndefined();
  for (const field of ["lifetime", "revision", "scopeRevision"] as const) {
    const changed = structuredClone(search); changed.providers.runs[0].consumer[field]++;
    expect(currentSourceResolution(changed, observation, "tabs")).toBeUndefined();
  }
  const detached = structuredClone(search); detached.providers.runs[0].consumer = null;
  expect(currentSourceResolution(detached, observation, "tabs")).toBeUndefined();
  const pending = structuredClone(search); pending.providers.desired.push({ source: "tabs", query: search.query });
  expect(currentSourceResolution(pending, observation, "tabs")).toBeUndefined();
  const beforeAdmission = structuredClone(search); beforeAdmission.providers.runs[0].generation += 2;
  expect(currentSourceResolution(beforeAdmission, observation, "tabs")).toBeUndefined();
  const rawPending = { ...search, pending: true };
  expect(currentSourceResolution(rawPending, observation, "tabs")).toBeUndefined();
});
test("settlement requires a matching terminal native owner and gate, never absence or retired work", () => {
  const search = searchState();
  search.providers = { version: 1, runs: [{ source: "tabs", generation: held.generation, consumer: search.query, terminal: "success" }], desired: [] };
  const observation = { version: 1, scenario: "all-providers", logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false,
    runs: [{ ...held, state: "completed", outcome: "success" }], pendingRunIds: [] };
  expect(currentSourceResolution(search, observation, "tabs")).toMatchObject({ status: "settled", run: { id: held.id } });
  expect(currentSourceResolution(search, { ...observation, runs: [] }, "tabs")).toBeUndefined();
  expect(currentSourceResolution(search, { ...observation, runs: [held] }, "tabs")).toBeUndefined();
  expect(currentSourceResolution(search, { ...observation, runs: [{ ...held, state: "completed", outcome: "empty" }] }, "tabs")).toBeUndefined();
  for (const terminal of ["staleDiscarded", "cancelled"]) {
    const retired = structuredClone(search); retired.providers.runs[0].terminal = terminal;
    expect(currentSourceResolution(retired, observation, "tabs")).toBeUndefined();
  }
  expect(currentSourceResolution(search, { ...observation, runs: [{ ...held, kind: "sourceChange", state: "awaiting-admission" }] }, "tabs")).toBeUndefined();
});
test("query-independent catalogue ownership survives input while its pending demand still blocks settlement", () => {
  const search = searchState(); const run = { ...held, source: "scripts" };
  search.providers = { version: 1, runs: [{ source: "scripts", generation: run.generation, queryBound: false, consumer: null, terminal: null }], desired: [] };
  const observation = { version: 1, scenario: "all-providers", logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false, runs: [run], pendingRunIds: [run.id] };
  expect(currentSourceResolution(search, observation, "scripts")).toMatchObject({ status: "admitted", run: { id: run.id } });
  search.providers.desired.push({ source: "scripts", query: { ...search.query, lifetime: search.query.lifetime + 1 } });
  expect(currentSourceResolution(search, observation, "scripts")).toBeUndefined();
});
test("a current cache proof is independent of a detached producer and never satisfies a fresh-run lower bound", () => {
  const search = searchState();
  const cache = { source: "tabs" as const, query: { ...search.query }, cacheIdentity: "tabs:7", cacheStateRevision: 7, rowCount: 4 };
  search.sourceCacheReadiness = [cache];
  search.providers = { version: 1, runs: [{ source: "tabs", generation: held.generation, consumer: null, queryBound: true, terminal: "success" }],
    desired: [{ source: "tabs", query: search.query }] };
  const observation = { version: 1, scenario: "all-providers", logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false,
    runs: [{ ...held, state: "completed", outcome: "success" }], pendingRunIds: [] };
  expect(currentSourceResolution(search, observation, "tabs")).toBeUndefined();
  expect(currentSourceCache(search, "tabs")).toBe(cache);
  expect(search.providers.runs[0].consumer).toBeNull();
  expect(currentSourceCache(search, "tabs", held.id)).toBeUndefined();
  expect(currentSourceCache(search, "files")).toBeUndefined();
  expect(currentSourceCache({ ...search, sourceCacheReadiness: [] }, "tabs")).toBeUndefined();
  expect(currentSourceCache({ ...search, pending: true }, "tabs")).toBeUndefined();
  expect(currentSourceCache({ ...search, computedQuery: { ...search.query, revision: search.query.revision + 1 } }, "tabs")).toBeUndefined();
  for (const field of ["lifetime", "revision", "scopeRevision"] as const) {
    const changed = { ...cache, query: { ...cache.query, [field]: cache.query[field] + 1 } };
    expect(currentSourceCache({ ...search, sourceCacheReadiness: [changed] }, "tabs")).toBeUndefined();
  }
});
test("cache readiness refuses malformed, duplicated or invented producer provenance", () => {
  const search = searchState();
  const cache = { source: "files" as const, query: { ...search.query }, cacheIdentity: "files:compiled-query", cacheStateRevision: null, rowCount: 0 };
  search.sourceCacheReadiness = [cache];
  expect(currentSourceCache(search, "files")).toBe(cache);
  expect(currentSourceCache({ ...search, sourceCacheReadiness: [cache, cache] }, "files")).toBeUndefined();
  for (const changed of [{ ...cache, rowCount: -1 }, { ...cache, cacheIdentity: "" }, { ...cache, cacheStateRevision: 1 }, { ...cache, producerRunId: held.id }])
    expect(currentSourceCache({ ...search, sourceCacheReadiness: [changed] }, "files")).toBeUndefined();
});

const target: AutomationTargetSnapshot = { windowId: "main", windowGeneration: 1, appViewVariant: "ScriptList", targetGeneration: 1, surfaceGeneration: 1, dataGeneration: 2, presentationRevision: 1, themeRevision: 1, frameGeneration: 2 };
const frame: CompletedFrameIdentity = { target, requestedTarget: { type: "instance", id: "main", generation: 1 }, pid: 12, processStartTime: "owned-start", processInstanceId: "owned-process", sessionGeneration: "owned-session", binarySha256: "a".repeat(64), manifestSha256: "b".repeat(64) };
const expectation: ScheduledCapture = { expected: { ...target, frameGeneration: 1 }, afterFrameGeneration: 1, afterNotificationEpoch: 5 };
function retainedFrame(generation: number, offset = 0) {
  const bounds = { x: 4, y: 20 + offset, width: 300, height: 24 };
  return { frame: { ...frame, target: { ...target, frameGeneration: generation } }, mode: "scheduled", cause: { kind: "notification", epoch: generation },
    traceGeneration: 3, notificationEpoch: generation, invalidationEpoch: generation, notificationCause: { source: "selection", sequence: generation },
    localInputFocused: true, nativeWindowActive: false, nativeWindow: { visible: false },
    search: { selectedSemanticId: "private selected subject", query: { lifetime: 2, revision: 7, scopeRevision: 1 } },
    paintFailures: [], pixelEvidenceComplete: true, pendingResources: 0, failedResources: 0,
    pixelEvidence: [{ kind: "selectionMarker", bounds, probe: { x: 4, y: 20 + offset, r: 114, g: 193, b: 168, a: 255 } }],
    paintBindings: [{ id: "row", kind: "mainSearchRow", bounds, visibleBounds: bounds, clipBounds: { x: 0, y: 0, width: 800, height: 600 },
      metadata: { stableKey: "private row key", selected: true, description: "private metadata ".repeat(512) } },
      { id: "fixed", kind: "mainSearchPreview", bounds: { x: 400, y: 0, width: 300, height: 600 }, metadata: { text: "private preview" } }] };
}
test("case-local frame tables preserve every intermediate frame while sharing frames, bindings and metadata", () => {
  const store = new SearchFrameStore(); const originals = Array.from({ length: 16 }, (_, index) => retainedFrame(index + 2, index));
  const limit = SEARCH_CASES.find(item => item.id === "automatic-higher-arrival")!.bounds.retainedBytes;
  expect(limit).toBe(131072);
  const document = store.retainWithin([...originals, originals[0]!, originals[15]!], refs => ({ evidence: { framePool: store.pool,
    phases: [{ completedFrames: refs.slice(0, 16) }, { frameEvidence: refs[16] }, { frameEvidence: refs[17] }] } }), limit - 40960);
  expect(store.pool.frames).toHaveLength(16); expect(store.pool.paintBindings).toHaveLength(17);
  expect(new Set(store.pool.paintBindings.map(entry => entry.metadataRef)).size).toBe(2);
  for (const field of ["search", "nativeWindow"] as const) expect(new Set(store.pool.frames.map(entry => entry.factRefs?.[field])).size).toBe(1);
  expect(new Set(store.pool.frames.map(entry => entry.ownerRef)).size).toBe(1);
  const references = document.evidence.phases[0]!.completedFrames!;
  expect(document.evidence.phases[1]!.frameEvidence).toEqual(references[0]); expect(document.evidence.phases[2]!.frameEvidence).toEqual(references[15]);
  references.forEach((reference, index) => expect(reconstructSearchFrame(store.pool, reference)).toEqual(originals[index]));
  expect(Buffer.byteLength(JSON.stringify(originals))).toBeGreaterThan(limit - 40960);
  expect(Buffer.byteLength(JSON.stringify(document))).toBeLessThanOrEqual(limit - 40960);
});
test("frame retention charges pooled payloads and rolls back an over-budget or conflicting insertion", () => {
  const store = new SearchFrameStore(); const original = retainedFrame(2);
  expect(() => store.retainWithin([original], refs => ({ framePool: store.pool, refs }), 100)).toThrow("search-evidence-byte-bound");
  expect(store.pool).toEqual({ version: 1, frames: [], paintBindings: [], metadata: [] });
  const document = store.retainWithin([original], refs => ({ framePool: store.pool, refs }), 131072 - 40960);
  expect(document.refs).toEqual([{ frameRef: 0 }]);
  const exact = Buffer.byteLength(JSON.stringify(document));
  expect(() => store.retainWithin([original], refs => ({ framePool: store.pool, refs }), exact - 1)).toThrow("search-evidence-byte-bound");
  expect(store.retainWithin([original], refs => ({ framePool: store.pool, refs }), exact)).toEqual(document);
  const changed = retainedFrame(2); changed.pixelEvidence[0]!.probe.r++;
  expect(() => store.retainWithin([changed], refs => ({ framePool: store.pool, refs }), 131072 - 40960)).toThrow("conflicting-frame-evidence-reference");
  const later = retainedFrame(3); later.paintBindings[0]!.metadata!.description = "private larger metadata ".repeat(2048);
  expect(() => store.retainWithin([later], refs => ({ framePool: store.pool, refs }), exact)).toThrow("search-evidence-byte-bound");
  expect(store.pool.frames).toHaveLength(1); expect(reconstructSearchFrame(store.pool, document.refs[0]!)).toEqual(original);
});
test("frame decoder refuses dangling and conflicting identities without trusting raw-content hashes", () => {
  const store = new SearchFrameStore(); const document = store.retainWithin([retainedFrame(2)], refs => ({ framePool: store.pool, refs }), 131072 - 40960);
  expect(() => reconstructSearchFrame(store.pool, { frameRef: 9 })).toThrow("dangling-frame-evidence-reference");
  const binding = structuredClone(store.pool); binding.frames[0]!.paintBindingRefs = [99];
  expect(() => reconstructSearchFrame(binding, document.refs[0]!)).toThrow("dangling-frame-evidence-reference");
  const metadata = structuredClone(store.pool); metadata.paintBindings[0]!.metadataRef = 99;
  expect(() => reconstructSearchFrame(metadata, document.refs[0]!)).toThrow("dangling-frame-evidence-reference");
  const duplicate = structuredClone(store.pool); duplicate.frames.push(structuredClone(duplicate.frames[0]!));
  expect(() => validateSearchFramePool(duplicate)).toThrow("conflicting-frame-evidence-reference");
  const shadowed = structuredClone(store.pool); shadowed.paintBindings[0]!.binding.metadata = {};
  expect(() => validateSearchFramePool(shadowed)).toThrow("conflicting-frame-evidence-reference");
});
test("saved frame references reconstruct after the actual producer privacy transformation", () => {
  const store = new SearchFrameStore(); const originals = [retainedFrame(2), retainedFrame(3, 1)];
  const document = store.retainWithin([...originals, originals[0]!], refs => ({ framePool: store.pool, completedFrames: refs }), 131072 - 40960);
  const options = { mode: "fixture-redacted" as const, fixtureId: "search-frame-pool" };
  const publicDocument = sanitizeReceipt(annotateOwnedEvidence(document), options).sanitized as typeof document;
  const expanded = sanitizeReceipt(annotateOwnedEvidence({ completedFrames: [...originals, originals[0]!] }), options).sanitized as { completedFrames: unknown[] };
  publicDocument.completedFrames.forEach((reference, index) => expect(reconstructSearchFrame(publicDocument.framePool, reference)).toEqual(expanded.completedFrames[index]));
  expect(publicDocument.completedFrames[0]).toEqual(publicDocument.completedFrames[2]);
  expect(typeof reconstructSearchFrameFacts(publicDocument.framePool, 0).search.selectedSemanticId).toBe("object");
  expect(JSON.stringify(publicDocument)).not.toContain("private ");
});
test("exact frame subobjects share storage while changed queries and frame identities remain distinct", () => {
  const store = new SearchFrameStore(); const originals = [retainedFrame(2), retainedFrame(3), retainedFrame(4)];
  originals[2]!.search.query.revision++;
  const retained = store.retainWithin(originals, refs => ({ framePool: store.pool, refs }), 131072 - 40960);
  const [first, second, third] = store.pool.frames;
  expect(first!.ownerRef).toBeDefined(); expect(second!.ownerRef).toBe(first!.ownerRef);
  expect(second!.factRefs).toEqual(first!.factRefs);
  expect(third!.factRefs!.search).not.toBe(first!.factRefs!.search);
  expect(first!.facts.frame.requestedTarget).toEqual(frame.requestedTarget);
  expect(first!.facts.frame.target.frameGeneration).toBe(2); expect(second!.facts.frame.target.frameGeneration).toBe(3);
  for (const field of ["search", "pixelEvidence", "nativeWindow"]) expect(Object.hasOwn(first!.facts, field)).toBe(false);
  retained.refs.forEach((reference, index) => expect(reconstructSearchFrame(store.pool, reference)).toEqual(originals[index]));
  const expanded = { framePool: { ...store.pool, frames: store.pool.frames.map((entry, index) => ({ facts: reconstructSearchFrameFacts(store.pool, index), paintBindingRefs: entry.paintBindingRefs })) }, refs: retained.refs };
  expect(Buffer.byteLength(JSON.stringify(retained))).toBeLessThan(Buffer.byteLength(JSON.stringify(expanded)));
});
test("frame fact and owner references reject dangling, cross-kind, mixed and foreign-window authority", () => {
  const store = new SearchFrameStore(); store.retainWithin([retainedFrame(2)], refs => ({ pool: store.pool, refs }), 131072 - 40960);
  const first = store.pool.frames[0]!;
  for (const field of ["search", "pixelEvidence", "nativeWindow"] as const) {
    const dangling = structuredClone(store.pool); dangling.frames[0]!.factRefs![field] = 999;
    expect(() => validateSearchFramePool(dangling)).toThrow("dangling-frame-evidence-reference");
    const mixed = structuredClone(store.pool); mixed.frames[0]!.facts[field] = reconstructSearchFrameFacts(store.pool, 0)[field];
    expect(() => validateSearchFramePool(mixed)).toThrow("conflicting-frame-evidence-reference");
    const crossKind = structuredClone(store.pool); crossKind.frames[0]!.factRefs![field] = first.ownerRef!;
    expect(() => reconstructSearchFrameFacts(crossKind, 0)).toThrow("invalid-frame-fact-reference");
  }
  const extra = structuredClone(store.pool); (extra.frames[0]!.factRefs as Json).unknown = first.ownerRef;
  expect(() => validateSearchFramePool(extra)).toThrow("conflicting-frame-evidence-reference");
  const crossOwner = structuredClone(store.pool); crossOwner.frames[0]!.ownerRef = first.factRefs!.search;
  expect(() => validateSearchFramePool(crossOwner)).toThrow("invalid-frame-owner-reference");
  for (const change of [(owner: Json) => { owner.processInstanceId = "foreign"; }, (owner: Json) => { owner.sessionGeneration = "foreign"; },
    (owner: Json) => { owner.requestedTarget.id = "other-window"; }, (owner: Json) => { owner.target = target; }]) {
    const foreign = structuredClone(store.pool); change(foreign.metadata[first.ownerRef!]!);
    expect(() => validateSearchFramePool(foreign)).toThrow("invalid-frame-owner-reference");
  }
  const shadow = structuredClone(store.pool); shadow.frames[0]!.facts.frame.binarySha256 = "c".repeat(64);
  expect(() => validateSearchFramePool(shadow)).toThrow("invalid-frame-owner-reference");
});

function cursorRecipeFixture() {
  let traceGeneration = 3; let prepareSucceeds = false;
  const cursors: (OwnedFrameCursor | undefined)[] = []; const controls: Json[] = [];
  const generations = [2, 3]; const stats = { requestsSent: 0 };
  const client = {
    driver: { stats },
    async inspect(_target: Json, cursor?: OwnedFrameCursor) {
      stats.requestsSent++; cursors.push(cursor ? { ...cursor } : undefined);
      return { targetIdentity: { ...target, frameGeneration: generations.at(-1)! },
        frameEvidence: { traceGeneration, traceOverflow: false, afterFrameGeneration: cursor?.afterFrameGeneration ?? null,
          latestFrameGeneration: generations.at(-1)!, completedFrames: generations.filter(generation => generation > (cursor?.afterFrameGeneration ?? 0)).map(generation => ({
            ...retainedFrame(generation), traceGeneration, search: undefined, fileSearch: { version: 1, selectedOrdinal: 0 },
          })) } };
    },
    async design(command: Json) {
      stats.requestsSent++; controls.push(command);
      if (!prepareSucceeds) throw new DriverCommandRefused("fixture-prepare-refused", "request");
      traceGeneration++;
      // Stop after successful prepare at its independent required source-plan validation.
      return { operation: "fixtureControl", ok: true, observation: { searchProviders: { version: 1, scenario: "all-providers",
        logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false, runs: [], pendingRunIds: [] } } };
    },
  };
  const runtime: SearchRuntime = { client: client as unknown as OwnedEvaluationClient,
    target: { type: "instance", id: target.windowId, generation: target.windowGeneration }, safety: { id: "cursor-fixture" } };
  return { runtime, cursors, controls, addFrame: () => generations.push(generations.at(-1)! + 1),
    retireUnexpectedly: () => { traceGeneration++; }, allowPrepare: () => { prepareSucceeds = true; } };
}
test("recipes acknowledge only retained pages across cases and preserve numeric privacy-safe frame references", async () => {
  const fixture = cursorRecipeFixture(); const contract = SEARCH_CASES[0]!; const schedule = searchContractSpec().schedules[0]!;
  const first = await runSearchSchedule(fixture.runtime, contract, schedule); fixture.addFrame();
  const second = await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(fixture.cursors).toEqual([undefined, { traceGeneration: 3, afterFrameGeneration: 3 }]);
  expect(first.issues).toContain("fixture-prepare-refused"); expect(second.issues).toContain("fixture-prepare-refused");
  const options = { mode: "fixture-redacted" as const, fixtureId: "search-frame-pool" };
  const receipts = sanitizeReceipt(annotateOwnedEvidence([first, second]), options).sanitized as Json[];
  const observed = receipts.flatMap(result => result.evidence.phases.flatMap((phase: Json) => (phase.completedFrames ?? []).map((reference: { frameRef: number }) =>
    reconstructSearchFrame(result.evidence.framePool, reference))));
  expect(observed.map(stamp => stamp.frame.target.frameGeneration)).toEqual([2, 3, 4]);
  expect(JSON.stringify(receipts)).not.toContain("private "); expect(fixture.controls).toHaveLength(2);
});
test("runtime retirement reads only unacknowledged frames and unmounts the actual latest authority", async () => {
  const fixture = cursorRecipeFixture(); const effects: Json[] = []; const unmounts: Json[] = [];
  await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!); fixture.addFrame();
  fixture.runtime.client.unmount = async (instance, expected) => { unmounts.push({ target: instance, expected }); };
  await retireSearchRuntime(fixture.runtime, effects);
  expect(fixture.cursors).toEqual([undefined, { traceGeneration: 3, afterFrameGeneration: 3 }]);
  expect(unmounts).toEqual([{ target: fixture.runtime.target, expected: { ...target, frameGeneration: 4 } }]);
  expect(effects[0]).toMatchObject({ id: "search-runtime-retirement", requestedCursor: { traceGeneration: 3, afterFrameGeneration: 3 },
    target: { frameGeneration: 4 }, unmounted: true });
  expect(effects[0]!.frameEvidence.completedFrames.map((frame: Json) => frame.frame.target.frameGeneration)).toEqual([4]);
});
test("runtime retirement retains its observation when unmount refuses and never refreshes or retries", async () => {
  const fixture = cursorRecipeFixture(); const effects: Json[] = []; let unmounts = 0;
  await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!); fixture.addFrame();
  fixture.runtime.client.unmount = async () => { unmounts++; throw new DriverCommandRefused("stale_target_identity", "request"); };
  await expect(retireSearchRuntime(fixture.runtime, effects)).rejects.toMatchObject({ code: "stale_target_identity" });
  expect(unmounts).toBe(1);
  expect(fixture.cursors).toEqual([undefined, { traceGeneration: 3, afterFrameGeneration: 3 }]);
  expect(effects[0]).toMatchObject({ target: { frameGeneration: 4 }, unmounted: false });
  expect(effects[0]!.frameEvidence.completedFrames.map((frame: Json) => frame.frame.target.frameGeneration)).toEqual([4]);
});
test("runtime retirement refuses a cursor from another target before inspection or unmount", async () => {
  const fixture = cursorRecipeFixture(); const effects: Json[] = []; let unmounts = 0;
  await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!);
  fixture.runtime.target = { ...fixture.runtime.target, generation: fixture.runtime.target.generation + 1 };
  fixture.runtime.client.unmount = async () => { unmounts++; };
  await expect(retireSearchRuntime(fixture.runtime, effects)).rejects.toMatchObject({ code: "frame_cursor_binding_mismatch" });
  expect(fixture.cursors).toEqual([undefined]); expect(unmounts).toBe(0); expect(effects).toEqual([]);
});
test("an over-budget frame page is not acknowledged or followed by prepare and survives later retirement as a failure", async () => {
  const fixture = cursorRecipeFixture(); const contract = SEARCH_CASES[0]!; const schedule = searchContractSpec().schedules[0]!;
  const failed = await runSearchSchedule(fixture.runtime, contract, { ...schedule, bounds: { ...schedule.bounds, retainedBytes: 40961 } });
  expect(failed.issues).toContain("search-evidence-byte-bound"); expect(fixture.controls).toHaveLength(0);
  const failure = failed.evidence as Json;
  expect(failure.framePool.frames).toEqual([]);
  expect(failure.failure.unretainedFramePage).toMatchObject({ traceGeneration: 3, latestFrameGeneration: 3, frameGenerations: [2, 3], acknowledged: false });
  fixture.allowPrepare(); const later = await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(fixture.cursors).toEqual([undefined, undefined]); expect(fixture.controls).toHaveLength(1);
  expect((later.evidence as Json).framePool.frames).toHaveLength(2);
  expect(failure.failure.unretainedFramePage.acknowledged).toBe(false); expect(failed.status).toBe("failed");
});
test("successful prepare clears acknowledged history but an unexpected lifetime never triggers a healing full read", async () => {
  const contract = SEARCH_CASES[0]!; const schedule = searchContractSpec().schedules[0]!;
  const prepared = cursorRecipeFixture(); prepared.allowPrepare();
  await runSearchSchedule(prepared.runtime, contract, schedule); await runSearchSchedule(prepared.runtime, contract, schedule);
  expect(prepared.cursors).toEqual([undefined, undefined]); expect(prepared.controls).toHaveLength(2);
  const stale = cursorRecipeFixture(); await runSearchSchedule(stale.runtime, contract, schedule); stale.retireUnexpectedly();
  const failed = await runSearchSchedule(stale.runtime, contract, schedule);
  expect(failed.issues).toContain("frame_cursor_stale"); expect(stale.controls).toHaveLength(1);
  expect(stale.cursors).toEqual([undefined, { traceGeneration: 3, afterFrameGeneration: 3 }]);
});

function captureCursorRecipeFixture(fault?: "conflict" | "oversize" | "ack-refused" | "stale-action" | "after-action" | "provider-transition" | "receipt-advance" | "stale-receipt-advance" | "receipt-capture" | "stale-receipt-capture", runs: SearchProviderRun[] = [], maxRetainedTraceBytes = 8 * 1024 * 1024) {
  let traceGeneration = 3; let generation = 0; let preparations = 0; let captured = false; let retiredBefore = 0;
  const cursors: (OwnedFrameCursor | undefined)[] = []; const captureCursors: (OwnedFrameCursor | undefined)[] = [];
  const actions: { action: Json; expected: AutomationTargetSnapshot }[] = [];
  const acknowledgements: OwnedFrameCursor[] = [];
  const waits: Json[] = [];
  const controls: Json[] = []; const captureExpectations: (ScheduledCapture | undefined)[] = []; const operations: string[] = [];
  const receiptCapture = fault === "receipt-capture" || fault === "stale-receipt-capture";
  const receiptAuthority = receiptCapture || fault === "receipt-advance" || fault === "stale-receipt-advance";
  const stats = { requestsSent: 0 }; let search = searchState(); const row = search.committedRows[0]!;
  const elements = { projectionVersion: 2, elements: search.committedRows.map(item => ({ role: "row", id: item.semanticId,
    index: item.selectableOrdinal, selectable: item.selectable, selected: item.semanticId === search.selectedSemanticId })) };
  const providers = { version: 1, scenario: "all-providers", logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false, runs, pendingRunIds: runs.filter(run => run.state === "held").map(run => run.id) };
  const identityAt = (value: number) => ({ ...target, frameGeneration: value, dataGeneration: target.dataGeneration + search.selectionRevision - 1,
    presentationRevision: target.presentationRevision + search.selectionRevision - 1 });
  const stamp = (value: number) => ({ ...retainedFrame(value), frame: { ...frame, target: identityAt(value) }, traceGeneration,
    mode: value === 2 ? "forced" : "scheduled", ...(receiptCapture ? { scheduledCapability: true } : {}), search,
    pixelEvidence: [], paintBindings: [{ kind: "mainSearchRow", id: row.semanticId, metadata: { stableKey: row.stableKey,
      contentFingerprint: row.contentFingerprint, selected: true, activatable: true, description: fault === "oversize" ? "private ".repeat(18000) : "private row" } }] });
  const trace = (cursor?: OwnedFrameCursor) => ({ traceGeneration, traceOverflow: false, afterFrameGeneration: cursor?.afterFrameGeneration ?? null,
    maxRetainedTraceBytes, retainedTraceBytes: Math.max(0, generation - Math.max(1, retiredBefore) + 1) * 32768,
    retiredBeforeFrameGeneration: retiredBefore, latestFrameGeneration: generation, completedFrames: Array.from({ length: generation }, (_, index) => index + 1)
      .filter(value => value >= retiredBefore && value > (cursor?.afterFrameGeneration ?? 0)).map(stamp) });
  const state = (cursor?: OwnedFrameCursor) => ({ targetIdentity: identityAt(generation), windowVisible: false,
    searchObservation: search, searchProviders: providers, mainWindowPreflight: { selectedResultKey: row.stableKey }, frameEvidence: trace(cursor) });
  const client = {
    driver: { stats },
    async inspect(_target: Json, cursor?: OwnedFrameCursor) {
      stats.requestsSent++; operations.push("inspect"); cursors.push(cursor ? { ...cursor } : undefined);
      if (fault === "provider-transition" && preparations === 1 && !captured) {
        providers.runs = providers.runs.map(run => ({ ...run, state: "completed", outcome: "success" })); providers.pendingRunIds = [];
      }
      return state(cursor);
    },
    async query() { stats.requestsSent++; operations.push("query"); if (captured && !receiptCapture) throw new DriverCommandRefused("fixture-after-capture", "request"); return elements; },
    async wait(_target: Json, condition: Json) {
      stats.requestsSent++; waits.push(condition); throw new DriverCommandRefused("fixture-source-admission", "waitFor");
    },
    async design(request: Json) {
      stats.requestsSent++; operations.push(request.control.operation);
      if (request.control.operation !== "prepare") {
        controls.push(request);
        if (fault === "stale-receipt-advance") { generation++; throw new DriverCommandRefused("stale_target_identity", "fixtureControl"); }
        if (!receiptAuthority) throw new DriverCommandRefused("fixture-after-capture", "request");
        providers.logicalTimeMs += request.control.milliseconds;
        return { operation: "fixtureControl", ok: true, observation: { searchProviders: providers } };
      }
      if (preparations++) throw new DriverCommandRefused("fixture-after-capture", "request");
      traceGeneration++;
      return { operation: "fixtureControl", ok: true, observation: { searchProviders: providers, suggestedInput: "a",
        sourcePlans: SEARCH_PROVIDERS.map(source => ({ source, scope: "root", workKind: "query-bound", input: source === "directory" ? "files: ~/fixture/" : "a" })),
        fileViewInputs: { full: "/fixture/", mini: "~/fixture/", preview: "~/fixture/image" } } };
    },
    async act(_target: Json, action: Json, expected: AutomationTargetSnapshot) {
      stats.requestsSent++; operations.push("act"); actions.push({ action, expected: { ...expected } });
      if (fault === "stale-action") { generation++; throw new DriverCommandRefused("stale_target_identity", "act"); }
      if (fault === "after-action") { generation++; return { actionReceipt: { applied: true } }; }
      if (receiptAuthority) {
        if (!receiptCapture || action.type !== "setInput") generation++;
        if (action.type === "select") search = { ...search, selectionIntent: { kind: "explicitAnchor", semanticId: row.semanticId }, selectionRevision: search.selectionRevision + 1 };
        const receipt = { requestId: `fixture-action-${actions.length}`, operationId: `fixture-action-${actions.length}:operation`,
          before: expected, after: identityAt(generation), dispatchCompleted: true, effect: { kind: action.type === "select" ? "stateChanged" : "noOp" } };
        return receiptCapture ? { actionReceipt: receipt } : { results: [{ actionReceipt: receipt }] };
      }
      throw new DriverCommandRefused("fixture-after-capture", "act");
    },
    async captureFrame(_target: Json, _includeImage: boolean, scheduled?: ScheduledCapture, cursor?: OwnedFrameCursor) {
      stats.requestsSent++; operations.push("capture"); captureCursors.push(cursor ? { ...cursor } : undefined); captureExpectations.push(scheduled);
      if (receiptCapture && captured && fault === "stale-receipt-capture") { generation++; throw new DriverCommandRefused("stale_target_identity", "captureFrame"); }
      generation = receiptCapture && captured ? generation : 2; captured = true;
      const captureTrace = trace(cursor);
      if (fault === "conflict") captureTrace.completedFrames[0]!.paintBindings[0]!.metadata.description = "different same-frame facts";
      const current = stamp(generation);
      return { operation: "captureFrame", ok: true, frame: current.frame,
        snapshot: receiptCapture ? { status: "captured", capture: { width: 800, height: 600 } } : { capture: {} }, state: state(cursor), elements,
        frameEvidence: { ...current, ...captureTrace }, phaseDurationsMs: {} };
    },
    async acknowledgeFrames(instance: Json, expected: AutomationTargetSnapshot, cursor: OwnedFrameCursor) {
      stats.requestsSent++; operations.push("acknowledge"); acknowledgements.push({ ...cursor });
      if (fault === "ack-refused") throw new DriverCommandRefused("fixture-acknowledgement-refused", "acknowledgeFrames");
      if (cursor.traceGeneration !== traceGeneration || cursor.afterFrameGeneration < retiredBefore || cursor.afterFrameGeneration > generation)
        throw new DriverCommandRefused("frame_cursor_invalid", "acknowledgeFrames");
      const retiredFrames = Math.max(0, cursor.afterFrameGeneration - Math.max(1, retiredBefore));
      retiredBefore = cursor.afterFrameGeneration;
      return { operation: "acknowledgeFrames", ok: true, target: instance, expected, acknowledgedCursor: { ...cursor },
        retiredFrames, retainedFrames: generation - retiredBefore + 1, retainedTraceBytes: (generation - retiredBefore + 1) * 32768 };
    },
  };
  const runtime: SearchRuntime = { client: client as unknown as OwnedEvaluationClient,
    target: { type: "instance", id: target.windowId, generation: target.windowGeneration }, safety: { id: "capture-cursor-fixture" } };
  return { runtime, cursors, captureCursors, actions, waits, controls, captureExpectations, operations, acknowledgements };
}
test("recipe capture retains both page copies and current facts before acknowledging the capture cursor", async () => {
  const fixture = captureCursorRecipeFixture(undefined, [], 4 * 2 * 32768); const contract = SEARCH_CASES[0]!; const schedule = searchContractSpec().schedules[0]!;
  const result = await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(result.issues).toContain("fixture-after-capture");
  expect(fixture.captureCursors).toEqual([{ traceGeneration: 4, afterFrameGeneration: 0 }]);
  expect(fixture.acknowledgements).toEqual([{ traceGeneration: 4, afterFrameGeneration: 2 }]);
  expect(fixture.cursors).toEqual([undefined, undefined]);
  expect(fixture.actions).toHaveLength(1); expect(fixture.actions[0]!.expected.frameGeneration).toBe(2);
  const phases = (result.evidence as Json).phases;
  expect(phases.find((entry: Json) => entry.id === "prepared-forced-baseline:state").reusedObservation).toBe(false);
  expect(phases.find((entry: Json) => entry.id === "before-input").reusedObservation).toBe(true);
  const phaseIndex = (result.evidence as Json).phases.findIndex((entry: Json) => entry.id === "prepared-forced-baseline");
  expect(phases.findIndex((entry: Json) => entry.id === "prepared-forced-baseline:frame-acknowledgement")).toBeGreaterThan(phaseIndex);
  const publicResult = sanitizeReceipt(annotateOwnedEvidence(result), { mode: "fixture-redacted", fixtureId: "search-frame-pool" }).sanitized as Json;
  const phase = publicResult.evidence.phases[phaseIndex];
  expect(publicResult.evidence.framePool.frames).toHaveLength(2);
  expect(phase.completedFrames.map((reference: { frameRef: number }) => reconstructSearchFrame(publicResult.evidence.framePool, reference).frame.target.frameGeneration)).toEqual([1, 2, 1, 2]);
  expect(reconstructSearchFrame(publicResult.evidence.framePool, phase.frameEvidence).frame.target.frameGeneration).toBe(2);
  expect(phase.completedFrames[0]).toEqual(phase.completedFrames[2]); expect(JSON.stringify(publicResult)).not.toContain("private ");
  await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(fixture.cursors.at(-1)).toEqual({ traceGeneration: 4, afterFrameGeneration: 2 });
});
test("recipe capture below native retention pressure preserves evidence without spending an acknowledgement request", async () => {
  const fixture = captureCursorRecipeFixture(undefined, [], 4 * 2 * 32768 + 1);
  const result = await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!);
  expect(result.issues).toContain("fixture-after-capture");
  expect(fixture.acknowledgements).toEqual([]);
  expect((result.evidence as Json).framePool.frames).toHaveLength(2);
  expect(fixture.actions).toHaveLength(1);
  expect(fixture.actions[0]!.expected.frameGeneration).toBe(2);
});
test.each([0, -1, Number.NaN, 32768])("recipe capture refuses inconsistent native retention capacity: %s", async capacity => {
  const fixture = captureCursorRecipeFixture(undefined, [], capacity);
  const result = await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!);
  expect(result.issues).toContain("missing-capability:frame-retention-pressure");
  expect(fixture.acknowledgements).toEqual([]);
  expect(fixture.actions).toEqual([]);
});
test("a conflicting or over-budget second capture history cannot acknowledge or partially retain the page", async () => {
  const contract = SEARCH_CASES[0]!; const schedule = searchContractSpec().schedules[0]!;
  for (const fault of ["conflict", "oversize"] as const) {
    const fixture = captureCursorRecipeFixture(fault);
    const result = await runSearchSchedule(fixture.runtime, contract, schedule); const evidence = result.evidence as Json;
    expect(result.issues).toContain(fault === "conflict" ? "conflicting-frame-evidence-reference" : "search-evidence-byte-bound");
    expect(evidence.framePool.frames).toEqual([]);
    expect(fixture.acknowledgements).toEqual([]);
    expect(evidence.failure.unretainedFramePage).toMatchObject({ traceGeneration: 4, afterFrameGeneration: 0,
      latestFrameGeneration: 2, frameGenerations: [2, 1, 2, 1, 2], acknowledged: false });
    await runSearchSchedule(fixture.runtime, contract, schedule);
    expect(fixture.cursors.at(-1)).toEqual({ traceGeneration: 4, afterFrameGeneration: 0 });
    expect(evidence.failure.unretainedFramePage.acknowledged).toBe(false);
  }
});
test("a refused native acknowledgement preserves all delivered frames and never retries", async () => {
  const fixture = captureCursorRecipeFixture("ack-refused", [], 4 * 2 * 32768);
  const result = await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!);
  expect(result.issues).toContain("fixture-acknowledgement-refused");
  expect(fixture.acknowledgements).toEqual([{ traceGeneration: 4, afterFrameGeneration: 2 }]);
  const evidence = result.evidence as Json;
  expect(evidence.framePool.frames).toHaveLength(2);
  expect(evidence.phases.some((phase: Json) => phase.id === "prepared-forced-baseline")).toBe(true);
  expect(evidence.phases.some((phase: Json) => phase.id.endsWith(":frame-acknowledgement"))).toBe(false);
  expect(fixture.actions).toEqual([]);
});
test("an intervening native change refuses the retained exact target without healing or retry", async () => {
  const fixture = captureCursorRecipeFixture("stale-action");
  const result = await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!);
  expect(result.issues).toContain("stale_target_identity");
  expect(fixture.actions).toHaveLength(1); expect(fixture.actions[0]!.expected.frameGeneration).toBe(2);
  expect(fixture.cursors).toEqual([undefined, undefined]); expect(fixture.captureCursors).toHaveLength(1);
  expect((result.evidence as Json).phases.find((entry: Json) => entry.id === "before-input").reusedObservation).toBe(true);
});
test("a successful transport invalidates cached observations before the next exact control", async () => {
  const fixture = captureCursorRecipeFixture("after-action");
  const result = await runSearchSchedule(fixture.runtime, { ...SEARCH_CASES[0]!, inputRoute: "setInput" }, searchContractSpec().schedules[0]!);
  expect(result.issues).toContain("fixture-after-capture"); expect(fixture.actions).toHaveLength(1);
  expect(fixture.cursors).toEqual([undefined, undefined, { traceGeneration: 4, afterFrameGeneration: 2 }]);
  const evidence = result.evidence as Json;
  const phase = evidence.phases.find((entry: Json) => entry.id === "before-search-advance");
  expect(phase.reusedObservation).toBe(false);
  expect(phase.completedFrames.map((reference: { frameRef: number }) => reconstructSearchFrame(evidence.framePool, reference).frame.target.frameGeneration)).toEqual([3]);
});
test("the actual post-action receipt binds an immediate advance before deferred frame drainage", async () => {
  const fixture = captureCursorRecipeFixture("receipt-advance");
  const result = await runSearchSchedule(fixture.runtime, { ...SEARCH_CASES[0]!, inputRoute: "setInput" }, searchContractSpec().schedules[0]!);
  expect(result.issues).toContain("fixture-after-capture");
  const evidence = result.evidence as Json; const receipt = evidence.actions[0].receipt;
  expect(receipt.after.frameGeneration).toBe(3); expect(fixture.actions[0]!.expected.frameGeneration).toBe(2);
  expect(fixture.controls).toHaveLength(1); expect(fixture.controls[0]!.expected).toEqual(receipt.after);
  const actionAt = fixture.operations.indexOf("act");
  expect(fixture.operations.slice(actionAt, actionAt + 4)).toEqual(["act", "advance", "inspect", "query"]);
  expect(evidence.phases.find((phase: Json) => phase.id === "before-search-advance")).toMatchObject({ targetIdentity: receipt.after,
    requestId: receipt.requestId, operationId: receipt.operationId, reusedActionReceipt: true, deferredFrameDrain: true });
  expect(evidence.framePool.frames.map((_entry: Json, index: number) => reconstructSearchFrameFacts(evidence.framePool, index).frame.target.frameGeneration).sort()).toEqual([1, 2, 3]);
});
test("a stale receipt-bound advance refuses without inspection or retry and leaves unseen frames unacknowledged", async () => {
  const fixture = captureCursorRecipeFixture("stale-receipt-advance"); const contract = { ...SEARCH_CASES[0]!, inputRoute: "setInput" as const }; const schedule = searchContractSpec().schedules[0]!;
  const result = await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(result.issues).toContain("stale_target_identity"); expect(fixture.controls).toHaveLength(1);
  expect(fixture.controls[0]!.expected.frameGeneration).toBe(3);
  expect(fixture.operations.slice(fixture.operations.indexOf("act"))).toEqual(["act", "advance"]);
  expect(fixture.cursors).toEqual([undefined, undefined]); expect((result.evidence as Json).framePool.frames).toHaveLength(2);
  const later = await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(fixture.cursors.at(-1)).toEqual({ traceGeneration: 4, afterFrameGeneration: 2 });
  const retained = (later.evidence as Json).framePool;
  expect(retained.frames.map((_entry: Json, index: number) => reconstructSearchFrameFacts(retained, index).frame.target.frameGeneration)).toEqual([3, 4]);
});
test("a receipt-bound anchor retains its complete capture before asserting the actual captured state", async () => {
  const fixture = captureCursorRecipeFixture("receipt-capture"); const contract = SEARCH_CASES.find(item => item.id === "semantic-anchor-current-first")!;
  const schedule = searchContractSpec().schedules.find(item => item.caseId === contract.id && item.recipe.kind === "primary")!;
  const result = await runSearchSchedule(fixture.runtime, contract, schedule); const evidence = result.evidence as Json;
  expect(result.issues).toContain("fixture-source-admission"); expect(fixture.captureCursors).toHaveLength(2);
  expect(fixture.captureExpectations[1]!.expected).toEqual(evidence.actions[1].receipt.after);
  expect(fixture.captureExpectations[1]!.expected.dataGeneration).toBe(3);
  const selectAt = fixture.operations.lastIndexOf("act"); expect(fixture.operations[selectAt + 1]).toBe("capture");
  const capturedAt = evidence.phases.findIndex((phase: Json) => phase.id === "explicit-anchor");
  const stateAt = evidence.phases.findIndex((phase: Json) => phase.id === "explicit-anchor:state");
  expect(stateAt).toBeGreaterThan(capturedAt);
  expect(evidence.phases[stateAt]).toMatchObject({ selectionIntent: { kind: "explicitAnchor" }, selectionRevision: 2 });
  for (const id of ["explicit-anchor:draw-owner-join", "explicit-anchor:state:collector-version", "explicit-anchor:state:preflight-selection"])
    expect(result.assertions.find(assertion => assertion.id === id)?.pass).toBe(true);
  expect(evidence.framePool.frames.map((_entry: Json, index: number) => reconstructSearchFrameFacts(evidence.framePool, index).frame.target.frameGeneration).sort()).toEqual([1, 2, 3]);
});
test("stale receipt-bound scheduled capture cannot heal its target or acknowledge unseen frames", async () => {
  const fixture = captureCursorRecipeFixture("stale-receipt-capture"); const contract = SEARCH_CASES.find(item => item.id === "semantic-anchor-current-first")!;
  const schedule = searchContractSpec().schedules.find(item => item.caseId === contract.id && item.recipe.kind === "primary")!;
  const result = await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(result.issues).toContain("stale_target_identity"); expect(fixture.captureCursors).toHaveLength(2);
  expect(fixture.captureExpectations[1]!.expected.frameGeneration).toBe(3);
  expect(fixture.operations.slice(fixture.operations.lastIndexOf("act"))).toEqual(["act", "capture"]);
  expect((result.evidence as Json).framePool.frames).toHaveLength(2);
  const later = await runSearchSchedule(fixture.runtime, contract, schedule);
  expect(fixture.cursors.at(-1)).toEqual({ traceGeneration: 4, afterFrameGeneration: 2 });
  const retained = (later.evidence as Json).framePool;
  expect(retained.frames.map((_entry: Json, index: number) => reconstructSearchFrameFacts(retained, index).frame.target.frameGeneration)).toEqual([3, 4]);
});
async function recordedCohortFixture() {
  const fixture = captureCursorRecipeFixture();
  const schedule = searchContractSpec().schedules.find(item => item.recipe.kind === "cohort" && item.recipe.cohort === 0 && item.recipe.order[0] === "tab-hoist")!;
  const result = await runSearchSchedule(fixture.runtime, SEARCH_CASES.find(item => item.id === schedule.caseId)!, schedule);
  return { fixture, result, evidence: result.evidence as Json };
}
test("same-source entry retains one exact observation and still invokes the native admission wait", async () => {
  const { fixture, result, evidence } = await recordedCohortFixture();
  expect(result.issues).toContain("fixture-source-admission");
  expect(fixture.cursors).toEqual([undefined, undefined]); expect(fixture.captureCursors).toHaveLength(1);
  expect(fixture.waits).toEqual([{ type: "searchProvider", source: "tabs", query: searchState().query, afterRunId: 0 }]);
  const indices = evidence.phases.flatMap((phase: Json, index: number) => phase.id === "source-input-current" ? [index] : []);
  expect(indices).toHaveLength(2);
  const original = evidence.phases[indices[0]]; const alias = evidence.phases[indices[1]];
  expect(alias).toEqual({ id: "source-input-current", observationRef: indices[0], reusedObservation: true, providerRuns: [], providerRunsAreChanges: true, completedFrames: [] });
  expect(reconstructSearchObservationPhase(evidence.phases, indices[1])).toEqual({ ...original, providerRuns: [], providerRunsAreChanges: true, completedFrames: [] });
  expect(() => validateSearchObservationPhases(evidence.phases)).not.toThrow();
  expect(result.assertions.find(assertion => assertion.id === "source-input-current:collector-version")?.pass).toBe(true);
  expect(result.assertions.find(assertion => assertion.id === "source-input-current:preflight-selection")?.pass).toBe(true);
  const expanded = { ...result, evidence: { ...evidence, phases: evidence.phases.map((_phase: Json, index: number) => reconstructSearchObservationPhase(evidence.phases, index)) } };
  expect(Buffer.byteLength(JSON.stringify(result))).toBeLessThan(Buffer.byteLength(JSON.stringify(expanded)));
});
test("observation aliases preserve private descriptors without replaying source deltas", async () => {
  const { result, evidence } = await recordedCohortFixture();
  const publicResult = sanitizeReceipt(annotateOwnedEvidence(result), { mode: "fixture-redacted", fixtureId: "search-frame-pool" }).sanitized as Json;
  const index = evidence.phases.findIndex((phase: Json) => Object.hasOwn(phase, "observationRef"));
  const phases = publicResult.evidence.phases; const alias = phases[index]; const original = phases[alias.observationRef];
  expect(() => validateSearchObservationPhases(phases)).not.toThrow();
  expect(reconstructSearchObservationPhase(phases, index)).toEqual({ ...original, id: alias.id, reusedObservation: true, providerRuns: [], providerRunsAreChanges: true, completedFrames: [] });
  const renamed = [...phases, { ...alias, id: "before-input" }];
  expect(reconstructSearchObservationPhase(renamed, renamed.length - 1).id).toBe("before-input");
  expect(typeof original.selectionFingerprint).toBe("object"); expect(JSON.stringify(publicResult)).not.toContain("private ");
  expect(Object.hasOwn(alias, "observationRef")).toBe(true);
});
test("observation references refuse forward, chained, cross-kind and authority-overriding aliases", async () => {
  const { evidence } = await recordedCohortFixture(); const phases: Json[] = evidence.phases;
  const index = phases.findIndex(phase => Object.hasOwn(phase, "observationRef")); const alias = phases[index]!;
  for (const reference of [-1, 1.5, index, index + 1, phases.length, null]) {
    const invalid = [...phases]; invalid[index] = { ...alias, observationRef: reference };
    expect(() => validateSearchObservationPhases(invalid)).toThrow("dangling-search-observation-reference");
  }
  for (const patch of [{ query: searchState().query }, { providerRuns: [{ id: 1 }] }, { completedFrames: [{ frameRef: 0 }] }, { reusedObservation: false }, { unknown: true }]) {
    const invalid = [...phases]; invalid[index] = { ...alias, ...patch };
    expect(() => validateSearchObservationPhases(invalid)).toThrow("invalid-search-observation-reference");
  }
  const crossKind = [...phases]; crossKind[index] = { ...alias, observationRef: phases.findIndex(phase => phase.id === "prepare") };
  expect(() => validateSearchObservationPhases(crossKind)).toThrow("invalid-search-observation-origin");
  expect(() => validateSearchObservationPhases([...phases, { ...alias, observationRef: index }])).toThrow("invalid-search-observation-origin");
  const missingAuthority = structuredClone(phases); delete missingAuthority[alias.observationRef]!.providerRunsHash;
  expect(() => reconstructSearchObservationPhase(missingAuthority, index)).toThrow("invalid-search-observation-origin");
});
test("prepare retains the initial provider ledger once and baseline records only actual transitions", async () => {
  const held: SearchProviderRun = { id: 1, kind: "worker", source: "tabs", query: "a", generation: 1, state: "held", publicationPolicy: "visible" };
  for (const fault of [undefined, "provider-transition"] as const) {
    const fixture = captureCursorRecipeFixture(fault, [held]);
    const result = await runSearchSchedule(fixture.runtime, SEARCH_CASES[0]!, searchContractSpec().schedules[0]!);
    expect(result.issues).toContain("fixture-after-capture");
    const phases = (result.evidence as Json).phases;
    expect(phases.find((phase: Json) => phase.id === "prepare").providers.runs).toEqual([held]);
    const baseline = phases.find((phase: Json) => phase.id === "prepared-forced-baseline:state");
    expect(baseline.providerRunsAreChanges).toBe(true); expect(baseline.providerRunCount).toBe(1);
    expect(baseline.providerRuns).toEqual(fault ? [{ ...held, state: "completed", outcome: "success" }] : []);
    expect(result.assertions.find(assertion => assertion.id === "prepared-forced-baseline:state:provider-runs-retained")?.pass).toBe(true);
  }
});
test("capture pixel references preserve native samples and reject cross-frame or mixed authority", async () => {
  const { evidence } = await recordedCohortFixture();
  const phase = evidence.phases.find((item: Json) => item.id === "prepared-forced-baseline");
  expect(phase.pixels).toEqual({ nativeSamplesFrame: phase.frameEvidence });
  expect(reconstructSearchCapturePixels(evidence.framePool, phase)).toEqual({ frameGeneration: 2, nativeSamples: [], captureHash: undefined });
  for (const pixels of [{ nativeSamplesFrame: { frameRef: phase.frameEvidence.frameRef === 0 ? 1 : 0 } }, { nativeSamplesFrame: { frameRef: 99 } },
    { nativeSamplesFrame: { ...phase.frameEvidence, extra: true } }, { ...phase.pixels, nativeSamples: [] },
    { ...phase.pixels, captureHash: "other-image" }, { ...phase.pixels, frameGeneration: 9 }, { ...phase.pixels, sampled: null }])
    expect(() => reconstructSearchCapturePixels(evidence.framePool, { ...phase, pixels })).toThrow("invalid-search-pixel-reference");
  const noSamples = structuredClone(evidence.framePool); delete noSamples.frames[phase.frameEvidence.frameRef].factRefs.pixelEvidence;
  expect(() => reconstructSearchCapturePixels(noSamples, phase)).toThrow("invalid-search-pixel-origin");
});
function capture(): OwnedFrameCapture {
  return { operation: "captureFrame", ok: true, frame, snapshot: { source: "gpuiRenderReadback", scope: "liveAutomationWindowRenderReadback", status: "captured", frameIdentity: frame,
    capture: { width: 800, height: 600, hiDpi: true }, limitation: "owned-only" }, state: {}, elements: {}, layout: {}, phaseDurationsMs: {},
    frameEvidence: { scheduledCapability: true, mode: "scheduled", notificationEpoch: 6, traceGeneration: 3, traceOverflow: false, completedFrames: [{ frame, traceGeneration: 3 }], nativeWindow: { visible: false }, nativeWindowActive: false } };
}
test("natural evidence refuses forced paint, absent notification, trace overflow, stale state and blank readback", () => {
  expect(naturalEvidenceIssues(capture(), expectation)).toEqual([]);
  const forced = capture(); forced.frameEvidence!.mode = "forced";
  expect(naturalEvidenceIssues(forced, expectation)).toContain("missing-scheduled-frame");
  const quiet = capture(); quiet.frameEvidence!.notificationEpoch = 5;
  expect(naturalEvidenceIssues(quiet, expectation)).toContain("missing-scheduled-frame");
  const overflow = capture(); overflow.frameEvidence!.traceOverflow = true;
  expect(naturalEvidenceIssues(overflow, expectation)).toContain("frame-trace-overflow-or-missing");
  const noStamp = capture(); noStamp.frameEvidence!.completedFrames = [];
  expect(naturalEvidenceIssues(noStamp, expectation)).toContain("completed-stamp-missing");
  const stale = capture(); stale.frame = { ...frame, target: { ...target, dataGeneration: 1 } };
  expect(naturalEvidenceIssues(stale, expectation)).toContain("frame-state-stale");
  const blank = capture(); blank.snapshot = { ...blank.snapshot, status: "blankImageRejected" };
  expect(naturalEvidenceIssues(blank, expectation)).toContain("qualified-readback-missing");
});

test("an acknowledged current stamp resolves only from its exact retained frame and trace lifetime", () => {
  const store = new SearchFrameStore();
  store.retainWithin([retainedFrame(2)], refs => ({ framePool: store.pool, refs }), 131072 - 40960);
  const delta = capture(); delta.frameEvidence!.completedFrames = [];
  delta.frameEvidence!.afterFrameGeneration = 2; delta.frameEvidence!.latestFrameGeneration = 2;
  delta.frameEvidence!.frame = frame;
  expect(naturalEvidenceIssues(delta, expectation)).toContain("completed-stamp-missing");
  expect(naturalEvidenceIssues(delta, expectation, store.pool)).toEqual([]);
  for (const change of [
    (facts: Json) => { facts.traceGeneration++; },
    (facts: Json) => { facts.frame.target.windowId = "foreign"; },
    (facts: Json) => { facts.frame.target.windowGeneration++; },
    (facts: Json) => { facts.frame.target.frameGeneration++; },
    (facts: Json) => { facts.frame.target.dataGeneration++; },
    (facts: Json) => { facts.frame.processInstanceId = "foreign"; },
    (facts: Json) => { facts.frame.sessionGeneration = "foreign"; },
    (facts: Json) => { facts.frame.nativeWindowId = 999; },
    (facts: Json) => { facts.frame.binarySha256 = "c".repeat(64); },
  ]) {
    const foreign = structuredClone(store.pool); change(foreign.frames[0]!.facts);
    expect(naturalEvidenceIssues(delta, expectation, foreign)).toContain("completed-stamp-missing");
  }
  const forced = structuredClone(delta); forced.frameEvidence!.mode = "forced";
  expect(naturalEvidenceIssues(forced, expectation, store.pool)).toContain("missing-scheduled-frame");
  const oldNotification = structuredClone(delta); oldNotification.frameEvidence!.notificationEpoch = expectation.afterNotificationEpoch;
  expect(naturalEvidenceIssues(oldNotification, expectation, store.pool)).toContain("missing-scheduled-frame");
});
test("a returned same-number stamp from another owner or trace cannot replace the exact completion", () => {
  const foreign = capture(); foreign.frameEvidence!.completedFrames[0].frame = { ...frame, processInstanceId: "foreign" };
  expect(naturalEvidenceIssues(foreign, expectation)).toContain("completed-stamp-missing");
  const oldTrace = capture(); oldTrace.frameEvidence!.completedFrames[0].traceGeneration = 2;
  expect(naturalEvidenceIssues(oldTrace, expectation)).toContain("completed-stamp-missing");
});

test("selection evidence rejects empty, transparent or wrong-color retained pixels", () => {
  expect(selectionPixelIssues([{ x: 3, y: 4, r: 0x72, g: 0xc1, b: 0xa8, a: 255 }], 0x72c1a8)).toEqual([]);
  expect(selectionPixelIssues([], 0x72c1a8)).toContain("missing-capability:selection-pixels");
  expect(selectionPixelIssues([{ x: 3, y: 4, r: 0, g: 0, b: 0, a: 0 }], 0x72c1a8)).toContain("selected-marker-pixels-disagree");
});

test("admission reserves lifecycle resources and keeps actual comparison identities in one runtime", () => {
  const schedules = searchContractSpec().schedules;
  const shards = partitionSearchSchedules(schedules);
  expect(shards.flatMap(shard => shard.schedules.map(schedule => schedule.id))).toEqual(schedules.map(schedule => schedule.id));
  const owners = new Map<string, number>();
  for (const shard of shards) {
    expect(shard.bounds.requests).toBeLessThanOrEqual(4096 - 128);
    expect(shard.bounds.frames).toBeLessThanOrEqual(2048 - 32);
    expect(shard.bounds.wallMilliseconds).toBeLessThanOrEqual(600000 - 30000);
    expect(shard.bounds.logicalMilliseconds).toBeLessThanOrEqual(599000);
    for (const schedule of shard.schedules) {
      const group = searchScheduleComparisonGroup(schedule);
      if (!group) continue;
      if (owners.has(group)) expect(owners.get(group)).toBe(shard.index); else owners.set(group, shard.index);
    }
  }
  const pair = schedules.filter(schedule => searchScheduleComparisonGroup(schedule) === "pair:directory+files");
  expect(() => partitionSearchSchedules(pair.map(schedule => ({ ...schedule, bounds: { ...schedule.bounds, requests: 1500 } })))).toThrow("unadmittable-search-comparison");
});
test("output admission splits five independent interactions and isolates every indivisible comparison family", () => {
  const spec = searchContractSpec();
  const firstFive = spec.schedules.slice(0, 5);
  expect(partitionSearchSchedules(firstFive).map(shard => shard.schedules.map(schedule => schedule.id))).toEqual([
    firstFive.slice(0, 3).map(schedule => schedule.id), firstFive.slice(3).map(schedule => schedule.id),
  ]);
  const timings = spec.schedules.filter(schedule => schedule.recipe.kind === "timing").slice(0, 3);
  expect(partitionSearchSchedules(timings).map(shard => shard.schedules.length)).toEqual([1, 1, 1]);
  const shards = partitionSearchSchedules(spec.schedules);
  expect(shards.flatMap(shard => shard.schedules)).toHaveLength(spec.schedules.length);
  for (const shard of shards) {
    const group = searchScheduleComparisonGroup(shard.schedules[0]!);
    if (group) {
      expect(shard.schedules.every(schedule => searchScheduleComparisonGroup(schedule) === group)).toBe(true);
      expect(shard.schedules.map(schedule => schedule.id)).toEqual(spec.schedules.filter(schedule => searchScheduleComparisonGroup(schedule) === group).map(schedule => schedule.id));
      expect(shard.schedules).toHaveLength(group.startsWith("cohort:") ? 6 : 3);
    } else if (shard.schedules.length > 1) {
      expect(shard.schedules.length).toBeLessThanOrEqual(spec.admission.outputPacking.maxIndependentSchedules);
      expect(shard.bounds.requests).toBeLessThanOrEqual(spec.admission.outputPacking.maxIndependentRequests);
      expect(shard.schedules.every(schedule => searchScheduleComparisonGroup(schedule) === null)).toBe(true);
    }
  }
});

function searchState(): SearchObservation {
  const makeRow = (stableKey: string, groupedIndex: number, selectableOrdinal: number | null) => ({ stableKey, groupedIndex, selectableOrdinal,
    semanticId: `main-list-row:v2:${createHash("sha256").update(stableKey).digest("hex")}`, contentFingerprint: "a".repeat(64),
    subjectKind: "searchResult", selectable: selectableOrdinal !== null, activatable: selectableOrdinal !== null });
  const first = makeRow("fixture/first", 1, 0); const reserved = makeRow("fixture/reserved", 2, null);
  return { version: 1, query: { lifetime: 1, revision: 1, scopeRevision: 1 }, computedQuery: { lifetime: 1, revision: 1, scopeRevision: 1 },
    pending: false, rawInput: "a", computedInput: "a", resultRevision: 1, selectionRevision: 1, viewportRevision: 1,
    selectionIntent: { kind: "automaticAnchor", semanticId: first.semanticId }, selectionArmed: true, viewportIntent: "followSelection", reconciliationReason: null,
    selectedSemanticId: first.semanticId, selectedOrdinal: 0, selectedIndex: 1, committedRows: [first, reserved], publication: null, publicationError: null, providers: {} };
}
test("query ABA, semantic identity, inert eligibility and explicit anchor cannot be inferred from text or ordinals", () => {
  const state = searchState(); expect(searchObservationIssues(state)).toEqual([]);
  const aba = structuredClone(state); aba.query.revision = 3;
  expect(searchObservationIssues(aba)).toContain("query-current-stamp-disagreement");
  aba.pending = true; expect(searchObservationIssues(aba)).toEqual([]);
  const indexed = structuredClone(state); indexed.committedRows[0]!.semanticId = "choice:0:legacy";
  expect(searchObservationIssues(indexed)).toContain("invalid-canonical-row-identity");
  const inert = structuredClone(state); inert.committedRows[1]!.selectableOrdinal = 1;
  expect(searchObservationIssues(inert)).toContain("eligibility-ordinal-disagreement");
  const moved = structuredClone(state); moved.selectionIntent = { kind: "explicitAnchor", semanticId: moved.committedRows[1]!.semanticId };
  expect(searchObservationIssues(moved)).toContain("explicit-anchor-disagreement");
  const elements = { elements: state.committedRows.map(row => ({ elementType: "choice", semanticId: row.semanticId, index: row.selectableOrdinal,
    selectable: row.selectable, selected: row.semanticId === state.selectedSemanticId })) };
  expect(searchObservationIssues(state, elements)).toEqual([]);
  elements.elements[0]!.selected = false;
  expect(searchObservationIssues(state, elements)).toContain("semantic-selection-disagreement");
});
test("native unarmed recents are distinct from absent rows and cannot expose effective selection", () => {
  const unarmed = searchState(); unarmed.selectionArmed = false;
  expect(searchObservationIssues(unarmed)).toContain("unarmed-effective-selection");
  unarmed.selectedSemanticId = null; unarmed.selectedOrdinal = null; unarmed.selectedIndex = null;
  expect(unarmed.committedRows.some(row => row.selectable)).toBe(true);
  expect(searchObservationIssues(unarmed)).toEqual([]);
});
test("flushed Enter dispatch uses the current canonical subject rather than a guard refusal as query proof", () => {
  const current = searchState(); const row = current.committedRows[0]!;
  current.query.revision = 2; current.computedQuery.revision = 2;
  current.dispatch = { query: { ...current.query }, stableKey: row.stableKey, contentFingerprint: row.contentFingerprint, status: "refused", reason: "owned_effect_refused" };
  expect(dispatchBindingIssues(current)).toEqual([]);
  const oldQuery = structuredClone(current); oldQuery.dispatch!.query.revision = 1;
  expect(dispatchBindingIssues(oldQuery)).toContain("stale-dispatch-subject");
  const oldContent = structuredClone(current); oldContent.dispatch!.contentFingerprint = "b".repeat(64);
  expect(dispatchBindingIssues(oldContent)).toContain("stale-dispatch-subject");
  const rebound = structuredClone(current); rebound.dispatch!.stableKey = "fixture/reserved";
  expect(dispatchBindingIssues(rebound)).toContain("stale-dispatch-subject");
  const requested = structuredClone(current); requested.dispatch!.status = "dispatchRequested";
  expect(dispatchBindingIssues(requested)).toContain("dispatch-not-terminal");
  const empty = searchState(); empty.selectedSemanticId = null; empty.selectedOrdinal = null; empty.dispatch = null;
  expect(dispatchBindingIssues(empty)).toEqual([]);
  empty.dispatch = current.dispatch;
  expect(dispatchBindingIssues(empty)).toContain("dispatch-without-current-subject");
});
test("owned copy proof requires one completed UTF-8 value write even when the text repeats", () => {
  const text = "四";
  const first: OwnedCopySinkObservation = { text, receipt: { destination: "ownedProcessLocal", byteLength: Buffer.byteLength(text),
    sha256: createHash("sha256").update(text).digest("hex"), revision: 1 } };
  expect(copySinkIssues(null, first, text)).toEqual([]);
  expect(copySinkIssues(first, first, text)).toContain("owned-copy-count-did-not-advance-once");
  const second = structuredClone(first); second.receipt.revision++;
  expect(copySinkIssues(first, second, text)).toEqual([]);
  expect(copySinkIssues(undefined, first, text)).toContain("missing-prior-copy-sink-revision");
  expect(copySinkIssues(null, null, text)).toContain("missing-owned-copy-completion");
  const wrongBytes = structuredClone(first); wrongBytes.receipt.byteLength = text.length;
  expect(copySinkIssues(null, wrongBytes, text)).toContain("owned-copy-value-disagreement");
  const oldValue = structuredClone(first); oldValue.text = "old";
  expect(copySinkIssues(null, oldValue, text)).toContain("owned-copy-value-disagreement");
});
test("same-key changed content alters actual ranking fingerprints and stale paint bindings fail", () => {
  const state = searchState(); const row = state.committedRows[0]!;
  const bindings = [{ kind: "mainSearchRow", id: row.semanticId, metadata: { stableKey: row.stableKey, contentFingerprint: row.contentFingerprint, selected: true, activatable: true } },
    { kind: "mainSearchPreview", id: row.semanticId, metadata: { stableKey: row.stableKey, contentFingerprint: row.contentFingerprint, query: state.computedQuery } }];
  expect(paintBindingIssues(state, bindings)).toEqual([]);
  const updated = structuredClone(state); updated.committedRows[0]!.contentFingerprint = "b".repeat(64); updated.resultRevision++;
  expect(rankingFingerprint(updated)).not.toBe(rankingFingerprint(state));
  expect(paintBindingIssues(updated, bindings)).toContain("paint-row-projection-disagreement");
  expect(paintBindingIssues(updated, bindings)).toContain("paint-preview-subject-disagreement");
  const newLifetime = structuredClone(state); newLifetime.query.lifetime++; newLifetime.computedQuery.lifetime++;
  expect(rankingFingerprint(newLifetime)).toBe(rankingFingerprint(state));
  const otherScope = structuredClone(state); otherScope.rawInput = otherScope.computedInput = "@file:example.invalid";
  expect(rankingFingerprint(otherScope)).not.toBe(rankingFingerprint(state));
});
test("source plans are complete and synchronous admission is not a worker outcome", () => {
  const plans = SEARCH_PROVIDERS.map(source => ({ source, input: "example.invalid", scope: "root" as const, workKind: "query-bound" as const }));
  expect(sourcePlanIssues(plans)).toEqual([]);
  expect(sourcePlanIssues(plans.slice(1))).toContain("missing-capability:complete-source-plans");
  expect(sourcePlanIssues([...plans.slice(1), plans[1]!])).toContain("missing-capability:complete-source-plans");
  const admission: SearchProviderRun = { ...held, id: 32, kind: "sourceChange", source: "brain-lexical", state: "awaiting-admission", publicationPolicy: null,
    outcome: null, resultCount: null, capabilityRefusal: "synchronous_source_has_no_worker" };
  const observation = { version: 1, scenario: "disconnect", logicalTimeMs: 0, displayUnixMs: 1_777_597_200_000, retired: false, overflow: false, runs: [admission], pendingRunIds: [] };
  expect(providerObservationIssues(observation)).toEqual([]);
  expect(providerObservationIssues({ ...observation, runs: [{ ...admission, outcome: "unavailable" }] })).toContain("invalid-provider-run");
  expect(providerObservationIssues({ ...observation, runs: [{ ...admission, publicationPolicy: "capability-refused" }] })).toContain("invalid-provider-run");
});
interface ComparisonFixture extends SearchScheduleResult { evidence: { orderComparisons: SearchOrderReceipt[] } }
function comparisonResults(): ComparisonFixture[] {
  const schedules = searchContractSpec().schedules.filter(schedule => !schedule.structuralNotApplicable && searchScheduleComparisonGroup(schedule) === "pair:directory+files");
  const order = (schedule: typeof schedules[number]) => schedule.recipe.kind === "same-turn" ? "same-turn" : schedule.providers.join("-then-");
  return schedules.map(schedule => ({ id: schedule.id, caseId: schedule.caseId, status: "failed", executed: true, assertions: [], issues: [],
    evidence: { orderComparisons: ["automatic", "deliberate-when-eligible"].map(intent => ({ key: `pair:directory+files:${intent}`, order: order(schedule),
      expectedOrders: schedules.map(order), fingerprint: "a".repeat(64) })) } }));
}
test("final order comparison requires actual corresponding receipts for both intents", () => {
  const equal = comparisonResults(); compareSearchOrders(equal);
  expect(equal.every(result => result.assertions.find(assertion => assertion.id === "final-candidates-equal")?.pass)).toBe(true);
  const missing = comparisonResults().slice(1); compareSearchOrders(missing);
  expect(missing.every(result => result.issues.some(issue => issue.startsWith("missing-order-comparison:")))).toBe(true);
  const changed = comparisonResults(); changed[0]!.evidence.orderComparisons[0]!.fingerprint = "b".repeat(64);
  compareSearchOrders(changed); expect(changed.every(result => result.assertions.some(assertion => assertion.id === "final-candidates-equal" && !assertion.pass))).toBe(true);
  const forged = comparisonResults(); forged[0]!.evidence.orderComparisons[0]!.expectedOrders = ["files-then-directory"];
  compareSearchOrders(forged); expect(forged[0]!.issues).toContain("invalid-order-comparison-receipt");
});

test("probe run reaches argument validation without a cyclic module wait", () => {
  const result = Bun.spawnSync([process.execPath, `${import.meta.dir}/launcher-selection-stability-probe.ts`, "run", "--artifact"], {
    timeout: 5000,
    env: { ...process.env, SCRIPT_KIT_NONINTERACTIVE: "1" },
  });
  expect(result.exitCode).toBe(1);
  expect(result.stderr.toString()).toContain("artifact_reference_and_fresh_output_required");
}, 10000);

function shardRetentionFixture(closed = true, catalog: Json = {}) {
  const events: string[] = []; let launches = 0;
  const cleanup = { ...emptyOwnedCleanup(), resourcesAcquired: true, ownedWindowsClosed: true, closed,
    streamsDrained: closed, failureCodes: closed ? [] : ["fixture-drain-failed"] };
  const launch = spyOn(OwnedEvaluationClient, "launch").mockImplementation(async () => {
    const shard = launches++; events.push(`launch:${shard}`);
    return { cleanup, driver: { observedReceivedOutputBytes: 123, maxOutputBytes: 67108864 },
      async discover() { return catalog; },
      async close() { events.push(`close:${shard}`); return cleanup; },
    } as unknown as OwnedEvaluationClient;
  });
  const reference = {} as Parameters<typeof runSearchJourney>[0]; const claim = {} as Parameters<typeof runSearchJourney>[1];
  return { launch, events, cleanup, reference, claim };
}
test("a failed launch preserves its protocol cause and cleanup evidence", async () => {
  const fixture = shardRetentionFixture();
  const cause = new DriverProtocolError("response_timeout");
  fixture.launch.mockImplementation(async () => { throw new DriverLifecycleError("Driver launch failed", fixture.cleanup, { cause }); });
  try {
    const journey = await runSearchJourney(fixture.reference, fixture.claim, { caseId: "automatic-higher-arrival" });
    expect(journey.effects[0]).toMatchObject({ id: "search-shard-failure", code: "response_timeout", name: "DriverProtocolError",
      diagnostic: { message: "response_timeout" } });
    const result = journey.coverage.results.find(result => result.caseId === "automatic-higher-arrival")!;
    expect(result.executed).toBe(false); expect(result.issues).toContain("response_timeout");
    expect(journey.cleanup.closed).toBe(true); expect(journey.pass).toBe(false);
  } finally { fixture.launch.mockRestore(); }
});
test("shard retention runs after close with finalized results and leaves only bound summaries", async () => {
  const fixture = shardRetentionFixture(); const retained: SearchShardEvidence[] = [];
  try {
    const journey = await runSearchJourney(fixture.reference, fixture.claim, { caseId: "automatic-higher-arrival", retainShard(evidence) {
      fixture.events.push(`retain:${evidence.shard}`); retained.push(evidence);
      expect(evidence.cleanup.closed).toBe(true);
      expect(evidence.results.every(result => result.status === "failed" && result.issues.some(issue => issue.startsWith("uncovered-assertion:")))).toBe(true);
      return { artifactId: "search-shard-0", shard: evidence.shard, scheduleIds: evidence.scheduleIds };
    } });
    expect(fixture.events).toEqual(["launch:0", "close:0", "retain:0"]); expect(retained).toHaveLength(1);
    const evidence = retained[0]!; const reference = { artifactId: "search-shard-0", shard: 0, scheduleIds: evidence.scheduleIds };
    expect(evidence.version).toBe(1); expect(evidence.caseSetHash).toBe(journey.caseSetHash);
    expect(evidence.effects.map(effect => effect.id)).toEqual(["search-shard-failure", "search-runtime-output"]);
    expect(evidence.effects[1]).toMatchObject({ observedReceivedOutputBytes: 123, cleanupClosed: true });
    expect(journey.shardReferences).toEqual([reference]);
    expect(journey.effects).toEqual([{ id: "search-shard-evidence", evidenceReference: reference, cleanupClosed: true }]);
    for (const result of evidence.results) {
      const { evidence: _evidence, ...summary } = result;
      expect(journey.coverage.results.find(current => current.id === result.id)).toEqual({ ...summary,
        evidenceReference: { artifactId: reference.artifactId, shard: 0, scheduleId: result.id } });
    }
    expect(journey.cleanup.closed).toBe(true); expect(journey.pass).toBe(false);
  } finally { fixture.launch.mockRestore(); }
});
test("journey admission requires exactly the supported current-cache sources", async () => {
  for (const cacheSources of [undefined, [], ["tabs", "files", "directory"], [...OWNED_SEARCH_CACHE_SOURCES.slice(1), "files"],
    [...OWNED_SEARCH_CACHE_SOURCES, "brain-semantic"], [...OWNED_SEARCH_CACHE_SOURCES].reverse()]) {
    const fixture = shardRetentionFixture(true, { frameCursor: { version: 1, operation: "getState", captureFrame: true },
      frameAcknowledgement: { version: 1, operation: "acknowledgeFrames", retainsCursorFrame: true, readCursorsArePassive: true, draws: false },
      searchProviderWait: { version: 1, conditionType: "searchProvider", sources: SEARCH_PROVIDERS,
        statuses: ["admitted", "blocked", "settled", "cached"], sourceChange: "explicitFixtureControl", acceptCached: true, cacheAfterRunId: 0, cacheSources },
      fileSearchStreamWait: { version: 1, conditionType: "fileSearchStream", identityFields: ["generation", "query"],
        terminalPhases: ["completed", "failed", "cancelled", "unavailable"] },
      fileSearchPreviewWait: { version: 1, conditionType: "fileSearchPreview", identityFields: ["generation", "query", "workSequence"], phase: "held" } });
    try {
      const journey = await runSearchJourney(fixture.reference, fixture.claim, { caseId: "automatic-higher-arrival" });
      const complete = cacheSources?.length === OWNED_SEARCH_CACHE_SOURCES.length &&
        OWNED_SEARCH_CACHE_SOURCES.every(source => cacheSources.includes(source));
      expect(journey.effects[0]).toMatchObject({ id: "search-shard-failure", code: complete ?
        "missing-capability:complete-search-fixture-catalog" : "missing-capability:current-source-admission-wait" });
      expect(fixture.events).toEqual(["launch:0", "close:0"]); expect(journey.pass).toBe(false);
    } finally { fixture.launch.mockRestore(); }
  }
});
test("journey admission refuses missing or unsafe frame acknowledgement semantics", async () => {
  const supported = { version: 1, operation: "acknowledgeFrames", retainsCursorFrame: true, readCursorsArePassive: true, draws: false };
  for (const frameAcknowledgement of [undefined, { ...supported, version: 2 }, { ...supported, retainsCursorFrame: false },
      { ...supported, readCursorsArePassive: false }, { ...supported, draws: true }]) {
    const fixture = shardRetentionFixture(true, { frameCursor: { version: 1, operation: "getState", captureFrame: true }, frameAcknowledgement });
    try {
      const journey = await runSearchJourney(fixture.reference, fixture.claim, { caseId: "automatic-higher-arrival" });
      expect(journey.effects[0]).toMatchObject({ id: "search-shard-failure", code: "missing-capability:frame-acknowledgement" });
      expect(fixture.events).toEqual(["launch:0", "close:0"]);
    } finally { fixture.launch.mockRestore(); }
  }
});
test("inline diagnostic mode retains original shard effects without requiring an artifact callback", async () => {
  const fixture = shardRetentionFixture();
  try {
    const journey = await runSearchJourney(fixture.reference, fixture.claim, { caseId: "automatic-higher-arrival" });
    expect(journey.shardReferences).toEqual([]);
    expect(journey.effects.map(effect => effect.id)).toEqual(["search-shard-failure", "search-runtime-output"]);
    expect(journey.coverage.results.every(result => result.evidenceReference === undefined)).toBe(true);
    expect(fixture.events).toEqual(["launch:0", "close:0"]);
  } finally { fixture.launch.mockRestore(); }
});
test("a retention failure preserves completed references and cleanup and prevents subsequent launches", async () => {
  const fixture = shardRetentionFixture(); const retained: SearchShardEvidence[] = [];
  try {
    const journey = await runSearchJourney(fixture.reference, fixture.claim, { retainShard(evidence) {
      fixture.events.push(`retain:${evidence.shard}`); retained.push(evidence);
      if (evidence.shard === 1) throw new Error("fixture-write-failed");
      return { artifactId: "search-shard-0", shard: evidence.shard, scheduleIds: evidence.scheduleIds };
    } });
    expect(fixture.events).toEqual(["launch:0", "close:0", "retain:0", "launch:1", "close:1", "retain:1"]);
    expect(journey.shardReferences).toHaveLength(1); expect(journey.error).toBe("search-shard-retention-failed");
    expect(journey.cleanup.closed).toBe(true); expect(journey.pass).toBe(false);
    expect(journey.effects[1]).toMatchObject({ id: "search-shard-retention-failure", shard: 1, scheduleIds: retained[1]!.scheduleIds });
    const failed = journey.coverage.results.filter(result => result.issues.includes("search-shard-retention-failed"));
    expect(failed.map(result => result.id)).toEqual(retained[1]!.scheduleIds);
    expect(failed.every(result => result.status === "failed" && !Object.hasOwn(result, "evidence") && !Object.hasOwn(result, "evidenceReference"))).toBe(true);
    expect(journey.coverage.blocked).toBeGreaterThan(0);
  } finally { fixture.launch.mockRestore(); }
});
test("a callback cannot bind another shard, another schedule set or an empty artifact identity", async () => {
  for (const corrupt of [
    (reference: SearchShardEvidenceReference) => ({ ...reference, shard: reference.shard + 1 }),
    (reference: SearchShardEvidenceReference) => ({ ...reference, scheduleIds: [] }),
    (reference: SearchShardEvidenceReference) => ({ ...reference, artifactId: "" }),
    (reference: SearchShardEvidenceReference) => ({ ...reference, unexpected: true }),
  ]) {
    const fixture = shardRetentionFixture();
    try {
      const journey = await runSearchJourney(fixture.reference, fixture.claim, { retainShard: evidence =>
        corrupt({ artifactId: "search-shard-0", shard: evidence.shard, scheduleIds: evidence.scheduleIds }) });
      expect(fixture.events).toEqual(["launch:0", "close:0"]);
      expect(journey.error).toBe("search-shard-retention-failed"); expect(journey.shardReferences).toEqual([]);
      expect(journey.effects[0]).toMatchObject({ id: "search-shard-retention-failure", code: "invalid-search-shard-reference" });
      expect(journey.cleanup.closed).toBe(true); expect(journey.pass).toBe(false);
    } finally { fixture.launch.mockRestore(); }
  }
});
test("unclean shard closure is retained before stopping and never launches a replacement runtime", async () => {
  const fixture = shardRetentionFixture(false); const retained: SearchShardEvidence[] = [];
  try {
    const journey = await runSearchJourney(fixture.reference, fixture.claim, { retainShard(evidence) {
      fixture.events.push(`retain:${evidence.shard}`); retained.push(evidence);
      return { artifactId: "failed-shard-0", shard: evidence.shard, scheduleIds: evidence.scheduleIds };
    } });
    expect(fixture.events).toEqual(["launch:0", "close:0", "retain:0"]); expect(retained).toHaveLength(1);
    expect(retained[0]!.cleanup.closed).toBe(false); expect(journey.cleanup.closed).toBe(false);
    expect(journey.cleanup.failureCodes).toContain("fixture-drain-failed"); expect(journey.error).toBe("INVALID_CLEANUP");
    expect(journey.shardReferences).toHaveLength(1); expect(journey.pass).toBe(false);
  } finally { fixture.launch.mockRestore(); }
});
