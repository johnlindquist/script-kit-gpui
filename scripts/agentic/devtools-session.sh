#!/usr/bin/env bash
# Canonical front door for isolated DevTools bootstrap (Oracle isolated-devtools-agent-bootstrap).
#
# Usage:
#   devtools-session.sh classify [--script PATH] [--mode auto|script-only|reuse-dev-watch|isolated] [--rust-changed auto|yes|no]
#   devtools-session.sh verify-script --script PATH [--timeout-sec 5]
#   devtools-session.sh start --session NAME [--mode auto|...] [--build auto|always|never] [--ready-timeout-sec 60] [--rpc-timeout-ms 10000] [--notes-sandbox] [--cleanup-on-fail] [--prove]
#   devtools-session.sh prove --session NAME [--rpc-timeout-ms 10000]
#   devtools-session.sh cleanup --session NAME --expected-pid PID --expected-generation GENERATION
#
# stdout: one final JSON envelope per subcommand
# stderr: progress NDJSON
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=devtools-session-lib.sh
source "${SCRIPT_DIR}/devtools-session-lib.sh"

cd "$DEVTOOLS_SESSION_REPO_ROOT"

SESSION=""
MODE="auto"
BUILD_POLICY="auto"
RUST_CHANGED="auto"
SCRIPT_PATH=""
READY_TIMEOUT_SEC=60
RPC_TIMEOUT_MS=10000
VERIFY_TIMEOUT_SEC=5
NOTES_SANDBOX=0
CLEANUP_ON_FAIL=0
DO_PROVE=0
EXPECTED_PID=""
EXPECTED_GENERATION=""
NOTES_FLAG=()

json_emit() {
  printf '%s\n' "$1"
}

json_error() {
  local phase="$1"
  local code="$2"
  local message="$3"
  local next="${4:-}"
  json_emit "$(printf '{"schemaVersion":1,"tool":"devtools-session","status":"error","phase":"%s","session":"%s","mode":"%s","error":{"code":"%s","message":"%s","next":"%s"}}\n' \
    "$phase" "$(json_escape "${SESSION:-}")" "$(json_escape "${MODE:-}")" \
    "$code" "$(json_escape "$message")" "$(json_escape "$next")")"
}

read_session_ownership() {
  local session_dir="$1"
  SESSION_OWNER_PID=""
  SESSION_OWNER_GENERATION=""
  if [[ -L "$session_dir" || -L "${session_dir}/pid" || -L "${session_dir}/generation" ]]; then
    return 1
  fi
  if [[ ! -f "${session_dir}/pid" || ! -f "${session_dir}/generation" ]]; then
    return 1
  fi
  SESSION_OWNER_PID="$(tr -d '\r\n' < "${session_dir}/pid")"
  SESSION_OWNER_GENERATION="$(tr -d '\r\n' < "${session_dir}/generation")"
  session_positive_integer "$SESSION_OWNER_PID" && session_name_valid "$SESSION_OWNER_GENERATION"
}

classify_mode() {
  local requested="$1"
  if [[ "$requested" != "auto" ]]; then
    printf '%s' "$requested"
    return
  fi

  if [[ -n "$SCRIPT_PATH" ]] && script_supports_sk_verify "$SCRIPT_PATH"; then
    if [[ "$RUST_CHANGED" == "no" ]] || { [[ "$RUST_CHANGED" == "auto" ]] && ! rust_changed_since_head; }; then
      if [[ "$BUILD_POLICY" == "never" ]]; then
        printf '%s' "script-only"
        return
      fi
    fi
  fi

  if detect_dev_sh && detect_dev_watch_healthy; then
    if [[ "$RUST_CHANGED" == "yes" ]] || { [[ "$RUST_CHANGED" == "auto" ]] && rust_changed_since_head; }; then
      printf '%s' "isolated"
      return
    fi
    printf '%s' "reuse-dev-watch"
    return
  fi

  printf '%s' "isolated"
}

