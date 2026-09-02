import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createBuildWorkspace } from "./build-artifact-fixture.ts";
import { spawnOwnedProcess } from "./owned-process.ts";

const scripts = import.meta.dir;
const roots: string[] = [];
const lockPath = (root: string) => join("/tmp/sk-dev-launcher-locks", `${createHash("sha1").update(root).digest("hex")}.lock`);
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

function fixture() {
  const workspace = createBuildWorkspace(mkdtempSync(join(tmpdir(), "script-kit-human-development-")));
  const { root, env } = workspace;
  roots.push(root);
  const bin = join(root, "fixture-bin"), localScripts = join(root, "scripts/agentic");
  const events = join(root, "development-events.jsonl");
  copyFileSync(join(scripts, "../../dev.sh"), join(root, "dev.sh"));
  for (const name of ["dev-cycle.sh", "dev-relaunch.sh", "compiler-input-paths.txt"]) copyFileSync(join(scripts, name), join(localScripts, name));
  mkdirSync(join(root, ".cargo"));
  copyFileSync(join(scripts, "../../.cargo/config.toml"), join(root, ".cargo/config.toml"));
  for (const key of Object.keys(env)) {
    if (/^(SCRIPT_KIT_DEV_|SCRIPT_KIT_CARGO_|CARGO_TARGET_.*_LINKER$|FIXTURE_)/.test(key)) delete env[key];
  }
  Object.assign(env, {
    HOME: join(root, "home"), TMPDIR: join(root, "tmp"), WATCHER_EVENTS: events,
    SCRIPT_KIT_NONINTERACTIVE: "0", SCRIPT_KIT_REPORT_CACHE_SIZE: "0",
    SCRIPT_KIT_USE_LLD: "0", SCRIPT_KIT_USE_SCCACHE: "0",
    SCRIPT_KIT_GPUI_BINARY: join(root, "target-agent/pools/agent-debug/debug/script-kit-gpui"),
    SCRIPT_KIT_DEV_SESSION_NAME: "fixture-dev-watch", SCRIPT_KIT_SESSION_DIR: join(root, "sessions"),
    SCRIPT_KIT_DEV_FORCE_RELAUNCH: "1", SCRIPT_KIT_DEV_STAMP_FILE: "",
    SCRIPT_KIT_DEV_STAMP_DIR: join(root, "stamps"),
  });
  mkdirSync(env.HOME!, { recursive: true });
  mkdirSync(env.TMPDIR!, { recursive: true });
  writeFileSync(events, "");
  writeFileSync(join(localScripts, "ensure-pi-sidecar.sh"), '#!/bin/bash\nprintf \'{"kind":"sidecar"}\\n\' >> "$WATCHER_EVENTS"\n');
  writeFileSync(join(localScripts, "session.sh"), `#!/bin/bash
printf '{"kind":"session","action":"%s","binary":"%s"}\\n' "$1" "$SCRIPT_KIT_GPUI_BINARY" >> "$WATCHER_EVENTS"
if [[ "$1" == "stop" && "\${FIXTURE_SESSION_STOP_REFUSED:-0}" == "1" ]]; then exit 78; fi
printf '{"session":"fixture-dev-watch","status":"started","ready":true}\\n'
`);
  const prelude = `#!${process.execPath}
import { appendFileSync, readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
const args = process.argv.slice(2), env = process.env;
const record = (kind, fields = {}) => appendFileSync(env.WATCHER_EVENTS, JSON.stringify({ kind, ...fields }) + "\\n");
`;
  writeFileSync(join(bin, "cargo-watch"), "#!/bin/bash\nexit 97\n", { mode: 0o755 });
  writeFileSync(join(bin, "cargo"), `${prelude}
record("cargo", { args, target: env.CARGO_TARGET_DIR, binary: env.SCRIPT_KIT_GPUI_BINARY });
if (args[0] === "watch") {
  const key = new Bun.CryptoHasher("sha1").update(process.cwd()).digest("hex");
  const lock = join("/tmp/sk-dev-launcher-locks", key + (env.SCRIPT_KIT_DEV_ALLOW_MULTI === "1" ? "-" + process.ppid : "") + ".lock");
  record("watcher", { lock, pid: readFileSync(join(lock, "pid"), "utf8").trim(), generation: readFileSync(join(lock, "generation"), "utf8").trim(), start: readFileSync(join(lock, "process-start"), "utf8").trim() });
  if (env.FIXTURE_REPLACE_GENERATION === "1") writeFileSync(join(lock, "generation"), "foreign-replacement\\n");
  if (env.FIXTURE_WATCH_ONLY === "1") process.exit(0);
  const result = Bun.spawnSync(["/bin/bash", "-c", args[args.indexOf("-s") + 1]], { env, cwd: process.cwd(), stdout: "pipe", stderr: "pipe" });
  process.stdout.write(result.stdout); process.stderr.write(result.stderr); process.exit(result.exitCode);
}
if (args[0] !== "build") process.exit(96);
if (env.FIXTURE_BUILD_EXIT) process.exit(Number(env.FIXTURE_BUILD_EXIT));
// Model Cargo's environment-over-config precedence; compiler and linker are also fakes.
const config = Bun.TOML.parse(readFileSync(".cargo/config.toml", "utf8"));
const flags = env.CARGO_ENCODED_RUSTFLAGS !== undefined ? env.CARGO_ENCODED_RUSTFLAGS.split("\\x1f") : env.RUSTFLAGS !== undefined ? env.RUSTFLAGS.split(/\\s+/) : config.target['cfg(target_os = "macos")'].rustflags;
const compiler = env.RUSTC_WRAPPER ? [env.RUSTC_WRAPPER, "rustc"] : ["rustc"];
const result = Bun.spawnSync([...compiler, ...flags], { env, stdout: "pipe", stderr: "pipe" });
if (result.exitCode !== 0) { process.stderr.write(result.stderr); process.exit(result.exitCode); }
const output = join(env.CARGO_TARGET_DIR, "debug"); mkdirSync(output, { recursive: true });
writeFileSync(join(output, args[args.indexOf("--bin") + 1]), "#!/bin/bash\\nexit 95\\n", { mode: 0o755 });
`, { mode: 0o755 });
  writeFileSync(join(bin, "rustc"), `${prelude}
if (args[0] === "-vV") { console.log("host: aarch64-apple-darwin"); process.exit(0); }
record("compiler", { args, rustflags: env.RUSTFLAGS ?? null, encoded: env.CARGO_ENCODED_RUSTFLAGS ?? null });
const linker = env.CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER ?? "clang";
const linkArgs = args.filter(arg => arg.startsWith("link-arg=")).map(arg => arg.slice("link-arg=".length));
const result = Bun.spawnSync([linker, ...linkArgs, "fixture.o", "-o", "fixture-output"], { env, stdout: "pipe", stderr: "pipe" });
process.stderr.write(result.stderr); process.exit(result.exitCode);
`, { mode: 0o755 });
  writeFileSync(join(bin, "clang"), `${prelude}\nrecord("linker", { args });\n`, { mode: 0o755 });
  writeFileSync(join(bin, "sccache"), `${prelude}
record("sccache", { args });
const result = Bun.spawnSync(args, { env, stdout: "pipe", stderr: "pipe" });
process.stderr.write(result.stderr); process.exit(result.exitCode);
`, { mode: 0o755 });
  const llvm = join(root, "llvm");
  mkdirSync(join(llvm, "bin"), { recursive: true });
  writeFileSync(join(llvm, "bin/ld64.lld"), "#!/bin/bash\nexit 94\n", { mode: 0o755 });
  writeFileSync(join(bin, "brew"), `#!/bin/bash\nprintf '%s\\n' '${llvm}'\n`, { mode: 0o755 });
  return { ...workspace, events, llvm, records: () => readFileSync(events, "utf8").trim().split("\n").filter(Boolean).map(line => JSON.parse(line)) };
}

