#!/usr/bin/env bash
# Shared helpers for devtools-session.sh and isolated session scripts.
set -euo pipefail

DEVTOOLS_SESSION_REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEVTOOLS_SESSION_BINARY="${SCRIPT_KIT_GPUI_BINARY:-${DEVTOOLS_SESSION_REPO_ROOT}/target/debug/script-kit-gpui}"
DEVTOOLS_SESSION_START_TS="${SECONDS:-0}"

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\t/\\t/g' | tr '\n' ' '
}

progress() {
  local phase="$1"
  local message="$2"
  local elapsed="${SECONDS:-0}"
  printf '{"schemaVersion":1,"tool":"devtools-session","event":"progress","phase":"%s","elapsedSec":%s,"message":"%s"}\n' \
    "$phase" "$elapsed" "$(json_escape "$message")" >&2
}

detect_dev_sh() {
  pgrep -f 'cargo-watch watch.*dev-cycle' >/dev/null 2>&1
}

gpui_instance_count() {
  pgrep -x -c 'script-kit-gpui' 2>/dev/null || echo 0
}

detect_dev_watch_healthy() {
  local status
  status="$(bash "${DEVTOOLS_SESSION_REPO_ROOT}/scripts/agentic/session.sh" status dev-watch 2>/dev/null || true)"
  [[ "$status" == *'"healthy":true'* ]] || [[ "$status" == *'"alive":true'* && "$status" == *'"forwarderAlive":true'* ]]
}

session_name_valid() {
  local name="$1"
  [[ "$name" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]
}

session_positive_integer() {
  local value="$1"
  [[ "$value" =~ ^[1-9][0-9]*$ ]]
}

session_is_borrowed() {
  [[ "$1" == "dev-watch" ]]
}

session_sdir() {
  local name="$1"
  local base="${SCRIPT_KIT_SESSION_DIR:-/tmp/sk-agentic-sessions}"
  if ! session_name_valid "$name"; then
    printf '[devtools-session] unsafe session identity: %s\n' "$name" >&2
    return 64
  fi
  printf '%s/%s' "$base" "$name"
}

rust_changed_since_head() {
  if ! git -C "${DEVTOOLS_SESSION_REPO_ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    return 1
  fi
  local changed
  changed="$(git -C "${DEVTOOLS_SESSION_REPO_ROOT}" diff --name-only HEAD 2>/dev/null || true)"
  changed+="$(git -C "${DEVTOOLS_SESSION_REPO_ROOT}" diff --name-only --cached HEAD 2>/dev/null || true)"
  if [[ -z "$changed" ]]; then
    return 1
  fi
  if printf '%s\n' "$changed" | grep -qE '^(src/|Cargo\.(toml|lock)|build\.rs)'; then
    return 0
  fi
  return 1
}

script_supports_sk_verify() {
  local script_path="$1"
  grep -q 'SK_VERIFY' "$script_path" 2>/dev/null
}
