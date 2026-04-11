# Script Kit GPUI Knowledge Graph

This lattice captures the durable, code-backed knowledge for Script Kit GPUI while broader markdown migration is still in progress.

## Canonical entrypoints

Root tool-facing documents still matter here. `README.md`, `CLAUDE.md`, `AGENTS.md`, and `.impeccable.md` stay in place while the wider internal knowledge graph moves into `lat.md/`.

- [[overview]]
- [[architecture]]
- [[scripting]]
- [[protocol]]
- [[ai-context]]
- [[acp-chat]]
- [[design]]
- [[windowing]]
- [[verification]]
- [[distribution]]
- [[migration]]

## Current migration scope

The first wave replaces the authored wiki with current-code-backed lattice pages for the core product and contributor workflow.

It now covers overview, architecture, scripting, protocol, AI context, ACP chat, design, windowing, verification, and distribution. The old authored wiki and standalone `docs/` tree have been folded down into these code-backed pages. Historical plans, audits, research, and session artifacts stay outside the lattice until their durable facts are distilled.

## Current sources

These root entrypoints and live code files back the current lattice.

- [README.md](../README.md)
- [CLAUDE.md](../CLAUDE.md)
- [AGENTS.md](../AGENTS.md)
- [package.json](../package.json)
- [scripts/kit-sdk.ts](../scripts/kit-sdk.ts)
- [src/main_sections/app_view_state.rs](../src/main_sections/app_view_state.rs)
- [src/app_impl/tab_ai_mode.rs](../src/app_impl/tab_ai_mode.rs)
- [src/protocol/mod.rs](../src/protocol/mod.rs)
- [src/mcp_resources/mod.rs](../src/mcp_resources/mod.rs)
