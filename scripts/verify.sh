#!/usr/bin/env bash
set -euo pipefail

SKIP_BUNDLE=0
ONLY=""
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  echo "usage: bash scripts/verify.sh [--skip-bundle] [--only <phase>]" >&2
  echo "phases: fmt check clippy test test-compile integration-tests domain-tests first-run-fixtures permissions-fixtures mock-ai-fixtures privacy-fixtures proof-contracts consistency-catalog sdk-types sdk-tests pi-sidecar bundle bundle-sidecar bundle-verify" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-bundle)
      SKIP_BUNDLE=1
      shift
      ;;
    --only)
      if [[ $# -lt 2 || -z "${2:-}" ]]; then
        usage
        exit 64
      fi
      ONLY="$2"
      shift 2
      ;;
    --only=*)
      ONLY="${1#--only=}"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 64
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
    echo "[verify] REFUSED unsafe setting ${unsafe_setting}=1; verification must remain nonintrusive" >&2
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

for worker_setting in CARGO_BUILD_JOBS RUST_TEST_THREADS; do
  worker_count="${!worker_setting:-2}"
  if [[ ! "$worker_count" =~ ^[12]$ ]]; then
    echo "[verify] REFUSED ${worker_setting}=${worker_count}; noninteractive verification permits only one or two workers" >&2
    exit 78
  fi
  export "${worker_setting}=${worker_count}"
done

require_clean_source_identity() {
  if [[ "${SCRIPT_KIT_REQUIRE_CLEAN_SOURCE:-0}" != "1" ]]; then
    return
  fi

  local head_sha
  head_sha="$(git -C "${REPO_ROOT}" rev-parse HEAD)"
  if [[ -n "${GITHUB_SHA:-}" && "${GITHUB_SHA}" != "${head_sha}" ]]; then
    echo "[verify] REFUSED source identity mismatch expected=${GITHUB_SHA} actual=${head_sha}" >&2
    exit 78
  fi

  # Canonical machine-readable release source inventory. The standalone
  # release-evidence gate parses this same bounded array before publishing.
  local -a required_source_owners=(
    .github/workflows/release.yml
    scripts/verify.sh
    scripts/verify-macos-bundle.sh
    scripts/verify-release-version.sh
    scripts/release-evidence.ts
    scripts/release-evidence.test.ts
    scripts/generate-surface-contracts.ts
    scripts/kit-sdk.ts
    scripts/test-runner.ts
    scripts/check-sdk-types.ts
    scripts/devtools/consistency-catalog.md
    scripts/devtools/consistency.ts
    scripts/devtools/consistency.test.ts
    scripts/devtools/surfaces.ts
    scripts/devtools/coverage.ts
    scripts/devtools/driver.ts
    scripts/devtools/elements.ts
    scripts/devtools/layout.ts
    scripts/devtools/lib/geometry-evidence.ts
    scripts/devtools/text.ts
    scripts/devtools/focus.ts
    scripts/devtools/scroll.ts
    scripts/devtools/surface.test.ts
    scripts/devtools/surfaces-bindings.test.ts
    scripts/devtools/actions-projection.test.ts
    scripts/devtools/elements.test.ts
    scripts/devtools/focus.test.ts
    scripts/devtools/layout.test.ts
    scripts/devtools/geometry-evidence.test.ts
    scripts/devtools/text.test.ts
    scripts/devtools/scroll.test.ts
    scripts/devtools/privacy.test.ts
    scripts/devtools/operator-safety.test.ts
    scripts/devtools/actions.ts
    scripts/devtools/agent_chat.ts
    scripts/devtools/dictation.ts
    scripts/devtools/events.ts
    scripts/devtools/main.ts
    scripts/devtools/notes-live-resize.ts
    scripts/devtools/notes-bottom-resize.ts
    scripts/devtools/notes-glass-entry-fallback.ts
    scripts/devtools/actions-entry-filmstrip.ts
    scripts/devtools/glass-lifecycle-filmstrip.ts
    scripts/devtools/rapid-toggle-stress.ts
    scripts/devtools/glass-observers.ts
    scripts/devtools/glass-interference.ts
    scripts/devtools/glass-motion-contrast.ts
    scripts/devtools/glass-native-helper-cache.ts
    scripts/devtools/spotlight-sync-filmstrip.ts
    scripts/devtools/main-window-native-drag.ts
    scripts/devtools/act.ts
    scripts/devtools/devtools.ts
    scripts/devtools/perf.ts
    scripts/devtools/capture-dom-fidelity.ts
    scripts/devtools/window-engine-foundation.ts
    scripts/devtools/inspect.ts
    scripts/devtools/notes.ts
    scripts/devtools/target-identity.test.ts
    scripts/devtools/__tests__/client-lib.test.ts
    scripts/devtools/receipt-output.test.ts
    scripts/devtools/receipt-schema.test.ts
    scripts/devtools/coverage.test.ts
    scripts/devtools/runtime-coverage.test.ts
    scripts/devtools/performance-contract.test.ts
    scripts/devtools/lib/client.ts
    scripts/devtools/lib/operator-safety.ts
    scripts/devtools/lib/target-identity.ts
    scripts/devtools/lib/privacy.ts
    scripts/devtools/lib/evidence-class.ts
    scripts/devtools/lib/task-proof-policy.ts
    scripts/devtools/lib/receipt-schema.ts
    scripts/devtools/lib/runtime-coverage.ts
    scripts/devtools/family-fixtures.ts
    scripts/devtools/family-fixtures.test.ts
    scripts/devtools/facade-ledger.ts
    scripts/devtools/facade-ledger.test.ts
    scripts/devtools/facade-migrations.ts
    scripts/devtools/facade-migrations.test.ts
    scripts/devtools/safe-task-proofs.ts
    scripts/devtools/safe-task-proofs.test.ts
    scripts/devtools/protected-sources.ts
    scripts/devtools/protected-sources.test.ts
    scripts/devtools/state-ownership.ts
    scripts/devtools/state-ownership.test.ts
    scripts/devtools/design-conflicts.ts
    scripts/devtools/design-conflicts.test.ts
    scripts/devtools/generated-byte-compare.ts
    scripts/devtools/generated-byte-compare.test.ts
    scripts/devtools/alpha-byte-contract-harness.rs
    scripts/devtools/alpha-byte-contract.test.ts
    scripts/devtools/glass-entry-motion-contract.test.ts
    scripts/devtools/glass-lifecycle-filmstrip.test.ts
    scripts/devtools/rapid-toggle-stress.test.ts
    scripts/devtools/test-status.ts
    scripts/agent-check.sh
    scripts/agentic/session.sh
    scripts/agentic/index.ts
    scripts/agentic/flow-composer-multiline-probe.ts
    scripts/agentic/cons-flow-ux/dictation-history-probe.ts
    scripts/agentic/cons-flow-ux/conversation-hosts-probe.ts
    scripts/agentic/cons-flow-ux/notes-actions-probe.ts
    scripts/agentic/cons-flow-ux/semantic-command-probe.ts
    scripts/agentic/cons-flow-ux/entry-verbs-probe.ts
    scripts/agentic/cons-flow-ux/dictation-recovery-focus-probe.ts
    scripts/agentic/cons-flow-ux/notes-search-probe.ts
    scripts/agentic/cons-flow-ux/dictation-delivery-probe.ts
    scripts/agentic/cons-flow-ux/context-preparation-probe.ts
    scripts/agentic/cons-flow-ux/notes-today-probe.ts
    scripts/agentic/cons-flow-ux/dictation-dismiss-targets-probe.ts
    scripts/agentic/cons-flow-ux/flow-history-probe.ts
    scripts/agentic/cons-flow-ux/notes-handoff-probe.ts
    scripts/agentic/cons-flow-ux/notes-agent-chat-return-probe.ts
    scripts/agentic/cons-flow-ux/context-lifecycle-probe.ts
    scripts/agentic/cons-flow-ux/final-workflow-audit.ts
    scripts/agentic/cons-flow-ux/final-workflow-audit.test.ts
    scripts/agentic/cons-proof-gov/story-geometry-proof.mjs
    scripts/agentic/cons-proof-gov/story-geometry-proof.test.ts
    scripts/agentic/cons-proof-gov/semantic-projection-proof.ts
    scripts/agentic/cons-proof-gov/layout-text-proof.ts
    scripts/agentic/cons-proof-gov/ax-scroll-proof.ts
    scripts/agentic/cons-proof-gov/proof-foundation-safety.test.ts
    scripts/agentic/root-search-visual-stability.ts
    scripts/agentic/glass-smoke-study.ts
    scripts/agentic/automation-window.ts
    scripts/agentic/verify-shot.ts
    scripts/agentic/window.ts
    scripts/agentic/macos-input.ts
    scripts/agentic/macos-input.test.ts
    scripts/agentic/filterable-surface-matrix.ts
    scripts/agentic/surface-navigator.ts
    scripts/agentic/surface-navigator-inventory-audit.ts
    scripts/agentic/target-thread.ts
    scripts/agentic/scenario.ts
    scripts/agentic/devtools-session-lib.sh
    scripts/agentic/start-isolated.sh
    scripts/agentic/devtools-session.sh
    scripts/agentic/wait-session-ready.sh
    scripts/agentic/agent-cargo.sh
    scripts/agentic/cargo-cache-locks.sh
    scripts/agentic/cargo-build-policy.test.ts
    scripts/agentic/reuse-rust-test-binary.sh
    scripts/agentic/build-isolated-binary.sh
    scripts/agentic/cargo-timings-summary.ts
    scripts/agentic/cargo-timings-summary.test.ts
    scripts/agentic/quick-ai-latency-bench.test.ts
    scripts/agentic/ai-phase-trace-report.test.ts
    scripts/agentic/root-typing-lag-benchmark.test.ts
    scripts/agentic/root-search-frame-stability.test.ts
    scripts/migrate/__tests__/classify.test.ts
    tests/ai_capability_preflight_contract.rs
    tests/legacy_design_variant_migration.rs
    tests/protocol_batch.rs
    tests/protocol_wait_for.rs
    tests/script_content_model.rs
    tests/window_resize_logic.rs
    tests/sdk/capability-types.fixture.ts
    tests/sdk/fixtures/runner-negative-case.ts
    tests/sdk/runner-safety.test.ts
    kit-init/sdk/menu-syntax.test.ts
    kit-init/types/menu-syntax.test.ts
    docs/ai/contracts/surface-contracts.json
    design/mockups/tests/story-browser-geometry-harness.mjs
    design/mockups/stories/stories.json
    design/mockups/stories/10-conversation-three-modes/story.js
    design/mockups/stories/11-launcher-flows-and-scripts/story.js
    design/mockups/generated/tokens.json
    design/mockups/generated/tokens.css
  )

  for required_artifact in "${required_source_owners[@]}"; do
    if ! git -C "${REPO_ROOT}" ls-files --error-unmatch "${required_artifact}" >/dev/null 2>&1; then
      echo "[verify] REFUSED untracked or missing release contract ${required_artifact}" >&2
      exit 78
    fi
  done

  if ! git -C "${REPO_ROOT}" diff --quiet HEAD --; then
    echo "[verify] REFUSED dirty tracked source; committed release evidence must match HEAD exactly" >&2
    exit 78
  fi
}

require_clean_source_identity

if [[ -n "${SCRIPT_KIT_CARGO:-}" ]]; then
  CARGO_CMD="$SCRIPT_KIT_CARGO"
elif [[ "${CI:-}" == "true" && "${GITHUB_ACTIONS:-}" == "true" ]]; then
  # Hosted runners already own isolated targets and do not satisfy the local
  # workstation's 25-GiB pool floor; their inherited worker counts stay bounded.
  CARGO_CMD="cargo"
else
  CARGO_CMD="${REPO_ROOT}/scripts/agentic/agent-cargo.sh"
fi

sanitize_id() {
  printf '%s' "$1" | tr -c 'a-zA-Z0-9._-' '-'
}

bundle_target_dir() {
  if [[ -n "${SCRIPT_KIT_BUNDLE_TARGET_DIR:-}" ]]; then
    printf '%s\n' "${SCRIPT_KIT_BUNDLE_TARGET_DIR}"
    return
  fi

  if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    printf '%s\n' "${CARGO_TARGET_DIR}"
    return
  fi

  if [[ "${CARGO_CMD}" == *"agent-cargo.sh"* ]]; then
    local target_mode="${SCRIPT_KIT_AGENT_TARGET_MODE:-pool}"
    case "${target_mode}" in
      pool)
        local pool
        pool="$(sanitize_id "${SCRIPT_KIT_CARGO_TARGET_POOL:-agent-debug}")"
        printf '%s\n' "${REPO_ROOT}/target-agent/pools/${pool}"
        return
        ;;
      exclusive)
        local agent_id
        agent_id="$(sanitize_id "${SCRIPT_KIT_AGENT_ID:-${USER:-agent}-${PPID:-$$}}")"
        printf '%s\n' "${REPO_ROOT}/target-agent/agents/${agent_id}"
        return
        ;;
    esac
  fi

  printf '%s\n' "${REPO_ROOT}/target"
}

