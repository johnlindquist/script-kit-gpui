#!/usr/bin/env bun
/** Battery D (live): fuzz the stdin command dispatch with hostile-but-valid-JSON
 *  frames and verify the app never crashes and stays responsive. Protocol-only. */
import { Driver } from "../devtools/driver";
import { join } from "node:path";
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");
const d = await Driver.launch({ sandboxHome: true, binary: BINARY });
d.send({ type: "show" });
await Bun.sleep(400);
await d.waitForSettle({ timeoutMs: 3000 });

// Build a deeply nested object (~600 levels) to test recursion/stack handling.
let nested: any = { leaf: true };
for (let i = 0; i < 600; i++) nested = { n: nested };

const BIG = "x".repeat(200_000);
const HUGE = Number.MAX_SAFE_INTEGER;

const frames: [string, any][] = [
  ["unknown-type", { type: "totallyUnknownCommand", foo: "bar" }],
  ["empty-type", { type: "" }],
  ["no-type", { foo: "bar" }],
  ["setFilter-missing-text", { type: "setFilter" }],
  ["setFilter-text-number", { type: "setFilter", text: 12345 }],
  ["setFilter-text-null", { type: "setFilter", text: null }],
  ["setFilter-text-object", { type: "setFilter", text: { a: 1 } }],
  ["setFilter-text-array", { type: "setFilter", text: [1, 2, 3] }],
  ["setFilter-oversized", { type: "setFilter", text: BIG }],
  ["simulateKey-missing-key", { type: "simulateKey" }],
  ["simulateKey-key-number", { type: "simulateKey", key: 999 }],
  ["simulateKey-bad-key", { type: "simulateKey", key: "NotARealKey", modifiers: ["notamod"] }],
  ["simulateKey-empty-key", { type: "simulateKey", key: "" }],
  ["simulateKey-huge-modifiers", { type: "simulateKey", key: "down", modifiers: Array.from({ length: 5000 }, () => "cmd") }],
  ["triggerBuiltin-name-array", { type: "triggerBuiltin", name: [] }],
  ["triggerBuiltin-unknown", { type: "triggerBuiltin", name: "no-such-builtin-xyz" }],
  ["triggerBuiltin-huge-name", { type: "triggerBuiltin", name: BIG }],
  ["deeply-nested", { type: "getState", junk: nested }],
  ["huge-number-field", { type: "simulateKey", key: "down", repeat: HUGE, count: 1e308 }],
  ["negative-numbers", { type: "getElements", index: -999999, depth: -1 }],
  ["waitFor-malformed", { type: "waitFor", condition: { type: "notreal", state: 42 }, timeout: -5 }],
  ["batch-hostile", { type: "batch", commands: [{ type: "setFilter", text: 1 }, { type: "???" }, null, 42], options: { timeout: -1 } }],
  ["requestId-object", { type: "getState", requestId: { not: "a string" } }],
  ["null-command", null],
  ["array-command", [1, 2, 3]],
  ["bool-command", true],
  ["number-command", 42],
];

let crashed = "";
let sent = 0;
for (const [label, frame] of frames) {
  try {
    d.send(frame as any); // fire-and-forget hostile frame
    sent++;
    await Bun.sleep(60);
    // Liveness probe after each hostile frame.
    const s = await d.getState({ timeoutMs: 8000 });
    if (!s || typeof s !== "object") { crashed = `${label}: bad state`; break; }
  } catch (e) {
    crashed = `${label}: ${String(e).slice(0, 120)}`;
    break;
  }
}

// Final functional check: does a normal filter still work after all that abuse?
let functional = false;
try {
  d.setFilter("");
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await Bun.sleep(200);
  d.setFilter("recover-check");
  await Bun.sleep(200);
  const s = await d.getState({ timeoutMs: 8000 });
  functional = s.inputValue === "recover-check";
} catch (e) { crashed = crashed || `recovery: ${String(e).slice(0, 120)}`; }

await d.close();
const verdict = crashed ? "FAIL" : functional ? "PASS" : "SUSPECT";
console.log(JSON.stringify({ verdict, sent, totalFrames: frames.length, crashed: crashed || null, functionalAfter: functional }, null, 2));
console.error(`[${verdict}] protocol-fuzz: sent ${sent}/${frames.length} hostile frames; ${crashed ? "CRASH: " + crashed : "app alive"}; functional-after=${functional}`);
