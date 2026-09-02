#!/bin/bash

# Dev runner script for script-kit-gpui
# Uses cargo-watch to auto-rebuild on Rust file changes.
#
# Visibility: prints a banner at t=0, then a per-iteration heartbeat
# (build/relaunch elapsed seconds) via scripts/agentic/dev-cycle.sh so the
# loop never looks frozen. Screen clearing is opt-in (SCRIPT_KIT_DEV_CLEAR=1).
#
# Log mode:
#   Defaults to SCRIPT_KIT_AI_LOG=1 (compact AI format: SS.mmm|L|C|message)
#   Override with: SCRIPT_KIT_AI_LOG=0 ./dev.sh   (standard verbose logs)
#   Or use:        RUST_LOG=debug ./dev.sh         (debug-level verbose logs)
#
# Flags:
#   --takeover | --force | -f  Kill any previous ./dev.sh watcher (and orphaned
#                              cargo-watch processes for this repo), then start.
#                              Env equivalent: SCRIPT_KIT_DEV_TAKEOVER=1
#   --stop                     Stop the running watcher for this repo and exit.
#                              The app itself is left running; the next dev.sh
#                              build relaunches it via the session contract.
#   --status                   Show lock holder + orphaned watcher state, exit.
#   -h | --help                Show usage.

set -e

# --- Flags and authorization: finish parsing before installing cleanup traps ---
dev_sh_usage() {
    sed -n '/^# Flags:/,/^$/p' "$0" | sed 's/^# \{0,1\}//'
}
SCRIPT_KIT_DEV_TAKEOVER="${SCRIPT_KIT_DEV_TAKEOVER:-0}"
DEV_SH_MODE="run"
DEV_SH_HELP=0
for arg in "$@"; do
    case "$arg" in
        --takeover|--force|-f) mode="takeover" ;;
        --stop) mode="stop" ;;
        --status) mode="status" ;;
        -h|--help) DEV_SH_HELP=1; continue ;;
        *)
            echo "[dev.sh] ERROR unknown flag: $arg" >&2
            dev_sh_usage >&2
            exit 64
            ;;
    esac
    if [[ "$DEV_SH_MODE" != "run" && "$DEV_SH_MODE" != "$mode" ]]; then
        echo "[dev.sh] ERROR conflicting modes: $DEV_SH_MODE and $mode" >&2
        dev_sh_usage >&2
        exit 64
    fi
    DEV_SH_MODE="$mode"
done
if [[ "$DEV_SH_HELP" == "1" ]]; then
    dev_sh_usage
    exit 0
fi
if [[ "$DEV_SH_MODE" != "status" && ( "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" || ! -t 0 || ! -t 2 ) ]]; then
    echo "[dev.sh] REFUSED: watcher start/stop/takeover/recovery requires an interactive human terminal" >&2
    exit 78
fi
if [[ "$DEV_SH_MODE" == "takeover" ]]; then
    SCRIPT_KIT_DEV_TAKEOVER=1
    DEV_SH_MODE="run"
fi

