import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  compareWindowLifetimeSnapshots,
  pickWindows,
  proofTransactionIdentity,
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
