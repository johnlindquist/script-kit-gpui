import type { OwnedCleanup } from "../../agentic/artifact-lifecycle.ts";
import { resolveReceiptDetails } from "./receipt-artifact.ts";

export type ProofLevel = "domain" | "GPUI-test-platform" | "owned-production-runtime" | "hidden-native-framebuffer" | "native-AppKit";
export const STORY_TEST_PREFIX = "production_story_";
export const PRODUCTION_STORIES = [
  { id: "launcher-filtering", testLeaf: "production_story_launcher_filtering_policy", level: "domain" },
  { id: "prompt-actions", testLeaf: "production_story_prompt_action_selection", level: "GPUI-test-platform" },
  { id: "prompt-buttons", testLeaf: "production_story_prompt_button_dispatch", level: "GPUI-test-platform" },
  { id: "ai-failure-retry", testLeaf: "production_story_ai_failure_retry_recovery", level: "domain" },
  { id: "ai-stop-cancel", testLeaf: "production_story_ai_stop_cancel", level: "domain" },
  { id: "notes-edit-selection", testLeaf: "production_story_notes_edit_and_selection", level: "GPUI-test-platform" },
  { id: "dictation-target-refusal", testLeaf: "production_story_dictation_target_refusal", level: "domain" },
  { id: "conversation-portal", testLeaf: "production_story_conversation_portal_contract", level: "domain" },
  { id: "pi-reply-identity", testLeaf: "production_story_pi_reply_identity", level: "domain" },
] as const;
export const CORE_JOURNEYS = [
  "launcher-ranking-provider", "choice-prompt-completion", "editable-prompt-validation", "actions-popup-activation",
  "notes-day-roundtrip", "conversation-recovery-stop", "dictation-delivery-refusal", "theme-publication-revert", "close-reopen-stale",
] as const;
export type CoreJourneyId = typeof CORE_JOURNEYS[number];
export interface LibtestSelection { names: string[]; issues: string[] }
export interface LibtestObservation {
  exitCode: number; startedCount: number; passedCount: number; failedCount: number; ignoredCount: number; measuredCount: number;
  tests: Array<{ name: string; status: "ok" | "FAILED" | "ignored" }>; issues: string[];
}

export function selectStoryTests(output: string): LibtestSelection {
  const names: string[] = [];
  const issues: string[] = [];
  for (const line of output.split(/\r?\n/)) {
    if (!line.trim()) continue;
    const test = /^([A-Za-z0-9_:]+): test$/.exec(line);
    if (test) names.push(test[1]!);
    else if (!/^\d+ tests?, \d+ benchmarks?$/.test(line)) issues.push("unexpected_libtest_listing_record");
  }
  if (names.length !== PRODUCTION_STORIES.length) issues.push("story_selection_count_mismatch");
  if (new Set(names).size !== names.length) issues.push("duplicate_listed_test");
  for (const story of PRODUCTION_STORIES) {
    if (names.filter(name => name === story.testLeaf || name.endsWith(`::${story.testLeaf}`)).length !== 1)
      issues.push(`story_selection_missing_or_ambiguous:${story.id}`);
  }
  for (const name of names) {
    if (!PRODUCTION_STORIES.some(story => name === story.testLeaf || name.endsWith(`::${story.testLeaf}`))) issues.push("unreviewed_story_test");
  }
  return { names: names.sort(), issues };
}

export function observeStoryTests(output: string, exitCode: number, expectedNames: readonly string[]): LibtestObservation {
  const starts = [...output.matchAll(/^running (\d+) tests?$/gm)];
  const summaries = [...output.matchAll(/^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; \d+ filtered out;/gm)];
  const tests = [...output.matchAll(/^test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored)(?: .*)?$/gm)]
    .map(match => ({ name: match[1]!, status: match[2]! as "ok" | "FAILED" | "ignored" }));
  const summary = summaries.length === 1 ? summaries[0] : undefined;
  const observation: LibtestObservation = { exitCode, startedCount: starts.length === 1 ? Number(starts[0]![1]) : -1,
    passedCount: summary ? Number(summary[1]) : -1, failedCount: summary ? Number(summary[2]) : -1,
    ignoredCount: summary ? Number(summary[3]) : -1, measuredCount: summary ? Number(summary[4]) : -1, tests, issues: [] };
  if (exitCode !== 0) observation.issues.push("libtest_nonzero_exit");
  if (observation.startedCount !== expectedNames.length || observation.startedCount <= 0) observation.issues.push("libtest_started_count_mismatch");
  if (summaries.length !== 1) observation.issues.push("libtest_terminal_summary_missing_or_duplicate");
  if (observation.passedCount !== expectedNames.length || observation.failedCount !== 0 || observation.ignoredCount !== 0 || observation.measuredCount !== 0)
    observation.issues.push("libtest_terminal_counts_not_passing");
  const observedNames = tests.map(test => test.name).sort();
  const expected = [...expectedNames].sort();
  if (observedNames.length !== expected.length || new Set(observedNames).size !== observedNames.length || observedNames.some((name, index) => name !== expected[index]))
    observation.issues.push("libtest_executed_identity_mismatch");
  if (tests.some(test => test.status !== "ok")) observation.issues.push("libtest_nonpassing_case");
  return observation;
}

export function completedGpuiDispatchIssues(response: Record<string, unknown>): string[] {
  const issues: string[] = [];
  if (response.type !== "simulateGpuiEventResult") issues.push("wrong_response_type");
  if (typeof response.requestId !== "string" || !response.requestId) issues.push("missing_request_id");
  if (response.success !== true) issues.push("dispatch_failed");
  if (response.dispatchCompleted !== true || response.dispatchScheduled !== false) issues.push("dispatch_not_completed");
  if (response.dispatchPath !== "exact_handle" && !(response.dispatchPath === "exact_handle_deferred" && response.wasDeferred === true)) issues.push("dispatch_not_exact");
  if (typeof response.resolvedWindowId !== "string" || !response.resolvedWindowId) issues.push("missing_resolved_window");
  return issues;
}

