#!/bin/bash
# MCP Server Smoke Test Suite
# Tests the MCP JSON-RPC API for Script Kit GPUI
#
# Usage:
#   ./tests/mcp/mcp-smoke-test.sh          # Run all tests
#   ./tests/mcp/mcp-smoke-test.sh --quick  # Run essential tests only
#
# Prerequisites:
#   - Script Kit GPUI app must be running
#   - Token file at ~/.kenv/agent-token
#   - curl and jq installed

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Config
MCP_PORT="${MCP_PORT:-43210}"
MCP_HOST="${MCP_HOST:-localhost}"
TOKEN_FILE="${HOME}/.kenv/agent-token"
BASE_URL="http://${MCP_HOST}:${MCP_PORT}"

# Counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Get token
get_token() {
  if [[ ! -f "$TOKEN_FILE" ]]; then
    echo -e "${RED}ERROR: Token file not found at $TOKEN_FILE${NC}"
    echo "Make sure Script Kit GPUI is running"
    exit 1
  fi
  cat "$TOKEN_FILE"
}

TOKEN=$(get_token)

# JSON-RPC helper
rpc() {
  local method="$1"
  local params="$2"
  local id="$3"
  
  # Use explicit defaults to avoid bash ${:-} issues with curly braces
  if [[ -z "$params" ]]; then
    params="{}"
  fi
  if [[ -z "$id" ]]; then
    id="1"
  fi
  
  curl -s -X POST "${BASE_URL}/rpc" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":${id},\"method\":\"${method}\",\"params\":${params}}"
}

# Test helper
run_test() {
  local name="$1"
  local expected="$2"
  local actual="$3"
  
  TESTS_RUN=$((TESTS_RUN + 1))
  
  if [[ "$actual" == "$expected" ]]; then
    echo -e "${GREEN}  PASS${NC} $name"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
  else
    echo -e "${RED}  FAIL${NC} $name"
    echo -e "       Expected: $expected"
    echo -e "       Actual:   $actual"
    TESTS_FAILED=$((TESTS_FAILED + 1))
    return 1
  fi
}

# Assert contains helper
assert_contains() {
  local name="$1"
  local haystack="$2"
  local needle="$3"
  
  TESTS_RUN=$((TESTS_RUN + 1))
  
  if echo "$haystack" | grep -q "$needle"; then
    echo -e "${GREEN}  PASS${NC} $name"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
  else
    echo -e "${RED}  FAIL${NC} $name"
    echo -e "       Expected to contain: $needle"
    echo -e "       Actual: ${haystack:0:200}..."
    TESTS_FAILED=$((TESTS_FAILED + 1))
    return 1
  fi
}

# Assert JSON path equals
assert_json() {
  local name="$1"
  local json="$2"
  local path="$3"
  local expected="$4"
  
  local actual
  actual=$(echo "$json" | jq -r "$path" 2>/dev/null || echo "JQ_ERROR")
  run_test "$name" "$expected" "$actual"
}

# Assert JSON path exists
assert_json_exists() {
  local name="$1"
  local json="$2"
  local path="$3"
  
  TESTS_RUN=$((TESTS_RUN + 1))
  
  local value
  value=$(echo "$json" | jq -e "$path" 2>/dev/null)
  
  if [[ $? -eq 0 ]] && [[ "$value" != "null" ]]; then
    echo -e "${GREEN}  PASS${NC} $name"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
  else
    echo -e "${RED}  FAIL${NC} $name"
    echo -e "       Path '$path' not found or null in JSON"
    TESTS_FAILED=$((TESTS_FAILED + 1))
    return 1
  fi
}

# Check server is running
check_server() {
  echo -e "${BLUE}Checking MCP server at ${BASE_URL}...${NC}"
  
  if ! curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/rpc" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}' | grep -q "200"; then
    echo -e "${RED}ERROR: MCP server not responding${NC}"
    echo "Make sure Script Kit GPUI is running"
    exit 1
  fi
  
  echo -e "${GREEN}Server is running${NC}\n"
}

# ============================================================================
# Test Suites
# ============================================================================

test_initialize() {
  echo -e "${YELLOW}=== Initialize ===${NC}"
  
  local response
  response=$(rpc "initialize" '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}')
  
  assert_json "Returns jsonrpc 2.0" "$response" ".jsonrpc" "2.0"
  assert_json "Returns id" "$response" ".id" "1"
  assert_json_exists "Has result" "$response" ".result"
  assert_json_exists "Has capabilities" "$response" ".result.capabilities"
  assert_json "Server name is script-kit" "$response" ".result.serverInfo.name" "script-kit"
  
  echo ""
}

