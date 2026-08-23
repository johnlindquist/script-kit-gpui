#!/usr/bin/env bun
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import {
  exitCodeForSpotlightDisposition,
  gradeSpotlightSyncBundle,
  type SpotlightFailure,
} from "./spotlight-sync-filmstrip-contract.ts";
import { newRunId } from "./glass-evidence-contract.ts";
import { assertNoninteractiveVisualProbe } from "./lib/operator-safety.ts";

type CommandReceipt = {
  argv: string[];
  startedAt: string;
  finishedAt: string;
  durationMs: number;
  exitCode: number | null;
  timedOut: boolean;
  pid: number;
  stdoutTail: string;
  stderrTail: string;
  stdoutBytes: number;
  stderrBytes: number;
  stdoutSha256: string;
  stderrSha256: string;
};

const MAX_CAPTURED_TEXT = 4_000;

function arg(name: string, fallback?: string): string | undefined {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

function hasFlag(name: string): boolean {
  return process.argv.includes(name);
}

function positiveNumberArg(name: string, fallback: number): number {
  const parsed = Number(arg(name, String(fallback)));
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive finite number`);
  }
  return parsed;
}

function rootDir(): string {
  return resolve(import.meta.dir, "../..");
}

function sha256Bytes(bytes: Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function sha256File(path: string): string {
  return sha256Bytes(readFileSync(path));
}

function atomicWriteJson(path: string, value: unknown): void {
  mkdirSync(dirname(path), { recursive: true });
  const staging = `${path}.tmp-${process.pid}`;
  writeFileSync(staging, `${JSON.stringify(value, null, 2)}\n`);
  renameSync(staging, path);
}

function readJson(path: string): unknown {
  const text = readFileSync(path, "utf8");
  // Python's json module emits bare NaN/Infinity tokens (e.g.
  // glass-motion-color-metrics.py's maximumNeighboringSettledMaterialDeltaE00
  // when only one settled sample pair exists). Strict JSON.parse rejects
  // them; map the tokens to null so the grader sees "value unavailable"
  // instead of failing the whole run on transport. Only bare tokens in value
  // position are rewritten — quoted strings are untouched.
  const sanitized = text.replace(
    /(?<=[:,[\s])(?:NaN|-?Infinity)(?=[,\]}\s])/g,
    "null",
  );
  return JSON.parse(sanitized);
}

async function captureBoundedStream(
  stream: ReadableStream<Uint8Array>,
): Promise<{ tail: string; bytes: number; sha256: string }> {
  const hash = createHash("sha256");
  let bytes = 0;
  let retained = Buffer.alloc(0);
  const reader = stream.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    hash.update(value);
    bytes += value.byteLength;
    retained = Buffer.concat([retained, Buffer.from(value)]);
    if (retained.byteLength > MAX_CAPTURED_TEXT) {
      retained = retained.subarray(retained.byteLength - MAX_CAPTURED_TEXT);
    }
  }
  return {
    tail: retained.toString("utf8").trim(),
    bytes,
    sha256: hash.digest("hex"),
  };
}

async function runBounded(
  argv: string[],
  options: {
    timeoutMs: number;
    cwd: string;
    env?: Record<string, string>;
  },
): Promise<CommandReceipt> {
  const startedAt = new Date().toISOString();
  const startedMs = performance.now();
  const child = Bun.spawn(argv, {
    cwd: options.cwd,
    env: { ...process.env, ...(options.env ?? {}) },
    stdout: "pipe",
    stderr: "pipe",
  });
  let timedOut = false;
  let escalation: ReturnType<typeof setTimeout> | null = null;
  const timeout = setTimeout(() => {
    timedOut = true;
    try {
      child.kill();
    } catch {
      // The process may already have exited.
    }
    escalation = setTimeout(() => {
      try {
        child.kill(9);
      } catch {
        // The process may already have exited.
      }
    }, 1_000);
  }, options.timeoutMs);
  const [stdout, stderr, exitCode] = await Promise.all([
    captureBoundedStream(child.stdout),
    captureBoundedStream(child.stderr),
    child.exited,
  ]);
  clearTimeout(timeout);
  if (escalation) clearTimeout(escalation);
  return {
    argv,
    startedAt,
    finishedAt: new Date().toISOString(),
    durationMs: Number((performance.now() - startedMs).toFixed(2)),
    exitCode,
    timedOut,
    pid: child.pid,
    stdoutTail: stdout.tail,
    stderrTail: stderr.tail,
    stdoutBytes: stdout.bytes,
    stderrBytes: stderr.bytes,
    stdoutSha256: stdout.sha256,
    stderrSha256: stderr.sha256,
  };
}

function optionalPassThrough(
  argv: string[],
  flag: string,
  value: string | undefined,
): void {
  if (value) argv.push(flag, resolve(value));
}

function relativeOrAbsolute(root: string, path: string): string {
  const candidate = relative(root, path);
  return candidate.startsWith("..") ? path : candidate;
}

function sourceReceipt(root: string, path: string): {
  path: string;
  sha256: string | null;
  sizeBytes: number | null;
} {
  return existsSync(path)
    ? {
      path: relativeOrAbsolute(root, path),
      sha256: sha256File(path),
      sizeBytes: statSync(path).size,
    }
    : {
      path: relativeOrAbsolute(root, path),
      sha256: null,
      sizeBytes: null,
    };
}

function setupFailure(metric: string, message: string, observed: unknown): SpotlightFailure {
  return {
    kind: "observer",
    phase: "capture",
    sequence: null,
    metric,
    observed,
    expected: "valid probe setup and child receipts",
    message,
  };
}

async function gitCommit(root: string): Promise<string | null> {
  const result = await runBounded(["git", "rev-parse", "HEAD"], {
    timeoutMs: 5_000,
    cwd: root,
  });
  return result.exitCode === 0 && /^[0-9a-f]{40}$/.test(result.stdoutTail)
    ? result.stdoutTail
    : null;
}

export async function main(): Promise<number> {
  const gradeOnly = hasFlag("--grade-only");
  if (!gradeOnly) {
    assertNoninteractiveVisualProbe("spotlight-sync-filmstrip");
  }
  const startedAt = new Date().toISOString();
  const root = rootDir();
  const outPath = resolve(
    arg("--out", ".artifacts/spotlight-sync-filmstrip/receipt.json")!,
  );
  const runId = process.env.SCRIPT_KIT_GLASS_RUN_ID ?? newRunId();
  const attemptId = `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`;
  const attemptDir = join(dirname(outPath), "attempts", attemptId);
  mkdirSync(attemptDir, { recursive: true });

  const lifecycleReceiptArg = arg("--lifecycle-receipt");
  const lifecyclePath = resolve(
    lifecycleReceiptArg ?? join(attemptDir, "lifecycle", "receipt.json"),
  );
  const entryColorPath = resolve(
    arg("--entry-color-receipt", join(attemptDir, "entry-color.json"))!,
  );
  const exitColorPath = resolve(
    arg("--exit-color-receipt", join(attemptDir, "exit-color.json"))!,
  );
  const commands: Record<string, CommandReceipt> = {};
  let binaryIdentity: Record<string, unknown> | null = null;
  let fixtureIdentity: Record<string, unknown> | null = null;
  let setupError: SpotlightFailure | null = null;

  try {
    if (!gradeOnly) {
      if (basename(lifecyclePath) !== "receipt.json") {
        throw new Error(
          "--lifecycle-receipt must end in receipt.json outside --grade-only mode",
        );
      }
      for (const [label, path] of [
        ["lifecycle", lifecyclePath],
        ["entry color", entryColorPath],
        ["exit color", exitColorPath],
      ] as const) {
        if (existsSync(path)) {
          throw new Error(`refusing pre-existing live ${label} receipt: ${path}`);
        }
      }
      const binaryArg = arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY);
      if (!binaryArg) {
        throw new Error("--binary is required outside --grade-only mode");
      }
      const binary = resolve(binaryArg);
      if (!existsSync(binary)) {
        throw new Error(`binary missing: ${binary}`);
      }
      const fixture = resolve(
        arg(
          "--theme-fixture",
          "scripts/agentic/fixtures/glass-motion-calibration-theme.json",
        )!,
      );
      if (!existsSync(fixture)) {
        throw new Error(`theme fixture missing: ${fixture}`);
      }
      const commit = await gitCommit(root);
      if (!commit) {
        throw new Error("unable to resolve the repository HEAD commit");
      }
      const binarySha256 = sha256File(binary);
      const fixtureSha256 = sha256File(fixture);
      binaryIdentity = {
        path: relativeOrAbsolute(root, binary),
        sha256: binarySha256,
        sizeBytes: statSync(binary).size,
        gitCommit: commit,
      };
      fixtureIdentity = {
        path: relativeOrAbsolute(root, fixture),
        sha256: fixtureSha256,
        sizeBytes: statSync(fixture).size,
      };
      const childEnv = {
        SCRIPT_KIT_GLASS_RUN_ID: runId,
        SCRIPT_KIT_GLASS_GIT_COMMIT: commit,
        SCRIPT_KIT_GLASS_BINARY: binary,
        SCRIPT_KIT_GLASS_BINARY_SHA256: binarySha256,
      };
      const lifecycleArgv = [
        "bun",
        resolve(root, "scripts/devtools/glass-lifecycle-filmstrip.ts"),
        "--binary",
        binary,
        "--theme-fixture",
        fixture,
        "--profile",
        "entry-color",
        "--analysis-mode",
        "inline",
        "--out",
        dirname(lifecyclePath),
      ];
      optionalPassThrough(
        lifecycleArgv,
        "--background-fixture-receipt",
        arg("--background-fixture-receipt"),
      );
      optionalPassThrough(
        lifecycleArgv,
        "--filmstrip-helper",
        arg("--filmstrip-helper"),
      );
      optionalPassThrough(
        lifecycleArgv,
        "--interference-helper",
        arg("--interference-helper"),
      );
      optionalPassThrough(
        lifecycleArgv,
        "--window-query-helper",
        arg("--window-query-helper"),
      );
      commands.lifecycle = await runBounded(lifecycleArgv, {
        timeoutMs: positiveNumberArg("--lifecycle-timeout-ms", 300_000),
        cwd: root,
        env: childEnv,
      });

      if (!existsSync(lifecyclePath)) {
        throw new Error(
          `lifecycle child did not emit ${lifecyclePath}; exit=${commands.lifecycle.exitCode}`,
        );
      }
      const colorScript = resolve(
        root,
        "scripts/agentic/glass-motion-color-metrics.py",
      );
      const entryColorArgv = [
        "python3",
        colorScript,
        "--lifecycle-receipt",
        lifecyclePath,
        "--scenario",
        "main-entry",
        "--lifecycle-phase",
        "entry",
        "--out",
        entryColorPath,
      ];
      commands.entryColor = await runBounded(entryColorArgv, {
        timeoutMs: positiveNumberArg("--color-timeout-ms", 120_000),
        cwd: root,
        env: childEnv,
      });
      const exitColorArgv = [
        "python3",
        colorScript,
        "--lifecycle-receipt",
        lifecyclePath,
        "--scenario",
        "main-entry",
        "--lifecycle-phase",
        "exit",
        "--out",
        exitColorPath,
      ];
      commands.exitColor = await runBounded(exitColorArgv, {
        timeoutMs: positiveNumberArg("--color-timeout-ms", 120_000),
        cwd: root,
        env: childEnv,
      });
      const incompleteCommand = Object.entries(commands).find(
        ([, receipt]) => receipt.timedOut || receipt.exitCode === null,
      );
      if (incompleteCommand) {
        throw new Error(
          `${incompleteCommand[0]} child did not terminate cleanly`,
        );
      }
    }

    for (const [label, path] of [
      ["lifecycle", lifecyclePath],
      ["entry color", entryColorPath],
      ["exit color", exitColorPath],
    ] as const) {
      if (!existsSync(path)) {
        throw new Error(`${label} receipt missing: ${path}`);
      }
    }
  } catch (error) {
    setupError = setupFailure(
      "probe.setup",
      error instanceof Error ? error.message : String(error),
      {
        lifecycle: sourceReceipt(root, lifecyclePath),
        entryColor: sourceReceipt(root, entryColorPath),
        exitColor: sourceReceipt(root, exitColorPath),
      },
    );
  }

  let lifecycleReceipt: unknown = null;
  let entryColorReceipt: unknown = null;
  let exitColorReceipt: unknown = null;
  try {
    lifecycleReceipt = existsSync(lifecyclePath) ? readJson(lifecyclePath) : null;
    entryColorReceipt = existsSync(entryColorPath) ? readJson(entryColorPath) : null;
    exitColorReceipt = existsSync(exitColorPath) ? readJson(exitColorPath) : null;
  } catch (error) {
    setupError ??= setupFailure(
      "probe.receipt.parse",
      error instanceof Error ? error.message : String(error),
      {
        lifecycle: sourceReceipt(root, lifecyclePath),
        entryColor: sourceReceipt(root, entryColorPath),
        exitColor: sourceReceipt(root, exitColorPath),
      },
    );
  }
  let grade = gradeSpotlightSyncBundle({
    lifecycle: lifecycleReceipt,
    entryColor: entryColorReceipt,
    exitColor: exitColorReceipt,
    lifecycleReceiptSha256: existsSync(lifecyclePath)
      ? sha256File(lifecyclePath)
      : null,
  });
  if (setupError) {
    const failures = [setupError, ...grade.failures];
    grade = {
      ...grade,
      disposition: "INVALID_OBSERVER",
      pass: false,
      failures,
      firstFailure: failures[0]!,
      observerFailureCount: grade.observerFailureCount + 1,
    };
  }

  const redReceiptArg = arg("--red-receipt");
  const redContrastReceipt = redReceiptArg
    ? sourceReceipt(root, resolve(redReceiptArg))
    : null;
  const receipt = {
    schemaVersion: 1,
    kind: "spotlight-sync-filmstrip-receipt",
    tool: "scripts/devtools/spotlight-sync-filmstrip.ts",
    runId,
    startedAt,
    finishedAt: new Date().toISOString(),
    gradeOnly,
    identity: {
      binary: binaryIdentity,
      themeFixture: fixtureIdentity,
      implementation: {
        probeSha256: sha256File(resolve(import.meta.dir, "spotlight-sync-filmstrip.ts")),
        contractSha256: sha256File(
          resolve(import.meta.dir, "spotlight-sync-filmstrip-contract.ts"),
        ),
      },
    },
    commands,
    sourceReceipts: {
      lifecycle: sourceReceipt(root, lifecyclePath),
      entryColor: sourceReceipt(root, entryColorPath),
      exitColor: sourceReceipt(root, exitColorPath),
      redContrast: redContrastReceipt,
    },
    ...grade,
  };
  atomicWriteJson(outPath, receipt);
  console.log(JSON.stringify({
    receiptPath: outPath,
    disposition: receipt.disposition,
    pass: receipt.pass,
    firstFailure: receipt.firstFailure,
  }, null, 2));
  return exitCodeForSpotlightDisposition(receipt.disposition);
}

if (import.meta.main) {
  process.exit(await main());
}
