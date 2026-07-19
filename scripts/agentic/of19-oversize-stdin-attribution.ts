#!/usr/bin/env bun
/** OF-19 attribution: oversized stdin line retention and response behavior. */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const root = resolve(import.meta.dir, "../..");
const binary =
  process.env.PROBE_BINARY ??
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(root, "target-agent/artifacts/chaos-brain-today/script-kit-gpui");
const outputDir = resolve(
  process.env.OF19_OUTPUT_DIR ??
    join(root, ".test-output", "of19-oversize-stdin"),
);
mkdirSync(outputDir, { recursive: true });

const payload = "X".repeat(20 * 1024);
const early: Json = {
  type: "setInput",
  requestId: "of19-id-early",
  text: payload,
};
const late: Json = {
  type: "setInput",
  text: payload,
  requestId: "of19-id-late",
};
const inputs = { early, late };
const serialized = Object.fromEntries(
  Object.entries(inputs).map(([name, command]) => {
    const requestId = String(command.requestId);
    const { requestId: _callerRequestId, ...rest } = command;
    return [name, JSON.stringify({ requestId, ...rest })];
  }),
) as Record<string, string>;
const cap = 16 * 1024;

const receipt: Json = {
  schemaVersion: 1,
  tool: "of19-oversize-stdin-attribution",
  binary,
  cap,
  serialized: Object.fromEntries(
    Object.entries(serialized).map(([name, line]) => [
      name,
      {
        inputRequestIdOffset: JSON.stringify(
          inputs[name as keyof typeof inputs],
        ).indexOf(`of19-id-${name}`),
        bytes: Buffer.byteLength(line),
        wireRequestIdOffset: line.indexOf(`of19-id-${name}`),
        retainedPrefixContainsRequestId: line
          .slice(0, cap)
          .includes(`of19-id-${name}`),
      },
    ]),
  ),
  attempts: [],
};

const driver = await Driver.launch({
  binary,
  sessionName: `of19-${process.pid}`,
  sandboxHome: true,
  readyTimeoutMs: 15_000,
  defaultTimeoutMs: 2_000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});
receipt.sessionDir = driver.sessionDir;

try {
  for (const [name, command] of [
    ["early", early],
    ["late", late],
  ] as const) {
    const started = performance.now();
    try {
      const response = await driver.request(command, { timeoutMs: 900 });
      receipt.attempts.push({
        name,
        outcome: "response",
        elapsedMs: performance.now() - started,
        response,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      receipt.attempts.push({
        name,
        outcome: message.includes("[line_too_long]")
          ? "typed_error"
          : "unexpected_error",
        elapsedMs: Math.round(performance.now() - started),
        error: message,
      });
    }
  }

  const recoveryStarted = performance.now();
  const recovery = await driver.request(
    { type: "getState", requestId: "of19-recovery" },
    { timeoutMs: 3_000 },
  );
  receipt.recovery = {
    outcome: "response",
    elapsedMs: Math.round(performance.now() - recoveryStarted),
    type: recovery.type,
    requestId: recovery.requestId,
  };
  await Bun.sleep(150);
  const appLog = readFileSync(driver.logPath, "utf8");
  receipt.logs = appLog
    .split("\n")
    .filter((line) =>
      /stdin_command_too_large|Skipping oversized external command/.test(line),
    )
    .map((line) => line.slice(0, 1_000));
  receipt.pass =
    receipt.attempts.every(
      (attempt: Json) =>
        attempt.outcome === "typed_error" && attempt.elapsedMs < 900,
    ) &&
    receipt.recovery.requestId === "of19-recovery" &&
    receipt.serialized.early.retainedPrefixContainsRequestId === true &&
    receipt.serialized.late.retainedPrefixContainsRequestId === true;
} finally {
  await driver.close();
}

const receiptPath = join(outputDir, "receipt.json");
writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(
  JSON.stringify(
    {
      pass: receipt.pass,
      receiptPath,
      attempts: receipt.attempts,
      recovery: receipt.recovery,
    },
    null,
    2,
  ),
);
process.exit(receipt.pass ? 0 : 1);
