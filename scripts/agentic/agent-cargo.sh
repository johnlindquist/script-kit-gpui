#!/usr/bin/env bash
# Run cargo from an AI agent (Claude Code, Codex, etc.) against a bounded
# agent-owned CARGO_TARGET_DIR so it does not contend on `target/.cargo-lock`
# with the always-on `./dev.sh` cargo-watch loop.
#
# Usage:
#   ./scripts/agentic/agent-cargo.sh test --lib notes_editor::spine
#   ./scripts/agentic/agent-cargo.sh check --lib
#   SCRIPT_KIT_CARGO_TARGET_POOL=agent-debug ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui
#   SCRIPT_KIT_AGENT_TARGET_MODE=exclusive SCRIPT_KIT_AGENT_ID=claude-a ./scripts/agentic/agent-cargo.sh check --lib
#
# Parallel tasks that need a stable binary should NOT mint a new pool. Build in
# the shared pool and export an APFS clone of the binary (~0 bytes, instant):
#   SCRIPT_KIT_AGENT_ARTIFACT_NAME=<task> ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui
#   # -> target-agent/artifacts/<task>/script-kit-gpui
#
# Disk policy (enforced synchronously at lock acquisition, before cargo runs):
#   SCRIPT_KIT_AGENT_TARGET_BUDGET_GB  total budget for target-agent pools+agents (default 40)
#   SCRIPT_KIT_AGENT_MIN_FREE_GB       free-disk floor that triggers LRU pool eviction (default 25)
#   SCRIPT_KIT_AGENT_CRITICAL_FREE_GB  harder floor; below it the requested pool's
#                                      own incremental/ dir is pruned too (default 10)
# Eviction only removes unlocked pools/agent dirs, LRU first, never the one
# being requested. Deterministic and synchronous: it never races a live build.
#
# Cache-size policy:
#   CARGO_PROFILE_DEV_DEBUG defaults to line-tables-only (usable backtraces, far
#   smaller deps/incremental). CARGO_INCREMENTAL stays on only for the default
#   shared pool; ephemeral pools and exclusive dirs get CARGO_INCREMENTAL=0.
#   Both respect pre-set env overrides.
#
# sccache: SCRIPT_KIT_AGENT_USE_SCCACHE=auto (default) uses sccache when on
# PATH, 1 requires it, 0 disables. Shared caches survive individual-pool eviction.
# Builds default to two workers and fail before starting under the disk floor.
# Compiler flags, inherited worker settings, and Rust test-harness threads cannot
# exceed the configured ceiling. Noninteractive runs never exceed two workers
# and cannot inherit intentionally heavyweight search/storage stress corpora.
# SCRIPT_KIT_AGENT_TIMINGS=1 emits Cargo's target/cargo-timings HTML report.

set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${SCRIPT_KIT_REPO_ROOT:-$(cd "${SCRIPT_ROOT}/../.." && pwd)}"
# shellcheck source=scripts/agentic/cargo-cache-locks.sh
source "${SCRIPT_ROOT}/cargo-cache-locks.sh"

worker_failure() {
  echo "AGENT_CARGO error: $1" >&2
  exit 64
}

validate_worker_count() {
  local value="$1" label="$2" ceiling="$3"
  [[ "$value" =~ ^[1-9][0-9]*$ ]] || worker_failure "${label} must be a positive whole worker count; got ${value}"
  (( value <= ceiling )) || worker_failure "${label}=${value} exceeds the ${ceiling}-worker safety ceiling"
}

noninteractive_mode="${SCRIPT_KIT_NONINTERACTIVE:-1}"
case "$noninteractive_mode" in
  0|1) ;;
  *) worker_failure "SCRIPT_KIT_NONINTERACTIVE must be 0 or 1; got ${noninteractive_mode}" ;;
esac
export SCRIPT_KIT_NONINTERACTIVE="$noninteractive_mode"

if [[ "$noninteractive_mode" == "1" ]]; then
  for unsafe_setting in \
    SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER \
    SCRIPT_KIT_ALLOW_NATIVE_INPUT \
    SCRIPT_KIT_ALLOW_SCREEN_CAPTURE \
    SCRIPT_KIT_ALLOW_VISIBLE_PROBES \
    SCRIPT_KIT_ALLOW_LIVE_AI \
    SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH; do
    if [[ "${!unsafe_setting:-0}" == "1" ]]; then
      worker_failure "noninteractive agent Cargo refuses ${unsafe_setting}=1"
    fi
    export "${unsafe_setting}=0"
  done
fi

