#!/usr/bin/env bun

import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic, privateReceiptFingerprint } from "./lib/privacy.ts";
import { resolveTargetReceipt } from "./lib/target-identity.ts";
import {
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveSessionCommand,
} from "./lib/operator-safety.ts";

type JsonObject = Record<string, unknown>;

type Args = {
  command: "inspect" | "deliver-fixture";
  session: string;
  includeEnvPayload: boolean;
  start: boolean;
  show: boolean;
  openOverlayFixture: boolean;
  keepOpen: boolean;
  timeoutMs: number;
  target: string;
  fixtureId: string;
  expectRefusal: boolean;
};

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/dictation.ts inspect [--session <name>] [--start] [--show] [--open-overlay-fixture] [--keep-open] [--include-env-payload]",
    "  bun scripts/devtools/dictation.ts deliver-fixture [--session <name>] [--start] [--show] [--target <label>] [--fixture-id <id>] [--expect-refusal]",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }
  if (argv[0] !== "inspect" && argv[0] !== "deliver-fixture") {
    console.error(usage());
    process.exit(2);
  }
  const args: Args = {
    command: argv[0],
    session: "default",
    includeEnvPayload: false,
    start: false,
    show: false,
    openOverlayFixture: false,
    keepOpen: false,
    timeoutMs: 8000,
    target: "mainWindowFilter",
    fixtureId: "short-phrase",
    expectRefusal: false,
  };
  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
    } else if (arg === "--include-env-payload") {
      args.includeEnvPayload = true;
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--show") {
      args.show = true;
    } else if (arg === "--open-overlay-fixture") {
      args.openOverlayFixture = true;
    } else if (arg === "--keep-open") {
      args.keepOpen = true;
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
    } else if (arg === "--target") {
      args.target = argv[++index] ?? args.target;
    } else if (arg === "--fixture-id") {
      args.fixtureId = argv[++index] ?? args.fixtureId;
    } else if (arg === "--expect-refusal") {
      args.expectRefusal = true;
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
    return { status: "ok", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
  }
}

async function maybeStartAndShow(args: Args) {
  if (args.start) {
    await run(["bash", "scripts/agentic/session.sh", "start", args.session], "session-start");
  }
  if (args.show) {
    await run(["bash", "scripts/agentic/session.sh", "send", args.session, JSON.stringify({ type: "show" }), "--await-parse", "--timeout", String(args.timeoutMs)], "session-show");
  }
}

async function openOverlayFixture(args: Args) {
  if (!args.openOverlayFixture) {
    return null;
  }
  const requestId = `devtools-dictation-overlay-fixture-${Date.now()}`;
  const openReceipt = await run([
    "bash",
    "scripts/agentic/session.sh",
    "send",
    args.session,
    JSON.stringify({
      type: "openDictationOverlayFixture",
      requestId,
    }),
    "--await-parse",
    "--timeout",
    String(args.timeoutMs),
  ], "openDictationOverlayFixture");

  let targetReceipt: JsonObject | null = null;
  for (let attempt = 0; attempt < 6; attempt += 1) {
    targetReceipt = await run([
      "bun",
      "scripts/devtools/targets.ts",
      "inspect",
      "--session",
      args.session,
      "--target-kind",
      "dictation",
      "--surface",
      "Dictation",
      "--strict",
      "--timeout",
      String(args.timeoutMs),
    ], "targets.inspect.dictation");
    if (targetReceipt.classification === "ok") {
      break;
    }
    await Bun.sleep(50);
  }

  return {
    requestId,
    keepOpen: args.keepOpen,
    noMicrophoneCaptureRequired: true,
    command: "openDictationOverlayFixture",
    openReceipt,
    targetReceipt,
  };
}

async function rpc(session: string, payload: JsonObject, expect: string, timeoutMs: number) {
  assertNoninteractiveProtocolCommand(payload);
  return run(["bash", "scripts/agentic/session.sh", "rpc", session, JSON.stringify(payload), "--expect", expect, "--timeout", String(timeoutMs)], String(payload.type ?? "rpc"));
}

async function read(path: string) {
  return Bun.file(path).text();
}

function asArray(value: unknown): JsonObject[] {
  return Array.isArray(value) ? value.filter((entry): entry is JsonObject => typeof entry === "object" && entry !== null) : [];
}

function surface(coverage: JsonObject, id: string) {
  return asArray(coverage.surfaces).find((entry) => entry.id === id) ?? {};
}

