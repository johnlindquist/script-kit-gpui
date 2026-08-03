# Core UX Execution Ledger

- premise: `.notes/oracle/cons-core-ux/premise.md`
- assigned coverage: UX-001..UX-018, GOV-001 (19 tasks)
- consult slug: `cons-core-ux`
- consult count: 1 / 1
- plan status: complete (`plan.md`, 19/19 task IDs covered)
- protocol/profile: v2 / `profile-a-2`
- execution status: C01–C13 complete (`UX-002`, `UX-001`, `UX-017`, `UX-015`, `UX-006`, `UX-010`, `UX-003`, `UX-009`, `UX-004`, `UX-005`, `UX-011`, `UX-012`, `UX-013`, `UX-014`, `UX-016`); starting C14
- audit verdict: pending

## Receipts

- Bundle: 55 files, 946,994 bytes, non-empty, below 1 MiB.
- Prompt: prepared; exact size validated before submission.

## Step ledger

### C01 — UX-002 canonical shortcut stream

- **Status:** Complete; verification green.
- **Implementation:** `hint_strip::shortcut_tokens_from_hint` owns display tokenization. Removed the independent footer and Actions character parsers; Button, `UnifiedListItem`, PromptFooter, Select, Actions, recorder, `ListItem`, Notes actions, and native footer consumers now delegate to or cache the shared tokens.
- **Decision branch:** The Actions CLI’s optional shortcut-activation primitive was not required by the UX-002 proof contract. Its first run omitted keep-open and correctly blocked; a guarded run exposed exact target/runtime geometry; subsequent subprocess runs repeated a response timeout. Per the DevTools guidance, the final real-boundary check moved to one persistent `Driver` process rather than weakening the inspector.
- **Focused receipts:**
  - `.artifacts/consistency/UX-002/cross-consumer-tests.log` — 1 passed.
  - `.artifacts/consistency/UX-002/prompt-footer-tests.log` — 1 passed.
  - `.artifacts/consistency/UX-002/actions-parser-tests.log` — 24 passed.
  - Existing footer, recorder, Button, Unified row, Select, parser, and check receipts under `.artifacts/consistency/UX-002/` remain green.
  - `.artifacts/consistency/UX-002/check-lib-final.log` — `cargo check --lib` finished.
  - `.artifacts/consistency/UX-002/build-final.log` — product binary build finished; SHA-256 `6cf490d3dd24d27e096d601da53e2165334dfe5785d34fbcf63fdc3821291a1a`.
  - `.artifacts/consistency/UX-002/driver-runtime-proof.json` — `RUNTIME-CONFIRMED`; exact `actions-dialog`; 6/6 row token vectors match layout vectors; every token has positive runtime bounds; full process/stream/log cleanup.
- **Guardrails:** Native glyph x/y offsets and horizontal padding still resolve through shared footer helpers. No glass, motion, fixture, threshold, or generated token changed.
- **Commit:** `195e70e9d` — `Implement UX-002: establish one canonical shortcut token stream across GPUI and AppKit consumers`.

### C02 — UX-001 cue kinds and UX-017 info-state tones

- **Status:** Complete; focused, runtime, visual, and cleanup proof green.
- **Implementation:** Replaced overloaded `InfoGuidanceItem.shortcut` with required `InfoCue` variants and validated shortcut construction. Migrated permission, Agent Chat, and launcher compact guidance. Added real cue-width measurement by cue family, semantic snapshots, all six tone presentations, shared Lucide icon resolution, and no-wash rendering. Projected InfoState cues into `getElements`; for actual launcher tag/filter guidance, projected the existing rich `MenuSyntaxMainHintSnapshot` rather than fabricating an InfoState.
- **Decision branch:** Current render precedence shows `MenuSyntaxMainHint` for unmatched tag/advanced-query guidance and fallback actions for ordinary unmatched text. The Oracle premise assumed every no-result case rendered InfoState. Current source/runtime outrank that assumption, so the implementation preserves the intentional rich menu-syntax owner and gives both owners the same cue-kind semantics.
- **Focused receipts:**
  - `.artifacts/consistency/UX-001/info-state-tests-final.log` — 16 passed.
  - `.artifacts/consistency/UX-001/info-elements-tests-final.log` — binary target 1 passed.
  - `.artifacts/consistency/UX-001/agent-chat-elements-tests-final.log` — binary target 1 passed.
  - `.artifacts/consistency/UX-001/check-lib-final.log` and `build-final.log` — finished; SHA-256 `96228805b32da827e27219fd11fbc44225bd825aef68df7214dcc8c661680adc`.
  - `.artifacts/consistency/UX-001/runtime-cues-proof-final.json` — `RUNTIME-CONFIRMED`; launcher syntax and Agent Chat trigger/shortcut semantics; complete cleanup.
  - `.artifacts/consistency/UX-017/tone-visual-proof-final.json` — `RUNTIME-CONFIRMED`; Help and Recovery in dark/light; four complete cleanup receipts.
  - `.artifacts/consistency/UX-017/{help-dark,help-light,recovery-dark,recovery-light}.png` — visually inspected; semantic accent hierarchy present, no decorative tone wash.
