#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { Driver, type Json } from "./driver.ts";
import {
  identityFromEnvironment,
  newRunId,
} from "./glass-evidence-contract.ts";
import { announceTestStatus } from "./test-status.ts";

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
  receipt.pass = reveal?.bodyVisible === true
    && reveal?.fallbackUsed === true
    && reveal?.nativeConfigured === false
    && Number(reveal?.completedFrameCount ?? 0) >= 2
    && typeof reveal?.firstFrameAtMonotonicNs === "number"
    && typeof reveal?.settleCompleteAtMonotonicNs === "number"
    && typeof reveal?.visibleAtMonotonicNs === "number"
    && elapsedMs >= 200
    && elapsedMs <= 1_200;
  driver.send({ type: "openNotes", requestId: "notes-fallback-close" });
} catch (error) {
  receipt.error = String(error);
  receipt.pass = false;
} finally {
  await driver.close();
  receipt.cleanedUp = !driver.alive;
  receipt.finishedAt = new Date().toISOString();
  receipt.pass = receipt.pass === true && receipt.cleanedUp === true;
  receipt.disposition = receipt.pass === true
    ? "EVALUABLE_PASS"
    : receipt.error
    ? "INVALID_OBSERVER"
    : "EVALUABLE_FAIL";
  writeFileSync(out, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receiptPath: out, pass: receipt.pass }, null, 2));
}

process.exit(receipt.pass ? 0 : 2);
