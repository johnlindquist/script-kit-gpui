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
import { externalContent } from "./privacy.ts";

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

export function pickWindows(windows: JsonObject) {
  return asArray(windows.windows ?? windows.automationWindows ?? windows.targets).map((window, index) => ({
    index,
    automationId: window.id ?? window.windowId ?? window.automationId ?? null,
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
  }));
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
  const listedWindow = pickWindows(windows).find((window) => window.automationId === windowId) ?? {};
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
      targetKind: snapshot.windowKind ?? snapshot.kind ?? null,
      hostKind: snapshot.hostKind ?? null,
      parentAutomationId:
        snapshot.parentAutomationId ?? snapshot.parentWindowId ?? (listedWindow as JsonObject).parentAutomationId ?? null,
      openerAutomationId: snapshot.openerAutomationId ?? null,
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
      bounds: resolvedBounds,
      screenId: snapshot.screenId ?? null,
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
  windows: JsonObject[];
  errors: JsonObject[];
}

/**
 * Resolve strict target identity in-process: listAutomationWindows +
 * inspectAutomationWindow, then the identity/classification pipeline. Returns
 * the same receipt shape `targets.ts inspect` prints, so downstream code that
 * reads requestedTarget/resolvedTarget/classification is unchanged.
 */
export async function resolveTargetReceipt(
  args: Pick<TargetArgs, "session" | "target" | "strict" | "expectedSurfaceKind" | "timeoutMs">,
  opts: { tool?: string; hiDpi?: boolean } = {},
): Promise<TargetReceipt> {
  const tool = opts.tool ?? "targets";
  const windowsEnvelope = await rpc(
    args.session,
    { type: "listAutomationWindows", requestId: requestId(tool, "list") },
    "automationWindowListResult",
    args.timeoutMs,
  );
  const windows = responseOf(windowsEnvelope);
  const errors = [windowsEnvelope].filter((value) => value.status === "error");

  const target = args.target ?? { type: "focused" };
  const inspectEnvelope = await rpc(
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
  const inspect = responseOf(inspectEnvelope);
  const inspectErrors = [...errors, inspectEnvelope].filter((value) => value.status === "error");
  const identity = targetIdentity(args, inspect, windows);

  return {
    classification: classifyTarget(args, identity, inspectErrors),
    ...identity,
    windows: pickWindows(windows) as unknown as JsonObject[],
    rawInspect: inspect,
    lifecycleCodes: lifecycleCodes(inspectErrors),
    lifecycleDetails: primaryLifecycleDetails(inspectErrors),
    sessionLifecycle: primarySessionLifecycle(inspectErrors),
    parsedError: primaryParsedError(inspectErrors),
    errors: inspectErrors,
  };
}
