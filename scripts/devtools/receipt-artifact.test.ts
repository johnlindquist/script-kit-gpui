import { afterEach, expect, spyOn, test } from "bun:test";
import { mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildArtifactLifecycle, claimOutput, emptyOwnedCleanup, validateArtifact, validateOutputTarget, writeJsonArtifactAtomic } from "../agentic/artifact-lifecycle.ts";
import { annotateOwnedEvidence, commitOwnedReport, runDesign } from "./design.ts";
import { runStories } from "./stories.ts";
import { emitValidatedReceipt, prepareValidatedReceipt, validateReceipt, validateReceiptFile } from "./lib/receipt-schema.ts";
import { compactOwnedReceipt, MAX_RECEIPT_DETAIL_BYTES, OBSERVATION_SPEC, readReceiptDocument, resolveReceiptDetails } from "./lib/receipt-artifact.ts";
import { familyCampaignIssues } from "./lib/fixture-contract.ts";
import { PRODUCTION_STORIES, observeStoryTests, productionStoryReceiptIssues, selectStoryTests } from "./lib/story-contract.ts";
import { productStatic, sanitizeReceipt, userContent } from "./lib/privacy.ts";
import { currentIdentity, discoverReceipts, receiptStaleReasons } from "./consistency.ts";
import { buildRuntimeCoverageScorecard, discoverRuntimeCoverageReceipts } from "./lib/runtime-coverage.ts";
import type { Json } from "./driver.ts";

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

function candidate(): Json {
  return { schemaVersion: 2, tool: "script-kit-devtools.design", command: "design.discover", classification: "ok",
    evidenceClass: "STATIC_INVENTORY", provesRuntimeBehavior: false,
    artifactReference: { manifestPath: "target-agent/artifacts/fixture/manifest.json", manifestSha256: "a".repeat(64) },
    observation: productStatic({ fixtures: [{ id: "main.script-list" }] }),
    assertions: [{ id: "observed", pass: true }], cleanup: emptyOwnedCleanup(), errors: [], warnings: [] };
}

function retained(input = candidate(), kind: "directory" | "receipt" = "directory") {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "receipt-reference-"))); roots.push(root);
  const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output", kind === "directory" ? "proof" : "proof.json"), kind, probeId: "receipt-contract" }));
  const primitiveId = input.tool === "script-kit-devtools.stories" ? "devtools.stories.run" : "devtools.design.run";
  const prepared = prepareValidatedReceipt(primitiveId, input);
  expect(prepared.validation.errors).toEqual([]);
  const wire = commitOwnedReport(claim, prepared.receipt, emptyOwnedCleanup());
  return { root, claim, primitiveId, detail: prepared.receipt, wire, payloadPath: join(claim.artifactsRoot, "observation.json") };
}

function campaign(): Json {
  const fixtures = Array.from({ length: 122 }, (_, index) => {
    const descriptor = { id: `contract.${index}`, root: "main", family: "main", owner: "src/fixture.rs", factoryOwners: ["src/fixture.rs"],
      expectedSemanticSurface: "scriptList", requiredSemanticIds: ["input:filter"], appViewVariant: "ScriptList" };
    const identity = { windowId: "main", windowGeneration: 1, appViewVariant: "ScriptList", targetGeneration: 1, surfaceGeneration: 1,
      dataGeneration: 1, presentationRevision: 1, themeRevision: 1, frameGeneration: 1 };
    const effects = [{ frame: { target: identity }, frameIdentity: { target: identity }, status: "captured", source: "gpuiRenderReadback", scope: "liveAutomationWindowRenderReadback",
      capture: { width: 400, height: 300 }, state: { targetIdentity: identity, windowVisible: false, window: { id: "main", generation: 1, visible: false, focused: false },
        surfaceContract: { presentation: { shellOwner: "main", inputOwner: "InputState", themeOwner: "Theme", rowPrimitive: "unifiedListItem" } } },
      elements: { targetIdentity: identity, semanticSurface: "scriptList", elements: [{ semanticId: "input:filter" }] },
      layout: { targetIdentity: identity, components: [{ bounds: { x: 0, y: 0, width: 400, height: 300 } }] } }];
    return { id: descriptor.id, binding: { descriptor }, effects, pass: true, assertions: [{ id: "observed", pass: true }] };
  });
  return { ...candidate(), command: "design.run", evidenceClass: "DIRECT_RUNTIME_PROOF", provesRuntimeBehavior: true,
    observation: annotateOwnedEvidence({ scenario: "production-family-matrix", catalogue: fixtures.map(value => value.binding.descriptor), fixtures }) };
}

