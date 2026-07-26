#!/usr/bin/env bun
/**
 * Manifest-driven multi-build glass lifecycle study
 * (glass-smoke-harness-max-info WP7).
 *
 * One fixture lifetime, one display session, a balanced mirrored-cyclic
 * schedule across N builds, consolidated warmups, per-attempt eligibility,
 * and fail-closed block semantics: a block containing ANY invalid attempt is
 * excluded from paired inference and rescheduled with retryOfBlockId —
 * never silently overwritten.
 *
 * For N builds the ladder needs 13 × N capture runs before early stopping
 * versus 26 × (N − 1) for repeated legacy pair sessions.
 *
 * Verification (WP7): `bun test scripts/agentic/glass-smoke-study.test.ts`
 * plus a `--dry-run` schedule against the example manifest.
 */

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
  appendFileSync,
} from "node:fs";
import os from "node:os";
import { join, resolve } from "node:path";
import { alphaToBitsHex } from "./glass-system-telemetry.ts";

// ---------------------------------------------------------------------------
// Pure schedule algebra
// ---------------------------------------------------------------------------

export interface MirroredBlock {
  blockIndex: number;
  forward: string[];
  reverse: string[];
  slots: string[];
}

/** The plan's exact mirrored-cyclic schedule: every block contains two runs
 * per build and reverses temporal position; the rotation offset cycles the
 * lead position across builds. For two builds, block 0 is A,B,B,A. */
export function mirroredCyclicSchedule(
  buildIds: readonly string[],
  blockCount: number,
): MirroredBlock[] {
  return Array.from({ length: blockCount }, (_, blockIndex) => {
    const offset = blockIndex % buildIds.length;
    const forward = buildIds.map(
      (_, index) => buildIds[(index + offset) % buildIds.length]!,
    );
    return {
      blockIndex,
      forward,
      reverse: [...forward].reverse(),
      slots: [...forward, ...[...forward].reverse()],
    };
  });
}

/**
 * Consolidated warmups: `rounds` rounds; every build appears exactly once
 * per round; the starting position rotates per round and odd rounds run in
 * reversed order. The baseline is warmed `rounds` times TOTAL — not per
 * candidate pair. Warmups never count toward inference or early stopping.
 */
export function warmupRounds(
  buildIds: readonly string[],
  rounds: number,
): string[][] {
  return Array.from({ length: rounds }, (_, round) => {
    const rotated = buildIds.map(
      (_, index) => buildIds[(index + round) % buildIds.length]!,
    );
    return round % 2 === 1 ? rotated.reverse() : rotated;
  });
}

export interface ScheduledSlot {
  slotId: string;
  kind: "warmup" | "scheduled";
  blockIndex: number | null;
  roundIndex: number | null;
  positionIndex: number;
  buildId: string;
}

export function planScheduledSlots(manifest: {
  builds: { id: string }[];
  design: { warmupsPerBuild: number; requiredBlocks: number };
}): ScheduledSlot[] {
  const buildIds = manifest.builds.map((build) => build.id);
  const slots: ScheduledSlot[] = [];
  warmupRounds(buildIds, manifest.design.warmupsPerBuild).forEach(
    (round, roundIndex) => {
      round.forEach((buildId, positionIndex) => {
        slots.push({
          slotId: `warmup-r${roundIndex}-p${positionIndex}-${buildId}`,
          kind: "warmup",
          blockIndex: null,
          roundIndex,
          positionIndex,
          buildId,
        });
      });
    },
  );
  mirroredCyclicSchedule(buildIds, manifest.design.requiredBlocks).forEach(
    (block) => {
      block.slots.forEach((buildId, positionIndex) => {
        slots.push({
          slotId: `block${block.blockIndex}-s${positionIndex}-${buildId}`,
          kind: "scheduled",
          blockIndex: block.blockIndex,
          roundIndex: null,
          positionIndex,
          buildId,
        });
      });
    },
  );
  return slots;
}

// ---------------------------------------------------------------------------
// Manifest validation + resolution
// ---------------------------------------------------------------------------

