For a map of main UI surfaces to code implementation, see [GLOSSARY.md]

# Before Starting Work

- **Route through a project flow first.** Flows are the primary mechanism for agents working with this code: pick the owner from the routing rules in "Project Flows — Primary Mechanism" below (or read `flows/README.md`), then delegate with `md flows/<name>.md "<task>"`. Work directly only for trivial edits, when mdflow is unavailable, or when the user explicitly asks you to.
- Inspect the relevant source, tests, and repo-local skills before editing.
- Prefer current code and generated artifacts over stale notes or memory.
- Keep edits narrowly scoped and verify them with the smallest check that can fail for the changed behavior.
- Keep tool-facing root docs in place: `README.md`, `CLAUDE.md`, `AGENTS.md`, and `.impeccable.md`.

## Oracle / Packx Bundle Context

For Oracle review or `oracle-packx` work in this repository, include the repo process context in the bundle or prompt unless the user explicitly excludes it: `AGENTS.md`, the owning `.agents/skills/<skill>/SKILL.md`, and relevant source, tests, generated contracts, and verification notes.

For runtime/UX bugs headed to `oracle-packx-conversation`, run `flows/devtools.md` first and include its investigation receipts (intake, primitive stack, measurements, classification, likely owner, red/green proof plan) in the bundle — that pairing is a primary workflow in this repo.

If a `packx` preview with include globs unexpectedly matches `0` files in this repository, rebuild the bundle from an explicit path list instead of widening blindly. A reliable workaround is:

```bash
rg --files <scope> | rg '<owners-or-patterns>' > /tmp/script-kit-gpui-packx-files.txt
xargs packx --preview --no-interactive -x "**/CLAUDE.md" < /tmp/script-kit-gpui-packx-files.txt
xargs packx --limit 900k --strip-comments --minify -f markdown --no-interactive --stdout -x "**/CLAUDE.md" < /tmp/script-kit-gpui-packx-files.txt > ~/.oracle/bundles/<slug>.txt
```

Use this when directory/include-glob matching undercounts relevant files; keep `CLAUDE.md` excluded and verify the preview count plus final non-empty bundle before consulting Oracle.

## Project Flows — Primary Mechanism

Project flows are the **primary mechanism for agents working with this code**.
Every substantive task — building, debugging, auditing, fixing, testing,
releasing — starts by routing to the owning flow; direct hand-editing is the
fallback, not the default.