test("122 detailed fixture effects are owned once; stdout, disk and validation stay compact", () => {
  const { claim, wire, detail, payloadPath, primitiveId } = retained(campaign());
  const persisted = readReceiptDocument(claim.receiptPath);
  expect(persisted).toEqual(wire);
  expect(persisted.observation).toBeUndefined();
  expect(readReceiptDocument(payloadPath).receipt).toEqual(detail);
  expect(readdirSync(claim.artifactsRoot)).toEqual(["observation.json"]);
  expect(Buffer.byteLength(JSON.stringify(persisted))).toBeLessThan(Buffer.byteLength(JSON.stringify(detail)) / 20);
  expect(resolveReceiptDetails(persisted)).toEqual(detail);
  expect(familyCampaignIssues(persisted)).toEqual([]);
  expect(validateReceipt(primitiveId, persisted)).toEqual(validateReceipt(primitiveId, detail));
  expect(validateReceiptFile(primitiveId, claim.receiptPath).receipt).toEqual(wire);
  const output = spyOn(console, "log").mockImplementation(() => {});
  try {
    expect(emitValidatedReceipt(primitiveId, wire)).toEqual(wire);
    expect(JSON.parse(String(output.mock.calls[0]![0]))).toEqual(wire);
  } finally { output.mockRestore(); }
});

test("bound family effects decide qualification rather than compact pass flags", () => {
  const sanitized = prepareValidatedReceipt("devtools.design.run", campaign()).receipt as Json;
  sanitized.observation.fixtures[0].effects[0].elements.elements = [];
  const { claim } = retained();
  const other = claimOutput(validateOutputTarget({ repoRoot: claim.plan.repoRoot, candidate: join(claim.plan.repoRoot, ".test-output", "bad-family"), kind: "directory", probeId: "receipt-contract" }));
  const wire = commitOwnedReport(other, sanitized, emptyOwnedCleanup());
  expect(familyCampaignIssues(wire).join(" ")).toContain("fixture_required_control_missing");
  expect(validateReceipt("devtools.design.run", wire).valid).toBe(false);
  expect(prepareValidatedReceipt("devtools.design.run", wire).exitCode).not.toBe(0);
});

test("privacy and proof calibration survive in the sole detailed payload", () => {
  const input = candidate();
  input.observation = { text: userContent("receipt-private-value"), calibration: productStatic({ rgba: [1, 2, 3, 4], scale: 2, exact: true }) };
  const { wire, payloadPath, detail, claim } = retained(input);
  expect(readFileSync(payloadPath, "utf8")).not.toContain("receipt-private-value");
  expect(JSON.stringify(wire)).not.toContain("receipt-private-value");
  expect(JSON.stringify(wire)).not.toContain(claim.owner.token);
  expect(readFileSync(payloadPath, "utf8")).not.toContain(claim.owner.token);
  expect(resolveReceiptDetails(wire)).toEqual(detail);
  expect((resolveReceiptDetails(wire).observation as Json).calibration).toEqual({ rgba: [1, 2, 3, 4], scale: 2, exact: true });
});

