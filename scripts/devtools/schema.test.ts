import { expect, test } from "bun:test";
import type { SchemaReport } from "./schema.ts";

test("schema advertises target-scoped AI reliability state", () => {
  const result = Bun.spawnSync(["bun", "scripts/devtools/schema.ts"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.exitCode).toBe(0);
  const schema: SchemaReport = JSON.parse(result.stdout.toString());
  const primitive = schema.primitiveSchemas.find(
    (entry: { primitive: string }) =>
      entry.primitive === "devtools.aiReliability.inspect",
  );
  expect(primitive).toBeDefined();
  expect(primitive?.requiredResultFields).toContain("state.diagnostic.redacted");
});

test("schema JSON preserves its report envelope and undeclared primitive fields", () => {
  const result = Bun.spawnSync(["bun", "scripts/devtools/schema.ts"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.exitCode).toBe(0);
  const output = result.stdout.toString();
  const schema: SchemaReport = JSON.parse(output);
  expect(output).toBe(`${JSON.stringify(schema, null, 2)}\n`);
  expect(Object.keys(schema)).toEqual([
    "schemaVersion", "tool", "generatedAt", "source", "philosophy",
    "receiptEnvelopeFields", "classifications", "targetIdentityFields",
    "primitiveSchemas", "ownedEvaluationProtocol", "executableReceiptRegistry",
    "acceptanceBar",
  ]);
  expect(schema.schemaVersion).toBe(2);
  expect(schema.tool).toBe("script-kit-devtools.schema");
  expect(schema.primitiveSchemas.slice(0, 3)).toEqual([
    { primitive: "devtools.design.run", requiredResultFields: ["artifactReference", "observation", "assertions", "cleanup"] },
    { primitive: "devtools.stories.run", requiredResultFields: ["library", "journeys", "cleanup"] },
    { primitive: "devtools.build-ops", requiredResultFields: ["buildOps", "safety", "cleanup"] },
  ]);
  expect(schema.ownedEvaluationProtocol.captureFrame.request).toEqual({
    type: "design",
    command: {
      operation: "captureFrame",
      target: { type: "instance", id: "string", generation: "positive integer" },
      includeImage: "boolean",
    },
  });
  expect(schema.ownedEvaluationProtocol.responseEncoding.capability).toEqual({
    version: 1, encoding: "zlib-json-base64-v1", requestField: "responseEncoding", responseType: "encodedResponse",
    delivery: "always", maxDecodedBytes: 6291456, maxCompressedBytes: 4194304,
  });
  expect(schema.ownedEvaluationProtocol.responseEncoding.request).toEqual({ responseEncoding: "zlib-json-base64-v1" });
  expect(schema.ownedEvaluationProtocol.responseEncoding.additionalResponseFields).toBe(false);
  expect(schema.ownedEvaluationProtocol.responseEncoding.requiredResponseFields).toEqual([
    "type", "version", "encoding", "requestId", "protocolVersion", "responseType", "decodedBytes", "compressedBytes", "payload",
  ]);
});

test("schema markdown renders every primitive and marks absent restrictions as not declared", () => {
  const json = Bun.spawnSync(["bun", "scripts/devtools/schema.ts"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(json.exitCode).toBe(0);
  const schema: SchemaReport = JSON.parse(json.stdout.toString());
  const result = Bun.spawnSync(["bun", "scripts/devtools/schema.ts", "--markdown"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.stderr.toString()).toBe("");
  expect(result.exitCode).toBe(0);
  const output = result.stdout.toString();
  expect(output).toStartWith("# Script Kit DevTools Receipt Schema\n");
  const rows = output.split("\n").filter((line) => line.startsWith("| devtools."));
  expect(rows).toHaveLength(schema.primitiveSchemas.length);
  for (const [index, primitive] of schema.primitiveSchemas.entries()) {
    expect(rows[index]).toBe(
      `| ${primitive.primitive} | ${primitive.requiredResultFields.join(", ")} | ${primitive.failClosedWhen === undefined ? "Not declared" : primitive.failClosedWhen.join(", ")} |`,
    );
  }
  expect(rows[0]).toBe("| devtools.design.run | artifactReference, observation, assertions, cleanup | Not declared |");
  expect(output).toContain(JSON.stringify(schema.ownedEvaluationProtocol.captureFrame, null, 2));
  expect(output).toContain(JSON.stringify(schema.ownedEvaluationProtocol.responseEncoding, null, 2));
  expect(output).toEndWith(`${schema.acceptanceBar.map((item) => `- ${item}`).join("\n")}\n`);
});
