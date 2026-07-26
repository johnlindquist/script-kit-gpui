// Characterization lock for the LEGACY pair-v1 ABBA shell runner.
//
// Oracle plan glass-smoke-harness-max-info, work package 1. Executes the REAL
// scripts/agentic/glass-entry-abba.sh under a temporary PATH of bounded stubs
// (bun, xcrun, pmset, sysctl, system_profiler, uptime, sw_vers, ps) plus a
// pre-placed fixture-helper stub, with real python3/shasum/awk/seq. These
// assertions freeze the legacy contract the in-flight alpha arc depends on;
// the additive v2 study harness gets NEW commands and must not reinterpret
// this one. Run from the repo root: bun test scripts/agentic/glass-entry-abba.test.ts

import { describe, expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const REPO = resolve(import.meta.dir, "../..");
const SCRIPT = join(REPO, "scripts/agentic/glass-entry-abba.sh");

interface Harness {
  root: string;
  out: string;
  stubLog: string;
  binaryA: string;
  binaryB: string;
  env: Record<string, string>;
}

function writeExecutable(path: string, content: string): void {
  writeFileSync(path, content);
  chmodSync(path, 0o755);
}

function makeHarness(overrides: Record<string, string> = {}): Harness {
  const root = mkdtempSync(join(tmpdir(), "glass-abba-lock-"));
  const stubs = join(root, "stubs");
  const out = join(root, "out");
  mkdirSync(stubs);
  mkdirSync(out);
  const stubLog = join(root, "stub-invocations.log");
  writeFileSync(stubLog, "");

  const binaryA = join(root, "binary-a");
  const binaryB = join(root, "binary-b");
  writeExecutable(binaryA, "#!/bin/bash\n# stub product binary A\n");
  writeExecutable(binaryB, "#!/bin/bash\n# stub product binary B\n");

  writeExecutable(
    join(stubs, "bun"),
    `#!/bin/bash
echo "bun $*" >> "$STUB_LOG"
outDir=""
prev=""
for a in "$@"; do
  [ "$prev" = "--out" ] && outDir="$a"
  prev="$a"
done
case "$1" in
  *main-window-native-drag.ts)
    if [ "\${DRAG_NO_RECEIPT:-0}" != "1" ]; then
      mkdir -p "$outDir"
      echo '{"stub":"drag"}' > "$outDir/receipt.json"
    fi
    exit 3 ;;
  *glass-lifecycle-filmstrip.ts)
    mkdir -p "$outDir"
    echo '{"stub":"lifecycle"}' > "$outDir/receipt.json"
    exit 0 ;;
esac
exit 0
`,
  );
  writeExecutable(
    join(stubs, "sysctl"),
    `#!/bin/bash\necho "{ \${STUB_LOAD1:-1.50} 1.40 1.30 }"\n`,
  );
  writeExecutable(
    join(stubs, "pmset"),
    `#!/bin/bash\necho " CPU_Speed_Limit = 100"\n`,
  );
  writeExecutable(join(stubs, "uptime"), "#!/bin/bash\necho up-stub\n");
  writeExecutable(
    join(stubs, "sw_vers"),
    "#!/bin/bash\necho 26.5.1-stub\n",
  );
  writeExecutable(
    join(stubs, "system_profiler"),
    "#!/bin/bash\necho '{}'\n",
  );
  writeExecutable(
    join(stubs, "ps"),
    "#!/bin/bash\nprintf '%s\\n' ' 1.0 1 stub-a' ' 0.5 2 stub-b'\n",
  );
  // The runner must never compile Swift here: the helper is pre-placed.
  writeExecutable(join(stubs, "xcrun"), "#!/bin/bash\nexit 97\n");
  writeExecutable(
    join(out, "macos-glass-background-fixture"),
    `#!/bin/bash
receipt=""
prev=""
for a in "$@"; do
  [ "$prev" = "--receipt" ] && receipt="$a"
  prev="$a"
done
echo '{"stub":"fixture"}' > "$receipt"
sleep 300
`,
  );

  return {
    root,
    out,
    stubLog,
    binaryA,
    binaryB,
    env: {
      ...process.env,
      PATH: `${stubs}:${process.env.PATH ?? ""}`,
      STUB_LOG: stubLog,
      ...overrides,
    },
  };
}

function runScript(
  harness: Harness,
  args: string[],
): { exitCode: number; stderr: string } {
  const proc = Bun.spawnSync(
    [
      "bash",
      SCRIPT,
      "--a",
      harness.binaryA,
      "--b",
      harness.binaryB,
      "--out",
      harness.out,
      ...args,
    ],
    { cwd: REPO, env: harness.env, timeout: 120_000 },
  );
  return {
    exitCode: proc.exitCode ?? -1,
    stderr: proc.stderr.toString(),
  };
}

function readRuns(harness: Harness): Array<Record<string, unknown>> {
  return readFileSync(join(harness.out, "runs.jsonl"), "utf8")
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line) as Record<string, unknown>);
}