- **Negative controls:** Syntax/trigger/label/empty/missing-ID inputs cannot construct shortcuts; `;todo` remains one syntax semantic; dynamic query content is absent from InfoState semantic snapshots; all non-shortcut guidance is inert; no unsupported action is created.
- **Cleanup:** Every Driver process exited and drained; `pgrep` found no `cons-core-info` or `cons-core-ux` Script Kit binary after proof.
- **Commit:** `b7ac0aa8f` — `Implement UX-001 and UX-017: type InfoState cues and make tone semantic without decorative washes`.

### C03 — UX-015 static hint semantics

- **Status:** Complete; model, real GPUI dispatch, product runtime, visual, and cleanup proof green.
- **Implementation:** Renamed the static icon entry points and removed all pointer/hover/active button chrome from them. Static `HintStrip` instances no longer register root pointer handlers. Interactive `HintStrip`, `ClickableHint`, and `SelectableHint` paths require non-empty stable action IDs and non-optional callbacks; their render IDs and debug selectors use those action IDs rather than positions. The actual Notes resource preview shares those IDs with its automation state. Its shared root now fills the host height so bottom hint actions remain visible.
- **Decision branch:** The first product probe exposed that Notes preview hint actions existed but rendered below the constrained viewport. Adding `h_full()` at the shared resource-preview root made the existing contract visible without changing hint metrics, motion, or host geometry. The real Driver proof then used a sandbox Notes target, a deterministic `kit://scripts` preview, a user-equivalent taller Notes frame, and an exact-handle GPUI click on `Back to Note`.
- **Focused receipts:**
  - `.artifacts/consistency/UX-015/hint-strip-tests.log` — 10 passed, including static zero-dispatch and interactive exactly-once real GPUI pointer dispatch.
  - `.artifacts/consistency/UX-015/resource-preview-tests.log` — 4 passed.
  - `.artifacts/consistency/UX-015/check-lib.log` and `build.log` — finished.
  - `.artifacts/consistency/UX-015/runtime-proof.json` — `RUNTIME-CONFIRMED`; stable action identity/capability, visible 350×520 preview, exact Notes handle dispatch, preview closed, full cleanup.
  - `.artifacts/consistency/UX-015/notes-preview.png` — clickable Copy URI and Back to Note hints visible inside the window.
  - Stable binary SHA-256: `2b8d208f2666d97bc06a6193ebdad25531814e47368b8742f88bc83f0de0dc8d`.
- **Negative controls:** Empty action IDs are rejected; missing callbacks are unrepresentable; static snapshots have no action/pointer/hover/active/click semantics; inventory finds no old fake-clickable static APIs or positional interactive IDs.
- **Cleanup:** Every exploratory/final Driver closed. Exploratory files were removed. Final `pgrep` found no `cons-core-hints` or history-probe Script Kit process.
- **Commit:** `53ed38109` — `Implement UX-015: make static hint strips inert and reserve pointer chrome for real actions`.

### C04 — UX-006 shared row visual-state palette