const PROFILES = new Set(["full", "extended", "entry-color"]);
const ROLES = new Set(["baseline", "candidate", "negative-control"]);
const STATISTICS_FIXTURE_MODE = "saturated-stripes";

export function validateManifest(raw: any): string[] {
  const errors: string[] = [];
  if (raw?.schemaVersion !== 1) errors.push("schemaVersion must be 1");
  if (typeof raw?.studyId !== "string" || raw.studyId.length === 0) {
    errors.push("studyId must be a non-empty string");
  }
  if (!PROFILES.has(raw?.profile)) {
    errors.push(`profile must be one of ${[...PROFILES].join(", ")}`);
  }
  if (!Array.isArray(raw?.builds) || raw.builds.length === 0) {
    errors.push("builds must be a non-empty array");
    return errors;
  }
  const ids = new Set<string>();
  for (const build of raw.builds) {
    if (typeof build?.id !== "string" || build.id.length === 0) {
      errors.push("every build needs a non-empty id");
      continue;
    }
    if (ids.has(build.id)) errors.push(`duplicate build id: ${build.id}`);
    ids.add(build.id);
    if (!ROLES.has(build?.role)) {
      errors.push(`build ${build.id}: role must be one of ${[...ROLES].join(", ")}`);
    }
    if (typeof build?.binary !== "string" || build.binary.length === 0) {
      errors.push(`build ${build.id}: binary path required`);
    }
    const alpha = build?.expected?.morphStartAlpha;
    if (typeof alpha !== "number" || alpha < 0 || alpha > 1) {
      errors.push(`build ${build.id}: expected.morphStartAlpha must be in [0, 1]`);
    } else if (
      typeof build?.expected?.morphStartAlphaBits === "string"
      && build.expected.morphStartAlphaBits !== alphaToBitsHex(alpha)
    ) {
      errors.push(
        `build ${build.id}: expected.morphStartAlphaBits ${build.expected.morphStartAlphaBits} does not equal the f64 bits of morphStartAlpha (${alphaToBitsHex(alpha)})`,
      );
    }
  }
  if (raw?.design?.type !== "mirrored-cyclic") {
    errors.push("design.type must be mirrored-cyclic");
  }
  if (!(Number.isInteger(raw?.design?.warmupsPerBuild) && raw.design.warmupsPerBuild >= 3)) {
    errors.push("design.warmupsPerBuild must be an integer >= 3 (never auto-reduced below 3)");
  }
  if (!(Number.isInteger(raw?.design?.requiredBlocks) && raw.design.requiredBlocks >= 1)) {
    errors.push("design.requiredBlocks must be an integer >= 1");
  }
  if (typeof raw?.design?.failureOnlyEarlyStop !== "boolean") {
    errors.push("design.failureOnlyEarlyStop must be a boolean");
  }
  if (typeof raw?.fixture?.mode !== "string") {
    errors.push("fixture.mode required");
  } else if (
    raw.fixture.mode !== STATISTICS_FIXTURE_MODE
    && raw?.profile !== "entry-color"
  ) {
    // Sentinel backdrops are entry-color diagnostics only; cross-build
    // statistics REQUIRE the saturated-stripes fixture (WP9 contract).
    errors.push(
      `fixture.mode ${raw.fixture.mode} is a sentinel backdrop: statistics profiles require ${STATISTICS_FIXTURE_MODE}; use profile entry-color for sentinels`,
    );
  }
  return errors;
}

export interface ResolvedBuild {
  id: string;
  role: string;
  binary: string;
  binarySha256: string;
  expected: {
    morphStartAlpha: number;
    morphStartAlphaBits: string;
    settleDurationNs: number;
  };
}

export const DEFAULT_SETTLE_DURATION_NS = 280_000_000;

/**
 * Resolve a validated manifest: absolute binary paths, SHA-256 per binary,
 * canonical f64 alpha bits. A missing binary fails BEFORE any fixture
 * starts; duplicate SHAs are rejected unless allowDuplicateBinarySha
 * permits a deliberate A/A control.
 */
