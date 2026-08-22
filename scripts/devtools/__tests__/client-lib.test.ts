import { describe, expect, test, afterEach } from "bun:test";
import {
  classifyEnvelopeError,
  classifyEnvelopes,
  clipboardCaptureFixtureCommand,
  gpuiKeyDownCommand,
  triggerActionCommand,
  finishReceipt,
  parseTargetArgs,
  requestId,
  serializeTargetFlags,
  startClock,
  SESSION_LIFECYCLE_CODES,
} from "../lib/client.ts";

describe("DevTools command builders", () => {
  test("clipboard fixture uses a file payload and preserves capture metadata", () => {
    expect(clipboardCaptureFixtureCommand({
      payloadPath: "/sandbox/.scriptkit/devtools-fixtures/oversize.txt",
      sourceBundleId: "com.apple.TextEdit",
      concealedTypes: ["org.nspasteboard.ConcealedType"],
      changeGeneration: 42,
    })).toEqual({
      type: "injectClipboardCaptureFixture",
      payload: { type: "textFile", path: "/sandbox/.scriptkit/devtools-fixtures/oversize.txt" },
      sourceBundleId: "com.apple.TextEdit",
      concealedTypes: ["org.nspasteboard.ConcealedType"],
      changeGeneration: 42,
    });
  });

  test("real key and direct action builders use receipt-backed routes", () => {
    expect(gpuiKeyDownCommand("space", " ", [], { type: "main" })).toEqual({
      type: "simulateGpuiEvent",
      target: { type: "main" },
      event: { type: "keyDown", key: "space", modifiers: [], text: " " },
    });
    expect(triggerActionCommand("clip:clipboard_keep_in_today", "clipboardHistory")).toEqual({
      type: "triggerAction",
      actionId: "clip:clipboard_keep_in_today",
      host: "clipboardHistory",
    });
  });
});

describe("classifyEnvelopeError", () => {
  test("ok envelope classifies ok", () => {
    expect(classifyEnvelopeError({ status: "ok" })).toBe("ok");
    expect(classifyEnvelopeError({ type: "stateResult" })).toBe("ok");
  });

  test("every session lifecycle code maps to blocked-by-session-lifecycle", () => {
    for (const code of SESSION_LIFECYCLE_CODES) {
      // session.sh json_lifecycle_error emits {status:"error", error:{code}}
      expect(
        classifyEnvelopeError({ status: "error", error: { code } }),
      ).toBe("blocked-by-session-lifecycle");
      // run() surfaces the same envelope re-parsed from stdout as parsedError
      expect(
        classifyEnvelopeError({ status: "error", parsedError: { status: "error", error: { code } } }),
      ).toBe("blocked-by-session-lifecycle");
    }
  });

  test("transport codes map to their precise classifications", () => {
    const cases: Array<[string, string]> = [
      ["queue_timeout", "blocked-by-session-queue"],
      ["response_timeout", "blocked-by-response-timeout"],
      ["timeout", "blocked-by-response-timeout"],
      ["parse_error", "blocked-by-parse-error"],
    ];
    for (const [code, expected] of cases) {
      expect(
        classifyEnvelopeError({ status: "error", error: { code } }),
      ).toBe(expected);
    }
  });

  test("unrecognized error still fails closed", () => {
    const classification = classifyEnvelopeError({ status: "error", stderr: "garbage" });
    expect(classification).not.toBe("ok");
    expect(classification.startsWith("blocked-by-")).toBe(true);
  });
});

describe("classifyEnvelopes", () => {
  test("returns first non-ok classification", () => {
    expect(
      classifyEnvelopes([
        { status: "ok" },
        { status: "error", error: { code: "session_dead" } },
        { status: "error", error: { code: "parse_error" } },
      ]),
    ).toBe("blocked-by-session-lifecycle");
  });

  test("all ok yields ok", () => {
    expect(classifyEnvelopes([{ status: "ok" }, { type: "elementsResult" }])).toBe("ok");
  });
});

describe("finishReceipt", () => {
  test("adds the shared envelope header around the body", () => {
    const clock = startClock();
    const receipt = finishReceipt(
      { tool: "script-kit-devtools.test", command: "test.run", session: "unit-test-session", clock },
      { classification: "ok", custom: 42 },
    );
    expect(receipt.schemaVersion).toBe(2);
    expect(receipt.tool).toBe("script-kit-devtools.test");
    expect(receipt.command).toBe("test.run");
    expect(receipt.session).toBe("unit-test-session");
    expect(typeof receipt.startedAt).toBe("string");
    expect(typeof receipt.endedAt).toBe("string");
    expect(typeof receipt.durationMs).toBe("number");
    expect((receipt.durationMs as number) >= 0).toBe(true);
    // No session dir exists for this name — fingerprint must degrade, not throw.
    expect(receipt.binary === null || typeof receipt.binary === "object").toBe(true);
    expect(receipt.classification).toBe("ok");
    expect(receipt.custom).toBe(42);
    expect(receipt.evidenceClass).toBe("UNCLASSIFIED");
  });

  test("derives hidden versus visible runtime evidence from observed target visibility", () => {
    const meta = {
      tool: "script-kit-devtools.test",
      command: "test.run",
      session: "unit-test-session",
      clock: startClock(),
    };
    const hidden = finishReceipt(meta, {
      transaction: { transactionId: "proof:hidden" },
      target: { visible: false },
    });
    expect(hidden.evidenceClass).toBe("RUNTIME_HIDDEN");
    expect(hidden.evidenceObservation).toEqual({
      observedWindowVisible: false,
      visibilitySources: ["target.visible"],
      errors: [],
    });

    const visible = finishReceipt(meta, {
      transaction: { transactionId: "proof:visible" },
      resolvedTarget: { visible: true },
    });
    expect(visible.evidenceClass).toBe("RUNTIME_VISIBLE");

    const unobserved = finishReceipt(meta, {
      transaction: { transactionId: "proof:unobserved" },
    });
    expect(unobserved.evidenceClass).toBe("RUNTIME_VISIBILITY_UNVERIFIED");
  });

  test("an unscoped live target inventory is never advertised as static or direct proof", () => {
    const receipt = finishReceipt(
      {
        tool: "script-kit-devtools.targets",
        command: "targets.list",
        session: "unit-test-session",
        clock: startClock(),
      },
      { targetCount: 1, targets: [{ automationId: "main", visible: false }] },
    );
    expect(receipt.evidenceClass).toBe("RUNTIME_UNSCOPED");
  });
});

