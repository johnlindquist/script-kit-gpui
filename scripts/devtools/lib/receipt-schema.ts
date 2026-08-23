import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve } from "node:path";
import {
  assertNoCleartextCanaries,
  sanitizeReceipt,
  type JsonObject,
  type ReceiptPrivacyMode,
} from "./privacy.ts";
import { classifyReceiptEvidence } from "./evidence-class.ts";
import { taskProofPolicy } from "./task-proof-policy.ts";

export const RECEIPT_SCHEMA_VERSION = 2;
export const RECEIPT_REGISTRY_VERSION = 1;

export const receiptDispositions = [
  "EVALUABLE_PASS",
  "EVALUABLE_FAIL",
  "BLOCKED_MISSING_PRIMITIVE",
  "BLOCKED_TARGET_AMBIGUITY",
  "BLOCKED_STALE_GENERATION",
  "BLOCKED_PERMISSION",
  "BLOCKED_REAL_DATA_RISK",
  "BLOCKED_TIMEOUT",
  "BLOCKED_SCOPE_DRIFT",
  "BLOCKED_UNSUPPORTED_PROJECTION",
  "INVALID_SCHEMA",
  "INVALID_IDENTITY",
  "INVALID_GENERATION",
  "INVALID_PRIVACY",
  "INVALID_BINARY",
  "INVALID_FIXTURE",
  "INVALID_OBSERVER",
  "INVALID_INTERFERENCE",
  "INVALID_CLEANUP",
  "ANALYSIS_PENDING",
] as const;

export type ReceiptDisposition = (typeof receiptDispositions)[number];
export type EvidenceLayer =
  | "intended"
  | "model"
  | "rendered"
  | "accessibility"
  | "interaction";
export type PrivacyPolicy =
  | "metadata-only"
  | "redacted-content"
  | "fixture-cleartext-allowed";
export type IdentityPolicy =
  | "none"
  | "strict-target"
  | "same-transaction"
  | "declared-transition";

export interface ReceiptPredicate {
  id: string;
  validate: (receipt: JsonObject, disposition: ReceiptDisposition) => string[];
}

export interface ReceiptSchemaDefinition {
  primitiveId: string;
  version: number;
  tool: string;
  commands: string[];
  requiredPaths: string[];
  nonNullPaths: string[];
  allowedDispositions: ReceiptDisposition[];
  requiredEvidenceLayers: EvidenceLayer[];
  privacyPolicy: PrivacyPolicy;
  identityPolicy: IdentityPolicy;
  forbidMissingPrimitivesOnPass?: boolean;
  activationProof?: boolean;
  predicates: ReceiptPredicate[];
  description: string;
}

const commonTargetFields = ["requestedTarget", "target"];
const allDispositions = [...receiptDispositions];

function schema(
  definition: Omit<ReceiptSchemaDefinition,
    | "version"
    | "nonNullPaths"
    | "allowedDispositions"
    | "requiredEvidenceLayers"
    | "privacyPolicy"
    | "identityPolicy"
    | "predicates"
  > & Partial<Pick<ReceiptSchemaDefinition,
    | "version"
    | "nonNullPaths"
    | "allowedDispositions"
    | "requiredEvidenceLayers"
    | "privacyPolicy"
    | "identityPolicy"
    | "predicates"
  >>,
): ReceiptSchemaDefinition {
  return {
    version: 1,
    nonNullPaths: definition.requiredPaths,
    allowedDispositions: allDispositions,
    requiredEvidenceLayers: [],
    privacyPolicy: "redacted-content",
    identityPolicy: "strict-target",
    predicates: [],
    ...definition,
  };
}

