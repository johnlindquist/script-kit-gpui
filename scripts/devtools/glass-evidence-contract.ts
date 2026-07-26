import { createHash, randomUUID } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

export type EvidenceDisposition =
  | "EVALUABLE_PASS"
  | "EVALUABLE_FAIL"
  | "INVALID_INTERFERENCE"
  | "INVALID_OBSERVER"
  | "INVALID_SETUP"
  | "BLOCKED_ENVIRONMENT";

export type EvidenceIdentity = {
  runId: string;
  gitCommit: string;
  binary: string;
  binarySha256: string;
};

export const sha256File = (path: string) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");

export const newRunId = () => `glass-${randomUUID()}`;

export function assertFreshOutputDirectory(path: string) {
  if (existsSync(path) && readdirSync(path).length > 0) {
    throw new Error(`refusing non-empty evidence directory: ${resolve(path)}`);
  }
}

export function identityFromEnvironment(
  fallback: EvidenceIdentity,
): EvidenceIdentity {
  return {
    runId: process.env.SCRIPT_KIT_GLASS_RUN_ID ?? fallback.runId,
    gitCommit: process.env.SCRIPT_KIT_GLASS_GIT_COMMIT ?? fallback.gitCommit,
    binary: resolve(process.env.SCRIPT_KIT_GLASS_BINARY ?? fallback.binary),
    binarySha256:
      process.env.SCRIPT_KIT_GLASS_BINARY_SHA256 ?? fallback.binarySha256,
  };
}

export function validateChildReceipt(
  receipt: any,
  expected: EvidenceIdentity,
  expectedScenario: string,
  exitCode: number,
): string[] {
  const errors: string[] = [];
  if (!receipt || typeof receipt !== "object") return ["receipt missing"];
  if (receipt.schemaVersion !== 2) errors.push("schemaVersion must be 2");
  if (receipt.runId !== expected.runId) errors.push("runId mismatch");
  if (receipt.gitCommit !== expected.gitCommit) errors.push("gitCommit mismatch");
  if (resolve(receipt.binary ?? "") !== resolve(expected.binary)) {
    errors.push("binary path mismatch");
  }
  if (receipt.binarySha256 !== expected.binarySha256) {
    errors.push("binary SHA-256 mismatch");
  }
  if (receipt.scenario !== expectedScenario) errors.push("scenario mismatch");
  if (!receipt.startedAt || !receipt.finishedAt) errors.push("timestamps missing");
  if (!receipt.disposition) errors.push("disposition missing");
  if (exitCode !== 0 && receipt.pass === true) {
    errors.push("nonzero child exit cannot be a pass");
  }
  if (exitCode === 0 && receipt.pass !== true) {
    errors.push("zero child exit requires pass=true");
  }
  if (!(Number(receipt.pid) > 0)) errors.push("child PID missing");
  if (receipt.disposition !== "EVALUABLE_PASS" && receipt.pass === true) {
    errors.push("passing child must be EVALUABLE_PASS");
  }
  if (expectedScenario === "main-window") {
    if (receipt.visualMatrix?.states?.length !== 4) {
      errors.push("main-window visual matrix must contain exactly four states");
    }
    if (receipt.widthMatrix?.rows?.length !== 6) {
      errors.push("main-window width matrix must contain exactly six rows");
    }
    if (receipt.initialCompleteNativeInventory?.pass !== true) {
      errors.push("main-window initial complete topology missing");
    }
    if (receipt.finalCompleteNativeInventory?.pass !== true) {
      errors.push("main-window final complete topology missing");
    }
  } else if (expectedScenario.startsWith("locked:")) {
    if (receipt.stationary?.pass !== true) {
      errors.push("locked treatment stationary proof missing");
    }
  } else if (expectedScenario === "rapid-toggle") {
    if (
      !["actions", "notes", "dictation"].every(
        (name) => receipt.phases?.[name]?.pass === true,
      )
    ) {
      errors.push("rapid-toggle exact phase set missing or failed");
    }
    if (receipt.initialNativeInventory?.topology?.pass !== true) {
      errors.push("rapid-toggle complete topology missing");
    }
    if (receipt.interference?.receipt == null) {
      errors.push("rapid-toggle interference telemetry missing");
    }
  } else if (expectedScenario === "notes-fallback") {
    if (receipt.hostClockTiming?.ordered !== true) {
      errors.push("notes fallback shared-host-clock timing missing");
    }
    if (receipt.interference?.receipt == null) {
      errors.push("notes fallback interference telemetry missing");
    }
  } else if (expectedScenario.startsWith("lifecycle")) {
    const required = [
      "main-exit",
      "main-entry",
      "notes-entry",
      "notes-close-before-settle-reopen",
      "dictation-exit-reopen",
    ];
    errors.push(...validateUniqueScenarioSet(
      (receipt.scenarios ?? []).map((scenario: any) => scenario?.name),
      required,
    ).map((error) => `lifecycle ${error}`));
    if (
      !(receipt.scenarios ?? []).every(
        (scenario: any) =>
          scenario?.filmstrip?.receipt?.captureHealthPass === true
          && scenario?.filmstrip?.pass === true,
      )
    ) {
      errors.push("lifecycle complete capture health missing");
    }
    if (receipt.initialNativeTopology?.pass !== true) {
      errors.push("lifecycle complete initial topology missing");
    }
    if (receipt.interference?.receipt == null) {
      errors.push("lifecycle interference telemetry missing");
    }
  }
  return errors;
}

