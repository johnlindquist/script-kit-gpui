#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { readFileSync, renameSync, rmSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";

import { BROWSER_COLLECTOR_SOURCE } from "./design-fidelity";
import { assertNoninteractiveVisualProbe } from "./lib/operator-safety.ts";

function value(args: string[], flag: string): string {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] ?? "" : "";
}

function usage(): string {
  return [
    "Usage: bun scripts/devtools/capture-dom-fidelity.ts",
    "  --session <agent-browser-session>",
    "  --out <receipt.json>",
    "  [--url <mockup-url>]",
    "  [--screenshot <pixel-reference.png>]",
    "  [--width <css-px> --height <css-px> --dpr <scale>]",
  ].join(" ");
}

async function runAgentBrowser(args: string[]): Promise<string> {
  const process = Bun.spawn(["agent-browser", ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(process.stdout).text(),
    new Response(process.stderr).text(),
    process.exited,
  ]);
  if (exitCode !== 0) {
    throw new Error(
      `agent-browser exited ${exitCode}: ${stderr.trim() || stdout.trim()}`,
    );
  }
  return stdout.trim();
}

function sha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function pngDimensions(path: string): { width: number; height: number } {
  const bytes = readFileSync(path);
  if (
    bytes.length < 24 ||
    bytes.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a"
  ) {
    throw new Error(`Agent-browser screenshot is not a valid PNG: ${path}`);
  }
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
}

function workspaceRelative(path: string): string {
  const candidate = relative(process.cwd(), path);
  return candidate.startsWith("..") ? path : candidate;
}

async function collectDomReceipt(session: string): Promise<Record<string, any>> {
  const raw = await runAgentBrowser([
    "--session",
    session,
    "eval",
    BROWSER_COLLECTOR_SOURCE,
  ]);
  const receipt = JSON.parse(raw);
  if (!receipt.screenId || !Array.isArray(receipt.elements)) {
    throw new Error("Browser collector returned an incomplete fidelity receipt");
  }
  return receipt;
}

async function main(): Promise<void> {
  const args = Bun.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    console.log(usage());
    return;
  }
  const session = value(args, "--session");
  const out = value(args, "--out");
  const url = value(args, "--url");
  const screenshot = value(args, "--screenshot");
  const widthRaw = value(args, "--width");
  const heightRaw = value(args, "--height");
  const dprRaw = value(args, "--dpr");
  const width = Number(widthRaw);
  const height = Number(heightRaw);
  const dpr = Number(dprRaw);
  if (!session || !out) {
    console.error(usage());
    process.exit(2);
  }
  assertNoninteractiveVisualProbe("browser-dom-fidelity.capture");

  const viewportRequested = Boolean(widthRaw || heightRaw || dprRaw);
  if (viewportRequested) {
    if (!(width > 0) || !(height > 0) || !(dpr > 0)) {
      throw new Error("--width, --height, and --dpr must be positive and supplied together");
    }
    await runAgentBrowser([
      "--session",
      session,
      "set",
      "viewport",
      String(width),
      String(height),
      String(dpr),
    ]);
  }

  if (url) {
    await runAgentBrowser(["--session", session, "open", url]);
  } else {
    await runAgentBrowser(["--session", session, "reload"]);
  }

  let receipt = await collectDomReceipt(session);
  if (screenshot) {
    const screenshotPath = resolve(screenshot);
    await mkdir(dirname(screenshotPath), { recursive: true });
    let captured = false;
    let lastReasons: string[] = [];
    for (let attempt = 1; attempt <= 4; attempt++) {
      const suffix = `${process.pid}-${Date.now()}-${attempt}`;
      const captureA = `${screenshotPath}.capture-a-${suffix}.png`;
      const captureB = `${screenshotPath}.capture-b-${suffix}.png`;
      try {
        const dom0 = await collectDomReceipt(session);
        await runAgentBrowser(["--session", session, "screenshot", "body", captureA]);
        const dom1 = await collectDomReceipt(session);
        await runAgentBrowser(["--session", session, "screenshot", "body", captureB]);
        const dom2 = await collectDomReceipt(session);
        const hashA = sha256(captureA);
        const hashB = sha256(captureB);
        const dimensionsA = pngDimensions(captureA);
        const dimensionsB = pngDimensions(captureB);
        const domStable = JSON.stringify(dom0) === JSON.stringify(dom1) &&
          JSON.stringify(dom1) === JSON.stringify(dom2);
        const capturesIdentical = hashA === hashB;
        const expectedWidth = dom2.windowRect.width * dom2.devicePixelRatio;
        const expectedHeight = dom2.windowRect.height * dom2.devicePixelRatio;
        const dimensionsValid =
          dimensionsA.width === expectedWidth &&
          dimensionsA.height === expectedHeight &&
          dimensionsB.width === expectedWidth &&
          dimensionsB.height === expectedHeight;
        lastReasons = [
          domStable ? "" : "DOM geometry/style changed inside capture bracket",
          capturesIdentical ? "" : "consecutive browser captures were not byte-identical",
          dimensionsValid ? "" : "browser capture dimensions did not match DOM viewport and DPR",
        ].filter(Boolean);
        if (lastReasons.length === 0) {
          renameSync(captureB, screenshotPath);
          receipt = {
            ...dom2,
            captureBracket: {
              valid: true,
              attempt,
              domStable,
              capturesIdentical,
              bracket: "DOM0-shotA-DOM1-shotB-DOM2",
            },
            screenshotEvidence: {
              path: workspaceRelative(screenshotPath),
              sha256: hashB,
              duplicateCaptureSha256: hashA,
              capturesIdentical,
              width: dimensionsB.width,
              height: dimensionsB.height,
              devicePixelRatio: dom2.devicePixelRatio,
              source: "agent-browser element screenshot",
            },
          };
          captured = true;
          break;
        }
      } finally {
        rmSync(captureA, { force: true });
        rmSync(captureB, { force: true });
      }
    }
    if (!captured) {
      throw new Error(`Could not capture a stable DOM screenshot: ${lastReasons.join("; ")}`);
    }
  }

  const outputPath = resolve(out);
  await mkdir(dirname(outputPath), { recursive: true });
  await Bun.write(outputPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(
    JSON.stringify({
      classification: "ok",
      screenId: receipt.screenId,
      elementCount: receipt.elements.length,
      outputPath,
      screenshot: receipt.screenshotEvidence ?? null,
    }),
  );
}

await main();
