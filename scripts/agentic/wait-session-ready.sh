#!/usr/bin/env bash
# Poll the owned session app.log until ready, dead, or timed out.
#
# Usage:
#   wait-session-ready.sh <SESSION_NAME> [TIMEOUT_SEC]
#
# Exit 0: STARTUP_READY or APP_READY seen in a live session.
# Exit 1: session missing or not alive. Exit 41: readiness timeout.
# Exit 42: app.log stayed empty while process alive (stuck startup).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=devtools-session-lib.sh
source "${SCRIPT_DIR}/devtools-session-lib.sh"

SESSION="${1:-}"
TIMEOUT_SEC="${2:-60}"

if [[ -z "$SESSION" ]]; then
  echo "usage: wait-session-ready.sh <SESSION_NAME> [TIMEOUT_SEC]" >&2
  exit 2
fi

if ! session_name_valid "$SESSION"; then
  echo "[wait-session-ready] unsafe session identity: ${SESSION}" >&2
  exit 64
fi
if ! session_positive_integer "$TIMEOUT_SEC"; then
  echo "[wait-session-ready] timeout must be a positive whole number: ${TIMEOUT_SEC}" >&2
  exit 64
fi

SDIR="$(session_sdir "$SESSION")"
LOG="${SDIR}/app.log"

if [[ ! -d "$SDIR" ]]; then
  echo "[wait-session-ready] no session dir: ${SDIR}" >&2
  exit 1
fi
EXPECTED_GENERATION="$(cat "${SDIR}/generation" 2>/dev/null || true)"
[[ -n "$EXPECTED_GENERATION" ]] || { echo '[wait-session-ready] missing session generation' >&2; exit 64; }

deadline=$(( SECONDS + TIMEOUT_SEC ))
last_size=0
stall_loops=0
ready_marker=""

echo "[wait-session-ready] session=${SESSION} timeout=${TIMEOUT_SEC}s log=${LOG}" >&2

# Always inspect liveness once, even when the timeout expires during setup.
while :; do
  if [[ "$(cat "${SDIR}/generation" 2>/dev/null || true)" != "$EXPECTED_GENERATION" ]]; then
    echo '[wait-session-ready] session generation changed' >&2; exit 64
  fi
  if ! bash "${DEVTOOLS_SESSION_REPO_ROOT}/scripts/agentic/session.sh" status "$SESSION" 2>/dev/null | grep -q '"alive":true'; then
    echo "[wait-session-ready] app process not alive" >&2
    bash "${DEVTOOLS_SESSION_REPO_ROOT}/scripts/agentic/session.sh" status "$SESSION" >&2 || true
    exit 1
  fi

  if grep -Fq "STARTUP_READY " "$LOG" 2>/dev/null; then
    ready_marker="startup_ready"
    echo "[wait-session-ready] ok: STARTUP_READY in app.log" >&2
    exit 0
  fi
  if grep -Fq "APP_READY|" "$LOG" 2>/dev/null; then
    ready_marker="app_ready"
    echo "[wait-session-ready] ok: APP_READY in app.log" >&2
    exit 0
  fi


  size=0
  if [[ -f "$LOG" ]]; then
    size="$(wc -c < "$LOG" | tr -d '[:space:]')"
  fi

  if [[ "$size" -gt "$last_size" ]]; then
    stall_loops=0
    last_size="$size"
    tail -n 3 "$LOG" 2>/dev/null | sed 's/^/[wait-session-ready] log> /' >&2 || true
  else
    stall_loops=$((stall_loops + 1))
  fi

  if [[ "$stall_loops" -ge 15 && "$size" -eq 0 ]]; then
    echo "[wait-session-ready] fail: app alive but app.log still empty (stuck startup? multiple instances?)" >&2
    pgrep -fl 'target/debug/script-kit-gpui' 2>/dev/null | sed 's/^/[wait-session-ready] proc> /' >&2 || true
    exit 42
  fi
  [[ "$SECONDS" -lt "$deadline" ]] || break


  remaining=$(( deadline - SECONDS ))
  if [[ "$remaining" -gt 2 ]]; then remaining=2; fi
  [[ "$remaining" -gt 0 ]] || break
  sleep "$remaining"
done

echo "[wait-session-ready] timeout after ${TIMEOUT_SEC}s (log_bytes=${last_size})" >&2
bash "${DEVTOOLS_SESSION_REPO_ROOT}/scripts/agentic/session.sh" status "$SESSION" >&2 || true
if [[ -f "$LOG" ]]; then
  echo "[wait-session-ready] --- app.log tail ---" >&2
  tail -n 40 "$LOG" >&2 || true
fi
exit 41
