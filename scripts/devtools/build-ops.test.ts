import { afterEach, expect, spyOn, test } from "bun:test";
import { chmodSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, utimesSync, writeFileSync } from "node:fs";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createArtifactFixture, createBuildWorkspace } from "../agentic/build-artifact-fixture.ts";
import { BUILD_ACTIONS, BuildOperationError, executeBuildAction, routeChanged, runBuildOps } from "./build-ops.ts";
import type { BuildOperationResult, ChangedRoute } from "./build-ops.ts";
import { buildDependencies, buildStorage } from "./lib/build-ops-inventory.ts";
import * as inventory from "./lib/build-ops-inventory.ts";
import { artifactHash, assertSourceBoundary, bundleTreeHash, canonicalArtifactPath, observeArtifactSource, unsignedMachOPayloadSha256, verifyImmutableArtifact } from "../agentic/build-artifact.ts";
import { assertOutputOwnership, beginManagedTask, cacheLease, claimOutput, emptyOwnedCleanup, finalizeManagedTask, managedKeepSet, managedRetentionPlan, managedTaskRecordPath, pruneManagedRecords, readManagedTask, updateManagedTask, validateOutputTarget } from "../agentic/artifact-lifecycle.ts";
import type { ArtifactKind, ArtifactReference } from "../agentic/build-artifact.ts";
import * as ownedProcess from "../agentic/owned-process.ts";
import * as lifecycle from "../agentic/artifact-lifecycle.ts";
import type { BuildWorkspaceFixture } from "../agentic/build-artifact-fixture.ts";
import type { OwnedCleanup, TaskIdentity } from "../agentic/artifact-lifecycle.ts";
import type { OwnedProcess } from "../agentic/owned-process.ts";

const roots: string[] = [];
function fixture() {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "build-ops-behavior-")));
  roots.push(root); return createBuildWorkspace(root);
}
afterEach(() => {
  const writable = (path: string): void => { const stat = lstatSync(path); if (stat.isDirectory() && !stat.isSymbolicLink()) { chmodSync(path, 0o700); for (const child of readdirSync(path)) writable(join(path, child)); } };
  for (const root of roots.splice(0)) { writable(root); rmSync(root, { recursive: true }); }
});
function build(root: string, env: Record<string, string>, kind: ArtifactKind, extra: Record<string, string> = {}) {
  const args = kind === "rust-libtest" ? ["test", "--locked", "--lib", "--no-run"] : ["build", "--locked", "--bin", kind === "application" ? "script-kit-gpui" : "export_design_tokens"];
  const out = Bun.spawnSync(["bash", resolve(import.meta.dir, "../agentic/agent-cargo.sh"), ...args], { cwd: root, env: { ...env, SCRIPT_KIT_AGENT_ARTIFACT_KIND: kind, ...extra }, timeout: 25_000 });
  return { status: out.exitCode, stderr: out.stderr.toString(), result: out.stdout.length ? JSON.parse(out.stdout.toString()) : null };
}
const expectation = (kind: ArtifactKind) => ({ kind, packageName: "script-kit-gpui", targetName: kind === "application" ? "script-kit-gpui" : kind === "tool" ? "export_design_tokens" : "script_kit_gpui", sourcePolicy: "recorded-content" as const });

test("passive inventory does not invoke tools or create managed outputs", () => {
  const f = fixture(); const before = readdirSync(f.root).sort();
  expect(buildDependencies(f.root).startsSidecars).toBe(false);
  expect(buildStorage(f.root, false).uniquePhysicalBytes).toBeNull();
  expect(readdirSync(f.root).sort()).toEqual(before);
  expect(existsSync(f.invocations)).toBe(false);
  expect(existsSync(join(f.root, "target-agent"))).toBe(false);
});
test("discover CLI preserves clipboard action argv without running tools or writing outputs", () => {
  const f = fixture(), before = readdirSync(f.root).sort();
  const out = Bun.spawnSync([process.execPath, resolve(import.meta.dir, "build-ops.ts"), "discover"], { cwd: f.root, env: f.env, timeout: 25_000 });
  expect(out.exitCode).toBe(0);
  const receipt = JSON.parse(out.stdout.toString());
  expect(receipt.pass).toBe(true);
  expect(receipt.buildOps.result.actions["clipboard-test"]).toEqual(["test", "--locked", "-p", "sk-clipboard"]);
  expect(readdirSync(f.root).sort()).toEqual(before);
  expect(existsSync(f.invocations)).toBe(false);
  expect(existsSync(join(f.root, "target-agent"))).toBe(false);
  expect(existsSync(join(f.root, ".test-output"))).toBe(false);
});
test("storage query classifies diagnostic containers without hiding accounting or leaking text", async () => {
  const f = fixture(), before = readdirSync(f.root).sort();
  const storage = buildStorage(f.root, false);
  const messages = ["permission denied while reading private toolchain", "volume query failed for private storage", "nested allocation failed", "nested scanner detail"];
  const observation = {
    ...storage,
    external: [{
      category: "toolchains", path: join(f.root, "external-toolchains"), deletionScope: "never",
      present: true, complete: false, logicalBytes: 17, allocatedBytes: 4096, entries: 3, linksNotFollowed: 1,
      errors: [messages[0]!],
      details: { error: { message: messages[2]!, attempts: 2, bytes: 512, nested: [{ errors: [messages[3]!], retryable: false, next: null }] } },
    }],
    volume: { complete: false, availableBytes: null, reason: messages[1]! },
  };
  const query = spyOn(inventory, "buildStorage").mockReturnValue(observation);
  const output = spyOn(console, "log").mockImplementation(() => {});
  const previousRoot = process.env.SCRIPT_KIT_REPO_ROOT, previousExitCode = process.exitCode;
  try {
    process.env.SCRIPT_KIT_REPO_ROOT = f.root;
    await runBuildOps(["query", "storage"]);
    const serialized = String(output.mock.calls.at(-1)?.[0]);
    const receipt = JSON.parse(serialized), result = receipt.buildOps.result, external = result.external[0];
    expect(receipt.pass).toBe(true);
    expect(receipt.privacy.unclassifiedSensitivePaths).toEqual([]);
    expect(receipt.privacy.rawContentReturned).toBe(false);
    expect(result.categories).toEqual(storage.categories);
    expect(result.agentPoolAllocatedBytes).toBe(storage.agentPoolAllocatedBytes);
    expect(result.uniquePhysicalBytes).toBeNull();
    expect(result.reclaimablePhysicalBytes).toBeNull();
    expect(external).toMatchObject({ category: "toolchains", deletionScope: "never", present: true, complete: false, logicalBytes: 17, allocatedBytes: 4096, entries: 3, linksNotFollowed: 1 });
    expect(external.errors).toHaveLength(1);
    expect(external.errors[0]).toMatchObject({ redacted: true, contentKind: "Diagnostic" });
    expect(result.volume).toMatchObject({ complete: false, availableBytes: null, reason: { redacted: true, contentKind: "Diagnostic" } });
    expect(external.details.error).toMatchObject({ message: { redacted: true, contentKind: "Diagnostic" }, attempts: 2, bytes: 512 });
    expect(external.details.error.nested).toEqual([{ errors: [expect.objectContaining({ redacted: true, contentKind: "Diagnostic" })], retryable: false, next: null }]);
    for (const message of messages) expect(serialized).not.toContain(message);
    expect(serialized).not.toContain(f.root);
    expect(readdirSync(f.root).sort()).toEqual(before);
    expect(existsSync(f.invocations)).toBe(false);
  } finally {
    query.mockRestore(); output.mockRestore(); process.exitCode = previousExitCode ?? 0;
    if (previousRoot === undefined) delete process.env.SCRIPT_KIT_REPO_ROOT; else process.env.SCRIPT_KIT_REPO_ROOT = previousRoot;
  }
});
test("domain-only route invokes no application or native target", () => {
  const route = routeChanged(["crates/sk-protocol/src/lib.rs"]);
  expect(route.domainOnly).toBe(true);
  expect(route.steps.map(step => step.args)).toEqual([
    ["check", "--locked", "-p", "sk-protocol"],
    ["clippy", "--locked", "-p", "sk-protocol", "--all-targets", "--", "-D", "warnings"],
    ["test", "--locked", "-p", "sk-protocol"],
  ]);
  expect(routeChanged(["crates/sk-protocol/src/lib.rs", "src/main.rs"]).domainOnly).toBe(false);
});

test("changed routing skips Rust only for reviewed documentation and evidence", () => {
  for (const path of ["README.md", "./README.md", "scripts/devtools/README.md", "GLOSSARY.md", "POLISH.md", "VISION.md", "FEATURES.md", "docs/guide.md", ".notes", ".notes/review.json", ".test-output/run/receipt.json"]) {
    const route = routeChanged([path]);
    expect(route).toMatchObject({ rustRequired: false, domainOnly: false, scope: "noncompiler-only", selectedPackages: [], steps: [], documentationPathCount: 1 });
    expect(route.reasons).toContain("reviewed_documentation_or_evidence");
    expect(route.coverageGaps).toContain("documentation_and_evidence_contents_not_verified");
  }
});

