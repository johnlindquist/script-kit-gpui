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
# SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT=1..100 optionally refuses compiler
# work before pool creation when current one-minute load plus its workers
# would exceed the selected fraction of available logical CPUs. Metadata,
# formatting, and dependency-tree inspection remain available under pressure.
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

use_sccache="${SCRIPT_KIT_AGENT_USE_SCCACHE:-auto}"
case "$use_sccache" in
  0|1|auto) ;;
  *) worker_failure "SCRIPT_KIT_AGENT_USE_SCCACHE must be 0, 1, or auto; got ${use_sccache}" ;;
esac

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

if [[ "$noninteractive_mode" == "1" ]]; then
  cargo_subcommand="${requested_args[0]:-}"
  for (( owner_index=1; owner_index<${#requested_args[@]}; owner_index++ )); do
    owner_argument="${requested_args[$owner_index]}"
    selected_package=""
    case "$owner_argument" in
      --manifest-path|--manifest-path=*|-C|-C=*)
        worker_failure "Cargo workspace ownership cannot be overridden by ${owner_argument}"
        ;;
      --workspace|--all)
        case "$cargo_subcommand" in
          test|nextest|run)
            worker_failure "noninteractive agent Cargo refuses unreviewed workspace or package: ${owner_argument}"
            ;;
        esac
        ;;
      -p|--package)
        (( owner_index + 1 < ${#requested_args[@]} )) \
          || worker_failure "noninteractive agent Cargo refuses unreviewed workspace or package: ${owner_argument}"
        owner_index=$((owner_index + 1))
        selected_package="${requested_args[$owner_index]}"
        ;;
      --package=*)
        selected_package="${owner_argument#--package=}"
        ;;
      -p*)
        selected_package="${owner_argument#-p}"
        selected_package="${selected_package#=}"
        ;;
    esac
    if [[ -n "$selected_package" ]]; then
      case "$cargo_subcommand" in
        test|nextest|run)
          case "$selected_package" in
            script-kit-gpui|sk-clipboard|sk-protocol|sk-storage) ;;
            *) worker_failure "noninteractive agent Cargo refuses unreviewed workspace or package: ${selected_package}" ;;
          esac
          ;;
      esac
    fi
  done
  case "$cargo_subcommand" in
    build|check|clippy|fmt|metadata|tree|bundle|rustc|rustdoc)
      ;;
    doc)
      for argument in "${requested_args[@]}"; do
        if [[ "$argument" == "--open" || "$argument" == --open=* ]]; then
          worker_failure "noninteractive agent Cargo refuses application launch or live benchmarks"
        fi
      done
      ;;
    run)
      reviewed_exporter=0
      for (( command_index=1; command_index<${#requested_args[@]}; command_index++ )); do
        case "${requested_args[$command_index]}" in
          --bin)
            (( command_index + 1 < ${#requested_args[@]} )) \
              || worker_failure "noninteractive agent Cargo refuses application launch or live benchmarks"
            command_index=$((command_index + 1))
            [[ "${requested_args[$command_index]}" == "export_design_tokens" ]] \
              || worker_failure "noninteractive agent Cargo refuses application launch or live benchmarks"
            reviewed_exporter=1
            ;;
          --bin=*)
            [[ "${requested_args[$command_index]#--bin=}" == "export_design_tokens" ]] \
              || worker_failure "noninteractive agent Cargo refuses application launch or live benchmarks"
            reviewed_exporter=1
            ;;
        esac
      done
      (( reviewed_exporter == 1 )) \
        || worker_failure "noninteractive agent Cargo refuses application launch or live benchmarks"
      ;;
    bench)
      worker_failure "noninteractive agent Cargo refuses application launch or live benchmarks"
      ;;
    test|nextest)
      reviewed_app_target=0
      reviewed_domain_target=0
      unreviewed_package=0
      for (( command_index=1; command_index<${#requested_args[@]}; command_index++ )); do
        argument="${requested_args[$command_index]}"
        package_name=""
        feature_selection=""
        case "$argument" in
          --ignored|--ignored=*|--include-ignored|--include-ignored=*|--run-ignored|--run-ignored=*|--all-features|--tests|--all-targets|--bins|--examples)
            worker_failure "noninteractive agent Cargo refuses unsafe test selection: ${argument}"
            ;;
          --lib|--test|--test=*)
            reviewed_app_target=1
            ;;
          -p|--package)
            (( command_index + 1 < ${#requested_args[@]} )) \
              || worker_failure "noninteractive agent Cargo requires an explicit reviewed --lib, --test, or safe domain package"
            command_index=$((command_index + 1))
            package_name="${requested_args[$command_index]}"
            ;;
          --package=*)
            package_name="${argument#--package=}"
            ;;
          -p*)
            package_name="${argument#-p}"
            package_name="${package_name#=}"
            ;;
          --features|-F)
            (( command_index + 1 < ${#requested_args[@]} )) \
              || worker_failure "noninteractive agent Cargo refuses unsafe test selection: ${argument}"
            command_index=$((command_index + 1))
            feature_selection="${requested_args[$command_index]}"
            ;;
          --features=*)
            feature_selection="${argument#--features=}"
            ;;
          -F*)
            feature_selection="${argument#-F}"
            feature_selection="${feature_selection#=}"
            ;;
        esac

        if [[ -n "$package_name" ]]; then
          case "$package_name" in
            sk-clipboard|sk-protocol|sk-storage) reviewed_domain_target=1 ;;
            *) unreviewed_package=1 ;;
          esac
        fi

        if [[ -n "$feature_selection" ]]; then
          IFS=', ' read -r -a requested_features <<< "$feature_selection"
          for selected_feature in "${requested_features[@]}"; do
            if [[ "$selected_feature" == "system-tests" || "$selected_feature" == */system-tests ]]; then
              worker_failure "noninteractive agent Cargo refuses unsafe test selection: ${selected_feature}"
            fi
          done
        fi
      done

      if (( reviewed_app_target == 0 && (reviewed_domain_target == 0 || unreviewed_package == 1) )); then
        worker_failure "noninteractive agent Cargo requires an explicit reviewed --lib, --test, or safe domain package"
      fi
      ;;
    *)
      worker_failure "noninteractive agent Cargo refuses unreviewed subcommand or alias: ${cargo_subcommand:-<missing>}"
      ;;
  esac
