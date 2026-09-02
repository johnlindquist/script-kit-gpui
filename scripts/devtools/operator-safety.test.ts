import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  assertNoninteractiveDriverLaunch,
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveSessionCommand,
  assertNoninteractiveSubprocess,
  assertNoninteractiveUnownedSessionCommand,
  assertNoninteractiveVisualProbe,
  inspectionSessionCleanup,
  NONINTERACTIVE_SAFE_COMMAND_TYPES,
  NoninteractiveSafetyError,
  requireSuccessfulSessionAction,
  SessionOwnershipRegistry,
} from "./lib/operator-safety.ts";
import { AttachedDriver } from "./driver.ts";

const environment = {
  SCRIPT_KIT_NONINTERACTIVE: "1",
  SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
  SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
  SCRIPT_KIT_ALLOW_LIVE_AI: "0",
  SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
  SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
};

function check(command: Record<string, unknown>): void {
  assertNoninteractiveProtocolCommand(command, {
    noninteractive: true,
    environment,
  });
}

function child(source: string, overrides: Record<string, string> = {}) {
  return Bun.spawnSync(["bun", "-e", source], {
    cwd: new URL("../..", import.meta.url).pathname,
    env: {
      ...process.env,
      ...environment,
      CI: "false",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
      ...overrides,
    },
    stdout: "pipe",
    stderr: "pipe",
  });
}

function withParentAuthority(
  overrides: Record<string, string>,
  action: () => void,
): void {
  const changes = {
    SCRIPT_KIT_NONINTERACTIVE: "1",
    SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
    SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
    SCRIPT_KIT_ALLOW_LIVE_AI: "0",
    SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
    SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
    SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
    CI: "false",
    ...overrides,
  };
  const originals = Object.fromEntries(
    Object.keys(changes).map((key) => [key, process.env[key]]),
  );
  try {
    Object.assign(process.env, changes);
    action();
  } finally {
    for (const [key, original] of Object.entries(originals)) {
      if (original === undefined) delete process.env[key];
      else process.env[key] = original;
    }
  }
}