test("changed routing gives compiler owners priority over documentation extensions", () => {
  for (const path of ["src/help.md", "assets/guide.md", "vendor/lib/README.md", "kit-init/README.md", "tests/theme/snapshots/example.md", "Cargo.toml", "Cargo.lock", "build.rs", ".cargo/config.toml", "rust-toolchain.toml", "scripts/kit-sdk.ts", "scripts/mcp-cli.ts", "scripts/examples/menu-syntax/README.md", "scripts/agentic/compiler-input-paths.txt", "tests/theme"]) {
    const route = routeChanged([path]);
    expect(route.rustRequired).toBe(true);
    expect(route.steps.filter(step => step.executor === "cargo").map(step => step.action)).toEqual(["app-check", "app-clippy", "lib-test", "integration-test", "domain-test"]);
  }
  expect(routeChanged(["crates/sk-storage/README.md"]).selectedPackages).toEqual(["sk-storage"]);
});

test("changed routing reads the current compiler inventory rather than a duplicate root classifier", () => {
  const f = fixture(), owner = join(f.root, "scripts/agentic/compiler-input-paths.txt");
  writeFileSync(owner, `${readFileSync(owner, "utf8")}README.md\nscripts/devtools/README.md\ndocs/embedded\n`);
  for (const path of ["README.md", "scripts/devtools/README.md", "docs/embedded/help.md"]) {
    const route = routeChanged([path], false, f.root);
    expect(route.rustRequired).toBe(true);
    expect(route.reasons).toContain("reviewed_compiler_inputs");
  }
  writeFileSync(owner, "src\n../outside\n");
  expect(() => routeChanged(["README.md"], false, f.root)).toThrow("invalid_compiler_input_inventory");
  expect(existsSync(f.invocations)).toBe(false);
});

test("changed routing retains exact reviewed TypeScript Python and shell behavior contracts", () => {
  for (const [owner, contract] of [
    ["scripts/devtools/build-ops.ts", "scripts/devtools/build-ops.test.ts"],
    ["scripts/agentic/session-supervisor.py", "scripts/agentic/owned-process.test.ts"],
    ["scripts/agentic/session.sh", "scripts/agentic/session-stop-ownership.test.ts"],
    ["scripts/agent-check.sh", "scripts/devtools/build-ops.test.ts"],
    ["scripts/devtools/lib/receipt-artifact.ts", "scripts/devtools/receipt-artifact.test.ts"],
    ["scripts/devtools/lib/runtime-coverage.ts", "scripts/devtools/runtime-coverage.test.ts"],
    ["scripts/devtools/lib/owned-evaluation.ts", "scripts/devtools/owned-evaluation.test.ts"],
    ["scripts/agentic/launcher-selection-stability-probe.ts", "scripts/agentic/launcher-selection-stability-probe.test.ts"],
    ["scripts/agentic/launcher-search-contract.ts", "scripts/agentic/launcher-selection-stability-probe.test.ts"],
    ["scripts/agentic/launcher-search-recipes.ts", "scripts/agentic/launcher-selection-stability-probe.test.ts"],
    ["scripts/devtools/design.ts", "scripts/agentic/launcher-selection-stability-probe.test.ts"],
    ["scripts/agentic/launcher-search-receipt.ts", "scripts/agentic/launcher-search-receipt.test.ts"],
    ["scripts/devtools/design.ts", "scripts/agentic/launcher-search-receipt.test.ts"],
    ["scripts/devtools/consistency.ts", "scripts/devtools/consistency.test.ts"],
    ["scripts/devtools/compare.ts", "scripts/devtools/compare.test.ts"],
    ["scripts/devtools/image-diff.ts", "scripts/devtools/image-diff.test.ts"],
    ["scripts/release-evidence.ts", "scripts/release-evidence.test.ts"],
    ["scripts/verify.sh", "scripts/release-evidence.test.ts"],
  ] as const) {
    for (const quick of [false, true]) {
      const route = routeChanged([owner, "README.md"], quick);
      expect(route.rustRequired).toBe(false);
      expect(route.steps.every(step => step.executor === "bun")).toBe(true);
      expect(route.steps.some(step => step.args.includes(`./${contract}`))).toBe(true);
      expect(route.reasons).toContain("reviewed_noncompiler_contracts");
    }
  }
  const route = routeChanged(["scripts/devtools/build-ops.ts", "scripts/devtools/build-ops.test.ts"]);
  expect(route.steps).toHaveLength(1);
});

test("changed routing covers mixed migrated receipt owners without Rust or unknown contract gaps", () => {
  const receiptOwners = [
    "scripts/devtools/lib/receipt-artifact.ts", "scripts/devtools/design.ts", "scripts/devtools/stories.ts",
    "scripts/devtools/lib/receipt-schema.ts", "scripts/devtools/lib/fixture-contract.ts", "scripts/devtools/lib/story-contract.ts",
    "scripts/devtools/lib/runtime-coverage.ts", "scripts/devtools/consistency.ts", "scripts/devtools/compare.ts",
    "scripts/devtools/image-diff.ts", "scripts/devtools/generated-byte-compare.ts", "scripts/release-evidence.ts", "scripts/verify.sh",
  ];
  for (const owner of receiptOwners) {
    expect(routeChanged([owner]).steps.some(step => step.args.includes("./scripts/devtools/receipt-artifact.test.ts"))).toBe(true);
  }
  const route = routeChanged([
    ...receiptOwners, "scripts/devtools/build-ops.ts", "scripts/devtools/build-ops.test.ts",
    "scripts/agentic/artifact-lifecycle.ts", "scripts/agentic/owned-process.ts", "scripts/agentic/session-supervisor.py", "scripts/agentic/session.sh",
  ]);
  expect(route).toMatchObject({ rustRequired: false, scope: "noncompiler-only", unknownPathCount: 0, unreviewedPathIndices: [], coverageGaps: [] });
  expect(route.steps.length).toBeGreaterThan(0);
  expect(route.steps.every(step => step.executor === "bun")).toBe(true);
  expect(route.steps.filter(step => step.args.includes("./scripts/devtools/receipt-artifact.test.ts"))).toHaveLength(1);
});

test("changed routing keeps empty unknown and mixed scopes conservative without guessing filters", () => {
  for (const paths of [[], ["unreviewed.md"], ["scripts/unreviewed.ts"], ["crates/sk-protocol-extra/src/lib.rs"], ["crates/sk-protocol/src/lib.rs", "README.md"], ["src/main.rs", ".notes/report.md"]]) {
    const route = routeChanged(paths);
    expect(route.steps.filter(step => step.executor === "cargo").map(step => step.action)).toEqual(["app-check", "app-clippy", "lib-test", "integration-test", "domain-test"]);
    expect(route.filterGuessing).toBe(false);
  }
  const unknown = routeChanged(["scripts/unreviewed.ts"]);
  expect(unknown.unknownPathCount).toBe(1);
  expect(unknown.coverageGaps).toContain("unknown_paths_have_no_reviewed_noncompiler_contract");
  expect(routeChanged(["README.md", "scripts/unreviewed.ts", "scripts/devtools/build-ops.ts", "other.py"]).unreviewedPathIndices).toEqual([1, 3]);
  const mixed = routeChanged(["src/main.rs", "scripts/devtools/build-ops.ts"], true);
  expect(mixed.steps).toEqual([
    { action: "app-check", executor: "cargo", args: BUILD_ACTIONS["app-check"] },
    { action: "contract-test", executor: "bun", args: ["test", "--timeout", "60000", "./scripts/devtools/build-ops.test.ts"] },
  ]);
  expect(mixed.coverageGaps).toContain("quick_omits_rust_clippy_and_tests");
  expect(routeChanged(["crates/sk-storage", "crates/sk-protocol/src/lib.rs"], true).steps).toEqual([
    { action: "domain-check", executor: "cargo", args: ["check", "--locked", "-p", "sk-protocol", "-p", "sk-storage"] },
  ]);
});

