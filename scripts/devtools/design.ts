#!/usr/bin/env bun
import { readFileSync, lstatSync, realpathSync, watch, openSync, closeSync, readSync, fstatSync, constants } from "node:fs";
import { join, resolve, relative } from "node:path";
import { createInterface } from "node:readline";
import { createHash } from "node:crypto";
import { claimOutput, validateOutputTarget, writeJsonArtifactAtomic, beginManagedTask, finalizeManagedTask,
  type OutputClaim, type OwnedCleanup } from "../agentic/artifact-lifecycle.ts";
import { buildArtifactLifecycle, validateArtifact, commitFinalReceipt, type ArtifactSpec } from "../agentic/artifact-lifecycle.ts";
import { ArtifactVerificationError, type ArtifactReference } from "../agentic/build-artifact.ts";
import { DriverCommandRefused, DriverLifecycleError, unknownOwnedCleanup, type Json } from "./driver.ts";
import { OwnedEvaluationClient, EvaluationContractError, publicationCausalityIssues, validateThemeEdits, isOwnedFileSearchStreamObservation, isOwnedFileSearchPreviewObservation,
  NATIVE_SAFETY_PROBES, type NativeSafetyProbeResult, type FixtureDescriptor, type LiveThemeEdit, type FixtureControl } from "./lib/owned-evaluation.ts";
import { type AutomationInstance, type CompletedFrameIdentity } from "./lib/target-identity.ts";
import { CORE_JOURNEYS, aggregateCleanup, type CoreJourneyId } from "./lib/story-contract.ts";
import { prepareValidatedReceipt, validateReceiptFile, RECEIPT_SCHEMA_VERSION } from "./lib/receipt-schema.ts";
import { compactOwnedReceipt, ownedObservationDocument, OBSERVATION_SPEC, resolveReceiptDetails } from "./lib/receipt-artifact.ts";
import { productStatic, diagnostic, userContent, filePath, classifyReceiptContent, inferredKindForKey } from "./lib/privacy.ts";
import { hashPngRegion, type PngDimensions, type PixelRegion } from "./lib/png-rgba.ts";
import { OWNED_EVALUATION_LIMITS } from "./lib/operator-safety.ts";
import { buildMeasurementJoins } from "./layout.ts";
import { runNativeLifecycleCampaign } from "./native-lifecycle.ts";
import { fixtureEvidenceIssues, type FixtureBinding } from "./lib/fixture-contract.ts";
import { runSdkJourney } from "./sdk-journey.ts";
import { runConversationAcceptance, CONVERSATION_FIXTURE_IDS } from "./conversation-journey.ts";
import { runNotesAcceptance, NOTES_ACCEPTANCE_FIXTURES } from "./notes-acceptance.ts";
import { runFooterOwnershipJourney, FOOTER_JOURNEY_ID } from "./footer-journey.ts";
import { runSearchJourney, type SearchRecipeOptions } from "../agentic/launcher-search-recipes.ts";
import { retainSearchShardEvidence, searchShardArtifactSpecs } from "../agentic/launcher-search-receipt.ts";
import { SEARCH_CASES, SEARCH_PROVIDERS, SEARCH_FIXTURE_ID, searchContractSpec, searchScheduleComparisonGroup } from "../agentic/launcher-search-contract.ts";

const REPOSITORY_ROOT = resolve(import.meta.dir, "../..");
const retainedCaptureSpecs = new WeakMap<OutputClaim, ArtifactSpec[]>();

async function captureEvidence(client: OwnedEvaluationClient, target: AutomationInstance, claim: OutputClaim): Promise<Json> {
  const retained = retainedCaptureSpecs.get(claim) ?? [];
  const includeImage = retained.length < 8;
  const response = await client.captureFrame(target, includeImage);
  const { snapshot, ...observation } = response;
  if (!includeImage) return { ...observation, ...snapshot, retainedImage: false };
  const sourceName = `render-capture-${retained.length + 1}.json`;
  writeJsonArtifactAtomic(claim, sourceName, { schemaVersion: 1, fixture: { kind: "owned-synthetic" }, ...response });
  retained.push({ id: `render-capture-${retained.length + 1}`, sourceName, required: true, mediaType: "application/json", kind: "json" });
  retainedCaptureSpecs.set(claim, retained);
  const { pngBase64: _pngBase64, ...capture } = snapshot.capture!;
  return { ...observation, ...snapshot, capture, retainedImage: true, sourceName };
}
export interface StoryAssertion { id: string; pass: boolean }
export interface RuntimeJourneyReceipt {
  id: string; proofLevel: "owned-production-runtime"; pass: boolean;
  assertions: StoryAssertion[]; frames: CompletedFrameIdentity[]; effects: Json[];
  fixtureIds: string[]; cleanup: OwnedCleanup; error?: string;
  binding?: FixtureBinding;
}
export const CORE_FIXTURES: Record<CoreJourneyId, readonly string[]> = {
  "launcher-ranking-provider": [SEARCH_FIXTURE_ID],
  "choice-prompt-completion": ["prompt.arg", "prompt.mini", "prompt.select"],
  "editable-prompt-validation": ["prompt.form", "prompt.fields", "prompt.editor", "prompt.template"],
  "actions-popup-activation": ["main.script-list", "secondary.actions", "secondary.confirm", "dictation.recording", "dictation.microphone-picker", "secondary.footer", "secondary.shortcut-recorder"],
  "notes-day-roundtrip": [...NOTES_ACCEPTANCE_FIXTURES],
  "conversation-recovery-stop": [...CONVERSATION_FIXTURE_IDS],
  "dictation-delivery-refusal": ["main.script-list", "dictation.recording", "agent-chat.standard.populated", "agent-chat.popup.history", "day-page.today"],
  "theme-publication-revert": ["main.script-list", "notes.editor", "main.theme-chooser"],
  "close-reopen-stale": ["main.script-list"],
};
export function readArtifactReference(path: string): ArtifactReference {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 16384) throw new EvaluationContractError("invalid_artifact_reference_file");
  const reference = JSON.parse(readFileSync(path, "utf8"));
  if (!reference || Object.keys(reference).sort().join(",") !== "manifestPath,manifestSha256" ||
      typeof reference.manifestPath !== "string" || !/^[a-f0-9]{64}$/.test(reference.manifestSha256)) throw new EvaluationContractError("invalid_artifact_reference");
  return reference;
}
export function createEvaluationClaim(out: string, probeId: string): OutputClaim {
  return claimOutput(validateOutputTarget({ repoRoot: REPOSITORY_ROOT, candidate: out, kind: "directory", probeId }));
}
function assertStory(receipt: RuntimeJourneyReceipt, id: string, pass: boolean): void {
  receipt.assertions.push({ id, pass });
  if (!pass) throw new EvaluationContractError(id);
}
function nodes(response: Json): Json[] {
  const values = response.elements ?? response.snapshot?.elements;
  if (!Array.isArray(values) || !values.length) throw new EvaluationContractError("nonempty_production_semantics_required");
  const ids = values.map((node: Json) => node.semanticId ?? node.id).filter((id: unknown) => typeof id === "string");
  if (new Set(ids).size !== ids.length) throw new EvaluationContractError("duplicate_semantic_id");
  return values;
}
async function observedState(client: OwnedEvaluationClient, target: AutomationInstance, predicate: (state: Json) => boolean): Promise<Json> {
  const deadline = performance.now() + 5000;
  for (let poll = 0; poll < 100 && performance.now() < deadline; poll++) {
    const state = await client.inspect(target);
    if (predicate(state)) return state;
    await client.frame(target);
  }
  throw new EvaluationContractError("production_postcondition_deadline");
}
async function fixtureControl(client: OwnedEvaluationClient, target: AutomationInstance, control: FixtureControl): Promise<Json> {
  const state = await client.inspect(target);
  const response = await client.driver.request({ type: "design", command: { operation: "fixtureControl", target,
    expected: state.targetIdentity ?? state.surfaceContract?.targetIdentity, control } });
  if (response.result?.ok !== true || response.result.operation !== "fixtureControl")
    throw new EvaluationContractError(response.result?.error?.code ?? "fixture_control_failed");
  return response.result;
}
// The opaque middle of the selected-row marker excludes rounded ends, caret blink,
// hover and background motion. Bounds come from this completed frame's paint, not screen positions.
export function selectedThemePixelRegion(state: Json, layout: Json, dimensions: PngDimensions, frameGeneration: number) {
  const scroll = state.mainListScroll;
  const selectedSemanticId = scroll?.selectedSemanticId;
  if (scroll?.selectedRowVisible !== true || typeof selectedSemanticId !== "string" || !selectedSemanticId.length)
    throw new EvaluationContractError("theme_selected_row_not_visible");
  const windowDimensions = { width: layout.windowWidth, height: layout.windowHeight };
  if (![windowDimensions.width, windowDimensions.height].every(value => typeof value === "number" && Number.isFinite(value) && value > 0) ||
      ![dimensions.width, dimensions.height].every(value => Number.isSafeInteger(value) && value > 0) ||
      dimensions.width * dimensions.height > OWNED_EVALUATION_LIMITS.maxImagePixels)
    throw new EvaluationContractError("theme_pixel_dimensions_invalid");
  const rect = (value: Json | undefined): PixelRegion => {
    if (!value || ![value.x, value.y, value.width, value.height].every(item => typeof item === "number" && Number.isFinite(item)) ||
        value.width <= 0 || value.height <= 0) throw new EvaluationContractError("theme_paint_bounds_missing");
    return { x: value.x, y: value.y, width: value.width, height: value.height };
  };
  const contains = (outer: PixelRegion, inner: PixelRegion) => inner.x >= outer.x && inner.y >= outer.y &&
    inner.x + inner.width <= outer.x + outer.width && inner.y + inner.height <= outer.y + outer.height;
  const measured = (name: string) => {
    const matches = (layout.components ?? []).filter((node: Json) => node.name === name && node.measurementProvenance === "paint-time");
    if (matches.length !== 1 || matches[0].coordinateSpace !== "window" || matches[0].measurementFrameGeneration !== frameGeneration)
      throw new EvaluationContractError("theme_frame_coherent_paint_required");
    const node = matches[0];
    const bounds = rect(node.bounds);
    if (!contains(rect(node.visibleBounds), bounds) || !contains(rect(node.clipBounds), bounds) ||
        !contains({ x: 0, y: 0, ...windowDimensions }, bounds)) throw new EvaluationContractError("theme_selected_region_clipped");
    return bounds;
  };
  const selectionMarkerId = `${selectedSemanticId}:selection-marker`;
  const rowBounds = measured(`list-row:${selectedSemanticId}`);
  const markerBounds = measured(selectionMarkerId);
  if (!contains(rowBounds, markerBounds)) throw new EvaluationContractError("theme_marker_outside_selected_row");
  const scaleX = dimensions.width / windowDimensions.width;
  const scaleY = dimensions.height / windowDimensions.height;
  const x = Math.ceil(markerBounds.x * scaleX);
  const y = Math.ceil((markerBounds.y + markerBounds.height / 4) * scaleY);
  const region = { x, y, width: Math.floor((markerBounds.x + markerBounds.width) * scaleX) - x,
    height: Math.floor((markerBounds.y + markerBounds.height * 3 / 4) * scaleY) - y };
  if (region.width <= 0 || region.height <= 0 || region.width * region.height < 8)
    throw new EvaluationContractError("theme_selected_region_too_small");
  return { selectedSemanticId, selectionMarkerId, coordinateSpace: "window", windowDimensions, dimensions, rowBounds, markerBounds, region };
}

