#!/usr/bin/env bash

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

QUICK_MODE=false
MAX_RELATED_FILTERS=4
declare -a CHANGED_FILES=()

usage() {
  cat <<'USAGE'
Usage: bash scripts/agent-check.sh [--quick] [changed-file ...]

Runs scoped verification for agent changes.

Options:
  --quick      Run only agent-cargo check (fast iteration mode)
  -h, --help   Show this help message

Examples:
  bash scripts/agent-check.sh
  bash scripts/agent-check.sh --quick src/actions/dialog.rs
  bash scripts/agent-check.sh src/actions/dialog.rs src/theme.rs
USAGE
}

format_duration() {
  local total_seconds="$1"
  printf "%02dm%02ds" "$((total_seconds / 60))" "$((total_seconds % 60))"
}

run_step() {
  local label="$1"
  shift

  local step_start step_end elapsed
  step_start="$(date +%s)"
  echo ""
  echo ">> $label"
  "$@"
  local status=$?
  step_end="$(date +%s)"
  elapsed="$((step_end - step_start))"

  if [[ "$status" -eq 0 ]]; then
    echo "[PASS] $label ($(format_duration "$elapsed"))"
  else
    echo "[FAIL] $label ($(format_duration "$elapsed"))"
  fi

  return "$status"
}

is_noise_token() {
  local token="$1"
  case "$token" in
    src|scripts|tests|test|smoke|sdk|target|debug|release|main|lib|mod|bin|doc|docs|json|yaml|yml|toml|lock|rs|ts|tsx|js|jsx|sh|md)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

derive_related_filters() {
  local changed_path token_stream token
  local file_catalog token_lines unique_tokens
  local count

  if [[ "${#CHANGED_FILES[@]}" -eq 0 ]]; then
    return 0
  fi

  file_catalog="$(rg --files tests src 2>/dev/null || true)"
  if [[ -z "$file_catalog" ]]; then
    return 0
  fi

  token_lines=""
  for changed_path in "${CHANGED_FILES[@]}"; do
    changed_path="${changed_path#./}"
    token_stream="${changed_path//\// }"
    token_stream="${token_stream//./ }"
    token_stream="${token_stream//_/ }"
    token_stream="${token_stream//-/ }"

    for token in $token_stream; do
      token="$(printf '%s' "$token" | tr '[:upper:]' '[:lower:]')"
      [[ "${#token}" -lt 3 ]] && continue
      is_noise_token "$token" && continue
      token_lines="${token_lines}${token}
"
    done
  done

  unique_tokens="$(printf '%s' "$token_lines" | awk 'NF { print $0 }' | sort -u)"
  count=0
  while IFS= read -r token; do
    [[ -z "$token" ]] && continue
    if printf '%s\n' "$file_catalog" | grep -qi "$token"; then
      echo "$token"
      count=$((count + 1))
    fi
    [[ "$count" -ge "$MAX_RELATED_FILTERS" ]] && break
  done <<EOF
$unique_tokens
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick)
      QUICK_MODE=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      while [[ $# -gt 0 ]]; do
        CHANGED_FILES+=("$1")
        shift
      done
      ;;
    -*)
      echo "Unknown option: $1"
      echo ""
      usage
      exit 2
      ;;
    *)
      CHANGED_FILES+=("$1")
      shift
      ;;
  esac
done

for unsafe_setting in \
  SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER \
  SCRIPT_KIT_ALLOW_NATIVE_INPUT \
  SCRIPT_KIT_ALLOW_SCREEN_CAPTURE \
  SCRIPT_KIT_ALLOW_VISIBLE_PROBES \
  SCRIPT_KIT_ALLOW_LIVE_AI \
  SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH; do
  if [[ "${!unsafe_setting:-0}" == "1" ]]; then
    echo "[agent-check] REFUSED unsafe setting ${unsafe_setting}=1; agent verification must remain nonintrusive" >&2
    exit 78
  fi
done

