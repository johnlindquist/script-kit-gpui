import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createArtifactFixture, createBuildWorkspace } from "./build-artifact-fixture.ts";
import type { ArtifactFixture } from "./build-artifact-fixture.ts";
import { artifactHash } from "./build-artifact.ts";
import { spawnOwnedProcess } from "./owned-process.ts";

interface PolicyWorkspace { root: string; bin: string; capture: string; env: Record<string, string>; }
function wrapperRecord(result: { stdout: string }) {
  const outcome = JSON.parse(result.stdout);
  return JSON.parse(readFileSync(outcome.recordPath, "utf8"));
}

const scripts = import.meta.dir;
const temporaryDirectories: string[] = [];
const temporaryArtifacts: ArtifactFixture[] = [];

function temporaryDirectory(prefix: string) {
  const path = mkdtempSync(join(tmpdir(), prefix));
  temporaryDirectories.push(path);
  return path;
}

afterEach(() => {
  // Restore permissions only inside roots created by this test; never follow fixture symlinks.
  const writable = (path: string): void => {
    const stat = lstatSync(path);
    if (stat.isDirectory() && !stat.isSymbolicLink()) {
      chmodSync(path, 0o700);
      for (const child of readdirSync(path)) writable(join(path, child));
    }
  };
  for (const artifact of temporaryArtifacts.splice(0)) artifact.dispose();
  for (const path of temporaryDirectories.splice(0)) {
    writable(path);
    rmSync(path, { recursive: true, force: true });
  }
});

