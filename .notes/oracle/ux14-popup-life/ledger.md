# UX-014 Oracle Execute Ledger

- premise: `.notes/oracle/ux14-popup-life/premise.md`
- consult: `ux14-popup-life`
- consult_count: 1 / 1
- transport: round-robin protocol v2, browser, Extended
- assigned_profile: `profile-b`
- allocation_id: `profile-lease-00000708`
- oracle_status: completed
- oracle_receipts: `.notes/oracle/ux14-popup-life/oracle-output.log`, `.notes/oracle/ux14-popup-life/oracle-meta.json`
- plan: `.notes/oracle/ux14-popup-life/plan.md`
- committed_baseline: `5ef3c45de`
- implementation_status: complete; commit pending
- local_audit: `VERDICT: PASS_PENDING_COMMIT` (`.artifacts/ux14-popup-life/17-adversarial-audit.txt`)
- product_binary: `target-agent/artifacts/ux14-popup-life/script-kit-gpui`
- product_binary_sha256: `a2e667d19f8e5bc0b3c558995b93df1de69a96c241862ecbf6fa8b322eecc4c3`
- final_commit: this single UX-014 commit; exact hash recorded by post-commit read-only receipt

## Fixed decisions

- One narrow generation-scoped lifecycle cell lives in `src/components/inline_popup_window.rs`; history query/selection, portal state, microphone rows, and device actions remain consumer-owned.
- Interactive popups start `show:false, focus:false`, verify the exact live parent and AppKit child relationship while hidden, and publish automation/runtime/semantic identity only after attach-ready.
- Generation identity flows through protocol targets, registry descriptors, runtime handles, semantic caches, slots, callbacks, batch commands, screenshot/event dispatch, and conditional cleanup.
- Agent Chat history and Dictation microphone are the representative migrations. Actions remains independent; Confirm remains generationless and parent-key-routed; menu syntax stays a main-list projection; footer keeps the compatibility attach wrapper.
- Parent-owned close paths reconcile owner state directly before closing the child, preventing GPUI double leases. Child-owned close paths notify the owner once through the shared idempotent close gate.
- Native-close automation requires an exact `Instance { id, generation }` PromptPopup target. AppKit temporarily receives the behavior-only closable mask so `performClose:` traverses GPUI's should-close callback for borderless children.
- Protected glass timing, alpha, scale, placement, material, optics, fixtures, and thresholds are unchanged.

## Work packages

- [x] W0 — immutable premise, full owner inventory, 642,652-byte bundle, one Oracle consult
- [x] W1 — shared lifecycle, legal transitions, hidden attach handshake, typed receipts, close gate
- [x] W2 — generation-aware protocol/registry/runtime/cache identity and strict target resolution
- [x] W3 — Agent Chat history lifecycle migration and centralized owner transitions
- [x] W4 — Dictation microphone lifecycle migration and no-persistence fixture
- [x] W5 — one-layer parent Escape, popup-local routes, outside click, exact focus return
- [x] W6 — generation-conditional cleanup, fresh reopen, closing-slot protection
- [x] W7 — exact PromptPopup subtype resolver, collector, batch routing, exact event dispatch
- [x] W8 — deterministic Agent Chat/Dictation fixtures and strict native-close primitive
- [x] W9 — direct lifecycle/protocol tests plus obsolete source-reader cleanup
- [x] W10 — format, binary check/build, Bun anti-drift, source-audit and visual-literal checks
- [x] W11 — final stable product artifact/SHA
- [x] W12 — real Agent Chat and Dictation runtime/visual lifecycle proof
- [x] W13 — final Actions/live regression receipts, including explicit fail-closed CLI instrumentation deviations
- [x] W14 — exact process/clipboard cleanup
- [x] W15 — progress docs and adversarial audit (`PASS_PENDING_COMMIT`)
- [x] W16 — this one prompt-style local commit; post-commit checks are read-only

## Implementation receipts

### Shared lifecycle and strict identity

