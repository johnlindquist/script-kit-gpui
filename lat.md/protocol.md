# Protocol

Script Kit GPUI’s live protocol is split between a tagged JSONL app message enum and an HTTP MCP server. Together they expose prompt control, automation queries, and read-only resource access.

## Current shape

The protocol lives under `src/protocol/` and is assembled from `message`, `types`, `semantic_id`, `io`, `transaction_executor`, and `transaction_trace`.

`Message` is a serde-tagged enum built from prompt, system-control, query, and AI-related variant groups.

## Prompt and control messages

`Message` is a serde-tagged enum built from prompt, response, and system-control families.

The current message families cover the prompt surfaces (`arg`, `div`, `editor`, `fields`, `form`, `path`, `drop`, `hotkey`, `term`, `chat`, `mic`, `webcam`), response messages (`submit`, `update`), and system-control operations (`exit`, `show`, `hide`, window management, and UI update messages).

The `capabilities` module advertises handshake flags such as `submitJson`, `semanticIdV2`, `unknownTypeOk`, `forwardCompat`, `choiceKey`, and `mouseDataV2`.

## Query and introspection

The live query surface includes `getState`, `getElements`, `getLayoutInfo`, `captureScreenshot`, and scriptlet/file-search variants.

`getState` and `getElements` both accept an optional `target: AutomationWindowTarget`, so automation can inspect non-default surfaces explicitly.

`elementsResult` now returns the visible element list plus `totalCount`, `truncated`, `focusedSemanticId`, `selectedSemanticId`, and machine-readable `warnings`. That is the current contract, not just a basic element list.

## Deterministic transactions

`waitFor` and `batch` are built on `TransactionStateProvider` in `src/protocol/transaction_executor.rs`.

They operate on `UiStateSnapshot`, emit traces, and return structured failures with stable error codes instead of forcing callers to parse ad hoc logs.

This is the part of the protocol that keeps UI automation repeatable when a script needs to wait for focus, selection, or a particular semantic ID.

## MCP resources

MCP resources are the read-only side channel for state and metadata.

The current set includes `kit://state`, `scripts://`, `scriptlets://`, `kit://scripts`, `kit://scriptlets`, `kit://sdk-reference`, `kit://context`, `kit://context/schema`, `kit://clipboard-history`, `kit://focused-item`, `kit://git-status`, `kit://git-diff`, `kit://processes`, `kit://system`, `kit://dictation`, `kit://calendar`, `kit://notifications`, and the transaction resources.

The stale wiki wording only showed a smaller subset. The current code also supports context diagnostics, schema-versioned script and scriptlet envelopes, and transaction resource documents.

## MCP server

The MCP server is a separate HTTP entrypoint layered on top of the same app state and resource registries.

- It listens on `localhost:43210` by default and honors `MCP_PORT`.
- It uses bearer-token authentication from `~/.scriptkit/agent-token`.
- It writes discovery metadata to `~/.scriptkit/server.json`.
- The live RPC surface is JSON-RPC 2.0 over HTTP with `initialize`, `tools/list`, `tools/call`, `resources/list`, and `resources/read`.

That is the current durable server contract. The older doc framing around a tiny fixed resource set is stale.

## Tool exposure

Two different tool families are exposed through MCP:

- Built-in `kit/*` tools come from `src/mcp_kit_tools.rs`.
- Script-defined tools are generated from scripts that declare schema through the local SDK surface.

That means the MCP tool catalog is partly static and partly derived from the current script inventory, rather than being a hand-maintained list.

## Drift from older docs

The old protocol docs lag the live module split. In current code:

- `getState` and `getElements` both support explicit window targets.
- `elementsResult` includes focus, selection, truncation, and warnings.
- `kit://context` supports profiles, per-field flags, diagnostics, and a schema URI.
- `kit://sdk-reference` is schema-versioned and includes a harness workflow contract.
- `kit://state`, `scripts://`, and `scriptlets://` still exist as legacy aliases alongside versioned resources.

## Source files

Current code references for this page:

- [src/protocol/mod.rs](../src/protocol/mod.rs)
- [src/protocol/message/mod.rs](../src/protocol/message/mod.rs)
- [src/protocol/message/variants/query_ops.rs](../src/protocol/message/variants/query_ops.rs)
- [src/protocol/types/mod.rs](../src/protocol/types/mod.rs)
- [src/protocol/transaction_executor.rs](../src/protocol/transaction_executor.rs)
- [src/mcp_resources/mod.rs](../src/mcp_resources/mod.rs)
- [src/mcp_server/mod.rs](../src/mcp_server/mod.rs)
- [src/mcp_kit_tools.rs](../src/mcp_kit_tools.rs)
- [scripts/kit-sdk.ts](../scripts/kit-sdk.ts)
