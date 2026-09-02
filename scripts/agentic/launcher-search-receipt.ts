import { lstatSync } from "node:fs";
import { dirname, join } from "node:path";
import { isDeepStrictEqual } from "node:util";
import { validateArtifact, writeJsonArtifactAtomic, type ArtifactSpec, type OutputClaim } from "./artifact-lifecycle.ts";
import { accountSearchCoverage, searchContractSpec, type SearchScheduleResult } from "./launcher-search-contract.ts";
import { compareSearchOrders, reconstructSearchCapturePixels, reconstructSearchFrameFacts, reconstructSearchObservationPhase, validateSearchObservationPhases, validateSearchFramePool, type SearchFrameReference, type SearchJourneyReceipt, type SearchShardEvidence, type SearchShardEvidenceReference } from "./launcher-search-recipes.ts";
import type { Json } from "../devtools/driver.ts";
import { aggregateCleanup } from "../devtools/lib/story-contract.ts";
import { isReferenceReceipt, ownedObservationDocument, readReceiptDocument, resolveReceiptDetails } from "../devtools/lib/receipt-artifact.ts";

const retainedSpecs = new WeakMap<OutputClaim, readonly ArtifactSpec[]>();
function fail(reason: string): never { throw new Error(`invalid_search_shard_receipt:${reason}`); }
const object = (value: unknown): value is Json => value !== null && typeof value === "object" && !Array.isArray(value);
const same = (actual: unknown, expected: unknown, reason: string): void => { if (!isDeepStrictEqual(actual, expected)) fail(reason); };
function shardSpec(shard: number): ArtifactSpec {
  return { id: `search-shard-${shard}`, sourceName: `search-shard-${shard}.json`, required: true, mediaType: "application/json", kind: "json" };
}

function frameReferences(evidence: Json): SearchFrameReference[] {
  const references: SearchFrameReference[] = [];
  const pending: unknown[] = [evidence];
  while (pending.length) {
    const value = pending.pop();
    if (Array.isArray(value)) { for (const child of value) pending.push(child); }
    else if (object(value)) {
      if (Object.hasOwn(value, "frameRef")) references.push(value as SearchFrameReference);
      else for (const [key, child] of Object.entries(value)) if (key !== "framePool") pending.push(child);
    }
  }
  return references;
}

function comparableFingerprint(value: unknown): string | undefined {
  if (!object(value) || Object.keys(value).length !== 7 || value.redacted !== true || value.contentKind !== "UserContent" ||
      value.length !== 64 || value.byteLength !== 64 || value.fingerprintAlgorithm !== "hmac-sha256" || value.rawContentReturned !== false ||
      typeof value.fingerprint !== "string" || !/^[a-f0-9]{64}$/.test(value.fingerprint)) return undefined;
  return value.fingerprint;
}

function hasPixelReference(phase: Json): boolean {
  return object(phase.pixels) && Object.hasOwn(phase.pixels, "nativeSamplesFrame");
}

function hasInternedFrameFacts(pool: unknown): boolean {
  return object(pool) && Array.isArray(pool.frames) && pool.frames.some(entry =>
    object(entry) && (Object.hasOwn(entry, "factRefs") || Object.hasOwn(entry, "ownerRef")));
}

