import { afterEach, expect, test } from "bun:test";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { spawnOwnedProcess, validateNativeLifecycle, type NativeLifecycleObservation, type OwnedProcessOptions } from "./owned-process.ts";
import { spawnSync } from "node:child_process";

const paths: string[] = [];
afterEach(() => { for (const path of paths.splice(0)) rmSync(path, { recursive: true, force: true }); });
const environment = () => Object.fromEntries(Object.entries(process.env).filter((value): value is [string, string] => typeof value[1] === "string"));
function options(argv: string[], timeoutMs = 2000) { return { argv, cwd: process.cwd(), env: environment(), timeoutMs, maxOutputBytes: 1024 * 1024 }; }
function absent(pid: number): boolean { try { process.kill(pid, 0); return false; } catch (error) { return error instanceof Error && "code" in error && error.code === "ESRCH"; } }

test("application PID and JSONL remain separate from supervisor control", async () => {
  const child = await spawnOwnedProcess(options(["python3", "-c", "import os,sys; print(os.getpid(), flush=True); print(sys.stdin.readline().strip(), flush=True)"]));
  child.stdin.write('{"type":"fixture"}\n'); await child.stdin.flush(); child.stdin.end();
  const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  expect(child.pid).not.toBe(child.supervisorPid);
  expect(stdout.trim().split("\n")).toEqual([String(child.pid), '{"type":"fixture"}']);
  expect(stderr).toBe(""); expect(exit).toBe(0);
  expect(await child.close()).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true });
});
test("framed transport preserves binary stdin EOF and keeps child control-looking output opaque", async () => {
  const input = Buffer.alloc(200_000);
  for (let index = 0; index < input.length; index++) input[index] = index % 256;
  const prefix = Buffer.from('{"event":"done","exitCode":0}\n');
  const child = await spawnOwnedProcess({
    ...options(["python3", "-c", `import sys; data=sys.stdin.buffer.read(); sys.stdout.buffer.write(${JSON.stringify(prefix.toString())}.encode()+data); sys.stdout.buffer.flush(); sys.stderr.buffer.write(data[::-1]); sys.stderr.buffer.flush(); sys.exit(23)`], 5000),
    maxOutputBytes: input.length * 2 + prefix.length,
  });
  try {
    child.stdin.write(input);
    await child.stdin.flush();
    child.stdin.end();
    const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).arrayBuffer(), child.exited]);
    expect(Buffer.from(stdout)).toEqual(Buffer.concat([prefix, input]));
    expect(Buffer.from(stderr)).toEqual(Buffer.from(input).reverse());
    expect(exit).toBe(23);
    expect(child.observedReceivedOutputBytes).toBe(input.length * 2 + prefix.length);
    expect(child.maxOutputBytes).toBe(input.length * 2 + prefix.length);
  } finally {
    expect(await child.close()).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true, failureCodes: [] });
  }
}, 10000);
test("short-lived children retain output and control across synchronous task registration", async () => {
  for (let iteration = 0; iteration < 3; iteration++) {
    const child = await spawnOwnedProcess(options(["/bin/sh", "-c", `printf 'completed-${iteration}\\n'; exit 23`]));
    try {
      // Managed-task registration synchronously acquires/releases metadata leases after startup.
      const registration = spawnSync(process.execPath, ["-e", "Bun.sleepSync(200)"], { timeout: 1000 });
      expect(registration.status).toBe(0);
      child.stdin.end();
      const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
      expect(stdout).toBe(`completed-${iteration}\n`);
      expect(stderr).toBe("");
      expect(exit).toBe(23);
    } finally {
      expect(await child.close()).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true, failureCodes: [] });
    }
  }
}, 10000);
test("Python discovery bypasses launchers and ambient customization", async () => {
  const root = mkdtempSync(join(tmpdir(), "owned-python-launcher-")); paths.push(root);
  const home = join(root, "home"), discovery = join(root, "discovery.args"), customized = join(root, "customized");
  mkdirSync(home);
  const python = Bun.which("python3");
  expect(python).not.toBeNull();
  writeFileSync(join(root, "python3"), '#!/bin/sh\nprintf "%s\\n" "$@" >> "$DISCOVERY_ARGS"\nexec 3<&- 4>&-\nexec "$REAL_PYTHON" "$@"\n', { mode: 0o700 });
  writeFileSync(join(root, "sitecustomize.py"), `from pathlib import Path\nPath(${JSON.stringify(customized)}).write_text("unexpected customization")\n`);
  const child = await spawnOwnedProcess({
    argv: ["/bin/sh", "-c", 'printf "%s\\n%s\\n" "$HOME" "$OWNED_TEST_VALUE"; exit 23'],
    cwd: root, timeoutMs: 2000, maxOutputBytes: 4096,
    env: { PATH: `${root}:/usr/bin:/bin`, HOME: home, LANG: "C.UTF-8", PYTHONPATH: root,
      REAL_PYTHON: python!, DISCOVERY_ARGS: discovery, OWNED_TEST_VALUE: "private" },
  });
  try {
    child.stdin.end();
    const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
    expect(stdout).toBe(`${home}\nprivate\n`);
    expect(stderr).toBe("");
    expect(exit).toBe(23);
    expect(readFileSync(discovery, "utf8")).toBe("-I\n-S\n-B\n-c\nimport sys; print(sys.executable)\n");
    expect(existsSync(customized)).toBe(false);
  } finally {
    expect(await child.close()).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true });
  }
});
test("isolated supervisor and exec-child imports leave no bytecode across real lifecycles", async () => {
  const root = mkdtempSync(join(tmpdir(), "owned-bytecode-"));
  const cache = join(root, "infrastructure-cache"), baselineCache = join(root, "baseline-cache");
  const observations = join(root, "imports.log");
  const env = { ...environment(), PYTHONDONTWRITEBYTECODE: "1" };
  try {
    writeFileSync(join(root, "bytecode_probe.py"), "VALUE = 1\n");
    // The negative control proves this writable sandbox would cache imports without -B,
    // even when the caller supplies PYTHONDONTWRITEBYTECODE under isolated Python.
    const baseline = spawnSync("python3", ["-I", "-S", "-c", [
      "import sys",
      `sys.pycache_prefix = ${JSON.stringify(baselineCache)}`,
      `sys.path.insert(0, ${JSON.stringify(root)})`,
      "import bytecode_probe",
      "print(sys.dont_write_bytecode)",
      "print(bytecode_probe.__cached__)",
    ].join("\n")], { cwd: root, env, encoding: "utf8", timeout: 5000, maxBuffer: 4096 });
    expect(baseline.status).toBe(0);
    expect(baseline.stderr).toBe("");
    const [suppressed, cached] = baseline.stdout.trim().split("\n");
    expect(suppressed).toBe("False");
    expect(cached!.startsWith(`${baselineCache}/`)).toBe(true);
    expect(existsSync(cached!)).toBe(true);

    // Copy the unchanged TS launcher so its sibling supervisor can add a disposable
    // import probe before executing the real Python source. Both real entrypoints
    // keep their original flags, gates, process groups, framing and cleanup paths.
    copyFileSync(join(import.meta.dir, "owned-process.ts"), join(root, "owned-process.ts"));
    const supervisor = join(import.meta.dir, "session-supervisor.py");
    writeFileSync(join(root, "session-supervisor.py"), [
      "import sys",
      `sys.pycache_prefix = ${JSON.stringify(cache)}`,
      `sys.path.insert(0, ${JSON.stringify(root)})`,
      "import bytecode_probe",
      "sys.path.pop(0)",
      `with open(${JSON.stringify(observations)}, "a") as observation:`,
      '    observation.write(f"{sys.argv[1]}:{int(sys.dont_write_bytecode)}:{sys.flags.isolated}:{sys.flags.no_site}\\n")',
      `with open(${JSON.stringify(supervisor)}) as source:`,
      `    exec(compile(source.read(), ${JSON.stringify(supervisor)}, "exec"), globals())`,
    ].join("\n"));
    // The fixture module path is chosen by mkdtemp, so it cannot be statically imported.
    const { spawnOwnedProcess: sandboxSpawn } = await import(join(root, "owned-process.ts"));
    for (let iteration = 0; iteration < 2; iteration++) {
      const child = await sandboxSpawn({
        ...options(["python3", "-I", "-S", "-c", 'import os,sys; print(f"{os.getpid()}:{sys.dont_write_bytecode}"); print(sys.stdin.read(), end=""); sys.stderr.write("fixture-stderr\\n")'], 3000),
        cwd: root, env, maxOutputBytes: 4096,
      });
      try {
        child.stdin.write(`fixture-${iteration}\n`);
        await child.stdin.flush();
        child.stdin.end();
        const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
        expect(child.pid).not.toBe(child.supervisorPid);
        // The application's own Python policy is not changed by infrastructure -B.
        expect(stdout).toBe(`${child.pid}:False\nfixture-${iteration}\n`);
        expect(stderr).toBe("fixture-stderr\n");
        expect(exit).toBe(0);
      } finally {
        expect(await child.close()).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true, failureCodes: [] });
        expect(absent(child.pid)).toBe(true);
        expect(absent(child.supervisorPid)).toBe(true);
      }
    }
    expect(readFileSync(observations, "utf8").trim().split("\n")).toEqual([
      "--request-owned:1:1:1", "--exec-child:1:1:1", "--request-owned:1:1:1", "--exec-child:1:1:1",
    ]);
    expect(existsSync(cache) ? readdirSync(cache, { recursive: true }) : []).toEqual([]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}, 15_000);

