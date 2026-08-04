# Flow C05 Adversarial Audit

- Premise: `.notes/oracle/flow-c05-archive/premise.md`
- Plan/checklist: `.notes/oracle/flow-c05-archive/plan.md` A–AC
- Start HEAD: `d4287ef3a1da5b83cd66569e4e53ccc4662ec81e`
- Final stable artifact: `target-agent/artifacts/cons-flow-c05/script-kit-gpui`
- Final SHA-256: `1bded18b685837831f5579ba3fcf4692c22908ae6f75c27633e491fd69398ece`
- Runtime receipt: `.artifacts/consistency/cons-flow-ux/c05-flow-archive-v1/runtime/flow-history-receipt.json`
- Oracle consult: `flow-c05-archive`, 1/1, completed; no second consult

| Clause | Authoritative evidence | Counterexample searched | Result | Remediation owner |
|---|---|---|---|---|
| A. v0–v3 migration | Exact version migration tests in final `flows::session` 55-pass suite; canonical reload/turn equality | Lost/reordered turns, multiple active threads, v4 legacy top-level writes | PASS | `src/flows/session.rs` canonicalizer |
| B. malformed v4 | Canonicalization matrix covers pointer/state conflicts, IDs, timestamps, parent edges, top-level fallback, future version | Empty selection despite retained turns, duplicate IDs, aggregate loss, rewrite loop | PASS | `canonicalize_persisted_conversation` |
| C. missing vs empty | Focused model tests plus runtime New/restart keep one zero-turn active manifest | File deletion, `None` restore, old transcript becoming current | PASS | loader/writer and New/Delete-active transitions |
| D. uncapped retention | Exact 20/1,000-turn round trips; runtime seed/restores 15; state says `uncappedByApp`, `turnCap:null` | `take(12)`, truncation/slicing, “unlimited storage” promise | PASS | persistence and identity projection |
| E. New | Runtime `new-archives-active`; model tests include populated and empty active metadata | Old turns lost/current, manifest removed, delete command used | PASS | `archive_active_and_start_empty`, `start_fresh_flow_conversation` |
| F. archive read-only | Runtime archive has `readOnly:true` and no composer; closed archive command model tests | Send/composer/Terminate/New leaking into archive or mutating revision | PASS | selection renderer, command facts, submit guard |
| G. Continue as New | Runtime archive retained, writable child has inherited 15 and parent retained; lifecycle model tests | Source removal, empty archive fabrication, missing parent/inherited count | PASS | `continue_archive_as_new` |
| H. stopped partial output | Active Terminate runtime checkpoint settles exactly one additional turn; event/model tests preserve raw output with Stopped outcome | Failure-card classification, persisted display caption, duplicate terminal turn | PASS | `finish_flow_turn`, event projection |
| I. rethread truth | Model acceptance matrix and runtime Continue/Terminate/ordinary-turn checkpoints | Clearing before mdflow acceptance; remaining true after fast Succeeded | PASS | submit/event sync; `mdflow_run_accepted_context` |
| J. Terminate Runtime | Descriptor has no shortcut; Shift+Cmd+Escape ignored; runtime idle/active Terminate preserves thread/archive/draft and settles first | Session removal, file deletion, early runtime forget, reduced thread count | PASS | command metadata and termination settlement |
| K. Delete selected | Private marker types; runtime cancel no-op and archive confirm; model active/archive delete tests | Confirmation bypass, full manifest deletion, nonselected loss | PASS | Actions confirm closure and selected-delete transition |
| L. tombstone ordering | Model rejects stale and forged-higher snapshots; runtime held stale release cannot restore deleted archive | Deleted ID returning after release/flush/restart | PASS | `FlowConversationStore` |
| M. non-delete dismissal | Runtime Escape/Cmd+W manifest count fingerprint preservation; model Back/Background/Close paths | Delete handler/revision mutation/secret stop from dismissal | PASS | key/footer/window dismissal handlers |
| N. draft ownership | Model archive/Back/Terminate/Delete rules; runtime idle Terminate retains 20-char draft | Sole `filter_text` ownership, hidden archive draft loss, wrong clear semantics | PASS | per-session draft helpers |
| O. command parity | Conversation actions 10/10, Flow Session 32/32, Flow Desk 10/10; shared descriptors drive footer/Actions/elements/keys | Dead shortcut, missing handler, blank disabled reason, archive command leak | PASS | `conversation_actions.rs` and Flow adapters |
| P. identity/privacy | Identity matrix; runtime five privacy counters zero; receipt canary scan PASS | Raw cwd/path/transcript/draft/provider/roster error; combined engine-model string | PASS | `FlowSessionIdentitySnapshot`, collectors and automation |
| Q. typed desk states | Pure resolver tests plus runtime Loading/Missing/Incompatible/Failed/Empty/NoMatch/Ready scenarios | RosterFailed→ReadyEmpty, Loading enabled row, conflated missing/incompatible | PASS | `FlowDeskState` resolver |
| R. selected-row verbs | Descriptor tests and dynamic footer/Actions/activation consumers; runtime Ready/NoMatch paths | Universal Converse, workflow chat, duplicate Run Once, interactive Converse | PASS | `FlowDeskRowDescriptor` |
| S. deterministic deadline | Injected runner passes 20 same-process iterations with one exact deadline; owned process-group deadline test PASS | 50 ms sleep, regenerated named deadline, child survivor | PASS | `resolve_mdflow_turn_arg_with_runner`, explain cache |
| T. current bytes/stale cleanup | Final logs: session 55/1 ignored, actions 10, Session bin 32, Desk bin 10, `check --lib` PASS | Obsolete Terminate key variant/test, contradictory fixture, broad normal delete API | PASS | C05 Rust owners and initializer tests |
| U. real runtime | Stable SHA matches build and receipt; 15 messages traverse built app + real mdflow event protocol; nine Driver scenarios | Seed-only/mock-only UI, stale binary, sleep-only assertion | PASS | C05 Driver probe and fixture |
| V. process cleanup | All 9 rows: process/streams/log true, app/fixture counts 0, no forced termination | Surviving exact executable/PID, missing finally, broad `pkill`/`killall` | PASS | probe `try/finally` + Driver close |
| W. clipboard | Every scenario records untouched and equal pasteboard change counts; no copy action activated | Changed clipboard, logged contents, false restoration claim | PASS | probe |
| X. source-audit/dead APIs | Inventory reports no new guarded sites vs `d4287ef3a`; diff match is production definition-model read, not a test audit | New test source read, exact formatted count lock, stale broad delete/Terminate API | PASS | owning Rust tests/APIs |
| Y. hardcoded visuals | `audit/new-hardcoded-visuals.txt` empty after final focus-race repair | New local color, geometry, opacity, animation, or timing value | PASS | Flow renderer/native footer |
| Z. protected glass | Protected changed-path list empty; Bun 40/40; calibration fixture 1/1 | Changed locked owner/fixture/envelope/threshold or weakened test | PASS | restore-only; no C05 retune allowed |
| AA. progress/ledgers | Progress is 29/75 with one SAFE-003 and one WF-011 section, numbered steps, final SHA; both ledgers current; index 18 | Duplicate section, stale hash, mock-only claim, missing receipt/user path | PASS | progress and Oracle ledgers |
| AB. staged scope/commit | Authorized staged paths, exact subject, start parent, and postcommit receipts are recorded in `gates/{staged-paths,final-commit,final-parent}.txt` | Unrelated file, secret/runtime binary, wrong parent/subject, multiple final commits | PASS | staging/commit step |
| AC. no publication | Ledger records local commit only; no push/deploy/tag/release/publication action | Remote mutation or concealed publication | PASS | execution discipline |

## Audit conclusion

Every premise clause has model/current-byte proof at its owning layer, and runtime clauses cross the final built product/mdflow/Driver boundary. The two runtime-discovered races were repaired at their owning state transitions, followed by a stable-artifact rebuild and a complete nine-scenario rerun. No fixture, threshold, privacy rule, or locked glass value was weakened to obtain green results.

VERDICT: PASS
