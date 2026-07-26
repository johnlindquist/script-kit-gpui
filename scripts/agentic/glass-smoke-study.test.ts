/**
 * WP7 (glass-smoke-harness-max-info): the multi-build study runner's
 * schedule algebra and fail-closed semantics.
 *
 * Blind spots locked here: N=2 must produce exactly A,B,B,A; N=3/N=4 must
 * balance temporal positions with exactly two appearances per build per
 * block; duplicate binary SHAs are rejected without an explicit A/A
 * declaration; a nonempty output directory is refused; one invalid attempt
 * poisons its ENTIRE block for paired inference and schedules a retry with
 * retryOfBlockId; warmups consolidate across the ladder instead of
 * repeating per pair.
 */

import { describe, expect, test } from "bun:test";
import { mkdtempSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  attemptSatisfiesSlot,
  type AttemptRow,
  blockInferenceValidity,
  mirroredCyclicSchedule,
  p95Bytes,
  parseDfFreeBytes,
  planScheduledSlots,
  requiredFreeStorageBytes,
  resolveManifest,
  scheduleRetryBlock,
  STORAGE_FLOOR_BYTES,
  validateManifest,
  warmupRounds,
} from "./glass-smoke-study.ts";

const MANIFEST = {
  schemaVersion: 1,
  studyId: "test-study",
  profile: "full",
  builds: [
    {
      id: "a",
      role: "baseline",
      binary: "bin-a",
      expected: { morphStartAlpha: 0.85 },
    },
    {
      id: "b",
      role: "candidate",
      binary: "bin-b",
      expected: { morphStartAlpha: 0.9 },
    },
  ],
  design: {
    type: "mirrored-cyclic",
    warmupsPerBuild: 3,
    requiredBlocks: 5,
    failureOnlyEarlyStop: true,
  },
  fixture: { mode: "saturated-stripes" },
};

describe("mirrored cyclic schedule", () => {
  test("N=2 block 0 is exactly A,B,B,A", () => {
    const [block] = mirroredCyclicSchedule(["A", "B"], 1);
    expect(block.slots).toEqual(["A", "B", "B", "A"]);
  });

  test("N=2 rotation alternates the lead build across blocks", () => {
    const blocks = mirroredCyclicSchedule(["A", "B"], 4);
    expect(blocks.map((block) => block.slots[0])).toEqual(["A", "B", "A", "B"]);
    for (const block of blocks) {
      expect(block.slots).toEqual([...block.forward, ...block.reverse]);
    }
  });

  for (const n of [3, 4]) {
    test(`N=${n}: every block has exactly two appearances per build and mirrored positions`, () => {
      const ids = Array.from({ length: n }, (_, i) => `b${i}`);
      const blocks = mirroredCyclicSchedule(ids, n * 2);
      for (const block of blocks) {
        expect(block.slots).toHaveLength(n * 2);
        for (const id of ids) {
          const positions = block.slots
            .map((slot, index) => (slot === id ? index : -1))
            .filter((index) => index >= 0);
          expect(positions).toHaveLength(2);
          // Mirrored: the two appearances are symmetric around the center.
          expect(positions[0] + positions[1]).toBe(n * 2 - 1);
        }
      }
      // Balance across blocks: each build leads the forward pass equally
      // often over a full rotation cycle.
      const leads = new Map<string, number>();
      for (const block of blocks) {
        leads.set(block.forward[0], (leads.get(block.forward[0]) ?? 0) + 1);
      }
      for (const id of ids) expect(leads.get(id)).toBe(2);
    });
  }
});

describe("consolidated warmups", () => {
  test("three rounds, every build once per round, rotated and alternated", () => {
    const rounds = warmupRounds(["a", "b", "c"], 3);
    expect(rounds).toHaveLength(3);
    for (const round of rounds) {
      expect([...round].sort()).toEqual(["a", "b", "c"]);
    }
    // Rotation + alternation change the temporal position of each build.
    expect(rounds[0]).toEqual(["a", "b", "c"]);
    expect(rounds[1]).toEqual(["a", "c", "b"]); // rotated by 1 then reversed
    expect(rounds[2]).toEqual(["c", "a", "b"]);
  });

  test("the ladder warms the baseline 3 times total, not 3 per pair", () => {
    const slots = planScheduledSlots({
      builds: [{ id: "base" }, { id: "c1" }, { id: "c2" }, { id: "c3" }],
      design: { warmupsPerBuild: 3, requiredBlocks: 5 },
    });
    const baseWarmups = slots.filter(
      (slot) => slot.kind === "warmup" && slot.buildId === "base",
    );
    expect(baseWarmups).toHaveLength(3);
  });

  test("13×N ladder economics: scheduled slots equal 2N per block", () => {
    const slots = planScheduledSlots({
      builds: [{ id: "a" }, { id: "b" }, { id: "c" }],
      design: { warmupsPerBuild: 3, requiredBlocks: 5 },
    });
    const scheduled = slots.filter((slot) => slot.kind === "scheduled");
    expect(scheduled).toHaveLength(5 * 2 * 3);
    const warmups = slots.filter((slot) => slot.kind === "warmup");
    expect(warmups).toHaveLength(3 * 3);
  });
});

