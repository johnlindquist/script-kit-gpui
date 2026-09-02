#!/usr/bin/env bash
# scripts/agentic/dev-crash-watchdog.sh — supervise the dev session app process.
#
# dev.sh launches the app through the agentic session contract, but nothing
# watches the pid afterwards: when the app crashes (e.g. SIGSEGV on first
# render), the hotkey silently stops working and the dev loop still prints
# "Finished". This watchdog closes that gap:
#
#   1. Polls the session pid. Distinguishes an intentional relaunch
#      (dev-relaunch.sh rewrites the pid file) from an abnormal death.
#   2. On abnormal death, prints a LOUD banner including the matching
#      macOS DiagnosticReports .ips file and its faulting symbol.
#   3. Auto-relaunches the session so the hotkey keeps working.
#   4. Stops after repeated crashes of the same binary. Cache/source mutation
#      is never a crash-recovery shortcut; investigate retained exit receipts.
#
# Usage: dev-crash-watchdog.sh <session-name>
# Env:
#   SCRIPT_KIT_SESSION_DIR            — session root (default /tmp/sk-agentic-sessions)
#   SCRIPT_KIT_WATCHDOG_POLL_S        — poll interval seconds (default 2)
#   SCRIPT_KIT_WATCHDOG_MAX_RELAUNCH  — same-binary crashes before giving up (default 3)

set -uo pipefail

if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" ]]; then
    echo "[watchdog] REFUSED: human opt-in dev recovery only" >&2; exit 78
fi

SESSION_NAME="${1:-${SCRIPT_KIT_DEV_SESSION_NAME:-dev-watch}}"
SESSION_DIR="${SCRIPT_KIT_SESSION_DIR:-/tmp/sk-agentic-sessions}/${SESSION_NAME}"
PID_FILE="${SESSION_DIR}/pid"
BIN_PATH="target/debug/script-kit-gpui"
RELAUNCH_SCRIPT="scripts/agentic/dev-relaunch.sh"
POLL_S="${SCRIPT_KIT_WATCHDOG_POLL_S:-2}"
MAX_RELAUNCH="${SCRIPT_KIT_WATCHDOG_MAX_RELAUNCH:-3}"
REPORT_DIRS=("$HOME/Library/Logs/DiagnosticReports" "$HOME/Library/Logs/DiagnosticReports/Retired")

