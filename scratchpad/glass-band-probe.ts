#!/usr/bin/env bun
import { mkdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { Driver } from "../scripts/devtools/driver.ts";

const binary = "/Users/johnlindquist/dev/script-kit-gpui/target-agent/artifacts/glass-scroll-bands/script-kit-gpui";
const outputDir = `/tmp/glass-band-probe-${process.pid}-${Date.now().toString(36)}`;
mkdirSync(outputDir, { recursive: true });

async function screenCapture(driver: Driver, label: string) {
  const windows = await driver.listAutomationWindows({ timeoutMs: 10_000 });
  const main = (windows.windows ?? []).find((window: any) => window.id === "main");
  if (!main?.bounds) throw new Error("main automation window bounds unavailable");
  const { x, y, width, height } = main.bounds;
  const path = join(outputDir, `${label}.png`);
  const proc = Bun.spawn([
    "screencapture",
    "-x",
    `-R${Math.round(x)},${Math.round(y)},${Math.round(width)},${Math.round(height)}`,
    path,
  ], { stdout: "pipe", stderr: "pipe" });
  const exitCode = await proc.exited;
  const error = await new Response(proc.stderr).text();
  return {
    path,
    exitCode,
    error: error.trim() || null,
    bytes: exitCode === 0 ? statSync(path).size : 0,
    bounds: main.bounds,
  };
}

const driver = await Driver.launch({
  binary,
  sessionName: "glass-band-probe",
  sandboxHome: true,
  sharedModels: false,
  readyTimeoutMs: 30_000,
  defaultTimeoutMs: 15_000,
});

const receipt: Record<string, any> = {
  schemaVersion: 1,
  probe: "glass-scroll-bands",
  binary,
  outputDir,
  sessionDir: driver.sessionDir,
};

try {
  driver.send({ type: "show" });
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
  await driver.waitForSettle({ timeoutMs: 10_000 });

  const initial = await driver.getState({ timeoutMs: 10_000 });
  receipt.initial = {
    promptType: initial.promptType ?? null,
    visibleChoiceCount: initial.visibleChoiceCount ?? null,
    selectedIndex: initial.selectedIndex ?? null,
    activeFooter: initial.activeFooter ?? null,
    scroll: initial.mainListScroll ?? null,
    screenshot: await screenCapture(driver, "initial"),
  };

  for (let index = 0; index < 7; index += 1) driver.simulateKey("down");
  await driver.waitForSettle({ timeoutMs: 5_000 });
  const middle = await driver.getState({ timeoutMs: 10_000 });
  receipt.middle = {
    selectedIndex: middle.selectedIndex ?? null,
    scroll: middle.mainListScroll ?? null,
    screenshot: await screenCapture(driver, "middle-scroll"),
  };

  const count = Math.max(1, Number(middle.visibleChoiceCount ?? initial.visibleChoiceCount ?? 1));
  for (let index = 0; index < count + 12; index += 1) driver.simulateKey("down");
  await driver.waitForSettle({ timeoutMs: 10_000 });
  const end = await driver.getState({ timeoutMs: 10_000 });
  receipt.end = {
    selectedIndex: end.selectedIndex ?? null,
    visibleChoiceCount: end.visibleChoiceCount ?? null,
    scroll: end.mainListScroll ?? null,
    lastRowScrollsClear:
      end.mainListScroll?.selectedRowAboveFooter === true &&
      end.mainListScroll?.selectedRowWithinSafeViewport === true,
    screenshot: await screenCapture(driver, "last-row-clear"),
  };

  const logs = await driver.getLogs({ limit: 500 }, { timeoutMs: 10_000 });
  const serializedLogs = JSON.stringify(logs);
  receipt.installLogs = {
    glassFooterBandInstalled: serializedLogs.includes("glass_footer_band_installed"),
    glassHeaderStripInstalled: serializedLogs.includes("glass_header_strip_installed"),
  };
} finally {
  await driver.close();
  const diskLog = readFileSync(driver.logPath, "utf8");
  receipt.diskInstallLogs = {
    glassFooterBandInstalled: diskLog.includes("glass_footer_band_installed"),
    glassHeaderStripInstalled: diskLog.includes("glass_header_strip_installed"),
    matchingLines: diskLog
      .split("\n")
      .filter((line) => line.includes("glass_footer_band_installed") || line.includes("glass_header_strip_installed")),
  };
}

console.log(JSON.stringify(receipt, null, 2));
