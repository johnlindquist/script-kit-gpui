#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="${SCRIPT_KIT_REPO_ROOT:-/Users/johnlindquist/dev/script-kit-gpui}"
STATE_DIR="${SCRIPT_KIT_WATCHER_STATE_DIR:-$HOME/Library/Application Support/script-kit-gpui/disk-space-cargo-watcher}"
THRESHOLD_GIB="${SCRIPT_KIT_FREE_THRESHOLD_GIB:-25}"
TARGET_FREE_GIB="${SCRIPT_KIT_TARGET_FREE_GIB:-35}"
CLAUDE_BIN="${CLAUDE_BIN:-/Users/johnlindquist/.local/bin/claude}"
REASON="manual"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) REPO_ROOT="$2"; shift 2 ;;
        --state-dir) STATE_DIR="$2"; shift 2 ;;
        --threshold-gib) THRESHOLD_GIB="$2"; shift 2 ;;
        --target-free-gib) TARGET_FREE_GIB="$2"; shift 2 ;;
        --reason) REASON="$2"; shift 2 ;;
        --help|-h)
            cat <<EOF
Usage: $0 --repo /path/to/repo --threshold-gib 25 --target-free-gib 35 --state-dir /path/to/state --reason fsevents
EOF
            exit 0
            ;;
        *) echo "[claude-cleanup] unknown argument: $1" >&2; exit 2 ;;
    esac
done

PATH="/Users/johnlindquist/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export PATH

mkdir -p "$STATE_DIR"
cd "$REPO_ROOT"

log() { echo "[claude-cleanup] $(date '+%Y-%m-%dT%H:%M:%S%z') $*" >&2; }

free_gib() {
    df -Pk "$REPO_ROOT" | awk 'NR==2 { printf "%.1f", $4 / 1048576 }'
}

ge_float() { awk -v a="$1" -v b="$2" 'BEGIN { exit(a >= b ? 0 : 1) }'; }

shell_quote() { printf "%q" "$1"; }

# --- Lock ---

LOCK_DIR="$STATE_DIR/cleanup.lock"

acquire_lock() {
    if mkdir "$LOCK_DIR" 2>/dev/null; then
        echo "$$" > "$LOCK_DIR/pid"
        date '+%Y-%m-%dT%H:%M:%S%z' > "$LOCK_DIR/started_at"
        echo "$REASON" > "$LOCK_DIR/reason"
        return 0
    fi

    if [ -f "$LOCK_DIR/pid" ]; then
        local old_pid
        old_pid="$(cat "$LOCK_DIR/pid" 2>/dev/null || true)"
        if [ -n "$old_pid" ] && kill -0 "$old_pid" 2>/dev/null; then
            log "cleanup already running pid=$old_pid"
            exit 0
        fi
    fi

    log "removing stale lock $LOCK_DIR"
    rm -rf "$LOCK_DIR"
    mkdir "$LOCK_DIR"
    echo "$$" > "$LOCK_DIR/pid"
    date '+%Y-%m-%dT%H:%M:%S%z' > "$LOCK_DIR/started_at"
    echo "$REASON" > "$LOCK_DIR/reason"
}

acquire_lock
trap 'rm -rf "$LOCK_DIR"' EXIT INT TERM

# --- Pre-flight ---

CURRENT_FREE="$(free_gib)"
if ge_float "$CURRENT_FREE" "$THRESHOLD_GIB"; then
    log "free disk recovered before Claude launch free=${CURRENT_FREE}GiB threshold=${THRESHOLD_GIB}GiB"
    exit 0
fi

# Existing installed watcher binaries still invoke this legacy path. Make the
# safe deterministic helper effective immediately without restarting a shared
# LaunchAgent or spawning an unattended AI session on a busy machine.
if [[ "${SCRIPT_KIT_ALLOW_CLEANUP_AGENT:-0}" != "1" ]]; then
    log "running deterministic lock-safe cleanup directly; background AI delegation is disabled"
    set +e
    bash "$REPO_ROOT/scripts/agentic/disk-space-cargo-emergency-clean.sh" \
        --apply \
        --repo "$REPO_ROOT" \
        --threshold-gib "$THRESHOLD_GIB" \
        --target-free-gib "$TARGET_FREE_GIB" \
        --state-dir "$STATE_DIR" \
        --reason "$REASON"
    status=$?
    set -e
    exit "$status"
