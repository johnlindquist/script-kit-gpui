#!/usr/bin/env bun
/**
 * scripts/agentic/await-response.ts
 *
 * Await a stdin protocol response for a given requestId.
 * Prefers the dedicated protocol-responses.ndjson bus; optionally
 * falls back to app.log scraping when SCRIPT_KIT_ALLOW_LOG_RPC_FALLBACK=1.
 *
 * Usage:
 *   bun await-response.ts --session default --request-id agent_chat1 --expect agent_chatStateResult --timeout 3000
 *
 * Exit codes:
 *   0 = response found
 *   1 = response timeout
 *   2 = infrastructure error (missing session, bad args)
 *   3 = parse error detected preemptively (stdin_parse_failed for this requestId)
 */

import { existsSync, readFileSync, statSync } from "fs";
import { join } from "path";

const SCHEMA_VERSION = 1;
const SESSION_DIR =
  process.env.SCRIPT_KIT_SESSION_DIR ?? "/tmp/sk-agentic-sessions";
const DEFAULT_TIMEOUT = 5000;
const POLL_INTERVAL = 50; // ms between scans
const ALLOW_LOG_FALLBACK =
  process.env.SCRIPT_KIT_ALLOW_LOG_RPC_FALLBACK === "1";

const PARSE_ERROR_EXIT_CODE = 3;
const REQUEST_ID_CHARSET = /^[A-Za-z0-9_.:/-]+$/;
const ERROR_MSG_MAX_CHARS = 200;

// --- types -----------------------------------------------------------------

interface RpcResponse {
  schemaVersion: number;
  status: "ok";
  session: string;
  requestId: string;
  responseType: string;
  response: unknown;
}

interface RpcError {
  schemaVersion: number;
  status: "error";
  session: string;
  requestId: string;
  error: { code: string; message: string };
}

type RpcResult = RpcResponse | RpcError;

interface ProtocolBusEnvelope {
  kind?: string;
  requestId?: string;
  responseType?: string;
  response?: unknown;
}

async function printResult(result: RpcResult): Promise<void> {
  const output = `${JSON.stringify(result)}\n`;

  try {
    await new Promise<void>((resolve, reject) => {
      let settled = false;
      const settle = (error?: Error | null): void => {
        if (settled) return;
        settled = true;
        if (error) {
          reject(error);
        } else {
          resolve();
        }
      };
      const onError = (error: Error): void => {
        settle(error);
      };

      // A failed Writable write can invoke the callback and subsequently emit
      // "error". Keep a temporary listener so EPIPE cannot become an unhandled
      // stream error.
      process.stdout.once("error", onError);
      try {
        process.stdout.write(output, (error) => {
          if (!error) {
            process.stdout.off("error", onError);
          }
          // On error, leave the once-listener installed long enough to consume
          // the corresponding stream "error" event.
          settle(error);
        });
      } catch (error) {
        process.stdout.off("error", onError);
        settle(error instanceof Error ? error : new Error(String(error)));
      }
    });
  } catch (error) {
    const message = (error instanceof Error ? error.message : String(error))
      .replace(/\s+/g, " ")
      .slice(0, 200);

    // stdout cannot carry the result, so do not attempt another JSON envelope
    // there. stderr is diagnostic-only and deliberately best effort.
    try {
      process.stderr.write(`[await-response] stdout_write_failed: ${message}\n`);
    } catch {
      // stderr may also be closed.
    }
    process.exit(2);
  }
}

// --- helpers ---------------------------------------------------------------

function errorResult(
  session: string,
  requestId: string,
  code: string,
  message: string,
): RpcError {
  return {
    schemaVersion: SCHEMA_VERSION,
    status: "error",
    session,
    requestId,
    error: { code, message },
  };
}

function findJsonObjectEnd(text: string): number | null {
  let depth = 0;
  let inString = false;
  let escaped = false;

  for (let index = 0; index < text.length; index += 1) {
    const ch = text[index];

    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (ch === "\\") {
        escaped = true;
      } else if (ch === "\"") {
        inString = false;
      }
      continue;
    }

    if (ch === "\"") {
      inString = true;
    } else if (ch === "{") {
      depth += 1;
    } else if (ch === "}") {
      depth -= 1;
      if (depth === 0) {
        return index + 1;
      }
    }
  }

  return null;
}

