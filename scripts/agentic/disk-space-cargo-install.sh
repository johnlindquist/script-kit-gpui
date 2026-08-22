#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="/Users/johnlindquist/dev/script-kit-gpui"
LABEL="com.scriptkit.gpui.disk-space-cargo-watcher"
BIN_DIR="$REPO_ROOT/scripts/agentic/bin"
LOG_DIR="$HOME/Library/Logs/script-kit-gpui"
STATE_DIR="$HOME/Library/Application Support/script-kit-gpui/disk-space-cargo-watcher"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"

mkdir -p "$BIN_DIR" "$LOG_DIR" "$STATE_DIR" "$HOME/Library/LaunchAgents"

# Compile Swift watcher
SWIFTC="$(xcrun --find swiftc 2>/dev/null || command -v swiftc || true)"
if [ -z "$SWIFTC" ]; then
    echo "swiftc not found. Install Apple's Command Line Tools with: xcode-select --install" >&2
    exit 1
fi

echo "Compiling Swift watcher..."
"$SWIFTC" -O \
    -target arm64-apple-macosx16.0 \
    -sdk "$(xcrun --show-sdk-path)" \
    "$REPO_ROOT/scripts/agentic/disk-space-cargo-watcher.swift" \
    -framework CoreServices \
    -o "$BIN_DIR/disk-space-cargo-watcher"
chmod +x "$BIN_DIR/disk-space-cargo-watcher"

# Write launchd plist
cat > "$PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN_DIR/disk-space-cargo-watcher</string>
        <string>--repo</string>
        <string>$REPO_ROOT</string>
        <string>--threshold-gib</string>
        <string>25</string>
        <string>--target-free-gib</string>
        <string>35</string>
        <string>--cooldown-seconds</string>
        <string>1800</string>
        <string>--debounce-seconds</string>
        <string>15</string>
        <string>--fsevent-latency-seconds</string>
        <string>5</string>
        <string>--cleanup</string>
        <string>$REPO_ROOT/scripts/agentic/disk-space-cargo-emergency-clean.sh</string>
        <string>--state-dir</string>
        <string>$STATE_DIR</string>
    </array>
    <key>WorkingDirectory</key>
    <string>$REPO_ROOT</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ThrottleInterval</key>
    <integer>10</integer>
    <key>ProcessType</key>
    <string>Background</string>
    <key>LowPriorityIO</key>
    <true/>
    <key>LowPriorityBackgroundIO</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/disk-space-cargo-watcher.out.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/disk-space-cargo-watcher.err.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOME</key>
        <string>$HOME</string>
        <key>PATH</key>
        <string>/Users/johnlindquist/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
        <key>SCRIPT_KIT_REPO_ROOT</key>
        <string>$REPO_ROOT</string>
        <key>SCRIPT_KIT_WATCHER_STATE_DIR</key>
        <string>$STATE_DIR</string>
        <key>SCRIPT_KIT_FREE_THRESHOLD_GIB</key>
        <string>25</string>
        <key>SCRIPT_KIT_TARGET_FREE_GIB</key>
        <string>35</string>
        <key>SCRIPT_KIT_ALLOW_SHARED_INCREMENTAL_EVICTION</key>
        <string>0</string>
        <key>SCRIPT_KIT_ALLOW_CLEANUP_AGENT</key>
        <string>0</string>
    </dict>
</dict>
</plist>
PLIST

plutil -lint "$PLIST"

# Install launchd agent
launchctl bootout "gui/$(id -u)" "$PLIST" >/dev/null 2>&1 || true
launchctl bootstrap "gui/$(id -u)" "$PLIST"
launchctl enable "gui/$(id -u)/$LABEL"
launchctl kickstart -k "gui/$(id -u)/$LABEL"

echo
echo "Installed $LABEL"
echo "Watcher:"
echo "  $BIN_DIR/disk-space-cargo-watcher"
echo "LaunchAgent:"
echo "  $PLIST"
echo "Logs:"
echo "  $LOG_DIR/disk-space-cargo-watcher.out.log"
echo "  $LOG_DIR/disk-space-cargo-watcher.err.log"
echo "State:"
echo "  $STATE_DIR"
echo
echo "To uninstall:"
echo "  launchctl bootout \"gui/\$(id -u)\" \"$PLIST\" && rm -f \"$PLIST\""
echo
launchctl print "gui/$(id -u)/$LABEL" 2>&1 | head -20
