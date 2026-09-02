import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claimOutput, emptyOwnedCleanup, validateOutputTarget, type OutputClaim } from "./artifact-lifecycle.ts";
import { accountSearchCoverage, searchContractSpec, searchScheduleComparisonGroup, SEARCH_FIXTURE_ID } from "./launcher-search-contract.ts";
import { reconstructSearchFrame, type SearchShardEvidence } from "./launcher-search-recipes.ts";
import { retainSearchShardEvidence, resolveSearchJourneyReceipt, searchShardArtifactSpecs } from "./launcher-search-receipt.ts";
import { annotateOwnedEvidence, commitOwnedReport } from "../devtools/design.ts";
import { aggregateCleanup } from "../devtools/lib/story-contract.ts";
import { prepareValidatedReceipt } from "../devtools/lib/receipt-schema.ts";
import { MAX_COMPACT_RECEIPT_BYTES, MAX_RECEIPT_DETAIL_BYTES, readReceiptDocument, resolveReceiptDetails } from "../devtools/lib/receipt-artifact.ts";
import type { Json } from "../devtools/driver.ts";

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
function claim(): OutputClaim {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "search-shard-receipt-"))); roots.push(root);
  return claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output", "proof"), kind: "directory", probeId: "search-shard-contract" }));
}
function rawShard(index: number, closed = true, passed = false): SearchShardEvidence {
  const spec = searchContractSpec();
  const schedule = (passed ? spec.schedules.filter(schedule => !schedule.structuralNotApplicable && !schedule.terminalIntents && !searchScheduleComparisonGroup(schedule)) : spec.schedules)[index]!;
  const target = { windowId: "main", windowGeneration: 1, frameGeneration: index + 1 };
  return { version: 1, caseSetHash: spec.caseSetHash, shard: index, scheduleIds: [schedule.id],
    results: [{ id: schedule.id, caseId: schedule.caseId, status: passed ? "passed" : "failed", executed: true,
      issues: passed ? [] : ["fixture-diagnostic"], assertions: schedule.assertions.map(id => ({ id, pass: passed })), notApplicableAssertions: schedule.notApplicableAssertions,
      evidence: { phases: [{ id: "fixture-frame", frame: { frameRef: 0 } }], actions: [], orderComparisons: [], framePool: { version: 1,
        frames: [{ facts: { frame: { processInstanceId: "fixture-process", sessionGeneration: "fixture-session", target,
          requestedTarget: { type: "instance", id: "main", generation: 1 } }, mode: "scheduled", invalidationEpoch: 1, notificationEpoch: 1,
          localInputFocused: false, nativeWindowActive: false }, paintBindingRefs: [0] }],
        paintBindings: [{ binding: { kind: "mainSearchRow" }, metadataRef: 0 }], metadata: [{ numeric: [0, 1, 2], privateValue: "private-search-fixture" }] } } }],
    effects: [{ id: "search-runtime-output", shard: index, observedReceivedOutputBytes: 1024, cleanupClosed: closed }],
    cleanup: { ...emptyOwnedCleanup(), closed, ...(closed ? {} : { streamsDrained: false, failureCodes: ["fixture-streams-open"] }) } };
}

