import type { RuntimeJourneyReceipt } from "./design.ts";
import { DriverCommandRefused, type Json } from "./driver.ts";
import { EvaluationContractError, type FixtureControl, type OwnedEvaluationClient } from "./lib/owned-evaluation.ts";
import type { AutomationInstance } from "./lib/target-identity.ts";

export const NOTES_ACCEPTANCE_FIXTURES = ["notes.editor", "day-page.today", "dictation.recording"] as const;

/** Extends the owned runner; never launches, attaches to, or closes a process itself. */
export async function runNotesAcceptance(client: OwnedEvaluationClient, receipt: RuntimeJourneyReceipt): Promise<void> {
  const check = (id: string, pass: boolean) => {
    receipt.assertions.push({ id, pass });
    if (!pass) throw new EvaluationContractError(id);
  };
  const observe = async (target: AutomationInstance, predicate: (state: Json) => boolean): Promise<Json> => {
    const deadline = performance.now() + 5000;
    let lastState: Json | undefined;
    while (performance.now() < deadline) {
      const state = await client.inspect(target);
      lastState = state;
      if (predicate(state)) return state;
      receipt.frames.push(await client.frame(target));
    }
    receipt.effects.push({ kind: "notesAcceptanceDeadline", target, targetIdentity: lastState?.targetIdentity,
      notes: lastState?.notes, dayPage: lastState?.dayPage, fixtureObservation: lastState?.fixtureObservation });
    throw new EvaluationContractError("notes_acceptance_postcondition_deadline");
  };
  const key = async (target: AutomationInstance, key: string, modifiers: string[] = [], text?: string) => {
    receipt.effects.push(await client.act(target, { type: "key", key, modifiers, ...(text === undefined ? {} : { text }) }));
  };
  const input = async (target: AutomationInstance, text: string) => {
    receipt.effects.push(await client.act(target, { type: "setInput", text }));
  };
  const control = async (target: AutomationInstance, control: FixtureControl) => {
    const expected = (await client.inspect(target)).targetIdentity;
    const result = await client.design({ operation: "fixtureControl", target, expected, control });
    if (result.operation !== "fixtureControl" || !result.ok) throw new EvaluationContractError("notes_fixture_control_required");
    receipt.effects.push(result);
    return result as Json;
  };
  const registry = async (target: AutomationInstance): Promise<Json> => {
    const result = await client.driver.request({ type: "listAutomationWindows" });
    const exact = result.windows?.filter((entry: Json) => entry.id === target.id && entry.generation === target.generation);
    if (!Array.isArray(exact) || exact.length !== 1) throw new EvaluationContractError("notes_exact_registered_instance_required");
    return exact[0];
  };
  const freezeDictation = async (overlay: AutomationInstance, destination: "notes" | "dayPage") => {
    await control(overlay, { family: "dictation", operation: "begin", destination });
    await control(overlay, { family: "dictation", operation: "recording", text: "obsolete document delivery", bars: [0.1, 0.2, 0.3, 0.4, 0.5, 0.4, 0.3, 0.2, 0.1] });
    await control(overlay, { family: "dictation", operation: "confirm" });
    await control(overlay, { family: "dictation", operation: "transcribe" });
  };
  const staleEvent = async (target: AutomationInstance, retained: Json, id: string) => {
    const before = await client.inspect(target);
    let refused = false;
    try {
      await client.driver.request({ type: "simulateGpuiEvent", target, expected: retained.targetIdentity,
        event: { type: "keyDown", key: "x", text: "obsolete document edit", modifiers: [] } });
    } catch (error) { refused = error instanceof DriverCommandRefused && error.code === "stale_target_identity"; }
    const after = await client.inspect(target);
    receipt.effects.push({ kind: "notesStaleDocumentRefusal", id, refused,
      retainedIdentity: retained.targetIdentity, beforeIdentity: before.targetIdentity, afterIdentity: after.targetIdentity });
    check(id, refused && before.inputValue === after.inputValue &&
      before.targetIdentity.surfaceGeneration === after.targetIdentity.surfaceGeneration &&
      before.targetIdentity.dataGeneration === after.targetIdentity.dataGeneration);
  };
  const searchAndSwitch = async (notes: AutomationInstance, query: string, noteId: string) => {
    await key(notes, "p", ["cmd"]);
    check(query === "Beta" ? "notes_switcher_open" : "notes_switcher_reopens", (await client.inspect(notes)).notes?.view?.showBrowsePanel === true);
    for (const character of query) await key(notes, character, [], character);
    const filtered = await observe(notes, state => state.notes?.commandBars?.noteSwitcher?.selectedActionId === `note_${noteId}`);
    await key(notes, "enter");
    const switched = await observe(notes, state => state.notes?.activeNoteId === noteId && state.notes?.view?.showBrowsePanel === false);
    receipt.effects.push({ kind: "notesSearchSwitch", query, filtered: filtered.notes.commandBars.noteSwitcher,
      activeNoteId: switched.notes.activeNoteId, targetIdentity: switched.targetIdentity });
    return switched;
  };

  const notes = await client.mount("notes.editor");
  const initial = await client.inspect(notes);
  receipt.effects.push({ kind: "notesEntryRevealInitial", targetIdentity: initial.targetIdentity, entryReveal: initial.notes?.entryReveal });
  check("notes_owned_hidden_storage", initial.windowVisible === false && initial.isFocused === false &&
    initial.notes?.counts?.notes >= 2 && initial.notes?.storage?.fullySandboxed === true);
  check("notes_reveal_begins_hidden", initial.notes?.entryReveal?.bodyVisible === false && initial.notes.entryReveal.completedFrameCount < 2);
  const revealed = await observe(notes, state => state.notes?.entryReveal?.phase === "visible" && state.notes.entryReveal.completedFrameCount === 2);
  const reveal = revealed.notes.entryReveal;
  receipt.effects.push({ kind: "notesEntryReveal", targetIdentity: revealed.targetIdentity, initial: initial.notes.entryReveal, completed: reveal,
    elapsedToRevealMs: (reveal.revealRequestedAtMonotonicNs - reveal.configuredAtMonotonicNs) / 1_000_000 });
  const times = [reveal.configuredAtMonotonicNs, reveal.firstFrameAtMonotonicNs, reveal.revealRequestedAtMonotonicNs, reveal.visibleAtMonotonicNs];
  check("notes_ordered_two_frame_reveal", reveal.bodyVisible === true && reveal.generation === initial.notes.entryReveal.generation &&
    times.every((time: unknown) => typeof time === "number" && Number.isFinite(time) && time > 0) &&
    times.every((time: number, index: number) => index === 0 || time >= times[index - 1]) &&
    reveal.revealAnchorAtMonotonicNs === reveal.revealRequestedAtMonotonicNs);
  check("notes_real_bounded_material_fallback", reveal.nativeConfigured === false && reveal.fallbackUsed === true &&
    reveal.styleApplied === false && reveal.morphStarted === false && reveal.settleDurationMs === 250 && reveal.revealDelayMs === 250 &&
    reveal.revealRequestedAtMonotonicNs - reveal.configuredAtMonotonicNs >= reveal.revealDelayMs * 1_000_000);
  receipt.frames.push(await client.frame(notes));

  const original = revealed.inputValue;
  check("notes_fixture_document_loaded", typeof original === "string" && original.includes("- [ ]") && original.includes("https://example.invalid"));
  await key(notes, "x", [], " fixture edit");
  const edited = await client.inspect(notes);
  check("notes_real_edit_revision", edited.targetIdentity.dataGeneration > revealed.targetIdentity.dataGeneration);
  await key(notes, "z", ["cmd"]);
  const undone = await observe(notes, state => state.notes?.editor?.textFingerprint === revealed.notes.editor.textFingerprint);
  check("notes_undo_restores_content", undone.inputValue === original);

  await key(notes, "p", ["cmd", "shift"]);
  check("notes_task_uses_real_preview", (await client.inspect(notes)).notes?.view?.previewEnabled === true);
  const markerStart = Buffer.byteLength(original.slice(0, original.indexOf("[ ]")), "utf8");
  await control(notes, { family: "notes", operation: "toggleTask", markerStart, markerEnd: markerStart + 3, checked: false });
  const toggled = await observe(notes, state => state.inputValue === original.replace("[ ]", "[x]"));
  check("notes_task_toggle_mutates_editor", toggled.notes.dirtyState.hasUnsavedChanges === true && toggled.targetIdentity.dataGeneration > undone.targetIdentity.dataGeneration);
  let staleMarkerRefused = false;
  try { await control(notes, { family: "notes", operation: "toggleTask", markerStart, markerEnd: markerStart + 3, checked: false }); }
  catch (error) { staleMarkerRefused = error instanceof DriverCommandRefused && error.code === "stale_task_marker"; }
  check("notes_stale_task_marker_refused", staleMarkerRefused && (await client.inspect(notes)).inputValue === toggled.inputValue);
  await key(notes, "p", ["cmd", "shift"]);
  await key(notes, "z", ["cmd"]);
  check("notes_task_toggle_undo", (await observe(notes, state => state.inputValue === original)).inputValue === original);

  // Height must change on the registry's exact lifetime, not only in a desired-height model.
  const beforeResize = await registry(notes);
  const beforeAutosize = (await client.inspect(notes)).notes.autosize.generation;
  const longText = `${original}\n${Array.from({ length: 90 }, (_, index) => `Owned resize line ${index}`).join("\n")}`;
  await input(notes, longText);
  const grown = await observe(notes, state => state.notes?.autosize?.generation > beforeAutosize &&
    state.notes?.lastAutosizeTransition?.applied === true && state.notes.lastAutosizeTransition.afterHeight > beforeResize.bounds.height);
  receipt.frames.push(await client.frame(notes));
  const grownWindow = await registry(notes);
  receipt.effects.push({ kind: "notesAutosizeGrow", before: beforeResize, grown: grownWindow,
    targetIdentity: grown.targetIdentity, growTransition: grown.notes.lastAutosizeTransition });
  check("notes_autosize_grows_exact_instance", grownWindow.bounds.height > beforeResize.bounds.height &&
    grownWindow.bounds.width === beforeResize.bounds.width && grownWindow.visible === false && grownWindow.focused === false &&
    grown.notes.generations.target === notes.generation);
  await input(notes, original);
  const shrunk = await observe(notes, state => state.notes?.autosize?.generation > grown.notes.autosize.generation &&
    state.notes?.lastAutosizeTransition?.applied === true && state.notes.lastAutosizeTransition.afterHeight < grownWindow.bounds.height);
  receipt.frames.push(await client.frame(notes));
  const shrunkWindow = await registry(notes);
  receipt.effects.push({ kind: "notesAutosize", before: beforeResize, grown: grownWindow, shrunk: shrunkWindow,
    growTransition: grown.notes.lastAutosizeTransition, shrinkTransition: shrunk.notes.lastAutosizeTransition });
  check("notes_autosize_shrinks_exact_instance", shrunkWindow.bounds.height < grownWindow.bounds.height &&
    shrunkWindow.bounds.width === beforeResize.bounds.width && shrunkWindow.visible === false && shrunkWindow.focused === false &&
    shrunk.notes.autosize.composition.footerActionRow.height === 0 && shrunk.notes.autosize.composition.footerActionRow.present === false);

  const themeBefore = await client.inspect(notes);
  const highlights = (state: Json) => state.notes.editor.markdownLinkHighlights.ranges.map((entry: Json) => ({ range: entry.range, color: entry.color, content: entry.content, role: entry.role }));
  const baselineHighlights = highlights(themeBefore);
  check("notes_existing_link_highlights_observed", baselineHighlights.length > 0 && baselineHighlights.every((entry: Json) => typeof entry.color?.h === "number"));
  const tokenId = "theme.colors.accent.selected";
  const baselineValue = themeBefore.liveTheme?.resolved?.values?.find((entry: Json) => entry.tokenId === tokenId)?.value;
  check("notes_theme_baseline_observed", Number.isSafeInteger(baselineValue));
  const publication = await client.applyTheme(themeBefore.targetIdentity.themeRevision, [{ tokenId, value: baselineValue === 0x72c1a8 ? 0x5b9dff : 0x72c1a8 }]);
  const themeAfter = await observe(notes, state => state.notes?.themeRevision === publication.revision);
  const editedHighlights = highlights(themeAfter);
  receipt.effects.push({ kind: "notesHighlightPublication", targetIdentity: themeAfter.targetIdentity,
    beforeRevision: themeBefore.notes.themeRevision, publishedRevision: publication.revision,
    before: baselineHighlights, edited: editedHighlights });
  const contentAndRanges = (values: Json[]) => values.map(({ color: _color, ...entry }) => entry);
  check("notes_theme_refreshes_actual_highlights", JSON.stringify(editedHighlights) !== JSON.stringify(baselineHighlights) &&
    JSON.stringify(contentAndRanges(editedHighlights)) === JSON.stringify(contentAndRanges(baselineHighlights)) &&
    themeAfter.inputValue === themeBefore.inputValue && themeAfter.notes.dataRevision === themeBefore.notes.dataRevision);
  const reverted = await client.revertTheme(publication.revision);
  const themeRestored = await observe(notes, state => state.notes?.themeRevision === reverted.revision);
  receipt.effects.push({ kind: "notesHighlightRefresh", beforeRevision: themeBefore.notes.themeRevision, publishedRevision: publication.revision,
    restoredRevision: reverted.revision, before: baselineHighlights, edited: editedHighlights, restored: highlights(themeRestored) });
  check("notes_theme_revert_restores_highlights", JSON.stringify(highlights(themeRestored)) === JSON.stringify(baselineHighlights));

  const persistedNote = `${original}\nOwned persisted note edit\n`;
  await input(notes, persistedNote);
  await observe(notes, state => state.inputValue === persistedNote);
  const overlay = await client.mount("dictation.recording");
  await freezeDictation(overlay, "notes");
  const retainedNote = await client.inspect(notes);
  const betaId = "d0197594-1111-4000-8000-000000000002";
  const beta = await searchAndSwitch(notes, "Beta", betaId);
  check("notes_search_switch_changes_document", beta.inputValue !== original && beta.notes.surfaceRevision > retainedNote.notes.surfaceRevision);
  await staleEvent(notes, retainedNote, "notes_stale_document_target_refused");
  const staleNotes = await control(overlay, { family: "dictation", operation: "deliver" });
  check("notes_stale_dictation_target_refused", staleNotes.observation.deliveryOutcome === "staleTarget" && (await client.inspect(notes)).inputValue === beta.inputValue);
  const reloaded = await searchAndSwitch(notes, "Alpha", revealed.notes.activeNoteId);
  receipt.effects.push({ kind: "notesSaveSwitchReload", targetIdentity: reloaded.targetIdentity,
    activeNoteId: reloaded.notes.activeNoteId, editorFingerprint: reloaded.notes.editor.textFingerprint,
    dirtyState: reloaded.notes.dirtyState, storage: reloaded.notes.storage });
  check("notes_save_switch_reload", reloaded.inputValue === persistedNote && reloaded.notes.dirtyState.hasUnsavedChanges === false);
  await key(notes, "f", ["cmd", "shift"]);
  for (const character of "Beta") await key(notes, character, [], character);
  const searched = await observe(notes, state => state.notes?.search?.queryLength === 4 && state.notes?.activeNoteId === betaId && state.notes?.counts?.notes === 1);
  receipt.effects.push({ kind: "notesStorageSearch", search: searched.notes.search, counts: searched.notes.counts,
    activeNoteId: searched.notes.activeNoteId, storage: searched.notes.storage });
  check("notes_real_storage_search", searched.notes.search.visible === true && searched.inputValue === beta.inputValue);
  await key(notes, "f", ["cmd", "shift"]);
  const searchClosed = await observe(notes, state => state.notes?.search?.visible === false && state.notes?.search?.queryLength === 0 && state.notes?.counts?.notes >= 2);
  check("notes_search_close_restores_corpus", searchClosed.inputValue === beta.inputValue);
  await client.unmount(overlay);
  await client.unmount(notes);
  const reopenedNotes = await client.mount("notes.editor");
  receipt.frames.push(await client.frame(reopenedNotes));
  const fromStorage = await client.inspect(reopenedNotes);
  receipt.effects.push({ kind: "notesCanonicalReopen", previousWindow: notes, reopenedWindow: reopenedNotes,
    activeNoteId: fromStorage.notes.activeNoteId, editorFingerprint: fromStorage.notes.editor.textFingerprint, storage: fromStorage.notes.storage });
  check("notes_save_reopen_canonical_storage", fromStorage.inputValue === persistedNote && fromStorage.notes.activeNoteId === revealed.notes.activeNoteId &&
    fromStorage.notes.storage.fullySandboxed === true && fromStorage.notes.dirtyState.hasUnsavedChanges === false);
  await client.unmount(reopenedNotes);

  const day = await client.mount("day-page.today");
  receipt.frames.push(await client.frame(day));
  const dayBefore = await client.inspect(day);
  check("day_owned_canonical_shelf", dayBefore.windowVisible === false && dayBefore.fixtureObservation?.clipboardShelfCount > 0);
  await key(day, "x", [], " round trip");
  await key(day, "s", ["cmd"]);
  const saved = await observe(day, state => state.dayPage?.dirty === false && state.fixtureObservation?.canonicalReadbackMatchesEditor === true);
  check("day_document_saved", saved.dayPage.inputFingerprint !== dayBefore.dayPage.inputFingerprint &&
    saved.fixtureObservation.canonicalContentFingerprint !== dayBefore.fixtureObservation.canonicalContentFingerprint &&
    saved.fixtureObservation.clipboardShelfFingerprint === dayBefore.fixtureObservation.clipboardShelfFingerprint);
  await key(day, "x", [], " @here");
  const choosing = await observe(day, state => state.targetIdentity?.appViewVariant === "ScriptList" && state.inputValue === "@here");
  check("day_context_uses_same_window", choosing.targetIdentity.windowGeneration === saved.targetIdentity.windowGeneration);
  const rows = (await client.query(day, "elements")).elements?.filter((node: Json) => node.role === "row" && node.kind === "context" && node.selectable !== false && node.selected === true);
  check("day_context_builtin_available", Array.isArray(rows) && rows.length === 1 && choosing.mainListScroll?.selectedStableKey === "spine:@:builtin:here");
  receipt.effects.push(await client.act(day, { type: "select", semanticId: rows[0].semanticId, submit: true }));
  const returned = await observe(day, state => state.dayPage?.lastContextRoundTripReceipt?.status === "completed");
  check("day_context_roundtrip", returned.targetIdentity.windowGeneration === saved.targetIdentity.windowGeneration &&
    returned.dayPage.lastContextRoundTripReceipt.receiptKind === "dayPage.contextRoundTrip" && returned.dayPage.contextReferenceLedger?.markdownReferenceCount >= 1);
  await key(day, "s", ["cmd"]);
  const returnSaved = await observe(day, state => state.fixtureObservation?.canonicalReadbackMatchesEditor === true);
  check("day_context_return_preserves_canonical_shelf", returnSaved.fixtureObservation.clipboardShelfCount === dayBefore.fixtureObservation.clipboardShelfCount &&
    returnSaved.fixtureObservation.clipboardShelfFingerprint === dayBefore.fixtureObservation.clipboardShelfFingerprint);
  receipt.effects.push({ kind: "dayCanonicalRoundTrip", before: dayBefore.fixtureObservation, saved: saved.fixtureObservation,
    returned: returnSaved.fixtureObservation, contextReceipt: returned.dayPage.lastContextRoundTripReceipt });

  // A same-window rebind invalidates both retained document commands and frozen dictation.
  const dayOverlay = await client.mount("dictation.recording");
  await freezeDictation(dayOverlay, "dayPage");
  const retainedDay = await client.inspect(day);
  await key(day, "p", ["cmd"]);
  for (const character of "Beta") await key(day, character, [], character);
  await observe(day, state => state.dayPage?.noteSwitcher?.selectedActionId === `note_${betaId}`);
  await key(day, "enter");
  const switchedDay = await observe(day, state => state.dayPage?.surfaceRevision > retainedDay.dayPage.surfaceRevision &&
    state.dayPage.documentIdentityFingerprint !== retainedDay.dayPage.documentIdentityFingerprint);
  await staleEvent(day, retainedDay, "day_stale_document_target_refused");
  const staleDay = await control(dayOverlay, { family: "dictation", operation: "deliver" });
  check("day_stale_dictation_target_refused", staleDay.observation.deliveryOutcome === "staleTarget" &&
    (await client.inspect(day)).dayPage.inputFingerprint === switchedDay.dayPage.inputFingerprint);
  await client.unmount(dayOverlay);
  await client.unmount(day);
  const reopenedDay = await client.mount("day-page.today");
  receipt.frames.push(await client.frame(reopenedDay));
  const reloadedDay = await client.inspect(reopenedDay);
  check("day_save_reopen_canonical_roundtrip", reloadedDay.dayPage.inputFingerprint === returnSaved.dayPage.inputFingerprint &&
    reloadedDay.fixtureObservation.canonicalReadbackMatchesEditor === true &&
    reloadedDay.fixtureObservation.canonicalContentFingerprint === returnSaved.fixtureObservation.canonicalContentFingerprint &&
    reloadedDay.fixtureObservation.clipboardShelfFingerprint === returnSaved.fixtureObservation.clipboardShelfFingerprint);
}
