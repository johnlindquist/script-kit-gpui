#!/usr/bin/env bash
# scripts/agentic/session.sh — Reusable named-pipe session management for Script Kit GPUI.
#
# Usage:
#   session.sh start  [SESSION_NAME]   — Create or resume a session (default: "default")
#   session.sh send   SESSION_NAME CMD [--await-parse [--timeout MS]]
#                                      — Send a JSON command. Default: fire-and-forget
#                                        (returns `sent:true` without waiting). With
#                                        `--await-parse`, tails app.log for the next
#                                        `stdin_command_parsed` or `stdin_parse_failed`
#                                        event emitted after the send offset and
#                                        returns `parseOutcome:"parsed"` +
#                                        `commandType:<kind>` on success, or
#                                        `parseOutcome:"parseError"` + `error:<msg>`
#                                        on failure (closes the "silent-drop on
#                                        unknown variant" gap Pass #8 Run 4 probe
#                                        exposed — see tool-session-send-parse-receipt).
#   session.sh rpc    SESSION_NAME CMD [--expect TYPE] [--timeout MS]
#                                      — Send a JSON command and await the response
#   session.sh stop   [SESSION_NAME] [--expected-pid PID --expected-generation GENERATION]
#                                      — Stop a session and clean up. When both
#                                        ownership options are supplied, fail
#                                        closed unless the registry still has
#                                        that exact PID and generation.
#   session.sh status [SESSION_NAME]   — Print session state as JSON
#
# All output on stdout is machine-readable JSON. Diagnostics go to stderr.
# Sessions survive across shells — no fd 3 trick required.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# Direct shell callers must cross the same fail-closed boundary as Driver and
# DevTools clients. Validate before creating the session registry, resolving a
# binary, touching an existing FIFO, or inspecting a user-owned process.
if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" ]]; then
  SCRIPT_KIT_SESSION_GUARD_ROOT="$PROJECT_ROOT" bun -e '
    const root = process.env.SCRIPT_KIT_SESSION_GUARD_ROOT;
    process.chdir(root);
    const { assertNoninteractiveSessionCommand } = await import(
      `${root}/scripts/devtools/lib/operator-safety.ts`
    );
    try {
      assertNoninteractiveSessionCommand([
        "bash",
        `${root}/scripts/agentic/session.sh`,
        ...process.argv.slice(1),
      ]);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      process.stdout.write(`${JSON.stringify({
        schemaVersion: 1,
        status: "error",
        error: { code: "noninteractive_safety_refused", message },
      })}\n`);
      process.stderr.write(`${message}\n`);
      process.exit(78);
    }
  ' "$@"
fi

SCHEMA_VERSION=1
SESSION_DIR_RAW="${SCRIPT_KIT_SESSION_DIR:-/tmp/sk-agentic-sessions}"
SESSION_SUBCOMMAND="${1:-}"
canonical_session_dir() {
  local dir="$1"
  if [ ! -d "$dir" ]; then
    if [ "$SESSION_SUBCOMMAND" != "start" ]; then
      printf '%s\n' "$dir"
      return 0
    fi
    mkdir -p "$dir"
  fi
  (cd "$dir" && pwd -P)
}
SESSION_DIR="$(canonical_session_dir "$SESSION_DIR_RAW")"
# With no explicit override, pick the freshest of the dev.sh binary and the
# agent-cargo pool binary so a just-built agent binary is never silently
# shadowed by a stale target/debug one (and vice versa).
resolve_default_binary() {
  local dev_bin="${PROJECT_ROOT}/target/debug/script-kit-gpui"
  local agent_bin="${PROJECT_ROOT}/target-agent/pools/agent-debug/debug/script-kit-gpui"
  if [ ! -x "$agent_bin" ]; then echo "$dev_bin"; return; fi
  if [ ! -x "$dev_bin" ]; then echo "$agent_bin"; return; fi
  if [ "$agent_bin" -nt "$dev_bin" ]; then
    echo "[session.sh] binary: $agent_bin (fresher than target/debug; set SCRIPT_KIT_GPUI_BINARY to override)" >&2
    echo "$agent_bin"
  else
    echo "[session.sh] binary: $dev_bin (fresher than agent pool; set SCRIPT_KIT_GPUI_BINARY to override)" >&2
    echo "$dev_bin"
  fi
}
if [ "$SESSION_SUBCOMMAND" = "start" ]; then
  BINARY="${SCRIPT_KIT_GPUI_BINARY:-$(resolve_default_binary)}"
else
  BINARY="${SCRIPT_KIT_GPUI_BINARY:-}"
fi
READY_TIMEOUT_MS="${SCRIPT_KIT_SESSION_READY_TIMEOUT_MS:-3000}"
STOP_GRACE_MS="${SCRIPT_KIT_SESSION_STOP_GRACE_MS:-2000}"
STOP_KILL_TIMEOUT_MS="${SCRIPT_KIT_SESSION_STOP_KILL_TIMEOUT_MS:-2000}"
READY_LOG_MARKER_APP="APP_READY|main-window-ready show=false focus=false stdin-safe"
READY_LOG_MARKER_STARTUP="STARTUP_READY "
READY_WAIT_MS_RESULT=0
READY_MARKER_RESULT="none"

# --- helpers ----------------------------------------------------------------

log() { echo "[session.sh] $*" >&2; }

