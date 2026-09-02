For a map of main UI surfaces to code implementation, see [GLOSSARY.md]

# Before Starting Work

- **Do the work yourself.** Project flows are DISABLED (see "Project Flows — Disabled" below). Do not delegate to `md flows/<name>.md`. Read the source, make the change, and verify it in this session.
- Inspect the relevant source, tests, and repo-local skills before editing.
- Prefer current code and generated artifacts over stale notes or memory.
- Keep edits narrowly scoped and verify them with the smallest check that can fail for the changed behavior.
- Keep tool-facing root docs in place: `README.md`, `CLAUDE.md`, `AGENTS.md`, and `.impeccable.md`.

## Oracle / Packx Bundle Context

For Oracle review or `oracle-packx` work in this repository, include the repo process context in the bundle or prompt unless the user explicitly excludes it: `AGENTS.md`, the owning `.agents/skills/<skill>/SKILL.md`, and relevant source, tests, generated contracts, and verification notes.

For runtime/UX bugs headed to `oracle-packx-conversation`, gather the investigation receipts yourself first — intake, primitive stack, measurements, classification, likely owner, red/green proof plan — and include them in the bundle. The `script-kit-devtools` skill and the probes under `scripts/devtools/` and `scripts/agentic/` are the tools for that; run them directly.

If a `packx` preview with include globs unexpectedly matches `0` files in this repository, rebuild the bundle from an explicit path list instead of widening blindly. A reliable workaround is:

```bash
rg --files <scope> | rg '<owners-or-patterns>' > /tmp/script-kit-gpui-packx-files.txt
xargs packx --preview --no-interactive -x "**/CLAUDE.md" < /tmp/script-kit-gpui-packx-files.txt
xargs packx --limit 900k --strip-comments --minify -f markdown --no-interactive --stdout -x "**/CLAUDE.md" < /tmp/script-kit-gpui-packx-files.txt > ~/.oracle/bundles/<slug>.txt
```

Use this when directory/include-glob matching undercounts relevant files; keep `CLAUDE.md` excluded and verify the preview count plus final non-empty bundle before consulting Oracle.

## Project Flows — Disabled

**Flows are OFF. Do not route work through `md flows/<name>.md`.**
Disabled 2026-07-25 by the repo owner. Do the work yourself, in this session.

Why: delegation kept producing the appearance of progress without progress.
Lanes and subagents sat for hours showing running timers while finished or
dead; one subagent died three times on a harness model-resolution error; a
"stuck flow" turned out to be a misread. The failure mode is consistent — the
delegating agent cannot see whether the delegate is working, so it reports
motion it has not verified. Direct work removes the gap between doing and
knowing.

What this means in practice:

- Read the source, make the change, run the check, report the receipt. No
  `md flows/...` calls.
- Do not fan out to subagents for work you can do yourself. A subagent's
  self-report is not proof; if you use one at all, verify its claims against
  the tree before repeating them.
- The routing rules that used to live here were a map of who owns what. That
  map is still useful — it now lives in `flows/README.md` and in the flow files
  themselves. **Read them as documentation of ownership, not as dispatch.**
  `GLOSSARY.md` maps UI surfaces to implementation files and is usually the
  faster start.
- The `flows/**` files stay on disk. They are not deleted, and their prompt
  content is still a good statement of each area's contract and gotchas —
  `flows/escape.md`'s three-lockstep-keyboard-paths warning, for example, is
  real and load-bearing whether or not a flow ever runs again.

Re-enabling is the owner's call, not an agent's. If flows come back, the thing
that has to change first is observability: a delegating agent must be able to
tell working from finished from dead without asking a human to look at a pane.

## Domain Crate Boundaries

- The app may depend on workspace domain crates under `crates/sk-*`; domain crates must never depend on `script-kit-gpui`.
- App-independent protocol primitives belong in `crates/sk-protocol/**`. Keep app services and compatibility adapters under `src/protocol/**` while migration is in progress.
- Move pure unit tests with their domain implementation so `./scripts/agentic/agent-cargo.sh test -p <crate>` does not link GPUI.
- For each extraction, verify the domain dependency tree and preserve the existing app-facing path with a temporary re-export when callers still rely on it.
- Update `GLOSSARY.md` (and the ownership notes in `flows/README.md`, kept as documentation) whenever a domain moves into `crates/**`, so the next agent can still find the owner.

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

