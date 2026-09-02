import { beforeAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";

const repositoryRoot = realpathSync(fileURLToPath(new URL("../..", import.meta.url)));
const wrapper = "scripts/agentic/agent-cargo.sh";
const productionTests = [
  "alpha_byte_is_one_byte",
  "authored_preserves_all_u8_values",
  "from_normalized_clamps",
  "from_normalized_preserves_existing_quantization",
  "rounded_quantization_is_distinct_and_explicit",
  "pack_rgb_alpha_preserves_channel_order",
];
interface CargoContractRun {
  result: { passedTests: number };
  output: string;
}
let unitRun: CargoContractRun;
let compilerRun: CargoContractRun;

function runCargoContract(args: string[]) {
  // The wrapper owns the pinned toolchain, shared compiler lease, source
  // identity, process supervision and cleanup. Never invoke rustc directly.
  const execution = spawnSync("bash", [wrapper, ...args], {
    cwd: repositoryRoot,
    encoding: "utf8",
    timeout: 660_000,
    maxBuffer: 16 * 1024 * 1024,
    env: {
      ...process.env,
      SCRIPT_KIT_REPO_ROOT: repositoryRoot,
      SCRIPT_KIT_NONINTERACTIVE: "1",
      SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
      SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
      SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
      SCRIPT_KIT_ALLOW_LIVE_AI: "0",
      SCRIPT_KIT_ALLOW_NATIVE_INPUT: "0",
      SCRIPT_KIT_ALLOW_SCREEN_CAPTURE: "0",
      SCRIPT_KIT_AGENT_ARTIFACT_KIND: "",
      SCRIPT_KIT_AGENT_RESULT_PATH: "",
      SCRIPT_KIT_AGENT_TIMEOUT_MS: "600000",
    },
  });
  if (execution.error || execution.status !== 0) {
    throw new Error(`canonical AlphaByte Cargo contract failed: ${execution.error ?? execution.status}\n${execution.stderr}`);
  }
  const result = JSON.parse(execution.stdout);
  expect(result).toMatchObject({
    status: "succeeded",
    exitCode: 0,
    failedTests: 0,
    cleanup: {
      closed: true,
      processExited: true,
      processGroupExited: true,
      streamsDrained: true,
      referencesFinalized: true,
      survivors: [],
      failureCodes: [],
    },
  });
  expect(result.testSummaries).toBeGreaterThan(0);
  expect(result.task.kind).toBe("build-job");
  // Raw Cargo output is retained on stderr; stdout is the owned task result.
  return { result, output: execution.stderr };
}

beforeAll(() => {
  unitRun = runCargoContract([
    "test", "--locked", "--lib", "theme::alpha::alpha_byte_tests::",
  ]);
  compilerRun = runCargoContract([
    "test", "--locked", "--doc", "--package", "script-kit-gpui", "theme::alpha::",
  ]);
}, 1_320_000);

describe("real production authored-alpha unit and compiler contracts", () => {
  test("canonical Cargo executes every current production AlphaByte behavior test", () => {
    expect(unitRun.result.passedTests).toBe(productionTests.length);
    for (const name of productionTests) {
      expect(unitRun.output).toContain(`theme::alpha::alpha_byte_tests::${name} ... ok`);
    }
    expect(unitRun.output).toContain(`${productionTests.length} passed; 0 failed; 0 ignored`);
    expect(compilerRun.result.passedTests).toBe(3);
    expect(compilerRun.output).toContain("3 passed; 0 failed; 0 ignored");
  });

  test("a raw normalized float cannot compile through the typed packer", () => {
    expect(compilerRun.output).toMatch(/theme::alpha::pack_rgb_alpha \(line \d+\) - compile fail \.\.\. ok/);
  });

  test("the authored-byte constructor cannot accept a floating opacity", () => {
    expect(compilerRun.output).toMatch(/theme::alpha::AlphaByte::authored \(line \d+\) - compile fail \.\.\. ok/);
  });

  test("private tuple storage cannot bypass explicit unit constructors", () => {
    expect(compilerRun.output).toMatch(/theme::alpha::AlphaByte \(line \d+\) - compile fail \.\.\. ok/);
  });
});
