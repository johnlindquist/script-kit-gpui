#!/usr/bin/env bun
// Chaos-15 diagnostic 3: registry ground truth across openNotes toggles.
import { Driver, type Json } from "../devtools/driver";

const binary =
  process.env.PROBE_BINARY ?? "target-agent/artifacts/monkey-notes/script-kit-gpui";
const runId = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
const target = { type: "kind", kind: "notes", index: 0 };
const out: Record<string, Json> = {};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `monkey-notes-registry-diag-${runId}`,
  readyTimeoutMs: 30000,
  defaultTimeoutMs: 12000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

async function openNotes(tag: string) {
  driver.send({ type: "openNotes", requestId: `${runId}-${tag}` });
  await Bun.sleep(1100);
}
async function windows(tag: string) {
  const r = (await driver.request({ type: "listAutomationWindows" }, { timeoutMs: 5000 })) as Json;
  out[`windows_${tag}`] = ((r.windows ?? []) as Json[]).map((w) => ({
    id: w.id,
    kind: w.kind,
    focused: w.focused,
    visible: w.visible,
  }));
}
async function stateProbe(tag: string) {
  try {
    const r = (await driver.request(
      { type: "getState", target },
      { expect: "stateResult", timeoutMs: 4000 },
    )) as Json;
    out[`state_${tag}`] = {
      keys: Object.keys(r).slice(0, 14),
      hasNotes: Boolean(r.notes),
      error: r.error ?? null,
    };
  } catch (e) {
    out[`state_${tag}`] = { threw: String(e).slice(0, 140) };
  }
}

try {
  await windows("boot");
  await openNotes("open1");
  await windows("after_open1");
  await stateProbe("after_open1");
  await openNotes("off");
  await windows("after_off");
  await stateProbe("after_off");
  await openNotes("on");
  await windows("after_on");
  await stateProbe("after_on");
  await Bun.sleep(2000);
  await windows("after_on_2s");
  await stateProbe("after_on_2s");
} finally {
  console.log(JSON.stringify(out, null, 2));
  await driver.close();
}
