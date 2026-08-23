#!/usr/bin/env bun
/**
 * Shared exact-target identity guard for advanced agentic scenarios.
 *
 * Kind/index targets are discovery-only. A proof run must promote them to a
 * stable exact `{ type: "id" }` target once, then reuse that target for every
 * RPC, native input focus receipt, and strict capture assertion.
 */

import { resolve } from "path";
import { assertNoninteractiveSubprocess } from "../devtools/lib/operator-safety.ts";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");

export type AutomationTargetJson =
  | { type: "focused" }
  | { type: "main" }
  | { type: "id"; id: string }
  | { type: "kind"; kind: string; index?: number }
  | { type: "titleContains"; text: string };

export interface TargetThreadIdentity {
  targetJson: AutomationTargetJson;
  automationWindowId: string;
  osWindowId: number | null;
  surfaceId: string | null;
  windowKind: string | null;
  title?: string | null;
  semanticSurface?: string | null;
  parentAutomationWindowId?: string | null;
  popupFamily?: string | null;
  popupId?: string | null;
  agent_chatViewId?: string | null;
  agent_chatGeneration?: number | null;
  originAutomationWindowId?: string | null;
  originSurfaceId?: string | null;
  originAgentChatViewId?: string | null;
  originAgentChatGeneration?: number | null;
  portalId?: string | null;
  portalFamily?: string | null;
  permissionSurfaceId?: string | null;
  recorderId?: string | null;
  recorderGeneration?: number | null;
}

export interface TargetThreadFailure {
  code:
    | "target_resolution_failed"
    | "target_identity_drift"
    | "missing_os_window_id"
    | "missing_surface_id"
    | "wrong_popup_family"
    | "wrong_popup_id"
    | "insufficient_target_count"
    | "untargeted_rpc_forbidden"
    | "missing_origin_identity"
    | "origin_identity_drift"
    | "portal_target_resolution_failed"
    | "portal_rows_missing"
    | "selection_not_found"
    | "portal_return_failed"
    | "wrong_context_part_origin"
    | "context_part_missing"
    | "unredacted_path_leak"
    | "permission_surface_unavailable"
    | "missing_permission_status"
    | "invalid_permission_status_kind"
    | "permission_status_mismatch"
    | "permission_prompt_attempted"
    | "system_settings_opened"
    | "forbidden_permission_mutation"
    | "unsafe_config_root"
    | "recorder_surface_unavailable"
    | "missing_recorder_focus"
    | "recorder_focus_drift"
    | "native_chord_failed"
    | "chord_capture_failed"
    | "captured_chord_mismatch"
    | "state_elements_mismatch"
    | "global_hotkey_leak"
    | "hotkey_persisted_outside_sandbox"
    | "missing_portal_round_trip_origin_receipt"
    | "permission_preflight_failed"
    | "missing_shortcut_recorder_capture_receipt"
    | "missing_template_prompt_automation_receipt"
    | "missing_current_app_commands_frontmost_receipt"
    | "missing_actions_captured_subject_receipt"
    | "template_prompt_state_missing"
    | "template_prompt_elements_missing"
    | "template_prompt_actions_unavailable"
    | "template_prompt_force_submit_failed"
    | "current_app_snapshot_missing"
    | "current_app_alias_drift"
    | "current_app_filter_drift"
    | "current_app_wrong_app_execution"
    | "actions_subject_missing"
    | "actions_subject_drift"
    | "actions_frame_drift"
    | "actions_focus_restore_failed"
    | "missing_drop_prompt_native_drop_receipt"
    | "drop_prompt_path_leak"
    | "path_prompt_filesystem_edge_failed"
    | "missing_screenshot_identity_context_receipt"
    | "missing_clipboard_portal_range_receipt"
    | "missing_browser_cache_identity_receipt"
    | "missing_scroll_selection_reanchor_receipt";
  expected?: Partial<TargetThreadIdentity>;
  actual?: Partial<TargetThreadIdentity>;
  stepName: string;
  message: string;
}

export interface StepReceipt {
  name: string;
  status: "pass" | "fail" | "error" | "skipped";
  output: unknown;
  durationMs: number;
}

export async function runTool(
  cmd: string[],
  label: string
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  assertNoninteractiveSubprocess(cmd);
  const proc = Bun.spawn(cmd, {
    stdout: "pipe",
    stderr: "pipe",
    cwd: PROJECT_ROOT,
  });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const exitCode = await proc.exited;
  console.error(
    JSON.stringify({
      event: "target_thread.tool_complete",
      label,
      exitCode,
      ts: new Date().toISOString(),
    })
  );
  return { exitCode, stdout: stdout.trim(), stderr: stderr.trim() };
}

function parseJson(text: string): Record<string, unknown> {
  try {
    return JSON.parse(text);
  } catch {
    return { raw: text };
  }
}

