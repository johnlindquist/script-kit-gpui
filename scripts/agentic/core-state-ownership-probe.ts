#!/usr/bin/env bun

import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const binary = resolve("target-agent/artifacts/core-state-ownership/script-kit-gpui");
const artifactDir = resolve(".artifacts/consistency/UX-018-GOV-001");
const receiptPath = join(artifactDir, "runtime-state-ownership.json");
mkdirSync(artifactDir, { recursive: true });

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(`${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`);
  }
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const output = new TextDecoder().decode(result.stdout);
  const normalized = resolve(executable);
  return output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const command = match[2].trim().split(/\s+/, 1)[0];
      return resolve(command) === normalized ? [Number(match[1])] : [];
    });
}

function elements(result: Json): Json[] {
  return Array.isArray(result.elements) ? result.elements as Json[] : [];
}

function components(result: Json): Json[] {
  return Array.isArray(result.components) ? result.components as Json[] : [];
}

function collectStrings(value: unknown, output: string[] = []): string[] {
  if (typeof value === "string") output.push(value);
  else if (Array.isArray(value)) for (const item of value) collectStrings(item, output);
  else if (value && typeof value === "object") {
    for (const item of Object.values(value)) collectStrings(item, output);
  }
  return output;
}

function sha256(path: string): string {
  const result = Bun.spawnSync(["shasum", "-a", "256", path], {
    stdout: "pipe",
    stderr: "pipe",
  });
  assert(result.exitCode === 0, "failed to hash runtime binary");
  return new TextDecoder().decode(result.stdout).trim().split(/\s+/, 1)[0];
}

async function capture(driver: Driver, filename: string, target: Json = { type: "main" }): Promise<Json> {
  const path = join(artifactDir, filename);
  const shot = await driver.captureScreenshot({ target, savePath: path, timeoutMs: 15_000 });
  assert(!shot.error && shot.width && shot.height, `screenshot failed: ${filename}`, shot);
  return { path, width: shot.width, height: shot.height };
}

async function waitForState(
  driver: Driver,
  predicate: (state: Json) => boolean,
  label: string,
  timeoutMs = 10_000,
): Promise<Json> {
  const deadline = Date.now() + timeoutMs;
  let last: Json = {};
  while (Date.now() < deadline) {
    last = await driver.getState({ target: { type: "main" } }, { timeoutMs: 15_000 });
    if (predicate(last)) return last;
    await Bun.sleep(25);
  }
  throw new Error(`${label} did not settle\n${JSON.stringify(last, null, 2)}`);
}

const cleanups: Json[] = [];

async function withDriver<T>(sessionName: string, run: (driver: Driver) => Promise<T>): Promise<T> {
  assert(exactExecutablePids(binary).length === 0, `${sessionName} started with an owned artifact process already running`);
  const driver = await Driver.launch({
    binary,
    sessionName,
    sandboxHome: true,
    env: {
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
    },
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 15_000,
  });
  try {
    return await run(driver);
  } finally {
    await driver.close();
    const ownedPids = exactExecutablePids(binary);
    const cleanup = {
      sessionName,
      ...driver.finalization,
      ownedProcessCount: ownedPids.length,
      ownedPids,
    };
    cleanups.push(cleanup);
    assert(driver.finalization.processExited, `${sessionName} process did not exit`, cleanup);
    assert(driver.finalization.streamsDrained, `${sessionName} streams did not drain`, cleanup);
    assert(driver.finalization.logWriterClosed, `${sessionName} log writer did not close`, cleanup);
    assert(ownedPids.length === 0, `${sessionName} left an owned artifact process`, cleanup);
  }
}

let receipt: Json = {
  classification: "RUNTIME-FAILED",
  binary,
  binarySha256: sha256(binary),
};

