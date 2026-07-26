/**
 * System + runtime-contract telemetry for glass smoke captures
 * (glass-smoke-harness-max-info WP6).
 *
 * Four responsibilities, all fail-closed:
 *
 * 1. Runtime contract: parse the instrumentation-only
 *    `event=glass_morph ... phase=enter` log line (start_alpha_bits,
 *    settle_duration_ns, configured_at_host_time_ns,
 *    expected_settle_deadline_ns) and compare it against the DECLARED build
 *    configuration. A mismatch is INVALID_SETUP — the labeled artifact is
 *    not the artifact the study claims to measure.
 * 2. System sampling: a low-overhead in-process sampler (os.loadavg /
 *    os.freemem every 250 ms) plus scenario-boundary and pre/post command
 *    snapshots (vm_stat, vm.swapusage, ps, pmset -g therm,
 *    memory_pressure -Q). Unknown or unsupported parses are recorded as
 *    "unavailable" — they NEVER silently become "normal".
 * 3. GPU capability probe: bounded, never a gate, never sudo-prompting.
 * 4. Interference statistics: promote the full classification into run rows
 *    and bucket timestamped interference events into scenario intervals.
 */

import os from "node:os";
import { createHash } from "node:crypto";

// ---------------------------------------------------------------------------
// Runtime contract
// ---------------------------------------------------------------------------

export interface DeclaredMorphContract {
  /** The alpha the labeled build is DECLARED to start its enter morph at. */
  declaredMorphStartAlpha: number;
  /** Expected settle duration in nanoseconds (0.28 s => 280_000_000). */
  expectedDurationNs: number;
}

export interface ParsedMorphEnterLog {
  windowName: string;
  variant: string;
  observedMorphStartAlpha: number;
  observedMorphStartAlphaBits: string;
  configuredDurationNs: number;
  configuredAtHostTimeNs: number;
  expectedSettleDeadlineNs: number;
  rawLine: string;
}

const MORPH_ENTER_PATTERN =
  /event=glass_morph window=(?<window>.+?) variant=(?<variant>\S+) phase=enter [^\n]*?start_alpha=(?<alpha>[\d.]+) start_alpha_bits=(?<bits>[0-9a-f]{16}) settle_duration_ns=(?<duration>\d+) configured_at_host_time_ns=(?<configured>\d+) expected_settle_deadline_ns=(?<deadline>\d+)/g;

export function alphaFromBits(bitsHex: string): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(`0x${bitsHex}`), false);
  return view.getFloat64(0, false);
}

export function alphaToBitsHex(alpha: number): string {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, alpha, false);
  return view.getBigUint64(0, false).toString(16).padStart(16, "0");
}

/**
 * Parse every instrumented enter-morph line in a log. Lines WITHOUT the
 * instrumentation fields do not match — an old binary yields zero parses,
 * which the contract check reports as a hard failure (never a silent pass).
 */
export function parseMorphEnterLogs(logText: string): ParsedMorphEnterLog[] {
  const rows: ParsedMorphEnterLog[] = [];
  for (const match of logText.matchAll(MORPH_ENTER_PATTERN)) {
    const groups = match.groups!;
    rows.push({
      windowName: groups.window,
      variant: groups.variant,
      observedMorphStartAlpha: alphaFromBits(groups.bits),
      observedMorphStartAlphaBits: groups.bits,
      configuredDurationNs: Number(groups.duration),
      configuredAtHostTimeNs: Number(groups.configured),
      expectedSettleDeadlineNs: Number(groups.deadline),
      rawLine: match[0],
    });
  }
  return rows;
}

export interface RuntimeContractResult {
  declaredMorphStartAlpha: number;
  declaredMorphStartAlphaBits: string;
  observedMorphStartAlpha: number | null;
  observedMorphStartAlphaBits: string | null;
  configuredDurationNs: number | null;
  expectedDurationNs: number;
  alphaBitsMatch: boolean;
  durationMatch: boolean;
  observedLineCount: number;
  disposition: "EVALUABLE" | "INVALID_SETUP";
  errors: string[];
  pass: boolean;
}

/**
 * Compare the declared build contract against the observed instrumented
 * lines for one window. Zero observed lines, an alpha-bits mismatch, or a
 * duration mismatch are each INVALID_SETUP: the run must be excluded (the
 * artifact is not what the study labeled it), never converted into a
 * product failure or pass.
 */