test("timeout reaps an owned fork without touching a borrowed sentinel", async () => {
  const sentinel = Bun.spawn(["sleep", "30"], { stdout: "ignore", stderr: "ignore" });
  try {
    const child = await spawnOwnedProcess(options(["python3", "-c", "import subprocess,time; p=subprocess.Popen(['sleep','30']); print(p.pid,flush=True); time.sleep(30)"], 150));
    const [stdout] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
    const cleanup = await child.close();
    expect(cleanup.processExited).toBe(true); expect(cleanup.processGroupExited).toBe(true);
    expect(cleanup.failureCodes).toContain("process_timeout");
    expect(absent(child.pid)).toBe(true);
    expect(Number(stdout.trim())).toBeGreaterThan(0);
    expect(absent(sentinel.pid)).toBe(false);
  } finally { sentinel.kill(); await sentinel.exited; }
});
test("output overflow is bounded and never becomes successful execution", async () => {
  const child = await spawnOwnedProcess({ ...options(["python3", "-c", "import sys,time; sys.stdout.write('x'*100000); sys.stdout.flush(); time.sleep(30)"]), maxOutputBytes: 4096 });
  await Promise.allSettled([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  const cleanup = await child.close();
  expect(cleanup.processExited).toBe(true); expect(cleanup.processGroupExited).toBe(true);
  expect(cleanup.failureCodes).toContain("process_output_limit");
});
test("finite stdout and stderr bursts pause behind a stalled consumer and drain losslessly", async () => {
  const bytes = 4 * 1024 * 1024;
  const child = await spawnOwnedProcess({
    ...options(["python3", "-c", `import sys; sys.stdout.buffer.write(b'o' * ${bytes}); sys.stdout.buffer.flush(); sys.stderr.buffer.write(b'e' * ${bytes}); sys.stderr.buffer.flush()`], 5000),
    maxOutputBytes: bytes * 2,
  });
  try {
    child.stdin.end();
    // Hold the real reader event loop so the supervisor must backpressure the child pipes.
    Bun.sleepSync(200);
    const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
    expect(exit).toBe(0);
    expect(stdout).toBe("o".repeat(bytes));
    expect(stderr).toBe("e".repeat(bytes));
  } finally {
    expect(await child.close()).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true, failureCodes: [] });
  }
}, 10000);

