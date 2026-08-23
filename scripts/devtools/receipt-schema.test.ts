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
    proofMode: "inspection",
    requestedTarget: { selector: { type: "main" } },
    target: { automationId: "main", bounds: { x: 0, y: 0, width: 800, height: 600 } },
    window: { rect: { x: 0, y: 0, width: 800, height: 600 } },
    regions: [],
    resizePressure: { windowCanGrow: true },
    pressure: { pressureScore: 0 },
    truthLayers: {
      model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
      rendered: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
      joins: [],
      comparableJoinCount: 1,
      unjoinedMeasurementIds: [],
    },
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
    expect(prepared.receipt.evidenceClass).toBe("RUNTIME_VISIBILITY_UNVERIFIED");
    expect((prepared.receipt.validation as Record<string, unknown>).passed).toBe(true);
  });

  test("observed target visibility classifies real target-scoped runtime evidence", () => {
    const hidden = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        target: {
          automationId: "main",
          visible: false,
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
      }),
    );
    expect(hidden.exitCode).toBe(0);
    expect(hidden.receipt.evidenceClass).toBe("RUNTIME_HIDDEN");

    const visible = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        target: {
          automationId: "main",
          visible: true,
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
      }),
    );
    if (process.env.SCRIPT_KIT_NONINTERACTIVE === "1") {
      expect(visible.exitCode).toBe(4);
      expect(visible.receipt.disposition).toBe("INVALID_SCHEMA");
      expect(visible.validation.errors).toContain(
        "noninteractive runtime evidence cannot inspect a visible target",
      );
    } else {
      expect(visible.exitCode).toBe(0);
      expect(visible.receipt.evidenceClass).toBe("RUNTIME_VISIBLE");
    }
  });

  test("hidden claims without hidden observations fail closed", () => {
    const unobserved = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ evidenceClass: "RUNTIME_HIDDEN" }),
    );
    expect(unobserved.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(unobserved.validation.errors).toContain(
      "hidden runtime evidence requires an observed hidden target",
    );

    const contradictory = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        evidenceClass: "RUNTIME_HIDDEN",
        target: {
          automationId: "main",
          visible: true,
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
      }),
    );
    expect(contradictory.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(contradictory.validation.errors).toContain(
      "hidden runtime evidence observed a visible target",
    );
  });

  test("conflicting target visibility and invented evidence classes cannot pass", () => {
    const conflicting = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        target: {
          automationId: "main",
          visible: false,
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
        windowVisible: true,
      }),
    );
    expect(conflicting.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(conflicting.validation.errors.join("\n")).toContain(
      "target visibility observations disagree",
    );

    const invented = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ evidenceClass: "VERY_CONFIDENT_RUNTIME_PROOF" }),
    );
    expect(invented.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(invented.validation.errors).toContain(
      "unsupported receipt evidence class: VERY_CONFIDENT_RUNTIME_PROOF",
    );
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

  test("target identity cannot disagree with its proof transaction", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        target: {
          automationId: "notes",
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
      }),
    );
    expect(prepared.receipt.disposition).toBe("INVALID_IDENTITY");
    expect(JSON.stringify(prepared.receipt)).toContain("target.automationId");
  });

  test("requested selector identity cannot silently target another window", () => {
    const mismatchedId = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        requestedTarget: { selector: { type: "id", id: "notes" } },
      }),
    );
    expect(mismatchedId.receipt.disposition).toBe("INVALID_IDENTITY");
    expect(mismatchedId.validation.errors).toContain(
      "proof transaction identity disagrees with requestedTarget.selector.id",
    );

    const emptyId = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ requestedTarget: { selector: { type: "id", id: "" } } }),
    );
    expect(emptyId.receipt.disposition).toBe("INVALID_IDENTITY");
    expect(emptyId.validation.errors.join("\n")).toContain(
      "empty requested target selector id",
    );

    const validId = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ requestedTarget: { selector: { type: "id", id: "main" } } }),
    );
    expect(validId.receipt.disposition).toBe("EVALUABLE_PASS");
  });

  test("main selectors and direct requested window identifiers bind their transaction", () => {
    const wrongMain = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        target: {
          automationId: "notes",
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
        transaction: proofTransaction({ automationId: "notes" }),
      }),
    );
    expect(wrongMain.receipt.disposition).toBe("INVALID_IDENTITY");
    expect(wrongMain.validation.errors).toContain(
      "proof transaction identity disagrees with requested main target",
    );

    for (const field of ["id", "windowId", "automationId"]) {
      const mismatched = prepareValidatedReceipt(
        "devtools.layout.measure",
        baseReceipt({
          requestedTarget: {
            selector: { type: "main" },
            [field]: "notes",
          },
        }),
      );
      expect(mismatched.receipt.disposition).toBe("INVALID_IDENTITY");
      expect(mismatched.validation.errors).toContain(
        `proof transaction identity disagrees with requestedTarget.${field}`,
      );
    }
  });

  test("after-target and canonical nested hidden identities cannot forge window lineage", () => {
    for (const [location, extra] of [
      ["targetAfter.windowId", { targetAfter: { windowId: "notes" } }],
      ["targetIdentity.windowId", { targetIdentity: { windowId: "notes" } }],
      [
        "surfaceContract.targetIdentity.windowId",
        { surfaceContract: { targetIdentity: { windowId: "notes" } } },
      ],
      [
        "state.surfaceContract.targetIdentity.windowId",
        { state: { surfaceContract: { targetIdentity: { windowId: "notes" } } } },
      ],
      [
        "resolvedTarget.surfaceContract.targetIdentity.windowId",
        {
          resolvedTarget: {
            automationId: "main",
            surfaceContract: { targetIdentity: { windowId: "notes" } },
          },
        },
      ],
    ] as Array<[string, Record<string, unknown>]>) {
      const prepared = prepareValidatedReceipt(
        "devtools.layout.measure",
        baseReceipt(extra),
      );

      expect(prepared.receipt.disposition, location).toBe("INVALID_IDENTITY");
      expect(prepared.validation.errors.join("\n"), location).toContain(location);
    }
  });

  test("canonical hidden-state generations must match the proof transaction", () => {
    for (const field of [
      "windowGeneration",
      "targetGeneration",
      "surfaceGeneration",
      "dataGeneration",
    ]) {
      const prepared = prepareValidatedReceipt(
        "devtools.layout.measure",
        baseReceipt({
          state: {
            surfaceContract: {
              targetIdentity: {
                windowId: "main",
                [field]: 2,
              },
            },
          },
        }),
      );

      expect(prepared.receipt.disposition, field).toBe("INVALID_GENERATION");
      expect(prepared.validation.errors.join("\n"), field).toContain(
        `state.surfaceContract.targetIdentity.${field}`,
      );
    }
    const valid = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        state: {
          surfaceContract: {
            targetIdentity: {
              windowId: "main",
              windowGeneration: 1,
              targetGeneration: 1,
              surfaceGeneration: 1,
              dataGeneration: 1,
            },
          },
        },
      }),
    );
    expect(valid.receipt.disposition).toBe("EVALUABLE_PASS");
  });

  test("reopened window instances cannot reuse an earlier target proof", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        target: {
          automationId: "main",
          windowInstanceId: "main@2",
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
      }),
    );
    expect(prepared.receipt.disposition).toBe("INVALID_IDENTITY");
    expect(JSON.stringify(prepared.receipt)).toContain("target.windowInstanceId");
  });

  test("target generation drift is invalid rather than evaluable", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({
        target: {
          automationId: "main",
          targetGeneration: 2,
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
      }),
    );
    expect(prepared.receipt.disposition).toBe("INVALID_GENERATION");
    expect(JSON.stringify(prepared.receipt)).toContain("target.targetGeneration");
  });

  test("malformed and mismatched binary fingerprints fail closed", () => {
    const malformed = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ transaction: proofTransaction({ binarySha256: "not-a-sha" }) }),
    );
    expect(malformed.receipt.disposition).toBe("INVALID_BINARY");

    const mismatched = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ binary: { sha256: "b".repeat(64) } }),
    );
    expect(mismatched.receipt.disposition).toBe("INVALID_BINARY");
  });

  test("receipt and proof transaction run IDs must agree", () => {
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ runId: "another-run" }),
    );
    expect(prepared.receipt.disposition).toBe("INVALID_IDENTITY");
    expect(JSON.stringify(prepared.receipt)).toContain("run identity");
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
      semanticSurface: { collectorSurface: "fixture" },
      semanticProjection: {
        semanticSurface: "fixture",
        version: 1,
        quality: "complete",
        reasonCodes: [],
        proofMode: "inspection",
        proofAllowed: true,
      },
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

  test("partial semantic projections remain typed blocks and cannot claim pass", () => {
    const candidate = {
      schemaVersion: 2,
      tool: "script-kit-devtools.elements",
      command: "elements.snapshot",
      classification: "blocked-by-unsupported-projection",
      requestedTarget: { selector: { type: "main" } },
      target: { automationId: "main" },
      semanticSurface: { collectorSurface: "about" },
      semanticProjection: {
        semanticSurface: "about",
        version: 1,
        quality: "partial",
        reasonCodes: ["collectorUnavailable"],
        proofMode: "action",
        proofAllowed: false,
      },
      nodes: [{ semanticId: "panel:about" }],
      duplicateSemanticIds: [],
      transaction: proofTransaction(),
      missingPrimitives: ["completeSemanticProjection"],
      errors: [],
    };
    const blocked = prepareValidatedReceipt("devtools.elements.snapshot", candidate);
    expect(blocked.receipt.disposition).toBe("BLOCKED_UNSUPPORTED_PROJECTION");
    expect(blocked.exitCode).toBe(3);

    const falsePass = prepareValidatedReceipt("devtools.elements.snapshot", {
      ...candidate,
      classification: "ok",
      missingPrimitives: [],
    });
    expect(falsePass.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(falsePass.exitCode).toBe(4);
  });

  test("semantic action, focus, AX, and privacy proof cannot be asserted without observed evidence", () => {
    const base = {
      schemaVersion: 2,
      tool: "script-kit-devtools.elements",
      command: "elements.snapshot",
      classification: "ok",
      requestedTarget: { selector: { type: "main" } },
      target: { automationId: "main" },
      semanticSurface: { collectorSurface: "fixture" },
      semanticProjection: {
        semanticSurface: "fixture",
        version: 1,
        quality: "complete",
        reasonCodes: [],
        proofMode: "inspection",
        proofAllowed: true,
      },
      nodes: [{ semanticId: "node:one", focused: false, activatable: false }],
      duplicateSemanticIds: [],
      transaction: proofTransaction(),
      missingPrimitives: [],
      errors: [],
    };
    for (const [mode, expected] of [
      ["action", "enabled activatable semantic node"],
      ["focus", "exactly one focused semantic node"],
      ["ax", "independently observed native accessibility peers"],
    ]) {
      const invalid = validateReceipt("devtools.elements.snapshot", {
        ...base,
        semanticProjection: { ...base.semanticProjection, proofMode: mode },
      });
      expect(invalid.valid).toBe(false);
      expect(invalid.errors.some((error) => error.includes(expected))).toBe(true);
    }
    const leaked = validateReceipt("devtools.elements.snapshot", {
      ...base,
      privacyViolationSemanticIds: ["node:one"],
    });
    expect(leaked.valid).toBe(false);
    expect(leaked.errors.some((error) => error.includes("privacy descriptors"))).toBe(true);
  });

  test("AX receipts independently verify visible native peers and the one real focused owner", () => {
    const labelSha256 = "b".repeat(64);
    const peer = {
      semanticId: "footer-action:run",
      action: "run",
      enabled: true,
      disabledReason: null,
      expectedSelector: "runFooterAction:",
      axPeer: {
        structuralId: "native-footer-run",
        accessibilityIdentifier: "footer-action:run",
        role: "AXButton",
        labelSha256,
        labelLength: 3,
        enabled: true,
        focused: false,
        accessibilityElement: true,
        hidden: false,
        alpha: 1,
        actionSelector: "runFooterAction:",
        bounds: { x: 10, y: 10, width: 80, height: 32 },
      },
      errors: [],
      parityPass: true,
    };
    const parity = {
      semanticButtonCount: 1,
      axNodeCount: 1,
      peerCount: 1,
      duplicateAxIds: [],
      duplicateAxStructuralIds: [],
      duplicateSemanticIds: [],
      peers: [peer],
      complete: true,
    };
    const graph = {
      nodes: [{ semanticId: "input:search", previous: null, next: null }],
      reciprocal: true,
      duplicateSemanticIds: [],
      hiddenFocusableIds: [],
      focusedSemanticIds: ["input:search"],
    };
    const candidate = {
      schemaVersion: 2,
      tool: "script-kit-devtools.focus",
      command: "focus.inspect",
      classification: "ok",
      proofMode: "ax",
      requestedTarget: { selector: { type: "main" } },
      target: { automationId: "main", bounds: { x: 0, y: 0, width: 800, height: 600 } },
      transaction: proofTransaction(),
      windowFocused: true,
      focusedSemanticId: "input:search",
      keyboardOwner: { surfaceKind: "ScriptList" },
      semanticProjection: { quality: "complete", proofAllowed: true },
      nativeFooter: { axParity: parity },
      focusGraph: graph,
      missingPrimitives: [],
      errors: [],
    };
    expect(prepareValidatedReceipt("devtools.focus.inspect", candidate).exitCode).toBe(0);

    for (const [name, axParity, focusGraph] of [
      ["missing peer", { ...parity, peers: [] }, graph],
      ["forged count", { ...parity, peerCount: 2 }, graph],
      ["hidden peer", { ...parity, peers: [{ ...peer, axPeer: { ...peer.axPeer, hidden: true } }] }, graph],
      ["nonfinite alpha", { ...parity, peers: [{ ...peer, axPeer: { ...peer.axPeer, alpha: NaN } }] }, graph],
      ["zero geometry", { ...parity, peers: [{ ...peer, axPeer: { ...peer.axPeer, bounds: { ...peer.axPeer.bounds, width: 0 } } }] }, graph],
      ["forged selector", { ...parity, peers: [{ ...peer, axPeer: { ...peer.axPeer, actionSelector: "deleteEverything:" } }] }, graph],
      ["duplicate native owners", { ...parity, duplicateAxStructuralIds: ["native-footer-run"] }, graph],
      ["empty graph", parity, { ...graph, nodes: [], focusedSemanticIds: [] }],
      ["missing focused owner", parity, { ...graph, focusedSemanticIds: [] }],
      ["wrong focused owner", parity, { ...graph, focusedSemanticIds: ["input:other"] }],
      ["forged graph edges", parity, { ...graph, nodes: [{ ...graph.nodes[0], next: "input:other" }] }],
    ] as Array<[string, Record<string, unknown>, Record<string, unknown>]>) {
      const prepared = prepareValidatedReceipt("devtools.focus.inspect", {
        ...candidate,
        nativeFooter: { axParity },
        focusGraph,
      });
      expect(prepared.exitCode, name).not.toBe(0);
      expect(prepared.receipt.disposition, name).toBe("INVALID_SCHEMA");
    }
  });

  test("scroll receipts independently recompute selected-row identity, clip geometry, and generations", () => {
    const rowBounds = { x: 0, y: 20, width: 100, height: 20 };
    const viewportBounds = { x: 0, y: 0, width: 100, height: 100 };
    const rendered = {
      required: true,
      classification: "ok",
      selectedSemanticId: "row:selected",
      rowMeasurementId: "layout:row:selected",
      safeViewportMeasurementId: "layout:main-view-main",
      rowObservationCount: 1,
      safeViewportObservationCount: 1,
      rowBounds,
      rowVisibleBounds: rowBounds,
      rowClipBounds: rowBounds,
      safeViewportBounds: viewportBounds,
      safeViewportClipBounds: viewportBounds,
      safeViewportPaintBounds: viewportBounds,
      coordinateSpace: "window",
      visibleRatio: 1,
      withinSafeViewport: true,
      frameGeneration: 8,
      viewportFrameGeneration: 8,
      frameMatches: true,
      targetDataGeneration: 1,
      missingPrimitives: [],
    };
    const candidate = {
      schemaVersion: 2,
      tool: "script-kit-devtools.scroll",
      command: "scroll.inspect",
      classification: "ok",
      requestedTarget: { selector: { type: "main" } },
      target: { automationId: "main", bounds: { x: 0, y: 0, width: 800, height: 600 } },
      transaction: proofTransaction(),
      scroll: { selectedSemanticId: "row:selected", selectedRowWithinSafeViewport: true },
      resizePressure: { selectedRowOutsideSafeViewport: false },
      renderedSafeViewport: rendered,
      missingPrimitives: [],
      errors: [],
    };
    expect(prepareValidatedReceipt("devtools.scroll.inspect", candidate).exitCode).toBe(0);

    for (const [name, forged] of [
      ["missing row", { ...rendered, rowObservationCount: 0 }],
      ["duplicate row", { ...rendered, rowObservationCount: 2 }],
      ["duplicate viewport", { ...rendered, safeViewportObservationCount: 2 }],
      ["wrong selected owner", { ...rendered, selectedSemanticId: "row:other" }],
      ["zero-area row", { ...rendered, rowBounds: { ...rowBounds, width: 0 } }],
      ["negative viewport", { ...rendered, safeViewportBounds: { ...viewportBounds, height: -1 } }],
      ["hidden row clip", { ...rendered, rowClipBounds: { ...rowBounds, width: 99 } }],
      ["hidden viewport clip", { ...rendered, safeViewportClipBounds: { ...viewportBounds, width: 99 } }],
      ["forged visible ratio", { ...rendered, visibleRatio: 0.5 }],
      ["missing coordinate space", { ...rendered, coordinateSpace: null }],
      ["fractional frame", { ...rendered, frameGeneration: 8.5, viewportFrameGeneration: 8.5 }],
      ["mismatched frame", { ...rendered, viewportFrameGeneration: 9 }],
      ["negative generation", { ...rendered, targetDataGeneration: -1 }],
      ["wrong transaction generation", { ...rendered, targetDataGeneration: 2 }],
      ["invented missing primitive", { ...rendered, missingPrimitives: ["paint"] }],
    ] as Array<[string, Record<string, unknown>]>) {
      const prepared = prepareValidatedReceipt("devtools.scroll.inspect", {
        ...candidate,
        renderedSafeViewport: forged,
      });
      expect(prepared.exitCode, name).not.toBe(0);
      expect(["INVALID_SCHEMA", "INVALID_GENERATION"], name)
        .toContain(prepared.receipt.disposition);
    }
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

  test("undeclared password and provider-token receipt fields fail closed without leaking bytes", () => {
    const secrets = ["unregistered-private-password", "sk-proj-private-provider-token"];
    const prepared = prepareValidatedReceipt(
      "devtools.layout.measure",
      baseReceipt({ credentials: { password: secrets[0], arbitrary: secrets[1] } }),
    );

    expect(prepared.receipt.disposition).toBe("INVALID_PRIVACY");
    expect(prepared.exitCode).toBe(4);
    const serialized = JSON.stringify(prepared.receipt);
    for (const value of secrets) expect(serialized).not.toContain(value);
    expect(serialized).toContain("unclassified sensitive fields");
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
    expect(prepared.receipt.runId).toBe("receipt-schema-test");
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

  test("join proof cannot hide rendered clipping behind a clean model", () => {
    const prepared = prepareValidatedReceipt("devtools.layout.measure", baseReceipt({
      proofMode: "join",
      truthLayers: {
        model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        rendered: { nodeCount: 1, clippedNodeCount: 1, overlapCount: 0 },
        joins: [{ comparability: "Comparable", classification: "Clipped" }],
        comparableJoinCount: 1,
        unjoinedMeasurementIds: [],
      },
    }));
    expect(prepared.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(prepared.exitCode).toBe(4);
  });

  test("join proof cannot hide model overlap or model-paint drift behind clean paint", () => {
    const modelOverlap = prepareValidatedReceipt("devtools.layout.measure", baseReceipt({
      proofMode: "join",
      truthLayers: {
        model: { nodeCount: 2, clippedNodeCount: 0, overlapCount: 1 },
        rendered: { nodeCount: 2, clippedNodeCount: 0, overlapCount: 0 },
        joins: [{ comparability: "Comparable", classification: "Match" }],
        comparableJoinCount: 1,
        unjoinedMeasurementIds: [],
      },
    }));
    const drift = prepareValidatedReceipt("devtools.layout.measure", baseReceipt({
      proofMode: "join",
      truthLayers: {
        model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        rendered: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
        joins: [{ comparability: "Comparable", classification: "OutOfTolerance" }],
        comparableJoinCount: 1,
        unjoinedMeasurementIds: [],
      },
    }));
    expect(modelOverlap.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(drift.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(modelOverlap.exitCode).toBe(4);
    expect(drift.exitCode).toBe(4);
  });

  test("join proof derives comparable counts and rejects duplicate, invalid, or incomplete identities", () => {
    const bounds = { x: 0, y: 0, width: 100, height: 20 };
    const validJoin = {
      measurementId: "layout:row",
      semanticId: "row:1",
      role: "rowSlot",
      coordinateSpace: "window",
      comparability: "Comparable",
      classification: "Match",
      model: { bounds, generation: 7 },
      rendered: {
        bounds,
        visibleBounds: bounds,
        clipBounds: bounds,
        frameGeneration: 7,
        source: "paint-time",
      },
      delta: { x: 0, y: 0, width: 0, height: 0 },
      tolerance: { x: 1, y: 1, width: 1, height: 1 },
    };
    const truth = {
      model: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
      rendered: { nodeCount: 1, clippedNodeCount: 0, overlapCount: 0 },
      joins: [validJoin],
      comparableJoinCount: 1,
      unjoinedMeasurementIds: [],
    };

    expect(prepareValidatedReceipt("devtools.layout.measure", baseReceipt({
      proofMode: "join",
      truthLayers: truth,
    })).exitCode).toBe(0);

    for (const forged of [
      { ...truth, joins: [], comparableJoinCount: 1 },
      { ...truth, comparableJoinCount: 2 },
      { ...truth, joins: [validJoin, { ...validJoin, comparability: "DuplicateMeasurement" }] },
      { ...truth, joins: [{ ...validJoin, semanticId: null }] },
      { ...truth, joins: [{ ...validJoin, coordinateSpace: "unknown" }] },
      { ...truth, joins: [{ ...validJoin, delta: { ...validJoin.delta, x: NaN } }] },
      { ...truth, joins: [{ ...validJoin, rendered: {
        ...validJoin.rendered,
        visibleBounds: { ...bounds, width: 99 },
      } }] },
      { ...truth, joins: [{ ...validJoin, rendered: {
        ...validJoin.rendered,
        frameGeneration: 8,
      } }] },
    ]) {
      expect(prepareValidatedReceipt("devtools.layout.measure", baseReceipt({
        proofMode: "join",
        truthLayers: forged,
      })).exitCode).not.toBe(0);
    }
  });

  test("fit proof requires font-ready same-frame unoccluded glyphs", () => {
    const candidate = {
      schemaVersion: 2,
      tool: "script-kit-devtools.text",
      command: "text.measure",
      classification: "ok",
      proofMode: "fit",
      requestedTarget: { selector: { type: "main" } },
      target: { automationId: "main", bounds: { x: 0, y: 0, width: 800, height: 600 } },
      transaction: proofTransaction(),
      textSummary: { inputLength: 7, inputFingerprint: "fixture" },
      rows: [{ semanticId: "input:notes-editor", textLength: 7, fingerprint: "fixture" }],
      textFits: [{
        measurementId: "text:notes-editor:line:0:0",
        semanticId: "input:notes-editor",
        role: "textLineBox",
        lineBoxBounds: { x: 0, y: 0, width: 100, height: 20 },
        glyphBounds: { x: 0, y: 0, width: 80, height: 16 },
        clipBounds: { x: 0, y: 0, width: 100, height: 20 },
        visibleBounds: { x: 0, y: 0, width: 100, height: 20 },
        visibleRatio: 1,
        truncationPolicy: "fullDisplay",
        occluderMeasurementIds: [],
        fontFamilyFingerprint: "fixture-font",
        fontSize: 14,
        lineHeight: 20,
        backingScaleFactor: 2,
        fontsReady: true,
        contentFingerprint: "fixture-content",
        graphemeCount: 7,
        geometryValid: true,
        measurementIdentityValid: true,
        paintOrderValid: true,
        fullDisplayPass: true,
        rawContentReturned: false,
        frameMatches: true,
        backingScaleMatches: true,
      }],
      missingPrimitives: [],
      warnings: [],
      errors: [],
    };
    const valid = prepareValidatedReceipt("devtools.text.measure", candidate);
    expect(valid.receipt.disposition).toBe("EVALUABLE_PASS");

    const invalid = prepareValidatedReceipt("devtools.text.measure", {
      ...candidate,
      textFits: [{
        fullDisplayPass: false,
        rawContentReturned: false,
        frameMatches: false,
      }],
    });
    expect(invalid.receipt.disposition).toBe("INVALID_SCHEMA");
    expect(invalid.exitCode).toBe(4);
  });

  test("fit proof cannot trust forged passing booleans over missing or private glyph evidence", () => {
    const bounds = { x: 0, y: 0, width: 100, height: 20 };
    const fit = {
      measurementId: "text:row",
      semanticId: "input:notes-editor",
      role: "textLineBox",
      lineBoxBounds: bounds,
      glyphBounds: { x: 0, y: 0, width: 80, height: 16 },
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
      graphemeCount: 7,
      geometryValid: true,
      measurementIdentityValid: true,
      paintOrderValid: true,
      fullDisplayPass: true,
      rawContentReturned: false,
      frameMatches: true,
      backingScaleMatches: true,
    };
    const candidate = {
      schemaVersion: 2,
      tool: "script-kit-devtools.text",
      command: "text.measure",
      classification: "ok",
      proofMode: "fit",
      requestedTarget: { selector: { type: "main" } },
      target: { automationId: "main", bounds: { x: 0, y: 0, width: 800, height: 600 } },
      transaction: proofTransaction(),
      textSummary: { inputLength: 7, inputFingerprint: "fixture" },
      rows: [{ semanticId: "input:notes-editor", textLength: 7, fingerprint: "fixture" }],
      textFits: [fit],
      missingPrimitives: [],
      warnings: [],
      errors: [],
    };
    expect(prepareValidatedReceipt("devtools.text.measure", candidate).exitCode).toBe(0);
    for (const forged of [
      { ...fit, glyphBounds: { ...fit.glyphBounds, width: 0 } },
      { ...fit, visibleRatio: 0 },
      { ...fit, occluderMeasurementIds: ["overlay"] },
      { ...fit, fontsReady: false },
      { ...fit, fontFamilyFingerprint: null },
      { ...fit, contentFingerprint: null },
      { ...fit, geometryValid: false },
      { ...fit, measurementIdentityValid: false },
      { ...fit, paintOrderValid: false },
      { ...fit, rawContentReturned: true },
    ]) {
      expect(prepareValidatedReceipt("devtools.text.measure", {
        ...candidate,
        textFits: [forged],
      }).exitCode).not.toBe(0);
    }
  });
});
