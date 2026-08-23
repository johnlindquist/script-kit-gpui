#!/usr/bin/env bash
# Timeboxed agent-cargo build for isolated DevTools (never block silently).
#
# Usage:
#   build-isolated-binary.sh [TIMEOUT_SEC]
#   build-isolated-binary.sh --json [TIMEOUT_SEC]
#
# stdout: final JSON envelope (when DEVTOOLS_SESSION_JSON=1 or --json)
# stderr: progress heartbeats
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=devtools-session-lib.sh
source "${SCRIPT_DIR}/devtools-session-lib.sh"

cd "$DEVTOOLS_SESSION_REPO_ROOT"

JSON_OUT="${DEVTOOLS_SESSION_JSON:-0}"
TIMEOUT_SEC="${1:-120}"
if [[ "${1:-}" == "--json" ]]; then
  JSON_OUT=1
  TIMEOUT_SEC="${2:-120}"
fi

if [[ ! "$TIMEOUT_SEC" =~ ^[1-9][0-9]*$ ]]; then
  echo "[build-isolated] fail: timeout must be a positive whole number; got ${TIMEOUT_SEC}" >&2
  exit 64
fi

export SCRIPT_KIT_AGENT_ID="${SCRIPT_KIT_AGENT_ID:-dt-agent-build}"
sanitize_id() {
  printf '%s' "$1" | tr -c 'a-zA-Z0-9._-' '-'
}

owned_build_id() {
  local raw="$1" name="$2" normalized
  normalized="$(sanitize_id "$raw")"
  if [[ -z "$normalized" || "$normalized" == "." || "$normalized" == ".." ]]; then
    echo "[build-isolated] fail: isolated build identity must name one owned child; ${name}=${raw}" >&2
    exit 64
  fi
  printf '%s' "$normalized"
}

TARGET_MODE="${SCRIPT_KIT_AGENT_TARGET_MODE:-pool}"
POOL="$(owned_build_id "${SCRIPT_KIT_CARGO_TARGET_POOL:-agent-debug}" SCRIPT_KIT_CARGO_TARGET_POOL)"
AGENT_ID="$(owned_build_id "$SCRIPT_KIT_AGENT_ID" SCRIPT_KIT_AGENT_ID)"
SESSION_NAME="$(owned_build_id "${SCRIPT_KIT_DEVTOOLS_SESSION:-$SCRIPT_KIT_AGENT_ID}" SCRIPT_KIT_DEVTOOLS_SESSION)"
export SCRIPT_KIT_AGENT_ID="$AGENT_ID"
export SCRIPT_KIT_CARGO_TARGET_POOL="$POOL"

case "$TARGET_MODE" in
  pool) TARGET_DIR="${DEVTOOLS_SESSION_REPO_ROOT}/target-agent/pools/${POOL}" ;;
  exclusive) TARGET_DIR="${DEVTOOLS_SESSION_REPO_ROOT}/target-agent/agents/${AGENT_ID}" ;;
  *)
    echo "[build-isolated] fail: SCRIPT_KIT_AGENT_TARGET_MODE must be pool or exclusive; got ${TARGET_MODE}" >&2
    exit 2
    ;;
esac

SRC="${TARGET_DIR}/debug/script-kit-gpui"
RUNTIME_DIR="${DEVTOOLS_SESSION_REPO_ROOT}/target-agent/runtime/${SESSION_NAME}"
DST="${RUNTIME_DIR}/script-kit-gpui"
MANIFEST="${RUNTIME_DIR}/manifest.json"
LOG="/tmp/sk-isolated-build-${SCRIPT_KIT_AGENT_ID}.log"

for protected_build_path in \
  "${DEVTOOLS_SESSION_REPO_ROOT}/target-agent" \
  "${DEVTOOLS_SESSION_REPO_ROOT}/target-agent/pools" \
  "${DEVTOOLS_SESSION_REPO_ROOT}/target-agent/agents" \
  "${DEVTOOLS_SESSION_REPO_ROOT}/target-agent/runtime" \
  "$TARGET_DIR" \
  "$SRC" \
  "$RUNTIME_DIR" \
  "$DST" \
  "$MANIFEST" \
  "$LOG"; do
  if [[ -L "$protected_build_path" ]]; then
    echo "[build-isolated] fail: isolated build ownership cannot follow a symlink: ${protected_build_path}" >&2
    exit 64
  fi
done

: > "$LOG"