should_skip_build() {
  local mode="$1"
  [[ "$BUILD_POLICY" == "never" ]] && return 0
  [[ "$mode" == "script-only" ]] && return 0
  [[ "$mode" == "reuse-dev-watch" ]] && return 0
  if [[ "$mode" == "isolated" && "$BUILD_POLICY" == "auto" && -z "${SCRIPT_KIT_GPUI_BINARY:-}" ]]; then
    return 1
  fi
  if [[ "$BUILD_POLICY" == "auto" ]] && ! rust_changed_since_head && [[ -x "$DEVTOOLS_SESSION_BINARY" ]]; then
    return 0
  fi
  return 1
}

cmd_classify() {
  progress classify "classifying bootstrap mode"
  MODE="$(classify_mode "$MODE")"
  local dev_sh=false
  local gpui_count
  detect_dev_sh && dev_sh=true
  gpui_count="$(gpui_instance_count)"
  local dev_watch_healthy=false
  detect_dev_watch_healthy && dev_watch_healthy=true
  local rust_changed=false
  if [[ "$RUST_CHANGED" == "yes" ]] || { [[ "$RUST_CHANGED" == "auto" ]] && rust_changed_since_head; }; then
    rust_changed=true
  fi
  local sk_verify=false
  [[ -n "$SCRIPT_PATH" ]] && script_supports_sk_verify "$SCRIPT_PATH" && sk_verify=true

  json_emit "$(printf '{"schemaVersion":1,"tool":"devtools-session","status":"ok","phase":"classify","mode":"%s","recommendedMode":"%s","signals":{"devSh":%s,"gpuiInstances":%s,"devWatchHealthy":%s,"rustChanged":%s,"skVerify":%s},"buildPolicy":"%s"}\n' \
    "$MODE" "$MODE" "$dev_sh" "$gpui_count" "$dev_watch_healthy" "$rust_changed" "$sk_verify" "$BUILD_POLICY")"
}

cmd_verify_script() {
  progress verify-script "running SK_VERIFY script proof"
  if [[ -z "$SCRIPT_PATH" ]]; then
    json_error verify-script script_verify_missing "verify-script requires --script PATH" "Pass --script kit-init/examples/scripts/todoist-demo.ts"
    exit 21
  fi
  if [[ ! -f "$SCRIPT_PATH" ]]; then
    json_error verify-script script_verify_missing "script not found: ${SCRIPT_PATH}" "Check the path relative to repo root."
    exit 21
  fi

  local output=""
  local verify_status=0
  set +e
  output="$(timeout "$VERIFY_TIMEOUT_SEC" env SK_VERIFY=1 bun "$SCRIPT_PATH" 2>&1)"
  verify_status=$?
  set -e

  if [[ "$verify_status" -ne 0 ]] || printf '%s' "$output" | grep -qiE 'error:|Cannot find module'; then
    json_error verify-script script_verify_failed "SK_VERIFY script failed (exit ${verify_status})" "${output:0:200}"
    exit 20
  fi

  json_emit "$(printf '{"schemaVersion":1,"tool":"devtools-session","status":"ok","phase":"verify-script","mode":"script-only","proof":{"script":"%s","output":"%s"}}\n' \
    "$(json_escape "$SCRIPT_PATH")" "$(json_escape "$output")")"
}

