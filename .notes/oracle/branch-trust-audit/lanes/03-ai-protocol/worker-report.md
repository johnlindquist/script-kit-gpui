# 03-ai-protocol worker report

## Candidate identity

- Candidate: `a3fc52549b4c1566bbfc585a79fa1c18478ddc86`
- Frozen merge base: `3775672d251cc8895583ed246e7600c10b723a94`
- Lane scope: 53 paths, +5,421/−2,162.
- Candidate identity and lane statistics matched the manager brief before review.

## Scope accounting

All 53 owned paths are recorded exactly once in `.artifacts/branch-trust-audit/03-ai-protocol/path-accounting.tsv`; no row is unreviewed, unknown, or outside the owned list. The review traced the domain reducer, app reliability boundary, Agent Chat thread/view/history, staged context and preflight persistence, protocol constructors/types, and focused tests. No production path was modified.

## Verified improvements

1. **03-F001, C3:** Explicit Agent Chat Stop now reaches `UserStopped` cancellation through `RuntimeStopped`, preserving partial-output identity instead of presenting a provider failure. Domain and thread behavior filters passed.
2. **03-F002, C3:** Preflight JSON/SQLite no longer persists raw/authored prompt bytes, context failures carry safe typed source information, and protocol elements support redacted descriptors plus explicit projection quality/reasons. Focused privacy/protocol tests passed.
3. **03-F003, C3:** Staged context now has explicit provenance, primary/supplemental role, lifecycle, generation, and immutable receipt state. Production thread tests passed for accepted-start commit, unaccepted-send restoration, saved-history cleanup, and Quick AI policy guards.
4. Popup automation cleanup is generation-scoped and live element call sites use the explicit projection constructor. These remain C2/C3 structure and behavior improvements until native/protocol runtime receipts exist.

## Confirmed regressions

1. **03-F004, C2, P1:** `AgentChatHistoryPopupSnapshot::selected_index` indexes logical entries, but the virtualized list includes date headers. `navigate`, `jump_to_boundary`, and `page_navigate` pass the logical ordinal directly to `scroll_to_item`. With two headers before logical entry 3, visual row 6 is required while row 3 is requested. This is a logically forced scroll-target defect; the visible symptom still needs GPUI runtime confirmation.
2. **03-F007, C3, P2 proof regression:** The changed context-contract integration test ran and failed. The production resolver now XML-escapes query-string ampersands, while the test still expects the raw URI. This is stale proof against safer production output, not evidence that context resolution itself regressed.

## Breakage candidates

- **03-F005, C1, P1:** Two same-source `TextBlock` snapshots with different content and equal role/provenance hash to the same staged identity. The second takes the `Duplicate` path without replacing content. A live caller reproduction is required before promoting the predicted silent context loss.

## Unproven claims and proof gaps

- **03-F006:** `stable_draft_fingerprint` records only raw/authored byte lengths. It is privacy-safe but cannot distinguish equal-length drafts. No current send/freshness decision was found to consume it, so this is an identity claim gap rather than a reproduced product failure.
- **03-F008:** Two new deletion tests name file/index deletion and idempotence but never call production `delete_conversation`; they only serialize and filter local values.
- **03-F009:** Generation guards for popup close/reopen are structurally present, but AppKit close ordering, focus restoration, and registry convergence were not exercised.
- **03-F010:** Constructor and serde tests prove redaction/projection vocabulary, not that a live Agent Chat renderer and `getElements` collector agree. No real protocol boundary was crossed.
- The planned domain-tree command was invalid as written. Its one corrected attempt timed out waiting for the shared `agent-debug` lock, so dependency-direction verification is blocked rather than green.
- The pre-existing `getAgentChatState.input_text` cleartext field remains outside the material branch change; this lane does not claim that broader state snapshot is private.

## Focused verification

- Passed: owned diff check; sk-protocol reliability (18 tests); app reliability (28); message parts (40); staged context (4); preflight (4); Agent Chat thread (94); history popup (6); getElements (24); automation window (40); wait/protocol (46); source-audit ratchet (1); `check --lib`.
- Failed: context contract integration (1 test), recorded as 03-F007.
- Command error: planned `cargo tree --edges normal,no-dev` is rejected by Cargo.
- Blocked corrected attempt: `cargo tree --edges normal` timed out after 120 seconds waiting for the shared wrapper lock.
- Skipped: worker runtime build. The integrated plan assigns one exact-candidate binary build and screenshot/runtime matrix to the manager.
- Full commands, timeouts, exit codes, test counts, and receipt paths are in `.artifacts/branch-trust-audit/03-ai-protocol/verification.tsv`.

## Runtime boundaries crossed

No C4 runtime, native-window, filesystem-retention, or live protocol boundary was crossed in this worker lane. Direct reducer/thread behavior tests reached C3. Static call/type traces reached C2.

## Runtime boundaries not crossed

Concrete manager requests are in `.artifacts/branch-trust-audit/03-ai-protocol/runtime-requests.jsonl`:

1. Typed usage-limit/upgrade fact → safe recovery card → live semantic action.
2. User Stop after distinctive UTF-8 partial output.
3. Live Agent Chat `getElements` privacy/truth matrix across ready, streaming, stopped, failure, setup, permission, context-failure, and mismatch states.
4. Four-bucket history popup navigation plus rapid close/reopen and focus/registry cleanup.
5. Quick AI zero-retention filesystem/SQLite/Brain/day-trace/notification/export check.

## Screenshot requests

Five exact-candidate requests are in `.artifacts/branch-trust-audit/03-ai-protocol/screenshot-requests.jsonl`: typed recovery, quiet Stop, context lifecycle, sectioned history navigation, and model mismatch. Every request names the required runtime sidecars and explicitly limits what a frame can prove.

## Prioritized next actions

1. **P1:** Convert history logical entry indices to visual list indices before every scroll operation; add direct section-boundary behavior coverage and a live keyboard receipt.
2. **P1:** Define same-source changing-context semantics—replacement or accumulation—and add the missing negative control before relying on staged identity.
3. **P1:** Run the live typed failure/Stop/getElements matrix and reject any raw diagnostic, missing visible action, or enabled action without callback.
4. **P1/P2:** Replace the length-only preflight “fingerprint” if it will control freshness; otherwise rename/document it as size metadata.
5. **P2:** Update the context-contract behavior assertion to validate XML-escaped attributes semantically.
6. **P2:** Replace deletion simulations/source assertions with a path-injected production deletion behavior test.
7. **P1:** Run the Quick AI zero-retention before/after filesystem and database check.

## Integration requests

`.notes/oracle/branch-trust-audit/lanes/03-ai-protocol/integration-requests.jsonl` contains article findings, ledger entries, five screenshot requests, and future-remediation requests mapped to N04, N09, N10, and N16. Worker classifications are confidence-limited and are not manager admission decisions.

## Production-write proof

`.artifacts/branch-trust-audit/03-ai-protocol/receipts/99-production-write-proof.json` records 53 owned production paths checked and zero modified. Only lane-owned report/integration files and `.artifacts/branch-trust-audit/03-ai-protocol/**` were written. No git index operation was performed.

## Stop statement

Audit evidence and article inputs are complete for 03-ai-protocol; production files were not modified, and no commit, push, merge, application deployment, glass retune, or publication was performed.
