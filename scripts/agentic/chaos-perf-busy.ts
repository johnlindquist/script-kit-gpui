#!/usr/bin/env bun
/** Keep the app churning on a long non-ASCII query so `sample <pid>` can profile the hot stack. */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");
const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
console.log("APP_PID=" + (d as any).proc.pid);
d.send({ type: "triggerBuiltin", name: "mainList" });
await Bun.sleep(500);
const big = "thé qüick bröwn 🦊 jümps övér thé lazy døg 素早い ".repeat(180); // ~8000 chars, non-ASCII
// Fire many distinct filters (distinct so the cache never short-circuits) for ~14s.
const start = performance.now();
let i = 0;
while (performance.now() - start < 14000) {
  d.setFilter(big + " " + i);
  i++;
  await Bun.sleep(120);
}
console.log("fired " + i + " filters");
await d.close();
