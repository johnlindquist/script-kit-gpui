# 06-integrate-article worker report

## Candidate identity

- Branch: `consistency/default-recommendations`
- Base/main: `3775672d251cc8895583ed246e7600c10b723a94`
- Candidate: `a3fc52549b4c1566bbfc585a79fa1c18478ddc86`
- Frozen package boundary: 8 changed paths, +769/−126.
- Candidate, main, merge base, path count, and package numstat were checked before review. Baseline receipt: `.artifacts/branch-trust-audit/06-integrate-article/receipts/00-baseline.txt`.

## Scope accounting

All eight assigned paths have one nonempty row in `.artifacts/branch-trust-audit/06-integrate-article/path-accounting.tsv`; validation reports 8 expected, 8 accounted, 0 missing, and 0 extra.

| Path | Disposition | Finding IDs |
|---|---|---|
| `CLAUDE.md` | Verified static policy correction | 06-F001 |
| `GLOSSARY.md` | Portable links improved; semantic line fragments remain stale | 06-F002, 06-F003 |
| `src/actions_button_visibility_tests.rs` | Source-audit proof debt | 06-F007 |
| `src/footer_popup.rs` | Descriptor improvement plus false-success regression and clean-entry candidate | 06-F004, 06-F005, 06-F006 |
| `src/list_item_tests.rs` | Direct shared eligibility behavior coverage | 06-F008 |
| `src/mcp_computer_use_tools.rs` | No material production change; fixture maintenance | 06-F010 |
| `src/test_support/agent_chat_portal.rs` | Production-shaped fixture and passing round trip | 06-F009 |
| `src/warning_banner.rs` | Stable dismiss identity; live AX/keyboard remains open | 06-F011 |

Machine validation: `.artifacts/branch-trust-audit/06-integrate-article/receipts/99-report-validation.json`.

## Verified improvements

- **06-F001, C2:** `CLAUDE.md` now documents the live `LIQUID_GLASS_CAPSULE_VEIL_ALPHA = 0.0` value instead of the stale 0.80 policy. This is documentation/source parity, not a new visual calibration receipt.
- **06-F002, C2:** changed `GLOSSARY.md` links are repository-relative instead of tied to one `/Users/...` checkout, and the owner-map link validator passes.
- **06-F004, C3:** footer descriptors now carry stable semantic IDs, canonical shortcut tokens/routes, enabled state, disabled reason, and placement. Duplicate IDs fail validation; duplicate enabled shortcuts are retained for diagnostics but made non-routable. The direct model tests pass and both shortcut and action dispatch consumers use the descriptor.
- **06-F008, C3:** direct binary tests cover the `RowEligibility` lattice, inert-row skipping, disabled-explanation selection, empty lists, and invalid state rejection. Grouped Actions consumes the same helpers.
- **06-F009, C3:** Agent Chat portal test support now constructs the production dictation-history version and split target identity/label fields. Seven integration tests pass through the production formatter and parser.
- **06-F011, C2:** the warning dismiss control now has stable ID `warning-banner:dismiss`; the shared Button renders that ID as GPUI identity/debug selector, and the callback still stops propagation before dismissal. Three contrast/chrome tests pass.

## Confirmed regressions

- **06-F005, P1, C2:** `activate_native_main_footer_button` sets `dispatched=true` immediately after `performClick:`. The AppKit target then calls a bounded channel `try_send`; any full/closed-channel error is logged and discarded. `ExternalCommand::TriggerAction` derives `receipt_ok` from `dispatched`, so the new automation seam can return a green result even when the canonical action was not accepted and no resulting state occurred. The wrong-output branch is logically forced by the current source; live channel saturation was not run.

## Breakage candidates

- **06-F006, P1, C1:** from a fresh empty Script List with its default auto-selected row, footer `Agent` routes through standard `main_launcher_with_variant`, whose policy is `AmbientOrFocused`. The explicit default-row suppression is applied by Cmd+Enter, clean-main-launcher, and quick-question entry points, not by this footer route. A live footer activation may therefore inherit an unintended first-row context chip. `observed` remains null because this lane did not run the application.

## Unproven claims and proof gaps

- **06-F003, C2:** the glossary path validator passes, but at least five retained `#L` fragments land on unrelated current code. Examples: `app_view_state.rs#L1361` is a browser-tabs contract row while `MainWindowMode` is at line 1748; `dictation/window.rs#L503` is the global Escape monitor while `DictationOverlay` is at line 888; `render_prompts/other.rs#L441` is template-prompt tracing while `render_chat_prompt` is at line 531.
- **06-F007, C1:** 17 changed source-reading tests pass, but first mouse, native hit testing, listener cardinality, canonical acceptance, and resulting UI state remain runtime claims. The source-audit ratchet ran one shrink-only count test and does not raise those claims.
- The owned MCP diff adds `generation: None` to three test fixtures only. All 168 focused MCP tests pass, but no live MCP projection before/after Full, Mini, Actions, Agent Chat, or warning states was observed.
- Warning contrast has behavior coverage, but live `getElements`, focus, Enter/Space activation, callback cardinality, parent-action isolation, and post-dismiss state were not observed.
- No AppKit/native runtime, screenshot, temporal, lifecycle, or glass boundary was crossed by this worker.

