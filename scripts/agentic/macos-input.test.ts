import { createHash } from "node:crypto";
import { readFileSync, realpathSync } from "node:fs";
import { describe, expect, test } from "bun:test";
import {
  evaluateDeliveryEvidence,
  evaluateNonactivation,
  evaluatePassiveKeyboardReadiness,
  parseJsonDocuments,
  planInputRoute,
  planNativeKey,
  resultFor,
  selectExactRpcEnvelope,
  selectExactlyOne,
  SOURCE_PROVENANCE,
  SYSTEM_EVENTS_KEY_CODES,
} from "./macos-input";

describe("complete JSON document framing", () => {
  test("parses pretty multiline JSON", () => {
    expect(parseJsonDocuments('{\n  "status": "ok",\n  "data": [1, 2]\n}')).toEqual([{ status: "ok", data: [1, 2] }]);
  });

  test("parses a pretty document after diagnostic text", () => {
    expect(parseJsonDocuments('diagnostic prefix\n{\n  "status": "ok",\n  "quoted": "} \\\" ["\n}\ntrailer')).toEqual([
      { status: "ok", quoted: '} " [' },
    ]);
  });

  test("parses multiple documents and requires predicate selection", () => {
    const documents = parseJsonDocuments('note\n{"id":"a"}\n[1,{"id":"nested"}]\n{"id":"b"}');
    expect(documents).toHaveLength(3);
    expect(selectExactlyOne<any>(documents, (document) => document?.id === "b", "id")).toEqual({ id: "b" });
  });

  test("rejects a truncated final document", () => {
    expect(() => parseJsonDocuments('{"ok":true}\n{"truncated":')).toThrow("truncated_json_document");
  });

  test("rejects empty output", () => {
    expect(() => parseJsonDocuments(" \n ")).toThrow("empty_json_output");
  });
});

describe("exact session RPC correlation", () => {
  const requestId = "r-1";
  const expectedType = "stateResult";
  const session = "s-1";
  const exact = {
    status: "ok",
    session,
    requestId,
    responseType: expectedType,
    response: { requestId, type: expectedType, windowVisible: true },
  };

  test("accepts only exact outer and inner correlation", () => {
    const selected = selectExactRpcEnvelope([exact], session, requestId, expectedType);
    expect(selected.response.windowVisible).toBe(true);
    expect(selected.correlation).toEqual({
      sessionExact: true,
      outerRequestIdExact: true,
      outerResponseTypeExact: true,
      innerRequestIdExact: true,
      innerTypeExact: true,
      exact: true,
    });
  });

  test("rejects duplicate exact envelopes", () => {
    expect(() => selectExactRpcEnvelope([exact, structuredClone(exact)], session, requestId, expectedType)).toThrow("ambiguous_exact_match");
  });

  test.each([
    ["status", { status: undefined }],
    ["session", { session: undefined }],
    ["outer requestId", { requestId: undefined }],
    ["outer responseType", { responseType: undefined }],
    ["inner requestId", { response: { ...exact.response, requestId: undefined } }],
    ["inner type", { response: { ...exact.response, type: undefined } }],
    ["wrong session", { session: "other" }],
    ["wrong outer requestId", { requestId: "r-10" }],
    ["wrong outer responseType", { responseType: "elementsResult" }],
    ["wrong inner requestId", { response: { ...exact.response, requestId: "r-10" } }],
    ["wrong inner type", { response: { ...exact.response, type: "elementsResult" } }],
  ])("rejects missing or mismatched %s", (_label, change) => {
    expect(() => selectExactRpcEnvelope([{ ...exact, ...change }], session, requestId, expectedType)).toThrow("no_exact_match");
  });
});

describe("native System Events key planning and receipts", () => {
  test.each([["Escape", 53], ["Delete", 51], ["Down", 125]] as const)("keeps %s keyCode in the plan and receipt", (key, keyCode) => {
    const plan = planNativeKey(key, []);
    expect(plan.kind).toBe("keyCode");
    expect(plan.actualMethod).toBe("native.systemEvents.keyCode");
    expect(plan.keyCode).toBe(keyCode);
    const result = resultFor(plan.actualMethod, "accessibility", { key, keyCode: plan.keyCode });
    expect(result.keyCode).toBe(keyCode);
    expect(result.receipt.keyCode).toBe(keyCode);
    expect(result.actualMethod).toBe("native.systemEvents.keyCode");
    expect(result.receipt.actualMethod).toBe("native.systemEvents.keyCode");
  });

  test("literal plans and type receipts use dotted method and null keyCode", () => {
    const plan = planNativeKey('"', ["cmd"]);
    expect(plan).toMatchObject({ kind: "keystroke", actualMethod: "native.systemEvents.keystroke", keyCode: null });
    const result = resultFor("native.systemEvents.keystroke", "accessibility", { text: "hello", keyCode: null });
    expect(result.keyCode).toBeNull();
    expect(result.receipt.keyCode).toBeNull();
    expect(result.actualMethod).toBe("native.systemEvents.keystroke");
  });

  test("the named table retains the complete special/function/keypad surface", () => {
    expect(Object.keys(SYSTEM_EVENTS_KEY_CODES).length).toBeGreaterThanOrEqual(75);
    for (const name of ["enter", "escape", "forwarddelete", "home", "end", "f1", "f20", "keypad0", "keypad9", "volumeup", "function"]) {
      expect(SYSTEM_EVENTS_KEY_CODES[name]).toBeNumber();
    }
  });

  test("unknown named keys fail hard", () => {
    expect(() => planNativeKey("definitely-not-a-key", [])).toThrow("Unknown key");
  });
});