test("changed routing rejects noncanonical and control-bearing paths", () => {
  for (const path of ["", ".", "./", "././README.md", "/README.md", "../README.md", "docs/../README.md", "docs//README.md", "docs/./README.md", "docs/", "C:\\README.md", "docs\\README.md", "README.md\0", "README.md\n"]) {
    expect(() => routeChanged([path])).toThrow("invalid_changed_path");
  }
});
for (const kind of ["application", "rust-libtest", "tool"] as const) test(`publishes independently verifiable ${kind} Cargo output`, () => {
  const f = fixture(); const outcome = build(f.root, f.env, kind);
  expect(outcome.status).toBe(0);
  const reference: ArtifactReference = outcome.result.artifacts[0];
  const artifact = verifyImmutableArtifact(f.root, reference, expectation(kind));
  expect(artifact.manifest.artifactKind).toBe(kind);
  expect(artifact.manifest.publication.exportedWhileLeaseHeld).toBe(true);
  expect(readManagedTask(outcome.result.recordPath, outcome.result.task).state).toBe("closed");
  expect(existsSync(join(f.root, "target-agent/.locks/pool-agent-debug.lock"))).toBe(false);
});
for (const alias of ["trailing-slash", "symlink"] as const) test(`canonical wrapper ingress preserves ${alias} repository identity`, () => {
  const f = fixture();
  const root = alias === "trailing-slash" ? `${f.root}/` : join(f.root, "root-alias");
  if (alias === "symlink") symlinkSync(f.root, root);
  const outcome = build(f.root, f.env, "application", { SCRIPT_KIT_REPO_ROOT: root });
  expect(outcome.status).toBe(0);
  expect(outcome.result.cleanup).toMatchObject({ closed: true, referencesFinalized: true, failureCodes: [] });
  expect(verifyImmutableArtifact(f.root, outcome.result.artifacts[0], expectation("application")).manifest.artifactKind).toBe("application");
  expect(existsSync(join(f.root, "target-agent/.locks/pool-agent-debug.lock"))).toBe(false);
});
for (const mode of ["invalid-path", "failed-release", "foreign-generation", "stale-start", "signed-publication"] as const) test(`pre-task lease admission preserves ownership and cleanup: ${mode}`, () => {
  const f = fixture(), lock = join(f.root, "target-agent/.locks/pool-agent-debug.lock");
  // Real isolated leases exercise the handoff; no compiler or application runs.
  const script = `
    import { readFileSync, writeFileSync } from "node:fs";
    import { join } from "node:path";
    import { cacheLease } from ${JSON.stringify(resolve(import.meta.dir, "../agentic/artifact-lifecycle.ts"))};
    import { runWrapperCargo } from ${JSON.stringify(resolve(import.meta.dir, "../agentic/build-artifact.ts"))};
    const root = process.env.SCRIPT_KIT_REPO_ROOT;
    const lock = join(root, "target-agent/.locks/pool-agent-debug.lock");
    const generation = "owned-admission-fixture";
    const mode = ${JSON.stringify(mode)};
    cacheLease("acquire", lock, [String(process.pid), generation, "1000"]);
    if (mode === "failed-release") writeFileSync(join(lock, "sentinel"), "preserve unknown lease evidence");
    if (mode === "stale-start") {
      const lease = JSON.parse(readFileSync(join(lock, "lease.json"), "utf8"));
      lease.processStartTime = "Thu Jan  1 00:00:00 1970";
      writeFileSync(join(lock, "lease.json"), JSON.stringify(lease));
    }
    writeFileSync(join(root, "lease-before.json"), readFileSync(join(lock, "lease.json")));
    process.env.SCRIPT_KIT_AGENT_LEASE_GENERATION = mode === "foreign-generation" ? "foreign-generation" : generation;
    process.env.SCRIPT_KIT_AGENT_LEASE_PATH = mode === "signed-publication" ? lock : root + "//target-agent/.locks/pool-agent-debug.lock";
    process.exitCode = await runWrapperCargo(mode === "signed-publication" ? ["publish-signed-bundle"] : ["check", "--lib"]);
  `;
  const out = Bun.spawnSync([process.execPath, "-e", script], { cwd: f.root, env: f.env, timeout: 25_000 });
  const result = JSON.parse(out.stdout.toString());
  expect(out.exitCode).not.toBe(0);
  expect(result.status).toBe("failed");
  expect(result.artifacts).toEqual([]);
  expect(result.task == null).toBe(true);
  expect(existsSync(f.invocations)).toBe(false);
  if (mode === "invalid-path" || mode === "signed-publication") {
    expect(result.cleanup).toMatchObject({ closed: true, referencesFinalized: true, failureCodes: [] });
    expect(existsSync(lock)).toBe(false);
    expect(result.failureCode).toContain(mode === "invalid-path" ? "wrapper_lease_required" : "signed_bundle_requires_input_bundle_attestation");
  } else {
    expect(result.cleanup).toMatchObject({ closed: false, referencesFinalized: false });
    expect(result.cleanup.failureCodes).toContain(mode === "failed-release" ? "lease_release_failed" : "lease_ownership_unproved");
    expect(readFileSync(join(lock, "lease.json"), "utf8")).toBe(readFileSync(join(f.root, "lease-before.json"), "utf8"));
    if (mode === "failed-release") expect(readFileSync(join(lock, "sentinel"), "utf8")).toBe("preserve unknown lease evidence");
  }
});
test("pre-task lease admission never releases another live owner", () => {
  const f = fixture(), lock = join(f.root, "target-agent/.locks/pool-agent-debug.lock"), generation = "parent-owned-admission-fixture";
  cacheLease("acquire", lock, [String(process.pid), generation, "1000"]);
  const before = readFileSync(join(lock, "lease.json"), "utf8");
  try {
    const out = Bun.spawnSync([process.execPath, resolve(import.meta.dir, "../agentic/build-artifact.ts"), "run-wrapper", "check", "--lib"], {
      cwd: f.root, env: { ...f.env, SCRIPT_KIT_AGENT_LEASE_PATH: lock, SCRIPT_KIT_AGENT_LEASE_GENERATION: generation }, timeout: 25_000,
    });
    const result = JSON.parse(out.stdout.toString());
    expect(out.exitCode).not.toBe(0);
    expect(result.failureCode).toContain("wrapper_lease_owner_mismatch");
    expect(result.cleanup).toMatchObject({ closed: false, referencesFinalized: false, failureCodes: ["lease_ownership_unproved"] });
    expect(readFileSync(join(lock, "lease.json"), "utf8")).toBe(before);
    expect(existsSync(f.invocations)).toBe(false);
  } finally { cacheLease("release", lock, [String(process.pid), generation]); }
});
test("unproved warm executable gets one root-only provenance rebuild", () => {
  const f = fixture(); const first = build(f.root, f.env, "application", { FAKE_CARGO_WARM: "1" });
  expect(first.status).toBe(0);
  const invocations = readFileSync(f.invocations, "utf8").trim().split("\n").map(line => JSON.parse(line));
  expect(invocations).toHaveLength(2);
  expect(invocations[0].generation).toBe("");
  expect(invocations[1].generation).not.toBe("");
  expect(invocations.every(value => value.args[0] === "build" && !value.args.includes("clean"))).toBe(true);
  const exportsBefore = readdirSync(join(f.root, "target-agent/artifacts")).sort();
  const second = build(f.root, f.env, "application", { FAKE_CARGO_WARM: "1" });
  expect(second.status).toBe(0);
  expect(second.result.artifacts[0]).toEqual(first.result.artifacts[0]);
  expect(second.result.artifactReused).toBe(true);
  expect(readdirSync(join(f.root, "target-agent/artifacts")).sort()).toEqual(exportsBefore);
  expect(readFileSync(f.invocations, "utf8").trim().split("\n")).toHaveLength(3);
});
for (const extra of [{ FAKE_CARGO_MUTATE: "1" }, { FAKE_CARGO_MUTATE: "1", FAKE_CARGO_RESTORE: "1" }, { FAKE_CARGO_AMBIGUOUS: "1" }, { FAKE_CARGO_FRESH_ALWAYS: "1" }]) test(`refuses unqualified publication ${JSON.stringify(extra)}`, () => {
  const f = fixture(), outcome = build(f.root, f.env, "application", extra);
  expect(outcome.status).not.toBe(0);
  expect(outcome.result.artifacts).toEqual([]);
});
test("active reference protects only its artifact and stale prune plans refuse", () => {
  const f = fixture(), first = build(f.root, f.env, "application");
  writeFileSync(join(f.root, "src/main.rs"), "fn main() { let _version = 2; }\n");
  const second = build(f.root, f.env, "application");
  expect(first.status).toBe(0); expect(second.status).toBe(0);
  const ref = first.result.artifacts[0];
  const prior = managedRetentionPlan(f.root);
  const claim = claimOutput(validateOutputTarget({ repoRoot: f.root, candidate: join(f.root, ".test-output/runtime-reference"), kind: "directory", probeId: "reference-fixture" }));
  const task = beginManagedTask(claim, "runtime-run", [ref]);
  const plan = managedRetentionPlan(f.root);
  expect(plan.candidates.some((candidate: { generation: string }) => candidate.generation === ref.manifestSha256)).toBe(false);
  expect(plan.candidates.some((candidate: { generation: string }) => candidate.generation === second.result.artifacts[0].manifestSha256)).toBe(true);
  expect(() => pruneManagedRecords(f.root, prior.revision, prior.candidates)).toThrow("retention_plan_changed");
  finalizeManagedTask(task, emptyOwnedCleanup());
  const closed = managedRetentionPlan(f.root);
  expect(pruneManagedRecords(f.root, closed.revision, closed.candidates).removed).toBeDefined();
});
test("dirty bytes are recorded and backdated edits cannot reuse provenance", () => {
  const f = fixture(); writeFileSync(join(f.root, "src/main.rs"), "// dirty compiler input\n");
  const outcome = build(f.root, f.env, "application"); expect(outcome.status).toBe(0);
  const artifact = verifyImmutableArtifact(f.root, outcome.result.artifacts[0], expectation("application"));
  expect(artifact.manifest.source.compilerDirty).toBe(true);
  expect(artifact.manifest.source.hermeticBuild).toBe(false);
  const stamp = lstatSync(join(f.root, "src/main.rs"));
  writeFileSync(join(f.root, "src/main.rs"), "// different current bytes\n");
  utimesSync(join(f.root, "src/main.rs"), stamp.atime, stamp.mtime);
  expect(() => verifyImmutableArtifact(f.root, outcome.result.artifacts[0], { ...expectation("application"), sourcePolicy: "current-content" })).toThrow("reviewed worktree bytes differ");
});

