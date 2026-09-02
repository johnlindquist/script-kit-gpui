#!/usr/bin/env bash
# Preflight before isolated DevTools session (fail fast on common hangs).
#
# Usage:
#   preflight-isolated.sh [--mode isolated|reuse-dev-watch|script-only] [--allow-dev-sh] [--skip-binary]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=devtools-session-lib.sh
source "${SCRIPT_DIR}/devtools-session-lib.sh"

MODE="${PREFLIGHT_MODE:-isolated}"
ALLOW_DEV_SH=0
SKIP_BINARY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode) MODE="${2:-isolated}"; shift 2 ;;
    --allow-dev-sh) ALLOW_DEV_SH=1; shift ;;
    --skip-binary) SKIP_BINARY=1; shift ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done
if [[ "$SKIP_BINARY" == "0" ]]; then resolve_session_artifact; fi

if [[ "$SKIP_BINARY" -eq 0 && ! -x "$DEVTOOLS_SESSION_BINARY" ]]; then
  echo "[preflight-isolated] fail: missing ${DEVTOOLS_SESSION_BINARY} (stage from target-agent or build first)" >&2
  exit 13
fi

if [[ "$MODE" == "script-only" ]]; then
  echo "[preflight-isolated] ok mode=script-only (no GPUI checks)" >&2
  exit 0
fi

count="$(gpui_instance_count)"
if [[ "$MODE" == "isolated" && "$count" -gt 1 ]]; then
  echo "[preflight-isolated] fail: ${count} script-kit-gpui instances (macOS single-instance). Stop orphans:" >&2
  pgrep -x -fl 'script-kit-gpui' 2>/dev/null | sed 's/^/  /' >&2 || true
  echo "  Inspect the exact registered session owners; do not kill by executable name." >&2
  exit 12
fi

if [[ "$MODE" == "isolated" && "$count" -eq 1 ]]; then
  echo "[preflight-isolated] warn: one script-kit-gpui already running — reuse its session or stop it first" >&2
fi

if detect_dev_sh; then
  if [[ "$MODE" == "isolated" ]]; then
    echo "[preflight-isolated] fail: ./dev.sh cargo-watch is running (contends with isolated session + agent-cargo)." >&2
    echo "  Stop dev.sh, or use --mode reuse-dev-watch." >&2
    exit 11
  fi
  if [[ "$ALLOW_DEV_SH" -eq 0 && "$MODE" == "reuse-dev-watch" ]]; then
    : # allowed
  fi
fi

if [[ "$MODE" == "reuse-dev-watch" ]]; then
  if ! detect_dev_sh; then
    echo "[preflight-isolated] warn: reuse-dev-watch but ./dev.sh cargo-watch not detected" >&2
  fi
  if ! detect_dev_watch_healthy; then
    echo "[preflight-isolated] fail: dev-watch session unhealthy or missing" >&2
    exit 10
  fi
fi

echo "[preflight-isolated] ok mode=${MODE} binary=$(basename "$DEVTOOLS_SESSION_BINARY")" >&2
