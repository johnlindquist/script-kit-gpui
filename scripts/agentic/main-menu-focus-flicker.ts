#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import {
  buildArtifactLifecycle,
  claimOutput,
  commitFinalReceipt,
  materializeAtomic,
  validateArtifact,
  validateOutputTarget,
  writeJsonArtifactAtomic,
  type ArtifactReceipt,
  type ArtifactSpec,
} from "./artifact-lifecycle";

const repoRoot = resolve(import.meta.dir, "../..");

function usage(): string {
  return `Usage: bun scripts/agentic/main-menu-focus-flicker.ts [options]

Options:
  --session <name>       Session label (uniquified per process/run)
  --out <directory>      Fresh output directory under .test-output or the system temp directory
  --duration-ms <ms>     Sampling duration (default: 160)
  -h, --help             Show this help`;
}

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(usage());
  process.exit(0);
}

function argValue(name: string, fallback: string): string {
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
}

const sessionLabel = argValue("--session", "main-menu-focus-flicker");
const sessionName = `${sessionLabel}-${process.pid}-${Date.now()}`;
const outDir =
  process.argv.includes("--out")
    ? resolve(argValue("--out", ""))
    : join(repoRoot, ".test-output", "main-menu-focus-flicker", sessionName);
const durationMs = Number(argValue("--duration-ms", "160"));
const outputPlan = validateOutputTarget({
  repoRoot,
  candidate: outDir,
  kind: "directory",
  probeId: "main-menu-focus-flicker",
});

const homeDir = join(outDir, "home");
const kitDir = join(homeDir, ".scriptkit");
const scriptsDir = join(kitDir, "plugins", "main", "scripts");

function seedFixtures() {
  mkdirSync(scriptsDir, { recursive: true });
  writeFileSync(
    join(kitDir, "config.ts"),
    `export default {
  unifiedSearch: {
    files: { enabled: false, globalSearch: false, recentFiles: false, directoryBrowse: false },
    notes: { enabled: false },
    clipboardHistory: { enabled: false },
    dictationHistory: { enabled: false },
    agent_chatHistory: { enabled: false },
    aiVault: { enabled: false },
    browserTabs: { enabled: false },
    browserHistory: { enabled: false },
  },
};
`,
  );

  for (const [name, description] of [
    ["flicker-alpha", "Flicker Alpha description proves selected rows expose details."],
    ["flicker-beta", "Flicker Beta description proves selected rows expose details."],
    ["flicker-gamma", "Flicker Gamma description proves selected rows expose details."],
  ]) {
    writeFileSync(
      join(scriptsDir, `${name}.ts`),
      `// Name: ${name.replace("-", " ")}
// Description: ${description}
console.log(${JSON.stringify(name)});
`,
    );
  }
}

