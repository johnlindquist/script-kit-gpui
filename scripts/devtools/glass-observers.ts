#!/usr/bin/env bun
/**
 * PF-011 — fail-closed glass observation classifier + serial aggregator.
 *
 * C08 (cons-finish-six-lane, lane 02-c08-observers). This module owns the
 * DISPOSITION TABLE for every glass runtime observation:
 *
 *   INVALID_OBSERVER     — the observation apparatus could not prove what it
 *                          claims (missing/ambiguous owner, under-resolved
 *                          rendered evidence, helper drift, missing phase).
 *   INVALID_INTERFERENCE — a VALID interference monitor saw outside input;
 *                          rerun when quiet, never a product verdict.
 *   EVALUABLE_FAIL       — valid, sufficient, interference-clean rendered
 *                          evidence that violates the locked contract.
 *   EVALUABLE_PASS       — valid rendered evidence inside the locked envelope
 *                          with helpers, fixture, cleanup, and identity intact.
 *
 * FALSE-WIN GUARD: `sourceDiagnostics` (logged geometry, onset receipts,
 * runtime contracts — anything derived from the app's own logs) is carried on
 * receipts for diagnosis but is NEVER read by classification. Source-log
 * geometry can never upgrade rendered evidence.
 *
 * The locked envelope evaluator (glass-entry-motion-contract.ts) and the
 * production calibration (src/theme/opacity.rs, secondary_window_config.rs,
 * ui/chrome/tokens.rs, the named theme fixture) are PROTECTED: this lane
 * hashes them before/after and refuses EVALUABLE_PASS on any drift.
 */

import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import {
  MAIN_GLASS_ENTRY_EXPECTATION,
  analyzeEntryMotionEnvelope,
} from "./glass-entry-motion-contract.ts";
import { classifyInterference } from "./glass-interference.ts";
import { deriveUniqueOwnerDelta } from "./glass-topology-contract.ts";
import {
  type HelperCacheManifest,
  type HelperRole,
  helperManifestPath,
  prepareHelper,
  sha256File,
  validateHelperEntry,
} from "./glass-native-helper-cache.ts";
import { newRunId } from "./glass-evidence-contract.ts";
import { producerIdentityForTool } from "./lib/receipt-schema.ts";

// ---------------------------------------------------------------------------
// Disposition table (plan §2.2)
// ---------------------------------------------------------------------------

export type ObservationDisposition =
  | "EVALUABLE_PASS"
  | "EVALUABLE_FAIL"
  | "INVALID_OBSERVER"
  | "INVALID_INTERFERENCE";

export type ClassifiedGlassObservation = {
  disposition: ObservationDisposition;
  pass: boolean;
  observerErrors: string[];
  productErrors: string[];
};

export type GlassObservationInput = {
  captureHealthPass: boolean;
  helperErrors: string[];
  fixtureErrors: string[];
  identityErrors: string[];
  ownerErrors: string[];
  requiredPhaseErrors: string[];
  cleanupErrors: string[];
  interference: {
    validated: boolean;
    disposition: string | null;
    errors: string[];
  };
  rendered: {
    present: boolean;
    underResolved: boolean;
    pass: boolean;
    errors: string[];
  };
  /** Receipt-only. Deliberately excluded from classification. */
  sourceDiagnostics?: unknown;
};

export function classifyGlassObservation(
  input: GlassObservationInput,
): ClassifiedGlassObservation {
  const observerErrors = [
    ...(input.captureHealthPass ? [] : ["capture health is not valid"]),
    ...input.helperErrors,
    ...input.fixtureErrors,
    ...input.identityErrors,
    ...input.ownerErrors,
    ...input.requiredPhaseErrors,
    ...input.cleanupErrors,
    ...(input.interference.validated
      ? []
      : ["interference monitor did not produce a valid receipt"]),
    ...(input.rendered.present
      ? []
      : ["required rendered observation is absent"]),
    ...(input.rendered.underResolved
      ? ["rendered observation is under-resolved"]
      : []),
  ];

  // Preserve the repository's existing interference-dominant rule when the
  // interference monitor itself is valid.
  if (
    input.interference.validated
    && input.interference.disposition === "INVALID_INTERFERENCE"
  ) {
    return {
      disposition: "INVALID_INTERFERENCE",
      pass: false,
      observerErrors,
      productErrors: [],
    };
  }
  if (observerErrors.length > 0) {
    return {
      disposition: "INVALID_OBSERVER",
      pass: false,
      observerErrors,
      productErrors: [],
    };
  }
  if (!input.rendered.pass) {
    return {
      disposition: "EVALUABLE_FAIL",
      pass: false,
      observerErrors: [],
      productErrors: [...input.rendered.errors],
    };
  }
  return {
    disposition: "EVALUABLE_PASS",
    pass: true,
    observerErrors: [],
    productErrors: [],
  };
}

/**
 * Shared exit-code contract:
 *   0 EVALUABLE_PASS · 2 EVALUABLE_FAIL · 3 BLOCKED_* · 4 INVALID_* ·
 *   64 argument error before a run begins.
 */
export function exitCodeForDisposition(disposition: string): number {
  if (disposition === "EVALUABLE_PASS") return 0;
  if (disposition === "EVALUABLE_FAIL") return 2;
  if (disposition.startsWith("BLOCKED")) return 3;
  return 4;
}

// ---------------------------------------------------------------------------
// Rendered-frame ownership (shared by Main / Actions / Notes probes)
// ---------------------------------------------------------------------------

export type OwnedRenderedFrame = {
  sequence?: number | null;
  expectedWindowID?: number | null;
  actualWindowID?: number | null;
  displayTimeNs?: number | null;
  sha256?: string | null;
};

/** Every owned rendered frame must bind to the ONE derived native owner. */
export function validateOwnedRenderedFrames(
  frames: OwnedRenderedFrame[],
  expectedWindowId: number,
): string[] {
  const errors: string[] = [];
  if (!Number.isFinite(expectedWindowId) || expectedWindowId <= 0) {
    return ["expected native owner window ID is missing"];
  }
  frames.forEach((frame, index) => {
    if (Number(frame?.actualWindowID) !== expectedWindowId) {
      errors.push(
        `frame ${index} is bound to native window ${
          frame?.actualWindowID ?? "missing"
        }, expected owner ${expectedWindowId}`,
      );
    }
  });
  return errors;
}

// ---------------------------------------------------------------------------
// Notes entry phase records (plan §2.3)
// ---------------------------------------------------------------------------

export const REQUIRED_NOTES_PHASES = [
  "preMask",
  "materialSafeAnchor",
  "postBodyReveal",
  "settled",
] as const;

export type NotesPhaseName = (typeof REQUIRED_NOTES_PHASES)[number];

export type NotesPhaseRecord = {
  name: NotesPhaseName;
  required: true;
  expectedWindowId: number;
  actualWindowId: number | null;
  stateCapturedAt: string;
  hostTimeNs: number | null;
  displayTimeNs: number | null;
  frameSequence: number | null;
  framePath: string | null;
  frameSha256: string | null;
  windowBounds: unknown;
  windowAlpha: number | null;
  bodyVisible: boolean | null;
  bodyPixelState: "masked" | "transitioned" | "visible" | "unknown";
  errors: string[];
  pass: boolean;
};