test("paused output reads do not suspend the absolute process deadline", async () => {
  const child = await spawnOwnedProcess({
    ...options(["python3", "-c", "import sys; sys.stdout.buffer.write(b'x' * (16 * 1024 * 1024)); sys.stdout.buffer.flush()"], 100),
    maxOutputBytes: 32 * 1024 * 1024,
  });
  try {
    child.stdin.end();
    Bun.sleepSync(250);
    const [stdout, , exit] = await Promise.all([new Response(child.stdout).arrayBuffer(), new Response(child.stderr).text(), child.exited]);
    expect(exit).not.toBe(0);
    expect(stdout.byteLength).toBeGreaterThan(0);
  } finally {
    const cleanup = await child.close();
    expect(cleanup).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true });
    expect(cleanup.failureCodes).toEqual(["process_timeout"]);
  }
}, 10000);
test("setup failure after identity publication still finalizes owned process", async () => {
  const child = await spawnOwnedProcess(options(["/missing/owned-fixture-executable"]));
  const [_, stderr, status] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  expect(status).not.toBe(0); expect(stderr.length).toBeGreaterThan(0);
  expect(await child.close()).toMatchObject({ processExited: true, processGroupExited: true });
});
test("killed request owner closes lifetime pipe and leaves no owned group", async () => {
  const root = mkdtempSync(join(tmpdir(), "owned-parent-loss-")); paths.push(root);
  const pidFile = join(root, "identity.json"), script = join(root, "request.ts");
  writeFileSync(script, `import {spawnOwnedProcess} from ${JSON.stringify(resolve(import.meta.dir, "owned-process.ts"))};\nconst p=await spawnOwnedProcess({argv:['sleep','30'],cwd:process.cwd(),env:Object.fromEntries(Object.entries(process.env).filter(x=>typeof x[1]==='string')),timeoutMs:30000,maxOutputBytes:4096}); await Bun.write(${JSON.stringify(pidFile)},JSON.stringify(p.identity)); await p.exited;`);
  const requester = Bun.spawn([process.execPath, script], { stdout: "ignore", stderr: "ignore" });
  const started = performance.now();
  // This deliberately observes kernel process death after SIGKILL; fake JS time cannot advance another process or reap its group.
  try {
    while (!existsSync(pidFile) && performance.now() - started < 5000) await Bun.sleep(20);
    expect(existsSync(pidFile)).toBe(true);
    const identity = JSON.parse(readFileSync(pidFile, "utf8"));
    requester.kill("SIGKILL"); await requester.exited;
    while ((!absent(identity.pid) || !absent(identity.supervisorPid)) && performance.now() - started < 15000) await Bun.sleep(25);
    expect(absent(identity.pid)).toBe(true); expect(absent(identity.supervisorPid)).toBe(true);
  } finally { requester.kill(); await requester.exited; }
}, 20000);

