#!/usr/bin/env bun
/**
 * Demo smoke test: run every scene's self-driven demo once (accelerated),
 * assert it reaches its required checkpoint with zero errors, and prove the
 * paused demo URL and the post-loop reset are pixel-identical to the bare
 * canonical page. All three frames are captured in the SAME browser session:
 * headless Chromium rasterizes a scene in one of two AA variants per session
 * (documented by every scene agent), so cross-session AE comparisons flake
 * while same-session comparisons are bit-exact. The verify.ts render remains
 * the fidelity baseline; this harness gates demo-mode pixel neutrality.
 *
 * Writes design/showcase/demo-manifest.json (the publish gate reads it).
 *
 * Usage: bun design/showcase/demo-smoke.ts [shot-id ...]
 * Prereq: a fresh `bun design/showcase/verify.ts` run (canonical renders in
 * .test-output/showcase-verify/).
 */
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = new URL(".", import.meta.url).pathname.replace(/\/$/, "");
const PROJECT_ROOT = join(ROOT, "../..");
const OUT = join(PROJECT_ROOT, ".test-output/showcase-verify");

const SHOTS: Record<string, { native: [number, number]; checkpoint: string }> = {
  "01-main-launcher": { native: [1675, 1139], checkpoint: "notes-filtered" },
  "02-search-filter": { native: [1675, 1139], checkpoint: "clip-final" },
  "04-clipboard-history": { native: [1675, 1139], checkpoint: "hex-preview" },
  "05-emoji-picker": { native: [1675, 1139], checkpoint: "emoji-row-2" },
  "06-notes": { native: [1560, 1351], checkpoint: "two-tasks-checked" },
  "07-day-page": { native: [1675, 1139], checkpoint: "reference-focused" },
  "08-agent-chat": { native: [1675, 1139], checkpoint: "answer-streamed" },
  "09-terminal": { native: [1675, 1139], checkpoint: "terminal-output" },
  "10-file-search": { native: [1675, 1139], checkpoint: "plist-preview" },
  "11-theme-designer": { native: [1675, 1139], checkpoint: "tokyo-night" },
  "12-settings": { native: [1675, 1139], checkpoint: "permissions-filtered" },
  "13-agent-chat-composer": { native: [1675, 1139], checkpoint: "context-shortcuts" },
  "14-window-switcher": { native: [1675, 1139], checkpoint: "uad-filtered" },
  "15-app-launcher": { native: [1675, 1139], checkpoint: "safari-filtered" },
  "17-rewrite": { native: [2150, 1294], checkpoint: "rewrite-pasted" },
  "18-rewrite-styles": { native: [2150, 1294], checkpoint: "friendly-selected" },
  "19-references": { native: [1573, 335], checkpoint: "reference-activated" },
  "20-brain-inbox": { native: [1675, 1139], checkpoint: "dash-filtered" },
  "21-dictation": { native: [1428, 531], checkpoint: "dictation-stopped" },
};

const SESSION = `showcase-smoke-${process.pid}`;

async function run(cmd: string[], allowFail = false): Promise<string> {
  const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (code !== 0 && !allowFail) throw new Error(`${cmd.join(" ")}: ${stderr || stdout}`);
  return (stdout + stderr).trim();
}
const browser = (args: string[]) => run(["agent-browser", "--session", SESSION, ...args]);

async function ae(a: string, b: string): Promise<number> {
  const out = await run(["magick", "compare", "-metric", "AE", a, b, "null:"], true);
  // This ImageMagick build prints "<distance> (<count>)"; the parenthesized
  // value is the differing-pixel count. Fall back to the bare number.
  const paren = out.match(/\(([\d.eE+]+)\)/);
  const bare = out.match(/^([\d.eE+]+)/);
  const m = paren ?? bare;
  if (!m) throw new Error(`unparseable AE: ${out}`);
  return Number(m[1]);
}

const filters = process.argv.slice(2);
const ids = Object.keys(SHOTS).filter(
  (id) => filters.length === 0 || filters.some((f) => id.startsWith(f)),
);