The locked production contract is (entry retuned 2026-07-26 to the
Spotlight visible tail — Oracle session `glass-entry-spotlight-retune`,
user-authorized, evidence: the frame-by-frame measurement page
https://eager-hollow-dyyf.here.now/ and the before/after receipts in
`.artifacts/glass-entry-spotlight-retune/`; lineage 09bddd931 → eb8e1e115 →
cd5634ec8; exit/material values remain from the 2026-07-24 calibration
against `CleanShot 2026-07-24 at 09.18.40.mp4`):

- **2026-07-27 SPEED SCALE (authorized): every entry duration is HALVED**
  relative to Spotlight's measured wall clock, at explicit user request
  ("make the animate in like 2x faster"). Shape, ratios, curves, geometry,
  and alpha semantics are Spotlight's; only the tempo departs. Restoring
  1:1 Spotlight timing = doubling every entry duration below;
- default entry duration (visible tail) `0.105s`: `35ms` ease-out
  compression, NO explicit hold, `70ms` ease-in-out rebound. The
  compression ease-out ends at zero velocity and the rebound begins at zero
  velocity — that turn, not a dead hold, is the physical settling;
- material onset prefix `44ms` (glass Clear→Regular + tint ramp at a
  CONSTANT `0.85` NSWindow alpha, curve `(0.18,0)/(0.14,0)`); GPUI content
  roots fade in WITH the material from the first photon — hold `0ms`, fade
  `44ms`, ending exactly at tail start (2026-08-13 content-timing retune:
  the 57fps Spotlight reference shows content faintly present from frame 1,
  and the prior 26ms hold produced readable empty-body frames once the
  native footer stopped enrolling in the content fade). **2026-08-13
  empty-window retune (user report: "the main window starts with an
  'empty' window"):** content roots seed at `0.21 ×` their natural alpha —
  Spotlight's measured first-photon presence (~21% of settled) — never at
  `0.0`, then fade to full (`GLASS_ENTRY_CONTENT_START_ALPHA`,
  `SCRIPT_KIT_GLASS_CONTENT_START` override). Total entry `149ms`
  = onset + tail (Oracle session `glass-entry-onset-v2`, measured from
  `CleanShot 2026-07-27 at 10.08.42.mp4`);
- entry inset `0.006` per side, producing a main shrink-in
  `103.05% → 101.2% → 98.7% → 100%` width path (Notes/Dictation stay
  `101.2% → 98.7% → 100%`) and an Actions/popup grow-in
  `98.8% → 101.3% → 100%` path. **2026-08-13 soft-materialize retune
  (user-authorized, measured from `CleanShot 2026-08-13 at 00.25.36.mp4`,
  57fps):** the MAIN window's first photon is `103.05%` wide (Spotlight's
  measured first photon), easing to the preserved `101.2%` visible-tail
  start over `18ms` inside the material prefix, with the main backdrop's
  onset defocus raised to `12pt → 0` resolved across the `44ms` prefix
  (`GLASS_MAIN_ONSET_START_WIDTH_SCALE`, `GLASS_MAIN_ONSET_GEOMETRY_DURATION`,
  `GLASS_MAIN_ENTRY_BLUR_RADIUS`; popups/secondary keep the shared `8pt`
  full-entry ramp. Each floating footer `NSGlassEffectView` capsule uses the
  same main `12pt → 0` defocus across `44ms`, applied per capsule with
  `masksToBounds`, AND — 2026-08-13 capsule material parity, user report:
  the buttons "don't match the blur of the main window" — the same
  Clear→Regular + tint material ramp as the main backdrop across the `44ms`
  prefix, with each capsule's own foreground contentView joining the shared
  content fade at the `0.21` presence floor; the footer container, hints
  host, and transparent inter-capsule gaps remain filter-free and never
  fade).
  NSWindow alpha below 0.85 still exposes desktop pixels, so the sub-0.85
  presence prefix remains deliberately omitted — the wider+blurrier first
  photon at the 0.85 floor is the safe reproduction. Height participation
  is `0` (vertical damping 0.0 — Spotlight measures ±0–2px). Squish factor
  stays `0.25`, clamped `0.0065–0.015` per side; the default hits the
  0.0065 minimum = Spotlight's measured `−1.3%` total squish;
- entry alpha: visible start `0.85`, easing to `0.99` over `18ms`
  (ease-out), holding the model value at `0.99` through max compression,
  then easing `0.99 → 1.0` over `26ms` from rebound start. A shrink-in
  frame must NEVER be fully opaque while wider than natural size;
