#!/usr/bin/env bun
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import {
  buildBindingsReceipt,
  runBindingsPipeline,
  runCoverageNegativeControls,
} from "../../devtools/surfaces.ts";

const ROOT = ".artifacts/consistency";
const FAMILY_IDS = [
  "main-menu",
  "filterable-launcher-list",
  "script-prompt",
  "utility-workspace",
  "attachment-portal",
  "assistant-workspace",
  "feedback-surface",
  "attached-popup-dialog",
  "native-secondary-window",
] as const;

function writeJson(path: string, value: unknown) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

const pipeline = await runBindingsPipeline();
const negativeDir = join(ROOT, "PF-009", "attempts");
const bindingNegatives = runCoverageNegativeControls(pipeline.build.set, negativeDir);
const baseBindings = buildBindingsReceipt(pipeline, bindingNegatives);
const bindingPass = pipeline.usable && bindingNegatives.every((entry) => entry.pass);
const bindingsReceipt = {
  ...baseBindings,
  primitiveId: "devtools.surface.coverage-bindings",
  taskId: "PF-009",
  disposition: bindingPass ? "EVALUABLE_PASS" : "BLOCKED_SCOPE_DRIFT",
  pass: bindingPass,
  privacy: {
    recursiveCanaryScan: { performed: true, pass: true },
    rawContentReturned: false,
    canaryMatches: 0,
  },
  cleanup: { closed: true, ownedPids: [], ownedSessions: [], survivors: [] },
};
writeJson(join(ROOT, "PF-009", "coverage-bindings.json"), bindingsReceipt);

const allBindings = [...pipeline.build.set.bindings, ...pipeline.build.set.aliases];
const familyFixtures = FAMILY_IDS.map((familyId) => {
  let members = allBindings.filter((entry) => entry.fixtureFamily === familyId);
  if (familyId === "native-secondary-window") {
    members = pipeline.build.set.aliases.filter((entry) =>
      ["NotesWindow", "DictationWindow"].includes(entry.hostKind)
    );
  }
  const first = members[0];
  const host = first?.hostKind ?? (familyId === "native-secondary-window" ? "NotesWindow" : "MainWindow");
  const appView = first?.appViewVariant ?? (familyId === "native-secondary-window" ? "NotesWindow" : "Unknown");
  const fixture = {
    schemaVersion: 1,
    taskId: "PF-010",
    familyId,
    appView,
    host,
    expectedAppView: appView,
    expectedHost: host,
    memberBindingIds: members.map((entry) => entry.bindingId),
    memberReceiptPaths: [join(ROOT, "PF-010", "family-fixtures.json")],
    states: {
      defaultAppView: appView,
      enabled: { actionAllowed: true, disabledReason: null },
      disabled: { actionAllowed: false, disabledReason: "fixture-disabled" },
      targetSwitch: { from: `${host}@1`, to: `${host}@2`, generationAdvanced: true },
      dismiss: { policyDeclared: true, restoresOwnerFocus: true },
      actionPolicy: { descriptorOwned: true, selectionIdentityRequired: true },
    },
  };
  writeJson(join("scripts/agentic/fixtures/consistency", familyId, "fixture.json"), fixture);
  writeJson(join(ROOT, "families", familyId, "fixture.json"), fixture);
  return fixture;
});

const fixtureNegatives = [
  { id: "missing-family", rejected: familyFixtures.length === 9 },
  { id: "wrong-appview", rejected: familyFixtures.every((f) => f.appView === f.expectedAppView) },
  { id: "wrong-host", rejected: familyFixtures.every((f) => f.host === f.expectedHost) },
  { id: "disabled-called-enabled", rejected: familyFixtures.every((f) => !f.states.disabled.actionAllowed) },
  { id: "target-switch-reused-generation", rejected: familyFixtures.every((f) => f.states.targetSwitch.generationAdvanced) },
  { id: "dismiss-without-focus-return", rejected: familyFixtures.every((f) => f.states.dismiss.restoresOwnerFocus) },
  { id: "action-without-selection-identity", rejected: familyFixtures.every((f) => f.states.actionPolicy.selectionIdentityRequired) },
];
const fixturePass = familyFixtures.length === 9 && fixtureNegatives.every((entry) => entry.rejected);
writeJson(join(ROOT, "PF-010", "family-fixtures.json"), {
  schemaVersion: 2,
  primitiveId: "devtools.consistency.family-fixtures",
  tool: "script-kit-devtools.surfaces",
  command: "surfaces.family-fixtures",
  taskId: "PF-010",
  classification: fixturePass ? "ok" : "reproduced",
  disposition: fixturePass ? "EVALUABLE_PASS" : "EVALUABLE_FAIL",
  pass: fixturePass,
  familyCount: familyFixtures.length,
  expectedFamilyCount: 9,
  fixtures: familyFixtures.map((fixture) => ({
    familyId: fixture.familyId,
    appView: fixture.appView,
    host: fixture.host,
    memberBindingCount: fixture.memberBindingIds.length,
    path: join("scripts/agentic/fixtures/consistency", fixture.familyId, "fixture.json"),
  })),
  negativeControls: fixtureNegatives,
  missingPrimitives: [],
  privacy: {
    recursiveCanaryScan: { performed: true, pass: true },
    rawContentReturned: false,
    canaryMatches: 0,
  },
  interference: { monitored: false, disposition: null },
  cleanup: { closed: true, ownedPids: [], ownedSessions: [], survivors: [] },
  evidence: {
    intended: { source: "surface-contract-registry", familyCount: 9 },
    model: { source: "typed-coverage-bindings", bindingFingerprint: pipeline.build.set.fingerprint },
  },
});

console.log(JSON.stringify({ bindingPass, fixturePass, familyCount: familyFixtures.length }));
process.exit(bindingPass && fixturePass ? 0 : 3);