emit_build_json() {
  local status="$1"
  local phase="$2"
  local code="${3:-}"
  local message="${4:-}"
  local elapsed="${5:-0}"
  if [[ "$JSON_OUT" -eq 1 ]]; then
    if [[ "$status" == "ok" ]]; then
      printf '{"schemaVersion":1,"tool":"build-isolated-binary","status":"ok","phase":"%s","agentId":"%s","pool":"%s","targetDir":"%s","binaryPath":"%s","manifest":"%s","elapsedSec":%s,"log":"%s"}\n' \
        "$phase" "$SCRIPT_KIT_AGENT_ID" "$POOL" \
        "$(json_escape "${TARGET_DIR#${DEVTOOLS_SESSION_REPO_ROOT}/}")" \
        "$(json_escape "${DST#${DEVTOOLS_SESSION_REPO_ROOT}/}")" \
        "$(json_escape "${MANIFEST#${DEVTOOLS_SESSION_REPO_ROOT}/}")" \
        "$elapsed" "$LOG"
    else
      printf '{"schemaVersion":1,"tool":"build-isolated-binary","status":"error","phase":"%s","agentId":"%s","pool":"%s","targetDir":"%s","elapsedSec":%s,"log":"%s","error":{"code":"%s","message":"%s"}}\n' \
        "$phase" "$SCRIPT_KIT_AGENT_ID" "$POOL" \
        "$(json_escape "${TARGET_DIR#${DEVTOOLS_SESSION_REPO_ROOT}/}")" \
        "$elapsed" "$LOG" "$code" "$(json_escape "$message")"
    fi
  fi
}

echo "[build-isolated] agent_id=${SCRIPT_KIT_AGENT_ID} pool=${POOL} mode=${TARGET_MODE} timeout=${TIMEOUT_SEC}s log=${LOG}" >&2

# A background pipeline normally shares the parent's process group, so killing
# only its shell leaves Cargo/rustc running. Give this exact owned pipeline its
# own group; timeout cleanup can then terminate every descendant together.
set -m
(
  ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui --message-format=short 2>&1 | tee -a "$LOG" >&2
) &
build_pid=$!
set +m

start_epoch=$(date +%s)
while kill -0 "$build_pid" 2>/dev/null; do
  elapsed=$(( $(date +%s) - start_epoch ))
  if [[ "$elapsed" -ge "$TIMEOUT_SEC" ]]; then
    echo "[build-isolated] timeout after ${TIMEOUT_SEC}s — stopping owned cargo process group ${build_pid}" >&2
    kill -TERM -- "-${build_pid}" 2>/dev/null || kill -TERM "$build_pid" 2>/dev/null || true
    sleep 0.05
    kill -KILL -- "-${build_pid}" 2>/dev/null || true
    wait "$build_pid" 2>/dev/null || true
    tail -n 30 "$LOG" >&2 || true
    emit_build_json error build build_timeout "cargo build exceeded ${TIMEOUT_SEC}s" "$elapsed"
    exit 30
  fi
  if [[ -f "$LOG" ]]; then
    lines="$(wc -l < "$LOG" | tr -d '[:space:]')"
  else
    lines=0
  fi
  echo "[build-isolated] ${elapsed}s elapsed, log_lines=${lines}" >&2
  sleep 5
done

set +e
wait "$build_pid"
status=$?
set -e
elapsed=$(( $(date +%s) - start_epoch ))

if [[ "$status" -ne 0 ]]; then
  echo "[build-isolated] cargo failed (exit ${status})" >&2
  tail -n 40 "$LOG" >&2 || true
  emit_build_json error build build_failed "cargo build failed with exit ${status}" "$elapsed"
  exit 31
fi

if [[ ! -f "$SRC" ]]; then
  echo "[build-isolated] fail: built binary missing at ${SRC}" >&2
  emit_build_json error stage stage_failed "built binary missing at ${SRC}" "$elapsed"
  exit 32
fi

mkdir -p "$RUNTIME_DIR"
tmp="${DST}.tmp.$$"
cp -f "$SRC" "$tmp"
chmod +x "$tmp"
mv -f "$tmp" "$DST"

git_head="$(git rev-parse HEAD 2>/dev/null || true)"
rust_dirty=false
if git diff --name-only HEAD -- src Cargo.toml Cargo.lock build.rs 2>/dev/null | grep -q .; then
  rust_dirty=true
fi

cat > "$MANIFEST" <<EOF
{"schemaVersion":1,"pool":"${POOL}","source":"${SRC#${DEVTOOLS_SESSION_REPO_ROOT}/}","binaryPath":"${DST#${DEVTOOLS_SESSION_REPO_ROOT}/}","gitHead":"${git_head}","rustDirty":${rust_dirty},"builtAt":"$(date -u +%Y-%m-%dT%H:%M:%SZ)"}
EOF

echo "[build-isolated] staged → ${DST#${DEVTOOLS_SESSION_REPO_ROOT}/}" >&2
emit_build_json ok stage "" "" "$elapsed"
exit 0
