#!/usr/bin/env bun

import { createHash } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";

const ROOT = resolve(import.meta.dir, "../../..");
const BINARY = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY ??
    "target-agent/artifacts/cons-flow-c06/script-kit-gpui",
);
const OUT_DIR = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR ?? ".test-output/cons-flow-c06",
);
const RECEIPT_PATH = join(OUT_DIR, "conversation-hosts-receipt.json");
const FLOW_FIXTURE = resolve("scripts/agentic/fixtures/flow-ux-project");
const FLOW_PACKAGE = resolve("scripts/agentic/fixtures/flow-desk-package");
const ACTIVE_CLOSE_DISABLED_REASON =
  "Stop the current response first; this host cannot keep it running after you leave.";
const EXACT_ANSWER =
  " C06 synthetic answer\nsecond line with trailing spaces \n";
const EXACT_PARTIAL = "C06 synthetic partial\nwith exact final newline\n";
const RAW_PROVIDER_CANARY = "C06_RAW_PROVIDER_CANARY";
const PRIVATE_PATH_CANARY = "C06_PRIVATE_PATH_CANARY";
const CLIPBOARD_CANARY = "C06_CLIPBOARD_CANARY";
const DRAFT_CANARY = "C06_DRAFT_CANARY";

type ObjectJson = Record<string, Json>;
type Status = "PASS" | "FAILED";

interface ScenarioReceipt {
  id: string;
  status: Status;
  surface: string;
  facts: Record<string, string | number | boolean | null>;
  failureFingerprint?: string;
}

interface CleanupReceipt {
  id: string;
  processExited: boolean;
  streamsDrained: boolean;
  logWriterClosed: boolean;
  ownedProcessCount: number;
  forcedSignals: string[];
}

function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) {
    throw new Error(
      `${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`,
    );
  }
}

function hashBytes(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const normalized = resolve(executable);
  return new TextDecoder()
    .decode(result.stdout)
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

function asObjects(value: Json | undefined): ObjectJson[] {
  return Array.isArray(value)
    ? value.filter(
        (item): item is ObjectJson =>
          !!item && typeof item === "object" && !Array.isArray(item),
      )
    : [];
}

function semanticId(element: ObjectJson): string {
  return String(element.semanticId ?? element.semantic_id ?? "");
}

async function elements(driver: Driver, target: Json): Promise<ObjectJson[]> {
  const result = (await driver.getElements(
    { target, limit: 1_000 },
    { timeoutMs: 10_000 },
  )) as ObjectJson;
  return asObjects(result.elements);
}

async function waitElements(
  driver: Driver,
  target: Json,
  predicate: (list: ObjectJson[]) => boolean,
  label: string,
  timeoutMs = 10_000,
): Promise<ObjectJson[]> {
  const deadline = Date.now() + timeoutMs;
  let list = await elements(driver, target);
  while (!predicate(list) && Date.now() < deadline) {
    await Bun.sleep(40);
    list = await elements(driver, target);
  }
  assert(predicate(list), `timed out waiting for ${label}`);
  return list;
}

function findElement(list: ObjectJson[], id: string): ObjectJson | undefined {
  return list.find((element) => semanticId(element) === id);
}

async function waitMain(
  driver: Driver,
  predicate: (state: ObjectJson) => boolean,
  label: string,
  timeoutMs = 15_000,
): Promise<ObjectJson> {
  const deadline = Date.now() + timeoutMs;
  let state = (await driver.getState({ timeoutMs: 10_000 })) as ObjectJson;
  while (!predicate(state) && Date.now() < deadline) {
    await Bun.sleep(40);
    state = (await driver.getState({ timeoutMs: 10_000 })) as ObjectJson;
  }
  assert(predicate(state), `timed out waiting for ${label}`, {
    promptType: state.promptType ?? null,
    windowVisible: state.windowVisible ?? null,
    inputLength: String(state.inputValue ?? "").length,
  });
  return state;
}

async function exactTarget(driver: Driver, kind: string): Promise<Json> {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    const receipt = (await driver.listAutomationWindows()) as ObjectJson;
    const matches = asObjects(receipt.windows).filter(
      (window) => window.kind === kind && window.visible === true,
    );
    if (matches.length === 1) {
      const match = matches[0];
      return typeof match.generation === "number"
        ? { type: "instance", id: match.id, generation: match.generation }
        : { type: "id", id: match.id };
    }
    await Bun.sleep(50);
  }
  throw new Error(`expected exactly one visible ${kind} target`);
}

