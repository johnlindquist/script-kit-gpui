import { deflateSync } from "node:zlib";
import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createArtifactFixture } from "../agentic/build-artifact-fixture.ts";
import { verifyImmutableArtifact, type VerifiedArtifact } from "../agentic/build-artifact.ts";
import { claimOutput, validateOutputTarget, createOwnedStagingDirectory, emptyOwnedCleanup } from "../agentic/artifact-lifecycle.ts";
import { issueOwnedEvaluationPermit, consumeOwnedEvaluationPermit, assertOwnedEvaluationCommand, ownedEvaluationEnvironment,
  OWNED_EVALUATION_LIMITS, OWNED_EVALUATION_POLICY_SHA256, type OwnedEvaluationPermit } from "./lib/operator-safety.ts";
import { OwnedEvaluationClient, NATIVE_SAFETY_PROBES, OWNED_SEARCH_CACHE_SOURCES, type NativeSafetyProbeResult, type OwnedFrameCursor } from "./lib/owned-evaluation.ts";
import { annotateOwnedEvidence, nativeSafetyProbeAssertions } from "./design.ts";
import { sanitizeReceipt } from "./lib/privacy.ts";
import { DriverCommandRefused, ProtocolCore, OWNED_RESPONSE_CODEC, OWNED_RESPONSE_ENCODING, PROTOCOL_VERSION, type Driver, type Json } from "./driver.ts";
import type { AutomationTargetSnapshot, OwnedRuntimeIdentity } from "./lib/target-identity.ts";

const cleanups: Array<() => void> = [];
afterEach(() => { for (const cleanup of cleanups.splice(0).reverse()) cleanup(); });
test("owned observations classify sensitive containers without losing structured metadata", () => {
  const scan = sanitizeReceipt(annotateOwnedEvidence({
    clock: "parent-monotonic",
    unexpectedTiming: { clock: "private clock description" },
    terminalFallbackCompletionKind: "synchronousRefusal",
    unexpectedCompletion: { terminalFallbackCompletionKind: "private completion detail" },
    reliability: { diagnostic: { code: "provider_timeout", raw: "private diagnostic bytes" } },
    transcriptScroll: { anchorId: "private message identity", offset: 17 },
    composer: { text: "private draft bytes" },
    credentials: { cookie: "private credential bytes" },
    errors: [{ code: "refused", detail: "private failure bytes" }],
  }), { mode: "fixture-redacted", fixtureId: "owned-container-classification" });
  const value = scan.sanitized as Json;
  expect(scan.unclassifiedSensitivePaths).toEqual([]);
  expect(scan.rawContentReturned).toBe(false);
  expect(value.clock).toBe("parent-monotonic");
  expect(value.unexpectedTiming.clock.redacted).toBe(true);
  expect(value.terminalFallbackCompletionKind).toBe("synchronousRefusal");
  expect(value.unexpectedCompletion.terminalFallbackCompletionKind.redacted).toBe(true);
  expect(value.reliability.diagnostic.code).toBe("provider_timeout");
  expect(value.reliability.diagnostic.raw.contentKind).toBe("Diagnostic");
  expect(value.transcriptScroll.offset).toBe(17);
  expect(value.transcriptScroll.anchorId.contentKind).toBe("UserContent");
  expect(value.composer.text.redacted).toBe(true);
  expect(value.credentials.cookie.contentKind).toBe("Secret");
  expect(value.errors[0].code).toBe("refused");
  expect(JSON.stringify(value)).not.toContain("private ");
});
function fixture(features: readonly string[] = ["owned-ui-evaluation"]) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "owned-evaluator-contract-")));
  cleanups.push(() => rmSync(root, { recursive: true, force: true }));
  const published = createArtifactFixture(root, { features }); cleanups.push(published.dispose);
  const artifact = verifyImmutableArtifact(root, published.reference, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" });
  const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/evaluation"), kind: "directory", probeId: "evaluation-test" }));
  return { artifact, claim };
}
test("owned permits require verified feature capability, cannot serialize, and are consumed once", () => {
  const { artifact, claim } = fixture();
  expect(() => issueOwnedEvaluationPermit({ ...artifact } as VerifiedArtifact, claim, ["main.script-list"])).toThrow("verified evaluator application required");
  const permit = issueOwnedEvaluationPermit(artifact, claim, ["main.script-list"]);
  expect(() => consumeOwnedEvaluationPermit(JSON.parse(JSON.stringify(permit)) as OwnedEvaluationPermit)).toThrow("forged or consumed permit");
  const facts = consumeOwnedEvaluationPermit(permit);
  expect(facts.artifact).toBe(artifact);
  expect(Object.isFrozen(facts.claim)).toBe(true);
  expect(Object.isFrozen(facts.claim.owner)).toBe(true);
  expect(() => consumeOwnedEvaluationPermit(permit)).toThrow("forged or consumed permit");
  expect(() => assertOwnedEvaluationCommand(permit, { type: "design", command: { operation: "mount", fixtureId: "notes.editor" } })).toThrow("fixture outside sealed subset");
  expect(() => assertOwnedEvaluationCommand(permit, { type: "show" })).toThrow("operation is not evaluator-local");
  expect(() => assertOwnedEvaluationCommand(permit, { type: "getState", target: { type: "main" } })).toThrow("exact mounted instance required");
  expect(() => assertOwnedEvaluationCommand(permit, { type: "getState", target: { type: "instance", id: "main", generation: 1 } })).not.toThrow();
  const capture = { operation: "captureFrame", target: { type: "instance", id: "main", generation: 1 }, includeImage: false };
  expect(() => assertOwnedEvaluationCommand(permit, { type: "design", command: capture })).not.toThrow();
  for (const target of [{ type: "main" }, { type: "instance", id: "main" }, { type: "instance", id: "", generation: 1 }])
    expect(() => assertOwnedEvaluationCommand(permit, { type: "design", command: { ...capture, target } })).toThrow("exact mounted instance required");
  for (const extra of [{ includeImage: undefined }, { includeImage: "true" }, { expected: initial }])
    expect(() => assertOwnedEvaluationCommand(permit, { type: "design", command: { ...capture, ...extra } })).toThrow("invalid atomic capture command");
});

test("owned scheduled capture admits exact expectations and rejects malformed or widened payloads", () => {
  const { artifact, claim } = fixture();
  const permit = issueOwnedEvaluationPermit(artifact, claim, ["main-search-contract"]); consumeOwnedEvaluationPermit(permit);
  const target = { type: "instance", id: initial.windowId, generation: initial.windowGeneration };
  const scheduled = { expected: { ...initial }, afterFrameGeneration: 0, afterNotificationEpoch: 0 };
  const command = { operation: "captureFrame", target, includeImage: false, scheduled };
  const check = (body: Json) => assertOwnedEvaluationCommand(permit, { type: "design", command: body });
  expect(() => check(command)).not.toThrow();
  for (const frameCursor of [{ traceGeneration: 1, afterFrameGeneration: 0 }, null, { traceGeneration: 1, afterFrameGeneration: 0.5 },
      { traceGeneration: 1, afterFrameGeneration: 0, extra: true }]) {
    // Only the sealed owned transport admits this field; the native parser owns deliberate malformed probes.
    expect(() => check({ ...command, frameCursor })).not.toThrow();
    for (const extra of [{ nativeCapture: true }, { focus: true }, { screen: "desktop" }, { path: "/tmp/other.png" }])
      expect(() => check({ ...command, frameCursor, ...extra })).toThrow("invalid atomic capture command");
  }
  for (const value of [null, undefined, {}, { ...scheduled, extra: true }, { ...scheduled, afterFrameGeneration: -1 },
      { ...scheduled, afterNotificationEpoch: 0.5 }, { ...scheduled, afterNotificationEpoch: Number.MAX_SAFE_INTEGER + 1 }])
    expect(() => check({ ...command, scheduled: value })).toThrow("invalid scheduled capture command");
  for (const expected of [null, {}, { ...initial, windowId: "other" }, { ...initial, windowGeneration: 3 },
      { ...initial, dataGeneration: -1 }, { ...initial, frameGeneration: undefined }, { ...initial, query: "injected" }])
    expect(() => check({ ...command, scheduled: { ...scheduled, expected } })).toThrow("exact target expectation required");
  expect(() => check({ ...command, nativeCapture: true })).toThrow("invalid atomic capture command");
});

test("owned frame acknowledgement requires the sealed fixture and an exact bounded command", () => {
  const { artifact, claim } = fixture();
  const permit = issueOwnedEvaluationPermit(artifact, claim, ["main-search-contract"]); consumeOwnedEvaluationPermit(permit);
  const command = { operation: "acknowledgeFrames", target: { type: "instance", id: initial.windowId, generation: initial.windowGeneration },
    expected: { ...initial, frameGeneration: 19 }, cursor: { traceGeneration: 7, afterFrameGeneration: 19 } };
  const check = (body: Json) => assertOwnedEvaluationCommand(permit, { type: "design", command: body });
  expect(() => check(command)).not.toThrow();
  for (const cursor of [undefined, null, {}, { ...command.cursor, afterFrameGeneration: -1 },
      { ...command.cursor, traceGeneration: 1.5 }, { ...command.cursor, clearError: true }])
    expect(() => check({ ...command, cursor })).toThrow("invalid frame acknowledgement cursor");
  expect(() => check({ ...command, expected: { ...command.expected, windowId: "other" } })).toThrow("exact target expectation required");
  expect(() => check({ ...command, draw: true })).toThrow("frame acknowledgement outside sealed fixture");
  const ordinary = issueOwnedEvaluationPermit(artifact, claim, ["main.script-list"]); consumeOwnedEvaluationPermit(ordinary);
  expect(() => assertOwnedEvaluationCommand(ordinary, { type: "design", command })).toThrow("frame acknowledgement outside sealed fixture");
});

test("owned search control admits only its bounded wire shape on the sealed search fixture", () => {
  const { artifact, claim } = fixture();
  const permit = issueOwnedEvaluationPermit(artifact, claim, ["main-search-contract"]); consumeOwnedEvaluationPermit(permit);
  const target = { type: "instance", id: initial.windowId, generation: initial.windowGeneration };
  const check = (control: Json, expected: Json = initial) => assertOwnedEvaluationCommand(permit,
    { type: "design", command: { operation: "fixtureControl", target, expected, control: { family: "search", ...control } } });
  for (const control of [{ operation: "prepare", scenario: "tab-domain-hoist" }, { operation: "release", runIds: [7] },
      { operation: "release", runIds: [7, 9] }, { operation: "advance", milliseconds: 1000 }])
    expect(() => check(control)).not.toThrow();
  for (const control of [{ operation: "prepare", scenario: "../arbitrary-path" }, { operation: "prepare", scenario: "tab-domain-hoist", rows: [] },
      { operation: "release", runId: 7 }, { operation: "release", runIds: [] }, { operation: "release", runIds: [0] },
      { operation: "release", runIds: [1.5] }, { operation: "release", runIds: [7, 7] },
      { operation: "release", runIds: Array.from({ length: 129 }, (_, index) => index + 1) },
      { operation: "advance", milliseconds: 1001 }, { operation: "replaceRows" }])
    expect(() => check(control)).toThrow("invalid search control command");
  expect(() => check({ operation: "release", runIds: [7] }, { ...initial, windowId: "other" })).toThrow("exact target expectation required");
  const ordinary = issueOwnedEvaluationPermit(artifact, claim, ["main.script-list"]); consumeOwnedEvaluationPermit(ordinary);
  expect(() => assertOwnedEvaluationCommand(ordinary, { type: "design", command: { operation: "fixtureControl", target, expected: initial,
    control: { family: "search", operation: "prepare", scenario: "tab-domain-hoist" } } })).toThrow("search control outside sealed fixture");
});

test("retained pixel probe admission is native-resolution, bounded and revision-targeted", () => {
  const { artifact, claim } = fixture();
  const permit = issueOwnedEvaluationPermit(artifact, claim, ["main-search-contract"]); consumeOwnedEvaluationPermit(permit);
  const target = { type: "instance", id: initial.windowId, generation: initial.windowGeneration };
  const request = { target, expected: { ...initial, frameGeneration: 1 }, includeImage: false, hiDpi: true, probes: [{ x: 0, y: 0 }] };
  const check = (value: Json) => assertOwnedEvaluationCommand(permit, { type: "captureRenderWindow", request: value });
  expect(() => check(request)).not.toThrow();
  for (const probes of [[], Array.from({ length: 65 }, () => ({ x: 0, y: 0 })), [{ x: -1, y: 0 }], [{ x: 0.5, y: 0 }],
      [{ x: 0x100000000, y: 0 }], [{ x: 0, y: 0, radius: 1 }]]) expect(() => check({ ...request, probes })).toThrow("invalid retained pixel probes");
  expect(() => check({ ...request, hiDpi: false })).toThrow("invalid retained pixel probes");
  expect(() => check({ ...request, expected: initial })).toThrow("invalid retained pixel probes");
  expect(() => check({ ...request, expected: { ...request.expected, windowId: "other" } })).toThrow("exact target expectation required");
});

test("owned coordinate admission requires the full exact completed frame, never a newer target hint", () => {
  const { artifact, claim } = fixture();
  const permit = issueOwnedEvaluationPermit(artifact, claim, ["main-search-contract"]); consumeOwnedEvaluationPermit(permit);
  const target = { type: "instance", id: initial.windowId, generation: initial.windowGeneration };
  const expected = { ...initial, frameGeneration: 7 };
  const frame = { ...identity, requestedTarget: target, target: expected };
  const command = { type: "simulateGpuiEvent", target, expected, expectedFrame: frame, event: { type: "mouseDown", x: 12, y: 40, button: "left" } };
  expect(() => assertOwnedEvaluationCommand(permit, command)).not.toThrow();
  for (const expectedFrame of [undefined, null, { ...frame, target: { ...expected, frameGeneration: 8 } },
      { ...frame, requestedTarget: { ...target, id: "other" } }, { ...frame, target: { ...expected, dataGeneration: 9 } },
      { ...frame, processInstanceId: "" }, { ...frame, binarySha256: "wrong" }, { ...frame, additional: true }])
    expect(() => assertOwnedEvaluationCommand(permit, { ...command, expectedFrame })).toThrow("exact completed pointer frame required");
});

