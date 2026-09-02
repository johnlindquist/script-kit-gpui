#!/usr/bin/env bun
/**
 * Green-proof probe for the Agent Chat composer multiline growth fix.
 *
 * Red symptom (user screenshot, 2026-07-10): multiline composer text in the
 * embedded Agent Chat surface escaped the fixed-height input shell — spilling
 * up over the header/context zone (off-window) and down over the empty-state
 * guidance.
 *
 * Green contract, proven via the `composerScroll` runtime metrics (measured
 * by GPUI layout from the composer scroll container, not re-derived from the
 * input text):
 *   - viewport height = wrapped-line count * canonical 26px main-menu search
 *     line, clamped to 6 lines
 *   - content beyond 6 lines => maxScrollTopPx > 0 (scrollable, clipped)
 *   - cursor at end => scrollTopPx == maxScrollTopPx (cursor-follow)
 * Screenshots are captured best-effort as visual receipts; they are skipped
 * (not failed) when macOS Screen Recording TCC blocks the rebuilt binary.
 */
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const LINE = 26.0;
const outDir = resolve(process.env.PROBE_OUT_DIR ?? ".test-output/composer-grow-verify");
mkdirSync(outDir, { recursive: true });
const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ?? "target-agent/artifacts/composer-grow/script-kit-gpui";

const receipt: Json = {
  schemaVersion: 2,
  tool: "composer-grow-verify-probe",
  binary,
  pass: false,
  failures: [] as Json[],
  steps: [] as Json[],
};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: "composer-grow-verify",
  readyTimeoutMs: 30_000,
  defaultTimeoutMs: 10_000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

function close(a: number, b: number, tol = 1.5): boolean {
  return Math.abs(a - b) <= tol;
}

async function agentChatState(): Promise<Json> {
  const result = await driver.request(
    { type: "getAgentChatState" },
    { expect: "agent_chatStateResult", timeoutMs: 10_000 },
  );
  return (result.state ?? result) as Json;
}

interface Expect {
  viewportLines: number;
  canScroll: boolean;
  /** expected maxScrollTopPx when scrollable and line count is exact */
  maxScrollLines?: number;
  /** cursor sits at the end => scrollTop == maxScrollTop */
  scrolledToBottom?: boolean;
}

async function step(name: string, inputText: string | null, expect: Expect): Promise<void> {
  const failures = receipt.failures as Json[];
  if (inputText !== null) {
    const set = await driver.request(
      { type: "setAgentChatInput", text: inputText, submit: false },
      { expect: "externalCommandResult", timeoutMs: 10_000 },
    );
    if (set.ok === false || set.success === false) {
      failures.push({ name: `set_input_failed:${name}` });
    }
    await driver.waitForSettle().catch(() => {});
  }
  const state = await agentChatState();
  const cs = (state.composerScroll ?? null) as {
    scrollTopPx: number;
    maxScrollTopPx: number;
    viewportHeightPx: number;
    canScrollY: boolean;
  } | null;
  const shotPath = `${outDir}/${name}.png`;
  const shot = await driver
    .captureScreenshot({ target: { type: "main" }, savePath: shotPath })
    .catch((error) => ({ error: String(error) }));

  const stepReceipt: Json = {
    name,
    inputTextMatches: inputText === null || state.inputText === inputText,
    composerScroll: cs,
    expected: expect,
    screenshot: shot.error ? { skipped: true, error: shot.error } : { path: shotPath },
  };
  (receipt.steps as Json[]).push(stepReceipt);

  if (inputText !== null && state.inputText !== inputText) {
    failures.push({ name: `input_text_mismatch:${name}` });
  }
  if (!cs) {
    failures.push({ name: `composer_scroll_missing:${name}` });
    return;
  }
  if (!close(cs.viewportHeightPx, expect.viewportLines * LINE)) {
    failures.push({
      name: `viewport_mismatch:${name}`,
      got: cs.viewportHeightPx,
      want: expect.viewportLines * LINE,
    });
  }
  if (cs.canScrollY !== expect.canScroll) {
    failures.push({ name: `can_scroll_mismatch:${name}`, got: cs.canScrollY });
  }
  if (expect.maxScrollLines !== undefined && !close(cs.maxScrollTopPx, expect.maxScrollLines * LINE)) {
    failures.push({
      name: `max_scroll_mismatch:${name}`,
      got: cs.maxScrollTopPx,
      want: expect.maxScrollLines * LINE,
    });
  }
  if (expect.scrolledToBottom && !close(cs.scrollTopPx, cs.maxScrollTopPx)) {
    failures.push({
      name: `cursor_follow_mismatch:${name}`,
      scrollTop: cs.scrollTopPx,
      maxScrollTop: cs.maxScrollTopPx,
    });
  }
  if (!expect.canScroll && !close(cs.scrollTopPx, 0)) {
    failures.push({ name: `scroll_reset_mismatch:${name}`, scrollTop: cs.scrollTopPx });
  }
}

try {
  await driver.request({ type: "show" }, { expect: "windowVisibilityAck", timeoutMs: 10_000 }).catch(() => driver.send({ type: "show" }));
  await driver.waitForSettle().catch(() => {});
  const opened = await driver.request(
    { type: "openAgentChatKitchenSinkFixture" },
    { expect: "externalCommandResult", timeoutMs: 15_000 },
  );
  receipt.opened = { ok: opened.ok !== false };
  await Bun.sleep(750);
  await driver.waitForSettle().catch(() => {});

  await step("1-single-line", "hello composer", { viewportLines: 1, canScroll: false });
  await step("2-three-newlines", "line one\nline two\nline three", {
    viewportLines: 3,
    canScroll: false,
  });
  await step(
    "3-twelve-newlines",
    Array.from({ length: 12 }, (_, i) => `newline row ${i + 1}`).join("\n"),
    { viewportLines: 6, canScroll: true, maxScrollLines: 6, scrolledToBottom: true },
  );
  // Wrap-only overflow: no explicit newlines; must exceed 6 visual lines by
  // word wrap alone to prove wrap-aware measurement (not '\n' counting).
  await step(
    "4-wrap-only-overflow",
    Array.from({ length: 90 }, (_, i) => `wrapword${i + 1}`).join(" "),
    { viewportLines: 6, canScroll: true, scrolledToBottom: true },
  );
  // Shrink back: growth must be reversible and the offset must reset.
  await step("5-shrink-back", "small again", { viewportLines: 1, canScroll: false });

  receipt.pass = (receipt.failures as Json[]).length === 0;
} catch (error) {
  (receipt.failures as Json[]).push({ name: "probe_error", error: String(error) });
} finally {
  await driver.close().catch(() => {});
}

await Bun.write(`${outDir}/receipt.json`, JSON.stringify(receipt, null, 2));
console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.pass ? 0 : 1);
