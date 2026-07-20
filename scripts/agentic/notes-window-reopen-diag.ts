#!/usr/bin/env bun
// Diagnostic (chaos-15): is the Notes window blank after toggle-reopen in
// general, or only after the canonical note file vanished?
import { readFileSync, existsSync, rmSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const binary =
  process.env.PROBE_BINARY ?? "target-agent/artifacts/monkey-notes/script-kit-gpui";
const runId = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`;
const target = { type: "kind", kind: "notes", index: 0 };
const out: Record<string, Json> = {};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `monkey-notes-reopen-diag-${runId}`,
  readyTimeoutMs: 30000,
  defaultTimeoutMs: 12000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});
const notesDir = join(driver.sessionDir, "home", ".scriptkit", "brain", "notes");

async function openNotes(tag: string) {
  driver.send({ type: "openNotes", requestId: `${runId}-${tag}` });
  await Bun.sleep(1100);
}
async function elementsSnapshot(tag: string): Promise<Json> {
  try {
    const r = (await driver.getElements({ target, limit: 180 }, { timeoutMs: 6000 })) as Json;
    const flat: Json[] = [];
    const walk = (n: unknown) => {
      if (!n || typeof n !== "object") return;
      if (Array.isArray(n)) return n.forEach(walk);
      const j = n as Json;
      if (typeof j.semanticId === "string" || typeof j.id === "string")
        flat.push({ semanticId: j.semanticId ?? j.id, valueLen: typeof j.value === "string" ? j.value.length : null });
      Object.values(j).forEach(walk);
    };
    walk(r);
    return { tag, ids: flat, warnings: r.warnings ?? null, error: null };
  } catch (e) {
    return { tag, ids: null, warnings: null, error: String(e).slice(0, 160) };
  }
}
async function setText(text: string): Promise<boolean> {
  const b = (await driver.request(
    {
      type: "batch",
      requestId: `${runId}-set-${Date.now()}`,
      target,
      commands: [{ type: "setInput", text }],
      options: { stopOnError: true, timeout: 5000 },
    },
    { expect: "batchResult", timeoutMs: 8000 },
  )) as Json;
  return b.success === true;
}
function fileWith(marker: string): string | null {
  if (!existsSync(notesDir)) return null;
  for (const f of readdirSync(notesDir)) {
    const p = join(notesDir, f);
    try {
      if (readFileSync(p, "utf8").includes(marker)) return p;
    } catch {}
  }
  return null;
}

try {
  // Phase A: seed a note, toggle OFF/ON WITHOUT touching files.
  await openNotes("open1");
  const marker = `diag ${runId}`;
  out.seed_set = { ok: await setText(`# Diag Note ${runId}\n\n${marker}\n`) };
  await Bun.sleep(1500);
  out.canonical = { file: fileWith(marker) };
  out.a_before_toggle = await elementsSnapshot("a_before");
  await openNotes("a_off"); // toggle OFF
  await openNotes("a_on"); // toggle ON
  await Bun.sleep(1000);
  out.a_after_reopen = await elementsSnapshot("a_after");
  out.a_settext_after_reopen = { ok: await setText(`# Diag Note ${runId}\n\n${marker}\nA-extra\n`) };

  // Phase B: vanish the canonical file while clean, toggle OFF/ON.
  await Bun.sleep(1500);
  const canonical = fileWith(marker);
  if (canonical) rmSync(canonical, { force: true });
  out.b_deleted = { canonical };
  await Bun.sleep(600);
  await openNotes("b_off"); // toggle OFF
  await openNotes("b_on"); // toggle ON
  await Bun.sleep(1000);
  out.b_after_reopen = await elementsSnapshot("b_after");
  out.b_settext_after_reopen = { ok: await setText(`# Diag Note ${runId}\n\n${marker}\nB-extra\n`) };
} finally {
  console.log(JSON.stringify(out, null, 2));
  await driver.close();
}