# --- Signal cleanup: one Ctrl+C must stop cargo-watch and all helper children ---
SCRIPT_KIT_DEV_CACHE_PID=""
SCRIPT_KIT_DEV_WATCHDOG_PID=""
DEV_SH_CLEANED_UP=0
DEV_SH_EXIT_CODE=0
SCRIPT_KIT_DEV_LOCK_DIR=""
# Cleanup may remove only a stamp created by this invocation, never an inherited path.
SCRIPT_KIT_DEV_STAMP_FILE=""
dev_sh_pid_alive() {
    local pid="$1"
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}
dev_sh_lock_key() {
    local root="$1"
    if command -v shasum >/dev/null 2>&1; then
        printf '%s' "$root" | shasum -a 1 | awk '{print $1}'
    else
        printf '%s' "$root" | md5 -q
    fi
}
# A registered PID must match its original start time and generation.
dev_sh_pid_is_dev_sh() {
    local lock="${2:-${SCRIPT_KIT_DEV_LOCK_DIR:-${DEV_SH_LOCK_DIR:-}}}" expected actual
    [[ -n "$lock" && -f "$lock/generation" && -f "$lock/process-start" && ! -L "$lock" ]] || return 1
    [[ "$(cat "$lock/pid" 2>/dev/null)" == "$1" ]] || return 1
    expected="$(cat "$lock/process-start")"
    actual="$(LC_ALL=C ps -p "$1" -o lstart= 2>/dev/null | sed 's/^ *//;s/ *$//')"
    [[ -n "$expected" && "$expected" == "$actual" ]]
}
dev_sh_kill_tree() {
    local pid="$1" sig="${2:-TERM}" child identity
    identity="$(LC_ALL=C ps -p "$pid" -o lstart= 2>/dev/null)"
    [[ -n "$identity" ]] || return 0
    for child in $(pgrep -P "$pid" 2>/dev/null || true); do
        dev_sh_kill_tree "$child" "$sig"
    done
    [[ "$(LC_ALL=C ps -p "$pid" -o lstart= 2>/dev/null)" == "$identity" ]] || return 1
    kill "-$sig" "$pid" 2>/dev/null || true
}
# cargo-watch / dev-cycle processes whose cwd is this repo. These get orphaned
# when a previous dev.sh dies via SIGKILL (EXIT trap never runs), and keep
# rebuilding+relaunching invisibly while a new dev.sh fights them.
dev_sh_repo_watcher_pids() {
    local repo_root="$1" pid cwd
    for pid in $(pgrep -f 'cargo-watch watch|scripts/agentic/dev-cycle\.sh' 2>/dev/null || true); do
        [ "$pid" = "$$" ] && continue
        cwd="$(lsof -a -p "$pid" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p' | head -1)"
        [ "$cwd" = "$repo_root" ] && echo "$pid"
    done
    return 0
}
dev_sh_write_lock() {
    printf '%s\n' "$$" > "$SCRIPT_KIT_DEV_LOCK_DIR/pid"
    printf '%s\n' "${SCRIPT_KIT_DEV_SESSION_NAME:-dev-watch}" > "$SCRIPT_KIT_DEV_LOCK_DIR/session"
    printf '%s\n' "$1" > "$SCRIPT_KIT_DEV_LOCK_DIR/root"
    LC_ALL=C ps -p "$$" -o lstart= | sed 's/^ *//;s/ *$//' > "$SCRIPT_KIT_DEV_LOCK_DIR/process-start"
    python3 -c 'import uuid; print(uuid.uuid4())' > "$SCRIPT_KIT_DEV_LOCK_DIR/generation"
    SCRIPT_KIT_DEV_LOCK_GENERATION="$(cat "$SCRIPT_KIT_DEV_LOCK_DIR/generation")"
    export SCRIPT_KIT_DEV_LOCK_GENERATION
}
# Stop only a registered lifetime. Name/CWD census is diagnostic, never authority.
dev_sh_stop_existing() {
    local repo_root="$1" lock_dir="$2" old_pid expected_generation i
    [[ -d "$lock_dir" && ! -L "$lock_dir" ]] || return 0
    old_pid="$(cat "$lock_dir/pid" 2>/dev/null || true)"
    expected_generation="$(cat "$lock_dir/generation" 2>/dev/null || true)"
    if [[ -z "$expected_generation" ]] || ! dev_sh_pid_is_dev_sh "$old_pid" "$lock_dir"; then
        echo "[dev.sh] REFUSED: unknown/stale/reused watcher lease remains protected: $lock_dir" >&2
        return 78
    fi
    kill -TERM "$old_pid"
    for (( i=0; i<100; i++ )); do
        if ! dev_sh_pid_alive "$old_pid"; then break; fi
        sleep 0.1
    done
    if dev_sh_pid_alive "$old_pid"; then
        echo "[dev.sh] REFUSED: registered watcher teardown not confirmed" >&2; return 78
    fi
    # The original launcher's cleanup removes its own exact generation.
    [[ ! -e "$lock_dir" ]] || { echo "[dev.sh] REFUSED: watcher cleanup record remains" >&2; return 78; }
}
dev_sh_acquire_lock() {
    local lock_root="/tmp/sk-dev-launcher-locks"
    local repo_root
    repo_root="$(pwd -P)"
    mkdir -p "$lock_root"
    SCRIPT_KIT_DEV_LOCK_DIR="${lock_root}/$(dev_sh_lock_key "$repo_root").lock"
    if [[ "${SCRIPT_KIT_DEV_ALLOW_MULTI:-0}" == "1" ]]; then
        SCRIPT_KIT_DEV_LOCK_DIR="${lock_root}/$(dev_sh_lock_key "$repo_root")-$$.lock"
    fi

    if mkdir "$SCRIPT_KIT_DEV_LOCK_DIR" 2>/dev/null; then
        dev_sh_write_lock "$repo_root"
        return 0
    fi

    local old_pid=""
    old_pid="$(cat "$SCRIPT_KIT_DEV_LOCK_DIR/pid" 2>/dev/null || true)"
    if dev_sh_pid_alive "$old_pid" && dev_sh_pid_is_dev_sh "$old_pid"; then
        if [ "${SCRIPT_KIT_DEV_TAKEOVER:-0}" = "1" ]; then
            dev_sh_stop_existing "$repo_root" "$SCRIPT_KIT_DEV_LOCK_DIR"
        else
            echo "[dev.sh] ERROR another ./dev.sh is already running for this repo: pid=${old_pid} session=$(cat "$SCRIPT_KIT_DEV_LOCK_DIR/session" 2>/dev/null || echo '?')" >&2
            echo "[dev.sh] Take it over with: ./dev.sh --takeover   (or stop it: ./dev.sh --stop)" >&2
            echo "[dev.sh] Or set SCRIPT_KIT_DEV_ALLOW_MULTI=1 if you intentionally want multiple watchers." >&2
            exit 2
        fi
    else
        echo "[dev.sh] REFUSED: incomplete or stale watcher registration remains protected: ${SCRIPT_KIT_DEV_LOCK_DIR}" >&2
        exit 78
    fi

    mkdir "$SCRIPT_KIT_DEV_LOCK_DIR"
    dev_sh_write_lock "$repo_root"
}
dev_sh_cleanup() {
    if [ "$DEV_SH_CLEANED_UP" = "1" ]; then
        return 0
    fi
    DEV_SH_CLEANED_UP=1
    trap - INT TERM EXIT

    if [ -n "${SCRIPT_KIT_DEV_CACHE_PID:-}" ]; then
        kill "$SCRIPT_KIT_DEV_CACHE_PID" 2>/dev/null || true
        wait "$SCRIPT_KIT_DEV_CACHE_PID" 2>/dev/null || true
    fi

    if [ -n "${SCRIPT_KIT_DEV_WATCHDOG_PID:-}" ]; then
        kill "$SCRIPT_KIT_DEV_WATCHDOG_PID" 2>/dev/null || true
        wait "$SCRIPT_KIT_DEV_WATCHDOG_PID" 2>/dev/null || true
    fi

    # Revalidate each direct child/tree lifetime before signalling.
    for child in $(pgrep -P "$$" 2>/dev/null || true); do dev_sh_kill_tree "$child" TERM; done

    rm -f "$SCRIPT_KIT_DEV_STAMP_FILE" 2>/dev/null || true

    if [[ -n "${SCRIPT_KIT_DEV_LOCK_DIR:-}" && -n "${SCRIPT_KIT_DEV_LOCK_GENERATION:-}" ]] \
        && dev_sh_pid_is_dev_sh "$$" "$SCRIPT_KIT_DEV_LOCK_DIR" \
        && [[ "$(cat "$SCRIPT_KIT_DEV_LOCK_DIR/generation")" == "$SCRIPT_KIT_DEV_LOCK_GENERATION" ]]; then
        rm -f "$SCRIPT_KIT_DEV_LOCK_DIR/pid" "$SCRIPT_KIT_DEV_LOCK_DIR/root" "$SCRIPT_KIT_DEV_LOCK_DIR/session" "$SCRIPT_KIT_DEV_LOCK_DIR/process-start" "$SCRIPT_KIT_DEV_LOCK_DIR/generation"
        rmdir "$SCRIPT_KIT_DEV_LOCK_DIR" 2>/dev/null || true
    fi

    if [ "$DEV_SH_EXIT_CODE" -ne 0 ]; then
        exit "$DEV_SH_EXIT_CODE"
    fi
}
dev_sh_on_signal() {
    local sig="$1"
    case "$sig" in
        INT) DEV_SH_EXIT_CODE=130 ;;
        TERM) DEV_SH_EXIT_CODE=143 ;;
    esac
    dev_sh_cleanup
}

