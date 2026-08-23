#!/usr/bin/env bash
# Oracle verification plan for isolated-devtools-agent-bootstrap.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FAIL=0

pass() { echo "[verify-devtools-session] PASS: $*" >&2; }
fail() { echo "[verify-devtools-session] FAIL: $*" >&2; FAIL=1; }

echo "[verify-devtools-session] === 1. Bun-only SK_VERIFY (no GPUI) ===" >&2
TODO_APP_SCRIPT="${TODO_APP_SCRIPT:-kit-init/examples/scripts/todo-app.ts}"

if out="$(SK_VERIFY=1 timeout 5 bun "$TODO_APP_SCRIPT" 2>&1)"; then
  if printf '%s' "$out" | grep -q '"ok":true'; then
    pass "todo-app SK_VERIFY"
  else
    fail "todo-app SK_VERIFY missing ok:true: $out"
  fi
else
  fail "todo-app SK_VERIFY exit $? ($TODO_APP_SCRIPT)"
fi

if json="$(bash scripts/agentic/devtools-session.sh verify-script --script "$TODO_APP_SCRIPT" 2>/dev/null)"; then
  if printf '%s' "$json" | grep -q '"status":"ok"'; then
    pass "devtools-session verify-script JSON"
  else
    fail "verify-script JSON: $json"
  fi
else
  fail "devtools-session verify-script"
fi

echo "[verify-devtools-session] === 2. Build timeout wrapper (<10s with timeout=1) ===" >&2
SECONDS=0
DEVTOOLS_SESSION_JSON=1 SCRIPT_KIT_AGENT_ID=dt-agent-build SCRIPT_KIT_CARGO_TARGET_POOL=agent-debug bash scripts/agentic/build-isolated-binary.sh --json 1 >/tmp/sk-build-timeout.json 2>/tmp/sk-build-timeout.err || true
if [[ "$SECONDS" -lt 10 ]]; then
  pass "build-isolated-binary 1s cap elapsed=${SECONDS}s"
else
  fail "build hung ${SECONDS}s (expected <10s)"
fi
if grep -Eq '"status":"(ok|error)"' /tmp/sk-build-timeout.json 2>/dev/null && ! grep -q 'promotedTo' /tmp/sk-build-timeout.json 2>/dev/null; then
  pass "build-isolated-binary JSON contract"
else
  fail "build-isolated-binary JSON contract"
fi

echo "[verify-devtools-session] === 3. classify ===" >&2
if json="$(bash scripts/agentic/devtools-session.sh classify --script kit-init/examples/scripts/todo-app.ts 2>/dev/null)"; then
  if printf '%s' "$json" | grep -q '"status":"ok"'; then
    pass "classify JSON"
  else
    fail "classify: $json"
  fi
else
  fail "classify command"
fi

echo "[verify-devtools-session] === 4. preflight (isolated) ===" >&2
if bash scripts/agentic/preflight-isolated.sh --mode isolated >/dev/null 2>&1; then
  pass "preflight isolated (clean enough)"
else
  code=$?
  echo "[verify-devtools-session] SKIP preflight isolated exit=${code} (dev.sh or multi-GPUI may be active)" >&2
fi

echo "[verify-devtools-session] === 5. isolated start (optional; needs clean machine) ===" >&2
if pgrep -f 'cargo-watch watch.*dev-cycle' >/dev/null 2>&1; then
  echo "[verify-devtools-session] SKIP isolated start: ./dev.sh running" >&2
elif [[ "$(pgrep -fl 'script-kit-gpui' 2>/dev/null | grep -E 'script-kit-gpui($|[[:space:]])' | wc -l | tr -d '[:space:]')" -gt 1 ]]; then
  echo "[verify-devtools-session] SKIP isolated start: multiple GPUI instances" >&2
else
  SESSION="dt-smoke-$$"
  if json="$(bash scripts/agentic/devtools-session.sh start --session "$SESSION" --mode isolated --build never --ready-timeout-sec 60 --cleanup-on-fail --prove 2>/dev/null)"; then
    if printf '%s' "$json" | grep -q '"ready":true'; then
      pass "isolated start ready:true"
      owner_pid="$(printf '%s' "$json" | sed -nE 's/.*"ownership":\{"created":true,"pid":([0-9]+),"generation":"[^"]+"\}.*/\1/p')"
      owner_generation="$(printf '%s' "$json" | sed -nE 's/.*"ownership":\{"created":true,"pid":[0-9]+,"generation":"([^"]+)"\}.*/\1/p')"
      if [[ -n "$owner_pid" && -n "$owner_generation" ]]; then
        bash scripts/agentic/devtools-session.sh cleanup --session "$SESSION" \
          --expected-pid "$owner_pid" --expected-generation "$owner_generation" >/dev/null 2>&1 || true
      else
        fail "isolated start did not return exact cleanup ownership"
      fi
    else
      fail "start without ready:true: $json"
    fi
  else
    fail "isolated start failed"
  fi
fi

echo "[verify-devtools-session] === 6. reuse-dev-watch (optional) ===" >&2
if pgrep -f 'cargo-watch watch.*dev-cycle' >/dev/null 2>&1; then
  if json="$(bash scripts/agentic/devtools-session.sh start --mode reuse-dev-watch --session dev-watch --build never --ready-timeout-sec 10 2>/dev/null)"; then
    if printf '%s' "$json" | grep -q '"mode":"reuse-dev-watch"'; then
      pass "reuse-dev-watch attach"
    else
      fail "reuse-dev-watch: $json"
    fi
  else
  code=$?
  if [[ "$code" -eq 10 ]] || printf '%s' "$json" 2>/dev/null | grep -q dev_watch_unhealthy; then
    echo "[verify-devtools-session] SKIP reuse-dev-watch: dev-watch unhealthy (exit ${code})" >&2
  else
    fail "reuse-dev-watch start exit ${code}"
  fi
  fi
else
  echo "[verify-devtools-session] SKIP reuse-dev-watch: ./dev.sh not running" >&2
fi

exit "$FAIL"