function responsePayload(envelope: Record<string, unknown>): Record<string, unknown> {
  const response = envelope.response;
  return response && typeof response === "object"
    ? (response as Record<string, unknown>)
    : envelope;
}

function identityFromInspect(
  parsed: Record<string, unknown>,
  exactId?: string
): TargetThreadIdentity | null {
  if (parsed.status && parsed.status !== "ok") return null;

  const inspect = parsed.inspect && typeof parsed.inspect === "object"
    ? (parsed.inspect as Record<string, unknown>)
    : {};
  const automationWindowId =
    parsed.automationWindowId != null
      ? String(parsed.automationWindowId)
      : inspect.windowId != null
        ? String(inspect.windowId)
        : exactId ?? "";

  if (!automationWindowId) return null;

  const osWindowId =
    typeof parsed.osWindowId === "number"
      ? parsed.osWindowId
      : typeof inspect.osWindowId === "number"
        ? (inspect.osWindowId as number)
        : null;

  const windowKind =
    typeof parsed.windowKind === "string"
      ? parsed.windowKind
      : typeof inspect.windowKind === "string"
        ? (inspect.windowKind as string)
        : null;

  const title =
    typeof parsed.title === "string"
      ? parsed.title
      : typeof inspect.title === "string"
        ? (inspect.title as string)
        : null;

  const semanticSurface =
    typeof inspect.semanticSurface === "string"
      ? (inspect.semanticSurface as string)
      : null;

  return {
    targetJson: { type: "id", id: automationWindowId },
    automationWindowId,
    osWindowId,
    surfaceId: typeof parsed.surfaceId === "string" ? parsed.surfaceId : null,
    windowKind,
    title,
    semanticSurface,
    parentAutomationWindowId:
      typeof inspect.parentAutomationWindowId === "string"
        ? (inspect.parentAutomationWindowId as string)
        : null,
    popupFamily:
      typeof inspect.popupFamily === "string"
        ? (inspect.popupFamily as string)
        : null,
    popupId:
      typeof inspect.popupId === "string"
        ? (inspect.popupId as string)
        : automationWindowId.startsWith("agent_chat-") ? automationWindowId : null,
    agent_chatViewId:
      typeof inspect.agent_chatViewId === "string" ? (inspect.agent_chatViewId as string) : null,
    agent_chatGeneration:
      typeof inspect.agent_chatGeneration === "number"
        ? (inspect.agent_chatGeneration as number)
        : null,
    originAutomationWindowId:
      typeof inspect.originAutomationWindowId === "string"
        ? (inspect.originAutomationWindowId as string)
        : null,
    originSurfaceId:
      typeof inspect.originSurfaceId === "string"
        ? (inspect.originSurfaceId as string)
        : null,
    originAgentChatViewId:
      typeof inspect.originAgentChatViewId === "string"
        ? (inspect.originAgentChatViewId as string)
        : null,
    originAgentChatGeneration:
      typeof inspect.originAgentChatGeneration === "number"
        ? (inspect.originAgentChatGeneration as number)
        : null,
    portalId:
      typeof inspect.portalId === "string" ? (inspect.portalId as string) : null,
    portalFamily:
      typeof inspect.portalFamily === "string"
        ? (inspect.portalFamily as string)
        : null,
    permissionSurfaceId:
      typeof inspect.permissionSurfaceId === "string"
        ? (inspect.permissionSurfaceId as string)
        : null,
    recorderId:
      typeof inspect.recorderId === "string" ? (inspect.recorderId as string) : null,
    recorderGeneration:
      typeof inspect.recorderGeneration === "number"
        ? (inspect.recorderGeneration as number)
        : null,
  };
}

export async function promoteExactTarget(opts: {
  session: string;
  kind: string;
  index: number;
  expected?: {
    automationWindowId?: string;
    popupFamily?: string;
    popupId?: string;
    windowKind?: string;
  };
}): Promise<TargetThreadIdentity> {
  const result = await runTool(
    [
      "bun",
      "scripts/agentic/automation-window.ts",
      "inspect",
      "--session",
      opts.session,
      "--kind",
      opts.kind,
      "--index",
      String(opts.index),
    ],
    "promote-exact-target"
  );
  const parsed = parseJson(result.stdout);
  const identity = identityFromInspect(parsed);

  if (result.exitCode !== 0 || !identity) {
    throw Object.assign(
      new Error(result.stdout || result.stderr || "target promotion failed"),
      {
        failure: {
          code: "target_resolution_failed",
          stepName: "promote-exact-target",
          message: "Could not promote kind/index target to exact automation id",
          actual: parsed,
        } satisfies TargetThreadFailure,
      }
    );
  }

  const expected = opts.expected ?? {};
  if (
    expected.automationWindowId &&
    identity.automationWindowId !== expected.automationWindowId
  ) {
    throwDrift("target_identity_drift", "promote-exact-target", expected, identity);
  }
  if (expected.windowKind && identity.windowKind !== expected.windowKind) {
    throwDrift("target_identity_drift", "promote-exact-target", expected, identity);
  }
  if (expected.popupFamily && identity.popupFamily !== expected.popupFamily) {
    throwDrift("wrong_popup_family", "promote-exact-target", expected, identity);
  }
  if (expected.popupId && identity.popupId !== expected.popupId) {
    throwDrift("wrong_popup_id", "promote-exact-target", expected, identity);
  }

  return identity;
}

