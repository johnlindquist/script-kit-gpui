// Name: SDK Test - Local MCP Client Safety
// Description: Verifies SDK computer helpers using private in-memory MCP fixtures only.

import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

type Outcome = "running" | "pass" | "fail";

function report(test: string, status: Outcome, details: Record<string, unknown> = {}): void {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), ...details }));
}

const fixtureDirectory = mkdtempSync(join(tmpdir(), "script-kit-sdk-mcp-"));
const discoveryPath = join(fixtureDirectory, "server.json");
const privateToken = "sdk-local-private-bearer-fixture";
const originalFetch = globalThis.fetch;
const previousDiscoveryPath = process.env.SCRIPT_KIT_MCP_SERVER_JSON;
const previousTimeout = process.env.SCRIPT_KIT_MCP_TIMEOUT_MS;
const previousKitPath = process.env.SK_PATH;
const stdioServerPath = join(fixtureDirectory, "mcp-stdio-fixture.ts");
const descendantPidPath = join(fixtureDirectory, "owned-descendant.pid");
const unsafeLaunchPath = join(fixtureDirectory, "unsafe-server-launched.txt");
process.env.SK_PATH = fixtureDirectory;

writeFileSync(stdioServerPath, `
import { createInterface } from "node:readline";
const mode = process.argv[2];
if (mode === "unsafe-override") await Bun.write(process.argv[3], "launched");
for await (const line of createInterface({ input: process.stdin })) {
  const request = JSON.parse(line);
  if (mode === "owned-descendant" && request.method === "initialize") {
    const child = Bun.spawn({ cmd: ["/bin/sleep", "5"], stdout: "inherit", stderr: "inherit" });
    await Bun.write(process.argv[3], String(child.pid));
    continue;
  }
  if (mode === "never-respond") continue;
  if (request.id === undefined) continue;
  if (mode === "stderr-flood") {
    process.stderr.write("x".repeat(1024 * 1024 + 1));
    continue;
  }
  if (mode === "private-error") {
    process.stdout.write(JSON.stringify({
      jsonrpc: "2.0",
      id: request.id,
      error: { code: -32001, message: "failed Bearer " + process.env.MCP_TOKEN },
    }) + "\\n");
    continue;
  }
  const id = mode === "wrong-response" ? request.id + 1 : request.id;
  const result = request.method === "tools/list"
    ? { tools: [{ name: "owned-local-tool", description: "isolated fixture" }] }
    : { capabilities: {} };
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, result }) + "\\n");
}
`);
writeFileSync(join(fixtureDirectory, "config.ts"), `export default ${JSON.stringify({
  mcp: {
    enabled: true,
    servers: {
      stdioHealthy: { transport: "stdio", command: process.execPath, args: [stdioServerPath, "healthy"] },
      stdioNever: { transport: "stdio", command: process.execPath, args: [stdioServerPath, "never-respond"] },
      stdioWrong: { transport: "stdio", command: process.execPath, args: [stdioServerPath, "wrong-response"] },
      stdioFlood: { transport: "stdio", command: process.execPath, args: [stdioServerPath, "stderr-flood"] },
      stdioPrivateError: {
        transport: "stdio",
        command: process.execPath,
        args: [stdioServerPath, "private-error"],
        env: { MCP_TOKEN: privateToken },
      },
      stdioUnsafe: {
        transport: "stdio",
        command: process.execPath,
        args: [stdioServerPath, "unsafe-override", unsafeLaunchPath],
        env: { SCRIPT_KIT_ALLOW_NATIVE_INPUT: "1" },
      },
      remoteExplicit: {
        transport: "http",
        endpoint: "https://configured-remote.example/rpc",
        headers: { authorization: "Bearer explicitly-configured-remote-token" },
      },
      stdioOwned: {
        transport: "stdio",
        command: process.execPath,
        args: [stdioServerPath, "owned-descendant", descendantPidPath],
      },
    },
  },
})};\n`, { mode: 0o600 });

function writeDiscovery(url = "http://127.0.0.1:43129", token = privateToken): void {
  if (existsSync(discoveryPath)) rmSync(discoveryPath);
  writeFileSync(discoveryPath, JSON.stringify({ url, token }), { mode: 0o600 });
  chmodSync(discoveryPath, 0o600);
  process.env.SCRIPT_KIT_MCP_SERVER_JSON = discoveryPath;
}

