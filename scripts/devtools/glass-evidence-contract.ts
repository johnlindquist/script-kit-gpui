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
