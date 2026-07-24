#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Driver, type Json } from "./driver.ts";
import {
  identityFromEnvironment,
  newRunId,
} from "./glass-evidence-contract.ts";
import { announceTestStatus } from "./test-status.ts";
import {
  finishInterferenceMonitor,
  startInterferenceMonitor,
  waitForInterferenceReady,
} from "./glass-interference.ts";

const arg = (name: string, fallback?: string) => {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
};

const binary = resolve(arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY ?? "")!);
const out = resolve(
  arg("--out", ".artifacts/glass-motion-contrast/notes-fallback.json")!,
);
if (!binary || !existsSync(binary)) throw new Error(`binary missing: ${binary}`);
mkdirSync(dirname(out), { recursive: true });
const interferenceHelper = join(
  dirname(out),
  "macos-glass-interference-monitor",
);
const interferenceCompile = Bun.spawnSync([
  "xcrun",
  "swiftc",
  "-O",
  resolve(import.meta.dir, "../agentic/macos-glass-interference-monitor.swift"),
  "-o",
  interferenceHelper,
]);
if (interferenceCompile.exitCode !== 0) {
  throw new Error(
    `interference helper compile failed: ${interferenceCompile.stderr.toString()}`,
  );
}
let interferenceMonitor: ReturnType<typeof startInterferenceMonitor> | null = null;

const receipt: Json = {
  schemaVersion: 2,
  ...identityFromEnvironment({
    runId: newRunId(),
    gitCommit: Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout.toString().trim(),
    binary,
    binarySha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
  }),
  scenario: process.env.SCRIPT_KIT_GLASS_SCENARIO ?? "notes-fallback",
  startedAt: new Date().toISOString(),
  samples: [],
  pass: false,
};

await announceTestStatus(
  "Notes glass fallback",
  "Native glass is deliberately unavailable; body reveal must remain bounded and two-frame gated",
);

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `notes-glass-fallback-${process.pid}`,
  defaultTimeoutMs: 8_000,
  env: { SCRIPT_KIT_DEBUG_NO_GLASS: "1" },
});

try {
  receipt.pid = driver.pid;
  driver.send({ type: "show", requestId: "notes-fallback-main" });
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 5_000 });
  interferenceMonitor = startInterferenceMonitor(
    interferenceHelper,
    dirname(out),
  );
  receipt.interferenceReady = await waitForInterferenceReady(interferenceMonitor);
  const openedAt = performance.now();
  driver.send({ type: "openNotes", requestId: "notes-fallback-open" });
  let reveal: Json = null;
  while (performance.now() - openedAt < 1_500) {
    const state = await driver.getTargetState(
      { type: "id", id: "notes" },
      { timeoutMs: 5_000 },
    ).catch(() => null);
    reveal = state?.notes?.entryReveal ?? state?.entryReveal ?? null;
    (receipt.samples as Json[]).push({
      elapsedMs: Number((performance.now() - openedAt).toFixed(2)),
      reveal,
    });
    if (reveal?.bodyVisible === true) break;
    await Bun.sleep(20);
  }
  const elapsedMs = performance.now() - openedAt;
  receipt.finalReveal = reveal;
  receipt.elapsedMs = Number(elapsedMs.toFixed(2));
  const timing = {
    configured: Number(reveal?.configuredAtMonotonicNs),
    firstFrame: Number(reveal?.firstFrameAtMonotonicNs),
    revealAnchor: Number(
      reveal?.revealAnchorAtMonotonicNs
        ?? reveal?.settleCompleteAtMonotonicNs,
    ),
    revealRequested: Number(reveal?.revealRequestedAtMonotonicNs),
    visible: Number(reveal?.visibleAtMonotonicNs),
  };
  const ordered = Object.values(timing).every(Number.isFinite)
    && timing.configured <= timing.firstFrame
    && timing.firstFrame <= timing.revealAnchor
    && timing.revealAnchor <= timing.revealRequested
    && timing.revealRequested <= timing.visible;
  const minimumFallbackNs = 250_000_000 + 2 * (1_000_000_000 / 60);
  const revealAnchorDelayNs = timing.revealAnchor - timing.configured;
  const fallbackDelayNs = timing.visible - timing.configured;
  receipt.hostClockTiming = {
    times: timing,
    ordered,
    minimumFallbackNs,
    revealAnchorDelayNs,
    fallbackDelayNs,
  };
  receipt.pass = reveal?.bodyVisible === true
    && reveal?.fallbackUsed === true
    && reveal?.nativeConfigured === false
    && Number(reveal?.completedFrameCount ?? 0) >= 2
    && typeof reveal?.firstFrameAtMonotonicNs === "number"
    && typeof reveal?.revealAnchorAtMonotonicNs === "number"
    && typeof reveal?.visibleAtMonotonicNs === "number"
    && ordered
    && revealAnchorDelayNs >= 250_000_000 - 1_000_000
    && revealAnchorDelayNs <= 350_000_000
    && fallbackDelayNs >= minimumFallbackNs - 1_000_000
    && fallbackDelayNs <= 450_000_000
    && elapsedMs >= 280
    && elapsedMs <= 700;
  driver.send({ type: "openNotes", requestId: "notes-fallback-close" });
} catch (error) {
  receipt.error = String(error);
  receipt.pass = false;
} finally {
  if (interferenceMonitor) {
    receipt.interference = await finishInterferenceMonitor(interferenceMonitor);
    receipt.pass = receipt.pass === true && receipt.interference.pass === true;
  }
  await driver.close();
  receipt.cleanedUp = !driver.alive;
  receipt.finishedAt = new Date().toISOString();
  receipt.pass = receipt.pass === true && receipt.cleanedUp === true;
  receipt.disposition = receipt.interference?.disposition === "INVALID_INTERFERENCE"
    ? "INVALID_INTERFERENCE"
    : receipt.pass === true
    ? "EVALUABLE_PASS"
    : receipt.error
    ? "INVALID_OBSERVER"
    : "EVALUABLE_FAIL";
  writeFileSync(out, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receiptPath: out, pass: receipt.pass }, null, 2));
}

process.exit(receipt.pass ? 0 : 2);