export const receiptSchemaRegistry: ReceiptSchemaDefinition[] = [
  schema({
    primitiveId: "devtools.targets.list",
    tool: "script-kit-devtools.targets",
    commands: ["targets.list"],
    requiredPaths: ["targetCount", "targets"],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    description: "Enumerate registered automation targets.",
  }),
  schema({
    primitiveId: "devtools.targets.inspect",
    tool: "script-kit-devtools.targets",
    commands: ["targets.inspect"],
    requiredPaths: ["requestedTarget", "resolvedTarget.automationId", "resolvedTarget.bounds"],
    description: "Resolve one strict automation target identity.",
  }),
  schema({
    primitiveId: "devtools.surface.inspect",
    tool: "script-kit-devtools.surface",
    commands: ["surface.inspect"],
    requiredPaths: [...commonTargetFields, "contract.surfaceKind", "runtime.capabilities", "runtime.missingPrimitives"],
    forbidMissingPrimitivesOnPass: true,
    description: "Join a source contract to the live target surface.",
  }),
  schema({
    primitiveId: "devtools.elements.snapshot",
    tool: "script-kit-devtools.elements",
    commands: ["elements.snapshot"],
    requiredPaths: [
      ...commonTargetFields,
      "semanticSurface",
      "semanticProjection.semanticSurface",
      "semanticProjection.version",
      "semanticProjection.quality",
      "semanticProjection.reasonCodes",
      "semanticProjection.proofMode",
      "semanticProjection.proofAllowed",
      "nodes[].semanticId",
      "duplicateSemanticIds",
    ],
    forbidMissingPrimitivesOnPass: true,
    predicates: [{
      id: "semantic-projection-quality",
      validate(receipt, disposition) {
        const projection = receipt.semanticProjection && typeof receipt.semanticProjection === "object"
          ? receipt.semanticProjection as JsonObject
          : {};
        if (Object.keys(projection).length === 0 && disposition !== "EVALUABLE_PASS") {
          return [];
        }
        const quality = projection.quality;
        const reasons = Array.isArray(projection.reasonCodes) ? projection.reasonCodes : [];
        const errors: string[] = [];
        if (quality !== "complete" && reasons.length === 0) {
          errors.push("incomplete semantic projection requires typed reason codes");
        }
        if (disposition === "EVALUABLE_PASS" && quality !== "complete") {
          errors.push("semantic action/focus proof requires a complete projection");
        }
        if (disposition === "EVALUABLE_PASS" && projection.proofAllowed !== true) {
          errors.push("pass receipt must explicitly allow semantic proof");
        }
        if (disposition === "EVALUABLE_PASS") {
          const nodes = Array.isArray(receipt.nodes)
            ? receipt.nodes.map((node) => asObject(node))
            : [];
          if (
            projection.proofMode === "ax" &&
            projection.nativeAccessibilityObserved !== true
          ) {
            errors.push("AX projection proof requires independently observed native accessibility peers");
          }
          if (
            projection.proofMode === "action" &&
            !nodes.some((node) => node.activatable === true)
          ) {
            errors.push("action projection proof requires an enabled activatable semantic node");
          }
          if (
            projection.proofMode === "focus" &&
            nodes.filter((node) => node.focused === true).length !== 1
          ) {
            errors.push("focus projection proof requires exactly one focused semantic node");
          }
          if (
            Array.isArray(receipt.privacyViolationSemanticIds) &&
            receipt.privacyViolationSemanticIds.length > 0
          ) {
            errors.push("semantic receipt contains invalid or cleartext production privacy descriptors");
          }
        }
        return errors;
      },
    }],
    description: "Capture privacy-safe semantic element measurements with explicit projection quality.",
  }),
  schema({
    primitiveId: "devtools.layout.measure",
    tool: "script-kit-devtools.layout",
    commands: ["layout.measure"],
    requiredPaths: [
      ...commonTargetFields,
      "target.bounds",
      "proofMode",
      "window",
      "regions",
      "resizePressure",
      "pressure",
      "truthLayers.model",
      "truthLayers.rendered",
      "truthLayers.joins",
    ],
    forbidMissingPrimitivesOnPass: true,
    predicates: [{
      id: "join-proof-keeps-model-and-rendered-truth-independent",
      validate(receipt, disposition) {
        if (receipt.proofMode !== "join" || disposition !== "EVALUABLE_PASS") return [];
        const truth = receipt.truthLayers && typeof receipt.truthLayers === "object"
          ? receipt.truthLayers as JsonObject
          : {};
        const model = truth.model && typeof truth.model === "object"
          ? truth.model as JsonObject
          : {};
        const rendered = truth.rendered && typeof truth.rendered === "object"
          ? truth.rendered as JsonObject
          : {};
        const joins = Array.isArray(truth.joins) ? truth.joins : [];
        const comparableJoinCount = Number(truth.comparableJoinCount ?? 0);
        const joinedGeometryAgrees = joins.every((value) => {
          const join = value && typeof value === "object" ? value as JsonObject : {};
          return join.comparability !== "Comparable" || join.classification === "Match";
        });
        return comparableJoinCount > 0 &&
            Number(model.clippedNodeCount ?? 0) === 0 &&
            Number(model.overlapCount ?? 0) === 0 &&
            Number(rendered.clippedNodeCount ?? 0) === 0 &&
            Number(rendered.overlapCount ?? 0) === 0 &&
            joinedGeometryAgrees
          ? []
          : ["join proof requires matching same-frame joins plus unclipped non-overlapping model and rendered evidence"];
      },
    }],
    description: "Measure target-scoped intended/model/rendered geometry without collapsing truth layers.",
  }),
  schema({
    primitiveId: "devtools.scroll.inspect",
    tool: "script-kit-devtools.scroll",
    commands: ["scroll.inspect"],
    requiredPaths: [...commonTargetFields, "scroll", "resizePressure"],
    forbidMissingPrimitivesOnPass: true,
    predicates: [{
      id: "rendered-safe-viewport-requires-same-frame-full-visibility",
      validate(receipt, disposition) {
        const rendered = receipt.renderedSafeViewport && typeof receipt.renderedSafeViewport === "object"
          ? receipt.renderedSafeViewport as JsonObject
          : {};
        if (rendered.required !== true || disposition !== "EVALUABLE_PASS") return [];
        return rendered.classification === "ok" &&
            Number(rendered.visibleRatio) >= 0.999 &&
            rendered.withinSafeViewport === true &&
            rendered.frameMatches === true &&
            typeof rendered.targetDataGeneration === "number"
          ? []
          : ["rendered safe-viewport proof requires a same-frame fully visible selected row and target data generation"];
      },
    }],
    description: "Measure selected-row and safe-viewport scroll state with optional completed-frame proof.",
  }),
  schema({
    primitiveId: "devtools.focus.inspect",
    tool: "script-kit-devtools.focus",
    commands: ["focus.inspect"],
    requiredPaths: [
      ...commonTargetFields,
      "windowFocused",
      "focusedSemanticId",
      "keyboardOwner",
      "semanticProjection.quality",
      "semanticProjection.proofAllowed",
    ],
    forbidMissingPrimitivesOnPass: true,
    predicates: [{
      id: "focus-requires-complete-projection",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const projection = receipt.semanticProjection && typeof receipt.semanticProjection === "object"
          ? receipt.semanticProjection as JsonObject
          : {};
        return projection.quality === "complete" && projection.proofAllowed === true
          ? []
          : ["focus proof requires a complete semantic projection"];
      },
    }, {
      id: "ax-proof-requires-reciprocal-semantic-peers",
      validate(receipt, disposition) {
        if (receipt.proofMode !== "ax" || disposition !== "EVALUABLE_PASS") return [];
        const footer = receipt.nativeFooter && typeof receipt.nativeFooter === "object"
          ? receipt.nativeFooter as JsonObject
          : {};
        const parity = footer.axParity && typeof footer.axParity === "object"
          ? footer.axParity as JsonObject
          : {};
        const graph = receipt.focusGraph && typeof receipt.focusGraph === "object"
          ? receipt.focusGraph as JsonObject
          : {};
        return parity.complete === true && graph.reciprocal === true
          ? []
          : ["AX proof requires complete semantic peers and a reciprocal focus graph"];
      },
    }],
    description: "Inspect focus ownership and optional independent semantic-to-AX parity without claiming activation.",
  }),
  schema({
    primitiveId: "devtools.text.measure",
    tool: "script-kit-devtools.text",
    commands: ["text.measure"],
    requiredPaths: [
      ...commonTargetFields,
      "proofMode",
      "textSummary.inputLength",
      "textSummary.inputFingerprint",
      "rows[].textLength",
      "rows[].fingerprint",
    ],
    forbidMissingPrimitivesOnPass: true,
    predicates: [{
      id: "fit-proof-requires-same-frame-unoccluded-glyphs",
      validate(receipt, disposition) {
        if (receipt.proofMode !== "fit" || disposition !== "EVALUABLE_PASS") return [];
        const fits = Array.isArray(receipt.textFits) ? receipt.textFits : [];
        return fits.length > 0 && fits.every((fit) => {
          const measurement = fit && typeof fit === "object" ? fit as JsonObject : {};
          return measurement.fullDisplayPass === true &&
            measurement.rawContentReturned !== true &&
            measurement.frameMatches === true &&
            measurement.backingScaleMatches === true;
        })
          ? []
          : ["fit proof requires same-frame, font-ready, unoccluded full glyph display without raw content"];
      },
    }],
    description: "Measure redacted text metadata and optional completed-frame glyph fit.",
  }),
  schema({
    primitiveId: "devtools.keyboard.inspect",
    tool: "script-kit-devtools.keyboard",
    commands: ["keyboard.inspect"],
    requiredPaths: [...commonTargetFields, "keyboardPolicy", "inputOwnership", "bindings", "duplicateKeys"],
    forbidMissingPrimitivesOnPass: true,
    activationProof: false,
    description: "Inspect keyboard bindings; this primitive does not prove activation.",
  }),
  schema({
    primitiveId: "devtools.actions.inspect",
    tool: "script-kit-devtools.actions",
    commands: ["actions.inspect"],
    requiredPaths: ["target", "actions", "missingPrimitives"],
    forbidMissingPrimitivesOnPass: true,
    description: "Inspect Actions popup rows, shortcuts, and geometry.",
  }),
  schema({
    primitiveId: "devtools.act",
    tool: "script-kit-devtools.act",
    commands: ["act.set-input", "act.select", "act.key", "act.open-actions", "act.set-theme-control"],
    requiredPaths: ["actionId", "targetBefore", "result"],
    activationProof: true,
    identityPolicy: "declared-transition",
    description: "Perform one guarded user-like activation with pre/post evidence.",
  }),
  schema({
    primitiveId: "devtools.compare.redgreen",
    tool: "script-kit-devtools.compare",
    commands: ["compare.redgreen"],
    requiredPaths: ["assertions", "classification"],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    description: "Compare comparable red and green proof receipts.",
  }),
  schema({
    primitiveId: "devtools.notes.inspect",
    tool: "script-kit-devtools.notes",
    commands: ["notes.inspect"],
    requiredPaths: ["target", "notesState", "runtimeState", "coverage", "receipts"],
    forbidMissingPrimitivesOnPass: true,
    description: "Inspect redacted Notes runtime state and primitive receipts.",
  }),
  schema({
    primitiveId: "devtools.notes.resizeCompare",
    tool: "script-kit-devtools.notes",
    commands: ["notes.resize-compare"],
    requiredPaths: ["safety.mutationMode", "resizeCompare", "assertions", "cleanup"],
    forbidMissingPrimitivesOnPass: true,
    identityPolicy: "declared-transition",
    privacyPolicy: "fixture-cleartext-allowed",
    description: "Compare sandboxed Notes autosize generations.",
  }),
  schema({
    primitiveId: "devtools.dictation.inspect",
    tool: "script-kit-devtools.dictation",
    commands: ["dictation.inspect"],
    requiredPaths: ["passiveSafety", "coverage", "runtimeState", "sourceEvidence", "missingPrimitives"],
    forbidMissingPrimitivesOnPass: true,
    identityPolicy: "none",
    description: "Passively inspect redacted Dictation readiness and delivery facts.",
  }),
  schema({
    primitiveId: "devtools.dictation.deliverFixture",
    tool: "script-kit-devtools.dictation",
    commands: ["dictation.deliverFixture"],
    requiredPaths: ["safety", "target", "delivery", "missingPrimitives"],
    forbidMissingPrimitivesOnPass: true,
    activationProof: true,
    identityPolicy: "declared-transition",
    privacyPolicy: "fixture-cleartext-allowed",
    description: "Inject and verify one synthetic transcript delivery.",
  }),
  schema({
    primitiveId: "devtools.inspect.orchestrate",
    tool: "script-kit-devtools.inspect",
    commands: ["inspect.orchestrate"],
    requiredPaths: ["requestedTarget", "resolvedTarget", "primitiveStack", "missingPrimitives", "cleanup"],
    forbidMissingPrimitivesOnPass: true,
    description: "Orchestrate a fail-closed target-scoped investigation stack.",
  }),
  schema({
    primitiveId: "devtools.coverage.bindings",
    tool: "script-kit-devtools.surfaces",
    commands: ["surfaces.coverage-bindings"],
    requiredPaths: [
      "evidenceClass",
      "catalogBinding.taskId",
      "catalogBinding.title",
      "catalogBinding.sectionSha256",
      "census.expected.contractKindCount",
      "census.expected.contractMappingCount",
      "census.actual.contractKindCount",
      "census.actual.contractMappingCount",
      "sourceParity.pass",
      "featureMapGate.pass",
      "profileRegistry.validationErrorCount",
      "bindingSetUsable",
      "bindings",
      "aliases",
      "summary.staticDirectBindingCount",
      "summary.freshDirectRuntimeProofCount",
      "summary.runtimeProofDisposition",
      "negativeControls",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "binding-inventory-is-complete-fail-closed-and-never-runtime-proof",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        const census = asObject(receipt.census);
        const expected = asObject(census.expected);
        const actual = asObject(census.actual);
        const sourceParity = asObject(receipt.sourceParity);
        const featureMapGate = asObject(receipt.featureMapGate);
        const profileRegistry = asObject(receipt.profileRegistry);
        const summary = asObject(receipt.summary);
        const bindings = Array.isArray(receipt.bindings) ? receipt.bindings : [];
        const negativeControls = Array.isArray(receipt.negativeControls)
          ? receipt.negativeControls
          : [];

        if (receipt.evidenceClass !== "STATIC_INVENTORY") {
          errors.push("coverage bindings are static inventory, never direct runtime proof");
        }
        const catalogBinding = asObject(receipt.catalogBinding);
        if (
          catalogBinding.taskId !== "PF-009" ||
          typeof catalogBinding.title !== "string" ||
          !/^[a-f0-9]{64}$/.test(String(catalogBinding.sectionSha256 ?? ""))
        ) {
          errors.push("coverage bindings must bind to the canonical PF-009 catalog section");
        }
        if (
          expected.contractKindCount !== 37 ||
          expected.contractMappingCount !== 54 ||
          actual.contractKindCount !== 37 ||
          actual.contractMappingCount !== 54
        ) {
          errors.push("coverage bindings require the exact 37-kind / 54-mapping census");
        }
        if (sourceParity.pass !== true) {
          errors.push("coverage bindings require exact source-contract parity");
        }
        if (featureMapGate.pass !== true) {
          errors.push("coverage bindings require a valid real feature map");
        }
        if (profileRegistry.validationErrorCount !== 0) {
          errors.push("coverage bindings require valid repository-contained source owners");
        }
        if (receipt.bindingSetUsable !== true || bindings.length !== 54) {
          errors.push("coverage bindings require 54 usable canonical mappings");
        }
        if (
          summary.freshDirectRuntimeProofCount !== 0 ||
          summary.runtimeProofDisposition !== "NOT_EVALUATED"
        ) {
          errors.push("static coverage bindings cannot claim fresh direct runtime proof");
        }
        if (
          negativeControls.length === 0 ||
          negativeControls.some((control) => asObject(control).pass !== true)
        ) {
          errors.push("coverage bindings require passing deterministic negative controls");
        }
        return errors;
      },
    }],
    description:
      "Validate the complete static 37-kind / 54-mapping surface binding inventory without claiming runtime proof.",
  }),
  schema({
    primitiveId: "devtools.consistency.safe-task-proof",
    tool: "script-kit-devtools.safe-task-proofs",
    commands: ["safe-task-proofs.verify"],
    requiredPaths: [
      "taskId",
      "taskIds",
      "evidenceClass",
      "provesRuntimeBehavior",
      "catalogBinding.taskId",
      "catalogBinding.title",
      "catalogBinding.sectionSha256",
      "testCommand",
      "testRun.pass",
      "testRun.exitCode",
      "testRun.passedTestCount",
      "testRun.failedTestCount",
      "testRun.expectationCount",
      "testRun.suiteFiles",
      "testRun.executedSuiteFiles",
      "testRun.outputSha256",
      "sourceFingerprints",
      "assertions",
      "negativeControls",
      "safety.noninteractive",
      "safety.startsApplication",
      "safety.revealsWindow",
      "safety.focusesWindow",
      "safety.drivesNativeInput",
      "safety.capturesScreen",
      "safety.accessesNetwork",
      "safety.usesLiveAi",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "offline-task-proof-must-run-fresh-owned-behavior-tests",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        const taskId = typeof receipt.taskId === "string" ? receipt.taskId : "";
        const policy = taskProofPolicy(taskId);
        const evidenceClass = String(receipt.evidenceClass ?? "");
        if (
          !policy ||
          policy.provesRuntimeBehavior ||
          !policy.acceptedEvidenceClasses.includes(evidenceClass as never) ||
          (evidenceClass !== "STATIC_INVENTORY" && evidenceClass !== "UNIT_BEHAVIOR") ||
          receipt.provesRuntimeBehavior !== false
        ) {
          errors.push("offline task proof cannot discharge a runtime interaction obligation");
        }
        const binding = asObject(receipt.catalogBinding);
        const taskIds = stringArray(receipt.taskIds);
        if (
          binding.taskId !== taskId ||
          !taskIds.includes(taskId) ||
          typeof binding.title !== "string" ||
          !/^[a-f0-9]{64}$/.test(String(binding.sectionSha256 ?? ""))
        ) {
          errors.push("offline task proof requires one exact canonical catalog section binding");
        }
        const testRun = asObject(receipt.testRun);
        const suiteFiles = stringArray(testRun.suiteFiles);
        const executedSuiteFiles = stringArray(testRun.executedSuiteFiles);
        const testCommand = stringArray(receipt.testCommand);
        if (
          testRun.pass !== true ||
          testRun.exitCode !== 0 ||
          Number(testRun.passedTestCount) < 1 ||
          testRun.failedTestCount !== 0 ||
          Number(testRun.expectationCount) < 1 ||
          suiteFiles.length === 0 ||
          executedSuiteFiles.length === 0 ||
          new Set(executedSuiteFiles).size !== executedSuiteFiles.length ||
          suiteFiles.some((path) => !executedSuiteFiles.includes(path)) ||
          testCommand[0] !== "bun" ||
          testCommand[1] !== "test" ||
          testCommand.length !== executedSuiteFiles.length + 2 ||
          executedSuiteFiles.some((path, index) => testCommand[index + 2] !== `./${path}`) ||
          !/^[a-f0-9]{64}$/.test(String(testRun.outputSha256 ?? ""))
        ) {
          errors.push("offline task proof requires executed passing nonempty behavior tests");
        }
        const hashes = asObject(receipt.sourceFingerprints);
        const productionSources = stringArray(receipt.productionSources);
        const reviewedWorkflowSuite =
          "scripts/agentic/cons-flow-ux/final-workflow-audit.test.ts";
        const reviewedWorkflowOwner =
          "scripts/agentic/cons-flow-ux/final-workflow-audit.ts";
        if (
          executedSuiteFiles.some((path) =>
            !(path.startsWith("scripts/devtools/") || path === reviewedWorkflowSuite) ||
            !path.endsWith(".test.ts") ||
            path.split("/").includes("..") ||
            !/^[a-f0-9]{64}$/.test(String(hashes[path] ?? "")) ||
            fileFingerprint(resolve(process.cwd(), path)) !== hashes[path]
          )
        ) {
          errors.push("offline task proof requires exact current fingerprinted reviewed behavior suites");
        }
        if (
          productionSources.some((path) =>
            !(
              path.startsWith("src/") ||
              path.startsWith("scripts/devtools/") ||
              path.startsWith("crates/sk-protocol/src/") ||
              path.startsWith("design/mockups/generated/") ||
              (taskId === "GOV-006" && path === reviewedWorkflowOwner)
            ) ||
            path.split("/").includes("..") ||
            !/^[a-f0-9]{64}$/.test(String(hashes[path] ?? "")) ||
            fileFingerprint(resolve(process.cwd(), path)) !== hashes[path]
          ) ||
          (taskId === "GOV-002" &&
            (
              !productionSources.includes(
                "scripts/devtools/facade-ledger.ts",
              ) ||
              !productionSources.includes(
                "scripts/devtools/facade-migrations.ts",
              ) ||
              !productionSources.includes("src/components/conversation_style.rs") ||
              !productionSources.includes("src/components/inline_popup_window.rs") ||
              !productionSources.includes(
                "src/ai/agent_chat/ui/popup_automation.rs",
              ) ||
              !suiteFiles.includes("scripts/devtools/facade-ledger.test.ts") ||
              !suiteFiles.includes(
                "scripts/devtools/facade-migrations.test.ts",
              )
            )) ||
          (taskId === "GOV-003" && !productionSources.includes("src/theme/alpha.rs")) ||
          (taskId === "GOV-005" &&
            !productionSources.includes("design/mockups/generated/tokens.json")) ||
          (taskId === "GOV-006" && (
            !productionSources.includes("scripts/devtools/consistency.ts") ||
            !productionSources.includes(reviewedWorkflowOwner) ||
            !suiteFiles.includes("scripts/devtools/consistency.test.ts") ||
            !suiteFiles.includes(reviewedWorkflowSuite)
          ))
        ) {
          errors.push("offline governance proof requires fingerprints for its actual production owner");
        }
        const safety = asObject(receipt.safety);
        if (safety.noninteractive !== true) {
          errors.push("offline task proof must enforce noninteractive execution");
        }
        for (const field of [
          "startsApplication",
          "revealsWindow",
          "focusesWindow",
          "drivesNativeInput",
          "capturesScreen",
          "accessesNetwork",
          "usesLiveAi",
        ]) {
          if (safety[field] !== false) errors.push(`offline task proof must prohibit ${field}`);
        }
        const negatives = Array.isArray(receipt.negativeControls)
          ? receipt.negativeControls
          : [];
        if (
          negatives.length === 0 ||
          negatives.some((negative) => asObject(negative).pass !== true)
        ) {
          errors.push("offline task proof requires passing deterministic negative controls");
        }
        return errors;
      },
    }],
    description:
      "Fresh, canonical-section-bound offline behavior proof for explicitly safe infrastructure tasks only.",
  }),
  schema({
    primitiveId: "mockups.story.browserGeometry",
    tool: "script-kit-mockups.story-browser-geometry",
    commands: ["story.browser-geometry"],
    requiredPaths: [
      "taskId",
      "taskIds",
      "catalogBinding.taskId",
      "catalogBinding.title",
      "catalogBinding.sectionSha256",
      "evidenceClass",
      "provesRuntimeBehavior",
      "evidenceBoundary",
      "repository.gitCommit",
      "browser.dependency",
      "browser.headed",
      "browser.observedVisible",
      "browser.sessionId",
      "target.visible",
      "viewport.width",
      "viewport.height",
      "viewport.devicePixelRatio",
      "stories",
      "fixture.path",
      "fixture.sha256",
      "fixtures",
      "assets",
      "assetFingerprint",
      "evidence.rendered.boundary",
      "evidence.rendered.toleranceCssPx",
      "negativeControls",
      "cleanup.closed",
      "cleanup.browserClosed",
      "cleanup.serverClosed",
      "cleanup.survivors",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    requiredEvidenceLayers: ["rendered"],
    forbidMissingPrimitivesOnPass: true,
    predicates: [{
      id: "headed-browser-geometry-must-observe-both-source-current-stories",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        const browser = asObject(receipt.browser);
        const viewport = asObject(receipt.viewport);
        const repository = asObject(receipt.repository);
        const binding = asObject(receipt.catalogBinding);
        const expectedStories = [
          "10-conversation-three-modes",
          "11-launcher-flows-and-scripts",
        ];
        const dependencies = ["playwright", "@playwright/test", "puppeteer", "puppeteer-core"];
        if (
          receipt.taskId !== "PF-012" ||
          binding.taskId !== "PF-012" ||
          !stringArray(receipt.taskIds).includes("PF-012") ||
          !/^[a-f0-9]{64}$/.test(String(binding.sectionSha256 ?? "")) ||
          receipt.evidenceClass !== "RUNTIME_VISIBLE" ||
          receipt.provesRuntimeBehavior !== true ||
          receipt.evidenceBoundary !== "HTML_BROWSER_ONLY"
        ) {
          errors.push("browser geometry must be canonical PF-012 visible HTML-only runtime proof");
        }
        if (
          repository.gitCommit !== gitCommit() ||
          !/^[a-f0-9]{40}$/.test(String(repository.gitCommit ?? ""))
        ) {
          errors.push("browser geometry requires the exact current source commit");
        }
        if (
          !dependencies.includes(String(browser.dependency ?? "")) ||
          browser.headed !== true ||
          browser.observedVisible !== true ||
          asObject(receipt.target).visible !== true ||
          typeof browser.sessionId !== "string" ||
          browser.sessionId.length < 8
        ) {
          errors.push("browser geometry requires an actually observed approved headed browser");
        }
        if (viewport.width !== 1280 || viewport.height !== 720 || viewport.devicePixelRatio !== 1) {
          errors.push("browser geometry requires the exact 1280x720 CSS viewport at DPR 1");
        }

        const stories = Array.isArray(receipt.stories) ? receipt.stories.map(asObject) : [];
        const observedStories = stories.map((story) => String(story.id ?? "")).sort();
        if (
          observedStories.length !== expectedStories.length ||
          observedStories.some((story, index) => story !== [...expectedStories].sort()[index]) ||
          stories.some((story) => {
            const observation = asObject(story.observation);
            const observedBrowser = asObject(observation.browser);
            const fonts = asObject(observation.fonts);
            const autoplay = asObject(observation.autoplay);
            const comparisons = Array.isArray(story.comparisons)
              ? story.comparisons.map(asObject)
              : [];
            return story.pass !== true ||
              observation.sourceCommit !== repository.gitCommit ||
              asObject(observation.story).id !== story.id ||
              observedBrowser.sessionId !== browser.sessionId ||
              observedBrowser.observedVisible !== true ||
              fonts.awaited !== true ||
              fonts.pass !== true ||
              fonts.topLevelStatus !== "loaded" ||
              fonts.surfaceStatus !== "loaded" ||
              autoplay.overrideInstalled !== true ||
              autoplay.remainedStopped !== true ||
              autoplay.clockBeforeFramesMs !== 0 ||
              autoplay.clockAfterFramesMs !== 0 ||
              comparisons.length !== 2 ||
              comparisons.some((comparison) =>
                comparison.pass !== true || comparison.toleranceCssPx !== 0
              );
          })
        ) {
          errors.push("browser geometry requires both exact settled, font-ready, zero-tolerance stories");
        }

        const fixture = asObject(receipt.fixture);
        const fixtures = Array.isArray(receipt.fixtures) ? receipt.fixtures.map(asObject) : [];
        const requiredFixturePaths = expectedStories.map((story) =>
          `design/mockups/stories/${story}/story.js`
        ).sort();
        if (
          fixture.path !== "design/mockups/stories/stories.json" ||
          fileFingerprint(resolve(process.cwd(), String(fixture.path ?? ""))) !== fixture.sha256 ||
          fixtures.length !== requiredFixturePaths.length ||
          fixtures.map((entry) => String(entry.path ?? "")).sort()
            .some((path, index) => path !== requiredFixturePaths[index]) ||
          fixtures.some((entry) =>
            fileFingerprint(resolve(process.cwd(), String(entry.path ?? ""))) !== entry.sha256
          )
        ) {
          errors.push("browser geometry requires both exact current story fixture fingerprints");
        }

        const assets = Array.isArray(receipt.assets) ? receipt.assets.map(asObject) : [];
        const assetBytes = assets
          .map((asset) => `${asset.path}\n${asset.sha256}\n${asset.sizeBytes}`)
          .join("\n");
        if (
          assets.length === 0 ||
          assets.some((asset) =>
            typeof asset.path !== "string" ||
            asset.path.split("/").includes("..") ||
            !/^[a-f0-9]{64}$/.test(String(asset.sha256 ?? "")) ||
            typeof asset.sizeBytes !== "number" ||
            asset.sizeBytes < 1
          ) ||
          sha256(assetBytes) !== receipt.assetFingerprint
        ) {
          errors.push("browser geometry requires fingerprinted assets actually served by the loopback observer");
        }

        const controls = Array.isArray(receipt.negativeControls)
          ? receipt.negativeControls.map(asObject)
          : [];
        const expectedControls = expectedStories.flatMap((story) => [
          "one-pixel-offset",
          "missing-selector",
          "wrong-chapter",
          "unresolved-fonts",
          "stale-source",
          "invalid-tolerance",
        ].map((control) => `${story}:${control}`)).sort();
        if (
          controls.length !== expectedControls.length ||
          controls.map((control) => String(control.id ?? "")).sort()
            .some((id, index) => id !== expectedControls[index]) ||
          controls.some((control) => control.pass !== true)
        ) {
          errors.push("browser geometry requires all executed adversarial controls for both stories");
        }

        const cleanup = asObject(receipt.cleanup);
        if (
          cleanup.closed !== true ||
          cleanup.browserClosed !== true ||
          cleanup.serverClosed !== true ||
          !Array.isArray(cleanup.survivors) ||
          cleanup.survivors.length > 0
        ) {
          errors.push("browser geometry requires closed browser/server ownership with no survivors");
        }
        return errors;
      },
    }],
    description:
      "Prove both source-bound mockup stories in one observed headed browser without treating synthetic rectangles as runtime evidence.",
  }),
  schema({
    primitiveId: "devtools.consistency.family-fixtures",
    tool: "script-kit-devtools.family-fixtures",
    commands: ["family-fixtures.verify"],
    requiredPaths: [
      "evidenceClass",
      "provesRuntimeBehavior",
      "catalogBinding.taskId",
      "catalogBinding.title",
      "catalogBinding.sectionSha256",
      "expectedFamilyCount",
      "auditedFamilyCount",
      "expectedCanonicalBindingCount",
      "auditedCanonicalBindingCount",
      "expectedAliasBindingCount",
      "auditedAliasBindingCount",
      "verifiedRuntimeProofCount",
      "safety.startsApplication",
      "safety.revealsWindow",
      "safety.focusesWindow",
      "safety.drivesNativeInput",
      "safety.capturesScreen",
      "safety.accessesNetwork",
      "safety.usesLiveAi",
      "families",
      "negativeControls",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "fixture-families-are-exhaustive-safe-and-never-runtime-proof",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        const families = Array.isArray(receipt.families) ? receipt.families : [];
        const negatives = Array.isArray(receipt.negativeControls)
          ? receipt.negativeControls
          : [];
        if (
          receipt.evidenceClass !== "FIXTURE_CONTRACT" ||
          receipt.provesRuntimeBehavior !== false ||
          receipt.verifiedRuntimeProofCount !== 0
        ) {
          errors.push("fixture contracts cannot claim direct runtime behavior proof");
        }
        const catalogBinding = asObject(receipt.catalogBinding);
        if (
          catalogBinding.taskId !== "PF-010" ||
          typeof catalogBinding.title !== "string" ||
          !/^[a-f0-9]{64}$/.test(String(catalogBinding.sectionSha256 ?? ""))
        ) {
          errors.push("fixture contracts must bind to the canonical PF-010 catalog section");
        }
        if (
          receipt.expectedFamilyCount !== 9 ||
          receipt.auditedFamilyCount !== 9 ||
          families.length !== 9 ||
          families.some((family) => asObject(family).pass !== true)
        ) {
          errors.push("fixture contracts require all nine validated surface families");
        }
        if (
          receipt.expectedCanonicalBindingCount !== 54 ||
          receipt.auditedCanonicalBindingCount !== 54 ||
          receipt.expectedAliasBindingCount !== 5 ||
          receipt.auditedAliasBindingCount !== 5
        ) {
          errors.push("fixture contracts require all 54 canonical and five alias bindings");
        }
        const safety = asObject(receipt.safety);
        for (const field of [
          "startsApplication",
          "revealsWindow",
          "focusesWindow",
          "drivesNativeInput",
          "capturesScreen",
          "accessesNetwork",
          "usesLiveAi",
        ]) {
          if (safety[field] !== false) {
            errors.push(`fixture contracts must prohibit ${field}`);
          }
        }
        if (
          negatives.length === 0 ||
          negatives.some((negative) => asObject(negative).pass !== true)
        ) {
          errors.push("fixture contracts require passing deterministic negative controls");
        }
        return errors;
      },
    }],
    description:
      "Verify all nine deterministic family fixtures against canonical bindings without runtime interaction.",
  }),
  // ── GOV-006 consistency completion auditor (metadata-only; identity none) ──
  schema({
    primitiveId: "devtools.consistency.catalog",
    tool: "script-kit-devtools.consistency",
    commands: ["consistency.catalog"],
    requiredPaths: [
      "evidenceClass",
      "provesRuntimeBehavior",
      "catalogPath",
      "catalogTaskCount",
      "expectedProgramTaskCount",
      "expectedScopeTaskCount",
      "missingTaskIds",
      "unknownTaskIds",
      "duplicateTaskIds",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "catalog-pass-requires-exact-id-sets",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        if (receipt.evidenceClass !== "STATIC_INVENTORY") {
          errors.push("catalog evidence must be classified as static inventory");
        }
        if (receipt.provesRuntimeBehavior !== false) {
          errors.push("catalog inventory must never claim runtime behavior proof");
        }
        if (receipt.catalogTaskCount !== 75) errors.push("catalog pass requires exactly 75 tasks");
        if (receipt.expectedProgramTaskCount !== 75) errors.push("catalog pass requires the 75-ID program set");
        if (receipt.expectedScopeTaskCount !== 28) errors.push("catalog pass requires the 28-ID primary scope set");
        for (const field of ["missingTaskIds", "unknownTaskIds", "duplicateTaskIds"]) {
          const value = receipt[field];
          if (!Array.isArray(value) || value.length > 0) errors.push(`catalog pass requires empty ${field}`);
        }
        return errors;
      },
    }],
    description: "Parse and validate the exact 75/28 consistency task catalog.",
  }),
  schema({
    primitiveId: "devtools.consistency.verify-task",
    tool: "script-kit-devtools.consistency",
    commands: ["consistency.verify-task"],
    requiredPaths: [
      "taskId",
      "scope",
      "positiveReceiptPaths",
      "negativeControlSummary",
      "privacyStatus",
      "interferenceStatus",
      "cleanupStatus",
      "identities",
      "staleReasons",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "task-pass-requires-fresh-clean-evidence",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        const stale = Array.isArray(receipt.staleReasons) ? receipt.staleReasons : [];
        if (stale.length > 0) errors.push("task pass requires zero staleness-by-identity reasons");
        const positives = Array.isArray(receipt.positiveReceiptPaths) ? receipt.positiveReceiptPaths : [];
        if (positives.length === 0) errors.push("task pass requires at least one positive receipt");
        const negatives = asObject(receipt.negativeControlSummary);
        if (Number(negatives.failedCount ?? 0) !== 0 || Number(negatives.totalCount ?? 0) < 1) {
          errors.push("task pass requires present, passing negative controls");
        }
        const privacy = asObject(receipt.privacyStatus);
        if (privacy.pass !== true || privacy.rawContentReturned === true) {
          errors.push("task pass requires a passing privacy status without raw content");
        }
        const interference = asObject(receipt.interferenceStatus);
        if (interference.pass !== true) errors.push("task pass cannot pass through interference");
        const cleanup = asObject(receipt.cleanupStatus);
        const survivors = Array.isArray(cleanup.survivors) ? cleanup.survivors : [];
        if (cleanup.closed !== true || survivors.length > 0) {
          errors.push("task pass requires closed survivor-free cleanup");
        }
        return errors;
      },
    }],
    description: "Aggregate one task's receipts into a fresh, identity-bound completion receipt.",
  }),
  schema({
    primitiveId: "devtools.consistency.verify-family",
    tool: "script-kit-devtools.consistency",
    commands: ["consistency.verify-family"],
    requiredPaths: [
      "evidenceClass",
      "familyId",
      "binding",
      "memberReceiptCount",
      "runtimeProofCount",
      "runtimeProofPaths",
      "unprovenMemberReceiptPaths",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "family-pass-requires-declared-binding",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const binding = asObject(receipt.binding);
        const errors: string[] = [];
        if (!(
          typeof binding.familyId === "string" && binding.familyId === receipt.familyId &&
          typeof binding.appView === "string" && binding.appView.length > 0 &&
          typeof binding.host === "string" && binding.host.length > 0
        )) {
          errors.push(
            "family pass requires a declared binding with matching familyId, AppView, and host",
          );
        }
        const runtimeProofPaths = Array.isArray(receipt.runtimeProofPaths)
          ? receipt.runtimeProofPaths
          : [];
        const unproven = Array.isArray(receipt.unprovenMemberReceiptPaths)
          ? receipt.unprovenMemberReceiptPaths
          : [];
        if (
          receipt.evidenceClass !== "DIRECT_RUNTIME_PROOF" ||
          Number(receipt.memberReceiptCount) < 1 ||
          receipt.runtimeProofCount !== receipt.memberReceiptCount ||
          runtimeProofPaths.length !== Number(receipt.runtimeProofCount) ||
          unproven.length > 0
        ) {
          errors.push(
            "family pass requires a fresh target-matched direct runtime receipt for every member",
          );
        }
        return errors;
      },
    }],
    description: "Verify one deterministic family fixture binding and its member receipts.",
  }),
  schema({
    primitiveId: "devtools.consistency.verify-scope",
    tool: "script-kit-devtools.consistency",
    commands: ["consistency.verify-scope"],
    requiredPaths: [
      "scope",
      "catalogTaskCount",
      "scopeTaskCount",
      "scopePassedTaskCount",
      "missingScopeTaskIds",
      "taskDispositions",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "scope-pass-requires-complete-scope",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        if (receipt.catalogTaskCount !== 75) errors.push("scope pass requires the 75-task catalog");
        if (receipt.scopeTaskCount !== receipt.scopePassedTaskCount) {
          errors.push("scope pass requires every scope task to pass");
        }
        const missing = Array.isArray(receipt.missingScopeTaskIds) ? receipt.missingScopeTaskIds : [];
        if (missing.length > 0) errors.push("scope pass requires zero missing scope tasks");
        return errors;
      },
    }],
    description: "Aggregate the 28-task cons-proof-gov scope into one completion receipt.",
  }),
  schema({
    primitiveId: "devtools.consistency.verify-all",
    tool: "script-kit-devtools.consistency",
    commands: ["consistency.verify-all"],
    requiredPaths: [
      "programTaskCount",
      "passedTaskCount",
      "missingTaskIds",
      "blockedTaskIds",
      "invalidTaskIds",
      "failedTaskIds",
      "proofCoverage.runtimeInteractionRequiredTaskCount",
      "proofCoverage.runtimeInteractionProvenTaskCount",
      "proofCoverage.runtimeInteractionBlockedTaskIds",
      "privacyPass",
      "cleanup.closed",
      "protectedHashesPass",
      "generatedOutputsPass",
      "conflictLifecyclePass",
      "facadeLifecyclePass",
    ],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "program-pass-requires-75-of-75-and-clean-governance",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const errors: string[] = [];
        if (receipt.programTaskCount !== 75 || receipt.passedTaskCount !== 75) {
          errors.push("program pass requires exactly 75 of 75 passing tasks");
        }
        const proofCoverage = asObject(receipt.proofCoverage);
        if (
          proofCoverage.runtimeInteractionRequiredTaskCount !==
            proofCoverage.runtimeInteractionProvenTaskCount ||
          !Array.isArray(proofCoverage.runtimeInteractionBlockedTaskIds) ||
          proofCoverage.runtimeInteractionBlockedTaskIds.length > 0
        ) {
          errors.push("program pass requires direct runtime proof for every interaction task");
        }
        for (const field of ["missingTaskIds", "blockedTaskIds", "invalidTaskIds", "failedTaskIds"]) {
          const value = receipt[field];
          if (!Array.isArray(value) || value.length > 0) errors.push(`program pass requires empty ${field}`);
        }
        if (receipt.privacyPass !== true) errors.push("program pass requires privacy pass");
        if (asObject(receipt.cleanup).closed !== true) errors.push("program pass requires closed cleanup");
        for (const field of [
          "protectedHashesPass",
          "generatedOutputsPass",
          "conflictLifecyclePass",
          "facadeLifecyclePass",
        ]) {
          if (receipt[field] !== true) errors.push(`program pass requires ${field}`);
        }
        return errors;
      },
    }],
    description: "Audit all 75 consistency tasks against final source, binary, fixture, and cleanup identities.",
  }),
];

