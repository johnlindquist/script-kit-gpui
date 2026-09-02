#!/usr/bin/env bash
# scripts/agentic/dev-cycle.sh — one build + conditional relaunch iteration for dev.sh.
#
# Prints timestamped [dev.sh] build start / build N s elapsed… / build done in Xs
# heartbeat lines on stderr so the loop never looks frozen, then relaunches the
# reusable agentic session only when the binary actually changed (or the session
# is unhealthy). Designed to be invoked by `cargo watch -s "bash dev-cycle.sh"`.
#
# Env knobs:
#   SCRIPT_KIT_DEV_SESSION_NAME   — session name (default: dev-watch)
#   SCRIPT_KIT_CARGO_MESSAGE_FORMAT — cargo --message-format value (default: short)
#   SCRIPT_KIT_USE_SCCACHE=1      — wrap rustc in sccache if available
#   SCRIPT_KIT_USE_LLD=1          — opt-in Homebrew llvm ld64.lld linker
#   SCRIPT_KIT_DEV_FORCE_RELAUNCH=1 — relaunch even if binary mtime unchanged

set -euo pipefail

if [[ "${1:-}" == "--linker" ]]; then
    shift
    exec clang "-fuse-ld=${SCRIPT_KIT_DEV_LLD:?missing registered LLD path}" "$@"
fi
if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" ]]; then
    echo "[dev-cycle] REFUSED: interactive human watcher only" >&2
    exit 78
fi
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"
export SCRIPT_KIT_GPUI_BINARY="${PROJECT_ROOT}/target/debug/script-kit-gpui"

# Track heartbeat so exit cannot leave a spinner loop running.
# Do not trap INT/TERM here — that would swallow Ctrl+C before it reaches the
# foreground cargo build. EXIT covers normal completion; run_with_heartbeat also
# calls dev_cycle_cleanup after the build command returns.
DEV_CYCLE_HB_PID=""
dev_cycle_cleanup() {
    if [ -n "${DEV_CYCLE_HB_PID:-}" ]; then
        kill "$DEV_CYCLE_HB_PID" 2>/dev/null || true
        wait "$DEV_CYCLE_HB_PID" 2>/dev/null || true
        DEV_CYCLE_HB_PID=""
    fi
    clear_tty_line
}
trap 'dev_cycle_status=$?; dev_cycle_cleanup; exit "$dev_cycle_status"' EXIT

SESSION_NAME="${SCRIPT_KIT_DEV_SESSION_NAME:-dev-watch}"
SESSION_SCRIPT="scripts/agentic/session.sh"
RELAUNCH_SCRIPT="scripts/agentic/dev-relaunch.sh"
BIN_PATH="target/debug/script-kit-gpui"

ts() {
    date '+%Y-%m-%dT%H:%M:%S%z'
}

mtime() {
    if [ -e "$1" ]; then
        stat -f %m "$1" 2>/dev/null || echo 0
    else
        echo 0
    fi
}

clear_tty_line() {
    if [ -t 2 ]; then
        printf '\r%*s\r' 100 '' >&2
    fi
}

heartbeat() {
    local label="$1"
    local start="$2"
    local spin='|/-\'
    local i=0
    while true; do
        local elapsed=$((SECONDS - start))
        if [ -t 2 ]; then
            printf '\r[dev.sh] %s %ss elapsed… %s' "$label" "$elapsed" "${spin:i++%4:1}" >&2
        else
            echo "[dev.sh] ${label} ${elapsed}s elapsed…" >&2
        fi
        sleep 1
    done
}

run_with_heartbeat() {
    local label="$1"
    shift
    local start=$SECONDS
    echo "[dev.sh] ${label} start t=$(ts) cmd=$*" >&2
    # Run the build in the foreground so SIGINT reaches cargo/rustc directly.
    # Only the heartbeat spinner is backgrounded; dev_cycle_cleanup kills it on exit.
    heartbeat "$label" "$start" &
    DEV_CYCLE_HB_PID=$!
    set +e
    "$@"
    local status=$?
    set -e
    dev_cycle_cleanup
    echo "[dev.sh] ${label} done in $((SECONDS - start))s status=${status}" >&2
    return "$status"
}

session_healthy() {
    local name="$1"
    local result
    result="$(bash "$SESSION_SCRIPT" status "$name" 2>/dev/null || true)"
    RESULT_JSON="$result" python3 - <<'PY'
import json, os, sys
try:
    data = json.loads(os.environ.get("RESULT_JSON", ""))
except Exception:
    raise SystemExit(1)
healthy = data.get("healthy")
if healthy is None:
    healthy = data.get("status") in ("ready", "running", "started")
raise SystemExit(0 if healthy else 1)
PY
}

