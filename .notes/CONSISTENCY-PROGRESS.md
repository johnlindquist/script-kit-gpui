# Script Kit Consistency Implementation Progress

> Governing goal: follow all default recommendations in `.notes/CONSISTENCY-FIXES.md` using three parallel Oracle-guided execution lanes. Every completed recommendation must record changed behavior, proof, and exact user testing steps here.

## Status

- Branch: `consistency/default-recommendations`
- Baseline commit: `e20590073` — visual review explorer
- Recommendation coverage: 16 / 75 implemented and verified in this execution pass
- Oracle execution lanes: three plans complete through protocol v2; implementation active
- Maximum concurrent Oracle consults: 3
- Product push/deploy: not authorized
- Locked glass calibration: protected; no retune authorized
- Published before/after progress explainer: https://lemon-ether-2sj4.here.now/ (authenticated, permanent)

## Lanes

| Lane | Scope | Tasks | Status |
|---|---|---:|---|
| Core UX | cues, actions, context semantics, rows, inputs, popups, states, state ownership | 19 | C01–C13 complete; C14 pending |
| Workflow safety | AI preparation, conversations, Flow, Notes/Today, Dictation | 28 | Plan complete; C01 starting |
| Proof and governance | report truth, evidence, accessibility, geometry, design contracts, owner maps, glass documentation | 28 | C01 complete; C02 starting |

## How to view the baseline proposal explorer

1. From the repository root, run `python3 -m http.server 4173`.
2. Open `http://127.0.0.1:4173/design/consistency/`.
3. Choose a group to inspect the current source-derived behavior and proposed contract.
4. Product implementation status is tracked below; proposal cards remain labeled `PROPOSAL · NOT IMPLEMENTED` until their corresponding recommendation is verified.

## Completed recommendations

### RPT-001 — Publish evidence status and corrected inventory language

- **Status:** Complete
- **Changed behavior:** DevTools surface and coverage inventories now report contract kinds, kind-to-AppView mappings, unique AppView variants, runtime coverage profiles, and non-counting orientation aliases as separate namespaces. The surface report reads the real compatibility index instead of an empty string and explicitly reports that its maintained atlas target is currently missing. The audit labels all 15 findings and all 75 tasks with evidence status, treats 74/100 as qualitative only, and stops citing `/tmp` working files as durable proof.
- **Exact owners:**
  - `scripts/devtools/surfaces.ts::buildReport`
  - `scripts/devtools/coverage.ts::filteredCoverage`
  - `scripts/devtools/surface.test.ts`
  - `.notes/CONSISTENCY.md`
  - `.notes/CONSISTENCY-FIXES.md`
- **Commit:** `9206bb6fe` — `Report contract mappings, variants, profiles, and aliases as separate DevTools inventory namespaces`
- **Focused verification:** `bun test scripts/devtools/surface.test.ts` → PASS (2 tests, 12 expectations).
- **Runtime/source receipts:**
  - `.artifacts/consistency/RPT-001/surfaces.json` → 37 kinds, 54 mappings, 53 unique variants, 11 runtime profiles, 4 non-counting aliases.
  - `.artifacts/consistency/RPT-001/coverage.json` → 11 runtime profiles: 1 supported, 9 partial, 1 planned.
  - `.artifacts/consistency/RPT-001/report-truth.json` → PASS; 75/75 tasks have evidence status and proof, 15/15 findings have evidence status.
- **Negative controls:** The report-truth validator fails when a task lacks evidence status/proof or a finding lacks evidence status. A missing maintained feature atlas is reported as `compatibility-index-points-to-missing-atlas`, not silently converted to an empty successful feature map.
- **User test/view:**
  1. Run `bun scripts/devtools/surfaces.ts > /tmp/script-kit-surfaces.json`.
  2. Open `/tmp/script-kit-surfaces.json` and inspect `inventoryNamespaces`; expect `37`, `54`, `53`, `11`, and `4` for kinds, mappings, unique variants, runtime profiles, and aliases respectively.
  3. Inspect `featureMapSource`; expect `status: "compatibility-index-points-to-missing-atlas"` until `feature-map/index.md` is restored.
  4. Run `bun scripts/devtools/coverage.ts > /tmp/script-kit-coverage.json` and inspect `inventoryNamespaces.statusCounts`; expect 1 supported, 9 partial, and 1 planned.
  5. Open `.notes/CONSISTENCY.md`; each `F-###` finding now has an explicit evidence status and the score is labeled qualitative.
- **Intentional differences preserved:** Inventory classes remain separate; an orientation alias is never upgraded into direct runtime coverage, and generated/source projection remains distinct from rendered proof.

### UX-002 — Establish one canonical shortcut token stream

- **Status:** Complete
- **Changed behavior:** `hint_strip::shortcut_tokens_from_hint` is now the sole display-token parser used by GPUI and native/AppKit shortcut consumers. Footer, PromptFooter, Button, `ListItem`, `UnifiedListItem`, Actions, Select, Notes actions, the shortcut recorder, and native footer popup paths now consume or cache the same token vectors rather than maintaining independent character/split rules. `Cmd+K`, `cmd+k`, and `⌘K` resolve identically; literal Plus, Minus, and Backslash survive parsing; F-keys, Page Up/Down, Home/End, Escape, and Enter remain grouped and normalized. Actions no longer splits `⌘F12` into `⌘`, `F`, `1`, `2`.
- **Exact owners:**
  - `src/components/hint_strip.rs::shortcut_tokens_from_hint`
  - `src/components/footer_chrome.rs::split_footer_shortcut`
  - `src/components/prompt_footer.rs`
  - `src/footer_popup.rs` native left-pinned and trailing shortcut consumers
  - `src/components/button/component.rs::Button::{shortcut,shortcut_opt}`
  - `src/list_item/mod.rs::ListItem::{shortcut,shortcut_opt}`
  - `src/components/unified_list_item/{types.rs,render.rs}`
  - `src/actions/types/action_model.rs::Action::{with_shortcut,with_shortcut_opt}`
  - `src/actions/dialog.rs::ActionsDialog::parse_shortcut_keycaps`
  - `src/components/shortcut_recorder/types.rs::RecordedShortcut::to_keycaps`
  - `src/prompts/select/render.rs`
- **Commit:** `195e70e9d` — `Implement UX-002: establish one canonical shortcut token stream across GPUI and AppKit consumers`.
- **Focused verification:**
  - `./scripts/agentic/agent-cargo.sh test --lib shortcut_consumers_share_one_alias_and_literal_key_stream` → PASS (1 cross-consumer matrix test).
  - `./scripts/agentic/agent-cargo.sh test --lib test_split_shortcut_parses_simple_and_complex_keys` → PASS.
  - `./scripts/agentic/agent-cargo.sh test --lib parse_shortcut_keycaps` → PASS (24 Actions parser tests).
  - Existing focused Button, footer, recorder, `UnifiedListItem`, Select, and canonical-parser tests remain green; receipts are under `.artifacts/consistency/UX-002/`.
  - `./scripts/agentic/agent-cargo.sh check --lib` and stable product build → PASS; binary SHA-256 `6cf490d3dd24d27e096d601da53e2165334dfe5785d34fbcf63fdc3821291a1a`.
- **Runtime receipt:** `.artifacts/consistency/UX-002/driver-runtime-proof.json` → `RUNTIME-CONFIRMED`. The exact `actions-dialog` target exposed six shortcut rows from `runtime.footerChrome.shortcutKeycapLayoutModel`; every canonical token matched the layout token and had positive bounds. The Driver closed the app with process, streams, and log writer all finalized.
- **Negative controls / escalation:** The first Actions CLI run omitted the keep-open guard and correctly failed `blocked-by-target-ambiguity`. A guarded run proved target identity and shortcut geometry, while two later subprocess-per-command runs hit the same `response_timeout`. The final proof therefore used the repository’s persistent `Driver`, which crossed the same real product boundary without weakening the fail-closed Actions inspector. No locked AppKit glyph offsets, padding, motion, or glass values changed.
- **User test/view:**
  1. Run `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib shortcut_consumers_share_one_alias_and_literal_key_stream`; expect 1 passed and 0 failed.
  2. Run `bun .artifacts/consistency/UX-002/driver-runtime-proof.ts`; expect `classification: "RUNTIME-CONFIRMED"`, `pass: true`, and six Actions rows whose `shortcutTokens` equal `layoutTokens`.
  3. Launch Script Kit, focus a launcher row, and press `Cmd+K`; verify shortcuts render as separate compact key tokens (for example Command, Shift, and K), not one raw `⌘⇧K` text blob.
  4. Press Escape; the Actions popup should close without changing the launcher selection or running an action.
  5. Open the shortcut recorder, record Command+Plus, and verify it shows a Command token plus one literal `+` token. Repeat with a function key such as F12 and verify `F12` remains one token.
- **Intentional differences preserved:** GPUI and AppKit remain separate renderers. `ListItem` retains its legacy plain-string `↩` compatibility output, while its actual render path consumes cached canonical tokens. Syntax and trigger cues are not reclassified here; UX-001 is the next step that removes `;todo`, `:#work`, `/`, `@`, and “Filter” from shortcut rendering entirely.

### UX-001 — Give guidance cues explicit shortcut, trigger, syntax, and label types

- **Status:** Complete
- **Changed behavior:** `InfoGuidanceItem` no longer stores an overloaded optional shortcut string. Its required `InfoCue` is one of `Shortcut`, `Trigger`, `Syntax`, or `Label`. Shortcut construction caches the UX-002 token vector and canonical route plus a required action ID; validation rejects empty cues, syntax, triggers, labels, and missing action IDs. Agent Chat now renders `/` and `@` as accent monospace triggers, launcher `:#` / `:tag:` / `;todo` / `type:` cues as uninterrupted syntax, and “Filter” as plain label text. Only executable keyboard cues receive keycap anatomy.
- **Exact owners:**
  - `src/components/info_state.rs::{InfoCue,InfoGuidanceItem,InfoCueSemanticSnapshot}`
  - `src/components/info_state.rs::{render_info_cue,info_guidance_cue_slot_width_px}`
  - `src/components/footer_chrome.rs::footer_shortcut_keycaps_measured_width_from_tokens`
  - `src/app_layout/collect_elements.rs::{info_state_elements,menu_syntax_guidance_elements}`
  - `src/components/mod.rs`
