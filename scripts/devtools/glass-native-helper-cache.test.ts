// WP4 (glass-smoke-harness-max-info): the native helper cache must compile
// once per (source, compiler, flags) identity, refuse poisoned binaries, and
// treat role mismatches as INVALID_SETUP — never silently recompile or reuse.
// Run from the repo root: bun test ./scripts/devtools/glass-native-helper-cache.test.ts

import { describe, expect, test } from "bun:test";
import {
  appendFileSync,
  mkdtempSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  computeHelperKey,
  helperManifestPath,
  prepareHelper,
  requireValidatedHelper,
  validateHelperEntry,
} from "./glass-native-helper-cache.ts";

const TRIVIAL_SWIFT = `import Foundation\nprint("helper-cache-test")\n`;

function makeWorkspace() {
  const root = mkdtempSync(join(tmpdir(), "glass-helper-cache-"));
  const sourcePath = join(root, "trivial.swift");
  writeFileSync(sourcePath, TRIVIAL_SWIFT);
  return { root, sourcePath, cacheDir: join(root, "cache") };
}

describe("helper cache key", () => {
  const base = {
    sourcePath: "/tmp/a.swift",
    sourceSha256: "a".repeat(64),
    swiftcVersion: "swiftc 6.0",
    sdkPath: "/sdk",
    architecture: "arm64",
    flags: ["-O"],
  };

  test("changing compiler flags changes the key", () => {
    expect(computeHelperKey(base)).not.toBe(
      computeHelperKey({ ...base, flags: ["-O", "-parse-as-library"] }),
    );
  });

  test("changing the source hash changes the key", () => {
    expect(computeHelperKey(base)).not.toBe(
      computeHelperKey({ ...base, sourceSha256: "b".repeat(64) }),
    );
  });

  test("identical identity is stable", () => {
    expect(computeHelperKey(base)).toBe(computeHelperKey({ ...base }));
  });
});

describe("prepareHelper", () => {
  test("compiles once, then reuses; poisoning the binary fails loudly", async () => {
    const workspace = makeWorkspace();
    const first = await prepareHelper("fixture", {
      cacheDir: workspace.cacheDir,
      sourcePath: workspace.sourcePath,
    });
    expect(first.compiled).toBe(true);
    expect(first.manifest.role).toBe("fixture");

    const second = await prepareHelper("fixture", {
      cacheDir: workspace.cacheDir,
      sourcePath: workspace.sourcePath,
    });
    expect(second.compiled).toBe(false);
    expect(second.binaryPath).toBe(first.binaryPath);
    expect(second.manifest.binarySha256).toBe(first.manifest.binarySha256);

    // Append one byte to the cached helper: the entry is poisoned and must
    // fail before any consumer launches the app — never silently reused.
    appendFileSync(first.binaryPath, "\0");
    await expect(
      prepareHelper("fixture", {
        cacheDir: workspace.cacheDir,
        sourcePath: workspace.sourcePath,
      }),
    ).rejects.toThrow("poisoned helper cache entry");
    expect(
      validateHelperEntry(first.manifest, "fixture", first.binaryPath),
    ).toContainEqual(expect.stringContaining("sha256 mismatch"));
  }, 60_000);

  test("changed source or flags produce a new key; the old entry is never reused", async () => {
    const workspace = makeWorkspace();
    const first = await prepareHelper("fixture", {
      cacheDir: workspace.cacheDir,
      sourcePath: workspace.sourcePath,
    });
    writeFileSync(
      workspace.sourcePath,
      `${TRIVIAL_SWIFT}// changed source\n`,
    );
    const changedSource = await prepareHelper("fixture", {
      cacheDir: workspace.cacheDir,
      sourcePath: workspace.sourcePath,
    });
    expect(changedSource.compiled).toBe(true);
    expect(changedSource.manifest.key).not.toBe(first.manifest.key);
    expect(changedSource.binaryPath).not.toBe(first.binaryPath);

    const changedFlags = await prepareHelper("fixture", {
      cacheDir: workspace.cacheDir,
      sourcePath: workspace.sourcePath,
      flags: ["-Onone"],
    });
    expect(changedFlags.compiled).toBe(true);
    expect(changedFlags.manifest.key).not.toBe(changedSource.manifest.key);
  }, 120_000);

  test("role mismatch fails even when the binary is executable", async () => {
    const workspace = makeWorkspace();
    const entry = await prepareHelper("interference", {
      cacheDir: workspace.cacheDir,
      sourcePath: workspace.sourcePath,
    });
    // Supplying the interference helper as the filmstrip helper must fail
    // validation before launch.
    expect(() =>
      requireValidatedHelper(entry.binaryPath, "filmstrip"),
    ).toThrow("helper role mismatch");
    // The correct role still validates.
    const validated = requireValidatedHelper(entry.binaryPath, "interference");
    expect(validated.manifest.key).toBe(entry.manifest.key);
    expect(helperManifestPath(entry.binaryPath)).toBe(entry.manifestPath);
  }, 60_000);

  test("a helper path without a manifest is INVALID_SETUP", () => {
    const workspace = makeWorkspace();
    const orphan = join(workspace.root, "orphan-helper");
    writeFileSync(orphan, "#!/bin/bash\n");
    expect(() => requireValidatedHelper(orphan, "filmstrip")).toThrow(
      "INVALID_SETUP",
    );
  });
});