if [ "$DEV_SH_MODE" != "run" ]; then
    DEV_SH_REPO_ROOT="$(pwd -P)"
    DEV_SH_LOCK_DIR="/tmp/sk-dev-launcher-locks/$(dev_sh_lock_key "$DEV_SH_REPO_ROOT").lock"
    DEV_SH_LOCK_PID="$(cat "$DEV_SH_LOCK_DIR/pid" 2>/dev/null || true)"
    case "$DEV_SH_MODE" in
        status)
            if [ -n "$DEV_SH_LOCK_PID" ] && dev_sh_pid_alive "$DEV_SH_LOCK_PID" && dev_sh_pid_is_dev_sh "$DEV_SH_LOCK_PID"; then
                echo "[dev.sh] RUNNING pid=${DEV_SH_LOCK_PID} session=$(cat "$DEV_SH_LOCK_DIR/session" 2>/dev/null || echo '?') tty=$(ps -p "$DEV_SH_LOCK_PID" -o tty= 2>/dev/null | tr -d ' ')"
            elif [ -d "$DEV_SH_LOCK_DIR" ]; then
                echo "[dev.sh] STALE lock present but pid ${DEV_SH_LOCK_PID:-?} is not a live dev.sh (next start clears it)"
            else
                echo "[dev.sh] NOT RUNNING (no lock)"
            fi
            DEV_SH_ORPHANS="$(dev_sh_repo_watcher_pids "$DEV_SH_REPO_ROOT")"
            if [ -n "$DEV_SH_ORPHANS" ]; then
                echo "[dev.sh] watcher processes for this repo:"
                # shellcheck disable=SC2086
                ps -o pid,ppid,etime,command -p $DEV_SH_ORPHANS 2>/dev/null || true
            fi
            exit 0
            ;;
        stop)
            if [ ! -d "$DEV_SH_LOCK_DIR" ] && [ -z "$(dev_sh_repo_watcher_pids "$DEV_SH_REPO_ROOT")" ]; then
                echo "[dev.sh] nothing to stop (no lock, no watcher processes)"
                exit 0
            fi
            dev_sh_stop_existing "$DEV_SH_REPO_ROOT" "$DEV_SH_LOCK_DIR"
            echo "[dev.sh] stopped. The app itself (if running) was left alone."
            exit 0
            ;;
    esac