function parseJsonResponseLine(trimmed: string): unknown | null {
  try {
    return JSON.parse(trimmed);
  } catch {
    const end = findJsonObjectEnd(trimmed);
    if (end == null) {
      return null;
    }

    try {
      return JSON.parse(trimmed.substring(0, end));
    } catch {
      return null;
    }
  }
}

function scanProtocolBus(
  responsesPath: string,
  requestId: string,
  expect: string | null,
  startOffset: number,
): [unknown | null, string | null, number] {
  if (!existsSync(responsesPath)) {
    return [null, null, startOffset];
  }

  let size: number;
  try {
    size = statSync(responsesPath).size;
  } catch {
    return [null, null, startOffset];
  }

  if (size < startOffset) {
    startOffset = 0;
  }
  if (size <= startOffset) {
    return [null, null, startOffset];
  }

  const buf = readFileSync(responsesPath);
  const newBytes = buf.subarray(startOffset);
  const lastNewline = newBytes.lastIndexOf(0x0a);
  if (lastNewline < 0) {
    return [null, null, startOffset];
  }

  const completeContent = newBytes.subarray(0, lastNewline + 1).toString("utf8");
  const newOffset = startOffset + lastNewline + 1;

  for (const line of completeContent.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;

    const parsed = parseJsonResponseLine(trimmed) as ProtocolBusEnvelope | null;
    if (parsed == null || typeof parsed !== "object") continue;
    if (parsed.kind !== "protocolResponse") continue;
    if (parsed.requestId !== requestId) continue;

    const responseType = parsed.responseType ?? null;
    if (expect && responseType !== expect) continue;

    const response = parsed.response ?? parsed;
    return [response, responseType, newOffset];
  }

  return [null, null, newOffset];
}

function scanLog(
  logPath: string,
  requestId: string,
  expect: string | null,
  startOffset: number,
): [unknown | null, string | null, number] {
  let size: number;
  try {
    size = statSync(logPath).size;
  } catch {
    return [null, null, startOffset];
  }

  if (size < startOffset) {
    startOffset = 0;
  }

  if (size <= startOffset) return [null, null, startOffset];

  const buf = readFileSync(logPath);
  const newBytes = buf.subarray(startOffset);
  const lastNewline = newBytes.lastIndexOf(0x0a);

  if (lastNewline < 0) {
    return [null, null, startOffset];
  }

  const completeContent = newBytes.subarray(0, lastNewline + 1).toString("utf8");
  const newOffset = startOffset + lastNewline + 1;

  for (const line of completeContent.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || !trimmed.startsWith("{")) continue;
    if (!trimmed.includes(requestId)) continue;

    const parsed = parseJsonResponseLine(trimmed);
    if (parsed == null || typeof parsed !== "object") continue;
    const response = parsed as { requestId?: string; type?: string; responseType?: string };
    if (response.requestId !== requestId) continue;

    const responseType = response.type ?? response.responseType ?? null;
    if (expect && responseType !== expect) continue;

    return [parsed, responseType, newOffset];
  }

  return [null, null, newOffset];
}

function scanForParseFailure(
  logPath: string,
  requestId: string,
  startOffset: number,
): [string | null, number] {
  let size: number;
  try {
    size = statSync(logPath).size;
  } catch {
    return [null, startOffset];
  }

  if (size < startOffset) {
    startOffset = 0;
  }

  if (size <= startOffset) return [null, startOffset];

  const buf = readFileSync(logPath);
  const newBytes = buf.subarray(startOffset);
  const lastNewline = newBytes.lastIndexOf(0x0a);

  if (lastNewline < 0) {
    return [null, startOffset];
  }

  const completeContent = newBytes.subarray(0, lastNewline + 1).toString("utf8");
  const newOffset = startOffset + lastNewline + 1;

  const cidMarker = `cid=stdin:req:${requestId} `;
  for (const line of completeContent.split("\n")) {
    if (!line.includes(cidMarker)) continue;
    if (!line.includes("event_type=stdin_parse_failed")) continue;
    const errIdx = line.indexOf(" error=");
    if (errIdx < 0) continue;
    let errMsg = line.substring(errIdx + " error=".length);
    errMsg = errMsg.replace(/ at line \d+ column \d+.*$/, "");
    if (errMsg.length > ERROR_MSG_MAX_CHARS) {
      errMsg = errMsg.substring(0, ERROR_MSG_MAX_CHARS);
    }
    return [errMsg, newOffset];
  }
  return [null, newOffset];
}

