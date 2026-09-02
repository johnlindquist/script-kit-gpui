import { createHash } from "node:crypto";
import { OWNED_EVALUATION_LIMITS } from "../devtools/lib/operator-safety.ts";
import type { SearchProviderRun } from "../devtools/lib/owned-evaluation.ts";
import sentenceScenarios from "../../src/design_evaluation/search_sentences.json";

/** Finite production contract. Missing execution or evidence never counts as a pass. */
export const SEARCH_CONTRACT_VERSION = 5;
export const SEARCH_FIXTURE_ID = "main-search-contract";
export const SEARCH_PROVIDERS = ["files", "directory", "brain-lexical", "brain-semantic", "tabs", "history", "windows", "icons", "notes", "todos", "clipboard", "dictation", "conversations", "spine", "brain-inbox", "scripts", "apps", "skills", "validation", "flow-roster"] as const;
export type SearchProvider = typeof SEARCH_PROVIDERS[number];
export const SEARCH_SENTENCE_PROFILES = ["burst", "paced", "word-pauses", "reverse-completions", "correction-aba", "cursor-edit", "deliberate-selection"] as const;
export type SearchSentenceProfile = typeof SEARCH_SENTENCE_PROFILES[number];
export type SearchBounds = { steps: number; requests: number; frames: number; retainedBytes: number; logicalMilliseconds: number; wallMilliseconds: number };
export interface SearchCase {
  id: string;
  inputRoute: "gpui-text" | "gpui-keyboard" | "gpui-pointer" | "gpui-scroll" | "semantic" | "setInput" | "owned-lifecycle";
  providers: readonly SearchProvider[];
  publicationPolicy: "visible" | "cache-only" | "completion-time" | "scope-bound";
  fixture: string;
  selectionIntent: "automatic-top" | "explicit-anchor" | "none" | "both";
  viewportIntent: "leading" | "preserve" | "user-controlled";
  assertions: readonly string[];
  evidence: readonly string[];
  bounds: SearchBounds;
}
const UI = ["production-consumer", "scheduled-frame", "paint-geometry", "retained-pixels"];
const bounds: SearchBounds = { steps: 64, requests: 128, frames: 16, retainedBytes: 131072, logicalMilliseconds: 3000, wallMilliseconds: 20000 };
const OUTPUT_PACKING = Object.freeze({ maxIndependentSchedules: 3, maxIndependentRequests: bounds.requests * 3,
  comparisonFamilies: "exclusive-whole-family", oversizedSchedules: "exclusive-single-schedule",
  byteFit: "runtime-enforced-not-inferred-from-retained-log" });
