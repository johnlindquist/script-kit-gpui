# 01-shared-ui worker report

## Candidate identity

- Branch: `consistency/default-recommendations`
- Candidate: `a3fc52549b4c1566bbfc585a79fa1c18478ddc86`
- Frozen base/main: `3775672d251cc8895583ed246e7600c10b723a94`
- Identity and package totals were rechecked before evidence collection.

## Scope accounting

- Accounted: 87/87 owned paths, exactly once.
- Diff totals: +14,460 / -3,219.
- Machine ledger: `.artifacts/branch-trust-audit/01-shared-ui/path-accounting.tsv`.
- Reviewed groups: consistency explorer and generator; generated design tokens/contracts; theme/chrome and row geometry; semantic/layout collectors; shared inputs/forms/buttons/shortcut/toast primitives; popup lifecycle; semantic states/conversation/Notes primitives; four vendored GPUI-component paths.
- No path is empty, TODO, UNKNOWN, UNREVIEWED, or outside the package ownership list.

## Verified improvements

1. **01-F002, C3:** shared row state precedence, typed alpha quantization, geometry roles, and launcher marker ownership are covered by 284 theme tests, 60 list-item tests, and 2 design-contract tests. Real pointer-parked keyboard behavior remains outside this proof.
2. **01-F003, C3:** semantic projection outcomes now type Complete/Partial/Unsupported and reject fabricated completeness in three binary-target production tests; the debug grid consumes the canonical layout model.
3. **01-F004, C3:** stable button identity, form shell states, shortcut feedback, and toast generation/focus behavior have focused direct test coverage.
4. **01-F005, C3:** the shared popup lifecycle rejects open-before-attach and stale callbacks in direct tests. Native AppKit attachment and focus return remain unproven.
5. **01-F006, C3:** closed conversation command descriptors and semantic state anatomy have direct behavior and binary semantic-ID coverage.
6. **01-F009, C2:** the browser consistency explorer loads 12 groups and 75 scenes with neutral truth labels and unset local decisions.

## Confirmed regressions

- **01-F001, P1, C2:** isolated regeneration of `design/consistency/data/groups.json` from the current checked-in generator and ledger exits successfully but is not byte-identical to the checked-in manifest. Three fields are stale: WF-002 owners, GOV-003 status, and GOV-005 status. The structural and browser suites do not detect this generated-parity failure.

## Breakage candidates

- None. No predicted product failure was represented as observed.

## Unproven claims and proof gaps

- **01-F007, P1, C1:** the package-plan `--lib app_layout_projection_tests` and `--lib prompt_and_script_list_collectors` commands both exited zero while running zero tests. The corrected binary-target command runs three generic outcome tests, but no direct prompt-collector test module was found. Live canary privacy, duplicate IDs, selected/focused visibility, truncation totals, and renderer parity remain manager runtime work.
- **01-F008, P2, C1:** `gpui-component` resolves and all 130 package tests pass, including changed input and notification cases, but no direct vendor Button Enter/Space regression test was identified for the new broad keyboard handler. No incorrect activation was reproduced.
- The prescribed direct Bun invocation for `validate-explorer.mjs` fails because the file uses `node:test`; Bun test discovery also misses its filename. `node --test` passes all six tests. The failed first attempts are preserved.
- Locked glass production owners are outside this lane. No lane-owned locked value was changed. Three glass proof scripts changed elsewhere in the branch, so the manager must still run the locked anti-drift suite without changing expectations.

## Focused verification

- 23 command/receipt rows are recorded in `.artifacts/branch-trust-audit/01-shared-ui/verification.tsv`.
- Passing focused boundaries include: Node explorer validation (6); browser explorer smoke (12 groups/75 scenes); story geometry harness; design contract (2); theme (284); corrected binary projection (3); list item (60); inline dropdown (20); form fields (27); shortcut recorder (16); inline popup (10); InfoState (17); binary InfoState semantics (3); conversation actions (14); toast (8); and resolved `gpui-component` package (130).
- Failed/non-admissible checks are preserved: two invalid explorer invocations, two zero-test collector filters, and isolated generator parity.
- All Cargo commands used `./scripts/agentic/agent-cargo.sh` with bounded timeouts.

## Runtime boundaries crossed

- Browser runtime: all 12 explorer groups and 75 task scenes loaded successfully.
- Direct GPUI/Rust behavior: component, theme, row, popup state-machine, semantic-state, toast, and vendor package tests.
- Generated boundary: isolated consistency-manifest regeneration decisively exposed stale checked-in output.

## Runtime boundaries not crossed

- Real app live `getElements`/MCP privacy canaries and renderer/collector parity.
- Pointer-parked keyboard navigation through the real dispatch path.
- AppKit popup attach-before-show, one-layer close, and exact focus restoration.
- Cross-host conversation command renderer-to-handler parity.
- Accessibility traversal and host focus restoration for forms/shortcuts/toasts.
- Locked glass runtime and anti-drift probes; manager owns these because the changed proof scripts are outside lane scope.
- No screenshots were captured by this worker; six manager screenshot requests were emitted.

## Screenshot requests

Six exact requests are in `.artifacts/branch-trust-audit/01-shared-ui/screenshot-requests.jsonl`:

- S01 shared row family across Main, Actions, and compact dropdown.
- S02 keyboard selection with the pointer parked on another row.
- S03 shared semantic state anatomy paired with live collector privacy evidence.
- S04 shared form/shortcut states paired with disabled-activation evidence.
- S05 attached popup over its exact parent paired with native lifecycle evidence.
- S06 conversation command availability plus separately labeled explorer truth states.

## Prioritized next actions

1. **N18 / P1:** regenerate in isolation, reconcile the three stale manifest fields, and require byte parity in the publication evidence path. Stop only at `byteEqual=true` without rewriting evidence during this audit.
2. **N04 / P1:** run live private-canary `getElements`/MCP coverage and add direct prompt/list collector behavior tests. Stop at unique IDs, truthful total/truncation/quality, visible selected/focused controls, and zero raw canary bytes.
3. **N08 / P1:** execute native popup attach/show/close/focus receipts and caller generation checks. Stop at hidden-before-attach, exact parent relation, one-layer close, and exact focus return.
4. **N17 / P2:** add direct real-dispatch Enter/Space coverage for the vendored Button patch and record upstream/local lineage. Stop at narrow proven behavior across disabled/loading/focused states.
5. **N15 / P2:** run the pointer-parked keyboard scenario and stale-callback generation matrix across shared rows and controls.
6. **N07 / P0 guard:** manager must run the immutable glass anti-drift matrix because three proof scripts changed outside this lane; restore locked values on any drift and never retune.

## Integration requests

- 11 machine-readable requests are in `.notes/oracle/branch-trust-audit/lanes/01-shared-ui/integration-requests.jsonl`.
- They include the stale-manifest regression, verified shared-UI improvements, collector and vendor proof gaps, six screenshot requests, and the manager-owned glass guard.

## Production-write proof

- `.artifacts/branch-trust-audit/01-shared-ui/receipts/99-production-write-proof.json` compares all 87 owned current bytes with the frozen candidate and reports zero mismatches.
- Only lane-owned report, integration-request, and artifact paths were written.
- Git index, production sources, other lanes, manager/integrated/article/publication paths, and locked glass values were not modified.

## Stop statement

Audit evidence and article inputs are complete for 01-shared-ui; production files were not modified, and no commit, push, merge, application deployment, glass retune, or publication was performed.