test("compiled theme golden bytes invalidate libtest artifacts despite backdated timestamps, unlike docs-only edits", () => {
  const f = fixture();
  writeFileSync(join(f.root, "scripts/agentic/compiler-input-paths.txt"),
    readFileSync(resolve(import.meta.dir, "../agentic/compiler-input-paths.txt")));
  const snapshotRoot = join(f.root, "tests/theme/snapshots");
  mkdirSync(snapshotRoot, { recursive: true });
  const snapshots = [
    "theme_dark_default.json",
    "theme_light_default.json",
    "preset_preview_colors.json",
    "color_string_parse_matrix.json",
  ];
  for (const snapshot of snapshots) writeFileSync(join(snapshotRoot, snapshot), '{"golden":0}\n');
  const artifact = createArtifactFixture(f.root, { existingRepository: true, kind: "rust-libtest" });
  const currentExpectation = { ...expectation("rust-libtest"), sourcePolicy: "current-content" as const };
  try {
    const before = verifyImmutableArtifact(f.root, artifact.reference, currentExpectation);
    mkdirSync(join(f.root, "docs"));
    writeFileSync(join(f.root, "docs/compiler-notes.txt"), "documentation-only edit\n");
    const afterDocs = verifyImmutableArtifact(f.root, artifact.reference, currentExpectation);
    expect(afterDocs.binary.sourceCommit).toBe(before.binary.sourceCommit);
    expect(afterDocs.manifest.source.compilerInputSha256).toBe(before.manifest.source.compilerInputSha256);
    for (const snapshot of snapshots) {
      const path = join(snapshotRoot, snapshot), bytes = readFileSync(path), stamp = lstatSync(path);
      try {
        writeFileSync(path, '{"golden":1}\n');
        utimesSync(path, stamp.atime, stamp.mtime);
        expect(() => verifyImmutableArtifact(f.root, artifact.reference, currentExpectation), snapshot)
          .toThrow("reviewed worktree bytes differ");
        expect(verifyImmutableArtifact(f.root, artifact.reference, expectation("rust-libtest")).reference)
          .toEqual(artifact.reference);
      } finally {
        writeFileSync(path, bytes);
        utimesSync(path, stamp.atime, stamp.mtime);
      }
    }
    expect(verifyImmutableArtifact(f.root, artifact.reference, currentExpectation).reference).toEqual(artifact.reference);
  } finally { artifact.dispose(); }
});

test("kind, reference hash and unfinished publication cannot become launch authority", () => {
  const f = fixture(), outcome = build(f.root, f.env, "application");
  expect(outcome.status).toBe(0);
  const reference = outcome.result.artifacts[0];
  expect(() => verifyImmutableArtifact(f.root, reference, expectation("rust-libtest"))).toThrow();
  expect(() => verifyImmutableArtifact(f.root, { ...reference, manifestSha256: "0".repeat(64) }, expectation("application"))).toThrow();
  expect(() => verifyImmutableArtifact(f.root, { ...reference, manifestPath: "../outside/manifest.json" }, expectation("application"))).toThrow();
  const record = JSON.parse(readFileSync(outcome.result.recordPath, "utf8"));
  record.state = "running";
  writeFileSync(outcome.result.recordPath, JSON.stringify(record));
  expect(() => verifyImmutableArtifact(f.root, reference, expectation("application"))).toThrow("publication task not successfully finalized");
});

test("absolute compiler input symlinks refuse before any Cargo invocation", () => {
  const f = fixture();
  renameSync(join(f.root, "src"), join(f.root, "original-src"));
  symlinkSync(join(f.root, "original-src"), join(f.root, "src"));
  const outcome = build(f.root, f.env, "application");
  expect(outcome.status).not.toBe(0);
  expect(existsSync(f.invocations)).toBe(false);
});

test("relative source links pin target bytes and retargets without relaxing artifact paths", () => {
  const f = fixture(), target = join(f.root, "licenses/LICENSE"), link = join(f.root, "license-link");
  mkdirSync(join(f.root, "licenses"));
  writeFileSync(target, "license bytes\n");
  writeFileSync(join(f.root, "licenses/other"), "license bytes\n");
  symlinkSync("licenses/LICENSE", link);
  symlinkSync("../license-link", join(f.root, "src/LICENSE"));
  const before = observeArtifactSource(f.root), originalStat = lstatSync(target);
  expect(() => assertSourceBoundary(before, observeArtifactSource(f.root))).not.toThrow();
  expect(() => canonicalArtifactPath(f.root, "src/LICENSE")).toThrow("symlink in owned path");
  writeFileSync(target, "changed bytes\n"); utimesSync(target, originalStat.atime, originalStat.mtime);
  const changed = observeArtifactSource(f.root);
  expect(changed.compilerInputSha256).not.toBe(before.compilerInputSha256);
  expect(() => assertSourceBoundary(before, changed)).toThrow("source changed");
  writeFileSync(target, "license bytes\n"); utimesSync(target, originalStat.atime, originalStat.mtime);
  const restored = observeArtifactSource(f.root);
  expect(restored.compilerInputSha256).toBe(before.compilerInputSha256);
  expect(() => assertSourceBoundary(before, restored)).toThrow("source changed");
  rmSync(link); symlinkSync("licenses/other", link);
  expect(observeArtifactSource(f.root).compilerInputSha256).not.toBe(before.compilerInputSha256);
  rmSync(link); symlinkSync("licenses/LICENSE", link);
  const retargetRestored = observeArtifactSource(f.root);
  expect(retargetRestored.compilerInputSha256).toBe(before.compilerInputSha256);
  expect(() => assertSourceBoundary(restored, retargetRestored)).toThrow("source changed");
});

test("relative directory links include descendants and registered paths", () => {
  const f = fixture();
  renameSync(join(f.root, "src"), join(f.root, "original-src"));
  symlinkSync("original-src", join(f.root, "src"));
  const before = observeArtifactSource(f.root, ["src/bin/export_design_tokens.rs"]);
  expect(() => assertSourceBoundary(before, observeArtifactSource(f.root, ["src/bin/export_design_tokens.rs"]))).not.toThrow();
  writeFileSync(join(f.root, "original-src/bin/export_design_tokens.rs"), "// changed linked descendant\n");
  const after = observeArtifactSource(f.root, ["src/bin/export_design_tokens.rs"]);
  expect(after.compilerInputSha256).not.toBe(before.compilerInputSha256);
  expect(() => assertSourceBoundary(before, after)).toThrow("source changed");
});

test("dangling source links record absence, appearance, and create-restore metadata", () => {
  const f = fixture(), target = join(f.root, "metadata/LICENSE"), link = join(f.root, "src/LICENSE");
  mkdirSync(join(f.root, "metadata")); symlinkSync("../metadata/LICENSE", link);
  const before = observeArtifactSource(f.root);
  expect(() => assertSourceBoundary(before, observeArtifactSource(f.root))).not.toThrow();
  writeFileSync(target, "new license bytes\n");
  expect(observeArtifactSource(f.root).compilerInputSha256).not.toBe(before.compilerInputSha256);
  rmSync(target);
  const restored = observeArtifactSource(f.root);
  expect(restored.compilerInputSha256).toBe(before.compilerInputSha256);
  expect(() => assertSourceBoundary(before, restored)).toThrow("source changed");
  rmSync(link); symlinkSync("../metadata/OTHER", link);
  expect(observeArtifactSource(f.root).compilerInputSha256).not.toBe(before.compilerInputSha256);
});

test("a missing component before '..' is absent rather than guessed target bytes", () => {
  const f = fixture(); symlinkSync("../missing/../Cargo.toml", join(f.root, "src/manifest-link"));
  const absent = observeArtifactSource(f.root);
  mkdirSync(join(f.root, "missing"));
  expect(observeArtifactSource(f.root).compilerInputSha256).not.toBe(absent.compilerInputSha256);
});

for (const target of ["../../outside", "/etc/passwd", "link", ".", "../escape/../Cargo.toml"]) test(`unsafe source link ${target} fails closed`, () => {
  const f = fixture();
  symlinkSync("../../outside", join(f.root, "escape"));
  symlinkSync(target, join(f.root, "src/link"));
  expect(() => observeArtifactSource(f.root)).toThrow();
});

test("mutually recursive source links fail closed", () => {
  const f = fixture();
  symlinkSync("second", join(f.root, "src/first")); symlinkSync("first", join(f.root, "src/second"));
  expect(() => observeArtifactSource(f.root)).toThrow("symlink cycle");
});

test("replacing a source ancestor while its file is hashed fails closed", () => {
  const f = fixture(), outside = fixture(), targetStat = lstatSync(join(f.root, "src/main.rs"));
  const originalRead = fs.readSync;
  let replaced = false;
  const read = spyOn(fs, "readSync").mockImplementation(((...args: unknown[]) => {
    const count = Reflect.apply(originalRead, fs, args) as number;
    const opened = fs.fstatSync(args[0] as number);
    if (!replaced && opened.dev === targetStat.dev && opened.ino === targetStat.ino) {
      replaced = true;
      renameSync(join(f.root, "src"), join(f.root, "saved-src"));
      symlinkSync(join(outside.root, "src"), join(f.root, "src"));
    }
    return count;
  }) as typeof fs.readSync);
  try {
    expect(() => observeArtifactSource(f.root)).toThrow("compiler input changed");
    expect(replaced).toBe(true);
  } finally { read.mockRestore(); }
});