export function nativeSafetyProbeAssertions(result: NativeSafetyProbeResult): StoryAssertion[] {
  const assertions: StoryAssertion[] = [];
  const check = (id: string, pass: boolean) => assertions.push({ id: `${result.probe}:${id}`, pass });
  const before = result.before ?? {}; const after = result.after ?? {}; const observed = result.observation ?? {};
  const beforeNative = before.native ?? {}; const afterNative = after.native ?? {};
  const validCount = (value: unknown) => Number.isSafeInteger(value) && (value as number) >= 0;
  check("negative_only", result.negativeOnly === true && result.productionEvidence === false);
  check("no_implementation_gap", result.implementationGap === null && observed.implementationGap == null);
  check("observed_native_counters", [beforeNative, afterNative].every(native => native.installed === true &&
    [native.openedWindows, native.liveWindows, native.completedFrames, native.readbackImages, native.refusedOperations].every(validCount)));
  check("owned_window_preserved", beforeNative.liveWindows > 0 && beforeNative.liveWindows === afterNative.liveWindows);
  check("native_state_preserved", result.windowStateUnchanged === true && [before.window, after.window].every(window =>
    window?.ownedHidden === true && window.active === false && window.native?.visible === false &&
    window.native?.key === false && window.native?.miniaturized === false && window.native?.appActive === false) &&
    ["focus"].every(field => before.window?.[field] === after.window?.[field]) &&
    ["x", "y", "width", "height"].every(field => before.window?.bounds?.[field] === after.window?.bounds?.[field]));
  check("fixture_effects_unchanged", validCount(before.completedFixtureEffects) && before.completedFixtureEffects === after.completedFixtureEffects);
  check("owned_copy_unchanged", result.ownedCopyUnchanged === true);
  if (observed.auxiliaryWindowsRemaining !== undefined) check("no_auxiliary_survivors", observed.auxiliaryWindowsRemaining === 0);
  if (observed.auxiliaryWindowClosed !== undefined) check("auxiliary_close_observed", observed.auxiliaryWindowClosed === true);
  const nativeRefused = () => check("native_refusal_observed", afterNative.refusedOperations > beforeNative.refusedOperations);
  const applicationRefused = () => check("application_refusal_observed", validCount(before.refusedEffects) && after.refusedEffects > before.refusedEffects);
  switch (result.probe) {
    case "invalidShow": case "invalidFocus": case "invalidDialog": case "invalidTabbing": case "invalidOversize": {
      nativeRefused();
      const code = result.probe === "invalidDialog" ? "owned_hidden_window_kind" :
        result.probe === "invalidTabbing" ? "owned_hidden_tabbing" : result.probe === "invalidOversize" ? "owned_hidden_pixel_limit" : "owned_hidden_show_or_focus";
      check("rejected_before_creation", observed.result?.returnedOk === false && observed.result?.errorCode === code &&
        observed.rootConstructorCalled === false && beforeNative.openedWindows === afterNative.openedWindows);
      break;
    }
    case "nativeActivation": case "nativeIme": case "globalPointer": case "directAppActivation":
      nativeRefused();
      check("native_owner_completed", result.probe === "globalPointer" ? observed.returnedInertOrigin === true : observed.returnedVoid === true);
      break;
    case "deferredDispatch":
      check("cancel_exact_terminal", observed.cancelTerminalReplies === 1 && observed.cancelled?.errorCode === "dispatch_cancelled" &&
        observed.cancelled.success === false && observed.cancelled.dispatchCompleted === false &&
        observed.cancelled.dispatchScheduled === false && observed.cancelled.wasDeferred === true);
      check("deadline_exact_terminal", observed.deadlineTerminalReplies === 1 && observed.expired?.errorCode === "dispatch_deadline_exceeded" &&
        observed.expired.success === false && observed.expired.dispatchCompleted === false &&
        observed.expired.dispatchScheduled === false && observed.expired.wasDeferred === true);
      check("duplicates_and_owner_unchanged", observed.duplicateTerminalReplies === 0 && observed.ownerUnchanged === true && observed.identityUnchanged === true);
      check("queued_stale_expectation_refused", observed.staleTerminalReplies === 1 && observed.staleExpectation?.errorCode === "stale_target_identity" &&
        observed.staleExpectation.success === false && observed.staleExpectation.dispatchCompleted === false &&
        observed.staleExpectation.dispatchScheduled === false && observed.staleExpectation.wasDeferred === true);
      check("ordinary_batch_deadline_bounded", observed.batchTerminalReplies === 1 && observed.batchReplayReplies === 0 &&
        observed.batchWait?.success === false && observed.batchWait.results?.[0]?.error?.code === "wait_condition_timeout" &&
        validCount(observed.batchWait.totalElapsed) && observed.batchWait.totalElapsed < 1000);
      if (observed.forgedSelection != null) check("forged_semantic_suffix_refused", observed.forgedSelection.success === false);
      break;
    case "clipboardRead": case "clipboardWrite": case "process": case "provider": case "credentials": case "device": case "openExternal": case "notification":
      applicationRefused();
      check("production_owner_refused", result.probe === "credentials" ? observed.returnedEmptyObject === true :
        result.probe === "notification" ? observed.returnedVoid === true : observed.result?.returnedOk === false);
      if (result.probe === "clipboardRead" || result.probe === "clipboardWrite") {
        check("apply_back_constructor_never_invoked", observed.applyBack?.constructorCalls === 0 && observed.applyBack.readRefused === true && observed.applyBack.writeRefused === true);
        check("terminal_fallback_refused_before_native_io", observed.applyBack?.terminalNoSelection === true && observed.applyBack.terminalPrimeRefused === true &&
          observed.applyBack.terminalFallbackCompleted === true && observed.applyBack.terminalFallbackRefused === true && observed.applyBack.terminalFallbackCompletions === 1 &&
          observed.applyBack.terminalFallbackCompletionKind === "synchronousRefusal" && observed.applyBack.terminalCallbackScheduled === false);
        check("terminal_probe_resources_released", observed.applyBack?.probeCleared === true && observed.applyBack.terminalFixtureReleased === true);
      }
      break;
    case "blankReadback": case "failedReadback":
      check("fault_reached_readback", observed.faultReachedBoundary === true && observed.faultCleared === true && observed.pixelsArePristine === false);
      check("no_accepted_pixels", observed.capture?.capture == null && (result.probe === "blankReadback" ?
        observed.completedFrame?.returnedOk === true && observed.capture?.status === "blankImageRejected" && observed.capture?.error?.code === "blank_image_rejected" :
        observed.completedFrame?.returnedOk === false && observed.completedFrame?.errorCode === "owned_readback_fault_failure" &&
        ["targetNotFound", "captureFailed"].includes(observed.capture?.status)));
      break;
    case "missingRequiredImage": case "missingRequiredSvg": case "oversizedImage":
      check("required_asset_failure_observed", observed.resources?.failed > 0 && observed.readback?.returnedOk === false &&
        ["owned_render_asset_failed", "owned_render_resources_incomplete"].includes(observed.readback?.errorCode));
      check("negative_root_not_published", observed.registeredProductionTarget === false && observed.publishedProductionFrame === false && observed.auxiliaryWindowClosed === true);
      if (result.probe === "oversizedImage") check("actual_oversized_image", observed.faultImage?.pixels > observed.faultImage?.pixelLimit &&
        observed.faultImage?.pixels === observed.faultImage?.width * observed.faultImage?.height && observed.faultImage?.frameCount === 1);
      break;
    case "duplicateSemanticIdentity": {
      let rejected = false;
      try { nodes({ elements: observed.elements }); } catch (error) { rejected = error instanceof EvaluationContractError && error.code === "duplicate_semantic_id"; }
      check("semantic_ambiguity_rejected", rejected);
      check("negative_root_not_published", observed.registeredProductionTarget === false && observed.publishedProductionFrame === false && observed.auxiliaryWindowClosed === true);
      break;
    }
    case "duplicateMeasurementIdentity": {
      const joins = buildMeasurementJoins((observed.layout?.components ?? []).map((node: Json) => ({
        measurementId: node.measurementId ?? `layout:${node.name}`, semanticId: node.semanticId ?? null,
        role: node.geometryRole ?? "other", bounds: node.bounds, visibleBounds: node.visibleBounds ?? null,
        clipBounds: node.clipBounds ?? null, measurementProvenance: node.measurementProvenance,
        coordinateSpace: node.coordinateSpace, measurementFrameGeneration: node.measurementFrameGeneration,
      })));
      check("measurement_ambiguity_rejected", joins.some(join => join.comparability === "DuplicateMeasurement" && join.classification === "NotComparable"));
      check("negative_root_not_published", observed.registeredProductionTarget === false && observed.publishedProductionFrame === false && observed.auxiliaryWindowClosed === true);
      break;
    }
    default: { const exhaustive: never = result.probe; throw new EvaluationContractError("unknown_native_probe", [exhaustive]); }
  }
  return assertions;
}