function fixture(): PolicyWorkspace {
  const workspace = createBuildWorkspace(temporaryDirectory("script-kit-cargo-policy-"));
  const { root } = workspace;
  const bin = join(root, "fixture-bin"), localScripts = join(root, "scripts/agentic");
  for (const path of ["target-agent/.locks", "target-agent/pools/agent-debug", "target-agent/shared", "target-agent/artifacts"]) mkdirSync(join(root, path), { recursive: true });
  for (const script of ["cargo-cache-locks.sh", "prune-cargo-targets.sh", "disk-space-cargo-emergency-clean.sh"]) symlinkSync(join(scripts, script), join(localScripts, script));
  const capture = join(root, "cargo-invocation.txt");
  copyFileSync(join(bin, "cargo"), join(bin, "cargo-fixture"));
  writeFileSync(join(bin, "cargo"), `#!/bin/bash
if [[ "$1" == "-V" ]]; then exec "${bin}/cargo-fixture" "$@"; fi
printf 'jobs=%s\\ntest_threads=%s\\nnoninteractive=%s\\nsearch_stress=%s\\nstorage_stress=%s\\nmodule=%s\\nwrapper=%s\\nsocket=%s\\ntakeover=%s input=%s capture=%s visible=%s live_ai=%s app=%s\\nargs=' "$CARGO_BUILD_JOBS" "\${RUST_TEST_THREADS:-}" "\${SCRIPT_KIT_NONINTERACTIVE:-}" "\${SCRIPT_KIT_SEARCH_FULL_STRESS:-}" "\${SCRIPT_KIT_STORAGE_FULL_STRESS:-}" "$SCRIPT_KIT_METAL_MODULE_CACHE_DIR" "\${RUSTC_WRAPPER:-}" "\${SCCACHE_SERVER_UDS:-}" "\${SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER:-}" "\${SCRIPT_KIT_ALLOW_NATIVE_INPUT:-}" "\${SCRIPT_KIT_ALLOW_SCREEN_CAPTURE:-}" "\${SCRIPT_KIT_ALLOW_VISIBLE_PROBES:-}" "\${SCRIPT_KIT_ALLOW_LIVE_AI:-}" "\${SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH:-}" > "$CARGO_POLICY_CAPTURE"
printf '%s ' "$@" >> "$CARGO_POLICY_CAPTURE"
printf '\\n' >> "$CARGO_POLICY_CAPTURE"
if [[ "\${POLICY_EXERCISE_COMPILER:-0}" == "1" ]]; then
  if [[ -n "\${RUSTC_WRAPPER:-}" ]]; then "$RUSTC_WRAPPER" "$RUSTC" --fixture-compile; else "$RUSTC" --fixture-compile; fi
fi
exec "${bin}/cargo-fixture" "$@"
`);
  chmodSync(join(bin, "cargo"), 0o755);
  for (const key of ["CARGO_BUILD_BUILD_DIR", "CARGO_RESOLVER_LOCKFILE_PATH", "SCRIPT_KIT_AGENT_MAX_JOBS", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"]) delete workspace.env[key];
  return { root, bin, capture, env: { ...workspace.env, CARGO_POLICY_CAPTURE: capture, SCRIPT_KIT_AGENT_CRITICAL_FREE_GB: "0" } };
}

function makeOld(directory: string) {
  mkdirSync(directory, { recursive: true });
  const old = new Date(Date.now() - 20 * 24 * 60 * 60 * 1000);
  utimesSync(directory, old, old);
}

function liveLock(root: string, name: string, writePid = true) {
  const directory = join(root, "target-agent", ".locks", `pool-${name}.lock`);
  mkdirSync(directory, { recursive: true });
  if (writePid) writeFileSync(join(directory, "pid"), `${process.pid}\n`);
  return directory;
}

function run(script: string, args: string[], env: NodeJS.ProcessEnv) {
  const result = Bun.spawnSync(["/bin/bash", join(scripts, script), ...args], {
    env,
    stdout: "pipe",
    stderr: "pipe",
  });
  return {
    status: result.exitCode,
    stdout: result.stdout.toString(),
    stderr: result.stderr.toString(),
  };
}

function installFakeSccache(workspace: PolicyWorkspace, usable = true) {
  writeFileSync(join(workspace.bin, "sccache"), `#!/bin/bash\nif [[ "$1" == "--show-stats" ]]; then exit 0; fi\n${usable ? 'exec "$@"' : "exit 2"}\n`, { mode: 0o755 });
}

function installCompilerCapture(workspace: PolicyWorkspace): string {
  const capture = join(workspace.root, "compiler-arguments.txt");
  copyFileSync(join(workspace.bin, "rustc"), join(workspace.bin, "rustc-version"));
  writeFileSync(join(workspace.bin, "rustc"), `#!/bin/bash
if [[ "$1" == "-vV" ]]; then exec "${workspace.bin}/rustc-version" "$@"; fi
printf '%s\\n' "$@" >> "${capture}"
`, { mode: 0o755 });
  return capture;
}

function semanticWrapper(path: string, cfg: string): void {
  writeFileSync(path, `#!/bin/bash\ncompiler="$1"; shift\nexec "$compiler" --cfg ${cfg} "$@"\n`, { mode: 0o755 });
}

function verifyWrapperArtifact(workspace: PolicyWorkspace, reference: unknown, env: NodeJS.ProcessEnv) {
  const script = `import { verifyImmutableArtifact, ArtifactVerificationError } from ${JSON.stringify(join(scripts, "build-artifact.ts"))};
try {
  verifyImmutableArtifact(process.env.SCRIPT_KIT_REPO_ROOT, ${JSON.stringify(reference)}, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" });
  console.log(JSON.stringify({ verified: true }));
} catch (error) {
  if (!(error instanceof ArtifactVerificationError)) throw error;
  console.log(JSON.stringify({ code: error.code, disposition: error.disposition }));
}`;
  const result = Bun.spawnSync([process.execPath, "-e", script], { cwd: workspace.root, env, stdout: "pipe", stderr: "pipe" });
  expect(result.exitCode).toBe(0);
  return JSON.parse(result.stdout.toString());
}

function installFakeSystemLoad(
  workspace: PolicyWorkspace,
  load: string,
  logicalCpus: string,
) {
  writeFileSync(
    join(workspace.bin, "uptime"),
    `#!/bin/bash\nprintf '03:00 2 users, load averages: ${load} 1.00 1.00\\n'\n`,
  );
  writeFileSync(
    join(workspace.bin, "getconf"),
    `#!/bin/bash\nprintf '${logicalCpus}\\n'\n`,
  );
  chmodSync(join(workspace.bin, "uptime"), 0o755);
  chmodSync(join(workspace.bin, "getconf"), 0o755);
}



describe("bounded Cargo builds", () => {
  test("local release verification defaults to the bounded agent Cargo wrapper", () => {
    const workspace = fixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(scripts, "..", "verify.sh"), "--skip-bundle", "--only", "check"],
      {
        env: {
          ...workspace.env,
          SCRIPT_KIT_CARGO: "",
          CI: "",
          GITHUB_ACTIONS: "",
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("agent-cargo.sh check --locked");
    expect(result.stderr.toString()).toContain("AGENT_CARGO mode=pool");
    expect(readFileSync(workspace.capture, "utf8")).toContain("jobs=2\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("test_threads=2\n");
  });

  test.each([
    ["proof-contracts", true],
    ["sdk-tests", false],
  ] as const)("release verifier owns nested %s output destinations (capture=%s)", (phase, captureLog) => {
    const workspace = fixture();
    const runnerBin = join(workspace.bin, "verify-runner");
    const runnerScript = join(runnerBin, "verify-fixture.ts");
    mkdirSync(runnerBin);
    const testLog = join(runnerBin, "parent.log");
    const receiptPath = join(runnerBin, "parent-receipt.json");
    const childCapture = join(runnerBin, "child.json");
    const environmentCapture = join(runnerBin, "environments.jsonl");
    const verify = join(scripts, "..", "verify.sh");
    writeFileSync(receiptPath, "parent receipt pending\n");
    const reporterXml = [
      '<?xml version="1.0" encoding="UTF-8"?>',
      '<testsuites name="bun test" tests="1" assertions="1" failures="0" skipped="0" time="0">',
      '  <testsuite name="scripts/agentic/cargo-build-policy.test.ts" file="scripts/agentic/cargo-build-policy.test.ts" tests="1" assertions="1" failures="0" skipped="0" time="0" hostname="fixture.invalid">',
      '    <testcase name="nested verifier fixture" classname="bounded Cargo builds" file="scripts/agentic/cargo-build-policy.test.ts" line="184" assertions="1" time="0" />',
      '  </testsuite>',
      '</testsuites>',
    ].join("\n");
    // Replace only suite execution and receipt publication; the nested verifier
    // still uses the real bounded Cargo wrapper and this test's Cargo fixture.
    writeFileSync(join(runnerBin, "bun"), `#!/bin/bash\nexec "${process.execPath}" "${runnerScript}" "$@"\n`, { mode: 0o755 });
    writeFileSync(runnerScript, `
import { existsSync, readFileSync, writeFileSync } from "node:fs";
const args = process.argv.slice(2);
const reporterIndex = args.indexOf("--reporter-outfile");
const reporterPath = reporterIndex >= 0 ? args[reporterIndex + 1] : undefined;
const reporter = reporterPath ? { path: reporterPath, existed: existsSync(reporterPath) } : null;
const destinations = {
  log: "SCRIPT_KIT_VERIFY_TEST_LOG" in process.env,
  receipt: "SCRIPT_KIT_VERIFY_RECEIPT" in process.env,
  dirtyDiagnostic: "SCRIPT_KIT_ALLOW_DIRTY_DIAGNOSTIC_EVIDENCE" in process.env,
  dirtyOwners: "SCRIPT_KIT_DIRTY_EVIDENCE_OWNER_PATHS" in process.env,
};
if (args[0] === ${JSON.stringify(join(scripts, "..", "release-evidence.ts"))}) {
  const output = args[args.indexOf("--output") + 1];
  const result = args.includes("--result") ? args[args.indexOf("--result") + 1] : undefined;
  writeFileSync(output, JSON.stringify({ args, destinations,
    previousReceipt: readFileSync(output, "utf8"),
    log: result ? readFileSync(result, "utf8") : null,
  }));
  process.exit(0);
}
writeFileSync(${JSON.stringify(environmentCapture)}, JSON.stringify({ args, destinations, reporter }) + "\\n", { flag: "a" });
if (args[0] === "scripts/generate-surface-contracts.ts" || args[0] === "scripts/devtools/family-fixtures.ts") process.exit(0);
if (args[0] !== "test" && !(args[0] === "run" && args[1] === "scripts/test-runner.ts")) process.exit(64);
if (args[0] === "test" && (!args.includes("--reporter=junit") || !reporterPath?.startsWith(${JSON.stringify(`${runnerBin}/`)}))) process.exit(64);
console.log("suite before nested verifier");
const childArgs = ["/bin/bash", ${JSON.stringify(verify)}, "--skip-bundle", "--only", "check"];
const childEnv = { ...process.env, PATH: ${JSON.stringify(workspace.env.PATH)}, SCRIPT_KIT_CARGO: "" };
const child = Bun.spawnSync(childArgs, {
  env: childEnv,
  stdout: "pipe", stderr: "pipe",
});
writeFileSync(${JSON.stringify(childCapture)}, JSON.stringify({
  status: child.exitCode, stdout: child.stdout.toString(), stderr: child.stderr.toString(),
  args: childArgs,
  inheritedReporter: reporterPath ? Object.values(childEnv).includes(reporterPath) : false,
  reporterExistedAfterChild: reporterPath ? existsSync(reporterPath) : false,
}));
console.log(child.stdout.toString());
console.error(child.stderr.toString());
console.log("suite after nested verifier");
if (child.exitCode === 0 && reporterPath) writeFileSync(reporterPath, ${JSON.stringify(reporterXml)}, { flag: "wx" });
process.exit(child.exitCode);
`);
    const result = Bun.spawnSync(["/bin/bash", verify, "--skip-bundle", "--only", phase], {
      env: {
        ...workspace.env,
        PATH: `${runnerBin}:${workspace.env.PATH}`,
        TMPDIR: runnerBin,
        SCRIPT_KIT_CARGO: "",
        SCRIPT_KIT_VERIFY_TEST_LOG: captureLog ? testLog : "",
        SCRIPT_KIT_VERIFY_RECEIPT: receiptPath,
        SCRIPT_KIT_SDK_TEST_RECEIPT: "",
        SCRIPT_KIT_REQUIRE_CLEAN_SOURCE: "0",
        SCRIPT_KIT_ALLOW_DIRTY_DIAGNOSTIC_EVIDENCE: "1",
        SCRIPT_KIT_DIRTY_EVIDENCE_OWNER_PATHS: "scripts/agentic/cargo-build-policy.test.ts",
        GITHUB_SHA: "a".repeat(40),
        CI: "",
        GITHUB_ACTIONS: "",
      },
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode, `stdout:\n${result.stdout.toString()}\nstderr:\n${result.stderr.toString()}`).toBe(0);
    const child = JSON.parse(readFileSync(childCapture, "utf8"));
    expect(child.status, `stdout:\n${child.stdout}\nstderr:\n${child.stderr}`).toBe(0);
    expect(child.stdout).toContain("agent-cargo.sh check --locked");
    expect(child.stdout).not.toContain("AGENT_CARGO mode=pool");
    expect(child.stderr).toContain("AGENT_CARGO mode=pool");
    expect(child.args).toEqual(["/bin/bash", verify, "--skip-bundle", "--only", "check"]);
    expect(child.inheritedReporter).toBe(false);
    expect(child.reporterExistedAfterChild).toBe(false);
    const environments = readFileSync(environmentCapture, "utf8").trim().split("\n").map(line => JSON.parse(line));
    expect(environments).toHaveLength(phase === "proof-contracts" ? 3 : 1);
    for (const environment of environments) expect(environment.destinations).toEqual({
      log: false, receipt: false, dirtyDiagnostic: false, dirtyOwners: false,
    });
    const suiteEnvironment = environments[environments.length - 1];
    if (phase === "proof-contracts") {
      expect(suiteEnvironment.args.slice(0, 6)).toEqual([
        "test", "--isolate", "--timeout", "30000", "--reporter=junit", "--reporter-outfile",
      ]);
      const reporterPath = suiteEnvironment.args[6];
      expect(suiteEnvironment.reporter).toEqual({ path: reporterPath, existed: false });
      expect(dirname(dirname(reporterPath))).toBe(runnerBin);
      expect(existsSync(reporterPath)).toBe(false);
      expect(readFileSync(workspace.capture, "utf8")).not.toContain(reporterPath);
      expect(child.stdout).not.toContain(reporterPath);
      expect(child.stderr).not.toContain(reporterPath);
    } else {
      expect(suiteEnvironment.reporter).toBeNull();
    }
    const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
    expect(receipt.args).toEqual(expect.arrayContaining(["--gate", phase, "--output", receiptPath]));
    expect(receipt.args).toEqual(expect.arrayContaining([
      "--diagnostic-dirty", "--owner", "scripts/agentic/cargo-build-policy.test.ts",
    ]));
    expect(receipt.previousReceipt).toBe("parent receipt pending\n");
    expect(receipt.destinations).toEqual({ log: true, receipt: true, dirtyDiagnostic: true, dirtyOwners: true });
    const output = captureLog ? readFileSync(testLog, "utf8") : result.stdout.toString();
    expect(output).toContain("suite before nested verifier");
    expect(output).toContain("suite after nested verifier");
    if (captureLog) {
      expect(output).toContain("AGENT_CARGO mode=pool");
      expect(output).toContain(`[verify] BEGIN bun-junit\n${reporterXml}\n[verify] END bun-junit\n`);
      expect(receipt.log).toBe(output);
    } else {
      expect(result.stderr.toString()).toContain("AGENT_CARGO mode=pool");
      expect(existsSync(testLog)).toBe(false);
      expect(receipt.log).toBeNull();
    }
  });

  test("hosted GitHub verification keeps its isolated Cargo target and bounded workers", () => {
    const workspace = fixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(scripts, "..", "verify.sh"), "--skip-bundle", "--only", "check"],
      {
        env: {
          ...workspace.env,
          SCRIPT_KIT_CARGO: "",
          CI: "true",
          GITHUB_ACTIONS: "true",
          RUST_TEST_THREADS: "1",
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("check :: cargo check --locked");
    expect(result.stderr.toString()).not.toContain("AGENT_CARGO mode=pool");
    expect(readFileSync(workspace.capture, "utf8")).toContain("jobs=2\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("test_threads=1\n");
  });

  test.each([
    ["true", ""],
    ["", "true"],
  ])("partial CI identity cannot bypass the local Cargo wrapper (%s, %s)", (ci, githubActions) => {
    const workspace = fixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(scripts, "..", "verify.sh"), "--skip-bundle", "--only", "check"],
      {
        env: {
          ...workspace.env,
          SCRIPT_KIT_CARGO: "",
          CI: ci,
          GITHUB_ACTIONS: githubActions,
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("agent-cargo.sh check --locked");
    expect(result.stderr.toString()).toContain("AGENT_CARGO mode=pool");
  });

  test("release verifier refuses hostile compiler or harness concurrency before any Cargo starts", () => {
    for (const [variable, value] of [
      ["CARGO_BUILD_JOBS", "48"],
      ["RUST_TEST_THREADS", "24"],
      ["CARGO_BUILD_JOBS", "0"],
      ["RUST_TEST_THREADS", "invalid"],
    ]) {
      const workspace = fixture();
      const result = Bun.spawnSync(
        ["/bin/bash", join(scripts, "..", "verify.sh"), "--skip-bundle", "--only", "check"],
        {
          env: { ...workspace.env, SCRIPT_KIT_CARGO: "", [variable!]: value },
          stdout: "pipe",
          stderr: "pipe",
        },
      );

      expect(result.exitCode).toBe(78);
      expect(result.stderr.toString()).toContain("permits only one or two workers");
      expect(existsSync(workspace.capture)).toBe(false);
    }
  });

  test("release verifier disables heavyweight inherited search and storage stress", () => {
    const workspace = fixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(scripts, "..", "verify.sh"), "--skip-bundle", "--only", "check"],
      {
        env: {
          ...workspace.env,
          SCRIPT_KIT_CARGO: join(workspace.bin, "cargo"),
          SCRIPT_KIT_SEARCH_FULL_STRESS: "1",
          SCRIPT_KIT_STORAGE_FULL_STRESS: "1",
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain("search_stress=0\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("storage_stress=0\n");
  });

  test("release verifier strictly lints both the library and shipping binary", () => {
    const workspace = fixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(scripts, "..", "verify.sh"), "--skip-bundle", "--only", "clippy"],
      {
        env: {
          ...workspace.env,
          SCRIPT_KIT_CARGO: join(workspace.bin, "cargo"),
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain(
      "args=clippy --locked --lib --bin script-kit-gpui --no-deps -- -D warnings ",
    );
  });

  test("agent-scoped verification rejects inherited desktop permissions before Cargo runs", () => {
    for (const permission of [
      "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
      "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
      "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
      "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
      "SCRIPT_KIT_ALLOW_LIVE_AI",
      "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
    ]) {
      const workspace = fixture();
      const result = Bun.spawnSync(
        ["/bin/bash", join(scripts, "..", "agent-check.sh"), "--quick"],
        {
          env: { ...workspace.env, [permission]: "1" },
          stdout: "pipe",
          stderr: "pipe",
        },
      );

      expect(result.exitCode).toBe(78);
      expect(result.stderr.toString()).toContain(permission);
      expect(existsSync(workspace.capture)).toBe(false);
    }
  });

  test("agent-scoped verification routes app edits without guessed filters and owns safe permissions", () => {
    const workspace = fixture();
    const result = Bun.spawnSync(["/bin/bash", join(scripts, "..", "agent-check.sh"), "src/ai/reliability.rs"], {
      env: { ...workspace.env, SCRIPT_KIT_NONINTERACTIVE: "0", SCRIPT_KIT_SEARCH_FULL_STRESS: "1", SCRIPT_KIT_STORAGE_FULL_STRESS: "1" }, stdout: "pipe", stderr: "pipe",
    });
    expect(result.exitCode, result.stderr.toString()).toBe(0);
    const calls = readFileSync(join(workspace.root, "cargo-invocations.jsonl"), "utf8").trim().split("\n").map(line => JSON.parse(line));
    expect(calls.map(call => call.args[0])).toEqual(["check", "clippy", "test", "test", "test"]);
    expect(calls[2].args).toEqual(expect.arrayContaining(["--lib", "--no-run", "--message-format=json-render-diagnostics"]));
    const completed = JSON.parse(result.stdout.toString()).buildOps.result.completed;
    expect(completed.map((operation: { task: { kind: string } }) => operation.task.kind)).toEqual(["build-job", "build-job", "build-job", "runtime-run", "build-job", "build-job"]);
    const libtestRun = completed[3];
    expect(libtestRun).toMatchObject({ status: "succeeded", passedTests: 1, failedTests: 0, cleanup: { closed: true } });
    expect(libtestRun.artifact).toEqual(completed[2].artifact);
    expect(libtestRun.artifact).toEqual({ manifestPath: expect.stringMatching(/^target-agent\/artifacts\//), manifestSha256: expect.stringMatching(/^[a-f0-9]{64}$/) });
    const libtestRecord = JSON.parse(readFileSync(join(workspace.root, ".test-output/managed-tasks", libtestRun.task.id, "task.json"), "utf8"));
    expect(libtestRecord).toMatchObject({ state: "closed", artifactReferences: [libtestRun.artifact], cleanup: { closed: true } });
    expect(calls.some(call => call.args.includes("reliability"))).toBe(false);
    expect(readFileSync(workspace.capture, "utf8")).toContain("takeover=0 input=0 capture=0 visible=0 live_ai=0 app=0");
  }, 30000);

  test("uses two workers, a persistent sandbox-safe shader cache, timings, and a truthful receipt", () => {
    const workspace = fixture();
    const result = run(
      "agent-cargo.sh",
      ["test", "--lib", "--", "reviewed_filter"],
      { ...workspace.env, SCRIPT_KIT_AGENT_TIMINGS: "1" },
    );

    expect(result.status).toBe(0);
    const invocation = readFileSync(workspace.capture, "utf8");
    expect(invocation).toContain("jobs=2\n");
    expect(invocation).toContain("test_threads=2\n");
    expect(invocation).toContain(
      `module=${join(workspace.root, "target-agent", "shared", "clang-modules")}\n`,
    );
    expect(invocation).toContain("args=test --lib --timings -- reviewed_filter ");

    const record = wrapperRecord(result);
    expect(record).toMatchObject({ state: "closed", cleanup: { closed: true }, result: { status: "succeeded", passedTests: 1 } });
    expect(record.effectiveConfiguration.requestedPolicy).toMatchObject({ pool: "agent-debug", compilerWorkers: 2, testWorkers: 2 });
    expect(record.effectiveConfiguration.compilerCache).toBe("disabled");
  });

  test("defers a CPU-intensive build before Cargo starts when the explicit machine budget is full", () => {
    const workspace = fixture();
    installFakeSystemLoad(workspace, "11.00", "16");
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT: "75",
    });

    expect(result.status).toBe(75);
    expect(result.stderr).toContain("machine CPU pressure exceeds the explicit 75% budget");
    expect(result.stderr).toContain("load=11.00");
    expect(result.stderr).toContain("logical_cpus=16");
    expect(result.stderr).toContain("compiler_workers=2");
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(workspace.root, "target-agent", ".locks", "pool-agent-debug.lock")))
      .toBe(false);
  });

  test("accepts an exact projected CPU budget and records the actual machine observation", () => {
    const workspace = fixture();
    installFakeSystemLoad(workspace, "10.00", "16");
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT: "75",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain("jobs=2\n");
    expect(wrapperRecord(result).result.admission).toMatchObject({ systemLoad1m: 10, logicalCpus: 16, loadLimitPercent: 75, reservedWorkers: 2 });
  });

  test("machine pressure accounts for the reviewed effective one-worker Cargo override", () => {
    const workspace = fixture();
    installFakeSystemLoad(workspace, "11.00", "16");
    const result = run("agent-cargo.sh", ["check", "--lib", "--jobs", "1"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT: "75",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain("jobs=1\n");
  });

  test("machine pressure reserves the larger Rust test-harness worker count", () => {
    const workspace = fixture();
    installFakeSystemLoad(workspace, "11.00", "16");
    const result = run("agent-cargo.sh", ["test", "--lib", "reviewed_filter"], {
      ...workspace.env,
      CARGO_BUILD_JOBS: "1",
      RUST_TEST_THREADS: "2",
      SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT: "75",
    });

    expect(result.status).toBe(75);
    expect(result.stderr).toContain("compiler_workers=1");
    expect(result.stderr).toContain("test_workers=2");
    expect(result.stderr).toContain("workload_workers=2");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("metadata-only Cargo remains available during explicit high machine pressure", () => {
    const workspace = fixture();
    installFakeSystemLoad(workspace, "90.00", "16");
    const result = run("agent-cargo.sh", ["metadata", "--no-deps"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT: "75",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain("args=metadata --no-deps ");
  });

  test.each(["0", "101", "1.5", "auto"])(
    "rejects malformed machine CPU budget %s before invoking Cargo",
    (budget) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["check", "--lib"], {
        ...workspace.env,
        SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT: budget,
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain(
        "SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT must be a whole percentage from 1 to 100",
      );
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test.each([
    ["unavailable", "16", "one-minute system load"],
    ["10.00", "unknown", "logical CPU count"],
  ])("fails closed when machine pressure observation is invalid: %s / %s", (load, cpus, detail) => {
    const workspace = fixture();
    installFakeSystemLoad(workspace, load!, cpus!);
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT: "75",
    });

    expect(result.status).toBe(64);
    expect(result.stderr).toContain(detail!);
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["--jobs", "8"],
    ["--jobs=8"],
    ["-j", "8"],
    ["-j8"],
  ].map(args => [args] as const))("refuses explicit compiler-worker bypass %j before Cargo runs", (workerArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["test", "--lib", ...workerArgs], workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("exceeds the 2-worker safety ceiling");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["--target-dir", "target"],
    ["--target-dir=target"],
    ["--target-dir", "/tmp/foreign-agent-target"],
  ].map(args => [args] as const))("refuses protected-pool target-directory escape %j before Cargo runs", (targetArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib", ...targetArgs], workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("target directory is owned by the protected Cargo pool");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["--lockfile-path", "/tmp/foreign/Cargo.lock"],
    ["--lockfile-path=/tmp/foreign/Cargo.lock"],
    ["--build-dir", "/tmp/foreign/build"],
    ["--build-dir=/tmp/foreign/build"],
  ].map(args => [args] as const))("refuses Cargo storage relocation %j before Cargo runs", (storageArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib", ...storageArgs], workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("Cargo storage ownership cannot be overridden");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each(["CARGO_BUILD_BUILD_DIR", "CARGO_RESOLVER_LOCKFILE_PATH"])(
    "refuses inherited Cargo storage ownership override %s before Cargo runs",
    (setting) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["check", "--lib"], {
        ...workspace.env,
        [setting]: "/tmp/foreign-cargo-storage",
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain(`Cargo storage ownership cannot be overridden by ${setting}`);
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test.each([".", ".."])(
    "refuses traversal-like protected pool identifier %s before Cargo runs",
    (poolName) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["check", "--lib"], {
        ...workspace.env,
        SCRIPT_KIT_CARGO_TARGET_POOL: poolName,
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain("must name one owned cache child");
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test.each([".", ".."])(
    "refuses traversal-like exclusive agent identifier %s before Cargo runs",
    (agentName) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["check", "--lib"], {
        ...workspace.env,
        SCRIPT_KIT_AGENT_TARGET_MODE: "exclusive",
        SCRIPT_KIT_AGENT_ID: agentName,
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain("single agent-debug pool");
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );


  test("refuses a symlinked build pool before touching its external destination", () => {
    const workspace = fixture();
    const external = temporaryDirectory("script-kit-external-pool-");
    rmSync(join(workspace.root, "target-agent/pools/agent-debug"), { recursive: true });
    symlinkSync(external, join(workspace.root, "target-agent/pools/agent-debug"));
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
    });

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("protected cache ownership cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(external, ".last_used"))).toBe(false);
  });

  test("refuses a symlinked artifact authority before Cargo writes", () => {
    const workspace = fixture(), external = temporaryDirectory("script-kit-external-artifact-");
    rmSync(join(workspace.root, "target-agent/artifacts"), { recursive: true });
    symlinkSync(external, join(workspace.root, "target-agent/artifacts"));
    const result = run("agent-cargo.sh", ["build", "--bin", "export_design_tokens"], workspace.env);
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("protected cache ownership cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["--config", "build.jobs=48"],
    ["--config=build.jobs=48"],
    ["--config", 'build.target-dir="target"'],
    ["--config", "/tmp/foreign-cargo-config.toml"],
  ].map(args => [args] as const))("refuses command-line Cargo config policy bypass %j before Cargo runs", (configArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib", ...configArgs], workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("command-line Cargo config cannot override protected build policy");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["test", "--lib", "--manifest-path", "/tmp/unreviewed/Cargo.toml"],
    ["test", "--lib", "--manifest-path=/tmp/unreviewed/Cargo.toml"],
    ["check", "--manifest-path", "/tmp/unreviewed/Cargo.toml"],
    ["run", "--bin", "export_design_tokens", "--manifest-path", "/tmp/unreviewed/Cargo.toml"],
    ["test", "--lib", "-m", "/tmp/unreviewed/Cargo.toml"],
    ["check", "--lib", "-m/tmp/unreviewed/Cargo.toml"],
    ["run", "--bin", "export_design_tokens", "-m=/tmp/unreviewed/Cargo.toml"],
    ["test", "--lib", "-C", "/tmp/unreviewed"],
  ].map(args => [args] as const))("refuses foreign Cargo workspace ownership %j before Cargo runs", (foreignArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", foreignArgs, workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("Cargo workspace ownership cannot be overridden");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["test", "--lib", "-p", "script-kit-ghost-llm-helper"],
    ["test", "--lib", "--package=foreign-plugin"],
    ["test", "--lib", "-p", "sk-storage", "-p", "foreign-plugin"],
    ["test", "--lib", "--workspace"],
    ["test", "--lib", "--all"],
    ["run", "--bin", "export_design_tokens", "-p", "foreign-plugin"],
  ].map(args => [args] as const))("refuses unreviewed Cargo package expansion %j before Cargo runs", (packageArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", packageArgs, workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo refuses unreviewed workspace or package");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  for (const [packageName, filter] of [
    ["gpui_macos", "readback_alpha_tests"],
    ["gpui-component", "input::state::revision_tests"],
    ["gpui-component", "notification::tests::closing_window_releases_notification_before_autohide_tick"],
  ] as const) {
    const reviewedArgs = ["test", "--locked", "-p", packageName, "--lib", filter];
    test(`admits exact reviewed vendor contract ${packageName}:${filter} without widening safety`, () => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", reviewedArgs, workspace.env);
      expect(result.status).toBe(0);
      const invocation = readFileSync(workspace.capture, "utf8");
      expect(invocation).toContain(`args=${reviewedArgs.join(" ")} `);
      expect(invocation).toContain("noninteractive=1");
      expect(invocation).toContain("takeover=0 input=0 capture=0 visible=0 live_ai=0 app=0");
      expect(wrapperRecord(result)).toMatchObject({ state: "closed", cleanup: { closed: true } });
    });

    test.each([
      reviewedArgs.slice(0, -1),
      [...reviewedArgs.slice(0, -1), "other_tests"],
      [...reviewedArgs.slice(0, -1), "notification::tests"],
      [...reviewedArgs.slice(0, -1), packageName === "gpui_macos" ? "input::state::revision_tests" : "readback_alpha_tests"],
      ["test", "--locked", "-p", packageName, filter],
      ["test", "--locked", "--package", packageName, "--lib", filter],
      ["test", "--locked", "-p", "gpui", "--lib", filter],
      ["run", ...reviewedArgs.slice(1)],
      ["nextest", ...reviewedArgs.slice(1)],
      [...reviewedArgs, "--", "other_filter"],
      [...reviewedArgs, "--", "--ignored"],
      [...reviewedArgs, "--", "--include-ignored"],
      [...reviewedArgs, "--features", "system-tests"],
      [...reviewedArgs, "--all-features"],
      [...reviewedArgs, "--workspace"],
      [...reviewedArgs, "-p", "sk-protocol"],
      [...reviewedArgs, "--tests"],
      [...reviewedArgs, "--all-targets"],
      [...reviewedArgs, "--doc"],
      [...reviewedArgs, "--jobs", "8"],
      [...reviewedArgs, "--", "--test-threads=8"],
    ].map(args => [args] as const))(`refuses vendor contract expansion for ${packageName}:${filter}: %j`, (args) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", args, workspace.env);
      expect(result.status).toBe(64);
      expect(result.stderr).toContain("noninteractive agent Cargo refuses unreviewed workspace or package");
      expect(existsSync(workspace.capture)).toBe(false);
    });

    test.each(["CARGO_BUILD_JOBS", "RUST_TEST_THREADS"])(`reviewed vendor contract ${packageName}:${filter} preserves inherited %s ceiling`, (variable) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", reviewedArgs, { ...workspace.env, [variable]: "8" });
      expect(result.status).toBe(64);
      expect(result.stderr).toContain("exceeds the 2-worker safety ceiling");
      expect(existsSync(workspace.capture)).toBe(false);
    });
  }

  test.each([
    ["test", "--lib", "--", "--ignored"],
    ["test", "--lib", "--", "--include-ignored"],
    ["test", "--lib", "--features", "system-tests"],
    ["test", "--lib", "--features=ocr,system-tests"],
    ["test", "--lib", "-F", "script-kit-gpui/system-tests"],
    ["test", "--lib", "-Fsystem-tests"],
    ["test", "--lib", "--all-features"],
    ["test", "--lib", "--tests"],
    ["test", "--lib", "--all-targets"],
    ["nextest", "run", "--lib", "--run-ignored", "all"],
    ["nextest", "run", "--lib", "--run-ignored=all"],
  ].map(args => [args] as const))("refuses unsafe system/ignored test activation %j before Cargo runs", (unsafeArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", unsafeArgs, workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo refuses unsafe test selection");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["test"],
    ["test", "--locked"],
    ["test", "privacy"],
    ["test", "-p", "script-kit-gpui"],
    ["nextest", "run"],
  ].map(args => [args] as const))("refuses unreviewed blanket Rust test discovery %j before Cargo runs", (unscopedArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", unscopedArgs, workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("requires an explicit reviewed --lib, --test, or safe domain package");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["test", "--doc"],
    ["test", "--doc", "--lib"],
    ["test", "--doc", "-p", "sk-protocol"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "other::alpha::"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "--", "other"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "--", "--ignored"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "--lib"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "--workspace"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "-p", "sk-storage"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "--all-targets"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "--features", "system-tests"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::", "--no-run"],
    ["nextest", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::"],
  ].map(args => [args] as const))("refuses unreviewed doctest expansion %j before Cargo runs", (args) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", args, workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["run"],
    ["run", "--bin", "script-kit-gpui"],
    ["run", "--bin=liquid-glass-demo"],
    ["bench"],
    ["doc", "--open"],
  ].map(args => [args] as const))("refuses application launch or live benchmark %j before Cargo runs", (launchArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", launchArgs, workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo refuses application launch or live benchmarks");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["t", "--lib", "--", "--ignored"],
    ["r", "--bin", "script-kit-gpui"],
    ["test-serial", "--ignored"],
    ["unreviewed-cargo-plugin", "--", "--ignored"],
  ].map(args => [args] as const))("refuses unreviewed Cargo alias or external subcommand %j before startup", (aliasArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", aliasArgs, workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo refuses unreviewed subcommand or alias");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["test", "--locked", "--lib"],
    ["test", "--locked", "--test", "protocol_batch"],
    ["test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::"],
    ["test", "-p", "sk-protocol"],
    ["test", "--package=sk-storage"],
    ["nextest", "run", "--lib"],
    ["run", "--bin", "export_design_tokens"],
  ].map(args => [args] as const))("preserves explicitly reviewed app, domain, and exporter command %j", (reviewedArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", reviewedArgs, workspace.env);
  
    expect(result.status).toBe(0);
    expect(existsSync(workspace.capture)).toBe(true);
  });

  test("refuses inherited compiler and Rust-test worker bypasses before Cargo runs", () => {
    for (const [variable, value] of [
      ["CARGO_BUILD_JOBS", "48"],
      ["RUST_TEST_THREADS", "48"],
    ]) {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["test", "--lib"], {
        ...workspace.env,
        [variable!]: value,
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain("exceeds the 2-worker safety ceiling");
      expect(existsSync(workspace.capture)).toBe(false);
    }
  });

  test.each(["0", "-1", "1.5", "workers"])(
    "refuses malformed inherited worker count %s before Cargo runs",
    (value) => {
      for (const variable of ["CARGO_BUILD_JOBS", "RUST_TEST_THREADS", "SCRIPT_KIT_AGENT_MAX_JOBS"]) {
        const workspace = fixture();
        const result = run("agent-cargo.sh", ["test", "--lib"], {
          ...workspace.env,
          [variable]: value,
        });

        expect(result.status).toBe(64);
        expect(result.stderr).toContain("must be a positive whole worker count");
        expect(existsSync(workspace.capture)).toBe(false);
      }
    },
  );

  test.each([
    ["--test-threads", "24"],
    ["--test-threads=24"],
  ].map(args => [args] as const))("refuses Rust-harness thread bypass %j before Cargo runs", (threadArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["test", "--lib", "--", ...threadArgs], workspace.env);
  
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("exceeds the 2-worker safety ceiling");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("noninteractive builds cannot raise their worker ceiling or inherit heavyweight fuzz", () => {
    const blocked = fixture();
    const raised = run("agent-cargo.sh", ["test", "--lib"], {
      ...blocked.env,
      SCRIPT_KIT_NONINTERACTIVE: "1",
      SCRIPT_KIT_AGENT_MAX_JOBS: "8",
    });

    expect(raised.status).toBe(64);
    expect(raised.stderr).toContain("noninteractive builds cannot exceed two workers");
    expect(existsSync(blocked.capture)).toBe(false);

    const isolated = fixture();
    const bounded = run("agent-cargo.sh", ["test", "--lib"], {
      ...isolated.env,
      SCRIPT_KIT_NONINTERACTIVE: "1",
      SCRIPT_KIT_SEARCH_FULL_STRESS: "1",
      SCRIPT_KIT_STORAGE_FULL_STRESS: "1",
    });

    expect(bounded.status).toBe(0);
    expect(readFileSync(isolated.capture, "utf8")).toContain("search_stress=0\n");
    expect(readFileSync(isolated.capture, "utf8")).toContain("storage_stress=0\n");
  });

  test("direct agent Cargo defaults to noninteractive heavyweight-stress isolation", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["test", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_NONINTERACTIVE: "",
      SCRIPT_KIT_SEARCH_FULL_STRESS: "1",
      SCRIPT_KIT_STORAGE_FULL_STRESS: "1",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain("noninteractive=1\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("search_stress=0\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("storage_stress=0\n");
  });

  test("noninteractive agent Cargo rejects dangerous inherited desktop permissions before startup", () => {
    for (const permission of [
      "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
      "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
      "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
      "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
      "SCRIPT_KIT_ALLOW_LIVE_AI",
      "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
    ]) {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["test", "--lib"], {
        ...workspace.env,
        SCRIPT_KIT_NONINTERACTIVE: "1",
        [permission]: "1",
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain(permission);
      expect(existsSync(workspace.capture)).toBe(false);
    }
  });

  test.each(["yes", "2", "interactive"])(
    "refuses malformed noninteractive safety policy %s before Cargo runs",
    (policy) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["test", "--lib"], {
        ...workspace.env,
        SCRIPT_KIT_NONINTERACTIVE: policy,
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain("SCRIPT_KIT_NONINTERACTIVE must be 0 or 1");
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test("explicit lower worker limits remain valid and bound the Rust test harness", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["test", "--lib", "--jobs", "1"], {
      ...workspace.env,
      CARGO_BUILD_JOBS: "1",
      RUST_TEST_THREADS: "1",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain("jobs=1\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("test_threads=1\n");
  });

  test("explicit interactive ceilings preserve deliberate larger compiler overrides", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--jobs=4"], {
      ...workspace.env,
      SCRIPT_KIT_NONINTERACTIVE: "0",
      SCRIPT_KIT_AGENT_MAX_JOBS: "4",
      SCRIPT_KIT_ALLOW_NATIVE_INPUT: "1",
      SCRIPT_KIT_SEARCH_FULL_STRESS: "1",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain("noninteractive=0\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("jobs=4\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("test_threads=4\n");
    expect(readFileSync(workspace.capture, "utf8")).toContain("search_stress=1\n");
  });








  test("fails before launching Cargo when the available disk floor is impossible", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_MIN_FREE_GB: "99999",
    });

    expect(result.status).toBe(75);
    expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "resource_free_space_below_floor", disposition: "BLOCKED_RESOURCE_BUDGET", cleanup: { closed: true }, resources: { scope: "target-agent", refusal: { phase: "preflight" } } });
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(workspace.root, "target-agent", "pools", "agent-debug"))).toBe(true);
  });

  test("fails explicitly when compiler caching was required but no sccache exists", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_USE_SCCACHE: "1",
      RUSTC_WRAPPER: "",
    });

    expect(result.status).toBe(69);
    expect(result.stderr).toContain("sccache is unavailable");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("preserves an explicitly configured compiler wrapper", () => {
    const workspace = fixture();
    const external = join(workspace.bin, "controlled-wrapper");
    semanticWrapper(external, "controlled");
    const compilerCapture = installCompilerCapture(workspace);
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      RUSTC_WRAPPER: external, POLICY_EXERCISE_COMPILER: "1",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain(
      `wrapper=${external}\n`,
    );
    expect(wrapperRecord(result).effectiveConfiguration.compilerCache).toBe("external");
    expect(wrapperRecord(result).effectiveConfiguration.requestedPolicy.cachePolicy).toBe("0");
    expect(readFileSync(compilerCapture, "utf8")).toContain("--cfg\ncontrolled\n--fixture-compile\n");
    expect(wrapperRecord(result).effectiveConfiguration.compatibility.compilerWrappers.rustc).toEqual({ path: external, sha256: artifactHash(readFileSync(external)) });
  });

  test("semantic wrapper identity and content invalidate a published artifact", () => {
    const workspace = fixture(), first = join(workspace.bin, "wrapper-a"), second = join(workspace.bin, "wrapper-b");
    semanticWrapper(first, "wrapper_a"); semanticWrapper(second, "wrapper_b");
    const compilerCapture = installCompilerCapture(workspace);
    const env = { ...workspace.env, RUSTC_WRAPPER: first, POLICY_EXERCISE_COMPILER: "1", SCRIPT_KIT_AGENT_ARTIFACT_KIND: "application" };
    const built = run("agent-cargo.sh", ["build", "--bin", "script-kit-gpui"], env);
    expect(built.status).toBe(0);
    const reference = JSON.parse(built.stdout).artifacts[0];
    expect(readFileSync(compilerCapture, "utf8")).toContain("--cfg\nwrapper_a\n");
    expect(verifyWrapperArtifact(workspace, reference, env)).toEqual({ verified: true });
    expect(verifyWrapperArtifact(workspace, reference, { ...env, RUSTC_WRAPPER: second })).toEqual({ code: "configuration_stale", disposition: "BLOCKED_STALE_GENERATION" });
    const rebuilt = run("agent-cargo.sh", ["build", "--bin", "script-kit-gpui"], { ...env, RUSTC_WRAPPER: second });
    expect(rebuilt.status).toBe(0);
    expect(readFileSync(compilerCapture, "utf8")).toContain("--cfg\nwrapper_b\n");
    semanticWrapper(first, "wrapper_replaced");
    expect(verifyWrapperArtifact(workspace, reference, env)).toEqual({ code: "configuration_stale", disposition: "BLOCKED_STALE_GENERATION" });
  });

  test("unavailable external wrappers fail closed before Cargo", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, RUSTC_WRAPPER: join(workspace.bin, "missing-wrapper") });
    expect(result.status).not.toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "configuration_stale", disposition: "BLOCKED_STALE_GENERATION" });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("Cargo-configured wrapper bytes remain semantic compiler inputs", () => {
    const workspace = fixture(), external = join(workspace.bin, "configured-wrapper");
    semanticWrapper(external, "configured_a");
    const compilerCapture = installCompilerCapture(workspace);
    mkdirSync(join(workspace.root, ".cargo"));
    writeFileSync(join(workspace.root, ".cargo/config.toml"), `[build]\nrustc-wrapper = ${JSON.stringify(external)}\n`);
    const env = { ...workspace.env, POLICY_EXERCISE_COMPILER: "1", SCRIPT_KIT_AGENT_ARTIFACT_KIND: "application" };
    const built = run("agent-cargo.sh", ["build", "--bin", "script-kit-gpui"], env);
    expect(built.status).toBe(0);
    expect(readFileSync(compilerCapture, "utf8")).toContain("--cfg\nconfigured_a\n");
    const reference = JSON.parse(built.stdout).artifacts[0];
    expect(verifyWrapperArtifact(workspace, reference, env)).toEqual({ verified: true });
    semanticWrapper(external, "configured_b");
    expect(verifyWrapperArtifact(workspace, reference, env)).toEqual({ code: "configuration_stale", disposition: "BLOCKED_STALE_GENERATION" });
  });

  test("undeclared Cargo environment wrapper overrides fail closed", () => {
    const workspace = fixture(), external = join(workspace.bin, "hidden-wrapper");
    semanticWrapper(external, "hidden_cfg");
    mkdirSync(join(workspace.root, ".cargo"));
    writeFileSync(join(workspace.root, ".cargo/config.toml"), `[env]\nRUSTC_WRAPPER = ${JSON.stringify(external)}\n`);
    const result = run("agent-cargo.sh", ["check", "--lib"], workspace.env);
    expect(result.status).not.toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "configuration_stale", disposition: "BLOCKED_STALE_GENERATION" });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("transparent cache availability changes neither compatibility nor current verification", () => {
    const workspace = fixture();
    installFakeSccache(workspace);
    const compilerCapture = installCompilerCapture(workspace);
    const env = { ...workspace.env, POLICY_EXERCISE_COMPILER: "1", SCRIPT_KIT_AGENT_USE_SCCACHE: "auto", SCRIPT_KIT_AGENT_ARTIFACT_KIND: "application" };
    const cached = run("agent-cargo.sh", ["build", "--bin", "script-kit-gpui"], env);
    expect(cached.status).toBe(0);
    expect(wrapperRecord(cached).effectiveConfiguration.compilerCache).toBe("sccache");
    expect(readFileSync(compilerCapture, "utf8")).toContain("--fixture-compile\n");
    const reference = JSON.parse(cached.stdout).artifacts[0];
    installFakeSccache(workspace, false);
    const uncached = run("agent-cargo.sh", ["build", "--bin", "script-kit-gpui"], env);
    expect(uncached.status).toBe(0);
    expect(wrapperRecord(uncached).effectiveConfiguration.compilerCache).toBe("unavailable");
    expect(wrapperRecord(uncached).effectiveConfiguration.compatibility).toEqual(wrapperRecord(cached).effectiveConfiguration.compatibility);
    expect(verifyWrapperArtifact(workspace, reference, env)).toEqual({ verified: true });
    // An explicit wrapper remains semantic even if it calls itself sccache.
    expect(verifyWrapperArtifact(workspace, reference, { ...env, RUSTC_WRAPPER: join(workspace.bin, "sccache") })).toEqual({ code: "configuration_stale", disposition: "BLOCKED_STALE_GENERATION" });
  });

  test("enables a healthy compiler cache through a workspace-owned Unix socket", () => {
    const workspace = fixture();
    installFakeSccache(workspace);
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_USE_SCCACHE: "1",
    });

    expect(result.status).toBe(0);
    const invocation = readFileSync(workspace.capture, "utf8");
    expect(invocation).toContain(`wrapper=${join(workspace.bin, "sccache")}\n`);
    expect(invocation).toContain(
      `socket=${join(workspace.root, "target-agent", "shared", "sccache.sock")}\n`,
    );
    expect(wrapperRecord(result).effectiveConfiguration.compilerCache).toBe("sccache");
    expect(wrapperRecord(result).effectiveConfiguration.requestedPolicy.cachePolicy).toBe("1");
  });

  test("explicitly falls back when sandboxing allows cache statistics but blocks rustc", () => {
    const workspace = fixture();
    installFakeSccache(workspace, false);
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_USE_SCCACHE: "auto",
    });

    expect(result.status).toBe(0);
    expect(result.stderr).toContain("sccache cannot execute rustc in this sandbox");
    expect(result.stderr).toContain("SCRIPT_KIT_AGENT_USE_SCCACHE=1");
    expect(result.stderr).toContain("sandbox permissions");
    expect(readFileSync(workspace.capture, "utf8")).toContain("wrapper=\n");
    expect(wrapperRecord(result).effectiveConfiguration.compilerCache).toBe("unavailable");
    expect(wrapperRecord(result).effectiveConfiguration.requestedPolicy.cachePolicy).toBe("auto");
  });

  test.each(["true", "2", "enabled"])(
    "rejects malformed compiler-cache policy %s before invoking Cargo",
    (policy) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["check", "--lib"], {
        ...workspace.env,
        SCRIPT_KIT_AGENT_USE_SCCACHE: policy,
      });
      expect(result.status).toBe(64);
      expect(result.stderr).toContain("SCRIPT_KIT_AGENT_USE_SCCACHE must be 0, 1, or auto");
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test("fails closed when a required compiler cache cannot execute rustc", () => {
    const workspace = fixture();
    installFakeSccache(workspace, false);
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_USE_SCCACHE: "1",
    });

    expect(result.status).toBe(69);
    expect(result.stderr).toContain("required sccache cannot execute rustc");
    expect(existsSync(workspace.capture)).toBe(false);
  });
  test.each([["build", "--profile=dev"], ["test", "--lib", "--no-run", "--profile", "test"]].map(args => [args] as const))("normalizes inherited incremental settings for %j", (args) => {
    const workspace = fixture();
    mkdirSync(join(workspace.root, ".cargo"));
    writeFileSync(join(workspace.root, ".cargo/config.toml"), '[env]\nCARGO_INCREMENTAL="1"\nCARGO_PROFILE_DEV_INCREMENTAL="true"\nCARGO_PROFILE_TEST_INCREMENTAL="true"\n');
    const result = run("agent-cargo.sh", args, { ...workspace.env, CARGO_INCREMENTAL: "1", CARGO_BUILD_INCREMENTAL: "true", CARGO_PROFILE_DEV_INCREMENTAL: "true", CARGO_PROFILE_TEST_INCREMENTAL: "true" });
    expect(result.status).toBe(0);
    const call = JSON.parse(readFileSync(join(workspace.root, "cargo-invocations.jsonl"), "utf8").trim());
    expect(call.environment).toMatchObject({ CARGO_INCREMENTAL: "0", CARGO_BUILD_INCREMENTAL: "false", CARGO_PROFILE_DEV_INCREMENTAL: "false", CARGO_PROFILE_TEST_INCREMENTAL: "false" });
    expect(wrapperRecord(result).effectiveConfiguration.incremental).toMatchObject({ enabled: false, owner: "agent-cargo" });
  });

  test.each(["CARGO_INCREMENTAL", "CARGO_BUILD_INCREMENTAL", "CARGO_PROFILE_DEV_INCREMENTAL", "CARGO_PROFILE_TEST_INCREMENTAL"])("refuses forced Cargo env conflict %s", name => {
    const workspace = fixture();
    writeFileSync(join(workspace.env.CARGO_HOME!, "config.toml"), `[env]\n${name}={value="1",force=true}\n`);
    const result = run("agent-cargo.sh", ["check", "--lib"], workspace.env);
    expect(result.status).not.toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "configuration_stale", cleanup: { closed: true } });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("honors legacy config selection and nearer Cargo env precedence", () => {
    const parent = temporaryDirectory("script-kit-cargo-ancestor-"), root = join(parent, "workspace");
    mkdirSync(root); const workspace = createBuildWorkspace(root);
    mkdirSync(join(parent, ".cargo")); mkdirSync(join(root, ".cargo"));
    writeFileSync(join(parent, ".cargo/config"), '[env]\nCARGO_INCREMENTAL={value="1",force=true}\n');
    writeFileSync(join(root, ".cargo/config"), '[env]\nCARGO_INCREMENTAL={value="0",force=true}\n');
    writeFileSync(join(root, ".cargo/config.toml"), '[env]\nCARGO_INCREMENTAL={value="1",force=true}\n');
    expect(run("agent-cargo.sh", ["check", "--lib"], workspace.env).status).toBe(0);
    rmSync(join(root, ".cargo/config"));
    expect(run("agent-cargo.sh", ["check", "--lib"], workspace.env).status).not.toBe(0);
  });

  test.each([
    { RUSTFLAGS: "-C incremental=/tmp/unowned" },
    { CARGO_ENCODED_RUSTFLAGS: "-C\u001fincremental=/tmp/unowned", RUSTFLAGS: "--cfg safe" },
    { RUSTDOCFLAGS: "--codegen=incremental=/tmp/unowned" },
  ])("rejects effective raw incremental flags %j", flags => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, ...flags });
    expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "configuration_stale" });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("encoded flags supersede otherwise conflicting ordinary and Cargo flags", () => {
    const workspace = fixture(); mkdirSync(join(workspace.root, ".cargo"));
    writeFileSync(join(workspace.root, ".cargo/config.toml"), '[build]\nrustflags=["-C","incremental=/tmp/unowned"]\n');
    const result = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, RUSTFLAGS: "-C incremental=/tmp/unowned", CARGO_ENCODED_RUSTFLAGS: "--cfg\u001fsafe" });
    expect(result.status).toBe(0);
    const call = JSON.parse(readFileSync(join(workspace.root, "cargo-invocations.jsonl"), "utf8").trim());
    expect(call.environment.CARGO_ENCODED_RUSTFLAGS).toBe("--cfg\u001fsafe");
  });

  test.each(["rustc", "rustdoc"])("rejects trailing %s incremental flags", subcommand => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", [subcommand, "--lib", "--", "-Cincremental=/tmp/unowned"], workspace.env);
    expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "configuration_stale" });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each(['[build]\ntarget-dir="outside"', '[build]\nbuild-dir="outside"', '[env]\nCARGO_TARGET_DIR={value="outside",force=true}', '[build]\nrustflags=["-C","incremental=outside"]'])("refuses configured output escape %s", config => {
    const workspace = fixture(); mkdirSync(join(workspace.root, ".cargo"));
    writeFileSync(join(workspace.root, ".cargo/config.toml"), `${config}\n`);
    expect(run("agent-cargo.sh", ["check", "--lib"], workspace.env).status).not.toBe(0);
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(workspace.root, "outside"))).toBe(false);
  });

  test.each([["--profile=custom"], ["--profile", "custom"]].map(args => [args] as const))("refuses unsupported profile %j", (args) => {
    const workspace = fixture();
    expect(run("agent-cargo.sh", ["check", "--lib", ...args], workspace.env).status).not.toBe(0);
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("required sccache cannot hide behind configured semantic wrappers", () => {
    const workspace = fixture(), wrapper = join(workspace.bin, "semantic"); semanticWrapper(wrapper, "semantic"); installFakeSccache(workspace);
    mkdirSync(join(workspace.root, ".cargo")); writeFileSync(join(workspace.root, ".cargo/config.toml"), `[build]\nrustc-wrapper=${JSON.stringify(wrapper)}\n`);
    const result = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, SCRIPT_KIT_AGENT_USE_SCCACHE: "1" });
    expect(result.status).toBe(69); expect(result.stderr).toContain("conflicts with external semantic compiler wrapper");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("cache probe uses the pinned compiler rather than ambient rustc", () => {
    const workspace = fixture(), pin = join(workspace.bin, "pinned-rustc"), capture = join(workspace.root, "cache-probe.txt");
    copyFileSync(join(workspace.bin, "rustc"), pin);
    writeFileSync(join(workspace.bin, "rustup"), `#!/bin/sh\nif [ "$4" = rustc ]; then printf '%s\\n' '${pin}'; else printf '%s\\n' '${join(workspace.bin, "cargo")}'; fi\n`, { mode: 0o700 });
    writeFileSync(join(workspace.bin, "rustc"), "#!/bin/sh\nexit 99\n", { mode: 0o700 });
    writeFileSync(join(workspace.bin, "sccache"), `#!/bin/sh\n[ "$1" = --show-stats ] && exit 0\nprintf '%s\\n' "$1" > '${capture}'\nexec "$@"\n`, { mode: 0o700 });
    const result = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, SCRIPT_KIT_AGENT_USE_SCCACHE: "1" });
    expect(result.status).toBe(0); expect(readFileSync(capture, "utf8").trim()).toBe(pin);
    expect(wrapperRecord(result).effectiveConfiguration.compilerCacheProbe).toMatchObject({ probeStatus: "pinned-rustc-succeeded", measuredHits: null, measuredMisses: null });
  });

  test("passive policy reports ownership without starting cache probes or Cargo", () => {
    const workspace = fixture(), marker = join(workspace.root, "cache-probed");
    writeFileSync(join(workspace.bin, "sccache"), `#!/bin/sh\ntouch '${marker}'\nexit 0\n`, { mode: 0o700 });
    const result = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, SCRIPT_KIT_AGENT_POLICY_ONLY: "1", SCRIPT_KIT_AGENT_USE_SCCACHE: "1" });
    expect(result.status).toBe(0); expect(JSON.parse(result.stdout)).toMatchObject({ passive: true, incremental: { enabled: false, owner: "agent-cargo" }, cacheProbeStatus: "not-probed" });
    expect(existsSync(marker)).toBe(false); expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(workspace.root, "target-agent/.locks/pool-agent-debug.lock"))).toBe(false);
  });

  test("only effective target flags participate in incremental refusal", () => {
    const workspace = fixture(); mkdirSync(join(workspace.root, ".cargo"));
    writeFileSync(join(workspace.root, ".cargo/config.toml"), '[build]\nrustflags=["-C","incremental=ignored-build"]\n[target.\'cfg(target_os = "macos")\']\nrustflags=["--cfg","safe"]\n[target.\'cfg(target_os = "linux")\']\nrustflags=["-C","incremental=inactive-target"]\n');
    expect(run("agent-cargo.sh", ["check", "--lib"], workspace.env).status).toBe(0);
    writeFileSync(join(workspace.root, ".cargo/config.toml"), '[build]\nrustflags=["-C","incremental=effective-build"]\n[target.\'cfg(target_os = "linux")\']\nrustflags=["--cfg","inactive"]\n');
    const result = run("agent-cargo.sh", ["check", "--lib"], workspace.env);
    expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "configuration_stale" });
    expect(readFileSync(join(workspace.root, "cargo-invocations.jsonl"), "utf8").trim().split("\n")).toHaveLength(1);
  });

  test("interactive wrapper cannot bypass managed resource policy", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, SCRIPT_KIT_NONINTERACTIVE: "0", SCRIPT_KIT_AGENT_ALLOW_LOW_DISK: "1" });
    expect(result.status).toBe(75); expect(JSON.parse(result.stdout)).toMatchObject({ failureCode: "resource_policy_conflict", cleanup: { closed: true } });
    expect(existsSync(workspace.capture)).toBe(false);
  });

});

