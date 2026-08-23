#!/usr/bin/env bun
/**
 * Warm-session, state-first navigator for known Script Kit GPUI surfaces.
 *
 * This is intentionally script-level orchestration over the existing stdin JSON
 * protocol. It does not add protocol fields; it composes parse receipts,
 * typed RPCs, batch receipts, getElements, and strict screenshot capture.
 */

import { existsSync, mkdirSync, statSync, writeFileSync } from "fs";
import { resolve } from "path";
import { assertNoninteractiveSubprocess } from "../devtools/lib/operator-safety.ts";
import {
  ATTACHED_POPUP_SURFACE_MATRIX,
  selectedAttachedPopupCases,
  type AttachedPopupSurfaceEntry,
} from "./attached-popup-surface-matrix";
import {
  FILTERABLE_SURFACE_MATRIX,
  type FilterableSurfaceMatrixEntry,
  type JsonObject,
  type MatrixAutomationTarget,
  elementsFrom,
  enterFilterableSurface,
  getStateAndElements,
  rpc,
  selectedCases,
  sendAndAwaitParse,
  sessionStart,
  sessionStop,
  waitForPromptType,
} from "./filterable-surface-matrix";

const SCHEMA_VERSION = 1;
const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const DEFAULT_OUT_DIR = ".notes/image-library";
type SourceSurfaceGroup = "filterable-main" | "attached-popup";
type SurfaceGroup = SourceSurfaceGroup | "all-active";
type NavigatorCase = FilterableSurfaceMatrixEntry | AttachedPopupSurfaceEntry;

interface SelectedNavigatorCase {
  sourceGroup: SourceSurfaceGroup;
  entry: NavigatorCase;
}

interface NavigatorOptions {
  session: string;
  group: SurfaceGroup;
  caseId: string;
  timeoutMs: number;
  interact: "none" | "safe";
  capture: boolean;
  outDir: string;
  manifestPath: string;
  freshPerCase: boolean;
  keepSession: boolean;
  json: boolean;
  list: boolean;
}

interface StepReceipt {
  name: string;
  status: "pass" | "fail" | "error" | "skipped";
  output: unknown;
  durationMs: number;
}

interface ResolvedSurfaceTarget {
  targetJson: { type: "id"; id: string };
  automationWindowId: string;
  osWindowId: number;
  windowKind: string;
  title: string | null;
}

interface AttachedPopupHostSetup {
  kind: "filterable-main" | "agent_chat-chat";
  caseId?: string;
  trigger?: "slash";
  viewName: string;
  promptType: string;
  entryReceipt: JsonObject | null;
  showReceipt: JsonObject;
  readyState: JsonObject | null;
  resolvedTarget: ResolvedSurfaceTarget;
  observation: JsonObject;
  state: JsonObject | null;
  elements: JsonObject | null;
}

function hasFlag(flag: string): boolean {
  return process.argv.includes(flag);
}

function argValue(flag: string, fallback: string): string {
  const index = process.argv.indexOf(flag);
  if (index < 0) {
    return fallback;
  }
  return process.argv[index + 1] ?? fallback;
}

function parseArgs(): NavigatorOptions {
  const interact = argValue("--interact", "none");
  if (interact !== "none" && interact !== "safe") {
    throw new Error(`Unsupported --interact ${interact}; expected none or safe`);
  }
  const group = argValue("--group", "filterable-main");
  if (group !== "filterable-main" && group !== "attached-popup" && group !== "all-active") {
    throw new Error(
      `Unsupported --group ${group}; expected filterable-main, attached-popup, or all-active`,
    );
  }
  const outDir = argValue("--out-dir", DEFAULT_OUT_DIR);
  const freshPerCase = hasFlag("--fresh-per-case");
  if (freshPerCase && hasFlag("--keep-session")) {
    throw new Error("--fresh-per-case cannot be combined with --keep-session");
  }
  return {
    session: argValue("--session", "surface-navigator"),
    group,
    caseId: argValue("--case", "all"),
    timeoutMs: Number(argValue("--timeout", "5000")),
    interact,
    capture: hasFlag("--capture"),
    outDir,
    manifestPath: argValue("--manifest", `${outDir}/manifest.json`),
    freshPerCase,
    keepSession: hasFlag("--keep-session"),
    json: hasFlag("--json"),
    list: hasFlag("--list"),
  };
}

