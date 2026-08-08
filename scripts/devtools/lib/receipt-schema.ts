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
  // ── GOV-006 consistency completion auditor (metadata-only; identity none) ──
  schema({
    primitiveId: "devtools.consistency.catalog",
    tool: "script-kit-devtools.consistency",
    commands: ["consistency.catalog"],
    requiredPaths: [
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
    requiredPaths: ["familyId", "binding", "memberReceiptCount"],
    identityPolicy: "none",
    privacyPolicy: "metadata-only",
    predicates: [{
      id: "family-pass-requires-declared-binding",
      validate(receipt, disposition) {
        if (disposition !== "EVALUABLE_PASS") return [];
        const binding = asObject(receipt.binding);
        return typeof binding.familyId === "string" && binding.familyId === receipt.familyId &&
            typeof binding.appView === "string" && binding.appView.length > 0 &&
            typeof binding.host === "string" && binding.host.length > 0
          ? []
          : ["family pass requires a declared binding with matching familyId, AppView, and host"];
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
  return errors;
}

function invalidDispositionFor(errors: string[]): ReceiptDisposition {
  if (errors.some((error) => error.includes("duplicate semantic IDs"))) return "INVALID_IDENTITY";
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

function producerSourceFingerprint(tool: unknown): string {
  const filename = producerFileByTool[String(tool ?? "")];
  const producerPath = filename ? resolve(import.meta.dir, "..", filename) : null;
  const schemaPath = resolve(import.meta.dir, "receipt-schema.ts");
  const fingerprints = [fileFingerprint(schemaPath), producerPath ? fileFingerprint(producerPath) : null]
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
  const runId = typeof sanitized.runId === "string"
    ? sanitized.runId
    : typeof sanitized.session === "string"
      ? sanitized.session
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

  return {
    ...sanitized,
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    primitiveId,
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
/// fingerprint over this schema file's exact bytes. Any schema, predicate,
/// or mapping change (including additive sibling reconciliation) changes the
/// fingerprint, so downstream auditors detect stale-by-identity receipts
/// without trusting timestamps.
export function receiptRegistryIdentity(): {
  schemaVersion: number;
  registryVersion: number;
  registryFingerprint: string;
} {
  const schemaPath = resolve(import.meta.dir, "receipt-schema.ts");
  const fingerprint = fileFingerprint(schemaPath) ?? "missing-receipt-schema-source";
  return {
    schemaVersion: RECEIPT_SCHEMA_VERSION,
    registryVersion: RECEIPT_REGISTRY_VERSION,
    registryFingerprint: sha256(`${RECEIPT_REGISTRY_VERSION}:${fingerprint}`),
  };
}

/// Producer identity for one tool name: the resolved producer source path
/// (null when the tool is unknown) and the same schema+producer fingerprint
/// the receipt envelope records, so auditors never duplicate hashing rules.
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
