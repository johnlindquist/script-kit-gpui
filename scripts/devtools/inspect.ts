#!/usr/bin/env bun

import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic, externalContent, productStatic, userContent } from "./lib/privacy.ts";
import { classifyTransportError } from "./lib/transport-errors.ts";
import {
  compareWindowLifetimeSnapshots,
  proofTransactionIdentity,
  strictTransactionMissingFields,
  targetIdentity,
} from "./lib/target-identity.ts";

type JsonObject = Record<string, unknown>;

type InspectArgs = {
  session: string;
  target?: JsonObject;
  bug?: string;
  surface?: string;
  limit: number;
  timeoutMs: number;
  hiDpi: boolean;
  start: boolean;
  show: boolean;
};

function parseArgs(argv: string[]): InspectArgs {
  const args: InspectArgs = {
    session: "default",
    limit: 200,
    timeoutMs: 8000,
    hiDpi: false,
    start: false,
    show: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--session") {
      args.session = argv[++index] ?? args.session;
    } else if (arg === "--bug") {
      args.bug = argv[++index] ?? "";
    } else if (arg === "--surface") {
      args.surface = argv[++index] ?? "";
    } else if (arg === "--target-id") {
      args.target = { type: "id", id: argv[++index] ?? "" };
    } else if (arg === "--target-kind") {
      const kind = argv[++index] ?? "main";
      args.target = { type: "kind", kind };
    } else if (arg === "--target-index") {
      if (!args.target || args.target.type !== "kind") {
        throw new Error("--target-index requires --target-kind first");
      }
      args.target.index = Number(argv[++index] ?? 0);
    } else if (arg === "--target-title") {
      args.target = { type: "titleContains", text: argv[++index] ?? "" };
    } else if (arg === "--focused") {
      args.target = { type: "focused" };
    } else if (arg === "--main") {
      args.target = { type: "main" };
    } else if (arg === "--limit") {
      args.limit = Number(argv[++index] ?? args.limit);
    } else if (arg === "--timeout") {
      args.timeoutMs = Number(argv[++index] ?? args.timeoutMs);
    } else if (arg === "--hi-dpi") {
      args.hiDpi = true;
    } else if (arg === "--start") {
      args.start = true;
    } else if (arg === "--show") {
      args.show = true;
    }
  }

  return args;
}