export function resolveManifest(
  raw: any,
  options: { repoRoot: string },
): { resolved: any; errors: string[] } {
  const errors: string[] = [];
  const builds: ResolvedBuild[] = [];
  for (const build of raw.builds) {
    const binaryPath = resolve(options.repoRoot, build.binary);
    if (!existsSync(binaryPath)) {
      errors.push(`build ${build.id}: binary missing: ${binaryPath}`);
      continue;
    }
    builds.push({
      id: build.id,
      role: build.role,
      binary: binaryPath,
      binarySha256: createHash("sha256")
        .update(readFileSync(binaryPath))
        .digest("hex"),
      expected: {
        morphStartAlpha: build.expected.morphStartAlpha,
        morphStartAlphaBits: alphaToBitsHex(build.expected.morphStartAlpha),
        settleDurationNs:
          build.expected.settleDurationNs ?? DEFAULT_SETTLE_DURATION_NS,
      },
    });
  }
  const shaToIds = new Map<string, string[]>();
  for (const build of builds) {
    shaToIds.set(build.binarySha256, [
      ...(shaToIds.get(build.binarySha256) ?? []),
      build.id,
    ]);
  }
  for (const [sha, buildIds] of shaToIds) {
    if (buildIds.length > 1 && raw.allowDuplicateBinarySha !== true) {
      errors.push(
        `builds ${buildIds.join(", ")} share binary sha256 ${sha} — accidental mislabeled rung? Set allowDuplicateBinarySha: true only for a deliberate A/A control`,
      );
    }
  }
  return {
    resolved: {
      ...raw,
      builds,
      resolvedAt: new Date().toISOString(),
      repoRoot: options.repoRoot,
    },
    errors,
  };
}

// ---------------------------------------------------------------------------
// Storage preflight
// ---------------------------------------------------------------------------

export const STORAGE_HEADROOM_FACTOR = 1.25;
export const STORAGE_FLOOR_BYTES = 5 * 1024 ** 3;

export function requiredFreeStorageBytes(
  p95LifecycleBytes: number,
  plannedCaptureSlots: number,
): number {
  return Math.ceil(
    p95LifecycleBytes * plannedCaptureSlots * STORAGE_HEADROOM_FACTOR
      + STORAGE_FLOOR_BYTES,
  );
}

export function p95Bytes(sizes: number[]): number {
  if (sizes.length === 0) return 0;
  const sorted = [...sizes].sort((a, b) => a - b);
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil(0.95 * sorted.length) - 1),
  );
  return sorted[index];
}

// ---------------------------------------------------------------------------
// Attempt / block semantics
// ---------------------------------------------------------------------------

export interface AttemptRow {
  attemptId: string;
  slotId: string;
  blockIndex: number | null;
  buildId: string;
  disposition: string;
  loadEligible: boolean;
  thermalEligible: boolean;
  binaryHashStable: boolean;
  evaluable: boolean;
}

export const INVALID_ATTEMPT_DISPOSITIONS = new Set([
  "INVALID_INTERFERENCE",
  "INVALID_OBSERVER",
  "INVALID_SETUP",
]);

export function attemptSatisfiesSlot(attempt: AttemptRow): boolean {
  return (
    attempt.loadEligible
    && attempt.thermalEligible
    && attempt.binaryHashStable
    && attempt.evaluable
    && !INVALID_ATTEMPT_DISPOSITIONS.has(attempt.disposition)
  );
}

export interface BlockValidity {
  blockIndex: number;
  valid: boolean;
  reasons: string[];
}

/**
 * A mirrored block is valid for paired inference ONLY when every scheduled
 * slot is satisfied by an eligible evaluable attempt. One invalid slot
 * invalidates the WHOLE block (paired inference would otherwise compare a
 * clean run against a polluted temporal neighborhood). Evaluable product
 * FAILURES remain evaluable — they invalidate nothing and can trigger
 * failure-only early stopping after grading.
 */