test("typed source-change refusal stays separate from a native worker terminal in retained metadata", () => {
  const value = sanitizeReceipt(annotateOwnedEvidence({ runs: [
    { id: 1, kind: "sourceChange", source: "brain-lexical", query: "private source query", generation: 0, state: "awaiting-admission",
      publicationPolicy: null, outcome: null, capabilityRefusal: "synchronous_source_has_no_worker" },
    { id: 2, kind: "worker", source: "tabs", query: "private tab query", generation: 2, state: "failed", publicationPolicy: "visible", outcome: "disconnected" },
  ] }), { mode: "fixture-redacted", fixtureId: "source-admission-metadata" }).sanitized as Json;
  expect(value.runs[0].state).toBe("awaiting-admission"); expect(value.runs[0].outcome).toBeNull();
  expect(value.runs[0].capabilityRefusal).toBe("synchronous_source_has_no_worker");
  expect(value.runs[1].outcome).toBe("disconnected");
  expect(JSON.stringify(value)).not.toContain("private ");
});
test("owned-copy receipt metadata remains inspectable while copied text stays private", () => {
  const value = sanitizeReceipt(annotateOwnedEvidence({ copySink: { text: "private copied value", receipt: {
    destination: "ownedProcessLocal", byteLength: 20, sha256: "a".repeat(64), revision: 1,
  } } }), { mode: "fixture-redacted", fixtureId: "copy-sink-metadata" }).sanitized as Json;
  expect(value.copySink.receipt.destination).toBe("ownedProcessLocal");
  expect(value.copySink.receipt.sha256).toBe("a".repeat(64));
  expect(value.copySink.receipt.revision).toBe(1);
  expect(JSON.stringify(value)).not.toContain("private copied value");
});
test("finite coverage criteria and factor links remain metadata without exposing arbitrary labels", () => {
  const value = sanitizeReceipt(annotateOwnedEvidence({ caseCriteria: [{ required: ["headers-inert", "private criterion"], proved: ["headers-inert"] }],
    terminalIntents: { required: ["automaticTop"], notApplicable: [{ intent: "explicitAnchor", status: "notApplicable", proof: false, cause: "separateTerminalIntentSchedules" }] },
    factors: [{ outcome: "disconnect", intent: "explicitAnchor", scheduleIds: ["provider-terminal-errors/tabs/disconnect", "private factor"] }],
  }), { mode: "fixture-redacted", fixtureId: "search-coverage-metadata" }).sanitized as Json;
  expect(value.caseCriteria[0].required[0]).toBe("headers-inert"); expect(value.caseCriteria[0].proved).toEqual(["headers-inert"]);
  expect(value.terminalIntents.required).toEqual(["automaticTop"]); expect(value.terminalIntents.notApplicable[0].intent).toBe("explicitAnchor");
  expect(value.factors[0].scheduleIds[0]).toBe("provider-terminal-errors/tabs/disconnect");
  expect(JSON.stringify(value)).not.toContain("private ");
});
test("search evidence references preserve exact public identities without exposing arbitrary labels", () => {
  const scheduleId = "provider-terminal-errors/tabs/disconnect";
  const references = [
    { artifactId: "search-shard-3", shard: 3, scheduleId },
    { artifactId: "search-shard-3", shard: 3, scheduleIds: [scheduleId] },
    { artifactId: "private artifact", shard: 3, scheduleId },
    { artifactId: "search-shard-3", shard: 3, scheduleId: "private schedule" },
    { artifactId: "search-shard-4", shard: 3, scheduleId },
  ];
  const value = sanitizeReceipt(annotateOwnedEvidence({ references }), { mode: "fixture-redacted", fixtureId: "search-evidence-references" }).sanitized as Json;
  expect(value.references.slice(0, 2)).toEqual(references.slice(0, 2));
  for (const reference of value.references.slice(2)) expect(typeof reference.artifactId).toBe("object");
  expect(JSON.stringify(value)).not.toContain("private ");
});
test("compiled comparison labels stay readable while data fingerprints remain private", () => {
  const comparison = { key: "pair:apps+tabs:automatic", order: "apps-then-tabs", expectedOrders: ["apps-then-tabs", "tabs-then-apps", "same-turn"], fingerprint: "a".repeat(64) };
  const value = sanitizeReceipt(annotateOwnedEvidence({ orderComparisons: [comparison, { ...comparison, key: "private comparison" }] }),
    { mode: "fixture-redacted", fixtureId: "search-comparison-metadata" }).sanitized as Json;
  expect(value.orderComparisons[0].key).toBe(comparison.key);
  expect(value.orderComparisons[0].order).toBe(comparison.order);
  expect(value.orderComparisons[0].expectedOrders).toEqual(comparison.expectedOrders);
  expect(typeof value.orderComparisons[0].fingerprint).toBe("object");
  expect(typeof value.orderComparisons[1].key).toBe("object");
  expect(JSON.stringify(value)).not.toContain("private comparison");
});
test("typed terminal receipts retain canonical selection IDs but not arbitrary subjects", () => {
  const terminal = { source: "tabs", requestedOutcome: "error", intent: "explicitAnchor", query: { lifetime: 1, revision: 2, scopeRevision: 3 },
    selectionArmed: true, selectedSemanticId: `main-list-row:v2:${"a".repeat(64)}`, provider: { source: "tabs" } };
  const value = sanitizeReceipt(annotateOwnedEvidence({ terminalReceipts: [terminal, { ...terminal, selectedSemanticId: "private subject" }], unrelated: terminal }),
    { mode: "fixture-redacted", fixtureId: "terminal-selection-metadata" }).sanitized as Json;
  expect(value.terminalReceipts[0].selectedSemanticId).toBe(terminal.selectedSemanticId);
  expect(typeof value.terminalReceipts[1].selectedSemanticId).toBe("object");
  expect(typeof value.unrelated.selectedSemanticId).toBe("object");
  expect(JSON.stringify(value)).not.toContain("private subject");
});
test("owned lifetime limits only narrow authority and participate in policy identity", () => {
  const { artifact, claim } = fixture();
  const defaults = consumeOwnedEvaluationPermit(issueOwnedEvaluationPermit(artifact, claim, []));
  const explicit = consumeOwnedEvaluationPermit(issueOwnedEvaluationPermit(artifact, claim, [], { maxLifetimeMs: OWNED_EVALUATION_LIMITS.maxLifetimeMs }));
  const lower = consumeOwnedEvaluationPermit(issueOwnedEvaluationPermit(artifact, claim, [], { maxLifetimeMs: 3000 }));
  expect(defaults.limits).toEqual(OWNED_EVALUATION_LIMITS);
  expect(defaults.policySha256).toBe(OWNED_EVALUATION_POLICY_SHA256);
  expect(explicit.policySha256).toBe(defaults.policySha256);
  expect(lower.limits).toEqual({ ...OWNED_EVALUATION_LIMITS, maxLifetimeMs: 3000 });
  expect(lower.policySha256).not.toBe(defaults.policySha256);
  expect(Object.isFrozen(lower.limits)).toBe(true);
  const environment = ownedEvaluationEnvironment(lower, createOwnedStagingDirectory(claim));
  expect(JSON.parse(environment.SCRIPT_KIT_OWNED_EVALUATION_LIMITS!).maxLifetimeMs).toBe(3000);
  expect(environment.SCRIPT_KIT_OWNED_EVALUATION_POLICY_SHA256).toBe(lower.policySha256);
  for (const value of [0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, OWNED_EVALUATION_LIMITS.maxLifetimeMs + 1, null, "3000"])
    expect(() => issueOwnedEvaluationPermit(artifact, claim, [], { maxLifetimeMs: value as number })).toThrow("positive safe integer");
  expect(() => issueOwnedEvaluationPermit(artifact, claim, [], { maxWindows: 99 } as never)).toThrow("positive safe integer");
});
test("native glass profile is explicit, sealed and limited to its owned child", () => {
  const { artifact, claim } = fixture();
  const directory = createOwnedStagingDirectory(claim);
  const defaults = consumeOwnedEvaluationPermit(issueOwnedEvaluationPermit(artifact, claim, []));
  const explicitDefault = consumeOwnedEvaluationPermit(issueOwnedEvaluationPermit(artifact, claim, [], { nativeGlass: "platform-default" }));
  const disabled = consumeOwnedEvaluationPermit(issueOwnedEvaluationPermit(artifact, claim, [], { nativeGlass: "disabled" }));
  for (const facts of [defaults, explicitDefault]) {
    expect(facts.nativeGlass).toBe("platform-default");
    expect(Object.isFrozen(facts)).toBe(true);
    expect(ownedEvaluationEnvironment(facts, directory).SCRIPT_KIT_DEBUG_NO_GLASS).toBeUndefined();
  }
  expect(disabled.nativeGlass).toBe("disabled");
  expect(Object.isFrozen(disabled)).toBe(true);
  expect(ownedEvaluationEnvironment(disabled, directory).SCRIPT_KIT_DEBUG_NO_GLASS).toBe("1");
  for (const nativeGlass of [null, true, false, 0, "enabled", {}, []])
    expect(() => issueOwnedEvaluationPermit(artifact, claim, [], { nativeGlass } as never)).toThrow("nativeGlass must be platform-default or disabled");
});
test("ordinary artifacts cannot obtain evaluator authority", () => {
  const { artifact, claim } = fixture([]);
  expect(() => issueOwnedEvaluationPermit(artifact, claim, [])).toThrow("verified evaluator application required");
});
test("owned child environment never inherits credentials, user paths, live opt-ins or CI", () => {
  const { artifact, claim } = fixture();
  const keys = ["OPENAI_API_KEY", "SCRIPT_KIT_ALLOW_LIVE_AI", "CI", "SCRIPT_KIT_DEBUG_NO_GLASS"];
  const old = keys.map(key => process.env[key]);
  try {
    process.env.OPENAI_API_KEY = "sk-parent-secret"; process.env.SCRIPT_KIT_ALLOW_LIVE_AI = "1"; process.env.CI = "true";
    process.env.SCRIPT_KIT_DEBUG_NO_GLASS = "1";
    const facts = consumeOwnedEvaluationPermit(issueOwnedEvaluationPermit(artifact, claim, []));
    const directory = createOwnedStagingDirectory(claim);
    const environment = ownedEvaluationEnvironment(facts, directory);
    expect(environment.OPENAI_API_KEY).toBeUndefined(); expect(environment.CI).toBeUndefined();
    expect(environment.SCRIPT_KIT_ALLOW_LIVE_AI).toBe("0");
    expect(environment.SCRIPT_KIT_DEBUG_NO_GLASS).toBeUndefined();
    expect(environment.PATH).toBe("/usr/bin:/bin:/usr/sbin:/sbin");
    expect(environment.HOME).toBe(join(directory, "home"));
    expect(environment.CODEX_HOME).toBe(join(directory, "home/.codex"));
    expect(environment.SCRIPT_KIT_OWNED_EVALUATION_NONCE).toBe(facts.launchNonce);
    expect(() => ownedEvaluationEnvironment(facts, join(claim.root, "../escape"))).toThrow("within the bound output claim");
  } finally { keys.forEach((key, index) => old[index] === undefined ? delete process.env[key] : process.env[key] = old[index]); }
});

const identity: OwnedRuntimeIdentity = { pid: 22, processStartTime: "fixture-start", processInstanceId: "process", sessionGeneration: "session", binarySha256: "a".repeat(64), manifestSha256: "b".repeat(64) };
const initial: AutomationTargetSnapshot = { windowId: "main", windowGeneration: 2, appViewVariant: "ScriptList", targetGeneration: 1, surfaceGeneration: 1, dataGeneration: 1, presentationRevision: 1, themeRevision: 1, frameGeneration: 0 };
const capturePng = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a0l8AAAAASUVORK5CYII=";
function clientFixture(captureBytes: Json = { pngBase64: capturePng }, alterAtomicResult: (result: Json) => void = () => {}, alterStateResult: (state: Json, command: Json) => void = () => {}) {
  let current = { ...initial }; const requests: Json[] = [];
  const snapshot = (target: Json, includeImage: boolean) => ({
    status: "captured", source: "gpuiRenderReadback", scope: "liveAutomationWindowRenderReadback", correlationId: "request",
    frameIdentity: { ...identity, requestedTarget: target, target: { ...current } },
    capture: { width: 1, height: 1, hiDpi: true, ...(includeImage ? captureBytes : {}) },
  });
  const driver = {
    qualification: { identity }, alive: true, finalization: emptyOwnedCleanup(),
    async request(command: Json) {
      requests.push(command);
      if (command.type === "design" && command.command.operation === "captureFrame") {
        const target = command.command.target;
        if (target.id !== current.windowId) throw new DriverCommandRefused("target_not_mounted", "request");
        if (target.generation !== current.windowGeneration) throw new DriverCommandRefused("stale_window_generation", "request");
        current = { ...current, frameGeneration: current.frameGeneration + 1 };
        const result: Json = { operation: "captureFrame", ok: true,
          frame: { ...identity, requestedTarget: target, target: { ...current } },
          snapshot: snapshot(target, command.command.includeImage),
          state: { type: "stateResult", requestId: "request:state", targetIdentity: { ...current } },
          elements: { type: "elementsResult", requestId: "request:elements", targetIdentity: { ...current }, elements: [] },
          layout: { type: "layoutInfoResult", requestId: "request:layout", targetIdentity: { ...current }, info: {} },
          phaseDurationsMs: { layoutPaint: 1, gpuReadback: 2, pngEncoding: 3 } };
        if (command.command.scheduled) result.state.frameEvidence = { traceGeneration: 1, traceOverflow: false,
          afterFrameGeneration: command.command.scheduled.afterFrameGeneration, latestFrameGeneration: current.frameGeneration,
          completedFrames: [{ traceGeneration: 1, frame: result.frame }] };
        if (command.command.frameCursor) {
          const trace = { traceGeneration: command.command.frameCursor.traceGeneration, traceOverflow: false,
            afterFrameGeneration: command.command.frameCursor.afterFrameGeneration, latestFrameGeneration: current.frameGeneration,
            completedFrames: [], historyScope: "captureBundle" };
          result.state.frameEvidence = structuredClone(trace);
          result.frameEvidence = { ...structuredClone(trace), frame: result.frame, mode: command.command.scheduled ? "scheduled" : "forced",
            notificationEpoch: 9, paintBindings: [{ kind: "current-frame", id: "unchanged" }], pixelEvidence: [{ r: 10, g: 20, b: 30 }] };
          const count = current.frameGeneration > command.command.frameCursor.afterFrameGeneration ? 1 : 0;
          result.frameHistoryBundle = { version: 1, captureFrameCount: count, stateFrameCount: count };
        }
        alterAtomicResult(result);
        return { requestId: "request", type: "designResult", result };
      }
      if (command.type === "design" && command.command.operation === "diagnose")
        return { requestId: "request", type: "designResult", result: { ok: true, operation: "diagnose", targets: [{ ...current }] } };
      if (command.type === "design" && command.command.operation === "unmount")
        return { requestId: "request", type: "designResult", result: { ok: true, operation: "unmount", target: command.command.target, closed: true } };
      if (command.type === "design") return { requestId: "request", type: "designResult", result: { ok: true, operation: command.command.operation, fixtureId: "main.script-list", target: current } };
      if (command.type === "getState") {
        const state: Json = { type: "stateResult", targetIdentity: { ...current } };
        alterStateResult(state, command); return state;
      }
      if (command.type === "getElements") return { type: "elementsResult", targetIdentity: current,
        elements: command.includeHeaders ? [{ elementType: "panel", role: "sectionHeader", selectable: false, index: null }] : [] };
      if (command.type === "simulateGpuiEvent") {
        const before = current; current = { ...current, dataGeneration: current.dataGeneration + 1 };
        return { requestId: "request", type: "simulateGpuiEventResult", actionReceipt: { requestId: "request", operationId: "op", dispatchCompleted: true, before, after: current, effect: { kind: "stateChanged", owner: "main", revision: current.dataGeneration } } };
      }
      if (command.type === "waitFor") {
        if (Object.hasOwn(command.condition, "expected") && JSON.stringify(command.condition.expected) !== JSON.stringify(current))
          throw new DriverCommandRefused("stale_target_identity", "request");
        current = { ...current, frameGeneration: current.frameGeneration + 1 };
        return { type: "waitForResult", success: true, frameIdentity: { ...identity, requestedTarget: command.target, target: current } };
      }
      if (command.type === "captureRenderWindow") {
        const expected = command.request.expected;
        if (Object.entries(expected).some(([key, value]) => key !== "frameGeneration" && current[key as keyof AutomationTargetSnapshot] !== value))
          throw new DriverCommandRefused("stale_target_identity", "request");
        if (expected.frameGeneration !== current.frameGeneration) throw new DriverCommandRefused("stale_frame_identity", "request");
        return { type: "captureRenderWindowResult", snapshot: snapshot(command.request.target, command.request.includeImage) };
      }
      throw new Error("unexpected fixture command");
    },
  };
  const Constructor = OwnedEvaluationClient as unknown as new (driver: Driver) => OwnedEvaluationClient;
  return { client: new Constructor(driver as unknown as Driver), requests, changeState: () => { current = { ...current, dataGeneration: current.dataGeneration + 1 }; } };
}