describe("manifest validation", () => {
  test("the example-shaped manifest validates", () => {
    expect(validateManifest(MANIFEST)).toEqual([]);
  });

  test("warmups below three are rejected — never auto-reduced", () => {
    const manifest = structuredClone(MANIFEST);
    manifest.design.warmupsPerBuild = 2;
    expect(validateManifest(manifest).join(" ")).toContain(">= 3");
  });

  test("sentinel fixtures are rejected for statistics profiles", () => {
    const manifest = structuredClone(MANIFEST);
    manifest.fixture.mode = "dark-terminal";
    expect(validateManifest(manifest).join(" ")).toContain("sentinel backdrop");
    manifest.profile = "entry-color";
    expect(validateManifest(manifest)).toEqual([]);
  });

  test("declared alpha bits must equal the f64 bits of the declared alpha", () => {
    const manifest = structuredClone(MANIFEST);
    (manifest.builds[0].expected as any).morphStartAlphaBits =
      "3f66666666666666";
    expect(validateManifest(manifest).join(" ")).toContain(
      "does not equal the f64 bits",
    );
  });
});

describe("manifest resolution", () => {
  function writeBinaries(contents: Record<string, string>) {
    const root = mkdtempSync(join(tmpdir(), "glass-study-"));
    for (const [name, content] of Object.entries(contents)) {
      writeFileSync(join(root, name), content);
    }
    return root;
  }

  test("a missing binary fails BEFORE any fixture could start", () => {
    const root = writeBinaries({ "bin-a": "aaa" });
    const { errors } = resolveManifest(MANIFEST, { repoRoot: root });
    expect(errors.join(" ")).toContain("binary missing");
  });

  test("duplicate binary SHAs are rejected unless an A/A control is declared", () => {
    const root = writeBinaries({ "bin-a": "same", "bin-b": "same" });
    const { errors } = resolveManifest(MANIFEST, { repoRoot: root });
    expect(errors.join(" ")).toContain("share binary sha256");
    const aa = { ...structuredClone(MANIFEST), allowDuplicateBinarySha: true };
    const { errors: aaErrors, resolved } = resolveManifest(aa, {
      repoRoot: root,
    });
    expect(aaErrors).toEqual([]);
    expect(resolved.builds[0].expected.morphStartAlphaBits).toBe(
      "3feb333333333333",
    );
  });
});

describe("storage preflight arithmetic", () => {
  test("required bytes = p95 × slots × 1.25 + 5 GiB", () => {
    expect(requiredFreeStorageBytes(100, 10)).toBe(
      Math.ceil(100 * 10 * 1.25 + STORAGE_FLOOR_BYTES),
    );
    expect(p95Bytes([1, 2, 3, 100])).toBe(100);
    expect(p95Bytes([])).toBe(0);
  });
});

