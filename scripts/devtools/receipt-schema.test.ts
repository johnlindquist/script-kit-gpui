import { afterEach, describe, expect, test } from "bun:test";
import {
  prepareValidatedReceipt,
  receiptRegistryReport,
  validateReceipt,
} from "./lib/receipt-schema.ts";
import { diagnostic } from "./lib/privacy.ts";
import { pickWindows } from "./lib/target-identity.ts";

function proofTransaction(extra: Record<string, unknown> = {}) {
  return {
    transactionId: "proof:test",
    runId: "receipt-schema-test",
    capturedAt: "2026-08-07T00:00:00.000Z",
    pid: 42,
    processStartTime: "Fri Aug 7 00:00:00 2026",
    binarySha256: "a".repeat(64),
    automationId: "main",
    windowInstanceId: "main@1",
    nativeWindowId: null,
    axWindowId: null,
    windowKind: "Main",
    hostKind: null,
    parentAutomationId: null,
    parentWindowInstanceId: null,
    openerAutomationId: null,
    surfaceKind: "ScriptList",
    semanticSurface: "scriptList",
    appViewVariant: "ScriptList",
    routeId: null,
    routeStack: [],
    screenId: null,
    backingScaleFactor: 2,
    bounds: { x: 0, y: 0, width: 800, height: 600 },
    windowGeneration: 1,
    targetGeneration: 1,
    surfaceGeneration: 1,
    dataGeneration: 1,
    layoutGeneration: null,
    selectionGeneration: null,
    scrollGeneration: null,
    frameGeneration: null,
    ...extra,
  };
}

function baseReceipt(extra: Record<string, unknown> = {}) {
  return {
    schemaVersion: 2,
    tool: "script-kit-devtools.layout",
    command: "layout.measure",
    classification: "ok",
    requestedTarget: { selector: { type: "main" } },
    target: { automationId: "main", bounds: { x: 0, y: 0, width: 800, height: 600 } },
    window: { rect: { x: 0, y: 0, width: 800, height: 600 } },
    regions: [],
    resizePressure: { windowCanGrow: true },
    pressure: { pressureScore: 0 },
    transaction: proofTransaction(),
    missingPrimitives: [],
    warnings: [],
    errors: [],
    ...extra,
  };
}

afterEach(() => {
  delete process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES;
});