cmd_cleanup() {
  if [[ -z "$SESSION" ]]; then
    json_error cleanup usage_error "cleanup requires --session NAME" ""
    exit 2
  fi
  if session_is_borrowed "$SESSION"; then
    json_error cleanup borrowed_session_protected "The existing operator session is borrowed and must never be stopped by DevTools cleanup." "Detach without stopping dev-watch."
    exit 64
  fi
  if ! session_positive_integer "$EXPECTED_PID" || ! session_name_valid "$EXPECTED_GENERATION"; then
    json_error cleanup session_ownership_required "Cleanup requires the exact PID and generation from the isolated-start receipt." "Pass --expected-pid PID --expected-generation GENERATION."
    exit 64
  fi
  progress cleanup "stopping exactly owned session"
  local stop_cmd="bash scripts/agentic/session.sh stop ${SESSION} --expected-pid ${EXPECTED_PID} --expected-generation ${EXPECTED_GENERATION}"
  if ! bash scripts/agentic/session.sh stop "$SESSION" --expected-pid "$EXPECTED_PID" --expected-generation "$EXPECTED_GENERATION" >/dev/null 2>&1; then
    json_error cleanup cleanup_failed "session.sh stop failed for ${SESSION}" "Run: ${stop_cmd}"
    exit 60
  fi
  json_emit "$(printf '{"schemaVersion":1,"tool":"devtools-session","status":"ok","phase":"cleanup","session":"%s","ownership":{"pid":%s,"generation":"%s"},"cleanup":{"createdSession":true,"command":"%s"}}\n' \
    "$(json_escape "$SESSION")" "$EXPECTED_PID" "$(json_escape "$EXPECTED_GENERATION")" "$(json_escape "$stop_cmd")")"
}

cmd_prove() {
  progress prove "getState RPC proof"
  if [[ -z "$SESSION" ]]; then
    json_error prove usage_error "prove requires --session NAME" ""
    exit 2
  fi
  local rpc_json='{"type":"getState","requestId":"devtools-session-prove","summaryOnly":true}'
  local result
  if ! result="$(bash scripts/agentic/session.sh rpc "$SESSION" "$rpc_json" --expect stateResult --timeout "$RPC_TIMEOUT_MS" 2>/dev/null)"; then
    json_error prove rpc_timeout "getState RPC failed or timed out after ${RPC_TIMEOUT_MS}ms" "Check session status and app.log."
    exit 50
  fi
  if ! printf '%s' "$result" | grep -q '"responseType":"stateResult"'; then
    json_error prove rpc_parse_error "getState did not return stateResult" "$(json_escape "$result" | head -c 200)"
    exit 51
  fi
  json_emit "$(printf '{"schemaVersion":1,"tool":"devtools-session","status":"ok","phase":"prove","session":"%s","proof":{"getState":"ok","responseType":"stateResult"}}\n' \
    "$(json_escape "$SESSION")")"
}

