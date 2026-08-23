/**
 * PF-011 disposition-table lock (C08, cons-finish-six-lane lane 02).
 *
 * WHY: the previous glass probes let source-log geometry and under-resolved
 * rendered captures flow into a green verdict. These tests lock the exact
 * disposition table: INVALID_OBSERVER / INVALID_INTERFERENCE / EVALUABLE_FAIL
 * / EVALUABLE_PASS, the exit-code mapping (only EVALUABLE_PASS is exit 0),
 * and all eight synthetic negative controls.
 */

import { afterAll, describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  MAIN_GLASS_ENTRY_EXPECTATION,
  analyzeEntryMotionEnvelope,
} from "./glass-entry-motion-contract.ts";
import {
  classifyGlassObservation,
  classifySyntheticObservation,
  exitCodeForDisposition,
  hashProtectedPaths,
  type GlassObservationInput,
  REQUIRED_NOTES_PHASES,
  runSyntheticNegativeControls,
  syntheticValidMainEntryFrames,
  syntheticValidNotesPhaseRecords,
  validateNotesPhaseRecords,
  validateOwnedRenderedFrames,
} from "./glass-observers.ts";
import { LOCKED_GLASS_SOURCE_PATHS } from "./protected-sources.ts";

const SETTLED: [[number, number], [number, number]] = [[0, 0], [750, 501]];
const DISPLAY_PERIOD_NS = 8_333_333;

function cleanInput(): GlassObservationInput {
  return {
    captureHealthPass: true,
    helperErrors: [],
    fixtureErrors: [],
    identityErrors: [],
    ownerErrors: [],
    requiredPhaseErrors: [],
    cleanupErrors: [],
    interference: { validated: true, disposition: "EVALUABLE_PASS", errors: [] },
    rendered: { present: true, underResolved: false, pass: true, errors: [] },
  };
}

describe("disposition table", () => {
  test("hashes every extracted calibrated production and anti-drift owner", () => {
    const hashes = hashProtectedPaths();
    expect(Object.keys(hashes)).toEqual(expect.arrayContaining([...LOCKED_GLASS_SOURCE_PATHS]));
    for (const path of LOCKED_GLASS_SOURCE_PATHS) {
      expect(hashes[path]).toMatch(/^[a-f0-9]{64}$/);
    }
  });

  test("clean rendered evidence inside the envelope is EVALUABLE_PASS (exit 0)", () => {
    const classified = classifyGlassObservation(cleanInput());
    expect(classified.disposition).toBe("EVALUABLE_PASS");
    expect(classified.pass).toBe(true);
    expect(exitCodeForDisposition(classified.disposition)).toBe(0);
  });

  test("valid rendered evidence outside the envelope is EVALUABLE_FAIL (exit 2)", () => {
    const input = cleanInput();
    input.rendered = {
      present: true,
      underResolved: false,
      pass: false,
      errors: ["grow-in maximum width scale 1.0500 outside 1.007–1.019"],
    };
    const classified = classifyGlassObservation(input);
    expect(classified.disposition).toBe("EVALUABLE_FAIL");
    expect(classified.productErrors).toContain(
      "grow-in maximum width scale 1.0500 outside 1.007–1.019",
    );
    expect(exitCodeForDisposition(classified.disposition)).toBe(2);
  });

  test("under-resolved rendered evidence is INVALID_OBSERVER, never a fail", () => {
    const input = cleanInput();
    input.rendered = { present: true, underResolved: true, pass: false, errors: [] };
    const classified = classifyGlassObservation(input);
    expect(classified.disposition).toBe("INVALID_OBSERVER");
    expect(classified.productErrors).toEqual([]);
    expect(exitCodeForDisposition(classified.disposition)).toBe(4);
  });

  test("absent rendered evidence is INVALID_OBSERVER even with green everything else", () => {
    const input = cleanInput();
    input.rendered = { present: false, underResolved: false, pass: true, errors: [] };
    expect(classifyGlassObservation(input).disposition).toBe("INVALID_OBSERVER");
  });

  test("a VALID interference monitor dominates every other defect", () => {
    const input = cleanInput();
    input.ownerErrors = ["two candidate owners"];
    input.interference = {
      validated: true,
      disposition: "INVALID_INTERFERENCE",
      errors: ["untagged keyboard or pointer input observed"],
    };
    const classified = classifyGlassObservation(input);
    expect(classified.disposition).toBe("INVALID_INTERFERENCE");
    expect(exitCodeForDisposition(classified.disposition)).toBe(4);
  });

  test("an INVALID monitor is an observer failure, not interference", () => {
    const input = cleanInput();
    input.interference = { validated: false, disposition: null, errors: [] };
    expect(classifyGlassObservation(input).disposition).toBe("INVALID_OBSERVER");
  });

  test("sourceDiagnostics can NEVER upgrade invalid rendered evidence", () => {
    const input = cleanInput();
    input.rendered = { present: true, underResolved: true, pass: false, errors: [] };
    input.sourceDiagnostics = {
      loggedGeometryDiagnostic: { pass: true },
      onset: { present: true, supported: true },
      runtimeContract: { pass: true },
    };
    const classified = classifyGlassObservation(input);
    expect(classified.disposition).toBe("INVALID_OBSERVER");
    expect(classified.pass).toBe(false);
  });

  test("exit-code mapping: only EVALUABLE_PASS is zero", () => {
    expect(exitCodeForDisposition("EVALUABLE_PASS")).toBe(0);
    expect(exitCodeForDisposition("EVALUABLE_FAIL")).toBe(2);
    expect(exitCodeForDisposition("INVALID_OBSERVER")).toBe(4);
    expect(exitCodeForDisposition("INVALID_INTERFERENCE")).toBe(4);
    expect(exitCodeForDisposition("INVALID_BINARY")).toBe(4);
    expect(exitCodeForDisposition("INVALID_FIXTURE")).toBe(4);
    expect(exitCodeForDisposition("INVALID_CLEANUP")).toBe(4);
    expect(exitCodeForDisposition("BLOCKED_TIMEOUT")).toBe(3);
    expect(exitCodeForDisposition("BLOCKED_MISSING_PRIMITIVE")).toBe(3);
  });
});