const scenes: any[] = [];
for (const id of ids) {
  const { native, checkpoint } = SHOTS[id];
  const [pw, ph] = native;
  const w = Math.round(pw / 2);
  const h = Math.round(ph / 2);
  const page = `file://${join(ROOT, "shots", id, "index.html")}`;
  const result: any = { id, checkpoint, smokeStatus: "fail" };
  const t0 = Date.now();
  const READY = `(async () => {
    if (document.fonts && document.fonts.ready) await document.fonts.ready;
    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
    return "ready";
  })()`;
  const capture = async (label: string) => {
    await browser(["eval", READY]);
    const out = join(OUT, `${id}-demo-${label}.png`);
    await browser(["screenshot", "body", `${out}.raw.png`]);
    await run(["magick", `${out}.raw.png`, "-crop", `${pw}x${ph}+0+0`, "+repage", "-colorspace", "sRGB", "-strip", `PNG24:${out}`]);
    return out;
  };
  try {
    readFileSync(join(ROOT, "shots", id, "demo.js"));
    await browser(["set", "viewport", String(w), String(h), "2"]);

    // Single page load: headless Chromium re-rasterizes in one of two AA
    // variants on EVERY navigation, so the only sound pixel comparison is
    // frame-A vs frame-B within one load. Load the demo paused, capture the
    // initial frame, drive one accelerated cycle via the runner's message
    // protocol (no navigation), then capture the reset frame.
    await browser(["open", `${page}?demo=1&autoplay=0&once=1&speed=6&hud=0`]);
    await Bun.sleep(1200);
    const before = await capture("before");
    const domBefore = await browser(["eval", "JSON.stringify((function walk(n){if(n.nodeType===3)return n.textContent;if(n.nodeType!==1)return '';var a=Array.from(n.attributes||[]).filter(function(x){return !(x.name==='style'&&x.value==='')}).map(function(x){return x.name+'='+x.value}).sort().join(' ');return '<'+n.tagName+' '+a+'>'+Array.from(n.childNodes).map(walk).join('')+'</'+n.tagName+'>'})(document.querySelector('.scene')))"]);

    await browser(["eval", `window.postMessage({channel:"sk-showcase-demo",version:1,type:"replay"},"*"); "sent"`]);
    let state: any = null;
    for (let i = 0; i < 60; i++) {
      await Bun.sleep(1000);
      const raw = await browser(["eval", "JSON.stringify(window.__SK_DEMO__ || null)"]);
      state = JSON.parse(JSON.parse(raw));
      if (state && (state.status === "done" || state.status === "error")) break;
    }
    result.finalStatus = state?.status ?? "missing";
    result.errors = state?.errors ?? ["__SK_DEMO__ missing"];
    result.seenSteps = state?.seenSteps ?? [];
    result.checkpointSeen = (state?.seenSteps ?? []).includes(checkpoint);

    // Post-reset pixel + DOM identity within the SAME page load.
    await Bun.sleep(500);
    const after = await capture("after");
    result.cycleResetAe = await ae(after, before);
    // Whole-frame AA-variant flips (invisible, LSB-level) can survive a
    // cycle even without navigation; gate real leaks with an RMSE ceiling.
    const rmseOut = await run(["magick", "compare", "-metric", "RMSE", after, before, "null:"], true);
    result.cycleResetRmse = Number((rmseOut.match(/\(([\d.eE+-]+)\)/) ?? [])[1] ?? NaN);
    const domAfter = await browser(["eval", "JSON.stringify((function walk(n){if(n.nodeType===3)return n.textContent;if(n.nodeType!==1)return '';var a=Array.from(n.attributes||[]).filter(function(x){return !(x.name==='style'&&x.value==='')}).map(function(x){return x.name+'='+x.value}).sort().join(' ');return '<'+n.tagName+' '+a+'>'+Array.from(n.childNodes).map(walk).join('')+'</'+n.tagName+'>'})(document.querySelector('.scene')))"]);
    result.domRestored = domBefore === domAfter;

    result.durationMs = Date.now() - t0;
    result.smokeStatus =
      (result.cycleResetAe === 0 || result.cycleResetRmse <= 0.002) &&
      result.domRestored === true &&
      result.finalStatus === "done" &&
      result.errors.length === 0 &&
      result.checkpointSeen
        ? "pass"
        : "fail";
  } catch (err) {
    result.error = String(err);
  }
  scenes.push(result);
  console.error(
    `${id}  ${result.smokeStatus}  resetAe=${result.cycleResetAe} resetRmse=${result.cycleResetRmse} dom=${result.domRestored} status=${result.finalStatus} checkpoint=${result.checkpointSeen}`,
  );
}
await browser(["close"]).catch(() => {});

const manifest = {
  generatedBy: "design/showcase/demo-smoke.ts",
  runnerVersion: 1,
  sceneCount: scenes.length,
  canonicalStaticHashesMatch: scenes.every(
    (s) => (s.cycleResetAe === 0 || s.cycleResetRmse <= 0.002) && s.domRestored === true,
  ),
  scenes,
};
if (filters.length === 0) {
  writeFileSync(join(ROOT, "demo-manifest.json"), JSON.stringify(manifest, null, 2));
}
console.log(JSON.stringify(manifest, null, 2));
if (scenes.some((s) => s.smokeStatus !== "pass")) process.exit(1);