- **Status:** Complete; exact-byte, geometry, dark/light runtime, and cleanup proof green.
- **Implementation:** Added renderer-neutral `RowStateFlags`, `RowVisualState`, `RowStateColors`, `RowStatePalette`, and family paint inputs in `theme::chrome`. State precedence is `disabled+selected > disabled > active > selected > hovered > rest`. Main-menu compatibility palettes now derive from that resolver. Unified rows and dense/soft-compact inline dropdown rows provide their existing bases, opacities, and foreground tiers to the same resolver while retaining family-owned layout.
- **Byte preservation:** Main InfoBarBase remains text-primary at hover `0x12` and selected/active `0x20`; accent row kinds retain their accent bases and CarbonNeon/OperatorMono selected foreground rules. Unified retains its selected/hover alpha truncation and selected-disabled icon emphasis. Dense dropdown stays `0.23` selected / `0.06` hover; soft compact stays `0.18` selected / `0.06` hover.
- **Focused receipts:**
  - `.artifacts/consistency/UX-006/theme-chrome-tests.log` — 13 passed.
  - `.artifacts/consistency/UX-006/unified-list-item-tests.log` — 16 passed.
  - `.artifacts/consistency/UX-006/inline-dropdown-tests.log` — 18 passed.
  - `.artifacts/consistency/UX-006/list-item-tests.log` — 36 passed.
  - `.artifacts/consistency/UX-006/check-lib.log` and `build.log` — finished.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-006/runtime-proof.json` — `RUNTIME-CONFIRMED` in dark and light. Main exposed five 44px rows; a real GPUI pointer-hover moved semantic selection without changing any row bounds. Actions exposed its existing 24px headers and 36px action rows. Screenshots: `main-dark.png`, `main-light.png`, `actions-dark.png`, `actions-light.png`.
- **Negative controls:** Disabled hover resolves to disabled paint; disabled-selected preserves selected background; active outranks selection/hover; current selected and active bytes remain equal; no migrated renderer computes `selected_opacity * 255` locally; soft compact height is exactly `36.0`.
- **Cleanup:** Both final Driver processes exited, streams drained, and log writers closed. Exact artifact-path process check returned `OWNED_PROCESSES_CLEAN`.
- **Binary:** SHA-256 `7ac8a8408a9db2d3e718bd6261c3286ed64eaec0cedd4f30324a539b33278a24`.
- **Commit:** `b44b4f2eb` — `Implement UX-006: consolidate row visual-state resolution without changing family geometry`.

### C05 — UX-010 UnifiedListItem API honesty

- **Status:** Complete; production inventory, focused tests, runtime/visual proof, and cleanup green.
- **Decision branch:** Repository-wide production inventory found exactly one constructor owner, `src/prompts/select/render.rs`, and zero production callers for `TextContent::Custom`, `LeadingContent::Custom`, `TrailingContent::Custom`, `a11y_label`, or `a11y_hint`. The zero-caller branch fired: all unsupported variants/builders/fields and discard render arms were deleted rather than made more elaborate. Semantic identity and action behavior remain honestly owned by the Select host wrapper.
- **Implementation:** `TextContent` now represents only Plain and Highlighted source text and always returns `&str`; Leading keeps Emoji/Icon/AppIcon/placeholder; Trailing keeps shortcut/hint/count/chevron/checkmark. Removed the obsolete module-wide dead-code allowance. Deleted the tracked but unwired duplicate `src/components/unified_list_item/tests.rs`; preserved its useful canonical-shortcut assertion in the wired `src/components/unified_list_item_tests.rs`.
- **Focused receipts:**
  - `.artifacts/consistency/UX-010/inventory.json` — PASS; one production constructor owner, all removed API counts zero, no discard arms, duplicate test file absent.
  - `.artifacts/consistency/UX-010/unified-list-item-tests.log` — 17 passed.
  - `.artifacts/consistency/UX-010/check-lib.log` and `build.log` — finished.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-010/runtime-proof.json` — `RUNTIME-CONFIRMED`; the real Select prompt rendered its focused Unified row with stable semantic identity, emoji/icon cue, title, subtitle metadata, and canonical shortcut. Screenshot: `.artifacts/consistency/UX-010/select-production-row.png`.
- **Negative controls:** Unsupported custom branches and inert accessibility builders are unconstructable; repository inventory reports zero old APIs; no custom-content discard arm remains; strict semantic identity stays at the actual Select host rather than a fictional component callback.
- **Cleanup:** Driver exited, streams drained, log writer closed, and exact artifact-path process inventory returned `OWNED_PROCESSES_CLEAN`.
- **Binary:** SHA-256 `f531dfea56c35e6b783aaea134a5a2806d0e400d8d525681668b0c6a9d8d4f35`.
- **Commit:** `37cea6b69` — `Implement UX-010: remove false UnifiedListItem API branches and expose retained semantics`.

### C06 — UX-003 and UX-009 explicit availability and selection eligibility