function entry(id: string, inputRoute: SearchCase["inputRoute"], providers: SearchCase["providers"], publicationPolicy: SearchCase["publicationPolicy"], fixture: string, selectionIntent: SearchCase["selectionIntent"], viewportIntent: SearchCase["viewportIntent"], assertions: string[], evidence = UI): SearchCase {
  const expensive = ["selected-row-removal", "metadata-same-identity", "same-count-replacement", "preview-stale-completion", "late-reveal-retired"].includes(id);
  const units = id === "provider-terminal-errors" ? 30 : id === "publisher-orders" ? 24 :
    id === "passive-budget" ? 8 : ["eligibility-calculator", "retired-window-lifetime", "raw-query-before-commit", "query-aba", "scroll-pending-refresh"].includes(id) ? 4 : expensive ? 3 : selectionIntent === "both" || id === "directory-scope" ? 2 : 1;
  return { id, inputRoute, providers, publicationPolicy, fixture, selectionIntent, viewportIntent, assertions, evidence,
    bounds: { ...bounds, steps: (id === "provider-terminal-errors" ? bounds.requests : bounds.steps) * units, requests: bounds.requests * units, frames: bounds.frames * units,
      retainedBytes: bounds.retainedBytes * units, logicalMilliseconds: expensive ? 120000 : bounds.logicalMilliseconds * units,
      wallMilliseconds: id === "provider-terminal-errors" || id === "publisher-orders" ? 540000 : bounds.wallMilliseconds * units } };
}
export const SEARCH_CASES: readonly SearchCase[] = [
  entry("automatic-higher-arrival", "gpui-text", ["tabs"], "visible", "tab-domain-hoist", "automatic-top", "preserve", ["late-arrival-visible", "automatic-anchor-preserved", "selected-position-preserved", "enter-keeps-visible-subject", "filter-local-focus"]),
  entry("keyboard-anchor-arrival", "gpui-keyboard", ["tabs"], "visible", "tab-domain-hoist", "explicit-anchor", "preserve", ["down-up-dispatched", "late-arrival-visible", "anchor-preserved", "ordinal-preserved", "filter-local-focus"]),
  entry("semantic-anchor-current-first", "semantic", ["tabs"], "visible", "tab-domain-hoist", "explicit-anchor", "preserve", ["same-index-deliberate-intent", "owner-revision-advanced", "anchor-preserved"]),
  entry("click-anchor-arrival", "gpui-pointer", ["tabs"], "visible", "tab-domain-hoist", "explicit-anchor", "preserve", ["unselected-row-click", "anchor-preserved", "filter-local-focus"]),
  entry("pointer-down-publication-up", "gpui-pointer", ["tabs"], "visible", "tab-domain-hoist", "explicit-anchor", "preserve", ["gesture-subject-preserved-or-refused", "no-rebound-activation"]),
  entry("publisher-orders", "setInput", SEARCH_PROVIDERS, "completion-time", "all-providers", "both", "preserve", ["every-intermediate-intent", "final-candidates-equal", "all-provider-timings", "same-turn-completion"]),
  entry("passive-budget", "semantic", ["tabs", "history", "notes", "todos", "clipboard", "dictation", "conversations", "scripts", "apps"], "visible", "passive-budget", "both", "preserve", ["source-budget-deterministic", "cap-removal-explicit"]),
  entry("raw-query-before-commit", "gpui-text", ["tabs", "files"], "completion-time", "all-providers", "automatic-top", "leading", ["old-completion-not-new-intent", "enter-uses-current-query", "controlled-coalescer"]),
  entry("query-aba", "gpui-text", ["tabs", "files"], "completion-time", "all-providers", "automatic-top", "leading", ["old-a-authority-retired"]),
  entry("same-input-noop", "setInput", ["tabs"], "visible", "tab-domain-hoist", "explicit-anchor", "preserve", ["selection-unchanged", "no-worker-duplication", "no-revision-churn"]),
  entry("implicit-files-cache-only", "setInput", ["files"], "cache-only", "files-handoff", "both", "preserve", ["terminal-cache-settled", "rows-selection-viewport-unchanged", "no-list-republication"]),
  entry("explicit-files-publish", "setInput", ["files"], "visible", "files-explicit", "automatic-top", "leading", ["completion-time-visible-policy", "file-rows-published"]),
  entry("pending-files-reuse", "setInput", ["files"], "completion-time", "files-handoff", "automatic-top", "leading", ["compatible-work-reused", "current-attachment-authority", "no-worker-duplication"]),
  entry("directory-scope", "gpui-text", ["directory"], "scope-bound", "directory-browse", "automatic-top", "leading", ["fragment-scope-correct", "stale-directory-rejected", "hidden-option-rejected", "selection-valid"]),
  entry("brain-lexical-semantic", "setInput", ["brain-lexical", "brain-semantic"], "visible", "brain-replacement", "both", "preserve", ["different-semantic-batch-consumed", "selection-policy"]),
  entry("provider-terminal-errors", "setInput", SEARCH_PROVIDERS, "scope-bound", "error", "both", "preserve", ["error-unavailable-disconnect-distinct", "last-good-or-empty-policy", "no-source-broadening"]),
  entry("empty-inert-rows", "setInput", ["windows", "icons"], "visible", "empty", "none", "leading", ["no-selection", "no-marker", "no-preview", "no-row-submission"]),
  entry("selected-row-removal", "semantic", ["tabs", "notes"], "visible", "removal", "explicit-anchor", "preserve", ["anchor-removed-no-fallback", "removed-target-enter-inert", "old-target-refused"]),
  entry("metadata-same-identity", "semantic", ["tabs"], "visible", "metadata", "explicit-anchor", "preserve", ["content-revision-advanced", "new-content-painted", "anchor-preserved"]),
  entry("same-count-replacement", "semantic", ["tabs", "notes"], "visible", "replacement", "explicit-anchor", "preserve", ["real-list-replacement", "row-preview-footer-agree"]),
  entry("eligibility-calculator", "gpui-keyboard", ["scripts", "validation"], "visible", "eligibility", "both", "leading", ["headers-inert", "reserved-slots-inert", "calculator-subject", "navigation-count-preflight-paint-submit-agree"]),
  entry("source-unarmed-down", "gpui-keyboard", ["spine"], "scope-bound", "empty", "none", "leading", ["empty-source-unarmed", "first-down-chooses-first"]),
  entry("scroll-pending-refresh", "gpui-scroll", ["tabs"], "visible", "deep-list", "both", "user-controlled", ["wheel-and-scrollbar", "viewport-anchor-preserved", "no-hover-or-selection-change"]),
  entry("late-reveal-retired", "gpui-scroll", ["tabs"], "visible", "deep-list", "explicit-anchor", "user-controlled", ["old-reveal-cannot-scroll-new-query", "old-reveal-cannot-undo-wheel"]),
  entry("preview-stale-completion", "gpui-keyboard", ["files"], "visible", "replacement", "explicit-anchor", "preserve", ["preview-current-selected-subject"]),
  entry("stale-agent-target", "semantic", ["tabs"], "visible", "tab-domain-hoist", "explicit-anchor", "preserve", ["stale-operation-refused-before-effect", "current-state-unchanged"]),
  entry("retired-window-lifetime", "owned-lifecycle", SEARCH_PROVIDERS, "scope-bound", "owner-retirement", "both", "leading", ["new-window-lifetime", "old-provider-action-frame-refused"]),
  entry("tilde-file-view-isolation", "gpui-text", ["files"], "scope-bound", "directory-browse", "both", "leading", ["full-mini-file-view", "auto-first-user-locked-isolated"]),
  { ...entry("sentence-typing", "gpui-text", SEARCH_PROVIDERS, "completion-time", sentenceScenarios[0]!.id, "both", "preserve",
    ["character-input-preserved", "query-authority-preserved", "natural-sentence-frames", "matching-source-results", "asynchronous-completions-observed", "completion-selection-policy"]),
    bounds: { steps: 1536, requests: 2048, frames: 160, retainedBytes: 4194304, logicalMilliseconds: 12000, wallMilliseconds: 180000 } },
];
export type SearchTiming = "before-initial-commit" | "after-initial-commit" | "after-deliberate-selection" | "after-superseding-query" | "after-owner-retirement";
export type SearchScheduleRecipe = { kind: "primary" } | { kind: "timing"; timing: SearchTiming } |
  { kind: "order" } | { kind: "same-turn" } | { kind: "cohort"; cohort: number; order: readonly string[] } |
  { kind: "terminal"; outcome: "error" | "unavailable" | "disconnect" } |
  { kind: "sentence"; fixture: string; input: string; profile: SearchSentenceProfile; entry: "forward" | "caret-prefix" };
