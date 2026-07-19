#!/usr/bin/env bun
/** Confirm the F1 non-ASCII long-query perf cliff isn't lurking on other filterable surfaces. */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");
const d = await Driver.launch({ sandboxHome: true, binary: BINARY });

const q = "thé qüick 🦊 素早い bröwn ".repeat(130).slice(0, 3000); // ~3000 chars, non-ASCII
const surfaces = ["mainList", "emojiPicker", "fileSearch", "clipboardHistory", "appLauncher"];
for (const name of surfaces) {
  d.send({ type: "triggerBuiltin", name });
  await Bun.sleep(300);
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  const t0 = performance.now();
  d.setFilter(q);
  let ms: number, ok = true;
  try { await d.getState({ timeoutMs: 20000 }); ms = Math.round(performance.now() - t0); }
  catch { ms = Math.round(performance.now() - t0); ok = false; }
  console.error(`${name.padEnd(18)} 3000-char non-ASCII filter → getState=${String(ms).padStart(6)}ms ${ok ? "" : "TIMEOUT"} ${ms > 1000 ? "  <-- CLIFF?" : ""}`);
  d.setFilter("");
  for (let i = 0; i < 3; i++) { d.simulateKey("escape"); await Bun.sleep(40); }
}
await d.close();
console.log("done");
