#!/usr/bin/env bash
# Run reviewed, explicitly named Rust test groups from a current app harness
# without rebuilding/linking 15,000 unrelated tests for every filter.

set -euo pipefail

REPO_ROOT="${SCRIPT_KIT_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
POOL="${SCRIPT_KIT_CARGO_TARGET_POOL:-agent-debug}"

if [[ $# -eq 0 ]]; then
  echo "usage: scripts/agentic/reuse-rust-test-binary.sh <reviewed-test-filter> [additional-filter ...]" >&2
  exit 2
fi

if [[ ! "$POOL" =~ ^[a-zA-Z0-9._-]+$ || "$POOL" == "." || "$POOL" == ".." ]]; then
  echo "RUST_TEST_REUSE error: cached test pool must name one owned child; got ${POOL}" >&2
  exit 64
fi

POOL_DIR="${REPO_ROOT}/target-agent/pools/${POOL}"
DEPS_DIR="${POOL_DIR}/debug/deps"
for protected_pool_path in \
  "${REPO_ROOT}/target-agent" \
  "${REPO_ROOT}/target-agent/pools" \
  "$POOL_DIR" \
  "${POOL_DIR}/debug" \
  "$DEPS_DIR"; do
  if [[ -L "$protected_pool_path" ]]; then
    echo "RUST_TEST_REUSE error: cached test pool cannot follow a symlink: ${protected_pool_path}" >&2
    exit 64
  fi
done

for reviewed_filter in "$@"; do
  if [[ ! "$reviewed_filter" =~ ^[a-zA-Z_][a-zA-Z0-9_:]*$ ]]; then
    echo "RUST_TEST_REUSE error: reviewed Rust test filters must be nonempty identifier selectors; got ${reviewed_filter}" >&2
    exit 64
  fi
done

if [[ ! -d "$DEPS_DIR" ]]; then
  echo "RUST_TEST_REUSE error: no cached test harness; first run ./scripts/agentic/agent-cargo.sh test --lib --no-run" >&2
  exit 66
fi

binary=""
while IFS= read -r candidate; do
  if [[ -z "$binary" || "$candidate" -nt "$binary" ]]; then
    binary="$candidate"
  fi
done < <(find "$DEPS_DIR" -maxdepth 1 -type f -name 'script_kit_gpui-*' -perm -111 -print)

if [[ -z "$binary" ]]; then
  echo "RUST_TEST_REUSE error: no executable application-library harness exists" >&2
  exit 66
fi

source_inputs=(
  "${REPO_ROOT}/src" \
  "${REPO_ROOT}/crates" \
  "${REPO_ROOT}/vendor" \
  "${REPO_ROOT}/.cargo" \
  "${REPO_ROOT}/Cargo.toml" \
  "${REPO_ROOT}/Cargo.lock"
)
for embedded_input in \
  "${REPO_ROOT}/build.rs" \
  "${REPO_ROOT}/rust-toolchain.toml" \
  "${REPO_ROOT}/scripts/kit-sdk.ts" \
  "${REPO_ROOT}/kit-init" \
  "${REPO_ROOT}/assets"; do
  [[ -e "$embedded_input" ]] && source_inputs+=("$embedded_input")
done
stale_source="$(find "${source_inputs[@]}" -type f -newer "$binary" -print -quit 2>/dev/null)"
if [[ -n "$stale_source" ]]; then
  echo "RUST_TEST_REUSE error: cached harness is older than ${stale_source}; rebuild before claiming behavior proof" >&2
  exit 65
fi

export SCRIPT_KIT_NONINTERACTIVE=1
export SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=0
export SCRIPT_KIT_ALLOW_VISIBLE_PROBES=0
export SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER=0
export SCRIPT_KIT_ALLOW_LIVE_AI=0
export SCRIPT_KIT_ALLOW_NATIVE_INPUT=0
export SCRIPT_KIT_ALLOW_SCREEN_CAPTURE=0
export SCRIPT_KIT_SEARCH_FULL_STRESS=0
export SCRIPT_KIT_STORAGE_FULL_STRESS=0
export RUST_TEST_THREADS=1

echo "RUST_TEST_REUSE binary=${binary} filters=$# threads=1 noninteractive=1" >&2
for filter in "$@"; do
  echo "RUST_TEST_REUSE filter=${filter}" >&2
  "$binary" "$filter" --test-threads=1
done
