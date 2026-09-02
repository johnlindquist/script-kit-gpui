import { resolve } from "node:path";
import type { ArtifactReference } from "../agentic/build-artifact.ts";
import type { OutputClaim } from "../agentic/artifact-lifecycle.ts";
import { DriverCommandRefused, DriverLifecycleError, unknownOwnedCleanup, type Json } from "./driver.ts";
import { OwnedEvaluationClient, EvaluationContractError, publicationCausalityIssues } from "./lib/owned-evaluation.ts";
import type { AutomationInstance } from "./lib/target-identity.ts";
import type { RuntimeJourneyReceipt } from "./design.ts";

export const FOOTER_JOURNEY_ID = "footer-owner-isolation" as const;
export const FOOTER_JOURNEY_FIXTURES = ["main.script-list", "agent-chat.detached.retryable-failure", "secondary.footer"] as const;
const ROOT = resolve(import.meta.dir, "../..");
const STALE_CODES = ["target_not_mounted", "stale_target_identity", "stale_frame_identity", "stale_window_generation"];

/** Real Main + detached Agent owners; equal-config/cache cases live in the existing registry libtests. */
export async function runFooterOwnershipJourney(reference: ArtifactReference, claim: OutputClaim): Promise<RuntimeJourneyReceipt> {
  const receipt: RuntimeJourneyReceipt = {
    id: FOOTER_JOURNEY_ID, proofLevel: "owned-production-runtime", pass: false,
    fixtureIds: [...FOOTER_JOURNEY_FIXTURES], assertions: [], frames: [], effects: [], cleanup: unknownOwnedCleanup(false),
  };
  const check = (id: string, pass: boolean) => {
    receipt.assertions.push({ id, pass });
    if (!pass) throw new EvaluationContractError(id);
  };
  let client: OwnedEvaluationClient | undefined;
  try {
    // This journey exercises two real popup-footer owners. The glass profile
    // intentionally uses an inline Agent rail instead of a popup-footer owner.
    client = await OwnedEvaluationClient.launch(ROOT, reference, claim, receipt.fixtureIds,
      "current-content", { nativeGlass: "disabled" });
    receipt.effects.push({ kind: "fixture-profile-selected", name: "nativeGlass", status: "disabled",
      source: "sealed-owned-evaluation-launch:SCRIPT_KIT_DEBUG_NO_GLASS=1",
      rawNativeFooterPixelsExcluded: true,
      scope: "GPUI footer overlay only; AppKit footer material, native glyphs and child peers excluded" });
    const active = client;
    const capture = async (target: AutomationInstance, phase: string) => {
      const result = await active.captureFrame(target, false);
      receipt.frames.push(result.frame);
      const state = result.state;
      check(`${phase}:footer_owner_observed`, state.fixtureObservation?.family === "footer" &&
        Number.isSafeInteger(state.fixtureObservation.completedActionCount));
      receipt.effects.push({ phase, target, frame: result.frame, observation: state.fixtureObservation,
        elements: result.elements.elements, rawNativeFooterPixelsExcluded: true,
        captureScope: "GPUI footer overlay only; AppKit footer material, native glyphs and child peers excluded" });
      return state;
    };
    const main = await active.mount("main.script-list");
    await active.frame(main);
    const agent = await active.mount("agent-chat.detached.retryable-failure");
    const agentFrame = await active.frame(agent);
    check("detached_parent_keeps_conversation_identity", agentFrame.target.appViewVariant === "agentChatChat");
    const mainFooter = await active.mount("secondary.footer", main);
    const agentFooter = await active.mount("secondary.footer", agent);
    const mainBefore = await capture(mainFooter, "main-before");
    const agentBefore = await capture(agentFooter, "agent-before");
    // live_gpui_target_identity exposes FooterBinding.host_generation as surfaceGeneration;
    // targetGeneration is a per-target metadata revision, initialized to 1 for every window.
    check("distinct_real_footer_owners", main.id !== agent.id && mainFooter.id !== agentFooter.id &&
      mainBefore.fixtureObservation.owner === main.id && agentBefore.fixtureObservation.owner === agent.id &&
      mainBefore.targetIdentity.surfaceGeneration !== agentBefore.targetIdentity.surfaceGeneration);

    const baselineToken = mainBefore.liveTheme.resolved.values.find((value: Json) => value.tokenId === "theme.colors.accent.selected")?.value;
    check("baseline_theme_token_observed", Number.isSafeInteger(baselineToken));
    const editedValue = baselineToken === 0x72c1a8 ? 0x7281a8 : 0x72c1a8;
    const publication = await active.applyTheme(mainBefore.targetIdentity.themeRevision,
      [{ tokenId: "theme.colors.accent.selected", value: editedValue }]);
    receipt.effects.push({ phase: "theme-published", publication, rawNativeFooterPixelsExcluded: true });
    try {
      check("theme_publication_notifies_both_footer_lifetimes", publicationCausalityIssues(publication, [mainFooter, agentFooter]).length === 0);
      const mainEdited = await capture(mainFooter, "main-themed");
      const agentEdited = await capture(agentFooter, "agent-themed");
      for (const [name, before, after] of [["main", mainBefore, mainEdited], ["agent", agentBefore, agentEdited]] as const) {
        check(`${name}:theme_refreshes_unchanged_footer_config`,
          after.fixtureObservation.appliedThemeRevision === publication.revision &&
          after.fixtureObservation.semanticRevision === before.fixtureObservation.semanticRevision &&
          after.fixtureObservation.presentationRevision > before.fixtureObservation.presentationRevision &&
          after.fixtureObservation.completedActionCount === before.fixtureObservation.completedActionCount &&
          after.liveTheme.resolved.values.some((value: Json) => value.tokenId === "theme.colors.accent.selected" && value.value === editedValue));
      }
    } finally {
      const reverted = await active.revertTheme(publication.revision);
      receipt.effects.push({ phase: "theme-reverted", publication: reverted, rawNativeFooterPixelsExcluded: true });
      const mainRestored = await capture(mainFooter, "main-restored");
      const agentRestored = await capture(agentFooter, "agent-restored");
      check("both_footer_lifetimes_apply_reverted_theme", [mainRestored, agentRestored].every(state =>
        state.fixtureObservation.appliedThemeRevision === reverted.revision &&
        state.liveTheme.resolved.values.some((value: Json) => value.tokenId === "theme.colors.accent.selected" && value.value === baselineToken)));
    }

    const mainStable = await active.inspect(mainFooter);
    const mainElements = await active.query(mainFooter, "elements");
    const agentStable = await active.inspect(agentFooter);
    const agentOwnerBefore = await active.inspect(agent);
    const controls = (await active.query(agentFooter, "elements")).elements;
    check("current_retry_is_enabled", Array.isArray(controls) && controls.some((node: Json) =>
      node.semanticId === "footer-action:retry" && node.selectable === true && !node.actionDisabled));
    const retained = { type: "batch", target: agentFooter, expected: agentStable.targetIdentity,
      commands: [{ type: "selectBySemanticId", semanticId: "footer-action:retry", submit: true }], options: { stopOnError: true, timeout: 5000 } };
    receipt.effects.push(await active.act(agentFooter, { type: "select", semanticId: "footer-action:retry", submit: true }));
    await active.frame(agent);
    const agentAfter = await capture(agentFooter, "agent-after-retry");
    const agentOwnerAfter = await active.inspect(agent);
    check("current_enabled_action_executes_one_real_owner_effect",
      agentAfter.fixtureObservation.completedActionCount === agentStable.fixtureObservation.completedActionCount + 1 &&
      agentOwnerAfter.fixtureObservation.startedTurns === agentOwnerBefore.fixtureObservation.startedTurns + 1);
    let refused = false;
    try { await active.driver.request(retained); }
    catch (error) { refused = error instanceof DriverCommandRefused && STALE_CODES.includes(error.code); }
    const afterStale = await active.inspect(agentFooter);
    check("retained_stale_action_executes_zero_effects", refused &&
      afterStale.fixtureObservation.completedActionCount === agentAfter.fixtureObservation.completedActionCount &&
      (await active.inspect(agent)).fixtureObservation.startedTurns === agentOwnerAfter.fixtureObservation.startedTurns);
    const mainAfter = await active.inspect(mainFooter);
    check("secondary_refresh_cannot_overwrite_main_state",
      JSON.stringify(mainAfter.fixtureObservation) === JSON.stringify(mainStable.fixtureObservation) &&
      JSON.stringify((await active.query(mainFooter, "elements")).elements) === JSON.stringify(mainElements.elements));

    await active.unmount(agent);
    const recreated = await active.mount("agent-chat.detached.retryable-failure");
    const recreatedFrame = await active.frame(recreated);
    check("recreated_parent_keeps_conversation_identity", recreatedFrame.target.appViewVariant === "agentChatChat");
    const recreatedFooter = await active.mount("secondary.footer", recreated);
    const recreatedState = await capture(recreatedFooter, "agent-recreated");
    check("teardown_recreates_distinct_footer_lifetime", recreated.generation !== agent.generation &&
      recreatedState.targetIdentity.surfaceGeneration !== agentAfter.targetIdentity.surfaceGeneration &&
      recreatedState.fixtureObservation.completedActionCount === 0);
    refused = false;
    try { await active.driver.request(retained); }
    catch (error) { refused = error instanceof DriverCommandRefused && STALE_CODES.includes(error.code); }
    check("retired_binding_cannot_route_to_recreated_owner", refused &&
      (await active.inspect(recreatedFooter)).fixtureObservation.completedActionCount === 0 &&
      (await active.inspect(mainFooter)).fixtureObservation.completedActionCount === mainStable.fixtureObservation.completedActionCount);
    check("native_footer_pixels_explicitly_excluded", receipt.effects.filter(effect => effect.captureScope).every(effect => effect.rawNativeFooterPixelsExcluded === true));
  } catch (error) {
    receipt.error = error instanceof Error ? error.message : String(error);
    if (error instanceof DriverLifecycleError) receipt.cleanup = error.cleanup;
    else if (error && typeof error === "object" && "cleanup" in error) receipt.cleanup = error.cleanup as RuntimeJourneyReceipt["cleanup"];
  } finally {
    if (client) {
      try { receipt.cleanup = await client.close(); }
      catch (error) {
        receipt.cleanup = error instanceof DriverLifecycleError ? error.cleanup : client.cleanup;
        receipt.error = `${receipt.error ?? ""};footer_cleanup:${String(error)}`;
      }
    }
  }
  receipt.pass = !receipt.error && receipt.assertions.length > 0 && receipt.assertions.every(item => item.pass) && receipt.cleanup.closed;
  return receipt;
}
