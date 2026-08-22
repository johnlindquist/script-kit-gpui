import { describe, expect, test } from "bun:test";
import {
  assertNoninteractiveDriverLaunch,
  assertNoninteractiveProtocolCommand,
  assertNoninteractiveSessionCommand,
  assertNoninteractiveUnownedSessionCommand,
  NONINTERACTIVE_SAFE_COMMAND_TYPES,
  NoninteractiveSafetyError,
} from "./lib/operator-safety.ts";
import { AttachedDriver } from "./driver.ts";

const environment = {
  SCRIPT_KIT_NONINTERACTIVE: "1",
  SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
  SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
  SCRIPT_KIT_ALLOW_LIVE_AI: "0",
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

  test("contradictory takeover, visible, or live-AI opt-ins fail closed", () => {
    for (const unsafeSetting of [
      "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
      "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
      "SCRIPT_KIT_ALLOW_LIVE_AI",
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