function enumVariants(source: string, enumName: string) {
  const match = new RegExp(`pub enum ${enumName} \\{([\\s\\S]*?)\\n\\}`, "m").exec(source);
  if (!match) {
    return [];
  }
  return match[1]
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("///") && !line.startsWith("#"))
    .map((line) => line.replace(/\(.*$/, "").replace(/,.*/, "").trim())
    .filter(Boolean);
}

function hasRealProviderData(value: unknown) {
  if (!value || typeof value !== "object") {
    return false;
  }
  const object = value as JsonObject;
  if (object.available === false) {
    return false;
  }
  if (Array.isArray(object.items) && object.items.length > 0) {
    return true;
  }
  const envelopeKeys = new Set(["schemaVersion", "type", "ok", "available", "source", "items", "note", "nextStep"]);
  return Object.keys(object).some((key) => !envelopeKeys.has(key));
}

function providerResource(includeEnvPayload: boolean) {
  const raw = process.env.SCRIPT_KIT_DICTATION_JSON ?? "";
  let parsed: JsonObject | null = null;
  let parseError: string | null = null;
  if (raw) {
    try {
      parsed = JSON.parse(raw) as JsonObject;
    } catch (error) {
      parseError = error instanceof Error ? error.message : String(error);
    }
  }
  return {
    uri: "kit://dictation",
    envVar: "SCRIPT_KIT_DICTATION_JSON",
    source: raw ? "env" : "none",
    available: Boolean(raw && parsed && hasRealProviderData(parsed)),
    parseError,
    payloadFingerprint: raw ? fingerprint(raw) : null,
    payloadLength: raw.length,
    payload: includeEnvPayload ? parsed : null,
  };
}

function fingerprint(value: string) {
  return privateReceiptFingerprint(value);
}

function stdinEvidence(stdinSource: string) {
  return {
    pushDictationResultVerb: stdinSource.includes("PushDictationResult") && stdinSource.includes("\"pushDictationResult\""),
    acceptedTargetLabels: [
      "mainWindowFilter",
      "mainWindowPrompt",
      "notesEditor",
      "aiChatComposer",
      "tabAiHarness",
      "externalApp",
    ],
  };
}

function fixtureTranscript(id: string) {
  if (id === "short-phrase") {
    return "DevTools fixture transcript";
  }
  if (id === "punctuation") {
    return "DevTools fixture transcript, with punctuation.";
  }
  if (id === "multiline") {
    return "DevTools fixture transcript\nwith a second line";
  }
  throw new Error(`Unknown dictation fixture id: ${id}`);
}

function classify(media: JsonObject, sourceEvidence: JsonObject, provider: JsonObject, runtimeState: JsonObject | null) {
  if (media.status === "ok") {
    return "ok";
  }
  if (!runtimeState?.dictation) {
    return "blocked-by-missing-primitive";
  }
  if (!(sourceEvidence.stdin as JsonObject | undefined)?.pushDictationResultVerb) {
    return "blocked-by-missing-primitive";
  }
  if (provider.parseError) {
    return "blocked-by-real-data-risk";
  }
  return "blocked-by-missing-primitive";
}

function hasDeliveryReceipt(runtimeState: JsonObject | null) {
  const receipt = runtimeState?.dictation && typeof runtimeState.dictation === "object"
    ? (runtimeState.dictation as JsonObject).lastDelivery
    : null;
  if (!receipt || typeof receipt !== "object") {
    return false;
  }
  const object = receipt as JsonObject;
  return Boolean(
    object.generation
      && object.target
      && object.destination
      && object.transcriptFingerprint
      && object.transcriptLen !== undefined
      && object.redacted === true,
  );
}

function deliveryGeneration(runtimeState: JsonObject | null) {
  const receipt = runtimeState?.dictation && typeof runtimeState.dictation === "object"
    ? (runtimeState.dictation as JsonObject).lastDelivery
    : null;
  if (!receipt || typeof receipt !== "object") {
    return 0;
  }
  const generation = (receipt as JsonObject).generation;
  return typeof generation === "number" ? generation : 0;
}

function deliveryReceipt(runtimeState: JsonObject | null) {
  const receipt = runtimeState?.dictation && typeof runtimeState.dictation === "object"
    ? (runtimeState.dictation as JsonObject).lastDelivery
    : null;
  return receipt && typeof receipt === "object" ? receipt as JsonObject : null;
}