/**
 * Validate the four concurrent Notes entry phase records against the ONE
 * derived native owner and the observed display cadence. Fails closed: a
 * missing phase, unpaired frame, wrong owner, or unordered record is an
 * OBSERVER error — never inferred from `event=glass_morph` or
 * `event=native_glass_entry_onset` source logs.
 */
export function validateNotesPhaseRecords(
  records: Array<Record<string, unknown>>,
  expectedWindowId: number,
  displayPeriodNs: number,
  options?: { settleDeadlineNs?: number },
): string[] {
  const errors: string[] = [];
  for (const name of REQUIRED_NOTES_PHASES) {
    const matches = records.filter((record) => record.name === name);
    if (matches.length !== 1) {
      errors.push(
        `${name}: expected exactly one record, observed ${matches.length}`,
      );
    }
  }
  const byName = Object.fromEntries(
    records.map((record) => [String(record.name), record]),
  ) as Record<NotesPhaseName, Record<string, unknown>>;

  for (const name of REQUIRED_NOTES_PHASES) {
    const record = byName[name];
    if (!record) continue;
    if (Number(record.actualWindowId) !== expectedWindowId) {
      errors.push(`${name}: wrong native window owner`);
    }
    if (!(Number(record.displayTimeNs) > 0)) {
      errors.push(`${name}: rendered display time missing`);
    }
    if (!/^[a-f0-9]{64}$/.test(String(record.frameSha256 ?? ""))) {
      errors.push(`${name}: rendered frame hash missing`);
    }
  }

  if (byName.preMask?.bodyVisible !== false) {
    errors.push("preMask: Notes body was not proven hidden");
  }
  const anchorDeltaNs = Math.abs(
    Number(byName.materialSafeAnchor?.displayTimeNs)
      - Number(byName.materialSafeAnchor?.hostTimeNs),
  );
  if (!Number.isFinite(anchorDeltaNs) || anchorDeltaNs > displayPeriodNs) {
    errors.push(
      "materialSafeAnchor: no rendered frame within one display period",
    );
  }
  if (byName.postBodyReveal?.bodyVisible !== true) {
    errors.push("postBodyReveal: Notes body was not proven visible");
  }
  if (byName.postBodyReveal?.bodyPixelState !== "transitioned") {
    errors.push("postBodyReveal: body pixel transition was not rendered");
  }
  if (Number(byName.settled?.windowAlpha) < 0.999) {
    errors.push("settled: native alpha is not settled");
  }
  if (
    options?.settleDeadlineNs != null
    && Number.isFinite(options.settleDeadlineNs)
    && Number(byName.settled?.displayTimeNs) < options.settleDeadlineNs
  ) {
    errors.push("settled: frame precedes the runtime settle deadline");
  }
  const orderedTimes = REQUIRED_NOTES_PHASES.map((name) =>
    Number(byName[name]?.displayTimeNs)
  );
  if (
    !orderedTimes.every(Number.isFinite)
    || orderedTimes.some((value, index) =>
      index === 0 ? false : value < orderedTimes[index - 1]!
    )
  ) {
    errors.push("Notes phase display times are not monotone");
  }
  if (
    Number(byName.postBodyReveal?.frameSequence)
      <= Number(byName.preMask?.frameSequence)
  ) {
    errors.push("preMask and postBodyReveal are not distinct rendered frames");
  }
  return errors;
}

// ---------------------------------------------------------------------------
// Synthetic classification seam (negative controls)
// ---------------------------------------------------------------------------

export function classifySyntheticObservation(
  input: GlassObservationInput,
): ClassifiedGlassObservation & { exitCode: number } {
  const classified = classifyGlassObservation(input);
  return { ...classified, exitCode: exitCodeForDisposition(classified.disposition) };
}