test("nested supervisors survive outer group termination long enough to reap resistant children", async () => {
  const root = mkdtempSync(join(tmpdir(), "owned-nested-loss-")); paths.push(root);
  const modulePath = JSON.stringify(resolve(import.meta.dir, "owned-process.ts"));
  const innerPath = join(root, "inner.ts"), requesterPath = join(root, "requester.ts"), identitiesPath = join(root, "identities.json");
  writeFileSync(innerPath, `import {spawnOwnedProcess} from ${modulePath};\nconst child=await spawnOwnedProcess({argv:['python3','-c','import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print("ready",flush=True); time.sleep(60)'],cwd:process.cwd(),env:Object.fromEntries(Object.entries(process.env).filter(x=>typeof x[1]==='string')),timeoutMs:60000,maxOutputBytes:4096});\nconst reader=child.stdout.getReader(); await reader.read(); reader.releaseLock(); console.log(JSON.stringify(child.identity)); await child.exited;`);
  writeFileSync(requesterPath, `import {spawnOwnedProcess} from ${modulePath};\nconst child=await spawnOwnedProcess({argv:[process.execPath,${JSON.stringify(innerPath)}],cwd:process.cwd(),env:Object.fromEntries(Object.entries(process.env).filter(x=>typeof x[1]==='string')),timeoutMs:60000,maxOutputBytes:4096});\nconst reader=child.stdout.getReader(); const first=await reader.read(); reader.releaseLock(); await Bun.write(${JSON.stringify(identitiesPath)},JSON.stringify([child.identity,JSON.parse(new TextDecoder().decode(first.value))])); await child.exited;`);
  const requester = Bun.spawn([process.execPath, requesterPath], { stdout: "ignore", stderr: "ignore" });
  const started = performance.now();
  try {
    // Real kernel death/reaping across three independent processes cannot be driven by fake JavaScript time.
    while (!existsSync(identitiesPath) && performance.now() - started < 10000) await Bun.sleep(20);
    expect(existsSync(identitiesPath)).toBe(true);
    const identities = JSON.parse(readFileSync(identitiesPath, "utf8"));
    requester.kill("SIGKILL"); await requester.exited;
    const pids: number[] = identities.flatMap((identity: { pid: number; supervisorPid: number }) => [identity.pid, identity.supervisorPid]);
    while (pids.some(pid => !absent(pid)) && performance.now() - started < 20000) await Bun.sleep(25);
    expect(pids.every(absent)).toBe(true);
  } finally { requester.kill(); await requester.exited; }
}, 25000);

