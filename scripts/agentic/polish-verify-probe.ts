#!/usr/bin/env bun
// Polish-fix verification probe: notes footer/header, confirm modal, agent chat footer stability.
// Receipts: one JSON summary + screenshots under .test-screenshots/polish-verify/.
import { Driver } from "../devtools/driver.ts";
import { mkdirSync } from "node:fs";

const SHOT_DIR = ".test-screenshots/polish-verify";
mkdirSync(SHOT_DIR, { recursive: true });

const receipt: Record<string, unknown> = {};
const driver = await Driver.launch({ sandboxHome: true, sessionName: "polish-verify" });
try {
  await driver.waitForSettle();

  // ── 1. Notes window: footer must be three-key strip + save slot, no scattered info ──
  await driver.request({ type: "openNotes" }).catch((e: unknown) => (receipt.notesOpenError = String(e)));
  await driver.waitForSettle();
  const notesElements = await driver
    .getElements({ target: { type: "kind", kind: "notes" } })
    .catch((e: unknown) => ({ error: String(e) }));
  const notesText = JSON.stringify(notesElements);
  receipt.notes = {
    hasActionsHint: notesText.includes("⌘K"),
    hasAgentHint: notesText.includes("⌘↵"),
    hasNotesHint: notesText.includes("⌘P"),
    oldItemsGone: {
      words: !/\d+ words/.test(notesText),
      readingTime: !notesText.includes("min read"),
      sortLabel: !notesText.includes("updated ↓"),
      trashBadge: !/trash \(\d+\)/.test(notesText),
      autoSize: !notesText.includes("auto-size"),
    },
  };
  await driver
    .captureScreenshot({ target: { type: "kind", kind: "notes" }, savePath: `${SHOT_DIR}/notes-window.png` })
    .catch((e: unknown) => (receipt.notesShotError = String(e)));
  driver.simulateKey("escape");
  await driver.waitForSettle();

  // ── 2. Confirm modal fixture: shared action row with keycaps ──
  await driver.request({ type: "openConfirmPrompt" }).catch((e: unknown) => (receipt.confirmOpenError = String(e)));
  await driver.waitForSettle();
  const confirmElements = await driver.getElements().catch((e: unknown) => ({ error: String(e) }));
  const confirmText = JSON.stringify(confirmElements);
  receipt.confirm = {
    hasCancel: confirmText.toLowerCase().includes("cancel"),
    hasEscKeycap: confirmText.includes("Esc"),
    hasEnterKeycap: confirmText.includes("↵"),
  };
  await driver
    .captureScreenshot({ savePath: `${SHOT_DIR}/confirm-modal.png` })
    .catch((e: unknown) => (receipt.confirmShotError = String(e)));
  driver.simulateKey("escape");
  await driver.waitForSettle();

  // ── 3. Agent Chat surface: footer leading slot + actions ──
  // (was driven by the removed kitchen-sink fixture command; a plain Agent
  // Chat open exercises the same current-surface footer chrome)
  await driver
    .request({ type: "openAi" })
    .catch((e: unknown) => (receipt.chatOpenError = String(e)));
  await driver.waitForSettle();
  const chatState = await driver.request({ type: "getAgentChatState" }).catch((e: unknown) => ({ error: String(e) }));
  const chatElements = await driver
    .getElements({ target: { type: "kind", kind: "agentChat" } })
    .catch((e: unknown) => ({ error: String(e) }));
  const chatText = JSON.stringify(chatElements) + JSON.stringify(chatState);
  receipt.agentChat = {
    footerHasActions: chatText.includes("⌘K") || chatText.toLowerCase().includes("actions"),
    footerHasLeadingRun: chatText.includes("Send") || chatText.includes("Stop"),
  };
  await driver
    .captureScreenshot({ savePath: `${SHOT_DIR}/agent-chat.png` })
    .catch((e: unknown) => (receipt.chatShotError = String(e)));

  receipt.ok = true;
} catch (e) {
  receipt.ok = false;
  receipt.fatal = String(e);
} finally {
  await driver.close();
}
console.log(JSON.stringify(receipt, null, 2));
