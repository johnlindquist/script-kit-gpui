# Script Kit MCP Integration

Script Kit GPUI implements the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) to expose scripts, scriptlets, and app functionality to AI agents and other MCP clients.

## Quick Start

### 1. Start Script Kit

```bash
./target/release/script-kit-gpui
```

The MCP server starts automatically on port **43210**.

### 2. Get Your Token

```bash
cat ~/.kenv/agent-token
```

### 3. Test the Connection

```bash
TOKEN=$(cat ~/.kenv/agent-token)

curl -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         AI Agent / MCP Client                        │
├─────────────────────────────────────────────────────────────────────┤
│                              │                                       │
│                      JSON-RPC 2.0 over HTTP                          │
│                              │                                       │
├─────────────────────────────────────────────────────────────────────┤
│                       Script Kit MCP Server                          │
│                      (localhost:43210/rpc)                           │
├──────────────┬──────────────┬───────────────┬───────────────────────┤
│   Kit Tools  │ Script Tools │   Resources   │     Authentication    │
│  kit/show    │ scripts/*    │ kit://state   │    Bearer Token       │
│  kit/hide    │              │ scripts://    │  ~/.kenv/agent-token  │
│  kit/state   │              │ scriptlets:// │                       │
└──────────────┴──────────────┴───────────────┴───────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────────┐
│                         Script Execution                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │
│  │   Scripts   │    │  Scriptlets │    │     SDK     │              │
│  │ ~/.kenv/    │    │ ~/.kenv/    │    │ input()     │              │
│  │  scripts/   │    │ scriptlets/ │    │ output()    │              │
│  └─────────────┘    └─────────────┘    └─────────────┘              │
└─────────────────────────────────────────────────────────────────────┘
```

## Server Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| Port | 43210 | HTTP server port |
| Token File | `~/.kenv/agent-token` | Authentication token location |
| Discovery File | `~/.kenv/server.json` | Server info for clients |

### Discovery File (`~/.kenv/server.json`)

```json
{
  "url": "http://localhost:43210",
  "token": "your-token-here",
  "version": "1.0.0"
}
```

## Authentication

All requests require a Bearer token in the Authorization header:

```bash
curl -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '...'
```

The token is automatically generated on first run and stored at `~/.kenv/agent-token`.

## API Reference

### Protocol

- **Transport**: HTTP POST
- **Endpoint**: `http://localhost:43210/rpc`
- **Format**: JSON-RPC 2.0

### Methods

| Method | Description |
|--------|-------------|
| `initialize` | Initialize MCP session |
| `tools/list` | List available tools |
| `tools/call` | Execute a tool |
| `resources/list` | List available resources |
| `resources/read` | Read resource content |

---

## Tools

### Kit Tools

Built-in tools for controlling Script Kit:

#### `kit/show`

Show the Script Kit window.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "kit/show",
    "arguments": {}
  }
}
```

#### `kit/hide`

Hide the Script Kit window.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "kit/hide",
    "arguments": {}
  }
}
```

#### `kit/state`

Get current app state.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "kit/state",
    "arguments": {}
  }
}
```

**Response:**
```json
{
  "visible": false,
  "focused": false,
  "scripts_count": 340,
  "scriptlets_count": 27,
  "current_filter": ""
}
```

### Script Tools

Scripts with a `schema` definition are automatically exposed as MCP tools.

#### Creating a Script Tool

**Option 1: Using `defineSchema()` (Recommended)**

```typescript
import "@johnlindquist/kit"

metadata = {
  name: "My Tool",
  description: "Does something useful",
}

const { input, output } = defineSchema({
  input: {
    message: { type: "string", required: true },
    count: { type: "number", default: 1 },
  },
  output: {
    result: { type: "string" },
  },
} as const)

const { message, count } = await input()
output({ result: `${message} x${count}` })
```

**Option 2: Direct Schema Assignment**

```typescript
import "@johnlindquist/kit"