function validateShard(value: unknown): SearchShardEvidence {
  if (!object(value) || value.version !== 1 || !Number.isSafeInteger(value.shard) || value.shard < 0 ||
      Object.keys(value).some(key => !["version", "caseSetHash", "shard", "scheduleIds", "results", "effects", "cleanup"].includes(key))) fail("shard_schema");
  const spec = searchContractSpec();
  if (value.caseSetHash !== spec.caseSetHash || !Array.isArray(value.scheduleIds) || !value.scheduleIds.length ||
      value.scheduleIds.some(id => typeof id !== "string") || new Set(value.scheduleIds).size !== value.scheduleIds.length ||
      !Array.isArray(value.results) || !Array.isArray(value.effects) || value.effects.some(effect => !object(effect)) ||
      !object(value.cleanup) || typeof value.cleanup.closed !== "boolean") fail("shard_identity");
  same(value.results.map((result: unknown) => object(result) ? result.id : null), value.scheduleIds, "schedule_list");
  for (const result of value.results) {
    const schedule = spec.schedules.find(schedule => schedule.id === result.id);
    if (!schedule || result.caseId !== schedule.caseId || Object.hasOwn(result, "evidenceReference") ||
        !["passed", "failed", "blocked", "notApplicable"].includes(result.status) || typeof result.executed !== "boolean" ||
        !Array.isArray(result.issues) || result.issues.some((issue: unknown) => typeof issue !== "string") ||
        !Array.isArray(result.assertions) || result.assertions.some((assertion: unknown) => !object(assertion) || typeof assertion.id !== "string" || typeof assertion.pass !== "boolean")) fail("result_schema");
    const evidence = result.evidence;
    if (object(evidence) && Object.hasOwn(evidence, "phases")) validateSearchObservationPhases(evidence.phases);
    const pixelPhases: Json[] = object(evidence) && Array.isArray(evidence.phases) ? evidence.phases.filter(hasPixelReference) : [];
    // Failed runs retain incomplete observations as diagnostics; explicit references still must resolve.
    if (result.status === "passed" || result.status === "notApplicable" || pixelPhases.length || hasInternedFrameFacts(evidence?.framePool)) {
      if (!object(evidence)) fail("missing_passed_evidence");
      validateSearchFramePool(evidence.framePool, frameReferences(evidence));
      for (const phase of pixelPhases) reconstructSearchCapturePixels(evidence.framePool, phase);
    }
  }
  // These validators already define schedule, assertion, terminal and order proof semantics.
  // Only copy the small fields compareSearchOrders mutates, never the frame evidence.
  const compared: SearchScheduleResult[] = value.results.map((result: SearchScheduleResult) => {
    const evidence = result.evidence as Json | undefined;
    const comparisons = Array.isArray(evidence?.orderComparisons) ? evidence.orderComparisons.map((comparison: Json) => {
      const fingerprint = comparableFingerprint(comparison?.fingerprint);
      if (!fingerprint && (result.status === "passed" || result.status === "notApplicable")) fail("comparison_fingerprint");
      return { ...comparison, fingerprint: fingerprint ?? comparison?.fingerprint };
    }) : [];
    return { ...result, issues: [...result.issues], assertions: result.assertions.map(assertion => ({ ...assertion })),
      evidence: evidence ? { ...evidence, orderComparisons: comparisons } : evidence };
  });
  compareSearchOrders(compared);
  for (let index = 0; index < compared.length; index++) {
    const result = value.results[index];
    if (result.status === "passed" || result.status === "notApplicable") {
      same(compared[index]!.issues, result.issues, "order_proof");
      same(compared[index]!.assertions, result.assertions, "order_proof");
    }
  }
  accountSearchCoverage(spec.schedules, value.results);
  return value as SearchShardEvidence;
}

function materializeResult(result: SearchScheduleResult): SearchScheduleResult {
  const evidence = result.evidence;
  if (!object(evidence)) return result;
  let framePool = evidence.framePool;
  if (hasInternedFrameFacts(framePool)) {
    const frames = framePool.frames.map((entry: Json, index: number) => {
      if (!Object.hasOwn(entry, "factRefs") && !Object.hasOwn(entry, "ownerRef")) return entry;
      const { factRefs: _facts, ownerRef: _owner, ...retained } = entry;
      return { ...retained, facts: reconstructSearchFrameFacts(framePool, index) };
    });
    framePool = { ...framePool, frames };
  }
  let phases = evidence.phases;
  if (Array.isArray(phases) && phases.some(phase => Object.hasOwn(phase, "observationRef") || hasPixelReference(phase))) {
    phases = phases.map((phase: Json, index: number) => {
      const full = Object.hasOwn(phase, "observationRef") ? reconstructSearchObservationPhase(evidence.phases, index) : phase;
      return hasPixelReference(full) ? { ...full, pixels: reconstructSearchCapturePixels(evidence.framePool, full) } : full;
    });
  }
  if (framePool === evidence.framePool && phases === evidence.phases) return result;
  return { ...result, evidence: { ...evidence,
    ...(framePool !== evidence.framePool ? { framePool } : {}), ...(phases !== evidence.phases ? { phases } : {}) } };
}

function preparedShard(receipt: Json): SearchShardEvidence {
  if (receipt.schemaVersion !== 2 || receipt.primitiveId !== "devtools.design.run" || receipt.tool !== "script-kit-devtools.design" ||
      receipt.command !== "design.run" || receipt.producerValidation?.valid !== true || receipt.privacy?.recursiveCanaryScan?.pass !== true ||
      isReferenceReceipt(receipt) || Object.hasOwn(receipt, "artifactLifecycle")) fail("prepared_receipt_schema");
  return validateShard(receipt.observation);
}

