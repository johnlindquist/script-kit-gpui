import { expect, test } from "bun:test";

test("generated surface inventory includes every AI recovery host", () => {
  const generated = Bun.spawnSync(
    ["bun", "scripts/generate-surface-contracts.ts", "--check"],
    { stdout: "pipe", stderr: "pipe" },
  );
  expect(generated.exitCode).toBe(0);
  const result = Bun.spawnSync(["bun", "scripts/devtools/surfaces.ts"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  expect(result.exitCode).toBe(0);
  const report = JSON.parse(result.stdout.toString());
  const variants = report.surfaceContracts.flatMap(
    (entry: { appViewVariants: string[] }) => entry.appViewVariants,
  );
  for (const variant of [
    "AgentChatView",
    "ChatPrompt",
    "FlowUxView",
    "FlowSessionView",
  ]) {
    expect(variants).toContain(variant);
  }
});
