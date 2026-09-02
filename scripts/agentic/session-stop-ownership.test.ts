import type { Subprocess } from "bun";
import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const repoRoot = resolve(import.meta.dir, "../..");
const sessionScript = join(repoRoot, "scripts", "agentic", "session.sh");
const roots: string[] = [];
const sentinels: Subprocess<"ignore", "ignore", "ignore">[] = [];
const sessions: { root: string; dir: string; name: string; pid: number; generation: string }[] = [];

type SessionResult = {
  status: number | null;
  json: Record<string, any>;
  stderr: string;
};

function launchSentinel(): number {
  const child = Bun.spawn(["sleep", "60"], { stdin: "ignore", stdout: "ignore", stderr: "ignore" });
  sentinels.push(child);
  return child.pid;
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

function makeSession(name: string): { root: string; dir: string; pid: number; generation: string } {
  const root = mkdtempSync(join(tmpdir(), "session-stop-ownership-"));
  roots.push(root);
  const dir = join(root, name);
  writeFileSync(join(root, "fixture.sh"), '#!/bin/sh\ntrap "exit 0" TERM INT\nprintf "STARTUP_READY fixture\\n"\nwhile IFS= read -r line; do :; done\n', { mode: 0o700 });
  const result = runSession(root, ["start", name]);
  if (result.status !== 0) throw new Error(`fixture start failed: ${result.stderr}`);
  const pid = result.json.pid as number, generation = result.json.sessionGeneration as string;
  sessions.push({ root, dir, name, pid, generation });
  if (result.json.ready !== true) throw new Error(`fixture readiness failed: ${result.stderr}`);
  writeFileSync(join(dir, "marker"), "preserve-me\n");
  return { root, dir, pid, generation };
}

function runSession(root: string, args: string[]): SessionResult {
  const result = spawnSync("bash", [sessionScript, ...args], {
    cwd: repoRoot,
    env: { PATH: process.env.PATH, HOME: root, LANG: "C", SCRIPT_KIT_SESSION_DIR: root,
      SCRIPT_KIT_GPUI_BINARY: join(root, "fixture.sh"), SCRIPT_KIT_SESSION_READY_TIMEOUT_MS: "2000" },
    encoding: "utf8",
    timeout: 15_000,
    maxBuffer: 16 * 1024,
  });
  return {
    status: result.status,
    json: JSON.parse(result.stdout.trim()),
    stderr: result.stderr,
  };
}

afterEach(async () => {
  const failures: unknown[] = [], protectedRoots = new Set<string>();
  for (const session of sessions.splice(0)) {
    if (!existsSync(session.dir)) continue;
    try {
      // The missing-generation case intentionally damages its own registry. Restore
      // only that fixture field so real exact-supervisor cleanup remains available.
      if (!existsSync(join(session.dir, "generation"))) writeFileSync(join(session.dir, "generation"), `${session.generation}\n`);
      const stopped = runSession(session.root, ["stop", session.name, "--expected-pid", String(session.pid), "--expected-generation", session.generation]);
      if (stopped.status !== 0) throw new Error(`fixture stop failed: ${stopped.stderr}`);
    } catch (error) {
      failures.push(error);
      protectedRoots.add(session.root);
    }
  }
  for (const child of sentinels.splice(0)) {
    if (child.exitCode === null) child.kill("SIGKILL");
    await child.exited;
  }
  for (const root of roots.splice(0)) if (!protectedRoots.has(root)) rmSync(root, { recursive: true, force: true });
  if (failures.length) throw new AggregateError(failures, "Exact fixture cleanup could not be proved; preserving its registry");
}, 20_000);

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
    const name = `legacy-${process.pid}-${Date.now()}`;
    const { root, dir, pid } = makeSession(name);

    const result = runSession(root, ["stop", name]);

    expect(result.status).toBe(0);
    expect(result.json).toMatchObject({ status: "ok", session: name, wasRunning: true });
    expect(result.json.ownershipVerified).toBeUndefined();
    expect(await waitForDead(pid)).toBe(true);
    expect(existsSync(dir)).toBe(false);
  }, 15_000);

  test("a replacement identity mismatch is non-destructive", () => {
    const originalPid = launchSentinel();
    const name = `replacement-${process.pid}-${Date.now()}`;
    const { root, dir, pid: replacementPid, generation } = makeSession(name);

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
      actualGeneration: generation,
      error: { code: "session_ownership_mismatch" },
    });
    expect(alive(originalPid)).toBe(true);
    expect(alive(replacementPid)).toBe(true);
    expect(readFileSync(join(dir, "marker"), "utf8")).toBe("preserve-me\n");
    expect(existsSync(join(dir, "pipe"))).toBe(true);
    expect(existsSync(join(dir, "input"))).toBe(true);
  }, 15_000);

  test("missing registry identity fails closed without signaling or cleanup", () => {
    const name = `missing-identity-${process.pid}-${Date.now()}`;
    const { root, dir, pid: sentinelPid, generation } = makeSession(name);
    rmSync(join(dir, "generation"));

    const result = runSession(root, [
      "stop", name,
      "--expected-pid", String(sentinelPid),
      "--expected-generation", generation,
    ]);

    expect(result.status).not.toBe(0);
    expect(result.json).toMatchObject({
      status: "error",
      ownershipVerified: false,
      expectedPid: sentinelPid,
      actualPid: sentinelPid,
      expectedGeneration: generation,
      actualGeneration: null,
      error: { code: "session_ownership_mismatch" },
    });
    expect(alive(sentinelPid)).toBe(true);
    expect(readFileSync(join(dir, "marker"), "utf8")).toBe("preserve-me\n");
    expect(existsSync(join(dir, "pipe"))).toBe(true);
    expect(existsSync(join(dir, "input"))).toBe(true);
  }, 15_000);

  test("an exact stop kills only the owned PID and leaves final status not_found", async () => {
    const unrelatedPid = launchSentinel();
    const name = `exact-${process.pid}-${Date.now()}`;
    const { root, dir, pid: ownedPid, generation } = makeSession(name);

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

test("traditional session helpers leave no bytecode through start resume status and exact stop", () => {
  const root = mkdtempSync(join(tmpdir(), "session-bytecode-"));
  const registry = join(root, "sessions"), cache = join(root, "cache"), observations = join(root, "imports.ndjson");
  const binary = join(root, "fixture.sh"), name = "bytecode-fixture", dir = join(registry, name);
  // Only a shell reader runs: no application binary, native input or network.
  writeFileSync(binary, '#!/bin/sh\ntrap "exit 0" TERM INT\nprintf "STARTUP_READY fixture\\n"\nwhile IFS= read -r line; do printf "%s\\n" "$line"; done\n', { mode: 0o700 });
  writeFileSync(join(root, "sitecustomize.py"), [
    "import json, sys",
    `with open(${JSON.stringify(observations)}, "a") as observation:`,
    '    observation.write(json.dumps({"mode": "forwarder" if sys.argv[0] == "-c" else sys.argv[1], "disabled": sys.dont_write_bytecode}) + "\\n")',
  ].join("\n"));
  const run = (args: string[]) => spawnSync("bash", [sessionScript, ...args], {
    cwd: root, encoding: "utf8", timeout: 15_000, maxBuffer: 16 * 1024,
    env: { PATH: process.env.PATH, HOME: root, LANG: "C", SCRIPT_KIT_SESSION_DIR: registry,
      SCRIPT_KIT_GPUI_BINARY: binary, SCRIPT_KIT_SESSION_READY_TIMEOUT_MS: "2000",
      PYTHONPATH: root, PYTHONPYCACHEPREFIX: cache, PYTHONDONTWRITEBYTECODE: "" },
  });
  let stopArgs = ["stop", name];
  try {
    const started = run(["start", name]);
    expect(started.status).toBe(0);
    const launch = JSON.parse(started.stdout.trim());
    expect(launch).toMatchObject({ status: "ok", ready: true, resumed: false, binary });
    stopArgs = ["stop", name, "--expected-pid", String(launch.pid), "--expected-generation", launch.sessionGeneration];
    const resumed = run(["start", name]);
    expect(resumed.status).toBe(0);
    expect(JSON.parse(resumed.stdout.trim())).toMatchObject({ status: "ok", pid: launch.pid, resumed: true });
    const status = run(["status", name]);
    expect(status.status).toBe(0);
    expect(JSON.parse(status.stdout.trim())).toMatchObject({ status: "ok", alive: true, healthy: true });
    const stopped = run(stopArgs);
    expect(stopped.status).toBe(0);
    expect(JSON.parse(stopped.stdout.trim())).toMatchObject({ status: "ok", ownershipVerified: true, wasRunning: true });
    expect(existsSync(dir)).toBe(false);
    const closed = `${dir}.closed-${launch.sessionGeneration}`;
    const receipt = JSON.parse(readFileSync(join(closed, "app-exit.json"), "utf8"));
    expect(receipt).toMatchObject({ event: "app_process_exited", pid: launch.pid, sessionGeneration: launch.sessionGeneration,
      cleanup: { closed: true, processExited: true, processGroupExited: true, logWriterClosed: true, failureCodes: [] } });
    expect(readFileSync(join(closed, "lifecycle.ndjson"), "utf8").trim().split("\n").map(line => JSON.parse(line))).toContainEqual(receipt);
    const imports = readFileSync(observations, "utf8").trim().split("\n").map(line => JSON.parse(line));
    expect(imports.map(entry => entry.mode).sort()).toEqual([
      "--binary", "--check-session", "--check-session", "--exec-child", "--stop-session", "forwarder",
    ]);
    expect(imports.every(entry => entry.disabled === true)).toBe(true);
    expect(existsSync(cache) ? readdirSync(cache, { recursive: true }) : []).toEqual([]);
  } finally {
    // Failures still use the exact registry-owned stop path before fixture removal.
    if (existsSync(dir)) expect(run(stopArgs).status).toBe(0);
    rmSync(root, { recursive: true, force: true });
  }
}, 30_000);