json_envelope() {
  local status="$1"; shift
  # Remaining args are key:value pairs injected into the envelope
  local extra=""
  while [ $# -gt 0 ]; do
    extra="${extra},\"${1%%:*}\":${1#*:}"
    shift
  done
  printf '{"schemaVersion":%d,"status":"%s"%s}\n' "$SCHEMA_VERSION" "$status" "$extra"
}

json_error() {
  local code="$1" msg="$2"
  printf '{"schemaVersion":%d,"status":"error","error":{"code":"%s","message":"%s"}}\n' \
    "$SCHEMA_VERSION" "$code" "$msg"
}

STOP_OWNERSHIP_STRICT=false
STOP_EXPECTED_PID=""
STOP_EXPECTED_GENERATION=""
STOP_ACTUAL_PID=""
STOP_ACTUAL_GENERATION=""

read_stop_identity() {
  local sdir="$1"
  STOP_ACTUAL_PID=""
  STOP_ACTUAL_GENERATION=""
  if [ -f "${sdir}/pid" ]; then
    STOP_ACTUAL_PID="$(tr -d '\r\n' < "${sdir}/pid")"
  fi
  if [ -f "${sdir}/generation" ]; then
    STOP_ACTUAL_GENERATION="$(tr -d '\r\n' < "${sdir}/generation")"
  fi
}

stop_pid_json_value() {
  local value="$1"
  if [ -z "$value" ]; then
    printf 'null'
  elif [[ "$value" =~ ^[0-9]+$ ]]; then
    printf '%s' "$value"
  else
    printf '"%s"' "$(json_escape "$value")"
  fi
}

stop_generation_json_value() {
  local value="$1"
  if [ -z "$value" ]; then
    printf 'null'
  else
    printf '"%s"' "$(json_escape "$value")"
  fi
}

verify_stop_ownership() {
  local name="$1"
  local sdir="$2"
  if [ "$STOP_OWNERSHIP_STRICT" != true ]; then
    return 0
  fi

  read_stop_identity "$sdir"
  if [ "$STOP_ACTUAL_PID" = "$STOP_EXPECTED_PID" ] \
     && [ "$STOP_ACTUAL_GENERATION" = "$STOP_EXPECTED_GENERATION" ]; then
    return 0
  fi

  local expected_generation actual_pid actual_generation
  expected_generation="$(json_escape "$STOP_EXPECTED_GENERATION")"
  actual_pid="$(stop_pid_json_value "$STOP_ACTUAL_PID")"
  actual_generation="$(stop_generation_json_value "$STOP_ACTUAL_GENERATION")"
  printf '{"schemaVersion":%d,"status":"error","session":"%s","ownershipVerified":false,"expectedPid":%s,"actualPid":%s,"expectedGeneration":"%s","actualGeneration":%s,"error":{"code":"session_ownership_mismatch","message":"Session ownership changed or is missing; refusing to stop or mutate the session."}}\n' \
    "$SCHEMA_VERSION" "$(json_escape "$name")" "$STOP_EXPECTED_PID" "$actual_pid" \
    "$expected_generation" "$actual_generation"
  return 1
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

append_lifecycle_event() {
  local name="$1" event="$2" code="$3" message="$4"
  local sdir lifecycle_path keep_actions_window_open escaped_message
  sdir="$(session_dir "$name")"
  lifecycle_path="${sdir}/lifecycle.ndjson"
  keep_actions_window_open="false"
  if [ -f "${sdir}/keep_actions_window_open" ]; then
    keep_actions_window_open="$(cat "${sdir}/keep_actions_window_open")"
  fi
  escaped_message="$(json_escape "$message")"
  mkdir -p "$sdir"
  printf '{"schemaVersion":%d,"event":"%s","code":"%s","message":"%s","keepActionsWindowOpen":%s,"timestamp":"%s"}\n' \
    "$SCHEMA_VERSION" "$event" "$code" "$escaped_message" "$keep_actions_window_open" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    >> "$lifecycle_path" 2>/dev/null || true
}

last_lifecycle_event_json() {
  local name="$1"
  local lifecycle_path
  lifecycle_path="$(session_dir "$name")/lifecycle.ndjson"
  if [ -f "$lifecycle_path" ]; then
    tail -n 1 "$lifecycle_path" 2>/dev/null || true
  fi
}

json_lifecycle_error() {
  local name="$1" code="$2" msg="$3"
  local lifecycle_path keep_actions_window_open escaped_msg last_event
  lifecycle_path="$(session_dir "$name")/lifecycle.ndjson"
  keep_actions_window_open="false"
  if [ -f "$(session_dir "$name")/keep_actions_window_open" ]; then
    keep_actions_window_open="$(cat "$(session_dir "$name")/keep_actions_window_open")"
  fi
  append_lifecycle_event "$name" "session_lifecycle_error" "$code" "$msg"
  escaped_msg="$(json_escape "$msg")"
  last_event="$(last_lifecycle_event_json "$name")"
  if [ -z "$last_event" ]; then
    last_event="null"
  fi
  printf '{"schemaVersion":%d,"status":"error","session":"%s","keepActionsWindowOpen":%s,"lifecycle":"%s","sessionLifecycle":%s,"error":{"code":"%s","message":"%s"}}\n' \
    "$SCHEMA_VERSION" "$name" "$keep_actions_window_open" "$lifecycle_path" "$last_event" "$code" "$escaped_msg"
}

session_dir() { echo "${SESSION_DIR}/$1"; }

session_now_ms() {
  if [ -n "${EPOCHREALTIME:-}" ]; then
    printf '%s' "$EPOCHREALTIME" | awk -F. '{printf "%d", $1*1000 + int(substr($2"000000",1,3))}'
  else
    echo $(( $(date +%s) * 1000 ))
  fi
}

session_lock_dir() {
  echo "$(session_dir "$1")/command.lock"
}

acquire_session_lock() {
  local sdir="$1"
  local timeout_ms="${2:-5000}"
  local lock_dir="${sdir}/command.lock"
  local waited=0
  local step_ms=25

  mkdir -p "$sdir"
  while [ "$waited" -lt "$timeout_ms" ]; do
    if mkdir "$lock_dir" 2>/dev/null; then
      return 0
    fi
    sleep 0.025
    waited=$((waited + step_ms))
  done
  return 1
}

release_session_lock() {
  local sdir="$1"
  rmdir "${sdir}/command.lock" 2>/dev/null || true
}

ensure_session_protocol_bus() {
  local sdir="$1"
  local protocol_responses_path="${sdir}/protocol-responses.ndjson"
  local generation_path="${sdir}/generation"
  mkdir -p "$sdir"
  if [ ! -f "$protocol_responses_path" ]; then
    : > "$protocol_responses_path"
  fi
  if [ ! -f "$generation_path" ]; then
    if command -v uuidgen >/dev/null 2>&1; then
      uuidgen > "$generation_path"
    else
      printf '%s-%s\n' "$(date +%s)" "$$" > "$generation_path"
    fi
  fi
}

# Detect which readiness marker is present in the log file.
# Prints the marker name ("startup_ready" or "app_ready") and returns 0 if found.
detect_ready_marker() {
  local log_path="$1"
  if [ -f "$log_path" ] && grep -Fq "$READY_LOG_MARKER_STARTUP" "$log_path" 2>/dev/null; then
    printf '%s' "startup_ready"
    return 0
  fi
  if [ -f "$log_path" ] && grep -Fq "$READY_LOG_MARKER_APP" "$log_path" 2>/dev/null; then
    printf '%s' "app_ready"
    return 0
  fi
  return 1
}

# Wait for the earliest safe readiness log marker instead of a fixed sleep.
# Sets:
#   READY_WAIT_MS_RESULT  - number of milliseconds waited
#   READY_MARKER_RESULT   - startup_ready | app_ready | none
# Exit codes: 0 = marker found, 1 = timeout, 2 = process exited early.
wait_for_ready_log() {
  local log_path="$1"
  local pid="$2"
  local timeout_ms="${3:-$READY_TIMEOUT_MS}"
  local waited=0
  local step_ms=25

  READY_WAIT_MS_RESULT=0
  READY_MARKER_RESULT="none"

  while [ "$waited" -lt "$timeout_ms" ]; do
    if ! kill -0 "$pid" 2>/dev/null; then
      READY_WAIT_MS_RESULT="$waited"
      return 2
    fi
    local marker_name=""
    if marker_name="$(detect_ready_marker "$log_path")"; then
      READY_WAIT_MS_RESULT="$waited"
      READY_MARKER_RESULT="$marker_name"
      return 0
    fi
    sleep 0.025
    waited=$((waited + step_ms))
  done

  READY_WAIT_MS_RESULT="$waited"
  return 1
}

wait_for_process_exit() {
  local pid="$1"
  local timeout_ms="$2"
  local waited=0
  local step_ms=25

  while kill -0 "$pid" 2>/dev/null; do
    if [ "$waited" -ge "$timeout_ms" ]; then
      return 1
    fi
    sleep 0.025
    waited=$((waited + step_ms))
  done
  return 0
}

# Session apps are children of session-supervisor.py, not of the shell running
# `session.sh stop`. A shell `wait "$pid"` therefore returns immediately and
# cannot prove teardown. Keep the supervisor alive to reap the app, wait for a
# bounded graceful exit, then kill the app's dedicated process group before any
# session registry files are removed.
SESSION_STOP_FORCED_KILL=false
terminate_session_app() {
  local name="$1"
  local sdir="$2"
  local pid="$3"
  local supervisor_pid="${4:-}"

  SESSION_STOP_FORCED_KILL=false
  if ! kill -0 "$pid" 2>/dev/null; then
    return 0
  fi

  if [ -n "$supervisor_pid" ] && kill -0 "$supervisor_pid" 2>/dev/null; then
    # The supervisor forwards SIGTERM to the app's dedicated process group and
    # stays alive until it has reaped the child and written its exit receipt.
    if ! verify_stop_ownership "$name" "$sdir"; then
      return 1
    fi
    kill -TERM "$supervisor_pid" 2>/dev/null || true
  else
    if ! verify_stop_ownership "$name" "$sdir"; then
      return 1
    fi
    if ! kill -TERM -- "-$pid" 2>/dev/null; then
      if ! verify_stop_ownership "$name" "$sdir"; then
        return 1
      fi
      kill -TERM "$pid" 2>/dev/null || true
    fi
  fi

  if wait_for_process_exit "$pid" "$STOP_GRACE_MS"; then
    log "Stopped session '${name}' gracefully (pid ${pid})"
    return 0
  fi

  SESSION_STOP_FORCED_KILL=true
  log "Session '${name}' ignored SIGTERM; sending SIGKILL to process group ${pid}"
  if ! verify_stop_ownership "$name" "$sdir"; then
    return 1
  fi
  if ! kill -KILL -- "-$pid" 2>/dev/null; then
    if ! verify_stop_ownership "$name" "$sdir"; then
      return 1
    fi
    kill -KILL "$pid" 2>/dev/null || true
  fi
  if wait_for_process_exit "$pid" "$STOP_KILL_TIMEOUT_MS"; then
    log "Stopped session '${name}' with SIGKILL (pid ${pid})"
    return 0
  fi

  json_lifecycle_error "$name" "session_stop_failed" \
    "Session '${name}' app process (pid ${pid}) survived SIGTERM and SIGKILL; preserving ${sdir} for recovery."
  return 1
}

path_aliases() {
  local file_path="$1"
  printf '%s\n' "$file_path"
  if [ "$SESSION_DIR_RAW" != "$SESSION_DIR" ] && [[ "$file_path" == "$SESSION_DIR"* ]]; then
    printf '%s\n' "${SESSION_DIR_RAW}${file_path#"$SESSION_DIR"}"
  fi
}

regex_escape() {
  printf '%s' "$1" | sed 's/[][(){}.^$*+?|\\]/\\&/g'
}

session_forwarder_pids() {
  local pipe_path="$1"
  local input_fifo="$2"
  local candidate
  local pipe_pattern
  local input_pattern

  while IFS= read -r candidate; do
    pipe_pattern="$(regex_escape "$candidate")"
    pgrep -f "$pipe_pattern" 2>/dev/null || true
  done < <(path_aliases "$pipe_path")

  while IFS= read -r candidate; do
    input_pattern="$(regex_escape "$candidate")"
    pgrep -f "cat ${input_pattern}" 2>/dev/null || true
  done < <(path_aliases "$input_fifo")
}

is_descendant_of() {
  local pid="$1"
  local ancestor="${2:-}"

  if [ -z "$ancestor" ]; then
    return 1
  fi

  while [ -n "$pid" ] && [ "$pid" != "0" ] && [ "$pid" != "1" ]; do
    if [ "$pid" = "$ancestor" ]; then
      return 0
    fi
    pid="$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')"
  done

  return 1
}

cleanup_orphan_session_forwarders() {
  local pipe_path="$1"
  local input_fifo="$2"
  local keep_pid="${3:-}"
  local ownership_name="${4:-}"
  local ownership_sdir="${5:-}"

  while IFS= read -r orphan_pid; do
    if [ -z "$orphan_pid" ]; then
      continue
    fi
    if [ -n "$keep_pid" ] && is_descendant_of "$orphan_pid" "$keep_pid"; then
      continue
    fi
    if [ "$orphan_pid" = "$$" ]; then
      continue
    fi
    if [ -n "$ownership_name" ] && ! verify_stop_ownership "$ownership_name" "$ownership_sdir"; then
      return 1
    fi
    kill "$orphan_pid" 2>/dev/null || true
  done < <(session_forwarder_pids "$pipe_path" "$input_fifo")
}

send_startup_keepalive() {
  local input_fifo="$1"
  local app_pid="$2"
  timeout 2 bash -c 'printf "{\"type\":\"getState\",\"requestId\":\"session-start-%s\"}\n" "$2" > "$1"' _ "$input_fifo" "$app_pid" 2>/dev/null
}

# --- start ------------------------------------------------------------------

cmd_start() {
  local name="${1:-default}"
  local sdir
  sdir="$(session_dir "$name")"
  local input_fifo="${sdir}/input"
  local log_path="${sdir}/app.log"
  local responses_path="${sdir}/responses.ndjson"
  local protocol_responses_path="${sdir}/protocol-responses.ndjson"
  local generation_path="${sdir}/generation"
  local lifecycle_path="${sdir}/lifecycle.ndjson"
  local exit_receipt_path="${sdir}/app-exit.json"
  local keep_actions_window_open_requested=false
  if [ "${SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN:-}" = "1" ]; then
    keep_actions_window_open_requested=true
  fi

  # Resume only if ALL components are healthy: app PID, forwarder PID, input FIFO, primary pipe
  if [ -f "${sdir}/pid" ]; then
    local old_pid
    old_pid="$(cat "${sdir}/pid")"
    local old_fwd_pid=""
    if [ -f "${sdir}/fwd_pid" ]; then
      old_fwd_pid="$(cat "${sdir}/fwd_pid")"
    fi
    local old_supervisor_pid=""
    if [ -f "${sdir}/supervisor_pid" ]; then
      old_supervisor_pid="$(cat "${sdir}/supervisor_pid")"
    fi
    local primary_pipe="${sdir}/pipe"

    local can_resume=true
    local reason=""

    if ! kill -0 "$old_pid" 2>/dev/null; then
      can_resume=false
      reason="app process (pid ${old_pid}) dead"
    elif [ -z "$old_fwd_pid" ] || ! kill -0 "$old_fwd_pid" 2>/dev/null; then
      can_resume=false
      reason="forwarder process (pid ${old_fwd_pid:-unknown}) dead"
    elif [ ! -p "$input_fifo" ]; then
      can_resume=false
      reason="input FIFO missing"
    elif [ ! -p "$primary_pipe" ]; then
      can_resume=false
      reason="primary pipe missing"
    fi

    if [ "$can_resume" = true ]; then
      local existing_keep_actions_window_open=false
      if [ -f "${sdir}/keep_actions_window_open" ]; then
        existing_keep_actions_window_open="$(cat "${sdir}/keep_actions_window_open")"
      fi
      if [ "$keep_actions_window_open_requested" = true ] \
         && [ "$existing_keep_actions_window_open" != true ]; then
        json_error "session_env_mismatch" "Session '${name}' exists but was not launched with SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN=1. Use a fresh session name or stop it explicitly."
        return 1
      fi
      cleanup_orphan_session_forwarders "$primary_pipe" "$input_fifo" "$old_fwd_pid"
      log "Resuming existing session '${name}' (pid ${old_pid})"
      json_envelope "ok" \
        "session:\"${name}\"" \
        "pid:${old_pid}" \
        "pipe:\"${input_fifo}\"" \
        "binary:\"${BINARY}\"" \
        "log:\"${log_path}\"" \
        "responses:\"${responses_path}\"" \
        "lifecycle:\"${lifecycle_path}\"" \
        "resumed:true" \
        "ready:true" \
        "readyWaitMs:0" \
        "readyMarker:\"existing_session\"" \
        "keepActionsWindowOpen:${existing_keep_actions_window_open}"
      return 0
    else
      log "Stale session '${name}': ${reason}. Cleaning up."
      # Preserve the registry unless the stale app is confirmed dead; removing
      # it first makes a stubborn global-hotkey owner impossible to address by
      # session name.
      if ! terminate_session_app "$name" "$sdir" "$old_pid" "$old_supervisor_pid"; then
        return 1
      fi
      if [ -n "$old_fwd_pid" ] && kill -0 "$old_fwd_pid" 2>/dev/null; then
        kill -TERM "$old_fwd_pid" 2>/dev/null || true
      fi
      if [ -n "$old_supervisor_pid" ] && kill -0 "$old_supervisor_pid" 2>/dev/null; then
        kill -TERM "$old_supervisor_pid" 2>/dev/null || true
      fi
      cleanup_orphan_session_forwarders "$primary_pipe" "$input_fifo"
      rm -rf "${sdir}"
    fi
  fi

  # Check binary
  if [ ! -x "$BINARY" ]; then
    json_error "binary_missing" "Binary not found at ${BINARY}. Run cargo build first."
    return 1
  fi

  # Create session directory and pipe
  mkdir -p "${sdir}"
  printf '%s\n' "$keep_actions_window_open_requested" > "${sdir}/keep_actions_window_open"
  printf '%s\n' "$BINARY" > "${sdir}/binary"
  local pipe_path="${sdir}/pipe"
  local pid_path="${sdir}/pid"
  local supervisor_pid_path="${sdir}/supervisor_pid"

  rm -f "$pipe_path" "$exit_receipt_path" "$supervisor_pid_path"
  mkfifo "$pipe_path"

  # Agents send commands by appending to the input FIFO via `session.sh send`.
  # We use a secondary FIFO as an input queue that a background forwarder relays
  # into the app pipe while keeping the write end open across shells.
  rm -f "$input_fifo"
  mkfifo "$input_fifo"
  cleanup_orphan_session_forwarders "$pipe_path" "$input_fifo"

  # Create the responses artifact files
  : > "$responses_path"
  : > "$protocol_responses_path"
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen > "$generation_path"
  else
    printf '%s-%s\n' "$(date +%s)" "$$" > "$generation_path"
  fi
  : > "$lifecycle_path"

  # Background forwarder: reads from input_fifo and writes to pipe.
  # It is started before the app so the app's read-open on the primary FIFO
  # does not block forever waiting for a writer.
  nohup python3 -c 'import os, sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])' bash -c '
    trap "" HUP
    pipe_path="$1"
    input_fifo="$2"

    # One background process owns the primary pipe writer for the app. Keep
    # input_fifo open read-write so one-shot senders can disconnect without
    # delivering EOF through the app stdin pipe.
    exec 3<>"$input_fifo"
    while [ -p "$input_fifo" ]; do
      if IFS= read -r line <&3; then
        printf "%s\n" "$line"
      else
        sleep 0.05
      fi
    done > "$pipe_path"
  ' _ "$pipe_path" "$input_fifo" </dev/null >/dev/null 2>&1 &
  local fwd_pid=$!
  echo "$fwd_pid" > "${sdir}/fwd_pid"

  # Launch the app reading from the pipe after the forwarder has opened the
  # write end, otherwise the shell can deadlock opening the read end.
  local session_generation
  session_generation="$(tr -d '\n' < "$generation_path")"
  local keep_actions_window_open_env=0
  if [ "$keep_actions_window_open_requested" = true ]; then
    keep_actions_window_open_env=1
  fi
  local agentic_rust_log="${SCRIPT_KIT_AGENTIC_RUST_LOG:-info,gpui::window=off,gpui=warn,hyper=warn,reqwest=warn}"

  nohup env \
    SCRIPT_KIT_AI_LOG=1 \
    SCRIPT_KIT_SHORTCUT_DEBUG=1 \
    RUST_LOG="$agentic_rust_log" \
    SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN="$keep_actions_window_open_env" \
    SCRIPT_KIT_AGENTIC_PROTOCOL_RESPONSES_PATH="$protocol_responses_path" \
    SCRIPT_KIT_AGENTIC_SESSION_NAME="$name" \
    SCRIPT_KIT_AGENTIC_SESSION_GENERATION="$session_generation" \
    python3 "${PROJECT_ROOT}/scripts/agentic/session-supervisor.py" \
      --binary "$BINARY" \
      --stdin-path "$pipe_path" \
      --stdout-path "$log_path" \
      --session-dir "$sdir" \
      --session-name "$name" \
      --generation "$session_generation" \
      </dev/null >/dev/null 2>&1 &
  local supervisor_pid=$!
  echo "$supervisor_pid" > "$supervisor_pid_path"
  local app_pid=""
  local pid_wait_ms=0
  while [ "$pid_wait_ms" -lt 3000 ]; do
    if [ -s "$pid_path" ]; then
      app_pid="$(cat "$pid_path" 2>/dev/null || true)"
      if [ -n "$app_pid" ] && kill -0 "$app_pid" 2>/dev/null; then
        break
      fi
    fi
    if ! kill -0 "$supervisor_pid" 2>/dev/null; then
      kill "$fwd_pid" 2>/dev/null || true
      wait "$fwd_pid" 2>/dev/null || true
      json_error "start_failed" "Session supervisor exited before writing app pid. Check ${log_path}"
      rm -rf "${sdir}"
      return 1
    fi
    sleep 0.025
    pid_wait_ms=$((pid_wait_ms + 25))
  done
  if [ -z "$app_pid" ] || ! kill -0 "$app_pid" 2>/dev/null; then
    kill "$supervisor_pid" 2>/dev/null || true
    kill "$fwd_pid" 2>/dev/null || true
    wait "$fwd_pid" 2>/dev/null || true
    json_error "start_failed" "Session supervisor did not produce a live app pid. Check ${log_path}"
    rm -rf "${sdir}"
    return 1
  fi
  local startup_keepalive=false
  if send_startup_keepalive "$input_fifo" "$app_pid"; then
    startup_keepalive=true
  fi

  # Wait for the earliest safe readiness marker instead of a fixed sleep.
  local ready=false
  local ready_wait_ms=0
  local ready_marker="none"
  if wait_for_ready_log "$log_path" "$app_pid" "$READY_TIMEOUT_MS"; then
    ready=true
    ready_wait_ms="$READY_WAIT_MS_RESULT"
    ready_marker="$READY_MARKER_RESULT"
    log "Session '${name}' reached readiness marker '${ready_marker}' in ${ready_wait_ms}ms"
  else
    local ready_status=$?
    ready_wait_ms="$READY_WAIT_MS_RESULT"
    ready_marker="$READY_MARKER_RESULT"
    if [ "$ready_status" -eq 2 ]; then
      kill "$supervisor_pid" 2>/dev/null || true
      kill "$fwd_pid" 2>/dev/null || true
      wait "$fwd_pid" 2>/dev/null || true
      json_error "start_failed" "App process exited before readiness marker. Check ${log_path}"
      rm -rf "${sdir}"
      return 1
    fi
    log "Session '${name}' did not emit readiness marker within ${READY_TIMEOUT_MS}ms; continuing"
  fi

  # Final liveness check
  if ! kill -0 "$app_pid" 2>/dev/null; then
    kill "$supervisor_pid" 2>/dev/null || true
    kill "$fwd_pid" 2>/dev/null || true
    wait "$fwd_pid" 2>/dev/null || true
    json_error "start_failed" "App process exited immediately. Check ${log_path}"
    rm -rf "${sdir}"
    return 1
  fi

  log "Started session '${name}' (pid ${app_pid}, ready=${ready}, marker=${ready_marker}, waited=${ready_wait_ms}ms)"
  json_envelope "ok" \
    "session:\"${name}\"" \
    "pid:${app_pid}" \
    "pipe:\"${input_fifo}\"" \
    "binary:\"${BINARY}\"" \
    "log:\"${log_path}\"" \
    "responses:\"${responses_path}\"" \
    "protocolResponses:\"${protocol_responses_path}\"" \
    "sessionGeneration:\"${session_generation}\"" \
    "lifecycle:\"${lifecycle_path}\"" \
    "aiLog:true" \
    "shortcutDebug:true" \
    "rustLog:\"${agentic_rust_log}\"" \
    "resumed:false" \
    "ready:${ready}" \
    "readyWaitMs:${ready_wait_ms}" \
    "readyMarker:\"${ready_marker}\"" \
    "startupKeepalive:${startup_keepalive}" \
    "keepActionsWindowOpen:${keep_actions_window_open_requested}"
}

# --- send -------------------------------------------------------------------

cmd_send() {
  local name="${1:-default}"
  local cmd="${2:-}"
  shift 2 2>/dev/null || true

  # Parse optional flags: --await-parse, --timeout MS
  local await_parse=""
  local timeout_ms=2000
  while [ $# -gt 0 ]; do
    case "$1" in
      --await-parse) await_parse="1"; shift ;;
      --timeout)     timeout_ms="${2:-2000}"; shift 2 ;;
      *)             shift ;;
    esac
  done

  # Reject non-numeric --timeout before any arithmetic below — otherwise
  # `set -u` at `deadline_ms=$(( now_ms + timeout_ms ))` treats the
  # string as an unbound variable and `set -e` kills the function with
  # no JSON envelope (anomaly session-send-await-parse-timeout-non-numeric,
  # Run 5 Pass #8). Contract pinned by
  # tests/session_send_await_parse_contract.rs::cmd_send_rejects_non_numeric_timeout.
  if [ -n "$await_parse" ] && ! [[ "$timeout_ms" =~ ^[0-9]+$ ]]; then
    json_error "invalid_timeout" "--timeout must be a non-negative integer; got: ${timeout_ms}"
    return 1
  fi

  if [ -z "$cmd" ]; then
    json_error "missing_command" "Usage: session.sh send SESSION_NAME JSON_COMMAND [--await-parse [--timeout MS]]"
    return 1
  fi

  # Reject flag-as-command: arg-order typo where a flag binds to $2 and
  # the JSON payload ends up consumed by the flag-parse loop's catch-all.
  # Pre-fix, `send SESSION --await-parse JSON` wrote the literal bytes
  # `--await-parse\n` (13 bytes) to the input FIFO and returned
  # `{sent:true}`; the stdin parser logged
  # `stdin_parse_failed line_len=13 error="invalid number at line 1 column 2"`
  # but the caller saw no signal. Anomaly
  # `attacker-session-send-argorder-swallow`, Run 8 Pass #20 (commit
  # d3a15cefc). Contract pinned by
  # tests/session_send_await_parse_contract.rs::cmd_send_rejects_flag_as_command.
  if [[ "$cmd" == --* ]]; then
    json_error "flag_as_command" "First non-session arg looks like a flag (got: '${cmd}'). Usage: session.sh send SESSION_NAME JSON_COMMAND [--await-parse [--timeout MS]] — flags go AFTER the JSON payload."
    return 1
  fi

  local sdir
  sdir="$(session_dir "$name")"
  local input_fifo="${sdir}/input"
  local log_path="${sdir}/app.log"

  if [ ! -p "$input_fifo" ]; then
    json_error "no_session" "Session '${name}' not found or input FIFO missing."
    return 1
  fi

  # Verify app is alive
  if [ -f "${sdir}/pid" ]; then
    local pid
    pid="$(cat "${sdir}/pid")"
    if ! kill -0 "$pid" 2>/dev/null; then
      json_lifecycle_error "$name" "app_process_dead_before_send" "Session '${name}' app process (pid ${pid}) is not running."
      return 1
    fi
  fi

  # Verify forwarder is alive — fail fast instead of hanging on a dead pipe
  if [ -f "${sdir}/fwd_pid" ]; then
    local fwd_pid
    fwd_pid="$(cat "${sdir}/fwd_pid")"
    if ! kill -0 "$fwd_pid" 2>/dev/null; then
      json_lifecycle_error "$name" "forwarder_dead_before_send" "Session '${name}' input forwarder is not running."
      return 1
    fi
  else
    json_lifecycle_error "$name" "forwarder_dead_before_send" "Session '${name}' input forwarder PID file missing."
    return 1
  fi

  if [ -n "$await_parse" ]; then
    ensure_session_protocol_bus "$sdir"
    if ! acquire_session_lock "$sdir" "$timeout_ms"; then
      json_error "queue_timeout" "Timed out waiting for session '${name}' command queue after ${timeout_ms}ms."
      return 1
    fi
  fi

  # Record app.log offset BEFORE sending so we only scan the event emitted
  # for THIS send. stdin is line-serialized in the app, so the next
  # stdin_command_parsed or stdin_parse_failed after this offset
  # corresponds to this send.
  local start_offset=0
  if [ -n "$await_parse" ] && [ -f "$log_path" ]; then
    start_offset="$(wc -c < "$log_path" | tr -d ' ')"
  fi

  # Write command to the input FIFO (non-blocking via timeout)
  if ! printf '%s\n' "$cmd" > "$input_fifo" 2>/dev/null; then
    if [ -n "$await_parse" ]; then
      release_session_lock "$sdir"
    fi
    json_error "send_failed" "Failed to write to session '${name}' input FIFO."
    return 1
  fi

  if [ -z "$await_parse" ]; then
    json_envelope "ok" "session:\"${name}\"" "sent:true"
    return 0
  fi

  # Poll app.log from start_offset for the next parse outcome event.
  # Uses millisecond-resolution deadline via bash's $EPOCHREALTIME when
  # available, else falls back to seconds-resolution.
  local now_ms
  if [ -n "${EPOCHREALTIME:-}" ]; then
    now_ms="$(printf '%s' "$EPOCHREALTIME" | awk -F. '{printf "%d", $1*1000 + int(substr($2"000000",1,3))}')"
  else
    now_ms="$(($(date +%s) * 1000))"
  fi
  local deadline_ms=$(( now_ms + timeout_ms ))

  # Extract requestId from the sent payload so the happy-path grep can
  # scope on `cid=stdin:req:<request_id> ` instead of first-event-past-
  # offset. Without this scoping, two concurrent --await-parse sends
  # share a pre-send offset window and BOTH latch onto whichever parse
  # event landed first — anomaly
  # session-send-await-parse-concurrent-cross-correlation filed Pass #8
  # Run 5, where 5 parallel sends with distinct commandTypes all
  # reported commandType:"show" instead of their own.
  #
  # Only trust values that match a conservative charset (letters,
  # digits, `-_.:/`) — anything outside that (shell metachars,
  # whitespace, newlines) falls back to the legacy offset-first grep
  # to avoid grep-injection via attacker-controlled requestId.
  #
  # Contract pinned by
  # tests/session_send_await_parse_contract.rs::cmd_send_scopes_happy_path_grep_on_request_id.
  local req_id=""
  req_id="$(printf '%s' "$cmd" | sed -nE 's/.*"requestId"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p' | head -n1)"
  if ! [[ "$req_id" =~ ^[A-Za-z0-9_.:/-]+$ ]]; then
    req_id=""
  fi

  while :; do
    if [ -f "$log_path" ]; then
      # Read everything after start_offset. Use `tail -c +N` where N is
      # 1-indexed (start_offset+1 = byte after the recorded offset).
      local tail_content=""
      tail_content="$(tail -c "+$((start_offset + 1))" "$log_path" 2>/dev/null || true)"
      # stdin_command_parsed — happy path. When req_id is set, scope
      # the match to this send's correlation_id so concurrent sends
      # don't cross-correlate. When req_id is empty (no requestId in
      # payload), fall back to the legacy offset-first grep which
      # assumes single-caller serial usage.
      local parsed_line=""
      if [ -n "$req_id" ]; then
        parsed_line="$(printf '%s' "$tail_content" | grep -F -- "cid=stdin:req:${req_id} " | grep -m1 'event_type=stdin_command_parsed' || true)"
      else
        parsed_line="$(printf '%s' "$tail_content" | grep -m1 'event_type=stdin_command_parsed' || true)"
      fi
      if [ -n "$parsed_line" ]; then
        local command_type
        command_type="$(printf '%s' "$parsed_line" | sed -nE 's/.*command_type=([^ ]+).*/\1/p')"
        json_envelope "ok" \
          "session:\"${name}\"" \
          "sent:true" \
          "parseOutcome:\"parsed\"" \
          "commandType:\"${command_type:-unknown}\""
        release_session_lock "$sdir"
        return 0
      fi
      # stdin_parse_failed — sad path. Symmetric with the happy-path
      # scope: when req_id is set, filter on `cid=stdin:req:${req_id} `
      # so concurrent parse failures de-interleave. Requires the Rust
      # listener to emit `correlation_id = "stdin:req:<request_id>"` on
      # the parse-failed tracing span even when full deserialization
      # fails — achieved by `extract_request_id_lenient` in
      # src/stdin_commands/mod.rs. Without that Rust-side lenient
      # extract, this grep would match zero lines under concurrency
      # (the only such events carry `stdin:parse:<uuid>` instead) and
      # we'd fall through to `timeout`. Fallback to legacy offset-first
      # grep preserves the single-caller precondition for payloads that
      # lack an extractable requestId (e.g. malformed JSON where the
      # `"requestId":"..."` structure is itself broken). Contract
      # pinned by tests/session_send_await_parse_contract.rs::cmd_send_scopes_sad_path_grep_on_request_id.
      local failed_line=""
      if [ -n "$req_id" ]; then
        failed_line="$(printf '%s' "$tail_content" | grep -F -- "cid=stdin:req:${req_id} " | grep -m1 'event_type=stdin_parse_failed' || true)"
      else
        failed_line="$(printf '%s' "$tail_content" | grep -m1 'event_type=stdin_parse_failed' || true)"
      fi
      if [ -n "$failed_line" ]; then
        # Extract `error=…` up to ` at line ` (or end of line). Truncate
        # to 200 chars and JSON-escape backslashes + double quotes so the
        # envelope stays valid regardless of serde's error text.
        local err_msg
        err_msg="$(printf '%s' "$failed_line" | sed -nE 's/.*error=(.*)/\1/p' | sed -E 's/ at line [0-9]+ column [0-9]+.*$//' | cut -c1-200 | sed 's/\\/\\\\/g; s/"/\\"/g')"
        json_envelope "ok" \
          "session:\"${name}\"" \
          "sent:true" \
          "parseOutcome:\"parseError\"" \
          "error:\"${err_msg}\""
        release_session_lock "$sdir"
        return 0
      fi
    fi

    if [ -n "${EPOCHREALTIME:-}" ]; then
      now_ms="$(printf '%s' "$EPOCHREALTIME" | awk -F. '{printf "%d", $1*1000 + int(substr($2"000000",1,3))}')"
    else
      now_ms="$(($(date +%s) * 1000))"
    fi
    if [ "$now_ms" -ge "$deadline_ms" ]; then
      json_envelope "ok" \
        "session:\"${name}\"" \
        "sent:true" \
        "parseOutcome:\"timeout\"" \
        "timeoutMs:${timeout_ms}"
      release_session_lock "$sdir"
      return 0
    fi
    sleep 0.05
  done
}