describe("requestId", () => {
  test("is unique and carries tool + phase", () => {
    const a = requestId("focus", "state");
    const b = requestId("focus", "state");
    expect(a).toContain("focus");
    expect(a).toContain("state");
    expect(a).not.toBe(b);
  });
});

describe("parseTargetArgs", () => {
  const savedEnv = process.env.SCRIPT_KIT_DEVTOOLS_SESSION;
  afterEach(() => {
    if (savedEnv === undefined) {
      delete process.env.SCRIPT_KIT_DEVTOOLS_SESSION;
    } else {
      process.env.SCRIPT_KIT_DEVTOOLS_SESSION = savedEnv;
    }
  });

  test("defaults", () => {
    delete process.env.SCRIPT_KIT_DEVTOOLS_SESSION;
    const { args, warnings } = parseTargetArgs([]);
    expect(args.session).toBe("default");
    expect(args.sessionExplicit).toBe(false);
    expect(args.strict).toBe(false);
    expect(args.start).toBe(false);
    expect(args.target).toBeUndefined();
    expect(warnings).toEqual([]);
  });

  test("target selectors", () => {
    expect(parseTargetArgs(["--target-id", "win-3"]).args.target).toEqual({ type: "id", id: "win-3" });
    expect(parseTargetArgs(["--target-kind", "notes", "--target-index", "2"]).args.target).toEqual({
      type: "kind",
      kind: "notes",
      index: 2,
    });
    expect(parseTargetArgs(["--target-title", "Day Page"]).args.target).toEqual({
      type: "titleContains",
      text: "Day Page",
    });
    expect(parseTargetArgs(["--focused"]).args.target).toEqual({ type: "focused" });
    expect(parseTargetArgs(["--main"]).args.target).toEqual({ type: "main" });
    expect(parseTargetArgs(["--target-json", '{"type":"kind","kind":"terminal"}']).args.target).toEqual({
      type: "kind",
      kind: "terminal",
    });
  });

  test("extras are typed and defaulted by the caller", () => {
    const { extras } = parseTargetArgs(["--limit", "50", "--hi-dpi"], {
      extras: { "--limit": "number", "--hi-dpi": "boolean" },
    });
    expect(extras["--limit"]).toBe(50);
    expect(extras["--hi-dpi"]).toBe(true);
  });

  test("env session override marks session explicit", () => {
    process.env.SCRIPT_KIT_DEVTOOLS_SESSION = "loop-7";
    const { args, warnings } = parseTargetArgs(["--start"]);
    expect(args.session).toBe("loop-7");
    expect(args.sessionExplicit).toBe(true);
    expect(warnings).toEqual([]);
  });

  test("--start on the implicit shared session warns", () => {
    delete process.env.SCRIPT_KIT_DEVTOOLS_SESSION;
    const { warnings } = parseTargetArgs(["--start"]);
    expect(warnings.length).toBe(1);
    expect(warnings[0]).toContain("implicit shared session");
  });

  test("--session flag wins and suppresses the warning", () => {
    delete process.env.SCRIPT_KIT_DEVTOOLS_SESSION;
    const { args, warnings } = parseTargetArgs(["--session", "probe-1", "--start"]);
    expect(args.session).toBe("probe-1");
    expect(args.sessionExplicit).toBe(true);
    expect(warnings).toEqual([]);
  });
});

describe("serializeTargetFlags", () => {
  test("round-trips through parseTargetArgs", () => {
    const original = parseTargetArgs([
      "--session",
      "s9",
      "--target-kind",
      "notes",
      "--target-index",
      "1",
      "--strict",
      "--surface",
      "Notes",
      "--timeout",
      "9000",
    ]).args;
    const reparsed = parseTargetArgs(serializeTargetFlags(original)).args;
    expect(reparsed.session).toBe("s9");
    expect(reparsed.target).toEqual({ type: "kind", kind: "notes", index: 1 });
    expect(reparsed.strict).toBe(true);
    expect(reparsed.expectedSurfaceKind).toBe("Notes");
    expect(reparsed.timeoutMs).toBe(9000);
  });
});