// These are transport fixtures, not proof that a real native window closed.
const nativeExpectation = { launchNonce: "00000000-0000-4000-8000-000000000001",
  binarySha256: "b".repeat(64), manifestSha256: "c".repeat(64), policySha256: "d".repeat(64) };
function fakeNativeOptions(mode = "valid", timeoutMs = 2000): OwnedProcessOptions {
  const script = `import os,sys,json,time
sys.stdin.buffer.read()
if os.environ['FAKE_NATIVE_MODE'] == 'unobserved':
    time.sleep(30)
identity={'pid':os.getpid(),'processStartTime':os.environ['SCRIPT_KIT_PROCESS_START_TIME'],'processInstanceId':os.environ['SCRIPT_KIT_PROCESS_INSTANCE_ID'],'sessionGeneration':os.environ['SCRIPT_KIT_SESSION_GENERATION'],'binarySha256':os.environ['SCRIPT_KIT_OWNED_EVALUATION_BINARY_SHA256'],'manifestSha256':os.environ['SCRIPT_KIT_OWNED_EVALUATION_MANIFEST_SHA256']}
result={'schemaVersion':1,'operation':'end','lifecycle':True,'shutdownReason':'inputEof','identity':identity,'launchNonce':os.environ['SCRIPT_KIT_OWNED_EVALUATION_NONCE'],'policySha256':os.environ['SCRIPT_KIT_OWNED_EVALUATION_POLICY_SHA256'],'ok':True,'ownedWindowsClosed':True,'remainingWindows':0,'refusedEffects':0,'native':{'installed':True,'openedWindows':1,'liveWindows':0,'automationWindows':0,'completedFrames':2,'readbackImages':0,'refusedOperations':0}}
if os.environ['FAKE_NATIVE_MODE'] == 'wrongNonce':
    result['launchNonce']='wrong'
value={'type':'designResult','protocolVersion':2,'result':result}
print(json.dumps(value),flush=True)
if os.environ['FAKE_NATIVE_MODE'] == 'duplicate':
    print(json.dumps(value),flush=True)
if os.environ['FAKE_NATIVE_MODE'] == 'postLifecycleFailure':
    sys.exit(23)
`;
  return { ...options(["python3", "-c", script, "--owned-ui-evaluation"], timeoutMs), ownedNative: nativeExpectation,
    env: { ...environment(), FAKE_NATIVE_MODE: mode, SCRIPT_KIT_OWNED_EVALUATION: "1",
      SCRIPT_KIT_OWNED_EVALUATION_NONCE: nativeExpectation.launchNonce,
      SCRIPT_KIT_OWNED_EVALUATION_POLICY_SHA256: nativeExpectation.policySha256,
      SCRIPT_KIT_OWNED_EVALUATION_BINARY_SHA256: nativeExpectation.binarySha256,
      SCRIPT_KIT_OWNED_EVALUATION_MANIFEST_SHA256: nativeExpectation.manifestSha256 } };
}