function frameAcknowledgementClientFixture(alter: (reply: Json) => void = () => {}) {
  const fixture = clientFixture();
  const request = fixture.client.driver.request.bind(fixture.client.driver);
  fixture.client.driver.request = async (command, options) => {
    if (command.type !== "design" || command.command?.operation !== "acknowledgeFrames") return request(command, options);
    fixture.requests.push(command);
    const result = { operation: "acknowledgeFrames", ok: true, target: structuredClone(command.command.target),
      expected: structuredClone(command.command.expected), acknowledgedCursor: structuredClone(command.command.cursor),
      retiredFrames: 4, retainedFrames: 1, retainedTraceBytes: 32768 };
    alter(result);
    return { requestId: "request", type: "designResult", result };
  };
  return fixture;
}

test("frame acknowledgement sends one explicit mutation without capture or state polling", async () => {
  const fixture = frameAcknowledgementClientFixture();
  const target = await fixture.client.mount("main.script-list");
  const frame = await fixture.client.captureFrame(target, false);
  const cursor = { traceGeneration: 7, afterFrameGeneration: frame.frame.target.frameGeneration };
  const before = fixture.requests.length;
  const reply = await fixture.client.acknowledgeFrames(target, frame.frame.target, cursor);
  expect(fixture.requests.slice(before)).toEqual([{ type: "design", command: { operation: "acknowledgeFrames", target, expected: frame.frame.target, cursor } }]);
  expect(reply.acknowledgedCursor).toEqual(cursor);
  expect(reply.retainedFrames).toBe(1);
  expect((await fixture.client.inspect(target)).targetIdentity.frameGeneration).toBe(frame.frame.target.frameGeneration);
});

test("frame acknowledgement rejects malformed expectations before transport and mismatched replies", async () => {
  const fixture = frameAcknowledgementClientFixture();
  const target = await fixture.client.mount("main.script-list");
  const frame = await fixture.client.captureFrame(target, false);
  const cursor = { traceGeneration: 7, afterFrameGeneration: frame.frame.target.frameGeneration };
  const before = fixture.requests.length;
  await expect(fixture.client.acknowledgeFrames(target, frame.frame.target, { ...cursor, afterFrameGeneration: cursor.afterFrameGeneration + 1 })).rejects.toThrow("invalid_frame_acknowledgement_expectation");
  await expect(fixture.client.acknowledgeFrames(target, frame.frame.target, { ...cursor, force: true } as OwnedFrameCursor)).rejects.toThrow("frame_cursor_invalid");
  expect(fixture.requests.length).toBe(before);
  for (const alter of [
    (reply: Json) => { reply.target.generation++; },
    (reply: Json) => { reply.expected.dataGeneration++; },
    (reply: Json) => { reply.acknowledgedCursor.traceGeneration++; },
    (reply: Json) => { reply.retiredFrames = -1; },
    (reply: Json) => { reply.retainedFrames = 0; },
    (reply: Json) => { reply.retainedTraceBytes = Number.NaN; },
  ]) {
    const invalid = frameAcknowledgementClientFixture(alter);
    const mounted = await invalid.client.mount("main.script-list");
    const capture = await invalid.client.captureFrame(mounted, false);
    await expect(invalid.client.acknowledgeFrames(mounted, capture.frame.target, cursor)).rejects.toThrow("frame_acknowledgement_response_mismatch");
  }
});

function providerWaitClientFixture(alter: (reply: Json, command: Json) => void = () => {}) {
  const fixture = clientFixture();
  const request = fixture.client.driver.request.bind(fixture.client.driver);
  fixture.client.driver.request = async (command, options) => {
    if (command.type !== "waitFor" || command.condition?.type !== "searchProvider") return request(command, options);
    fixture.requests.push(command);
    const { source, query, afterRunId } = command.condition;
    const reply: Json = { type: "waitForResult", success: true, elapsed: 61,
      targetIdentity: { ...initial, frameGeneration: 1, dataGeneration: 2 },
      searchProvider: { version: 1, source, query: structuredClone(query), afterRunId, status: "admitted",
        owner: { source, generation: 5, workQuery: "provider-specific-work-key", workScope: "root",
          consumer: structuredClone(query), publicationPolicy: "visible", queryBound: true, terminal: null },
        run: { id: 12, source, generation: 5, query: "different raw gate query", kind: "worker", state: "held", publicationPolicy: "visible", outcome: null },
        blockers: [], pendingDesired: false, availabilityReason: "heldCurrentRun" } };
    alter(reply, command);
    return reply;
  };
  return fixture;
}

function streamWaitClientFixture(alter: (reply: Json, command: Json) => void = () => {}) {
  const fixture = clientFixture();
  const request = fixture.client.driver.request.bind(fixture.client.driver);
  fixture.client.driver.request = async (command, options) => {
    if (command.type !== "waitFor" || command.condition?.type !== "fileSearchStream") return request(command, options);
    fixture.requests.push(command);
    const reply: Json = { type: "waitForResult", success: true, elapsed: 7,
      targetIdentity: { ...initial, frameGeneration: 1, dataGeneration: 2 },
      fileSearchStream: { generation: command.condition.generation, query: command.condition.query,
        directory: "/owned/fixture", showHidden: false, phase: "completed", loading: false,
        resultCount: 0, visibleCount: 0, failure: null } };
    alter(reply, command); return reply;
  };
  return fixture;
}

function previewWaitClientFixture(alter: (reply: Json) => void = () => {}) {
  const fixture = clientFixture(); const request = fixture.client.driver.request.bind(fixture.client.driver);
  fixture.client.driver.request = async (command, options) => {
    if (command.type !== "waitFor" || command.condition?.type !== "fileSearchPreview") return request(command, options);
    fixture.requests.push(command);
    const { generation, query, workSequence } = command.condition;
    const reply: Json = { type: "waitForResult", success: true, elapsed: 16,
      targetIdentity: { ...initial, frameGeneration: 1, dataGeneration: 2 },
      fileSearchPreview: { version: 1, generation, query, workSequence, phase: "held", path: "/owned/preview.png",
        decoded: true, contentHash: "a".repeat(64), logicalTimeMs: 0, dueAtMs: 64 } };
    alter(reply); return reply;
  };
  return fixture;
}

class EncodedClientProtocol extends ProtocolCore {
  readonly requests: Json[] = [];
  readonly qualification = { identity };
  readonly finalization = emptyOwnedCleanup();
  constructor(private readonly makeResponse: (command: Json) => Promise<Json>) { super(1000, "encoded-client-test"); }
  protected authorizeCommand(_command: Json): void {}
  protected writeCommand(command: Json): void {
    this.requests.push(command);
    void this.makeResponse(command).then(raw => {
      const response: Json = { ...raw, requestId: command.requestId, protocolVersion: PROTOCOL_VERSION };
      const decoded = Buffer.from(JSON.stringify(response));
      if (command.responseEncoding) {
        const compressed = deflateSync(decoded, { level: 1 });
        this.handleResponse({ type: "encodedResponse", version: 1, encoding: OWNED_RESPONSE_ENCODING,
          requestId: command.requestId, protocolVersion: PROTOCOL_VERSION, responseType: response.type,
          decodedBytes: decoded.length, compressedBytes: compressed.length, payload: compressed.toString("base64") });
      } else this.handleResponse(JSON.parse(decoded.toString("utf8")));
    }).catch(error => this.failAllPending(error instanceof Error ? error : new Error(String(error))));
  }
  get alive(): boolean { return true; }
  async close(): Promise<void> { this.failAllPending(new Error("test closed")); }
}

function encodedClientFixture(alterCatalog: (catalog: Json) => void = () => {}, alterCapture: (capture: Json) => void = () => {}) {
  const source = clientFixture();
  const driver = new EncodedClientProtocol(async command => {
    if (command.type === "design" && command.command.operation === "catalog") {
      const catalog: Json = { operation: "catalog", ok: true, fixtures: [], targets: [{ ...initial }],
        operations: [], safetyProbes: [], settings: {}, runtimeQualified: true, responseEncoding: { ...OWNED_RESPONSE_CODEC },
        fileSearchStreamWait: { version: 1, conditionType: "fileSearchStream", identityFields: ["generation", "query"],
          terminalPhases: ["completed", "failed", "cancelled", "unavailable"] },
        fileSearchPreviewWait: { version: 1, conditionType: "fileSearchPreview", identityFields: ["generation", "query", "workSequence"], phase: "held" } };
      alterCatalog(catalog); return { type: "designResult", result: catalog };
    }
    const response = await source.client.driver.request(command);
    if (response.result?.operation === "captureFrame") {
      response.result.snapshot.correlationId = command.requestId;
      for (const facet of ["state", "elements", "layout"]) response.result[facet].requestId = `${command.requestId}:${facet}`;
      alterCapture(response.result);
    }
    return response;
  });
  const Constructor = OwnedEvaluationClient as unknown as new (driver: Driver) => OwnedEvaluationClient;
  return { client: new Constructor(driver as unknown as Driver), driver };
}

test("exact codec discovery opts raw requests in and decoding precedes atomic frame restoration", async () => {
  const { client, driver } = encodedClientFixture(() => {}, capture => {
    capture.frameEvidence.paintBindings = [{ kind: "mainSearch", id: "main-search", metadata: { query: "résumé", selected: "same real binding" } }];
    capture.frameEvidence.searchMetadataRef = 0;
  });
  await client.discover();
  expect(driver.requests[0]!.responseEncoding).toBeUndefined();
  const target = await client.mount("main.script-list");
  const raw = await client.driver.request({ type: "getState", target });
  expect(raw.type).toBe("stateResult");
  const capture = await client.captureFrame(target, true, undefined, { traceGeneration: 1, afterFrameGeneration: 0 });
  expect(capture.snapshot.capture!.pngBase64).toBe(capturePng);
  expect(capture.snapshot.correlationId).toBe(driver.requests.at(-1)!.requestId);
  expect(capture.frameEvidence!.search).toEqual({ query: "résumé", selected: "same real binding" });
  expect(capture.frameEvidence!.search).toBe(capture.frameEvidence!.paintBindings[0].metadata);
  expect(capture.frameEvidence!.searchMetadataRef).toBeUndefined();
  expect(capture.frameEvidence!.completedFrames).toHaveLength(1);
  expect(capture.state.frameEvidence.completedFrames).toHaveLength(1);
  expect(driver.requests.slice(1).every(command => command.responseEncoding === OWNED_RESPONSE_ENCODING)).toBe(true);
  await driver.close();
});

test("codec negotiation rejects a mismatched capability before changing targets or encoding requests", async () => {
  for (const mutate of [
    (catalog: Json) => { catalog.responseEncoding.version = 2; },
    (catalog: Json) => { catalog.responseEncoding.encoding = "other"; },
    (catalog: Json) => { catalog.responseEncoding.maxDecodedBytes += 1; },
    (catalog: Json) => { catalog.responseEncoding.maxCompressedBytes += 1; },
    (catalog: Json) => { catalog.responseEncoding.delivery = "identity-fallback"; },
    (catalog: Json) => { catalog.responseEncoding.requestField = "other"; },
    (catalog: Json) => { catalog.responseEncoding.responseType = "other"; },
    (catalog: Json) => { catalog.responseEncoding.extra = true; },
    (catalog: Json) => { catalog.responseEncoding = null; },
  ]) {
    const { client, driver } = encodedClientFixture(mutate);
    await expect(client.discover()).rejects.toThrow("response_encoding_capability_mismatch");
    expect(client.targets).toEqual([]);
    await driver.request({ type: "getState" });
    expect(driver.requests.every(command => !Object.hasOwn(command, "responseEncoding"))).toBe(true);
    await driver.close();
  }
  const { client, driver } = encodedClientFixture(catalog => { delete catalog.responseEncoding; });
  await client.discover(); await driver.request({ type: "getState" });
  expect(driver.requests.every(command => !Object.hasOwn(command, "responseEncoding"))).toBe(true);
  await driver.close();
});

test("compressed captures retain qualified readback and completed-frame identity checks", async () => {
  for (const [mutate, code] of [
    [(capture: Json) => { capture.snapshot.frameIdentity.target.frameGeneration++; }, "frame_generation_mismatch"],
    [(capture: Json) => { capture.snapshot.status = "captureFailed"; }, "qualified_readback_failed"],
    [(capture: Json) => { capture.elements.targetIdentity.frameGeneration++; }, "capture_observation_identity_mismatch"],
  ] as const) {
    const { client, driver } = encodedClientFixture(() => {}, mutate);
    await client.discover(); const target = await client.mount("main.script-list");
    await expect(client.captureFrame(target, true)).rejects.toThrow(code);
    expect(client.targets).toEqual([initial]);
    expect(driver.requests.at(-1)!.responseEncoding).toBe(OWNED_RESPONSE_ENCODING);
    await driver.close();
  }
});

test("file stream wait returns every real terminal phase through one native wait", async () => {
  for (const phase of ["completed", "failed", "cancelled", "unavailable"]) {
    const { client, requests } = streamWaitClientFixture(reply => {
      reply.fileSearchStream.phase = phase;
      reply.fileSearchStream.failure = phase === "completed" ? null : "observed worker failure";
    });
    const target = await client.mount("main.script-list"); const before = requests.length;
    const condition = { generation: 7, query: "exact query" };
    const result = await client.waitForFileSearchStream(target, condition, 1000);
    expect(requests.slice(before)).toEqual([{ type: "waitFor", target, condition: { ...condition, type: "fileSearchStream" }, timeout: 1000, pollInterval: 5 }]);
    expect(result.fileSearchStream).toMatchObject({ ...condition, phase, loading: false, resultCount: 0, visibleCount: 0 });
    expect(client.targets[0]!.dataGeneration).toBe(2);
  }
});

test("file stream wait refuses malformed conditions before transport", async () => {
  const { client, requests } = streamWaitClientFixture(); const target = await client.mount("main.script-list"); const before = requests.length;
  for (const condition of [{ generation: 0, query: "x" }, { generation: -1, query: "x" }, { generation: 1.5, query: "x" },
    { generation: Number.MAX_SAFE_INTEGER + 1, query: "x" }, { generation: 1, query: null }, { generation: 1 },
    { generation: 1, query: "x", source: "directory" }]) {
    await expect(client.waitForFileSearchStream(target, condition as never)).rejects.toThrow("file_search_stream_condition_invalid");
  }
  for (const timeout of [-1, 0.5, OWNED_EVALUATION_LIMITS.maxLifetimeMs + 1])
    await expect(client.waitForFileSearchStream(target, { generation: 1, query: "x" }, timeout)).rejects.toThrow("file_search_stream_condition_invalid");
  expect(requests).toHaveLength(before); expect(client.targets).toEqual([initial]);
});

