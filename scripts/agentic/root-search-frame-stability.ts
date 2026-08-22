#!/usr/bin/env bun
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { basename, dirname, join, resolve } from "node:path";
import { Driver } from "../devtools/driver";
import { assertNoninteractiveProtocolCommand } from "../devtools/lib/operator-safety.ts";
import { assertPerformanceContract } from "../devtools/lib/performance-contract.ts";
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

type Json = Record<string, any>;

const repoRoot = resolve(import.meta.dir, "../..");

function argValue(name: string): string | null {
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : null;
}

function usage(): string {
  return `Usage: bun scripts/agentic/root-search-frame-stability.ts --binary <path> --receipt <path> [options]

Required:
  --binary <path>             Stable script-kit-gpui artifact to launch
  --receipt <path>            JSON receipt output path

Options:
  --session <name>            Session label (uniquified per process/run)
  --query <text>              Root query (default: zzqxframeproof)
  --timeout <ms>              Protocol timeout (default: 10000)
  --poll <ms>                 Sample interval (default: 25)
  --inject-forbidden-shift    Deterministically fail the frame-identity gate
  --describe-contract         Print static hidden-runtime safety/metric metadata
  -h, --help                  Show this help

Runtime app launch requires CI=true, SCRIPT_KIT_NONINTERACTIVE=1, and
SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1. Static inspection never starts the app.`;
}

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(usage());
  process.exit(0);
}
if (process.argv.includes("--describe-contract")) {
  const contract = {
    schemaVersion: 1,
    tool: "root-search-frame-stability",
    evidenceClass: "STATIC_INVENTORY",
    runtimeEvidenceClass: "RUNTIME_HIDDEN",
    metricKind: "semantic_frame_identity",
    observationClass: "SEMANTIC_FRAME",
    observationPoint: "stateResult.mainWindowPreflight.semanticFingerprint",
    measuresPaint: false,
    safety: {
      startsApplication: false,
      runtimeStartsApplication: true,
      runtimeRequiresSandboxHome: true,
      runtimeRequiresHiddenWindow: true,
      runtimeRequiresNoninteractive: true,
      runtimeRequiresCiEnvironment: true,
      runtimeRequiresIsolatedAppLaunchOptIn:
        "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1",
      revealsWindow: false,
      focusesWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
    },
  };
  assertPerformanceContract(contract);
  console.log(JSON.stringify(contract, null, 2));
  process.exit(0);
}
if (process.env.SCRIPT_KIT_NONINTERACTIVE !== "1") {
  throw new Error(
    "hidden root frame benchmark refused before app launch; " +
      "SCRIPT_KIT_NONINTERACTIVE=1 is required for the capture-free sandbox",
  );
}
if (process.env.SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH !== "1") {
  throw new Error(
    "hidden root frame benchmark refused before app launch; " +
      "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1 is required for an explicitly approved isolated CI run",
  );
}
if (process.env.CI !== "true") {
  throw new Error(
    "hidden root frame benchmark refused before app launch; " +
      "CI=true is required and operator-local app launches are forbidden",
  );
}
assertNoninteractiveProtocolCommand({ type: "getState", target: { type: "main" } });

const binaryArg = argValue("--binary");
const receiptArg = argValue("--receipt");
if (!binaryArg || !receiptArg) {
  console.error(usage());
  throw new Error("--binary and --receipt are required");
}

const binary = resolve(repoRoot, binaryArg);
const receiptPath = resolve(repoRoot, receiptArg);
const outputPlan = validateOutputTarget({
  repoRoot,
  candidate: receiptPath,
  kind: "receipt",
  probeId: "root-search-frame-stability",
});
const sessionLabel = argValue("--session") ?? "root-search-frame-stability";
const sessionName = `${sessionLabel}-${process.pid}-${Date.now()}`;
const query = argValue("--query") ?? "zzqxframeproof";
const timeoutMs = Number(argValue("--timeout") ?? "10000");
const pollMs = Number(argValue("--poll") ?? "25");
const injectForbiddenShift = process.argv.includes("--inject-forbidden-shift");
const fixtureResultPath = `/tmp/${query}-late-provider-result.txt`;
let hiddenStateAssertionCount = 0;

if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
  throw new Error(`--timeout must be a positive number, got ${JSON.stringify(timeoutMs)}`);
}
if (!Number.isFinite(pollMs) || pollMs <= 0) {
  throw new Error(`--poll must be a positive number, got ${JSON.stringify(pollMs)}`);
}

