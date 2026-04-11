# Architecture

Script Kit GPUI is split into a Rust launcher shell, prompt and utility view modules, a protocol boundary for script communication, and separate AI and Notes subsystems.

## Key Facts
The main shell is a routed Rust app, not a single flat window implementation.

- `src/main_sections/` holds the shared app state, view routing, and render dispatch that drive the launcher shell.
- `src/app_impl/` owns startup, keyboard routing, surface transitions, attachment portals, and most of the user-facing routing logic.
- `src/app_execute/` contains built-in execution and utility-view openers, including file search and terminal-style surfaces.
- `src/ai/` contains ACP chat, the harness/context plumbing, and the compatibility-named Tab AI code that still feeds ACP.
- `src/notes/` is a separate window subsystem rather than another `AppView` branch inside the launcher shell.
- `src/protocol/` and `src/mcp_resources/` define the script and AI automation boundary.

## Key Files
These are the live files that define the routing and module boundaries.

- [src/main_sections/app_view_state.rs](/Users/johnlindquist/dev/script-kit-gpui/src/main_sections/app_view_state.rs) - The `AppView` enum that names every first-class launcher surface.
- [src/main_sections/render_impl.rs](/Users/johnlindquist/dev/script-kit-gpui/src/main_sections/render_impl.rs) - Render dispatch for the current `AppView`.
- [src/app_impl/startup.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/startup.rs) - Main window startup and key interception.
- [src/app_impl/startup_new_tab.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/startup_new_tab.rs) - New-tab startup path and the same key-routing contract.
- [src/app_impl/tab_ai_mode.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/tab_ai_mode.rs) - ACP entry paths, harness routing, and Tab/Shift+Tab AI behavior.
- [src/app_impl/attachment_portal.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/attachment_portal.rs) - ACP attachment portal open/return flow for file search, clipboard history, notes, and related targets.
- [src/app_execute/builtin_execution.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_execute/builtin_execution.rs) - Built-in commands and the AI-related execution paths.
- [src/app_execute/utility_views.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_execute/utility_views.rs) - File search and quick-terminal utility surface helpers.
- [src/mcp_resources/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/mcp_resources/mod.rs) - MCP resource registry for current state, scripts, scriptlets, and context.
- [src/notes/window.rs](/Users/johnlindquist/dev/script-kit-gpui/src/notes/window.rs) - Separate Notes window host and embedded ACP surface.

## Source Documents
These code files are the source of truth for the current architecture description.

- [src/main_sections/app_view_state.rs](/Users/johnlindquist/dev/script-kit-gpui/src/main_sections/app_view_state.rs)
- [src/main_sections/render_impl.rs](/Users/johnlindquist/dev/script-kit-gpui/src/main_sections/render_impl.rs)
- [src/app_impl/startup.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/startup.rs)
- [src/app_impl/startup_new_tab.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/startup_new_tab.rs)
- [src/app_impl/tab_ai_mode.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/tab_ai_mode.rs)
- [src/app_impl/attachment_portal.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_impl/attachment_portal.rs)
- [src/app_execute/builtin_execution.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_execute/builtin_execution.rs)
- [src/app_execute/utility_views.rs](/Users/johnlindquist/dev/script-kit-gpui/src/app_execute/utility_views.rs)
- [src/mcp_resources/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/mcp_resources/mod.rs)
- [src/notes/window.rs](/Users/johnlindquist/dev/script-kit-gpui/src/notes/window.rs)

## Related Pages
These pages cover the adjacent product and contract details.

- [overview](./overview.md)
- [scripting](./scripting.md)
- [protocol](./protocol.md)
- [ai-context](./ai-context.md)
- [acp-chat](./acp-chat.md)
- [windowing](./windowing.md)

## Surface Routing
These routes are the current interaction paths that matter when you follow a keystroke through the app.

- `AppView` is the state machine for the main shell. Render dispatch and keyboard interceptors branch on it directly.
- `ScriptList` is the normal landing surface. From there the app can open utility views, built-ins, or AI paths.
- `Tab` from `ScriptList` routes into ACP context capture or AI handoff logic; `Shift+Tab` is still reserved in some surfaces such as file search and the AI harness path.
- ACP chat consumes `Tab` and `Shift+Tab` locally when it is open.
- `QuickTerminalView` receives raw Tab bytes so the PTY handler can own terminal navigation and shell interaction.
- Attachment portals temporarily replace the visible surface and then restore the originating ACP context on return.
- The Notes window is a separate host that can surface its own editor or an embedded ACP session.
