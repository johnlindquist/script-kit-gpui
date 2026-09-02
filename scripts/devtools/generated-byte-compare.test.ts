import { afterEach, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createArtifactFixture } from "../agentic/build-artifact-fixture.ts";
import { GENERATED_BYTE_COMPARE_SOURCE_PATHS, GENERATED_BYTE_COMPARE_OUTPUT_PATHS, generateAuthoritativeByteComparison,
  parseGeneratedByteCompareArgs, validateGeneratedByteCompareEnvironment, validateGeneratedByteCompareReceipt } from "./generated-byte-compare.ts";
import { readReceiptDocument } from "./lib/receipt-artifact.ts";

const environment = { SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0", SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
  SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0", SCRIPT_KIT_ALLOW_LIVE_AI: "0", SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0", SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0" };
const cleanup: Array<() => void> = [];
afterEach(() => { for (const dispose of cleanup.splice(0).reverse()) dispose(); });
const bytes = { "tokens.json": '{"fixture":true}\n', "tokens.css": ':root{--fixture:1}\n' };
function fixture(body?: string) {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "exporter-v3-"))); cleanup.push(() => rmSync(root, { recursive: true, force: true }));
  const bootstrap = createArtifactFixture(root); bootstrap.dispose();
  for (const path of GENERATED_BYTE_COMPARE_SOURCE_PATHS) {
    const absolute = join(root, path); mkdirSync(dirname(absolute), { recursive: true });
    if (!existsSync(absolute)) writeFileSync(absolute, "// declared fixture owner\n");
  }
  for (const path of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
    const absolute = join(root, path); mkdirSync(dirname(absolute), { recursive: true }); writeFileSync(absolute, bytes[path.endsWith(".json") ? "tokens.json" : "tokens.css"]);
  }
  const script = '#!/bin/sh\nprintf started > exporter-started\n' + (body ?? `printf '%s' '${bytes["tokens.json"]}' > "$1/tokens.json"\nprintf '%s' '${bytes["tokens.css"]}' > "$1/tokens.css"\n`);
  const artifact = createArtifactFixture(root, { kind: "tool", existingRepository: true, executable: script }); cleanup.push(artifact.dispose);
  const options = { artifactReference: artifact.reference, outputPath: join(root, ".test-output", "byte-proof.json") };
  const run = (env = environment) => generateAuthoritativeByteComparison(options, { repositoryRoot: root, environment: env });
  const hash = (path: string) => { try { return createHash("sha256").update(readFileSync(join(root, path))).digest("hex"); } catch { return null; } };
  return { root, artifact, options, run, hash };
}