for (const checking of [false, true]) test(`source-mutating fmt ${checking ? "check refuses" : "records both observations without publishing"}`, () => {
  const f = fixture(), before = observeArtifactSource(f.root);
  const args = ["fmt", "--all", ...(checking ? ["--check"] : [])];
  const out = Bun.spawnSync(["bash", resolve(import.meta.dir, "../agentic/agent-cargo.sh"), ...args], { cwd: f.root, env: { ...f.env, SCRIPT_KIT_AGENT_ARTIFACT_KIND: "", FAKE_CARGO_MUTATE: "1" }, timeout: 25_000 });
  const result = JSON.parse(out.stdout.toString());
  expect(result.artifacts).toEqual([]);
  if (checking) {
    expect(out.exitCode).not.toBe(0);
    expect(result.failureCode).toBe("source_stale");
  } else {
    expect(out.exitCode).toBe(0);
    expect(result.status).toBe("succeeded");
    expect(result.sourceMutation.before.compilerInputSha256).toBe(before.compilerInputSha256);
    expect(result.sourceMutation.after.compilerInputSha256).toBe(observeArtifactSource(f.root).compilerInputSha256);
    expect(result.sourceMutation.after.compilerInputSha256).not.toBe(result.sourceMutation.before.compilerInputSha256);
    expect(readManagedTask(result.recordPath, result.task).result.sourceMutation).toEqual(result.sourceMutation);
  }
});

test("dirty source never satisfies clean release provenance", () => {
  const f = fixture(); writeFileSync(join(f.root, "src/main.rs"), "// dirty release input\n");
  const outcome = build(f.root, f.env, "application");
  expect(outcome.status).toBe(0);
  expect(() => verifyImmutableArtifact(f.root, outcome.result.artifacts[0], { ...expectation("application"), sourcePolicy: "clean-exact-head" })).toThrow("clean exact source head required");
});

test("task generations, copied claims and directory replacement fail closed", () => {
  const f = fixture();
  const claim = claimOutput(validateOutputTarget({ repoRoot: f.root, candidate: join(f.root, ".test-output/identity"), kind: "directory", probeId: "identity" }));
  const task = beginManagedTask(claim, "evidence-run", []);
  expect(() => readManagedTask(task.recordPath, { id: task.identity.id, generation: "wrong-generation" })).toThrow();
  expect(() => assertOutputOwnership({ ...claim })).toThrow();
  const first = updateManagedTask(task, { state: "running" });
  expect(first.identity.revision).toBe(2);
  finalizeManagedTask(task, { ...emptyOwnedCleanup(), closed: false, processGroupExited: false, survivors: [{ kind: "process-group", identity: "unknown", observation: "unknown" }] });
  expect(readManagedTask(task.recordPath, task.identity).state).toBe("protected");
  expect(managedRetentionPlan(f.root).candidates).toHaveLength(0);
  renameSync(claim.root, `${claim.root}-original`);
  mkdirSync(claim.root);
  writeFileSync(claim.markerPath, readFileSync(`${claim.root}-original/.artifact-lifecycle-owner.json`));
  expect(() => assertOutputOwnership(claim)).toThrow();
});

test("malformed leases remain protected and cannot be recovered by guessing", () => {
  const f = fixture(), lock = join(f.root, "target-agent/.locks/pool-agent-debug.lock");
  mkdirSync(lock, { recursive: true });
  const observation = cacheLease("diagnose", lock, []);
  expect(observation.state).toBe("protected");
  expect(() => cacheLease("recover", lock, [JSON.stringify(observation)])).toThrow();
  expect(existsSync(lock)).toBe(true);
});

test("zero-match libtest selection is a failing build operation", () => {
  const f = fixture();
  const out = Bun.spawnSync([process.execPath, resolve(import.meta.dir, "build-ops.ts"), "act", "lib-test", "--filter", "missing_case"], { cwd: f.root, env: { ...f.env, FAKE_CARGO_ZERO: "1" }, timeout: 30000 });
  expect(out.exitCode).not.toBe(0);
  const receipt = JSON.parse(out.stdout.toString());
  expect(receipt.buildOps.result.status).toBe("failed");
  expect(receipt.evidenceClass).not.toBe("UNIT_BEHAVIOR");
});

test("signing derivation ignores only signing-owned Mach-O bytes", () => {
  const f = fixture();
  const image = (signatureBytes: number) => {
    const bytes = Buffer.alloc(192 + signatureBytes);
    bytes.writeUInt32LE(0xfeedfacf, 0); bytes.writeUInt32LE(2, 16); bytes.writeUInt32LE(88, 20);
    bytes.writeUInt32LE(0x19, 32); bytes.writeUInt32LE(72, 36); bytes.write("__LINKEDIT", 40);
    bytes.writeBigUInt64LE(BigInt(signatureBytes), 64); bytes.writeBigUInt64LE(BigInt(signatureBytes), 80);
    bytes.writeUInt32LE(0x1d, 104); bytes.writeUInt32LE(16, 108); bytes.writeUInt32LE(192, 112); bytes.writeUInt32LE(signatureBytes, 116);
    bytes.fill(7, 120, 192); bytes.fill(signatureBytes, 192);
    return bytes;
  };
  const original = join(f.root, "original"), signed = join(f.root, "signed");
  writeFileSync(original, image(32)); writeFileSync(signed, image(48));
  expect(artifactHash(readFileSync(original))).not.toBe(artifactHash(readFileSync(signed)));
  expect(unsignedMachOPayloadSha256(original)).toBe(unsignedMachOPayloadSha256(signed));
  const changed = image(48); changed[150] = 9; writeFileSync(signed, changed);
  expect(unsignedMachOPayloadSha256(original)).not.toBe(unsignedMachOPayloadSha256(signed));
});

test("published libtest reuse never launches Cargo and rejects an empty selection", () => {
  const f = fixture(), built = build(f.root, f.env, "rust-libtest");
  expect(built.status).toBe(0);
  const referencePath = join(f.root, ".test-output/libtest.reference.json");
  writeFileSync(referencePath, JSON.stringify(built.result.artifacts[0]));
  const invocations = readFileSync(f.invocations, "utf8");
  const exportsBefore = readdirSync(join(f.root, "target-agent/artifacts")).sort();
  const manifestPath = join(f.root, built.result.artifacts[0].manifestPath);
  const manifestBefore = readFileSync(manifestPath, "utf8");
  for (const [filter, success] of [["fixture", true], ["fixture", true], ["missing_case", false]] as const) {
    const out = Bun.spawnSync([process.execPath, resolve(import.meta.dir, "build-ops.ts"), "act", "lib-test", "--reference", referencePath, "--filter", filter], { cwd: f.root, env: f.env, timeout: 30000 });
    expect(out.exitCode === 0).toBe(success);
    const receipt = JSON.parse(out.stdout.toString());
    expect(receipt.buildOps.result.passedTests).toBe(success ? 1 : 0);
    expect(receipt.cleanup.closed).toBe(true);
    expect(receipt.buildOps.result.artifact).toEqual(built.result.artifacts[0]);
    const record = readManagedTask(managedTaskRecordPath(f.root, receipt.buildOps.identity), receipt.buildOps.identity);
    expect(record.identity).toEqual(receipt.buildOps.identity);
    expect(record.state).toBe("closed");
    expect(record.cleanup.referencesFinalized).toBe(true);
  }
  expect(readFileSync(f.invocations, "utf8")).toBe(invocations);
  expect(readdirSync(join(f.root, "target-agent/artifacts")).sort()).toEqual(exportsBefore);
  expect(readFileSync(manifestPath, "utf8")).toBe(manifestBefore);
});

test("sealing a bundle preserves content identity without accepting writable publication", () => {
  const f = fixture(), bundle = join(f.root, "Fixture.app"), executable = join(bundle, "program");
  mkdirSync(bundle); writeFileSync(executable, "fixture payload", { mode: 0o755 });
  const before = bundleTreeHash(bundle);
  chmodSync(executable, 0o500); chmodSync(bundle, 0o500);
  expect(bundleTreeHash(bundle, true)).toBe(before);
  chmodSync(executable, 0o600);
  expect(() => bundleTreeHash(bundle, true)).toThrow("mutable entry");
});

test.skipIf(process.platform !== "darwin")("bundle identity includes raw Darwin extended attributes on files and directories", () => {
  const f = fixture(), bundle = join(f.root, "Attributes.app"), file = join(bundle, "payload");
  mkdirSync(bundle); writeFileSync(file, "unchanged payload\n");
  const before = bundleTreeHash(bundle), name = "com.scriptkit.fixture";
  const xattr = (...args: string[]) => { expect(Bun.spawnSync(["/usr/bin/xattr", ...args], { timeout: 10_000 }).exitCode).toBe(0); };
  xattr("-wx", name, "00ff0a80", file);
  const fileAttribute = bundleTreeHash(bundle);
  expect(fileAttribute).not.toBe(before);
  xattr("-wx", name, "00ff0a81", file);
  expect(bundleTreeHash(bundle)).not.toBe(fileAttribute);
  xattr("-d", name, file);
  expect(bundleTreeHash(bundle)).toBe(before);
  xattr("-wx", name, "00ff0a80", bundle);
  expect(bundleTreeHash(bundle)).not.toBe(before);
  xattr("-d", name, bundle);
  expect(bundleTreeHash(bundle)).toBe(before);
});