function run(workspace: ReturnType<typeof fixture>, script: string, args: string[] = [], interactive = false) {
  const command = ["/bin/bash", join(workspace.root, script), ...args];
  // No host input relay: only the owned child receives the slave terminal.
  const terminal = `import errno, os, pty, select, signal, subprocess, sys, time
master, slave = pty.openpty()
child = subprocess.Popen(sys.argv[1:], stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
os.close(slave)
os.set_blocking(master, False)
deadline = time.monotonic() + 15
timed_out = False
def group_alive():
    try:
        os.killpg(child.pid, 0)
        return True
    except ProcessLookupError:
        return False
def signal_group(sig):
    try:
        os.killpg(child.pid, sig)
    except ProcessLookupError:
        pass
def drain():
    while select.select([master], [], [], 0)[0]:
        try:
            data = os.read(master, 65536)
        except OSError as error:
            if error.errno in (errno.EIO, errno.EAGAIN):
                break
            raise
        if not data:
            break
        sys.stdout.buffer.write(data)
        sys.stdout.buffer.flush()
try:
    while child.poll() is None:
        drain()
        if time.monotonic() >= deadline:
            timed_out = True
            break
        time.sleep(0.01)
finally:
    # The child created this private session/group; never signal a name/CWD census.
    if group_alive():
        signal_group(signal.SIGTERM)
        until = time.monotonic() + 1
        while group_alive() and time.monotonic() < until:
            child.poll()
            drain()
            time.sleep(0.01)
        if group_alive():
            signal_group(signal.SIGKILL)
    child.wait(timeout=1)
    until = time.monotonic() + 1
    while group_alive() and time.monotonic() < until:
        drain()
        time.sleep(0.01)
    drain()
    os.close(master)
if group_alive():
    print("fixture process group cleanup unconfirmed", file=sys.stderr)
    sys.exit(125)
sys.exit(124 if timed_out else child.returncode if child.returncode >= 0 else 128 - child.returncode)
`;
  const argv = interactive ? ["python3", "-c", terminal, ...command] : command;
  const result = Bun.spawnSync(argv, { cwd: workspace.root, env: workspace.env, stdin: "ignore", stdout: "pipe", stderr: "pipe", timeout: 20_000 });
  return { status: result.exitCode, stdout: result.stdout.toString(), stderr: result.stderr.toString() };
}

