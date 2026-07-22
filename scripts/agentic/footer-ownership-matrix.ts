#!/usr/bin/env bun
/**
 * State-first footer ownership matrix.
 *
 * Runs prompt and builtin surfaces through getState.activeFooter plus
 * getElements footer rows. The matrix fails on duplicate footer rows, missing
 * footer rows, stale native ownership, and DropPrompt's empty-submit affordance.
 */

import { mkdirSync, writeFileSync } from "fs";
import { join, resolve } from "path";

type Json = Record<string, any>;

const repoRoot = resolve(import.meta.dir, "../..");
const outDir = join(repoRoot, ".test-output", "footer-ownership-matrix");
mkdirSync(outDir, { recursive: true });

const session = arg("--session", "footer-ownership-matrix");
const timeoutMs = Number(arg("--timeout", "8000"));

type ExpectedOwner = "native" | "prompt" | "popup" | "content" | "none";

interface CaseSpec {
  id: string;
  command: Json;
  entry?: "send" | "search" | "scriptListFilter";
  expectSurface: string | null;
  expectOwner: ExpectedOwner;
  expectRows: 0 | 1;
  expectDisabled?: { action: string; reason: string };
  expectLabels?: string[];
  forbidLabels?: string[];
}

const choice = { name: "Alpha", value: "alpha" };
const action = { name: "Inspect", value: "inspect", hasAction: false };

const cases: CaseSpec[] = [
  { id: "form", command: { type: "form", id: "footer-form", html: "<input name='x' />", actions: [action] }, expectSurface: "form_prompt", expectOwner: "native", expectRows: 1 },
  { id: "select", command: { type: "select", id: "footer-select", placeholder: "Pick", choices: [choice], multiple: false }, expectSurface: "select_prompt", expectOwner: "native", expectRows: 1 },
  { id: "drop-empty", command: { type: "drop", id: "footer-drop" }, expectSurface: "drop_prompt", expectOwner: "native", expectRows: 1, expectDisabled: { action: "run", reason: "no_files" } },
  { id: "env", command: { type: "env", id: "footer-env", key: "FOOTER_MATRIX", prompt: "Value", title: "Env", secret: false }, expectSurface: "env_prompt", expectOwner: "native", expectRows: 1 },
  { id: "path", command: { type: "path", id: "footer-path", startPath: repoRoot, hint: "Pick a path" }, expectSurface: "path_prompt", expectOwner: "native", expectRows: 1, expectLabels: ["Select", "Actions"], forbidLabels: ["AI"] },
  { id: "terminal", command: { type: "term", id: "footer-term", command: "printf ready", actions: [action] }, expectSurface: null, expectOwner: "prompt", expectRows: 1 },
  { id: "editor", command: { type: "editor", id: "footer-editor", content: "hello", language: "text", actions: [action] }, expectSurface: "editor_prompt", expectOwner: "native", expectRows: 1 },
  { id: "webcam", command: { type: "triggerBuiltin", builtinId: "builtin/webcam" }, expectSurface: "webcam_prompt", expectOwner: "native", expectRows: 1 },
  { id: "template", command: { type: "template", id: "footer-template", template: "Hello {{name}}" }, expectSurface: "template_prompt", expectOwner: "native", expectRows: 1 },
  { id: "mic-stub", command: { type: "micro", id: "footer-micro", placeholder: "Small", choices: [choice] }, expectSurface: null, expectOwner: "none", expectRows: 0 },
  { id: "about", command: { type: "openAbout" }, expectSurface: null, expectOwner: "content", expectRows: 1 },
  { id: "actions-popup", command: { type: "arg", id: "footer-actions", placeholder: "Actions", choices: [choice], actions: [action] }, expectSurface: "arg_prompt", expectOwner: "popup", expectRows: 1 },
  { id: "create-flow", command: { type: "triggerBuiltin", builtinId: "builtin/new-script" }, expectSurface: "naming_prompt", expectOwner: "native", expectRows: 1 },
  { id: "sdk-browser", command: { type: "triggerBuiltin", builtinId: "builtin/sdk-reference" }, expectSurface: null, expectOwner: "prompt", expectRows: 1 },
  { id: "sdk-terminal", command: { type: "term", id: "footer-sdk-term", command: "printf sdk", actions: [action] }, expectSurface: null, expectOwner: "prompt", expectRows: 1 },
  { id: "kit-store", command: { type: "triggerBuiltin", builtinId: "builtin/browse-kit-store" }, expectSurface: "kit_store_browse", expectOwner: "native", expectRows: 1 },
  { id: "theme-chooser", command: { type: "triggerBuiltin", builtinId: "builtin/choose-theme" }, expectSurface: "theme_chooser", expectOwner: "native", expectRows: 1 },
  { id: "quick-terminal", command: { type: "triggerBuiltin", builtinId: "builtin/quick-terminal" }, expectSurface: "quick_terminal", expectOwner: "native", expectRows: 1 },
  { id: "menu-syntax-popup", command: { type: "setFilter", text: ":" }, entry: "scriptListFilter", expectSurface: "script_list", expectOwner: "popup", expectRows: 1 },
];