function throwDrift(
  code: TargetThreadFailure["code"],
  stepName: string,
  expected: Partial<TargetThreadIdentity>,
  actual: Partial<TargetThreadIdentity>
): never {
  throw Object.assign(new Error(`${code} at ${stepName}`), {
    failure: {
      code,
      stepName,
      expected,
      actual,
      message: `${code}: target identity did not match expected values`,
    } satisfies TargetThreadFailure,
  });
}

export async function assertTargetStable(opts: {
  session: string;
  identity: TargetThreadIdentity;
  stepName: string;
}): Promise<
  | { ok: true; identity: TargetThreadIdentity }
  | { ok: false; failure: TargetThreadFailure }
> {
  const result = await runTool(
    [
      "bun",
      "scripts/agentic/automation-window.ts",
      "inspect",
      "--session",
      opts.session,
      "--id",
      opts.identity.automationWindowId,
    ],
    `assert-target-stable:${opts.stepName}`
  );
  const actual = identityFromInspect(parseJson(result.stdout), opts.identity.automationWindowId);
  if (result.exitCode !== 0 || !actual) {
    return {
      ok: false,
      failure: {
        code: "target_resolution_failed",
        stepName: opts.stepName,
        expected: opts.identity,
        message: "Exact target could not be inspected during stability check",
      },
    };
  }

  const expected = opts.identity;
  const driftFields: Array<keyof TargetThreadIdentity> = [
    "automationWindowId",
    "surfaceId",
    "windowKind",
    "osWindowId",
    "agent_chatViewId",
    "agent_chatGeneration",
  ];
  for (const field of driftFields) {
    if (expected[field] != null && actual[field] !== expected[field]) {
      return {
        ok: false,
        failure: {
          code: "target_identity_drift",
          stepName: opts.stepName,
          expected: { [field]: expected[field] } as Partial<TargetThreadIdentity>,
          actual: { [field]: actual[field] } as Partial<TargetThreadIdentity>,
          message: `Target identity drifted at ${String(field)}`,
        },
      };
    }
  }
  return { ok: true, identity: actual };
}

export async function targetedRpc(opts: {
  session: string;
  identity: TargetThreadIdentity;
  requestId: string;
  command: Record<string, unknown>;
  expect: string;
  timeout?: number;
  stepName?: string;
}): Promise<StepReceipt> {
  const start = Date.now();
  if (opts.identity.targetJson.type !== "id") {
    return {
      name: opts.stepName ?? opts.requestId,
      status: "error",
      durationMs: Date.now() - start,
      output: {
        failure: {
          code: "untargeted_rpc_forbidden",
          stepName: opts.stepName ?? opts.requestId,
          message: "targetedRpc requires an exact id target",
          actual: opts.identity,
        } satisfies TargetThreadFailure,
      },
    };
  }

  const payload = {
    ...opts.command,
    requestId: opts.requestId,
    target: opts.identity.targetJson,
  };
  const result = await runTool(
    [
      "bash",
      "scripts/agentic/session.sh",
      "rpc",
      opts.session,
      JSON.stringify(payload),
      "--expect",
      opts.expect,
      "--timeout",
      String(opts.timeout ?? 5000),
    ],
    `targeted-rpc:${opts.requestId}`
  );
  return {
    name: opts.stepName ?? opts.requestId,
    status: result.exitCode === 0 ? "pass" : result.exitCode === 2 ? "error" : "fail",
    output: responsePayload(parseJson(result.stdout || result.stderr)),
    durationMs: Date.now() - start,
  };
}

export async function listNativePeerWindows(opts: {
  family: "agentChatDetached" | "promptPopup";
}): Promise<Array<Record<string, unknown>>> {
  const result = await runTool(["bun", "scripts/agentic/window.ts", "list"], "window-list");
  if (result.exitCode !== 0) return [];
  const parsed = parseJson(result.stdout);
  const surfaces = (parsed.data as { surfaces?: Array<Record<string, unknown>> } | undefined)
    ?.surfaces ?? [];
  if (opts.family === "agentChatDetached") {
    return surfaces.filter((surface) => {
      const surfaceId = String(surface.surfaceId ?? "");
      const title = String(surface.title ?? "");
      return surfaceId.includes("agent_chat") || title.toLowerCase().includes("agent");
    });
  }
  return surfaces.filter((surface) => {
    const title = String(surface.title ?? "");
    return title.toLowerCase().includes("popup") || title.toLowerCase().includes("prompt");
  });
}
