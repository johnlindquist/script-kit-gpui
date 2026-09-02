#!/usr/bin/env bash
# Stable publication is performed under agent-cargo's lease, never post-build copy.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
if [[ "${1:-}" == "--json" ]]; then shift; fi
TIMEOUT_SEC="${1:-1800}"
if [[ $# -gt 1 || ! "$TIMEOUT_SEC" =~ ^[1-9][0-9]*$ || "$TIMEOUT_SEC" -gt 7170 ]]; then
  echo '[build-isolated] timeout must be 1..7170 seconds' >&2; exit 64
fi
exec bun "$ROOT/scripts/devtools/devtools.ts" build-ops act app-build --timeout-ms "$((TIMEOUT_SEC * 1000))"
