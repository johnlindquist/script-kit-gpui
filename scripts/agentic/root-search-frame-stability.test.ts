import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { normalizeProtocolResponse, OWNED_RESPONSE_ENCODING } from "../devtools/driver.ts";
import { validateArtifact, type ArtifactSpec } from "./artifact-lifecycle.ts";

const benchmark = "scripts/agentic/root-search-frame-stability.ts";
function inspect(args: string[], environment: Record<string, string> = {}) {
  return Bun.spawnSync(["bun", benchmark, ...args], {
    env: { ...process.env, CI: "false", SCRIPT_KIT_GPUI_BINARY: "/nonexistent/must-never-launch-gpui",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0", SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
      SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0", SCRIPT_KIT_ALLOW_LIVE_AI: "0", ...environment },
    stdout: "pipe", stderr: "pipe",
  });
}
describe("owned semantic frame stability proof contract", () => {
  test("help remains passive without an artifact or output receipt", () => {
    const result = inspect(["--help"]);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("--describe-contract");
    expect(result.stdout.toString()).toContain("--artifact <reference.json>");
  });
  test("semantic evidence excludes painted, focus and native-input claims", () => {
    const result = inspect(["--describe-contract"]);
    expect(result.exitCode).toBe(0);
    const contract = JSON.parse(result.stdout.toString());
    expect(contract).toMatchObject({ evidenceClass: "STATIC_INVENTORY", runtimeEvidenceClass: "RUNTIME_HIDDEN",
      metricKind: "semantic_frame_identity", observationClass: "SEMANTIC_FRAME", measuresPaint: false });
    expect(contract.safety).toEqual({ startsApplication: false, runtimeStartsApplication: true,
      runtimeRequiresSandboxHome: true, runtimeRequiresHiddenWindow: true, runtimeRequiresNoninteractive: true,
      runtimeRequiresCiEnvironment: false, runtimeRequiresSealedEvaluatorPermit: true,
      revealsWindow: false, focusesWindow: false, drivesNativeInput: false, capturesScreen: false });
  });
  test("dedicated delayed fixture keeps analyzer corruption separate from native evidence", () => {
    const result = inspect(["--describe-contract"]);
    expect(result.exitCode).toBe(0);
    const contract = JSON.parse(result.stdout.toString());
    expect(contract.fixtureId).toBe("main.root-search-frame-stability");
    expect(contract.negativeControl).toEqual({
      kind: "synthetic_semantic_fingerprint_mutation",
      appliedAfterProviderSettlement: true,
      nativeShiftObserved: false,
    });
  });
  test("runtime still requires explicit noninteractive intent before artifact access", () => {
    const result = inspect([], { SCRIPT_KIT_NONINTERACTIVE: "0" });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 is required");
    expect(result.stderr.toString()).not.toContain("must-never-launch-gpui");
  });
  test.each(["false", "true"])("CI=%s cannot replace an explicit evaluator artifact reference", (ci) => {
    const result = inspect([], { SCRIPT_KIT_NONINTERACTIVE: "1", CI: ci, SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1" });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("--artifact <reference.json>");
    expect(result.stderr.toString()).not.toContain("CI=true is required");
  });
  test("legacy executable overrides cannot select a proof artifact", () => {
    const result = inspect(["--binary", "/nonexistent/must-never-launch-gpui", "--receipt", "/tmp/frame-unused.json"], { SCRIPT_KIT_NONINTERACTIVE: "1" });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("--artifact <reference.json>");
    expect(result.stderr.toString()).not.toContain("ENOENT");
  });
});

test("physical owned response logs preserve exact protocol artifact correlations", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "root-frame-response-log-")));
  try {
    const legacy = ' { "type": "stateResult", "requestId": "legacy", "protocolVersion": 2 } ';
    const response = { type: "stateResult", requestId: "encoded", protocolVersion: 2,
      mainWindowPreflight: { selectedIndex: 0, visibleRowFingerprint: "same semantic frame" } };
    const decoded = Buffer.from(JSON.stringify(response)); const compressed = deflateSync(decoded, { level: 1 });
    const encoded = { type: "encodedResponse", version: 1, encoding: OWNED_RESPONSE_ENCODING,
      requestId: response.requestId, protocolVersion: response.protocolVersion, responseType: response.type,
      decodedBytes: decoded.length, compressedBytes: compressed.length, payload: compressed.toString("base64") };
    const physical = `${legacy}\n${JSON.stringify(encoded)}\n`;
    const log = join(root, "app.log"); const protocol = join(root, "protocol-responses.ndjson");
    writeFileSync(log, physical);
    const parsedLegacy = JSON.parse(legacy);
    expect(normalizeProtocolResponse(parsedLegacy)).toBe(parsedLegacy);
    writeFileSync(protocol, `${legacy}\n${JSON.stringify(normalizeProtocolResponse(encoded))}\n`);
    const spec: ArtifactSpec = { id: "protocol-responses", sourceName: "protocol-responses.ndjson", required: true,
      mediaType: "application/x-ndjson", kind: "ndjson",
      correlations: [{ requestId: "legacy", expectedType: "stateResult" }, { requestId: "encoded", expectedType: "stateResult" }] };
    const artifact = validateArtifact(protocol, spec, root);
    expect(artifact.validation.failures).toEqual([]);
    expect(artifact.validation.correlation).toMatchObject({ matchedExactlyOnce: 2, missing: [], duplicates: [], unexpectedType: [] });
    expect(readFileSync(log, "utf8")).toBe(physical);
    expect(readFileSync(protocol, "utf8")).toStartWith(`${legacy}\n`);
    expect(JSON.parse(readFileSync(protocol, "utf8").split("\n")[1]!)).toEqual(response);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