export function blockInferenceValidity(
  block: MirroredBlock,
  attemptsBySlot: Map<string, AttemptRow[]>,
): BlockValidity {
  const reasons: string[] = [];
  block.slots.forEach((buildId, positionIndex) => {
    const slotId = `block${block.blockIndex}-s${positionIndex}-${buildId}`;
    const attempts = attemptsBySlot.get(slotId) ?? [];
    const satisfied = attempts.some(attemptSatisfiesSlot);
    if (!satisfied) {
      const last = attempts[attempts.length - 1];
      reasons.push(
        last
          ? `slot ${slotId}: last attempt ${last.attemptId} not eligible (disposition=${last.disposition}, load=${last.loadEligible}, thermal=${last.thermalEligible}, hashStable=${last.binaryHashStable}, evaluable=${last.evaluable})`
          : `slot ${slotId}: no attempt recorded`,
      );
    }
  });
  return { blockIndex: block.blockIndex, valid: reasons.length === 0, reasons };
}

export interface RetryBlock extends MirroredBlock {
  retryOfBlockId: number;
}

export function scheduleRetryBlock(
  block: MirroredBlock,
  nextBlockIndex: number,
): RetryBlock {
  return {
    blockIndex: nextBlockIndex,
    forward: [...block.forward],
    reverse: [...block.reverse],
    slots: [...block.slots],
    retryOfBlockId: block.blockIndex,
  };
}

// ---------------------------------------------------------------------------
// CLI orchestration
// ---------------------------------------------------------------------------