max_workers="${SCRIPT_KIT_AGENT_MAX_JOBS:-2}"
[[ "$max_workers" =~ ^[1-9][0-9]*$ ]] \
  || worker_failure "SCRIPT_KIT_AGENT_MAX_JOBS must be a positive whole worker count; got ${max_workers}"
if [[ "$noninteractive_mode" == "1" && "$max_workers" -gt 2 ]]; then
  worker_failure "noninteractive builds cannot exceed two workers; got SCRIPT_KIT_AGENT_MAX_JOBS=${max_workers}"
fi

compiler_workers="${CARGO_BUILD_JOBS:-$max_workers}"
validate_worker_count "$compiler_workers" "CARGO_BUILD_JOBS" "$max_workers"
test_workers="${RUST_TEST_THREADS:-$compiler_workers}"
validate_worker_count "$test_workers" "RUST_TEST_THREADS" "$max_workers"

requested_args=("$@")
for (( worker_arg_index=0; worker_arg_index<${#requested_args[@]}; worker_arg_index++ )); do
  argument="${requested_args[$worker_arg_index]}"
  worker_value=""
  worker_label=""
  case "$argument" in
    --target-dir|--target-dir=*)
      worker_failure "target directory is owned by the protected Cargo pool; do not override --target-dir"
      ;;
    --config|--config=*)
      worker_failure "command-line Cargo config cannot override protected build policy"
      ;;
    --jobs|-j|--test-threads)
      (( worker_arg_index + 1 < ${#requested_args[@]} )) \
        || worker_failure "${argument} requires a positive worker count"
      worker_value="${requested_args[$((worker_arg_index + 1))]}"
      worker_label="$argument"
      worker_arg_index=$((worker_arg_index + 1))
      ;;
    --jobs=*|--test-threads=*)
      worker_value="${argument#*=}"
      worker_label="${argument%%=*}"
      ;;
    -j*)
      worker_value="${argument#-j}"
      worker_value="${worker_value#=}"
      worker_label="-j"
      ;;
    *)
      continue
      ;;
  esac
  validate_worker_count "$worker_value" "$worker_label" "$max_workers"
  if [[ "$worker_label" == "--test-threads" ]]; then
    test_workers="$worker_value"
  else
    compiler_workers="$worker_value"
    if [[ -z "${RUST_TEST_THREADS:-}" ]]; then
      test_workers="$compiler_workers"
    fi
  fi
done

export CARGO_BUILD_JOBS="$compiler_workers"
export RUST_TEST_THREADS="$test_workers"
if [[ "$noninteractive_mode" == "1" ]]; then
  export SCRIPT_KIT_SEARCH_FULL_STRESS=0
  export SCRIPT_KIT_STORAGE_FULL_STRESS=0
fi

sanitize_id() {
  printf '%s' "$1" | tr -c 'a-zA-Z0-9._-' '-'
}

agent_id="$(sanitize_id "${SCRIPT_KIT_AGENT_ID:-${USER:-agent}-${PPID:-$$}}")"
target_mode="${SCRIPT_KIT_AGENT_TARGET_MODE:-pool}"
pool="$(sanitize_id "${SCRIPT_KIT_CARGO_TARGET_POOL:-agent-debug}")"
default_pool="agent-debug"

case "$target_mode" in
  pool)
    target_dir="${REPO_ROOT}/target-agent/pools/${pool}"
    lock_name="pool-${pool}"
    ;;
  exclusive)
    target_dir="${REPO_ROOT}/target-agent/agents/${agent_id}"
    lock_name="agent-${agent_id}"
    ;;
  *)
    echo "AGENT_CARGO error: SCRIPT_KIT_AGENT_TARGET_MODE must be pool or exclusive; got ${target_mode}" >&2
    exit 2
    ;;
esac

lock_root="${REPO_ROOT}/target-agent/.locks"
lock_dir="${lock_root}/${lock_name}.lock"
shared_cache_dir="${REPO_ROOT}/target-agent/shared"
metal_module_cache="${SCRIPT_KIT_METAL_MODULE_CACHE_DIR:-${shared_cache_dir}/clang-modules}"
mkdir -p "$target_dir" "$lock_root" "$metal_module_cache"

export CARGO_TARGET_DIR="$target_dir"
export SCRIPT_KIT_METAL_MODULE_CACHE_DIR="$metal_module_cache"
export CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$metal_module_cache}"

# Slim debug info: agents read backtraces, they do not attach debuggers.
export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-line-tables-only}"