cmd_start() {
  if [[ -z "$SESSION" ]]; then
    json_error start usage_error "start requires --session NAME" ""
    exit 2
  fi
  progress start "resolving mode"
  MODE="$(classify_mode "$MODE")"

  if [[ "$MODE" != "reuse-dev-watch" ]] && session_is_borrowed "$SESSION"; then
    json_error start borrowed_session_protected "The existing operator session cannot be claimed as an isolated DevTools session." "Use --mode reuse-dev-watch to attach without cleanup authority."
    exit 64
  fi

  if [[ "$MODE" == "script-only" ]]; then
    json_error start usage_error "start cannot run in script-only mode" "Use verify-script or classify with --mode script-only."
    exit 2
  fi

  export SCRIPT_KIT_SESSION_DIR="${SCRIPT_KIT_SESSION_DIR:-/tmp/sk-agentic-sessions}"
  local sdir
  sdir="$(session_sdir "$SESSION")"
  local session_preexisting=false
  if [[ -e "$sdir" || -L "$sdir" ]]; then
    session_preexisting=true
  fi
  local app_log="${sdir}/app.log"
  local bus="${sdir}/protocol-responses.ndjson"
  local build_log="/tmp/sk-isolated-build-${SCRIPT_KIT_AGENT_ID:-dt-agent-build}.log"
  local build_json=""
  local binary_path="$DEVTOOLS_SESSION_BINARY"

  progress preflight "preflight mode=${MODE}"
  local preflight_args=(--mode "$MODE")
  [[ "$MODE" == "reuse-dev-watch" ]] && preflight_args+=(--allow-dev-sh)
  if [[ "$MODE" == "isolated" ]] && ! should_skip_build "$MODE"; then
    preflight_args+=(--skip-binary)
  fi
  set +e
  bash "${SCRIPT_DIR}/preflight-isolated.sh" "${preflight_args[@]}"
  preflight_status=$?
  set -e
  if [[ "$preflight_status" -ne 0 ]]; then
    case "$preflight_status" in
      11) json_error preflight dev_sh_running "./dev.sh is running; isolated mode must not start a second GPUI instance." "Use --mode reuse-dev-watch or stop ./dev.sh."; exit 11 ;;
      12) json_error preflight multiple_gpui_instances "Multiple script-kit-gpui instances detected." "pkill orphans, then retry."; exit 12 ;;
      13) json_error preflight binary_missing "DevTools binary is missing: ${DEVTOOLS_SESSION_BINARY}" "Run build-isolated-binary.sh or --build always."; exit 13 ;;
      10)
        if [[ "$MODE" == "reuse-dev-watch" ]]; then
          json_error preflight dev_watch_unhealthy "dev-watch session is not healthy" "Ensure ./dev.sh is running and dev-watch started."
          exit 10
        fi
        json_error preflight preflight_failed "preflight failed (exit ${preflight_status})" "See stderr progress."
        exit 10
        ;;
      *) json_error preflight preflight_failed "preflight failed (exit ${preflight_status})" "See stderr progress."; exit 10 ;;
    esac
  fi

  if [[ "$MODE" == "reuse-dev-watch" ]]; then
    progress reuse-dev-watch "attaching to dev-watch session"
    local status_json
    status_json="$(bash scripts/agentic/session.sh status dev-watch 2>/dev/null || true)"
    if ! printf '%s' "$status_json" | grep -q '"healthy":true\|"alive":true'; then
      json_error start dev_watch_unhealthy "dev-watch session is not healthy" "Ensure ./dev.sh is running and dev-watch started."
      exit 10
    fi
    SESSION="dev-watch"
    local proof_get_state="skipped"
    local proof_response_type=""
    if [[ "$DO_PROVE" -eq 1 ]]; then
      progress prove "getState RPC on dev-watch"
      local rpc_json='{"type":"getState","requestId":"devtools-session-prove","summaryOnly":true}'
      local result
      if result="$(bash scripts/agentic/session.sh rpc dev-watch "$rpc_json" --expect stateResult --timeout "$RPC_TIMEOUT_MS" 2>/dev/null)"; then
        if printf '%s' "$result" | grep -q '"responseType":"stateResult"'; then
          proof_get_state="ok"
          proof_response_type="stateResult"
        else
          proof_get_state="parse_error"
        fi
      else
        proof_get_state="timeout"
      fi
    fi
    json_emit "$(printf '{"schemaVersion":1,"tool":"devtools-session","status":"ok","phase":"complete","mode":"reuse-dev-watch","session":"dev-watch","ready":true,"readyMarker":"existing_session","timeouts":{"verifySec":%s,"buildSec":120,"readySec":%s,"rpcMs":%s},"paths":{"appLog":"%s","protocolResponses":"%s","buildLog":"%s"},"proof":{"getState":"%s","responseType":"%s"},"ownership":{"created":false},"cleanup":{"createdSession":false,"command":null}}\n' \
      "$VERIFY_TIMEOUT_SEC" "$READY_TIMEOUT_SEC" "$RPC_TIMEOUT_MS" \
      "$(json_escape "$(session_sdir dev-watch)/app.log")" \
      "$(json_escape "$(session_sdir dev-watch)/protocol-responses.ndjson")" \
      "$(json_escape "$build_log")" \
      "$proof_get_state" "$proof_response_type")"
    return 0
  fi

  if [[ "$NOTES_SANDBOX" -eq 1 ]]; then
    export SCRIPT_KIT_TEST_NOTES_DB_PATH="${SCRIPT_KIT_TEST_NOTES_DB_PATH:-/tmp/sk-notes-${SESSION}.db}"
    NOTES_FLAG=(--notes-sandbox)
  fi

  if ! should_skip_build "$MODE"; then
    if [[ "$BUILD_POLICY" == "always" ]] || [[ "$BUILD_POLICY" == "auto" ]]; then
      progress build "publishing isolated artifact under the shared build lease (timeout 120s)"
      set +e
      build_json="$(
        DEVTOOLS_SESSION_JSON=1 \
        SCRIPT_KIT_DEVTOOLS_SESSION="$SESSION" \
        SCRIPT_KIT_CARGO_TARGET_POOL="${SCRIPT_KIT_CARGO_TARGET_POOL:-agent-debug}" \
        bash "${SCRIPT_DIR}/build-isolated-binary.sh" --json 120
      )"
      build_status=$?
      set -e
      if [[ "$build_status" -ne 0 ]]; then
        case "$build_status" in
          30) json_error build build_timeout "cargo build exceeded 120s" "Retry with warm target-agent cache."; exit 30 ;;
          31) json_error build build_failed "cargo build failed" "See build log."; exit 31 ;;
          32) json_error build stage_failed "stage failed after build" "Check target-agent output."; exit 32 ;;
          *) json_error build build_failed "build failed (exit ${build_status})" ""; exit 31 ;;
        esac
      fi
      local reference_path
      reference_path="$(BUILD_JSON="$build_json" python3 - <<'PY'