fi

trap 'dev_sh_on_signal INT' INT
trap 'dev_sh_on_signal TERM' TERM
trap dev_sh_cleanup EXIT

# --- Banner FIRST so the user sees activity within ~1s, before any du scan ---
echo "[dev.sh] start t=$(date '+%Y-%m-%dT%H:%M:%S%z') pid=$$"
echo "[dev.sh] First build may take several minutes; subsequent rebuilds are incremental."
echo "[dev.sh] Build output is visible (--message-format=short). Press Ctrl+C to stop."
echo ""

# Default to compact AI log mode unless explicitly overridden
export SCRIPT_KIT_AI_LOG="${SCRIPT_KIT_AI_LOG:-1}"

# Default cargo features for the dev build. local-llm enables the on-device
# ghost-text predictor (Notes inline completions). Opt out with an explicit
# empty value: SCRIPT_KIT_CARGO_FEATURES="" ./dev.sh
export SCRIPT_KIT_CARGO_FEATURES="${SCRIPT_KIT_CARGO_FEATURES-local-llm}"

# Dev startup profile: optimize for time-to-usable-session during cargo-watch loops.
export SCRIPT_KIT_STARTUP_PROFILE="${SCRIPT_KIT_STARTUP_PROFILE:-dev-fast}"
export SCRIPT_KIT_DEFER_SCHEDULER_STARTUP="${SCRIPT_KIT_DEFER_SCHEDULER_STARTUP:-1}"
export SCRIPT_KIT_STARTUP_READY_LOG="${SCRIPT_KIT_STARTUP_READY_LOG:-1}"
export SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM="${SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM:-0}"
export SCRIPT_KIT_DISABLE_QUICK_TERMINAL_WARM_PTY="${SCRIPT_KIT_DISABLE_QUICK_TERMINAL_WARM_PTY:-1}"
export SCRIPT_KIT_DEV_MARKER_HOTKEY="${SCRIPT_KIT_DEV_MARKER_HOTKEY:-1}"

