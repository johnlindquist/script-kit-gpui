import { describe, expect, test } from "bun:test";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const projectRoot = resolve(import.meta.dir, "../..");
const runnerPath = resolve(projectRoot, "scripts/test-runner.ts");
const fixturePath = resolve(import.meta.dir, "fixtures/runner-negative-case.ts");

interface RunnerResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  summary?: {
    total_passed: number;
    total_failed: number;
    files: Array<{
      tests: Array<{ test: string; status: string; error?: string }>;
    }>;
  };
}

async function runFixture(
  mode: string,
  extraArgs: string[] = [],
  selectedFixture: string | null = fixturePath,
  environmentOverrides: Record<string, string> = {},
): Promise<RunnerResult> {
  const child = Bun.spawn({
    cmd: [
      "bun",
      "run",
      runnerPath,
      "--json",
      ...extraArgs,
      ...(selectedFixture ? [selectedFixture] : []),
    ],
    cwd: projectRoot,
    stdout: "pipe",
    stderr: "pipe",
    env: {
      ...process.env,
      SCRIPT_KIT_NONINTERACTIVE: "1",
      SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "1",
      SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "1",
      SCRIPT_KIT_ALLOW_NATIVE_INPUT: "1",
      SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "1",
      SCRIPT_KIT_ALLOW_LIVE_AI: "1",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1",
      SCRIPT_KIT_TEST_STATUS: "1",
      INCLUDE_SYSTEM_INPUT: "1",
      SDK_TEST_TIMEOUT: "1",
      SDK_RUNNER_FAILURE_FIXTURE: mode,
      SDK_TEST_VERBOSE: "false",
      ...environmentOverrides,
    },
  });

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  const messages = stdout
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));

  return {
    exitCode,
    stdout,
    stderr,
    summary: messages.find((message) => message.type === "summary"),
  };
}