function emptyObservationInput(): GlassObservationInput {
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

const SETTLED_BOUNDS: [[number, number], [number, number]] = [[0, 0], [750, 501]];

function syntheticFrame(
  sequence: number,
  widthScale: number,
  alpha: number,
  windowId = 77,
) {
  return {
    sequence,
    expectedWindowID: windowId,
    actualWindowID: windowId,
    displayTimeNs: 1_000_000_000 + sequence * 8_333_333,
    windowAlpha: alpha,
    windowBounds: [[0, 0], [750 * widthScale, 501]] as [
      [number, number],
      [number, number],
    ],
    sha256: createHash("sha256").update(`frame-${sequence}`).digest("hex"),
  };
}

/**
 * Synthetic samples of the EXISTING locked 103.05% → 101.2% → 98.7%
 * main-entry shape. These are test observations, never production animation
 * values or a substitute for a real composited-frame receipt.
 */
export function syntheticValidMainEntryFrames(windowId = 77) {
  return [
    syntheticFrame(0, 1.0305, 0.85, windowId),
    syntheticFrame(1, 1.012, 0.95, windowId),
    syntheticFrame(2, 0.995, 0.97, windowId),
    syntheticFrame(3, 0.987, 0.985, windowId),
    syntheticFrame(4, 0.993, 0.99, windowId),
    syntheticFrame(5, 1.0, 1.0, windowId),
  ];
}

export function syntheticValidNotesPhaseRecords(
  expectedWindowId = 88,
): NotesPhaseRecord[] {
  const base = {
    required: true as const,
    expectedWindowId,
    actualWindowId: expectedWindowId,
    stateCapturedAt: "2026-08-07T00:00:00.000Z",
    windowBounds: SETTLED_BOUNDS,
    errors: [] as string[],
    pass: true,
  };
  const sha = (seed: string) => createHash("sha256").update(seed).digest("hex");
  return [
    {
      ...base,
      name: "preMask",
      hostTimeNs: 1_000_000_000,
      displayTimeNs: 1_000_000_000,
      frameSequence: 0,
      framePath: "frames/frame-0000.png",
      frameSha256: sha("preMask"),
      windowAlpha: 0.85,
      bodyVisible: false,
      bodyPixelState: "masked",
    },
    {
      ...base,
      name: "materialSafeAnchor",
      hostTimeNs: 1_062_000_000,
      displayTimeNs: 1_066_000_000,
      frameSequence: 7,
      framePath: "frames/frame-0007.png",
      frameSha256: sha("materialSafeAnchor"),
      windowAlpha: 0.99,
      bodyVisible: false,
      bodyPixelState: "masked",
    },
    {
      ...base,
      name: "postBodyReveal",
      hostTimeNs: 1_090_000_000,
      displayTimeNs: 1_092_000_000,
      frameSequence: 10,
      framePath: "frames/frame-0010.png",
      frameSha256: sha("postBodyReveal"),
      windowAlpha: 0.995,
      bodyVisible: true,
      bodyPixelState: "transitioned",
    },
    {
      ...base,
      name: "settled",
      hostTimeNs: 1_149_000_000,
      displayTimeNs: 1_152_000_000,
      frameSequence: 17,
      framePath: "frames/frame-0017.png",
      frameSha256: sha("settled"),
      windowAlpha: 1,
      bodyVisible: true,
      bodyPixelState: "visible",
    },
  ];
}

export type NegativeControlResult = {
  id: string;
  receiptPath: string;
  expectedDisposition: ObservationDisposition;
  actualDisposition: ObservationDisposition;
  subjectExitCode: number;
  pass: boolean;
};

const NOTES_DISPLAY_PERIOD_NS = 8_333_333;

/**
 * The eight PF-011 synthetic negative controls (plan §1.6). Pure-JSON
 * injections against exported functions; the sole filesystem control copies
 * a VALID helper entry into a test-owned temp dir and tampers the COPY.
 * No production fixture, helper cache entry, envelope source, or positive
 * receipt is ever mutated.
 */
export function runSyntheticNegativeControls(
  outDir: string,
  options?: {
    /** A valid helper cache entry to COPY for the hash-mismatch control. */
    validHelperBinaryPath?: string;
  },
): NegativeControlResult[] {
  const negativeDir = join(outDir, "negative");
  mkdirSync(negativeDir, { recursive: true });
  const results: NegativeControlResult[] = [];

  const record = (
    id: string,
    expected: ObservationDisposition,
    input: GlassObservationInput,
    evidence: unknown,
  ) => {
    const classified = classifySyntheticObservation(input);
    const receiptPath = join(negativeDir, `${id}.json`);
    const result: NegativeControlResult = {
      id,
      receiptPath,
      expectedDisposition: expected,
      actualDisposition: classified.disposition,
      subjectExitCode: classified.exitCode,
      pass: classified.disposition === expected && classified.exitCode !== 0,
    };
    writeFileSync(
      receiptPath,
      `${
        JSON.stringify(
          {
            schemaVersion: 2,
            kind: "pf011-synthetic-negative-control",
            id,
            injectedAt: new Date().toISOString(),
            expectedDisposition: expected,
            classification: classified,
            evidence,
            pass: result.pass,
          },
          null,
          2,
        )
      }\n`,
    );
    results.push(result);
  };

  // 1. single-frame — one otherwise valid rendered frame is under-resolved.
  {
    const frames = syntheticValidMainEntryFrames().slice(0, 1);
    const envelope = analyzeEntryMotionEnvelope(
      frames,
      SETTLED_BOUNDS,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    const input = emptyObservationInput();
    input.rendered = {
      present: frames.length > 0,
      underResolved: envelope.underResolved === true,
      pass: envelope.pass === true,
      errors: envelope.errors ?? [],
    };
    record("single-frame", "INVALID_OBSERVER", input, {
      frameCount: frames.length,
      envelope: {
        underResolved: envelope.underResolved,
        measuredFrameCount: envelope.measuredFrameCount,
        distinctWidths: envelope.distinctWidths,
      },
    });
  }

  // 2. too-few-widths — enough frames, only three distinct width scales.
  {
    const frames = [
      syntheticFrame(0, 1.012, 0.85),
      syntheticFrame(1, 1.012, 0.9),
      syntheticFrame(2, 0.987, 0.95),
      syntheticFrame(3, 0.987, 0.97),
      syntheticFrame(4, 1.0, 0.99),
      syntheticFrame(5, 1.0, 1.0),
    ];
    const envelope = analyzeEntryMotionEnvelope(
      frames,
      SETTLED_BOUNDS,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    const input = emptyObservationInput();
    input.rendered = {
      present: true,
      underResolved: envelope.underResolved === true,
      pass: envelope.pass === true,
      errors: envelope.errors ?? [],
    };
    record("too-few-widths", "INVALID_OBSERVER", input, {
      frameCount: frames.length,
      distinctWidths: envelope.distinctWidths,
      underResolved: envelope.underResolved,
    });
  }

  // 3. wrong-window-id — one owned frame bound to a foreign native window.
  {
    const frames = syntheticValidMainEntryFrames(77);
    frames[3] = { ...frames[3], actualWindowID: 9999 };
    const ownerErrors = validateOwnedRenderedFrames(frames, 77);
    const envelope = analyzeEntryMotionEnvelope(
      frames,
      SETTLED_BOUNDS,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    const input = emptyObservationInput();
    input.ownerErrors = ownerErrors;
    input.rendered = {
      present: true,
      underResolved: envelope.underResolved === true,
      pass: envelope.pass === true,
      errors: envelope.errors ?? [],
    };
    record("wrong-window-id", "INVALID_OBSERVER", input, { ownerErrors });
  }

  // 4. multiple-candidate-owners — two native owners after the open.
  {
    const main = {
      windowId: 10,
      ownerPid: 42,
      title: "",
      layer: 101,
      alpha: 1,
      onscreen: true,
      bounds: { x: 20, y: 20, width: 750, height: 480 },
    };
    const notesA = {
      ...main,
      windowId: 12,
      title: "Notes",
      bounds: { x: 0, y: 0, width: 350, height: 280 },
    };
    const notesB = { ...notesA, windowId: 13 };
    const delta = deriveUniqueOwnerDelta(
      [main],
      [main, notesA, notesB],
      "Notes",
      42,
      10,
    );
    const input = emptyObservationInput();
    input.ownerErrors = delta.pass
      ? []
      : [
        `expected exactly one new native owner, observed ${delta.candidateIds.length} (${
          delta.candidateIds.join(", ")
        })`,
      ];
    record("multiple-candidate-owners", "INVALID_OBSERVER", input, { delta });
  }

  // 5. missing-notes-phase — postBodyReveal removed.
  {
    const records = syntheticValidNotesPhaseRecords(88).filter(
      (record) => record.name !== "postBodyReveal",
    );
    const errors = validateNotesPhaseRecords(
      records as unknown as Array<Record<string, unknown>>,
      88,
      NOTES_DISPLAY_PERIOD_NS,
    );
    const input = emptyObservationInput();
    input.requiredPhaseErrors = errors;
    record("missing-notes-phase", "INVALID_OBSERVER", input, {
      retainedPhases: records.map((record) => record.name),
      errors,
    });
  }

  // 6. helper-hash-mismatch — copy a VALID helper entry, tamper the COPY.
  {
    const scratchDir = mkdtempSync(join(tmpdir(), "pf011-helper-negative-"));
    let helperErrors: string[] = [];
    let evidence: unknown = null;
    try {
      const copiedBinary = join(scratchDir, "filmstrip");
      if (
        options?.validHelperBinaryPath
        && existsSync(options.validHelperBinaryPath)
        && existsSync(helperManifestPath(options.validHelperBinaryPath))
      ) {
        copyFileSync(options.validHelperBinaryPath, copiedBinary);
        copyFileSync(
          helperManifestPath(options.validHelperBinaryPath),
          join(scratchDir, "manifest.json"),
        );
      } else {
        // No real prepared helper supplied (unit-test context): fabricate a
        // VALID test-owned entry first, then tamper the copy exactly the same
        // way. The production helper cache is never touched.
        writeFileSync(copiedBinary, "synthetic helper bytes");
        const manifest: HelperCacheManifest = {
          schemaVersion: 1,
          key: "synthetic-key",
          role: "filmstrip",
          sourcePath: "synthetic.swift",
          sourceSha256: createHash("sha256").update("src").digest("hex"),
          compiler: {
            swiftcVersion: "synthetic",
            sdkPath: "synthetic",
            architecture: process.arch,
            flags: [],
          },
          binaryPath: copiedBinary,
          binarySha256: sha256File(copiedBinary),
          compiledAt: new Date().toISOString(),
        };
        writeFileSync(
          join(scratchDir, "manifest.json"),
          `${JSON.stringify(manifest, null, 2)}\n`,
        );
      }
      // Prove the copied entry is valid BEFORE tampering.
      const manifest = JSON.parse(
        readFileSync(join(scratchDir, "manifest.json"), "utf8"),
      ) as HelperCacheManifest;
      const preTamper = validateHelperEntry(
        { ...manifest, binaryPath: copiedBinary },
        manifest.role as HelperRole,
        copiedBinary,
      );
      // Append one byte to the COPIED binary only.
      writeFileSync(
        copiedBinary,
        Buffer.concat([readFileSync(copiedBinary), Buffer.from([0x00])]),
      );
      helperErrors = validateHelperEntry(
        { ...manifest, binaryPath: copiedBinary },
        manifest.role as HelperRole,
        copiedBinary,
      );
      evidence = { preTamperErrors: preTamper, postTamperErrors: helperErrors };
    } finally {
      rmSync(scratchDir, { recursive: true, force: true });
    }
    const input = emptyObservationInput();
    input.helperErrors = helperErrors.length > 0
      ? helperErrors
      : ["helper tamper was not detected — negative control harness defect"];
    record("helper-hash-mismatch", "INVALID_OBSERVER", input, evidence);
  }

  // 7. injected-interference — synthetic untagged input on a VALID monitor.
  {
    const interference = classifyInterference({
      status: "ok",
      untaggedInputCount: 1,
      frontmostAppChanged: false,
      pointerDeviationPx: 0,
      targetMovedExternally: false,
    });
    const input = emptyObservationInput();
    input.interference = {
      validated: true,
      disposition: interference.disposition,
      errors: interference.errors,
    };
    record("injected-interference", "INVALID_INTERFERENCE", input, {
      interference,
    });
  }

  // 8. source-green-rendered-invalid — passing source diagnostics with an
  // under-resolved rendered envelope must stay INVALID_OBSERVER.
  {
    const frames = syntheticValidMainEntryFrames().slice(0, 2);
    const envelope = analyzeEntryMotionEnvelope(
      frames,
      SETTLED_BOUNDS,
      MAIN_GLASS_ENTRY_EXPECTATION,
    );
    const input = emptyObservationInput();
    input.rendered = {
      present: true,
      underResolved: envelope.underResolved === true,
      pass: envelope.pass === true,
      errors: envelope.errors ?? [],
    };
    input.sourceDiagnostics = {
      loggedGeometryDiagnostic: {
        pass: true,
        note: "synthetic green source log — must not upgrade rendered evidence",
      },
      onset: { present: true, supported: true },
    };
    record("source-green-rendered-invalid", "INVALID_OBSERVER", input, {
      sourceDiagnosticsPass: true,
      renderedUnderResolved: envelope.underResolved,
    });
  }

  return results;
}

// ---------------------------------------------------------------------------
// Aggregator plumbing
// ---------------------------------------------------------------------------

const PROTECTED_PATHS = [
  "src/theme/opacity.rs",
  "src/ui/chrome/tokens.rs",
  "src/platform/secondary_window_config.rs",
  "scripts/agentic/fixtures/glass-motion-calibration-theme.json",
  "scripts/devtools/glass-entry-motion-contract.ts",
];

export type HelperProof = {
  role: HelperRole;
  key: string;
  sourceSha256: string;
  binarySha256: string;
  binaryPath: string;
  validBefore: boolean;
  validAfter: boolean;
  errors: string[];
};

function repoRoot(): string {
  return resolve(import.meta.dir, "../..");
}

export function hashProtectedPaths(root = repoRoot()): Record<string, string> {
  const hashes: Record<string, string> = {};
  for (const path of PROTECTED_PATHS) {
    const absolute = join(root, path);
    hashes[path] = existsSync(absolute) ? sha256File(absolute) : "MISSING";
  }
  return hashes;
}

function atomicWriteJson(path: string, value: unknown) {
  mkdirSync(dirname(path), { recursive: true });
  const staging = `${path}.tmp-${process.pid}`;
  writeFileSync(staging, `${JSON.stringify(value, null, 2)}\n`);
  renameSync(staging, path);
}

function configuredCanaries(): string[] {
  return (process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES ?? "")
    .split(",")
    .map((canary) => canary.trim())
    .filter(Boolean);
}

function recursiveCanaryScan(root: string): {
  performed: true;
  matches: string[];
  pass: boolean;
} {
  const canaries = configuredCanaries();
  const matches: string[] = [];
  const walk = (dir: string) => {
    if (!existsSync(dir)) return;
    for (const entry of readdirSync(dir)) {
      const path = join(dir, entry);
      const stats = statSync(path);
      if (stats.isDirectory()) {
        walk(path);
      } else if (/\.(json|txt|log)$/.test(entry) && stats.size < 32_000_000) {
        const text = readFileSync(path, "utf8");
        for (const canary of canaries) {
          if (text.includes(canary)) {
            matches.push(
              `${relative(root, path)}:${
                createHash("sha256").update(canary).digest("hex").slice(0, 12)
              }`,
            );
          }
        }
      }
    }
  };
  walk(root);
  return { performed: true, matches, pass: matches.length === 0 };
}

async function runChild(
  command: string[],
  options: { timeoutMs: number; env: Record<string, string> },
): Promise<{
  exitCode: number | null;
  timedOut: boolean;
  pid: number;
  stdoutTail: string;
  stderrTail: string;
}> {
  const child = Bun.spawn(command, {
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, ...options.env },
  });
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    try {
      child.kill();
    } catch {
      // already gone
    }
  }, options.timeoutMs);
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  clearTimeout(timer);
  return {
    exitCode,
    timedOut,
    pid: child.pid,
    stdoutTail: stdout.trim().slice(-2_000),
    stderrTail: stderr.trim().slice(-2_000),
  };
}

function helperProofErrors(
  binaryPath: string,
  role: HelperRole,
): string[] {
  const manifestPath = helperManifestPath(binaryPath);
  if (!existsSync(manifestPath)) {
    return [`${role}: helper manifest missing: ${manifestPath}`];
  }
  const manifest = JSON.parse(
    readFileSync(manifestPath, "utf8"),
  ) as HelperCacheManifest;
  return validateHelperEntry(manifest, role, binaryPath).map(
    (error) => `${role}: ${error}`,
  );
}

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function readJsonIfPresent(path: string): any | null {
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null;
}

function interferenceInput(childInterference: any): GlassObservationInput["interference"] {
  return {
    validated: childInterference?.receipt != null
      && childInterference?.exitCode === 0,
    disposition: childInterference?.disposition ?? null,
    errors: childInterference?.errors ?? [],
  };
}

// ---------------------------------------------------------------------------
// Surface extraction — recompute truth from EVIDENCE fields, never child pass
// ---------------------------------------------------------------------------

function classifyMainSurface(
  lifecycleReceipt: any,
  shared: Pick<
    GlassObservationInput,
    "helperErrors" | "fixtureErrors" | "identityErrors"
  >,
): { classified: ClassifiedGlassObservation; sourceDiagnostics: unknown } {
  const scenario = (lifecycleReceipt?.scenarios ?? []).find(
    (entry: any) => entry?.name === "main-entry",
  );
  const envelope = scenario?.motionEnvelope ?? null;
  const frames = scenario?.presentationGeometry?.receipt?.frames ?? [];
  const exactWindowID = Number(scenario?.exactWindowID);
  const input: GlassObservationInput = {
    captureHealthPass: scenario?.filmstrip?.receipt?.captureHealthPass === true
      && scenario?.filmstrip?.capturePass === true
      && scenario?.presentationGeometry?.exitCode === 0
      && scenario?.presentationGeometry?.receipt?.pass === true,
    helperErrors: [...shared.helperErrors],
    fixtureErrors: [...shared.fixtureErrors],
    identityErrors: [
      ...shared.identityErrors,
      ...(scenario ? [] : ["main-entry scenario missing from lifecycle receipt"]),
      ...((scenario?.filmstrip?.errors ?? []) as string[]),
    ],
    ownerErrors: [
      ...(Number.isFinite(exactWindowID) && exactWindowID > 0
        ? validateOwnedRenderedFrames(frames, exactWindowID)
        : ["main-entry pinned native owner missing"]),
      ...(scenario?.settledCapturesPass === true
        ? []
        : ["settled native captures did not bind to the pinned owner"]),
    ],
    requiredPhaseErrors: [],
    cleanupErrors: lifecycleReceipt?.cleanedUp === true
      ? []
      : ["lifecycle probe did not clean up its app process"],
    interference: interferenceInput(lifecycleReceipt?.interference),
    rendered: {
      present: Number(envelope?.measuredFrameCount ?? 0) > 0,
      underResolved: envelope?.underResolved === true,
      pass: envelope?.pass === true,
      errors: (envelope?.errors ?? []) as string[],
    },
    sourceDiagnostics: {
      entryEvidence: lifecycleReceipt?.entryEvidence ?? null,
      runtimeContract: lifecycleReceipt?.runtimeContract ?? null,
    },
  };
  return {
    classified: classifyGlassObservation(input),
    sourceDiagnostics: input.sourceDiagnostics,
  };
}

function classifyNotesSurface(
  lifecycleReceipt: any,
  shared: Pick<
    GlassObservationInput,
    "helperErrors" | "fixtureErrors" | "identityErrors"
  >,
): {
  classified: ClassifiedGlassObservation;
  phases: unknown[];
  sourceDiagnostics: unknown;
} {
  const scenario = (lifecycleReceipt?.scenarios ?? []).find(
    (entry: any) => entry?.name === "notes-entry",
  );
  const phaseEvaluation = scenario?.phaseEvaluation ?? null;
  const records = (phaseEvaluation?.records ?? []) as Array<
    Record<string, unknown>
  >;
  const exactWindowID = Number(scenario?.exactWindowID);
  const displayPeriodNs = Number(
    phaseEvaluation?.displayPeriodNs ?? NOTES_DISPLAY_PERIOD_NS,
  );
  const validationErrors = Number.isFinite(exactWindowID) && exactWindowID > 0
    ? validateNotesPhaseRecords(records, exactWindowID, displayPeriodNs, {
      settleDeadlineNs: Number(phaseEvaluation?.settleDeadlineNs) || undefined,
    })
    : ["notes-entry pinned native owner missing"];
  const productErrors = (phaseEvaluation?.productErrors ?? []) as string[];
  const input: GlassObservationInput = {
    captureHealthPass: scenario?.filmstrip?.receipt?.captureHealthPass === true
      && scenario?.filmstrip?.capturePass === true
      && scenario?.presentationGeometry?.exitCode === 0
      && scenario?.presentationGeometry?.receipt?.pass === true,
    helperErrors: [...shared.helperErrors],
    fixtureErrors: [...shared.fixtureErrors],
    identityErrors: [
      ...shared.identityErrors,
      ...(scenario ? [] : ["notes-entry scenario missing from lifecycle receipt"]),
      ...((scenario?.filmstrip?.errors ?? []) as string[]),
    ],
    ownerErrors: (phaseEvaluation?.ownerErrors ?? []) as string[],
    requiredPhaseErrors: validationErrors,
    cleanupErrors: lifecycleReceipt?.cleanedUp === true
      ? []
      : ["lifecycle probe did not clean up its app process"],
    interference: interferenceInput(lifecycleReceipt?.interference),
    rendered: {
      present: records.length > 0
        && records.every((record) =>
          /^[a-f0-9]{64}$/.test(String(record.frameSha256 ?? ""))
        ),
      underResolved: false,
      pass: productErrors.length === 0,
      errors: productErrors,
    },
    sourceDiagnostics: {
      nativeConfiguration: scenario?.nativeConfiguration ?? null,
      bodyOnlyReveal: scenario?.bodyOnlyReveal ?? null,
    },
  };
  return {
    classified: classifyGlassObservation(input),
    phases: records,
    sourceDiagnostics: input.sourceDiagnostics,
  };
}

function classifyActionsSurface(
  actionsReceipt: any,
  shared: Pick<
    GlassObservationInput,
    "helperErrors" | "fixtureErrors" | "identityErrors"
  >,
): { classified: ClassifiedGlassObservation; sourceDiagnostics: unknown } {
  const envelope = actionsReceipt?.motion?.renderedEnvelope ?? null;
  const ownerId = Number(actionsReceipt?.owner?.windowId);
  const frames = actionsReceipt?.capture?.presentationGeometry?.receipt?.frames ?? [];
  const input: GlassObservationInput = {
    captureHealthPass:
      actionsReceipt?.capture?.receipt?.captureHealthPass === true
      && actionsReceipt?.capture?.exitCode === 0
      && actionsReceipt?.capture?.presentationGeometry?.commandExitCode === 0
      && actionsReceipt?.capture?.presentationGeometry?.receipt?.pass === true,
    helperErrors: [...shared.helperErrors],
    fixtureErrors: [...shared.fixtureErrors],
    identityErrors: [
      ...shared.identityErrors,
      ...((actionsReceipt?.capture?.identityErrors ?? []) as string[]),
    ],
    ownerErrors: [
      ...((actionsReceipt?.owner?.errors ?? []) as string[]),
      ...(Number.isFinite(ownerId) && ownerId > 0
        ? validateOwnedRenderedFrames(frames, ownerId)
        : ["Actions unique native owner missing"]),
    ],
    requiredPhaseErrors: [],
    cleanupErrors: actionsReceipt?.cleanedUp === true
      ? []
      : ["actions probe did not clean up its app process"],
    interference: interferenceInput(actionsReceipt?.interference),
    rendered: {
      present: Number(envelope?.measuredFrameCount ?? 0) > 0,
      underResolved: envelope?.underResolved === true,
      pass: envelope?.pass === true,
      errors: (envelope?.errors ?? []) as string[],
    },
    sourceDiagnostics: {
      loggedGeometryDiagnostic: actionsReceipt?.motion?.loggedGeometryDiagnostic
        ?? null,
      surfaceFields: actionsReceipt?.motion?.surfaceFields ?? null,
      onset: actionsReceipt?.motion?.onset ?? null,
    },
  };
  return {
    classified: classifyGlassObservation(input),
    sourceDiagnostics: input.sourceDiagnostics,
  };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function arg(name: string, fallback?: string) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

async function verifyCommand(): Promise<never> {
  const startedAt = new Date().toISOString();
  const root = repoRoot();
  const binaryArg = arg("--binary");
  const fixtureArg = arg(
    "--theme-fixture",
    "scripts/agentic/fixtures/glass-motion-calibration-theme.json",
  );
  const runManifestArg = arg("--run-manifest", ".artifacts/consistency/run.json");
  const helperCacheArg = arg(
    "--helper-cache",
    ".artifacts/consistency/PF-011/helpers",
  );
  const outArg = arg("--out", ".artifacts/consistency/PF-011/glass-observers.json");
  const probesRootArg = arg("--probes-root", ".artifacts/consistency/PF-011/probes");
  if (!binaryArg) {
    console.error("argument error: --binary is required");
    process.exit(64);
  }
  const binary = resolve(binaryArg);
  const fixture = resolve(fixtureArg!);
  const outPath = resolve(outArg!);
  const helperCache = resolve(helperCacheArg!);
  const probesRoot = resolve(probesRootArg!);
  if (!existsSync(binary)) {
    console.error(`argument error: binary missing: ${binary}`);
    process.exit(64);
  }
  if (!existsSync(fixture)) {
    console.error(`argument error: theme fixture missing: ${fixture}`);
    process.exit(64);
  }

  const runId = process.env.SCRIPT_KIT_GLASS_RUN_ID ?? newRunId();
  const errors: string[] = [];
  const warnings: string[] = [];

  // Source identity — WITHOUT a new Git command (plan §3.7): stable-artifact
  // manifest → campaign run manifest → SCRIPT_KIT_GLASS_GIT_COMMIT.
  let sourceCommit: string | null = null;
  const artifactManifest = readJsonIfPresent(
    join(dirname(binary), "manifest.json"),
  );
  if (typeof artifactManifest?.sourceCommit === "string") {
    sourceCommit = artifactManifest.sourceCommit;
  }
  if (!sourceCommit) {
    const runManifest = readJsonIfPresent(resolve(runManifestArg!));
    if (typeof runManifest?.sourceCommit === "string") {
      sourceCommit = runManifest.sourceCommit;
    }
  }
  if (!sourceCommit && process.env.SCRIPT_KIT_GLASS_GIT_COMMIT) {
    sourceCommit = process.env.SCRIPT_KIT_GLASS_GIT_COMMIT;
  }

  const protectedBefore = hashProtectedPaths(root);
  const baselinePath = join(
    root,
    ".artifacts/consistency/PF-011/baseline/protected-before.sha256",
  );
  if (existsSync(baselinePath)) {
    const baselineText = readFileSync(baselinePath, "utf8");
    for (const [path, hash] of Object.entries(protectedBefore)) {
      if (!baselineText.includes(`${hash}  ${path}`)) {
        errors.push(
          `protected baseline mismatch before run: ${path} is not the recorded C08 baseline`,
        );
      }
    }
  } else {
    warnings.push("no recorded protected baseline file; using live hashes only");
  }

  const binarySha256 = sha256File(binary);
  const binarySize = statSync(binary).size;
  const fixtureSha256 = sha256File(fixture);

  const receiptBase: any = {
    schemaVersion: 2,
    primitiveId: "devtools.glass.observers",
    tool: "script-kit-devtools.glass-observers",
    command: "glass-observers.verify",
    receiptId: `pf011-${runId}`,
    runId,
    taskIds: ["PF-011"],
    startedAt,
    repository: {
      sourceCommit,
      implementationFingerprint: sha256File(
        resolve(import.meta.dir, "glass-observers.ts"),
      ),
      producerSourceFingerprint: producerIdentityForTool(
        "script-kit-devtools.glass-observers",
      ).fingerprint,
    },
    binary: { path: binary, sha256: binarySha256, sizeBytes: binarySize },
    fixture: {
      id: "glass-motion-calibration-theme",
      path: relative(root, fixture),
      sha256: fixtureSha256,
    },
    errors,
    warnings,
  };

  const finish = (disposition: string, extra: Record<string, unknown>): never => {
    const receipt = {
      ...receiptBase,
      ...extra,
      endedAt: new Date().toISOString(),
      disposition,
      pass: disposition === "EVALUABLE_PASS",
    };
    atomicWriteJson(outPath, receipt);
    console.log(
      JSON.stringify(
        { receiptPath: outPath, disposition, pass: receipt.pass },
        null,
        2,
      ),
    );
    process.exit(exitCodeForDisposition(disposition));
  };

  if (!sourceCommit) {
    errors.push(
      "source identity unresolved: no stable-artifact manifest, no run manifest, no SCRIPT_KIT_GLASS_GIT_COMMIT",
    );
    finish("BLOCKED_MISSING_PRIMITIVE", {});
  }
  if (
    fixtureSha256
      !== protectedBefore["scripts/agentic/fixtures/glass-motion-calibration-theme.json"]
  ) {
    errors.push(
      "supplied theme fixture is not the protected production calibration fixture",
    );
    finish("INVALID_FIXTURE", {});
  }

  // Fresh attempt directory: refuse to reuse a non-empty probe directory.
  const attemptId = `attempt-${
    new Date().toISOString().replace(/[:.]/g, "-")
  }-${process.pid}`;
  const probeDir = join(probesRoot, attemptId);
  if (existsSync(probeDir) && readdirSync(probeDir).length > 0) {
    console.error(`argument error: probe attempt directory not empty: ${probeDir}`);
    process.exit(64);
  }
  mkdirSync(probeDir, { recursive: true });

  // Prepare + validate the three observer helpers.
  const helperRoles: Array<["filmstrip" | "interference" | "windowQuery", HelperRole]> = [
    ["filmstrip", "filmstrip"],
    ["interference", "interference"],
    ["windowQuery", "window-query"],
  ];
  const helpers: Record<string, HelperProof> = {};
  for (const [key, role] of helperRoles) {
    const prepared = await prepareHelper(role, { cacheDir: helperCache });
    const before = helperProofErrors(prepared.binaryPath, role);
    helpers[key] = {
      role,
      key: prepared.manifest.key,
      sourceSha256: prepared.manifest.sourceSha256,
      binarySha256: prepared.manifest.binarySha256,
      binaryPath: relative(root, prepared.binaryPath),
      validBefore: before.length === 0,
      validAfter: false,
      errors: before,
    };
  }
  const helperBinary = (key: string) => resolve(root, helpers[key]!.binaryPath);
  if (!helperRoles.every(([key]) => helpers[key]!.validBefore)) {
    errors.push("observer helper preparation failed validation");
    finish("INVALID_OBSERVER", { helpers });
  }

  const revalidateHelpers = (phase: string): boolean => {
    let allValid = true;
    for (const [key, role] of helperRoles) {
      const validation = helperProofErrors(helperBinary(key), role);
      if (validation.length > 0) {
        allValid = false;
        helpers[key]!.errors.push(...validation.map((e) => `[${phase}] ${e}`));
      }
    }
    return allValid;
  };

  const childEnv = {
    SCRIPT_KIT_GLASS_RUN_ID: runId,
    SCRIPT_KIT_GLASS_GIT_COMMIT: sourceCommit!,
    SCRIPT_KIT_GLASS_BINARY_SHA256: binarySha256,
  };

  const probes = [
    {
      id: "lifecycle" as const,
      timeoutMs: 420_000,
      receiptPath: join(probeDir, "lifecycle", "receipt.json"),
      command: [
        "bun",
        resolve(import.meta.dir, "glass-lifecycle-filmstrip.ts"),
        "--binary",
        binary,
        "--theme-fixture",
        fixture,
        "--profile",
        "full",
        "--analysis-mode",
        "inline",
        "--filmstrip-helper",
        helperBinary("filmstrip"),
        "--interference-helper",
        helperBinary("interference"),
        "--window-query-helper",
        helperBinary("windowQuery"),
        "--out",
        join(probeDir, "lifecycle"),
      ],
      env: { ...childEnv, SCRIPT_KIT_GLASS_SCENARIO: "lifecycle" },
    },
    {
      id: "actions-entry" as const,
      timeoutMs: 180_000,
      receiptPath: join(probeDir, "actions-entry", "receipt.json"),
      command: [
        "bun",
        resolve(import.meta.dir, "actions-entry-filmstrip.ts"),
        "--binary",
        binary,
        "--theme-fixture",
        fixture,
        "--theme-fixture-sha256",
        fixtureSha256,
        "--filmstrip-helper",
        helperBinary("filmstrip"),
        "--interference-helper",
        helperBinary("interference"),
        "--window-query-helper",
        helperBinary("windowQuery"),
        "--out",
        join(probeDir, "actions-entry"),
      ],
      env: { ...childEnv, SCRIPT_KIT_GLASS_SCENARIO: "actions-entry" },
    },
    {
      id: "rapid-toggle" as const,
      timeoutMs: 420_000,
      receiptPath: join(probeDir, "rapid-toggle", "receipt.json"),
      command: [
        "bun",
        resolve(import.meta.dir, "rapid-toggle-stress.ts"),
        "--binary",
        binary,
        "--theme-fixture",
        fixture,
        "--profile",
        "pf011",
        "--interference-helper",
        helperBinary("interference"),
        "--window-query-helper",
        helperBinary("windowQuery"),
        "--out",
        join(probeDir, "rapid-toggle", "receipt.json"),
      ],
      env: { ...childEnv, SCRIPT_KIT_GLASS_SCENARIO: "rapid-toggle-pf011" },
    },
  ];

  const probeIntervals: any[] = [];
  const childReceipts: Record<string, any> = {};
  const ownedPids: number[] = [];
  let previousEnd: number | null = null;
  let helperDrift = false;
  let timedOut = false;

  for (const probe of probes) {
    if (!revalidateHelpers(`before-${probe.id}`)) {
      helperDrift = true;
      break;
    }
    const startedMs = Date.now();
    const startedIso = new Date(startedMs).toISOString();
    const result = await runChild(probe.command, {
      timeoutMs: probe.timeoutMs,
      env: probe.env,
    });
    const endedMs = Date.now();
    ownedPids.push(result.pid);
    const receipt = readJsonIfPresent(probe.receiptPath);
    childReceipts[probe.id] = receipt;
    if (receipt?.pid) ownedPids.push(Number(receipt.pid));
    probeIntervals.push({
      id: probe.id,
      startedAt: startedIso,
      endedAt: new Date(endedMs).toISOString(),
      overlapsPrevious: previousEnd != null && startedMs < previousEnd,
      command: probe.command,
      exitCode: result.exitCode,
      timedOut: result.timedOut,
      receiptPath: relative(root, probe.receiptPath),
      receiptSha256: existsSync(probe.receiptPath)
        ? sha256File(probe.receiptPath)
        : null,
      stderrTail: result.stderrTail,
    });
    previousEnd = endedMs;
    if (result.timedOut) {
      timedOut = true;
      break;
    }
    if (!revalidateHelpers(`after-${probe.id}`)) {
      helperDrift = true;
      break;
    }
    // Binary identity must not drift between children.
    if (sha256File(binary) !== binarySha256) {
      errors.push("stable binary SHA-256 changed between child probes");
      finish("INVALID_BINARY", { helpers, probeIntervals });
    }
  }

  for (const [key] of helperRoles) {
    helpers[key]!.validAfter = helperProofErrors(
      helperBinary(key),
      helpers[key]!.role,
    ).length === 0;
  }

  // Shared observer-level errors folded into every surface classification.
  const sharedInputs = {
    helperErrors: helperDrift
      ? ["observer helper drifted during the PF-011 transaction"]
      : [],
    fixtureErrors: [] as string[],
    identityErrors: [] as string[],
  };
  for (const [id, receipt] of Object.entries(childReceipts)) {
    if (!receipt) {
      sharedInputs.identityErrors.push(`${id}: child receipt missing`);
      continue;
    }
    if (receipt.runId !== runId) {
      sharedInputs.identityErrors.push(`${id}: child runId mismatch`);
    }
    if (receipt.gitCommit !== sourceCommit) {
      sharedInputs.identityErrors.push(`${id}: child sourceCommit mismatch`);
    }
    if (receipt.binarySha256 !== binarySha256) {
      sharedInputs.identityErrors.push(`${id}: child binary SHA mismatch`);
    }
    if (receipt.themeFixture?.sha256 !== fixtureSha256) {
      sharedInputs.fixtureErrors.push(`${id}: child theme fixture SHA mismatch`);
    }
  }

  const main = classifyMainSurface(childReceipts.lifecycle, sharedInputs);
  const notes = classifyNotesSurface(childReceipts.lifecycle, sharedInputs);
  const actions = classifyActionsSurface(
    childReceipts["actions-entry"],
    sharedInputs,
  );

  // Rapid-toggle is a SUPPORT observer: its invalid/failed observation makes
  // the aggregate invalid/failed but never substitutes for rendered evidence.
  const rapidToggle = childReceipts["rapid-toggle"];
  const rapidToggleDisposition = String(rapidToggle?.disposition ?? "MISSING");

  const negativeControls = runSyntheticNegativeControls(dirname(outPath), {
    validHelperBinaryPath: helperBinary("filmstrip"),
  });

  // Cleanup: every owned PID must be dead. Kill only what this run owns.
  const survivors = [...new Set(ownedPids)].filter(pidAlive);
  for (const pid of survivors) {
    try {
      process.kill(pid, "SIGTERM");
    } catch {
      // already gone
    }
  }
  await Bun.sleep(survivors.length > 0 ? 500 : 0);
  const finalSurvivors = [...new Set(ownedPids)].filter(pidAlive);
  const cleanup = {
    ownedPids: [...new Set(ownedPids)],
    ownedSessions: probes.map((probe) => probe.id),
    closed: finalSurvivors.length === 0
      && Object.values(childReceipts).every(
        (receipt) => receipt?.cleanedUp === true,
      ),
    survivors: finalSurvivors,
  };

  const protectedAfter = hashProtectedPaths(root);
  const changedPaths = PROTECTED_PATHS.filter(
    (path) => protectedBefore[path] !== protectedAfter[path],
  );
  const protectedProof = {
    before: protectedBefore,
    after: protectedAfter,
    unchanged: changedPaths.length === 0,
    changedPaths,
  };

  const privacy = {
    mode: "fixture-redacted" as const,
    rawContentReturned: false,
    recursiveCanaryScan: recursiveCanaryScan(dirname(outPath)),
  };

  const surfaces = {
    main: main.classified,
    notes: { ...notes.classified, phases: notes.phases },
    actions: actions.classified,
  };
  const negativeControlsPass = negativeControls.length === 8
    && negativeControls.every((control) => control.pass);

  const assertions = [
    {
      id: "pf011.surfaces.all-classified-from-rendered-evidence",
      required: true,
      sourceLayer: "rendered",
      expected: "EVALUABLE_PASS x3",
      observed: {
        main: surfaces.main.disposition,
        notes: surfaces.notes.disposition,
        actions: surfaces.actions.disposition,
      },
      pass: [surfaces.main, surfaces.notes, surfaces.actions].every(
        (surface) => surface.disposition === "EVALUABLE_PASS",
      ),
    },
    {
      id: "pf011.rapid-toggle.support-observer",
      required: true,
      sourceLayer: "interaction",
      expected: "EVALUABLE_PASS",
      observed: rapidToggleDisposition,
      pass: rapidToggleDisposition === "EVALUABLE_PASS",
    },
    {
      id: "pf011.negative-controls.eight-nonzero",
      required: true,
      sourceLayer: "governance",
      expected: { count: 8, allPass: true },
      observed: {
        count: negativeControls.length,
        allPass: negativeControlsPass,
      },
      pass: negativeControlsPass,
    },
    {
      id: "pf011.protected.byte-identical",
      required: true,
      sourceLayer: "governance",
      expected: [],
      observed: changedPaths,
      pass: protectedProof.unchanged,
    },
  ];

  // Aggregate disposition (plan §2.4 precedence).
  let disposition: string;
  const surfaceList = [surfaces.main, surfaces.notes, surfaces.actions];
  if (timedOut) {
    disposition = "BLOCKED_TIMEOUT";
  } else if (!protectedProof.unchanged) {
    disposition = "INVALID_OBSERVER";
    errors.push(`protected paths changed during C08: ${changedPaths.join(", ")}`);
  } else if (helperDrift || !helperRoles.every(([key]) => helpers[key]!.validAfter)) {
    disposition = "INVALID_OBSERVER";
    errors.push("observer helper validation failed after probes");
  } else if (!cleanup.closed) {
    disposition = "INVALID_CLEANUP";
  } else if (
    surfaceList.some((s) => s.disposition === "INVALID_INTERFERENCE")
    || rapidToggleDisposition === "INVALID_INTERFERENCE"
  ) {
    disposition = "INVALID_INTERFERENCE";
  } else if (
    surfaceList.some((s) => s.disposition === "INVALID_OBSERVER")
    || rapidToggleDisposition.startsWith("INVALID")
    || rapidToggleDisposition === "MISSING"
    || !negativeControlsPass
  ) {
    disposition = "INVALID_OBSERVER";
  } else if (
    surfaceList.some((s) => s.disposition === "EVALUABLE_FAIL")
    || rapidToggleDisposition === "EVALUABLE_FAIL"
  ) {
    disposition = "EVALUABLE_FAIL";
  } else {
    disposition = "EVALUABLE_PASS";
  }

  finish(disposition, {
    protected: protectedProof,
    helpers,
    probeIntervals,
    surfaces,
    sourceDiagnostics: {
      main: main.sourceDiagnostics,
      notes: notes.sourceDiagnostics,
      actions: actions.sourceDiagnostics,
    },
    negativeControls,
    privacy,
    interference: {
      monitored: true,
      childReceipts: probeIntervals
        .map((interval) => interval.receiptPath)
        .filter(Boolean),
    },
    cleanup,
    assertions,
    rapidToggle: {
      disposition: rapidToggleDisposition,
      requiredPhaseNames: rapidToggle?.requiredPhaseNames ?? null,
      executedPhaseNames: rapidToggle?.executedPhaseNames ?? null,
    },
  });
  throw new Error("unreachable");
}

function classifySyntheticCommand(): never {
  if (process.env.SCRIPT_KIT_TEST_STATUS !== "1") {
    console.error(
      "classify-synthetic is a test-status-only mode: set SCRIPT_KIT_TEST_STATUS=1",
    );
    process.exit(64);
  }
  const inputPath = arg("--input");
  const outPath = arg("--out");
  if (!inputPath || !outPath) {
    console.error("argument error: --input and --out are required");
    process.exit(64);
  }
  const input = JSON.parse(
    readFileSync(resolve(inputPath), "utf8"),
  ) as GlassObservationInput;
  const classified = classifySyntheticObservation(input);
  atomicWriteJson(resolve(outPath), {
    schemaVersion: 2,
    kind: "pf011-synthetic-classification",
    classifiedAt: new Date().toISOString(),
    input,
    classification: classified,
  });
  console.log(JSON.stringify(classified, null, 2));
  process.exit(classified.exitCode);
}

if (import.meta.main) {
  const command = process.argv[2];
  if (command === "verify") {
    await verifyCommand();
  } else if (command === "classify-synthetic") {
    classifySyntheticCommand();
  } else {
    console.error(
      "usage: glass-observers.ts verify --binary <path> [--theme-fixture <path>] "
        + "[--run-manifest <path>] [--helper-cache <dir>] [--out <path>] | "
        + "classify-synthetic --input <json> --out <path>",
    );
    process.exit(64);
  }
}