// Name comes from metadata (preferred) or // Name: comment
metadata = {
  name: "My Tool",
  description: "Does something useful",
}

schema = {
  input: {
    message: { type: "string", required: true },
  },
  output: {
    result: { type: "string" },
  },
}

const data = await input()
output({ result: `Got: ${data.message}` })
```

#### Calling a Script Tool

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "scripts/my-tool",
    "arguments": {
      "message": "Hello",
      "count": 3
    }
  }
}
```

#### Tool Naming

Tool names are derived from `metadata.name` (priority) or `// Name:` comment:

| Source | Example | Tool Name |
|--------|---------|-----------|
| `metadata.name = "My Tool"` | `metadata = { name: "My Tool" }` | `scripts/my-tool` |
| `// Name: My Tool` | Comment at top of file | `scripts/my-tool` |
| Filename | `my-tool.ts` | `scripts/my-tool` |

**Priority**: `metadata.name` > `// Name:` comment > filename

---

## Resources

Resources provide read-only access to Script Kit data.

### `kit://state`

Current app state.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": { "uri": "kit://state" }
}
```

### `scripts://`

List of all scripts.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": { "uri": "scripts://" }
}
```

**Response:**
```json
[
  {
    "name": "Hello World",
    "path": "/Users/x/.kenv/scripts/hello-world.ts",
    "extension": "ts",
    "description": "A simple greeting script",
    "has_schema": true
  },
  ...
]
```

### `scriptlets://`

List of all scriptlets.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": { "uri": "scriptlets://" }
}
```

**Response:**
```json
[
  {
    "name": "Current Time",
    "tool": "js",
    "group": "MCP Examples"
  },
  ...
]
```

---

## Schema Reference

### Field Types

| Type | TypeScript | JSON Schema | Example |
|------|------------|-------------|---------|
| `string` | `string` | `{"type": "string"}` | `"hello"` |
| `number` | `number` | `{"type": "number"}` | `42` |
| `boolean` | `boolean` | `{"type": "boolean"}` | `true` |
| `array` | `T[]` | `{"type": "array", "items": {...}}` | `[1, 2, 3]` |
| `object` | `object` | `{"type": "object"}` | `{"a": 1}` |

### Field Properties

| Property | Type | Description |
|----------|------|-------------|
| `type` | string | Field type (required) |
| `description` | string | Human-readable description |
| `required` | boolean | Whether field is required (default: false) |
| `default` | any | Default value if not provided |
| `enum` | array | Allowed values |
| `items` | object | Schema for array items |

### Example Schema

```typescript
const { input, output } = defineSchema({
  input: {
    // Required string with description
    query: {
      type: "string",
      description: "Search query",
      required: true,
    },
    // Optional number with default
    limit: {
      type: "number",
      description: "Max results",
      default: 10,
    },
    // Enum constraint
    sort: {
      type: "string",
      description: "Sort order",
      enum: ["asc", "desc"],
      default: "asc",
    },
    // Array of strings
    tags: {
      type: "array",
      description: "Filter tags",
      items: { type: "string" },
    },
  },
  output: {
    results: {
      type: "array",
      description: "Search results",
    },
    total: {
      type: "number",
      description: "Total count",
    },
  },
} as const)
```

### Generated JSON Schema

The above produces this MCP tool definition:

```json
{
  "name": "scripts/search-tool",
  "description": "Search for items",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Search query"
      },
      "limit": {
        "type": "number",
        "description": "Max results",
        "default": 10
      },
      "sort": {
        "type": "string",
        "description": "Sort order",
        "enum": ["asc", "desc"],
        "default": "asc"
      },
      "tags": {
        "type": "array",
        "description": "Filter tags",
        "items": { "type": "string" }
      }
    },
    "required": ["query"]
  }
}
```

---

## SDK Functions

### `input<T>()`

Get typed input from the agent.

```typescript
const { input } = defineSchema({
  input: {
    name: { type: "string", required: true },
  },
} as const)