test.each(["EOF", "timeout", "close"])("fake native %s retains validated final stdout before reaping", async reason => {
  const child = await spawnOwnedProcess(fakeNativeOptions("valid", reason === "timeout" ? 100 : 2000));
  const output = new Response(child.stdout).text(), errors = new Response(child.stderr).text();
  if (reason === "EOF") child.stdin.end();
  const closing = reason === "close" ? child.close() : undefined;
  const [stdout, stderr] = await Promise.all([output, errors, child.exited]);
  const observation = JSON.parse(stdout.trim());
  expect(validateNativeLifecycle(observation, child.identity, nativeExpectation)).toEqual(child.nativeLifecycle!);
  expect(stderr).toBe("");
  expect(await (closing ?? child.close())).toMatchObject({ closed: true, ownedWindowsClosed: true, processGroupExited: true });
});

test.each(["duplicate", "wrongNonce", "unobserved"])("fake native %s never upgrades PID death into closure", async mode => {
  const child = await spawnOwnedProcess(fakeNativeOptions(mode));
  child.stdin.end();
  await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  const cleanup = await child.close();
  expect(cleanup.processExited).toBe(true);
  expect(cleanup.processGroupExited).toBe(true);
  expect(cleanup.ownedWindowsClosed).not.toBe(true);
  expect(cleanup.closed).toBe(false);
}, 10000);

test("native lifecycle followed by destructor failure cannot finalize references", async () => {
  const child = await spawnOwnedProcess(fakeNativeOptions("postLifecycleFailure"));
  child.stdin.end();
  const [stdout, stderr, exit] = await Promise.all([
    new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
  ]);
  expect(validateNativeLifecycle(JSON.parse(stdout.trim()), child.identity, nativeExpectation).result.ownedWindowsClosed).toBe(true);
  expect(stderr).toBe("");
  expect(exit).toBe(23);
  const cleanup = await child.close();
  expect(cleanup).toMatchObject({ processExited: true, processGroupExited: true,
    ownedWindowsClosed: true, referencesFinalized: false, closed: false });
  expect(cleanup.failureCodes).toContain("native_references_not_finalized");
});

