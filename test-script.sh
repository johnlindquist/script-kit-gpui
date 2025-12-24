#!/bin/bash
# Helper to test script execution via the running app
# Usage: ./test-script.sh <script-name>

SCRIPT_NAME="${1:-smoke-test-simple.ts}"
CMD_FILE="/tmp/script-kit-gpui-cmd.txt"
LOG_FILE="/var/folders/c3/r013q3_93_s4zycmx0mdnt2h0000gn/T/script-kit-gpui.log"

echo "Testing script: $SCRIPT_NAME"
echo "run:$SCRIPT_NAME" > "$CMD_FILE"

sleep 2

echo ""
echo "=== Recent log entries ==="
grep -E "\[TEST\]|\[EXEC\]" "$LOG_FILE" | tail -10