run_relaunch() {
    local start=$SECONDS
    echo "[dev.sh] relaunch start t=$(ts) session=${SESSION_NAME}" >&2
    set +e
    local result
    result="$(bash "$RELAUNCH_SCRIPT" "$SESSION_NAME")"
    local status=$?
    set -e
    printf '%s\n' "$result"
    RESULT_JSON="$result" ELAPSED="$((SECONDS - start))" STATUS="$status" python3 - <<'PY'
import json, os, sys
elapsed = os.environ.get("ELAPSED", "?")
status = os.environ.get("STATUS", "?")
try:
    data = json.loads(os.environ.get("RESULT_JSON", ""))
except Exception as exc:
    print(f"[dev.sh] relaunch done in {elapsed}s status={status} ready=? readyMarker=? parse_error={exc}", file=sys.stderr)
    raise SystemExit(0)
print(
    "[dev.sh] relaunch done "
    f"in {elapsed}s status={status} "
    f"ready={data.get('ready')} "
    f"readyMarker={data.get('readyMarker')} "
    f"readyWaitMs={data.get('readyWaitMs')} "
    f"session={data.get('session')}",
    file=sys.stderr,
)
PY
    return "$status"
}

if [[ -n "${CARGO_TARGET_DIR:-}" && "$CARGO_TARGET_DIR" != "${PROJECT_ROOT}/target" ]]; then
    echo "[dev.sh] REFUSED conflicting inherited CARGO_TARGET_DIR=${CARGO_TARGET_DIR}" >&2
    exit 78
fi
export CARGO_TARGET_DIR="${PROJECT_ROOT}/target"

# --- Suggestion 1: stale launcher ------------------------------------------
# If dev.sh / dev-cycle.sh / dev-relaunch.sh have changed since the running
# cargo-watch was started, the loop is executing this iteration with the OLD
# launcher behavior. Tell the user (or the watching AI agent) to restart.
suggest_restart_if_launcher_stale() {
    local stamp_file="${SCRIPT_KIT_DEV_STAMP_FILE:-}"
    [ -z "$stamp_file" ] && return 0
    [ ! -f "$stamp_file" ] && return 0
    local recorded current
    recorded="$(cat "$stamp_file" 2>/dev/null || true)"
    if command -v shasum >/dev/null 2>&1; then
        current="$(shasum -a 1 dev.sh scripts/agentic/dev-cycle.sh scripts/agentic/dev-relaunch.sh 2>/dev/null | awk '{print $1}' | paste -sd, -)"
    else
        current="$(md5 -q dev.sh scripts/agentic/dev-cycle.sh scripts/agentic/dev-relaunch.sh 2>/dev/null | paste -sd, -)"
    fi
    if [ -n "$current" ] && [ "$current" != "$recorded" ]; then
        echo "[dev.sh] SUGGEST launcher scripts changed since this cargo-watch started." >&2
        echo "[dev.sh] ACTION: stop this dev.sh and restart it; current loop is using old launcher behavior." >&2
    fi
}
suggest_restart_if_launcher_stale

if [ "${SCRIPT_KIT_USE_SCCACHE:-0}" = "1" ]; then
    if [[ -n "${RUSTC_WRAPPER:-}" && "$RUSTC_WRAPPER" != "sccache" && "${RUSTC_WRAPPER##*/}" != "sccache" ]]; then
        echo "[dev.sh] REFUSED conflicting RUSTC_WRAPPER" >&2; exit 78
    fi
    if command -v sccache >/dev/null 2>&1; then
        export RUSTC_WRAPPER=sccache
        echo "[dev.sh] sccache enabled" >&2
    else
        echo "[dev.sh] warning: SCRIPT_KIT_USE_SCCACHE=1 but sccache is not on PATH" >&2
    fi
fi

if [ "${SCRIPT_KIT_USE_LLD:-0}" = "1" ]; then
    llvm_prefix="$(brew --prefix llvm 2>/dev/null || true)"
    if [ -n "$llvm_prefix" ] && [ -x "${llvm_prefix}/bin/ld64.lld" ]; then
        host="$(rustc -vV | sed -n 's/^host: //p')"
        linker_key="CARGO_TARGET_$(printf '%s' "$host" | tr '[:lower:]-' '[:upper:]_')_LINKER"
        if [[ -n "${!linker_key:-}" || -n "${RUSTFLAGS:-}" || -n "${CARGO_ENCODED_RUSTFLAGS:-}" ]]; then
            echo "[dev.sh] REFUSED conflicting explicit linker/rustflags configuration" >&2; exit 78
        fi
        export SCRIPT_KIT_DEV_LLD="${llvm_prefix}/bin/ld64.lld"
        adapter_dir="${SCRIPT_KIT_DEV_STAMP_DIR:-${TMPDIR:-/tmp}/sk-dev-launcher-stamps}"
        mkdir -p "$adapter_dir"
        adapter="${adapter_dir}/lld-adapter-${PPID}"
        printf '#!/bin/bash\nexec bash %q --linker "$@"\n' "${PROJECT_ROOT}/scripts/agentic/dev-cycle.sh" > "$adapter"
        chmod 700 "$adapter"
        export "${linker_key}=${adapter}"
        echo "[dev.sh] LLD enabled ld64.lld=${llvm_prefix}/bin/ld64.lld" >&2
    else
        echo "[dev.sh] warning: SCRIPT_KIT_USE_LLD=1 but Homebrew llvm ld64.lld was not found" >&2
    fi
