# Codex MCP Setup

This guide registers the live Script Kit GPUI app as a Codex MCP server over HTTP. The app owns server discovery through `~/.scriptkit/server.json`; use that file instead of inventing a URL or copying token material into Codex config.

## Prerequisites

- Script Kit GPUI is built and running.
- `codex` is available on your `PATH`.
- `jq` and `curl` are available in the shell where you verify the setup.
- You have local shell access to the same user account that runs Script Kit GPUI.

## Start or Confirm the Server

Build and start the app if it is not already running:

```bash
cargo build
SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui
```

In another shell, confirm the app wrote its discovery file:

```bash
SERVER_JSON="$HOME/.scriptkit/server.json"
test -f "$SERVER_JSON"
jq -e '.url and .token and .version and (.capabilities.tools == true)' "$SERVER_JSON"
```

`~/.scriptkit/server.json` contains sensitive bearer-token material. Do not paste it into issues, shell transcripts, logs, screenshots, commits, or shared docs. The file is app-owned discovery metadata and may disappear when the app stops because the MCP server removes discovery metadata on shutdown.

If the app runs with `SK_PATH`, discovery lives at `$SK_PATH/server.json`; the shipped `scriptkit` CLI and SDK use that same active workspace. Discovery must be an owner-only regular file, and the app's bearer token is accepted only for a loopback `localhost`, `127.0.0.1`, or `[::1]` `/rpc` endpoint. Redirects and remote/lookalike destinations are refused before the token is sent.

The file's `.url` value is the base server URL, not the JSON-RPC endpoint. Codex and direct JSON-RPC checks must append `/rpc`:

```bash
SCRIPT_KIT_MCP_BASE_URL="$(jq -r '.url' "$SERVER_JSON")"
SCRIPT_KIT_MCP_ENDPOINT="${SCRIPT_KIT_MCP_BASE_URL%/}/rpc"
export SCRIPT_KIT_MCP_TOKEN="$(jq -r '.token' "$SERVER_JSON")"

curl -fsS "${SCRIPT_KIT_MCP_BASE_URL%/}/health" | jq -e '.status=="healthy"'
test "${SCRIPT_KIT_MCP_ENDPOINT}" != "${SCRIPT_KIT_MCP_BASE_URL}"
case "$SCRIPT_KIT_MCP_ENDPOINT" in
  */rpc) ;;
  *) echo "MCP endpoint must end in /rpc: $SCRIPT_KIT_MCP_ENDPOINT" >&2; exit 1 ;;
esac
```

The token environment variable must be available to the shell or launcher environment where Codex runs. Do not put the token literal in `.codex/config.toml`.

## Register with Codex

Script Kit GPUI is an HTTP MCP server. Register it with Codex's `--url` path and have Codex read the bearer token from `SCRIPT_KIT_MCP_TOKEN` at runtime:

```bash
codex mcp remove script-kit >/dev/null 2>&1 || true
codex mcp add script-kit \
  --url "$SCRIPT_KIT_MCP_ENDPOINT" \
  --bearer-token-env-var SCRIPT_KIT_MCP_TOKEN

codex mcp list
codex mcp get script-kit
```

The repo's `.codex/config.toml` contains a stdio MCP entry for `lat`; that shape is not the Script Kit setup. `lat` uses a command, while Script Kit GPUI uses the live app's HTTP `/rpc` endpoint plus bearer-token authentication.

## Verify JSON-RPC Directly

Direct JSON-RPC checks prove the app server independently of Codex UI behavior:

```bash
rpc() {
  local id="$1"
  local method="$2"
  local params="$3"

  curl -sS -X POST "$SCRIPT_KIT_MCP_ENDPOINT" \
    -H "Authorization: Bearer ${SCRIPT_KIT_MCP_TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":\"${id}\",\"method\":\"${method}\",\"params\":${params}}"
}

rpc "codex-guide-init" "initialize" '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"codex-guide-verifier","version":"1.0.0"}}' \
  | tee /tmp/script-kit-mcp-init.json
jq -e '.jsonrpc=="2.0" and .id=="codex-guide-init" and .result.serverInfo.name=="script-kit"' /tmp/script-kit-mcp-init.json

rpc "codex-guide-tools" "tools/list" '{}' \
  | tee /tmp/script-kit-mcp-tools-list.json
jq -e '.result.tools | map(.name) |
  index("kit/show") and
  index("kit/hide") and
  index("kit/state") and
  index("kit/trigger_builtin") and
  index("kit/notes_create") and
  index("kit/scripts_create") and
  index("kit/clipboard_pin") and
  index("kit/config_get")' /tmp/script-kit-mcp-tools-list.json

rpc "codex-guide-resources" "resources/list" '{}' \
  | tee /tmp/script-kit-mcp-resources-list.json
jq -e '.result.resources | map(.uri) |
  index("kit://state") and
  index("scripts://") and
  index("scriptlets://") and
  index("kit://trigger-builtins")' /tmp/script-kit-mcp-resources-list.json

rpc "codex-guide-read-state" "resources/read" '{"uri":"kit://state"}' \
  | tee /tmp/script-kit-mcp-resources-read-state.json
jq -e '.result.contents[0].uri=="kit://state" and (.result.contents[0].text|type=="string")' /tmp/script-kit-mcp-resources-read-state.json

rpc "codex-guide-kit-state" "tools/call" '{"name":"kit/state","arguments":{}}' \
  | tee /tmp/script-kit-mcp-tool-call-kit-state.json
jq -e '.result.content[0].type=="text" and ((.result.content[0].text | fromjson) | has("visible") and has("focused"))' /tmp/script-kit-mcp-tool-call-kit-state.json
```

Use `kit/state` for setup proof because it is safe and read-only. Avoid `kit/show` or `kit/hide` unless you specifically want to change the app window visibility while testing.

For shell workflows that should use the same server discovery and bearer-token behavior, use the app-managed `scriptkit` command shim. Script Kit refreshes it at `~/.scriptkit/bin/scriptkit` on startup:

```bash
~/.scriptkit/bin/scriptkit --help
~/.scriptkit/bin/scriptkit mcp tools
~/.scriptkit/bin/scriptkit mcp read kit://trigger-builtins
~/.scriptkit/bin/scriptkit mcp call kit/trigger_builtin '{"builtinId":"builtin/clipboard-history"}'
```

To add a shorter `scriptkit` command in your shell, run:

```bash
~/.scriptkit/bin/scriptkit install-command ~/.local/bin/scriptkit
```

Then make sure `~/.local/bin` is on `PATH`.

## Troubleshooting

Missing `~/.scriptkit/server.json` means the app server is not running or has shut down. Start Script Kit GPUI again and reread the discovery file.

Stale discovery usually shows up as a failed `/health` check. Reread `~/.scriptkit/server.json` after the app restarts because the port can change when `MCP_PORT` is set.

Port conflicts on `43210` should be handled by the app environment, not by hard-coding the default in Codex. Set `MCP_PORT` before starting Script Kit GPUI, then read the actual `.url` from `server.json`.

Invalid bearer auth returns `401` for non-health endpoints. Export `SCRIPT_KIT_MCP_TOKEN` from the current `server.json` and make sure Codex is launched from an environment that can read that variable.

Token rotation requires rereading `~/.scriptkit/server.json`, exporting the new `SCRIPT_KIT_MCP_TOKEN`, and rerunning `codex mcp add` if the endpoint also changed. Never preserve old token values in files or shell snippets.
