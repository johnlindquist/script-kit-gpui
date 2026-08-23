# MCP and Agent Context

Script Kit GPUI uses MCP in both directions: the running app **is** an MCP server that external agents can read and control, and scripts/Agent Chat can **call** MCP servers you configure.

## Script Kit as an MCP Server

While the app runs, it serves HTTP MCP on `localhost:43210`:

- **Auth** — bearer token stored at `~/.scriptkit/agent-token` (generated on first run).
- **Discovery** — the app writes `~/.scriptkit/server.json` with the live `url`, `token`, version, and capabilities. Always re-read it instead of hard-coding values; it can be removed or rotated when the app stops.
- **Health** — `GET /health` returns `{"status":"healthy"}` without auth.
- **JSON-RPC** — clients use the `/rpc` endpoint (the `.url` in `server.json` is the base URL; append `/rpc`).

```bash
SERVER_JSON="$HOME/.scriptkit/server.json"
jq -e '.url and .token and (.capabilities.tools == true)' "$SERVER_JSON"
curl -fsS "$(jq -r '.url' "$SERVER_JSON" | sed 's:/$::')/health"
```

Treat `server.json` and `agent-token` as sensitive — never paste them into issues, logs, or committed config.

To register the live app with Codex, follow [Codex MCP Setup](../codex-mcp-setup.md).

## `kit://` Resources

External agents read structured, schema-versioned app state:

| Resource | Contents |
| --- | --- |
| `kit://context` | AI-relevant desktop snapshot (selected text, frontmost app, menu bar, browser URL, focused window). Supports `?profile=minimal`, `?diagnostics=1`, and per-field flags |
| `kit://context/schema` | Self-describing schema for the context resource |
| `kit://state` | App state for safe setup/proof checks |
| `kit://scripts`, `kit://scriptlets` | Discovered script/scriptlet metadata |
| `kit://sdk-reference` | The SDK function reference (same data as the in-app SDK Reference) |
| `kit://script-templates` | Starter templates shared with the launcher |
| `kit://failed-scripts` | Scripts that failed validation |
| `kit://notes` | Notes list; `?tag=...`, `&full=true`, or `kit://notes/{id}` |
| `kit://brain` | Local memory: `kit://brain/recall?q=...`, `/doc`, `/docs`, `/signals` |
| `kit://clipboard-history` | Newest clipboard entries (`?id=`, `?limit=`) |
| `kit://dictation`, `kit://dictation-history` | Latest dictation and saved transcripts (`?id=`) |
| `kit://focused-item` | Active surface selection/focus metadata |
| `kit://computer-use/readiness` | Computer-use permission/readiness receipt |
| `kit://git-status`, `kit://git-diff`, `kit://processes`, `kit://system` | Environment snapshots |
| `kit://audit` | Recent MCP mutation audit events |

## Tools

Read tools like `kit/state` are always safe. Mutation tools (`kit/show`, `kit/hide`, `kit/trigger_builtin`, `kit/notes_create`, `kit/scripts_run`, `kit/clipboard_pin`, `kit/config_set`, ...) return structured JSON and are audited to an `mcp-audit.jsonl` file under `~/.scriptkit/`. Destructive tools require explicit confirmation, and token scopes can narrow access by domain (`notes:write`, `scripts:run`, `config:write`, ...).

Computer-use observation tools (`computer/list_native_windows`, `computer/capture_native_window`, `computer/see`, ...) are covered in [Computer Use](./computer-use.md).

## CLI Access

The app maintains a command shim at `~/.scriptkit/bin/scriptkit` that reads `server.json` for you:

```bash
~/.scriptkit/bin/scriptkit mcp tools
~/.scriptkit/bin/scriptkit mcp resources
~/.scriptkit/bin/scriptkit mcp read kit://state
~/.scriptkit/bin/scriptkit mcp call kit/trigger_builtin '{"builtinId":"builtin/clipboard-history"}'
```

It requires the app to be running and also accepts `SCRIPT_KIT_MCP_ENDPOINT` / `SCRIPT_KIT_MCP_TOKEN` overrides. If `SK_PATH` selects a custom Script Kit workspace, including a `~`, `$VARIABLE`, or `${VARIABLE}` path, the CLI and SDK read that workspace's `server.json` and `config.ts` instead of another account directory.