function candidate(shard: SearchShardEvidence): Json {
  return { schemaVersion: 2, tool: "script-kit-devtools.design", command: "design.run", classification: shard.cleanup.closed ? "reproduced" : "invalid-cleanup",
    disposition: shard.cleanup.closed ? undefined : "INVALID_CLEANUP", evidenceClass: "DIRECT_RUNTIME_PROOF", provesRuntimeBehavior: false,
    artifactReference: { manifestPath: "target-agent/artifacts/fixture/manifest.json", manifestSha256: "a".repeat(64) },
    observation: annotateOwnedEvidence(shard), assertions: shard.results.map(result => ({ id: result.id, pass: result.status === "passed" })), cleanup: shard.cleanup, errors: [], warnings: [] };
}
function prepared(shard: SearchShardEvidence): Json {
  const value = prepareValidatedReceipt("devtools.design.run", candidate(shard));
  expect(value.receipt.producerValidation.valid).toBe(true);
  return value.receipt;
}
function fixture(shards = [rawShard(0), rawShard(1)], mutate?: (receipt: Json, owner: OutputClaim) => void) {
  const owner = claim();
  const documents = shards.map(prepared);
  const references = documents.map(document => retainSearchShardEvidence(owner, document));
  const summaryResults = shards.flatMap((shard, index) => shard.results.map(({ evidence: _evidence, ...result }) => ({ ...result,
    evidenceReference: { artifactId: references[index]!.artifactId, shard: shard.shard, scheduleId: result.id } })));
  const cleanup = aggregateCleanup(shards.map(shard => shard.cleanup));
  const coverage = accountSearchCoverage(searchContractSpec().schedules, summaryResults);
  const journey = { id: "launcher-ranking-provider", proofLevel: "owned-production-runtime", pass: false,
    assertions: coverage.results.filter(result => result.status !== "notApplicable").map(result => ({ id: result.id, pass: result.status === "passed" })), frames: [],
    effects: references.map((reference, index) => ({ id: "search-shard-evidence", evidenceReference: reference, cleanupClosed: shards[index]!.cleanup.closed })),
    fixtureIds: [SEARCH_FIXTURE_ID], cleanup, coverage, caseSetHash: searchContractSpec().caseSetHash, shardReferences: references };
  const final = prepareValidatedReceipt("devtools.design.run", { ...candidate({ ...shards[0]!, cleanup }), observation: annotateOwnedEvidence({ journeys: [journey] }) }).receipt;
  expect(final.producerValidation.valid).toBe(true);
  mutate?.(final, owner);
  return { owner, documents, wire: commitOwnedReport(owner, final, cleanup), journey };
}

test("multiple shards are retained once and reconstructed from the final compact receipt", () => {
  const { owner, documents, wire } = fixture();
  expect(searchShardArtifactSpecs(owner).map(spec => spec.id)).toEqual(["search-shard-0", "search-shard-1"]);
  expect(readdirSync(owner.artifactsRoot).sort()).toEqual(["observation.json", "search-shard-0.json", "search-shard-1.json"]);
  expect(wire.observation).toBeUndefined();
  const summary = resolveReceiptDetails(wire).observation.journeys[0];
  expect(summary.coverage.results.filter((result: Json) => result.evidenceReference).every((result: Json) => !Object.hasOwn(result, "evidence"))).toBe(true);
  const resolved = resolveSearchJourneyReceipt(readReceiptDocument(owner.receiptPath), owner.receiptPath);
  for (const document of documents) {
    const source = document.observation.results[0];
    const result = resolved.coverage.results.find(result => result.id === source.id)!;
    expect(result).toEqual(source);
    expect(reconstructSearchFrame((result.evidence as Json).framePool, { frameRef: 0 })).toEqual(reconstructSearchFrame(source.evidence.framePool, { frameRef: 0 }));
  }
  expect(resolved.effects).toEqual(documents.flatMap(document => document.observation.effects));
  expect(readFileSync(join(owner.artifactsRoot, "search-shard-0.json"), "utf8")).not.toContain("private-search-fixture");
});

test("every admitted shard and the final observation fit the unchanged compact receipt limit", () => {
  const shardCount = searchContractSpec().admission.shards.length;
  const { owner, wire } = fixture(Array.from({ length: shardCount }, (_, index) => rawShard(index)));
  expect(wire.artifactLifecycle.artifacts).toHaveLength(shardCount + 1);
  expect(Buffer.byteLength(JSON.stringify(wire, null, 2)) + 1).toBeLessThanOrEqual(MAX_COMPACT_RECEIPT_BYTES);
  expect(searchShardArtifactSpecs(owner)).toHaveLength(shardCount);
  const resolved = resolveSearchJourneyReceipt(wire);
  expect(resolved.coverage.results).toHaveLength(searchContractSpec().schedules.length);
  expect(resolved.coverage.results.filter(result => result.evidence)).toHaveLength(shardCount);
}, 60_000);