// --- main ------------------------------------------------------------------

const args = process.argv.slice(2);

function getArg(flag: string): string | null {
  const idx = args.indexOf(flag);
  return idx >= 0 && args[idx + 1] ? args[idx + 1] : null;
}

function parseNonNegativeInt(value: string | null, fallback: number): number {
  if (value == null) {
    return fallback;
  }

  const parsed = parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback;
}

const sessionName = getArg("--session") ?? "default";
const requestId = getArg("--request-id");
const expect = getArg("--expect");
const timeout = parseNonNegativeInt(getArg("--timeout"), DEFAULT_TIMEOUT);
const startOffset = parseNonNegativeInt(getArg("--start-offset"), 0);
const responsesPathArg = getArg("--responses-path");

if (!requestId) {
  const result = errorResult(
    sessionName,
    "",
    "missing_request_id",
    "Usage: await-response.ts --request-id ID [--expect TYPE] [--timeout MS]",
  );
  await printResult(result);
  process.exit(2);
}

const sdir = join(SESSION_DIR, sessionName);
const logPath = join(sdir, "app.log");
const responsesPath =
  responsesPathArg ?? join(sdir, "protocol-responses.ndjson");

if (!existsSync(sdir)) {
  const result = errorResult(
    sessionName,
    requestId,
    "no_session",
    `Session '${sessionName}' not found at ${sdir}`,
  );
  await printResult(result);
  process.exit(2);
}

const deadline = Date.now() + timeout;
let busOffset = startOffset;
let logOffset = startOffset;
let parseFailScanOffset = startOffset;
const charsetSafeRequestId = REQUEST_ID_CHARSET.test(requestId);

while (Date.now() < deadline) {
  if (charsetSafeRequestId && ALLOW_LOG_FALLBACK) {
    const [errMsg, newFailOffset] = scanForParseFailure(
      logPath,
      requestId,
      parseFailScanOffset,
    );
    parseFailScanOffset = newFailOffset;
    if (errMsg) {
      const parseErr = errorResult(
        sessionName,
        requestId,
        "parse_error",
        errMsg,
      );
      await printResult(parseErr);
      process.exit(PARSE_ERROR_EXIT_CODE);
    }
  }

  const [busFound, busType, newBusOffset] = scanProtocolBus(
    responsesPath,
    requestId,
    expect,
    busOffset,
  );
  busOffset = newBusOffset;

  if (busFound) {
    const result: RpcResponse = {
      schemaVersion: SCHEMA_VERSION,
      status: "ok",
      session: sessionName,
      requestId,
      responseType: busType ?? "unknown",
      response: busFound,
    };
    await printResult(result);
    process.exit(0);
  }

  if (ALLOW_LOG_FALLBACK) {
    const [found, responseType, newLogOffset] = scanLog(
      logPath,
      requestId,
      expect,
      logOffset,
    );
    logOffset = newLogOffset;

    if (found) {
      const result: RpcResponse = {
        schemaVersion: SCHEMA_VERSION,
        status: "ok",
        session: sessionName,
        requestId,
        responseType: responseType ?? "unknown",
        response: found,
      };
      await printResult(result);
      process.exit(0);
    }
  }

  await Bun.sleep(POLL_INTERVAL);
}

const result = errorResult(
  sessionName,
  requestId,
  "response_timeout",
  `No response matching requestId '${requestId}'${expect ? ` and type '${expect}'` : ""} within ${timeout}ms`,
);
await printResult(result);
process.exit(1);
