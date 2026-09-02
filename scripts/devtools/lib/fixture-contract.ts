import type { Json } from "../driver.ts";
import type { FixtureDescriptor } from "./owned-evaluation.ts";
import type { AutomationInstance } from "./target-identity.ts";
import { isReferenceReceipt, resolveReceiptDetails } from "./receipt-artifact.ts";

export interface FixtureBinding {
  descriptor: FixtureDescriptor;
  parent?: AutomationInstance;
}

const identityFields = ["windowId", "windowGeneration", "appViewVariant", "targetGeneration", "surfaceGeneration", "dataGeneration", "presentationRevision", "themeRevision", "frameGeneration"] as const;
const object = (value: unknown): Json => value !== null && typeof value === "object" && !Array.isArray(value) ? value as Json : {};

/** Recompute qualification from the retained observations; assertion flags are not proof. */
export function fixtureEvidenceIssues(binding: FixtureBinding, evidence: Json): string[] {
  const fixture = binding.descriptor;
  const state = object(evidence.state); const elements = object(evidence.elements); const layout = object(evidence.layout);
  const frame = object(object(evidence.frame).target); const window = object(state.window);
  const nodes: Json[] = Array.isArray(elements.elements) ? elements.elements : [];
  const issues: string[] = [];
  if (!fixture || !fixture.id || !fixture.expectedSemanticSurface || !Array.isArray(fixture.requiredSemanticIds) || !fixture.requiredSemanticIds.length)
    return ["fixture_contract_missing"];
  if (typeof frame.windowId !== "string" || typeof frame.appViewVariant !== "string" ||
      identityFields.filter(key => key !== "windowId" && key !== "appViewVariant").some(key => !Number.isSafeInteger(frame[key]) || frame[key] < 0)) issues.push("fixture_frame_identity_incomplete");
  const captured = object(object(evidence.frameIdentity).target);
  if (evidence.status !== "captured" || evidence.source !== "gpuiRenderReadback" || evidence.scope !== "liveAutomationWindowRenderReadback" || identityFields.some(key => captured[key] !== frame[key])) issues.push("fixture_framebuffer_identity_mismatch");
  if (state.windowVisible !== false || window.visible !== false || window.focused !== false) issues.push("fixture_not_hidden");
  for (const [name, observation] of [["state", state], ["elements", elements], ["layout", layout]] as const) {
    const identity = object(observation.targetIdentity);
    if (!Number.isSafeInteger(frame.windowGeneration) || identityFields.some(key => identity[key] !== frame[key])) issues.push(`fixture_${name}_frame_mismatch`);
  }
  if (window.id !== frame.windowId || window.generation !== frame.windowGeneration) issues.push("fixture_window_identity_mismatch");
  if (fixture.appViewVariant && frame.appViewVariant !== fixture.appViewVariant) issues.push("fixture_route_mismatch");
  const semanticSurface = elements.semanticSurface ?? window.semanticSurface;
  if (semanticSurface !== fixture.expectedSemanticSurface) issues.push("fixture_semantic_surface_mismatch");
  const contract = object(state.surfaceContract);
  if (fixture.root === "main") {
    const presentation = object(contract.presentation);
    if ([presentation.shellOwner, presentation.inputOwner, presentation.themeOwner, presentation.rowPrimitive].some(value => typeof value !== "string" || !value.length)) issues.push("fixture_presentation_missing");
    if (fixture.surfaceVariant && contract.surfaceKind !== fixture.surfaceVariant) issues.push("fixture_presentation_mismatch");
  }
  if (fixture.parentFixtureId) {
    const parent = binding.parent;
    if (!parent) issues.push("fixture_parent_missing");
    else if (fixture.root === "secondary") {
      if (window.id === parent.id || window.parentWindowId !== parent.id || window.parentWindowGeneration !== parent.generation) issues.push("fixture_child_parent_mismatch");
    } else if (frame.windowId !== parent.id || frame.windowGeneration !== parent.generation) issues.push("fixture_in_scene_parent_mismatch");
  }
  const ids = nodes.map(node => node.semanticId);
  if (ids.some(id => typeof id !== "string" || !id.length) || new Set(ids).size !== ids.length) issues.push("fixture_semantic_ids_invalid");
  for (const required of fixture.requiredSemanticIds) {
    const prefix = required.endsWith("*") ? required.slice(0, -1) : undefined;
    if (!required.length || prefix === "" || !ids.some(id => typeof id === "string" && (prefix === undefined ? id === required : id.startsWith(prefix)))) issues.push(`fixture_required_control_missing:${required}`);
  }
  const components: Json[] = Array.isArray(layout.components) ? layout.components : [];
  if (!components.some(component => component.bounds?.width > 0 && component.bounds?.height > 0)) issues.push("fixture_layout_missing");
  if (!Number.isFinite(evidence.capture?.width) || !Number.isFinite(evidence.capture?.height) || evidence.capture.width <= 0 || evidence.capture.height <= 0) issues.push("fixture_readback_missing");
  // These are distinct catalogue presentations, not aliases for launcher/overlay.
  if (fixture.id === "main.file-search-mini" && (frame.appViewVariant !== "FileSearchView" || contract.surfaceKind !== "FileSearchMini")) issues.push("file_search_mini_entry_mismatch");
  if (fixture.id === "main.file-search-full" && (frame.appViewVariant !== "FileSearchView" || contract.surfaceKind !== "FileSearchFull")) issues.push("file_search_full_entry_mismatch");
  if (fixture.id === "dictation.microphone-picker" && (window.id !== "dictation-microphone-popup" || window.semanticSurface !== "dictationMicrophonePopup" || frame.appViewVariant !== "dictationMicrophonePopup")) issues.push("microphone_child_required");
  return issues;
}

export function familyCampaignIssues(observation: Json): string[] {
  if (isReferenceReceipt(observation)) {
    try { observation = object(resolveReceiptDetails(observation).observation); }
    catch (error) { return [error instanceof Error ? error.message : String(error)]; }
  }
  if (!Array.isArray(observation.fixtures)) return ["complete_122_fixture_inventory_required"];
  const catalogue: FixtureDescriptor[] = Array.isArray(observation.catalogue) ? observation.catalogue : [];
  const rows: Json[] = observation.fixtures;
  const issues: string[] = [];
  if (catalogue.length !== 122 || rows.length !== catalogue.length || new Set(catalogue.map(value => value.id)).size !== 122 || new Set(rows.map(value => value.id)).size !== 122) issues.push("complete_122_fixture_inventory_required");
  for (const descriptor of catalogue) {
    const row = rows.find(value => value.id === descriptor.id);
    if (!row || row.pass !== true || !row.binding || JSON.stringify(row.binding.descriptor) !== JSON.stringify(descriptor)) { issues.push(`fixture_binding_missing:${descriptor.id}`); continue; }
    if (!Array.isArray(row.effects) || row.effects.length !== 1) { issues.push(`fixture_atomic_evidence_required:${descriptor.id}`); continue; }
    issues.push(...fixtureEvidenceIssues(row.binding, row.effects[0]).map(issue => `${descriptor.id}:${issue}`));
  }
  return issues;
}