export function aggregateCleanup(cleanups: readonly OwnedCleanup[]): OwnedCleanup {
  if (!cleanups.length) return { resourcesAcquired: false, processExited: true, processGroupExited: true, streamsDrained: true,
    logWriterClosed: true, ownedWindowsClosed: true, referencesFinalized: true, closed: true, survivors: [], failureCodes: [] };
  return { resourcesAcquired: cleanups.some(cleanup => cleanup.resourcesAcquired), processExited: cleanups.every(cleanup => cleanup.processExited),
    processGroupExited: cleanups.every(cleanup => cleanup.processGroupExited), streamsDrained: cleanups.every(cleanup => cleanup.streamsDrained),
    logWriterClosed: cleanups.every(cleanup => cleanup.logWriterClosed), ownedWindowsClosed: cleanups.some(cleanup => cleanup.ownedWindowsClosed == null) ? null : cleanups.every(cleanup => cleanup.ownedWindowsClosed),
    referencesFinalized: cleanups.every(cleanup => cleanup.referencesFinalized), closed: cleanups.every(cleanup => cleanup.closed),
    survivors: cleanups.flatMap(cleanup => cleanup.survivors), failureCodes: cleanups.flatMap(cleanup => cleanup.failureCodes) };
}

export const SDK_RUNTIME_ASSERTIONS = [
  "submit:actual-sdk-received-payload-once", "cancel:actual-sdk-received-payload-once",
  "full:truthful-completion-failure", "full:actual-sdk-received-payload-once",
  "disconnected:truthful-completion-failure", "disconnected:retired-without-delivery",
  ...["submit", "cancel", "full", "disconnected"].flatMap(caseId => [
    `${caseId}:independent-rpc-correlation`, `${caseId}:sdk-rpc-does-not-complete-prompt`,
  ]),
  "full:failure-preserves-rpc", "disconnected:failure-preserves-rpc",
] as const;

export const FOOTER_RUNTIME_ASSERTIONS = [
  "distinct_real_footer_owners", "theme_publication_notifies_both_footer_lifetimes",
  "both_footer_lifetimes_apply_reverted_theme", "current_enabled_action_executes_one_real_owner_effect",
  "retained_stale_action_executes_zero_effects", "secondary_refresh_cannot_overwrite_main_state",
  "teardown_recreates_distinct_footer_lifetime", "retired_binding_cannot_route_to_recreated_owner",
  "native_footer_pixels_explicitly_excluded",
] as const;

export function productionStoryReceiptIssues(receipt: Record<string, unknown>): string[] {
  try { receipt = resolveReceiptDetails(receipt); }
  catch (error) { return [error instanceof Error ? error.message : String(error)]; }
  const issues: string[] = [];
  const library = receipt.library as { selection?: LibtestSelection; execution?: LibtestObservation } | undefined;
  if (!library?.selection || !library.execution || library.selection.issues.length || library.execution.issues.length ||
      library.selection.names.length !== PRODUCTION_STORIES.length || library.execution.startedCount !== PRODUCTION_STORIES.length)
    issues.push("nonzero_exact_library_story_proof_required");
  if (receipt.lane !== "library") {
    const journeys = receipt.journeys as Array<{ id: string; proofLevel?: string; assertions: Array<{ id?: string; pass: boolean }>; pass: boolean }> | undefined;
    if (!journeys || CORE_JOURNEYS.some(id => journeys.filter(journey => journey.id === id && journey.pass && journey.assertions.length > 0 && journey.assertions.every(assertion => assertion.pass)).length !== 1))
      issues.push("complete_core_runtime_journeys_required");
    const sdk = journeys?.filter(journey => journey.id === "sdk-prompt-roundtrip") ?? [];
    if (sdk.length !== 1 || sdk[0]!.proofLevel !== "owned-production-runtime" || sdk[0]!.pass !== true ||
        !Array.isArray(sdk[0]!.assertions) || !sdk[0]!.assertions.every(assertion => assertion && assertion.pass === true) ||
        SDK_RUNTIME_ASSERTIONS.some(id => sdk[0]!.assertions.filter(assertion => assertion.id === id && assertion.pass).length !== 1))
      issues.push("complete_sdk_runtime_journey_required");
    const footer = journeys?.filter(journey => journey.id === "footer-owner-isolation") ?? [];
    if (footer.length !== 1 || footer[0]!.proofLevel !== "owned-production-runtime" || footer[0]!.pass !== true ||
        !Array.isArray(footer[0]!.assertions) || !footer[0]!.assertions.every(assertion => assertion && assertion.pass === true) ||
        FOOTER_RUNTIME_ASSERTIONS.some(id => footer[0]!.assertions.filter(assertion => assertion.id === id && assertion.pass).length !== 1))
      issues.push("complete_footer_runtime_journey_required");
  } else if (receipt.provesRuntimeBehavior !== false || receipt.evidenceClass !== "UNIT_BEHAVIOR") issues.push("library_lane_cannot_claim_native_or_runtime_proof");
  const cleanup = receipt.cleanup as OwnedCleanup | undefined;
  if (!cleanup?.closed || !cleanup.processExited || !cleanup.processGroupExited || !cleanup.streamsDrained || !cleanup.logWriterClosed || !cleanup.referencesFinalized || cleanup.survivors.length)
    issues.push("complete_owned_cleanup_required");
  return issues;
}
