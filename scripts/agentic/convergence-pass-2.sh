#!/usr/bin/env bash
# Convergence Sweep PASS #2 runner.
#
# Roster authority: .notes/chaos-ledger.md battery records (27 gates), never a
# filename glob. Full execution is deliberately double-gated by manager GO and
# a SCREEN acknowledgement. `--row-0` is the only pre-GO plumbing smoke.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

OUTPUT_BASE="$REPO_ROOT/.test-output/convergence-pass-2"
ARTIFACT="$REPO_ROOT/target-agent/artifacts/convergence-pass-2/script-kit-gpui"
LEDGER="$REPO_ROOT/.notes/chaos-ledger.md"
ROW_TIMEOUT_SECONDS="${CONVERGENCE_ROW_TIMEOUT_SECONDS:-300}"
TIMEOUT_BIN="$(command -v timeout || command -v gtimeout || true)"
MODE="${1:-}"
SCREEN_CLAIMED=0
RUN_ROOT=""
ARTIFACT_SHA256=""

usage() {
  cat <<'EOF'
Usage:
  scripts/agentic/convergence-pass-2.sh --row-0
      Build and hash the frozen artifact, then run one cheap hidden lifecycle
      probe. This does NOT claim SCREEN or start any sweep row.

  CONVERGENCE_PASS_2_MANAGER_GO=1 \
  CONVERGENCE_PASS_2_SCREEN_CLAIMED=1 \
    scripts/agentic/convergence-pass-2.sh --run
      Run the approved 27-gate sweep sequentially. Do not use before manager GO.

  scripts/agentic/convergence-pass-2.sh --list
      Print the ledger-derived roster without building or running anything.
EOF
}

roster() {
  cat <<'EOF'
01 chaos-corrupt-state.ts
02 chaos-encoding-edges.ts
03 chaos-protocol-fuzz.ts
04 chaos-interaction-stress.ts
05 chaos-input-nav-21b-probe.ts
06 chaos-focus-escape-ladder-probe.ts (hidden + frontmost modes)
07 chaos-actions-dialog-storm-probe.ts
08 chaos-builtin-prompt-surfaces-probe.ts
09 chaos-clipboard-history-hostile-probe.ts
10 chaos-dir-browse-churn.ts
11 chaos-frecency-input-history-probe.ts
12 chaos-script-prompts-probe.ts
13 chaos-terminal-surface-probe.ts
14 chaos-smoke-sheet.ts
15 chaos-cls-perf-probe.ts
16 chaos-huge-input-latency.ts
17 chaos-multisurface-perf.ts
18 chaos-perf-factors.ts
19 chaos-perf-attribute.ts
20 chaos-perf-busy.ts
21 chaos-input-nav-storms-probe.ts
22 chaos-input-nav-21b-frontmost.ts
23 chaos-prompt-cls-layout-probe.ts
24 root-typing-lag-benchmark.ts --enforce
25 notes-editor-hostile-chaos-probe.ts
26 notes-window-file-churn-probe.ts
27 day-editor-rapid-newline-scroll-probe.ts
EOF
}

loadavg_1m() {
  sysctl -n vm.loadavg | tr -d '{}' | awk '{print $1}'
}

timestamp() {
  date -u '+%Y-%m-%dT%H:%M:%SZ'
}

build_and_hash_artifact() {
  mkdir -p "$RUN_ROOT"
  {
    printf 'started_at=%s\n' "$(timestamp)"
    printf 'command=SCRIPT_KIT_AGENT_TARGET_BUDGET_GB=55 SCRIPT_KIT_AGENT_ARTIFACT_NAME=convergence-pass-2 ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui\n'
  } > "$RUN_ROOT/artifact-build.meta"

  SCRIPT_KIT_AGENT_TARGET_BUDGET_GB=55 \
    SCRIPT_KIT_AGENT_ARTIFACT_NAME=convergence-pass-2 \
    ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui \
    > "$RUN_ROOT/artifact-build.log" 2>&1

  test -x "$ARTIFACT"
  ARTIFACT_SHA256="$(shasum -a 256 "$ARTIFACT" | awk '{print $1}')"
  printf '%s  %s\n' "$ARTIFACT_SHA256" "$ARTIFACT" > "$RUN_ROOT/artifact.sha256"
  printf 'completed_at=%s\nsha256=%s\n' "$(timestamp)" "$ARTIFACT_SHA256" \
    >> "$RUN_ROOT/artifact-build.meta"
}

verify_frozen_artifact() {
  local current
  current="$(shasum -a 256 "$ARTIFACT" | awk '{print $1}')"
  if [[ "$current" != "$ARTIFACT_SHA256" ]]; then
    echo "artifact hash changed: expected=$ARTIFACT_SHA256 actual=$current" >&2
    return 1
  fi
}

claim_screen() {
  # The manager-approved invocation requires the caller to post the claim
  # before launch; this acknowledgement arms the single release finalizer.
  SCREEN_CLAIMED=1
}

