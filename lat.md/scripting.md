# Scripting

Script Kit GPUI runs TypeScript through Bun, but the local SDK is still a real product surface rather than a thin compatibility shim. The current scripting contract is split between `scripts/kit-sdk.ts`, the generated `kit://sdk-reference` MCP resource, and Bun-driven repo tooling.

## Key Facts

- `scripts/kit-sdk.ts` still documents itself as a message-passing SDK layered over the Rust app.
- The generated `kit://sdk-reference` resource is the current concise source of truth for exposed script APIs, script directories, and harness workflow.
- The live SDK reference still includes Script Kit helpers such as `exec`, clipboard helpers, filesystem helpers, `getState`, `getElements`, `waitFor`, and `batch`.
- Bun is still used directly in repo-side tooling for process execution and config inspection, especially in `scripts/config-cli.ts` and the `scripts/agentic/` helpers.
- Script discovery is plugin-based. `~/.scriptkit/kit/main/scripts/` is the default personal plugin, and scriptlets are discovered from `~/.scriptkit/kit/*/scriptlets/*.md`.

## Key Files

- [scripts/kit-sdk.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/kit-sdk.ts) - Local SDK preload and the current message-passing runtime contract.
- [src/mcp_resources/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/mcp_resources/mod.rs) - Generated `kit://sdk-reference`, script, and scriptlet resources.
- [scripts/config-cli.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/config-cli.ts) - Bun-based CLI for inspecting and editing `~/.scriptkit/kit/config.ts`.
- [scripts/agentic/index.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/agentic/index.ts) - Bun-driven automation helpers used by the repo's verification flows.

## Source Documents

- [scripts/kit-sdk.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/kit-sdk.ts)
- [src/mcp_resources/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/mcp_resources/mod.rs)
- [scripts/config-cli.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/config-cli.ts)
- [scripts/agentic/index.ts](/Users/johnlindquist/dev/script-kit-gpui/scripts/agentic/index.ts)

## Related Pages

- [overview](./overview.md)
- [architecture](./architecture.md)
- [protocol](./protocol.md)

## Runtime Contract

- Scripts talk to the Rust shell through JSON messages rather than embedding UI logic locally.
- `kit://sdk-reference` is generated from Rust code, which makes it a better current reference than older narrative migration docs.
- Bun-native process helpers and the local SDK coexist. Repo tooling uses Bun directly where that is simpler, while scripts still rely on the Script Kit APIs for prompts, automation, clipboard, and app integration.
