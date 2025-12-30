#!/bin/bash
# Visual Regression Test Runner for Script Kit GPUI
#
# Runs a test script, captures screenshot, compares against baseline.
# If baseline doesn't exist, creates it. If diff exceeds threshold, fails.
#
# Usage: ./scripts/visual-regression.sh <test-script.ts> [options]
#
# Options:
#   --threshold <n>   Diff percentage threshold (default: 0.1)
#   --tolerance <n>   Per-channel color tolerance 0-255 (default: 0)
#   --update          Update baseline if it exists
#   --wait <n>        Wait time in seconds for render (default: 2)
#   --json            Output only JSONL result
#
# Exit codes:
#   0 - Test passed (or baseline created)
#   1 - Visual regression detected (diff exceeds threshold)
#   2 - Test script error
#   3 - Configuration error

set -e

# Parse arguments
SCRIPT_PATH=""
THRESHOLD="0.1"
TOLERANCE="0"
UPDATE_BASELINE=false
WAIT_SECS="2"
JSON_ONLY=false
TEST_NAME=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --threshold)
      THRESHOLD="$2"
      shift 2
      ;;
    --tolerance)
      TOLERANCE="$2"
      shift 2
      ;;
    --update)
      UPDATE_BASELINE=true
      shift
      ;;
    --wait)
      WAIT_SECS="$2"
      shift 2
      ;;
    --json)
      JSON_ONLY=true
      shift
      ;;
    --name)
      TEST_NAME="$2"
      shift 2
      ;;
    -h|--help)
      echo "Visual Regression Test Runner"
      echo ""
      echo "Usage: $0 <test-script.ts> [options]"
      echo ""
      echo "Options:"
      echo "  --threshold <n>   Diff percentage threshold (default: 0.1)"
      echo "  --tolerance <n>   Per-channel color tolerance 0-255 (default: 0)"
      echo "  --update          Update baseline even if it exists"
      echo "  --wait <n>        Wait time in seconds for render (default: 2)"
      echo "  --name <name>     Override test name (default: script filename)"
      echo "  --json            Output only JSONL result"
      echo ""
      echo "Exit codes:"
      echo "  0 - Test passed (or baseline created)"
      echo "  1 - Visual regression detected"
      echo "  2 - Test script error"
      echo "  3 - Configuration error"
      exit 0
      ;;
    *)
      if [[ -z "$SCRIPT_PATH" ]]; then
        SCRIPT_PATH="$1"
      else
        echo "ERROR: Unknown argument: $1" >&2
        exit 3
      fi
      shift
      ;;
  esac
done

if [[ -z "$SCRIPT_PATH" ]]; then
  echo "ERROR: No test script specified" >&2
  echo "Usage: $0 <test-script.ts> [options]" >&2
  exit 3
fi

# Get project directory
cd "$(dirname "$0")/.."
PROJECT_DIR=$(pwd)

