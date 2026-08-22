import { createHash } from "node:crypto";

/**
 * scripts/devtools/lib/target-identity.ts — strict target identity resolution,
 * extracted from targets.ts so inspector CLIs (elements/focus/layout/keyboard/
 * text/scroll) resolve targets IN-PROCESS instead of re-invoking
 * `bun scripts/devtools/targets.ts inspect` as a subprocess per receipt.
 *
 * Before this module a single focus receipt cost ~5 process spawns (targets
 * subprocess with 2 RPCs + 2 own RPCs, each through session.sh). Now the
 * targets subprocess and its hand-maintained `forwarded[]` flag echo are gone.
 */

import {
  type JsonObject,
  type TargetArgs,
  asArray,
  binaryFingerprint,
  classifyEnvelopes,
  hasSessionLifecycleError,
  lifecycleCodes,
  primaryLifecycleDetails,
  primaryParsedError,
  primarySessionLifecycle,
  requestId,
  responseOf,
  rpc,
  run,
} from "./client.ts";
import { externalContent, productStatic } from "./privacy.ts";
import {
  assertNoninteractiveProtocolCommand,
  NoninteractiveSafetyError,
} from "./operator-safety.ts";

export function stableWindowKind(value: unknown) {
  if (value === "actionsDialog") return "ActionsDialog";
  if (value === "promptPopup") return "PromptPopup";
  if (value === "agentChatDetached") return "AgentChatDetached";
  if (value === "dictation") return "Dictation";
  if (value === "miniAi") return "MiniAi";
  if (value === "ai") return "Ai";
  if (value === "main") return "Main";
  if (value === "notes") return "Notes";
  if (value === "hud") return "Hud";
  return value ?? null;
}

function derivedHostKind(windowKind: unknown): string | null {
  const stable = stableWindowKind(windowKind);
  if (stable === "Main") return "mainWindow";
  if (stable === "ActionsDialog" || stable === "PromptPopup") return "attachedPopup";
  if (typeof stable === "string") return "detachedWindow";
  return null;
}

export function stableWindowInstanceId(
  automationId: unknown,
  generation: unknown,
): string | null {
  return typeof automationId === "string"
      && automationId.length > 0
      && typeof generation === "number"
    ? `${automationId}@${generation}`
    : null;
}

export function pickWindows(windows: JsonObject) {
  return asArray(windows.windows ?? windows.automationWindows ?? windows.targets).map((window, index) => {
    const automationId = window.id ?? window.windowId ?? window.automationId ?? null;
    const windowGeneration = window.generation ?? window.windowGeneration ?? null;
    return {
      index,
      automationId,
      windowInstanceId: stableWindowInstanceId(automationId, windowGeneration),
      windowGeneration,
      windowKind: stableWindowKind(window.kind ?? window.windowKind),
      title: externalContent(window.title ?? null),
      visible: window.visible ?? null,
      focused: window.focused ?? null,
      bounds: window.bounds ?? window.resolvedBounds ?? null,
      surfaceKind: window.surfaceKind ?? null,
      semanticSurface: window.semanticSurface ?? null,
      appViewVariant: window.appViewVariant ?? null,
      parentAutomationId: window.parentAutomationId ?? window.parentWindowId ?? null,
      parentKind: window.parentKind ?? null,
      pid: window.pid ?? null,
    };
  });
}

type SurfaceCandidate = { field: string; value: string };
type ActualSurfaceCandidate = {
  automationId: unknown;
  windowKind: unknown;
  surfaceKind: unknown;
  semanticSurface: unknown;
  appViewVariant: unknown;
};

