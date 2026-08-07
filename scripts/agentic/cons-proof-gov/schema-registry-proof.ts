#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  prepareValidatedReceipt,
  receiptRegistryReport,
} from "../../devtools/lib/receipt-schema.ts";

process.env.SCRIPT_KIT_RECEIPT_TASK_IDS = "PF-001,PF-003";

const artifactDir = resolve(
  process.env.CONSISTENCY_RECEIPT_DIR
    ?? ".artifacts/consistency/PF-001",
);
const registryPath = join(artifactDir, "schema-registry.json");
const positiveInputPath = join(artifactDir, "positive-layout-input.json");
const positiveValidationPath = join(artifactDir, "positive-layout-validation.json");
const negativeInputPath = join(artifactDir, "negative-missing-bounds-input.json");
const negativeValidationPath = join(artifactDir, "negative-missing-bounds-validation.json");

type Obj = Record<string, unknown>;

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Obj
    : {};
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function fingerprint(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function layoutReceipt(extra: Obj = {}): Obj {
  return {
    schemaVersion: 2,
    tool: "script-kit-devtools.layout",
    command: "layout.measure",
    classification: "ok",
    requestedTarget: { selector: { type: "main" } },
    target: {
      automationId: "main",
      bounds: { x: 0, y: 0, width: 800, height: 600 },
    },
    window: { rect: { x: 0, y: 0, width: 800, height: 600 } },
    regions: [],
    resizePressure: { windowCanGrow: true },
    pressure: { pressureScore: 0 },
    missingPrimitives: [],
    warnings: [],
    errors: [],
    ...extra,
  };
}

async function runJson(command: string[], timeoutMs = 20_000): Promise<{
  exitCode: number;
  json: Obj;
  stdoutBytes: number;
  stdoutFingerprint: string;
}> {
  const proc = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const timeout = Bun.sleep(timeoutMs).then(() => "timeout" as const);
  const completion = Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]).then(([stdout, stderr, exitCode]) => ({ stdout, stderr, exitCode }));
  const result = await Promise.race([completion, timeout]);
  if (result === "timeout") {
    proc.kill();
    throw new Error("producer validation timed out");
  }
  const parsed = JSON.parse(result.stdout) as Obj;
  return {
    exitCode: result.exitCode,
    json: parsed,
    stdoutBytes: new TextEncoder().encode(result.stdout).length,
    stdoutFingerprint: fingerprint(result.stdout),
  };
}

await mkdir(artifactDir, { recursive: true });
const positiveInput = layoutReceipt();
const negativeInput = layoutReceipt({
  target: { automationId: "main" },
});
await writeFile(positiveInputPath, `${JSON.stringify(positiveInput, null, 2)}\n`);
await writeFile(negativeInputPath, `${JSON.stringify(negativeInput, null, 2)}\n`);

const positiveCli = await runJson([
  "bun",
  "scripts/devtools/schema.ts",
  "validate",
  "--primitive",
  "devtools.layout.measure",
  "--receipt",
  positiveInputPath,
]);
const negativeCli = await runJson([
  "bun",
  "scripts/devtools/schema.ts",
  "validate",
  "--primitive",
  "devtools.layout.measure",
  "--receipt",
  negativeInputPath,
]);
await writeFile(positiveValidationPath, `${JSON.stringify(positiveCli.json, null, 2)}\n`);
await writeFile(negativeValidationPath, `${JSON.stringify(negativeCli.json, null, 2)}\n`);
assert(positiveCli.exitCode === 0, "positive schema validation did not exit zero");
assert(positiveCli.json.disposition === "EVALUABLE_PASS", "positive schema validation did not pass");
assert(negativeCli.exitCode !== 0, "missing-bounds validation did not exit nonzero");
assert(negativeCli.json.disposition === "INVALID_SCHEMA", "missing-bounds validation did not fail closed");

const negativeControls = {
  missingBounds: prepareValidatedReceipt(
    "devtools.layout.measure",
    negativeInput,
  ).receipt.disposition,
  requiredNull: prepareValidatedReceipt(
    "devtools.layout.measure",
    layoutReceipt({ resizePressure: null }),
  ).receipt.disposition,
  failedAssertionMarkedPass: prepareValidatedReceipt(
    "devtools.layout.measure",
    layoutReceipt({ assertions: [{ name: "fits", pass: false }] }),
  ).receipt.disposition,
  requiredMissingPrimitive: prepareValidatedReceipt(
    "devtools.layout.measure",
    layoutReceipt({ missingPrimitives: ["windowCanGrow"] }),
  ).receipt.disposition,
  duplicateSemanticIds: prepareValidatedReceipt(
    "devtools.elements.snapshot",
    {
      schemaVersion: 2,
      tool: "script-kit-devtools.elements",
      command: "elements.snapshot",
      classification: "ok",
      requestedTarget: {},
      target: {},
      semanticSurface: {},
      nodes: [{ semanticId: "same" }, { semanticId: "same" }],
      duplicateSemanticIds: ["same"],
      missingPrimitives: [],
      errors: [],
    },
  ).receipt.disposition,
  duplicateKeyboardKey: prepareValidatedReceipt(
    "devtools.keyboard.inspect",
    {
      schemaVersion: 2,
      tool: "script-kit-devtools.keyboard",
      command: "keyboard.inspect",
      classification: "ok",
      requestedTarget: {},
      target: {},
      keyboardPolicy: "host",
      inputOwnership: "host",
      bindings: [{ key: "cmd+k" }, { key: "cmd+k" }],
      duplicateKeys: ["cmd+k"],
      missingPrimitives: [],
      errors: [],
    },
  ).receipt.disposition,
};
for (const [name, disposition] of Object.entries(negativeControls)) {
  assert(String(disposition).startsWith("INVALID_"), `${name} did not produce an INVALID_* disposition`);
}

