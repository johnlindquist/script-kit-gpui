import { createHash } from "node:crypto";
import {
  mkdirSync,
  chmodSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterAll, afterEach, describe, expect, test } from "bun:test";
import {
  currentIdentity,
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  parseProgressSections,
  parseTaskCatalog,
  receiptStaleReasons,
  stalenessReasons,
  verifyTask,
} from "./consistency.ts";
import {
  RUNTIME_TASK_PROOF_SPECS,
  type RuntimeTaskProofId,
} from "./lib/receipt-schema.ts";
import {
  prepareBlockedRuntimeTaskProof,
  prepareRuntimeTaskProof,
  runtimeTaskProofSourceOwners,
  verifyRuntimeBinaryProvenance,
} from "./lib/runtime-task-proof.ts";
import { createArtifactFixture } from "../agentic/build-artifact-fixture.ts";
import { ArtifactVerificationError, verifyImmutableArtifact, type ArtifactReference } from "../agentic/build-artifact.ts";

type Obj = Record<string, unknown>;

const catalog = parseTaskCatalog(
  readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
  DEFAULT_CONSISTENCY_CATALOG_PATH,
);
const gitHead = currentIdentity().headCommit!;
const binaryPath = "scripts/devtools/lib/runtime-task-proof.ts";
const binarySha = createHash("sha256").update(readFileSync(binaryPath)).digest("hex");
const bounds = { x: 0, y: 0, width: 800, height: 600 };
const temporaryDirectories: string[] = [];
let signedBinary: Obj | undefined;
let disposeArtifactFixture: (() => void) | undefined;

afterAll(() => disposeArtifactFixture?.());

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

function controls(taskId: RuntimeTaskProofId): Record<string, boolean> {
  return Object.fromEntries(
    RUNTIME_TASK_PROOF_SPECS[taskId].negativeControlIds.map((id) => [id, true]),
  );
}