test("native lifecycle boundary rejects wrong runtime, nonce, version, counters and correlated lookalikes", () => {
  const identity = { pid: 42, processStartTime: "actual", processInstanceId: "instance", sessionGeneration: "generation",
    processGroupId: 42, supervisorPid: 7, supervisorStartTime: "supervisor" };
  const observation: NativeLifecycleObservation = { type: "designResult", protocolVersion: 2, result: {
    schemaVersion: 1, operation: "end", lifecycle: true, shutdownReason: "inputEof", identity: { ...identity, ...nativeExpectation },
    launchNonce: nativeExpectation.launchNonce, policySha256: nativeExpectation.policySha256, ok: true, ownedWindowsClosed: true,
    remainingWindows: 0, refusedEffects: 0, native: { installed: true, openedWindows: 1, liveWindows: 0, automationWindows: 0,
      completedFrames: 2, readbackImages: 0, refusedOperations: 0 } } };
  expect(validateNativeLifecycle(observation, identity, nativeExpectation).result.ownedWindowsClosed).toBe(true);
  for (const invalid of [
    { ...observation, requestId: "pending" }, { ...observation, protocolVersion: 1 },
    { ...observation, result: { ...observation.result, launchNonce: "foreign" } },
    { ...observation, result: { ...observation.result, identity: { ...observation.result.identity, pid: 43 } } },
    { ...observation, result: { ...observation.result, identity: { ...observation.result.identity, processStartTime: "reused" } } },
    { ...observation, result: { ...observation.result, native: { ...observation.result.native, automationWindows: 1 } } },
    { ...observation, result: { ...observation.result, native: { ...observation.result.native, completedFrames: -1 } } },
    { ...observation, result: { ...observation.result, ok: false } },
  ]) expect(() => validateNativeLifecycle(invalid, identity, nativeExpectation)).toThrow("native_lifecycle_invalid");
});

