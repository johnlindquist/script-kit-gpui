import { afterEach, expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Driver, ProtocolCore, type Json } from "./driver";

const roots: string[] = [];
const DIRECT_LIFECYCLE_ENV = "SCRIPT_KIT_DRIVER_LIFECYCLE_DIRECT";

// Bun 1.3.11 has two test-entry modes: the common `bun test path/to/test`
// filter form loses stdout/stderr from child processes spawned by that test,
// while `bun test ./path/to/test` consumes them correctly. Lifecycle coverage
// must not depend on command spelling, so the four process/stream tests
// delegate once to Bun's direct-path mode and propagate any failure verbatim.
function delegateToDirectPath(testName: string): boolean {
  if (process.env[DIRECT_LIFECYCLE_ENV] === "1") return false;
  const run = Bun.spawnSync(
    [
      process.execPath,
      "test",
      "./scripts/devtools/driver-lifecycle.test.ts",
      "-t",
      testName,
    ],
    {
      cwd: join(import.meta.dir, "../.."),
      env: { ...process.env, [DIRECT_LIFECYCLE_ENV]: "1" },
      stdout: "pipe",
      stderr: "pipe",
    },
  );
  if (run.exitCode !== 0) {
    throw new Error(
      `direct-path lifecycle child failed (${run.exitCode})\n${run.stdout.toString()}${run.stderr.toString()}`,
    );
  }
  return true;
}

function fakeExecutable(root: string): string {
  const executable = join(root, "fake-app.sh");
  writeFileSync(
    executable,
    `#!/bin/sh
trap 'echo "final stdout after TERM"; echo "final stderr after TERM" >&2; exit 0' TERM
echo "APP_READY|main-window-ready show=false focus=false stdin-safe"
while :; do sleep 0.05; done
`,
  );
  chmodSync(executable, 0o755);
  return executable;
}

function startupFailureExecutable(
  root: string,
  mode: "exit" | "timeout",
): { executable: string; pidPath: string } {
  const executable = join(root, `fake-startup-${mode}.js`);
  const pidPath = join(root, `fake-startup-${mode}.pid`);
  const body =
    mode === "exit"
      ? `
process.stdout.write("exit trailing stdout");
process.stderr.write("exit trailing stderr");
process.exit(17);
`
      : `
process.on("SIGTERM", () => {
  process.stdout.write("timeout trailing stdout");
  process.stderr.write("timeout trailing stderr");
  process.exit(0);
});
setInterval(() => {}, 1_000);
`;
  writeFileSync(
    executable,
    `#!/usr/bin/env bun
import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(pidPath)}, String(process.pid));
${body}`,
  );
  chmodSync(executable, 0o755);
  return { executable, pidPath };
}

function processIsAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function expectLogWriterClosed(logPath: string): void {
  const openFiles = spawnSync(
    "lsof",
    ["-p", String(process.pid), "-Fn", "--", logPath],
    {
      encoding: "utf8",
    },
  );
  expect(`${openFiles.stdout}${openFiles.stderr}`).not.toContain(logPath);
}

afterEach(() => {
  for (const root of roots.splice(0))
    rmSync(root, { recursive: true, force: true });
});

class RecordingProtocol extends ProtocolCore {
  lastPayload: Json | null = null;

  constructor() {
    super(1_000, "recording");
  }

  protected writeCommand(payload: Json): void {
    this.lastPayload = payload;
  }

  get alive(): boolean {
    return true;
  }

  async close(): Promise<void> {}

  respond(response: Json): void {
    this.handleResponse(response);
  }
}

test("ProtocolCore records the requested response type separately from the observed type", async () => {
  const protocol = new RecordingProtocol();
  const pending = protocol.request(
    { type: "getState" },
    { expect: "stateResult" },
  );
  const requestId = protocol.lastPayload?.requestId;
  protocol.respond({ requestId, type: "wrongResult" });
  await expect(pending).resolves.toMatchObject({ type: "wrongResult" });
  expect(protocol.matchedResponses).toEqual([
    {
      requestId,
      expectedType: "stateResult",
      responseType: "wrongResult",
    },
  ]);
});

test("ProtocolCore rejects a request-correlated malformed-payload response", async () => {
  const protocol = new RecordingProtocol();
  const pending = protocol.request(
    { type: "simulateGpuiEvent", event: {} },
    { expect: "simulateGpuiEventResult" },
  );
  const requestId = protocol.lastPayload?.requestId;
  protocol.respond({
    type: "externalCommandResult",
    requestId,
    command: "simulateGpuiEvent",
    ok: false,
    errorCode: "invalid_payload",
    errorMessage: "missing field `deltaX`",
  });
  await expect(pending).rejects.toThrow(
    "missing field `deltaX` [invalid_payload]",
  );
});