describe("structured wrapper finalization and resource enforcement", () => {
  function instrumented(workspace: PolicyWorkspace, injection: string, args: string[], extraEnv: Record<string, string> = {}, after = "") {
    const script = `import { spyOn } from "bun:test";
import * as fs from "node:fs";
import { join } from "node:path";
import * as lifecycle from ${JSON.stringify(join(scripts, "artifact-lifecycle.ts"))};
import * as inventory from ${JSON.stringify(join(scripts, "../devtools/lib/build-ops-inventory.ts"))};
import { runWrapperCargo } from ${JSON.stringify(join(scripts, "build-artifact.ts"))};
const root = process.env.SCRIPT_KIT_REPO_ROOT, lock = join(root,"target-agent/.locks/pool-agent-debug.lock");
process.env.SCRIPT_KIT_AGENT_LEASE_PATH=lock; process.env.SCRIPT_KIT_AGENT_LEASE_GENERATION="instrumented-wrapper";
process.env.CARGO_TARGET_DIR=join(root,"target-agent/pools/agent-debug"); process.env.CARGO_BUILD_JOBS="2"; process.env.RUST_TEST_THREADS="2";
${injection}
lifecycle.cacheLease("acquire",lock,[String(process.pid),"instrumented-wrapper","1000"]);
const status = await runWrapperCargo(${JSON.stringify(args)});
${after}
process.exitCode=status;`;
    const result = Bun.spawnSync([process.execPath, "-e", script], { cwd: workspace.root, env: { ...workspace.env, ...extraEnv }, timeout: 30_000, stdout: "pipe", stderr: "pipe" });
    const lines = result.stdout.toString().trim().split("\n");
    return { status: result.exitCode, outcome: JSON.parse(lines[0]!), evidence: lines[1] ? JSON.parse(lines[1]) : undefined };
  }

  for (const failure of ["result-update", "task-finalize", "log-flush", "log-close"] as const) test(`wrapper ${failure} fault attempts independent finalizers and returns actual identity`, () => {
    const workspace = fixture();
    const injection = `const failure=${JSON.stringify(failure)};
let logFd, closed=false, finalized=false;
const actualOpen=fs.openSync, actualClose=fs.closeSync, actualFlush=fs.fsyncSync, actualUpdate=lifecycle.updateManagedTask, actualFinalize=lifecycle.finalizeManagedTask;
spyOn(fs,"openSync").mockImplementation((path,flags,mode)=>{const fd=actualOpen(path,flags,mode);if(String(path).endsWith("/cargo.log"))logFd=fd;return fd;});
spyOn(fs,"fsyncSync").mockImplementation(fd=>{if(!closed&&fd===logFd&&failure==="log-flush")throw new Error("injected_flush");return actualFlush(fd);});
spyOn(fs,"closeSync").mockImplementation(fd=>{if(!closed&&fd===logFd){if(failure==="log-close")throw new Error("injected_close");closed=true;}return actualClose(fd);});
spyOn(lifecycle,"updateManagedTask").mockImplementation((task,patch)=>{if(failure==="result-update"&&patch.result)throw new Error("injected_result");return actualUpdate(task,patch);});
spyOn(lifecycle,"finalizeManagedTask").mockImplementation((task,cleanup)=>{finalized=true;if(failure==="task-finalize")throw new Error("injected_finalizer");return actualFinalize(task,cleanup);});`;
    const result = instrumented(workspace, injection, ["check", "--lib"], {}, 'console.log(JSON.stringify({closed,finalized}));if(logFd!==undefined&&!closed)actualClose(logFd);');
    expect(result.status).not.toBe(0);
    expect(result.evidence).toEqual({ closed: failure !== "log-close", finalized: true });
    expect(result.outcome.cleanup).toMatchObject({ closed: false, referencesFinalized: false, processExited: true, processGroupExited: true, logWriterClosed: failure !== "log-close" });
    const record = JSON.parse(readFileSync(result.outcome.recordPath, "utf8"));
    expect(result.outcome.task).toEqual(record.identity);
    expect(record.state).toBe(failure === "task-finalize" ? "finalizing" : "protected");
    expect(existsSync(join(workspace.root, "target-agent/.locks/pool-agent-debug.lock"))).toBe(false);
    expect(result.outcome.cleanup.failureCodes).toContain({ "result-update": "task_result_finalization_failed", "task-finalize": "task_finalization_failed", "log-flush": "log_flush_failed", "log-close": "log_close_failed" }[failure]);
    if (failure === "result-update" || failure === "task-finalize") {
      const unresolved = result.outcome.cleanup.survivors.find((item: { kind: string }) => item.kind === "task-record");
      expect(unresolved.observation).toBe("unknown"); expect(JSON.parse(unresolved.identity)).toEqual(record.identity);
    }
  });

  for (const failure of ["result-update", "task-finalize"] as const) test(`signed publication ${failure} does not skip its task finalizer`, () => {
    const workspace = fixture();
    writeFileSync(join(workspace.root, ".git/info/exclude"), "cargo-invocation.txt\n");
    // The policy fixture adds real helper symlinks after its initial commit; signing requires a clean source identity.
    for (const args of [["add", "scripts/agentic"], ["-c", "user.name=Fixture", "-c", "user.email=fixture@invalid", "-c", "commit.gpgsign=false", "commit", "-qm", "record fixture helpers"]]) {
      const committed = Bun.spawnSync(["git", "-c", "core.hooksPath=/dev/null", ...args], { cwd: workspace.root, env: workspace.env, stdout: "pipe", stderr: "pipe" });
      expect(committed.exitCode, committed.stderr.toString()).toBe(0);
    }
    const built = run("agent-cargo.sh", ["build", "--bin", "script-kit-gpui"], { ...workspace.env, SCRIPT_KIT_AGENT_ARTIFACT_KIND: "application" });
    expect(built.status).toBe(0);
    expect(JSON.parse(readFileSync(join(workspace.root, JSON.parse(built.stdout).artifacts[0].manifestPath), "utf8")).source.repositoryDirty).toBe(false);
    const input = join(workspace.root, ".test-output/input.reference.json"); writeFileSync(input, JSON.stringify(JSON.parse(built.stdout).artifacts[0]));
    const result = instrumented(workspace, `let finalized=false;
const actualUpdate=lifecycle.updateManagedTask, actualFinalize=lifecycle.finalizeManagedTask;
spyOn(lifecycle,"updateManagedTask").mockImplementation((task,patch)=>{if(${JSON.stringify(failure)}==="result-update"&&patch.result)throw new Error("injected_result");return actualUpdate(task,patch);});
spyOn(lifecycle,"finalizeManagedTask").mockImplementation((task,cleanup)=>{finalized=true;if(${JSON.stringify(failure)}==="task-finalize")throw new Error("injected_finalizer");return actualFinalize(task,cleanup);});`, ["publish-signed-bundle", "--input", input, "--bundle", workspace.root, "--attestation", input], {}, 'console.log(JSON.stringify({finalized}));');
    expect(result.evidence.finalized, JSON.stringify(result.outcome)).toBe(true);
    expect(result.outcome.cleanup).toMatchObject({ closed: false, referencesFinalized: false });
    expect(result.outcome.task).toEqual(JSON.parse(readFileSync(result.outcome.recordPath, "utf8")).identity);
    expect(result.outcome.artifacts).toEqual([]);
  });

  for (const phase of ["preflight", "post-cargo", "pre-publication", "post-publication"] as const) test(`resource refusal at ${phase} preserves compiler outputs without a usable export`, () => {
    const workspace = fixture();
    const injection = `const actualAdmission=inventory.requireBuildAdmission;
spyOn(inventory,"requireBuildAdmission").mockImplementation((root,options)=>{
const observation=actualAdmission(root,options);
if(options?.phase===${JSON.stringify(phase)})throw new inventory.BuildResourceError("resource_budget_exceeded",{...observation,withinLimits:false,targetAgentBudgetBytes:1,failureCodes:["resource_budget_exceeded"]});
return observation;});`;
    const result = instrumented(workspace, injection, ["build", "--bin", "script-kit-gpui"], { SCRIPT_KIT_AGENT_ARTIFACT_KIND: "application", FAKE_CARGO_ALLOCATE_BYTES: "65536" });
    expect(result.status).toBe(75);
    expect(result.outcome).toMatchObject({ status: "failed", artifacts: [], failureCode: "resource_budget_exceeded", resources: { refusal: { phase }, hardQuota: false }, cleanup: { closed: true } });
    expect(existsSync(workspace.capture)).toBe(phase !== "preflight");
    expect(existsSync(join(workspace.root, "target-agent/pools/agent-debug/fixture-allocation.bin"))).toBe(phase !== "preflight");
    if (phase === "post-publication") {
      const record = JSON.parse(readFileSync(result.outcome.recordPath, "utf8"));
      expect(record.publicationIntents).toHaveLength(1); expect(record.publicationIntents[0].phase).toBe("failed");
      expect(existsSync(join(workspace.root, record.publicationIntents[0].pendingPath))).toBe(true);
    }
  });

  test("post-Cargo resource accounting also runs after compiler failure", () => {
    const workspace = fixture();
    const result = instrumented(workspace, "", ["check", "--lib"], { FAKE_CARGO_FAIL: "1", FAKE_CARGO_ALLOCATE_BYTES: "65536" });
    expect(result.outcome.artifacts).toEqual([]);
    expect(result.outcome.resources.checks.map((item: { phase: string }) => item.phase)).toContain("post-cargo");
    expect(result.outcome.cleanup.closed).toBe(true);
    expect(existsSync(join(workspace.root, "target-agent/pools/agent-debug/fixture-allocation.bin"))).toBe(true);
  });

  test("in-build resource refusal closes only the owned compiler through its guard", () => {
    const workspace = fixture();
    const result = instrumented(workspace, `const observation=inventory.requireBuildAdmission(root,{phase:"sampled"});
const refusal={...observation,withinLimits:false,failureCodes:["resource_budget_exceeded"]};
spyOn(inventory,"startBuildResourceGuard").mockImplementation((_root,onRefusal)=>{
let triggered=false;
const observe=()=>{if(!triggered&&fs.existsSync(join(root,"cargo-ready"))){triggered=true;watcher.close();onRefusal(new inventory.BuildResourceError("resource_budget_exceeded",refusal));}};
const watcher=fs.watch(root,observe);observe();
return {async stop(){watcher.close();return {sampleCount:1,maximumSampledAllocatedBytes:65536,minimumSampledAvailableBytes:observation.availableBytes,complete:true,workerClosed:true,trigger:triggered?refusal:null};}};});`, ["build", "--bin", "script-kit-gpui"], { SCRIPT_KIT_AGENT_ARTIFACT_KIND: "application", FAKE_CARGO_ALLOCATE_BYTES: "65536", FAKE_CARGO_HANDSHAKE: "1" });
    expect(result.status).toBe(75);
    expect(result.outcome).toMatchObject({ artifacts: [], failureCode: "resource_budget_exceeded", cleanup: { processExited: true, processGroupExited: true, closed: true }, resources: { monitoring: { sampleCount: 1 } } });
    expect(existsSync(join(workspace.root, "target-agent/pools/agent-debug/fixture-allocation.bin"))).toBe(true);
  });
  test.each([false, true])("incomplete monitor stop cannot publish even when workerClosed=%s", workerClosed => {
    const workspace = fixture();
    const result = instrumented(workspace, `spyOn(inventory,"startBuildResourceGuard").mockImplementation(()=>({async stop(){return {sampleCount:0,maximumSampledAllocatedBytes:null,minimumSampledAvailableBytes:null,complete:false,workerClosed:${workerClosed},workerThreadId:42,trigger:null};}}));`, ["build", "--bin", "script-kit-gpui"], { SCRIPT_KIT_AGENT_ARTIFACT_KIND: "application" }, 'console.log(JSON.stringify({wrapperPid:process.pid}));');
    expect(result.outcome).toMatchObject({ artifacts: [], failureCode: "resource_observation_incomplete", resources: { refusal: { phase: "monitor-stop", complete: false } } });
    expect(result.outcome.cleanup.closed).toBe(workerClosed);
    expect(existsSync(join(workspace.root, "target-agent/.locks/pool-agent-debug.lock"))).toBe(false);
    if (!workerClosed) {
      const survivor = result.outcome.cleanup.survivors.find((item: { kind: string }) => item.kind === "resource-monitor");
      expect(survivor.observation).toBe("unknown");
      expect(JSON.parse(survivor.identity)).toEqual({ task: { id: result.outcome.task.id, generation: result.outcome.task.generation }, wrapperPid: result.evidence.wrapperPid, workerThreadId: 42 });
      expect(result.outcome.cleanup.referencesFinalized).toBe(false);
    }
  });

});

