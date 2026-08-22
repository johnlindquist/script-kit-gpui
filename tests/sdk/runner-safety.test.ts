import { describe, expect, test } from "bun:test";
import { copyFileSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
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
  selectedFixture = fixturePath,
  environmentOverrides: Record<string, string> = {},
): Promise<RunnerResult> {
  const child = Bun.spawn({
    cmd: ["bun", "run", runnerPath, "--json", ...extraArgs, selectedFixture],
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

  test("noninteractive mode refuses system input before child execution", async () => {
    const result = await runFixture("safety-env", ["--include-system"]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stdout).toBe("");
    expect(result.stderr).toContain("SCRIPT_KIT_NONINTERACTIVE=1 prohibits real system input");
  });

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
});
