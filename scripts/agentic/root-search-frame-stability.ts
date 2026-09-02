#!/usr/bin/env bun
import { readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { OwnedEvaluationClient } from "../devtools/lib/owned-evaluation.ts";
import { readArtifactReference } from "../devtools/design.ts";
import { DriverLifecycleError, normalizeProtocolResponse, unknownOwnedCleanup, type Json } from "../devtools/driver.ts";
import { verifyImmutableArtifact } from "./build-artifact.ts";
import { claimOutput, validateOutputTarget, beginManagedTask, finalizeManagedTask, materializeAtomic, writeJsonArtifactAtomic,
  validateArtifact, buildArtifactLifecycle, commitFinalReceipt, type ArtifactSpec, type OwnedCleanup } from "./artifact-lifecycle.ts";
import { assertPerformanceContract } from "../devtools/lib/performance-contract.ts";
import type { AutomationInstance } from "../devtools/lib/target-identity.ts";

const repoRoot = resolve(import.meta.dir, "../..");
const args = Bun.argv.slice(2);
const arg = (name: string) => { const index = args.indexOf(name); return index < 0 ? undefined : args[index + 1]; };
const fixtureId = "main.root-search-frame-stability";
const negativeControlContract = { kind: "synthetic_semantic_fingerprint_mutation", appliedAfterProviderSettlement: true, nativeShiftObserved: false };
const help = "root-search-frame-stability --artifact <reference.json> --receipt <fresh-receipt.json> [--query zzqxframeproof] [--inject-forbidden-shift] [--describe-contract]\nUses the sealed owned evaluator, never CI forgery, inherited launch env or native capture.";
const safety = { startsApplication: false, runtimeStartsApplication: true, runtimeRequiresSandboxHome: true, runtimeRequiresHiddenWindow: true,
  runtimeRequiresNoninteractive: true, runtimeRequiresCiEnvironment: false, runtimeRequiresSealedEvaluatorPermit: true,
  revealsWindow: false, focusesWindow: false, drivesNativeInput: false, capturesScreen: false };
if (args.includes("--help") || args.includes("-h")) { console.log(help); process.exit(0); }
if (args.includes("--describe-contract")) {
  const contract = { schemaVersion: 1, tool: "root-search-frame-stability", evidenceClass: "STATIC_INVENTORY", runtimeEvidenceClass: "RUNTIME_HIDDEN",
    metricKind: "semantic_frame_identity", observationClass: "SEMANTIC_FRAME", observationPoint: "stateResult.mainWindowPreflight.semanticFingerprint", measuresPaint: false,
    fixtureId, negativeControl: negativeControlContract, safety };
  assertPerformanceContract(contract); console.log(JSON.stringify(contract)); process.exit(0);
}
if (process.env.SCRIPT_KIT_NONINTERACTIVE !== "1") throw new Error("refused before app launch: SCRIPT_KIT_NONINTERACTIVE=1 is required");
if (!arg("--artifact") || !arg("--receipt")) throw new Error(help);
const reference = readArtifactReference(arg("--artifact")!);
const artifact = verifyImmutableArtifact(repoRoot, reference, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui",
  sourcePolicy: args.includes("--packaged") ? "clean-exact-head" : "current-content" });
