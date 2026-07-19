#!/usr/bin/env bun
/**
 * chaos-interaction-stress.ts — battery G/H: rapid interaction churn against the
 * live app (protocol-only, sandbox HOME, no OS input). Hunts for dropped state,
 * stuck popups, out-of-bounds selection, coalescing bugs, and panics under load.
 * Classifies each scenario PASS/SUSPECT/FAIL on app-alive + expected state + no
 * new product error-log (GPUI show/capture re-entrancy + screenshot noise ignored).
 */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");

type R = { id: string; verdict: string; reason: string; errs: string[] };
const results: R[] = [];

const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
d.send({ type: "show" });
await Bun.sleep(400);
await d.waitForSettle({ timeoutMs: 4000 });

const errSet = async (): Promise<Set<string>> => {
  try {
    const r = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
    const es: any[] = (r as any).entries ?? (r as any).logs ?? [];
    return new Set(es.map((e: any) => `${e.target ?? ""}|${e.message ?? ""}`));
  } catch { return new Set(); }
};
const isNoise = (k: string) =>
  /captureScreenshot|automation window|screenshot/i.test(k) ||
  (/RefCell already borrowed/i.test(k) && /vendor\/gpui\/src\/window\.rs/i.test(k));

const st = async () => { try { return await d.getState({ timeoutMs: 8000 }); } catch (e) { return { __crashed: String(e) }; } };
const reset = async () => {
  for (let i = 0; i < 3; i++) { d.simulateKey("escape"); await Bun.sleep(40); }
  d.setFilter(""); await Bun.sleep(100);
  d.send({ type: "triggerBuiltin", name: "mainList" }); await Bun.sleep(150);
  await d.waitForSettle({ timeoutMs: 3000 });
};

async function scenario(id: string, body: () => Promise<{ ok: boolean; reason: string }>) {
  await reset();
  const before = await errSet();
  let out = { ok: false, reason: "threw" };
  try { out = await body(); } catch (e) { out = { ok: false, reason: `exception: ${String(e).slice(0, 160)}` }; }
  const after = await errSet();
  const errs = [...after].filter((k) => !before.has(k) && !isNoise(k)).slice(0, 6);
  const alive = !(await st()).__crashed;
  const verdict = !alive ? "FAIL" : errs.length ? "SUSPECT" : out.ok ? "PASS" : "SUSPECT";
  results.push({ id, verdict, reason: errs[0] ? `app error: ${errs[0]}` : out.reason, errs });
  console.error(`  [${verdict}] ${id} — ${errs[0] ?? out.reason}`);
}

// 1. Rapid builtin open/close churn (lifecycle).
await scenario("churn-builtins", async () => {
  const names = ["clipboardHistory", "emojiPicker", "tips", "fileSearch", "mainList"];
  for (let i = 0; i < 20; i++) {
    d.send({ type: "triggerBuiltin", name: names[i % names.length] });
    await Bun.sleep(30);
    d.simulateKey("escape");
    await Bun.sleep(20);
  }
  const s = await st();
  return { ok: !s.__crashed, reason: `survived 20 open/close cycles (view=${s.promptType ?? s.view})` };
});

// 2. Keystroke flooding — 150 arrow keys, no waits.
await scenario("arrow-flood", async () => {
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await Bun.sleep(100);
  for (let i = 0; i < 150; i++) d.simulateKey(i % 3 === 0 ? "up" : "down");
  await Bun.sleep(300);
  const s = await st();
  const sel = s.selectedIndex ?? s.selectedRow;
  return { ok: !s.__crashed && (sel == null || sel >= 0), reason: `150 arrows → sel=${sel} vis=${s.visibleChoiceCount}` };
});

// 3. Filter coalescing — fire 40 distinct filters back-to-back, assert final wins.
await scenario("filter-coalesce", async () => {
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await Bun.sleep(100);
  for (let i = 0; i < 40; i++) d.setFilter("q" + i);
  d.setFilter("final-query-xyz");
  await Bun.sleep(400);
  const s = await st();
  return { ok: !s.__crashed && s.inputValue === "final-query-xyz", reason: `after 41 rapid filters, inputValue=${JSON.stringify(s.inputValue)}` };
});

// 4. Actions menu (Cmd+K) open → type in it → escape → confirm main usable.
await scenario("actions-nested", async () => {
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await Bun.sleep(120);
  d.simulateKey("k", ["cmd"]); await Bun.sleep(120);
  d.setFilter("copy"); await Bun.sleep(120);
  d.simulateKey("escape"); await Bun.sleep(80);
  d.simulateKey("escape"); await Bun.sleep(80);
  d.setFilter("recover-probe"); await Bun.sleep(200);
  const s = await st();
  return { ok: !s.__crashed && s.inputValue === "recover-probe", reason: `actions open/type/escape → main input=${JSON.stringify(s.inputValue)}` };
});

// 5. Rapid show/hide toggle (window lifecycle).
await scenario("show-hide-toggle", async () => {
  for (let i = 0; i < 15; i++) { d.send({ type: i % 2 ? "hide" : "show" }); await Bun.sleep(40); }
  d.send({ type: "show" }); await Bun.sleep(150);
  d.setFilter("post-toggle"); await Bun.sleep(200);
  const s = await st();
  return { ok: !s.__crashed && s.inputValue === "post-toggle", reason: `15 show/hide toggles → input=${JSON.stringify(s.inputValue)}` };
});

// 6. Escape flood on main (dismiss grammar) — 30 escapes, then must still accept input.
await scenario("escape-flood", async () => {
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await Bun.sleep(100);
  for (let i = 0; i < 30; i++) { d.simulateKey("escape"); await Bun.sleep(15); }
  d.send({ type: "show" }); await Bun.sleep(120);
  d.setFilter("after-escapes"); await Bun.sleep(200);
  const s = await st();
  return { ok: !s.__crashed && s.inputValue === "after-escapes", reason: `30 escapes → input=${JSON.stringify(s.inputValue)}` };
});

let alive = false; try { await d.getState({ timeoutMs: 5000 }); alive = true; } catch {}
await d.close();
console.log(JSON.stringify({
  finalAlive: alive,
  counts: { pass: results.filter(r => r.verdict === "PASS").length, suspect: results.filter(r => r.verdict === "SUSPECT").length, fail: results.filter(r => r.verdict === "FAIL").length },
  results,
}, null, 2));