describe("legacyPairV1 shell runner contract", () => {
  test("legacyPairV1: one block schedules builds in exactly A,B,B,A", () => {
    const harness = makeHarness();
    const { exitCode } = runScript(harness, ["--warmups", "0", "--blocks", "1"]);
    expect(exitCode).toBe(0);

    const rows = readRuns(harness);
    const accepted = rows.filter((row) => row.accepted === true);
    expect(accepted.map((row) => row.build)).toEqual(["A", "B", "B", "A"]);
    expect(accepted.map((row) => row.run)).toEqual([
      "run-01-A",
      "run-02-B",
      "run-03-B",
      "run-04-A",
    ]);
    expect(accepted.every((row) => row.eligible === true)).toBe(true);

    // The lifecycle probe must have been driven with the matching binaries
    // in the same order.
    const lifecycleBinaries = readFileSync(harness.stubLog, "utf8")
      .split("\n")
      .filter((line) => line.includes("glass-lifecycle-filmstrip.ts"))
      .map((line) => {
        const parts = line.split(" ");
        return parts[parts.indexOf("--binary") + 1];
      });
    expect(lifecycleBinaries).toEqual([
      harness.binaryA,
      harness.binaryB,
      harness.binaryB,
      harness.binaryA,
    ]);
  });

  test("legacyPairV1: --blocks 0 produces zero accepted runs", () => {
    // Regression lock for the BSD `seq 1 0` counts-DOWN defect: zero blocks
    // must mean zero accepted runs, not two phantom blocks.
    const harness = makeHarness();
    const { exitCode } = runScript(harness, ["--warmups", "1", "--blocks", "0"]);
    expect(exitCode).toBe(0);

    const rows = readRuns(harness);
    expect(rows.filter((row) => row.accepted === true)).toHaveLength(0);
    expect(rows.map((row) => row.run)).toEqual(["warmup-A-1", "warmup-B-1"]);
  });

  test("legacyPairV1: missing layout receipt is fatal to metric grading", () => {
    const harness = makeHarness({ DRAG_NO_RECEIPT: "1" });
    const { exitCode, stderr } = runScript(harness, [
      "--warmups",
      "0",
      "--blocks",
      "1",
    ]);
    expect(exitCode).toBe(0);
    expect(stderr).toContain("layout receipt missing");

    const rows = readRuns(harness);
    expect(rows).toHaveLength(4);
    for (const row of rows) {
      expect(row.metricExit).not.toBe(0);
      expect(row.metricPass).toBeNull();
      expect(row.runMaximumDisplayedEntryDeltaE00).toBeNull();
    }
  });

  test("legacyPairV1: ineligible rows are retained, never silently retried", () => {
    const harness = makeHarness({ STUB_LOAD1: "7.50" });
    const { exitCode } = runScript(harness, ["--warmups", "0", "--blocks", "1"]);
    expect(exitCode).toBe(0);

    const rows = readRuns(harness);
    expect(rows).toHaveLength(4);
    for (const row of rows) {
      expect(row.accepted).toBe(true);
      expect(row.eligible).toBe(false);
    }
  });
});