export interface ReceiptValidationResult {
  primitiveId: string;
  valid: boolean;
  disposition: ReceiptDisposition;
  errors: string[];
  requiredFields: string[];
  activationProof: boolean;
}

function asObject(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : {};
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === "string")
    : [];
}

function objectArray(value: unknown): JsonObject[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is JsonObject => Boolean(entry) && typeof entry === "object" && !Array.isArray(entry))
    : [];
}

function pathValues(value: unknown, path: string): unknown[] {
  const segments = path.split(".");
  let values: unknown[] = [value];
  for (const segment of segments) {
    const array = segment.endsWith("[]");
    const key = array ? segment.slice(0, -2) : segment;
    const next: unknown[] = [];
    for (const candidate of values) {
      if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) continue;
      const field = (candidate as JsonObject)[key];
      if (array) {
        if (Array.isArray(field)) next.push(...field);
      } else {
        next.push(field);
      }
    }
    values = next;
  }
  return values;
}

function hasRequiredPath(receipt: JsonObject, path: string): boolean {
  const values = pathValues(receipt, path);
  return values.length > 0 && values.every((value) => value !== null && value !== undefined);
}

function deepFailedAssertions(value: unknown, path: string[] = []): string[] {
  if (Array.isArray(value)) {
    return value.flatMap((entry, index) => deepFailedAssertions(entry, [...path, String(index)]));
  }
  if (!value || typeof value !== "object") return [];
  const object = value as JsonObject;
  const failures: string[] = [];
  if (object.pass === false || object.ok === false) failures.push(path.join(".") || "assertions");
  for (const [key, entry] of Object.entries(object)) {
    failures.push(...deepFailedAssertions(entry, [...path, key]));
  }
  return failures;
}

