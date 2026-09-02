import { describe, expect, test } from "bun:test";
import { familyCampaignIssues, fixtureEvidenceIssues, type FixtureBinding } from "./fixture-contract.ts";
import type { Json } from "../driver.ts";

function fixtureCase(microphone = false): { binding: FixtureBinding; evidence: Json } {
  const id = microphone ? "dictation.microphone-picker" : "main.file-search-mini";
  const identity = { windowId: microphone ? "dictation-microphone-popup" : "main", windowGeneration: 8,
    appViewVariant: microphone ? "dictationMicrophonePopup" : "FileSearchView", targetGeneration: 1,
    surfaceGeneration: 2, dataGeneration: 3, presentationRevision: 4, themeRevision: 5, frameGeneration: 6 };
  const semanticSurface = microphone ? "dictationMicrophonePopup" : "fileSearch";
  const binding: FixtureBinding = { descriptor: { id, family: microphone ? "dictationMicrophone" : "main", root: microphone ? "secondary" : "main",
    owner: "src/fixture-owner.rs", factoryOwners: ["src/fixture-owner.rs"], proofBoundary: "owned-production-runtime", nativeExclusions: [],
    presentationOwner: "src/production-renderer.rs", appViewVariant: microphone ? undefined : "FileSearchView",
    surfaceVariant: microphone ? undefined : "FileSearchMini", parentFixtureId: microphone ? "dictation.recording" : undefined,
    expectedSemanticSurface: semanticSurface, requiredSemanticIds: microphone ? ["choice:0:dictation-mic-row-0"] : ["input:file-search-input", "list:file-results"] },
    parent: microphone ? { type: "instance", id: "dictation", generation: 7 } : undefined };
  const window = { id: identity.windowId, generation: identity.windowGeneration, visible: false, focused: false, semanticSurface,
    parentWindowId: microphone ? "dictation" : undefined, parentWindowGeneration: microphone ? 7 : undefined };
  return { binding, evidence: { frame: { target: identity }, capture: { width: 400, height: 300 },
    frameIdentity: { target: identity }, status: "captured", source: "gpuiRenderReadback", scope: "liveAutomationWindowRenderReadback",
    state: { targetIdentity: identity, window, windowVisible: false, surfaceContract: { surfaceKind: "FileSearchMini", presentation: {
      shellOwner: "main", inputOwner: "InputState", themeOwner: "Theme", rowPrimitive: "unifiedListItem" } } },
    elements: { targetIdentity: identity, semanticSurface, elements: binding.descriptor.requiredSemanticIds.map(semanticId => ({ semanticId })) },
    layout: { targetIdentity: identity, components: [{ bounds: { x: 0, y: 0, width: 400, height: 300 } }] } } };
}

describe("production fixture binding", () => {
  test("qualifies coherent FileSearchMini and exact microphone child evidence", () => {
    for (const microphone of [false, true]) { const { binding, evidence } = fixtureCase(microphone); expect(fixtureEvidenceIssues(binding, evidence)).toEqual([]); }
  });
  test("ScriptList cannot stand in for FileSearchMini even with passing flags", () => {
    const { binding, evidence } = fixtureCase(); evidence.frame.target.appViewVariant = "ScriptList";
    expect(fixtureEvidenceIssues(binding, evidence)).toContain("fixture_route_mismatch");
  });
  test("parent cannot stand in for microphone child", () => {
    const { binding, evidence } = fixtureCase(true); evidence.frame.target.windowId = "dictation"; evidence.state.window.id = "dictation";
    expect(fixtureEvidenceIssues(binding, evidence)).toContain("microphone_child_required");
    expect(fixtureEvidenceIssues(binding, evidence)).toContain("fixture_child_parent_mismatch");
  });
  test("wrong presentation and omitted controls fail independently", () => {
    const { binding, evidence } = fixtureCase(); evidence.state.surfaceContract.surfaceKind = "FileSearchFull"; evidence.elements.elements.pop();
    expect(fixtureEvidenceIssues(binding, evidence)).toContain("fixture_presentation_mismatch");
    expect(fixtureEvidenceIssues(binding, evidence)).toContain("fixture_required_control_missing:list:file-results");
  });
  test("replaced parent and independently sampled semantics are refused", () => {
    const { binding, evidence } = fixtureCase(true); evidence.state.window.parentWindowGeneration++;
    evidence.elements.targetIdentity = { ...evidence.frame.target, frameGeneration: 2 };
    expect(fixtureEvidenceIssues(binding, evidence)).toContain("fixture_child_parent_mismatch");
    expect(fixtureEvidenceIssues(binding, evidence)).toContain("fixture_elements_frame_mismatch");
  });
  test("passing campaign flags cannot replace the full retained inventory", () => {
    expect(familyCampaignIssues({ fixtures: [{ id: "main.file-search-mini", pass: true, assertions: [{ pass: true }] }] })).toContain("complete_122_fixture_inventory_required");
    expect(familyCampaignIssues({ scenario: "production-family-matrix" })).toContain("complete_122_fixture_inventory_required");
    expect(familyCampaignIssues({})).toContain("complete_122_fixture_inventory_required");
  });
});