function arg(name: string, fallback: string): string {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] ?? fallback : fallback;
}

function run(cmd: string[], input?: string): string {
  const proc = Bun.spawnSync(cmd, { cwd: repoRoot, stdin: input ?? undefined, stdout: "pipe", stderr: "pipe" });
  const stdout = proc.stdout.toString().trim();
  const stderr = proc.stderr.toString().trim();
  if (proc.exitCode !== 0) {
    throw new Error(`${cmd.join(" ")} failed\n${stdout}\n${stderr}`);
  }
  return stdout;
}

function rpc(payload: Json, expect: string): Json {
  const requestId = String(payload.requestId ?? `${payload.type}-${Date.now()}-${Math.random().toString(16).slice(2)}`);
  const json = JSON.stringify({ requestId, ...payload });
  const raw = run(["bash", "scripts/agentic/session.sh", "rpc", session, json, "--expect", expect, "--timeout", String(timeoutMs)]);
  let envelope: Json;
  try {
    envelope = JSON.parse(raw);
  } catch (error) {
    throw new Error(`${payload.type} returned invalid JSON (${raw.length} bytes): ${String(error)}\n${raw.slice(0, 500)}`);
  }
  if (envelope.status !== "ok") {
    throw new Error(`${payload.type} failed: ${JSON.stringify(envelope)}`);
  }
  return envelope.response;
}

function send(payload: Json): void {
  run([
    "bash",
    "scripts/agentic/session.sh",
    "send",
    session,
    JSON.stringify({ requestId: `${payload.type}-${Date.now()}`, ...payload }),
    "--await-parse",
    "--timeout",
    String(timeoutMs),
  ]);
}

function state(): Json {
  return rpc({ type: "getState" }, "stateResult");
}

function elements(): Json {
  return rpc({ type: "getElements", limit: 1000 }, "elementsResult");
}

async function waitForState(spec: CaseSpec): Promise<Json> {
  let last: unknown;
  for (let attempt = 0; attempt < 30; attempt += 1) {
    try {
      const stateResult = state();
      const nativeSurface = stateResult.surfaceContract?.nativeFooterSurface ?? null;
      const owner = stateResult.activeFooter?.owner;
      if (nativeSurface === spec.expectSurface && owner === spec.expectOwner) {
        return stateResult;
      }
      last = { nativeSurface, owner, promptType: stateResult.promptType, semanticSurface: stateResult.semanticSurface };
    } catch (error) {
      last = error;
    }
    await Bun.sleep(100);
  }
  throw new Error(`${spec.id}: timed out waiting for surface/owner ${spec.expectSurface}/${spec.expectOwner}; last=${JSON.stringify(String((last as Error)?.stack ?? last))}`);
}

async function openFromSearch(text: string): Promise<void> {
  rpc({ type: "hide" }, "windowVisibilityAck");
  rpc({ type: "show" }, "windowVisibilityAck");
  await Bun.sleep(150);
  send({ type: "setFilter", text });
  await Bun.sleep(350);
  send({ type: "simulateKey", key: "enter", modifiers: [] });
}

async function filterScriptList(text: string): Promise<void> {
  rpc({ type: "hide" }, "windowVisibilityAck");
  rpc({ type: "show" }, "windowVisibilityAck");
  await Bun.sleep(150);
  send({ type: "setFilter", text });
}

function footerRows(elementsResult: Json): Json[] {
  return (elementsResult.elements ?? []).filter((element: Json) =>
    element.role === "footer" && String(element.kind ?? "").endsWith("FooterRow")
  );
}