- **Status:** Complete; compiler, navigation, activation-route, visual, runtime, and cleanup proof green.
- **Decision branch:** Repository inventory found one constructible `Action` owner plus one SDK conversion literal, duplicated selection helpers in Actions and Command Bar, and four execution routes (selected Enter, pointer activation, displayed shortcut, and direct action ID). Availability is therefore private and compiler-enforced on `Action`; selection eligibility is shared in `list_item`; every executable route converges on `ActionsDialog::activate_action_id`. The first combined runtime script let Enter remove its target before later routes and correctly failed. The final proof isolates each route by closing/reopening after the product's intentional recent-close cooldown.
- **Implementation:** Added validated enabled/disabled availability with required reasons; `RowEligibility` with `activatable ⇒ selectable ⇒ focusable`; `Option<usize>` as the sole Actions no-selection representation; semantic identity restoration and nearest-eligible fallback across refresh; disabled explanatory rows that remain visible/selectable but never activate; one 132px single-line reason slot; disabled/duplicate shortcut suppression; typed `Blocked` activation; and fail-closed detached direct-ID routing that cannot fall through to a host while an Actions surface is reported open. Main, Actions, Command Bar, Notes, Day Page, and detached Agent Chat handlers now preserve blocked state without closing or executing.
- **Focused receipts:**
  - `.artifacts/consistency/UX-003/disabled-action-tests-final.log` — 5 passed: mandatory reason, blank-reason rejection, no shortcut promise, and selected/direct callback suppression.
  - `.artifacts/consistency/UX-003/direct-close-policy-test.log` — 1 passed: direct-ID activation uses the activated action's SDK close policy, not the selected row's policy.
  - `.artifacts/consistency/UX-003/refresh-selection-test-final.log` — 1 passed: identity restore, selected-disabled preservation, nearest-row fallback, and no-selection state.
  - `.artifacts/consistency/UX-009/row-eligibility-tests-final.log` — 4 passed, including both constructor invariant negative controls.
  - `.artifacts/consistency/UX-009/no-selection-test-final.log` — 1 passed.
  - `.artifacts/consistency/UX-003/check-lib-final.log` and `build-final.log` — finished; stable binary SHA-256 `7965c81f6547d3cd7c2f203bc748b585d88f9951b988be7815e53da9cd0d3e4a`.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-003/runtime-proof.json` and mirrored UX-009 receipt — `RUNTIME-CONFIRMED`; all 13 assertions true. The real detached Actions window showed the selected disabled row and semantic reason, exposed focusable/selectable/activatable as true/true/false, hid its configured keycap, blocked direct ID/Enter/two-click activation, ignored the hidden configured shortcut, emitted no host/callback execution, and crossed the exact GPUI click handle. Screenshot: `.artifacts/consistency/UX-003/disabled-action-selected.png`.
- **Negative controls:** Blank disabled reasons panic; impossible eligibility combinations panic; disabled and duplicate shortcuts do not render/register; empty dialogs store `None`; direct-ID handle races fail closed as `actions_dialog_unavailable`; blocked logs contain no `actions_host_execute` or executed outcome.
- **Cleanup:** Every exploratory and final Driver closed its owned app. Finalization reports process exited, streams drained, and log writer closed; exact artifact-path process count is zero.
- **Guardrails:** No glass-motion value, generated token, animation fixture, footer optical offset, or family geometry changed. Disabled Actions retain the existing 36px row rhythm and selected-location paint.
- **Commit:** `e005f5db2` — `Implement UX-003 and UX-009 atomically: make Actions availability explicit and selection eligibility safe`.

### C07 — UX-004 one footer action descriptor

- **Status:** Complete; model, native AppKit, GPUI overlay, key/click parity, negative controls, optical anti-drift, runtime cleanup, and build proof green.
- **Implementation:** `FooterButtonConfig` now owns stable ID, executable route, raw shortcut, cached UX-002 tokens, canonical shortcut, routability, user-facing verb, enabled/disabled reason, selection, and placement. `MainWindowFooterConfig` validates unique IDs, suppresses canonical collisions without treating them as slot violations, and resolves keyboard routes only for one enabled descriptor. Native AppKit, the GPUI overlay, prompt rails, Dictation, semantic projection, and active-footer protocol schema 2 consume the same descriptor vector. ScriptList footer Run and Enter share the same menu-syntax, submit-echo, spine, fallback, and execution dispatcher. Active-footer automation now reports whether the GPUI overlay is actually live.
- **Decision branch:** `getLayoutInfo(main)` with fidelity capture exposed descriptor-derived AppKit item geometry, so the native proof used an exact OS pointer click at that measured item center. The no-glass path exposed a separately registered `footer-overlay`; its own generic target did not project element bounds, but the parent main fidelity receipt included the completed overlay paint target and descriptor-derived bounds. The proof therefore used that authoritative paint-time geometry with exact-handle `simulateGpuiClick`, rather than approximating or weakening target identity.
- **Focused receipts:**
  - `.artifacts/consistency/UX-004/footer-popup-tests-final.log` — 47 passed.
  - `.artifacts/consistency/UX-004/footer-chrome-tests-final.log` — 14 passed.
  - `.artifacts/consistency/UX-004/prompt-layout-shell-tests-final.log` — 53 passed.
  - `.artifacts/consistency/UX-004/render-script-list-tests-final.log` — 11 passed on the binary target.
  - `.artifacts/consistency/UX-004/check-lib-final.log` and `build-final.log` — finished; binary SHA-256 `ca7ea565f5e42c81f09d8e8dc07a7f6df8d7d4aa0b14c33a0ec2235889e4570c`.
- **Runtime receipt:** `.artifacts/consistency/UX-004/runtime-proof.json` — `RUNTIME-CONFIRMED`; key and native click opened the same Actions route; disabled key/click failed closed; duplicate canonical shortcuts hid keycaps and did not route; a changed verb preserved identity/route; native and GPUI renderers consumed `footer-action:actions`; every owned process finalized.
- **Guardrails:** `.artifacts/consistency/UX-004/footer-appkit-helper-bodies-compare.txt` reports exact body equality against `e005f5db2` for all three protected AppKit glyph/padding helpers. No glass motion/material, layout geometry, optical offset, or generated token changed. Positional footer ID inventory is zero.
- **Cleanup:** Every Driver process exited, streams drained, and log writer closed; exact artifact-path inventory returned `owned_process_count=0`.
- **Commit:** `7c7258950` — `Implement UX-004: drive native and GPUI footer behavior from one action descriptor`.

### C08 — UX-005 typed semantic chip roles

- **Status:** Complete; model, direct and legacy keyboard, strict semantics, real pointer, negative controls, build, and cleanup proof green.
- **Implementation:** Added validated `SemanticChipRole`, `SemanticChipAction`, and `SemanticChipSpec` plus a unique-ID `MainViewContextZoneSpec`. Main CWD/model/Quick AI/selection chips and Agent Chat identities now advertise only role-valid actions and executable keycaps. Quick AI has a distinct ID. Unavailable identities are inert with reasons. Selection context opens safe focused-text details with no instruction, so body activation cannot remove context or submit a rewrite. `AiContextPart` projects stable redacted IDs from kind/source/label rather than content or position. `getElements` projects the same typed main-zone model consumed by rendering.
- **Decision branch:** The selected-context semantic element and visible screenshot were present, but nested chip fidelity IDs were not projected into the parent node list. The proof retained strict `getElements` assertions and crossed the real product boundary with a bounded native macOS click at the deterministic visible chip center. The first click may only activate the window; the final green run stopped after the second click emitted `selection_context_details_opened`. This was treated as activation behavior, not weakened into a source-only assertion.
- **Focused receipts:**
  - `.artifacts/consistency/UX-005/main-view-chrome-tests-final.log` — 14 passed.
  - `.artifacts/consistency/UX-005/message-parts-tests-final.log` — 33 passed.
  - `.artifacts/consistency/UX-005/check-lib-final.log` and `build-final.log` — finished; binary SHA-256 `89a6c4de4f8504fea1bc1781a1f1cf9492f04b99fb20df7bae68ea93f5d5879a`.
- **Runtime receipt:** `.artifacts/consistency/UX-005/runtime-proof.json` — `RUNTIME-CONFIRMED`; normal identities, Tab-only CWD, Shift+Tab-only model, distinct Quick AI, unavailable direct/legacy inertness, safe context-body details, and all owned-process cleanup assertions are true.
- **Negative controls:** Role/action matrices reject context submission/removal from the body, identity removal, and destination side effects. Disabled chips cannot carry actions/keycaps. Duplicate zone IDs fail. IDs exclude raw context body bytes and removability. Inventory reports zero old selected-text instant-submit routes, positional pending-context IDs, external legacy context models, and literal `opacity(0.55)`.
- **Guardrails:** No global chip store or workflow-lifetime migration. Agent Chat preserves composer Tab ownership and explicit context-removal affordances. No glass-motion, material, geometry, generated-token, or native optical value changed.
- **Cleanup:** Eight Driver finalizations passed; exact artifact-path process inventory returned `owned_process_count=0`.
- **Commit:** `560c534a7` — `Implement UX-005: type context, identity, and destination chip roles and actions`.

### C09 — UX-011 shared form-field shell

- **Status:** Complete; model, renderer, real input interaction, validation/disabled semantics, visual, build, inventory, and cleanup proof green.
- **Implementation:** Added validated `FormFieldShellSpec`, explicit neutral/valid/invalid state, shared token-derived style, one border/background/padding/radius owner, stable label/surface/message IDs, and visible supporting copy. Migrated `FormTextField`, `FormTextArea`, and menu-syntax InputState bodies. Menu syntax now maps domain snapshots once for rendering and `getElements`, disables internal input chrome, traverses only editable fields, blocks disabled direct edits, submits focused single-line fields on plain Enter, and preserves multiline Shift+Enter newlines.
- **Decision branches:** The first bin test exposed that `app_layout` is compiled into both lib and bin while `render_script_list` is not a bin crate-root module. The menu-domain-to-shell adapter therefore moved into `components::form_fields`, where both renderer and collector consume it. A disabled runtime run then showed Tab returned to the editable main command input, so ordinary text changed the canonical command rather than the disabled field. The final negative control uses the direct field-edit boundary and adds editable-only traversal/direct-update guards; disabled fields expose no caret, focus, semantic selection, or mutation.
- **Test cleanup:** Deleted unwired duplicate `src/components/form_fields/tests.rs`. Replaced two source-reading typography/layout tests with model/behavior tests and pruned renderer-expression assertions that had broken on the legitimate shared-shell refactor. Regenerated `tests/source_audit_inventory.md`; reader sites are 2,819 and the base guard reports no additions.
- **Focused receipts:**
  - `.artifacts/consistency/UX-011/form-fields-tests-final.log` — 27 passed.
  - `.artifacts/consistency/UX-011/render-script-list-tests-second.log` — 14 passed on the binary target.
  - `.artifacts/consistency/UX-011/menu-syntax-contract-tests-final-2.log` — 21 passed.
  - `.artifacts/consistency/UX-011/check-lib-final-2.log` and `build-final.log` — finished; binary SHA-256 `b4be26abea4ef8227ae8b2019976dccc2a7bb9e45f4f139f01a1aebfe223faf5`.
  - `.artifacts/consistency/UX-011/source-audit-inventory-check-final.log` — no new guarded reader sites relative to `main`.
  - `.artifacts/consistency/UX-011/hardcoded-visual-check-final.log` — no hardcoded visual additions relative to `main`.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-011/runtime-proof.json` — `RUNTIME-CONFIRMED`; Tab/Shift+Tab, Unicode paste, single-line submit, multiline newline retention, invalid supporting copy, disabled inertness, and all owned-process cleanup assertions are true. Four inspected screenshots are in the same artifact directory.
