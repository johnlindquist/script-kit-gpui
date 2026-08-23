#!/usr/bin/env bun
/**
 * Script Kit MCP CLI
 *
 * Thin JSON CLI for the live Script Kit MCP server. It reads
 * ~/.scriptkit/server.json by default, or accepts env overrides:
 *   SCRIPT_KIT_MCP_SERVER_JSON
 *   SCRIPT_KIT_MCP_ENDPOINT
 *   SCRIPT_KIT_MCP_TOKEN
 */

import {
  chmodSync,
  closeSync,
  constants as fsConstants,
  existsSync,
  fstatSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  readlinkSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";

type DiscoveryInfo = {
  url: string;
  token: string;
  version?: string;
  capabilities?: Record<string, unknown>;
};

type CliResult =
  | { success: true; data: unknown }
  | { success: false; error: string };

class CliFailure extends Error {}

function usage() {
  return [
    "Script Kit command line",
    "",
    "Usage:",
    "  scriptkit --help",
    "  scriptkit mcp tools",
    "  scriptkit mcp resources",
    "  scriptkit mcp call <tool-name> [json-arguments]",
    "  scriptkit mcp read <resource-uri>",
    "  scriptkit mcp rpc <method> [json-params]",
    "  scriptkit install-command [target-path]",
    "",
    "Examples:",
    "  scriptkit mcp tools",
    "  scriptkit mcp read kit://trigger-builtins",
    "  scriptkit mcp call kit/trigger_builtin '{\"builtinId\":\"builtin/clipboard-history\"}'",
    "  scriptkit install-command ~/.local/bin/scriptkit",
    "",
    "MCP commands require Script Kit to be running so ~/.scriptkit/server.json exists.",
    "The discovery file contains a bearer token; do not paste it into logs or docs.",
  ].join("\n");
}

function mcpUsage() {
  return [
    "Script Kit MCP commands",
    "",
    "Usage:",
    "  scriptkit mcp tools",
    "  scriptkit mcp resources",
    "  scriptkit mcp call <tool-name> [json-arguments]",
    "  scriptkit mcp read <resource-uri>",
    "  scriptkit mcp rpc <method> [json-params]",
    "",
    "Environment overrides:",
    "  SCRIPT_KIT_MCP_SERVER_JSON  Path to server.json",
    "  SCRIPT_KIT_MCP_ENDPOINT     Base URL or /rpc endpoint",
    "  SCRIPT_KIT_MCP_TOKEN        Bearer token",
    "  SCRIPT_KIT_MCP_TIMEOUT_MS   Request timeout, at most 120000ms",
    "  SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES  Response budget, at most 67108864 bytes",
  ].join("\n");
}

function print(result: CliResult): void {
  console.log(JSON.stringify(result, null, 2));
}

function fail(message: string): never {
  throw new CliFailure(message);
}

function parseJsonArg(raw: string | undefined, fallback: unknown): unknown {
  if (raw === undefined || raw.trim() === "") {
    return fallback;
  }
  try {
    return JSON.parse(raw);
  } catch (error) {
    fail(`Invalid JSON argument: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function discoveryPath(): string {
  const explicit = process.env.SCRIPT_KIT_MCP_SERVER_JSON?.trim();
  if (explicit) return explicit;

  const configured = process.env.SK_PATH?.trim();
  if (!configured) return join(homedir(), ".scriptkit", "server.json");
  const homeExpanded = configured === "~" || configured.startsWith("~/")
    ? join(homedir(), configured.slice(1))
    : configured;
  const workspace = homeExpanded.replace(
    /\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)/g,
    (match, braced: string | undefined, unbraced: string | undefined) =>
      process.env[braced ?? unbraced ?? ""] ?? match,
  );
  return join(workspace, "server.json");
}

function loadDiscovery(): DiscoveryInfo | null {
  const path = discoveryPath();
  if (!existsSync(path)) {
    return null;
  }
  let descriptor: number | undefined;
  try {
    const owner = lstatSync(path);
    if (!owner.isFile() || owner.isSymbolicLink()) {
      fail(`Script Kit MCP requires a private regular discovery file: ${path}`);
    }
    descriptor = openSync(path, fsConstants.O_RDONLY | (fsConstants.O_NOFOLLOW ?? 0));
    const opened = fstatSync(descriptor);
    if (!opened.isFile() || opened.dev !== owner.dev || opened.ino !== owner.ino) {
      fail(`Script Kit MCP requires a private regular discovery file: ${path}`);
    }
    if (process.platform !== "win32" && (opened.mode & 0o077) !== 0) {
      fail(`Script Kit MCP discovery requires owner-only permissions: ${path}`);
    }
    if (typeof process.getuid === "function" && opened.uid !== process.getuid()) {
      fail(`Script Kit MCP discovery must belong to the current user: ${path}`);
    }
    const discovery = JSON.parse(readFileSync(descriptor, "utf8")) as DiscoveryInfo;
    if (!discovery || typeof discovery !== "object") {
      fail(`Script Kit MCP discovery must contain a private endpoint and token: ${path}`);
    }
    return discovery;
  } catch (error) {
    if (error instanceof CliFailure) throw error;
    fail(`Failed to parse ${path}: ${error instanceof Error ? error.message : String(error)}`);
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

function envValue(name: string): string | undefined {
  const value = process.env[name];
  return value && value.trim() !== "" ? value : undefined;
}

function reviewedLocalEndpoint(endpointOrBase: string): string {
  let endpoint: URL;
  try {
    endpoint = new URL(endpointOrBase);
  } catch {
    fail("A local Script Kit MCP endpoint must be a valid loopback HTTP /rpc URL.");
  }
  const loopback = endpoint.hostname === "localhost"
    || endpoint.hostname === "127.0.0.1"
    || endpoint.hostname === "[::1]";
  if (
    !loopback ||
    !["http:", "https:"].includes(endpoint.protocol) ||
    endpoint.username ||
    endpoint.password ||
    endpoint.search ||
    endpoint.hash ||
    !["/", "/rpc"].includes(endpoint.pathname)
  ) {
    fail("A local Script Kit MCP endpoint must be a loopback HTTP /rpc URL without credentials, query, or fragment.");
  }
  endpoint.pathname = "/rpc";
  return endpoint.toString();
}

function resolveEndpointAndToken(): { endpoint: string; token: string } {
  const endpointOverride = envValue("SCRIPT_KIT_MCP_ENDPOINT");
  const tokenOverride = envValue("SCRIPT_KIT_MCP_TOKEN");
  // Validate an explicit destination before opening local bearer-token state.
  const reviewedOverride = endpointOverride ? reviewedLocalEndpoint(endpointOverride) : undefined;
  const discovery = reviewedOverride && tokenOverride ? null : loadDiscovery();
  const token = tokenOverride ?? discovery?.token;
  const endpointOrBase = reviewedOverride ?? discovery?.url;

  if (!endpointOrBase) {
    fail(
      `Missing MCP endpoint. Set SCRIPT_KIT_MCP_ENDPOINT or start Script Kit so ${discoveryPath()} exists.`,
    );
  }
  if (typeof token !== "string" || !token.trim() || /[\r\n]/.test(token)) {
    fail(
      `Missing MCP token. Set SCRIPT_KIT_MCP_TOKEN or start Script Kit so ${discoveryPath()} contains a token.`,
    );
  }

  if (typeof endpointOrBase !== "string") {
    fail("A local Script Kit MCP endpoint must be a valid loopback HTTP /rpc URL.");
  }
  const endpoint = reviewedOverride ?? reviewedLocalEndpoint(endpointOrBase);
  return { endpoint, token };
}

function mcpRequestTimeoutMs(): number {
  const configured = process.env.SCRIPT_KIT_MCP_TIMEOUT_MS;
  if (configured === undefined || configured === "") return 30_000;
  const timeout = Number(configured);
  if (!/^[1-9][0-9]*$/.test(configured) || !Number.isSafeInteger(timeout) || timeout > 120_000) {
    fail("SCRIPT_KIT_MCP_TIMEOUT_MS must be a positive whole duration no greater than 120000ms.");
  }
  return timeout;
}

function mcpMaxResponseBytes(): number {
  const configured = process.env.SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES;
  if (configured === undefined || configured === "") return 16 * 1024 * 1024;
  const budget = Number(configured);
  if (!/^[1-9][0-9]*$/.test(configured) || !Number.isSafeInteger(budget) || budget > 64 * 1024 * 1024) {
    fail("SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES must be a positive whole byte budget no greater than 67108864.");
  }
  return budget;
}

async function readBoundedMcpResponse(response: Response, maximumBytes: number): Promise<string> {
  const declared = response.headers.get("content-length");
  if (declared !== null && /^\d+$/.test(declared) && Number(declared) > maximumBytes) {
    await response.body?.cancel();
    fail(`Script Kit MCP exceeded its ${maximumBytes}-byte response safety budget.`);
  }

  const reader = response.body?.getReader();
  if (!reader) return "";
  const chunks: Uint8Array[] = [];
  let received = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      received += value.byteLength;
      if (received > maximumBytes) {
        await reader.cancel();
        fail(`Script Kit MCP exceeded its ${maximumBytes}-byte response safety budget.`);
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  return new TextDecoder().decode(Buffer.concat(chunks, received));
}

function safeMcpDiagnostic(message: string, token: string): string {
  return message
    .split(token).join("[REDACTED]")
    .replace(/\bBearer\s+[^\s,;"'}]+/gi, "Bearer [REDACTED]")
    .slice(0, 512);
}

function defaultCommandTarget(): string {
  return join(homedir(), ".local", "bin", "scriptkit");
}

function currentScriptPath(): string {
  const url = new URL(import.meta.url);
  if (url.protocol !== "file:") {
    fail("Cannot install command because the current CLI is not running from a file path.");
  }
  return url.pathname;
}

function installCommand(targetArg: string | undefined): CliResult {
  const source = currentScriptPath();
  const target = resolve(targetArg?.trim() || defaultCommandTarget());
  mkdirSync(dirname(target), { recursive: true });

  if (existsSync(target)) {
    const stat = lstatSync(target);
    if (stat.isSymbolicLink()) {
      const existing = readlinkSync(target);
      if (resolve(dirname(target), existing) !== source) {
        rmSync(target);
      }
    } else {
      fail(
        `Refusing to replace non-symlink at ${target}. Remove it or pass a different target path.`,
      );
    }
  }

  if (!existsSync(target)) {
    symlinkSync(source, target);
  }
  chmodSync(source, 0o755);

  return {
    success: true,
    data: {
      command: "scriptkit",
      target,
      source,
      note:
        "Add the target directory to PATH if `scriptkit --help` is not found in new shells.",
    },
  };
}

export async function rpc(method: string, params: unknown): Promise<unknown> {
  const { endpoint, token } = resolveEndpointAndToken();
  const timeoutMs = mcpRequestTimeoutMs();
  const maximumResponseBytes = mcpMaxResponseBytes();
  const requestId = `script-kit-mcp-cli-${crypto.randomUUID()}`;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  let response: Response;
  let text: string;
  try {
    response = await fetch(endpoint, {
      method: "POST",
      redirect: "error",
      signal: controller.signal,
      headers: {
        authorization: `Bearer ${token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: requestId,
        method,
        params,
      }),
    });
    text = await readBoundedMcpResponse(response, maximumResponseBytes);
  } catch (error) {
    if (controller.signal.aborted) {
      fail(`Script Kit MCP ${method} timed out after ${timeoutMs}ms.`);
    }
    const detail = error instanceof Error ? error.message : String(error);
    fail(`Script Kit MCP ${method} request failed: ${safeMcpDiagnostic(detail, token)}`);
  } finally {
    clearTimeout(timer);
  }

  let payload: unknown;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    fail(`MCP server returned non-JSON HTTP ${response.status}.`);
  }

  if (!response.ok) {
    fail(`MCP server returned HTTP ${response.status}.`);
  }

  if (
    !payload ||
    typeof payload !== "object" ||
    Array.isArray(payload) ||
    (payload as Record<string, unknown>).jsonrpc !== "2.0" ||
    (payload as Record<string, unknown>).id !== requestId
  ) {
    fail(`Script Kit MCP ${method} returned an invalid JSON-RPC response.`);
  }
  const envelope = payload as Record<string, unknown>;
  const hasResult = Object.hasOwn(envelope, "result");
  const hasError = Object.hasOwn(envelope, "error");
  if (hasResult === hasError) {
    fail(`Script Kit MCP ${method} returned an invalid JSON-RPC response.`);
  }
  if (hasError) {
    const error = envelope.error;
    if (
      !error ||
      typeof error !== "object" ||
      !Number.isSafeInteger((error as Record<string, unknown>).code) ||
      typeof (error as Record<string, unknown>).message !== "string" ||
      !(error as Record<string, unknown>).message
    ) {
      fail(`Script Kit MCP ${method} returned an invalid JSON-RPC response.`);
    }
    const typedError = error as { code: number; message: string };
    fail(`Script Kit MCP ${method} failed (${typedError.code}): ${safeMcpDiagnostic(typedError.message, token)}`);
  }

  return payload;
}

