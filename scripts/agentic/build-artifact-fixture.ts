import { chmodSync, lstatSync, mkdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { artifactHash, assertSourceBoundary, compilerCompatibility, observeArtifactSource, observeToolchain, publishImmutableArtifact } from "./build-artifact.ts";
import type { ArtifactKind, ArtifactReference } from "./build-artifact.ts";
import { beginManagedTask, cacheLease, canonicalJson, claimOutput, emptyOwnedCleanup, finalizeManagedTask, listManagedTasks, removeFinalizedManagedTask, updateManagedTask, validateOutputTarget, withManagedMetadata } from "./artifact-lifecycle.ts";
import type { ManagedTask } from "./artifact-lifecycle.ts";

export interface ArtifactFixture { reference: ArtifactReference; executablePath: string; publicationDirectory: string; task: ManagedTask; dispose(): void; }
/** Test fixture only: publishes through the real lease/publisher/task contracts.
 * A fresh temporary repository is initialized; existingRepository reuses source without mutating it. */
export function createArtifactFixture(directory: string, options: { kind?: ArtifactKind; executable?: string; existingRepository?: boolean; features?: readonly string[] } = {}): ArtifactFixture {
  const root = realpathSync(directory);
  const kind = options.kind ?? "application";
  const name = kind === "application" ? "script-kit-gpui" : kind === "rust-libtest" ? "script_kit_gpui" : "export_design_tokens";
  if (!options.existingRepository) {
  writeFileSync(join(root, ".gitignore"), "target-agent/\n.test-output/\n");
  mkdirSync(join(root, "scripts/agentic"), { recursive: true });
  mkdirSync(join(root, "src/bin"), { recursive: true });
  writeFileSync(join(root, "scripts/agentic/compiler-input-paths.txt"), "src\nCargo.toml\nCargo.lock\nrust-toolchain.toml\n");
  writeFileSync(join(root, "Cargo.toml"), '[package]\nname="script-kit-gpui"\nversion="0.0.0"\nedition="2021"\n');
  writeFileSync(join(root, "Cargo.lock"), 'version = 4\n');
  writeFileSync(join(root, "rust-toolchain.toml"), '[toolchain]\nchannel="1.98.0"\nprofile="minimal"\ncomponents=["rustfmt","clippy","rust-analyzer"]\n');
  writeFileSync(join(root, "src/main.rs"), "fn main() {}\n");
  writeFileSync(join(root, "src/lib.rs"), "pub fn fixture() {}\n");
  writeFileSync(join(root, "src/bin/export_design_tokens.rs"), "fn main() {}\n");
  for (const args of [["init", "-q"], ["add", "."], ["-c", "user.name=Fixture", "-c", "user.email=fixture@invalid", "commit", "-qm", "fixture"]]) {
    const out = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    if (out.status !== 0) throw new Error(out.stderr);
  }
  }
  const source = observeArtifactSource(root);
  const { rustcPath: _rustcPath, cargoPath: _cargoPath, host, ...toolchain } = observeToolchain(root);
  const requestedPolicy = { fixture: true };
  const effectiveConfiguration = { requestedPolicy, compatibility: compilerCompatibility(root) };
  const id = `fixture-${randomUUID()}`;
  const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/managed-tasks", id), kind: "directory", probeId: "artifact-fixture" }), id);
  const task = beginManagedTask(claim, "build-job", []);
  updateManagedTask(task, { state: "running", source, effectiveConfiguration });
  const generation = randomUUID(), lock = join(root, "target-agent/.locks/pool-agent-debug.lock");
  cacheLease("acquire", lock, [String(process.pid), generation, "1000"]);
  let reference: ArtifactReference;
  try {
    const pool = join(root, "target-agent/pools/agent-debug/debug", `.fixture-${id}`);
    mkdirSync(pool, { recursive: true });
    const executable = join(pool, name);
    writeFileSync(executable, options.executable ?? "#!/bin/sh\nprintf 'fixture\\n'\n", { mode: 0o700 });
    reference = publishImmutableArtifact(root, task, executable, {
      artifactKind: kind, target: { packageId: `path+file://${root}#script-kit-gpui@0.0.0`, packageName: "script-kit-gpui", targetName: name,
        targetKind: [kind === "rust-libtest" ? "lib" : "bin"], crateTypes: [kind === "rust-libtest" ? "lib" : "bin"], sourcePath: kind === "rust-libtest" ? "src/lib.rs" : kind === "tool" ? "src/bin/export_design_tokens.rs" : "src/main.rs",
        features: [...(options.features ?? [])], cargoProfile: { test: kind === "rust-libtest" }, requestedProfile: kind === "rust-libtest" ? "test" : "dev", targetTriple: host },
      source, toolchain, requestedPolicySha256: artifactHash(canonicalJson(requestedPolicy)), effectiveConfiguration,
      effectiveConfigurationSha256: artifactHash(canonicalJson(effectiveConfiguration)), requiresExactGitHead: false,
      publication: { owner: "scripts/agentic/agent-cargo.sh", pool: "agent-debug", leaseGeneration: generation, buildTask: task.identity, immutable: true, exportedWhileLeaseHeld: true },
    }, () => assertSourceBoundary(source, observeArtifactSource(root)));
    updateManagedTask(task, { result: { status: "succeeded", artifacts: [reference] } });
    rmSync(pool, { recursive: true });
  } finally { cacheLease("release", lock, [String(process.pid), generation]); }
  finalizeManagedTask(task, emptyOwnedCleanup());
  const publicationDirectory = dirname(join(root, reference.manifestPath));
  const identity = lstatSync(publicationDirectory);
  return { reference, executablePath: join(publicationDirectory, name), publicationDirectory, task,
    dispose() {
      withManagedMetadata(root, () => {
        if (listManagedTasks(root).some(entry => !entry.record || (entry.record.state !== "closed" && entry.record.artifactReferences.some(ref => canonicalJson(ref) === canonicalJson(reference))))) throw new Error("fixture_artifact_still_referenced");
        const current = lstatSync(publicationDirectory);
        if (current.dev !== identity.dev || current.ino !== identity.ino || current.isSymbolicLink()) throw new Error("fixture_publication_identity_changed");
        chmodSync(publicationDirectory, 0o700);
        rmSync(publicationDirectory, { recursive: true });
      });
      removeFinalizedManagedTask(task);
    },
  };
}

export interface BuildWorkspaceFixture { root: string; env: Record<string, string>; invocations: string; }
export function createBuildWorkspace(directory: string): BuildWorkspaceFixture {
  const root = realpathSync(directory), bin = join(root, "fixture-bin"), invocations = join(root, "cargo-invocations.jsonl");
  mkdirSync(bin, { recursive: true });
  mkdirSync(join(root, "src/bin"), { recursive: true });
  mkdirSync(join(root, "scripts/agentic"), { recursive: true });
  writeFileSync(join(root, "scripts/agentic/compiler-input-paths.txt"), "src\nCargo.toml\nCargo.lock\nrust-toolchain.toml\n");
  writeFileSync(join(root, ".gitignore"), "target-agent/\n.test-output/\nfixture-bin/\ncargo-invocations.jsonl\n");
  writeFileSync(join(root, "Cargo.toml"), '[package]\nname="script-kit-gpui"\nversion="0.0.0"\nedition="2021"\n');
  writeFileSync(join(root, "Cargo.lock"), "version = 4\n");
  writeFileSync(join(root, "rust-toolchain.toml"), '[toolchain]\nchannel="1.98.0"\nprofile="minimal"\ncomponents=["rustfmt","clippy","rust-analyzer"]\n');
  for (const name of ["main.rs", "lib.rs", "bin/export_design_tokens.rs"]) writeFileSync(join(root, "src", name), "// fixture input\n");
  for (const args of [["init", "-q"], ["add", "."], ["-c", "user.name=Fixture", "-c", "user.email=fixture@invalid", "commit", "-qm", "fixture"]]) {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    if (result.status !== 0) throw new Error(result.stderr);
  }
  writeFileSync(join(bin, "rustup"), `#!/bin/sh\n[ "$1" = which ] || exit 64\nprintf '%s/%s\\n' '${bin}' "$4"\n`, { mode: 0o700 });
  writeFileSync(join(bin, "rustc"), "#!/bin/sh\nif [ \"$1\" = --print ] && [ \"$2\" = cfg ]; then printf 'target_arch=\"aarch64\"\\ntarget_os=\"macos\"\\ntarget_family=\"unix\"\\nunix\\n'; else printf 'rustc 1.98.0 (fixture)\\nrelease: 1.98.0\\nhost: aarch64-apple-darwin\\n'; fi\n", { mode: 0o700 });
  writeFileSync(join(bin, "cargo"), `#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, time
args = sys.argv[1:]
if args == ['-V']:
    print('cargo 1.98.0 (fixture)'); sys.exit(0)
root = pathlib.Path(os.environ['SCRIPT_KIT_REPO_ROOT'])
with open(root / 'cargo-invocations.jsonl', 'a') as file:
    file.write(json.dumps({'args':args, 'generation':os.environ.get('SCRIPT_KIT_PROVENANCE_GENERATION'), 'jobs':os.environ.get('CARGO_BUILD_JOBS'), 'environment':{name:os.environ.get(name) for name in ['CARGO_INCREMENTAL','CARGO_BUILD_INCREMENTAL','CARGO_PROFILE_DEV_INCREMENTAL','CARGO_PROFILE_TEST_INCREMENTAL','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC','RUSTC_WRAPPER']}}) + '\\n')
allocation = int(os.environ.get('FAKE_CARGO_ALLOCATE_BYTES', '0'))
if allocation:
    if allocation < 0 or allocation > 8 * 1024 * 1024: sys.exit(64)
    output = pathlib.Path(os.environ['CARGO_TARGET_DIR']) / 'fixture-allocation.bin'
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(b'x' * allocation)
if os.environ.get('FAKE_CARGO_HANDSHAKE') == '1':
    (root / 'cargo-ready').write_text('ready')
    deadline = time.monotonic() + 15
    while not (root / 'cargo-continue').exists() and time.monotonic() < deadline: time.sleep(0.02)
    if not (root / 'cargo-continue').exists(): sys.exit(75)
if os.environ.get('FAKE_CARGO_HANG') == '1':
    peer = subprocess.Popen(['sleep','60'])
    (root / 'descendant.pid').write_text(str(peer.pid))
    time.sleep(60)
if os.environ.get('FAKE_CARGO_MUTATE'):
    path = root / 'src/main.rs'; original = path.read_bytes(); path.write_bytes(original + b'// changed\\n')
    if os.environ.get('FAKE_CARGO_RESTORE') == '1': path.write_bytes(original)
if os.environ.get('FAKE_CARGO_FAIL') == '1': sys.exit(1)
if args[0] == 'test' and '--no-run' not in args:
    print('running 1 test')
    print('test fixture::passes ... ok')
    print('test result: ok. ' + ('0' if os.environ.get('FAKE_CARGO_ZERO') else '1') + ' passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s')
if '--message-format=json-render-diagnostics' in args:
    lib = args[0] == 'test' and '--lib' in args
    name = 'script_kit_gpui' if lib else (args[args.index('--bin')+1] if '--bin' in args else 'script-kit-gpui')
    target = pathlib.Path(os.environ['CARGO_TARGET_DIR']) / ('release' if '--release' in args else 'debug')
    target.mkdir(parents=True, exist_ok=True)
    executable = target / name
    generation = os.environ.get('SCRIPT_KIT_PROVENANCE_GENERATION', '')
    stamp = target / (name + '.generation')
    fresh = executable.exists() and stamp.exists() and stamp.read_text() == generation
    if os.environ.get('FAKE_CARGO_WARM') == '1' and not generation: fresh = True
    if os.environ.get('FAKE_CARGO_FRESH_ALWAYS') == '1': fresh = True
    if not executable.exists() or not fresh:
        if lib:
            executable.write_text('#!/bin/sh\\ncount=1\\ncase "$1" in ""|--*) ;; *) case fixture::passes in *"$1"*) ;; *) count=0;; esac;; esac\\nprintf "test result: ok. %s passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\\\\n" "$count"\\n')
        else:
            executable.write_text('#!/bin/sh\\nprintf fixture\\n')
        executable.chmod(0o700); stamp.write_text(generation)
    source = 'src/lib.rs' if lib else ('src/main.rs' if name == 'script-kit-gpui' else 'src/bin/export_design_tokens.rs')
    message = {'reason':'compiler-artifact','package_id':'path+' + root.as_uri() + '#script-kit-gpui@0.0.0',
      'target':{'name':name,'kind':['lib' if lib else 'bin'],'crate_types':['lib' if lib else 'bin'],'src_path':str(root/source)},
      'features':args[args.index('--features')+1].split(',') if '--features' in args else [], 'profile':{'test':lib},'executable':str(executable),'fresh':fresh}
    print(json.dumps(message))
    if os.environ.get('FAKE_CARGO_AMBIGUOUS') == '1': print(json.dumps(message))
`, { mode: 0o700 });
  const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
  for (const key of Object.keys(env)) if (/^(CARGO_PROFILE_|CARGO_TARGET_.*_RUSTFLAGS$|FAKE_CARGO_)/.test(key) || ["RUSTC", "RUSTUP_TOOLCHAIN", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR", "CARGO_RESOLVER_LOCKFILE_PATH", "CARGO_BUILD_TARGET", "CARGO_BUILD_JOBS", "RUST_TEST_THREADS", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTDOCFLAGS", "SCRIPT_KIT_METAL_MODULE_CACHE_DIR", "CLANG_MODULE_CACHE_PATH", "SCRIPT_KIT_AGENT_ALLOW_LOW_DISK", "SCRIPT_KIT_AGENT_POOL_BUDGET_GB"].includes(key)) delete env[key];
  delete env.SCRIPT_KIT_AGENT_TARGET_BUDGET_GB;
  const cargoHome = join(bin, "cargo-home");
  mkdirSync(cargoHome);
  Object.assign(env, { PATH: `${bin}:${dirname(process.execPath)}:/usr/bin:/bin`, CARGO_HOME: cargoHome, SCRIPT_KIT_REPO_ROOT: root, SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_AGENT_USE_SCCACHE: "0", SCRIPT_KIT_AGENT_MIN_FREE_GB: "0", CMAKE_BUILD_PARALLEL_LEVEL: "1" });
  return { root, env, invocations };
}