- `src/components/inline_popup_window.rs`: monotonic `InlinePopupGeneration`; `CreatedHidden → AttachPending → Open → Closing → Closed`; idempotent close gates; exact focus-return token; three deferred parent-readiness turns; hidden/non-key child checks; exact AppKit parent-pointer verification; typed native window numbers and attach facts.
- `src/protocol/types/automation_window.rs`: schema v2, optional descriptor generation, strict `AutomationWindowTarget::Instance`.
- `src/windows/{automation_registry,automation_runtime_handles,automation_surface_collector}.rs`: generation-conditional resolution, cleanup, and semantic collection; no “whichever PromptPopup is open” fallback.
- `src/prompt_handler/mod.rs`: exact Agent Chat history / Dictation microphone / Confirm subtype routing and per-command generation revalidation.
- `src/platform/gpui_event_simulator.rs`: strict generational runtime-handle lookup and deferred PromptPopup dispatch; no parent fallback for a stale instance.

### Consumer ownership

- Agent Chat: `history_popup_lifetime` owns one generation and prior composer focus; every direct `history_menu = None` outside the two central owner helpers was removed. Setup/session/draft/picker/portal/submit/selection transitions now enter `close_history_popup_for_owner_transition`. Child selection paths reconcile the owner then defer child close, avoiding entity re-entry.
- Dictation: overlay-owned lifetime requires exact parent `dictation`; parent Escape and parent mouse-down reconcile owner state before child close; popup Escape/outside/focus/native/accept/attach failure routes share one gate; fixture rows never persist a device.
- Removed dead `src/ai/agent_chat/ui/popup_registry.rs`; current registration/cleanup is owned by the generation-aware popup window path.

## Verification receipts

- Shared lifecycle focused tests (earlier final-owner implementation stage): `2 passed; 0 failed` for legal transition/hidden options coverage.
- Dictation source contract after pruning obsolete lifecycle substrings: `3 passed; 0 failed`.
- `./scripts/agentic/agent-cargo.sh check --bin script-kit-gpui`: finished successfully after final native-close and owner-centralization changes.
- Final product build after the native-key focus-pair supplement, through `SCRIPT_KIT_AGENT_ARTIFACT_NAME=ux14-popup-life ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui`: `Finished dev profile` in 1m04s and cloned `target-agent/artifacts/ux14-popup-life/script-kit-gpui`; SHA-256 `a2e667d19f8e5bc0b3c558995b93df1de69a96c241862ecbf6fa8b322eecc4c3`.
- `bun test scripts/devtools/glass-entry-motion-contract.test.ts scripts/devtools/glass-lifecycle-filmstrip.test.ts scripts/devtools/rapid-toggle-stress.test.ts`: `40 pass; 0 fail`.
- Protected glass owner diff: empty for footer, opacity, secondary-window calibration, chrome tokens, named fixture, motion contract, lifecycle/Actions filmstrip, and rapid-toggle sources.
- New visual-literal scan: empty.
- Source-audit inventory: app-source reader sites decreased `2331 → 2330`; all readers decreased `2819 → 2818`. No new reader was added.
- `git diff --check`: pass.

### Real Agent Chat history

Receipt: `.artifacts/ux14-popup-life/runtime-agent-history.json`; screenshot: `.artifacts/ux14-popup-life/agent-history-popup.png`.

- exact detached parent `agentChatDetached:detached-agent-chat-mock-fixture`;
- exact popup instance `agent_chat-history-popup`, generation 1;
- exact semantics: panel, list, and two deterministic history choices;
- strict popup screenshot (`415×463` on the final successful run);
- parent-routed Escape closed only history and left Agent Chat live;
- focus returned to the composer: typing Unicode `λ` yielded exact `focus-return:λ` at cursor 14;
- reopen advanced generation `1 → 2`; generation 1 returned explicit stale-target refusal;
- parent outside-click closed only generation 2; the parent was focused and typing `β` yielded exact `focus-return:λβ` at cursor 15;
- exact native AppKit close removed generation 3 and made the same instance stale; the parent was focused and typing `γ` yielded exact `focus-return:λβγ` at cursor 16;
- Driver finalization: process exited, streams drained, log writer closed, owned process/child counts zero, clipboard untouched.

### Real Dictation microphone popup

Receipt: `.artifacts/ux14-popup-life/runtime-dictation-microphone.json`; screenshot: `.artifacts/ux14-popup-life/dictation-microphone-popup.png`.