async function check(name: string, operation: () => Promise<unknown>): Promise<void> {
  report(name, "running");
  const started = Date.now();
  writeDiscovery();
  delete process.env.SCRIPT_KIT_MCP_TIMEOUT_MS;
  globalThis.fetch = (async () => {
    throw new Error("MCP fixture attempted an unapproved real network request");
  }) as typeof fetch;
  try {
    const result = await operation();
    report(name, "pass", { result, duration_ms: Date.now() - started });
  } catch (error) {
    report(name, "fail", { error: String(error), duration_ms: Date.now() - started });
  } finally {
    globalThis.fetch = originalFetch;
  }
}

async function expectFailure(operation: () => Promise<unknown>, expected: string): Promise<Error> {
  let failure: unknown;
  try {
    await operation();
  } catch (error) {
    failure = error;
  }
  if (!(failure instanceof Error) || !failure.message.includes(expected)) {
    throw new Error(`Expected failure containing ${JSON.stringify(expected)}, got ${String(failure)}`);
  }
  return failure;
}

function installServer(
  handler: (
    payload: Record<string, unknown>,
    request: Request,
    init?: RequestInit,
  ) => Promise<Response> | Response,
): { requests: number } {
  const observed = { requests: 0 };
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    observed.requests += 1;
    const request = new Request(input, init);
    const payload = await request.json() as Record<string, unknown>;
    return await handler(payload, request, init);
  }) as typeof fetch;
  return observed;
}

function successResponse(payload: Record<string, unknown>, headers: HeadersInit = {}): Response {
  if (payload.method === "notifications/initialized") {
    return new Response(null, { status: 202, headers });
  }
  if (payload.method === "initialize") {
    return Response.json({ jsonrpc: "2.0", id: payload.id, result: { capabilities: {} } }, { headers });
  }
  return Response.json({
    jsonrpc: "2.0",
    id: payload.id,
    result: {
      content: [{ type: "text", text: JSON.stringify({
        schemaVersion: 1,
        source: "fixture",
        scope: "isolated",
        status: "listed",
        appCount: 0,
        windowCount: 0,
        apps: [],
        warnings: [],
      }) }],
    },
  }, { headers });
}

