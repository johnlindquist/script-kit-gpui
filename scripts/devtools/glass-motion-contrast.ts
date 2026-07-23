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

const has = (name: string) => process.argv.slice(2).includes(name);

async function waitForFile(path: string, timeoutMs = 5_000) {
  const started = performance.now();
  while (performance.now() - started < timeoutMs) {
    if (existsSync(path)) return;
    await Bun.sleep(25);
  }
  throw new Error(`timed out waiting for ${path}`);
}

async function runLockedTreatmentCell(options: {
  binary: string;
  helper: string;
  outputDirectory: string;
  fixture: string;
}) {
  const slug = `${options.fixture}-locked-T55-R`;
  const cellDirectory = join(options.outputDirectory, slug);
  mkdirSync(cellDirectory, { recursive: true });
  const fixtureReceiptPath = join(cellDirectory, "fixture.json");
  await announceTestStatus(
    "Glass production T55/R",
    `${options.fixture} · exact-window capture and capsule contrast metrics`,
  );
  const fixture = Bun.spawn([
    options.helper,
    "--mode",
    options.fixture,
    "--receipt",
    fixtureReceiptPath,
  ], {
    cwd: resolve(import.meta.dir, "../.."),
    stdout: "pipe",
    stderr: "pipe",
  });
  let mainResult: CommandResult | null = null;
  try {
    await waitForFile(fixtureReceiptPath);
    const mainDirectory = join(cellDirectory, "main-window");
    const motionRequired = options.fixture === "saturated-stripes";
    const mainCommand = [
      "bun",
      resolve(import.meta.dir, "main-window-native-drag.ts"),
      "--binary",
      options.binary,
      "--out",
      mainDirectory,
      ...(motionRequired
        ? ["--trials", "fast-horizontal"]
        : ["--stationary-only"]),
      "--widths",
      "none",
    ];
    mainResult = await run(mainCommand, {
      SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
    });
    const mainReceiptPath = join(mainDirectory, "receipt.json");
    const metricsPath = join(cellDirectory, "metrics.json");
    const metricsResult = existsSync(mainReceiptPath)
      ? await run([
        "python3",
        resolve(import.meta.dir, "../agentic/glass-contrast-metrics.py"),
        "--receipt",
        mainReceiptPath,
        "--out",
        metricsPath,
      ])
      : { exitCode: 1, stdout: "", stderr: "main receipt missing" };
    const mainReceipt = existsSync(mainReceiptPath)
      ? JSON.parse(readFileSync(mainReceiptPath, "utf8"))
      : null;
    const metrics = existsSync(metricsPath)
      ? JSON.parse(readFileSync(metricsPath, "utf8"))
      : null;
    const motionMetricsPath = join(cellDirectory, "motion-metrics.json");
    const motionMetricsResult = motionRequired && existsSync(mainReceiptPath)
      ? await run([
        "python3",
        resolve(import.meta.dir, "../agentic/glass-motion-color-metrics.py"),
        "--receipt",
        mainReceiptPath,
        "--trajectory",
        "fast-horizontal",
        "--out",
        motionMetricsPath,
      ])
      : { exitCode: motionRequired ? 1 : 0, stdout: "", stderr: "" };
    const motionMetrics = existsSync(motionMetricsPath)
      ? JSON.parse(readFileSync(motionMetricsPath, "utf8"))
      : null;
    const structuralPass = mainReceipt?.pass === true;
    const materialRelationPass = metrics != null
      && metrics.summary?.maximumStageDeltaE00
        <= (motionRequired ? 25 : 10)
      && metrics.summary?.maximumStageAbsoluteLStarDifference <= 12;
    const boundaryPass = metrics != null
      && metrics.summary?.minimumMedianBoundaryLuminanceDifference >= 0.040
      && metrics.summary?.minimumP10BoundaryLuminanceDifference >= 0.015
      && metrics.summary?.minimumFractionAtLeast015 >= 0.80;
    const motionColorPass = !motionRequired
      || (motionMetricsResult.exitCode === 0
        && motionMetrics?.motionFrameCount === 15
        && motionMetrics?.settledFrameCount === 3
        && motionMetrics?.summary?.maximumStageDeltaE00 <= 25
        && motionMetrics?.summary?.maximumStageAbsoluteLStarDifference <= 18
        && motionMetrics?.summary?.motionRelationRangeDeltaE00 <= 10);
    return {
      slug,
      fixture: options.fixture,
      tintFloor: "T55",
      effectiveTintFloor: 0.55,
      separation: "R",
      fixtureReceipt: JSON.parse(readFileSync(fixtureReceiptPath, "utf8")),
      mainReceiptPath,
      metricsPath,
      motionRequired,
      motionMetricsPath: motionRequired ? motionMetricsPath : null,
      mainExitCode: mainResult.exitCode,
      metricsExitCode: metricsResult.exitCode,
      motionMetricsExitCode: motionMetricsResult.exitCode,
      metrics,
      motionMetrics,
      structuralPass,
      materialRelationPass,
      boundaryPass,
      motionColorPass,
      pass: structuralPass
        && materialRelationPass
        && boundaryPass
        && motionColorPass,
      stderr:
        `${mainResult.stderr}\n${metricsResult.stderr}\n${motionMetricsResult.stderr}`
          .trim()
          .slice(-4_000),
    };
  } finally {
    fixture.kill();
    await fixture.exited;
  }
}

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
  const requestedOutput = resolve(
    value("--out", ".artifacts/glass-motion-contrast/run")!,
  );
  const explicitReceiptPath = requestedOutput.endsWith(".json") ? requestedOutput : null;
  const outputDirectory = explicitReceiptPath
    ? resolve(explicitReceiptPath, "..")
    : requestedOutput;
  const mode = has("--all") ? "all" : value("--mode", "red")!;
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
  let lockedTreatment: any = null;
  if (mode === "all" || mode === "locked") {
    const helper = join(outputDirectory, "macos-glass-background-fixture");
    const compiled = await run([
      "xcrun",
      "swiftc",
      "-O",
      resolve(import.meta.dir, "../agentic/macos-glass-background-fixture.swift"),
      "-o",
      helper,
    ]);
    if (compiled.exitCode !== 0) {
      throw new Error(`fixture compile failed: ${compiled.stderr}`);
    }
    const lockedDirectory = join(outputDirectory, "locked-treatment");
    mkdirSync(lockedDirectory, { recursive: true });
    const cells = [];
    for (const fixture of [
      "saturated-stripes",
      "dark-terminal",
      "light-document",
      "material-matched",
    ]) {
      cells.push(await runLockedTreatmentCell({
        binary,
        helper,
        outputDirectory: lockedDirectory,
        fixture,
      }));
    }
    const stability = cells.find((cell) => cell.fixture === "saturated-stripes");
    const neutral = cells.filter((cell) => cell.fixture !== "saturated-stripes");
    const stabilityPass = stability?.structuralPass === true
      && stability?.motionColorPass === true
      && stability?.motionMetrics?.motionFrameCount === 15
      && stability?.motionMetrics?.settledFrameCount === 3
      && stability?.motionMetrics?.summary?.maximumStageDeltaE00 <= 25
      && stability?.motionMetrics?.summary?.maximumStageAbsoluteLStarDifference <= 18
      && stability?.motionMetrics?.summary?.motionRelationRangeDeltaE00 <= 10;
    const neutralPass = neutral.length === 3 && neutral.every((cell) => cell.pass);
    lockedTreatment = {
      helper,
      helperSha256: sha256(helper),
      policy: {
        tintFloor: 0.55,
        veilAlpha: 0.80,
        separation: "R",
        rimWidthPt: 1.0,
        rimAlphaDark: 0.24,
        rimAlphaLight: 0.18,
        shadow: "none",
      },
      cells,
      stabilityPass,
      neutralPass,
      complete: cells.length === 4
        && cells.every((cell) => typeof cell.pass === "boolean"),
      pass: stabilityPass && neutralPass,
    };
    writeFileSync(
      join(lockedDirectory, "production-policy.json"),
      `${JSON.stringify(lockedTreatment.policy, null, 2)}\n`,
    );
  }

  let rapidToggle: any = null;
  let lifecycleFilmstrip: any = null;
  if (mode === "all" || mode === "green") {
    const rapidPath = join(outputDirectory, "rapid-toggle.json");
    const rapidResult = await run([
      "bun",
      resolve(import.meta.dir, "rapid-toggle-stress.ts"),
      "--binary",
      binary,
      "--out",
      rapidPath,
    ], {
      SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
    });
    rapidToggle = {
      exitCode: rapidResult.exitCode,
      receiptPath: rapidPath,
      receipt: existsSync(rapidPath)
        ? JSON.parse(readFileSync(rapidPath, "utf8"))
        : null,
      stderr: rapidResult.stderr.slice(-4_000),
    };
    const lifecycleDirectory = join(outputDirectory, "lifecycle-filmstrips");
    const lifecycleResult = await run([
      "bun",
      resolve(import.meta.dir, "glass-lifecycle-filmstrip.ts"),
      "--binary",
      binary,
      "--out",
      lifecycleDirectory,
    ], {
      SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
    });
    const lifecycleReceiptPath = join(lifecycleDirectory, "receipt.json");
    lifecycleFilmstrip = {
      exitCode: lifecycleResult.exitCode,
      receiptPath: lifecycleReceiptPath,
      receipt: existsSync(lifecycleReceiptPath)
        ? JSON.parse(readFileSync(lifecycleReceiptPath, "utf8"))
        : null,
      stderr: lifecycleResult.stderr.slice(-4_000),
    };
  }
  const evidenceComplete = mode === "all"
    ? mainReceipt?.visualMatrix?.states?.length === 4
      && mainReceipt?.widthMatrix?.rows?.length === 6
      && lockedTreatment?.cells?.length === 4
      && lockedTreatment?.cells?.find(
          (cell: any) => cell.fixture === "saturated-stripes",
        )?.motionMetrics?.frames?.length === 18
      && lifecycleFilmstrip?.receipt?.scenarios?.length === 4
      && Object.keys(rapidToggle?.receipt?.phases ?? {}).length === 3
    : mode === "green"
    ? lifecycleFilmstrip?.receipt?.scenarios?.length === 4
      && Object.keys(rapidToggle?.receipt?.phases ?? {}).length === 3
    : mode === "locked"
    ? lockedTreatment?.cells?.length === 4
      && lockedTreatment?.cells?.find(
          (cell: any) => cell.fixture === "saturated-stripes",
        )?.motionMetrics?.frames?.length === 18
    : true;
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
    scenarioContract: {
      stationaryBackgrounds: ["dark-terminal", "light-document", "material-matched"],
      motionBackground: "saturated-stripes",
      widthsPt: [750, 560, 480, 400, 320, 280],
      lifecycle: ["main-exit", "notes-entry", "notes-close-before-settle-reopen"],
    },
    executedScenarios: {
      mainVisualMatrixRows: mainReceipt?.visualMatrix?.states ?? [],
      mainWidthRows: mainReceipt?.widthMatrix?.rows ?? [],
      saturatedMotionFrames:
        lockedTreatment?.cells?.find(
          (cell: any) => cell.fixture === "saturated-stripes",
        )?.motionMetrics?.frames ?? [],
      lifecycle: lifecycleFilmstrip?.receipt?.scenarios ?? [],
      rapidTogglePhases: rapidToggle?.receipt?.phases ?? {},
    },
    lockedTreatment,
    rapidToggle,
    lifecycleFilmstrip,
    evidenceComplete,
    pass: disposition === "EVALUABLE_PASS"
      && evidenceComplete
      && (lockedTreatment == null || lockedTreatment.pass === true)
      && (rapidToggle == null || rapidToggle.receipt?.pass === true)
      && (lifecycleFilmstrip == null
        || lifecycleFilmstrip.receipt?.pass === true),
    disposition,
  };
  const receiptPath = explicitReceiptPath ?? join(outputDirectory, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify(receipt, null, 2));
  process.exitCode = receipt.pass ? 0 : disposition.startsWith("INVALID_") ? 2 : 1;
}

await main();
