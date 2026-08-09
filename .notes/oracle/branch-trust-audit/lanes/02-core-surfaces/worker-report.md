# 02-core-surfaces worker report

## Candidate identity

- Branch: `consistency/default-recommendations`
- Candidate: `a3fc52549b4c1566bbfc585a79fa1c18478ddc86`
- Frozen main and merge base: `3775672d251cc8895583ed246e7600c10b723a94`
- Candidate identity was checked before review and matched the manager freeze.

## Scope accounting

- Exact owned scope: 128 paths, 13,122 insertions, 4,907 deletions.
- `path-accounting.tsv` contains exactly 128 unique rows and exactly the manager-owned path set.
- Every row has a subsystem, changed context, traced consumer description, evidence, review state, classification, severity, and confidence.
- Classification distribution: 16 `VERIFIED_IMPROVEMENT`, 5 `BREAKAGE_CANDIDATE`, 7 `PROOF_GAP`, 29 `PROOF_DEBT`, 55 `UNPROVEN_CLAIM`, and 16 `NO_MATERIAL_CHANGE` path rows. The high unproven/debt counts are deliberate: broad production/test churn was not promoted merely because it compiled or because a source-reading test existed.
- Supporting inventories: `actions-matrix.tsv`, `agent-chat-entry-matrix.tsv`, `footer-keyboard-parity.tsv`, source-audit delta, function-context index, and immutable owned diff.

## Verified improvements

1. **02-F001, C3:** `ActionsDialog::activate_selected` and direct `activate_action_id` reject disabled actions before callbacks. Eight focused GPUI/model tests passed, including callback non-execution.
2. **02-F005, C3:** Quick Question has an explicit `NoContext`, empty-seed, no-submit constructor policy. The corrected binary target ran 11 tests.
3. **02-F006, C3:** `ConversationTurnRenderKey` remains tied to the originating user message when an assistant reply appears. Five render-key and 78 chat tests passed.
4. **02-F009, C3:** Flow and Notes builders consume typed command descriptors for label, shortcut, availability, disabled reason, and handler selection. Seven corrected Flow tests and 15 Notes builder tests passed.

These are direct behavior/model results, not claims of native focus, temporal continuity, persistence, or AppKit correctness.

## Confirmed regressions

None. No deterministic product/runtime regression was reproduced in this lane.

## Breakage candidates

1. **02-F002, P1/C2 — Settings operation verb mismatch.** `SettingsAction::descriptor` assigns `primary_verb: "Open"` to every action, while destination operations include `clear`, `run-check`, `request`, and `reset`; the live Settings renderer formats that verb into its primary hint. Exact runtime state: select Clear Suggested Items, Check Permissions, Request Accessibility Access, or Reset Window Positions. Expected: truthful operation verb. Predicted wrong state: “Open.” Observed: `NOT_REPRODUCED`; manager runtime request R02-03 is required.
2. **02-F004, P1/C1 — Actions first-focus auto-close race.** Both activation observation and render can close when parent and popup are inactive, and render checks this before the deferred first focus request. This is an exact timing mechanism but not a reproduced flash-close. Manager runtime request R02-01 covers first open, nested route, exact host restoration, direct disabled activation, and 20 toggles.

## Unproven claims and proof gaps

- **02-F003, P0/C2:** Agent Chat entry completion is aggregate-count correlated. Submission is inferred from `message_count` growth and context from `context_chip_count` growth; the initiating `request_id` is copied into the outcome without proving that the observed mutation belongs to it. Duplicate context, unrelated message mutation, and delayed acceptance remain mandatory falsifiers.
- **02-F008, P1/C2:** Footer descriptor/grammar tests passed, but inactive first click, hit testing, listener cardinality, native click/keyboard parity, focus, and hidden-controls geometry were not crossed.
- **02-F010, P1/C2:** Current-view guards and stale-frame avoidance are visible statically, but no test switched surfaces while delayed filter/load/resize/handoff work completed.
- **02-F007, P2/C1:** The owned source-reader census is 138 sites, net +1 from main. The only focused test failure is a brittle exact-string audit expecting `upsert_runtime_window_handle`; production now calls generation-aware `upsert_runtime_window_handle_instance`. That red test is proof debt, not evidence of a runtime registration regression.
- General built-ins, startup/stdin routing, Day Page, confirms, HUDs, toasts, prompt variants, and launcher plumbing were diffed and consumer-traced, but were conservatively left C2 or below where the focused matrix did not cross their behavior boundary.

## Focused verification

