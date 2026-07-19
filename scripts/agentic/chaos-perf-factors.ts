#!/usr/bin/env bun
/** Isolate what makes a long query slow: length is held ~constant, one factor varies. */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");

function rep(unit: string, target = 3000): string {
  return unit.repeat(Math.ceil(target / unit.length)).slice(0, target);
}
const cases: [string, string][] = [
  ["ascii-lower", rep("the quick brown fox jumps over the lazy dog ")],
  ["ascii-Title", rep("The Quick Brown Fox Jumps Over The Lazy Dog ")],
  ["ascii-punct", rep("the, quick. brown! fox? jumps; over: the (lazy) dog ")],
  ["accented", rep("thé qüick bröwn fox jümps övér thé lazy døg ")],
  ["emoji-sparse", rep("the quick brown 🦊 jumps over the lazy dog ")],
  ["emoji-dense", rep("🦊🌊🎉🚀✨🔥💧🌍👋🎊 ")],
  ["cjk", rep("素早い茶色のキツネが怠け者の犬を飛び越える ")],
];

const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
d.send({ type: "triggerBuiltin", name: "mainList" });
await Bun.sleep(400);
await d.waitForSettle({ timeoutMs: 3000 });

for (const [label, q] of cases) {
  const t0 = performance.now();
  d.setFilter(q);
  let ms: number, ok = true;
  try { await d.getState({ timeoutMs: 30000 }); ms = Math.round(performance.now() - t0); }
  catch { ms = Math.round(performance.now() - t0); ok = false; }
  console.error(`${label.padEnd(14)} len=${q.length}  chars=${[...q].length}  getState=${String(ms).padStart(6)}ms  ${ok ? "" : "TIMEOUT"}`);
  d.setFilter("");
  await Bun.sleep(300);
}
await d.close();
console.log("done");
