import { expect, test } from "bun:test";

test("schema advertises target-scoped AI reliability state", () => {
  const result = Bun.spawnSync(["bun", "scripts/devtools/schema.ts"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.exitCode).toBe(0);
  const schema = JSON.parse(result.stdout.toString());
  const primitive = schema.primitiveSchemas.find(
    (entry: { primitive: string }) =>
      entry.primitive === "devtools.aiReliability.inspect",
  );
  expect(primitive).toBeDefined();
  expect(primitive.requiredResultFields).toContain("state.diagnostic.redacted");
});