export SCRIPT_KIT_NONINTERACTIVE=1
export SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER=0
export SCRIPT_KIT_ALLOW_NATIVE_INPUT=0
export SCRIPT_KIT_ALLOW_SCREEN_CAPTURE=0
export SCRIPT_KIT_ALLOW_VISIBLE_PROBES=0
export SCRIPT_KIT_ALLOW_LIVE_AI=0
export SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH=0
export SCRIPT_KIT_SEARCH_FULL_STRESS=0
export SCRIPT_KIT_STORAGE_FULL_STRESS=0

START_TIME="$(date +%s)"
CARGO_WRAPPER="./scripts/agentic/agent-cargo.sh"

echo "=== Agent Scoped Verification ==="
if [[ "$QUICK_MODE" == true ]]; then
  echo "Mode: quick (--quick)"
else
  echo "Mode: full"
fi

if [[ "${#CHANGED_FILES[@]}" -gt 0 ]]; then
  echo "Changed files (${#CHANGED_FILES[@]}):"
  for changed_file in "${CHANGED_FILES[@]}"; do
    echo "  - $changed_file"
  done
else
  echo "Changed files: none provided (running full default verification)"
fi

if ! run_step "agent-cargo check --locked --lib --bin script-kit-gpui" \
  "$CARGO_WRAPPER" check --locked --lib --bin script-kit-gpui; then
  TOTAL_TIME="$(( $(date +%s) - START_TIME ))"
  echo ""
  echo "=== RESULT: FAIL ($(format_duration "$TOTAL_TIME")) ==="
  exit 1
fi

if [[ "$QUICK_MODE" == true ]]; then
  TOTAL_TIME="$(( $(date +%s) - START_TIME ))"
  echo ""
  echo "=== RESULT: PASS ($(format_duration "$TOTAL_TIME")) ==="
  exit 0
fi

declare -a RELATED_FILTERS=()
RELATED_FILTERS_STR="$(derive_related_filters || true)"
while IFS= read -r filter; do
  [[ -z "$filter" ]] && continue
  RELATED_FILTERS+=("$filter")
done <<EOF
$RELATED_FILTERS_STR
EOF

if [[ "${#RELATED_FILTERS[@]}" -gt 0 ]]; then
  echo ""
  echo "Related test filters (fast feedback): ${RELATED_FILTERS[*]}"
  for filter in "${RELATED_FILTERS[@]}"; do
    if ! run_step "agent-cargo test --locked --lib ${filter}" \
      "$CARGO_WRAPPER" test --locked --lib "$filter"; then
      TOTAL_TIME="$(( $(date +%s) - START_TIME ))"
      echo ""
      echo "=== RESULT: FAIL ($(format_duration "$TOTAL_TIME")) ==="
      exit 1
    fi
  done
else
  echo ""
  echo "No related test filters discovered from changed files."
fi

if ! run_step "agent-cargo clippy --locked --lib --bin script-kit-gpui --no-deps -- -D warnings" \
  "$CARGO_WRAPPER" clippy --locked --lib --bin script-kit-gpui --no-deps -- -D warnings; then
  TOTAL_TIME="$(( $(date +%s) - START_TIME ))"
  echo ""
  echo "=== RESULT: FAIL ($(format_duration "$TOTAL_TIME")) ==="
  exit 1
fi

if ! run_step "agent-cargo test --locked --lib" "$CARGO_WRAPPER" test --locked --lib; then
  TOTAL_TIME="$(( $(date +%s) - START_TIME ))"
  echo ""
  echo "=== RESULT: FAIL ($(format_duration "$TOTAL_TIME")) ==="
  exit 1
fi

if ! run_step "agent-cargo test --locked -p sk-clipboard -p sk-protocol -p sk-storage" \
  "$CARGO_WRAPPER" test --locked -p sk-clipboard -p sk-protocol -p sk-storage; then
  TOTAL_TIME="$(( $(date +%s) - START_TIME ))"
  echo ""
  echo "=== RESULT: FAIL ($(format_duration "$TOTAL_TIME")) ==="
  exit 1
fi

TOTAL_TIME="$(( $(date +%s) - START_TIME ))"
echo ""
echo "=== RESULT: PASS ($(format_duration "$TOTAL_TIME")) ==="