describe("human development shell behavior", () => {
  test.each(["0", "1"])("authorized fake watcher preserves default sidecar, dev binary, sccache and root linker flags (lld=%s)", (lld) => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_USE_LLD: lld!, SCRIPT_KIT_USE_SCCACHE: "1" });
    const result = run(workspace, "dev.sh", [], true);
    expect(result.status, result.stdout + result.stderr).toBe(0);
    const records = workspace.records();
    expect(records[0]).toEqual({ kind: "sidecar" });
    expect(records.filter(record => record.kind === "sidecar")).toHaveLength(1);
    const watcher = records.find(record => record.kind === "watcher");
    expect(watcher.pid).toMatch(/^\d+$/);
    expect(watcher.generation).not.toBe("");
    expect(watcher.start).not.toBe("");
    const builds = records.filter(record => record.kind === "cargo" && record.args[0] === "build");
    expect(builds).toHaveLength(2);
    for (const build of builds) expect(build).toMatchObject({ target: join(workspace.root, "target"), binary: join(workspace.root, "target/debug/script-kit-gpui") });
    expect(builds[1].args).toEqual(["build", "--locked", "--bin", "script-kit-gpui", "--features", "local-llm", "--message-format=short"]);
    expect(records.filter(record => record.kind === "session")).toEqual(["stop", "start"].map(action => ({ kind: "session", action, binary: join(workspace.root, "target/debug/script-kit-gpui") })));
    expect(records.filter(record => record.kind === "sccache")).toHaveLength(2);
    const compilers = records.filter(record => record.kind === "compiler");
    expect(compilers).toHaveLength(2);
    for (const compiler of compilers) {
      expect(compiler).toMatchObject({ rustflags: null, encoded: null });
      expect(compiler.args).toEqual(["-C", "split-debuginfo=unpacked", "-C", "link-arg=-Wl,-dead_strip"]);
    }
    const links = records.filter(record => record.kind === "linker");
    expect(links).toHaveLength(2);
    for (const link of links) expect(link.args).toEqual([...(lld === "1" ? [`-fuse-ld=${workspace.llvm}/bin/ld64.lld`] : []), "-Wl,-dead_strip", "fixture.o", "-o", "fixture-output"]);
    expect(existsSync(lockPath(workspace.root))).toBe(false);
  }, 30_000);

  test("inspection and noninteractive refusals never provision, build or delete inherited stamps", () => {
    const workspace = fixture(), stamp = join(workspace.root, "protected.stamp");
    writeFileSync(stamp, "protected\n");
    workspace.env.SCRIPT_KIT_DEV_STAMP_FILE = stamp;
    workspace.env.SCRIPT_KIT_NONINTERACTIVE = "1";
    for (const args of [["--status"], ["--help"], ["--help", "--takeover"]]) {
      const result = run(workspace, "dev.sh", args);
      expect(result.status, result.stderr).toBe(0);
      expect(workspace.records()).toEqual([]);
      expect(readFileSync(stamp, "utf8")).toBe("protected\n");
    }
    for (const args of [[], ["--stop"], ["--takeover"]]) {
      const result = run(workspace, "dev.sh", args, true);
      expect(result.status, result.stdout + result.stderr).toBe(78);
      expect(workspace.records()).toEqual([]);
      expect(readFileSync(stamp, "utf8")).toBe("protected\n");
    }
    expect(existsSync(workspace.env.SCRIPT_KIT_SESSION_DIR!)).toBe(false);
  }, 30_000);

  test.each(["CARGO_TARGET_DIR", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER", "RUSTC_WRAPPER"])("dev cycle refuses conflicting inherited %s before compilation or relaunch", (key) => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_USE_LLD: "1", SCRIPT_KIT_USE_SCCACHE: "1", [key!]: key === "CARGO_TARGET_DIR" ? join(workspace.root, "target-agent/pools/agent-debug") : "conflicting-inherited-value" });
    const result = run(workspace, "scripts/agentic/dev-cycle.sh");
    expect(result.status, result.stderr).toBe(78);
    expect(result.stderr).toContain("REFUSED conflicting");
    expect(workspace.records()).toEqual([]);
  });

  test("empty feature opt-out builds the explicit dev binary and relaunches under system Bash", () => {
    const workspace = fixture();
    workspace.env.SCRIPT_KIT_CARGO_FEATURES = "";
    const result = run(workspace, "dev.sh", [], true);
    expect(result.status, result.stdout + result.stderr).toBe(0);
    const builds = workspace.records().filter(record => record.kind === "cargo" && record.args[0] === "build");
    expect(builds).toHaveLength(1);
    expect(builds[0]).toMatchObject({ args: ["build", "--locked", "--bin", "script-kit-gpui", "--message-format=short"], target: join(workspace.root, "target"), binary: join(workspace.root, "target/debug/script-kit-gpui") });
    expect(workspace.records().filter(record => record.kind === "session").map(record => record.action)).toEqual(["stop", "start"]);
    expect(existsSync(lockPath(workspace.root))).toBe(false);
  }, 30_000);

  test("failed dev builds preserve their exit status through heartbeat cleanup without relaunch", () => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_CARGO_FEATURES: "", FIXTURE_BUILD_EXIT: "42" });
    const result = run(workspace, "scripts/agentic/dev-cycle.sh");
    expect(result.status, result.stderr).toBe(42);
    expect(result.stderr).toContain("status=42");
    expect(workspace.records().filter(record => record.kind === "session")).toEqual([]);
  });

  test("dev relaunch does not start after previous exact session teardown refuses", () => {
    const workspace = fixture();
    workspace.env.FIXTURE_SESSION_STOP_REFUSED = "1";
    const result = run(workspace, "scripts/agentic/dev-cycle.sh");
    expect(result.status, result.stderr).toBe(78);
    expect(result.stderr).toContain("previous exact session teardown not confirmed");
    expect(workspace.records().filter(record => record.kind === "session").map(record => record.action)).toEqual(["stop"]);
  });

  test("repo-scoped duplicate refusal preserves the owner while explicit multi cleans only its own lease", async () => {
    const workspace = fixture(), lock = lockPath(workspace.root);
    const child = await spawnOwnedProcess({ argv: ["/bin/cat"], cwd: workspace.root, env: workspace.env, timeoutMs: 30_000, maxOutputBytes: 4096 });
    let registered = false;
    try {
      const start = Bun.spawnSync(["/bin/ps", "-p", String(child.pid), "-o", "lstart="], { env: { ...workspace.env, LC_ALL: "C" }, stdout: "pipe", stderr: "pipe" });
      expect(start.exitCode).toBe(0);
      mkdirSync("/tmp/sk-dev-launcher-locks", { recursive: true });
      mkdirSync(lock); registered = true;
      const lease = { pid: `${child.pid}\n`, "process-start": `${start.stdout.toString().trim()}\n`, generation: "fixture-owner\n", root: `${workspace.root}\n`, session: "fixture-owner\n" };
      for (const [name, value] of Object.entries(lease)) writeFileSync(join(lock, name), value);
      const stamp = join(workspace.root, "inherited.stamp");
      writeFileSync(stamp, "protected\n");
      workspace.env.SCRIPT_KIT_DEV_STAMP_FILE = stamp;
      const duplicate = run(workspace, "dev.sh", [], true);
      expect(duplicate.status, duplicate.stdout + duplicate.stderr).toBe(2);
      expect(duplicate.stdout + duplicate.stderr).toContain("already running");
      expect(workspace.records()).toEqual([]);
      expect(readFileSync(stamp, "utf8")).toBe("protected\n");
      Object.assign(workspace.env, { SCRIPT_KIT_DEV_ALLOW_MULTI: "1", FIXTURE_WATCH_ONLY: "1" });
      const multi = run(workspace, "dev.sh", [], true);
      expect(multi.status, multi.stdout + multi.stderr).toBe(0);
      const watcher = workspace.records().find(record => record.kind === "watcher");
      expect(watcher.lock).not.toBe(lock);
      expect(existsSync(watcher.lock)).toBe(false);
      expect(() => process.kill(child.pid, 0)).not.toThrow();
      for (const [name, value] of Object.entries(lease)) expect(readFileSync(join(lock, name), "utf8")).toBe(value);
      expect(readFileSync(stamp, "utf8")).toBe("protected\n");
      workspace.env.SCRIPT_KIT_DEV_ALLOW_MULTI = "0";
      lease["process-start"] = "foreign-process-lifetime\n";
      writeFileSync(join(lock, "process-start"), lease["process-start"]);
      const beforeRefusal = readFileSync(workspace.events, "utf8");
      for (const args of [["--stop"], ["--takeover"]]) {
        const refused = run(workspace, "dev.sh", args, true);
        expect(refused.status, refused.stdout + refused.stderr).toBe(78);
        expect(() => process.kill(child.pid, 0)).not.toThrow();
        expect(readFileSync(workspace.events, "utf8")).toBe(beforeRefusal);
        expect(readFileSync(stamp, "utf8")).toBe("protected\n");
        for (const [name, value] of Object.entries(lease)) expect(readFileSync(join(lock, name), "utf8")).toBe(value);
      }
    } finally {
      await child.close();
      if (registered) rmSync(lock, { recursive: true, force: true });
    }
  }, 40_000);

  test("authorized sidecar opt-out leaves provisioning disabled while watcher operation continues", () => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_DEV_ENSURE_PI_SIDECAR: "0", FIXTURE_WATCH_ONLY: "1" });
    const result = run(workspace, "dev.sh", [], true);
    expect(result.status, result.stdout + result.stderr).toBe(0);
    expect(workspace.records().filter(record => record.kind === "sidecar")).toEqual([]);
    expect(workspace.records().filter(record => record.kind === "watcher")).toHaveLength(1);
    expect(existsSync(lockPath(workspace.root))).toBe(false);
  }, 30_000);

  test("build-ops inspection neither provisions nor health-probes the sidecar", () => {
    const workspace = createBuildWorkspace(mkdtempSync(join(tmpdir(), "script-kit-passive-sidecar-")));
    roots.push(workspace.root);
    const events = join(workspace.root, "sidecar-events"), sidecar = join(workspace.root, "target/pi-sidecar/pi");
    mkdirSync(join(workspace.root, "target/pi-sidecar"), { recursive: true });
    const sentinel = "#!/bin/bash\nprintf 'unexpected invocation\\n' >> \"$SIDECAR_EVENTS\"\nexit 93\n";
    writeFileSync(sidecar, sentinel, { mode: 0o755 });
    writeFileSync(join(workspace.root, "scripts/agentic/ensure-pi-sidecar.sh"), sentinel, { mode: 0o755 });
    const result = Bun.spawnSync([process.execPath, join(scripts, "../devtools/build-ops.ts"), "inspect"], {
      cwd: workspace.root,
      env: { ...workspace.env, HOME: workspace.root, SIDECAR_EVENTS: events, SCRIPT_KIT_PI_BINARY: sidecar },
      stdin: "ignore", stdout: "pipe", stderr: "pipe", timeout: 20_000,
    });
    expect(result.exitCode, result.stderr.toString()).toBe(0);
    const receipt = JSON.parse(result.stdout.toString());
    expect(receipt.buildOps.result).toMatchObject({ performedInstallation: false, performedBuild: false, dependencies: { pi: { health: "not-probed" } } });
    expect(existsSync(events)).toBe(false);
    expect(existsSync(workspace.invocations)).toBe(false);
    expect(readFileSync(sidecar, "utf8")).toBe(sentinel);
  }, 30_000);

  test("watcher cleanup retains a foreign replacement generation", () => {
    const workspace = fixture(), lock = lockPath(workspace.root);
    Object.assign(workspace.env, { FIXTURE_WATCH_ONLY: "1", FIXTURE_REPLACE_GENERATION: "1" });
    try {
      const result = run(workspace, "dev.sh", [], true);
      expect(result.status, result.stdout + result.stderr).toBe(0);
      const watcher = workspace.records().find(record => record.kind === "watcher");
      expect(readFileSync(join(lock, "pid"), "utf8").trim()).toBe(watcher.pid);
      expect(readFileSync(join(lock, "generation"), "utf8")).toBe("foreign-replacement\n");
      expect(readFileSync(join(lock, "process-start"), "utf8").trim()).toBe(watcher.start);
    } finally { rmSync(lock, { recursive: true, force: true }); }
  }, 30_000);
});