- `agent-cargo.sh check --lib`: passed in 121.7 seconds; compile warnings remain and were not represented as UX proof.
- Nonzero focused tests passed:
  - Actions runtime paths: 8
  - Notes action builders: 15
  - Quick Question corrected binary target: 11
  - Conversation turn key: 5
  - Chat prompt: 78
  - Settings contract: 16
  - Flow discoverability corrected binary target: 7
  - Select prompt: 50
  - Main footer corrected binary target: 11
  - Locked glass Bun contracts: 40
  - Locked glass Rust fixture: 1
- Actions window filter: 28 tests ran; 27 passed and one exact-string source audit failed. The first receipt is preserved; no production edit or green-seeking rerun was performed.
- Three initial `--lib` filters and two attempted footer binary filters ran zero tests and are explicitly recorded as `ZERO_TESTS`, not green. One bounded binary test-list discovery identified the correct `ui_window::tests` filter.
- No command timed out. All Cargo commands used `./scripts/agentic/agent-cargo.sh`.
- Locked glass checks passed without fixture, threshold, geometry, timing, curve, alpha, or source changes.

## Runtime boundaries crossed

None. This lane crossed compile, pure/model, unit, and GPUI test boundaries up to C3. It did not start the app or claim C4.

## Runtime boundaries not crossed

- Actions first focus, nested Escape, parent focus restoration, automation/runtime handle registration, and rapid-toggle cleanup.
- Quick Question temporal no-context/no-submit behavior and deliberate selected-row entry with exactly one context.
- Request-correlated Agent Chat completion under duplicate context, unrelated mutation, and delayed acceptance.
- Native footer inactive first click, hit testing, listener cardinality, click/keyboard parity, and hidden-controls geometry.
- Settings visible/semantic/AX verb and Enter execution parity.
- Streaming selection/scroll continuity and Stop preservation.
- Stale async result/resize/handoff rejection after view changes.
- Notes Browse standalone/portal execution, confirmation exclusivity, and live screenshots.

Concrete sequences and stop conditions are in `runtime-requests.jsonl`.

## Screenshot requests

Nine privacy-safe, pinned-binary requests are in `screenshot-requests.jsonl`: Actions root; nested Actions route; Settings verbs; Notes Browse standalone/portal; active Flow Actions; clean Quick Question; streaming chat; disabled Select; and exclusive confirmation. Each request states what the frame supports and what it cannot prove. No screenshot was captured by this lane because no exact-candidate runtime was started.

## Prioritized next actions

1. **P0 — Correlate Agent Chat entry completion by request/turn/context identity (N05).** Stop only when duplicate context, unrelated mutation, delayed acceptance, fresh/reused thread, and real timeout produce identity-backed outcomes. Extending five seconds is forbidden.
2. **P1 — Run Actions lifecycle R02-01 (N08).** If flash-close or orphaning reproduces, future remediation should use explicit `Opening → FocusRequested → Active → Closing → Closed` phases with idempotent cleanup. Locked motion remains untouched.
3. **P1 — Run Settings parity R02-03 (N14).** Compare visible footer, getElements, AX, Enter, action ID, and actual operation for Open/Clear/Check/Request/Reset. A reproduced semantic mismatch is must-fix.
4. **P1 — Run native footer parity R02-04 (N06).** Require one inactive input event to yield one canonical action and one correct result, with no stale listener.
5. **P1 — Run stale-generation R02-06 (N15).** Switch views during delayed filter/load/resize/handoff work and require a structured stale-drop receipt before mutation.
6. **P2 — Replace the failing runtime-handle source-string audit (N16).** Move proof to actual registration/simulated dispatch; do not patch the expected helper spelling or add another source read.

## Integration requests

Eleven JSONL requests were emitted: five article findings, one integrated ledger entry, one screenshot batch, and four future-remediation cards. Every request carries candidate identity, evidence, confidence, target anchor, and limitations. No manager, integrated-ledger, article, screenshot-manifest, or publication file was edited.

## Production-write proof

- `99-production-write-proof.tsv` hashes all 128 owned production files from `HEAD` and the working tree: 128 matches, 0 mismatches.
- `99-owned-production-diff.patch` is empty.
- Only the lane report, lane integration requests, and `.artifacts/branch-trust-audit/02-core-surfaces/**` were written.
- Git index, production, other lanes, manager outputs, integrated outputs, article/publication outputs, fixtures, and glass calibration were not modified.

## Stop statement

Audit evidence and article inputs are complete for 02-core-surfaces; production files were not modified, and no commit, push, merge, application deployment, glass retune, or publication was performed.