function dispositionForClassification(classification: unknown): ReceiptDisposition {
  const value = String(classification ?? "");
  if (value === "ok" || value === "fixed" || value === "not-reproduced") return "EVALUABLE_PASS";
  if (value === "reproduced") return "EVALUABLE_FAIL";
  if (value.includes("target-ambiguity")) return "BLOCKED_TARGET_AMBIGUITY";
  if (value.includes("stale-generation")) return "BLOCKED_STALE_GENERATION";
  if (value.includes("permission")) return "BLOCKED_PERMISSION";
  if (value.includes("real-data") || value.includes("unsafe")) return "BLOCKED_REAL_DATA_RISK";
  if (value.includes("timeout") || value.includes("queue") || value.includes("parse-error")) return "BLOCKED_TIMEOUT";
  if (value.includes("scope-drift")) return "BLOCKED_SCOPE_DRIFT";
  if (value.includes("unsupported-projection")) return "BLOCKED_UNSUPPORTED_PROJECTION";
  if (value.includes("invalid-identity")) return "INVALID_IDENTITY";
  if (value.includes("invalid-generation")) return "INVALID_GENERATION";
  if (value.includes("invalid-binary")) return "INVALID_BINARY";
  if (value.includes("invalid-fixture")) return "INVALID_FIXTURE";
  if (value.includes("interference")) return "INVALID_INTERFERENCE";
  if (value.includes("observer")) return "INVALID_OBSERVER";
  if (value.includes("cleanup")) return "INVALID_CLEANUP";
  if (value.includes("analysis-pending")) return "ANALYSIS_PENDING";
  if (value.includes("invalid-schema")) return "INVALID_SCHEMA";
  if (value.includes("invalid-privacy")) return "INVALID_PRIVACY";
  return "BLOCKED_MISSING_PRIMITIVE";
}