/** Persist each closed shard before another runtime starts. Earlier artifacts are never removed. */
export function retainSearchShardEvidence(claim: OutputClaim, preparedReceipt: Json): SearchShardEvidenceReference {
  const evidence = preparedShard(preparedReceipt);
  const spec = Object.freeze(shardSpec(evidence.shard));
  const previous = retainedSpecs.get(claim) ?? [];
  if (previous.some(item => item.id === spec.id)) fail("duplicate_shard");
  const document = ownedObservationDocument(claim, preparedReceipt);
  writeJsonArtifactAtomic(claim, spec.sourceName, document);
  // Register immediately after the atomic write, even if subsequent validation fails.
  retainedSpecs.set(claim, Object.freeze([...previous, spec]));
  const artifact = validateArtifact(join(claim.artifactsRoot, spec.sourceName), spec, claim.artifactsRoot);
  if (!artifact.readable || artifact.validation.failures.length) fail("persisted_artifact_validation");
  return { artifactId: spec.id, shard: evidence.shard, scheduleIds: [...evidence.scheduleIds] };
}

export function searchShardArtifactSpecs(claim: OutputClaim): readonly ArtifactSpec[] {
  return retainedSpecs.get(claim) ?? [];
}

function stableIdentity(path: string): string {
  const stat = lstatSync(path, { bigint: true });
  return [stat.dev, stat.ino, stat.nlink, stat.size, stat.mtimeNs, stat.ctimeNs].join(":");
}