interface FacadeReceipt {
  pass: boolean;
  evidenceClass: string;
  disposition: string;
  cleanup: OwnedCleanup;
  buildOps: { identity: TaskIdentity; result: BuildOperationResult };
}

async function captureBuildReceipt(f: BuildWorkspaceFixture, args: string[]): Promise<FacadeReceipt> {
  const output = spyOn(console, "log").mockImplementation(() => {});
  const previousExitCode = process.exitCode, previousRoot = process.env.SCRIPT_KIT_REPO_ROOT;
  try {
    process.env.SCRIPT_KIT_REPO_ROOT = f.root;
    await runBuildOps(args);
    return JSON.parse(String(output.mock.calls.at(-1)?.[0]));
  } finally {
    // Bun leaves the current status unchanged when assigned undefined.
    output.mockRestore(); process.exitCode = previousExitCode ?? 0;
    if (previousRoot === undefined) delete process.env.SCRIPT_KIT_REPO_ROOT; else process.env.SCRIPT_KIT_REPO_ROOT = previousRoot;
  }
}

async function withBuildEnvironment(f: BuildWorkspaceFixture, action: () => Promise<void>): Promise<void> {
  const previous = { ...process.env };
  try {
    for (const key of Object.keys(process.env)) delete process.env[key];
    Object.assign(process.env, f.env);
    await action();
  } finally {
    for (const key of Object.keys(process.env)) delete process.env[key];
    Object.assign(process.env, previous);
  }
}

test("changed route query exposes exact selected checks without spawning or creating outputs", async () => {
  const f = fixture(), before = readdirSync(f.root).sort();
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess");
  try {
    for (const paths of [["README.md"], ["src/main.rs"], ["scripts/devtools/build-ops.ts"], ["scripts/unreviewed.py"], []]) {
      const receipt = await captureBuildReceipt(f, ["query", "route", ...paths]);
      expect(receipt.pass).toBe(true);
      expect(receipt.buildOps.result).toEqual({ ...routeChanged(paths, false, f.root), performedVerification: false });
      expect(receipt.evidenceClass).toBe("STATIC_INVENTORY");
    }
    expect(spawn).not.toHaveBeenCalled();
    expect(readdirSync(f.root).sort()).toEqual(before);
    expect(existsSync(f.invocations)).toBe(false);
    expect(existsSync(join(f.root, "target-agent"))).toBe(false);
    expect(existsSync(join(f.root, ".test-output"))).toBe(false);
  } finally { spawn.mockRestore(); }
});

test("changed action intentionally empty route succeeds unlike an unknown action", async () => {
  const f = fixture(), spawn = spyOn(ownedProcess, "spawnOwnedProcess");
  try {
    const receipt = await captureBuildReceipt(f, ["act", "changed", "README.md", ".notes/report.md", ".test-output/receipt.json"]);
    expect(receipt.pass).toBe(true);
    expect(receipt.evidenceClass).toBe("STATIC_INVENTORY");
    expect(receipt.buildOps.result).toMatchObject({ status: "succeeded", noRustDecision: true, performedVerification: false, selectedChecksComplete: true, attemptedSteps: [], notExecutedSteps: [] });
    expect((receipt.buildOps.result.route as ChangedRoute).steps).toEqual([]);
    expect((receipt.buildOps.result.route as ChangedRoute).coverageGaps).toContain("documentation_and_evidence_contents_not_verified");
    const unknown = await captureBuildReceipt(f, ["act", "not-a-build-action"]);
    expect(unknown.pass).toBe(false);
    expect(unknown.buildOps.result.status).toBe("failed");
    expect(unknown.buildOps.result.noRustDecision).toBeUndefined();
    expect(spawn).not.toHaveBeenCalled();
    expect(receipt.cleanup).toMatchObject({ closed: true, resourcesAcquired: false });
  } finally { spawn.mockRestore(); }
});

test("agent-check executes the explicit documentation decision without Rust or output roots", () => {
  const f = fixture(), before = readdirSync(f.root).sort();
  const out = Bun.spawnSync(["bash", resolve(import.meta.dir, "../agent-check.sh"), "--", "README.md"], { cwd: f.root, env: f.env, timeout: 25_000 });
  expect(out.exitCode).toBe(0);
  const receipt = JSON.parse(out.stdout.toString());
  expect(receipt.pass).toBe(true);
  expect(receipt.buildOps.result).toMatchObject({ status: "succeeded", noRustDecision: true, performedVerification: false });
  expect(readdirSync(f.root).sort()).toEqual(before);
  expect(existsSync(f.invocations)).toBe(false);
});

for (const outcome of ["pass", "fail", "zero"] as const) test(`changed action runs owned noncompiler contracts and reports ${outcome}`, async () => {
  const f = fixture();
  mkdirSync(join(f.root, "scripts/devtools"));
  writeFileSync(join(f.root, "scripts/devtools/build-ops.test.ts"), `import { test, expect } from "bun:test";\n${outcome === "zero" ? 'test.skip("fixture", () => {});' : `test("fixture", () => expect(1).toBe(${outcome === "pass" ? 1 : 2}));`}\n`);
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess");
  const stderr = spyOn(process.stderr, "write").mockImplementation(() => true);
  try {
    await withBuildEnvironment(f, async () => {
      const query = await captureBuildReceipt(f, ["query", "route", "scripts/devtools/build-ops.ts"]);
      const receipt = await captureBuildReceipt(f, ["act", "changed", "scripts/devtools/build-ops.ts", "--timeout-ms", "10000"]);
      const route = receipt.buildOps.result.route as ChangedRoute;
      expect(receipt.pass).toBe(outcome === "pass");
      expect(receipt.buildOps.result.status).toBe(outcome === "pass" ? "succeeded" : "failed");
      expect(route.steps).toEqual(query.buildOps.result.steps);
      expect(receipt.buildOps.result.attemptedSteps).toEqual(route.steps);
      expect(receipt.buildOps.result.notExecutedSteps).toEqual([]);
      expect(receipt.buildOps.result.noRustDecision).toBe(true);
      expect(receipt.buildOps.result.passedTests).toBe(outcome === "pass" ? 1 : 0);
      expect(receipt.evidenceClass).toBe(outcome === "pass" ? "UNIT_BEHAVIOR" : "STATIC_INVENTORY");
      expect(receipt.cleanup).toMatchObject({ resourcesAcquired: true, closed: true, processGroupExited: true, streamsDrained: true, survivors: [] });
      expect(spawn).toHaveBeenCalledTimes(1);
      const request = spawn.mock.calls[0]![0];
      expect(request.argv).toEqual([process.execPath, ...route.steps[0]!.args]);
      expect(request.env).toMatchObject({ SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0", SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0", PYTHONDONTWRITEBYTECODE: "1" });
      expect(request.maxOutputBytes).toBe(8 * 1024 * 1024);
      expect(existsSync(f.invocations)).toBe(false);
      expect(existsSync(join(f.root, "target-agent"))).toBe(false);
    });
  } finally { spawn.mockRestore(); stderr.mockRestore(); }
});

test("changed action refuses unsafe opt-ins and hidden verification overrides before subprocess acquisition", async () => {
  const f = fixture(), spawn = spyOn(ownedProcess, "spawnOwnedProcess");
  try {
    await withBuildEnvironment(f, async () => {
      for (const verb of ["act", "query"]) {
        const receipt = await captureBuildReceipt(f, [verb, verb === "act" ? "changed" : "route", "README.md", "--filter", "guessed_test"]);
        expect(receipt.pass).toBe(false);
      }
      process.env.SCRIPT_KIT_ALLOW_NATIVE_INPUT = "1";
      const receipt = await captureBuildReceipt(f, ["act", "changed", "scripts/devtools/build-ops.ts"]);
      expect(receipt.pass).toBe(false);
      expect(receipt.cleanup).toMatchObject({ resourcesAcquired: false, closed: true });
      expect(receipt.buildOps.result.performedVerification).toBe(false);
      expect(spawn).not.toHaveBeenCalled();
    });
  } finally { spawn.mockRestore(); }
});

async function wrapperOutputProcess(f: BuildWorkspaceFixture, output: string): Promise<OwnedProcess> {
  return ownedProcess.spawnOwnedProcess({ argv: [process.execPath, "-e", "process.stdout.write(process.env.FACADE_OUTPUT)"], cwd: f.root,
    env: { ...f.env, FACADE_OUTPUT: output }, timeoutMs: 5_000, maxOutputBytes: 16 * 1024 * 1024 });
}

