#!/usr/bin/env bun

import {
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveSessionCommand,
} from "./lib/operator-safety.ts";
import { privateReceiptFingerprint } from "./lib/privacy.ts";

type JsonObject = Record<string, unknown>;

type Args = {
  session: string;
  timeoutMs: number;
  start: boolean;
  show: boolean;
  proveOpenCloseFreshness: boolean;
  proveEarlyFrameFreshness: boolean;
  sampleMs: number;
  intervalMs: number;
};

const MAIN_TARGET_ARGS = ["--main", "--strict", "--surface", "ScriptList"];

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/main.ts inspect [--session <name>] [--start] [--show] [--prove-open-close-freshness] [--prove-early-frame-freshness] [--timeout <ms>]",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  if (argv[0] !== "inspect") {
    console.error(usage());
    process.exit(2);
  }

  const args: Args = {
    session: "default",
    timeoutMs: 8000,
    start: false,
    show: false,
    proveOpenCloseFreshness: false,
    proveEarlyFrameFreshness: false,
    sampleMs: 900,
    intervalMs: 50,
  };
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--show") {
      args.show = true;
    } else if (arg === "--prove-open-close-freshness") {
      args.proveOpenCloseFreshness = true;
    } else if (arg === "--prove-early-frame-freshness") {
      args.proveEarlyFrameFreshness = true;
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
    } else if (arg === "--sample-ms") {
      args.sampleMs = Number(argv[++index] ?? args.sampleMs);
    } else if (arg === "--interval-ms") {
      args.intervalMs = Number(argv[++index] ?? args.intervalMs);
    }
  }
  return args;
}