test_tools_list() {
  echo -e "${YELLOW}=== Tools List ===${NC}"
  
  local response
  response=$(rpc "tools/list" "{}")
  
  assert_json_exists "Returns tools array" "$response" ".result.tools"
  
  # Check kit/* tools exist
  local tools
  tools=$(echo "$response" | jq -r '.result.tools[].name')
  
  assert_contains "Has kit/show tool" "$tools" "kit/show"
  assert_contains "Has kit/hide tool" "$tools" "kit/hide"
  assert_contains "Has kit/state tool" "$tools" "kit/state"
  
  echo ""
}

test_kit_tools() {
  echo -e "${YELLOW}=== Kit Tools ===${NC}"
  
  # Test kit/state
  local state_response
  state_response=$(rpc "tools/call" '{"name":"kit/state","arguments":{}}')
  
  assert_json_exists "kit/state returns content" "$state_response" ".result.content"
  assert_json "kit/state content type is text" "$state_response" ".result.content[0].type" "text"
  
  # Parse the state JSON from the text content
  local state_text
  state_text=$(echo "$state_response" | jq -r '.result.content[0].text')
  assert_contains "State contains visible field" "$state_text" '"visible"'
  assert_contains "State contains focused field" "$state_text" '"focused"'
  
  # Test kit/show
  local show_response
  show_response=$(rpc "tools/call" '{"name":"kit/show","arguments":{}}')
  assert_json "kit/show returns success" "$show_response" ".result.content[0].type" "text"
  
  # Test kit/hide
  local hide_response
  hide_response=$(rpc "tools/call" '{"name":"kit/hide","arguments":{}}')
  assert_json "kit/hide returns success" "$hide_response" ".result.content[0].type" "text"
  
  echo ""
}

test_resources_list() {
  echo -e "${YELLOW}=== Resources List ===${NC}"
  
  local response
  response=$(rpc "resources/list" "{}")
  
  assert_json_exists "Returns resources array" "$response" ".result.resources"
  
  local resources
  resources=$(echo "$response" | jq -r '.result.resources[].uri')
  
  assert_contains "Has kit://state resource" "$resources" "kit://state"
  assert_contains "Has scripts:// resource" "$resources" "scripts://"
  assert_contains "Has scriptlets:// resource" "$resources" "scriptlets://"
  
  echo ""
}

test_resources_read() {
  echo -e "${YELLOW}=== Resources Read ===${NC}"
  
  # Read kit://state
  local state_response
  state_response=$(rpc "resources/read" '{"uri":"kit://state"}')
  
  assert_json_exists "State resource returns contents" "$state_response" ".result.contents"
  assert_json "State resource URI" "$state_response" ".result.contents[0].uri" "kit://state"
  
  # Read scripts://
  local scripts_response
  scripts_response=$(rpc "resources/read" '{"uri":"scripts://"}')
  
  assert_json_exists "Scripts resource returns contents" "$scripts_response" ".result.contents"
  
  local scripts_text
  scripts_text=$(echo "$scripts_response" | jq -r '.result.contents[0].text')
  assert_contains "Scripts contain name field" "$scripts_text" '"name"'
  assert_contains "Scripts contain path field" "$scripts_text" '"path"'
  
  # Read scriptlets://
  local scriptlets_response
  scriptlets_response=$(rpc "resources/read" '{"uri":"scriptlets://"}')
  
  assert_json_exists "Scriptlets resource returns contents" "$scriptlets_response" ".result.contents"
  
  echo ""
}

test_script_tools() {
  echo -e "${YELLOW}=== Script Tools ===${NC}"
  
  local response
  response=$(rpc "tools/list" "{}")
  
  # Get script tools (those starting with scripts/)
  local script_tools
  script_tools=$(echo "$response" | jq '[.result.tools[] | select(.name | startswith("scripts/"))]')
  
  local tool_count
  tool_count=$(echo "$script_tools" | jq 'length')
  
  echo "  Found $tool_count script tools"
  
  if [[ "$tool_count" -gt 0 ]]; then
    # Check first script tool has required fields
    local first_tool
    first_tool=$(echo "$script_tools" | jq '.[0]')
    
    assert_json_exists "Script tool has name" "$first_tool" ".name"
    assert_json_exists "Script tool has description" "$first_tool" ".description"
    assert_json_exists "Script tool has inputSchema" "$first_tool" ".inputSchema"
    assert_json "Script tool inputSchema type is object" "$first_tool" ".inputSchema.type" "object"
    
    # Test calling a script tool
    local tool_name
    tool_name=$(echo "$first_tool" | jq -r '.name')
    
    local call_response
    call_response=$(rpc "tools/call" "{\"name\":\"${tool_name}\",\"arguments\":{}}")
    
    assert_json_exists "Script tool call returns content" "$call_response" ".result.content"
    
    local result_text
    result_text=$(echo "$call_response" | jq -r '.result.content[0].text')
    assert_contains "Script tool result has status" "$result_text" '"status"'
  else
    echo -e "${YELLOW}  SKIP${NC} No script tools with schema found"
  fi
  
  echo ""
}

