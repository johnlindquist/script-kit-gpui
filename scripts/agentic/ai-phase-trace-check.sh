#!/usr/bin/env bash
#
# The one command that re-checks the AI phase-trace work, cold.
#
#   bash scripts/agentic/ai-phase-trace-check.sh
#
# It runs, in order:
#   1. the shared trace module's own unit tests (redaction, latching,
#      concurrency, disabled-path cost);
#   2. the per-transport wiring proofs for Pi and Flows;
#   3. Quick AI's suite, to prove mirroring the shared trace did not regress it;
#   4. the analyzer's Bun tests;
#   5. the analyzer against the committed trace receipt, reprinting the
#      per-surface verdicts.
#
# Add --probe to ALSO drive live turns through the real app first. That needs a
# built binary, network, and provider auth, so it is opt-in: the default path
# is deterministic and offline.

set -uo pipefail
cd "$(dirname "$0")/../.."

PROBE=0
[[ "${1:-}" == "--probe" ]] && PROBE=1

CARGO=./scripts/agentic/agent-cargo.sh
export RUST_MIN_STACK=${RUST_MIN_STACK:-268435456}
# The committed receipt from the measured run. `.notes/` is gitignored, so a
# receipt left there would vanish on a fresh clone and this check would report
# a missing trace rather than the numbers it exists to re-print. Override with
# SCRIPT_KIT_AI_TRACE_RECEIPT to inspect a fresh probe run instead.
TRACE=${SCRIPT_KIT_AI_TRACE_RECEIPT:-scripts/agentic/fixtures/ai-phase-trace-receipt.ndjson}

fails=0
step() {
  local name="$1"; shift
  echo ""
  echo "=== $name ==="
  if "$@"; then
    echo "PASS: $name"
  else
    echo "FAIL: $name"
    fails=$((fails + 1))
  fi
}

rust_test() {
  # agent-cargo's output is noisy; require the explicit success line rather
  # than trusting an exit code that a piped tail could mask.
  local filter="$1"
  local out
  out=$(timeout 2400 "$CARGO" test --lib "$filter" 2>&1)
  echo "$out" | grep -E 'test result:' || { echo "$out" | tail -20; return 1; }
  echo "$out" | grep -qE 'test result: ok\.' || return 1
  echo "$out" | grep -qE 'test result: ok\. 0 passed' && {
    echo "REFUSING: filter '$filter' matched zero tests"; return 1; }
  return 0
}

step "shared phase trace unit tests"      rust_test ai::phase_trace
step "Pi transport wiring proof"          rust_test pi_transport_emits_the_phase_trace
step "Flows transport wiring proof"       rust_test flows::codex_client
step "Quick AI not regressed"             rust_test quick_ai
step "analyzer unit tests"                bash -c 'timeout 300 bun test scripts/agentic/ai-phase-trace-report.test.ts'

if [[ $PROBE -eq 1 ]]; then
  step "live probe (real turns)" bash -c \
    'timeout 900 bun scripts/agentic/ai-phase-trace-probe.ts --trials 6 --surfaces quick-ai'
  TRACE=.notes/oracle/ai-phase-trace-all/phase-trace.ndjson
fi

echo ""
echo "=== per-surface report ==="
if [[ -f "$TRACE" ]]; then
  timeout 120 bun scripts/agentic/ai-phase-trace-report.ts "$TRACE" || fails=$((fails + 1))
else
  echo "NO_TRACE_RECEIPT at $TRACE — run with --probe to generate one."
  fails=$((fails + 1))
fi

echo ""
if [[ $fails -eq 0 ]]; then
  echo "AI_PHASE_TRACE_CHECK=PASS"
  exit 0
fi
echo "AI_PHASE_TRACE_CHECK=FAIL failedSteps=$fails"
exit 1