banner() {
    echo "" >&2
    echo "[watchdog] ============================================================" >&2
    while [ $# -gt 0 ]; do
        echo "[watchdog] $1" >&2
        shift
    done
    echo "[watchdog] ============================================================" >&2
}

bin_mtime() {
    stat -f %m "$BIN_PATH" 2>/dev/null || echo 0
}

# Newest app crash report created at/after the given epoch, if any.
find_crash_report() {
    local since_epoch="$1"
    local newest="" newest_m=0 f m
    for dir in "${REPORT_DIRS[@]}"; do
        [ -d "$dir" ] || continue
        while IFS= read -r f; do
            m="$(stat -f %m "$f" 2>/dev/null || echo 0)"
            if [ "$m" -ge "$since_epoch" ] && [ "$m" -gt "$newest_m" ]; then
                newest="$f"
                newest_m="$m"
            fi
        done < <(find "$dir" -maxdepth 1 -name 'script-kit-gpui-*.ips' 2>/dev/null)
    done
    [ -n "$newest" ] && printf '%s\n' "$newest"
}

crash_signature() {
    local report="$1"
    REPORT_PATH="$report" python3 - <<'PY' 2>/dev/null || true
import json, os
path = os.environ["REPORT_PATH"]
d = json.loads(open(path).read().split("\n", 1)[1])
exc = d.get("exception", {})
t = d["threads"][d["faultingThread"]]
syms = [f.get("symbol", "?") for f in t["frames"][:3]]
print(f"{exc.get('signal', '?')} ({exc.get('subtype', '')})")
for s in syms:
    print(f"  {s[:110]}")
PY
}

exit_receipt_for_pid() {
    local expected_pid="$1"
    local receipt="${SESSION_DIR}/app-exit.json"
    [ -f "$receipt" ] || return 1
    RECEIPT="$receipt" EXPECTED_PID="$expected_pid" python3 - <<'PY'
import json, os, sys
try:
    data = json.load(open(os.environ["RECEIPT"]))
except Exception:
    raise SystemExit(1)
if str(data.get("pid")) != str(os.environ["EXPECTED_PID"]):
    raise SystemExit(1)
print(json.dumps(data, separators=(",", ":")))
PY
}

exit_receipt_is_clean() {
    RECEIPT_JSON="$1" python3 - <<'PY'
import json, os
data = json.loads(os.environ["RECEIPT_JSON"])
raise SystemExit(0 if data.get("cleanExit") is True else 1)
PY
}

exit_receipt_status() {
    RECEIPT_JSON="$1" python3 - <<'PY'
import json, os
data = json.loads(os.environ["RECEIPT_JSON"])
print(data.get("exitStatus", "unknown"))
PY
}

relaunch() {
    if bash "$RELAUNCH_SCRIPT" "$SESSION_NAME" >/dev/null 2>&1; then
        echo "[watchdog] session '${SESSION_NAME}' relaunched" >&2
    else
        echo "[watchdog] relaunch FAILED — run: bash ${RELAUNCH_SCRIPT} ${SESSION_NAME}" >&2
    fi
}

crash_count=0
crash_bin_mtime=""
watched_pid=""
watched_since=0
gave_up=0

while true; do
    sleep "$POLL_S"
    pid="$(cat "$PID_FILE" 2>/dev/null || true)"
    [ -n "$pid" ] || continue

    if kill -0 "$pid" 2>/dev/null; then
        if [ "$pid" != "$watched_pid" ]; then
            watched_pid="$pid"
            watched_since="$(date +%s)"
            gave_up=0
        fi
        continue
    fi

    # Pid from the file is dead. Give an intentional relaunch a moment to
    # rewrite the pid file before declaring a crash.
    [ -n "$watched_pid" ] || continue
    sleep 2
    new_pid="$(cat "$PID_FILE" 2>/dev/null || true)"
    if [ -n "$new_pid" ] && [ "$new_pid" != "$watched_pid" ] && kill -0 "$new_pid" 2>/dev/null; then
        watched_pid="$new_pid"
        watched_since="$(date +%s)"
        continue
    fi
    [ "$gave_up" = "1" ] && continue

    receipt="$(exit_receipt_for_pid "$watched_pid" 2>/dev/null || true)"
    if [ -n "$receipt" ] && exit_receipt_is_clean "$receipt"; then
        echo "[watchdog] session '${SESSION_NAME}' app exited cleanly pid=${watched_pid}; not relaunching" >&2
        watched_pid=""
        crash_count=0
        continue
    fi

    # Abnormal death. macOS can take several seconds to write the .ips report.
    report=""
    for _ in 1 2 3 4 5 6; do
        sleep 2
        report="$(find_crash_report "$watched_since")"
        [ -n "$report" ] && break
    done

    current_bin="$(bin_mtime)"
    if [ "$current_bin" = "$crash_bin_mtime" ]; then
        crash_count=$((crash_count + 1))
    else
        crash_count=1
        crash_bin_mtime="$current_bin"
    fi

    if [ -n "$report" ]; then
        lines=("APP CRASHED (pid ${watched_pid}, crash #${crash_count} for this binary)")
    elif [ -n "$receipt" ]; then
        lines=("APP EXITED ABNORMALLY (pid ${watched_pid}, crash #${crash_count} for this binary, exit=$(exit_receipt_status "$receipt"))")
    else
        lines=("APP DIED WITHOUT EXIT RECEIPT (pid ${watched_pid}, crash #${crash_count} for this binary)")
    fi
    if [ -n "$report" ]; then
        lines+=("report: ${report}")
        while IFS= read -r sig_line; do
            lines+=("$sig_line")
        done < <(crash_signature "$report")
    else
        lines+=("no DiagnosticReports .ips found yet — app died without a crash report?")
    fi

    if [ "$crash_count" -ge "$((MAX_RELAUNCH + 1))" ]; then
        gave_up=1
        lines+=("crash #${crash_count} of the same binary — NOT relaunching again.")
        lines+=("Repeated crash: investigate the retained exit receipt and report above.")
        lines+=("ACTION: fix the crash, then save a file to rebuild + relaunch.")
        banner "${lines[@]}"
        watched_pid=""
        continue
    fi


    lines+=("auto-relaunching so the global hotkey keeps working…")
    banner "${lines[@]}"
    relaunch
    watched_pid=""
done