test("actual owned exporter binds V3 provenance and exactly the JSON/CSS bytes", async () => {
  const sandbox = fixture(); const receipt = await sandbox.run();
  expect(receipt.pass).toBe(true); expect(receipt.evidenceClass).toBe("UNIT_BEHAVIOR"); expect(receipt.provesRuntimeBehavior).toBe(false);
  expect(receipt.outputs).toHaveLength(2); expect(receipt.generatedOutputHashes).toEqual(receipt.outputHashes);
  expect(receipt.binary.artifactReference).toEqual(sandbox.artifact.reference);
  expect(receipt.cleanup).toMatchObject({ closed: true, processExited: true, processGroupExited: true, streamsDrained: true, referencesFinalized: true });
  expect(existsSync(sandbox.options.outputPath)).toBe(true);
  expect(validateGeneratedByteCompareReceipt(receipt, { currentSourceSha: receipt.sourceSha, currentFileSha256: sandbox.hash })).toEqual({ pass: true, errors: [] });
  const wire = readReceiptDocument(sandbox.options.outputPath);
  expect(wire.receiptFormat).toBeUndefined();
  expect(wire.outputs).toHaveLength(2);
  expect(validateGeneratedByteCompareReceipt(wire, { currentFileSha256: sandbox.hash })).toEqual({ pass: true, errors: [] });
});
test.each(Object.keys(environment).filter(key => key !== "SCRIPT_KIT_NONINTERACTIVE"))("unsafe %s refuses before executable invocation", async setting => {
  const sandbox = fixture(); await expect(sandbox.run({ ...environment, [setting]: "1" })).rejects.toThrow(setting);
  expect(existsSync(join(sandbox.root, "exporter-started"))).toBe(false);
});
test("noninteractive environment is mandatory before spawning", async () => {
  const sandbox = fixture(); await expect(sandbox.run({ ...environment, SCRIPT_KIT_NONINTERACTIVE: "0" })).rejects.toThrow("NONINTERACTIVE");
  expect(existsSync(join(sandbox.root, "exporter-started"))).toBe(false);
});
test("missing authority, altered manifest hash, wrong target and nonexecutable bytes refuse before spawn", async () => {
  const sandbox = fixture();
  for (const artifactReference of [undefined, { ...sandbox.artifact.reference, manifestPath: "../outside/manifest.json" }, { ...sandbox.artifact.reference, manifestSha256: "0".repeat(64) }])
    await expect(generateAuthoritativeByteComparison({ ...sandbox.options, artifactReference: artifactReference! }, { repositoryRoot: sandbox.root, environment })).rejects.toThrow();
  chmodSync(sandbox.artifact.executablePath, 0o400);
  try { await expect(sandbox.run()).rejects.toThrow(); } finally { chmodSync(sandbox.artifact.executablePath, 0o500); }
  expect(existsSync(join(sandbox.root, "exporter-started"))).toBe(false);
});
test("compiler-content drift rejects stale exporter rather than relabeling current HEAD", async () => {
  const sandbox = fixture(); const path = join(sandbox.root, "src/main.rs"); const original = readFileSync(path);
  writeFileSync(path, "fn changed() {}\n");
  try { await expect(sandbox.run()).rejects.toThrow(); } finally { writeFileSync(path, original); }
  expect(existsSync(join(sandbox.root, "exporter-started"))).toBe(false);
});
test("an exporter crash retains failure and closes its exact owned process", async () => {
  const sandbox = fixture("exit 3\n"); const receipt = await sandbox.run();
  expect(receipt.pass).toBe(false); expect(receipt.execution.exitCode).toBe(3); expect(receipt.cleanup.closed).toBe(true);
});
test.each(["tokens.json", "tokens.css"])("missing or mismatching generated %s cannot pass", async missing => {
  const other = missing === "tokens.json" ? "tokens.css" : "tokens.json";
  for (const mismatch of [false, true]) {
    const sandbox = fixture(`printf '%s' '${bytes[other]}' > "$1/${other}"\n${mismatch ? `printf wrong > "$1/${missing}"\n` : ""}`);
    const receipt = await sandbox.run(); expect(receipt.pass).toBe(false); expect(receipt.cleanup.closed).toBe(true);
  }
});
test("changing checked-in output during export fails even when generated bytes match baseline", async () => {
  const sandbox = fixture(`printf '%s' '${bytes["tokens.json"]}' > "$1/tokens.json"\nprintf '%s' '${bytes["tokens.css"]}' > "$1/tokens.css"\nprintf modified > design/mockups/generated/tokens.css\n`);
  const receipt = await sandbox.run(); expect(receipt.pass).toBe(false); expect(receipt.error).toContain("changed checked-in"); expect(receipt.cleanup.closed).toBe(true);
});
test("compiler-owner change during export is observed after execution with cleanup retained", async () => {
  const sandbox = fixture(`printf '%s' '${bytes["tokens.json"]}' > "$1/tokens.json"\nprintf '%s' '${bytes["tokens.css"]}' > "$1/tokens.css"\nprintf modified > src/main.rs\n`);
  const original = readFileSync(join(sandbox.root, "src/main.rs"));
  try { const receipt = await sandbox.run(); expect(receipt.pass).toBe(false); expect(receipt.cleanup.closed).toBe(true); }
  finally { writeFileSync(join(sandbox.root, "src/main.rs"), original); }
});
test("fresh owned output refuses overwriting an existing sentinel", async () => {
  const sandbox = fixture(); mkdirSync(dirname(sandbox.options.outputPath), { recursive: true }); writeFileSync(sandbox.options.outputPath, "sentinel");
  await expect(sandbox.run()).rejects.toThrow(); expect(readFileSync(sandbox.options.outputPath, "utf8")).toBe("sentinel");
});
test("CLI consumes an explicit reference and rejects duplicates and legacy binary/source claims", () => {
  const sandbox = fixture(); const referencePath = join(sandbox.root, "reference.json"); writeFileSync(referencePath, JSON.stringify(sandbox.artifact.reference));
  expect(parseGeneratedByteCompareArgs(["--artifact", referencePath, "--out", sandbox.options.outputPath]).artifactReference).toEqual(sandbox.artifact.reference);
  for (const invalid of [[], ["--binary", "export_design_tokens", "--source-sha", "a".repeat(40)], ["--artifact", referencePath],
    ["--artifact", referencePath, "--artifact", referencePath, "--out", sandbox.options.outputPath]]) expect(() => parseGeneratedByteCompareArgs(invalid)).toThrow();
  expect(validateGeneratedByteCompareEnvironment(environment)).toEqual([]);
});
test("every declared exporter owner and both checked-in outputs remain freshness-bound", async () => {
  const sandbox = fixture(); const receipt = await sandbox.run();
  for (const path of [...GENERATED_BYTE_COMPARE_SOURCE_PATHS, ...GENERATED_BYTE_COMPARE_OUTPUT_PATHS]) {
    expect(validateGeneratedByteCompareReceipt(receipt, { currentFileSha256: candidate => candidate === path ? "0".repeat(64) : sandbox.hash(candidate) }).pass).toBe(false);
  }
});
test("forged provenance, missing outputs, runtime claims, false safety, failed execution and cleanup remain non-green", async () => {
  const sandbox = fixture(); const receipt = await sandbox.run();
  const invalids = [
    { ...receipt, sourceSha: "not-a-commit" }, { ...receipt, binary: { ...receipt.binary, artifactReference: undefined } },
    { ...receipt, binary: { ...receipt.binary, sourceCommit: "b".repeat(40) } }, { ...receipt, outputs: receipt.outputs.slice(1) },
    { ...receipt, outputHashes: {} }, { ...receipt, evidenceClass: "RUNTIME_HIDDEN" }, { ...receipt, provesRuntimeBehavior: true },
    { ...receipt, safety: { ...receipt.safety, startsApplication: true } }, { ...receipt, execution: { ...receipt.execution, exitCode: 1 } },
    { ...receipt, cleanup: { ...receipt.cleanup, closed: false, survivors: [{ kind: "process", identity: "unknown", observation: "unknown" }] } },
    ...["/outside/export_design_tokens", "../outside/export_design_tokens", "target/../export_design_tokens", "target/script-kit-gpui"].map(path => ({ ...receipt, binary: { ...receipt.binary, path } })),
  ];
  for (const invalid of invalids) expect(validateGeneratedByteCompareReceipt(invalid, { currentFileSha256: sandbox.hash }).pass).toBe(false);
});

test("standalone release proof survives copying alone and stores no duplicate observation", async () => {
  const sandbox = fixture(); const receipt = await sandbox.run();
  const root = receipt.artifactLifecycle.output.root;
  expect(receipt.artifactLifecycle.artifacts).toEqual([]);
  expect(readdirSync(root)).not.toContain("observation.json");
  const publishedRoot = realpathSync(mkdtempSync(join(tmpdir(), "published-export-proof-")));
  cleanup.push(() => rmSync(publishedRoot, { recursive: true, force: true }));
  const copied = join(publishedRoot, "published-proof.json");
  copyFileSync(sandbox.options.outputPath, copied);
  // Dispose the immutable exporter while every managed record is still present.
  // Only then remove the source repository; the published receipt is independent.
  sandbox.artifact.dispose();
  cleanup.splice(cleanup.indexOf(sandbox.artifact.dispose), 1);
  rmSync(sandbox.root, { recursive: true, force: true });
  expect(validateGeneratedByteCompareReceipt(readReceiptDocument(copied), { currentSourceSha: receipt.sourceSha })).toEqual({ pass: true, errors: [] });
});