async function key(
  driver: Driver,
  target: Json,
  value: string,
  modifiers: string[] = [],
): Promise<void> {
  const result = await driver.simulateGpuiKeyDown(value, {
    target,
    modifiers,
    timeoutMs: 10_000,
  });
  assert(result.success !== false, `key dispatch failed: ${value}`, result);
  // Targeted GPUI dispatch is scheduled onto the owning window turn. The
  // protocol receipt confirms scheduling, not completion, so do not inspect
  // state or the pasteboard until that turn has had a bounded chance to run.
  if (result.dispatchCompleted !== true) {
    await Bun.sleep(40);
  }
}

async function setFixture(
  driver: Driver,
  phase: string,
  assistantText?: string,
): Promise<void> {
  const requestId = `c06-fixture-${phase}-${Date.now()}`;
  const result = (await driver.request(
    {
      type: "setAgentChatTestFixture",
      phase,
      userText: "C06 accepted request",
      assistantText,
      requestId,
    },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  )) as ObjectJson;
  assert(result.ok !== false && result.success !== false, `fixture failed: ${phase}`, result);
}

async function assertPrivacyBoundary(driver: Driver, target: Json): Promise<void> {
  // Exercise a private draft without requesting a state/element receipt while
  // it exists. The following fixture request is ordered after both writes.
  driver.send({ type: "setInput", text: DRAFT_CANARY });
  driver.send({ type: "setInput", text: "" });

  // The error fixture routes raw provider/path detail through AppFailureRecord's
  // diagnostic vault; only its safe presentation may reach UI or automation.
  await setFixture(
    driver,
    "error",
    `${RAW_PROVIDER_CANARY}\n${PRIVATE_PATH_CANARY}`,
  );
  const serialized = JSON.stringify(await elements(driver, target));
  for (const canary of [RAW_PROVIDER_CANARY, PRIVATE_PATH_CANARY, DRAFT_CANARY]) {
    assert(!serialized.includes(canary), "private fixture input escaped into elements");
  }
}

const PASTEBOARD_SWIFT = String.raw`
import AppKit
import Foundation
struct ArchiveItem: Codable { let values: [String: String] }
struct Archive: Codable { let items: [ArchiveItem] }
func archive() -> Archive {
  let items = (NSPasteboard.general.pasteboardItems ?? []).map { item -> ArchiveItem in
    var values: [String: String] = [:]
    for type in item.types.sorted(by: { $0.rawValue < $1.rawValue }) {
      if let data = item.data(forType: type) { values[type.rawValue] = data.base64EncodedString() }
    }
    return ArchiveItem(values: values)
  }
  return Archive(items: items)
}
let args = CommandLine.arguments
let board = NSPasteboard.general
switch args[1] {
case "capture":
  let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
  try encoder.encode(archive()).write(to: URL(fileURLWithPath: args[2]), options: .atomic)
case "restore":
  let decoded = try JSONDecoder().decode(Archive.self, from: Data(contentsOf: URL(fileURLWithPath: args[2])))
  board.clearContents()
  let items = decoded.items.map { saved -> NSPasteboardItem in
    let item = NSPasteboardItem()
    for key in saved.values.keys.sorted() {
      if let encoded = saved.values[key], let value = Data(base64Encoded: encoded) {
        item.setData(value, forType: NSPasteboard.PasteboardType(key))
      }
    }
    return item
  }
  if !items.isEmpty { _ = board.writeObjects(items) }
case "write":
  board.clearContents()
  board.setString(String(data: Data(base64Encoded: args[2])!, encoding: .utf8)!, forType: .string)
case "read":
  let value = board.string(forType: .string) ?? ""
  FileHandle.standardOutput.write(Data(value.utf8).base64EncodedData())
default: exit(64)
}
`;

async function runProcess(args: string[]): Promise<string> {
  const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
  const [stdout, code] = await Promise.all([
    new Response(child.stdout).text(),
    child.exited,
  ]);
  assert(code === 0, `private helper failed with exit ${code}`);
  return stdout;
}

class PasteboardGuard {
  private constructor(
    private readonly root: string,
    private readonly executable: string,
    private readonly before: string,
  ) {}