test("provider waits retain typed ownership and reason codes while source queries stay private", () => {
  const query = { lifetime: 1, revision: 2, scopeRevision: 3 };
  const owner = { source: "brain-semantic", generation: 4, workQuery: "private query", workScope: "private scope",
    consumer: query, publicationPolicy: "visible", queryBound: true, terminal: null };
  const run = { id: 5, source: "brain-semantic", generation: 4, query: "private query", kind: "worker", state: "held", publicationPolicy: "visible", outcome: null };
  const wait = { version: 1, source: "brain-semantic", query, afterRunId: 3, status: "admitted", owner, run, blockers: [], pendingDesired: false, availabilityReason: "heldCurrentRun" };
  const settled = { ...wait, status: "settled", owner: { ...owner, terminal: "empty" }, run: { ...run, state: "completed", outcome: "empty" }, availabilityReason: "empty" };
  const cache = { source: "tabs", query, cacheIdentity: "private cached query", cacheStateRevision: 7, rowCount: 0 };
  const cached = { ...wait, source: "tabs", afterRunId: 0, status: "cached", owner: null, run: null, cache,
    pendingDesired: true, availabilityReason: "sourceCacheReuse" };
  const desired = { source: "brain-semantic", query, workQuery: "private query", workScope: "private scope", publicationPolicy: "visible" };
  const value = sanitizeReceipt(annotateOwnedEvidence({ searchProvider: wait, settled, desired, cached,
    sourceCacheReadiness: [{ ...cache, cacheIdentity: "success" }],
    malformed: { desired: { ...desired, publicationPolicy: "private policy" } }, unknown: { ...wait, availabilityReason: "private reason" } }),
    { mode: "fixture-redacted", fixtureId: "provider-wait" }).sanitized as Json;
  expect(value.searchProvider.availabilityReason).toBe("heldCurrentRun");
  expect(value.searchProvider.owner.publicationPolicy).toBe("visible");
  expect(value.searchProvider.run.state).toBe("held");
  expect(value.searchProvider.owner.consumer).toEqual(query);
  expect(value.settled.availabilityReason).toBe("empty");
  expect(value.settled.owner.terminal).toBe("empty");
  expect(value.settled.run.outcome).toBe("empty");
  expect(value.desired.publicationPolicy).toBe("visible");
  expect(value.desired.query).toEqual(query);
  expect(typeof value.desired.workQuery).toBe("object");
  expect(typeof value.desired.workScope).toBe("object");
  expect(typeof value.malformed.desired.publicationPolicy).toBe("object");
  expect(value.cached.availabilityReason).toBe("sourceCacheReuse");
  expect(value.cached.owner).toBeNull();
  expect(value.cached.run).toBeNull();
  expect(value.cached.cache.query).toEqual(query);
  expect(value.cached.cache.rowCount).toBe(0);
  expect(typeof value.cached.cache.cacheIdentity).toBe("object");
  expect(typeof value.sourceCacheReadiness[0].cacheIdentity).toBe("object");
  expect(typeof value.unknown.availabilityReason).toBe("object");
  expect(JSON.stringify(value)).not.toContain("private ");
});

test("File Search stream evidence preserves typed phases without exposing query paths or diagnostics", () => {
  const stream = { generation: 4, query: "private query", directory: "/private/corpus", showHidden: false,
    phase: "completed", loading: false, resultCount: 2, visibleCount: 1, failure: null };
  const value = sanitizeReceipt(annotateOwnedEvidence({ fileSearchStream: stream,
    fileSearch: { stream: { ...stream, phase: "running", loading: true } },
    failed: { fileSearchStream: { ...stream, phase: "failed", failure: "private source error" } },
    malformed: { fileSearchStream: { ...stream, phase: "private phase" } }, unrelated: stream }),
    { mode: "fixture-redacted", fixtureId: "file-search-stream" }).sanitized as Json;
  expect(value.fileSearchStream.phase).toBe("completed");
  expect(value.fileSearch.stream.phase).toBe("running");
  expect(value.failed.fileSearchStream.phase).toBe("failed");
  expect(value.fileSearchStream.query.contentKind).toBe("UserContent");
  expect(value.fileSearchStream.directory.contentKind).toBe("FilePath");
  expect(value.failed.fileSearchStream.failure.contentKind).toBe("Diagnostic");
  expect(value.fileSearchStream.generation).toBe(4);
  expect(value.fileSearchStream.resultCount).toBe(2);
  expect(value.fileSearchStream.visibleCount).toBe(1);
  expect(value.fileSearchStream.failure).toBeNull();
  expect(typeof value.malformed.fileSearchStream.phase).toBe("object");
  expect(typeof value.unrelated.phase).toBe("object");
  expect(JSON.stringify(value)).not.toContain("private");
});

test("preview readiness preserves typed held evidence without exposing paths or queries", () => {
  const preview = { version: 1, generation: 7, query: "private query", workSequence: 3, phase: "held",
    path: "/private/preview.png", decoded: true, contentHash: "a".repeat(64), logicalTimeMs: 0, dueAtMs: 64 };
  const value = sanitizeReceipt(annotateOwnedEvidence({ fileSearchPreview: preview, pendingPreviewCompletions: [preview],
    malformed: { fileSearchPreview: { ...preview, phase: "private phase" } }, unrelated: preview }),
    { mode: "fixture-redacted", fixtureId: "file-search-preview" }).sanitized as Json;
  expect(value.fileSearchPreview.phase).toBe("held");
  expect(value.pendingPreviewCompletions[0].phase).toBe("held");
  expect(value.fileSearchPreview.query.contentKind).toBe("UserContent");
  expect(value.fileSearchPreview.path.contentKind).toBe("FilePath");
  expect(value.fileSearchPreview.decoded).toBe(true);
  expect(value.fileSearchPreview.workSequence).toBe(3);
  expect(value.fileSearchPreview.dueAtMs).toBe(64);
  expect(typeof value.malformed.fileSearchPreview.phase).toBe("object");
  expect(typeof value.unrelated.phase).toBe("object");
  expect(JSON.stringify(value)).not.toContain("private");
});