- **Commit:** `b7ac0aa8f` — `Implement UX-001 and UX-017: type InfoState cues and make tone semantic without decorative washes`.
- **Focused verification:** `components::info_state` → 16 passed; binary-target InfoState projection → 1 passed; binary-target empty Agent Chat projection → 1 passed; `check --lib` and product build passed. Final binary SHA-256: `96228805b32da827e27219fd11fbc44225bd825aef68df7214dcc8c661680adc`.
- **Runtime receipt:** `.artifacts/consistency/UX-001/runtime-cues-proof-final.json` → `RUNTIME-CONFIRMED`. The actual `MenuSyntaxMainHint` owner exposes `:#work`, `:tag:work`, and `type:` as inert syntax cues; empty Agent Chat exposes `/` and `@` as triggers and `⇧↵`, `⌘P`, `⌘K` as canonical shortcuts. All projected guidance is non-selectable.
- **Negative controls:** `try_shortcut` rejects `;todo`, `:#`, `:tag:`, `type:`, `/`, `@`, “Filter”, empty input, and missing action ID. Snapshot tests require `;todo` to remain one syntax element and prevent non-shortcut cues from acquiring canonical shortcuts/actions.
- **User test/view:**
  1. Launch Script Kit and type `#work` when no tagged result exists; the examples `:#work` and `:tag:work` should read as code/syntax, never keyboard keycaps.
  2. Type `type:script` with an unmatched search word; `type:` and its example remain one uninterrupted syntax token.
  3. Open a clean Agent Chat; `/` and `@` appear as accent triggers with no keycap border, while Shift+Enter, Command+P, and Command+K retain keycap anatomy.
  4. Run `bun .artifacts/consistency/UX-001/runtime-cues-proof.ts`; expect `RUNTIME-CONFIRMED`, all assertions true, and process/stream/log cleanup true.
- **Intentional differences preserved:** Rich menu-syntax guidance remains owned by `MenuSyntaxMainHint` rather than being forced into InfoState. Its semantic projection comes from the same snapshot the rich renderer consumes. InfoState owns compact grouped guidance; both expose the same cue-kind vocabulary.

### UX-017 — Make InfoState tones semantic without decorative washes

- **Status:** Complete
- **Changed behavior:** `resolve_info_tone` maps Neutral, Help, Setup, Permission, Recovery, and About to distinct semantic kinds, accessible prefixes, and shared Lucide icon hints. Tone icon/eyebrow foreground uses theme accent, while the content/background anatomy stays unchanged; every presentation explicitly forbids a full-card background wash. `InfoStateSpec::semantic_snapshot` reports redacted shape, tone, cues, and no invented actions.
- **Exact owners:**
  - `src/components/info_state.rs::{InfoStateTone,InfoTonePresentation,resolve_info_tone}`
  - `src/components/info_state.rs::{render_info_tone_header,InfoStateSpec::semantic_snapshot}`
  - `src/app_layout/collect_elements.rs::info_state_elements`
- **Commit:** Same C02 commit as UX-001.
- **Focused verification:** Tone matrix covers all six tones, distinct Help/Recovery semantics, exact prefixes/icon hints, and `background_wash == false`. Redaction tests prove dynamic launcher query text does not survive the semantic snapshot and unsupported actions stay absent.
- **Runtime/visual receipts:** `.artifacts/consistency/UX-017/tone-visual-proof-final.json` → `RUNTIME-CONFIRMED`. Dark and light captures passed for Help (750×480) and Recovery (340×84); every Driver process, stream, and log writer finalized. Screenshots: `help-dark.png`, `help-light.png`, `recovery-dark.png`, `recovery-light.png` in the same artifact directory.
- **User test/view:**
  1. Open empty Agent Chat in dark appearance; expect an accent Help icon/eyebrow, normal title/body surfaces, and no tinted card wash.
  2. Switch to light appearance and reopen; expect the same semantic hierarchy using the light theme’s accent/foreground values.
  3. Open Actions and type a query with no matches; expect an accent Recovery icon/eyebrow above “No actions match your search,” with the popup surface unchanged.
  4. Run `bun .artifacts/consistency/UX-017/tone-visual-proof.ts`; expect four passing captures and four complete cleanup receipts.
- **Intentional differences preserved:** `ai_recovery` remains the owner of executable retry/auth/open-settings actions. InfoState tone never manufactures recovery buttons or disabled action fiction.

### UX-015 — Remove fake pointer affordances from static hints

- **Status:** Complete
- **Changed behavior:** Static hint strips now render content only: no pointer cursor, hover/active paint, callback, action identity, or root-level pointer interception. Interactive and selectable hints require a non-empty stable action ID plus a non-optional callback before they receive pointer chrome. Positional `hint-btn-{i}` / `hint-click-{i}` / `hint-sel-{i}` identities are gone. The Notes resource preview now constrains its shared surface to the host height so its real clickable hints remain visible rather than falling below the viewport.
- **Exact owners:**
  - `src/components/hint_strip.rs::{HintStrip,HintAction,HintInteractionSnapshot}`
  - `src/components/hint_strip.rs::{render_static_hint_icons,render_static_hint_icons_hsla}`
  - `src/components/hint_strip.rs::{ClickableHint,SelectableHint,render_hint_icons_clickable,render_selectable_hint_icons}`
  - `src/components/prompt_layout_shell.rs::render_universal_prompt_hint_strip_clickable_with_primary_key_label`
  - `src/components/resource_preview.rs::render_resource_preview`
  - `src/notes/window/render_ui.rs::NotesApp::render_kit_resource_preview`
  - `src/notes/window/navigation.rs::NotesApp::automation_kit_resource_preview_state`
- **Commit:** `53ed38109` — `Implement UX-015: make static hint strips inert and reserve pointer chrome for real actions`.
- **Focused verification:**
  - `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib components::hint_strip` → PASS (10 tests), including real GPUI click dispatch: the static hint dispatched zero actions and one click on the interactive hint dispatched exactly once.
  - `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib components::resource_preview` → PASS (4 tests).
  - `./scripts/agentic/agent-cargo.sh check --lib` and stable product build → PASS.
  - Receipts: `.artifacts/consistency/UX-015/{hint-strip-tests.log,resource-preview-tests.log,check-lib.log,build.log}`.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-015/runtime-proof.json` → `RUNTIME-CONFIRMED`. The real Notes target projected `notes-kit-resource-preview-hint-back` as interactive, rendered it in the 350×520 captured window, dispatched a real GPUI pointer click through the exact Notes handle, and closed the preview. Screenshot: `.artifacts/consistency/UX-015/notes-preview.png`. Binary SHA-256: `2b8d208f2666d97bc06a6193ebdad25531814e47368b8742f88bc83f0de0dc8d`.
- **Negative controls:** Empty action IDs panic at construction; `ClickableHint` and `SelectableHint` cannot represent a missing callback; static snapshots contain no action ID or pointer/hover/active/click capability; repository inventory finds no old static `render_hint_icons*`, `on_hint_clicks`, optional `HintClickHandler`, or positional interactive hint IDs.
- **Cleanup:** The Driver process exited, both streams drained, and the log writer closed. A final process check found no `cons-core-hints` Script Kit instance.
- **User test/view:**
  1. Open Notes, enter `kit://scripts`, place the caret in the link, and press Command+Period to open the read-only resource preview.
  2. If desired, resize Notes taller; the `Copy URI` and `Back to Note` hints remain inside the bottom edge instead of falling below the viewport.
  3. Hover `Back to Note`; it should show restrained hover feedback. Click it once; the preview should close exactly once and return to the note.
  4. Open Agent Chat history with Command+P and inspect its explanatory `Type to Search`, arrow-navigation, and Resume hint strip. Those static hints should not gain a pointer cursor or hover/active button background when the pointer passes over them.
  5. Run `bun .artifacts/consistency/UX-015/runtime-proof.ts`; expect `RUNTIME-CONFIRMED`, all assertions true, and complete cleanup.
- **Intentional differences preserved:** Static hints retain the same icon/keycap/text grammar but no control affordance. Interactive hints retain the shared pointer/hover/active paint. Native and GPUI footer systems remain separate; no motion, glass, keycap optical offset, or generated token changed.

### UX-006 — Consolidate row visual-state resolution without changing family geometry

- **Status:** Complete
- **Changed behavior:** Main launcher rows, Unified rows, and dense/soft-compact picker rows now resolve rest, hover, selected, active, disabled, and disabled-selected paint through one renderer-neutral state model. Disabled hover cannot brighten a row; active wins over selected/hover; disabled-selected keeps location feedback. Existing family visuals remain byte-compatible: Main keeps each theme variant’s fill base and alpha, Unified keeps its theme opacity tiers, dense picker selection remains `0.23`, soft compact remains `0.18`, and both picker hovers remain `0.06`.
- **Exact owners:**
  - `src/theme/chrome.rs::{RowStateFlags,RowVisualState,RowStateColors,RowStatePalette}`
  - `src/theme/chrome.rs::{resolve_row_state_palette,row_visual_state_from_flags}`
  - `src/theme/chrome.rs::resolve_main_menu_row_state_palette_from_parts`
  - `src/components/unified_list_item/types.rs::UnifiedListItemColors::row_state_palette`
  - `src/components/unified_list_item/render.rs::UnifiedListItem::render`
  - `src/components/inline_dropdown/row.rs::picker_row_state_palette`