async function runNativeSafetyProbes(client: OwnedEvaluationClient, target: AutomationInstance): Promise<Json> {
  const catalog = await client.discover();
  const initialSurface = await client.inspect(target);
  const assertions: StoryAssertion[] = [{ id: "complete_safety_catalog", pass: Array.isArray(catalog.safetyProbes) &&
    catalog.safetyProbes.length === NATIVE_SAFETY_PROBES.length && new Set(catalog.safetyProbes).size === NATIVE_SAFETY_PROBES.length &&
    NATIVE_SAFETY_PROBES.every(probe => catalog.safetyProbes.includes(probe)) }];
  const probes: Json[] = [];
  const frameWaitContract = { conditionType: "completedFrame", targetType: "instance", expectedOptional: true, omittedExpectedObservesCurrent: true };
  const diagnostics = await client.diagnose();
  assertions.push({ id: "owned_request_capabilities_discoverable", pass: [catalog, diagnostics].every(value => {
    const capability = value as Json;
    return JSON.stringify(capability.reservedRequestIdPrefixes) === JSON.stringify(["owned-evaluation:batch:"]) &&
      Object.entries(frameWaitContract).every(([key, expected]) => capability.completedFrameWait?.[key] === expected);
  }) });
  let reservedCode: string | undefined;
  try { await client.driver.request({ type: "getState", target, requestId: "owned-evaluation:batch:external-probe" }); }
  catch (error) { if (error instanceof DriverCommandRefused) reservedCode = error.code; else throw error; }
  assertions.push({ id: "external_reserved_request_id_refused", pass: reservedCode === "evaluation_reserved_request_id" });
  probes.push({ id: "reserved_request_id", code: reservedCode ?? "unexpected_success" });
  for (const probe of NATIVE_SAFETY_PROBES) {
    try {
      const result = await client.probeSafety(target, probe);
      probes.push({ id: probe, result }); assertions.push(...nativeSafetyProbeAssertions(result));
      if (probe === "deferredDispatch" && target.id === "main" && initialSurface.targetIdentity.appViewVariant === "ScriptList")
        assertions.push({ id: "main_forged_semantic_suffix_refused", pass: result.observation.forgedSelection?.success === false });
    } catch (error) {
      const code = error instanceof EvaluationContractError || error instanceof DriverCommandRefused ? error.code : "native_probe_failed";
      probes.push({ id: probe, error: code }); assertions.push({ id: `${probe}:executed`, pass: false });
    }
  }
  const refused = async (id: string, message: Json, codes: readonly string[]) => {
    let code: string | undefined;
    try { if (message.type === "design") await client.design(message.command); else await client.driver.request(message); }
    catch (error) { if (error instanceof EvaluationContractError || error instanceof DriverCommandRefused) code = error.code; else throw error; }
    assertions.push({ id, pass: code !== undefined && codes.includes(code) }); probes.push({ id, code: code ?? "unexpected_success" });
  };
  try {
    const state = await client.inspect(target); const expected = state.targetIdentity;
    await client.act(target, { type: "setInput", text: "launch" });
    const changed = await client.inspect(target);
    assertions.push({ id: "owner_revision_actually_advanced", pass: changed.targetIdentity.dataGeneration > expected.dataGeneration });
    probes.push({ id: "owner_revision_transition", before: expected, after: changed.targetIdentity });
    const before = await client.diagnose();
    await refused("stale_owner_revision_rejected", { type: "design", command: { operation: "probeSafety", target,
      expected, probe: "nativeActivation" } }, ["stale_target_identity"]);
    const after = await client.diagnose();
    assertions.push({ id: "stale_revision_never_reaches_native_effect", pass: (before as Json).native?.refusedOperations === (after as Json).native?.refusedOperations });
    await client.act(target, { type: "setInput", text: "" });
    const themeBefore = await client.inspect(target);
    const publication = await client.applyTheme(themeBefore.targetIdentity.themeRevision, [{ tokenId: "theme.colors.accent.selected", value: 0x72c1a8 }]);
    try {
      const published = await observedState(client, target, state => state.targetIdentity.themeRevision === publication.revision);
      assertions.push({ id: "theme_revision_actually_advanced", pass: publication.revision > themeBefore.targetIdentity.themeRevision &&
        published.targetIdentity.themeRevision === publication.revision });
      await refused("stale_theme_publication_rejected", { type: "design", command: { operation: "applyTheme",
        expectedRevision: themeBefore.targetIdentity.themeRevision, edits: [{ tokenId: "theme.colors.accent.selected", value: 0x7281a8 }] } }, ["stale_theme_revision"]);
      const unchanged = await client.inspect(target);
      assertions.push({ id: "rejected_theme_remains_atomic", pass: unchanged.targetIdentity.themeRevision === publication.revision &&
        JSON.stringify(unchanged.liveTheme) === JSON.stringify(published.liveTheme) });
      probes.push({ id: "theme_revision_transition", before: themeBefore.targetIdentity, publication, after: unchanged.targetIdentity });
    } finally { await client.revertTheme(publication.revision); }
    const oldFrame = await client.frame(target); const currentFrame = await client.frame(target);
    let staleCapture: Json;
    await refused("explicit_stale_frame_wait_rejected", { type: "waitFor", target, condition: {
      type: "completedFrame", expected, afterFrameGeneration: oldFrame.target.frameGeneration,
    } }, ["stale_target_identity"]);
    for (const invalidExpected of [null, {}]) {
      let code: string | undefined;
      try { await client.driver.request({ type: "waitFor", target, condition: { type: "completedFrame", expected: invalidExpected,
        afterFrameGeneration: currentFrame.target.frameGeneration } }); }
      catch (error) { if (error instanceof DriverCommandRefused) code = error.code; else throw error; }
      assertions.push({ id: `explicit_${invalidExpected === null ? "null" : "malformed"}_frame_expectation_refused`, pass: code !== undefined });
      probes.push({ id: "malformed_frame_expectation", expected: invalidExpected, code: code ?? "unexpected_success" });
    }
    try { staleCapture = await client.driver.request({ type: "captureRenderWindow", request: { target, expected: oldFrame.target, hiDpi: true, includeImage: false } }); }
    catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; staleCapture = { refusedCode: error.code }; }
    probes.push({ id: "stale_completed_scene", oldFrame, currentFrame, response: staleCapture });
    assertions.push({ id: "stale_completed_scene_rejected", pass: currentFrame.target.frameGeneration > oldFrame.target.frameGeneration &&
      (staleCapture.refusedCode === "stale_frame_identity" || staleCapture.snapshot?.status === "captureFailed" && staleCapture.snapshot?.error?.code === "capture_frame_identity_stale") });
    const atomic = await client.captureFrame(target, false);
    probes.push({ id: "atomic_current_frame", frame: atomic.frame, snapshot: atomic.snapshot });
    assertions.push({ id: "atomic_frame_advances_without_prior_expectation", pass: atomic.frame.target.frameGeneration > currentFrame.target.frameGeneration });
    await refused("atomic_foreign_instance_rejected", { type: "design", command: { operation: "captureFrame",
      target: { type: "instance", id: "foreign-unmounted-window", generation: target.generation }, includeImage: false } }, ["target_not_mounted"]);
    await refused("atomic_stale_instance_rejected", { type: "design", command: { operation: "captureFrame",
      target: { ...target, generation: target.generation + 1 }, includeImage: false } }, ["stale_window_generation"]);
    const footerFixture = catalog.fixtures.find(fixture => fixture.family === "footer" && fixture.parentFixtureId === "main.script-list");
    assertions.push({ id: "parent_bound_footer_fixture_available", pass: footerFixture !== undefined });
    if (footerFixture) {
      const footer = await client.mount(footerFixture.id, target); const footerFrame = await client.frame(footer);
      await client.unmount(target); const reopened = await client.mount("main.script-list");
      assertions.push({ id: "parent_lifetime_reopened", pass: reopened.id === target.id && reopened.generation !== target.generation });
      await refused("stale_footer_parent_rejected", { type: "design", command: { operation: "mount", fixtureId: footerFixture.id,
        parent: target } }, ["stale_window_generation"]);
      await refused("retired_footer_cannot_target_reopened_parent", { type: "captureRenderWindow", request: {
        target: footer, expected: footerFrame.target, hiDpi: true, includeImage: false } }, ["target_not_mounted", "stale_window_generation"]);
      await refused("atomic_retired_instance_rejected", { type: "design", command: { operation: "captureFrame",
        target, includeImage: false } }, ["stale_window_generation"]);
      probes.push({ id: "footer_parent_retirement", parent: target, reopened, footer, footerFrame });
    }
    assertions.push({ id: "identity_negatives_completed", pass: true });
  } catch (error) {
    probes.push({ id: "identity_negatives", error: error instanceof EvaluationContractError || error instanceof DriverCommandRefused ? error.code : "identity_negative_failed" });
    assertions.push({ id: "identity_negatives_completed", pass: false });
  }
  return { negativeOnly: true, productionEvidence: false, probes, assertions };
}