describe("route-correct delivery evidence", () => {
  test("native evidence is injector-only with a 50 ms non-proof settle", () => {
    expect(resultFor("native.systemEvents.keyCode", "accessibility")).toMatchObject({
      injectorAccepted: true, ingressVerified: false, postconditionVerified: false,
      deliveryScope: "injector", delivered: true, settleMs: 50, settleIsProof: false,
    });
  });

  test("GPUI evidence is ingress-only with no settle", () => {
    expect(resultFor("protocol.simulateGpuiEvent.keyDown", "gpuiDispatch")).toMatchObject({
      injectorAccepted: false, ingressVerified: true, postconditionVerified: false,
      deliveryScope: "ingress", delivered: true, settleMs: 0, settleIsProof: false,
    });
  });

  test("verified batch evidence reaches postcondition scope", () => {
    expect(resultFor("protocol.batch.setInput", "directBatch", {}, [], true)).toMatchObject({
      injectorAccepted: false, ingressVerified: true, postconditionVerified: true,
      deliveryScope: "postcondition", delivered: true, settleMs: 0, settleIsProof: false,
    });
  });

  test("delivered is true for independently proved ingress/postcondition", () => {
    expect(evaluateDeliveryEvidence({ injectorAccepted: false, ingressVerified: true, postconditionVerified: false, deliveryScope: "ingress", settleMs: 0 }).delivered).toBe(true);
  });

  test("routing exposes exact dotted methods", () => {
    expect(planInputRoute("key", true, true, true)).toEqual(["native.systemEvents.keyCode", "native.systemEvents.keystroke"]);
    expect(planInputRoute("type", false, true, true).slice(0, 2)).toEqual(["protocol.batch.setInput", "protocol.simulateGpuiEvent.keyDown"]);
  });
});

describe("passive exact main-window keyboard readiness", () => {
  const ready = {
    expectedPid: 42, statusPid: 42, pidFilePid: 42,
    expectedGeneration: "g1", generationFile: "g1",
    requestedKind: "main", protocolTargetType: "main", surfaceId: "main", targetWindowId: 777,
    protocolRequestId: "r-1", protocolExpectedType: "stateResult", protocolExactCorrelation: true,
    windowVisible: true, protocolFocused: true, promptType: "none", surfaceKind: "ScriptList",
    automationSemanticSurface: "scriptList", inputOwnership: "LauncherFilter",
    focusPolicy: "LauncherFilterFocus", keyboardPolicy: "LauncherListKeyboard",
    axPid: null, axFocusedWindowPresent: false, axFocusedWindowId: null,
  };

  test("accepts an exact nonactivating key-panel contract with no AX focused window", () => {
    const evidence = evaluatePassiveKeyboardReadiness(ready);
    expect(evidence.ready).toBe(true);
    expect(evidence.failures).toEqual([]);
    expect(evidence.target).toMatchObject({ surfaceId: "main", windowId: 777, exact: true });
    expect(evidence.protocol).toMatchObject({ exactCorrelation: true, windowVisible: true, isFocused: true });
    expect(evidence.accessibility).toMatchObject({
      focusedWindowPresent: false,
      focusedWindowId: null,
      exactWindowMatch: false,
      requiredForReadiness: false,
    });
  });

  test("a different AX focused window is diagnostic-only", () => {
    const evidence = evaluatePassiveKeyboardReadiness({
      ...ready,
      axPid: 42,
      axFocusedWindowPresent: true,
      axFocusedWindowId: 778,
    });
    expect(evidence.ready).toBe(true);
    expect(evidence.accessibility.exactWindowMatch).toBe(false);
  });

  test("the exact Quartz main target remains mandatory", () => {
    const evidence = evaluatePassiveKeyboardReadiness({ ...ready, targetWindowId: null });
    expect(evidence.ready).toBe(false);
    expect(evidence.failures).toContain("strict_main_target_required");
  });

  test("protocol keyboard focus remains mandatory", () => {
    const evidence = evaluatePassiveKeyboardReadiness({ ...ready, protocolFocused: false });
    expect(evidence.ready).toBe(false);
    expect(evidence.failures).toContain("launcher_keyboard_policy_not_exact");
  });

  test("passive result fields never claim enforcement, activation, or focus mutation", () => {
    const result = resultFor("native.systemEvents.keyCode", "accessibility", {
      focusCheckRequested: true, focusVerified: true, focusEnforced: false,
      activationAttempted: false, focusMutationAttempted: false, focusVerificationMode: "passive",
      keyboardReadiness: evaluatePassiveKeyboardReadiness(ready),
    });
    expect(result).toMatchObject({ focusEnforced: false, activationAttempted: false, focusMutationAttempted: false, focusVerificationMode: "passive" });
  });
});

describe("independent nonactivation", () => {
  const terminal = { pid: 9, bundleId: "com.apple.Terminal", name: "Terminal" };

  test("passes for unchanged exact external identity", () => {
    expect(evaluateNonactivation(terminal, { ...terminal }, 42)).toMatchObject({ baselineIsExternal: true, unchanged: true, verified: true });
  });

  test("fails when target is frontmost at baseline", () => {
    expect(evaluateNonactivation({ ...terminal, pid: 42 }, { ...terminal, pid: 42 }, 42).verified).toBe(false);
  });

  test.each([
    ["pid", { ...terminal, pid: 10 }],
    ["bundle", { ...terminal, bundleId: "com.apple.finder" }],
  ])("fails when frontmost %s changes", (_label, after) => {
    expect(evaluateNonactivation(terminal, after, 42).verified).toBe(false);
  });
});

test("helper source provenance hashes the exact current source bytes", () => {
  const path = realpathSync(new URL("./macos-input.ts", import.meta.url).pathname);
  const sha256 = createHash("sha256").update(readFileSync(path)).digest("hex");
  expect(SOURCE_PROVENANCE).toEqual({ path, sha256 });
});