if (args.includes("--packaged") && !artifact.manifest.derivation) throw new Error("packaged_root_frame_requires_signed_bundle_derivation");
const query = arg("--query") ?? "zzqxframeproof";
const timeoutMs = Number(arg("--timeout") ?? "10000");
if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 60000) throw new Error("invalid root-frame timeout");
const claim = claimOutput(validateOutputTarget({ repoRoot, candidate: resolve(arg("--receipt")!), kind: "receipt", probeId: "root-search-frame-stability" }));
const task = beginManagedTask(claim, "evidence-run", [reference]);
const inject = args.includes("--inject-forbidden-shift");
let hiddenStates = 0;
function requireHidden(state: Json): void {
  if (state.windowVisible !== false) throw new Error("hidden root identity not observed"); hiddenStates++;
}
export function semanticFrame(state: Json, elements: Json): Json {
  const preflight = state.mainWindowPreflight;
  const fields = ["selectedIndex", "selectedResultKey", "selectedResultRole", "visibleResultKeyFingerprint", "visibleRowFingerprint", "visibleResultCount", "visibleResults", "enterAction"];
  if (!preflight || fields.some(field => !(field in preflight))) throw new Error("mainWindowPreflight semantic fields unavailable");
  if (!Array.isArray(elements.elements)) throw new Error("semantic elements unavailable");
  return { ...Object.fromEntries(fields.map(field => [field, preflight[field]])), elementsFingerprint: elements.elements
    .filter((element: Json) => typeof element.semanticId === "string")
    .map((element: Json) => [element.role ?? "", element.semanticId, element.text ?? "", element.index ?? "", element.action ?? ""].join(":"))
    .join("|") };
}
export function assertSameSemanticFrame(before: Json, after: Json): void {
  if (JSON.stringify(before) !== JSON.stringify(after)) throw new Error("visible semantic frame shifted while provider resolved");
}
let client: OwnedEvaluationClient | undefined;
let cleanup: OwnedCleanup = unknownOwnedCleanup(false);
const receipt: Json = { schemaVersion: 3, gateId: "root-frame-stable", evidenceClass: "RUNTIME_HIDDEN", metricKind: "semantic_frame_identity",
  observationClass: "SEMANTIC_FRAME", observationPoint: "stateResult.mainWindowPreflight.semanticFingerprint", measuresPaint: false,
  status: "error", behavior: { status: "fail", failure: null }, samples: [], query, fixtureId, injectForbiddenShift: inject,
  delayedProvider: { status: "pending" }, stableSemanticFrames: { status: "pending" },
  negativeControl: inject ? { ...negativeControlContract, applied: false } : null,
  provenance: { artifactReference: reference, binary: artifact.executablePath, binarySha256: artifact.manifest.binarySha256,
    gitSha: artifact.manifest.source.gitHead, sourceDirty: artifact.manifest.source.repositoryDirty, source: artifact.manifest.source,
    derivation: artifact.manifest.derivation ?? null }, safety: { ...safety, startsApplication: true, hiddenStateAssertionCount: 0 } };