export async function runRuntimeJourney(id: CoreJourneyId, reference: ArtifactReference, claim: OutputClaim, searchOptions: Omit<SearchRecipeOptions, "retainShard"> = {}): Promise<RuntimeJourneyReceipt> {
  if (id === "launcher-ranking-provider") return runSearchJourney(reference, claim, { ...searchOptions, retainShard: evidence => {
    const passed = evidence.cleanup.closed && evidence.results.every(result => result.status === "passed" || result.status === "notApplicable");
    const prepared = prepareValidatedReceipt("devtools.design.run", {
      schemaVersion: RECEIPT_SCHEMA_VERSION, tool: "script-kit-devtools.design", command: "design.run",
      classification: !evidence.cleanup.closed ? "invalid-cleanup" : passed ? "ok" : "reproduced",
      disposition: evidence.cleanup.closed ? undefined : "INVALID_CLEANUP", artifactReference: reference,
      evidenceClass: "DIRECT_RUNTIME_PROOF", provesRuntimeBehavior: passed, proofLevel: "owned-production-runtime",
      fixture: { kind: "owned-synthetic" }, observation: annotateOwnedEvidence(evidence),
      assertions: evidence.results.map(result => ({ id: result.id, pass: result.status === "passed" || result.status === "notApplicable" })),
      cleanup: evidence.cleanup, errors: diagnostic([]),
    });
    return retainSearchShardEvidence(claim, prepared.receipt);
  } });
  const receipt: RuntimeJourneyReceipt = { id, proofLevel: "owned-production-runtime", pass: false, assertions: [], frames: [], effects: [],
    fixtureIds: [...CORE_FIXTURES[id]], cleanup: unknownOwnedCleanup(false) };
  let client: OwnedEvaluationClient | undefined;
  try {
    const active = client = await OwnedEvaluationClient.launch(REPOSITORY_ROOT, reference, claim, receipt.fixtureIds);
    const mount = async (fixture: string, parent?: AutomationInstance) => {
      const target = await active.mount(fixture, parent);
      assertStory(receipt, `${fixture}:hidden`, (await active.inspect(target)).windowVisible === false);
      receipt.frames.push(await active.frame(target));
      nodes(await active.query(target, "elements"));
      return target;
    };
    const key = async (target: AutomationInstance, value: string, modifiers: string[] = [], text?: string) => {
      const effect = await active.act(target, { type: "key", key: value, modifiers, ...(text === undefined ? {} : { text }) });
      receipt.effects.push(effect); return effect;
    };
    const input = async (target: AutomationInstance, text: string) => {
      const effect = await active.act(target, { type: "setInput", text }); receipt.effects.push(effect); return effect;
    };
    switch (id) {
      case "choice-prompt-completion": {
        for (const fixture of CORE_FIXTURES[id]) {
          const target = await mount(fixture); await input(target, "");
          if (fixture === "prompt.select") {
            await input(target, "Choice 6");
            const filtered = (await active.inspect(target)).promptObservation;
            receipt.effects.push({ selectFiltered: filtered });
            assertStory(receipt, "select:filter_without_submission", filtered.input === "Choice 6" &&
              filtered.choiceCount === 1 && filtered.values.length === 0 && filtered.completion && !filtered.completion.receipt);
            await input(target, "");
            const cleared = (await active.inspect(target)).promptObservation;
            assertStory(receipt, "select:clear_restores_choices_without_submission", cleared.input === "" &&
              cleared.choiceCount === 6 && cleared.values.length === 0 && cleared.completion && !cleared.completion.receipt);
          }
          const initialFrame = await active.captureFrame(target, false); receipt.frames.push(initialFrame.frame);
          if (fixture === "prompt.arg" || fixture === "prompt.mini") {
            const retained = initialFrame.frame.target;
            const staleSubmit = async (label: string) => {
              let refused = false;
              try { await active.driver.request({ type: "simulateGpuiEvent", target, expected: retained, event: { type: "keyDown", key: "enter", modifiers: [] } }); }
              catch (error) { refused = error instanceof DriverCommandRefused && error.code === "stale_target_identity"; }
              const current = await active.inspect(target);
              assertStory(receipt, `${fixture}:${label}`, refused && !current.promptObservation.completion.receipt && current.targetIdentity.dataGeneration > retained.dataGeneration);
            };
            await key(target, "down"); await staleSubmit("retained_selection_submit_refused");
            await key(target, "up"); await staleSubmit("selection_aba_submit_refused");
          }
          if (fixture === "prompt.mini") {
            const viewport = initialFrame.state.promptObservation.choiceViewport;
            const row = initialFrame.layout.components.find((component: Json) => component.name === "mini-row:0" && component.measurementProvenance === "paint-time");
            assertStory(receipt, "mini:production_five_row_geometry", row?.bounds?.height === 44 && Math.abs(viewport.height - row.bounds.height * 5) < 1 && viewport.contentHeight > viewport.height && viewport.scrollOffsetY === 0);
          }
          for (let index = 0; index < 5; index++) await key(target, "down");
          const state = await active.inspect(target); const scroll = state.activeListScroll ?? state.mainListScroll;
          assertStory(receipt, `${fixture}:sixth_choice_revealed`, scroll?.selectedIndex === 5 && scroll?.selectedRowVisible === true && scroll?.selectedRowWithinSafeViewport === true);
          if (fixture === "prompt.mini") {
            const frame = await active.captureFrame(target, false); receipt.frames.push(frame.frame); receipt.effects.push(frame);
            const viewport = frame.state.promptObservation.choiceViewport;
            const row = frame.layout.components.find((component: Json) => component.name === "mini-row:5" && component.measurementProvenance === "paint-time");
            assertStory(receipt, "mini:sixth_row_real_scroll_and_safe_bounds", viewport.scrollOffsetY < initialFrame.state.promptObservation.choiceViewport.scrollOffsetY && viewport.pendingRevealIndex === null &&
              viewport.height === initialFrame.state.promptObservation.choiceViewport.height && row?.bounds?.y >= viewport.y && row.bounds.y + row.bounds.height <= viewport.y + viewport.height + 0.5);
          }
          await key(target, fixture === "prompt.select" ? "escape" : "enter");
          const completed = await observedState(active, target, state => !!state.promptObservation?.completion?.receipt);
          assertStory(receipt, `${fixture}:completion_once`, completed.promptObservation.completion.receipt.sequence === 1);
          await active.unmount(target);
        }
        break;
      }
      case "editable-prompt-validation": {
        for (const fixture of CORE_FIXTURES[id]) {
          const target = await mount(fixture); const before = await active.inspect(target);
          if (fixture === "prompt.form" || fixture === "prompt.fields") {
            await key(target, "enter");
            assertStory(receipt, `${fixture}:invalid_form_not_submitted`, !(await active.inspect(target)).promptObservation?.completion?.receipt);
          }
          await key(target, "a", ["cmd"]); await key(target, "x", [], ["prompt.form", "prompt.fields"].includes(fixture) ? "fixture@example.invalid" : fixture === "prompt.template" ? "edited-fixture" : "edited fixture");
          if (fixture === "prompt.template") {
            await key(target, "tab");
            assertStory(receipt, "prompt.template:tab_focuses_email", (await active.inspect(target)).promptObservation?.selectedIndex === 1);
            await key(target, "x", [], "fixture@example.invalid");
          }
          if (fixture === "prompt.editor") {
            await key(target, "enter");
            assertStory(receipt, "editor_newline_without_submission", !(await active.inspect(target)).promptObservation?.completion?.receipt);
          }
          const edited = await active.inspect(target);
          assertStory(receipt, `${fixture}:input_changed`, JSON.stringify(edited.promptObservation?.input ?? edited.promptObservation?.values) !== JSON.stringify(before.promptObservation?.input ?? before.promptObservation?.values));
          if (fixture === "prompt.template") {
            assertStory(receipt, "prompt.template:field_values_retained", JSON.stringify(edited.promptObservation?.values) === JSON.stringify([["script_name", "edited-fixture"], ["email", "fixture@example.invalid"]]));
          }
          const completionFrame = fixture === "prompt.template" ? await active.captureFrame(target, false) : undefined;
          await key(target, "enter", fixture === "prompt.editor" ? ["cmd"] : []);
          const completed = await observedState(active, target, state => !!state.promptObservation?.completion?.receipt);
          assertStory(receipt, `${fixture}:completion_once`, completed.promptObservation.completion.receipt.sequence === 1);
          if (completionFrame) {
            assertStory(receipt, "template:completion_advances_owner_epoch", completed.promptObservation.completion.semanticRevision > completionFrame.state.promptObservation.completion.semanticRevision && completed.targetIdentity.dataGeneration > completionFrame.frame.target.dataGeneration);
            let staleCapture: Json;
            try { staleCapture = await active.driver.request({ type: "captureRenderWindow", request: { target, expected: completionFrame.frame.target, hiDpi: true, includeImage: false } }); }
            catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; staleCapture = { refusedCode: error.code }; }
            assertStory(receipt, "template:precompletion_frame_rejected", ["stale_target_identity", "stale_frame_identity", "capture_frame_identity_stale"].includes(staleCapture.refusedCode ?? staleCapture.snapshot?.error?.code));
            const fresh = await active.captureFrame(target, false); receipt.frames.push(fresh.frame); receipt.effects.push(fresh);
            assertStory(receipt, "template:fresh_frame_observes_completion", fresh.state.promptObservation.completion.receipt.sequence === 1);
          }
          await active.unmount(target);
        }
        break;
      }
      case "actions-popup-activation": {
        const parent = await mount("main.script-list"); const target = await mount("secondary.actions", parent);
        await input(target, ""); await key(target, "down");
        const selected = nodes(await active.query(target, "elements")).find(node => node.selected && node.activatable !== false);
        assertStory(receipt, "selected_production_action", !!selected);
        const result = await active.act(target, { type: "select", semanticId: String(selected!.semanticId ?? selected!.id), submit: true }); receipt.effects.push(result);
        assertStory(receipt, "actual_action_sink_delivery", (result.actionReceipt ?? result.results?.[0]?.actionReceipt)?.effect?.kind === "submissionDelivered");
        await active.inspect(parent); assertStory(receipt, "same_parent_survives_activation", true);
        const windowLifetimes = async () => {
          const diagnosis = await active.diagnose();
          if (diagnosis.operation !== "diagnose" || diagnosis.ok !== true) throw new EvaluationContractError("owned_diagnosis_required");
          return diagnosis.targets.map(value => `${value.windowId}:${value.windowGeneration}`).sort();
        };
        let disabledTargetsExercised = 0;
        for (const fixture of ["secondary.confirm", "dictation.microphone-picker", "secondary.shortcut-recorder", "secondary.footer"]) {
          const host = fixture === "dictation.microphone-picker" ? await mount("dictation.recording") : parent;
          const child = await mount(fixture, host);
          const deferred = await active.probeSafety(child, "deferredDispatch"); receipt.effects.push(deferred);
          for (const assertion of nativeSafetyProbeAssertions(deferred)) assertStory(receipt, `${fixture}:${assertion.id}`, assertion.pass);
          const controls = nodes(await active.query(child, "elements"));
          const semanticId = fixture === "secondary.confirm" ? "button:0:confirm"
            : fixture === "dictation.microphone-picker" ? "choice:1:dictation-mic-row-1"
            : fixture === "secondary.shortcut-recorder" ? "shortcut-cancel-button" : "footer-action:actions";
          assertStory(receipt, `${fixture}:production_control_present`, controls.some(node => node.semanticId === semanticId && !node.actionDisabled));
          const before = await active.inspect(child); const windows = await windowLifetimes();
          const parentBefore = (await active.inspect(host)).fixtureObservation;
          let refused = false;
          try { await active.act(child, { type: "select", semanticId, submit: false }); }
          catch (error) { refused = error instanceof DriverCommandRefused && error.code === "unsupported_command"; }
          const unchanged = await active.inspect(child);
          assertStory(receipt, `${fixture}:selection_only_refused_before_mutation`, refused && unchanged.targetIdentity.dataGeneration === before.targetIdentity.dataGeneration &&
            JSON.stringify(unchanged.fixtureObservation) === JSON.stringify(before.fixtureObservation) && JSON.stringify(await windowLifetimes()) === JSON.stringify(windows));
          if (fixture === "dictation.microphone-picker") assertStory(receipt, "microphone:selection_only_preserves_completion", (await active.inspect(host)).fixtureObservation.microphoneSelectionCount === parentBefore.microphoneSelectionCount);
          for (const disabled of controls.filter(node => node.actionDisabled || node.selectable === false && node.elementType === "button")) {
            disabledTargetsExercised++;
            let disabledRefused = false;
            try { await active.act(child, { type: "select", semanticId: disabled.semanticId, submit: true }); }
            catch (error) { disabledRefused = error instanceof DriverCommandRefused || error instanceof EvaluationContractError; }
            assertStory(receipt, `${fixture}:disabled_${disabled.semanticId}_refused`, disabledRefused && (await active.inspect(child)).targetIdentity.dataGeneration === before.targetIdentity.dataGeneration);
          }
          const expected = (await active.inspect(child)).targetIdentity;
          const staleCommand = { type: "batch", target: child, expected: { ...expected, windowGeneration: expected.windowGeneration + 1 },
            commands: [{ type: "selectBySemanticId", semanticId, submit: true }], options: { stopOnError: true, timeout: 5000 } };
          let staleRefused = false;
          try { await active.driver.request(staleCommand); } catch (error) { staleRefused = error instanceof DriverCommandRefused && error.code === "stale_target_identity"; }
          assertStory(receipt, `${fixture}:stale_selection_no_mutation`, staleRefused && (await active.inspect(child)).targetIdentity.dataGeneration === before.targetIdentity.dataGeneration);
          const activated = await active.act(child, { type: "select", semanticId, submit: true }); receipt.effects.push(activated);
          assertStory(receipt, `${fixture}:explicit_submit_activates`, !["noOp", "refused"].includes(activated.actionReceipt.effect.kind));
          const afterWindows = await windowLifetimes();
          let duplicateRefused = false;
          try { await active.driver.request({ ...staleCommand, expected }); }
          catch (error) { duplicateRefused = error instanceof DriverCommandRefused && ["target_not_mounted", "stale_target_identity", "stale_frame_identity", "stale_window_generation"].includes(error.code); }
          assertStory(receipt, `${fixture}:retained_submit_cannot_execute_twice`, duplicateRefused && JSON.stringify(await windowLifetimes()) === JSON.stringify(afterWindows));
          if (fixture === "dictation.microphone-picker") {
            const completed = (await active.inspect(host)).fixtureObservation;
            assertStory(receipt, "microphone:accepted_selected_row_exactly_once", completed.microphoneSelectionCount === parentBefore.microphoneSelectionCount + 1 && completed.selectedMicrophoneSemanticId === semanticId);
          }
          if (fixture === "dictation.microphone-picker") await active.unmount(host);
        }
        assertStory(receipt, "disabled_target_activation_exercised", disabledTargetsExercised > 0);

        break;
      }
      case "notes-day-roundtrip": {
        await runNotesAcceptance(active, receipt);
        break;
      }
      case "conversation-recovery-stop": {
        const detached = await mount("agent-chat.detached.retryable-failure");
        await active.captureFrame(detached, false);
        const mutation = await fixtureControl(active, detached, { family: "agentChat", operation: "mutateInputBeforePaint", text: "mutation before paint" });
        receipt.effects.push(mutation);
        const observed = mutation.observation;
        assertStory(receipt, "detached:mutation_epoch_precedes_paint", observed.after.dataGeneration > observed.before.dataGeneration && observed.after.frameGeneration === observed.before.frameGeneration && observed.owner.ownedDictation.input === "mutation before paint");
        assertStory(receipt, "detached:prior_completed_frame_rejected_before_paint", (observed.oldCapture.errorCode ?? observed.oldCapture.error?.code) === "capture_frame_identity_stale");
        const current = await active.captureFrame(detached, false); receipt.frames.push(current.frame); receipt.effects.push(current);
        assertStory(receipt, "detached:atomic_frame_observes_mutation", current.state.fixtureObservation.ownedDictation.input === "mutation before paint" && current.frame.target.frameGeneration > observed.before.frameGeneration);
        const deferred = await active.probeSafety(detached, "deferredDispatch"); receipt.effects.push(deferred);
        for (const assertion of nativeSafetyProbeAssertions(deferred)) assertStory(receipt, `detached:${assertion.id}`, assertion.pass);
        await active.unmount(detached);
        await runConversationAcceptance(active, receipt);
        break;
      }
      case "dictation-delivery-refusal": {
        const destination = await mount("main.script-list"); const target = await mount("dictation.recording");
        await fixtureControl(active, target, { family: "dictation", operation: "begin", destination: "mainFilter" });
        await fixtureControl(active, target, { family: "dictation", operation: "recording", text: "owned dictation fixture", bars: [0.1,0.2,0.3,0.4,0.5,0.4,0.3,0.2,0.1] });
        await fixtureControl(active, target, { family: "dictation", operation: "confirm" });
        await fixtureControl(active, target, { family: "dictation", operation: "transcribe" });
        let locked = false;
        try { await fixtureControl(active, target, { family: "dictation", operation: "retarget", destination: "mainFilter" }); }
        catch (error) { locked = error instanceof EvaluationContractError && error.code === "destination_locked"; }
        assertStory(receipt, "processing_destination_locked", locked);
        const delivered = await fixtureControl(active, target, { family: "dictation", operation: "deliver" }); receipt.effects.push(delivered);
        assertStory(receipt, "actual_internal_insertion", (await active.inspect(destination)).inputValue === "owned dictation fixture");
        await fixtureControl(active, target, { family: "dictation", operation: "begin", destination: "mainFilter" });
        await fixtureControl(active, target, { family: "dictation", operation: "recording", text: "stale target fixture", bars: [0.1,0.2,0.3,0.4,0.5,0.4,0.3,0.2,0.1] });
        await fixtureControl(active, target, { family: "dictation", operation: "confirm" });
        await fixtureControl(active, target, { family: "dictation", operation: "transcribe" });
        await active.unmount(destination);
        const reopened = await mount("main.script-list"); const reopenedInput = (await active.inspect(reopened)).inputValue;
        const stale = await fixtureControl(active, target, { family: "dictation", operation: "deliver" }); receipt.effects.push(stale);
        assertStory(receipt, "stale_destination_refused", stale.observation.deliveryOutcome === "staleTarget");
        assertStory(receipt, "reopened_destination_unchanged", (await active.inspect(reopened)).inputValue === reopenedInput);
        await active.unmount(target); await active.unmount(reopened);
        for (const [fixture, destinationKind] of [["agent-chat.standard.populated", "agentChat"], ["day-page.today", "dayPage"]] as const) {
          const destination = await mount(fixture); const overlay = await mount("dictation.recording");
          const before = (await active.inspect(destination)).fixtureObservation;
          const text = "owned exact-window delivery";
          const prepare = async () => {
            await fixtureControl(active, overlay, { family: "dictation", operation: "begin", destination: destinationKind });
            await fixtureControl(active, overlay, { family: "dictation", operation: "recording", text, bars: [0.1,0.2,0.3,0.4,0.5,0.4,0.3,0.2,0.1] });
            await fixtureControl(active, overlay, { family: "dictation", operation: "confirm" });
            await fixtureControl(active, overlay, { family: "dictation", operation: "transcribe" });
          };
          await prepare();
          const delivery = await fixtureControl(active, overlay, { family: "dictation", operation: "deliver" }); receipt.effects.push(delivery);
          const after = (await active.inspect(destination)).fixtureObservation;
          assertStory(receipt, `${destinationKind}:exact_destination_window`, delivery.observation.deliveryOutcome === "delivered" &&
            delivery.delivery.destinationWindow.windowId === destination.id && delivery.delivery.destinationWindow.windowGeneration === destination.generation && destination.id !== overlay.id);
          if (destinationKind === "agentChat") {
            const old = before.ownedDictation; const inserted = after.ownedDictation;
            const characters = [...old.input];
            const expected = [...characters.slice(0, old.selectionStart), ...text, ...characters.slice(old.selectionEnd)].join("");
            assertStory(receipt, "agentChat:actual_text_selection_and_cached_parent", inserted.input === expected && inserted.selectionStart === old.selectionStart + [...text].length &&
              inserted.selectionEnd === inserted.selectionStart && inserted.parentWindowId === destination.id && inserted.parentWindowGeneration === destination.generation);
            const picker = await mount("agent-chat.popup.history", destination); const pickerState = await active.inspect(picker);
            assertStory(receipt, "agentChat:subsequent_picker_retains_main_parent", pickerState.window.parentWindowId === destination.id && pickerState.window.parentWindowGeneration === destination.generation);
            await key(picker, "escape");
            const afterPickerClose = await active.diagnose();
            receipt.effects.push(afterPickerClose);
            assertStory(receipt, "agentChat:escape_retires_exact_picker", !afterPickerClose.targets.some(candidate =>
              candidate.windowId === picker.id && candidate.windowGeneration === picker.generation));
            assertStory(receipt, "agentChat:focus_return_retains_composer", (await active.inspect(destination)).fixtureObservation.ownedDictation.input === expected);
          } else {
            const bytes = Buffer.from(before.inputText); const expected = Buffer.concat([bytes.subarray(0, before.selection.start), Buffer.from(text), bytes.subarray(before.selection.end)]).toString();
            assertStory(receipt, "dayPage:actual_text_selection_and_canonical_save", after.inputText === expected && after.selection.start === before.selection.start + Buffer.byteLength(text) &&
              after.selection.end === after.selection.start && after.saved === true && after.documentPath === before.documentPath && after.canonicalContentFingerprint !== before.canonicalContentFingerprint);
          }
          await prepare(); const replaced = await mount(fixture); const replacement = (await active.inspect(replaced)).fixtureObservation;
          const refused = await fixtureControl(active, overlay, { family: "dictation", operation: "deliver" }); receipt.effects.push(refused);
          const replacementAfterRefusal = (await active.inspect(replaced)).fixtureObservation;
          // Agent Chat also observes the shared Dictation session. A stale-target
          // refusal must report its failed phase while leaving every destination
          // field (including the composer and selection) unchanged.
          const expectedReplacement = destinationKind === "agentChat"
            ? { ...replacement, state: { ...replacement.state, dictationPhase: "failed" } }
            : replacement;
          receipt.effects.push({ destinationKind, replacement, replacementAfterRefusal });
          assertStory(receipt, `${destinationKind}:replaced_destination_refused`, refused.observation.deliveryOutcome === "staleTarget" &&
            JSON.stringify(replacementAfterRefusal) === JSON.stringify(expectedReplacement));
          receipt.effects.push(await captureEvidence(active, replaced, claim));
          await active.unmount(overlay); await active.unmount(replaced);
        }
        break;
      }
      case "theme-publication-revert": {
        const main = await mount("main.script-list"); const notes = await mount("notes.editor");
        const tokenId = "theme.colors.accent.selected";
        const resolvedValue = (result: { resolved: readonly LiveThemeEdit[] }) => {
          const matches = Array.isArray(result.resolved) ? result.resolved.filter(edit => edit.tokenId === tokenId) : [];
          if (matches.length !== 1 || !Number.isSafeInteger(matches[0]!.value)) throw new EvaluationContractError("theme_resolved_token_missing");
          return matches[0]!.value;
        };
        const initial = await active.inspect(main);
        assertStory(receipt, "baseline_resolved_revision_matches_owner", initial.liveTheme?.revision === initial.targetIdentity.themeRevision);
        const baselineValue = resolvedValue({ resolved: initial.liveTheme?.resolved?.values });
        const requestedValue = baselineValue === 0x5b9dff ? 0x72c1a8 : 0x5b9dff;
        const captureSelected = async (phase: string, revision: number) => {
          const frame = await active.frame(main); receipt.frames.push(frame);
          assertStory(receipt, `${phase}:main_applied_revision`, frame.target.themeRevision === revision);
          const snapshot = (await active.capture(main, true)).snapshot;
          const state = await active.inspect(main);
          assertStory(receipt, `${phase}:same_capture_state`, Object.entries(frame.target).every(([key, value]) => state.targetIdentity?.[key] === value));
          const layout = await active.query(main, "layout");
          const { width, height, pngBase64 } = snapshot.capture;
          const selectedRegion = selectedThemePixelRegion(state, layout, { width, height }, frame.target.frameGeneration);
          const pixels = hashPngRegion(Buffer.from(pngBase64, "base64"), selectedRegion.dimensions, selectedRegion.region);
          receipt.effects.push({ kind: "themePixelRegion", phase, frameIdentity: snapshot.frameIdentity, ...selectedRegion, ...pixels });
          assertStory(receipt, `${phase}:opaque_selected_marker`, pixels.opaquePixels === pixels.sampledPixels);
          return { selectedRegion, ...pixels };
        };
        const baseline = await captureSelected("baseline", initial.liveTheme.revision);
        const publication = await active.applyTheme(initial.liveTheme.revision, [{ tokenId, value: requestedValue }]);
        // Assert publication cause before any explicit frame can mask lost delivery.
        assertStory(receipt, "two_family_causal_invalidation", publicationCausalityIssues(publication, [main, notes]).length === 0);
        assertStory(receipt, "requested_token_resolved", resolvedValue(publication) === requestedValue && requestedValue !== baselineValue);
        const edited = await captureSelected("edited", publication.revision);
        const editedRegionSame = JSON.stringify(edited.selectedRegion) === JSON.stringify(baseline.selectedRegion);
        const editChangedPixels = edited.sha256 !== baseline.sha256;
        receipt.effects.push({ kind: "themePixelComparison", phase: "edit", tokenId, baselineValue, resolvedValue: resolvedValue(publication),
          baselineHash: baseline.sha256, observedHash: edited.sha256, sameRegion: editedRegionSame, pixelsChanged: editChangedPixels });
        assertStory(receipt, "edit_preserves_selected_region", editedRegionSame);
        assertStory(receipt, "edit_changes_selected_region_pixels", editChangedPixels);
        receipt.frames.push(await active.frame(notes)); await active.capture(notes, false);
        const reverted = await active.revertTheme(publication.revision);
        assertStory(receipt, "same_lifetime_revert", publicationCausalityIssues(reverted, [main, notes]).length === 0);
        assertStory(receipt, "revert_restores_resolved_token", resolvedValue(reverted) === baselineValue);
        const restored = await captureSelected("reverted", reverted.revision);
        const revertedRegionSame = JSON.stringify(restored.selectedRegion) === JSON.stringify(baseline.selectedRegion);
        const revertRestoredPixels = restored.sha256 === baseline.sha256;
        receipt.effects.push({ kind: "themePixelComparison", phase: "revert", tokenId, baselineValue, resolvedValue: resolvedValue(reverted),
          baselineHash: baseline.sha256, observedHash: restored.sha256, sameRegion: revertedRegionSame, pixelsRestored: revertRestoredPixels });
        assertStory(receipt, "revert_preserves_selected_region", revertedRegionSame);
        assertStory(receipt, "revert_restores_selected_region_pixels", revertRestoredPixels);
        receipt.frames.push(await active.frame(notes)); await active.capture(notes, false);
        await fixtureControl(active, notes, { family: "fault", operation: "suppressThemeNotification", target: notes });
        const negative = await active.applyTheme(reverted.revision, [{ tokenId: "theme.colors.accent.selected", value: 0x72c1a8 }]);
        const detected = publicationCausalityIssues(negative, [main, notes]).length > 0;
        await active.inspect(notes); await active.frame(notes);
        assertStory(receipt, "missing_notification_negative_after_explicit_frame", detected);
        const finalRevert = await active.revertTheme(negative.revision);
        assertStory(receipt, "negative_control_restores_resolved_token", resolvedValue(finalRevert) === baselineValue);
        receipt.effects.push(publication, reverted, negative, finalRevert);
        let wrongThemeTargetRefused = false;
        try { await fixtureControl(active, notes, { family: "theme", operation: "armSaveFailure" }); }
        catch (error) {
          if (!(error instanceof EvaluationContractError || error instanceof DriverCommandRefused)) throw error;
          wrongThemeTargetRefused = error.code === "theme_chooser_fixture_required";
          receipt.effects.push({ kind: "themeFixtureWrongTarget", target: notes, refusedCode: error.code });
        }
        assertStory(receipt, "theme_fault_requires_actual_theme_chooser", wrongThemeTargetRefused);
        const chooser = await mount("main.theme-chooser");
        if ((await active.inspect(chooser)).fixtureObservation?.panelMode === "customize") await key(chooser, "e", ["cmd"]);
        const captureStatus = async (phase: string) => {
          const evidence = await captureEvidence(active, chooser, claim);
          assertStory(receipt, `${phase}:status_image_retained`, evidence.retainedImage === true && typeof evidence.sourceName === "string");
          const captured: Json = JSON.parse(readFileSync(join(claim.artifactsRoot, evidence.sourceName), "utf8"));
          receipt.frames.push(captured.frame);
          const management = captured.state.fixtureObservation;
          const status = nodes(captured.elements).find(node => node.semanticId === "status:theme-chooser-dirty-state");
          assertStory(receipt, `${phase}:actual_theme_status`, management?.family === "themeChooser" && management.panelMode === "preview" &&
            typeof management.status === "string" && status?.text === management.status && status.statusKind === management.statusKind);
          const measured = (captured.layout.components ?? []).filter((component: Json) =>
            component.name === "status:theme-chooser-dirty-state" && component.measurementProvenance === "paint-time");
          assertStory(receipt, `${phase}:status_painted_in_same_frame`, measured.length === 1 && measured[0].coordinateSpace === "window" &&
            measured[0].measurementFrameGeneration === captured.frame.target.frameGeneration);
          const visible = measured[0].visibleBounds;
          const dimensions = { width: captured.snapshot.capture.width, height: captured.snapshot.capture.height };
          const windowDimensions = { width: captured.layout.windowWidth, height: captured.layout.windowHeight };
          assertStory(receipt, `${phase}:status_visible`, !!visible &&
            [visible.x, visible.y, visible.width, visible.height, windowDimensions.width, windowDimensions.height].every(value => typeof value === "number" && Number.isFinite(value)) &&
            visible.x >= 0 && visible.y >= 0 && visible.width > 0 && visible.height > 0 &&
            visible.x + visible.width <= windowDimensions.width && visible.y + visible.height <= windowDimensions.height);
          receipt.effects.push({ kind: "themeStatusPaint", phase, sourceName: evidence.sourceName,
            frame: captured.frame, management, semanticStatus: status, measurement: measured[0] });
          return { management, frame: captured.frame, visible, dimensions, windowDimensions,
            png: Buffer.from(captured.snapshot.capture.pngBase64, "base64") };
        };
        const beforeSave = await captureStatus("before-save-failure");
        const armed = await fixtureControl(active, chooser, { family: "theme", operation: "armSaveFailure" });
        const originalHash = armed.observation.originalFileSha256;
        assertStory(receipt, "real_theme_file_save_obstruction", armed.observation.family === "theme" && armed.observation.path === "theme.json" &&
          armed.observation.blockerPresent === true && (originalHash === null || typeof originalHash === "string" && /^[a-f0-9]{64}$/.test(originalHash)));
        receipt.effects.push(armed);
        await key(chooser, "enter");
        const failedSave = await captureStatus("save-failed");
        assertStory(receipt, "production_save_error_is_visible", failedSave.management.statusKind === "error" && failedSave.management.status.startsWith("Save failed:") &&
          failedSave.management.isDirty === beforeSave.management.isDirty && failedSave.frame.target.themeRevision === beforeSave.frame.target.themeRevision &&
          failedSave.frame.target.dataGeneration > beforeSave.frame.target.dataGeneration);
        assertStory(receipt, "status_comparison_uses_same_canvas", JSON.stringify(beforeSave.dimensions) === JSON.stringify(failedSave.dimensions) &&
          JSON.stringify(beforeSave.windowDimensions) === JSON.stringify(failedSave.windowDimensions));
        const scaleX = beforeSave.dimensions.width / beforeSave.windowDimensions.width;
        const scaleY = beforeSave.dimensions.height / beforeSave.windowDimensions.height;
        const left = Math.max(beforeSave.visible.x, failedSave.visible.x), top = Math.max(beforeSave.visible.y, failedSave.visible.y);
        const right = Math.min(beforeSave.visible.x + beforeSave.visible.width, failedSave.visible.x + failedSave.visible.width);
        const bottom = Math.min(beforeSave.visible.y + beforeSave.visible.height, failedSave.visible.y + failedSave.visible.height);
        const region = { x: Math.ceil(left * scaleX), y: Math.ceil(top * scaleY),
          width: Math.floor(right * scaleX) - Math.ceil(left * scaleX), height: Math.floor(bottom * scaleY) - Math.ceil(top * scaleY) };
        const beforePixels = hashPngRegion(beforeSave.png, beforeSave.dimensions, region);
        const failedPixels = hashPngRegion(failedSave.png, failedSave.dimensions, region);
        assertStory(receipt, "save_error_changes_actual_status_pixels", beforePixels.sha256 !== failedPixels.sha256);
        receipt.effects.push({ kind: "themeSaveFailurePixels", region, before: beforePixels, failed: failedPixels });
        const cleared = await fixtureControl(active, chooser, { family: "theme", operation: "clearSaveFailure" });
        assertStory(receipt, "save_failure_restores_original_file", cleared.observation.blockerPresent === false &&
          cleared.observation.originalFileSha256 === originalHash && cleared.observation.restoredFileSha256 === originalHash);
        const malformed = await fixtureControl(active, chooser, { family: "theme", operation: "malformedReload" });
        const reload = malformed.observation;
        assertStory(receipt, "malformed_reload_retains_last_good_theme", typeof reload.reloadError === "string" && reload.reloadError.length > 0 &&
          Number.isSafeInteger(reload.beforeRevision) && reload.beforeRevision >= 0 && reload.afterRevision === reload.beforeRevision &&
          typeof reload.beforeThemeSha256 === "string" && /^[a-f0-9]{64}$/.test(reload.beforeThemeSha256) && reload.afterThemeSha256 === reload.beforeThemeSha256);
        assertStory(receipt, "malformed_reload_restores_original_bytes", typeof reload.malformedFileSha256 === "string" && /^[a-f0-9]{64}$/.test(reload.malformedFileSha256) &&
          reload.malformedFileSha256 !== originalHash && reload.originalFileSha256 === originalHash && reload.restoredFileSha256 === originalHash && reload.ordinaryRestartProven === false);
        receipt.effects.push(cleared, malformed);
        const unmountBlocker = await fixtureControl(active, chooser, { family: "theme", operation: "armSaveFailure" });
        assertStory(receipt, "theme_obstruction_armed_before_unmount", unmountBlocker.observation.blockerPresent === true);
        await active.unmount(chooser);
        const reopenedChooser = await mount("main.theme-chooser");
        const restoredAfterUnmount = await fixtureControl(active, reopenedChooser, { family: "theme", operation: "malformedReload" });
        assertStory(receipt, "theme_unmount_restores_owned_file", reopenedChooser.generation !== chooser.generation &&
          restoredAfterUnmount.observation.originalFileSha256 === originalHash && restoredAfterUnmount.observation.restoredFileSha256 === originalHash);
        const endBlocker = await fixtureControl(active, reopenedChooser, { family: "theme", operation: "armSaveFailure" });
        assertStory(receipt, "theme_obstruction_enrolled_in_end_cleanup", endBlocker.observation.blockerPresent === true);
        receipt.effects.push(unmountBlocker, restoredAfterUnmount, { kind: "themeEndRestoreRequired", ...endBlocker });
        break;
      }
      case "close-reopen-stale": {
        const old = await mount("main.script-list"); const expected = (await active.inspect(old)).targetIdentity;
        assertStory(receipt, "old_identity_observed", expected?.windowGeneration === old.generation);
        await active.unmount(old); const current = await mount("main.script-list"); const initialInput = (await active.inspect(current)).inputValue;
        assertStory(receipt, "window_lifetime_changed", old.id === current.id && old.generation !== current.generation);
        let refused = false;
        try { await active.driver.request({ type: "simulateGpuiEvent", target: old, expected, event: { type: "keyDown", key: "x", text: "stale" } }); }
        catch (error) { refused = error instanceof DriverCommandRefused && error.code === "stale_window_generation"; }
        assertStory(receipt, "old_event_refused", refused);
        assertStory(receipt, "new_root_not_mutated", (await active.inspect(current)).inputValue === initialInput);
        await key(current, "down"); break;
      }
    }
    assertStory(receipt, "nonempty_behavior_and_frames", receipt.assertions.length > 0 && receipt.frames.length > 0 && receipt.effects.length > 0);
    // Production actions may retire secondary windows without an explicit
    // evaluator unmount; choose only from the authoritative live inventory.
    await active.diagnose();
    const last = active.targets.at(-1);
    if (last) {
      const target: AutomationInstance = { type: "instance", id: last.windowId, generation: last.windowGeneration };
      receipt.frames.push(await active.frame(target)); receipt.effects.push(await captureEvidence(active, target, claim));
    }
    receipt.pass = true;
  } catch (error) {
    receipt.error = error instanceof EvaluationContractError || error instanceof DriverCommandRefused ? error.code : "runtime_journey_failed";
    if (error instanceof DriverLifecycleError) receipt.cleanup = error.cleanup;
  } finally {
    if (client) { try { receipt.cleanup = await client.close(); } catch { receipt.cleanup = client.cleanup; receipt.pass = false; } }
    if (!receipt.cleanup.closed) receipt.pass = false;
  }
  return receipt;
}