describe("locked envelope integration (evaluator unchanged)", () => {
  test("the synthetic valid frame set passes the LOCKED evaluator", () => {
    const envelope = analyzeEntryMotionEnvelope(
      syntheticValidMainEntryFrames(),
      SETTLED,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(envelope.underResolved).toBe(false);
    expect(envelope.pass).toBe(true);
    expect(envelope.firstVisible?.widthScale).toBeCloseTo(1.0305, 6);
    expect(envelope.onsetTailVisible?.widthScale).toBeCloseTo(1.012, 6);
  });

  test("a single rendered frame is under-resolved by the LOCKED evaluator", () => {
    const envelope = analyzeEntryMotionEnvelope(
      syntheticValidMainEntryFrames().slice(0, 1),
      SETTLED,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(envelope.underResolved).toBe(true);
    expect(envelope.pass).toBe(false);
  });

  test("three distinct widths are under-resolved by the LOCKED evaluator", () => {
    const frames = syntheticValidMainEntryFrames().map((frame, index) => ({
      ...frame,
      windowBounds: [
        [0, 0],
        [750 * [1.012, 1.012, 0.987, 0.987, 1, 1][index]!, 501],
      ] as [[number, number], [number, number]],
    }));
    const envelope = analyzeEntryMotionEnvelope(
      frames,
      SETTLED,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    expect(envelope.distinctWidths).toBe(3);
    expect(envelope.underResolved).toBe(true);
  });
});

describe("frame ownership", () => {
  test("every owned frame must bind to the one derived owner", () => {
    const frames = syntheticValidMainEntryFrames(77);
    expect(validateOwnedRenderedFrames(frames, 77)).toEqual([]);
    frames[2] = { ...frames[2], actualWindowID: 9999 };
    const errors = validateOwnedRenderedFrames(frames, 77);
    expect(errors.length).toBe(1);
    expect(errors[0]).toContain("frame 2");
  });

  test("a missing owner id fails closed", () => {
    expect(validateOwnedRenderedFrames([], Number.NaN)).toEqual([
      "expected native owner window ID is missing",
    ]);
  });
});

describe("Notes phase records", () => {
  test("the four required phases validate when complete and conforming", () => {
    const records = syntheticValidNotesPhaseRecords(88);
    expect(
      validateNotesPhaseRecords(
        records as unknown as Array<Record<string, unknown>>,
        88,
        DISPLAY_PERIOD_NS,
      ),
    ).toEqual([]);
    expect(REQUIRED_NOTES_PHASES).toEqual([
      "preMask",
      "materialSafeAnchor",
      "postBodyReveal",
      "settled",
    ]);
  });

  test("a missing postBodyReveal phase is an observer error", () => {
    const records = syntheticValidNotesPhaseRecords(88).filter(
      (record) => record.name !== "postBodyReveal",
    );
    const errors = validateNotesPhaseRecords(
      records as unknown as Array<Record<string, unknown>>,
      88,
      DISPLAY_PERIOD_NS,
    );
    expect(errors.join("\n")).toContain(
      "postBodyReveal: expected exactly one record, observed 0",
    );
  });

  test("a wrong owner, unordered times, and shared frames are rejected", () => {
    const records = syntheticValidNotesPhaseRecords(88);
    records[1] = { ...records[1], actualWindowId: 9 };
    records[3] = { ...records[3], displayTimeNs: 1 };
    records[2] = { ...records[2], frameSequence: 0 };
    const errors = validateNotesPhaseRecords(
      records as unknown as Array<Record<string, unknown>>,
      88,
      DISPLAY_PERIOD_NS,
    );
    expect(errors).toContain("materialSafeAnchor: wrong native window owner");
    expect(errors).toContain("Notes phase display times are not monotone");
    expect(errors).toContain(
      "preMask and postBodyReveal are not distinct rendered frames",
    );
  });

  test("an anchor without a rendered frame inside one display period fails", () => {
    const records = syntheticValidNotesPhaseRecords(88);
    records[1] = {
      ...records[1],
      hostTimeNs: 1_062_000_000,
      displayTimeNs: 1_062_000_000 + DISPLAY_PERIOD_NS + 1,
    };
    const errors = validateNotesPhaseRecords(
      records as unknown as Array<Record<string, unknown>>,
      88,
      DISPLAY_PERIOD_NS,
    );
    expect(errors).toContain(
      "materialSafeAnchor: no rendered frame within one display period",
    );
  });

  test("a settled frame before the runtime settle deadline fails", () => {
    const records = syntheticValidNotesPhaseRecords(88);
    const errors = validateNotesPhaseRecords(
      records as unknown as Array<Record<string, unknown>>,
      88,
      DISPLAY_PERIOD_NS,
      { settleDeadlineNs: 2_000_000_000 },
    );
    expect(errors).toContain(
      "settled: frame precedes the runtime settle deadline",
    );
  });
});

describe("synthetic negative controls", () => {
  const scratch = mkdtempSync(join(tmpdir(), "pf011-negative-tests-"));
  afterAll(() => rmSync(scratch, { recursive: true, force: true }));

  const results = runSyntheticNegativeControls(scratch);

  test("exactly eight controls, each with the required disposition and a nonzero exit", () => {
    const expected: Record<string, string> = {
      "single-frame": "INVALID_OBSERVER",
      "too-few-widths": "INVALID_OBSERVER",
      "wrong-window-id": "INVALID_OBSERVER",
      "multiple-candidate-owners": "INVALID_OBSERVER",
      "missing-notes-phase": "INVALID_OBSERVER",
      "helper-hash-mismatch": "INVALID_OBSERVER",
      "injected-interference": "INVALID_INTERFERENCE",
      "source-green-rendered-invalid": "INVALID_OBSERVER",
    };
    expect(results.map((result) => result.id).sort()).toEqual(
      Object.keys(expected).sort(),
    );
    for (const result of results) {
      expect(result.actualDisposition).toBe(
        expected[result.id] as typeof result.actualDisposition,
      );
      expect(result.subjectExitCode).not.toBe(0);
      expect(result.pass).toBe(true);
      const receipt = JSON.parse(readFileSync(result.receiptPath, "utf8"));
      expect(receipt.id).toBe(result.id);
      expect(receipt.classification.disposition).toBe(expected[result.id]);
      expect(receipt.pass).toBe(true);
    }
  });

  test("no negative control can ever be EVALUABLE_PASS", () => {
    for (const result of results) {
      expect(result.actualDisposition).not.toBe("EVALUABLE_PASS");
    }
  });

  test("classifySyntheticObservation carries the disposition-mapped exit code", () => {
    const input = cleanInput();
    expect(classifySyntheticObservation(input).exitCode).toBe(0);
    input.rendered.pass = false;
    expect(classifySyntheticObservation(input).exitCode).toBe(2);
    input.rendered.underResolved = true;
    expect(classifySyntheticObservation(input).exitCode).toBe(4);
  });
});

describe("classify-synthetic CLI gate", () => {
  test("refuses to run without SCRIPT_KIT_TEST_STATUS=1", async () => {
    const child = Bun.spawn(
      [
        "bun",
        join(import.meta.dir, "glass-observers.ts"),
        "classify-synthetic",
        "--input",
        "/dev/null",
        "--out",
        "/dev/null",
      ],
      {
        stdout: "pipe",
        stderr: "pipe",
        env: { ...process.env, SCRIPT_KIT_TEST_STATUS: "" },
      },
    );
    const [stderr, exitCode] = await Promise.all([
      new Response(child.stderr).text(),
      child.exited,
    ]);
    expect(exitCode).toBe(64);
    expect(stderr).toContain("test-status-only");
  });

  test("classifies a synthetic input file under test status", async () => {
    const inputPath = join(scratchDirForCli, "input.json");
    const outPath = join(scratchDirForCli, "out.json");
    const input = cleanInput();
    input.rendered = { present: true, underResolved: true, pass: false, errors: [] };
    await Bun.write(inputPath, JSON.stringify(input));
    const child = Bun.spawn(
      [
        "bun",
        join(import.meta.dir, "glass-observers.ts"),
        "classify-synthetic",
        "--input",
        inputPath,
        "--out",
        outPath,
      ],
      {
        stdout: "pipe",
        stderr: "pipe",
        env: { ...process.env, SCRIPT_KIT_TEST_STATUS: "1" },
      },
    );
    const exitCode = await child.exited;
    expect(exitCode).toBe(4);
    const receipt = JSON.parse(readFileSync(outPath, "utf8"));
    expect(receipt.classification.disposition).toBe("INVALID_OBSERVER");
  });
});

const scratchDirForCli = mkdtempSync(join(tmpdir(), "pf011-cli-tests-"));
afterAll(() => rmSync(scratchDirForCli, { recursive: true, force: true }));