fi

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

system_load_1m_json="null"
system_logical_cpus_json="null"
system_load_limit_percent_json="null"
system_load_reserved_workers_json="null"
max_system_load_percent="${SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT:-}"
if [[ -n "$max_system_load_percent" ]]; then
  if [[ ! "$max_system_load_percent" =~ ^[1-9][0-9]*$ ]] \
    || (( max_system_load_percent > 100 )); then
    worker_failure "SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT must be a whole percentage from 1 to 100; got ${max_system_load_percent}"
  fi
  system_load_limit_percent_json="$max_system_load_percent"

  case "${requested_args[0]:-}" in
    build|check|clippy|test|nextest|rustc|rustdoc|doc|bench)
      workload_workers="$compiler_workers"
      case "${requested_args[0]}" in
        test|nextest)
          if (( test_workers > workload_workers )); then
            workload_workers="$test_workers"
          fi
          ;;
      esac

      uptime_observation="$(LC_ALL=C uptime 2>/dev/null)" \
        || worker_failure "could not observe one-minute system load before starting Cargo"
      system_load_1m="$(printf '%s\n' "$uptime_observation" | awk -F'load averages?:[[:space:]]*' '
        NF == 2 {
          split($2, values, /[[:space:],]+/)
          print values[1]
        }
      ')"
      [[ "$system_load_1m" =~ ^[0-9]+([.][0-9]+)?$ ]] \
        || worker_failure "could not observe a valid one-minute system load before starting Cargo"

      system_logical_cpus="$(getconf _NPROCESSORS_ONLN 2>/dev/null)" \
        || worker_failure "could not observe the logical CPU count before starting Cargo"
      [[ "$system_logical_cpus" =~ ^[1-9][0-9]*$ ]] \
        || worker_failure "could not observe a valid logical CPU count before starting Cargo"

      if ! awk \
        -v current_load="$system_load_1m" \
        -v workers="$workload_workers" \
        -v logical_cpus="$system_logical_cpus" \
        -v budget_percent="$max_system_load_percent" \
        'BEGIN { exit ((current_load + workers) * 100 <= logical_cpus * budget_percent ? 0 : 1) }'; then
        echo "AGENT_CARGO deferred: machine CPU pressure exceeds the explicit ${max_system_load_percent}% budget; load=${system_load_1m} logical_cpus=${system_logical_cpus} compiler_workers=${compiler_workers} test_workers=${test_workers} workload_workers=${workload_workers}; retry when other work subsides" >&2
        exit 75
      fi

      system_load_1m_json="$system_load_1m"
      system_logical_cpus_json="$system_logical_cpus"
      system_load_reserved_workers_json="$workload_workers"
      ;;
  esac