export function checkRuntimeContract(
  declared: DeclaredMorphContract,
  parsed: ParsedMorphEnterLog[],
  windowName: string,
): RuntimeContractResult {
  const declaredBits = alphaToBitsHex(declared.declaredMorphStartAlpha);
  const errors: string[] = [];
  const relevant = parsed.filter((row) => row.windowName === windowName);
  if (relevant.length === 0) {
    errors.push(
      `no instrumented enter-morph log line observed for window ${JSON.stringify(windowName)} — old binary or missing log capture`,
    );
    return {
      declaredMorphStartAlpha: declared.declaredMorphStartAlpha,
      declaredMorphStartAlphaBits: declaredBits,
      observedMorphStartAlpha: null,
      observedMorphStartAlphaBits: null,
      configuredDurationNs: null,
      expectedDurationNs: declared.expectedDurationNs,
      alphaBitsMatch: false,
      durationMatch: false,
      observedLineCount: 0,
      disposition: "INVALID_SETUP",
      errors,
      pass: false,
    };
  }
  const observed = relevant[relevant.length - 1];
  const alphaBitsMatch =
    observed.observedMorphStartAlphaBits === declaredBits;
  const durationMatch =
    observed.configuredDurationNs === declared.expectedDurationNs;
  if (!alphaBitsMatch) {
    errors.push(
      `observed morph start alpha bits ${observed.observedMorphStartAlphaBits} do not match declared ${declaredBits} (declared alpha ${declared.declaredMorphStartAlpha})`,
    );
  }
  if (!durationMatch) {
    errors.push(
      `observed settle duration ${observed.configuredDurationNs}ns does not match expected ${declared.expectedDurationNs}ns`,
    );
  }
  const distinctBits = new Set(
    relevant.map((row) => row.observedMorphStartAlphaBits),
  );
  if (distinctBits.size > 1) {
    errors.push(
      `observed ${distinctBits.size} distinct start-alpha bit patterns in one run: ${[...distinctBits].join(", ")}`,
    );
  }
  const pass = errors.length === 0;
  return {
    declaredMorphStartAlpha: declared.declaredMorphStartAlpha,
    declaredMorphStartAlphaBits: declaredBits,
    observedMorphStartAlpha: observed.observedMorphStartAlpha,
    observedMorphStartAlphaBits: observed.observedMorphStartAlphaBits,
    configuredDurationNs: observed.configuredDurationNs,
    expectedDurationNs: declared.expectedDurationNs,
    alphaBitsMatch,
    durationMatch,
    observedLineCount: relevant.length,
    disposition: pass ? "EVALUABLE" : "INVALID_SETUP",
    errors,
    pass,
  };
}

// ---------------------------------------------------------------------------
// Command runners (bounded, raw-preserving)
// ---------------------------------------------------------------------------

export interface BoundedCommandResult {
  command: string[];
  exitCode: number | null;
  stdout: string;
  stderr: string;
  stdoutSha256: string;
  timedOut: boolean;
}

export async function runBounded(
  command: string[],
  timeoutMs = 10_000,
): Promise<BoundedCommandResult> {
  try {
    const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
    const timer = setTimeout(() => child.kill(), timeoutMs);
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    clearTimeout(timer);
    return {
      command,
      exitCode,
      stdout,
      stderr,
      stdoutSha256: createHash("sha256").update(stdout).digest("hex"),
      timedOut: exitCode === null,
    };
  } catch (error) {
    return {
      command,
      exitCode: null,
      stdout: "",
      stderr: String(error),
      stdoutSha256: createHash("sha256").update("").digest("hex"),
      timedOut: false,
    };
  }
}

// ---------------------------------------------------------------------------
// Parsers — every unknown shape resolves to "unavailable", never "normal"
// ---------------------------------------------------------------------------

export function parseMemoryPressure(stdout: string): string {
  const match = stdout.match(
    /System-wide memory free percentage:\s*(\d+)%/,
  );
  const level = stdout.match(/memory pressure(?: status)?:?\s*(normal|warn|critical)/i);
  if (level) return level[1].toLowerCase();
  if (match) {
    // memory_pressure -Q emits only the free percentage on some builds;
    // the level line is absent. That is an unknown level, not "normal".
    return "unavailable";
  }
  return "unavailable";
}

export const MEMORY_PRESSURE_SEVERITY: Record<string, number> = {
  normal: 0,
  unavailable: 1,
  warn: 2,
  critical: 3,
};

export function worstMemoryPressure(levels: string[]): string {
  let worst = "unavailable";
  let worstRank = -1;
  for (const level of levels) {
    const rank = MEMORY_PRESSURE_SEVERITY[level] ?? 1;
    if (rank > worstRank) {
      worstRank = rank;
      worst = level in MEMORY_PRESSURE_SEVERITY ? level : "unavailable";
    }
  }
  return worst;
}

