#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  echo 'usage: scripts/agent-check.sh [--quick] [--] [changed-file ...]'; exit 0
fi
for setting in SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER SCRIPT_KIT_ALLOW_NATIVE_INPUT SCRIPT_KIT_ALLOW_SCREEN_CAPTURE SCRIPT_KIT_ALLOW_VISIBLE_PROBES SCRIPT_KIT_ALLOW_LIVE_AI SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH; do
  [[ "${!setting:-0}" != "1" ]] || { echo "[agent-check] REFUSED unsafe ${setting}" >&2; exit 78; }
done
export SCRIPT_KIT_NONINTERACTIVE=1
exec bun "$ROOT/scripts/devtools/devtools.ts" build-ops act changed "$@"