test("frame-bundle discovery stays readable without promoting arbitrary scope text", () => {
  const capability = { version: 1, requiresFrameCursor: true, pageScope: "captureBundle", decodedScope: "complete" };
  const value = sanitizeReceipt(annotateOwnedEvidence({ frameCursor: { captureHistoryBundle: capability },
    malformed: { captureHistoryBundle: { ...capability, pageScope: "private scope" } }, unrelated: capability }),
    { mode: "fixture-redacted", fixtureId: "frame-bundle-discovery" }).sanitized as Json;
  expect(value.frameCursor.captureHistoryBundle).toEqual(capability);
  expect(typeof value.malformed.captureHistoryBundle.pageScope).toBe("object");
  expect(typeof value.malformed.captureHistoryBundle.decodedScope).toBe("object");
  expect(typeof value.unrelated.pageScope).toBe("object");
  expect(typeof value.unrelated.decodedScope).toBe("object");
  expect(JSON.stringify(value)).not.toContain("private scope");
});

test.each(["directory", "receipt"] as const)("%s ownership binds missing, tampered and aliased detail fail closed", kind => {
  const { wire, claim, payloadPath, primitiveId } = retained(candidate(), kind);
  expect(validateReceipt(primitiveId, wire).valid).toBe(true);
  const original = readFileSync(payloadPath);
  writeFileSync(payloadPath, Buffer.from(original.toString().replace("script-list", "script-lost")));
  expect(validateReceipt(primitiveId, wire).valid).toBe(false);
  writeFileSync(payloadPath, original);
  const saved = `${payloadPath}.saved`;
  renameSync(payloadPath, saved);
  expect(validateReceipt(primitiveId, wire).valid).toBe(false);
  expect(validateReceiptFile(primitiveId, claim.receiptPath).exitCode).toBe(4);
  symlinkSync(saved, payloadPath);
  expect(validateReceipt(primitiveId, wire).valid).toBe(false);
  rmSync(payloadPath); renameSync(saved, payloadPath);
  const marker = readReceiptDocument(claim.markerPath);
  writeFileSync(claim.markerPath, JSON.stringify({ ...marker, token: "stale-token" }));
  expect(validateReceipt(primitiveId, wire).valid).toBe(false);
});

test("traversal, identity, hash/size, duplicate artifacts and nested references are refused", () => {
  const { wire, primitiveId, payloadPath, claim } = retained();
  const changes: Array<(value: Json) => void> = [
    value => { value.artifactLifecycle.artifacts[0].relativePath = "../observation.json"; },
    value => { value.artifactLifecycle.artifacts[0].path = "/outside/observation.json"; },
    value => { value.artifactLifecycle.artifacts[0].bytes++; },
    value => { value.artifactLifecycle.artifacts[0].bytes = MAX_RECEIPT_DETAIL_BYTES + 1; },
    value => { value.artifactLifecycle.artifacts[0].sha256 = "0".repeat(64); },
    value => { value.artifactLifecycle.artifacts.push(value.artifactLifecycle.artifacts[0]); },
    value => { value.detailReference.artifactId = "another"; },
    value => { value.artifactLifecycle.output.runId = "another"; },
    value => { value.receiptFormatVersion = 0; },
    value => { value.pass = false; },
    value => { value.observation = { fixtures: [] }; },
  ];
  for (const change of changes) {
    const invalid = structuredClone(wire); change(invalid);
    expect(validateReceipt(primitiveId, invalid).valid).toBe(false);
    expect(prepareValidatedReceipt(primitiveId, invalid).exitCode).toBe(4);
  }
  const document = readReceiptDocument(payloadPath);
  document.receipt = wire;
  writeFileSync(payloadPath, JSON.stringify(document));
  const recursive = structuredClone(wire);
  recursive.artifactLifecycle.artifacts = [validateArtifact(payloadPath, OBSERVATION_SPEC, claim.artifactsRoot)];
  expect(validateReceipt(primitiveId, recursive).errors.join(" ")).toContain("nested_reference");
});

