import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { outputSummary, summarizeText } from "./lib/receipt-output.ts";

const envKeys = [
  "SCRIPT_KIT_TEST_STATUS",
  "SCRIPT_KIT_RECEIPT_SANDBOX",
  "SCRIPT_KIT_RECEIPT_ALLOW_FIXTURE_CLEARTEXT",
] as const;
const tempDirs: string[] = [];

afterEach(() => {
  for (const key of envKeys) delete process.env[key];
  for (const path of tempDirs.splice(0)) rmSync(path, { recursive: true, force: true });
});

describe("receipt output privacy", () => {
  test("summaries expose length and fingerprint but never a preview", () => {
    const canary = "PF003_OUTPUT_PREVIEW_CANARY";
    const summary = summarizeText(canary, 8);
    expect(summary.preview).toBeNull();
    expect(summary.rawContentReturned).toBe(false);
    expect(summary.bytes).toBe(canary.length);
    expect(summary.fingerprint).not.toBe("");
    expect(JSON.stringify(summary)).not.toContain(canary);
  });

  test("requesting fixture cleartext outside sandbox mode fails closed", async () => {
    process.env.SCRIPT_KIT_TEST_STATUS = "1";
    process.env.SCRIPT_KIT_RECEIPT_ALLOW_FIXTURE_CLEARTEXT = "1";
    const canary = "PF003_OUTSIDE_SANDBOX_CANARY";
    const directory = mkdtempSync(join(tmpdir(), "pf003-output-"));
    tempDirs.push(directory);
    const outputPath = join(directory, "receipt.json");

    const result = await outputSummary("outside-sandbox", canary, canary, {
      outputPath,
      previewBytes: 8,
      inlineFullOutput: true,
    });
    const artifact = readFileSync(outputPath, "utf8");

    expect(result.inlineFullOutput).toBe(false);
    expect(result.cleartextDenied).toBe(true);
    expect(result.stdout).toBeNull();
    expect(result.stderr).toBeNull();
    expect(artifact).not.toContain(canary);
  });

  test("fixture cleartext requires every explicit test and sandbox gate", async () => {
    process.env.SCRIPT_KIT_TEST_STATUS = "1";
    process.env.SCRIPT_KIT_RECEIPT_SANDBOX = "1";
    process.env.SCRIPT_KIT_RECEIPT_ALLOW_FIXTURE_CLEARTEXT = "1";
    const fixture = "deterministic-fixture-copy";

    const result = await outputSummary("sandbox-fixture", fixture, "", {
      outputPath: null,
      previewBytes: 8,
      inlineFullOutput: true,
    });

    expect(result.inlineFullOutput).toBe(true);
    expect(result.cleartextDenied).toBe(false);
    expect(result.stdout).toBe(fixture);
  });
});
