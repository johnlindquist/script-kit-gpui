#!/usr/bin/env bun
/** Battery I: hostile encoding edge cases fired into the live launcher filter.
 *  All special characters use explicit escapes so the hostile bytes are real. */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");
const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
d.send({ type: "triggerBuiltin", name: "mainList" });
await Bun.sleep(400);
await d.waitForSettle({ timeoutMs: 3000 });

const cases: [string, string][] = [
  ["null-bytes", "abc\x00def\x00ghi"],
  ["rtl-hebrew", "שלום test"],
  ["rtl-arabic", "مرحبا search"],
  ["combining", "é́̀ test"], // e + stacked combining marks
  ["zero-width", "a​b‌c‍d﻿e"], // ZWSP/ZWNJ/ZWJ/BOM
  ["control-chars", "a\x01b\x07c\x1bd\x7fe"], // SOH/BEL/ESC/DEL
  ["bom-prefix", "﻿filter after bom"],
  ["astral-plane", "\u{1D54F}\u{1D550}\u{1D56B} math bold"],
  ["bidi-override", "abc‮mix‬‭end‬"], // RLO/PDF/LRO
  ["huge-combining", "a" + "́".repeat(2000)], // 2000 combining marks on one base
  ["mixed-newlines", "line1\nline2\r\nline3\rline4"],
  ["huge-rtl", "שלום ".repeat(400)], // long RTL
];

let crashed = false;
for (const [label, q] of cases) {
  const t0 = performance.now();
  d.setFilter(q);
  let ms: number, ok = true, vis: any = null, inp: any = null;
  try { const s = await d.getState({ timeoutMs: 15000 }); ms = Math.round(performance.now() - t0); vis = s.visibleChoiceCount; inp = (s.inputValue ?? "").length; }
  catch { ms = Math.round(performance.now() - t0); ok = false; crashed = true; }
  console.error(`${label.padEnd(16)} getState=${String(ms).padStart(6)}ms vis=${vis} inLen=${inp} ${ok ? "" : "*** CRASH/HANG ***"}${ms > 1000 ? " <-- slow" : ""}`);
  d.setFilter("");
  await Bun.sleep(150);
}
let alive = false; try { await d.getState({ timeoutMs: 5000 }); alive = true; } catch {}
await d.close();
console.log(JSON.stringify({ finalAlive: alive, anyCrash: crashed }));
