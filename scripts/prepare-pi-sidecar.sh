#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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
	source_tmp="${CACHE_ROOT}/.source-${PI_AGENT_RUST_REF}.$$"
	rm -rf "${source_tmp}"
	mkdir -p "${source_tmp}"
	cleanup_source() { rm -rf "${source_tmp}"; }
	trap cleanup_source EXIT
	git --git-dir="${OBJECT_CACHE}" archive "${PI_AGENT_RUST_REF}" | tar -x -C "${source_tmp}"
	mv "${source_tmp}" "${SOURCE_DIR}"
	trap - EXIT
fi

# The snapshot lives under the app repo's target/, so without its own
# [workspace] table cargo attaches it to the app workspace and refuses to
# build ("current package believes it's in a workspace when it's not").
if ! grep -q '^\[workspace\]' "${SOURCE_DIR}/Cargo.toml"; then
	printf '\n[workspace]\n' >> "${SOURCE_DIR}/Cargo.toml"
fi

log "build source=${SOURCE_DIR} ref=${actual_ref} target_dir=${PI_TARGET_DIR}"
CARGO_TARGET_DIR="${PI_TARGET_DIR}" cargo build \
	--manifest-path "${SOURCE_DIR}/Cargo.toml" \
	--locked --release --bin pi

# Probe a candidate in the destination directory, then use same-directory
# rename so observers see either the previous healthy binary or the new one.
candidate="${DEST_DIR}/.pi.candidate.$$"
cleanup_candidate() { rm -f "${candidate}"; }
trap cleanup_candidate EXIT
install -m 0755 "${PI_BIN}" "${candidate}"
bun "${REPO_ROOT}/scripts/agentic/pi-sidecar-health.ts" "${candidate}"
mv -f "${candidate}" "${DEST}"
trap - EXIT

log "ready dest=${DEST} ref=${actual_ref}"