export async function runMcpCli(argv: string[]): Promise<CliResult | string> {
  const [rawCommand, ...rest] = argv;
  if (!rawCommand || rawCommand === "--help" || rawCommand === "-h") {
    return usage();
  }

  if (rawCommand === "install-command") {
    return installCommand(rest[0]);
  }

  let command = rawCommand;
  let args = rest;
  if (rawCommand === "mcp") {
    const [mcpCommand, ...mcpArgs] = rest;
    if (!mcpCommand || mcpCommand === "--help" || mcpCommand === "-h") {
      return mcpUsage();
    }
    command = mcpCommand;
    args = mcpArgs;
  }

  const [first, second] = args;
  let data: unknown;
  if (command === "tools" || command === "list-tools") {
    data = await rpc("tools/list", {});
  } else if (command === "resources" || command === "list-resources") {
    data = await rpc("resources/list", {});
  } else if (command === "call") {
    if (!first) {
      fail("call requires a tool name");
    }
    data = await rpc("tools/call", {
      name: first,
      arguments: parseJsonArg(second, {}),
    });
  } else if (command === "read") {
    if (!first) {
      fail("read requires a resource URI");
    }
    data = await rpc("resources/read", { uri: first });
  } else if (command === "rpc") {
    if (!first) {
      fail("rpc requires a method");
    }
    data = await rpc(first, parseJsonArg(second, {}));
  } else {
    fail(`Unknown command: ${rawCommand}. Use --help for usage.`);
  }

  return { success: true, data };
}

async function main() {
  const result = await runMcpCli(process.argv.slice(2));
  if (typeof result === "string") {
    console.log(result);
  } else {
    print(result);
  }
}

if (import.meta.main) {
  main().catch((error) => {
    print({
      success: false,
      error: error instanceof Error ? error.message : String(error),
    });
    process.exit(error instanceof CliFailure ? 1 : 1);
  });
}