release_screen() {
  if [[ "$SCREEN_CLAIMED" == "1" ]]; then
    printf -- '- %s L4 monkey-x-perf: **SCREEN RELEASED — CONVERGENCE SWEEP PASS #2 runner finalized. NEXT: manager queue.** Run root `%s`.\n' "$(timestamp)" "$RUN_ROOT" >> "$LEDGER"
    SCREEN_CLAIMED=0
  fi
}

abort_run() {
  release_screen
  trap - INT TERM
  exit 130
}

run_row_command() {
  local row="$1"
  local row_dir="$2"

  case "$row" in
    01) bun scripts/agentic/chaos-corrupt-state.ts ;;
    02) bun scripts/agentic/chaos-encoding-edges.ts ;;
    03) bun scripts/agentic/chaos-protocol-fuzz.ts ;;
    04) bun scripts/agentic/chaos-interaction-stress.ts ;;
    05) bun scripts/agentic/chaos-input-nav-21b-probe.ts ;;
    06)
      mkdir -p "$row_dir/hidden" "$row_dir/frontmost"
      CONVERGENCE_ROW_OUTPUT_DIR="$row_dir/hidden" \
        bun scripts/agentic/chaos-focus-escape-ladder-probe.ts
      CHAOS_FRONTMOST=1 CONVERGENCE_ROW_OUTPUT_DIR="$row_dir/frontmost" \
        bun scripts/agentic/chaos-focus-escape-ladder-probe.ts
      ;;
    07) bun scripts/agentic/chaos-actions-dialog-storm-probe.ts ;;
    08) bun scripts/agentic/chaos-builtin-prompt-surfaces-probe.ts ;;
    09) bun scripts/agentic/chaos-clipboard-history-hostile-probe.ts ;;
    10) bun scripts/agentic/chaos-dir-browse-churn.ts ;;
    11) bun scripts/agentic/chaos-frecency-input-history-probe.ts ;;
    12) bun scripts/agentic/chaos-script-prompts-probe.ts ;;
    13) bun scripts/agentic/chaos-terminal-surface-probe.ts ;;
    14) bun scripts/agentic/chaos-smoke-sheet.ts ;;
    15) bun scripts/agentic/chaos-cls-perf-probe.ts ;;
    16) bun scripts/agentic/chaos-huge-input-latency.ts ;;
    17) bun scripts/agentic/chaos-multisurface-perf.ts ;;
    18) bun scripts/agentic/chaos-perf-factors.ts ;;
    19) bun scripts/agentic/chaos-perf-attribute.ts ;;
    20) bun scripts/agentic/chaos-perf-busy.ts ;;
    21) bun scripts/agentic/chaos-input-nav-storms-probe.ts ;;
    22) bun scripts/agentic/chaos-input-nav-21b-frontmost.ts ;;
    23) bun scripts/agentic/chaos-prompt-cls-layout-probe.ts ;;
    24)
      bun scripts/agentic/root-typing-lag-benchmark.ts \
        --enforce --output-dir "$row_dir/benchmark"
      ;;
    25) bun scripts/agentic/notes-editor-hostile-chaos-probe.ts ;;
    26) bun scripts/agentic/notes-window-file-churn-probe.ts ;;
    27) bun scripts/agentic/day-editor-rapid-newline-scroll-probe.ts ;;
    *) echo "unknown convergence row: $row" >&2; return 2 ;;
  esac
}
export -f run_row_command

run_row() {
  local row="$1"
  local name="$2"
  local row_dir="$RUN_ROOT/$row-$name"
  local before after exit_code verdict

  mkdir -p "$row_dir/home" "$row_dir/tmp" "$row_dir/sessions"
  verify_frozen_artifact
  before="$(loadavg_1m)"
  printf 'row=%s\nname=%s\nstarted_at=%s\nloadavg_1m_before=%s\nartifact_sha256=%s\ntimeout_seconds=%s\n' \
    "$row" "$name" "$(timestamp)" "$before" "$ARTIFACT_SHA256" "$ROW_TIMEOUT_SECONDS" > "$row_dir/row.meta"

  set +e
  HOME="$row_dir/home" \
    SK_PATH="$row_dir/home/.scriptkit" \
    TMPDIR="$row_dir/tmp" \
    SCRIPT_KIT_SESSION_DIR="$row_dir/sessions" \
    SCRIPT_KIT_GPUI_BINARY="$ARTIFACT" \
    PROBE_BINARY="$ARTIFACT" \
    CONVERGENCE_ROW_OUTPUT_DIR="$row_dir" \
    "$TIMEOUT_BIN" --signal=TERM --kill-after=5 "${ROW_TIMEOUT_SECONDS}s" \
      bash -c 'run_row_command "$1" "$2"' _ "$row" "$row_dir" \
      > "$row_dir/console.log" 2>&1
  exit_code=$?
  set -e

  after="$(loadavg_1m)"
  verdict="FAIL"
  [[ "$exit_code" -eq 0 ]] && verdict="PASS"
  printf 'completed_at=%s\nloadavg_1m_after=%s\nexit_code=%s\nverdict=%s\n' \
    "$(timestamp)" "$after" "$exit_code" "$verdict" >> "$row_dir/row.meta"

  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$row" "$name" "$exit_code" "$before" "$after" "$verdict" >> "$RUN_ROOT/results.tsv"
}

