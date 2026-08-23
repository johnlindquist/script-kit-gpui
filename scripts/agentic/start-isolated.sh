#!/usr/bin/env bash
# Start an isolated agentic session for DevTools (no ./dev.sh).
#
# Usage: start-isolated.sh <SESSION_NAME> [--notes-sandbox] [--wait-sec SEC]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=devtools-session-lib.sh
source "${SCRIPT_DIR}/devtools-session-lib.sh"

cd "$DEVTOOLS_SESSION_REPO_ROOT"

SESSION="${1:-}"
if [[ -z "$SESSION" ]]; then
  echo "usage: start-isolated.sh <SESSION_NAME> [--notes-sandbox] [--wait-sec SEC]" >&2
  exit 2
fi

NOTES_SANDBOX=0
WAIT_SEC=60
shift || true
while [[ $# -gt 0 ]]; do
  case "$1" in
    --notes-sandbox) NOTES_SANDBOX=1; shift ;;
    --wait-sec) WAIT_SEC="${2:-60}"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

if ! session_name_valid "$SESSION"; then
  echo "[start-isolated] unsafe session identity: ${SESSION}" >&2
  exit 64
fi
if session_is_borrowed "$SESSION"; then
  echo "[start-isolated] refusing to claim the borrowed operator session: ${SESSION}" >&2
  exit 64
fi
if ! session_positive_integer "$WAIT_SEC"; then
  echo "[start-isolated] readiness timeout must be a positive whole number: ${WAIT_SEC}" >&2
  exit 64
fi

bash "${SCRIPT_DIR}/preflight-isolated.sh" --mode isolated

export SCRIPT_KIT_SESSION_DIR="${SCRIPT_KIT_SESSION_DIR:-/tmp/sk-agentic-sessions}"
if [[ "$NOTES_SANDBOX" -eq 1 ]]; then
  export SCRIPT_KIT_TEST_NOTES_DB_PATH="${SCRIPT_KIT_TEST_NOTES_DB_PATH:-/tmp/sk-notes-${SESSION}.db}"
fi

export SCRIPT_KIT_AI_LOG="${SCRIPT_KIT_AI_LOG:-1}"
export SCRIPT_KIT_STARTUP_READY_LOG="${SCRIPT_KIT_STARTUP_READY_LOG:-1}"
export SCRIPT_KIT_STARTUP_PROFILE="${SCRIPT_KIT_STARTUP_PROFILE:-dev-fast}"
# Short internal wait; visible gate is wait-session-ready.sh (Oracle: no double 60s).
export SCRIPT_KIT_SESSION_READY_TIMEOUT_MS="${SCRIPT_KIT_SESSION_READY_TIMEOUT_MS:-5000}"

echo "[start-isolated] SESSION=${SESSION}" >&2
echo "[start-isolated] SCRIPT_KIT_SESSION_DIR=${SCRIPT_KIT_SESSION_DIR}" >&2
[[ -n "${SCRIPT_KIT_TEST_NOTES_DB_PATH:-}" ]] && echo "[start-isolated] SCRIPT_KIT_TEST_NOTES_DB_PATH=${SCRIPT_KIT_TEST_NOTES_DB_PATH}" >&2
echo "[start-isolated] binary=${DEVTOOLS_SESSION_BINARY}" >&2

bash scripts/agentic/session.sh start "$SESSION"
bash scripts/agentic/session.sh status "$SESSION"

if bash "${SCRIPT_DIR}/wait-session-ready.sh" "$SESSION" "$WAIT_SEC"; then
  :
else
  wait_status=$?
  echo "[start-isolated] fail: session not ready within ${WAIT_SEC}s (exit ${wait_status})" >&2
  exit "$wait_status"
fi

echo "[start-isolated] ready" >&2
