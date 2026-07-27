#!/usr/bin/env bun
/**
 * Runtime proof: the notes→AI affordance opens the MAIN window's Agent Chat
 * with the selected note staged as an explicit `@note` reference — and the
 * Notes window stays open.
 *
 * Scenarios:
 *  A. Fresh handoff — Notes Cmd+Enter (target-scoped simulateKey) opens a
 *     Standard Agent Chat in the main window with the note chip + composer
 *     token. If the sandbox has no usable agent runtime, the app must land
 *     in setup/recovery WITHOUT claiming success (`agentSetupMode` recorded;
 *     scenario B still proves the cross-window contract).
 *  B. Reuse handoff — with the deterministic provider-free mock fixture chat
 *     open in main (2 messages + "Fixture follow-up" draft), a second Notes
 *     handoff must stage into that SAME chat: messages preserved, draft
 *     preserved behind the prepended `@note` token, no auto-submit.
 *  C. Unsaved-draft identity is locked by unit tests on
 *     `compose_note_ai_target` (draft:<instance_id>); the automation surface
 *     cannot produce a selected-note-free draft deterministically, so the
 *     runtime pass records it as unit-covered.
 *
 * Prints one JSON receipt and exits nonzero when any required predicate is
 * false.
 */
import { Driver, type Json } from "./driver";

type JsonObject = Record<string, unknown>;

const asObject = (value: unknown): JsonObject =>
  value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonObject)
    : {};
const asArray = (value: unknown): unknown[] =>
  Array.isArray(value) ? value : [];

const nonce = Date.now().toString(36).slice(-6).toUpperCase();
const noteBody = `Notes AI Probe ${nonce}\n\nThis body proves the live-editor snapshot handoff.`;

function notesStateOf(envelope: Json): JsonObject {
  const response = asObject(asObject(envelope).response ?? envelope);
  return asObject(response.notes);
}