- **Commit:** `b44b4f2eb` — `Implement UX-006: consolidate row visual-state resolution without changing family geometry`.
- **Focused verification:**
  - `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib theme::chrome` → PASS (13 tests).
  - `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib components::unified_list_item` → PASS (16 tests).
  - `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib components::inline_dropdown` → PASS (18 tests).
  - `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib list_item` → PASS (36 tests).
  - `./scripts/agentic/agent-cargo.sh check --lib` and stable product build → PASS.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-006/runtime-proof.json` → `RUNTIME-CONFIRMED`. Dark and light runs exposed five 44px Main rows and the existing Actions 24px header / 36px action-row rhythm. A real GPUI pointer-hover moved Main semantic selection while every row bound stayed identical. Screenshots: `.artifacts/consistency/UX-006/{main-dark,main-light,actions-dark,actions-light}.png`.
- **Negative controls:** Disabled+selected, disabled, active, selected, hover, and rest precedence is table-tested. Disabled hover returns disabled paint. Main, Unified, dense, and soft-compact exact RGBA expectations are locked before/after migration. `SOFT_COMPACT_PICKER_ROW_HEIGHT == 36.0`; no geometry token changed; migrated renderers contain no local `selected_opacity * 255` formula.
- **Cleanup:** Both Driver processes exited, streams drained, and log writers closed; exact artifact-path process inventory returned `OWNED_PROCESSES_CLEAN`.
- **Binary:** `target-agent/artifacts/cons-core-rows/script-kit-gpui`; SHA-256 `7ac8a8408a9db2d3e718bd6261c3286ed64eaec0cedd4f30324a539b33278a24`.
- **User test/view:**
  1. Launch Script Kit in dark appearance and move the pointer across launcher rows; the hovered/selected paint should change without any row, icon, title, or accessory shifting.
  2. Press Command+K; Actions should retain its compact 24px section-header and 36px action-row rhythm while selection remains stronger than hover.
  3. Repeat in light appearance; state ordering should be the same while colors resolve from the light theme.
  4. Open Dictation’s microphone picker or another soft-compact picker; rows remain exactly 36px high, selected is stronger than hover, and no new leading marker appears.
  5. Run `bun .artifacts/consistency/UX-006/runtime-proof.ts`; expect `RUNTIME-CONFIRMED`, all dark/light assertions true, and complete cleanup.
- **Intentional differences preserved:** Main launcher rows remain 44px and retain per-theme text/accent fill rules. Actions keeps its specialized 24px headers and 36px rows. Unified keeps its own density/layout. Soft compact remains 36px. No selection marker was added (UX-007 owns that later), and native footer, glass, motion, typography, icon, and accessory geometry are unchanged.

### UX-010 — Remove false UnifiedListItem API branches and expose retained semantics

- **Status:** Complete
- **Changed behavior:** `UnifiedListItem` can no longer accept custom title, leading, or trailing elements that it silently discarded, and it no longer advertises accessibility label/hint builders that never reached semantics. Its public content model now contains only branches the renderer actually paints. The real Select prompt remains the sole production consumer and still renders its leading cue, title/highlight, subtitle metadata, canonical shortcut, focus/selection state, and host-owned stable semantic identity.
- **Exact owners:**
  - `src/components/unified_list_item/types.rs::{TextContent,LeadingContent,TrailingContent}`
  - `src/components/unified_list_item/render.rs::{UnifiedListItem,render_leading,render_text_content,render_trailing}`
  - `src/prompts/select/render.rs::SelectPrompt::render`
  - `src/components/unified_list_item_tests.rs`
- **Commit:** `37cea6b69` — `Implement UX-010: remove false UnifiedListItem API branches and expose retained semantics`.
- **Focused verification:**
  - `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib components::unified_list_item` → PASS (17 tests).
  - `./scripts/agentic/agent-cargo.sh check --lib` and stable product build → PASS.
  - `.artifacts/consistency/UX-010/inventory.json` → PASS: one production constructor in `src/prompts/select/render.rs`; all six removed API patterns have zero occurrences; no discard arms remain; unwired duplicate test file is gone.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-010/runtime-proof.json` → `RUNTIME-CONFIRMED`. A real Select protocol prompt rendered the production Unified row with stable semantic ID, focused state, leading rocket cue, “Deploy API” title, “Script · Last run 2h ago” subtitle, and canonical Command/Shift/D shortcut. Screenshot: `.artifacts/consistency/UX-010/select-production-row.png`.
- **Negative controls:** `TextContent::Custom`, `TextContent::custom`, `LeadingContent::Custom`, `TrailingContent::Custom`, `.a11y_label(...)`, and `.a11y_hint(...)` are absent repository-wide. Unsupported content cannot compile. No `Custom(_) => div()` or `Custom(_) => None` discard path remains. Semantic identity/action capability stays in the Select host wrapper, where the callback actually lives.
- **Cleanup:** The Driver process exited, streams drained, and log writer closed; exact artifact-path process inventory returned `OWNED_PROCESSES_CLEAN`.
- **Binary:** `target-agent/artifacts/cons-core-unified/script-kit-gpui`; SHA-256 `f531dfea56c35e6b783aaea134a5a2806d0e400d8d525681668b0c6a9d8d4f35`.
- **User test/view:**
  1. Run `bun .artifacts/consistency/UX-010/runtime-proof.ts`; expect `RUNTIME-CONFIRMED`, all assertions true, and complete cleanup.
  2. Inspect `.artifacts/consistency/UX-010/select-production-row.png`; the row should show its leading cue, title, subtitle metadata, and shortcut with no blank/discarded slot.
  3. Launch any real Select prompt and type part of a choice name; matching title fragments remain highlighted while the row’s stable semantic ID and focus state remain observable through `getElements`.
  4. Move focus between rows and press Escape; focus paint changes, the prompt dismisses, and no unsupported component-local action is implied.
- **Intentional differences preserved:** UnifiedListItem remains a presentational row owned by Select. Its host wrapper continues to own stable semantic identity, hover/click callbacks, and activation. `ListItem` remains the launcher-family row; no third row family or speculative accessibility/action model was added.

### UX-003 — Make Actions availability explicit and block every execution route consistently

- **Status:** Complete
- **Changed behavior:** Every `Action` is explicitly enabled by default or disabled through a validated builder that requires a non-blank reason. Disabled Actions stay visible and selectable so the user can read why they are unavailable, but Enter, two-click pointer activation, direct action ID, and configured shortcut routes all return a typed blocked outcome without executing a callback or closing the popup. Disabled and duplicate shortcuts are not displayed as executable promises. The disabled reason occupies one 132px, single-line trailing slot without changing the 36px row height.
- **Exact owners:**
  - `src/actions/types/action_model.rs::{ActionAvailability,Action::disabled,Action::is_enabled,Action::disabled_reason}`
  - `src/actions/dialog.rs::{ActionsDialogActivation,ActionsDialog::activate_action_id,action_canonical_shortcut,visible_action_shortcut_bindings,action_has_routable_shortcut}`
  - `src/actions/dialog.rs::{append_disabled_action_test_fixture,ActionsDialog::render}`
  - `src/actions/window.rs::{ActionsWindow::handle_dialog_activation,activate_detached_actions_window_action}`
  - `src/app_impl/actions_dialog.rs::ScriptListApp::handle_actions_dialog_activation`
  - `src/main_entry/app_run_setup.rs::ExternalCommand::TriggerAction`
  - `src/windows/automation_surface_collector.rs::collect_actions_dialog_elements`