fi

if [ ! -x "$CLAUDE_BIN" ]; then
    log "claude binary not executable: $CLAUDE_BIN"
    exit 127
fi

if ! "$CLAUDE_BIN" auth status >/dev/null 2>&1; then
    log "claude auth status failed; run '$CLAUDE_BIN auth login' in an interactive terminal"
    exit 78
fi

# --- Build prompt ---

PROMPT_FILE="$REPO_ROOT/scripts/agentic/disk-space-cargo-cleanup-prompt.md"
SETTINGS_FILE="$REPO_ROOT/scripts/agentic/disk-space-cargo-claude-settings.json"

HELPER_CMD="bash scripts/agentic/disk-space-cargo-emergency-clean.sh --apply --repo $(shell_quote "$REPO_ROOT") --threshold-gib $(shell_quote "$THRESHOLD_GIB") --target-free-gib $(shell_quote "$TARGET_FREE_GIB") --state-dir $(shell_quote "$STATE_DIR") --reason $(shell_quote "$REASON")"

BEFORE_REPORT="$(
    {
        echo "date=$(date '+%Y-%m-%dT%H:%M:%S%z')"
        echo "repo=$REPO_ROOT"
        echo "reason=$REASON"
        echo "threshold=${THRESHOLD_GIB}GiB"
        echo "target_free=${TARGET_FREE_GIB}GiB"
        echo
        df -h "$REPO_ROOT"
        echo
        du -sh target target/debug target/debug/incremental target-agent target-agent/pools target-agent/agents target-agent/runtime 2>/dev/null || true
        echo
        if [ -d target-agent/.locks ]; then
            find target-agent/.locks -mindepth 1 -maxdepth 2 -type f -name pid \
                -print -exec sh -c 'printf "  "; cat "$1"; printf "\n"' sh {} \; \
                2>/dev/null || true
        fi
    } 2>&1
)"

PROMPT="$(
    cat "$PROMPT_FILE"
    cat <<EOF

## Runtime invocation

Repo root:
\`\`\`
$REPO_ROOT
\`\`\`

Current state:
\`\`\`
$BEFORE_REPORT
\`\`\`

Run this exact helper command now:
\`\`\`bash
$HELPER_CMD
\`\`\`

After it completes, verify:
\`\`\`bash
df -h .
du -sh target target-agent 2>/dev/null || true
\`\`\`
EOF
)"

# --- Launch Claude ---

SESSION_LOG="$STATE_DIR/claude-cleanup-$(date '+%Y%m%dT%H%M%S').jsonl"
LATEST_LOG="$STATE_DIR/latest-claude-cleanup.jsonl"

log "launching Claude cleanup free=${CURRENT_FREE}GiB log=$SESSION_LOG"

export CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1
export CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1
export BASH_DEFAULT_TIMEOUT_MS=600000
export BASH_MAX_TIMEOUT_MS=1200000
export BASH_MAX_OUTPUT_LENGTH=150000

set +e
"$CLAUDE_BIN" \
    --disable-slash-commands \
    --settings "$SETTINGS_FILE" \
    --permission-mode dontAsk \
    --tools "Bash,Read" \
    --name "disk-space-cargo-cleanup" \
    --max-turns 8 \
    --output-format stream-json \
    --verbose \
    -p "$PROMPT" >> "$SESSION_LOG" 2>&1
status=$?
set -e

ln -sf "$SESSION_LOG" "$LATEST_LOG" 2>/dev/null || true

AFTER_FREE="$(free_gib)"
log "Claude exited status=$status free_after=${AFTER_FREE}GiB log=$SESSION_LOG"

if ge_float "$AFTER_FREE" "$THRESHOLD_GIB"; then
    exit 0
fi

exit "$status"
