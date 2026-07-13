#!/usr/bin/env bun
/**
 * Consolidated showcase verification (v2 — exact native canvas).
 *
 * Per shot:
 *   - Render the scene headless at its logical size, DPR 2.
 *   - Wait for document.fonts.ready + two animation frames (no fixed sleep);
 *     animations/transitions are disabled for determinism.
 *   - CROP the render to the reference's NATIVE pixel size. The reference is
 *     NEVER resized (the v1 harness resized it by ±1px, smearing sub-pixel
 *     error across the whole frame and inflating glyph-edge noise).
 *   - Both sides normalized to sRGB, profiles stripped; dimensions asserted.
 *   - Metric vector (never collapsed into one score):
 *       rmse               global normalized RMSE (strict regression guard)
 *       ssimDissimilarity  local ImageMagick SSIM prints a dissimilarity in
 *                          the normalized slot (self-compare = 0)
 *       lowPassRmse        RMSE after identical 0x1 gaussian blur on both
 *                          sides (suppresses JPEG grain/AA noise, keeps
 *                          geometry error)
 *       windowRmse         RMSE inside the app-window rect (UI anatomy/text)
 *       wallpaperRmse      derived RMSE outside the window rect
 *
 * Writes design/showcase/verify-summary.json (repo-relative paths) and
 * per-shot artifacts under .test-output/showcase-verify/.
 *
 * Usage: bun design/showcase/verify.ts [shot-id ...]
 */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL(".", import.meta.url).pathname.replace(/\/$/, "");
const PROJECT_ROOT = join(ROOT, "../..");
const OUT = join(PROJECT_ROOT, ".test-output/showcase-verify");

/** id → native reference JPG size in physical pixels (2x captures). */
const SHOTS: Record<string, [number, number]> = {
  "01-main-launcher": [1675, 1139],
  "02-search-filter": [1675, 1139],
  "04-clipboard-history": [1675, 1139],
  "05-emoji-picker": [1675, 1139],
  "06-notes": [1560, 1351],
  "07-day-page": [1675, 1139],
  "08-agent-chat": [1675, 1139],
  "09-terminal": [1675, 1139],
  "10-file-search": [1675, 1139],
  "11-theme-designer": [1675, 1139],
  "12-settings": [1675, 1139],
  "13-agent-chat-composer": [1675, 1139],
  "14-window-switcher": [1675, 1139],
  "15-app-launcher": [1675, 1139],
  "17-rewrite": [2150, 1294],
  "18-rewrite-styles": [2150, 1294],
  "19-references": [1573, 335],
  "20-brain-inbox": [1675, 1139],
  "21-dictation": [1428, 531],
};

/** Selector list used to locate the app-window rect inside a scene. A scene
 * may pin its own with [data-verify-window]. */
const WINDOW_SELECTOR =
  "[data-verify-window], .sk-window, .win, .window, .te-win, .te-window";

const SESSION = `showcase-verify-${process.pid}`;