export async function runFamilyFixture(fixture: FixtureDescriptor, reference: ArtifactReference, claim: OutputClaim): Promise<RuntimeJourneyReceipt> {
  const receipt: RuntimeJourneyReceipt = { id: fixture.id, proofLevel: "owned-production-runtime", fixtureIds: [fixture.id], assertions: [], frames: [], effects: [], pass: false, cleanup: unknownOwnedCleanup(false) };
  let client: OwnedEvaluationClient | undefined;
  try {
    client = await OwnedEvaluationClient.launch(REPOSITORY_ROOT, reference, claim, fixture.parentFixtureId ? [fixture.parentFixtureId, fixture.id] : [fixture.id]);
    const parent = fixture.parentFixtureId ? await client.mount(fixture.parentFixtureId) : undefined;
    const target = await client.mount(fixture.id, parent);
    receipt.binding = { descriptor: fixture, parent };
    const evidence = await captureEvidence(client, target, claim);
    receipt.frames.push(evidence.frame); receipt.effects.push(evidence);
    const bindingIssues = fixtureEvidenceIssues(receipt.binding, evidence);
    evidence.issues = bindingIssues;
    assertStory(receipt, "fixture_route_presentation_controls_bound", bindingIssues.length === 0);
    assertStory(receipt, "actual_hidden_root", evidence.state.windowVisible === false);
    assertStory(receipt, "actual_production_semantics", nodes(evidence.elements).length > 0);
    assertStory(receipt, "qualified_framebuffer", true); receipt.pass = true;
  } catch (error) {
    receipt.error = error instanceof EvaluationContractError || error instanceof DriverCommandRefused ? error.code : "family_fixture_failed";
    if (error instanceof DriverLifecycleError) receipt.cleanup = error.cleanup;
  } finally { if (client) { try { receipt.cleanup = await client.close(); } catch { receipt.cleanup = client.cleanup; receipt.pass = false; } } }
  return receipt;
}
export async function discoverFixtures(reference: ArtifactReference, claim: OutputClaim): Promise<{ fixtures: readonly FixtureDescriptor[]; cleanup: OwnedCleanup }> {
  const client = await OwnedEvaluationClient.launch(REPOSITORY_ROOT, reference, claim, []);
  try { const fixtures = (await client.discover()).fixtures; return { fixtures, cleanup: await client.close() }; }
  finally { await client.close(); }
}
export function latencySummary(samples: readonly { frameMs: number; readbackMs: number }[]) {
  if (samples.length !== 30 || samples.some(sample => !Number.isFinite(sample.frameMs) || !Number.isFinite(sample.readbackMs))) throw new EvaluationContractError("thirty_measured_edits_required");
  const p95 = (values: number[]) => values.sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1]!;
  const frameP95Ms = p95(samples.map(sample => sample.frameMs)); const readbackP95Ms = p95(samples.map(sample => sample.readbackMs));
  return { samples: 30, warmups: 5, clock: "parent-monotonic", frameP95Ms, readbackP95Ms, frameBudgetMs: 100, readbackBudgetMs: 250, pass: frameP95Ms <= 100 && readbackP95Ms <= 250 };
}
export async function measureLiveEdits(client: OwnedEvaluationClient, target: AutomationInstance) {
  let revision = (await client.inspect(target)).targetIdentity.themeRevision;
  const samples: Array<{ frameMs: number; readbackMs: number; frame: CompletedFrameIdentity; readbackFrame: CompletedFrameIdentity; childPhaseDurationsMs: Readonly<Record<string, number>> }> = [];
  for (let index = 0; index < 35; index++) {
    const start = performance.now(); const publication = await client.applyTheme(revision, [{ tokenId: "theme.colors.accent.selected", value: index % 2 ? 0x5b9dff : 0x72c1a8 }]);
    revision = publication.revision;
    if (publicationCausalityIssues(publication, [target]).length) throw new EvaluationContractError("latency_publication_not_causal");
    const frame = await client.frame(target); const frameMs = performance.now() - start;
    if (frame.target.themeRevision !== revision) throw new EvaluationContractError("latency_frame_revision_mismatch");
    const captured = await client.capture(target, false); const readbackMs = performance.now() - start;
    if (index >= 5) samples.push({ frameMs, readbackMs, frame, readbackFrame: captured.snapshot.frameIdentity, childPhaseDurationsMs: { ...publication.phaseDurationsMs,
      ...client.lastFramePhaseDurationsMs, ...(captured.snapshot?.phaseDurationsMs ?? {}) } });
  }
  await client.revertTheme(revision); return { ...latencySummary(samples), observations: samples };
}
export function readOwnedEditDocument(claim: OutputClaim, path: string): readonly LiveThemeEdit[] {
  const resolved = resolve(path); const rel = relative(claim.root, resolved);
  if (rel.startsWith("..") || rel.startsWith("/") || !rel || realpathSync(resolved) !== resolved) throw new EvaluationContractError("edit_file_outside_owned_output");
  const fd = openSync(resolved, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile() || stat.nlink !== 1 || stat.size > 16384 || stat.uid !== process.getuid?.()) throw new EvaluationContractError("unsafe_edit_document");
    const bytes = Buffer.alloc(stat.size); if (readSync(fd, bytes, 0, bytes.length, 0) !== bytes.length) throw new EvaluationContractError("edit_document_changed");
    const document = JSON.parse(bytes.toString("utf8"));
    if (!document || Object.keys(document).join(",") !== "edits") throw new EvaluationContractError("invalid_edit_document");
    return validateThemeEdits(document.edits);
  } finally { closeSync(fd); }
}
export async function watchLiveEdits(client: OwnedEvaluationClient, claim: OutputClaim, path: string, target: AutomationInstance, signal: AbortSignal): Promise<Json> {
  let pending = true; let wake: (() => void) | undefined; let revision = (await client.inspect(target)).targetIdentity.themeRevision; let lastDigest = "";
  const publications: Json[] = []; const failures: string[] = [];
  const watcher = watch(claim.root, () => { pending = true; wake?.(); }); const abort = () => wake?.(); signal.addEventListener("abort", abort);
  try {
    while (!signal.aborted) {
      if (!pending) await new Promise<void>(resolveWake => { wake = resolveWake; });
      if (signal.aborted) break; pending = false;
      try {
        const edits = readOwnedEditDocument(claim, path); const digest = createHash("sha256").update(JSON.stringify(edits)).digest("hex");
        if (digest === lastDigest) continue;
        const start = performance.now(); const publication = await client.applyTheme(revision, edits); revision = publication.revision; lastDigest = digest;
        if (publicationCausalityIssues(publication, [target]).length) throw new EvaluationContractError("watch_publication_not_causal");
        const frame = await client.frame(target);
        if (frame.target.themeRevision !== revision) throw new EvaluationContractError("watch_frame_revision_mismatch");
        const observation = { revision, frame, elapsedMs: performance.now() - start }; publications.push(observation); console.log(JSON.stringify({ type: "designWatchPublication", ...observation }));
      } catch (error) {
        if (error instanceof EvaluationContractError && ["watch_publication_not_causal", "watch_frame_revision_mismatch"].includes(error.code)) throw error;
        const code = error instanceof EvaluationContractError ? error.code : "edit_document_or_publication_failed";
        if (failures.length < 128) failures.push(code); console.log(JSON.stringify({ type: "designWatchRefusal", code, retainedRevision: revision }));
      }
    }
  } finally { watcher.close(); signal.removeEventListener("abort", abort); }
  if (publications.length) await client.revertTheme(revision);
  return { publications, failures, lastGoodRevision: revision };
}

