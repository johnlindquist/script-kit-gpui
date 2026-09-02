#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
for worker_setting in CARGO_BUILD_JOBS RUST_TEST_THREADS; do
  worker_count="${!worker_setting:-2}"
  [[ "$worker_count" =~ ^[12]$ ]] || { echo "pi_sidecar invalid ${worker_setting}" >&2; exit 78; }
  export "${worker_setting}=${worker_count}"
done
native_workers="${CMAKE_BUILD_PARALLEL_LEVEL:-1}"
[[ "$native_workers" == "1" ]] || { echo 'pi_sidecar native worker budget must be one inside Cargo' >&2; exit 78; }
export CMAKE_BUILD_PARALLEL_LEVEL=1
PI_AGENT_RUST_URL="${PI_AGENT_RUST_URL:-https://github.com/Dicklesworthstone/pi_agent_rust.git}"
PI_AGENT_RUST_REF="3d1a3950c16ffdb10cd81780b26921c75c180770"
CACHE_ROOT="${PI_AGENT_RUST_CACHE_DIR:-${REPO_ROOT}/target/pi-sidecar/cache}"
OBJECT_CACHE="${CACHE_ROOT}/pi_agent_rust.git"
SOURCE_DIR="${CACHE_ROOT}/source-${PI_AGENT_RUST_REF}"
PI_TARGET_DIR="${PI_AGENT_RUST_TARGET_DIR:-${CACHE_ROOT}/cargo-target}"
PI_BIN="${PI_TARGET_DIR}/release/pi"
DEST_DIR="${REPO_ROOT}/target/pi-sidecar"
DEST="${DEST_DIR}/pi"

log() { echo "pi_sidecar $*"; }
if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" ]]; then
  [[ "$PI_AGENT_RUST_URL" == 'https://github.com/Dicklesworthstone/pi_agent_rust.git' \
    && "$CACHE_ROOT" == "${REPO_ROOT}/target/pi-sidecar/cache" \
    && "$PI_TARGET_DIR" == "${REPO_ROOT}/target/pi-sidecar/cache/cargo-target" ]] \
    || { echo 'pi_sidecar unregistered agent source/cache/target override' >&2; exit 78; }
fi
for path in "$CACHE_ROOT" "$OBJECT_CACHE" "$SOURCE_DIR" "$PI_TARGET_DIR" "$DEST_DIR" "$DEST"; do
  [[ ! -L "$path" ]] || { echo 'pi_sidecar refuses symlink ownership' >&2; exit 78; }
done
source "${REPO_ROOT}/scripts/agentic/cargo-cache-locks.sh"
lease="${REPO_ROOT}/target-agent/.locks/pi-sidecar-source.lock"
generation="$(python3 -c 'import uuid; print(uuid.uuid4())')"
cargo_cache_lease acquire "$lease" "$$" "$generation" 600000 >/dev/null
source_tmp=""; candidate=""
cleanup() {
  [[ -z "$source_tmp" ]] || rm -rf -- "$source_tmp"
  [[ -z "$candidate" ]] || rm -f -- "$candidate"
  cargo_cache_lease release "$lease" "$$" "$generation" >/dev/null
}
trap cleanup EXIT

mkdir -p "${CACHE_ROOT}" "${DEST_DIR}"

if [[ ! -d "${OBJECT_CACHE}" ]]; then
	log "initializing repo-managed object cache=${OBJECT_CACHE}"
	git init --bare "${OBJECT_CACHE}"
	git --git-dir="${OBJECT_CACHE}" remote add origin "${PI_AGENT_RUST_URL}"
fi

# Offline first: only contact the remote when the pinned commit is absent.
if ! git --git-dir="${OBJECT_CACHE}" cat-file -e "${PI_AGENT_RUST_REF}^{commit}" 2>/dev/null; then
	log "pinned ref missing from cache; fetching ref=${PI_AGENT_RUST_REF}"
	git --git-dir="${OBJECT_CACHE}" fetch --filter=blob:none --no-tags origin "${PI_AGENT_RUST_REF}"
fi
actual_ref="$(git --git-dir="${OBJECT_CACHE}" rev-parse "${PI_AGENT_RUST_REF}^{commit}")"
if [[ "${actual_ref}" != "${PI_AGENT_RUST_REF}" ]]; then
	echo "pi_sidecar expected ${PI_AGENT_RUST_REF}, got ${actual_ref}" >&2
	exit 1
fi

# Materialize an immutable source snapshot from the exact cached commit. This
# never reads or mutates an adjacent developer checkout.
if [[ ! -f "${SOURCE_DIR}/Cargo.toml" ]]; then
  source_tmp="${CACHE_ROOT}/.source-${PI_AGENT_RUST_REF}-${generation}"
  mkdir "${source_tmp}"
	git --git-dir="${OBJECT_CACHE}" archive "${PI_AGENT_RUST_REF}" | tar -x -C "${source_tmp}"
	mv "${source_tmp}" "${SOURCE_DIR}"
  source_tmp=""
fi

# The snapshot lives under the app repo's target/, so without its own
# [workspace] table cargo attaches it to the app workspace and refuses to
# build ("current package believes it's in a workspace when it's not").
if ! grep -q '^\[workspace\]' "${SOURCE_DIR}/Cargo.toml"; then
	printf '\n[workspace]\n' >> "${SOURCE_DIR}/Cargo.toml"
fi

python3 "${REPO_ROOT}/scripts/agentic/pi-sidecar-source.py" "$OBJECT_CACHE" "$PI_AGENT_RUST_REF" "$SOURCE_DIR"

log "build source=${SOURCE_DIR} ref=${actual_ref} target_dir=${PI_TARGET_DIR}"
if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" ]]; then
  bash "${REPO_ROOT}/scripts/agentic/agent-cargo.sh" pi-sidecar-build
else
  CARGO_TARGET_DIR="${PI_TARGET_DIR}" cargo build \
    --manifest-path "${SOURCE_DIR}/Cargo.toml" --locked --release --bin pi
fi

# Probe a candidate in the destination directory, then use same-directory
# rename so observers see either the previous healthy binary or the new one.
candidate="${DEST_DIR}/.pi.candidate.${generation}"
install -m 0755 "${PI_BIN}" "${candidate}"
bun "${REPO_ROOT}/scripts/agentic/pi-sidecar-health.ts" "${candidate}"
mv -f "${candidate}" "${DEST}"
candidate=""

log "ready dest=${DEST} ref=${actual_ref}"