function isEvaluable(disposition: ReceiptDisposition): boolean {
  return disposition === "EVALUABLE_PASS" || disposition === "EVALUABLE_FAIL";
}

function transactionIdentityErrors(receipt: JsonObject): string[] {
  const transaction = asObject(receipt.transaction);
  const required = [
    "transactionId",
    "runId",
    "pid",
    "processStartTime",
    "binarySha256",
    "automationId",
    "windowInstanceId",
    "windowGeneration",
    "windowKind",
    "bounds",
    "targetGeneration",
    "surfaceGeneration",
    "dataGeneration",
  ];
  const errors = required
    .filter((field) => transaction[field] === null || transaction[field] === undefined || transaction[field] === "")
    .map((field) => `missing proof transaction field: ${field}`);
  if (transaction.surfaceKind == null && transaction.semanticSurface == null) {
    errors.push("missing proof transaction field: surfaceKind|semanticSurface");
  }

  if (
    transaction.binarySha256 != null &&
    !/^[a-fA-F0-9]{64}$/.test(String(transaction.binarySha256))
  ) {
    errors.push("invalid binary proof transaction fingerprint");
  }
  if (
    transaction.pid != null &&
    (!Number.isSafeInteger(transaction.pid) || Number(transaction.pid) <= 0)
  ) {
    errors.push("invalid proof transaction process identity");
  }
  for (const field of [
    "windowGeneration",
    "targetGeneration",
    "surfaceGeneration",
    "dataGeneration",
  ]) {
    if (
      transaction[field] != null &&
      (!Number.isSafeInteger(transaction[field]) || Number(transaction[field]) < 0)
    ) {
      errors.push(`invalid proof transaction generation: ${field}`);
    }
  }

  const explicitRunId =
    typeof receipt.runId === "string"
      ? receipt.runId
      : typeof receipt.session === "string"
        ? receipt.session
        : null;
  if (
    explicitRunId !== null &&
    typeof transaction.runId === "string" &&
    explicitRunId !== transaction.runId
  ) {
    errors.push("proof transaction run identity disagrees with receipt run identity");
  }

  const targetFields = [
    "automationId",
    "windowInstanceId",
    "pid",
    "windowGeneration",
    "targetGeneration",
    "surfaceGeneration",
    "dataGeneration",
    "surfaceKind",
    "semanticSurface",
    "appViewVariant",
  ] as const;
  const candidates: Array<[string, JsonObject]> = [
    ["target", asObject(receipt.target)],
    ["resolvedTarget", asObject(receipt.resolvedTarget)],
    ["targetBefore", asObject(receipt.targetBefore)],
    ["targetAfter", asObject(receipt.targetAfter)],
    ["targetIdentity", asObject(receipt.targetIdentity)],
    [
      "surfaceContract.targetIdentity",
      asObject(asObject(receipt.surfaceContract).targetIdentity),
    ],
    [
      "state.targetIdentity",
      asObject(asObject(receipt.state).targetIdentity),
    ],
    [
      "state.surfaceContract.targetIdentity",
      asObject(asObject(asObject(receipt.state).surfaceContract).targetIdentity),
    ],
    [
      "target.surfaceContract.targetIdentity",
      asObject(asObject(asObject(receipt.target).surfaceContract).targetIdentity),
    ],
    [
      "resolvedTarget.surfaceContract.targetIdentity",
      asObject(asObject(asObject(receipt.resolvedTarget).surfaceContract).targetIdentity),
    ],
  ];
  for (const [location, candidate] of candidates) {
    if (
      candidate.windowId != null &&
      transaction.automationId != null &&
      candidate.windowId !== transaction.automationId
    ) {
      errors.push(
        `proof transaction identity disagrees with ${location}.windowId`,
      );
    }
    if (
      candidate.stableTargetId != null &&
      transaction.automationId != null &&
      candidate.stableTargetId !== transaction.automationId
    ) {
      errors.push(
        `proof transaction identity disagrees with ${location}.stableTargetId`,
      );
    }
    for (const field of targetFields) {
      if (
        candidate[field] != null &&
        transaction[field] != null &&
        candidate[field] !== transaction[field]
      ) {
        const kind = field.endsWith("Generation") ? "generation" : "identity";
        errors.push(
          `proof transaction ${kind} disagrees with ${location}.${field}`,
        );
      }
    }
  }

  const requestedTarget = asObject(receipt.requestedTarget);
  const requestedSelector = asObject(requestedTarget.selector);
  const selector = Object.keys(requestedSelector).length > 0
    ? requestedSelector
    : requestedTarget;
  if (selector.type === "id") {
    if (typeof selector.id !== "string" || selector.id.length === 0) {
      errors.push("proof transaction identity disagrees with empty requested target selector id");
    } else if (
      transaction.automationId != null &&
      selector.id !== transaction.automationId
    ) {
      errors.push("proof transaction identity disagrees with requestedTarget.selector.id");
    }
  } else if (
    selector.type === "main" &&
    transaction.automationId != null &&
    transaction.automationId !== "main"
  ) {
    errors.push("proof transaction identity disagrees with requested main target");
  }
  if (
    process.env.SCRIPT_KIT_NONINTERACTIVE === "1" &&
    selector.type === "focused"
  ) {
    errors.push("proof transaction identity disagrees with noninteractive focused target policy");
  }
  for (const field of ["id", "windowId", "automationId"] as const) {
    if (
      requestedTarget[field] != null &&
      transaction.automationId != null &&
      requestedTarget[field] !== transaction.automationId
    ) {
      errors.push(
        `proof transaction identity disagrees with requestedTarget.${field}`,
      );
    }
  }

  const binary = asObject(receipt.binary);
  const reportedBinarySha = binary.sha256 ?? binary.binarySha256;
  if (
    reportedBinarySha != null &&
    transaction.binarySha256 != null &&
    reportedBinarySha !== transaction.binarySha256
  ) {
    errors.push("invalid binary: receipt fingerprint disagrees with proof transaction");
  }
  return errors;
}

