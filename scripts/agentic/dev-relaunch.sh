#!/usr/bin/env bash
# scripts/agentic/dev-relaunch.sh — Stop stale session and start a fresh one after a successful build.
# Called by dev.sh after `cargo build --quiet` succeeds.
#
# Usage: dev-relaunch.sh [SESSION_NAME]
# Output: session JSON on stdout, DEV_SESSION_RELAUNCH status lines on stderr.

set -euo pipefail

SESSION_NAME="${1:-${SCRIPT_KIT_DEV_SESSION_NAME:-dev-watch}}"
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SESSION_SCRIPT="${PROJECT_ROOT}/scripts/agentic/session.sh"

echo "DEV_SESSION_RELAUNCH session=${SESSION_NAME} phase=stop-old" >&2
if ! bash "${SESSION_SCRIPT}" stop "${SESSION_NAME}" >/dev/null; then
  echo "DEV_SESSION_RELAUNCH refused: previous exact session teardown not confirmed" >&2
  exit 78
fi

echo "DEV_SESSION_RELAUNCH session=${SESSION_NAME} phase=start-new" >&2
set +e
RESULT="$(bash "${SESSION_SCRIPT}" start "${SESSION_NAME}")"
START_STATUS=$?
set -e
printf '%s\n' "${RESULT}"

RESULT_JSON="${RESULT}" START_STATUS="${START_STATUS}" python3 - <<'PY'
import json
import os
import sys
start_status = os.environ.get("START_STATUS", "?")
try:
    data = json.loads(os.environ["RESULT_JSON"])
except Exception as exc:
    print(f"DEV_SESSION_RELAUNCH phase=parse-json status={start_status} error={exc}", file=sys.stderr)
    raise SystemExit(0)
print(
    "DEV_SESSION_RELAUNCH "
    f"session={data.get('session')} "
    f"status={data.get('status')} "
    f"startStatus={start_status} "
    f"ready={data.get('ready')} "
    f"readyMarker={data.get('readyMarker')} "
    f"readyWaitMs={data.get('readyWaitMs')}",
    file=sys.stderr,
)
PY
exit "${START_STATUS}"