const { name } = await input()
// name: string
```

### `output(data)`

Send typed output to the agent. Can be called multiple times - results accumulate.

```typescript
const { output } = defineSchema({
  output: {
    step1: { type: "string" },
    step2: { type: "string" },
  },
} as const)

output({ step1: "Done" })
// Later...
output({ step2: "Also done" })
// Final output: { step1: "Done", step2: "Also done" }
```

### `defineSchema(schema)`

Create typed `input`/`output` functions with full TypeScript inference.

```typescript
const { input, output, schema } = defineSchema({
  input: { /* ... */ },
  output: { /* ... */ },
} as const)

// input() returns typed object
// output() accepts typed object
// schema is the raw schema object
```

### Internal Functions

| Function | Description |
|----------|-------------|
| `_setScriptInput(data)` | Set input data (called by runtime) |
| `_getScriptOutput()` | Get accumulated output |
| `_resetScriptIO()` | Reset input/output state (testing) |

---

## Testing

### Smoke Test Suite

Run the full MCP test suite:

```bash
# Start Script Kit first
./target/release/script-kit-gpui &
sleep 3

# Run tests
./tests/mcp/mcp-smoke-test.sh

# Quick tests only
./tests/mcp/mcp-smoke-test.sh --quick
```

### Manual Testing with curl

```bash
TOKEN=$(cat ~/.kenv/agent-token)

# List tools
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq

# Call a tool
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "id":1,
    "method":"tools/call",
    "params":{
      "name":"kit/state",
      "arguments":{}
    }
  }' | jq
```

### Testing with mcp-cli

```bash
# Note: mcp-cli may have OAuth issues - use curl for testing
bunx @wong2/mcp-cli --url "http://localhost:43210/mcp?token=$TOKEN"
```

---

## Example Scripts

See `tests/mcp/scripts/` for complete examples:

| Script | Description |
|--------|-------------|
| `greeting-tool.ts` | Simple greeting with style enum |
| `calculator-tool.ts` | Math operations with validation |
| `file-info-tool.ts` | File system information |
| `text-transform-tool.ts` | Text transformations with arrays |
| `json-tool.ts` | JSON parsing and extraction |
| `no-schema-tool.ts` | Script without schema (not exposed) |

---

## Troubleshooting

### Server Not Responding

```bash
# Check if running
lsof -i :43210

# Check logs
tail -100 ~/.kenv/logs/script-kit-gpui.jsonl | grep -i mcp
```

### Token Issues

```bash
# Verify token exists
cat ~/.kenv/agent-token

# Token is regenerated on app restart if missing
```

### Tool Not Appearing

1. Ensure script has `schema = {...}` or `defineSchema({...})`
2. Check for syntax errors in schema
3. Restart Script Kit to reload scripts
4. Check logs for parsing errors

### Script Not Executing

Currently, `tools/call` returns `"status": "pending"` - the script is queued but execution and output capture is not yet fully implemented. The `input()`/`output()` functions work correctly when scripts run interactively.

---

## File Locations

| File | Purpose |
|------|---------|
| `~/.kenv/agent-token` | Authentication token |
| `~/.kenv/server.json` | Server discovery info |
| `~/.kenv/scripts/` | User scripts |
| `~/.kenv/scriptlets/` | Scriptlet markdown files |
| `~/.kenv/logs/script-kit-gpui.jsonl` | Application logs |

---

## Source Files

| File | Description |
|------|-------------|
| `src/mcp_server.rs` | HTTP server and routing |
| `src/mcp_protocol.rs` | JSON-RPC protocol handling |
| `src/mcp_kit_tools.rs` | kit/* tool implementations |
| `src/mcp_script_tools.rs` | scripts/* tool generation |
| `src/mcp_resources.rs` | Resource handlers |
| `src/schema_parser.rs` | Schema extraction from scripts |
| `src/metadata_parser.rs` | Metadata extraction |
| `scripts/kit-sdk.ts` | SDK with input/output functions |