describe("Cargo cache ownership", () => {
  test("unproved compiler group cleanup retains the lease until exact absence recovery", async () => {
    const workspace = fixture(), lock = join(workspace.root, "target-agent/.locks/pool-agent-debug.lock");
    writeFileSync(join(workspace.bin, "ps"), `#!/bin/bash
if [[ "$1" == "-axo" && "\${POLICY_GROUP_UNKNOWN:-0}" == "1" ]]; then exit 71; fi
exec /bin/ps "$@"
`, { mode: 0o755 });
    const unrelated = Bun.spawn(["/bin/cat"], { stdin: "pipe", stdout: "ignore", stderr: "ignore" });
    try {
      const incomplete = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, POLICY_GROUP_UNKNOWN: "1" });
      expect(incomplete.status).not.toBe(0);
      const record = wrapperRecord(incomplete);
      expect(record.state).toBe("protected");
      expect(record.cleanup).toMatchObject({ resourcesAcquired: true, closed: false, processGroupExited: false, referencesFinalized: false });
      expect(record.cleanup.failureCodes).toContain("lease_retained_cleanup_unproved");
      expect(record.cleanup.survivors).toContainEqual(expect.objectContaining({ kind: "process-group", observation: "unknown" }));
      const retained = readFileSync(join(lock, "lease.json"), "utf8");
      expect(JSON.parse(retained).children).toEqual(record.ownedProcesses);
      const invocations = readFileSync(join(workspace.root, "cargo-invocations.jsonl"), "utf8");
      const blocked = run("agent-cargo.sh", ["check", "--lib"], { ...workspace.env, SCRIPT_KIT_AGENT_LOCK_TIMEOUT_SEC: "0" });
      expect(blocked.status).not.toBe(0);
      expect(blocked.stderr).toContain("lease_busy_or_incomplete");
      expect(readFileSync(join(workspace.root, "cargo-invocations.jsonl"), "utf8")).toBe(invocations);
      const unknown = run("cargo-cache-locks.sh", ["diagnose", lock], { ...workspace.env, POLICY_GROUP_UNKNOWN: "1" });
      expect(JSON.parse(unknown.stdout)).toMatchObject({ state: "protected", reasonCode: "group_observation_unknown" });
      const diagnosis = run("cargo-cache-locks.sh", ["diagnose", lock], workspace.env);
      expect(diagnosis.status).toBe(0);
      const exact = JSON.parse(diagnosis.stdout);
      expect(exact.state).toBe("recoverable");
      expect(exact.observations.every((observation: { observed: unknown }) => observation.observed === null)).toBe(true);
      const stale = run("cargo-cache-locks.sh", ["recover", lock, JSON.stringify({ ...exact, recordSha256: "0".repeat(64) })], workspace.env);
      expect(stale.status).not.toBe(0);
      expect(readFileSync(join(lock, "lease.json"), "utf8")).toBe(retained);
      expect(() => process.kill(unrelated.pid, 0)).not.toThrow();
      const recovered = run("cargo-cache-locks.sh", ["recover", lock, JSON.stringify(exact)], workspace.env);
      expect(recovered.status).toBe(0);
      expect(existsSync(lock)).toBe(false);
      expect(readFileSync(join(JSON.parse(recovered.stdout).evidence, "lease.json"), "utf8")).toBe(retained);
      expect(() => process.kill(unrelated.pid, 0)).not.toThrow();
      expect(run("agent-cargo.sh", ["check", "--lib"], workspace.env).status).toBe(0);
      expect(readFileSync(join(workspace.root, "cargo-invocations.jsonl"), "utf8")).not.toBe(invocations);
    } finally {
      unrelated.stdin.end();
      await unrelated.exited;
    }
  }, 30_000);

  test("release independently refuses live and unknown child ownership", async () => {
    const workspace = fixture(), lock = join(workspace.root, "target-agent/.locks/pool-agent-debug.lock");
    const owner = [String(process.pid), "release-child-fixture"];
    expect(run("cargo-cache-locks.sh", ["acquire", lock, ...owner, "1000"], workspace.env).status).toBe(0);
    expect(run("cargo-cache-locks.sh", ["reserve-child", lock, ...owner], workspace.env).status).toBe(0);
    const pending = run("cargo-cache-locks.sh", ["release", lock, ...owner], workspace.env);
    expect(pending.status).not.toBe(0);
    expect(pending.stderr).toContain("lease_child_identity_unknown");
    const child = await spawnOwnedProcess({ argv: ["/bin/cat"], cwd: workspace.root, env: workspace.env, timeoutMs: 20_000, maxOutputBytes: 4096 });
    try {
      expect(run("cargo-cache-locks.sh", ["bind", lock, ...owner, JSON.stringify(child.identity)], workspace.env).status).toBe(0);
      const live = run("cargo-cache-locks.sh", ["release", lock, ...owner], workspace.env);
      expect(live.status).not.toBe(0);
      expect(live.stderr).toContain("lease_children_present");
      expect(existsSync(lock)).toBe(true);
      expect(() => process.kill(child.pid, 0)).not.toThrow();
      child.stdin.end();
      await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
      expect((await child.close()).closed).toBe(true);
      writeFileSync(join(workspace.bin, "ps"), '#!/bin/bash\nif [[ "$1" == "-axo" ]]; then exit 71; fi\nexec /bin/ps "$@"\n', { mode: 0o755 });
      const unknown = run("cargo-cache-locks.sh", ["release", lock, ...owner], workspace.env);
      expect(unknown.status).not.toBe(0);
      expect(unknown.stderr).toContain("group_observation_unknown");
      expect(existsSync(lock)).toBe(true);
      writeFileSync(join(workspace.bin, "ps"), '#!/bin/bash\nexec /bin/ps "$@"\n', { mode: 0o755 });
      expect(run("cargo-cache-locks.sh", ["release", lock, ...owner], workspace.env).status).toBe(0);
      expect(existsSync(lock)).toBe(false);
    } finally { await child.close(); }
  }, 30_000);

  test("unknown spawned identity cannot be recovered after its wrapper exits", () => {
    const workspace = fixture(), lock = join(workspace.root, "target-agent/.locks/pool-agent-debug.lock");
    const script = `import { cacheLease } from ${JSON.stringify(join(scripts, "artifact-lifecycle.ts"))};
const lock = ${JSON.stringify(lock)}, owner = [String(process.pid), "pending-owner"];
cacheLease("acquire", lock, [...owner, "1000"]);
cacheLease("reserve-child", lock, owner);`;
    const owner = Bun.spawnSync([process.execPath, "-e", script], { cwd: workspace.root, env: workspace.env, stdout: "pipe", stderr: "pipe" });
    expect(owner.exitCode).toBe(0);
    const diagnosis = run("cargo-cache-locks.sh", ["diagnose", lock], workspace.env);
    expect(JSON.parse(diagnosis.stdout)).toMatchObject({ state: "protected", reasonCode: "lease_child_identity_unknown" });
    expect(run("cargo-cache-locks.sh", ["recover", lock, diagnosis.stdout], workspace.env).status).not.toBe(0);
    expect(existsSync(lock)).toBe(true);
  });

  test("removes only stale unlocked pools and preserves live, warm, shared, and artifact caches", () => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_NONINTERACTIVE: "0", SCRIPT_KIT_ALLOW_CARGO_CACHE_RECOVERY: "1" });
    const live = join(workspace.root, "target-agent", "pools", "active-owner");
    const stale = join(workspace.root, "target-agent", "pools", "stale-unused");
    const warm = join(workspace.root, "target-agent", "pools", "agent-debug");
    makeOld(live);
    makeOld(stale);
    makeOld(warm);
    liveLock(workspace.root, "active-owner");

    const result = run("prune-cargo-targets.sh", ["--apply"], workspace.env);

    expect(result.status).toBe(0);
    expect(existsSync(stale)).toBe(false);
    expect(existsSync(live)).toBe(true);
    expect(existsSync(warm)).toBe(true);
    expect(existsSync(join(workspace.root, "target-agent", "shared"))).toBe(true);
    expect(existsSync(join(workspace.root, "target-agent", "artifacts"))).toBe(true);
    expect(existsSync(join(workspace.root, "target-agent", ".locks"))).toBe(true);
    expect(result.stderr).toContain("preserve active pool");
  });

  test("treats a half-written lock lease as protected", () => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_NONINTERACTIVE: "0", SCRIPT_KIT_ALLOW_CARGO_CACHE_RECOVERY: "1" });
    const incomplete = join(workspace.root, "target-agent", "pools", "starting-owner");
    makeOld(incomplete);
    liveLock(workspace.root, "starting-owner", false);

    const result = run("prune-cargo-targets.sh", ["--apply"], workspace.env);

    expect(result.status).toBe(0);
    expect(existsSync(incomplete)).toBe(true);
  });

  test("emergency cleanup never terminates a live owner or deletes its pool", () => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_NONINTERACTIVE: "0", SCRIPT_KIT_ALLOW_CARGO_CACHE_RECOVERY: "1" });
    const live = join(workspace.root, "target-agent", "pools", "live-emergency");
    const stale = join(workspace.root, "target-agent", "pools", "stale-emergency");
    makeOld(live);
    makeOld(stale);
    liveLock(workspace.root, "live-emergency");

    const result = run(
      "disk-space-cargo-emergency-clean.sh",
      ["--apply", "--repo", workspace.root, "--threshold-gib", "9999", "--target-free-gib", "9999"],
      workspace.env,
    );

    expect(result.status).toBe(2);
    expect(existsSync(stale)).toBe(false);
    expect(existsSync(live)).toBe(true);
    expect(existsSync(join(workspace.root, "target-agent", "pools", "agent-debug"))).toBe(true);
    expect(result.stderr).toContain("active Cargo build leases remain protected");
    expect(() => process.kill(process.pid, 0)).not.toThrow();
  });

  test("already-installed legacy watchers clean directly without starting an AI agent", () => {
    const workspace = fixture();
    Object.assign(workspace.env, { SCRIPT_KIT_NONINTERACTIVE: "0", SCRIPT_KIT_ALLOW_CARGO_CACHE_RECOVERY: "1" });
    const live = join(workspace.root, "target-agent", "pools", "legacy-live-owner");
    makeOld(live);
    liveLock(workspace.root, "legacy-live-owner");

    const result = run(
      "disk-space-cargo-run-claude-cleanup.sh",
      [
        "--repo", workspace.root,
        "--state-dir", join(workspace.root, "watcher-state"),
        "--threshold-gib", "9999",
        "--target-free-gib", "9999",
      ],
      { ...workspace.env, CLAUDE_BIN: "/missing/never-start-this-agent" },
    );

    expect(result.status).toBe(2);
    expect(result.stderr).toContain("background AI delegation is disabled");
    expect(result.stderr).not.toContain("claude binary not executable");
    expect(existsSync(live)).toBe(true);
  });
});