The app-owned MCP endpoint is always local: only `localhost`, `127.0.0.1`, or `[::1]` with the exact `/rpc` path can receive its discovery bearer token. Discovery must be an owner-only regular file, redirects never forward credentials, and errors never print bearer values. The SDK and CLI validate the exact JSON-RPC request ID and report provider failures rather than treating HTTP 200 as success.

Requests default to a 30-second timeout and a 16-MiB response budget. Set `SCRIPT_KIT_MCP_TIMEOUT_MS` to a positive whole number no greater than `120000` when a deliberately slower local or configured remote server needs another time budget. Set `SCRIPT_KIT_MCP_MAX_RESPONSE_BYTES` to a positive whole number no greater than `67108864` when an approved image response requires more space. Oversized HTTP bodies stop streaming immediately; overflowing or timed-out stdio servers and their owned subprocesses are stopped together.

Tool discovery is bounded instead of starting every configured server at once: unattended runs use at most two workers, interactive runs default to four, and `SCRIPT_KIT_MCP_CONCURRENCY` can select an explicit positive limit up to two unattended or eight interactive. Results keep configuration order, invalid limits fail before transport, and a failing discovery waits for its already-owned requests to finish before returning.

## External MCP Servers (Script Kit as a Client)

Define servers in `~/.scriptkit/config.ts` under `mcp.servers` — `transport` selects stdio or HTTP:

```typescript
export default {
  mcp: {
    enabled: true,
    servers: {
      localTools: {
        transport: "stdio",
        command: "my-mcp-server",
        args: [],
        env: {},
      },
      remoteTools: {
        transport: "http",
        endpoint: "http://127.0.0.1:3000/rpc",
        headers: { authorization: `Bearer ${process.env.MY_MCP_TOKEN ?? ""}` },
      },
    },
  },
};
```

Then call them from scripts with the SDK's `mcp.*` helpers:

```typescript
import "@scriptkit/sdk";

const tools = await mcp.listTools();          // across configured servers
const matches = await mcp.discover("calendar");
const result = await mcp.call("localTools", "create_event", { title: "Standup" });
```

The local-only restriction applies to the app's own discovery token, not to explicitly configured external HTTP servers. Remote MCP servers remain supported when their endpoint and credentials are deliberately defined under the active workspace's `mcp.servers` configuration.

`mcp.listServers()` and `mcp.getServer(id)` expose safe server metadata, not private credentials: authorization headers, API keys, tokens, passwords, cookies, sensitive command-line arguments, endpoint credentials/query values, and bearer-like values are removed or returned as `[REDACTED]`. The original configured credentials remain private and are still used for actual server requests. Changes, creation, and deletion of the current workspace's `config.ts` take effect without restarting the SDK, including edits that preserve file size and modification time; relative TypeScript imports remain supported. In noninteractive verification, configured stdio servers cannot disable noninteractive mode, enable native/system input, emit operator test-status notifications, or override another desktop permission.

## Agent Chat Context

Agent Chat (double-tap the main hotkey, or `⌘⇧Space`) stages context for the active profile:

- **`@` mentions** — in the main input or the chat composer, `@selection`, `@clipboard`, `@file:readme`, `@notes:...`, `@dictation:...` attach specific context parts. See [Main Menu Input](./main-menu-input.md).
- **Add to AI** — surfaces like clipboard history and file search have "Attach to Agent Chat" actions (`⌃⌘A` in clipboard history).
- Context resolution is tolerant: if one part fails (e.g. no browser detected), the rest still submit and the failure is recorded in a resolution receipt.

## Best Practices

- Re-read `~/.scriptkit/server.json` after every app restart.
- Prefer `kit://context?profile=minimal` unless the agent needs more; use `?diagnostics=1` to debug missing fields.
- Use resources for read-only context; use tools only when an action is required.
- In scripts, call `mcp.discover(query)` before hard-coding tool names if the server set is user-configurable.

## Related

- [Codex MCP Setup](../codex-mcp-setup.md) — full registration and verification flow
- [Computer Use](./computer-use.md) — window observation for agents
- [Dictation](./dictation.md) — dictation history as agent context
