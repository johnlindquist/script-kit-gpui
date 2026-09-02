/**
 * GOV-006 auditor tests — synthetic filesystem/receipt mutations.
 *
 * Every required mutation from the lane plan (§5.5) must exit nonzero with
 * its exact reason code; only EVALUABLE_PASS exits zero. Fixtures are
 * generated per-test in unique temp directories so the shared file-hash
 * cache never sees two states of one path.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import {
  RECEIPT_REGISTRY_VERSION,
  prepareValidatedReceipt,
  receiptRegistryIdentity,
  RUNTIME_TASK_PROOF_SPECS,
  validateReceipt,
} from "./lib/receipt-schema.ts";
import {
  AUTHORIZED_CONFLICT_COUNT,
  CONS_FLOW_UX_IDS,
  CONS_PROOF_GOV_IDS,
  DEFAULT_CONSISTENCY_CATALOG_PATH,
  FAMILY_IDS,
  PROGRAM_IDS,
  UsageError,
  currentIdentity,
  fileSha256,
  parseArgs,
  parseProgressSections,
  parseTaskCatalog,
  receiptStaleReasons,
  stalenessReasons,
  verifyAll,
  verifyFamily,
  verifyScope,
  verifyTask,
  type CurrentIdentity,
} from "./consistency.ts";
import {
  TASK_PROOF_POLICIES,
  taskProofPolicy,
} from "./lib/task-proof-policy.ts";
import {
  WORKFLOW_TASK_PRIMITIVE_ID,
  WORKFLOW_TASK_PROOF_MODE,
  WORKFLOW_TASK_PROOF_SPECS,
  workflowTaskProofSourceOwners,
  type WorkflowTaskProofId,
} from "./lib/workflow-task-contract.ts";
import {
  attachFacadeMigrationScope,
  auditFacadeMigrationScope,
  CONVERSATION_STYLE_FACADE,
  CONVERSATION_STYLE_OWNER,
  POPUP_AUTOMATION_POLICY,
  POPUP_WINDOW_FACADE,
  POPUP_WINDOW_OWNER,
  REQUIRED_POPUP_CONSUMERS,
  SHARED_COMPONENTS_MODULE,
} from "./facade-migrations.ts";
import { createArtifactFixture, type ArtifactFixture } from "../agentic/build-artifact-fixture.ts";
import { verifyImmutableArtifact } from "../agentic/build-artifact.ts";

const HEAD = currentIdentity().headCommit!;
const WRONG_SHA = "0".repeat(64);
let ownedSyntheticBinary: Record<string, unknown> | undefined;
let ownedSyntheticArtifact: ArtifactFixture | undefined;

afterAll(() => {
  ownedSyntheticArtifact?.dispose();
});

function syntheticArtifactBinary(): Record<string, unknown> {
  if (!ownedSyntheticBinary) {
    ownedSyntheticArtifact = createArtifactFixture(process.cwd(), {
      existingRepository: true,
      executable: readFileSync("scripts/devtools/lib/runtime-task-proof.ts", "utf8"),
    });
    const artifact = verifyImmutableArtifact(process.cwd(), ownedSyntheticArtifact.reference, {
      kind: "application", packageName: "script-kit-gpui", targetName: "script-kit-gpui", sourcePolicy: "current-content",
    });
    ownedSyntheticBinary = { ...artifact.binary, artifactReference: artifact.reference };
  }
  return structuredClone(ownedSyntheticBinary);
}

function catalogMarkdown(ids: Iterable<string> = PROGRAM_IDS, extraLines: string[] = []): string {
  const sections = [...ids].map((id) => `### ${id} — Synthetic ${id}\n\n- catalog body for ${id}\n`);
  return `# Synthetic catalog\n\n## Tasks\n\n${sections.join("\n")}\n${extraLines.join("\n")}\n`;
}

function progressMarkdown(ids: Iterable<string> = PROGRAM_IDS, duplicates: string[] = []): string {
  const sections = [...ids, ...duplicates].map(
    (id) => `### ${id} — Synthetic ${id}\n\n- **Status:** Complete\n`,
  );
  return `# Synthetic progress\n\n## Completed recommendations\n\n${sections.join("\n")}\n`;
}

const syntheticCatalogBindings = parseTaskCatalog(catalogMarkdown()).byId;

function passingFacadeLedger(): Record<string, unknown> {
  const sources = [
    {
      path: SHARED_COMPONENTS_MODULE,
      content: "pub mod conversation_style;\npub mod inline_popup_window;\n",
    },
    { path: CONVERSATION_STYLE_FACADE, content: undefined },
    { path: CONVERSATION_STYLE_OWNER, content: "pub struct ConversationStyle;" },
    { path: POPUP_WINDOW_FACADE, content: undefined },
    { path: POPUP_WINDOW_OWNER, content: "pub struct InlinePopupWindow;" },
    {
      path: POPUP_AUTOMATION_POLICY,
      content: "pub(crate) fn agent_chat_popup_policy() {}",
    },
    {
      path: "src/ai/agent_chat/ui/components/transcript.rs",
      content: "use crate::components::conversation_style::ConversationStyle;",
    },
    ...REQUIRED_POPUP_CONSUMERS.map((path) => ({
      path,
      content: "use crate::components::inline_popup_window::InlinePopupWindow;",
    })),
  ];
  const scope = auditFacadeMigrationScope(sources, [
    "crate::components::conversation_style::ASSISTANT_MESSAGE_PADDING",
  ]);
  return attachFacadeMigrationScope(
    {
      schemaVersion: 1,
      generatedBy: "scripts/devtools/facade-ledger.ts",
      taskId: "GOV-002",
      evidenceClass: "STATIC_INVENTORY",
      provesRuntimeBehavior: false,
      provesExporterByteEquality: false,
      assertions: {
        allFacadesValueFree: true,
        allProductionCallersMigrated: true,
        allTestCallersMigrated: true,
        zeroCallerFacadesRemoved: true,
        persistedNamesLiveAtCanonicalOwnersOnly: true,
      },
      disposition: "EVALUABLE_PASS",
    },
    scope,
  );
}

interface ReceiptOverrides {
  [key: string]: unknown;
}

function syntheticFoundationReceipt(taskId: string, candidate: Record<string, any>) {
  const bounds = { x: 0, y: 0, width: 100, height: 20 };
  if (taskId === "PF-004") {
    return {
      primitiveId: "devtools.elements.snapshot",
      candidate: {
        ...candidate,
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
        nodes: [{ semanticId: "button:run", activatable: true }],
        duplicateSemanticIds: [],
      },
    };
  }
  if (taskId === "PF-005") {
    return {
      primitiveId: "devtools.layout.measure",
      candidate: {
        ...candidate,
        proofMode: "join",
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
            model: { bounds, generation: 1 },
            rendered: { bounds, visibleBounds: bounds, clipBounds: bounds, frameGeneration: 1 },
            delta: { x: 0, y: 0, width: 0, height: 0 },
            tolerance: { x: 1, y: 1, width: 1, height: 1 },
          }],
          comparableJoinCount: 1,
          unjoinedMeasurementIds: [],
        },
      },
    };
  }
  if (taskId === "PF-006") {
    return {
      primitiveId: "devtools.text.measure",
      candidate: {
        ...candidate,
        tool: "script-kit-devtools.text",
        command: "text.measure",
        proofMode: "fit",
        textSummary: { inputLength: 4, inputFingerprint: "synthetic" },
        rows: [{ textLength: 4, fingerprint: "synthetic" }],
        textFits: [{
          measurementId: "text:line",
          semanticId: "input:search",
          role: "textLineBox",
          lineBoxBounds: bounds,
          glyphBounds: bounds,
          clipBounds: bounds,
          visibleBounds: bounds,
          visibleRatio: 1,
          truncationPolicy: "fullDisplay",
          occluderMeasurementIds: [],
          fontFamilyFingerprint: "font",
          fontSize: 14,
          lineHeight: 20,
          backingScaleFactor: 2,
          fontsReady: true,
          contentFingerprint: "content",
          graphemeCount: 4,
          geometryValid: true,
          measurementIdentityValid: true,
          paintOrderValid: true,
          fullDisplayPass: true,
          rawContentReturned: false,
          frameMatches: true,
          backingScaleMatches: true,
        }],
      },
    };
  }
  if (taskId === "PF-007") {
    return {
      primitiveId: "devtools.focus.inspect",
      candidate: {
        ...candidate,
        tool: "script-kit-devtools.focus",
        command: "focus.inspect",
        proofMode: "ax",
        windowFocused: true,
        focusedSemanticId: "input:search",
        keyboardOwner: {},
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
                structuralId: "footer-run",
                accessibilityIdentifier: "footer-action:run",
                role: "AXButton",
                labelSha256: "a".repeat(64),
                labelLength: 3,
                enabled: true,
                accessibilityElement: true,
                hidden: false,
                alpha: 1,
                actionSelector: "runFooterAction:",
                bounds,
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
      },
    };
  }
  if (taskId === "PF-008") {
    return {
      primitiveId: "devtools.scroll.inspect",
      candidate: {
        ...candidate,
        tool: "script-kit-devtools.scroll",
        command: "scroll.inspect",
        scroll: { selectedSemanticId: "row:one", selectedRowWithinSafeViewport: true },
        renderedSafeViewport: {
          required: true,
          classification: "ok",
          selectedSemanticId: "row:one",
          rowMeasurementId: "layout:row",
          safeViewportMeasurementId: "layout:viewport",
          rowObservationCount: 1,
          safeViewportObservationCount: 1,
          rowBounds: bounds,
          rowVisibleBounds: bounds,
          rowClipBounds: bounds,
          safeViewportBounds: bounds,
          safeViewportClipBounds: bounds,
          safeViewportPaintBounds: bounds,
          coordinateSpace: "window",
          visibleRatio: 1,
          withinSafeViewport: true,
          frameGeneration: 1,
          viewportFrameGeneration: 1,
          frameMatches: true,
          targetDataGeneration: 1,
          missingPrimitives: [],
        },
      },
    };
  }
  return { primitiveId: "devtools.layout.measure", candidate };
}

function passingReceipt(taskId: string, overrides: ReceiptOverrides = {}): Record<string, unknown> {
  const policy = taskProofPolicy(taskId);
  const canonicalTask = syntheticCatalogBindings.get(taskId);
  const common = {
    schemaVersion: 2,
    tool: "synthetic-producer",
    command: "synthetic.prove",
    taskId,
    catalogBinding: canonicalTask
      ? {
          taskId,
          title: canonicalTask.title,
          sectionSha256: canonicalTask.sectionSha256,
        }
      : null,
    evidenceClass: policy?.acceptedEvidenceClasses[0] ?? "UNIT_BEHAVIOR",
    disposition: "EVALUABLE_PASS",
    pass: true,
    classification: "ok",
    negativeControls: [{ id: `${taskId}-negative`, pass: true }],
    privacy: {
      rawContentReturned: false,
      canaryMatches: 0,
      recursiveCanaryScan: { performed: true, pass: true },
    },
    interference: { monitored: true, disposition: null },
    cleanup: { closed: true, ownedPids: [], ownedSessions: [], ownedBrowserPids: [], survivors: [] },
    producerValidation: { registryVersion: RECEIPT_REGISTRY_VERSION },
  };
  if (!policy?.provesRuntimeBehavior) {
    return { ...common, ...overrides };
  }

  const signedBinary = syntheticArtifactBinary();
  const bounds = { x: 0, y: 0, width: 800, height: 600 };
  const candidate = {
    ...common,
    tool: "script-kit-devtools.layout",
    command: "layout.measure",
    proofMode: "inspection",
    requestedTarget: { selector: { type: "main" } },
    target: { automationId: "main", visible: false, bounds },
    window: { rect: bounds },
    regions: [],
    resizePressure: { windowCanGrow: true },
    pressure: { pressureScore: 0 },
    truthLayers: {
      model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
      rendered: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
      joins: [],
      comparableJoinCount: 1,
    },
    repository: { gitCommit: HEAD },
    binary: signedBinary,
    transaction: {
      transactionId: `proof:${taskId.toLowerCase()}`,
      runId: `proof-run-${taskId.toLowerCase()}`,
      pid: 42,
      processStartTime: "Fri Aug 7 00:00:00 2026",
      binarySha256: signedBinary.sha256,
      automationId: "main",
      windowInstanceId: "main@1",
      windowGeneration: 1,
      windowKind: "Main",
      hostKind: "mainWindow",
      surfaceKind: "ScriptList",
      semanticSurface: "scriptList",
      appViewVariant: "ScriptList",
      bounds,
      targetGeneration: 1,
      surfaceGeneration: 1,
      dataGeneration: 1,
    },
    missingPrimitives: [],
    errors: [],
    ...overrides,
  };
  delete (candidate as Record<string, unknown>).producerValidation;
  const workflowSpec = WORKFLOW_TASK_PROOF_SPECS[taskId as WorkflowTaskProofId];
  if (workflowSpec) {
    const sourceOwners = workflowTaskProofSourceOwners(taskId as WorkflowTaskProofId);
    const transaction = {
      ...candidate.transaction,
      binarySha256: signedBinary.sha256,
    };
    const target = {
      ...candidate.target,
      pid: transaction.pid,
      windowInstanceId: transaction.windowInstanceId,
      targetGeneration: transaction.targetGeneration,
      surfaceGeneration: transaction.surfaceGeneration,
      dataGeneration: transaction.dataGeneration,
    };
    const cleanup = {
      processExited: true,
      streamsDrained: true,
      logWriterClosed: true,
      ownedProcessCount: 0,
      clipboardTouched: false,
      closeError: null,
    };
    return prepareValidatedReceipt(WORKFLOW_TASK_PRIMITIVE_ID, {
      ...candidate,
      tool: "script-kit-devtools.workflow-proof",
      command: "workflow.prove",
      target,
      transaction,
      binary: signedBinary,
      sourceFingerprints: Object.fromEntries(
        sourceOwners.map((path) => [
          path,
          createHash("sha256").update(readFileSync(path)).digest("hex"),
        ]),
      ),
      workflowTaskProof: {
        taskId,
        proofMode: WORKFLOW_TASK_PROOF_MODE,
        producerOwner: workflowSpec.producerOwner,
        sourceOwners,
        observedSegments: [{
          id: `segment:${taskId}`,
          runId: transaction.runId,
          target,
          transaction,
          cleanup,
        }],
        stages: workflowSpec.stageIds.map((id, index) => ({
          id,
          primitiveId: "devtools.act",
          segmentId: `segment:${taskId}`,
          runId: transaction.runId,
          transaction,
          pass: true,
          observation: {
            command: "act.synthetic-fixture",
            requestId: `${taskId}:${index}`,
            resultSha256: createHash("sha256").update(id).digest("hex"),
          },
        })),
        safety: {
          microphoneCaptureStarted: false,
          nativeInputInjected: false,
          liveAiStarted: false,
          screenTakeoverStarted: false,
          clipboardTouched: false,
        },
      },
      negativeControls: workflowSpec.negativeControlIds.map((id) => ({
        id,
        pass: true,
        executed: true,
      })),
      cleanup: { ...candidate.cleanup, ...cleanup },
    }).receipt as Record<string, unknown>;
  }
  const foundation = syntheticFoundationReceipt(taskId, candidate);
  const spec = RUNTIME_TASK_PROOF_SPECS[taskId as keyof typeof RUNTIME_TASK_PROOF_SPECS];
  if (spec) {
    const sourceOwners = [
      "scripts/devtools/lib/runtime-task-proof.ts",
      "scripts/agentic/compiler-input-paths.txt",
      "scripts/devtools/lib/receipt-schema.ts",
      spec.productionOwner,
      spec.runtimeProducer,
    ];
    Object.assign(foundation.candidate, {
      runtimeTaskProof: {
        sourceOwners,
        productionOwner: spec.productionOwner,
        runtimeProducer: spec.runtimeProducer,
        proofMode: spec.proofMode,
      },
      sourceFingerprints: Object.fromEntries(
        sourceOwners.map((path) => [
          path,
          createHash("sha256").update(readFileSync(path)).digest("hex"),
        ]),
      ),
      negativeControls: spec.negativeControlIds.map((id) => ({ id, pass: true })),
    });
  }
  return prepareValidatedReceipt(foundation.primitiveId, foundation.candidate)
    .receipt as Record<string, unknown>;
}

interface Tree {
  receiptsRoot: string;
  fixesPath: string;
  progressPath: string;
  current: CurrentIdentity;
  writeReceipt: (taskId: string, name: string, receipt: Record<string, unknown>) => string;
  writeFile: (relativePath: string, contents: string) => string;
  runTask: (taskId: string) => { receipt: Record<string, any>; exitCode: number };
  runScope: () => { receipt: Record<string, any>; exitCode: number };
  runAll: () => { receipt: Record<string, any>; exitCode: number };
}

function setup(options: {
  catalog?: string;
  progress?: string;
  receiptTaskIds?: Iterable<string>;
  skipReceiptsFor?: Set<string>;
} = {}): Tree {
  const base = mkdtempSync(join(tmpdir(), "consistency-audit-test-"));
  const receiptsRoot = join(base, "artifacts");
  mkdirSync(receiptsRoot, { recursive: true });
  const fixesPath = join(base, "CONSISTENCY-FIXES.md");
  const progressPath = join(base, "CONSISTENCY-PROGRESS.md");
  writeFileSync(fixesPath, options.catalog ?? catalogMarkdown());
  writeFileSync(progressPath, options.progress ?? progressMarkdown());
  const current = currentIdentity({ headCommit: HEAD });

  const writeReceipt = (taskId: string, name: string, receipt: Record<string, unknown>) => {
    const dir = join(receiptsRoot, taskId);
    mkdirSync(dir, { recursive: true });
    const path = join(dir, name);
    writeFileSync(path, JSON.stringify(receipt, null, 2));
    return path;
  };
  const writeFile = (relativePath: string, contents: string) => {
    const path = join(receiptsRoot, relativePath);
    mkdirSync(join(path, ".."), { recursive: true });
    writeFileSync(path, contents);
    return path;
  };

  // Most mutation tests inspect GOV-002 only. Materializing every scope task
  // for each case performs thousands of unrelated filesystem writes and makes
  // honest per-test timeouts flaky under an unrelated Rust compile.
  const receiptIds = options.receiptTaskIds ?? ["GOV-002"];
  for (const taskId of receiptIds) {
    if (options.skipReceiptsFor?.has(taskId)) continue;
    writeReceipt(taskId, "proof.json", passingReceipt(taskId));
  }

  const catalogParsed = () => parseTaskCatalog(readFile(fixesPath), fixesPath);
  const progressParsed = () => parseProgressSections(readFile(progressPath), progressPath);
  const readFile = (path: string) => require("node:fs").readFileSync(path, "utf8") as string;

  return {
    receiptsRoot,
    fixesPath,
    progressPath,
    current,
    writeReceipt,
    writeFile,
    runTask: (taskId: string) =>
      verifyTask({
        taskId,
        scope: "cons-proof-gov",
        receiptsRoot,
        catalog: catalogParsed(),
        progress: progressParsed(),
        current,
      }),
    runScope: () =>
      verifyScope({
        scope: "cons-proof-gov",
        fixesPath,
        progressPath,
        receiptsRoot,
        current,
      }),
    runAll: () =>
      verifyAll({
        fixesPath,
        progressPath,
        receiptsRoot,
        current,
      }),
  };
}

function errorCodes(receipt: Record<string, any>): string[] {
  return (receipt.errors ?? []).map((error: any) => error.code);
}

function taskErrorCodes(tree: Tree, taskId: string): { codes: string[]; receipt: Record<string, any>; exitCode: number } {
  const { receipt, exitCode } = tree.runTask(taskId);
  return { codes: errorCodes(receipt), receipt, exitCode };
}

describe("canonical ID sets", () => {
  test("program has 75 tasks and both independent proof/workflow scopes contain exactly 28", () => {
    expect(PROGRAM_IDS.size).toBe(75);
    expect(CONS_PROOF_GOV_IDS.size).toBe(28);
    expect(CONS_FLOW_UX_IDS.size).toBe(28);
    for (const id of CONS_PROOF_GOV_IDS) expect(PROGRAM_IDS.has(id)).toBe(true);
    for (const id of CONS_FLOW_UX_IDS) {
      expect(PROGRAM_IDS.has(id)).toBe(true);
      expect(CONS_PROOF_GOV_IDS.has(id)).toBe(false);
      expect(id.startsWith("SAFE-") || id.startsWith("WF-")).toBe(true);
    }
    expect(PROGRAM_IDS.has("GOV-001")).toBe(true);
    expect(CONS_PROOF_GOV_IDS.has("GOV-001")).toBe(false);
    expect(CONS_PROOF_GOV_IDS.has("SAFE-001")).toBe(false);
    expect(CONS_FLOW_UX_IDS.has("SAFE-001")).toBe(true);
    expect(CONS_FLOW_UX_IDS.has("WF-024")).toBe(true);
    expect(CONS_FLOW_UX_IDS.has("PF-001")).toBe(false);
  });

  test("every canonical task has an explicit evidence policy", () => {
    expect(TASK_PROOF_POLICIES.size).toBe(75);
    expect([...TASK_PROOF_POLICIES.keys()].sort()).toEqual(
      [...PROGRAM_IDS].sort(),
    );
    expect(taskProofPolicy("PF-009")?.requirement).toBe("static-inventory");
    expect(taskProofPolicy("PF-010")?.requirement).toBe("fixture-contract");
    expect(taskProofPolicy("GOV-002")?.requirement).toBe("unit-behavior");
    for (const taskId of ["SAFE-001", "PF-004", "UX-001", "WF-001", "GEO-002"]) {
      expect(taskProofPolicy(taskId)?.requirement).toBe("direct-runtime");
      expect(taskProofPolicy(taskId)?.acceptedEvidenceClasses).not.toContain(
        "STATIC_INVENTORY",
      );
    }
  });

  test("the real repository catalog parses to the exact 75-ID set", () => {
    const markdown = require("node:fs").readFileSync(
      DEFAULT_CONSISTENCY_CATALOG_PATH,
      "utf8",
    );
    const catalog = parseTaskCatalog(markdown, DEFAULT_CONSISTENCY_CATALOG_PATH);
    expect(catalog.tasks.length).toBe(75);
    expect(catalog.errors).toEqual([]);
    expect(catalog.path).toBe("scripts/devtools/consistency-catalog.md");
    expect(catalog.byId.get("PF-009")?.title).toBe(
      "Generate typed coverage bindings for all 37 kinds and 54 mappings",
    );
    expect(catalog.byId.get("GOV-006")?.title).toBe(
      "Add a final consistency completion auditor",
    );
  });
});

describe("catalog parser mutations", () => {
  test("unknown task ID (replacement) is rejected with both unknown and missing codes", () => {
    const ids = [...PROGRAM_IDS].map((id) => (id === "GOV-007" ? "GOV-999" : id));
    const tree = setup({ catalog: catalogMarkdown(ids) });
    const catalog = parseTaskCatalog(catalogMarkdown(ids), tree.fixesPath);
    const codes = catalog.errors.map((error) => error.code);
    expect(codes).toContain("unknown-task-id");
    expect(codes).toContain("missing-task-id");
    const scope = tree.runScope();
    expect(scope.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(scope.exitCode).toBe(4);
  });

  test("duplicate task ID is rejected with both line numbers", () => {
    const catalog = parseTaskCatalog(catalogMarkdown([...PROGRAM_IDS, "GOV-002"]));
    const duplicate = catalog.errors.find((error) => error.code === "duplicate-task-id");
    expect(duplicate).toBeDefined();
    expect(duplicate!.detail).toMatch(/GOV-002 defined at lines \d+ and \d+/);
    const tree = setup({ catalog: catalogMarkdown([...PROGRAM_IDS, "GOV-002"]) });
    expect(tree.runScope().exitCode).toBe(4);
  });

  test("a task heading hidden inside a code fence does not count and is reported", () => {
    const ids = [...PROGRAM_IDS].filter((id) => id !== "GOV-007");
    const fenced = ["```", "### GOV-007 — Synthetic GOV-007", "```"];
    const catalog = parseTaskCatalog(catalogMarkdown(ids, fenced));
    const codes = catalog.errors.map((error) => error.code);
    expect(codes).toContain("missing-task-id");
    expect(codes).toContain("task-heading-inside-code-fence");
    expect(catalog.byId.has("GOV-007")).toBe(false);
  });

  test("a malformed nearly-valid heading is rejected", () => {
    const catalog = parseTaskCatalog(catalogMarkdown(PROGRAM_IDS, ["### GOV-08 - wrong grammar"]));
    expect(catalog.errors.map((error) => error.code)).toContain("malformed-task-heading");
  });
});

describe("verify-task mutations", () => {
  test("source fingerprints invalidate missing files and same-size rewrites at one path", () => {
    const directory = mkdtempSync(join(tmpdir(), "consistency-fingerprint-drift-"));
    const path = join(directory, "source.txt");
    expect(fileSha256(path)).toBeNull();
    writeFileSync(path, "aaaa");
    const first = fileSha256(path);
    expect(first).toBe(createHash("sha256").update("aaaa").digest("hex"));
    const originalStat = statSync(path);
    writeFileSync(path, "bbbb");
    utimesSync(path, originalStat.atime, originalStat.mtime);
    expect(fileSha256(path)).toBe(createHash("sha256").update("bbbb").digest("hex"));
    rmSync(directory, { recursive: true, force: true });
  });

  test("baseline synthetic scope task passes", () => {
    const tree = setup();
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(receipt.disposition).toBe("EVALUABLE_PASS");
    expect(receipt.pass).toBe(true);
    expect(exitCode).toBe(0);
    expect(receipt.producerValidation.valid).toBe(true);
    expect(receipt.proofPolicy.requirement).toBe("unit-behavior");
  });

  test("unit and static receipts can never complete a runtime interaction task", () => {
    for (const evidenceClass of ["UNIT_BEHAVIOR", "STATIC_INVENTORY", "FIXTURE_CONTRACT"]) {
      const tree = setup({ receiptTaskIds: ["SAFE-001"] });
      tree.writeReceipt("SAFE-001", "proof.json", {
        ...passingReceipt("GOV-002"),
        taskId: "SAFE-001",
        evidenceClass,
      });
      const { codes, receipt, exitCode } = taskErrorCodes(tree, "SAFE-001");
      expect(codes).toContain("task-evidence-class-not-accepted");
      expect(receipt.disposition).toBe("INVALID_SCHEMA");
      expect(exitCode).toBe(4);
    }
  });

  test("claimed hidden runtime proof requires observed visibility and registry validation", () => {
    const unobserved = setup({ receiptTaskIds: ["UX-001"] });
    unobserved.writeReceipt("UX-001", "proof.json", {
      ...passingReceipt("GOV-002"),
      taskId: "UX-001",
      evidenceClass: "RUNTIME_HIDDEN",
    });
    const badObservation = taskErrorCodes(unobserved, "UX-001");
    expect(badObservation.codes).toContain("task-evidence-observation-invalid");
    expect(badObservation.codes).toContain(
      "task-runtime-proof-not-registry-validated",
    );
    expect(badObservation.exitCode).toBe(4);

    const unregistered = setup({ receiptTaskIds: ["WF-001"] });
    unregistered.writeReceipt("WF-001", "proof.json", {
      ...passingReceipt("GOV-002"),
      taskId: "WF-001",
      evidenceClass: "RUNTIME_HIDDEN",
      target: { visible: false },
      transaction: { transactionId: "invented" },
      producerValidation: { registryVersion: RECEIPT_REGISTRY_VERSION, valid: true },
    });
    const badProducer = taskErrorCodes(unregistered, "WF-001");
    expect(badProducer.codes).toContain(
      "task-runtime-proof-not-registry-validated",
    );
    expect(badProducer.exitCode).toBe(4);
  });

  test("registered hidden producer evidence can discharge a runtime task", () => {
    const tree = setup({ receiptTaskIds: ["UX-001"] });
    const { receipt, exitCode } = tree.runTask("UX-001");
    expect(receipt.proofPolicy.requirement).toBe("direct-runtime");
    expect(receipt.disposition).toBe("EVALUABLE_PASS");
    expect(exitCode).toBe(0);
  });

  test.each([...CONS_FLOW_UX_IDS])(
    "a registered layout inspection cannot discharge the %s safety or workflow journey",
    (taskId) => {
      const tree = setup({ receiptTaskIds: [] });
      const binding = syntheticCatalogBindings.get(taskId)!;
      tree.writeReceipt(taskId, "proof.json", passingReceipt("UX-001", {
        taskId,
        catalogBinding: {
          taskId,
          title: binding.title,
          sectionSha256: binding.sectionSha256,
        },
      }));
      const { receipt, exitCode } = tree.runTask(taskId);
      expect(exitCode).toBe(4);
      expect(receipt.errors.map((error: { code: string }) => error.code))
        .toContain("task-workflow-proof-contract-missing");
    },
  );

  test("a registered layout inspector cannot falsely satisfy semantic projection", () => {
    const tree = setup({ receiptTaskIds: ["PF-004"] });
    const binding = syntheticCatalogBindings.get("PF-004")!;
    tree.writeReceipt("PF-004", "proof.json", passingReceipt("UX-001", {
      taskId: "PF-004",
      catalogBinding: {
        taskId: "PF-004",
        title: binding.title,
        sectionSha256: binding.sectionSha256,
      },
    }));
    const { receipt, exitCode } = tree.runTask("PF-004");
    expect(exitCode).toBe(4);
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(receipt.errors.map((error: { code: string }) => error.code))
      .toContain("task-runtime-primitive-mismatch");
  });

  test("the correct registered primitive still cannot substitute inspection for real proof", () => {
    const tree = setup({ receiptTaskIds: ["PF-004"] });
    const receipt = passingReceipt("PF-004");
    (receipt.semanticProjection as Record<string, unknown>).proofMode = "inspection";
    tree.writeReceipt("PF-004", "proof.json", receipt);
    const result = tree.runTask("PF-004");
    expect(result.exitCode).toBe(4);
    expect(result.receipt.errors.map((error: { code: string }) => error.code))
      .toContain("task-runtime-proof-mode-mismatch");
  });

  test("foundation proof cannot omit its actual runtime owner or executed adversarial cases", () => {
    const tree = setup({ receiptTaskIds: ["PF-004"] });
    const ownerless = passingReceipt("PF-004");
    delete ownerless.runtimeTaskProof;
    tree.writeReceipt("PF-004", "proof.json", ownerless);
    const missingOwner = tree.runTask("PF-004");
    expect(missingOwner.exitCode).toBe(4);
    expect(missingOwner.receipt.errors.map((error: { code: string }) => error.code))
      .toContain("task-runtime-proof-source-ownership-mismatch");

    const missingControl = passingReceipt("PF-004");
    (missingControl.negativeControls as unknown[]).pop();
    tree.writeReceipt("PF-004", "controls.json", missingControl);
    const missingAdversary = tree.runTask("PF-004");
    expect(missingAdversary.exitCode).toBe(4);
    expect(missingAdversary.receipt.errors.map((error: { code: string }) => error.code))
      .toContain("task-runtime-required-negative-control-missing");
  });

  test("swapped GOV-006/GOV-007 obligation IDs and section hashes never pass", () => {
    for (const [requested, substituted] of [
      ["GOV-006", "GOV-007"],
      ["GOV-007", "GOV-006"],
    ]) {
      const tree = setup();
      const wrong = passingReceipt(substituted!);
      tree.writeReceipt(requested!, "proof.json", {
        ...wrong,
        taskId: requested,
      });
      const result = taskErrorCodes(tree, requested!);
      expect(result.codes).toContain("task-canonical-catalog-binding-mismatch");
      expect(result.receipt.disposition).toBe("INVALID_SCHEMA");
      expect(result.exitCode).toBe(4);
    }
  });

  test("matching task text without its canonical section identity is not proof", () => {
    const tree = setup();
    tree.writeReceipt("GOV-006", "proof.json", {
      ...passingReceipt("GOV-006"),
      catalogBinding: {
        taskId: "GOV-006",
        title: "Synthetic GOV-006",
        sectionSha256: "0".repeat(64),
      },
    });
    const result = taskErrorCodes(tree, "GOV-006");
    expect(result.codes).toContain("task-canonical-catalog-binding-mismatch");
    expect(result.exitCode).toBe(4);
  });

  test("missing task receipt blocks with missing-task-receipt", () => {
    const tree = setup({ skipReceiptsFor: new Set(["GOV-002"]) });
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("missing-task-receipt");
    expect(receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(exitCode).toBe(3);
  });

  test("unknown task ID is invalid", () => {
    const tree = setup();
    const { receipt, exitCode } = tree.runTask("ZZZ-999");
    expect(errorCodes(receipt)).toContain("unknown-task-id");
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("missing progress section is invalid with missing-progress-section", () => {
    const ids = [...PROGRAM_IDS].filter((id) => id !== "GOV-002");
    const tree = setup({ progress: progressMarkdown(ids) });
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("missing-progress-section");
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("duplicate progress section is invalid with duplicate-progress-section", () => {
    const tree = setup({ progress: progressMarkdown(PROGRAM_IDS, ["GOV-002"]) });
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("duplicate-progress-section");
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("a pass receipt with required missing primitives is invalid", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      missingPrimitives: ["devtools.act"],
    }));
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("pass-receipt-missing-primitives");
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("stale registry version blocks as stale generation", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      producerValidation: { registryVersion: RECEIPT_REGISTRY_VERSION + 1 },
    }));
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(receipt.staleReasons.map((reason: any) => reason.code)).toContain("stale-registry-version");
    expect(receipt.disposition).toBe("BLOCKED_STALE_GENERATION");
    expect(exitCode).toBe(3);
  });

  test("stale registry fingerprint blocks as stale generation", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      producerValidation: {
        registryVersion: RECEIPT_REGISTRY_VERSION,
        registryFingerprint: "not-the-current-registry-fingerprint",
      },
    }));
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(receipt.staleReasons.map((reason: any) => reason.code)).toContain("stale-registry-fingerprint");
    expect(exitCode).toBe(3);
  });

  test("stale producer fingerprint blocks as stale generation", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      tool: "script-kit-devtools.scroll",
      repository: { producerSourceFingerprint: "not-the-current-producer-fingerprint" },
    }));
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(receipt.staleReasons.map((reason: any) => reason.code)).toContain("stale-producer");
    expect(exitCode).toBe(3);
  });

  test("offline behavior evidence rejects stale suite bytes and unowned source paths", () => {
    const tree = setup();
    const sourcePath = "scripts/devtools/privacy.test.ts";
    const stale = receiptStaleReasons({
      path: "synthetic-proof.json",
      disposition: "EVALUABLE_PASS",
      archived: false,
      receipt: {
        primitiveId: "devtools.consistency.safe-task-proof",
        sourceFingerprints: { [sourcePath]: WRONG_SHA },
      },
    }, tree.current);
    expect(stale.map((reason) => reason.code)).toContain("stale-proof-source");

    const unowned = receiptStaleReasons({
      path: "synthetic-proof.json",
      disposition: "EVALUABLE_PASS",
      archived: false,
      receipt: {
        primitiveId: "devtools.consistency.safe-task-proof",
        sourceFingerprints: { "../../private-user-data": "a".repeat(64) },
      },
    }, tree.current);
    expect(unowned.map((reason) => reason.code)).toContain("stale-proof-source-owner");
  });

  test("shared workflow suite is fresh for offline tasks but its production owner belongs only to GOV-006", () => {
    const tree = setup();
    const reviewedSuite = "scripts/agentic/cons-flow-ux/final-workflow-audit.test.ts";
    const reviewedOwner = "scripts/agentic/cons-flow-ux/final-workflow-audit.ts";
    const suiteHash = tree.current.fileSha256(reviewedSuite)!;
    const ownerHash = tree.current.fileSha256(reviewedOwner)!;
    const entry = (taskId: string, sourceFingerprints: Record<string, string>) => ({
      path: "synthetic-proof.json",
      disposition: "EVALUABLE_PASS" as const,
      archived: false,
      receipt: {
        primitiveId: "devtools.consistency.safe-task-proof",
        taskId,
        sourceFingerprints,
      },
    });

    expect(receiptStaleReasons(entry("GEO-001", {
      [reviewedSuite]: suiteHash,
    }), tree.current)).toEqual([]);
    expect(receiptStaleReasons(entry("GOV-006", {
      [reviewedSuite]: suiteHash,
      [reviewedOwner]: ownerHash,
    }), tree.current)).toEqual([]);
    expect(receiptStaleReasons(entry("GEO-001", {
      [reviewedOwner]: ownerHash,
    }), tree.current).map((reason) => reason.code)).toContain("stale-proof-source-owner");
    expect(receiptStaleReasons(entry("GOV-006", {
      "scripts/agentic/macos-input.test.ts": "a".repeat(64),
    }), tree.current).map((reason) => reason.code)).toContain("stale-proof-source-owner");
  });

  test("stale implementation fingerprint is detected on a re-read aggregate", () => {
    const tree = setup();
    const { receipt } = tree.runTask("GOV-002");
    expect(receipt.disposition).toBe("EVALUABLE_PASS");
    expect(stalenessReasons(receipt, tree.current)).toEqual([]);
    const tampered = { ...receipt, implementationFingerprint: WRONG_SHA };
    const reasons = stalenessReasons(tampered, tree.current).map((reason) => reason.code);
    expect(reasons).toContain("stale-implementation");
  });

  test("wrong fixture SHA blocks as stale generation", () => {
    const tree = setup();
    const fixturePath = tree.writeFile("fixtures/synthetic-fixture.json", "{\"id\":\"fx\"}\n");
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      fixture: { id: "fx", path: fixturePath, sha256: WRONG_SHA },
    }));
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(receipt.staleReasons.map((reason: any) => reason.code)).toContain("stale-fixture");
    expect(exitCode).toBe(3);
  });

  test("wrong binary SHA blocks as stale generation", () => {
    const tree = setup();
    const binaryPath = tree.writeFile("bin/synthetic-binary", "not-a-real-binary\n");
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      binary: { path: binaryPath, sha256: WRONG_SHA, sourceCommit: HEAD },
    }));
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(receipt.staleReasons.map((reason: any) => reason.code)).toContain("stale-binary");
    expect(exitCode).toBe(3);
  });

  test("binary sourceCommit != HEAD blocks as stale generation", () => {
    const tree = setup();
    const binaryPath = tree.writeFile("bin/synthetic-binary", "not-a-real-binary\n");
    const sha = require("node:crypto")
      .createHash("sha256")
      .update(require("node:fs").readFileSync(binaryPath))
      .digest("hex");
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      binary: { path: binaryPath, sha256: sha, sourceCommit: "a".repeat(40) },
    }));
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(receipt.staleReasons.map((reason: any) => reason.code)).toContain("stale-binary-source-commit");
    expect(exitCode).toBe(3);
  });

  test("failed negative control is an evaluable failure", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      negativeControls: [{ id: "one-point-overflow", pass: false }],
    }));
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("failed-negative-control");
    expect(receipt.disposition).toBe("EVALUABLE_FAIL");
    expect(exitCode).toBe(2);
  });

  test("INVALID_OBSERVER marked pass is invalid", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "observer.json", passingReceipt("GOV-002", {
      disposition: "INVALID_OBSERVER",
      classification: "invalid-observer",
      pass: true,
    }));
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("invalid-receipt-marked-pass");
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("INVALID_INTERFERENCE marked pass is interference pass-through", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "interference.json", passingReceipt("GOV-002", {
      disposition: "INVALID_INTERFERENCE",
      classification: "invalid-interference",
      pass: true,
    }));
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("interference-pass-through");
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("BLOCKED_PERMISSION marked pass is invalid", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "blocked.json", passingReceipt("GOV-002", {
      disposition: "BLOCKED_PERMISSION",
      classification: "blocked-permission",
      pass: true,
    }));
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("blocked-receipt-marked-pass");
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("cleanup survivor is invalid cleanup", () => {
    const tree = setup();
    tree.writeReceipt("GOV-002", "proof.json", passingReceipt("GOV-002", {
      cleanup: { closed: true, ownedPids: [], ownedSessions: [], ownedBrowserPids: [], survivors: [4242] },
    }));
    const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
    expect(codes).toContain("cleanup-survivor");
    expect(receipt.disposition).toBe("INVALID_CLEANUP");
    expect(exitCode).toBe(4);
  });

  test("archived directories are preserved history, never current evidence", () => {
    const tree = setup({ skipReceiptsFor: new Set(["GOV-002"]) });
    const dir = join(tree.receiptsRoot, "GOV-002", "attempts");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "old-invalid.json"), JSON.stringify(passingReceipt("GOV-002", {
      disposition: "INVALID_INTERFERENCE",
      classification: "invalid-interference",
      pass: false,
    })));
    const { receipt } = tree.runTask("GOV-002");
    // The archived invalid attempt neither passes the task nor invalidates it.
    expect(receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(receipt.archivedReceiptCount).toBe(1);
  });

  test("the exact typed GOV-002 facade ledger is an auxiliary artifact, never task proof", () => {
    const tree = setup();
    tree.writeFile(
      "GOV-002/facade-ledger.json",
      JSON.stringify(passingFacadeLedger()),
    );
    const { receipt, exitCode } = tree.runTask("GOV-002");
    expect(exitCode).toBe(0);
    expect(receipt.positiveReceiptPaths.length).toBe(1);
    expect(receipt.evidenceArtifactPaths[0]).toContain("facade-ledger.json");
  });

  test("a malformed or task-receipt-shaped facade-ledger filename fails closed", () => {
    const complete = passingFacadeLedger();
    const scope = complete.facadeMigrations as Record<string, unknown>;
    const facades = scope.facades as Array<Record<string, unknown>>;
    for (const collision of [
      passingReceipt("GOV-002"),
      {
        schemaVersion: 1,
        generatedBy: "scripts/devtools/facade-ledger.ts",
        taskId: "GOV-002",
        evidenceClass: "RUNTIME_HIDDEN",
        provesRuntimeBehavior: true,
        assertions: {},
        disposition: "EVALUABLE_PASS",
      },
      {
        ...complete,
        facadeMigrations: { ...scope, facades: facades.slice(0, 1) },
        facades: facades.slice(0, 1),
      },
      {
        ...complete,
        facades: facades.slice(0, 1),
      },
      {
        ...complete,
        facadeMigrations: {
          ...scope,
          facades: facades.map((facade) =>
            facade.id === "popup-window"
              ? { ...facade, canonicalOwner: CONVERSATION_STYLE_OWNER }
              : facade,
          ),
        },
      },
      {
        ...complete,
        provesExporterByteEquality: true,
      },
    ]) {
      const tree = setup();
      tree.writeFile("GOV-002/facade-ledger.json", JSON.stringify(collision));
      const { codes, receipt, exitCode } = taskErrorCodes(tree, "GOV-002");
      expect(codes).toContain("unreadable-receipt");
      expect(receipt.disposition).toBe("INVALID_SCHEMA");
      expect(exitCode).toBe(4);
    }
  });
});

describe("verify-family mutations", () => {
  const binding = (familyId: string) => ({
    familyId,
    appView: "ScriptList",
    host: "MainWindow",
    memberReceiptPaths: ["families/main-menu/member-0.json"],
  });

  const runtimeMember = (
    overrides: Record<string, unknown> = {},
  ): Record<string, unknown> => {
    const binary = syntheticArtifactBinary();
    const candidate = {
      schemaVersion: 2,
      tool: "script-kit-devtools.layout",
      command: "layout.measure",
      classification: "ok",
      evidenceClass: "RUNTIME_HIDDEN",
      proofMode: "inspection",
      requestedTarget: { selector: { type: "main" } },
      target: {
        automationId: "main",
        visible: false,
        bounds: { x: 0, y: 0, width: 800, height: 600 },
      },
      window: { rect: { x: 0, y: 0, width: 800, height: 600 } },
      regions: [],
      resizePressure: { windowCanGrow: true },
      pressure: { pressureScore: 0 },
      truthLayers: {
        model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        rendered: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        joins: [],
        comparableJoinCount: 1,
      },
      repository: { gitCommit: HEAD },
      binary,
      cleanup: { closed: true, ownedPids: [], ownedSessions: [], ownedBrowserPids: [], survivors: [] },
      transaction: {
        transactionId: "proof:family-member",
        runId: "family-member-test",
        pid: 42,
        processStartTime: "Fri Aug 7 00:00:00 2026",
        binarySha256: binary.sha256,
        automationId: "main",
        windowInstanceId: "main@1",
        windowGeneration: 1,
        windowKind: "Main",
        hostKind: "MainWindow",
        surfaceKind: "ScriptList",
        semanticSurface: "scriptList",
        appViewVariant: "ScriptList",
        bounds: { x: 0, y: 0, width: 800, height: 600 },
        targetGeneration: 1,
        surfaceGeneration: 1,
        dataGeneration: 1,
      },
      missingPrimitives: [],
      errors: [],
      ...overrides,
    };
    return prepareValidatedReceipt("devtools.layout.measure", candidate)
      .receipt as Record<string, unknown>;
  };

  test("declared binding passes", () => {
    const tree = setup();
    tree.writeFile("families/main-menu/fixture.json", JSON.stringify(binding("main-menu")));
    tree.writeFile(
      "families/main-menu/member-0.json",
      JSON.stringify(runtimeMember()),
    );
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(receipt.disposition).toBe("EVALUABLE_PASS");
    expect(receipt.evidenceClass).toBe("DIRECT_RUNTIME_PROOF");
    expect(receipt.runtimeProofCount).toBe(1);
    expect(exitCode).toBe(0);
  });

  test("a declared receipt path is not evidence when the file is missing", () => {
    const tree = setup();
    tree.writeFile(
      "families/main-menu/fixture.json",
      JSON.stringify(binding("main-menu")),
    );
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain(
      "missing-or-unreadable-family-member-receipt",
    );
    expect(receipt.runtimeProofCount).toBe(0);
    expect(receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(exitCode).toBe(3);
  });

  test("static family inventories cannot masquerade as target-scoped runtime proof", () => {
    const tree = setup();
    tree.writeFile(
      "families/main-menu/fixture.json",
      JSON.stringify(binding("main-menu")),
    );
    tree.writeFile(
      "families/main-menu/member-0.json",
      JSON.stringify(runtimeMember({ evidenceClass: "STATIC_INVENTORY" })),
    );
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain(
      "family-member-not-direct-runtime-evidence",
    );
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("runtime receipts for another AppView cannot prove this family", () => {
    const tree = setup();
    tree.writeFile(
      "families/main-menu/fixture.json",
      JSON.stringify(binding("main-menu")),
    );
    const member = runtimeMember();
    (member.transaction as Record<string, unknown>).appViewVariant = "SettingsView";
    tree.writeFile("families/main-menu/member-0.json", JSON.stringify(member));
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain(
      "family-member-target-identity-mismatch",
    );
    expect(receipt.disposition).toBe("INVALID_IDENTITY");
    expect(exitCode).toBe(4);
  });

  test("stale source identity cannot satisfy a family runtime proof", () => {
    const tree = setup();
    tree.writeFile(
      "families/main-menu/fixture.json",
      JSON.stringify(binding("main-menu")),
    );
    const member = runtimeMember();
    (member.repository as Record<string, unknown>).gitCommit = "0".repeat(40);
    tree.writeFile("families/main-menu/member-0.json", JSON.stringify(member));
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain("stale-family-member-receipt");
    expect(receipt.disposition).toBe("BLOCKED_STALE_GENERATION");
    expect(exitCode).toBe(3);
  });

  test("family member receipts cannot escape their owned receipts root", () => {
    const tree = setup();
    tree.writeFile(
      "families/main-menu/fixture.json",
      JSON.stringify({
        ...binding("main-menu"),
        memberReceiptPaths: ["../someone-elses-receipt.json"],
      }),
    );
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain(
      "family-member-path-escapes-receipts-root",
    );
    expect(receipt.disposition).toBe("INVALID_SCHEMA");
    expect(exitCode).toBe(4);
  });

  test("missing family binding is blocked with missing-family-binding", () => {
    const tree = setup();
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain("missing-family-binding");
    expect(receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(exitCode).toBe(3);
  });

  test("wrong family AppView/host fails with wrong-family-appview-host", () => {
    const tree = setup();
    tree.writeFile("families/main-menu/fixture.json", JSON.stringify(binding("script-prompt")));
    const { receipt, exitCode } = verifyFamily({
      familyId: "main-menu",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain("wrong-family-appview-host");
    expect(receipt.disposition).toBe("EVALUABLE_FAIL");
    expect(exitCode).toBe(2);
  });

  test("unknown family ID is invalid", () => {
    const tree = setup();
    const { receipt, exitCode } = verifyFamily({
      familyId: "not-a-family",
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });
    expect(errorCodes(receipt)).toContain("unknown-family-id");
    expect(exitCode).toBe(4);
  });

  test("all nine plan families are known", () => {
    expect(FAMILY_IDS.length).toBe(9);
  });
});

describe("verify-scope and verify-all", () => {
  describe("complete synthetic proof/governance scope", () => {
    let tree: Tree;
    let scope: { receipt: Record<string, any>; exitCode: number };

    beforeAll(() => {
      tree = setup({ receiptTaskIds: CONS_PROOF_GOV_IDS });
      // Share a real source-bound scope audit, not a second copy of its proof
      // preparation. Both observations use this unchanged receipt tree.
      scope = tree.runScope();
    });

    test("complete synthetic scope passes and registry validation agrees", () => {
      const { receipt, exitCode } = scope;
      expect(receipt.disposition).toBe("EVALUABLE_PASS");
      expect(receipt.scopePassedTaskCount).toBe(28);
      expect(receipt.missingScopeTaskIds).toEqual([]);
      expect(exitCode).toBe(0);
      const validation = validateReceipt("devtools.consistency.verify-scope", receipt);
      expect(validation.valid).toBe(true);
      expect(validation.disposition).toBe("EVALUABLE_PASS");
    });

    test("a passing 28-task scope NEVER makes verify-all pass; the 47 absent program tasks are named", () => {
      expect(scope.exitCode).toBe(0);
      expect(scope.receipt.scopePassedTaskCount).toBe(28);
      expect(scope.receipt.missingScopeTaskIds).toEqual([]);
      const all = tree.runAll();
      expect(all.exitCode).not.toBe(0);
      const expectedMissing = [...PROGRAM_IDS].filter((id) => !CONS_PROOF_GOV_IDS.has(id)).sort();
      expect(expectedMissing.length).toBe(47);
      expect([...all.receipt.missingTaskIds].sort()).toEqual(expectedMissing);
      expect(errorCodes(all.receipt)).toContain("missing-run-manifest");
      expect(all.receipt.proofCoverage.runtimeInteractionRequiredTaskCount).toBeGreaterThan(50);
      expect(all.receipt.proofCoverage.runtimeInteractionProvenTaskCount).toBeGreaterThan(0);
      expect(all.receipt.proofCoverage.runtimeInteractionBlockedTaskIds).toContain("SAFE-001");
      expect(all.receipt.proofCoverage.runtimeInteractionBlockedTaskIds).toContain("UX-001");
      expect(all.receipt.proofCoverage.runtimeInteractionBlockedTaskIds).toContain("WF-001");
      expect(all.receipt.proofCoverage.note).toContain("never count as direct runtime");
    });
  });

  test("one missing scope task keeps the scope nonzero with its exact ID", () => {
    const tree = setup({
      receiptTaskIds: CONS_PROOF_GOV_IDS,
      skipReceiptsFor: new Set(["PF-007"]),
    });
    const { receipt, exitCode } = tree.runScope();
    expect(receipt.missingScopeTaskIds).toEqual(["PF-007"]);
    expect(receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(exitCode).toBe(3);
  });

  test("workflow metadata requires explicit immutable identity without legacy provenance", () => {
    const receipt = passingReceipt("SAFE-001");
    expect(validateReceipt(WORKFLOW_TASK_PRIMITIVE_ID, receipt).valid).toBe(true);
    const mutations: Array<(binary: Record<string, unknown>) => void> = [
      (binary) => { delete binary.artifactReference; },
      (binary) => { binary.manifestSha256 = WRONG_SHA; },
      (binary) => { binary.path = "target-agent/runtime/unowned/script-kit-gpui"; },
      (binary) => { binary.provenance = { path: `${binary.path}.provenance.json` }; },
    ];
    for (const mutate of mutations) {
      const candidate = structuredClone(receipt);
      mutate(candidate.binary as Record<string, unknown>);
      const validation = validateReceipt(WORKFLOW_TASK_PRIMITIVE_ID, candidate);
      expect(validation.valid).toBe(false);
      expect(validation.errors.join("; ")).toContain("explicit immutable artifact reference");
    }
  });

  // This aggregate validates all 28 direct runtime receipts against source/artifact
  // bytes in scope and program audits; its observed 56s run needs a bounded 120s deadline.
  test("a complete workflow scope requires all 28 exact, source-current direct runtime receipts", () => {
    const tree = setup({ receiptTaskIds: CONS_FLOW_UX_IDS });
    const { receipt, exitCode } = verifyScope({
      scope: "cons-flow-ux",
      fixesPath: tree.fixesPath,
      progressPath: tree.progressPath,
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });

    expect(receipt.scope).toBe("cons-flow-ux");
    expect(receipt.scopeTaskCount).toBe(28);
    expect(receipt.scopePassedTaskCount).toBe(28);
    expect(receipt.missingScopeTaskIds).toEqual([]);
    expect(receipt.headCommit).toBe(HEAD);
    expect(receipt.disposition).toBe("EVALUABLE_PASS");
    expect(exitCode).toBe(0);
    expect(validateReceipt("devtools.consistency.verify-scope", receipt).valid).toBe(true);

    const all = tree.runAll();
    expect(all.exitCode).not.toBe(0);
    expect(all.receipt.missingTaskIds.length).toBe(47);
  }, 120_000);

  test("a fabricated legacy workflow lane cannot discharge any canonical runtime obligation", () => {
    const tree = setup({ receiptTaskIds: [] });
    tree.writeFile("cons-flow-ux/final-audit/lane-receipt.json", JSON.stringify({
      verdict: "PASS",
      productCommit: "493769e03208b411790f2b5639222179d8bd7eff",
      taskCoverage: { expected: 28, passed: 28, taskIds: [...CONS_FLOW_UX_IDS] },
      focusedMatrix: { "ai::agent_chat": { passed: 625, failed: 0 } },
      governance: { protectedGlassContracts: { passed: 40, failed: 0 } },
    }));
    const { receipt, exitCode } = verifyScope({
      scope: "cons-flow-ux",
      fixesPath: tree.fixesPath,
      progressPath: tree.progressPath,
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });

    expect(receipt.scopeTaskCount).toBe(28);
    expect(receipt.scopePassedTaskCount).toBe(0);
    expect(receipt.missingScopeTaskIds).toEqual([...CONS_FLOW_UX_IDS].sort());
    expect(receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(exitCode).toBe(3);
  });

  test("one missing exact workflow receipt names its task without weakening the other 27", () => {
    const tree = setup({
      receiptTaskIds: CONS_FLOW_UX_IDS,
      skipReceiptsFor: new Set(["SAFE-001"]),
    });
    const { receipt, exitCode } = verifyScope({
      scope: "cons-flow-ux",
      fixesPath: tree.fixesPath,
      progressPath: tree.progressPath,
      receiptsRoot: tree.receiptsRoot,
      current: tree.current,
    });

    expect(receipt.scopePassedTaskCount).toBe(27);
    expect(receipt.missingScopeTaskIds).toEqual(["SAFE-001"]);
    expect(receipt.disposition).toBe("BLOCKED_MISSING_PRIMITIVE");
    expect(exitCode).toBe(3);
  });


  test("protected hash drift fails verify-all with protected-hash-drift", () => {
    const tree = setup();
    const protectedFile = tree.writeFile("synthetic-protected.rs", "pub const VEIL: f32 = 0.0;\n");
    tree.writeFile("run.json", JSON.stringify({
      protectedPaths: [{ path: protectedFile, sha256: WRONG_SHA }],
    }));
    const all = tree.runAll();
    expect(errorCodes(all.receipt)).toContain("protected-hash-drift");
    expect(all.receipt.protectedHashesPass).toBe(false);
    expect(all.exitCode).not.toBe(0);
  });

  test("hand-edited generated output fails verify-all", () => {
    const tree = setup();
    tree.writeFile("GOV-005/generated-byte-compare.json", JSON.stringify({
      byteEqual: false,
      handEditedGeneratedOutput: true,
      outputHashes: {},
    }));
    const all = tree.runAll();
    expect(errorCodes(all.receipt)).toContain("hand-edited-generated-output");
    expect(all.receipt.generatedOutputsPass).toBe(false);
    expect(all.exitCode).not.toBe(0);
  });

  test("a deleted conflict lowering the count fails verify-all with conflict-count-drift", () => {
    const tree = setup();
    tree.writeFile("GOV-005/conflicts.json", JSON.stringify({
      observedConflictCount: AUTHORIZED_CONFLICT_COUNT - 1,
      classifiedConflictCount: AUTHORIZED_CONFLICT_COUNT - 1,
      duplicateIds: [],
      unownedHighConflicts: [],
      incompleteLifecycleRecords: [],
    }));
    const all = tree.runAll();
    expect(errorCodes(all.receipt)).toContain("conflict-count-drift");
    expect(all.receipt.conflictLifecyclePass).toBe(false);
    expect(all.exitCode).not.toBe(0);
  });

  test("an incomplete facade ledger fails verify-all", () => {
    const tree = setup();
    tree.writeFile("GOV-002/facade-ledger.json", JSON.stringify({
      assertions: {
        allFacadesValueFree: true,
        allProductionCallersMigrated: false,
        allTestCallersMigrated: true,
        zeroCallerFacadesRemoved: true,
        persistedNamesLiveAtCanonicalOwnersOnly: true,
      },
      disposition: "BLOCKED_SCOPE_DRIFT",
    }));
    const all = tree.runAll();
    expect(errorCodes(all.receipt)).toContain("incomplete-facade-lifecycle");
    expect(all.receipt.facadeLifecyclePass).toBe(false);
    expect(all.exitCode).not.toBe(0);
  });

  test("green legacy assertions cannot hide the unmigrated second facade", () => {
    const tree = setup();
    const complete = passingFacadeLedger();
    const scope = complete.facadeMigrations as Record<string, unknown>;
    const oneFacade = (scope.facades as unknown[]).slice(0, 1);
    tree.writeFile(
      "GOV-002/facade-ledger.json",
      JSON.stringify({
        ...complete,
        facadeMigrations: { ...scope, facades: oneFacade },
        facades: oneFacade,
      }),
    );
    const all = tree.runAll();
    const lifecycle = all.receipt.errors.find(
      (error: Record<string, unknown>) =>
        error.code === "incomplete-facade-lifecycle",
    );
    expect(lifecycle).toBeDefined();
    expect(lifecycle.detail.scopeFailures).toContain(
      "incomplete-required-facade-migration-set",
    );
    expect(all.receipt.facadeLifecyclePass).toBe(false);
  });

  test("facade source hashes are reconciled with current canonical production bytes", () => {
    const tree = setup();
    tree.writeFile(
      "GOV-002/facade-ledger.json",
      JSON.stringify(passingFacadeLedger()),
    );
    const all = tree.runAll();
    const lifecycle = all.receipt.errors.find(
      (error: Record<string, unknown>) =>
        error.code === "incomplete-facade-lifecycle",
    );
    expect(lifecycle.detail.scopeFailures).toContain(
      "facade-source-identity-drift:" + POPUP_WINDOW_OWNER,
    );
    expect(all.receipt.facadeLifecyclePass).toBe(false);
  });
});

describe("CLI contract", () => {
  test("parseArgs rejects unknown commands and missing arguments as usage errors", () => {
    expect(() => parseArgs(["bogus"])).toThrow(UsageError);
    expect(() => parseArgs([])).toThrow(UsageError);
    expect(() => parseArgs(["verify-task"])).toThrow(UsageError);
    expect(() => parseArgs(["verify-scope", "--scope", "cons-proof-gov"])).toThrow(UsageError);
    expect(() => parseArgs(["verify-all"])).toThrow(UsageError);
  });

  test("parseArgs accepts the documented command forms", () => {
    expect(parseArgs(["catalog"])).toEqual({
      kind: "catalog",
      fixesPath: "scripts/devtools/consistency-catalog.md",
    });
    expect(parseArgs(["catalog", "--fixes", "x.md"])).toEqual({ kind: "catalog", fixesPath: "x.md" });
    expect(parseArgs(["verify-task", "GOV-002", "--receipts", "r", "--out", "o"])).toEqual({
      kind: "verify-task",
      taskId: "GOV-002",
      fixesPath: "scripts/devtools/consistency-catalog.md",
      receiptsRoot: "r",
      outPath: "o",
    });
    expect(
      parseArgs([
        "verify-task",
        "--fixes",
        "reviewed-catalog.md",
        "GOV-002",
      ]),
    ).toEqual({
      kind: "verify-task",
      taskId: "GOV-002",
      fixesPath: "reviewed-catalog.md",
      receiptsRoot: ".artifacts/consistency",
      outPath: ".artifacts/consistency/GOV-002/task.json",
    });
    expect(parseArgs(["verify-family", "--family", "main-menu", "--receipts", "r"])).toEqual({
      kind: "verify-family",
      familyId: "main-menu",
      receiptsRoot: "r",
      outPath: undefined,
    });
  });

  test("the CLI exits 64 on a usage error before evaluation", () => {
    const result = Bun.spawnSync(["bun", "scripts/devtools/consistency.ts", "bogus-command"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    expect(result.exitCode).toBe(64);
  });

  test("verify-task succeeds in a clean checkout without the ignored fixes note", () => {
    const tree = setup();
    const cleanCheckout = dirname(tree.receiptsRoot);
    const portableCatalog = join(cleanCheckout, DEFAULT_CONSISTENCY_CATALOG_PATH);
    const trackedProgress = join(
      cleanCheckout,
      ".notes/CONSISTENCY-PROGRESS.md",
    );
    mkdirSync(dirname(portableCatalog), { recursive: true });
    mkdirSync(dirname(trackedProgress), { recursive: true });
    writeFileSync(
      portableCatalog,
      readFileSync(DEFAULT_CONSISTENCY_CATALOG_PATH, "utf8"),
    );
    writeFileSync(trackedProgress, readFileSync(tree.progressPath, "utf8"));
    const realCatalog = parseTaskCatalog(
      readFileSync(portableCatalog, "utf8"),
      portableCatalog,
    );
    const canonical = realCatalog.byId.get("GOV-002")!;
    tree.writeReceipt("GOV-002", "proof.json", {
      ...passingReceipt("GOV-002"),
      catalogBinding: {
        taskId: canonical.id,
        title: canonical.title,
        sectionSha256: canonical.sectionSha256,
      },
    });
    expect(
      existsSync(join(cleanCheckout, ".notes/CONSISTENCY-FIXES.md")),
    ).toBe(false);

    const outPath = join(tree.receiptsRoot, "GOV-002", "task.json");
    const result = Bun.spawnSync(
      [
        process.execPath,
        resolve(import.meta.dir, "consistency.ts"),
        "verify-task",
        "GOV-002",
        "--receipts",
        tree.receiptsRoot,
        "--out",
        outPath,
      ],
      { cwd: cleanCheckout, stdout: "pipe", stderr: "pipe" },
    );

    expect(result.exitCode).toBe(0);
    const receipt = JSON.parse(result.stdout.toString());
    expect(receipt.disposition).toBe("EVALUABLE_PASS");
    expect(receipt.taskId).toBe("GOV-002");
    expect(existsSync(outPath)).toBe(true);
  });
});
