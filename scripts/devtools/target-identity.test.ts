import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  compareWindowLifetimeSnapshots,
  hiddenTargetInspectionSnapshot,
  pickWindows,
  proofTransactionIdentity,
  resolveTargetReceipt,
  stableWindowInstanceId,
  strictTransactionMissingFields,
  targetIdentity,
} from "./lib/target-identity.ts";

const temporaryRoots: string[] = [];
const originalSessionRoot = process.env.SCRIPT_KIT_SESSION_DIR;

afterEach(() => {
  for (const root of temporaryRoots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
  if (originalSessionRoot === undefined) {
    delete process.env.SCRIPT_KIT_SESSION_DIR;
  } else {
    process.env.SCRIPT_KIT_SESSION_DIR = originalSessionRoot;
  }
});

describe("proof transaction identity", () => {
  test("window identity combines a stable automation id with its lifetime generation", () => {
    expect(stableWindowInstanceId("main", 7)).toBe("main@7");
    expect(stableWindowInstanceId("main", null)).toBeNull();
  });

  test("target identity preserves window and parent instances", () => {
    const windows = {
      windows: [
        {
          id: "main",
          kind: "main",
          generation: 11,
          pid: process.pid,
          visible: true,
          focused: true,
          bounds: { x: 0, y: 0, width: 800, height: 600 },
        },
        {
          id: "actions-dialog",
          kind: "actionsDialog",
          generation: 12,
          parentWindowId: "main",
          pid: process.pid,
          visible: true,
          focused: false,
          bounds: { x: 40, y: 40, width: 640, height: 480 },
          semanticSurface: "ActionsDialog",
        },
      ],
    };
    const identity = targetIdentity(
      {
        target: { type: "id", id: "actions-dialog" },
        strict: true,
        expectedSurfaceKind: "ActionsDialog",
      },
      {
        windowId: "actions-dialog",
        windowKind: "ActionsDialog",
        semanticSurface: "ActionsDialog",
        targetGeneration: 4,
        surfaceGeneration: 5,
        dataGeneration: 6,
        resolvedBounds: { x: 40, y: 40, width: 640, height: 480 },
        pid: process.pid,
      },
      windows,
    );
    expect(identity.resolvedTarget.windowInstanceId).toBe("actions-dialog@12");
    expect(identity.resolvedTarget.parentWindowInstanceId).toBe("main@11");
    expect(identity.resolvedTarget.strictTargetMatch).toBe(true);
  });

  test("lifetime comparison rejects reopen and geometry drift", () => {
    const before = {
      windows: [{
        id: "notes",
        kind: "notes",
        generation: 3,
        pid: 10,
        bounds: { x: 0, y: 0, width: 600, height: 500 },
      }],
    };
    expect(compareWindowLifetimeSnapshots("notes", before, before).consistent).toBe(true);

    const reopened = {
      windows: [{
        id: "notes",
        kind: "notes",
        generation: 4,
        pid: 10,
        bounds: { x: 0, y: 0, width: 600, height: 500 },
      }],
    };
    const result = compareWindowLifetimeSnapshots("notes", before, reopened);
    expect(result.consistent).toBe(false);
    expect(result.errors).toContain("window instance changed during target inspection");
  });

  test("complete transactions bind process, binary, target, surface, and generations", () => {
    const root = mkdtempSync(join(tmpdir(), "pf002-target-identity-"));
    temporaryRoots.push(root);
    process.env.SCRIPT_KIT_SESSION_DIR = root;
    const binary = join(root, "fixture-binary");
    writeFileSync(binary, "fixture-binary-bytes");
    const session = "transaction-test";
    mkdirSync(join(root, session), { recursive: true });
    writeFileSync(join(root, session, "binary"), `${binary}\n`);

    const transaction = proofTransactionIdentity(session, {
      automationId: "main",
      windowInstanceId: "main@9",
      windowGeneration: 9,
      windowKind: "Main",
      surfaceKind: "ScriptList",
      semanticSurface: "scriptList",
      appViewVariant: "ScriptList",
      targetGeneration: 2,
      surfaceGeneration: 3,
      dataGeneration: 4,
      bounds: { x: 0, y: 0, width: 800, height: 600 },
      pid: process.pid,
    });

    expect(strictTransactionMissingFields(transaction)).toEqual([]);
    expect(transaction.binarySha256).toHaveLength(64);
    expect(transaction.processStartTime).toBeTruthy();
    expect(transaction.windowInstanceId).toBe("main@9");
  });
});

describe("capture-free hidden target resolution", () => {
  function hiddenWindow(overrides: Record<string, unknown> = {}) {
    return {
      id: "main",
      kind: "main",
      generation: 9,
      pid: process.pid,
      visible: false,
      focused: false,
      semanticSurface: "scriptList",
      bounds: { x: 0, y: 0, width: 800, height: 600 },
      ...overrides,
    };
  }

  function hiddenState(overrides: Record<string, unknown> = {}) {
    return {
      windowVisible: false,
      isFocused: false,
      surfaceContract: {
        surfaceKind: "ScriptList",
        automationSemanticSurface: "scriptList",
        targetIdentity: {
          windowId: "main",
          windowGeneration: 9,
          appViewVariant: "ScriptList",
          targetGeneration: 2,
          surfaceGeneration: 3,
          dataGeneration: 4,
        },
      },
      ...overrides,
    };
  }

  function fixtureSession(): string {
    const root = mkdtempSync(join(tmpdir(), "hidden-target-identity-"));
    temporaryRoots.push(root);
    process.env.SCRIPT_KIT_SESSION_DIR = root;
    const binary = join(root, "fixture-binary");
    writeFileSync(binary, "fixture-binary-bytes");
    const session = "hidden-target-test";
    mkdirSync(join(root, session), { recursive: true });
    writeFileSync(join(root, session, "binary"), `${binary}\n`);
    return session;
  }

  test("hidden state supplies canonical identity without pixel inspection", async () => {
    const session = fixtureSession();
    const commands: string[] = [];
    const receipt = await resolveTargetReceipt(
      {
        session,
        target: { type: "main" },
        strict: true,
        expectedSurfaceKind: "ScriptList",
        timeoutMs: 250,
      },
      {
        noninteractive: true,
        rpcFn: async (_session, payload) => {
          commands.push(String(payload.type));
          if (payload.type === "listAutomationWindows") {
            return { response: { windows: [hiddenWindow()] } };
          }
          if (payload.type === "getState") {
            return { response: hiddenState() };
          }
          throw new Error(`unreviewed RPC: ${String(payload.type)}`);
        },
      },
    );

    expect(commands).toEqual([
      "listAutomationWindows",
      "getState",
      "listAutomationWindows",
    ]);
    expect(commands).not.toContain("inspectAutomationWindow");
    expect(receipt.classification).toBe("ok");
    expect(receipt.inspectionMode).toBe("capture-free-hidden-state");
    expect(receipt.resolvedTarget.visible).toBe(false);
    expect(receipt.transaction.windowInstanceId).toBe("main@9");
    expect(receipt.transaction.appViewVariant).toBe("ScriptList");
    expect(receipt.transaction.targetGeneration).toBe(2);
    expect(receipt.transaction.surfaceGeneration).toBe(3);
    expect(receipt.transaction.dataGeneration).toBe(4);
    expect(receipt.transactionValidation.valid).toBe(true);
  });

  test("missing canonical generations remain blocked rather than invented", async () => {
    const receipt = await resolveTargetReceipt(
      {
        session: fixtureSession(),
        target: { type: "main" },
        strict: true,
        expectedSurfaceKind: "ScriptList",
        timeoutMs: 250,
      },
      {
        noninteractive: true,
        rpcFn: async (_session, payload) =>
          payload.type === "listAutomationWindows"
            ? { response: { windows: [hiddenWindow()] } }
            : {
                response: hiddenState({
                  surfaceContract: {
                    surfaceKind: "ScriptList",
                    automationSemanticSurface: "scriptList",
                  },
                }),
              },
      },
    );

    expect(receipt.classification).toBe("blocked-by-missing-primitive");
    expect(receipt.transaction.targetGeneration).toBeNull();
    expect(receipt.transaction.surfaceGeneration).toBeNull();
    expect(receipt.transaction.dataGeneration).toBeNull();
    expect(receipt.transactionValidation.missingFields).toEqual([
      "appViewVariant",
      "targetGeneration",
      "surfaceGeneration",
      "dataGeneration",
    ]);
  });

  test("top-level generation claims cannot replace missing canonical runtime authority", async () => {
    const counterfeit = hiddenState({
      surfaceContract: {
        surfaceKind: "ScriptList",
        automationSemanticSurface: "scriptList",
      },
      targetIdentity: {
        windowId: "main",
        windowGeneration: 9,
        appViewVariant: "ScriptList",
        targetGeneration: 20,
        surfaceGeneration: 30,
        dataGeneration: 40,
        layoutGeneration: 50,
        selectionGeneration: 60,
        scrollGeneration: 70,
        frameGeneration: 80,
      },
      targetGeneration: 200,
      surfaceGeneration: 300,
      dataGeneration: 400,
    });
    const listed = pickWindows({ windows: [hiddenWindow()] })[0]!;
    const snapshot = hiddenTargetInspectionSnapshot(listed, counterfeit);

    for (const field of [
      "targetGeneration",
      "surfaceGeneration",
      "dataGeneration",
      "layoutGeneration",
      "selectionGeneration",
      "scrollGeneration",
      "frameGeneration",
    ]) {
      expect(snapshot[field], field).toBeNull();
    }

    const receipt = await resolveTargetReceipt(
      {
        session: fixtureSession(),
        target: { type: "main" },
        strict: true,
        expectedSurfaceKind: "ScriptList",
        timeoutMs: 250,
      },
      {
        noninteractive: true,
        rpcFn: async (_session, payload) =>
          payload.type === "listAutomationWindows"
            ? { response: { windows: [hiddenWindow()] } }
            : { response: counterfeit },
      },
    );

    expect(receipt.classification).toBe("blocked-by-missing-primitive");
    expect(receipt.transaction.targetGeneration).toBeNull();
    expect(receipt.transaction.surfaceGeneration).toBeNull();
    expect(receipt.transaction.dataGeneration).toBeNull();
    expect(receipt.transactionValidation.missingFields).toEqual([
      "targetGeneration",
      "surfaceGeneration",
      "dataGeneration",
    ]);
  });

  test("conflicting fallback target identity cannot borrow another window or lifetime", () => {
    const listed = pickWindows({ windows: [hiddenWindow()] })[0]!;

    for (const [location, source, expected] of [
      [
        "state.targetIdentity.windowId",
        { targetIdentity: { windowId: "other-window" } },
        "different automation window",
      ],
      [
        "state.targetIdentity.windowGeneration",
        { targetIdentity: { windowId: "main", windowGeneration: 10 } },
        "stale automation window generation",
      ],
      [
        "state.windowId",
        { windowId: "other-window" },
        "different automation window",
      ],
      [
        "state.windowGeneration",
        { windowGeneration: 10 },
        "stale automation window generation",
      ],
    ] as Array<[string, Record<string, unknown>, string]>) {
      expect(() =>
        hiddenTargetInspectionSnapshot(listed, hiddenState(source)),
      ).toThrow(expected);
      expect(() =>
        hiddenTargetInspectionSnapshot(listed, hiddenState(source)),
      ).toThrow(location.replace(/\.(windowId|windowGeneration)$/, ""));
    }
  });

  test("secondary generation data cannot conflict with canonical hidden target proof", () => {
    const listed = pickWindows({ windows: [hiddenWindow()] })[0]!;

    for (const field of ["targetGeneration", "surfaceGeneration", "dataGeneration"]) {
      for (const fallback of [
        { targetIdentity: { [field]: 999 } },
        { [field]: 999 },
      ]) {
        expect(() =>
          hiddenTargetInspectionSnapshot(listed, hiddenState(fallback)),
        ).toThrow(`${field} conflicts with the canonical target identity`);
      }
    }

    const agreeing = hiddenTargetInspectionSnapshot(
      listed,
      hiddenState({
        targetIdentity: {
          windowId: "main",
          windowGeneration: 9,
          targetGeneration: 2,
          surfaceGeneration: 3,
          dataGeneration: 4,
        },
        targetGeneration: 2,
      }),
    );
    expect(agreeing.targetGeneration).toBe(2);
    expect(agreeing.surfaceGeneration).toBe(3);
    expect(agreeing.dataGeneration).toBe(4);
  });

  test("visible registry targets are rejected before querying their state", async () => {
    const commands: string[] = [];
    expect(
      resolveTargetReceipt(
        {
          session: "never-attach",
          target: { type: "main" },
          strict: true,
          expectedSurfaceKind: "ScriptList",
          timeoutMs: 250,
        },
        {
          noninteractive: true,
          rpcFn: async (_session, payload) => {
            commands.push(String(payload.type));
            return { response: { windows: [hiddenWindow({ visible: true })] } };
          },
        },
      ),
    ).rejects.toThrow("target is visible or its visibility is unknown");
    expect(commands).toEqual(["listAutomationWindows"]);
  });

  test("focused selectors fail before issuing any protocol request", async () => {
    const commands: string[] = [];
    await expect(
      resolveTargetReceipt(
        {
          session: "never-attach",
          target: { type: "focused" },
          strict: true,
          expectedSurfaceKind: "ScriptList",
          timeoutMs: 250,
        },
        {
          noninteractive: true,
          rpcFn: async (_session, payload) => {
            commands.push(String(payload.type));
            return {};
          },
        },
      ),
    ).rejects.toThrow("focused-window selectors");
    expect(commands).toEqual([]);
  });

  test("both independent hidden observations are mandatory", () => {
    const listed = pickWindows({ windows: [hiddenWindow()] })[0]!;
    expect(() =>
      hiddenTargetInspectionSnapshot(listed, hiddenState({ windowVisible: true })),
    ).toThrow("both the automation registry and state response");
  });

  test("canonical hidden-state identity cannot belong to another target or lifetime", () => {
    const listed = pickWindows({ windows: [hiddenWindow()] })[0]!;
    const current = hiddenState();
    const contract = current.surfaceContract as Record<string, any>;
    const identity = contract.targetIdentity as Record<string, unknown>;

    expect(() =>
      hiddenTargetInspectionSnapshot(listed, {
        ...current,
        surfaceContract: {
          ...contract,
          targetIdentity: { ...identity, windowId: "other-window" },
        },
      }),
    ).toThrow("different automation window");

    expect(() =>
      hiddenTargetInspectionSnapshot(listed, {
        ...current,
        surfaceContract: {
          ...contract,
          targetIdentity: { ...identity, windowGeneration: 10 },
        },
      }),
    ).toThrow("stale automation window generation");
  });
});