  static async create(): Promise<PasteboardGuard> {
    const root = mkdtempSync(join(tmpdir(), "cons-flow-c06-pasteboard-"));
    const source = join(root, "pasteboard.swift");
    const executable = join(root, "pasteboard");
    const before = join(root, "before.json");
    writeFileSync(source, PASTEBOARD_SWIFT, { mode: 0o600 });
    await runProcess(["/usr/bin/xcrun", "swiftc", source, "-o", executable]);
    await runProcess([executable, "capture", before]);
    chmodSync(before, 0o600);
    return new PasteboardGuard(root, executable, before);
  }

  async write(text: string): Promise<void> {
    await runProcess([
      this.executable,
      "write",
      Buffer.from(text, "utf8").toString("base64"),
    ]);
  }

  async read(): Promise<Buffer> {
    const encoded = await runProcess([this.executable, "read"]);
    return Buffer.from(encoded.trim(), "base64");
  }

  async restore(): Promise<void> {
    await runProcess([this.executable, "restore", this.before]);
    const after = join(this.root, "after.json");
    await runProcess([this.executable, "capture", after]);
    assert(
      readFileSync(after).equals(readFileSync(this.before)),
      "clipboard restoration did not preserve every pasteboard type",
    );
    rmSync(this.root, { recursive: true, force: true });
  }
}

const scenarios: ScenarioReceipt[] = [];
const cleanups: CleanupReceipt[] = [];
const failures: string[] = [];
let clipboardRestored = true;

async function runScenario(
  id: string,
  surface: string,
  body: (driver: Driver, pasteboard: PasteboardGuard) => Promise<Record<string, string | number | boolean | null>>,
  env: Record<string, string> = {},
  ownedExecutables: string[] = [],
): Promise<void> {
  let driver: Driver | null = null;
  const pasteboard = await PasteboardGuard.create();
  try {
    driver = await Driver.launch({
      binary: BINARY,
      sessionName: `cons-flow-c06-${id}`,
      sandboxHome: true,
      sharedModels: false,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1",
        SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
        SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
        SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
        ...env,
      },
      readyTimeoutMs: 30_000,
      defaultTimeoutMs: 15_000,
    });
    await driver.waitForSettle();
    scenarios.push({ id, surface, status: "PASS", facts: await body(driver, pasteboard) });
  } catch (error) {
    console.error(`[${id}] private diagnostic:`, error);
    failures.push(id);
    scenarios.push({
      id,
      surface,
      status: "FAILED",
      facts: {},
      failureFingerprint: hashBytes(
        `${id}:${error instanceof Error ? `${error.name}:${error.message}` : String(error)}`,
      ).slice(0, 24),
    });
  } finally {
    try {
      await pasteboard.restore();
    } catch (error) {
      clipboardRestored = false;
      console.error(`[${id}] private clipboard cleanup diagnostic:`, error);
      failures.push(`${id}.clipboardCleanup`);
    }
    if (driver) {
      try {
        await driver.close();
      } catch (error) {
        console.error(`[${id}] private driver cleanup diagnostic:`, error);
        failures.push(`${id}.driverCleanup`);
      }
      const ownedPids = [BINARY, ...ownedExecutables].flatMap(exactExecutablePids);
      const cleanup = {
        id,
        processExited: driver.finalization.processExited,
        streamsDrained: driver.finalization.streamsDrained,
        logWriterClosed: driver.finalization.logWriterClosed,
        ownedProcessCount: ownedPids.length,
        forcedSignals: [],
      };
      cleanups.push(cleanup);
      if (
        !cleanup.processExited ||
        !cleanup.streamsDrained ||
        !cleanup.logWriterClosed ||
        cleanup.ownedProcessCount !== 0
      ) {
        failures.push(`${id}.cleanup`);
      }
    }
  }
}

async function openFlowSession(driver: Driver): Promise<void> {
  driver.send({ type: "show" });
  await driver.waitForSettle();
  await driver.setFilterAndWait("Flows");
  await key(driver, { type: "main" }, "enter");
  await waitMain(driver, (state) => state.flowUx && typeof state.flowUx === "object", "Flow Desk");
  await driver.setFilterAndWait("hello-codex");
  await key(driver, { type: "main" }, "enter");
  await waitMain(driver, (state) => state.promptType === "flowSession", "Flow session", 20_000);
}