async function run(command: string[], label: string): Promise<JsonObject> {
  assertNoninteractiveSessionCommand(command);
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (exitCode !== 0) {
    return { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
  }
  try {
    return JSON.parse(stdout);
  } catch {
    return { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim(), error: "invalid_json_output" };
  }
}

function requestId(prefix: string) {
  return `devtools-main-${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function rpc(session: string, payload: JsonObject, expect: string, timeoutMs: number) {
  assertNoninteractiveProtocolCommand(payload);
  return run(["bash", "scripts/agentic/session.sh", "rpc", session, JSON.stringify(payload), "--expect", expect, "--timeout", String(timeoutMs)], String(payload.type ?? "rpc"));
}

async function send(session: string, payload: JsonObject, timeoutMs: number) {
  return run([
    "bash",
    "scripts/agentic/session.sh",
    "send",
    session,
    JSON.stringify({ requestId: requestId(String(payload.type ?? "send")), ...payload }),
    "--await-parse",
    "--timeout",
    String(timeoutMs),
  ], String(payload.type ?? "send"));
}

function responseOf(envelope: JsonObject): JsonObject {
  return (envelope.response as JsonObject | undefined) ?? envelope;
}

function asArray(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.filter((entry): entry is JsonObject => typeof entry === "object" && entry !== null) : [];
}

async function maybeStartSession(args: Args) {
  if (!args.start) return null;
  return run(["bash", "scripts/agentic/session.sh", "start", args.session], "session.start");
}

async function maybeShowMain(args: Args) {
  if (!args.show) return null;
  return showMain(args);
}

async function inspectMain(args: Args, label: string): Promise<JsonObject> {
  return run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    ...MAIN_TARGET_ARGS,
    "--timeout",
    String(args.timeoutMs),
  ], label);
}

async function getMainState(args: Args, label: string): Promise<JsonObject> {
  return rpc(args.session, {
    type: "getState",
    requestId: requestId(label),
    target: { type: "main" },
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
}

function fingerprintText(value: unknown): string | null {
  if (typeof value !== "string") return null;
  return `sha256:${privateReceiptFingerprint(value)}`;
}

function mainInputValue(state: JsonObject) {
  if (typeof state.inputValue === "string") return state.inputValue;
  const filter = state.filter as JsonObject | undefined;
  if (typeof filter?.value === "string") return filter.value;
  const input = state.input as JsonObject | undefined;
  if (typeof input?.value === "string") return input.value;
  return "";
}

function summarizeMainState(stateReceipt: JsonObject, targetReceipt: JsonObject, markerFingerprint?: string | null) {
  const state = responseOf(stateReceipt);
  const target = (targetReceipt.resolvedTarget as JsonObject | undefined) ?? {};
  const surfaceContract = (state.surfaceContract as JsonObject | undefined) ?? {};
  const activeFooter = (state.activeFooter as JsonObject | undefined) ?? {};
  const inputValue = mainInputValue(state);
  const inputFingerprint = fingerprintText(inputValue);
  return {
    target: {
      automationId: target.automationId ?? null,
      targetKind: target.targetKind ?? null,
      surfaceKind: target.surfaceKind ?? null,
      semanticSurface: target.semanticSurface ?? null,
      appViewVariant: target.appViewVariant ?? null,
      routeId: target.routeId ?? null,
      routeStack: target.routeStack ?? [],
      nativeFooterSurface: target.nativeFooterSurface ?? null,
      targetGeneration: target.targetGeneration ?? null,
      surfaceGeneration: target.surfaceGeneration ?? null,
      dataGeneration: target.dataGeneration ?? null,
      visible: target.visible ?? null,
      focused: target.focused ?? null,
      strictTargetMatch: target.strictTargetMatch ?? null,
    },
    state: {
      promptType: state.promptType ?? null,
      promptId: state.promptId ?? null,
      surfaceKind: surfaceContract.surfaceKind ?? target.surfaceKind ?? null,
      surfaceContract: {
        surfaceKind: surfaceContract.surfaceKind ?? null,
        keyboardPolicy: surfaceContract.keyboardPolicy ?? null,
        inputOwnership: surfaceContract.inputOwnership ?? null,
        focusPolicy: surfaceContract.focusPolicy ?? null,
      },
      inputLength: inputValue.length,
      inputFingerprint,
      visibleChoiceCount: state.visibleChoiceCount ?? null,
      selectedIndex: state.selectedIndex ?? null,
      windowVisible: state.windowVisible ?? null,
      activePopupPresent: Boolean(state.activePopupContract),
      activeFooterOwner: activeFooter.owner ?? null,
      activeFooter: {
        owner: activeFooter.owner ?? null,
        activeSurface: activeFooter.activeSurface ?? null,
        expectedSurface: activeFooter.expectedSurface ?? null,
        nativeFooterHostInstalled: activeFooter.nativeFooterHostInstalled ?? null,
        buttonCount: activeFooter.buttonCount ?? asArray(activeFooter.buttons).length,
        actionSlotCount: activeFooter.actionSlotCount ?? null,
        contextChipCount: activeFooter.contextChipCount ?? null,
        duplicateShortcutKeys: activeFooter.duplicateShortcutKeys ?? [],
        slotContractViolation: activeFooter.slotContractViolation ?? null,
      },
    },
    staleInputObserved: markerFingerprint != null && inputFingerprint === markerFingerprint,
  };
}

async function setMainMarker(args: Args, marker: string) {
  return run([
    "bun",
    "scripts/devtools/act.ts",
    "set-input",
    "--session",
    args.session,
    "--text",
    marker,
    ...MAIN_TARGET_ARGS,
    "--timeout",
    String(args.timeoutMs),
  ], "act.set-input.marker");
}

async function closeMain(args: Args) {
  const first = await run([
    "bun",
    "scripts/devtools/act.ts",
    "key",
    "--session",
    args.session,
    "--key",
    "Escape",
    ...MAIN_TARGET_ARGS,
    "--timeout",
    String(args.timeoutMs),
  ], "act.key.escape.close-main.first");
  const afterFirstTarget = await inspectMain(args, "targets.inspect.main.after-first-escape");
  const afterFirstState = await getMainState(args, "main-state-after-first-escape");
  const afterFirst = summarizeMainState(afterFirstState, afterFirstTarget);
  if (afterFirst.target.visible === false || afterFirst.state.windowVisible === false) {
    return { classification: "ok", first, second: null, afterFirstTarget, afterFirstState };
  }
  const second = await run([
    "bun",
    "scripts/devtools/act.ts",
    "key",
    "--session",
    args.session,
    "--key",
    "Escape",
    ...MAIN_TARGET_ARGS,
    "--timeout",
    String(args.timeoutMs),
  ], "act.key.escape.close-main.second");
  return { classification: second.classification ?? second.status ?? "ok", first, second, afterFirstTarget, afterFirstState };
}

async function showMain(args: Args) {
  return send(args.session, { type: "show" }, args.timeoutMs);
}

async function sampleMainAfterReopen(args: Args, markerFingerprint: string | null) {
  const samples = [];
  const startedAt = Date.now();
  while (samples.length < 3 || Date.now() - startedAt < args.sampleMs) {
    const target = await inspectMain(args, "targets.inspect.main.sample");
    const state = await getMainState(args, "main-state-sample");
    samples.push({
      elapsedMs: Date.now() - startedAt,
      ...summarizeMainState(state, target, markerFingerprint),
    });
    await Bun.sleep(args.intervalMs);
  }
  return samples;
}

async function runOpenCloseFreshnessProof(args: Args) {
  const marker = `__dt_main_freshness_${Date.now()}_${Math.random().toString(16).slice(2)}__`;
  const markerFingerprint = fingerprintText(marker);
  const beforeTarget = await inspectMain(args, "targets.inspect.main.before");
  const beforeState = await getMainState(args, "main-state-before");
  const setMarker = await setMainMarker(args, marker);
  const markedTarget = await inspectMain(args, "targets.inspect.main.marked");
  const markedState = await getMainState(args, "main-state-marked");
  const markedSummary = summarizeMainState(markedState, markedTarget, markerFingerprint);
  const closeReceipt = await closeMain(args);
  const closedTarget = await inspectMain(args, "targets.inspect.main.closed");
  const closedState = await getMainState(args, "main-state-closed");
  const showReceipt = await showMain(args);
  const samplesAfterReopen = await sampleMainAfterReopen(args, markerFingerprint);
  const closedSummary = summarizeMainState(closedState, closedTarget, markerFingerprint);
  const markerApplied = markedSummary.staleInputObserved === true;
  const closeObserved = closedSummary.target.visible === false
    || closedSummary.state.windowVisible === false
    || closeReceipt.classification === "ok";
  const reopenVisible = samplesAfterReopen.some((sample) =>
    sample.target.visible === true || sample.state.windowVisible === true
  );
  const noStaleInputValue = samplesAfterReopen.every((sample) => sample.staleInputObserved === false);
  const targetStable = samplesAfterReopen.every((sample) =>
    sample.target.automationId === "main"
    && sample.target.strictTargetMatch === true
    && (sample.target.surfaceKind === "ScriptList" || sample.state.surfaceKind === "ScriptList")
  );
  const assertions = {
    markerApplied,
    closeObserved,
    reopenVisible,
    noStaleInputValue,
    targetStable,
    sampledReopenFrames: samplesAfterReopen.length >= 3,
  };
  const ok = Object.values(assertions).every(Boolean);
  return {
    classification: ok ? "ok" : "blocked-by-stale-view",
    command: "main.openCloseFreshnessProof",
    marker: {
      inputLength: marker.length,
      inputFingerprint: markerFingerprint,
      rawValueRedacted: true,
    },
    assertions,
    samplesAfterReopen,
    receipts: {
      beforeTarget,
      beforeState,
      setMarker,
      markedTarget,
      markedState,
      closeReceipt,
      closedTarget,
      closedState,
      showReceipt,
    },
  };
}

function numericValues(samples: JsonObject[], path: (sample: JsonObject) => unknown) {
  return samples
    .map(path)
    .filter((value): value is number => typeof value === "number" && Number.isFinite(value));
}

function monotonicNonDecreasing(values: number[]) {
  return values.every((value, index) => index === 0 || value >= values[index - 1]);
}

function summarizeGenerationSeries(samples: JsonObject[]) {
  const targetGeneration = numericValues(samples, (sample) => (sample.target as JsonObject | undefined)?.targetGeneration);
  const surfaceGeneration = numericValues(samples, (sample) => (sample.target as JsonObject | undefined)?.surfaceGeneration);
  const dataGeneration = numericValues(samples, (sample) => (sample.target as JsonObject | undefined)?.dataGeneration);
  const generationFieldsAvailable = targetGeneration.length >= 2
    && surfaceGeneration.length >= 2
    && dataGeneration.length >= 2;
  const generationMonotonic = monotonicNonDecreasing(targetGeneration)
    && monotonicNonDecreasing(surfaceGeneration)
    && monotonicNonDecreasing(dataGeneration);
  return {
    targetGeneration,
    surfaceGeneration,
    dataGeneration,
    fieldsChecked: [
      targetGeneration.length >= 2 ? "targetGeneration" : "",
      surfaceGeneration.length >= 2 ? "surfaceGeneration" : "",
      dataGeneration.length >= 2 ? "dataGeneration" : "",
    ].filter(Boolean),
    missingOrSingleSampleFields: [
      targetGeneration.length < 2 ? "targetGeneration" : "",
      surfaceGeneration.length < 2 ? "surfaceGeneration" : "",
      dataGeneration.length < 2 ? "dataGeneration" : "",
    ].filter(Boolean),
    generationFieldsAvailable,
    generationMonotonic,
    monotonicWhenAvailable: generationMonotonic,
  };
}

function footerSurfaceFresh(sample: JsonObject) {
  const state = (sample.state as JsonObject | undefined) ?? {};
  const footer = (state.activeFooter as JsonObject | undefined) ?? {};
  const activeSurface = footer.activeSurface;
  const expectedSurface = footer.expectedSurface;
  const slotViolation = footer.slotContractViolation;
  const duplicates = footer.duplicateShortcutKeys;
  return (expectedSurface == null || activeSurface == null || activeSurface === expectedSurface)
    && (slotViolation == null || slotViolation === false)
    && (!Array.isArray(duplicates) || duplicates.length === 0);
}

function sampleSurfaceFresh(sample: JsonObject) {
  const target = (sample.target as JsonObject | undefined) ?? {};
  const state = (sample.state as JsonObject | undefined) ?? {};
  return target.automationId === "main"
    && target.strictTargetMatch === true
    && (target.surfaceKind === "ScriptList" || state.surfaceKind === "ScriptList");
}

function buildEarlyFrameFreshnessProof(openCloseFreshnessProof: JsonObject | null) {
  const samples = asArray(openCloseFreshnessProof?.samplesAfterReopen);
  const marker = (openCloseFreshnessProof?.marker as JsonObject | undefined) ?? {};
  const firstVisibleFrame = samples.find((sample) => {
    const target = (sample.target as JsonObject | undefined) ?? {};
    const state = (sample.state as JsonObject | undefined) ?? {};
    return target.visible === true || state.windowVisible === true;
  }) ?? null;
  const generation = summarizeGenerationSeries(samples);
  const assertions = {
    baseOpenCloseProofOk: openCloseFreshnessProof?.classification === "ok",
    sampledEarlyFrames: samples.length >= 3,
    firstVisibleFrameFresh: firstVisibleFrame != null && sampleSurfaceFresh(firstVisibleFrame),
    everySampleTargetStable: samples.every(sampleSurfaceFresh),
    noStaleInputValue: samples.every((sample) => sample.staleInputObserved === false),
    noPromptIdOnReopen: samples.every((sample) => ((sample.state as JsonObject | undefined)?.promptId ?? null) == null),
    noActivePopupOnReopen: samples.every((sample) => ((sample.state as JsonObject | undefined)?.activePopupPresent ?? false) === false),
    footerSurfaceFresh: samples.every(footerSurfaceFresh),
    generationFieldsAvailable: generation.generationFieldsAvailable,
    generationMonotonic: generation.generationMonotonic,
    generationMonotonicWhenAvailable: generation.monotonicWhenAvailable,
  };
  const ok = Object.values(assertions).every(Boolean);
  const classification = ok
    ? "ok"
    : generation.generationFieldsAvailable === false
      ? "blocked-by-missing-primitive"
      : "blocked-by-stale-view";
  return {
    classification,
    command: "main.earlyFrameFreshnessProof",
    marker: {
      inputFingerprint: marker.inputFingerprint ?? null,
      rawValueRedacted: marker.rawValueRedacted === true,
    },
    assertions,
    generation,
    firstVisibleFrame,
    samplesAfterReopen: samples,
  };
}

function classifyMainReceipt(
  targetReceipt: JsonObject,
  stateReceipt: JsonObject,
  proof: JsonObject | null,
  earlyFrameProof: JsonObject | null,
) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  if (stateReceipt.status === "error") {
    return "blocked-by-timeout";
  }
  if (proof?.classification && proof.classification !== "ok") {
    return proof.classification;
  }
  if (earlyFrameProof?.classification && earlyFrameProof.classification !== "ok") {
    return earlyFrameProof.classification;
  }
  return "ok";
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  const startReceipt = await maybeStartSession(args);
  const showReceipt = await maybeShowMain(args);
  const targetReceipt = await inspectMain(args, "targets.inspect.main");
  const stateReceipt = await getMainState(args, "main-state");
  const stateSummary = summarizeMainState(stateReceipt, targetReceipt);
  const shouldRunOpenCloseFreshness = args.proveOpenCloseFreshness || args.proveEarlyFrameFreshness;
  const openCloseFreshnessProof = shouldRunOpenCloseFreshness
    ? await runOpenCloseFreshnessProof(args)
    : null;
  const earlyFrameFreshnessProof = args.proveEarlyFrameFreshness
    ? buildEarlyFrameFreshnessProof(openCloseFreshnessProof)
    : null;
  const classification = classifyMainReceipt(targetReceipt, stateReceipt, openCloseFreshnessProof, earlyFrameFreshnessProof);
  console.log(JSON.stringify({
    schemaVersion: 1,
    tool: "script-kit-devtools.main",
    command: "main.inspect",
    classification,
    session: args.session,
    target: stateSummary.target,
    state: stateSummary.state,
    openCloseFreshnessProof,
    earlyFrameFreshnessProof,
    receipts: { startReceipt, showReceipt, target: targetReceipt, state: stateReceipt },
    missingPrimitives: [],
    warnings: [],
    errors: [targetReceipt, stateReceipt].filter((receipt) => receipt.status === "error"),
  }, null, 2));
}

await main();
