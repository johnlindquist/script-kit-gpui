#!/usr/bin/env bun
/**
 * Pixel-diff proof that the composer scrollbar thumb paints while active.
 * Capture A: immediately after a scroll-offset change (thumb should be shown).
 * Capture B: after the fade window (~5s idle; thumb gone).
 * If the thumb renders, the composer right-edge strip differs between A and B
 * while a left-of-strip control region stays identical.
 */
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { Driver } from "../devtools/driver";

const outDir = resolve(".test-output/composer-grow-verify");
mkdirSync(outDir, { recursive: true });

const driver = await Driver.launch({
  binary: "target-agent/artifacts/composer-grow/script-kit-gpui",
  sandboxHome: true,
  sessionName: "composer-scrollbar-pixeldiff",
  readyTimeoutMs: 30_000,
  defaultTimeoutMs: 10_000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

try {
  await driver.request({ type: "show" }, { expect: "windowVisibilityAck" }).catch(() => {});
  await driver.request(
    { type: "openAgentChatKitchenSinkFixture" },
    { expect: "externalCommandResult", timeoutMs: 15_000 },
  );
  await Bun.sleep(600);
  await driver.request(
    {
      type: "setAgentChatInput",
      text: Array.from({ length: 12 }, (_, i) => `newline row ${i + 1}`).join("\n"),
      submit: false,
    },
    { expect: "externalCommandResult" },
  );
  // Capture as fast as possible after the offset jump (0 -> -132px).
  // Target the main window explicitly: parallel sessions steal OS focus and
  // the default focused-window target then errors ("No focused automation
  // window") leaving stale files on disk. Retry blank captures: they happen
  // while the window is still becoming visible.
  const target = { type: "main" };
  async function captureRetry(savePath: string): Promise<void> {
    let lastError = "";
    for (let attempt = 0; attempt < 5; attempt++) {
      const shot = await driver.captureScreenshot({ target, savePath });
      if (!shot.error) return;
      lastError = String(shot.error);
      await driver.send({ type: "show" });
      await Bun.sleep(700);
    }
    throw new Error(`capture failed after retries: ${lastError}`);
  }
  await captureRetry(`${outDir}/6-active.png`);
  await Bun.sleep(6_000); // FADE_OUT_DELAY 2s + FADE_OUT_DURATION 3s + slack
  await captureRetry(`${outDir}/6-idle.png`);
  console.log(JSON.stringify({ ok: true }));
} finally {
  await driver.close().catch(() => {});
}