try {
  await check("mcp-custom-workspace-aligns-config-and-public-sdk-paths", async () => {
    if (skPath() !== fixtureDirectory || kitPath("scripts") !== join(fixtureDirectory, "scripts")) {
      throw new Error("SDK paths do not respect the Rust host's active SK_PATH workspace");
    }
    const servers = await mcp.listServers();
    if (!servers.some((server) => server.id === "stdioHealthy")) {
      throw new Error("SDK loaded another workspace's MCP configuration");
    }
    return { configuredServerCount: servers.length, activeWorkspaceRespected: true };
  });

  await check("mcp-workspace-cache-never-reuses-another-SK_PATH-configuration", async () => {
    const alternate = join(fixtureDirectory, "alternate-workspace");
    mkdirSync(alternate, { recursive: true });
    writeFileSync(join(alternate, "config.ts"), 'export default { mcp: { servers: {} } };\n');
    process.env.SK_PATH = alternate;
    try {
      const alternateServers = await mcp.listServers();
      if (alternateServers.length !== 0) {
        throw new Error("An alternate workspace inherited another workspace's private MCP servers");
      }
    } finally {
      process.env.SK_PATH = fixtureDirectory;
    }
    const restored = await mcp.listServers();
    if (!restored.some((server) => server.id === "stdioHealthy")) {
      throw new Error("The original workspace did not recover its actual MCP configuration");
    }
    return { alternateServerCount: 0, originalWorkspaceRestored: true };
  });

  await check("mcp-self-discovery-defaults-to-the-owner-selected-workspace", async () => {
    delete process.env.SCRIPT_KIT_MCP_SERVER_JSON;
    const observed = installServer((payload) => successResponse(payload));
    const result = await computer.listNativeWindows();
    if (result.status !== "listed" || observed.requests !== 3) {
      throw new Error("SDK did not load discovery from the active SK_PATH workspace");
    }
    return { requests: observed.requests, activeWorkspaceRespected: true };
  });

  await check("mcp-private-local-discovery-preserves-auth-and-session", async () => {
    const observed = installServer((payload, request, init) => {
      if (request.headers.get("authorization") !== `Bearer ${privateToken}`) {
        throw new Error("MCP request lost its private bearer authorization");
      }
      if (init?.redirect !== "error") {
        throw new Error("MCP request could redirect its private bearer authorization");
      }
      if (payload.method !== "initialize" && request.headers.get("mcp-session-id") !== "owned-session") {
        throw new Error("MCP request lost its negotiated server session");
      }
      return successResponse(payload, { "mcp-session-id": "owned-session" });
    });
    const result = await computer.listNativeWindows();
    if (result.status !== "listed" || result.windowCount !== 0 || observed.requests !== 3) {
      throw new Error(`Unexpected isolated MCP result: ${JSON.stringify(result)}`);
    }
    return { requests: observed.requests, realNetworkUsed: false, nativeInputUsed: false };
  });

  await check("mcp-remote-discovery-never-receives-a-local-bearer-token", async () => {
    writeDiscovery("https://malicious.example");
    const observed = installServer((payload) => successResponse(payload));
    await expectFailure(() => computer.listNativeWindows(), "local Script Kit MCP endpoint");
    if (observed.requests !== 0) throw new Error("Remote endpoint received a private request");
    return { requests: observed.requests };
  });

  await check("mcp-symlinked-token-discovery-refuses-before-reading", async () => {
    const external = join(fixtureDirectory, "external-server.json");
    writeFileSync(external, JSON.stringify({ url: "http://127.0.0.1:43129", token: privateToken }), {
      mode: 0o600,
    });
    rmSync(discoveryPath);
    symlinkSync(external, discoveryPath);
    const observed = installServer((payload) => successResponse(payload));
    try {
      await expectFailure(() => computer.listNativeWindows(), "private regular discovery file");
      if (observed.requests !== 0) throw new Error("Symlinked token owner reached transport");
    } finally {
      rmSync(discoveryPath, { force: true });
      rmSync(external, { force: true });
    }
    return { requests: observed.requests };
  });

  await check("mcp-world-readable-token-discovery-refuses-before-fetch", async () => {
    chmodSync(discoveryPath, 0o644);
    const observed = installServer((payload) => successResponse(payload));
    await expectFailure(() => computer.listNativeWindows(), "owner-only permissions");
    if (observed.requests !== 0) throw new Error("World-readable token owner reached transport");
    return { requests: observed.requests };
  });

  for (const [mode, envelope] of [
    ["wrong-id", (id: unknown) => ({ jsonrpc: "2.0", id: `${String(id)}-stale`, result: {} })],
    ["wrong-protocol", (id: unknown) => ({ jsonrpc: "1.0", id, result: {} })],
    ["missing-outcome", (id: unknown) => ({ jsonrpc: "2.0", id })],
    ["contradictory-outcome", (id: unknown) => ({ jsonrpc: "2.0", id, result: {}, error: { code: 1, message: "failed" } })],
  ] as const) {
    await check(`mcp-${mode}-cannot-complete-the-wrong-request`, async () => {
      installServer((payload) => Response.json(envelope(payload.id)));
      await expectFailure(() => computer.listNativeWindows(), "invalid JSON-RPC response");
      return { invalidEnvelopeRejected: mode };
    });
  }

  await check("mcp-provider-errors-never-expose-private-bearer-values", async () => {
    installServer((payload) => Response.json({
      jsonrpc: "2.0",
      id: payload.id,
      error: { code: -32001, message: `Authorization failed for Bearer ${privateToken}` },
    }));
    const failure = await expectFailure(() => computer.listNativeWindows(), "Authorization failed");
    if (failure.message.includes(privateToken)) {
      throw new Error("MCP failure exposed its private bearer credential");
    }
    return { tokenLeaked: false };
  });

  await check("mcp-unresponsive-local-server-times-out-without-a-real-socket", async () => {
    process.env.SCRIPT_KIT_MCP_TIMEOUT_MS = "25";
    globalThis.fetch = ((_input: RequestInfo | URL, init?: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => {
          reject(new DOMException("The operation was aborted", "AbortError"));
        }, { once: true });
      })) as typeof fetch;
    const started = Date.now();
    await expectFailure(() => computer.listNativeWindows(), "timed out after 25ms");
    if (Date.now() - started > 1_000) throw new Error("MCP timeout was not bounded");
    return { bounded: true, realNetworkUsed: false };
  });

  await check("mcp-invalid-timeout-refuses-before-network-or-token-transfer", async () => {
    process.env.SCRIPT_KIT_MCP_TIMEOUT_MS = "0";
    const observed = installServer((payload) => successResponse(payload));
    await expectFailure(() => computer.listNativeWindows(), "SCRIPT_KIT_MCP_TIMEOUT_MS");
    if (observed.requests !== 0) throw new Error("Invalid timeout reached MCP transport");
    return { requests: observed.requests };
  });

  await check("mcp-owned-stdio-server-preserves-real-request-and-response-identity", async () => {
    const tools = await mcp.listTools("stdioHealthy");
    if (tools.length !== 1 || tools[0]?.name !== "owned-local-tool") {
      throw new Error(`Unexpected isolated stdio tools: ${JSON.stringify(tools)}`);
    }
    return { toolCount: tools.length, appStarted: false };
  });

  await check("mcp-explicit-remote-server-remains-supported-with-safe-redirect-policy", async () => {
    const observed = installServer((payload, request, init) => {
      if (new URL(request.url).hostname !== "configured-remote.example") {
        throw new Error("Explicitly configured remote MCP server was unexpectedly rewritten");
      }
      if (request.headers.get("authorization") !== "Bearer explicitly-configured-remote-token") {
        throw new Error("Explicitly configured remote MCP server lost its own credential");
      }
      if (init?.redirect !== "error") throw new Error("Configured MCP credential could follow a redirect");
      if (payload.method === "tools/list") {
        return Response.json({ jsonrpc: "2.0", id: payload.id, result: { tools: [{ name: "remote-tool" }] } });
      }
      return successResponse(payload);
    });
    const tools = await mcp.listTools("remoteExplicit");
    if (tools[0]?.name !== "remote-tool" || observed.requests !== 3) {
      throw new Error("Configured remote MCP server did not complete its isolated fixture journey");
    }
    return { requests: observed.requests, actualNetworkUsed: false };
  });

  await check("mcp-stdio-config-cannot-override-noninteractive-native-input-authority", async () => {
    rmSync(unsafeLaunchPath, { force: true });
    await expectFailure(() => mcp.listTools("stdioUnsafe"), "noninteractive MCP server cannot override");
    if (existsSync(unsafeLaunchPath)) throw new Error("Unsafe configured MCP server actually started");
    return { unauthorizedChildStarted: false };
  });

  await check("mcp-stdio-failures-never-echo-private-server-environment-values", async () => {
    const failure = await expectFailure(() => mcp.listTools("stdioPrivateError"), "failed");
    if (failure.message.includes(privateToken)) {
      throw new Error("Stdio MCP error exposed a configured private server credential");
    }
    return { tokenLeaked: false };
  });

  await check("mcp-stdio-stderr-is-bounded-and-owned-process-cleanup-is-immediate", async () => {
    const started = Date.now();
    await expectFailure(() => mcp.listTools("stdioFlood"), "1048576-byte safety budget");
    if (Date.now() - started > 1_000) throw new Error("Stdio stderr flood cleanup was not bounded");
    return { outputBudgetBytes: 1_048_576 };
  });

  await check("mcp-wrong-stdio-response-id-fails-without-hanging", async () => {
    process.env.SCRIPT_KIT_MCP_TIMEOUT_MS = "150";
    await expectFailure(() => mcp.listTools("stdioWrong"), "invalid JSON-RPC response");
    return { wrongResponseRejected: true };
  });

  await check("mcp-unresponsive-stdio-server-times-out-and-closes", async () => {
    process.env.SCRIPT_KIT_MCP_TIMEOUT_MS = "100";
    const started = Date.now();
    await expectFailure(() => mcp.listTools("stdioNever"), "timed out after 100ms");
    if (Date.now() - started > 1_000) throw new Error("Stdio MCP timeout was not bounded");
    return { bounded: true };
  });

  await check("mcp-timeout-terminates-its-exact-owned-stdio-descendant-group", async () => {
    process.env.SCRIPT_KIT_MCP_TIMEOUT_MS = "150";
    let descendant = 0;
    try {
      await expectFailure(() => mcp.listTools("stdioOwned"), "timed out after 150ms");
      descendant = Number.parseInt(readFileSync(descendantPidPath, "utf8"), 10);
      let alive = true;
      try {
        process.kill(descendant, 0);
      } catch {
        alive = false;
      }
      if (alive) throw new Error("Timed-out MCP server left its owned descendant alive");
      return { descendantTerminated: true };
    } finally {
      if (!descendant && existsSync(descendantPidPath)) {
        descendant = Number.parseInt(readFileSync(descendantPidPath, "utf8"), 10);
      }
      if (descendant > 0) {
        try { process.kill(descendant, "SIGKILL"); } catch {}
      }
      rmSync(descendantPidPath, { force: true });
    }
  });
} finally {
  globalThis.fetch = originalFetch;
  if (previousDiscoveryPath === undefined) {
    delete process.env.SCRIPT_KIT_MCP_SERVER_JSON;
  } else {
    process.env.SCRIPT_KIT_MCP_SERVER_JSON = previousDiscoveryPath;
  }
  if (previousTimeout === undefined) {
    delete process.env.SCRIPT_KIT_MCP_TIMEOUT_MS;
  } else {
    process.env.SCRIPT_KIT_MCP_TIMEOUT_MS = previousTimeout;
  }
  if (previousKitPath === undefined) {
    delete process.env.SK_PATH;
  } else {
    process.env.SK_PATH = previousKitPath;
  }
  rmSync(fixtureDirectory, { recursive: true, force: true });
}