fi

sanitize_id() {
  printf '%s' "$1" | tr -c 'a-zA-Z0-9._-' '-'
}

owned_cache_id() {
  local raw="$1" label="$2" normalized
  normalized="$(sanitize_id "$raw")"
  if [[ -z "$normalized" || "$normalized" == "." || "$normalized" == ".." ]]; then
    worker_failure "${label}=${raw} must name one owned cache child"
  fi
  printf '%s' "$normalized"
}

agent_id="$(owned_cache_id "${SCRIPT_KIT_AGENT_ID:-${USER:-agent}-${PPID:-$$}}" SCRIPT_KIT_AGENT_ID)"
target_mode="${SCRIPT_KIT_AGENT_TARGET_MODE:-pool}"
pool="$(owned_cache_id "${SCRIPT_KIT_CARGO_TARGET_POOL:-agent-debug}" SCRIPT_KIT_CARGO_TARGET_POOL)"
default_pool="agent-debug"
validated_artifact_name=""
if [[ -n "${SCRIPT_KIT_AGENT_ARTIFACT_NAME:-}" ]]; then
  validated_artifact_name="$(owned_cache_id "$SCRIPT_KIT_AGENT_ARTIFACT_NAME" SCRIPT_KIT_AGENT_ARTIFACT_NAME)"
fi

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

for protected_cache_path in \
  "${REPO_ROOT}/target-agent" \
  "${REPO_ROOT}/target-agent/pools" \
  "${REPO_ROOT}/target-agent/agents" \
  "${REPO_ROOT}/target-agent/.locks" \
  "${REPO_ROOT}/target-agent/shared" \
  "${REPO_ROOT}/target-agent/artifacts" \
  "$target_dir" \
  "$metal_module_cache"; do
  if [[ -L "$protected_cache_path" ]]; then
    worker_failure "protected cache ownership cannot follow a symlink: ${protected_cache_path}"
  fi
done
if [[ -n "$validated_artifact_name" && -L "${REPO_ROOT}/target-agent/artifacts/${validated_artifact_name}" ]]; then
  worker_failure "protected cache ownership cannot follow a symlink: ${REPO_ROOT}/target-agent/artifacts/${validated_artifact_name}"