/** Resolve only an explicitly requested search journey; ordinary receipt reads stay compact. */
export function resolveSearchJourneyReceipt(wire: Json, receiptPath?: string): SearchJourneyReceipt {
  try {
    // This authenticates every descriptor, path, size, hash and current output owner first.
    const detail = resolveReceiptDetails(wire, receiptPath);
    if (!isReferenceReceipt(wire)) fail("owned_receipt_required");
    const design = detail.primitiveId === "devtools.design.run";
    const stories = detail.primitiveId === "devtools.stories.run";
    if ((!design && !stories) || (design && Object.hasOwn(detail, "journeys")) ||
        (stories && object(detail.observation) && Object.hasOwn(detail.observation, "journeys"))) fail("ambiguous_journey");
    const journeys = design ? detail.observation?.journeys : detail.journeys;
    if (!Array.isArray(journeys)) fail("journey_schema");
    const matches = journeys.filter(journey => journey?.id === "launcher-ranking-provider");
    if (matches.length !== 1 || (design && journeys.length !== 1)) fail("ambiguous_journey");
    const journey = matches[0];
    if (journey.caseSetHash !== searchContractSpec().caseSetHash || !object(journey.coverage) || !Array.isArray(journey.coverage.results) ||
        !Array.isArray(journey.shardReferences) || !Array.isArray(journey.effects)) fail("journey_schema");
    const failedRetention = journey.effects.filter((effect: Json) => effect?.id === "search-shard-retention-failure");
    if (failedRetention.length > 1 || (!journey.shardReferences.length && !failedRetention.length)) fail("retention_failure_schema");
    const failure = failedRetention[0];
    const unretained = new Set<string>();
    if (failure) {
      if (journey.pass !== false || !Number.isSafeInteger(failure.shard) || failure.shard < 0 || !Array.isArray(failure.scheduleIds) || !failure.scheduleIds.length ||
          failure.scheduleIds.some((id: unknown) => typeof id !== "string") || new Set(failure.scheduleIds).size !== failure.scheduleIds.length ||
          Object.hasOwn(failure, "evidenceReference")) fail("retention_failure_schema");
      for (const id of failure.scheduleIds) unretained.add(id);
    }
    const shards: SearchShardEvidence[] = [];
    const fullResults = new Map<string, { result: SearchScheduleResult; reference: Json }>();
    const usedArtifacts = new Set<string>();
    for (const reference of journey.shardReferences) {
      if (!object(reference) || Object.keys(reference).length !== 3 || !Number.isSafeInteger(reference.shard) || reference.shard < 0 ||
          reference.artifactId !== shardSpec(reference.shard).id || usedArtifacts.has(reference.artifactId)) fail("shard_reference");
      usedArtifacts.add(reference.artifactId);
      const spec = shardSpec(reference.shard);
      const artifact = wire.artifactLifecycle.artifacts.find((artifact: Json) => artifact.id === reference.artifactId);
      if (!artifact || !artifact.required || artifact.relativePath !== spec.sourceName || artifact.identity.sourceName !== spec.sourceName ||
          artifact.identity.destinationName !== spec.sourceName || artifact.identity.kind !== spec.kind || artifact.identity.mediaType !== spec.mediaType) fail("shard_artifact_identity");
      const identity = stableIdentity(artifact.path);
      const document = readReceiptDocument(artifact.path);
      same(validateArtifact(artifact.path, spec, dirname(artifact.path)), artifact, "shard_artifact_changed");
      same(stableIdentity(artifact.path), identity, "shard_artifact_changed");
      if (document.schemaVersion !== 1 || document.kind !== "owned-receipt-observation" || document.ownerSha256 !== wire.detailReference.ownerSha256 ||
          !object(document.receipt) || Object.keys(document).some(key => !["schemaVersion", "kind", "ownerSha256", "receipt"].includes(key))) fail("shard_owner");
      const shard = preparedShard(document.receipt);
      if (!object(document.receipt.artifactReference)) fail("artifact_provenance");
      if (design) same(document.receipt.artifactReference, detail.artifactReference, "artifact_provenance");
      else if (!Array.isArray(detail.artifactReferences) || !detail.artifactReferences.some((reference: Json) => isDeepStrictEqual(reference, document.receipt.artifactReference))) fail("artifact_provenance");
      same(shard.shard, reference.shard, "shard_number");
      same(shard.caseSetHash, journey.caseSetHash, "case_set_hash");
      same(shard.scheduleIds, reference.scheduleIds, "reference_schedule_list");
      shards.push(shard);
      for (const result of shard.results) {
        if (fullResults.has(result.id)) fail("duplicate_schedule");
        fullResults.set(result.id, { result, reference: { artifactId: reference.artifactId, shard: shard.shard, scheduleId: result.id } });
      }
    }
    if (failure && (usedArtifacts.has(shardSpec(failure.shard).id) || [...unretained].some(id => fullResults.has(id)))) fail("retention_failure_overlap");
    const expectedEffects = journey.shardReferences.map((reference: Json, index: number) => ({ id: "search-shard-evidence", evidenceReference: reference, cleanupClosed: shards[index]!.cleanup.closed }));
    if (failure) expectedEffects.push(failure);
    same(journey.effects, expectedEffects, "effect_references");
    const artifactIds = wire.artifactLifecycle.artifacts.filter((artifact: Json) => /^search-shard-/.test(artifact.id)).map((artifact: Json) => artifact.id);
    if (artifactIds.length !== usedArtifacts.size || artifactIds.some((id: string) => !usedArtifacts.has(id))) fail("unreferenced_shard");
    const seenResults = new Set<string>();
    const results: SearchScheduleResult[] = journey.coverage.results.map((summary: Json) => {
      if (!object(summary) || seenResults.has(summary.id) || Object.hasOwn(summary, "evidence")) fail("summary_result");
      seenResults.add(summary.id);
      const full = fullResults.get(summary.id);
      if (!full) {
        if (Object.hasOwn(summary, "evidenceReference")) fail("missing_schedule_reference");
        if (unretained.has(summary.id)) {
          if (summary.status !== "failed" || !Array.isArray(summary.issues) || !summary.issues.includes("search-shard-retention-failed")) fail("retention_failure_result");
        } else if (summary.status !== "blocked" || summary.executed !== false) fail("missing_schedule_reference");
        return summary as SearchScheduleResult;
      }
      const { evidenceReference, ...small } = summary;
      const { evidence: _evidence, ...expected } = full.result;
      same(evidenceReference, full.reference, "result_reference");
      same(small, expected, "result_summary_mismatch");
      return full.result;
    });
    if (seenResults.size !== searchContractSpec().schedules.length || [...fullResults.keys(), ...unretained].some(id => !seenResults.has(id))) fail("unreferenced_schedule");
    const coverage = accountSearchCoverage(searchContractSpec().schedules, results);
    const { results: _summaryResults, ...summaryCoverage } = journey.coverage;
    const { results: _fullResults, ...fullCoverage } = coverage;
    same(summaryCoverage, fullCoverage, "coverage_summary_mismatch");
    const cleanup = failure ? journey.cleanup : aggregateCleanup(shards.map(shard => shard.cleanup));
    if (!failure) same(journey.cleanup, cleanup, "cleanup_summary_mismatch");
    same(journey.pass, coverage.complete && cleanup.closed, "journey_pass_mismatch");
    same(journey.assertions, coverage.results.filter(result => result.status !== "notApplicable").map(result => ({ id: result.id, pass: result.status === "passed" })), "journey_assertions_mismatch");
    return { ...journey, coverage: { ...coverage, results: coverage.results.map(materializeResult) }, effects: [...shards.flatMap(shard => shard.effects), ...failedRetention], cleanup } as SearchJourneyReceipt;
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("invalid_search_shard_receipt:")) throw error;
    return fail("unreadable_or_invalid_shard");
  }
}
