#!/usr/bin/env bun
/**
 * Runtime proof for the dictation overlay caption redesign.
 *
 * Opens the visual-only dictation overlay fixture (no microphone, no model),
 * verifies the automation registry reports the 560×100 base geometry, checks
 * the layout info exposes the header row (timer + destination chips + target
 * badge) above the caption band, and (best-effort — screen capture TCC resets
 * on every rebuild) captures a screenshot of the overlay. Prints one JSON
 * receipt.
 */
import { join } from "node:path";
import { Driver } from "../devtools/driver.ts";

const OUT_DIR = process.env.DICTATION_PROOF_OUT ?? "/tmp/dictation-caption-proof";
await Bun.$`mkdir -p ${OUT_DIR}`.quiet();

const receipt: Record<string, unknown> = { steps: [] as unknown[] };
const steps = receipt.steps as unknown[];

const driver = await Driver.launch({
  binary:
    process.env.DICTATION_PROOF_BINARY ??
    "target-agent/artifacts/dictation-caption/script-kit-gpui",
  sessionName: "dictation-caption-proof",
  sandboxHome: true,
});

try {
  await driver.waitForSettle();

  driver.send({ type: "openDictationOverlayFixture" });
  await Bun.sleep(1200);

  const windows = (await driver.listAutomationWindows()) as {
    windows?: Array<Record<string, unknown>>;
  };
  const dictation = windows.windows?.find((w) => w.id === "dictation");
  const bounds = (dictation?.bounds ?? null) as {
    width?: number;
    height?: number;
  } | null;
  const geometryOk = bounds?.width === 560 && bounds?.height === 100;
  steps.push({
    step: "fixture-window-registered",
    found: Boolean(dictation),
    bounds,
    geometryOk,
  });

  const layout = (await driver.getLayoutInfo({
    target: { type: "id", id: "dictation" },
  })) as { components?: Array<{ name?: string }> };
  const names = (layout.components ?? []).map((c) => c.name);
  const expected = [
    "DictationHeaderRow",
    "DictationTimerSlot",
    "DictationDestinationChips",
    "DictationTargetBadge",
    "DictationSignalBand",
  ];
  const missing = expected.filter((n) => !names.includes(n));
  steps.push({ step: "header-row-layout", names, missing });

  const glassLogs = (await driver.getLogs({
    target: "script_kit::dictation",
    contains: "dictation_glass_button_host_installed",
    limit: 20,
  })) as { entries?: Array<Record<string, unknown>> };
  const glassInstalled = (glassLogs.entries ?? []).some((entry) =>
    String(entry.message ?? "").includes(
      "dictation_glass_button_host_installed",
    ),
  );
  steps.push({
    step: "liquid-glass-button-host",
    installed: glassInstalled,
    matchingLogs: glassLogs.entries ?? [],
  });

  let screenshotOk = false;
  try {
    const shotPath = join(OUT_DIR, "dictation-overlay-recording.png");
    const shot = (await driver.captureScreenshot({
      savePath: shotPath,
      target: { type: "id", id: "dictation" },
      timeoutMs: 8000,
    })) as { width?: number; error?: string };
    screenshotOk = !shot.error && (shot.width ?? 0) > 0;
    steps.push({ step: "overlay-screenshot", path: shotPath, ...shot });
  } catch (error) {
    steps.push({
      step: "overlay-screenshot",
      skipped: `screen capture unavailable (TCC resets per rebuild): ${error}`,
    });
  }

  receipt.screenshotOk = screenshotOk;
  receipt.ok =
    Boolean(dictation) && geometryOk && missing.length === 0 && glassInstalled;
} catch (error) {
  receipt.ok = false;
  receipt.error = String(error);
} finally {
  await driver.close();
}

console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.ok ? 0 : 1);