describe("noninteractive DevTools operator safety", () => {
  const suiteAuthority = {
    ...environment,
    CI: "false",
    SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
  };
  let originalAuthority: Record<string, string | undefined> = {};

  beforeEach(() => {
    originalAuthority = Object.fromEntries(
      Object.keys(suiteAuthority).map((key) => [key, process.env[key]]),
    );
    Object.assign(process.env, suiteAuthority);
  });

  afterEach(() => {
    for (const [key, value] of Object.entries(originalAuthority)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  });

  test("only reviewed hidden-root inspection and filter commands are allowed", () => {
    const ownedOnly = new Set(["setFilter", "setInput", "hide"]);
    for (const commandType of NONINTERACTIVE_SAFE_COMMAND_TYPES) {
      if (ownedOnly.has(commandType)) {
        expect(() => check({ type: commandType })).toThrow(
          "mutating an existing operator session is forbidden",
        );
      } else {
        expect(() => check({ type: commandType })).not.toThrow();
      }
    }
    expect(() =>
      check({
        type: "batch",
        commands: [
          { type: "getState" },
          { type: "waitFor", condition: { type: "stateMatch", state: { inputValue: "quiet" } } },
        ],
      })
    ).toThrow("mutating protocol batches require an explicitly owned isolated CI sandbox");
  });

  test("only actual immutable CI authority can mutate its owned hidden driver", () => {
    for (const command of [
      { type: "setFilter", text: "quiet" },
      { type: "setInput", text: "quiet" },
      { type: "hide" },
      { type: "batch", commands: [{ type: "getState" }] },
    ]) {
      withParentAuthority({}, () => {
        expect(() => assertNoninteractiveProtocolCommand(command)).toThrow(
          "isolated CI sandbox",
        );
        expect(() =>
          assertNoninteractiveProtocolCommand(command, {
            noninteractive: true,
            environment: {
              ...environment,
              CI: "true",
              SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1",
            },
          }),
        ).toThrow("isolated CI sandbox");
      });
      withParentAuthority(
        { CI: "true", SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1" },
        () => {
          expect(() => assertNoninteractiveProtocolCommand(command)).not.toThrow();
        },
      );
    }
  });

  test("protocol options cannot disable immutable parent noninteractive authority", () => {
    for (const overrides of [
      { noninteractive: false },
      { environment: { ...environment, SCRIPT_KIT_NONINTERACTIVE: "0" } },
      {
        noninteractive: false,
        environment: { ...environment, SCRIPT_KIT_NONINTERACTIVE: "0" },
      },
    ]) {
      expect(() =>
        assertNoninteractiveProtocolCommand({ type: "captureScreenshot" }, overrides),
      ).toThrow("immutable parent safety authority");
    }
  });

  test.each([
    "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
    "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
  ])("replacement protocol environments cannot hide inherited %s authority", (unsafeSetting) => {
    withParentAuthority({ [unsafeSetting]: "1" }, () => {
      expect(() =>
        assertNoninteractiveProtocolCommand(
          { type: "getState" },
          {
            noninteractive: true,
            environment,
          },
        ),
      ).toThrow(`${unsafeSetting}=1 contradicts noninteractive execution`);
    });
  });

  test("window reveal, focus, input, pixel capture, live AI, and actions fail closed", () => {
    for (const commandType of [
      "show",
      "openNotes",
      "openActions",
      "focusWindow",
      "activateWindow",
      "simulateKey",
      "simulateGpuiEvent",
      "captureScreenshot",
      "captureWindow",
      "inspectAutomationWindow",
      "triggerAction",
      "triggerBuiltin",
      "submit",
      "startDictation",
      "sendAgentChatMessage",
      "unknownFutureMutation",
    ]) {
      expect(() => check({ type: commandType })).toThrow(
        NoninteractiveSafetyError,
      );
    }
  });

  test("capture cursor payloads never grant an unowned transport capture or screen authority", () => {
    for (const frameCursor of [{ traceGeneration: 1, afterFrameGeneration: 0 }, null,
        { traceGeneration: 1, afterFrameGeneration: 0, extra: true }]) {
      const capture = { type: "design", command: { operation: "captureFrame", target: { type: "instance", id: "main", generation: 1 }, includeImage: false, frameCursor } };
      expect(() => check(capture)).toThrow(NoninteractiveSafetyError);
      expect(() => check({ type: "batch", commands: [capture] })).toThrow(NoninteractiveSafetyError);
      for (const type of ["captureScreenshot", "captureWindow", "simulateGpuiEvent", "show"])
        expect(() => check({ type, frameCursor })).toThrow(NoninteractiveSafetyError);
    }
  });

  test("batch wrapping cannot smuggle visible or submitting commands", () => {
    for (const unsafe of [
      { type: "show" },
      { type: "openActions" },
      { type: "typeAndSubmit", text: "unsafe" },
      { type: "batch", commands: [{ type: "captureScreenshot" }] },
    ]) {
      expect(() =>
        check({
          type: "batch",
          commands: [{ type: "getState" }, unsafe],
        })
      ).toThrow("nested command 1 is unsafe");
    }
  });

  test("the outer batch envelope cannot hide silent, focused, or activating mutations", () => {
    const safeCommands = [{ type: "getState" }];
    for (const modifier of ["submit", "open", "reveal", "focus", "activate", "show"]) {
      expect(() =>
        check({ type: "batch", commands: safeCommands, [modifier]: true }),
      ).toThrow(`${modifier}=true`);
      expect(() =>
        check({ type: "batch", commands: safeCommands, options: { [modifier]: true } }),
      ).toThrow(`${modifier}=true`);
    }
    for (const hiddenReply of [
      { noResponse: true },
      { no_response: true },
      { options: { noResponse: true } },
      { options: { no_response: true } },
    ]) {
      expect(() => check({ type: "batch", commands: safeCommands, ...hiddenReply }))
        .toThrow("silent mutations are forbidden");
    }
    expect(() =>
      check({ type: "batch", commands: safeCommands, target: { type: "focused" } }),
    ).toThrow("focused-window selectors");
    expect(() =>
      check({
        type: "batch",
        commands: safeCommands,
        options: { target: { selector: { type: "focused" } } },
      }),
    ).toThrow("focused-window selectors");
  });

  test("alternate command containers cannot smuggle nested capture, AI, or focus", () => {
    for (const container of ["operations", "actions", "steps", "messages", "requests", "payload", "command"]) {
      expect(() =>
        check({
          type: "getState",
          [container]: [{ type: "captureScreenshot" }],
        }),
      ).toThrow(`alternate ${container} command container`);
      expect(() =>
        check({
          type: "batch",
          commands: [{ type: "getState" }],
          options: { [container]: { type: "openAgentChat" } },
        }),
      ).toThrow(`alternate ${container} command container`);
    }
    expect(() =>
      check({ type: "getState", commands: [{ type: "show" }] }),
    ).toThrow("only in the explicit batch.commands array");
    expect(() =>
      check({ type: "batch", commands: [], options: { commands: [{ type: "show" }] } }),
    ).toThrow("only in the explicit batch.commands array");
  });

  test("noResponse never makes an AI-revealing or silently mutating command safe", () => {
    for (const command of [
      { type: "openAi", noResponse: true },
      { type: "openAgentChat", noResponse: true },
      { type: "aiStartChat", message: "visible draft", noResponse: true },
      { type: "setAiInput", text: "visible draft", submit: false, noResponse: true },
      { type: "setInput", text: "hidden", noResponse: true },
      { type: "setInput", text: "hidden", options: { noResponse: true } },
    ]) {
      expect(() => check(command)).toThrow(NoninteractiveSafetyError);
      expect(() =>
        check({ type: "batch", commands: [command] })
      ).toThrow("nested command 0 is unsafe");
    }
  });

  test("safe command names cannot smuggle submit, reveal, focus, or activation", () => {
    for (const modifier of ["submit", "open", "reveal", "focus", "activate"]) {
      expect(() =>
        check({ type: "setInput", text: "unsafe", [modifier]: true })
      ).toThrow(`${modifier}=true`);
      expect(() =>
        check({ type: "getState", options: { [modifier]: true } })
      ).toThrow(`${modifier}=true`);
    }
  });

  test("focused selectors and visible-window waits are denied", () => {
    expect(() =>
      check({ type: "getElements", target: { type: "focused" } })
    ).toThrow("focused-window selectors");
    expect(() => check({ type: "waitFor", condition: "windowVisible" })).toThrow(
      "visible window",
    );
    expect(() =>
      check({
        type: "waitFor",
        condition: { type: "stateMatch", state: { windowVisible: true } },
      })
    ).toThrow("visible window");
    expect(() =>
      check({
        type: "waitFor",
        condition: { type: "stateMatch", state: { windowVisible: false } },
      })
    ).not.toThrow();
  });

  test("named, structured, and nested focus waits fail before transport", () => {
    for (const condition of [
      "windowFocused",
      { type: "windowFocused" },
      { type: "stateMatch", state: { windowFocused: true } },
      { type: "stateMatch", state: { isFocused: true } },
      { type: "stateMatch", state: { focused: true } },
    ]) {
      expect(() => check({ type: "waitFor", condition })).toThrow(
        "focused window",
      );
      expect(() =>
        check({
          type: "batch",
          commands: [
            { type: "getState" },
            { type: "waitFor", condition },
          ],
        })
      ).toThrow("nested command 1 is unsafe");
    }
    expect(() =>
      check({
        type: "waitFor",
        condition: {
          type: "stateMatch",
          state: { windowVisible: false, isFocused: false },
        },
      })
    ).not.toThrow();
  });

  test("contradictory takeover, visible, input, capture, or live-AI opt-ins fail closed", () => {
    for (const unsafeSetting of [
      "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
      "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
      "SCRIPT_KIT_ALLOW_LIVE_AI",
      "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
      "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
    ]) {
      expect(() =>
        assertNoninteractiveProtocolCommand(
          { type: "getState" },
          {
            noninteractive: true,
            environment: { ...environment, [unsafeSetting]: "1" },
          },
        )
      ).toThrow(`${unsafeSetting}=1 contradicts noninteractive execution`);
    }
  });

  test.each([
    "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
    "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
  ])("rejects inherited %s before any launch or existing-session transport", (unsafeSetting) => {
    withParentAuthority({ [unsafeSetting]: "1" }, () => {
      expect(() =>
        assertNoninteractiveProtocolCommand({ type: "getState" }),
      ).toThrow(`${unsafeSetting}=1 contradicts noninteractive execution`);
      expect(() =>
        assertNoninteractiveDriverLaunch({ sandboxHome: true }),
      ).toThrow(`${unsafeSetting}=1 contradicts noninteractive execution`);
      expect(() =>
        assertNoninteractiveSessionCommand([
          "bash",
          "scripts/agentic/session.sh",
          "status",
          "default",
        ]),
      ).toThrow(`${unsafeSetting}=1 contradicts noninteractive execution`);
    });
  });

  test("the shared Driver transport rejects unsafe commands before writing them", () => {
    const result = child(`
      import { ProtocolCore } from "./scripts/devtools/driver.ts";
      class Probe extends ProtocolCore {
        constructor() { super(10); }
        get alive() { return true; }
        async close() {}
        writeCommand() { throw new Error("UNSAFE_TRANSPORT_WRITE"); }
      }
      try { new Probe().send({ type: "show" }); }
      catch (error) { console.log(error.message); }
    `);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("refused show");
    expect(result.stdout.toString()).not.toContain("UNSAFE_TRANSPORT_WRITE");
  });

  test("unsandboxed driver launches fail before resolving or starting a binary", () => {
    const result = child(`
      import { Driver } from "./scripts/devtools/driver.ts";
      try {
        await Driver.launch({ binary: "/nonexistent/never-launch", sandboxHome: false });
      } catch (error) { console.log(error.message); }
    `);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("requires an isolated sandboxHome");
    expect(result.stdout.toString()).not.toContain("Binary not found");
  });

  test("a sandboxed local Driver.launch still refuses before resolving a binary", () => {
    const source = `
      import { Driver } from "./scripts/devtools/driver.ts";
      try {
        await Driver.launch({ binary: "/nonexistent/never-launch", sandboxHome: true });
      } catch (error) { console.log(error.message); }
    `;
    const noOptIn = child(source);
    expect(noOptIn.exitCode).toBe(0);
    expect(noOptIn.stdout.toString()).toContain(
      "requires SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1",
    );
    expect(noOptIn.stdout.toString()).not.toContain("Binary not found");

    const localOptIn = child(source, {
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1",
      CI: "false",
    });
    expect(localOptIn.exitCode).toBe(0);
    expect(localOptIn.stdout.toString()).toContain("requires CI=true");
    expect(localOptIn.stdout.toString()).not.toContain("Binary not found");
  });

  test("launch options cannot forge isolated CI authority or disable child safety", () => {
    const authorityKeys = [
      "CI",
      "SCRIPT_KIT_NONINTERACTIVE",
      "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
      "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
      "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
      "SCRIPT_KIT_ALLOW_LIVE_AI",
      "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
      "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
    ] as const;
    const originals = Object.fromEntries(
      authorityKeys.map((key) => [key, process.env[key]]),
    );
    try {
      process.env.SCRIPT_KIT_NONINTERACTIVE = "1";
      process.env.SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH = "0";
      process.env.SCRIPT_KIT_ALLOW_VISIBLE_PROBES = "0";
      process.env.SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER = "0";
      process.env.SCRIPT_KIT_ALLOW_LIVE_AI = "0";
      process.env.SCRIPT_KIT_ALLOW_NATIVE_INPUT = "0";
      process.env.SCRIPT_KIT_ALLOW_SCREEN_CAPTURE = "0";
      process.env.CI = "false";

      expect(() =>
        assertNoninteractiveDriverLaunch({
          sandboxHome: true,
          env: { CI: "true", SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1" },
        }),
      ).toThrow("CI cannot override immutable parent safety authority");
      expect(() =>
        assertNoninteractiveDriverLaunch({
          sandboxHome: true,
          env: { SCRIPT_KIT_NONINTERACTIVE: "0" },
        }),
      ).toThrow("SCRIPT_KIT_NONINTERACTIVE cannot override immutable parent safety authority");
      expect(() =>
        assertNoninteractiveDriverLaunch({
          sandboxHome: true,
          env: { SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1" },
        }),
      ).toThrow("SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH cannot override immutable parent safety authority");
      for (const unsafeSetting of [
        "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
        "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
      ]) {
        expect(() =>
          assertNoninteractiveDriverLaunch({
            sandboxHome: true,
            env: { [unsafeSetting]: "1" },
          }),
        ).toThrow(`${unsafeSetting}=1 contradicts noninteractive execution`);
      }

      process.env.CI = "true";
      process.env.SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH = "1";
      expect(() =>
        assertNoninteractiveDriverLaunch({
          sandboxHome: true,
          env: { SCRIPT_KIT_STARTUP_PROFILE: "dev-fast" },
        }),
      ).not.toThrow();
    } finally {
      for (const key of authorityKeys) {
        const value = originals[key];
        if (value === undefined) delete process.env[key];
        else process.env[key] = value;
      }
    }
  });

  test("only the exact reviewed session transport can spawn in strict mode", () => {
    for (const command of [
      ["open", "-a", "Script Kit"],
      ["osascript", "-e", "activate application"],
      ["bash", "-c", "scripts/agentic/session.sh start hidden"],
      ["bun", "scripts/devtools/fake-capture.ts"],
      ["/tmp/fake-app", "--background"],
      ["bash", "/tmp/evil/scripts/agentic/session.sh", "status", "default"],
    ]) {
      expect(() => assertNoninteractiveSessionCommand(command))
        .toThrow("only the reviewed bash scripts/agentic/session.sh");
    }
    expect(() =>
      assertNoninteractiveSessionCommand([
        "bash",
        "scripts/agentic/session.sh",
        "status",
        "default",
      ]),
    ).not.toThrow();
    expect(() =>
      assertNoninteractiveSessionCommand([
        "bash",
        "scripts/agentic/session.sh",
        "rpc",
        "safe-session.1",
        JSON.stringify({ type: "getState", target: { type: "main" } }),
        "--expect",
        "stateResult",
        "--timeout",
        "1000",
      ]),
    ).not.toThrow();
    expect(() =>
      assertNoninteractiveSessionCommand([
        "bash",
        "scripts/agentic/session.sh",
        "send",
        "safe-session",
        JSON.stringify({ type: "getState" }),
        "--await-parse",
        "--timeout",
        "1000",
      ]),
    ).not.toThrow();
  });

  test("existing session RPC/send cannot mutate or hide an operator-owned window", () => {
    for (const operation of ["rpc", "send"]) {
      for (const command of [
        { type: "setFilter", text: "operator text" },
        { type: "setInput", text: "operator text" },
        { type: "hide" },
        { type: "batch", commands: [{ type: "getState" }] },
      ]) {
        expect(() =>
          assertNoninteractiveSessionCommand([
            "bash",
            "scripts/agentic/session.sh",
            operation,
            "default",
            JSON.stringify(command),
          ]),
        ).toThrow("unowned existing session permits only side-effect-free read-only inspection");
      }
    }
  });

  test("even real CI cannot mutate a session or FIFO it did not launch and own", () => {
    withParentAuthority(
      { CI: "true", SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1" },
      () => {
        for (const command of [
          { type: "setFilter", text: "operator text" },
          { type: "setInput", text: "operator text" },
          { type: "hide" },
          { type: "batch", commands: [{ type: "getState" }] },
        ]) {
          expect(() =>
            assertNoninteractiveUnownedSessionCommand(command, "AttachedDriver"),
          ).toThrow("unowned existing session permits only side-effect-free read-only inspection");
          expect(() =>
            assertNoninteractiveSessionCommand([
              "bash",
              "scripts/agentic/session.sh",
              "rpc",
              "default",
              JSON.stringify(command),
            ]),
          ).toThrow("unowned existing session permits only side-effect-free read-only inspection");
        }
        expect(() =>
          assertNoninteractiveUnownedSessionCommand(
            { type: "getState", target: { type: "main" } },
            "AttachedDriver",
          ),
        ).not.toThrow();
      },
    );
  });

  test("attached-driver FIFO refuses mutation before touching an existing session", () => {
    withParentAuthority(
      { CI: "true", SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1" },
      () => {
        const unattachedFixture = Object.create(
          AttachedDriver.prototype,
        ) as AttachedDriver;
        for (const command of [
          { type: "setFilter", text: "operator text" },
          { type: "setInput", text: "operator text" },
          { type: "hide" },
          { type: "batch", commands: [{ type: "getState" }] },
        ]) {
          expect(() => unattachedFixture.send(command)).toThrow(
            "unowned existing session permits only side-effect-free read-only inspection",
          );
        }
      },
    );
  });

  test("session envelopes reject command injection and unreviewed trailing arguments", () => {
    expect(() =>
      assertNoninteractiveSessionCommand([
        "bash",
        "scripts/agentic/session.sh",
        "rpc",
        "../user-session",
        JSON.stringify({ type: "getState" }),
      ]),
    ).toThrow("explicit reviewed identifier");
    expect(() =>
      assertNoninteractiveSessionCommand([
        "bash",
        "scripts/agentic/session.sh",
        "status",
        "default",
        "--show",
      ]),
    ).toThrow("unreviewed trailing arguments");
    expect(() =>
      assertNoninteractiveSessionCommand([
        "bash",
        "scripts/agentic/session.sh",
        "rpc",
        "default",
        JSON.stringify({ type: "getState" }),
        "--show",
      ]),
    ).toThrow("unreviewed session transport argument: --show");
    expect(() =>
      assertNoninteractiveSessionCommand([
        "bash",
        "scripts/agentic/session.sh",
        "rpc",
        "default",
        JSON.stringify({ type: "getState" }),
        "--timeout",
        "not-a-timeout",
      ]),
    ).toThrow("--timeout requires a valid explicit value");
  });

  test("legacy session.sh RPC/send cannot bypass the shared transport policy", () => {
    const result = child(`
      import { run } from "./scripts/devtools/lib/client.ts";
      try {
        await run([
          "bash", "scripts/agentic/session.sh", "send", "default",
          JSON.stringify({ type: "captureScreenshot" }),
        ], "unsafe-test");
      } catch (error) { console.log(error.message); }
    `);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("refused captureScreenshot");
  });

  test.each([
    ["start", ["start", "isolated-safety-probe"], "session lifecycle mutation is forbidden"],
    ["stop", ["stop", "isolated-safety-probe"], "session lifecycle mutation is forbidden"],
    [
      "send",
      ["send", "isolated-safety-probe", JSON.stringify({ type: "captureScreenshot" })],
      "refused captureScreenshot",
    ],
    [
      "rpc",
      ["rpc", "isolated-safety-probe", JSON.stringify({ type: "hide" })],
      "unowned existing session permits only side-effect-free read-only inspection",
    ],
  ])("direct session.sh %s refuses before creating a session or resolving a binary", (_name, args, reason) => {
    const temporaryRoot = mkdtempSync(join(tmpdir(), "script-kit-session-guard-"));
    const sessionRoot = join(temporaryRoot, "must-remain-absent");
    try {
      const result = Bun.spawnSync(["bash", "scripts/agentic/session.sh", ...args], {
        cwd: new URL("../..", import.meta.url).pathname,
        env: {
          ...process.env,
          ...environment,
          CI: "false",
          SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
          SCRIPT_KIT_SESSION_DIR: sessionRoot,
          SCRIPT_KIT_GPUI_BINARY: join(temporaryRoot, "never-resolve-or-launch"),
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      expect(result.exitCode).toBe(78);
      expect(result.stderr.toString()).toContain(reason);
      expect(existsSync(sessionRoot)).toBe(false);
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true });
    }
  });

  test.each([
    "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
    "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
  ])("direct read-only session status rejects inherited %s before filesystem mutation", (unsafeSetting) => {
    const temporaryRoot = mkdtempSync(join(tmpdir(), "script-kit-session-opt-in-"));
    const sessionRoot = join(temporaryRoot, "must-remain-absent");
    try {
      const result = Bun.spawnSync([
        "bash",
        "scripts/agentic/session.sh",
        "status",
        "isolated-safety-probe",
      ], {
        cwd: new URL("../..", import.meta.url).pathname,
        env: {
          ...process.env,
          ...environment,
          [unsafeSetting]: "1",
          SCRIPT_KIT_SESSION_DIR: sessionRoot,
          SCRIPT_KIT_GPUI_BINARY: join(temporaryRoot, "never-resolve-or-launch"),
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      expect(result.exitCode).toBe(78);
      expect(result.stderr.toString()).toContain(
        `${unsafeSetting}=1 contradicts noninteractive execution`,
      );
      expect(existsSync(sessionRoot)).toBe(false);
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true });
    }
  });

  test("direct session status preserves reviewed noninteractive read-only inspection", () => {
    const temporaryRoot = mkdtempSync(join(tmpdir(), "script-kit-session-status-"));
    const sessionRoot = join(temporaryRoot, "sessions");
    try {
      const result = Bun.spawnSync([
        "bash",
        "scripts/agentic/session.sh",
        "status",
        "isolated-safety-probe",
      ], {
        cwd: new URL("../..", import.meta.url).pathname,
        env: {
          ...process.env,
          ...environment,
          SCRIPT_KIT_SESSION_DIR: sessionRoot,
          SCRIPT_KIT_GPUI_BINARY: join(temporaryRoot, "never-resolve-or-launch"),
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      expect(result.exitCode).toBe(0);
      expect(JSON.parse(result.stdout.toString())).toMatchObject({
        status: "not_found",
        session: "isolated-safety-probe",
        alive: false,
      });
      expect(existsSync(sessionRoot)).toBe(false);
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true });
    }
  });

  test.each([
    ["send", ["send", "isolated-safety-probe", JSON.stringify({ type: "getState" })]],
    [
      "rpc",
      [
        "rpc",
        "isolated-safety-probe",
        JSON.stringify({ type: "getState", requestId: "read-only-session-probe" }),
      ],
    ],
  ])("missing read-only session %s does not create a registry or resolve an app binary", (_name, args) => {
    const temporaryRoot = mkdtempSync(join(tmpdir(), "script-kit-session-readonly-"));
    const sessionRoot = join(temporaryRoot, "must-remain-absent");
    try {
      const result = Bun.spawnSync(["bash", "scripts/agentic/session.sh", ...args], {
        cwd: new URL("../..", import.meta.url).pathname,
        env: {
          ...process.env,
          ...environment,
          SCRIPT_KIT_SESSION_DIR: sessionRoot,
          SCRIPT_KIT_GPUI_BINARY: join(temporaryRoot, "never-resolve-or-launch"),
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      expect(result.exitCode).toBe(1);
      expect(JSON.parse(result.stdout.toString())).toMatchObject({
        status: "error",
        error: { code: "no_session" },
      });
      expect(existsSync(sessionRoot)).toBe(false);
    } finally {
      rmSync(temporaryRoot, { recursive: true, force: true });
    }
  });

  test("noninteractive status announcements cannot spawn AppleScript notifications", () => {
    const result = child(`
      import { announceTestStatus } from "./scripts/devtools/test-status.ts";
      Bun.spawn = (() => {
        throw new Error("unsafe operator notification attempted");
      });
      await announceTestStatus("read-only verification");
      console.log("operator-notification-suppressed");
    `, { SCRIPT_KIT_TEST_STATUS: "1" });

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("operator-notification-suppressed");
    expect(result.stderr.toString()).not.toContain("unsafe operator notification attempted");
  });

  test("legacy --start and stop cannot create or disturb a user-owned GUI session", () => {
    for (const operation of ["start", "stop", "futureLifecycleMutation"]) {
      const result = child(`
        import { run } from "./scripts/devtools/lib/client.ts";
        try {
          await run([
            "bash", "scripts/agentic/session.sh", ${JSON.stringify(operation)}, "default",
          ], "unsafe-session-lifecycle");
        } catch (error) { console.log(error.message); }
      `);
      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain(`refused session.${operation}`);
      expect(result.stdout.toString()).toContain("session lifecycle mutation is forbidden");
    }
  });

  test("passive inspector sessions never acquire cleanup authority", () => {
    expect(inspectionSessionCleanup("dev-watch", null)).toEqual({
      required: false,
      createdSession: false,
      command: null,
    });
  });

  test("resumed inspector sessions remain borrowed even when startup was requested", () => {
    expect(inspectionSessionCleanup("reviewed-session", {
      status: "ok",
      session: "reviewed-session",
      pid: 4242,
      resumed: true,
      ready: true,
    })).toEqual({
      required: false,
      createdSession: false,
      command: null,
    });
  });

  test("a newly created inspector session gets only exact PID-and-generation cleanup", () => {
    expect(inspectionSessionCleanup("reviewed-session", {
      status: "ok",
      session: "reviewed-session",
      pid: 4242,
      sessionGeneration: "reviewed-generation",
      resumed: false,
      ready: true,
    })).toEqual({
      required: true,
      createdSession: true,
      ownership: { pid: 4242, generation: "reviewed-generation" },
      command: "scripts/agentic/session.sh stop reviewed-session --expected-pid 4242 --expected-generation reviewed-generation",
    });
  });

  test.each([
    ["failed-start", { status: "error", error: "start_failed" }],
    ["wrong-session", { status: "ok", session: "other-session", resumed: false, ready: true, pid: 4242, sessionGeneration: "generation" }],
    ["not-ready", { status: "ok", session: "reviewed-session", resumed: false, ready: false, pid: 4242, sessionGeneration: "generation" }],
    ["missing-resume-fact", { status: "ok", session: "reviewed-session", ready: true, pid: 4242, sessionGeneration: "generation" }],
    ["missing-pid", { status: "ok", session: "reviewed-session", resumed: false, ready: true, sessionGeneration: "generation" }],
    ["zero-pid", { status: "ok", session: "reviewed-session", resumed: false, ready: true, pid: 0, sessionGeneration: "generation" }],
    ["fractional-pid", { status: "ok", session: "reviewed-session", resumed: false, ready: true, pid: 4.2, sessionGeneration: "generation" }],
    ["missing-generation", { status: "ok", session: "reviewed-session", resumed: false, ready: true, pid: 4242 }],
    ["traversal-generation", { status: "ok", session: "reviewed-session", resumed: false, ready: true, pid: 4242, sessionGeneration: "../generation" }],
    ["shell-generation", { status: "ok", session: "reviewed-session", resumed: false, ready: true, pid: 4242, sessionGeneration: "generation;stop" }],
  ])("inspector rejects unowned startup fact %s without constructing a stop command", (_name, receipt) => {
    expect(() => inspectionSessionCleanup("reviewed-session", receipt!)).toThrow();
  });

  test("the live dev-watch session can never become inspector-owned", () => {
    expect(() => inspectionSessionCleanup("dev-watch", {
      status: "ok",
      session: "dev-watch",
      pid: 4242,
      sessionGeneration: "fake-generation",
      resumed: false,
      ready: true,
    })).toThrow("borrowed operator session");
  });

  test.each(["..", "../dev-watch", "unsafe name", "-option"])(
    "inspector rejects unsafe session identity %s without creating a shell command",
    (session) => {
      expect(() => inspectionSessionCleanup(session!, null)).toThrow("safe session identity");
    },
  );

  test.each([
    ["failed start", "start", { status: "error", error: { code: "readiness_timeout" } }, "readiness_timeout"],
    ["failed parsed start", "start", { status: "error", parsedError: { error: { code: "start_failed" } } }, "start_failed"],
    ["unready start", "start", { status: "ok", session: "reviewed-session", ready: false, resumed: false }, "not ready"],
    ["missing readiness", "start", { status: "ok", session: "reviewed-session", resumed: false }, "not ready"],
    ["foreign start", "start", { status: "ok", session: "other-session", ready: true, resumed: false }, "identity mismatch"],
    ["unknown ownership", "start", { status: "ok", session: "reviewed-session", ready: true }, "ownership is unknown"],
    ["failed show", "show", { status: "error", error: { code: "send_failed" } }, "send_failed"],
    ["foreign show", "show", { status: "ok", session: "other-session" }, "identity mismatch"],
  ])("shared session lifecycle rejects %s before any follow-up", (_name, action, receipt, detail) => {
    expect(() => requireSuccessfulSessionAction(
      "reviewed-session",
      action as "start" | "show",
      receipt as Record<string, unknown>,
    )).toThrow(detail as string);
  });

  test("shared session lifecycle retains valid exact resumed and show receipts", () => {
    const started = {
      status: "ok",
      session: "reviewed-session",
      ready: true,
      resumed: true,
    };
    const shown = { status: "ok", session: "reviewed-session" };
    expect(requireSuccessfulSessionAction("reviewed-session", "start", started)).toBe(started);
    expect(requireSuccessfulSessionAction("reviewed-session", "show", shown)).toBe(shown);
    expect(() => requireSuccessfulSessionAction("../dev-watch", "start", started))
      .toThrow("safe session identity");
  });

  test.each([
    ["AppleScript", ["osascript", "-e", "display notification unsafe"]],
    ["Swift", ["swift", "scripts/agentic/macos-window-query.swift"]],
    ["native input", ["bun", "scripts/agentic/macos-input.ts", "key", "enter"]],
    ["screen capture", ["screencapture", "-x", "/tmp/unsafe.png"]],
    ["shell mutation", ["bash", "-lc", "open -a System Settings"]],
  ])("generic subprocess guard refuses %s before any child can start", (_name, command) => {
    expect(() => assertNoninteractiveSubprocess(command!)).toThrow(
      "only the reviewed bash scripts/agentic/session.sh",
    );
  });

  test("visible native probes fail closed while explicit interactive mode remains available", () => {
    expect(() => assertNoninteractiveVisualProbe("reviewed-native-probe"))
      .toThrow("visible windows, native pointer or keyboard input, screen capture");
    withParentAuthority({ SCRIPT_KIT_NONINTERACTIVE: "0" }, () => {
      expect(() => assertNoninteractiveVisualProbe("reviewed-native-probe"))
        .not.toThrow();
    });
  });

  test("generic subprocess guard preserves reviewed read-only session transport", () => {
    expect(() => assertNoninteractiveSubprocess([
      "bash", "scripts/agentic/session.sh", "status", "reviewed-session",
    ])).not.toThrow();
    expect(() => assertNoninteractiveSubprocess([
      "bash", "scripts/agentic/session.sh", "rpc", "reviewed-session",
      JSON.stringify({ type: "getState" }), "--expect", "stateResult", "--timeout", "1000",
    ])).not.toThrow();
  });

  test.each([
    ["SCRIPT_KIT_NONINTERACTIVE", "0"],
    ["SCRIPT_KIT_ALLOW_NATIVE_INPUT", "1"],
    ["SCRIPT_KIT_ALLOW_SCREEN_CAPTURE", "1"],
    ["SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH", "1"],
    ["CI", "true"],
  ])("generic subprocess guard rejects child authority override %s", (key, value) => {
    expect(() => assertNoninteractiveSubprocess(
      ["bash", "scripts/agentic/session.sh", "status", "reviewed-session"],
      { [key!]: value! },
    )).toThrow("immutable parent safety authority");
  });

  test("the shared ownership registry emits only exact owned stop commands", () => {
    const ownership = new SessionOwnershipRegistry();
    const receipt = {
      status: "ok",
      session: "reviewed-session",
      pid: 4242,
      sessionGeneration: "reviewed-generation",
      resumed: false,
      ready: true,
    };

    expect(ownership.rememberStart("reviewed-session", receipt).createdSession).toBe(true);
    expect(ownership.stopCommand("reviewed-session")).toEqual([
      "bash", "scripts/agentic/session.sh", "stop", "reviewed-session",
      "--expected-pid", "4242", "--expected-generation", "reviewed-generation",
    ]);
    ownership.release("reviewed-session");
    expect(() => ownership.stopCommand("reviewed-session")).toThrow("not owned");
  });

  test("the shared ownership registry never claims a resumed session", () => {
    const ownership = new SessionOwnershipRegistry();
    expect(ownership.rememberStart("reviewed-session", {
      status: "ok",
      session: "reviewed-session",
      pid: 4242,
      resumed: true,
      ready: true,
    }).createdSession).toBe(false);
    expect(() => ownership.stopCommand("reviewed-session")).toThrow("not owned");
  });

  test("pending startup can retain cleanup identity without masquerading as ready", () => {
    const ownership = new SessionOwnershipRegistry();
    const pending = {
      status: "ok",
      session: "reviewed-session",
      pid: 4242,
      sessionGeneration: "pending-generation",
      resumed: false,
      ready: false,
    };

    expect(() => ownership.rememberStart("reviewed-session", pending)).toThrow("unready");
    expect(ownership.rememberStart("reviewed-session", pending, {
      allowPendingReadiness: true,
    }).createdSession).toBe(true);
    expect(ownership.stopCommand("reviewed-session")).toContain("pending-generation");
  });

  test.each([
    "filterable-surface-matrix",
    "target-thread",
    "scenario",
    "automation-window",
    "surface-navigator",
  ])("%s subprocess transport refuses native commands before Bun.spawn", (moduleName) => {
    const result = child(`
      import { runTool } from "./scripts/agentic/${moduleName}.ts";
      Bun.spawn = (() => { throw new Error("unsafe native subprocess started"); });
      try {
        await runTool(["osascript", "-e", "display notification unsafe"], "unsafe-native");
      } catch (error) { console.log(error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("only the reviewed bash scripts/agentic/session.sh");
    expect(result.stdout.toString()).not.toContain("unsafe native subprocess started");
  });

  test.each([
    ["key", ["key", "Enter"]],
    ["type", ["type", "unsafe typing"]],
    ["click", ["click", "10", "20"]],
  ])("direct macOS %s helper refuses before any real input subprocess", (_kind, args) => {
    const result = child(`
      import { main } from "./scripts/agentic/macos-input.ts";
      Bun.spawn = (() => { throw new Error("unsafe macOS input was delivered"); });
      const status = await main(${JSON.stringify(args)});
      console.log("exit=" + status);
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("NONINTERACTIVE_SAFETY_REFUSED");
    expect(result.stdout.toString()).toContain("exit=1");
    expect(result.stdout.toString()).not.toContain("unsafe macOS input was delivered");
  });

  test("direct macOS input capability check remains passive and available", () => {
    const result = child(`
      import { main } from "./scripts/agentic/macos-input.ts";
      Bun.spawn = (() => { throw new Error("passive capability check spawned a process"); });
      const status = await main(["check"]);
      console.log("exit=" + status);
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain('"status": "ok"');
    expect(result.stdout.toString()).toContain("exit=0");
    expect(result.stdout.toString()).not.toContain("passive capability check spawned");
  });

  test.each([
    ["focus", ["focus"]],
    ["capture", ["capture", "/tmp/script-kit-never-capture.png"]],
    ["status", ["status"]],
  ])("direct window %s refuses before AppleScript, Swift, or screen capture", (_kind, args) => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("unsafe native window subprocess started"); });
      process.argv = ["bun", "scripts/agentic/window.ts", ...${JSON.stringify(args)}];
      await import("./scripts/agentic/window.ts");
    `);

    expect(result.exitCode).toBe(1);
    expect(result.stdout.toString()).toContain("NONINTERACTIVE_SAFETY_REFUSED");
    expect(result.stdout.toString()).not.toContain("unsafe native window subprocess started");
  });

  test.each([
    ["default OS capture", ["--skip-state", "--skip-probe", "--out", "/tmp/script-kit-never-capture.png"]],
    ["render capture", ["--skip-state", "--skip-probe", "--visual-source", "render", "--target-json", '{"type":"id","id":"fake"}', "--out", "/tmp/script-kit-never-render.png"]],
    ["automatic fallback", ["--skip-state", "--skip-probe", "--visual-source", "auto", "--out", "/tmp/script-kit-never-fallback.png"]],
  ])("verify-shot refuses %s before session show, MCP, or screenshot spawn", (_kind, args) => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("unsafe screenshot subprocess started"); });
      globalThis.fetch = (() => { throw new Error("unsafe screenshot MCP network contacted"); });
      process.argv = ["bun", "scripts/agentic/verify-shot.ts", ...${JSON.stringify(args)}];
      await import("./scripts/agentic/verify-shot.ts");
    `);

    expect(result.exitCode).toBe(2);
    expect(result.stdout.toString()).toContain("NONINTERACTIVE_SAFETY_REFUSED");
    expect(result.stdout.toString()).not.toContain("unsafe screenshot subprocess started");
    expect(result.stdout.toString()).not.toContain("unsafe screenshot MCP network contacted");
  });

  test("verify-shot skip-screenshot remains a genuinely passive receipt", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("passive screenshot proof started a subprocess"); });
      globalThis.fetch = (() => { throw new Error("passive screenshot proof contacted MCP"); });
      process.argv = ["bun", "scripts/agentic/verify-shot.ts", "--skip-screenshot", "--skip-state", "--skip-probe", "--out", "/tmp/script-kit-never-write.png"];
      await import("./scripts/agentic/verify-shot.ts");
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain('"status": "pass"');
    expect(result.stdout.toString()).toContain('"screenshot": null');
    expect(result.stdout.toString()).not.toContain("passive screenshot proof");
  });

  test.each([
    [
      "shared target resolver",
      'const { maybeStartAndShow } = await import("./scripts/devtools/lib/target-identity.ts"); await maybeStartAndShow({ session: "reviewed-session", start: true, show: true, timeoutMs: 100 });',
    ],
    [
      "Actions",
      'const { maybeStartSession } = await import("./scripts/devtools/actions.ts"); await maybeStartSession({ session: "reviewed-session", start: true, keepOpen: false });',
    ],
    [
      "Main window",
      'process.argv = ["bun", "scripts/devtools/main.ts", "inspect", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/main.ts");',
    ],
    [
      "Dictation",
      'process.argv = ["bun", "scripts/devtools/dictation.ts", "inspect", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/dictation.ts");',
    ],
    [
      "Agent Chat",
      'process.argv = ["bun", "scripts/devtools/agent_chat.ts", "open-detached-placeholder", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/agent_chat.ts");',
    ],
    [
      "Events",
      'process.argv = ["bun", "scripts/devtools/events.ts", "tail", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/events.ts");',
    ],
  ])("%s stops before follow-up work after session startup fails", (_owner, invocation) => {
    const result = child(`
      process.env.SCRIPT_KIT_NONINTERACTIVE = "0";
      let calls = 0;
      Bun.spawn = ((command) => {
        calls += 1;
        if (calls > 1) throw new Error("unsafe follow-up after failed session startup");
        return {
          stdout: new Response(JSON.stringify({ status: "error", error: { code: "readiness_timeout" } })).body,
          stderr: new Response("").body,
          exited: Promise.resolve(1),
        };
      });
      try { ${invocation} }
      catch (error) { console.log("failure=" + error.message); }
      console.log("calls=" + calls);
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("failure=DevTools session start failed");
    expect(result.stdout.toString()).toContain("calls=1");
    expect(result.stdout.toString()).not.toContain("unsafe follow-up");
  });

  test.each([
    [
      "shared target resolver",
      'const { maybeStartAndShow } = await import("./scripts/devtools/lib/target-identity.ts"); await maybeStartAndShow({ session: "reviewed-session", start: true, show: true, timeoutMs: 100 });',
    ],
    [
      "Main window",
      'process.argv = ["bun", "scripts/devtools/main.ts", "inspect", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/main.ts");',
    ],
    [
      "Dictation",
      'process.argv = ["bun", "scripts/devtools/dictation.ts", "inspect", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/dictation.ts");',
    ],
    [
      "Agent Chat",
      'process.argv = ["bun", "scripts/devtools/agent_chat.ts", "open-detached-placeholder", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/agent_chat.ts");',
    ],
    [
      "Events",
      'process.argv = ["bun", "scripts/devtools/events.ts", "tail", "--session", "reviewed-session", "--start", "--show"]; await import("./scripts/devtools/events.ts");',
    ],
  ])("%s stops before follow-up work after session show fails", (_owner, invocation) => {
    const result = child(`
      process.env.SCRIPT_KIT_NONINTERACTIVE = "0";
      let calls = 0;
      Bun.spawn = ((command) => {
        calls += 1;
        if (calls > 2) throw new Error("unsafe follow-up after failed session show");
        const starting = command[2] === "start";
        const payload = starting
          ? { status: "ok", session: "reviewed-session", ready: true, resumed: true }
          : { status: "error", error: { code: "send_failed" } };
        return {
          stdout: new Response(JSON.stringify(payload)).body,
          stderr: new Response("").body,
          exited: Promise.resolve(starting ? 0 : 1),
        };
      });
      try { ${invocation} }
      catch (error) { console.log("failure=" + error.message); }
      console.log("calls=" + calls);
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("failure=DevTools session show failed");
    expect(result.stdout.toString()).toContain("calls=2");
    expect(result.stdout.toString()).not.toContain("unsafe follow-up");
  });

  test.each([
    ["SCRIPT_KIT_NONINTERACTIVE", "0"],
    ["SCRIPT_KIT_ALLOW_NATIVE_INPUT", "1"],
    ["SCRIPT_KIT_ALLOW_SCREEN_CAPTURE", "1"],
    ["CI", "true"],
  ])("Actions transport rejects child authority override %s before spawn", (key, value) => {
    const result = child(`
      const { runActionsSubprocess } = await import("./scripts/devtools/actions.ts");
      Bun.spawn = (() => { throw new Error("unsafe Actions subprocess started"); });
      try {
        await runActionsSubprocess(
          ["bash", "scripts/agentic/session.sh", "status", "reviewed-session"],
          "session-status",
          { ${JSON.stringify(key)}: ${JSON.stringify(value)} },
        );
      } catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("immutable parent safety authority");
    expect(result.stdout.toString()).not.toContain("unsafe Actions subprocess started");
  });

  test.each([
    ["Notes live resize", "notes-live-resize.ts"],
    ["Notes bottom resize", "notes-bottom-resize.ts"],
    ["Notes glass fallback", "notes-glass-entry-fallback.ts"],
    ["Actions entry filmstrip", "actions-entry-filmstrip.ts"],
    ["glass lifecycle filmstrip", "glass-lifecycle-filmstrip.ts"],
    ["rapid toggle stress", "rapid-toggle-stress.ts"],
  ])("%s refuses before native compilation, screen takeover, or output mutation", (_owner, file) => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-visual-probe-safety-"));
    const output = join(root, "uncreated-output");
    try {
      const result = child(`
        import { existsSync } from "node:fs";
        Bun.spawn = (() => { throw new Error("unsafe visual-probe subprocess started"); });
        Bun.spawnSync = (() => { throw new Error("unsafe visual-probe native compiler started"); });
        process.argv = ["bun", ${JSON.stringify(`scripts/devtools/${file}`)}, "--binary", process.execPath, "--out", ${JSON.stringify(output)}];
        try { await import(${JSON.stringify(`./scripts/devtools/${file}`)}); }
        catch (error) { console.log("failure=" + error.message); }
        console.log("outExists=" + existsSync(${JSON.stringify(output)}));
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
      expect(result.stdout.toString()).toContain("outExists=false");
      expect(result.stdout.toString()).not.toContain("unsafe visual-probe");
      expect(existsSync(output)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test.each([
    ["glass observer verification", "glass-observers.ts", "verifyCommand"],
    ["Spotlight live filmstrip", "spotlight-sync-filmstrip.ts", "main"],
    ["main-window native drag", "main-window-native-drag.ts", "cli"],
  ])("%s refuses before native capture while pure imports stay available", (_owner, file, entrypoint) => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-visible-entry-safety-"));
    const output = join(root, "uncreated-output");
    try {
      const result = child(`
        import { existsSync } from "node:fs";
        Bun.spawn = (() => { throw new Error("unsafe visible entry subprocess started"); });
        Bun.spawnSync = (() => { throw new Error("unsafe visible entry compiler started"); });
        process.argv = ["bun", ${JSON.stringify(`scripts/devtools/${file}`)}, "verify", "--binary", process.execPath, "--out", ${JSON.stringify(output)}];
        const entry = await import(${JSON.stringify(`./scripts/devtools/${file}`)});
        try { await entry[${JSON.stringify(entrypoint)}](); }
        catch (error) { console.log("failure=" + error.message); }
        console.log("outExists=" + existsSync(${JSON.stringify(output)}));
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
      expect(result.stdout.toString()).toContain("outExists=false");
      expect(result.stdout.toString()).not.toContain("unsafe visible entry");
      expect(existsSync(output)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("Flow multiline proof refuses before reading the operator clipboard or creating output", () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-clipboard-proof-safety-"));
    const output = join(root, ".test-output", "ai-rock-solid-ux");
    const probe = join(new URL("../..", import.meta.url).pathname, "scripts/agentic/flow-composer-multiline-probe.ts");
    try {
      const result = child(`
        import { existsSync } from "node:fs";
        process.chdir(${JSON.stringify(root)});
        Bun.spawn = ((command) => { throw new Error("operator clipboard accessed: " + command[0]); });
        try { await import(${JSON.stringify(probe)}); }
        catch (error) { console.log("failure=" + error.message); }
        console.log("outExists=" + existsSync(${JSON.stringify(output)}));
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
      expect(result.stdout.toString()).not.toContain("operator clipboard accessed");
      expect(result.stdout.toString()).toContain("outExists=false");
      expect(existsSync(output)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("Dictation History proof refuses before touching the operator pasteboard", () => {
    const result = child(`
      import * as filesystem from "node:fs";
      import { mock } from "bun:test";
      mock.module("node:fs", () => ({ ...filesystem, mkdirSync: (() => {}) }));
      Bun.spawnSync = ((command) => { throw new Error("operator clipboard accessed: " + command[0]); });
      try { await import("./scripts/agentic/cons-flow-ux/dictation-history-probe.ts"); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("operator clipboard accessed");
  });

  test.each([
    ["semantic-command", "cons-flow-ux.semantic-command"],
    ["entry-verbs", "cons-flow-ux.entry-verbs"],
    ["dictation-recovery-focus", "cons-flow-ux.dictation-recovery-focus"],
    ["notes-search", "cons-flow-ux.notes-search"],
    ["dictation-delivery", "cons-flow-ux.dictation-delivery"],
    ["context-preparation", "cons-flow-ux.context-preparation"],
    ["notes-today", "cons-flow-ux.notes-today"],
    ["dictation-dismiss-targets", "cons-flow-ux.dictation-dismiss-targets"],
    ["flow-history", "cons-flow-ux.flow-history"],
    ["notes-handoff", "cons-flow-ux.notes-handoff"],
    ["notes-agent-chat-return", "cons-flow-ux.notes-agent-chat-return"],
    ["context-lifecycle", "cons-flow-ux.context-lifecycle"],
    ["conversation-hosts", "conversation-hosts.private-pasteboard-archive"],
    ["notes-actions", "notes-actions.private-pasteboard-archive"],
    ["dictation-history", "dictation-history.system-clipboard"],
  ])("consistency workflow %s refuses before runtime receipts, fixtures, or child processes", (name, expectedOwner) => {
    const result = child(`
      import * as filesystem from "node:fs";
      import * as asyncFilesystem from "node:fs/promises";
      import { mock } from "bun:test";
      const unsafe = (() => { throw new Error("unsafe consistency workflow side effect reached"); });
      mock.module("node:fs", () => ({
        ...filesystem,
        mkdirSync: unsafe,
        mkdtempSync: unsafe,
        writeFileSync: unsafe,
        rmSync: unsafe,
        unlinkSync: unsafe,
      }));
      mock.module("node:fs/promises", () => ({
        ...asyncFilesystem,
        mkdir: unsafe,
        unlink: unsafe,
        writeFile: unsafe,
      }));
      Bun.spawn = unsafe;
      Bun.spawnSync = unsafe;
      try { await import(${JSON.stringify("./scripts/agentic/cons-flow-ux/")} + ${JSON.stringify(name)} + "-probe.ts"); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain(`SCRIPT_KIT_NONINTERACTIVE=1 refused ${expectedOwner}`);
    expect(result.stdout.toString()).not.toContain("unsafe consistency workflow side effect reached");
  });

  test.each([
    ["notes-spine-host-wiring", "notes-spine-host-wiring.workflow-child"],
    ["day-page-context-roundtrip", "day-page-context-roundtrip.workflow-child"],
    ["day-page-agent-chat-handoff-scope", "day-page-agent-chat-handoff-scope.workflow-child"],
    ["day-agent-chat-return", "day-agent-chat-return.workflow-child"],
  ])("Notes/Today child %s refuses before auth, output, app, or native access", (name, expectedOwner) => {
    const result = child(`
      import * as filesystem from "node:fs";
      import { mock } from "bun:test";
      const unsafe = (() => { throw new Error("unsafe Notes/Today child side effect reached"); });
      mock.module("node:fs", () => ({
        ...filesystem,
        copyFileSync: unsafe,
        mkdirSync: unsafe,
        mkdtempSync: unsafe,
        writeFileSync: unsafe,
      }));
      Bun.spawn = unsafe;
      Bun.spawnSync = unsafe;
      try { await import(${JSON.stringify("./scripts/agentic/")} + ${JSON.stringify(name)} + "-probe.ts"); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain(`SCRIPT_KIT_NONINTERACTIVE=1 refused ${expectedOwner}`);
    expect(result.stdout.toString()).not.toContain("unsafe Notes/Today child side effect reached");
  });

  test("SAFE-001 refusal preserves its exact preexisting authoritative runtime receipt", () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-safe001-receipt-safety-"));
    const directory = join(root, ".artifacts", "consistency", "cons-flow-ux", "safe001-canonical-v2", "SAFE-001");
    const receipt = join(directory, "receipt.json");
    const owner = join(new URL("../..", import.meta.url).pathname, "scripts/agentic/cons-flow-ux/context-preparation-probe.ts");
    mkdirSync(directory, { recursive: true });
    writeFileSync(receipt, "preserve-exact-runtime-evidence");
    try {
      const result = child(`
        import { existsSync, readFileSync } from "node:fs";
        process.chdir(${JSON.stringify(root)});
        Bun.spawnSync = (() => { throw new Error("unsafe SAFE-001 native hash started"); });
        try { await import(${JSON.stringify(owner)}); }
        catch (error) { console.log("failure=" + error.message); }
        console.log("receiptExists=" + existsSync(${JSON.stringify(receipt)}));
        if (existsSync(${JSON.stringify(receipt)})) console.log("receipt=" + readFileSync(${JSON.stringify(receipt)}, "utf8"));
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused cons-flow-ux.context-preparation");
      expect(result.stdout.toString()).toContain("receiptExists=true");
      expect(result.stdout.toString()).toContain("receipt=preserve-exact-runtime-evidence");
      expect(result.stdout.toString()).not.toContain("unsafe SAFE-001 native hash started");
      expect(readFileSync(receipt, "utf8")).toBe("preserve-exact-runtime-evidence");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test.each([
    ["conversation hosts", "conversation-hosts-probe.ts"],
    ["Notes actions", "notes-actions-probe.ts"],
  ])("%s refuses before archiving every private pasteboard format", (_owner, file) => {
    const result = child(`
      import * as filesystem from "node:fs";
      import { mock } from "bun:test";
      mock.module("node:fs", () => ({
        ...filesystem,
        mkdtempSync: (() => "/tmp/script-kit-never-created-private-pasteboard"),
        mkdirSync: (() => undefined),
        writeFileSync: (() => undefined),
      }));
      Bun.spawnSync = ((command) => ({
        exitCode: 0,
        stdout: Buffer.from("0123456789abcdef  /tmp/reviewed-binary"),
        stderr: Buffer.from(""),
      }));
      Bun.spawn = ((command) => {
        console.log("private pasteboard archive helper reached: " + command[0]);
        throw new Error("private pasteboard archive helper reached: " + command[0]);
      });
      try { await import(${JSON.stringify("./scripts/agentic/cons-flow-ux/") } + ${JSON.stringify(file)}); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("private pasteboard archive helper reached");
  });

  test("root-search visual proof refuses before creating screenshots or launching the app", () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-root-visual-safety-"));
    const output = join(root, "uncreated-output");
    try {
      const result = child(`
        import { existsSync } from "node:fs";
        process.env.SCRIPT_KIT_GPUI_BINARY = process.execPath;
        process.argv = ["bun", "scripts/agentic/root-search-visual-stability.ts", "--out", ${JSON.stringify(output)}];
        Bun.spawn = (() => {
          console.log("unsafe root-search application started");
          throw new Error("unsafe root-search application started");
        });
        try { await import("./scripts/agentic/root-search-visual-stability.ts"); }
        catch (error) { console.log("failure=" + error.message); }
        console.log("outExists=" + existsSync(${JSON.stringify(output)}));
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
      expect(result.stdout.toString()).not.toContain("unsafe root-search application started");
      expect(result.stdout.toString()).toContain("outExists=false");
      expect(existsSync(output)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test.each([
    ["remote navigation", ["--url", "https://example.invalid/private"]],
    ["pixel capture", ["--screenshot", "/tmp/script-kit-never-captured.png"]],
  ])("browser fidelity %s refuses before launching agent-browser", (_kind, extra) => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("unsafe browser or screen capture started"); });
      process.argv = ["bun", "scripts/devtools/capture-dom-fidelity.ts", "--session", "operator-session", "--out", "/tmp/script-kit-never-written.json", ...${JSON.stringify(extra)}];
      try { await import("./scripts/devtools/capture-dom-fidelity.ts"); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("unsafe browser or screen capture started");
  });

  test("browser fidelity help stays passive and never starts a browser", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("passive browser help started a child"); });
      process.argv = ["bun", "scripts/devtools/capture-dom-fidelity.ts", "--help"];
      await import("./scripts/devtools/capture-dom-fidelity.ts");
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("capture-dom-fidelity.ts");
    expect(result.stdout.toString()).not.toContain("passive browser help started");
  });

  test("global input monitor preserves existing files and refuses before native observation", () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-interference-safety-"));
    const names = ["interference-ready.json", "interference-stop", "interference.json"];
    for (const name of names) writeFileSync(join(root, name), `preserve-${name}`);
    try {
      const result = child(`
        Bun.spawn = (() => { throw new Error("global keyboard and pointer monitor started"); });
        const { startInterferenceMonitor } = await import("./scripts/devtools/glass-interference.ts");
        try { startInterferenceMonitor("/tmp/never-launched-observer", ${JSON.stringify(root)}); }
        catch (error) { console.log("failure=" + error.message); }
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
      expect(result.stdout.toString()).not.toContain("global keyboard and pointer monitor started");
      for (const name of names) {
        expect(readFileSync(join(root, name), "utf8")).toBe(`preserve-${name}`);
      }
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("pure interference classification remains available without monitoring the operator", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("pure interference grading started a monitor"); });
      const { classifyInterference } = await import("./scripts/devtools/glass-interference.ts");
      console.log(JSON.stringify(classifyInterference({ status: "ok", untaggedInputCount: 0, frontmostAppChanged: false, pointerDeviationPx: 0, targetMovedExternally: false })));
    `);

    expect(result.exitCode).toBe(0);
    expect(JSON.parse(result.stdout.toString()).pass).toBe(true);
  });

  test.each([
    ["compiler identity", "queryCompilerIdentity"],
    ["cached native helper", "prepareHelper"],
  ])("%s refuses before Swift subprocess or cache creation", (_kind, owner) => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-swift-helper-safety-"));
    const cache = join(root, "uncreated-cache");
    try {
      const result = child(`
        import { existsSync } from "node:fs";
        Bun.spawn = (() => { throw new Error("unsafe Swift compiler started"); });
        const helper = await import("./scripts/devtools/glass-native-helper-cache.ts");
        try {
          if (${JSON.stringify(owner)} === "prepareHelper") {
            await helper.prepareHelper("fixture", { cacheDir: ${JSON.stringify(cache)} });
          } else {
            await helper.queryCompilerIdentity();
          }
        } catch (error) { console.log("failure=" + error.message); }
        console.log("cacheExists=" + existsSync(${JSON.stringify(cache)}));
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
      expect(result.stdout.toString()).not.toContain("unsafe Swift compiler started");
      expect(result.stdout.toString()).toContain("cacheExists=false");
      expect(existsSync(cache)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("native AppKit fixture refuses before Swift discovery, temporary files, or windows", () => {
    const result = child(`
      import * as subprocess from "node:child_process";
      import { mock } from "bun:test";
      mock.module("node:child_process", () => ({
        ...subprocess,
        spawn: (() => { throw new Error("unsafe AppKit fixture application started"); }),
        spawnSync: (() => { throw new Error("unsafe AppKit fixture compiler started"); }),
      }));
      const { SUITES } = await import("./scripts/devtools/window-engine-foundation.ts");
      try { await SUITES.native({ cycles: 1 }); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("unsafe AppKit fixture");
  });

  test("glass contrast refuses before output creation, subprocesses, or backdrop launch", () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-contrast-safety-"));
    const output = join(root, "uncreated-output");
    try {
      const result = child(`
        import { existsSync } from "node:fs";
        Bun.spawn = (() => { throw new Error("unsafe glass contrast subprocess started"); });
        process.argv = ["bun", "scripts/devtools/glass-motion-contrast.ts", "--binary", process.execPath, "--mode", "red", "--out", ${JSON.stringify(output)}];
        try { await import("./scripts/devtools/glass-motion-contrast.ts"); }
        catch (error) { console.log("failure=" + error.message); }
        console.log("outExists=" + existsSync(${JSON.stringify(output)}));
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
      expect(result.stdout.toString()).not.toContain("unsafe glass contrast subprocess started");
      expect(result.stdout.toString()).toContain("outExists=false");
      expect(existsSync(output)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("live smoke study refuses before reading manifests or validating backdrop permissions", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("unsafe smoke-study capture started"); });
      process.argv = ["bun", "scripts/agentic/glass-smoke-study.ts", "--manifest", "/tmp/script-kit-never-read-manifest.json", "--out", "/tmp/script-kit-never-written-study"];
      const { main } = await import("./scripts/agentic/glass-smoke-study.ts");
      try { await main(); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("unsafe smoke-study capture started");
    expect(result.stdout.toString()).not.toContain("ENOENT");
  });

  test("smoke-study dry-run keeps pure schedules and storage inspection without desktop effects", () => {
    const root = mkdtempSync(join(tmpdir(), "script-kit-smoke-dry-run-safety-"));
    const manifestPath = join(root, "study.json");
    const output = join(root, "uncreated-output");
    const baseline = join(root, "baseline");
    const candidate = join(root, "candidate");
    writeFileSync(baseline, "reviewed-baseline");
    writeFileSync(candidate, "reviewed-candidate");
    writeFileSync(manifestPath, JSON.stringify({
      schemaVersion: 1,
      studyId: "reviewed-passive-study",
      profile: "full",
      builds: [
        { id: "baseline", role: "baseline", binary: baseline, expected: { morphStartAlpha: 0.85 } },
        { id: "candidate", role: "candidate", binary: candidate, expected: { morphStartAlpha: 0.9 } },
      ],
      design: { type: "mirrored-cyclic", warmupsPerBuild: 3, requiredBlocks: 5, failureOnlyEarlyStop: true },
      fixture: { mode: "saturated-stripes" },
    }));
    try {
      const result = child(`
        Bun.spawn = ((command) => {
          if (!["df", "du"].includes(command[0])) {
            throw new Error("unsafe smoke-study desktop child started: " + command[0]);
          }
          const stdout = command[0] === "df"
            ? "Filesystem 1024-blocks Used Available Capacity Mounted on\\n/dev/reviewed 99999999 1 99999999 1% /\\n"
            : "1\\t/reviewed-history\\n";
          return { stdout: new Response(stdout).body, stderr: new Response("").body, exited: Promise.resolve(0), kill() {} };
        });
        process.argv = ["bun", "scripts/agentic/glass-smoke-study.ts", "--manifest", ${JSON.stringify(manifestPath)}, "--out", ${JSON.stringify(output)}, "--dry-run"];
        const { main } = await import("./scripts/agentic/glass-smoke-study.ts");
        const exitCode = await main();
        console.log("exitCode=" + exitCode);
      `);

      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString()).toContain('"status": "DRY_RUN"');
      expect(result.stdout.toString()).toContain("exitCode=0");
      expect(result.stdout.toString()).not.toContain("unsafe smoke-study desktop child started");
      expect(existsSync(output)).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test.each([
    ["surface navigation", ["surface-navigate", "--session", "reviewed-session"]],
    ["vision delegation", ["vision-loop", "--receipt", "/tmp/never-read.json", "--out-dir", "/tmp/never-written"]],
  ])("central agentic %s refuses its independent child before spawn", (_kind, args) => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("unsafe central agentic child started"); });
      process.argv = ["bun", "scripts/agentic/index.ts", ...${JSON.stringify(args)}];
      try { await import("./scripts/agentic/index.ts"); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("unsafe central agentic child started");
  });

  test("central agentic help restores all commands without spawning or promising a missing scenario", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("passive agentic help started a subprocess"); });
      process.argv = ["bun", "scripts/agentic/index.ts", "help", "--json"];
      await import("./scripts/agentic/index.ts");
    `);

    expect(result.exitCode).toBe(0);
    const help = JSON.parse(result.stdout.toString()) as {
      script: string;
      commands: Array<{ name: string; description: string }>;
    };
    expect(help.script).toBe("index");
    expect(help.commands.length).toBeGreaterThanOrEqual(127);
    expect(help.commands.find((command) => command.name === "confirm-modal-style-preview-proof")?.description)
      .toContain("scenario owner is missing");
    expect(result.stderr.toString()).not.toContain("passive agentic help started");
  });

  test("missing Confirm Modal scenario fails honestly without disabling unrelated commands", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("missing scenario started a subprocess"); });
      process.argv = ["bun", "scripts/agentic/index.ts", "confirm-modal-style-preview-proof", "--json"];
      await import("./scripts/agentic/index.ts");
    `);

    expect(result.exitCode).toBe(2);
    const receipt = JSON.parse(result.stdout.toString()) as Record<string, unknown>;
    expect(receipt.status).toBe("error");
    expect(receipt.failClosed).toBe(true);
    expect(receipt.reasonCode).toBe("missing_confirm_modal_style_preview_scenario");
    expect(result.stdout.toString()).not.toContain("missing scenario started");
  });

  test.each([
    ["activation", ["act", "set-input", "--text", "unsafe"]],
    ["app launch", ["driver", "smoke"]],
    ["performance recording", ["perf", "record", "--pid", "42", "--template", "Leaks"]],
    ["session start", ["actions", "inspect", "--start"]],
    ["window reveal", ["notes", "inspect", "--open"]],
  ])("central DevTools dispatcher rejects %s before delegation", (_kind, args) => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("unsafe DevTools dispatcher child started"); });
      process.argv = ["bun", "scripts/devtools/devtools.ts", ...${JSON.stringify(args)}];
      try { await import("./scripts/devtools/devtools.ts"); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("unsafe DevTools dispatcher child started");
  });

  test("central DevTools inventory remains genuinely passive", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("passive DevTools inventory started a subprocess"); });
      process.argv = ["bun", "scripts/devtools/devtools.ts", "list"];
      await import("./scripts/devtools/devtools.ts");
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("Usage: bun scripts/devtools/devtools.ts");
    expect(result.stdout.toString()).toContain("targets");
    expect(result.stderr.toString()).not.toContain("passive DevTools inventory started");
  });

  test.each([
    ["record", ["record", "--pid", "42", "--template", "Leaks"]],
    ["analyze", ["analyze", "--input", "/tmp/never-read.trace"]],
  ])("direct performance %s refuses before xctrace or filesystem access", (_kind, args) => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("unsafe xctrace profiler started"); });
      process.argv = ["bun", "scripts/devtools/perf.ts", ...${JSON.stringify(args)}];
      const { main } = await import("./scripts/devtools/perf.ts");
      try { await main(); }
      catch (error) { console.log("failure=" + error.message); }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 refused");
    expect(result.stdout.toString()).not.toContain("unsafe xctrace profiler started");
  });

  test("performance help remains available without profiling", () => {
    const result = child(`
      Bun.spawn = (() => { throw new Error("performance help started a profiler"); });
      process.argv = ["bun", "scripts/devtools/perf.ts", "--help"];
      const { main } = await import("./scripts/devtools/perf.ts");
      await main();
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("bun scripts/devtools/perf.ts record");
    expect(result.stdout.toString()).not.toContain("performance help started");
  });

  test("filterable matrix cleanup forwards exact owned identity and protects resumed sessions", () => {
    const result = child(`
      process.env.SCRIPT_KIT_NONINTERACTIVE = "0";
      const calls = [];
      let resumed = false;
      Bun.spawn = ((command) => {
        calls.push(command);
        const payload = command[2] === "start"
          ? { status: "ok", session: command[3], pid: 4242, sessionGeneration: "owned-generation", resumed, ready: true }
          : { status: "ok", session: command[3], ownershipVerified: true };
        return {
          stdout: new Response(JSON.stringify(payload)).body,
          stderr: new Response("").body,
          exited: Promise.resolve(0),
        };
      });
      const { sessionStart, sessionStop } = await import("./scripts/agentic/filterable-surface-matrix.ts");
      await sessionStart("owned-session");
      await sessionStop("owned-session");
      resumed = true;
      await sessionStart("borrowed-session");
      try { await sessionStop("borrowed-session"); }
      catch (error) { console.log("borrowed=" + error.message); }
      console.log("calls=" + JSON.stringify(calls));
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("borrowed=DevTools session is not owned");
    expect(result.stdout.toString()).toContain(
      '["bash","scripts/agentic/session.sh","stop","owned-session","--expected-pid","4242","--expected-generation","owned-generation"]',
    );
    expect(result.stdout.toString()).not.toContain('"stop","borrowed-session"');
  });

  test("scenario cleanup rewrites name-only stops to exact owned process identity", () => {
    const result = child(`
      process.env.SCRIPT_KIT_NONINTERACTIVE = "0";
      const calls = [];
      let resumed = false;
      Bun.spawn = ((command) => {
        calls.push(command);
        const payload = command[2] === "start"
          ? { status: "ok", session: command[3], pid: 4242, sessionGeneration: "scenario-generation", resumed, ready: true }
          : { status: "ok", session: command[3], ownershipVerified: true };
        return {
          stdout: new Response(JSON.stringify(payload)).body,
          stderr: new Response("").body,
          exited: Promise.resolve(0),
        };
      });
      const { runTool } = await import("./scripts/agentic/scenario.ts");
      await runTool(["bash", "scripts/agentic/session.sh", "start", "owned-session"], "start");
      await runTool(["bash", "scripts/agentic/session.sh", "stop", "owned-session"], "stop");
      resumed = true;
      await runTool(["bash", "scripts/agentic/session.sh", "start", "borrowed-session"], "resume");
      try { await runTool(["bash", "scripts/agentic/session.sh", "stop", "borrowed-session"], "stop-borrowed"); }
      catch (error) { console.log("borrowed=" + error.message); }
      console.log("calls=" + JSON.stringify(calls));
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("borrowed=DevTools session is not owned");
    expect(result.stdout.toString()).toContain(
      '["bash","scripts/agentic/session.sh","stop","owned-session","--expected-pid","4242","--expected-generation","scenario-generation"]',
    );
    expect(result.stdout.toString()).not.toContain('"stop","borrowed-session"');
  });

  test("scenario never starts or stops the reserved live operator session", () => {
    const result = child(`
      process.env.SCRIPT_KIT_NONINTERACTIVE = "0";
      Bun.spawn = (() => { throw new Error("live operator session was touched"); });
      const { runTool } = await import("./scripts/agentic/scenario.ts");
      for (const operation of ["start", "stop"]) {
        try {
          await runTool(["bash", "scripts/agentic/session.sh", operation, "dev-watch"], operation);
        } catch (error) { console.log(operation + "=" + error.message); }
      }
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("start=Agentic scenario cannot claim the borrowed operator session");
    expect(result.stdout.toString()).toContain("stop=DevTools session is not owned");
    expect(result.stdout.toString()).not.toContain("live operator session was touched");
  });

  test("Notes cleanup owns its exact session and never stops resumed operator state", () => {
    const result = child(`
      process.env.SCRIPT_KIT_NONINTERACTIVE = "0";
      const calls = [];
      let resumed = false;
      Bun.spawn = ((command) => {
        calls.push(command);
        const payload = command[2] === "start"
          ? { status: "ok", session: command[3], pid: 4242, sessionGeneration: "notes-generation", resumed, ready: true }
          : { status: "ok", session: command[3], ownershipVerified: true };
        return {
          stdout: new Response(JSON.stringify(payload)).body,
          stderr: new Response("").body,
          exited: Promise.resolve(0),
        };
      });
      const { maybeOpenNotes, stopSession } = await import("./scripts/devtools/notes.ts");
      await maybeOpenNotes({ start: true, session: "owned-notes", open: false });
      await stopSession("owned-notes");
      resumed = true;
      await maybeOpenNotes({ start: true, session: "borrowed-notes", open: false });
      try { await stopSession("borrowed-notes"); }
      catch (error) { console.log("borrowed=" + error.message); }
      try { await maybeOpenNotes({ start: true, session: "dev-watch", open: false }); }
      catch (error) { console.log("reserved=" + error.message); }
      console.log("calls=" + JSON.stringify(calls));
    `);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("borrowed=DevTools session is not owned");
    expect(result.stdout.toString()).toContain("reserved=Notes DevTools cannot claim the borrowed operator session");
    expect(result.stdout.toString()).toContain(
      '["bash","scripts/agentic/session.sh","stop","owned-notes","--expected-pid","4242","--expected-generation","notes-generation"]',
    );
    expect(result.stdout.toString()).not.toContain('"stop","borrowed-notes"');
    expect(result.stdout.toString()).not.toContain('"start","dev-watch"');
  });

  test("independent DevTools transports cannot bypass strict session-start safety", () => {
    const commands: Array<[string, string[]]> = [
      ["actions", ["inspect", "--start"]],
      ["dictation", ["inspect", "--start"]],
      ["inspect", ["--start"]],
      ["events", ["record", "--start", "--", "bun", "scripts/devtools/surfaces.ts"]],
      ["notes", ["inspect", "--start"]],
      ["main", ["inspect", "--start"]],
      ["agent_chat", ["open-detached-placeholder", "--start"]],
    ];
    for (const [tool, args] of commands) {
      const result = Bun.spawnSync([
        "bun",
        `scripts/devtools/${tool}.ts`,
        ...args,
      ], {
        cwd: new URL("../..", import.meta.url).pathname,
        env: {
          ...process.env,
          ...environment,
          CI: "false",
          SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
        },
        stdout: "pipe",
        stderr: "pipe",
      });
      expect(result.exitCode, `${tool} must refuse before starting a session`).not.toBe(0);
      expect(result.stderr.toString(), `${tool} must use the shared safety boundary`)
        .toContain("refused session.start");
      expect(result.stderr.toString()).toContain("session lifecycle mutation is forbidden");
    }
  });
});