test("file stream wait rejects nonterminal malformed and stale snapshots without repairing identity", async () => {
  for (const alter of [
    (value: Json) => { value.generation++; }, (value: Json) => { value.query = "stale"; },
    (value: Json) => { value.phase = "accepted"; }, (value: Json) => { value.phase = "running"; },
    (value: Json) => { value.phase = "invented"; }, (value: Json) => { value.loading = true; },
    (value: Json) => { value.directory = 17; }, (value: Json) => { value.showHidden = null; },
    (value: Json) => { value.resultCount = -1; }, (value: Json) => { value.visibleCount = 0.5; },
    (value: Json) => { value.failure = {}; }, (value: Json) => { delete value.failure; },
    (value: Json) => { value.providerRunId = 99; },
  ]) {
    const { client, requests } = streamWaitClientFixture(reply => alter(reply.fileSearchStream));
    const target = await client.mount("main.script-list"); const before = requests.length;
    await expect(client.waitForFileSearchStream(target, { generation: 7, query: "exact" })).rejects.toThrow("file_search_stream_wait_contract_mismatch");
    expect(requests.length - before).toBe(1); expect(client.targets).toEqual([initial]);
  }
  for (const [alter, code] of [
    [(reply: Json) => { reply.targetIdentity.windowGeneration++; }, "file_search_stream_wait_identity_mismatch"],
    [(reply: Json) => { reply.targetIdentity.targetGeneration++; }, "file_search_stream_wait_identity_mismatch"],
    [(reply: Json) => { reply.targetIdentity.surfaceGeneration++; }, "file_search_stream_wait_identity_mismatch"],
    [(reply: Json) => { reply.targetIdentity.dataGeneration = 0; }, "file_search_stream_wait_identity_mismatch"],
    [(reply: Json) => { reply.targetIdentity.appViewVariant = "Other"; }, "file_search_stream_wait_identity_mismatch"],
    [(reply: Json) => { reply.elapsed = -1; }, "file_search_stream_wait_identity_mismatch"],
    [(reply: Json) => { reply.success = false; }, "wait_condition_not_observed"],
    [() => { throw new DriverCommandRefused("file_search_stream_query_stale", "request"); }, "file_search_stream_query_stale"],
  ] as const) {
    const { client, requests } = streamWaitClientFixture(alter); const target = await client.mount("main.script-list"); const before = requests.length;
    await expect(client.waitForFileSearchStream(target, { generation: 7, query: "exact" })).rejects.toThrow(code);
    expect(requests.length - before).toBe(1); expect(client.targets).toEqual([initial]);
  }
});

test("file stream discovery refuses a weakened or different terminal identity contract", async () => {
  for (const mutate of [
    (value: Json) => { value.version = 2; }, (value: Json) => { value.conditionType = "searchProvider"; },
    (value: Json) => { value.identityFields = ["query"]; }, (value: Json) => { value.terminalPhases.push("running"); },
    (value: Json) => { value.extra = true; },
  ]) {
    const { client, driver } = encodedClientFixture(catalog => mutate(catalog.fileSearchStreamWait));
    await expect(client.discover()).rejects.toThrow("file_search_stream_capability_mismatch");
    expect(client.targets).toEqual([]); await driver.close();
  }
});

test("preview wait preserves real decoder outcome and exact held work identity", async () => {
  for (const decoded of [true, false]) {
    const { client, requests } = previewWaitClientFixture(reply => {
      reply.fileSearchPreview.decoded = decoded; reply.fileSearchPreview.contentHash = decoded ? "a".repeat(64) : null;
    });
    const target = await client.mount("main.script-list"); const before = requests.length;
    const condition = { generation: 7, query: "exact query", workSequence: 3 };
    const result = await client.waitForFileSearchPreview(target, condition, 1000);
    expect(requests.slice(before)).toEqual([{ type: "waitFor", target, condition: { ...condition, type: "fileSearchPreview" }, timeout: 1000, pollInterval: 5 }]);
    expect(result.fileSearchPreview).toMatchObject({ ...condition, phase: "held", decoded, logicalTimeMs: 0, dueAtMs: 64 });
    expect(client.targets[0]!.dataGeneration).toBe(2);
  }
});

test("preview wait refuses ambiguous work before transport", async () => {
  const { client, requests } = previewWaitClientFixture(); const target = await client.mount("main.script-list"); const before = requests.length;
  for (const condition of [{ generation: 0, query: "x", workSequence: 1 }, { generation: 1, query: "x", workSequence: 0 },
    { generation: 1, query: "x", workSequence: 1.5 }, { generation: 1, query: null, workSequence: 1 },
    { generation: 1, query: "x", workSequence: 1, path: "/alternate" }])
    await expect(client.waitForFileSearchPreview(target, condition as never)).rejects.toThrow("file_search_preview_condition_invalid");
  for (const timeout of [-1, 0.5, OWNED_EVALUATION_LIMITS.maxLifetimeMs + 1])
    await expect(client.waitForFileSearchPreview(target, { generation: 1, query: "x", workSequence: 1 }, timeout)).rejects.toThrow("file_search_preview_condition_invalid");
  expect(requests).toHaveLength(before); expect(client.targets).toEqual([initial]);
});

test("preview wait rejects stale or invented decode evidence without repairing targets", async () => {
  for (const mutate of [
    (value: Json) => { value.generation++; }, (value: Json) => { value.query += "x"; }, (value: Json) => { value.workSequence++; },
    (value: Json) => { value.phase = "installed"; }, (value: Json) => { value.decoded = false; },
    (value: Json) => { value.contentHash = null; }, (value: Json) => { value.contentHash = "not-a-hash"; },
    (value: Json) => { value.path = ""; }, (value: Json) => { value.logicalTimeMs = -1; },
    (value: Json) => { value.dueAtMs = value.logicalTimeMs; }, (value: Json) => { value.extra = true; },
  ]) {
    const { client, requests } = previewWaitClientFixture(reply => mutate(reply.fileSearchPreview));
    const target = await client.mount("main.script-list"); const before = requests.length;
    await expect(client.waitForFileSearchPreview(target, { generation: 7, query: "exact", workSequence: 3 })).rejects.toThrow("file_search_preview_wait_contract_mismatch");
    expect(requests.length - before).toBe(1); expect(client.targets).toEqual([initial]);
  }
  for (const mutate of [(reply: Json) => { reply.targetIdentity.surfaceGeneration++; },
    (reply: Json) => { reply.targetIdentity.dataGeneration = 0; }, (reply: Json) => { reply.targetIdentity.appViewVariant = "Other"; }]) {
    const { client } = previewWaitClientFixture(mutate); const target = await client.mount("main.script-list");
    await expect(client.waitForFileSearchPreview(target, { generation: 7, query: "exact", workSequence: 3 })).rejects.toThrow("file_search_preview_wait_identity_mismatch");
    expect(client.targets).toEqual([initial]);
  }
});

test("preview wait discovery rejects weakened sequence or phase contracts", async () => {
  for (const mutate of [(value: Json) => { value.version = 2; }, (value: Json) => { value.identityFields = ["generation", "query"]; },
    (value: Json) => { value.phase = "pending"; }, (value: Json) => { value.extra = true; }]) {
    const { client, driver } = encodedClientFixture(catalog => mutate(catalog.fileSearchPreviewWait));
    await expect(client.discover()).rejects.toThrow("file_search_preview_capability_mismatch");
    expect(client.targets).toEqual([]); await driver.close();
  }
});

test("provider wait validates delayed admission through one existing waitFor request without inspection or capture", async () => {
  const { client, requests } = providerWaitClientFixture();
  const target = await client.mount("main.script-list"); const before = requests.length;
  const condition = { type: "searchProvider", source: "brain-semantic", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 11 };
  const result = await client.wait(target, condition, 1000);
  expect(requests.slice(before)).toEqual([{ type: "waitFor", target, condition, timeout: 1000, pollInterval: 5 }]);
  expect(result.searchProvider.run.id).toBe(12);
  expect(result.searchProvider.owner.workQuery).not.toBe(result.searchProvider.run.query);
  expect(client.targets[0]?.frameGeneration).toBe(1);
});

test("provider wait rejects malformed conditions before transport", async () => {
  const { client, requests } = providerWaitClientFixture();
  const target = await client.mount("main.script-list"); const before = requests.length;
  const condition = { type: "searchProvider", source: "files", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 0 };
  for (const invalid of [{ ...condition, source: "unknown" }, { ...condition, afterRunId: null }, { ...condition, afterRunId: -1 },
      { ...condition, afterRunId: 1.5 }, { ...condition, afterRunId: Number.MAX_SAFE_INTEGER + 1 }, { ...condition, query: null },
      { ...condition, acceptCached: null }, { ...condition, acceptCached: "true" }, { ...condition, acceptCached: 1 },
      { ...condition, acceptCached: true, afterRunId: 1 },
      { ...condition, query: { ...condition.query, revision: "19" } }, { ...condition, query: { ...condition.query, other: 1 } },
      { ...condition, query: { lifetime: 7, revision: 19 } }, { ...condition, release: true }])
    await expect(client.wait(target, invalid)).rejects.toMatchObject({ code: "search_provider_condition_invalid" });
  for (const timeout of [-1, 0.5, Number.NaN, OWNED_EVALUATION_LIMITS.maxLifetimeMs + 1])
    await expect(client.wait(target, condition, timeout)).rejects.toMatchObject({ code: "search_provider_condition_invalid" });
  expect(requests.length).toBe(before);
});

test("provider wait refuses ignored echoes ABA consumers old IDs and uncorrelated generations", async () => {
  const mutations: Array<(reply: Json) => void> = [
    reply => { delete reply.searchProvider; }, reply => { reply.searchProvider.version = 2; },
    reply => { reply.searchProvider.source = "files"; }, reply => { reply.searchProvider.query.revision += 1; },
    reply => { reply.searchProvider.afterRunId = 0; }, reply => { reply.searchProvider.owner.consumer = null; },
    reply => { reply.searchProvider.owner.consumer.scopeRevision += 1; }, reply => { reply.searchProvider.owner.generation += 1; },
    reply => { reply.searchProvider.owner.terminal = "cancelled"; }, reply => { reply.searchProvider.run.id = 11; },
    reply => { reply.searchProvider.run.kind = "sourceChange"; }, reply => { reply.searchProvider.pendingDesired = true; },
    reply => { reply.searchProvider.run.outcome = "cancelled"; }, reply => { reply.searchProvider.run.capabilityRefusal = "unavailable"; },
    reply => { reply.searchProvider.availabilityReason = "guessedIdle"; },
  ];
  for (const alter of mutations) {
    const { client, requests } = providerWaitClientFixture(alter); const target = await client.mount("main.script-list");
    const before = requests.length;
    await expect(client.wait(target, { type: "searchProvider", source: "brain-semantic", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 11 }))
      .rejects.toMatchObject({ code: "search_provider_wait_contract_mismatch" });
    expect(requests.length - before).toBe(1); expect(client.targets).toEqual([initial]);
  }
});

test("provider wait accepts only correlated current terminals above the requested run bound", async () => {
  for (const terminal of ["success", "empty", "failed", "unavailable", "disconnected"] as const) {
    const { client } = providerWaitClientFixture(reply => {
      const value = reply.searchProvider; value.status = "settled"; value.owner.terminal = terminal;
      value.run.kind = "synchronousRead";
      value.run.state = terminal === "unavailable" ? "unavailable" : terminal === "failed" || terminal === "disconnected" ? "failed" : "completed";
      value.run.outcome = terminal === "failed" ? "error" : terminal; value.availabilityReason = value.run.outcome;
    });
    const target = await client.mount("main.script-list");
    const condition = { type: "searchProvider", source: "brain-lexical", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 11 };
    expect((await client.wait(target, condition)).searchProvider.owner.terminal).toBe(terminal);
    await expect(client.wait(target, { ...condition, afterRunId: 12 })).rejects.toMatchObject({ code: "search_provider_wait_contract_mismatch" });
  }
  const { client } = providerWaitClientFixture(reply => { reply.searchProvider.owner.queryBound = false; reply.searchProvider.owner.consumer = null; });
  const target = await client.mount("main.script-list");
  expect((await client.wait(target, { type: "searchProvider", source: "apps", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 11 })).searchProvider.status).toBe("admitted");
});

test("provider wait requires desired-work proof and exact held owner tickets for blockers", async () => {
  const blocked = (reply: Json) => {
    const value = reply.searchProvider;
    value.status = "blocked"; value.pendingDesired = true; value.availabilityReason = "pendingReplacement";
    value.owner.source = "files"; value.run.source = "files"; value.run.id = 4;
    value.blockers = [{ owner: value.owner, run: value.run }]; value.owner = null; value.run = null;
  };
  const condition = { type: "searchProvider", source: "directory", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 11 };
  const fixture = providerWaitClientFixture(blocked); const target = await fixture.client.mount("main.script-list");
  const result = await fixture.client.wait(target, condition);
  expect(result.searchProvider.blockers[0].run.id).toBe(4);
  expect(result.searchProvider.blockers[0].owner.consumer).toEqual(condition.query);
  for (const alter of [
    (value: Json) => { value.pendingDesired = false; }, (value: Json) => { value.blockers[0].run.generation += 1; },
    (value: Json) => { value.blockers[0].run.state = "released"; }, (value: Json) => { value.blockers[0].owner.terminal = "staleDiscarded"; },
    (value: Json) => { value.blockers[0].run.outcome = "stale-discarded"; },
    (value: Json) => { value.blockers[0].owner.source = "spine"; value.blockers[0].run.source = "spine"; },
    (value: Json) => { value.blockers.push(value.blockers[0]); },
  ]) {
    const { client } = providerWaitClientFixture(reply => { blocked(reply); alter(reply.searchProvider); });
    const target = await client.mount("main.script-list");
    await expect(client.wait(target, condition)).rejects.toMatchObject({ code: "search_provider_wait_contract_mismatch" });
  }
});

test("provider wait preserves native refusals and never refreshes a mismatched target or timeout", async () => {
  const condition = { type: "searchProvider", source: "files", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 0 };
  for (const [alter, code] of [
    [(reply: Json) => { reply.targetIdentity.windowGeneration += 1; }, "search_provider_wait_identity_mismatch"],
    [(reply: Json) => { reply.targetIdentity.surfaceGeneration += 1; }, "search_provider_wait_identity_mismatch"],
    [(reply: Json) => { reply.success = false; }, "wait_condition_not_observed"],
    [() => { throw new DriverCommandRefused("search_provider_query_stale", "request"); }, "search_provider_query_stale"],
  ] as const) {
    const { client, requests } = providerWaitClientFixture(alter); const target = await client.mount("main.script-list"); const before = requests.length;
    await expect(client.wait(target, condition)).rejects.toMatchObject({ code });
    expect(requests.length - before).toBe(1); expect(client.targets).toEqual([initial]);
  }
});

