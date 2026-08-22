import { describe, expect, test } from "bun:test";

const benchmark = "scripts/agentic/root-search-frame-stability.ts";

function inspect(args: string[], environment: Record<string, string> = {}) {
  return Bun.spawnSync(["bun", benchmark, ...args], {
    env: {
      ...process.env,
      CI: "false",
      SCRIPT_KIT_GPUI_BINARY: "/nonexistent/must-never-launch-gpui",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
      SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
      SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
      SCRIPT_KIT_ALLOW_LIVE_AI: "0",
      ...environment,
    },
    stdout: "pipe",
    stderr: "pipe",
  });
}

describe("hidden semantic frame stability proof contract", () => {
  test("help remains safe without a binary or output receipt", () => {
    const result = inspect(["--help"]);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString()).toContain("--describe-contract");
  });

  test("contract explicitly refuses visible, focused, native-input, or painted claims", () => {
    const result = inspect(["--describe-contract"]);
    expect(result.exitCode).toBe(0);
    const contract = JSON.parse(result.stdout.toString());
    expect(contract.evidenceClass).toBe("STATIC_INVENTORY");
    expect(contract.runtimeEvidenceClass).toBe("RUNTIME_HIDDEN");
    expect(contract.metricKind).toBe("semantic_frame_identity");
    expect(contract.observationClass).toBe("SEMANTIC_FRAME");
    expect(contract.measuresPaint).toBe(false);
    expect(contract.safety).toEqual({
      startsApplication: false,
      runtimeStartsApplication: true,
      runtimeRequiresSandboxHome: true,
      runtimeRequiresHiddenWindow: true,
      runtimeRequiresNoninteractive: true,
      runtimeRequiresCiEnvironment: true,
      runtimeRequiresIsolatedAppLaunchOptIn:
        "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1",
      revealsWindow: false,
      focusesWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
    });
  });

  test("runtime refuses before app launch unless the strict noninteractive boundary is active", () => {
    const result = inspect([], { SCRIPT_KIT_NONINTERACTIVE: "0" });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("refused before app launch");
    expect(result.stderr.toString()).toContain("SCRIPT_KIT_NONINTERACTIVE=1 is required");
    expect(result.stderr.toString()).not.toContain("/nonexistent/must-never-launch-gpui");
  });

  test("strict local execution cannot launch the app without isolated CI authorization", () => {
    const result = inspect([], { SCRIPT_KIT_NONINTERACTIVE: "1" });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("refused before app launch");
    expect(result.stderr.toString()).toContain(
      "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=1 is required",
    );
    expect(result.stderr.toString()).not.toContain("--binary and --receipt are required");
  });

  test("an isolated-launch opt-in cannot turn an operator-local shell into CI", () => {
    const result = inspect([], {
      SCRIPT_KIT_NONINTERACTIVE: "1",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1",
      CI: "false",
    });
    expect(result.exitCode).not.toBe(0);
    expect(result.stderr.toString()).toContain("CI=true is required");
    expect(result.stderr.toString()).not.toContain("--binary and --receipt are required");
  });

  test("contradictory visible, capture, and live-AI opt-ins fail before launch", () => {
    for (const unsafeSetting of [
      "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
      "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
      "SCRIPT_KIT_ALLOW_LIVE_AI",
    ]) {
      const result = inspect([], {
        SCRIPT_KIT_NONINTERACTIVE: "1",
        SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "1",
        CI: "true",
        [unsafeSetting]: "1",
      });
      expect(result.exitCode).not.toBe(0);
      expect(result.stderr.toString()).toContain(
        `${unsafeSetting}=1 contradicts noninteractive execution`,
      );
      expect(result.stderr.toString()).not.toContain("--binary and --receipt are required");
    }
  });
});