test("stdout parsing errors carry the real acquired process and closure in a named failure", async () => {
  const f = fixture(), child = await wrapperOutputProcess(f, "{incomplete");
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess").mockResolvedValue(child);
  try {
    let failure: unknown;
    try { await executeBuildAction(f.root, "domain-check", ["check"], 5_000); } catch (error) { failure = error; }
    expect(failure).toBeInstanceOf(BuildOperationError);
    const operation = failure as BuildOperationError;
    expect(operation.code).toBe("wrapper_result_unavailable");
    expect(operation.result.ownedProcesses).toEqual([child.identity]);
    expect(operation.cleanup).toMatchObject({ resourcesAcquired: true, processExited: true, processGroupExited: true, streamsDrained: true, referencesFinalized: false, closed: false });
    expect(operation.cleanup.failureCodes).toContain("wrapper_cleanup_unproved");
  } finally { spawn.mockRestore(); await child.close(); }
});

test("an unobserved post-spawn close emits INVALID_CLEANUP with the exact identities", async () => {
  const f = fixture(), child = await wrapperOutputProcess(f, "{incomplete");
  const actualClose = child.close.bind(child);
  const close = spyOn(child, "close").mockRejectedValue(new Error("injected_close_observation_failure"));
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess").mockResolvedValue(child);
  try {
    const receipt = await captureBuildReceipt(f, ["act", "domain-check"]);
    expect(receipt.disposition).toBe("INVALID_CLEANUP");
    expect(receipt.buildOps.result.failureCode).toBe("wrapper_result_unavailable");
    expect(receipt.buildOps.result.ownedProcesses).toEqual([child.identity]);
    expect(receipt.cleanup).toMatchObject({ resourcesAcquired: true, closed: false, processExited: false, processGroupExited: false });
    expect(receipt.cleanup.survivors).toContainEqual({ kind: "process-group", identity: `${child.identity.processGroupId}:${child.identity.sessionGeneration}`, observation: "unknown" });
    expect(close).toHaveBeenCalledTimes(1);
  } finally { spawn.mockRestore(); close.mockRestore(); await actualClose(); }
});

for (const failure of ["source_stale", "configuration_stale"] as const) test(`post-build ${failure} preserves its typed disposition and actual cleanup`, async () => {
  const f = fixture(), built = build(f.root, f.env, "application");
  expect(built.status).toBe(0);
  const child = await wrapperOutputProcess(f, JSON.stringify(built.result));
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess").mockResolvedValue(child);
  try {
    await withBuildEnvironment(f, async () => {
      if (failure === "source_stale") writeFileSync(join(f.root, "src/main.rs"), "// changed after publication\n");
      else process.env.RUSTFLAGS = "--cfg facade_configuration_injection";
      const receipt = await captureBuildReceipt(f, ["act", "app-build"]);
      expect(receipt.disposition).toBe("BLOCKED_STALE_GENERATION");
      expect(receipt.buildOps.result.failureCode).toBe(failure);
      expect(receipt.buildOps.result.disposition).toBe("BLOCKED_STALE_GENERATION");
      expect(receipt.buildOps.result.ownedProcesses).toContainEqual(child.identity);
      expect(receipt.buildOps.identity.id).toBe(built.result.task.id);
      expect(receipt.buildOps.identity.generation).toBe(built.result.task.generation);
      expect(receipt.cleanup).toMatchObject({ resourcesAcquired: true, closed: true, processExited: true, processGroupExited: true, referencesFinalized: true, survivors: [] });
    });
  } finally { spawn.mockRestore(); await child.close(); }
});

for (const failure of ["result-update", "task-finalize", "log-close"] as const) test(`libtest ${failure} failure does not skip independent finalizers or discard process evidence`, async () => {
  const f = fixture(), built = build(f.root, f.env, "rust-libtest");
  expect(built.status).toBe(0);
  const referencePath = join(f.root, ".test-output/facade-libtest.reference.json");
  writeFileSync(referencePath, JSON.stringify(built.result.artifacts[0]));
  const actualUpdate = lifecycle.updateManagedTask, actualFinalize = lifecycle.finalizeManagedTask, actualOpen = fs.openSync, actualClose = fs.closeSync;
  let logFd: number | undefined, logClosed = false, finalizerCalled = false;
  const open = spyOn(fs, "openSync").mockImplementation((path, flags, mode) => {
    const fd = actualOpen(path, flags, mode);
    if (String(path).endsWith("/libtest.log")) logFd = fd;
    return fd;
  });
  const close = spyOn(fs, "closeSync").mockImplementation(fd => {
    if (fd === logFd) {
      if (failure === "log-close") throw new Error("injected_log_close_failure");
      logClosed = true;
    }
    return actualClose(fd);
  });
  const update = spyOn(lifecycle, "updateManagedTask").mockImplementation((task, patch) => {
    if (failure === "result-update" && task.identity.kind === "runtime-run" && patch.state === "finalizing") throw new Error("injected_result_update_failure");
    return actualUpdate(task, patch);
  });
  const finalize = spyOn(lifecycle, "finalizeManagedTask").mockImplementation((task, cleanup) => {
    if (task.identity.kind === "runtime-run") {
      finalizerCalled = true;
      if (failure === "task-finalize") throw new Error("injected_task_finalization_failure");
    }
    return actualFinalize(task, cleanup);
  });
  try {
    await withBuildEnvironment(f, async () => {
      const receipt = await captureBuildReceipt(f, ["act", "lib-test", "--reference", referencePath, "--filter", "fixture"]);
      expect(receipt.disposition).toBe("INVALID_CLEANUP");
      expect(receipt.cleanup).toMatchObject({ resourcesAcquired: true, closed: false, processExited: true, processGroupExited: true, logWriterClosed: failure !== "log-close" });
      expect(receipt.buildOps.result.failureCode).toBe("libtest_finalization_failed");
      expect(receipt.buildOps.result.ownedProcesses).toHaveLength(1);
      expect(finalizerCalled).toBe(true);
      expect(logClosed).toBe(failure !== "log-close");
      const identity = receipt.buildOps.identity;
      const record = lifecycle.readManagedTask(lifecycle.managedTaskRecordPath(f.root, identity), identity);
      expect(receipt.buildOps.result.ownedProcesses).toEqual(record.ownedProcesses);
      expect(record.state).toBe(failure === "task-finalize" ? "finalizing" : "protected");
      expect(receipt.cleanup.failureCodes).toContain(failure === "result-update" ? "task_result_finalization_failed" : failure === "task-finalize" ? "task_finalization_failed" : "log_close_failed");
      if (failure !== "log-close") expect(receipt.cleanup.referencesFinalized).toBe(false);
    });
  } finally {
    open.mockRestore(); close.mockRestore(); update.mockRestore(); finalize.mockRestore();
    if (logFd !== undefined && !logClosed) actualClose(logFd);
  }
});

test("pre-acquisition refusal alone retains the empty closed cleanup", async () => {
  const f = fixture();
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess");
  try {
    const receipt = await captureBuildReceipt(f, ["act", "not-a-build-action"]);
    expect(spawn).not.toHaveBeenCalled();
    expect(receipt.cleanup).toMatchObject({ resourcesAcquired: false, closed: true, survivors: [] });
  } finally { spawn.mockRestore(); }
});

test("stdout size failure retains process closure instead of pre-spawn cleanup", async () => {
  const f = fixture();
  const child = await ownedProcess.spawnOwnedProcess({ argv: [process.execPath, "-e", "process.stdout.write('x'.repeat(9 * 1024 * 1024))"], cwd: f.root, env: f.env, timeoutMs: 5_000, maxOutputBytes: 16 * 1024 * 1024 });
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess").mockResolvedValue(child);
  try {
    const receipt = await captureBuildReceipt(f, ["act", "domain-check"]);
    expect(receipt.buildOps.result.failureCode).toBe("wrapper_result_limit");
    expect(receipt.buildOps.result.ownedProcesses).toEqual([child.identity]);
    expect(receipt.disposition).toBe("INVALID_CLEANUP");
    expect(receipt.cleanup.resourcesAcquired).toBe(true);
    expect(receipt.cleanup.closed).toBe(false);
    expect(receipt.cleanup.failureCodes).toContain("wrapper_cleanup_unproved");
  } finally { spawn.mockRestore(); await child.close(); }
});

test("typed wrapper failures keep code and disposition even when cleanup is unknown", async () => {
  const f = fixture();
  const wrapperCleanup: OwnedCleanup = { ...emptyOwnedCleanup(), resourcesAcquired: true, closed: false, referencesFinalized: false,
    failureCodes: ["injected_reference_finalization_failure"], survivors: [{ kind: "managed-task", identity: "wrapper-fixture-generation", observation: "unknown" }] };
  const child = await wrapperOutputProcess(f, JSON.stringify({ status: "failed", failureCode: "configuration_stale", disposition: "BLOCKED_STALE_GENERATION", cleanup: wrapperCleanup }));
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess").mockResolvedValue(child);
  try {
    const receipt = await captureBuildReceipt(f, ["act", "domain-check"]);
    expect(receipt.disposition).toBe("INVALID_CLEANUP");
    expect(receipt.buildOps.result.failureCode).toBe("configuration_stale");
    expect(receipt.buildOps.result.disposition).toBe("BLOCKED_STALE_GENERATION");
    expect(receipt.buildOps.result.ownedProcesses).toEqual([child.identity]);
    expect(receipt.cleanup).toMatchObject({ resourcesAcquired: true, closed: false, referencesFinalized: false });
    expect(receipt.cleanup.failureCodes).toContain("injected_reference_finalization_failure");
    expect(receipt.cleanup.survivors).toEqual(wrapperCleanup.survivors);
  } finally { spawn.mockRestore(); await child.close(); }
});

