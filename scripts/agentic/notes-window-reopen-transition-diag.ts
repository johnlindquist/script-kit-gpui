#!/usr/bin/env bun
// Chaos-15 diagnostic 2: after openNotes toggle OFF→ON, poll the notes target
// every 150ms and timestamp exactly when (and how) it stops resolving.
import { Driver, type Json } from "../devtools/driver";

const binary =
  process.env.PROBE_BINARY ?? "target-agent/artifacts/monkey-notes/script-kit-gpui";
const runId = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
const target = { type: "kind", kind: "notes", index: 0 };
const out: Record<string, Json> = { samples: [] };

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `monkey-notes-reopen-transition-${runId}`,
  readyTimeoutMs: 30000,
  defaultTimeoutMs: 12000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

async function openNotes(tag: string) {
  driver.send({ type: "openNotes", requestId: `${runId}-${tag}` });
  await Bun.sleep(1100);
}

async function probeNotes(): Promise<string> {
  try {
    const r = (await driver.request(
      { type: "getState", target },
      { expect: "stateResult", timeoutMs: 3000 },
    )) as Json;
    if (r.error) return `error:${String(r.error).slice(0, 90)}`;
    return r.notes || r.promptType === "notes" || r.view ? "ok" : `ok?:${Object.keys(r).slice(0, 6).join(",")}`;
  } catch (e) {
    return `threw:${String(e).slice(0, 90)}`;
  }
}

try {
  await openNotes("open1");
  out.first_open_probe = { result: await probeNotes(), wallClock: new Date().toISOString() };
  await openNotes("off"); // toggle OFF
  out.toggle_off_at = new Date().toISOString();
  await openNotes("on"); // toggle ON
  out.toggle_on_at = new Date().toISOString();
  const t0 = performance.now();
  for (let i = 0; i < 45; i += 1) {
    const result = await probeNotes();
    (out.samples as Json[]).push({
      ms: Math.round(performance.now() - t0),
      wallClock: new Date().toISOString().slice(11, 23),
      result,
    });
    await Bun.sleep(150);
  }
} finally {
  console.log(JSON.stringify(out, null, 2));
  await driver.close();
}