# --- rpc --------------------------------------------------------------------

cmd_rpc() {
  local name="${1:-default}"
  local cmd="${2:-}"
  shift 2 || true

  if [ -z "$cmd" ]; then
    json_error "missing_command" "Usage: session.sh rpc SESSION_NAME JSON_COMMAND [--expect TYPE] [--timeout MS]"
    return 1
  fi

  # Extract requestId from the command JSON (validate early, before session checks)
  local request_id
  request_id="$(
    printf '%s' "$cmd" \
      | sed -nE 's/.*"requestId"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
      | head -1 \
      || true
  )"
  if [ -z "$request_id" ]; then
    json_error "missing_request_id" "RPC command must contain a requestId field."
    return 1
  fi
  # Round-trip gate: reject requestIds whose sed-extracted bytes won't match
  # the Rust-side JSON-decoded value. Control chars (NUL, newline, tab, etc.)
  # and backslash-bearing escape sequences (`\u0000`, `\n`, `\t`, `\\`) are
  # the observed failure classes — the sed extractor keeps them literal while
  # the app's serde JSON parser decodes them, so the request_id string used
  # to correlate with responses.ndjson never matches and the caller gets a
  # silent `timeout` envelope. See Run 8 Pass #24 anomaly
  # `attacker-stdin-requestid-nul-newline-match-loss` (audits/afk/stories.md);
  # acceptance option (a). Pinned by
  # tests/session_rpc_requestid_charset_reject_contract.rs.
  if [[ "$request_id" == *'\'* ]] || [[ "$request_id" =~ [[:cntrl:]] ]]; then
    json_error "invalid_request_id_charset" "RPC requestId must not contain control characters or backslash-bearing escape sequences — such bytes round-trip differently between the sender-side sed extractor and the Rust-side JSON parser and produce silent timeouts. Use requestIds matching [A-Za-z0-9_.:/-]+ to stay within the Rust correlation charset."
    return 1
  fi

  local sdir
  sdir="$(session_dir "$name")"
  local input_fifo="${sdir}/input"
  local log_path="${sdir}/app.log"
  local responses_path="${sdir}/responses.ndjson"
  local protocol_responses_path="${sdir}/protocol-responses.ndjson"

  if [ ! -p "$input_fifo" ]; then
    json_error "no_session" "Session '${name}' not found or input FIFO missing."
    return 1
  fi

  ensure_session_protocol_bus "$sdir"

  # Verify app is alive
  if [ -f "${sdir}/pid" ]; then
    local pid
    pid="$(cat "${sdir}/pid")"
    if ! kill -0 "$pid" 2>/dev/null; then
      json_lifecycle_error "$name" "app_process_dead_before_rpc" "Session '${name}' app process (pid ${pid}) is not running."
      return 1
    fi
  fi

  # Verify forwarder is alive
  if [ -f "${sdir}/fwd_pid" ]; then
    local fwd_pid
    fwd_pid="$(cat "${sdir}/fwd_pid")"
    if ! kill -0 "$fwd_pid" 2>/dev/null; then
      json_lifecycle_error "$name" "forwarder_dead_before_rpc" "Session '${name}' input forwarder is not running."
      return 1
    fi
  else
    json_lifecycle_error "$name" "forwarder_dead_before_rpc" "Session '${name}' input forwarder PID file missing."
    return 1
  fi

  # Parse optional flags
  local expect_type=""
  local timeout_ms="5000"
  while [ $# -gt 0 ]; do
    case "$1" in
      --expect)  expect_type="${2:-}"; shift 2 ;;
      --timeout) timeout_ms="${2:-5000}"; shift 2 ;;
      *)         shift ;;
    esac
  done

  if ! acquire_session_lock "$sdir" "$timeout_ms"; then
    json_error "queue_timeout" "Timed out waiting for session '${name}' command queue after ${timeout_ms}ms."
    return 1
  fi

  local start_offset="0"
  if [ -f "$protocol_responses_path" ]; then
    start_offset="$(wc -c < "$protocol_responses_path" | tr -d '[:space:]')"
  fi

  # Send the command (fire-and-forget to the pipe)
  if ! printf '%s\n' "$cmd" > "$input_fifo" 2>/dev/null; then
    release_session_lock "$sdir"
    json_error "send_failed" "Failed to write to session '${name}' input FIFO."
    return 1
  fi

  # Await the response using the TypeScript helper
  local await_args=(
    --session "$name"
    --request-id "$request_id"
    --timeout "$timeout_ms"
    --start-offset "$start_offset"
    --responses-path "$protocol_responses_path"
  )
  if [ -n "$expect_type" ]; then
    await_args+=(--expect "$expect_type")
  fi

  local result
  local exit_code=0
  result="$(bun "${PROJECT_ROOT}/scripts/agentic/await-response.ts" "${await_args[@]}")" || exit_code=$?

  # Post-hoc parse-failure surfacing. If await-response.ts exited
  # non-zero (typically generic `timeout`), check the app.log tail since
  # start_offset for a `stdin_parse_failed` event scoped to this request.
  # Without this, a malformed payload returns a misleading timeout error
  # with no indication that the app's parser rejected the command. The
  # Rust listener emits `correlation_id=stdin:req:<request_id>` on parse
  # failures via `extract_request_id_lenient` (src/stdin_commands/mod.rs)
  # — same cid scheme as the happy path. Trailing space in the grep
  # pattern prevents prefix matches (e.g. `p15-get` vs `p15-get-notarget`).
  # Contract pinned by tests/session_rpc_parse_error_surface_contract.rs.
  if [ "$exit_code" -ne 0 ] && [ -f "$log_path" ]; then
    local new_size
    new_size="$(wc -c < "$log_path" | tr -d '[:space:]')"
    if [ "$new_size" -gt "$start_offset" ]; then
      local tail_bytes=$(( new_size - start_offset ))
      local failed_line
      # Charset gate for grep scoping — mirrors cmd_send's gate at line
      # ~390. Rust's extract_request_id_lenient (src/stdin_commands/mod.rs)
      # accepts only `[A-Za-z0-9_.:/-]` as a correlation id; payloads with
      # requestIds outside that charset (e.g. `a+b`, `a\b`) get an
      # auto-generated `stdin:parse:<uuid>` cid instead, which the scoped
      # `cid=stdin:req:${request_id}` grep would never match — silently
      # degrading the post-hoc scan to a generic `timeout` envelope even
      # though app.log HAS a correlated parse-failure line (just under a
      # uuid cid). Fall back to unscoped grep on the log tail when the
      # charset gate fails, preserving parse-error surfacing at the cost
      # of de-interleaving across concurrent malformed sends (the same
      # trade cmd_send makes on lines 390-392, 408-410, 438-440). Pinned
      # by tests/session_rpc_parse_error_surface_contract.rs::cmd_rpc_falls_back_to_unscoped_grep_on_charset_boundary.
      if [[ "$request_id" =~ ^[A-Za-z0-9_.:/-]+$ ]]; then
        failed_line="$(tail -c "$tail_bytes" "$log_path" 2>/dev/null | grep -F -- "cid=stdin:req:${request_id} " | grep -m1 'event_type=stdin_parse_failed' || true)"
      else
        failed_line="$(tail -c "$tail_bytes" "$log_path" 2>/dev/null | grep -m1 'event_type=stdin_parse_failed' || true)"
      fi
      if [ -n "$failed_line" ]; then
        local err_msg
        err_msg="$(printf '%s' "$failed_line" | sed -nE 's/.*error=(.*)/\1/p' | sed -E 's/ at line [0-9]+ column [0-9]+.*$//' | cut -c1-200 | sed 's/\\/\\\\/g; s/"/\\"/g')"
        result="$(printf '{"schemaVersion":1,"status":"error","session":"%s","requestId":"%s","error":{"code":"parse_error","message":"%s"}}' "$name" "$request_id" "$err_msg")"
      fi
    fi
  fi

  # Append to responses artifact
  if [ -n "$result" ]; then
    printf '%s\n' "$result" >> "$responses_path" 2>/dev/null || true
  fi

  release_session_lock "$sdir"

  printf '%s\n' "$result"
  return $exit_code
}

