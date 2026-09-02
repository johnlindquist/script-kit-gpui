import { createHash, createHmac, randomBytes } from "node:crypto";
import { extname } from "node:path";

export type JsonObject = Record<string, unknown>;

export const receiptContentKinds = [
  "ProductStatic",
  "UserContent",
  "ExternalContent",
  "FilePath",
  "Secret",
  "Diagnostic",
] as const;

export type ReceiptContentKind = (typeof receiptContentKinds)[number];
export type ReceiptPrivacyMode =
  | "live-redacted"
  | "fixture-redacted"
  | "fixture-cleartext";

const CONTENT_MARKER = "__scriptKitReceiptContent";
const liveFingerprintKey = randomBytes(32);
const liveKeyId = createHash("sha256")
  .update(liveFingerprintKey)
  .digest("hex")
  .slice(0, 16);

export interface ClassifiedReceiptContent {
  [CONTENT_MARKER]: true;
  kind: ReceiptContentKind;
  value: unknown;
}

export interface ReceiptPrivacyOptions {
  mode?: ReceiptPrivacyMode;
  fixtureId?: string | null;
  sandboxHome?: boolean;
  fixtureAllowsCleartext?: boolean;
  callerAllowsCleartext?: boolean;
  nativeDataInvolved?: boolean;
}

export interface PrivacyScanResult {
  sanitized: unknown;
  mode: ReceiptPrivacyMode;
  fingerprintAlgorithm: "hmac-sha256";
  keyId: string;
  rawContentReturned: boolean;
  redactedCount: number;
  canariesRedacted: number;
  canaryMatches: number;
  unclassifiedSensitivePaths: string[];
}

export function classifyReceiptContent(
  kind: ReceiptContentKind,
  value: unknown,
): ClassifiedReceiptContent {
  return { [CONTENT_MARKER]: true, kind, value };
}

export const productStatic = (value: unknown) =>
  classifyReceiptContent("ProductStatic", value);
export const userContent = (value: unknown) =>
  classifyReceiptContent("UserContent", value);
export const externalContent = (value: unknown) =>
  classifyReceiptContent("ExternalContent", value);
export const filePath = (value: unknown) =>
  classifyReceiptContent("FilePath", value);
export const secret = (value: unknown) => classifyReceiptContent("Secret", value);
export const diagnostic = (value: unknown) =>
  classifyReceiptContent("Diagnostic", value);

function privacyContext(options: ReceiptPrivacyOptions) {
  const mode = options.mode ?? "live-redacted";
  if (mode === "fixture-cleartext") {
    const allowed = options.sandboxHome === true
      && options.fixtureAllowsCleartext === true
      && options.callerAllowsCleartext === true
      && options.nativeDataInvolved !== true;
    if (!allowed) {
      return {
        mode: "fixture-redacted" as const,
        key: createHash("sha256")
          .update(`script-kit-fixture:${options.fixtureId ?? "missing"}`)
          .digest(),
        keyId: `fixture:${createHash("sha256").update(options.fixtureId ?? "missing").digest("hex").slice(0, 16)}`,
        cleartextAllowed: false,
        policyViolation: true,
      };
    }
    return {
      mode,
      key: createHash("sha256")
        .update(`script-kit-fixture:${options.fixtureId ?? "missing"}`)
        .digest(),
      keyId: `fixture:${createHash("sha256").update(options.fixtureId ?? "missing").digest("hex").slice(0, 16)}`,
      cleartextAllowed: true,
      policyViolation: false,
    };
  }
  if (mode === "fixture-redacted") {
    const fixtureId = options.fixtureId ?? "missing";
    return {
      mode,
      key: createHash("sha256").update(`script-kit-fixture:${fixtureId}`).digest(),
      keyId: `fixture:${createHash("sha256").update(fixtureId).digest("hex").slice(0, 16)}`,
      cleartextAllowed: false,
      policyViolation: false,
    };
  }
  return {
    mode,
    key: liveFingerprintKey,
    keyId: `run:${liveKeyId}`,
    cleartextAllowed: false,
    policyViolation: false,
  };
}

