#!/usr/bin/env bash
# Protected Cargo execution owner. All noninteractive tasks share agent-debug.
# Publication exports Cargo-emitted executables under this lease to unique,
# immutable manifests; no label path or physical APFS savings is asserted.
# Admission never evicts caches. Explicit human recovery stays separately gated.
# --version and SCRIPT_KIT_AGENT_POLICY_ONLY=1 are passive policy observations.

set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${SCRIPT_KIT_REPO_ROOT:-$(cd "${SCRIPT_ROOT}/../.." && pwd)}"
# Normalize repository aliases before constructing or acquiring any lease.
REPO_ROOT="$(cd "$REPO_ROOT" && pwd -P)"
export SCRIPT_KIT_REPO_ROOT="$REPO_ROOT"
# shellcheck source=scripts/agentic/cargo-cache-locks.sh
source "${SCRIPT_ROOT}/cargo-cache-locks.sh"

if [[ "${1:-}" == "--version" ]]; then
  printf '{"schemaVersion":3,"owner":"scripts/agentic/agent-cargo.sh","pool":"agent-debug","toolchainAuthority":"rust-toolchain.toml","publication":"immutable-v3","processOwner":"session-supervisor.py","passive":true}\n'
  exit 0
fi

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


# Sole reviewed external workspace. It is not an arbitrary manifest escape.
registered_sidecar=0
unset SCRIPT_KIT_AGENT_REGISTERED_TASK SCRIPT_KIT_AGENT_SIDECAR_SOURCE
if [[ "${1:-}" == "pi-sidecar-build" ]]; then
  [[ $# -eq 1 ]] || worker_failure "registered sidecar task accepts no Cargo arguments"
  [[ "${PI_AGENT_RUST_URL:-https://github.com/Dicklesworthstone/pi_agent_rust.git}" == "https://github.com/Dicklesworthstone/pi_agent_rust.git" ]] || worker_failure "sidecar URL override is not registered"
  registered_sidecar=1
  export SCRIPT_KIT_AGENT_REGISTERED_TASK=pi-sidecar
  set -- build --locked --release --bin pi
fi
requested_args=("$@")

# Cargo's stable intermediate-directory and lockfile relocation controls must
# not route an otherwise protected invocation outside this workspace/pool.
for ownership_setting in CARGO_BUILD_BUILD_DIR CARGO_RESOLVER_LOCKFILE_PATH; do
  if [[ -n "${!ownership_setting:-}" ]]; then
    worker_failure "Cargo storage ownership cannot be overridden by ${ownership_setting}"
  fi
done

if [[ "$noninteractive_mode" == "1" ]]; then
  cargo_subcommand="${requested_args[0]:-}"
  for (( owner_index=1; owner_index<${#requested_args[@]}; owner_index++ )); do
    owner_argument="${requested_args[$owner_index]}"
    selected_package=""
    case "$owner_argument" in
      --manifest-path|--manifest-path=*|-m|-m?*|-C|-C=*)
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
            gpui_macos|gpui-component)
              # These exact byte-only/TestAppContext contracts are reviewed, not
              # the vendor packages' other tests, binaries, or documentation.
              case "$selected_package" in
                gpui_macos) reviewed_vendor_filter="readback_alpha_tests" ;;
                gpui-component)
                  reviewed_vendor_filter="input::state::revision_tests"
                  if [[ "${requested_args[5]:-}" == "notification::tests::closing_window_releases_notification_before_autohide_tick" ]]; then
                    reviewed_vendor_filter="${requested_args[5]}"
                  fi
                  ;;
              esac
              if (( ${#requested_args[@]} != 6 )) \
                || [[ "${requested_args[0]}" != "test" \
                   || "${requested_args[1]}" != "--locked" \
                   || "${requested_args[2]}" != "-p" \
                   || "${requested_args[3]}" != "$selected_package" \
                   || "${requested_args[4]}" != "--lib" \
                   || "${requested_args[5]}" != "$reviewed_vendor_filter" ]]; then
                worker_failure "noninteractive agent Cargo refuses unreviewed workspace or package: ${selected_package}; requires its exact reviewed --lib contract filter"
              fi
              ;;
            *) worker_failure "noninteractive agent Cargo refuses unreviewed workspace or package: ${selected_package}" ;;
          esac
          ;;
      esac
    fi
  done
  case "$cargo_subcommand" in
    build|check|clippy|fmt|metadata|tree|bundle|rustc|rustdoc|publish-signed-bundle)
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
      requested_doctests=0
      for (( command_index=1; command_index<${#requested_args[@]}; command_index++ )); do
        argument="${requested_args[$command_index]}"
        package_name=""
        feature_selection=""
        case "$argument" in
          --doc|--doc=*)
            requested_doctests=1
            ;;
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

      # Only these production compile_fail examples are reviewed. Do not admit
      # general doctest discovery: documentation examples may launch real apps.
      if (( requested_doctests == 1 )); then
        if (( ${#requested_args[@]} != 6 )) \
          || [[ "${requested_args[0]}" != "test" \
             || "${requested_args[1]}" != "--locked" \
             || "${requested_args[2]}" != "--doc" \
             || "${requested_args[3]}" != "--package" \
             || "${requested_args[4]}" != "script-kit-gpui" \
             || "${requested_args[5]}" != "theme::alpha::" ]]; then
          worker_failure "noninteractive agent Cargo refuses unreviewed doctests; requires exact test --locked --doc --package script-kit-gpui theme::alpha::"
        fi
        reviewed_app_target=1
      fi

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
    --lockfile-path|--lockfile-path=*|--build-dir|--build-dir=*)
      worker_failure "Cargo storage ownership cannot be overridden by ${argument}"
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
  for native_setting in CMAKE_BUILD_PARALLEL_LEVEL RAYON_NUM_THREADS OMP_NUM_THREADS OPENBLAS_NUM_THREADS; do
    native_workers="${!native_setting:-1}"
    validate_worker_count "$native_workers" "$native_setting" 1
    export "${native_setting}=${native_workers}"
  done
fi

system_load_1m_json="null"
system_logical_cpus_json="null"
system_load_limit_percent_json="null"
system_load_reserved_workers_json="null"
max_system_load_percent="${SCRIPT_KIT_AGENT_MAX_SYSTEM_LOAD_PERCENT:-}"
if [[ -n "$max_system_load_percent" && "${SCRIPT_KIT_AGENT_POLICY_ONLY:-0}" != "1" ]]; then
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

target_mode="${SCRIPT_KIT_AGENT_TARGET_MODE:-pool}"
pool="$(owned_cache_id "${SCRIPT_KIT_CARGO_TARGET_POOL:-agent-debug}" SCRIPT_KIT_CARGO_TARGET_POOL)"
default_pool="agent-debug"

if [[ "$target_mode" != "pool" || "$pool" != "$default_pool" ]]; then
  worker_failure "agent work must use the single agent-debug pool"
fi
target_dir="${REPO_ROOT}/target-agent/pools/agent-debug"
lock_name="pool-agent-debug"
if [[ -n "${CARGO_TARGET_DIR:-}" && "$CARGO_TARGET_DIR" != "$target_dir" ]]; then
  worker_failure "inherited CARGO_TARGET_DIR conflicts with protected pool ownership"
fi

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

if [[ "${SCRIPT_KIT_AGENT_POLICY_ONLY:-0}" == "1" ]]; then
  printf '{"schemaVersion":3,"pool":"%s","jobs":%s,"test_threads":%s,"native_workers":1,"cachePolicy":"%s","incremental":{"enabled":false,"owner":"agent-cargo"},"cacheProbeStatus":"not-probed","passive":true}\n' "$pool" "$CARGO_BUILD_JOBS" "$RUST_TEST_THREADS" "$use_sccache"
  exit 0
fi
mkdir -p "$target_dir" "$lock_root" "$metal_module_cache"
export CARGO_TARGET_DIR="$target_dir"
export SCRIPT_KIT_METAL_MODULE_CACHE_DIR="$metal_module_cache"
export CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$metal_module_cache}"
export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-line-tables-only}"
# Managed compiler policy is authoritative; the human watcher does not use this lane.
export CARGO_INCREMENTAL=0 CARGO_BUILD_INCREMENTAL=false
export CARGO_PROFILE_DEV_INCREMENTAL=false CARGO_PROFILE_TEST_INCREMENTAL=false
export SCRIPT_KIT_AGENT_USE_SCCACHE="$use_sccache"
export SCRIPT_KIT_AGENT_LEASE_PATH="$lock_dir"
export SCRIPT_KIT_AGENT_LEASE_GENERATION="$(python3 -c 'import uuid; print(uuid.uuid4())')"
cargo_cache_lease acquire "$lock_dir" "$$" "$SCRIPT_KIT_AGENT_LEASE_GENERATION" "$(( ${SCRIPT_KIT_AGENT_LOCK_TIMEOUT_SEC:-600} * 1000 ))" >/dev/null
release_lock() { cargo_cache_lease release "$lock_dir" "$$" "$SCRIPT_KIT_AGENT_LEASE_GENERATION" >/dev/null; }
trap release_lock EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ "$registered_sidecar" == "1" ]]; then
  sidecar_source="${REPO_ROOT}/target/pi-sidecar/cache/source-3d1a3950c16ffdb10cd81780b26921c75c180770"
  sidecar_target="${REPO_ROOT}/target/pi-sidecar/cache/cargo-target"
  [[ "${PI_AGENT_RUST_CACHE_DIR:-${REPO_ROOT}/target/pi-sidecar/cache}" == "${REPO_ROOT}/target/pi-sidecar/cache" ]] || worker_failure "unregistered sidecar cache"
  [[ "${PI_AGENT_RUST_TARGET_DIR:-$sidecar_target}" == "$sidecar_target" ]] || worker_failure "unregistered sidecar target"
  [[ -f "${sidecar_source}/Cargo.toml" && ! -L "$sidecar_source" && ! -L "$sidecar_target" ]] || worker_failure "registered sidecar source missing/unsafe"
  export CARGO_TARGET_DIR="$sidecar_target"
  export SCRIPT_KIT_AGENT_SIDECAR_SOURCE="$sidecar_source"
fi
# The structured owner resolves semantic wrappers and probes the pinned rustc.
# Admission occurs after exact lease proof, within its cleanup/result boundary.
unset SCRIPT_KIT_AGENT_COMPILER_CACHE_WRAPPER SCRIPT_KIT_AGENT_COMPILER_CACHE_BACKEND
export SCRIPT_KIT_AGENT_ADMISSION_OBSERVATION="{\"systemLoad1m\":${system_load_1m_json},\"logicalCpus\":${system_logical_cpus_json},\"loadLimitPercent\":${system_load_limit_percent_json},\"reservedWorkers\":${system_load_reserved_workers_json}}"
touch "${target_dir}/.last_used"
cargo_args=("$@")
if [[ "$registered_sidecar" == "1" ]]; then
  cargo_args+=(--manifest-path "${SCRIPT_KIT_AGENT_SIDECAR_SOURCE}/Cargo.toml")
fi
if [[ "${SCRIPT_KIT_AGENT_TIMINGS:-0}" == "1" ]]; then
  case "${cargo_args[0]:-}" in
    build|check|test)
      timed_args=(); inserted=0
      for argument in "${cargo_args[@]}"; do
        [[ "$argument" == "--timings" || "$argument" == --timings=* ]] && inserted=1
        if [[ "$argument" == "--" && "$inserted" == "0" ]]; then timed_args+=("--timings"); inserted=1; fi
        timed_args+=("$argument")
      done
      [[ "$inserted" == "1" ]] || timed_args+=("--timings")
      cargo_args=("${timed_args[@]}")
      ;;
  esac
fi
echo "AGENT_CARGO mode=${target_mode} pool=${pool} jobs=${CARGO_BUILD_JOBS} test_threads=${RUST_TEST_THREADS} target_dir=${CARGO_TARGET_DIR} incremental=disabled compiler_cache=owner-qualified cargo ${cargo_args[*]}" >&2
# exec preserves the wrapper's exact PID/start identity and lease. This focused
# publisher delegates Cargo to the existing supervisor, releases this same lease,
# then finalizes the task; the six-verb facade never acquires a compiler lease.
exec bun "${SCRIPT_ROOT}/build-artifact.ts" run-wrapper "${cargo_args[@]}"