export function validateUniqueScenarioSet(
  observed: string[],
  required: string[],
): string[] {
  const errors: string[] = [];
  for (const name of required) {
    const count = observed.filter((value) => value === name).length;
    if (count !== 1) errors.push(`${name}: expected exactly one, observed ${count}`);
  }
  for (const name of new Set(observed)) {
    if (!required.includes(name)) errors.push(`unexpected scenario: ${name}`);
  }
  return errors;
}

export function aggregateDisposition(
  children: Array<{ disposition?: EvidenceDisposition; pass?: boolean }>,
  setupErrors: string[] = [],
): EvidenceDisposition {
  if (setupErrors.length > 0) return "INVALID_SETUP";
  const dispositions = children.map((child) => child.disposition);
  if (dispositions.includes("INVALID_INTERFERENCE")) return "INVALID_INTERFERENCE";
  if (dispositions.includes("INVALID_OBSERVER")) return "INVALID_OBSERVER";
  if (dispositions.includes("INVALID_SETUP")) return "INVALID_SETUP";
  if (dispositions.includes("BLOCKED_ENVIRONMENT")) return "BLOCKED_ENVIRONMENT";
  if (
    children.length === 0
    || children.some((child) =>
      child.pass !== true || child.disposition !== "EVALUABLE_PASS"
    )
  ) {
    return "EVALUABLE_FAIL";
  }
  return "EVALUABLE_PASS";
}

export function compositeEvaluator(
  pass: boolean,
  observerFailed: boolean,
): { pass: boolean; disposition: EvidenceDisposition } {
  return {
    pass,
    disposition: pass
      ? "EVALUABLE_PASS"
      : observerFailed
      ? "INVALID_OBSERVER"
      : "EVALUABLE_FAIL",
  };
}

/**
 * WP9 (glass-smoke-harness-max-info): validate that an already-captured
 * lifecycle receipt may be REUSED as evidence by another probe instead of
 * duplicating the capture.
 *
 * Fail-closed on every axis: a reused receipt is accepted only when the
 * binary SHA, theme-fixture SHA, background-fixture identity, display
 * identity, exact scenario set, capture health, interference, helper
 * hashes, and every frame hash all match. A field the caller requires that
 * the receipt does not carry is a MISMATCH, never a silent pass — and
 * INVALID_INTERFERENCE imported evidence stays invalid.
 */
export type ArtifactReferenceExpectation = {
  binarySha256: string;
  themeFixtureSha256?: string;
  backgroundFixtureMode?: string;
  backgroundFixtureConfigurationSha256?: string;
  displayId?: number;
  refreshRateHz?: number;
  backingScale?: number;
  requiredScenarioNames: string[];
  helperSha256?: string;
};

export function validateArtifactReference(
  receipt: any,
  expected: ArtifactReferenceExpectation,
  options: {
    /** Re-hash a referenced frame on disk. Return null when unreadable. */
    hashFile?: (path: string) => string | null;
  } = {},
): string[] {
  const errors: string[] = [];
  if (!receipt || typeof receipt !== "object") {
    return ["imported lifecycle receipt is missing or not an object"];
  }
  const expect = (
    label: string,
    actual: unknown,
    wanted: unknown,
  ) => {
    if (wanted === undefined) return;
    if (actual == null) {
      errors.push(`${label}: receipt does not carry the field (required ${wanted})`);
    } else if (actual !== wanted) {
      errors.push(`${label}: receipt has ${actual}, expected ${wanted}`);
    }
  };
  expect("binarySha256", receipt.binarySha256, expected.binarySha256);
  expect(
    "themeFixtureSha256",
    receipt.themeFixture?.sha256,
    expected.themeFixtureSha256,
  );
  expect(
    "backgroundFixtureMode",
    receipt.backgroundFixture?.mode,
    expected.backgroundFixtureMode,
  );
  expect(
    "backgroundFixtureConfigurationSha256",
    receipt.backgroundFixture?.configurationSha256,
    expected.backgroundFixtureConfigurationSha256,
  );
  expect(
    "displayId",
    receipt.backgroundFixture?.displayID ?? receipt.displayId,
    expected.displayId,
  );
  expect("helperSha256", receipt.helperSha256, expected.helperSha256);

  const scenarios: any[] = Array.isArray(receipt.scenarios)
    ? receipt.scenarios
    : [];
  errors.push(
    ...validateUniqueScenarioSet(
      scenarios.map((scenario) => String(scenario?.name)),
      expected.requiredScenarioNames,
    ).map((error) => `scenario set: ${error}`),
  );
  if (receipt.interference?.pass !== true) {
    errors.push(
      `interference: imported evidence is not interference-clean (disposition ${receipt.interference?.disposition ?? "missing"})`,
    );
  }
  for (const scenario of scenarios) {
    const filmstripReceipt = scenario?.filmstrip?.receipt;
    if (filmstripReceipt?.captureHealthPass !== true) {
      errors.push(`${scenario?.name}: captureHealthPass is not true`);
    }
    if (expected.refreshRateHz !== undefined) {
      expect(
        `${scenario?.name}: refreshRateHz`,
        filmstripReceipt?.refreshRateHz,
        expected.refreshRateHz,
      );
    }
    if (expected.backingScale !== undefined) {
      expect(
        `${scenario?.name}: backingScale`,
        filmstripReceipt?.backingScale ?? receipt.backingScale,
        expected.backingScale,
      );
    }
    if (options.hashFile) {
      for (const frame of filmstripReceipt?.frames ?? []) {
        const actual = options.hashFile(String(frame?.path ?? ""));
        if (actual === null) {
          errors.push(`${scenario?.name}: frame missing on disk: ${frame?.path}`);
        } else if (actual !== frame?.sha256) {
          errors.push(
            `${scenario?.name}: frame hash mismatch for ${frame?.path}`,
          );
        }
      }
    }
  }
  return errors;
}