test_error_handling() {
  echo -e "${YELLOW}=== Error Handling ===${NC}"
  
  # Invalid method
  local invalid_method
  invalid_method=$(rpc "invalid/method" "{}")
  assert_json_exists "Invalid method returns error" "$invalid_method" ".error"
  
  # Missing required params
  local missing_params
  missing_params=$(rpc "tools/call" "{}")
  assert_json_exists "Missing params returns error" "$missing_params" ".error"
  
  # Unknown tool
  local unknown_tool
  unknown_tool=$(rpc "tools/call" '{"name":"unknown/tool","arguments":{}}')
  assert_json_exists "Unknown tool returns error" "$unknown_tool" ".error"
  
  # Invalid resource URI
  local invalid_resource
  invalid_resource=$(rpc "resources/read" '{"uri":"invalid://resource"}')
  assert_json_exists "Invalid resource returns error" "$invalid_resource" ".error"
  
  echo ""
}

test_authentication() {
  echo -e "${YELLOW}=== Authentication ===${NC}"
  
  # Test with invalid token
  local bad_response
  bad_response=$(curl -s -X POST "${BASE_URL}/rpc" \
    -H "Authorization: Bearer invalid-token" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')
  
  TESTS_RUN=$((TESTS_RUN + 1))
  if echo "$bad_response" | grep -qi "invalid\|unauthorized\|error"; then
    echo -e "${GREEN}  PASS${NC} Invalid token rejected"
    TESTS_PASSED=$((TESTS_PASSED + 1))
  else
    echo -e "${RED}  FAIL${NC} Invalid token should be rejected"
    TESTS_FAILED=$((TESTS_FAILED + 1))
  fi
  
  # Test without token
  local no_token_response
  no_token_response=$(curl -s -X POST "${BASE_URL}/rpc" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}')
  
  TESTS_RUN=$((TESTS_RUN + 1))
  if echo "$no_token_response" | grep -qi "missing\|unauthorized\|error"; then
    echo -e "${GREEN}  PASS${NC} Missing token rejected"
    TESTS_PASSED=$((TESTS_PASSED + 1))
  else
    echo -e "${RED}  FAIL${NC} Missing token should be rejected"
    TESTS_FAILED=$((TESTS_FAILED + 1))
  fi
  
  echo ""
}

test_metadata_name_priority() {
  echo -e "${YELLOW}=== Metadata Name Priority ===${NC}"
  
  local response
  response=$(rpc "tools/list" "{}")
  
  # Check if the test script exists and uses metadata.name
  local test_tool
  test_tool=$(echo "$response" | jq '.result.tools[] | select(.name | contains("mcp-io-test-via-metadata"))')
  
  if [[ -n "$test_tool" ]] && [[ "$test_tool" != "null" ]]; then
    assert_json "Tool uses metadata.name not // Name comment" "$test_tool" ".name" "scripts/mcp-io-test-via-metadata"
    assert_json "Description from metadata" "$test_tool" ".description" "Tests that metadata.name is used for MCP tool naming"
  else
    echo -e "${YELLOW}  SKIP${NC} Test script mcp-test-input-output.ts not found"
  fi
  
  echo ""
}

# ============================================================================
# Main
# ============================================================================

main() {
  echo ""
  echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
  echo -e "${BLUE}║          MCP Server Smoke Test Suite                       ║${NC}"
  echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
  echo ""
  
  check_server
  
  if [[ "$1" == "--quick" ]]; then
    echo -e "${YELLOW}Running quick tests only...${NC}\n"
    test_initialize
    test_tools_list
    test_resources_list
  else
    test_initialize
    test_tools_list
    test_kit_tools
    test_resources_list
    test_resources_read
    test_script_tools
    test_error_handling
    test_authentication
    test_metadata_name_priority
  fi
  
  # Summary
  echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
  echo -e "${BLUE}║                        Summary                             ║${NC}"
  echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
  echo ""
  echo -e "  Tests Run:    ${TESTS_RUN}"
  echo -e "  ${GREEN}Passed:       ${TESTS_PASSED}${NC}"
  echo -e "  ${RED}Failed:       ${TESTS_FAILED}${NC}"
  echo ""
  
  if [[ $TESTS_FAILED -gt 0 ]]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
  else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
  fi
}

main "$@"
