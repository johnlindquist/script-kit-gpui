import { describe, expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const scripts = import.meta.dir;

function fixture() {
  const root = mkdtempSync(join(tmpdir(), "script-kit-cargo-policy-"));
  const bin = join(root, "fake-bin");
  const localScripts = join(root, "scripts", "agentic");
  mkdirSync(bin, { recursive: true });
  mkdirSync(localScripts, { recursive: true });
  mkdirSync(join(root, "target-agent", ".locks"), { recursive: true });
  mkdirSync(join(root, "target-agent", "pools", "agent-debug"), {
    recursive: true,
  });
  mkdirSync(join(root, "target-agent", "shared"), { recursive: true });
  mkdirSync(join(root, "target-agent", "artifacts"), { recursive: true });

  for (const script of [
    "cargo-cache-locks.sh",
    "prune-cargo-targets.sh",
    "disk-space-cargo-emergency-clean.sh",
  ]) {
    symlinkSync(join(scripts, script), join(localScripts, script));
  }

  const capture = join(root, "cargo-invocation.txt");
  writeFileSync(
    join(bin, "cargo"),
    `#!/bin/bash
printf 'jobs=%s\\ntest_threads=%s\\nnoninteractive=%s\\nsearch_stress=%s\\nstorage_stress=%s\\nmodule=%s\\nwrapper=%s\\nsocket=%s\\nargs=' "$CARGO_BUILD_JOBS" "\${RUST_TEST_THREADS:-}" "\${SCRIPT_KIT_NONINTERACTIVE:-}" "\${SCRIPT_KIT_SEARCH_FULL_STRESS:-}" "\${SCRIPT_KIT_STORAGE_FULL_STRESS:-}" "$SCRIPT_KIT_METAL_MODULE_CACHE_DIR" "\${RUSTC_WRAPPER:-}" "\${SCCACHE_SERVER_UDS:-}" > "$CARGO_POLICY_CAPTURE"
printf '%s ' "$@" >> "$CARGO_POLICY_CAPTURE"
printf '\\n' >> "$CARGO_POLICY_CAPTURE"
`,
  );
  chmodSync(join(bin, "cargo"), 0o755);

  return {
    root,
    bin,
    capture,
    env: {
      ...process.env,
      PATH: `${bin}:/usr/bin:/bin`,
      SCRIPT_KIT_REPO_ROOT: root,
      CARGO_POLICY_CAPTURE: capture,
      SCRIPT_KIT_AGENT_USE_SCCACHE: "0",
      SCRIPT_KIT_AGENT_MIN_FREE_GB: "0",
      SCRIPT_KIT_AGENT_CRITICAL_FREE_GB: "0",
    },
  };
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

function installFakeSccache(workspace: ReturnType<typeof fixture>, usable = true) {
  writeFileSync(
    join(workspace.bin, "sccache"),
    `#!/bin/bash\nif [[ "$1" == "--show-stats" ]]; then exit 0; fi\nexit ${usable ? "0" : "2"}\n`,
  );
  writeFileSync(join(workspace.bin, "rustc"), "#!/bin/bash\nexit 0\n");
  chmodSync(join(workspace.bin, "sccache"), 0o755);
  chmodSync(join(workspace.bin, "rustc"), 0o755);
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

  test("agent-scoped verification owns safe child permissions and reviews only bounded Rust targets", () => {
    const workspace = fixture();
    writeFileSync(join(workspace.bin, "rg"), '#!/bin/bash\nprintf "src/ai/reliability.rs\\n"\n');
    chmodSync(join(workspace.bin, "rg"), 0o755);
    writeFileSync(
      join(workspace.bin, "cargo"),
      '#!/bin/bash\nprintf "args=%s noninteractive=%s takeover=%s input=%s capture=%s visible=%s live_ai=%s app=%s search=%s storage=%s\\n" "$*" "$SCRIPT_KIT_NONINTERACTIVE" "$SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER" "$SCRIPT_KIT_ALLOW_NATIVE_INPUT" "$SCRIPT_KIT_ALLOW_SCREEN_CAPTURE" "$SCRIPT_KIT_ALLOW_VISIBLE_PROBES" "$SCRIPT_KIT_ALLOW_LIVE_AI" "$SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH" "$SCRIPT_KIT_SEARCH_FULL_STRESS" "$SCRIPT_KIT_STORAGE_FULL_STRESS" >> "$CARGO_POLICY_CAPTURE"\n',
    );
    const result = Bun.spawnSync(
      ["/bin/bash", join(scripts, "..", "agent-check.sh"), "src/ai/reliability.rs"],
      {
        env: {
          ...workspace.env,
          SCRIPT_KIT_NONINTERACTIVE: "0",
          SCRIPT_KIT_SEARCH_FULL_STRESS: "1",
          SCRIPT_KIT_STORAGE_FULL_STRESS: "1",
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(0);
    const invocations = readFileSync(workspace.capture, "utf8").trim().split("\n");
    expect(invocations).toHaveLength(5);
    expect(invocations.map((line) => line.split(" noninteractive=")[0])).toEqual([
      "args=check --locked --lib --bin script-kit-gpui",
      "args=test --locked --lib reliability",
      "args=clippy --locked --lib --no-deps -- -D warnings",
      "args=test --locked --lib",
      "args=test --locked -p sk-clipboard -p sk-protocol -p sk-storage",
    ]);
    for (const invocation of invocations) {
      expect(invocation).toContain(
        "noninteractive=1 takeover=0 input=0 capture=0 visible=0 live_ai=0 app=0 search=0 storage=0",
      );
    }
  });

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

    const receipt = JSON.parse(
      readFileSync(join(workspace.root, "target-agent", "build-receipts.jsonl"), "utf8"),
    );
    expect(receipt).toMatchObject({
      status: 0,
      pool: "agent-debug",
      cache: "cold",
      jobs: 2,
      test_threads: 2,
      command: "test",
      timings: 1,
    });
  });

  test.each([
    ["--jobs", "8"],
    ["--jobs=8"],
    ["-j", "8"],
    ["-j8"],
  ])("refuses explicit compiler-worker bypass %j before Cargo runs", (...workerArgs) => {
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
  ])("refuses protected-pool target-directory escape %j before Cargo runs", (...targetArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib", ...targetArgs], workspace.env);

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("target directory is owned by the protected Cargo pool");
    expect(existsSync(workspace.capture)).toBe(false);
  });

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
      expect(result.stderr).toContain("must name one owned cache child");
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test.each([".", ".."])(
    "refuses traversal-like artifact export identifier %s before Cargo runs",
    (artifactName) => {
      const workspace = fixture();
      const result = run("agent-cargo.sh", ["build", "--bin", "export_design_tokens"], {
        ...workspace.env,
        SCRIPT_KIT_AGENT_ARTIFACT_NAME: artifactName,
      });

      expect(result.status).toBe(64);
      expect(result.stderr).toContain("must name one owned cache child");
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test("refuses a symlinked build pool before touching its external destination", () => {
    const workspace = fixture();
    const external = mkdtempSync(join(tmpdir(), "script-kit-external-pool-"));
    symlinkSync(external, join(workspace.root, "target-agent", "pools", "escaped"));
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_CARGO_TARGET_POOL: "escaped",
    });

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("protected cache ownership cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(external, ".last_used"))).toBe(false);
  });

  test("refuses a symlinked artifact export before Cargo can write outside its owner", () => {
    const workspace = fixture();
    const external = mkdtempSync(join(tmpdir(), "script-kit-external-artifact-"));
    symlinkSync(external, join(workspace.root, "target-agent", "artifacts", "escaped"));
    const result = run("agent-cargo.sh", ["build", "--bin", "export_design_tokens"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_ARTIFACT_NAME: "escaped",
    });

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("protected cache ownership cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(external, "export_design_tokens"))).toBe(false);
  });

  test.each([
    ["--config", "build.jobs=48"],
    ["--config=build.jobs=48"],
    ["--config", 'build.target-dir="target"'],
    ["--config", "/tmp/foreign-cargo-config.toml"],
  ])("refuses command-line Cargo config policy bypass %j before Cargo runs", (...configArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib", ...configArgs], workspace.env);

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("command-line Cargo config cannot override protected build policy");
    expect(existsSync(workspace.capture)).toBe(false);
  });

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
  ])("refuses unsafe system/ignored test activation %j before Cargo runs", (...unsafeArgs) => {
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
  ])("refuses unreviewed blanket Rust test discovery %j before Cargo runs", (...unscopedArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", unscopedArgs, workspace.env);

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("requires an explicit reviewed --lib, --test, or safe domain package");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["run"],
    ["run", "--bin", "script-kit-gpui"],
    ["run", "--bin=liquid-glass-demo"],
    ["bench"],
  ])("refuses application launch or live benchmark %j before Cargo runs", (...launchArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", launchArgs, workspace.env);

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo refuses application launch or live benchmarks");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    ["test", "--locked", "--lib"],
    ["test", "--locked", "--test", "protocol_batch"],
    ["test", "-p", "sk-protocol"],
    ["test", "--package=sk-storage"],
    ["nextest", "run", "--lib"],
    ["run", "--bin", "export_design_tokens"],
  ])("preserves explicitly reviewed app, domain, and exporter command %j", (...reviewedArgs) => {
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
  ])("refuses Rust-harness thread bypass %j before Cargo runs", (...threadArgs) => {
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

  test("exports a cheap correctness-profile binary from Cargo's actual debug directory", () => {
    const workspace = fixture();
    const binary = join(
      workspace.root,
      "target-agent",
      "pools",
      "agent-debug",
      "debug",
      "export_design_tokens",
    );
    mkdirSync(join(workspace.root, "target-agent", "pools", "agent-debug", "debug"), {
      recursive: true,
    });
    writeFileSync(binary, "#!/bin/sh\nprintf 'real-exporter\\n'\n");
    chmodSync(binary, 0o755);

    const result = run(
      "agent-cargo.sh",
      ["build", "--profile", "test", "--bin", "export_design_tokens"],
      { ...workspace.env, SCRIPT_KIT_AGENT_ARTIFACT_NAME: "consistency-gov005" },
    );
    const exported = join(
      workspace.root,
      "target-agent",
      "artifacts",
      "consistency-gov005",
      "export_design_tokens",
    );

    expect(result.status).toBe(0);
    expect(result.stderr).toContain(`artifact bin=export_design_tokens path=${exported}`);
    expect(readFileSync(exported, "utf8")).toBe(readFileSync(binary, "utf8"));
  });

  test("fails before launching Cargo when the available disk floor is impossible", () => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      SCRIPT_KIT_AGENT_MIN_FREE_GB: "99999",
    });

    expect(result.status).toBe(75);
    expect(result.stderr).toContain("refusing an unpredictable build");
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
    const result = run("agent-cargo.sh", ["check", "--lib"], {
      ...workspace.env,
      RUSTC_WRAPPER: "/existing/controlled-wrapper",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toContain(
      "wrapper=/existing/controlled-wrapper\n",
    );
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
    expect(invocation).toContain("wrapper=sccache\n");
    expect(invocation).toContain(
      `socket=${join(workspace.root, "target-agent", "shared", "sccache.sock")}\n`,
    );
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
    expect(readFileSync(workspace.capture, "utf8")).toContain("wrapper=\n");
  });

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
});

describe("Cargo cache ownership", () => {
  test("removes only stale unlocked pools and preserves live, warm, shared, and artifact caches", () => {
    const workspace = fixture();
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
    const incomplete = join(workspace.root, "target-agent", "pools", "starting-owner");
    makeOld(incomplete);
    liveLock(workspace.root, "starting-owner", false);

    const result = run("prune-cargo-targets.sh", ["--apply"], workspace.env);

    expect(result.status).toBe(0);
    expect(existsSync(incomplete)).toBe(true);
  });

  test("emergency cleanup never terminates a live owner or deletes its pool", () => {
    const workspace = fixture();
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

describe("reviewed Rust harness reuse", () => {
  test("rejects stale binaries before executing any test", () => {
    const workspace = fixture();
    for (const source of ["src", "crates", "vendor", ".cargo"]) {
      mkdirSync(join(workspace.root, source), { recursive: true });
    }
    writeFileSync(join(workspace.root, "Cargo.toml"), "[workspace]\n");
    writeFileSync(join(workspace.root, "Cargo.lock"), "version = 4\n");
    const deps = join(workspace.root, "target-agent", "pools", "agent-debug", "debug", "deps");
    mkdirSync(deps, { recursive: true });
    const binary = join(deps, "script_kit_gpui-test-fixture");
    writeFileSync(binary, "#!/bin/bash\nexit 99\n");
    chmodSync(binary, 0o755);
    const old = new Date(Date.now() - 60_000);
    utimesSync(binary, old, old);
    writeFileSync(join(workspace.root, "src", "newer.rs"), "// updated source\n");

    const result = run("reuse-rust-test-binary.sh", ["safe_filter"], workspace.env);

    expect(result.status).toBe(65);
    expect(result.stderr).toContain("cached harness is older than");
  });

  test("executes current reviewed filters serially with every takeover permission disabled", () => {
    const workspace = fixture();
    for (const source of ["src", "crates", "vendor", ".cargo"]) {
      mkdirSync(join(workspace.root, source), { recursive: true });
    }
    writeFileSync(join(workspace.root, "Cargo.toml"), "[workspace]\n");
    writeFileSync(join(workspace.root, "Cargo.lock"), "version = 4\n");
    const deps = join(workspace.root, "target-agent", "pools", "agent-debug", "debug", "deps");
    mkdirSync(deps, { recursive: true });
    const binary = join(deps, "script_kit_gpui-current-fixture");
    writeFileSync(
      binary,
      '#!/bin/bash\nprintf "%s:%s:%s:%s:%s\\n" "$1" "$SCRIPT_KIT_NONINTERACTIVE" "$SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER" "$SCRIPT_KIT_ALLOW_NATIVE_INPUT" "$SCRIPT_KIT_ALLOW_LIVE_AI" >> "$CARGO_POLICY_CAPTURE"\n',
    );
    chmodSync(binary, 0o755);

    const result = run(
      "reuse-rust-test-binary.sh",
      ["first_reviewed_group", "second_reviewed_group"],
      workspace.env,
    );

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toBe(
      "first_reviewed_group:1:0:0:0\nsecond_reviewed_group:1:0:0:0\n",
    );
  });
});

describe("development, correctness-test, and CI profile separation", () => {
  test("keeps interactive rendering optimized while correctness harnesses remain unoptimized", () => {
    const manifest = Bun.TOML.parse(readFileSync(join(scripts, "..", "..", "Cargo.toml"), "utf8"));
    expect(manifest.profile.dev.package["*"]["opt-level"]).toBe(2);
    expect(manifest.profile.dev.package.gpui["opt-level"]).toBe(2);
    expect(manifest.profile.dev.package["gpui-component"]["opt-level"]).toBe(2);
    expect(manifest.profile.test.package["*"]["opt-level"]).toBe(0);
    expect(manifest.profile.test.package.gpui["opt-level"]).toBe(0);
    expect(manifest.profile.test.package["gpui-component"]["opt-level"]).toBe(0);
  });

  test("does not force incompatible global incremental settings into sccache", () => {
    const config = Bun.TOML.parse(
      readFileSync(join(scripts, "..", "..", ".cargo", "config.toml"), "utf8"),
    );
    expect(config.env.CARGO_INCREMENTAL).toBeUndefined();
    expect(config.env.CARGO_PROFILE_DEV_INCREMENTAL).toBe("true");
    expect(config.env.CARGO_PROFILE_TEST_INCREMENTAL).toBe("true");

    for (const workflow of ["ci.yml", "release.yml"]) {
      const parsed = Bun.YAML.parse(
        readFileSync(join(scripts, "..", "..", ".github", "workflows", workflow), "utf8"),
      );
      expect(parsed.env.CARGO_INCREMENTAL).toBe("0");
      expect(parsed.env.SCRIPT_KIT_NONINTERACTIVE).toBe("1");
    }
  });

  test("Metal shader bindings never rebuild the entire GPUI crate as a build dependency", () => {
    const manifest = Bun.TOML.parse(
      readFileSync(join(scripts, "..", "..", "vendor", "gpui_macos", "Cargo.toml"), "utf8"),
    );
    const macos = manifest.target['cfg(target_os = "macos")'];
    expect(macos["build-dependencies"].cbindgen).toBeDefined();
    expect(macos["build-dependencies"].gpui).toBeUndefined();
    expect(manifest.dependencies.gpui).toBeDefined();
  });

  test("docs-only commits do not rebuild local app harnesses while release provenance stays exact", () => {
    const output = join(mkdtempSync(join(tmpdir(), "script-kit-build-policy-")), "build-policy-tests");
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
