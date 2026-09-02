#!/usr/bin/env bash
# Validate or repair the Pi binary selected by the debug runtime.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SIDECAR="${REPO_ROOT}/target/pi-sidecar/pi"
MODE="${1:---auto}"

log() { echo "[ensure-pi-sidecar] $*" >&2; }
usage() {
	echo "usage: scripts/agentic/ensure-pi-sidecar.sh [--check|--repair]" >&2
	exit 2
}

case "${MODE}" in
--auto | --check | --repair) ;;
*) usage ;;
esac
if [[ $# -gt 1 ]]; then
	usage
fi
if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" && "$MODE" == "--auto" ]]; then
  log 'ERROR: agent provisioning requires explicit --repair (inspection never provisions)'
  exit 78
fi

health_receipt() {
	bun "${REPO_ROOT}/scripts/agentic/pi-sidecar-health.ts" "$1"
}

healthy() {
	local candidate="$1"
	[[ -x "${candidate}" ]] && health_receipt "${candidate}"
}

resolved() {
	log "pi available: $1"
	exit 0
}

# Rust's default_pi_binary treats an explicit override as authoritative. Never
# let a healthy repo-local sidecar mask a broken override.
if [[ -n "${SCRIPT_KIT_PI_BINARY:-}" ]]; then
	override="${SCRIPT_KIT_PI_BINARY/#\~/$HOME}"
	if healthy "${override}"; then
		resolved "${override} (SCRIPT_KIT_PI_BINARY)"
	fi
	log "ERROR: explicit SCRIPT_KIT_PI_BINARY=${SCRIPT_KIT_PI_BINARY} is not RPC-healthy"
	log "Unset the override or repair that exact binary; fallback binaries are intentionally ignored."
	exit 1
fi

# An executable target sidecar is also authoritative in debug Rust resolution.
# Default mode checks first and repairs only when needed; explicit --repair
# deliberately rebuilds the managed pinned candidate.
if [[ "${MODE}" != "--repair" ]] && healthy "${SIDECAR}"; then
	resolved "${SIDECAR}"
fi
if [[ "${MODE}" == "--check" ]]; then
	if [[ -x "${SIDECAR}" ]]; then
		log "ERROR: runtime-selected ${SIDECAR} is executable but unhealthy"
		exit 1
	fi
	for candidate in "${HOME}/dev/pi_agent_rust/target/release/pi" "${HOME}/dev/pi_agent_rust/target/debug/pi"; do
		if healthy "${candidate}"; then
			resolved "${candidate} (debug fallback)"
		fi
	done
	log "ERROR: no RPC-healthy Pi binary resolves"
	exit 1
fi

if [[ -x "${SIDECAR}" ]]; then
	log "repairing unhealthy runtime-selected sidecar: ${SIDECAR}"
else
	log "preparing missing repo-local Pi sidecar"
fi
bash "${REPO_ROOT}/scripts/prepare-pi-sidecar.sh"
if healthy "${SIDECAR}"; then
	resolved "${SIDECAR} (repaired)"
fi

log "ERROR: prepare-pi-sidecar.sh completed but ${SIDECAR} is not RPC-healthy"
exit 1