export function commitOwnedReport(claim: OutputClaim, receipt: Json, cleanup: OwnedCleanup): Json {
  writeJsonArtifactAtomic(claim, "observation.json", ownedObservationDocument(claim, receipt));
  const specs: ArtifactSpec[] = [OBSERVATION_SPEC, ...(retainedCaptureSpecs.get(claim) ?? []), ...searchShardArtifactSpecs(claim)];
  const artifacts = specs.map(spec => validateArtifact(join(claim.artifactsRoot, spec.sourceName), spec, claim.artifactsRoot));
  const artifactLifecycle = buildArtifactLifecycle({ claim, finalizationKind: "driver-close", writersFinalized: cleanup.closed, specs, artifacts });
  const compact = compactOwnedReceipt(claim, receipt, artifactLifecycle);
  resolveReceiptDetails(compact);
  commitFinalReceipt(claim, compact, specs, artifacts);
  return compact;
}

    const providerStates = ["awaiting-admission", "reading", "held", "released", "delivered", "completed", "failed", "unavailable", "stale-discarded", "cancelled"];
    const publicationPolicies = ["visible", "cache-only", "visible-synchronous"];
    const providerOutcomes = ["success", "empty", "error", "unavailable", "disconnected"];
let searchScheduleIds: Set<string> | undefined;
let searchOrderLabels: Map<string, readonly string[]> | undefined;
function compiledSearchOrderLabels(): ReadonlyMap<string, readonly string[]> {
  if (!searchOrderLabels) {
    const groups = new Map<string, string[]>();
    for (const schedule of searchContractSpec().schedules) {
      const group = searchScheduleComparisonGroup(schedule);
      if (!group || schedule.structuralNotApplicable) continue;
      const orders = groups.get(group) ?? [];
      orders.push(schedule.recipe.kind === "same-turn" ? "same-turn" : schedule.recipe.kind === "cohort" ? schedule.recipe.order.join("-then-") : schedule.providers.join("-then-"));
      groups.set(group, orders);
    }
    searchOrderLabels = new Map();
    for (const [group, orders] of groups) for (const intent of ["automatic", "deliberate-when-eligible"])
      searchOrderLabels.set(`${group}:${intent}`, orders);
  }
  return searchOrderLabels;
}
export function annotateOwnedEvidence(value: unknown, key = ""): unknown {
  const contentKind = inferredKindForKey(key);
  if (Array.isArray(value)) {
    const entries = value.map(entry => annotateOwnedEvidence(entry, key));
    return contentKind ? classifyReceiptContent(contentKind, entries) : entries;
  }
  if (value && typeof value === "object") {
    const record = value as Json;
    const providerRun = ["runs", "run", "provider", "providerRuns", "terminal", "admission", "reads"].includes(key) && Number.isSafeInteger(record.id) && record.id > 0 &&
      SEARCH_PROVIDERS.some(source => source === record.source) && typeof record.query === "string" &&
      ["worker", "sourceChange", "synchronousRead"].includes(record.kind) && Number.isSafeInteger(record.generation) && record.generation >= 0 && providerStates.includes(record.state) &&
      (record.kind === "sourceChange" ? record.publicationPolicy === null && record.outcome == null : publicationPolicies.includes(record.publicationPolicy) && (record.outcome == null || providerOutcomes.includes(record.outcome)));
    const frameTarget = record.frame?.target;
    const frameIdentity = frameTarget && record.frame?.requestedTarget?.type === "instance" &&
      record.frame.requestedTarget.id === frameTarget.windowId && record.frame.requestedTarget.generation === frameTarget.windowGeneration &&
      typeof frameTarget.windowId === "string" && frameTarget.windowId.length > 0 &&
      Number.isSafeInteger(frameTarget.windowGeneration) && frameTarget.windowGeneration > 0 &&
      Number.isSafeInteger(frameTarget.frameGeneration) && frameTarget.frameGeneration > 0;
    const frameStamp = (key === "frameEvidence" || key === "completedFrames" || key === "facts") && frameIdentity &&
      ["scheduled", "forced"].includes(record.mode) &&
      ((record.mode === "forced" && record.scheduledCapability === false) ||
        (Number.isSafeInteger(record.invalidationEpoch) && record.invalidationEpoch >= 0 &&
         Number.isSafeInteger(record.notificationEpoch) && record.notificationEpoch >= 0 &&
         typeof record.localInputFocused === "boolean" && typeof record.nativeWindowActive === "boolean"));
    const captureMetadata = key === "capture" && record.mimeType === "image/png" && record.hiDpi === true &&
      Number.isSafeInteger(record.width) && record.width > 0 && Number.isSafeInteger(record.height) && record.height > 0 &&
      record.width * record.height <= OWNED_EVALUATION_LIMITS.maxImagePixels &&
      Number.isSafeInteger(record.byteLength) && record.byteLength > 0 && record.byteLength <= OWNED_EVALUATION_LIMITS.maxPngBytes &&
      typeof record.sha256 === "string" && /^[a-f0-9]{64}$/.test(record.sha256);
    const copyReceipt = key === "receipt" && record.destination === "ownedProcessLocal" &&
      Number.isSafeInteger(record.byteLength) && record.byteLength >= 0 && Number.isSafeInteger(record.revision) && record.revision > 0 &&
      typeof record.sha256 === "string" && /^[a-f0-9]{64}$/.test(record.sha256);
    const captureHistoryCapability = key === "captureHistoryBundle" && record.version === 1 && record.requiresFrameCursor === true &&
      record.pageScope === "captureBundle" && record.decodedScope === "complete";
    const searchReference = Number.isSafeInteger(record.shard) && record.shard >= 0 && record.artifactId === `search-shard-${record.shard}` &&
      ((typeof record.scheduleId === "string" && (searchScheduleIds ??= new Set(searchContractSpec().schedules.map(schedule => schedule.id))).has(record.scheduleId) &&
        Object.keys(record).every(field => ["artifactId", "shard", "scheduleId"].includes(field))) ||
       (Array.isArray(record.scheduleIds) && record.scheduleIds.length > 0 &&
        record.scheduleIds.every((id: unknown) => typeof id === "string" && (searchScheduleIds ??= new Set(searchContractSpec().schedules.map(schedule => schedule.id))).has(id)) &&
        Object.keys(record).every(field => ["artifactId", "shard", "scheduleIds"].includes(field))));
    const expectedOrders = key === "orderComparisons" ? compiledSearchOrderLabels().get(record.key) : undefined;
    const searchComparison = expectedOrders && Object.keys(record).length === 4 && typeof record.fingerprint === "string" &&
      /^[a-f0-9]{64}$/.test(record.fingerprint) && expectedOrders.includes(record.order) && Array.isArray(record.expectedOrders) &&
      record.expectedOrders.length === expectedOrders.length && expectedOrders.every(order => record.expectedOrders.includes(order));
    const terminalSelection = key === "terminalReceipts" && SEARCH_PROVIDERS.includes(record.source) &&
      ["error", "unavailable", "disconnect"].includes(record.requestedOutcome) && ["automatic", "explicitAnchor"].includes(record.intent) &&
      typeof record.selectionArmed === "boolean" && record.provider?.source === record.source &&
      record.query && [record.query.lifetime, record.query.revision, record.query.scopeRevision].every(value => Number.isSafeInteger(value) && value >= 0) &&
      typeof record.selectedSemanticId === "string" && /^main-list-row:v2:[a-f0-9]{64}$/.test(record.selectedSemanticId);
    const providerOwner = key === "owner" && SEARCH_PROVIDERS.includes(record.source) && Number.isSafeInteger(record.generation) && record.generation >= 0 &&
      typeof record.workQuery === "string" && typeof record.workScope === "string" && typeof record.queryBound === "boolean" &&
      publicationPolicies.includes(record.publicationPolicy) &&
      (record.consumer === null || record.consumer && [record.consumer.lifetime, record.consumer.revision, record.consumer.scopeRevision].every(value => Number.isSafeInteger(value) && value >= 0)) &&
      (record.terminal === null || ["success", "empty", "failed", "unavailable", "disconnected", "cancelled", "staleDiscarded"].includes(record.terminal));
    const providerDesired = key === "desired" && SEARCH_PROVIDERS.includes(record.source) &&
      typeof record.workQuery === "string" && typeof record.workScope === "string" && publicationPolicies.includes(record.publicationPolicy) &&
      record.query && [record.query.lifetime, record.query.revision, record.query.scopeRevision].every(value => Number.isSafeInteger(value) && value >= 0);
    const providerWait = record.version === 1 && SEARCH_PROVIDERS.includes(record.source) &&
      record.query && [record.query.lifetime, record.query.revision, record.query.scopeRevision].every(value => Number.isSafeInteger(value) && value >= 0) &&
      Number.isSafeInteger(record.afterRunId) && record.afterRunId >= 0 && typeof record.pendingDesired === "boolean" && Array.isArray(record.blockers) &&
      ((record.status === "admitted" && record.availabilityReason === "heldCurrentRun") ||
       (record.status === "blocked" && record.availabilityReason === "pendingReplacement") ||
       (record.status === "settled" && ["success", "empty", "error", "unavailable", "disconnected"].includes(record.availabilityReason)) ||
       (record.status === "cached" && record.afterRunId === 0 && record.owner === null && record.run === null &&
        record.blockers.length === 0 && record.cache?.source === record.source && record.availabilityReason === "sourceCacheReuse"));
    const fileSearchStream = (key === "fileSearchStream" || key === "stream") && isOwnedFileSearchStreamObservation(record);
    const fileSearchPreview = (key === "fileSearchPreview" || key === "pendingPreviewCompletions") && isOwnedFileSearchPreviewObservation(record);
    const entries = Object.fromEntries(Object.entries(value).map(([field, entry]) => {
      if (fileSearchStream) {
        if (field === "phase") return [field, productStatic(entry)];
        if (field === "query") return [field, userContent(entry)];
        if (field === "directory") return [field, entry === null ? null : filePath(entry)];
        if (field === "failure") return [field, entry === null ? null : diagnostic(entry)];
      }
      if (fileSearchPreview) {
        if (field === "phase") return [field, productStatic(entry)];
        if (field === "query") return [field, userContent(entry)];
        if (field === "path") return [field, filePath(entry)];
      }
      const staticProviderField = providerRun && (["state", "publicationPolicy", "outcome", "kind"].includes(field) ||
        (field === "capabilityRefusal" && entry === "synchronous_source_has_no_worker") ||
        (field === "plannedResponse" && providerOutcomes.includes(String(entry))));
      const staticFrameField = frameStamp && field === "mode";
      const staticCaptureField = captureMetadata && (field === "mimeType" || field === "sha256");
      const staticCopyField = copyReceipt && (field === "destination" || field === "sha256");
      const staticSearchReference = searchReference && (field === "artifactId" || field === "scheduleId");
      const staticSearchComparison = searchComparison && ["key", "order", "expectedOrders"].includes(field);
      const staticTerminalSelection = terminalSelection && field === "selectedSemanticId";
      const staticProviderOwner = providerOwner && ["terminal", "publicationPolicy"].includes(field);
      const staticProviderDesired = providerDesired && field === "publicationPolicy";
      const staticProviderWait = providerWait && field === "availabilityReason";
      const staticCaptureHistory = captureHistoryCapability && (field === "pageScope" || field === "decodedScope");
      return [field, staticProviderField || staticFrameField || staticCaptureField || staticCopyField || staticSearchReference || staticSearchComparison || staticTerminalSelection || staticProviderOwner || staticProviderDesired || staticProviderWait || staticCaptureHistory ? productStatic(entry) : annotateOwnedEvidence(entry, field)];
    }));
    return contentKind ? classifyReceiptContent(contentKind, entries) : entries;
  }
  if (typeof value !== "string") return value;
  if (key === "caseId" && SEARCH_CASES.some(item => item.id === value)) return productStatic(value);
  if (key === "caseSetHash" && value === searchContractSpec().caseSetHash) return productStatic(value);
  if ((key === "required" || key === "proved") && SEARCH_CASES.some(item => item.assertions.includes(value))) return productStatic(value);
  if (["intent", "requestedIntent", "required"].includes(key) && ["automatic", "explicitAnchor"].includes(value)) return productStatic(value);
  if (["requestedOutcome", "outcome"].includes(key) && ["error", "unavailable", "disconnect"].includes(value)) return productStatic(value);
  if (key === "scheduleIds" && (searchScheduleIds ??= new Set(searchContractSpec().schedules.map(schedule => schedule.id))).has(value)) return productStatic(value);
  if (contentKind) return classifyReceiptContent(contentKind, value);
  if (key === "clock" && value === "parent-monotonic") return productStatic(value);
  if (key === "terminalFallbackCompletionKind" && value === "synchronousRefusal") return productStatic(value);
  if (/(?:path|cwd|home)$/i.test(key)) return filePath(value);
  const metadata: Record<string, true> = { id: true, type: true, kind: true, operation: true, family: true, owner: true, scenario: true,
    fixtureId: true, fixtureIds: true, status: true, code: true, error: true, errorCode: true, probe: true, shutdownReason: true, comparability: true, issues: true, names: true, name: true, sourceName: true,
    proofLevel: true, proofLevels: true, source: true, scope: true, limitation: true, requestId: true, operationId: true,
    windowId: true, appViewVariant: true, processStartTime: true, processInstanceId: true, sessionGeneration: true,
    binarySha256: true, manifestSha256: true, tokenId: true, cause: true, observation: true, failureCodes: true, identity: true };
  for (const field of ["root", "parentFixtureId", "factoryOwners", "presentationOwner", "surfaceVariant", "expectedSemanticSurface", "requiredSemanticIds", "semanticId", "semanticSurface", "parentWindowId", "shellOwner", "inputOwner", "themeOwner", "rowPrimitive", "surfaceKind"]) metadata[field] = true;
  return metadata[key] ? productStatic(value) : userContent(value);
}

