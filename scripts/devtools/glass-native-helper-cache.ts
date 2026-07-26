// Hash-bound compile cache for the native Swift observer helpers.
//
// Oracle plan glass-smoke-harness-max-info, work package 4. The lifecycle,
// actions, rapid-toggle, drag, and contrast probes each recompiled their
// Swift helpers on every invocation. The v2 study orchestrator prepares each
// helper ONCE per unique (source, compiler) identity and hands validated
// binaries to consumers; standalone scripts keep their legacy compile
// behavior when no helper is supplied.
//
// Poisoning is a first-class failure: every consumer must hash the provided
// binary immediately before launch and compare it against the manifest — a
// mismatch is INVALID_SETUP before the app is launched, never a silent
// fallback recompile.

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";

export type HelperRole =
  | "filmstrip"
  | "interference"
  | "drag"
  | "resize"
  | "fixture"
  | "window-query";

/** Canonical helper sources and their exact legacy compile flags. */
export const HELPER_DEFINITIONS: Record<
  HelperRole,
  { source: string; flags: string[] }
> = {
  filmstrip: {
    source: "scripts/agentic/macos-native-window-filmstrip.swift",
    flags: ["-parse-as-library", "-O"],
  },
  interference: {
    source: "scripts/agentic/macos-glass-interference-monitor.swift",
    flags: ["-O"],
  },
  drag: {
    source: "scripts/agentic/macos-native-drag-sampler.swift",
    flags: ["-O", "-whole-module-optimization"],
  },
  resize: {
    source: "scripts/agentic/macos-window-resize.swift",
    flags: ["-O"],
  },
  fixture: {
    source: "scripts/agentic/macos-glass-background-fixture.swift",
    flags: ["-O"],
  },
  "window-query": {
    source: "scripts/agentic/macos-window-query.swift",
    flags: ["-O"],
  },
};

export type HelperCompilerIdentity = {
  swiftcVersion: string;
  sdkPath: string;
  architecture: string;
  flags: string[];
};

export type HelperCacheManifest = {
  schemaVersion: 1;
  key: string;
  role: HelperRole;
  sourcePath: string;
  sourceSha256: string;
  compiler: HelperCompilerIdentity;
  binaryPath: string;
  binarySha256: string;
  compiledAt: string;
};

export const sha256Bytes = (bytes: Uint8Array | Buffer | string): string =>
  createHash("sha256").update(bytes).digest("hex");

export const sha256File = (path: string): string =>
  sha256Bytes(readFileSync(path));

export function computeHelperKey(input: {
  sourcePath: string;
  sourceSha256: string;
  swiftcVersion: string;
  sdkPath: string;
  architecture: string;
  flags: string[];
}): string {
  return sha256Bytes(
    JSON.stringify({
      sourcePath: input.sourcePath,
      sourceSha256: input.sourceSha256,
      swiftcVersion: input.swiftcVersion,
      sdkPath: input.sdkPath,
      architecture: input.architecture,
      flags: input.flags,
    }),
  );
}

async function runCommand(command: string[]) {
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { stdout, stderr, exitCode };
}

let cachedCompilerIdentity:
  | { swiftcVersion: string; sdkPath: string; architecture: string }
  | null = null;

export async function queryCompilerIdentity(): Promise<{
  swiftcVersion: string;
  sdkPath: string;
  architecture: string;
}> {
  if (cachedCompilerIdentity) return cachedCompilerIdentity;
  const version = await runCommand(["xcrun", "swiftc", "--version"]);
  if (version.exitCode !== 0) {
    throw new Error(`swiftc --version failed: ${version.stderr}`);
  }
  const sdk = await runCommand(["xcrun", "--show-sdk-path"]);
  if (sdk.exitCode !== 0) {
    throw new Error(`xcrun --show-sdk-path failed: ${sdk.stderr}`);
  }
  cachedCompilerIdentity = {
    swiftcVersion: version.stdout.split("\n")[0]?.trim() ?? "",
    sdkPath: sdk.stdout.trim(),
    architecture: process.arch,
  };
  return cachedCompilerIdentity;
}

export type PrepareHelperResult = {
  manifest: HelperCacheManifest;
  binaryPath: string;
  manifestPath: string;
  compiled: boolean;
};

export const helperManifestPath = (binaryPath: string): string =>
  join(dirname(binaryPath), "manifest.json");

/**
 * Prepare a helper binary, compiling at most once per unique cache key.
 * Reuse ALWAYS re-hashes the cached binary against the manifest: a mutated
 * binary is never reused — the poisoned entry fails loudly instead.
 */
