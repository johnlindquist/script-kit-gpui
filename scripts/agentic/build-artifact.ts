#!/usr/bin/env bun
import { createHash, randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import { chmodSync, closeSync, constants, copyFileSync, existsSync, fstatSync, fsyncSync, lstatSync, mkdirSync, openSync, readFileSync, readlinkSync, readSync, readdirSync, realpathSync, renameSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { homedir } from "node:os";
import { pathToFileURL } from "node:url";
import type { Stats } from "node:fs";
import { atomicManagedJson, beginManagedTask, bindSupervisorTask, cacheLease, canonicalJson, claimOutput, emptyOwnedCleanup, finalizeManagedTask, finalizeSupervisorTask, isRetiredManagedArtifact, managedTaskRecordPath, readManagedTask, readOwnedJson, registerManagedArtifactReference, registerManagedPublicationIntent, updateManagedPublicationIntent, updateManagedTask, validateOutputTarget, withManagedMetadata } from "./artifact-lifecycle.ts";
import type { ManagedTask, OwnedCleanup, OwnedProcessIdentity, TaskIdentity } from "./artifact-lifecycle.ts";
import { spawnOwnedProcess } from "./owned-process.ts";
import { BuildResourceError, requireBuildAdmission, startBuildResourceGuard } from "../devtools/lib/build-ops-inventory.ts";
import type { BuildResourceObservation, BuildResourceReport, BuildResourceSummary } from "../devtools/lib/build-ops-inventory.ts";

export type Sha256 = string;
export type ArtifactKind = "application" | "rust-libtest" | "tool";
export interface ArtifactReference { readonly manifestPath: string; readonly manifestSha256: Sha256; }
export interface CargoTargetIdentity {
  readonly packageId: string; readonly packageName: string; readonly targetName: string;
  readonly targetKind: readonly string[]; readonly crateTypes: readonly string[];
  readonly sourcePath: string; readonly features: readonly string[];
  readonly cargoProfile: Readonly<Record<string, unknown>>;
  readonly requestedProfile: "dev" | "test" | "release"; readonly targetTriple: string;
}
export interface SourceIdentity {
  readonly algorithm: "reviewed-worktree-content-v1";
  readonly compilerInputSha256: Sha256; readonly compilerInputOwnerSha256: Sha256;
  readonly committedInputTreeSha256: Sha256; readonly gitHead: string;
  readonly repositoryDirty: boolean; readonly compilerDirty: boolean; readonly inputPaths: readonly string[];
  readonly changeBoundaryBeforeSha256: Sha256; readonly changeBoundaryAfterSha256: Sha256;
  readonly hermeticBuild: false;
}
export interface ArtifactManifestV3 {
  readonly schemaVersion: 3; readonly artifactId: string; readonly artifactKind: ArtifactKind;
  readonly binaryPath: string; readonly binarySha256: Sha256; readonly sizeBytes: number;
  readonly target: CargoTargetIdentity; readonly source: SourceIdentity;
  readonly toolchain: { readonly channel: string; readonly rustcVerboseVersion: string; readonly cargoVersion: string; readonly rustcSha256: Sha256; readonly cargoSha256: Sha256; };
  readonly requestedPolicySha256: Sha256;
  readonly effectiveConfiguration: Readonly<Record<string, unknown>>;
  readonly effectiveConfigurationSha256: Sha256; readonly requiresExactGitHead: boolean;
  readonly publication: { readonly owner: "scripts/agentic/agent-cargo.sh"; readonly pool: "agent-debug";
    readonly leaseGeneration: string; readonly buildTask: TaskIdentity; readonly immutable: true; readonly exportedWhileLeaseHeld: true; };
  readonly derivation?: { readonly input: ArtifactReference; readonly transformation: "signed-and-stapled-bundle";
    readonly attestationSha256: Sha256; readonly bundleTreeSha256: Sha256; };
}
export interface ArtifactExpectation {
  readonly kind: ArtifactKind; readonly packageName: string; readonly targetName: string;
  readonly profile?: "dev" | "test" | "release";
  readonly sourcePolicy: "current-content" | "clean-exact-head" | "recorded-content";
}
export interface VerifiedArtifact {
  readonly reference: ArtifactReference; readonly manifest: ArtifactManifestV3;
  readonly executablePath: string; readonly binary: Readonly<Record<string, unknown>>;
}
export type ArtifactFailureCode = "manifest_invalid" | "manifest_hash_mismatch" | "binary_hash_mismatch" | "source_stale" | "configuration_stale" | "toolchain_mismatch" | "publication_not_finalized" | "unsafe_artifact_path";
export class ArtifactVerificationError extends Error {
  constructor(readonly code: ArtifactFailureCode, readonly disposition: "INVALID_BINARY" | "BLOCKED_STALE_GENERATION" | "BLOCKED_SCOPE_DRIFT", message: string) { super(message); this.name = "ArtifactVerificationError"; }
}
const verifiedArtifacts = new WeakSet<object>();
const verificationStack = new Set<string>();
export function isVerifiedArtifact(value: unknown): value is VerifiedArtifact { return !!value && typeof value === "object" && verifiedArtifacts.has(value); }
export const artifactHash = (bytes: string | Uint8Array): Sha256 => createHash("sha256").update(bytes).digest("hex");
const fingerprint = (value: unknown): Sha256 => artifactHash(canonicalJson(value));
const isHash = (value: unknown): value is Sha256 => typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
function refuse(code: ArtifactFailureCode, message: string): never {
  throw new ArtifactVerificationError(code, ["source_stale", "configuration_stale", "toolchain_mismatch"].includes(code) ? "BLOCKED_STALE_GENERATION" : "INVALID_BINARY", message);
}

export function canonicalArtifactPath(root: string, child: string): string {
  if (!child || isAbsolute(child) || child.includes("\\") || child.split("/").some(part => !part || part === "." || part === "..")) refuse("unsafe_artifact_path", "noncanonical repository-relative path");
  let cursor = realpathSync(root);
  for (const part of child.split("/")) {
    cursor = join(cursor, part);
    try { if (lstatSync(cursor).isSymbolicLink()) refuse("unsafe_artifact_path", "symlink in owned path"); }
    catch (error) { if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error; }
  }
  return cursor;
}

function stamp(stat: Stats): unknown[] { return [stat.dev, stat.ino, stat.mode, stat.size, stat.mtimeMs, stat.ctimeMs]; }
export function hashArtifactFile(path: string): Sha256 {
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const before = fstatSync(fd);
    if (!before.isFile()) refuse("unsafe_artifact_path", "expected regular file");
    const hash = createHash("sha256");
    const buffer = Buffer.allocUnsafe(1024 * 1024);
    let count: number;
    while ((count = readSync(fd, buffer, 0, buffer.length, null))) hash.update(buffer.subarray(0, count));
    if (canonicalJson(stamp(before)) !== canonicalJson(stamp(fstatSync(fd))) || canonicalJson(stamp(before)) !== canonicalJson(stamp(lstatSync(path)))) refuse("source_stale", "file changed while hashing");
    return hash.digest("hex");
  } finally { closeSync(fd); }
}

function command(root: string, argv: string[]): string {
  const out = spawnSync(argv[0]!, argv.slice(1), { cwd: root, encoding: "utf8", timeout: 15_000, maxBuffer: 64 * 1024 * 1024, env: { ...process.env, LC_ALL: "C", GIT_OPTIONAL_LOCKS: "0" } });
  if (out.status !== 0) throw new Error(`identity_command_failed:${argv[0]}:${out.stderr.trim()}`);
  return out.stdout;
}

export function observeArtifactSource(root: string, registeredInputs: readonly string[] = []): SourceIdentity {
  const sourceRoot = realpathSync(root), ownerChild = "scripts/agentic/compiler-input-paths.txt";
  type Observation = { child: string; stat: Stats | null; identity: string; link?: string };
  const contents: unknown[] = [], observations = new Map<string, Observation>();
  const seen = new Set<string>(), activeDirectories = new Set<string>(), absentTargetParents = new Set<string>();
  const identity = (child: string, stat: Stats | null): string => canonicalJson(stat ? child ? stamp(stat) : [stat.dev, stat.ino, stat.mode] : null);
  const inspect = (child: string): Observation => {
    let stat: Stats | null = null;
    try { stat = lstatSync(join(sourceRoot, child)); }
    catch (error) { if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error; }
    const current = identity(child, stat), previous = observations.get(child);
    if (previous && previous.identity !== current) refuse("source_stale", "compiler input changed during observation");
    const link = stat?.isSymbolicLink() ? readlinkSync(join(sourceRoot, child)) : undefined;
    if (previous && previous.link !== link) refuse("source_stale", "compiler input link changed during observation");
    if (previous) return previous;
    if (observations.size >= 250_000) refuse("source_stale", "compiler input inventory limit");
    const observation = { child, stat, identity: current, link };
    observations.set(child, observation);
    return observation;
  };
  const parentOf = (child: string): string => child.includes("/") ? child.slice(0, child.lastIndexOf("/")) : "";
  const step = (parts: string[], part: string): void => {
    if (part === "..") {
      if (!parts.length) refuse("unsafe_artifact_path", "compiler input link escapes repository");
      parts.pop();
    } else if (part && part !== ".") parts.push(part);
  };
  // Resolve each component before processing '..': lexical normalization can hide
  // an escaping intermediate link or incorrectly turn a dangling link into bytes.
  const resolveParts = (base: string[], parts: string[], links: Set<string>): Observation => {
    let node = inspect(base.join("/"));
    for (let index = 0; index < parts.length; index++) {
      const part = parts[index]!;
      if (!part) continue;
      if (!node.stat) {
        if (links.size) absentTargetParents.add(parentOf(node.child));
        const unresolved = [...base, ...parts.slice(index)], normalized: string[] = [];
        for (const rest of unresolved) step(normalized, rest);
        return { ...node, child: unresolved.join("/") };
      }
      if (!node.stat.isDirectory()) refuse("unsafe_artifact_path", "compiler input has a non-directory ancestor");
      step(base, part);
      node = inspect(base.join("/"));
      if (node.link !== undefined) {
        if (links.has(node.child) || links.size >= 40) refuse("unsafe_artifact_path", "compiler input symlink cycle");
        if (!node.link || isAbsolute(node.link) || node.link.includes("\\") || node.link.includes("\0")) refuse("unsafe_artifact_path", "compiler input link must be repository-relative");
        if (!seen.has(node.child)) { seen.add(node.child); contents.push([node.child, "symlink", node.link]); }
        links.add(node.child);
        const target = resolveParts(base.slice(0, -1), node.link.split("/"), links);
        if (!target.stat) absentTargetParents.add(parentOf(target.child));
        links.delete(node.child);
        node = target;
        base = node.child ? node.child.split("/") : [];
      }
    }
    if (!node.stat && links.size) absentTargetParents.add(parentOf(node.child));
    return node;
  };
  const resolveSource = (child: string): Observation => resolveParts([], child ? child.split("/") : [], new Set());
  let buffer: Buffer | undefined;
  const readSourceFile = (node: Observation, collect = false): { sha256: string; text?: string } => {
    const fd = openSync(join(sourceRoot, node.child), constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
    try {
      const opened = fstatSync(fd);
      if (!opened.isFile() || identity(node.child, opened) !== node.identity) refuse("source_stale", "compiler input replaced before hashing");
      let sha256: string, text: string | undefined;
      if (collect) {
        const bytes = readFileSync(fd); sha256 = artifactHash(bytes); text = bytes.toString("utf8");
      } else {
        const hash = createHash("sha256"); buffer ??= Buffer.allocUnsafe(1024 * 1024);
        let count: number;
        while ((count = readSync(fd, buffer, 0, buffer.length, null))) hash.update(buffer.subarray(0, count));
        sha256 = hash.digest("hex");
      }
      if (identity(node.child, fstatSync(fd)) !== node.identity || resolveSource(node.child).identity !== node.identity) refuse("source_stale", "compiler input changed while hashing");
      return { sha256, text };
    } finally { closeSync(fd); }
  };
  const owner = resolveSource(ownerChild);
  if (!owner.stat?.isFile()) refuse("source_stale", "compiler input inventory unavailable");
  const ownerContent = readSourceFile(owner, true);
  const inputPaths = [...new Set([...ownerContent.text!.trim().split("\n"), ownerChild, ...registeredInputs])].sort();
  const visit = (child: string): void => {
    const node = resolveSource(child);
    child = node.child;
    if (activeDirectories.has(child)) refuse("unsafe_artifact_path", "compiler input directory cycle");
    if (seen.has(child)) return;
    seen.add(child);
    if (!node.stat) { contents.push([child, "absent"]); return; }
    if (node.stat.isDirectory()) {
      if (!child) refuse("unsafe_artifact_path", "compiler input directory cycle");
      for (const parent of activeDirectories) if (parent.startsWith(`${child}/`)) refuse("unsafe_artifact_path", "compiler input directory cycle");
      contents.push([child, "directory"]); activeDirectories.add(child);
      for (const name of readdirSync(join(sourceRoot, child)).sort()) visit(`${child}/${name}`);
      activeDirectories.delete(child);
    } else if (node.stat.isFile()) contents.push([child, node.stat.mode & 0o111, node.stat.size, child === owner.child ? ownerContent.sha256 : readSourceFile(node).sha256]);
    else refuse("unsafe_artifact_path", "compiler input is not a regular file/directory/link");
  };
  for (const child of inputPaths) {
    if (!child || isAbsolute(child) || child.includes("\\") || child.includes("\0") || child.split("/").some(part => !part || part === "." || part === "..")) refuse("unsafe_artifact_path", "noncanonical compiler input path");
    visit(child);
  }
  const gitHead = command(root, ["git", "rev-parse", "HEAD"]).trim();
  if (!/^[a-f0-9]{40,64}$/.test(gitHead)) refuse("source_stale", "source head unavailable");
  const committedInputTreeSha256 = artifactHash(command(root, ["git", "ls-tree", "-r", gitHead, "--", ...inputPaths]));
  const repositoryDirty = command(root, ["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"]).length > 0;
  const compilerDirty = command(root, ["git", "status", "--porcelain=v1", "-z", "--untracked-files=all", "--", ...inputPaths]).length > 0;
  for (const [child, before] of observations) {
    // Check ancestors again before touching the leaf; never accept a pathname
    // redirected between resolution and hashing, even if its bytes match.
    if (child && resolveSource(parentOf(child)).child !== parentOf(child)) refuse("source_stale", "compiler input ancestor changed");
    inspect(child);
    if (absentTargetParents.has(child) && before.stat && canonicalJson(stamp(before.stat)) !== canonicalJson(stamp(lstatSync(join(sourceRoot, child))))) refuse("source_stale", "absent compiler input target changed during observation");
  }
  const boundary = fingerprint([...observations].map(([child, node]) => [child, absentTargetParents.has(child) && node.stat ? stamp(node.stat) : JSON.parse(node.identity)]));
  return { algorithm: "reviewed-worktree-content-v1", compilerInputSha256: fingerprint(contents),
    compilerInputOwnerSha256: ownerContent.sha256, committedInputTreeSha256, gitHead, repositoryDirty, compilerDirty,
    inputPaths, changeBoundaryBeforeSha256: boundary, changeBoundaryAfterSha256: boundary, hermeticBuild: false };
}

export function assertSourceBoundary(before: SourceIdentity, after: SourceIdentity): void {
  if (["compilerInputSha256", "compilerInputOwnerSha256", "gitHead", "committedInputTreeSha256", "changeBoundaryBeforeSha256"].some(key => before[key as keyof SourceIdentity] !== after[key as keyof SourceIdentity])) refuse("source_stale", "source changed during build/publication");
}

export function observeToolchain(root: string): ArtifactManifestV3["toolchain"] & { rustcPath: string; cargoPath: string; host: string } {
  const pin = Bun.TOML.parse(readFileSync(join(root, "rust-toolchain.toml"), "utf8")) as { toolchain: { channel: string } };
  const channel = pin.toolchain.channel;
  if (typeof channel !== "string" || !/^\d+\.\d+\.\d+$/.test(channel)) refuse("toolchain_mismatch", "root toolchain must pin an exact stable release");
  const rustcPath = realpathSync(command(root, ["rustup", "which", "--toolchain", channel, "rustc"]).trim());
  const cargoPath = realpathSync(command(root, ["rustup", "which", "--toolchain", channel, "cargo"]).trim());
  if (process.env.RUSTC && realpathSync(process.env.RUSTC) !== rustcPath) refuse("toolchain_mismatch", "RUSTC differs from pinned compiler");
  const rustcVerboseVersion = command(root, [rustcPath, "-vV"]);
  const cargoVersion = command(root, [cargoPath, "-V"]);
  const host = /^host: (.+)$/m.exec(rustcVerboseVersion)?.[1];
  if (!host || !rustcVerboseVersion.includes(`release: ${channel}\n`) || !cargoVersion.startsWith(`cargo ${channel} `)) refuse("toolchain_mismatch", "installed toolchain differs from root pin");
  if (process.env.RUSTUP_TOOLCHAIN && ![channel, `${channel}-${host}`].includes(process.env.RUSTUP_TOOLCHAIN)) refuse("toolchain_mismatch", "root toolchain override differs");
  return { channel, rustcVerboseVersion, cargoVersion, rustcSha256: hashArtifactFile(rustcPath), cargoSha256: hashArtifactFile(cargoPath), rustcPath, cargoPath, host };
}

export interface CompilerWrapperIdentity { readonly path: string; readonly sha256: Sha256; }
export interface CompilerCompatibility {
  readonly environment: Readonly<Record<string, string | null>>;
  readonly configs: readonly [string, string][];
  readonly compilerWrappers: { readonly rustc: CompilerWrapperIdentity | null; readonly workspace: CompilerWrapperIdentity | null };
  readonly target: string; readonly platform: string; readonly architecture: string;
}
function compilerWrapperIdentity(root: string, wrapper: string): CompilerWrapperIdentity | null {
  if (!wrapper) return null;
  try {
    const resolved = wrapper.includes("/") ? resolve(root, wrapper) : Bun.which(wrapper, { cwd: root });
    if (!resolved) refuse("configuration_stale", "configured compiler wrapper is unavailable");
    const path = realpathSync(resolved), stat = lstatSync(path);
    if (!stat.isFile() || !(stat.mode & 0o111)) refuse("configuration_stale", "configured compiler wrapper is not executable");
    return { path, sha256: hashArtifactFile(path) };
  } catch (error) {
    if (error instanceof ArtifactVerificationError) throw error;
    refuse("configuration_stale", `configured compiler wrapper cannot be qualified: ${String(error)}`);
  }
}

const agentIncrementalEnvironment = {
  CARGO_INCREMENTAL: "0", CARGO_BUILD_INCREMENTAL: "false",
  CARGO_PROFILE_DEV_INCREMENTAL: "false", CARGO_PROFILE_TEST_INCREMENTAL: "false",
} as const;

function cargoCfgMatches(expression: string, values: ReadonlySet<string>): boolean {
  const tokens = expression.match(/"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[(),=]/g) ?? [];
  if (tokens.join("") !== expression.replace(/\s+(?=(?:[^"]*"[^"]*")*[^"]*$)/g, "")) refuse("configuration_stale", "unqualified Cargo target cfg expression");
  let index = 0;
  const parse = (): boolean => {
    const name = tokens[index++];
    if (!name) refuse("configuration_stale", "invalid Cargo target cfg expression");
    if (tokens[index] === "(") {
      index++;
      const children: boolean[] = [];
      while (tokens[index] !== ")") {
        children.push(parse());
        if (tokens[index] !== ",") break;
        index++;
      }
      if (tokens[index++] !== ")") refuse("configuration_stale", "invalid Cargo cfg delimiter");
      if (name === "all") return children.every(Boolean);
      if (name === "any") return children.some(Boolean);
      if ((name === "not" || name === "cfg") && children.length === 1) return name === "not" ? !children[0] : children[0]!;
      refuse("configuration_stale", "unsupported Cargo cfg predicate");
    }
    if (tokens[index] === "=") { index++; const value = tokens[index++]; if (!value?.startsWith('"')) refuse("configuration_stale", "invalid Cargo cfg value"); return values.has(`${name}=${value}`); }
    return values.has(name);
  };
  const matches = parse();
  if (index !== tokens.length) refuse("configuration_stale", "trailing Cargo cfg expression");
  return matches;
}

const compilerIdentityEnvironmentPattern = /^(CARGO_PROFILE_|CARGO_TARGET_.*_(LINKER|RUSTFLAGS)$|ORT_|WHISPER_|GGML_|PKG_CONFIG_)/;
const compilerIdentityEnvironmentNames: Readonly<Record<string, true>> = {
  RUSTFLAGS: true, CARGO_ENCODED_RUSTFLAGS: true, CARGO_BUILD_RUSTFLAGS: true,
  RUSTDOCFLAGS: true, CARGO_ENCODED_RUSTDOCFLAGS: true, CARGO_BUILD_RUSTDOCFLAGS: true,
  CARGO_INCREMENTAL: true, CARGO_BUILD_INCREMENTAL: true, CC: true, CXX: true, AR: true,
  CFLAGS: true, CXXFLAGS: true, LDFLAGS: true, SDKROOT: true, DEVELOPER_DIR: true,
  MACOSX_DEPLOYMENT_TARGET: true, LIBCLANG_PATH: true, SOURCE_DATE_EPOCH: true,
  GITHUB_SHA: true, SCRIPT_KIT_TRACK_GIT_HEAD: true,
};

/** The same compiler inputs must be fingerprinted and retained by isolated artifact verifiers. */
export function isCompilerIdentityEnvironmentVariable(name: string): boolean {
  return compilerIdentityEnvironmentPattern.test(name) || compilerIdentityEnvironmentNames[name] === true;
}

/** Cargo selects legacy config over config.toml at each level, then merges from home to cwd. */
export function normalizeCompilerPolicy(root: string, args: readonly string[] = []) {
  const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
  Object.assign(env, agentIncrementalEnvironment);
  env.CARGO_PROFILE_DEV_DEBUG ||= "line-tables-only";
  const configs: Array<[string, string]> = [];
  const select = (directory: string) => {
    const path = [join(directory, "config"), join(directory, "config.toml")].find(existsSync);
    if (path && !configs.some(([existing]) => existing === path)) configs.push([path, hashArtifactFile(path)]);
  };
  let directory = realpathSync(root);
  while (true) {
    select(join(directory, ".cargo"));
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  select(resolve(env.CARGO_HOME || join(homedir(), ".cargo")));
  interface CargoConfiguration extends Record<string, unknown> {
    build?: Record<string, unknown>; resolver?: Record<string, unknown>;
    env?: Record<string, string | { value: string; force?: boolean; relative?: boolean }>;
    target?: Record<string, { rustflags?: string | string[] }>;
  }
  type Config = Record<string, unknown>;
  const merged: CargoConfiguration = {}, configured: Record<string, { value: string; root: string }> = {}, configuredStorage: Record<string, string> = {}, envRoots: Record<string, string> = {};
  const merge = (target: Config, source: Config): void => {
    for (const [key, value] of Object.entries(source)) {
      if (Array.isArray(value)) { const previous = target[key]; target[key] = [...(Array.isArray(previous) ? previous : []), ...value]; }
      else if (value && typeof value === "object") {
        const previous = target[key];
        if (!previous || typeof previous !== "object" || Array.isArray(previous)) target[key] = {};
        merge(target[key] as Config, value as Config);
      }
      else target[key] = value;
    }
  };
  for (const [path] of [...configs].reverse()) {
    const config = Bun.TOML.parse(readFileSync(path, "utf8")) as CargoConfiguration;
    merge(merged, config);
    for (const key of ["rustc-wrapper", "rustc-workspace-wrapper"]) {
      const value = config.build?.[key];
      if (value !== undefined && typeof value !== "string") refuse("configuration_stale", "compiler wrapper configuration must be a string");
      if (typeof value === "string") configured[key] = { value, root: dirname(dirname(path)) };
    }
    for (const key of ["target-dir", "build-dir"]) {
      const value = config.build?.[key];
      if (value !== undefined && typeof value !== "string") refuse("configuration_stale", `invalid Cargo build.${key}`);
      if (typeof value === "string") configuredStorage[key] = resolve(dirname(dirname(path)), value);
    }
    for (const name of Object.keys(config.env ?? {})) envRoots[name] = dirname(dirname(path));
  }
  for (const [key, value] of Object.entries(configuredStorage)) if (value !== join(root, "target-agent/pools/agent-debug")) refuse("configuration_stale", `Cargo build.${key} escapes protected storage`);
  if (merged.resolver?.["lockfile-path"] !== undefined) refuse("configuration_stale", "Cargo lockfile relocation is unqualified");
  const wrappers = ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"];
  const storage = ["CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR", "CARGO_RESOLVER_LOCKFILE_PATH", "SCRIPT_KIT_METAL_MODULE_CACHE_DIR", "CLANG_MODULE_CACHE_PATH"];
  for (const [name, setting] of Object.entries(merged.env ?? {})) {
    if (wrappers.includes(name)) refuse("configuration_stale", "compiler wrappers in Cargo [env] are unqualified; use the declared build wrapper or environment");
    const entry = typeof setting === "string" ? { value: setting } : setting as { value: string; force?: boolean; relative?: boolean };
    if (!entry || typeof entry.value !== "string") refuse("configuration_stale", "invalid Cargo environment configuration");
    if (name in agentIncrementalEnvironment && entry.force && entry.value !== env[name]) refuse("configuration_stale", `forced Cargo [env] conflicts with agent policy: ${name}`);
    if (entry.force && ["RUSTC", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_JOBS", "RUST_TEST_THREADS", "CMAKE_BUILD_PARALLEL_LEVEL", "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS"].includes(name) && entry.value !== env[name]) refuse("configuration_stale", `forced Cargo [env] conflicts with compiler ownership: ${name}`);
    if (storage.includes(name) && (entry.force || env[name] === undefined)) refuse("configuration_stale", `Cargo [env] storage relocation is unqualified: ${name}`);
    if (entry.force || env[name] === undefined) {
      env[name] = entry.relative ? resolve(envRoots[name]!, entry.value) : entry.value;
    }
  }
  Object.assign(env, agentIncrementalEnvironment);
  for (const name of ["CARGO_BUILD_BUILD_DIR", "CARGO_RESOLVER_LOCKFILE_PATH"]) if (env[name]) refuse("configuration_stale", `Cargo storage relocation is unqualified: ${name}`);
  const pool = env.SCRIPT_KIT_AGENT_REGISTERED_TASK === "pi-sidecar" ? join(root, "target/pi-sidecar/cache/cargo-target") : join(root, "target-agent/pools/agent-debug");
  for (const name of ["CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR"]) if (env[name] && resolve(root, env[name]) !== pool) refuse("configuration_stale", `Cargo storage relocation escapes protected destination: ${name}`);
  for (const name of ["SCRIPT_KIT_METAL_MODULE_CACHE_DIR", "CLANG_MODULE_CACHE_PATH"]) if (env[name] && !resolve(root, env[name]).startsWith(join(root, "target-agent/shared") + sep)) refuse("configuration_stale", `shader cache relocation escapes protected storage: ${name}`);
  const separator = args.indexOf("--"), cargoArgs = separator < 0 ? args : args.slice(0, separator);
  let profile: CargoTargetIdentity["requestedProfile"] = args[0] === "test" ? "test" : "dev";
  for (let index = 1; index < cargoArgs.length; index++) {
    const arg = cargoArgs[index]!;
    const value = arg === "--profile" ? cargoArgs[++index] : arg.startsWith("--profile=") ? arg.slice(10) : ["--release", "-r"].includes(arg) ? "release" : undefined;
    if (arg === "--profile" || arg.startsWith("--profile=") || ["--release", "-r"].includes(arg)) {
      if (value !== "dev" && value !== "test" && value !== "release") refuse("configuration_stale", "unqualified_profile");
      profile = value;
    }
  }
  const checkFlags = (flags: unknown): void => {
    const text = Array.isArray(flags) ? flags.join(" ") : String(flags ?? "").replaceAll("\u001f", " ");
    if (/(?:^|\s)(?:-C\s*=?\s*|--codegen(?:=|\s+))incremental(?:=|\s|$)/.test(text)) refuse("configuration_stale", "explicit rustc incremental flags conflict with agent policy");
  };
  if (env.CARGO_ENCODED_RUSTFLAGS !== undefined) checkFlags(env.CARGO_ENCODED_RUSTFLAGS);
  else if (env.RUSTFLAGS !== undefined) checkFlags(env.RUSTFLAGS);
  else {
    const targetEntries = Object.entries(merged.target ?? {});
    const hasTargetEnvironment = Object.keys(env).some(name => /^CARGO_TARGET_.*_RUSTFLAGS$/.test(name));
    let targetFlags = false;
    if (hasTargetEnvironment || targetEntries.some(([, value]) => value.rustflags !== undefined)) {
      const toolchain = observeToolchain(root);
      const targetIndex = cargoArgs.indexOf("--target");
      const selectedTarget = (targetIndex < 0 ? cargoArgs.find(arg => arg.startsWith("--target="))?.slice(9) : cargoArgs[targetIndex + 1]) ?? env.CARGO_BUILD_TARGET ?? merged.build?.target ?? toolchain.host;
      if (typeof selectedTarget !== "string" || !selectedTarget || selectedTarget.endsWith(".json")) refuse("configuration_stale", "unqualified compiler target");
      const targetEnvironment = env[`CARGO_TARGET_${selectedTarget.toUpperCase().replaceAll("-", "_")}_RUSTFLAGS`];
      const tripleFlags = targetEnvironment ?? merged.target?.[selectedTarget]?.rustflags;
      if (tripleFlags !== undefined) { targetFlags = true; checkFlags(tripleFlags); }
      const cfgEntries = targetEntries.filter(([name, value]) => name.startsWith("cfg(") && value.rustflags !== undefined);
      if (cfgEntries.length) {
        const cfg = new Set(command(root, [toolchain.rustcPath, "--print", "cfg", "--target", selectedTarget]).trim().split("\n"));
        if (![...cfg].some(value => value.startsWith("target_arch="))) refuse("configuration_stale", "compiler target cfg observation unavailable");
        for (const [name, value] of cfgEntries) if (cargoCfgMatches(name, cfg)) { targetFlags = true; checkFlags(value.rustflags); }
      }
    }
    if (!targetFlags) checkFlags(env.CARGO_BUILD_RUSTFLAGS ?? merged.build?.rustflags);
  }
  checkFlags(env.CARGO_ENCODED_RUSTDOCFLAGS ?? env.RUSTDOCFLAGS ?? env.CARGO_BUILD_RUSTDOCFLAGS ?? merged.build?.rustdocflags);
  if (["rustc", "rustdoc"].includes(args[0]!) && separator >= 0) checkFlags(args.slice(separator + 1));
  const environment: Record<string, string | null> = {};
  for (const name of Object.keys(env).sort()) {
    if (isCompilerIdentityEnvironmentVariable(name)) environment[name] = artifactHash(env[name]!);
  }
  const wrapper = (key: string, name: string, cargoName: string): CompilerWrapperIdentity | null => {
    const value = env[name] ?? env[cargoName];
    const selected = value !== undefined ? { value, root } : configured[key];
    return selected ? compilerWrapperIdentity(selected.root, selected.value) : null;
  };
  const configuredTarget = env.CARGO_BUILD_TARGET ?? merged.build?.target ?? "host";
  if (typeof configuredTarget !== "string") refuse("configuration_stale", "multiple build targets are unqualified");
  const targetIndex = cargoArgs.indexOf("--target");
  const requestedTarget = (targetIndex < 0 ? cargoArgs.find(arg => arg.startsWith("--target="))?.slice(9) : cargoArgs[targetIndex + 1]) ?? configuredTarget;
  if (typeof requestedTarget !== "string" || (requestedTarget !== "host" && !/^[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+)+$/.test(requestedTarget))) refuse("configuration_stale", "unqualified compiler target");
  const compatibility: CompilerCompatibility = { environment, configs, compilerWrappers: {
    rustc: wrapper("rustc-wrapper", "RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"),
    workspace: wrapper("rustc-workspace-wrapper", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"),
  }, target: configuredTarget, platform: process.platform, architecture: process.arch };
  return { env, compatibility, profile, requestedTarget, incremental: { enabled: false, owner: "agent-cargo", environment: agentIncrementalEnvironment } };
}

export function compilerCompatibility(root: string, args: readonly string[] = []): CompilerCompatibility { return normalizeCompilerPolicy(root, args).compatibility; }

function selectCompilerCache(root: string, rustcPath: string, policy: { env: Record<string, string>; compatibility: CompilerCompatibility }) {
  const env = policy.env, required = env.SCRIPT_KIT_AGENT_USE_SCCACHE === "1";
  const chain = policy.compatibility.compilerWrappers;
  if (chain.rustc || chain.workspace) {
    if (required) throw Object.assign(new Error("required sccache conflicts with external semantic compiler wrapper"), { exitCode: 69 });
    if (chain.rustc) env.RUSTC_WRAPPER = chain.rustc.path;
    if (chain.workspace) env.RUSTC_WORKSPACE_WRAPPER = chain.workspace.path;
    return { backend: "external", probeStatus: "not-probed-semantic-wrapper", measuredHits: null, measuredMisses: null };
  }
  if (env.SCRIPT_KIT_AGENT_USE_SCCACHE === "0") return { backend: "disabled", probeStatus: "not-requested", measuredHits: null, measuredMisses: null };
  const cache = Bun.which("sccache", { PATH: env.PATH, cwd: root });
  if (cache) {
    const shared = join(root, "target-agent/shared");
    env.SCCACHE_DIR ||= join(shared, "sccache"); env.SCCACHE_CACHE_SIZE ||= "10G";
    env.SCCACHE_BASEDIRS ||= root; env.SCCACHE_SERVER_UDS ||= join(shared, "sccache.sock");
    const probe = (args: string[]) => spawnSync(cache, args, { cwd: root, env, timeout: 10_000, stdio: "ignore" }).status === 0;
    if (probe(["--show-stats"]) && probe([rustcPath, "-vV"])) { env.RUSTC_WRAPPER = cache; return { backend: "sccache", probeStatus: "pinned-rustc-succeeded", measuredHits: null, measuredMisses: null }; }
  }
  const message = cache ? "sccache cannot execute rustc in this sandbox; use approved sandbox permissions or SCRIPT_KIT_AGENT_USE_SCCACHE=1" : "sccache is unavailable";
  if (required) throw Object.assign(new Error(`required ${message}`), { exitCode: 69 });
  process.stderr.write(`AGENT_CARGO warning: ${message}; continuing without compiler caching\n`);
  return { backend: cache ? "unavailable" : "disabled", probeStatus: cache ? "pinned-rustc-failed" : "unavailable", measuredHits: null, measuredMisses: null };
}

function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object" && !Object.isFrozen(value)) { Object.freeze(value); for (const child of Object.values(value)) deepFreeze(child); }
  return value;
}

export function verifyImmutableArtifact(repositoryRoot: string, reference: ArtifactReference, expected: ArtifactExpectation): VerifiedArtifact {
  const verificationKey = `${repositoryRoot}:${reference?.manifestPath}:${reference?.manifestSha256}`;
  if (verificationStack.has(verificationKey) || verificationStack.size >= 8) refuse("manifest_invalid", "cyclic or excessive artifact derivation");
  verificationStack.add(verificationKey);
  try {
    const root = realpathSync(repositoryRoot);
    if (!reference || !/^target-agent\/artifacts\/[A-Za-z0-9][A-Za-z0-9._-]*\/manifest\.json$/.test(reference.manifestPath) || !isHash(reference.manifestSha256)) refuse("unsafe_artifact_path", "explicit canonical artifact reference required");
    const manifestPath = canonicalArtifactPath(root, reference.manifestPath);
    if (hashArtifactFile(manifestPath) !== reference.manifestSha256) refuse("manifest_hash_mismatch", "manifest bytes differ from supplied reference");
    const manifest = readOwnedJson(manifestPath) as unknown as ArtifactManifestV3;
    const m = manifest;
    if (m.schemaVersion !== 3 || m.artifactId !== basename(dirname(manifestPath)) || m.artifactKind !== expected.kind
      || m.target?.packageName !== expected.packageName || m.target?.targetName !== expected.targetName
      || (expected.profile && m.target.requestedProfile !== expected.profile)
      || !["dev", "test", "release"].includes(m.target.requestedProfile)
      || !Array.isArray(m.target.targetKind) || !Array.isArray(m.target.crateTypes) || !Array.isArray(m.target.features)
      || !m.target.packageId || !m.target.sourcePath || !m.target.targetTriple
      || m.source?.algorithm !== "reviewed-worktree-content-v1" || m.source.hermeticBuild !== false
      || !Array.isArray(m.source.inputPaths) || !m.source.inputPaths.length
      || typeof m.source.compilerDirty !== "boolean" || typeof m.source.repositoryDirty !== "boolean"
      || !/^[a-f0-9]{40,64}$/.test(m.source.gitHead)
      || ![m.binarySha256, m.requestedPolicySha256, m.effectiveConfigurationSha256, m.source.compilerInputSha256, m.source.compilerInputOwnerSha256, m.source.committedInputTreeSha256, m.source.changeBoundaryBeforeSha256, m.source.changeBoundaryAfterSha256, m.toolchain?.rustcSha256, m.toolchain?.cargoSha256].every(isHash)
      || m.source.changeBoundaryBeforeSha256 !== m.source.changeBoundaryAfterSha256
      || !Number.isSafeInteger(m.sizeBytes) || m.sizeBytes <= 0 || typeof m.requiresExactGitHead !== "boolean"
      || m.publication?.owner !== "scripts/agentic/agent-cargo.sh" || m.publication.pool !== "agent-debug"
      || m.publication.immutable !== true || m.publication.exportedWhileLeaseHeld !== true || !m.publication.leaseGeneration
      || m.publication.buildTask?.kind !== "build-job") refuse("manifest_invalid", "manifest contract mismatch");
    if ((m.artifactKind === "rust-libtest" && (!m.target.targetKind.includes("lib") || m.target.cargoProfile.test !== true))
      || (m.artifactKind !== "rust-libtest" && !m.target.targetKind.includes("bin"))) refuse("manifest_invalid", "Cargo target selection mismatch");
    if (fingerprint(m.effectiveConfiguration) !== m.effectiveConfigurationSha256 || fingerprint(m.effectiveConfiguration.requestedPolicy) !== m.requestedPolicySha256) refuse("manifest_invalid", "recorded configuration hash mismatch");
    const executablePath = canonicalArtifactPath(root, m.binaryPath);
    if (!executablePath.startsWith(dirname(manifestPath) + sep)) refuse("unsafe_artifact_path", "executable outside immutable artifact");
    const binaryStat = lstatSync(executablePath), manifestStat = lstatSync(manifestPath), dirStat = lstatSync(dirname(manifestPath));
    if (!binaryStat.isFile() || binaryStat.nlink !== 1 || !(binaryStat.mode & 0o111) || (binaryStat.mode & 0o222)
      || (manifestStat.mode & 0o222) || manifestStat.nlink !== 1 || (dirStat.mode & 0o222)) refuse("unsafe_artifact_path", "publication is writable, linked, or nonexecutable");
    if (binaryStat.size !== m.sizeBytes || hashArtifactFile(executablePath) !== m.binarySha256) refuse("binary_hash_mismatch", "binary size/hash differs");
    try {
      const task = readManagedTask(managedTaskRecordPath(root, m.publication.buildTask), m.publication.buildTask);
      const refs = task.result.artifacts;
      if (task.state !== "closed" || !task.cleanup.closed || task.result.status !== "succeeded"
        || task.identity.revision <= m.publication.buildTask.revision || !Array.isArray(refs)
        || !refs.some(ref => canonicalJson(ref) === canonicalJson(reference))
        || fingerprint(task.source) !== fingerprint(m.source)
        || fingerprint(task.effectiveConfiguration) !== m.effectiveConfigurationSha256) refuse("publication_not_finalized", "publication task not successfully finalized");
    } catch (error) { if (error instanceof ArtifactVerificationError) throw error; refuse("publication_not_finalized", "matching build task unavailable"); }
    if (m.derivation) {
      if (m.derivation.transformation !== "signed-and-stapled-bundle" || !isHash(m.derivation.attestationSha256) || !isHash(m.derivation.bundleTreeSha256)) refuse("manifest_invalid", "invalid signed bundle derivation");
      const input = verifyImmutableArtifact(root, m.derivation.input, { ...expected, sourcePolicy: "recorded-content" });
      const bundle = join(dirname(manifestPath), "Script Kit.app");
      if (input.manifest.derivation || bundleTreeHash(bundle, true) !== m.derivation.bundleTreeSha256 || input.manifest.source.compilerInputSha256 !== m.source.compilerInputSha256
        || unsignedMachOPayloadSha256(input.executablePath) !== unsignedMachOPayloadSha256(executablePath)) refuse("binary_hash_mismatch", "signed bundle/input identity differs");
      const attestation = join(dirname(manifestPath), "release-attestation.json");
      if (hashArtifactFile(attestation) !== m.derivation.attestationSha256) refuse("manifest_hash_mismatch", "release attestation differs");
    }
    if (expected.sourcePolicy !== "recorded-content") {
      const current = observeArtifactSource(root);
      if (current.compilerInputSha256 !== m.source.compilerInputSha256 || current.compilerInputOwnerSha256 !== m.source.compilerInputOwnerSha256) refuse("source_stale", "reviewed worktree bytes differ");
      if ((m.requiresExactGitHead || expected.sourcePolicy === "clean-exact-head") && (current.gitHead !== m.source.gitHead || current.repositoryDirty || m.source.repositoryDirty)) refuse("source_stale", "clean exact source head required");
      const requested = m.effectiveConfiguration.requestedPolicy as Record<string, unknown>;
      const args = requested.args === undefined ? [] : requested.args;
      if (!Array.isArray(args) || !args.every(value => typeof value === "string")) refuse("manifest_invalid", "invalid recorded compiler arguments");
      if (fingerprint(compilerCompatibility(root, args)) !== fingerprint(m.effectiveConfiguration.compatibility)) refuse("configuration_stale", "current compiler configuration differs");
      const currentToolchain = observeToolchain(root);
      if (["channel", "rustcVerboseVersion", "cargoVersion", "rustcSha256", "cargoSha256"].some(key => currentToolchain[key as keyof typeof currentToolchain] !== m.toolchain[key as keyof typeof m.toolchain])) refuse("toolchain_mismatch", "actual pinned compiler bytes differ");
    }
    const verified = deepFreeze({ reference: { ...reference }, manifest, executablePath,
      binary: { path: m.binaryPath, sha256: m.binarySha256, sizeBytes: m.sizeBytes, sourceCommit: m.source.gitHead, sourceDirty: m.source.repositoryDirty, manifestPath: reference.manifestPath, manifestSha256: reference.manifestSha256 } });
    verifiedArtifacts.add(verified);
    return verified;
  } catch (error) {
    if (error instanceof ArtifactVerificationError) throw error;
    refuse("manifest_invalid", `artifact validation failed: ${String(error)}`);
  } finally { verificationStack.delete(verificationKey); }
}

export function bundleTreeHash(root: string, requireReadOnly = false): Sha256 {
  const entries: unknown[] = [];
  const canonicalRoot = realpathSync(root);
  const visit = (path: string): void => {
    const stat = lstatSync(path);
    if (stat.isSymbolicLink()) {
      const target = readlinkSync(path), resolved = realpathSync(path);
      if (isAbsolute(target) || !resolved.startsWith(canonicalRoot + sep)) refuse("unsafe_artifact_path", "bundle symlink escapes immutable tree");
      entries.push([relative(root, path), "symlink", target]);
      return;
    }
    if (requireReadOnly && (stat.mode & 0o222)) refuse("unsafe_artifact_path", "mutable entry in published bundle");
    if (stat.isDirectory()) { for (const name of readdirSync(path).sort()) visit(join(path, name)); }
    else if (stat.isFile() && stat.nlink === 1) entries.push([relative(root, path), Boolean(stat.mode & 0o111), stat.size, hashArtifactFile(path)]);
    else refuse("unsafe_artifact_path", "bundle hardlink or special file");
  };
  if (process.platform === "darwin") {
    const program = `import ctypes,json,os,sys
libc=ctypes.CDLL(None,use_errno=True)
libc.listxattr.argtypes=[ctypes.c_char_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_int]
libc.listxattr.restype=ctypes.c_ssize_t
libc.getxattr.argtypes=[ctypes.c_char_p,ctypes.c_char_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_uint32,ctypes.c_int]
libc.getxattr.restype=ctypes.c_ssize_t
def checked(count):
    if count<0: raise OSError(ctypes.get_errno(),os.strerror(ctypes.get_errno()))
    if count>32*1024*1024: raise ValueError('extended attribute observation limit')
    return count
def attribute_bytes(path,name=None):
    def invoke(buffer,size):
        return checked(libc.listxattr(path,buffer,size,1) if name is None else libc.getxattr(path,name,buffer,size,0,1))
    size=invoke(None,0)
    buffer=ctypes.create_string_buffer(max(1,size))
    if invoke(buffer,size)!=size: raise ValueError('extended attributes changed during observation')
    return buffer.raw[:size]
def stamp(path):
    stat=os.lstat(path)
    return (stat.st_dev,stat.st_ino,stat.st_mode,stat.st_size,stat.st_mtime_ns,stat.st_ctime_ns)
root=os.fsencode(sys.argv[1]); paths={root}; entries=[]
def walk_error(error): raise error
for parent,dirs,files in os.walk(root,followlinks=False,onerror=walk_error):
    paths.update(os.path.join(parent,name) for name in dirs+files)
    if len(paths)>250000: raise ValueError('bundle inventory limit')
for path in sorted(paths):
    before=stamp(path)
    names=attribute_bytes(path)
    if names and not names.endswith(b'\\0'): raise ValueError('invalid extended attribute names')
    for name in sorted(names[:-1].split(b'\\0') if names else []):
        entries.append((os.fsdecode(os.path.relpath(path,root)),os.fsdecode(name),attribute_bytes(path,name).hex()))
    if before!=stamp(path) or names!=attribute_bytes(path): raise ValueError('extended attributes changed during observation')
print(json.dumps(entries,separators=(',',':')))`;
    const attributes = spawnSync("python3", ["-B", "-c", program, root], { encoding: "utf8", timeout: 30_000, maxBuffer: 32 * 1024 * 1024 });
    if (attributes.status !== 0) refuse("manifest_invalid", "bundle extended attributes could not be observed");
    entries.push(["extendedAttributes", JSON.parse(attributes.stdout)]);
  }
  visit(root);
  return fingerprint(entries);
}

export interface CargoArtifactMessage {
  reason: "compiler-artifact";
  package_id: string;
  target: { name: string; kind: string[]; crate_types: string[]; src_path: string };
  features: string[];
  profile: Record<string, unknown>;
  executable: string | null;
  fresh: boolean;
}

export function selectCargoArtifact(root: string, messages: readonly CargoArtifactMessage[], kind: ArtifactKind, profile: CargoTargetIdentity["requestedProfile"], targetTriple: string): { message: CargoArtifactMessage; target: CargoTargetIdentity } {
  const name = kind === "application" ? "script-kit-gpui" : kind === "rust-libtest" ? "script_kit_gpui" : "export_design_tokens";
  const source = kind === "application" ? "src/main.rs" : kind === "rust-libtest" ? "src/lib.rs" : "src/bin/export_design_tokens.rs";
  const packagePrefix = `path+${pathToFileURL(root).href}#`;
  const selected = messages.filter(message => message.reason === "compiler-artifact" && message.target?.name === name
    && message.package_id.startsWith(packagePrefix)
    && resolve(message.target.src_path) === join(root, source)
    && Array.isArray(message.target.kind) && message.target.kind.includes(kind === "rust-libtest" ? "lib" : "bin")
    && (kind !== "rust-libtest" || message.profile?.test === true)
    && typeof message.executable === "string" && message.executable.length > 0);
  if (selected.length !== 1) refuse("manifest_invalid", "missing or ambiguous Cargo-emitted executable");
  const message = selected[0]!;
  return { message, target: { packageId: message.package_id, packageName: "script-kit-gpui", targetName: name,
    targetKind: message.target.kind, crateTypes: message.target.crate_types, sourcePath: source, features: [...message.features].sort(),
    cargoProfile: message.profile, requestedProfile: profile, targetTriple } };
}

function releaseWrapperLease(lockPath: string, generation: string, cleanup: OwnedCleanup): OwnedCleanup {
  if (!cleanup.processExited || !cleanup.processGroupExited || !cleanup.streamsDrained || cleanup.survivors.some(item => !["task-record", "resource-monitor"].includes(item.kind))) {
    return { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "lease_retained_cleanup_unproved"] };
  }
  try { cacheLease("release", lockPath, [String(process.pid), generation]); return cleanup; }
  catch { return { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "lease_release_failed"] }; }
}

function renameDirectoryNoReplace(source: string, destination: string): void {
  const program = "import ctypes,os,sys\nl=ctypes.CDLL(None,use_errno=True)\na,b=map(os.fsencode,sys.argv[1:])\nif sys.platform=='darwin': r=l.renamex_np(a,b,4)\nelse: r=l.renameat2(-100,a,-100,b,1)\nif r: raise OSError(ctypes.get_errno(),os.strerror(ctypes.get_errno()))";
  command(dirname(source), ["python3", "-B", "-c", program, source, destination]);
}

function buildResourceChecks(root: string) {
  const resources: BuildResourceReport = { scope: "target-agent", measurement: "allocated-blocks", hardQuota: false, automaticEviction: false, checks: [], monitoring: null, refusal: null };
  const check = (phase: string, reserveBytes = 0): BuildResourceObservation => {
    try { const observation = requireBuildAdmission(root, { phase, reserveBytes }); if (resources.checks.length < 12) resources.checks.push(observation); return observation; }
    catch (error) {
      if (error instanceof BuildResourceError) { resources.refusal ||= error.observation; if (resources.checks.length < 12) resources.checks.push(error.observation); }
      throw error;
    }
  };
  const monitor = (summary: BuildResourceSummary): void => {
    const prior = resources.monitoring;
    resources.monitoring = prior ? { ...prior, sampleCount: prior.sampleCount + summary.sampleCount,
      maximumSampledAllocatedBytes: prior.maximumSampledAllocatedBytes === null ? summary.maximumSampledAllocatedBytes : summary.maximumSampledAllocatedBytes === null ? prior.maximumSampledAllocatedBytes : Math.max(prior.maximumSampledAllocatedBytes, summary.maximumSampledAllocatedBytes),
      minimumSampledAvailableBytes: prior.minimumSampledAvailableBytes === null ? summary.minimumSampledAvailableBytes : summary.minimumSampledAvailableBytes === null ? prior.minimumSampledAvailableBytes : Math.min(prior.minimumSampledAvailableBytes, summary.minimumSampledAvailableBytes),
      complete: prior.complete && summary.complete, workerClosed: prior.workerClosed !== false && summary.workerClosed !== false, workerThreadId: summary.workerThreadId ?? prior.workerThreadId, trigger: prior.trigger ?? summary.trigger } : summary;
    resources.refusal ||= summary.trigger;
  };
  return { resources, check, monitor };
}

async function stopResourceGuard(root: string, guard: { stop(): Promise<BuildResourceSummary> }, monitor?: (summary: BuildResourceSummary) => void): Promise<void> {
  let summary: BuildResourceSummary;
  try { summary = await guard.stop(); }
  catch { summary = { sampleCount: 0, maximumSampledAllocatedBytes: null, minimumSampledAvailableBytes: null, complete: false, workerClosed: false, trigger: null }; }
  if ((!summary.complete || summary.workerClosed === false) && !summary.trigger) {
    let observation: BuildResourceObservation;
    try { observation = requireBuildAdmission(root, { phase: "monitor-stop" }); }
    catch (error) { if (!(error instanceof BuildResourceError)) throw error; observation = error.observation; }
    summary = { ...summary, trigger: { ...observation, phase: "monitor-stop", complete: false, withinLimits: false, failureCodes: ["resource_observation_incomplete"] } };
  }
  monitor?.(summary);
  if (summary.trigger) throw Object.assign(new BuildResourceError(summary.trigger.failureCodes[0] || "resource_observation_incomplete", summary.trigger), { resourceWorkerUnclosed: summary.workerClosed === false, resourceWorkerThreadId: summary.workerThreadId ?? null });
}

function monitorCleanup(task: ManagedTask, cleanup: OwnedCleanup, error: unknown): OwnedCleanup {
  if (!error || typeof error !== "object" || !("resourceWorkerUnclosed" in error) || !error.resourceWorkerUnclosed) return cleanup;
  const workerThreadId = "resourceWorkerThreadId" in error && typeof error.resourceWorkerThreadId === "number" ? error.resourceWorkerThreadId : null;
  return { ...cleanup, closed: false, referencesFinalized: false,
    survivors: [...cleanup.survivors.filter(item => item.kind !== "resource-monitor"), { kind: "resource-monitor", identity: canonicalJson({ task: { id: task.identity.id, generation: task.identity.generation }, wrapperPid: process.pid, workerThreadId }), observation: "unknown" }],
    failureCodes: cleanup.failureCodes.includes("resource_monitor_cleanup_unproved") ? cleanup.failureCodes : [...cleanup.failureCodes, "resource_monitor_cleanup_unproved"] };
}

function failedTaskCleanup(task: ManagedTask, cleanup: OwnedCleanup, code: string): OwnedCleanup {
  return { ...cleanup, closed: false, referencesFinalized: false,
    survivors: [...cleanup.survivors.filter(item => item.kind !== "task-record"), { kind: "task-record", identity: canonicalJson(task.identity), observation: "unknown" }],
    failureCodes: cleanup.failureCodes.includes(code) ? cleanup.failureCodes : [...cleanup.failureCodes, code] };
}

function finalizeWrapperTask(task: ManagedTask, cleanup: OwnedCleanup, result: Record<string, unknown>) {
  let failure: string | undefined;
  try { updateManagedTask(task, { state: "finalizing", result }); }
  catch (error) { failure = String(error); cleanup = failedTaskCleanup(task, cleanup, "task_result_finalization_failed"); }
  if (failure) cleanup = { ...cleanup, survivors: cleanup.survivors.map(item => item.kind === "task-record" ? { ...item, identity: canonicalJson({ ...task.identity, revision: task.identity.revision + 1 }) } : item) };
  try { cleanup = finalizeManagedTask(task, cleanup).cleanup; }
  catch (error) { failure = [failure, String(error)].filter(Boolean).join("; "); cleanup = failedTaskCleanup(task, cleanup, "task_finalization_failed"); }
  if (failure) cleanup = { ...cleanup, survivors: cleanup.survivors.map(item => item.kind === "task-record" ? { ...item, identity: canonicalJson(task.identity) } : item) };
  return { cleanup, failure };
}


export function publishImmutableArtifact(root: string, task: ManagedTask, executable: string, body: Omit<ArtifactManifestV3, "schemaVersion" | "artifactId" | "binaryPath" | "binarySha256" | "sizeBytes">, finalBoundary: () => void, resourceCheck = (phase: string, reserveBytes = 0): BuildResourceObservation => requireBuildAdmission(root, { phase, reserveBytes })): ArtifactReference {
  return withManagedMetadata(root, () => {
    const leasePath = join(root, "target-agent/.locks/pool-agent-debug.lock");
    const lease = readOwnedJson(join(leasePath, "lease.json"));
    if (lease.pid !== process.pid || lease.generation !== body.publication.leaseGeneration) refuse("publication_not_finalized", "wrapper lease not owned");
    const source = canonicalArtifactPath(root, relative(root, executable));
    if (!source.startsWith(join(root, "target-agent/pools/agent-debug") + sep)) refuse("unsafe_artifact_path", "Cargo output outside shared pool");
    const stat = lstatSync(source);
    if (!stat.isFile() || !(stat.mode & 0o111) || stat.size > 2 * 1024 ** 3) refuse("unsafe_artifact_path", "Cargo executable type/mode/size invalid");
    resourceCheck("pre-publication", Math.ceil(stat.size / 4096) * 4096 + 64 * 1024);
    const artifactId = `artifact-${randomUUID()}`;
    const parent = canonicalArtifactPath(root, "target-agent/artifacts");
    mkdirSync(parent, { recursive: true, mode: 0o700 });
    const staging = join(parent, `.pending-${artifactId}`), destination = join(parent, artifactId);
    mkdirSync(staging, { mode: 0o700 });
    const directory = lstatSync(staging);
    registerManagedPublicationIntent(task, { id: artifactId, generation: task.identity.generation, pendingPath: relative(root, staging), destinationPath: relative(root, destination), directoryDevice: directory.dev, directoryInode: directory.ino });
    try {
    const binaryName = body.target.targetName;
    const copied = join(staging, binaryName);
    const hash = hashArtifactFile(source);
    copyFileSync(source, copied, constants.COPYFILE_EXCL | constants.COPYFILE_FICLONE);
    const copiedStat = lstatSync(copied);
    if (copiedStat.ino === stat.ino || copiedStat.nlink !== 1 || copiedStat.size !== stat.size
      || hashArtifactFile(copied) !== hash || hashArtifactFile(source) !== hash) refuse("binary_hash_mismatch", "copied executable differs or aliases mutable output");
    chmodSync(copied, 0o500);
    const manifest: ArtifactManifestV3 = { ...body, schemaVersion: 3, artifactId,
      binaryPath: `target-agent/artifacts/${artifactId}/${binaryName}`, binarySha256: hash, sizeBytes: stat.size };
    const manifestPath = join(staging, "manifest.json");
    writeFileSync(manifestPath, `${canonicalJson(manifest)}\n`, { flag: "wx", mode: 0o400 });
    for (const path of [copied, manifestPath]) { const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW); try { fsyncSync(fd); } finally { closeSync(fd); } }
    finalBoundary();
    resourceCheck("post-publication");
    // Eligibility still requires the successfully finalized task after lease release.
    if (readOwnedJson(join(leasePath, "lease.json")).generation !== lease.generation || task.identity.generation !== body.publication.buildTask.generation) refuse("publication_not_finalized", "publication ownership changed");
    chmodSync(staging, 0o500);
    renameDirectoryNoReplace(staging, destination);
    updateManagedPublicationIntent(task, artifactId, "published");
    return { manifestPath: `target-agent/artifacts/${artifactId}/manifest.json`, manifestSha256: hashArtifactFile(join(destination, "manifest.json")) };
    } catch (error) {
      try { updateManagedPublicationIntent(task, artifactId, "failed"); }
      catch (intentError) { throw Object.assign(error instanceof Error ? error : new Error(String(error)), { publicationIntentFailure: String(intentError) }); }
      throw error;
    }
  });
}

function emitWrapperResult(root: string, result: Record<string, unknown>, task: ManagedTask | undefined, cleanup: OwnedCleanup, status: number): number {
  let output: Record<string, unknown> = { ...result, exitCode: status, task: task?.identity ?? null, recordPath: task?.recordPath ?? null, cleanup };
  try {
    if (process.env.SCRIPT_KIT_AGENT_RESULT_PATH) {
      const path = canonicalArtifactPath(root, relative(root, resolve(process.env.SCRIPT_KIT_AGENT_RESULT_PATH)));
      if (!path.startsWith(join(root, ".test-output") + sep)) throw new Error("wrapper_result_outside_owned_outputs");
      writeFileSync(path, `${canonicalJson(output)}\n`, { flag: "wx", mode: 0o600 });
    }
  } catch (error) {
    status = 70;
    cleanup = task ? failedTaskCleanup(task, cleanup, "wrapper_result_persistence_failed") : { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "wrapper_result_persistence_failed"] };
    output = { ...output, status: "failed", artifacts: [], artifact: null, exitCode: status, cleanup, finalizationFailure: String(error) };
  }
  process.stdout.write(`${canonicalJson(output)}\n`);
  return status;
}

export async function runWrapperCargo(args: string[]): Promise<number> {
  const root = realpathSync(process.env.SCRIPT_KIT_REPO_ROOT || resolve(import.meta.dir, "../.."));
  const registeredInputs = process.env.SCRIPT_KIT_AGENT_REGISTERED_TASK === "pi-sidecar"
    ? ["target/pi-sidecar/cache/source-3d1a3950c16ffdb10cd81780b26921c75c180770"] : [];
  const observeSource = () => observeArtifactSource(root, registeredInputs);
  const lockPath = join(root, "target-agent/.locks/pool-agent-debug.lock");
  const generation = process.env.SCRIPT_KIT_AGENT_LEASE_GENERATION || "";
  let ownsLease = false;
  let leaseDelegated = false;
  let task: ManagedTask | undefined;
  let cleanup = emptyOwnedCleanup();
  let result: Record<string, unknown> = { status: "failed", artifacts: [] };
  let status = 70;
  let logFd: number | undefined;
  let sourceMutation: { before: SourceIdentity; after: SourceIdentity } | undefined;
  const references: ArtifactReference[] = [];
  const { resources, check: resourceCheck, monitor } = buildResourceChecks(root);
  let artifactReused = false;
  try {
    // Admission is inside the same cleanup boundary as execution. Never use a
    // caller-supplied path to release a lease, even when admission rejects it.
    const admission = cacheLease("diagnose", lockPath);
    const lease = admission.lease;
    if (admission.state !== "protected" || lease?.pid !== process.pid ||
      lease.generation !== generation || lease.children.length !== 0 ||
      admission.observations?.[0]?.observed !== lease.processStartTime) {
      throw new Error("wrapper_lease_owner_mismatch");
    }
    ownsLease = true;
    cleanup = { ...cleanup, resourcesAcquired: true };
    if (process.env.SCRIPT_KIT_AGENT_LEASE_PATH !== lockPath) throw new Error("wrapper_lease_required");
    if (args[0] === "publish-signed-bundle") {
      leaseDelegated = true;
      return await runSignedBundlePublication(root, args, lockPath, generation);
    }
    const id = `build-${randomUUID()}`;
    const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/managed-tasks", id), kind: "directory", probeId: "agent-cargo" }), id);
    task = beginManagedTask(claim, "build-job", []);
    resourceCheck("preflight");
    logFd = openSync(join(claim.root, "cargo.log"), constants.O_CREAT | constants.O_EXCL | constants.O_WRONLY | constants.O_NOFOLLOW, 0o600);
    const wantsArtifact = !!process.env.SCRIPT_KIT_AGENT_ARTIFACT_KIND;
    const mutatesSource = !wantsArtifact && ((args[0] === "fmt" && !args.includes("--check")) || args[0] === "metadata");
    const kind: ArtifactKind = process.env.SCRIPT_KIT_AGENT_ARTIFACT_KIND === "rust-libtest" ? "rust-libtest"
      : args.some((value, i) => (value === "--bin" && args[i + 1] === "export_design_tokens") || value === "--bin=export_design_tokens") ? "tool" : "application";
    if (wantsArtifact && (kind === "rust-libtest" ? !(args[0] === "test" && args.includes("--lib") && args.includes("--no-run")) : args[0] !== "build")) throw new Error("artifact_producer_mismatch");
    const toolchain = observeToolchain(root);
    const { rustcPath, cargoPath: _cargoPath, host, ...toolchainRecord } = toolchain;
    const normalized = normalizeCompilerPolicy(root, args);
    const { compatibility, profile } = normalized;
    const cache = selectCompilerCache(root, rustcPath, normalized);
    const requestedPolicy = { args, pool: "agent-debug", compilerWorkers: Number(process.env.CARGO_BUILD_JOBS), testWorkers: Number(process.env.RUST_TEST_THREADS), cachePolicy: process.env.SCRIPT_KIT_AGENT_USE_SCCACHE || "auto", loadLimitPercent: process.env.SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT || null };
    const source = observeSource();
    if (!registeredInputs.length && (profile === "release" || process.env.SCRIPT_KIT_TRACK_GIT_HEAD === "1") && source.repositoryDirty) refuse("source_stale", "release requires clean source");
    const ledgerPath = join(root, "target-agent/pools/agent-debug/.executable-provenance.json");
    const ledger = existsSync(ledgerPath) ? readOwnedJson(ledgerPath) : {};
    const ledgerKey = fingerprint({ kind, profile, args, compatibility, toolchainRecord, content: source.compilerInputSha256 });
    let provenanceGeneration = typeof ledger.__provenanceGeneration === "string" ? ledger.__provenanceGeneration : "";
    let effective: Record<string, unknown> = {};
    const makeEffective = () => ({ requestedPolicy, compatibility, provenanceGeneration,
      incremental: normalized.incremental, compilerCache: cache.backend, compilerCacheProbe: cache,
      apfsCloneRequested: process.platform === "darwin", uniquePhysicalSavingsBytes: null });
    const cargoArgs = [...args];
    if (wantsArtifact) {
      if (cargoArgs.some(arg => arg.startsWith("--message-format"))) throw new Error("artifact_publication_owns_message_format");
      cargoArgs.splice(cargoArgs.indexOf("--") < 0 ? cargoArgs.length : cargoArgs.indexOf("--"), 0, "--message-format=json-render-diagnostics");
    }
    let messages: CargoArtifactMessage[] = [];
    let outputBytes = 0;
    let passed = 0, failed = 0, summaries = 0;
    const execute = async (): Promise<number> => {
      const preflight = resourceCheck("pre-cargo");
      effective = makeEffective();
      updateManagedTask(task!, { state: "running", source, effectiveConfiguration: effective });
      const env = { ...normalized.env, RUSTC: rustcPath, RUSTUP_TOOLCHAIN: toolchain.channel, SCRIPT_KIT_PROVENANCE_GENERATION: provenanceGeneration };
      cacheLease("reserve-child", lockPath, [String(process.pid), generation]);
      cleanup = { ...cleanup, processExited: false, processGroupExited: false, streamsDrained: false, closed: false,
        survivors: [{ kind: "process-group", identity: "unobserved", observation: "unknown" }], failureCodes: ["compiler_start_unproved"] };
      const child = await spawnOwnedProcess({ argv: [toolchain.cargoPath, ...cargoArgs], cwd: root, env,
        timeoutMs: Number(process.env.SCRIPT_KIT_AGENT_TIMEOUT_MS || 1_800_000), maxOutputBytes: 64 * 1024 * 1024 }).catch(error => {
          if (error && typeof error === "object" && "cleanup" in error && error.cleanup) cleanup = error.cleanup as OwnedCleanup;
          resourceCheck("post-cargo");
          throw error;
        });
      let resourceFailure: BuildResourceError | undefined;
      let guard: { stop(): Promise<BuildResourceSummary> } | undefined;
      const consume = async (stream: ReadableStream<Uint8Array>, channel: "stdout" | "stderr"): Promise<void> => {
        const reader = stream.getReader(), decoder = new TextDecoder();
        let buffer = "";
        try {
          while (true) {
            const { value, done } = await reader.read();
            if (done) break;
            const keep = value.subarray(0, Math.max(0, 4 * 1024 * 1024 - outputBytes));
            outputBytes += value.length;
            if (keep.length) writeFileSync(logFd!, keep);
            process.stderr.write(value);
            buffer += decoder.decode(value, { stream: true });
            if (buffer.length > 8 * 1024 * 1024) throw new Error("cargo_line_limit");
            let newline: number;
            while ((newline = buffer.indexOf("\n")) >= 0) {
              const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
              const summary = /test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;/.exec(line);
              if (summary) { summaries++; passed += Number(summary[1]); failed += Number(summary[2]); }
              if (wantsArtifact && channel === "stdout" && line.startsWith("{")) {
                const message = JSON.parse(line);
                if (message.reason === "compiler-artifact" && message.executable) messages.push(message);
                if (messages.length > 4096) throw new Error("cargo_artifact_message_limit");
              }
            }
          }
          if (buffer.trim() && wantsArtifact && channel === "stdout") throw new Error("truncated_cargo_json");
        } finally { reader.releaseLock(); }
      };
      try {
        guard = startBuildResourceGuard(root, error => { resourceFailure ||= error; resources.refusal ||= error.observation; void child.close().catch(() => {}); }, preflight);
        cacheLease("bind", lockPath, [String(process.pid), generation, JSON.stringify(child.identity)]);
        updateManagedTask(task!, { ownedProcesses: [...readManagedTask(task!.recordPath, task!.identity).ownedProcesses, child.identity] });
        await Promise.all([consume(child.stdout, "stdout"), consume(child.stderr, "stderr")]);
        return await child.exited;
      } finally {
        try { cleanup = await child.close(); }
        finally {
          try { if (guard) await stopResourceGuard(root, guard, monitor); }
          catch (error) {
            cleanup = monitorCleanup(task!, cleanup, error);
            throw error;
          }
          finally { resourceCheck("post-cargo"); }
        }
        if (resourceFailure) throw resourceFailure;
      }
    };
    status = await execute();
    if (!cleanup.closed) throw new Error("compiler_cleanup_unproved");
    const sourceAfter = observeSource();
    if (mutatesSource) sourceMutation = { before: source, after: sourceAfter };
    else assertSourceBoundary(source, sourceAfter);
    if (fingerprint(compilerCompatibility(root, args)) !== fingerprint(compatibility)) refuse("configuration_stale", "configuration changed during build");
    if (status !== 0) throw new Error("cargo_failed");
    if (args[0] === "test" && !args.includes("--no-run") && (summaries < 1 || passed + failed < 1 || failed > 0)) { status = 1; throw new Error("empty_or_failed_test_selection"); }
    if (wantsArtifact) {
      let selected = selectCargoArtifact(root, messages, kind, profile, normalized.requestedTarget === "host" ? host : normalized.requestedTarget);
      let executable = resolve(selected.message.executable!);
      if (selected.message.fresh && ledger[ledgerKey]?.binarySha256 !== hashArtifactFile(executable)) {
        provenanceGeneration = randomUUID();
        messages = [];
        status = await execute();
        if (status !== 0 || !cleanup.closed) throw new Error("root_provenance_rebuild_failed");
        selected = selectCargoArtifact(root, messages, kind, profile, normalized.requestedTarget === "host" ? host : normalized.requestedTarget);
        if (selected.message.fresh) throw new Error("root_provenance_rebuild_not_observed");
        executable = resolve(selected.message.executable!);
      }
      assertSourceBoundary(source, observeSource());
      updateManagedTask(task, { state: "finalizing", effectiveConfiguration: effective });
      let reference: ArtifactReference | undefined;
      const previous = ledger[ledgerKey]?.reference as ArtifactReference | undefined;
      if (selected.message.fresh && previous) {
        if (!/^target-agent\/artifacts\/[A-Za-z0-9][A-Za-z0-9._-]*\/manifest\.json$/.test(previous.manifestPath) || !isHash(previous.manifestSha256)) refuse("manifest_invalid", "invalid warm publication ledger reference");
        const manifestPath = canonicalArtifactPath(root, previous.manifestPath);
        if (existsSync(manifestPath)) {
          const verified = verifyImmutableArtifact(root, previous, { kind, packageName: "script-kit-gpui", targetName: selected.target.targetName, profile, sourcePolicy: "current-content" });
          if (verified.manifest.binarySha256 !== hashArtifactFile(executable) || fingerprint(verified.manifest.target) !== fingerprint(selected.target)) refuse("binary_hash_mismatch", "warm Cargo output differs from published reference");
          registerManagedArtifactReference(task, previous);
          reference = verified.reference;
          artifactReused = true;
        } else if (existsSync(dirname(manifestPath)) || !isRetiredManagedArtifact(root, previous)) refuse("publication_not_finalized", "warm publication disappeared without an exact retirement receipt");
      }
      reference ??= publishImmutableArtifact(root, task, executable, {
        artifactKind: kind, target: selected.target, source, toolchain: toolchainRecord,
        requestedPolicySha256: fingerprint(requestedPolicy), effectiveConfiguration: effective,
        effectiveConfigurationSha256: fingerprint(effective), requiresExactGitHead: profile === "release" || process.env.SCRIPT_KIT_TRACK_GIT_HEAD === "1",
        publication: { owner: "scripts/agentic/agent-cargo.sh", pool: "agent-debug", leaseGeneration: generation,
          buildTask: task.identity, immutable: true, exportedWhileLeaseHeld: true },
      }, () => {
        assertSourceBoundary(source, observeSource());
        if (fingerprint(compilerCompatibility(root, args)) !== fingerprint(compatibility)) refuse("configuration_stale", "configuration changed during publication");
        const finalToolchain = observeToolchain(root);
        if (finalToolchain.rustcSha256 !== toolchain.rustcSha256 || finalToolchain.cargoSha256 !== toolchain.cargoSha256) refuse("toolchain_mismatch", "compiler replaced during publication");
      }, resourceCheck);
      references.push(reference);
      ledger[ledgerKey] = { binarySha256: hashArtifactFile(executable), provenanceGeneration, reference };
      ledger.__provenanceGeneration = provenanceGeneration;
      atomicManagedJson(ledgerPath, ledger);
    }
    result = { status: "succeeded", exitCode: status, artifacts: references, artifactReused, ...(sourceMutation ? { sourceMutation } : {}), passedTests: passed, failedTests: failed, testSummaries: summaries, outputBytes, retainedLogBytes: Math.min(outputBytes, 4 * 1024 * 1024), admission: JSON.parse(process.env.SCRIPT_KIT_AGENT_ADMISSION_OBSERVATION || "{}") };
  } catch (error) {
    if (status === 0) status = 70;
    if (error && typeof error === "object" && "exitCode" in error && typeof error.exitCode === "number") status = error.exitCode;
    if (error && typeof error === "object" && "cleanup" in error && error.cleanup) cleanup = error.cleanup as OwnedCleanup;
    if (task && error && typeof error === "object" && "publicationIntentFailure" in error) cleanup = failedTaskCleanup(task, cleanup, "publication_intent_finalization_failed");
    if (task) cleanup = monitorCleanup(task, cleanup, error);
    result = { status: "failed", exitCode: status, artifacts: [], ...(sourceMutation ? { sourceMutation } : {}),
      ...(error instanceof BuildResourceError ? { failureCode: error.code, disposition: "BLOCKED_RESOURCE_BUDGET" } : error instanceof ArtifactVerificationError ? { failureCode: error.code, disposition: error.disposition } : { failureCode: String(error) }) };
    process.stderr.write(`AGENT_CARGO error: ${String(error)}\n`);
  } finally {
    if (logFd !== undefined) {
      try { fsyncSync(logFd); }
      catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "log_flush_failed"] }; }
      try { closeSync(logFd); }
      catch { cleanup = { ...cleanup, closed: false, referencesFinalized: false, logWriterClosed: false, failureCodes: [...cleanup.failureCodes, "log_close_failed"] }; }
    }
    if (ownsLease && !leaseDelegated) {
      cleanup = releaseWrapperLease(lockPath, generation, cleanup);
    } else if (!ownsLease) {
      cleanup = { ...cleanup, closed: false, referencesFinalized: false, failureCodes: [...cleanup.failureCodes, "lease_ownership_unproved"] };
    }
    result = { ...result, resources };
    if (task) {
      const finalized = finalizeWrapperTask(task, cleanup, result);
      cleanup = finalized.cleanup;
      if (finalized.failure) { status = 70; result = { ...result, status: "failed", artifacts: [], finalizationFailure: finalized.failure }; }
    }
  }
  if (!cleanup.closed) { status = 70; result = { ...result, status: "failed", artifacts: [] }; }
  status = emitWrapperResult(root, result, task, cleanup, status);
  return status;
}

if (import.meta.main && process.argv[2] === "run-wrapper") process.exitCode = await runWrapperCargo(process.argv.slice(3));

function applicationExpectation(): ArtifactExpectation {
  return { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" };
}
if (import.meta.main && process.argv[2] === "verify-reference") {
  const root = realpathSync(process.argv[3]!);
  const reference = readOwnedJson(process.argv[4]!) as unknown as ArtifactReference;
  const artifact = verifyImmutableArtifact(root, reference, applicationExpectation());
  process.stdout.write(`${artifact.executablePath}\n`);
}
if (import.meta.main && process.argv[2] === "session-pin") {
  const root = realpathSync(process.argv[3]!), session = realpathSync(process.argv[4]!);
  const reference = readOwnedJson(join(session, "artifact-reference.json")) as unknown as ArtifactReference;
  const artifact = verifyImmutableArtifact(root, reference, applicationExpectation());
  const identity = readOwnedJson(join(session, "process-identity.json"));
  if (identity.supervisorPid !== process.ppid) throw new Error("session_pin_requires_supervisor");
  const id = `runtime-${randomUUID()}`;
  const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/managed-tasks", id), kind: "directory", probeId: "session" }), id);
  const task = beginManagedTask(claim, "runtime-run", [reference]);
  updateManagedTask(task, { state: "running", ownedProcesses: [identity as unknown as OwnedProcessIdentity], source: artifact.manifest.source, effectiveConfiguration: artifact.manifest.effectiveConfiguration });
  atomicManagedJson(join(session, "managed-task.json"), { recordPath: task.recordPath, identity: task.identity });
}
if (import.meta.main && process.argv[2] === "session-finalize") {
  const root = realpathSync(process.argv[3]!), session = realpathSync(process.argv[4]!);
  const handle = readOwnedJson(join(session, "managed-task.json")), exit = readOwnedJson(join(session, "app-exit.json"));
  finalizeSupervisorTask(root, handle.recordPath, handle.identity, exit.cleanup, exit.exitCode ?? 70);
}

if (import.meta.main && ["native-task-bind", "native-task-finalize"].includes(process.argv[2]!)) {
  const payload = JSON.parse(await new Response(Bun.stdin.stream()).text());
  if (process.argv[2] === "native-task-bind")
    bindSupervisorTask(payload.repositoryRoot, payload.recordPath, payload.identity, payload.processIdentity);
  else
    finalizeSupervisorTask(payload.repositoryRoot, payload.recordPath, payload.identity, payload.cleanup, payload.exitCode, payload.nativeLifecycle, payload.processIdentity);
}

/** Compare Mach-O compiler payload, excluding only signing-owned bytes. */
export function unsignedMachOPayloadSha256(path: string): Sha256 {
  const bytes = readFileSync(path);
  if (bytes.length < 32 || bytes.readUInt32LE(0) !== 0xfeedfacf) refuse("manifest_invalid", "signed derivation requires a thin 64-bit Mach-O executable");
  const commands = bytes.readUInt32LE(16), commandBytes = bytes.readUInt32LE(20);
  if (commands > 4096 || 32 + commandBytes > bytes.length) refuse("manifest_invalid", "Mach-O load command bounds");
  const normalized = Buffer.from(bytes);
  let cursor = 32, payloadEnd = bytes.length, signatures = 0;
  for (let index = 0; index < commands; index++) {
    if (cursor + 8 > 32 + commandBytes) refuse("manifest_invalid", "truncated Mach-O command");
    const command = bytes.readUInt32LE(cursor), size = bytes.readUInt32LE(cursor + 4);
    if (size < 8 || cursor + size > 32 + commandBytes) refuse("manifest_invalid", "invalid Mach-O command size");
    if (command === 0x1d) {
      if (size !== 16 || ++signatures !== 1) refuse("manifest_invalid", "ambiguous Mach-O signature command");
      const offset = bytes.readUInt32LE(cursor + 8), length = bytes.readUInt32LE(cursor + 12);
      if (offset < 32 + commandBytes || offset + length !== bytes.length) refuse("manifest_invalid", "signature is not the terminal Mach-O region");
      payloadEnd = offset;
      normalized.fill(0, cursor + 8, cursor + 16);
    } else if (command === 0x19 && size >= 72 && bytes.subarray(cursor + 8, cursor + 24).toString("ascii").replace(/\0+$/, "") === "__LINKEDIT") {
      normalized.fill(0, cursor + 32, cursor + 40);
      normalized.fill(0, cursor + 48, cursor + 56);
    }
    cursor += size;
  }
  if (signatures !== 1) refuse("manifest_invalid", "expected compiler ad-hoc/signing signature region");
  return artifactHash(normalized.subarray(0, payloadEnd));
}

function publicationCopyReservation(path: string): number {
  let bytes = 64 * 1024, entries = 0;
  const device = lstatSync(path).dev;
  const visit = (entry: string): void => {
    const stat = lstatSync(entry);
    if (++entries > 100_000 || stat.dev !== device) refuse("unsafe_artifact_path", "publication copy cannot be bounded");
    bytes += Math.max(stat.blocks * 512, Math.ceil(stat.size / 4096) * 4096) + 4096;
    if (stat.isDirectory()) for (const name of readdirSync(entry)) visit(join(entry, name));
    else if (!stat.isFile() && !stat.isSymbolicLink()) refuse("unsafe_artifact_path", "unsupported publication copy entry");
  };
  visit(path);
  return bytes;
}

export async function publishSignedBundle(root: string, task: ManagedTask, inputReference: ArtifactReference, bundle: string, attestationPath: string, leaseGeneration: string,
  resourceCheck = (phase: string, reserveBytes = 0): BuildResourceObservation => requireBuildAdmission(root, { phase, reserveBytes }), monitor?: (summary: BuildResourceSummary) => void): Promise<{ reference: ArtifactReference; cleanup: OwnedCleanup }> {
  const input = verifyImmutableArtifact(root, inputReference, { ...applicationExpectation(), sourcePolicy: "clean-exact-head" });
  const signedExecutable = join(bundle, "Contents/MacOS/script-kit-gpui"), attestation = readOwnedJson(attestationPath);
  const signedHash = hashArtifactFile(signedExecutable), treeHash = bundleTreeHash(bundle);
  if (attestation.sourceSha !== input.manifest.source.gitHead || attestation.binarySha256 !== signedHash
    || attestation.sidecarSha256 !== hashArtifactFile(join(bundle, "Contents/MacOS/pi"))
    || !isHash(attestation.notarizedArchiveSha256) || !/^[A-Z0-9]{10}$/.test(attestation.teamIdentifier)
    || attestation.hardenedRuntime !== true || attestation.stapled !== true || attestation.gatekeeperAccepted !== true
    || unsignedMachOPayloadSha256(input.executablePath) !== unsignedMachOPayloadSha256(signedExecutable)) refuse("binary_hash_mismatch", "signed bundle is not the attested compiler artifact derivation");
  const leasePath = join(root, "target-agent/.locks/pool-agent-debug.lock/lease.json");
  const requireLease = () => { const lease = readOwnedJson(leasePath); if (lease.pid !== process.pid || lease.generation !== leaseGeneration) refuse("publication_not_finalized", "signed publication requires wrapper lease"); };
  const parent = canonicalArtifactPath(root, "target-agent/artifacts"), artifactId = `artifact-${randomUUID()}`;
  const staging = join(parent, `.pending-${artifactId}`), destination = join(parent, artifactId), copiedBundle = join(staging, "Script Kit.app");
  requireLease();
  resourceCheck("pre-publication", publicationCopyReservation(bundle) + publicationCopyReservation(attestationPath));
  withManagedMetadata(root, () => {
    requireLease(); mkdirSync(parent, { recursive: true, mode: 0o700 }); mkdirSync(staging, { mode: 0o700 });
    const directory = lstatSync(staging);
    registerManagedPublicationIntent(task, { id: artifactId, generation: task.identity.generation, pendingPath: relative(root, staging), destinationPath: relative(root, destination), directoryDevice: directory.dev, directoryInode: directory.ino });
  });
  let cleanup = emptyOwnedCleanup();
  try {
    // Preserve notarization/resource-fork/xattr metadata, then validate the actual copied bundle.
    for (const argv of [["/usr/bin/ditto", "--rsrc", "--extattr", "--acl", bundle, copiedBundle],
      ["/usr/bin/codesign", "--verify", "--deep", "--strict", copiedBundle],
      ["/usr/bin/xcrun", "stapler", "validate", copiedBundle],
      ["/usr/sbin/spctl", "--assess", "--type", "execute", copiedBundle]]) {
      const preflight = resourceCheck("pre-publication-command");
      const env: Record<string, string> = { PATH: "/usr/bin:/bin:/usr/sbin:/sbin", LANG: "C" };
      if (process.env.DEVELOPER_DIR) env.DEVELOPER_DIR = process.env.DEVELOPER_DIR;
      cacheLease("reserve-child", dirname(leasePath), [String(process.pid), leaseGeneration]);
      cleanup = { ...cleanup, resourcesAcquired: true, processExited: false, processGroupExited: false, streamsDrained: false, closed: false,
        survivors: [{ kind: "process-group", identity: "unobserved", observation: "unknown" }], failureCodes: ["publication_process_start_unproved"] };
      const child = await spawnOwnedProcess({ argv, cwd: root, env, timeoutMs: 180_000, maxOutputBytes: 8 * 1024 * 1024 }).catch(error => {
        if (error && typeof error === "object" && "cleanup" in error && error.cleanup) cleanup = error.cleanup as OwnedCleanup;
        resourceCheck("post-publication-command");
        throw error;
      });
      let guard: { stop(): Promise<BuildResourceSummary> } | undefined, resourceFailure: BuildResourceError | undefined;
      try {
        guard = startBuildResourceGuard(root, error => { resourceFailure ||= error; void child.close().catch(() => {}); }, preflight);
        cacheLease("bind", dirname(leasePath), [String(process.pid), leaseGeneration, JSON.stringify(child.identity)]);
        updateManagedTask(task, { ownedProcesses: [...readManagedTask(task.recordPath, task.identity).ownedProcesses, child.identity] });
        child.stdin.end();
        const [stdout, stderr, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
        if (code !== 0) throw new Error(`signed_bundle_command_failed:${argv[0]}:${stdout}:${stderr}`);
      } finally {
        try { cleanup = await child.close(); }
        finally {
          try { if (guard) await stopResourceGuard(root, guard, monitor); }
          catch (error) {
            cleanup = monitorCleanup(task, cleanup, error);
            throw error;
          }
          finally { resourceCheck("post-publication-command"); }
        }
        if (resourceFailure) throw resourceFailure;
      }
      if (!cleanup.closed) throw new Error("signed_bundle_command_cleanup_unproved");
    }
    const reference = withManagedMetadata(root, () => {
      requireLease();
      copyFileSync(attestationPath, join(staging, "release-attestation.json"), constants.COPYFILE_EXCL);
      const copiedExecutable = join(copiedBundle, "Contents/MacOS/script-kit-gpui"), attestationSha256 = hashArtifactFile(attestationPath);
      if (hashArtifactFile(copiedExecutable) !== signedHash || bundleTreeHash(copiedBundle) !== treeHash || bundleTreeHash(bundle) !== treeHash
        || hashArtifactFile(join(staging, "release-attestation.json")) !== attestationSha256) refuse("binary_hash_mismatch", "bundle changed during immutable copy");
      const manifest: ArtifactManifestV3 = { ...input.manifest, artifactId,
        binaryPath: `target-agent/artifacts/${artifactId}/Script Kit.app/Contents/MacOS/script-kit-gpui`, binarySha256: signedHash, sizeBytes: lstatSync(copiedExecutable).size,
        publication: { owner: "scripts/agentic/agent-cargo.sh", pool: "agent-debug", leaseGeneration, buildTask: task.identity, immutable: true, exportedWhileLeaseHeld: true },
        derivation: { input: inputReference, transformation: "signed-and-stapled-bundle", attestationSha256, bundleTreeSha256: treeHash } };
      writeFileSync(join(staging, "manifest.json"), `${canonicalJson(manifest)}\n`, { flag: "wx", mode: 0o400 });
      verifyImmutableArtifact(root, inputReference, { ...applicationExpectation(), sourcePolicy: "clean-exact-head" });
      const seal = (path: string): void => {
        const stat = lstatSync(path);
        if (stat.isDirectory()) { for (const name of readdirSync(path)) seal(join(path, name)); chmodSync(path, 0o500); }
        else if (!stat.isSymbolicLink()) { if (stat.nlink !== 1) refuse("unsafe_artifact_path", "bundle hardlink"); chmodSync(path, stat.mode & 0o111 ? 0o500 : 0o400); const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW); try { fsyncSync(fd); } finally { closeSync(fd); } }
      };
      seal(staging);
      if (bundleTreeHash(copiedBundle, true) !== treeHash) refuse("binary_hash_mismatch", "sealed bundle identity differs");
      resourceCheck("post-publication");
      renameDirectoryNoReplace(staging, destination);
      updateManagedPublicationIntent(task, artifactId, "published");
      return { manifestPath: `target-agent/artifacts/${artifactId}/manifest.json`, manifestSha256: hashArtifactFile(join(destination, "manifest.json")) };
    });
    return { reference, cleanup };
  } catch (error) {
    if (error && typeof error === "object" && "cleanup" in error && error.cleanup) cleanup = error.cleanup as OwnedCleanup;
    try { updateManagedPublicationIntent(task, artifactId, "failed"); }
    catch { cleanup = failedTaskCleanup(task, cleanup, "publication_intent_finalization_failed"); }
    throw Object.assign(error instanceof Error ? error : new Error(String(error)), { cleanup });
  }
}

async function runSignedBundlePublication(root: string, args: string[], lockPath: string, generation: string): Promise<number> {
  let task: ManagedTask | undefined, cleanup = emptyOwnedCleanup();
  let result: Record<string, unknown> = { status: "failed", artifacts: [] }, status = 70;
  const { resources, check: resourceCheck, monitor } = buildResourceChecks(root);
  try {
    const flags = new Map<string, string>();
    for (let index = 1; index < args.length; index += 2) {
      const key = args[index]!, value = args[index + 1];
      if (!["--input", "--bundle", "--attestation"].includes(key) || !value || flags.has(key)) throw new Error("invalid_bundle_publication_arguments");
      flags.set(key, value);
    }
    if (flags.size !== 3) throw new Error("signed_bundle_requires_input_bundle_attestation");
    const input = readOwnedJson(resolve(flags.get("--input")!)) as unknown as ArtifactReference;
    const artifact = verifyImmutableArtifact(root, input, { ...applicationExpectation(), sourcePolicy: "clean-exact-head" });
    const id = `build-${randomUUID()}`;
    const claim = claimOutput(validateOutputTarget({ repoRoot: root, candidate: join(root, ".test-output/managed-tasks", id), kind: "directory", probeId: "signed-bundle" }), id);
    task = beginManagedTask(claim, "build-job", [input]);
    resourceCheck("preflight");
    updateManagedTask(task, { state: "running", source: artifact.manifest.source, effectiveConfiguration: artifact.manifest.effectiveConfiguration });
    const published = await publishSignedBundle(root, task, input, realpathSync(flags.get("--bundle")!), resolve(flags.get("--attestation")!), generation, resourceCheck, monitor);
    const reference = published.reference;
    cleanup = published.cleanup;
    result = { status: "succeeded", artifacts: [reference], artifact: reference };
    status = 0;
  } catch (error) {
    if (error instanceof BuildResourceError) { status = error.exitCode; resources.refusal ||= error.observation; }
    if (error && typeof error === "object" && "cleanup" in error && error.cleanup) cleanup = error.cleanup as OwnedCleanup;
    result = { status: "failed", artifacts: [],
      ...(error instanceof BuildResourceError ? { failureCode: error.code, disposition: "BLOCKED_RESOURCE_BUDGET" } : error instanceof ArtifactVerificationError ? { failureCode: error.code, disposition: error.disposition } : { failureCode: String(error) }) };
  }
  finally {
    cleanup = releaseWrapperLease(lockPath, generation, cleanup);
    if (!cleanup.closed) status = 70;
    result = { ...result, resources };
    if (task) {
      const finalized = finalizeWrapperTask(task, cleanup, result);
      cleanup = finalized.cleanup;
      if (finalized.failure || !cleanup.closed) { status = 70; result = { ...result, status: "failed", artifacts: [], artifact: null, ...(finalized.failure ? { finalizationFailure: finalized.failure } : {}) }; }
    }
  }
  status = emitWrapperResult(root, { ...result, ...(status === 0 ? {} : { status: "failed", artifacts: [], artifact: null }) }, task, cleanup, status);
  return status;
}
