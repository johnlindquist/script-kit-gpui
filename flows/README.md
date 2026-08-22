# Flows: the Script Kit GPUI agent roster

One markdown agent per job. Run one with `md flows/<name>.md "request"`.
Every run costs one engine turn. Dry runs are free:
`md flows/<name>.md --_dry-run`.

All flows run on codex (see ../.mdflow.yaml) at gpt-5.6-sol, medium reasoning.
Sandbox mode is pinned per flow in frontmatter.

Migrated from the retired imps/ fleet (codex-imps runtime) on 2026-07-04.

## App-independent domain ownership

- `crates/sk-storage/src/lib.rs` owns atomic/private file persistence and its
  focused regression tests. `src/atomic_file.rs` temporarily re-exports that
  domain for existing app callers; storage tests must not depend on the app,
  GPUI, Metal shaders, Whisper, or ONNX.

## Roster

- **[actions.md](./actions.md)** (workspace-write): Actions menu, trigger picker, confirm popup, action builders, and keyboard affordances.
- **[agent-chat.md](./agent-chat.md)** (workspace-write): Agent Chat portal, AI context picker, file attachment parity, context mentions, and Pi handoff.
- **[ai-core.md](./ai-core.md)** (workspace-write): AI capabilities outside Agent Chat: local LLM (ghost completion backend), dictation/whisper capture, computer use, OCR, camera, and AI vault.
- **[auditor.md](./auditor.md)** (read-only): Read-only audit sweeps: UX inconsistencies, shared-component bypasses, hardcoded theme values, policy violations; prioritized findings, never edits.
- **[brain.md](./brain.md)** (workspace-write): Day Page, Today behavior, brain markdown substrate, fragments, spine flows, and notes parity.
- **[build-doctor.md](./build-doctor.md)** (workspace-write): Build and toolchain medic: agent-cargo pools, cargo lock contention, target-agent disk budget, clippy and fmt debt, stuck or slow builds.
- **[builtins.md](./builtins.md)** (workspace-write): Launcher-accessible built-in utility surfaces: file search, app launcher, emoji, calculator, browser history, process manager, window switcher, permissions wizard.
- **[clipboard.md](./clipboard.md)** (workspace-write): Clipboard history, quiet sediment rules, post-copy tracker, and no-popup brain capture contracts.
- **[components.md](./components.md)** (workspace-write): Shared UI primitives, prompt shells, rows, forms, buttons, toasts, theme, chrome, and design tokens.
- **[devex.md](./devex.md)** (workspace-write): Repo process docs, project flows bootstrap, agent-cargo usage, dev probes, source-audit ratchets.
- **[devtools.md](./devtools.md)** (workspace-write): DevTools operator (converted from the script-kit-devtools skill): drives protocol/MCP/CLI primitives — driver library, inspect/investigate, elements/layout/scroll/focus/text/keyboard, events, red/green compare — to inspect, measure, and prove real app behavior; produces fail-closed investigation receipts that feed oracle-packx-conversation bundles.
- **[escape.md](./escape.md)** (workspace-write): Escape/dismiss medic: owns the cross-surface Escape grammar — the ScriptList escape ladder, the opened_from_main_menu origin flag, DismissPolicy, go_back_or_close vs close_and_reset_window, and the 'extra Escape needed' bug family.
- **[execution.md](./execution.md)** (workspace-write): Script discovery, metadata parsing, menu cache, scheduler, keywords, snippets, scriptlets, aliases, and execution lifecycle.
- **[gpui-vendor.md](./gpui-vendor.md)** (workspace-write): Vendored GPUI internals owner: vendor/gpui (list element, ListState, elements, window draw loop) and vendor/gpui-component (TextView/markdown, scrollbar, highlighter) — semantics questions, minimal semantics-preserving patches, and the source-audit tests in src that pin vendor source text.
- **[hotkeys.md](./hotkeys.md)** (workspace-write): Hotkey gesture classification, main-hotkey routing, shortcuts, focus restoration, and low-latency surface morphs.
- **[launcher.md](./launcher.md)** (workspace-write): Script list, main window, main menu, mini/full view, selection behavior, frecency, favorites, fallbacks, and shared main-window chrome. Escape/dismiss ladder bugs route to imp-sk-escape.
- **[mcp.md](./mcp.md)** (workspace-write): MCP server, protocol handling, resources, script tools, schema compatibility, and tests.
- **[migrate.md](./migrate.md)** (workspace-write): v1→v2 script migration engine: classifier, compat map, agent port pipeline, validator ladder, honesty pass, and the Migrate board built-in.
- **[perf.md](./perf.md)** (workspace-write): Runtime performance medic: reproduces lag/jank complaints with real input events, CPU-profiles the live app with /usr/bin/sample, computes draw-share red/green deltas, and owns the frame-cost playbook (dev-profile opt levels, per-frame allocation churn, measure storms).
- **[platform.md](./platform.md)** (workspace-write): macOS platform integration, app/window orchestration, panels, icons, tray/menu bar, permissions, and startup/Pi sidecar.
- **[prompts.md](./prompts.md)** (workspace-write): SDK prompt renderers and protocol-to-renderer contracts.
- **[release.md](./release.md)** (workspace-write): Release pipeline: version bumps matching both Cargo.toml version fields, v* tag flow, pre-tag clippy gate, CI release workflow health.
- **[scout.md](./scout.md)** (read-only): Read-only intake specialist for owner discovery, routing, and required context.
- **[screenshots.md](./screenshots.md)** (workspace-write): Marketing screenshot capture: regenerates the numbered "glamour" shot set for the scriptkit.com static site via the devtools driver and OS-level capture; owns the site/images naming contract and JPEG conversion.
- **[settings.md](./settings.md)** (workspace-write): Config and settings persistence, onboarding/NUX, kit store, sync, updates, login item, startup profile, and secrets.
- **[site.md](./site.md)** (workspace-write): scriptkit.com static marketing site under site/: page content, screenshot wiring, GitHub latest-release download links, local preview, Vercel deploys (domain cutover only with explicit user approval).
- **[terminal.md](./terminal.md)** (workspace-write): Terminal prompt rendering, PTY lifecycle, command bar UI, and terminal theme adaptation.
- **[tests.md](./tests.md)** (workspace-write): Test authorship and policy: enforcement-ladder placement, behavior tests over source audits, contract tests, ratchet maintenance, flaky-test diagnosis.
- **[videos.md](./videos.md)** (workspace-write): Marketing video capture: glamour demo-reel loops of the real app in use, built from storyboarded driver scenarios in scripts/agentic/glamour-video-probe.ts, recorded over a clean desktop, encoded to small autoplay MP4 loops; owns the site/videos contract.
