#!/usr/bin/env bun

import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic, userContent } from "./lib/privacy.ts";

type JsonObject = Record<string, unknown>;

type ActionKind =
  | "set-input"
  | "select"
  | "key"
  | "open-actions"
  | "set-theme-control";

type Args = {
  actionKind: ActionKind;
  session: string;
  target?: JsonObject;
  text: string;
  value: string;
  semanticId: string;
  control: string;
  key: string;
  modifiers: string[];
  allowSubmit: boolean;
  submitIntent: string;
  allowSubmitReason: string;
  preflightOnly: boolean;
  strict: boolean;
  expectedSurfaceKind: string;
  timeoutMs: number;
  start: boolean;
  show: boolean;
  forwarded: string[];
};

type SubmitLifecycleState =
  | { state: "not-submit"; reason: string }
  | { state: "blocked-before-dispatch"; reason: string; gateName?: string; selectedSemanticId?: string | null; nextSafeCommand?: string | null; requiredFlags?: string[]; missingPrimitive?: string | null; requestedIntent?: string | null }
  | { state: "dispatched"; actionId?: string | null; allowedBy?: string | null; proofIntent?: string | null; parentSubjectId?: string | null; parentSubjectText?: string | null }
  | { state: "source-live"; actionId?: string | null; parentSubjectId?: string | null; parentSubjectText?: string | null }
  | { state: "source-closed-parent-live"; actionId?: string | null; parentSubjectId?: string | null; parentSubjectText?: string | null; parentTarget?: JsonObject | null }
  | { state: "failed"; reason: string; actionId?: string | null; sourceAfter?: JsonObject | null; parentAfter?: JsonObject | null };

type PostActionLifecycleState =
  | { state: "not-lifecycle-sensitive"; reason: string }
  | { state: "blocked-before-dispatch"; reason: string; selectedSemanticId?: string | null }
  | { state: "dispatched"; actionId?: string | null }
  | { state: "dismissed"; reason: "escape" | "cmd-k" | "cmd-w" }
  | { state: "source-live"; actionId?: string | null }
  | { state: "source-closed-parent-live"; actionId?: string | null; parentTarget?: JsonObject | null }
  | { state: "failed"; reason: string; actionId?: string | null; sourceAfter?: JsonObject | null; parentAfter?: JsonObject | null };

const blockedSubmitKeys = new Set(["enter", "return"]);
const allowedKeys = new Set([
  "arrowup",
  "arrowdown",
  "arrowleft",
  "arrowright",
  "up",
  "down",
  "left",
  "right",
  "tab",
  "escape",
  "esc",
  "enter",
  "return",
  "k",
  "p",
  "w",
  "backspace",
]);

function isPrintableTextKey(args: Args) {
  return args.actionKind === "key"
    && args.key.length === 1
    && args.modifiers.length === 0
    && !/[\r\n\t]/.test(args.key);
}

function isBackspaceTextKey(args: Args) {
  return args.actionKind === "key"
    && args.key.toLowerCase() === "backspace"
    && args.modifiers.length === 0;
}

function isTextEditingKey(args: Args) {
  return isPrintableTextKey(args) || isBackspaceTextKey(args);
}

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/act.ts set-input --text <value> [target args]",
    "  bun scripts/devtools/act.ts set-input --value <value> [target args]  # alias for --text",
    "  bun scripts/devtools/act.ts select --semantic-id <id> [--allow-submit] [target args]",
    "  bun scripts/devtools/act.ts key --key <name-or-character> [--modifiers cmd,shift] [--allow-submit] [target args]",
    "  bun scripts/devtools/act.ts key --key Enter --modifiers cmd --preflight-only [target args]",
    "  bun scripts/devtools/act.ts key --key Enter --modifiers cmd --allow-submit --submit-intent agent-chat-route --allow-submit-reason <why> [target args]",
    "  bun scripts/devtools/act.ts open-actions [target args]",
    "  bun scripts/devtools/act.ts set-theme-control --control <id> --value <value> --surface ThemeChooser [target args]",
    "",
    "Target args match scripts/devtools/targets.ts inspect, e.g. --session <name> --main --strict --surface ScriptList.",
  ].join("\n");
}

function parseArgs(argv: string[]): Args {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(usage());
    process.exit(0);
  }

  const command = argv[0] as ActionKind | undefined;
  if (
    !command
    || ![
      "set-input",
      "select",
      "key",
      "open-actions",
      "set-theme-control",
    ].includes(command)
  ) {
    console.error(usage());
    process.exit(2);
  }

  const args: Args = {
    actionKind: command,
    session: "default",
    text: "",
    value: "",
    semanticId: "",
    control: "",
    key: "",
    modifiers: [],
    allowSubmit: false,
    submitIntent: "",
    allowSubmitReason: "",
    preflightOnly: false,
    strict: false,
    expectedSurfaceKind: "",
    timeoutMs: 8000,
    start: false,
    show: false,
    forwarded: [],
  };

  for (let index = 1; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
      args.forwarded.push("--session", args.session);
    } else if (arg === "--target-id") {
      args.target = { type: "id", id: argv[++index] ?? "" };
      args.forwarded.push("--target-id", String(args.target.id ?? ""));
    } else if (arg === "--target-kind") {
      const kind = argv[++index] ?? "main";
      args.target = { type: "kind", kind };
      args.forwarded.push("--target-kind", kind);
    } else if (arg === "--target-index") {
      const value = Number(argv[++index] ?? 0);
      if (!args.target || args.target.type !== "kind") {
        throw new Error("--target-index requires --target-kind first");
      }
      args.target.index = value;
      args.forwarded.push("--target-index", String(value));
    } else if (arg === "--target-title") {
      args.target = { type: "titleContains", text: argv[++index] ?? "" };
      args.forwarded.push("--target-title", String(args.target.text ?? ""));
    } else if (arg === "--focused") {
      args.target = { type: "focused" };
      args.forwarded.push("--focused");
    } else if (arg === "--main") {
      args.target = { type: "main" };
      args.forwarded.push("--main");
    } else if (arg === "--surface") {
      args.expectedSurfaceKind = argv[++index] ?? "";
      args.forwarded.push("--surface", args.expectedSurfaceKind);
    } else if (arg === "--strict") {
      args.strict = true;
      args.forwarded.push("--strict");
    } else if (arg === "--text" || arg === "--value") {
      args.text = argv[++index] ?? "";
      args.value = args.text;
    } else if (arg === "--semantic-id") {
      args.semanticId = argv[++index] ?? "";
    } else if (arg === "--control") {
      args.control = argv[++index] ?? "";
    } else if (arg === "--key") {
      args.key = argv[++index] ?? "";
    } else if (arg === "--modifiers") {
      args.modifiers = (argv[++index] ?? "")
        .split(",")
        .map((modifier) => modifier.trim().toLowerCase())
        .filter(Boolean);
    } else if (arg === "--allow-submit") {
      args.allowSubmit = true;
    } else if (arg === "--submit-intent") {
      args.submitIntent = argv[++index] ?? "";
    } else if (arg === "--allow-submit-reason") {
      args.allowSubmitReason = argv[++index] ?? "";
    } else if (arg === "--preflight-only") {
      args.preflightOnly = true;
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
      args.forwarded.push("--timeout", String(args.timeoutMs));
    } else if (arg === "--start") {
      args.start = true;
      args.forwarded.push("--start");
    } else if (arg === "--show") {
      args.show = true;
      args.forwarded.push("--show");
    } else if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    }
  }

  return args;
}