- **Negative controls:** No inner input border, local menu border/background token reads, blank identity/reason/message, inverted heights, disabled+validation fiction, disabled focus/update, source-reader additions, hardcoded visual additions, or duplicate test owner survives.
- **Guardrails:** Menu domain behavior and general editor implementation remain separate inside one shell; no glass, motion, popup, footer, native optical, or generated-token value changed.
- **Cleanup:** Four Driver finalizations passed; clipboard bytes restored in `finally`; exact artifact-path process inventory returned `owned_process_count=0`.
- **Commit:** `776d0ace9` — `Implement UX-011: render menu-syntax fields through the shared form-field shell`.

### C10 — UX-012 Actions search InputState ownership

- **Status:** Complete; model, native input, parent/legacy routing, build, runtime, visual, and cleanup proof green.
- **Implementation:** `ActionsDialog` now installs and renders a real entity-backed `gpui_component::InputState`, mirrors Change events into filtering/semantic state, and forwards programmatic parent-window edits through the same cursor/selection/history owner. Detached, main-hosted, CommandBar, Notes, Day Page, Path prompt, automation, and legacy keyboard routes no longer edit `search_text` directly. Route restoration and batch `setInput` reconcile into the live entity. The dead compact fake-cursor renderer was removed.
- **Local escalation:** The first live render aborted because gpui-component Input assumed every host was wrapped in `Root`; the Actions popup intentionally is not, because Root's background breaks native vibrancy. Input now makes only the optional shared `focused_input` slot conditional while preserving `Window::handle_input`. The next live pass showed native Input and popup host both applying local text; local popup editing now stays with focused InputState while host routes retain navigation/action shortcuts. A final pass showed Tab arriving as a control character; Actions now consumes named Tab and InputState is configured for tab navigation.
- **Focused receipts:** `input-owner-test-post-runtime-fix.log` PASS (real cursor/selection/replacement/history); `window-lifecycle-final-2.log` PASS (10); `command-bar-final-2.log` PASS (16); `build-final-2.log` PASS; source-reader and hardcoded-visual guards report no additions.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-012/runtime-proof.json` — `RUNTIME-CONFIRMED`; Unicode typing, UTF-8 cursor offsets, punctuation insertion, selection replacement, undo/redo, safe paste, named-key rejection, parent focus return, synchronization, screenshot, clipboard restoration, and exact process cleanup all passed.
- **Negative controls:** No production manual string editor, fake compact cursor renderer, duplicate local edit, named-key insertion, paste activation, stale post-dismiss mutation, missing-owner automation success, visual hardcode, source-reader addition, or owned process survives.
- **Guardrails:** Actions keeps Up/Down/Home/End/Page/Enter/Escape/Cmd+K/row-shortcut ownership and calibrated non-activating transparent popup behavior. Glass/motion/material/geometry/footer/native optical/generated-token values are untouched.
- **Commit:** `5a931dce2` — `Implement UX-012: move Actions search editing to the existing input owner`.

### C11 — UX-013 top search and fixed Actions shell

- **Status:** Complete for the UX-013 product contract; focused, build, runtime/visual, Actions-entry glass, rapid-toggle, inventory, cleanup, and local adversarial evidence are green. The broader main/Notes lifecycle filmstrip observer deviation is preserved as an explicit non-product-owned receipt.
- **Implementation:** Deleted `SearchPosition::Bottom` so `Top | Hidden` is compiler-enforced. Detached outer size is resolved once from root unfiltered actions and opening chrome inputs, stored in `ActionsWindow`, projected as fixed interior height into `ActionsDialog`, and exposed with the one-lifetime generation in automation. Top search and a non-content-clamped fixed viewport render inside that shell. All filter/edit/route/action-list resize calls and the dead resize APIs, re-exports, automation updates, helpers, event, and source-reader resize test are gone.
- **Decision branch:** Full-tree inventory found zero true parent/display geometry callers; the old resize API family was deleted rather than renamed. The action-refresh protocol branch was unavailable for an already-open exact target, so command-bar/action replacement stays behavior-tested and compiler-guarded by the absence of a resize API; no new protocol was added solely for this task. Hidden remains the intentional external-search configuration and installs no local InputState.
- **Focused receipts:** `actions-lib-tests.log` → 6,202/0; `ux13-lib-tests-first.log` → 5/0; `config-matrix-tests.log` → 46/0; `ux12-inputstate-bin-test.log` → 1/0; `check-lib-first.log`, `check-bin.log`, and `product-build.log` finished; glass static 40/0 and calibration fixture 1/0; source-audit/hardcoded-visual guards report no additions.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-013/runtime-proof.json` is `RUNTIME-CONFIRMED`. Main, Agent Chat, and Notes each retained one exact rect, target, and generation through their first lifetime while editing/zero results, route push/pop, and filter/clear changed content. Wrong ID/generation, stale state, and stale dispatch failed; reopen advanced generation 1→2. Three inspected PNGs show one top search row and no clipping/duplicate search.
- **Negative controls:** No Bottom mode, content resize API, fake editor, hidden InputState, weakened assertion, ignored test, enlarged tolerance, fixture update, protected glass/token diff, source-reader addition, or hardcoded visual addition survives. The runtime comparator fails any x/y/width/height delta over 0.25 logical px.
- **Guardrails:** Existing anchors and `WindowPosition` placement, UX-012 InputState, navigation/action shortcuts, routes, transparent non-Root vibrancy, selection/scroll logic, glass calibration, footer optics, and generated tokens are unchanged. Actions-entry and rapid-toggle probes pass. Two full lifecycle observer captures remained red only on unchanged main-entry/Notes pre-reveal sampling; they cleaned up with zero interference and were not papered over by changing product or observer thresholds.
- **Cleanup:** Every runtime Driver closed and reported dead, clipboard bytes restored by SHA, no signals or broad kill were used, stale registries were removed, parent targets remained live, and exact artifact-path process count is zero.
- **Commit:** `5ef3c45de` — `Implement UX-013: place searchable Actions at the top and freeze popup bounds`.