function surfaceCandidates(snapshot: JsonObject, listedWindow: JsonObject): SurfaceCandidate[] {
  return [
    ["snapshot.windowKind", snapshot.windowKind],
    ["snapshot.kind", snapshot.kind],
    ["snapshot.surfaceKind", snapshot.surfaceKind],
    ["snapshot.semanticSurface", snapshot.semanticSurface],
    ["snapshot.appViewVariant", snapshot.appViewVariant],
    ["snapshot.surfaceContract.surfaceKind", (snapshot.surfaceContract as JsonObject | undefined)?.surfaceKind],
    ["snapshot.state.surfaceKind", (snapshot.state as JsonObject | undefined)?.surfaceKind],
    ["listedWindow.windowKind", listedWindow.windowKind],
    ["listedWindow.semanticSurface", listedWindow.semanticSurface],
    ["listedWindow.surfaceKind", listedWindow.surfaceKind],
    ["listedWindow.appViewVariant", listedWindow.appViewVariant],
  ]
    .filter((entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0)
    .map(([field, value]) => ({ field, value }));
}

function acceptedSurfaceValues(expectedSurfaceKind: string): Set<string> {
  const values = new Set<string>([expectedSurfaceKind]);
  // Agent Chat detached windows expose their UI contract through automation
  // semanticSurface while their window kind remains AgentChatDetached.
  if (expectedSurfaceKind === "AgentChat") {
    values.add("agentChatChat");
  }
  return values;
}

function surfaceMatch(snapshot: JsonObject, listedWindow: JsonObject, expectedSurfaceKind: string) {
  if (!expectedSurfaceKind) {
    return {
      ok: true,
      expectedSurfaceKind: null,
      acceptedValues: [] as string[],
      matchedCandidate: null as SurfaceCandidate | null,
      candidates: [] as SurfaceCandidate[],
      actualValues: [] as string[],
      mismatchReason: null,
    };
  }
  const candidates = surfaceCandidates(snapshot, listedWindow);
  const actualValues = [...new Set(candidates.map((candidate) => candidate.value))];
  const acceptedValues = acceptedSurfaceValues(expectedSurfaceKind);
  const matchedCandidate = candidates.find((candidate) => acceptedValues.has(candidate.value)) ?? null;
  const ok = matchedCandidate != null;
  return {
    ok,
    expectedSurfaceKind,
    acceptedValues: [...acceptedValues],
    matchedCandidate,
    candidates,
    actualValues,
    mismatchReason: ok ? null : "expected-surface-not-found",
  };
}

function actualSurfaceCandidate(windowId: unknown, listedWindow: JsonObject): ActualSurfaceCandidate {
  return {
    automationId: windowId,
    windowKind: listedWindow.windowKind ?? null,
    surfaceKind: listedWindow.surfaceKind ?? null,
    semanticSurface: listedWindow.semanticSurface ?? null,
    appViewVariant: listedWindow.appViewVariant ?? null,
  };
}

export type TargetIdentityArgs = Pick<TargetArgs, "target" | "strict" | "expectedSurfaceKind">;

export function targetIdentity(args: TargetIdentityArgs, inspect: JsonObject, windows: JsonObject) {
  const snapshot = (inspect.snapshot as JsonObject | undefined) ?? inspect;
  const resolvedBounds = snapshot.resolvedBounds ?? snapshot.bounds ?? null;
  const windowId = snapshot.windowId ?? snapshot.id ?? null;
  const listedWindows = pickWindows(windows);
  const listedWindow = listedWindows.find((window) => window.automationId === windowId) ?? {};
  const windowGeneration = snapshot.windowGeneration
    ?? snapshot.generation
    ?? (listedWindow as JsonObject).windowGeneration
    ?? null;
  const windowInstanceId = stableWindowInstanceId(windowId, windowGeneration);
  const windowKind = stableWindowKind(
    snapshot.windowKind ?? snapshot.kind ?? (listedWindow as JsonObject).windowKind,
  );
  const parentAutomationId = snapshot.parentAutomationId
    ?? snapshot.parentWindowId
    ?? (listedWindow as JsonObject).parentAutomationId
    ?? null;
  const parentWindow = listedWindows.find((window) => window.automationId === parentAutomationId);
  const match = surfaceMatch(snapshot, listedWindow as JsonObject, args.expectedSurfaceKind);
  const strictTargetMatch = Boolean(windowId) && match.ok;
  const ambiguity = args.strict && !windowId ? pickWindows(windows) : [];

  return {
    requestedTarget: {
      selector: args.target ?? { type: "focused" },
      strict: args.strict,
      expectedSurfaceKind: args.expectedSurfaceKind || null,
    },
    resolvedTarget: {
      automationId: windowId,
      stableTargetId: windowId,
      windowInstanceId,
      windowGeneration,
      windowKind,
      targetKind: snapshot.windowKind ?? snapshot.kind ?? null,
      hostKind: snapshot.hostKind ?? derivedHostKind(windowKind),
      parentAutomationId,
      parentWindowInstanceId: parentWindow?.windowInstanceId ?? null,
      openerAutomationId: snapshot.openerAutomationId ?? null,
      nativeWindowId: snapshot.osWindowId ?? snapshot.nativeWindowId ?? null,
      axWindowId: snapshot.axWindowId ?? null,
      surfaceKind: snapshot.surfaceKind ?? null,
      semanticSurface: snapshot.semanticSurface ?? (listedWindow as JsonObject).semanticSurface ?? null,
      appViewVariant: snapshot.appViewVariant ?? null,
      nativeFooterSurface: snapshot.nativeFooterSurface ?? null,
      surfaceFamily: snapshot.surfaceFamily ?? null,
      routeId: snapshot.routeId ?? null,
      routeStack: snapshot.routeStack ?? [],
      targetGeneration: snapshot.targetGeneration ?? null,
      surfaceGeneration: snapshot.surfaceGeneration ?? null,
      dataGeneration: snapshot.dataGeneration ?? null,
      layoutGeneration: snapshot.layoutGeneration ?? null,
      selectionGeneration: snapshot.selectionGeneration ?? null,
      scrollGeneration: snapshot.scrollGeneration ?? null,
      frameGeneration: snapshot.frameGeneration ?? null,
      bounds: resolvedBounds,
      screenId: snapshot.screenId ?? null,
      backingScaleFactor: snapshot.backingScaleFactor ?? null,
      zOrder: snapshot.zOrder ?? null,
      visible: snapshot.visible ?? null,
      frontmost: snapshot.frontmost ?? null,
      focused: snapshot.focused ?? null,
      screenshotIdentity: {
        width: snapshot.screenshotWidth ?? snapshot.screenshot_width ?? null,
        height: snapshot.screenshotHeight ?? snapshot.screenshot_height ?? null,
        targetBoundsInScreenshot: snapshot.targetBoundsInScreenshot ?? null,
        nonBlankRatio: snapshot.nonBlankRatio ?? null,
      },
      pid: snapshot.pid ?? (listedWindow as JsonObject).pid ?? null,
      strictTargetMatch,
      strictTargetMismatch:
        args.strict && !strictTargetMatch
          ? {
              expectedSurfaceKind: args.expectedSurfaceKind || null,
              automationId: windowId,
              surfaceCandidates: match.candidates,
              actualCandidates: [actualSurfaceCandidate(windowId, listedWindow as JsonObject)],
              actualValues: match.actualValues,
              mismatchReason: match.mismatchReason,
            }
          : null,
      ambiguity,
    },
  };
}

export interface ProofTransactionIdentity extends JsonObject {
  transactionId: string;
  runId: string;
  capturedAt: string;
  pid: number | null;
  processStartTime: string | null;
  binarySha256: string | null;
  automationId: string | null;
  windowInstanceId: string | null;
  nativeWindowId: number | null;
  axWindowId: string | null;
  windowKind: string | null;
  hostKind: string | null;
  parentAutomationId: string | null;
  parentWindowInstanceId: string | null;
  openerAutomationId: string | null;
  surfaceKind: string | null;
  semanticSurface: string | null;
  appViewVariant: string | null;
  routeId: string | null;
  routeStack: unknown[];
  screenId: string | null;
  backingScaleFactor: number | null;
  bounds: unknown;
  windowGeneration: number | null;
  targetGeneration: number | null;
  surfaceGeneration: number | null;
  dataGeneration: number | null;
  layoutGeneration: number | null;
  selectionGeneration: number | null;
  scrollGeneration: number | null;
  frameGeneration: number | null;
}

function processStartTime(pid: unknown): string | null {
  if (typeof pid !== "number" || !Number.isFinite(pid)) return null;
  if (pid === process.pid) {
    // Self-owned unit fixtures have a direct process clock. They must not
    // depend on sandbox permission to enumerate host processes.
    return new Date(Date.now() - process.uptime() * 1_000).toISOString();
  }
  try {
    const result = Bun.spawnSync(["ps", "-p", String(pid), "-o", "lstart="], {
      stdout: "pipe",
      stderr: "pipe",
    });
    if (result.exitCode !== 0) return null;
    const value = new TextDecoder().decode(result.stdout).trim();
    return value || null;
  } catch {
    // Restricted CI/sandboxes may deny process enumeration. Leave the proof
    // incomplete so strict identity blocks; never invent another PID's start.
    return null;
  }
}

export function proofTransactionIdentity(
  session: string,
  resolvedTarget: JsonObject,
  capturedAt = new Date().toISOString(),
): ProofTransactionIdentity {
  const pid = typeof resolvedTarget.pid === "number" ? resolvedTarget.pid : null;
  const binary = binaryFingerprint(session);
  const identitySeed = JSON.stringify({
    session,
    capturedAt,
    automationId: resolvedTarget.automationId ?? null,
    windowInstanceId: resolvedTarget.windowInstanceId ?? null,
    targetGeneration: resolvedTarget.targetGeneration ?? null,
    surfaceGeneration: resolvedTarget.surfaceGeneration ?? null,
    dataGeneration: resolvedTarget.dataGeneration ?? null,
  });
  return {
    transactionId: `proof:${createHash("sha256").update(identitySeed).digest("hex").slice(0, 24)}`,
    runId: session,
    capturedAt,
    pid,
    processStartTime: processStartTime(pid),
    binarySha256: binary?.sha256 ?? null,
    automationId: typeof resolvedTarget.automationId === "string"
      ? resolvedTarget.automationId
      : null,
    windowInstanceId: typeof resolvedTarget.windowInstanceId === "string"
      ? resolvedTarget.windowInstanceId
      : null,
    nativeWindowId: typeof resolvedTarget.nativeWindowId === "number"
      ? resolvedTarget.nativeWindowId
      : null,
    axWindowId: typeof resolvedTarget.axWindowId === "string"
      ? resolvedTarget.axWindowId
      : null,
    windowKind: typeof resolvedTarget.windowKind === "string"
      ? resolvedTarget.windowKind
      : typeof resolvedTarget.targetKind === "string"
        ? resolvedTarget.targetKind
        : null,
    hostKind: typeof resolvedTarget.hostKind === "string" ? resolvedTarget.hostKind : null,
    parentAutomationId: typeof resolvedTarget.parentAutomationId === "string"
      ? resolvedTarget.parentAutomationId
      : null,
    parentWindowInstanceId: typeof resolvedTarget.parentWindowInstanceId === "string"
      ? resolvedTarget.parentWindowInstanceId
      : null,
    openerAutomationId: typeof resolvedTarget.openerAutomationId === "string"
      ? resolvedTarget.openerAutomationId
      : null,
    surfaceKind: typeof resolvedTarget.surfaceKind === "string" ? resolvedTarget.surfaceKind : null,
    semanticSurface: typeof resolvedTarget.semanticSurface === "string"
      ? resolvedTarget.semanticSurface
      : null,
    appViewVariant: typeof resolvedTarget.appViewVariant === "string"
      ? resolvedTarget.appViewVariant
      : null,
    routeId: typeof resolvedTarget.routeId === "string" ? resolvedTarget.routeId : null,
    routeStack: Array.isArray(resolvedTarget.routeStack) ? resolvedTarget.routeStack : [],
    screenId: typeof resolvedTarget.screenId === "string" ? resolvedTarget.screenId : null,
    backingScaleFactor: typeof resolvedTarget.backingScaleFactor === "number"
      ? resolvedTarget.backingScaleFactor
      : null,
    bounds: resolvedTarget.bounds ?? null,
    windowGeneration: typeof resolvedTarget.windowGeneration === "number"
      ? resolvedTarget.windowGeneration
      : null,
    targetGeneration: typeof resolvedTarget.targetGeneration === "number"
      ? resolvedTarget.targetGeneration
      : null,
    surfaceGeneration: typeof resolvedTarget.surfaceGeneration === "number"
      ? resolvedTarget.surfaceGeneration
      : null,
    dataGeneration: typeof resolvedTarget.dataGeneration === "number"
      ? resolvedTarget.dataGeneration
      : null,
    layoutGeneration: typeof resolvedTarget.layoutGeneration === "number"
      ? resolvedTarget.layoutGeneration
      : null,
    selectionGeneration: typeof resolvedTarget.selectionGeneration === "number"
      ? resolvedTarget.selectionGeneration
      : null,
    scrollGeneration: typeof resolvedTarget.scrollGeneration === "number"
      ? resolvedTarget.scrollGeneration
      : null,
    frameGeneration: typeof resolvedTarget.frameGeneration === "number"
      ? resolvedTarget.frameGeneration
      : null,
  };
}

export function strictTransactionMissingFields(
  transaction: ProofTransactionIdentity,
): string[] {
  return [
    transaction.automationId ? "" : "automationId",
    transaction.windowInstanceId ? "" : "windowInstanceId",
    transaction.windowGeneration != null ? "" : "windowGeneration",
    transaction.pid != null ? "" : "pid",
    transaction.processStartTime ? "" : "processStartTime",
    transaction.binarySha256 ? "" : "binarySha256",
    transaction.windowKind ? "" : "windowKind",
    transaction.surfaceKind || transaction.semanticSurface ? "" : "surfaceKind",
    transaction.windowKind !== "Main" || transaction.appViewVariant ? "" : "appViewVariant",
    transaction.bounds != null ? "" : "bounds",
    transaction.targetGeneration != null ? "" : "targetGeneration",
    transaction.surfaceGeneration != null ? "" : "surfaceGeneration",
    transaction.dataGeneration != null ? "" : "dataGeneration",
  ].filter(Boolean);
}

export function compareWindowLifetimeSnapshots(
  automationId: unknown,
  beforeWindows: JsonObject,
  afterWindows: JsonObject,
) {
  const before = pickWindows(beforeWindows).find((window) => window.automationId === automationId);
  const after = pickWindows(afterWindows).find((window) => window.automationId === automationId);
  const errors = [
    before ? "" : "target missing from pre-inspection registry snapshot",
    after ? "" : "target missing from post-inspection registry snapshot",
    before && after && before.windowInstanceId !== after.windowInstanceId
      ? "window instance changed during target inspection"
      : "",
    before && after && before.pid !== after.pid
      ? "window owner pid changed during target inspection"
      : "",
    before && after && JSON.stringify(before.bounds) !== JSON.stringify(after.bounds)
      ? "window bounds changed during target inspection"
      : "",
  ].filter(Boolean);
  return {
    consistent: errors.length === 0,
    automationId: automationId ?? null,
    before: before ?? null,
    after: after ?? null,
    errors,
  };
}

export function classifyTarget(
  args: TargetIdentityArgs,
  identity: ReturnType<typeof targetIdentity>,
  errors: JsonObject[],
): string {
  if (hasSessionLifecycleError(errors)) {
    return "blocked-by-session-lifecycle";
  }
  if (errors.length > 0) {
    return classifyEnvelopes(errors);
  }
  if (args.strict && !identity.resolvedTarget.automationId) {
    return "blocked-by-target-ambiguity";
  }
  if (args.strict && !identity.resolvedTarget.strictTargetMatch) {
    return "blocked-by-target-ambiguity";
  }
  return "ok";
}

export async function maybeStartAndShow(args: Pick<TargetArgs, "session" | "start" | "show" | "timeoutMs">) {
  if (args.start) {
    await run(["bash", "scripts/agentic/session.sh", "start", args.session], "session-start");
  }
  if (args.show) {
    await run(
      [
        "bash",
        "scripts/agentic/session.sh",
        "send",
        args.session,
        JSON.stringify({ type: "show" }),
        "--await-parse",
        "--timeout",
        String(args.timeoutMs),
      ],
      "session-show",
    );
  }
}

export interface TargetReceipt extends JsonObject {
  classification: string;
  requestedTarget: JsonObject;
  resolvedTarget: JsonObject;
  transaction: ProofTransactionIdentity;
  transactionValidation: JsonObject;
  windows: JsonObject[];
  windowsAfter: JsonObject[];
  errors: JsonObject[];
}

function record(value: unknown): JsonObject {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as JsonObject
    : {};
}

function hiddenTargetWindow(
  selector: JsonObject,
  windows: JsonObject,
): JsonObject {
  const selectorType = selector.type;
  if (selectorType !== "main" && selectorType !== "id" && selectorType !== "instance") {
    throw new NoninteractiveSafetyError(
      "resolveTargetReceipt",
      "hidden target resolution requires an explicit main, id, or exact instance selector",
    );
  }
  const matches = pickWindows(windows).filter((window) => {
    if (selectorType === "main") {
      return window.automationId === "main" || window.windowKind === "Main";
    }
    if (window.automationId !== selector.id) return false;
    return selectorType !== "instance" || window.windowGeneration === selector.generation;
  });
  if (matches.length !== 1) {
    throw new NoninteractiveSafetyError(
      "resolveTargetReceipt",
      `explicit hidden target resolved to ${matches.length} windows instead of exactly one`,
    );
  }
  const window = matches[0]!;
  if (window.visible !== false) {
    throw new NoninteractiveSafetyError(
      "resolveTargetReceipt",
      "the explicit target is visible or its visibility is unknown",
    );
  }
  return window;
}

/**
 * Compose the capture-free equivalent of an automation inspection. Generations
 * come only from the runtime's canonical state surface contract. Missing
 * generations stay null and make strict proof BLOCKED_MISSING_PRIMITIVE.
 */
export function hiddenTargetInspectionSnapshot(
  window: JsonObject,
  state: JsonObject,
): JsonObject {
  if (window.visible !== false || state.windowVisible !== false) {
    throw new NoninteractiveSafetyError(
      "resolveTargetReceipt",
      "both the automation registry and state response must observe a hidden target",
    );
  }
  const surfaceContract = record(state.surfaceContract);
  const canonicalIdentity = record(surfaceContract.targetIdentity);
  const topLevelIdentity = record(state.targetIdentity);
  const generationFields = [
    "targetGeneration",
    "surfaceGeneration",
    "dataGeneration",
    "layoutGeneration",
    "selectionGeneration",
    "scrollGeneration",
    "frameGeneration",
  ] as const;
  for (const [location, identity] of [
    ["state.surfaceContract.targetIdentity", canonicalIdentity],
    ["state.targetIdentity", topLevelIdentity],
    ["state", state],
  ] as const) {
    if (
      typeof identity.windowId === "string" &&
      identity.windowId !== window.automationId
    ) {
      throw new NoninteractiveSafetyError(
        "resolveTargetReceipt",
        `${location} target identity belongs to a different automation window`,
      );
    }
    if (
      typeof identity.windowGeneration === "number" &&
      identity.windowGeneration !== window.windowGeneration
    ) {
      throw new NoninteractiveSafetyError(
        "resolveTargetReceipt",
        `${location} target identity belongs to a stale automation window generation`,
      );
    }
    if (identity !== canonicalIdentity) {
      for (const field of generationFields) {
        if (
          identity[field] != null &&
          canonicalIdentity[field] != null &&
          identity[field] !== canonicalIdentity[field]
        ) {
          throw new NoninteractiveSafetyError(
            "resolveTargetReceipt",
            `${location}.${field} conflicts with the canonical target identity`,
          );
        }
      }
    }
  }
  const identityMetadata = (key: string): unknown =>
    canonicalIdentity[key] ?? topLevelIdentity[key] ?? state[key] ?? null;

  return {
    windowId: window.automationId ?? null,
    windowKind: window.windowKind ?? null,
    windowGeneration: canonicalIdentity.windowGeneration ?? window.windowGeneration ?? null,
    parentAutomationId: window.parentAutomationId ?? null,
    resolvedBounds: window.bounds ?? null,
    visible: false,
    focused: typeof state.isFocused === "boolean" ? state.isFocused : window.focused ?? null,
    pid: window.pid ?? null,
    surfaceKind:
      identityMetadata("surfaceKind") ?? surfaceContract.surfaceKind ?? window.surfaceKind ?? null,
    semanticSurface:
      identityMetadata("semanticSurface") ??
      surfaceContract.automationSemanticSurface ??
      window.semanticSurface ??
      null,
    appViewVariant:
      identityMetadata("appViewVariant") ??
      surfaceContract.appViewVariant ??
      window.appViewVariant ??
      null,
    targetGeneration: canonicalIdentity.targetGeneration ?? null,
    surfaceGeneration: canonicalIdentity.surfaceGeneration ?? null,
    dataGeneration: canonicalIdentity.dataGeneration ?? null,
    layoutGeneration: canonicalIdentity.layoutGeneration ?? null,
    selectionGeneration: canonicalIdentity.selectionGeneration ?? null,
    scrollGeneration: canonicalIdentity.scrollGeneration ?? null,
    frameGeneration: canonicalIdentity.frameGeneration ?? null,
    surfaceContract,
    observationMode: "capture-free-hidden-state",
  };
}

/**
 * Resolve strict target identity in-process: listAutomationWindows +
 * inspectAutomationWindow, then the identity/classification pipeline. Returns
 * the same receipt shape `targets.ts inspect` prints, so downstream code that
 * reads requestedTarget/resolvedTarget/classification is unchanged.
 */
export async function resolveTargetReceipt(
  args: Pick<TargetArgs, "session" | "target" | "strict" | "expectedSurfaceKind" | "timeoutMs">,
  opts: {
    tool?: string;
    hiDpi?: boolean;
    noninteractive?: boolean;
    rpcFn?: typeof rpc;
  } = {},
): Promise<TargetReceipt> {
  const tool = opts.tool ?? "targets";
  const invokeRpc = opts.rpcFn ?? rpc;
  const noninteractive =
    process.env.SCRIPT_KIT_NONINTERACTIVE === "1" || opts.noninteractive === true;
  const target = args.target ?? { type: "focused" };
  if (noninteractive) {
    assertNoninteractiveProtocolCommand(
      { type: "getState", target },
      { noninteractive: true },
    );
  }

  const windowsEnvelope = await invokeRpc(
    args.session,
    { type: "listAutomationWindows", requestId: requestId(tool, "list") },
    "automationWindowListResult",
    args.timeoutMs,
  );
  const windows = responseOf(windowsEnvelope);
  const errors = [windowsEnvelope].filter((value) => value.status === "error");

  let inspectEnvelope: JsonObject;
  let inspect: JsonObject;
  if (noninteractive) {
    const window = hiddenTargetWindow(target, windows);
    inspectEnvelope = await invokeRpc(
      args.session,
      {
        type: "getState",
        requestId: requestId(tool, "hidden-state"),
        target,
        summaryOnly: true,
      },
      "stateResult",
      args.timeoutMs,
    );
    const state = responseOf(inspectEnvelope);
    inspect = inspectEnvelope.status === "error"
      ? {}
      : hiddenTargetInspectionSnapshot(window, state);
  } else {
    inspectEnvelope = await invokeRpc(
      args.session,
      {
        type: "inspectAutomationWindow",
        requestId: requestId(tool, "inspect"),
        target,
        hiDpi: opts.hiDpi ?? false,
        probes: [],
      },
      "automationInspectResult",
      args.timeoutMs,
    );
    inspect = responseOf(inspectEnvelope);
  }
  const windowsAfterEnvelope = await invokeRpc(
    args.session,
    { type: "listAutomationWindows", requestId: requestId(tool, "list-after") },
    "automationWindowListResult",
    args.timeoutMs,
  );
  const windowsAfter = responseOf(windowsAfterEnvelope);
  const inspectErrors = [...errors, inspectEnvelope, windowsAfterEnvelope]
    .filter((value) => value.status === "error");
  const identity = targetIdentity(args, inspect, windows);
  const transaction = proofTransactionIdentity(args.session, identity.resolvedTarget);
  const transactionMissingFields = strictTransactionMissingFields(transaction);
  const lifetimeConsistency = compareWindowLifetimeSnapshots(
    identity.resolvedTarget.automationId,
    windows,
    windowsAfter,
  );
  const baseClassification = classifyTarget(args, identity, inspectErrors);
  const classification = baseClassification !== "ok"
    ? baseClassification
    : !lifetimeConsistency.consistent
      ? "blocked-by-stale-generation"
      : args.strict && transactionMissingFields.length > 0
        ? "blocked-by-missing-primitive"
        : "ok";

  return {
    classification,
    ...identity,
    transaction,
    transactionValidation: {
      valid: lifetimeConsistency.consistent && transactionMissingFields.length === 0,
      lifetimeConsistency: {
        ...lifetimeConsistency,
        errors: productStatic(lifetimeConsistency.errors),
      },
      missingFields: transactionMissingFields,
    },
    windows: pickWindows(windows) as unknown as JsonObject[],
    windowsAfter: pickWindows(windowsAfter) as unknown as JsonObject[],
    rawInspect: inspect,
    lifecycleCodes: lifecycleCodes(inspectErrors),
    lifecycleDetails: primaryLifecycleDetails(inspectErrors),
    sessionLifecycle: primarySessionLifecycle(inspectErrors),
    parsedError: primaryParsedError(inspectErrors),
    errors: inspectErrors,
    inspectionMode: noninteractive ? "capture-free-hidden-state" : "automation-window-inspection",
  };
}