export type SearchTerminalIntent = "automatic" | "explicitAnchor";
export type SearchTerminalOutcome = "error" | "unavailable" | "disconnect";
export interface SearchTerminalReceipt {
  source: SearchProvider; requestedOutcome: SearchTerminalOutcome; intent: SearchTerminalIntent;
  query: { lifetime: number; revision: number; scopeRevision: number };
  selectionArmed: boolean; selectedSemanticId: string | null; provider: SearchProviderRun;
}
export interface SearchInapplicableAssertion { id: string; status: "notApplicable"; proof: false; cause: "separateProviderTimingSchedules" | "separateAtomicBatchSchedules" | "singlePhysicalOwnerExclusiveScopes" }
export interface SearchAssertionApplicability { required: readonly string[]; notApplicable: readonly SearchInapplicableAssertion[]; structuralNotApplicable: "singlePhysicalOwnerExclusiveScopes" | null }
export function searchAssertionApplicability(contract: SearchCase, recipe: SearchScheduleRecipe, providers: readonly SearchProvider[]): SearchAssertionApplicability {
  if (contract.id !== "publisher-orders") return { required: contract.assertions, notApplicable: [], structuralNotApplicable: null };
  if (recipe.kind === "same-turn" && providers.length === 2 && providers.includes("files") && providers.includes("directory"))
    return { required: [], notApplicable: contract.assertions.map<SearchInapplicableAssertion>(id => ({ id, status: "notApplicable", proof: false, cause: "singlePhysicalOwnerExclusiveScopes" })), structuralNotApplicable: "singlePhysicalOwnerExclusiveScopes" };
  const notApplicable: SearchInapplicableAssertion[] = [];
  if (recipe.kind !== "timing") notApplicable.push({ id: "all-provider-timings", status: "notApplicable", proof: false, cause: "separateProviderTimingSchedules" });
  if (recipe.kind !== "same-turn") notApplicable.push({ id: "same-turn-completion", status: "notApplicable", proof: false, cause: "separateAtomicBatchSchedules" });
  return { required: contract.assertions.filter(id => !notApplicable.some(assertion => assertion.id === id)), notApplicable, structuralNotApplicable: null };
}
export interface SearchSchedule {
  id: string; caseId: string; providers: readonly SearchProvider[]; events: readonly string[]; recipe: SearchScheduleRecipe; bounds: SearchBounds;
  assertions: readonly string[]; notApplicableAssertions: readonly SearchInapplicableAssertion[];
  structuralNotApplicable: "singlePhysicalOwnerExclusiveScopes" | null;
  terminalIntents: { required: readonly SearchTerminalIntent[]; notApplicable: readonly { intent: "explicitAnchor"; status: "notApplicable"; proof: false; cause: "separateTerminalIntentSchedules" }[] } | null;
}
const TIMINGS: readonly SearchTiming[] = ["before-initial-commit", "after-initial-commit", "after-deliberate-selection", "after-superseding-query", "after-owner-retirement"];
function permutations<T>(values: readonly T[]): T[][] {
  if (values.length < 2) return [[...values]];
  return values.flatMap((value, index) => permutations(values.filter((_, i) => i !== index)).map(rest => [value, ...rest]));
}
export function generateSearchSchedules(cases: readonly SearchCase[] = SEARCH_CASES): SearchSchedule[] {
  const schedules: SearchSchedule[] = [];
  const add = (c: SearchCase, suffix: string, providers: readonly SearchProvider[], events: string[], recipe: SearchScheduleRecipe = { kind: "primary" }) => {
    const applicability = searchAssertionApplicability(c, recipe, providers);
    // Ordered comparisons include both intents, source switches, and final normalization.
    const units = recipe.kind === "cohort" || recipe.kind === "order" || recipe.kind === "same-turn" ? 4 : recipe.kind === "terminal" || (recipe.kind === "timing" && ["after-deliberate-selection", "after-owner-retirement"].includes(recipe.timing)) ? 3 : recipe.kind === "primary" ? 1 : 2;
    schedules.push({ id: `${c.id}/${suffix}`, caseId: c.id, providers, events, recipe, assertions: applicability.required, notApplicableAssertions: applicability.notApplicable, structuralNotApplicable: applicability.structuralNotApplicable,
      terminalIntents: c.id !== "provider-terminal-errors" ? null : recipe.kind === "primary" ?
        { required: ["automatic"], notApplicable: [{ intent: "explicitAnchor", status: "notApplicable", proof: false, cause: "separateTerminalIntentSchedules" }] } :
        { required: ["automatic", "explicitAnchor"], notApplicable: [] },
      bounds: recipe.kind === "primary" || recipe.kind === "sentence" ? { ...c.bounds } : { ...bounds, steps: bounds.steps * units, requests: bounds.requests * units,
        frames: bounds.frames * units, retainedBytes: bounds.retainedBytes * units,
        logicalMilliseconds: bounds.logicalMilliseconds * units, wallMilliseconds: bounds.wallMilliseconds * units } });
  };
  for (const c of cases) {
    if (c.id === "sentence-typing") {
      for (const scenario of sentenceScenarios) for (const profile of SEARCH_SENTENCE_PROFILES) {
        // Forward Space from an empty launcher opens Day Page. Literal leading
        // spaces are search edits, not that shortcut's text-entry intent.
        const entry = scenario.input.startsWith(" ") ? "caret-prefix" : "forward";
        add(c, `${scenario.id}/${profile}`, c.providers, ["prepare", entry, "character-input", profile, "release-overlapping-work", "assert-every-frame", "final-matching-source"],
          { kind: "sentence", fixture: scenario.id, input: scenario.input, profile, entry });
      }
      continue;
    }
    add(c, "primary", c.providers, ["prepare", "input", "commit", "intent", "observe-held", "release", "natural-frame", "assert"]);
    if (c.id === "publisher-orders") {
      for (const provider of c.providers) for (const timing of TIMINGS) add(c, `${provider}/${timing}`, [provider], ["prepare", "input", timing, `release:${provider}`, "natural-frame", "assert"], { kind: "timing", timing });
      for (let a = 0; a < c.providers.length; a++) for (let b = a + 1; b < c.providers.length; b++) {
        const pair = [c.providers[a]!, c.providers[b]!];
        for (const order of [pair, [...pair].reverse()]) add(c, order.join("-then-"), order, ["prepare", "input", ...order.flatMap(provider => [`release:${provider}`, "natural-frame", "assert-intermediate"]), "assert-final"], { kind: "order" });
        add(c, `${pair.join("+")}/same-turn`, pair, ["prepare", "input", "release-same-turn", "natural-frame", "assert"], { kind: "same-turn" });
      }
      const cohorts = [["tab-hoist", "brain-replacement", "files-handoff"], ["catalogue-arrival", "passive-arrival", "budget-competition"], ["selected-removal", "replacement", "other-source-arrival"], ["source-change", "scope-change", "compatible-work-reuse"]];
      const cohortProviders: readonly (readonly SearchProvider[])[] = [["tabs", "brain-semantic", "files"], ["scripts", "notes", "history"], ["tabs", "notes", "todos"], ["files", "directory", "spine"]];
      for (const [index, cohort] of cohorts.entries()) for (const order of permutations(cohort)) add(c, `cohort-${index}/${order.join("-then-")}`, cohortProviders[index]!, ["prepare", "input", ...order, "assert-every-frame"], { kind: "cohort", cohort: index, order });
    }
    if (c.id === "provider-terminal-errors") for (const provider of c.providers) for (const outcome of ["error", "unavailable", "disconnect"] as const) add(c, `${provider}/${outcome}`, [provider], ["prepare", "input", outcome, "release", "terminal-observation", "assert"], { kind: "terminal", outcome });
  }
  return schedules;
}
export function searchInventoryIssues(cases: readonly SearchCase[], schedules: readonly SearchSchedule[]): string[] {
  const issues: string[] = [];
  if (cases.length !== SEARCH_CASES.length || new Set(cases.map(c => c.id)).size !== SEARCH_CASES.length) issues.push("complete-case-inventory-required");
  if (new Set(schedules.map(s => s.id)).size !== schedules.length) issues.push("duplicate-schedule");
  for (const declared of SEARCH_CASES) {
    const supplied = cases.find(c => c.id === declared.id);
    if (!supplied) issues.push(`missing-case:${declared.id}`);
    else for (const assertion of declared.assertions) if (!supplied.assertions.includes(assertion)) issues.push(`missing-assertion:${declared.id}:${assertion}`);
  }
  const suppliedSchedules = new Set(schedules.map(s => s.id));
  for (const schedule of generateSearchSchedules()) if (!suppliedSchedules.has(schedule.id)) issues.push(`missing-schedule:${schedule.id}`);
  for (const c of cases) {
    if (!c.assertions.length || !c.evidence.length || !c.providers.length || !schedules.some(s => s.caseId === c.id)) issues.push(`uncovered-case:${c.id}`);
    if (Object.values(c.bounds).some(n => !Number.isSafeInteger(n) || n <= 0) || c.bounds.requests > 4096 || c.bounds.frames > 2048) issues.push(`invalid-bounds:${c.id}`);
  }
  for (const provider of SEARCH_PROVIDERS) if (!cases.some(c => c.providers.includes(provider))) issues.push(`missing-provider:${provider}`);
  for (const schedule of schedules) if (!cases.some(c => c.id === schedule.caseId)) issues.push(`unknown-case:${schedule.caseId}`);
  return issues;
}
export interface SearchShard { index: number; schedules: readonly SearchSchedule[]; bounds: SearchBounds }
export function searchScheduleComparisonGroup(schedule: SearchSchedule): string | null {
  if (schedule.recipe.kind === "order" || schedule.recipe.kind === "same-turn") return `pair:${[...schedule.providers].sort().join("+")}`;
  return schedule.recipe.kind === "cohort" ? `cohort:${schedule.recipe.cohort}` : null;
}
/** Keep comparison families on one owned fixture root; reserve lifecycle and safety first. */
export function partitionSearchSchedules(schedules: readonly SearchSchedule[]): SearchShard[] {
  const capacity: SearchBounds = { steps: Number.MAX_SAFE_INTEGER, requests: OWNED_EVALUATION_LIMITS.maxRequests - 128,
    frames: OWNED_EVALUATION_LIMITS.maxFrames - 32, retainedBytes: 64 * 1024 * 1024,
    logicalMilliseconds: 599000, wallMilliseconds: OWNED_EVALUATION_LIMITS.maxLifetimeMs - 30000 };
  const keys: readonly (keyof SearchBounds)[] = ["steps", "requests", "frames", "retainedBytes", "logicalMilliseconds", "wallMilliseconds"];
  const bundles = new Map<string, SearchSchedule[]>();
  for (const schedule of schedules) {
    if (keys.some(key => !Number.isSafeInteger(schedule.bounds[key]) || schedule.bounds[key] < 1 || schedule.bounds[key] > capacity[key])) throw new Error(`unadmittable-search-schedule:${schedule.id}`);
    const key = searchScheduleComparisonGroup(schedule) ?? schedule.id;
    const bundle = bundles.get(key) ?? []; bundle.push(schedule); bundles.set(key, bundle);
  }
  const shards: SearchShard[] = [];
  let current: SearchSchedule[] = [];
  let total: SearchBounds = { steps: 0, requests: 0, frames: 0, retainedBytes: 0, logicalMilliseconds: 0, wallMilliseconds: 0 };
  const flush = () => {
    if (!current.length) return;
    shards.push({ index: shards.length, schedules: current, bounds: total });
    current = []; total = { steps: 0, requests: 0, frames: 0, retainedBytes: 0, logicalMilliseconds: 0, wallMilliseconds: 0 };
  };
  for (const [id, bundle] of bundles) {
    const bounds: SearchBounds = { steps: 0, requests: 0, frames: 0, retainedBytes: 0, logicalMilliseconds: 0, wallMilliseconds: 0 };
    for (const schedule of bundle) for (const key of keys) bounds[key] += schedule.bounds[key];
    if (keys.some(key => bounds[key] > capacity[key])) throw new Error(`unadmittable-search-comparison:${id}`);
    // Comparison families cannot cross owned fixture roots; isolate rather than split them.
    // Request counts are a conservative packing proxy, never a wire-byte certificate.
    if (searchScheduleComparisonGroup(bundle[0]!) !== null || bounds.requests > OUTPUT_PACKING.maxIndependentRequests) {
      flush(); shards.push({ index: shards.length, schedules: bundle, bounds }); continue;
    }
    if (current.length && (current.length + bundle.length > OUTPUT_PACKING.maxIndependentSchedules ||
        total.requests + bounds.requests > OUTPUT_PACKING.maxIndependentRequests || keys.some(key => total[key] + bounds[key] > capacity[key]))) flush();
    current.push(...bundle);
    for (const key of keys) total[key] += bounds[key];
  }
  flush();
  return shards;
}
export function searchContractSpec() {
  const schedules = generateSearchSchedules();
  return { version: SEARCH_CONTRACT_VERSION, fixtureId: SEARCH_FIXTURE_ID, cases: SEARCH_CASES, providers: SEARCH_PROVIDERS, schedules,
    caseSetHash: createHash("sha256").update(JSON.stringify({ version: SEARCH_CONTRACT_VERSION, cases: SEARCH_CASES, schedules })).digest("hex"),
    reduction: { count: 0, reason: "No behavioral commutativity proof; no schedules reduced" },
    admission: { ...OWNED_EVALUATION_LIMITS, reserveRequests: 128, reserveFrames: 32, reserveLifetimeMs: 30000, outputPacking: OUTPUT_PACKING,
      shards: partitionSearchSchedules(schedules).map(shard => ({ index: shard.index, scheduleIds: shard.schedules.map(schedule => schedule.id), bounds: shard.bounds })) },
    issues: searchInventoryIssues(SEARCH_CASES, schedules), nativeExclusions: ["live-provider-IO", "WindowServer", "native-focus", "OS-IME", "global-input", "external-app-effects"] };
}
export interface SearchAuxiliaryExecution { kind: "singlePhysicalOwnerDrain"; executed: boolean; pass: boolean; assertions: { id: string; pass: boolean }[] }
export interface SearchScheduleResult {
  id: string; caseId: string; status: "passed" | "failed" | "blocked" | "notApplicable"; executed: boolean; issues: string[]; assertions: { id: string; pass: boolean }[];
  notApplicableAssertions?: readonly SearchInapplicableAssertion[]; auxiliaryExecution?: SearchAuxiliaryExecution; evidence?: unknown;
  terminalReceipts?: SearchTerminalReceipt[];
}
export interface SearchCoverage {
  generated: number; eligible: number; executed: number; auxiliaryExecuted: number; notApplicable: number; reduced: number; blocked: number; failed: number; passed: number;
  complete: boolean; results: SearchScheduleResult[];
  caseCriteria: { caseId: string; required: readonly string[]; proved: string[]; complete: boolean }[];
  terminalCoverage: { required: number; proved: number; complete: boolean; factors: { source: SearchProvider; outcome: SearchTerminalOutcome; intent: SearchTerminalIntent; scheduleIds: string[] }[] };
}
export function accountSearchCoverage(schedules: readonly SearchSchedule[], results: readonly SearchScheduleResult[]): SearchCoverage {
  const byId = new Map<string, SearchScheduleResult>();
  const canonical = generateSearchSchedules();
  const declared = new Map(schedules.map(schedule => [schedule.id, schedule]));
  const canonicalById = new Map(canonical.map(schedule => [schedule.id, schedule]));
  const terminalProof = new Map<string, string[]>();
  if (declared.size !== schedules.length) throw new Error("invalid-schedule-accounting");
  for (const result of results) {
    const schedule = declared.get(result.id); const expected = canonicalById.get(result.id);
    if (byId.has(result.id) || !schedule || !expected || schedule.caseId !== result.caseId || JSON.stringify(schedule) !== JSON.stringify(expected)) throw new Error("invalid-schedule-accounting");
    const required = expected.assertions; const notApplicable = result.notApplicableAssertions ?? [];
    const applicabilityMatches = Array.isArray(notApplicable) && notApplicable.length === expected.notApplicableAssertions.length &&
      expected.notApplicableAssertions.every(assertion => notApplicable.some(actual => actual.id === assertion.id && actual.status === "notApplicable" && actual.proof === false && actual.cause === assertion.cause));
    if (result.status === "passed" && (!result.executed || !required.length || required.some(id => !result.assertions.some(a => a.id === id && a.pass)) ||
        !applicabilityMatches || result.assertions.some(assertion => expected.notApplicableAssertions.some(item => item.id === assertion.id)) ||
        !result.assertions.length || result.assertions.some(a => !a.pass) || result.issues.length)) throw new Error("unsupported-pass-claim");
    if (result.status === "notApplicable" && (!expected.structuralNotApplicable || result.executed || result.assertions.length || result.issues.length || !applicabilityMatches ||
        result.auxiliaryExecution?.kind !== "singlePhysicalOwnerDrain" || !result.auxiliaryExecution.executed || !result.auxiliaryExecution.pass ||
        !result.auxiliaryExecution.assertions.length || result.auxiliaryExecution.assertions.some(assertion => !assertion.pass))) throw new Error("unsupported-inapplicability-claim");
    if (result.status === "passed" && expected.terminalIntents) {
      const outcomes: readonly SearchTerminalOutcome[] = expected.recipe.kind === "terminal" ? [expected.recipe.outcome] : ["error", "unavailable", "disconnect"];
      const receipts = result.terminalReceipts ?? [];
      const needed = expected.providers.flatMap(source => outcomes.flatMap(outcome => expected.terminalIntents!.required.map(intent => `${source}:${outcome}:${intent}`)));
      const observed = new Set<string>();
      for (const receipt of receipts) {
        const key = `${receipt.source}:${receipt.requestedOutcome}:${receipt.intent}`; const run = receipt.provider;
        const synchronous = receipt.source === "brain-lexical" || receipt.source === "brain-inbox";
        const nativeTerminal = synchronous && receipt.requestedOutcome === "disconnect" ?
          run?.kind === "sourceChange" && run.capabilityRefusal === "synchronous_source_has_no_worker" && run.admissionApplied === false && run.outcome == null :
          run?.kind === (synchronous ? "synchronousRead" : "worker") && run.state === (receipt.requestedOutcome === "unavailable" ? "unavailable" : "failed") &&
          run.outcome === (receipt.requestedOutcome === "disconnect" ? "disconnected" : receipt.requestedOutcome) &&
          (!synchronous || Number.isSafeInteger(run.originAdmissionId) && run.originAdmissionId! > 0);
        if (!needed.includes(key) || observed.has(key) || run?.source !== receipt.source || !Number.isSafeInteger(run.id) || run.id <= 0 || run.payloadPhase !== 1 ||
            !nativeTerminal || !receipt.query || ![receipt.query.lifetime, receipt.query.revision, receipt.query.scopeRevision].every(value => Number.isSafeInteger(value) && value >= 0) ||
            (receipt.intent === "explicitAnchor" && (!receipt.selectionArmed || !/^main-list-row:v2:[a-f0-9]{64}$/.test(receipt.selectedSemanticId ?? "")))) throw new Error("unsupported-terminal-intent-proof");
        observed.add(key); const links = terminalProof.get(key) ?? []; links.push(result.id); terminalProof.set(key, links);
      }
      if (observed.size !== needed.length || needed.some(key => !observed.has(key))) throw new Error("missing-terminal-intent-proof");
    }
    byId.set(result.id, result);
  }
  const accounted: SearchScheduleResult[] = schedules.map(s => byId.get(s.id) ?? { id: s.id, caseId: s.caseId, status: "blocked", executed: false, issues: ["missing-execution-receipt"], assertions: [], notApplicableAssertions: s.notApplicableAssertions });
  const terminalFactors = SEARCH_PROVIDERS.flatMap(source => (["error", "unavailable", "disconnect"] as const).flatMap(outcome =>
    (["automatic", "explicitAnchor"] as const).map(intent => ({ source, outcome, intent, scheduleIds: terminalProof.get(`${source}:${outcome}:${intent}`) ?? [] }))));
  const terminalCoverage = { required: terminalFactors.length, proved: terminalFactors.filter(factor => factor.scheduleIds.length > 0).length,
    complete: terminalFactors.every(factor => factor.scheduleIds.length > 0), factors: terminalFactors };
  const caseCriteria = SEARCH_CASES.map(contract => {
    const proved = [...new Set(accounted.filter(result => result.caseId === contract.id && result.status === "passed")
      .flatMap(result => result.assertions.filter(assertion => assertion.pass && contract.assertions.includes(assertion.id)).map(assertion => assertion.id)))];
    return { caseId: contract.id, required: contract.assertions, proved, complete: contract.assertions.every(id => proved.includes(id)) && (contract.id !== "provider-terminal-errors" || terminalCoverage.complete) };
  });
  return { generated: schedules.length, eligible: canonical.filter(schedule => !schedule.structuralNotApplicable).length, executed: accounted.filter(r => r.executed).length,
    auxiliaryExecuted: accounted.filter(r => r.auxiliaryExecution?.executed).length, notApplicable: accounted.filter(r => r.status === "notApplicable").length, reduced: 0,
    blocked: accounted.filter(r => r.status === "blocked").length, failed: accounted.filter(r => r.status === "failed").length,
    passed: accounted.filter(r => r.status === "passed").length, complete: declared.size === canonical.length && canonical.every(schedule => declared.has(schedule.id)) &&
      accounted.every(r => r.status === "passed" || r.status === "notApplicable") && caseCriteria.every(item => item.complete), results: accounted, caseCriteria, terminalCoverage };
}
