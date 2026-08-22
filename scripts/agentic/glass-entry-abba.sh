#!/bin/bash
# Paired, load-controlled before/after entry-color measurement (ABBA).
#
# Oracle plan floating-capsule-entry-material, step 4. Two PRESERVED builds:
#   A = alpha-zero baseline commit artifact
#   B = alpha-0.85 candidate commit artifact
# 3 discarded warmups per build, then 5 blocks of A,B,B,A (20 accepted runs).
# A run is ELIGIBLE only when the 1-minute load average is <= 6.0 at both
# boundaries and the thermal state shows no CPU speed limit. Ineligible runs
# are recorded and rejected, never silently retried into the accepted set.
#
# Usage:
#   scripts/agentic/glass-entry-abba.sh \
#     --a target-agent/artifacts/alpha0-baseline/script-kit-gpui \
#     --b target-agent/artifacts/alpha085-candidate/script-kit-gpui \
#     --out .artifacts/glass-entry-abba/<date>
#   [--blocks 5] [--warmups 3]
#
# Per accepted run the receipts are:
#   <out>/<run-id>/lifecycle/receipt.json          lifecycle filmstrip
#   <out>/<run-id>/entry-metrics.json              schema-v2 color metric
#   <out>/runs.jsonl                               one line per run w/ gates
# Summarize with: python3 scripts/agentic/glass-entry-abba-summary.py <out>
set -euo pipefail

A_BINARY="" B_BINARY="" OUT="" BLOCKS=5 WARMUPS=3
while [ $# -gt 0 ]; do
  case "$1" in
    --a) A_BINARY="$2"; shift 2 ;;
    --b) B_BINARY="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --blocks) BLOCKS="$2"; shift 2 ;;
    --warmups) WARMUPS="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done
[ -x "$A_BINARY" ] || { echo "missing A binary: $A_BINARY" >&2; exit 2; }
[ -x "$B_BINARY" ] || { echo "missing B binary: $B_BINARY" >&2; exit 2; }
[ -n "$OUT" ] || { echo "--out required" >&2; exit 2; }

# Disabled by owner request (2026-08-13): this study launches the app over a
# full-screen saturated-stripes (rainbow) backdrop and drives it — a complete
# screen takeover. Opt in only for a deliberate, supervised capture session.
if [ "${SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER:-0}" != "1" ]; then
  echo "glass-entry-abba.sh disabled: it launches Script Kit over a full-screen rainbow backdrop (screen takeover). Set SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER=1 to run deliberately." >&2
  exit 3
fi
mkdir -p "$OUT"

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
FIXTURE="$REPO/scripts/agentic/fixtures/glass-motion-calibration-theme.json"
METRIC="$REPO/scripts/agentic/glass-motion-color-metrics.py"
LIFECYCLE="$REPO/scripts/devtools/glass-lifecycle-filmstrip.ts"
DRAG="$REPO/scripts/devtools/main-window-native-drag.ts"
RUNS="$OUT/runs.jsonl"

sha() { shasum -a 256 "$1" | awk '{print $1}'; }
load1() { sysctl -n vm.loadavg | awk '{print $2}'; }
# Apple Silicon hosts report "Note: No CPU power status has been recorded"
# instead of an Intel-style CPU_Speed_Limit line. That sentence is pmset's
# positive statement that no throttle event has occurred — parse it as
# not-limited. Empty or unrecognized output still fails closed: "unknown" is
# never equal to "false", so eligibility stays red without evidence.
therm_limited() {
  local out
  out="$(pmset -g therm 2>/dev/null || true)"
  case "$out" in
    *CPU_Speed_Limit*)
      printf '%s\n' "$out" | awk -F= '/CPU_Speed_Limit/ {gsub(/ /,"",$2); print ($2+0 < 100) ? "true" : "false"}' ;;
    *"No CPU power status has been recorded"*)
      echo "false" ;;
    *)
      echo "unknown" ;;
  esac
}