BUNDLE_APP_PATH="${SCRIPT_KIT_BUNDLE_APP_PATH:-$(bundle_target_dir)/release/bundle/osx/Script Kit.app}"

run_step() {
  local name="$1"
  shift
  local test_log="${SCRIPT_KIT_VERIFY_TEST_LOG:-}"

  printf "\n[verify] RUN  %s :: %s\n" "$name" "$*"
  if [[ -n "${test_log}" ]]; then
    mkdir -p "$(dirname "${test_log}")"
  fi

  if [[ -n "${test_log}" ]]; then
    if "$@" 2>&1 | tee "${test_log}"; then
      printf "[verify] PASS %s\n" "$name"
    else
      local exit_code="${PIPESTATUS[0]}"
      printf "[verify] FAIL %s (exit %s)\n" "$name" "$exit_code" >&2
      exit "$exit_code"
    fi
  elif "$@"; then
    printf "[verify] PASS %s\n" "$name"
  else
    local exit_code=$?
    printf "[verify] FAIL %s (exit %s)\n" "$name" "$exit_code" >&2
    exit "$exit_code"
  fi
}

run_step_quiet() {
  local name="$1"
  shift

  printf "\n[verify] RUN  %s :: %s\n" "$name" "$*"
  if "$@" >/dev/null; then
    printf "[verify] PASS %s\n" "$name"
  else
    local exit_code=$?
    printf "[verify] FAIL %s (exit %s)\n" "$name" "$exit_code" >&2
    exit "$exit_code"
  fi
}