describe("attempt and block semantics", () => {
  const attempt = (overrides: Partial<AttemptRow>): AttemptRow => ({
    attemptId: "attempt-0001",
    slotId: "block0-s0-A",
    blockIndex: 0,
    buildId: "A",
    disposition: "EVALUABLE_PASS",
    loadEligible: true,
    thermalEligible: true,
    binaryHashStable: true,
    evaluable: true,
    ...overrides,
  });

  test("only an eligible evaluable attempt satisfies a slot", () => {
    expect(attemptSatisfiesSlot(attempt({}))).toBe(true);
    expect(attemptSatisfiesSlot(attempt({ loadEligible: false }))).toBe(false);
    expect(attemptSatisfiesSlot(attempt({ thermalEligible: false }))).toBe(false);
    expect(attemptSatisfiesSlot(attempt({ binaryHashStable: false }))).toBe(false);
    expect(attemptSatisfiesSlot(attempt({ evaluable: false }))).toBe(false);
    expect(
      attemptSatisfiesSlot(attempt({ disposition: "INVALID_INTERFERENCE" })),
    ).toBe(false);
    expect(
      attemptSatisfiesSlot(attempt({ disposition: "INVALID_SETUP" })),
    ).toBe(false);
  });

  test("an evaluable product FAILURE still satisfies its slot", () => {
    // Product failures are evidence (they can trigger failure-only early
    // stop); they never invalidate a block.
    expect(
      attemptSatisfiesSlot(attempt({ disposition: "EVALUABLE_FAIL" })),
    ).toBe(true);
  });

  test("one invalid attempt poisons the WHOLE block and schedules a retry", () => {
    const [block] = mirroredCyclicSchedule(["A", "B"], 1);
    const attemptsBySlot = new Map<string, AttemptRow[]>([
      ["block0-s0-A", [attempt({ slotId: "block0-s0-A" })]],
      [
        "block0-s1-B",
        [
          attempt({
            slotId: "block0-s1-B",
            buildId: "B",
            disposition: "INVALID_INTERFERENCE",
          }),
        ],
      ],
      ["block0-s2-B", [attempt({ slotId: "block0-s2-B", buildId: "B" })]],
      ["block0-s3-A", [attempt({ slotId: "block0-s3-A" })]],
    ]);
    const validity = blockInferenceValidity(block, attemptsBySlot);
    expect(validity.valid).toBe(false);
    expect(validity.reasons.join(" ")).toContain("block0-s1-B");
    const retry = scheduleRetryBlock(block, 7);
    expect(retry.retryOfBlockId).toBe(0);
    expect(retry.blockIndex).toBe(7);
    expect(retry.slots).toEqual(["A", "B", "B", "A"]);
  });

  test("retry chains count against the ROOT block, not the immediate parent", () => {
    // Regression for the 2026-07-26 runaway: keying the retry cap on the
    // immediate parent gives every generation a fresh zero counter, so a
    // persistently poisoned block retried ~175 times (1064 attempts) until
    // the disk filled. rootBlockIndex must survive arbitrary chain depth.
    const [block] = mirroredCyclicSchedule(["A", "B"], 1);
    const retry1 = scheduleRetryBlock(block, 10);
    expect(retry1.retryOfBlockId).toBe(block.blockIndex);
    expect(retry1.rootBlockIndex).toBe(block.blockIndex);
    const retry2 = scheduleRetryBlock(retry1, 11);
    expect(retry2.retryOfBlockId).toBe(10);
    expect(retry2.rootBlockIndex).toBe(block.blockIndex);
    const retry3 = scheduleRetryBlock(retry2, 12);
    expect(retry3.retryOfBlockId).toBe(11);
    expect(retry3.rootBlockIndex).toBe(block.blockIndex);
  });

  test("parseDfFreeBytes reads the df -k free column and fails closed", () => {
    const df =
      "Filesystem 1024-blocks      Used Available Capacity iused ifree %iused  Mounted on\n"
      + "/dev/disk3s5   971350180 838860800 108003328    89%     12M  1.1G    1%   /System/Volumes/Data";
    expect(parseDfFreeBytes(df)).toBe(108003328 * 1024);
    expect(parseDfFreeBytes("")).toBeNull();
    expect(parseDfFreeBytes("garbage output")).toBeNull();
  });

  test("a fully green block is valid for paired inference", () => {
    const [block] = mirroredCyclicSchedule(["A", "B"], 1);
    const attemptsBySlot = new Map<string, AttemptRow[]>(
      block.slots.map((buildId, position) => {
        const slotId = `block0-s${position}-${buildId}`;
        return [slotId, [attempt({ slotId, buildId })]];
      }),
    );
    expect(blockInferenceValidity(block, attemptsBySlot).valid).toBe(true);
  });

  test("a retried slot with a later good attempt satisfies the block", () => {
    const [block] = mirroredCyclicSchedule(["A", "B"], 1);
    const attemptsBySlot = new Map<string, AttemptRow[]>(
      block.slots.map((buildId, position) => {
        const slotId = `block0-s${position}-${buildId}`;
        return [slotId, [attempt({ slotId, buildId })]];
      }),
    );
    // The first attempt on slot 0 was invalid; a retained retry succeeded.
    attemptsBySlot.set("block0-s0-A", [
      attempt({ slotId: "block0-s0-A", disposition: "INVALID_OBSERVER" }),
      attempt({ slotId: "block0-s0-A", attemptId: "attempt-0009" }),
    ]);
    expect(blockInferenceValidity(block, attemptsBySlot).valid).toBe(true);
  });
});