test("passed frame pools stay numeric and dangling references refuse persistence", () => {
  const owner = claim();
  retainSearchShardEvidence(owner, prepared(rawShard(0, true, true)));
  const loaded = readReceiptDocument(join(owner.artifactsRoot, "search-shard-0.json")).receipt.observation.results[0].evidence;
  expect(loaded.phases[0].frame).toEqual({ frameRef: 0 });
  expect(loaded.framePool.frames[0].paintBindingRefs).toEqual([0]);
  const invalid = prepared(rawShard(1, true, true));
  invalid.observation.results[0].evidence.phases[0].frame.frameRef = 99;
  expect(() => retainSearchShardEvidence(owner, invalid)).toThrow("dangling-frame-evidence-reference");
  expect(searchShardArtifactSpecs(owner)).toHaveLength(1);
});

test("failed cleanup and incomplete evidence survive as diagnostics", () => {
  const raw = rawShard(0, false);
  (raw.results[0]!.evidence as Json).failure = { id: "fixture-partial-frame", unretainedFramePage: { acknowledged: false } };
  const { documents, wire } = fixture([raw]);
  const resolved = resolveSearchJourneyReceipt(wire);
  expect(resolved.pass).toBe(false);
  expect(resolved.cleanup.closed).toBe(false);
  expect(resolved.coverage.results.find(result => result.id === raw.results[0]!.id)).toEqual(documents[0]!.observation.results[0]);
  expect(resolved.effects[0]!.cleanupClosed).toBe(false);
});

test.each(["tamper", "missing", "alias"])("%s shard artifacts fail closed", mode => {
  const { owner, wire } = fixture();
  const path = join(owner.artifactsRoot, "search-shard-1.json");
  if (mode === "tamper") writeFileSync(path, readFileSync(path, "utf8").replace("1024", "1025"));
  else { renameSync(path, `${path}.saved`); if (mode === "alias") symlinkSync(`${path}.saved`, path); }
  expect(() => resolveSearchJourneyReceipt(wire)).toThrow();
});

test.each(["schedule-list", "result-summary", "result-ref", "duplicate-ref", "missing-ref", "effect-ref", "ambiguous", "pass", "assertions", "provenance"])("%s mismatch fails even with valid final hashes", mode => {
  const { wire } = fixture(undefined, final => {
    const journey = final.observation.journeys[0];
    const result = journey.coverage.results.find((result: Json) => result.evidenceReference);
    if (mode === "schedule-list") journey.shardReferences[0].scheduleIds = [journey.shardReferences[1].scheduleIds[0]];
    if (mode === "result-summary") result.issues.push("invented-issue");
    if (mode === "result-ref") result.evidenceReference.shard = 1;
    if (mode === "duplicate-ref") journey.shardReferences.push(journey.shardReferences[0]);
    if (mode === "missing-ref") journey.shardReferences.pop();
    if (mode === "effect-ref") journey.effects[0].cleanupClosed = false;
    if (mode === "ambiguous") final.journeys = final.observation.journeys;
    if (mode === "pass") journey.pass = true;
    if (mode === "assertions") journey.assertions[0].pass = true;
    if (mode === "provenance") final.artifactReference.manifestSha256 = "b".repeat(64);
  });
  expect(() => resolveSearchJourneyReceipt(wire)).toThrow();
});

test.each(["owner", "nested", "version"])("%s shard mismatch fails with its replacement hash committed", mode => {
  const { wire } = fixture(undefined, (_final, owner) => {
    const path = join(owner.artifactsRoot, "search-shard-0.json");
    const document = readReceiptDocument(path);
    if (mode === "owner") document.ownerSha256 = "b".repeat(64);
    if (mode === "nested") document.receipt.receiptFormat = "script-kit-owned-receipt";
    if (mode === "version") document.receipt.observation.version = 2;
    writeFileSync(path, `${JSON.stringify(document, null, 2)}\n`);
  });
  expect(() => resolveSearchJourneyReceipt(wire)).toThrow();
});