fi
if [[ -n "$validated_artifact_name" && "${requested_args[0]:-}" == "build" ]]; then
  for (( export_arg_index = 0; export_arg_index < ${#requested_args[@]}; export_arg_index++ )); do
    if [[ "${requested_args[$export_arg_index]}" != "--bin" ]]; then
      continue
    fi
    if (( export_arg_index + 1 >= ${#requested_args[@]} )); then
      continue
    fi
    export_binary_name="${requested_args[$((export_arg_index + 1))]}"
    [[ "$export_binary_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] \
      || worker_failure "exported binary must name one owned artifact child; got ${export_binary_name}"
    export_binary_path="${REPO_ROOT}/target-agent/artifacts/${validated_artifact_name}/${export_binary_name}"
    if [[ -L "$export_binary_path" || -L "${export_binary_path}.provenance.json" ]]; then
      worker_failure "protected artifact provenance cannot follow a symlink: ${export_binary_path}"
    fi
  done
fi

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
      echo "AGENT_CARGO warning: sccache cannot execute rustc in this sandbox; use approved sandbox permissions or set SCRIPT_KIT_AGENT_USE_SCCACHE=1 to refuse uncached builds; continuing without compiler caching" >&2
    fi
  elif [[ "$use_sccache" == "1" ]]; then
    echo "AGENT_CARGO error: SCRIPT_KIT_AGENT_USE_SCCACHE=1 but sccache is unavailable; install the official prebuilt package or use auto" >&2
    exit 69
  fi
fi

case "$rustc_wrapper_state" in
  sccache) compiler_cache_backend="sccache" ;;
  existing:*) compiler_cache_backend="external" ;;
  unavailable) compiler_cache_backend="unavailable" ;;
  none) compiler_cache_backend="disabled" ;;
  *) worker_failure "unknown compiler cache backend: ${rustc_wrapper_state}" ;;
esac
compiler_cache_required="false"
if [[ "$use_sccache" == "1" ]]; then
  compiler_cache_required="true"
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

artifact_source_commit=""
artifact_source_dirty=false
artifact_source_compiler_sha=""
artifact_observed_commit=""
artifact_observed_dirty=false
artifact_observed_compiler_sha=""
compiler_input_paths=()

load_compiler_input_paths() {
  local owner="${SCRIPT_ROOT}/compiler-input-paths.txt" path
  [[ -f "$owner" && ! -L "$owner" ]] \
    || worker_failure "exported artifact requires the canonical reviewed compiler-input owner"
  while IFS= read -r path || [[ -n "$path" ]]; do
    [[ -n "$path" && "$path" != /* && "$path" != *".."* ]] \
      || worker_failure "compiler-input owner contains an invalid repository-relative path"
    compiler_input_paths+=("$path")
  done < "$owner"
  (( ${#compiler_input_paths[@]} > 0 )) \
    || worker_failure "exported artifact requires a nonempty reviewed compiler-input owner"
}

observe_artifact_source() {
  local changed
  if (( ${#compiler_input_paths[@]} == 0 )); then
    load_compiler_input_paths
  fi
  artifact_observed_commit="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null)" \
    || worker_failure "exported artifact requires an independently observed Git source commit"
  [[ "$artifact_observed_commit" =~ ^[a-f0-9]{40}$ ]] \
    || worker_failure "exported artifact requires a full 40-character Git source commit"
  changed="$(git -C "$REPO_ROOT" status --porcelain --untracked-files=all -- \
    "${compiler_input_paths[@]}" 2>/dev/null)" \
    || worker_failure "exported artifact requires independently observed compiler-input cleanliness"
  artifact_observed_compiler_sha="$(
    git -C "$REPO_ROOT" ls-tree -r "$artifact_observed_commit" -- "${compiler_input_paths[@]}" \
      | shasum -a 256 | awk '{print $1}'
  )" || worker_failure "exported artifact requires an independently observed compiler-input tree"
  [[ "$artifact_observed_compiler_sha" =~ ^[a-f0-9]{64}$ ]] \
    || worker_failure "exported artifact requires a complete SHA-256 compiler-input fingerprint"
  artifact_observed_dirty=false
  [[ -z "$changed" ]] || artifact_observed_dirty=true
}

# After a successful `build --bin X`, clone the binary to a stable per-task
# path so parallel drivers never need a 26 GB pool of their own. APFS clones
# (cp -c) are instant and copy-on-write. Every clone carries an independently
# observed source/byte manifest; current HEAD alone is never build provenance.
export_artifacts() {
  local artifact_name profile_dir="debug" bins=() i=0 argc=$#
  local args=("$@")
  artifact_name="$validated_artifact_name"
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

  observe_artifact_source
  [[ "$artifact_observed_commit" == "$artifact_source_commit" ]] \
    || worker_failure "Git source commit changed during the build; refusing misleading artifact provenance"
  [[ "$artifact_observed_compiler_sha" == "$artifact_source_compiler_sha" ]] \
    || worker_failure "reviewed compiler inputs changed during the build; refusing misleading artifact provenance"
  if [[ "$artifact_observed_dirty" == "true" ]]; then
    artifact_source_dirty=true
  fi

  local artifact_dir="${REPO_ROOT}/target-agent/artifacts/${artifact_name}"
  mkdir -p "$artifact_dir"
  local bin src dest tmp manifest manifest_tmp binary_sha binary_size requires_exact_git=false
  if [[ "$profile_dir" == "release" || -n "${GITHUB_SHA:-}" || "${SCRIPT_KIT_TRACK_GIT_HEAD:-0}" == "1" ]]; then
    requires_exact_git=true
  fi
  for bin in "${bins[@]}"; do
    [[ "$bin" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] \
      || worker_failure "exported binary must name one owned artifact child; got ${bin}"
    src="${target_dir}/${profile_dir}/${bin}"
    if [[ ! -x "$src" ]]; then
      echo "AGENT_CARGO warning: built binary not found at ${src}; skipped export" >&2
      continue
    fi
    dest="${artifact_dir}/${bin}"
    manifest="${dest}.provenance.json"
    if [[ -L "$src" || -L "$dest" || -L "$manifest" ]]; then
      worker_failure "protected artifact provenance cannot follow a symlink: ${dest}"
    fi
    tmp="${dest}.tmp.$$"
    manifest_tmp="${manifest}.tmp.$$"
    if ! cp -c "$src" "$tmp" 2>/dev/null; then
      cp -p "$src" "$tmp"
    fi
    binary_sha="$(shasum -a 256 "$tmp" | awk '{print $1}')"
    binary_size="$(wc -c < "$tmp" | tr -d '[:space:]')"
    printf '{"schemaVersion":2,"pool":"%s","source":"%s","binaryPath":"%s","binarySha256":"%s","sizeBytes":%s,"gitHead":"%s","compilerInputSha256":"%s","profile":"%s","requiresExactGitHead":%s,"rustDirty":%s,"builtAt":"%s"}\n' \
      "$pool" "${src#${REPO_ROOT}/}" "${dest#${REPO_ROOT}/}" \
      "$binary_sha" "$binary_size" "$artifact_source_commit" "$artifact_source_compiler_sha" \
      "$profile_dir" "$requires_exact_git" "$artifact_source_dirty" \
      "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$manifest_tmp"
    mv -f "$tmp" "$dest"
    mv -f "$manifest_tmp" "$manifest"
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
echo "AGENT_CARGO mode=${target_mode} pool=${pool} cache=${cache_state} jobs=${CARGO_BUILD_JOBS} test_threads=${RUST_TEST_THREADS} cpu_load=${system_load_1m_json} logical_cpus=${system_logical_cpus_json} cpu_budget_percent=${system_load_limit_percent_json} cpu_reserved_workers=${system_load_reserved_workers_json} target_dir=${CARGO_TARGET_DIR} metal_module_cache=${metal_module_cache} lock=${lock_name} rustc_wrapper=${rustc_wrapper_state} debug=${CARGO_PROFILE_DEV_DEBUG} incremental=${CARGO_INCREMENTAL:-default} cargo ${cargo_args[*]}" >&2

if [[ -n "$validated_artifact_name" && "${cargo_args[0]:-}" == "build" ]]; then
  observe_artifact_source
  artifact_source_commit="$artifact_observed_commit"
  artifact_source_dirty="$artifact_observed_dirty"
  artifact_source_compiler_sha="$artifact_observed_compiler_sha"
fi

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
printf '{"started_epoch":%s,"elapsed_seconds":%s,"status":%s,"pool":"%s","cache":"%s","jobs":%s,"test_threads":%s,"free_before_gb":%s,"free_after_gb":%s,"command":"%s","timings":%s,"compiler_cache_backend":"%s","compiler_cache_required":%s,"system_load_1m":%s,"system_logical_cpus":%s,"system_load_limit_percent":%s,"system_load_reserved_workers":%s}\n' \
  "$started_epoch" "$elapsed_seconds" "$status" "$pool" "$cache_state" "$CARGO_BUILD_JOBS" "$RUST_TEST_THREADS" \
  "$free_before_gb" "$free_after_gb" "${cargo_args[0]:-unknown}" "${SCRIPT_KIT_AGENT_TIMINGS:-0}" \
  "$compiler_cache_backend" "$compiler_cache_required" \
  "$system_load_1m_json" "$system_logical_cpus_json" "$system_load_limit_percent_json" \
  "$system_load_reserved_workers_json" \
  >> "$receipt_path" 2>/dev/null || true
echo "AGENT_CARGO result status=${status} elapsed=${elapsed_seconds}s cache=${cache_state} compiler_cache=${compiler_cache_backend} free=${free_before_gb}G->${free_after_gb}G receipt=${receipt_path}" >&2

if [[ "$status" -eq 0 ]]; then
  export_artifacts "$@"
fi

release_lock
exit "$status"