async function run(command: string[], label: string): Promise<JsonObject> {
  const proc = Bun.spawn(command, {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  if (exitCode !== 0) {
    return {
      status: "error",
      label,
      exitCode,
      stdout: stdout.trim(),
      stderr: stderr.trim(),
    };
  }

  try {
    return JSON.parse(stdout);
  } catch {
    return {
      status: "error",
      label,
      exitCode,
      stdout: stdout.trim(),
      stderr: stderr.trim(),
      error: "invalid_json_output",
    };
  }
}

function requestId(prefix: string) {
  return `devtools-${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function rpc(session: string, payload: JsonObject, expect: string, timeoutMs: number) {
  return run(
    [
      "bash",
      "scripts/agentic/session.sh",
      "rpc",
      session,
      JSON.stringify(payload),
      "--expect",
      expect,
      "--timeout",
      String(timeoutMs),
    ],
    String(payload.type ?? "rpc"),
  );
}

function responseOf(envelope: JsonObject): JsonObject {
  return (envelope.response as JsonObject | undefined) ?? envelope;
}

function statePromptType(state: JsonObject) {
  if (state.status === "error") {
    return "tool_error";
  }
  return String(state.promptType ?? "");
}

function pickNumber(value: JsonObject, ...names: string[]) {
  for (const name of names) {
    const current = value[name];
    if (typeof current === "number") {
      return current;
    }
  }
  return null;
}

function pickArray(value: JsonObject, ...names: string[]) {
  for (const name of names) {
    const current = value[name];
    if (Array.isArray(current)) {
      return current;
    }
  }
  return [];
}

function warningsFrom(...values: Array<JsonObject | undefined>) {
  const warnings = values.flatMap((value) => {
    const warnings = value?.warnings;
    return Array.isArray(warnings) ? warnings.map(String) : [];
  });
  const errors = values
    .filter((value) => value?.status === "error")
    .map((value) => `${String(value?.label ?? "rpc")}: ${String(value?.error ?? value?.stderr ?? "error")}`);
  return [...warnings, ...errors];
}

function rpcErrors(...values: Array<JsonObject | undefined>) {
  return values
    .filter((value) => value?.status === "error")
    .map((value) => ({
      label: value?.label ?? "rpc",
      exitCode: value?.exitCode ?? null,
      error: value?.error ?? null,
      stderr: value?.stderr ?? null,
    }));
}

function layoutComponents(report: { layout: JsonObject }) {
  return ((report.layout.info as JsonObject | undefined)?.components
    ?? report.layout.components) as unknown;
}

function inspectTargetKind(inspect: JsonObject) {
  return String(inspect.windowKind ?? inspect.window_kind ?? "").toLowerCase();
}

function isMainTarget(inspect: JsonObject) {
  return inspectTargetKind(inspect) === "main";
}

function capabilityStatus(status: "supported" | "unsupported" | "partial" | "blocked", source: string, reason?: string, nextPrimitive?: string) {
  return {
    status,
    source,
    reason: reason ?? null,
    nextPrimitive: nextPrimitive ?? null,
  };
}

function capabilityDetails(report: {
  state: JsonObject;
  elements: JsonObject;
  layout: JsonObject;
  inspect: JsonObject;
}) {
  const stateType = statePromptType(report.state);
  const components = layoutComponents(report);
  const elementCount = Array.isArray(report.elements.elements) ? report.elements.elements.length : 0;
  const screenshotWidth = pickNumber(report.inspect, "screenshotWidth", "screenshot_width");
  const screenshotHeight = pickNumber(report.inspect, "screenshotHeight", "screenshot_height");
  const targetKind = inspectTargetKind(report.inspect);
  const state = (() => {
    if (stateType === "tool_error") {
      return capabilityStatus("blocked", "getState", "state RPC failed", "inspectAutomationWindow");
    }
    if (stateType === "unsupported" || stateType === "target_resolution_failed") {
      return capabilityStatus("unsupported", "getState", String(report.state.reason ?? stateType), "getElements");
    }
    return capabilityStatus("supported", "getState");
  })();
  const elements = (() => {
    if (elementCount === 0) {
      return capabilityStatus("blocked", "getElements", "semantic element collection returned no nodes", "add-surface-element-collector");
    }
    if (report.inspect.semanticQuality && report.inspect.semanticQuality !== "full") {
      return capabilityStatus("partial", "getElements + inspectAutomationWindow", `semanticQuality=${String(report.inspect.semanticQuality)}`, "add-surface-element-collector");
    }
    return capabilityStatus("supported", "getElements + inspectAutomationWindow");
  })();
  const layout = (() => {
    if (Array.isArray(components) && components.length > 0) {
      return capabilityStatus("supported", "getLayoutInfo");
    }
    if (!isMainTarget(report.inspect) && targetKind) {
      return capabilityStatus("unsupported", "getLayoutInfo", `target-scoped layout missing for ${targetKind}`, "devtools.measure");
    }
    return capabilityStatus("blocked", "getLayoutInfo", "layout components unavailable", "devtools.measure");
  })();
  const screenshot = (() => {
    if (screenshotWidth == null || screenshotHeight == null) {
      return capabilityStatus("partial", "inspectAutomationWindow", "screenshot metadata unavailable", "verify-shot-strict-window");
    }
    return capabilityStatus("supported", "inspectAutomationWindow");
  })();

  return {
    state,
    elements,
    layout,
    screenshot,
    windows: capabilityStatus("supported", "listAutomationWindows"),
    batch: {
      ...capabilityStatus("partial", "batch", "inspect is read-only; use act/batch for mutation proof", "devtools.act"),
      setInput: true,
      selectBySemanticId: true,
      openActions: true,
      waitFor: true,
      forceSubmit: false,
    },
  };
}

function missingFieldDetails(report: {
  state: JsonObject;
  elements: JsonObject;
  layout: JsonObject;
  inspect: JsonObject;
}) {
  const missing: Array<{ field: string; reason: string; blocks: string[]; nextPrimitive: string }> = [];
  const details = capabilityDetails(report);
  const stateType = statePromptType(report.state);
  if (stateType === "tool_error" || stateType === "unsupported" || stateType === "target_resolution_failed") {
    missing.push({
      field: "target_state",
      reason: details.state.reason ?? stateType,
      blocks: ["state-dependent red/green proof", "keyboard ownership proof", "selected item proof"],
      nextPrimitive: details.state.nextPrimitive ?? "getElements",
    });
  }
  if (!Array.isArray(report.elements.elements) || report.elements.elements.length === 0) {
    missing.push({
      field: "semantic_elements",
      reason: details.elements.reason ?? "no semantic elements returned",
      blocks: ["visible control identity", "selected/focused node proof", "safe action targeting"],
      nextPrimitive: details.elements.nextPrimitive ?? "add-surface-element-collector",
    });
  }
  if (report.inspect.semanticQuality && report.inspect.semanticQuality !== "full") {
    missing.push({
      field: "full_semantic_elements",
      reason: details.elements.reason ?? "semantic quality is partial",
      blocks: ["row-level proof", "popup content proof", "accessibility mapping"],
      nextPrimitive: details.elements.nextPrimitive ?? "add-surface-element-collector",
    });
  }
  const components = layoutComponents(report);
  if (!Array.isArray(components) || components.length === 0) {
    missing.push({
      field: "target_layout_info",
      reason: details.layout.reason ?? "layout components unavailable",
      blocks: ["clipping proof", "popup anchor drift proof", "overlap proof", "resize pressure proof"],
      nextPrimitive: details.layout.nextPrimitive ?? "devtools.measure",
    });
  }
  const screenshotWidth = pickNumber(report.inspect, "screenshotWidth", "screenshot_width");
  const screenshotHeight = pickNumber(report.inspect, "screenshotHeight", "screenshot_height");
  if (screenshotWidth == null || screenshotHeight == null) {
    missing.push({
      field: "screenshot_metadata",
      reason: details.screenshot.reason ?? "screenshot metadata unavailable",
      blocks: ["visual proof", "screenshot-to-semantics comparison", "nonblank target proof"],
      nextPrimitive: details.screenshot.nextPrimitive ?? "verify-shot-strict-window",
    });
  }
  const seen = new Set<string>();
  return missing.filter((item) => {
    if (seen.has(item.field)) {
      return false;
    }
    seen.add(item.field);
    return true;
  });
}

function missingFields(report: {
  state: JsonObject;
  elements: JsonObject;
  layout: JsonObject;
  inspect: JsonObject;
}) {
  return missingFieldDetails(report).map((item) => item.field);
}

function capabilities(report: {
  state: JsonObject;
  elements: JsonObject;
  layout: JsonObject;
  inspect: JsonObject;
}) {
  const components = layoutComponents(report);
  return {
    state: !["tool_error", "unsupported", "target_resolution_failed"].includes(statePromptType(report.state)),
    elements: Array.isArray(report.elements.elements) && report.elements.elements.length > 0,
    fullSemantics: report.inspect.semanticQuality === "full",
    layout: Array.isArray(components) && components.length > 0,
    screenshotMetadata: pickNumber(report.inspect, "screenshotWidth", "screenshot_width") != null
      && pickNumber(report.inspect, "screenshotHeight", "screenshot_height") != null,
    pixelProbes: Array.isArray(report.inspect.pixelProbes) || Array.isArray(report.inspect.pixel_probes),
    hitPoints: pickArray(report.inspect, "suggestedHitPoints", "suggested_hit_points").length > 0,
  };
}

function recommendedNext(missing: string[]) {
  const next = ["devtools.measure"];
  if (missing.includes("target_state")) {
    next.push("inspectAutomationWindow", "getElements");
  }
  if (missing.includes("target_layout_info")) {
    next.push("add-target-scoped-layout-info");
  }
  if (missing.includes("full_semantic_elements")) {
    next.push("add-surface-element-collector");
  }
  if (missing.includes("screenshot_metadata")) {
    next.push("verify-shot-strict-window");
  }
  return [...new Set(next)];
}

function classification(errors: JsonObject[], missing: string[]) {
  if (errors.length > 0) {
    const transportCodes = errors
      .map((entry) => classifyTransportError(entry))
      .filter((value) => value !== "ok");
    if (transportCodes.length > 0) {
      return transportCodes[0];
    }
    return "blocked-by-response-timeout";
  }
  if (missing.length > 0) {
    return "blocked-by-missing-primitive";
  }
  return "ok";
}

function primitiveStack(entries: Array<{ name: string; command: string; envelope: JsonObject; classification?: string }>) {
  return entries.map((entry) => ({
    name: entry.name,
    command: entry.command,
    status: entry.envelope.status === "error" ? "blocked" : "ok",
    classification: entry.classification ?? classifyTransportError(entry.envelope),
    receiptId: (entry.envelope.response as JsonObject | undefined)?.requestId ?? null,
    error: diagnostic(
      entry.envelope.status === "error"
        ? String(entry.envelope.error ?? entry.envelope.stderr ?? "error")
        : null,
    ),
  }));
}

function visibleWindowProof(inspect: JsonObject) {
  const screenshotWidth = pickNumber(inspect, "screenshotWidth", "screenshot_width");
  const screenshotHeight = pickNumber(inspect, "screenshotHeight", "screenshot_height");
  return {
    windowId: inspect.windowId ?? null,
    windowKind: inspect.windowKind ?? null,
    title: inspect.title ?? null,
    visible: inspect.visible ?? null,
    focused: inspect.focused ?? null,
    frontmost: inspect.frontmost ?? null,
    resolvedBounds: inspect.resolvedBounds ?? null,
    screenshotIdentity: {
      width: screenshotWidth,
      height: screenshotHeight,
      osWindowId: inspect.osWindowId ?? null,
      targetBoundsInScreenshot: inspect.targetBoundsInScreenshot ?? null,
    },
    strictTargetMatch: inspect.strictTargetMatch ?? null,
  };
}

function likelyOwners(surface: string | undefined, targetKind: string, missing: string[]) {
  const owners = new Set<string>();
  const normalizedSurface = String(surface ?? "").toLowerCase();
  if (normalizedSurface.includes("notes") || targetKind === "notes") {
    owners.add("src/notes/window.rs");
    owners.add("src/notes/window/render_editor_body.rs");
    owners.add("scripts/devtools/notes.ts");
  }
  if (normalizedSurface.includes("actions") || targetKind === "actionsdialog") {
    owners.add("src/actions/dialog.rs");
    owners.add("src/actions/window.rs");
    owners.add("scripts/devtools/actions.ts");
  }
  if (normalizedSurface.includes("dictation")) {
    owners.add("src/dictation");
    owners.add("scripts/devtools/dictation.ts");
    owners.add("scripts/devtools/media.ts");
  }
  if (missing.includes("target_layout_info")) {
    owners.add("src/protocol/types/layout_info.rs");
    owners.add("scripts/devtools/layout.ts");
  }
  if (missing.includes("semantic_elements") || missing.includes("full_semantic_elements")) {
    owners.add("src/protocol/types/elements.rs");
    owners.add("scripts/devtools/elements.ts");
  }
  if (missing.includes("target_state")) {
    owners.add("src/prompt_handler/mod.rs");
    owners.add("src/main_entry/runtime_stdin.rs");
  }
  return [...owners];
}

function recipeBoundaryReason(classification: string, missing: string[]) {
  if (classification === "ok") {
    return "Direct DevTools primitives are available; use recipes only after isolating a stable regression path.";
  }
  if (missing.length > 0) {
    return `Do not use a canned recipe to hide missing instrumentation: build or run the named primitive for ${missing.join(", ")} first.`;
  }
  return "Do not replace target-scoped proof with screenshots, sleeps, or broad recipes; resolve the blocked primitive first.";
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  if (args.start) {
    await run(["bash", "scripts/agentic/session.sh", "start", args.session], "session-start");
  }
  if (args.show) {
    await run([
      "bash",
      "scripts/agentic/session.sh",
      "send",
      args.session,
      JSON.stringify({ type: "show" }),
      "--await-parse",
      "--timeout",
      String(args.timeoutMs),
    ], "session-show");
  }

  const target = args.target ?? { type: "focused" };
  const windowsEnvelope = await rpc(args.session, {
    type: "listAutomationWindows",
    requestId: requestId("windows"),
  }, "automationWindowListResult", args.timeoutMs);
  const inspectEnvelope = await rpc(args.session, {
    type: "inspectAutomationWindow",
    requestId: requestId("inspect"),
    target,
    hiDpi: args.hiDpi,
    probes: [],
  }, "automationInspectResult", args.timeoutMs);
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: requestId("state"),
    target,
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
  const elementsEnvelope = await rpc(args.session, {
    type: "getElements",
    requestId: requestId("elements"),
    target,
    limit: args.limit,
  }, "elementsResult", args.timeoutMs);
  const layoutEnvelope = await rpc(args.session, {
    type: "getLayoutInfo",
    requestId: requestId("layout"),
    target,
  }, "layoutInfoResult", args.timeoutMs);
  const windowsAfterEnvelope = await rpc(args.session, {
    type: "listAutomationWindows",
    requestId: requestId("windows-after"),
  }, "automationWindowListResult", args.timeoutMs);

  const windows = responseOf(windowsEnvelope);
  const inspect = responseOf(inspectEnvelope).snapshot as JsonObject | undefined ?? responseOf(inspectEnvelope);
  const state = responseOf(stateEnvelope);
  const elements = responseOf(elementsEnvelope);
  const layout = responseOf(layoutEnvelope);
  const windowsAfter = responseOf(windowsAfterEnvelope);
  const identity = targetIdentity(
    { target, strict: true, expectedSurfaceKind: args.surface ?? "" },
    inspect,
    windows,
  );
  const transaction = proofTransactionIdentity(args.session, identity.resolvedTarget);
  const transactionMissingFields = strictTransactionMissingFields(transaction);
  const lifetimeConsistency = compareWindowLifetimeSnapshots(
    identity.resolvedTarget.automationId,
    windows,
    windowsAfter,
  );
  const report = { state, elements, layout, inspect };
  const missing = [
    ...missingFields(report),
    ...transactionMissingFields.map((field) => `transaction.${field}`),
    ...(lifetimeConsistency.consistent ? [] : ["transaction.windowLifetime"]),
  ];
  const missingDetails = missingFieldDetails(report);
  const errors = rpcErrors(
    windowsEnvelope,
    inspectEnvelope,
    stateEnvelope,
    elementsEnvelope,
    layoutEnvelope,
    windowsAfterEnvelope,
  );
  const baseClassification = classification(errors, missing);
  const classificationValue = baseClassification !== "ok"
    ? baseClassification
    : lifetimeConsistency.consistent
      ? "ok"
      : "blocked-by-stale-generation";
  const screenshotWidth = pickNumber(inspect, "screenshotWidth", "screenshot_width");
  const screenshotHeight = pickNumber(inspect, "screenshotHeight", "screenshot_height");
  const suggestedHitPoints = pickArray(inspect, "suggestedHitPoints", "suggested_hit_points");
  const pixelProbes = pickArray(inspect, "pixelProbes", "pixel_probes");
  const stack = primitiveStack([
    { name: "devtools.targets.list", command: "listAutomationWindows", envelope: windowsEnvelope },
    { name: "devtools.targets.inspect", command: "inspectAutomationWindow", envelope: inspectEnvelope },
    { name: "devtools.state.inspect", command: "getState(summaryOnly)", envelope: stateEnvelope },
    { name: "devtools.elements.snapshot", command: "getElements", envelope: elementsEnvelope },
    { name: "devtools.layout.measure", command: "getLayoutInfo", envelope: layoutEnvelope },
  ]);
  const targetKind = inspectTargetKind(inspect);

  emitValidatedReceipt("devtools.inspect.orchestrate", {
    schemaVersion: 2,
    tool: "script-kit-devtools.inspect",
    command: "inspect.orchestrate",
    session: args.session,
    sessionId: args.session,
    bug: {
      text: userContent(args.bug ?? null),
      suspectedSurface: args.surface ?? null,
      hints: [userContent(args.bug ?? null), args.surface].filter(Boolean),
    },
    requestedTarget: target,
    resolvedTarget: identity.resolvedTarget,
    transaction,
    transactionValidation: {
      valid: lifetimeConsistency.consistent && transactionMissingFields.length === 0,
      lifetimeConsistency: {
        ...lifetimeConsistency,
        errors: productStatic(lifetimeConsistency.errors),
      },
      missingFields: transactionMissingFields,
    },
    visibleWindowProof: visibleWindowProof(inspect),
    primitiveStack: stack,
    status: errors.length === 0 ? (missing.length === 0 ? "ok" : "partial") : "blocked",
    classification: classificationValue,
    errors: diagnostic(errors),
    windows: diagnostic(windows),
    target: {
      windowId: inspect.windowId ?? null,
      windowKind: inspect.windowKind ?? null,
      title: externalContent(inspect.title ?? null),
      resolvedBounds: inspect.resolvedBounds ?? null,
      semanticQuality: inspect.semanticQuality ?? null,
      targetBoundsInScreenshot: inspect.targetBoundsInScreenshot ?? null,
      surfaceHitPoint: inspect.surfaceHitPoint ?? null,
      suggestedHitPoints,
    },
    capabilities: capabilities(report),
    capabilityDetails: capabilityDetails(report),
    missingFields: missing,
    missingFieldDetails: missingDetails,
    recommendedNext: recommendedNext(missing),
    recommendedNextPrimitives: recommendedNext(missing),
    likelyOwners: likelyOwners(args.surface, targetKind, missing),
    doNotUseRecipeReason: recipeBoundaryReason(classificationValue, missing),
    cleanup: {
      required: Boolean(args.start),
      command: args.start ? `scripts/agentic/session.sh stop ${args.session}` : null,
      statusCommand: `scripts/agentic/session.sh status ${args.session}`,
    },
    warnings: diagnostic(warningsFrom(inspect, elements, layout, state)),
    state: diagnostic(state),
    elements: diagnostic(elements),
    layout: diagnostic(layout),
    screenshot: {
      width: screenshotWidth,
      height: screenshotHeight,
      osWindowId: inspect.osWindowId ?? null,
      pixelProbes,
    },
  });
}

await main();