describe("development watcher authorization", () => {
  test.each(["0", "1"])("dev.sh authorizes final modes before signaling or stamp cleanup (noninteractive=%s)", async (noninteractive) => {
    const workspace = fixture(), root = realpathSync(workspace.root);
    const launcher = join(root, "dev.sh"), stamp = join(root, "inherited-launcher.stamp");
    copyFileSync(join(scripts, "../../dev.sh"), launcher);
    writeFileSync(stamp, "protected inherited launcher stamp\n");
    const stampBefore = lstatSync(stamp);
    const env = { ...workspace.env, SCRIPT_KIT_NONINTERACTIVE: noninteractive!, SCRIPT_KIT_DEV_STAMP_FILE: stamp, SCRIPT_KIT_DEV_TAKEOVER: "0" };
    const child = await spawnOwnedProcess({ argv: ["/bin/cat"], cwd: root, env, timeoutMs: 30_000, maxOutputBytes: 4096 });
    const lock = join("/tmp/sk-dev-launcher-locks", `${createHash("sha1").update(root).digest("hex")}.lock`);
    let createdLock = false;
    try {
      const started = Bun.spawnSync(["/bin/ps", "-p", String(child.pid), "-o", "lstart="], { env: { ...env, LC_ALL: "C" }, stdout: "pipe", stderr: "pipe" });
      expect(started.exitCode, started.stderr.toString()).toBe(0);
      expect(started.stdout.toString().trim()).not.toBe("");
      // Only the disposable child and canonical fixture root authorize this registration.
      mkdirSync(dirname(lock), { recursive: true });
      mkdirSync(lock);
      createdLock = true;
      const lease = {
        pid: `${child.pid}\n`,
        "process-start": `${started.stdout.toString().trim()}\n`,
        generation: "disposable-watcher-authorization-fixture\n",
        root: `${root}\n`,
        session: "disposable-watcher\n",
      };
      for (const [name, value] of Object.entries(lease)) writeFileSync(join(lock, name), value);
      const invoke = async (args: string[], takeover = "0") => {
        const command = Bun.spawn(["/bin/bash", launcher, ...args], {
          cwd: root,
          env: { ...env, SCRIPT_KIT_DEV_TAKEOVER: takeover },
          stdin: "ignore", stdout: "pipe", stderr: "pipe",
        });
        const [status, stdout, stderr] = await Promise.all([command.exited, new Response(command.stdout).text(), new Response(command.stderr).text()]);
        expect(() => process.kill(child.pid, 0), args.join(" ")).not.toThrow();
        expect(readFileSync(stamp, "utf8")).toBe("protected inherited launcher stamp\n");
        expect(lstatSync(stamp).mtimeMs).toBe(stampBefore.mtimeMs);
        expect(lstatSync(stamp).ino).toBe(stampBefore.ino);
        expect(readdirSync(lock).sort()).toEqual(Object.keys(lease).sort());
        for (const [name, value] of Object.entries(lease)) expect(readFileSync(join(lock, name), "utf8")).toBe(value);
        return { status, stdout, stderr };
      };
      for (const takeover of ["0", "1"]) {
        const status = await invoke(["--status"], takeover);
        expect(status.status, status.stderr).toBe(0);
        expect(status.stdout).toContain(`RUNNING pid=${child.pid} session=disposable-watcher`);
      }
      for (const args of [
        ["--status", "--stop"], ["--stop", "--status"],
        ["--takeover", "--status"], ["--status", "--takeover"],
        ["--takeover", "--stop"], ["--stop", "--takeover"],
        ["--force", "--status"], ["--status", "-f"],
        ["--help", "--status", "--stop"], ["--takeover", "--status", "--help"],
      ]) {
        const conflict = await invoke(args);
        expect(conflict.status, conflict.stderr).toBe(64);
        expect(conflict.stderr).toContain("conflicting modes");
      }
      for (const args of [[], ["--stop"], ["--takeover"], ["--force"], ["-f"]]) {
        const refused = await invoke(args);
        expect(refused.status, refused.stderr).toBe(78);
        expect(refused.stderr).toContain("requires an interactive human terminal");
      }
      expect((await invoke([], "1")).status).toBe(78);
      for (const args of [
        ["--help"], ["-h"],
        ["--takeover", "--help"], ["--help", "--takeover"],
        ["--stop", "--help"], ["--help", "--stop"],
        ["--status", "--help"], ["--help", "--status"],
      ]) {
        const help = await invoke(args);
        expect(help.status, help.stderr).toBe(0);
        expect(help.stdout).toContain("Flags:");
      }
      for (const args of [["--unknown"], ["--help", "--unknown"], ["--unknown", "--help"]]) {
        const invalid = await invoke(args);
        expect(invalid.status, invalid.stderr).toBe(64);
        expect(invalid.stderr).toContain("unknown flag");
      }
    } finally {
      await child.close();
      if (createdLock) rmSync(lock, { recursive: true, force: true });
    }
  }, 40_000);
});

