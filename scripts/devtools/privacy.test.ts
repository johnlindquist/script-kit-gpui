import { afterEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  assertNoCleartextCanaries,
  diagnostic,
  externalContent,
  filePath,
  privateReceiptFingerprint,
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
  test("legacy tool fingerprints use the same process-private HMAC instead of guessable public hashes", () => {
    const secret = "private spoken account password";
    const publicHash = createHash("sha256").update(secret).digest("hex");
    const first = privateReceiptFingerprint(secret);

    expect(first).toMatch(/^[a-f0-9]{64}$/);
    expect(first).toBe(privateReceiptFingerprint(secret));
    expect(first).not.toBe(publicHash);
    expect(first).not.toBe(privateReceiptFingerprint("another private phrase"));
  });

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

  test("undeclared credential fields and bare provider tokens never escape receipt redaction", () => {
    const privateValues = [
      "sk-proj-private-provider-secret",
      "unregistered-private-password",
      "unregistered-private-passphrase",
      "unregistered-private-cookie",
      "Bearer unregistered-private-bearer",
      "-----BEGIN PRIVATE KEY-----\nprivate-key-material\n-----END PRIVATE KEY-----",
      "gsk_unregistered_private_provider_token",
    ];
    const result = sanitizeReceipt({
      nested: {
        apiKey: privateValues[0],
        password: privateValues[1],
        passphrase: privateValues[2],
        cookie: privateValues[3],
        authorizationHeader: privateValues[4],
        privateKey: privateValues[5],
        unrelatedField: privateValues[6],
      },
    });

    const serialized = JSON.stringify(result.sanitized);
    for (const privateValue of privateValues) {
      expect(serialized).not.toContain(privateValue);
    }
    expect(result.redactedCount).toBe(privateValues.length);
    expect(result.unclassifiedSensitivePaths).toContain("nested.apiKey");
    expect(result.unclassifiedSensitivePaths).toContain("nested.password");
    expect(result.unclassifiedSensitivePaths).toContain("nested.unrelatedField");
  });

  test("known product crate paths never masquerade as provider credentials", () => {
    const path = "crates/sk-protocol/src/command_contract.rs";
    const result = sanitizeReceipt({ productionSources: [path] });

    expect(result.sanitized).toEqual({ productionSources: [path] });
    expect(result.redactedCount).toBe(0);
    expect(result.unclassifiedSensitivePaths).toEqual([]);
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

  test("authorized synthetic cleartext can never include actual credential bytes", () => {
    const password = "private-fixture-password";
    const providerKey = "sk-proj-private-fixture-provider-key";
    const result = sanitizeReceipt(
      {
        note: userContent("approved synthetic fixture text"),
        credential: secret(password),
        nested: { apiKey: providerKey },
      },
      {
        mode: "fixture-cleartext",
        fixtureId: "privacy-fixture-v1",
        sandboxHome: true,
        fixtureAllowsCleartext: true,
        callerAllowsCleartext: true,
        nativeDataInvolved: false,
      },
    );

    const serialized = JSON.stringify(result.sanitized);
    expect(result.mode).toBe("fixture-cleartext");
    expect(serialized).toContain("approved synthetic fixture text");
    expect(serialized).not.toContain(password);
    expect(serialized).not.toContain(providerKey);
    expect(result.redactedCount).toBe(2);
    expect(result.rawContentReturned).toBe(true);
  });

  test("cleartext canary assertion fails after downstream mutation", () => {
    process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES = canaries.note;
    expect(() => assertNoCleartextCanaries({ safe: canaries.note })).toThrow(
      "receipt privacy canary escaped",
    );
  });
});