# Agentic session name: dev.sh launches through the reusable session contract.
export SCRIPT_KIT_DEV_SESSION_NAME="${SCRIPT_KIT_DEV_SESSION_NAME:-dev-watch}"
SESSION_DIR_RAW="${SCRIPT_KIT_SESSION_DIR:-/tmp/sk-agentic-sessions}"
mkdir -p "$SESSION_DIR_RAW"
export SCRIPT_KIT_SESSION_DIR="$(cd "$SESSION_DIR_RAW" && pwd -P)"

if [ "${SCRIPT_KIT_DEV_ALLOW_MULTI:-0}" = "1" ]; then
    echo "[dev.sh] WARNING explicitly authorized multiple watchers"
fi
dev_sh_acquire_lock

# --- Launcher self-update stamp ---------------------------------------------
# Record a digest of the launcher + helper scripts at start. dev-cycle.sh will
# compare this against the live files each iteration and suggest a restart if
# they have changed under the running cargo-watch process.
SCRIPT_KIT_DEV_STAMP_DIR="${TMPDIR:-/tmp}/sk-dev-launcher-stamps"
mkdir -p "$SCRIPT_KIT_DEV_STAMP_DIR"
SCRIPT_KIT_DEV_STAMP_FILE="${SCRIPT_KIT_DEV_STAMP_DIR}/$$.stamp"
shasum_of() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 1 "$@" 2>/dev/null | awk '{print $1}' | paste -sd, -
    else
        md5 -q "$@" 2>/dev/null | paste -sd, -
    fi
}
shasum_of dev.sh scripts/agentic/dev-cycle.sh scripts/agentic/dev-relaunch.sh > "$SCRIPT_KIT_DEV_STAMP_FILE"
export SCRIPT_KIT_DEV_STAMP_FILE

# --- Async cache-size reporter -----------------------------------------------
# Reporting target/ size synchronously can take 10+ seconds with a 77 GB
# target/, which is the bulk of the "frozen" silence. Run it in the background
# so the heartbeat starts immediately. Result lands on stderr when ready.
SCRIPT_KIT_TARGET_CLEAN_THRESHOLD_GB="${SCRIPT_KIT_TARGET_CLEAN_THRESHOLD_GB:-50}"
SCRIPT_KIT_TARGET_AGENT_THRESHOLD_GB="${SCRIPT_KIT_TARGET_AGENT_THRESHOLD_GB:-50}"
if [ "${SCRIPT_KIT_REPORT_CACHE_SIZE:-1}" = "1" ]; then
    bun scripts/devtools/devtools.ts build-ops query storage >&2 &
    SCRIPT_KIT_DEV_CACHE_PID=$!
fi

# --- Pi sidecar availability ---------------------------------------------------
# Dev runs execute the bare target binary, so the bundled Contents/MacOS/pi
# never resolves. Make sure the repo-local Pi sidecar exists for Agent Chat
# (cmd+enter) before launching the app; otherwise the first session can start
# with Pi unavailable while the sidecar is still building in the background.
if [ "${SCRIPT_KIT_DEV_ENSURE_PI_SIDECAR:-1}" = "1" ]; then
    bash scripts/agentic/ensure-pi-sidecar.sh
fi