test("provider wait accepts explicit current cached readiness without inventing worker provenance", async () => {
  const identities: Record<typeof OWNED_SEARCH_CACHE_SOURCES[number], string> = {
    tabs: "browser-tabs-snapshot", files: "exact-file-work-key", directory: "exact-file-work-key",
    history: "browser-history-snapshot", notes: "notes-query-cache", todos: "todos-snapshot", clipboard: "clipboard-history-snapshot",
    dictation: "dictation-history-snapshot", conversations: "conversation-history-snapshot", windows: "window-snapshot",
  };
  for (const source of OWNED_SEARCH_CACHE_SOURCES) {
    const { client, requests } = providerWaitClientFixture(reply => {
      const value = reply.searchProvider;
      value.status = "cached"; value.owner = null; value.run = null;
      value.pendingDesired = source === "tabs"; value.availabilityReason = "sourceCacheReuse";
      value.cache = { source, query: structuredClone(value.query), cacheIdentity: identities[source],
        cacheStateRevision: source === "files" || source === "directory" ? null : source === "notes" ? 0 : 17, rowCount: 0 };
    });
    const target = await client.mount("main.script-list"); const before = requests.length;
    const condition = { type: "searchProvider", source, query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 0, acceptCached: true };
    const result = await client.wait(target, condition);
    expect(requests.slice(before)).toEqual([{ type: "waitFor", target, condition, timeout: 5000, pollInterval: 5 }]);
    expect(result.searchProvider).toMatchObject({ status: "cached", owner: null, run: null,
      cache: { source, query: condition.query, rowCount: 0 } });
    await expect(client.wait(target, { ...condition, acceptCached: false })).rejects.toMatchObject({ code: "search_provider_wait_contract_mismatch" });
    const { acceptCached: _, ...ordinary } = condition;
    await expect(client.wait(target, ordinary)).rejects.toMatchObject({ code: "search_provider_wait_contract_mismatch" });
  }
});

test("source cache readiness requires real numeric revisions except file work-key caches", async () => {
  for (const source of OWNED_SEARCH_CACHE_SOURCES) {
    for (const revision of source === "files" || source === "directory" ? [0, 17] : [null, -1, 0.5, "17"]) {
      const { client } = providerWaitClientFixture(reply => {
        const value = reply.searchProvider;
        value.status = "cached"; value.owner = null; value.run = null; value.availabilityReason = "sourceCacheReuse";
        value.cache = { source, query: structuredClone(value.query), cacheIdentity: "source-owned-cache", cacheStateRevision: revision, rowCount: 0 };
      });
      const target = await client.mount("main.script-list");
      await expect(client.wait(target, { type: "searchProvider", source, query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 0, acceptCached: true }))
        .rejects.toThrow("search_provider_wait_contract_mismatch");
      expect(client.targets).toEqual([initial]);
    }
  }
});

test("provider wait refuses cached evidence with stale query fabricated origin or malformed source identity", async () => {
  const mutations: Array<(value: Json) => void> = [
    value => { delete value.cache; }, value => { value.cache.query.revision += 1; },
    value => { value.cache.source = "files"; }, value => { value.cache.cacheIdentity = ""; },
    value => { value.cache.cacheStateRevision = null; }, value => { value.cache.cacheStateRevision = -1; },
    value => { value.cache.rowCount = -1; }, value => { value.cache.rowCount = 1.5; },
    value => { value.cache.generation = 5; }, value => { value.cache.query.text = "same query"; },
    value => { value.run = { id: 12, source: "tabs", generation: 5, query: "old", kind: "worker", state: "completed", publicationPolicy: "cache-only", outcome: "success" }; },
    value => { value.owner = { source: "tabs", generation: 5, workQuery: "old", workScope: "root", consumer: null, publicationPolicy: "cache-only", queryBound: true, terminal: "success" }; },
    value => { value.blockers = [{}]; }, value => { value.availabilityReason = "missingCache"; },
    value => { value.status = "inactive"; }, value => { value.status = "settled"; },
  ];
  for (const alter of mutations) {
    const { client } = providerWaitClientFixture(reply => {
      const value = reply.searchProvider;
      value.status = "cached"; value.owner = null; value.run = null; value.availabilityReason = "sourceCacheReuse";
      value.cache = { source: "tabs", query: structuredClone(value.query), cacheIdentity: "browser-tabs-snapshot", cacheStateRevision: 17, rowCount: 2 };
      alter(value);
    });
    const target = await client.mount("main.script-list");
    await expect(client.wait(target, { type: "searchProvider", source: "tabs", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 0, acceptCached: true }))
      .rejects.toMatchObject({ code: "search_provider_wait_contract_mismatch" });
    expect(client.targets).toEqual([initial]);
  }
});

test("provider wait cache opt-in does not revive a detached worker consumer", async () => {
  const { client } = providerWaitClientFixture(reply => { reply.searchProvider.owner.consumer = null; });
  const target = await client.mount("main.script-list");
  await expect(client.wait(target, { type: "searchProvider", source: "files", query: { lifetime: 7, revision: 19, scopeRevision: 2 }, afterRunId: 0, acceptCached: true }))
    .rejects.toMatchObject({ code: "search_provider_wait_contract_mismatch" });
});