test.each(["valid", "unobserved", "delayedBind"])("fake native parent loss persists exact managed task as %s without inferring closure", async mode => {
  // Managed metadata rejects symlink ancestors, including macOS lexical /var and /tmp aliases.
  const root = realpathSync(mkdtempSync(join(tmpdir(), "owned-native-parent-loss-")));
  // Unknown native closure leaves protected evidence, not a disposable fixture.
  if (mode !== "unobserved") paths.push(root);
  const pidFile = join(root, "identity.json"), script = join(root, "request.ts");
  const home = join(root, "home"); mkdirSync(home);
  const spec = fakeNativeOptions(mode === "delayedBind" ? "valid" : mode, 30000);
  spec.cwd = root;
  spec.env = { ...spec.env, HOME: home };
  const modulePath = resolve(import.meta.dir, "owned-process.ts"), lifecyclePath = resolve(import.meta.dir, "artifact-lifecycle.ts");
  const helperExecutable = mode === "delayedBind" ? join(root, "delayed-helper") : process.execPath;
  if (mode === "delayedBind") {
    // Real helper latency may exceed three seconds while staying inside the five-second
    // metadata lease wait. exec preserves the supervisor's direct-child authority.
    writeFileSync(helperExecutable, `#!${Bun.which("python3")}\nimport os,sys,time\ntime.sleep(4)\nos.execv(${JSON.stringify(process.execPath)},[${JSON.stringify(process.execPath)},*sys.argv[1:]])\n`, { mode: 0o700 });
  }
  writeFileSync(script, `import {spawnOwnedProcess} from ${JSON.stringify(modulePath)};
import {beginManagedTask,claimOutput,validateOutputTarget,adoptSupervisorTask} from ${JSON.stringify(lifecyclePath)};
const claim=claimOutput(validateOutputTarget({repoRoot:${JSON.stringify(root)},candidate:${JSON.stringify(join(root, ".test-output", "task"))},kind:'directory',probeId:'native-parent-fixture'}));
const task=beginManagedTask(claim,'runtime-run',[]);
const spec=${JSON.stringify(spec)};
spec.ownedNative.task={repositoryRoot:${JSON.stringify(root)},recordPath:task.recordPath,identity:task.identity,helperExecutable:${JSON.stringify(helperExecutable)}};
const child=await spawnOwnedProcess(spec); adoptSupervisorTask(task,child.identity);
await Bun.write(${JSON.stringify(pidFile)},JSON.stringify({process:child.identity,task:{identity:task.identity,recordPath:task.recordPath}}));
await child.exited;`);
  const requester = Bun.spawn([process.execPath, script], { cwd: root, env: { ...environment(), HOME: home }, stdout: "ignore", stderr: "pipe" });
  let requesterExitCode: number | undefined, requesterStderr = "";
  const requesterExited = requester.exited.then(code => { requesterExitCode = code; return code; });
  const stderrDrained = (async () => {
    const decoder = new TextDecoder();
    try {
      for await (const chunk of requester.stderr) {
        if (requesterStderr.length < 8192) requesterStderr += decoder.decode(chunk).slice(0, 8192 - requesterStderr.length);
      }
    } catch (error) {
      requesterStderr = `${requesterStderr}\nstderr read failed: ${String(error)}`.slice(0, 8192);
    }
  })();
  const started = performance.now();
  try {
    // Kernel parent death and independent Python EOF/reaping cannot be advanced by JS fake timers.
    while (!existsSync(pidFile) && requesterExitCode === undefined && performance.now() - started < 10000) await Bun.sleep(20);
    if (!existsSync(pidFile)) {
      await Promise.race([stderrDrained, Bun.sleep(200)]);
      throw new Error(`fake native ${mode} requester ${requesterExitCode === undefined ? "timed out" : `exited (${requesterExitCode})`} before publishing ${pidFile}\nstderr (first 8192 characters):\n${requesterStderr || "(empty)"}`);
    }
    const ownership = JSON.parse(readFileSync(pidFile, "utf8"));
    requester.kill("SIGKILL"); await requester.exited;
    while ((!absent(ownership.process.pid) || !absent(ownership.process.supervisorPid)) && performance.now() - started < 20000) await Bun.sleep(25);
    expect(absent(ownership.process.pid)).toBe(true);
    expect(absent(ownership.process.supervisorPid)).toBe(true);
    const record = JSON.parse(readFileSync(ownership.task.recordPath, "utf8"));
    expect(record.identity.id).toBe(ownership.task.identity.id);
    expect(record.identity.generation).toBe(ownership.task.identity.generation);
    expect(record.ownedProcesses).toEqual([ownership.process]);
    expect(record.cleanup).toMatchObject({ processExited: true, processGroupExited: true, referencesFinalized: mode !== "unobserved" });
    if (mode !== "unobserved") {
      expect(record.state).toBe("closed");
      expect(record.cleanup.ownedWindowsClosed).toBe(true);
      expect(record.result.nativeLifecycle.result.identity.pid).toBe(ownership.process.pid);
      expect(record.result.nativeLifecycle.result.shutdownReason).toBe("inputEof");
    } else {
      expect(record.state).toBe("protected");
      expect(record.cleanup.closed).toBe(false);
      expect(record.cleanup.ownedWindowsClosed).not.toBe(true);
      expect(record.cleanup.failureCodes).toContain("windows_not_observed_closed");
      expect(record.cleanup.failureCodes).toContain("native_references_not_finalized");
    }
  } finally {
    requester.kill(); await requesterExited;
    // Pipe EOF is a real subprocess event; cap diagnostics if a descendant retains stderr.
    await Promise.race([stderrDrained, Bun.sleep(200)]);
  }
}, 25000);

test("failed native binding retains its cause without claiming startup or closure", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "owned-native-bind-failure-"))); paths.push(root);
  const failure = await spawnOwnedProcess({ ...fakeNativeOptions(), ownedNative: { ...nativeExpectation,
    task: { repositoryRoot: root, recordPath: join(root, "task.json"),
      identity: { id: "unbound-fixture", generation: "unbound-generation" }, helperExecutable: Bun.which("false")! } },
  }).then(async child => { await child.close(); throw new Error("unexpected_native_start"); }, error => error);
  expect(failure).toBeInstanceOf(Error);
  expect(failure.cleanup).toMatchObject({ closed: false, referencesFinalized: false, ownedWindowsClosed: null });
  expect(failure.cleanup.failureCodes).toContain("native_task_bind_failed");
  expect(failure.cleanup.failureCodes).toContain("Error: supervisor_identity_missing");
});