function syntheticBinary(unsigned = false): Obj {
  if (unsigned) return { path: binaryPath, sha256: binarySha, sourceCommit: gitHead };
  if (!signedBinary) {
    // Publish once; each candidate owns a deep copy of the same immutable artifact identity.
    // Production proof verification still checks current source and provenance on every call.
    const fixture = createArtifactFixture(process.cwd(), { existingRepository: true, executable: readFileSync(binaryPath, "utf8") });
    disposeArtifactFixture = fixture.dispose;
    const artifact = verifyImmutableArtifact(process.cwd(), fixture.reference, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" });
    signedBinary = { ...artifact.binary, artifactReference: artifact.reference };
  }
  return structuredClone(signedBinary);
}

function baseCandidate(unsigned = false): Obj {
  return {
    schemaVersion: 2,
    classification: "ok",
    requestedTarget: { selector: { type: "main" } },
    target: {
      automationId: "main",
      windowInstanceId: "main@1",
      windowGeneration: 1,
      targetGeneration: 1,
      surfaceGeneration: 1,
      dataGeneration: 1,
      visible: false,
      bounds,
    },
    transaction: {
      transactionId: "proof:synthetic-runtime-task",
      runId: "synthetic-runtime-task",
      capturedAt: "2026-08-22T00:00:00.000Z",
      pid: process.pid,
      processStartTime: "2026-08-22T00:00:00.000Z",
      binarySha256: binarySha,
      automationId: "main",
      windowInstanceId: "main@1",
      windowGeneration: 1,
      windowKind: "Main",
      surfaceKind: "ScriptList",
      semanticSurface: "scriptList",
      appViewVariant: "ScriptList",
      bounds,
      targetGeneration: 1,
      surfaceGeneration: 1,
      dataGeneration: 1,
    },
    binary: syntheticBinary(unsigned),
    repository: { gitCommit: gitHead },
    cleanup: {
      processExited: true,
      streamsDrained: true,
      logWriterClosed: true,
      ownedProcessCount: 0,
      closeError: null,
      clipboardTouched: false,
    },
    missingPrimitives: [],
    warnings: [],
    errors: [],
  };
}

function candidateFor(taskId: RuntimeTaskProofId, unsigned = false): Obj {
  const common = baseCandidate(unsigned);
  if (taskId === "PF-004") {
    return {
      ...common,
      tool: "script-kit-devtools.elements",
      command: "elements.snapshot",
      semanticSurface: { collectorSurface: "settings" },
      semanticProjection: {
        semanticSurface: "settings",
        version: 1,
        quality: "complete",
        reasonCodes: [],
        proofMode: "action",
        proofAllowed: true,
      },
      nodes: [{ semanticId: "button:save", activatable: true }],
      duplicateSemanticIds: [],
      privacyViolationSemanticIds: [],
    };
  }
  if (taskId === "PF-005") {
    const row = { x: 0, y: 0, width: 100, height: 20 };
    return {
      ...common,
      tool: "script-kit-devtools.layout",
      command: "layout.measure",
      proofMode: "join",
      window: { rect: bounds },
      regions: [],
      resizePressure: { windowCanGrow: true },
      pressure: { pressureScore: 0 },
      truthLayers: {
        model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        rendered: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        joins: [{
          measurementId: "layout:row",
          semanticId: "row:one",
          role: "rowSlot",
          coordinateSpace: "window",
          comparability: "Comparable",
          classification: "Match",
          model: { bounds: row, generation: 8 },
          rendered: {
            bounds: row,
            visibleBounds: row,
            clipBounds: row,
            frameGeneration: 8,
            source: "paint-time",
          },
          delta: { x: 0, y: 0, width: 0, height: 0 },
          tolerance: { x: 1, y: 1, width: 1, height: 1 },
        }],
        comparableJoinCount: 1,
        unjoinedMeasurementIds: [],
      },
    };
  }
  if (taskId === "PF-006") {
    const line = { x: 0, y: 0, width: 100, height: 20 };
    return {
      ...common,
      tool: "script-kit-devtools.text",
      command: "text.measure",
      proofMode: "fit",
      textSummary: { inputLength: 7, inputFingerprint: "content-fingerprint" },
      rows: [{ semanticId: "input:notes-editor", textLength: 7, fingerprint: "content-fingerprint" }],
      notes: { count: 2, fullDisplayPass: true, rawContentReturned: false },
      dayPage: { count: 2, fullDisplayPass: true, rawContentReturned: false },
      textFits: [{
        measurementId: "text:notes:line:0",
        semanticId: "input:notes-editor",
        role: "textLineBox",
        lineBoxBounds: line,
        glyphBounds: { x: 0, y: 0, width: 80, height: 16 },
        clipBounds: line,
        visibleBounds: line,
        visibleRatio: 1,
        truncationPolicy: "fullDisplay",
        occluderMeasurementIds: [],
        fontFamilyFingerprint: "font-fingerprint",
        fontSize: 14,
        lineHeight: 20,
        backingScaleFactor: 2,
        fontsReady: true,
        contentFingerprint: "content-fingerprint",
        graphemeCount: 7,
        geometryValid: true,
        measurementIdentityValid: true,
        paintOrderValid: true,
        fullDisplayPass: true,
        rawContentReturned: false,
        frameMatches: true,
        backingScaleMatches: true,
      }],
    };
  }
  if (taskId === "PF-007") {
    return {
      ...common,
      tool: "script-kit-devtools.focus",
      command: "focus.inspect",
      proofMode: "ax",
      windowFocused: true,
      focusedSemanticId: "input:search",
      keyboardOwner: { surfaceKind: "ScriptList" },
      semanticProjection: { quality: "complete", proofAllowed: true },
      nativeFooter: {
        axParity: {
          semanticButtonCount: 1,
          axNodeCount: 1,
          peerCount: 1,
          duplicateAxIds: [],
          duplicateAxStructuralIds: [],
          duplicateSemanticIds: [],
          complete: true,
          peers: [{
            semanticId: "footer-action:run",
            action: "run",
            enabled: true,
            disabledReason: null,
            expectedSelector: "runFooterAction:",
            axPeer: {
              structuralId: "native-footer-run",
              accessibilityIdentifier: "footer-action:run",
              role: "AXButton",
              labelSha256: "a".repeat(64),
              labelLength: 3,
              enabled: true,
              accessibilityElement: true,
              hidden: false,
              alpha: 1,
              actionSelector: "runFooterAction:",
              bounds: { x: 0, y: 0, width: 80, height: 32 },
            },
            errors: [],
            parityPass: true,
          }],
        },
      },
      focusGraph: {
        nodes: [{ semanticId: "input:search", previous: null, next: null }],
        reciprocal: true,
        duplicateSemanticIds: [],
        hiddenFocusableIds: [],
        focusedSemanticIds: ["input:search"],
      },
      activationEvidence: {
        enabled: {
          host: "NativeFooter",
          actionId: "footer-action:run",
          resultOk: true,
          resultErrorCode: null,
          expectedSemanticId: "footer-action:run",
          postconditionObserved: true,
          complete: true,
          activation: {
            semanticId: "footer-action:run",
            accessibilityRole: "AXButton",
            actionSelector: "runFooterAction:",
            expectedActionSelector: "runFooterAction:",
            descriptorEnabled: true,
            appkitEnabled: true,
            refusedDisabled: false,
            dispatched: true,
          },
        },
        disabled: {
          host: "NativeFooter",
          actionId: "footer-action:run",
          resultOk: false,
          resultErrorCode: "action_disabled",
          expectedSemanticId: "footer-action:run",
          postconditionObserved: true,
          complete: true,
          activation: {
            semanticId: "footer-action:run",
            accessibilityRole: "AXButton",
            actionSelector: "runFooterAction:",
            expectedActionSelector: "runFooterAction:",
            descriptorEnabled: false,
            appkitEnabled: false,
            refusedDisabled: true,
            dispatched: false,
            errorCode: "action_disabled",
          },
        },
      },
    };
  }

  const row = { x: 0, y: 20, width: 100, height: 20 };
  const viewport = { x: 0, y: 0, width: 100, height: 100 };
  return {
    ...common,
    tool: "script-kit-devtools.scroll",
    command: "scroll.inspect",
    scroll: { selectedSemanticId: "row:selected", selectedRowWithinSafeViewport: true },
    resizePressure: { selectedRowOutsideSafeViewport: false },
    selectedRow: {
      semanticIdSha256: createHash("sha256").update("row:selected").digest("hex"),
      semanticIdReturnedRaw: false,
      selectionChanged: true,
      transaction: {
        before: {
          windowInstanceId: "main@1",
          targetGeneration: 1,
          surfaceGeneration: 1,
          dataGeneration: 0,
        },
        after: {
          windowInstanceId: "main@1",
          targetGeneration: 1,
          surfaceGeneration: 1,
          dataGeneration: 1,
        },
        stableWindowInstance: true,
        stableTargetGeneration: true,
        stableSurfaceGeneration: true,
        dataGenerationAdvanced: true,
        dataGenerationPresent: true,
      },
    },
    renderedSafeViewport: {
      required: true,
      classification: "ok",
      selectedSemanticId: "row:selected",
      rowMeasurementId: "layout:row:selected",
      safeViewportMeasurementId: "layout:main-view-main",
      rowObservationCount: 1,
      safeViewportObservationCount: 1,
      rowBounds: row,
      rowVisibleBounds: row,
      rowClipBounds: row,
      safeViewportBounds: viewport,
      safeViewportClipBounds: viewport,
      safeViewportPaintBounds: viewport,
      coordinateSpace: "window",
      visibleRatio: 1,
      withinSafeViewport: true,
      frameGeneration: 8,
      viewportFrameGeneration: 8,
      frameMatches: true,
      targetDataGeneration: 1,
      missingPrimitives: [],
    },
  };
}

describe("canonical direct runtime task receipts", () => {
  test("an unsigned existing executable cannot borrow current HEAD as build provenance", () => {
    expect(() => prepareRuntimeTaskProof("PF-004", candidateFor("PF-004", true), controls("PF-004")))
      .toThrow();
  });

  test("explicit manifest and byte mismatches fail closed without adjacent-manifest discovery", () => {
    const binary = syntheticBinary(); const reference = binary.artifactReference as ArtifactReference;
    expect(() => verifyRuntimeBinaryProvenance({ ...reference, manifestSha256: "0".repeat(64) }))
      .toThrow(ArtifactVerificationError);
    const path = binary.path as string; const bytes = readFileSync(path);
    const mode = statSync(path).mode & 0o777;
    try {
      chmodSync(path, mode | 0o200); writeFileSync(path, "substituted executable"); chmodSync(path, mode);
      expect(() => verifyRuntimeBinaryProvenance(reference)).toThrow(ArtifactVerificationError);
    } finally {
      chmodSync(path, mode | 0o200);
      try { writeFileSync(path, bytes); } finally { chmodSync(path, mode); }
    }
    const directory = dirname(reference.manifestPath);
    const directoryMode = statSync(directory).mode & 0o777;
    const extra = join(directory, "unrelated.provenance.json");
    try {
      chmodSync(directory, directoryMode | 0o200);
      writeFileSync(extra, "{}");
      chmodSync(directory, directoryMode);
      expect(verifyRuntimeBinaryProvenance(reference).sha256).toBe(binarySha);
    } finally {
      chmodSync(directory, directoryMode | 0o200);
      try { rmSync(extra, { force: true }); } finally { chmodSync(directory, directoryMode); }
    }
  });

  test("documentation-only commits preserve recorded build identity and current compiler-content compatibility", () => {
    const repository = mkdtempSync(join(tmpdir(), "runtime-task-doc-only-repository-"));
    temporaryDirectories.push(repository);
    const fixture = createArtifactFixture(repository);
    try {
      const before = verifyImmutableArtifact(repository, fixture.reference, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" });
      writeFileSync(join(repository, "documentation.txt"), "documentation-only fixture change\n");
      for (const args of [["add", "documentation.txt"], ["-c", "user.name=Fixture", "-c", "user.email=fixture@invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "-qm", "documentation"]]) {
        const result = Bun.spawnSync(["git", "-C", repository, ...args], { stdout: "pipe", stderr: "pipe" });
        expect(result.exitCode).toBe(0);
      }
      const after = verifyImmutableArtifact(repository, fixture.reference, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content" });
      expect(after.binary.sourceCommit).toBe(before.manifest.source.gitHead);
      expect(after.manifest.source.compilerInputSha256).toBe(before.manifest.source.compilerInputSha256);
      expect(() => verifyImmutableArtifact(repository, fixture.reference, { kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "clean-exact-head" })).toThrow();
      const current = currentIdentity();
      const previousDirectory = process.cwd();
      try {
        process.chdir(repository);
        const fixtureCurrent = currentIdentity({ registry: current.registry });
        expect(fixtureCurrent.headCommit).not.toBe(after.binary.sourceCommit);
        const binary = { ...after.binary, artifactReference: after.reference };
        expect(receiptStaleReasons({
          path: "observed.json", disposition: "EVALUABLE_PASS", archived: false,
          receipt: { runtimeTaskProof: {}, binary },
        }, fixtureCurrent)).toEqual([]);
        expect(stalenessReasons({
          taskId: "PF-004",
          identities: {
            receiptRegistryVersion: fixtureCurrent.registry.registryVersion,
            receiptRegistryFingerprint: fixtureCurrent.registry.registryFingerprint,
            binaries: [binary],
          },
        }, fixtureCurrent)).toEqual([]);
      } finally { process.chdir(previousDirectory); }
    } finally { fixture.dispose(); }

    const taskId = "PF-004";
    const prepared = prepareRuntimeTaskProof(taskId, candidateFor(taskId), controls(taskId));
    const directory = mkdtempSync(join(tmpdir(), "runtime-task-equivalent-source-"));
    temporaryDirectories.push(directory);
    const taskDirectory = join(directory, taskId);
    mkdirSync(taskDirectory, { recursive: true });
    writeFileSync(join(taskDirectory, "observed.json"), JSON.stringify(prepared.receipt));
    const progressText = `### ${taskId} — ${catalog.byId.get(taskId)!.title}\n`;
    expect(verifyTask({
      taskId,
      scope: "cons-proof-gov",
      receiptsRoot: directory,
      catalog,
      progress: parseProgressSections(progressText),
      current: currentIdentity(),
    }).exitCode).toBe(0);
  });

  test.each(Object.keys(RUNTIME_TASK_PROOF_SPECS) as RuntimeTaskProofId[])(
    "%s binds the real primitive, exact task, source owners, controls, and target transaction",
    (taskId) => {
      const prepared = prepareRuntimeTaskProof(taskId, candidateFor(taskId), controls(taskId));
      expect(prepared.exitCode).toBe(0);
      expect(prepared.receipt.primitiveId).toBe(RUNTIME_TASK_PROOF_SPECS[taskId].primitiveId);
      expect(prepared.receipt.taskIds).toEqual([taskId]);
      expect(prepared.receipt.evidenceClass).toBe("RUNTIME_HIDDEN");
      expect(prepared.receipt.disposition).toBe("EVALUABLE_PASS");
      const binary = prepared.receipt.binary as Obj;
      const reference = binary.artifactReference as ArtifactReference;
      expect(binary.manifestPath).toBe(reference.manifestPath);
      expect(binary.manifestSha256).toBe(reference.manifestSha256);
      expect(binary.provenance).toBeUndefined();
      expect((prepared.receipt.catalogBinding as Obj).sectionSha256)
        .toBe(catalog.byId.get(taskId)!.sectionSha256);
      expect(Object.keys(prepared.receipt.sourceFingerprints as Obj).sort())
        .toEqual(runtimeTaskProofSourceOwners(taskId).sort());
      expect((prepared.receipt.producerValidation as Obj).registryFingerprint)
        .toMatch(/^[a-f0-9]{64}$/);

      const directory = mkdtempSync(join(tmpdir(), "runtime-task-proof-"));
      temporaryDirectories.push(directory);
      const taskDirectory = join(directory, taskId);
      mkdirSync(taskDirectory, { recursive: true });
      writeFileSync(join(taskDirectory, "observed.json"), JSON.stringify(prepared.receipt));
      const progressPath = join(directory, "progress.md");
      const progressText = `### ${taskId} — ${catalog.byId.get(taskId)!.title}\n`;
      writeFileSync(progressPath, progressText);
      const audited = verifyTask({
        taskId,
        scope: "cons-proof-gov",
        receiptsRoot: directory,
        catalog,
        progress: parseProgressSections(progressText, progressPath),
        current: currentIdentity(),
      });
      expect(audited.exitCode).toBe(0);
      expect(audited.receipt.disposition).toBe("EVALUABLE_PASS");
      expect(stalenessReasons(audited.receipt, currentIdentity())).toEqual([]);
    },
  );

  test("canonical audits require explicit artifact identity and redact rejected private references", () => {
    const taskId = "PF-004";
    const prepared = prepareRuntimeTaskProof(taskId, candidateFor(taskId), controls(taskId));
    const directory = mkdtempSync(join(tmpdir(), "runtime-task-artifact-identity-"));
    temporaryDirectories.push(directory);
    const taskDirectory = join(directory, taskId);
    mkdirSync(taskDirectory, { recursive: true });
    const privatePath = "/Users/example/Sensitive Script/manifest.json";
    const variants: Array<[(binary: Obj) => void, Obj]> = [
      [(binary) => { delete binary.artifactReference; }, { code: "stale-binary-provenance-missing" }],
      [(binary) => { binary.sourceCommit = "a".repeat(40); }, {
        code: "stale-binary-provenance-identity", detail: "observed_sourceCommit_mismatch",
      }],
      [(binary) => { binary.manifestSha256 = "b".repeat(64); }, {
        code: "stale-binary-provenance-identity", detail: "observed_manifestSha256_mismatch",
      }],
      [(binary) => { (binary.artifactReference as Obj).manifestPath = privatePath; }, {
        code: "stale-binary-provenance", detail: "unsafe_artifact_path",
      }],
    ];
    const current = currentIdentity();
    for (const [mutate, expectedReason] of variants) {
      const receipt = structuredClone(prepared.receipt);
      mutate(receipt.binary as Obj);
      writeFileSync(join(taskDirectory, "observed.json"), JSON.stringify(receipt));
      const audited = verifyTask({
        taskId, scope: "cons-proof-gov", receiptsRoot: directory, catalog,
        progress: parseProgressSections(`### ${taskId} — ${catalog.byId.get(taskId)!.title}\n`),
        current,
      });
      expect(audited.exitCode).not.toBe(0);
      expect(audited.receipt.staleReasons).toContainEqual(expectedReason);
      expect(JSON.stringify(audited.receipt)).not.toContain(privatePath);
    }
  });

  test("generic or swapped registered inspectors cannot discharge another obligation", () => {
    expect(() => prepareRuntimeTaskProof("PF-004", candidateFor("PF-005"), controls("PF-004")))
      .toThrow("actual devtools.elements.snapshot production primitive");
    const inspection = candidateFor("PF-005");
    inspection.proofMode = "inspection";
    expect(() => prepareRuntimeTaskProof("PF-005", inspection, controls("PF-005")))
      .toThrow("actual join runtime observation");
    const noRendered = candidateFor("PF-008");
    (noRendered.renderedSafeViewport as Obj).required = false;
    expect(() => prepareRuntimeTaskProof("PF-008", noRendered, controls("PF-008")))
      .toThrow("actual rendered-safe-viewport runtime observation");
  });

  test("missing, duplicate, or failing negative controls cannot be invented", () => {
    const taskId = "PF-004";
    const actual = controls(taskId);
    const missing = { ...actual };
    delete missing.partialActionProofBlocked;
    expect(() => prepareRuntimeTaskProof(taskId, candidateFor(taskId), missing))
      .toThrow("missing required negative controls");
    expect(() => prepareRuntimeTaskProof(taskId, candidateFor(taskId), {
      ...actual,
      duplicateSemanticIdsInvalid: false,
    })).toThrow("failed or unexecuted negative control");
    expect(() => prepareRuntimeTaskProof(taskId, candidateFor(taskId), [
      ...Object.entries(actual).map(([id, pass]) => ({ id, pass })),
      { id: "duplicateSemanticIdsInvalid", pass: true },
    ])).toThrow("uniquely identified");
  });

  test.each<[string, (candidate: Obj) => void, string]>([
      ["unobserved target", (candidate) => delete (candidate.target as Obj).visible, "observed hidden or visible"],
      ["stale source", (candidate) => (candidate.repository as Obj).gitCommit = "a".repeat(40), "source commit"],
      ["stale binary source", (candidate) => (candidate.binary as Obj).sourceCommit = "a".repeat(40), "observed_sourceCommit_mismatch"],
      ["wrong manifest identity", (candidate) => (candidate.binary as Obj).manifestSha256 = "b".repeat(64), "observed_manifestSha256_mismatch"],
      ["wrong binary bytes", (candidate) => {
        (candidate.binary as Obj).sha256 = "b".repeat(64);
        (candidate.transaction as Obj).binarySha256 = "b".repeat(64);
      }, "binary bytes"],
      ["transaction binary drift", (candidate) => (candidate.transaction as Obj).binarySha256 = "b".repeat(64), "matching proof transaction"],
      ["surviving app", (candidate) => (candidate.cleanup as Obj).processExited = false, "cleanup"],
      ["touched clipboard", (candidate) => (candidate.cleanup as Obj).clipboardTouched = true, "cleanup"],
  ])("%s fails closed", (name, mutate, expected) => {
    const taskId = "PF-004";
    const candidate = candidateFor(taskId);
    mutate(candidate);
    expect(() => prepareRuntimeTaskProof(taskId, candidate, controls(taskId)), name)
      .toThrow(expected);
  });

  test("foreign canonical section and broken primitive observations cannot become runtime proof", () => {
    const taskId = "PF-006";
    const swapped = candidateFor(taskId);
    swapped.catalogBinding = {
      taskId: "PF-005",
      title: catalog.byId.get("PF-005")!.title,
      sectionSha256: catalog.byId.get("PF-005")!.sectionSha256,
    };
    expect(() => prepareRuntimeTaskProof(taskId, swapped, controls(taskId)))
      .toThrow("different catalog obligation");

    const missingGlyph = candidateFor(taskId);
    (missingGlyph.textFits as Obj[])[0].glyphBounds = { x: 0, y: 0, width: 0, height: 20 };
    expect(() => prepareRuntimeTaskProof(taskId, missingGlyph, controls(taskId)))
      .toThrow("runtime primitive is invalid");
  });

  test("shared text requires actual day-page glyph observations", () => {
    const missingDayPage = candidateFor("PF-006");
    delete missingDayPage.dayPage;
    expect(() => prepareRuntimeTaskProof("PF-006", missingDayPage, controls("PF-006")))
      .toThrow("dayPage glyph evidence");
  });

  test("native activation requires actual disabled activation observations", () => {
    const missingDisabled = candidateFor("PF-007");
    delete (missingDisabled.activationEvidence as Obj).disabled;
    expect(() => prepareRuntimeTaskProof("PF-007", missingDisabled, controls("PF-007")))
      .toThrow("disabled native activation");
  });

  test("native activation rejects a forged enabled action selector", () => {
    const forgedSelector = candidateFor("PF-007");
    (((forgedSelector.activationEvidence as Obj).enabled as Obj).activation as Obj)
      .actionSelector = "deleteEverything:";
    expect(() => prepareRuntimeTaskProof("PF-007", forgedSelector, controls("PF-007")))
      .toThrow("enabled native activation");
  });

  test("selection transition requires an advancing data-generation transaction", () => {
    const noSelection = candidateFor("PF-008");
    ((noSelection.selectedRow as Obj).transaction as Obj).dataGenerationAdvanced = false;
    expect(() => prepareRuntimeTaskProof("PF-008", noSelection, controls("PF-008")))
      .toThrow("advancing data-generation transaction");
  });

  test("the canonical auditor detects drift in the actual runtime producer or adapter bytes", () => {
    const taskId = "PF-004";
    const prepared = prepareRuntimeTaskProof(taskId, candidateFor(taskId), controls(taskId));
    const fingerprints = prepared.receipt.sourceFingerprints as Obj;
    fingerprints[RUNTIME_TASK_PROOF_SPECS[taskId].runtimeProducer] = "c".repeat(64);

    const directory = mkdtempSync(join(tmpdir(), "runtime-task-stale-"));
    temporaryDirectories.push(directory);
    const taskDirectory = join(directory, taskId);
    mkdirSync(taskDirectory, { recursive: true });
    writeFileSync(join(taskDirectory, "observed.json"), JSON.stringify(prepared.receipt));
    const progressPath = join(directory, "progress.md");
    const progressText = `### ${taskId} — ${catalog.byId.get(taskId)!.title}\n`;
    writeFileSync(progressPath, progressText);
    const audited = verifyTask({
      taskId,
      scope: "cons-proof-gov",
      receiptsRoot: directory,
      catalog,
      progress: parseProgressSections(progressText, progressPath),
      current: currentIdentity(),
    });
    expect(audited.exitCode).toBe(3);
    expect(audited.receipt.disposition).toBe("BLOCKED_STALE_GENERATION");
    expect(JSON.stringify(audited.receipt)).toContain("stale-runtime-proof-source");
  });

  test("final auditing rejects build provenance that is mutated after receipt publication", () => {
    const taskId = "PF-004";
    const prepared = prepareRuntimeTaskProof(taskId, candidateFor(taskId), controls(taskId));
    const { manifestPath } = (prepared.receipt.binary as Obj).artifactReference as ArtifactReference;
    const bytes = readFileSync(manifestPath);
    const mode = statSync(manifestPath).mode & 0o777;
    const manifest = JSON.parse(bytes.toString());
    manifest.source.gitHead = "b".repeat(40);

    const directory = mkdtempSync(join(tmpdir(), "runtime-task-provenance-"));
    temporaryDirectories.push(directory);
    const taskDirectory = join(directory, taskId);
    mkdirSync(taskDirectory, { recursive: true });
    writeFileSync(join(taskDirectory, "observed.json"), JSON.stringify(prepared.receipt));
    const progressText = `### ${taskId} — ${catalog.byId.get(taskId)!.title}\n`;
    try {
      chmodSync(manifestPath, mode | 0o200);
      writeFileSync(manifestPath, JSON.stringify(manifest));
      chmodSync(manifestPath, mode);
      const audited = verifyTask({
        taskId,
        scope: "cons-proof-gov",
        receiptsRoot: directory,
        catalog,
        progress: parseProgressSections(progressText),
        current: currentIdentity(),
      });
      expect(audited.exitCode).toBe(3);
      expect(audited.receipt.disposition).toBe("BLOCKED_STALE_GENERATION");
      expect(audited.receipt.staleReasons).toContainEqual({
        code: "stale-binary-provenance", detail: "manifest_hash_mismatch",
      });
    } finally {
      chmodSync(manifestPath, mode | 0o200);
      try { writeFileSync(manifestPath, bytes); } finally { chmodSync(manifestPath, mode); }
    }
  });

  test("an absent application emits a registered typed block without revealing diagnostic content", () => {
    const privateReason = "private runtime path /Users/example/Sensitive Script.ts";
    const blocked = prepareBlockedRuntimeTaskProof("PF-004", {
      stage: "launch",
      reason: privateReason,
      cleanup: { processExited: false },
    });
    expect(blocked.exitCode).toBe(3);
    expect(blocked.receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(blocked.receipt.pass).toBe(false);
    expect(blocked.receipt.primitiveId).toBe("devtools.elements.snapshot");
    expect((blocked.receipt.producerValidation as Obj).valid).toBe(true);
    expect(JSON.stringify(blocked.receipt)).not.toContain(privateReason);
    expect((blocked.receipt.runtimeFailure as Obj).reasonSha256).toMatch(/^[a-f0-9]{64}$/);
  });
});