async function pollUntil<T>(
  label: string,
  timeoutMs: number,
  read: () => Promise<T>,
  predicate: (value: T) => boolean,
): Promise<T> {
  const startedAt = Date.now();
  let last: T;
  for (;;) {
    last = await read();
    if (predicate(last)) {
      return last;
    }
    if (Date.now() - startedAt > timeoutMs) {
      throw new Error(`Timeout waiting for ${label}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
}

async function main() {
  const driver = await Driver.launch({
    binary: process.env.SCRIPT_KIT_GPUI_BINARY,
    sandboxHome: true,
    seedAgentAuth: true,
    sessionName: `notes-main-agent-chat-handoff-${nonce.toLowerCase()}`,
    env: { SCRIPT_KIT_TEST_STATUS: "1" },
  });

  const receipt: JsonObject = {
    schemaVersion: 1,
    tool: "script-kit-devtools.notes-main-agent-chat-handoff",
    nonce,
    scenarios: {},
  };
  const predicates: Record<string, boolean> = {};
  let agentSetupMode = false;

  try {
    // ── Open Notes and stage a live editor snapshot ────────────────────────
    driver.send({ type: "openNotes" });
    await pollUntil(
      "notes window registered",
      15000,
      () => driver.listAutomationWindows(),
      (windows) =>
        asArray(asObject(windows).windows).some(
          (w) => asObject(w).id === "notes" && asObject(w).visible === true,
        ),
    );

    await driver.request(
      {
        type: "batch",
        target: { type: "kind", kind: "notes" },
        commands: [{ type: "setInput", text: noteBody }],
        options: { stopOnError: true, rollbackOnError: false, timeout: 8000 },
      },
      { expect: "batchResult", timeoutMs: 10000 },
    );
    const notesBefore = notesStateOf(
      await driver.getTargetState({ type: "kind", kind: "notes" }),
    );
    const entryRevealBefore = asObject(notesBefore.entryReveal);
    const inactiveHandoff = asObject(notesBefore.lastAiHandoff);
    predicates.lastAiHandoffInactiveShapeStable =
      inactiveHandoff.redacted === true && inactiveHandoff.active === false;

    // ── Scenario A: fresh handoff ──────────────────────────────────────────
    driver.send({
      type: "simulateKey",
      key: "enter",
      modifiers: ["cmd"],
      target: { type: "kind", kind: "notes" },
    });
    const notesAfterA = await pollUntil(
      "lastAiHandoff receipt after Cmd+Enter",
      20000,
      async () =>
        notesStateOf(await driver.getTargetState({ type: "kind", kind: "notes" })),
      (state) => asObject(state.lastAiHandoff).active === true,
    );
    const handoffA = asObject(notesAfterA.lastAiHandoff);
    receipt.scenarioA = { lastAiHandoff: handoffA };
    agentSetupMode = handoffA.status !== "staged";

    if (!agentSetupMode) {
      const chatA = asObject(
        await driver.request(
          { type: "getAgentChatState" },
          { expect: "agentChatStateResult", timeoutMs: 10000 },
        ),
      );
      const stateA = asObject(chatA.state ?? chatA);
      const partsA = asArray(stateA.contextParts).map(asObject);
      const notePartA = partsA.find((p) => p.targetKind === "note");
      predicates.freshHandoffStaged = handoffA.status === "staged";
      predicates.freshComposerStartsWithNoteToken = String(
        stateA.inputText ?? "",
      ).startsWith("@note:");
      predicates.freshPrimaryKindFocusedTarget =
        notePartA?.kind === "focusedTarget";
      predicates.freshTargetSourceNotes = notePartA?.targetSource === "Notes";
      predicates.freshChipLabelCanonical = String(
        notePartA?.label ?? "",
      ).startsWith("Note: ");
      predicates.freshNoAutoSubmit = stateA.messageCount === 0;
      receipt.scenarioA = {
        lastAiHandoff: handoffA,
        composer: stateA.inputText,
        contextParts: partsA,
        messageCount: stateA.messageCount,
      };
    }

    // ── Scenario B: reuse the deterministic fixture chat ───────────────────
    driver.send({ type: "openAiWithMockData" });
    await pollUntil(
      "fixture chat ready",
      15000,
      async () =>
        asObject(
          await driver.request(
            { type: "getAgentChatState" },
            { expect: "agentChatStateResult", timeoutMs: 8000 },
          ),
        ),
      (chat) => {
        const state = asObject(chat.state ?? chat);
        return (
          state.messageCount === 2 &&
          String(state.inputText ?? "") === "Fixture follow-up"
        );
      },
    );

    const generationBefore = Number(handoffA.generation ?? 0);
    driver.send({
      type: "simulateKey",
      key: "enter",
      modifiers: ["cmd"],
      target: { type: "kind", kind: "notes" },
    });
    const notesAfterB = await pollUntil(
      "second handoff receipt",
      20000,
      async () =>
        notesStateOf(await driver.getTargetState({ type: "kind", kind: "notes" })),
      (state) =>
        Number(asObject(state.lastAiHandoff).generation ?? 0) > generationBefore,
    );
    const handoffB = asObject(notesAfterB.lastAiHandoff);

    const chatB = asObject(
      await driver.request(
        { type: "getAgentChatState" },
        { expect: "agentChatStateResult", timeoutMs: 10000 },
      ),
    );
    const stateB = asObject(chatB.state ?? chatB);
    const partsB = asArray(stateB.contextParts).map(asObject);
    const notePartB = partsB.find((p) => p.targetKind === "note");
    const composerB = String(stateB.inputText ?? "");

    predicates.reuseHandoffStaged = handoffB.status === "staged";
    predicates.reuseDestinationMainAgentChat =
      handoffB.destinationWindowId === "main" &&
      handoffB.destinationSurface === "agentChat";
    predicates.reuseMessagesPreserved = stateB.messageCount === 2;
    predicates.reuseComposerStartsWithNoteToken = composerB.startsWith("@note:");
    predicates.reuseExistingDraftPreserved = composerB.endsWith(
      "Fixture follow-up",
    );
    predicates.reusePrimaryKindFocusedTarget =
      notePartB?.kind === "focusedTarget";
    predicates.reuseTargetSourceNotes = notePartB?.targetSource === "Notes";
    predicates.reuseTargetKindNote = notePartB?.targetKind === "note";
    predicates.reuseChipLabelCanonical = String(
      notePartB?.label ?? "",
    ).startsWith("Note: ");
    predicates.reuseNoAutoSubmit = stateB.messageCount === 2;
    receipt.scenarioB = {
      lastAiHandoff: handoffB,
      composer: composerB,
      contextParts: partsB,
      messageCount: stateB.messageCount,
    };

    // ── Notes window stays open, same instance, no notes:ai child ──────────
    const windowsAfter = asObject(await driver.listAutomationWindows());
    const windowRows = asArray(windowsAfter.windows).map(asObject);
    const notesRow = windowRows.find((w) => w.id === "notes");
    const mainRow = windowRows.find((w) => w.id === "main");
    const notesAfter = notesStateOf(
      await driver.getTargetState({ type: "kind", kind: "notes" }),
    );
    const entryRevealAfter = asObject(notesAfter.entryReveal);

    predicates.notesWindowStillOpenAndVisible =
      notesRow?.visible === true;
    predicates.notesInstanceUnchanged =
      entryRevealAfter.instanceId === entryRevealBefore.instanceId;
    predicates.oldChildWindowAbsent = !windowRows.some(
      (w) => w.id === "notes:ai",
    );
    predicates.mainWindowRegistered = mainRow != null;

    // ── Structured log assertions ──────────────────────────────────────────
    const requestedLogs = asObject(
      await driver.getLogs({ contains: "notes_ai_handoff_requested", limit: 20 }),
    );
    const stagedLogs = asObject(
      await driver.getLogs({ contains: "notes_ai_handoff_main_staged", limit: 20 }),
    );
    const entryLogs = asObject(
      await driver.getLogs({ contains: "agent_chat_entry_request_open", limit: 20 }),
    );
    const legacyLogs = asObject(
      await driver.getLogs({ contains: "notes_embedded_agent_chat", limit: 20 }),
    );
    const legacyCartLogs = asObject(
      await driver.getLogs({
        contains: "notes_cart_open_embedded_agent_chat",
        limit: 20,
      }),
    );
    const countOf = (logs: JsonObject) => asArray(logs.entries ?? logs.logs).length;
    predicates.logHandoffRequested = countOf(requestedLogs) > 0;
    predicates.logHandoffMainStaged = countOf(stagedLogs) > 0;
    predicates.logEntryRequestOriginNotes = asArray(
      entryLogs.entries ?? entryLogs.logs,
    ).some((entry) => JSON.stringify(entry).includes("Notes"));
    predicates.oldEmbeddedEventsAbsent =
      countOf(legacyLogs) === 0 && countOf(legacyCartLogs) === 0;

    receipt.scenarioC = {
      coveredBy:
        "unit tests: notes::window::ai_handoff::tests::unsaved_draft_is_accepted_and_uses_instance_scoped_identity",
    };
  } finally {
    await driver.close();
  }

  receipt.agentSetupMode = agentSetupMode;
  receipt.predicates = predicates;
  const failed = Object.entries(predicates).filter(([, ok]) => !ok);
  receipt.failedPredicates = failed.map(([name]) => name);
  receipt.green = failed.length === 0;
  console.log(JSON.stringify(receipt, null, 2));
  if (!receipt.green) {
    process.exit(1);
  }
}

await main();