- **Commit:** `e005f5db2` — `Implement UX-003 and UX-009 atomically: make Actions availability explicit and selection eligibility safe`.
- **Focused verification:**
  - `.artifacts/consistency/UX-003/disabled-action-tests-final.log` → PASS (5 tests): enabled default, required reason, blank-reason rejection, shortcut suppression, and selected/direct callback blocking.
  - `.artifacts/consistency/UX-003/direct-close-policy-test.log` → PASS (1 test): direct-ID activation uses the activated action’s close policy rather than borrowing it from the selected row.
  - `.artifacts/consistency/UX-003/check-lib-final.log` and `build-final.log` → PASS; stable binary SHA-256 `7965c81f6547d3cd7c2f203bc748b585d88f9951b988be7815e53da9cd0d3e4a`.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-003/runtime-proof.json` → `RUNTIME-CONFIRMED`, all 13 assertions true. A real detached Actions window exposed the disabled reason through state and `getElements`, hid the configured shortcut, blocked direct ID/Enter/two-click activation, ignored the hidden key chord, stayed open after every route, and emitted no host/callback execution. Screenshot: `.artifacts/consistency/UX-003/disabled-action-selected.png`.
- **Negative controls:** Blank reasons panic at construction. Disabled and colliding shortcuts are non-routable and absent from row chrome. If the host reports an Actions surface but the detached handle disappears, direct action-ID automation fails closed with `actions_dialog_unavailable` instead of falling through to host execution. Runtime logs contain no `actions_host_execute` or executed outcome for the fixture.
- **Cleanup:** The Driver process exited, streams drained, and the log writer closed. An exact artifact-path process inventory returned `owned_process_count=0`.
- **User test/view:**
  1. Run `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib disabled_action`; expect 5 passed and 0 failed.
  2. Run `bun .artifacts/consistency/UX-003/runtime-proof.ts`; expect `classification: "RUNTIME-CONFIRMED"`, `pass: true`, and all 13 assertions true. The script owns and closes its Script Kit process.
  3. Open `.artifacts/consistency/UX-003/disabled-action-selected.png`; the selected `Unavailable Test Action` row should retain location feedback, show `Requires an unavailable test capability` at the trailing edge, and show no shortcut keycap.
  4. Inspect `activationRoutes` in the JSON receipt; direct ID reports `action_disabled`, Enter and both exact-handle clicks leave the same semantic action selected, and the configured hidden shortcut leaves the popup open.
  5. Inspect `logs.entries`; expect a blocked activation record and no `actions_host_execute` or executed outcome.
- **Intentional differences preserved:** Existing enabled Actions, destructive styling, drill-down behavior, SDK close policy, section/header geometry, and host-specific execution remain unchanged. No production action is disabled speculatively; owners opt in only when they have a real unavailable condition and reason. Glass motion, native popup material, and 24px header / 36px row geometry remain locked.

### UX-009 — Separate focus, selection, and activation eligibility and remove sentinel selection

- **Status:** Complete
- **Changed behavior:** Shared `RowEligibility` now distinguishes `focusable`, `selectable`, and `activatable`, with constructor invariants `activatable ⇒ selectable ⇒ focusable`. Actions section headers are inert, enabled Actions are focusable/selectable/activatable, and disabled explanatory Actions are focusable/selectable/non-activatable. `ActionsDialog.selected_index` is now `Option<usize>`; `None` is the only no-selection state. Refreshes restore semantic action identity, keep a selected action selected when it becomes disabled, choose the nearest eligible visual row when an action disappears, and return to `None` when no selectable row exists. Actions and Command Bar navigation use the shared eligibility helpers rather than maintaining separate index rules.
- **Exact owners:**
  - `src/list_item/mod.rs::{RowEligibility,first_selectable_eligibility_index,last_selectable_eligibility_index,coerce_eligibility_selection}`
  - `src/list_item/mod.rs::{grouped_list_item_eligibility,ListItem::disabled}`
  - `src/actions/dialog.rs::{grouped_action_item_eligibility,initial_selection_index,ActionsDialog::apply_refresh_selection,ActionsDialog::restore_selected_action_id}`
  - `src/actions/command_bar.rs`
  - `src/actions/window.rs`
  - `src/theme/chrome.rs::MainMenuRowStatePalette::for_flags`
  - `src/list_item_tests.rs`
  - `src/actions/tests/dialog_runtime_path_tests.rs`
- **Commit:** Same atomic C06 commit as UX-003: `e005f5db2`.
- **Focused verification:**
  - `.artifacts/consistency/UX-009/row-eligibility-tests-final.log` → PASS (4 bin-target tests), including both impossible-state panic controls.
  - `.artifacts/consistency/UX-009/no-selection-test-final.log` → PASS (1 test): an empty Actions dialog stores `None` and activation returns `NoSelection`.
  - `.artifacts/consistency/UX-003/refresh-selection-test-final.log` → PASS (1 test): identity restore, selected-disabled preservation, nearest eligible fallback, and empty replacement.
  - Existing focused Actions, Actions-window, Command Bar, ListItem, theme-chrome, and Unified row suites remain green under `.artifacts/consistency/UX-003/` and `UX-009/`.
- **Runtime receipt:** `.artifacts/consistency/UX-009/runtime-proof.json` mirrors the final real-boundary receipt. The selected fixture reports `focusable: true`, `selectable: true`, `activatable: false`; its grouped visual index equals the optional selection index before and after blocked routes.
- **Negative controls:** `RowEligibility::new(true, false, true)` and `RowEligibility::new(false, true, false)` panic. Header-only and empty lists return `None`; no fallback index zero is manufactured. Disabled hover cannot brighten a row, active styling is suppressed, shortcut chrome is absent, and disabled-selected paint preserves location without implying execution.
- **User test/view:**
  1. Run `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --bin script-kit-gpui row_eligibility`; expect 4 passed and 0 failed.
  2. Run `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib refresh_restores_identity_then_uses_nearest_eligible_row`; expect 1 passed.
  3. Run `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib empty_actions_dialog_has_no_selected_row_and_cannot_activate`; expect 1 passed.
  4. Run `bun .artifacts/consistency/UX-003/runtime-proof.ts`, then inspect `fixture.actionSummary` and `fixture.row`; expect focusable/selectable/activatable to be `true/true/false` and `selected: true`.
  5. Inspect the screenshot and receipt after direct ID, click, shortcut, and Enter routes; the disabled row should remain the selected location while never activating or closing the popup.
- **Intentional differences preserved:** Main launcher item rows remain fully activatable; section/status rows remain inert. Actions disabled explanations are intentionally selectable, unlike decorative headers. Main, Unified, dense picker, and Actions row families keep their own geometry and exact paint inputs while sharing only state resolution and eligibility rules.

### UX-004 — Drive native and GPUI footer behavior from one action descriptor

- **Status:** Complete
- **Changed behavior:** Every main-window footer control now carries one validated `FooterButtonConfig` descriptor with a stable semantic ID, executable action route, raw shortcut spelling, canonical token stream, canonical key route, user-facing verb, enabled/disabled reason, selection, and leading/trailing placement. Native AppKit, the GPUI footer overlay, prompt footer rails, Dictation, semantic elements, and `getState.activeFooter` consume that same vector. Click and keyboard paths converge on `dispatch_main_window_footer_action`; ScriptList's Run click now preserves the same menu-syntax, submit-echo, spine, and fallback guards as Enter. Disabled descriptors expose their reason but render/register no key route or callback. Duplicate IDs fail validation; duplicate enabled canonical shortcuts remain diagnostic but hide both keycap promises and routes. Protocol schema 2 exposes this contract directly.
- **Exact owners:**
  - `src/footer_popup.rs::{FooterButtonConfig,FooterPlacement,MainWindowFooterConfig}`
  - `src/footer_popup.rs::{GpuiFooterOverlay::render_button,make_footer_hint_item,apply_footer_descriptor_test_fixture}`
  - `src/components/footer_chrome.rs::render_main_window_footer_config_rail`
  - `src/app_impl/ui_window.rs::{dispatch_main_window_footer_shortcut,dispatch_main_window_footer_action}`
  - `src/app_impl/startup.rs` Enter, Cmd+K, and global shortcut interceptors
  - `src/prompt_handler/mod.rs::active_footer_snapshot`
  - `src/protocol/types/automation_surface.rs::{ActiveFooterSnapshot,ActiveFooterButtonSnapshot}`
  - `src/app_layout/collect_elements.rs` footer semantic projection
  - `src/dictation/window.rs` descriptor-backed footer rail
- **Commit:** `7c7258950` — `Implement UX-004: drive native and GPUI footer behavior from one action descriptor`.
- **Focused verification:**
  - `.artifacts/consistency/UX-004/footer-popup-tests-final.log` → PASS (47 tests), including identity, canonical routing, disabled state, duplicate IDs/shortcuts, label-independent dispatch, and deterministic runtime fixture modes.
  - `.artifacts/consistency/UX-004/footer-chrome-tests-final.log` → PASS (14 tests).
  - `.artifacts/consistency/UX-004/prompt-layout-shell-tests-final.log` → PASS (53 tests).
  - `.artifacts/consistency/UX-004/render-script-list-tests-final.log` → PASS (11 binary-target footer tests).
  - `.artifacts/consistency/UX-004/check-lib-final.log` and `build-final.log` → PASS; stable binary SHA-256 `ca7ea565f5e42c81f09d8e8dc07a7f6df8d7d4aa0b14c33a0ec2235889e4570c`.
- **Runtime receipt:** `.artifacts/consistency/UX-004/runtime-proof.json` → `RUNTIME-CONFIRMED`; all six behavior scenarios and all six top-level assertions passed. A real Cmd+K dispatch and an exact native AppKit click on the descriptor-derived Actions item both opened the same Actions surface. A disabled descriptor blocked key and native click while exposing its reason and no keycaps. A canonical collision exposed two non-routable descriptors, zero keycaps, and no key activation. Renaming `Actions` to `More Actions` preserved `footer-action:actions` and `cmd+k`. The no-glass GPUI overlay exposed `agent-chat.footer-overlay.footer-action:actions` from the same descriptor and opened Actions through an exact-handle GPUI click. `gpuiFallbackVisible` now reports the actual live overlay rather than inferring it from prompt ownership.
- **Negative controls:** Blank explicit IDs and disabled reasons panic. Duplicate IDs panic. Disabled descriptors have no click handler and no keyboard route. Canonical shortcut collisions remain visible for diagnostics in `duplicateShortcutKeys` but are non-routable and paint no keycaps. Repository inventory reports zero positional `config-footer-button-*` / `dictation-footer-action-*` IDs and zero direct reads of the removed `left_pinned` field. The active-footer schema exposes duplicate IDs separately from shortcut collisions.
- **Protected proof:** `.artifacts/consistency/UX-004/footer-appkit-helper-bodies-compare.txt` reports `exact_match=true` against C06 baseline `e005f5db2` for `footer_appkit_glyph_x`, `footer_appkit_glyph_y`, and `footer_keycap_padding_x_for_token`. No glass-motion, material, geometry, optical-offset, or generated-token value changed.
- **Cleanup:** Every runtime Driver finalized its process, stdout/stderr streams, and log writer. Exact artifact-path process inventory returned `owned_process_count=0`.
- **User test/view:**
  1. Run `bun .artifacts/consistency/UX-004/runtime-proof.ts`; expect `classification: "RUNTIME-CONFIRMED"` and every assertion `true`.
  2. Launch Script Kit and press Cmd+K; Actions should open. Press Escape, then click the visible Actions footer control; the same popup should open with the same selected footer state and returned focus behavior.
  3. Inspect `runtime-proof.json` under `scenarios.nativeClick` and `scenarios.key`; both resolve descriptor ID `footer-action:actions`, canonical shortcut `cmd+k`, and an open `actions-dialog`.
  4. Inspect `scenarios.disabled`; the Actions descriptor has `enabled: false`, a non-empty `actionDisabled`, `shortcutRoutable: false`, zero AppKit keycaps, and both key/click checks stay closed.
  5. Inspect `scenarios.collision`; both `cmd+k` descriptors are non-routable, `duplicateShortcutKeys` contains `cmd+k`, no colliding keycap paints, and Cmd+K does not open Actions.
  6. Inspect `scenarios.gpuiFallback`; the live `footer-overlay` uses fidelity ID `agent-chat.footer-overlay.footer-action:actions`, reports `gpuiFallbackVisible: true`, and an exact GPUI click opens Actions.
- **Intentional differences preserved:** AppKit and GPUI remain separate rendering technologies with family-specific measurement; they now share descriptor identity, route, state, and shortcut semantics. Icon-only context chips remain pointer controls without fabricated keyboard routes. Native footer optical centering and the locked glass-motion calibration are unchanged.

### UX-005 — Type context, identity, and destination chip roles and actions

- **Status:** Complete
- **Changed behavior:** Main and Agent Chat chips now expose an explicit semantic role, stable ID, allowed body/trailing action, executable shortcut only when available, and a reason when disabled. Main CWD and model chips are identities that open selectors; Quick AI is a separate identity that opens its surface; selected text is a context attachment whose body opens safe details without removing context or submitting AI work. “No cwd” and unavailable model identities are inert and show no false keycap. Direct and legacy Tab paths consult the same typed action model, and Agent Chat’s model identity follows the live profile-switch capability. Pending Agent Chat context parts use redacted, content-independent IDs rather than positional indices.
- **Exact owners:**
  - `src/components/main_view_chrome.rs::{SemanticChipRole,SemanticChipAction,SemanticChipSpec,MainViewContextZoneSpec}`
  - `src/components/main_view_chrome.rs::{render_main_view_context_zone_required,SemanticChipInvocation}`
  - `src/app_impl/ui_window.rs::{main_view_context_zone_spec,main_view_context_chip_has_action,selection_hint_chip}`
  - `src/app_impl/agent_handoff/focused_text_entry.rs::open_selection_context_details`
  - `src/app_impl/{startup.rs,simulate_key_dispatch.rs,profile_search_view.rs}`
  - `src/ai/message_parts.rs::AiContextPart::semantic_chip_projection`
  - `src/ai/agent_chat/ui/view.rs` Agent Chat identity and pending-context rendering
  - `src/app_layout/collect_elements.rs::main_view_context_elements`
- **Commit boundary:** C08 — `560c534a7` — `Implement UX-005: type context, identity, and destination chip roles and actions`.
- **Focused verification:**
  - `.artifacts/consistency/UX-005/main-view-chrome-tests-final.log` → PASS (14 tests), including the complete body/trailing role-action matrix, disabled invariants, unique-zone IDs, Quick AI/CWD identity separation, and unsupported host trailing-action rejection.
  - `.artifacts/consistency/UX-005/message-parts-tests-final.log` → PASS (33 tests), including redacted stable identity, content-independence, source sensitivity, and capability-neutral removability.
  - `.artifacts/consistency/UX-005/check-lib-final.log` and `build-final.log` → PASS; stable binary SHA-256 `89a6c4de4f8504fea1bc1781a1f1cf9492f04b99fb20df7bae68ea93f5d5879a`.
- **Runtime receipt:** `.artifacts/consistency/UX-005/runtime-proof.json` → `RUNTIME-CONFIRMED`; all seven assertions passed. Normal Main exposes CWD/model identities with Tab/Shift+Tab selector actions; Quick AI has its own `main-view-context-quick-ai-button`; unavailable CWD/model identities are inert through direct and legacy keyboard paths; and a real macOS pointer click opened selected-text details without removal, input mutation, or instant-rewrite submission.
- **Negative controls:** Invalid body and trailing actions are rejected by role. Disabled chips require a non-empty reason and cannot carry callbacks or shortcuts. Duplicate IDs fail main-zone construction. Quick AI cannot reuse the CWD ID. Destination selectors cannot remove context or open identity surfaces. Context bodies cannot remove or submit. Repository inventory reports zero positional Agent Chat context IDs, zero legacy context-label models outside the renderer adapter, zero old selected-text instant-submit route, and zero new literal disabled opacity.
- **Cleanup:** All eight Driver scenarios exited their owned process, drained streams, and closed log writers. `.artifacts/consistency/UX-005/process-cleanup-final.txt` reports `owned_process_count=0` for the exact stable binary path.
- **User test/view:**
  1. Launch Script Kit with a valid working directory and model. With an empty launcher query, press Tab; File Search should open for the CWD identity. Return to Main and press Shift+Tab; the Agent/Model selector should open.
  2. Type `explain rust lifetimes`; the leading chip becomes Quick AI with its own identity. Press Tab; Quick AI opens rather than File Search.
  3. Run `SCRIPT_KIT_TEST_STATUS=1 SCRIPT_KIT_TEST_CONTEXT_CHIP_FIXTURE=unavailable target-agent/artifacts/cons-core-context/script-kit-gpui`, inspect the leading/trailing identities, and verify they show no Tab/Shift+Tab keycap and neither pointer nor keyboard opens a selector.
  4. Run `SCRIPT_KIT_TEST_STATUS=1 SCRIPT_KIT_TEST_CONTEXT_CHIP_FIXTURE=selection target-agent/artifacts/cons-core-context/script-kit-gpui`; click the `Selected: …` chip body. Focused-text details should open while the chip remains attached and no rewrite is submitted.
  5. Run `bun .artifacts/consistency/UX-005/runtime-proof.ts`; expect `classification: "RUNTIME-CONFIRMED"` and all seven assertions `true`. The macOS selection-body proof may use a second bounded click when the first click only activates the window.
- **Intentional differences preserved:** The main context zone currently has no trailing removal affordance, so it rejects removable context specs rather than silently dropping that action. Agent Chat pending context chips keep their explicit trailing remove control. Main CWD is intentionally inert inside Agent Chat because plain Tab belongs to the composer. Context provenance/lifetime remains workflow-owned; no global chip store was added. Native footer optics, glass motion/material, geometry, and generated tokens are unchanged.

### UX-011 — Render menu-syntax fields through the shared form-field shell

- **Status:** Complete
- **Changed behavior:** Protocol `FormTextField`, `FormTextArea`, and menu-syntax capture fields now use one validated shell for stable identity, label anatomy, padding, radius, background, focused/idle/disabled/invalid border roles, placeholder/value/disabled colors, height constraints, and visible supporting copy. Menu-syntax fields retain their real entity-backed `InputState` bodies with internal borders disabled, so there is one field border rather than nested chrome. Required invalid fields show an explicit message instead of relying on border color; unavailable fields are dimmed, explained, non-focusable, and reject direct field edits. Tab and Shift+Tab traverse only editable fields. Plain Enter from a focused single-line field now submits through the existing form owner; Shift+Enter in a multiline snippet field keeps its newline and does not submit.
- **Exact owners:**
  - `src/components/form_fields/shell.rs::{FormFieldShellSpec,FormFieldValidation,FormFieldShellStyle}`
  - `src/components/form_fields/shell.rs::{render_form_field_shell,resolve_form_field_shell_style,menu_syntax_form_field_shell_spec}`
  - `src/components/form_fields/colors.rs::{FormFieldColors,FormFieldMetrics}`
  - `src/components/form_fields/text_field/render.rs::FormTextField::render`
  - `src/components/form_fields/text_area/render.rs::FormTextArea::render`
  - `src/render_script_list/mod.rs::{render_menu_syntax_form_field,render_menu_syntax_form}`
  - `src/app_impl/menu_syntax_main_hint.rs` InputEvent submission, editable-only focus traversal, and direct-edit guard
  - `src/app_layout/collect_elements.rs::collect_script_list_elements`
- **Commit boundary:** C09 — `776d0ace9` — `Implement UX-011: render menu-syntax fields through the shared form-field shell`.
- **Focused verification:**
  - `.artifacts/consistency/UX-011/form-fields-tests-final.log` → PASS (27 tests), including neutral/valid/invalid distinction, mandatory disabled and invalid copy, stable anatomy IDs, metric-derived heights, and fail-closed impossible states.
  - `.artifacts/consistency/UX-011/render-script-list-tests-second.log` → PASS (14 binary-target renderer tests).
  - `.artifacts/consistency/UX-011/menu-syntax-contract-tests-final-2.log` → PASS (21 maintained handler-form contracts).
  - `.artifacts/consistency/UX-011/check-lib-final-2.log` and `build-final.log` → PASS; stable binary SHA-256 `b4be26abea4ef8227ae8b2019976dccc2a7bb9e45f4f139f01a1aebfe223faf5`.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-011/runtime-proof.json` → `RUNTIME-CONFIRMED`; all six assertions passed. A real GPUI Tab focused Todo body, Unicode clipboard paste produced `Réviser café ☕`, Tab/Shift+Tab moved Body → Tags → Body, and Enter submitted/reset the form. In Snippet, Shift+Enter produced `line one\nline two` without submission. Disabled fields exposed reasons, accepted no focus or direct update, and invalid fixtures exposed semantic validation. Screenshots: `menu-form-normal.png`, `menu-form-multiline.png`, `menu-form-disabled.png`, and `menu-form-invalid.png`.