describe("executable receipt registry", () => {
  test("registry exposes stable producer-side primitive ids", () => {
    const ids = receiptRegistryReport().map((entry) => entry.primitiveId);
    expect(ids).toContain("devtools.layout.measure");
    expect(ids).toContain("devtools.elements.snapshot");
    expect(ids).toContain("devtools.keyboard.inspect");
    expect(new Set(ids).size).toBe(ids.length);
  });

  test("positive layout receipt becomes EVALUABLE_PASS", () => {
    const prepared = prepareValidatedReceipt("devtools.layout.measure", baseReceipt());
    expect(prepared.exitCode).toBe(0);
    expect(prepared.receipt.disposition).toBe("EVALUABLE_PASS");
    expect((prepared.receipt.validation as Record<string, unknown>).passed).toBe(true);
  });

  test("missing target bounds is INVALID_SCHEMA", () => {
    const receipt = baseReceipt({ target: { automationId: "main" } });
    const prepared = prepareValidatedReceipt("devtools.layout.measure", receipt);
    expect(prepared.exitCode).toBe(4);
    expect(prepared.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(JSON.stringify(prepared.receipt)).toContain("target");
  });

  test("missing proof transaction identity fails an evaluable receipt", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ transaction: null }),
    );
    expect(prepared.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(JSON.stringify(prepared.receipt)).toContain("missing proof transaction field");
  });

  test("required null and pass-with-missing-primitives fail closed", () => {
    const nullResult = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ resizePressure: null }),
    );
    expect(nullResult.receipt.disposition).toBe("INVALID_SCHEMA");

    const missingResult = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ missingPrimitives: ["windowCanGrow"] }),
    );
    expect(missingResult.receipt.disposition).toBe("INVALID_SCHEMA");
  });

  test("failed assertion cannot be marked pass", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ assertions: [{ name: "fits", pass: false }] }),
    );
    expect(prepared.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(JSON.stringify(prepared.receipt)).toContain("failed assertions");
  });

  test("duplicate semantic ids invalidate an elements pass", () => {
    const validation = validateReceipt("devtools.elements.snapshot", {
      schemaVersion: 2,
      tool: "script-kit-devtools.elements",
      command: "elements.snapshot",
      classification: "ok",
      requestedTarget: {},
      target: {},
      semanticSurface: {},
      nodes: [
        { semanticId: "same" },
        { semanticId: "same" },
      ],
      duplicateSemanticIds: ["same"],
      missingPrimitives: [],
    });
    expect(validation.valid).toBe(false);
    expect(validation.errors).toContain("duplicate semantic IDs are not evaluable");
  });

  test("duplicate keyboard key requires explicit routing priority", () => {
    const receipt = {
      schemaVersion: 2,
      tool: "script-kit-devtools.keyboard",
      command: "keyboard.inspect",
      classification: "ok",
      requestedTarget: {},
      target: {},
      keyboardPolicy: "host",
      inputOwnership: "host",
      bindings: [{ key: "cmd+k" }, { key: "cmd+k" }],
      duplicateKeys: ["cmd+k"],
      transaction: proofTransaction(),
      missingPrimitives: [],
    };
    expect(validateReceipt("devtools.keyboard.inspect", receipt).valid).toBe(false);
    expect(
      validateReceipt("devtools.keyboard.inspect", {
        ...receipt,
        routingPriorityResolved: true,
      }).valid,
    ).toBe(true);
  });

  test("external window titles are redacted in target-list receipts", () => {
    process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES = "PF003_EXTERNAL_WINDOW_TITLE";
    const targets = pickWindows({
      windows: [{
        id: "external-window",
        kind: "main",
        title: "PF003_EXTERNAL_WINDOW_TITLE",
        visible: true,
        focused: false,
      }],
    });
    const prepared = prepareValidatedReceipt("devtools.targets.list", {
      schemaVersion: 2,
      tool: "script-kit-devtools.targets",
      command: "targets.list",
      classification: "ok",
      targetCount: targets.length,
      targets,
      errors: [],
      warnings: [],
    });
    expect(prepared.receipt.disposition).toBe("EVALUABLE_PASS");
    expect(JSON.stringify(prepared.receipt)).not.toContain("PF003_EXTERNAL_WINDOW_TITLE");
    expect((prepared.receipt.privacy as Record<string, unknown>).canaryMatches).toBe(0);
  });

  test("unclassified diagnostics are invalid while explicitly typed diagnostics are safe", () => {
    const unclassified = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ errors: [{ message: "provider detail" }] }),
    );
    expect(unclassified.receipt.disposition).toBe("INVALID_PRIVACY");
    expect(JSON.stringify(unclassified.receipt)).toContain("unclassified sensitive fields");

    const classified = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ diagnosticDetails: diagnostic([{ message: "provider detail" }]) }),
    );
    expect(classified.receipt.disposition).toBe("EVALUABLE_PASS");
    expect(JSON.stringify(classified.receipt)).not.toContain("provider detail");
  });

  test("blocked evidence retains a precise disposition and exits three", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ classification: "blocked-by-stale-generation" }),
    );
    expect(prepared.validation.valid).toBe(true);
    expect(prepared.receipt.disposition).toBe("BLOCKED_STALE_GENERATION");
    expect(prepared.exitCode).toBe(3);
  });

  test("declared-transition producers require the same complete transaction identity", () => {
    const candidate = {
      schemaVersion: 2,
      tool: "script-kit-devtools.dictation",
      command: "dictation.deliverFixture",
      classification: "ok",
      safety: { syntheticTranscriptInjected: true },
      target: { requested: "mainWindowFilter" },
      delivery: { advanced: true },
      missingPrimitives: [],
      transaction: proofTransaction(),
      errors: [],
    };
    const valid = prepareValidatedReceipt("devtools.dictation.deliverFixture", candidate);
    expect(valid.receipt.disposition).toBe("EVALUABLE_PASS");
    expect(valid.exitCode).toBe(0);

    const invalid = prepareValidatedReceipt("devtools.dictation.deliverFixture", {
      ...candidate,
      transaction: null,
    });
    expect(invalid.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(invalid.exitCode).toBe(4);
  });

  test("ReceiptEnvelopeV2 is complete and pass is derived from disposition", () => {
    const prepared = prepareValidatedReceipt("devtools.layout.measure", baseReceipt());
    expect(prepared.receipt.schemaVersion).toBe(2);
    expect(typeof prepared.receipt.receiptId).toBe("string");
    expect(typeof prepared.receipt.runId).toBe("string");
    expect(Array.isArray(prepared.receipt.taskIds)).toBe(true);
    expect(typeof prepared.receipt.repository).toBe("object");
    expect(typeof prepared.receipt.evidence).toBe("object");
    expect(typeof prepared.receipt.interference).toBe("object");
    expect((prepared.receipt.cleanup as Record<string, unknown>).closed).toBe(true);
    expect((prepared.receipt.producerValidation as Record<string, unknown>).valid).toBe(true);
    expect(prepared.receipt.pass).toBe(true);
  });

  test("an evaluable failure exits two rather than masquerading as success", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ classification: "reproduced" }),
    );
    expect(prepared.receipt.disposition).toBe("EVALUABLE_FAIL");
    expect(prepared.receipt.pass).toBe(false);
    expect(prepared.exitCode).toBe(2);
  });
});