function invalidDispositionFor(errors: string[]): ReceiptDisposition {
  if (errors.some((error) => error.includes("invalid binary"))) {
    return "INVALID_BINARY";
  }
  if (errors.some((error) => error.includes("generation"))) {
    return "INVALID_GENERATION";
  }
  if (
    errors.some(
      (error) =>
        error.includes("duplicate semantic IDs") ||
        error.includes("identity disagrees") ||
        error.includes("process identity"),
    )
  ) {
    return "INVALID_IDENTITY";
  }
  return "INVALID_SCHEMA";
}

export function receiptSchema(primitiveId: string): ReceiptSchemaDefinition | undefined {
  return receiptSchemaRegistry.find((entry) => entry.primitiveId === primitiveId);
}

export function validateReceipt(
  primitiveId: string,
  receipt: JsonObject,
): ReceiptValidationResult {
  const definition = receiptSchema(primitiveId);
  const errors: string[] = [];
  if (!definition) {
    errors.push(`unknown primitive: ${primitiveId}`);
    return {
      primitiveId,
      valid: false,
      disposition: "INVALID_SCHEMA",
      errors,
      requiredFields: [],
      activationProof: false,
    };
  }
  if (receipt.schemaVersion !== RECEIPT_SCHEMA_VERSION) {
    errors.push(`schemaVersion must equal ${RECEIPT_SCHEMA_VERSION}`);
  }
  if (receipt.tool !== definition.tool) errors.push(`tool must equal ${definition.tool}`);
  if (!definition.commands.includes(String(receipt.command ?? ""))) {
    errors.push(`command must be one of ${definition.commands.join(", ")}`);
  }
  if (typeof receipt.classification !== "string") errors.push("classification is required");
  const evidenceObservation = classifyReceiptEvidence(receipt);
  errors.push(...evidenceObservation.errors);
  const requestedDisposition = dispositionForClassification(receipt.classification);
  if (!definition.allowedDispositions.includes(requestedDisposition)) {
    errors.push(`disposition ${requestedDisposition} is not allowed`);
  }
  if (typeof receipt.disposition === "string" && receipt.disposition !== requestedDisposition) {
    errors.push("candidate disposition disagrees with classification");
  }
  if (typeof receipt.pass === "boolean" && receipt.pass !== (requestedDisposition === "EVALUABLE_PASS")) {
    errors.push("pass must equal disposition === EVALUABLE_PASS");
  }
  if (isEvaluable(requestedDisposition)) {
    for (const field of definition.nonNullPaths) {
      if (!hasRequiredPath(receipt, field)) errors.push(`missing required field: ${field}`);
    }
    if (definition.identityPolicy !== "none") {
      errors.push(...transactionIdentityErrors(receipt));
    }
  }

  const missing = stringArray(receipt.missingPrimitives).filter(Boolean);
  if (requestedDisposition === "EVALUABLE_PASS" && definition.forbidMissingPrimitivesOnPass && missing.length > 0) {
    errors.push(`pass receipt has required missing primitives: ${missing.join(", ")}`);
  }
  if (requestedDisposition === "EVALUABLE_PASS" && Array.isArray(receipt.errors) && receipt.errors.length > 0) {
    errors.push("pass receipt contains errors");
  }
  if (requestedDisposition === "EVALUABLE_PASS") {
    const failedAssertions = deepFailedAssertions(receipt.assertions);
    if (failedAssertions.length > 0) {
      errors.push(`pass receipt contains failed assertions: ${failedAssertions.join(", ")}`);
    }
  }
  if (primitiveId === "devtools.elements.snapshot") {
    const duplicates = Array.isArray(receipt.duplicateSemanticIds) ? receipt.duplicateSemanticIds : [];
    if (duplicates.length > 0) errors.push("duplicate semantic IDs are not evaluable");
  }
  if (primitiveId === "devtools.keyboard.inspect") {
    const duplicates = Array.isArray(receipt.duplicateKeys) ? receipt.duplicateKeys : [];
    if (requestedDisposition === "EVALUABLE_PASS" && duplicates.length > 0 && receipt.routingPriorityResolved !== true) {
      errors.push("duplicate shortcut keys lack an explicit routing priority");
    }
  }
  for (const predicate of definition.predicates) {
    errors.push(...predicate.validate(receipt, requestedDisposition));
  }

  return {
    primitiveId,
    valid: errors.length === 0,
    disposition: errors.length === 0 ? requestedDisposition : invalidDispositionFor(errors),
    errors,
    requiredFields: definition.requiredPaths,
    activationProof: definition.activationProof === true,
  };
}