Flows live under `flows/` at the repo root: one markdown agent per job, run
by [mdflow](https://mdflow.dev) (`npm i -g mdflow@next`). They run on codex
(pinned in `.mdflow.yaml`) at `gpt-5.6-sol` with `medium` reasoning; sandbox mode
is pinned per flow in frontmatter. Each `flows/<name>.md` file is
**self-contained**: frontmatter owns the engine contract, the body owns the
instructions, and there is no central registry. `flows/README.md` is the
roster index. Flow learning is eval-driven: when a run disappoints, add a
failing case to `flows/<name>.eval.ts`, then edit the flow until
`md eval flows/<name>.md` passes.

Default workflow for any task:

1. Pick the owner from the routing rules below (unclear? run `md flows/scout.md "<task>"`).
2. Delegate the task: `md flows/<name>.md "<task>"`.
3. Pair the owner with at most one cross-cutting role flow when needed (audit → fix, fix → probe, etc.).
4. The calling agent stays responsible for source inspection, patch review, preserving unrelated dirty work, and final verification — delegating the work does not delegate accountability.

Skip the flow only when the task is trivial (≤ a few mechanical lines),
mdflow is unavailable or repeatedly stalls, or the user explicitly directs
otherwise — and say in the final answer which flow was skipped and why.

Use (from the repo root):

```bash
md flows/<name>.md "<task prompt>"        # delegate one job (one engine turn)
md flows/<name>.md "<task>" --_dry-run    # free: exact command + resolved prompt
md eval flows/<name>.md                   # run the flow's eval suite
cat flows/README.md                       # roster
```

Routing rules:

- Use `flows/scout.md` when ownership is unclear.
- Agent Chat, `@file`, `@context`, attachments, portal, or Pi handoff -> `flows/agent-chat.md`.
- Day Page, Today, brain, fragments, spine, or Notes parity -> `flows/brain.md`.
- Clipboard history, sediment, post-copy, copy-to-brain, no-popup capture, or `crates/sk-clipboard/**` -> `flows/clipboard.md`.
- Shared UI, components, list rows, inputs, prompt shells, chrome, or theme tokens -> `flows/components.md`.
- Script List, main window, mini/full view, launcher selection -> `flows/launcher.md`.
- Actions menu, command palette, trigger picker, confirm popup -> `flows/actions.md`.
- Hotkeys, gestures, tap/hold/double-tap, focus restoration -> `flows/hotkeys.md`.
- Script prompt renderers, protocol-to-renderer contracts, or `crates/sk-protocol/**` -> `flows/prompts.md`.
- Built-in utility surfaces -> `flows/builtins.md`.
- Terminal prompt, PTY, command bar, terminal theme -> `flows/terminal.md`.
- Script execution, menu cache, metadata, scheduler -> `flows/execution.md`.
- macOS platform, windows, tray/menu bar, icons, permissions, startup, Pi sidecar -> `flows/platform.md`.
- MCP server, resources, script tools, schema compatibility -> `flows/mcp.md`.
- Repo process docs, `.agents/**`, `flows/**`, probes, source audits, `dev.sh`, cargo wrappers -> `flows/devex.md`.
- Local LLM/ghost backend, dictation, whisper, computer use, OCR, camera, AI vault -> `flows/ai-core.md`.
- Settings, config persistence, onboarding/NUX, kit store, sync, updates, login item, secrets -> `flows/settings.md`.
- v1→v2 script migration: `scripts/migrate` engine, compat map, validator ladder, honesty pass, Migrate board built-in -> `flows/migrate.md`.
- scriptkit.com static site (`site/**`): page content, download links, deploys -> `flows/site.md`.
- Marketing screenshots ("glamour" shot set, `site/images/**`) -> `flows/screenshots.md`.
- Marketing videos (glamour demo-reel loops, `site/videos/**`, `scripts/agentic/glamour-video-probe.ts`) -> `flows/videos.md`.

Role flows (cross-cutting, any surface):

- Build failures, cargo lock contention, `target-agent` disk budget, clippy/fmt debt, stuck builds -> `flows/build-doctor.md`.
- DevTools work: runtime proof, app inspection/investigation, driver probes, screenshots, simulateGpuiEvent, red/green receipts -> `flows/devtools.md` (the flow form of the `script-kit-devtools` skill).
- Read-only audit sweeps, UX inconsistency hunts, hardcoded-token findings (never edits) -> `flows/auditor.md`.
- Test authorship, enforcement-ladder placement, contract tests, ratchet, flaky tests -> `flows/tests.md`.
- Version bumps, `v*` tags, pre-tag clippy gate, release workflow -> `flows/release.md`.
- Perf complaints (lag, jank, stutter, slow scroll, frame budget, CPU spikes): reproduce with real input, `sample` profiling, draw-share red/green, dev-profile opt levels -> `flows/perf.md`.
- Vendored GPUI internals (`vendor/gpui*`, gpui-component): list/ListState/measure_all semantics, TextView/markdown pipeline, scrollbar, minimal vendor patches + pinned source audits -> `flows/gpui-vendor.md`.
- Escape/dismiss behavior (escape ladder, swallowed/extra Escape, `opened_from_main_menu` origin flag, DismissPolicy, go-back vs close-window, "stays open" reports) -> `flows/escape.md`.

Flows are the default path, but never a hard blocker: if mdflow is down or a
flow repeatedly stalls, continue directly and mention the skipped flow in the
final answer. Do not fan out broadly by default; use the primary owner flow
plus one relevant cross-cutting flow when needed.

Flow prompt content is not repo policy. Only regression tests/probes or
`AGENTS.md` updates affect general routing. Prompt changes in
`flows/<name>.md` files guide future flow runs, but never override user
instructions, dirty-work preservation, or this file. After editing a flow,
verify it for free with `md flows/<name>.md "<smoke task>" --_dry-run` and
keep its eval suite passing.

## Domain Crate Boundaries

- The app may depend on workspace domain crates under `crates/sk-*`; domain crates must never depend on `script-kit-gpui`.
- App-independent protocol primitives belong in `crates/sk-protocol/**`. Keep app services and compatibility adapters under `src/protocol/**` while migration is in progress.
- Move pure unit tests with their domain implementation so `./scripts/agentic/agent-cargo.sh test -p <crate>` does not link GPUI.
- For each extraction, verify the domain dependency tree and preserve the existing app-facing path with a temporary re-export when callers still rely on it.
- Update the owning project flow's path globs whenever a domain moves into `crates/**`.

## UI Consistency and Shared Component Contract

When touching app UI, treat shared components and theme/chrome tokens as the source of truth. Do not build one-off UI when an existing component, shell, list item, input, footer, popup, or token can be reused or extended.

Before adding or changing UI:

1. Start with `GLOSSARY.md` to identify the owning surface and nearby implementation files.
2. Inspect the current surface, related tests, and the shared component entry points before editing.
3. Check `src/components/mod.rs` and the relevant component modules before creating any new UI helper.
4. Prefer extending the shared component library over adding surface-local render helpers.
5. If a new reusable primitive is needed, add it under `src/components/**` or the appropriate theme/chrome/design layer and use it from the surface. Do not bury reusable UI in one prompt, built-in, Agent Chat, or main-window renderer.

Shared UI entry points to check first:

- Inputs/search/menu fields: `src/components/text_input.rs`, `src/components/text_input/**`, `src/components/inline_prompt_input.rs`, `src/components/inline_dropdown/**`, `src/components/inline_picker.rs`, and `src/components/inline_popup_window.rs`.
- List rows and sections: `src/components/unified_list_item/**`; preserve existing `crate::list_item` usage where that is the current surface contract, but do not invent a third row system.
- Prompt shells and prompt chrome: `src/components/prompt_layout_shell.rs`, `src/components/prompt_container.rs`, `src/components/prompt_footer.rs`, and `src/components/minimal_prompt_shell.rs`.
- Footer and hint strips: `src/components/hint_strip.rs`, `src/components/footer_chrome.rs`, `src/footer_popup.rs`, and native footer handling in `src/app_impl/ui_window.rs`.
- Main-window chrome/layout: `src/components/main_view_chrome.rs`, `src/main_sections/**`, `src/render_script_list/**`, and `src/app_layout/**`.
- Empty/info/non-list states: `src/components/info_state.rs` and `src/components/non_list_state.rs`.
- Forms/buttons/toasts/shortcuts: `src/components/form_fields/**`, `src/components/button.rs`, `src/components/toast/**`, and `src/components/shortcut_recorder.rs`.

Theme and visual values must be tokenized:

- Resolve colors and chrome surfaces through `crate::theme`, especially `AppChromeColors::from_theme`, `PromptColors`, theme opacity, and the design token layers.
- Use chrome/layout constants from `src/ui/chrome/tokens.rs`, `src/theme/**`, `src/designs/core/**`, and `src/designs/traits/**`.
- Do not hardcode new colors, opacity values, spacing, typography, border radii, borders, popup surfaces, vibrancy behavior, or chrome layer semantics in surface renderers when an existing token/helper exists.
- If a visual value needs to become standard, add or extend a token/helper in the appropriate shared layer so theme changes propagate automatically.

Cross-surface behavior must stay predictable:

- Main window, prompt/make windows, built-ins, and Agent Chat/Agent Chat should share inputs, menu/search behavior, list rows, prompt shells, hint strips, footer affordances, popup/dropdown mechanics, and chrome tokens wherever possible.
- Native left-pinned and trailing footer keycaps must both resolve token-specific
  x/y glyph offsets and horizontal padding through the shared
  `footer_appkit_glyph_x`, `footer_appkit_glyph_y`, and
  `footer_keycap_padding_x_for_token` helpers. Do not duplicate or locally
  approximate keycap centering formulas.
- Actions UI should feel like the main list: same row language, same search treatment, same shortcut/keycap conventions, and no extra local chrome unless the owning contract requires it.
- Expanded/preview surfaces may differ in layout, but their list side, footer, and chrome should still use the shared anatomy and tokens.
- Any intentional divergence must be documented in the code or PR summary with the owning surface, the reused alternatives considered, and why the shared component could not fit.

## Glass Motion Calibration Lock

The main-window, Actions, Notes, Dictation, and popup glass motion was calibrated
against frame-by-frame Spotlight reference footage. **Agents must not retune,
normalize, simplify, or "clean up" these values unless the user explicitly asks
for animation retuning or unlocks the calibration in the current request.** A
generic bug fix, refactor, theme change, or contrast task is not permission.
If a requested change appears to require different motion values, preserve the
calibration and ask for explicit permission instead.

The locked production contract is (retuned 2026-07-24 from side-by-side
Spotlight/Script Kit footage, `CleanShot 2026-07-24 at 09.18.40.mp4`, against
the measurement page https://eager-hollow-dyyf.here.now/):

- default entry duration `0.28s`: compression `140ms`, squish hold `50ms`
  (Spotlight rests ~3 frames at max compression), rebound `90ms`;
- entry inset `0.03`, producing a main-window `106% → 98.5% → 100%` width path
  and an Actions/popup `94% → 101.5% → 100%` path — Spotlight's measured max
  undershoot is `−1.3%` of TOTAL width; the earlier `97%`/`103%` mid-points
  had doubled the measurement via a per-side ×2 and read rubbery;
- entry start alpha `0.0`, animated to `1.0` during phase one;
- vertical damping `0.4`, squish factor `0.25` (per side, of the inset,
  clamped `0.006–0.015`), squish hold `0.05s`, phase-one fraction `0.5`;
- Notes body reveal is derived from the calibrated geometry: it starts at the
  phase-one settled-size crossing (`97ms` with the default calibration), then
  fades over `90ms` while the window compresses and rebounds;
- glass material: stability tint floor `0.35` and capsule veil `0.80`
  (`src/ui/chrome/tokens.rs`) — the Jul 23 `0.55`/`0.94` stack read
  near-solid mid-entry, visibly heavier than the Spotlight reference fade;
- detached main-window exit is fixed-frame fade-only;
- popup exit duration `0.12s`, removal delay `135ms`, grow x/y `0.03/0.012`,
  shrink x/y `0.05/0.035`, and blur radius `8.0`.

Owning sources and anti-drift evidence:

- defaults: `src/theme/opacity.rs`;
- native physics/lifecycle: `src/platform/secondary_window_config.rs`;
- named production fixture:
  `scripts/agentic/fixtures/glass-motion-calibration-theme.json`;
- geometry envelope:
  `scripts/devtools/glass-entry-motion-contract.ts`;
- frame/runtime probes:
  `scripts/devtools/glass-lifecycle-filmstrip.ts`,
  `scripts/devtools/actions-entry-filmstrip.ts`, and
  `scripts/devtools/rapid-toggle-stress.ts`.

Do not update the fixture, envelope thresholds, expected geometry, or their
tests merely to make a changed animation pass. With explicit user permission,
retune from new frame-by-frame evidence, update all owning sources together,
and report the before/after measurements. Without that permission, restore the
locked values.

Minimum anti-drift check:

```bash
bun test scripts/devtools/glass-entry-motion-contract.test.ts \
  scripts/devtools/glass-lifecycle-filmstrip.test.ts \
  scripts/devtools/rapid-toggle-stress.test.ts
./scripts/agentic/agent-cargo.sh test --lib \
  platform::secondary_window_config_tests::glass_motion_fixture_matches_the_measured_production_calibration
```

For runtime/visual changes, also run the lifecycle, Actions-entry, and rapid
toggle probes with `SCRIPT_KIT_TEST_STATUS=1` and the named calibration fixture.
An `INVALID_INTERFERENCE` receipt means rerun when input is quiet; it is not a
product failure and must not be converted into a green result.

## Agent Chat Entry Context Contract

Launcher-sourced Agent Chat opens stage the currently selected launcher row as a context chip (and pre-fill the composer with its `@cmd:` mention) UNLESS the entry request suppresses the focused part. The default selection on a fresh launcher is a real, resolvable row (`first_selectable_index` — a Brain Inbox capture, a flow, a script), so any "just open a clean chat" affordance wired through the default entry silently inherits it. This bit us on 2026-07-10: main-hotkey double-tap opened Agent Chat with the first row attached as if deliberately chosen.

- Quick/clean-chat affordances (double-tap of the main hotkey, and anything with the same "I just have a quick question" intent) MUST route through `AgentChatEntryRequest::quick_question()` / `open_agent_chat_for_quick_question`, or otherwise pass `suppress_focused_part: true`.
- Only entries where the user deliberately targeted a row (Cmd+Enter on a selection, actions payloads, explicit handoffs) may stage the focused row — and Cmd+Enter already suppresses the *default auto-selected* row via its empty-input guard in `agent_handoff/mod.rs`. Mirror that guard's intent in any new launcher-sourced entry.
- The contract is locked by `quick_question_entry_suppresses_all_implicit_context` in `src/app_impl/agent_handoff/agent_chat_entry.rs`; keep new entry points honest against it rather than weakening the test.

# Agent Cargo Wrapper

`./dev.sh` runs `cargo watch` on the shared `target/` dir continuously. Bare `cargo build/test/check/clippy` from an AI agent contends on `target/.cargo-lock` and stalls for minutes ("Blocking waiting for file lock on build directory").

All agent-driven cargo invocations MUST go through `./scripts/agentic/agent-cargo.sh`, which defaults to the bounded shared `CARGO_TARGET_DIR=target-agent/pools/agent-debug` pool with a visible lock. Examples:

- `./scripts/agentic/agent-cargo.sh test --lib notes_editor::spine`
- `./scripts/agentic/agent-cargo.sh check --lib`
- `./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui`

Use `SCRIPT_KIT_CARGO_TARGET_POOL=<name>` for an intentional shared pool, and set `SCRIPT_KIT_AGENT_TARGET_MODE=exclusive` only when a task truly needs a per-agent cache under `target-agent/agents/<agent-id>`. Do not run bare `cargo` against this repo while `./dev.sh` may be running.

Disk policy: the wrapper enforces a total `target-agent` budget at lock acquisition (`SCRIPT_KIT_AGENT_TARGET_BUDGET_GB`, default 40) plus a free-disk floor (`SCRIPT_KIT_AGENT_MIN_FREE_GB`, default 25), evicting least-recently-used unlocked pools before building. Extra pools are therefore ephemeral by design — do NOT mint a pool per parallel task. When a task needs a stable binary path, export an APFS clone instead: `SCRIPT_KIT_AGENT_ARTIFACT_NAME=<task> ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui` produces `target-agent/artifacts/<task>/script-kit-gpui` (~0 bytes, replaced atomically on rebuild). Dev builds use `CARGO_PROFILE_DEV_DEBUG=line-tables-only` and non-default pools disable incremental; both respect pre-set env overrides.

# AI Reliability Rules

Any work on an AI failure, recovery card, or engine transport (Quick AI, Flow
chat, Agent Chat) must follow `rules/AI_RELIABILITY.md`. The short version:

- Classify from the FACT the runtime stated, never from prose you formatted
  yourself. `classify_provider_failure` pattern-matches English and returns
  `Unknown` for anything it does not recognise.
- Carry the `AppFailureRecord`. Reducing it to its safe copy and re-classifying
  that copy later always downgrades to `Unknown`.
- Raw provider text, stderr, OS errors, and adapter internals stop at the
  diagnostic vault. Screens get `primary_message()`; logs get the code plus the
  diagnostic fingerprint.
- A recovery action a surface cannot perform is never rendered enabled — and a
  surface that forgets to install its recovery callback silently hides every
  useful button.
- A user Stop is cancellation, not an error.
- If `getElements` cannot see it, it is not proven. Project new shared AI
  elements into the element collector from the same source the renderer uses.

Focused checks are listed at the bottom of that file.

# Source Audit Test Policy

Source-audit tests (tests that `read_to_string`/`include_str!` app source and assert on its text) are decision locks, not behavior coverage. They are a scarce resource — do NOT mint one per feature pass.

Reality check: `tests/source_audit_inventory.md` is generated by `python3 -B tests/source_audit_inventory.py` and currently reports 3,218 reader sites across 454 of 548 Rust test files. Of those, 2,716 sites in 396 files resolve to `src/**`; another 49 sites in 31 files have dynamic targets the scanner cannot prove safe (the file categories can overlap). The ordinary-PR guard compares normalized per-file reader-site multisets against the exact base tree, so adding a source read inside a grandfathered file or swapping one target for another fails; new unresolved dynamic reads fail conservatively too. Separately, `tests/source_audit_ratchet.rs` has a shrink-only allowlist for `.matches(...).count()` / `.match_indices(...).count()` assertions; it is not a corpus-wide source-audit ratchet. The existing corpus is NOT precedent for writing more. New invariants climb the ladder below, and when a grandfathered audit blocks a legitimate refactor, apply the pruning rule instead of patching its strings.

Enforcement ladder — pick the highest rung that can express the invariant:

1. **Compiler/type system** — exhaustive `match` without a wildcard arm, newtypes for tokens, visibility. If the compiler can enforce it, do not write a test for it.
2. **Lints** — `#[deny]` attributes, clippy `disallowed-methods`/`disallowed-types` in `clippy.toml`.
3. **Behavior test** — `#[gpui::test]`/`TestAppContext`, or a unit test on the extracted logic.
4. **Runtime proof** — a devtools probe script under `scripts/agentic/` for window/focus/render behavior tests cannot reach.
5. **Source audit** — last resort, only for genuinely load-bearing architectural invariants (e.g. the footer blur trio) that no higher rung can express.

When a source audit is justified:

- Prefer asserting the **absence of a dangerous pattern** (e.g. no `_ =>` wildcard, no `cx.notify()` in a hot path) over the presence of exact formatted code.
- Scope assertions with a `function_body`-style structural helper, not whole-file substring search.
- NEVER assert exact occurrence counts of formatted source lines (`source.matches(...).count() == N`); enumerate the expected sites explicitly instead. `tests/source_audit_ratchet.rs` enforces this.
- Document in the test's doc comment WHY the invariant exists, so a failing assertion can be evaluated rather than blindly appeased.

Pruning rule: when a source audit fails on legitimate refactors (no behavior change) for the third time, do not patch the string again — rewrite it structurally, move it up the ladder, or delete it.

# Post-Task Checklist

After every task, before responding to the user:

- [ ] Run the smallest source, test, build, or runtime proof that can fail for the changed behavior.
- [ ] Use `./scripts/agentic/agent-cargo.sh` (not bare `cargo`) for any cargo invocation while `./dev.sh` may be running.
- [ ] Report any skipped verification and why it was skipped.