record_env() { # $1 = file prefix
  uptime > "$1.uptime.txt"
  pmset -g therm > "$1.therm.txt" 2>&1 || true
  # `|| true` because under pipefail, sort surfaces head's early-exit as
  # SIGPIPE (141) and set -e would kill the whole session on a snapshot.
  { ps -A -o %cpu,pid,comm | sort -nr | head -20 > "$1.top.txt"; } || true
}

# Deterministic backdrop fixture (same helper the canonical
# glass-motion-contrast cells use). Without it the entry frames sit over the
# live desktop and the background-reference envelope recovery cannot work.
FIXTURE_HELPER="$OUT/macos-glass-background-fixture"
if [ ! -x "$FIXTURE_HELPER" ]; then
  xcrun swiftc -O "$REPO/scripts/agentic/macos-glass-background-fixture.swift" \
    -o "$FIXTURE_HELPER"
fi
FIXTURE_RECEIPT="$OUT/fixture.json"
"$FIXTURE_HELPER" --mode saturated-stripes --receipt "$FIXTURE_RECEIPT" &
FIXTURE_PID=$!
trap '[ -n "${FIXTURE_PID:-}" ] && kill "$FIXTURE_PID" 2>/dev/null || true' EXIT
for _ in $(seq 1 100); do
  [ -f "$FIXTURE_RECEIPT" ] && break
  sleep 0.1
done
[ -f "$FIXTURE_RECEIPT" ] || { echo "backdrop fixture failed to start" >&2; exit 2; }

# Session identity, captured once.
{
  echo "{"
  echo "  \"aBinary\": \"$A_BINARY\", \"aSha256\": \"$(sha "$A_BINARY")\","
  echo "  \"bBinary\": \"$B_BINARY\", \"bSha256\": \"$(sha "$B_BINARY")\","
  echo "  \"fixture\": \"$FIXTURE\", \"fixtureSha256\": \"$(sha "$FIXTURE")\","
  echo "  \"macos\": \"$(sw_vers -productVersion) $(sw_vers -buildVersion)\","
  echo "  \"startedAt\": \"$(date -u +%FT%TZ)\""
  echo "}"
} > "$OUT/session.json"
system_profiler SPDisplaysDataType -json > "$OUT/displays.json" 2>/dev/null || true

# Per-build layout receipt (metric --receipt input; appKit geometry for the
# entry analysis itself comes from the lifecycle receipt's settledLayout).
layout_receipt() { # $1 = build tag, $2 = binary -> echoes receipt path
  local dir="$OUT/layout-$1"
  if [ ! -f "$dir/receipt.json" ]; then
    # The probe exits nonzero in --stationary-only --widths none mode (no
    # trials to grade); the receipt is a provenance input for the entry
    # metric, so gate on the file, not the exit code.
    SCRIPT_KIT_TEST_STATUS=1 bun "$DRAG" --binary "$2" --out "$dir" \
      --stationary-only --widths none \
      >"$dir.stdout.txt" 2>"$dir.stderr.txt" || true
  fi
  [ -f "$dir/receipt.json" ] || { echo "layout receipt missing for $1" >&2; return 1; }
  echo "$dir/receipt.json"
}