describe("isolated session readiness ownership", () => {
  function sessionFixture() {
    const workspace = fixture();
    const localScripts = join(workspace.root, "scripts", "agentic");
    for (const script of [
      "devtools-session-lib.sh",
      "devtools-session.sh",
      "start-isolated.sh",
    ]) {
      copyFileSync(join(scripts, script), join(localScripts, script));
    }
    for (const file of ["build-artifact.ts", "artifact-lifecycle.ts", "owned-process.ts", "session-supervisor.py"]) copyFileSync(join(scripts, file), join(localScripts, file));
    const devtoolsLib = join(workspace.root, "scripts/devtools/lib");
    mkdirSync(devtoolsLib, { recursive: true });
    copyFileSync(join(scripts, "../devtools/lib/build-ops-inventory.ts"), join(devtoolsLib, "build-ops-inventory.ts"));
    const artifact = createArtifactFixture(workspace.root, { existingRepository: true });
    temporaryArtifacts.push(artifact);
    const referencePath = join(workspace.root, ".test-output/session.reference.json");
    writeFileSync(referencePath, JSON.stringify(artifact.reference));

    writeFileSync(
      join(localScripts, "preflight-isolated.sh"),
      '#!/bin/bash\ncount=0\nif [[ -f "$SESSION_PREFLIGHT_CAPTURE" ]]; then count="$(<"$SESSION_PREFLIGHT_CAPTURE")"; fi\ncount=$((count + 1))\nprintf "%s\\n" "$count" > "$SESSION_PREFLIGHT_CAPTURE"\nif [[ "$count" == "2" ]]; then exit "${SESSION_FAKE_SECOND_PREFLIGHT_STATUS:-0}"; fi\nexit 0\n',
    );
    writeFileSync(
      join(localScripts, "wait-session-ready.sh"),
      '#!/bin/bash\nexit "${SESSION_FAKE_READY_STATUS:-41}"\n',
    );
    writeFileSync(
      join(localScripts, "session.sh"),
      '#!/bin/bash\nprintf "%s:%s" "$1" "${2:-}" >> "$CARGO_POLICY_CAPTURE"\nfor arg in "${@:3}"; do printf " %s" "$arg" >> "$CARGO_POLICY_CAPTURE"; done\nprintf "\\n" >> "$CARGO_POLICY_CAPTURE"\nif [[ "$1" == "start" ]]; then\n  mkdir -p "$SCRIPT_KIT_SESSION_DIR/$2"\n  printf "4242\\n" > "$SCRIPT_KIT_SESSION_DIR/$2/pid"\n  if [[ "${SESSION_FAKE_MISSING_GENERATION:-0}" != "1" ]]; then printf "%s\\n" "${SESSION_FAKE_GENERATION:-fake-generation}" > "$SCRIPT_KIT_SESSION_DIR/$2/generation"; fi\nfi\nif [[ "$1" == "status" ]]; then printf \'{"alive":%s,"healthy":%s,"pid":4242}\\n\' "${SESSION_FAKE_ALIVE:-true}" "${SESSION_FAKE_ALIVE:-true}"; fi\n',
    );
    writeFileSync(
      join(localScripts, "build-isolated-binary.sh"),
      `#!/bin/bash\nprintf '%s\\n' '${JSON.stringify({ status: "succeeded", artifact: artifact.reference })}'\n`,
    );
    for (const executable of [
      "preflight-isolated.sh",
      "wait-session-ready.sh",
      "session.sh",
      "build-isolated-binary.sh",
    ]) {
      chmodSync(join(localScripts, executable), 0o755);
    }

    return {
      ...workspace,
      localScripts,
      env: {
        ...Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === "string")),
        SCRIPT_KIT_REPO_ROOT: workspace.root,
        PATH: `${dirname(process.execPath)}:${process.env.PATH}`,
        CARGO_POLICY_CAPTURE: workspace.capture,
        SCRIPT_KIT_ARTIFACT_REFERENCE: referencePath,
        SCRIPT_KIT_SESSION_DIR: join(workspace.root, "session-registry"),
        SESSION_PREFLIGHT_CAPTURE: join(workspace.root, "preflight-count"),
      },
    };
  }

  test("isolated startup preserves the real readiness timeout instead of returning success", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "start-isolated.sh"), "reviewed-session", "--wait-sec", "1"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(41);
    expect(result.stderr.toString()).toContain("exit 41");
  });

  test.each([
    ["41", "ready_timeout"],
    ["42", "app_log_empty"],
  ])("DevTools bootstrap preserves exact readiness failure %s", (exitCode, failureCode) => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never"],
      {
        env: { ...workspace.env, SESSION_FAKE_READY_STATUS: exitCode! },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(Number(exitCode));
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      phase: "wait-ready",
      error: { code: failureCode },
    });
  });

  test.each([
    ["11", "dev_sh_running"],
    ["12", "multiple_gpui_instances"],
    ["13", "binary_missing"],
  ])("post-build preflight preserves exact failure %s without starting a session", (exitCode, failureCode) => {
    const workspace = sessionFixture();
    const referenceTemp = join(workspace.root, "reference-temp");
    const referenceTempAlias = join(workspace.root, "reference-temp-alias");
    mkdirSync(referenceTemp);
    symlinkSync(referenceTemp, referenceTempAlias);
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "always"],
      {
        env: { ...workspace.env, TMPDIR: referenceTempAlias, SESSION_FAKE_SECOND_PREFLIGHT_STATUS: exitCode! },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode, result.stderr.toString()).toBe(Number(exitCode));
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      phase: "preflight",
      error: { code: failureCode },
    });
    expect(existsSync(workspace.capture)).toBe(false);
    expect(readFileSync(workspace.env.SESSION_PREFLIGHT_CAPTURE, "utf8")).toBe("2\n");
    expect(readdirSync(referenceTemp)).toEqual([]);
  });

  test("successful fake readiness still produces a truthful ready receipt", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never"],
      {
        env: { ...workspace.env, SESSION_FAKE_READY_STATUS: "0" },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "ok",
      session: "reviewed-session",
      ready: true,
      pid: 4242,
      ownership: { created: true, pid: 4242, generation: "fake-generation" },
      cleanup: {
        createdSession: true,
        command: "bash scripts/agentic/session.sh stop reviewed-session --expected-pid 4242 --expected-generation fake-generation",
      },
    });
  });

  test("borrowed dev-watch attachment never provides a destructive cleanup command", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "dev-watch", "--mode", "reuse-dev-watch", "--build", "never"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(0);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "ok",
      mode: "reuse-dev-watch",
      session: "dev-watch",
      ownership: { created: false },
      cleanup: { createdSession: false, command: null },
    });
    expect(readFileSync(workspace.capture, "utf8")).not.toContain("stop:");
  });

  test("cleanup refuses the borrowed operator session before any stop command", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "cleanup", "--session", "dev-watch", "--expected-pid", "4242", "--expected-generation", "fake-generation"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(64);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      phase: "cleanup",
      error: { code: "borrowed_session_protected" },
    });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("cleanup refuses a name-only stop without exact ownership evidence", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "cleanup", "--session", "reviewed-session"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(64);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      phase: "cleanup",
      error: { code: "session_ownership_required" },
    });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("owned cleanup forwards the exact PID and generation to the strict session stop", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "cleanup", "--session", "reviewed-session", "--expected-pid", "4242", "--expected-generation", "fake-generation"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(0);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "ok",
      phase: "cleanup",
      ownership: { pid: 4242, generation: "fake-generation" },
    });
    expect(readFileSync(workspace.capture, "utf8")).toBe(
      "stop:reviewed-session --expected-pid 4242 --expected-generation fake-generation\n",
    );
  });

  test("agy cleanup propagates exact owned identity without exposing name-only stop", () => {
    const workspace = sessionFixture();
    copyFileSync(join(scripts, "agy-devtools.sh"), join(workspace.localScripts, "agy-devtools.sh"));
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "agy-devtools.sh"), "cleanup", "--session", "reviewed-session", "--expected-pid", "4242", "--expected-generation", "fake-generation"],
      {
        cwd: workspace.root,
        env: workspace.env,
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "ok",
      ownership: { pid: 4242, generation: "fake-generation" },
    });
    expect(readFileSync(workspace.capture, "utf8")).toContain(
      "stop:reviewed-session --expected-pid 4242 --expected-generation fake-generation\n",
    );
  });

  test("agy cleanup without owned identity cannot stop an unrelated named session", () => {
    const workspace = sessionFixture();
    copyFileSync(join(scripts, "agy-devtools.sh"), join(workspace.localScripts, "agy-devtools.sh"));
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "agy-devtools.sh"), "cleanup", "--session", "reviewed-session"],
      {
        cwd: workspace.root,
        env: workspace.env,
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(64);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      error: { code: "session_ownership_required" },
    });
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([".", "..", "../dev-watch", "owned/child", "unsafe name", "-option"])(
    "DevTools refuses unsafe session identity %s before preflight or mutation",
    (session) => {
      const workspace = sessionFixture();
      const result = Bun.spawnSync(
        ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", session!, "--mode", "isolated", "--build", "never"],
        { env: workspace.env, stdout: "pipe", stderr: "pipe" },
      );

      expect(result.exitCode).toBe(64);
      expect(JSON.parse(result.stdout.toString())).toMatchObject({
        status: "error",
        error: { code: "invalid_session_name" },
      });
      expect(existsSync(workspace.capture)).toBe(false);
      expect(existsSync(workspace.env.SESSION_PREFLIGHT_CAPTURE)).toBe(false);
    },
  );

  test.each(["dev-watch", "..", "../dev-watch", "unsafe name"])(
    "direct isolated startup rejects reserved or unsafe identity %s before preflight",
    (session) => {
      const workspace = sessionFixture();
      const result = Bun.spawnSync(
        ["/bin/bash", join(workspace.localScripts, "start-isolated.sh"), session!],
        { env: workspace.env, stdout: "pipe", stderr: "pipe" },
      );

      expect(result.exitCode).toBe(64);
      expect(existsSync(workspace.capture)).toBe(false);
      expect(existsSync(workspace.env.SESSION_PREFLIGHT_CAPTURE)).toBe(false);
    },
  );

  test("isolated DevTools cannot claim the reserved borrowed dev-watch identity", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "dev-watch", "--mode", "isolated", "--build", "never", "--cleanup-on-fail"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(64);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      error: { code: "borrowed_session_protected" },
    });
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(workspace.env.SESSION_PREFLIGHT_CAPTURE)).toBe(false);
  });

  test.each(["0", "-1", "1.5", "abc"])(
    "DevTools rejects invalid readiness timeout %s before preflight",
    (timeout) => {
      const workspace = sessionFixture();
      const result = Bun.spawnSync(
        ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never", "--ready-timeout-sec", timeout!],
        { env: workspace.env, stdout: "pipe", stderr: "pipe" },
      );

      expect(result.exitCode).toBe(64);
      expect(JSON.parse(result.stdout.toString())).toMatchObject({
        status: "error",
        error: { code: "invalid_timeout" },
      });
      expect(existsSync(workspace.env.SESSION_PREFLIGHT_CAPTURE)).toBe(false);
    },
  );

  test.each(["0", "-1", "1.5", "abc"])(
    "DevTools rejects invalid RPC timeout %s before preflight",
    (timeout) => {
      const workspace = sessionFixture();
      const result = Bun.spawnSync(
        ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never", "--rpc-timeout-ms", timeout!],
        { env: workspace.env, stdout: "pipe", stderr: "pipe" },
      );

      expect(result.exitCode).toBe(64);
      expect(JSON.parse(result.stdout.toString())).toMatchObject({
        error: { code: "invalid_timeout" },
      });
      expect(existsSync(workspace.env.SESSION_PREFLIGHT_CAPTURE)).toBe(false);
    },
  );

  test.each([
    ["--mode", "unexpected", "invalid_mode"],
    ["--build", "unexpected", "invalid_build_policy"],
    ["--rust-changed", "unexpected", "invalid_rust_change_policy"],
  ])("DevTools rejects unknown %s policy before preflight", (option, value, errorCode) => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", option!, value!],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(64);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      error: { code: errorCode },
    });
    expect(existsSync(workspace.env.SESSION_PREFLIGHT_CAPTURE)).toBe(false);
  });

  test.each(["0", "-1", "1.5", "abc"])(
    "direct isolated startup rejects invalid readiness timeout %s before preflight",
    (timeout) => {
      const workspace = sessionFixture();
      const result = Bun.spawnSync(
        ["/bin/bash", join(workspace.localScripts, "start-isolated.sh"), "reviewed-session", "--wait-sec", timeout!],
        { env: workspace.env, stdout: "pipe", stderr: "pipe" },
      );

      expect(result.exitCode).toBe(64);
      expect(existsSync(workspace.env.SESSION_PREFLIGHT_CAPTURE)).toBe(false);
    },
  );

  test("cleanup-on-failure strictly stops only the newly created exact session", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never", "--cleanup-on-fail"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(41);
    expect(readFileSync(workspace.capture, "utf8")).toContain(
      "stop:reviewed-session --expected-pid 4242 --expected-generation fake-generation\n",
    );
  });

  test("cleanup-on-failure never stops a preexisting borrowed named session", () => {
    const workspace = sessionFixture();
    const existing = join(workspace.env.SCRIPT_KIT_SESSION_DIR, "reviewed-session");
    mkdirSync(existing, { recursive: true });
    writeFileSync(join(existing, "pid"), "4242\n");
    writeFileSync(join(existing, "generation"), "preexisting-generation\n");
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never", "--cleanup-on-fail"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(41);
    expect(readFileSync(workspace.capture, "utf8")).not.toContain("stop:");
  });

  test("successful borrowed named-session reuse does not authorize cleanup", () => {
    const workspace = sessionFixture();
    const existing = join(workspace.env.SCRIPT_KIT_SESSION_DIR, "reviewed-session");
    mkdirSync(existing, { recursive: true });
    writeFileSync(join(existing, "pid"), "4242\n");
    writeFileSync(join(existing, "generation"), "preexisting-generation\n");
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never"],
      {
        env: { ...workspace.env, SESSION_FAKE_READY_STATUS: "0" },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "ok",
      ownership: { created: false },
      cleanup: { createdSession: false, command: null },
    });
  });

  test("successful readiness fails closed when the owned session lacks a generation", () => {
    const workspace = sessionFixture();
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "never"],
      {
        env: { ...workspace.env, SESSION_FAKE_READY_STATUS: "0", SESSION_FAKE_MISSING_GENERATION: "1" },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(64);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      error: { code: "session_ownership_missing" },
    });
  });

  test.each(["0", "-1", "1.5", "abc"])(
    "standalone readiness waiter rejects invalid timeout %s before inspecting a session",
    (timeout) => {
      const workspace = sessionFixture();
      copyFileSync(join(scripts, "wait-session-ready.sh"), join(workspace.localScripts, "wait-session-ready.sh"));
      const result = Bun.spawnSync(
        ["/bin/bash", join(workspace.localScripts, "wait-session-ready.sh"), "reviewed-session", timeout!],
        { env: workspace.env, stdout: "pipe", stderr: "pipe" },
      );

      expect(result.exitCode).toBe(64);
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test.each([
    ["startup", "STARTUP_READY stale-generation\n", ""],
    ["app", "APP_READY|stale-generation\n", ""],
    ["protocol", "", '{"responseType":"stateResult"}\n'],
  ])("standalone readiness refuses a stale %s marker from a dead session", (_kind, log, bus) => {
    const workspace = sessionFixture();
    copyFileSync(join(scripts, "wait-session-ready.sh"), join(workspace.localScripts, "wait-session-ready.sh"));
    const session = join(workspace.env.SCRIPT_KIT_SESSION_DIR, "reviewed-session");
    mkdirSync(session, { recursive: true });
    writeFileSync(join(session, "app.log"), log!);
    writeFileSync(join(session, "protocol-responses.ndjson"), bus!);
    writeFileSync(join(session, "generation"), "stale-generation\n");
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "wait-session-ready.sh"), "reviewed-session", "1"],
      {
        env: { ...workspace.env, SESSION_FAKE_ALIVE: "false" },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(1);
    expect(result.stderr.toString()).toContain("not alive");
  });

  test("standalone readiness still accepts a current marker from a live owned session", () => {
    const workspace = sessionFixture();
    copyFileSync(join(scripts, "wait-session-ready.sh"), join(workspace.localScripts, "wait-session-ready.sh"));
    const session = join(workspace.env.SCRIPT_KIT_SESSION_DIR, "reviewed-session");
    mkdirSync(session, { recursive: true });
    writeFileSync(join(session, "app.log"), "STARTUP_READY live-generation\n");
    writeFileSync(join(session, "generation"), "live-generation\n");
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "wait-session-ready.sh"), "reviewed-session", "1"],
      { env: workspace.env, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(0);
    expect(result.stderr.toString()).toContain("STARTUP_READY");
    expect(readFileSync(workspace.capture, "utf8")).toContain("status:reviewed-session\n");
  });
});