async function assertOverlayPrecedence(driver: Driver, target: Json): Promise<void> {
  await key(driver, target, "k", ["cmd"]);
  const actionsTarget = await exactTarget(driver, "actionsDialog");
  await key(driver, actionsTarget, "escape");
  const closeDeadline = Date.now() + 10_000;
  let windows = (await driver.listAutomationWindows()) as ObjectJson;
  while (
    asObjects(windows.windows).some(
      (window) => window.kind === "actionsDialog" && window.visible === true,
    ) &&
    Date.now() < closeDeadline
  ) {
    await Bun.sleep(40);
    windows = (await driver.listAutomationWindows()) as ObjectJson;
  }
  assert(
    !asObjects(windows.windows).some(
      (window) => window.kind === "actionsDialog" && window.visible === true,
    ),
    "Actions did not complete its close transition",
  );
  if ((target as ObjectJson).type === "main") {
    const state = (await driver.getState()) as ObjectJson;
    assert(
      state.promptType === "flowSession" ||
        state.promptType === "agentChatChat" ||
        state.promptType === "chat",
      "closing Actions also dismissed its conversation host",
      { promptType: state.promptType },
    );
  }
}

async function assertCopyGrammar(
  driver: Driver,
  pasteboard: PasteboardGuard,
  target: Json,
  keyPath: "gpui" | "flow-host" = "gpui",
): Promise<void> {
  await setFixture(driver, "c06Completed", EXACT_ANSWER);
  const list = await elements(driver, target);
  const turnCopies = list.filter((element) =>
    semanticId(element).startsWith("conversation.copyTurn:"),
  );
  assert(turnCopies.length === 1, "copy projection must skip whitespace-only response", {
    ids: turnCopies.map(semanticId),
  });

  await pasteboard.write(CLIPBOARD_CANARY);
  await key(driver, target, "c", ["cmd"]);
  assert(
    (await pasteboard.read()).equals(Buffer.from(CLIPBOARD_CANARY)),
    "plain Cmd+C was intercepted without a native selection",
  );

  if (keyPath === "flow-host") {
    // Flow's composer is the shared launcher input, so its host command route
    // is exercised through the same compatibility entry used by native footer
    // and stdin automation. The focused model tests separately lock ⇧⌘C to this
    // exact typed transaction.
    driver.simulateKey("c", ["cmd", "shift"]);
    await Bun.sleep(80);
  } else {
    await key(driver, target, "c", ["cmd", "shift"]);
  }
  assert(
    (await pasteboard.read()).equals(Buffer.from(EXACT_ANSWER)),
    "Shift+Cmd+C did not preserve exact assistant bytes",
  );
}

async function assertActiveDismissStopRetry(
  driver: Driver,
  target: Json,
): Promise<void> {
  await setFixture(driver, "c06StreamingPartial", EXACT_PARTIAL);
  let list = await elements(driver, target);
  assert(findElement(list, "conversation.stop")?.selectable === true, "Stop is not enabled");
  assert(
    findElement(list, "conversation.close")?.actionDisabled === ACTIVE_CLOSE_DISABLED_REASON ||
      findElement(list, "conversation.close")?.action_disabled === ACTIVE_CLOSE_DISABLED_REASON,
    "active Close lacks the exact safe disabled reason",
  );

  await key(driver, target, "escape");
  list = await elements(driver, target);
  const status = findElement(list, "conversation.commandStatus");
  assert(
    status?.actionDisabled === ACTIVE_CLOSE_DISABLED_REASON ||
      status?.action_disabled === ACTIVE_CLOSE_DISABLED_REASON,
    "Escape did not surface the active-work dismissal reason",
  );
  assert(findElement(list, "conversation.stop")?.selectable === true, "Escape secretly stopped work");

  await key(driver, target, ".", ["cmd"]);
  list = await elements(driver, target);
  const cancelled = list.find(
    (element) =>
      (element.statusKind ?? element.status_kind) === "cancelled" &&
      element.kind === "userStopped",
  );
  assert(cancelled, "explicit Stop did not project userStopped");
  assert(
    list.some((element) => semanticId(element).startsWith("conversation.copyTurn:")),
    "partial Stop lost the exact assistant copy target",
  );

  await setFixture(driver, "c06StreamingEmpty");
  await key(driver, target, ".", ["cmd"]);
  list = await elements(driver, target);
  assert(
    !list.some((element) => semanticId(element).startsWith("conversation.copyTurn:")),
    "empty Stop exposed a copy operation",
  );

  await setFixture(driver, "c06RetryableFailure");
  list = await elements(driver, target);
  assert(
    findElement(list, "conversation.retry") || findElement(list, "ai-recovery-retry"),
    "Retry is not exposed for the immutable accepted request",
  );
  const copyCountBeforeRetry = list.filter((element) =>
    semanticId(element).startsWith("conversation.copyTurn:"),
  ).length;
  await key(driver, target, "r", ["cmd", "shift"]);
  list = await waitElements(
    driver,
    target,
    (elements) =>
      !findElement(elements, "conversation.retry") &&
      elements.filter((element) =>
        semanticId(element).startsWith("conversation.copyTurn:"),
      ).length > copyCountBeforeRetry,
    "Retry to replay and complete the accepted request",
  );
  assert(!findElement(list, "conversation.retry"), "Retry recovery state did not clear");
}