## Focused verification

The complete matrix is `.artifacts/branch-trust-audit/06-integrate-article/verification.tsv`.

- `V01` glossary owner-path validator: pass.
- `V02` initial `--lib list_item_tests` filter matched nine unrelated unified-list-item tests; rejected as a filter mismatch. Corrected binary target `V02b`: 14/14 pass.
- `V03` actions/footer source audits: 17/17 pass, admitted at C1 only for native claims.
- `V04` and `V04b` library quick-question filters ran zero tests and were rejected. Correct binary target `V04c`: 11/11 pass.
- `V05` library portal filter ran zero tests and was rejected. Correct integration target `V05b`: 7/7 pass.
- `V06` MCP computer-use filter: 168/168 pass.
- `V07` source-audit ratchet: 1/1 pass with shrink-only interpretation.
- `V08` bounded `check --lib`: pass.
- `V09` footer descriptor filter: 2/2 pass.
- `V10` warning-banner filter: 3/3 pass.

Every Cargo invocation used `./scripts/agentic/agent-cargo.sh` under the campaign bounded runner. Zero-test filters are preserved as non-green evidence rather than omitted.

## Runtime boundaries crossed

None. This worker performed static diff/consumer tracing, direct pure/model tests, integration tests, source audits, and a library check only.

## Runtime boundaries not crossed

- Native inactive first click, edge hit tests, label/keycap/wrapper targeting, drag-versus-click, and one-event dispatch.
- Bounded-channel enqueue acceptance, listener cardinality, canonical handler acceptance, and resulting state.
- Twenty Full/Mini/Agent Chat footer transitions and stale configuration/handle rejection.
- Footer Agent clean entry from a real default selected row.
- Live MCP/getElements projection across footer, Actions, Agent Chat, and warning states.
- Warning-banner focus, accessibility, keyboard dismissal, callback isolation, and post-dismiss disappearance.
- Real portal picker focus/lifecycle.
- Any screenshot, pixel, temporal, glass, application deployment, or publication boundary.

## Screenshot requests

Five bounded manager requests are in `.artifacts/branch-trust-audit/06-integrate-article/screenshot-requests.jsonl`:

1. `06-S01` Full Script List native footer with paired inactive-first-click and resulting-state trace.
2. `06-S02` Mini footer with paired 20-transition listener/stale-control trace.
3. `06-S03` Actions opened by footer semantic ID with enqueue, generation, focus, and close receipts.
4. `06-S04` clean Agent Chat footer entry from a real default auto-selected row, proving zero context and zero submit.
5. `06-S05` warning banner with live getElements, focus, keyboard dismissal, callback isolation, and post-state receipts.

Screenshots are explicitly limited to visible anatomy; none may substitute for the paired temporal/runtime receipt.

## Prioritized next actions

1. **P1 / N06:** Change native footer automation success from `performClick` issuance to a correlated chain: enqueue accepted → canonical handler accepted → expected state observed. Propagate `try_send` failure. Stop only when a forced full/closed-channel control returns failure and one normal action returns exactly one resulting state.
2. **P1 / N06:** Run the native footer matrix across inactive Full, Mini, and Agent Chat, edge hit regions, drag, disabled controls, and twenty transitions. Stop at one input → one action → one state, with one listener and no stale config/handle.
3. **P1 / N05:** Run footer `Agent` from an empty launcher with its real default selected row. If context appears, route only this clean affordance through `clean_main_launcher`/quick-question suppression; preserve deliberate Cmd+Enter and targeted-row context.
4. **P1 / N04:** Compare live renderer state with MCP/getElements before and after Full, Mini, Actions, Agent Chat, and warning states. Stop at unique stable IDs, truthful enabled/visible state, no stale controls, and no private canary.
5. **P2 / N16:** Keep the source audits as architecture locks but add runtime proofs for first mouse, hit testing, listener lifecycle, and resulting state; do not describe source checks as runtime verification.
6. **P2 / N18:** Replace stale glossary line fragments with symbol-stable links or validate that each fragment lands near its named owner.
7. **P2 / N04:** Prove `warning-banner:dismiss` in live semantic output and activate it with keyboard while showing the parent action does not fire.

## Integration requests

Nine typed requests are in `.notes/oracle/branch-trust-audit/lanes/06-integrate-article/integration-requests.jsonl`: three article findings, two ledger entries, one consolidated screenshot request, and three future-remediation requests. The manager should independently rerun the native-footer false-success falsifier before admitting 06-F005 above C2.

## Production-write proof

- `.artifacts/branch-trust-audit/06-integrate-article/receipts/99-owned-head-vs-worktree.tsv` confirms all eight owned production/document paths byte-match candidate `HEAD` after the audit.
- Worker writes are confined to `.artifacts/branch-trust-audit/06-integrate-article/**` and the two authorized lane report files.
- No production source, documentation source, other lane, manager/integrated/article/publication file, or Git index entry was modified by this worker.

## Stop statement

Audit evidence and article inputs are complete for 06-integrate-article; production files were not modified, and no commit, push, merge, application deployment, glass retune, or publication was performed.
