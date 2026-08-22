import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { basename, join, resolve } from "node:path";
import {
  GENERATED_BYTE_COMPARE_OUTPUT_PATHS,
  GENERATED_BYTE_COMPARE_SOURCE_PATHS,
  generateAuthoritativeByteComparison,
  parseGeneratedByteCompareArgs,
  validateGeneratedByteCompareEnvironment,
  validateGeneratedByteCompareReceipt,
  type ExporterProcessResult,
  type GeneratedByteCompareDependencies,
} from "./generated-byte-compare.ts";

const repositoryRoot = "/synthetic/exporter-repository";
const sourceSha = "a".repeat(40);
const binaryPath = "target-agent/pools/agent-debug/debug/export_design_tokens";
const absoluteBinary = resolve(repositoryRoot, binaryPath);
const environment = {
  SCRIPT_KIT_NONINTERACTIVE: "1",
  SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: "0",
  SCRIPT_KIT_ALLOW_VISIBLE_PROBES: "0",
  SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER: "0",
  SCRIPT_KIT_ALLOW_LIVE_AI: "0",
};

function fixture() {
  const files = new Map<string, Uint8Array>();
  files.set(absoluteBinary, Buffer.from("actual prebuilt exporter bytes"));
  for (const path of GENERATED_BYTE_COMPARE_SOURCE_PATHS) {
    files.set(
      resolve(repositoryRoot, path),
      Buffer.from("actual source bytes for " + path),
    );
  }
  for (const path of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
    files.set(
      resolve(repositoryRoot, path),
      Buffer.from("actual checked-in exporter bytes for " + path),
    );
  }

  let processStarts = 0;
  let currentSha = sourceSha;
  let tempExists = false;
  let cleanupCalls = 0;
  const temporaryDirectory = "/tmp/script-kit-exporter-byte-fixture";
  const calls: Array<{
    binary: string;
    arguments_: readonly string[];
    environment: Record<string, string | undefined>;
  }> = [];
  let result: ExporterProcessResult = {
    status: 0,
    stdout: Buffer.from("wrote exporter outputs"),
    stderr: Buffer.alloc(0),
  };
  let produceOutputs = true;
  let onRun: (() => void) | undefined;

  const dependencies: GeneratedByteCompareDependencies = {
    repositoryRoot,
    environment,
    readFile(path) {
      const bytes = files.get(path);
      if (bytes === undefined) throw new Error("missing virtual file " + path);
      return bytes;
    },
    resolveRealPath(path) {
      if (!files.has(path)) throw new Error("missing virtual binary " + path);
      return path;
    },
    fileStats(path) {
      const bytes = files.get(path);
      if (!bytes) throw new Error("missing virtual stat " + path);
      return {
        isFile: () => true,
        mode: 0o100755,
        size: bytes.byteLength,
      };
    },
    createTemporaryDirectory() {
      tempExists = true;
      return temporaryDirectory;
    },
    removeTemporaryDirectory(path) {
      expect(path).toBe(temporaryDirectory);
      cleanupCalls += 1;
      tempExists = false;
      for (const output of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
        files.delete(join(path, basename(output)));
      }
    },
    pathExists(path) {
      return path === temporaryDirectory && tempExists;
    },
    currentSourceSha: () => currentSha,
    runExporter(binary, arguments_, childEnvironment) {
      processStarts += 1;
      calls.push({ binary, arguments_, environment: childEnvironment });
      if (produceOutputs) {
        for (const output of GENERATED_BYTE_COMPARE_OUTPUT_PATHS) {
          files.set(
            join(temporaryDirectory, basename(output)),
            files.get(resolve(repositoryRoot, output))!,
          );
        }
      }
      onRun?.();
      return result;
    },
  };

  return {
    files,
    dependencies,
    calls,
    temporaryDirectory,
    get processStarts() { return processStarts; },
    get cleanupCalls() { return cleanupCalls; },
    get tempExists() { return tempExists; },
    setCurrentSha(value: string) { currentSha = value; },
    setResult(value: ExporterProcessResult) { result = value; },
    setProduceOutputs(value: boolean) { produceOutputs = value; },
    onRun(callback: () => void) { onRun = callback; },
    run(overrides: Partial<{ binaryPath: string; sourceSha: string }> = {}) {
      return generateAuthoritativeByteComparison(
        { binaryPath, sourceSha, ...overrides },
        dependencies,
      );
    },
  };
}

function currentIdentity(sandbox: ReturnType<typeof fixture>) {
  return {
    currentSourceSha: sourceSha,
    currentFileSha256(path: string) {
      const bytes = sandbox.files.get(resolve(repositoryRoot, path));
      return bytes
        ? createHash("sha256").update(bytes).digest("hex")
        : null;
    },
  };
}

