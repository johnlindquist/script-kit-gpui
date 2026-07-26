/**
 * WP11 (glass-smoke-harness-max-info): the standalone Actions entry probe
 * must fail closed at setup — a supplied helper that is orphaned (no
 * manifest), tampered (hash mismatch), or role-confused aborts with
 * INVALID_SETUP BEFORE any app process launches. This is the same
 * validated-helper contract the lifecycle probe carries (WP4); locking it
 * here keeps the Actions probe honest while it shares the helper cache.
 */

import { describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const PROBE = resolve(import.meta.dir, "actions-entry-filmstrip.ts");

async function runProbe(args: string[]) {
  const child = Bun.spawn(["bun", PROBE, ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const timer = setTimeout(() => child.kill(), 30_000);
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  clearTimeout(timer);
  return { stdout, stderr, exitCode };
}

function scratch() {
  const root = mkdtempSync(join(tmpdir(), "actions-probe-"));
  // A file that exists is enough to pass the binary existence check; the
  // helper validation must abort the run long before anything executes it.
  const fakeBinary = join(root, "fake-app");
  writeFileSync(fakeBinary, "not a real app");
  return { root, fakeBinary };
}

describe("actions probe validated-helper contract", () => {
  test("an orphan helper (no manifest) is INVALID_SETUP before app launch", async () => {
    const { root, fakeBinary } = scratch();
    const helperDir = join(root, "helper");
    mkdirSync(helperDir);
    const orphanHelper = join(helperDir, "macos-native-window-filmstrip");
    writeFileSync(orphanHelper, "binary bytes");
    const result = await runProbe([
      "--binary",
      fakeBinary,
      "--filmstrip-helper",
      orphanHelper,
      "--out",
      join(root, "out"),
    ]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("INVALID_SETUP");
    expect(result.stderr).toContain("has no manifest");
  });

  test("a byte-tampered helper with a stale manifest is INVALID_SETUP", async () => {
    const { root, fakeBinary } = scratch();
    const helperDir = join(root, "helper");
    mkdirSync(helperDir);
    const helper = join(helperDir, "macos-native-window-filmstrip");
    writeFileSync(helper, "original bytes");
    const staleSha = createHash("sha256")
      .update("original bytes")
      .digest("hex");
    writeFileSync(
      join(helperDir, "manifest.json"),
      JSON.stringify({
        schemaVersion: 1,
        key: "test-key",
        role: "filmstrip",
        sourcePath: "test.swift",
        sourceSha256: "src",
        compiler: {},
        binaryPath: helper,
        binarySha256: staleSha,
        compiledAt: "2026-07-25T00:00:00Z",
      }),
    );
    // Tamper AFTER the manifest was written.
    writeFileSync(helper, "tampered bytes");
    const result = await runProbe([
      "--binary",
      fakeBinary,
      "--filmstrip-helper",
      helper,
      "--out",
      join(root, "out"),
    ]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("INVALID_SETUP");
    expect(result.stderr).toContain("sha256 mismatch");
  });

  test("a role-confused helper (interference manifest) is INVALID_SETUP", async () => {
    const { root, fakeBinary } = scratch();
    const helperDir = join(root, "helper");
    mkdirSync(helperDir);
    const helper = join(helperDir, "macos-glass-interference-monitor");
    writeFileSync(helper, "monitor bytes");
    writeFileSync(
      join(helperDir, "manifest.json"),
      JSON.stringify({
        schemaVersion: 1,
        key: "test-key",
        role: "interference",
        sourcePath: "test.swift",
        sourceSha256: "src",
        compiler: {},
        binaryPath: helper,
        binarySha256: createHash("sha256")
          .update("monitor bytes")
          .digest("hex"),
        compiledAt: "2026-07-25T00:00:00Z",
      }),
    );
    const result = await runProbe([
      "--binary",
      fakeBinary,
      "--filmstrip-helper",
      helper,
      "--out",
      join(root, "out"),
    ]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr).toContain("INVALID_SETUP");
    expect(result.stderr).toContain("role mismatch");
  });
});