test("story and design diagnose verify details without printing them", async () => {
  const names = PRODUCTION_STORIES.map(story => story.testLeaf);
  const listed = names.map(name => `${name}: test`).join("\n");
  const output = `running ${names.length} tests\n${names.map(name => `test ${name} ... ok`).join("\n")}\ntest result: ok. ${names.length} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n`;
  const story = retained({ ...candidate(), tool: "script-kit-devtools.stories", command: "stories.run", lane: "library",
    evidenceClass: "UNIT_BEHAVIOR", library: annotateOwnedEvidence({ selection: selectStoryTests(listed), execution: observeStoryTests(output, 0, names) }), journeys: [] });
  expect(productionStoryReceiptIssues(story.wire)).toEqual([]);
  const design = retained();
  const logging = spyOn(console, "log").mockImplementation(() => {});
  try {
    await runDesign(["diagnose", "--receipt", design.claim.receiptPath]);
    await runStories(["diagnose", "--receipt", story.claim.receiptPath]);
    for (const [entry] of logging.mock.calls) {
      const result = JSON.parse(String(entry));
      expect(result.historicalValidation.exitCode).toBe(0);
      expect(result.historicalValidation.receipt.receiptFormatVersion).toBe(1);
      expect(result.historicalValidation.receipt.library).toBeUndefined();
      expect(result.historicalValidation.receipt.observation).toBeUndefined();
    }
  } finally { logging.mockRestore(); }
  rmSync(story.payloadPath);
  expect(productionStoryReceiptIssues(story.wire).length).toBeGreaterThan(0);
  expect(validateReceiptFile(story.primitiveId, story.claim.receiptPath).exitCode).toBe(4);
});

test("evidence discovery resolves one receipt rather than double-counting detail", () => {
  const { root, wire, payloadPath } = retained();
  const discovered = discoverReceipts(join(root, ".test-output"));
  expect(discovered.receipts).toHaveLength(1);
  expect(discovered.receipts[0]!.receipt.observation).toBeDefined();
  expect(discoverRuntimeCoverageReceipts(join(root, ".test-output"))).toHaveLength(1);
  rmSync(payloadPath);
  expect(discoverReceipts(join(root, ".test-output")).unreadablePaths.length).toBeGreaterThan(0);
  const scorecard = buildRuntimeCoverageScorecard([], [{ path: "missing.json", receipt: wire }]);
  expect(scorecard.rejectedReceipts).toHaveLength(1);
});

test("all 122 retained captures remain valid without an arbitrary artifact-count ceiling", () => {
  const { claim, detail } = retained();
  const specs = [OBSERVATION_SPEC];
  for (let index = 0; index < 122; index++) {
    const sourceName = `capture-${index}.json`;
    writeJsonArtifactAtomic(claim, sourceName, { calibration: { rgba: [1, 2, 3, 4], scale: 2 }, index });
    specs.push({ id: `capture-${index}`, sourceName, required: true, mediaType: "application/json", kind: "json" });
  }
  const artifacts = specs.map(spec => validateArtifact(join(claim.artifactsRoot, spec.sourceName), spec, claim.artifactsRoot));
  const lifecycle = buildArtifactLifecycle({ claim, finalizationKind: "driver-close", writersFinalized: true, specs, artifacts });
  const wire = compactOwnedReceipt(claim, detail, lifecycle);
  expect(resolveReceiptDetails(wire)).toEqual(detail);
  expect(wire.artifactLifecycle.artifacts).toHaveLength(123);
  writeFileSync(join(claim.artifactsRoot, "capture-121.json"), "{}");
  expect(validateReceipt("devtools.design.run", wire).valid).toBe(false);
});

test("supplied receipt symlinks and direct stale-check inputs cannot bypass resolution", () => {
  const { wire, claim, payloadPath } = retained();
  const alias = join(claim.root, "alias.json"); symlinkSync(claim.receiptPath, alias);
  expect(validateReceiptFile("devtools.design.run", alias).exitCode).toBe(4);
  rmSync(payloadPath);
  expect(receiptStaleReasons({ path: claim.receiptPath, receipt: wire, disposition: "EVALUABLE_PASS", archived: false }, currentIdentity()).map(reason => reason.code)).toContain("invalid-receipt-reference");
});