fi

before_mtime="$(mtime "$BIN_PATH")"

# Opt-in cargo features for the dev build (space- or comma-separated). Lets the
# dev loop enable optional features like local-llm:
#   SCRIPT_KIT_CARGO_FEATURES=local-llm ./dev.sh
build_args=(build --locked --bin script-kit-gpui)
if [ -n "${SCRIPT_KIT_CARGO_FEATURES:-}" ]; then
    build_args+=(--features "$SCRIPT_KIT_CARGO_FEATURES")
fi
build_args+=(--message-format="${SCRIPT_KIT_CARGO_MESSAGE_FORMAT:-short}")

build_start=$SECONDS
if [[ " ${SCRIPT_KIT_CARGO_FEATURES:-} " == *"local-llm"* ]]; then
    run_with_heartbeat helper-build \
        cargo build --locked -p script-kit-ghost-llm-helper --bin script-kit-ghost-llm-helper --message-format="${SCRIPT_KIT_CARGO_MESSAGE_FORMAT:-short}"
    # Install the helper to a stable, non-purged location so the running app can
    # always find it. target/ and target-agent/pools/* are reclaimed under disk
    # pressure; ~/.scriptkit/bin survives, and resolve_helper_path() checks it.
    helper_src="target/debug/script-kit-ghost-llm-helper"
    helper_dst="$HOME/.scriptkit/bin/script-kit-ghost-llm-helper"
    if [ -f "$helper_src" ]; then
        mkdir -p "$HOME/.scriptkit/bin"
        if [ "$helper_src" -nt "$helper_dst" ] || [ ! -f "$helper_dst" ]; then
            cp -f "$helper_src" "$helper_dst" && echo "[dev.sh] installed ghost-llm helper → $helper_dst"
        fi
    fi
fi
run_with_heartbeat build \
    cargo "${build_args[@]}"
build_elapsed=$((SECONDS - build_start))

after_mtime="$(mtime "$BIN_PATH")"

# --- Suggestion 2: slow build → LLD / sccache / prune ----------------------
# Print one-off, actionable hints when the iteration took longer than the
# thresholds. Suppress duplicates within one cargo-watch lifetime via flag
# files in the stamp dir, so the user / AI agent only sees each suggestion
# once per dev.sh session.
SUGGEST_DIR="${SCRIPT_KIT_DEV_STAMP_DIR:-${TMPDIR:-/tmp}/sk-dev-launcher-stamps}"
mkdir -p "$SUGGEST_DIR" 2>/dev/null || true
SUGGEST_KEY="${SCRIPT_KIT_DEV_STAMP_FILE##*/}"
SUGGEST_KEY="${SUGGEST_KEY%.stamp}"
suggest_once() {
    local tag="$1"
    local msg="$2"
    local flag="${SUGGEST_DIR}/${SUGGEST_KEY}.${tag}.suggested"
    if [ ! -f "$flag" ]; then
        echo "[dev.sh] SUGGEST ${msg}" >&2
        : > "$flag" 2>/dev/null || true
    fi
}
SLOW_BUILD_LLD_THRESHOLD_S="${SCRIPT_KIT_SLOW_BUILD_LLD_S:-30}"
SLOW_BUILD_SCCACHE_THRESHOLD_S="${SCRIPT_KIT_SLOW_BUILD_SCCACHE_S:-90}"
if [ "$build_elapsed" -ge "$SLOW_BUILD_LLD_THRESHOLD_S" ] && [ "${SCRIPT_KIT_USE_LLD:-0}" != "1" ]; then
    suggest_once lld "build took ${build_elapsed}s (>=${SLOW_BUILD_LLD_THRESHOLD_S}s) — try: SCRIPT_KIT_USE_LLD=1 ./dev.sh  (requires: brew install llvm)"
fi
if [ "$build_elapsed" -ge "$SLOW_BUILD_SCCACHE_THRESHOLD_S" ] && [ "${SCRIPT_KIT_USE_SCCACHE:-0}" != "1" ]; then
    suggest_once sccache "build took ${build_elapsed}s (>=${SLOW_BUILD_SCCACHE_THRESHOLD_S}s) — try: SCRIPT_KIT_USE_SCCACHE=1 ./dev.sh  (requires: brew install sccache)"
fi

if [ "${SCRIPT_KIT_DEV_FORCE_RELAUNCH:-0}" != "1" ] \
    && [ "$before_mtime" = "$after_mtime" ] \
    && session_healthy "$SESSION_NAME"; then
    echo "[dev.sh] relaunch skipped: binary unchanged and session healthy" >&2
    exit 0
fi

run_relaunch
