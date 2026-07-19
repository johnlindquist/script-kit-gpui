#!/usr/bin/env bun
/** Attribute the long-query stall: app's own PERF search timing vs. total getState. */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");
const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
d.send({ type: "triggerBuiltin", name: "mainList" });
await Bun.sleep(400);
await d.waitForSettle({ timeoutMs: 3000 });

for (const n of [500, 1500, 3000]) {
  const s = "the quick brown fox jumps over the lazy dog ".repeat(Math.ceil(n / 44)).slice(0, n);
  const t0 = performance.now();
  d.setFilter(s);
  let total = 0;
  try { await d.getState({ timeoutMs: 30000 }); total = Math.round(performance.now() - t0); }
  catch { total = Math.round(performance.now() - t0); }
  // Pull the app's own PERF measurement for this search.
  const logs = await d.getLogs({ target: "PERF", limit: 20 }, { timeoutMs: 5000 }).catch(() => ({ entries: [] }));
  const entries: any[] = (logs as any).entries ?? (logs as any).logs ?? [];
  const perf = entries.reverse().find((e: any) => (e.message || "").startsWith("Search '"));
  const m = perf ? String(perf.message).match(/took ([\d.]+)ms/) : null;
  const searchMs = m ? parseFloat(m[1]) : null;
  console.error(`chars=${String(n).padStart(5)}  total=${String(total).padStart(6)}ms  app_search=${searchMs != null ? searchMs.toFixed(0) + "ms" : "n/a"}  (search/total=${searchMs != null && total ? (searchMs / total * 100).toFixed(0) + "%" : "?"})`);
  d.setFilter("");
  await Bun.sleep(250);
}
await d.close();
console.log("done");