# Resolve script path
if [[ "$SCRIPT_PATH" = /* ]]; then
  FULL_SCRIPT_PATH="$SCRIPT_PATH"
else
  FULL_SCRIPT_PATH="$PROJECT_DIR/$SCRIPT_PATH"
fi

if [[ ! -f "$FULL_SCRIPT_PATH" ]]; then
  echo "ERROR: Script not found: $FULL_SCRIPT_PATH" >&2
  exit 3
fi

# Determine test name
if [[ -z "$TEST_NAME" ]]; then
  TEST_NAME=$(basename "$SCRIPT_PATH" .ts | sed 's/[^a-zA-Z0-9_-]/-/g')
fi

# Directories
BASELINE_DIR="$PROJECT_DIR/.test-screenshots/baselines"
TEMP_DIR="$PROJECT_DIR/.test-screenshots"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)

# Ensure directories exist
mkdir -p "$BASELINE_DIR"
mkdir -p "$TEMP_DIR"

# Paths
BASELINE_PATH="$BASELINE_DIR/${TEST_NAME}.png"
ACTUAL_PATH="$TEMP_DIR/${TEST_NAME}-${TIMESTAMP}.png"
DIFF_PATH="$BASELINE_DIR/${TEST_NAME}-diff.png"

log() {
  if [[ "$JSON_ONLY" != "true" ]]; then
    echo "$1"
  fi
}

output_json() {
  local status="$1"
  local diff_percent="$2"
  local is_new_baseline="$3"
  local error_msg="$4"

  echo "{\"test\":\"${TEST_NAME}\",\"status\":\"${status}\",\"timestamp\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"diff_percent\":${diff_percent},\"is_new_baseline\":${is_new_baseline},\"baseline\":\"${BASELINE_PATH}\",\"actual\":\"${ACTUAL_PATH}\",\"diff\":\"${DIFF_PATH}\",\"error\":${error_msg:-null}}"
}

# Build the app
log "Building..."
if ! cargo build 2>&1 | grep -v "^warning:" | tail -3; then
  output_json "error" "0" "false" "\"Build failed\""
  exit 2
fi

# Start app with test script
log "Running test: $TEST_NAME"
log "Script: $FULL_SCRIPT_PATH"

# Run the app with the test script, capturing output
APP_OUTPUT=$(mktemp)
echo "{\"type\": \"run\", \"path\": \"$FULL_SCRIPT_PATH\"}" | \
  timeout "${WAIT_SECS}s" ./target/debug/script-kit-gpui 2>"$APP_OUTPUT" &
APP_PID=$!

# Wait for render
sleep "$WAIT_SECS"

# Find and capture the window
log "Capturing screenshot..."

# Try to find script-kit window using CGWindowListCopyWindowInfo
WINDOW_ID=$(swift -e '
import Cocoa
let windowList = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
for window in windowList {
    if let ownerName = window[kCGWindowOwnerName as String] as? String,
       ownerName.contains("script-kit") {
        if let windowID = window[kCGWindowNumber as String] as? Int {
            print(windowID)
            break
        }
    }
}
' 2>/dev/null || echo "")

if [[ -n "$WINDOW_ID" ]]; then
  screencapture -l"$WINDOW_ID" -x -o "$ACTUAL_PATH" 2>/dev/null || screencapture -m "$ACTUAL_PATH"
else
  log "Window not found, capturing main display"
  screencapture -m "$ACTUAL_PATH"
fi

# Kill the app
kill "$APP_PID" 2>/dev/null || true
wait "$APP_PID" 2>/dev/null || true

# Clean up temp file
rm -f "$APP_OUTPUT"

# Verify screenshot was captured
if [[ ! -f "$ACTUAL_PATH" ]]; then
  output_json "error" "0" "false" "\"Screenshot capture failed\""
  exit 2
fi

log "Screenshot saved: $ACTUAL_PATH"

# Check if baseline exists
if [[ ! -f "$BASELINE_PATH" ]] || [[ "$UPDATE_BASELINE" == "true" ]]; then
  # Create/update baseline
  cp "$ACTUAL_PATH" "$BASELINE_PATH"
  log "Baseline created: $BASELINE_PATH"
  output_json "pass" "0" "true" "null"
  exit 0
fi

# Compare against baseline using bun
log "Comparing against baseline..."

DIFF_RESULT=$(bun run "$PROJECT_DIR/tests/autonomous/screenshot-diff.ts" \
  "$BASELINE_PATH" "$ACTUAL_PATH" \
  --tolerance "$TOLERANCE" \
  --threshold "$THRESHOLD" \
  --diff \
  --json 2>&1) || true

# Parse the JSON result
DIFF_PERCENT=$(echo "$DIFF_RESULT" | jq -r '.diff_percent // 100' 2>/dev/null || echo "100")
DIFF_STATUS=$(echo "$DIFF_RESULT" | jq -r '.status // "fail"' 2>/dev/null || echo "fail")

log "Diff: ${DIFF_PERCENT}%"

if [[ "$DIFF_STATUS" == "pass" ]]; then
  log "PASS: Visual regression test passed"
  output_json "pass" "$DIFF_PERCENT" "false" "null"
  # Clean up actual screenshot on pass
  rm -f "$ACTUAL_PATH"
  exit 0
else
  log "FAIL: Visual regression detected"
  log "  Baseline: $BASELINE_PATH"
  log "  Actual: $ACTUAL_PATH"
  log "  Diff: $DIFF_PATH"
  output_json "fail" "$DIFF_PERCENT" "false" "\"Diff ${DIFF_PERCENT}% exceeds threshold ${THRESHOLD}%\""
  exit 1
fi