run_row_zero() {
  local row_dir="$RUN_ROOT/00-row-0-hidden-plumbing"
  local before after exit_code
  mkdir -p "$row_dir"
  before="$(loadavg_1m)"
  set +e
  SCRIPT_KIT_GPUI_BINARY="$ARTIFACT" \
    bun scripts/agentic/root-typing-lag-benchmark.ts \
      --hidden-dry-run --output-dir "$row_dir/probe" \
      > "$row_dir/console.log" 2>&1
  exit_code=$?
  set -e
  after="$(loadavg_1m)"

  # Hidden dry-run intentionally exits 1 at the show/focus boundary. Prove
  # lifecycle cleanup and exact diagnostic instead of treating exit 1 as red.
  [[ "$exit_code" -eq 1 ]]
  jq -e '
    .hiddenDryRun == true and
    .observationMode == "event_driven_wait_for" and
    .cleanup.hidden == true and
    .cleanup.stopped == true and
    .cleanup.error == null and
    (.failure | contains("expected show/focus boundary"))
  ' "$row_dir/probe/receipt.json" > "$row_dir/receipt-check.json"

  printf 'row=00\nname=row-0-hidden-plumbing\nexit_code=%s\nexpected_diagnostic=true\nloadavg_1m_before=%s\nloadavg_1m_after=%s\nartifact_sha256=%s\n' \
    "$exit_code" "$before" "$after" "$ARTIFACT_SHA256" > "$row_dir/row.meta"
}

case "$MODE" in
  --list)
    roster
    ;;
  --row-0)
    RUN_ROOT="$OUTPUT_BASE/row-0-$(date -u '+%Y%m%dT%H%M%SZ')-$$"
    mkdir -p "$RUN_ROOT"
    build_and_hash_artifact
    verify_frozen_artifact
    run_row_zero
    printf 'ROW_0_PASS run_root=%s sha256=%s\n' "$RUN_ROOT" "$ARTIFACT_SHA256"
    ;;
  --run)
    if [[ "${CONVERGENCE_PASS_2_MANAGER_GO:-0}" != "1" ]]; then
      echo "refusing full sweep: CONVERGENCE_PASS_2_MANAGER_GO=1 is required" >&2
      exit 2
    fi
    if [[ "${CONVERGENCE_PASS_2_SCREEN_CLAIMED:-0}" != "1" ]]; then
      echo "refusing full sweep: CONVERGENCE_PASS_2_SCREEN_CLAIMED=1 is required" >&2
      exit 2
    fi
    if [[ -z "$TIMEOUT_BIN" ]]; then
      echo "refusing full sweep: timeout or gtimeout is required" >&2
      exit 2
    fi
    RUN_ROOT="$OUTPUT_BASE/run-$(date -u '+%Y%m%dT%H%M%SZ')-$$"
    mkdir -p "$RUN_ROOT"
    roster > "$RUN_ROOT/roster.txt"
    printf 'row\tname\texit_code\tloadavg_before\tloadavg_after\tverdict\n' > "$RUN_ROOT/results.tsv"
    trap release_screen EXIT
    trap abort_run INT TERM
    build_and_hash_artifact
    claim_screen
    run_row 01 corrupt-state
    run_row 02 encoding-edges
    run_row 03 protocol-fuzz
    run_row 04 interaction-stress
    run_row 05 input-nav-21b-hidden
    run_row 06 focus-escape-ladder
    run_row 07 actions-dialog-storm
    run_row 08 builtin-prompt-surfaces
    run_row 09 clipboard-history-hostile
    run_row 10 dir-browse-churn
    run_row 11 frecency-input-history
    run_row 12 script-prompts
    run_row 13 terminal-surface
    run_row 14 smoke-sheet
    run_row 15 cls-perf
    run_row 16 huge-input-latency
    run_row 17 multisurface-perf
    run_row 18 perf-factors
    run_row 19 perf-attribute
    run_row 20 perf-busy
    run_row 21 input-nav-storms
    run_row 22 input-nav-21b-frontmost
    run_row 23 prompt-cls-layout
    run_row 24 root-typing-lag
    run_row 25 notes-editor-hostile
    run_row 26 notes-window-file-churn
    run_row 27 day-editor-rapid-newline-scroll
    if awk -F '\t' 'NR > 1 && $6 != "PASS" { bad=1 } END { exit bad }' "$RUN_ROOT/results.tsv"; then
      printf 'CONVERGENCE_PASS_2_PASS run_root=%s sha256=%s\n' "$RUN_ROOT" "$ARTIFACT_SHA256"
    else
      echo "CONVERGENCE_PASS_2_NOT_CLEAN run_root=$RUN_ROOT" >&2
      exit 1
    fi
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