- **Negative controls:** Disabled specs without reasons, invalid specs without messages, invalid+disabled combinations, blank IDs/labels, and inverted min/max heights fail construction. Menu inputs retain `.appearance(false).bordered(false).focus_bordered(false)` inside the shell; the local menu renderer contains zero border calls and zero old `main_hint_form_*` visual-token reads. The unwired duplicate `src/components/form_fields/tests.rs` was deleted. Brittle render-string assertions were pruned in favor of behavior tests; `tests/source_audit_inventory.md` was regenerated and the ordinary-PR reader guard reports no additions. The hardcoded-visual ratchet reports no additions.
- **Cleanup:** All four final Driver scenarios exited, drained streams, and closed log writers. The exact stable artifact path reports `owned_process_count=0`; the probe restored the pre-existing system clipboard bytes in `finally` without logging them.
- **User test/view:**
  1. Launch Script Kit and type `todo; `. The `Task *` field should use the same label/surface anatomy as other form fields and show a visible `Required` message while empty.
  2. Press Tab to focus Task, paste `Réviser café ☕`, then press Tab and Shift+Tab. Focus should move Task → Tags → Task, the pasted Unicode should remain intact, and Task should change from invalid to neutral.
  3. Press Enter from Task. The Todo capture should submit through the normal owner and the launcher input should reset; paste alone must not submit.
  4. Type `snippet; `, press Tab, type `line one`, press Shift+Enter, then type `line two`. The body remains focused and visibly contains two lines; it must not submit until plain Enter is used with all required fields satisfied.
  5. Run `bun .artifacts/consistency/UX-011/runtime-proof.ts`; expect `classification: "RUNTIME-CONFIRMED"`, every assertion `true`, and `ownedProcessCount: 0`.
  6. Inspect the disabled/invalid screenshots: disabled fields have dimmed copy plus an explicit reason and no caret; invalid fields have both the error border and textual message.