let cachedGitCommit: string | null | undefined;
function gitCommit(): string | null {
  if (cachedGitCommit !== undefined) return cachedGitCommit;
  const result = Bun.spawnSync(["git", "rev-parse", "HEAD"], { stdout: "pipe", stderr: "pipe" });
  cachedGitCommit = result.exitCode === 0
    ? new TextDecoder().decode(result.stdout).trim()
    : null;
  return cachedGitCommit;
}

const producerFileByTool: Record<string, string> = {
  "script-kit-devtools.targets": "targets.ts",
  "script-kit-devtools.surface": "surface.ts",
  "script-kit-devtools.surfaces": "surfaces.ts",
  "script-kit-devtools.elements": "elements.ts",
  "script-kit-devtools.layout": "layout.ts",
  "script-kit-devtools.scroll": "scroll.ts",
  "script-kit-devtools.focus": "focus.ts",
  "script-kit-devtools.text": "text.ts",
  "script-kit-devtools.keyboard": "keyboard.ts",
  "script-kit-devtools.actions": "actions.ts",
  "script-kit-devtools.act": "act.ts",
  "script-kit-devtools.compare": "compare.ts",
  "script-kit-devtools.notes": "notes.ts",
  "script-kit-devtools.dictation": "dictation.ts",
  "script-kit-devtools.inspect": "inspect.ts",
  "script-kit-devtools.consistency": "consistency.ts",
  "script-kit-devtools.family-fixtures": "family-fixtures.ts",
  "script-kit-devtools.safe-task-proofs": "safe-task-proofs.ts",
  "script-kit-devtools.glass-observers": "glass-observers.ts",
  "script-kit-mockups.story-browser-geometry":
    "../agentic/cons-proof-gov/story-geometry-proof.mjs",
};

