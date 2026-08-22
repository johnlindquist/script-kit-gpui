import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  auditFamilyFixture,
  auditFamilyFixtureSet,
  DEFAULT_FAMILY_FIXTURE_ROOT,
  fixtureNegativeControls,
} from "./family-fixtures.ts";
import { prepareValidatedReceipt } from "./lib/receipt-schema.ts";
import { runBindingsPipeline } from "./surfaces.ts";

function mainFixture(): Record<string, any> {
  return JSON.parse(
    readFileSync(`${DEFAULT_FAMILY_FIXTURE_ROOT}/main-menu/fixture.json`, "utf8"),
  ) as Record<string, any>;
}

async function bindings() {
  const pipeline = await runBindingsPipeline();
  return [...pipeline.build.set.bindings, ...pipeline.build.set.aliases];
}

describe("deterministic consistency fixture-family contracts", () => {
  test("all nine families cover exactly 54 canonical mappings and five non-counting aliases", async () => {
    const candidate = await auditFamilyFixtureSet();
    const prepared = prepareValidatedReceipt(
      "devtools.consistency.family-fixtures",
      candidate,
    );
    expect(prepared.exitCode).toBe(0);
    expect(prepared.receipt.disposition).toBe("EVALUABLE_PASS");
    expect(prepared.receipt.evidenceClass).toBe("FIXTURE_CONTRACT");
    expect(prepared.receipt.provesRuntimeBehavior).toBe(false);
    expect(prepared.receipt.verifiedRuntimeProofCount).toBe(0);
    expect(prepared.receipt.auditedFamilyCount).toBe(9);
    expect(prepared.receipt.auditedCanonicalBindingCount).toBe(54);
    expect(prepared.receipt.auditedAliasBindingCount).toBe(5);
    expect((prepared.receipt.catalogBinding as Record<string, unknown>).taskId).toBe(
      "PF-010",
    );
    expect(candidate.negativeControls.every((control) => control.pass)).toBe(true);
  });

  test("missing, duplicate, and foreign bindings fail the family contract", async () => {
    const available = await bindings();
    const fixture = mainFixture();

    const missing = auditFamilyFixture(
      "main-menu",
      { ...fixture, memberBindingIds: fixture.memberBindingIds.slice(1) },
      available,
    );
    expect(missing.pass).toBe(false);
    expect(missing.errors.join("\n")).toContain("missing-family-binding:");

    const duplicate = auditFamilyFixture(
      "main-menu",
      {
        ...fixture,
        memberBindingIds: [...fixture.memberBindingIds, fixture.memberBindingIds[0]],
      },
      available,
    );
    expect(duplicate.errors).toContain("duplicate-member-binding");

    const foreign = auditFamilyFixture(
      "main-menu",
      {
        ...fixture,
        memberBindingIds: [...fixture.memberBindingIds, "Settings::SettingsView@MainWindow"],
      },
      available,
    );
    expect(foreign.errors.join("\n")).toContain("unexpected-family-binding:");
  });

  test("wrong host, action availability, lifetime, dismissal, and ownership cannot pass", async () => {
    const available = await bindings();
    const fixture = mainFixture();
    const mutants: Array<[Record<string, unknown>, string]> = [
      [{ ...fixture, expectedHost: "NotesWindow" }, "fixture-host-or-appview-mismatch"],
      [
        {
          ...fixture,
          states: {
            ...fixture.states,
            enabled: { actionAllowed: false, disabledReason: null },
          },
        },
        "invalid-enabled-action-state",
      ],
      [
        {
          ...fixture,
          states: {
            ...fixture.states,
            targetSwitch: { ...fixture.states.targetSwitch, to: "MainWindow@1" },
          },
        },
        "target-lifetime-generation-did-not-advance",
      ],
      [
        {
          ...fixture,
          states: {
            ...fixture.states,
            dismiss: { ...fixture.states.dismiss, restoresOwnerFocus: false },
          },
        },
        "dismiss-does-not-declare-owner-focus-restoration",
      ],
      [
        {
          ...fixture,
          states: {
            ...fixture.states,
            actionPolicy: { ...fixture.states.actionPolicy, descriptorOwned: false },
          },
        },
        "action-policy-does-not-bind-an-executable-owner-and-selection",
      ],
    ];

    for (const [mutant, expectedError] of mutants) {
      const result = auditFamilyFixture("main-menu", mutant, available);
      expect(result.errors).toContain(expectedError);
      expect(result.pass).toBe(false);
    }
  });

  test("declared historical receipt paths are references, never runtime proof", async () => {
    const result = auditFamilyFixture("main-menu", mainFixture(), await bindings());
    expect(result.referencedRuntimeReceiptCount).toBe(1);
    expect(result.verifiedRuntimeProofCount).toBe(0);
    expect(result.pass).toBe(true);
  });

  test("the executable schema rejects unsafe claims and invented runtime proof", async () => {
    const baseline = await auditFamilyFixtureSet();
    for (const override of [
      { evidenceClass: "DIRECT_RUNTIME_PROOF" },
      { provesRuntimeBehavior: true },
      { verifiedRuntimeProofCount: 1 },
      { auditedCanonicalBindingCount: 53 },
      { catalogBinding: { ...baseline.catalogBinding!, taskId: "GOV-007" } },
      { safety: { ...baseline.safety, capturesScreen: true } },
      { negativeControls: [{ id: "broken", pass: false }] },
    ]) {
      const result = prepareValidatedReceipt(
        "devtools.consistency.family-fixtures",
        { ...baseline, ...override },
      );
      expect(result.receipt.disposition).toBe("INVALID_SCHEMA");
    }
  });

  test("fixture negative controls independently prove each fail-closed branch", async () => {
    const negatives = fixtureNegativeControls(mainFixture(), await bindings());
    expect(negatives.map((negative) => negative.id)).toEqual([
      "missing-canonical-member",
      "foreign-family-member",
      "disabled-action-still-enabled",
      "reused-target-lifetime",
      "lost-dismiss-focus-owner",
    ]);
    expect(negatives.every((negative) => negative.pass)).toBe(true);
  });
});