### C12 — UX-014 attached popup lifecycle

- **Status:** Complete for the UX-014 product contract; committed and verified.
- **Implementation:** Added a monotonic generation-scoped lifecycle in `components::inline_popup_window` with hidden/non-key creation, three deferred parent-readiness turns, exact GPUI parent runtime-handle and AppKit parent-pointer verification, attach receipts, one idempotent close gate, exact prior-focus tokens, GPUI-plus-native-key focus-pair arming for nonactivating panels, and generation-conditional cleanup. Protocol schema v2 now carries exact popup instances through registry, runtime handles, semantic caches, screenshot/event dispatch, and PromptPopup batch routing. Agent Chat history and Dictation microphone own one exact lifetime each and centralize parent-owned versus child-owned close so callbacks cannot double-lease their parent entity. The obsolete Agent Chat popup registry module and brittle lifecycle source-reader assertions were deleted.
- **Native close decision:** Borderless AppKit `performClose:` was a no-op without the closable mask, while synchronous `close` re-entered GPUI. The final exact-target path releases the GPUI borrow, schedules on the foreground executor, adds only the closable behavior bit, and calls `performClose:` so GPUI's should-close callback reconciles the owner.
- **Build and focused receipts:** Final check/build succeeded via `agent-cargo`; stable binary `target-agent/artifacts/ux14-popup-life/script-kit-gpui`, SHA-256 `a2e667d19f8e5bc0b3c558995b93df1de69a96c241862ecbf6fa8b322eecc4c3`. Shared lifecycle tests passed 2/2 and retained Dictation contracts passed 3/3. Source-audit inventory shrank app-source sites `2331 → 2330` and total sites `2819 → 2818`; `git diff --check` passes. A redundant cold focused-test rebuild was terminated with exit 143 by the repository low-disk watcher after 7m38s; the infrastructure failure is preserved and was not bypassed with bare Cargo.
- **Runtime/visual receipts:** `.artifacts/ux14-popup-life/runtime-agent-history.json` and `runtime-dictation-microphone.json` are green against the final artifact. Together they prove exact parent/child identity; strict semantics and screenshots; one-layer parent Escape; clean generation advance; stale-target refusal; parent outside-click; native AppKit close reconciliation; exact parent focus after Escape/outside/native close; Agent Chat Unicode insertion at the exact composer caret after all three routes; Dictation exact subtype batch selection; no-persistence fixture bytes; and full process/stream/log cleanup. Screenshots are `agent-history-popup.png` (`415×463`) and `dictation-microphone-popup.png` (`317×80`).
- **Negative controls:** Stale/fabricated generations return zero elements with an explicit stale-instance warning; events never fall back to the parent; delayed generation N cleanup cannot remove N+1; missing/mismatched parent remains hidden; native close accepts only an exact live PromptPopup instance; fixture microphone selection cannot write config; no second child is admitted while Closing.
- **Glass/Actions guardrails:** Protected glass source diff is empty and static tests pass 40/40. Final Actions-entry and rapid-toggle probes pass. The unchanged broad lifecycle observer remains `EVALUABLE_FAIL` on the known UX-013 main-entry/Notes capture cadence, and direct Actions inspector retries remain fail-closed on target ambiguity/stale-view instrumentation; neither was hidden or converted to green by changing product values or thresholds.
- **Cleanup:** Both final Driver receipts report process exited, streams drained, log writer closed, exact owned process/child count zero, and clipboard untouched. Independent exact-path process inventory also reports zero; named Actions sessions were stopped without broad signals.
- **Intentional differences preserved:** Actions remains independent; Confirm remains generationless and parent-key-routed; menu syntax remains a main-list projection; footer retains compatibility attach; UX-012 InputState, UX-013 fixed geometry/top search, host shortcuts/routes, and all locked glass values remain unchanged.
- **Commit:** `90d2dae90` — `Implement UX-014: enforce generation-scoped popup lifecycle and exact focus return`.

