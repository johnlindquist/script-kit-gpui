#!/usr/bin/env bun
import { mkdirSync, openSync, writeSync, closeSync } from "node:fs";
import { join, resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { spawnOwnedProcess, type OwnedProcess } from "../agentic/owned-process.ts";
import { beginManagedTask, updateManagedTask, finalizeManagedTask, createOwnedStagingDirectory, writeJsonArtifactAtomic, claimOutput, validateOutputTarget,
  type OutputClaim, type OwnedCleanup } from "../agentic/artifact-lifecycle.ts";
import { verifyImmutableArtifact, ArtifactVerificationError, type ArtifactReference } from "../agentic/build-artifact.ts";
import { boundedObservation, unknownOwnedCleanup, DriverLifecycleError } from "./driver.ts";
import { createEvaluationClaim, readArtifactReference, runRuntimeJourney, annotateOwnedEvidence, commitOwnedReport, type RuntimeJourneyReceipt } from "./design.ts";
import { PRODUCTION_STORIES, CORE_JOURNEYS, STORY_TEST_PREFIX, selectStoryTests, observeStoryTests, aggregateCleanup,
  type LibtestSelection, type LibtestObservation } from "./lib/story-contract.ts";
import { prepareValidatedReceipt, validateReceiptFile, RECEIPT_SCHEMA_VERSION } from "./lib/receipt-schema.ts";
import { diagnostic, filePath, productStatic } from "./lib/privacy.ts";
import { runSdkJourney } from "./sdk-journey.ts";
import { runFooterOwnershipJourney } from "./footer-journey.ts";

const ROOT = resolve(import.meta.dir, "../..");
export interface LibraryStoryResult {
  selection: LibtestSelection; execution?: LibtestObservation; cleanup: OwnedCleanup; error?: string;
  artifactReference: ArtifactReference; proofLevels: readonly string[];
}
interface ProcessObservation { stdout: string; stderr: string; exitCode: number; cleanup: OwnedCleanup }

async function runLibtestProcess(reference: ArtifactReference, claim: OutputClaim, executable: string, args: string[], label: string): Promise<ProcessObservation> {
  const runtimeClaim = claimOutput(validateOutputTarget({ repoRoot: claim.plan.repoRoot, candidate: join(claim.root, `libtest-${label}-${randomUUID()}`), kind: "directory", probeId: "libtest-runtime" }));
  const task = beginManagedTask(runtimeClaim, "runtime-run", [reference]);
  let proc: OwnedProcess | undefined;
  let stdout = ""; let stderr = ""; let exitCode = -1;
  let cleanup = unknownOwnedCleanup(false);
  const readers: ReadableStreamDefaultReader<Uint8Array>[] = [];
  let logFd: number | undefined;
  let streamPromises: Promise<void>[] = [];
  let failure: unknown;
  try {
  const directory = createOwnedStagingDirectory(runtimeClaim);
  const home = join(directory, "home"); const tmp = join(directory, "tmp");
  mkdirSync(home, { recursive: true, mode: 0o700 }); mkdirSync(tmp, { mode: 0o700 });
    logFd = openSync(join(directory, `${label}.log`), "wx", 0o600);
    proc = await spawnOwnedProcess({ argv: [executable, ...args], cwd: ROOT, timeoutMs: 120000, maxOutputBytes: 4 * 1024 * 1024,
      env: { PATH: "/usr/bin:/bin:/usr/sbin:/sbin", HOME: home, SK_PATH: join(home, ".scriptkit"), CODEX_HOME: join(home, ".codex"),
        XDG_CONFIG_HOME: join(home, ".config"), XDG_CACHE_HOME: join(home, ".cache"), XDG_DATA_HOME: join(home, ".local/share"), TMPDIR: tmp,
        LANG: "en_US.UTF-8", LC_ALL: "C", TZ: "UTC", SCRIPT_KIT_NONINTERACTIVE: "1", RUST_TEST_THREADS: "1", RUST_LOG: "warn",
        SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0", SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0", SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
        SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0", SCRIPT_KIT_ALLOW_LIVE_AI: "0" } });
    cleanup = unknownOwnedCleanup(true);
    updateManagedTask(task, { state: "running", ownedProcesses: [proc.identity] });
    const consume = async (stream: ReadableStream<Uint8Array>, isStdout: boolean) => {
      const reader = stream.getReader(); readers.push(reader); const decoder = new TextDecoder(); let bytes = 0;
      try {
        for (;;) {
          const next = await reader.read(); if (next.done) break;
          bytes += next.value.byteLength; if (bytes > 2 * 1024 * 1024) throw new Error("libtest_output_budget_exhausted");
          if (logFd !== undefined) writeSync(logFd, next.value);
          const text = decoder.decode(next.value, { stream: true }); if (isStdout) stdout += text; else stderr += text;
        }
        const text = decoder.decode(); if (isStdout) stdout += text; else stderr += text;
      } finally { reader.releaseLock(); }
    };
    streamPromises = [consume(proc.stdout, true), consume(proc.stderr, false)];
    // Observe readers and exit together, so a reader failure enters the same owned cleanup scope.
    const result = await boundedObservation(Promise.all([proc.exited, ...streamPromises]), 123000);
    if (result.completed === false) throw result.error;
    exitCode = result.value[0] as number;
  } catch (error) {
    failure = error;
    if (error && typeof error === "object" && "cleanup" in error) {
      // spawnOwnedProcess attaches its canonical in-process cleanup record to startup failures.
      const observedCleanup = error.cleanup as OwnedCleanup;
      cleanup = observedCleanup;
    }
  }
  finally {
    if (proc) {
      const closed = await boundedObservation(proc.close(), 8000);
      cleanup = closed.completed ? closed.value : unknownOwnedCleanup(true);
      const streams = await boundedObservation(Promise.allSettled(streamPromises), 1000);
      const drained = streams.completed && streams.value.every(result => result.status === "fulfilled");
      if (!drained) await boundedObservation(Promise.allSettled(readers.map(reader => reader.cancel())), 500);
      cleanup = { ...cleanup, streamsDrained: drained && cleanup.streamsDrained, closed: cleanup.closed && drained };
    }
    let logClosed = logFd === undefined;
    if (logFd !== undefined) { try { closeSync(logFd); logFd = undefined; logClosed = true; } catch { logClosed = false; } }
    cleanup = { ...cleanup, logWriterClosed: logClosed, closed: cleanup.closed && logClosed };
    try {
      updateManagedTask(task, { result: { status: !failure && exitCode === 0 ? "succeeded" : "failed", exitCode } });
      cleanup = finalizeManagedTask(task, cleanup).cleanup;
    } catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "runtime_record_finalization_failed"] }; }
  }
  return { stdout, stderr, exitCode, cleanup };
}

export async function runLibraryStories(reference: ArtifactReference, claim: OutputClaim): Promise<LibraryStoryResult> {
  const result: LibraryStoryResult = { artifactReference: reference, proofLevels: ["domain", "GPUI-test-platform"], selection: { names: [], issues: [] }, cleanup: unknownOwnedCleanup(false) };
  const cleanups: OwnedCleanup[] = [];
  try {
    const artifact = verifyImmutableArtifact(ROOT, reference, { kind: "rust-libtest", packageName: "script-kit-gpui", targetName: "script_kit_gpui", profile: "test", sourcePolicy: "current-content" });
    const listing = await runLibtestProcess(reference, claim, artifact.executablePath, ["--list", STORY_TEST_PREFIX], "list");
    cleanups.push(listing.cleanup); result.selection = selectStoryTests(listing.stdout);
    if (listing.exitCode !== 0) result.selection.issues.push("libtest_listing_failed");
    if (!listing.cleanup.closed || result.selection.issues.length) throw new Error("invalid_libtest_selection");
    // Multiple exact names are supported libtest filters, with --exact preventing prefix expansion.
    const execution = await runLibtestProcess(reference, claim, artifact.executablePath,
      ["--exact", ...result.selection.names, "--test-threads=1", "--format=pretty", "--nocapture"], "execute");
    cleanups.push(execution.cleanup); result.execution = observeStoryTests(execution.stdout, execution.exitCode, result.selection.names);
    verifyImmutableArtifact(ROOT, reference, { kind: "rust-libtest", packageName: "script-kit-gpui", targetName: "script_kit_gpui", profile: "test", sourcePolicy: "current-content" });
  } catch (error) {
    result.error = error instanceof ArtifactVerificationError ? error.code : "library_story_failed";
    if (error instanceof DriverLifecycleError) cleanups.push(error.cleanup);
  } finally { result.cleanup = aggregateCleanup(cleanups); }
  return result;
}

export async function runStories(argv: string[]): Promise<void> {
  const command = argv[0] ?? "discover";
  const arg = (name: string) => { const index = argv.indexOf(name); return index < 0 ? undefined : argv[index + 1]; };
  if (command === "discover") { console.log(JSON.stringify({ library: PRODUCTION_STORIES, core: CORE_JOURNEYS, proof: "catalogue-only", nativeExclusions: ["AppKit", "OS IME", "global input", "live providers"] })); return; }
  if (command === "diagnose") {
    if (!arg("--receipt")) throw new Error("--receipt required");
    const result = validateReceiptFile("devtools.stories.run", arg("--receipt")!); console.log(JSON.stringify({ historicalValidation: result, freshRuntimeProof: false })); return;
  }
  if (command !== "run" || !arg("--libtest") || !arg("--out")) throw new Error("stories run --libtest <reference.json> --app <reference.json> --out <fresh-dir> [--lane library]");
  const lane = arg("--lane") ?? "all";
  if (!["library", "all"].includes(lane) || (arg("--scope") && arg("--scope") !== "core")) throw new Error("unknown story lane/scope");
  if (lane !== "library" && !arg("--app")) throw new Error("default story lane requires --app");
  const reference = readArtifactReference(arg("--libtest")!); const app = arg("--app") ? readArtifactReference(arg("--app")!) : undefined;
  const claim = createEvaluationClaim(arg("--out")!, "devtools.stories");
  const task = beginManagedTask(claim, "evidence-run", [reference, ...(app ? [app] : [])]);
  const startedAt = new Date().toISOString(); const start = performance.now();
  const library = await runLibraryStories(reference, claim); const journeys: RuntimeJourneyReceipt[] = [];
  if (lane !== "library") {
    for (const id of CORE_JOURNEYS) {
      const journey = await runRuntimeJourney(id, app!, claim); journeys.push(journey);
    }
    const sdk = await runSdkJourney(app!, claim); journeys.push(sdk);
    const footer = await runFooterOwnershipJourney(app!, claim); journeys.push(footer);
  }
  let cleanup = aggregateCleanup([library.cleanup, ...journeys.map(journey => journey.cleanup)]);
  try { cleanup = finalizeManagedTask(task, cleanup).cleanup; } catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "evidence_finalization_failed"] }; }
  const pass = !library.error && !library.selection.issues.length && library.execution?.issues.length === 0 && journeys.every(journey => journey.pass) && cleanup.closed;
  const prepared = prepareValidatedReceipt("devtools.stories.run", { schemaVersion: RECEIPT_SCHEMA_VERSION, tool: "script-kit-devtools.stories", command: "stories.run", lane, startedAt, durationMs: performance.now() - start,
    classification: !cleanup.closed ? "invalid-cleanup" : pass ? "ok" : "reproduced", disposition: cleanup.closed ? undefined : "INVALID_CLEANUP",
    library: annotateOwnedEvidence(library), journeys: annotateOwnedEvidence(journeys),
    artifactReferences: [reference, ...(app ? [app] : [])], evidenceClass: lane === "library" ? "UNIT_BEHAVIOR" : "DIRECT_RUNTIME_PROOF",
    provesRuntimeBehavior: lane !== "library" && pass, proofLevels: productStatic(lane === "library" ? library.proofLevels : [...library.proofLevels, "owned-production-runtime"]),
    cleanup, assertions: [{ id: "exact_library_selection", pass: !library.error && library.execution?.issues.length === 0 }, ...journeys.map(journey => ({ id: journey.id, pass: journey.pass }))],
    errors: diagnostic(library.error ? [library.error] : []), output: filePath(claim.root),
  });
  const compact = commitOwnedReport(claim, prepared.receipt, cleanup); console.log(JSON.stringify(compact)); process.exitCode = prepared.exitCode;
}
if (import.meta.main) await runStories(Bun.argv.slice(2));
