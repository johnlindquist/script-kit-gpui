# Product Overview

Script Kit GPUI is a Rust GPUI launcher shell that runs TypeScript scripts through Bun, keeps its own launcher state in Rust, and exposes automation and context surfaces for scripts and AI tools.

## Key Facts

This section captures the current product shape and where the launcher shell starts.

- The current product is a native launcher shell with several utility surfaces layered on top of the same Rust app state.
- The app combines a Rust UI shell, a Bun-powered script runtime, and a local SDK in one repository.
- The local SDK still exposes a real Script Kit API surface, and the generated `kit://sdk-reference` resource is the clearest current summary of that surface.
- Bun-native APIs are used alongside the SDK in repo tooling and runtime helpers rather than replacing the SDK outright.
- `ScriptListApp` is the main launcher host; it routes the visible surface through `AppView`.
- Current user-facing surfaces include the script list, file search, clipboard history, app launcher, window switcher, notes, ACP chat, and the quick terminal.
- Notes is a separate floating window, but it can host an embedded ACP chat surface.
- The automation boundary is split between the JSONL protocol in `src/protocol/` and MCP resources in `src/mcp_resources/`.

## Key Files

These files anchor the current product story in source, not in the older wiki snapshot.

- [README.md](/Users/johnlindquist/dev/script-kit-gpui/README.md) - Public project entrypoint, setup, prompt APIs, configuration, and the current positioning of the rewrite.
- [scripts/kit-sdk.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/kit-sdk.ts) - Local SDK preload surface and Script Kit-specific script APIs.
- [src/mcp_resources/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/mcp_resources/mod.rs) - Embedded script reference content, including the concise SDK surface exposed to AI tooling.
- [src/main_sections/app_view_state.rs](/Users/johnlindquist/dev/script-kit-gpui/src/main_sections/app_view_state.rs) - `AppView` routing enum for launcher, prompt, utility, AI, and notes-related surfaces.
- [src/app_impl/startup.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/startup.rs) - Main keyboard interception and startup routing, including Tab-handling and window state transitions.
- [src/app_impl/tab_ai_mode.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/tab_ai_mode.rs) - ACP entry points, harness routing, and Tab AI compatibility plumbing.
- [src/notes/window.rs](/Users/johnlindquist/dev/script-kit-gpui/src/notes/window.rs) - Separate Notes window host and its embedded ACP surface.

## Source Documents

These are the live source files that back the overview.

- [README.md](/Users/johnlindquist/dev/script-kit-gpui/README.md)
- [scripts/kit-sdk.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/kit-sdk.ts)
- [src/mcp_resources/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/mcp_resources/mod.rs)
- [src/main_sections/app_view_state.rs](/Users/johnlindquist/dev/script-kit-gpui/src/main_sections/app_view_state.rs)
- [src/app_impl/startup.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/startup.rs)
- [src/app_impl/tab_ai_mode.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/tab_ai_mode.rs)
- [src/notes/window.rs](/Users/johnlindquist/dev/script-kit-gpui/src/notes/window.rs)

## Related Pages

These pages hold the deeper current details for routing, protocol, AI context, and verification.

- [architecture](./architecture.md)
- [scripting](./scripting.md)
- [protocol](./protocol.md)
- [ai-context](./ai-context.md)
- [acp-chat](./acp-chat.md)
- [windowing](./windowing.md)
- [verification](./verification.md)
