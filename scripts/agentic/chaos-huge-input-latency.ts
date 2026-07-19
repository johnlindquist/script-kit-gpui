#!/usr/bin/env bun
/** Measure filter→getState latency across input sizes to characterize the s4 huge-input stall. */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");

const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
d.send({ type: "triggerBuiltin", name: "mainList" });
await Bun.sleep(400);
await d.waitForSettle({ timeoutMs: 3000 });

const sizes = [50, 500, 1500, 3000, 5000, 8000, 12000];
const results: any[] = [];
for (const n of sizes) {
  const s = "The quick brown 🦊 jumps over the lazy dog. ".repeat(Math.ceil(n / 44)).slice(0, n);
  d.setFilter(s);
  const t0 = performance.now();
  let ms: number, ok = true, vis: any = null;
  try {
    const st = await d.getState({ timeoutMs: 20000 });
    ms = Math.round(performance.now() - t0);
    vis = st.visibleChoiceCount;
  } catch (e) {
    ms = Math.round(performance.now() - t0);
    ok = false;
  }
  results.push({ chars: n, getStateMs: ms, ok, visible: vis });
  console.error(`  chars=${String(n).padStart(6)}  getState=${String(ms).padStart(6)}ms  ok=${ok}  vis=${vis}`);
  d.setFilter("");
  await Bun.sleep(250);
}
await d.close();
console.log(JSON.stringify({ results }, null, 2));
