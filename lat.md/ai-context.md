# AI Context and MCP

Script Kit GPUI resolves AI context from schema-versioned snapshots, attachments, and portal-backed target selection.

The current code still uses compatibility names like `tab_ai_*`, but the live contracts are the context snapshot types, MCP resources, and `AiContextPart` resolution.

## Desktop snapshot

`CaptureContextOptions` in `src/context_snapshot/types.rs` controls which parts of desktop state are captured.

The important current profiles are:

- `all` for full metadata without screenshots
- `recommendation` for lightweight composer hints
- `minimal` for the leanest context resource path
- `tab_ai_submit` for submit-time ACP/Tab AI capture without screenshots
- `tab_ai` for the only built-in profile that enables both screenshot fields

`AiContextSnapshot` is schema version 4 and carries optional desktop fields plus warnings.

The current resource code also emits diagnostics wrappers with per-field status when `diagnostics=1` is requested.

## MCP context resources

`kit://context` and `kit://context/schema` are the main AI-facing MCP resources.

The current parser accepts `profile=minimal`, `profile=full`, `diagnostics=1`, and per-field flags such as `selectedText`, `frontmostApp`, `menuBar`, `browserUrl`, `focusedWindow`, `screenshot`, and `panelScreenshot`.

That is the current contract, not just a raw snapshot. The schema URI is self-describing, and the context resource keeps the query URI intact so diagnostics can reflect the effective profile.

Related MCP resources that matter for AI flows are `kit://focused-item` for the current resolved target, `kit://clipboard-history` for clipboard context, `kit://scripts` and `kit://scriptlets` for metadata envelopes, and `kit://sdk-reference` for the harness and SDK contract.

## Typed context parts

`AiContextPart` is the attachment model in `src/ai/message_parts.rs`.

The live variants are:

- `ResourceUri` for MCP resources such as `kit://context?profile=minimal`
- `FilePath` for local file attachments
- `SkillFile` for staged skill content
- `FocusedTarget` for explicit surface-native targets
- `AmbientContext` for promoted Ask Anything chips
- `TextBlock` for raw text, URLs, logs, or pasted snippets

Resolution happens at submit time.

Resource URIs become `<context>` blocks, files become `<attachment>` blocks, unreadable files fall back to metadata-only attachments, and failures are tracked in a `ContextResolutionReceipt` instead of aborting the whole submission.

`file_path_parts()` is the canonical helper for deriving attachment paths from the pending parts list, and the `ASK_ANYTHING_*` constants keep the ambient bootstrap path stable.

## Portal flow

ACP already wires portal callbacks to the attachment portal.

`open_attachment_portal()` routes to file search, clipboard history, notes browse, ACP history, or script/scriptlet/skill search, and the accepted selection becomes an `AiContextPart`.

That means context portalling is not just an idea in the docs anymore.

The code already preserves the return view, focus target, preview metadata, and the insertion path for inline context chips.

`ContextPreviewInfo` in `src/ai/window/context_preview.rs` is the synchronous metadata layer for those chips. It classifies context resources as minimal, full, custom, or file-backed without doing I/O.

## ACP handoff

`AcpChatView` is the live chat surface.

Current Tab-driven AI entry still uses compatibility-named helpers, but the live handoff is ACP plus `FocusedTarget` and the attachment portal pipeline. `open_tab_ai_acp_with_explicit_target()` is the clearest code path for a surface that wants to hand off a concrete target instead of letting the UI guess.

`QuickTerminalView` remains the PTY-backed wrapper for harness-native flows, but it is not the same thing as ACP chat. The stale wiki page treated those surfaces too loosely.

## Drift from older docs

The older AI-context docs were close in intent but not current in details. The main drift is:

- `tab_ai_submit` is text-safe and does not include screenshots.
- `tab_ai` is the only built-in profile that requests screenshot bytes.
- `kit://context` now supports diagnostics and per-field flags in addition to profile names.
- `FocusedTarget` and the attachment portal are live code, not only future design.
- `ContextPreviewInfo` gives the preview layer a stable current contract.

## Source files

Current code references for this page:

- [src/context_snapshot/types.rs](../src/context_snapshot/types.rs)
- [src/mcp_resources/mod.rs](../src/mcp_resources/mod.rs)
- [src/ai/message_parts.rs](../src/ai/message_parts.rs)
- [src/ai/window/context_picker/mod.rs](../src/ai/window/context_picker/mod.rs)
- [src/ai/window/context_preview.rs](../src/ai/window/context_preview.rs)
- [src/app_impl/attachment_portal.rs](../src/app_impl/attachment_portal.rs)
- [src/app_impl/tab_ai_mode.rs](../src/app_impl/tab_ai_mode.rs)
