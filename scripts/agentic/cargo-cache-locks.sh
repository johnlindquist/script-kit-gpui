#!/usr/bin/env bash
# Shared, fail-closed ownership rules for Script Kit Cargo target caches.
# Source this after defining REPO_ROOT. Cleanup must acquire the exact same
# directory lock as a build before removing one individual pool.

cargo_cache_lock_path() {
  local candidate="$1" parent name
  parent="$(dirname "$candidate")"
  name="$(basename "$candidate")"

  case "$parent" in
    "${REPO_ROOT}/target-agent/pools")
      printf '%s/target-agent/.locks/pool-%s.lock\n' "$REPO_ROOT" "$name"
      ;;
    "${REPO_ROOT}/target-agent/agents")
      printf '%s/target-agent/.locks/agent-%s.lock\n' "$REPO_ROOT" "$name"
      ;;
    *) return 1 ;;
  esac
}

cargo_cache_lock_is_active() {
  local lock="$1" pid
  [[ -d "$lock" ]] || return 1

  # mkdir(lock) and writing pid are separate operations. An incomplete or
  # malformed lease is protected, not an invitation to delete a live build.
  if [[ ! -f "${lock}/pid" ]] || ! IFS= read -r pid < "${lock}/pid"; then
    return 0
  fi
  [[ "$pid" =~ ^[0-9]+$ ]] || return 0
  kill -0 "$pid" 2>/dev/null
}

cargo_cache_candidate_is_locked() {
  local lock
  lock="$(cargo_cache_lock_path "$1")" || return 0
  cargo_cache_lock_is_active "$lock"
}

cargo_cache_any_live_lock() {
  local lock
  for lock in "${REPO_ROOT}"/target-agent/.locks/*.lock; do
    [[ -d "$lock" ]] || continue
    cargo_cache_lock_is_active "$lock" && return 0
  done
  return 1
}

cargo_cache_candidate_is_pinned() {
  [[ "$1" == "${REPO_ROOT}/target-agent/pools/${SCRIPT_KIT_AGENT_PINNED_POOL:-agent-debug}" ]]
}

cargo_cache_remove_candidate() {
  local candidate="$1" allow_pinned="${2:-0}" lock status=0

  [[ -d "$candidate" && ! -L "$candidate" ]] || return 1
  lock="$(cargo_cache_lock_path "$candidate")" || return 1

  if cargo_cache_candidate_is_pinned "$candidate" && [[ "$allow_pinned" != "1" ]]; then
    echo "CARGO_CACHE preserve pinned=${candidate}" >&2
    return 1
  fi

  mkdir -p "${REPO_ROOT}/target-agent/.locks"
  if ! mkdir "$lock" 2>/dev/null; then
    echo "CARGO_CACHE preserve locked=${candidate}" >&2
    return 1
  fi

  if ! printf '%s\n' "$$" > "${lock}/pid"; then
    rm -rf "$lock" 2>/dev/null || true
    return 1
  fi

  # The candidate is validated as one immediate child of pools/ or agents/;
  # parent directories, shared caches, artifacts, and .locks are never targets.
  if ! rm -rf -- "$candidate"; then
    status=1
  fi
  rm -rf "$lock" 2>/dev/null || true
  return "$status"
}
