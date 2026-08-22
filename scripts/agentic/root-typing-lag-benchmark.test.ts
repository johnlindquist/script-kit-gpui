import { describe, expect, test } from "bun:test";

const benchmark = "scripts/agentic/root-typing-lag-benchmark.ts";

function run(args: string[], overrides: Record<string, string> = {}) {
  const environment = { ...process.env };
  delete environment.SCRIPT_KIT_ALLOW_VISIBLE_PROBES;
  environment.SCRIPT_KIT_NONINTERACTIVE = "0";
  environment.SCRIPT_KIT_GPUI_BINARY = "/nonexistent/must-never-launch-gpui";
  Object.assign(environment, overrides);
  return Bun.spawnSync(["bun", benchmark, ...args], {
    env: environment,
    stdout: "pipe",
    stderr: "pipe",
  });
}

describe("root typing benchmark operator safety", () => {
  test("help cannot launch or reveal the application", () => {
    const result = run(["--help"]);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("SCRIPT_KIT_ALLOW_VISIBLE_PROBES=1");
  });

  test("contract inspection is static, honest, and screen-safe", () => {
    const result = run(["--describe-contract"]);
    expect(result.exitCode).toBe(0);
    const contract = JSON.parse(result.stdout.toString());
    expect(contract.evidenceClass).toBe("STATIC_INVENTORY");
    expect(contract.runtimeEvidenceClass).toBe("RUNTIME_VISIBLE");
    expect(contract.observationClass).toBe("STATE_ECHO");
    expect(contract.measuresPaint).toBe(false);
    expect(contract.proposedBudget.ratificationStatus).toBe(
      "USER_RATIFICATION_PENDING",
    );
    expect(contract.safety.startsApplication).toBe(false);
  });

  test("visible execution fails closed before app launch without explicit approval", () => {
    const result = run([]);
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("refused before app launch");
    expect(result.stderr.toString()).toContain(
      "SCRIPT_KIT_ALLOW_VISIBLE_PROBES=1",
    );
    expect(result.stderr.toString()).not.toContain(
      "/nonexistent/must-never-launch-gpui",
    );
  });

  test("strict noninteractive mode refuses both visible opt-ins and hidden session startup", () => {
    for (const [args, overrides] of [
      [[], { SCRIPT_KIT_NONINTERACTIVE: "1", SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "1" }],
      [["--hidden-dry-run"], { SCRIPT_KIT_NONINTERACTIVE: "1" }],
    ] as Array<[string[], Record<string, string>]>) {
      const result = run(args, overrides);
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr.toString()).toContain("categorically refuses the root typing benchmark");
      expect(result.stderr.toString()).toContain("before app/session launch");
      expect(result.stderr.toString()).not.toContain("/nonexistent/must-never-launch-gpui");
    }
  });
});