const temp = mkdtempSync(join(tmpdir(), "pf001-producers-"));
try {
  const comparisonFixture = {
    schemaVersion: 2,
    tool: "fixture",
    command: "fixture",
    classification: "blocked-by-missing-primitive",
    requestedTarget: { selector: { type: "main" } },
    target: { automationId: "main", surfaceKind: "ScriptList" },
    missingPrimitives: ["fixture"],
  };
  const redPath = join(temp, "red.json");
  const greenPath = join(temp, "green.json");
  await writeFile(redPath, JSON.stringify(comparisonFixture));
  await writeFile(greenPath, JSON.stringify(comparisonFixture));
  const producers: Array<[string, string[]]> = [
    ["targets.list", ["bun", "scripts/devtools/targets.ts", "list", "--session", "c02-missing"]],
    ["targets.inspect", ["bun", "scripts/devtools/targets.ts", "inspect", "--session", "c02-missing", "--main"]],
    ["surface.inspect", ["bun", "scripts/devtools/surface.ts", "inspect", "--session", "c02-missing", "--main", "--surface", "ScriptList"]],
    ["elements.snapshot", ["bun", "scripts/devtools/elements.ts", "snapshot", "--session", "c02-missing", "--main"]],
    ["layout.measure", ["bun", "scripts/devtools/layout.ts", "measure", "--session", "c02-missing", "--main"]],
    ["scroll.inspect", ["bun", "scripts/devtools/scroll.ts", "inspect", "--session", "c02-missing", "--main"]],
    ["focus.inspect", ["bun", "scripts/devtools/focus.ts", "inspect", "--session", "c02-missing", "--main"]],
    ["text.measure", ["bun", "scripts/devtools/text.ts", "measure", "--session", "c02-missing", "--main"]],
    ["keyboard.inspect", ["bun", "scripts/devtools/keyboard.ts", "inspect", "--session", "c02-missing", "--main"]],
    ["actions.inspect", ["bun", "scripts/devtools/actions.ts", "inspect", "--session", "c02-missing", "--main"]],
    ["act.key", ["bun", "scripts/devtools/act.ts", "key", "--session", "c02-missing", "--main", "--key", "Escape"]],
    ["compare.redgreen", ["bun", "scripts/devtools/compare.ts", "redgreen", "--red", redPath, "--green", greenPath]],
    ["notes.inspect", ["bun", "scripts/devtools/notes.ts", "inspect", "--session", "c02-missing"]],
    ["dictation.inspect", ["bun", "scripts/devtools/dictation.ts", "inspect", "--session", "c02-missing"]],
    ["inspect.orchestrate", ["bun", "scripts/devtools/inspect.ts", "--session", "c02-missing", "--main", "--bug", "static fixture", "--surface", "ScriptList"]],
  ];
  const producerMatrix = [];
  for (const [producer, command] of producers) {
    const result = await runJson(command);
    const validation = asObj(result.json.validation);
    const privacy = asObj(result.json.privacy);
    assert(validation.passed === true, `${producer} did not validate before output`);
    assert(!String(result.json.disposition).startsWith("INVALID_"), `${producer} emitted an invalid receipt`);
    assert(Array.isArray(privacy.unclassifiedSensitivePaths) && privacy.unclassifiedSensitivePaths.length === 0, `${producer} left unclassified sensitive fields`);
    producerMatrix.push({
      producer,
      disposition: result.json.disposition,
      classification: result.json.classification,
      validationPassed: true,
      unclassifiedSensitivePaths: 0,
      canaryMatches: privacy.canaryMatches ?? 0,
      exitCode: result.exitCode,
      stdoutBytes: result.stdoutBytes,
      stdoutFingerprint: result.stdoutFingerprint,
    });
  }

  const registry = receiptRegistryReport();
  const primitiveIds = registry.map((entry) => entry.primitiveId);
  assert(new Set(primitiveIds).size === primitiveIds.length, "primitive registry contains duplicate IDs");
  const receipt = {
    schemaVersion: 2,
    taskId: "PF-001",
    classification: "PASS",
    registry,
    registryCount: registry.length,
    primitiveIdsUnique: true,
    producerMatrix,
    producerCount: producerMatrix.length,
    positiveValidation: {
      command: "bun scripts/devtools/schema.ts validate --primitive devtools.layout.measure --receipt .artifacts/consistency/PF-001/positive-layout-input.json",
      exitCode: positiveCli.exitCode,
      disposition: positiveCli.json.disposition,
      validationPassed: asObj(positiveCli.json.validation).passed,
      stdoutBytes: positiveCli.stdoutBytes,
      stdoutFingerprint: positiveCli.stdoutFingerprint,
    },
    negativeValidation: {
      command: "bun scripts/devtools/schema.ts validate --primitive devtools.layout.measure --receipt .artifacts/consistency/PF-001/negative-missing-bounds-input.json",
      exitCode: negativeCli.exitCode,
      disposition: negativeCli.json.disposition,
      validationPassed: asObj(negativeCli.json.validation).passed,
      stdoutBytes: negativeCli.stdoutBytes,
      stdoutFingerprint: negativeCli.stdoutFingerprint,
    },
    negativeControls,
  };
  await writeFile(registryPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify(receipt, null, 2));
} finally {
  rmSync(temp, { recursive: true, force: true });
}