test("stories resolve their documented top-level journey shape", () => {
  const { wire } = fixture(undefined, final => {
    final.primitiveId = "devtools.stories.run"; final.tool = "script-kit-devtools.stories"; final.command = "stories.run";
    final.artifactReferences = [final.artifactReference]; delete final.artifactReference;
    final.journeys = final.observation.journeys; delete final.observation;
  });
  expect(resolveSearchJourneyReceipt(wire).coverage.results.filter(result => result.evidence)).toHaveLength(2);
});

test("duplicate IDs and oversized documents preserve prior shards without writing refused artifacts", () => {
  const owner = claim();
  const first = prepared(rawShard(0)); retainSearchShardEvidence(owner, first);
  const original = readFileSync(join(owner.artifactsRoot, "search-shard-0.json"));
  expect(() => retainSearchShardEvidence(owner, first)).toThrow("duplicate_shard");
  const oversized = prepared(rawShard(1)); oversized.padding = "x".repeat(MAX_RECEIPT_DETAIL_BYTES);
  expect(() => retainSearchShardEvidence(owner, oversized)).toThrow("detail_size_limit");
  expect(searchShardArtifactSpecs(owner)).toHaveLength(1);
  expect(readdirSync(owner.artifactsRoot)).toEqual(["search-shard-0.json"]);
  expect(readFileSync(join(owner.artifactsRoot, "search-shard-0.json"))).toEqual(original);
});

function observationAliasShard(): SearchShardEvidence {
  const shard = rawShard(0, true, true);
  const evidence = shard.results[0]!.evidence as Json;
  const query = { lifetime: 1, revision: 1, scopeRevision: 1 };
  evidence.phases = [{ id: "first-observation", reusedObservation: false, targetIdentity: { windowId: "main", windowGeneration: 1 },
    query, computedQuery: query, providerRunCount: 1, providerRuns: [{ id: 1 }], providerRunsAreChanges: true, completedFrames: [{ frameRef: 0 }],
    pending: false, resultRevision: 1, selectionRevision: 1, viewportRevision: 1, selectedSemanticId: null, selectedOrdinal: null,
    selectionIntent: { kind: "automaticTop" }, selectionArmed: false, viewportIntent: "followSelection", reconciliationReason: null,
    publication: null, rankingFingerprint: "r".repeat(64), selectionFingerprint: "s".repeat(64), providerRunsHash: "p".repeat(64), traceGeneration: 1 },
    { id: "repeated-observation", observationRef: 0, reusedObservation: true, providerRuns: [], providerRunsAreChanges: true, completedFrames: [] }];
  return shard;
}

test("canonical reader expands backward observation aliases without replaying deltas or changing stored evidence", () => {
  const { wire, owner, documents } = fixture([observationAliasShard()]);
  const source = documents[0]!.observation.results[0].evidence.phases;
  const path = join(owner.artifactsRoot, "search-shard-0.json");
  const before = readFileSync(path);
  const resolved = resolveSearchJourneyReceipt(wire).coverage.results.find(result => result.evidence)!;
  const phases = (resolved.evidence as Json).phases;
  expect(phases[0]).toEqual(source[0]);
  const { observationRef: _reference, ...alias } = source[1];
  expect(phases[1]).toEqual({ ...source[0], ...alias });
  expect(phases[1].providerRuns).toEqual([]);
  expect(phases[1].completedFrames).toEqual([]);
  expect(phases[1].rankingFingerprint.redacted).toBe(true);
  expect(readFileSync(path)).toEqual(before);
  expect(readReceiptDocument(path).receipt.observation.results[0].evidence.phases[1].observationRef).toBe(0);
});