function fingerprint(value: string, key: Uint8Array): string {
  return createHmac("sha256", key).update(value).digest("hex");
}

/** Process-private equality token for tools consuming legacy cleartext state. */
export function privateReceiptFingerprint(value: string): string {
  return fingerprint(value, liveFingerprintKey);
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

function textDescriptor(
  kind: Exclude<ReceiptContentKind, "ProductStatic">,
  value: string,
  key: Uint8Array,
) {
  const descriptor: JsonObject = {
    redacted: true,
    contentKind: kind,
    length: value.length,
    byteLength: byteLength(value),
    fingerprintAlgorithm: "hmac-sha256",
    fingerprint: fingerprint(value, key),
    rawContentReturned: false,
  };
  if (kind === "FilePath") {
    descriptor.extension = extname(value) || null;
    descriptor.basenamePolicy = "fingerprint-only";
  }
  return descriptor;
}

function isClassified(value: unknown): value is ClassifiedReceiptContent {
  return Boolean(
    value
      && typeof value === "object"
      && !Array.isArray(value)
      && (value as JsonObject)[CONTENT_MARKER] === true
      && receiptContentKinds.includes((value as JsonObject).kind as ReceiptContentKind),
  );
}

const diagnosticKeys = new Set([
  "raw",
  "diagnostic",
  "diagnostics",
  "stdout",
  "stderr",
  "parsedError",
  "providerError",
  "osError",
  "errorMessage",
  "error",
  "errors",
]);

const userContentKeys = new Set([
  "inputValue",
  "selectedValue",
  "composerValue",
  "composerText",
  "transcript",
  "clipboard",
  "clipboardContent",
  "noteContent",
  "promptText",
  "selectionText",
]);

const secretKeyPattern =
  /(?:api[_-]?key|(?:access|refresh|bearer|auth|id|session)?[_-]?token|secret|password|passwd|passphrase|credential|authorization|cookie|private[_-]?key)(?:value|text|content|data|header)?$/i;
const secretValuePattern =
  /(?:\bbearer\s+[a-z0-9._~-]{8,}\b|\b(?:sk-(?:proj-[a-z0-9_-]{8,}|ant-api\d{2}-[a-z0-9_-]{8,}|[a-z0-9_-]{20,})|gsk_[a-z0-9_-]{8,}|gh[pousr]_[a-z0-9_]{8,}|github_pat_[a-z0-9_]{8,}|xox[baprs]-[a-z0-9-]{8,}|AIza[a-z0-9_-]{20,})\b|-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----)/i;

export function inferredKindForKey(key: string): Exclude<ReceiptContentKind, "ProductStatic"> | null {
  if (secretKeyPattern.test(key)) return "Secret";
  if (
    diagnosticKeys.has(key)
    || /^(?:raw|diagnostic)/i.test(key)
    || /(?:stdout|stderr|providerError|osError)$/i.test(key)
  ) {
    return "Diagnostic";
  }
  if (
    userContentKeys.has(key)
    || /^(?:composer|transcript|clipboard)/i.test(key)
    || /(?:input|selected|selection|note|prompt)(?:Value|Text|Content)$/i.test(key)
  ) {
    return "UserContent";
  }
  return null;
}

function looksLikeAbsolutePath(value: string): boolean {
  return value.startsWith("/") || value.startsWith("~/") || /^[A-Za-z]:[\\/]/.test(value);
}

function containsString(value: unknown): boolean {
  if (typeof value === "string") return value.length > 0;
  if (Array.isArray(value)) return value.some(containsString);
  if (value && typeof value === "object") return Object.values(value).some(containsString);
  return false;
}

function configuredCanaries(): string[] {
  return (process.env.SCRIPT_KIT_RECEIPT_PRIVACY_CANARIES ?? "")
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}

function configuredSecrets(): string[] {
  return Object.entries(process.env)
    .filter(([key, value]) =>
      Boolean(value)
      && value!.length >= 8
      && /(?:TOKEN|SECRET|PASSWORD|API_KEY|PRIVATE_KEY)$/i.test(key))
    .map(([, value]) => value!)
    .filter(Boolean);
}

export function sanitizeReceipt(
  value: unknown,
  options: ReceiptPrivacyOptions = {},
): PrivacyScanResult {
  const context = privacyContext(options);
  const canaries = configuredCanaries();
  const secrets = configuredSecrets();
  let redactedCount = 0;
  let canariesRedacted = 0;
  let rawContentReturned = false;
  const unclassifiedSensitivePaths = new Set<string>();
  if (context.policyViolation) {
    unclassifiedSensitivePaths.add("$privacy.fixtureCleartextPolicy");
  }

  function sanitize(
    current: unknown,
    path: string[],
    inheritedKind?: ReceiptContentKind,
  ): unknown {
    if (isClassified(current)) {
      if (current.kind === "ProductStatic") {
        return sanitize(current.value, path, "ProductStatic");
      }
      if (typeof current.value === "string") {
        const text = current.value;
        if (context.cleartextAllowed && current.kind !== "Secret") {
          rawContentReturned = true;
          return text;
        }
        canariesRedacted += canaries.filter((canary) => text.includes(canary)).length;
        redactedCount += 1;
        return textDescriptor(current.kind, text, context.key);
      }
      return sanitize(current.value, path, current.kind);
    }

    if (typeof current === "string") {
      const canary = canaries.find((candidate) => current.includes(candidate));
      const secretValue = secrets.find((candidate) => current.includes(candidate));
      const looksLikeCredential = secretValuePattern.test(current);
      if (canary) canariesRedacted += 1;
      const key = path[path.length - 1] ?? "";
      const inferredFromKey = inferredKindForKey(key);
      const inferredKind = inheritedKind && inheritedKind !== "ProductStatic"
        ? inheritedKind
        : inferredFromKey ?? (secretValue || looksLikeCredential
          ? "Secret"
          : looksLikeAbsolutePath(current) && /(?:path|uri|file|cwd|home)/i.test(key)
            ? "FilePath"
            : canary
              ? "UserContent"
              : null);
      if (inferredKind) {
        if (context.cleartextAllowed && inferredKind !== "Secret") {
          rawContentReturned = true;
          return current;
        }
        redactedCount += 1;
        if (!inheritedKind && (inferredFromKey || looksLikeCredential)) {
          unclassifiedSensitivePaths.add(path.join("."));
        }
        return textDescriptor(inferredKind, current, context.key);
      }
      return current;
    }

    if (Array.isArray(current)) {
      return current.map((entry, index) => sanitize(entry, [...path, String(index)], inheritedKind));
    }

    if (current && typeof current === "object") {
      const output: JsonObject = {};
      for (const [key, entry] of Object.entries(current as JsonObject)) {
        const inferredFromKey = inferredKindForKey(key);
        if (inferredFromKey && !inheritedKind && !isClassified(entry) && containsString(entry)) {
          unclassifiedSensitivePaths.add([...path, key].join("."));
        }
        output[key] = sanitize(
          entry,
          [...path, key],
          inferredFromKey ?? inheritedKind,
        );
      }
      return output;
    }

    return current;
  }

  const sanitized = sanitize(value, []);
  const serialized = JSON.stringify(sanitized);
  const canaryMatches = canaries.filter((canary) => serialized.includes(canary)).length;
  return {
    sanitized,
    mode: context.mode,
    fingerprintAlgorithm: "hmac-sha256",
    keyId: context.keyId,
    rawContentReturned,
    redactedCount,
    canariesRedacted,
    canaryMatches,
    unclassifiedSensitivePaths: [...unclassifiedSensitivePaths].sort(),
  };
}

export function assertNoCleartextCanaries(value: unknown): void {
  const serialized = JSON.stringify(value);
  for (const canary of configuredCanaries()) {
    if (serialized.includes(canary)) {
      throw new Error(
        `receipt privacy canary escaped: ${createHash("sha256").update(canary).digest("hex").slice(0, 12)}`,
      );
    }
  }
}
