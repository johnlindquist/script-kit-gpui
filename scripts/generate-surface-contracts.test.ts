import { describe, expect, test } from "bun:test";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { dirname, join, resolve } from "path";
import { linkRouteInventory } from "./generate-surface-contracts.ts";
import type { FixtureDescriptor } from "./devtools/lib/owned-evaluation.ts";

function fixture(id: string, appViewVariant: string, surfaceVariant?: string) {
  return { id, family: "main", root: "main" as const, owner: "src/main_sections/render_impl.rs",
    appViewVariant, surfaceVariant, presentationOwner: "production-view",
    proofBoundary: "owned-production-runtime" as const, nativeExclusions: [] } satisfies FixtureDescriptor & {
      appViewVariant: string; surfaceVariant?: string; presentationOwner: string;
    };
}

describe("source inventory catalogue linkage", () => {
  test("links by explicit route metadata rather than interpreting fixture names", () => {
    const inventory = linkRouteInventory([{ surfaceKind: "About", appViewVariants: ["About"] }], [fixture("opaque-id", "About")]);
    expect(inventory[0]?.fixtureIds).toEqual(["opaque-id"]);
    expect(inventory[0]?.presentationOwners).toEqual(["production-view"]);
  });

  test("requires distinct Mini and Full FileSearch mapping evidence", () => {
    const entries = ["FileSearchMini", "FileSearchFull"].map((surfaceKind) => ({ surfaceKind, appViewVariants: ["FileSearchView"] }));
    expect(() => linkRouteInventory(entries, [fixture("mini", "FileSearchView", "FileSearchMini")])).toThrow("FileSearchFull");
    const inventory = linkRouteInventory(entries, [fixture("mini", "FileSearchView", "FileSearchMini"), fixture("full", "FileSearchView", "FileSearchFull")]);
    expect(inventory.map((row) => row.fixtureIds)).toEqual([["mini"], ["full"]]);
  });

  test("does not turn an inactive legacy mapping into runtime proof", () => {
    const inventory = linkRouteInventory([{ surfaceKind: "ActionsDialog", appViewVariants: ["ActionsDialog"] }], []);
    expect(inventory[0]?.proofBoundary).toBe("inactive-legacy-route");
    expect(inventory[0]?.fixtureIds).toEqual([]);
  });

  test("rejects missing live mappings and duplicate descriptor identities", () => {
    expect(() => linkRouteInventory([{ surfaceKind: "About", appViewVariants: ["About"] }], [])).toThrow("Missing compiled fixture");
    expect(() => linkRouteInventory([], [fixture("same", "About"), fixture("same", "ScriptList")])).toThrow("duplicate");
  });
});

const matrixPath = "docs/ai/contracts/surface-contracts.json";

function withSnapshotProject(run: (root: string) => void): void {
  const root = mkdtempSync(join(tmpdir(), "surface-contract-snapshot-"));
  try {
    for (const path of ["scripts/generate-surface-contracts.ts", "src/main_sections/app_view_state.rs", matrixPath]) {
      const destination = join(root, path);
      mkdirSync(dirname(destination), { recursive: true });
      copyFileSync(resolve(import.meta.dir, "..", path), destination);
    }
    run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function generate(root: string, ...args: string[]) {
  return Bun.spawnSync([process.execPath, "scripts/generate-surface-contracts.ts", ...args], {
    cwd: root, stdout: "pipe", stderr: "pipe",
  });
}

describe("single generated surface contract snapshot", () => {
  test("checks a clean source tree without a native catalogue and detects stale source-derived fields", () => {
    withSnapshotProject((root) => {
      const current = generate(root, "--check");
      expect(current.stderr.toString()).toBe("");
      expect(current.exitCode).toBe(0);

      const matrix = JSON.parse(readFileSync(join(root, matrixPath), "utf8"));
      matrix.entries[0].focusPolicy = "stale-policy";
      writeFileSync(join(root, matrixPath), `${JSON.stringify(matrix, null, 2)}\n`);
      const stale = generate(root, "--check");
      expect(stale.exitCode).not.toBe(0);
      expect(stale.stderr.toString()).toContain("is stale");
    });
  });

  test("requires explicit compiled catalogue input for writing and stdout", () => {
    withSnapshotProject((root) => {
      const original = readFileSync(join(root, matrixPath), "utf8");
      for (const mode of ["--write", "--stdout"]) {
        const missing = generate(root, mode);
        expect(missing.exitCode).not.toBe(0);
        expect(missing.stderr.toString()).toContain("require --catalogue");
        expect(missing.stdout.toString()).toBe("");
      }
      expect(readFileSync(join(root, matrixPath), "utf8")).toBe(original);
    });
  });

  test("embeds every compiled fixture field and detects catalogue drift until regenerated", () => {
    withSnapshotProject((root) => {
      const matrix = JSON.parse(readFileSync(join(root, matrixPath), "utf8"));
      matrix.fixtures[0].nativeExclusions.push("additional compiled catalogue exclusion");
      const catalogue = join(root, "design-discover.json");
      writeFileSync(catalogue, JSON.stringify({ schemaVersion: 1, fixtures: matrix.fixtures }));

      const printed = generate(root, "--catalogue", catalogue, "--stdout");
      expect(printed.exitCode).toBe(0);
      expect(JSON.parse(printed.stdout.toString()).fixtures).toEqual(matrix.fixtures);
      const stale = generate(root, "--catalogue", catalogue, "--check");
      expect(stale.exitCode).not.toBe(0);
      expect(stale.stderr.toString()).toContain("is stale");

      expect(generate(root, "--catalogue", catalogue, "--write").exitCode).toBe(0);
      expect(readFileSync(join(root, matrixPath), "utf8")).toBe(printed.stdout.toString());
      expect(generate(root, "--check").exitCode).toBe(0);
    });
  });
});
