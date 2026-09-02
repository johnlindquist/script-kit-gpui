#!/usr/bin/env bash
# Cargo remains responsible for harness freshness when no published ref is supplied.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
[[ $# -gt 0 ]] || { echo 'usage: reuse-rust-test-binary.sh <reviewed-test-filter> [...]' >&2; exit 64; }
for filter in "$@"; do
  [[ "$filter" =~ ^[A-Za-z_][A-Za-z0-9_:]*$ ]] || { echo 'invalid reviewed Rust test filter' >&2; exit 64; }
done
export SCRIPT_KIT_NONINTERACTIVE=1
export RUST_TEST_THREADS=1
reference="${SCRIPT_KIT_ARTIFACT_REFERENCE:-}"
if [[ -z "$reference" ]]; then
  reference="$ROOT/.test-output/libtest-$(python3 -c 'import uuid; print(uuid.uuid4())').reference.json"
  bun "$ROOT/scripts/devtools/devtools.ts" build-ops act libtest-build --artifact-out "$reference"
fi
for filter in "$@"; do
  bun "$ROOT/scripts/devtools/devtools.ts" build-ops act lib-test --reference "$reference" --filter "$filter"
done
