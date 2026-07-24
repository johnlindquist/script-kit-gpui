import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export function classifyInterference(receipt: any) {
  const errors: string[] = [];
  if (!receipt || receipt.status !== "ok") errors.push("interference receipt missing");
  if (Number(receipt?.untaggedInputCount ?? -1) > 0) {
    errors.push("untagged keyboard or pointer input observed");
  }
  if (receipt?.frontmostAppChanged === true) errors.push("frontmost application changed");
  if (Number(receipt?.pointerDeviationPx ?? Infinity) > 1) {
    errors.push("pointer deviated by more than one pixel");
  }
  if (receipt?.targetMovedExternally === true) errors.push("target moved externally");
  return {
    errors,
    pass: errors.length === 0,
    disposition: errors.length === 0 ? "EVALUABLE_PASS" : "INVALID_INTERFERENCE",
  };
}

export function startInterferenceMonitor(helper: string, directory: string) {
  mkdirSync(directory, { recursive: true });
  const readyPath = join(directory, "interference-ready.json");
  const stopPath = join(directory, "interference-stop");
  const receiptPath = join(directory, "interference.json");
  rmSync(readyPath, { force: true });
  rmSync(stopPath, { force: true });
  rmSync(receiptPath, { force: true });
  const process = Bun.spawn([
    helper,
    "--ready",
    readyPath,
    "--stop",
    stopPath,
    "--out",
    receiptPath,
  ], { stdout: "pipe", stderr: "pipe" });
  return { process, readyPath, stopPath, receiptPath };
}

export async function waitForInterferenceReady(
  monitor: ReturnType<typeof startInterferenceMonitor>,
  timeoutMs = 3_000,
) {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    if (existsSync(monitor.readyPath)) {
      return JSON.parse(readFileSync(monitor.readyPath, "utf8"));
    }
    await Bun.sleep(10);
  }
  throw new Error("interference monitor did not become ready");
}

export async function finishInterferenceMonitor(
  monitor: ReturnType<typeof startInterferenceMonitor>,
) {
  writeFileSync(monitor.stopPath, "stop\n");
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(monitor.process.stdout).text(),
    new Response(monitor.process.stderr).text(),
    monitor.process.exited,
  ]);
  const receipt = existsSync(monitor.receiptPath)
    ? JSON.parse(readFileSync(monitor.receiptPath, "utf8"))
    : null;
  const classification = classifyInterference(receipt);
  return {
    exitCode,
    stdout: stdout.trim().slice(-1_000),
    stderr: stderr.trim().slice(-1_000),
    receiptPath: monitor.receiptPath,
    receipt,
    ...classification,
    pass: exitCode === 0 && classification.pass,
  };
}
