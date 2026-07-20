import { expect, test } from "bun:test";
import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const repoRoot = resolve(import.meta.dir, "../..");
const outputRoot = join(repoRoot, ".test-output", "main-menu-focus-flicker");

function outputEntries(): string[] | null {
  return existsSync(outputRoot) ? readdirSync(outputRoot).sort() : null;
}

test("--help exits without claiming output or launching the probe", () => {
  const before = outputEntries();
  const result = Bun.spawnSync(
    ["bun", join(import.meta.dir, "main-menu-focus-flicker.ts"), "--help"],
    {
      cwd: repoRoot,
      stdout: "pipe",
      stderr: "pipe",
    },
  );

  expect(result.exitCode).toBe(0);
  expect(result.stdout.toString()).toContain("Usage: bun scripts/agentic/main-menu-focus-flicker.ts");
  expect(result.stderr.toString()).toBe("");
  expect(outputEntries()).toEqual(before);
});
