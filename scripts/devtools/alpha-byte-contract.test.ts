import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const harness = "scripts/devtools/alpha-byte-contract-harness.rs";
let directory = "";
let unitRun: ReturnType<typeof spawnSync>;
let libraryPath = "";

function rustc(args: string[], input?: string) {
  return spawnSync("rustc", args, {
    cwd: new URL("../..", import.meta.url).pathname,
    encoding: "utf8",
    input,
    timeout: 15_000,
    env: {
      ...process.env,
      SCRIPT_KIT_NONINTERACTIVE: "1",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
      SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
      SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
      SCRIPT_KIT_ALLOW_LIVE_AI: "0",
    },
  });
}

beforeAll(() => {
  directory = mkdtempSync(join(tmpdir(), "script-kit-alpha-byte-unit-"));
  const testBinary = join(directory, "alpha-byte-production-tests");
  const compiledTests = rustc(["--edition=2021", "--test", harness, "-o", testBinary]);
  if (compiledTests.status !== 0) {
    throw new Error(`production AlphaByte unit harness failed to compile: ${compiledTests.stderr}`);
  }
  unitRun = spawnSync(testBinary, ["--test-threads=1"], {
    cwd: directory,
    encoding: "utf8",
    timeout: 10_000,
  });
  libraryPath = join(directory, "libalpha_byte_contract.rlib");
  const compiledLibrary = rustc([
    "--edition=2021",
    "--crate-type=lib",
    "--crate-name=alpha_byte_contract",
    harness,
    "-o",
    libraryPath,
  ]);
  if (compiledLibrary.status !== 0) {
    throw new Error(`production AlphaByte compile-fail library unavailable: ${compiledLibrary.stderr}`);
  }
});

afterAll(() => {
  if (directory) rmSync(directory, { recursive: true, force: true });
});

function compileRejected(source: string, label: string) {
  return rustc([
    "--edition=2021",
    "--crate-type=lib",
    "-",
    "--extern",
    `alpha_byte_contract=${libraryPath}`,
    "-o",
    join(directory, `${label}.rlib`),
  ], source);
}

describe("real production authored-alpha unit and compiler contracts", () => {
  test("direct rustc executes every existing production AlphaByte behavior test", () => {
    expect(unitRun.status).toBe(0);
    expect(unitRun.stdout).toContain("running 9 tests");
    expect(unitRun.stdout).toContain("test result: ok. 9 passed; 0 failed");
    for (const existingProductionTest of [
      "alpha_byte_is_one_byte",
      "authored_preserves_all_u8_values",
      "from_normalized_clamps",
      "from_normalized_preserves_existing_quantization",
      "rounded_quantization_is_distinct_and_explicit",
      "from_authored_f32_preserves_the_historical_round_cast",
      "pack_rgb_alpha_preserves_channel_order",
    ]) {
      expect(unitRun.stdout).toContain(existingProductionTest);
    }
  });

  test("a raw normalized float cannot compile through the typed packer", () => {
    const rejected = compileRejected(
      "pub fn reject() { let _ = alpha_byte_contract::pack_rgb_alpha(0xEF4444, 0.5_f32); }",
      "reject-normalized-packer",
    );
    expect(rejected.status).not.toBe(0);
    expect(rejected.stderr).toContain("mismatched types");
    expect(rejected.stderr).toContain("AlphaByte");
  });

  test("the authored-byte constructor cannot accept a floating opacity", () => {
    const rejected = compileRejected(
      "pub fn reject() { let _ = alpha_byte_contract::AlphaByte::authored(0.85_f32); }",
      "reject-float-authored",
    );
    expect(rejected.status).not.toBe(0);
    expect(rejected.stderr).toContain("mismatched types");
    expect(rejected.stderr).toContain("u8");
  });

  test("private tuple storage cannot bypass explicit unit constructors", () => {
    const rejected = compileRejected(
      "pub fn reject() { let _ = alpha_byte_contract::AlphaByte(50_u8); }",
      "reject-private-storage",
    );
    expect(rejected.status).not.toBe(0);
    expect(rejected.stderr).toContain("private fields");
  });
});