run_sdk_tests() {
  local receipt_path="${SCRIPT_KIT_SDK_TEST_RECEIPT:-}"

  if [[ -z "${receipt_path}" ]]; then
    bun run scripts/test-runner.ts --parallel
    return
  fi

  mkdir -p "$(dirname "${receipt_path}")"
  bun run scripts/test-runner.ts --parallel --json > "${receipt_path}"
  bun "${REPO_ROOT}/scripts/release-evidence.ts" sdk-summary --result "${receipt_path}"
}

write_gate_evidence() {
  local phase="$1"
  local receipt_path="${SCRIPT_KIT_VERIFY_RECEIPT:-}"
  local evidence_class
  local source_sha
  local gate_id
  local diagnostic_owner
  local result_args=()
  local provenance_args=()
  local diagnostic_owners=()

  if [[ -z "${receipt_path}" ]]; then
    return
  fi

  case "${phase}" in
    test)
      gate_id="rust-tests"
      evidence_class="UNIT_BEHAVIOR"
      if [[ -n "${SCRIPT_KIT_VERIFY_TEST_LOG:-}" ]]; then
        result_args=(--result "${SCRIPT_KIT_VERIFY_TEST_LOG}")
      fi
      ;;
    integration-tests|domain-tests|first-run-fixtures|permissions-fixtures|mock-ai-fixtures|privacy-fixtures)
      gate_id="${phase}"
      evidence_class="UNIT_BEHAVIOR"
      if [[ -n "${SCRIPT_KIT_VERIFY_TEST_LOG:-}" ]]; then
        result_args=(--result "${SCRIPT_KIT_VERIFY_TEST_LOG}")
      fi
      ;;
    proof-contracts)
      gate_id="${phase}"
      evidence_class="UNIT_BEHAVIOR"
      if [[ -n "${SCRIPT_KIT_VERIFY_TEST_LOG:-}" ]]; then
        result_args=(--result "${SCRIPT_KIT_VERIFY_TEST_LOG}")
      fi
      ;;
    consistency-catalog)
      gate_id="consistency-catalog"
      evidence_class="STATIC_INVENTORY"
      ;;
    sdk-tests)
      gate_id="sdk-tests"
      evidence_class="SDK_BEHAVIOR"
      if [[ -n "${SCRIPT_KIT_SDK_TEST_RECEIPT:-}" ]]; then
        result_args=(--result "${SCRIPT_KIT_SDK_TEST_RECEIPT}")
      fi
      ;;
    *)
      echo "[verify] REFUSED release evidence for non-behavior phase '${phase}'" >&2
      exit 64
      ;;
  esac

  if [[ "${SCRIPT_KIT_ALLOW_DIRTY_DIAGNOSTIC_EVIDENCE:-0}" == "1" ]]; then
    provenance_args+=(--diagnostic-dirty)
    if [[ -n "${SCRIPT_KIT_DIRTY_EVIDENCE_OWNER_PATHS:-}" ]]; then
      IFS=':' read -r -a diagnostic_owners <<< "${SCRIPT_KIT_DIRTY_EVIDENCE_OWNER_PATHS}"
      for diagnostic_owner in "${diagnostic_owners[@]}"; do
        provenance_args+=(--owner "${diagnostic_owner}")
      done
    fi
  fi

  source_sha="${GITHUB_SHA:-$(git -C "${REPO_ROOT}" rev-parse HEAD)}"
  bun "${REPO_ROOT}/scripts/release-evidence.ts" gate \
    --gate "${gate_id}" \
    --class "${evidence_class}" \
    --source-sha "${source_sha}" \
    --output "${receipt_path}" \
    "${result_args[@]}" \
    "${provenance_args[@]}"
}

