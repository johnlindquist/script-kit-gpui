#!/bin/bash
set -euo pipefail

: "${SCRIPT_KIT_CLIPBOARD_PROBE_REAL_BINARY:?missing real app binary}"
: "${SCRIPT_KIT_CLIPBOARD_PROBE_SCRIPT:?missing probe script}"
: "${SCRIPT_KIT_CLIPBOARD_PROBE_FIXTURE:?missing fixture path}"

# Driver.launch({sandboxHome:true}) creates HOME immediately before this wrapper
# starts. Seed that isolated HOME, then replace this process with the pinned app
# so Driver still owns and reaps the real Script Kit process.
/usr/bin/env bun "$SCRIPT_KIT_CLIPBOARD_PROBE_SCRIPT" \
	--seed-home "$HOME" \
	--fixture "$SCRIPT_KIT_CLIPBOARD_PROBE_FIXTURE" \
	>/dev/null

exec "$SCRIPT_KIT_CLIPBOARD_PROBE_REAL_BINARY"