import json, os, tempfile
data = json.loads(os.environ['BUILD_JSON'])
reference = data.get('artifact')
if not isinstance(reference, dict) or not reference.get('manifestSha256'):
    raise SystemExit('build did not return a finalized immutable artifact reference')
fd, path = tempfile.mkstemp(prefix='sk-isolated-reference-', suffix='.json', dir=os.path.realpath(tempfile.gettempdir()))
with os.fdopen(fd, 'w') as output:
    json.dump(reference, output)
print(path)
PY
      )"
      # Only this invocation's freshly created reference is removed on exit.
      trap "rm -f -- $(printf '%q' "$reference_path")" EXIT
      export SCRIPT_KIT_ARTIFACT_REFERENCE="$reference_path"
      binary_path="$(bun "${SCRIPT_DIR}/build-artifact.ts" verify-reference "$DEVTOOLS_SESSION_REPO_ROOT" "$reference_path")"
      if [[ "$binary_path" != /* ]]; then
        binary_path="${DEVTOOLS_SESSION_REPO_ROOT}/${binary_path}"
      fi
      export SCRIPT_KIT_GPUI_BINARY="$binary_path"
      DEVTOOLS_SESSION_BINARY="$binary_path"
      progress preflight "preflight staged binary"
      if bash "${SCRIPT_DIR}/preflight-isolated.sh" --mode isolated; then
        :
      else
        preflight_status=$?
        case "$preflight_status" in
          11) json_error preflight dev_sh_running "./dev.sh is running; isolated mode must not start a second GPUI instance." "Use --mode reuse-dev-watch or stop ./dev.sh."; exit 11 ;;
          12) json_error preflight multiple_gpui_instances "Multiple script-kit-gpui instances detected." "pkill orphans, then retry."; exit 12 ;;
          13) json_error preflight binary_missing "Staged DevTools binary is missing: ${DEVTOOLS_SESSION_BINARY}" "Re-run build-isolated-binary.sh."; exit 13 ;;
          *) json_error preflight preflight_failed "preflight failed after staging (exit ${preflight_status})" "See stderr progress."; exit 10 ;;
        esac
      fi
    fi
  else
    progress build "skipping compilation by explicit policy; launch checks any supplied artifact reference"
  fi

  progress start "starting isolated session (internal ready timeout 5s)"
  local start_args=("$SESSION")
  [[ "${#NOTES_FLAG[@]}" -gt 0 ]] && start_args+=("${NOTES_FLAG[@]}")
  start_args+=(--wait-sec "$READY_TIMEOUT_SEC")
  if bash "${SCRIPT_DIR}/start-isolated.sh" "${start_args[@]}" >&2; then
    :
  else
    start_status=$?
    if [[ "$CLEANUP_ON_FAIL" -eq 1 && "$session_preexisting" == false ]] \
      && read_session_ownership "$sdir"; then
      bash scripts/agentic/session.sh stop "$SESSION" \
        --expected-pid "$SESSION_OWNER_PID" \
        --expected-generation "$SESSION_OWNER_GENERATION" >/dev/null 2>&1 || true
    fi
    case "$start_status" in
      41) json_error wait-ready ready_timeout "Session did not reach STARTUP_READY/APP_READY/stateResult before timeout." "Stop stale sessions and retry."; exit 41 ;;
      42) json_error wait-ready app_log_empty "app.log stayed empty while process alive." "Check for multiple GPUI instances."; exit 42 ;;
      11|12|13) exit "$start_status" ;;
      *) json_error start start_failed "start-isolated failed (exit ${start_status})" "Check ${app_log}."; exit 40 ;;
    esac
  fi

  local pid=""
  local status_json
  status_json="$(bash scripts/agentic/session.sh status "$SESSION" 2>/dev/null || true)"
  pid="$(printf '%s' "$status_json" | sed -nE 's/.*"pid":([0-9]+).*/\1/p' | head -1)"
  if ! read_session_ownership "$sdir" || [[ "$pid" != "$SESSION_OWNER_PID" ]]; then
    json_error start session_ownership_missing "The ready session does not expose one matching, safe PID and generation." "Inspect the isolated-session registry before attempting cleanup."
    exit 64
  fi

  local proof_get_state="skipped"
  local proof_response_type=""
  if [[ "$DO_PROVE" -eq 1 ]]; then
    progress prove "getState RPC"
    local rpc_json='{"type":"getState","requestId":"devtools-session-prove","summaryOnly":true}'
    local result
    if result="$(bash scripts/agentic/session.sh rpc "$SESSION" "$rpc_json" --expect stateResult --timeout "$RPC_TIMEOUT_MS" 2>/dev/null)"; then
      if printf '%s' "$result" | grep -q '"responseType":"stateResult"'; then
        proof_get_state="ok"
        proof_response_type="stateResult"
      else
        proof_get_state="parse_error"
      fi
    else
      proof_get_state="timeout"
    fi
  fi

  local cleanup_json
  local created_session=true
  if [[ "$session_preexisting" == true ]]; then
    created_session=false
    cleanup_json='{"createdSession":false,"command":null}'
  else
    local cleanup_cmd="bash scripts/agentic/session.sh stop ${SESSION} --expected-pid ${SESSION_OWNER_PID} --expected-generation ${SESSION_OWNER_GENERATION}"
    cleanup_json="$(printf '{"createdSession":true,"command":"%s"}' "$(json_escape "$cleanup_cmd")")"
  fi
  json_emit "$(printf '{"schemaVersion":1,"tool":"devtools-session","status":"ok","phase":"complete","mode":"isolated","session":"%s","sessionDir":"%s","pid":%s,"ready":true,"readyMarker":"startup_ready","timeouts":{"verifySec":%s,"buildSec":120,"readySec":%s,"rpcMs":%s},"paths":{"appLog":"%s","protocolResponses":"%s","buildLog":"%s","binary":"%s"},"proof":{"getState":"%s","responseType":"%s"},"ownership":{"created":%s,"pid":%s,"generation":"%s"},"cleanup":%s}\n' \
    "$(json_escape "$SESSION")" "$(json_escape "$sdir")" "${pid:-0}" \
    "$VERIFY_TIMEOUT_SEC" "$READY_TIMEOUT_SEC" "$RPC_TIMEOUT_MS" \
    "$(json_escape "$app_log")" "$(json_escape "$bus")" "$(json_escape "$build_log")" "$(json_escape "$DEVTOOLS_SESSION_BINARY")" \
    "$proof_get_state" "$proof_response_type" "$created_session" \
    "$SESSION_OWNER_PID" "$(json_escape "$SESSION_OWNER_GENERATION")" "$cleanup_json")"
}

usage() {
  cat >&2 <<'EOF'
Usage:
  devtools-session.sh classify [--script PATH] [--mode MODE] [--rust-changed auto|yes|no] [--build auto|always|never]
  devtools-session.sh verify-script --script PATH [--timeout-sec 5]
  devtools-session.sh start --session NAME [options]
  devtools-session.sh prove --session NAME [--rpc-timeout-ms 10000]
  devtools-session.sh cleanup --session NAME --expected-pid PID --expected-generation GENERATION
EOF
  exit 2
}

SUBCMD="${1:-}"
shift || true

while [[ $# -gt 0 ]]; do
  case "$1" in
    --session) SESSION="${2:-}"; shift 2 ;;
    --mode) MODE="${2:-auto}"; shift 2 ;;
    --build) BUILD_POLICY="${2:-auto}"; shift 2 ;;
    --rust-changed) RUST_CHANGED="${2:-auto}"; shift 2 ;;
    --script) SCRIPT_PATH="${2:-}"; shift 2 ;;
    --ready-timeout-sec) READY_TIMEOUT_SEC="${2:-60}"; shift 2 ;;
    --rpc-timeout-ms) RPC_TIMEOUT_MS="${2:-10000}"; shift 2 ;;
    --timeout-sec) VERIFY_TIMEOUT_SEC="${2:-5}"; shift 2 ;;
    --expected-pid) EXPECTED_PID="${2:-}"; shift 2 ;;
    --expected-generation) EXPECTED_GENERATION="${2:-}"; shift 2 ;;
    --notes-sandbox) NOTES_SANDBOX=1; shift ;;
    --cleanup-on-fail) CLEANUP_ON_FAIL=1; shift ;;
    --prove) DO_PROVE=1; shift ;;
    -h|--help) usage ;;
    *)
      if [[ -z "$SCRIPT_PATH" && -f "$1" ]]; then
        SCRIPT_PATH="$1"
        shift
      else
        echo "unknown arg: $1" >&2
        usage
      fi
      ;;
  esac