# Incremental compilation is worth its disk cost only in the long-lived shared
# pool; ephemeral pools/exclusive dirs rarely live long enough to amortize it.
if [[ -z "${CARGO_INCREMENTAL:-}" ]]; then
  if [[ "$target_mode" != "pool" || "$pool" != "$default_pool" ]]; then
    export CARGO_INCREMENTAL=0
  fi
fi

rustc_wrapper_state="none"
use_sccache="${SCRIPT_KIT_AGENT_USE_SCCACHE:-auto}"
if [[ -n "${RUSTC_WRAPPER:-}" ]]; then
  rustc_wrapper_state="existing:${RUSTC_WRAPPER}"
elif [[ "$use_sccache" == "1" || "$use_sccache" == "auto" ]]; then
  if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="sccache"
    export SCCACHE_DIR="${SCCACHE_DIR:-${shared_cache_dir}/sccache}"
    export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-10G}"
    export SCCACHE_BASEDIRS="${SCCACHE_BASEDIRS:-$REPO_ROOT}"
    export SCCACHE_SERVER_UDS="${SCCACHE_SERVER_UDS:-${shared_cache_dir}/sccache.sock}"
    if sccache --show-stats >/dev/null 2>&1 \
      && sccache "$(command -v rustc)" -vV >/dev/null 2>&1; then
      rustc_wrapper_state="sccache"
    elif [[ "$use_sccache" == "1" ]]; then
      echo "AGENT_CARGO error: required sccache cannot execute rustc through ${SCCACHE_SERVER_UDS}; start its server or run with the required sandbox permissions" >&2
      exit 69
    else
      unset RUSTC_WRAPPER
      rustc_wrapper_state="unavailable"
      echo "AGENT_CARGO warning: sccache cannot execute rustc in this sandbox; continuing without compiler caching" >&2
    fi
  elif [[ "$use_sccache" == "1" ]]; then
    echo "AGENT_CARGO error: SCRIPT_KIT_AGENT_USE_SCCACHE=1 but sccache is unavailable; install the official prebuilt package or use auto" >&2
    exit 69
  fi
fi

free_disk_kb() {
  df -k "$REPO_ROOT" | awk 'NR==2 {print $4}'
}

dir_kb() {
  du -sk "$1" 2>/dev/null | awk '{print $1}'
}

# A candidate dir is evictable if no live lock holds it.
candidate_locked() {
  cargo_cache_candidate_is_locked "$1"
}

