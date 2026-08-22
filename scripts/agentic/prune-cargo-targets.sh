#!/usr/bin/env bash
# scripts/agentic/prune-cargo-targets.sh — Safely trim target/ and target-agent/.
#
# Goals:
#   - Never delete the whole target/ (cargo clean forces a cold rebuild with
#     no progress output).
#   - Use cargo-sweep to drop artifacts not touched recently. Dry-run first.
#   - Drop individual stale, unlocked agent pools; never delete parent
#     directories, the default warm pool, shared caches, artifacts, or leases.
#
# Usage:
#   scripts/agentic/prune-cargo-targets.sh                # dry-run, no changes
#   scripts/agentic/prune-cargo-targets.sh --apply        # actually prune
#   PRUNE_TIME_DAYS=14 PRUNE_AGENT_DAYS=7 scripts/agentic/prune-cargo-targets.sh --apply
#
# Env:
#   PRUNE_TIME_DAYS    — cargo sweep --time threshold (default: 14)
#   PRUNE_AGENT_DAYS   — find -mtime threshold for target-agent/<id>/ (default: 7)
#   PRUNE_INCREMENTAL_DAYS — find -mtime threshold for target/debug/incremental/* (default: 14)

set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${SCRIPT_KIT_REPO_ROOT:-$(cd "${SCRIPT_ROOT}/../.." && pwd)}"
# shellcheck source=scripts/agentic/cargo-cache-locks.sh
source "${SCRIPT_ROOT}/cargo-cache-locks.sh"
cd "$REPO_ROOT"

APPLY=0
if [ "${1:-}" = "--apply" ]; then
    APPLY=1
fi

PRUNE_TIME_DAYS="${PRUNE_TIME_DAYS:-14}"
PRUNE_AGENT_DAYS="${PRUNE_AGENT_DAYS:-7}"
PRUNE_INCREMENTAL_DAYS="${PRUNE_INCREMENTAL_DAYS:-14}"

log() { echo "[prune] $*" >&2; }

if [ "$APPLY" = "1" ]; then
    log "mode=APPLY — will actually delete"
else
    log "mode=DRY-RUN — no changes; pass --apply to prune"
fi

log "before sizes:"
du -sh target target/debug target/debug/incremental target-agent 2>/dev/null || true

# 1. cargo-sweep on target/, never while a known build owns an active lease.
if cargo_cache_any_live_lock; then
    log "active Cargo build lease detected; preserving shared target artifacts"
elif ! command -v cargo-sweep >/dev/null 2>&1; then
    log "cargo-sweep not installed. Install with: cargo install cargo-sweep"
else
    if [ -d target ]; then
        log "cargo sweep --dry-run --time ${PRUNE_TIME_DAYS} (target/)"
        cargo sweep --dry-run --time "$PRUNE_TIME_DAYS" || true
        if [ "$APPLY" = "1" ]; then
            log "cargo sweep --time ${PRUNE_TIME_DAYS} (target/)"
            cargo sweep --time "$PRUNE_TIME_DAYS" || true
        fi
        log "cargo sweep --dry-run --installed (target/)"
        cargo sweep --dry-run --installed || true
        if [ "$APPLY" = "1" ]; then
            log "cargo sweep --installed (target/)"
            cargo sweep --installed || true
        fi
    fi
fi

# 2. Stale incremental dirs under target/debug/incremental
if [ -d target/debug/incremental ] && ! cargo_cache_any_live_lock; then
    log "stale incremental dirs (-mtime +${PRUNE_INCREMENTAL_DAYS}):"
    find target/debug/incremental -mindepth 1 -maxdepth 1 -type d -mtime +"$PRUNE_INCREMENTAL_DAYS" -print || true
    if [ "$APPLY" = "1" ]; then
        find target/debug/incremental -mindepth 1 -maxdepth 1 -type d -mtime +"$PRUNE_INCREMENTAL_DAYS" -exec rm -rf {} + || true
    fi
fi

# 3. Individual stale pools only. The former top-level find could delete
# target-agent/pools, target-agent/.locks, and active compiler artifacts.
log "stale individual target-agent pools (-mtime +${PRUNE_AGENT_DAYS}):"
for candidate in "${REPO_ROOT}"/target-agent/pools/* "${REPO_ROOT}"/target-agent/agents/*; do
    [[ -d "$candidate" && ! -L "$candidate" ]] || continue

    if cargo_cache_candidate_is_pinned "$candidate"; then
        log "preserve pinned pool ${candidate}"
        continue
    fi
    if cargo_cache_candidate_is_locked "$candidate"; then
        log "preserve active pool ${candidate}"
        continue
    fi
    if [[ -z "$(find "$candidate" -maxdepth 0 -type d -mtime +"$PRUNE_AGENT_DAYS" -print)" ]]; then
        continue
    fi

    printf '%s\n' "$candidate"
    if [[ "$APPLY" == "1" ]]; then
        if cargo_cache_remove_candidate "$candidate"; then
            log "removed unlocked stale pool ${candidate}"
        else
            log "preserved pool after ownership changed ${candidate}"
        fi
    fi
done

log "after sizes:"
du -sh target target/debug target/debug/incremental target-agent 2>/dev/null || true

if [ "$APPLY" != "1" ]; then
    log "Dry-run complete. Re-run with --apply to prune."
fi