export async function runDesign(argv: string[]): Promise<void> {
  const command = argv[0] ?? "discover";
  const arg = (name: string) => { const index = argv.indexOf(name); return index < 0 ? undefined : argv[index + 1]; };
  if (command === "diagnose" && arg("--receipt")) { console.log(JSON.stringify({ historicalValidation: validateReceiptFile("devtools.design.run", arg("--receipt")!), freshRuntimeProof: false })); return; }
  if ((command === "discover" || command === "spec") && arg("--scenario") === "launcher-ranking-provider") {
    console.log(JSON.stringify(searchContractSpec())); return;
  }
  if (argv.includes("--help")) { console.log("design <discover|inspect|query|act|wait|diagnose|loop|run|watch> --artifact <reference.json> --out <fresh-dir> [--scenario <journey|production-family-matrix|live-edit-latency|native-safety-negatives|native-lifecycle-negatives>] [--fixture <id>]\nSearch: design spec --scenario launcher-ranking-provider is passive; run accepts --search-case <case-id> --search-shard <zero-based-index>. Unrequested schedules remain uncovered.\nloop reads existing Message JSONL. watch --edits <out>/edits.json reads {edits:[{tokenId,value}]}; SIGINT ends and reverts."); return; }
  if (!arg("--artifact") || !arg("--out")) throw new EvaluationContractError("artifact_reference_and_fresh_output_required");
  const reference = readArtifactReference(arg("--artifact")!); const claim = createEvaluationClaim(arg("--out")!, "devtools.design");
  const task = beginManagedTask(claim, "evidence-run", [reference]); const startedAt = new Date().toISOString(); const start = performance.now();
  const cleanups: OwnedCleanup[] = []; let client: OwnedEvaluationClient | undefined; let observation: Json = {}; let error: string | undefined; let disposition: string | undefined;
  try {
    if (command === "run" && (CORE_JOURNEYS as readonly string[]).includes(arg("--scenario") ?? "")) {
      const journey = await runRuntimeJourney(arg("--scenario") as CoreJourneyId, reference, claim, {
        caseId: arg("--search-case"), shard: arg("--search-shard") === undefined ? undefined : Number(arg("--search-shard")),
      }); observation = { journeys: [journey], assertions: journey.assertions }; cleanups.push(journey.cleanup);
      if (!journey.pass) throw new EvaluationContractError(journey.error ?? "journey_failed");
    } else if (command === "run" && arg("--scenario") === "sdk-prompt-roundtrip") {
      const journey = await runSdkJourney(reference, claim); observation = { journeys: [journey], assertions: journey.assertions }; cleanups.push(journey.cleanup);
      if (!journey.pass) throw new EvaluationContractError(journey.error ?? "sdk_journey_failed");
    } else if (command === "run" && arg("--scenario") === FOOTER_JOURNEY_ID) {
      const journey = await runFooterOwnershipJourney(reference, claim); observation = { journeys: [journey], assertions: journey.assertions }; cleanups.push(journey.cleanup);
      if (!journey.pass) throw new EvaluationContractError(journey.error ?? "footer_journey_failed");
    } else if (command === "run" && arg("--scenario") === "production-family-matrix") {
      const catalogue = await discoverFixtures(reference, claim); cleanups.push(catalogue.cleanup);
      if (!catalogue.fixtures.length) throw new EvaluationContractError("empty_production_fixture_catalog");
      const fixtures: RuntimeJourneyReceipt[] = [];
      for (const descriptor of catalogue.fixtures) {
        const fixture = await runFamilyFixture(descriptor, reference, claim); fixtures.push(fixture); cleanups.push(fixture.cleanup);
      }
      observation = { scenario: "production-family-matrix", catalogue: catalogue.fixtures, fixtures, assertions: fixtures.map(fixture => ({ id: fixture.id, pass: fixture.pass })) };
      if (fixtures.some(fixture => !fixture.pass)) throw new EvaluationContractError("production_family_campaign_failed");
    } else if (command === "run" && arg("--scenario") === "native-lifecycle-negatives") {
      const campaign = await runNativeLifecycleCampaign(REPOSITORY_ROOT, reference, claim);
      cleanups.push(...campaign.cleanups); observation = { negativeOnly: true, productionEvidence: false, cases: campaign.observations, assertions: campaign.assertions };
      if (campaign.assertions.some(assertion => !assertion.pass)) throw new EvaluationContractError("native_lifecycle_negative_failed");
    } else {
      const catalogue = await discoverFixtures(reference, claim); cleanups.push(catalogue.cleanup);
      writeJsonArtifactAtomic(claim, "fixture-catalogue.json", { schemaVersion: 1, fixtures: catalogue.fixtures });
      client = await OwnedEvaluationClient.launch(REPOSITORY_ROOT, reference, claim, catalogue.fixtures.map(fixture => fixture.id));
      if (command === "discover") observation = await client.discover();
      else if (command === "diagnose") observation = await client.diagnose();
      else if (command === "loop") {
        const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
        try { for await (const line of input) {
          if (!line.trim()) continue; if (Buffer.byteLength(line) > 16384) throw new EvaluationContractError("stdin_line_too_long");
          const message = JSON.parse(line); const response = await client.driver.request(message); console.log(JSON.stringify(response));
          if (message.type === "design" && message.command?.operation === "end") {
            if (response.result?.ownedWindowsClosed !== true || response.result?.remainingWindows !== 0)
              throw new EvaluationContractError("native_end_not_closed");
            await client.driver.awaitNativeLifecycle(); break;
          }
        } } finally { input.close(); }
        observation = { transport: "existing-message-jsonl", requests: client.driver.stats.requestsSent };
      } else {
        const target = await client.mount(arg("--fixture") ?? "main.script-list");
        if (command === "inspect") observation = await client.inspect(target);
        else if (command === "query") {
          if (arg("--facet") !== "frame") await client.frame(target);
          observation = arg("--facet") === "frame" && argv.includes("--image") ? await captureEvidence(client, target, claim) :
            await client.query(target, (arg("--facet") ?? "elements") as "elements" | "layout" | "text" | "frame");
        }
        else if (command === "act") { if (!arg("--action")) throw new EvaluationContractError("action_json_required"); observation = await client.act(target, JSON.parse(arg("--action")!)); }
        else if (command === "wait") observation = await client.wait(target, JSON.parse(arg("--condition") ?? '{"type":"completedFrame","afterFrameGeneration":0}'));
        else if (command === "run" && arg("--scenario") === "live-edit-latency") { observation = await measureLiveEdits(client, target); if (observation.pass !== true) throw new EvaluationContractError("live_edit_latency_budget_missed"); }
        else if (command === "run" && arg("--scenario") === "native-safety-negatives") {
          observation = await runNativeSafetyProbes(client, target);
          if (observation.assertions.some((assertion: StoryAssertion) => !assertion.pass)) throw new EvaluationContractError("native_safety_negative_failed");
        }
        else if (command === "watch") {
          const abort = new AbortController(); const stop = () => abort.abort(); process.once("SIGINT", stop); process.once("SIGTERM", stop); const timer = setTimeout(stop, 540000);
          try { observation = await watchLiveEdits(client, claim, arg("--edits") ?? join(claim.root, "edits.json"), target, abort.signal); }
          finally { clearTimeout(timer); process.off("SIGINT", stop); process.off("SIGTERM", stop); }
        } else throw new EvaluationContractError("unknown_design_operation");
      }
    }
  } catch (cause) {
    error = cause instanceof EvaluationContractError || cause instanceof DriverCommandRefused ? cause.code : "design_operation_failed";
    if (error === "search-contract-uncovered") disposition = "BLOCKED_MISSING_PRIMITIVE";
    if (cause instanceof ArtifactVerificationError) disposition = cause.disposition;
    if (cause instanceof DriverLifecycleError) cleanups.push(cause.cleanup);
  } finally { if (client) { try { cleanups.push(await client.close()); } catch { cleanups.push(client.cleanup); } } }
  let cleanup = aggregateCleanup(cleanups);
  try { cleanup = finalizeManagedTask(task, cleanup).cleanup; } catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "evidence_finalization_failed"] }; }
  if (!cleanup.closed) disposition = "INVALID_CLEANUP";
  const passive = command === "discover" || command === "diagnose";
  const negative = command === "run" && ["native-safety-negatives", "native-lifecycle-negatives"].includes(arg("--scenario") ?? "");
  const prepared = prepareValidatedReceipt("devtools.design.run", { schemaVersion: RECEIPT_SCHEMA_VERSION, tool: "script-kit-devtools.design", command: `design.${command}`,
    classification: disposition ? disposition.toLowerCase().replaceAll("_", "-") : error ? "reproduced" : "ok", disposition,
    startedAt, durationMs: performance.now() - start, artifactReference: reference, fixture: { kind: "owned-synthetic" },
    evidenceClass: passive ? "STATIC_INVENTORY" : "DIRECT_RUNTIME_PROOF", provesRuntimeBehavior: !passive && !error,
    proofLevel: passive ? "catalogue-only" : negative ? "owned-native-negative-controls" : "owned-production-runtime", nativeExclusions: productStatic(["WindowServer", "AppKit material/glyph pixels", "native focus", "OS IME", "global input", "live providers/devices"]),
    observation: annotateOwnedEvidence(observation), assertions: observation.assertions ?? [{ id: "operation_completed", pass: !error && cleanup.closed }],
    output: filePath(claim.root), cleanup, errors: diagnostic(error ? [error] : []),
  });
  const compact = commitOwnedReport(claim, prepared.receipt, cleanup); console.log(JSON.stringify(compact)); process.exitCode = prepared.exitCode;
}
if (import.meta.main) await runDesign(Bun.argv.slice(2));