done

if [[ -n "$SESSION" ]] && ! session_name_valid "$SESSION"; then
  json_error "${SUBCMD:-start}" invalid_session_name "Session identity must be one safe registry child." "Use only letters, digits, dots, underscores, and hyphens; start with a letter or digit."
  exit 64
fi
case "$MODE" in
  auto|script-only|reuse-dev-watch|isolated) ;;
  *)
    json_error "${SUBCMD:-start}" invalid_mode "Unrecognized DevTools session mode: ${MODE}" "Use auto, script-only, reuse-dev-watch, or isolated."
    exit 64
    ;;
esac
case "$BUILD_POLICY" in
  auto|always|never) ;;
  *)
    json_error "${SUBCMD:-start}" invalid_build_policy "Unrecognized DevTools build policy: ${BUILD_POLICY}" "Use auto, always, or never."
    exit 64
    ;;
esac
case "$RUST_CHANGED" in
  auto|yes|no) ;;
  *)
    json_error "${SUBCMD:-start}" invalid_rust_change_policy "Unrecognized Rust-change policy: ${RUST_CHANGED}" "Use auto, yes, or no."
    exit 64
    ;;
esac
for timeout_value in "$READY_TIMEOUT_SEC" "$RPC_TIMEOUT_MS" "$VERIFY_TIMEOUT_SEC"; do
  if ! session_positive_integer "$timeout_value"; then
    json_error "${SUBCMD:-start}" invalid_timeout "Session timeouts must be positive whole numbers." "Provide a nonzero integer number of seconds or milliseconds."
    exit 64
  fi
done

case "$SUBCMD" in
  classify) cmd_classify ;;
  verify-script) cmd_verify_script ;;
  start) cmd_start ;;
  prove) cmd_prove ;;
  cleanup) cmd_cleanup ;;
  "") usage ;;
  *) echo "unknown subcommand: $SUBCMD" >&2; usage ;;
esac