run_phase() {
  local phase="$1"

  case "$phase" in
    fmt)
      run_step "fmt" "$CARGO_CMD" fmt --check
      ;;
    check)
      run_step "check" "$CARGO_CMD" check --locked
      ;;
    clippy)
      run_step "clippy" "$CARGO_CMD" clippy --locked --lib --no-deps -- -D warnings
      ;;
    test)
      run_step "test" "$CARGO_CMD" test --locked --lib
      ;;
    test-compile)
      run_step "test-compile" "$CARGO_CMD" test --no-run --locked --lib
      ;;
    domain-tests)
      run_step "domain-tests" "$CARGO_CMD" test --locked -p sk-clipboard -p sk-protocol -p sk-storage
      ;;
    integration-tests)
      run_step "integration-tests" "$CARGO_CMD" test --locked \
        --test ai_capability_preflight_contract \
        --test legacy_design_variant_migration \
        --test protocol_batch \
        --test protocol_wait_for \
        --test script_content_model \
        --test window_resize_logic
      ;;
    first-run-fixtures)
      run_step "first-run-fixtures" "$CARGO_CMD" test --locked --lib \
        setup::tests::test_fresh_install_seeds_canonical_menu_syntax_handlers -- --exact
      ;;
    permissions-fixtures)
      run_step "permissions-fixtures" "$CARGO_CMD" test --locked --lib \
        permissions_wizard::tests::test_snapshot_missing_required -- --exact
      ;;
    mock-ai-fixtures)
      run_step "mock-ai-fixtures" "$CARGO_CMD" test --locked -p sk-protocol \
        ai_reliability::model_tests::blocked_capability_produces_actionable_recovery_without_starting \
        -- --exact
      ;;
    privacy-fixtures)
      run_step "privacy-fixtures" "$CARGO_CMD" test --locked --lib \
        ai::reliability::tests::redactor_allowlists_json_masks_secrets_paths_and_bounds_output \
        -- --exact
      ;;
    proof-contracts)
      run_step "generated-surface-contracts" bun scripts/generate-surface-contracts.ts --check
      run_step_quiet "consistency-family-fixtures" bun scripts/devtools/family-fixtures.ts
      run_step "proof-contracts" bun test --timeout 30000 \
        ./scripts/release-evidence.test.ts \
        ./scripts/devtools/consistency.test.ts \
        ./scripts/devtools/surface.test.ts \
        ./scripts/devtools/surfaces-bindings.test.ts \
        ./scripts/devtools/actions-projection.test.ts \
        ./scripts/devtools/elements.test.ts \
        ./scripts/devtools/focus.test.ts \
        ./scripts/devtools/layout.test.ts \
        ./scripts/devtools/geometry-evidence.test.ts \
        ./scripts/devtools/text.test.ts \
        ./scripts/devtools/scroll.test.ts \
        ./scripts/devtools/privacy.test.ts \
        ./scripts/devtools/operator-safety.test.ts \
        ./scripts/devtools/target-identity.test.ts \
        ./scripts/devtools/__tests__/client-lib.test.ts \
        ./scripts/devtools/receipt-output.test.ts \
        ./scripts/devtools/receipt-schema.test.ts \
        ./scripts/devtools/coverage.test.ts \
        ./scripts/devtools/runtime-coverage.test.ts \
        ./scripts/devtools/performance-contract.test.ts \
        ./scripts/devtools/facade-ledger.test.ts \
        ./scripts/devtools/facade-migrations.test.ts \
        ./scripts/devtools/protected-sources.test.ts \
        ./scripts/devtools/state-ownership.test.ts \
        ./scripts/devtools/alpha-byte-contract.test.ts \
        ./scripts/devtools/generated-byte-compare.test.ts \
        ./scripts/devtools/design-conflicts.test.ts \
        ./scripts/devtools/safe-task-proofs.test.ts \
        ./scripts/devtools/family-fixtures.test.ts \
        ./scripts/devtools/glass-entry-motion-contract.test.ts \
        ./scripts/devtools/glass-lifecycle-filmstrip.test.ts \
        ./scripts/devtools/rapid-toggle-stress.test.ts \
        ./scripts/agentic/cargo-build-policy.test.ts \
        ./scripts/agentic/macos-input.test.ts \
        ./scripts/agentic/cons-flow-ux/final-workflow-audit.test.ts \
        ./scripts/agentic/cons-proof-gov/story-geometry-proof.test.ts \
        ./scripts/agentic/cons-proof-gov/proof-foundation-safety.test.ts \
        ./scripts/agentic/cargo-timings-summary.test.ts \
        ./scripts/agentic/quick-ai-latency-bench.test.ts \
        ./scripts/agentic/ai-phase-trace-report.test.ts \
        ./scripts/agentic/root-typing-lag-benchmark.test.ts \
        ./scripts/agentic/root-search-frame-stability.test.ts \
        ./scripts/migrate/__tests__/classify.test.ts \
        ./tests/sdk/runner-safety.test.ts
      ;;
    consistency-catalog)
      run_step "consistency-catalog" bun scripts/devtools/consistency.ts \
        catalog --fixes scripts/devtools/consistency-catalog.md
      ;;
    sdk-types)
      run_step "sdk-types" bun run scripts/check-sdk-types.ts
      run_step "sdk-capability-type-fixtures" ./node_modules/.bin/tsc \
        --noEmit --lib ES2022 --target ES2022 --types node \
        --moduleResolution bundler --module ES2022 --skipLibCheck \
        tests/sdk/capability-types.fixture.ts \
        kit-init/sdk/menu-syntax.test.ts \
        kit-init/types/menu-syntax.test.ts
      ;;
    sdk-tests)
      run_step "sdk-tests" run_sdk_tests
      ;;
    pi-sidecar)
      run_step "pi-sidecar" bash scripts/prepare-pi-sidecar.sh
      ;;
    bundle)
      run_step_quiet "bundle-lock" "$CARGO_CMD" metadata --locked --format-version=1 --no-deps
      run_step "bundle" "$CARGO_CMD" bundle --release --bin script-kit-gpui
      ;;
    bundle-sidecar)
      run_step "bundle-sidecar" bash scripts/install-pi-sidecar-into-bundle.sh "${BUNDLE_APP_PATH}"
      ;;
    bundle-verify)
      run_step "bundle-verify" bash scripts/verify-macos-bundle.sh "${BUNDLE_APP_PATH}"
      ;;
    *)
      echo "unknown verify phase: $phase" >&2
      usage
      exit 64
      ;;
  esac

  write_gate_evidence "${phase}"
}