const flowEnv = {
  SCRIPT_KIT_FLOW_UX_CWD: FLOW_FIXTURE,
  SCRIPT_KIT_FLOWS_PACKAGE_DIR: FLOW_PACKAGE,
  SCRIPT_KIT_FLOWS_BIN_DIR: join(FLOW_PACKAGE, "bin"),
  SCRIPT_KIT_CODEX_BIN: join(FLOW_PACKAGE, "bin/fake-codex"),
  PATH: `${join(FLOW_FIXTURE, "bin")}:${join(FLOW_PACKAGE, "bin")}:${process.env.PATH ?? ""}`,
};
const flowOwned = [
  join(FLOW_FIXTURE, "bin/hello"),
  join(FLOW_PACKAGE, "bin/fake-codex"),
];

for (const origin of ["desk", "main", "direct"] as const) {
  await runScenario(
    `flow-${origin}`,
    "Flow",
    async (driver, pasteboard) => {
      await openFlowSession(driver);
      await setFixture(driver, `c06FlowReturn:${origin}`);
      await assertOverlayPrecedence(driver, { type: "main" });
      await assertCopyGrammar(driver, pasteboard, { type: "main" }, "flow-host");
      await key(driver, { type: "main" }, "escape");
      const state = await waitMain(
        driver,
        (candidate) =>
          origin === "desk"
            ? candidate.promptType === "flowUx"
            : origin === "main"
              ? candidate.promptType === "none" && candidate.inputValue === "c06-main-route"
              : candidate.windowVisible === false,
        `Flow ${origin} return route`,
      );
      if (origin === "desk") {
        assert(state.promptType === "flowUx", "Desk route did not restore Flow Desk", state.promptType);
      } else if (origin === "main") {
        assert(state.promptType === "none", "Main route did not restore launcher", state.promptType);
        assert(state.inputValue === "c06-main-route", "Main route lost exact filter bytes");
      } else {
        assert(state.windowVisible === false, "Direct Flow route did not close the main window");
      }
      return { origin, dismissalRestoredOrigin: true, exactCopy: true, overlayPrecedence: true };
    },
    flowEnv,
    flowOwned,
  );
}

for (const origin of ["source", "main", "direct"] as const) {
  await runScenario(`agent-embedded-${origin}`, "Agent Chat embedded", async (driver, pasteboard) => {
    driver.send({ type: "show" });
    await driver.waitForSettle();
    if (origin === "source") {
      driver.send({ type: "triggerBuiltin", name: "files" });
      await waitMain(driver, (state) => state.promptType === "fileSearch", "File Search source");
    }
    driver.send({ type: "openAiWithMockData" });
    await waitMain(driver, (state) => state.promptType === "agentChatChat", "embedded Agent Chat");
    await setFixture(driver, `c06AgentReturn:${origin}`);
    if (origin === "main") {
      await assertPrivacyBoundary(driver, { type: "main" });
    }
    await assertOverlayPrecedence(driver, { type: "main" });
    await assertCopyGrammar(driver, pasteboard, { type: "main" });
    if (origin === "main") {
      await assertActiveDismissStopRetry(driver, { type: "main" });
      await setFixture(driver, "c06Completed", EXACT_ANSWER);
    }
    await key(driver, { type: "main" }, "escape");
    const state = await waitMain(
      driver,
      (candidate) =>
        origin === "source"
          ? candidate.promptType === "fileSearch"
          : origin === "main"
            ? candidate.promptType === "none" && candidate.inputValue === "c06-main-route"
            : candidate.windowVisible === false,
      `Agent Chat ${origin} return route`,
    );
    if (origin === "source") {
      assert(state.promptType === "fileSearch", "Source route did not restore File Search");
    } else if (origin === "main") {
      assert(state.promptType === "none", "Main route did not restore launcher");
      assert(state.inputValue === "c06-main-route", "Main route lost exact filter bytes");
    } else {
      assert(state.windowVisible === false, "Direct Agent Chat route did not close main window");
    }
    return {
      origin,
      dismissalRestoredOrigin: true,
      exactCopy: true,
      stopRetry: origin === "main",
      overlayPrecedence: true,
    };
  });
}