function inspectionPage(state: Json, command: Json): void {
  state.targetIdentity = { ...state.targetIdentity, frameGeneration: 4 };
  state.frameEvidence = { traceGeneration: 7, traceOverflow: false, afterFrameGeneration: command.frameCursor?.afterFrameGeneration ?? null,
    latestFrameGeneration: 4, completedFrames: [1, 3, 4].filter(generation => generation > (command.frameCursor?.afterFrameGeneration ?? 0)).map(generation => ({
      traceGeneration: 7, frame: { ...identity, requestedTarget: command.target, target: { ...state.targetIdentity, frameGeneration: generation } },
    })) };
}
test("owned frame cursors omit full reads, return ordered unseen stamps, and allow native negative-readback gaps", async () => {
  const { client, requests } = clientFixture(undefined, undefined, inspectionPage);
  const target = await client.mount("main.script-list");
  const full = await client.inspect(target);
  expect(requests.at(-1)).toEqual({ type: "getState", target });
  expect(full.frameEvidence.afterFrameGeneration).toBeNull();
  expect(full.frameEvidence.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([1, 3, 4]);
  const cursor = Object.freeze({ traceGeneration: 7, afterFrameGeneration: 1 });
  const page = await client.inspect(target, cursor);
  expect(requests.at(-1)).toEqual({ type: "getState", target, frameCursor: cursor });
  expect(page.frameEvidence.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([3, 4]);
  const empty = await client.inspect(target, { traceGeneration: 7, afterFrameGeneration: 4 });
  expect(empty.frameEvidence.completedFrames).toEqual([]); expect(requests).toHaveLength(4);
  const isolated = clientFixture(undefined, undefined, (state, command) => {
    inspectionPage(state, command); state.frameEvidence.completedFrames = []; state.frameEvidence.latestFrameGeneration = 5;
    state.targetIdentity.frameGeneration = 5;
  });
  const isolatedTarget = await isolated.client.mount("main.script-list");
  await expect(isolated.client.inspect(isolatedTarget, { traceGeneration: 7, afterFrameGeneration: 4 })).resolves.toMatchObject({
    frameEvidence: { latestFrameGeneration: 5, completedFrames: [] } });
});
test("malformed owned frame cursors fail before transport or target refresh", async () => {
  const { client, requests } = clientFixture(); const target = await client.mount("main.script-list");
  const before = structuredClone(client.targets);
  for (const cursor of [null, [], {}, 1, { traceGeneration: 1 }, { traceGeneration: 0, afterFrameGeneration: 0 },
      { traceGeneration: 1.5, afterFrameGeneration: 0 }, { traceGeneration: 1, afterFrameGeneration: -1 },
      { traceGeneration: 1, afterFrameGeneration: 0.5 }, { traceGeneration: 1, afterFrameGeneration: Number.MAX_SAFE_INTEGER + 1 },
      { traceGeneration: 1, afterFrameGeneration: 0, extra: true }]) {
    await expect(client.inspect(target, cursor as OwnedFrameCursor)).rejects.toThrow("frame_cursor_invalid");
    expect(requests).toHaveLength(1); expect(client.targets).toEqual(before);
  }
});
test("ignored, foreign and malformed frame pages fail without healing cached target authority", async () => {
  for (const change of [
    (state: Json) => { delete state.frameEvidence; },
    (state: Json) => { delete state.frameEvidence.afterFrameGeneration; },
    (state: Json) => { state.frameEvidence.afterFrameGeneration = null; },
    (state: Json) => { state.frameEvidence.traceGeneration++; },
    (state: Json) => { state.frameEvidence.latestFrameGeneration = 0; },
    (state: Json) => { state.frameEvidence.latestFrameGeneration = 4.5; },
    (state: Json) => { state.frameEvidence.traceOverflow = true; },
    (state: Json) => { state.frameEvidence.completedFrames.reverse(); },
    (state: Json) => { state.frameEvidence.completedFrames.push(state.frameEvidence.completedFrames[0]); },
    (state: Json) => { state.frameEvidence.completedFrames[0].frame.target.frameGeneration = 1; },
    (state: Json) => { state.frameEvidence.completedFrames[0].traceGeneration++; },
    (state: Json) => { state.frameEvidence.completedFrames[0].frame.processInstanceId = "other"; },
  ]) {
    const { client, requests } = clientFixture(undefined, undefined, (state, command) => { inspectionPage(state, command); change(state); });
    const target = await client.mount("main.script-list"); const before = structuredClone(client.targets);
    await expect(client.inspect(target, { traceGeneration: 7, afterFrameGeneration: 1 })).rejects.toThrow("frame_cursor_response_mismatch");
    expect(requests).toHaveLength(2); expect(client.targets).toEqual(before);
  }
});
test("native stale retired and future cursor refusals propagate with no full-read retry", async () => {
  for (const code of ["frame_cursor_stale", "frame_cursor_retired", "frame_cursor_future"]) {
    const { client, requests } = clientFixture(undefined, undefined, () => { throw new DriverCommandRefused(code, "request"); });
    const target = await client.mount("main.script-list"); const before = structuredClone(client.targets);
    await expect(client.inspect(target, { traceGeneration: 7, afterFrameGeneration: 1 })).rejects.toMatchObject({ code });
    expect(requests).toHaveLength(2); expect(client.targets).toEqual(before);
  }
});

test("explicit unmount sends the supplied observation unchanged without an inspection", async () => {
  const { client, requests } = clientFixture(); const target = await client.mount("main.script-list");
  const expected = Object.freeze({ ...initial, dataGeneration: 9, frameGeneration: 4 });
  await client.unmount(target, expected);
  expect(requests.slice(1)).toEqual([{ type: "design", command: { operation: "unmount", target, expected } }]);
  expect(client.targets).toEqual([]);
});
test("explicit unmount propagates stale refusal without refresh or retirement of the cached target", async () => {
  const { client } = clientFixture(); const target = await client.mount("main.script-list");
  const before = structuredClone(client.targets); const sent: Json[] = [];
  client.driver.request = async command => { sent.push(command); throw new DriverCommandRefused("stale_target_identity", "request"); };
  const expected = { ...initial, dataGeneration: 9 };
  await expect(client.unmount(target, expected)).rejects.toMatchObject({ code: "stale_target_identity" });
  expect(sent).toEqual([{ type: "design", command: { operation: "unmount", target, expected } }]);
  expect(client.targets).toEqual(before);
});
test("default unmount still inspects current authority while foreign explicit expectations stop before transport", async () => {
  const { client, requests, changeState } = clientFixture(); const target = await client.mount("main.script-list");
  await expect(client.unmount(target, { ...initial, windowGeneration: initial.windowGeneration + 1 })).rejects.toThrow("invalid_unmount_expectation");
  expect(requests).toHaveLength(1); changeState();
  await client.unmount(target);
  expect(requests[1]).toEqual({ type: "getState", target });
  expect(requests[2]).toEqual({ type: "design", command: { operation: "unmount", target, expected: { ...initial, dataGeneration: 2 } } });
  expect(client.targets).toEqual([]);
});
test("owned semantic queries explicitly request chrome without forcing a frame", async () => {
  const { client, requests } = clientFixture();
  const target = await client.mount("main.script-list"); const start = requests.length;
  const response = await client.query(target, "elements", { includeHeaders: true });
  expect(requests.slice(start)).toEqual([{ type: "getElements", target, includeHeaders: true }]);
  expect(response.elements).toEqual([{ elementType: "panel", role: "sectionHeader", selectable: false, index: null }]);
});
test("action completion records observed after identity, and frames refresh asynchronous state", async () => {
  const { client, requests, changeState } = clientFixture();
  const target = await client.mount("main.script-list");
  await client.act(target, { type: "key", key: "x", text: "x" });
  expect(client.targets[0]?.dataGeneration).toBe(2);
  changeState();
  const requestsBeforeFrame = requests.length;
  const frame = await client.frame(target);
  expect(frame.target.dataGeneration).toBe(3);
  expect(frame.target.frameGeneration).toBe(1);
  expect(requests.length - requestsBeforeFrame).toBe(1);
  expect(requests.at(-1)?.type).toBe("waitFor");
  expect(Object.hasOwn(requests.at(-1)!.condition, "expected")).toBe(false);
  await expect(client.wait(target, { type: "completedFrame", afterFrameGeneration: 0, expected: initial })).rejects.toMatchObject({ code: "stale_target_identity" });
});
test("authoritative target snapshots retire delayed popup closures without promoting stale lifetimes", async () => {
  for (const method of ["discover", "diagnose"] as const) {
    const { client, requests } = clientFixture();
    const main = await client.mount("main.script-list");
    const popup = { ...initial, windowId: "agent_chat-history-popup", windowGeneration: 3, appViewVariant: "promptPopup" };
    const popupTarget = { type: "instance" as const, id: popup.windowId, generation: popup.windowGeneration };
    let targets = [initial, popup];
    const request = client.driver.request.bind(client.driver);
    client.driver.request = async command => {
      if (command.type === "design" && ["catalog", "diagnose"].includes(command.command.operation))
        return { requestId: "request", type: "designResult", result: { operation: command.command.operation, ok: true, targets, fixtures: [] } };
      if (command.type === "getState" && command.target.id === popupTarget.id)
        return { type: "stateResult", targetIdentity: popup };
      if (command.type === "simulateGpuiEvent" && command.target.id === popupTarget.id)
        return { requestId: "request", type: "simulateGpuiEventResult", actionReceipt: {
          requestId: "request", operationId: "popup-key", dispatchCompleted: true, before: popup, after: popup,
          effect: { kind: "noOp", reason: "no_owner_change_observed" } } };
      return request(command);
    };
    await client[method]();
    expect(client.targets).toEqual([initial, popup]);
    await client.act(popupTarget, { type: "key", key: "escape" });
    expect(client.targets).toEqual([initial, popup]);
    targets = [initial];
    await client[method]();
    expect(client.targets).toEqual([initial]);
    const requestCount = requests.length;
    await expect(client.frame(popupTarget)).rejects.toThrow("target_not_mounted");
    expect(requests.length).toBe(requestCount);
    const reopened = { ...initial, windowGeneration: 4 };
    targets = [reopened];
    await client[method]();
    expect(client.targets).toEqual([reopened]);
    await expect(client.captureFrame(main, false)).rejects.toThrow("target_not_mounted");
    expect(client.targets).toEqual([reopened]);
    expect(requests.length).toBe(requestCount);
    targets = [];
    await client[method]();
    expect(client.targets).toEqual([]);
  }
});
test("invalid authoritative target snapshots cannot partially retire the cached lifetimes", async () => {
  const invalid = [null, {}, [null], [initial, initial], [initial, { ...initial, windowGeneration: 3 }],
    [initial, { ...initial, windowId: "" }], [initial, { ...initial, windowId: 7 }],
    [initial, { ...initial, windowId: "popup", windowGeneration: 0 }],
    [initial, { ...initial, windowId: "popup", dataGeneration: -1 }],
    [initial, { ...initial, windowId: "popup", appViewVariant: "" }]];
  for (const method of ["discover", "diagnose"] as const) {
    for (const targets of invalid) {
      const { client } = clientFixture();
      await client.mount("main.script-list");
      client.driver.request = async command => ({ requestId: "request", type: "designResult",
        result: { operation: command.command.operation, ok: true, targets, fixtures: [] } });
      await expect(client[method]()).rejects.toThrow("invalid_target_catalog");
      expect(client.targets).toEqual([initial]);
    }
  }
});
test("validated nondispatched refusals preserve their typed code without accepting incomplete success", async () => {
  for (const fault of ["refused", "uncorrelated", "foreign", "incomplete-success"] as const) {
    const { client } = clientFixture();
    const target = await client.mount("main.script-list");
    const request = client.driver.request.bind(client.driver);
    client.driver.request = async (command, options) => {
      const response = await request(command, options);
      if (command.type === "simulateGpuiEvent") {
        const receipt = response.actionReceipt;
        receipt.dispatchCompleted = false;
        receipt.after = receipt.before;
        if (fault !== "incomplete-success") receipt.effect = { kind: "refused", code: "unsupported_command" };
        if (fault === "uncorrelated") receipt.requestId = "foreign-request";
        if (fault === "foreign") receipt.before = { ...receipt.before, windowGeneration: target.generation + 1 };
      }
      return response;
    };
    const action = client.act(target, { type: "key", key: "enter" });
    if (fault === "refused") {
      await expect(action).rejects.toBeInstanceOf(DriverCommandRefused);
      await expect(action).rejects.toMatchObject({ code: "unsupported_command", requestId: "request" });
    } else {
      await expect(action).rejects.toThrow(fault === "foreign" ? "action_identity_mismatch" : "completed_action_receipt_required");
    }
    expect(client.targets[0]).toEqual(initial);
  }
});
test("capture cannot use an uncompleted frame", async () => {
  const { client } = clientFixture(); const target = await client.mount("main.script-list");
  await expect(client.capture(target, true)).rejects.toThrow("completed_frame_required");
});
test("capture accepts canonical PNG bytes and rejects obsolete payload aliases", async () => {
  const { client } = clientFixture();
  const target = await client.mount("main.script-list");
  await client.frame(target);
  expect((await client.capture(target)).snapshot.capture.pngBase64).toBe(capturePng);
  for (const payload of [{ data: capturePng }, { image: { data: capturePng } }, {}]) {
    const { client: invalid } = clientFixture(payload);
    const mounted = await invalid.mount("main.script-list");
    await invalid.frame(mounted);
    await expect(invalid.capture(mounted)).rejects.toThrow("invalid_capture_bytes");
  }
});
test("owned capture preserves native resolution and rejects resampled responses", async () => {
  const { client, requests } = clientFixture();
  const target = await client.mount("main.script-list");
  await client.frame(target);
  expect((await client.capture(target)).snapshot.capture.hiDpi).toBe(true);
  expect(requests.at(-1)?.request.hiDpi).toBe(true);
  for (const hiDpi of [false, undefined]) {
    const { client: invalid } = clientFixture({ pngBase64: capturePng, hiDpi });
    const mounted = await invalid.mount("main.script-list");
    await invalid.frame(mounted);
    await expect(invalid.capture(mounted)).rejects.toThrow("native_resolution_readback_required");
  }
});
test("atomic capture needs no prior frame and observes current state in one request", async () => {
  const { client, requests, changeState } = clientFixture();
  const target = await client.mount("main.script-list");
  changeState();
  const response = await client.captureFrame(target, true);
  expect(requests).toHaveLength(2);
  expect(requests[1]).toEqual({ type: "design", command: { operation: "captureFrame", target, includeImage: true } });
  expect(response.frame.target).toEqual({ ...initial, dataGeneration: 2, frameGeneration: 1 });
  expect(response.snapshot.frameIdentity).toEqual(response.frame);
  for (const facet of ["state", "elements", "layout"] as const) expect(response[facet].targetIdentity).toEqual(response.frame.target);
  expect(response.snapshot.capture?.pngBase64).toBe(capturePng);
  expect(client.targets[0]).toEqual(response.frame.target);
  expect(client.lastFramePhaseDurationsMs).toEqual(response.phaseDurationsMs);
  changeState();
  const latest = await client.query(target, "frame");
  expect(latest.frame.target).toEqual({ ...initial, dataGeneration: 3, frameGeneration: 2 });
  expect(latest.snapshot.capture.pngBase64).toBeUndefined();
  expect(response.frame.target.dataGeneration).toBe(2);
  expect(requests).toHaveLength(3);
});
test("atomic capture refuses unmounted and stale instances without promoting targets", async () => {
  const { client, requests } = clientFixture(); const target = await client.mount("main.script-list");
  await expect(client.captureFrame({ ...target, id: "foreign" }, false)).rejects.toThrow("target_not_mounted");
  await expect(client.captureFrame({ ...target, generation: target.generation + 1 }, false)).rejects.toThrow("target_not_mounted");
  expect(requests).toHaveLength(1);
  await expect(client.design({ operation: "captureFrame", target: { ...target, generation: target.generation + 1 }, includeImage: false }))
    .rejects.toMatchObject({ code: "stale_window_generation" });
});
test("atomic capture rejects malformed, foreign, stale and uncorrelated observations", async () => {
  const corruptions: Array<[string, (result: Json) => void]> = [
    ["invalid_completed_frame", result => { delete result.frame; }],
    ["invalid_completed_frame", result => { result.frame.pid++; }],
    ["invalid_completed_frame", result => { result.frame.target.windowGeneration++; }],
    ["invalid_completed_frame", result => { result.frame.target.frameGeneration = 0; }],
    ["qualified_readback_failed", result => { result.snapshot.status = "captureFailed"; }],
    ["frame_pid_mismatch", result => { result.snapshot.frameIdentity.pid++; }],
    ["frame_dataGeneration_stale", result => { result.snapshot.frameIdentity.target.dataGeneration++; }],
    ["frame_generation_mismatch", result => { result.snapshot.frameIdentity.target.frameGeneration++; }],
    ["frame_surface_mismatch", result => { result.snapshot.frameIdentity.target.appViewVariant = "NotesApp"; }],
    ["frame_native_window_mismatch", result => { result.snapshot.frameIdentity.nativeWindowId = 9; }],
    ["invalid_capture_dimensions", result => { result.snapshot.capture.width = 0; }],
    ["invalid_capture_dimensions", result => { result.snapshot.capture.width = OWNED_EVALUATION_LIMITS.maxImagePixels + 1; }],
    ["native_resolution_readback_required", result => { result.snapshot.capture.hiDpi = false; }],
    ["invalid_capture_bytes", result => { delete result.snapshot.capture.pngBase64; }],
    ["invalid_capture_bytes", result => { result.snapshot.capture.pngBase64 = Buffer.alloc(OWNED_EVALUATION_LIMITS.maxPngBytes + 1).toString("base64"); }],
    ["capture_response_correlation_mismatch", result => { result.snapshot.correlationId = "foreign"; }],
    ["invalid_capture_frame_timing", result => { result.phaseDurationsMs.gpuReadback = -1; }],
    ...(["state", "elements", "layout"] as const).flatMap(facet => [
      ["capture_observation_identity_mismatch", (result: Json) => { result[facet].targetIdentity.frameGeneration++; }],
      ["capture_response_correlation_mismatch", (result: Json) => { result[facet].requestId = "request:other"; }],
    ] as Array<[string, (result: Json) => void]>),
  ];
  for (const [code, corrupt] of corruptions) {
    const { client } = clientFixture({ pngBase64: capturePng }, corrupt); const target = await client.mount("main.script-list");
    await expect(client.captureFrame(target, true)).rejects.toThrow(code);
    expect(client.targets[0]).toEqual(initial);
  }
});
test("atomic capture shares image and frame budgets with exact capture", async () => {
  const { client, requests } = clientFixture(); const target = await client.mount("main.script-list");
  for (let index = 0; index < OWNED_EVALUATION_LIMITS.maxRetainedImages; index++) {
    if (index % 2 === 0) await client.captureFrame(target, true); else await client.capture(target, true);
  }
  const before = requests.length;
  await expect(client.captureFrame(target, true)).rejects.toThrow("retained_image_budget_exhausted");
  await expect(client.capture(target, true)).rejects.toThrow("retained_image_budget_exhausted");
  expect(requests).toHaveLength(before);
  await client.captureFrame(target, false);
  const frames = clientFixture(); const mounted = await frames.client.mount("main.script-list");
  await frames.client.frame(mounted);
  for (let index = 1; index < OWNED_EVALUATION_LIMITS.maxFrames; index++) await frames.client.captureFrame(mounted, false);
  const frameRequests = frames.requests.length;
  await expect(frames.client.captureFrame(mounted, false)).rejects.toThrow("frame_budget_exhausted");
  await expect(frames.client.frame(mounted)).rejects.toThrow("frame_budget_exhausted");
  expect(frames.requests).toHaveLength(frameRequests);
});
test("exact capture still refuses state advanced after a completed frame", async () => {
  const { client, requests, changeState } = clientFixture(); const target = await client.mount("main.script-list");
  const prior = await client.frame(target); changeState();
  await expect(client.capture(target, false)).rejects.toMatchObject({ code: "stale_target_identity" });
  expect(requests.at(-1)?.request.expected).toEqual(prior.target);
  const current = await client.captureFrame(target, false);
  expect(current.frame.target.dataGeneration).toBe(prior.target.dataGeneration + 1);
});
test("atomic capture rejects a returned frame that did not advance the previous observation", async () => {
  const { client } = clientFixture({ pngBase64: capturePng }, result => { result.frame.target.frameGeneration = 1; });
  const target = await client.mount("main.script-list");
  const prior = await client.frame(target);
  await expect(client.captureFrame(target, false)).rejects.toThrow("invalid_completed_frame");
  expect(client.targets[0]).toEqual(prior.target);
});

function closingClientFixture(initiallyObserved = false, failObservation = false) {
  const calls: string[] = []; let observed = initiallyObserved;
  const lifecycle = { type: "designResult", protocolVersion: 2, result: { operation: "end", lifecycle: true,
    schemaVersion: 1, identity, launchNonce: "fixture-nonce", policySha256: "c".repeat(64),
    shutdownReason: initiallyObserved ? "inputEof" : "explicitEnd", ok: true, ownedWindowsClosed: true,
    remainingWindows: 0, refusedEffects: 0, native: { installed: true, openedWindows: 1, liveWindows: 0,
      automationWindows: 0, completedFrames: 1, readbackImages: 1, refusedOperations: 0 } } };
  const driver = {
    alive: true, finalization: { ...emptyOwnedCleanup(), closed: false, resourcesAcquired: true },
    get nativeLifecycle() { return observed ? lifecycle : null; },
    async request(command: Json) {
      calls.push(`request:${command.command.operation}`);
      return { type: "designResult", requestId: "end-request", result: { operation: "end", ok: true, ownedWindowsClosed: true, remainingWindows: 0 } };
    },
    async awaitNativeLifecycle() {
      calls.push("observe-native-final");
      if (failObservation) throw new Error("native_lifecycle_unobserved");
      observed = true; return lifecycle;
    },
    async close() {
      calls.push("close-driver");
      this.finalization = { ...emptyOwnedCleanup(), resourcesAcquired: true, closed: observed, ownedWindowsClosed: observed ? true : null };
    },
  };
  const Constructor = OwnedEvaluationClient as unknown as new (driver: Driver) => OwnedEvaluationClient;
  return { client: new Constructor(driver as unknown as Driver), calls };
}
test("client End waits for observed native closure and remains idempotent", async () => {
  const { client, calls } = closingClientFixture();
  const first = client.close(); expect(client.close()).toBe(first);
  expect((await first).ownedWindowsClosed).toBe(true);
  expect(calls).toEqual(["request:end", "observe-native-final", "close-driver"]);
});
test("client does not send End after an observed EOF shutdown", async () => {
  const { client, calls } = closingClientFixture(true);
  expect((await client.close()).closed).toBe(true);
  expect(calls).toEqual(["close-driver"]);
});
test("an End reply cannot substitute for unobserved native closure", async () => {
  const { client, calls } = closingClientFixture(false, true);
  await expect(client.close()).rejects.toThrow("native_lifecycle_unobserved");
  expect(client.cleanup.closed).toBe(false);
  expect(client.cleanup.ownedWindowsClosed).toBeNull();
  expect(calls).toEqual(["request:end", "observe-native-final", "close-driver"]);
});

function safetyResult(probe: NativeSafetyProbeResult["probe"]): NativeSafetyProbeResult {
  const native = { installed: true, openedWindows: 1, liveWindows: 1, completedFrames: 1, readbackImages: 1, refusedOperations: 0 };
  const window = { ownedHidden: true, active: false, focus: null, bounds: { x: 0, y: 0, width: 640, height: 480 },
    native: { nativeWindowId: 0, visible: false, key: false, miniaturized: false, appActive: false } };
  return { operation: "probeSafety", ok: true, probe, negativeOnly: true, productionEvidence: false,
    target: { type: "instance", id: initial.windowId, generation: initial.windowGeneration }, targetIdentity: initial,
    implementationGap: null, before: { native: { ...native }, window: structuredClone(window), refusedEffects: 0, completedFixtureEffects: 0 },
    after: { native: { ...native }, window: structuredClone(window), refusedEffects: 0, completedFixtureEffects: 0 },
    observation: {}, windowStateUnchanged: true, ownedCopyUnchanged: true, elapsedMs: 1 };
}
test("executed native probe labels cannot pass without their observed negative", () => {
  for (const probe of NATIVE_SAFETY_PROBES) expect(nativeSafetyProbeAssertions(safetyResult(probe)).some(assertion => !assertion.pass)).toBe(true);
});
test("precreation proof requires real refusal, no constructor and inactive native state", () => {
  const result = safetyResult("invalidShow"); result.after.native.refusedOperations = 1;
  result.observation = { result: { returnedOk: false, errorCode: "owned_hidden_show_or_focus" }, rootConstructorCalled: false, auxiliaryWindowsRemaining: 0 };
  expect(nativeSafetyProbeAssertions(result).every(assertion => assertion.pass)).toBe(true);
  for (const mutate of [
    (value: NativeSafetyProbeResult) => { value.after.native.openedWindows++; },
    (value: NativeSafetyProbeResult) => { value.observation.rootConstructorCalled = true; },
    (value: NativeSafetyProbeResult) => { value.after.window.native.visible = true; },
    (value: NativeSafetyProbeResult) => { value.implementationGap = "guard_missing"; },
  ]) { const invalid = structuredClone(result); mutate(invalid); expect(nativeSafetyProbeAssertions(invalid).some(assertion => !assertion.pass)).toBe(true); }
});
test("clipboard safety proof requires real helpers and completed terminal fallback", () => {
  for (const probe of ["clipboardRead", "clipboardWrite"] as const) {
    const result = safetyResult(probe); result.after.refusedEffects = 1;
    result.observation = { result: { returnedOk: false }, applyBack: {
      constructorCalls: 0, readRefused: true, writeRefused: true, terminalNoSelection: true,
      terminalFallbackCompletionKind: "synchronousRefusal", terminalCallbackScheduled: false,
      terminalPrimeRefused: true, terminalFallbackCompleted: true, terminalFallbackRefused: true,
      terminalFallbackCompletions: 1, probeCleared: true, terminalFixtureReleased: true,
    } };
    expect(nativeSafetyProbeAssertions(result).every(assertion => assertion.pass)).toBe(true);
    for (const field of Object.keys(result.observation.applyBack)) {
      for (const missing of [true, false]) {
        const invalid = structuredClone(result);
        if (missing) delete invalid.observation.applyBack[field];
        else invalid.observation.applyBack[field] = field === "constructorCalls" ? 1 : field === "terminalFallbackCompletions" ? 2 : field === "terminalCallbackScheduled" ? true : false;
        expect(nativeSafetyProbeAssertions(invalid).some(assertion => !assertion.pass), `${probe}:${field}:${missing}`).toBe(true);
      }
    }
  }
});
test("deferred refusal proof requires terminal flags and bounded integer elapsed time", () => {
  const result = safetyResult("deferredDispatch");
  const refusal = { success: false, dispatchCompleted: false, dispatchScheduled: false, wasDeferred: true };
  result.observation = {
    cancelTerminalReplies: 1, cancelled: { ...refusal, errorCode: "dispatch_cancelled" },
    deadlineTerminalReplies: 1, expired: { ...refusal, errorCode: "dispatch_deadline_exceeded" },
    staleTerminalReplies: 1, staleExpectation: { ...refusal, errorCode: "stale_target_identity" },
    duplicateTerminalReplies: 0, ownerUnchanged: true, identityUnchanged: true,
    batchTerminalReplies: 1, batchReplayReplies: 0,
    batchWait: { success: false, results: [{ error: { code: "wait_condition_timeout" } }], totalElapsed: 40 },
  };
  expect(nativeSafetyProbeAssertions(result).every(assertion => assertion.pass)).toBe(true);
  for (const terminal of ["cancelled", "expired", "staleExpectation"]) {
    for (const field of ["success", "dispatchScheduled", "wasDeferred"]) {
      for (const missing of [false, true]) {
        const invalid = structuredClone(result);
        if (missing) delete invalid.observation[terminal][field];
        else invalid.observation[terminal][field] = field !== "wasDeferred";
        expect(nativeSafetyProbeAssertions(invalid).some(assertion => !assertion.pass), `${terminal}:${field}:${missing ? "missing" : "opposite"}`).toBe(true);
      }
    }
  }
  for (const elapsed of [-1, 0.5, "0", null, undefined, 1000, NaN, Infinity]) {
    const invalid = structuredClone(result); invalid.observation.batchWait.totalElapsed = elapsed;
    expect(nativeSafetyProbeAssertions(invalid).find(assertion => assertion.id.endsWith("ordinary_batch_deadline_bounded"))?.pass).toBe(false);
  }
  for (const elapsed of [0, 999]) {
    const valid = structuredClone(result); valid.observation.batchWait.totalElapsed = elapsed;
    expect(nativeSafetyProbeAssertions(valid).every(assertion => assertion.pass)).toBe(true);
  }
});
test("measurement negatives preserve legitimate model-paint pairs", () => {
  const result = safetyResult("duplicateMeasurementIdentity");
  const bounds = { x: 0, y: 0, width: 10, height: 10 };
  const component = { name: "Header", measurementId: "layout:header", geometryRole: "mainHeaderChrome", bounds,
    visibleBounds: bounds, clipBounds: bounds, coordinateSpace: "window", measurementFrameGeneration: 1 };
  result.observation = { registeredProductionTarget: false, publishedProductionFrame: false, auxiliaryWindowClosed: true,
    auxiliaryWindowsRemaining: 0, layout: { components: [{ ...component, measurementProvenance: "model" }, { ...component, measurementProvenance: "paint-time" }] } };
  expect(nativeSafetyProbeAssertions(result).find(assertion => assertion.id.endsWith("measurement_ambiguity_rejected"))?.pass).toBe(false);
  result.observation.layout.components.push({ ...component, measurementProvenance: "paint-time" });
  expect(nativeSafetyProbeAssertions(result).every(assertion => assertion.pass)).toBe(true);
});

test("explicit action expectation is sent unchanged without an inspection repair", async () => {
  const { client, requests, changeState } = clientFixture();
  const target = await client.mount("main.script-list");
  const expected = { ...client.targets[0]! };
  changeState();
  const request = client.driver.request.bind(client.driver);
  client.driver.request = async (command, options) => {
    if (command.type === "simulateGpuiEvent") {
      expect(command.expected).toEqual(expected);
      throw new DriverCommandRefused("stale_target_identity", "stale-action");
    }
    return request(command, options);
  };
  const before = requests.length;
  await expect(client.act(target, { type: "key", key: "down" }, expected)).rejects.toMatchObject({ code: "stale_target_identity" });
  expect(requests.slice(before).some(command => command.type === "getState")).toBe(false);
  expect(client.targets[0]).toEqual(expected);
});

test("pointer actions carry the observed frame unchanged through a later stale refusal", async () => {
  const { client, requests, changeState } = clientFixture();
  const target = await client.mount("main.script-list");
  const frame = await client.frame(target); const expected = frame.target;
  changeState();
  let observed: Json | undefined;
  const request = client.driver.request.bind(client.driver);
  client.driver.request = async (command, options) => {
    if (command.type === "simulateGpuiEvent") {
      observed = command;
      throw new DriverCommandRefused("stale_frame_identity", "pointer-action");
    }
    return request(command, options);
  };
  const before = requests.length;
  await expect(client.act(target, { type: "gpuiEvent", frame, event: { type: "mouseUp", x: 12, y: 40, button: "left" } }, expected)).rejects.toMatchObject({ code: "stale_frame_identity" });
  expect(observed?.expectedFrame).toEqual(frame); expect(observed?.expected).toEqual(expected);
  expect(requests.slice(before).some(command => command.type === "getState")).toBe(false);
});

test("a successful action cannot rebound its caller expectation to a newer observation", async () => {
  const { client, changeState } = clientFixture();
  const target = await client.mount("main.script-list");
  const expected = { ...client.targets[0]! };
  changeState();
  await expect(client.act(target, { type: "key", key: "down" }, expected)).rejects.toThrow("action_expectation_rebound");
  expect(client.targets[0]).toEqual(expected);
});

test("single semantic batch requires its completed command rather than only outer success", async () => {
  const { client } = clientFixture(); const target = await client.mount("main.script-list");
  const expected = { ...client.targets[0]! };
  client.driver.request = async (command) => {
    expect(command.type).toBe("batch"); expect(command.commands).toHaveLength(1); expect(command.expected).toEqual(expected);
    return { type: "batchResult", requestId: "batch", success: true, results: [{ success: false, actionReceipt: {
      requestId: "batch", operationId: "select", before: expected, after: expected, dispatchCompleted: true, wasDeferred: true,
      effect: { kind: "noOp", reason: "failed-later-command" },
    } }] };
  };
  await expect(client.act(target, { type: "select", semanticId: "row:observed" }, expected)).rejects.toThrow("single_action_batch_not_completed");
});

test("scheduled capture never accepts a forced result or a missing notification", async () => {
  for (const mode of ["forced", "scheduled"]) {
    const { client } = clientFixture({ pngBase64: capturePng }, result => {
      result.frameEvidence = { mode, notificationEpoch: mode === "forced" ? 2 : 1 };
    });
    const target = await client.mount("main.script-list");
    await expect(client.captureFrame(target, false, { expected: { ...initial }, afterFrameGeneration: 0, afterNotificationEpoch: 1 })).rejects.toThrow("scheduled_notification_not_observed");
  }
});

test("scheduled capture preserves its caller baseline and exact owner expectation", async () => {
  const { client, requests } = clientFixture({ pngBase64: capturePng }, result => {
    result.frameEvidence = { mode: "scheduled", notificationEpoch: 9, traceGeneration: 1 };
  });
  const target = await client.mount("main.script-list");
  const scheduled = { expected: { ...initial }, afterFrameGeneration: 0, afterNotificationEpoch: 8 };
  const capture = await client.captureFrame(target, false, scheduled);
  expect(capture.frame.target.frameGeneration).toBe(1);
  expect(requests.at(-1)?.command.scheduled).toEqual(scheduled);
  expect(requests.at(-1)?.command.includeImage).toBe(false);
});

test("scheduled capture rejects ignored or mismatched nested frame cursors without rebinding its expectation", async () => {
  for (const change of [
    (trace: Json) => { delete trace.afterFrameGeneration; },
    (trace: Json) => { trace.afterFrameGeneration = null; },
    (trace: Json) => { trace.traceGeneration++; },
    (trace: Json) => { trace.latestFrameGeneration++; },
  ]) {
    const { client, requests } = clientFixture(undefined, result => {
      result.frameEvidence = { mode: "scheduled", notificationEpoch: 9, traceGeneration: 1 };
      change(result.state.frameEvidence);
    });
    const target = await client.mount("main.script-list"); const before = structuredClone(client.targets);
    const scheduled = { expected: { ...initial }, afterFrameGeneration: 0, afterNotificationEpoch: 8 };
    await expect(client.captureFrame(target, false, scheduled)).rejects.toThrow("frame_cursor_response_mismatch");
    expect(client.targets).toEqual(before); expect(requests.at(-1)?.command.scheduled).toEqual(scheduled);
    expect(requests).toHaveLength(2);
  }
});

test("pixel sampling rejects blank readback and does not issue a new draw", async () => {
  const { client } = clientFixture(); const target = await client.mount("main.script-list");
  const frame = await client.frame(target);
  client.driver.request = async (command) => {
    expect(command.type).toBe("captureRenderWindow"); expect(command.request.includeImage).toBe(false);
    expect(command.request.expected).toEqual(frame.target);
    return { snapshot: { status: "blankImageRejected", source: "gpuiRenderReadback", scope: "liveAutomationWindowRenderReadback" } };
  };
  await expect(client.probePixels(target, frame.target, [{ x: 0, y: 0 }])).rejects.toThrow("qualified_readback_failed");
});

test("explicit capture cursors page both histories without replacing the scheduled baseline or current facts", async () => {
  for (const scheduledCapture of [false, true]) {
    const { client, requests } = clientFixture(); const target = await client.mount("main.script-list");
    const previous = await client.frame(target);
    const cursor = Object.freeze({ traceGeneration: 1, afterFrameGeneration: previous.target.frameGeneration });
    const scheduled = scheduledCapture ? { expected: previous.target, afterFrameGeneration: 0, afterNotificationEpoch: 8 } : undefined;
    const capture = await client.captureFrame(target, false, scheduled, cursor);
    expect(requests.at(-1)?.command).toEqual({ operation: "captureFrame", target, includeImage: false, frameCursor: cursor,
      ...(scheduled ? { scheduled } : {}) });
    for (const trace of [capture.frameEvidence!, capture.state.frameEvidence]) {
      expect(trace).toMatchObject({ traceGeneration: 1, afterFrameGeneration: 1, latestFrameGeneration: 2 });
      expect(trace.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([2]);
    }
    expect(capture.frameHistoryBundle).toBeUndefined();
    expect(capture.frameEvidence!.historyScope).toBeUndefined();
    expect(capture.state.frameEvidence.historyScope).toBeUndefined();
    expect(capture.frameEvidence!.completedFrames[0]).toBe(capture.state.frameEvidence.completedFrames[0]);
    expect(capture.frameEvidence!.frame).toEqual(capture.frame);
    expect(capture.frameEvidence!.paintBindings).toEqual([{ kind: "current-frame", id: "unchanged" }]);
    expect(capture.frameEvidence!.pixelEvidence).toEqual([{ r: 10, g: 20, b: 30 }]);
    expect(requests).toHaveLength(3);
  }
});
test("capture cursor validation refuses malformed input before any capture request", async () => {
  const { client, requests } = clientFixture(); const target = await client.mount("main.script-list"); const before = structuredClone(client.targets);
  for (const cursor of [null, {}, [], { traceGeneration: 0, afterFrameGeneration: 0 }, { traceGeneration: 1, afterFrameGeneration: -1 },
      { traceGeneration: 1, afterFrameGeneration: 0.5 }, { traceGeneration: 1, afterFrameGeneration: Number.MAX_SAFE_INTEGER + 1 },
      { traceGeneration: 1, afterFrameGeneration: 0, extra: true }]) {
    await expect(client.captureFrame(target, false, undefined, cursor as OwnedFrameCursor)).rejects.toThrow("frame_cursor_invalid");
    expect(requests).toHaveLength(1); expect(client.targets).toEqual(before);
  }
});
test("either ignored or malformed capture history fails closed before cached authority changes", async () => {
  for (const facet of ["frameEvidence", "state"] as const) for (const alter of [
    (trace: Json) => { delete trace.afterFrameGeneration; },
    (trace: Json) => { trace.afterFrameGeneration = null; },
    (trace: Json) => { trace.traceGeneration++; },
    (trace: Json) => { trace.latestFrameGeneration++; },
    (trace: Json, capture: Json) => { const frame = structuredClone(capture.frame); frame.target.frameGeneration = 0; trace.completedFrames.push({ traceGeneration: trace.traceGeneration, frame }); },
    (trace: Json, capture: Json) => { trace.completedFrames.push({ traceGeneration: trace.traceGeneration, frame: structuredClone(capture.frame) }); },
    (trace: Json, capture: Json) => { const frame = structuredClone(capture.frame); frame.processInstanceId = "foreign"; trace.completedFrames.push({ traceGeneration: trace.traceGeneration, frame }); },
    (trace: Json) => { trace.traceOverflow = true; },
  ]) {
    const { client, requests } = clientFixture(undefined, capture => alter(facet === "state" ? capture.state.frameEvidence : capture.frameEvidence, capture));
    const target = await client.mount("main.script-list"); const before = structuredClone(client.targets);
    await expect(client.captureFrame(target, false, undefined, { traceGeneration: 1, afterFrameGeneration: 0 })).rejects.toThrow("frame_cursor_response_mismatch");
    expect(requests).toHaveLength(2); expect(client.targets).toEqual(before);
  }
});
test("native capture cursor refusals never cause a retry or forced fallback", async () => {
  for (const code of ["frame_cursor_stale", "frame_cursor_retired", "frame_cursor_future"]) {
    const { client } = clientFixture(); const target = await client.mount("main.script-list"); const before = structuredClone(client.targets); const sent: Json[] = [];
    client.driver.request = async command => { sent.push(command); return { requestId: "request", result: {
      operation: "captureFrame", ok: false, error: { code, message: code } } }; };
    const cursor = { traceGeneration: 1, afterFrameGeneration: 0 };
    await expect(client.captureFrame(target, false, undefined, cursor)).rejects.toMatchObject({ code });
    expect(sent).toEqual([{ type: "design", command: { operation: "captureFrame", target, includeImage: false, frameCursor: cursor } }]);
    expect(client.targets).toEqual(before);
  }
});
test("omitting the capture cursor retains the existing forced full and scheduled baseline response contracts", async () => {
  for (const scheduledCapture of [false, true]) {
    const { client, requests } = clientFixture(undefined, result => {
      result.frameEvidence = { traceGeneration: 1, mode: scheduledCapture ? "scheduled" : "forced", notificationEpoch: 9,
        afterFrameGeneration: scheduledCapture ? 0 : null, latestFrameGeneration: 1, traceOverflow: false,
        completedFrames: [{ traceGeneration: 1, frame: result.frame }] };
      if (!scheduledCapture) result.state.frameEvidence = structuredClone(result.frameEvidence);
    });
    const target = await client.mount("main.script-list");
    const scheduled = scheduledCapture ? { expected: initial, afterFrameGeneration: 0, afterNotificationEpoch: 8 } : undefined;
    const capture = await client.captureFrame(target, false, scheduled);
    expect(Object.hasOwn(requests.at(-1)!.command, "frameCursor")).toBe(false);
    expect(capture.frameEvidence!.afterFrameGeneration).toBe(scheduledCapture ? 0 : null);
    expect(capture.frameEvidence!.completedFrames).toHaveLength(1);
    expect(capture.frameHistoryBundle).toBeUndefined();
    expect(Object.hasOwn(capture.frameEvidence!, "historyScope")).toBe(false);
  }
});

function captureBundleFixture(alter: (result: Json, current: Json) => void = () => {}) {
  const evidence: { wire?: Json; response?: Json; current?: Json } = {};
  const fixture = clientFixture(undefined, result => {
    for (const target of [result.frame.target, result.snapshot.frameIdentity.target,
      result.state.targetIdentity, result.elements.targetIdentity, result.layout.targetIdentity]) target.frameGeneration = 7;
    const current: Json = { frame: structuredClone(result.frame), traceGeneration: 4, mode: "scheduled",
      invalidationEpoch: 19, notificationEpoch: 20, notificationCause: { kind: "mainSearchPublication", sequence: 3, notificationEpoch: 20 },
      cause: "rootEntityNotify", localInputFocused: true, nativeWindowActive: false, nativeWindow: { visible: false, key: false },
      search: { rawInput: "private pooled query", committedRows: [{ label: "private pooled row" }] }, fileSearch: null,
      paintBindings: [{ kind: "mainSearch", id: "main-search", metadata: { privatePayload: "private binding bytes" } }],
      paintFailures: [], pixelEvidence: [{ kind: "selectionMarker", probe: { x: 1, y: 2, r: 3, g: 4, b: 5, a: 255 } }],
      pixelEvidenceComplete: true, pendingResources: 0, failedResources: 0, additionalNativeFact: { preserved: true } };
    const earlier = [2, 5].map(generation => {
      const stamp = structuredClone(current); stamp.frame.target.frameGeneration = generation; return stamp;
    });
    const page = { traceGeneration: 4, afterFrameGeneration: 1, latestFrameGeneration: 7, traceOverflow: false,
      maxCompletedStamps: 96, maxRetainedTraceBytes: 1048576, historyScope: "captureBundle" };
    result.frameEvidence = { ...current, ...page, completedFrames: earlier, scheduledCapability: true,
      transientPixelsRetained: true, transientPixelEvidence: "bounded-native-selection-samples; full latest framebuffer only" };
    result.state.frameEvidence = { ...page, completedFrames: [], scheduledCapability: true, traceError: null,
      retiredBeforeFrameGeneration: 0, retainedTraceBytes: 1234 };
    result.frameHistoryBundle = { version: 1, captureFrameCount: 3, stateFrameCount: 3 };
    evidence.current = structuredClone(current);
    alter(result, current);
    evidence.wire = structuredClone(result); evidence.response = result;
  });
  return { ...fixture, evidence };
}

test("capture bundle restores complete pages losslessly with shared references safe for receipt privacy", async () => {
  const { client, requests, evidence } = captureBundleFixture(); const target = await client.mount("main.script-list");
  const capture = await client.captureFrame(target, false, undefined, { traceGeneration: 4, afterFrameGeneration: 1 });
  expect(requests).toHaveLength(2);
  expect(evidence.wire!.frameEvidence.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([2, 5]);
  expect(evidence.wire!.state.frameEvidence.completedFrames).toEqual([]);
  expect(evidence.wire!.frameHistoryBundle).toEqual({ version: 1, captureFrameCount: 3, stateFrameCount: 3 });
  for (const page of [capture.frameEvidence!, capture.state.frameEvidence]) {
    expect(page.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([2, 5, 7]);
    expect(Object.hasOwn(page, "historyScope")).toBe(false);
  }
  expect(capture.frameHistoryBundle).toBeUndefined();
  const current = capture.frameEvidence!.completedFrames[2];
  expect(current).toEqual(evidence.current);
  expect(current.search).toBe(capture.frameEvidence!.search);
  expect(current.paintBindings).toBe(capture.frameEvidence!.paintBindings);
  for (let index = 0; index < 3; index++) expect(capture.frameEvidence!.completedFrames[index]).toBe(capture.state.frameEvidence.completedFrames[index]);
  const before = JSON.stringify(capture);
  const scan = sanitizeReceipt(annotateOwnedEvidence(capture), { mode: "fixture-redacted", fixtureId: "capture-bundle" });
  expect(scan.rawContentReturned).toBe(false);
  expect(JSON.stringify(scan.sanitized)).not.toContain("private ");
  expect(JSON.stringify(capture)).toBe(before);
  expect(capture.frameEvidence!.completedFrames[2]).toBe(capture.state.frameEvidence.completedFrames[2]);
});

test("capture bundle rejects missing duplicate foreign and miscounted frame evidence without advancing authority", async () => {
  const mutations: Array<(result: Json, current: Json) => void> = [
    result => { delete result.frameHistoryBundle; }, result => { result.frameHistoryBundle.version = 2; },
    result => { result.frameHistoryBundle.extra = true; }, result => { result.frameHistoryBundle.captureFrameCount = 2; },
    result => { result.frameHistoryBundle.stateFrameCount = 4; }, result => { result.frameHistoryBundle.stateFrameCount = 1.5; },
    result => { delete result.frameEvidence.historyScope; }, result => { delete result.state.frameEvidence.historyScope; },
    result => { result.frameEvidence.historyScope = "complete"; }, result => { result.state.frameEvidence.traceGeneration++; },
    result => { result.state.frameEvidence.afterFrameGeneration = 0; }, result => { result.frameEvidence.completedFrames.pop(); },
    result => { result.frameEvidence.completedFrames.reverse(); },
    (result, current) => { result.frameEvidence.completedFrames.push(structuredClone(current)); },
    result => { result.state.frameEvidence.completedFrames.push(structuredClone(result.frameEvidence.completedFrames[0])); },
    result => { result.frameEvidence.completedFrames[0].frame.processInstanceId = "foreign"; },
    result => { result.frameEvidence.frame = { ...result.frameEvidence.frame, nativeWindowId: 9 }; },
  ];
  for (const alter of mutations) {
    const { client, requests } = captureBundleFixture(alter); const target = await client.mount("main.script-list");
    await expect(client.captureFrame(target, false, undefined, { traceGeneration: 4, afterFrameGeneration: 1 }))
      .rejects.toThrow("frame_cursor_response_mismatch");
    expect(client.targets).toEqual([initial]); expect(requests).toHaveLength(2);
  }
});

test("capture bundle preserves genuinely newer state frames before enforcing existing atomic capture identity", async () => {
  const { client, evidence } = captureBundleFixture((result, current) => {
    const newer = structuredClone(current); newer.frame.target.frameGeneration = 9;
    result.state.frameEvidence.completedFrames.push(newer); result.state.frameEvidence.latestFrameGeneration = 9;
    result.frameHistoryBundle.stateFrameCount = 4;
  });
  const target = await client.mount("main.script-list");
  await expect(client.captureFrame(target, false, undefined, { traceGeneration: 4, afterFrameGeneration: 1 }))
    .rejects.toThrow("frame_cursor_response_mismatch");
  expect(evidence.response!.frameEvidence.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([2, 5, 7]);
  expect(evidence.response!.state.frameEvidence.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([2, 5, 7, 9]);
  expect(client.targets).toEqual([initial]);
});

test("ordinary inspection and default capture never accept partial capture bundle pages", async () => {
  for (const cursor of [undefined, { traceGeneration: 7, afterFrameGeneration: 1 }]) {
    const { client } = clientFixture(undefined, undefined, (state, command) => {
      inspectionPage(state, command); state.frameEvidence.historyScope = "captureBundle";
    });
    const target = await client.mount("main.script-list");
    await expect(client.inspect(target, cursor)).rejects.toThrow("frame_cursor_response_mismatch");
    expect(client.targets).toEqual([initial]);
  }
  const { client } = captureBundleFixture(); const target = await client.mount("main.script-list");
  await expect(client.captureFrame(target, false)).rejects.toThrow("frame_cursor_response_mismatch");
  expect(client.targets).toEqual([initial]);
});

test("capture bundle keeps an already-acknowledged current stamp outside both restored history windows", async () => {
  const { client, evidence } = captureBundleFixture(result => {
    for (const page of [result.frameEvidence, result.state.frameEvidence]) {
      page.afterFrameGeneration = 7; page.completedFrames = [];
    }
    result.frameHistoryBundle.captureFrameCount = 0; result.frameHistoryBundle.stateFrameCount = 0;
  });
  const target = await client.mount("main.script-list");
  const capture = await client.captureFrame(target, false,
    { expected: initial, afterFrameGeneration: 0, afterNotificationEpoch: 19 }, { traceGeneration: 4, afterFrameGeneration: 7 });
  expect(capture.frameEvidence!.completedFrames).toEqual([]);
  expect(capture.state.frameEvidence.completedFrames).toEqual([]);
  expect(capture.frameEvidence!.search).toEqual(evidence.current!.search);
  expect(capture.frameEvidence!.frame).toEqual(evidence.current!.frame);
});

function referenceFixtureSearchMetadata(stamp: Json): void {
  stamp.paintBindings.unshift({ kind: "mainSearchRow", id: "unrelated-row", metadata: {} });
  const index = stamp.paintBindings.findIndex((binding: Json) => binding.kind === "mainSearch" && binding.id === "main-search");
  stamp.paintBindings[index].metadata = structuredClone(stamp.search);
  delete stamp.search;
  stamp.searchMetadataRef = index;
}

test("search metadata references restore capture facts before bundle reconstruction without mutating receipt inputs", async () => {
  const { client, evidence } = captureBundleFixture(result => {
    for (const stamp of [result.frameEvidence, ...result.frameEvidence.completedFrames]) referenceFixtureSearchMetadata(stamp);
  });
  const target = await client.mount("main.script-list");
  const capture = await client.captureFrame(target, false, undefined, { traceGeneration: 4, afterFrameGeneration: 1 });
  expect(evidence.wire!.frameEvidence.searchMetadataRef).toBe(1);
  expect(Object.hasOwn(evidence.wire!.frameEvidence, "search")).toBe(false);
  expect(capture.frameEvidence!.search).toEqual(evidence.current!.search);
  expect(capture.frameEvidence!.search).toBe(capture.frameEvidence!.paintBindings[1].metadata);
  expect(capture.frameEvidence!.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([2, 5, 7]);
  for (const stamp of capture.frameEvidence!.completedFrames) {
    expect(stamp.search).toBe(stamp.paintBindings[1].metadata);
    expect(Object.hasOwn(stamp, "searchMetadataRef")).toBe(false);
  }
  expect(capture.state.frameEvidence.completedFrames[2]).toBe(capture.frameEvidence!.completedFrames[2]);
  const before = JSON.stringify(capture);
  const scan = sanitizeReceipt(annotateOwnedEvidence(capture), { mode: "fixture-redacted", fixtureId: "search-metadata-ref" });
  expect(JSON.stringify(scan.sanitized)).not.toContain("private ");
  expect(scan.rawContentReturned).toBe(false);
  expect(JSON.stringify(capture)).toBe(before);
});

test("search metadata references restore standalone cursor histories using the exact paint binding index", async () => {
  const { client, requests } = clientFixture(undefined, undefined, (state, command) => {
    inspectionPage(state, command);
    for (const stamp of state.frameEvidence.completedFrames) {
      stamp.search = { rawInput: "private inspected query", committedRows: [] };
      stamp.paintBindings = [{ kind: "mainSearch", id: "main-search", metadata: {} }];
      referenceFixtureSearchMetadata(stamp);
    }
  });
  const target = await client.mount("main.script-list");
  const state = await client.inspect(target, { traceGeneration: 7, afterFrameGeneration: 1 });
  expect(state.frameEvidence.completedFrames.map((stamp: Json) => stamp.frame.target.frameGeneration)).toEqual([3, 4]);
  for (const stamp of state.frameEvidence.completedFrames) {
    expect(stamp.search.rawInput).toBe("private inspected query");
    expect(stamp.search).toBe(stamp.paintBindings[1].metadata);
    expect(Object.hasOwn(stamp, "searchMetadataRef")).toBe(false);
  }
  expect(requests).toHaveLength(2);
});

test("search metadata references reject mixed malformed foreign and ambiguous bindings before authority changes", async () => {
  const mutations: Array<(stamp: Json) => void> = [
    stamp => { stamp.search = {}; }, stamp => { stamp.search = undefined; },
    ...[null, -1, 0.5, "1", Number.MAX_SAFE_INTEGER + 1, 9].map(index => (stamp: Json) => { stamp.searchMetadataRef = index; }),
    stamp => { stamp.searchMetadataRef = 0; }, stamp => { delete stamp.paintBindings; },
    stamp => { stamp.paintBindings[1].kind = "mainSearchRow"; }, stamp => { stamp.paintBindings[1].id = "other-root"; },
    stamp => { stamp.paintBindings[1].metadata = null; }, stamp => { stamp.paintBindings[1].metadata = []; },
    stamp => { stamp.paintBindings[1].metadata = "not-search-facts"; },
    stamp => { stamp.paintBindings.push(structuredClone(stamp.paintBindings[1])); }, stamp => { delete stamp.frame; },
  ];
  for (const surface of ["capture", "inspect"] as const) for (const alter of mutations) {
    const fixture = surface === "capture" ? captureBundleFixture(result => {
      referenceFixtureSearchMetadata(result.frameEvidence); alter(result.frameEvidence);
    }) : clientFixture(undefined, undefined, (state, command) => {
      inspectionPage(state, command);
      const stamp = state.frameEvidence.completedFrames[0];
      stamp.search = { rawInput: "private inspected query" };
      stamp.paintBindings = [{ kind: "mainSearch", id: "main-search", metadata: {} }];
      referenceFixtureSearchMetadata(stamp); alter(stamp);
    });
    const target = await fixture.client.mount("main.script-list");
    await expect(surface === "capture" ? fixture.client.captureFrame(target, false, undefined, { traceGeneration: 4, afterFrameGeneration: 1 }) :
      fixture.client.inspect(target, { traceGeneration: 7, afterFrameGeneration: 1 })).rejects.toThrow("frame_cursor_response_mismatch");
    expect(fixture.client.targets).toEqual([initial]); expect(fixture.requests).toHaveLength(2);
  }
});

test("search metadata references are refused in unopted default inspection and capture replies", async () => {
  const inspected = clientFixture(undefined, undefined, (state, command) => {
    inspectionPage(state, command);
    const stamp = state.frameEvidence.completedFrames[0];
    stamp.search = { rawInput: "private default query" };
    stamp.paintBindings = [{ kind: "mainSearch", id: "main-search", metadata: {} }];
    referenceFixtureSearchMetadata(stamp);
  });
  const inspectedTarget = await inspected.client.mount("main.script-list");
  await expect(inspected.client.inspect(inspectedTarget)).rejects.toThrow("frame_cursor_response_mismatch");
  expect(inspected.client.targets).toEqual([initial]);
  for (const scheduledCapture of [false, true]) {
    const { client } = captureBundleFixture(result => {
      referenceFixtureSearchMetadata(result.frameEvidence);
      delete result.frameHistoryBundle;
      for (const page of [result.frameEvidence, result.state.frameEvidence]) {
        delete page.historyScope; page.afterFrameGeneration = scheduledCapture ? 0 : null;
      }
      result.frameEvidence.mode = scheduledCapture ? "scheduled" : "forced";
    });
    const target = await client.mount("main.script-list");
    const scheduled = scheduledCapture ? { expected: initial, afterFrameGeneration: 0, afterNotificationEpoch: 19 } : undefined;
    await expect(client.captureFrame(target, false, scheduled)).rejects.toThrow("frame_cursor_response_mismatch");
    expect(client.targets).toEqual([initial]);
  }
});