- the `0.85` floor lineage: retuned from `0.0` on 2026-07-25 (HITL
  submission `98cab5e5-6f15-4311-8d49-83e31602e641` / Oracle plan
  `floating-capsule-entry-material` — NSWindow alpha multiplies every
  contributed pixel, so low-alpha visible frames displayed mostly
  wallpaper). Truly hidden parking (window ordered out) stays alpha `0.0`
  via `GLASS_HIDDEN_PARK_ALPHA`; zero-alpha parking of a visible window is
  a contract violation (runtime tripwire
  `glass_hidden_park_on_visible_window`);
- phase-one fraction `1/3`, squish hold `0.0s`, fade fraction `2/3`;
- Notes body reveal uses the material-safe anchor, ABSOLUTE from native
  configure: onset + `max(geometric crossing 11ms, alpha ramp 18ms)` =
  `62ms`, then keeps its `90ms` body fade;
- glass material: stability tint floor `0.35` and capsule veil `0.0`
  (`src/ui/chrome/tokens.rs`) — the veil was removed in the user-authorized
  Jul 27 calibration; the older `0.80` policy text was stale. The Jul 23
  `0.55`/`0.94` stack read near-solid mid-entry, visibly heavier than the
  Spotlight reference fade;
- detached main-window exit is fixed-frame fade-only;
- popup exit duration `0.12s`, removal delay `135ms`, grow x/y `0.03/0.012`,
  shrink x/y `0.05/0.035`, and blur radius `8.0`.

Owning sources and anti-drift evidence:

- defaults: `src/theme/opacity.rs`;
- native physics/lifecycle: `src/platform/secondary_window_config.rs`;
- clipped footer capsule inventory: `src/footer_popup.rs`;
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
bun test ./scripts/devtools/glass-entry-motion-contract.test.ts \
  ./scripts/devtools/glass-lifecycle-filmstrip.test.ts \
  ./scripts/devtools/rapid-toggle-stress.test.ts
./scripts/agentic/agent-cargo.sh test --lib \
  platform::secondary_window_config_tests::glass_motion_fixture_matches_the_measured_production_calibration
