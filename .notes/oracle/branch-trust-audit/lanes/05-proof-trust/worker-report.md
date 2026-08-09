# 05-proof-trust worker report

## Candidate identity

- Branch: `consistency/default-recommendations`
- Base/main: `3775672d251cc8895583ed246e7600c10b723a94`
- Candidate: `a3fc52549b4c1566bbfc585a79fa1c18478ddc86`
- Frozen lane boundary: 160 paths, +29,425/−1,209.
- Candidate identity and lane statistics matched the manager brief before audit execution.

## Scope accounting

All 160 owned paths are present exactly once in `.artifacts/branch-trust-audit/05-proof-trust/path-accounting.tsv`: 34 checked-in receipts, 7 notes/ledgers, 1 generated feature map, 16 Flow/runtime probes, 8 proof/governance probes, 11 other runtime helpers, 11 fixtures, 42 DevTools files, 1 owner-map validator, 27 Rust proof contracts, and 2 explicit source-audit files. The two Notes Browse test paths deleted by the candidate are accounted as deleted, not missing. No row remains TODO, UNKNOWN, or UNREVIEWED.

## Verified improvements

`05-F008` is the bounded verified improvement. The new owner-map validator passes its self-test and validates `GLOSSARY.md`; the rendered capsule geometry helper passes three isolated tests. This is direct executable proof of those helpers, not proof of live UI geometry or routing.

## Confirmed regressions

- `05-F001` (P0/C3): `verify-task GOV-002` accepted a current-registry synthetic receipt stored under `GOV-002` even though its embedded task ID was `NOT-GOV-002`, its producer and command were unregistered, and all evidence layers were absent. The aggregate exited 0 as `EVALUABLE_PASS`.
- `05-F002` (P0/C3): `verify-all` accepted 75 similarly forged task receipts plus self-asserted program metadata. It reported `passedTaskCount: 75`, every task `EVALUABLE_PASS`, and exited 0.
- `05-F003` (P0/C3): `verify-family main-menu` accepted `does/not/exist.json` as its sole member because the implementation counts declared strings without resolving or validating them.
- `05-F004` (P1/C3): the execution-plan coverage-bindings command exits 4 because `devtools.coverage.bindings` is not registered in the receipt schema.
- `05-F005` (P1/C1): two branch-owned source-contract targets fail four assertions: Agent Chat entry contract 1 passed/2 failed; submit ownership contract 9 passed/2 failed. These are confirmed proof-suite regressions, but their source-reading nature caps runtime confidence at C1.

## Breakage candidates

No separate `BREAKAGE_CANDIDATE` was emitted. The adversarial admission failures and failing proof contracts reproduced deterministically and were therefore classified as confirmed regressions; unexecuted runtime mechanisms remain proof gaps rather than predictions presented as observations.

## Unproven claims and proof gaps

- `05-F006` (UNPROVEN_CLAIM/C0): all 34 checked-in Flow receipts are historical claims for this candidate. None binds the current candidate; none declares a primitive, tool, or producer validation; none references a currently existing binary; none carries fixture or evidence-layer identity. A normalized duplicate cluster contains 28 final-audit wrappers. This does not assert that the historical runs were fabricated; it means they are inadmissible as current proof.
- `05-F007` (PROOF_DEBT/C1): 22 owned Rust test files contain 239 source-reader sites. The shrink-only ratchet passes, while two Notes Browse source-audit files were deleted without a lane-proven higher-rung replacement and two other source-contract binaries currently fail.
- `05-F009` (PROOF_GAP/C1): 156 Bun unit tests pass across proof-core and DevTools primitives, but this lane did not launch the app. Native focus, lifecycle, pixels, temporal behavior, current target identity, and cleanup remain manager-owned runtime boundaries.

## Focused verification

The complete matrix is `.artifacts/branch-trust-audit/05-proof-trust/verification.tsv` with immutable command metadata and stdout/stderr receipts.

- Passed: owner-map self-test; GLOSSARY owner-map validation; 96 trust-core Bun tests; 60 DevTools primitive Bun tests; 75-task catalog; 3 rendered-geometry tests; source-audit ratchet; 7 Dictation lifecycle pure behavior tests.
- Failed as product/proof checks: coverage-bindings receipt registration; Agent Chat entry source contract; submit ownership source contract.
- Failed as adversarial falsifiers, meaning the verifier false-greened: forged task, forged 75-task program, and forged family member.
- Superseded: the first forged-task attempt used registry version 2 and was rejected as stale; the unchanged control rerun with current registry version 1 was accepted. Both receipts are preserved so the decisive result is not copy-greened.

## Runtime boundaries crossed

The lane crossed direct executable proof-tool boundaries (Bun/Python/Cargo test processes) and the current receipt-admission CLI boundary. This supports C3 for the false-green admission findings and bounded helper behavior.

## Runtime boundaries not crossed

No Script Kit app process, GPUI renderer, AppKit window, external application, clipboard, filesystem persistence scenario, pixel capture, keyboard routing, focus lifecycle, or temporal glass boundary was exercised. No `INVALID_INTERFERENCE` receipt arose. No runtime screenshot is claimed.

## Screenshot requests

`S05-01` requests one sanitized receipt-lineage matrix. It must show current-candidate, producer, primitive, binary, fixture, evidence-layer, duplicate-cluster, and forged-control disposition facts. It is an evidence graphic, not a decorative application screenshot, and it must explicitly state that it proves no product UI behavior.

## Prioritized next actions

1. `N02` P0: make receipt admission reject unknown producers, wrong directory/task binding, missing current source identity, and absent evidence layers. Stop only when both forged task and forged 75-task controls return nonzero typed admission errors while one valid current receipt remains green.
2. `N03` P0: resolve and validate every family member receipt, enforce repository-safe paths/current disposition, and register `devtools.coverage.bindings`. Stop only when a missing/stale/failed/wrong-family member blocks the family and the real nine-family census remains valid.
3. `N16` P1/P2: repair the four failing proof-contract assertions by either restoring the intended architecture or replacing brittle source locks with higher-rung behavior/runtime proof; name replacements for the deleted Notes Browse audits. Do not patch strings merely to make source tests green.
4. `N18` P2: manager-run the current candidate binary for the user-visible claims and bind every receipt/screenshot to candidate, binary, target, fixture, generation, interference, and cleanup identity.

## Integration requests

Ten typed requests are in `.notes/oracle/branch-trust-audit/lanes/05-proof-trust/integration-requests.jsonl`: five finding/ledger inputs, one screenshot request, and four future-remediation inputs. The primary integration disposition is merge-blocking proof admission, not a claim that all product work is broken.

## Production-write proof

`.artifacts/branch-trust-audit/05-proof-trust/receipts/production-write-proof.tsv` compares all 160 owned candidate paths to `AUDIT_HEAD`: 158 present paths match byte-for-byte and both candidate-deleted paths remain absent. `git status --short` was empty after artifact generation. No production path or git index entry was modified.

## Stop statement

Audit evidence and article inputs are complete for 05-proof-trust; production files were not modified, and no commit, push, merge, application deployment, glass retune, or publication was performed.