function footerButtons(elementsResult: Json): Json[] {
  return (elementsResult.elements ?? []).filter((element: Json) =>
    element.role === "footer" && String(element.kind ?? "").endsWith("FooterButton")
  );
}

function assertCase(spec: CaseSpec, stateResult: Json, elementsResult: Json): Json {
  const activeFooter = stateResult.activeFooter;
  if (!activeFooter) throw new Error(`${spec.id}: missing getState.activeFooter`);

  const nativeSurface = stateResult.surfaceContract?.nativeFooterSurface ?? null;
  if (nativeSurface !== spec.expectSurface) {
    throw new Error(`${spec.id}: expected nativeFooterSurface ${spec.expectSurface}, got ${nativeSurface}`);
  }
  if (activeFooter.owner !== spec.expectOwner) {
    throw new Error(`${spec.id}: expected activeFooter.owner ${spec.expectOwner}, got ${activeFooter.owner}`);
  }

  const rows = footerRows(elementsResult);
  if (rows.length !== spec.expectRows) {
    throw new Error(`${spec.id}: expected ${spec.expectRows} footer rows, got ${rows.length}`);
  }
  if (rows.length > 1) throw new Error(`${spec.id}: double footer rows detected`);

  const buttons = footerButtons(elementsResult);
  const nativeButtons = buttons.filter((button) => button.kind === "nativeFooterButton");
  const promptButtons = buttons.filter((button) => button.kind === "promptFooterButton");
  if (nativeButtons.length > 0 && promptButtons.length > 0) {
    throw new Error(`${spec.id}: duplicate native and prompt footer buttons`);
  }
  if (spec.expectOwner === "native" && !activeFooter.nativeFooterHostInstalled) {
    throw new Error(`${spec.id}: native owner without nativeFooterHostInstalled`);
  }

  for (const label of spec.expectLabels ?? []) {
    if (!activeFooter.buttons.some((button: Json) => button.label === label)) {
      throw new Error(`${spec.id}: missing footer label ${label}`);
    }
  }
  for (const label of spec.forbidLabels ?? []) {
    if (activeFooter.buttons.some((button: Json) => button.label === label)) {
      throw new Error(`${spec.id}: forbidden footer label ${label}`);
    }
  }
  if (spec.expectDisabled) {
    const button = activeFooter.buttons.find((candidate: Json) => candidate.action === spec.expectDisabled?.action);
    if (!button || button.enabled !== false || button.actionDisabled !== spec.expectDisabled.reason) {
      throw new Error(`${spec.id}: expected ${spec.expectDisabled.action} disabled as ${spec.expectDisabled.reason}`);
    }
  }

  return { id: spec.id, owner: activeFooter.owner, surface: nativeSurface, rows: rows.length, buttons: activeFooter.buttons };
}

const receipt: Json = { schemaVersion: 1, session, cases: [] };

try {
  run(["bash", "scripts/agentic/session.sh", "start", session]);
  rpc({ type: "show" }, "windowVisibilityAck");
  for (const spec of cases) {
    if (spec.id === "actions-popup") {
      send(spec.command);
      send({ type: "simulateKey", key: "k", modifiers: ["cmd"] });
    } else if (spec.entry === "search") {
      await openFromSearch(spec.command.text);
    } else if (spec.entry === "scriptListFilter") {
      await filterScriptList(spec.command.text);
    } else {
      send(spec.command);
    }
    await Bun.sleep(150);
    const stateResult = await waitForState(spec);
    const elementsResult = elements();
    receipt.cases.push(assertCase(spec, stateResult, elementsResult));
    if (spec.id === "actions-popup" || spec.id === "menu-syntax-popup") {
      send({ type: "simulateKey", key: "escape", modifiers: [] });
    }
  }
  const hide = rpc({ type: "hide" }, "windowVisibilityAck");
  if (hide.windowVisible !== false) {
    throw new Error(`final cleanup failed: hide=${hide.windowVisible}`);
  }
  receipt.finalWindowVisible = hide.windowVisible;
  receipt.status = "pass";
  writeFileSync(join(outDir, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ status: "pass", receiptPath: ".test-output/footer-ownership-matrix/receipt.json" }, null, 2));
} catch (error) {
  try {
    rpc({ type: "hide" }, "windowVisibilityAck");
  } catch {}
  receipt.status = "fail";
  receipt.error = String((error as Error)?.stack ?? error);
  writeFileSync(join(outDir, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
  console.error(receipt.error);
  process.exit(1);
}
