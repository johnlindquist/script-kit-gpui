import { expect, test } from "bun:test";
import { PRODUCTION_STORIES, CORE_JOURNEYS, SDK_RUNTIME_ASSERTIONS, FOOTER_RUNTIME_ASSERTIONS, selectStoryTests, observeStoryTests, completedGpuiDispatchIssues, aggregateCleanup, productionStoryReceiptIssues } from "./lib/story-contract.ts";
import { publicationCausalityIssues, validateThemeEdits } from "./lib/owned-evaluation.ts";
import { completedFrameIssues, declaredTransitionIssues, type AutomationTargetSnapshot } from "./lib/target-identity.ts";
import { unknownOwnedCleanup } from "./driver.ts";
import { latencySummary } from "./design.ts";

const names = PRODUCTION_STORIES.map(story => `test_support::production_stories::${story.testLeaf}`);
const listed = names.map(name => `${name}: test`).join("\n");
const passing = `running ${names.length} tests\n${names.map(name => `test ${name} ... ok`).join("\n")}\ntest result: ok. ${names.length} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n`;

test("library selection is unique, reviewed, nonempty and exact", () => {
  expect(selectStoryTests(listed).issues).toEqual([]);
  for (const invalid of ["", listed + "\nforeign: test", listed + `\n${names[0]}: test`, names.slice(1).map(name => `${name}: test`).join("\n")])
    expect(selectStoryTests(invalid).issues.length).toBeGreaterThan(0);
});
test("libtest proof rejects zero, ignored, duplicate, wrong-identity and partial runs", () => {
  expect(observeStoryTests(passing, 0, names).issues).toEqual([]);
  for (const invalid of ["", passing.replace(`running ${names.length} tests`, "running 0 tests"), passing.replace(" ... ok", " ... ignored"),
    passing.replace(names[0]!, "foreign"), passing + passing, passing.replace("0 failed", "1 failed")])
    expect(observeStoryTests(invalid, 0, names).issues.length).toBeGreaterThan(0);
  expect(observeStoryTests(passing, 1, names).issues).toContain("libtest_nonzero_exit");
});
test("terminal deferred dispatch is completed proof but scheduling is not", () => {
  const response = { type: "simulateGpuiEventResult", requestId: "r", success: true, dispatchScheduled: false,
    dispatchCompleted: true, dispatchPath: "exact_handle_deferred", wasDeferred: true, resolvedWindowId: "main" };
  expect(completedGpuiDispatchIssues(response)).toEqual([]);
  expect(completedGpuiDispatchIssues({ ...response, dispatchCompleted: false, dispatchScheduled: true })).toContain("dispatch_not_completed");
});
test("publication must causally invalidate both families before frames can hide a missing notification", () => {
  const main = { type: "instance" as const, id: "main", generation: 1 }; const notes = { ...main, id: "notes" };
  const publication = { operation: "applyTheme" as const, ok: true as const, revision: 2, previousRevision: 1,
    resolved: [], phaseDurationsMs: {}, invalidations: [main, notes].map(target => ({ target, revision: 2, cause: "themePublication" as const, invalidationEpoch: 4 })) };
  expect(publicationCausalityIssues(publication, [main, notes])).toEqual([]);
  const missing = { ...publication, invalidations: publication.invalidations.slice(0, 1) };
  expect(publicationCausalityIssues(missing, [main, notes])).toEqual(["publication_not_delivered:notes:1"]);
  expect(publicationCausalityIssues({ ...publication, invalidations: [...publication.invalidations, publication.invalidations[0]!] }, [main, notes]).length).toBeGreaterThan(0);
});
const target: AutomationTargetSnapshot = { windowId: "main", windowGeneration: 2, appViewVariant: "ScriptList", targetGeneration: 1,
  surfaceGeneration: 1, dataGeneration: 4, presentationRevision: 1, themeRevision: 2, frameGeneration: 3 };