```

For runtime/visual changes, also run the lifecycle, Actions-entry, and rapid
toggle probes with `SCRIPT_KIT_TEST_STATUS=1` and the named calibration fixture.
An `INVALID_INTERFERENCE` receipt means rerun when input is quiet; it is not a
product failure and must not be converted into a green result.

**Rainbow-backdrop capture sessions are DISABLED by owner request
(2026-08-13).** The full-screen backdrop fixture
(`scripts/agentic/macos-glass-background-fixture.swift`) and every probe that
launches the app over it — `glass-motion-contrast.ts --all`/`--mode locked`,
`glass-smoke-study.ts` live capture, and `glass-entry-abba.sh` — refuse to run
(fail closed) unless `SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER=1` is set. These runs
cover the operator's screen and drive real input; only set the opt-in for a
deliberate, user-approved capture session. Do not weaken or work around the
gate; `glass-smoke-study.ts --dry-run` and all grade-only paths stay available.

## Agent Chat Entry Context Contract

Launcher-sourced Agent Chat opens stage the currently selected launcher row as a context chip (and pre-fill the composer with its `@cmd:` mention) UNLESS the entry request suppresses the focused part. The default selection on a fresh launcher is a real, resolvable row (`first_selectable_index` — a Brain Inbox capture, a flow, a script), so any "just open a clean chat" affordance wired through the default entry silently inherits it. This bit us on 2026-07-10: main-hotkey double-tap opened Agent Chat with the first row attached as if deliberately chosen.

- Quick/clean-chat affordances (double-tap of the main hotkey, and anything with the same "I just have a quick question" intent) MUST route through `AgentChatEntryRequest::quick_question()` / `open_agent_chat_for_quick_question`, or otherwise pass `suppress_focused_part: true`.
- Only entries where the user deliberately targeted a row (Cmd+Enter on a selection, actions payloads, explicit handoffs) may stage the focused row — and Cmd+Enter already suppresses the *default auto-selected* row via its empty-input guard in `agent_handoff/mod.rs`. Mirror that guard's intent in any new launcher-sourced entry.
- The contract is locked by `quick_question_entry_suppresses_all_implicit_context` in `src/app_impl/agent_handoff/agent_chat_entry.rs`; keep new entry points honest against it rather than weakening the test.

# Agent Cargo Wrapper

Every explicit `bun test` file or directory MUST start with `./`, `../`, or
`/`. An unrooted argument such as `bun test scripts/devtools/example.test.ts`
enters Bun's repository-wide filter mode instead of executing the named path
directly. On Bun 1.3.14/macOS, that scan can exceed Darwin's 10,240-descriptor
limit, spike system load, and silently lose child-process stdout/stderr; the
resulting empty or failing evidence is invalid. Use
`bun test ./scripts/devtools/example.test.ts` instead.

`./dev.sh` runs `cargo watch` on the shared `target/` dir continuously. Bare `cargo build/test/check/clippy` from an AI agent contends on `target/.cargo-lock` and stalls for minutes ("Blocking waiting for file lock on build directory").

All agent-driven cargo invocations MUST go through `./scripts/agentic/agent-cargo.sh`, which defaults to the bounded shared `CARGO_TARGET_DIR=target-agent/pools/agent-debug` pool with a visible lock. Examples:

- `./scripts/agentic/agent-cargo.sh test --lib notes_editor::spine`
- `./scripts/agentic/agent-cargo.sh check --lib`
- `./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui`
- `./scripts/agentic/agent-cargo.sh test -p sk-storage` for app-independent private persistence; this never compiles GPUI, Metal, Whisper, or ONNX.
- `SCRIPT_KIT_ARTIFACT_REFERENCE=<reference.json> ./scripts/agentic/reuse-rust-test-binary.sh <reviewed-filter> [additional-filter...]` to rerun an already-built, source-current application test harness without Cargo or relinking. The explicit immutable reference is required for this no-Cargo guarantee; omitting it allows a build through the facade. The runner validates every identifier-only filter, rejects option injection and stale or ambiguous artifacts, forces noninteractive/no-input/no-capture operation, disables heavyweight stress corpora, and pins the harness to one thread.
- `SCRIPT_KIT_AGENT_TIMINGS=1 ./scripts/agentic/agent-cargo.sh test --lib --no-run` to save Cargo's critical-path HTML and the adjacent machine-readable `cargo-timing-summary.json`.

The managed lane uses only `target-agent/pools/agent-debug`. Non-default pools and exclusive per-agent target directories are refused; do not mint a pool per task or bypass the wrapper while `./dev.sh` may be running. The human watcher's separate `target/` and profile defaults remain untouched.

Builds and Rust test harnesses default to **noninteractive operation and two workers**. Direct wrapper invocations reject inherited screen-takeover, native-input, screen-capture, visible-window, live-AI, and application-launch permissions before Cargo starts; only explicit `SCRIPT_KIT_NONINTERACTIVE=0` opts into intentional interactive operation. `CARGO_BUILD_JOBS`, `RUST_TEST_THREADS`, `--jobs`, `-j`, and `--test-threads` remain explicit overrides only within the `SCRIPT_KIT_AGENT_MAX_JOBS` ceiling; an intentional interactive higher limit must raise that ceiling too. Noninteractive execution can never exceed two workers and forcibly disables inherited heavyweight search/storage stress corpora. Malformed safety modes, worker counts, or over-limit settings fail before Cargo starts.

The resource policy covers **all of `target-agent`**, including the pool, shared caches, immutable exports, pending publications, quarantine, and unknown entries: `SCRIPT_KIT_AGENT_TARGET_BUDGET_GB` defaults to 40 and `SCRIPT_KIT_AGENT_MIN_FREE_GB` to 25. Admission, monitored Cargo execution, and publication checks fail closed on incomplete measurements or resource refusal; `SCRIPT_KIT_AGENT_ALLOW_LOW_DISK=1` cannot bypass this managed policy, even in interactive mode. Monitoring is sampled cancellation, not a filesystem quota or unique-APFS-extent measurement. It never automatically evicts data. Stop a refused batch rather than raising the ceiling, bypassing the guard, or deleting the default pool/shared caches. Explicit retention requires current revisions and exact authorized candidate selections; live owners and unresolved references remain protected.

The wrapper owns `CARGO_TARGET_DIR` and effective compiler configuration. Direct `--target-dir` and `--config` overrides are rejected before any cache, lock, or compiler is touched because they can redirect a supposedly protected build into the live dev target or replace its bounded worker policy. Managed roots, leases, manifests, and output paths require exact owned identities; traversal, symlinked managed paths, and stale revisions fail closed.

Noninteractive Cargo tests must explicitly select `--lib`, named `--test` suites, or the reviewed `sk-clipboard` / `sk-protocol` / `sk-storage` domain packages. Blanket test discovery, ignored-test activation, `system-tests`, `--all-features`, broad target selectors, application launches, browser-opening docs, live benchmarks, Cargo aliases, and unreviewed external subcommands are rejected before Cargo starts; the non-GUI `run --bin export_design_tokens` exception remains available. Existing ignored tests really do send native keystrokes, alter selected text/clipboard, open System Settings, and call live AI providers, so environment flags alone are not sufficient protection.

Local `scripts/verify.sh` invocations use the same bounded agent wrapper by default, so release checks cannot silently bypass the protected pool, free-disk floor, or workstation resource policy. Only a GitHub-hosted runner declaring both `CI=true` and `GITHUB_ACTIONS=true` retains its separately isolated bare-Cargo target. The top-level verifier independently permits at most two compiler workers and two Rust-test threads and disables heavyweight stress corpora, including when an explicit `SCRIPT_KIT_CARGO` override is supplied.

`scripts/agent-check.sh` is always noninteractive: inherited desktop, native-input, capture, visible-window, live-AI, and application-launch opt-ins are refused before Cargo starts. Its reviewed verification targets are the app library and the `sk-clipboard`, `sk-protocol`, and `sk-storage` domain crates; never replace them with blanket `cargo test`, unrestricted integration discovery, or expensive `clippy --all-targets`.

`scripts/agentic/build-isolated-binary.sh` must own one dedicated build process group: a timeout terminates its Cargo/rustc descendants rather than orphaning a live compiler, and failed builds still emit their structured JSON receipt. Timeouts and pool/agent/session identities are validated before any child starts; traversal-like identifiers may never stage a binary outside its owned runtime directory.

`scripts/agentic/start-isolated.sh` and `scripts/agentic/devtools-session.sh` must preserve exact readiness/preflight exit codes; never capture `$?` after `if ! command`, because the negation turns the real failure into success. The canonical DevTools entry point reserves stdout for exactly one final JSON envelope and routes intermediate startup/status output to stderr.

`dev-watch` and every preexisting named session are borrowed operator state, never agent-owned cleanup targets. Reusing either returns `cleanup.command: null`; direct isolated startup cannot claim `dev-watch`, and `devtools-session.sh cleanup` requires both `--expected-pid` and `--expected-generation` from the same new-session receipt. Failure cleanup forwards the same exact identity to `session.sh stop` and never stops a preexisting session. Session names, positive-whole timeouts, modes, and build policies fail closed before preflight; readiness markers count only while their owning session is actually alive.

The canonical `scripts/devtools/inspect.ts` orchestrator must follow the same ownership contract: a resumed or borrowed session never receives cleanup authority, and a newly created session emits a stop command only after proving its exact PID and generation. Failed or unready startup and failed show requests are terminal; never discard their result and continue collecting misleading inspection receipts.

All agentic scenario, target-thread, and filterable-surface subprocesses cross the shared noninteractive subprocess guard before `Bun.spawn`: only reviewed read-only session transports are permitted, while AppleScript, Swift, native input, screen capture, arbitrary shell commands, and weakened child safety authority fail closed. Scenario cleanup, the filterable matrix, both surface navigators, and Notes use an in-memory owned-session registry; resumed sessions are borrowed, and newly created sessions can be stopped only with their original PID and generation. A pending-ready process may retain cleanup identity without converting its genuine `ready: false` receipt into a successful readiness proof.

The direct native leaf tools follow the same policy even when invoked outside a scenario: `macos-input.ts key|type|click` and `window.ts focus|capture|status|list|find` fail with `NONINTERACTIVE_SAFETY_REFUSED` before launching System Events, AppleScript, Swift, `cliclick`, or a window/full-screen screenshot. Pure `macos-input.ts check` and help stay available. The automation-window transport and navigator's independent screenshot subprocess are guarded before spawn, and the actual native-input behavior suite is mandatory in release proof.

`scripts/agentic/verify-shot.ts` is itself a native screen-capture owner. In noninteractive mode its default OS, app-render, and automatic-fallback capture paths fail with `NONINTERACTIVE_SAFETY_REFUSED` before showing a window, reading MCP credentials, contacting a provider, creating a screenshot directory, or starting any child. `--skip-screenshot` remains available for reviewed passive evidence and never creates the screenshot directory; every internal session/native subprocess independently crosses the same shared guard.

Every DevTools session consumer must validate the actual lifecycle receipt before follow-up work: start requires `status: "ok"`, the exact requested session, `ready: true`, and explicit borrowed/new ownership; show requires an exact successful session receipt. Shared target resolution, Actions, Agent Chat, Dictation, and Events all fail before inspection, capture, log collection, or another child when startup/show fails. Subprocess environment overrides must cross `assertNoninteractiveSubprocess`; no child may weaken inherited noninteractive, CI, native-input, screen-capture, or launch authority. All four independent consumer owners are mandatory release sources.

Visible/native proof entrypoints must call `assertNoninteractiveVisualProbe` before resolving an application binary, creating/deleting output directories, compiling Swift helpers, activating/focusing windows, injecting keyboard/pointer input, capturing pixels, or starting any child. Both Notes resize probes, Actions/glass lifecycle filmstrips, rapid-toggle stress, the live glass observer aggregator, Spotlight live capture, and main-window native drag fail closed under `SCRIPT_KIT_NONINTERACTIVE=1`; pure imported analyzers, `classify-synthetic`, and Spotlight `--grade-only` remain available. The main-window inspector validates exact successful start/show lifecycle receipts. Every actual visual owner is mandatory release provenance. This protects the operator without changing the locked glass-motion calibration.

System clipboard reads are native operator-data access, even when a probe promises to restore the value. Flow multiline, Dictation History, Conversation Hosts, and Notes Actions must reject noninteractive runs before output creation, `pbpaste`/`pbcopy`, or compiling a helper that archives every private `NSPasteboard` representation; never rely on a later `Driver.launch` guard. Every one of the fifteen `scripts/agentic/cons-flow-ux/*-probe.ts` workflow owners rejects noninteractive execution at its own entrypoint before deleting an existing authoritative runtime receipt, creating shared state or fixtures, hashing an application, observing pasteboard changes, launching a scenario child, or importing live AI credentials. Root-search visual stability, Notes glass fallback, browser DOM/screenshot capture, AppKit window fixtures, Swift helper identity/compilation, global keyboard/pointer monitoring, glass contrast, and live smoke studies also fail before application startup, filesystem mutation, clipboard access, native subprocesses, capture, or backdrop takeover. Pure helper-key/interference/fixture analyzers, browser help, and smoke-study `--dry-run` remain available. The central agentic recipe orchestrator validates every child (including standalone navigator/vision delegation), and the DevTools dispatcher rejects activation, launch, profiling, start/show/open routes before delegation. Direct `xctrace` recording/export refuses while noninteractive; command inventories and help stay passive. Missing optional scenario owners must yield a typed, fail-closed receipt and must never prevent the other 126 agentic commands or `help --json` from loading.

The wrapper qualifies installed `sccache` against the pinned compiler, preserves semantic compiler wrappers, and uses repository-owned compiler/shader caches under `target-agent/shared`. Auto mode reports an unusable cache honestly; required caching fails rather than silently replacing a semantic wrapper or pretending to be enabled. **Every managed build forces `CARGO_INCREMENTAL=0` and Cargo build/dev/test incremental settings to `false`**, regardless of inherited environment defaults. Conflicting forced configuration and incremental rustflags are refused. Do not change the human watcher's incremental/profile defaults to enforce this agent-only policy. Dev UI dependencies intentionally stay optimized, correctness-test dependencies are unoptimized, and local docs-only Git commits do not invalidate the app harness; release/CI Git provenance remains exact.

Use `bun scripts/devtools/devtools.ts build-ops act app-build --artifact-out <new-reference.json>` (or `libtest-build` / `exporter-build`) for immutable artifacts, adding the features the actual task requires. Consumers use the returned `ArtifactReference` and verified manifest, never an mtime-selected binary, mutable named export, or hand-written path alias. An unchanged warm build reuses its exact verified publication. A fresh evidence directory, CLI-only edit, or different test filter does not require new binaries; retain and reuse current references. Discover the keep-set/prune input schemas, query their current revisions, and select only this task's authorized obsolete outputs. See the README's owned native workflow for complete commands, recovery, and proof limitations.

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