function wrongTargetRefusalGeneration(runtimeState: JsonObject | null) {
  const receipt = runtimeState?.dictation && typeof runtimeState.dictation === "object"
    ? (runtimeState.dictation as JsonObject).wrongTargetRefusal
    : null;
  if (!receipt || typeof receipt !== "object") {
    return 0;
  }
  const generation = (receipt as JsonObject).generation;
  return typeof generation === "number" ? generation : 0;
}

function wrongTargetRefusalReceipt(runtimeState: JsonObject | null) {
  const receipt = runtimeState?.dictation && typeof runtimeState.dictation === "object"
    ? (runtimeState.dictation as JsonObject).wrongTargetRefusal
    : null;
  return receipt && typeof receipt === "object" ? receipt as JsonObject : null;
}

async function state(session: string, timeoutMs: number) {
  const envelope = await rpc(session, {
    type: "getState",
    requestId: `devtools-dictation-state-${Date.now()}`,
    target: { type: "main" },
    summaryOnly: true,
  }, "stateResult", timeoutMs);
  return (envelope.response as JsonObject | undefined) ?? null;
}

async function deliverFixture(args: Args) {
  await maybeStartAndShow(args);
  const targetBefore = await resolveTargetReceipt(
    {
      session: args.session,
      target: { type: "main" },
      strict: true,
      expectedSurfaceKind: "",
      timeoutMs: args.timeoutMs,
    },
    { tool: "dictation-deliver-before" },
  );
  const before = await state(args.session, args.timeoutMs);
  const beforeGeneration = deliveryGeneration(before);
  const beforeRefusalGeneration = wrongTargetRefusalGeneration(before);
  const transcript = fixtureTranscript(args.fixtureId);
  const requestId = `devtools-dictation-deliver-${Date.now()}`;
  const sendReceipt = await run([
    "bash",
    "scripts/agentic/session.sh",
    "send",
    args.session,
    JSON.stringify({
      type: "pushDictationResult",
      requestId,
      transcript,
      target: args.target,
    }),
    "--await-parse",
    "--timeout",
    String(args.timeoutMs),
  ], "pushDictationResult");
  const after = await state(args.session, args.timeoutMs);
  const targetAfter = await resolveTargetReceipt(
    {
      session: args.session,
      target: { type: "main" },
      strict: true,
      expectedSurfaceKind: "",
      timeoutMs: args.timeoutMs,
    },
    { tool: "dictation-deliver-after" },
  );
  const afterReceipt = deliveryReceipt(after);
  const afterGeneration = deliveryGeneration(after);
  const afterRefusalReceipt = wrongTargetRefusalReceipt(after);
  const afterRefusalGeneration = wrongTargetRefusalGeneration(after);
  const deliveryAdvanced = afterGeneration > beforeGeneration;
  const refusalAdvanced = afterRefusalGeneration > beforeRefusalGeneration;
  const requestedTargetMatches = String(afterReceipt?.target ?? "").toLowerCase() === args.target.toLowerCase()
    || String(afterReceipt?.targetLabel ?? "").toLowerCase() === args.target.toLowerCase();
  const redacted = afterReceipt?.redacted === true && afterReceipt?.transcriptFingerprint && afterReceipt?.transcriptLen === transcript.length;
  const insertionRange = afterReceipt?.insertionRange && typeof afterReceipt.insertionRange === "object"
    ? afterReceipt.insertionRange as JsonObject
    : null;
  const insertionRangeAvailable = insertionRange?.available === true;
  const refusalProven =
    sendReceipt.status !== "error" &&
    !deliveryAdvanced &&
    refusalAdvanced &&
    afterRefusalReceipt?.redacted === true &&
    afterRefusalReceipt?.noDeliveryAttempted === true;
  const sameWindowInstance =
    targetBefore.transaction.windowInstanceId != null
    && targetBefore.transaction.windowInstanceId === targetAfter.transaction.windowInstanceId;
  const sameProcess =
    targetBefore.transaction.pid != null
    && targetBefore.transaction.pid === targetAfter.transaction.pid
    && targetBefore.transaction.processStartTime === targetAfter.transaction.processStartTime;
  const sameBinary =
    targetBefore.transaction.binarySha256 != null
    && targetBefore.transaction.binarySha256 === targetAfter.transaction.binarySha256;
  const targetTransitionValid =
    targetBefore.classification === "ok"
    && targetAfter.classification === "ok"
    && sameWindowInstance
    && sameProcess
    && sameBinary;
  const deliveryClassification = args.expectRefusal
    ? refusalProven ? "ok" : "blocked-by-missing-primitive"
    : sendReceipt.status === "error"
      ? "blocked-by-timeout"
      : deliveryAdvanced && redacted && insertionRangeAvailable
        ? "ok"
        : "blocked-by-missing-primitive";
  const classification = targetBefore.classification !== "ok"
    ? targetBefore.classification
    : targetAfter.classification !== "ok"
      ? targetAfter.classification
      : !sameBinary
        ? "invalid-binary"
        : !sameWindowInstance || !sameProcess
          ? "blocked-by-stale-generation"
          : deliveryClassification;

  emitValidatedReceipt("devtools.dictation.deliverFixture", {
    schemaVersion: 2,
    tool: "script-kit-devtools.dictation",
    command: "dictation.deliverFixture",
    classification,
    session: args.session,
    transaction: targetBefore.transaction,
    transactionTransition: {
      valid: targetTransitionValid,
      before: targetBefore.transaction,
      after: targetAfter.transaction,
      sameWindowInstance,
      sameProcess,
      sameBinary,
    },
    safety: {
      noMicrophoneCaptureRequired: true,
      syntheticTranscriptInjected: true,
      transcriptContentReturned: false,
      fixtureId: args.fixtureId,
    },
    target: {
      requested: args.target,
      receiptTarget: afterReceipt?.target ?? null,
      receiptTargetLabel: afterReceipt?.targetLabel ?? null,
      requestedTargetMatches,
    },
    delivery: {
      requestId,
      beforeGeneration,
      afterGeneration,
      advanced: deliveryAdvanced,
      receipt: afterReceipt,
      insertionRange,
      insertionRangeAvailable,
      transcriptLenMatchesFixture: afterReceipt?.transcriptLen === transcript.length,
      transcriptFingerprintAvailable: typeof afterReceipt?.transcriptFingerprint === "string",
      rawTranscriptReturned: false,
    },
    refusal: {
      beforeGeneration: beforeRefusalGeneration,
      afterGeneration: afterRefusalGeneration,
      advanced: refusalAdvanced,
      receipt: afterRefusalReceipt,
      redacted: afterRefusalReceipt?.redacted === true,
      noDeliveryAttempted: afterRefusalReceipt?.noDeliveryAttempted === true,
    },
    missingPrimitives: [
      targetTransitionValid ? "" : "stable target proof transaction",
      args.expectRefusal
        ? ""
        : deliveryAdvanced ? "" : "target delivery generation",
      args.expectRefusal ? "" : afterReceipt?.transcriptFingerprint ? "" : "transcript fingerprint",
      args.expectRefusal ? "" : requestedTargetMatches ? "" : "requested target match",
      args.expectRefusal ? "" : insertionRangeAvailable ? "" : "cursor insertion range",
      args.expectRefusal && !refusalProven ? "wrong-target refusal receipt" : "",
    ].filter(Boolean),
    recommendedNext: [
      "Add cursor insertion range for Notes/Agent Chat/frontmost destinations.",
      "Add wrong-target refusal receipts for stale or incompatible dictation targets.",
    ],
    errors: diagnostic(
      [sendReceipt].filter((receipt) => receipt.status === "error"),
    ),
  });
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  if (args.command === "deliver-fixture") {
    await deliverFixture(args);
    return;
  }
  await maybeStartAndShow(args);
  const overlayFixture = await openOverlayFixture(args);
  const [coverage, typesSource, runtimeSource, stdinSource] = await Promise.all([
    run(["bun", "scripts/devtools/coverage.ts", "--surface", "dictation"], "coverage.dictation"),
    read("src/dictation/types.rs"),
    read("src/dictation/runtime.rs"),
    read("src/stdin_commands/mod.rs"),
  ]);
  const coveragePath = `/tmp/devtools-dictation-coverage-${process.pid}.json`;
  await Bun.write(coveragePath, JSON.stringify(coverage));
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: `devtools-dictation-state-${Date.now()}`,
    target: { type: "main" },
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
  const stateReceiptPath = `/tmp/devtools-dictation-state-${process.pid}.json`;
  await Bun.write(stateReceiptPath, JSON.stringify(stateEnvelope));
  const media = await run([
    "bun",
    "scripts/devtools/media.ts",
    "--surface",
    "dictation",
    "--coverage",
    coveragePath,
    "--receipt",
    stateReceiptPath,
  ], "media.inspect");
  const runtimeState = (stateEnvelope.response as JsonObject | undefined) ?? null;
  const dictationSurface = surface(coverage, "dictation");
  const provider = providerResource(args.includeEnvPayload);
  const deliveryReceiptAvailable = hasDeliveryReceipt(runtimeState);
  const sourceEvidence = {
    phases: enumVariants(typesSource, "DictationSessionPhase"),
    targets: enumVariants(typesSource, "DictationTarget"),
    runtimeReadApis: {
      isRecording: runtimeSource.includes("pub fn is_dictation_recording()"),
      elapsed: runtimeSource.includes("pub fn dictation_elapsed()"),
      currentPhase: runtimeSource.includes("pub fn current_dictation_phase()"),
      currentTarget: runtimeSource.includes("pub fn get_dictation_target()"),
      lastDelivery: runtimeSource.includes("pub fn last_delivery_receipt()"),
      redactedFingerprint: runtimeSource.includes("pub fn redacted_transcript_fingerprint("),
      automationState: runtimeSource.includes("pub fn automation_state()"),
    },
    stdin: stdinEvidence(stdinSource),
  };
  const missing = [
    ...new Set([
      ...(Array.isArray(media.missingRuntimePrimitives) ? media.missingRuntimePrimitives.map(String) : []),
      ...(Array.isArray(dictationSurface.missingRuntimePrimitives) ? dictationSurface.missingRuntimePrimitives.map(String) : []),
      runtimeState?.dictation ? "" : "passive current phase RPC",
      runtimeState?.dictation ? "" : "passive current target RPC",
      deliveryReceiptAvailable ? "" : "target delivery generation receipt",
      deliveryReceiptAvailable ? "" : "transcript fingerprint receipt",
    ].filter(Boolean)),
  ];

  emitValidatedReceipt("devtools.dictation.inspect", {
    schemaVersion: 2,
    tool: "script-kit-devtools.dictation",
    command: "dictation.inspect",
    classification: classify(media, sourceEvidence, provider, runtimeState),
    session: args.session,
    passiveSafety: {
      noMicrophoneCaptureRequired: true,
      noSystemSettingsRequired: true,
      noTccMutationRequired: true,
      noSyntheticTranscriptInjected: true,
      visualOverlayFixtureOpened: Boolean(overlayFixture),
    },
    overlayFixture,
    redaction: {
      transcriptContentReturned: false,
      deviceLabelsReturned: false,
      targetLabelsReturned: false,
      rawDeviceIdsReturned: false,
    },
    coverage: {
      id: dictationSurface.id ?? null,
      status: dictationSurface.status ?? null,
      features: dictationSurface.features ?? [],
      shortcuts: dictationSurface.shortcuts ?? [],
      supportedNow: dictationSurface.supportedNow ?? [],
      missingRuntimePrimitives: dictationSurface.missingRuntimePrimitives ?? [],
    },
    mediaReceipt: diagnostic(media),
    runtimeState: diagnostic(runtimeState?.dictation ?? null),
    deliveryReceiptAvailable,
    runtimeStateReceipt: diagnostic(stateEnvelope),
    providerResource: diagnostic(provider),
    sourceEvidence,
    deliveryTargets: sourceEvidence.targets,
    sessionPhases: sourceEvidence.phases,
    missingPrimitives: missing,
    recommendedNext: [
      "Keep expanding passive dictation runtime state from source-owned receipts before any live microphone proof.",
      deliveryReceiptAvailable
        ? "Use pushDictationResult in an explicit delivery test to compare lastDelivery.generation, target, destination, and transcriptFingerprint before and after the user path."
        : "Expose delivery receipts for pushDictationResult and real transcription that include target generation and redacted transcript fingerprint.",
      "Expose cursor insertion range and wrong-target refusal receipts without starting microphone capture.",
    ],
    warnings: [
      "Dictation inspection is intentionally passive; use pushDictationResult only in a separate explicit delivery test.",
      missing.length > 0 ? "Dictation remains fail-closed until passive runtime state and delivery receipts exist." : "",
    ].filter(Boolean),
    errors: diagnostic(
      [media].filter((receipt) => receipt.status === "error"),
    ),
  });
}

await main();
