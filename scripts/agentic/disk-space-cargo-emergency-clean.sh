#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="${SCRIPT_KIT_REPO_ROOT:-/Users/johnlindquist/dev/script-kit-gpui}"
STATE_DIR="${SCRIPT_KIT_WATCHER_STATE_DIR:-$HOME/Library/Application Support/script-kit-gpui/disk-space-cargo-watcher}"
THRESHOLD_GIB="${SCRIPT_KIT_FREE_THRESHOLD_GIB:-25}"
TARGET_FREE_GIB="${SCRIPT_KIT_TARGET_FREE_GIB:-35}"
APPLY=0
REASON="manual"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --apply) APPLY=1; shift ;;
        --repo) REPO_ROOT="$2"; shift 2 ;;
        --state-dir) STATE_DIR="$2"; shift 2 ;;
        --threshold-gib) THRESHOLD_GIB="$2"; shift 2 ;;
        --target-free-gib) TARGET_FREE_GIB="$2"; shift 2 ;;
        --reason) REASON="$2"; shift 2 ;;
        --help|-h)
            cat <<EOF
Usage: $0 --apply --repo /path/to/repo --threshold-gib 25 --target-free-gib 35 --state-dir /path/to/state
EOF
            exit 0
            ;;
        *) echo "[cargo-clean] unknown argument: $1" >&2; exit 2 ;;
    esac
done

PATH="/Users/johnlindquist/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export PATH

cd "$REPO_ROOT"
SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/agentic/cargo-cache-locks.sh
source "${SCRIPT_ROOT}/cargo-cache-locks.sh"

log() { echo "[cargo-clean] $(date '+%Y-%m-%dT%H:%M:%S%z') $*" >&2; }

free_gib() {
    df -Pk "$REPO_ROOT" | awk 'NR==2 { printf "%.1f", $4 / 1048576 }'
}

ge_float() { awk -v a="$1" -v b="$2" 'BEGIN { exit(a >= b ? 0 : 1) }'; }
lt_float() { awk -v a="$1" -v b="$2" 'BEGIN { exit(a < b ? 0 : 1) }'; }

show_report() {
    log "free=$(free_gib)GiB threshold=${THRESHOLD_GIB}GiB target=${TARGET_FREE_GIB}GiB"
    du -sh \
        target \
        target/debug \
        target/debug/incremental \
        target-agent \
        target-agent/pools \
        target-agent/agents \
        target-agent/runtime \
        2>/dev/null || true
    if [ -d target-agent/.locks ]; then
        log "agent locks:"
        find target-agent/.locks -mindepth 1 -maxdepth 2 -type f -name pid \
            -print -exec sh -c 'printf "  "; cat "$1"; printf "\n"' sh {} \; \
            2>/dev/null || true
    fi
}

run_prune() {
    local prune_time_days="$1"
    local prune_agent_days="$2"
    local prune_incremental_days="$3"

    if [ ! -x ./scripts/agentic/prune-cargo-targets.sh ]; then
        log "missing executable ./scripts/agentic/prune-cargo-targets.sh"
        return 1
    fi

    if [ "$APPLY" = "1" ]; then
        log "running prune apply PRUNE_TIME_DAYS=${prune_time_days} PRUNE_AGENT_DAYS=${prune_agent_days} PRUNE_INCREMENTAL_DAYS=${prune_incremental_days}"
        PRUNE_TIME_DAYS="$prune_time_days" \
        PRUNE_AGENT_DAYS="$prune_agent_days" \
        PRUNE_INCREMENTAL_DAYS="$prune_incremental_days" \
            ./scripts/agentic/prune-cargo-targets.sh --apply || true
    else
        log "dry-run prune only; pass --apply to delete"
        PRUNE_TIME_DAYS="$prune_time_days" \
        PRUNE_AGENT_DAYS="$prune_agent_days" \
        PRUNE_INCREMENTAL_DAYS="$prune_incremental_days" \
            ./scripts/agentic/prune-cargo-targets.sh || true
    fi
}

emergency_delete_agent_targets() {
    local candidate
    [[ -d target-agent ]] || return 0
    log "emergency inspecting unlocked individual pools; active and warm pools stay protected"
    for candidate in "${REPO_ROOT}"/target-agent/pools/* "${REPO_ROOT}"/target-agent/agents/*; do
        [[ -d "$candidate" && ! -L "$candidate" ]] || continue
        if cargo_cache_candidate_is_pinned "$candidate" || cargo_cache_candidate_is_locked "$candidate"; then
            log "preserving protected pool ${candidate}"
            continue
        fi
        printf '%s\n' "$candidate"
        if [[ "$APPLY" == "1" ]]; then
            if cargo_cache_remove_candidate "$candidate"; then
                log "removed unlocked pool ${candidate}"
            else
                log "preserved pool after ownership changed ${candidate}"
            fi
        fi
    done
}

emergency_delete_incremental() {
    [[ -d target/debug/incremental ]] || return 0
    if cargo_cache_any_live_lock || [[ "${SCRIPT_KIT_ALLOW_SHARED_INCREMENTAL_EVICTION:-0}" != "1" ]]; then
        log "preserving shared incremental cache; explicit idle-machine opt-in is required"
        return 0
    fi
    log "emergency deleting target/debug/incremental contents"
    if [ "$APPLY" = "1" ]; then
        find target/debug/incremental -mindepth 1 -maxdepth 1 -print -exec rm -rf {} +
    else
        find target/debug/incremental -mindepth 1 -maxdepth 1 -print
    fi
}

# --- Main ---

log "start reason=${REASON} repo=${REPO_ROOT} apply=${APPLY}"
show_report

# Phase 1: normal prune
run_prune 14 7 14

if ge_float "$(free_gib)" "$TARGET_FREE_GIB"; then
    log "target free reached after normal prune"
    show_report
    exit 0
fi

# Phase 2: aggressive pruning of stale, individually unlocked pools. Never
# terminate a user's dev watcher, compiler, or active agent build.
log "still below target free after normal prune; inspecting unlocked stale pools"
if cargo_cache_any_live_lock; then
    log "active Cargo build leases remain protected throughout cleanup"
fi
run_prune 3 1 3

if ge_float "$(free_gib)" "$TARGET_FREE_GIB"; then
    log "target free reached after aggressive prune"
    show_report
    exit 0
fi

# Phase 3: remove only individually claimed, unlocked, non-pinned pools.
log "still below target free; inspecting bounded unlocked cache directories"
emergency_delete_agent_targets
emergency_delete_incremental
run_prune 1 0 1

show_report

if lt_float "$(free_gib)" "$THRESHOLD_GIB"; then
    log "free disk remains below threshold after cleanup"
    exit 2
fi

log "cleanup complete"
exit 0