function sha256(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

function fileFingerprint(path: string): string | null {
  try {
    return sha256(readFileSync(path));
  } catch {
    return null;
  }
}

const sharedReceiptPolicyOwners = [
  "receipt-schema.ts",
  "privacy.ts",
  "evidence-class.ts",
  "task-proof-policy.ts",
] as const;
let cachedReceiptPolicyFingerprint: string | null = null;

function receiptPolicySourceFingerprint(): string {
  if (cachedReceiptPolicyFingerprint !== null) {
    return cachedReceiptPolicyFingerprint;
  }
  cachedReceiptPolicyFingerprint = sha256(
    sharedReceiptPolicyOwners
      .map((filename) => {
        const fingerprint = fileFingerprint(resolve(import.meta.dir, filename));
        return `${filename}:${fingerprint ?? "missing"}`;
      })
      .join(":"),
  );
  return cachedReceiptPolicyFingerprint;
}

function producerSourceFingerprint(tool: unknown): string {
  const filename = producerFileByTool[String(tool ?? "")];
  const producerPath = filename ? resolve(import.meta.dir, "..", filename) : null;
  const fingerprints = [receiptPolicySourceFingerprint(), producerPath ? fileFingerprint(producerPath) : null]
    .filter((value): value is string => Boolean(value));
  return sha256(fingerprints.join(":"));
}

function safePath(value: unknown): unknown {
  if (typeof value !== "string") return value;
  if (!value.startsWith("/")) return value;
  const repoRelative = relative(process.cwd(), value);
  return !repoRelative.startsWith("..") && !repoRelative.startsWith("/")
    ? repoRelative
    : `external-sha256:${sha256(value).slice(0, 24)}`;
}

function privacyOptions(receipt: JsonObject) {
  const requested = receipt.privacyMode;
  const mode: ReceiptPrivacyMode = requested === "fixture-redacted" || requested === "fixture-cleartext"
    ? requested
    : "live-redacted";
  const fixture = asObject(receipt.fixture);
  const safety = asObject(receipt.safety);
  return {
    mode,
    fixtureId: typeof fixture.id === "string" ? fixture.id : null,
    sandboxHome: receipt.sandboxHome === true || safety.sandboxHome === true,
    fixtureAllowsCleartext: fixture.allowCleartext === true,
    callerAllowsCleartext: receipt.allowFixtureCleartext === true,
    nativeDataInvolved: receipt.nativeDataInvolved === true,
  };
}

function normalizeCandidatePaths(receipt: JsonObject): JsonObject {
  const candidate = { ...receipt };
  if (receipt.binary && typeof receipt.binary === "object" && !Array.isArray(receipt.binary)) {
    candidate.binary = { ...receipt.binary as JsonObject, path: safePath((receipt.binary as JsonObject).path) };
  }
  if (receipt.fixture && typeof receipt.fixture === "object" && !Array.isArray(receipt.fixture)) {
    candidate.fixture = { ...receipt.fixture as JsonObject, path: safePath((receipt.fixture as JsonObject).path) };
  }
  return candidate;
}

const envelopeRequiredPaths = [
  "schemaVersion",
  "primitiveId",
  "tool",
  "command",
  "receiptId",
  "runId",
  "taskIds",
  "startedAt",
  "endedAt",
  "durationMs",
  "repository",
  "privacy.mode",
  "privacy.fingerprintAlgorithm",
  "privacy.rawContentReturned",
  "privacy.recursiveCanaryScan.performed",
  "privacy.recursiveCanaryScan.pass",
  "evidence",
  "requiredPrimitives",
  "missingPrimitives",
  "assertions",
  "negativeControls",
  "interference",
  "cleanup.closed",
  "disposition",
  "pass",
  "errors",
  "warnings",
  "producerValidation.registryVersion",
  "producerValidation.schemaId",
  "producerValidation.valid",
  "producerValidation.errors",
];

function envelopeErrors(receipt: JsonObject): string[] {
  const errors = envelopeRequiredPaths
    .filter((path) => !hasRequiredPath(receipt, path))
    .map((path) => `missing receipt envelope field: ${path}`);
  const disposition = receipt.disposition as ReceiptDisposition;
  if (!receiptDispositions.includes(disposition)) errors.push("receipt envelope disposition is unknown");
  if (receipt.pass !== (disposition === "EVALUABLE_PASS")) {
    errors.push("receipt envelope pass disagrees with disposition");
  }
  return errors;
}

function normalizedCleanup(value: unknown): JsonObject {
  const cleanup = asObject(value);
  return {
    ...cleanup,
    ownedPids: Array.isArray(cleanup.ownedPids) ? cleanup.ownedPids : [],
    ownedSessions: Array.isArray(cleanup.ownedSessions) ? cleanup.ownedSessions : [],
    ownedBrowserPids: Array.isArray(cleanup.ownedBrowserPids) ? cleanup.ownedBrowserPids : [],
    closed: typeof cleanup.closed === "boolean" ? cleanup.closed : true,
    survivors: Array.isArray(cleanup.survivors) ? cleanup.survivors : [],
  };
}

function normalizedInterference(value: unknown): JsonObject {
  const interference = asObject(value);
  return {
    ...interference,
    monitored: typeof interference.monitored === "boolean" ? interference.monitored : false,
    disposition: interference.disposition ?? null,
    details: interference.details ?? null,
  };
}

function buildEnvelope(
  primitiveId: string,
  sanitized: JsonObject,
  validation: ReceiptValidationResult,
  privacy: ReturnType<typeof sanitizeReceipt>,
): JsonObject {
  const definition = receiptSchema(primitiveId)!;
  const startedAt = typeof sanitized.startedAt === "string" ? sanitized.startedAt : new Date().toISOString();
  const endedAt = typeof sanitized.endedAt === "string" ? sanitized.endedAt : new Date().toISOString();
  const transaction = asObject(sanitized.transaction);
  const runId = typeof sanitized.runId === "string"
    ? sanitized.runId
    : typeof sanitized.session === "string"
      ? sanitized.session
      : typeof transaction.runId === "string"
        ? transaction.runId
        : `process-${process.pid}`;
  const taskIds = stringArray(sanitized.taskIds).length > 0
    ? stringArray(sanitized.taskIds)
    : (process.env.SCRIPT_KIT_RECEIPT_TASK_IDS ?? "")
      .split(",")
      .map((value) => value.trim())
      .filter(Boolean);
  const sourceFingerprint = producerSourceFingerprint(sanitized.tool);
  const candidateRepository = asObject(sanitized.repository);
  const evidence = asObject(sanitized.evidence);
  const errors = Array.isArray(sanitized.errors) ? sanitized.errors : [];
  const warnings = Array.isArray(sanitized.warnings) ? sanitized.warnings : [];
  const disposition = validation.disposition;
  const receiptId = typeof sanitized.receiptId === "string"
    ? sanitized.receiptId
    : `${primitiveId}:${sha256(`${runId}:${startedAt}:${sanitized.command}`).slice(0, 20)}`;
  const evidenceObservation = classifyReceiptEvidence(sanitized);

  return {
    ...sanitized,
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    primitiveId,
    evidenceClass: evidenceObservation.evidenceClass,
    evidenceObservation: {
      observedWindowVisible: evidenceObservation.observedWindowVisible,
      visibilitySources: evidenceObservation.visibilitySources,
      errors: evidenceObservation.errors,
    },
    receiptId,
    runId,
    taskIds,
    startedAt,
    endedAt,
    durationMs: typeof sanitized.durationMs === "number" ? sanitized.durationMs : 0,
    repository: {
      gitCommit: candidateRepository.gitCommit ?? gitCommit(),
      implementationFingerprint: candidateRepository.implementationFingerprint ?? sourceFingerprint,
      producerSourceFingerprint: candidateRepository.producerSourceFingerprint ?? sourceFingerprint,
      baselineDirtyPaths: Array.isArray(candidateRepository.baselineDirtyPaths)
        ? candidateRepository.baselineDirtyPaths
        : [],
      newUnownedDirtyPaths: Array.isArray(candidateRepository.newUnownedDirtyPaths)
        ? candidateRepository.newUnownedDirtyPaths
        : [],
    },
    binary: sanitized.binary ?? null,
    fixture: sanitized.fixture ?? null,
    transaction: sanitized.transaction ?? null,
    privacy: {
      mode: privacy.mode,
      fingerprintAlgorithm: privacy.fingerprintAlgorithm,
      keyId: privacy.keyId,
      rawContentReturned: privacy.rawContentReturned,
      recursiveCanaryScan: {
        performed: true,
        matches: [],
        pass: privacy.canaryMatches === 0,
      },
      redactedCount: privacy.redactedCount,
      canariesRedacted: privacy.canariesRedacted,
      canaryMatches: privacy.canaryMatches,
      unclassifiedSensitivePaths: privacy.unclassifiedSensitivePaths,
    },
    evidence: {
      intended: evidence.intended ?? null,
      model: evidence.model ?? null,
      rendered: evidence.rendered ?? null,
      accessibility: evidence.accessibility ?? null,
      interaction: evidence.interaction ?? null,
    },
    requiredPrimitives: stringArray(sanitized.requiredPrimitives),
    missingPrimitives: stringArray(sanitized.missingPrimitives),
    assertions: objectArray(sanitized.assertions),
    negativeControls: objectArray(sanitized.negativeControls),
    interference: normalizedInterference(sanitized.interference),
    cleanup: normalizedCleanup(sanitized.cleanup),
    disposition,
    pass: disposition === "EVALUABLE_PASS",
    errors,
    warnings,
    proofCapabilities: {
      inspection: true,
      activation: validation.activationProof,
    },
    producerValidation: {
      registryVersion: RECEIPT_REGISTRY_VERSION,
      schemaId: `${primitiveId}@${definition.version}`,
      valid: true,
      errors: [],
    },
    validation: {
      passed: true,
      errors: [],
      requiredFields: validation.requiredFields,
    },
  };
}

function invalidEnvelope(
  primitiveId: string,
  sanitized: JsonObject,
  validation: ReceiptValidationResult,
  privacy: ReturnType<typeof sanitizeReceipt>,
): JsonObject {
  const disposition = validation.disposition;
  const now = new Date().toISOString();
  return {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    primitiveId,
    tool: typeof sanitized.tool === "string" ? sanitized.tool : null,
    command: typeof sanitized.command === "string" ? sanitized.command : null,
    receiptId: `${primitiveId}:invalid:${sha256(JSON.stringify(validation.errors)).slice(0, 16)}`,
    runId: typeof sanitized.session === "string" ? sanitized.session : `process-${process.pid}`,
    taskIds: stringArray(sanitized.taskIds),
    startedAt: typeof sanitized.startedAt === "string" ? sanitized.startedAt : now,
    endedAt: now,
    durationMs: typeof sanitized.durationMs === "number" ? sanitized.durationMs : 0,
    repository: {
      gitCommit: gitCommit(),
      implementationFingerprint: producerSourceFingerprint(sanitized.tool),
      producerSourceFingerprint: producerSourceFingerprint(sanitized.tool),
      baselineDirtyPaths: [],
      newUnownedDirtyPaths: [],
    },
    binary: null,
    fixture: null,
    transaction: null,
    classification: "blocked-by-invalid-receipt",
    privacy: {
      mode: privacy.mode,
      fingerprintAlgorithm: privacy.fingerprintAlgorithm,
      keyId: privacy.keyId,
      rawContentReturned: false,
      recursiveCanaryScan: {
        performed: true,
        matches: [],
        pass: privacy.canaryMatches === 0,
      },
      redactedCount: privacy.redactedCount,
      canariesRedacted: privacy.canariesRedacted,
      canaryMatches: privacy.canaryMatches,
      unclassifiedSensitivePaths: privacy.unclassifiedSensitivePaths,
    },
    evidence: {
      intended: null,
      model: null,
      rendered: null,
      accessibility: null,
      interaction: null,
    },
    requiredPrimitives: [],
    missingPrimitives: [],
    assertions: [],
    negativeControls: [],
    interference: normalizedInterference(null),
    cleanup: normalizedCleanup(null),
    disposition,
    pass: false,
    errors: [],
    warnings: [],
    producerValidation: {
      registryVersion: RECEIPT_REGISTRY_VERSION,
      schemaId: `${primitiveId}@${receiptSchema(primitiveId)?.version ?? "unknown"}`,
      valid: false,
      errors: validation.errors,
    },
    validation: {
      passed: false,
      errors: validation.errors,
      requiredFields: validation.requiredFields,
    },
  };
}

function exitCodeFor(disposition: ReceiptDisposition): number {
  if (disposition === "EVALUABLE_PASS") return 0;
  if (disposition === "EVALUABLE_FAIL") return 2;
  if (disposition.startsWith("BLOCKED_")) return 3;
  return 4;
}

export function prepareValidatedReceipt(
  primitiveId: string,
  receipt: JsonObject,
): { receipt: JsonObject; validation: ReceiptValidationResult; exitCode: number } {
  const privacy = sanitizeReceipt(normalizeCandidatePaths(receipt), privacyOptions(receipt));
  const sanitized = privacy.sanitized as JsonObject;
  if (privacy.mode !== "fixture-cleartext") assertNoCleartextCanaries(sanitized);
  const validation = validateReceipt(primitiveId, sanitized);
  if (privacy.unclassifiedSensitivePaths.length > 0 || privacy.canaryMatches > 0) {
    validation.valid = false;
    validation.disposition = "INVALID_PRIVACY";
    validation.errors.push(
      privacy.unclassifiedSensitivePaths.length > 0
        ? `unclassified sensitive fields: ${privacy.unclassifiedSensitivePaths.join(", ")}`
        : "receipt privacy canary escaped",
    );
  }
  if (!validation.valid) {
    const receipt = invalidEnvelope(primitiveId, sanitized, validation, privacy);
    return { receipt, validation, exitCode: exitCodeFor(validation.disposition) };
  }

  const output = buildEnvelope(primitiveId, sanitized, validation, privacy);
  const envelopeValidationErrors = envelopeErrors(output);
  if (envelopeValidationErrors.length > 0) {
    validation.valid = false;
    validation.disposition = "INVALID_SCHEMA";
    validation.errors.push(...envelopeValidationErrors);
    const receipt = invalidEnvelope(primitiveId, sanitized, validation, privacy);
    return { receipt, validation, exitCode: exitCodeFor(validation.disposition) };
  }
  return { receipt: output, validation, exitCode: exitCodeFor(validation.disposition) };
}

export function emitValidatedReceipt(
  primitiveId: string,
  receipt: JsonObject,
  outputPath?: string,
): JsonObject {
  const prepared = prepareValidatedReceipt(primitiveId, receipt);
  if (outputPath) {
    mkdirSync(dirname(outputPath), { recursive: true });
    const temporaryPath = `${outputPath}.tmp-${process.pid}`;
    writeFileSync(temporaryPath, `${JSON.stringify(prepared.receipt, null, 2)}\n`);
    renameSync(temporaryPath, outputPath);
  }
  console.log(JSON.stringify(prepared.receipt, null, 2));
  if (prepared.exitCode !== 0) process.exitCode = prepared.exitCode;
  return prepared.receipt;
}

export function validateReceiptFile(primitiveId: string, path: string) {
  const parsed = JSON.parse(readFileSync(path, "utf8")) as JsonObject;
  return prepareValidatedReceipt(primitiveId, parsed);
}

/// Stable identity for the receipt registry itself: version plus a
/// fingerprint over the schema and its actual shared privacy/evidence/policy
/// owners. Any schema, redaction, classification, or proof-policy change
/// invalidates stale receipts without trusting timestamps or producer claims.
export function receiptRegistryIdentity(): {
  schemaVersion: number;
  registryVersion: number;
  registryFingerprint: string;
} {
  const fingerprint = receiptPolicySourceFingerprint();
  return {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    registryVersion: RECEIPT_REGISTRY_VERSION,
    registryFingerprint: sha256(`${RECEIPT_REGISTRY_VERSION}:${fingerprint}`),
  };
}

/// Producer identity for one tool name: the resolved producer source path
/// (null when the tool is unknown) and the same shared-policy/producer
/// fingerprint the receipt envelope records, so auditors never duplicate
/// hashing rules or omit privacy policy from source provenance.
export function producerIdentityForTool(tool: string): {
  producerPath: string | null;
  fingerprint: string;
} {
  const filename = producerFileByTool[tool];
  return {
    producerPath: filename ? resolve(import.meta.dir, "..", filename) : null,
    fingerprint: producerSourceFingerprint(tool),
  };
}

export function receiptRegistryReport() {
  return receiptSchemaRegistry.map((definition) => ({
    primitiveId: definition.primitiveId,
    version: definition.version,
    tool: definition.tool,
    commands: definition.commands,
    requiredFields: definition.requiredPaths,
    nonNullPaths: definition.nonNullPaths,
    allowedDispositions: definition.allowedDispositions,
    requiredEvidenceLayers: definition.requiredEvidenceLayers,
    privacyPolicy: definition.privacyPolicy,
    identityPolicy: definition.identityPolicy,
    forbidMissingPrimitivesOnPass: definition.forbidMissingPrimitivesOnPass === true,
    activationProof: definition.activationProof === true,
    predicates: definition.predicates.map((predicate) => predicate.id),
    description: definition.description,
  }));
}