export function parseSwapUsage(stdout: string): number | null {
  // "total = 2048.00M  used = 1234.56M  free = 813.44M  (encrypted)"
  const match = stdout.match(/used\s*=\s*([\d.]+)([KMG])/);
  if (!match) return null;
  const value = Number(match[1]);
  const unit = match[2];
  const scale = unit === "K" ? 1024 : unit === "M" ? 1024 ** 2 : 1024 ** 3;
  return Math.round(value * scale);
}

export function parseCpuSpeedLimit(stdout: string): number | null {
  const match = stdout.match(/CPU_Speed_Limit\s*=\s*(\d+)/);
  return match ? Number(match[1]) : null;
}

export function parseVmStatFreeBytes(stdout: string): number | null {
  const pageSize = stdout.match(/page size of (\d+) bytes/);
  const free = stdout.match(/Pages free:\s*(\d+)/);
  if (!pageSize || !free) return null;
  return Number(free[1]) * Number(pageSize[1]);
}

export interface PsSample {
  rssBytes: number;
  virtualBytes: number;
  cpuPercent: number;
  memPercent: number;
}

export function parsePsLine(stdout: string): PsSample | null {
  // ps -o rss=,vsz=,%cpu=,%mem= -p PID → "123456 987654 12.3 4.5"
  const fields = stdout.trim().split(/\s+/);
  if (fields.length < 4) return null;
  const [rssKb, vszKb, cpu, mem] = fields.map(Number);
  if ([rssKb, vszKb, cpu, mem].some((value) => !Number.isFinite(value))) {
    return null;
  }
  return {
    rssBytes: rssKb * 1024,
    virtualBytes: vszKb * 1024,
    cpuPercent: cpu,
    memPercent: mem,
  };
}

// ---------------------------------------------------------------------------
// In-process sampler
// ---------------------------------------------------------------------------

export interface SamplerHandle {
  stop: () => SamplerSummary;
}

export interface SamplerSummary {
  sampleCount: number;
  intervalMs: number;
  load1Samples: number[];
  freeMemSamples: number[];
  load1P50: number;
  load1P95: number;
  load1Maximum: number;
  freeBytesMinimum: number;
}

export function percentile(values: number[], fraction: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil(fraction * sorted.length) - 1),
  );
  return sorted[index];
}

/**
 * Start the low-overhead sampler. os.loadavg()/os.freemem() are direct
 * syscall reads — no subprocesses — so the sampler itself cannot meaningfully
 * load the machine it measures. `intervalMs` defaults to 250 per the plan;
 * the WP6 overhead decision rule may lower it to 1000 on this host.
 */
export function startSampler(intervalMs = 250): SamplerHandle {
  const load1Samples: number[] = [];
  const freeMemSamples: number[] = [];
  const timer = setInterval(() => {
    load1Samples.push(os.loadavg()[0]);
    freeMemSamples.push(os.freemem());
  }, intervalMs);
  // Never keep the process alive just for telemetry.
  if (typeof timer.unref === "function") timer.unref();
  return {
    stop() {
      clearInterval(timer);
      return {
        sampleCount: load1Samples.length,
        intervalMs,
        load1Samples,
        freeMemSamples,
        load1P50: percentile(load1Samples, 0.5),
        load1P95: percentile(load1Samples, 0.95),
        load1Maximum: load1Samples.length ? Math.max(...load1Samples) : 0,
        freeBytesMinimum: freeMemSamples.length
          ? Math.min(...freeMemSamples)
          : 0,
      };
    },
  };
}

// ---------------------------------------------------------------------------
// Boundary + pre/post snapshots
// ---------------------------------------------------------------------------

export interface BoundarySnapshot {
  atUnixMs: number;
  label: string;
  load1: number;
  freeMemBytes: number;
  vmStatFreeBytes: number | null;
  swapUsedBytes: number | null;
  app: PsSample | null;
  raw: Record<string, BoundedCommandResult>;
}