one_run() { # $1 = run id, $2 = build tag (A|B), $3 = binary, $4 = accepted(bool)
  local run_dir="$OUT/$1" pre_load post_load pre_limited post_limited
  mkdir -p "$run_dir"
  record_env "$run_dir/pre"
  pre_load="$(load1)"; pre_limited="$(therm_limited)"
  local layout="" lifecycle_exit=0 metric_exit=0
  layout="$(layout_receipt "$2" "$3")" || metric_exit=98
  SCRIPT_KIT_TEST_STATUS=1 SCRIPT_KIT_GLASS_SCENARIO=lifecycle \
    bun "$LIFECYCLE" --binary "$3" --theme-fixture "$FIXTURE" \
    --out "$run_dir/lifecycle" >"$run_dir/lifecycle.stdout" \
    2>"$run_dir/lifecycle.stderr" \
    || lifecycle_exit=$?
  if [ "$metric_exit" -eq 0 ] && [ -f "$run_dir/lifecycle/receipt.json" ]; then
    python3 "$METRIC" --receipt "$layout" \
      --lifecycle-receipt "$run_dir/lifecycle/receipt.json" \
      --scenario main-entry --out "$run_dir/entry-metrics.json" \
      >/dev/null 2>"$run_dir/metric.stderr" || metric_exit=$?
  else
    metric_exit=97
  fi
  record_env "$run_dir/post"
  post_load="$(load1)"; post_limited="$(therm_limited)"
  local eligible=true
  awk -v a="$pre_load" -v b="$post_load" 'BEGIN{exit !(a<=6.0 && b<=6.0)}' || eligible=false
  [ "$pre_limited" = "false" ] && [ "$post_limited" = "false" ] || eligible=false
  python3 - "$RUNS" <<PY
import json, sys, pathlib
run_dir = pathlib.Path("$run_dir")
metrics_path = run_dir / "entry-metrics.json"
metrics = json.loads(metrics_path.read_text()) if metrics_path.exists() else None
summary = (metrics or {}).get("summary") or {}
alpha = summary.get("alphaPolicy") or {}
row = {
    "run": "$1", "build": "$2",
    "accepted": "$4" == "true", "eligible": "$eligible" == "true",
    "preLoad1": float("$pre_load"), "postLoad1": float("$post_load"),
    "thermLimited": "$pre_limited" == "true" or "$post_limited" == "true",
    "lifecycleExit": $lifecycle_exit, "metricExit": $metric_exit,
    "metricPass": (metrics or {}).get("pass"),
    "runMaximumDisplayedEntryDeltaE00": summary.get("maximumDisplayedEntryDeltaE00"),
    "maximumCapsuleStageRelationDriftDeltaE00": summary.get("maximumCapsuleStageRelationDriftDeltaE00"),
    "firstVisibleEntryAlpha": alpha.get("firstVisibleEntryAlpha"),
    "minimumVisibleEntryAlpha": alpha.get("minimumVisibleEntryAlpha"),
    "visibleFramesBelowAlphaFloor": len(alpha.get("visibleFramesBelowAlphaFloor") or []),
    "visibleZeroAlphaFrames": len(alpha.get("visibleZeroAlphaFrames") or []),
    "unmeasurableVisibleFrameCount": alpha.get("unmeasurableVisibleFrameCount"),
    "alphaPolicyPass": alpha.get("pass"),
    "errors": (metrics or {}).get("errors"),
}
with open(sys.argv[1], "a") as f:
    f.write(json.dumps(row) + "\n")
print(f"{row['run']} {row['build']} eligible={row['eligible']} "
      f"maxDisplayed={row['runMaximumDisplayedEntryDeltaE00']} "
      f"alphaPass={row['alphaPolicyPass']}")
PY
}

echo "== warmups (discarded) =="
# NOTE: BSD seq counts DOWN for `seq 1 0`; guard zero warmups explicitly
# (same defect class as the --blocks guard below; locked by
# scripts/agentic/glass-entry-abba.test.ts).
for i in $(test "$WARMUPS" -ge 1 && seq 1 "$WARMUPS"); do
  one_run "warmup-A-$i" A "$A_BINARY" false
  one_run "warmup-B-$i" B "$B_BINARY" false
done

echo "== accepted blocks (A B B A x $BLOCKS) =="
n=0
# NOTE: BSD seq counts DOWN for `seq 1 0`; guard zero blocks explicitly.
[ "$BLOCKS" -ge 1 ] || BLOCKS=0
for block in $(test "$BLOCKS" -ge 1 && seq 1 "$BLOCKS"); do
  for tag in A B B A; do
    n=$((n + 1))
    if [ "$tag" = A ]; then bin="$A_BINARY"; else bin="$B_BINARY"; fi
    one_run "run-$(printf '%02d' "$n")-$tag" "$tag" "$bin" true
  done
done

echo "done: $RUNS"
