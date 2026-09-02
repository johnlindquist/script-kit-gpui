import { afterEach, expect, test } from "bun:test";
import { appendFileSync, cpSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });

function fixture(link = "issues.jsonl") {
  const root = mkdtempSync(join(tmpdir(), "pi-source-"));
  roots.push(root);
  const repo = join(root, "repo"), source = join(root, "snapshot");
  mkdirSync(join(repo, ".beads"), { recursive: true });
  writeFileSync(join(repo, "Cargo.toml"), '[package]\nname = "fixture"\nversion = "0.1.0"\n');
  writeFileSync(join(repo, ".beads/issues.jsonl"), '{"id":"pinned"}\n');
  writeFileSync(join(repo, "other"), "other pinned content\n");
  symlinkSync(link, join(repo, ".beads/beads.jsonl"));
  function git(...args: string[]) {
    const result = Bun.spawnSync(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "core.hooksPath=/dev/null", ...args], {
      cwd: repo,
      env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" },
    });
    if (result.exitCode !== 0) throw new Error(result.stderr.toString());
    return result.stdout.toString().trim();
  }
  git("init", "-q");
  git("add", ".");
  git("commit", "-qm", "pinned");
  const ref = git("rev-parse", "HEAD");
  cpSync(repo, source, { recursive: true, verbatimSymlinks: true, filter: path => path !== join(repo, ".git") });
  appendFileSync(join(source, "Cargo.toml"), "\n[workspace]\n");
  return {
    root, source, git, ref,
    verify(commit = ref) {
      const result = Bun.spawnSync(["python3", join(import.meta.dir, "pi-sidecar-source.py"), join(repo, ".git"), commit, source]);
      return { code: result.exitCode, error: result.stderr.toString() };
    },
    relink(target: string) {
      rmSync(join(source, ".beads/beads.jsonl"));
      symlinkSync(target, join(source, ".beads/beads.jsonl"));
    },
  };
}

test("accepts an exact pinned internal symlink and isolated-workspace adaptation", () => {
  expect(fixture().verify()).toEqual({ code: 0, error: "" });
});

test("rejects a changed link even when its new target is tracked and internal", () => {
  const f = fixture();
  f.relink("../other");
  expect(f.verify().error).toContain("pinned source content differs");
});

test("rejects a pinned relative link outside the snapshot", () => {
  const f = fixture("../../outside");
  writeFileSync(join(f.root, "outside"), "private");
  expect(f.verify().error).toContain("link escapes snapshot");
});

test("rejects absolute links, even to an internal file", () => {
  const f = fixture();
  f.relink(join(f.source, "other"));
  expect(f.verify().error).toContain("link must be relative");
});

test("rejects dangling links and cycles", () => {
  const dangling = fixture("missing");
  expect(dangling.verify().code).not.toBe(0);
  const cycle = fixture("beads.jsonl");
  expect(cycle.verify().code).not.toBe(0);
});

test("rejects replacing a pinned symlink with a regular file", () => {
  const f = fixture();
  rmSync(join(f.source, ".beads/beads.jsonl"));
  writeFileSync(join(f.source, ".beads/beads.jsonl"), '{"id":"pinned"}\n');
  expect(f.verify().error).toContain("pinned link was replaced");
});

test("rejects replacing a regular file with a symlink", () => {
  const f = fixture();
  rmSync(join(f.source, "other"));
  symlinkSync(".beads/issues.jsonl", join(f.source, "other"));
  expect(f.verify().error).toContain("source file was replaced");
});

test("rejects directory symlinks", () => {
  const f = fixture();
  symlinkSync(".beads", join(f.source, "extra-directory"));
  expect(f.verify().error).toContain("directory symlink");
});

test("rejects added, missing, and changed source files", () => {
  const added = fixture();
  writeFileSync(join(added.source, "extra"), "not pinned");
  expect(added.verify().error).toContain("untracked file");
  const missing = fixture();
  rmSync(join(missing.source, "other"));
  expect(missing.verify().error).toContain("inventory differs");
  const changed = fixture();
  writeFileSync(join(changed.source, ".beads/issues.jsonl"), "not pinned");
  expect(changed.verify().error).toContain("pinned source content differs");
});

test("rejects submodules in the pinned Git tree", () => {
  const f = fixture();
  f.git("update-index", "--add", "--cacheinfo", `160000,${f.ref},submodule`);
  f.git("commit", "-qm", "submodule");
  expect(f.verify(f.git("rev-parse", "HEAD")).error).toContain("object/submodule");
});