- exact parent `dictation`, exact generation-scoped PromptPopup, deterministic panel/list/two-choice semantics;
- strict screenshot `317×80`;
- parent Escape closed only the selector, left the recording overlay, and returned exact overlay focus;
- reopen advanced generation and stale instance reads failed closed;
- parent outside-click and native AppKit close each reconciled the exact generation while the overlay remained and was focused;
- fourth-generation exact batch selection routed only to Dictation;
- fixture selection left sandbox `config.ts` bytes unchanged;
- Driver finalization: process exited, streams drained, log writer closed, owned process/child counts zero, clipboard untouched.

Final Dictation session: `/tmp/sk-driver-sessions/ux14-dictation-microphone-15516-1-mscm9h9y`. Final Agent Chat session: `/tmp/sk-driver-sessions/ux14-agent-history-14562-1-mscm91br`. Both receipt `--verify` modes returned `verified:true` against the final binary.

### Actions and protected-glass regressions

- `.artifacts/ux14-popup-life/glass-actions-entry/receipt.json`: pass against the final binary.
- `.artifacts/ux14-popup-life/rapid-toggle.json`: pass against the final binary.
- Static glass contract suite: `40 passed; 0 failed`.
- Protected glass source diff: empty. No fixture, threshold, duration, alpha, scale, geometry, material, placement, footer optic, or generated token changed.
- `.artifacts/ux14-popup-life/glass-lifecycle/receipt.json`: `EVALUABLE_FAIL` only on the unchanged broad main-entry cadence and Notes pre-reveal body-mask observations already recorded for UX-013; Notes close-before-settle-reopen and Dictation exit-reopen pass, cleanup is complete, and no interference invalidation occurred. The observer was not weakened to turn this green.
- Direct Actions inspector reruns are preserved as fail-closed tooling deviations: shortcut-open classified target ambiguity and close/Escape classified stale view. The open receipt itself reached `classification:"ok"`; the native Actions-entry filmstrip and rapid-toggle real-product proofs are green. No Actions source except the mechanically required `generation: None` descriptor field changed.

### Cleanup

- Both final lifecycle receipts report `processExited:true`, `streamsDrained:true`, `logWriterClosed:true`, `ownedProcessCount:0`, `ownedChildProcessCount:0`, and `clipboardTouched:false`.
- Final independent exact executable-path inventory for `target-agent/artifacts/ux14-popup-life/script-kit-gpui`: zero processes.
- Named Actions sessions were stopped. No broad signal, `pkill`, `killall`, or unrelated Script Kit process was used.

## Local escalations

1. Parent Escape initially crashed with `DictationOverlay` double lease. Root cause: parent listener updated child, whose close callback synchronously updated the already leased parent. Correction: parent-owned close skips child-to-parent callback and reconciles owner state directly.
2. Agent Chat parent Escape exposed the same pattern. Correction: `close_history_popup_for_owner_transition` owns parent transitions; child selection paths reconcile owner first and defer child close.
3. Strict stale target probe initially expected a thrown error, while `getElements` intentionally returned zero elements plus `target_resolution_failed`. Classified `HARNESS_FALSE_NEGATIVE`; probe now asserts the fail-closed warning envelope.
4. Borderless `performClose:` was a no-op. Direct `close` re-entered GPUI when invoked under an app borrow. Correction: resolve exact native identity first, schedule on the foreground executor after the borrow, temporarily add only `NSWindowStyleMaskClosable`, then call `performClose:`. Both popup families now prove native callback reconciliation.
5. Focused `cargo test --lib inline_popup_lifecycle_` was terminated with exit 143 after 7m38s when free disk crossed the repository's 25 GiB watcher floor; the watcher also evicted `target-agent`. This is preserved as an infrastructure failure, not reported green. The same final source compiled and built successfully, and the lifecycle behavior crosses the real product boundary in both Driver scenarios.
6. The adversarial proof pass found that GPUI activation alone can stay false for these nonactivating AppKit panels. The shared focus-pair guard now supplements GPUI activation with exact child/parent `isKeyWindow` state before arming the owned→unowned focus-loss transition. Attempting to prove that route by merely activating Finder was rejected as a harness false premise because the nonactivating parent intentionally remained AppKit's key recipient; the invalid step was removed instead of being labeled green. Escape, outside-click, and native-close now each prove exact parent focus, and Agent Chat proves the next Unicode character at the exact caret after all three safely automatable routes.

## Final commit boundary

This ledger is included in the single prompt-style UX-014 commit. After that commit, only read-only Git, exact-process, and audit checks are permitted. No push, deploy, tag, publication, rebuild, app launch, or second commit is authorized.