# --- status -----------------------------------------------------------------

cmd_status() {
  local name="${1:-default}"
  local sdir
  sdir="$(session_dir "$name")"

  if [ ! -d "$sdir" ]; then
    json_envelope "not_found" "session:\"${name}\"" "alive:false"
    return 0
  fi

  local pid="0"
  local alive="false"
  local pipe_path="${sdir}/input"
  local primary_pipe="${sdir}/pipe"
  local log_path="${sdir}/app.log"
  local responses_path="${sdir}/responses.ndjson"
  local lifecycle_path="${sdir}/lifecycle.ndjson"
  local pipe_writable="false"
  local forwarder_alive="false"
  local fwd_pid="0"
  local keep_actions_window_open="false"
  local launched_binary="$BINARY"

  if [ -f "${sdir}/keep_actions_window_open" ]; then
    keep_actions_window_open="$(cat "${sdir}/keep_actions_window_open")"
  fi
  if [ -f "${sdir}/binary" ]; then
    launched_binary="$(cat "${sdir}/binary")"
  elif [ -z "$launched_binary" ]; then
    launched_binary="$(resolve_default_binary)"
  fi

  if [ -f "${sdir}/pid" ]; then
    pid="$(cat "${sdir}/pid")"
    if kill -0 "$pid" 2>/dev/null; then
      alive="true"
    fi
  fi

  if [ -f "${sdir}/fwd_pid" ]; then
    fwd_pid="$(cat "${sdir}/fwd_pid")"
    if kill -0 "$fwd_pid" 2>/dev/null; then
      forwarder_alive="true"
    fi
  fi

  if [ -p "$pipe_path" ]; then
    pipe_writable="true"
  fi

  # Healthy = app alive AND forwarder alive AND both FIFOs exist
  local healthy="false"
  if [ "$alive" = "true" ] && [ "$forwarder_alive" = "true" ] \
     && [ -p "$pipe_path" ] && [ -p "$primary_pipe" ]; then
    healthy="true"
  fi

  # Collect issues
  local issues="[]"
  local issue_items=""
  if [ "$alive" = "false" ]; then
    issue_items="${issue_items:+$issue_items,}\"app_process_dead\""
  fi
  if [ "$forwarder_alive" = "false" ]; then
    issue_items="${issue_items:+$issue_items,}\"forwarder_dead\""
  fi
  if [ ! -p "$pipe_path" ]; then
    issue_items="${issue_items:+$issue_items,}\"input_fifo_missing\""
  fi
  if [ ! -p "$primary_pipe" ]; then
    issue_items="${issue_items:+$issue_items,}\"primary_pipe_missing\""
  fi
  if [ -n "$issue_items" ]; then
    issues="[${issue_items}]"
  fi

  json_envelope "ok" \
    "session:\"${name}\"" \
    "pid:${pid}" \
    "alive:${alive}" \
    "forwarderPid:${fwd_pid}" \
    "forwarderAlive:${forwarder_alive}" \
    "healthy:${healthy}" \
    "issues:${issues}" \
    "pipe:\"${pipe_path}\"" \
    "pipeWritable:${pipe_writable}" \
    "binary:\"${launched_binary}\"" \
    "keepActionsWindowOpen:${keep_actions_window_open}" \
    "log:\"${log_path}\"" \
    "responses:\"${responses_path}\"" \
    "lifecycle:\"${lifecycle_path}\""
}

