import { afterEach, describe, expect, it } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readlinkSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runMcpCli } from "./mcp-cli";

const originalFetch = globalThis.fetch;
let server: { url: URL; stop: (force?: boolean) => void } | null = null;

afterEach(() => {
  server?.stop(true);
  server = null;
  globalThis.fetch = originalFetch;
});

function startMockMcp(handler: (body: any) => any) {
  const previousFetch = globalThis.fetch;
  const url = new URL("http://127.0.0.1:43129/");
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const request = new Request(input, init);
      if (new URL(request.url).pathname !== "/rpc") {
        return new Response("not found", { status: 404 });
      }
      if (request.headers.get("authorization") !== "Bearer test-token") {
        return Response.json({ error: "unauthorized" }, { status: 401 });
      }
      const body = await request.json();
      return Response.json(handler(body));
  }) as typeof fetch;
  server = {
    url,
    stop() {
      globalThis.fetch = previousFetch;
    },
  };
  return server;
}

async function runCli(args: string[], env: Record<string, string>) {
  const previous = new Map<string, string | undefined>();
  for (const [key, value] of Object.entries(env)) {
    previous.set(key, process.env[key]);
    process.env[key] = value;
  }
  try {
    return await runMcpCli(args);
  } finally {
    for (const [key, value] of previous) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  }
}

function discoveryEnv(baseUrl: string) {
  const dir = mkdtempSync(join(tmpdir(), "script-kit-mcp-cli-"));
  const serverJson = join(dir, "server.json");
  writeFileSync(
    serverJson,
    JSON.stringify({
      url: baseUrl,
      token: "test-token",
      version: "test",
      capabilities: { tools: true },
    }),
    { mode: 0o600 },
  );
  return {
    dir,
    env: {
      SCRIPT_KIT_MCP_SERVER_JSON: serverJson,
    },
  };
}