try {
  client = await OwnedEvaluationClient.launch(repoRoot, reference, claim, [fixtureId], args.includes("--packaged") ? "clean-exact-head" : "current-content");
  const target: AutomationInstance = await client.mount(fixtureId);
  receipt.requestedTarget = target; receipt.session = { name: client.driver.sessionName, ...client.driver.processIdentity };
  requireHidden(await client.inspect(target));
  await client.act(target, { type: "setInput", text: query });
  const before = await client.inspect(target); requireHidden(before);
  const status = before.rootFileSearch;
  if (status?.query !== query || status.mode !== "GlobalQuery" || status.providerLoading !== true || status.loading !== true)
    throw new Error("early_global_query_provider_loading_frame_required");
  if (status.visibleResultCount !== 0 || status.cacheEntryCount !== 0 || status.cacheResultCount !== 0)
    throw new Error("delayed_provider_must_start_without_published_or_cached_results");
  if (status.visibleLoading !== false) throw new Error("passive_provider_must_not_own_visible_loading");
  receipt.delayedProvider.initial = status;
  const baseline = semanticFrame(before, await client.query(target, "elements")); receipt.baseline = baseline;
  const deadline = performance.now() + timeoutMs;
  let settled = false;
  for (let index = 0; index < 200 && performance.now() < deadline; index++) {
    await client.frame(target);
    const state = await client.inspect(target); requireHidden(state);
    const provider = state.rootFileSearch;
    if (provider?.query !== query || provider.mode !== "GlobalQuery" || provider.generation !== status.generation)
      throw new Error("provider_query_identity_changed");
    const frame = semanticFrame(state, await client.query(target, "elements"));
    receipt.samples.push({ rootFileSearch: provider, frame, injectionApplied: false });
    assertSameSemanticFrame(baseline, frame);
    if (provider.visibleResultCount !== 0) throw new Error("passive_provider_published_visible_results");
    if (provider.visibleLoading !== false) throw new Error("passive_provider_must_not_own_visible_loading");
    if (provider.providerLoading === false && provider.loading === false) {
      if (provider.cacheEntryCount !== 1 || provider.cacheResultCount !== 1)
        throw new Error("delayed_root_file_provider_completed_without_expected_result");
      settled = true; receipt.settled = frame;
      receipt.delayedProvider = { status: "pass", initial: status, completed: provider };
      break;
    }
  }
  if (!settled) throw new Error("delayed_root_file_provider_did_not_settle");
  receipt.completedFrame = await client.frame(target);
  receipt.stableSemanticFrames = { status: "pass", sampledFrames: receipt.samples.length };
  if (inject) {
    // Validator sensitivity only: never replace a real sample or claim a native shift.
    const candidateFrame = { ...receipt.settled, visibleRowFingerprint: "__injected_forbidden_shift__" };
    receipt.negativeControl = { ...negativeControlContract, applied: true, candidateFrame };
    assertSameSemanticFrame(baseline, candidateFrame);
    throw new Error("forbidden_shift_negative_control_was_not_rejected");
  }
  receipt.behavior.status = "pass";
} catch (error) {
  receipt.behavior.failure = error instanceof Error ? error.message : String(error);
  if (error instanceof DriverLifecycleError) cleanup = error.cleanup;
} finally {
  if (client) { try { cleanup = await client.close(); } catch { cleanup = client.cleanup; } }
  try { cleanup = finalizeManagedTask(task, cleanup).cleanup; } catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false }; }
  receipt.cleanup = cleanup; receipt.safety.hiddenStateAssertionCount = hiddenStates;
  const correlations = client?.driver.matchedResponses.map(({ requestId, expectedType }) => ({ requestId, expectedType: expectedType! })) ?? [];
  const specs: ArtifactSpec[] = [
    { id: "app-log", sourceName: "app.log", required: true, mediaType: "text/plain", kind: "text", acceptedTextMarkers: ['"operation":"bootstrap"'] },
    { id: "protocol-responses", sourceName: "protocol-responses.ndjson", required: true, mediaType: "application/x-ndjson", kind: "ndjson", correlations },
    { id: "lifecycle", sourceName: "lifecycle.json", required: true, mediaType: "application/json", kind: "json" },
  ];
  try {
    if (client?.driver.finalization.logWriterClosed) {
      materializeAtomic(claim, { sourceRoot: dirname(client.driver.logPath), sourceName: basename(client.driver.logPath), destinationName: "app.log" });
      const records: string[] = [];
      for (const line of readFileSync(client.driver.logPath, "utf8").split("\n")) {
        let parsed: Json;
        try { parsed = JSON.parse(line); } catch { continue; }
        // Malformed encoded evidence must fail preservation, not disappear as a non-JSON log line.
        const record = normalizeProtocolResponse(parsed);
        if (typeof record?.requestId === "string" && typeof record?.type === "string")
          records.push(record === parsed ? line : JSON.stringify(record));
      }
      writeFileSync(join(claim.artifactsRoot, "protocol-responses.ndjson"), records.join("\n") + "\n", { flag: "wx", mode: 0o600 });
    }
    writeJsonArtifactAtomic(claim, "lifecycle.json", { ...cleanup, schemaVersion: 1, probeId: "root-search-frame-stability", runId: claim.owner.runId, finalizationKind: "driver-close" });
  } catch (error) { receipt.behavior.failure ??= `artifact_preservation_failed:${String(error)}`; }
  const artifacts = specs.map(spec => validateArtifact(join(claim.artifactsRoot, spec.sourceName), spec, claim.artifactsRoot));
  receipt.artifactLifecycle = buildArtifactLifecycle({ claim, finalizationKind: "driver-close", writersFinalized: cleanup.closed, specs, artifacts });
  receipt.status = receipt.behavior.status === "pass" && cleanup.closed && receipt.artifactLifecycle.allRequiredValid && receipt.artifactLifecycle.allRecordedPathsReadable ? "pass" : "error";
  receipt.failure = receipt.behavior.failure ?? (!cleanup.closed ? "INVALID_CLEANUP" : null);
  commitFinalReceipt(claim, receipt, specs, artifacts);
}
console.log(JSON.stringify(receipt)); process.exit(receipt.status === "pass" ? 0 : 1);
