#!/usr/bin/env bun
/**
 * PF-010: deterministic, capture-free fixture contract verification.
 *
 * This verifies checked-in fixture/state coverage against the canonical
 * binding inventory. It never launches the app; no fixture contract, alias,
 * or declared receipt path is represented as runtime behavior evidence.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  FAMILY_IDS,
  parseTaskCatalog,
} from "./consistency.ts";
import { emitValidatedReceipt, RECEIPT_SCHEMA_VERSION } from "./lib/receipt-schema.ts";
import {
  runBindingsPipeline,
  type CoverageAliasBinding,
  type CoverageBindingRecord,
} from "./surfaces.ts";

type JsonObject = Record<string, unknown>;
type FamilyBinding = CoverageBindingRecord | CoverageAliasBinding;

export const DEFAULT_FAMILY_FIXTURE_ROOT =
  "scripts/agentic/fixtures/consistency";

function record(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : {};
}

function strings(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === "string")
    : [];
}

function generation(value: string, host: string): number | null {
  const prefix = `${host}@`;
  if (!value.startsWith(prefix)) return null;
  const parsed = Number(value.slice(prefix.length));
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : null;
}

export interface FamilyFixtureAudit {
  familyId: string;
  pass: boolean;
  canonicalMemberCount: number;
  aliasMemberCount: number;
  memberBindingCount: number;
  memberBindingIds: string[];
  referencedRuntimeReceiptCount: number;
  verifiedRuntimeProofCount: 0;
  appView: string | null;
  host: string | null;
  errors: string[];
}

export function auditFamilyFixture(
  expectedFamilyId: string,
  fixture: JsonObject,
  bindings: readonly FamilyBinding[],
): FamilyFixtureAudit {
  const errors: string[] = [];
  const familyBindings = bindings.filter(
    (binding) => binding.fixtureFamily === expectedFamilyId,
  );
  const expectedIds = familyBindings.map((binding) => binding.bindingId).sort();
  const actualIds = strings(fixture.memberBindingIds).sort();
  const actualUnique = new Set(actualIds);
  if (fixture.schemaVersion !== 1) errors.push("invalid-fixture-schema-version");
  if (fixture.taskId !== "PF-010") errors.push("wrong-task-id");
  if (fixture.familyId !== expectedFamilyId) errors.push("wrong-family-id");
  if (actualIds.length !== actualUnique.size) errors.push("duplicate-member-binding");
  if (expectedIds.length === 0) errors.push("family-without-canonical-bindings");
  for (const bindingId of expectedIds) {
    if (!actualUnique.has(bindingId)) {
      errors.push(`missing-family-binding:${bindingId}`);
    }
  }
  const knownIds = new Set(expectedIds);
  for (const bindingId of actualIds) {
    if (!knownIds.has(bindingId)) {
      errors.push(`unexpected-family-binding:${bindingId}`);
    }
  }

  const appView = typeof fixture.appView === "string" ? fixture.appView : null;
  const host = typeof fixture.host === "string" ? fixture.host : null;
  if (
    appView === null ||
    host === null ||
    fixture.expectedAppView !== appView ||
    fixture.expectedHost !== host
  ) {
    errors.push("fixture-host-or-appview-mismatch");
  }
  if (
    !familyBindings.some(
      (binding) => binding.appViewVariant === appView && binding.hostKind === host,
    )
  ) {
    errors.push("fixture-default-does-not-resolve-to-a-canonical-member");
  }

  const states = record(fixture.states);
  if (states.defaultAppView !== appView) errors.push("wrong-default-appview");
  const enabled = record(states.enabled);
  if (enabled.actionAllowed !== true || enabled.disabledReason !== null) {
    errors.push("invalid-enabled-action-state");
  }
  const disabled = record(states.disabled);
  if (
    disabled.actionAllowed !== false ||
    typeof disabled.disabledReason !== "string" ||
    disabled.disabledReason.length === 0
  ) {
    errors.push("invalid-disabled-action-state");
  }
  const targetSwitch = record(states.targetSwitch);
  const previous = generation(String(targetSwitch.from ?? ""), host ?? "");
  const next = generation(String(targetSwitch.to ?? ""), host ?? "");
  if (
    targetSwitch.generationAdvanced !== true ||
    previous === null ||
    next === null ||
    next <= previous
  ) {
    errors.push("target-lifetime-generation-did-not-advance");
  }
  const dismiss = record(states.dismiss);
  if (dismiss.policyDeclared !== true || dismiss.restoresOwnerFocus !== true) {
    errors.push("dismiss-does-not-declare-owner-focus-restoration");
  }
  const actionPolicy = record(states.actionPolicy);
  if (
    actionPolicy.descriptorOwned !== true ||
    actionPolicy.selectionIdentityRequired !== true
  ) {
    errors.push("action-policy-does-not-bind-an-executable-owner-and-selection");
  }

  const aliasCount = familyBindings.filter((binding) =>
    binding.bindingId.startsWith("alias:")
  ).length;
  return {
    familyId: expectedFamilyId,
    pass: errors.length === 0,
    canonicalMemberCount: familyBindings.length - aliasCount,
    aliasMemberCount: aliasCount,
    memberBindingCount: actualIds.length,
    memberBindingIds: actualIds,
    referencedRuntimeReceiptCount: strings(fixture.memberReceiptPaths).length,
    verifiedRuntimeProofCount: 0,
    appView,
    host,
    errors,
  };
}

function clone(value: JsonObject): JsonObject {
  return JSON.parse(JSON.stringify(value)) as JsonObject;
}

export function fixtureNegativeControls(
  fixture: JsonObject,
  bindings: readonly FamilyBinding[],
) {
  const expectedFamilyId = String(fixture.familyId);
  const cases: Array<[string, (candidate: JsonObject) => void, string]> = [
    ["missing-canonical-member", (candidate) => {
      candidate.memberBindingIds = strings(candidate.memberBindingIds).slice(1);
    }, "missing-family-binding:"],
    ["foreign-family-member", (candidate) => {
      candidate.memberBindingIds = [...strings(candidate.memberBindingIds), "Unknown::Other@MainWindow"];
    }, "unexpected-family-binding:"],
    ["disabled-action-still-enabled", (candidate) => {
      record(record(candidate.states).disabled).actionAllowed = true;
    }, "invalid-disabled-action-state"],
    ["reused-target-lifetime", (candidate) => {
      const targetSwitch = record(record(candidate.states).targetSwitch);
      targetSwitch.to = targetSwitch.from;
    }, "target-lifetime-generation-did-not-advance"],
    ["lost-dismiss-focus-owner", (candidate) => {
      record(record(candidate.states).dismiss).restoresOwnerFocus = false;
    }, "dismiss-does-not-declare-owner-focus-restoration"],
  ];
  return cases.map(([id, mutate, expectedError]) => {
    const candidate = clone(fixture);
    mutate(candidate);
    const result = auditFamilyFixture(expectedFamilyId, candidate, bindings);
    return {
      id,
      expectedError,
      pass: !result.pass && result.errors.some((error) => error.includes(expectedError)),
    };
  });
}

export async function auditFamilyFixtureSet(
  options: { fixturesRoot?: string } = {},
) {
  const fixturesRoot = options.fixturesRoot ?? DEFAULT_FAMILY_FIXTURE_ROOT;
  const pipeline = await runBindingsPipeline();
  const bindings = [...pipeline.build.set.bindings, ...pipeline.build.set.aliases];
  const results: FamilyFixtureAudit[] = [];
  const fixtures: JsonObject[] = [];
  const errors: string[] = [...pipeline.blockReasons];

  for (const familyId of FAMILY_IDS) {
    const path = join(fixturesRoot, familyId, "fixture.json");
    let fixture: JsonObject;
    try {
      fixture = record(JSON.parse(readFileSync(path, "utf8")));
    } catch {
      errors.push(`missing-or-invalid-family-fixture:${familyId}`);
      continue;
    }
    fixtures.push(fixture);
    const audit = auditFamilyFixture(familyId, fixture, bindings);
    results.push(audit);
    errors.push(...audit.errors.map((error) => `${familyId}:${error}`));
  }

  const negativeControls = fixtures.length > 0
    ? fixtureNegativeControls(fixtures[0]!, bindings)
    : [];
  for (const control of negativeControls) {
    if (!control.pass) errors.push(`negative-control-did-not-fail:${control.id}`);
  }

  const catalog = parseTaskCatalog(
    readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
    DEFAULT_CONSISTENCY_CATALOG_PATH,
  );
  const canonicalTask = catalog.byId.get("PF-010");
  if (!canonicalTask || catalog.errors.length > 0) {
    errors.push("canonical-family-task-catalog-binding-unavailable");
  }

  return {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    primitiveId: "devtools.consistency.family-fixtures",
    tool: "script-kit-devtools.family-fixtures",
    command: "family-fixtures.verify",
    evidenceClass: "FIXTURE_CONTRACT" as const,
    provesRuntimeBehavior: false as const,
    classification: errors.length === 0 ? "ok" : "reproduced",
    taskIds: ["PF-010"],
    catalogBinding: canonicalTask
      ? {
          catalogPath: DEFAULT_CONSISTENCY_CATALOG_PATH,
          taskId: canonicalTask.id,
          title: canonicalTask.title,
          sectionSha256: canonicalTask.sectionSha256,
        }
      : null,
    fixtureRoot: fixturesRoot,
    expectedFamilyCount: FAMILY_IDS.length,
    auditedFamilyCount: results.length,
    expectedCanonicalBindingCount: pipeline.build.set.bindings.length,
    auditedCanonicalBindingCount: results.reduce(
      (total, family) => total + family.canonicalMemberCount,
      0,
    ),
    expectedAliasBindingCount: pipeline.build.set.aliases.length,
    auditedAliasBindingCount: results.reduce(
      (total, family) => total + family.aliasMemberCount,
      0,
    ),
    verifiedRuntimeProofCount: 0,
    safety: {
      startsApplication: false,
      revealsWindow: false,
      focusesWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
      accessesNetwork: false,
      usesLiveAi: false,
    },
    families: results,
    negativeControls,
    errors,
  };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(
      "Usage: bun scripts/devtools/family-fixtures.ts [--fixtures <path>] [--out <receipt.json>]",
    );
  } else {
    const fixturesIndex = argv.indexOf("--fixtures");
    const outIndex = argv.indexOf("--out");
    const fixturesRoot = fixturesIndex >= 0 ? argv[fixturesIndex + 1] : undefined;
    const outputPath = outIndex >= 0 ? argv[outIndex + 1] : undefined;
    if ((fixturesIndex >= 0 && !fixturesRoot) || (outIndex >= 0 && !outputPath)) {
      console.error("--fixtures and --out each require a value");
      process.exit(64);
    }
    const receipt = await auditFamilyFixtureSet({ fixturesRoot });
    emitValidatedReceipt(
      "devtools.consistency.family-fixtures",
      receipt,
      outputPath,
    );
  }
}