for (const state of ["running", "finalizing", "stale-revision", "unregistered", "closed-failure"] as const) test(`wrapper ${state} cleanup is checked against its exact managed record`, async () => {
  const f = fixture();
  const claim = claimOutput(validateOutputTarget({ repoRoot: f.root, candidate: join(f.root, ".test-output/managed-tasks/finalization-fixture"), kind: "directory", probeId: "facade-finalization" }), "finalization-fixture");
  const task = beginManagedTask(claim, "build-job", []);
  const claimedCleanup = { ...emptyOwnedCleanup(), resourcesAcquired: true };
  if (state === "running" || state === "finalizing") updateManagedTask(task, { state });
  else finalizeManagedTask(task, claimedCleanup);
  const identity = { ...task.identity, ...(state === "stale-revision" ? { revision: task.identity.revision - 1 } : {}), ...(state === "unregistered" ? { id: "not-registered" } : {}) };
  const child = await wrapperOutputProcess(f, JSON.stringify({ status: "failed", exitCode: 42, failureCode: "fixture_compile_failed", task: identity, recordPath: task.recordPath, cleanup: claimedCleanup, ownedProcesses: [] }));
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess").mockResolvedValue(child);
  try {
    const receipt = await captureBuildReceipt(f, ["act", "domain-check"]);
    expect(receipt.buildOps.result.failureCode).toBe("fixture_compile_failed");
    expect(receipt.buildOps.result.exitCode).toBe(42);
    expect(receipt.disposition).toBe(state === "closed-failure" ? "EVALUABLE_FAIL" : "INVALID_CLEANUP");
    expect(receipt.cleanup.closed).toBe(state === "closed-failure");
    expect(receipt.cleanup.referencesFinalized).toBe(state === "closed-failure");
    expect(receipt.cleanup.processExited).toBe(true);
    if (state !== "closed-failure") expect(receipt.cleanup.failureCodes).toContain("wrapper_task_finalization_unproved");
    expect(readManagedTask(task.recordPath, task.identity).state).toBe(state === "running" || state === "finalizing" ? state : "closed");
  } finally { spawn.mockRestore(); await child.close(); }
});

for (const complete of [true, false]) test(`resource refusal requires an authoritative observation (${complete})`, async () => {
  const f = fixture();
  const refusal = { ...(complete ? { phase: "post-cargo" } : {}), complete: true, targetAgentAllocatedBytes: 2048, availableBytes: 4096,
    reserveBytes: 0, targetAgentBudgetBytes: 1024, minimumFreeBytes: 2048, withinLimits: false, failureCodes: ["resource_budget_exceeded"],
    scope: "target-agent", measurement: "allocated-blocks", hardQuota: false, automaticEviction: false };
  const resources = { scope: "target-agent", measurement: "allocated-blocks", hardQuota: false, automaticEviction: false, checks: [refusal], monitoring: null, refusal };
  const child = await wrapperOutputProcess(f, JSON.stringify({ status: "failed", failureCode: "resource_budget_exceeded", cleanup: emptyOwnedCleanup(), resources, ownedProcesses: [] }));
  const spawn = spyOn(ownedProcess, "spawnOwnedProcess").mockResolvedValue(child);
  try {
    const receipt = await captureBuildReceipt(f, ["act", "domain-check"]);
    expect(receipt.disposition).toBe(complete ? "BLOCKED_RESOURCE_BUDGET" : "INVALID_SCHEMA");
    expect(receipt.cleanup.closed).toBe(true);
    if (complete) {
      expect(receipt.buildOps.result.failureCode).toBe("resource_budget_exceeded");
      expect(receipt.buildOps.result.resources?.hardQuota).toBe(false);
      expect(receipt.buildOps.result.artifact).toBeUndefined();
    } else expect(receipt.buildOps).toBeUndefined();
  } finally { spawn.mockRestore(); await child.close(); }
});

test("facade prune requires an exact selection and leaves unselected outputs intact", async () => {
  const f = fixture();
  const tasks = ["selected", "unselected"].map(id => {
    const claim = claimOutput(validateOutputTarget({ repoRoot: f.root, candidate: join(f.root, `.test-output/managed-tasks/${id}`), kind: "directory", probeId: "facade-prune" }), id);
    const task = beginManagedTask(claim, "evidence-run", []);
    finalizeManagedTask(task, emptyOwnedCleanup());
    return task;
  });
  const plan = managedRetentionPlan(f.root);
  const ownerToken = lifecycle.readOwnedJson(join(dirname(tasks[0]!.recordPath), ".artifact-lifecycle-owner.json")).token;
  const queried = await captureBuildReceipt(f, ["query", "retention"]);
  expect(queried.disposition).toBe("EVALUABLE_PASS");
  expect(JSON.stringify(queried)).not.toContain(String(ownerToken));
  const publicCandidates = queried.buildOps.result.candidates;
  if (!Array.isArray(publicCandidates)) throw new Error("retention_candidates_missing");
  const refused = await captureBuildReceipt(f, ["act", "prune", "--expect-revision", plan.revision]);
  expect(refused.disposition).not.toBe("EVALUABLE_PASS");
  expect(tasks.every(task => existsSync(task.recordPath))).toBe(true);
  const selection = join(f.root, ".test-output/selected-prune.json");
  writeFileSync(selection, JSON.stringify({ candidates: publicCandidates.filter((candidate: { id: string }) => candidate.id === "selected") }));
  const receipt = await captureBuildReceipt(f, ["act", "prune", "--expect-revision", plan.revision, "--input", selection]);
  expect(receipt.disposition).toBe("EVALUABLE_PASS");
  expect(JSON.stringify(receipt)).not.toContain(String(ownerToken));
  expect(existsSync(tasks[0]!.recordPath)).toBe(false);
  expect(existsSync(tasks[1]!.recordPath)).toBe(true);
});

test("facade keep-set pins exact historical references and refuses a stale revision", async () => {
  const f = fixture(), built = build(f.root, f.env, "application");
  expect(built.status).toBe(0);
  const reference = built.result.artifacts[0];
  const prior = managedKeepSet(f.root);
  writeFileSync(join(f.root, "src/main.rs"), "fn main() { let _changed_after_publication = true; }\n");
  const selection = join(f.root, ".test-output/keep-selection.json");
  writeFileSync(selection, JSON.stringify({ references: [reference] }));
  const kept = await captureBuildReceipt(f, ["act", "keep-set", "--expect-revision", prior.revision, "--input", selection]);
  expect(kept.disposition).toBe("EVALUABLE_PASS");
  expect(managedKeepSet(f.root).references).toEqual([reference]);
  expect(managedRetentionPlan(f.root).candidates.some((candidate: { generation: string }) => candidate.generation === reference.manifestSha256)).toBe(false);
  const stale = await captureBuildReceipt(f, ["act", "keep-set", "--expect-revision", prior.revision, "--input", selection]);
  expect(stale.disposition).toBe("BLOCKED_STALE_GENERATION");
  expect(managedKeepSet(f.root).references).toEqual([reference]);
});

test("a durably retired warm export is republished without cleaning compiler outputs", () => {
  const f = fixture(), first = build(f.root, f.env, "application");
  expect(first.status).toBe(0);
  const reference = first.result.artifacts[0];
  const plan = managedRetentionPlan(f.root);
  const selected = plan.candidates.filter((candidate: { kind: string; generation: string }) => candidate.kind === "artifact" && candidate.generation === reference.manifestSha256);
  expect(selected).toHaveLength(1);
  expect(pruneManagedRecords(f.root, plan.revision, selected).removed).toHaveLength(1);
  expect(existsSync(join(f.root, reference.manifestPath))).toBe(false);
  const invocationsBefore = readFileSync(f.invocations, "utf8").trim().split("\n").length;
  const second = build(f.root, f.env, "application", { FAKE_CARGO_WARM: "1" });
  expect(second.status).toBe(0);
  expect(second.result.artifacts).toHaveLength(1);
  expect(second.result.artifacts[0]).not.toEqual(reference);
  expect(second.result.artifactReused).toBe(false);
  const invocations = readFileSync(f.invocations, "utf8").trim().split("\n").map(line => JSON.parse(line));
  expect(invocations).toHaveLength(invocationsBefore + 1);
  expect(invocations.every(value => !value.args.includes("clean"))).toBe(true);
});

test("unexplained disappearance of a warm export is not silently repaired", () => {
  const f = fixture(), first = build(f.root, f.env, "application");
  expect(first.status).toBe(0);
  const directory = dirname(join(f.root, first.result.artifacts[0].manifestPath));
  chmodSync(directory, 0o700);
  rmSync(directory, { recursive: true });
  const second = build(f.root, f.env, "application", { FAKE_CARGO_WARM: "1" });
  expect(second.status).not.toBe(0);
  expect(second.result.artifacts).toEqual([]);
  expect(readdirSync(join(f.root, "target-agent/artifacts")).filter(name => name.startsWith("artifact-"))).toEqual([]);
});