export async function captureBoundarySnapshot(
  label: string,
  appPid: number | null,
): Promise<BoundarySnapshot> {
  const commands: Record<string, string[]> = {
    vmStat: ["vm_stat"],
    swapUsage: ["sysctl", "-n", "vm.swapusage"],
  };
  if (appPid !== null) {
    commands.ps = [
      "ps",
      "-o",
      "rss=,vsz=,%cpu=,%mem=",
      "-p",
      String(appPid),
    ];
  }
  const raw: Record<string, BoundedCommandResult> = {};
  for (const [key, command] of Object.entries(commands)) {
    raw[key] = await runBounded(command);
  }
  return {
    atUnixMs: Date.now(),
    label,
    load1: os.loadavg()[0],
    freeMemBytes: os.freemem(),
    vmStatFreeBytes: raw.vmStat ? parseVmStatFreeBytes(raw.vmStat.stdout) : null,
    swapUsedBytes: raw.swapUsage ? parseSwapUsage(raw.swapUsage.stdout) : null,
    app: raw.ps ? parsePsLine(raw.ps.stdout) : null,
    raw,
  };
}

export interface EdgeSnapshot {
  atUnixMs: number;
  label: string;
  therm: BoundedCommandResult;
  cpuSpeedLimit: number | null;
  memoryPressureRaw: BoundedCommandResult;
  memoryPressure: string;
  uptime: BoundedCommandResult;
  topProcesses: BoundedCommandResult;
}

export async function captureEdgeSnapshot(label: string): Promise<EdgeSnapshot> {
  const therm = await runBounded(["pmset", "-g", "therm"]);
  const memoryPressureRaw = await runBounded(["memory_pressure", "-Q"]);
  const uptime = await runBounded(["uptime"]);
  const topProcesses = await runBounded([
    "ps",
    "-Ao",
    "pid,pcpu,pmem,comm",
    "-r",
  ]);
  topProcesses.stdout = topProcesses.stdout
    .split("\n")
    .slice(0, 21)
    .join("\n");
  return {
    atUnixMs: Date.now(),
    label,
    therm,
    cpuSpeedLimit: parseCpuSpeedLimit(therm.stdout),
    memoryPressureRaw,
    memoryPressure: parseMemoryPressure(memoryPressureRaw.stdout),
    uptime,
    topProcesses,
  };
}

// ---------------------------------------------------------------------------
// Aggregation into the plan's load / thermal / memory blocks
// ---------------------------------------------------------------------------

export const BOUNDARY_LOAD_LIMIT = 6.0;

export function summarizeTelemetry(options: {
  pre: EdgeSnapshot;
  post: EdgeSnapshot;
  sampler: SamplerSummary;
  boundaries: BoundarySnapshot[];
  preLoad1: number;
  postLoad1: number;
}) {
  const { pre, post, sampler, boundaries, preLoad1, postLoad1 } = options;
  const speedLimits = [pre.cpuSpeedLimit, post.cpuSpeedLimit].filter(
    (value): value is number => value !== null,
  );
  const pressureLevels = [pre.memoryPressure, post.memoryPressure];
  const swapValues = boundaries
    .map((row) => row.swapUsedBytes)
    .filter((value): value is number => value !== null);
  const appRss = boundaries
    .map((row) => row.app?.rssBytes)
    .filter((value): value is number => typeof value === "number");
  const appVirtual = boundaries
    .map((row) => row.app?.virtualBytes)
    .filter((value): value is number => typeof value === "number");
  const worstPressure = worstMemoryPressure(pressureLevels);
  return {
    load: {
      preLoad1,
      postLoad1,
      sampleCount: sampler.sampleCount,
      load1P50: sampler.load1P50,
      load1P95: sampler.load1P95,
      load1Maximum: sampler.load1Maximum,
      boundaryPass:
        preLoad1 <= BOUNDARY_LOAD_LIMIT && postLoad1 <= BOUNDARY_LOAD_LIMIT,
    },
    thermal: {
      preCpuSpeedLimit: pre.cpuSpeedLimit,
      postCpuSpeedLimit: post.cpuSpeedLimit,
      minimumCpuSpeedLimit: speedLimits.length
        ? Math.min(...speedLimits)
        : null,
      // Unknown thermal state fails closed: without a parsed CPU speed
      // limit we cannot assert the absence of throttling.
      pass: speedLimits.length === 2 && speedLimits.every((v) => v >= 100),
    },
    memory: {
      pressurePre: pre.memoryPressure,
      pressurePost: post.memoryPressure,
      pressureWorst: worstPressure,
      freeBytesMinimum: sampler.freeBytesMinimum,
      swapUsedBytesPre: swapValues.length ? swapValues[0] : null,
      swapUsedBytesPost: swapValues.length
        ? swapValues[swapValues.length - 1]
        : null,
      appRssBytesMaximum: appRss.length ? Math.max(...appRss) : null,
      appVirtualBytesMaximum: appVirtual.length
        ? Math.max(...appVirtual)
        : null,
      // Only a POSITIVELY observed critical state invalidates; unavailable
      // parses are recorded but do not fabricate an invalidation or a pass.
      telemetryPass: worstPressure !== "critical",
      pressureObservationQuality: pressureLevels.includes("unavailable")
        ? "partial"
        : "complete",
    },
  };
}

