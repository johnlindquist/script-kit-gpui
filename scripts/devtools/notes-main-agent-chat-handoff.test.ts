import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const probeSource = readFileSync(
  join(import.meta.dir, "notes-main-agent-chat-handoff.ts"),
  "utf8",
);

describe("notes-main-agent-chat-handoff probe contract", () => {
  test("uses direct primitives, never fixed sleeps for readiness", () => {
    for (const primitive of [
      "listAutomationWindows",
      "getTargetState",
      "getAgentChatState",
      "simulateKey",
      "getLogs",
    ]) {
      expect(probeSource).toContain(primitive);
    }
    expect(probeSource).toContain("pollUntil");
  });

  test("asserts every required cross-window predicate", () => {
    for (const predicate of [
      "lastAiHandoffInactiveShapeStable",
      "reuseHandoffStaged",
      "reuseDestinationMainAgentChat",
      "reuseMessagesPreserved",
      "reuseComposerStartsWithNoteToken",
      "reuseExistingDraftPreserved",
      "reusePrimaryKindFocusedTarget",
      "reuseTargetSourceNotes",
      "reuseTargetKindNote",
      "reuseNoAutoSubmit",
      "notesWindowStillOpenAndVisible",
      "notesInstanceUnchanged",
      "oldChildWindowAbsent",
      "logHandoffRequested",
      "logHandoffMainStaged",
      "logEntryRequestOriginNotes",
      "oldEmbeddedEventsAbsent",
    ]) {
      expect(probeSource).toContain(predicate);
    }
  });

  test("never drives the removed embedded-host surface", () => {
    for (const legacy of [
      "openNotesAgentChat",
      "embeddedAgentChat.automationId",
      "NotesEmbeddedAgentChat",
    ]) {
      expect(probeSource).not.toContain(legacy);
    }
    // The only permitted "notes:ai" mention is the negative
    // no-child-window-registered assertion.
    expect(probeSource).toContain('oldChildWindowAbsent = !windowRows.some');
  });

  test("closes the driver in a finally block and fails nonzero on red", () => {
    expect(probeSource).toContain("finally {");
    expect(probeSource).toContain("await driver.close()");
    expect(probeSource).toContain("process.exit(1)");
  });
});