async function browser(args: string[]): Promise<string> {
  const proc = Bun.spawn(["agent-browser", "--session", SESSION, ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (code !== 0) throw new Error(`agent-browser ${args.join(" ")}: ${stderr || stdout}`);
  return stdout.trim();
}

async function magick(args: string[]): Promise<{ code: number; out: string }> {
  const proc = Bun.spawn(["magick", ...args], { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  return { code, out: (stdout + stderr).trim() };
}

/** Normalized value from `magick compare` output "N (norm)". */
function parseNorm(out: string): number {
  const m = out.match(/\(([\d.eE+-]+)\)/);
  if (!m) throw new Error(`unparseable compare output: ${out}`);
  return Number(m[1]);
}

async function dims(path: string): Promise<[number, number]> {
  const r = await magick(["identify", "-format", "%w %h", path]);
  const [w, h] = r.out.split(/\s+/).map(Number);
  return [w, h];
}

async function compareMetric(
  metric: string,
  a: string,
  b: string,
  diff: string | null,
): Promise<number> {
  const r = await magick(["compare", "-metric", metric, a, b, diff ?? "null:"]);
  // compare exits 1 on "images differ" — only 2+ is a hard error.
  if (r.code > 1) throw new Error(`magick compare ${metric}: ${r.out}`);
  return parseNorm(r.out);
}

/** Readiness + measurement script evaluated inside the scene page. */
const READY_EVAL = `(async () => {
  const style = document.createElement("style");
  style.textContent = "*{animation:none!important;transition:none!important;caret-color:transparent!important}";
  document.head.appendChild(style);
  if (document.fonts && document.fonts.ready) await document.fonts.ready;
  await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
  const scene = document.querySelector(".scene");
  const win = document.querySelector(${JSON.stringify(WINDOW_SELECTOR)});
  const rect = win ? win.getBoundingClientRect() : null;
  return JSON.stringify({
    scene: scene ? { w: scene.offsetWidth, h: scene.offsetHeight } : null,
    window: rect ? { x: rect.x, y: rect.y, w: rect.width, h: rect.height } : null,
    fonts: Array.from(new Set(Array.from(document.querySelectorAll(".scene, .scene *")).slice(0, 40).map(el => getComputedStyle(el).fontFamily))).slice(0, 5),
    ua: navigator.userAgent,
  });
})()`;

const filters = process.argv.slice(2);
const ids = Object.keys(SHOTS).filter(
  (id) => filters.length === 0 || filters.some((f) => id.startsWith(f)),
);

mkdirSync(OUT, { recursive: true });
const magickVersion = (await magick(["--version"])).out.split("\n")[0];
const summary: any[] = [];
let userAgent: string | null = null;

for (const id of ids) {
  const [pw, ph] = SHOTS[id];
  const w = Math.round(pw / 2);
  const h = Math.round(ph / 2);
  const page = join(ROOT, "shots", id, "index.html");
  const renderRaw = join(OUT, `${id}-render-raw.png`);
  const render = join(OUT, `${id}-render.png`);
  const refPng = join(OUT, `${id}-ref.png`);
  const diff = join(OUT, `${id}-diff.png`);
  try {
    readFileSync(page);
  } catch {
    summary.push({ id, status: "missing" });
    continue;
  }
  try {
    await browser(["set", "viewport", String(w), String(h), "2"]);
    await browser(["open", `file://${page}`]);
    const ready = JSON.parse(await browser(["eval", READY_EVAL]).then((s) => JSON.parse(s)));
    userAgent = ready.ua ?? userAgent;
    if (!ready.scene) throw new Error("no .scene element after readiness wait");
    await browser(["screenshot", "body", renderRaw]);

    // Render is 2w×2h physical (≥ native). Crop to the native canvas; never
    // resize the reference.
    const [rw, rh] = await dims(renderRaw);
    if (rw < pw || rh < ph) throw new Error(`render ${rw}x${rh} smaller than native ${pw}x${ph}`);
    await magick([renderRaw, "-crop", `${pw}x${ph}+0+0`, "+repage", "-colorspace", "sRGB", "-strip", `PNG24:${render}`]);
    await magick([join(ROOT, "reference", `${id}.jpg`), "-auto-orient", "-colorspace", "sRGB", "-strip", `PNG24:${refPng}`]);
    const [aw, ah] = await dims(render);
    const [bw, bh] = await dims(refPng);
    if (aw !== bw || ah !== bh) throw new Error(`canvas mismatch render=${aw}x${ah} ref=${bw}x${bh}`);

    const rmse = await compareMetric("RMSE", render, refPng, diff);
    const ssimDissimilarity = await compareMetric("SSIM", render, refPng, null);

    // Low-pass: identical small blur on both sides.
    const lpA = join(OUT, `${id}-lp-render.png`);
    const lpB = join(OUT, `${id}-lp-ref.png`);
    await magick([render, "-gaussian-blur", "0x1", lpA]);
    await magick([refPng, "-gaussian-blur", "0x1", lpB]);
    const lowPassRmse = await compareMetric("RMSE", lpA, lpB, null);

    // Window-region metrics (physical px = CSS rect × 2, clamped to canvas).
    let windowRmse: number | null = null;
    let wallpaperRmse: number | null = null;
    let windowRect: number[] | null = null;
    if (ready.window) {
      const wx = Math.max(0, Math.round(ready.window.x * 2));
      const wy = Math.max(0, Math.round(ready.window.y * 2));
      const ww = Math.min(pw - wx, Math.round(ready.window.w * 2));
      const wh2 = Math.min(ph - wy, Math.round(ready.window.h * 2));
      if (ww > 0 && wh2 > 0) {
        windowRect = [wx, wy, ww, wh2];
        const winA = join(OUT, `${id}-win-render.png`);
        const winB = join(OUT, `${id}-win-ref.png`);
        await magick([render, "-crop", `${ww}x${wh2}+${wx}+${wy}`, "+repage", winA]);
        await magick([refPng, "-crop", `${ww}x${wh2}+${wx}+${wy}`, "+repage", winB]);
        windowRmse = await compareMetric("RMSE", winA, winB, null);
        // wallpaperMSE = (fullMSE·fullArea − winMSE·winArea) / (fullArea − winArea)
        const fullArea = pw * ph;
        const winArea = ww * wh2;
        if (fullArea > winArea) {
          const wallMse = (rmse * rmse * fullArea - windowRmse * windowRmse * winArea) / (fullArea - winArea);
          wallpaperRmse = Math.sqrt(Math.max(0, wallMse));
        }
      }
    }

    summary.push({
      id,
      status: "ok",
      referenceSize: [pw, ph],
      sceneSize: [w, h],
      windowRect,
      metrics: {
        rmse,
        ssimDissimilarity,
        lowPassRmse,
        windowRmse,
        wallpaperRmse,
      },
      fonts: ready.fonts,
      render: relative(PROJECT_ROOT, render),
      diff: relative(PROJECT_ROOT, diff),
    });
    console.error(
      `${id}  rmse=${rmse.toFixed(4)} dssim=${ssimDissimilarity.toFixed(4)} lowpass=${lowPassRmse.toFixed(4)} window=${windowRmse?.toFixed(4) ?? "-"} wallpaper=${wallpaperRmse?.toFixed(4) ?? "-"}`,
    );
  } catch (err) {
    summary.push({ id, status: "error", error: String(err) });
    console.error(`${id}  ERROR ${err}`);
  }
}
await browser(["close"]).catch(() => {});

const out = {
  generatedBy: "design/showcase/verify.ts v2 (exact native canvas)",
  environment: {
    dpr: 2,
    userAgent,
    magick: magickVersion,
    platform: process.platform,
  },
  shots: summary,
};
writeFileSync(join(ROOT, "verify-summary.json"), JSON.stringify(out, null, 2));
console.log(JSON.stringify(out, null, 2));