test.each(["forward", "chain", "cross-kind", "mixed"])("%s observation aliases are rejected after artifact hash verification", mode => {
  const { wire } = fixture([observationAliasShard()], (_final, owner) => {
    const path = join(owner.artifactsRoot, "search-shard-0.json");
    const document = readReceiptDocument(path);
    const phases = document.receipt.observation.results[0].evidence.phases;
    if (mode === "forward") phases[1].observationRef = 1;
    if (mode === "chain") phases.push({ ...phases[1], id: "chained", observationRef: 1 });
    if (mode === "cross-kind") phases[0] = { id: "capture", frame: { frameRef: 0 } };
    if (mode === "mixed") phases[1].query = phases[0].query;
    writeFileSync(path, `${JSON.stringify(document, null, 2)}\n`);
  });
  expect(() => resolveSearchJourneyReceipt(wire)).toThrow();
});

function pixelReferenceShard(): SearchShardEvidence {
  const shard = rawShard(0, true, true);
  const evidence = shard.results[0]!.evidence as Json;
  evidence.framePool.frames[0].facts.pixelEvidence = [{ kind: "selectionMarker", rgba: [1, 2, 3, 255], label: "private-sample" }];
  evidence.phases = [{ id: "capture", frameEvidence: { frameRef: 0 },
    capture: { mimeType: "image/png", hiDpi: true, width: 1, height: 1, byteLength: 1, sha256: "a".repeat(64) },
    pixels: { nativeSamplesFrame: { frameRef: 0 }, sampled: [{ r: 1, g: 2, b: 3, a: 255 }] } }];
  return shard;
}

test("canonical reader expands shared native pixels without changing pool or private descriptors", () => {
  const { wire, owner, documents } = fixture([pixelReferenceShard()]);
  const source = documents[0]!.observation.results[0].evidence;
  const path = join(owner.artifactsRoot, "search-shard-0.json");
  const before = readFileSync(path);
  const evidence = resolveSearchJourneyReceipt(wire).coverage.results.find(result => result.evidence)!.evidence as Json;
  expect(evidence.framePool).toEqual(source.framePool);
  expect(evidence.phases[0].pixels).toEqual({ frameGeneration: 1, nativeSamples: source.framePool.frames[0].facts.pixelEvidence,
    captureHash: source.phases[0].capture.sha256, sampled: source.phases[0].pixels.sampled });
  expect(evidence.phases[0].pixels.nativeSamples[0].label.redacted).toBe(true);
  expect(readFileSync(path)).toEqual(before);
});

test.each(["dangling", "cross-frame", "mixed", "missing-origin"])("%s pixel references fail after artifact verification", mode => {
  const { wire } = fixture([pixelReferenceShard()], (_final, owner) => {
    const path = join(owner.artifactsRoot, "search-shard-0.json");
    const document = readReceiptDocument(path);
    const evidence = document.receipt.observation.results[0].evidence;
    const phase = evidence.phases[0];
    if (mode === "dangling") phase.pixels.nativeSamplesFrame.frameRef = 99;
    if (mode === "mixed") phase.pixels.nativeSamples = [];
    if (mode === "missing-origin") delete evidence.framePool.frames[0].facts.pixelEvidence;
    if (mode === "cross-frame") {
      const second = structuredClone(evidence.framePool.frames[0]); second.facts.frame.target.frameGeneration++;
      evidence.framePool.frames.push(second); phase.frameEvidence.frameRef = 1;
    }
    writeFileSync(path, `${JSON.stringify(document, null, 2)}\n`);
  });
  expect(() => resolveSearchJourneyReceipt(wire)).toThrow();
});