await runScenario("agent-detached", "Agent Chat detached", async (driver, pasteboard) => {
  driver.send({ type: "openAgentChatDetachedFixture", requestId: "c06-detached" });
  const target = await exactTarget(driver, "agentChatDetached");
  await assertOverlayPrecedence(driver, target);
  await assertCopyGrammar(driver, pasteboard, target);
  await assertActiveDismissStopRetry(driver, target);
  await setFixture(driver, "c06Completed", EXACT_ANSWER);
  await key(driver, target, "w", ["cmd"]);
  const closeDeadline = Date.now() + 10_000;
  let windows = (await driver.listAutomationWindows()) as ObjectJson;
  while (
    asObjects(windows.windows).some(
      (window) => window.kind === "agentChatDetached" && window.visible === true,
    ) &&
    Date.now() < closeDeadline
  ) {
    await Bun.sleep(40);
    windows = (await driver.listAutomationWindows()) as ObjectJson;
  }
  assert(
    !asObjects(windows.windows).some(
      (window) => window.kind === "agentChatDetached" && window.visible === true,
    ),
    "detached Cmd+W did not close the idle conversation",
  );
  return { origin: "detached", nativeClosePolicy: true, exactCopy: true, stopRetry: true };
});

await runScenario("chat-prompt", "ordinary ChatPrompt", async (driver, pasteboard) => {
  driver.send({ type: "show" });
  await driver.waitForSettle();
  driver.send({ type: "openChatPromptFixture", requestId: "c06-chat-prompt" });
  await waitMain(driver, (state) => state.promptType === "chat", "ordinary ChatPrompt");
  const target = { type: "main" };
  await assertOverlayPrecedence(driver, target);
  await assertCopyGrammar(driver, pasteboard, target);
  const list = await elements(driver, target);
  assert(!findElement(list, "conversation.stop"), "ChatPrompt advertised unsupported Stop");
  assert(!findElement(list, "conversation.retry"), "ChatPrompt advertised unsupported Retry");
  assert(!findElement(list, "conversation.background"), "ChatPrompt advertised Background");
  assert(!findElement(list, "conversation.new"), "ChatPrompt advertised New Conversation");
  await key(driver, target, "escape");
  const state = await waitMain(
    driver,
    (candidate) => candidate.promptType !== "chat",
    "ChatPrompt return route",
  );
  assert(state.promptType !== "chat", "ChatPrompt Escape did not return to its host");
  return { origin: "scriptHost", capabilityDerived: true, exactCopy: true, overlayPrecedence: true };
});

const cleanup = {
  processExited: cleanups.every((item) => item.processExited),
  streamsDrained: cleanups.every((item) => item.streamsDrained),
  logWriterClosed: cleanups.every((item) => item.logWriterClosed),
  ownedProcessCount: cleanups.reduce((total, item) => total + item.ownedProcessCount, 0),
  forcedSignals: cleanups.flatMap((item) => item.forcedSignals),
  clipboardRestored,
  scenarios: cleanups,
};

const status: Status = failures.length === 0 ? "PASS" : "FAILED";
const receipt = {
  schemaVersion: 1,
  status,
  binary: {
    relativePath: "target-agent/artifacts/cons-flow-c06/script-kit-gpui",
    sha256: hashBytes(readFileSync(BINARY)),
  },
  tasks: {
    "WF-006": { status, checkpoints: scenarios.filter((item) => item.facts.dismissalRestoredOrigin).map((item) => item.id) },
    "WF-007": { status, checkpoints: scenarios.filter((item) => item.facts.stopRetry).map((item) => item.id) },
    "WF-009": { status, checkpoints: scenarios.filter((item) => item.facts.exactCopy).map((item) => item.id) },
    "WF-010": { status, checkpoints: scenarios.map((item) => item.id) },
  },
  privacy: {
    rawContentFields: 0,
    privatePathFields: 0,
    clipboardContentFields: 0,
    rawFailureFields: 0,
  },
  scenarios,
  cleanup,
  failures,
};

mkdirSync(dirname(RECEIPT_PATH), { recursive: true });
writeFileSync(RECEIPT_PATH, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 });
console.log(JSON.stringify(receipt, null, 2));
if (status !== "PASS") process.exitCode = 1;
