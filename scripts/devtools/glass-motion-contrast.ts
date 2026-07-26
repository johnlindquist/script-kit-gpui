#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import {
  aggregateDisposition,
  assertFreshOutputDirectory,
  compositeEvaluator,
  newRunId,
  sha256File,
  validateArtifactReference,
  validateChildReceipt,
  validateUniqueScenarioSet,
  type EvidenceIdentity,
} from "./glass-evidence-contract.ts";
import { LEGACY_FULL_SCENARIO_ORDER } from "./glass-lifecycle-filmstrip-contract.ts";
import { announceTestStatus } from "./test-status.ts";
import { requireValidatedHelper } from "./glass-native-helper-cache.ts";

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
  identity: EvidenceIdentity;
  policyId: string;
  importLifecycleReceipt?: string;
}) {
  const slug = `${options.fixture}-locked-${options.policyId}`;
  const cellDirectory = join(options.outputDirectory, slug);
  mkdirSync(cellDirectory, { recursive: true });
  const fixtureReceiptPath = join(cellDirectory, "fixture.json");
  await announceTestStatus(
    `Glass production ${options.policyId}`,
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
  const binarySha256Before = sha256File(options.binary);
  try {
    await waitForFile(fixtureReceiptPath);
    const fixtureReceipt = JSON.parse(readFileSync(fixtureReceiptPath, "utf8"));
    const fixtureIdentityPass = fixtureReceipt?.schemaVersion === 2
      && fixtureReceipt?.ignoresMouseEvents === true
      && fixtureReceipt?.windowLevel === 100
      && fixtureReceipt?.orderingContract === "one-level-below-popup-owner"
      && /^[a-f0-9]{64}$/.test(
        String(fixtureReceipt?.configurationSha256 ?? ""),
      );
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
      SCRIPT_KIT_GLASS_RUN_ID: options.identity.runId,
      SCRIPT_KIT_GLASS_GIT_COMMIT: options.identity.gitCommit,
      SCRIPT_KIT_GLASS_BINARY: options.identity.binary,
      SCRIPT_KIT_GLASS_BINARY_SHA256: options.identity.binarySha256,
      SCRIPT_KIT_GLASS_SCENARIO: `locked:${options.fixture}`,
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
    if (metrics) {
      Object.assign(metrics, {
        runId: options.identity.runId,
        gitCommit: options.identity.gitCommit,
        binarySha256: options.identity.binarySha256,
        scenario: `locked:${options.fixture}:stationary-metrics`,
      });
      writeFileSync(metricsPath, `${JSON.stringify(metrics, null, 2)}\n`);
    }
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
    if (motionMetrics) {
      Object.assign(motionMetrics, {
        runId: options.identity.runId,
        gitCommit: options.identity.gitCommit,
        binarySha256: options.identity.binarySha256,
        scenario: `locked:${options.fixture}:motion-metrics`,
      });
      writeFileSync(motionMetricsPath, `${JSON.stringify(motionMetrics, null, 2)}\n`);
    }
    let saturatedLifecycle: any = null;
    if (motionRequired && options.importLifecycleReceipt) {
      // WP9: reuse already-captured lifecycle evidence instead of
      // duplicating the capture — accepted ONLY when every identity and
      // integrity axis matches. A non-matching import is a hard failure,
      // never a silent fallback to a fresh capture (a fallback would let a
      // stale import mask a real mismatch).
      const importPath = resolve(options.importLifecycleReceipt);
      const importedReceipt = existsSync(importPath)
        ? JSON.parse(readFileSync(importPath, "utf8"))
        : null;
      const importErrors = validateArtifactReference(
        importedReceipt,
        {
          binarySha256: options.identity.binarySha256,
          requiredScenarioNames: [...LEGACY_FULL_SCENARIO_ORDER],
        },
        {
          hashFile: (path) => {
            try {
              return sha256File(path);
            } catch {
              return null;
            }
          },
        },
      );
      const importUsable = importErrors.length === 0;
      const entryMotionMetricsPath = join(
        cellDirectory,
        "main-entry-motion-metrics.json",
      );
      const entryMotionMetricsResult = importUsable
        ? await run([
          "python3",
          resolve(import.meta.dir, "../agentic/glass-motion-color-metrics.py"),
          "--receipt",
          mainReceiptPath,
          "--lifecycle-receipt",
          importPath,
          "--scenario",
          "main-entry",
          "--out",
          entryMotionMetricsPath,
        ])
        : {
          exitCode: 1,
          stdout: "",
          stderr: `imported lifecycle receipt rejected: ${importErrors.join("; ")}`,
        };
      saturatedLifecycle = {
        exitCode: importUsable ? 0 : 1,
        receiptPath: importPath,
        receipt: importedReceipt,
        importedFrom: importPath,
        importedReceiptSha256: existsSync(importPath)
          ? sha256File(importPath)
          : null,
        importErrors,
        entryMotionMetricsPath,
        entryMotionMetricsExitCode: entryMotionMetricsResult.exitCode,
        entryMotionMetrics: existsSync(entryMotionMetricsPath)
          ? JSON.parse(readFileSync(entryMotionMetricsPath, "utf8"))
          : null,
        stderr: entryMotionMetricsResult.stderr.trim().slice(-4_000),
        binarySha256Before: options.identity.binarySha256,
        binarySha256After: sha256File(options.binary),
      };
      if (saturatedLifecycle.entryMotionMetrics) {
        Object.assign(saturatedLifecycle.entryMotionMetrics, {
          runId: options.identity.runId,
          gitCommit: options.identity.gitCommit,
          binarySha256: options.identity.binarySha256,
          scenario: "lifecycle-saturated:main-entry-motion-metrics",
        });
        writeFileSync(
          entryMotionMetricsPath,
          `${JSON.stringify(saturatedLifecycle.entryMotionMetrics, null, 2)}\n`,
        );
      }
    } else if (motionRequired) {
      const lifecycleBinarySha256Before = sha256File(options.binary);
      const lifecycleDirectory = join(cellDirectory, "saturated-lifecycle");
      const lifecycleResult = await run([
        "bun",
        resolve(import.meta.dir, "glass-lifecycle-filmstrip.ts"),
        "--binary",
        options.binary,
        "--out",
        lifecycleDirectory,
      ], {
        SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
        SCRIPT_KIT_GLASS_RUN_ID: options.identity.runId,
        SCRIPT_KIT_GLASS_GIT_COMMIT: options.identity.gitCommit,
        SCRIPT_KIT_GLASS_BINARY: options.identity.binary,
        SCRIPT_KIT_GLASS_BINARY_SHA256: options.identity.binarySha256,
        SCRIPT_KIT_GLASS_SCENARIO: "lifecycle-saturated",
      });
      const lifecycleReceiptPath = join(lifecycleDirectory, "receipt.json");
      const entryMotionMetricsPath = join(
        lifecycleDirectory,
        "main-entry-motion-metrics.json",
      );
      const entryMotionMetricsResult = existsSync(lifecycleReceiptPath)
        ? await run([
          "python3",
          resolve(import.meta.dir, "../agentic/glass-motion-color-metrics.py"),
          "--receipt",
          mainReceiptPath,
          "--lifecycle-receipt",
          lifecycleReceiptPath,
          "--scenario",
          "main-entry",
          "--out",
          entryMotionMetricsPath,
        ])
        : { exitCode: 1, stdout: "", stderr: "lifecycle receipt missing" };
      saturatedLifecycle = {
        exitCode: lifecycleResult.exitCode,
        receiptPath: lifecycleReceiptPath,
        receipt: existsSync(lifecycleReceiptPath)
          ? JSON.parse(readFileSync(lifecycleReceiptPath, "utf8"))
          : null,
        entryMotionMetricsPath,
        entryMotionMetricsExitCode: entryMotionMetricsResult.exitCode,
        entryMotionMetrics: existsSync(entryMotionMetricsPath)
          ? JSON.parse(readFileSync(entryMotionMetricsPath, "utf8"))
          : null,
        stderr:
          `${lifecycleResult.stderr}\n${entryMotionMetricsResult.stderr}`
            .trim()
            .slice(-4_000),
        binarySha256Before: lifecycleBinarySha256Before,
        binarySha256After: sha256File(options.binary),
      };
      if (saturatedLifecycle.entryMotionMetrics) {
        Object.assign(saturatedLifecycle.entryMotionMetrics, {
          runId: options.identity.runId,
          gitCommit: options.identity.gitCommit,
          binarySha256: options.identity.binarySha256,
          scenario: "lifecycle-saturated:main-entry-motion-metrics",
        });
        writeFileSync(
          entryMotionMetricsPath,
          `${JSON.stringify(saturatedLifecycle.entryMotionMetrics, null, 2)}\n`,
        );
      }
    }
    const binarySha256After = sha256File(options.binary);
    const childValidationErrors = validateChildReceipt(
      mainReceipt,
      options.identity,
      `locked:${options.fixture}`,
      mainResult.exitCode,
    );
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
        && motionMetrics?.motionFrameCount >= 15
        && motionMetrics?.settledFrameCount >= 3
        && motionMetrics?.summary?.boundaryPassEveryFrame === true
        && motionMetrics?.summary?.maximumNeighboringSettledMaterialDeltaE00 <= 6
        && Object.values(
          motionMetrics?.summary?.materialStabilityCapsules ?? {},
        ).every((capsule: any) => capsule?.pass === true));
    // Schema v2 (displayed-color entry metric): materialStabilityCapsules now
    // grades RAW displayed pixels on every lifecycle-visible entry frame, and
    // summary.alphaPolicy hard-fails visible sub-0.85-alpha, visible
    // zero-alpha, and unmeasurable-visible frames. The intrinsic diagnostic
    // (summary.intrinsicMaterialDiagnosticCapsules) is non-gating on purpose.
    const entryMotionColorPass = !motionRequired
      || (saturatedLifecycle?.exitCode === 0
        && saturatedLifecycle?.receipt?.pass === true
        && saturatedLifecycle?.entryMotionMetricsExitCode === 0
        && saturatedLifecycle?.entryMotionMetrics?.schemaVersion === 2
        && saturatedLifecycle?.entryMotionMetrics?.pass === true
        && saturatedLifecycle?.entryMotionMetrics?.motionFrameCount >= 5
        && saturatedLifecycle?.entryMotionMetrics?.settledFrameCount >= 3
        && saturatedLifecycle?.entryMotionMetrics?.summary
          ?.alphaPolicy?.pass === true
        && saturatedLifecycle?.entryMotionMetrics?.summary
          ?.maximumDisplayedEntryDeltaE00 <= 5
        && saturatedLifecycle?.entryMotionMetrics?.summary
          ?.boundaryPassEverySettledFrame === true
        && saturatedLifecycle?.entryMotionMetrics?.summary
          ?.maximumNeighboringSettledMaterialDeltaE00 <= 6
        && Object.values(
          saturatedLifecycle?.entryMotionMetrics?.summary
            ?.materialStabilityCapsules ?? {},
        ).every((capsule: any) => capsule?.pass === true));
    return {
      slug,
      fixture: options.fixture,
      tintFloor: "T55",
      effectiveTintFloor: 0.55,
      separation: "R",
      fixtureReceipt,
      fixtureIdentityPass,
      binarySha256Before,
      binarySha256After,
      mainReceiptPath,
      metricsPath,
      motionRequired,
      motionMetricsPath: motionRequired ? motionMetricsPath : null,
      mainExitCode: mainResult.exitCode,
      metricsExitCode: metricsResult.exitCode,
      motionMetricsExitCode: motionMetricsResult.exitCode,
      metrics,
      motionMetrics,
      saturatedLifecycle,
      structuralPass,
      materialRelationPass,
      boundaryPass,
      motionColorPass,
      entryMotionColorPass,
      childValidationErrors,
      pass: childValidationErrors.length === 0
        && fixtureIdentityPass
        && binarySha256Before === options.identity.binarySha256
        && binarySha256After === options.identity.binarySha256
        && structuralPass
        && materialRelationPass
        && boundaryPass
        && motionColorPass
        && entryMotionColorPass,
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
  if (receipt?.disposition) return receipt.disposition as Disposition;
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
  const policyTintFloor = Number(value("--policy-tint-floor", "0.35"));
  const policyVeilAlpha = Number(value("--policy-veil-alpha", "0.80"));
  const policyId = value(
    "--policy-id",
    `T${Math.round(policyTintFloor * 100)}-V${Math.round(policyVeilAlpha * 100)}-R`,
  )!;
  if (
    !Number.isFinite(policyTintFloor)
    || !Number.isFinite(policyVeilAlpha)
    || policyTintFloor < 0
    || policyTintFloor > 1
    || policyVeilAlpha < 0
    || policyVeilAlpha > 1
  ) {
    throw new Error("policy tint floor and veil alpha must be numbers in [0, 1]");
  }
  if (!existsSync(binary)) {
    throw new Error(`binary missing: ${binary}`);
  }
  assertFreshOutputDirectory(outputDirectory);
  mkdirSync(outputDirectory, { recursive: true });
  const startedAt = new Date().toISOString();
  const gitCommit = (await run(["git", "rev-parse", "HEAD"])).stdout.trim();
  const identity: EvidenceIdentity = {
    runId: newRunId(),
    gitCommit,
    binary,
    binarySha256: sha256File(binary),
  };
  const childEnvironment = {
    SCRIPT_KIT_GLASS_RUN_ID: identity.runId,
    SCRIPT_KIT_GLASS_GIT_COMMIT: identity.gitCommit,
    SCRIPT_KIT_GLASS_BINARY: identity.binary,
    SCRIPT_KIT_GLASS_BINARY_SHA256: identity.binarySha256,
  };
  await announceTestStatus(
    `Glass motion ${mode}`,
    "Exact-window material, gutter, lane, and lifecycle proof",
  );

  const mainOutput = join(outputDirectory, "main-window");
  const mainBinarySha256Before = sha256File(binary);
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
    ...childEnvironment,
    SCRIPT_KIT_GLASS_SCENARIO: "main-window",
  });
  const mainReceiptPath = join(mainOutput, "receipt.json");
  const mainReceipt = existsSync(mainReceiptPath)
    ? JSON.parse(readFileSync(mainReceiptPath, "utf8"))
    : null;
  const mainBinarySha256After = sha256File(binary);
  const setupErrors = validateChildReceipt(
    mainReceipt,
    identity,
    "main-window",
    mainResult.exitCode,
  );
  if (
    mainBinarySha256Before !== identity.binarySha256
    || mainBinarySha256After !== identity.binarySha256
  ) {
    setupErrors.push("main-window binary changed before or after child");
  }
  const disposition = classify(mainResult, mainReceipt);
  let lockedTreatment: any = null;
  if (mode === "all" || mode === "locked") {
    // WP4 (glass-smoke-harness-max-info): accept a pre-compiled
    // hash-validated fixture helper; compile per-run only when absent.
    const suppliedFixtureHelper = value("--fixture-helper");
    let helper: string;
    if (suppliedFixtureHelper) {
      helper = requireValidatedHelper(suppliedFixtureHelper, "fixture")
        .binaryPath;
    } else {
      helper = join(outputDirectory, "macos-glass-background-fixture");
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
        identity,
        policyId,
        importLifecycleReceipt: fixture === "saturated-stripes"
          ? value("--import-lifecycle-receipt")
          : undefined,
      }));
    }
    const stability = cells.find((cell) => cell.fixture === "saturated-stripes");
    const neutral = cells.filter((cell) => cell.fixture !== "saturated-stripes");
    const stabilityPass = stability?.structuralPass === true
      && stability?.motionColorPass === true
      && stability?.motionMetrics?.motionFrameCount >= 15
      && stability?.motionMetrics?.settledFrameCount >= 3
      && stability?.motionMetrics?.summary?.boundaryPassEveryFrame === true
      && stability?.motionMetrics?.summary?.maximumNeighboringSettledMaterialDeltaE00 <= 6
      && Object.values(
        stability?.motionMetrics?.summary?.materialStabilityCapsules ?? {},
      ).every((capsule: any) => capsule?.pass === true);
    const neutralPass = neutral.length === 3 && neutral.every((cell) => cell.pass);
    lockedTreatment = {
      helper,
      helperSha256: sha256(helper),
      policy: {
        id: policyId,
        tintFloor: policyTintFloor,
        veilAlpha: policyVeilAlpha,
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
      pass: stabilityPass
        && neutralPass
        && stability?.pass === true,
    };
    writeFileSync(
      join(lockedDirectory, "production-policy.json"),
      `${JSON.stringify(lockedTreatment.policy, null, 2)}\n`,
    );
  }

  let rapidToggle: any = null;
  let lifecycleFilmstrip: any = mode === "locked"
    ? lockedTreatment?.cells?.find(
      (cell: any) => cell.fixture === "saturated-stripes",
    )?.saturatedLifecycle ?? null
    : null;
  let notesFallback: any = null;
  if (mode === "all" || mode === "green") {
    const rapidPath = join(outputDirectory, "rapid-toggle.json");
    const rapidBinarySha256Before = sha256File(binary);
    const rapidResult = await run([
      "bun",
      resolve(import.meta.dir, "rapid-toggle-stress.ts"),
      "--binary",
      binary,
      "--out",
      rapidPath,
    ], {
      SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
      ...childEnvironment,
      SCRIPT_KIT_GLASS_SCENARIO: "rapid-toggle",
    });
    rapidToggle = {
      exitCode: rapidResult.exitCode,
      receiptPath: rapidPath,
      receipt: existsSync(rapidPath)
        ? JSON.parse(readFileSync(rapidPath, "utf8"))
        : null,
      stderr: rapidResult.stderr.slice(-4_000),
      binarySha256Before: rapidBinarySha256Before,
      binarySha256After: sha256File(binary),
    };
    const notesFallbackPath = join(outputDirectory, "notes-fallback.json");
    const notesFallbackBinarySha256Before = sha256File(binary);
    const notesFallbackResult = await run([
      "bun",
      resolve(import.meta.dir, "notes-glass-entry-fallback.ts"),
      "--binary",
      binary,
      "--out",
      notesFallbackPath,
    ], {
      SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
      ...childEnvironment,
      SCRIPT_KIT_GLASS_SCENARIO: "notes-fallback",
    });
    notesFallback = {
      exitCode: notesFallbackResult.exitCode,
      receiptPath: notesFallbackPath,
      receipt: existsSync(notesFallbackPath)
        ? JSON.parse(readFileSync(notesFallbackPath, "utf8"))
        : null,
      stderr: notesFallbackResult.stderr.slice(-4_000),
      binarySha256Before: notesFallbackBinarySha256Before,
      binarySha256After: sha256File(binary),
    };
    lifecycleFilmstrip = mode === "all"
      ? lockedTreatment?.cells?.find(
        (cell: any) => cell.fixture === "saturated-stripes",
      )?.saturatedLifecycle ?? null
      : null;
    if (lifecycleFilmstrip == null) {
      const lifecycleDirectory = join(outputDirectory, "lifecycle-filmstrips");
      const lifecycleBinarySha256Before = sha256File(binary);
      const lifecycleResult = await run([
        "bun",
        resolve(import.meta.dir, "glass-lifecycle-filmstrip.ts"),
        "--binary",
        binary,
        "--out",
        lifecycleDirectory,
      ], {
        SCRIPT_KIT_TEST_STATUS: process.env.SCRIPT_KIT_TEST_STATUS ?? "1",
        ...childEnvironment,
        SCRIPT_KIT_GLASS_SCENARIO: "lifecycle",
      });
      const lifecycleReceiptPath = join(lifecycleDirectory, "receipt.json");
      lifecycleFilmstrip = {
        exitCode: lifecycleResult.exitCode,
        receiptPath: lifecycleReceiptPath,
        receipt: existsSync(lifecycleReceiptPath)
          ? JSON.parse(readFileSync(lifecycleReceiptPath, "utf8"))
          : null,
        stderr: lifecycleResult.stderr.slice(-4_000),
        binarySha256Before: lifecycleBinarySha256Before,
        binarySha256After: sha256File(binary),
      };
    }
  }
  const exactSet = (observed: string[], required: string[]) =>
    validateUniqueScenarioSet(observed, required).length === 0;
  const mainVisualMatrixComplete = exactSet(
    (mainReceipt?.visualMatrix?.states ?? []).map((state: any) => state?.name),
    [
      "matrix-full-expanded-2x",
      "matrix-disabled-confirm-2x",
      "matrix-bright-light-2x",
      "matrix-dark-plain-2x",
    ],
  );
  const mainWidthMatrixComplete = exactSet(
    (mainReceipt?.widthMatrix?.rows ?? []).map(
      (row: any) => String(row?.requestedWidth),
    ),
    ["750", "560", "480", "400", "320", "280"],
  );
  const lockedCellsComplete = exactSet(
    (lockedTreatment?.cells ?? []).map((cell: any) => cell?.fixture),
    [
      "saturated-stripes",
      "dark-terminal",
      "light-document",
      "material-matched",
    ],
  );
  const lifecycleComplete = exactSet(
    (lifecycleFilmstrip?.receipt?.scenarios ?? []).map(
      (scenario: any) => scenario?.name,
    ),
    [
      "main-exit",
      "main-entry",
      "notes-entry",
      "notes-close-before-settle-reopen",
      "dictation-exit-reopen",
    ],
  );
  const rapidComplete = exactSet(
    Object.keys(rapidToggle?.receipt?.phases ?? {}),
    ["actions", "notes", "dictation"],
  );
  const evidenceComplete = mode === "all"
    ? mainVisualMatrixComplete
      && mainWidthMatrixComplete
      && lockedCellsComplete
      && lockedTreatment?.cells?.find(
          (cell: any) => cell.fixture === "saturated-stripes",
        )?.motionMetrics?.frames?.length >= 18
      && lifecycleComplete
      && rapidComplete
    : mode === "green"
    ? lifecycleComplete
      && rapidComplete
    : mode === "locked"
    ? lockedCellsComplete
      && lockedTreatment?.cells?.find(
          (cell: any) => cell.fixture === "saturated-stripes",
        )?.motionMetrics?.frames?.length >= 18
      && lifecycleComplete
    : true;
  if (rapidToggle != null) {
    setupErrors.push(...validateChildReceipt(
      rapidToggle.receipt,
      identity,
      "rapid-toggle",
      rapidToggle.exitCode,
    ));
    if (
      rapidToggle.binarySha256Before !== identity.binarySha256
      || rapidToggle.binarySha256After !== identity.binarySha256
    ) setupErrors.push("rapid-toggle binary changed before or after child");
  }
  if (lifecycleFilmstrip != null) {
    setupErrors.push(...validateChildReceipt(
      lifecycleFilmstrip.receipt,
      identity,
      mode === "all" || mode === "locked" ? "lifecycle-saturated" : "lifecycle",
      lifecycleFilmstrip.exitCode,
    ));
    const lifecycleBefore = lifecycleFilmstrip.binarySha256Before
      ?? identity.binarySha256;
    const lifecycleAfter = lifecycleFilmstrip.binarySha256After
      ?? identity.binarySha256;
    if (
      lifecycleBefore !== identity.binarySha256
      || lifecycleAfter !== identity.binarySha256
    ) setupErrors.push("lifecycle binary changed before or after child");
  }
  if (notesFallback != null) {
    setupErrors.push(...validateChildReceipt(
      notesFallback.receipt,
      identity,
      "notes-fallback",
      notesFallback.exitCode,
    ));
    if (
      notesFallback.binarySha256Before !== identity.binarySha256
      || notesFallback.binarySha256After !== identity.binarySha256
    ) setupErrors.push("notes fallback binary changed before or after child");
  }
  const requiredChildScenarios = [
    "main-window",
    ...(lockedTreatment?.cells ?? []).map((cell: any) => `locked:${cell.fixture}`),
    ...(rapidToggle == null ? [] : ["rapid-toggle"]),
    ...(notesFallback == null ? [] : ["notes-fallback"]),
    ...(lifecycleFilmstrip == null
      ? []
      : [
        mode === "all" || mode === "locked"
          ? "lifecycle-saturated"
          : "lifecycle",
      ]),
  ];
  const observedChildScenarios = [
    mainReceipt?.scenario,
    ...(lockedTreatment?.cells ?? []).map((cell: any) =>
      existsSync(cell.mainReceiptPath)
        ? JSON.parse(readFileSync(cell.mainReceiptPath, "utf8"))?.scenario
        : null
    ),
    ...(rapidToggle == null ? [] : [rapidToggle.receipt?.scenario]),
    ...(notesFallback == null ? [] : [notesFallback.receipt?.scenario]),
    ...(lifecycleFilmstrip == null ? [] : [lifecycleFilmstrip.receipt?.scenario]),
  ].filter((item): item is string => typeof item === "string");
  setupErrors.push(...validateUniqueScenarioSet(
    observedChildScenarios,
    requiredChildScenarios,
  ));
  if (!evidenceComplete) {
    setupErrors.push("exact whole-premise evidence registry is incomplete");
  }
  const children = [
    mainReceipt,
    ...(lockedTreatment?.cells ?? []).map((cell: any) =>
      existsSync(cell.mainReceiptPath)
        ? JSON.parse(readFileSync(cell.mainReceiptPath, "utf8"))
        : null
    ),
    rapidToggle?.receipt,
    notesFallback?.receipt,
    lifecycleFilmstrip?.receipt,
  ].filter(Boolean);
  const lockedObserverFailed = (lockedTreatment?.cells ?? []).some(
    (cell: any) =>
      (
        typeof cell?.metricsExitCode === "number"
        && cell.metricsExitCode !== 0
        && cell?.metrics == null
      )
      || (
        typeof cell?.motionMetricsExitCode === "number"
        && cell.motionMetricsExitCode !== 0
        && cell?.motionMetrics == null
      )
      || (
        typeof cell?.saturatedLifecycle?.entryMotionMetricsExitCode === "number"
        && cell.saturatedLifecycle.entryMotionMetricsExitCode !== 0
        && cell?.saturatedLifecycle?.entryMotionMetrics == null
      ),
  );
  const compositeEvaluators = [
    ...(lockedTreatment == null
      ? []
      : [compositeEvaluator(lockedTreatment.pass === true, lockedObserverFailed)]),
  ];
  const finalBinarySha256 = sha256File(binary);
  if (finalBinarySha256 !== identity.binarySha256) {
    setupErrors.push("binary changed during evidence run");
  }
  const finalDisposition = aggregateDisposition(
    [...children, ...compositeEvaluators],
    setupErrors,
  );
  const receipt = {
    schemaVersion: 2,
    ...identity,
    scenario: "whole-premise",
    startedAt,
    finishedAt: new Date().toISOString(),
    mode,
    finalBinarySha256,
    setupErrors,
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
      lifecycle: [
        "main-exit",
        "main-entry",
        "notes-entry",
        "notes-close-before-settle-reopen",
        "dictation-exit-reopen",
      ],
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
    notesFallback,
    lifecycleFilmstrip,
    evidenceComplete,
    pass: finalDisposition === "EVALUABLE_PASS"
      && evidenceComplete
      && (lockedTreatment == null || lockedTreatment.pass === true)
      && (rapidToggle == null || rapidToggle.receipt?.pass === true)
      && (notesFallback == null || notesFallback.receipt?.pass === true)
      && (lifecycleFilmstrip == null
        || lifecycleFilmstrip.receipt?.pass === true),
    disposition: finalDisposition,
  };
  const receiptPath = explicitReceiptPath ?? join(outputDirectory, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify(receipt, null, 2));
  process.exitCode = receipt.pass
    ? 0
    : finalDisposition.startsWith("INVALID_")
    ? 2
    : 1;
}

await main();