// ---------------------------------------------------------------------------
// GPU capability probe — bounded, never a gate
// ---------------------------------------------------------------------------

export type GpuTelemetryStatus =
  | "available"
  | "unsupported"
  | "permission-denied"
  | "parse-unknown";

export async function probeGpuTelemetry(options?: {
  allowPowermetrics?: boolean;
}) {
  const ioreg = await runBounded([
    "ioreg",
    "-r",
    "-d",
    "1",
    "-c",
    "IOAccelerator",
  ]);
  let status: GpuTelemetryStatus;
  let utilizationPercent: number | null = null;
  if (ioreg.exitCode !== 0) {
    status = "unsupported";
  } else {
    const match = ioreg.stdout.match(/"Device Utilization %"\s*=\s*(\d+)/);
    if (match) {
      status = "available";
      utilizationPercent = Number(match[1]);
    } else if (ioreg.stdout.trim().length === 0) {
      status = "unsupported";
    } else {
      status = "parse-unknown";
    }
  }
  let powermetrics: BoundedCommandResult | null = null;
  if (options?.allowPowermetrics) {
    // sudo -n never prompts; a password requirement exits non-zero.
    powermetrics = await runBounded(
      ["sudo", "-n", "powermetrics", "-n", "1", "-i", "100", "--samplers", "gpu_power"],
      15_000,
    );
    if (powermetrics.exitCode !== 0) {
      if (/password|sudo/i.test(powermetrics.stderr)) {
        status = status === "available" ? status : "permission-denied";
      }
    }
  }
  return {
    status,
    utilizationPercent,
    ioregSha256: ioreg.stdoutSha256,
    ioregExitCode: ioreg.exitCode,
    powermetricsExitCode: powermetrics?.exitCode ?? null,
    gate: false as const,
  };
}

// ---------------------------------------------------------------------------
// Interference statistics + scenario attribution
// ---------------------------------------------------------------------------

export interface InterferenceEventRow {
  kind: string;
  atUnixMs: number;
  atUptimeNs: number;
  sampleIndex: number;
  eventType?: string;
}

export interface ScenarioInterval {
  name: string;
  startUnixMs: number;
  endUnixMs: number;
}

/**
 * Promote one run's interference classification into study-level statistics
 * and, when the monitor emitted timestamped events, bucket them into the
 * scenario intervals that were capturing at the time. A receipt WITHOUT
 * timestamped events explicitly rejects scenario attribution — run-level
 * classification is retained and nothing is guessed.
 */
export function interferenceStatistics(
  receipt: any,
  scenarioIntervals: ScenarioInterval[],
) {
  const causes = {
    untaggedInput: Number(receipt?.untaggedInputCount ?? 0),
    frontmostAppChange: receipt?.frontmostAppChanged === true ? 1 : 0,
    pointerDeviation:
      Number(receipt?.pointerDeviationPx ?? 0) > 1 ? 1 : 0,
    targetMovedExternally:
      receipt?.targetMovedExternally === true ? 1 : 0,
  };
  const events: InterferenceEventRow[] = Array.isArray(receipt?.events)
    ? receipt.events
    : [];
  const timestampsSupported = receipt?.eventTimestampsSupported === true;
  let scenarioAttribution: Record<string, number> | null = null;
  let unattributedEventCount = 0;
  if (timestampsSupported) {
    scenarioAttribution = Object.fromEntries(
      scenarioIntervals.map((interval) => [interval.name, 0]),
    );
    for (const event of events) {
      const interval = scenarioIntervals.find(
        (row) =>
          event.atUnixMs >= row.startUnixMs && event.atUnixMs <= row.endUnixMs,
      );
      if (interval) {
        scenarioAttribution[interval.name] += 1;
      } else {
        unattributedEventCount += 1;
      }
    }
  }
  return {
    causes,
    eventCount: events.length,
    droppedEventCount: Number(receipt?.droppedEventCount ?? 0),
    scenarioAttributionSupported: timestampsSupported,
    scenarioAttribution,
    unattributedEventCount,
    scenarioAttributionRejectedReason: timestampsSupported
      ? null
      : "monitor receipt carries no timestamped events",
  };
}