if [[ -n "$ONLY" ]]; then
  case "$ONLY" in
    fmt|check|clippy|test|test-compile|integration-tests|domain-tests|first-run-fixtures|permissions-fixtures|mock-ai-fixtures|privacy-fixtures|proof-contracts|consistency-catalog|sdk-types|sdk-tests)
      run_phase "$ONLY"
      printf "\n[verify] COMPLETE skip_bundle=%s only=%s\n" "$SKIP_BUNDLE" "$ONLY"
      exit 0
      ;;
    pi-sidecar|bundle|bundle-sidecar|bundle-verify)
      if [[ "$SKIP_BUNDLE" -eq 1 ]]; then
        echo "verify phase '$ONLY' is disabled by --skip-bundle" >&2
        exit 64
      fi
      run_phase "$ONLY"
      printf "\n[verify] COMPLETE skip_bundle=%s only=%s\n" "$SKIP_BUNDLE" "$ONLY"
      exit 0
      ;;
    *)
      echo "unknown verify phase: $ONLY" >&2
      usage
      exit 64
      ;;
  esac
fi

run_phase "fmt"
run_phase "check"
run_phase "clippy"
run_phase "test"
run_phase "integration-tests"
run_phase "domain-tests"
run_phase "first-run-fixtures"
run_phase "permissions-fixtures"
run_phase "mock-ai-fixtures"
run_phase "privacy-fixtures"
run_phase "proof-contracts"
run_phase "consistency-catalog"
run_phase "sdk-types"
run_phase "sdk-tests"

if [[ "$SKIP_BUNDLE" -eq 0 ]]; then
  run_phase "pi-sidecar"
  run_phase "bundle"
  run_phase "bundle-sidecar"
  run_phase "bundle-verify"
fi

printf "\n[verify] COMPLETE skip_bundle=%s\n" "$SKIP_BUNDLE"