describe("development, correctness-test, and CI profile separation", () => {
  test("required CI strictly lints both the library and shipping binary", () => {
    const workflow = Bun.YAML.parse(
      readFileSync(join(scripts, "..", "..", ".github", "workflows", "ci.yml"), "utf8"),
    ) as { jobs: { clippy: { steps: { run?: string }[] } } };
    const appLint = workflow.jobs.clippy.steps.filter(
      (step: { run?: string }) => step.run?.includes("cargo clippy") && step.run.includes("--lib"),
    );

    expect(appLint).toHaveLength(1);
    expect(appLint[0].run.trim().split(/\s+/)).toEqual([
      "cargo",
      "clippy",
      "--locked",
      "--lib",
      "--bin",
      "script-kit-gpui",
      "--no-deps",
      "--",
      "-D",
      "warnings",
    ]);
  });

  test("keeps interactive rendering optimized while correctness harnesses remain unoptimized", () => {
    const manifest = Bun.TOML.parse(readFileSync(join(scripts, "..", "..", "Cargo.toml"), "utf8"));
    expect(manifest).toHaveProperty(["profile", "dev", "package", "*", "opt-level"], 2);
    expect(manifest).toHaveProperty(["profile", "dev", "package", "gpui", "opt-level"], 2);
    expect(manifest).toHaveProperty(["profile", "dev", "package", "gpui-component", "opt-level"], 2);
    expect(manifest).toHaveProperty(["profile", "test", "package", "*", "opt-level"], 0);
    expect(manifest).toHaveProperty(["profile", "test", "package", "gpui", "opt-level"], 0);
    expect(manifest).toHaveProperty(["profile", "test", "package", "gpui-component", "opt-level"], 0);
  });

  test("does not force incompatible global incremental settings into sccache", () => {
    const config = Bun.TOML.parse(
      readFileSync(join(scripts, "..", "..", ".cargo", "config.toml"), "utf8"),
    );
    expect(config).not.toHaveProperty(["env", "CARGO_INCREMENTAL"]);
    expect(config).toHaveProperty(["env", "CARGO_PROFILE_DEV_INCREMENTAL"], "true");
    expect(config).toHaveProperty(["env", "CARGO_PROFILE_TEST_INCREMENTAL"], "true");

    for (const workflow of ["ci.yml", "release.yml"]) {
      const parsed = Bun.YAML.parse(
        readFileSync(join(scripts, "..", "..", ".github", "workflows", workflow), "utf8"),
      );
      expect(parsed).toHaveProperty(["env", "CARGO_INCREMENTAL"], "0");
      expect(parsed).toHaveProperty(["env", "SCRIPT_KIT_NONINTERACTIVE"], "1");
    }
  });

  test("Metal shader bindings never rebuild the entire GPUI crate as a build dependency", () => {
    const manifest = Bun.TOML.parse(
      readFileSync(join(scripts, "..", "..", "vendor", "gpui_macos", "Cargo.toml"), "utf8"),
    );
    expect(manifest).toHaveProperty(["target", 'cfg(target_os = "macos")', "build-dependencies", "cbindgen"]);
    expect(manifest).not.toHaveProperty(["target", 'cfg(target_os = "macos")', "build-dependencies", "gpui"]);
    expect(manifest).toHaveProperty(["dependencies", "gpui"]);
  });

  test("docs-only commits do not rebuild local app harnesses while release provenance stays exact", () => {
    const output = join(temporaryDirectory("script-kit-build-policy-"), "build-policy-tests");
    const compiled = Bun.spawnSync(
      ["rustc", "--edition=2021", "--test", join(scripts, "..", "..", "build.rs"), "-o", output],
      { stdout: "pipe", stderr: "pipe" },
    );
    expect(compiled.exitCode).toBe(0);

    const behavior = Bun.spawnSync([output, "--test-threads=1"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    expect(behavior.exitCode).toBe(0);
    expect(behavior.stdout.toString()).toContain("2 passed; 0 failed");
  });
});