test("Driver.close drains trailing stdout/stderr before resolving", async () => {
  if (
    delegateToDirectPath(
      "Driver.close drains trailing stdout/stderr before resolving",
    )
  )
    return;
  const root = mkdtempSync(join(tmpdir(), "driver-close-test-"));
  roots.push(root);
  const executable = fakeExecutable(root);
  const sessionDir = join(root, "driver-session");

  const driver = await Driver.launch({
    binary: executable,
    sessionName: "driver-close-test",
    sessionDir,
    sandboxHome: false,
    readyTimeoutMs: 2_000,
  });
  await driver.close();

  expect(driver.alive).toBe(false);
  expect(driver.finalization).toEqual({
    processExited: true,
    streamsDrained: true,
    logWriterClosed: true,
  });
  const log = readFileSync(driver.logPath, "utf8");
  expect(log).toContain("final stdout after TERM");
  expect(log).toContain("final stderr after TERM");

  await expect(driver.close()).resolves.toBeUndefined();
});

test("Driver.close rejects a recorded stream-consumer failure after closing the log", async () => {
  if (
    delegateToDirectPath(
      "Driver.close rejects a recorded stream-consumer failure after closing the log",
    )
  )
    return;
  const root = mkdtempSync(join(tmpdir(), "driver-stream-error-test-"));
  roots.push(root);
  const driver = await Driver.launch({
    binary: fakeExecutable(root),
    sessionName: "driver-stream-error-test",
    sessionDir: join(root, "driver-session"),
    sandboxHome: false,
    readyTimeoutMs: 2_000,
  });
  const internals = driver as unknown as { streamError: Error | null };
  internals.streamError = new Error("injected stream-consumer failure");

  await expect(driver.close()).rejects.toThrow(
    "injected stream-consumer failure",
  );
  expect(driver.alive).toBe(false);
  expect(driver.finalization).toEqual({
    processExited: true,
    streamsDrained: false,
    logWriterClosed: true,
  });
  expect(readFileSync(driver.logPath, "utf8")).toContain(
    "final stdout after TERM",
  );
});

test("Driver.launch finalizes a child that exits before readiness before rejecting", async () => {
  if (
    delegateToDirectPath(
      "Driver.launch finalizes a child that exits before readiness before rejecting",
    )
  )
    return;
  const root = mkdtempSync(join(tmpdir(), "driver-early-exit-test-"));
  roots.push(root);
  const { executable, pidPath } = startupFailureExecutable(root, "exit");
  const sessionDir = join(root, "driver-session");
  const logPath = join(sessionDir, "app.log");

  await expect(
    Driver.launch({
      binary: executable,
      sessionName: "driver-early-exit-test",
      sessionDir,
      sandboxHome: false,
      readyTimeoutMs: 2_000,
    }),
  ).rejects.toThrow("App process exited (code 17)");

  expect(existsSync(pidPath)).toBe(true);
  const pid = Number(readFileSync(pidPath, "utf8"));
  expect(processIsAlive(pid)).toBe(false);
  const log = readFileSync(logPath, "utf8");
  expect(log).toContain("exit trailing stdout");
  expect(log).toContain("exit trailing stderr");
  expectLogWriterClosed(logPath);
});

test("Driver.launch finalizes a readiness timeout before rejecting", async () => {
  if (
    delegateToDirectPath(
      "Driver.launch finalizes a readiness timeout before rejecting",
    )
  )
    return;
  const root = mkdtempSync(join(tmpdir(), "driver-ready-timeout-test-"));
  roots.push(root);
  const { executable, pidPath } = startupFailureExecutable(root, "timeout");
  const sessionDir = join(root, "driver-session");
  const logPath = join(sessionDir, "app.log");

  await expect(
    Driver.launch({
      binary: executable,
      sessionName: "driver-ready-timeout-test",
      sessionDir,
      sandboxHome: false,
      readyTimeoutMs: 20,
    }),
  ).rejects.toThrow("App did not become ready within 20ms");

  expect(existsSync(pidPath)).toBe(true);
  const pid = Number(readFileSync(pidPath, "utf8"));
  expect(processIsAlive(pid)).toBe(false);
  const log = readFileSync(logPath, "utf8");
  expect(log).toContain("timeout trailing stdout");
  expect(log).toContain("timeout trailing stderr");
  expectLogWriterClosed(logPath);
});