async function run(command: string[], label: string): Promise<JsonObject> {
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  let parsed: JsonObject | null = null;
  try {
    parsed = JSON.parse(stdout);
  } catch {
    parsed = null;
  }
  if (exitCode !== 0) {
    return {
      status: "error",
      label,
      exitCode,
      stdout: stdout.trim(),
      stderr: stderr.trim(),
      parsedError: parsed,
      lifecycle: parsed?.lifecycle ?? null,
    };
  }
  if (parsed) {
    return parsed;
  }
  return { status: "error", label, exitCode, stdout: stdout.trim(), stderr: stderr.trim(), error: "invalid_json_output" };
}

function requestId(prefix: string) {
  return `devtools-act-${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function rpc(session: string, payload: JsonObject, expect: string, timeoutMs: number) {
  return run(["bash", "scripts/agentic/session.sh", "rpc", session, JSON.stringify(payload), "--expect", expect, "--timeout", String(timeoutMs)], String(payload.type ?? "rpc"));
}

function responseOf(envelope: JsonObject): JsonObject {
  return (envelope.response as JsonObject | undefined) ?? envelope;
}

function targetSelector(targetReceipt: JsonObject, fallback?: JsonObject) {
  return (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ?? fallback ?? { type: "focused" };
}

async function focusReceipt(args: Args, label: string) {
  return run(["bun", "scripts/devtools/focus.ts", "inspect", ...args.forwarded], label);
}

async function scrollReceipt(args: Args, label: string) {
  return run(["bun", "scripts/devtools/scroll.ts", "inspect", ...args.forwarded], label);
}

function postActionReceiptArgs(args: Args): Args {
  return args;
}

function safety(args: Args) {
  const warnings: string[] = [];
  const errors: string[] = [];
  const normalizedKey = args.key.toLowerCase();

  if (args.actionKind === "set-input" && args.text === undefined) {
    errors.push("set-input requires --text or --value");
  }
  if (args.actionKind === "select" && !args.semanticId) {
    errors.push("select requires --semantic-id");
  }
  if (args.actionKind === "set-theme-control" && !args.control) {
    errors.push("set-theme-control requires --control");
  }
  if (args.actionKind === "set-theme-control" && args.text.length === 0) {
    errors.push("set-theme-control requires --value");
  }
  if (args.actionKind === "key" && !args.key) {
    errors.push("key requires --key");
  }
  if (args.actionKind === "key" && !allowedKeys.has(normalizedKey) && !isPrintableTextKey(args)) {
    errors.push(`key '${args.key}' is not in the safe DevTools key allowlist`);
  }
  if (args.actionKind === "key" && blockedSubmitKeys.has(normalizedKey) && !args.allowSubmit && !args.preflightOnly) {
    errors.push("submit-like key requires --allow-submit");
  }
  if (args.actionKind === "key" && args.modifiers.includes("cmd") && blockedSubmitKeys.has(normalizedKey) && !args.allowSubmit && !args.preflightOnly) {
    errors.push("cmd+enter requires --allow-submit");
  }
  if (args.actionKind === "select" && args.allowSubmit) {
    warnings.push("selection will submit because --allow-submit was passed");
  }
  if (args.actionKind === "key" && (normalizedKey === "escape" || normalizedKey === "esc")) {
    warnings.push("escape can close or dismiss UI; receipt will classify by post-action target state");
  }

  return {
    channel: args.actionKind === "key" && !isTextEditingKey(args) ? "simulateKey" : "batch",
    destructive: args.allowSubmit,
    submitAllowed: args.allowSubmit,
    submitIntent: args.submitIntent || null,
    allowSubmitReason: args.allowSubmitReason || null,
    nativeEscalation: false,
    errors,
    warnings,
    submitAttempted: false,
    submitPreflightSelectedSemanticId: null as string | null,
  };
}

function actionPayload(args: Args, selector: JsonObject) {
  if (args.actionKind === "set-input") {
    return {
      type: "batch",
      requestId: requestId("set-input"),
      target: selector,
      commands: [{ type: "setInput", text: args.text }],
      options: { stopOnError: true, rollbackOnError: false, timeout: args.timeoutMs },
      trace: "on",
    };
  }
  if (args.actionKind === "select") {
    return {
      type: "batch",
      requestId: requestId("select"),
      target: selector,
      commands: [{ type: "selectBySemanticId", semanticId: args.semanticId, submit: args.allowSubmit }],
      options: { stopOnError: true, rollbackOnError: false, timeout: args.timeoutMs },
      trace: "on",
    };
  }
  if (args.actionKind === "open-actions") {
    return {
      type: "batch",
      requestId: requestId("open-actions"),
      target: selector,
      commands: [{ type: "openActions" }],
      options: { stopOnError: true, rollbackOnError: false, timeout: args.timeoutMs },
      trace: "on",
    };
  }
  if (args.actionKind === "set-theme-control") {
    return {
      type: "batch",
      requestId: requestId("set-theme-control"),
      target: selector,
      commands: [{ type: "setThemeControl", control: args.control, value: args.value }],
      options: { stopOnError: true, rollbackOnError: false, timeout: args.timeoutMs },
      trace: "on",
    };
  }
  return {
    type: "simulateKey",
    requestId: requestId("key"),
    target: selector,
    key: args.key,
    modifiers: args.modifiers,
  };
}

function textEditingKeyPayload(args: Args, selector: JsonObject, before: JsonObject) {
  const currentInput =
    typeof (before.keyboardOwner as JsonObject | undefined)?.inputValue === "string"
      ? String((before.keyboardOwner as JsonObject).inputValue)
      : "";
  const nextInput = isBackspaceTextKey(args)
    ? Array.from(currentInput).slice(0, -1).join("")
    : `${currentInput}${args.key}`;
  return {
    type: "batch",
    requestId: requestId("text-editing-key"),
    target: selector,
    commands: [{ type: "setInput", text: nextInput }],
    options: { stopOnError: true, rollbackOnError: false, timeout: args.timeoutMs },
    trace: "on",
  };
}

function expectedResponse(args: Args) {
  return args.actionKind === "key" && !isTextEditingKey(args) ? "externalCommandResult" : "batchResult";
}

function expectedResponseForTarget(args: Args, _targetReceipt: JsonObject) {
  return expectedResponse(args);
}

function requiresPostIntentTargetProof(args: Args) {
  return args.submitIntent === "agent-chat-route"
    || args.submitIntent === "profile-search-select";
}

function expectedPostTargetForIntent(args: Args): JsonObject | null {
  if (args.submitIntent === "agent-chat-route") {
    return {
      intent: "agent-chat-route",
      targetArgs: ["--main", "--strict", "--surface", "AgentChat"],
      expectedSurfaceKind: "AgentChat",
      expectedAutomationId: "main",
    };
  }
  if (args.submitIntent === "profile-search-select") {
    return {
      intent: "profile-search-select",
      targetArgs: ["--main", "--strict", "--surface", "ScriptList"],
      expectedSurfaceKind: "ScriptList",
      expectedAutomationId: "main",
    };
  }
  return null;
}

async function waitForPostIntentTarget(args: Args): Promise<JsonObject | null> {
  const expected = expectedPostTargetForIntent(args);
  if (!expected) return null;

  const startedAt = Date.now();
  const attempts: JsonObject[] = [];
  const deadline = startedAt + args.timeoutMs;
  const targetArgs = Array.isArray(expected.targetArgs)
    ? expected.targetArgs.map(String)
    : [];

  while (Date.now() < deadline) {
    const receipt = await run([
      "bun",
      "scripts/devtools/targets.ts",
      "inspect",
      "--session",
      args.session,
      "--timeout",
      String(args.timeoutMs),
      ...targetArgs,
    ], "targets.inspect.post-intent");
    attempts.push({
      elapsedMs: Date.now() - startedAt,
      classification: receipt.classification ?? null,
      requestedTarget: receipt.requestedTarget ?? null,
      resolvedTarget: receipt.resolvedTarget ?? null,
    });
    if (
      receipt.classification === "ok"
      && (receipt.resolvedTarget as JsonObject | undefined)?.strictTargetMatch === true
    ) {
      return { status: "ok", classification: "ok", expected, receipt, attempts };
    }
    await Bun.sleep(50);
  }

  return {
    status: "error",
    classification: "blocked-by-target-ambiguity",
    reason: "post-intent target did not resolve",
    expected,
    attempts,
  };
}

function visibleResult(before: JsonObject, after: JsonObject, beforeScroll: JsonObject, afterScroll: JsonObject) {
  const beforeSubmitDiagnostics = before.submitDiagnostics ?? null;
  const afterSubmitDiagnostics = after.submitDiagnostics ?? null;
  return {
    focusChanged: before.focusedSemanticId !== after.focusedSemanticId,
    selectionChanged: before.selectedSemanticId !== after.selectedSemanticId,
    inputChanged: (before.keyboardOwner as JsonObject | undefined)?.inputValue !== (after.keyboardOwner as JsonObject | undefined)?.inputValue,
    windowVisibleBefore: before.windowVisible ?? null,
    windowVisibleAfter: after.windowVisible ?? null,
    scrollChanged: JSON.stringify(beforeScroll.scroll ?? null) !== JSON.stringify(afterScroll.scroll ?? null),
    submitDiagnosticsChanged: JSON.stringify(beforeSubmitDiagnostics) !== JSON.stringify(afterSubmitDiagnostics),
  };
}

function actionFailed(actionReceipt: JsonObject) {
  if (actionReceipt.status === "error") return true;
  if (actionReceipt.success === false) return true;
  if (actionReceipt.parseOutcome === "timeout") return true;
  if (actionReceipt.parseOutcome === "parseError") return true;
  if (Array.isArray(actionReceipt.results)) {
    return actionReceipt.results.some((result) => {
      return typeof result === "object" && result !== null && (result as JsonObject).success === false;
    });
  }
  return false;
}

const lifecycleCodes = new Set([
  "session_dead",
  "forwarder_dead",
  "no_session",
  "app_process_dead_before_send",
  "app_process_dead_before_rpc",
  "forwarder_dead_before_send",
  "forwarder_dead_before_rpc",
]);

const nonDestructiveLauncherSubmitIds = new Set([
  "open-file-search",
  "search-files",
]);

const nonDestructiveActionsDialogSubmitPairs = [
  { parentText: "Launchpad", actionId: "copy_deeplink" },
  { parentText: "Emoji Picker", actionId: "copy_deeplink" },
  { parentText: "Design Gallery", actionId: "copy_deeplink" },
  { parentText: "Open Notes", actionId: "copy_deeplink" },
];

function receiptErrorCode(receipt: JsonObject): string {
  const direct = (receipt.error as JsonObject | undefined)?.code;
  if (typeof direct === "string") return direct;
  const parsed = receipt.parsedError as JsonObject | undefined | null;
  const parsedCode = (parsed?.error as JsonObject | undefined)?.code;
  return typeof parsedCode === "string" ? parsedCode : "";
}

function isLifecycleClassification(receipt: JsonObject) {
  return receipt.classification === "blocked-by-session-lifecycle" || lifecycleCodes.has(receiptErrorCode(receipt));
}

function isSubmitLike(args: Args) {
  const normalizedKey = args.key.toLowerCase();
  return (args.allowSubmit || args.preflightOnly) && (
    (args.actionKind === "key" && blockedSubmitKeys.has(normalizedKey))
    || (args.actionKind === "select")
  );
}

function isCmdEnter(args: Args) {
  const normalizedKey = args.key.toLowerCase();
  return args.actionKind === "key"
    && (normalizedKey === "enter" || normalizedKey === "return")
    && args.modifiers.includes("cmd");
}

function isPlainEnter(args: Args) {
  const normalizedKey = args.key.toLowerCase();
  return args.actionKind === "key"
    && (normalizedKey === "enter" || normalizedKey === "return")
    && args.modifiers.length === 0;
}

function submitGateDetails(args: Args, targetReceipt: JsonObject, before: JsonObject) {
  const target = targetInfo(targetReceipt);
  return {
    gateName: "submit.preflight",
    actionKind: args.actionKind,
    key: args.key || null,
    modifiers: args.modifiers,
    allowSubmit: args.allowSubmit,
    allowSubmitReason: args.allowSubmitReason || null,
    submitIntent: args.submitIntent || null,
    selectedSemanticId: typeof before.selectedSemanticId === "string" ? before.selectedSemanticId : null,
    selectedActionId: selectedActionIdFromSemanticId(before.selectedSemanticId),
    target: {
      automationId: target?.automationId ?? null,
      targetKind: target?.targetKind ?? null,
      surfaceKind: target?.surfaceKind ?? null,
      appViewVariant: target?.appViewVariant ?? null,
      nativeFooterSurface: target?.nativeFooterSurface ?? null,
    },
  };
}

function isScopedAgentChatRoute(args: Args, targetReceipt: JsonObject) {
  return isCmdEnter(args)
    && args.allowSubmit
    && args.submitIntent === "agent-chat-route"
    && (isScriptListTargetReceipt(targetReceipt) || isPromptEntityTargetReceipt(targetReceipt));
}

function isDismissLike(args: Args) {
  const normalizedKey = args.key.toLowerCase();
  if (args.actionKind !== "key") return false;
  if (normalizedKey === "escape" || normalizedKey === "esc") return true;
  if (normalizedKey === "k" && args.modifiers.includes("cmd")) return true;
  if (normalizedKey === "w" && args.modifiers.includes("cmd")) return true;
  return false;
}

function selectedActionIdFromSemanticId(value: unknown) {
  if (typeof value !== "string") return null;
  const match = value.match(/^choice:\d+:(.+)$/);
  return match?.[1] ?? null;
}

function isActionsDialogTargetReceipt(receipt: JsonObject) {
  const resolved = receipt.resolvedTarget as JsonObject | undefined;
  return resolved?.targetKind === "ActionsDialog"
    || resolved?.surfaceKind === "ActionsDialog"
    || resolved?.appViewVariant === "ActionsDialog";
}

function isScriptListTargetReceipt(receipt: JsonObject) {
  const resolved = receipt.resolvedTarget as JsonObject | undefined;
  return resolved?.targetKind === "Main"
    || resolved?.surfaceKind === "ScriptList"
    || resolved?.appViewVariant === "ScriptList";
}

function isProfileSearchTargetReceipt(receipt: JsonObject) {
  const resolved = targetInfo(receipt);
  return resolved?.automationId === "main"
    && resolved?.targetKind === "Main"
    && (
      resolved?.surfaceKind === "ProfileSearch"
      || resolved?.appViewVariant === "ProfileSearchView"
      || resolved?.semanticSurface === "profileSearch"
    );
}

function isPromptEntityTargetReceipt(receipt: JsonObject) {
  const resolved = receipt.resolvedTarget as JsonObject | undefined;
  return resolved?.surfaceKind === "PromptEntity"
    || resolved?.appViewVariant === "MiniPrompt"
    || resolved?.appViewVariant === "ArgPrompt";
}

function isPromptPopupTargetReceipt(receipt: JsonObject) {
  const resolved = targetInfo(receipt);
  return resolved?.targetKind === "PromptPopup";
}

function isNonDestructiveLauncherSubmit(actionId: string | null) {
  return actionId !== null && nonDestructiveLauncherSubmitIds.has(actionId);
}

function isNonDestructiveProfileSwitchSubmit(args: Args, before: JsonObject) {
  if (args.submitIntent !== "profile-switch") return false;
  const selected = before.selectedNode as JsonObject | undefined;
  return selected?.kind === "profile"
    && selected?.sourceName === "Spine"
    && typeof selected?.semanticId === "string"
    && /^choice:\d+:[a-z0-9-]+$/.test(selected.semanticId);
}

function isScopedProfileSearchSelect(
  args: Args,
  targetReceipt: JsonObject,
  selectedSemanticId: string | null,
) {
  return isPlainEnter(args)
    && args.allowSubmit
    && args.submitIntent === "profile-search-select"
    && args.allowSubmitReason.trim().length > 0
    && isProfileSearchTargetReceipt(targetReceipt)
    && typeof selectedSemanticId === "string"
    && selectedSemanticId.startsWith("profile-search-row:");
}

function isMenuSyntaxTriggerPickerSelected(before: JsonObject) {
  const selected = before.selectedNode as JsonObject | undefined;
  return selected?.kind === "menuSyntaxTriggerPicker"
    || selected?.source === "menuSyntaxTriggerPicker"
    || selected?.role === "menu-syntax-trigger-row";
}

function isSelectSemanticActivation(args: Args) {
  return args.actionKind === "select"
    && args.allowSubmit
    && args.semanticId.length > 0;
}

function isScopedMenuSyntaxTriggerAccept(
  args: Args,
  targetReceipt: JsonObject,
  before: JsonObject,
  selectedSemanticId: string | null,
) {
  return (isPlainEnter(args) || isSelectSemanticActivation(args))
    && args.allowSubmit
    && args.submitIntent === "menu-syntax-trigger-accept"
    && args.allowSubmitReason.trim().length > 0
    && isScriptListTargetReceipt(targetReceipt)
    && typeof selectedSemanticId === "string"
    && selectedSemanticId.startsWith("choice:")
    && isMenuSyntaxTriggerPickerSelected(before);
}

function isScopedMenuSyntaxObjectSelectorAccept(
  args: Args,
  targetReceipt: JsonObject,
  before: JsonObject,
  selectedSemanticId: string | null,
) {
  const selected = before.selectedNode as JsonObject | undefined;
  return isPlainEnter(args)
    && args.allowSubmit
    && args.submitIntent === "menu-syntax-object-selector-accept"
    && args.allowSubmitReason.trim().length > 0
    && isScriptListTargetReceipt(targetReceipt)
    && typeof selectedSemanticId === "string"
    && selectedSemanticId.startsWith("choice:")
    && (
      selected?.kind === "menuSyntaxObjectSelector"
      || selected?.source === "menuSyntaxObjectSelector"
      || selected?.role === "menu-syntax-object-selector-row"
    );
}

function targetInfo(receipt: JsonObject) {
  return (receipt.resolvedTarget as JsonObject | undefined) ?? (receipt.target as JsonObject | undefined) ?? null;
}

async function parentFocusForTarget(args: Args, targetReceipt: JsonObject) {
  const resolved = targetInfo(targetReceipt);
  const parentAutomationId = resolved?.parentAutomationId;
  if (typeof parentAutomationId !== "string" || parentAutomationId.length === 0) {
    return null;
  }
  return run([
    "bun",
    "scripts/devtools/focus.ts",
    "inspect",
    "--session",
    args.session,
    "--target-id",
    parentAutomationId,
    "--strict",
    "--timeout",
    String(args.timeoutMs),
  ], "focus.parent-submit-preflight");
}

function selectedSubjectFromFocus(parentFocus: JsonObject | null) {
  const selectedNode = parentFocus?.selectedNode as JsonObject | undefined;
  const id = selectedNode?.semanticId;
  const text = selectedNode?.text;
  return {
    id: typeof id === "string" ? id : null,
    text: typeof text === "string" ? text : null,
  };
}

function isNonDestructiveActionsDialogSubmit(actionId: string | null, parentSubjectText: string | null) {
  if (!actionId || !parentSubjectText) return false;
  return nonDestructiveActionsDialogSubmitPairs.some((pair) => {
    return pair.actionId === actionId && pair.parentText === parentSubjectText;
  });
}

function isNotesSendToAgentChatRoute(args: Args, actionId: string | null, parentFocus: JsonObject | null) {
  if (args.submitIntent !== "notes-send-to-ai" || actionId !== "send_to_ai") return false;
  const parentTarget = targetInfo(parentFocus ?? {});
  return parentTarget?.automationId === "notes" || parentTarget?.targetKind === "Notes";
}

function isScopedAgentHandoffDryRun(
  args: Args,
  actionId: string | null,
  parentFocus: JsonObject | null,
) {
  if (args.submitIntent !== "agent-handoff-dry-run") return false;
  if (!args.allowSubmit || !args.allowSubmitReason.trim()) return false;
  const allowedPromptActions = new Set([
    "prompt-target/cmux-codex",
    "agent_chat:handoff:cmux_codex",
    "prompt-action/export-file",
    "prompt-action/export-gist",
    "prompt-action/copy-prompt",
  ]);
  if (!actionId || !allowedPromptActions.has(actionId)) return false;
  if (!truthyEnv("SCRIPT_KIT_AGENT_HANDOFF_DRY_RUN")) return false;
  if (!process.env.SCRIPT_KIT_AGENT_HANDOFF_RECEIPT_PATH?.trim()) return false;
  const parentTarget = targetInfo(parentFocus ?? {});
  return parentTarget?.targetKind === "AgentChat"
    || parentTarget?.surfaceKind === "AgentChat"
    || parentTarget?.appViewVariant === "AgentChatView"
    || parentTarget?.targetKind === "ScriptList"
    || parentTarget?.surfaceKind === "ScriptList"
    || parentTarget?.appViewVariant === "ScriptList"
    || parentTarget?.targetKind === "ActionsDialog"
    || parentTarget?.semanticSurface === "actionsDialog";
}

function truthyEnv(name: string) {
  const value = process.env[name];
  if (!value) return false;
  return ["1", "true", "yes", "on"].includes(value.trim().toLowerCase());
}

function requestedActivationSemanticId(args: Args, before: JsonObject) {
  if (args.actionKind === "select" && args.semanticId) return args.semanticId;
  return typeof before.selectedSemanticId === "string" ? before.selectedSemanticId : null;
}

function isNonDestructivePromptPopupProfileActivation(semanticId: string | null) {
  if (!semanticId) return false;
  return /^choice:\d+:(agent-chat-profile:)?(general|text|script-kit|agent_chat)$/.test(semanticId);
}

function isNonDestructiveSubmit(preflight: SubmitLifecycleState) {
  return preflight.state === "dispatched"
    && (isNonDestructiveLauncherSubmit(preflight.actionId ?? null)
      || isNonDestructiveActionsDialogSubmit(preflight.actionId ?? null, preflight.parentSubjectText ?? null)
      || isNonDestructiveProfileSearchSubmit(preflight));
}

function isNonDestructiveProfileSearchSubmit(preflight: SubmitLifecycleState) {
  return preflight.state === "dispatched"
    && preflight.allowedBy === "submitIntent:profile-search-select";
}

async function submitPreflight(args: Args, targetReceipt: JsonObject, before: JsonObject): Promise<SubmitLifecycleState> {
  if (!isSubmitLike(args)) {
    return { state: "not-submit", reason: "action is not submit-like" };
  }
  const selectedSemanticId = requestedActivationSemanticId(args, before);
  const actionId = selectedActionIdFromSemanticId(selectedSemanticId);
  if (args.submitIntent === "profile-picker-route") {
    return {
      state: "blocked-before-dispatch",
      reason: "native footer activation proof requires a nativeFooterActivationReceipt primitive",
      gateName: "native-footer.activation.missing",
      selectedSemanticId: selectedSemanticId as string | null,
      missingPrimitive: "nativeFooterActivationReceipt",
      requestedIntent: "profile-picker-route",
      nextSafeCommand: "bun scripts/devtools/keyboard.ts inspect --target-id <id> --strict",
    };
  }
  if (isScopedAgentChatRoute(args, targetReceipt)) {
    return {
      state: "dispatched",
      actionId: "cmd-enter-agent-chat-route",
      allowedBy: "submitIntent:agent-chat-route",
      proofIntent: args.submitIntent,
    };
  }
  if (args.allowSubmit && !args.submitIntent) {
    return {
      state: "blocked-before-dispatch",
      reason: "submit-like proof requires --submit-intent so --allow-submit is scoped",
      gateName: "submit.intent.required",
      selectedSemanticId: selectedSemanticId as string | null,
      requiredFlags: ["--allow-submit", "--submit-intent <intent>", "--allow-submit-reason <why>"],
      nextSafeCommand: "rerun with --preflight-only first, then --allow-submit --submit-intent agent-chat-route --allow-submit-reason <why>",
    };
  }
  if (args.submitIntent === "profile-search-select") {
    if (!args.allowSubmitReason.trim()) {
      return {
        state: "blocked-before-dispatch",
        reason: "profile-search-select requires --allow-submit-reason",
        gateName: "submit.reason.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--allow-submit",
          "--submit-intent profile-search-select",
          "--allow-submit-reason <why>",
        ],
        nextSafeCommand: "rerun with --preflight-only first, then --allow-submit --submit-intent profile-search-select --allow-submit-reason <why>",
      };
    }
    if (!isScopedProfileSearchSelect(args, targetReceipt, selectedSemanticId)) {
      return {
        state: "blocked-before-dispatch",
        reason: "profile-search-select requires plain Enter on main ProfileSearch with a selected profile row",
        gateName: "profile-search-select.target.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--main",
          "--strict",
          "--surface ProfileSearch",
          "--allow-submit",
          "--submit-intent profile-search-select",
          "--allow-submit-reason <why>",
        ],
      };
    }
    return {
      state: "dispatched",
      actionId: selectedSemanticId,
      allowedBy: "submitIntent:profile-search-select",
      proofIntent: args.submitIntent,
    };
  }
  if (args.submitIntent === "menu-syntax-trigger-accept") {
    if (!args.allowSubmitReason.trim()) {
      return {
        state: "blocked-before-dispatch",
        reason: "menu-syntax-trigger-accept requires --allow-submit-reason",
        gateName: "submit.reason.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--allow-submit",
          "--submit-intent menu-syntax-trigger-accept",
          "--allow-submit-reason <why>",
        ],
      };
    }
    if (!isScopedMenuSyntaxTriggerAccept(args, targetReceipt, before, selectedSemanticId)) {
      return {
        state: "blocked-before-dispatch",
        reason: "menu-syntax-trigger-accept requires plain Enter or selectBySemanticId on main ScriptList with a selected menuSyntaxTriggerPicker row",
        gateName: "menu-syntax-trigger-accept.target.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--main",
          "--strict",
          "--surface ScriptList",
          "--allow-submit",
          "--submit-intent menu-syntax-trigger-accept",
          "--allow-submit-reason <why>",
        ],
      };
    }
    return {
      state: "dispatched",
      actionId: selectedSemanticId,
      allowedBy: "submitIntent:menu-syntax-trigger-accept",
      proofIntent: args.submitIntent,
    };
  }
  if (args.submitIntent === "menu-syntax-object-selector-accept") {
    if (!args.allowSubmitReason.trim()) {
      return {
        state: "blocked-before-dispatch",
        reason: "menu-syntax-object-selector-accept requires --allow-submit-reason",
        gateName: "submit.reason.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--allow-submit",
          "--submit-intent menu-syntax-object-selector-accept",
          "--allow-submit-reason <why>",
        ],
      };
    }
    if (!isScopedMenuSyntaxObjectSelectorAccept(args, targetReceipt, before, selectedSemanticId)) {
      return {
        state: "blocked-before-dispatch",
        reason: "menu-syntax-object-selector-accept requires plain Enter on main ScriptList with a selected menuSyntaxObjectSelector row",
        gateName: "menu-syntax-object-selector-accept.target.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--main",
          "--strict",
          "--surface ScriptList",
          "--allow-submit",
          "--submit-intent menu-syntax-object-selector-accept",
          "--allow-submit-reason <why>",
        ],
      };
    }
    return {
      state: "dispatched",
      actionId: selectedSemanticId,
      allowedBy: "submitIntent:menu-syntax-object-selector-accept",
      proofIntent: args.submitIntent,
    };
  }
  if (!isActionsDialogTargetReceipt(targetReceipt)) {
    if (
      isPromptPopupTargetReceipt(targetReceipt)
      && isNonDestructivePromptPopupProfileActivation(selectedSemanticId)
    ) {
      return { state: "dispatched", actionId: selectedSemanticId };
    }
    if (isPromptEntityTargetReceipt(targetReceipt)) {
      return {
        state: "dispatched",
        actionId: typeof selectedSemanticId === "string" ? selectedSemanticId : actionId,
      };
    }
    if (isScriptListTargetReceipt(targetReceipt) && isNonDestructiveLauncherSubmit(actionId)) {
      return { state: "dispatched", actionId };
    }
    if (isScriptListTargetReceipt(targetReceipt) && isNonDestructiveProfileSwitchSubmit(args, before)) {
      return {
        state: "dispatched",
        actionId: typeof selectedSemanticId === "string" ? selectedSemanticId : actionId,
        allowedBy: "submitIntent:profile-switch",
        proofIntent: args.submitIntent,
      };
    }
    return { state: "blocked-before-dispatch", reason: "submit requires ActionsDialog target or non-destructive launcher allowlist", selectedSemanticId: selectedSemanticId as string | null };
  }
  if (!actionId) {
    return { state: "blocked-before-dispatch", reason: "submit requires selected ActionsDialog choice:* row", selectedSemanticId: selectedSemanticId as string | null };
  }
  if (args.submitIntent === "receipt-history-route") {
    if (!args.allowSubmitReason.trim()) {
      return {
        state: "blocked-before-dispatch",
        reason: "receipt-history-route requires --allow-submit-reason",
        gateName: "submit.reason.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--allow-submit",
          "--submit-intent receipt-history-route",
          "--allow-submit-reason <why>",
        ],
      };
    }
    if (actionId !== "agent_chat_show_receipt_history") {
      return {
        state: "blocked-before-dispatch",
        reason: "receipt-history-route requires the agent_chat_show_receipt_history ActionsDialog row",
        gateName: "receipt-history-route.action.required",
        selectedSemanticId: selectedSemanticId as string | null,
      };
    }
    return {
      state: "dispatched",
      actionId,
      allowedBy: "submitIntent:receipt-history-route",
      proofIntent: args.submitIntent,
    };
  }
  if (args.submitIntent === "receipt-history-copy") {
    if (!args.allowSubmitReason.trim()) {
      return {
        state: "blocked-before-dispatch",
        reason: "receipt-history-copy requires --allow-submit-reason",
        gateName: "submit.reason.required",
        selectedSemanticId: selectedSemanticId as string | null,
        requiredFlags: [
          "--allow-submit",
          "--submit-intent receipt-history-copy",
          "--allow-submit-reason <why>",
        ],
      };
    }
    if (!actionId.startsWith("agent_chat_receipt_history:copy:")) {
      return {
        state: "blocked-before-dispatch",
        reason: "receipt-history-copy requires an agent_chat_receipt_history:copy row",
        gateName: "receipt-history-copy.action.required",
        selectedSemanticId: selectedSemanticId as string | null,
      };
    }
    return {
      state: "dispatched",
      actionId,
      allowedBy: "submitIntent:receipt-history-copy",
      proofIntent: args.submitIntent,
    };
  }
  if (isScopedAgentHandoffDryRun(args, actionId, targetReceipt)) {
    return {
      state: "dispatched",
      actionId,
      allowedBy: "submitIntent:agent-handoff-dry-run",
      proofIntent: args.submitIntent,
    };
  }
  const parentFocus = await parentFocusForTarget(args, targetReceipt);
  const parentSubject = selectedSubjectFromFocus(parentFocus);
  if (isNotesSendToAgentChatRoute(args, actionId, parentFocus)) {
    return {
      state: "dispatched",
      actionId,
      allowedBy: "submitIntent:notes-send-to-ai",
      proofIntent: args.submitIntent,
      parentSubjectId: parentSubject.id,
      parentSubjectText: parentSubject.text,
    };
  }
  if (isScopedAgentHandoffDryRun(args, actionId, parentFocus)) {
    return {
      state: "dispatched",
      actionId,
      allowedBy: "submitIntent:agent-handoff-dry-run",
      proofIntent: args.submitIntent,
      parentSubjectId: parentSubject.id,
      parentSubjectText: parentSubject.text,
    };
  }
  if (!isNonDestructiveActionsDialogSubmit(actionId, parentSubject.text)) {
    return {
      state: "blocked-before-dispatch",
      reason: "submit requires named non-destructive ActionsDialog parent/action allowlist",
      selectedSemanticId: selectedSemanticId as string | null,
    };
  }
  return {
    state: "dispatched",
    actionId,
    parentSubjectId: parentSubject.id,
    parentSubjectText: parentSubject.text,
  };
}

async function inspectParentAfterSubmit(args: Args, targetReceipt: JsonObject) {
  return inspectParentAfterAction(args, targetReceipt);
}

async function inspectParentAfterAction(args: Args, targetReceipt: JsonObject) {
  const resolved = targetReceipt.resolvedTarget as JsonObject | undefined;
  const parentAutomationId = resolved?.parentAutomationId;
  const parentArgs = parentAutomationId
    ? ["--target-id", String(parentAutomationId)]
    : ["--main", "--surface", "ScriptList"];
  return run([
    "bun",
    "scripts/devtools/targets.ts",
    "inspect",
    "--session",
    args.session,
    "--strict",
    "--timeout",
    String(args.timeoutMs),
    ...parentArgs,
  ], "targets.inspect.parent-after-submit");
}

function resolveSubmitLifecycleAfterAction(
  preflight: SubmitLifecycleState,
  sourceAfter: JsonObject,
  parentAfter: JsonObject | null,
  postIntentTargetProof: JsonObject | null,
): SubmitLifecycleState {
  if (preflight.state !== "dispatched") return preflight;
  if (postIntentTargetProof) {
    if (postIntentTargetProof.classification === "ok") {
      const receipt = postIntentTargetProof.receipt as JsonObject | undefined;
      return {
        state: "source-closed-parent-live",
        actionId: preflight.actionId,
        parentSubjectId: preflight.parentSubjectId,
        parentSubjectText: preflight.parentSubjectText,
        parentTarget: (receipt?.resolvedTarget as JsonObject | undefined) ?? null,
      };
    }
    return {
      state: "failed",
      reason: String(postIntentTargetProof.reason ?? "post-intent target did not resolve"),
      actionId: preflight.actionId,
      sourceAfter,
      parentAfter: postIntentTargetProof,
    };
  }
  if (sourceAfter.submitDiagnostics && typeof sourceAfter.submitDiagnostics === "object") {
    return {
      state: "source-live",
      actionId: preflight.actionId,
      parentSubjectId: preflight.parentSubjectId,
      parentSubjectText: preflight.parentSubjectText,
    };
  }
  if (sourceAfter.classification === "ok") {
    return {
      state: "source-live",
      actionId: preflight.actionId,
      parentSubjectId: preflight.parentSubjectId,
      parentSubjectText: preflight.parentSubjectText,
    };
  }
  if (parentAfter?.classification === "ok") {
    return {
      state: "source-closed-parent-live",
      actionId: preflight.actionId,
      parentSubjectId: preflight.parentSubjectId,
      parentSubjectText: preflight.parentSubjectText,
      parentTarget: parentAfter.resolvedTarget as JsonObject | undefined ?? null,
    };
  }
  const sourceLifecycle = isLifecycleClassification(sourceAfter);
  const parentLifecycle = parentAfter ? isLifecycleClassification(parentAfter) : false;
  return {
    state: "failed",
    reason: sourceLifecycle || parentLifecycle ? "blocked-by-session-lifecycle" : "post-submit target not inspectable",
    actionId: preflight.actionId,
    sourceAfter,
    parentAfter,
  };
}

function dismissReason(args: Args): "escape" | "cmd-k" | "cmd-w" {
  const normalizedKey = args.key.toLowerCase();
  if (normalizedKey === "k" && args.modifiers.includes("cmd")) return "cmd-k";
  if (normalizedKey === "w" && args.modifiers.includes("cmd")) return "cmd-w";
  return "escape";
}

function resolvePostActionLifecycle(
  args: Args,
  submitLifecycle: SubmitLifecycleState,
  sourceAfter: JsonObject,
  parentAfter: JsonObject | null,
  lifecycleSensitiveAction = false,
): PostActionLifecycleState {
  if (isSubmitLike(args)) {
    return submitLifecycle;
  }
  if (!isDismissLike(args) && !lifecycleSensitiveAction) {
    return { state: "not-lifecycle-sensitive", reason: "action keeps source target inspectable" };
  }
  if (sourceAfter.classification === "ok") {
    return { state: "source-live", actionId: null };
  }
  if (parentAfter?.classification === "ok") {
    return {
      state: "source-closed-parent-live",
      actionId: null,
      parentTarget: parentAfter.resolvedTarget as JsonObject | undefined ?? null,
    };
  }
  const sourceLifecycle = isLifecycleClassification(sourceAfter);
  const parentLifecycle = parentAfter ? isLifecycleClassification(parentAfter) : false;
  if (sourceLifecycle || parentLifecycle) {
    return { state: "failed", reason: "blocked-by-session-lifecycle", actionId: null, sourceAfter, parentAfter };
  }
  return {
    state: "failed",
    reason: `dismissed source target but parent was not inspectable after ${dismissReason(args)}`,
    actionId: null,
    sourceAfter,
    parentAfter,
  };
}

function isSuccessfulPromptPopupSelect(args: Args, targetReceipt: JsonObject, actionReceipt: JsonObject) {
  return args.actionKind === "select"
    && isPromptPopupTargetReceipt(targetReceipt)
    && actionReceipt.type === "batchResult"
    && actionReceipt.success === true;
}

function classify(
  args: Args,
  targetReceipt: JsonObject,
  guard: ReturnType<typeof safety>,
  actionReceipt: JsonObject,
  after: JsonObject,
  submitLifecycle: SubmitLifecycleState,
  postActionLifecycle: PostActionLifecycleState,
  postIntentTargetProof: JsonObject | null,
) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  if (args.preflightOnly) {
    return "ok";
  }
  if (submitLifecycle.state === "blocked-before-dispatch") {
    if (submitLifecycle.gateName === "native-footer.activation.missing") {
      return "blocked-by-native-escalation-required";
    }
    return "blocked-by-unsafe-operation";
  }
  if (guard.errors.length > 0) {
    return "blocked-by-unsafe-operation";
  }
  if (requiresPostIntentTargetProof(args) && postIntentTargetProof?.classification !== "ok") {
    return postIntentTargetProof?.classification ?? "blocked-by-target-ambiguity";
  }
  if (submitLifecycle.state === "source-live" || submitLifecycle.state === "source-closed-parent-live") {
    return "ok";
  }
  if (postActionLifecycle.state === "source-live" || postActionLifecycle.state === "source-closed-parent-live") {
    return "ok";
  }
  if (postActionLifecycle.state === "failed" && postActionLifecycle.reason === "blocked-by-session-lifecycle") {
    return "blocked-by-session-lifecycle";
  }
  if (submitLifecycle.state === "failed" && submitLifecycle.reason === "blocked-by-session-lifecycle") {
    return "blocked-by-session-lifecycle";
  }
  if (isLifecycleClassification(actionReceipt) || isLifecycleClassification(after)) {
    return "blocked-by-session-lifecycle";
  }
  if (actionFailed(actionReceipt)) {
    return "blocked-by-timeout";
  }
  if (isPrintableTextKey(args) && after.target && after.classification === "blocked-by-missing-primitive") {
    return "ok";
  }
  if (after.classification && after.classification !== "ok") {
    return after.classification;
  }
  return "ok";
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  const targetReceipt = await run(["bun", "scripts/devtools/targets.ts", "inspect", ...args.forwarded], "targets.inspect");
  const selector = targetSelector(targetReceipt, args.target);
  const guard = safety(args);

  const before = await focusReceipt(args, "focus.before");
  const beforeScroll = await scrollReceipt(args, "scroll.before");
  const startedAt = new Date().toISOString();
  const preflight = await submitPreflight(args, targetReceipt, before);
  const blockedAction = preflight.state === "blocked-before-dispatch"
    ? {
      gateName: preflight.gateName ?? "submit.preflight",
      reason: preflight.reason,
      requiredFlags: preflight.requiredFlags ?? [],
      nextSafeCommand: preflight.nextSafeCommand ?? null,
      missingPrimitive: preflight.missingPrimitive ?? null,
      requestedIntent: preflight.requestedIntent ?? args.submitIntent ?? null,
    }
    : null;
  const guardWithPreflight = {
    ...guard,
    destructive: guard.destructive && !isNonDestructiveSubmit(preflight),
    submitAttempted: !args.preflightOnly && isSubmitLike(args) && preflight.state !== "blocked-before-dispatch",
    submitPreflightSelectedSemanticId: (before.selectedSemanticId as string | undefined) ?? null,
    errors: preflight.state === "blocked-before-dispatch"
      ? [...guard.errors, preflight.reason]
      : guard.errors,
  };

  let actionEnvelope: JsonObject = { status: "blocked", reason: "blocked-by-unsafe-operation" };
  let dispatchElapsedMs: number | null = null;
  if (args.preflightOnly) {
    actionEnvelope = {
      status: "ok",
      label: "act.preflight-only",
      skippedDispatch: true,
      reason: "preflight-only requested",
    };
  } else if (guardWithPreflight.errors.length === 0 && targetReceipt.classification === "ok") {
    const dispatchStartedAt = Date.now();
    const payload = isTextEditingKey(args)
      ? textEditingKeyPayload(args, selector, before)
      : actionPayload(args, selector);
    actionEnvelope = await rpc(
      args.session,
      payload,
      expectedResponse(args),
      args.timeoutMs,
    );
    dispatchElapsedMs = Date.now() - dispatchStartedAt;
  }

  const afterArgs = postActionReceiptArgs(args);
  const after = await focusReceipt(afterArgs, "focus.after");
  const afterScroll = await scrollReceipt(afterArgs, "scroll.after");
  const endedAt = new Date().toISOString();
  const actionReceipt = responseOf(actionEnvelope);
  const postIntentTargetProof = !args.preflightOnly && preflight.state === "dispatched"
    ? await waitForPostIntentTarget(args)
    : null;
  const parentAfterSubmit = isSubmitLike(args) ? await inspectParentAfterSubmit(args, targetReceipt) : null;
  const parentAfterAction = isDismissLike(args) || isSuccessfulPromptPopupSelect(args, targetReceipt, actionReceipt)
    ? await inspectParentAfterAction(args, targetReceipt)
    : parentAfterSubmit;
  const submitLifecycle = resolveSubmitLifecycleAfterAction(
    preflight,
    after,
    parentAfterSubmit,
    postIntentTargetProof,
  );
  const promptPopupSelectClosedSource = isSuccessfulPromptPopupSelect(args, targetReceipt, actionReceipt);
  const postActionLifecycle = resolvePostActionLifecycle(
    args,
    submitLifecycle,
    after,
    parentAfterAction,
    promptPopupSelectClosedSource,
  );
  const classification = classify(
    args,
    targetReceipt,
    guardWithPreflight,
    actionReceipt,
    after,
    submitLifecycle,
    postActionLifecycle,
    postIntentTargetProof,
  );

  emitValidatedReceipt("devtools.act", {
    schemaVersion: 2,
    tool: "script-kit-devtools.act",
    command: `act.${args.actionKind}`,
    classification,
    session: args.session,
    startedAt,
    endedAt,
    actionId: requestId(args.actionKind),
    actionKind: args.actionKind,
    proofIntent: args.submitIntent || null,
    preflightOnly: args.preflightOnly,
    submitGate: submitGateDetails(args, targetReceipt, before),
    blockedAction,
    requestedTarget: targetReceipt.requestedTarget ?? { selector },
    targetBefore: before.target ?? targetReceipt.resolvedTarget ?? null,
    input: {
      text: userContent(args.actionKind === "set-input" ? args.text : null),
      semanticId: args.actionKind === "select" ? args.semanticId : null,
      control: args.actionKind === "set-theme-control" ? args.control : null,
      value: args.actionKind === "set-theme-control" ? args.value : null,
      key: args.actionKind === "key" ? args.key : null,
      modifiers: args.actionKind === "key" ? args.modifiers : [],
    },
    safety: guardWithPreflight,
    submitLifecycle: diagnostic(submitLifecycle),
    postActionLifecycle: diagnostic(postActionLifecycle),
    dismissLifecycle: diagnostic(isDismissLike(args) ? postActionLifecycle : null),
    expected: {
      protocolResponse: expectedResponseForTarget(args, targetReceipt),
      submitAllowed: args.allowSubmit,
      noNativeEscalation: !guardWithPreflight.nativeEscalation,
      prePostReceipts: ["focus.inspect", "focus.inspect.submitDiagnostics", "scroll.inspect"],
      postIntentTarget: expectedPostTargetForIntent(args),
    },
    actionReceipt: diagnostic(actionReceipt),
    result: diagnostic(actionReceipt),
    actionTiming: {
      dispatchElapsedMs,
      appReportedElapsedMs:
        typeof actionReceipt.totalElapsed === "number" ? actionReceipt.totalElapsed
        : typeof actionReceipt.elapsed === "number" ? actionReceipt.elapsed
        : null,
    },
    postIntentTargetProof,
    targetAfter: after.target ?? null,
    submitDiagnostics: diagnostic({
      before: before.submitDiagnostics ?? null,
      after: after.submitDiagnostics ?? null,
    }),
    visibleResult: visibleResult(before, after, beforeScroll, afterScroll),
    before: diagnostic({ focus: before, scroll: beforeScroll }),
    after: diagnostic({ focus: after, scroll: afterScroll }),
    warnings: [
      ...guardWithPreflight.warnings,
      ...(Array.isArray(before.warnings) ? before.warnings : []),
      ...(Array.isArray(after.warnings) ? after.warnings : []),
    ],
    errors: diagnostic([
      ...guardWithPreflight.errors.map((error) => ({ error })),
      targetReceipt.classification !== "ok" ? targetReceipt : null,
      actionFailed(actionReceipt) ? actionReceipt : null,
      postIntentTargetProof && postIntentTargetProof.classification !== "ok" ? postIntentTargetProof : null,
    ].filter(Boolean)),
  });
}

await main();
