#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { announceTestStatus } from "./test-status.ts";

type Disposition =
  | "EVALUABLE_PASS"
  | "EVALUABLE_FAIL"
  | "INVALID_INTERFERENCE"
  | "INVALID_OBSERVER"
  | "INVALID_SETUP"
  | "BLOCKED_ENVIRONMENT";

type CommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

const run = async (
  command: string[],
  env?: Record<string, string>,
): Promise<CommandResult> => {
  const process = Bun.spawn(command, {
    cwd: resolve(import.meta.dir, "../.."),
    env: { ...globalThis.process.env, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [exitCode, stdout, stderr] = await Promise.all([
    process.exited,
    new Response(process.stdout).text(),
    new Response(process.stderr).text(),
  ]);
  return { exitCode, stdout, stderr };
};

const sha256 = (path: string) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

const value = (name: string, fallback?: string) => {
  const args = process.argv.slice(2);
  const index = args.indexOf(name);
  return index >= 0 && args[index + 1] ? args[index + 1] : fallback;
};

const classify = (result: CommandResult, receipt: any): Disposition => {
  const serialized = JSON.stringify(receipt ?? {});
  if (/untaggedInputCount[^0-9]*[1-9]|USER_OR_ENVIRONMENT/.test(serialized)) {
    return "INVALID_INTERFERENCE";
  }
  if (/observer|capture drop|timeout/i.test(serialized) && result.exitCode !== 0) {
    return "INVALID_OBSERVER";
  }
  if (result.exitCode === 0 && receipt?.pass === true) return "EVALUABLE_PASS";
  if (receipt && result.exitCode !== 0) return "EVALUABLE_FAIL";
  return "INVALID_SETUP";
};

async function main() {
  const requestedBinary = value("--binary") ?? process.env.SCRIPT_KIT_GPUI_BINARY;
  if (!requestedBinary) {
    throw new Error("binary missing: <unset>");
  }
  const binary = resolve(requestedBinary);
  const outputDirectory = resolve(
    value("--out", ".artifacts/glass-motion-contrast/run")!,
  );
  const mode = value("--mode", "red")!;
  if (!existsSync(binary)) {
    throw new Error(`binary missing: ${binary}`);
  }
  mkdirSync(outputDirectory, { recursive: true });
  const startedAt = new Date().toISOString();
  await announceTestStatus(
    `Glass motion ${mode}`,
    "Exact-window material, gutter, lane, and lifecycle proof",
  );

  const mainOutput = join(outputDirectory, "main-window");
  const mainResult = await run([
    "bun",
    resolve(import.meta.dir, "main-window-native-drag.ts"),
    "--binary",
    binary,
    "--out",
    mainOutput,
    "--visual-matrix",
  ], {
    SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
  });
  const mainReceiptPath = join(mainOutput, "receipt.json");
  const mainReceipt = existsSync(mainReceiptPath)
    ? JSON.parse(readFileSync(mainReceiptPath, "utf8"))
    : null;
  const disposition = classify(mainResult, mainReceipt);
  const receipt = {
    schemaVersion: 1,
    startedAt,
    finishedAt: new Date().toISOString(),
    mode,
    gitCommit: (await run(["git", "rev-parse", "HEAD"])).stdout.trim(),
    binary,
    binarySha256: sha256(binary),
    mainWindow: {
      command: [
        "bun",
        "scripts/devtools/main-window-native-drag.ts",
        "--binary",
        binary,
        "--out",
        mainOutput,
        "--visual-matrix",
      ],
      exitCode: mainResult.exitCode,
      receiptPath: mainReceiptPath,
      pinnedMainWindowNumber: mainReceipt?.pinnedMainWindowNumber ?? null,
      pid: mainReceipt?.pid ?? null,
      display: mainReceipt?.display ?? mainReceipt?.trials?.[0]?.trace?.display ?? null,
      interference: mainReceipt?.interferenceClassification
        ?? mainReceipt?.trials?.map((trial: any) => trial.analysis?.interferenceClassification)
        ?? null,
      pass: mainReceipt?.pass === true,
      disposition,
      stderr: mainResult.stderr.slice(-4_000),
    },
    scenarios: {
      stationaryBackgrounds: ["dark-terminal", "light-document", "material-matched"],
      motionBackground: "saturated-stripes",
      widthsPt: [750, 560, 480, 400, 320, 280],
      lifecycle: ["main-exit", "notes-entry", "notes-close-before-settle-reopen"],
    },
    pass: disposition === "EVALUABLE_PASS",
    disposition,
  };
  const receiptPath = join(outputDirectory, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify(receipt, null, 2));
  process.exitCode = receipt.pass ? 0 : disposition.startsWith("INVALID_") ? 2 : 1;
}

await main();
