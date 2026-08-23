import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const scripts = import.meta.dir;
const temporaryDirectories: string[] = [];

function temporaryDirectory(prefix: string) {
  const path = mkdtempSync(join(tmpdir(), prefix));
  temporaryDirectories.push(path);
  return path;
}

afterEach(() => {
  for (const path of temporaryDirectories.splice(0)) {
    rmSync(path, { recursive: true, force: true });
  }
});

function fixture() {
  const root = temporaryDirectory("script-kit-cargo-policy-");
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

function installFakeGit(workspace: ReturnType<typeof fixture>) {
  writeFileSync(
    join(workspace.bin, "git"),
    `#!/bin/bash
if [[ -n "\${CARGO_POLICY_GIT_ARGS_CAPTURE:-}" ]]; then printf '%s\\n' "$*" >> "$CARGO_POLICY_GIT_ARGS_CAPTURE"; fi
if [[ "$1" == "-C" ]]; then shift 2; fi
case "$1" in
  rev-parse)
    if [[ "$2" == "--is-inside-work-tree" ]]; then
      printf 'true\\n'
    elif [[ "\${CARGO_POLICY_GIT_CHANGE_AFTER_CARGO:-0}" == "1" && -f "$CARGO_POLICY_CAPTURE" ]]; then
      printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\\n'
    else
      printf '%s\\n' "\${CARGO_POLICY_GIT_HEAD:-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}"
    fi
    ;;
  status)
    if [[ "\${CARGO_POLICY_GIT_DIRTY:-0}" == "1" ]]; then printf ' M src/example.rs\\n'; fi
    ;;
  ls-tree)
    printf '100644 blob aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\tsrc/example.rs\\n'
    ;;
  diff)
    if [[ "\${CARGO_POLICY_GIT_DIRTY:-0}" == "1" ]]; then printf 'src/example.rs\\n'; fi
    ;;
  *) exit 64 ;;
esac
`,
  );
  chmodSync(join(workspace.bin, "git"), 0o755);
}

function fakeBuiltBinary(
  workspace: ReturnType<typeof fixture>,
  name = "export_design_tokens",
  profile = "debug",
) {
  const binary = join(
    workspace.root,
    "target-agent",
    "pools",
    "agent-debug",
    profile,
    name,
  );
  mkdirSync(join(workspace.root, "target-agent", "pools", "agent-debug", profile), {
    recursive: true,
  });
  writeFileSync(binary, "#!/bin/sh\nprintf 'real-exporter\\n'\n");
  chmodSync(binary, 0o755);
  return binary;
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
    const external = temporaryDirectory("script-kit-external-pool-");
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
    const external = temporaryDirectory("script-kit-external-artifact-");
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
    ["test", "--lib", "--manifest-path", "/tmp/unreviewed/Cargo.toml"],
    ["test", "--lib", "--manifest-path=/tmp/unreviewed/Cargo.toml"],
    ["check", "--manifest-path", "/tmp/unreviewed/Cargo.toml"],
    ["run", "--bin", "export_design_tokens", "--manifest-path", "/tmp/unreviewed/Cargo.toml"],
    ["test", "--lib", "-C", "/tmp/unreviewed"],
  ])("refuses foreign Cargo workspace ownership %j before Cargo runs", (...foreignArgs) => {
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
  ])("refuses unreviewed Cargo package expansion %j before Cargo runs", (...packageArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", packageArgs, workspace.env);

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo refuses unreviewed workspace or package");
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
    ["doc", "--open"],
  ])("refuses application launch or live benchmark %j before Cargo runs", (...launchArgs) => {
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
  ])("refuses unreviewed Cargo alias or external subcommand %j before startup", (...aliasArgs) => {
    const workspace = fixture();
    const result = run("agent-cargo.sh", aliasArgs, workspace.env);

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("noninteractive agent Cargo refuses unreviewed subcommand or alias");
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
    installFakeGit(workspace);
    const binary = fakeBuiltBinary(workspace);

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
    const manifest = JSON.parse(readFileSync(`${exported}.provenance.json`, "utf8"));
    expect(manifest).toMatchObject({
      schemaVersion: 2,
      pool: "agent-debug",
      binaryPath: "target-agent/artifacts/consistency-gov005/export_design_tokens",
      binarySha256: createHash("sha256").update(readFileSync(exported)).digest("hex"),
      sizeBytes: readFileSync(exported).byteLength,
      gitHead: "a".repeat(40),
      profile: "debug",
      requiresExactGitHead: false,
      rustDirty: false,
    });
    expect(manifest.compilerInputSha256).toMatch(/^[a-f0-9]{64}$/);
  });

  test("marks dirty compiler inputs explicitly instead of claiming a committed build", () => {
    const workspace = fixture();
    installFakeGit(workspace);
    fakeBuiltBinary(workspace);
    const result = run(
      "agent-cargo.sh",
      ["build", "--bin", "export_design_tokens"],
      {
        ...workspace.env,
        SCRIPT_KIT_AGENT_ARTIFACT_NAME: "dirty-source",
        CARGO_POLICY_GIT_DIRTY: "1",
      },
    );
    const manifestPath = join(
      workspace.root,
      "target-agent",
      "artifacts",
      "dirty-source",
      "export_design_tokens.provenance.json",
    );
    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(manifestPath, "utf8")).rustDirty).toBe(true);
  });

  test("source fingerprints include vendored Rust and every real embedded SDK asset", () => {
    const workspace = fixture();
    installFakeGit(workspace);
    fakeBuiltBinary(workspace);
    const capture = join(workspace.root, "reviewed-compiler-paths.txt");
    const result = run(
      "agent-cargo.sh",
      ["build", "--bin", "export_design_tokens"],
      {
        ...workspace.env,
        SCRIPT_KIT_AGENT_ARTIFACT_NAME: "complete-source-tree",
        CARGO_POLICY_GIT_ARGS_CAPTURE: capture,
      },
    );
    expect(result.status).toBe(0);
    const observed = readFileSync(capture, "utf8");
    for (const owner of ["src", "crates", "vendor", "assets", "kit-init", "scripts/kit-sdk.ts"]) {
      expect(observed).toContain(owner);
    }
  });

  test.each([
    ["release profile", ["build", "--release", "--bin", "export_design_tokens"], {}, "release"],
    ["hosted CI", ["build", "--bin", "export_design_tokens"], { GITHUB_SHA: "ci-commit" }, "debug"],
    [
      "explicit source tracking",
      ["build", "--bin", "export_design_tokens"],
      { SCRIPT_KIT_TRACK_GIT_HEAD: "1" },
      "debug",
    ],
  ])("%s retains exact build-commit provenance", (_label, args, overrides, profile) => {
    const workspace = fixture();
    installFakeGit(workspace);
    fakeBuiltBinary(workspace, "export_design_tokens", profile as string);
    const result = run("agent-cargo.sh", args as string[], {
      ...workspace.env,
      ...(overrides as Record<string, string>),
      SCRIPT_KIT_AGENT_ARTIFACT_NAME: "exact-source",
    });
    const manifestPath = join(
      workspace.root,
      "target-agent",
      "artifacts",
      "exact-source",
      "export_design_tokens.provenance.json",
    );
    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(manifestPath, "utf8"))).toMatchObject({
      profile,
      requiresExactGitHead: true,
    });
  });

  test("refuses an exported build before Cargo when its Git source cannot be observed", () => {
    const workspace = fixture();
    const result = run(
      "agent-cargo.sh",
      ["build", "--bin", "export_design_tokens"],
      { ...workspace.env, SCRIPT_KIT_AGENT_ARTIFACT_NAME: "missing-source" },
    );
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("independently observed Git source commit");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("refuses exported provenance when HEAD changes while the owned fake build runs", () => {
    const workspace = fixture();
    installFakeGit(workspace);
    fakeBuiltBinary(workspace);
    const result = run(
      "agent-cargo.sh",
      ["build", "--bin", "export_design_tokens"],
      {
        ...workspace.env,
        SCRIPT_KIT_AGENT_ARTIFACT_NAME: "changed-source",
        CARGO_POLICY_GIT_CHANGE_AFTER_CARGO: "1",
      },
    );
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("Git source commit changed during the build");
    expect(existsSync(join(
      workspace.root,
      "target-agent",
      "artifacts",
      "changed-source",
      "export_design_tokens",
    ))).toBe(false);
  });

  test("refuses a symlinked exported provenance manifest before Cargo runs", () => {
    const workspace = fixture();
    installFakeGit(workspace);
    fakeBuiltBinary(workspace);
    const external = temporaryDirectory("script-kit-external-provenance-");
    const protectedFile = join(external, "preserved.json");
    writeFileSync(protectedFile, "private-existing-state\n");
    const artifactDirectory = join(workspace.root, "target-agent", "artifacts", "symlinked-manifest");
    mkdirSync(artifactDirectory, { recursive: true });
    symlinkSync(protectedFile, join(artifactDirectory, "export_design_tokens.provenance.json"));
    const result = run(
      "agent-cargo.sh",
      ["build", "--bin", "export_design_tokens"],
      { ...workspace.env, SCRIPT_KIT_AGENT_ARTIFACT_NAME: "symlinked-manifest" },
    );
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("artifact provenance cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
    expect(readFileSync(protectedFile, "utf8")).toBe("private-existing-state\n");
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
  function currentHarness(pool = "agent-debug") {
    const workspace = fixture();
    installFakeGit(workspace);
    for (const source of ["src", "crates", "vendor", ".cargo"]) {
      mkdirSync(join(workspace.root, source), { recursive: true });
    }
    writeFileSync(join(workspace.root, "Cargo.toml"), "[workspace]\n");
    writeFileSync(join(workspace.root, "Cargo.lock"), "version = 4\n");
    const deps = join(workspace.root, "target-agent", "pools", pool, "debug", "deps");
    mkdirSync(deps, { recursive: true });
    const binary = join(deps, "script_kit_gpui-current-fixture");
    writeFileSync(
      binary,
      '#!/bin/bash\nprintf "%s:%s:%s:%s:%s:%s:%s\\n" "$1" "$SCRIPT_KIT_NONINTERACTIVE" "$SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER" "$SCRIPT_KIT_ALLOW_NATIVE_INPUT" "$SCRIPT_KIT_ALLOW_LIVE_AI" "${SCRIPT_KIT_SEARCH_FULL_STRESS:-}" "${SCRIPT_KIT_STORAGE_FULL_STRESS:-}" >> "$CARGO_POLICY_CAPTURE"\n',
    );
    chmodSync(binary, 0o755);
    return { ...workspace, deps, binary };
  }

  test("rejects stale binaries before executing any test", () => {
    const workspace = fixture();
    installFakeGit(workspace);
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
    installFakeGit(workspace);
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

  test("backdated dirty Rust source cannot masquerade as a current cached harness", () => {
    const workspace = currentHarness();
    const source = join(workspace.root, "src", "backdated.rs");
    writeFileSync(source, "// modified compiled source\n");
    const old = new Date(Date.now() - 120_000);
    utimesSync(source, old, old);
    const result = run("reuse-rust-test-binary.sh", ["reviewed_case"], {
      ...workspace.env,
      CARGO_POLICY_GIT_DIRTY: "1",
    });
    expect(result.status).toBe(65);
    expect(result.stderr).toContain("uncommitted reviewed compiler inputs");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("cached harness refuses a symlinked compiler-input owner before execution", () => {
    const workspace = currentHarness();
    const external = temporaryDirectory("script-kit-external-rust-input-");
    rmSync(join(workspace.root, "src"), { recursive: true, force: true });
    symlinkSync(external, join(workspace.root, "src"));
    const result = run("reuse-rust-test-binary.sh", ["reviewed_case"], workspace.env);
    expect(result.status).toBe(64);
    expect(result.stderr).toContain("reviewed compiler inputs cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test.each([
    "--ignored",
    "--include-ignored",
    "--test-threads=24",
    "",
    "invalid filter",
    "../escape",
  ])("rejects unsafe cached-harness filter %s before executing any test", (filter) => {
    const workspace = currentHarness();
    const result = run("reuse-rust-test-binary.sh", [filter], workspace.env);

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("reviewed Rust test filters must be nonempty identifier selectors");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("validates every reviewed filter before the first cached harness executes", () => {
    const workspace = currentHarness();
    const result = run(
      "reuse-rust-test-binary.sh",
      ["first_reviewed_group", "--ignored"],
      workspace.env,
    );

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("reviewed Rust test filters must be nonempty identifier selectors");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("isolates heavyweight inherited stress corpora before cached test execution", () => {
    const workspace = currentHarness();
    const result = run("reuse-rust-test-binary.sh", ["reviewed::safe_case"], {
      ...workspace.env,
      SCRIPT_KIT_SEARCH_FULL_STRESS: "1",
      SCRIPT_KIT_STORAGE_FULL_STRESS: "1",
    });

    expect(result.status).toBe(0);
    expect(readFileSync(workspace.capture, "utf8")).toBe("reviewed::safe_case:1:0:0:0:0:0\n");
  });

  test("rejects a parent-traversing cached pool before finding an external harness", () => {
    const workspace = currentHarness();
    const externalDeps = join(workspace.root, "target-agent", "debug", "deps");
    mkdirSync(externalDeps, { recursive: true });
    const externalBinary = join(externalDeps, "script_kit_gpui-external-fixture");
    writeFileSync(
      externalBinary,
      '#!/bin/bash\nprintf "external-executed\\n" > "$CARGO_POLICY_CAPTURE"\n',
    );
    chmodSync(externalBinary, 0o755);

    const result = run("reuse-rust-test-binary.sh", ["reviewed_case"], {
      ...workspace.env,
      SCRIPT_KIT_CARGO_TARGET_POOL: "..",
    });

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("cached test pool must name one owned child");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("rejects a symlinked cached pool before running an external harness", () => {
    const workspace = currentHarness();
    const external = temporaryDirectory("script-kit-external-reuse-");
    const externalDeps = join(external, "debug", "deps");
    mkdirSync(externalDeps, { recursive: true });
    const externalBinary = join(externalDeps, "script_kit_gpui-external-fixture");
    writeFileSync(
      externalBinary,
      '#!/bin/bash\nprintf "external-executed\\n" > "$CARGO_POLICY_CAPTURE"\n',
    );
    chmodSync(externalBinary, 0o755);
    symlinkSync(external, join(workspace.root, "target-agent", "pools", "escaped"));

    const result = run("reuse-rust-test-binary.sh", ["reviewed_case"], {
      ...workspace.env,
      SCRIPT_KIT_CARGO_TARGET_POOL: "escaped",
    });

    expect(result.status).toBe(64);
    expect(result.stderr).toContain("cached test pool cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
  });
});

describe("isolated build process ownership", () => {
  function isolatedFixture(builder: string) {
    const workspace = fixture();
    installFakeGit(workspace);
    const localScripts = join(workspace.root, "scripts", "agentic");
    copyFileSync(join(scripts, "build-isolated-binary.sh"), join(localScripts, "build-isolated-binary.sh"));
    copyFileSync(join(scripts, "compiler-input-paths.txt"), join(localScripts, "compiler-input-paths.txt"));
    copyFileSync(join(scripts, "devtools-session-lib.sh"), join(localScripts, "devtools-session-lib.sh"));
    writeFileSync(join(localScripts, "agent-cargo.sh"), builder);
    chmodSync(join(localScripts, "agent-cargo.sh"), 0o755);
    writeFileSync(
      join(workspace.bin, "sleep"),
      '#!/bin/bash\nif [[ "$1" == "5" || "$1" == "2" ]]; then exec /bin/sleep 0.02; fi\nexec /bin/sleep "$@"\n',
    );
    chmodSync(join(workspace.bin, "sleep"), 0o755);
    const agentId = `policy-${process.pid}-${Math.random().toString(36).slice(2)}`;
    temporaryDirectories.push(`/tmp/sk-isolated-build-${agentId}.log`);
    return {
      ...workspace,
      builderPath: join(localScripts, "build-isolated-binary.sh"),
      env: { ...workspace.env, SCRIPT_KIT_AGENT_ID: agentId },
    };
  }

  test("returns a structured build failure when its owned Cargo child exits nonzero", () => {
    const workspace = isolatedFixture('#!/bin/bash\nprintf "synthetic-build-failure\\n" >&2\nexit 75\n');
    const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "5"], {
      env: workspace.env,
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode).toBe(31);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      tool: "build-isolated-binary",
      status: "error",
      phase: "build",
      error: { code: "build_failed" },
    });
  });

  test.each(["0", "-1", "1.5", "invalid"])(
    "rejects malformed isolated build timeout %s before any child starts",
    (timeout) => {
      const workspace = isolatedFixture(
        '#!/bin/bash\nprintf "started\\n" > "$CARGO_POLICY_CAPTURE"\nexit 0\n',
      );
      const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", timeout], {
        env: workspace.env,
        stdout: "pipe",
        stderr: "pipe",
      });

      expect(result.exitCode).toBe(64);
      expect(result.stderr.toString()).toContain("timeout must be a positive whole number");
      expect(existsSync(workspace.capture)).toBe(false);
    },
  );

  test.each([
    ["SCRIPT_KIT_CARGO_TARGET_POOL", ".."],
    ["SCRIPT_KIT_AGENT_ID", ".."],
    ["SCRIPT_KIT_DEVTOOLS_SESSION", ".."],
  ])("rejects parent-traversing isolated identity %s before any child starts", (setting, value) => {
    const workspace = isolatedFixture(
      '#!/bin/bash\nprintf "started\\n" > "$CARGO_POLICY_CAPTURE"\nexit 0\n',
    );
    const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "5"], {
      env: { ...workspace.env, [setting!]: value },
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode).toBe(64);
    expect(result.stderr.toString()).toContain("isolated build identity must name one owned child");
    expect(existsSync(workspace.capture)).toBe(false);
  });

  test("refuses a symlinked isolated runtime destination before any build starts", () => {
    const workspace = isolatedFixture(
      '#!/bin/bash\nprintf "started\\n" > "$CARGO_POLICY_CAPTURE"\nexit 0\n',
    );
    const external = temporaryDirectory("script-kit-external-runtime-");
    const runtimeRoot = join(workspace.root, "target-agent", "runtime");
    mkdirSync(runtimeRoot, { recursive: true });
    symlinkSync(external, join(runtimeRoot, "escaped"));

    const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "5"], {
      env: { ...workspace.env, SCRIPT_KIT_DEVTOOLS_SESSION: "escaped" },
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode).toBe(64);
    expect(result.stderr.toString()).toContain("isolated build ownership cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
    expect(existsSync(join(external, "script-kit-gpui"))).toBe(false);
  });

  test("refuses a symlinked build log without truncating its external target", () => {
    const workspace = isolatedFixture(
      '#!/bin/bash\nprintf "started\\n" > "$CARGO_POLICY_CAPTURE"\nexit 0\n',
    );
    const external = temporaryDirectory("script-kit-external-build-log-");
    const protectedFile = join(external, "preserved.txt");
    writeFileSync(protectedFile, "preserve-private-state\n");
    const logPath = `/tmp/sk-isolated-build-${workspace.env.SCRIPT_KIT_AGENT_ID}.log`;
    symlinkSync(protectedFile, logPath);

    const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "5"], {
      env: workspace.env,
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode).toBe(64);
    expect(result.stderr.toString()).toContain("isolated build ownership cannot follow a symlink");
    expect(existsSync(workspace.capture)).toBe(false);
    expect(readFileSync(protectedFile, "utf8")).toBe("preserve-private-state\n");
  });

  test("stages a successful owned fake build and returns its structured manifest", () => {
    const workspace = isolatedFixture(
      '#!/bin/bash\ndestination="$SCRIPT_KIT_REPO_ROOT/target-agent/pools/$SCRIPT_KIT_CARGO_TARGET_POOL/debug/script-kit-gpui"\nmkdir -p "$(dirname "$destination")"\nprintf "#!/bin/sh\\nexit 0\\n" > "$destination"\nchmod +x "$destination"\n',
    );

    const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "5"], {
      env: workspace.env,
      stdout: "pipe",
      stderr: "pipe",
    });

    expect(result.exitCode).toBe(0);
    const receipt = JSON.parse(result.stdout.toString());
    expect(receipt).toMatchObject({
      tool: "build-isolated-binary",
      status: "ok",
      phase: "stage",
      pool: "agent-debug",
    });
    expect(existsSync(join(workspace.root, receipt.binaryPath))).toBe(true);
    expect(existsSync(join(workspace.root, receipt.manifest))).toBe(true);
    const manifest = JSON.parse(readFileSync(join(workspace.root, receipt.manifest), "utf8"));
    const binary = readFileSync(join(workspace.root, receipt.binaryPath));
    expect(manifest).toMatchObject({
      schemaVersion: 2,
      binaryPath: receipt.binaryPath,
      binarySha256: createHash("sha256").update(binary).digest("hex"),
      sizeBytes: binary.byteLength,
      gitHead: "a".repeat(40),
      profile: "debug",
      requiresExactGitHead: false,
      rustDirty: false,
    });
    expect(manifest.compilerInputSha256).toMatch(/^[a-f0-9]{64}$/);
  });

  test("marks isolated builds from dirty compiler inputs without claiming source purity", () => {
    const workspace = isolatedFixture(
      '#!/bin/bash\ndestination="$SCRIPT_KIT_REPO_ROOT/target-agent/pools/$SCRIPT_KIT_CARGO_TARGET_POOL/debug/script-kit-gpui"\nmkdir -p "$(dirname "$destination")"\nprintf "#!/bin/sh\\nexit 0\\n" > "$destination"\nchmod +x "$destination"\n',
    );
    const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "5"], {
      env: { ...workspace.env, CARGO_POLICY_GIT_DIRTY: "1" },
      stdout: "pipe",
      stderr: "pipe",
    });
    expect(result.exitCode).toBe(0);
    const receipt = JSON.parse(result.stdout.toString());
    expect(JSON.parse(readFileSync(join(workspace.root, receipt.manifest), "utf8")).rustDirty)
      .toBe(true);
  });

  test("refuses to stage an isolated executable when HEAD changes during its owned build", () => {
    const workspace = isolatedFixture(
      '#!/bin/bash\nprintf "built\\n" > "$CARGO_POLICY_CAPTURE"\ndestination="$SCRIPT_KIT_REPO_ROOT/target-agent/pools/$SCRIPT_KIT_CARGO_TARGET_POOL/debug/script-kit-gpui"\nmkdir -p "$(dirname "$destination")"\nprintf "#!/bin/sh\\nexit 0\\n" > "$destination"\nchmod +x "$destination"\n',
    );
    const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "5"], {
      env: { ...workspace.env, CARGO_POLICY_GIT_CHANGE_AFTER_CARGO: "1" },
      stdout: "pipe",
      stderr: "pipe",
    });
    expect(result.exitCode).toBe(33);
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      phase: "stage",
      error: { code: "source_changed" },
    });
    expect(existsSync(join(
      workspace.root,
      "target-agent",
      "runtime",
      workspace.env.SCRIPT_KIT_AGENT_ID,
      "script-kit-gpui",
    ))).toBe(false);
  });

  test("terminates the entire owned compiler process group on timeout", () => {
    const workspace = isolatedFixture(
      '#!/bin/bash\nexec >/dev/null 2>&1\nprintf "%s\\n" "$$" > "$CARGO_POLICY_CAPTURE"\nexec /bin/sleep 30\n',
    );
    let descendant = 0;
    try {
      const result = Bun.spawnSync(["/bin/bash", workspace.builderPath, "--json", "1"], {
        env: workspace.env,
        stdout: "pipe",
        stderr: "pipe",
      });
      descendant = Number.parseInt(readFileSync(workspace.capture, "utf8"), 10);
      expect(result.exitCode).toBe(30);
      expect(JSON.parse(result.stdout.toString())).toMatchObject({
        status: "error",
        error: { code: "build_timeout" },
      });
      let alive = true;
      try {
        process.kill(descendant, 0);
      } catch {
        alive = false;
      }
      expect(alive).toBe(false);
    } finally {
      if (!descendant && existsSync(workspace.capture)) {
        descendant = Number.parseInt(readFileSync(workspace.capture, "utf8"), 10);
      }
      if (descendant > 0) {
        try { process.kill(descendant, "SIGKILL"); } catch {}
      }
    }
  });
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
      '#!/bin/bash\nprintf \'{"status":"ok","binaryPath":"target-agent/runtime/synthetic/script-kit-gpui"}\\n\'\n',
    );
    writeFileSync(join(workspace.bin, "python3"), '#!/bin/bash\nprintf "target-agent/runtime/synthetic/script-kit-gpui\\n"\n');
    for (const executable of [
      "preflight-isolated.sh",
      "wait-session-ready.sh",
      "session.sh",
      "build-isolated-binary.sh",
    ]) {
      chmodSync(join(localScripts, executable), 0o755);
    }
    chmodSync(join(workspace.bin, "python3"), 0o755);

    return {
      ...workspace,
      localScripts,
      env: {
        ...workspace.env,
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
    const result = Bun.spawnSync(
      ["/bin/bash", join(workspace.localScripts, "devtools-session.sh"), "start", "--session", "reviewed-session", "--mode", "isolated", "--build", "always"],
      {
        env: { ...workspace.env, SESSION_FAKE_SECOND_PREFLIGHT_STATUS: exitCode! },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    expect(result.exitCode).toBe(Number(exitCode));
    expect(JSON.parse(result.stdout.toString())).toMatchObject({
      status: "error",
      phase: "preflight",
      error: { code: failureCode },
    });
    expect(existsSync(workspace.capture)).toBe(false);
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
