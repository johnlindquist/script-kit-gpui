import { afterEach, describe, expect, test } from "bun:test";
import {
  assertNoCleartextCanaries,
  diagnostic,
  externalContent,
  filePath,
  productStatic,
  sanitizeReceipt,
  secret,
  userContent,
} from "./lib/privacy.ts";

const canaries = {
  note: "PF003_NOTE_CONTENT_7a4f",
  clipboard: "PF003_CLIPBOARD_0c91",
  path: "/Users/private/PF003_PATH_3e22/note.md",
  agent: "PF003_AGENT_CHAT_9d11",
  env: "PF003_ENV_SECRET_5b88",
  diagnostic: "PF003_PROVIDER_ERROR_1af0",
};

afterEach(() => {
  delete process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES;
  delete process.env.PF003_TEST_API_KEY;
});

describe("recursive receipt privacy", () => {
  test("explicit content kinds preserve product copy and redact private bytes", () => {
    const result = sanitizeReceipt({
      label: productStatic("Open Notes"),
      note: userContent(canaries.note),
      external: externalContent(canaries.agent),
      path: filePath(canaries.path),
      credential: secret(canaries.env),
      error: diagnostic(canaries.diagnostic),
    });
    const serialized = JSON.stringify(result.sanitized);
    expect(serialized).toContain("Open Notes");
    for (const canary of Object.values(canaries)) expect(serialized).not.toContain(canary);
    expect(result.redactedCount).toBe(5);
    expect(result.mode).toBe("live-redacted");
    expect(result.fingerprintAlgorithm).toBe("hmac-sha256");
    expect(result.keyId.startsWith("run:")).toBe(true);
    expect(result.rawContentReturned).toBe(false);
  });

  test("nested arrays, raw diagnostics, clipboard, composer and transcript fail closed", () => {
    process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES = Object.values(canaries).join(",");
    const result = sanitizeReceipt({
      nodes: [
        { raw: { message: canaries.diagnostic } },
        { clipboardContent: canaries.clipboard },
      ],
      state: {
        composerValue: canaries.agent,
        transcript: canaries.note,
      },
      source: { path: canaries.path },
    });
    const serialized = JSON.stringify(result.sanitized);
    for (const canary of Object.values(canaries)) expect(serialized).not.toContain(canary);
    expect(result.canariesRedacted).toBeGreaterThanOrEqual(5);
    expect(result.canaryMatches).toBe(0);
    expect(result.unclassifiedSensitivePaths).toContain("nodes.0.raw");
  });

  test("environment secret values are redacted without printing their names or bytes", () => {
    process.env.PF003_TEST_API_KEY = canaries.env;
    const result = sanitizeReceipt({ nested: { arbitrary: `prefix:${canaries.env}:suffix` } });
    expect(JSON.stringify(result.sanitized)).not.toContain(canaries.env);
    expect(result.redactedCount).toBe(1);
  });

  test("fixture cleartext fails closed unless every sandbox gate is explicit", () => {
    const denied = sanitizeReceipt(
      { note: userContent(canaries.note) },
      {
        mode: "fixture-cleartext",
        fixtureId: "privacy-fixture-v1",
        sandboxHome: true,
        fixtureAllowsCleartext: true,
        callerAllowsCleartext: false,
        nativeDataInvolved: false,
      },
    );
    expect(denied.mode).toBe("fixture-redacted");
    expect(denied.rawContentReturned).toBe(false);
    expect(denied.unclassifiedSensitivePaths).toContain("$privacy.fixtureCleartextPolicy");
    expect(JSON.stringify(denied.sanitized)).not.toContain(canaries.note);
  });

  test("cleartext canary assertion fails after downstream mutation", () => {
    process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES = canaries.note;
    expect(() => assertNoCleartextCanaries({ safe: canaries.note })).toThrow(
      "receipt privacy canary escaped",
    );
  });
});