describe("mcp-cli", () => {
  it("prints product-oriented top-level help", async () => {
    const result = await runMcpCli(["--help"]);
    expect(typeof result).toBe("string");
    expect(result).toContain("scriptkit mcp tools");
    expect(result).toContain("scriptkit install-command");
    expect(result).toContain("~/.scriptkit/server.json");
  });

  it("prints mcp subcommand help", async () => {
    const result = await runMcpCli(["mcp", "--help"]);
    expect(typeof result).toBe("string");
    expect(result).toContain("Script Kit MCP commands");
    expect(result).toContain("SCRIPT_KIT_MCP_ENDPOINT");
  });

  it("lists tools through discovery server.json", async () => {
    const mock = startMockMcp((body) => {
      expect(body.method).toBe("tools/list");
      return {
        jsonrpc: "2.0",
        id: body.id,
        result: { tools: [{ name: "kit/trigger_builtin" }] },
      };
    });
    const { dir, env } = discoveryEnv(mock.url.origin);
    try {
      const result = await runCli(["mcp", "tools"], env);
      expect(typeof result).toBe("object");
      expect(result.success).toBe(true);
      expect((result as any).data.result.tools[0].name).toBe("kit/trigger_builtin");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("uses the same SK_PATH workspace as the running Script Kit host", async () => {
    const mock = startMockMcp((body) => ({
      jsonrpc: "2.0",
      id: body.id,
      result: { tools: [{ name: "workspace-owned-tool" }] },
    }));
    const { dir } = discoveryEnv(mock.url.origin);
    try {
      const result = await runCli(["mcp", "tools"], {
        SK_PATH: dir,
        SCRIPT_KIT_MCP_SERVER_JSON: "",
        SCRIPT_KIT_MCP_ENDPOINT: "",
        SCRIPT_KIT_MCP_TOKEN: "",
      });
      expect(typeof result).toBe("object");
      expect((result as any).data.result.tools[0].name).toBe("workspace-owned-tool");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it.each([
    "$SCRIPT_KIT_MCP_FIXTURE_ROOT",
    "${SCRIPT_KIT_MCP_FIXTURE_ROOT}",
  ])("expands host-style SK_PATH variables before private discovery: %s", async (root) => {
    const mock = startMockMcp((body) => ({
      jsonrpc: "2.0",
      id: body.id,
      result: { tools: [{ name: "expanded-workspace-tool" }] },
    }));
    const { dir } = discoveryEnv(mock.url.origin);
    try {
      const result = await runCli(["mcp", "tools"], {
        SK_PATH: root,
        SCRIPT_KIT_MCP_FIXTURE_ROOT: dir,
        SCRIPT_KIT_MCP_SERVER_JSON: "",
        SCRIPT_KIT_MCP_ENDPOINT: "",
        SCRIPT_KIT_MCP_TOKEN: "",
      });
      expect(typeof result).toBe("object");
      expect((result as any).data.result.tools[0].name).toBe("expanded-workspace-tool");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("calls tools with JSON arguments and bearer auth", async () => {
    const mock = startMockMcp((body) => {
      expect(body.method).toBe("tools/call");
      expect(body.params).toEqual({
        name: "kit/trigger_builtin",
        arguments: { builtinId: "builtin/clipboard-history" },
      });
      return {
        jsonrpc: "2.0",
        id: body.id,
        result: { content: [{ type: "text", text: "{\"ok\":true}" }] },
      };
    });
    const result = await runCli(
      [
        "call",
        "kit/trigger_builtin",
        JSON.stringify({ builtinId: "builtin/clipboard-history" }),
      ],
      {
        SCRIPT_KIT_MCP_ENDPOINT: `${mock.url.origin}/rpc`,
        SCRIPT_KIT_MCP_TOKEN: "test-token",
      },
    );
    expect(typeof result).toBe("object");
    expect(result.success).toBe(true);
    expect((result as any).data.result.content[0].text).toBe("{\"ok\":true}");
  });

  it("keeps direct mcp command aliases for repo-local workflows", async () => {
    const mock = startMockMcp((body) => {
      expect(body.method).toBe("tools/list");
      return {
        jsonrpc: "2.0",
        id: body.id,
        result: { tools: [] },
      };
    });
    const result = await runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: mock.url.origin,
      SCRIPT_KIT_MCP_TOKEN: "test-token",
    });
    expect(typeof result).toBe("object");
    expect(result.success).toBe(true);
  });

  it("reads resources", async () => {
    const mock = startMockMcp((body) => {
      expect(body.method).toBe("resources/read");
      expect(body.params).toEqual({ uri: "kit://trigger-builtins" });
      return {
        jsonrpc: "2.0",
        id: body.id,
        result: { contents: [{ uri: "kit://trigger-builtins", text: "ids" }] },
      };
    });
    const result = await runCli(["read", "kit://trigger-builtins"], {
      SCRIPT_KIT_MCP_ENDPOINT: mock.url.origin,
      SCRIPT_KIT_MCP_TOKEN: "test-token",
    });
    expect(typeof result).toBe("object");
    expect((result as any).data.result.contents[0].uri).toBe("kit://trigger-builtins");
  });

  it.each([
    "https://malicious.example/rpc",
    "http://127.0.0.1.malicious.example/rpc",
    "http://user:secret@127.0.0.1:43129/rpc",
    "ftp://127.0.0.1:43129/rpc",
    "http://127.0.0.1:43129/not-script-kit",
    "http://127.0.0.1:43129/rpc?forward=malicious",
    "http://127.0.0.1:43129/rpc#credentials",
  ])("refuses unsafe MCP endpoint %s before sending the local bearer token", async (endpoint) => {
    let requestCount = 0;
    globalThis.fetch = (async () => {
      requestCount += 1;
      return Response.json({ jsonrpc: "2.0", id: "forged", result: {} });
    }) as typeof fetch;

    await expect(runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: endpoint,
      SCRIPT_KIT_MCP_TOKEN: "private-local-bearer-token",
    })).rejects.toThrow("local Script Kit MCP endpoint");
    expect(requestCount).toBe(0);
  });

  it("refuses a remote endpoint supplied by private server discovery", async () => {
    const { dir, env } = discoveryEnv("https://malicious.example");
    let requestCount = 0;
    globalThis.fetch = (async () => {
      requestCount += 1;
      return Response.json({ jsonrpc: "2.0", id: "forged", result: {} });
    }) as typeof fetch;
    try {
      await expect(runCli(["tools"], env)).rejects.toThrow("local Script Kit MCP endpoint");
      expect(requestCount).toBe(0);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("refuses symlinked bearer-token discovery before reading its external owner", async () => {
    const { dir } = discoveryEnv("http://127.0.0.1:43129");
    const alias = join(dir, "aliased-server.json");
    symlinkSync(join(dir, "server.json"), alias);
    try {
      await expect(runCli(["tools"], {
        SCRIPT_KIT_MCP_SERVER_JSON: alias,
      })).rejects.toThrow("private regular discovery file");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("refuses bearer-token discovery readable by other users", async () => {
    const { dir, env } = discoveryEnv("http://127.0.0.1:43129");
    chmodSync(join(dir, "server.json"), 0o644);
    try {
      await expect(runCli(["tools"], env)).rejects.toThrow("owner-only permissions");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("reports JSON-RPC failures as failures instead of successful MCP calls", async () => {
    const mock = startMockMcp((body) => ({
      jsonrpc: "2.0",
      id: body.id,
      error: { code: -32601, message: "tool is unavailable" },
    }));
    await expect(runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: mock.url.origin,
      SCRIPT_KIT_MCP_TOKEN: "test-token",
    })).rejects.toThrow("tool is unavailable");
  });

  it.each(["localhost", "[::1]"])(
    "preserves supported loopback MCP endpoint %s",
    async (host) => {
      startMockMcp((body) => ({ jsonrpc: "2.0", id: body.id, result: { tools: [] } }));
      const result = await runCli(["tools"], {
        SCRIPT_KIT_MCP_ENDPOINT: `http://${host}:43129`,
        SCRIPT_KIT_MCP_TOKEN: "test-token",
      });
      expect(typeof result).toBe("object");
      expect((result as any).success).toBe(true);
    },
  );

  it("never follows a redirect with the private local bearer token", async () => {
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      expect(init?.redirect).toBe("error");
      const body = await new Request(input, init).json() as { id: string };
      return Response.json({ jsonrpc: "2.0", id: body.id, result: {} });
    }) as typeof fetch;
    const result = await runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: "http://127.0.0.1:43129/rpc",
      SCRIPT_KIT_MCP_TOKEN: "private-local-bearer-token",
    });
    expect(typeof result).toBe("object");
  });

  it.each(["non-json", "http-error", "json-rpc-error"])(
    "never exposes the local bearer credential in %s diagnostics",
    async (mode) => {
      const secret = "private-local-bearer-token-should-never-appear";
      globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
        if (mode === "non-json") {
          return new Response(`upstream echoed ${secret}`, { status: 502 });
        }
        if (mode === "http-error") {
          return Response.json({ error: `Bearer ${secret}` }, { status: 401 });
        }
        const body = await new Request(input, init).json() as { id: string };
        return Response.json({
          jsonrpc: "2.0",
          id: body.id,
          error: { code: -32001, message: `auth failed for Bearer ${secret}` },
        });
      }) as typeof fetch;

      let failure: unknown;
      try {
        await runCli(["tools"], {
          SCRIPT_KIT_MCP_ENDPOINT: "http://127.0.0.1:43129/rpc",
          SCRIPT_KIT_MCP_TOKEN: secret,
        });
      } catch (error) {
        failure = error;
      }
      expect(failure).toBeInstanceOf(Error);
      expect((failure as Error).message).not.toContain(secret);
    },
  );

  it.each([
    ["wrong-id", (id: string) => ({ jsonrpc: "2.0", id: `${id}-stale`, result: {} })],
    ["wrong-protocol", (id: string) => ({ jsonrpc: "1.0", id, result: {} })],
    ["missing-outcome", (id: string) => ({ jsonrpc: "2.0", id })],
    ["contradictory-outcome", (id: string) => ({ jsonrpc: "2.0", id, result: {}, error: { code: 1, message: "failed" } })],
  ])("refuses mismatched or malformed JSON-RPC response %s", async (_name, response) => {
    const mock = startMockMcp((body) => response(body.id));
    await expect(runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: mock.url.origin,
      SCRIPT_KIT_MCP_TOKEN: "test-token",
    })).rejects.toThrow("invalid JSON-RPC response");
  });

  it("bounds an unresponsive MCP request without using a socket or leaving a timer", async () => {
    globalThis.fetch = ((_input: RequestInfo | URL, init?: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => {
          reject(new DOMException("The operation was aborted", "AbortError"));
        }, { once: true });
      })) as typeof fetch;

    const started = performance.now();
    await expect(runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: "http://127.0.0.1:43129/rpc",
      SCRIPT_KIT_MCP_TOKEN: "test-token",
      SCRIPT_KIT_MCP_TIMEOUT_MS: "25",
    })).rejects.toThrow("timed out after 25ms");
    expect(performance.now() - started).toBeLessThan(1_000);
  });

  it("rejects a declared oversized MCP response before accepting its body", async () => {
    globalThis.fetch = (async () => new Response("x".repeat(257), {
      headers: { "content-length": "257" },
    })) as typeof fetch;
    await expect(runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: "http://127.0.0.1:43129/rpc",
      SCRIPT_KIT_MCP_TOKEN: "test-token",
      SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES: "256",
    })).rejects.toThrow("256-byte response safety budget");
  });

  it("cancels an oversized streamed MCP response without buffering unbounded bytes", async () => {
    let cancelled = false;
    let pullCount = 0;
    globalThis.fetch = (async () => new Response(new ReadableStream<Uint8Array>({
      pull(controller) {
        pullCount += 1;
        controller.enqueue(new Uint8Array(pullCount === 1 ? 128 : 129));
        if (pullCount >= 8) controller.close();
      },
      cancel() { cancelled = true; },
    }))) as typeof fetch;
    await expect(runCli(["tools"], {
      SCRIPT_KIT_MCP_ENDPOINT: "http://127.0.0.1:43129/rpc",
      SCRIPT_KIT_MCP_TOKEN: "test-token",
      SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES: "256",
    })).rejects.toThrow("256-byte response safety budget");
    expect(cancelled).toBe(true);
  });

  it.each(["0", "-1", "1.5", "67108865", "invalid"])(
    "refuses invalid MCP response budget %s before fetching",
    async (budget) => {
      let requestCount = 0;
      globalThis.fetch = (async () => {
        requestCount += 1;
        return Response.json({});
      }) as typeof fetch;
      await expect(runCli(["tools"], {
        SCRIPT_KIT_MCP_ENDPOINT: "http://127.0.0.1:43129/rpc",
        SCRIPT_KIT_MCP_TOKEN: "test-token",
        SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES: budget,
      })).rejects.toThrow("SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES");
      expect(requestCount).toBe(0);
    },
  );

  it.each(["0", "-1", "1.5", "120001", "invalid"])(
    "refuses invalid MCP request timeout %s before fetching",
    async (timeout) => {
      let requestCount = 0;
      globalThis.fetch = (async () => {
        requestCount += 1;
        return Response.json({});
      }) as typeof fetch;
      await expect(runCli(["tools"], {
        SCRIPT_KIT_MCP_ENDPOINT: "http://127.0.0.1:43129/rpc",
        SCRIPT_KIT_MCP_TOKEN: "test-token",
        SCRIPT_KIT_MCP_TIMEOUT_MS: timeout,
      })).rejects.toThrow("SCRIPT_KIT_MCP_TIMEOUT_MS");
      expect(requestCount).toBe(0);
    },
  );

  it("installs a scriptkit command symlink at a chosen target", async () => {
    const dir = mkdtempSync(join(tmpdir(), "script-kit-command-"));
    const target = join(dir, "scriptkit");
    try {
      const result = await runMcpCli(["install-command", target]);
      expect(typeof result).toBe("object");
      expect((result as any).success).toBe(true);
      expect(existsSync(target)).toBe(true);
      expect(readlinkSync(target)).toContain("mcp-cli.ts");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