try {
  const builtin = await withDriver("c16-semantic-builtin", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 15_000 });
    driver.send({ type: "triggerBuiltin", builtinId: "builtin/favorites" });
    const state = await waitForState(
      driver,
      (value) => value.promptType === "favorites" && Number(value.visibleChoiceCount ?? -1) === 0,
      "Favorites empty state",
    );
    const semantic = await driver.getElements(
      { target: { type: "main" }, limit: 240, includeHeaders: true },
      { timeoutMs: 15_000 },
    );
    const infoRoots = elements(semantic).filter((element) => element.role === "info-state");
    assert(infoRoots.length === 1, "migrated Favorites empty state did not expose one InfoState root", semantic);
    assert(infoRoots[0].sourceName === "favorites-empty", "Favorites empty state exposed the wrong semantic spec", infoRoots[0]);
    return {
      state: { promptType: state.promptType, visibleChoiceCount: state.visibleChoiceCount },
      infoRoot: infoRoots[0],
      screenshot: await capture(driver, "semantic-builtin-empty.png"),
    };
  });

  const menuSyntax = await withDriver("c16-rich-menu-syntax", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 15_000 });
    await driver.setFilterAndWait("type:script __c16_no_match_ownership__", { timeoutMs: 15_000 });
    await driver.waitForSettle({ timeoutMs: 15_000 });
    const state = await driver.getState({ target: { type: "main" } }, { timeoutMs: 15_000 });
    const semantic = await driver.getElements(
      { target: { type: "main" }, limit: 320, includeHeaders: true },
      { timeoutMs: 15_000 },
    );
    const richRoots = elements(semantic).filter((element) => element.source === "MenuSyntaxMainHint");
    assert(richRoots.length > 0, "menu syntax did not expose its rich composition owner", { state, semantic });
    assert(
      !elements(semantic).some((element) => element.source === "InfoState"),
      "rich menu syntax was flattened into InfoState",
      semantic,
    );
    const layout = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 15_000 });
    return {
      state: { promptType: state.promptType, menuSyntaxMainHint: state.menuSyntaxMainHint ?? null },
      ownerSources: [...new Set(richRoots.map((element) => String(element.source)))],
      componentNames: components(layout).map((component) => component.name).filter((name) => String(name).includes("MenuSyntax")),
      screenshot: await capture(driver, "rich-menu-syntax.png"),
    };
  });

  const about = await withDriver("c16-rich-about", async (driver) => {
    await driver.request({ type: "show" }, { timeoutMs: 15_000 });
    driver.send({ type: "openAbout" });
    const state = await waitForState(driver, (value) => value.promptType === "about", "About surface");
    const layout = await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 15_000 });
    const names = components(layout).map((component) => String(component.name));
    assert(names.includes("AboutHeader"), "About rich header geometry missing", layout);
    assert(names.includes("AboutUpdateCard"), "About rich update card geometry missing", layout);
    assert(names.includes("AboutQuickActions"), "About rich action composition missing", layout);
    return {
      state: { promptType: state.promptType, surfaceKind: state.surfaceKind ?? null },
      componentNames: names.filter((name) => name.startsWith("About")),
      screenshot: await capture(driver, "rich-about.png"),
    };
  });

  const aiRecovery = await withDriver("c16-typed-ai-recovery", async (driver) => {
    driver.send({ type: "openAiWithMockData" });
    await Bun.sleep(300);
    await driver.request(
      {
        type: "setAgentChatTestFixture",
        phase: "error",
        userText: "Please continue.",
        assistantText:
          'Provider error: openai-codex: OpenAI API error (HTTP 429): {"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"free"}}',
      },
      { expect: "externalCommandResult", timeoutMs: 15_000 },
    );
    driver.send({ type: "show" });
    await Bun.sleep(300);
    const windows = await driver.listAutomationWindows({ timeoutMs: 15_000 });
    const agentWindow = (Array.isArray(windows.windows) ? windows.windows as Json[] : []).find(
      (window) => window.semanticSurface === "agentChatChat" && window.visible === true,
    );
    const target = agentWindow?.id ? { type: "id", id: agentWindow.id } : { type: "main" };
    const semantic = await driver.getElements({ target, limit: 400 }, { timeoutMs: 15_000 });
    const strings = collectStrings(semantic);
    const recoveryIds = elements(semantic)
      .map((element) => String(element.semanticId ?? ""))
      .filter((id) => id.startsWith("ai-recovery-"));
    assert(recoveryIds.includes("ai-recovery-card"), "typed AI recovery card missing", semantic);
    assert(recoveryIds.includes("ai-recovery-switch-account"), "capability-owned primary recovery action missing", semantic);
    assert(!strings.some((value) => value.includes("plan_type")), "raw provider diagnostic leaked into UI semantics", semantic);
    const agentState = await driver.request(
      { type: "getAgentChatState", target: { type: "id", id: "main" } },
      { expect: "agent_chatStateResult", timeoutMs: 15_000 },
    );
    const reliability = agentState.reliability as Json | undefined;
    assert(reliability?.phase === "awaitingRecovery", "typed recovery phase missing", agentState);
    assert(reliability?.primaryActionId === "ai-recovery-switch-account", "typed recovery capability chose the wrong primary action", agentState);
    return {
      target,
      recoveryIds,
      reliability,
      screenshot: await capture(driver, "typed-ai-recovery.png", target),
    };
  });

  receipt = {
    classification: "RUNTIME-CONFIRMED",
    binary,
    binarySha256: sha256(binary),
    ownership: {
      semantic: ["InfoState"],
      rich: ["MenuSyntax", "About"],
      typedRecovery: ["AiRecovery"],
      thirdGenericRenderer: false,
    },
    builtin,
    menuSyntax,
    about,
    aiRecovery,
  };
} catch (error) {
  receipt.error = error instanceof Error ? { message: error.message, stack: error.stack } : String(error);
} finally {
  receipt.cleanup = cleanups;
  receipt.finalOwnedProcesses = exactExecutablePids(binary);
  await Bun.write(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
}

console.log(JSON.stringify(receipt, null, 2));
assert(receipt.classification === "RUNTIME-CONFIRMED", "C16 runtime ownership proof failed", receipt);
assert((receipt.cleanup as Json[]).every((cleanup) => cleanup.processExited === true), "a C16 runtime process did not exit", receipt.cleanup);
assert((receipt.cleanup as Json[]).every((cleanup) => cleanup.streamsDrained === true), "a C16 runtime stream did not drain", receipt.cleanup);
assert((receipt.cleanup as Json[]).every((cleanup) => cleanup.logWriterClosed === true), "a C16 runtime log writer did not close", receipt.cleanup);
assert((receipt.cleanup as Json[]).every((cleanup) => cleanup.ownedProcessCount === 0), "a C16 runtime scenario left its process running", receipt.cleanup);
assert((receipt.finalOwnedProcesses as number[]).length === 0, "C16 runtime proof left an artifact process running", receipt.finalOwnedProcesses);