# --- Crash watchdog -----------------------------------------------------------
# Optional supervisor for the session app pid: loud banner + exact relaunch on
# abnormal death; repeated crashes stop without deleting caches or touching source.
# off by default so using Script Kit's Quit command during dev leaves the app
# stopped instead of being silently relaunched by dev.sh.
SCRIPT_KIT_DEV_CRASH_WATCHDOG="${SCRIPT_KIT_DEV_CRASH_WATCHDOG:-0}"
if [ "$SCRIPT_KIT_DEV_CRASH_WATCHDOG" = "1" ]; then
    bash scripts/agentic/dev-crash-watchdog.sh "$SCRIPT_KIT_DEV_SESSION_NAME" &
    SCRIPT_KIT_DEV_WATCHDOG_PID=$!
fi

# Check if cargo-watch is installed
if ! command -v cargo-watch &> /dev/null; then
    echo "cargo-watch is not installed"
    echo ""
    echo "Install it with:"
    echo "  cargo install cargo-watch"
    echo ""
    exit 1
fi

echo "[dev.sh] cargo-watch ready"
echo "   Watching: src/, scripts/kit-sdk.ts, Cargo.toml, build.rs"
if [ "$SCRIPT_KIT_AI_LOG" = "1" ]; then
    echo "   Log mode: compact AI (SS.mmm|L|C|message). Override: SCRIPT_KIT_AI_LOG=0 ./dev.sh"
else
    echo "   Log mode: standard verbose"
fi
echo "   Agentic session: ${SCRIPT_KIT_DEV_SESSION_NAME}"
echo "   Startup profile: ${SCRIPT_KIT_STARTUP_PROFILE}"
echo "   Cargo features: ${SCRIPT_KIT_CARGO_FEATURES:-(none)} (opt out: SCRIPT_KIT_CARGO_FEATURES=\"\" ./dev.sh)"
echo "   Quick Terminal warm PTY: disabled"
echo "   Cargo dev profile: debug=0 incremental=true codegen-units=256"
echo "   Build target: script-kit-gpui only (skips smoke-test, vibrancy-poc, menu-syntax-doctor)"
echo "   Session log: ~/.scriptkit/logs/latest-session.jsonl"
if [ "$SCRIPT_KIT_DEV_MARKER_HOTKEY" = "1" ]; then
    echo "   Dev marker: Ctrl+M writes one structured marker + screenshot path, then opens Notes for context"
fi
echo "   Clear screen between rebuilds: SCRIPT_KIT_DEV_CLEAR=${SCRIPT_KIT_DEV_CLEAR:-0} (set =1 to enable cargo-watch -c)"
echo "   Crash watchdog: SCRIPT_KIT_DEV_CRASH_WATCHDOG=${SCRIPT_KIT_DEV_CRASH_WATCHDOG} (set =1 for banner + auto-relaunch on app crash)"
echo ""

# Clear-screen is opt-in. cargo-watch -c wipes the heartbeat output, so default
# it to OFF and let the user opt in with SCRIPT_KIT_DEV_CLEAR=1.
cargo_watch_args=()
if [ "${SCRIPT_KIT_DEV_CLEAR:-0}" = "1" ] && [ -t 1 ] && [ -n "${TERM:-}" ] && [ "${TERM}" != "dumb" ]; then
    cargo_watch_args+=(-c)
fi

# --no-restart prevents killing an in-flight build when the user saves again
# mid-compile. Debounce coalesces rapid save bursts.
cargo_watch_args+=(--no-restart -d "${SCRIPT_KIT_CARGO_WATCH_DELAY:-1.0}")

# Delegate the build+relaunch iteration to dev-cycle.sh so we get:
#   - timestamped "build start"/"build done in Xs"
#   - per-second heartbeat with elapsed seconds
#   - relaunch elapsed + ready marker
#   - skip-relaunch when binary mtime unchanged and session is healthy
compiler_watch_args=()
while IFS= read -r compiler_input || [[ -n "$compiler_input" ]]; do
    case "$compiler_input" in ""|/*|*..*) echo "[dev.sh] invalid compiler-input path" >&2; exit 78 ;; esac
    [[ ! -e "$compiler_input" ]] || compiler_watch_args+=(-w "$compiler_input")
done < scripts/agentic/compiler-input-paths.txt
cargo watch "${cargo_watch_args[@]}" -s "bash scripts/agentic/dev-cycle.sh" "${compiler_watch_args[@]}"
