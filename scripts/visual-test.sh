#!/bin/bash
# Visual Test Helper for Script Kit GPUI
# Takes a screenshot of the app window for comparison

set -e

SCREENSHOT_DIR="$HOME/dev/script-kit-gpui/screenshots"
mkdir -p "$SCREENSHOT_DIR"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SCREENSHOT_PATH="$SCREENSHOT_DIR/screenshot_$TIMESTAMP.png"

echo "Taking screenshot of Script Kit window..."

# Use screencapture on macOS to capture the frontmost window
# -l captures a specific window by ID, -w captures interactive window selection
# -o removes shadow

# First, try to find the Script Kit window
WINDOW_ID=$(osascript -e 'tell application "System Events" to get id of first window of (first process whose name contains "script-kit-gpui")' 2>/dev/null || echo "")

if [ -n "$WINDOW_ID" ]; then
    echo "Found Script Kit window (ID: $WINDOW_ID)"
    screencapture -l "$WINDOW_ID" -o "$SCREENSHOT_PATH"
else
    echo "Script Kit window not found. Using interactive mode..."
    echo "Click on the Script Kit window to capture it."
    screencapture -w -o "$SCREENSHOT_PATH"
fi

if [ -f "$SCREENSHOT_PATH" ]; then
    echo "Screenshot saved to: $SCREENSHOT_PATH"
    echo ""
    echo "To compare with reference:"
    echo "  open $SCREENSHOT_PATH"
    
    # If ImageMagick is installed, create a diff
    if command -v compare &> /dev/null; then
        REFERENCE="$SCREENSHOT_DIR/reference.png"
        if [ -f "$REFERENCE" ]; then
            DIFF_PATH="$SCREENSHOT_DIR/diff_$TIMESTAMP.png"
            compare "$REFERENCE" "$SCREENSHOT_PATH" "$DIFF_PATH" 2>/dev/null || true
            echo "  Diff saved to: $DIFF_PATH"
        fi
    fi
else
    echo "Failed to capture screenshot"
    exit 1
fi
