import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const repoRoot = resolve(import.meta.dir, "../..");
const sessionScript = join(repoRoot, "scripts", "agentic", "session.sh");
const roots: string[] = [];
const sentinels: number[] = [];

type SessionResult = {
  status: number | null;
  json: Record<string, any>;
  stderr: string;
};

function launchSentinel(): number {
  const source = [
    "import os, signal, sys, time",
    "pid = os.fork()",
    "if pid:",
    "    print(pid, flush=True)",
    "    sys.exit(0)",
    "os.setsid()",
    "devnull = os.open(os.devnull, os.O_RDWR)",
    "for fd in (0, 1, 2): os.dup2(devnull, fd)",
    "signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))",
    "time.sleep(60)",
  ].join("\n");
  const result = spawnSync("python3", ["-c", source], { encoding: "utf8", timeout: 5_000 });
  if (result.status !== 0) throw new Error(`sentinel launch failed: ${result.stderr}`);
  const pid = Number(result.stdout.trim());
  if (!Number.isInteger(pid) || pid <= 0) throw new Error(`invalid sentinel pid: ${result.stdout}`);
  sentinels.push(pid);
  return pid;
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function waitForDead(pid: number): Promise<boolean> {
  const deadline = Date.now() + 5_000;
  while (Date.now() < deadline) {
    if (!alive(pid)) return true;
    await Bun.sleep(25);
  }
  return !alive(pid);
}

function makeSession(name: string, pid: number, generation: string): { root: string; dir: string } {
  const root = mkdtempSync(join(tmpdir(), "session-stop-ownership-"));
  roots.push(root);
  const dir = join(root, name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "pid"), `${pid}\n`);
  writeFileSync(join(dir, "generation"), `${generation}\n`);
  writeFileSync(join(dir, "marker"), "preserve-me\n");
  for (const fifo of ["pipe", "input"]) {
    const result = spawnSync("mkfifo", [join(dir, fifo)], { encoding: "utf8" });
    if (result.status !== 0) throw new Error(`mkfifo failed: ${result.stderr}`);
  }
  return { root, dir };
}

function runSession(root: string, args: string[]): SessionResult {
  const result = spawnSync("bash", [sessionScript, ...args], {
    cwd: repoRoot,
    env: { ...process.env, SCRIPT_KIT_SESSION_DIR: root },
    encoding: "utf8",
    timeout: 10_000,
  });
  return {
    status: result.status,
    json: JSON.parse(result.stdout.trim()),
    stderr: result.stderr,
  };
}

afterEach(() => {
  for (const pid of sentinels.splice(0)) {
    try {
      process.kill(-pid, "SIGKILL");
    } catch {
      try {
        process.kill(pid, "SIGKILL");
      } catch {}
    }
  }
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe("ownership-guarded session stop", () => {
  test("requires expected PID and generation together", () => {
    const root = mkdtempSync(join(tmpdir(), "session-stop-options-"));
    roots.push(root);
    const result = runSession(root, ["stop", "missing", "--expected-pid", "123"]);

    expect(result.status).not.toBe(0);
    expect(result.json).toMatchObject({
      status: "error",
      error: { code: "invalid_stop_ownership_options" },
    });
  });

  test("legacy name-only stop remains compatible", async () => {
    const pid = launchSentinel();
    const name = `legacy-${process.pid}-${Date.now()}`;
    const { root, dir } = makeSession(name, pid, "legacy-generation");

    const result = runSession(root, ["stop", name]);

    expect(result.status).toBe(0);
    expect(result.json).toMatchObject({ status: "ok", session: name, wasRunning: true });
    expect(result.json.ownershipVerified).toBeUndefined();
    expect(await waitForDead(pid)).toBe(true);
    expect(existsSync(dir)).toBe(false);
  });

  test("a replacement identity mismatch is non-destructive", () => {
    const originalPid = launchSentinel();
    const replacementPid = launchSentinel();
    const name = `replacement-${process.pid}-${Date.now()}`;
    const { root, dir } = makeSession(name, replacementPid, "replacement-generation");

    const result = runSession(root, [
      "stop", name,
      "--expected-pid", String(originalPid),
      "--expected-generation", "original-generation",
    ]);

    expect(result.status).not.toBe(0);
    expect(result.json).toMatchObject({
      status: "error",
      session: name,
      ownershipVerified: false,
      expectedPid: originalPid,
      actualPid: replacementPid,
      expectedGeneration: "original-generation",
      actualGeneration: "replacement-generation",
      error: { code: "session_ownership_mismatch" },
    });
    expect(alive(originalPid)).toBe(true);
    expect(alive(replacementPid)).toBe(true);
    expect(readFileSync(join(dir, "marker"), "utf8")).toBe("preserve-me\n");
    expect(existsSync(join(dir, "pipe"))).toBe(true);
    expect(existsSync(join(dir, "input"))).toBe(true);
  });

  test("missing registry identity fails closed without signaling or cleanup", () => {
    const sentinelPid = launchSentinel();
    const name = `missing-identity-${process.pid}-${Date.now()}`;
    const { root, dir } = makeSession(name, sentinelPid, "present-generation");
    rmSync(join(dir, "generation"));

    const result = runSession(root, [
      "stop", name,
      "--expected-pid", String(sentinelPid),
      "--expected-generation", "present-generation",
    ]);

    expect(result.status).not.toBe(0);
    expect(result.json).toMatchObject({
      status: "error",
      ownershipVerified: false,
      expectedPid: sentinelPid,
      actualPid: sentinelPid,
      expectedGeneration: "present-generation",
      actualGeneration: null,
      error: { code: "session_ownership_mismatch" },
    });
    expect(alive(sentinelPid)).toBe(true);
    expect(readFileSync(join(dir, "marker"), "utf8")).toBe("preserve-me\n");
    expect(existsSync(join(dir, "pipe"))).toBe(true);
    expect(existsSync(join(dir, "input"))).toBe(true);
  });

  test("an exact stop kills only the owned PID and leaves final status not_found", async () => {
    const ownedPid = launchSentinel();
    const unrelatedPid = launchSentinel();
    const generation = `owned-generation-${Date.now()}`;
    const name = `exact-${process.pid}-${Date.now()}`;
    const { root, dir } = makeSession(name, ownedPid, generation);

    const result = runSession(root, [
      "stop", name,
      "--expected-pid", String(ownedPid),
      "--expected-generation", generation,
    ]);

    expect(result.status).toBe(0);
    expect(result.json).toMatchObject({
      status: "ok",
      session: name,
      wasRunning: true,
      ownershipVerified: true,
      expectedPid: ownedPid,
      actualPid: ownedPid,
      expectedGeneration: generation,
      actualGeneration: generation,
    });
    expect(await waitForDead(ownedPid)).toBe(true);
    expect(alive(unrelatedPid)).toBe(true);
    expect(existsSync(dir)).toBe(false);

    const status = runSession(root, ["status", name]);
    expect(status.status).toBe(0);
    expect(status.json).toMatchObject({ status: "not_found", session: name, alive: false });
  }, 15_000);
});