# Print evictable candidate dirs (not ours, not locked), LRU first.
eviction_candidates() {
  local dir stamp
  for dir in "${REPO_ROOT}"/target-agent/pools/* "${REPO_ROOT}"/target-agent/agents/*; do
    [[ -d "$dir" ]] || continue
    [[ "$dir" == "$target_dir" ]] && continue
    cargo_cache_candidate_is_pinned "$dir" && continue
    candidate_locked "$dir" && continue
    if [[ -f "${dir}/.last_used" ]]; then
      stamp="$(stat -f '%m' "${dir}/.last_used" 2>/dev/null || echo 0)"
    else
      stamp="$(stat -f '%m' "$dir" 2>/dev/null || echo 0)"
    fi
    printf '%s\t%s\n' "$stamp" "$dir"
  done | sort -n | cut -f2
}

total_agent_target_kb() {
  local dir total=0 kb
  for dir in "${REPO_ROOT}"/target-agent/pools/* "${REPO_ROOT}"/target-agent/agents/*; do
    [[ -d "$dir" ]] || continue
    kb="$(dir_kb "$dir")"
    total=$(( total + ${kb:-0} ))
  done
  echo "$total"
}

enforce_disk_budget() {
  local budget_gb="${SCRIPT_KIT_AGENT_TARGET_BUDGET_GB:-40}"
  local min_free_gb="${SCRIPT_KIT_AGENT_MIN_FREE_GB:-25}"
  local critical_free_gb="${SCRIPT_KIT_AGENT_CRITICAL_FREE_GB:-10}"
  local budget_kb=$(( budget_gb * 1024 * 1024 ))
  local min_free_kb=$(( min_free_gb * 1024 * 1024 ))
  local critical_free_kb=$(( critical_free_gb * 1024 * 1024 ))
  local total_kb free_kb dir

  total_kb="$(total_agent_target_kb)"
  free_kb="$(free_disk_kb)"

  if (( total_kb <= budget_kb && free_kb >= min_free_kb )); then
    return 0
  fi

  echo "AGENT_CARGO disk_budget total=$((total_kb / 1024 / 1024))G/${budget_gb}G free=$((free_kb / 1024 / 1024))G/min${min_free_gb}G; evicting LRU pools" >&2

  while IFS= read -r dir; do
    (( total_kb <= budget_kb && free_kb >= min_free_kb )) && break
    echo "AGENT_CARGO evict dir=${dir} size=$(( $(dir_kb "$dir") / 1024 / 1024 ))G" >&2
    if ! cargo_cache_remove_candidate "$dir"; then
      continue
    fi
    total_kb="$(total_agent_target_kb)"
    free_kb="$(free_disk_kb)"
  done < <(eviction_candidates)

  # Last resort: prune our own incremental cache (safe; next build is just slower).
  if (( free_kb < critical_free_kb )) && [[ -d "${target_dir}/debug/incremental" ]]; then
    echo "AGENT_CARGO evict_incremental dir=${target_dir}/debug/incremental size=$(( $(dir_kb "${target_dir}/debug/incremental") / 1024 / 1024 ))G" >&2
    rm -rf "${target_dir}/debug/incremental"
    free_kb="$(free_disk_kb)"
  fi

  if (( free_kb < min_free_kb )); then
    if [[ "${SCRIPT_KIT_AGENT_ALLOW_LOW_DISK:-0}" != "1" ]]; then
      echo "AGENT_CARGO error: free disk $((free_kb / 1024 / 1024))G remains below ${min_free_gb}G; refusing an unpredictable build before cache cleanup intervenes" >&2
      return 75
    fi
    echo "AGENT_CARGO warning: low-disk build explicitly allowed free=$((free_kb / 1024 / 1024))G floor=${min_free_gb}G" >&2
  fi
}

acquire_lock() {
  local timeout="${SCRIPT_KIT_AGENT_LOCK_TIMEOUT_SEC:-600}"
  local start elapsed old_pid
  start="$(date +%s)"

  while ! mkdir "$lock_dir" 2>/dev/null; do
    if [[ -f "${lock_dir}/pid" ]]; then
      old_pid="$(cat "${lock_dir}/pid" 2>/dev/null || true)"
      if [[ -n "$old_pid" ]] && ! kill -0 "$old_pid" 2>/dev/null; then
        echo "AGENT_CARGO stale_lock pid=${old_pid} lock=${lock_dir}; removing" >&2
        rm -rf "$lock_dir"
        continue
      fi
    fi

    elapsed=$(( $(date +%s) - start ))
    if [[ "$elapsed" -ge "$timeout" ]]; then
      echo "AGENT_CARGO error: timed out waiting for ${lock_name} after ${timeout}s" >&2
      exit 70
    fi
    echo "AGENT_CARGO waiting mode=${target_mode} pool=${pool} elapsed=${elapsed}s lock=${lock_dir}" >&2
    sleep 5
  done

  {
    echo "$$" > "${lock_dir}/pid"
    printf '%s\n' "$agent_id" > "${lock_dir}/owner"
    printf '%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "${lock_dir}/started_at"
    printf '%q ' cargo "$@" > "${lock_dir}/command"
    printf '\n' >> "${lock_dir}/command"
  } 2>/dev/null || true
}

release_lock() {
  rm -rf "$lock_dir" 2>/dev/null || true
}

# After a successful `build --bin X`, clone the binary to a stable per-task
# path so parallel drivers never need a 26 GB pool of their own. APFS clones
# (cp -c) are instant and copy-on-write.
export_artifacts() {
  local artifact_name profile_dir="debug" bins=() i=0 argc=$#
  local args=("$@")
  artifact_name="$(sanitize_id "${SCRIPT_KIT_AGENT_ARTIFACT_NAME:-}")"
  [[ -n "$artifact_name" ]] || return 0
  [[ "${args[0]:-}" == "build" ]] || return 0

  while (( i < argc )); do
    case "${args[$i]}" in
      --bin)
        (( i + 1 < argc )) && bins+=("${args[$((i + 1))]}")
        ;;
      --release)
        profile_dir="release"
        ;;
      --profile)
        if (( i + 1 < argc )); then
          profile_dir="${args[$((i + 1))]}"
          [[ "$profile_dir" == "dev" || "$profile_dir" == "test" ]] && profile_dir="debug"
        fi
        ;;
    esac
    i=$(( i + 1 ))
  done

  if (( ${#bins[@]} == 0 )); then
    echo "AGENT_CARGO warning: SCRIPT_KIT_AGENT_ARTIFACT_NAME=${artifact_name} set but no --bin in build args; nothing exported" >&2
    return 0
  fi

  local artifact_dir="${REPO_ROOT}/target-agent/artifacts/${artifact_name}"
  mkdir -p "$artifact_dir"
  local bin src dest tmp
  for bin in "${bins[@]}"; do
    src="${target_dir}/${profile_dir}/${bin}"
    if [[ ! -x "$src" ]]; then
      echo "AGENT_CARGO warning: built binary not found at ${src}; skipped export" >&2
      continue
    fi
    dest="${artifact_dir}/${bin}"
    tmp="${dest}.tmp.$$"
    if ! cp -c "$src" "$tmp" 2>/dev/null; then
      cp -p "$src" "$tmp"
    fi
    mv -f "$tmp" "$dest"
    echo "AGENT_CARGO artifact bin=${bin} path=${dest}" >&2
  done
}

acquire_lock "$@"
trap release_lock EXIT INT TERM

touch "${target_dir}/.last_used" 2>/dev/null || true
enforce_disk_budget

cargo_args=("$@")
if [[ "${SCRIPT_KIT_AGENT_TIMINGS:-0}" == "1" ]]; then
  case "${cargo_args[0]:-}" in
    build|check|test|bench)
      timed_args=()
      inserted=0
      for argument in "${cargo_args[@]}"; do
        if [[ "$argument" == "--timings" || "$argument" == --timings=* ]]; then
          inserted=1
        fi
        if [[ "$argument" == "--" && "$inserted" == "0" ]]; then
          timed_args+=("--timings")
          inserted=1
        fi
        timed_args+=("$argument")
      done
      if [[ "$inserted" == "0" ]]; then
        timed_args+=("--timings")
      fi
      cargo_args=("${timed_args[@]}")
      ;;
  esac
fi

cache_state="cold"
if [[ -d "${target_dir}/debug/deps" ]] && [[ -n "$(find "${target_dir}/debug/deps" -maxdepth 1 -type f -print -quit 2>/dev/null)" ]]; then
  cache_state="warm"
fi
started_epoch="$(date +%s)"
free_before_gb="$(( $(free_disk_kb) / 1024 / 1024 ))"
echo "AGENT_CARGO mode=${target_mode} pool=${pool} cache=${cache_state} jobs=${CARGO_BUILD_JOBS} test_threads=${RUST_TEST_THREADS} target_dir=${CARGO_TARGET_DIR} metal_module_cache=${metal_module_cache} lock=${lock_name} rustc_wrapper=${rustc_wrapper_state} debug=${CARGO_PROFILE_DEV_DEBUG} incremental=${CARGO_INCREMENTAL:-default} cargo ${cargo_args[*]}" >&2

set +e
cargo "${cargo_args[@]}"
status=$?
set -e

if [[ "$status" -eq 0 && "${SCRIPT_KIT_AGENT_TIMINGS:-0}" == "1" ]]; then
  timing_report="${target_dir}/cargo-timings/cargo-timing.html"
  timing_summary="${target_dir}/cargo-timings/cargo-timing-summary.json"
  if [[ -f "$timing_report" && -f "${SCRIPT_ROOT}/cargo-timings-summary.ts" ]] \
    && command -v bun >/dev/null 2>&1; then
    if bun "${SCRIPT_ROOT}/cargo-timings-summary.ts" \
      --report "$timing_report" --out "$timing_summary" >/dev/null; then
      echo "AGENT_CARGO timing_summary=${timing_summary}" >&2
    else
      echo "AGENT_CARGO warning: Cargo timing report could not be summarized" >&2
    fi
  fi
fi

elapsed_seconds="$(( $(date +%s) - started_epoch ))"
free_after_gb="$(( $(free_disk_kb) / 1024 / 1024 ))"
receipt_path="${SCRIPT_KIT_AGENT_BUILD_RECEIPT_PATH:-${REPO_ROOT}/target-agent/build-receipts.jsonl}"
printf '{"started_epoch":%s,"elapsed_seconds":%s,"status":%s,"pool":"%s","cache":"%s","jobs":%s,"test_threads":%s,"free_before_gb":%s,"free_after_gb":%s,"command":"%s","timings":%s}\n' \
  "$started_epoch" "$elapsed_seconds" "$status" "$pool" "$cache_state" "$CARGO_BUILD_JOBS" "$RUST_TEST_THREADS" \
  "$free_before_gb" "$free_after_gb" "${cargo_args[0]:-unknown}" "${SCRIPT_KIT_AGENT_TIMINGS:-0}" \
  >> "$receipt_path" 2>/dev/null || true
echo "AGENT_CARGO result status=${status} elapsed=${elapsed_seconds}s cache=${cache_state} free=${free_before_gb}G->${free_after_gb}G receipt=${receipt_path}" >&2

if [[ "$status" -eq 0 ]]; then
  export_artifacts "$@"
fi

release_lock
exit "$status"
