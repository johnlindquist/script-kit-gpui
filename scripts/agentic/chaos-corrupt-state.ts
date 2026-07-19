#!/usr/bin/env bun
/** Battery E (live): does the app degrade gracefully when its own state files are
 *  malformed/corrupt/wrong-shaped at startup — or does it panic/hang?
 *  Safe: uses a throwaway HOME (never the real ~/.scriptkit); models symlinked to
 *  avoid downloads. */
import { Driver } from "../devtools/driver";
import { mkdirSync, writeFileSync, symlinkSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");

type Case = { name: string; seed: (kit: string) => void };
const cases: Case[] = [
  { name: "invalid-json-input-history", seed: (k) => writeFileSync(join(k, "input_history.json"), "{{{ not json at all \x00\xff") },
  { name: "wrong-shape-input-history", seed: (k) => writeFileSync(join(k, "input_history.json"), JSON.stringify({ entries: "should-be-array", selected_results: 42 })) },
  { name: "input-history-is-a-directory", seed: (k) => mkdirSync(join(k, "input_history.json")) },
  { name: "huge-input-history", seed: (k) => writeFileSync(join(k, "input_history.json"), JSON.stringify({ entries: Array.from({ length: 100000 }, (_, i) => "e" + i) })) },
  { name: "truncated-secrets-age", seed: (k) => writeFileSync(join(k, "secrets.age"), "age-encryption.org/v1\n-> scrypt\nTRUNCATED-GARBAGE") },
  { name: "corrupt-config-json", seed: (k) => writeFileSync(join(k, "config-loader-cache.v1.json"), "\x00\x01\x02 not json {[}") },
  { name: "window-state-garbage", seed: (k) => writeFileSync(join(k, "window_state.json"), "not-valid-window-state") },
];

const results: any[] = [];
for (const c of cases) {
  const root = `/tmp/sk-corrupt-${process.pid}-${c.name}-${Date.now().toString(36)}`;
  const home = join(root, "home");
  const kit = join(home, ".scriptkit");
  mkdirSync(kit, { recursive: true });
  // Symlink the real model cache so nothing tries to download.
  try { symlinkSync(join(homedir(), ".scriptkit", "models"), join(kit, "models")); } catch {}
  c.seed(kit);

  let ready = false, alive = false, err = "";
  try {
    const d = await Driver.launch({ binary: BINARY, sandboxHome: false, env: { HOME: home, SK_PATH: kit } });
    ready = true; // launch resolves only after STARTUP_READY / APP_READY
    d.send({ type: "triggerBuiltin", name: "mainList" });
    await Bun.sleep(300);
    d.setFilter("probe");
    await Bun.sleep(200);
    const s = await d.getState({ timeoutMs: 8000 });
    alive = s.inputValue === "probe";
    await d.close();
  } catch (e) { err = String(e).slice(0, 160); }
  const verdict = err ? "FAIL" : (ready && alive) ? "PASS" : "SUSPECT";
  results.push({ name: c.name, verdict, ready, alive, err });
  console.error(`  [${verdict}] ${c.name}${err ? " — " + err : ready && alive ? " — started + usable" : " — started but not confirmed usable"}`);
  try { rmSync(root, { recursive: true, force: true }); } catch {}
}
console.log(JSON.stringify({
  counts: { pass: results.filter(r => r.verdict === "PASS").length, suspect: results.filter(r => r.verdict === "SUSPECT").length, fail: results.filter(r => r.verdict === "FAIL").length },
  results,
}, null, 2));
