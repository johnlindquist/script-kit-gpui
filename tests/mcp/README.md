# MCP Test Suite

This directory contains smoke tests and example scripts for the Script Kit MCP integration.

## Directory Structure

```
tests/mcp/
├── README.md                 # This file
├── mcp-smoke-test.sh         # Main test runner
├── scripts/                  # Example script tools
│   ├── greeting-tool.ts      # Simple greeting with enum
│   ├── calculator-tool.ts    # Math operations
│   ├── file-info-tool.ts     # File system info
│   ├── text-transform-tool.ts # Text transformations
│   ├── json-tool.ts          # JSON processing
│   └── no-schema-tool.ts     # Script without schema (negative test)
└── scriptlets/
    └── mcp-examples.md       # Example scriptlets
```

## Running Tests

### Prerequisites

1. Build and start Script Kit:
   ```bash
   cargo build --release
   ./target/release/script-kit-gpui &
   sleep 3
   ```

2. Verify MCP server is running:
   ```bash
   lsof -i :43210
   ```

### Run All Tests

```bash
./tests/mcp/mcp-smoke-test.sh
```

### Run Quick Tests Only

```bash
./tests/mcp/mcp-smoke-test.sh --quick
```

### Expected Output

```
╔════════════════════════════════════════════════════════════╗
║          MCP Server Smoke Test Suite                       ║
╚════════════════════════════════════════════════════════════╝

Checking MCP server at http://localhost:43210...
Server is running

=== Initialize ===
  PASS Returns jsonrpc 2.0
  PASS Returns id
  PASS Has result
  PASS Has capabilities
  PASS Server name is script-kit

=== Tools List ===
  PASS Returns tools array
  PASS Has kit/show tool
  PASS Has kit/hide tool
  PASS Has kit/state tool

...

╔════════════════════════════════════════════════════════════╗
║                        Summary                             ║
╚════════════════════════════════════════════════════════════╝

  Tests Run:    35
  Passed:       35
  Failed:       0

All tests passed!
```

## Example Scripts

### Creating a New MCP Tool

1. Create a script in `~/.kenv/scripts/`:

```typescript
import "@johnlindquist/kit"

metadata = {
  name: "My Tool Name",
  description: "What this tool does",
}

const { input, output } = defineSchema({
  input: {
    param1: { type: "string", required: true },
    param2: { type: "number", default: 10 },
  },
  output: {
    result: { type: "string" },
  },
} as const)

const { param1, param2 } = await input()
output({ result: `${param1} x ${param2}` })
```

2. The tool will appear as `scripts/my-tool-name` in the MCP tools list.

### Testing Your Tool

```bash
TOKEN=$(cat ~/.kenv/agent-token)

# List tools (verify your tool appears)
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | \
  jq '.result.tools[] | select(.name | contains("my-tool"))'

# Call your tool
curl -s -X POST "http://localhost:43210/rpc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "id":1,
    "method":"tools/call",
    "params":{
      "name":"scripts/my-tool-name",
      "arguments":{"param1":"hello","param2":5}
    }
  }' | jq
```

## Test Categories

| Category | Tests | Description |
|----------|-------|-------------|
| Initialize | 5 | MCP session initialization |
| Tools List | 4 | Tool discovery |
| Kit Tools | 4 | Built-in kit/* tools |
| Resources List | 4 | Resource discovery |
| Resources Read | 5 | Reading resource content |
| Script Tools | 5 | Script-based tools |
| Error Handling | 4 | Error responses |
| Authentication | 2 | Token validation |
| Metadata Priority | 2 | metadata.name vs // Name: |

## Adding New Tests

Edit `mcp-smoke-test.sh` and add a new test function:

```bash
test_my_feature() {
  echo -e "${YELLOW}=== My Feature ===${NC}"
  
  local response
  response=$(rpc "method/name" '{"param":"value"}')
  
  assert_json "Test description" "$response" ".path.to.value" "expected"
  
  echo ""
}
```

Then call it from `main()`.

## See Also

- [MCP.md](../../MCP.md) - Full MCP documentation
- [PROTOCOL.md](../../docs/PROTOCOL.md) - Protocol reference
- [SDK Tests](../sdk/) - SDK function tests