- **Intentional differences preserved:** Menu-syntax continues to own parsing, field order, suggestions, canonical filter synchronization, submission, and scroll reveal. General protocol fields keep their custom char-indexed editors; menu syntax keeps `gpui_component::InputState` for native selection, clipboard, undo, and IME. Single-line and multiline bodies share shell anatomy but retain distinct height and Enter behavior. No popup, footer, glass, motion, native optical, or generated design-token value changed.

### UX-012 — Move Actions search editing to the existing input owner

- **Status:** Complete
- **Changed behavior:** Actions search now renders and edits through one entity-backed `gpui_component::input::InputState`. Unicode insertion, cursor movement, range selection/replacement, Backspace/word deletion, clipboard paste, undo/redo, IME registration, and route/automation replacement all reconcile through that owner. Main-window, detached-popup, CommandBar, Notes, Day Page, Path prompt, and legacy `simulateKey` routes keep navigation/action shortcuts but no longer mutate an independent search string. The popup consumes named Tab rather than inserting its control character, and local native input events are not applied a second time by the popup host.
- **Exact owners:**
  - `src/actions/dialog.rs::{ensure_search_input,edit_search_input,sync_search_input_from_model,automation_state,Render}`
  - `src/actions/window.rs::{ActionsWindow::handle_key_event,open_actions_window,set_actions_dialog_search_text}`
  - `src/app_impl/actions_dialog.rs::route_key_to_actions_dialog`
  - `src/actions/command_bar.rs::{CommandBarKeyIntent,CommandBar::handle_char,CommandBar::move_search_cursor}`
  - `vendor/gpui-component/crates/ui/src/input/{state.rs,element.rs}` programmatic edit forwarding and transparent-popup Root tolerance
  - parent and legacy routes in `startup_new_actions.rs`, `simulate_key_dispatch.rs`, `render_prompts/path.rs`, `main_sections/day_page_switcher.rs`, and Notes keyboard/navigation