describe("SDK runner fail-closed and noninteractive contracts", () => {
  test("a process timeout cannot become a green partial pass", async () => {
    const result = await runFixture("timeout");
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.total_passed).toBe(1);
    expect(result.summary?.total_failed).toBeGreaterThan(0);
    expect(result.summary?.files[0]?.tests).toContainEqual(
      expect.objectContaining({
        test: expect.stringContaining("process timeout"),
        status: "fail",
      }),
    );
  });

  test("a timed-out SDK script terminates its entire owned subprocess group", async () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-sdk-descendant-"));
    const pidPath = join(root, "child.pid");
    let descendant = 0;
    try {
      const started = performance.now();
      const result = await runFixture("grandchild-timeout", [], fixturePath, {
        SDK_RUNNER_DESCENDANT_PID_PATH: pidPath,
      });
      const elapsed = performance.now() - started;
      descendant = Number.parseInt(readFileSync(pidPath, "utf8"), 10);
      let alive = true;
      try {
        process.kill(descendant, 0);
      } catch {
        alive = false;
      }
      expect(result.exitCode).not.toBe(0);
      expect(elapsed).toBeLessThan(2_300);
      expect(alive).toBe(false);
    } finally {
      if (!descendant && existsSync(pidPath)) {
        descendant = Number.parseInt(readFileSync(pidPath, "utf8"), 10);
      }
      if (descendant > 0) {
        try { process.kill(descendant, "SIGKILL"); } catch {}
      }
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("a nonzero exit cannot become a green partial pass", async () => {
    const result = await runFixture("nonzero");
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.total_passed).toBe(1);
    expect(result.summary?.total_failed).toBeGreaterThan(0);
  });

  test("unknown blocked outcomes fail instead of disappearing", async () => {
    const result = await runFixture("invalid-status");
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.files[0]?.tests).toContainEqual(
      expect.objectContaining({
        status: "fail",
        error: "Unrecognized test status: blocked",
      }),
    );
  });

  test("running outcomes require an explicit terminal receipt", async () => {
    const result = await runFixture("missing-terminal");
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.files[0]?.tests).toContainEqual(
      expect.objectContaining({
        test: "never-completed [missing terminal result]",
        status: "fail",
      }),
    );
  });

  test("a later passing message cannot erase the same script's real prior failure", async () => {
    const result = await runFixture("fail-then-pass");
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.total_failed).toBeGreaterThan(0);
    expect(result.summary?.files[0]?.tests).toContainEqual(
      expect.objectContaining({
        test: expect.stringContaining("contradictory-terminal"),
        status: "fail",
      }),
    );
  });

  test("a terminal script receipt cannot be reopened as running", async () => {
    const result = await runFixture("terminal-then-running");
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.files[0]?.tests).toContainEqual(
      expect.objectContaining({
        test: expect.stringContaining("reopened-terminal"),
        status: "fail",
      }),
    );
  });

  test("a passing script receipt cannot carry a real error payload", async () => {
    const result = await runFixture("pass-with-error");
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.total_failed).toBeGreaterThan(0);
  });

  test.each([
    ["missing-result-name", "nonempty test name"],
    ["missing-result-status", "recognized status"],
    ["malformed-result-json", "malformed SDK result"],
    ["invalid-result-timestamp", "valid timestamp"],
    ["invalid-result-duration", "nonnegative safe duration"],
    ["skip-with-error", "cannot carry an error"],
  ])("malformed script outcome %s cannot disappear behind a genuine pass", async (mode, diagnostic) => {
    const result = await runFixture(mode);
    expect(result.exitCode).not.toBe(0);
    expect(result.summary?.total_failed).toBeGreaterThan(0);
    expect(result.summary?.files[0]?.tests).toContainEqual(
      expect.objectContaining({ status: "fail", error: expect.stringContaining(diagnostic) }),
    );
  });

  test("noninteractive mode refuses system input before child execution", async () => {
    const result = await runFixture("safety-env", ["--include-system"]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stdout).toBe("");
    expect(result.stderr).toContain("SCRIPT_KIT_NONINTERACTIVE=1 prohibits real system input");
  });

  test.each(["", "2", "true", "-1"])(
    "malformed noninteractive authority %s refuses before any SDK script can execute",
    async (mode) => {
      const result = await runFixture("safety-env", [], fixturePath, {
        SCRIPT_KIT_NONINTERACTIVE: mode,
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("SCRIPT_KIT_NONINTERACTIVE must be 0 or 1");
    },
  );

  test("an explicit test filename cannot bypass the system-input exclusion", async () => {
    const result = await runFixture(
      "safety-env",
      [],
      resolve(import.meta.dir, "test-system.ts"),
    );
    expect(result.exitCode).not.toBe(0);
    expect(result.stdout).toBe("");
    expect(result.stderr).toContain("Refusing system-input test test-system.ts");
  });

  test("a harmless-looking symlink cannot bypass the system-input exclusion", async () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-sdk-symlink-"));
    const protectedFixture = join(root, "test-system.ts");
    const disguisedFixture = join(root, "ordinary-sdk-case.ts");
    try {
      // The synthetic protected target never performs actual OS input, so the
      // regression remains safe even while demonstrating the former bypass.
      copyFileSync(fixturePath, protectedFixture);
      symlinkSync(protectedFixture, disguisedFixture);
      const result = await runFixture("safety-env", [], disguisedFixture);

      expect(result.exitCode).not.toBe(0);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("Refusing system-input test test-system.ts");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("an unreviewed absolute external script cannot execute in noninteractive mode", async () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-sdk-external-owner-"));
    const external = join(root, "ordinary-sdk-case.ts");
    const sentinel = join(root, "executed.txt");
    writeFileSync(
      external,
      `await Bun.write(${JSON.stringify(sentinel)}, "executed");\n` +
      'console.log(JSON.stringify({test:"foreign-owner",status:"pass"}));\n',
    );
    try {
      const result = await runFixture("safety-env", [], external);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain("reviewed tests/sdk owner");
      expect(existsSync(sentinel)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("automatic discovery refuses an SDK-shaped symlink to an unreviewed external script", async () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-sdk-discovery-owner-"));
    const external = join(root, "foreign-owner.ts");
    const sentinel = join(root, "executed.txt");
    const discoveredName = `test-unreviewed-owner-${process.pid}-${Date.now()}.ts`;
    const discoveredPath = join(import.meta.dir, discoveredName);
    writeFileSync(
      external,
      `await Bun.write(${JSON.stringify(sentinel)}, "executed");\n` +
      'console.log(JSON.stringify({test:"foreign-owner",status:"pass",timestamp:new Date().toISOString()}));\n',
    );
    symlinkSync(external, discoveredPath);
    try {
      const result = await runFixture("safety-env", ["--filter", discoveredName], null);
      expect(result.exitCode).not.toBe(0);
      expect(existsSync(sentinel)).toBe(false);
      expect(result.stderr).toContain("reviewed tests/sdk owner");
    } finally {
      rmSync(discoveredPath, { force: true });
      rmSync(root, { recursive: true, force: true });
    }
  });

  test.each(["stdout-flood", "stderr-flood"])(
    "unbounded %s cannot consume memory or outlive the reviewed output budget",
    async (mode) => {
      const started = performance.now();
      const result = await runFixture(mode, [], fixturePath, {
        SDK_TEST_MAX_OUTPUT_BYTES: "1024",
        SDK_TEST_TIMEOUT: "5",
      });
      expect(performance.now() - started).toBeLessThan(2_000);
      expect(result.exitCode).not.toBe(0);
      expect(result.summary?.files[0]?.tests).toContainEqual(
        expect.objectContaining({
          status: "fail",
          error: expect.stringContaining("exceeds the 1024-byte safety budget"),
        }),
      );
    },
  );

  test.each(["0", "-1", "1.5", "9007199254740992"])(
    "invalid SDK output budget %s refuses before starting any child",
    async (budget) => {
      const result = await runFixture("safety-env", [], fixturePath, {
        SDK_TEST_MAX_OUTPUT_BYTES: budget,
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("SDK_TEST_MAX_OUTPUT_BYTES must be a positive safe integer");
    },
  );

  test("oversized SDK output budgets refuse before starting any child", async () => {
    const result = await runFixture("safety-env", [], fixturePath, {
      SDK_TEST_MAX_OUTPUT_BYTES: "8388609",
    });
    expect(result.exitCode).not.toBe(0);
    expect(result.stdout).toBe("");
    expect(result.stderr).toContain("SDK_TEST_MAX_OUTPUT_BYTES exceeds the eight-megabyte safety ceiling");
  });

  test("runner overrides inherited screen, input, and live-AI opt-ins", async () => {
    const result = await runFixture("safety-env");
    expect(result.exitCode).toBe(0);
    expect(result.summary?.total_passed).toBe(1);
    expect(result.summary?.total_failed).toBe(0);
  });

  test("noninteractive SDK workers default to two while preserving an explicit override", async () => {
    const bounded = await runFixture("safety-env", [], fixturePath, {
      SDK_TEST_CONCURRENCY: "",
      SDK_RUNNER_EXPECTED_CONCURRENCY: "2",
    });
    expect(bounded.exitCode).toBe(0);
    expect(bounded.summary?.total_passed).toBe(1);

    const explicit = await runFixture("safety-env", [], fixturePath, {
      SDK_TEST_CONCURRENCY: "3",
      SDK_RUNNER_EXPECTED_CONCURRENCY: "3",
    });
    expect(explicit.exitCode).toBe(0);
    expect(explicit.summary?.total_passed).toBe(1);
  });

  test.each(["0", "-1", "1.5", "invalid", "2workers"])(
    "invalid SDK worker count %s refuses before any child can spin or launch",
    async (concurrency) => {
      const result = await runFixture("safety-env", [], fixturePath, {
        SDK_TEST_CONCURRENCY: concurrency,
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain(
        "SDK_TEST_CONCURRENCY must be a positive safe integer",
      );
    },
  );

  test.each(["0", "-1", "1.5", "invalid", "2seconds"])(
    "invalid SDK timeout %s refuses before any test child starts",
    async (timeout) => {
      const result = await runFixture("safety-env", [], fixturePath, {
        SDK_TEST_TIMEOUT: timeout,
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain(
        "SDK_TEST_TIMEOUT must be a positive safe integer",
      );
    },
  );

  test.each(["2147484", "9007199254740991"])(
    "overflowing SDK timeout %s refuses before JavaScript can truncate its timer",
    async (timeout) => {
      const result = await runFixture("safety-env", [], fixturePath, {
        SDK_TEST_TIMEOUT: timeout,
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("SDK_TEST_TIMEOUT exceeds the supported timer range");
    },
  );

  test.each(["9", "64", "9007199254740991"])(
    "unbounded SDK worker count %s refuses before any script can start",
    async (concurrency) => {
      const result = await runFixture("safety-env", [], fixturePath, {
        SDK_TEST_CONCURRENCY: concurrency,
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("SDK_TEST_CONCURRENCY exceeds the eight-worker safety ceiling");
    },
  );

  test("a missing filter value fails closed instead of expanding into an unrequested suite", async () => {
    const result = await runFixture("safety-env", ["--filter", "--parallel"]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stdout).toBe("");
    expect(result.stderr).toContain("--filter requires one non-option pattern");
  });

  test("unknown runner flags fail before script discovery", async () => {
    const result = await runFixture("safety-env", ["--quietly-expand-every-script"]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stdout).toBe("");
    expect(result.stderr).toContain("unknown SDK test-runner option");
  });
});