function surfaceOutPath(entry: NavigatorCase, outDir: string): string {
  return resolve(PROJECT_ROOT, outDir, entry.imageLibraryName);
}

function receiptOutPath(entry: NavigatorCase, outDir: string): string {
  return resolve(
    PROJECT_ROOT,
    outDir,
    entry.imageLibraryName.replace(/\.png$/, ".receipt.json"),
  );
}

function manifestOutPath(opts: NavigatorOptions): string {
  return resolve(PROJECT_ROOT, opts.manifestPath);
}

function writeJsonFile(path: string, value: unknown): void {
  const dir = path.slice(0, path.lastIndexOf("/"));
  if (dir && !existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

async function timedStep(
  name: string,
  fn: () => Promise<unknown>,
): Promise<StepReceipt> {
  const started = Date.now();
  try {
    const output = await fn();
    return {
      name,
      status: "pass",
      output,
      durationMs: Date.now() - started,
    };
  } catch (error) {
    return {
      name,
      status: "error",
      output: { error: error instanceof Error ? error.message : String(error) },
      durationMs: Date.now() - started,
    };
  }
}

async function startOrReuseSession(session: string): Promise<JsonObject> {
  return sessionStart(session);
}

async function promoteExactTarget(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  timeoutMs: number,
): Promise<ResolvedSurfaceTarget> {
  const response = await rpc(
    session,
    {
      type: "inspectAutomationWindow",
      requestId: `${entry.id}-promote-${Date.now()}`,
      target: entry.target,
    },
    "automationInspectResult",
    timeoutMs,
  );
  const automationWindowId = String(response.windowId ?? response.automationWindowId ?? "");
  const osWindowId =
    typeof response.osWindowId === "number" && response.osWindowId > 0
      ? response.osWindowId
      : null;
  if (!automationWindowId) {
    throw new Error(`${entry.id}: inspectAutomationWindow did not return windowId`);
  }
  if (osWindowId == null) {
    throw new Error(`${entry.id}: inspectAutomationWindow did not return a usable osWindowId`);
  }
  return {
    targetJson: { type: "id", id: automationWindowId },
    automationWindowId,
    osWindowId,
    windowKind: String(response.windowKind ?? ""),
    title: typeof response.title === "string" ? response.title : null,
  };
}

async function navigateToSurface(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  timeoutMs: number,
): Promise<{
  entryReceipt: JsonObject;
  showReceipt: JsonObject;
  readyState: JsonObject;
  resolvedTarget: ResolvedSurfaceTarget;
}> {
  const entryReceipt = await enterFilterableSurface(session, entry, timeoutMs);
  const showReceipt = await sendAndAwaitParse(session, { type: "show" }, timeoutMs);
  await Bun.sleep(300);
  const readyState = await waitForPromptType(session, entry, timeoutMs);
  await sendAndAwaitParse(session, { type: "setFilter", text: "" }, timeoutMs);
  const resolvedTarget = await promoteExactTarget(session, entry, timeoutMs);
  return { entryReceipt, showReceipt, readyState, resolvedTarget };
}

function filterableEntryById(caseId: string): FilterableSurfaceMatrixEntry {
  const entry = FILTERABLE_SURFACE_MATRIX.find((candidate) => candidate.id === caseId);
  if (!entry) {
    throw new Error(`Unknown attached popup host fixture filterable case: ${caseId}`);
  }
  return entry;
}

async function enterAttachedPopupHostFixture(
  session: string,
  entry: AttachedPopupSurfaceEntry,
  timeoutMs: number,
): Promise<AttachedPopupHostSetup | null> {
  if (!entry.hostFixture) {
    return null;
  }
  if (entry.hostFixture.kind !== "filterable-main") {
    if (entry.hostFixture.kind !== "agent_chat-chat") {
      throw new Error(`${entry.id}: unsupported host fixture ${entry.hostFixture.kind}`);
    }
    const showReceipt = await sendAndAwaitParse(session, { type: "show" }, timeoutMs);
    const entryReceipt = await sendAndAwaitParse(
      session,
      { type: "triggerBuiltin", name: "tab-ai" },
      timeoutMs,
    );
    await rpc(
      session,
      {
        type: "waitFor",
        requestId: `${entry.id}-agent_chat-ready-${Date.now()}`,
        condition: { type: "agent_chatReady" },
        timeout: timeoutMs,
        pollInterval: 25,
        trace: "onFailure",
      },
      "waitForResult",
      timeoutMs,
    );
    const resolvedTarget = await promoteExactTarget(
      session,
      {
        id: `${entry.id}-agent_chat-host`,
        surface: "agentChatChat",
        viewName: "agent_chat-chat",
        imageLibraryName: "agent_chat-chat.png",
        promptType: "ai",
        listSemanticId: "list:agent_chat-messages",
        entryCommand: { type: "triggerBuiltin", name: "tab-ai" },
        filterText: "",
        expectedElementChromeCount: 0,
        target: { type: "kind", kind: "main", index: 0 },
        safeInteractions: {
          filter: false,
          selectFirstVisibleChoice: false,
          submit: false,
        },
      },
      timeoutMs,
    );
    const observation = await rpc(
      session,
      {
        type: "getAgentChatState",
        requestId: `${entry.id}-agent_chat-host-state-${Date.now()}`,
        target: resolvedTarget.targetJson,
      },
      "agent_chatStateResult",
      timeoutMs,
    );
    return {
      kind: "agent_chat-chat",
      trigger: entry.hostFixture.trigger,
      viewName: "agent_chat-chat",
      promptType: "ai",
      entryReceipt,
      showReceipt,
      readyState: null,
      resolvedTarget,
      observation,
      state: null,
      elements: null,
    };
  }

  const hostEntry = filterableEntryById(entry.hostFixture.caseId);
  const entryReceipt = await enterFilterableSurface(session, hostEntry, timeoutMs);
  const showReceipt = await sendAndAwaitParse(session, { type: "show" }, timeoutMs);
  await Bun.sleep(300);
  const readyState = await waitForPromptType(session, hostEntry, timeoutMs);
  await sendAndAwaitParse(session, { type: "setFilter", text: "" }, timeoutMs);
  const resolvedTarget = await promoteExactTarget(session, hostEntry, timeoutMs);
  const snapshot = await getStateAndElements(
    session,
    hostEntry,
    timeoutMs,
    "attached-popup-host",
    resolvedTarget.targetJson,
  );
  return {
    kind: "filterable-main",
    caseId: hostEntry.id,
    viewName: hostEntry.viewName,
    promptType: hostEntry.promptType,
    entryReceipt,
    showReceipt,
    readyState,
    resolvedTarget,
    observation: snapshot.observation as unknown as JsonObject,
    state: snapshot.state,
    elements: snapshot.elements,
  };
}

async function openAttachedPopup(
  session: string,
  entry: AttachedPopupSurfaceEntry,
  timeoutMs: number,
): Promise<JsonObject> {
  if (entry.targetKind === "promptPopup") {
    return sendAndAwaitParse(
      session,
      {
        type: "setAgentChatInput",
        requestId: `${entry.id}-set-agent_chat-input-${Date.now()}`,
        text: "/",
        submit: false,
      },
      timeoutMs,
    );
  }
  return sendAndAwaitParse(
    session,
    { type: "simulateKey", key: "k", modifiers: ["cmd"] },
    timeoutMs,
  );
}

async function getElementsForTarget(
  session: string,
  entry: AttachedPopupSurfaceEntry,
  timeoutMs: number,
  requestLabel: string,
  target: MatrixAutomationTarget,
): Promise<JsonObject> {
  return rpc(
    session,
    {
      type: "getElements",
      requestId: `${entry.id}-${requestLabel}-elements`,
      target,
      limit: 500,
    },
    "elementsResult",
    timeoutMs,
  );
}

async function promoteAttachedPopupTarget(
  session: string,
  entry: AttachedPopupSurfaceEntry,
  timeoutMs: number,
): Promise<{ resolvedTarget: ResolvedSurfaceTarget; inspection: JsonObject }> {
  const deadline = Date.now() + timeoutMs;
  let lastInspection: JsonObject | null = null;
  let lastError: string | null = null;
  while (Date.now() < deadline) {
    let response: JsonObject;
    try {
      response = await rpc(
        session,
        {
          type: "inspectAutomationWindow",
          requestId: `${entry.id}-promote-${Date.now()}`,
          target: entry.target,
        },
        "automationInspectResult",
        Math.min(timeoutMs, 2000),
      );
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
      await Bun.sleep(100);
      continue;
    }
    lastInspection = response;
    const automationWindowId = String(response.windowId ?? response.automationWindowId ?? "");
    if (
      entry.expectedAutomationWindowId &&
      automationWindowId !== entry.expectedAutomationWindowId
    ) {
      lastError = `${entry.id}: expected automation window id ${entry.expectedAutomationWindowId}, got ${automationWindowId || "none"}`;
      await Bun.sleep(100);
      continue;
    }
    const osWindowId =
      typeof response.osWindowId === "number" && response.osWindowId > 0
        ? response.osWindowId
        : null;
    const hasBounds =
      typeof response.targetBoundsInScreenshot === "object" &&
      response.targetBoundsInScreenshot != null;
    if (
      automationWindowId &&
      osWindowId != null &&
      response.windowKind === entry.windowKind &&
      hasBounds
    ) {
      const exactTarget: MatrixAutomationTarget = { type: "id", id: automationWindowId };
      const exactInspection = await rpc(
        session,
        {
          type: "inspectAutomationWindow",
          requestId: `${entry.id}-exact-${Date.now()}`,
          target: exactTarget,
        },
        "automationInspectResult",
        timeoutMs,
      );
      return {
        resolvedTarget: {
          targetJson: { type: "id", id: automationWindowId },
          automationWindowId,
          osWindowId,
          windowKind: String(exactInspection.windowKind ?? response.windowKind ?? ""),
          title:
            typeof exactInspection.title === "string"
              ? exactInspection.title
              : typeof response.title === "string"
                ? response.title
                : null,
        },
        inspection: exactInspection,
      };
    }
    await Bun.sleep(100);
  }
  const expectedId = entry.expectedAutomationWindowId
    ? ` and automation id ${entry.expectedAutomationWindowId}`
    : "";
  throw new Error(
    `${entry.id}: expected ${entry.windowKind}${expectedId} with osWindowId and crop bounds, last inspection ${JSON.stringify(lastInspection)}, last error ${lastError ?? "none"}`,
  );
}

async function navigateToAttachedPopup(
  session: string,
  entry: AttachedPopupSurfaceEntry,
  timeoutMs: number,
): Promise<{
  hostSetup: AttachedPopupHostSetup | null;
  showReceipt: JsonObject | null;
  openReceipt: JsonObject;
  resolvedTarget: ResolvedSurfaceTarget;
  preCaptureInspection: JsonObject;
}> {
  const hostSetup = await enterAttachedPopupHostFixture(session, entry, timeoutMs);
  const showReceipt = hostSetup
    ? null
    : await sendAndAwaitParse(session, { type: "show" }, timeoutMs);
  if (!hostSetup) {
    await Bun.sleep(300);
  }
  const openReceipt = await openAttachedPopup(session, entry, timeoutMs);
  const promoted = await promoteAttachedPopupTarget(session, entry, timeoutMs);
  return {
    hostSetup,
    showReceipt,
    openReceipt,
    resolvedTarget: promoted.resolvedTarget,
    preCaptureInspection: promoted.inspection,
  };
}

function firstVisibleChoice(elements: JsonObject): string | null {
  for (const element of elementsFrom(elements)) {
    const semanticId = element.semanticId;
    if (typeof semanticId === "string" && semanticId.startsWith("choice:")) {
      return semanticId;
    }
  }
  return null;
}

async function safeInteractWithSurface(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  timeoutMs: number,
  target: MatrixAutomationTarget,
): Promise<JsonObject> {
  if (!entry.safeInteractions.filter) {
    return { status: "skipped", reason: "filter interaction disabled" };
  }

  await sendAndAwaitParse(session, { type: "setFilter", text: entry.filterText }, timeoutMs);
  const filtered = await getStateAndElements(
    session,
    entry,
    timeoutMs,
    "safe-filtered",
    target,
  );
  const semanticId = entry.safeInteractions.selectFirstVisibleChoice
    ? firstVisibleChoice(filtered.elements)
    : null;

  let selectionReceipt: JsonObject | null = null;
  if (semanticId) {
    const selectCommand = {
      type: "batch",
      requestId: `${entry.id}-safe-select-${Date.now()}`,
      commands: [
        {
          type: "selectBySemanticId",
          semanticId,
          submit: false,
        },
      ],
      trace: "onFailure",
    };
    selectionReceipt = await rpc(session, selectCommand, "batchResult", timeoutMs);
    if (selectionReceipt.success !== true) {
      throw new Error(
        `${entry.id}: safe selectBySemanticId failed: ${JSON.stringify(selectionReceipt)}`,
      );
    }
  }

  await sendAndAwaitParse(session, { type: "setFilter", text: "" }, timeoutMs);
  return {
    status: "pass",
    method: "batch",
    filterText: entry.filterText,
    selectedSemanticId: semanticId,
    selectionSkipped: semanticId == null,
    filteredObservation: filtered.observation,
    selectionReceipt,
  };
}

export async function runTool(cmd: string[]): Promise<JsonObject> {
  assertNoninteractiveSubprocess(cmd);
  const proc = Bun.spawn(cmd, {
    cwd: PROJECT_ROOT,
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = (await new Response(proc.stdout).text()).trim();
  const stderr = (await new Response(proc.stderr).text()).trim();
  const exitCode = await proc.exited;
  let parsed: JsonObject;
  try {
    parsed = stdout ? (JSON.parse(stdout) as JsonObject) : {};
  } catch {
    parsed = { raw: stdout };
  }
  if (exitCode !== 0) {
    throw new Error(JSON.stringify({ exitCode, stdout: parsed, stderr }));
  }
  return parsed;
}

async function captureSurface(
  session: string,
  entry: NavigatorCase,
  outPath: string,
  resolved: ResolvedSurfaceTarget,
  expectedPopupCaptureStrategy?: "parent_capture_with_crop",
): Promise<JsonObject> {
  const outDir = outPath.slice(0, outPath.lastIndexOf("/"));
  if (outDir && !existsSync(outDir)) {
    mkdirSync(outDir, { recursive: true });
  }

  const receipt = await runTool([
    "bun",
    "scripts/agentic/verify-shot.ts",
    "--session",
    session,
    "--label",
    entry.viewName,
    "--out",
    outPath,
    "--target-json",
    JSON.stringify(resolved.targetJson),
    "--capture-window-id",
    String(resolved.osWindowId),
    "--strict-window",
    "--skip-state",
    "--skip-probe",
    "--json",
  ]);
  if (expectedPopupCaptureStrategy) {
    const popupCapture =
      typeof receipt.popupCapture === "object" && receipt.popupCapture != null
        ? (receipt.popupCapture as JsonObject)
        : null;
    if (popupCapture?.strategy !== expectedPopupCaptureStrategy) {
      throw new Error(
        `${entry.id}: expected popupCapture.strategy ${expectedPopupCaptureStrategy}, got ${JSON.stringify(popupCapture)}`,
      );
    }
    if (typeof popupCapture.targetBounds !== "object" || popupCapture.targetBounds == null) {
      throw new Error(`${entry.id}: expected popupCapture.targetBounds for attached popup`);
    }
  }
  return receipt;
}

function jsonOutput(step?: StepReceipt): JsonObject | null {
  return typeof step?.output === "object" && step.output != null
    ? (step.output as JsonObject)
    : null;
}

function buildManifest(opts: NavigatorOptions, caseReceipts: JsonObject[]): JsonObject {
  const entries = caseReceipts.map((receipt) => {
    const steps = Array.isArray(receipt.steps) ? (receipt.steps as StepReceipt[]) : [];
    const screenshotStep = steps.find((step) => step.name === "strict-screenshot");
    const preCaptureStep = steps.find(
      (step) => step.name === "pre-capture-state-and-elements",
    );
    const screenshotOutput = jsonOutput(screenshotStep);
    const screenshotReceipt =
      typeof screenshotOutput?.screenshotReceipt === "object" &&
      screenshotOutput.screenshotReceipt != null
        ? (screenshotOutput.screenshotReceipt as JsonObject)
        : null;
    return {
      id: receipt.id,
      sourceGroup: receipt.sourceGroup ?? null,
      viewName: receipt.viewName,
      surfaceClass: receipt.surfaceClass ?? "filterableMain",
      promptType: receipt.promptType,
      windowKind: receipt.windowKind ?? null,
      imageLibraryName: receipt.imageLibraryName,
      imagePath: receipt.imagePath,
      receiptPath: receipt.receiptPath,
      status: receipt.status,
      hostFixture: receipt.hostFixture ?? null,
      hostSetup: receipt.hostSetup ?? null,
      hostObservation:
        typeof receipt.hostSetup === "object" && receipt.hostSetup != null
          ? ((receipt.hostSetup as JsonObject).observation ?? null)
          : null,
      hostResolvedTarget:
        typeof receipt.hostSetup === "object" && receipt.hostSetup != null
          ? ((receipt.hostSetup as JsonObject).resolvedTarget ?? null)
          : null,
      resolvedTarget: receipt.resolvedTarget ?? null,
      finalObservation: jsonOutput(preCaptureStep)?.observation ?? null,
      preCaptureInspection: receipt.preCaptureInspection ?? null,
      preCaptureElements: receipt.preCaptureElements ?? null,
      captureTarget: screenshotOutput?.captureTarget ?? null,
      popupCapture: screenshotOutput?.popupCapture ?? null,
      screenshot: screenshotOutput?.screenshot ?? null,
      contentAudit: screenshotReceipt?.contentAudit ?? null,
    };
  });
  const failedEntries = entries.filter((entry) => entry.status !== "pass");
  return {
    schemaVersion: SCHEMA_VERSION,
    generatedAt: new Date().toISOString(),
    status: failedEntries.length > 0 ? "fail" : "pass",
    session: opts.session,
    group: opts.group,
    outDir: opts.outDir,
    freshPerCase: opts.freshPerCase,
    totalCases: entries.length,
    passedCases: entries.length - failedEntries.length,
    failedCases: failedEntries.length,
    entries,
  };
}

async function runSurfaceCase(
  session: string,
  entry: FilterableSurfaceMatrixEntry,
  opts: NavigatorOptions,
  sourceGroup: SourceSurfaceGroup,
): Promise<JsonObject> {
  const started = Date.now();
  const steps: StepReceipt[] = [];

  const navigateStep = await timedStep("navigate", () =>
    navigateToSurface(session, entry, opts.timeoutMs),
  );
  steps.push(navigateStep);
  if (navigateStep.status !== "pass") {
    return {
      id: entry.id,
      sourceGroup,
      viewName: entry.viewName,
      imageLibraryName: entry.imageLibraryName,
      promptType: entry.promptType,
      status: "fail",
      steps,
      durationMs: Date.now() - started,
    };
  }
  const navigateOutput = navigateStep.output as { resolvedTarget: ResolvedSurfaceTarget };
  const exactTarget = navigateOutput.resolvedTarget.targetJson;

  steps.push(
    await timedStep("state-and-elements", () =>
      getStateAndElements(session, entry, opts.timeoutMs, "ready", exactTarget),
    ),
  );

  if (opts.interact === "safe") {
    steps.push(
      await timedStep("safe-interaction", () =>
        safeInteractWithSurface(session, entry, opts.timeoutMs, exactTarget),
      ),
    );
  } else {
    steps.push({
      name: "safe-interaction",
      status: "skipped",
      output: { reason: "--interact none" },
      durationMs: 0,
    });
  }

  steps.push(
    await timedStep("pre-capture-state-and-elements", () =>
      getStateAndElements(session, entry, opts.timeoutMs, "pre-capture", exactTarget),
    ),
  );

  if (opts.capture) {
    const outPath = surfaceOutPath(entry, opts.outDir);
    steps.push(
      await timedStep("strict-screenshot", () =>
        captureSurface(session, entry, outPath, navigateOutput.resolvedTarget),
      ),
    );
  } else {
    steps.push({
      name: "strict-screenshot",
      status: "skipped",
      output: { reason: "--capture not set" },
      durationMs: 0,
    });
  }

  const failed = steps.find((step) => step.status === "fail" || step.status === "error");
  const caseReceipt = {
    id: entry.id,
    session,
    sourceGroup,
    viewName: entry.viewName,
    imageLibraryName: entry.imageLibraryName,
    imagePath: opts.capture ? surfaceOutPath(entry, opts.outDir) : null,
    receiptPath: opts.capture ? receiptOutPath(entry, opts.outDir) : null,
    promptType: entry.promptType,
    resolvedTarget: navigateOutput.resolvedTarget,
    status: failed ? "fail" : "pass",
    steps,
    durationMs: Date.now() - started,
  };
  if (opts.capture) {
    if (caseReceipt.imagePath && existsSync(caseReceipt.imagePath)) {
      caseReceipt.steps.push({
        name: "image-file",
        status: "pass",
        output: {
          path: caseReceipt.imagePath,
          sizeBytes: statSync(caseReceipt.imagePath).size,
        },
        durationMs: 0,
      });
    }
    writeJsonFile(receiptOutPath(entry, opts.outDir), caseReceipt);
  }
  return caseReceipt;
}

async function runAttachedPopupCase(
  session: string,
  entry: AttachedPopupSurfaceEntry,
  opts: NavigatorOptions,
  sourceGroup: SourceSurfaceGroup,
): Promise<JsonObject> {
  const started = Date.now();
  const steps: StepReceipt[] = [];

  const navigateStep = await timedStep("navigate-attached-popup", () =>
    navigateToAttachedPopup(session, entry, opts.timeoutMs),
  );
  steps.push(navigateStep);
  if (navigateStep.status !== "pass") {
    return {
      id: entry.id,
      sourceGroup,
      surfaceClass: entry.surfaceClass,
      viewName: entry.viewName,
      imageLibraryName: entry.imageLibraryName,
      windowKind: entry.windowKind,
      status: "fail",
      steps,
      durationMs: Date.now() - started,
    };
  }

  const navigateOutput = navigateStep.output as {
    hostSetup: AttachedPopupHostSetup | null;
    resolvedTarget: ResolvedSurfaceTarget;
    preCaptureInspection: JsonObject;
  };
  const exactTarget = navigateOutput.resolvedTarget.targetJson;

  const elementsStep = await timedStep("pre-capture-elements", () =>
    getElementsForTarget(session, entry, opts.timeoutMs, "pre-capture", exactTarget),
  );
  steps.push(elementsStep);

  steps.push({
    name: "safe-interaction",
    status: "skipped",
    output: { reason: "attached popup safe interactions disabled" },
    durationMs: 0,
  });

  if (opts.capture) {
    const outPath = surfaceOutPath(entry, opts.outDir);
    steps.push(
      await timedStep("strict-screenshot", () =>
        captureSurface(
          session,
          entry,
          outPath,
          navigateOutput.resolvedTarget,
          entry.expectedPopupCaptureStrategy,
        ),
      ),
    );
  } else {
    steps.push({
      name: "strict-screenshot",
      status: "skipped",
      output: { reason: "--capture not set" },
      durationMs: 0,
    });
  }

  const failed = steps.find((step) => step.status === "fail" || step.status === "error");
  const caseReceipt = {
    id: entry.id,
    session,
    sourceGroup,
    surfaceClass: entry.surfaceClass,
    viewName: entry.viewName,
    imageLibraryName: entry.imageLibraryName,
    imagePath: opts.capture ? surfaceOutPath(entry, opts.outDir) : null,
    receiptPath: opts.capture ? receiptOutPath(entry, opts.outDir) : null,
    windowKind: entry.windowKind,
    hostFixture: entry.hostFixture ?? null,
    hostSetup: navigateOutput.hostSetup,
    resolvedTarget: navigateOutput.resolvedTarget,
    preCaptureInspection: navigateOutput.preCaptureInspection,
    preCaptureElements: jsonOutput(elementsStep),
    status: failed ? "fail" : "pass",
    steps,
    durationMs: Date.now() - started,
  };
  if (opts.capture) {
    if (caseReceipt.imagePath && existsSync(caseReceipt.imagePath)) {
      caseReceipt.steps.push({
        name: "image-file",
        status: "pass",
        output: {
          path: caseReceipt.imagePath,
          sizeBytes: statSync(caseReceipt.imagePath).size,
        },
        durationMs: 0,
      });
    }
    writeJsonFile(receiptOutPath(entry, opts.outDir), caseReceipt);
  }
  return caseReceipt;
}

function selectedNavigatorCases(opts: NavigatorOptions): SelectedNavigatorCase[] {
  if (opts.group === "all-active") {
    if (opts.caseId !== "all") {
      throw new Error("--group all-active currently supports --case all only");
    }
    return [
      ...selectedCases("all").map((entry) => ({
        sourceGroup: "filterable-main" as const,
        entry,
      })),
      ...selectedAttachedPopupCases("all").map((entry) => ({
        sourceGroup: "attached-popup" as const,
        entry,
      })),
    ];
  }
  if (opts.group === "attached-popup") {
    return selectedAttachedPopupCases(opts.caseId).map((entry) => ({
      sourceGroup: "attached-popup" as const,
      entry,
    }));
  }
  return selectedCases(opts.caseId).map((entry) => ({
    sourceGroup: "filterable-main" as const,
    entry,
  }));
}

async function runNavigatorCase(
  session: string,
  selected: SelectedNavigatorCase,
  opts: NavigatorOptions,
): Promise<JsonObject> {
  return selected.sourceGroup === "attached-popup"
    ? runAttachedPopupCase(
        session,
        selected.entry as AttachedPopupSurfaceEntry,
        opts,
        selected.sourceGroup,
      )
    : runSurfaceCase(
        session,
        selected.entry as FilterableSurfaceMatrixEntry,
        opts,
        selected.sourceGroup,
      );
}

async function main(): Promise<void> {
  const opts = parseArgs();

  if (opts.list) {
    const matrix =
      opts.group === "all-active"
        ? selectedNavigatorCases(opts).map((selected) => ({
            sourceGroup: selected.sourceGroup,
            ...(selected.entry as JsonObject),
          }))
        : opts.group === "attached-popup"
          ? ATTACHED_POPUP_SURFACE_MATRIX.map((entry) => ({
              sourceGroup: "attached-popup",
              ...(entry as JsonObject),
            }))
          : FILTERABLE_SURFACE_MATRIX.map((entry) => ({
              sourceGroup: "filterable-main",
              ...(entry as JsonObject),
            }));
    process.stdout.write(
      `${JSON.stringify({
        schemaVersion: SCHEMA_VERSION,
        status: "pass",
        group: opts.group,
        matrix,
      }, null, 2)}\n`,
    );
    return;
  }

  const cases = selectedNavigatorCases(opts);
  const activeSessions = new Set<string>();
  const caseReceipts: JsonObject[] = [];
  const startReceipts: JsonObject[] = [];

  try {
    if (opts.freshPerCase) {
      for (const selected of cases) {
        const entry = selected.entry;
        const caseSession =
          cases.length === 1
            ? opts.session
            : `${opts.session}-${selected.sourceGroup}-${entry.viewName}`;
        const startReceipt = await startOrReuseSession(caseSession);
        const ownsCaseSession = startReceipt.resumed !== true;
        if (ownsCaseSession) activeSessions.add(caseSession);
        startReceipts.push({
          caseId: entry.id,
          sourceGroup: selected.sourceGroup,
          session: caseSession,
          startReceipt,
        });
        caseReceipts.push(await runNavigatorCase(caseSession, selected, opts));
        if (ownsCaseSession) {
          await sessionStop(caseSession);
          activeSessions.delete(caseSession);
        }
      }
    } else {
      const startReceipt = await startOrReuseSession(opts.session);
      const ownsSession = startReceipt.resumed !== true;
      if (ownsSession) activeSessions.add(opts.session);
      startReceipts.push({ session: opts.session, startReceipt });
      for (const selected of cases) {
        caseReceipts.push(await runNavigatorCase(opts.session, selected, opts));
      }
      if (!opts.keepSession && ownsSession) {
        await sessionStop(opts.session);
        activeSessions.delete(opts.session);
      }
    }
    let manifest: JsonObject | null = null;
    if (opts.capture) {
      manifest = buildManifest(opts, caseReceipts);
      writeJsonFile(manifestOutPath(opts), manifest);
    }

    const failed = caseReceipts.find((receipt) => receipt.status !== "pass");
    process.stdout.write(
      `${JSON.stringify({
        schemaVersion: SCHEMA_VERSION,
        status: failed ? "fail" : "pass",
        session: opts.session,
        group: opts.group,
        startReceipts,
        freshPerCase: opts.freshPerCase,
        keepSession: opts.keepSession,
        outDir: opts.outDir,
        manifestPath: opts.capture ? manifestOutPath(opts) : null,
        manifest,
        cases: caseReceipts,
      }, null, 2)}\n`,
    );
    process.exit(failed ? 1 : 0);
  } catch (error) {
    if (!opts.keepSession) {
      for (const session of activeSessions) {
        await sessionStop(session).catch(() => undefined);
      }
    }
    process.stdout.write(
      `${JSON.stringify({
        schemaVersion: SCHEMA_VERSION,
        status: "error",
        session: opts.session,
        error: error instanceof Error ? error.message : String(error),
        cases: caseReceipts,
      }, null, 2)}\n`,
    );
    process.exit(2);
  }
}

if (import.meta.main) {
  await main();
}