const owned = { pid: 20, processStartTime: "fixture-start", processInstanceId: "instance", sessionGeneration: "session", binarySha256: "a".repeat(64), manifestSha256: "b".repeat(64) };
test("frame identity is bound to requested lifetime, exact process, artifact and semantic revisions", () => {
  const requested = { type: "instance" as const, id: "main", generation: 2 }; const frame = { ...owned, target, requestedTarget: requested };
  expect(completedFrameIssues(requested, frame, owned, target)).toEqual([]);
  expect(completedFrameIssues(requested, { ...frame, pid: 21 }, owned)).toContain("frame_pid_mismatch");
  expect(completedFrameIssues({ ...requested, generation: 1 }, frame, owned)).toContain("requested_instance_mismatch");
  expect(completedFrameIssues(requested, { ...frame, target: { ...target, dataGeneration: 3 } }, owned, target)).toContain("frame_dataGeneration_stale");
});
test("declared data transitions preserve lifetime and reject revision regression", () => {
  expect(declaredTransitionIssues(target, { ...target, dataGeneration: 5 }, ["dataGeneration"])).toEqual([]);
  expect(declaredTransitionIssues(target, { ...target, dataGeneration: 5 }, [])).toContain("undeclared_transition:dataGeneration");
  expect(declaredTransitionIssues(target, { ...target, dataGeneration: 3 }, ["dataGeneration"])).toContain("revision_regressed:dataGeneration");
});
test("missing post-spawn cleanup cannot be aggregated into a closed run", () => {
  expect(aggregateCleanup([unknownOwnedCleanup(false), unknownOwnedCleanup(true)]).closed).toBe(false);
  expect(aggregateCleanup([unknownOwnedCleanup(true)]).survivors.length).toBe(1);
});
test("live edits reject locked tokens, credentials, duplicates and nonfinite values", () => {
  expect(validateThemeEdits([{ tokenId: "theme.colors.accent.selected", value: 0x5b9dff }])).toHaveLength(1);
  for (const edit of [[], [{ token: "theme.opacity.hover", value: 0.2 }], [{ tokenId: "glass.blur", value: 2 }],
    [{ tokenId: "theme.opacity.hover", value: Infinity }], [{ tokenId: "theme.opacity.hover", value: 2 }],
    [{ tokenId: "theme.opacity.hover", value: 0.2 }, { tokenId: "theme.opacity.hover", value: 0.3 }]]) expect(() => validateThemeEdits(edit)).toThrow();
});
test("latency proof requires 30 parent-clock observations and immutable warm budgets", () => {
  expect(() => latencySummary([])).toThrow("thirty_measured_edits_required");
  expect(latencySummary(Array.from({ length: 30 }, () => ({ frameMs: 99, readbackMs: 249 }))).pass).toBe(true);
  expect(latencySummary(Array.from({ length: 30 }, () => ({ frameMs: 101, readbackMs: 251 }))).pass).toBe(false);
});

test("runtime story gate requires exactly one complete real SDK journey", () => {
  const sdk = { id: "sdk-prompt-roundtrip", proofLevel: "owned-production-runtime", pass: true,
    assertions: SDK_RUNTIME_ASSERTIONS.map(id => ({ id, pass: true })) };
  const core = CORE_JOURNEYS.map(id => ({ id, pass: true, assertions: [{ id: "observed", pass: true }] }));
  const footer = { id: "footer-owner-isolation", proofLevel: "owned-production-runtime", pass: true,
    assertions: FOOTER_RUNTIME_ASSERTIONS.map(id => ({ id, pass: true })) };
  const receipt = { lane: "all", library: { selection: selectStoryTests(listed), execution: observeStoryTests(passing, 0, names) },
    cleanup: aggregateCleanup([]), journeys: [...core, sdk, footer] };
  expect(productionStoryReceiptIssues(receipt)).toEqual([]);
  for (const journeys of [core, [...core, sdk, sdk], [...core, sdk, { ...sdk, pass: false }],
    [...core, { ...sdk, pass: false }], [...core, { ...sdk, proofLevel: "domain" }],
    [...core, { ...sdk, assertions: [] }], [...core, { ...sdk, assertions: undefined }],
    [...core, { ...sdk, assertions: [...sdk.assertions, { id: "failed", pass: false }] }]]) {
    expect(productionStoryReceiptIssues({ ...receipt, journeys })).toContain("complete_sdk_runtime_journey_required");
  }
  for (const required of SDK_RUNTIME_ASSERTIONS) {
    const missing = { ...sdk, assertions: sdk.assertions.filter(assertion => assertion.id !== required) };
    const duplicate = { ...sdk, assertions: [...sdk.assertions, { id: required, pass: true }] };
    expect(productionStoryReceiptIssues({ ...receipt, journeys: [...core, missing] })).toContain("complete_sdk_runtime_journey_required");
    expect(productionStoryReceiptIssues({ ...receipt, journeys: [...core, duplicate] })).toContain("complete_sdk_runtime_journey_required");
  }
  for (const journeys of [[...core, sdk], [...core, sdk, footer, footer], [...core, sdk, { ...footer, pass: false }],
    [...core, sdk, { ...footer, assertions: footer.assertions.slice(1) }]]) {
    expect(productionStoryReceiptIssues({ ...receipt, journeys })).toContain("complete_footer_runtime_journey_required");
  }
  expect(productionStoryReceiptIssues({ ...receipt, lane: "library", journeys: [], provesRuntimeBehavior: false,
    evidenceClass: "UNIT_BEHAVIOR" })).toEqual([]);
});