- **Commit boundary:** C10 — `5a931dce2` — `Implement UX-012: move Actions search editing to the existing input owner`.
- **Focused verification:**
  - `.artifacts/consistency/UX-012/input-owner-test-final.log` and `input-owner-test-post-runtime-fix.log` → PASS; real GPUI InputState cursor, selection, replacement, undo, redo, and filtering behavior.
  - `.artifacts/consistency/UX-012/window-lifecycle-final-2.log` → PASS (10 tests).
  - `.artifacts/consistency/UX-012/command-bar-final-2.log` → PASS (16 tests).
  - `.artifacts/consistency/UX-012/build-final-2.log` → product build PASS; final stable binary SHA-256 is recorded in `binary-sha256.txt`.
  - `source-audit-inventory-check.log` and `hardcoded-visual-check.log` → no guarded additions relative to `main`.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-012/runtime-proof.json` → `RUNTIME-CONFIRMED`; every assertion passed. Real GPUI events typed `réopen-file`, moved the UTF-8 cursor, inserted punctuation in the middle, selected/replaced a range, undid/redid, pasted multiline/control-containing clipboard text without activation, rejected Tab as text, dismissed, and returned the next character to the parent input. `actions-input-owner.png` captures the live popup. The exact owned process count is zero and the original clipboard bytes were restored in `finally`.
- **Negative controls:** The removed compact fake cursor renderer and `ActionsDialog::{handle_char,handle_backspace,handle_backspace_word,handle_paste}` no longer exist. Local native edits return before host forwarding to prevent doubles. Row shortcuts receive first refusal before Cmd+V/Cmd+A/Cmd+Z input chords. Named Tab cannot become search text. Automation fails closed when the live input owner is unavailable. `automation_state.search.inputState.valueInSync` proves the renderer-neutral mirror and entity owner never diverge.
- **User test/view:**
  1. Open Actions with Cmd+K and type `réopen-file`; the Unicode query should remain intact.
  2. Press Left five times, type `!`, then use Left and Shift+Right to select the punctuation and type `?`; the character should be replaced in place rather than appended.
  3. Press Cmd+Z and Shift+Cmd+Z; the query should undo and return. Press Cmd+A, paste multiline text, and verify the popup stays open and no action runs.
  4. Press Tab; it must not add a tab/control character to the query. Press Escape, then type `x`; `x` must appear in the parent launcher input, not in stale Actions state.
  5. Run `bun .artifacts/consistency/UX-012/runtime-proof.ts`; expect `classification: "RUNTIME-CONFIRMED"`, every assertion `true`, and `ownedProcessCount: 0`.
- **Intentional differences preserved:** Up/Down, Home/End, Page Up/Down, Enter/Cmd+Enter, Escape, Cmd+K, and row shortcuts remain Actions-host commands. Parent windows still retain native key-window status for the calibrated attached-popup feel. The transparent popup remains non-Root to preserve vibrancy; the Input component now treats the shared Root focused-input slot as optional while retaining `Window::handle_input` for text/IME. Glass motion, material, popup geometry, footer optics, and generated design tokens are unchanged.

### UX-013 — Put searchable Actions at the top and freeze the shell

- **Status:** Complete for the UX-013 product contract; focused tests, real product build, multi-host runtime/visual proof, Actions-specific glass entry, rapid-toggle stress, inventory, and exact cleanup are green. The broader main/Notes lifecycle filmstrip observer still reports an unrelated capture-resolution failure described below; it did not justify changing protected motion or weakening its gate.
- **Baseline:** `5a931dce2`.
- **Commit:** `5ef3c45de` — `Implement UX-013: place searchable Actions at the top and freeze popup bounds`.
- **Observable before → after:** Searchable Actions could put their search row at the bottom, and filtering, route transitions, paste/edit history, and action updates could resize the detached AppKit/GPUI popup and republish its automation rect. Searchable Actions now have one top search row. Detached width and height are computed once from the opening root, unfiltered action set, and every later mutation reflows or scrolls content inside that shell.
- **Exact owners/symbols:** `src/actions/types/action_model.rs::SearchPosition`; `src/actions/command_bar.rs::{CommandBarConfig::main_menu_style,CommandBar::set_actions,CommandBar::handle_char,CommandBar::handle_backspace,CommandBar::handle_paste}`; `src/actions/dialog.rs::{ActionsDialogShellSizingSnapshot,ActionsDialog::search_is_visible,ActionsDialog::opening_shell_sizing_snapshot,ActionsDialog::attach_to_fixed_shell,ActionsDialog::release_fixed_shell,actions_dialog_fixed_shell_viewport_height,ActionsDialog::automation_state,ActionsDialog::render}`; `src/actions/window.rs::{ActionsWindow::fixed_shell_size,ActionsWindow::opening_shell_basis,open_actions_window,record_actions_popup_automation_snapshot}`; content-mutation callers in `src/app_impl/actions_dialog.rs`, `src/app_impl/startup_new_actions.rs`, `src/notes/window/panels.rs`, `src/main_sections/day_page_switcher.rs`, `src/prompt_handler/mod.rs`, and `src/ai/agent_chat/ui/chat_window.rs`.
- **Search contract matrix:** `SearchPosition` is compiler-enforced `Top | Hidden`; `Top` is the default. `ActionsDialogConfig::default` and `CommandBarConfig::{default,main_menu_style}` use Top while preserving Bottom anchor where that host already used it. AI/Notes presets keep Top search plus their existing Top anchor. `no_search()` remains Hidden plus Bottom anchor. `WindowPosition::{BottomRight,TopRight,TopCenter}` and placement formulas are unchanged.
- **Opening-shell policy:** `open_actions_window` snapshots the root route (even if a child route is current), counts root unfiltered actions and section headers, records search/header/footer visibility, effective row height, and live max height, and feeds those values once through the existing popup-height resolver. Empty lists still reserve the existing empty-row allowance. No query, filtered count, or child-route content participates.
- **Frozen geometry and automation:** `ActionsWindow` owns `fixed_shell_size` and `opening_shell_basis`; `ActionsDialog` receives only `fixed_shell_height_px` for interior viewport calculation and fills the host bounds. The attached snapshot exposes `fixedForLifetime: true`, `policy: rootUnfilteredAtOpen`, opening basis, fixed width/height, and one generation allocated at open. Filtering and routing change row/content metrics without changing that generation or outer rect.
- **Removed callers/APIs:** Full-tree inventory reports no `SearchPosition::Bottom`, `resize_actions_window`, `resize_actions_window_direct`, resize snapshot updater, resized-frame/origin helpers, or `ActionsPopupEvent::Resized`. The old content-driven resize APIs, re-exports, registry updates, event, callers, and resize-only tests are deleted. `notify_actions_window` remains the content rerender route. No external parent/display geometry caller existed, so no compatibility wrapper was retained.
- **Focused receipts:** `.artifacts/consistency/UX-013/actions-lib-tests.log` → 6,202 passed, 0 failed, 14 ignored; `ux13-lib-tests-first.log` → 5 new contract tests passed; `config-matrix-tests.log` → 46 passed; `ux12-inputstate-bin-test.log` → the real InputState cursor/selection/history regression passed on the binary target; `check-lib-first.log` and `check-bin.log` finished successfully; `glass-static-tests.log` → 40 passed; `glass-calibration-fixture-test.log` → 1 passed; source-reader and hardcoded-visual guards report no additions relative to `5a931dce2`; final formatting and `git diff --check` pass.
- **Product build:** `target-agent/artifacts/ux13-top-shell/script-kit-gpui`; SHA-256 `e2acacdc5bd81edcc3e28befa327a6805aa7768a4a7629ce05c531ccaf9f3924`. The disk watcher later evicted the Cargo pool while a redundant rebuild was running below its 25 GiB floor; the previously verified APFS clone with this exact hash was preserved under `.artifacts/consistency/UX-013/script-kit-gpui` and restored to the stable artifact path. No product-source change occurred between that build and runtime proof.
- **Runtime/visual receipts:** `.artifacts/consistency/UX-013/runtime-proof.json` → `RUNTIME-CONFIRMED`. Main retained one `340×360` rect across nine open/edit/undo/redo/clear/paste/zero-result snapshots; Agent Chat retained one `340×402` rect across route push/pop; Notes retained one `340×402` rect across filter/clear. Every first lifetime stayed generation 1 with the same `actions-dialog` target, and reopening each host produced generation 2. Wrong target, fabricated generation, stale post-close state, and stale dispatch were rejected while each parent remained live. Screenshots: `main-top-fixed-shell.png`, `agentChat-top-fixed-shell.png`, and `notes-top-fixed-shell.png`.
- **Glass anti-drift:** Protected motion/material/token/fixture owners have an empty diff. `glass-actions-entry/receipt.json` is `EVALUABLE_PASS` with matching binary hash, passing motion envelope, passing interference monitor, and cleanup. `rapid-toggle/receipt.json` is `EVALUABLE_PASS` with no crash markers, final recovery, passing interference monitor, and cleanup. The full lifecycle observer was run twice (`glass-lifecycle-attempt1/receipt.json`, `glass-lifecycle/receipt.json`): both cleaned up and had zero interference, but missed the main first-visible width sample and Notes pre-reveal body pixels at the current 120 Hz capture cadence. Those broad, unchanged main/Notes observers are recorded as `EVALUABLE_FAIL`; no protected value, fixture, tolerance, threshold, or test was altered to manufacture green.
- **Negative controls:** Bottom search is unrepresentable. Hidden search installs/focuses no InputState and exposes no search bounds. There is no content resize API. A coordinate delta over `0.25` logical px fails the runtime comparator. Wrong ID, wrong generation, stale registry state, and stale exact-handle dispatch fail. Paste does not activate. No source-reader/hardcoded-visual addition, ignored test, protected calibration drift, or regenerated token change exists.
- **Cleanup:** The final runtime receipt reports every Driver closed and dead after close, `cleanupEscalation: none`, `signalsSent: []`, `broadKillUsed: false`, clipboard SHA before/after equal, exact executable-path `ownedProcessCount: 0`, stale Actions registry removed, and parent surfaces still live. Glass probes also report `cleanedUp: true`; exact-path follow-up process checks found no surviving UX-013 instance.
- **User test/view:**
  1. Open Main Actions with Cmd+K. Confirm search is the first row while the popup retains its existing host placement.
  2. Type `réopen-file`, edit in the middle, undo/redo, clear, paste multiline text, and enter a query with zero results. The outer popup must not move or change size; short/empty results leave safe space inside the same shell.
  3. Open Agent Chat Actions, activate Change Profile to push its picker route, then press Escape. Route push/pop must occur inside the same shell and leave the popup open at the root.
  4. Open Notes and press Cmd+K. Confirm top search with the existing TopCenter placement; filtering and clearing must not resize the popup.
  5. Run `bun .artifacts/consistency/UX-013/runtime-proof.ts`; expect `classification: "RUNTIME-CONFIRMED"`, every assertion true, clipboard SHA equality, no signals/broad kill, and `ownedProcessCount: 0`.
  6. Open the three PNGs in `.artifacts/consistency/UX-013/`; confirm a single top search row, no bottom duplicate, fixed empty space for zero results, and no clipping or added footer chrome.
- **Intentional differences preserved:** Hidden-search API variants; existing `AnchorPosition` and `WindowPosition` choices; UX-012 InputState editing; host navigation/action shortcuts; route semantics; selected-row and scroll behavior; transparent non-Root vibrancy host; glass timing/scale/alpha/material/placement; footer optics; generated design tokens.

### UX-014 — Complete attached popup lifecycle and exact stale-instance safety

- **Status:** Complete for the UX-014 product contract. Agent Chat history and Dictation microphone popups now share hidden-before-attach mechanics and generation identity while retaining consumer-owned state. Real product proofs cover parent Escape, outside click, exact focus/text restoration, clean reopen, stale refusal, native AppKit close, strict semantics/screenshots, no-persistence selection, and exact cleanup.
- **Baseline:** `5ef3c45de`.
- **Commit:** `90d2dae90` — `Implement UX-014: enforce generation-scoped popup lifecycle and exact focus return`.
- **Observable before → after:** Interactive child windows could become visible before their deferred AppKit parent attachment, reused stable automation IDs could resolve a reopened lifetime, PromptPopup collection could fall through to another open popup, and independent owner/child close paths could re-enter a leased GPUI entity. Interactive children now begin hidden and non-key, verify the exact registered parent and native child relationship before show/publication, carry a monotonic generation through every registry/handle/cache/callback, close through one generation gate, restore the exact prior parent control, and reject stale lifetimes after reopen.
- **Exact owners/symbols:** `src/components/inline_popup_window.rs::{InlinePopupGeneration,InlinePopupLifecycle,InlinePopupAttachReceipt,InlinePopupFocusReturn,inline_popup_focus_pair_is_active,configure_inline_popup_window_lifecycle,close_prompt_popup_target_natively}`; `src/ai/agent_chat/ui/{history_popup.rs,view.rs,view/portal_host.rs,chat_window.rs}`; `src/dictation/{microphone_popup_window.rs,window.rs}`; `src/protocol/types/automation_window.rs`; `src/windows/{automation_registry.rs,automation_runtime_handles.rs,automation_surface_collector.rs}`; `src/prompt_handler/mod.rs`; `src/platform/gpui_event_simulator.rs`; `src/stdin_commands/mod.rs`; `scripts/agentic/ux14-popup-life-probe.ts`.
- **Lifecycle contract:** New interactive children use `show:false, focus:false`, advance only `CreatedHidden → AttachPending → Open → Closing → Closed`, retry parent readiness for three deferred GPUI turns without sleeps, and publish target/runtime/semantic identity only after an AppKit receipt proves hidden/non-key preconditions, exact parent pointer equality, and post-config visibility. Cleanup is conditional on `(id,generation)`, so delayed generation N callbacks cannot clear N+1.
- **Consumer ownership:** Agent Chat has one `history_popup_lifetime`; all setup/session/draft/picker/portal/submit/selection transitions enter `close_history_popup_for_owner_transition`, and only the two central helpers clear `history_menu`. Dictation has one overlay-owned microphone lifetime with exact parent `dictation`; parent-owned routes reconcile directly before child close while popup-owned routes notify the owner once. The dead Agent Chat popup RAII registry module was deleted.
- **Strict automation and resolver behavior:** Automation schema v2 adds optional descriptor generation plus `Instance { id, generation }`. Exact generation is revalidated in registry resolution, runtime handle dispatch, semantic collection, screenshots/events, and every PromptPopup batch command. PromptPopup subtype routing is exact for history, Dictation, and generationless Confirm; no unrelated popup fallback remains. `closePromptPopupNatively` refuses anything except an exact live PromptPopup instance.
- **Focused/static receipts:** Shared lifecycle tests passed `2/2`; the retained Dictation contract passed `3/3`; final `agent-cargo check --bin script-kit-gpui` and the final product build finished successfully; `git diff --check` passes. Source-audit inventory decreased app-source sites `2331 → 2330` and total sites `2819 → 2818`; no new visual literal or ignored test was added. A later redundant `cargo test --lib inline_popup_lifecycle_` cold rebuild was terminated with exit 143 after 7m38s by the repository's low-disk watcher at the 25 GiB floor; that infrastructure failure is preserved rather than called green or bypassed with bare Cargo.
- **Product build:** `target-agent/artifacts/ux14-popup-life/script-kit-gpui`; SHA-256 `a2e667d19f8e5bc0b3c558995b93df1de69a96c241862ecbf6fa8b322eecc4c3`. Both final runtime receipts name this exact path and were rerun after the final clone was created.
- **Agent Chat runtime/visual receipt:** `.artifacts/ux14-popup-life/runtime-agent-history.json` is green. It proves exact detached parent identity; generation 1 semantics and a strict `415×463` screenshot; parent Escape closes only history; exact composer return by typing `λ` (`focus-return:λ`, cursor 14); reopen advances `1 → 2`; generation 1 fails with explicit stale-target warning; parent outside-click closes generation 2 and typing `β` reaches the same composer (`focus-return:λβ`, cursor 15); exact AppKit close reconciles generation 3 and typing `γ` again reaches that composer (`focus-return:λβγ`, cursor 16). Screenshot: `.artifacts/ux14-popup-life/agent-history-popup.png`.
- **Dictation runtime/visual receipt:** `.artifacts/ux14-popup-life/runtime-dictation-microphone.json` is green. It proves exact `dictation` parent identity; strict panel/list/two-row semantics and `317×80` screenshot; selector-only parent Escape with the overlay focused; fresh generation and stale refusal; parent outside-click with focus returned; exact native close with focus returned; fourth-generation subtype-routed batch selection; and byte-identical sandbox config before/after the no-persistence fixture selection. Screenshot: `.artifacts/ux14-popup-life/dictation-microphone-popup.png`.
- **Glass/Actions anti-drift:** Protected footer, opacity, secondary-window calibration, chrome-token, fixture, filmstrip, and rapid-toggle source paths have an empty diff. The static glass suite passes `40/40`. `.artifacts/ux14-popup-life/glass-actions-entry/receipt.json` and `.artifacts/ux14-popup-life/rapid-toggle.json` both pass against the final binary. The full lifecycle observer remains `EVALUABLE_FAIL` on the same broad main-entry cadence and Notes pre-reveal body-mask observations already recorded for UX-013; Notes close/reopen and Dictation exit/reopen pass, no interference invalidation occurred, and no protected value/threshold was changed. Direct `actions.ts inspect` retries were preserved as instrumentation-blocked (`stale-view`/target ambiguity), while the native Actions entry and rapid-toggle proofs are green.
- **Negative controls:** A missing/mismatched parent fails while the child remains hidden; stale/fabricated generations resolve to zero elements plus an explicit stale-instance warning; exact event dispatch never falls back to the parent; old cleanup cannot delete the reopened generation; fixture microphone acceptance cannot persist config; native close requires exact PromptPopup kind and generation; wrong subtype cannot mutate another popup; no second child is admitted while the current slot is Closing.
- **Cleanup:** Every final Driver receipt reports `processExited:true`, `streamsDrained:true`, `logWriterClosed:true`, `ownedProcessCount:0`, `ownedChildProcessCount:0`, and `clipboardTouched:false`. Named Actions sessions were explicitly stopped. Final exact executable-path inventory reports zero owned processes; no broad signal or unrelated Script Kit process was used.
- **User test/view:**
  1. Open detached Agent Chat, focus its composer, enter `focus-return:`, and open History. Press Escape once, then type `λ`; History alone should close and the composer should read `focus-return:λ` at the prior caret.
  2. Reopen History after its short close debounce, then click the Agent Chat backdrop outside the popup. Agent Chat remains open and accepts typing.
  3. Open Dictation's deterministic/manual recording overlay, choose **Select Mic**, and press Escape once. Only the selector closes; the recording overlay remains.
  4. Reopen **Select Mic** and choose another row. Only one clean selector is visible and production microphone preference does not change in fixture mode.
  5. Open `.artifacts/ux14-popup-life/agent-history-popup.png` and `dictation-microphone-popup.png` to view the exact real-product popups.
  6. Run `bun scripts/agentic/ux14-popup-life-probe.ts --verify .artifacts/ux14-popup-life/runtime-agent-history.json` and the equivalent Dictation receipt; both must report `verified:true` and zero owned processes.
- **Intentional differences preserved:** Actions keeps its independent fixed-shell owner; Confirm remains generationless and parent-key-routed without an activation observer; menu syntax remains a main-list projection; footer uses the compatibility attach wrapper. UX-012 InputState ownership, UX-013 Actions geometry/search, host shortcut/navigation semantics, production microphone persistence, and all glass motion/material/geometry/optics remain unchanged.

### UX-016 — Make Button, Toast, and shortcut feedback keyboard-operable and uniquely identified

- **Status:** Complete for the UX-016 product contract; model, real GPUI keyboard dispatch, stable-ID semantics, exact toast timing/focus lifecycle, product runtime/visual proof, build, anti-drift, and cleanup are green.
- **Baseline:** `90d2dae90`.
- **Commit:** This section is committed atomically with `Implement UX-016: require stable control IDs and keyboard-operable toast and shortcut feedback`; use `git log -1 --oneline` for the immutable hash.
- **Observable before → after:** Script Kit Buttons could derive identity from visible labels and tracked focus without joining keyboard traversal. Toast queueing discarded action/callback/identity detail, duplicate messages could collide, dismiss was effectively hover-only, and the main launcher did not render its notification layer. Shortcut Recorder traversal could stop on unavailable Save/Clear actions. Buttons now require label-independent stable IDs, enabled interactive controls are tab stops with visible focus and Enter/Return/Space activation, Toast and ToastAction IDs are typed and independent from copy, the full Toast model survives queueing into entity-backed notifications, dismiss is visible at rest, and recorder traversal visits only currently eligible actions.
- **Exact owners/symbols:** `src/components/button/component.rs::Button`; `src/components/button/tests.rs`; `src/components/toast/{types.rs,model.rs,render.rs,tests.rs}`; `src/toast_manager/{mod.rs,notification.rs}`; `src/components/shortcut_recorder/{types.rs,component.rs,render.rs,tests.rs}`; `vendor/gpui-component/crates/ui/src/button/button.rs`; `vendor/gpui-component/crates/ui/src/notification.rs::{Notification,NotificationList}`; `src/main_sections/render_impl.rs`; `src/app_impl/lifecycle_reset.rs`; `scripts/agentic/ux16-keyboard-feedback-probe.ts`.
- **Identity and activation contract:** Script Kit `Button::new` requires a non-empty semantic ID and never falls back to label text; changing a label preserves identity and duplicate labels can coexist. Enabled clickable Buttons obtain keyed focus ownership and register `tab_stop(true)`; Enter, Return, and Space dispatch exactly once, while disabled/loading controls remain inert. `ToastId` identifies one toast lifetime, `ToastActionId` identifies one action inside it, and rendered root/action/dismiss IDs compose both typed identities rather than messages, labels, or positions.
- **Toast lifecycle:** `ToastManager` preserves the entire Toast model, including actions, dismiss callback, stable ID, variant, details, and exact optional duration. The notification bridge renders model-backed custom content, captures the prior focus handle, returns focus after action/dismiss, pauses only the remaining auto-dismiss budget while notification controls contain focus, and binds timers to the exact entity so an old same-ID timer cannot dismiss its replacement. The launcher now renders `Root::render_notification_layer` exactly once. Dismiss controls remain muted but visible without hover.
- **Shortcut Recorder behavior:** Eligibility is derived from current Save/Clear availability plus always-available Cancel. Initial/cleared state focuses Cancel, Tab/Shift+Tab skip unavailable actions, completed conflict-free chords focus Save, and Enter cannot invoke unavailable Save/Clear branches. Existing stable Save/Clear/Cancel IDs and canonical shortcut token rendering remain intact.
- **Focused verification:** `components::button` → 16 passed; `components::toast` → 8 passed; `components::shortcut_recorder` → 16 passed; `toast_manager` → 9 passed; `gpui-component notification::tests` → 4 passed; `agent-cargo check --lib` finished. These include real GPUI Enter/Space Button dispatch, disabled/loading inertness, real recorder Tab/Enter and chord/Enter dispatch, duplicate Toast identity enumeration, focus-paused exact-duration auto-hide, exact focus return, stale-timer replacement safety, and Space dispatch on the built-in dismiss control.
- **Product build:** `target-agent/artifacts/ux16-keyboard-feedback/script-kit-gpui`; SHA-256 `199911ec10b5c3d3876a046559f69ff12c6d963feae9339fbdcd3277ca2d7398`.
- **Runtime/visual receipt:** `.artifacts/consistency/UX-016/runtime-keyboard-feedback.json` is `RUNTIME-CONFIRMED` against that exact artifact. The real launcher renders two simultaneous `Duplicate status` Toasts and three separate visible `Open` actions under seven unique runtime control IDs. Exact GPUI pointer dispatch of one action removes only `ux16-runtime-a`; exact dismiss dispatch removes only `ux16-runtime-b`; the focused semantic ID is `input:filter` before, after action, and after dismiss. Screenshot: `.artifacts/consistency/UX-016/runtime-keyboard-feedback.png` (`750×501`). Component-level real GPUI tests supply the Enter/Space keyboard proof without overriding the launcher’s intentional global Tab/Shift+Tab ownership.
- **Negative controls:** Empty Button and ToastAction IDs panic; label/message changes never define identity; duplicate visible labels/messages retain distinct IDs; disabled/loading Buttons cannot activate; recorder traversal cannot land on unavailable actions; exact Toast durations are not bucketed; focused time is not charged to auto-dismiss; stale timers cannot dismiss replacements; the temporary Control-F6/notification-first Tab experiment is absent; host Tab/Shift+Tab routing is unchanged; no source-reader/count audit was added.
- **Glass/consistency guardrails:** Protected glass owner diff is empty. Static motion/lifecycle/rapid-toggle tests pass `40/40`; the production calibration fixture passes `1/1`. No glass value, fixture, threshold, footer optic, popup geometry, generated token, or host keyboard route changed.
- **Cleanup:** The final Driver reports `processExited:true`, `streamsDrained:true`, `logWriterClosed:true`, and exact artifact-path `ownedProcessCount:0`; no broad signal was used and clipboard was untouched.
- **User test/view:**
  1. Run `RUST_MIN_STACK=268435456 ./scripts/agentic/agent-cargo.sh test --lib components::button`; expect 16 passed, including real Enter/Space activation and disabled/loading inertness.
  2. Run the focused Toast, Shortcut Recorder, ToastManager, and `gpui-component notification::tests` commands listed above; expect `8/8`, `16/16`, `9/9`, and `4/4` green.
  3. Run `bun scripts/agentic/ux16-keyboard-feedback-probe.ts`; expect `classification: "RUNTIME-CONFIRMED"`, all seven stable controls, `input:filter` focus at all three checkpoints, and `ownedProcessCount: 0`.
  4. Open `.artifacts/consistency/UX-016/runtime-keyboard-feedback.png`; verify two duplicate-message Toasts coexist, the first has two same-label actions, the second has one, and each dismiss control is visible at rest.
  5. In a surface without a host-owned Tab command, focus a Button and press Enter or Space; it should activate once with visible focus. Disabled/loading Buttons must remain inert.
  6. Open Shortcut Recorder empty and press Tab then Enter; unavailable Save/Clear are skipped and Cancel runs. Record a valid chord, then press Enter; Save runs.
- **Intentional differences preserved:** The main launcher retains its established plain Tab and Shift+Tab commands rather than stealing them for Toast traversal. Keyboard operability is enforced at the reusable control and notification layers, while the product runtime proof validates real rendering, exact action/dismiss lifetimes, focus return, and cleanup. AppKit/native controls remain separate renderers, and all UX-012/UX-013/UX-014 ownership, fixed geometry, popup lifecycle, and host navigation semantics remain unchanged.

## Verification ledger

- `node --test design/consistency/tests/validate-explorer.mjs` — baseline PASS, 75/75 tasks represented.
- `node design/consistency/tests/browser-smoke.mjs` — baseline PASS, 12 groups and 75 task scenes rendered.
- `node design/mockups/tests/lint-mockups.mjs` — baseline PASS.

## User testing index

Testing steps will be added under each completed recommendation and summarized by surface here.