### C13 — UX-016 keyboard-operable, uniquely identified feedback controls

- **Status:** Complete; focused model/dispatch tests, product build, real runtime/visual proof, anti-drift, and cleanup green.
- **Implementation:** Script Kit Buttons now require non-empty stable IDs independent from labels, use keyed focus handles, register enabled interactive controls as real tab stops, and activate once on Enter/Return/Space while disabled/loading states stay inert. Typed `ToastId`/`ToastActionId` identities drive root/action/dismiss IDs; ToastManager preserves the full Toast model into entity-backed custom notifications. Prior focus is restored after action/dismiss, exact remaining auto-hide time pauses while controls contain focus, stale entity timers cannot dismiss replacements, and dismiss is visible at rest. Shortcut Recorder Tab traversal derives only eligible Save/Clear/Cancel actions. The main launcher now renders the Root notification layer.
- **Local escalation:** Main-window Tab/Shift+Tab are established launcher/Profile commands, so an attempted notification-first Tab/Control-F6 routing path was removed rather than overriding host semantics. The retained reusable fix is actual Button tab-stop registration. Keyboard behavior is proven through real GPUI dispatch at the reusable Button/notification/recorder layers; the product runtime crosses the real launcher boundary for simultaneous duplicate Toast rendering, exact action/dismiss activation, lifetime removal, focus return, screenshot, and cleanup.
- **Focused receipts:** Button `16/16`; Toast `8/8`; Shortcut Recorder `16/16`; ToastManager `9/9`; vendor Notification `4/4`; `agent-cargo check --lib` finished. Negative controls cover blank IDs, duplicate copy with distinct identity, disabled/loading inertness, unavailable recorder actions, exact-duration pause, focus return, and stale-timer replacement safety.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-016/runtime-keyboard-feedback.json` is `RUNTIME-CONFIRMED`; seven stable control IDs coexist across two identical messages and three identical action labels; one action and one dismiss each remove only their exact Toast; `input:filter` is focused before, after action, and after dismiss. Screenshot: `.artifacts/consistency/UX-016/runtime-keyboard-feedback.png` (`750×501`).
- **Build:** `target-agent/artifacts/ux16-keyboard-feedback/script-kit-gpui`; SHA-256 `199911ec10b5c3d3876a046559f69ff12c6d963feae9339fbdcd3277ca2d7398`.
- **Audit/guardrails:** No temporary F6/notification Tab route remains; host keyboard routing is unchanged; all Script Kit Button and ToastAction call sites supply stable IDs; no source-reader/count audit was added; protected glass owner diff is empty; static glass tests pass `40/40`; calibration fixture passes `1/1`.
- **Cleanup:** Final Driver reports process exited, streams drained, log writer closed, and exact artifact-path `ownedProcessCount:0`; clipboard untouched and no broad signal used.
- **Commit:** This section is committed atomically with `Implement UX-016: require stable control IDs and keyboard-operable toast and shortcut feedback`; use `git log -1 --oneline` for the immutable hash.