# --- stop -------------------------------------------------------------------

cmd_stop() {
  local name="${1:-default}"
  shift 1 2>/dev/null || true
  local expected_pid=""
  local expected_generation=""
  local expected_pid_set=false
  local expected_generation_set=false
  while [ $# -gt 0 ]; do
    case "$1" in
      --expected-pid)
        if [ $# -lt 2 ]; then
          json_error "invalid_stop_ownership_options" "--expected-pid requires a value."
          return 1
        fi
        expected_pid="$2"
        expected_pid_set=true
        shift 2
        ;;
      --expected-generation)
        if [ $# -lt 2 ]; then
          json_error "invalid_stop_ownership_options" "--expected-generation requires a value."
          return 1
        fi
        expected_generation="$2"
        expected_generation_set=true
        shift 2
        ;;
      *)
        json_error "invalid_stop_ownership_options" "Unknown stop option: $1"
        return 1
        ;;
    esac
  done

  if [ "$expected_pid_set" != "$expected_generation_set" ]; then
    json_error "invalid_stop_ownership_options" "--expected-pid and --expected-generation must be provided together."
    return 1
  fi
  if [ "$expected_pid_set" = true ]; then
    if ! [[ "$expected_pid" =~ ^[1-9][0-9]*$ ]] || [ -z "$expected_generation" ]; then
      json_error "invalid_stop_ownership_options" "Strict stop requires a positive integer PID and a non-empty generation."
      return 1
    fi
    STOP_OWNERSHIP_STRICT=true
    STOP_EXPECTED_PID="$expected_pid"
    STOP_EXPECTED_GENERATION="$expected_generation"
  fi

  local sdir
  sdir="$(session_dir "$name")"

  if [ ! -d "$sdir" ]; then
    if ! verify_stop_ownership "$name" "$sdir"; then
      return 1
    fi
    json_envelope "ok" "session:\"${name}\"" "wasRunning:false"
    return 0
  fi

  if ! verify_stop_ownership "$name" "$sdir"; then
    return 1
  fi

  local fwd_pid=""
  local supervisor_pid=""
  local pid=""
  if [ -f "${sdir}/fwd_pid" ]; then
    fwd_pid="$(cat "${sdir}/fwd_pid")"
  fi
  if [ -f "${sdir}/supervisor_pid" ]; then
    supervisor_pid="$(cat "${sdir}/supervisor_pid")"
  fi
  if [ -f "${sdir}/pid" ]; then
    pid="$(cat "${sdir}/pid")"
  fi

  # Stop and confirm the supervisor-owned app before deleting the registry that
  # identifies it. Otherwise an ignored SIGTERM becomes an unaddressable,
  # globally-hotkey-owning orphan.
  if [ -n "$pid" ] && ! terminate_session_app "$name" "$sdir" "$pid" "$supervisor_pid"; then
    return 1
  fi

  # The app is confirmed dead; its supervisor should exit after writing the
  # receipt, but stop any straggling supervisor/forwarder before FIFO cleanup.
  if [ -n "$supervisor_pid" ] && kill -0 "$supervisor_pid" 2>/dev/null; then
    if ! verify_stop_ownership "$name" "$sdir"; then
      return 1
    fi
    kill -TERM "$supervisor_pid" 2>/dev/null || true
  fi
  if [ -n "$fwd_pid" ] && kill -0 "$fwd_pid" 2>/dev/null; then
    if ! verify_stop_ownership "$name" "$sdir"; then
      return 1
    fi
    kill -TERM "$fwd_pid" 2>/dev/null || true
  fi
  if ! cleanup_orphan_session_forwarders "${sdir}/pipe" "${sdir}/input" "" "$name" "$sdir"; then
    return 1
  fi

  # Clean up FIFOs and directory
  if ! verify_stop_ownership "$name" "$sdir"; then
    return 1
  fi
  rm -f "${sdir}/pipe" "${sdir}/input"
  if ! verify_stop_ownership "$name" "$sdir"; then
    return 1
  fi
  rm -rf "${sdir}"

  if [ "$STOP_OWNERSHIP_STRICT" = true ]; then
    json_envelope "ok" \
      "session:\"${name}\"" \
      "wasRunning:true" \
      "forcedKill:${SESSION_STOP_FORCED_KILL}" \
      "ownershipVerified:true" \
      "expectedPid:${STOP_EXPECTED_PID}" \
      "actualPid:${STOP_ACTUAL_PID}" \
      "expectedGeneration:\"$(json_escape "$STOP_EXPECTED_GENERATION")\"" \
      "actualGeneration:\"$(json_escape "$STOP_ACTUAL_GENERATION")\""
  else
    json_envelope "ok" \
      "session:\"${name}\"" \
      "wasRunning:true" \
      "forcedKill:${SESSION_STOP_FORCED_KILL}"
  fi
}

# --- main -------------------------------------------------------------------

subcmd="${1:-}"
shift || true

case "$subcmd" in
  start)  cmd_start "$@" ;;
  send)   cmd_send "$@" ;;
  rpc)    cmd_rpc "$@" ;;
  stop)   cmd_stop "$@" ;;
  status) cmd_status "$@" ;;
  *)
    json_error "unknown_command" "Usage: session.sh {start|send|rpc|stop|status} [SESSION_NAME] [ARGS]"
    exit 1
    ;;
esac