function git(args: string[]): string {
  const result = spawnSync("git", args, { cwd: repoRoot, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.stderr.trim()}`);
  }
  return result.stdout.trim();
}

function binarySha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function elementsFingerprint(elementsResult: Json): string {
  const elements = Array.isArray(elementsResult.elements) ? elementsResult.elements : [];
  return elements
    .filter((element: Json) => typeof element.semanticId === "string")
    .map((element: Json) =>
      [
        element.role ?? "",
        element.semanticId,
        element.text ?? "",
        element.index ?? "",
        element.action ?? "",
      ].join(":"),
    )
    .join("|");
}

function requirePreflight(state: Json, label: string): Json {
  requireHiddenState(state, label);
  const preflight = state.mainWindowPreflight;
  if (!preflight) {
    throw new Error(`${label}: missing mainWindowPreflight in getState receipt`);
  }
  for (const field of [
    "selectedResultKey",
    "selectedResultRole",
    "visibleResultKeyFingerprint",
    "visibleRowFingerprint",
    "visibleResultCount",
    "visibleResults",
    "enterAction",
  ]) {
    if (!(field in preflight)) {
      throw new Error(`${label}: mainWindowPreflight missing ${field}`);
    }
  }
  return preflight;
}

function requireHiddenState(state: Json, label: string): void {
  if (state.windowVisible !== false) {
    throw new Error(
      `${label}: hidden semantic proof refused a visible or unknown window state: ${JSON.stringify({
        windowVisible: state.windowVisible ?? null,
      })}`,
    );
  }
  hiddenStateAssertionCount += 1;
}

async function comparable(driver: Driver, state: Json, tag: string): Promise<Json> {
  const preflight = requirePreflight(state, tag);
  const elements = (await driver.getElements(
    { target: { type: "main" } },
    { timeoutMs },
  )) as Json;
  return {
    selectedIndex: preflight.selectedIndex,
    selectedResultKey: preflight.selectedResultKey ?? null,
    selectedResultRole: preflight.selectedResultRole,
    visibleResultKeyFingerprint: preflight.visibleResultKeyFingerprint,
    visibleRowFingerprint: preflight.visibleRowFingerprint,
    visibleResultCount: preflight.visibleResultCount,
    visibleResults: preflight.visibleResults,
    enterAction: preflight.enterAction,
    elementsFingerprint: elementsFingerprint(elements),
  };
}

function assertSameFrame(baseline: Json, sample: Json, label: string) {
  const before = JSON.stringify(baseline);
  const after = JSON.stringify(sample);
  if (before !== after) {
    throw new Error(
      `${label}: visible frame changed while provider resolved\nbefore=${before}\nafter=${after}`,
    );
  }
}

function numericField(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function hasWarmRootFileCache(status: Json): boolean {
  return numericField(status.cacheEntryCount) > 0 && numericField(status.cacheResultCount) > 0;
}

function requireRootFileStatus(state: Json, label: string): Json {
  const status = state.rootFileSearch;
  if (status?.query !== query) {
    throw new Error(
      `${label}: root file search did not track query ${JSON.stringify(query)}: ${JSON.stringify(status)}`,
    );
  }
  if (status?.mode !== "GlobalQuery") {
    throw new Error(`${label}: expected GlobalQuery root file mode, got ${JSON.stringify(status)}`);
  }
  return status;
}

function classifyRootFileBaseline(status: Json): Json {
  if (status.visibleLoading !== true) {
    throw new Error(`baseline is not an early visible loading frame: ${JSON.stringify(status)}`);
  }
  if (status.loading === true) {
    return { kind: "loading", observedLoading: true, observedAsyncHandoff: false };
  }
  if (
    status.loading === false &&
    status.visibleResultCount === 0 &&
    numericField(status.generation) >= 1 &&
    hasWarmRootFileCache(status)
  ) {
    return {
      kind: "settled-provider-early-visible-loading",
      observedLoading: false,
      observedAsyncHandoff: true,
      generation: status.generation,
      cacheEntryCount: status.cacheEntryCount,
      cacheResultCount: status.cacheResultCount,
      visibleResultCount: status.visibleResultCount,
    };
  }
  throw new Error(
    `unsupported root file baseline; expected loading frame or settled-provider early visible-loading frame: ${JSON.stringify(status)}`,
  );
}

async function sampleUntilRootFileSettled(
  driver: Driver,
  baseline: Json,
  baselineProof: Json,
  samples: Json[],
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let observedLoading = baselineProof.observedLoading === true;
  let observedAsyncHandoff = baselineProof.observedAsyncHandoff === true;
  let settledStableSamples = 0;
  let injected = false;
  const requiredSettledStableSamples =
    baselineProof.kind === "settled-provider-early-visible-loading" ? 2 : 1;

  while (Date.now() < deadline) {
    const state = (await driver.getState({ timeoutMs })) as Json;
    requireHiddenState(state, `provider-sample-${samples.length}`);
    const status = state.rootFileSearch;
    if (status?.query === query && status?.mode === "GlobalQuery") {
      if (status.loading === true) observedLoading = true;

      const observedFrame = await comparable(driver, state, `sample-${samples.length}`);
      const frame =
        injectForbiddenShift && !injected
          ? { ...observedFrame, visibleRowFingerprint: "__injected_forbidden_shift__" }
          : observedFrame;
      if (injectForbiddenShift && !injected) injected = true;
      samples.push({ rootFileSearch: status, frame, injectionApplied: frame !== observedFrame });
      assertSameFrame(baseline, frame, `samples[${samples.length - 1}]`);

      if (status.loading === false) {
        if (!hasWarmRootFileCache(status)) {
          throw new Error(`provider settled without warming cache; status=${JSON.stringify(status)}`);
        }
        observedAsyncHandoff = true;
        if (!observedLoading && baselineProof.kind !== "settled-provider-early-visible-loading") {
          throw new Error(
            `provider settled without an accepted async handoff proof; baselineProof=${JSON.stringify(
              baselineProof,
            )} status=${JSON.stringify(status)}`,
          );
        }
        settledStableSamples += 1;
        if (observedAsyncHandoff && settledStableSamples >= requiredSettledStableSamples) {
          return state;
        }
      }
    }
    await Bun.sleep(Math.max(25, pollMs));
  }
  throw new Error(`root file search did not settle for ${JSON.stringify(query)}`);
}

const claim = claimOutput(outputPlan);
const receipt: Json = {
  schemaVersion: 3,
  gateId: "root-frame-stable",
  evidenceClass: "RUNTIME_HIDDEN",
  metricKind: "semantic_frame_identity",
  observationClass: "SEMANTIC_FRAME",
  observationPoint: "stateResult.mainWindowPreflight.semanticFingerprint",
  measuresPaint: false,
  status: "fail",
  behavior: { status: "fail", failure: null },
  query,
  injectForbiddenShift,
  receiptPath,
  provenance: {
    binary,
    binarySha256: binarySha256(binary),
    gitSha: git(["rev-parse", "HEAD"]),
    sourceDirty: git(["status", "--porcelain"]).length > 0,
  },
  session: { name: sessionName, pid: null },
  safety: {
    startsApplication: true,
    isolatedCiLaunchAuthorized: true,
    sandboxHome: true,
    windowRevealAllowed: false,
    windowFocusAllowed: false,
    nativeInputAllowed: false,
    screenCaptureAllowed: false,
    hiddenStateAssertionCount: 0,
  },
  samples: [],
};

let driver: Driver | null = null;
try {
  driver = await Driver.launch({
    binary,
    sandboxHome: true,
    sessionName,
    readyTimeoutMs: timeoutMs,
    defaultTimeoutMs: timeoutMs,
    env: {
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
      SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
      SCRIPT_KIT_STARTUP_READY_LOG: "1",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
      SCRIPT_KIT_ROOT_FILE_SEARCH_TEST_PROVIDER: JSON.stringify({
        query,
        delayMs: 250,
        results: [
          {
            path: fixtureResultPath,
            name: `${query}-late-provider-result.txt`,
            fileType: "document",
            size: 42,
            modified: 1,
          },
        ],
      }),
    },
  });
  receipt.session.pid = driver.pid ?? null;
  const initialState = (await driver.getState({ timeoutMs })) as Json;
  requireHiddenState(initialState, "initial protocol-ready state");
  await driver.setFilterAndWait(query, { timeoutMs });

  const before = (await driver.getState({ timeoutMs })) as Json;
  const beforeRootFileSearch = requireRootFileStatus(before, "before");
  const baselineProof = classifyRootFileBaseline(beforeRootFileSearch);
  const baseline = await comparable(driver, before, "before");
  receipt.baselineProof = baselineProof;
  receipt.baseline = {
    inputValue: before.inputValue,
    rootFileSearch: beforeRootFileSearch,
    mainWindowPreflight: baseline,
  };

  const settled = await sampleUntilRootFileSettled(
    driver,
    baseline,
    baselineProof,
    receipt.samples,
  );
  const settledFrame = await comparable(driver, settled, "settled");
  assertSameFrame(baseline, settledFrame, "settled");
  receipt.settled = {
    inputValue: settled.inputValue,
    rootFileSearch: settled.rootFileSearch,
    mainWindowPreflight: settledFrame,
  };
  receipt.behavior.status = "pass";
} catch (error) {
  receipt.behavior.failure = error instanceof Error ? error.message : String(error);
} finally {
  receipt.safety.hiddenStateAssertionCount = hiddenStateAssertionCount;
  receipt.cleanup = {
    attempted: driver !== null,
    hidden: false,
    hiddenState: null,
    closed: false,
    error: null,
  };
  if (driver) {
    try {
      driver.send({ type: "hide", requestId: `root-frame-cleanup-hide-${Date.now()}` });
      await driver.waitForState(
        { windowVisible: false },
        { timeoutMs, pollIntervalMs: pollMs },
      );
      const hiddenState = (await driver.getState({ timeoutMs })) as Json;
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
    probeId: "root-search-frame-stability",
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
      materializeAtomic(claim, {
        sourceRoot: dirname(driver.logPath),
        sourceName: basename(driver.logPath),
        destinationName: "app.log",
      });
      materializeAtomic(claim, {
        sourceRoot: driver.sessionDir,
        sourceName: "protocol-responses.ndjson",
        destinationName: "protocol-responses.ndjson",
      });
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
process.exit(receipt.status === "pass" ? 0 : 1);