describe("authoritative non-GUI generated-token byte comparison", () => {
  test("binds actual exporter, current source, and exact JSON/CSS bytes", () => {
    const sandbox = fixture();
    const receipt = sandbox.run();
    expect(sandbox.processStarts).toBe(1);
    expect(sandbox.calls[0]?.binary).toBe(absoluteBinary);
    expect(sandbox.calls[0]?.arguments_).toEqual([sandbox.temporaryDirectory]);
    expect(receipt.evidenceClass).toBe("UNIT_BEHAVIOR");
    expect(receipt.provesRuntimeBehavior).toBe(false);
    expect(receipt.byteEqual).toBe(true);
    expect(receipt.handEditedGeneratedOutput).toBe(false);
    expect(receipt.sourceSha).toBe(sourceSha);
    expect(receipt.binary.path).toBe(binaryPath);
    expect(receipt.binary.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(Object.keys(receipt.sourceFingerprints)).toEqual(
      [...GENERATED_BYTE_COMPARE_SOURCE_PATHS],
    );
    expect(Object.keys(receipt.outputHashes)).toEqual(
      [...GENERATED_BYTE_COMPARE_OUTPUT_PATHS],
    );
    expect(receipt.generatedOutputHashes).toEqual(receipt.outputHashes);
    expect(receipt.outputs).toHaveLength(2);
    expect(receipt.safety).toMatchObject({
      noninteractive: true,
      startsApplication: false,
      revealsWindow: false,
      focusesWindow: false,
      drivesNativeInput: false,
      capturesScreen: false,
      accessesNetwork: false,
      usesLiveAi: false,
      startsExporter: true,
      isolatedTempOutput: true,
    });
    expect(receipt.cleanup.closed).toBe(true);
    expect(sandbox.cleanupCalls).toBe(1);
    expect(sandbox.tempExists).toBe(false);
    expect(
      validateGeneratedByteCompareReceipt(
        receipt,
        currentIdentity(sandbox),
      ),
    ).toEqual({ pass: true, errors: [] });
  });

  test.each([
    "SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH",
    "SCRIPT_KIT_ALLOW_VISIBLE_PROBES",
    "SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER",
    "SCRIPT_KIT_ALLOW_LIVE_AI",
    "SCRIPT_KIT_ALLOW_NATIVE_INPUT",
    "SCRIPT_KIT_ALLOW_SCREEN_CAPTURE",
  ])("refuses unsafe %s before invoking any executable", (setting) => {
    const sandbox = fixture();
    sandbox.dependencies.environment = { ...environment, [setting]: "1" };
    expect(() => sandbox.run()).toThrow(setting);
    expect(sandbox.processStarts).toBe(0);
    expect(sandbox.cleanupCalls).toBe(0);
  });

  test("noninteractive mode is mandatory before any exporter process", () => {
    const sandbox = fixture();
    sandbox.dependencies.environment = {
      ...environment,
      SCRIPT_KIT_NONINTERACTIVE: "0",
    };
    expect(() => sandbox.run()).toThrow("SCRIPT_KIT_NONINTERACTIVE=1");
    expect(sandbox.processStarts).toBe(0);
  });

  test("missing, wrong, external, or non-executable binary cannot start", () => {
    for (const invalidPath of [
      "",
      "target/debug/script-kit-gpui",
      "target/debug/export_design_tokens",
      "/outside/export_design_tokens",
    ]) {
      const sandbox = fixture();
      expect(() => sandbox.run({ binaryPath: invalidPath })).toThrow();
      expect(sandbox.processStarts).toBe(0);
    }
    const nonExecutable = fixture();
    nonExecutable.dependencies.fileStats = (path) => ({
      isFile: () => true,
      mode: 0o100644,
      size: nonExecutable.files.get(path)?.byteLength ?? 0,
    });
    expect(() => nonExecutable.run()).toThrow("executable regular file");
    expect(nonExecutable.processStarts).toBe(0);
  });

  test("malformed or stale source commit fails before exporter execution", () => {
    const malformed = fixture();
    expect(() => malformed.run({ sourceSha: "not-a-commit" })).toThrow("--source-sha");
    expect(malformed.processStarts).toBe(0);
    const stale = fixture();
    stale.setCurrentSha("b".repeat(40));
    expect(() => stale.run()).toThrow("current checkout");
    expect(stale.processStarts).toBe(0);
  });

  test("missing named exporter source or checked-in artifact fails before execution", () => {
    for (const path of [
      GENERATED_BYTE_COMPARE_SOURCE_PATHS[1],
      GENERATED_BYTE_COMPARE_OUTPUT_PATHS[0],
    ]) {
      const sandbox = fixture();
      sandbox.files.delete(resolve(repositoryRoot, path));
      expect(() => sandbox.run()).toThrow(path);
      expect(sandbox.processStarts).toBe(0);
    }
  });

  test("exporter crash still closes its isolated temporary directory", () => {
    const sandbox = fixture();
    sandbox.setResult({ status: 3, stderr: Buffer.from("failure") });
    expect(() => sandbox.run()).toThrow("exporter failed");
    expect(sandbox.processStarts).toBe(1);
    expect(sandbox.cleanupCalls).toBe(1);
    expect(sandbox.tempExists).toBe(false);
  });

  test.each(GENERATED_BYTE_COMPARE_OUTPUT_PATHS)(
    "byte mismatch in %s can never produce a green receipt",
    (path) => {
      const sandbox = fixture();
      sandbox.onRun(() => {
        sandbox.files.set(
          join(sandbox.temporaryDirectory, basename(path)),
          Buffer.from("different actual exporter output"),
        );
      });
      expect(() => sandbox.run()).toThrow(path);
      expect(sandbox.cleanupCalls).toBe(1);
    },
  );

  test.each(GENERATED_BYTE_COMPARE_OUTPUT_PATHS)(
    "missing generated %s fails and cleans up",
    (path) => {
      const sandbox = fixture();
      sandbox.onRun(() => {
        sandbox.files.delete(join(sandbox.temporaryDirectory, basename(path)));
      });
      expect(() => sandbox.run()).toThrow(basename(path));
      expect(sandbox.cleanupCalls).toBe(1);
    },
  );

  test("the exporter cannot mutate checked-in outputs while claiming equality", () => {
    const sandbox = fixture();
    const path = GENERATED_BYTE_COMPARE_OUTPUT_PATHS[0];
    sandbox.onRun(() => {
      sandbox.files.set(
        resolve(repositoryRoot, path),
        Buffer.from("hand-edited checked-in output"),
      );
    });
    expect(() => sandbox.run()).toThrow("changed checked-in generated output");
    expect(sandbox.cleanupCalls).toBe(1);
  });

  test("binary and source identity cannot change during exporter execution", () => {
    const changedBinary = fixture();
    changedBinary.onRun(() => {
      changedBinary.files.set(absoluteBinary, Buffer.from("swapped exporter"));
    });
    expect(() => changedBinary.run()).toThrow("binary changed");
    expect(changedBinary.cleanupCalls).toBe(1);

    const changedSource = fixture();
    changedSource.onRun(() => {
      changedSource.files.set(
        resolve(repositoryRoot, GENERATED_BYTE_COMPARE_SOURCE_PATHS[1]),
        Buffer.from("changed exporter source"),
      );
    });
    expect(() => changedSource.run()).toThrow("source owner changed");
    expect(changedSource.cleanupCalls).toBe(1);

    const changedCommit = fixture();
    changedCommit.onRun(() => changedCommit.setCurrentSha("b".repeat(40)));
    expect(() => changedCommit.run()).toThrow("source commit changed");
    expect(changedCommit.cleanupCalls).toBe(1);
  });

  test("failed cleanup blocks otherwise matching exporter bytes", () => {
    const sandbox = fixture();
    sandbox.dependencies.removeTemporaryDirectory = () => {};
    expect(() => sandbox.run()).toThrow("survived cleanup");
  });

  test("repository-local output directories cannot be handed to the exporter", () => {
    const sandbox = fixture();
    sandbox.dependencies.createTemporaryDirectory = () =>
      resolve(repositoryRoot, ".artifacts/unsafe-exporter-output");
    sandbox.dependencies.removeTemporaryDirectory = () => {};
    expect(() => sandbox.run()).toThrow("isolated external temporary directory");
    expect(sandbox.processStarts).toBe(0);
  });

  test("CLI requires explicit binary/source identity and rejects duplicate flags", () => {
    expect(parseGeneratedByteCompareArgs([
      "--binary",
      binaryPath,
      "--source-sha",
      sourceSha,
    ])).toEqual({ binaryPath, sourceSha, outputPath: undefined });
    for (const invalid of [
      [],
      ["--binary", binaryPath],
      ["--source-sha", sourceSha],
      ["--binary", binaryPath, "--binary", binaryPath, "--source-sha", sourceSha],
      ["--binary", binaryPath, "--source-sha"],
      ["--binary", binaryPath, "--source-sha", sourceSha, "--unknown", "x"],
    ]) {
      expect(() => parseGeneratedByteCompareArgs(invalid)).toThrow();
    }
  });

  test("environment contract exposes exact fail-closed operator settings", () => {
    expect(validateGeneratedByteCompareEnvironment(environment)).toEqual([]);
    expect(
      validateGeneratedByteCompareEnvironment({
        ...environment,
        SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH: undefined,
      }),
    ).toContain("SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH must be disabled");
  });

  test("receipt validation rejects stale source commit, owner, and executable bytes", () => {
    const sandbox = fixture();
    const receipt = sandbox.run();
    expect(
      validateGeneratedByteCompareReceipt(
        { ...receipt, sourceSha: "b".repeat(40) },
        currentIdentity(sandbox),
      ).errors,
    ).toContain(
      "exporter receipt source commit differs from the current checkout",
    );

    const sourcePath = GENERATED_BYTE_COMPARE_SOURCE_PATHS[1];
    sandbox.files.set(
      resolve(repositoryRoot, sourcePath),
      Buffer.from("stale exporter owner"),
    );
    expect(
      validateGeneratedByteCompareReceipt(
        receipt,
        currentIdentity(sandbox),
      ).errors,
    ).toContain("stale exporter source fingerprint: " + sourcePath);

    sandbox.files.set(absoluteBinary, Buffer.from("stale exporter binary"));
    expect(
      validateGeneratedByteCompareReceipt(
        receipt,
        currentIdentity(sandbox),
      ).errors,
    ).toContain(
      "exporter binary fingerprint no longer matches the proven executable",
    );
  });

  test("one generated output or a forged matching flag cannot satisfy byte proof", () => {
    const sandbox = fixture();
    const receipt = sandbox.run();
    const cssPath = GENERATED_BYTE_COMPARE_OUTPUT_PATHS[1];
    const partial = {
      ...receipt,
      outputHashes: { ...receipt.outputHashes },
      generatedOutputHashes: { ...receipt.generatedOutputHashes },
      outputs: receipt.outputs.slice(0, 1),
    };
    delete partial.outputHashes[cssPath];
    delete partial.generatedOutputHashes[cssPath];
    const errors = validateGeneratedByteCompareReceipt(
      partial,
      currentIdentity(sandbox),
    ).errors;
    expect(errors).toContain(
      "checked-in output hashes must contain exactly tokens.json and tokens.css",
    );
    expect(errors).toContain(
      "generated output hashes must contain exactly tokens.json and tokens.css",
    );
    expect(errors).toContain(
      "exporter receipt requires exactly two distinct output observations",
    );
  });

  test("current checked-in outputs must still match both proven exporter hashes", () => {
    const sandbox = fixture();
    const receipt = sandbox.run();
    const path = GENERATED_BYTE_COMPARE_OUTPUT_PATHS[0];
    sandbox.files.set(
      resolve(repositoryRoot, path),
      Buffer.from("subsequently hand-edited output"),
    );
    expect(
      validateGeneratedByteCompareReceipt(
        receipt,
        currentIdentity(sandbox),
      ).errors,
    ).toContain(
      "checked-in generated output changed after comparison: " + path,
    );
  });

  test("app-runtime claims, false safety, failed execution, and cleanup fail closed", () => {
    const sandbox = fixture();
    const receipt = sandbox.run();
    for (const invalid of [
      { ...receipt, evidenceClass: "RUNTIME_HIDDEN" },
      { ...receipt, provesRuntimeBehavior: true },
      {
        ...receipt,
        safety: { ...receipt.safety, startsApplication: true },
      },
      {
        ...receipt,
        execution: { ...receipt.execution, exitCode: 3 },
      },
      {
        ...receipt,
        cleanup: { closed: false, survivors: ["exporter"] },
      },
    ]) {
      expect(
        validateGeneratedByteCompareReceipt(
          invalid,
          currentIdentity(sandbox),
        ).pass,
      ).toBe(false);
    }
  });

  test("untrusted receipt binary paths can never escape the repository", () => {
    const sandbox = fixture();
    const receipt = sandbox.run();
    for (const path of [
      "/outside/export_design_tokens",
      "../outside/export_design_tokens",
      "target/../export_design_tokens",
      "target/script-kit-gpui",
    ]) {
      expect(
        validateGeneratedByteCompareReceipt({
          ...receipt,
          binary: { ...receipt.binary, path },
        }).errors,
      ).toContain(
        "exporter binary identity is missing, malformed, or outside the repository",
      );
    }
  });
});