function internedFrameShard(): SearchShardEvidence {
  const shard = pixelReferenceShard();
  const pool = (shard.results[0]!.evidence as Json).framePool;
  const entry = pool.frames[0];
  entry.ownerRef = pool.metadata.length;
  pool.metadata.push({ binarySha256: "b".repeat(64), manifestSha256: "a".repeat(64), pid: 123,
    processInstanceId: entry.facts.frame.processInstanceId, processStartTime: "2026-09-01T00:00:00.000Z",
    sessionGeneration: entry.facts.frame.sessionGeneration, requestedTarget: entry.facts.frame.requestedTarget, nativeWindowId: "private-window-identity" });
  entry.factRefs = { search: pool.metadata.length, pixelEvidence: pool.metadata.length + 1, nativeWindow: pool.metadata.length + 2 };
  pool.metadata.push({ query: { lifetime: 1, revision: 1, scopeRevision: 1 }, selectedSemanticId: null },
    entry.facts.pixelEvidence.map((sample: Json) => ({ ...sample, probe: { x: 0, y: 0, r: 1, g: 2, b: 3, a: 255 } })), { visible: false });
  delete entry.facts.pixelEvidence;
  return shard;
}

test("interned frame facts and owner metadata materialize losslessly with private descriptors unchanged", () => {
  const { wire, owner, documents } = fixture([internedFrameShard()]);
  const storedPool = documents[0]!.observation.results[0].evidence.framePool;
  const stored = storedPool.frames[0];
  const before = readFileSync(join(owner.artifactsRoot, "search-shard-0.json"));
  const evidence = resolveSearchJourneyReceipt(wire).coverage.results.find(result => result.evidence)!.evidence as Json;
  const decoded = evidence.framePool.frames[0];
  expect(decoded.factRefs).toBeUndefined();
  expect(decoded.ownerRef).toBeUndefined();
  expect(decoded.facts.frame).toEqual({ ...storedPool.metadata[stored.ownerRef], ...stored.facts.frame });
  expect(decoded.facts.search).toEqual(storedPool.metadata[stored.factRefs.search]);
  expect(decoded.facts.pixelEvidence).toEqual(storedPool.metadata[stored.factRefs.pixelEvidence]);
  expect(decoded.facts.nativeWindow).toEqual(storedPool.metadata[stored.factRefs.nativeWindow]);
  expect(decoded.facts.frame.nativeWindowId.redacted).toBe(true);
  expect(evidence.framePool.metadata).toEqual(storedPool.metadata);
  expect(decoded.paintBindingRefs).toEqual(stored.paintBindingRefs);
  expect(evidence.phases[0].pixels.nativeSamples).toEqual(decoded.facts.pixelEvidence);
  expect(readFileSync(join(owner.artifactsRoot, "search-shard-0.json"))).toEqual(before);
});

test.each(["dangling", "cross-kind", "mixed", "extra-kind", "owner-id", "owner-window", "owner-shadow"])("%s interned frame references fail after hash verification", mode => {
  const { wire } = fixture([internedFrameShard()], (_final, owner) => {
    const path = join(owner.artifactsRoot, "search-shard-0.json");
    const document = readReceiptDocument(path);
    const pool = document.receipt.observation.results[0].evidence.framePool;
    const entry = pool.frames[0];
    if (mode === "dangling") entry.factRefs.search = pool.metadata.length;
    if (mode === "cross-kind") entry.factRefs.search = entry.factRefs.nativeWindow;
    if (mode === "mixed") entry.facts.search = pool.metadata[entry.factRefs.search];
    if (mode === "extra-kind") entry.factRefs.paintBindings = 0;
    if (mode === "owner-id") pool.metadata[entry.ownerRef].processInstanceId = "other-process";
    if (mode === "owner-window") pool.metadata[entry.ownerRef].requestedTarget = { type: "instance", id: "other-window", generation: 1 };
    if (mode === "owner-shadow") pool.metadata[entry.ownerRef].target = entry.facts.frame.target;
    writeFileSync(path, `${JSON.stringify(document, null, 2)}\n`);
  });
  expect(() => resolveSearchJourneyReceipt(wire)).toThrow();
});