export async function prepareHelper(
  role: HelperRole,
  options: {
    cacheDir: string;
    sourcePath?: string;
    flags?: string[];
    repoRoot?: string;
  },
): Promise<PrepareHelperResult> {
  const definition = HELPER_DEFINITIONS[role];
  if (!definition) throw new Error(`unknown helper role: ${role}`);
  const repoRoot = options.repoRoot ?? resolve(import.meta.dir, "../..");
  const sourcePath = resolve(options.sourcePath ?? join(repoRoot, definition.source));
  if (!existsSync(sourcePath)) {
    throw new Error(`helper source missing: ${sourcePath}`);
  }
  const flags = options.flags ?? definition.flags;
  const sourceSha256 = sha256File(sourcePath);
  const compiler = await queryCompilerIdentity();
  const key = computeHelperKey({
    sourcePath,
    sourceSha256,
    swiftcVersion: compiler.swiftcVersion,
    sdkPath: compiler.sdkPath,
    architecture: compiler.architecture,
    flags,
  });
  const entryDir = join(options.cacheDir, key);
  const binaryPath = join(entryDir, role);
  const manifestPath = join(entryDir, "manifest.json");

  if (existsSync(manifestPath) && existsSync(binaryPath)) {
    const manifest = JSON.parse(
      readFileSync(manifestPath, "utf8"),
    ) as HelperCacheManifest;
    const errors = validateHelperEntry(manifest, role, binaryPath);
    if (errors.length > 0) {
      throw new Error(
        `poisoned helper cache entry ${key} (${role}): ${errors.join("; ")}`,
      );
    }
    return { manifest, binaryPath, manifestPath, compiled: false };
  }

  mkdirSync(entryDir, { recursive: true });
  const stagingPath = `${binaryPath}.tmp-${process.pid}`;
  const compile = await runCommand([
    "xcrun",
    "swiftc",
    ...flags,
    sourcePath,
    "-o",
    stagingPath,
  ]);
  if (compile.exitCode !== 0) {
    throw new Error(`${role} helper compile failed: ${compile.stderr}`);
  }
  renameSync(stagingPath, binaryPath);
  const manifest: HelperCacheManifest = {
    schemaVersion: 1,
    key,
    role,
    sourcePath,
    sourceSha256,
    compiler: { ...compiler, flags },
    binaryPath,
    binarySha256: sha256File(binaryPath),
    compiledAt: new Date().toISOString(),
  };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return { manifest, binaryPath, manifestPath, compiled: true };
}

/**
 * Validate a cache entry against an expected role and the binary ON DISK
 * RIGHT NOW. Consumers call this immediately before launching the app; any
 * error is INVALID_SETUP for the run, never a silent recompile.
 */
export function validateHelperEntry(
  manifest: HelperCacheManifest,
  expectedRole: HelperRole,
  binaryPath: string,
): string[] {
  const errors: string[] = [];
  if (manifest.schemaVersion !== 1) {
    errors.push("helper manifest schemaVersion must be 1");
  }
  if (manifest.role !== expectedRole) {
    errors.push(
      `helper role mismatch: manifest says ${manifest.role}, expected ${expectedRole}`,
    );
  }
  if (!existsSync(binaryPath)) {
    errors.push(`helper binary missing: ${binaryPath}`);
    return errors;
  }
  const actual = sha256File(binaryPath);
  if (actual !== manifest.binarySha256) {
    errors.push(
      `helper binary sha256 mismatch: manifest ${manifest.binarySha256}, on disk ${actual}`,
    );
  }
  return errors;
}

/**
 * Resolve a consumer-facing `--<role>-helper <binaryPath>` argument: load the
 * sibling manifest, validate role + current hash, and fail closed. Returns
 * the validated binary path.
 */
export function requireValidatedHelper(
  binaryPath: string,
  expectedRole: HelperRole,
): { binaryPath: string; manifest: HelperCacheManifest } {
  const manifestPath = helperManifestPath(binaryPath);
  if (!existsSync(manifestPath)) {
    throw new Error(
      `INVALID_SETUP: supplied ${expectedRole} helper has no manifest: ${manifestPath}`,
    );
  }
  const manifest = JSON.parse(
    readFileSync(manifestPath, "utf8"),
  ) as HelperCacheManifest;
  const errors = validateHelperEntry(manifest, expectedRole, resolve(binaryPath));
  if (errors.length > 0) {
    throw new Error(
      `INVALID_SETUP: supplied ${expectedRole} helper failed validation: ${
        errors.join("; ")
      }`,
    );
  }
  return { binaryPath: resolve(binaryPath), manifest };
}