function provenance(binary: string): Json {
  const gitSha = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const gitStatus = spawnSync("git", ["status", "--porcelain"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  return {
    binary,
    binarySha256: existsSync(binary)
      ? createHash("sha256").update(readFileSync(binary)).digest("hex")
      : null,
    gitSha: gitSha.status === 0 ? gitSha.stdout.trim() : null,
    sourceDirty: gitStatus.status === 0 ? gitStatus.stdout.trim().length > 0 : null,
  };
}

function elementsFromReceipt(receipt: Json): Json[] {
  const candidates = [receipt.elements, receipt.nodes, receipt.elementSnapshot?.nodes];
  for (const candidate of candidates) {
    if (Array.isArray(candidate)) return candidate as Json[];
  }
  return [];
}

function selectedChoices(elements: Json[]): Json[] {
  return elements.filter(
    (element) =>
      element?.elementType === "choice" ||
      element?.type === "choice" ||
      element?.role === "row",
  ).filter((element) => element?.selected === true || element?.selected === "true");
}

async function sample(driver: Driver, label: string, t0: number): Promise<Json> {
  const [state, elementsReceipt] = await Promise.all([
    driver.getState({ timeoutMs: 5000 }),
    driver.getElements({ limit: 40 }, { timeoutMs: 5000 }),
  ]);
  const elements = elementsFromReceipt(elementsReceipt);
  const selected = selectedChoices(elements);
  return {
    label,
    tMs: Math.round(performance.now() - t0),
    inputValue: state.inputValue ?? null,
    promptType: state.promptType ?? null,
    selectedIndex: state.selectedIndex ?? null,
    visibleChoiceCount: state.visibleChoiceCount ?? null,
    selectedChoices: selected.map((choice) => ({
      semanticId: choice.semanticId ?? choice.semantic_id ?? null,
      text: choice.text ?? null,
      value: choice.value ?? null,
      index: choice.index ?? null,
    })),
  };
}

function assertStable(samples: Json[]) {
  const failures: Json[] = [];
  for (const sample of samples) {
    if (sample.inputValue !== "flicker") continue;
    if (Number(sample.visibleChoiceCount ?? 0) <= 0) continue;
    if (sample.selectedChoices.length !== 1) {
      failures.push({ reason: "expected exactly one selected choice", sample });
      continue;
    }
    const selected = sample.selectedChoices[0];
    if (typeof selected.value !== "string" || !selected.value.includes("description proves")) {
      failures.push({ reason: "selected choice did not expose description value", sample });
    }
  }
  if (failures.length > 0) {
    throw new Error(`main menu focus flicker samples failed: ${JSON.stringify(failures)}`);
  }
}

async function main() {
  const claim = claimOutput(outputPlan);
  const binary =
    process.env.SCRIPT_KIT_GPUI_BINARY ??
    join(repoRoot, "target-agent", "artifacts", "main-menu-focus-flicker", "script-kit-gpui");
  let driver: Driver | null = null;
  const samples: Json[] = [];
  const receipt: Json = {
    schemaVersion: 2,
    status: "error",
    behavior: { status: "fail", failure: null },
    durationMs,
    outDir,
    provenance: provenance(binary),
    session: { name: sessionName, pid: null },
    samples,
    cleanup: {
      attempted: false,
      hidden: false,
      hiddenState: null,
      closed: false,
      error: null,
    },
  };
  try {
    seedFixtures();
    driver = await Driver.launch({
      binary,
      sessionName,
      sessionDir: join(outDir, "driver"),
      sandboxHome: false,
      env: {
        HOME: homeDir,
        SK_PATH: kitDir,
        SCRIPT_KIT_AGENTIC_RUST_LOG:
          "info,script_kit::selection=debug,script_kit::scroll=debug,gpui=warn",
      },
      readyTimeoutMs: 15_000,
      defaultTimeoutMs: 5_000,
    });
    receipt.session.pid = driver.pid ?? null;
    await driver.setFilterAndWait("");
    await driver.waitForState({ promptType: "scriptList" }, { timeoutMs: 5000 });
    await driver.setFilterAndWait("gamma");
    await Bun.sleep(50);
    driver.simulateKey("down");
    await Bun.sleep(50);

    const t0 = performance.now();
    const setFilter = driver.setFilterAndWait("flicker", { timeoutMs: 5000 });
    while (performance.now() - t0 < durationMs) {
      samples.push(await sample(driver, "post-replacement", t0));
      await Bun.sleep(8);
    }
    await setFilter;
    samples.push(await sample(driver, "settled", t0));
    assertStable(samples);
    receipt.behavior.status = "pass";
    receipt.stats = driver.stats;
  } catch (error) {
    receipt.behavior.failure = error instanceof Error ? error.message : String(error);
  } finally {
    receipt.cleanup.attempted = driver !== null;
    if (driver) {
      try {
        driver.send({ type: "hide", requestId: `main-menu-flicker-cleanup-hide-${Date.now()}` });
        await driver.waitForState(
          { windowVisible: false },
          { timeoutMs: 5_000, pollIntervalMs: 10 },
        );
        const hiddenState = await driver.getState({ timeoutMs: 5_000 });
        if (hiddenState.windowVisible !== false) {
          throw new Error(`cleanup state remained visible: ${JSON.stringify(hiddenState)}`);
        }
        receipt.cleanup.hidden = true;
        receipt.cleanup.hiddenState = hiddenState;
      } catch (error) {
        const cleanupError = error instanceof Error ? error.message : String(error);
        receipt.cleanup.error = cleanupError;
        receipt.behavior.failure ??= `cleanup: ${cleanupError}`;
      }
      try {
        await driver.close();
        if (driver.alive) {
          throw new Error(`owned driver process ${driver.pid ?? "unknown"} survived close`);
        }
        receipt.cleanup.closed = true;
      } catch (error) {
        const closeError = error instanceof Error ? error.message : String(error);
        receipt.cleanup.error = receipt.cleanup.error
          ? `${receipt.cleanup.error}; close: ${closeError}`
          : `close: ${closeError}`;
        receipt.behavior.failure ??= `close: ${closeError}`;
      }
    }

    const correlations = driver?.matchedResponses.map(({ requestId, expectedType }) => ({
      requestId,
      expectedType: expectedType ?? "__missing_expected_type__",
    })) ?? [];
    const specs: ArtifactSpec[] = [
      {
        id: "app-log",
        sourceName: "app.log",
        required: true,
        mediaType: "text/plain",
        kind: "text",
        acceptedTextMarkers: ["STARTUP_READY ", "APP_READY|"],
      },
      {
        id: "protocol-responses",
        sourceName: "protocol-responses.ndjson",
        required: true,
        mediaType: "application/x-ndjson",
        kind: "ndjson",
        correlations,
      },
      {
        id: "lifecycle",
        sourceName: "lifecycle.json",
        required: true,
        mediaType: "application/json",
        kind: "json",
      },
    ];
    const artifacts: ArtifactReceipt[] = [];
    const writersFinalized = receipt.cleanup.closed === true
      && driver?.alive === false
      && driver.finalization.processExited === true
      && driver.finalization.streamsDrained === true
      && driver.finalization.logWriterClosed === true;
    const lifecycleProof = {
      schemaVersion: 1,
      probeId: "main-menu-focus-flicker",
      runId: claim.owner.runId,
      finalizationKind: "driver-close",
      hidden: receipt.cleanup.hidden === true,
      processExited: driver?.finalization.processExited ?? false,
      streamsDrained: driver?.finalization.streamsDrained ?? false,
      logWriterClosed: driver?.finalization.logWriterClosed ?? false,
      aliveAfterClose: driver?.alive ?? false,
      completedAt: new Date().toISOString(),
    };
    try {
      if (driver && driver.alive === false && driver.finalization.logWriterClosed) {
        for (const [source, destination] of [
          [driver.logPath, join(claim.artifactsRoot, "app.log")],
          [join(driver.sessionDir, "protocol-responses.ndjson"), join(claim.artifactsRoot, "protocol-responses.ndjson")],
        ] as const) {
          materializeAtomic(claim, {
            sourceRoot: dirname(source),
            sourceName: basename(source),
            destinationName: basename(destination),
          });
        }
      }
      writeJsonArtifactAtomic(claim, "lifecycle.json", lifecycleProof);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      receipt.cleanup.error = receipt.cleanup.error
        ? `${receipt.cleanup.error}; artifacts: ${message}`
        : `artifacts: ${message}`;
    }
    for (const spec of specs) {
      artifacts.push(validateArtifact(
        join(claim.artifactsRoot, spec.destinationName ?? spec.sourceName),
        spec,
        claim.artifactsRoot,
      ));
    }
    receipt.artifactLifecycle = buildArtifactLifecycle({
      claim,
      finalizationKind: "driver-close",
      writersFinalized,
      specs,
      artifacts,
    });
    const lifecycleValid = receipt.artifactLifecycle.allRequiredValid === true
      && receipt.artifactLifecycle.allRecordedPathsReadable === true;
    receipt.status = receipt.behavior.status === "pass"
      && receipt.cleanup.hidden === true
      && lifecycleValid
      ? "pass"
      : "error";
    receipt.failure = [receipt.behavior.failure, receipt.cleanup.error]
      .filter(Boolean)
      .join("; ") || null;
    if (receipt.status !== "pass") {
      receipt.failurePreservation = {
        outputRootPreserved: true,
        sessionRootPreserved: Boolean(driver && existsSync(driver.sessionDir)),
        stagingPreserved: false,
        paths: [claim.root, ...(driver && existsSync(driver.sessionDir) ? [driver.sessionDir] : [])],
        reason: receipt.failure ?? "artifact lifecycle validation failed",
      };
    }
    commitFinalReceipt(claim, receipt, specs, artifacts);
  }
  console.log(JSON.stringify(receipt, null, 2));
  process.exitCode = receipt.status === "pass" ? 0 : 1;
}

await main();