function cliArg(name: string, fallback?: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

async function runCommand(command: string[], timeoutMs = 60_000) {
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const timer = setTimeout(() => child.kill(), timeoutMs);
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  clearTimeout(timer);
  return { stdout, stderr, exitCode };
}

async function measureStoragePreflight(repoRoot: string, plannedSlots: number) {
  const smokeRoot = join(
    repoRoot,
    ".artifacts/glass-entry-abba/smoke3-2026-07-25",
  );
  const sizes: number[] = [];
  if (existsSync(smokeRoot)) {
    for (const entry of readdirSync(smokeRoot)) {
      const lifecycleDir = join(smokeRoot, entry, "lifecycle");
      if (!existsSync(lifecycleDir)) continue;
      const du = await runCommand(["du", "-sk", lifecycleDir]);
      const kb = Number(du.stdout.trim().split(/\s+/)[0]);
      if (Number.isFinite(kb)) sizes.push(kb * 1024);
    }
  }
  const df = await runCommand(["df", "-k", repoRoot]);
  const dfLine = df.stdout.trim().split("\n").at(-1) ?? "";
  const freeKb = Number(dfLine.split(/\s+/)[3]);
  const p95 = p95Bytes(sizes);
  const required = requiredFreeStorageBytes(p95, plannedSlots);
  const freeBytes = Number.isFinite(freeKb) ? freeKb * 1024 : 0;
  return {
    sampleCount: sizes.length,
    p95LifecycleBytes: p95,
    plannedCaptureSlots: plannedSlots,
    requiredFreeBytes: required,
    freeBytes,
    pass: freeBytes >= required,
  };
}

async function main(): Promise<number> {
  const manifestPath = cliArg("--manifest");
  const outDir = cliArg("--out");
  const dryRun = process.argv.includes("--dry-run");
  if (!manifestPath || !outDir) {
    console.error(
      "usage: glass-smoke-study.ts --manifest <path> --out <dir> [--dry-run]",
    );
    return 64;
  }
  const repoRoot = resolve(import.meta.dir, "../..");
  const raw = JSON.parse(readFileSync(resolve(manifestPath), "utf8"));
  const validationErrors = validateManifest(raw);
  if (validationErrors.length > 0) {
    console.log(
      JSON.stringify(
        { status: "INVALID_MANIFEST", errors: validationErrors },
        null,
        2,
      ),
    );
    return 2;
  }
  const { resolved, errors: resolveErrors } = resolveManifest(raw, { repoRoot });
  if (resolveErrors.length > 0) {
    // Missing builds and duplicate SHAs fail BEFORE any fixture starts.
    console.log(
      JSON.stringify(
        { status: "BLOCKED_ENVIRONMENT", errors: resolveErrors },
        null,
        2,
      ),
    );
    return 2;
  }
  const slots = planScheduledSlots(resolved);
  const captureSlots = slots.length;
  const storage = await measureStoragePreflight(repoRoot, captureSlots);
  const schedule = mirroredCyclicSchedule(
    resolved.builds.map((build: ResolvedBuild) => build.id),
    resolved.design.requiredBlocks,
  );
  const summary = {
    studyId: resolved.studyId,
    profile: resolved.profile,
    buildCount: resolved.builds.length,
    plannedCaptureSlots: captureSlots,
    warmupSlots: slots.filter((slot) => slot.kind === "warmup").length,
    scheduledSlots: slots.filter((slot) => slot.kind === "scheduled").length,
    ladderRunsBeforeEarlyStop: 13 * resolved.builds.length,
    legacyPairRuns: 26 * Math.max(0, resolved.builds.length - 1),
    schedule,
    warmupRounds: warmupRounds(
      resolved.builds.map((build: ResolvedBuild) => build.id),
      resolved.design.warmupsPerBuild,
    ),
    storage,
  };
  if (dryRun) {
    console.log(JSON.stringify({ status: "DRY_RUN", ...summary }, null, 2));
    return storage.pass ? 0 : 2;
  }
  if (!storage.pass) {
    console.log(
      JSON.stringify({ status: "BLOCKED_ENVIRONMENT", storage }, null, 2),
    );
    return 2;
  }
  const outAbsolute = resolve(outDir);
  if (existsSync(outAbsolute) && readdirSync(outAbsolute).length > 0) {
    console.log(
      JSON.stringify(
        {
          status: "BLOCKED_ENVIRONMENT",
          error: `output directory not empty: ${outAbsolute} — a study never reuses a run directory`,
        },
        null,
        2,
      ),
    );
    return 2;
  }
  mkdirSync(outAbsolute, { recursive: true });
  writeFileSync(
    join(outAbsolute, "resolved-manifest.json"),
    `${JSON.stringify(resolved, null, 2)}\n`,
  );

  // Live capture orchestration: helpers once, fixture once, warmups then
  // mirrored blocks, per-attempt eligibility, retry blocks, deferred
  // grading after the fixture stops.
  const { prepareHelper } = await import(
    "../devtools/glass-native-helper-cache.ts"
  );
  const cacheDir = join(repoRoot, ".artifacts/glass-helper-cache");
  const filmstripHelper = await prepareHelper("filmstrip", { cacheDir });
  const interferenceHelper = await prepareHelper("interference", { cacheDir });
  const fixtureHelper = await prepareHelper("fixture", { cacheDir });
  const fixtureReceiptPath = join(outAbsolute, "fixture.json");
  const fixtureProcess = Bun.spawn([
    fixtureHelper.binaryPath,
    "--mode",
    resolved.fixture.mode,
    "--receipt",
    fixtureReceiptPath,
  ]);
  const fixtureDeadline = performance.now() + 15_000;
  while (!existsSync(fixtureReceiptPath)) {
    if (performance.now() > fixtureDeadline) {
      fixtureProcess.kill();
      console.log(
        JSON.stringify(
          { status: "BLOCKED_ENVIRONMENT", error: "fixture never became ready" },
          null,
          2,
        ),
      );
      return 2;
    }
    await Bun.sleep(100);
  }
  const fixtureReceipt = JSON.parse(readFileSync(fixtureReceiptPath, "utf8"));
  const session: Record<string, unknown> = {
    startedAt: new Date().toISOString(),
    fixturePid: fixtureProcess.pid,
    fixtureWindowNumber: fixtureReceipt.windowNumber ?? null,
    fixtureConfigurationSha256: fixtureReceipt.configurationSha256 ?? null,
    fixtureDisplayId: fixtureReceipt.displayID ?? null,
    fixtureMode: fixtureReceipt.mode ?? null,
  };
  if (fixtureReceipt.status !== "ready" || fixtureReceipt.mode !== resolved.fixture.mode) {
    fixtureProcess.kill();
    console.log(
      JSON.stringify(
        {
          status: "INVALID_SETUP",
          error: `fixture receipt not ready or wrong mode: ${JSON.stringify({ status: fixtureReceipt.status, mode: fixtureReceipt.mode })}`,
        },
        null,
        2,
      ),
    );
    return 2;
  }

  const themeFixture = resolved.themeFixture
    ? resolve(repoRoot, resolved.themeFixture)
    : join(repoRoot, "scripts/agentic/fixtures/glass-motion-calibration-theme.json");
  const lifecycleProbe = join(
    repoRoot,
    "scripts/devtools/glass-lifecycle-filmstrip.ts",
  );
  const attemptsPath = join(outAbsolute, "attempts.jsonl");
  const buildsById = new Map<string, ResolvedBuild>(
    resolved.builds.map((build: ResolvedBuild) => [build.id, build]),
  );
  const haltedBuilds = new Set<string>();
  const attemptsBySlot = new Map<string, AttemptRow[]>();
  let attemptCounter = 0;

  const load1 = () => os.loadavg()[0];
  const cpuSpeedLimit = async () => {
    const therm = await runCommand(["pmset", "-g", "therm"]);
    const match = therm.stdout.match(/CPU_Speed_Limit\s*=\s*(\d+)/);
    return match ? Number(match[1]) : null;
  };
  const shaOf = (path: string) =>
    createHash("sha256").update(readFileSync(path)).digest("hex");

  async function runSlot(slot: ScheduledSlot): Promise<AttemptRow> {
    const build = buildsById.get(slot.buildId)!;
    attemptCounter += 1;
    const attemptId = `attempt-${String(attemptCounter).padStart(4, "0")}`;
    const attemptDir = join(outAbsolute, attemptId);
    mkdirSync(attemptDir, { recursive: true });
    const preSha = shaOf(build.binary);
    const preLoad = load1();
    const preLimit = await cpuSpeedLimit();
    const probe = await runCommand(
      [
        "bun",
        lifecycleProbe,
        "--binary",
        build.binary,
        "--theme-fixture",
        themeFixture,
        "--out",
        join(attemptDir, "lifecycle"),
        "--profile",
        resolved.profile,
        "--analysis-mode",
        "deferred",
        "--filmstrip-helper",
        filmstripHelper.binaryPath,
        "--interference-helper",
        interferenceHelper.binaryPath,
        "--declared-start-alpha",
        String(build.expected.morphStartAlpha),
        "--declared-duration-ns",
        String(build.expected.settleDurationNs),
        "--background-fixture-receipt",
        fixtureReceiptPath,
      ],
      10 * 60_000,
    );
    const postLoad = load1();
    const postLimit = await cpuSpeedLimit();
    const postSha = shaOf(build.binary);
    const receiptPath = join(attemptDir, "lifecycle", "capture-receipt.json");
    const receipt = existsSync(receiptPath)
      ? JSON.parse(readFileSync(receiptPath, "utf8"))
      : null;
    const row: AttemptRow & Record<string, unknown> = {
      attemptId,
      slotId: slot.slotId,
      blockIndex: slot.blockIndex,
      buildId: slot.buildId,
      disposition: receipt?.disposition ?? "INVALID_OBSERVER",
      loadEligible: preLoad <= 6.0 && postLoad <= 6.0,
      thermalEligible: preLimit === 100 && postLimit === 100,
      binaryHashStable: preSha === postSha,
      evaluable: receipt?.capturePass === true,
      preLoad1: preLoad,
      postLoad1: postLoad,
      preCpuSpeedLimit: preLimit,
      postCpuSpeedLimit: postLimit,
      binarySha256Pre: preSha,
      binarySha256Post: postSha,
      probeExitCode: probe.exitCode,
      kind: slot.kind,
    };
    appendFileSync(attemptsPath, `${JSON.stringify(row)}\n`);
    attemptsBySlot.set(slot.slotId, [
      ...(attemptsBySlot.get(slot.slotId) ?? []),
      row,
    ]);
    if (!row.binaryHashStable) {
      // A changed artifact invalidates the run and halts that build.
      haltedBuilds.add(slot.buildId);
    }
    return row;
  }

  let exitStatus = 0;
  try {
    const warmupSlots = slots.filter((slot) => slot.kind === "warmup");
    for (const slot of warmupSlots) {
      if (haltedBuilds.has(slot.buildId)) continue;
      await runSlot(slot);
    }
    const baseBlocks = mirroredCyclicSchedule(
      resolved.builds.map((build: ResolvedBuild) => build.id),
      resolved.design.requiredBlocks,
    );
    const pendingBlocks: (MirroredBlock | RetryBlock)[] = [...baseBlocks];
    const completedBlocks: BlockValidity[] = [];
    const maxRetries = resolved.design.maxBlockRetries ?? 2;
    let nextBlockIndex = baseBlocks.length;
    const retryCounts = new Map<number, number>();
    while (pendingBlocks.length > 0) {
      const block = pendingBlocks.shift()!;
      if ([...haltedBuilds].some((id) => block.slots.includes(id))) {
        completedBlocks.push({
          blockIndex: block.blockIndex,
          valid: false,
          reasons: [`build halted mid-session: ${[...haltedBuilds].join(", ")}`],
        });
        continue;
      }
      for (let position = 0; position < block.slots.length; position += 1) {
        const buildId = block.slots[position];
        await runSlot({
          slotId: `block${block.blockIndex}-s${position}-${buildId}`,
          kind: "scheduled",
          blockIndex: block.blockIndex,
          roundIndex: null,
          positionIndex: position,
          buildId,
        });
      }
      const validity = blockInferenceValidity(block, attemptsBySlot);
      completedBlocks.push(validity);
      if (!validity.valid) {
        const origin = (block as RetryBlock).retryOfBlockId ?? block.blockIndex;
        const used = retryCounts.get(origin) ?? 0;
        if (used < maxRetries) {
          retryCounts.set(origin, used + 1);
          pendingBlocks.push(scheduleRetryBlock(block, nextBlockIndex));
          nextBlockIndex += 1;
        }
      }
    }
    writeFileSync(
      join(outAbsolute, "blocks.json"),
      `${JSON.stringify(completedBlocks, null, 2)}\n`,
    );
  } finally {
    // Stop the fixture immediately after capture work, before grading.
    fixtureProcess.kill();
    session.finishedCaptureAt = new Date().toISOString();
  }

  // Deferred grading after the display session ends (post-session grading
  // is the default until the WP5 grading-placement trial authorizes
  // per-block grading on this host).
  const gradeAll = join(repoRoot, "scripts/agentic/glass-lifecycle-grade-all.py");
  const gradedAttempts: Record<string, number> = {};
  for (const [slotId, rows] of attemptsBySlot) {
    for (const row of rows) {
      const captureReceipt = join(
        outAbsolute,
        String(row.attemptId),
        "lifecycle",
        "capture-receipt.json",
      );
      if (!existsSync(captureReceipt)) continue;
      const grade = await runCommand(
        [
          "python3",
          gradeAll,
          "--receipt",
          captureReceipt,
          "--out",
          join(outAbsolute, String(row.attemptId), "grades"),
        ],
        10 * 60_000,
      );
      gradedAttempts[`${slotId}/${row.attemptId}`] = grade.exitCode ?? -1;
    }
  }
  writeFileSync(
    join(outAbsolute, "session.json"),
    `${JSON.stringify({ ...session, ...summary, gradedAttempts }, null, 2)}\n`,
  );
  console.log(
    JSON.stringify(
      { status: "CAPTURED", out: outAbsolute, ...summary },
      null,
      2,
    ),
  );
  return exitStatus;
}

if (import.meta.main) {
  process.exit(await main());
}
